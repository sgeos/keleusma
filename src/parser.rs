extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::ast::*;
use crate::token::{Span, Token, TokenKind};

/// A parse error with a message and source location.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Human-readable diagnostic message.
    pub message: String,
    /// Source span of the offending construct.
    pub span: Span,
}

/// If `expr` is a chain of `ArrayIndex` nodes rooted at a
/// `FieldAccess` whose receiver is a bare identifier in
/// `data_names`, return the `(data_name, field_name, indices,
/// span)` tuple where indices are in source order
/// (outermost-to-innermost). Otherwise return `None`.
///
/// Used by the parser to detect indexed assignment targets such
/// as `state.idx[i][j]` on the left-hand side of `=`.
fn data_indexed_lhs(
    expr: &Expr,
    data_names: &BTreeSet<String>,
) -> Option<(String, String, Vec<Expr>, Span)> {
    let mut indices: Vec<Expr> = Vec::new();
    let mut current = expr;
    let lhs_span = expr.span();
    loop {
        match current {
            Expr::ArrayIndex { object, index, .. } => {
                indices.push((**index).clone());
                current = object.as_ref();
            }
            Expr::FieldAccess { object, field, .. } => {
                if let Expr::Ident { name, .. } = object.as_ref()
                    && data_names.contains(name)
                    && !indices.is_empty()
                {
                    indices.reverse();
                    return Some((name.clone(), field.clone(), indices, lhs_span));
                }
                return None;
            }
            _ => return None,
        }
    }
}

/// Parse a token stream into a Keleusma AST.
pub fn parse(tokens: &[Token]) -> Result<Program, ParseError> {
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

/// Result of parsing an information-flow label spec attached to
/// a type expression. The two variants are exclusive: V0.2.0
/// rejects mixed positive-and-negative sets at parse time.
enum LabelSpec {
    Positive(Vec<String>),
    Negative(Vec<String>),
}

/// Recursive descent parser for Keleusma.
struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
    /// Known data block names, populated during parsing.
    data_names: BTreeSet<String>,
    /// Current depth in the recursive descent. Tracked at the
    /// entry points of every recursive AST node (expressions,
    /// type expressions, patterns). Incremented on entry,
    /// decremented on exit. Exceeding [`MAX_PARSE_DEPTH`] returns
    /// a [`ParseError`] instead of a stack overflow.
    depth: u32,
}

