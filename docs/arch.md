# AArch64 Architecture

## Exception vectors
- Vector table lives in `src/arch/aarch64/exception.S` and is installed into VBAR_EL1.
- IRQ and sync handlers save a full trap frame to the current stack and dispatch into Rust.

## Trap frame
- `src/arch/aarch64/trap.rs` defines the trap frame layout used by the assembly code.
- `TF_SIZE` in `exception.S` must match the Rust struct layout and alignment.

## MMU bring-up
- `src/arch/aarch64/mmu.rs` configures MAIR/TCR/TTBR and enables the MMU.
- TTBR0 is currently used for the kernel mapping; TTBR1 is disabled.

## Timer
- `src/arch/aarch64/timer.rs` provides generic timer access and delay helpers.

## Notes
- QEMU and RPi5 differ in peripheral base addresses (see `src/platform/board.rs`).
- The kernel currently runs with identity-mapped virtual addresses.
