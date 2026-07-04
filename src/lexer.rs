extern crate alloc;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use crate::token::{Span, Token, TokenKind};

/// A numeric-literal type suffix recognized by the lexer. Surface
/// forms are `Word`, `Byte`, `Float`, and `Fixed<N>`. Used only
/// internally to route a suffixed literal to the right token kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NumSuffix {
    /// No suffix present.
    None,
    /// `Word` suffix; the literal is a `Word` (the default integer).
    Word,
    /// `Byte` suffix; the literal is a `Byte`.
    Byte,
    /// `Float` suffix; the literal is a `Float`.
    Float,
    /// `Fixed<N>` suffix; the literal is a fixed-point value with
    /// `N` fraction bits.
    Fixed(u8),
}

/// Lexer error with source location.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    /// Human-readable diagnostic message.
    pub message: String,
    /// Source span of the offending input.
    pub span: Span,
}

/// Tokenize Keleusma source code into a sequence of tokens.
///
/// Produces tokens for all keywords, identifiers, literals, operators,
/// and delimiters defined in the Keleusma grammar specification. Line
/// comments (`//`) and block comments (`/* */`) are skipped. Newlines
/// are treated as whitespace. Semicolons serve as statement terminators.
/// The token stream ends with an Eof token.
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next_token()?;
        let is_eof = token.kind == TokenKind::Eof;
        tokens.push(token);
        if is_eof {
            break;
        }
    }

    Ok(tokens)
}

struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    line: u32,
    line_start: usize,
    /// Pending tokens emitted by multi-token lexer paths such as the
    /// f-string desugaring. `next_token` drains this queue before
    /// scanning fresh input. The queue's tokens already carry
    /// resolved spans pointing into the original source.
    pending: VecDeque<Token>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str) -> Self {
        let bytes = source.as_bytes();
        // If the source begins with a shebang line, skip past the next
        // newline so the lexer starts on line 2. This allows scripts to
        // be Unix-executable through a `#!/usr/bin/env keleusma` prefix.
        // Line numbers in error messages are preserved relative to the
        // original source: the shebang is line 1 and the first lexed
        // token reports line 2.
        let (start, line, line_start) = if bytes.starts_with(b"#!") {
            match bytes.iter().position(|&b| b == b'\n') {
                Some(nl) => (nl + 1, 2, nl + 1),
                None => (bytes.len(), 1, 0),
            }
        } else {
            (0, 1, 0)
        };
        Self {
            source: bytes,
            pos: start,
            line,
            line_start,
            pending: VecDeque::new(),
        }
    }

    fn peek(&self) -> Option<u8> {
        self.source.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.source.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let ch = self.source.get(self.pos).copied()?;
        self.pos += 1;
        Some(ch)
    }

    fn column(&self) -> u32 {
        (self.pos - self.line_start + 1) as u32
    }

    fn span_from(&self, start: usize, start_line: u32, start_col: u32) -> Span {
        Span {
            start,
            end: self.pos,
            line: start_line,
            column: start_col,
        }
    }

    fn error(&self, message: String, start: usize, start_line: u32, start_col: u32) -> LexError {
        LexError {
            message,
            span: self.span_from(start, start_line, start_col),
        }
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') => {
                    self.advance();
                }
                Some(b'\n') => {
                    self.advance();
                    self.line += 1;
                    self.line_start = self.pos;
                }
                Some(b'/') if self.peek_at(1) == Some(b'/') => {
                    // Line comment: skip to end of line.
                    self.advance();
                    self.advance();
                    while let Some(ch) = self.peek() {
                        if ch == b'\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                Some(b'/') if self.peek_at(1) == Some(b'*') => {
                    // Block comment: skip until closing */.
                    let start = self.pos;
                    let start_line = self.line;
                    let start_col = self.column();
                    self.advance(); // consume /
                    self.advance(); // consume *
                    loop {
                        match self.advance() {
                            None => {
                                return Err(self.error(
                                    String::from("unterminated block comment"),
                                    start,
                                    start_line,
                                    start_col,
                                ));
                            }
                            Some(b'\n') => {
                                self.line += 1;
                                self.line_start = self.pos;
                            }
                            Some(b'*') if self.peek() == Some(b'/') => {
                                self.advance(); // consume /
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    fn next_token(&mut self) -> Result<Token, LexError> {
        if let Some(t) = self.pending.pop_front() {
            return Ok(t);
        }
        self.skip_whitespace_and_comments()?;

        let start = self.pos;
        let start_line = self.line;
        let start_col = self.column();

        let ch = match self.advance() {
            Some(ch) => ch,
            None => {
                return Ok(Token {
                    kind: TokenKind::Eof,
                    span: self.span_from(start, start_line, start_col),
                });
            }
        };

        match ch {
            // Delimiters
            b'(' => Ok(Token {
                kind: TokenKind::LParen,
                span: self.span_from(start, start_line, start_col),
            }),
            b')' => Ok(Token {
                kind: TokenKind::RParen,
                span: self.span_from(start, start_line, start_col),
            }),
            b'{' => Ok(Token {
                kind: TokenKind::LBrace,
                span: self.span_from(start, start_line, start_col),
            }),
            b'}' => Ok(Token {
                kind: TokenKind::RBrace,
                span: self.span_from(start, start_line, start_col),
            }),
            b'[' => Ok(Token {
                kind: TokenKind::LBracket,
                span: self.span_from(start, start_line, start_col),
            }),
            b']' => Ok(Token {
                kind: TokenKind::RBracket,
                span: self.span_from(start, start_line, start_col),
            }),
            b',' => Ok(Token {
                kind: TokenKind::Comma,
                span: self.span_from(start, start_line, start_col),
            }),

            // Operators with multi-character variants
            b'+' => Ok(Token {
                kind: TokenKind::Plus,
                span: self.span_from(start, start_line, start_col),
            }),
            b'*' => Ok(Token {
                kind: TokenKind::Star,
                span: self.span_from(start, start_line, start_col),
            }),
            b'%' => Ok(Token {
                kind: TokenKind::Percent,
                span: self.span_from(start, start_line, start_col),
            }),

            b'-' => {
                if self.peek() == Some(b'>') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::Arrow,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Minus,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            // Slash: comments are already consumed by skip_whitespace_and_comments,
            // so a bare / here is the division operator.
            b'/' => Ok(Token {
                kind: TokenKind::Slash,
                span: self.span_from(start, start_line, start_col),
            }),

            b';' => Ok(Token {
                kind: TokenKind::Semicolon,
                span: self.span_from(start, start_line, start_col),
            }),

            b'=' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::EqEq,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else if self.peek() == Some(b'>') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::FatArrow,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Eq,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            b'!' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::NotEq,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else {
                    // Bare `!`. Admissible in V0.2.0 only as the
                    // negative-label prefix inside information-flow
                    // label sets at parameter and return type
                    // positions. The parser rejects out-of-position
                    // appearances; the lexer just emits the token.
                    Ok(Token {
                        kind: TokenKind::Bang,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            b'<' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::LtEq,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else if self.peek() == Some(b'<') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::Shl,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Lt,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            b'>' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::GtEq,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else if self.peek() == Some(b'>') {
                    self.advance();
                    // `>>>` is the logical right shift; `>>` the arithmetic
                    // one. Both also serve as stacked generic closes, which
                    // the parser splits back into single `>` tokens.
                    if self.peek() == Some(b'>') {
                        self.advance();
                        Ok(Token {
                            kind: TokenKind::Ushr,
                            span: self.span_from(start, start_line, start_col),
                        })
                    } else {
                        Ok(Token {
                            kind: TokenKind::Shr,
                            span: self.span_from(start, start_line, start_col),
                        })
                    }
                } else {
                    Ok(Token {
                        kind: TokenKind::Gt,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            b'|' => {
                if self.peek() == Some(b'>') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::Pipe,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Bar,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            b'.' => {
                if self.peek() == Some(b'.') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::DotDot,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Dot,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            b':' => {
                if self.peek() == Some(b':') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::ColonColon,
                        span: self.span_from(start, start_line, start_col),
                    })
                } else {
                    Ok(Token {
                        kind: TokenKind::Colon,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            b'@' => Ok(Token {
                kind: TokenKind::At,
                span: self.span_from(start, start_line, start_col),
            }),

            // String literal
            b'"' => self.lex_string(start, start_line, start_col),

            // Underscore: standalone is a token, with trailing alnum is a lower ident
            b'_' => {
                if self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
                {
                    self.lex_lower_ident(start, start_line, start_col)
                } else {
                    Ok(Token {
                        kind: TokenKind::Underscore,
                        span: self.span_from(start, start_line, start_col),
                    })
                }
            }

            // f-string syntax `f"..."` was removed in V0.2.0. The
            // script-side string-composition primitives are no
            // longer part of the language; hosts that want
            // formatting register dedicated native functions. The
            // bare `f` identifier path is unaffected; only the
            // `f"` two-character prefix is rejected here so that
            // existing scripts using f-strings fail with a clear
            // error rather than silently mis-tokenising.
            b'f' if self.peek() == Some(b'"') => Err(LexError {
                message: String::from(
                    "f-strings were removed in V0.2.0. Use a host native that formats the values you want to interpolate.",
                ),
                span: self.span_from(start, start_line, start_col),
            }),

            // Lowercase identifier or keyword
            b'a'..=b'z' => self.lex_lower_ident(start, start_line, start_col),

            // Uppercase identifier
            b'A'..=b'Z' => self.lex_upper_ident(start, start_line, start_col),

            // Numeric literal
            b'0'..=b'9' => self.lex_number(start, start_line, start_col, ch),

            _ => Err(self.error(
                {
                    let mut msg = String::from("unexpected character '");
                    msg.push(ch as char);
                    msg.push('\'');
                    msg
                },
                start,
                start_line,
                start_col,
            )),
        }
    }

    fn lex_lower_ident(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<Token, LexError> {
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
        {
            self.advance();
        }

        let text = core::str::from_utf8(&self.source[start..self.pos]).unwrap_or("");
        let kind =
            TokenKind::keyword(text).unwrap_or_else(|| TokenKind::LowerIdent(String::from(text)));

        Ok(Token {
            kind,
            span: self.span_from(start, start_line, start_col),
        })
    }

    fn lex_upper_ident(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<Token, LexError> {
        while self.peek().is_some_and(|c| c.is_ascii_alphanumeric()) {
            self.advance();
        }

        let text = core::str::from_utf8(&self.source[start..self.pos]).unwrap_or("");
        let kind = TokenKind::UpperIdent(String::from(text));

        Ok(Token {
            kind,
            span: self.span_from(start, start_line, start_col),
        })
    }

    fn lex_number(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
        first_digit: u8,
    ) -> Result<Token, LexError> {
        // Check for hex or binary prefix.
        if first_digit == b'0' {
            match self.peek() {
                Some(b'x') | Some(b'X') => {
                    self.advance();
                    return self.lex_hex(start, start_line, start_col);
                }
                // Lowercase `0b` is always a binary prefix. Uppercase
                // `0B` is only a binary prefix when a binary digit
                // follows; otherwise the `B` begins the `Byte` numeric
                // suffix, so `0Byte` lexes as the byte literal zero
                // rather than a malformed binary literal.
                Some(b'b') => {
                    self.advance();
                    return self.lex_binary(start, start_line, start_col);
                }
                Some(b'B') if matches!(self.peek_at(1), Some(b'0') | Some(b'1')) => {
                    self.advance();
                    return self.lex_binary(start, start_line, start_col);
                }
                _ => {}
            }
        }

        // Decimal integer or float.
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
        }

        // Check for float (decimal point followed by digit). The
        // float-literal path is only taken when the `floats` feature
        // is enabled at the parent crate; with the feature off the
        // lexer rejects float literals at this point so downstream
        // passes never see a `TokenKind::FloatLit`.
        if self.peek() == Some(b'.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            #[cfg(not(feature = "floats"))]
            {
                return Err(LexError {
                    message: String::from("float literals require the `floats` cargo feature"),
                    span: self.span_from(start, start_line, start_col),
                });
            }
            #[cfg(feature = "floats")]
            {
                self.advance(); // consume '.'
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.advance();
                }
                let value_end = self.pos;
                let text = core::str::from_utf8(&self.source[start..value_end]).unwrap_or("0");
                let value: f64 = text.parse().map_err(|_| LexError {
                    message: alloc::format!("float literal `{}` is not parseable as f64", text),
                    span: self.span_from(start, start_line, start_col),
                })?;
                // A fractional literal admits only the real-valued
                // suffixes `Float` and `Fixed<N>`. An integer type
                // suffix (`Word`, `Byte`) on a fractional literal is
                // rejected.
                let kind = match self.lex_numeric_suffix(start, start_line, start_col)? {
                    NumSuffix::None | NumSuffix::Float => TokenKind::FloatLit(value),
                    NumSuffix::Fixed(n) => {
                        let scale = (1u64 << n) as f64;
                        let scaled = libm::round(value * scale);
                        if !scaled.is_finite()
                            || scaled < i64::MIN as f64
                            || scaled > i64::MAX as f64
                        {
                            return Err(LexError {
                                message: alloc::format!(
                                    "`Fixed<{}>` literal `{}` overflows the fixed-point range",
                                    n,
                                    value
                                ),
                                span: self.span_from(start, start_line, start_col),
                            });
                        }
                        TokenKind::FixedLit(scaled as i64, n)
                    }
                    NumSuffix::Word | NumSuffix::Byte => {
                        return Err(LexError {
                            message: String::from(
                                "integer type suffix is not valid on a fractional literal; use `Float` or `Fixed<N>`",
                            ),
                            span: self.span_from(start, start_line, start_col),
                        });
                    }
                };
                return Ok(Token {
                    kind,
                    span: self.span_from(start, start_line, start_col),
                });
            }
        }

        // A decimal integer literal must fit in i64. Silently
        // truncating an oversize literal to zero would let a typo
        // or generated-source bug compile and run with the wrong
        // value; reject at lex time instead.
        let text = core::str::from_utf8(&self.source[start..self.pos]).unwrap_or("0");
        let value: i64 = text.parse().map_err(|_| LexError {
            message: alloc::format!("integer literal `{}` does not fit in i64", text),
            span: self.span_from(start, start_line, start_col),
        })?;
        // Optional type suffix: `Word`, `Byte`, `Float`, `Fixed<N>`.
        let kind = match self.lex_numeric_suffix(start, start_line, start_col)? {
            NumSuffix::None | NumSuffix::Word => TokenKind::IntLit(value),
            NumSuffix::Byte => {
                if !(0..=0xFF).contains(&value) {
                    return Err(LexError {
                        message: alloc::format!("`Byte` literal {} is out of range 0..=255", value),
                        span: self.span_from(start, start_line, start_col),
                    });
                }
                TokenKind::ByteLit(value as u8)
            }
            NumSuffix::Float => self.int_float_suffix_token(value, start, start_line, start_col)?,
            NumSuffix::Fixed(n) => {
                let raw = (value as i128) << n;
                if raw < i64::MIN as i128 || raw > i64::MAX as i128 {
                    return Err(LexError {
                        message: alloc::format!(
                            "`Fixed<{}>` literal {} overflows the fixed-point range",
                            n,
                            value
                        ),
                        span: self.span_from(start, start_line, start_col),
                    });
                }
                TokenKind::FixedLit(raw as i64, n)
            }
        };
        Ok(Token {
            kind,
            span: self.span_from(start, start_line, start_col),
        })
    }

    /// Scan an optional numeric-literal type suffix immediately
    /// following the digits of a numeric literal. Returns the
    /// recognized suffix, or [`NumSuffix::None`] when the following
    /// bytes do not begin a suffix keyword (in which case `self.pos`
    /// is restored so those bytes lex as their own token). The
    /// `Fixed` keyword commits to suffix parsing and requires a
    /// `<N>` fraction-bit argument, so a bare `Fixed` immediately
    /// following digits is an error rather than a separate token.
    fn lex_numeric_suffix(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<NumSuffix, LexError> {
        // Suffix keywords are uppercase type names. A non-uppercase
        // (or absent) next byte means there is no suffix.
        if !self.peek().is_some_and(|c| c.is_ascii_uppercase()) {
            return Ok(NumSuffix::None);
        }
        let ident_start = self.pos;
        while self
            .peek()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            self.advance();
        }
        let ident = core::str::from_utf8(&self.source[ident_start..self.pos]).unwrap_or("");
        match ident {
            "Word" => Ok(NumSuffix::Word),
            "Byte" => Ok(NumSuffix::Byte),
            "Float" => Ok(NumSuffix::Float),
            "Fixed" => {
                if self.peek() != Some(b'<') {
                    return Err(LexError {
                        message: String::from(
                            "`Fixed` numeric suffix requires a fraction-bit count, e.g. `Fixed<16>`",
                        ),
                        span: self.span_from(start, start_line, start_col),
                    });
                }
                self.advance(); // consume '<'
                let n_start = self.pos;
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    self.advance();
                }
                let n_text = core::str::from_utf8(&self.source[n_start..self.pos]).unwrap_or("");
                let n: u8 = n_text
                    .parse()
                    .ok()
                    .filter(|n: &u8| *n <= 62)
                    .ok_or_else(|| LexError {
                        message: alloc::format!(
                            "`Fixed<N>` fraction bits must be an integer in the range [0, 62], found `{}`",
                            n_text
                        ),
                        span: self.span_from(start, start_line, start_col),
                    })?;
                if self.peek() != Some(b'>') {
                    return Err(LexError {
                        message: String::from(
                            "expected `>` to close the `Fixed<N>` numeric suffix",
                        ),
                        span: self.span_from(start, start_line, start_col),
                    });
                }
                self.advance(); // consume '>'
                Ok(NumSuffix::Fixed(n))
            }
            _ => {
                // Not a recognized suffix; restore the position so
                // the identifier lexes as its own token.
                self.pos = ident_start;
                Ok(NumSuffix::None)
            }
        }
    }

    /// Build the token for a `Float`-suffixed integer-form literal.
    /// Gated on the `floats` feature so that `42Float` is rejected
    /// at lex time in integer-only builds, matching the treatment of
    /// fractional float literals.
    #[cfg(feature = "floats")]
    fn int_float_suffix_token(
        &self,
        value: i64,
        _start: usize,
        _start_line: u32,
        _start_col: u32,
    ) -> Result<TokenKind, LexError> {
        Ok(TokenKind::FloatLit(value as f64))
    }

    /// Integer-only build: the `Float` suffix is unavailable.
    #[cfg(not(feature = "floats"))]
    fn int_float_suffix_token(
        &self,
        _value: i64,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<TokenKind, LexError> {
        Err(LexError {
            message: String::from("the `Float` numeric suffix requires the `floats` cargo feature"),
            span: self.span_from(start, start_line, start_col),
        })
    }

    fn lex_hex(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<Token, LexError> {
        let digit_start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            self.advance();
        }
        if self.pos == digit_start {
            return Err(self.error(
                String::from("expected hex digits after '0x'"),
                start,
                start_line,
                start_col,
            ));
        }
        let hex_text = core::str::from_utf8(&self.source[digit_start..self.pos]).unwrap_or("0");
        let value = i64::from_str_radix(hex_text, 16).map_err(|_| LexError {
            message: alloc::format!("hex literal `0x{}` does not fit in i64", hex_text),
            span: self.span_from(start, start_line, start_col),
        })?;
        Ok(Token {
            kind: TokenKind::IntLit(value),
            span: self.span_from(start, start_line, start_col),
        })
    }

    fn lex_binary(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<Token, LexError> {
        let digit_start = self.pos;
        while self.peek().is_some_and(|c| c == b'0' || c == b'1') {
            self.advance();
        }
        if self.pos == digit_start {
            return Err(self.error(
                String::from("expected binary digits after '0b'"),
                start,
                start_line,
                start_col,
            ));
        }
        let bin_text = core::str::from_utf8(&self.source[digit_start..self.pos]).unwrap_or("0");
        let value = i64::from_str_radix(bin_text, 2).map_err(|_| LexError {
            message: alloc::format!("binary literal `0b{}` does not fit in i64", bin_text),
            span: self.span_from(start, start_line, start_col),
        })?;
        Ok(Token {
            kind: TokenKind::IntLit(value),
            span: self.span_from(start, start_line, start_col),
        })
    }

    fn lex_string(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<Token, LexError> {
        let mut value = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(self.error(
                        String::from("unterminated string literal"),
                        start,
                        start_line,
                        start_col,
                    ));
                }
                Some(b'"') => break,
                Some(b'\n') => {
                    return Err(self.error(
                        String::from("newline in string literal"),
                        start,
                        start_line,
                        start_col,
                    ));
                }
                Some(b'\\') => match self.advance() {
                    Some(b'n') => value.push('\n'),
                    Some(b't') => value.push('\t'),
                    Some(b'r') => value.push('\r'),
                    Some(b'\\') => value.push('\\'),
                    Some(b'"') => value.push('"'),
                    Some(b'0') => value.push('\0'),
                    Some(c) => {
                        return Err(self.error(
                            {
                                let mut msg = String::from("unknown escape sequence '\\");
                                msg.push(c as char);
                                msg.push('\'');
                                msg
                            },
                            start,
                            start_line,
                            start_col,
                        ));
                    }
                    None => {
                        return Err(self.error(
                            String::from("unterminated escape sequence"),
                            start,
                            start_line,
                            start_col,
                        ));
                    }
                },
                Some(c) => value.push(c as char),
            }
        }
        Ok(Token {
            kind: TokenKind::StringLit(value),
            span: self.span_from(start, start_line, start_col),
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::string::String;
    use alloc::vec;

    fn kinds(source: &str) -> Vec<TokenKind> {
        tokenize(source)
            .unwrap()
            .into_iter()
            .map(|t| t.kind)
            .collect()
    }

    #[test]
    fn empty_source() {
        let tokens = tokenize("").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn whitespace_only() {
        let tokens = tokenize("   \t  ").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn newlines_are_whitespace() {
        let tokens = tokenize("\n\n\n").unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].kind, TokenKind::Eof);
    }

    #[test]
    fn keywords() {
        let result = kinds(
            "fn yield loop break let for in if else match use struct enum true false as when not and or pure",
        );
        assert_eq!(
            result,
            vec![
                TokenKind::Fn,
                TokenKind::Yield,
                TokenKind::Loop,
                TokenKind::Break,
                TokenKind::Let,
                TokenKind::For,
                TokenKind::In,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::Match,
                TokenKind::Use,
                TokenKind::Struct,
                TokenKind::Enum,
                TokenKind::True,
                TokenKind::False,
                TokenKind::As,
                TokenKind::When,
                TokenKind::Not,
                TokenKind::And,
                TokenKind::Or,
                TokenKind::Pure,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lower_identifiers() {
        let result = kinds("foo bar_baz _private x1 my_var");
        assert_eq!(
            result,
            vec![
                TokenKind::LowerIdent(String::from("foo")),
                TokenKind::LowerIdent(String::from("bar_baz")),
                TokenKind::LowerIdent(String::from("_private")),
                TokenKind::LowerIdent(String::from("x1")),
                TokenKind::LowerIdent(String::from("my_var")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn upper_identifiers() {
        let result = kinds("Foo BarBaz Option AudioCommand");
        assert_eq!(
            result,
            vec![
                TokenKind::UpperIdent(String::from("Foo")),
                TokenKind::UpperIdent(String::from("BarBaz")),
                TokenKind::UpperIdent(String::from("Option")),
                TokenKind::UpperIdent(String::from("AudioCommand")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn integer_literals() {
        let result = kinds("0 42 123 0xff 0b1010 100Word");
        assert_eq!(
            result,
            vec![
                TokenKind::IntLit(0),
                TokenKind::IntLit(42),
                TokenKind::IntLit(123),
                TokenKind::IntLit(0xff),
                TokenKind::IntLit(0b1010),
                // `Word` is the integer type; the suffix is accepted
                // and the literal stays a plain `IntLit`.
                TokenKind::IntLit(100),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    #[cfg(feature = "floats")]
    fn float_literals() {
        let result = kinds("3.25 0.5 100.0 4.75Float");
        assert_eq!(
            result,
            vec![
                TokenKind::FloatLit(3.25),
                TokenKind::FloatLit(0.5),
                TokenKind::FloatLit(100.0),
                TokenKind::FloatLit(4.75),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn numeric_suffix_word_and_byte() {
        // `Word` keeps the literal an integer; `Byte` produces a
        // dedicated byte literal token.
        assert_eq!(
            kinds("7Word 200Byte"),
            vec![
                TokenKind::IntLit(7),
                TokenKind::ByteLit(200),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn byte_zero_suffix_not_binary_prefix() {
        // `0Byte` is the byte literal zero. The uppercase `B` must not
        // be grabbed as a `0B` binary prefix when no binary digit
        // follows.
        assert_eq!(kinds("0Byte"), vec![TokenKind::ByteLit(0), TokenKind::Eof]);
        // Uppercase binary with digits still lexes as binary.
        assert_eq!(
            kinds("0B1010"),
            vec![TokenKind::IntLit(0b1010), TokenKind::Eof]
        );
        // Lowercase `0b` with no digits still errors.
        assert!(tokenize("0b").is_err());
    }

    #[test]
    fn numeric_suffix_byte_out_of_range_rejected() {
        let err = tokenize("300Byte").unwrap_err();
        assert!(
            err.message.contains("out of range 0..=255"),
            "{}",
            err.message
        );
    }

    #[test]
    fn numeric_suffix_fixed_integer_form() {
        // `42Fixed<16>` encodes the Q-format raw value 42 << 16.
        assert_eq!(
            kinds("42Fixed<16>"),
            vec![TokenKind::FixedLit(42 << 16, 16), TokenKind::Eof]
        );
    }

    #[test]
    fn numeric_suffix_fixed_requires_fraction_bits() {
        let err = tokenize("42Fixed").unwrap_err();
        assert!(
            err.message.contains("requires a fraction-bit count"),
            "{}",
            err.message
        );
    }

    #[test]
    fn numeric_suffix_unknown_is_two_tokens() {
        // A non-suffix identifier directly after digits is not
        // consumed as a suffix; it lexes as its own token.
        assert_eq!(
            kinds("42Foo"),
            vec![
                TokenKind::IntLit(42),
                TokenKind::UpperIdent(String::from("Foo")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn i64_f64_suffixes_removed() {
        // The retired `i64`/`f64` suffixes now lex as a separate
        // identifier following the numeric literal.
        assert_eq!(
            kinds("100i64"),
            vec![
                TokenKind::IntLit(100),
                TokenKind::LowerIdent(String::from("i64")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    #[cfg(feature = "floats")]
    fn numeric_suffix_float_and_fixed_fractional() {
        // `42Float` promotes integer digits to a float; `3.5Fixed<16>`
        // encodes round(3.5 * 2^16); `3.14Word` is rejected.
        assert_eq!(
            kinds("42Float"),
            vec![TokenKind::FloatLit(42.0), TokenKind::Eof]
        );
        assert_eq!(
            kinds("3.5Fixed<16>"),
            vec![
                TokenKind::FixedLit((3.5 * 65536.0) as i64, 16),
                TokenKind::Eof
            ]
        );
        let err = tokenize("3.14Word").unwrap_err();
        assert!(
            err.message
                .contains("integer type suffix is not valid on a fractional literal"),
            "{}",
            err.message
        );
    }

    #[test]
    fn string_literals() {
        let result = kinds(r#""hello" "world\n" "tab\there" "quote\"end" "null\0""#);
        assert_eq!(
            result,
            vec![
                TokenKind::StringLit(String::from("hello")),
                TokenKind::StringLit(String::from("world\n")),
                TokenKind::StringLit(String::from("tab\there")),
                TokenKind::StringLit(String::from("quote\"end")),
                TokenKind::StringLit(String::from("null\0")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn arithmetic_operators() {
        let result = kinds("+ - * / %");
        assert_eq!(
            result,
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Slash,
                TokenKind::Percent,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn comparison_operators() {
        let result = kinds("== != < > <= >=");
        assert_eq!(
            result,
            vec![
                TokenKind::EqEq,
                TokenKind::NotEq,
                TokenKind::Lt,
                TokenKind::Gt,
                TokenKind::LtEq,
                TokenKind::GtEq,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn assignment_and_arrow() {
        let result = kinds("= ->");
        assert_eq!(
            result,
            vec![TokenKind::Eq, TokenKind::Arrow, TokenKind::Eof]
        );
    }

    #[test]
    fn pipeline() {
        let result = kinds("|>");
        assert_eq!(result, vec![TokenKind::Pipe, TokenKind::Eof]);
    }

    #[test]
    fn punctuation() {
        let result = kinds(". .. :: : ; , _");
        assert_eq!(
            result,
            vec![
                TokenKind::Dot,
                TokenKind::DotDot,
                TokenKind::ColonColon,
                TokenKind::Colon,
                TokenKind::Semicolon,
                TokenKind::Comma,
                TokenKind::Underscore,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn delimiters() {
        let result = kinds("( ) { } [ ]");
        assert_eq!(
            result,
            vec![
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::LBrace,
                TokenKind::RBrace,
                TokenKind::LBracket,
                TokenKind::RBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn line_comments_skipped() {
        let result = kinds("foo // this is a comment\nbar");
        assert_eq!(
            result,
            vec![
                TokenKind::LowerIdent(String::from("foo")),
                TokenKind::LowerIdent(String::from("bar")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn block_comments_skipped() {
        let result = kinds("foo /* block comment */ bar");
        assert_eq!(
            result,
            vec![
                TokenKind::LowerIdent(String::from("foo")),
                TokenKind::LowerIdent(String::from("bar")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn multiline_block_comment() {
        let result = kinds("foo /* line1\nline2\nline3 */ bar");
        assert_eq!(
            result,
            vec![
                TokenKind::LowerIdent(String::from("foo")),
                TokenKind::LowerIdent(String::from("bar")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn newlines_between_tokens() {
        let result = kinds("foo\n\n\nbar");
        assert_eq!(
            result,
            vec![
                TokenKind::LowerIdent(String::from("foo")),
                TokenKind::LowerIdent(String::from("bar")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn semicolons_as_separators() {
        let result = kinds("let x = 1; let y = 2;");
        assert_eq!(
            result,
            vec![
                TokenKind::Let,
                TokenKind::LowerIdent(String::from("x")),
                TokenKind::Eq,
                TokenKind::IntLit(1),
                TokenKind::Semicolon,
                TokenKind::Let,
                TokenKind::LowerIdent(String::from("y")),
                TokenKind::Eq,
                TokenKind::IntLit(2),
                TokenKind::Semicolon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn function_signature() {
        let result = kinds("fn process(cmd: AudioCommand) -> AudioAction {");
        assert_eq!(
            result,
            vec![
                TokenKind::Fn,
                TokenKind::LowerIdent(String::from("process")),
                TokenKind::LParen,
                TokenKind::LowerIdent(String::from("cmd")),
                TokenKind::Colon,
                TokenKind::UpperIdent(String::from("AudioCommand")),
                TokenKind::RParen,
                TokenKind::Arrow,
                TokenKind::UpperIdent(String::from("AudioAction")),
                TokenKind::LBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn pipeline_expression() {
        let result = kinds("value |> transform() |> filter(threshold)");
        assert_eq!(
            result,
            vec![
                TokenKind::LowerIdent(String::from("value")),
                TokenKind::Pipe,
                TokenKind::LowerIdent(String::from("transform")),
                TokenKind::LParen,
                TokenKind::RParen,
                TokenKind::Pipe,
                TokenKind::LowerIdent(String::from("filter")),
                TokenKind::LParen,
                TokenKind::LowerIdent(String::from("threshold")),
                TokenKind::RParen,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn enum_variant_path() {
        let result = kinds("Command::NoteOn");
        assert_eq!(
            result,
            vec![
                TokenKind::UpperIdent(String::from("Command")),
                TokenKind::ColonColon,
                TokenKind::UpperIdent(String::from("NoteOn")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn yield_expression() {
        let result = kinds("let input = yield output_expr;");
        assert_eq!(
            result,
            vec![
                TokenKind::Let,
                TokenKind::LowerIdent(String::from("input")),
                TokenKind::Eq,
                TokenKind::Yield,
                TokenKind::LowerIdent(String::from("output_expr")),
                TokenKind::Semicolon,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn match_expression() {
        let result = kinds("match cmd {\n  Command::NoteOn(ch, note, vel) => play(ch, note)\n}");
        assert_eq!(
            result,
            vec![
                TokenKind::Match,
                TokenKind::LowerIdent(String::from("cmd")),
                TokenKind::LBrace,
                TokenKind::UpperIdent(String::from("Command")),
                TokenKind::ColonColon,
                TokenKind::UpperIdent(String::from("NoteOn")),
                TokenKind::LParen,
                TokenKind::LowerIdent(String::from("ch")),
                TokenKind::Comma,
                TokenKind::LowerIdent(String::from("note")),
                TokenKind::Comma,
                TokenKind::LowerIdent(String::from("vel")),
                TokenKind::RParen,
                TokenKind::FatArrow,
                TokenKind::LowerIdent(String::from("play")),
                TokenKind::LParen,
                TokenKind::LowerIdent(String::from("ch")),
                TokenKind::Comma,
                TokenKind::LowerIdent(String::from("note")),
                TokenKind::RParen,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn struct_definition() {
        let result = kinds("struct Note { channel: Word, pitch: Word }");
        assert_eq!(
            result,
            vec![
                TokenKind::Struct,
                TokenKind::UpperIdent(String::from("Note")),
                TokenKind::LBrace,
                TokenKind::LowerIdent(String::from("channel")),
                TokenKind::Colon,
                TokenKind::UpperIdent(String::from("Word")),
                TokenKind::Comma,
                TokenKind::LowerIdent(String::from("pitch")),
                TokenKind::Colon,
                TokenKind::UpperIdent(String::from("Word")),
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn span_tracking() {
        let tokens = tokenize("fn foo").unwrap();
        assert_eq!(
            tokens[0].span,
            Span {
                start: 0,
                end: 2,
                line: 1,
                column: 1
            }
        );
        assert_eq!(
            tokens[1].span,
            Span {
                start: 3,
                end: 6,
                line: 1,
                column: 4
            }
        );
    }

    #[test]
    fn multiline_span_tracking() {
        let tokens = tokenize("fn\nfoo").unwrap();
        // tokens: [Fn, LowerIdent("foo"), Eof]
        assert_eq!(tokens[0].span.line, 1);
        assert_eq!(tokens[1].span.line, 2);
        assert_eq!(tokens[1].span.column, 1);
    }

    #[test]
    fn underscore_standalone() {
        let result = kinds("_ _foo");
        assert_eq!(
            result,
            vec![
                TokenKind::Underscore,
                TokenKind::LowerIdent(String::from("_foo")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn break_keyword() {
        let result = kinds("for i in 0..10 { break; }");
        assert_eq!(
            result,
            vec![
                TokenKind::For,
                TokenKind::LowerIdent(String::from("i")),
                TokenKind::In,
                TokenKind::IntLit(0),
                TokenKind::DotDot,
                TokenKind::IntLit(10),
                TokenKind::LBrace,
                TokenKind::Break,
                TokenKind::Semicolon,
                TokenKind::RBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn error_unterminated_string() {
        let result = tokenize("\"hello");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unterminated"));
    }

    #[test]
    fn error_unknown_escape() {
        let result = tokenize("\"\\q\"");
        assert!(result.is_err());
    }

    #[test]
    fn bare_pipe_is_bar() {
        // `|` alone is now the closure delimiter token. Bare `|` is
        // tokenized as `Bar`. Adjacent `|>` continues to lex as the
        // pipeline operator.
        let result = tokenize("| foo").unwrap();
        assert!(matches!(result[0].kind, TokenKind::Bar));
    }

    #[test]
    fn bare_bang_lexes_to_bang_token() {
        // V0.2.0 admits bare `!` as a token (negative-label
        // prefix in `T@!Label` and `T@{!N, ...}` syntax). The
        // pre-V0.2.0 lexer rejected it; the test is rewritten to
        // pin the new behaviour. Out-of-position appearances are
        // rejected at the parser, not the lexer.
        let tokens = tokenize("! foo").expect("lex");
        assert!(matches!(tokens[0].kind, TokenKind::Bang));
    }

    #[test]
    fn hex_no_digits() {
        let result = tokenize("0x");
        assert!(result.is_err());
    }

    #[test]
    fn binary_no_digits() {
        let result = tokenize("0b");
        assert!(result.is_err());
    }

    #[test]
    fn error_unterminated_block_comment() {
        let result = tokenize("foo /* unclosed comment");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.message.contains("unterminated block comment"));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn guard_clause() {
        let result = kinds("fn severity(level: Float) -> Text when level >= 0.9 {");
        assert!(result.contains(&TokenKind::When));
        assert!(result.contains(&TokenKind::GtEq));
        assert!(result.contains(&TokenKind::FloatLit(0.9)));
    }

    #[test]
    fn for_in_range() {
        let result = kinds("for i in 0..10 {");
        assert_eq!(
            result,
            vec![
                TokenKind::For,
                TokenKind::LowerIdent(String::from("i")),
                TokenKind::In,
                TokenKind::IntLit(0),
                TokenKind::DotDot,
                TokenKind::IntLit(10),
                TokenKind::LBrace,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn array_type() {
        let result = kinds("[Float; 8]");
        assert_eq!(
            result,
            vec![
                TokenKind::LBracket,
                TokenKind::UpperIdent(String::from("Float")),
                TokenKind::Semicolon,
                TokenKind::IntLit(8),
                TokenKind::RBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn block_comment_line_tracking() {
        // Block comment spanning 2 lines: bar should be on line 3.
        let tokens = tokenize("foo\n/* line2\nline3 */\nbar").unwrap();
        let bar = tokens
            .iter()
            .find(|t| t.kind == TokenKind::LowerIdent(String::from("bar")))
            .unwrap();
        assert_eq!(bar.span.line, 4);
        assert_eq!(bar.span.column, 1);
    }

    #[test]
    fn slash_after_comment() {
        // A division operator on the line after a comment.
        let result = kinds("a // comment\n/ b");
        assert_eq!(
            result,
            vec![
                TokenKind::LowerIdent(String::from("a")),
                TokenKind::Slash,
                TokenKind::LowerIdent(String::from("b")),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn fstring_syntax_is_rejected() {
        // V0.2.0: f-strings were removed from the surface. The bare
        // `f` identifier followed by `"` now produces a lex error
        // rather than attempting interpolation. Hosts that want
        // formatting register dedicated native functions.
        let err = tokenize("f\"hello\"").expect_err("expected lex error");
        assert!(
            err.message.contains("f-strings were removed"),
            "unexpected error message: {}",
            err.message
        );
    }
}
