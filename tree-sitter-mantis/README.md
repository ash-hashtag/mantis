# tree-sitter-mantis

This is the local Tree-sitter grammar used by the Helix integration.

```sh
pnpm install
pnpm exec tree-sitter generate
```

The generated `src/parser.c` is committed so Helix can build the grammar
without requiring Node at runtime.
