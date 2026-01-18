# Memory Management Implementation Plan

This plan reflects the current state of the kernel and outlines the next steps to reach a robust, per-process virtual memory model.

## Current State (as of now)

- **Physical memory model**
  - DTB parsing for `/memory` and `/reserved-memory`.
  - Normalized physical memory map with region precedence.
  - Boot allocator (`bootalloc`) to carve early allocations from usable RAM.
  - Bitmap frame allocator (`frame`) for 4 KiB pages.

- **Paging**
  - MMU enabled with 4 KiB granule.
  - Identity-mapped RAM using 2 MiB blocks.
  - Device memory mapped in a fixed window (board base + 16 MiB).
  - **Shared address space** for kernel + user; EL0 RW access to RAM.

- **Heap**
  - Kernel heap uses `linked_list_allocator` backed by contiguous frames.
  - User allocations provided by syscalls (alloc/realloc/free).
  - Global allocator routes EL0 allocations to syscall path.

## Priorities and Rationale

1. **Stabilize paging and boot invariants**
   - Ensure all early boot code continues to work with MMU on.
   - Confirm that UART, MMIO, timers, and framebuffer remain mapped.

2. **Define a stable virtual memory layout**
   - Introduce a **higher-half kernel layout** and a **physmap** region.
   - Keep a minimal identity map temporarily for early boot or remove once stable.

3. **Introduce per-process address spaces**
   - Add a per-process page table (TTBR0) with user VA range.
   - Keep kernel mapped in TTBR1 only (shared across all processes).

4. **User heap backed by mapped pages**
   - Stop returning kernel heap pointers to EL0.
   - Syscalls should map user pages into the process VA range.

## Target Virtual Address Layout (proposed)

- **Kernel (TTBR1, higher half)**
  - `KERNEL_BASE` (e.g. `0xFFFF_0000_0000_0000`)
  - Kernel text/rodata/data/bss
  - Kernel heap
  - Physmap (direct map of RAM): `PHYS_MAP_BASE + paddr`
  - MMIO window mapped as Device memory

- **User (TTBR0, lower half)**
  - User text/data/bss (per process)
  - User heap (grows upward)
  - User stack (grows downward)

## Implementation Plan

### Phase 1: Harden identity-mapped paging

- Add explicit **mapping attributes** (NX for data, RX for text).
- Enforce proper MAIR/TCR configuration.
- Ensure exception vectors are mapped in kernel VA.
- Add helper to map/unmap ranges with flags (RW/RX, user/kernel, device/normal).

### Phase 2: Introduce a higher-half kernel mapping

- Move kernel virtual addresses to higher half.
- Keep temporary identity mapping during transition.
- Switch kernel code/data pointers to higher-half VA.
- Validate UART/MMIO and frame allocator via physmap.

### Phase 3: Per-process virtual address spaces (TTBR0)

- Add a `PageTableRoot` per process:
  - Kernel uses TTBR1 (shared).
  - User uses TTBR0 (per process).
- On context switch:
  - Load TTBR0 with the process page table.
  - Keep TTBR1 unchanged.
- Add TLB invalidation per process on switch.

### Phase 4: User heap + syscall integration

- Define a user heap region in user VA space.
- Update syscalls `alloc/realloc/free` to:
  - Allocate frames
  - Map into the user heap region
  - Return user VA pointers (not kernel direct pointers)
- Implement grow-on-demand if needed.

### Phase 5: Guard pages + protections

- Add guard pages for:
  - User stacks
  - Kernel stacks
  - Heap boundaries
- Enforce NX for data and kernel/user separation.

## Migration Plan: Identity → Per-Process VA

1. **Boot with identity map + kernel VA mapping simultaneously**
   - Temporarily keep identity mapping so early boot code works.
   - Add higher-half mapping and switch execution to it.

2. **Switch kernel to higher-half only**
   - Remove identity mapping once stable.
   - Use physmap for all physical access.

3. **Introduce TTBR0 per process**
   - Create page tables for each process (user VA range).
   - On context switch, load TTBR0 and flush TLB for user range.

4. **Restrict user access**
   - Only map user pages into TTBR0 with EL0 permissions.
   - Ensure TTBR1 pages are EL0-inaccessible.

5. **Update syscalls + allocator**
   - User allocations should map into user VA, not kernel VA.
   - Remove direct exposure of kernel heap pointers to EL0.

6. **Remove shared identity mapping entirely**
   - At this point, all access should flow through kernel VA and physmap.

## Immediate Next Steps

1. Add page table helpers that support:
   - 4 KiB page mappings
   - Permissions (RW/RX, user/kernel)
   - Device vs Normal memory
2. Define the higher-half layout constants in `mm/layout.rs`.
3. Switch kernel execution to higher-half VA while keeping identity mapping.
4. Implement per-process TTBR0 and switch in scheduler.
5. Update syscall allocators to use user VA mappings.

---

This plan is deliberately staged to reduce risk: stabilizing paging and MMU first, then higher-half kernel, then user address spaces, then finally user heaps and protection hardening.
