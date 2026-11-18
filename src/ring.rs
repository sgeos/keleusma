//! The compiler-managed ring buffer type `Ring<T, N>`, increment R1.
//!
//! The design is `docs/decisions/RING_BUFFER_DESIGN.md`, and its section "R1 as
//! built" records what this module implements. R1 builds the type surface and
//! the data layout and no operation. This module holds the three pieces both
//! compilers must agree on byte for byte.
//!
//! 1. The admissibility rules, checked by [`check_ring_declarations`] before the
//!    type checker resolves anything, so that every refusal names the
//!    restriction it enforces.
//! 2. The hidden names of the three data fields a ring field lowers to, given by
//!    [`hidden_field_names`]. The reference compiler expands a ring field with
//!    [`expand_ring_fields`], and the self-hosted driver builds the same names
//!    from the same function, so the spelling cannot drift between the two.
//! 3. The capacity bound [`RING_CAPACITY_MAX`].
//!
//! # The hidden names, and why a dot
//!
//! A ring field `r` of block `d` becomes the data fields `r.len`, `r.head` and
//! `r.buf`, which the data layout records as `d.r.len`, `d.r.head` and
//! `d.r.buf`. A field name in source is one identifier, so no source program can
//! declare a field whose name contains a dot, and a source access `d.r.len`
//! parses as a field access on the ring field `r`, which the type checker
//! refuses. Every name a host can declare therefore has exactly one dot after the
//! block name and every hidden name has two, so the two sets cannot collide. A
//! host that addresses a shared field by name through
//! [`crate::bytecode::DataLayout::slot_of`] finds the three parts under their
//! full names and finds nothing under `d.r` itself, which is deliberate, since a
//! ring is not one slot.
//!
//! # Trust boundary
//!
//! A shared ring lives in a host-owned buffer, and a host may write any value
//! into its length and head between calls. R1 builds no operation, so nothing
//! reads those words yet. The design's decision D5, ruled option (a), makes every
//! later operation validate them and trap with `IndexOutOfBounds`.

use crate::ast::{
    ConstBinOp, ConstExpr, DataVisibility, Expr, PrimType, Program, Stmt, TypeDef, TypeExpr,
};
use crate::token::Span;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// The suffixes of the three hidden data fields a ring field lowers to, in slot
/// order: the live length, the head, and the element array.
pub const RING_HIDDEN_SUFFIXES: [&str; 3] = ["len", "head", "buf"];

/// The largest admissible ring capacity.
///
/// A ring occupies `2 + N` data slots, and the design requires `2 + N` to fit
/// strictly within the twenty-four-bit indexed data operand, the bound the
/// compiler applies to an indexed data array, which refuses a total of
/// [`crate::bytecode::MAX_DATA_ADDR`] or more. So `2 + N` is at most
/// `MAX_DATA_ADDR - 1` and `N` at most `MAX_DATA_ADDR - 3`.
pub const RING_CAPACITY_MAX: i64 = crate::bytecode::MAX_DATA_ADDR as i64 - 3;

/// A refusal raised by the ring admissibility rules, carrying the source span of
/// the offending type or field.
#[derive(Debug, Clone, PartialEq)]
pub struct RingError {
    /// The diagnostic, naming the restriction.
    pub message: String,
    /// The offending source span.
    pub span: Span,
}

impl RingError {
    fn new(message: String, span: Span) -> Self {
        Self { message, span }
    }
}

/// The three hidden field names of a ring field named `field`, in slot order.
///
/// These are the field names inside the data block. The data layout prefixes
/// each with the block name and a dot, as it does every field name.
#[must_use]
pub fn hidden_field_names(field: &str) -> [String; 3] {
    RING_HIDDEN_SUFFIXES.map(|suffix| format!("{field}.{suffix}"))
}

/// Whether `elem` is an admissible first-pass ring element type, one of `Word`,
/// `Byte`, `bool`, `Fixed` and `Float`, each of which occupies one data slot.
///
/// A labelled element is refused, since an information-flow label on a data
/// field belongs on the field and the first pass does not define how a label on
/// an element would propagate through the ring's operations.
#[must_use]
pub fn element_admissible(elem: &TypeExpr) -> bool {
    matches!(
        elem,
        TypeExpr::Prim(
            PrimType::Word | PrimType::Byte | PrimType::Bool | PrimType::Fixed(_) | PrimType::Float,
            _
        )
    )
}

