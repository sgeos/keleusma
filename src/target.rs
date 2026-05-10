//! Target descriptor for cross-architecture portability.
//!
//! Keleusma's bytecode wire format records the word, address, and
//! floating-point widths declared by the producer. The runtime
//! accepts bytecode whose widths are at most the runtime's own. A
//! `Target` describes the producer's intended target and includes
//! capability flags that gate feature usage at compile time, so the
//! producer can refuse to emit bytecode that would fail to load on
//! the intended runtime.
//!
//! Scope of the present implementation. The compiler accepts a
//! `Target` and bakes its widths into the wire format. The compiler
//! rejects programs that use features not supported by the target,
//! such as floating-point operations on a no-float target. The
//! runtime continues to be 64-bit; emitting bytecode for a narrower
//! target produces bytecode that the current runtime can still load
//! (because narrower-than-runtime widths are admissible) and the
//! integer arithmetic path masks results to the declared width via
//! `truncate_int`. Cross-target codegen, target-specific runtime
//! representations of `Value`, and target-defined primitive types
//! (`byte`, `bit`, `word`, `address`) remain future work tracked in
//! BACKLOG entry B10.
//!
//! Use cases. (1) A host that targets a future 32-bit embedded
//! runtime can compile against `Target::embedded_32()` to emit
//! bytecode whose declared widths match the embedded runtime, and
//! the current 64-bit runtime can still execute it during
//! development. (2) A host that wants to ensure its scripts do not
//! use floats can compile against a target with `has_floats =
//! false`; programs using float literals or float types are rejected
//! at compile time. (3) Tooling can inspect a Target's capability
//! flags to surface compile-time documentation about what features
//! the deployed runtime supports.

extern crate alloc;
use alloc::format;
use alloc::string::String;

use crate::ast::{Expr, Literal, PrimType, Program, Stmt, TypeExpr};
use crate::bytecode::{RUNTIME_ADDRESS_BITS_LOG2, RUNTIME_FLOAT_BITS_LOG2, RUNTIME_WORD_BITS_LOG2};
use crate::compiler::CompileError;
use crate::token::Span;
use crate::visitor::Visitor;

/// Target descriptor describing word/address/float widths and
/// feature flags for a compilation target.
///
/// Widths are encoded as base-2 exponents matching the wire-format
/// fields. Actual width in bits is `1 << field`. The runtime accepts
/// bytecode with widths at most its own. Construct a `Target`
/// through one of the const presets (`host`, `wasm32`, `embedded_32`,
/// `embedded_16`, `embedded_8`) or through the constructor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// Word size as the base-2 exponent of bits. Common values:
    /// 6 = 64-bit, 5 = 32-bit, 4 = 16-bit, 3 = 8-bit.
    pub word_bits_log2: u8,
    /// Address size as the base-2 exponent of bits. Often equal to
    /// the word size on flat-address targets. The 6502 is an example
    /// of an 8-bit-word, 16-bit-address target.
    pub addr_bits_log2: u8,
    /// Floating-point width as the base-2 exponent of bits.
    /// Honored only when `has_floats` is true.
    pub float_bits_log2: u8,
    /// Whether the target supports floating-point types and
    /// operations. When false, the compiler rejects programs that
    /// use float literals, the `f64` type, or float-conversion ops.
    pub has_floats: bool,
    /// Whether the target supports string types. When false, the
    /// compiler rejects programs that use string literals or the
    /// `String` type. Useful for very-small targets where dynamic
    /// strings are out of budget.
    pub has_strings: bool,
}

impl Target {
    /// Default target for the host runtime. Matches the runtime's
    /// declared widths and enables all features. Equivalent to
    /// passing no target descriptor.
    pub const fn host() -> Self {
        Self {
            word_bits_log2: RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: RUNTIME_FLOAT_BITS_LOG2,
            has_floats: true,
            has_strings: true,
        }
    }

    /// 32-bit WebAssembly target. 32-bit word and address, 64-bit
    /// floats, full feature set.
    pub const fn wasm32() -> Self {
        Self {
            word_bits_log2: 5,
            addr_bits_log2: 5,
            float_bits_log2: 6,
            has_floats: true,
            has_strings: true,
        }
    }

    /// 32-bit embedded target. 32-bit word and address, 32-bit
    /// floats, full feature set.
    pub const fn embedded_32() -> Self {
        Self {
            word_bits_log2: 5,
            addr_bits_log2: 5,
            float_bits_log2: 5,
            has_floats: true,
            has_strings: true,
        }
    }

