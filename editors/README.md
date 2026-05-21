# Editor support

> **Navigation**: [Repository Root](../README.md)

Editor and IDE integration for Keleusma. The contents of this directory are not part of the Keleusma runtime, not part of the published crates.io tarball, and not required by downstream embedders. They exist to help authors of Keleusma scripts edit them comfortably.

## Available integrations

| Editor | Path | Status |
|--------|------|--------|
| Vim / Neovim | [`vim/`](./vim/) | Syntax highlighting and file-type detection. |

## Planned

Future contributions adding syntax support for additional editors are welcome. Conventional sub-directory names are `vscode/` for the Visual Studio Code marketplace package, `sublime/` for the Sublime Text package, `emacs/` for an Emacs major-mode, `helix/` for Helix's tree-sitter grammar wrapper, and `zed/` for the Zed extension. Each new editor should land its own sub-directory with a self-contained README documenting installation.

## Why not upstream?

Editor support is intentionally bundled in the project repository so that the syntax definitions track the language. The grammar evolves between minor releases (V0.1.x retired closures and f-strings, V0.2.0 added newtypes with refinement predicates, the `signed` modifier, and information-flow labels), and the syntax files travel with the source that introduces those changes. When an integration matures and stabilises, the maintainer of that specific integration may upstream it to the corresponding editor's package registry; until then, an in-repo copy is the canonical location.
