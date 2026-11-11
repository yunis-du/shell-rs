#!/usr/bin/env bash
# Build a local macOS DMG: release binary, app bundle, ad-hoc signature and
# an Applications shortcut. Run from any directory:
#
#   scripts/build-dmg.sh
#
# SHELLRS_BUILD_NUMBER sets the bundle's build number (default: 1).
# These local builds leave automatic updates disabled. Distribution signing
# and notarization remain the release workflow's responsibility.
set -euo pipefail

if [ "$(uname -s)" != Darwin ]; then
    echo "error: building a DMG requires macOS" >&2
    exit 1
fi

root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"

build_target=$(rustc -vV | sed -n 's/^host: //p')
case "$build_target" in
    aarch64-apple-darwin) arch=aarch64 ;;
    x86_64-apple-darwin) arch=x86_64 ;;
    *) echo "error: unsupported Rust host: $build_target" >&2; exit 1 ;;
esac

app_version=$(awk -F '"' '/^version = / { print $2; exit }' Cargo.toml)
build_number=${SHELLRS_BUILD_NUMBER:-1}
build_dir=${CARGO_TARGET_DIR:-target}
out="$root/target/dmg"

# Explicitly select the host, even if CARGO_BUILD_TARGET names another target.
env -u SHELLRS_UPDATE_CHANNEL -u SHELLRS_APPLE_TEAM_ID \
    cargo build --release --locked --bin shellrs \
        --target "$build_target" --target-dir "$build_dir"

stage_dir=$(mktemp -d)
trap 'rm -rf "$stage_dir"' EXIT

bash packaging/macos/bundle.sh \
    "$build_dir/$build_target/release/shellrs" "$app_version" "$build_number" "$stage_dir"
codesign --force --sign - "$stage_dir/ShellRS.app"
codesign --verify --deep --strict "$stage_dir/ShellRS.app"
ln -s /Applications "$stage_dir/Applications"

mkdir -p "$out"
# Leave room beyond hdiutil's estimate for the bundle and filesystem metadata.
image_size_mb=$(( $(du -sm "$stage_dir" | cut -f1) * 2 + 20 ))
dmg="$out/ShellRS-${app_version}-macos-${arch}.dmg"
hdiutil create -volname ShellRS -srcfolder "$stage_dir" \
    -fs HFS+ -size "${image_size_mb}m" -format UDZO -ov "$dmg"
hdiutil verify "$dmg"

echo "DMG created: $dmg"