    /// 16-bit embedded target. 16-bit word and address, no floats,
    /// strings still allowed.
    pub const fn embedded_16() -> Self {
        Self {
            word_bits_log2: 4,
            addr_bits_log2: 4,
            float_bits_log2: 0,
            has_floats: false,
            has_strings: true,
        }
    }

    /// 8-bit embedded target with 16-bit address space (6502 class).
    /// No floats, no strings.
    pub const fn embedded_8() -> Self {
        Self {
            word_bits_log2: 3,
            addr_bits_log2: 4,
            float_bits_log2: 0,
            has_floats: false,
            has_strings: false,
        }
    }

    /// Width in bits of the target's word.
    pub const fn word_bits(&self) -> u32 {
        1u32 << self.word_bits_log2
    }

    /// Width in bits of the target's address.
    pub const fn address_bits(&self) -> u32 {
        1u32 << self.addr_bits_log2
    }

    /// Width in bits of the target's float type, when `has_floats`
    /// is true. When `has_floats` is false the value is not
    /// meaningful and should not be consulted.
    pub const fn float_bits(&self) -> u32 {
        1u32 << self.float_bits_log2
    }

    /// Validate that the target's widths are admissible by the
    /// current runtime. Returns an error describing the first
    /// width that exceeds the runtime's capability.
    pub fn validate_against_runtime(&self) -> Result<(), CompileError> {
        if self.word_bits_log2 > RUNTIME_WORD_BITS_LOG2 {
            return Err(CompileError {
                message: format!(
                    "target word_bits_log2 = {} exceeds runtime maximum {}",
                    self.word_bits_log2, RUNTIME_WORD_BITS_LOG2
                ),
                span: Span::default(),
            });
        }
        if self.addr_bits_log2 > RUNTIME_ADDRESS_BITS_LOG2 {
            return Err(CompileError {
                message: format!(
                    "target addr_bits_log2 = {} exceeds runtime maximum {}",
                    self.addr_bits_log2, RUNTIME_ADDRESS_BITS_LOG2
                ),
                span: Span::default(),
            });
        }
        if self.has_floats && self.float_bits_log2 > RUNTIME_FLOAT_BITS_LOG2 {
            return Err(CompileError {
                message: format!(
                    "target float_bits_log2 = {} exceeds runtime maximum {}",
                    self.float_bits_log2, RUNTIME_FLOAT_BITS_LOG2
                ),
                span: Span::default(),
            });
        }
        Ok(())
    }
}

impl Default for Target {
    fn default() -> Self {
        Self::host()
    }
}

/// Validate that the program does not use features unsupported by
/// the target. Walks the program's AST looking for float literals,
/// float types, string literals, and string types, and reports the
/// first violation as a `CompileError`.
pub(crate) fn validate_program_for_target(
    program: &Program,
    target: &Target,
) -> Result<(), CompileError> {
    if target.has_floats && target.has_strings {
        return Ok(());
    }
    let mut checker = TargetChecker {
        target,
        first_error: None,
    };
    for func in &program.functions {
        checker.check_type(&func.return_type);
        for param in &func.params {
            if let Some(t) = &param.type_expr {
                checker.check_type(t);
            }
        }
        checker.visit_block(&func.body);
    }
    for impl_block in &program.impls {
        for method in &impl_block.methods {
            checker.check_type(&method.return_type);
            for param in &method.params {
                if let Some(t) = &param.type_expr {
                    checker.check_type(t);
                }
            }
            checker.visit_block(&method.body);
        }
    }
    match checker.first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// AST visitor that records the first feature-violation error
/// encountered during a walk. Subsequent visits short-circuit so the
/// reported error points at the earliest source position.
struct TargetChecker<'a> {
    target: &'a Target,
    first_error: Option<CompileError>,
}

impl TargetChecker<'_> {
    fn check_type(&mut self, ty: &TypeExpr) {
        if self.first_error.is_some() {
            return;
        }
        match ty {
            TypeExpr::Prim(PrimType::F64, span) if !self.target.has_floats => {
                self.first_error = Some(CompileError {
                    message: String::from("target does not support floating-point types"),
                    span: *span,
                });
            }
            TypeExpr::Prim(PrimType::KString, span) if !self.target.has_strings => {
                self.first_error = Some(CompileError {
                    message: String::from("target does not support string types"),
                    span: *span,
                });
            }
            TypeExpr::Tuple(elems, _) => {
                for e in elems {
                    self.check_type(e);
                }
            }
            TypeExpr::Array(elem, _, _) => self.check_type(elem),
            TypeExpr::Option(inner, _) => self.check_type(inner),
            TypeExpr::Named(_, args, _) => {
                for a in args {
                    self.check_type(a);
                }
            }
            _ => {}
        }
    }
}

