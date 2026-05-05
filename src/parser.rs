extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::ast::*;
use crate::token::{Span, Token, TokenKind};

/// A parse error with a message and source location.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

/// Parse a token stream into a Keleusma AST.
pub fn parse(tokens: &[Token]) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

/// Recursive descent parser for Keleusma.
struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    // --- Lookahead and consumption helpers ---

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span
    }

    fn prev_span(&self) -> Span {
        self.tokens[self.pos - 1].span
    }

    fn at_end(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    /// Check if the current token matches a specific kind (discriminant only).
    fn at(&self, kind: &TokenKind) -> bool {
        core::mem::discriminant(self.peek()) == core::mem::discriminant(kind)
    }

    /// Check if the current token is a lower ident with a specific value.
    fn at_lower(&self, name: &str) -> bool {
        matches!(self.peek(), TokenKind::LowerIdent(s) if s == name)
    }

    /// Check if the current token is an upper ident with a specific value.
    fn at_upper(&self, name: &str) -> bool {
        matches!(self.peek(), TokenKind::UpperIdent(s) if s == name)
    }

    /// Advance the parser position by one token. Returns the span of the consumed token.
    fn bump(&mut self) -> Span {
        let span = self.tokens[self.pos].span;
        self.pos += 1;
        span
    }

    /// Consume the current token if it matches. Returns true if consumed.
    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.at(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Expect and consume a specific token kind, or return an error.
    fn expect(&mut self, kind: &TokenKind) -> Result<Span, ParseError> {
        if self.at(kind) {
            Ok(self.bump())
        } else {
            Err(self.error_expected(kind))
        }
    }

    /// Consume a lower ident and return its name and span.
    fn expect_lower_ident(&mut self) -> Result<(String, Span), ParseError> {
        let tok = self.tokens[self.pos].clone();
        match tok.kind {
            TokenKind::LowerIdent(name) => {
                self.pos += 1;
                Ok((name, tok.span))
            }
            _ => Err(ParseError {
                message: String::from("expected identifier"),
                span: tok.span,
            }),
        }
    }

    /// Consume an upper ident and return its name and span.
    fn expect_upper_ident(&mut self) -> Result<(String, Span), ParseError> {
        let tok = self.tokens[self.pos].clone();
        match tok.kind {
            TokenKind::UpperIdent(name) => {
                self.pos += 1;
                Ok((name, tok.span))
            }
            _ => Err(ParseError {
                message: String::from("expected type name"),
                span: tok.span,
            }),
        }
    }

    fn error(&self, msg: &str) -> ParseError {
        ParseError {
            message: String::from(msg),
            span: self.peek_span(),
        }
    }

    fn error_expected(&self, expected: &TokenKind) -> ParseError {
        ParseError {
            message: format!("expected {:?}", expected),
            span: self.peek_span(),
        }
    }

    // --- Top-level parsing ---

    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let start = self.peek_span();
        let mut uses = Vec::new();
        let mut types = Vec::new();
        let mut functions = Vec::new();

        // Parse use declarations.
        while self.at(&TokenKind::Use) {
            uses.push(self.parse_use_decl()?);
        }

        // Parse type definitions and function definitions.
        while !self.at_end() {
            match self.peek() {
                TokenKind::Struct => types.push(TypeDef::Struct(self.parse_struct_def()?)),
                TokenKind::Enum => types.push(TypeDef::Enum(self.parse_enum_def()?)),
                TokenKind::Fn | TokenKind::Yield | TokenKind::Loop | TokenKind::Pure => {
                    functions.push(self.parse_function_def()?);
                }
                _ => return Err(self.error("expected type definition or function definition")),
            }
        }

        let end = self.peek_span();
        Ok(Program {
            uses,
            types,
            functions,
            span: merge_spans(start, end),
        })
    }

    fn parse_use_decl(&mut self) -> Result<UseDecl, ParseError> {
        let start = self.expect(&TokenKind::Use)?;
        let mut path = Vec::new();

        let (first, _) = self.expect_lower_ident()?;
        path.push(first);

        // Parse path segments: `module::sub::...`
        while self.eat(&TokenKind::ColonColon) {
            // Next could be lower_ident (more path), '*' (wildcard), or final name.
            if self.at(&TokenKind::Star) {
                self.bump();
                let end = self.prev_span();
                return Ok(UseDecl {
                    path,
                    import: ImportItem::Wildcard,
                    span: merge_spans(start, end),
                });
            }
            let (segment, _) = self.expect_lower_ident()?;
            path.push(segment);
        }

        // The last segment is the imported name.
        let import_name = path.pop().unwrap_or_default();
        let end = self.prev_span();
        Ok(UseDecl {
            path,
            import: ImportItem::Name(import_name),
            span: merge_spans(start, end),
        })
    }

    fn parse_struct_def(&mut self) -> Result<StructDef, ParseError> {
        let start = self.expect(&TokenKind::Struct)?;
        let (name, _) = self.expect_upper_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            let (fname, fspan) = self.expect_lower_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ftype = self.parse_type_expr()?;
            let end = ftype.span();
            fields.push(FieldDecl {
                name: fname,
                type_expr: ftype,
                span: merge_spans(fspan, end),
            });
            // Optional trailing comma.
            self.eat(&TokenKind::Comma);
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Ok(StructDef {
            name,
            fields,
            span: merge_spans(start, end),
        })
    }

    fn parse_enum_def(&mut self) -> Result<EnumDef, ParseError> {
        let start = self.expect(&TokenKind::Enum)?;
        let (name, _) = self.expect_upper_ident()?;
        self.expect(&TokenKind::LBrace)?;

        let mut variants = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            let (vname, vspan) = self.expect_upper_ident()?;
            let mut fields = Vec::new();
            let mut end = vspan;
            if self.eat(&TokenKind::LParen) {
                if !self.at(&TokenKind::RParen) {
                    fields.push(self.parse_type_expr()?);
                    while self.eat(&TokenKind::Comma) {
                        if self.at(&TokenKind::RParen) {
                            break;
                        }
                        fields.push(self.parse_type_expr()?);
                    }
                }
                end = self.expect(&TokenKind::RParen)?;
            }
            variants.push(VariantDecl {
                name: vname,
                fields,
                span: merge_spans(vspan, end),
            });
            self.eat(&TokenKind::Comma);
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Ok(EnumDef {
            name,
            variants,
            span: merge_spans(start, end),
        })
    }

    // --- Function parsing ---

    fn parse_function_def(&mut self) -> Result<FunctionDef, ParseError> {
        let start = self.peek_span();

        // Optional `pure` annotation.
        let _pure = self.eat(&TokenKind::Pure);

        let category = match self.peek() {
            TokenKind::Fn => {
                self.bump();
                FunctionCategory::Fn
            }
            TokenKind::Yield => {
                self.bump();
                FunctionCategory::Yield
            }
            TokenKind::Loop => {
                self.bump();
                FunctionCategory::Loop
            }
            _ => return Err(self.error("expected 'fn', 'yield', or 'loop'")),
        };

        let (name, _) = self.expect_lower_ident()?;
        self.expect(&TokenKind::LParen)?;

        let mut params = Vec::new();
        if !self.at(&TokenKind::RParen) {
            params.push(self.parse_param()?);
            while self.eat(&TokenKind::Comma) {
                if self.at(&TokenKind::RParen) {
                    break;
                }
                params.push(self.parse_param()?);
            }
        }
        self.expect(&TokenKind::RParen)?;

        self.expect(&TokenKind::Arrow)?;
        let return_type = self.parse_type_expr()?;

        let guard = if self.eat(&TokenKind::When) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        let body = self.parse_block()?;
        let end = body.span;

        Ok(FunctionDef {
            category,
            name,
            params,
            return_type,
            guard,
            body,
            span: merge_spans(start, end),
        })
    }

    fn parse_param(&mut self) -> Result<Param, ParseError> {
        let pattern = self.parse_pattern()?;
        let start = pattern.span();
        let type_expr = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        let end = type_expr.as_ref().map_or(start, |t| t.span());
        Ok(Param {
            pattern,
            type_expr,
            span: merge_spans(start, end),
        })
    }

    // --- Block and statement parsing ---

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start = self.expect(&TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        let mut tail_expr = None;

        loop {
            if self.at(&TokenKind::RBrace) {
                break;
            }

            match self.peek() {
                TokenKind::Let => {
                    stmts.push(Stmt::Let(self.parse_let_stmt()?));
                }
                TokenKind::For => {
                    stmts.push(Stmt::For(self.parse_for_stmt()?));
                }
                TokenKind::Break => {
                    let span = self.bump();
                    self.expect(&TokenKind::Semicolon)?;
                    stmts.push(Stmt::Break(span));
                }
                _ => {
                    let expr = self.parse_expr()?;
                    if self.eat(&TokenKind::Semicolon) {
                        stmts.push(Stmt::Expr(expr));
                    } else if self.at(&TokenKind::RBrace) {
                        tail_expr = Some(Box::new(expr));
                    } else {
                        return Err(self.error("expected ';' or '}' after expression"));
                    }
                }
            }
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Ok(Block {
            stmts,
            tail_expr,
            span: merge_spans(start, end),
        })
    }

    fn parse_let_stmt(&mut self) -> Result<LetStmt, ParseError> {
        let start = self.expect(&TokenKind::Let)?;
        let pattern = self.parse_pattern()?;
        let type_expr = if self.eat(&TokenKind::Colon) {
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        self.expect(&TokenKind::Eq)?;
        let value = self.parse_expr()?;
        let end = self.expect(&TokenKind::Semicolon)?;
        Ok(LetStmt {
            pattern,
            type_expr,
            value,
            span: merge_spans(start, end),
        })
    }

    fn parse_for_stmt(&mut self) -> Result<ForStmt, ParseError> {
        let start = self.expect(&TokenKind::For)?;
        let (var, _) = self.expect_lower_ident()?;
        self.expect(&TokenKind::In)?;
        let iterable = self.parse_iterable()?;
        let body = self.parse_block()?;
        let end = body.span;
        Ok(ForStmt {
            var,
            iterable,
            body,
            span: merge_spans(start, end),
        })
    }

    fn parse_iterable(&mut self) -> Result<Iterable, ParseError> {
        let expr = self.parse_expr()?;
        if self.eat(&TokenKind::DotDot) {
            let end = self.parse_expr()?;
            Ok(Iterable::Range(Box::new(expr), Box::new(end)))
        } else {
            Ok(Iterable::Expr(expr))
        }
    }

    // --- Expression parsing (precedence climbing) ---

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_pipeline_expr()
    }

    fn parse_pipeline_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_logical_expr()?;

        while self.eat(&TokenKind::Pipe) {
            // Parse qualified function call after |>.
            let (func, func_span) = self.parse_qualified_name()?;
            self.expect(&TokenKind::LParen)?;
            let args = self.parse_arg_list()?;
            let end = self.expect(&TokenKind::RParen)?;
            let span = merge_spans(left.span(), end);
            let _ = func_span;
            left = Expr::Pipeline {
                left: Box::new(left),
                func,
                args,
                span,
            };
        }

        Ok(left)
    }

    fn parse_logical_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_comparison_expr()?;

        loop {
            let op = if self.eat(&TokenKind::And) {
                BinOp::And
            } else if self.eat(&TokenKind::Or) {
                BinOp::Or
            } else {
                break;
            };
            let right = self.parse_comparison_expr()?;
            let span = merge_spans(left.span(), right.span());
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    fn parse_comparison_expr(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_additive_expr()?;

        let op = if self.eat(&TokenKind::EqEq) {
            BinOp::Eq
        } else if self.eat(&TokenKind::NotEq) {
            BinOp::NotEq
        } else if self.eat(&TokenKind::LtEq) {
            BinOp::LtEq
        } else if self.eat(&TokenKind::GtEq) {
            BinOp::GtEq
        } else if self.eat(&TokenKind::Lt) {
            BinOp::Lt
        } else if self.eat(&TokenKind::Gt) {
            BinOp::Gt
        } else {
            return Ok(left);
        };

        let right = self.parse_additive_expr()?;
        let span = merge_spans(left.span(), right.span());
        Ok(Expr::BinOp {
            op,
            left: Box::new(left),
            right: Box::new(right),
            span,
        })
    }

    fn parse_additive_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative_expr()?;

        loop {
            let op = if self.eat(&TokenKind::Plus) {
                BinOp::Add
            } else if self.eat(&TokenKind::Minus) {
                BinOp::Sub
            } else {
                break;
            };
            let right = self.parse_multiplicative_expr()?;
            let span = merge_spans(left.span(), right.span());
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    fn parse_multiplicative_expr(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary_expr()?;

        loop {
            let op = if self.eat(&TokenKind::Star) {
                BinOp::Mul
            } else if self.eat(&TokenKind::Slash) {
                BinOp::Div
            } else if self.eat(&TokenKind::Percent) {
                BinOp::Mod
            } else {
                break;
            };
            let right = self.parse_unary_expr()?;
            let span = merge_spans(left.span(), right.span());
            left = Expr::BinOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }

    fn parse_unary_expr(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&TokenKind::Not) {
            let start = self.prev_span();
            let operand = self.parse_unary_expr()?;
            let span = merge_spans(start, operand.span());
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Not,
                operand: Box::new(operand),
                span,
            });
        }
        if self.eat(&TokenKind::Minus) {
            let start = self.prev_span();
            let operand = self.parse_unary_expr()?;
            let span = merge_spans(start, operand.span());
            return Ok(Expr::UnaryOp {
                op: UnaryOp::Neg,
                operand: Box::new(operand),
                span,
            });
        }
        self.parse_postfix_expr()
    }

    fn parse_postfix_expr(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            if self.eat(&TokenKind::Dot) {
                // Field access or tuple index.
                let tok = self.tokens[self.pos].clone();
                match tok.kind {
                    TokenKind::LowerIdent(field) => {
                        self.pos += 1;
                        let span = merge_spans(expr.span(), tok.span);
                        expr = Expr::FieldAccess {
                            object: Box::new(expr),
                            field,
                            span,
                        };
                    }
                    TokenKind::IntLit(idx) => {
                        self.pos += 1;
                        let span = merge_spans(expr.span(), tok.span);
                        expr = Expr::TupleIndex {
                            object: Box::new(expr),
                            index: idx as u64,
                            span,
                        };
                    }
                    _ => {
                        return Err(ParseError {
                            message: String::from("expected field name or tuple index after '.'"),
                            span: tok.span,
                        });
                    }
                }
            } else if self.eat(&TokenKind::LBracket) {
                let index = self.parse_expr()?;
                let end = self.expect(&TokenKind::RBracket)?;
                let span = merge_spans(expr.span(), end);
                expr = Expr::ArrayIndex {
                    object: Box::new(expr),
                    index: Box::new(index),
                    span,
                };
            } else if self.eat(&TokenKind::As) {
                let target = self.parse_type_expr()?;
                let span = merge_spans(expr.span(), target.span());
                expr = Expr::Cast {
                    expr: Box::new(expr),
                    target,
                    span,
                };
            } else {
                break;
            }
        }

        Ok(expr)
    }

    #[allow(clippy::too_many_lines)]
    fn parse_primary_expr(&mut self) -> Result<Expr, ParseError> {
        let tok = self.tokens[self.pos].clone();

        match tok.kind {
            // Literals.
            TokenKind::IntLit(v) => {
                self.pos += 1;
                Ok(Expr::Literal {
                    value: Literal::Int(v),
                    span: tok.span,
                })
            }
            TokenKind::FloatLit(v) => {
                self.pos += 1;
                Ok(Expr::Literal {
                    value: Literal::Float(v),
                    span: tok.span,
                })
            }
            TokenKind::StringLit(v) => {
                self.pos += 1;
                Ok(Expr::Literal {
                    value: Literal::String(v),
                    span: tok.span,
                })
            }
            TokenKind::True => {
                self.pos += 1;
                Ok(Expr::Literal {
                    value: Literal::Bool(true),
                    span: tok.span,
                })
            }
            TokenKind::False => {
                self.pos += 1;
                Ok(Expr::Literal {
                    value: Literal::Bool(false),
                    span: tok.span,
                })
            }

            // Identifier or qualified function call.
            TokenKind::LowerIdent(_) => {
                let (name, name_span) = self.expect_lower_ident()?;

                // Check for qualified path.
                let mut full_name = name;
                let mut end_span = name_span;
                while self.at(&TokenKind::ColonColon) {
                    // Peek ahead to see if next after :: is a lower ident.
                    if self.pos + 1 < self.tokens.len() {
                        if let TokenKind::LowerIdent(_) = &self.tokens[self.pos + 1].kind {
                            self.pos += 1; // consume ::
                            let (next, next_span) = self.expect_lower_ident()?;
                            full_name.push_str("::");
                            full_name.push_str(&next);
                            end_span = next_span;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // Function call?
                if self.at(&TokenKind::LParen) {
                    self.pos += 1;
                    let args = self.parse_arg_list()?;
                    let end = self.expect(&TokenKind::RParen)?;
                    let span = merge_spans(name_span, end);
                    Ok(Expr::Call {
                        name: full_name,
                        args,
                        span,
                    })
                } else {
                    Ok(Expr::Ident {
                        name: full_name,
                        span: merge_spans(name_span, end_span),
                    })
                }
            }

            // Upper ident: enum variant or struct init.
            TokenKind::UpperIdent(_) => {
                let (name, name_span) = self.expect_upper_ident()?;

                if self.eat(&TokenKind::ColonColon) {
                    // Enum variant.
                    let (variant, _) = self.expect_upper_ident()?;
                    let args = if self.at(&TokenKind::LParen) {
                        self.pos += 1;
                        let a = self.parse_arg_list()?;
                        self.expect(&TokenKind::RParen)?;
                        a
                    } else {
                        Vec::new()
                    };
                    let end = self.prev_span();
                    Ok(Expr::EnumVariant {
                        enum_name: name,
                        variant,
                        args,
                        span: merge_spans(name_span, end),
                    })
                } else if self.at(&TokenKind::LBrace) {
                    // Struct init.
                    self.pos += 1;
                    let mut fields = Vec::new();
                    while !self.at(&TokenKind::RBrace) {
                        let (fname, fspan) = self.expect_lower_ident()?;
                        self.expect(&TokenKind::Colon)?;
                        let value = self.parse_expr()?;
                        let end = value.span();
                        fields.push(FieldInit {
                            name: fname,
                            value,
                            span: merge_spans(fspan, end),
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self.expect(&TokenKind::RBrace)?;
                    Ok(Expr::StructInit {
                        name,
                        fields,
                        span: merge_spans(name_span, end),
                    })
                } else {
                    Err(ParseError {
                        message: String::from("expected '::' or '{' after type name in expression"),
                        span: name_span,
                    })
                }
            }

            // Yield expression.
            TokenKind::Yield => {
                self.pos += 1;
                let value = self.parse_expr()?;
                let span = merge_spans(tok.span, value.span());
                Ok(Expr::Yield {
                    value: Box::new(value),
                    span,
                })
            }

            // If expression.
            TokenKind::If => {
                self.pos += 1;
                let condition = self.parse_expr()?;
                let then_block = self.parse_block()?;
                let else_block = if self.eat(&TokenKind::Else) {
                    Some(self.parse_block()?)
                } else {
                    None
                };
                let end = else_block.as_ref().map_or(then_block.span, |b| b.span);
                Ok(Expr::If {
                    condition: Box::new(condition),
                    then_block,
                    else_block,
                    span: merge_spans(tok.span, end),
                })
            }

            // Match expression.
            TokenKind::Match => {
                self.pos += 1;
                let scrutinee = self.parse_expr()?;
                self.expect(&TokenKind::LBrace)?;
                let mut arms = Vec::new();
                while !self.at(&TokenKind::RBrace) {
                    let pattern = self.parse_pattern()?;
                    self.expect(&TokenKind::FatArrow)?;
                    let expr = self.parse_expr()?;
                    let arm_span = merge_spans(pattern.span(), expr.span());
                    arms.push(MatchArm {
                        pattern,
                        expr,
                        span: arm_span,
                    });
                    self.eat(&TokenKind::Comma);
                }
                let end = self.expect(&TokenKind::RBrace)?;
                Ok(Expr::Match {
                    scrutinee: Box::new(scrutinee),
                    arms,
                    span: merge_spans(tok.span, end),
                })
            }

            // Loop expression.
            TokenKind::Loop => {
                self.pos += 1;
                let body = self.parse_block()?;
                let span = merge_spans(tok.span, body.span);
                Ok(Expr::Loop { body, span })
            }

            // Parenthesized expression.
            TokenKind::LParen => {
                self.pos += 1;
                if self.eat(&TokenKind::RParen) {
                    // Unit literal.
                    return Ok(Expr::Literal {
                        value: Literal::Int(0),
                        span: merge_spans(tok.span, self.prev_span()),
                    });
                }
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                Ok(expr)
            }

            // Array literal.
            TokenKind::LBracket => {
                self.pos += 1;
                let mut elements = Vec::new();
                if !self.at(&TokenKind::RBracket) {
                    elements.push(self.parse_expr()?);
                    while self.eat(&TokenKind::Comma) {
                        if self.at(&TokenKind::RBracket) {
                            break;
                        }
                        elements.push(self.parse_expr()?);
                    }
                }
                let end = self.expect(&TokenKind::RBracket)?;
                Ok(Expr::ArrayLiteral {
                    elements,
                    span: merge_spans(tok.span, end),
                })
            }

            // Pipeline placeholder.
            TokenKind::Underscore => {
                self.pos += 1;
                Ok(Expr::Placeholder { span: tok.span })
            }

            _ => Err(ParseError {
                message: format!("unexpected token {:?} in expression", tok.kind),
                span: tok.span,
            }),
        }
    }

    // --- Helper for function call arguments ---

    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut args = Vec::new();
        if self.at(&TokenKind::RParen) {
            return Ok(args);
        }
        args.push(self.parse_expr()?);
        while self.eat(&TokenKind::Comma) {
            if self.at(&TokenKind::RParen) {
                break;
            }
            args.push(self.parse_expr()?);
        }
        Ok(args)
    }

    /// Parse a potentially qualified name: `ident` or `module::name`.
    fn parse_qualified_name(&mut self) -> Result<(String, Span), ParseError> {
        let (name, start) = self.expect_lower_ident()?;
        let mut full = name;
        let mut end = start;
        while self.at(&TokenKind::ColonColon) {
            if self.pos + 1 < self.tokens.len() {
                if let TokenKind::LowerIdent(_) = &self.tokens[self.pos + 1].kind {
                    self.pos += 1; // consume ::
                    let (next, next_span) = self.expect_lower_ident()?;
                    full.push_str("::");
                    full.push_str(&next);
                    end = next_span;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        Ok((full, merge_spans(start, end)))
    }

    // --- Type expression parsing ---

    fn parse_type_expr(&mut self) -> Result<TypeExpr, ParseError> {
        let span = self.peek_span();

        // Check for primitive types (lower ident).
        if self.at_lower("i64") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::I64, span));
        }
        if self.at_lower("f64") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::F64, span));
        }
        if self.at_lower("bool") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::Bool, span));
        }

        // Check for String (upper ident).
        if self.at_upper("String") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::KString, span));
        }

        // Option<T>.
        if self.at_upper("Option") {
            self.pos += 1;
            self.expect(&TokenKind::Lt)?;
            let inner = self.parse_type_expr()?;
            let end = self.expect(&TokenKind::Gt)?;
            return Ok(TypeExpr::Option(Box::new(inner), merge_spans(span, end)));
        }

        // Named type (other upper ident).
        if self.at(&TokenKind::UpperIdent(String::new())) {
            let (name, name_span) = self.expect_upper_ident()?;
            return Ok(TypeExpr::Named(name, name_span));
        }

        // Unit type `()` or tuple type `(T, U, ...)`.
        if self.eat(&TokenKind::LParen) {
            if self.eat(&TokenKind::RParen) {
                return Ok(TypeExpr::Unit(merge_spans(span, self.prev_span())));
            }
            let first = self.parse_type_expr()?;
            if self.eat(&TokenKind::Comma) {
                let mut types = vec![first];
                types.push(self.parse_type_expr()?);
                while self.eat(&TokenKind::Comma) {
                    if self.at(&TokenKind::RParen) {
                        break;
                    }
                    types.push(self.parse_type_expr()?);
                }
                let end = self.expect(&TokenKind::RParen)?;
                return Ok(TypeExpr::Tuple(types, merge_spans(span, end)));
            }
            let end = self.expect(&TokenKind::RParen)?;
            // Single type in parens - just return the inner type.
            let _ = end;
            return Ok(first);
        }

        // Array type `[T; N]`.
        if self.eat(&TokenKind::LBracket) {
            let elem = self.parse_type_expr()?;
            self.expect(&TokenKind::Semicolon)?;
            let tok = self.tokens[self.pos].clone();
            let size = match tok.kind {
                TokenKind::IntLit(n) => {
                    self.pos += 1;
                    n
                }
                _ => return Err(self.error("expected integer for array size")),
            };
            let end = self.expect(&TokenKind::RBracket)?;
            return Ok(TypeExpr::Array(
                Box::new(elem),
                size,
                merge_spans(span, end),
            ));
        }

        Err(self.error("expected type"))
    }

    // --- Pattern parsing ---

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let tok = self.tokens[self.pos].clone();

        match tok.kind {
            // Wildcard.
            TokenKind::Underscore => {
                self.pos += 1;
                Ok(Pattern::Wildcard(tok.span))
            }

            // Boolean literals.
            TokenKind::True => {
                self.pos += 1;
                Ok(Pattern::Literal(Literal::Bool(true), tok.span))
            }
            TokenKind::False => {
                self.pos += 1;
                Ok(Pattern::Literal(Literal::Bool(false), tok.span))
            }

            // Numeric literals.
            TokenKind::IntLit(v) => {
                self.pos += 1;
                Ok(Pattern::Literal(Literal::Int(v), tok.span))
            }
            TokenKind::FloatLit(v) => {
                self.pos += 1;
                Ok(Pattern::Literal(Literal::Float(v), tok.span))
            }

            // String literal.
            TokenKind::StringLit(v) => {
                self.pos += 1;
                Ok(Pattern::Literal(Literal::String(v), tok.span))
            }

            // Lower ident: variable binding.
            TokenKind::LowerIdent(name) => {
                self.pos += 1;
                Ok(Pattern::Variable(name, tok.span))
            }

            // Upper ident: enum variant or struct pattern.
            TokenKind::UpperIdent(_) => {
                let (name, name_span) = self.expect_upper_ident()?;

                if self.eat(&TokenKind::ColonColon) {
                    // Enum variant pattern.
                    let (variant, _) = self.expect_upper_ident()?;
                    let mut subpatterns = Vec::new();
                    if self.eat(&TokenKind::LParen) {
                        if !self.at(&TokenKind::RParen) {
                            subpatterns.push(self.parse_pattern()?);
                            while self.eat(&TokenKind::Comma) {
                                if self.at(&TokenKind::RParen) {
                                    break;
                                }
                                subpatterns.push(self.parse_pattern()?);
                            }
                        }
                        self.expect(&TokenKind::RParen)?;
                    }
                    let end = self.prev_span();
                    Ok(Pattern::Enum(
                        name,
                        variant,
                        subpatterns,
                        merge_spans(name_span, end),
                    ))
                } else if self.at(&TokenKind::LBrace) {
                    // Struct pattern.
                    self.pos += 1;
                    let mut fields = Vec::new();
                    while !self.at(&TokenKind::RBrace) {
                        let (fname, fspan) = self.expect_lower_ident()?;
                        let pat = if self.eat(&TokenKind::Colon) {
                            Some(self.parse_pattern()?)
                        } else {
                            None
                        };
                        let end = pat.as_ref().map_or(fspan, |p| p.span());
                        fields.push(FieldPattern {
                            name: fname,
                            pattern: pat,
                            span: merge_spans(fspan, end),
                        });
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    let end = self.expect(&TokenKind::RBrace)?;
                    Ok(Pattern::Struct(name, fields, merge_spans(name_span, end)))
                } else {
                    // Bare type name as pattern (unit enum variant without ::).
                    // This is not valid per the grammar, treat as error.
                    Err(ParseError {
                        message: String::from("expected '::' or '{' after type name in pattern"),
                        span: name_span,
                    })
                }
            }

            // Tuple pattern.
            TokenKind::LParen => {
                self.pos += 1;
                let mut patterns = Vec::new();
                if !self.at(&TokenKind::RParen) {
                    patterns.push(self.parse_pattern()?);
                    while self.eat(&TokenKind::Comma) {
                        if self.at(&TokenKind::RParen) {
                            break;
                        }
                        patterns.push(self.parse_pattern()?);
                    }
                }
                let end = self.expect(&TokenKind::RParen)?;
                Ok(Pattern::Tuple(patterns, merge_spans(tok.span, end)))
            }

            _ => Err(ParseError {
                message: format!("unexpected token {:?} in pattern", tok.kind),
                span: tok.span,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_str(src: &str) -> Result<Program, ParseError> {
        let tokens = tokenize(src).expect("lexer error");
        parse(&tokens)
    }

    fn parse_expr_str(src: &str) -> Result<Expr, ParseError> {
        // Wrap in a function to parse a single expression.
        let wrapped = alloc::format!("fn test() -> i64 {{ {} }}", src);
        let program = parse_str(&wrapped)?;
        let body = &program.functions[0].body;
        body.tail_expr
            .as_ref()
            .cloned()
            .map(|b| *b)
            .ok_or_else(|| ParseError {
                message: String::from("no tail expression"),
                span: body.span,
            })
    }

    #[test]
    fn parse_integer_literal() {
        let expr = parse_expr_str("42").unwrap();
        assert!(matches!(
            expr,
            Expr::Literal {
                value: Literal::Int(42),
                ..
            }
        ));
    }

    #[test]
    fn parse_float_literal() {
        let expr = parse_expr_str("2.75").unwrap();
        assert!(
            matches!(expr, Expr::Literal { value: Literal::Float(v), .. } if (v - 2.75).abs() < 1e-10)
        );
    }

    #[test]
    fn parse_string_literal() {
        let expr = parse_expr_str("\"hello\"").unwrap();
        assert!(
            matches!(expr, Expr::Literal { value: Literal::String(ref s), .. } if s == "hello")
        );
    }

    #[test]
    fn parse_bool_literals() {
        let t = parse_expr_str("true").unwrap();
        assert!(matches!(
            t,
            Expr::Literal {
                value: Literal::Bool(true),
                ..
            }
        ));
        let f = parse_expr_str("false").unwrap();
        assert!(matches!(
            f,
            Expr::Literal {
                value: Literal::Bool(false),
                ..
            }
        ));
    }

    #[test]
    fn parse_identifier() {
        let expr = parse_expr_str("x").unwrap();
        assert!(matches!(expr, Expr::Ident { ref name, .. } if name == "x"));
    }

    #[test]
    fn parse_binary_arithmetic() {
        let expr = parse_expr_str("a + b * c").unwrap();
        // Should be Add(a, Mul(b, c)) due to precedence.
        match expr {
            Expr::BinOp {
                op: BinOp::Add,
                ref left,
                ref right,
                ..
            } => {
                assert!(matches!(**left, Expr::Ident { ref name, .. } if name == "a"));
                assert!(matches!(**right, Expr::BinOp { op: BinOp::Mul, .. }));
            }
            _ => panic!("expected BinOp::Add, got {:?}", expr),
        }
    }

    #[test]
    fn parse_comparison() {
        let expr = parse_expr_str("x > 10").unwrap();
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Gt, .. }));
    }

    #[test]
    fn parse_logical_and_or() {
        let expr = parse_expr_str("a and b or c").unwrap();
        // `and` and `or` are same precedence, left-to-right.
        match expr {
            Expr::BinOp {
                op: BinOp::Or,
                ref left,
                ..
            } => {
                assert!(matches!(**left, Expr::BinOp { op: BinOp::And, .. }));
            }
            _ => panic!("expected Or, got {:?}", expr),
        }
    }

    #[test]
    fn parse_unary_not() {
        let expr = parse_expr_str("not true").unwrap();
        assert!(matches!(
            expr,
            Expr::UnaryOp {
                op: UnaryOp::Not,
                ..
            }
        ));
    }

    #[test]
    fn parse_unary_neg() {
        let expr = parse_expr_str("-x").unwrap();
        assert!(matches!(
            expr,
            Expr::UnaryOp {
                op: UnaryOp::Neg,
                ..
            }
        ));
    }

    #[test]
    fn parse_function_call() {
        let expr = parse_expr_str("foo(1, 2, 3)").unwrap();
        match expr {
            Expr::Call {
                ref name, ref args, ..
            } => {
                assert_eq!(name, "foo");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("expected Call, got {:?}", expr),
        }
    }

    #[test]
    fn parse_qualified_call() {
        let expr = parse_expr_str("audio::set_freq(440.0)").unwrap();
        match expr {
            Expr::Call {
                ref name, ref args, ..
            } => {
                assert_eq!(name, "audio::set_freq");
                assert_eq!(args.len(), 1);
            }
            _ => panic!("expected Call, got {:?}", expr),
        }
    }

    #[test]
    fn parse_enum_variant() {
        let expr = parse_expr_str("Command::NoteOn(1, 60, 0.8)").unwrap();
        match expr {
            Expr::EnumVariant {
                ref enum_name,
                ref variant,
                ref args,
                ..
            } => {
                assert_eq!(enum_name, "Command");
                assert_eq!(variant, "NoteOn");
                assert_eq!(args.len(), 3);
            }
            _ => panic!("expected EnumVariant, got {:?}", expr),
        }
    }

    #[test]
    fn parse_unit_enum_variant() {
        let expr = parse_expr_str("Command::Silence").unwrap();
        match expr {
            Expr::EnumVariant {
                ref enum_name,
                ref variant,
                ref args,
                ..
            } => {
                assert_eq!(enum_name, "Command");
                assert_eq!(variant, "Silence");
                assert!(args.is_empty());
            }
            _ => panic!("expected EnumVariant, got {:?}", expr),
        }
    }

    #[test]
    fn parse_struct_init() {
        let expr = parse_expr_str("Note { channel: 0, pitch: 60 }").unwrap();
        match expr {
            Expr::StructInit {
                ref name,
                ref fields,
                ..
            } => {
                assert_eq!(name, "Note");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "channel");
                assert_eq!(fields[1].name, "pitch");
            }
            _ => panic!("expected StructInit, got {:?}", expr),
        }
    }

    #[test]
    fn parse_field_access() {
        let expr = parse_expr_str("note.pitch").unwrap();
        match expr {
            Expr::FieldAccess { ref field, .. } => {
                assert_eq!(field, "pitch");
            }
            _ => panic!("expected FieldAccess, got {:?}", expr),
        }
    }

    #[test]
    fn parse_array_index() {
        let expr = parse_expr_str("arr[0]").unwrap();
        assert!(matches!(expr, Expr::ArrayIndex { .. }));
    }

    #[test]
    fn parse_cast() {
        let expr = parse_expr_str("x as f64").unwrap();
        match expr {
            Expr::Cast { ref target, .. } => {
                assert!(matches!(target, TypeExpr::Prim(PrimType::F64, _)));
            }
            _ => panic!("expected Cast, got {:?}", expr),
        }
    }

    #[test]
    fn parse_array_literal() {
        let expr = parse_expr_str("[1, 2, 3]").unwrap();
        match expr {
            Expr::ArrayLiteral { ref elements, .. } => {
                assert_eq!(elements.len(), 3);
            }
            _ => panic!("expected ArrayLiteral, got {:?}", expr),
        }
    }

    #[test]
    fn parse_if_else() {
        let src = "fn test() -> i64 { if x > 0 { 1 } else { 0 } }";
        let program = parse_str(src).unwrap();
        let tail = program.functions[0].body.tail_expr.as_ref().unwrap();
        assert!(matches!(**tail, Expr::If { ref else_block, .. } if else_block.is_some()));
    }

    #[test]
    fn parse_match_expr() {
        let src = r#"
            fn test() -> i64 {
                match x {
                    0 => 1,
                    _ => 2,
                }
            }
        "#;
        let program = parse_str(src).unwrap();
        let tail = program.functions[0].body.tail_expr.as_ref().unwrap();
        match **tail {
            Expr::Match { ref arms, .. } => assert_eq!(arms.len(), 2),
            _ => panic!("expected Match"),
        }
    }

    #[test]
    fn parse_let_statement() {
        let src = "fn test() -> i64 { let x: i64 = 42; x }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions[0].body.stmts.len(), 1);
        assert!(matches!(&program.functions[0].body.stmts[0], Stmt::Let(_)));
    }

    #[test]
    fn parse_for_range() {
        let src = "fn test() -> i64 { for i in 0..8 { foo(i); } 0 }";
        let program = parse_str(src).unwrap();
        match &program.functions[0].body.stmts[0] {
            Stmt::For(f) => {
                assert_eq!(f.var, "i");
                assert!(matches!(f.iterable, Iterable::Range(_, _)));
            }
            _ => panic!("expected For"),
        }
    }

    #[test]
    fn parse_for_expr_iterable() {
        let src = "fn test() -> i64 { for n in notes { play(n); } 0 }";
        let program = parse_str(src).unwrap();
        match &program.functions[0].body.stmts[0] {
            Stmt::For(f) => {
                assert!(matches!(f.iterable, Iterable::Expr(_)));
            }
            _ => panic!("expected For"),
        }
    }

    #[test]
    fn parse_break_statement() {
        let src = "fn test() -> i64 { for i in 0..8 { break; } 0 }";
        let program = parse_str(src).unwrap();
        let for_stmt = match &program.functions[0].body.stmts[0] {
            Stmt::For(f) => f,
            _ => panic!("expected For"),
        };
        assert!(matches!(&for_stmt.body.stmts[0], Stmt::Break(_)));
    }

    #[test]
    fn parse_fn_definition() {
        let src = "fn add(a: i64, b: i64) -> i64 { a + b }";
        let program = parse_str(src).unwrap();
        let f = &program.functions[0];
        assert_eq!(f.category, FunctionCategory::Fn);
        assert_eq!(f.name, "add");
        assert_eq!(f.params.len(), 2);
        assert!(f.body.tail_expr.is_some());
    }

    #[test]
    fn parse_yield_function() {
        let src = r#"
            yield process(cmd: AudioCommand) -> AudioAction {
                AudioAction::NoOp
            }
        "#;
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions[0].category, FunctionCategory::Yield);
    }

    #[test]
    fn parse_loop_function() {
        let src = r#"
            loop main(cmd: AudioCommand) -> AudioAction {
                let cmd = yield process(cmd);
            }
        "#;
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions[0].category, FunctionCategory::Loop);
    }

    #[test]
    fn parse_guard_clause() {
        let src = r#"
            fn severity(level: f64) -> i64 when level >= 0.9 {
                1
            }
        "#;
        let program = parse_str(src).unwrap();
        assert!(program.functions[0].guard.is_some());
    }

    #[test]
    fn parse_multiheaded_function() {
        let src = r#"
            fn describe(Command::NoteOn(ch, note, vel)) -> i64 {
                1
            }
            fn describe(Command::Silence) -> i64 {
                0
            }
        "#;
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions.len(), 2);
        assert_eq!(program.functions[0].name, "describe");
        assert_eq!(program.functions[1].name, "describe");
    }

    #[test]
    fn parse_use_decl() {
        let src = "use audio::set_frequency fn test() -> i64 { 0 }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.uses.len(), 1);
        assert_eq!(program.uses[0].path, vec!["audio"]);
        assert_eq!(
            program.uses[0].import,
            ImportItem::Name(String::from("set_frequency"))
        );
    }

    #[test]
    fn parse_use_wildcard() {
        let src = "use audio::* fn test() -> i64 { 0 }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.uses[0].import, ImportItem::Wildcard);
    }

    #[test]
    fn parse_struct_def() {
        let src = r#"
            struct Note {
                channel: i64,
                pitch: i64,
                velocity: f64,
            }
            fn test() -> i64 { 0 }
        "#;
        let program = parse_str(src).unwrap();
        assert_eq!(program.types.len(), 1);
        match &program.types[0] {
            TypeDef::Struct(s) => {
                assert_eq!(s.name, "Note");
                assert_eq!(s.fields.len(), 3);
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn parse_enum_def() {
        let src = r#"
            enum Command {
                NoteOn(i64, i64, f64),
                NoteOff(i64),
                Silence,
            }
            fn test() -> i64 { 0 }
        "#;
        let program = parse_str(src).unwrap();
        match &program.types[0] {
            TypeDef::Enum(e) => {
                assert_eq!(e.name, "Command");
                assert_eq!(e.variants.len(), 3);
                assert_eq!(e.variants[0].fields.len(), 3);
                assert!(e.variants[2].fields.is_empty());
            }
            _ => panic!("expected enum"),
        }
    }

    #[test]
    fn parse_pipeline() {
        let expr = parse_expr_str("x |> transform() |> output()").unwrap();
        match expr {
            Expr::Pipeline {
                ref func, ref left, ..
            } => {
                assert_eq!(func, "output");
                assert!(matches!(**left, Expr::Pipeline { .. }));
            }
            _ => panic!("expected Pipeline, got {:?}", expr),
        }
    }

    #[test]
    fn parse_pipeline_with_args() {
        let expr = parse_expr_str("x |> insert(coll, _)").unwrap();
        match expr {
            Expr::Pipeline {
                ref func, ref args, ..
            } => {
                assert_eq!(func, "insert");
                assert_eq!(args.len(), 2);
                assert!(matches!(args[1], Expr::Placeholder { .. }));
            }
            _ => panic!("expected Pipeline, got {:?}", expr),
        }
    }

    #[test]
    fn parse_option_type() {
        let src = "fn test(x: Option<i64>) -> i64 { 0 }";
        let program = parse_str(src).unwrap();
        let param_type = program.functions[0].params[0].type_expr.as_ref().unwrap();
        assert!(matches!(param_type, TypeExpr::Option(_, _)));
    }

    #[test]
    fn parse_array_type() {
        let src = "fn test(x: [f64; 8]) -> i64 { 0 }";
        let program = parse_str(src).unwrap();
        let param_type = program.functions[0].params[0].type_expr.as_ref().unwrap();
        match param_type {
            TypeExpr::Array(_, size, _) => assert_eq!(*size, 8),
            _ => panic!("expected Array type"),
        }
    }

    #[test]
    fn parse_yield_expression() {
        let src = r#"
            loop main(cmd: i64) -> i64 {
                let cmd = yield cmd;
            }
        "#;
        let program = parse_str(src).unwrap();
        match &program.functions[0].body.stmts[0] {
            Stmt::Let(l) => {
                assert!(matches!(l.value, Expr::Yield { .. }));
            }
            _ => panic!("expected Let with yield"),
        }
    }

    #[test]
    fn parse_full_program() {
        let src = r#"
            use audio::*

            enum AudioCommand {
                NoteOn(i64, i64, f64),
                NoteOff(i64),
                Tick,
            }

            enum AudioAction {
                PlayNote(i64, i64, f64),
                StopNote(i64),
                NoOp,
            }

            loop main(cmd: AudioCommand) -> AudioAction {
                let cmd = yield process(cmd);
            }

            fn process(AudioCommand::NoteOn(ch, note, vel)) -> AudioAction {
                AudioAction::PlayNote(ch, note, vel)
            }

            fn process(AudioCommand::NoteOff(ch)) -> AudioAction {
                AudioAction::StopNote(ch)
            }

            fn process(AudioCommand::Tick) -> AudioAction {
                AudioAction::NoOp
            }
        "#;
        let program = parse_str(src).unwrap();
        assert_eq!(program.uses.len(), 1);
        assert_eq!(program.types.len(), 2);
        assert_eq!(program.functions.len(), 4);
    }

    #[test]
    fn error_missing_semicolon() {
        let src = "fn test() -> i64 { let x = 1 x }";
        let result = parse_str(src);
        assert!(result.is_err());
    }

    #[test]
    fn error_unexpected_token() {
        let src = "fn test() -> i64 { + }";
        let result = parse_str(src);
        assert!(result.is_err());
    }
}
