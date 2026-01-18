# Memory Management

## Overview
Memory initialization is done in `mm::init` and has these stages:

1. Parse DTB memory ranges
2. Normalize the memory map (usable / reserved / mmio / kernel / bootinfo)
3. Initialize the boot allocator for early allocations
4. Initialize the frame allocator (physical pages)
5. Build identity-mapped page tables and enable the MMU
6. Initialize the heap allocator for dynamic allocations

## Key files
- src/mm/mod.rs: top-level init and logging
- src/mm/dtb.rs: DTB parsing into regions
- src/mm/region.rs: map normalization / merging
- src/mm/bootalloc.rs: early bump allocator
- src/mm/frame.rs: frame allocator
- src/mm/paging.rs: page tables and mapping
- src/mm/heap.rs: kernel heap allocator
- src/arch/aarch64/mmu.rs: MAIR/TCR/TTBR configuration

## Addressing
- Identity mapping is used currently (phys == virt).
- Device ranges are mapped as Device memory; RAM is mapped as Normal memory.

## Future work
- See `docs/todo/memory.md` for the migration plan to per-process VA spaces.
