extern crate alloc;
use alloc::string::String;

/// Source location for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    pub kind: TokenKind,
    pub span: Span,
}

/// All token types in the Keleusma language.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Fn,
    Yield,
    Loop,
    Break,
    Let,
    For,
    In,
    If,
    Else,
    Match,
    Use,
    /// `external` keyword as a modifier on a `use` declaration.
    /// Marks the import as an external native call whose iteration
    /// cost is tracked through a per-iteration invocation-count
    /// bound rather than a per-call WCET/WCMU attestation.
    External,
    Struct,
    Enum,
    Newtype,
    Where,
    Overflow,
    Underflow,
    SaturateMax,
    SaturateMin,
    /// `@` separator for information-flow labels on types and
    /// `classify`/`declassify` operators.
    At,
    True,
    False,
    As,
    When,
    Not,
    And,
    Or,
    Pure,
    Data,
    /// `shared` modifier on a `data` declaration. Shared data is
    /// host-visible through `Vm::set_data`/`Vm::get_data` and persists
    /// across resets. Equivalent to today's bare `data` declaration;
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
    Trait,
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
    /// String literal (escape sequences resolved).
    StringLit(String),

    // Arithmetic operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // Comparison operators
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,

    /// `!` punctuation. Distinct from the `not` keyword
    /// ([`TokenKind::Not`]) and from `!=` ([`TokenKind::NotEq`]).
    /// V0.2.0 admits this token only as the negative-label prefix
    /// inside information-flow label sets at parameter and return
    /// type positions: `T@!Label` or `T@{!N1, !N2}`. The lexer
    /// emits the token unconditionally; the parser rejects it
    /// outside its admissible positions.
    Bang,

    // Assignment
    Eq,

    // Pipeline
    Pipe,
    Bar,

    // Punctuation
    Dot,
    DotDot,
    ColonColon,
    Colon,
    Semicolon,
    Comma,
    Arrow,
    FatArrow,
    Underscore,

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    // End of file
    Eof,
}

impl TokenKind {
    /// Check if a string is a keyword and return the corresponding token kind.
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
