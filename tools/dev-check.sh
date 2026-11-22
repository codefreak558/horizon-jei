#!/usr/bin/env bash
# Format, lint and test the mod against the pinned SDK, then (when the launcher is installed) build it as the
# launcher does. HORIZON_INSTANCE picks the instance that then plays the build (default: the selected one).
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

"$root/tools/fetch-sdk.sh"

echo "== fmt"
cargo fmt --check
echo "== clippy"
cargo clippy --all-targets -- -D warnings
echo "== test"
cargo test

if command -v horizon-launcher > /dev/null; then
  echo "== native-build"
  horizon-launcher native-build "$root" ${HORIZON_INSTANCE:+--instance "$HORIZON_INSTANCE"} --trust-native-code
else
  echo "(horizon-launcher not on PATH: skipping native-build)"
fi
