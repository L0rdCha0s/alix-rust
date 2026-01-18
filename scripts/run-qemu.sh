#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KERNEL="$ROOT_DIR/target/aarch64-raspi3/release/kernel8.img"
if [ "${SKIP_BUILD:-0}" != "1" ]; then
  "$ROOT_DIR/scripts/build-qemu.sh"
fi

qemu-system-aarch64 \
  -M raspi3b \
  -smp 4 \
  -kernel "$KERNEL" \
  -serial stdio
