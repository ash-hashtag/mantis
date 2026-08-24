# Helix

Mantis ships a Tree-sitter grammar (`tree-sitter-mantis/`) with a one-command
build & install pipeline for Helix:

```sh
./scripts/install-helix-mantis.sh
```

This script:

1. Generates parser sources from `tree-sitter-mantis/grammar.js` (pnpm +
   tree-sitter-cli).
2. Validates every query in `runtime/queries/mantis/*.scm` against real code.
3. Compiles `mantis.so` with the same `cc` invocation Helix uses and installs
   it into both runtimes:
   - `<repo>/runtime/grammars/mantis.so` (project-local)
   - `~/.config/helix/runtime/grammars/mantis.so` (global)
4. Copies the queries to `~/.config/helix/runtime/queries/mantis/`.
5. Rewrites the absolute grammar source path in `.helix/languages.toml`
   (Helix resolves local grammar sources against the compiler's cwd, so the
   path must be absolute).

`~/.config/helix/runtime` is merged additively over the system runtime, so
all built-in languages keep working — no `HELIX_RUNTIME` variable needed.

## Configuration

Both configs register `mantis` (scope `source.mantis`, file-type `ms`) with
the LSP pointing at `/home/ash/.cargo/target/release/mantis_lsp`:

- **Global** (`~/.config/helix/languages.toml`): works everywhere.
- **Project** (`.helix/languages.toml`): same setup, kept in sync by the
  install script.

Do not use `cargo run` as a language-server command — it recompiles the
workspace on every file open (~15 s), floods Helix's log with compiler
warnings on stderr, blocks on cargo's build-directory lock, and kills the
server whenever a build fails mid-development.

## Usage

```sh
hx libc.ms        # from anywhere: global config, installed grammar + LSP
```

Re-run `./scripts/install-helix-mantis.sh` whenever you change
`grammar.js`, the queries, or want to refresh the LSP binary
(`cargo build --release -p mantis_lsp`).

Verify with `helix --health mantis`: parser, highlight, indent and
textobject queries should all show ✓.
