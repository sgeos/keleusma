extern crate alloc;
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::vec::Vec;

use crate::token::{Span, Token, TokenKind};

/// Lexer error with source location.
#[derive(Debug, Clone, PartialEq)]
pub struct LexError {
    pub message: String,
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

/// One segment of an f-string. Either a literal text run or a raw
/// expression source captured between `{` and `}` markers.
enum FStringPart {
    Literal(String),
    Interp(String),
}

/// Emit the desugared token stream for an f-string. The first token
/// is returned directly; the remainder are queued in `pending`.
///
/// Synthesized tokens carry the f-string's span as a single source
/// location. Interpolated expressions are recursively tokenized via
/// `tokenize`; the trailing `Eof` token from the recursive lex is
/// dropped. Lex errors inside an interpolation are propagated.
fn emit_fstring_desugar(
    parts: Vec<FStringPart>,
    span: Span,
    pending: &mut VecDeque<Token>,
) -> Result<Token, LexError> {
    // Sequence of token streams that together form the desugared
    // expression. Each stream is one of:
    //   - a single `StringLit` token for a literal segment
    //   - the tokens for `to_string(<expr>)` for an interpolation
    let mut segments: Vec<Vec<Token>> = Vec::new();
    for part in parts {
        match part {
            FStringPart::Literal(s) => {
                segments.push(alloc::vec![Token {
                    kind: TokenKind::StringLit(s),
                    span,
                }]);
            }
            FStringPart::Interp(src) => {
                let inner = tokenize(&src)?;
                let mut to_string_call: Vec<Token> = Vec::with_capacity(inner.len() + 2);
                to_string_call.push(Token {
                    kind: TokenKind::LowerIdent(alloc::string::String::from("to_string")),
                    span,
                });
                to_string_call.push(Token {
                    kind: TokenKind::LParen,
                    span,
                });
                for t in inner.into_iter() {
                    if t.kind == TokenKind::Eof {
                        continue;
                    }
                    to_string_call.push(Token { kind: t.kind, span });
                }
                to_string_call.push(Token {
                    kind: TokenKind::RParen,
                    span,
                });
                segments.push(to_string_call);
            }
        }
    }

    if segments.is_empty() {
        return Ok(Token {
            kind: TokenKind::StringLit(String::new()),
            span,
        });
    }

    // Fold segments left-associatively into a `concat(..., ...)`
    // chain. A single segment passes through unchanged.
    let mut iter = segments.into_iter();
    let mut acc = iter.next().expect("non-empty segments");
    for next in iter {
        let mut wrapped: Vec<Token> = Vec::with_capacity(acc.len() + next.len() + 4);
        wrapped.push(Token {
            kind: TokenKind::LowerIdent(alloc::string::String::from("concat")),
            span,
        });
        wrapped.push(Token {
            kind: TokenKind::LParen,
            span,
        });
        wrapped.extend(acc);
        wrapped.push(Token {
            kind: TokenKind::Comma,
            span,
        });
        wrapped.extend(next);
        wrapped.push(Token {
            kind: TokenKind::RParen,
            span,
        });
        acc = wrapped;
    }

    let mut iter = acc.into_iter();
    let first = iter.next().expect("non-empty desugared stream");
    for t in iter {
        pending.push_back(t);
    }
    Ok(first)
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
        Self {
            source: source.as_bytes(),
            pos: 0,
            line: 1,
            line_start: 0,
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
                    Err(self.error(
                        String::from("unexpected character '!'"),
                        start,
                        start_line,
                        start_col,
                    ))
                }
            }

