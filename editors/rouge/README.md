# Rouge Lexer for Keleusma

`keleusma.rb` is a [Rouge](https://github.com/rouge-ruby/rouge) lexer for
Keleusma source. Rouge is the syntax highlighter used by Jekyll and other
static-site toolchains, so this lexer is the path to highlighting Keleusma
code on a Rouge-based website.

The lexer emits standard Rouge token types, so any existing Rouge theme
styles Keleusma without bespoke CSS. Its token coverage mirrors the
TextMate grammar in `../vscode/syntaxes/keleusma.tmLanguage.json`.

## Use with rougify

```sh
rougify highlight path/to/file.kel -r ./keleusma.rb -l keleusma
```

## Use with Jekyll

The stock GitHub Pages branch build runs Jekyll in safe mode and ignores
plugins, so a custom lexer requires a build that permits plugins, for
example a GitHub Actions build.

1. Copy `keleusma.rb` into the site's `_plugins/` directory.
2. Keep `highlighter: rouge` in `_config.yml`.
3. Fence Keleusma code with ` ```keleusma `.

The lexer registers under the tag `keleusma` with the alias `kel` and the
file extension `.kel`.

## Token mapping

| Keleusma construct | Rouge token |
|--------------------|-------------|
| `fn`, `yield`, `loop` | `Keyword::Declaration` |
| `signed`, `ephemeral`, `shared`, `private`, `const` | `Keyword::Reserved` |
| `if`, `else`, `match`, `when`, `for`, `break`, `let`, `in`, `use`, `external`, `struct`, `enum`, `newtype`, `trait`, `impl`, `data`, `pure`, `where`, `as` | `Keyword` |
| `and`, `or`, `not` | `Operator::Word` |
| `Byte`, `Word`, `Fixed`, `Float`, `bool`, `Text`, `Option` | `Keyword::Type` |
| `true`, `false` | `Keyword::Constant` |
| `ok`, `overflow`, `underflow`, `saturate_max`, `saturate_min` | `Name::Builtin` |
| `classify`, `declassify`, `@Label`, `@!Label`, `@{set}` | `Name::Decorator` |
| capitalized names | `Name::Class` |
| integer, float, hex, binary literals | `Num::*` |
| string literals and escapes | `Str::Double`, `Str::Escape` |
| `\|>`, `->`, `=>`, `::`, `..`, comparison, arithmetic, bitwise | `Operator` |

A consuming site that wants the information-flow labels (`Name::Decorator`)
to stand out should ensure its theme styles the `.nd` class, which the
classic GitHub Rouge theme leaves undefined.