/// Evaluate a ring capacity to its value, or refuse it.
///
/// A literal and a total const expression over literals are admitted, as for an
/// array length. A const parameter cannot occur in a data block, which is never
/// generic, so a name is refused as not a constant. Every arithmetic step is
/// checked, and an overflowing expression is refused rather than wrapped. The
/// walk is iterative over an explicit stack, so its termination follows from the
/// finite expression, each node being pushed once.
///
/// # Errors
///
/// [`RingError`] when the capacity is not a constant, overflows, is not
/// positive, or exceeds [`RING_CAPACITY_MAX`].
pub fn ring_capacity(n: &ConstExpr) -> Result<i64, RingError> {
    let span = n.span();
    let not_constant = || {
        RingError::new(
            format!(
                "Ring capacity `{n}` is not a constant: a ring capacity must be a positive \
                 integer constant"
            ),
            span,
        )
    };
    // Post-order evaluation with an explicit stack. `Visit` expands a node,
    // `Apply` folds its two already-evaluated operands.
    enum Step<'a> {
        Visit(&'a ConstExpr),
        Apply(ConstBinOp),
    }
    let mut work: Vec<Step<'_>> = Vec::new();
    let mut values: Vec<i64> = Vec::new();
    work.push(Step::Visit(n));
    while let Some(step) = work.pop() {
        match step {
            Step::Visit(ConstExpr::Lit(v, _)) => values.push(*v),
            Step::Visit(ConstExpr::Param(_, _)) => return Err(not_constant()),
            Step::Visit(ConstExpr::Bin(op, l, r, _)) => {
                work.push(Step::Apply(*op));
                work.push(Step::Visit(r));
                work.push(Step::Visit(l));
            }
            Step::Apply(op) => {
                let (Some(r), Some(l)) = (values.pop(), values.pop()) else {
                    return Err(not_constant());
                };
                let folded = match op {
                    ConstBinOp::Add => l.checked_add(r),
                    ConstBinOp::Sub => l.checked_sub(r),
                    ConstBinOp::Mul => l.checked_mul(r),
                };
                values.push(folded.ok_or_else(|| {
                    RingError::new(
                        format!("Ring capacity `{n}` overflows the Word range"),
                        span,
                    )
                })?);
            }
        }
    }
    let value = match values.as_slice() {
        [v] => *v,
        _ => return Err(not_constant()),
    };
    if value < 1 {
        return Err(RingError::new(
            format!("Ring capacity {value} is not positive: a ring holds at least one element"),
            span,
        ));
    }
    if value > RING_CAPACITY_MAX {
        return Err(RingError::new(
            format!(
                "Ring capacity {value} is too large: a ring occupies 2 + N data slots and \
                 2 + N must be below the 24-bit indexed data operand bound of {}, so N is at \
                 most {RING_CAPACITY_MAX}",
                crate::bytecode::MAX_DATA_ADDR
            ),
            span,
        ));
    }
    Ok(value)
}

/// The first `Ring` anywhere inside `te`, including `te` itself, as its span.
///
/// The walk is iterative over an explicit stack, so its termination follows from
/// the finite type expression, each node being pushed once.
fn find_ring(te: &TypeExpr) -> Option<Span> {
    let mut work: Vec<&TypeExpr> = Vec::new();
    work.push(te);
    while let Some(t) = work.pop() {
        match t {
            TypeExpr::Ring(_, _, span) => return Some(*span),
            TypeExpr::Array(inner, _, _)
            | TypeExpr::Option(inner, _)
            | TypeExpr::Labelled(inner, _, _)
            | TypeExpr::NegativeLabelled(inner, _, _) => work.push(inner),
            TypeExpr::Tuple(items, _) | TypeExpr::Named(_, items, _, _) => {
                work.extend(items.iter())
            }
            TypeExpr::Prim(_, _)
            | TypeExpr::Unit(_)
            | TypeExpr::Multiword(_, _, _)
            | TypeExpr::TextN(_, _) => {}
        }
    }
    None
}

/// Refuse `te` when it contains a ring, naming the position `what`.
fn refuse_ring_in(te: &TypeExpr, what: &str) -> Result<(), RingError> {
    match find_ring(te) {
        Some(span) => Err(RingError::new(
            format!(
                "Ring<T, N> is not admissible in {what}: in this release a ring is admitted \
                 only as the declared type of a field of a `private data` or `shared data` block"
            ),
            span,
        )),
        None => Ok(()),
    }
}

/// Validate one data-block field whose declared type is a ring.
fn check_ring_field(
    block: &str,
    visibility: DataVisibility,
    field: &crate::ast::DataFieldDecl,
    elem: &TypeExpr,
    cap: &ConstExpr,
    span: Span,
) -> Result<(), RingError> {
    if visibility == DataVisibility::Const {
        return Err(RingError::new(
            format!(
                "data field `{block}.{}` is a Ring in a `const data` block: a ring that cannot \
                 change is an array, so declare `[T; N]` instead",
                field.name
            ),
            span,
        ));
    }
    if !element_admissible(elem) {
        return Err(RingError::new(
            format!(
                "Ring element type of `{block}.{}` is not admissible: in this release a ring \
                 element is one of Word, Byte, bool, Fixed and Float",
                field.name
            ),
            elem.span(),
        ));
    }
    ring_capacity(cap)?;
    if field.initializer.is_some() {
        return Err(RingError::new(
            format!(
                "ring field `{block}.{}` has an initializer: a ring field admits no initializer \
                 and is empty at load",
                field.name
            ),
            field.span,
        ));
    }
    Ok(())
}

