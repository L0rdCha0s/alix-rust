# Ralix Kernel Documentation

This folder documents the major subsystems of the kernel. Each section links to the
primary source files and describes the current design and boot/runtime flow.

- boot.md: Boot flow, exception vectors, and early init
- arch.md: AArch64 specifics (MMU, traps, timer)
- memory.md: Memory map, allocators, paging
- process.md: Process model and context layout
- scheduling.md: Scheduler and run queue behavior
- interrupts.md: IRQ routing and handlers
- syscalls.md: Syscall ABI and dispatch
- vfs.md: VFS layout and file descriptors
- drivers.md: UART, mailbox, framebuffer, keyboard, local interrupt controller
- gfx.md: Font rendering and framebuffer console
- platform.md: Board configuration and addresses
- user.md: Userland entry and shell
- build.md: Build + QEMU run scripts

The long-term memory plan lives in docs/todo/memory.md.
