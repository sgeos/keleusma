# Vim / Neovim syntax for Keleusma

> **Navigation**: [Editors](../README.md) | [Repository Root](../../README.md)

Syntax highlighting and file-type detection for Keleusma source files (`.kel`).

## Files

| File | Purpose |
|------|---------|
| [`syntax/keleusma.vim`](./syntax/keleusma.vim) | Syntax highlighting rules. Distinguishes keywords, control-flow constructs, primitive types, type-style identifiers, function calls, comments (line and block), and the V0.2.0 operators including the pipeline `|>`, the match arm `=>`, the path separator `::`, and the information-flow label separator `@`. |
| [`ftdetect/keleusma.vim`](./ftdetect/keleusma.vim) | File-type autocommand. Associates the `.kel` extension with `filetype=keleusma`. |

## Installation

### Vim 8 native packages

```sh
mkdir -p ~/.vim/pack/keleusma/start
cp -R editors/vim ~/.vim/pack/keleusma/start/keleusma
```

### Neovim

```sh
mkdir -p ~/.config/nvim/pack/keleusma/start
cp -R editors/vim ~/.config/nvim/pack/keleusma/start/keleusma
```

### Pathogen

```sh
cp -R editors/vim ~/.vim/bundle/keleusma
```

### Plug, packer.nvim, lazy.nvim

Point your plugin manager at the `editors/vim/` subdirectory of a clone of this repository, or extract the two files into your runtime path manually.

## Verifying installation

Open a `.kel` file. Vim should set the filetype automatically. To confirm:

```vim
:set filetype?
```

The expected output is `filetype=keleusma`.

If syntax highlighting is not active even with the correct filetype, ensure that `syntax on` is set in your `vimrc` or `init.vim`.

## Limitations

This is a minimal syntax file. It does not provide:

- Smart indentation. Use Vim's default `cindent` or roll your own indent file.
- Folding rules. Use Vim's `manual` or `marker` foldmethod.
- A language server. Keleusma does not currently ship an LSP implementation. Diagnostics surface through `keleusma run` or through the Rust embedder's compile pipeline.

Patches that extend coverage (a `keleusma-indent.vim`, fold rules, or LSP integration) are welcome.
