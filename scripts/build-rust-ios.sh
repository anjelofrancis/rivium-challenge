#!/usr/bin/env bash
# Build the LinkJobs Rust core as a static library for iOS.
#
# Xcode runs this from the "Build Rust core" phase of ios/LinkJobs.xcodeproj, using its
# PLATFORM_NAME / ARCHS / CONFIGURATION. It also works from a terminal:
#
#   scripts/build-rust-ios.sh                 # simulator, debug
#   PLATFORM_NAME=iphoneos CONFIGURATION=Release scripts/build-rust-ios.sh
#
# Output: $RUST_LIB_DIR/liblinkjobs.a (default: target/ios/<platform>/)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Xcode's PATH does not include rustup's shims.
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "error: cargo not found. Install Rust from https://rustup.rs" >&2
  exit 1
fi

PLATFORM_NAME="${PLATFORM_NAME:-iphonesimulator}"
CONFIGURATION="${CONFIGURATION:-Debug}"
ARCHS="${ARCHS:-$(uname -m)}"

targets=()
for arch in $ARCHS; do
  case "$PLATFORM_NAME/$arch" in
    iphoneos/arm64) targets+=(aarch64-apple-ios) ;;
    iphonesimulator/arm64) targets+=(aarch64-apple-ios-sim) ;;
    iphonesimulator/x86_64) targets+=(x86_64-apple-ios) ;;
    *) echo "error: unsupported platform/arch $PLATFORM_NAME/$arch" >&2; exit 1 ;;
  esac
done

profile_flag=()
profile_dir=debug
if [ "$CONFIGURATION" = "Release" ]; then
  profile_flag=(--release)
  profile_dir=release
fi

OUT="${RUST_LIB_DIR:-$ROOT/target/ios/$PLATFORM_NAME}"
mkdir -p "$OUT"

# Xcode exports hundreds of variables (SDKROOT, CC flags, deployment targets...) for the
# *iOS* SDK. They break C builds inside cargo (sqlite, ring) and the host build scripts, so
# cargo runs with a clean environment and lets cc-rs ask xcrun for the right SDK.
DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-17.0}"
clean_cargo() {
  env -i \
    HOME="$HOME" USER="${USER:-}" TERM="${TERM:-dumb}" \
    PATH="$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin:/usr/local/bin" \
    ${CARGO_HOME:+CARGO_HOME="$CARGO_HOME"} ${RUSTUP_HOME:+RUSTUP_HOME="$RUSTUP_HOME"} \
    DEVELOPER_DIR="${DEVELOPER_DIR:-$(xcode-select -p)}" \
    IPHONEOS_DEPLOYMENT_TARGET="$DEPLOYMENT_TARGET" \
    cargo "$@"
}

libs=()
for target in "${targets[@]}"; do
  if ! rustup target list --installed | grep -qx "$target"; then
    echo "note: installing Rust target $target"
    rustup target add "$target"
  fi
  echo "note: cargo build -p linkjobs-ios --target $target ${profile_flag[*]-}"
  (cd "$ROOT" && clean_cargo build -p linkjobs-ios --target "$target" ${profile_flag[@]+"${profile_flag[@]}"})
  libs+=("$ROOT/target/$target/$profile_dir/liblinkjobs.a")
done

if [ "${#libs[@]}" -eq 1 ]; then
  cp -f "${libs[0]}" "$OUT/liblinkjobs.a"
else
  lipo -create "${libs[@]}" -output "$OUT/liblinkjobs.a"
fi
echo "note: wrote $OUT/liblinkjobs.a"
