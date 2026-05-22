# Emacs major-mode for Keleusma

> **Navigation**: [Editors](../README.md) | [Repository Root](../../README.md)

A minimal major-mode providing syntax highlighting, comment toggling, and file-type recognition for `.kel` files.

## Files

| File | Purpose |
|------|---------|
| [`keleusma-mode.el`](./keleusma-mode.el) | The major-mode. Derives from `prog-mode`, registers the `.kel` extension via `auto-mode-alist`, and supplies a font-lock specification whose categorisation parallels the vim and VSCode integrations. |

## Installation

### Manual

```sh
cp editors/emacs/keleusma-mode.el ~/.emacs.d/lisp/
```

Then in your `init.el`:

```elisp
(add-to-list 'load-path "~/.emacs.d/lisp/")
(require 'keleusma-mode)
```

### use-package (with straight.el)

```elisp
(use-package keleusma-mode
  :straight (:host github :repo "sgeos/keleusma" :files ("editors/emacs/keleusma-mode.el"))
  :mode "\\.kel\\'")
```

### Doom Emacs

In `packages.el`:

```elisp
(package! keleusma-mode
  :recipe (:host github :repo "sgeos/keleusma"
           :files ("editors/emacs/keleusma-mode.el")))
```

In `config.el`:

```elisp
(use-package! keleusma-mode
  :mode "\\.kel\\'")
```

## Highlighting categories

The font-lock specification distinguishes:

| Category | Font-lock face | Examples |
|---|---|---|
| Function categories | `font-lock-keyword-face` | `fn`, `yield`, `loop` |
| Storage modifiers | `font-lock-type-face` | `signed`, `ephemeral`, `shared`, `private`, `const` |
| Checked-arithmetic arms | `font-lock-builtin-face` | `ok`, `overflow`, `underflow`, `saturate_max`, `saturate_min` |
| IFC labels and operators | `font-lock-builtin-face` | `@Label`, `@!Secret`, `classify`, `declassify` |
| Pipeline operator | `font-lock-builtin-face` | `\|>` |
| Other keywords | `font-lock-keyword-face` | `let`, `where`, `as`, `not`, `and`, `or`, `struct`, `enum`, `newtype`, `trait`, `impl`, ... |
| Primitive types | `font-lock-type-face` | `Word`, `Byte`, `Float`, `Fixed`, `bool`, `Text`, `Option` |
| User-defined types | `font-lock-type-face` | Uppercase-initial identifiers |
| Function names | `font-lock-function-name-face` | Lowercase identifier followed by `(` |
| Booleans | `font-lock-constant-face` | `true`, `false` |
| Numeric literals | `font-lock-constant-face` | `42`, `0xff`, `0b1010`, `3.14`, `42i64`, `3.14f64` |
| Comments | `font-lock-comment-face` | `// line`, `/* block */`, `#!shebang` |

Emacs has fewer distinct font-lock faces than Vim's standard highlight categories. The IFC machinery, pipeline operator, and checked-arithmetic arms cluster on `font-lock-builtin-face` together rather than splitting into separate scopes; this keeps the V0.2 surface visually distinguishable while staying within the standard face vocabulary that color themes ship.

## Limitations

- No indentation engine. The mode sets `tab-width 4` and `indent-tabs-mode nil` but does not perform smart indentation. Use `M-x indent-region` to align brace pairs manually, or contribute a `keleusma-indent-line` function.
- No LSP integration. Keleusma does not currently ship a language server.
- No flymake/flycheck integration. Diagnostics surface through `keleusma run` invocations rather than through editor-side checking.

Patches that extend coverage (indent engine, LSP client, flycheck checker) are welcome.
