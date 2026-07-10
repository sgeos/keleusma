# keleusma-lsp

A Language Server Protocol server for the Keleusma language. It reuses the core
compiler front end to give editors live feedback.

This is a host-side developer tool. It is a **detached-workspace crate** (like
`examples/rtos/`): it depends on the core `keleusma` crate by path but ships
`std`, `tokio`, and `tower-lsp`, so it is kept out of the core `no_std`
workspace's dependency graph and out of the published crate tarball. Build it
from its own directory.

## Status

- **M1 + M2 (done).**
  - **Live diagnostics over the full pipeline.** On open and on every edit the
    server runs `tokenize` → `parse` → `compile` → `verify`, so lex, parse,
    type, monomorphization, codegen, and — the signature feature — the
    worst-case-execution-time and worst-case-memory-usage *verifier rejections*
    surface as you type, positioned from the `Span` each error carries (verifier
    faults, which carry a chunk name rather than a span, anchor at the document
    start). The pipeline is fail-fast, so at most one diagnostic appears per pass.
  - **Document symbols.** Functions (with their `fn`/`yield`/`loop` category),
    structs, enums, newtypes, and traits.
  - **Completion.** The keyword and primitive-type vocabulary.
  - **Panic-safe.** A deep-compiler panic on half-written code degrades to "no
    diagnostics this pass", never crashes the server.

## Roadmap

- **M3 — VS Code client.** Wire the existing extension in `editors/vscode/` to
  this server with `vscode-languageclient`.
- **Later.** Hover (type at cursor), go-to-definition, in-scope identifier
  completion, and multi-error recovery beyond the fail-fast first error.

## Build and run

```sh
cd keleusma-lsp
cargo build --release        # or: cargo +1.92 build --release
```

The binary speaks LSP over stdio, which every major editor understands. Point
your editor's LSP client at the built `keleusma-lsp` binary for files with the
`.kel` extension. A minimal Neovim example:

```lua
vim.lsp.start({
  name = "keleusma-lsp",
  cmd = { "/path/to/keleusma/keleusma-lsp/target/release/keleusma-lsp" },
  root_dir = vim.fs.dirname(vim.fs.find({ ".git" }, { upward = true })[1]),
})
```

VS Code wiring lands in M3; until then, the `editors/vscode/` extension provides
syntax highlighting without live diagnostics.

## Design notes

- **Transport is stdio.** No sockets, no configuration; the universal default.
- **Positions are UTF-16.** `offset_to_position` converts the compiler's byte
  offsets to the protocol's default UTF-16 character units.
- **Full-text sync.** The server keeps a full copy of each open document and
  re-analyses on change. Keleusma programs are small, so incremental sync is not
  yet worth its complexity.
