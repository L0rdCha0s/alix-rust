# Platform

## Boards
- Raspberry Pi 5 (BCM2712)
- QEMU raspi3b (BCM283x)

## Key files
- src/platform/board.rs

## Notes
- Peripheral base addresses differ between platforms and are selected by feature flags.
- QEMU uses the raspi3b machine model and a DTB passed at boot.
