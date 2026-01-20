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

USE_MTOOLS=1
if [[ "$(uname -s)" == "Darwin" ]] && command -v hdiutil >/dev/null 2>&1; then
  USE_MTOOLS=0
else
  if ! command -v mformat >/dev/null 2>&1 || ! command -v mcopy >/dev/null 2>&1; then
    echo "error: mtools not found. Install mtools (mformat/mcopy) and retry." >&2
    echo "macOS: brew install mtools" >&2
    echo "Linux: sudo apt-get install mtools" >&2
    exit 1
  fi
fi

"$ROOT_DIR/scripts/build.sh"

KERNEL_IMG="$ROOT_DIR/target/aarch64-raspi5/release/kernel_2712.img"
OUT_IMG="$ROOT_DIR/target/aarch64-raspi5/release/ralix-rpi5.img"
OUT_CONFIG="$ROOT_DIR/target/aarch64-raspi5/release/config.txt"
OUT_CMDLINE="$ROOT_DIR/target/aarch64-raspi5/release/cmdline.txt"

if [ ! -f "$KERNEL_IMG" ]; then
  echo "error: kernel image not found: $KERNEL_IMG" >&2
  exit 1
fi

if [ "$SIZE_SET" -eq 0 ]; then
  FW_KB="$(du -sk "$FIRMWARE_DIR" | awk '{print $1}')"
  KERNEL_KB="$(du -sk "$KERNEL_IMG" | awk '{print $1}')"
  # Add 200 MiB slack for FAT metadata, cluster rounding, and firmware growth.
  NEED_KB=$((FW_KB + KERNEL_KB + (200 * 1024)))
  IMAGE_SIZE_MB=$(( (NEED_KB + 1023) / 1024 ))
  if [ "$IMAGE_SIZE_MB" -lt 256 ]; then
    IMAGE_SIZE_MB=256
  fi
fi
echo "Image size: ${IMAGE_SIZE_MB} MiB"

# Generate config/cmdline on disk so they are easy to inspect/edit.
cat >"$OUT_CONFIG" <<'EOF'
arm_64bit=1
enable_uart=1
enable_rp1_uart=1
dtparam=uart0=on
dtparam=uart0_console=on
kernel=kernel_2712.img
# Our linker script uses 0x80000 as the physical load base.
kernel_address=0x80000
# Pi 5: route UART0 to GPIO14/15 (header pins) instead of debug header.
dtoverlay=uart0-pi5
dtoverlay=disable-bt
# Firmware UART banner on GPIO14/15 for early bring-up.
uart_2ndstage=1
enable_rp1_uart=1
pciex4_reset=0
os_check=0
# Firmware-provided framebuffer (simplefb) for early HDMI output.
framebuffer_width=1280
framebuffer_height=720
framebuffer_depth=32
framebuffer_ignore_alpha=1
disable_overscan=1
EOF

cat >"$OUT_CMDLINE" <<'EOF'
EOF

if [ "$USE_MTOOLS" -eq 0 ]; then
  STAGE_DIR="$(mktemp -d)"
  # Disable extended attributes/resource forks for FAT image creation.
  export COPYFILE_DISABLE=1
  cp -R -X "$FIRMWARE_DIR"/. "$STAGE_DIR"/
  cp -X "$KERNEL_IMG" "$STAGE_DIR/kernel_2712.img"
  cp -X "$OUT_CONFIG" "$STAGE_DIR/config.txt"
  cp -X "$OUT_CMDLINE" "$STAGE_DIR/cmdline.txt"
  rm -f "$OUT_IMG" "${OUT_IMG}.dmg"
  hdiutil create -ov -size "${IMAGE_SIZE_MB}m" -fs "MS-DOS FAT32" -volname RALIX -layout MBRSPUD -srcfolder "$STAGE_DIR" -format UDRW "$OUT_IMG" >/dev/null
  if [ ! -f "$OUT_IMG" ] && [ -f "${OUT_IMG}.dmg" ]; then
    mv "${OUT_IMG}.dmg" "$OUT_IMG"
  elif [ -f "${OUT_IMG}.dmg" ]; then
    rm -f "$OUT_IMG"
    mv "${OUT_IMG}.dmg" "$OUT_IMG"
  fi
  rm -rf "$STAGE_DIR"
else
  dd if=/dev/zero of="$OUT_IMG" bs=1m count="$IMAGE_SIZE_MB" status=none
  mformat -i "$OUT_IMG" -F -v RALIX ::

  # Copy firmware contents (boot files, overlays, dtbs, etc.).
  mcopy -s -i "$OUT_IMG" "$FIRMWARE_DIR"/* ::
  mcopy -o -i "$OUT_IMG" "$KERNEL_IMG" ::kernel_2712.img
  mcopy -o -i "$OUT_IMG" "$OUT_CONFIG" ::config.txt
  mcopy -o -i "$OUT_IMG" "$OUT_CMDLINE" ::cmdline.txt
fi

echo "Wrote $OUT_IMG"
echo "Wrote $OUT_CONFIG"
echo "To flash (example): sudo dd if=$OUT_IMG of=/dev/rdiskN bs=4m conv=sync"