impl Visitor for TargetChecker<'_> {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        if self.first_error.is_some() {
            return;
        }
        if let Stmt::Let(l) = stmt
            && let Some(t) = &l.type_expr
        {
            self.check_type(t);
        }
        self.walk_stmt(stmt);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        if self.first_error.is_some() {
            return;
        }
        match expr {
            Expr::Literal { value, span } => match value {
                Literal::Float(_) if !self.target.has_floats => {
                    self.first_error = Some(CompileError {
                        message: String::from("target does not support floating-point literals"),
                        span: *span,
                    });
                }
                Literal::String(_) if !self.target.has_strings => {
                    self.first_error = Some(CompileError {
                        message: String::from("target does not support string literals"),
                        span: *span,
                    });
                }
                _ => {}
            },
            Expr::Cast {
                target: cast_target,
                ..
            } => self.check_type(cast_target),
            Expr::Closure {
                params,
                return_type,
                ..
            } => {
                for p in params {
                    if let Some(t) = &p.type_expr {
                        self.check_type(t);
                    }
                }
                if let Some(t) = return_type {
                    self.check_type(t);
                }
            }
            _ => {}
        }
        self.walk_expr(expr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile_with_target;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn try_compile_with_target(src: &str, target: &Target) -> Result<(), String> {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        compile_with_target(&program, target)
            .map(|_| ())
            .map_err(|e| e.message)
    }

    #[test]
    fn host_target_admits_full_program() {
        try_compile_with_target("fn main() -> i64 { 1 + 2 }", &Target::host()).unwrap();
    }

    #[test]
    fn host_target_admits_floats_and_strings() {
        try_compile_with_target(
            "fn main() -> i64 {\n\
                 let f: f64 = 1.5;\n\
                 let s: String = \"hello\";\n\
                 0\n\
             }",
            &Target::host(),
        )
        .unwrap();
    }

    #[test]
    fn embedded_16_rejects_float_literal() {
        let err = try_compile_with_target("fn main() -> f64 { 1.5 }", &Target::embedded_16())
            .unwrap_err();
        assert!(
            err.contains("does not support floating-point"),
            "unexpected error: {}",
            err,
        );
    }

    #[test]
    fn embedded_16_rejects_float_type_in_param() {
        let err = try_compile_with_target(
            "fn add(x: f64) -> f64 { x }\nfn main() -> i64 { 0 }",
            &Target::embedded_16(),
        )
        .unwrap_err();
        assert!(
            err.contains("does not support floating-point"),
            "unexpected error: {}",
            err,
        );
    }

    #[test]
    fn embedded_8_rejects_string_literal() {
        let err = try_compile_with_target(
            "fn main() -> i64 { let s = \"hello\"; 0 }",
            &Target::embedded_8(),
        )
        .unwrap_err();
        assert!(
            err.contains("does not support string"),
            "unexpected error: {}",
            err,
        );
    }

    #[test]
    fn embedded_8_admits_int_only_program() {
        try_compile_with_target(
            "fn main() -> i64 { let x: i64 = 7; x + 3 }",
            &Target::embedded_8(),
        )
        .unwrap();
    }

    #[test]
    fn target_widths_propagate_to_module() {
        let tokens = tokenize("fn main() -> i64 { 0 }").expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile_with_target(&program, &Target::embedded_16()).unwrap();
        assert_eq!(module.word_bits_log2, 4);
        assert_eq!(module.addr_bits_log2, 4);
    }

    #[test]
    fn host_widths_match_runtime_constants() {
        let tokens = tokenize("fn main() -> i64 { 0 }").expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile_with_target(&program, &Target::host()).unwrap();
        assert_eq!(module.word_bits_log2, RUNTIME_WORD_BITS_LOG2);
        assert_eq!(module.addr_bits_log2, RUNTIME_ADDRESS_BITS_LOG2);
        assert_eq!(module.float_bits_log2, RUNTIME_FLOAT_BITS_LOG2);
    }

    #[test]
    fn target_validation_against_runtime_rejects_oversized() {
        let oversize = Target {
            word_bits_log2: RUNTIME_WORD_BITS_LOG2 + 1,
            addr_bits_log2: RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: RUNTIME_FLOAT_BITS_LOG2,
            has_floats: true,
            has_strings: true,
        };
        let err = oversize.validate_against_runtime().unwrap_err();
        assert!(
            err.message.contains("word_bits_log2"),
            "unexpected error: {}",
            err.message,
        );
    }
}
