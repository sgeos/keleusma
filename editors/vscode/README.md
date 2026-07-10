# VSCode and Sublime Text syntax for Keleusma

> **Navigation**: [Editors](../README.md) | [Repository Root](../../README.md)

A TextMate grammar plus a language-configuration file. The TextMate format is consumed by Visual Studio Code, Sublime Text 3+, and historically Atom; one grammar covers all three editors.

## Files

| File | Purpose |
|------|---------|
| [`package.json`](./package.json) | VSCode extension manifest. Declares the `keleusma` language id, the `.kel` extension, the grammar, the language-configuration file, the language-client entry point, and the server settings. |
| [`extension.js`](./extension.js) | Language-client entry point. Launches the `keleusma-lsp` server over stdio and connects it to `.kel` documents. Plain JavaScript, no build step. |
| [`language-configuration.json`](./language-configuration.json) | Bracket matching, auto-closing pairs, comment toggle (`//` and `/* */`), surrounding pairs, indentation rules. |
| [`syntaxes/keleusma.tmLanguage.json`](./syntaxes/keleusma.tmLanguage.json) | TextMate grammar covering the V0.2 surface. Categorisation parallels the vim syntax file: function-category keywords, storage-discipline modifiers, word-form operators, checked-arithmetic arm keywords, IFC labels and operators, the pipeline operator, and the rest of the V0.2 grammar. |

## Language server (live diagnostics)

Beyond static highlighting, this extension is a client for [`keleusma-lsp`](../../keleusma-lsp/),
which provides **live diagnostics** — lex, parse, type, and the worst-case-execution-time
and worst-case-memory-usage *verifier rejections* — plus **document symbols** and
**completion**. To enable it:

1. **Build the server.** From the repository root:

   ```sh
   cargo build --release --manifest-path keleusma-lsp/Cargo.toml
   ```

   This produces `keleusma-lsp/target/release/keleusma-lsp`. Alternatively
   `cargo install --path keleusma-lsp` puts `keleusma-lsp` on your `PATH`.

2. **Install the client's dependency.** From `editors/vscode/`:

   ```sh
   npm install
   ```

3. **Point the extension at the server.** If `keleusma-lsp` is on your `PATH`, nothing more
   is needed. Otherwise set `keleusma.server.path` to the built binary's absolute path in
   your VSCode settings. Set `keleusma.server.enable` to `false` for highlighting only.

The client starts automatically the first time you open a `.kel` file.

## Installing in VSCode

### Development install (symlink)

```sh
ln -s "$(pwd)/editors/vscode" ~/.vscode/extensions/keleusma-0.2.2
```

Restart VSCode. Open any `.kel` file; it should show "Keleusma" in the lower-right language indicator. The editor will pick up syntax highlighting, comment toggling, and bracket matching automatically. For live diagnostics, symbols, and completion, follow the language-server steps above (build the server and run `npm install` in this directory).

### Packaged install (vsix)

If you have `vsce` installed:

```sh
cd editors/vscode
npm install
vsce package
code --install-extension keleusma-0.2.2.vsix
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
| `keyword.operator.word.kel` | `and`, `or`, `xor`, `not`, `andalso`, `orelse`, `lsl`, `asl`, `lsr`, `asr`, `band`, `bor`, `bxor`, `bnot` |
| `keyword.operator.pipeline.kel` | `\|>` |
| `support.type.primitive.kel` | `Multiword`, `Byte`, `Word`, `Fixed`, `Float`, `bool`, `Text`, `Option` |
| `entity.name.type.kel` | User-defined type identifiers (uppercase initial) |

The categorisation matches the vim syntax file under [`../vim/syntax/keleusma.vim`](../vim/syntax/keleusma.vim).

## Limitations

- Language-server coverage is diagnostics, document symbols, and completion. Hover and go-to-definition are not yet implemented; they are tracked in the [`keleusma-lsp`](../../keleusma-lsp/) roadmap.
- No tree-sitter grammar. Editors that prefer tree-sitter over TextMate (Helix, Zed, recent Neovim with `nvim-treesitter`) need a separate tree-sitter grammar. The repository tracks that as future work.
- No debugger integration. Embedded Keleusma debugs through the host, not the script.

Patches that extend coverage (language-server, formatter integration, debugger adapter) are welcome.
