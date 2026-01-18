# Scheduling

## Overview
The scheduler is a simple round-robin over a global run queue.
It is driven by timer IRQs and can run on all cores.

## Key files
- src/kernel/process/scheduler.rs
- src/kernel/interrupts.rs

## Current behavior
- Each CPU has a current process slot.
- Ready processes are pulled from the global run queue.
- On QEMU, logging is reduced to avoid serial spam.

## TODO
- Per-process virtual address spaces
- Priority scheduling
- Blocking/wakeup primitives