/// Collects the first ring found in a function body's type positions, which are
/// `let` annotations, cast targets and closure signatures.
struct BodyRingFinder {
    found: Option<RingError>,
}

impl BodyRingFinder {
    fn note(&mut self, te: &TypeExpr, what: &str) {
        if self.found.is_none()
            && let Err(e) = refuse_ring_in(te, what)
        {
            self.found = Some(e);
        }
    }
}

impl crate::visitor::Visitor for BodyRingFinder {
    fn visit_stmt(&mut self, stmt: &Stmt) {
        if let Stmt::Let(l) = stmt
            && let Some(te) = &l.type_expr
        {
            self.note(te, "a local binding");
        }
        self.walk_stmt(stmt);
    }
    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Cast { target, .. } => self.note(target, "a cast target"),
            Expr::Closure {
                params,
                return_type,
                ..
            } => {
                for p in params {
                    if let Some(te) = &p.type_expr {
                        self.note(te, "a closure parameter");
                    }
                }
                if let Some(te) = return_type {
                    self.note(te, "a closure return type");
                }
            }
            _ => {}
        }
        self.walk_expr(expr);
    }
}

/// Refuse a ring in any position of a function's signature or body.
fn check_function(f: &crate::ast::FunctionDef) -> Result<(), RingError> {
    for p in &f.params {
        if let Some(te) = &p.type_expr {
            refuse_ring_in(te, "a function parameter")?;
        }
    }
    // The return type of a `yield` or `loop` function is its yield type.
    refuse_ring_in(&f.return_type, "a function return or yield type")?;
    let mut finder = BodyRingFinder { found: None };
    crate::visitor::Visitor::visit_block(&mut finder, &f.body);
    if let Some(guard) = &f.guard {
        crate::visitor::Visitor::visit_expr(&mut finder, guard);
    }
    match finder.found {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// Check every ring in `program` against the R1 admissibility rules.
///
/// A ring is admitted only as the whole declared type of a field of a
/// `private data` or `shared data` block, with an admissible element type, a
/// positive constant capacity within [`RING_CAPACITY_MAX`] and no initializer.
/// Every other position is refused, and the walk covers every declaration form
/// that carries a type, by class: function signatures and bodies, impl methods
/// and their target type, trait signatures, struct fields, enum payloads,
/// newtypes, native signatures, and data fields, including a ring nested inside
/// a data field's type.
///
/// # Errors
///
/// The first [`RingError`] in declaration order.
pub fn check_ring_declarations(program: &Program) -> Result<(), RingError> {
    for decl in &program.data_decls {
        for field in &decl.fields {
            match &field.type_expr {
                TypeExpr::Ring(elem, cap, span) => {
                    check_ring_field(&decl.name, decl.visibility, field, elem, cap, *span)?
                }
                other => refuse_ring_in(other, "a nested position of a data field type")?,
            }
        }
    }
    for td in &program.types {
        match td {
            TypeDef::Struct(s) => {
                for f in &s.fields {
                    refuse_ring_in(&f.type_expr, "a struct field")?;
                }
            }
            TypeDef::Enum(e) => {
                for v in &e.variants {
                    for t in &v.fields {
                        refuse_ring_in(t, "an enum variant payload")?;
                    }
                }
            }
            TypeDef::Newtype(n) => refuse_ring_in(&n.underlying, "a newtype")?,
        }
    }
    for f in &program.functions {
        check_function(f)?;
    }
    for b in &program.impls {
        refuse_ring_in(&b.for_type, "an impl target type")?;
        for m in &b.methods {
            check_function(m)?;
        }
    }
    for t in &program.traits {
        for m in &t.methods {
            for p in &m.params {
                if let Some(te) = &p.type_expr {
                    refuse_ring_in(te, "a trait method parameter")?;
                }
            }
            refuse_ring_in(&m.return_type, "a trait method return type")?;
        }
    }
    for u in &program.uses {
        if let Some(sig) = &u.signature {
            for p in &sig.params {
                refuse_ring_in(p, "a native signature")?;
            }
            refuse_ring_in(&sig.return_type, "a native signature")?;
        }
    }
    Ok(())
}

/// The refusal for an expression that reads or writes the ring field
/// `block.field`, which R1 raises for every such expression because no ring
/// operation is built yet.
#[must_use]
pub fn ring_access_message(block: &str, field: &str) -> String {
    format!(
        "ring field `{block}.{field}` cannot be read or written: ring operations are not yet \
         implemented, and in this release a ring may only be declared"
    )
}

/// Replace every ring field of a `private data` or `shared data` block with its
/// three hidden fields, a length `Word`, a head `Word` and an element array
/// `[T; N]`, in that order and at the ring's position.
///
/// This is the frontend expansion of the design. It runs after the type checker
/// has admitted the program, so every ring here has passed
/// [`check_ring_declarations`] and no expression names a ring field. After it,
/// the data layout, the private initializers, the shared byte offsets and the
/// typed schema hash are computed by the existing paths for ordinary fields.
///
/// # Errors
///
/// [`RingError`] when a ring fails the capacity rule, which the type checker
/// should already have refused. It is reported rather than assumed.
pub fn expand_ring_fields(program: &mut Program) -> Result<(), RingError> {
    for decl in &mut program.data_decls {
        if !decl
            .fields
            .iter()
            .any(|f| matches!(f.type_expr, TypeExpr::Ring(..)))
        {
            continue;
        }
        let mut expanded = Vec::with_capacity(decl.fields.len().saturating_add(2));
        for field in core::mem::take(&mut decl.fields) {
            let TypeExpr::Ring(elem, cap, span) = &field.type_expr else {
                expanded.push(field);
                continue;
            };
            let n = ring_capacity(cap)?;
            let [len, head, buf] = hidden_field_names(&field.name);
            let word = TypeExpr::Prim(PrimType::Word, *span);
            for (name, type_expr) in [
                (len, word.clone()),
                (head, word),
                (
                    buf,
                    TypeExpr::array_lit(Box::new((**elem).clone()), n, *span),
                ),
            ] {
                expanded.push(crate::ast::DataFieldDecl {
                    name,
                    type_expr,
                    initializer: None,
                    span: field.span,
                });
            }
        }
        decl.fields = expanded;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sp() -> Span {
        Span::default()
    }

    #[test]
    fn hidden_names_are_in_slot_order() {
        assert_eq!(
            hidden_field_names("r"),
            [
                String::from("r.len"),
                String::from("r.head"),
                String::from("r.buf")
            ]
        );
    }

    #[test]
    fn capacity_folds_total_const_expressions() {
        let e = ConstExpr::Bin(
            ConstBinOp::Add,
            Box::new(ConstExpr::Lit(2, sp())),
            Box::new(ConstExpr::Bin(
                ConstBinOp::Mul,
                Box::new(ConstExpr::Lit(3, sp())),
                Box::new(ConstExpr::Lit(4, sp())),
                sp(),
            )),
            sp(),
        );
        assert_eq!(ring_capacity(&e), Ok(14));
    }

    #[test]
    fn capacity_bounds_are_exact() {
        assert_eq!(ring_capacity(&ConstExpr::Lit(1, sp())), Ok(1));
        assert!(ring_capacity(&ConstExpr::Lit(0, sp())).is_err());
        assert!(ring_capacity(&ConstExpr::Lit(-1, sp())).is_err());
        assert_eq!(
            ring_capacity(&ConstExpr::Lit(RING_CAPACITY_MAX, sp())),
            Ok(RING_CAPACITY_MAX)
        );
        assert!(ring_capacity(&ConstExpr::Lit(RING_CAPACITY_MAX + 1, sp())).is_err());
        assert_eq!(RING_CAPACITY_MAX + 2, (1 << 24) - 1);
    }

    #[test]
    fn capacity_refuses_names_and_overflow() {
        assert!(ring_capacity(&ConstExpr::Param("n".into(), sp())).is_err());
        let over = ConstExpr::Bin(
            ConstBinOp::Mul,
            Box::new(ConstExpr::Lit(i64::MAX, sp())),
            Box::new(ConstExpr::Lit(2, sp())),
            sp(),
        );
        let err = ring_capacity(&over).unwrap_err();
        assert!(err.message.contains("overflows"), "{}", err.message);
    }

    #[test]
    fn element_set_is_the_five_scalars() {
        for p in [
            PrimType::Word,
            PrimType::Byte,
            PrimType::Bool,
            PrimType::Fixed(None),
            PrimType::Fixed(Some(16)),
            PrimType::Float,
        ] {
            assert!(element_admissible(&TypeExpr::Prim(p, sp())));
        }
        assert!(!element_admissible(&TypeExpr::Prim(PrimType::Text, sp())));
        assert!(!element_admissible(&TypeExpr::Unit(sp())));
        assert!(!element_admissible(&TypeExpr::Tuple(
            alloc::vec![TypeExpr::Prim(PrimType::Word, sp())],
            sp()
        )));
    }
}
