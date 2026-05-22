" Vim syntax file
" Language:    Keleusma
" Maintainer:  Brendan Sechter <sgeos@hotmail.com>
" URL:         https://github.com/sgeos/keleusma
" License:     0BSD
"
" Highlighting categories audited against `docs/spec/GRAMMAR.md`
" Section 2 (keywords, operators, comments) and Section 3
" (primitive types). The grouping is shaped to help a reader new
" to Keleusma build a mental model of the opinionated bits:
"
"   - keleusmaFunctionCategory (Statement)
"       The three function categories `fn`, `yield`, `loop` get
"       their own group so the reader can see at a glance which
"       category each definition declares. The category drives
"       every verifier rule.
"   - keleusmaStorageClass (StorageClass)
"       The V0.2.0 discipline modifiers `signed`, `ephemeral`,
"       `shared`, `private`, `const`. Vim's StorageClass is the
"       conventional bucket for C/Rust `static`, `extern`, etc.
"   - keleusmaCheckedArm (Special)
"       The numeric overflow construct's arm keywords
"       `overflow`, `underflow`, `saturate_max`, `saturate_min`.
"       Distinct from generic keywords so the checked-arithmetic
"       block is visually obvious to readers encountering it for
"       the first time.
"   - keleusmaIFCOp, keleusmaIFCLabel (Special)
"       The information-flow operators `classify` /
"       `declassify` and the `@Label`, `@!Negative`, `@{A, B}`
"       label annotations. Most programmers have not used IFC,
"       so highlighting the IFC surface as a class signals "this
"       is the language's IFC machinery, look it up if you have
"       not seen it before".
"   - keleusmaPipe (Special)
"       The pipeline operator `|>`. Promoted out of generic
"       Operator so stream-style left-to-right composition is
"       visually distinct from arithmetic.
"
" Override any group locally in your vimrc (e.g.
" `hi link keleusmaIFCLabel WarningMsg`); per-group `hi def link`
" is the conventional escape hatch.

if exists("b:current_syntax")
  finish
endif

" Shebang. Matches only at the very start of the file (the
" `\%^` start-of-file atom).
syn match keleusmaShebang "\%^#!.*$"

" Comments. Line comments and C-style block comments.
syn keyword keleusmaTodo contained TODO FIXME NOTE XXX
syn match   keleusmaLineComment  "//.*$"           contains=keleusmaTodo,@Spell
syn region  keleusmaBlockComment start="/\*" end="\*/" contains=keleusmaTodo,@Spell

" Function-category keywords. The three categories drive every
" verifier rule; promoting them out of generic Keyword helps the
" reader recognise which category a declaration belongs to.
syn keyword keleusmaFunctionCategory fn yield loop

" Control flow.
syn keyword keleusmaConditional if else match when
syn keyword keleusmaRepeat for break

" V0.2.0 storage-discipline modifiers. Linked to StorageClass,
" the conventional Vim bucket for C/Rust `static`, `extern`,
" `const`, etc.
syn keyword keleusmaStorageClass signed ephemeral shared private const

" Reserved keywords that are neither function categories, storage
" modifiers, nor logical operators.
syn keyword keleusmaKeyword let in use external struct enum newtype
syn keyword keleusmaKeyword trait impl data pure where
syn keyword keleusmaKeyword as not and or

" Numeric overflow construct's arm keywords. The construct is one
" of Keleusma's headline V0.2 additions; highlighting the arm
" keywords as Special makes the construct visually distinct from
" surrounding code.
syn keyword keleusmaCheckedArm ok overflow underflow saturate_max saturate_min

" Boolean literals.
syn keyword keleusmaBoolean true false

" Information-flow operators. `classify` and `declassify` are
" recognised by the parser as context-sensitive operators in
" expression position when not followed by `(`. Vim regex cannot
" distinguish expression-position from call-position perfectly,
" so the match is unconditional. Convention is not to use
" `classify` or `declassify` as user-defined function names.
syn keyword keleusmaIFCOp classify declassify

