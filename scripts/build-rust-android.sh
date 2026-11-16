#!/usr/bin/env bash
# Build crates/linkjobs-jni for Android into android/app/src/main/jniLibs/<abi>/.
#
# Gradle runs this before every build (see android/app/build.gradle.kts) and passes
# ANDROID_NDK_HOME. From a terminal:
#
#   ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/27.2.12479018 scripts/build-rust-android.sh
#
# Env: LINKJOBS_ABIS (default "arm64-v8a x86_64"), LINKJOBS_PROFILE (release|debug, default release)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

if [ -z "${ANDROID_NDK_HOME:-}" ] || [ ! -d "$ANDROID_NDK_HOME" ]; then
  echo "error: ANDROID_NDK_HOME is not set to an installed NDK." >&2
  echo "       Install it in Android Studio: Settings > SDK Manager > SDK Tools > NDK (Side by side)." >&2
  exit 1
fi

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "note: installing cargo-ndk"
  cargo install cargo-ndk --locked
fi

ABIS="${LINKJOBS_ABIS:-arm64-v8a x86_64}"
PROFILE="${LINKJOBS_PROFILE:-release}"

targets=()
ndk_args=()
for abi in $ABIS; do
  case "$abi" in
    arm64-v8a) targets+=(aarch64-linux-android) ;;
    armeabi-v7a) targets+=(armv7-linux-androideabi) ;;
    x86_64) targets+=(x86_64-linux-android) ;;
    x86) targets+=(i686-linux-android) ;;
    *) echo "error: unknown ABI $abi" >&2; exit 1 ;;
  esac
  ndk_args+=(-t "$abi")
done
rustup target add "${targets[@]}" >/dev/null

profile_flag=()
[ "$PROFILE" = "release" ] && profile_flag=(--release)

cd "$ROOT"
cargo ndk "${ndk_args[@]}" --platform 26 -o android/app/src/main/jniLibs \
  build -p linkjobs-jni ${profile_flag[@]+"${profile_flag[@]}"}
echo "note: wrote $(find android/app/src/main/jniLibs -name '*.so' | tr '\n' ' ')"
