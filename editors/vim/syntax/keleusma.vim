" Vim syntax file
" Language:    Keleusma
" Maintainer:  Brendan Sechter <sgeos@hotmail.com>
" URL:         https://github.com/sgeos/keleusma
" License:     0BSD

if exists("b:current_syntax")
  finish
endif

" Comments. Line comments and C-style block comments.
syn keyword keleusmaTodo contained TODO FIXME NOTE XXX
syn match   keleusmaLineComment  "//.*$"           contains=keleusmaTodo,@Spell
syn region  keleusmaBlockComment start="/\*" end="\*/" contains=keleusmaTodo,@Spell

" Declaration keywords.
syn keyword keleusmaKeyword fn yield loop let for in use external
syn keyword keleusmaKeyword struct enum newtype where overflow underflow
syn keyword keleusmaKeyword saturate_max saturate_min
syn keyword keleusmaKeyword trait impl shared private const ephemeral signed
syn keyword keleusmaKeyword pure data

" Control flow.
syn keyword keleusmaConditional if else match when
syn keyword keleusmaRepeat for loop break
syn keyword keleusmaOperator not and or as

" Boolean literals.
syn keyword keleusmaBoolean true false

" Primitive and standard types. Type-style identifiers (uppercase
" initial) match the keleusmaType class generically further down;
" the explicit list here gives the well-known V0.2.0 primitives
" priority styling.
syn keyword keleusmaType Word Byte Float Fixed Bool Text Unit
syn keyword keleusmaType Option Vec

" Integer literals: decimal, hexadecimal, binary, with optional
" underscore separators.
syn match keleusmaNumber "\<\d[0-9_]*\>"
syn match keleusmaNumber "\<0x[0-9a-fA-F_]\+\>"
syn match keleusmaNumber "\<0b[01_]\+\>"

" Float literals.
syn match keleusmaFloat "\<\d[0-9_]*\.\d[0-9_]*\>"

" String literals. V0.2.0 retired f-string interpolation; only
" ordinary `"..."` strings remain.
syn region keleusmaString start=+"+ skip=+\\"+ end=+"+ contains=keleusmaEscape,@Spell
syn match  keleusmaEscape contained "\\[ntr\"\\\\0]"

" Pipeline, range, scope, return-arrow, and match-arrow operators.
" The character classes after \v expect Vim 7.0+ very-magic mode.
syn match keleusmaOperator "|>"
syn match keleusmaOperator "->"
syn match keleusmaOperator "=>"
syn match keleusmaOperator "::"
syn match keleusmaOperator "\.\."

" Information-flow label separator.
syn match keleusmaOperator "@"

" Identifiers. The order is significant: type-style identifiers
" (uppercase initial) are highlighted as types; function-call
" identifiers (lowercase, immediately followed by `(`) as
" functions; everything else falls through to default.
syn match keleusmaType     "\<[A-Z][A-Za-z0-9_]*\>"
syn match keleusmaFunction "\<[a-z_][a-zA-Z0-9_]*\>\ze\s*("

" Default-link the syntax groups to standard highlight categories.
hi def link keleusmaTodo         Todo
hi def link keleusmaLineComment  Comment
hi def link keleusmaBlockComment Comment
hi def link keleusmaKeyword      Keyword
hi def link keleusmaConditional  Conditional
hi def link keleusmaRepeat       Repeat
hi def link keleusmaBoolean      Boolean
hi def link keleusmaType         Type
hi def link keleusmaNumber       Number
hi def link keleusmaFloat        Float
hi def link keleusmaString       String
hi def link keleusmaEscape       SpecialChar
hi def link keleusmaOperator     Operator
hi def link keleusmaFunction     Function

let b:current_syntax = "keleusma"