/// Maximum recursive-descent depth before the parser bails with
/// an error. Each level of expression nesting traverses the
/// precedence chain (pipeline → logical → comparison → addition
/// → multiplication → unary → primary), so a single level of
/// parenthesisation consumes roughly 8-10 stack frames. The limit
/// is chosen so that a maximally-nested admissible program
/// consumes well under 2 MiB of stack even in a debug build with
/// fat frames, fitting comfortably inside the default cargo-test
/// thread stack and leaving headroom for the type checker,
/// compiler, and VM passes that follow.
const MAX_PARSE_DEPTH: u32 = 32;

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            data_names: BTreeSet::new(),
            depth: 0,
        }
    }

    /// Enter a recursive parsing step. Increments the depth and
    /// returns `Ok(())` while the depth is within the configured
    /// limit. Recursive parse functions call this on entry and
    /// [`leave_depth`](Self::leave_depth) on exit; the pair
    /// brackets every recursive call site.
    fn enter_depth(&mut self) -> Result<(), ParseError> {
        if self.depth >= MAX_PARSE_DEPTH {
            return Err(ParseError {
                message: format!(
                    "parser recursion depth {} exceeded; deeply nested expressions are rejected to prevent stack overflow",
                    MAX_PARSE_DEPTH
                ),
                span: self.peek_span(),
            });
        }
        self.depth += 1;
        Ok(())
    }

    fn leave_depth(&mut self) {
        self.depth -= 1;
    }

    // --- Lookahead and consumption helpers ---

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_ahead(&self, n: usize) -> &TokenKind {
        let idx = (self.pos + n).min(self.tokens.len() - 1);
        &self.tokens[idx].kind
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
        let mut data_decls = Vec::new();
        let mut functions = Vec::new();

        // Parse use declarations.
        while self.at(&TokenKind::Use) {
            uses.push(self.parse_use_decl()?);
        }

        // Parse type definitions, data declarations, function
        // definitions, traits, and impl blocks.
        let mut traits: Vec<TraitDef> = Vec::new();
        let mut impls: Vec<ImplBlock> = Vec::new();
        while !self.at_end() {
            match self.peek() {
                TokenKind::Struct => types.push(TypeDef::Struct(self.parse_struct_def()?)),
                TokenKind::Enum => types.push(TypeDef::Enum(self.parse_enum_def()?)),
                TokenKind::Newtype => types.push(TypeDef::Newtype(self.parse_newtype_def()?)),
                TokenKind::Data | TokenKind::Shared | TokenKind::Private | TokenKind::Const => {
                    data_decls.push(self.parse_data_decl()?);
                }
                TokenKind::Fn | TokenKind::Yield | TokenKind::Loop | TokenKind::Pure => {
                    functions.push(self.parse_function_def()?);
                }
                TokenKind::Ephemeral | TokenKind::Signed => {
                    functions.push(self.parse_function_def()?);
                }
                TokenKind::Trait => traits.push(self.parse_trait_def()?),
                TokenKind::Impl => impls.push(self.parse_impl_block()?),
                _ => {
                    return Err(self.error(
                        "expected type definition, data declaration, function, trait, or impl",
                    ));
                }
            }
        }

        let end = self.peek_span();
        Ok(Program {
            uses,
            types,
            data_decls,
            functions,
            traits,
            impls,
            span: merge_spans(start, end),
            // Populated by the type checker's recording pass (B28 P3 item 5);
            // empty at parse time.
            fn_expr_types: alloc::collections::BTreeMap::new(),
        })
    }

    fn parse_trait_def(&mut self) -> Result<TraitDef, ParseError> {
        let start = self.expect(&TokenKind::Trait)?;
        let (name, _) = self.expect_upper_ident()?;
        let type_params = self.parse_optional_type_params()?;
        self.expect(&TokenKind::LBrace)?;
        let mut methods: Vec<TraitMethodSig> = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            // Trait method: `fn name(args) -> ret;` (no body, semicolon
            // terminator). The optional `pure` and `fn`/`yield`/`loop`
            // category keywords are accepted in body positions only;
            // trait methods declare the signature shape only.
            self.expect(&TokenKind::Fn)?;
            let (mname, mspan) = self.expect_lower_ident()?;
            self.expect(&TokenKind::LParen)?;
            let mut params: Vec<Param> = Vec::new();
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
            let end = self.expect(&TokenKind::Semicolon)?;
            methods.push(TraitMethodSig {
                name: mname,
                params,
                return_type,
                span: merge_spans(mspan, end),
            });
        }
        let end = self.expect(&TokenKind::RBrace)?;
        Ok(TraitDef {
            name,
            type_params,
            methods,
            span: merge_spans(start, end),
        })
    }

    fn parse_impl_block(&mut self) -> Result<ImplBlock, ParseError> {
        let start = self.expect(&TokenKind::Impl)?;
        let type_params = self.parse_optional_type_params()?;
        let (trait_name, _) = self.expect_upper_ident()?;
        self.expect(&TokenKind::For)?;
        let for_type = self.parse_type_expr()?;
        self.expect(&TokenKind::LBrace)?;
        let mut methods: Vec<FunctionDef> = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            methods.push(self.parse_function_def()?);
        }
        let end = self.expect(&TokenKind::RBrace)?;
        Ok(ImplBlock {
            trait_name,
            type_params,
            for_type,
            methods,
            span: merge_spans(start, end),
        })
    }

    fn parse_use_decl(&mut self) -> Result<UseDecl, ParseError> {
        let start = self.expect(&TokenKind::Use)?;
        // Optional `external` modifier between `use` and the first
        // path segment. Marks the import as an external native
        // (`Op::CallExternalNative`) whose per-iteration cost is
        // bounded by invocation count rather than by an attested
        // per-call WCET/WCMU budget.
        let is_external = self.eat(&TokenKind::External);
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
                    signature: None,
                    is_external,
                    span: merge_spans(start, end),
                });
            }
            let (segment, _) = self.expect_lower_ident()?;
            path.push(segment);
        }

        // The last segment is the imported name.
        let import_name = path.pop().unwrap_or_default();

        // Optional signature: `(T1, T2, ...) -> R`. When the next
        // token is `(`, parse the parenthesised parameter type list
        // followed by `->` and the return type. The signature is
        // attached to the `UseDecl` so the type checker can validate
        // call-site argument types and assign the declared return
        // type to native calls.
        let signature = if self.at(&TokenKind::LParen) {
            let sig_start = self.expect(&TokenKind::LParen)?;
            let mut params: Vec<TypeExpr> = Vec::new();
            while !self.at(&TokenKind::RParen) {
                params.push(self.parse_type_expr()?);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RParen)?;
            self.expect(&TokenKind::Arrow)?;
            let return_type = self.parse_type_expr()?;
            let sig_end = return_type.span();
            Some(crate::ast::NativeSignature {
                params,
                return_type,
                span: merge_spans(sig_start, sig_end),
            })
        } else {
            None
        };

        let end = self.prev_span();
        Ok(UseDecl {
            path,
            import: ImportItem::Name(import_name),
            signature,
            is_external,
            span: merge_spans(start, end),
        })
    }

    /// `newtype Name = Underlying;`
    ///
    /// Introduces a distinct nominal type that wraps an underlying
    /// type. The bytecode representation is identical to the
    /// underlying type's; the distinction is purely at the type-
    /// checker level. Construction at expression position uses
    /// `Name(expr)`.
    fn parse_newtype_def(&mut self) -> Result<crate::ast::NewtypeDef, ParseError> {
        let start = self.expect(&TokenKind::Newtype)?;
        let (name, _) = self.expect_upper_ident()?;
        self.expect(&TokenKind::Eq)?;
        let underlying = self.parse_type_expr()?;
        // Optional refinement predicate:
        //     newtype Name = Underlying where predicate_name;
        // The predicate must be a function declared in the same
        // program with signature `fn(Underlying) -> Bool`. The
        // type checker enforces the signature; the compiler emits
        // a call followed by a trap at every newtype construction
        // site.
        let refinement = if self.eat(&TokenKind::Where) {
            let (predicate_name, _) = self.expect_lower_ident()?;
            Some(predicate_name)
        } else {
            None
        };
        // Optional saturation contract:
        //     newtype Name = Underlying where pred
        //         with saturate_max = N, saturate_min = M;
        // The values populate the newtype's saturation contract,
        // which the `saturate_max` and `saturate_min` keywords
        // inside a checked-overflow construct resolve to when the
        // construct's expected output type is this newtype. The
        // clause is optional; either field may be omitted; the
        // order is not significant.
        let mut saturate_max: Option<i64> = None;
        let mut saturate_min: Option<i64> = None;
        if self.at_lower("with") {
            self.bump();
            loop {
                let tok = self.tokens[self.pos].clone();
                let kind_label = match &tok.kind {
                    TokenKind::SaturateMax => "saturate_max",
                    TokenKind::SaturateMin => "saturate_min",
                    other => {
                        return Err(ParseError {
                            message: alloc::format!(
                                "expected `saturate_max` or `saturate_min` after `with`, found {:?}",
                                other
                            ),
                            span: tok.span,
                        });
                    }
                };
                self.bump();
                self.expect(&TokenKind::Eq)?;
                let value = self.parse_signed_integer_literal()?;
                if kind_label == "saturate_max" {
                    if saturate_max.is_some() {
                        return Err(ParseError {
                            message: alloc::string::String::from(
                                "duplicate `saturate_max` in newtype contract",
                            ),
                            span: tok.span,
                        });
                    }
                    saturate_max = Some(value);
                } else {
                    if saturate_min.is_some() {
                        return Err(ParseError {
                            message: alloc::string::String::from(
                                "duplicate `saturate_min` in newtype contract",
                            ),
                            span: tok.span,
                        });
                    }
                    saturate_min = Some(value);
                }
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        // Optional trailing semicolon for symmetry with `use` and
        // `let` declarations at the program-level scope.
        self.eat(&TokenKind::Semicolon);
        let end = self.prev_span();
        Ok(crate::ast::NewtypeDef {
            name,
            underlying,
            refinement,
            saturate_max,
            saturate_min,
            span: merge_spans(start, end),
        })
    }

    /// Parse a signed integer literal (admits a leading minus on
    /// a positive literal). Used by the newtype saturation
    /// contract.
    fn parse_signed_integer_literal(&mut self) -> Result<i64, ParseError> {
        let negate = self.eat(&TokenKind::Minus);
        let tok = self.tokens[self.pos].clone();
        match tok.kind {
            TokenKind::IntLit(n) => {
                self.bump();
                if negate { Ok(-n) } else { Ok(n) }
            }
            other => Err(ParseError {
                message: alloc::format!("expected integer literal, found {:?}", other),
                span: tok.span,
            }),
        }
    }

    fn parse_struct_def(&mut self) -> Result<StructDef, ParseError> {
        let start = self.expect(&TokenKind::Struct)?;
        let (name, _) = self.expect_upper_ident()?;
        let type_params = self.parse_optional_type_params()?;
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
            type_params,
            fields,
            span: merge_spans(start, end),
        })
    }

    /// Parse an optional generic type parameter list `<T, U>`.
    ///
    /// Returns an empty vector when no `<` is present. Used by both
    /// function and type definitions.
    fn parse_optional_type_params(&mut self) -> Result<Vec<TypeParam>, ParseError> {
        let mut type_params: Vec<TypeParam> = Vec::new();
        if self.eat(&TokenKind::Lt) {
            if !self.at(&TokenKind::Gt) {
                type_params.push(self.parse_type_param()?);
                while self.eat(&TokenKind::Comma) {
                    if self.at(&TokenKind::Gt) {
                        break;
                    }
                    type_params.push(self.parse_type_param()?);
                }
            }
            self.expect(&TokenKind::Gt)?;
        }
        Ok(type_params)
    }

    fn parse_data_decl(&mut self) -> Result<DataDecl, ParseError> {
        // Optional visibility modifier. `shared data ...` and `data ...`
        // are equivalent; `private data ...` marks the block as
        // host-invisible and arena-resident; `const data ...`
        // declares compile-time constants whose fields carry
        // literal initializers in the source.
        let (visibility, start) = match self.peek() {
            TokenKind::Shared => {
                let s = self.bump();
                self.expect(&TokenKind::Data)?;
                (DataVisibility::Shared, s)
            }
            TokenKind::Private => {
                let s = self.bump();
                self.expect(&TokenKind::Data)?;
                (DataVisibility::Private, s)
            }
            TokenKind::Const => {
                let s = self.bump();
                self.expect(&TokenKind::Data)?;
                (DataVisibility::Const, s)
            }
            _ => {
                let s = self.expect(&TokenKind::Data)?;
                (DataVisibility::Shared, s)
            }
        };
        let (name, _) = self.expect_lower_ident()?;
        self.data_names.insert(name.clone());
        self.expect(&TokenKind::LBrace)?;

        let mut fields = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            let (fname, fspan) = self.expect_lower_ident()?;
            self.expect(&TokenKind::Colon)?;
            let ftype = self.parse_type_expr()?;
            // Optional initializer: `= literal`. Required on
            // `const data` fields; rejected at the type-check
            // stage on `shared`/`private` data fields, but the
            // parser accepts the syntactic form uniformly and
            // defers the rule to the next pass for a better
            // error message.
            let mut end = ftype.span();
            let initializer = if self.eat(&TokenKind::Eq) {
                let init = self.parse_const_initializer()?;
                end = merge_spans(end, init.1);
                Some(init.0)
            } else {
                None
            };
            fields.push(DataFieldDecl {
                name: fname,
                type_expr: ftype,
                initializer,
                span: merge_spans(fspan, end),
            });
            self.eat(&TokenKind::Comma);
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Ok(DataDecl {
            name,
            fields,
            visibility,
            span: merge_spans(start, end),
        })
    }

    /// Parse a compile-time initializer following `=` in a
    /// `const data` field. Accepts scalar literals (integer,
    /// float, boolean, string, unit, plus optional leading
    /// minus on numerics) and composite forms `(init, init, ...)`
    /// for tuples and `[init, init, ...]` for arrays. Composites
    /// nest. Struct and enum initializers are reserved for a
    /// future iteration.
    fn parse_const_initializer(&mut self) -> Result<(ConstInitializer, Span), ParseError> {
        let start = self.peek_span();
        // Struct or enum literal: leading UpperIdent.
        if let TokenKind::UpperIdent(_) = self.peek() {
            let (name, name_span) = self.expect_upper_ident()?;
            // Enum variant: `Enum::Variant` or `Enum::Variant(args)`.
            if self.eat(&TokenKind::ColonColon) {
                let (variant, var_span) = self.expect_upper_ident()?;
                let mut args: Vec<ConstInitializer> = Vec::new();
                let mut end = var_span;
                if self.eat(&TokenKind::LParen) {
                    if !self.at(&TokenKind::RParen) {
                        let (first, _) = self.parse_const_initializer()?;
                        args.push(first);
                        while self.eat(&TokenKind::Comma) {
                            if self.at(&TokenKind::RParen) {
                                break;
                            }
                            let (next, _) = self.parse_const_initializer()?;
                            args.push(next);
                        }
                    }
                    end = self.expect(&TokenKind::RParen)?;
                }
                return Ok((
                    ConstInitializer::Enum {
                        enum_name: name,
                        variant,
                        args,
                    },
                    merge_spans(name_span, end),
                ));
            }
            // Struct literal: `Name { field: init, ... }`.
            self.expect(&TokenKind::LBrace)?;
            let mut fields: Vec<(String, ConstInitializer)> = Vec::new();
            while !self.at(&TokenKind::RBrace) {
                let (fname, _) = self.expect_lower_ident()?;
                self.expect(&TokenKind::Colon)?;
                let (finit, _) = self.parse_const_initializer()?;
                fields.push((fname, finit));
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            let end = self.expect(&TokenKind::RBrace)?;
            return Ok((
                ConstInitializer::Struct { name, fields },
                merge_spans(name_span, end),
            ));
        }
        // Array literal `[init, init, ...]`.
        if self.at(&TokenKind::LBracket) {
            self.bump();
            let mut elements: Vec<ConstInitializer> = Vec::new();
            if !self.at(&TokenKind::RBracket) {
                let (first, _) = self.parse_const_initializer()?;
                elements.push(first);
                while self.eat(&TokenKind::Comma) {
                    if self.at(&TokenKind::RBracket) {
                        break;
                    }
                    let (next, _) = self.parse_const_initializer()?;
                    elements.push(next);
                }
            }
            let end = self.expect(&TokenKind::RBracket)?;
            return Ok((ConstInitializer::Array(elements), merge_spans(start, end)));
        }
        // Tuple literal `(init, init, ...)` or unit literal `()`.
        // The scalar fast-path also accepts `()`; detect tuple by
        // peeking for a comma inside.
        if self.at(&TokenKind::LParen) {
            // Lookahead: distinguish unit `()` from a tuple. Save
            // the position and try to parse a tuple form; if we
            // see RParen immediately, treat as unit literal.
            let lparen_span = self.peek_span();
            self.bump();
            if self.at(&TokenKind::RParen) {
                let end = self.expect(&TokenKind::RParen)?;
                return Ok((
                    ConstInitializer::Scalar(Literal::Unit),
                    merge_spans(lparen_span, end),
                ));
            }
            let mut elements: Vec<ConstInitializer> = Vec::new();
            let (first, _) = self.parse_const_initializer()?;
            elements.push(first);
            // A single element followed by `)` is a parenthesised
            // scalar; conventionally treat as Scalar. With a
            // trailing comma the user signalled a tuple.
            let saw_comma = self.eat(&TokenKind::Comma);
            if saw_comma {
                while !self.at(&TokenKind::RParen) {
                    let (next, _) = self.parse_const_initializer()?;
                    elements.push(next);
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            let end = self.expect(&TokenKind::RParen)?;
            if !saw_comma {
                // Single parenthesised initializer is its inner
                // form, not a 1-tuple. Matches Rust's `(x)`
                // semantics.
                return Ok((
                    elements.into_iter().next().expect("single element present"),
                    merge_spans(lparen_span, end),
                ));
            }
            return Ok((
                ConstInitializer::Tuple(elements),
                merge_spans(lparen_span, end),
            ));
        }
        // Scalar literal.
        let (lit, span) = self.parse_scalar_literal()?;
        Ok((ConstInitializer::Scalar(lit), span))
    }

    /// Parse a scalar literal value usable as a const initializer.
    /// Accepts integer, float, boolean, string, and unit literals
    /// plus a leading unary minus on numeric literals.
    fn parse_scalar_literal(&mut self) -> Result<(Literal, Span), ParseError> {
        let start = self.peek_span();
        // Optional leading `-` on numeric literals.
        let negate = self.at(&TokenKind::Minus);
        if negate {
            self.bump();
        }
        let tok = self.tokens[self.pos].clone();
        let lit = match tok.kind {
            TokenKind::IntLit(n) => {
                self.pos += 1;
                let value = if negate { n.wrapping_neg() } else { n };
                Literal::Int(value)
            }
            TokenKind::FloatLit(f) => {
                self.pos += 1;
                let value = if negate { -f } else { f };
                Literal::Float(value)
            }
            TokenKind::ByteLit(b) => {
                self.pos += 1;
                if negate {
                    return Err(ParseError {
                        message: alloc::string::String::from(
                            "a `Byte` literal cannot be negated; `Byte` is unsigned",
                        ),
                        span: tok.span,
                    });
                }
                Literal::Byte(b)
            }
            TokenKind::FixedLit(raw, frac) => {
                self.pos += 1;
                let raw = if negate { raw.wrapping_neg() } else { raw };
                Literal::Fixed {
                    raw,
                    frac_bits: frac,
                }
            }
            TokenKind::True if !negate => {
                self.pos += 1;
                Literal::Bool(true)
            }
            TokenKind::False if !negate => {
                self.pos += 1;
                Literal::Bool(false)
            }
            TokenKind::StringLit(s) if !negate => {
                self.pos += 1;
                Literal::String(s)
            }
            TokenKind::LParen if !negate => {
                // Unit literal `()`.
                self.bump();
                let end = self.expect(&TokenKind::RParen)?;
                return Ok((Literal::Unit, merge_spans(start, end)));
            }
            _ => {
                return Err(ParseError {
                    message: alloc::format!(
                        "expected literal initializer (integer, float, true, false, string, or `()`), got {:?}",
                        tok.kind
                    ),
                    span: tok.span,
                });
            }
        };
        Ok((lit, merge_spans(start, tok.span)))
    }

    fn parse_enum_def(&mut self) -> Result<EnumDef, ParseError> {
        let start = self.expect(&TokenKind::Enum)?;
        let (name, _) = self.expect_upper_ident()?;
        let type_params = self.parse_optional_type_params()?;
        self.expect(&TokenKind::LBrace)?;

        let mut variants = Vec::new();
        // Auto-assignment runs from a counter that increments
        // after each variant. The first variant defaults to 0
        // (matching Rust's enum-without-discriminant convention).
        // An explicit `= N` clause resets the counter to N+1
        // for the next implicit variant.
        let mut next_implicit: i64 = 0;
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
            // Optional `= N` clause. Integer literal with an
            // optional leading unary minus. Expression-position
            // arithmetic is still not admissible.
            let (explicit_discriminant, discriminant_value) = if self.eat(&TokenKind::Eq) {
                let neg_span = if self.at(&TokenKind::Minus) {
                    Some(self.bump())
                } else {
                    None
                };
                let tok = self.tokens[self.pos].clone();
                match tok.kind {
                    TokenKind::IntLit(n) => {
                        self.pos += 1;
                        // `wrapping_neg` keeps `i64::MIN` stable;
                        // a literal `-9223372036854775808` round-
                        // trips correctly even though the
                        // positive form does not lex.
                        let value = if neg_span.is_some() {
                            n.wrapping_neg()
                        } else {
                            n
                        };
                        let span_start = neg_span.unwrap_or(tok.span);
                        end = merge_spans(end, merge_spans(span_start, tok.span));
                        (Some(value), value)
                    }
                    other => {
                        return Err(ParseError {
                            message: format!(
                                "expected integer literal after `=` in enum variant, got {:?}",
                                other
                            ),
                            span: tok.span,
                        });
                    }
                }
            } else {
                (None, next_implicit)
            };
            next_implicit = discriminant_value.wrapping_add(1);
            variants.push(VariantDecl {
                name: vname,
                fields,
                explicit_discriminant,
                discriminant_value,
                span: merge_spans(vspan, end),
            });
            self.eat(&TokenKind::Comma);
        }

        // Reject duplicate discriminant values within a single
        // enum. Implicit values can collide with explicit ones
        // (e.g., `A = 1, B` — A and B both want 1), and explicit
        // values can collide with each other (`A = 1, B = 1`).
        // The check is quadratic in the variant count; for
        // realistic enum sizes the cost is negligible.
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                if variants[i].discriminant_value == variants[j].discriminant_value {
                    return Err(ParseError {
                        message: format!(
                            "enum `{}`: variant `{}` discriminant {} duplicates variant `{}`",
                            name,
                            variants[j].name,
                            variants[j].discriminant_value,
                            variants[i].name
                        ),
                        span: variants[j].span,
                    });
                }
            }
        }

        let end = self.expect(&TokenKind::RBrace)?;
        Ok(EnumDef {
            name,
            type_params,
            variants,
            span: merge_spans(start, end),
        })
    }

    // --- Function parsing ---

    fn parse_function_def(&mut self) -> Result<FunctionDef, ParseError> {
        let start = self.peek_span();

        // Optional `ephemeral` and `signed` modifiers. Both are
        // entry-only assertions; either or both may precede the
        // function category keyword in either order. The type
        // checker rejects them on non-entry functions; the
        // verifier rejects the program if the `ephemeral` proof
        // fails. The `signed` modifier sets
        // `FLAG_REQUIRES_SIGNATURE` on the module header so the
        // load-time runtime refuses to admit the bytecode without
        // a verified signature.
        let mut ephemeral = false;
        let mut signed = false;
        loop {
            if !ephemeral && self.eat(&TokenKind::Ephemeral) {
                ephemeral = true;
                continue;
            }
            if !signed && self.eat(&TokenKind::Signed) {
                signed = true;
                continue;
            }
            break;
        }

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
            _ => {
                let expected_kw = match (ephemeral, signed) {
                    (true, true) => "expected 'fn', 'yield', or 'loop' after 'ephemeral'/'signed'",
                    (true, false) => "expected 'fn', 'yield', or 'loop' after 'ephemeral'",
                    (false, true) => "expected 'fn', 'yield', or 'loop' after 'signed'",
                    (false, false) => "expected 'fn', 'yield', or 'loop'",
                };
                return Err(self.error(expected_kw));
            }
        };

        let (name, _) = self.expect_lower_ident()?;

        // Optional generic type parameter list: `fn name<T, U>(...)`.
        // Each parameter is an upper-case identifier such as `T`. The
        // empty list is permitted but conventionally elided. Bounds
        // and trait constraints are reserved for future work and are
        // not parsed here.
        let mut type_params: Vec<TypeParam> = Vec::new();
        if self.eat(&TokenKind::Lt) {
            if !self.at(&TokenKind::Gt) {
                type_params.push(self.parse_type_param()?);
                while self.eat(&TokenKind::Comma) {
                    if self.at(&TokenKind::Gt) {
                        break;
                    }
                    type_params.push(self.parse_type_param()?);
                }
            }
            self.expect(&TokenKind::Gt)?;
        }

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
            type_params,
            params,
            return_type,
            guard,
            body,
            ephemeral,
            signed,
            span: merge_spans(start, end),
        })
    }

    fn parse_type_param(&mut self) -> Result<TypeParam, ParseError> {
        let (name, span) = self.expect_upper_ident()?;
        let mut bounds: Vec<alloc::string::String> = Vec::new();
        if self.eat(&TokenKind::Colon) {
            // First bound is required after the colon.
            let (b, _) = self.expect_upper_ident()?;
            bounds.push(b);
            // Additional bounds via `+ Trait`.
            while self.at(&TokenKind::Plus) {
                self.bump();
                let (b, _) = self.expect_upper_ident()?;
                bounds.push(b);
            }
        }
        Ok(TypeParam { name, bounds, span })
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

            // Contextual `assert` statement. `assert` is not a reserved
            // keyword; a lowercase `assert` at statement position that
            // is not followed by `(` is the assertion form. `assert(x)`
            // remains a call to a user function named `assert`.
            if matches!(self.peek(), TokenKind::LowerIdent(n) if n == "assert")
                && !matches!(self.peek_ahead(1), TokenKind::LParen)
            {
                stmts.push(self.parse_assert_stmt()?);
                continue;
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
                    if self.eat(&TokenKind::Eq) {
                        // Data field assignment: data_name.field = expr;
                        if let Expr::FieldAccess {
                            object,
                            field,
                            span: fa_span,
                        } = &expr
                            && let Expr::Ident { name, .. } = object.as_ref()
                            && self.data_names.contains(name)
                        {
                            let value = self.parse_expr()?;
                            let end = self.expect(&TokenKind::Semicolon)?;
                            stmts.push(Stmt::DataFieldAssign {
                                data_name: name.clone(),
                                field: field.clone(),
                                value,
                                span: merge_spans(*fa_span, end),
                            });
                            continue;
                        }
                        // Indexed data field assignment:
                        // `data_name.field[i]... = expr;`.
                        if let Some((data_name, field_name, indices, lhs_span)) =
                            data_indexed_lhs(&expr, &self.data_names)
                        {
                            let value = self.parse_expr()?;
                            let end = self.expect(&TokenKind::Semicolon)?;
                            stmts.push(Stmt::DataFieldIndexAssign {
                                data_name,
                                field: field_name,
                                indices,
                                value,
                                span: merge_spans(lhs_span, end),
                            });
                            continue;
                        }
                        return Err(
                            self.error("assignment is only supported for data block fields")
                        );
                    } else if self.eat(&TokenKind::Semicolon) {
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

    /// Parse a contextual `assert` statement:
    /// `assert <expr> [, "<message>"] ;`. The leading `assert`
    /// identifier has already been confirmed by the caller.
    fn parse_assert_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.bump(); // consume the `assert` identifier
        let cond = self.parse_expr()?;
        let message = if self.eat(&TokenKind::Comma) {
            if let TokenKind::StringLit(s) = self.peek().clone() {
                self.bump();
                Some(s)
            } else {
                return Err(self.error("expected a string-literal message after `,` in an assert"));
            }
        } else {
            None
        };
        let end = self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Assert {
            cond,
            message,
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
        self.enter_depth()?;
        let inner = self.parse_pipeline_expr();
        self.leave_depth();
        let mut inner = inner?;
        // Attach an overflow-checked arm block when one is
        // syntactically present. The construct is recognised by
        // an opening `{` followed by one of the arm keywords
        // (`overflow`, `underflow`, or the lowercase identifier
        // `ok`). Other `{`s (struct literals, block expressions
        // in if/match/let positions) are handled by their
        // respective parsers and do not reach this point.
        if matches!(self.peek(), TokenKind::LBrace) && self.peek_ahead_is_checked_arm_keyword() {
            inner = self.parse_checked_arms_after(inner)?;
        }
        Ok(inner)
    }

    fn peek_ahead_is_checked_arm_keyword(&self) -> bool {
        matches!(
            self.peek_ahead(1),
            TokenKind::Overflow | TokenKind::Underflow
        ) || matches!(self.peek_ahead(1), TokenKind::LowerIdent(s) if s == "ok" || s == "invalid_index" || s == "invalid_newtype" || s == "payload_discriminant" || s == "invalid_discriminant" || s == "error")
    }

    fn parse_checked_arms_after(&mut self, op_expr: Expr) -> Result<Expr, ParseError> {
        let start_span = op_expr.span();
        self.expect(&TokenKind::LBrace)?;
        let mut arms: alloc::vec::Vec<crate::ast::CheckedArm> = alloc::vec::Vec::new();
        while !self.at(&TokenKind::RBrace) {
            let arm_start = self.peek_span();
            let kind = self.parse_checked_arm_kind()?;
            let guard = if self.eat(&TokenKind::When) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect(&TokenKind::FatArrow)?;
            let body = self.parse_expr()?;
            let arm_end = body.span();
            arms.push(crate::ast::CheckedArm {
                kind,
                guard,
                body,
                span: merge_spans(arm_start, arm_end),
            });
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        let end = self.expect(&TokenKind::RBrace)?;
        Ok(Expr::Checked {
            op_expr: alloc::boxed::Box::new(op_expr),
            arms,
            span: merge_spans(start_span, end),
        })
    }

    /// Parse a single arm pattern position for a checked construct.
    /// Accepts the wildcard `_`, a bare lower-case identifier
    /// (binds), or an integer literal with optional leading `-`.
    fn parse_checked_arm_pattern(&mut self) -> Result<crate::ast::Pattern, ParseError> {
        let tok = self.tokens[self.pos].clone();
        match &tok.kind {
            TokenKind::Underscore => {
                self.bump();
                Ok(crate::ast::Pattern::Wildcard(tok.span))
            }
            TokenKind::LowerIdent(name) => {
                self.bump();
                Ok(crate::ast::Pattern::Variable(name.clone(), tok.span))
            }
            // An upper-case identifier names an enum variant in the
            // discriminant-to-enum construct's `ok` and
            // `payload_discriminant` arms (B35 P6). It is stored as a
            // `Variable`; the type checker distinguishes a variant
            // name from a binder by the leading-character case.
            TokenKind::UpperIdent(name) => {
                self.bump();
                Ok(crate::ast::Pattern::Variable(name.clone(), tok.span))
            }
            TokenKind::IntLit(_) | TokenKind::Minus => {
                let v = self.parse_signed_integer_literal()?;
                Ok(crate::ast::Pattern::Literal(
                    crate::ast::Literal::Int(v),
                    tok.span,
                ))
            }
            other => Err(ParseError {
                message: alloc::format!(
                    "expected `_`, identifier, or integer literal in checked-arm pattern, found {:?}",
                    other
                ),
                span: tok.span,
            }),
        }
    }

    /// Parse the optional second pattern of an `overflow`/`underflow`
    /// arm. `Word` operands use the two-pattern `(h, l)` form; `Byte`
    /// operands use the single-pattern `(w)` form. The type checker
    /// enforces the arity against the operand type.
    fn parse_optional_second_checked_pattern(
        &mut self,
    ) -> Result<Option<crate::ast::Pattern>, ParseError> {
        if self.at(&TokenKind::Comma) {
            self.bump();
            Ok(Some(self.parse_checked_arm_pattern()?))
        } else {
            Ok(None)
        }
    }

    fn parse_checked_arm_kind(&mut self) -> Result<crate::ast::CheckedArmKind, ParseError> {
        match self.peek().clone() {
            TokenKind::Overflow => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let h = self.parse_checked_arm_pattern()?;
                let l = self.parse_optional_second_checked_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::Overflow(h, l))
            }
            TokenKind::Underflow => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let h = self.parse_checked_arm_pattern()?;
                let l = self.parse_optional_second_checked_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::Underflow(h, l))
            }
            TokenKind::LowerIdent(name) if name == "ok" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::Ok(p))
            }
            TokenKind::LowerIdent(name) if name == "zero_divisor" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::ZeroDivisor(p))
            }
            TokenKind::LowerIdent(name) if name == "nan" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::Nan(p))
            }
            TokenKind::LowerIdent(name) if name == "invalid_index" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::InvalidIndex(p))
            }
            TokenKind::LowerIdent(name) if name == "invalid_newtype" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::InvalidNewtype(p))
            }
            TokenKind::LowerIdent(name) if name == "payload_discriminant" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::PayloadDiscriminant(p))
            }
            TokenKind::LowerIdent(name) if name == "invalid_discriminant" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::InvalidDiscriminant(p))
            }
            TokenKind::LowerIdent(name) if name == "error" => {
                self.bump();
                self.expect(&TokenKind::LParen)?;
                let p = self.parse_checked_arm_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(crate::ast::CheckedArmKind::Error(p))
            }
            other => Err(ParseError {
                message: alloc::format!(
                    "expected `ok(pattern)`, `overflow(...)`, `underflow(...)`, `zero_divisor(numerator)`, `nan(result)`, `invalid_index(index)`, `invalid_newtype(value)`, `payload_discriminant(Variant)`, `invalid_discriminant(raw)`, or `error(code)`, found {:?}",
                    other
                ),
                span: self.peek_span(),
            }),
        }
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
                    TokenKind::LowerIdent(name) => {
                        self.pos += 1;
                        // Distinguish field access from method call by
                        // looking ahead for `(`. `expr.name(args)` is
                        // a method call; `expr.name` without paren is
                        // a field access.
                        if self.at(&TokenKind::LParen) {
                            self.pos += 1;
                            let args = self.parse_arg_list()?;
                            let end = self.expect(&TokenKind::RParen)?;
                            let span = merge_spans(expr.span(), end);
                            expr = Expr::MethodCall {
                                receiver: Box::new(expr),
                                method: name,
                                args,
                                span,
                            };
                        } else {
                            let span = merge_spans(expr.span(), tok.span);
                            expr = Expr::FieldAccess {
                                object: Box::new(expr),
                                field: name,
                                span,
                            };
                        }
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

        // Saturation literals. Used inside overflow-checked arms
        // to denote the type's max or min value. The compiler
        // resolves the constant based on the surrounding
        // construct's expected type; V0.2 supports Word only.
        if matches!(tok.kind, TokenKind::SaturateMax) {
            self.bump();
            return Ok(Expr::SaturateMax { span: tok.span });
        }
        if matches!(tok.kind, TokenKind::SaturateMin) {
            self.bump();
            return Ok(Expr::SaturateMin { span: tok.span });
        }

        // Information-flow operators. `classify` and
        // `declassify` are context-sensitive: a lowercase
        // identifier with that spelling at the start of an
        // expression, followed by something other than `(`, is
        // the operator form. A `LowerIdent("classify")` followed
        // by `(` is a function call (the user may legitimately
        // name a function `classify`).
        if let TokenKind::LowerIdent(name) = &tok.kind
            && (name == "classify" || name == "declassify")
            && !matches!(self.peek_ahead(1), TokenKind::LParen)
        {
            let is_classify = name == "classify";
            self.bump();
            let value = self.parse_postfix_expr()?;
            self.expect(&TokenKind::At)?;
            let spec = self.parse_label_spec()?;
            let labels = match spec {
                LabelSpec::Positive(labels) => labels,
                LabelSpec::Negative(_) => {
                    return Err(self.error(
                        if is_classify {
                            "negative information-flow labels are not admitted in `classify` expressions; classify operates on positive labels only"
                        } else {
                            "negative information-flow labels are not admitted in `declassify` expressions; declassify operates on positive labels only"
                        },
                    ));
                }
            };
            let span = merge_spans(tok.span, self.prev_span());
            return if is_classify {
                Ok(Expr::Classify {
                    value: alloc::boxed::Box::new(value),
                    labels,
                    span,
                })
            } else {
                Ok(Expr::Declassify {
                    value: alloc::boxed::Box::new(value),
                    labels,
                    span,
                })
            };
        }

        match tok.kind {
            // Closure literal: `|args| body` or `|args| -> ret { body }`.
            // Bar (`|`) introduces the parameter list. The body can
            // be a brace block or a single expression.
            TokenKind::Bar => {
                self.pos += 1;
                let mut params: Vec<Param> = Vec::new();
                if !self.at(&TokenKind::Bar) {
                    params.push(self.parse_param()?);
                    while self.eat(&TokenKind::Comma) {
                        if self.at(&TokenKind::Bar) {
                            break;
                        }
                        params.push(self.parse_param()?);
                    }
                }
                self.expect(&TokenKind::Bar)?;
                let return_type = if self.eat(&TokenKind::Arrow) {
                    Some(self.parse_type_expr()?)
                } else {
                    None
                };
                let body = if self.at(&TokenKind::LBrace) {
                    self.parse_block()?
                } else {
                    // Single-expression body wraps into a block whose
                    // tail expression is the parsed expression. The
                    // span is the expression's span.
                    let e = self.parse_expr()?;
                    let span = e.span();
                    Block {
                        stmts: Vec::new(),
                        tail_expr: Some(Box::new(e)),
                        span,
                    }
                };
                let end = body.span;
                Ok(Expr::Closure {
                    params,
                    return_type,
                    body,
                    span: merge_spans(tok.span, end),
                })
            }
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
            TokenKind::ByteLit(v) => {
                self.pos += 1;
                Ok(Expr::Literal {
                    value: Literal::Byte(v),
                    span: tok.span,
                })
            }
            TokenKind::FixedLit(raw, frac) => {
                self.pos += 1;
                Ok(Expr::Literal {
                    value: Literal::Fixed {
                        raw,
                        frac_bits: frac,
                    },
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
                } else if self.at(&TokenKind::LParen) {
                    // Newtype construction: `Name(expr)`. The parser
                    // emits a `Call` expression with the type name as
                    // the function. The type checker resolves the
                    // name to a newtype constructor, validates the
                    // argument against the underlying type, and tags
                    // the resulting expression with the newtype's
                    // nominal type. If the name does not resolve to a
                    // declared newtype, the type checker reports an
                    // undefined-function error.
                    self.pos += 1;
                    let args = self.parse_arg_list()?;
                    let end = self.expect(&TokenKind::RParen)?;
                    Ok(Expr::Call {
                        name,
                        args,
                        span: merge_spans(name_span, end),
                    })
                } else {
                    Err(ParseError {
                        message: String::from(
                            "expected '::', '{', or '(' after type name in expression",
                        ),
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
                    let guard = if self.eat(&TokenKind::When) {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    self.expect(&TokenKind::FatArrow)?;
                    let expr = self.parse_expr()?;
                    let arm_span = merge_spans(pattern.span(), expr.span());
                    arms.push(MatchArm {
                        pattern,
                        guard,
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

            // Parenthesized expression or tuple literal.
            TokenKind::LParen => {
                self.pos += 1;
                if self.eat(&TokenKind::RParen) {
                    // Unit literal.
                    return Ok(Expr::Literal {
                        value: Literal::Unit,
                        span: merge_spans(tok.span, self.prev_span()),
                    });
                }
                let first = self.parse_expr()?;
                if self.eat(&TokenKind::Comma) {
                    // Tuple literal: (expr, expr, ...)
                    let mut elements = vec![first];
                    if !self.at(&TokenKind::RParen) {
                        elements.push(self.parse_expr()?);
                        while self.eat(&TokenKind::Comma) {
                            if self.at(&TokenKind::RParen) {
                                break;
                            }
                            elements.push(self.parse_expr()?);
                        }
                    }
                    let end = self.expect(&TokenKind::RParen)?;
                    Ok(Expr::TupleLiteral {
                        elements,
                        span: merge_spans(tok.span, end),
                    })
                } else {
                    // Grouped expression.
                    self.expect(&TokenKind::RParen)?;
                    Ok(first)
                }
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
        self.enter_depth()?;
        let result = self.parse_type_expr_inner();
        self.leave_depth();
        let inner = result?;
        // Attach an information-flow label set when one is
        // present. Surface forms:
        //   T@Label             — single positive label.
        //   T@!Label            — single negative label.
        //   T@{L1, L2, ...}     — multiple positive labels.
        //   T@{!N1, !N2, ...}   — multiple negative labels.
        //   T@{L1, !N1}         — mixed; rejected at parse time.
        // Negative labels are admissible only at parameter and
        // return type positions; the type checker enforces that
        // restriction on the resulting AST node.
        if self.eat(&TokenKind::At) {
            let spec = self.parse_label_spec()?;
            let span = merge_spans(inner.span(), self.prev_span());
            match spec {
                LabelSpec::Positive(labels) => Ok(TypeExpr::Labelled(
                    alloc::boxed::Box::new(inner),
                    labels,
                    span,
                )),
                LabelSpec::Negative(labels) => Ok(TypeExpr::NegativeLabelled(
                    alloc::boxed::Box::new(inner),
                    labels,
                    span,
                )),
            }
        } else {
            Ok(inner)
        }
    }

    fn parse_label_spec(&mut self) -> Result<LabelSpec, ParseError> {
        if self.eat(&TokenKind::LBrace) {
            let mut positives: Vec<String> = Vec::new();
            let mut negatives: Vec<String> = Vec::new();
            while !self.at(&TokenKind::RBrace) {
                if self.eat(&TokenKind::Bang) {
                    let (name, _) = self.expect_upper_ident()?;
                    negatives.push(name);
                } else {
                    let (name, _) = self.expect_upper_ident()?;
                    positives.push(name);
                }
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace)?;
            if !positives.is_empty() && !negatives.is_empty() {
                return Err(self.error(
                    "mixed positive and negative information-flow labels in the same set are not admitted in V0.2.0; remove either the positives or the negatives",
                ));
            }
            if !negatives.is_empty() {
                Ok(LabelSpec::Negative(negatives))
            } else {
                Ok(LabelSpec::Positive(positives))
            }
        } else if self.eat(&TokenKind::Bang) {
            let (name, _) = self.expect_upper_ident()?;
            Ok(LabelSpec::Negative(alloc::vec![name]))
        } else {
            let (name, _) = self.expect_upper_ident()?;
            Ok(LabelSpec::Positive(alloc::vec![name]))
        }
    }

    fn parse_type_expr_inner(&mut self) -> Result<TypeExpr, ParseError> {
        let span = self.peek_span();

        // Check for the boolean primitive (the only lowercase-named
        // primitive type in V0.2). Numeric and text primitives are
        // uppercase (Byte/Word/Fixed/Float/Text) and are matched
        // below through `at_upper`.
        if self.at_lower("bool") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::Bool, span));
        }

        // Canonical V0.2 numeric primitives. `Byte` is an 8-bit
        // unsigned integer; `Word` is the target word size (64-bit
        // signed on the host runtime); `Float` is the target
        // floating-point width.
        if self.at_upper("Byte") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::Byte, span));
        }
        if self.at_upper("Word") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::Word, span));
        }
        if self.at_upper("Fixed") {
            self.pos += 1;
            // Optional `<N>` argument pinning the fraction-bit count.
            // Without the argument the default form `PrimType::Fixed(None)`
            // resolves to the target-scaled default at type check.
            if self.eat(&TokenKind::Lt) {
                let tok = self.tokens[self.pos].clone();
                let frac_bits = match tok.kind {
                    TokenKind::IntLit(n) => {
                        if !(0..=62).contains(&n) {
                            return Err(
                                self.error("Fixed<N> fraction bits must be in the range [0, 62]")
                            );
                        }
                        self.pos += 1;
                        n as u8
                    }
                    _ => {
                        return Err(
                            self.error("expected integer literal for Fixed<N> fraction bits")
                        );
                    }
                };
                let end = self.expect(&TokenKind::Gt)?;
                return Ok(TypeExpr::Prim(
                    PrimType::Fixed(Some(frac_bits)),
                    merge_spans(span, end),
                ));
            }
            return Ok(TypeExpr::Prim(PrimType::Fixed(None), span));
        }
        if self.at_upper("Float") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::Float, span));
        }

        // Check for Text (upper ident). Keleusma's surface text type
        // is named `Text` to avoid confusion with Rust's `String`.
        // Gated on the `text` cargo feature; when disabled `Text`
        // falls through to the named-type path, where it will be
        // rejected as an unknown opaque type by the type checker.
        if self.at_upper("Text") {
            self.pos += 1;
            return Ok(TypeExpr::Prim(PrimType::Text, span));
        }

        // Option<T>.
        if self.at_upper("Option") {
            self.pos += 1;
            self.expect(&TokenKind::Lt)?;
            let inner = self.parse_type_expr()?;
            let end = self.expect(&TokenKind::Gt)?;
            return Ok(TypeExpr::Option(Box::new(inner), merge_spans(span, end)));
        }

        // Named type (other upper ident) with optional generic
        // arguments. `Cell` is a non-generic reference; `Cell<T>` is
        // a generic instantiation.
        if self.at(&TokenKind::UpperIdent(String::new())) {
            let (name, name_span) = self.expect_upper_ident()?;
            let mut args: Vec<TypeExpr> = Vec::new();
            let mut end = name_span;
            if self.eat(&TokenKind::Lt) {
                if !self.at(&TokenKind::Gt) {
                    args.push(self.parse_type_expr()?);
                    while self.eat(&TokenKind::Comma) {
                        if self.at(&TokenKind::Gt) {
                            break;
                        }
                        args.push(self.parse_type_expr()?);
                    }
                }
                end = self.expect(&TokenKind::Gt)?;
            }
            return Ok(TypeExpr::Named(name, args, merge_spans(name_span, end)));
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
        self.enter_depth()?;
        let result = self.parse_pattern_inner();
        self.leave_depth();
        result
    }

    fn parse_pattern_inner(&mut self) -> Result<Pattern, ParseError> {
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
            TokenKind::ByteLit(v) => {
                self.pos += 1;
                Ok(Pattern::Literal(Literal::Byte(v), tok.span))
            }
            TokenKind::FixedLit(raw, frac) => {
                self.pos += 1;
                Ok(Pattern::Literal(
                    Literal::Fixed {
                        raw,
                        frac_bits: frac,
                    },
                    tok.span,
                ))
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
        let wrapped = alloc::format!("fn test() -> Word {{ {} }}", src);
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
    fn deeply_nested_parens_reject_with_typed_error_not_stack_overflow() {
        let mut src = alloc::string::String::from("fn main() -> Word { ");
        for _ in 0..5000 {
            src.push('(');
        }
        src.push('1');
        for _ in 0..5000 {
            src.push(')');
        }
        src.push_str(" }");
        let err = parse_str(&src).expect_err("parser should reject");
        assert!(
            err.message.contains("recursion depth"),
            "expected depth error, got: {}",
            err.message
        );
    }

    #[test]
    fn modest_nesting_within_limit_parses() {
        // 16 layers of parens is well within MAX_PARSE_DEPTH=32.
        let mut src = alloc::string::String::from("fn main() -> Word { ");
        for _ in 0..16 {
            src.push('(');
        }
        src.push_str("42");
        for _ in 0..16 {
            src.push(')');
        }
        src.push_str(" }");
        let prog = parse_str(&src).expect("parser should accept");
        assert_eq!(prog.functions.len(), 1);
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
    #[cfg(feature = "floats")]
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
    #[cfg(feature = "floats")]
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
    #[cfg(feature = "floats")]
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
        let expr = parse_expr_str("x as Float").unwrap();
        match expr {
            Expr::Cast { ref target, .. } => {
                assert!(matches!(target, TypeExpr::Prim(PrimType::Float, _)));
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
        let src = "fn test() -> Word { if x > 0 { 1 } else { 0 } }";
        let program = parse_str(src).unwrap();
        let tail = program.functions[0].body.tail_expr.as_ref().unwrap();
        assert!(matches!(**tail, Expr::If { ref else_block, .. } if else_block.is_some()));
    }

    #[test]
    fn parse_match_expr() {
        let src = r#"
            fn test() -> Word {
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
        let src = "fn test() -> Word { let x: Word = 42; x }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions[0].body.stmts.len(), 1);
        assert!(matches!(&program.functions[0].body.stmts[0], Stmt::Let(_)));
    }

    #[test]
    fn parse_for_range() {
        let src = "fn test() -> Word { for i in 0..8 { foo(i); } 0 }";
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
        let src = "fn test() -> Word { for n in notes { play(n); } 0 }";
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
        let src = "fn test() -> Word { for i in 0..8 { break; } 0 }";
        let program = parse_str(src).unwrap();
        let for_stmt = match &program.functions[0].body.stmts[0] {
            Stmt::For(f) => f,
            _ => panic!("expected For"),
        };
        assert!(matches!(&for_stmt.body.stmts[0], Stmt::Break(_)));
    }

    #[test]
    fn parse_fn_definition() {
        let src = "fn add(a: Word, b: Word) -> Word { a + b }";
        let program = parse_str(src).unwrap();
        let f = &program.functions[0];
        assert_eq!(f.category, FunctionCategory::Fn);
        assert_eq!(f.name, "add");
        assert_eq!(f.type_params.len(), 0);
        assert_eq!(f.params.len(), 2);
        assert!(f.body.tail_expr.is_some());
    }

    #[test]
    fn parse_fn_with_single_type_param() {
        let src = "fn id<T>(x: T) -> T { x }";
        let program = parse_str(src).unwrap();
        let f = &program.functions[0];
        assert_eq!(f.name, "id");
        assert_eq!(f.type_params.len(), 1);
        assert_eq!(f.type_params[0].name, "T");
    }

    #[test]
    fn parse_fn_with_multiple_type_params() {
        let src = "fn pair<T, U>(a: T, b: U) -> T { a }";
        let program = parse_str(src).unwrap();
        let f = &program.functions[0];
        assert_eq!(f.type_params.len(), 2);
        assert_eq!(f.type_params[0].name, "T");
        assert_eq!(f.type_params[1].name, "U");
    }

    #[test]
    fn parse_fn_with_trailing_comma_in_type_params() {
        let src = "fn id<T,>(x: T) -> T { x }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions[0].type_params.len(), 1);
    }

    #[test]
    fn parse_closure_no_params_no_body() {
        // `|| 42` parses as a nullary closure whose tail expression
        // is a literal.
        let src = "fn main() -> Word { let f = || 42; 0 }";
        let program = parse_str(src).unwrap();
        let body = &program.functions[0].body;
        match &body.stmts[0] {
            Stmt::Let(l) => match &l.value {
                Expr::Closure { params, .. } => assert!(params.is_empty()),
                other => panic!("expected closure, got {:?}", other),
            },
            other => panic!("expected let, got {:?}", other),
        }
    }

    #[test]
    fn parse_closure_with_one_param() {
        let src = "fn main() -> Word { let f = |x: Word| x + 1; 0 }";
        let program = parse_str(src).unwrap();
        let body = &program.functions[0].body;
        match &body.stmts[0] {
            Stmt::Let(l) => match &l.value {
                Expr::Closure { params, .. } => assert_eq!(params.len(), 1),
                other => panic!("expected closure, got {:?}", other),
            },
            other => panic!("expected let, got {:?}", other),
        }
    }

    #[test]
    fn parse_closure_with_block_body() {
        let src = "fn main() -> Word { let f = |x: Word| -> Word { x * 2 }; 0 }";
        let program = parse_str(src).unwrap();
        let body = &program.functions[0].body;
        match &body.stmts[0] {
            Stmt::Let(l) => match &l.value {
                Expr::Closure {
                    return_type, body, ..
                } => {
                    assert!(return_type.is_some());
                    assert!(body.tail_expr.is_some());
                }
                other => panic!("expected closure, got {:?}", other),
            },
            other => panic!("expected let, got {:?}", other),
        }
    }

    #[test]
    fn parse_fn_empty_type_params_accepted() {
        // `fn name<>(...)` is admitted as the trivial empty-list case.
        // Conventional callers elide the brackets.
        let src = "fn nogen<>(x: Word) -> Word { x }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions[0].type_params.len(), 0);
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
    #[cfg(feature = "floats")]
    fn parse_guard_clause() {
        let src = r#"
            fn severity(level: Float) -> Word when level >= 0.9 {
                1
            }
        "#;
        let program = parse_str(src).unwrap();
        assert!(program.functions[0].guard.is_some());
    }

    #[test]
    fn parse_multiheaded_function() {
        let src = r#"
            fn describe(Command::NoteOn(ch, note, vel)) -> Word {
                1
            }
            fn describe(Command::Silence) -> Word {
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
        let src = "use audio::set_frequency fn test() -> Word { 0 }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.uses.len(), 1);
        assert_eq!(program.uses[0].path, vec!["audio"]);
        assert_eq!(
            program.uses[0].import,
            ImportItem::Name(String::from("set_frequency"))
        );
        assert!(!program.uses[0].is_external);
    }

    #[test]
    fn parse_use_wildcard() {
        let src = "use audio::* fn test() -> Word { 0 }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.uses[0].import, ImportItem::Wildcard);
        assert!(!program.uses[0].is_external);
    }

    #[test]
    fn parse_use_external() {
        let src = "use external host::log_event fn test() -> Word { 0 }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.uses.len(), 1);
        assert_eq!(program.uses[0].path, vec!["host"]);
        assert_eq!(
            program.uses[0].import,
            ImportItem::Name(String::from("log_event")),
        );
        assert!(program.uses[0].is_external);
    }

    #[test]
    fn parse_use_external_wildcard() {
        let src = "use external host::* fn test() -> Word { 0 }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.uses[0].import, ImportItem::Wildcard);
        assert!(program.uses[0].is_external);
    }

    #[test]
    fn parse_struct_def() {
        let src = r#"
            struct Note {
                channel: Word,
                pitch: Word,
                velocity: Float,
            }
            fn test() -> Word { 0 }
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
                NoteOn(Word, Word, Float),
                NoteOff(Word),
                Silence,
            }
            fn test() -> Word { 0 }
        "#;
        let program = parse_str(src).unwrap();
        match &program.types[0] {
            TypeDef::Enum(e) => {
                assert_eq!(e.name, "Command");
                assert_eq!(e.variants.len(), 3);
                assert_eq!(e.variants[0].fields.len(), 3);
                assert!(e.variants[2].fields.is_empty());
                // Without explicit discriminants, values auto-assign
                // from zero in declaration order.
                assert_eq!(e.variants[0].discriminant_value, 0);
                assert_eq!(e.variants[1].discriminant_value, 1);
                assert_eq!(e.variants[2].discriminant_value, 2);
                assert!(e.variants[0].explicit_discriminant.is_none());
            }
            _ => panic!("expected enum"),
        }
    }

    #[test]
    fn parse_enum_with_explicit_discriminants() {
        let src = r#"
            enum ErrorCode {
                OutOfRange = 1,
                NotConfigured = 2,
                Busy = 3,
                Timeout = 4,
                HardwareFault = 5,
                Unsupported = 6,
            }
            fn test() -> Word { 0 }
        "#;
        let program = parse_str(src).unwrap();
        match &program.types[0] {
            TypeDef::Enum(e) => {
                assert_eq!(e.variants.len(), 6);
                for (i, v) in e.variants.iter().enumerate() {
                    assert_eq!(v.explicit_discriminant, Some((i + 1) as i64));
                    assert_eq!(v.discriminant_value, (i + 1) as i64);
                }
            }
            _ => panic!("expected enum"),
        }
    }

    #[test]
    fn parse_enum_with_mixed_discriminants() {
        // Some variants have explicit values; the others auto-fill
        // from one past the preceding variant.
        let src = r#"
            enum Mixed {
                A,
                B = 10,
                C,
                D = 20,
                E,
            }
            fn test() -> Word { 0 }
        "#;
        let program = parse_str(src).unwrap();
        match &program.types[0] {
            TypeDef::Enum(e) => {
                let values: Vec<i64> = e.variants.iter().map(|v| v.discriminant_value).collect();
                assert_eq!(values, vec![0, 10, 11, 20, 21]);
                let explicit: Vec<Option<i64>> =
                    e.variants.iter().map(|v| v.explicit_discriminant).collect();
                assert_eq!(explicit, vec![None, Some(10), None, Some(20), None]);
            }
            _ => panic!("expected enum"),
        }
    }

    #[test]
    fn parse_enum_rejects_duplicate_explicit_discriminants() {
        let src = r#"
            enum Bad {
                A = 1,
                B = 1,
            }
            fn test() -> Word { 0 }
        "#;
        let err = parse_str(src).unwrap_err();
        assert!(
            err.message.contains("duplicate") || err.message.contains("duplicates"),
            "expected duplicate-discriminant error, got: {}",
            err.message
        );
    }

    #[test]
    fn parse_enum_rejects_implicit_and_explicit_collision() {
        // A defaults to 0, B explicitly takes 1, C implicitly
        // wants 2, D explicitly takes 1 — collides with B.
        let src = r#"
            enum Bad {
                A,
                B = 1,
                C,
                D = 1,
            }
            fn test() -> Word { 0 }
        "#;
        let err = parse_str(src).unwrap_err();
        assert!(
            err.message.contains("duplicate") || err.message.contains("duplicates"),
            "expected duplicate-discriminant error, got: {}",
            err.message
        );
    }

    #[test]
    fn parse_enum_accepts_negative_discriminants() {
        // Negative discriminants are useful for signed error
        // codes or for marking "no value yet" sentinels at the
        // low end of the range.
        let src = r#"
            enum Signed {
                Below = -2,
                Just = -1,
                Zero = 0,
                Above = 1,
            }
            fn test() -> Word { 0 }
        "#;
        let program = parse_str(src).unwrap();
        match &program.types[0] {
            TypeDef::Enum(e) => {
                let values: Vec<i64> = e.variants.iter().map(|v| v.discriminant_value).collect();
                assert_eq!(values, vec![-2, -1, 0, 1]);
                let explicit: Vec<Option<i64>> =
                    e.variants.iter().map(|v| v.explicit_discriminant).collect();
                assert_eq!(explicit, vec![Some(-2), Some(-1), Some(0), Some(1)]);
            }
            _ => panic!("expected enum"),
        }
    }

    #[test]
    fn parse_enum_negative_then_implicit_continues_correctly() {
        // After an explicit `= -5`, the implicit counter resumes
        // at -4.
        let src = r#"
            enum Run {
                A = -5,
                B,
                C,
            }
            fn test() -> Word { 0 }
        "#;
        let program = parse_str(src).unwrap();
        match &program.types[0] {
            TypeDef::Enum(e) => {
                let values: Vec<i64> = e.variants.iter().map(|v| v.discriminant_value).collect();
                assert_eq!(values, vec![-5, -4, -3]);
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
        let src = "fn test(x: Option<Word>) -> Word { 0 }";
        let program = parse_str(src).unwrap();
        let param_type = program.functions[0].params[0].type_expr.as_ref().unwrap();
        assert!(matches!(param_type, TypeExpr::Option(_, _)));
    }

    #[test]
    fn parse_array_type() {
        let src = "fn test(x: [Float; 8]) -> Word { 0 }";
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
            loop main(cmd: Word) -> Word {
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
                NoteOn(Word, Word, Float),
                NoteOff(Word),
                Tick,
            }

            enum AudioAction {
                PlayNote(Word, Word, Float),
                StopNote(Word),
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
        let src = "fn test() -> Word { let x = 1 x }";
        let result = parse_str(src);
        assert!(result.is_err());
    }

    #[test]
    fn error_unexpected_token() {
        let src = "fn test() -> Word { + }";
        let result = parse_str(src);
        assert!(result.is_err());
    }

    #[test]
    fn parse_data_decl() {
        let src = "\
            data ctx {\n\
                score: Word,\n\
                health: Float,\n\
            }\n\
            fn main() -> Word { ctx.score }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.data_decls.len(), 1);
        assert_eq!(program.data_decls[0].name, "ctx");
        assert_eq!(program.data_decls[0].visibility, DataVisibility::Shared);
        assert_eq!(program.data_decls[0].fields.len(), 2);
        assert_eq!(program.data_decls[0].fields[0].name, "score");
        assert_eq!(program.data_decls[0].fields[1].name, "health");
    }

    #[test]
    fn parse_shared_data_decl_explicit() {
        let src = "\
            shared data ctx {\n\
                score: Word,\n\
            }\n\
            fn main() -> Word { ctx.score }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.data_decls[0].visibility, DataVisibility::Shared);
        assert_eq!(program.data_decls[0].name, "ctx");
    }

    #[test]
    fn parse_private_data_decl() {
        let src = "\
            private data state {\n\
                counter: Word,\n\
            }\n\
            fn main() -> Word { state.counter }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.data_decls.len(), 1);
        assert_eq!(program.data_decls[0].name, "state");
        assert_eq!(program.data_decls[0].visibility, DataVisibility::Private);
        assert_eq!(program.data_decls[0].fields.len(), 1);
    }

    #[test]
    fn parse_ephemeral_fn_main() {
        let src = "\
            ephemeral fn main() -> Word { 0 }";
        let program = parse_str(src).unwrap();
        assert_eq!(program.functions.len(), 1);
        assert!(program.functions[0].ephemeral);
        assert_eq!(program.functions[0].category, FunctionCategory::Fn);
    }

    #[test]
    fn parse_ephemeral_loop_main() {
        let src = "\
            ephemeral loop main(_r: Word) -> (Word, Word) { yield (0, 0); (0, 0) }";
        let program = parse_str(src).unwrap();
        assert!(program.functions[0].ephemeral);
        assert_eq!(program.functions[0].category, FunctionCategory::Loop);
    }

    #[test]
    fn parse_non_ephemeral_function_defaults_to_false() {
        let src = "fn main() -> Word { 0 }";
        let program = parse_str(src).unwrap();
        assert!(!program.functions[0].ephemeral);
    }

    #[test]
    fn parse_data_field_assign() {
        let src = "\
            data ctx {\n\
                value: Word,\n\
            }\n\
            fn main() -> Word {\n\
                ctx.value = 42;\n\
                ctx.value\n\
            }";
        let program = parse_str(src).unwrap();
        let body = &program.functions[0].body;
        assert!(matches!(&body.stmts[0], Stmt::DataFieldAssign { .. }));
    }
}
