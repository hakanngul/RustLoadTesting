#!/usr/bin/env bash
set -euo pipefail
if [[ "$(uname -s)" != Darwin ]]; then
  echo 'Run this packaging script on macOS.' >&2
  exit 1
fi
if [[ $# -gt 1 ]]; then
  echo 'Usage: bash scripts/package-macos.sh [aarch64-apple-darwin|x86_64-apple-darwin]' >&2
  exit 1
fi
project_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$project_root"
target="${1:-}"
if [[ -n "$target" ]]; then
  case "$target" in
    aarch64-apple-darwin|x86_64-apple-darwin) ;;
    *) echo "Unsupported macOS target: $target" >&2; exit 1 ;;
  esac
  cargo build --release --locked --target "$target"
  binary="target/$target/release/rustload"
else
  cargo build --release --locked
  binary=target/release/rustload
fi
version=$(cargo metadata --locked --no-deps --format-version 1 | python3 -c \
  'import json, sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "rustload"))')
bundle="$project_root/dist/RustLoad.app"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$binary" "$bundle/Contents/MacOS/RustLoad"
chmod 755 "$bundle/Contents/MacOS/RustLoad"
python3 - "$bundle/Contents/Info.plist" "$version" "${MACOSX_DEPLOYMENT_TARGET:-12.0}" <<'PY'
import plistlib
import sys

path, version, minimum = sys.argv[1:]
numeric_version = version.split('-')[0].split('+')[0]
with open(path, 'wb') as output:
    plistlib.dump({
        'CFBundleExecutable': 'RustLoad',
        'CFBundleIdentifier': 'dev.rustload.desktop',
        'CFBundleName': 'RustLoad',
        'CFBundlePackageType': 'APPL',
        'CFBundleShortVersionString': numeric_version,
        'CFBundleVersion': numeric_version,
        'CFBundleGetInfoString': f'RustLoad {version}',
        'LSMinimumSystemVersion': minimum,
        'NSHighResolutionCapable': True,
    }, output)
PY
plutil -lint "$bundle/Contents/Info.plist"
codesign --force --sign - "$bundle"
codesign --verify --strict --verbose=2 "$bundle"
printf 'Ad-hoc signed app bundle (not notarized): %s\n' "$bundle"
