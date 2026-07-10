extern crate alloc;
use alloc::string::String;

/// Source location for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Span {
    /// Byte offset of the token start in the source.
    pub start: usize,
    /// Byte offset one past the token end in the source.
    pub end: usize,
    /// 1-based line number.
    pub line: u32,
    /// 1-based column number (byte offset from line start).
    pub column: u32,
}

/// A token produced by the lexer.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The token's variant.
    pub kind: TokenKind,
    /// Source location for error reporting.
    pub span: Span,
}

/// All token types in the Keleusma language.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    /// `fn` keyword.
    Fn,
    /// `yield` keyword.
    Yield,
    /// `loop` keyword.
    Loop,
    /// `break` keyword.
    Break,
    /// `let` keyword.
    Let,
    /// `for` keyword.
    For,
    /// `in` keyword.
    In,
    /// `if` keyword.
    If,
    /// `else` keyword.
    Else,
    /// `match` keyword.
    Match,
    /// `use` keyword (native function import).
    Use,
    /// `external` keyword as a modifier on a `use` declaration.
    /// Marks the import as an external native call whose iteration
    /// cost is tracked through a per-iteration invocation-count
    /// bound rather than a per-call WCET/WCMU attestation.
    External,
    /// `struct` keyword.
    Struct,
    /// `enum` keyword.
    Enum,
    /// `newtype` keyword.
    Newtype,
    /// `where` clause keyword (refinement predicate).
    Where,
    /// `overflow` arm keyword in the numeric overflow construct.
    Overflow,
    /// `underflow` arm keyword in the numeric overflow construct.
    Underflow,
    /// `saturate_max` keyword inside an overflow arm.
    SaturateMax,
    /// `saturate_min` keyword inside an underflow arm.
    SaturateMin,
    /// `@` separator for information-flow labels on types and
    /// `classify`/`declassify` operators.
    At,
    /// `true` boolean literal.
    True,
    /// `false` boolean literal.
    False,
    /// `as` cast keyword.
    As,
    /// `when` guard clause keyword.
    When,
    /// `not` logical-negation keyword.
    Not,
    /// `and` eager (non-short-circuit) logical-and keyword.
    And,
    /// `or` eager (non-short-circuit) logical-or keyword.
    Or,
    /// `xor` eager logical exclusive-or keyword.
    Xor,
    /// `andalso` short-circuit logical-and keyword.
    Andalso,
    /// `orelse` short-circuit logical-or keyword.
    Orelse,
    /// `pure` declaration modifier reserved for future use.
    Pure,
    /// `data` block declaration keyword.
    Data,
    /// `shared` modifier on a `data` declaration. Shared data is the
    /// host-owned borrowed buffer, read and written through
    /// `Vm::get_shared`/`Vm::set_shared`, and persists across resets in
    /// the host's buffer. Equivalent to today's bare `data` declaration;
    /// the modifier is permitted explicitly for symmetry with
    /// `private` and for self-documenting code.
    Shared,
    /// `private` modifier on a `data` declaration. Private data lives
    /// in the arena's persistent region and is not exposed through
    /// the host API. Persists across resets.
    Private,
    /// `const` modifier on a `data` declaration. Const data is
    /// immutable; field reads compile to constant loads; field
    /// writes are compile errors. Each field has a compile-time
    /// literal initializer in the form `field: Type = literal`.
    /// Const data does not consume runtime data-segment slots.
    Const,
    /// `ephemeral` modifier on an entry-point function declaration.
    /// Asserts the module is provably ephemeral as defined in the
    /// verifier rule. The compile pipeline errors when the assertion
    /// does not hold.
    Ephemeral,
    /// `signed` modifier on an entry-point function declaration.
    /// Sets [`crate::wire_format::FLAG_REQUIRES_SIGNATURE`] on the
    /// resulting module so the load-time runtime refuses to admit
    /// the module unless its cryptographic signature verifies
    /// against the host's trust matrix. The keyword is admissible
    /// only on the entry function and on any of the three
    /// function categories (`fn` / `yield` / `loop`).
    Signed,
    /// `trait` declaration keyword.
    Trait,
    /// `impl` block keyword.
    Impl,

    // Identifiers
    /// Lowercase identifier: `[a-z_][a-z0-9_]*`
    LowerIdent(String),
    /// Uppercase identifier: `[A-Z][A-Za-z0-9]*`
    UpperIdent(String),

    // Literals
    /// Integer literal (decimal, hex, or binary).
    IntLit(i64),
    /// Floating-point literal.
    FloatLit(f64),
    /// Byte literal from a `Byte`-suffixed integer literal, e.g.
    /// `42Byte`. The lexer range-checks the value to `0..=255`.
    ByteLit(u8),
    /// Fixed-point literal from a `Fixed<N>`-suffixed numeric
    /// literal, e.g. `42Fixed<16>` or `3.14Fixed<16>`. The first
    /// field is the Q-format raw bit pattern computed by the lexer;
    /// the second is the fraction-bit count `N`.
    FixedLit(i64, u8),
    /// String literal (escape sequences resolved).
    StringLit(String),

    // Arithmetic operators
    /// `+` arithmetic plus.
    Plus,
    /// `-` arithmetic minus (binary and unary).
    Minus,
    /// `*` arithmetic multiply.
    Star,
    /// `/` arithmetic divide.
    Slash,
    /// `%` arithmetic remainder.
    Percent,

    // Comparison operators
    /// `==` equality test.
    EqEq,
    /// `!=` inequality test.
    NotEq,
    /// `<` strict-less-than test.
    Lt,
    /// `>` strict-greater-than test.
    Gt,
    /// `<=` less-than-or-equal test.
    LtEq,
    /// `>=` greater-than-or-equal test.
    GtEq,

    // Shift operators, keyword-named after the assembly mnemonics so the
    // arithmetic-versus-logical choice is explicit and there is no
    // collision with the generic-close `>`.
    /// `lsl` logical (and arithmetic-equivalent) left shift.
    Lsl,
    /// `asl` arithmetic left shift, which admits overflow capture.
    Asl,
    /// `lsr` logical (zero-fill) right shift.
    Lsr,
    /// `asr` arithmetic (sign-preserving) right shift.
    Asr,

    // Bitwise operators, keyword-named to stay distinct from the boolean
    // operators and from the `|` glyph used for match alternation.
    /// `band` bitwise and.
    Band,
    /// `bor` bitwise or.
    Bor,
    /// `bxor` bitwise exclusive or.
    Bxor,
    /// `bnot` bitwise complement (unary prefix).
    Bnot,

    /// `!` punctuation. Distinct from the `not` keyword
    /// ([`TokenKind::Not`]) and from `!=` ([`TokenKind::NotEq`]).
    /// V0.2.0 admits this token only as the negative-label prefix
    /// inside information-flow label sets at parameter and return
    /// type positions: `T@!Label` or `T@{!N1, !N2}`. The lexer
    /// emits the token unconditionally; the parser rejects it
    /// outside its admissible positions.
    Bang,

    // Assignment
    /// `=` assignment operator (data-segment write; let-binding initialiser).
    Eq,

    // Pipeline
    /// `|>` pipeline operator (left-to-right composition).
    Pipe,
    /// `|` bar token (match-arm alternation, IFC label set delimiter).
    Bar,

    // Punctuation
    /// `.` field-access punctuation.
    Dot,
    /// `..` exclusive-range punctuation.
    DotDot,
    /// `::` path separator (enum variants, qualified native names).
    ColonColon,
    /// `:` type-annotation punctuation.
    Colon,
    /// `;` statement separator.
    Semicolon,
    /// `,` list separator.
    Comma,
    /// `->` return-type arrow.
    Arrow,
    /// `=>` match-arm arrow.
    FatArrow,
    /// `_` wildcard pattern / placeholder identifier.
    Underscore,

    // Delimiters
    /// `(` left parenthesis.
    LParen,
    /// `)` right parenthesis.
    RParen,
    /// `{` left brace.
    LBrace,
    /// `}` right brace.
    RBrace,
    /// `[` left bracket.
    LBracket,
    /// `]` right bracket.
    RBracket,

    // End of file
    /// End of input.
    Eof,
}

