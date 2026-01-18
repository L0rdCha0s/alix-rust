# Syscalls

## Overview
Syscalls use the AArch64 SVC mechanism.
Arguments are passed in x0..x2, syscall number in x8.

## Key files
- src/kernel/syscall.rs
- src/kernel/user.rs (user-side wrappers)
- src/arch/aarch64/exception.S

## Current syscalls
- open, read, write, close
- sleep_ms
- alloc, realloc, free

## ABI notes
- Return value is in x0.
- User-space wrappers in `kernel::user` are thin asm shims.
