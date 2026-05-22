# VSCode and Sublime Text syntax for Keleusma

> **Navigation**: [Editors](../README.md) | [Repository Root](../../README.md)

A TextMate grammar plus a language-configuration file. The TextMate format is consumed by Visual Studio Code, Sublime Text 3+, and historically Atom; one grammar covers all three editors.

## Files

| File | Purpose |
|------|---------|
| [`package.json`](./package.json) | VSCode extension manifest. Declares the `keleusma` language id, the `.kel` extension, the grammar, and the language-configuration file. |
| [`language-configuration.json`](./language-configuration.json) | Bracket matching, auto-closing pairs, comment toggle (`//` and `/* */`), surrounding pairs, indentation rules. |
| [`syntaxes/keleusma.tmLanguage.json`](./syntaxes/keleusma.tmLanguage.json) | TextMate grammar covering the V0.2 surface. Categorisation parallels the vim syntax file: function-category keywords, storage-discipline modifiers, checked-arithmetic arm keywords, IFC labels and operators, the pipeline operator, and the rest of the V0.2 grammar. |

## Installing in VSCode

### Development install (symlink)

```sh
ln -s "$(pwd)/editors/vscode" ~/.vscode/extensions/keleusma-0.2.0
```

Restart VSCode. Open any `.kel` file; it should show "Keleusma" in the lower-right language indicator. The editor will pick up syntax highlighting, comment toggling, and bracket matching automatically.

### Packaged install (vsix)

If you have `vsce` installed:

```sh
cd editors/vscode
vsce package
code --install-extension keleusma-0.2.0.vsix
```

This builds a `.vsix` archive and installs it as a regular extension. Use this path if you want to share the extension with others without the source checkout.

## Installing in Sublime Text

Sublime Text 3 and later read TextMate grammars natively. Copy the grammar file into Sublime's user package directory:

```sh
# macOS
cp editors/vscode/syntaxes/keleusma.tmLanguage.json \
   "$HOME/Library/Application Support/Sublime Text/Packages/User/"

# Linux
cp editors/vscode/syntaxes/keleusma.tmLanguage.json \
   "$HOME/.config/sublime-text/Packages/User/"

# Windows
cp editors/vscode/syntaxes/keleusma.tmLanguage.json \
   "$APPDATA/Sublime Text/Packages/User/"
```

Sublime auto-detects the syntax on the next file open. Comment toggling, bracket matching, and other affordances handled by `language-configuration.json` in VSCode require a separate `.sublime-settings` or `.tmPreferences` file in Sublime; for V0.2 we ship only the grammar.

## Installing in older Atom / Pulsar

The same TextMate grammar drops into Atom's syntaxes directory. Atom's stable releases are end-of-life; Pulsar (the community continuation) follows Atom's package layout. Submit through Pulsar's package registry rather than dropping in the user directory if that is the deployment.

## Scopes used by this grammar

The grammar uses conventional TextMate scope names so any TextMate-compatible color scheme handles the styling without per-language tuning. Notable choices:

| Scope | Used for |
|-------|----------|
| `keyword.declaration.function-category.kel` | `fn`, `yield`, `loop` |
| `storage.modifier.kel` | `signed`, `ephemeral`, `shared`, `private`, `const` |
| `support.other.checked-arm.kel` | `ok`, `overflow`, `underflow`, `saturate_max`, `saturate_min` |
| `support.other.ifc-label.kel` | `@Label`, `@!Negative`, `@{A, B}`, `@{!A, !B}` |
| `support.other.ifc-operator.kel` | `classify`, `declassify` |
| `keyword.operator.pipeline.kel` | `\|>` |
| `support.type.primitive.kel` | `Byte`, `Word`, `Fixed`, `Float`, `bool`, `Text`, `Option` |
| `entity.name.type.kel` | User-defined type identifiers (uppercase initial) |

The categorisation matches the vim syntax file under [`../vim/syntax/keleusma.vim`](../vim/syntax/keleusma.vim).

## Limitations

- No language server. VSCode shows syntax highlighting and brace matching only; diagnostics from `keleusma run` would need a separate LSP implementation.
- No tree-sitter grammar. Editors that prefer tree-sitter over TextMate (Helix, Zed, recent Neovim with `nvim-treesitter`) need a separate tree-sitter grammar. The repository tracks that as future work.
- No debugger integration. Embedded Keleusma debugs through the host, not the script.

Patches that extend coverage (language-server, formatter integration, debugger adapter) are welcome.
