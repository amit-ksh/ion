#!/usr/bin/env bash

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

target_dir="${CARGO_TARGET_DIR:-target}"
version="$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"name":"waku","version":"\([^"]*\)".*/\1/p')"
target_triple="$(rustc -vV | sed -n 's/^host: //p')"
package="waku-${version}-${target_triple}"
archive="$target_dir/release/$package.tar.gz"
staging="$(mktemp -d)"
trap 'rm -rf -- "$staging"' EXIT

cargo build --locked --release \
  --package waku --bin waku --bin waku-updater --bin waku_js_repl \
  --package waku-daemon --bin waku-daemon \
  --package waku-computer-use --bin waku_computer_use

package_dir="$staging/$package"
bun scripts/cua-driver.ts bundle "$package_dir/bin" "$package_dir/share/waku" release
install -Dm755 "$target_dir/release/waku" "$package_dir/bin/waku"
install -Dm755 "$target_dir/release/waku-updater" "$package_dir/bin/waku-updater"
install -Dm755 "$target_dir/release/waku-daemon" "$package_dir/bin/waku-daemon"
install -Dm644 resources/linux/sh.waku.desktop \
  "$package_dir/share/applications/sh.waku.desktop"
install -Dm644 resources/linux/self-update-v1 \
  "$package_dir/share/waku/self-update-v1"
install -Dm644 website/public/app-icon.png \
  "$package_dir/share/icons/hicolor/256x256/apps/sh.waku.png"
install -Dm644 LICENSE "$package_dir/share/licenses/waku/LICENSE"

nemo_prefix="${NEMO_SPEECH_PREFIX:-}"
if [ -z "$nemo_prefix" ] || [ ! -x "$nemo_prefix/bin/nemo-speech" ] || \
   [ ! -f "$nemo_prefix/.nemo-speech-install" ] || \
   [ ! -d "$nemo_prefix/lib" ] || \
   [ ! -f "$nemo_prefix/share/licenses/nemo-speech/LICENSE" ]; then
  echo "NEMO_SPEECH_PREFIX must contain a verified NeMo-Speech.cpp 0.1.0 CPU install" >&2
  exit 1
fi
case "$(head -n 1 "$nemo_prefix/.nemo-speech-install")" in
  "0.1.0 linux "*" cpu") ;;
  *) echo "Unexpected NeMo-Speech.cpp release identity" >&2; exit 1 ;;
esac
mkdir -p "$package_dir/bin/nemo-speech"
cp -a "$nemo_prefix"/. "$package_dir/bin/nemo-speech/"
"$package_dir/bin/nemo-speech/bin/nemo-speech" --version | grep -F '0.1.0' >/dev/null

mkdir -p "$(dirname "$archive")"
tar -C "$staging" -czf "$archive" "$package"
printf 'Created %s\n' "$archive"
