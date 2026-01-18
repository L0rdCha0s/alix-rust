# Interrupts

## Overview
- Exception vectors live in `src/arch/aarch64/exception.S`.
- IRQs and synchronous exceptions both save a full trap frame and then call into Rust.

## Key files
- src/kernel/interrupts.rs
- src/arch/aarch64/exception.S
- src/drivers/local_intc.rs
- src/arch/aarch64/timer.rs

## Flow
1. Vector stub saves registers and trap frame.
2. `irq_handler` dispatches timer IRQs and triggers scheduling.
3. `sync_handler` logs faults (ESR/FAR/ELR).
