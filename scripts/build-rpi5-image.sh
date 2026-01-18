#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

FIRMWARE_DIR=""
IMAGE_SIZE_MB=64
SIZE_SET=0

usage() {
  cat <<'EOF'
Usage: scripts/build-rpi5-image.sh --firmware /path/to/firmware/boot [--size 128]

Creates a FAT32 disk image with Raspberry Pi 5 firmware + kernel_2712.img.
You can dd the resulting image to a disk (superfloppy layout, no partition table).

Required:
  --firmware  Path to the Raspberry Pi firmware "boot" directory (e.g. from the
              raspberrypi/firmware repo, or a Pi's /boot/firmware folder).

Optional:
  --size      Image size in MiB (default: 64).
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --firmware)
      FIRMWARE_DIR="${2:-}"
      shift 2
      ;;
    --size)
      IMAGE_SIZE_MB="${2:-}"
      SIZE_SET=1
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

if [ -z "$FIRMWARE_DIR" ]; then
  echo "error: --firmware is required" >&2
  usage
  exit 1
fi

if [ ! -d "$FIRMWARE_DIR" ]; then
  echo "error: firmware directory not found: $FIRMWARE_DIR" >&2
  exit 1
fi

if ! command -v mformat >/dev/null 2>&1 || ! command -v mcopy >/dev/null 2>&1; then
  echo "error: mtools not found. Install mtools (mformat/mcopy) and retry." >&2
  echo "macOS: brew install mtools" >&2
  echo "Linux: sudo apt-get install mtools" >&2
  exit 1
fi

"$ROOT_DIR/scripts/build.sh"

KERNEL_IMG="$ROOT_DIR/target/aarch64-raspi5/release/kernel_2712.img"
OUT_IMG="$ROOT_DIR/target/aarch64-raspi5/release/ralix-rpi5.img"

if [ ! -f "$KERNEL_IMG" ]; then
  echo "error: kernel image not found: $KERNEL_IMG" >&2
  exit 1
fi

if [ "$SIZE_SET" -eq 0 ]; then
  FW_KB="$(du -sk "$FIRMWARE_DIR" | awk '{print $1}')"
  KERNEL_KB="$(du -sk "$KERNEL_IMG" | awk '{print $1}')"
  # Add 20 MiB slack for FAT metadata and headroom.
  NEED_KB=$((FW_KB + KERNEL_KB + (20 * 1024)))
  IMAGE_SIZE_MB=$(( (NEED_KB + 1023) / 1024 ))
  if [ "$IMAGE_SIZE_MB" -lt 64 ]; then
    IMAGE_SIZE_MB=64
  fi
fi

dd if=/dev/zero of="$OUT_IMG" bs=1m count="$IMAGE_SIZE_MB" status=none
mformat -i "$OUT_IMG" -F -v RALIX ::

# Copy firmware contents (boot files, overlays, dtbs, etc.).
mcopy -s -i "$OUT_IMG" "$FIRMWARE_DIR"/* ::

# Ensure our kernel + config are present.
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

cat >"$TMP_DIR/config.txt" <<'EOF'
arm_64bit=1
enable_uart=1
kernel=kernel_2712.img
EOF

cat >"$TMP_DIR/cmdline.txt" <<'EOF'
EOF

mcopy -o -i "$OUT_IMG" "$KERNEL_IMG" ::kernel_2712.img
mcopy -o -i "$OUT_IMG" "$TMP_DIR/config.txt" ::config.txt
mcopy -o -i "$OUT_IMG" "$TMP_DIR/cmdline.txt" ::cmdline.txt

echo "Wrote $OUT_IMG"
echo "To flash (example): sudo dd if=$OUT_IMG of=/dev/rdiskN bs=4m conv=sync"
