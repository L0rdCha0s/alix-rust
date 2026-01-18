#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

gdb-multiarch \
  -ex "target remote :1234" \
  -ex "symbol-file $ROOT_DIR/target/aarch64-raspi3/release/kernel" \
  -ex "layout asm"
