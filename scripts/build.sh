#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! command -v rustup >/dev/null 2>&1; then
  echo "error: rustup not found. Install rustup and the nightly toolchain with rust-src + llvm-tools-preview." >&2
  echo "example: rustup toolchain install nightly --component rust-src llvm-tools-preview" >&2
  exit 1
fi

RUSTC_BIN="$(rustup which --toolchain nightly rustc)"
TOOLCHAIN_BIN="$(dirname "$RUSTC_BIN")"

export PATH="$TOOLCHAIN_BIN:$PATH"
export RUSTC="$RUSTC_BIN"

rustup run nightly cargo build --manifest-path "$ROOT_DIR/Cargo.toml" --release \
  -Z build-std=core,compiler_builtins -Z build-std-features=compiler-builtins-mem

OBJCOPY_BIN=""
if command -v rust-objcopy >/dev/null 2>&1; then
  OBJCOPY_BIN="rust-objcopy"
elif command -v llvm-objcopy >/dev/null 2>&1; then
  OBJCOPY_BIN="llvm-objcopy"
else
  SYSROOT="$(rustup run nightly rustc --print sysroot)"
  HOST="$(rustup run nightly rustc -vV | awk -F': ' '/^host:/{print $2}')"
  CANDIDATE="$SYSROOT/lib/rustlib/$HOST/bin/llvm-objcopy"
  if [ -x "$CANDIDATE" ]; then
    OBJCOPY_BIN="$CANDIDATE"
  fi
fi

if [ -z "$OBJCOPY_BIN" ]; then
  echo "error: rust-objcopy/llvm-objcopy not found. Install llvm-tools-preview for your nightly toolchain." >&2
  echo "example: rustup component add llvm-tools-preview --toolchain nightly" >&2
  exit 1
fi

"$OBJCOPY_BIN" -O binary \
  "$ROOT_DIR/target/aarch64-raspi5/release/kernel" \
  "$ROOT_DIR/target/aarch64-raspi5/release/kernel_2712.img"

cp "$ROOT_DIR/target/aarch64-raspi5/release/kernel_2712.img" \
  "$ROOT_DIR/target/aarch64-raspi5/release/kernel8.img"
