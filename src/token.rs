extern crate alloc;
use alloc::string::String;

/// Source location for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Struct,
    Enum,
    True,
    False,
    As,
    When,
    Not,
    And,
    Or,
    Pure,
    Data,

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

    // Assignment
    Eq,

    // Pipeline
    Pipe,

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
            "struct" => Some(TokenKind::Struct),
            "enum" => Some(TokenKind::Enum),
            "true" => Some(TokenKind::True),
            "false" => Some(TokenKind::False),
            "as" => Some(TokenKind::As),
            "when" => Some(TokenKind::When),
            "not" => Some(TokenKind::Not),
            "and" => Some(TokenKind::And),
            "or" => Some(TokenKind::Or),
            "pure" => Some(TokenKind::Pure),
            "data" => Some(TokenKind::Data),
            _ => None,
        }
    }
}