/// The complete, authoritative list of reserved keyword spellings.
///
/// This is the single source of truth for tooling that must mirror the keyword
/// vocabulary — the editor syntax highlighters, the `keleusma-lsp` completion
/// list, and the release-preflight drift check (see
/// `docs/process/RELEASE_PROCESS.md` step 1a). Every entry is recognized by
/// [`TokenKind::keyword`] (a unit test enforces this). When adding a keyword,
/// update both the [`TokenKind::keyword`] match **and** this list; they are kept
/// adjacent so the pairing is obvious.
pub const KEYWORDS: &[&str] = &[
    "and", "andalso", "as", "asl", "asr", "band", "bnot", "bor", "break", "bxor", "const",
    "data", "else", "enum", "ephemeral", "external", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "lsl", "lsr", "match", "newtype", "not", "or", "orelse", "overflow", "private",
    "pure", "saturate_max", "saturate_min", "shared", "signed", "struct", "trait", "true",
    "underflow", "use", "when", "where", "xor", "yield",
];

impl TokenKind {
    /// Check if a string is a keyword and return the corresponding token kind.
    ///
    /// When adding an arm here, also add its spelling to [`KEYWORDS`].
    pub fn keyword(s: &str) -> Option<TokenKind> {
        match s {
            "fn" => Some(TokenKind::Fn),
            "yield" => Some(TokenKind::Yield),
            "loop" => Some(TokenKind::Loop),
            "break" => Some(TokenKind::Break),
            "let" => Some(TokenKind::Let),
            "for" => Some(TokenKind::For),
            "in" => Some(TokenKind::In),
            "if" => Some(TokenKind::If),
            "else" => Some(TokenKind::Else),
            "match" => Some(TokenKind::Match),
            "use" => Some(TokenKind::Use),
            "external" => Some(TokenKind::External),
            "struct" => Some(TokenKind::Struct),
            "enum" => Some(TokenKind::Enum),
            "newtype" => Some(TokenKind::Newtype),
            "where" => Some(TokenKind::Where),
            "overflow" => Some(TokenKind::Overflow),
            "underflow" => Some(TokenKind::Underflow),
            "saturate_max" => Some(TokenKind::SaturateMax),
            "saturate_min" => Some(TokenKind::SaturateMin),
            "signed" => Some(TokenKind::Signed),
            // `classify` and `declassify` are intentionally NOT
            // reserved as keywords because they may also be used
            // as user-defined function names. The parser
            // recognises them as information-flow operators
            // through context-sensitive lookahead: in expression
            // position, a `classify` or `declassify` identifier
            // followed by anything other than `(` is the operator
            // form; followed by `(` it is a function call.
            "true" => Some(TokenKind::True),
            "false" => Some(TokenKind::False),
            "as" => Some(TokenKind::As),
            "when" => Some(TokenKind::When),
            "not" => Some(TokenKind::Not),
            "and" => Some(TokenKind::And),
            "or" => Some(TokenKind::Or),
            "xor" => Some(TokenKind::Xor),
            "andalso" => Some(TokenKind::Andalso),
            "orelse" => Some(TokenKind::Orelse),
            "lsl" => Some(TokenKind::Lsl),
            "asl" => Some(TokenKind::Asl),
            "lsr" => Some(TokenKind::Lsr),
            "asr" => Some(TokenKind::Asr),
            "band" => Some(TokenKind::Band),
            "bor" => Some(TokenKind::Bor),
            "bxor" => Some(TokenKind::Bxor),
            "bnot" => Some(TokenKind::Bnot),
            "pure" => Some(TokenKind::Pure),
            "data" => Some(TokenKind::Data),
            "shared" => Some(TokenKind::Shared),
            "private" => Some(TokenKind::Private),
            "const" => Some(TokenKind::Const),
            "ephemeral" => Some(TokenKind::Ephemeral),
            "trait" => Some(TokenKind::Trait),
            "impl" => Some(TokenKind::Impl),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_keyword_is_recognized() {
        for kw in KEYWORDS {
            assert!(
                TokenKind::keyword(kw).is_some(),
                "KEYWORDS lists `{kw}`, but TokenKind::keyword does not recognize it"
            );
        }
    }

    #[test]
    fn keywords_list_has_no_duplicates() {
        for (i, a) in KEYWORDS.iter().enumerate() {
            for b in &KEYWORDS[i + 1..] {
                assert_ne!(a, b, "KEYWORDS contains a duplicate: `{a}`");
            }
        }
    }

    #[test]
    fn a_plain_identifier_is_not_a_keyword() {
        assert!(TokenKind::keyword("frobnicate").is_none());
    }
}