            b'<' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    Ok(Token {
                        kind: TokenKind::LtEq,
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

            // f-string interpolation prefix: `f"..."` desugars to a
            // `concat`/`to_string` chain. Recognized before the
            // lowercase-ident path so the bare `f` identifier path
            // is unaffected when no `"` follows.
            b'f' if self.peek() == Some(b'"') => {
                // Consume the `"` and scan the f-string body.
                self.advance();
                self.lex_fstring(start, start_line, start_col)
            }

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
                Some(b'b') | Some(b'B') => {
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

        // Check for float (decimal point followed by digit).
        if self.peek() == Some(b'.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            self.advance(); // consume '.'
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                self.advance();
            }
            // Check for f64 suffix.
            if self.peek() == Some(b'f')
                && self.peek_at(1) == Some(b'6')
                && self.peek_at(2) == Some(b'4')
            {
                self.advance();
                self.advance();
                self.advance();
            }
            let text = core::str::from_utf8(&self.source[start..self.pos]).unwrap_or("0");
            let text = text.trim_end_matches("f64");
            let value: f64 = text.parse().unwrap_or(0.0);
            return Ok(Token {
                kind: TokenKind::FloatLit(value),
                span: self.span_from(start, start_line, start_col),
            });
        }

        // Check for i64 suffix.
        if self.peek() == Some(b'i')
            && self.peek_at(1) == Some(b'6')
            && self.peek_at(2) == Some(b'4')
        {
            self.advance();
            self.advance();
            self.advance();
        }

        let text = core::str::from_utf8(&self.source[start..self.pos]).unwrap_or("0");
        let text = text.trim_end_matches("i64");
        let value: i64 = text.parse().unwrap_or(0);
        Ok(Token {
            kind: TokenKind::IntLit(value),
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
        let value = i64::from_str_radix(hex_text, 16).unwrap_or(0);
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
        let value = i64::from_str_radix(bin_text, 2).unwrap_or(0);
        Ok(Token {
            kind: TokenKind::IntLit(value),
            span: self.span_from(start, start_line, start_col),
        })
    }

    /// Lex an f-string `f"text {expr} more"` into a desugared token
    /// stream. The parts are concatenated through repeated calls to
    /// the registered `concat` native; each interpolation expression
    /// is wrapped in `to_string(...)`. The outermost token returned
    /// is the first of the desugared stream and the remainder are
    /// queued in the lexer's `pending` buffer.
    ///
    /// Empty f-strings produce a `StringLit("")`. Single-literal
    /// f-strings produce the bare literal. Single-interpolation
    /// f-strings produce `to_string(<expr>)`. Mixed f-strings produce
    /// a left-associative chain of `concat(...)` calls.
    fn lex_fstring(
        &mut self,
        start: usize,
        start_line: u32,
        start_col: u32,
    ) -> Result<Token, LexError> {
        let mut parts: Vec<FStringPart> = Vec::new();
        let mut current_lit = String::new();
        loop {
            match self.advance() {
                None => {
                    return Err(self.error(
                        String::from("unterminated f-string literal"),
                        start,
                        start_line,
                        start_col,
                    ));
                }
                Some(b'"') => {
                    if !current_lit.is_empty() {
                        parts.push(FStringPart::Literal(core::mem::take(&mut current_lit)));
                    }
                    break;
                }
                Some(b'\n') => {
                    return Err(self.error(
                        String::from("newline in f-string literal"),
                        start,
                        start_line,
                        start_col,
                    ));
                }
                Some(b'\\') => match self.advance() {
                    Some(b'n') => current_lit.push('\n'),
                    Some(b't') => current_lit.push('\t'),
                    Some(b'r') => current_lit.push('\r'),
                    Some(b'\\') => current_lit.push('\\'),
                    Some(b'"') => current_lit.push('"'),
                    Some(b'0') => current_lit.push('\0'),
                    Some(b'{') => current_lit.push('{'),
                    Some(b'}') => current_lit.push('}'),
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
                Some(b'{') => {
                    if !current_lit.is_empty() {
                        parts.push(FStringPart::Literal(core::mem::take(&mut current_lit)));
                    }
                    let interp_start = self.pos;
                    let interp_start_line = self.line;
                    let interp_start_col = self.column();
                    let mut depth: usize = 1;
                    let mut interp_text = String::new();
                    loop {
                        match self.advance() {
                            None => {
                                return Err(self.error(
                                    String::from("unterminated f-string interpolation"),
                                    interp_start,
                                    interp_start_line,
                                    interp_start_col,
                                ));
                            }
                            Some(b'\n') => {
                                return Err(self.error(
                                    String::from("newline inside f-string interpolation"),
                                    interp_start,
                                    interp_start_line,
                                    interp_start_col,
                                ));
                            }
                            Some(b'{') => {
                                depth += 1;
                                interp_text.push('{');
                            }
                            Some(b'}') => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                                interp_text.push('}');
                            }
                            Some(c) => interp_text.push(c as char),
                        }
                    }
                    parts.push(FStringPart::Interp(interp_text));
                }
                Some(b'}') => {
                    return Err(self.error(
                        String::from(
                            "unmatched `}` in f-string; use `\\}` to embed a literal brace",
                        ),
                        start,
                        start_line,
                        start_col,
                    ));
                }
                Some(c) => current_lit.push(c as char),
            }
        }

        let span = self.span_from(start, start_line, start_col);
        emit_fstring_desugar(parts, span, &mut self.pending)
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
        let result = kinds("0 42 123 0xff 0b1010 100i64");
        assert_eq!(
            result,
            vec![
                TokenKind::IntLit(0),
                TokenKind::IntLit(42),
                TokenKind::IntLit(123),
                TokenKind::IntLit(0xff),
                TokenKind::IntLit(0b1010),
                TokenKind::IntLit(100),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn float_literals() {
        let result = kinds("3.25 0.5 100.0 4.75f64");
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
        let result = kinds("struct Note { channel: i64, pitch: i64 }");
        assert_eq!(
            result,
            vec![
                TokenKind::Struct,
                TokenKind::UpperIdent(String::from("Note")),
                TokenKind::LBrace,
                TokenKind::LowerIdent(String::from("channel")),
                TokenKind::Colon,
                TokenKind::LowerIdent(String::from("i64")),
                TokenKind::Comma,
                TokenKind::LowerIdent(String::from("pitch")),
                TokenKind::Colon,
                TokenKind::LowerIdent(String::from("i64")),
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
    fn error_bare_bang() {
        let result = tokenize("! foo");
        assert!(result.is_err());
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
    fn guard_clause() {
        let result = kinds("fn severity(level: f64) -> String when level >= 0.9 {");
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
        let result = kinds("[f64; 8]");
        assert_eq!(
            result,
            vec![
                TokenKind::LBracket,
                TokenKind::LowerIdent(String::from("f64")),
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
}
