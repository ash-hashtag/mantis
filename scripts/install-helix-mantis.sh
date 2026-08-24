#!/usr/bin/env bash
# Build the Mantis Tree-sitter grammar and install it for Helix.
#
# Produces:
#   <repo>/runtime/grammars/mantis.so            (project-local runtime)
#   ~/.config/helix/runtime/grammars/mantis.so   (global, merged with system runtime)
#   ~/.config/helix/runtime/queries/mantis/*.scm (global highlight queries)
#
# Also keeps the absolute grammar source path in .helix/languages.toml in sync
# (Helix resolves local grammar sources against the compiler cwd, so the path
# must be absolute).
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
grammar_dir="$repo_root/tree-sitter-mantis"
helix_runtime="${HOME}/.config/helix/runtime"

command -v pnpm >/dev/null || { echo "error: pnpm is required" >&2; exit 1; }
command -v cc >/dev/null || { echo "error: cc is required" >&2; exit 1; }

echo "==> Generating parser sources"
cd "$grammar_dir"
pnpm install --silent
./node_modules/.bin/tree-sitter generate

echo "==> Validating queries"
if compgen -G "$repo_root"/*.ms >/dev/null; then
    sample=$(ls "$repo_root"/*.ms | head -1)
    for q in highlights indents textobjects; do
        if [ -f "$repo_root/runtime/queries/mantis/$q.scm" ]; then
            ./node_modules/.bin/tree-sitter query \
                "../runtime/queries/mantis/$q.scm" "$sample" >/dev/null 2>&1 ||
                { echo "error: $q.scm failed to compile or match" >&2; exit 1; }
            echo "    ok: $q.scm"
        fi
    done
else
    echo "    skipped: no sample .ms file at repo root"
fi

echo "==> Compiling mantis.so"
install_to() {
    local out_dir="$1/grammars"
    mkdir -p "$out_dir"
    # Mirrors helix-loader's build_tree_sitter_library() invocation exactly.
    (
        cd "$grammar_dir/src"
        cc -fPIC -shared -fno-exceptions \
            -I . \
            -o "$out_dir/mantis.so" \
            -xc -std=c11 parser.c \
            -Wl,-z,relro,-z,now
    )
    echo "    installed: $out_dir/mantis.so"
}
install_to "$repo_root/runtime"
if [ ! -w "$(dirname "$helix_runtime")" ]; then
    echo "warning: cannot write to $(dirname "$helix_runtime"), skipping global install" >&2
else
    install_to "$helix_runtime"
fi

echo "==> Installing highlight queries"
mkdir -p "$helix_runtime/queries/mantis"
cp "$repo_root"/runtime/queries/mantis/*.scm "$helix_runtime/queries/mantis/"

echo "==> Building mantis_lsp (release)"
(
    cd "$repo_root"
    cargo build --release -p mantis_lsp 2>&1 | grep -E "^(error|warning: unused)" || true
)
lsp_bin="$(cargo metadata --no-deps --format-version 1 2>/dev/null \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/release/mantis_lsp"
[ -x "$lsp_bin" ] || { echo "error: LSP binary not found at $lsp_bin" >&2; exit 1; }
echo "    built: $lsp_bin"

echo "==> Syncing .helix/languages.toml grammar path"
sed -i \
    "s|source = { path = .* }|source = { path = \"$grammar_dir\" }|" \
    "$repo_root/.helix/languages.toml"

echo
echo "Done. Open any .ms file with Helix; highlighting + LSP work globally."
echo "Re-run this script whenever grammar.js, queries, or LSP sources change."
