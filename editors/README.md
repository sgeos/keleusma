# Editor support

> **Navigation**: [Repository Root](../README.md)

Editor and IDE integration for Keleusma. The contents of this directory are not part of the Keleusma runtime, not part of the published crates.io tarball, and not required by downstream embedders. They exist to help authors of Keleusma scripts edit them comfortably.

## Available integrations

| Editor | Path | Highlighting | File-type | Indent |
|--------|------|--------------|-----------|--------|
| Vim / Neovim | [`vim/`](./vim/) | ✓ | ✓ | basic |
| Visual Studio Code | [`vscode/`](./vscode/) | ✓ | ✓ | basic |
| Sublime Text | [`vscode/`](./vscode/) (reuses the TextMate grammar) | ✓ | ✓ | basic |
| Emacs | [`emacs/`](./emacs/) | ✓ | ✓ | none |
| Rouge (Jekyll, web) | [`rouge/`](./rouge/) | ✓ | n/a | n/a |
| Helix | [`helix/`](./helix/) | pending (tree-sitter) | ✓ | basic |
| Zed | pending | pending (tree-sitter) | n/a | n/a |
| JetBrains IDEs | not provided | n/a | n/a | n/a |

## Categorisation convention shared across integrations

The opinionated bits of the Keleusma surface get their own highlight categories so a reader new to the language can build a mental model of the program model from the highlighting alone:

- **Function-category keywords** (`fn`, `yield`, `loop`) are highlighted distinctly from generic keywords. The category distinction drives every verifier rule.
- **Storage-discipline modifiers** (`signed`, `ephemeral`, `shared`, `private`, `const`) are highlighted as a storage class. Readers from C, Rust, or Java recognise the colour pattern.
- **Checked-arithmetic arm keywords** (`ok`, `overflow`, `underflow`, `saturate_max`, `saturate_min`) are highlighted as a special construct so the checked-arithmetic block is visually distinct.
- **Information-flow operators** (`classify`, `declassify`) and **label annotations** (`@Label`, `@!Negative`, `@{A, B}`, `@{!A, !B}`) share a highlight category. Most programmers have not used information-flow control, so highlighting the IFC surface as a class signals "this is the language's IFC machinery, look it up if you have not seen it before".
- **Pipeline operator** (`\|>`) is promoted out of generic operators because stream-style left-to-right composition is a load-bearing idiom in Keleusma.

The vim file at [`vim/syntax/keleusma.vim`](./vim/syntax/keleusma.vim) carries authoritative commentary on the categorisation decisions; the other integrations track the same scheme.

## Planned

Future contributions adding syntax support for additional editors are welcome. Conventional sub-directory names are:

| Sub-directory | Editor |
|---------------|--------|
| `zed/` | Zed extension manifest |
| `jetbrains/` | IntelliJ Platform plugin |
| `kakoune/` | Kakoune syntax script |
| `nano/` | nanorc syntax file |

The tree-sitter grammar tracked at the planned `tree-sitter-keleusma` repository would unlock Helix, Zed, and Neovim's `nvim-treesitter` users simultaneously. See [`helix/README.md`](./helix/README.md) for the status.

## Why bundled in the project repository

Editor support is intentionally bundled here so that the syntax definitions track the language. The grammar evolves between minor releases (V0.1.x retired closures and f-strings, V0.2.0 added newtypes with refinement predicates, the `signed` modifier, and information-flow labels), and the syntax files travel with the source that introduces those changes. When an integration matures and stabilises, its maintainer may upstream it to the corresponding editor's package registry; until then, an in-repo copy is the canonical location.

The `editors/` directory is excluded from the crates.io tarball via the `exclude` field in [`Cargo.toml`](../Cargo.toml) so downstream library consumers do not pay the size for editor metadata they will not use.
