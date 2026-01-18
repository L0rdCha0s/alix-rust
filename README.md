# Ralix ARM64 Kernel (Raspberry Pi 5)

This is a minimal ARM64 (AArch64) kernel written in Rust. It targets Raspberry Pi 5 and prints `Hello, world!` to a 1280x1024 framebuffer using a small built-in terminal font, then halts.
The bundled font is intentionally tiny; extend `src/font.rs` to add more glyphs.

## Prerequisites

- Rust nightly via rustup
- `rust-src` + `llvm-tools-preview` components (installed by rustup)
- QEMU with `qemu-system-aarch64` (optional; Pi 5 itself is not emulated)

Toolchain setup example:

```bash
rustup toolchain install nightly --component rust-src llvm-tools-preview
```

If `rust-objcopy` is not on your PATH, the build script will use `llvm-objcopy` from the nightly sysroot.

## Build (Raspberry Pi 5)

```bash
scripts/build.sh
```

Output:
- `target/aarch64-raspi5/release/kernel` (ELF)
- `target/aarch64-raspi5/release/kernel_2712.img` (raw image for Pi 5)
- `target/aarch64-raspi5/release/kernel8.img` (copy for compatibility)

## Run on Raspberry Pi 5

1. Copy `target/aarch64-raspi5/release/kernel_2712.img` to the FAT boot partition.
2. (Optional) Add/verify in `config.txt`:

```
kernel=kernel_2712.img
```

The Pi 5 firmware defaults to `kernel_2712.img` if present; otherwise it falls back to `kernel8.img`.

## Run in QEMU (dev-only)

QEMU does not currently emulate Raspberry Pi 5, so this uses the `raspi3b` machine with a QEMU-specific build. The framebuffer output appears in the QEMU display window.
If the 1280x1024 request is rejected by QEMU, it falls back to 1024x768 for development.
The QEMU script boots with 4 cores (`-smp 4`) so you can see secondary-core bring-up logs.
`scripts/run-qemu.sh` always rebuilds the QEMU image unless you set `SKIP_BUILD=1`.

```bash
scripts/run-qemu.sh
```

You should see:

```
Hello, world!
```

If you want a serial fallback in QEMU, leave the `-serial stdio` option in the script and use the UART output path in `src/main.rs` (guarded by the `qemu` feature).

## QEMU build (optional)

```bash
scripts/build-qemu.sh
```

Output:
- `target/aarch64-raspi3/release/kernel` (ELF)
- `target/aarch64-raspi3/release/kernel8.img` (raw image)

## GDB (optional)

```bash
scripts/run-qemu-gdb.sh
```

In another terminal:

```bash
scripts/gdb.sh
```
