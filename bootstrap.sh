#!/usr/bin/env bash
set -euo pipefail

DEBUG=0
for arg in "$@"; do
    case "$arg" in
        --debug) DEBUG=1 ;;
        *) echo "usage: $0 [--debug]" >&2; exit 2 ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
MANTIS_TARGET_DIR="${MANTIS_TARGET_DIR:-$REPO_ROOT/build}"
PROFILE=release
PROFILE_FLAGS=(--release)
if [ "$DEBUG" -eq 1 ]; then
    PROFILE=debug
    PROFILE_FLAGS=()
fi
BUILD_DIR="$MANTIS_TARGET_DIR/$PROFILE"
PKGCACHE="${HOME}/.mantis/pkgcache"
mkdir -p "$TARGET_DIR" "$BUILD_DIR" "$PKGCACHE"

echo "=== 1. Building mantisc ($PROFILE) ==="
cargo build "${PROFILE_FLAGS[@]}" --bin mantisc
MANTISC="$TARGET_DIR/$PROFILE/mantisc"
if [ ! -x "$MANTISC" ]; then
    MANTISC="$REPO_ROOT/target/$PROFILE/mantisc"
fi

echo "=== 2. Installing built-in packages ==="
rm -rf "$PKGCACHE/libc-0.1.0" "$PKGCACHE/std-0.1.0" "$PKGCACHE/toml-0.1.0"
cp -r "$REPO_ROOT/packages/libc" "$PKGCACHE/libc-0.1.0"
cp -r "$REPO_ROOT/packages/std" "$PKGCACHE/std-0.1.0"
cp -r "$REPO_ROOT/packages/toml" "$PKGCACHE/toml-0.1.0"

echo "=== 3. Compiling built-in packages ($PROFILE) ==="
"$MANTISC" -c "$REPO_ROOT/packages/libc/src/lib.ms" -I "$REPO_ROOT/packages" -o "$BUILD_DIR/libc.o"
"$MANTISC" -c "$REPO_ROOT/packages/std/src/lib.ms" -I "$REPO_ROOT/packages" -o "$BUILD_DIR/std.o"
"$MANTISC" -c "$REPO_ROOT/packages/toml/src/lib.ms" -I "$REPO_ROOT/packages" -o "$BUILD_DIR/toml.o"
cp -f "$BUILD_DIR/libc.o" "$PKGCACHE/libc-0.1.0/libc.o"
cp -f "$BUILD_DIR/std.o" "$PKGCACHE/std-0.1.0/std.o"
cp -f "$BUILD_DIR/toml.o" "$PKGCACHE/toml-0.1.0/toml.o"

echo "=== 4. Compiling self-hosted CLI ($PROFILE) ==="
"$MANTISC" -c "$REPO_ROOT/packages/cli/src/main.ms" -I "$REPO_ROOT/packages" -o "$BUILD_DIR/cli.o"
CC_FLAGS=()
[ "$PROFILE" = release ] && CC_FLAGS=(-O2)
cc "${CC_FLAGS[@]}" -no-pie "$BUILD_DIR/cli.o" "$BUILD_DIR/libc.o" -lc -lpthread -lm -o "$TARGET_DIR/mantis"

echo "=== 5. Installing mantis ($PROFILE) ==="
mkdir -p "$HOME/.mantis/bin" "$HOME/.mantis/packages"
cp -f "$MANTISC" "$HOME/.mantis/bin/mantisc"
cp -f "$TARGET_DIR/mantis" "$HOME/.mantis/bin/mantis"
cp -rf "$REPO_ROOT/packages/"* "$HOME/.mantis/packages/"
"$TARGET_DIR/mantis" version
echo "=== Bootstrap complete: $TARGET_DIR/mantis ($PROFILE) ==="
