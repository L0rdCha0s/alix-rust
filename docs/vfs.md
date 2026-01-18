# VFS

## Overview
The VFS is ephemeral and reset each boot. It exposes:
- `/` root
- `/dev` (device nodes)

The current device nodes include:
- `/dev/fb0` (framebuffer)
- `/dev/kbd0` (keyboard)

## Key files
- src/kernel/vfs.rs
- src/drivers/framebuffer.rs
- src/drivers/keyboard.rs

## File descriptors
- Each process inherits stdin/stdout/stderr from its parent.
- The shell uses stdout for printing to the framebuffer.
