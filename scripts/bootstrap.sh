#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_ROOT/target}"
mkdir -p "$TARGET_DIR"

echo "=== 1. Building mantisc (Rust bootstrap compiler) ==="
cargo build --bin mantisc

MANTISC="$TARGET_DIR/debug/mantisc"
if [ ! -f "$MANTISC" ]; then
    MANTISC="$REPO_ROOT/target/debug/mantisc"
fi

echo "=== 2. Compiling self-hosted packages ==="
# packages/libc -> target/libc.o
echo "Compiling libc..."
"$MANTISC" -c "$REPO_ROOT/packages/libc/src/lib.ms" \
    -I "$REPO_ROOT/packages" \
    -o "$TARGET_DIR/libc.o"

# packages/std -> target/std.o
echo "Compiling std..."
"$MANTISC" -c "$REPO_ROOT/packages/std/src/lib.ms" \
    -I "$REPO_ROOT/packages" \
    -o "$TARGET_DIR/std.o"

# packages/toml -> target/toml.o
echo "Compiling toml..."
"$MANTISC" -c "$REPO_ROOT/packages/toml/src/lib.ms" \
    -I "$REPO_ROOT/packages" \
    -o "$TARGET_DIR/toml.o"

# packages/cli -> target/cli.o
echo "Compiling cli..."
"$MANTISC" -c "$REPO_ROOT/packages/cli/src/main.ms" \
    -I "$REPO_ROOT/packages" \
    -o "$TARGET_DIR/cli.o"

echo "=== 3. Linking self-hosted mantis binary ==="
cc -no-pie \
    "$TARGET_DIR/cli.o" \
    "$TARGET_DIR/libc.o" \
    "$REPO_ROOT/packages/cli/runtime.c" \
    -lc -lpthread -lm \
    -o "$TARGET_DIR/mantis"

echo "=== 4. Installing to ~/.mantis/ ==="
mkdir -p "$HOME/.mantis/bin" "$HOME/.mantis/packages"
cp -f "$MANTISC" "$HOME/.mantis/bin/mantisc"
cp -f "$TARGET_DIR/mantis" "$HOME/.mantis/bin/mantis"
cp -rf "$REPO_ROOT/packages/"* "$HOME/.mantis/packages/"

echo "=== 5. Verifying self-hosted mantis CLI ==="
"$TARGET_DIR/mantis" version
"$TARGET_DIR/mantis" help

echo "=== Bootstrapping complete! mantis binary is at $TARGET_DIR/mantis ==="
