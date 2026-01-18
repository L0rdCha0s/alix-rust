# Processes

## Overview
Processes are lightweight kernel-managed contexts with a saved trap frame.
Each process has:
- PID, name, state, mode (Kernel/User)
- Stack + saved context SP
- File descriptor table (inherited from parent or init)

## Key files
- src/kernel/process.rs
- src/arch/aarch64/trap.rs
- src/arch/aarch64/exception.S

## Context switching
- `TrapFrame` stores registers, ELR, SPSR, and SP_EL0.
- The scheduler switches by saving current state on IRQ entry and restoring the next.

## User vs kernel
- User processes are created via `create_user` and start at `kernel::user::user_start`.
- On QEMU, user processes run in EL1 for now; EL0 is planned once TTBR0/TTBR1 split lands.
