#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KERNEL="$ROOT_DIR/target/aarch64-raspi3/release/kernel8.img"
DTB_DEFAULT="$ROOT_DIR/rpi/firmware/boot/bcm2710-rpi-3-b.dtb"
DTB="${DTB_PATH:-$DTB_DEFAULT}"
QEMU_RAM="${QEMU_RAM:-1G}"
QEMU_LOG="${QEMU_LOG:-mmu,int}"
QEMU_LOG_FILE="${QEMU_LOG_FILE:-$ROOT_DIR/qemu.log}"
: > "$QEMU_LOG_FILE"
if [ "${SKIP_BUILD:-0}" != "1" ]; then
  "$ROOT_DIR/scripts/build-qemu.sh"
fi

if [ ! -f "$DTB" ]; then
  echo "error: DTB not found: $DTB" >&2
  echo "Set DTB_PATH to a valid raspi3 dtb (e.g. bcm2710-rpi-3-b.dtb)" >&2
  exit 1
fi

qemu-system-aarch64 \
  -M raspi3b \
  -m "$QEMU_RAM" \
  -smp 4 \
  -kernel "$KERNEL" \
  -dtb "$DTB" \
  -d "$QEMU_LOG" \
  -D "$QEMU_LOG_FILE" \
  -serial stdio \
  -monitor none
