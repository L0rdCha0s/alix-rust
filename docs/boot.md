# Boot

## Overview
Boot starts in `src/arch/aarch64/boot.S` at `_start`. The code:

1. Captures the DTB pointer (x0) for later parsing.
2. Drops to EL1 if started in EL2/EL3.
3. Installs the exception vector table.
4. Sets SP_EL1 and zeroes `.bss`.
5. Jumps into `kernel_main(dtb_pa)`.

Secondary cores park on `wfe` until released by core 0 via
`kernel::smp::start_secondary_cores()`.

## Key files
- src/arch/aarch64/boot.S
- src/main.rs
- src/kernel/smp.rs
- src/arch/aarch64/exception.S

## Early init order (current)
1. `uart::init()`
2. `mm::init(dtb_pa)` (memory map, boot allocator, frame allocator, paging, heap)
3. `process::init()` and `vfs::init()`
4. Framebuffer init attempts (QEMU retry loop or single try)
5. Spawn kernel idle processes + user shell process
6. Start secondary cores
7. `interrupts::init_per_cpu()` and `process::start_on_cpu(0)`

## Notes
- QEMU runs with a DTB passed by `scripts/run-qemu.sh`.
- The exception vectors are installed before most initialization so faults can be logged.
