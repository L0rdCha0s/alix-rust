# Build and Run

## Build (QEMU)
- `scripts/build-qemu.sh`
- Produces `target/aarch64-raspi3/release/kernel8.img`

## Run (QEMU)
- `scripts/run-qemu.sh`
- Uses raspi3b machine model, DTB from `rpi/firmware/boot`
- Logging: `QEMU_LOG` and `QEMU_LOG_FILE`

## Build (RPi5 image)
- `scripts/build-rpi5-image.sh`
- Builds a disk image containing firmware + kernel

## Notes
- Requires Rust nightly with `rust-src` and `llvm-tools-preview`