" Primitive and bundled types. Surface forms per
" `docs/spec/GRAMMAR.md` Section 3. Note `bool` is lowercase;
" `Byte`, `Word`, `Fixed`, `Float`, `Text` are uppercase. The
" `Fixed<N>` parameterised form is matched by the same rule
" because Vim's keyword match stops at the identifier boundary.
syn keyword keleusmaType Byte Word Fixed Float bool Text Option

" Type-style identifiers (uppercase initial). Caught last so the
" specific primitive list above takes priority.
syn match   keleusmaType "\<[A-Z][A-Za-z0-9_]*\>"

" Integer literals: decimal, hexadecimal, binary, with optional
" `i64` suffix.
syn match keleusmaNumber "\<\d[0-9_]*\(i64\)\?\>"
syn match keleusmaNumber "\<0x[0-9a-fA-F_]\+\>"
syn match keleusmaNumber "\<0b[01_]\+\>"

" Float literals. The grammar requires digits on both sides of
" the decimal point; the optional `f64` suffix is per literal
" suffix rules.
syn match keleusmaFloat "\<\d[0-9_]*\.\d[0-9_]*\(f64\)\?\>"

" String literals. V0.2.0 retired f-string interpolation; only
" ordinary `"..."` strings remain. The escape set is
" `\n \t \r \\ \" \0`.
syn region keleusmaString start=+"+ skip=+\\"+ end=+"+ contains=keleusmaEscape,@Spell
syn match  keleusmaEscape contained "\\[ntr\"\\\\0]"

" Pipeline operator. Promoted out of the generic Operator group
" so the language's left-to-right composition syntax is visually
" distinct from arithmetic operators.
syn match keleusmaPipe "|>"

" Information-flow label annotations. The `@` separator and the
" label name that follows are highlighted together so a labelled
" type like `Word@Open` or `Word@{Read, Write}` stands out from
" the underlying type. Negative labels (`@!Secret`) and brace
" sets (`@{A, B}`, `@{!A, !B}`) are covered by the same rule.
syn match keleusmaIFCLabel "@!\?\([A-Za-z_][A-Za-z0-9_]*\|{[^}]*}\)"

" Other structural and arithmetic operators.
syn match keleusmaOperator "->"
syn match keleusmaOperator "=>"
syn match keleusmaOperator "::"
syn match keleusmaOperator "\.\."

" Function-call identifiers: lowercase ident immediately followed
" by `(`. Caught after the keyword classes so keywords win.
syn match keleusmaFunction "\<[a-z_][a-zA-Z0-9_]*\>\ze\s*("

" Default-link the syntax groups to standard highlight
" categories. Per-group overrides at the user's vimrc are the
" conventional escape hatch (e.g.
" `hi link keleusmaIFCLabel WarningMsg`).
hi def link keleusmaShebang           PreProc
hi def link keleusmaTodo              Todo
hi def link keleusmaLineComment       Comment
hi def link keleusmaBlockComment      Comment
hi def link keleusmaFunctionCategory  Statement
hi def link keleusmaConditional       Conditional
hi def link keleusmaRepeat            Repeat
hi def link keleusmaStorageClass      StorageClass
hi def link keleusmaKeyword           Keyword
hi def link keleusmaCheckedArm        Special
hi def link keleusmaBoolean           Boolean
hi def link keleusmaIFCOp             Special
hi def link keleusmaType              Type
hi def link keleusmaNumber            Number
hi def link keleusmaFloat             Float
hi def link keleusmaString            String
hi def link keleusmaEscape            SpecialChar
hi def link keleusmaPipe              Special
hi def link keleusmaIFCLabel          Special
hi def link keleusmaOperator          Operator
hi def link keleusmaFunction          Function

let b:current_syntax = "keleusma"
