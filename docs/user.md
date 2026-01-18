# Userland

## Overview
Userland currently consists of a simple shell running in user mode on real hardware
(or EL1 on QEMU until TTBR0/TTBR1 split lands).

## Key files
- src/kernel/user.rs
- src/user/shell.rs

## Shell behavior
- Prints a prompt (`$ `)
- Reads from stdin and echoes input
- On Enter, prints `String: <input>` using a heap-backed `String`
