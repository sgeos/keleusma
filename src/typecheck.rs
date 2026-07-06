//! Static type checker for Keleusma source programs.
//!
//! Runs after parsing and before bytecode emission. Catches type errors
//! at compile time that would otherwise surface at runtime through
//! [`crate::vm::VmError::TypeError`].
//!
//! Coverage. The pass catches the following at compile time.
//!
//! - Function call argument count and argument types against parameter
//!   declarations.
//! - Function return expression type against declared return type.
//! - Let binding type against the value's type when annotation present.
//! - Arithmetic and comparison operations have type-compatible operands.
//! - Field access references defined fields on the operand type.
//! - Struct construction provides defined fields with the right types.
//! - Cast operations are between admissible types (i64 to f64 and back).
//! - Identifier references resolve to known locals or function names.
//! - Undefined function calls are rejected. Names declared in `use`
//!   declarations or names qualified with `::` are accepted as
//!   natives without signature checks because native signatures are
//!   not declared at compile time.
//! - Match arm patterns are structurally checked against the
//!   scrutinee's static type. Tuple arity, enum variant existence
//!   and payload arity, struct field name validity, and literal
//!   pattern type compatibility are all checked.
//! - Match arm exhaustiveness. Enum scrutinees must cover every
//!   variant or have a wildcard arm. Bool scrutinees must cover both
//!   true and false or have a wildcard. Unit scrutinees must cover
//!   `()` or have a wildcard. Other types require a wildcard.
//!
//! ## Hindley-Milner inference (B1)
//!
//! The pass uses Robinson-style unification through the `unify`
//! function and the `Subst` type. Inferred positions allocate fresh
//! type variables through the internal context. Unannotated let bindings,
//! unannotated function parameters, and recursive expression types
//! receive `Type::Var` placeholders that are resolved through
//! constraint solving as the pass walks the program.
//!
//! Without generic type parameters (B2), inference is monomorphic.
//! Each unannotated position has at most one resolved type once
//! constraints are solved. Future B2 work introduces generalization
//! and instantiation.
//!
//! Out of scope for this pass.
//!
//! - Generalization and instantiation (B2).
//! - Native function signatures (sound only with explicit
//!   `use ... : fn(...) -> ...` extensions, not yet supported).

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::ast::*;
use crate::token::Span;

/// Default `Fixed` fraction-bit count when the surface form is
/// `Fixed` without an explicit `<N>` argument and no target
/// descriptor is in scope. Matches the host 64-bit runtime's
/// Q31.32 format. The target-aware entry point
/// [`check_with_target`] resolves the default through the
/// supplied target's [`crate::target::Target::fixed_default_frac_bits`]
/// so cross-compilation to a 32-bit or 16-bit target produces
/// Q15.16 or Q7.8 respectively. The bare [`check`] entry point
/// falls back to this constant.
pub const DEFAULT_FIXED_FRAC_BITS: u8 = 32;

/// A resolved const dimension of an array or `Multiword` type. Concrete
/// (`Known`) in a non-generic context and after monomorphization; a
/// normalized const-expression string (`Sym`) inside a generic body
/// where it references a const parameter. Two `Sym`s unify by string
/// equality; a `Known` and a `Sym` unify only inside a generic body,
/// deferred to the mandatory post-monomorphization re-typecheck, which
/// sees only `Known` and is the real soundness gate (B40).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstDim {
    /// A concrete dimension.
    Known(i64),
    /// A symbolic dimension, the normalized const-expression string.
    Sym(String),
}

impl ConstDim {
    /// The concrete value, or `None` when symbolic.
    pub fn known(&self) -> Option<i64> {
        match self {
            ConstDim::Known(n) => Some(*n),
            ConstDim::Sym(_) => None,
        }
    }
}

impl core::fmt::Display for ConstDim {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ConstDim::Known(n) => write!(f, "{}", n),
            ConstDim::Sym(s) => write!(f, "{}", s),
        }
    }
}

/// Fully evaluate a const expression over literals; `None` if it
/// references a const parameter (so it is symbolic).
fn eval_const_lit(ce: &crate::ast::ConstExpr) -> Option<i64> {
    use crate::ast::{ConstBinOp, ConstExpr};
    match ce {
        ConstExpr::Lit(n, _) => Some(*n),
        ConstExpr::Param(_, _) => None,
        ConstExpr::Bin(op, l, r, _) => {
            let a = eval_const_lit(l)?;
            let b = eval_const_lit(r)?;
            Some(match op {
                ConstBinOp::Add => a.wrapping_add(b),
                ConstBinOp::Sub => a.wrapping_sub(b),
                ConstBinOp::Mul => a.wrapping_mul(b),
            })
        }
    }
}

/// Render a symbolic const expression in a canonical form so that
/// commutatively equivalent expressions compare equal. Fully-literal
/// subexpressions are folded; the operands of the commutative operators
/// `+` and `*` are ordered by their rendered form (so `n + 1` and
/// `1 + n` both render `(1 + n)`); `-` keeps its operand order. This is
/// a first-pass usability aid only: after monomorphization every const
/// dimension is a folded literal, so the post-monomorphization
/// re-typecheck, not this string comparison, is the soundness gate.
/// Associativity across nested additions or multiplications is not
/// normalized, so `(n + 1) + m` and `n + (1 + m)` still differ here and
/// defer to the re-typecheck (B40).
fn normalize_const_expr(ce: &crate::ast::ConstExpr) -> alloc::string::String {
    use crate::ast::{ConstBinOp, ConstExpr};
    if let Some(n) = eval_const_lit(ce) {
        return alloc::format!("{}", n);
    }
    match ce {
        ConstExpr::Lit(n, _) => alloc::format!("{}", n),
        ConstExpr::Param(name, _) => name.clone(),
        ConstExpr::Bin(op, l, r, _) => {
            let ls = normalize_const_expr(l);
            let rs = normalize_const_expr(r);
            match op {
                ConstBinOp::Add | ConstBinOp::Mul => {
                    let sym = if matches!(op, ConstBinOp::Add) {
                        "+"
                    } else {
                        "*"
                    };
                    let (a, b) = if ls <= rs { (ls, rs) } else { (rs, ls) };
                    alloc::format!("({} {} {})", a, sym, b)
                }
                ConstBinOp::Sub => alloc::format!("({} - {})", ls, rs),
            }
        }
    }
}

/// Resolve a const expression to a [`ConstDim`]: `Known` when it folds to
/// a literal, else `Sym` of its canonical rendered form (B40).
fn const_dim_from_expr(ce: &crate::ast::ConstExpr) -> ConstDim {
    match eval_const_lit(ce) {
        Some(n) => ConstDim::Known(n),
        None => ConstDim::Sym(normalize_const_expr(ce)),
    }
}

/// Whether two const dimensions may unify. Two `Known`s must be equal;
/// two `Sym`s must be string-equal; a `Known` and a `Sym` are accepted,
/// which arises only inside a generic body and is resolved by the
/// mandatory post-monomorphization re-typecheck, where both are `Known`
/// (B40).
fn const_dims_compatible(a: &ConstDim, b: &ConstDim) -> bool {
    match (a, b) {
        (ConstDim::Known(x), ConstDim::Known(y)) => x == y,
        (ConstDim::Sym(x), ConstDim::Sym(y)) => x == y,
        _ => true,
    }
}

/// A computed type. The internal representation is independent of the
/// `TypeExpr` AST node so the checker can reason about types without
/// surface-syntax detail.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Eight-bit unsigned integer. Range `[0, 255]`. Arithmetic
    /// uses wrapping `u8` semantics; conversions to and from
    /// `Word` go through the `as` cast.
    Byte,
    /// Target word size (signed). On the V0.1.x runtime this is
    /// 64-bit; narrower widths are reserved for future embedded
    /// targets.
    Word,
    /// Signed Q-format fixed-point with the given fraction-bit
    /// count. The default `Fixed` surface form resolves to the
    /// target-scaled value (32 on the host 64-bit runtime);
    /// `Fixed<N>` resolves to the literal N.
    Fixed(u8),
    /// Target floating-point width. IEEE 754 binary64 on the host;
    /// narrower widths are reserved for future embedded targets.
    Float,
    /// Boolean.
    Bool,
    /// Unit `()`.
    Unit,
    /// Static string.
    Str,
    /// Tuple of types.
    Tuple(Vec<Type>),
    /// Fixed-length array. The dimension is a [`ConstDim`]: a concrete
    /// length, or symbolic when it references a const parameter inside a
    /// generic body (B40).
    Array(Box<Type>, ConstDim),
    /// Fixed-width multi-word fixed-point, `Multiword<N, F>`, N words
    /// wide with F fractional bits, little-endian two's complement.
    /// F is zero for the big-integer case. Distinct nominal type; its
    /// runtime representation is a flat array of N words. N and F are
    /// [`ConstDim`]s: concrete post-monomorphization, possibly symbolic
    /// inside a generic body (B19, B40).
    Multiword(ConstDim, ConstDim),
    /// Option of a type.
    Option(Box<Type>),
    /// Named struct with optional generic type arguments. Empty
    /// `Vec<Type>` for non-generic structs.
    Struct(String, Vec<Type>),
    /// Named enum with optional generic type arguments. Empty
    /// `Vec<Type>` for non-generic enums.
    Enum(String, Vec<Type>),
    /// Distinct nominal type wrapping an underlying type. The
    /// bytecode representation matches the underlying; the wrapper
    /// exists only at the type-checker level. Two newtypes with
    /// different names are not assignable to one another even when
    /// their underlying types match.
    /// Newtype reference by name. The authoritative underlying
    /// type is stored in the checker's internal `Ctx::newtypes` map; the type variant
    /// itself carries only the nominal name because the unifier
    /// distinguishes newtypes by name alone.
    Newtype(String),
    /// Type with information-flow labels. The wrapped type is the
    /// underlying type; the label set is the set of user-defined
    /// labels the value carries. Empty label set is represented by
    /// the absence of the wrapper. Labels propagate through
    /// arithmetic by union, and assignment requires the source's
    /// label set to be a subset of the target's.
    Labelled(Box<Type>, BTreeSet<String>),
    /// Opaque type referenced by name.
    Opaque(String),
    /// Type variable for Hindley-Milner inference. Allocated by the
    /// checker for expressions whose type is constrained but not yet
    /// solved. Resolved through unification against the constraint
    /// set; a final pass applies the substitution and reports any
    /// unresolved variable as an inference failure. All
    /// unannotated positions produce a fresh `Type::Var` through
    /// the checker's internal `Ctx::fresh` allocator.
    Var(u32),
}

impl Type {
    /// Resolve a [`TypeExpr`] to a [`Type`] under a generic type
    /// parameter mapping and an explicit Fixed-default fraction-bit
    /// count.
    ///
    /// Names that match a key in `type_params` resolve to the mapped
    /// [`Type`], typically a `Type::Var` allocated at signature
    /// construction. Names that are not type parameters fall back to
    /// the existing struct/enum/opaque resolution.
    ///
    /// The `fixed_default_frac_bits` argument is the value substituted
    /// for `PrimType::Fixed(None)` (the surface form `Fixed` without
    /// `<N>`). The type checker reaches this method through
    /// [`Ctx::resolve_type`] and [`Ctx::resolve_type_with_params`],
    /// which forward the context's [`Ctx::fixed_default_frac_bits`].
    /// The target-aware entry point [`check_with_target`] populates
    /// that field from the supplied target; the bare [`check`] entry
    /// point falls back to [`DEFAULT_FIXED_FRAC_BITS`].
    fn from_expr_with_params_and_frac(
        expr: &TypeExpr,
        defined_types: &BTreeMap<String, TypeKind>,
        type_params: &BTreeMap<String, Type>,
        fixed_default_frac_bits: u8,
    ) -> Type {
        match expr {
            TypeExpr::Prim(p, _) => match p {
                PrimType::Byte => Type::Byte,
                PrimType::Word => Type::Word,
                PrimType::Fixed(maybe_n) => Type::Fixed(maybe_n.unwrap_or(fixed_default_frac_bits)),
                PrimType::Float => Type::Float,
                PrimType::Bool => Type::Bool,
                PrimType::Text => Type::Str,
            },
            TypeExpr::Unit(_) => Type::Unit,
            TypeExpr::Tuple(ts, _) => Type::Tuple(
                ts.iter()
                    .map(|t| {
                        Type::from_expr_with_params_and_frac(
                            t,
                            defined_types,
                            type_params,
                            fixed_default_frac_bits,
                        )
                    })
                    .collect(),
            ),
            TypeExpr::Array(elem, len, _) => Type::Array(
                Box::new(Type::from_expr_with_params_and_frac(
                    elem,
                    defined_types,
                    type_params,
                    fixed_default_frac_bits,
                )),
                const_dim_from_expr(len),
            ),
            TypeExpr::Multiword(words, frac, _) => {
                Type::Multiword(const_dim_from_expr(words), const_dim_from_expr(frac))
            }
            TypeExpr::Option(inner, _) => {
                Type::Option(Box::new(Type::from_expr_with_params_and_frac(
                    inner,
                    defined_types,
                    type_params,
                    fixed_default_frac_bits,
                )))
            }
            TypeExpr::Named(name, args, _, _) => {
                if let Some(t) = type_params.get(name) {
                    return t.clone();
                }
                let resolved_args: Vec<Type> = args
                    .iter()
                    .map(|a| {
                        Type::from_expr_with_params_and_frac(
                            a,
                            defined_types,
                            type_params,
                            fixed_default_frac_bits,
                        )
                    })
                    .collect();
                match defined_types.get(name) {
                    Some(TypeKind::Struct) => Type::Struct(name.clone(), resolved_args),
                    Some(TypeKind::Enum) => Type::Enum(name.clone(), resolved_args),
                    Some(TypeKind::Newtype) => {
                        // The newtype's underlying type lives in
                        // `Ctx::newtypes`; consult it at the use
                        // sites that actually need it (newtype
                        // construction, value extraction). The
                        // variant carries only the nominal name;
                        // equality and unification depend on the
                        // name, not on the underlying.
                        Type::Newtype(name.clone())
                    }
                    None => Type::Opaque(name.clone()),
                }
            }
            TypeExpr::Labelled(inner, labels, _) => {
                let inner_ty = Type::from_expr_with_params_and_frac(
                    inner,
                    defined_types,
                    type_params,
                    fixed_default_frac_bits,
                );
                let label_set: BTreeSet<String> = labels.iter().cloned().collect();
                if label_set.is_empty() {
                    inner_ty
                } else {
                    // Avoid nesting `Labelled` inside `Labelled`;
                    // union the labels and keep the underlying.
                    match inner_ty {
                        Type::Labelled(inner_inner, inner_labels) => {
                            let merged: BTreeSet<String> =
                                inner_labels.union(&label_set).cloned().collect();
                            Type::Labelled(inner_inner, merged)
                        }
                        other => Type::Labelled(Box::new(other), label_set),
                    }
                }
            }
            TypeExpr::NegativeLabelled(inner, _, _) => {
                // Negative labels do not propagate as a labelled type
                // through the lattice. They are a boundary clause
                // enforced by the type checker at parameter, return,
                // resume, and yield positions. At this conversion
                // site we keep only the underlying type; the
                // negative-label list is extracted by the
                // function-signature collection pass and stored
                // alongside the FnSig's params/return for later
                // boundary checks. Out-of-position appearances of
                // `NegativeLabelled` are caught by a separate
                // validation walk.
                Type::from_expr_with_params_and_frac(
                    inner,
                    defined_types,
                    type_params,
                    fixed_default_frac_bits,
                )
            }
        }
    }

    /// Human-readable type name for diagnostics.
    pub fn display(&self) -> String {
        match self {
            Type::Byte => "Byte".to_string(),
            Type::Word => "Word".to_string(),
            Type::Fixed(n) => alloc::format!("Fixed<{}>", n),
            Type::Float => "Float".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Unit => "()".to_string(),
            Type::Str => "Text".to_string(),
            Type::Tuple(ts) => {
                let inner: Vec<String> = ts.iter().map(|t| t.display()).collect();
                format!("({})", inner.join(", "))
            }
            Type::Array(elem, n) => format!("[{}; {}]", elem.display(), n),
            Type::Multiword(n, f) => {
                if f.known() == Some(0) {
                    format!("Multiword<{}>", n)
                } else {
                    format!("Multiword<{}, {}>", n, f)
                }
            }
            Type::Option(inner) => format!("Option<{}>", inner.display()),
            Type::Struct(name, args) | Type::Enum(name, args) => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let inner: Vec<String> = args.iter().map(|t| t.display()).collect();
                    format!("{}<{}>", name, inner.join(", "))
                }
            }
            Type::Newtype(name) => name.clone(),
            Type::Labelled(inner, labels) => {
                let mut labels_sorted: Vec<&str> = labels.iter().map(|s| s.as_str()).collect();
                labels_sorted.sort();
                if labels_sorted.len() == 1 {
                    format!("{}@{}", inner.display(), labels_sorted[0])
                } else {
                    format!("{}@{{{}}}", inner.display(), labels_sorted.join(", "))
                }
            }
            Type::Opaque(name) => name.clone(),
            Type::Var(n) => format!("?T{}", n),
        }
    }

    /// Whether the type contains the given type variable. Used by the
    /// occurs check during unification to prevent infinite types.
    pub fn occurs(&self, var: u32) -> bool {
        match self {
            Type::Var(v) => *v == var,
            Type::Tuple(items) => items.iter().any(|t| t.occurs(var)),
            Type::Array(elem, _) => elem.occurs(var),
            Type::Option(inner) => inner.occurs(var),
            Type::Struct(_, args) | Type::Enum(_, args) => args.iter().any(|t| t.occurs(var)),
            Type::Newtype(_) => false,
            Type::Labelled(inner, _) => inner.occurs(var),
            _ => false,
        }
    }

    /// Apply a substitution recursively, replacing type variables with
    /// their resolved types where the substitution provides one.
    pub fn apply(&self, subst: &Subst) -> Type {
        match self {
            Type::Var(v) => match subst.get(*v) {
                Some(t) => t.apply(subst),
                None => self.clone(),
            },
            Type::Tuple(items) => Type::Tuple(items.iter().map(|t| t.apply(subst)).collect()),
            Type::Array(elem, n) => Type::Array(Box::new(elem.apply(subst)), n.clone()),
            Type::Option(inner) => Type::Option(Box::new(inner.apply(subst))),
            Type::Struct(name, args) => {
                Type::Struct(name.clone(), args.iter().map(|t| t.apply(subst)).collect())
            }
            Type::Enum(name, args) => {
                Type::Enum(name.clone(), args.iter().map(|t| t.apply(subst)).collect())
            }
            Type::Newtype(name) => Type::Newtype(name.clone()),
            Type::Labelled(inner, labels) => {
                Type::Labelled(Box::new(inner.apply(subst)), labels.clone())
            }
            other => other.clone(),
        }
    }
}

/// A substitution from type variables to types.
///
/// Maps numeric type variable identifiers to the types they have been
/// resolved to. Composition is implicit through repeated application:
/// `a.apply(s).apply(s)` is equivalent to `a.apply(s)` because `apply`
/// is recursive. Use [`Subst::insert`] to extend the substitution
/// during unification.
#[derive(Debug, Clone, Default)]
pub struct Subst {
    map: BTreeMap<u32, Type>,
}

impl Subst {
    /// Construct an empty substitution.
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up the resolved type for a type variable.
    pub fn get(&self, var: u32) -> Option<&Type> {
        self.map.get(&var)
    }

    /// Bind a type variable to a type. Caller must have run the occurs
    /// check before calling this.
    pub fn insert(&mut self, var: u32, ty: Type) {
        self.map.insert(var, ty);
    }

    /// Number of bindings in the substitution.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the substitution is empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// A unification failure during constraint solving.
#[derive(Debug, Clone, PartialEq)]
pub enum UnifyError {
    /// Two types could not be unified because they have different
    /// outer constructors or carry incompatible payloads.
    Mismatch {
        /// Left-hand side of the failed unification.
        left: Type,
        /// Right-hand side of the failed unification.
        right: Type,
    },
    /// A type variable would refer to itself through a chain of
    /// constraints, producing an infinite type.
    OccursCheck {
        /// Index of the offending type variable.
        var: u32,
        /// Type that recursively references `var`.
        ty: Type,
    },
    /// Two arrays have different declared lengths.
    ArrayLengthMismatch {
        /// Left array's declared length (rendered; may be symbolic).
        left: String,
        /// Right array's declared length (rendered; may be symbolic).
        right: String,
    },
    /// Two tuples have different arity.
    TupleArityMismatch {
        /// Left tuple's arity.
        left: usize,
        /// Right tuple's arity.
        right: usize,
    },
}

/// Unify two types under an existing substitution.
///
/// Robinson's algorithm. On success, extends the substitution in place
/// so that applying it to either input produces the same type. On
/// failure, returns a [`UnifyError`] describing the structural reason
/// the two types are incompatible.
///
/// The implementation handles the common cases inline.
///
/// - Two identical primitive types unify trivially.
/// - A type variable unifies with any type after the occurs check.
/// - Two tuples unify if they have the same arity and pairwise unify.
/// - Two arrays unify if they have the same length and their element
///   types unify.
/// - Two options unify if their inner types unify.
/// - Named types unify only when their names match.
pub fn unify(a: &Type, b: &Type, subst: &mut Subst) -> Result<(), UnifyError> {
    // Strip information-flow labels at the unifier entry. The
    // labels are not part of structural compatibility; per-
    // position label flow is checked by `types_compatible` /
    // `flow_admissible` at the higher level. Stripping at every
    // unify call ensures nested labels (inside tuples, arrays,
    // options) are also seen through during recursive
    // unification.
    let mut a = a.apply(subst);
    let mut b = b.apply(subst);
    while let Type::Labelled(inner, _) = a {
        a = *inner;
    }
    while let Type::Labelled(inner, _) = b {
        b = *inner;
    }
    match (a, b) {
        (Type::Fixed(a_n), Type::Fixed(b_n)) => {
            if a_n == b_n {
                Ok(())
            } else {
                Err(UnifyError::Mismatch {
                    left: Type::Fixed(a_n),
                    right: Type::Fixed(b_n),
                })
            }
        }
        (Type::Multiword(a_n, a_f), Type::Multiword(b_n, b_f)) => {
            if const_dims_compatible(&a_n, &b_n) && const_dims_compatible(&a_f, &b_f) {
                Ok(())
            } else {
                Err(UnifyError::Mismatch {
                    left: Type::Multiword(a_n, a_f),
                    right: Type::Multiword(b_n, b_f),
                })
            }
        }
        (Type::Byte, Type::Byte)
        | (Type::Word, Type::Word)
        | (Type::Float, Type::Float)
        | (Type::Bool, Type::Bool)
        | (Type::Unit, Type::Unit)
        | (Type::Str, Type::Str) => Ok(()),
        (Type::Var(v), other) | (other, Type::Var(v)) => {
            if let Type::Var(w) = other
                && v == w
            {
                return Ok(());
            }
            if other.occurs(v) {
                return Err(UnifyError::OccursCheck { var: v, ty: other });
            }
            subst.insert(v, other);
            Ok(())
        }
        (Type::Tuple(ls), Type::Tuple(rs)) => {
            if ls.len() != rs.len() {
                return Err(UnifyError::TupleArityMismatch {
                    left: ls.len(),
                    right: rs.len(),
                });
            }
            for (l, r) in ls.iter().zip(rs.iter()) {
                unify(l, r, subst)?;
            }
            Ok(())
        }
        (Type::Array(le, ln), Type::Array(re, rn)) => {
            if !const_dims_compatible(&ln, &rn) {
                return Err(UnifyError::ArrayLengthMismatch {
                    left: ln.to_string(),
                    right: rn.to_string(),
                });
            }
            unify(&le, &re, subst)
        }
        (Type::Option(li), Type::Option(ri)) => unify(&li, &ri, subst),
        (Type::Struct(ln, la), Type::Struct(rn, ra)) | (Type::Enum(ln, la), Type::Enum(rn, ra))
            if ln == rn && la.len() == ra.len() =>
        {
            for (l, r) in la.iter().zip(ra.iter()) {
                unify(l, r, subst)?;
            }
            Ok(())
        }
        (Type::Opaque(ln), Type::Opaque(rn)) if ln == rn => Ok(()),
        // Newtypes unify by name alone. The stored underlying is a
        // placeholder at use sites (the resolver does not carry the
        // newtype map); the authoritative underlying lives in
        // `Ctx::newtypes` and is consulted at construction or cast
        // sites. Two newtypes with matching names are equivalent
        // regardless of their stored underlying.
        (Type::Newtype(ln), Type::Newtype(rn)) if ln == rn => Ok(()),
        (l, r) => Err(UnifyError::Mismatch { left: l, right: r }),
    }
}

/// Allocator for fresh type variables.
///
/// Held by the typing context across the inference of a function or
/// module. Allocates a fresh `Type::Var` on each call.
#[derive(Debug, Default)]
pub struct VarGen {
    next: u32,
}

impl VarGen {
    /// Allocate a fresh type variable.
    pub fn fresh(&mut self) -> Type {
        let v = self.next;
        self.next += 1;
        Type::Var(v)
    }

    /// The number of variables allocated so far.
    pub fn count(&self) -> u32 {
        self.next
    }
}

/// What kind of type a name refers to.
#[derive(Debug, Clone, Copy)]
enum TypeKind {
    Struct,
    Enum,
    /// Distinct nominal type wrapping an underlying primitive or
    /// composite. The underlying type is stored separately in
    /// `Ctx::newtypes` so the resolver can recover it at
    /// construction sites and at runtime cast points.
    Newtype,
}

/// Return the canonical head name of a type, used as the implementing
/// type's identity in the trait `impls` map. Primitive types have
/// stable lower-case names; named types use their declaration name.
/// Type variables are treated as matching any implementation;
/// the caller decides how to handle the underspecified case.
/// Remove the outermost `Type::Labelled` wrapper, returning the
/// underlying type. The labels are dropped; callers that need
/// to consult them must inspect the original type before
/// stripping. This is the standard preparation step for code
/// paths that dispatch on the underlying type's structure
/// without regard to information-flow markers.
fn strip_labels(t: Type) -> Type {
    match t {
        Type::Labelled(inner, _) => *inner,
        other => other,
    }
}

/// Extract the top-level negative-label set from a [`TypeExpr`].
/// Returns the set of labels declared via `T@!Label` or
/// `T@{!N1, !N2}` at the outermost type position. Returns an
/// empty set when the type expression has no top-level
/// [`TypeExpr::NegativeLabelled`] wrapper.
///
/// Used at function-signature collection time to record per-
/// parameter and per-return negative-label sets on the [`FnSig`].
/// Negative labels at nested positions (inside `Tuple`, `Array`,
/// `Option`, etc.) are not extracted here; they are detected and
/// rejected by [`validate_no_nested_negative_labels`].
fn top_level_negative_labels(t: &TypeExpr) -> BTreeSet<String> {
    match t {
        TypeExpr::NegativeLabelled(_, labels, _) => labels.iter().cloned().collect(),
        _ => BTreeSet::new(),
    }
}

/// Boundary check at a call site or return position: the
/// argument or returned value must not carry any of the
/// declared negative labels on the parameter or return type.
/// The check intersects the value's positive label set with the
/// declared negatives; a non-empty intersection rejects the
/// boundary crossing with a diagnostic naming the offending
/// labels.
fn check_negative_labels_against_arg(
    callee_name: &str,
    param_index: usize,
    arg_ty: &Type,
    param_negative_labels: &[BTreeSet<String>],
    span: Span,
) -> Result<(), TypeError> {
    let negatives = match param_negative_labels.get(param_index) {
        Some(set) if !set.is_empty() => set,
        _ => return Ok(()),
    };
    let arg_labels = labels_of(arg_ty);
    let intersection: BTreeSet<String> = arg_labels.intersection(negatives).cloned().collect();
    if !intersection.is_empty() {
        let mut sorted: Vec<&String> = intersection.iter().collect();
        sorted.sort();
        let names: Vec<String> = sorted.iter().map(|s| s.to_string()).collect();
        return Err(TypeError::new(
            alloc::format!(
                "argument {} to `{}` carries the label `{}` which the parameter's `!`-prefix declaration forbids",
                param_index + 1,
                callee_name,
                names.join(", "),
            ),
            span,
        ));
    }
    Ok(())
}

/// Boundary check at a script-side write to a data field. The
/// value being assigned must not carry any of the labels in the
/// field's declared negative-label set. Mirrors
/// [`check_negative_labels_against_arg`] for the data-block
/// channel: a `shared` data field is the host-script boundary, a
/// `private` data field is the yield-resume boundary; both
/// admit a negative-label clause expressing "values flowing into
/// this storage must not carry these labels", and the check fires
/// at every assignment.
fn check_negative_labels_against_data_write(
    data_name: &str,
    field: &str,
    value_ty: &Type,
    field_negative_labels: &BTreeSet<String>,
    span: Span,
) -> Result<(), TypeError> {
    if field_negative_labels.is_empty() {
        return Ok(());
    }
    let value_labels = labels_of(value_ty);
    let intersection: BTreeSet<String> = value_labels
        .intersection(field_negative_labels)
        .cloned()
        .collect();
    if !intersection.is_empty() {
        let mut sorted: Vec<&String> = intersection.iter().collect();
        sorted.sort();
        let names: Vec<String> = sorted.iter().map(|s| s.to_string()).collect();
        return Err(TypeError::new(
            alloc::format!(
                "assignment to `{}.{}` carries the label `{}` which the field's `!`-prefix declaration forbids",
                data_name,
                field,
                names.join(", "),
            ),
            span,
        ));
    }
    Ok(())
}

/// Boundary check at a return statement or yield expression: the
/// value flowing out must not carry any of the function's
/// declared return-type negative labels. Same semantics as
/// [`check_negative_labels_against_arg`] applied at the outbound
/// boundary.
fn check_negative_labels_against_return(
    context_description: &str,
    value_ty: &Type,
    return_negative_labels: &BTreeSet<String>,
    span: Span,
) -> Result<(), TypeError> {
    if return_negative_labels.is_empty() {
        return Ok(());
    }
    let value_labels = labels_of(value_ty);
    let intersection: BTreeSet<String> = value_labels
        .intersection(return_negative_labels)
        .cloned()
        .collect();
    if !intersection.is_empty() {
        let mut sorted: Vec<&String> = intersection.iter().collect();
        sorted.sort();
        let names: Vec<String> = sorted.iter().map(|s| s.to_string()).collect();
        return Err(TypeError::new(
            alloc::format!(
                "{} carries the label `{}` which the function's return-type `!`-prefix declaration forbids",
                context_description,
                names.join(", "),
            ),
            span,
        ));
    }
    Ok(())
}

/// Walk a [`TypeExpr`] tree and produce a diagnostic for any
/// [`TypeExpr::NegativeLabelled`] wrapper found at a nested
/// position. The caller supplies the outermost position so the
/// helper can distinguish admissible top-level negatives at
/// parameter and return types from inadmissible nested
/// occurrences inside tuples, arrays, options, and named
/// composites.
///
/// V0.2.0 admits negative labels only at top-level parameter and
/// return type positions. Every other type position rejects them
/// with a diagnostic naming the offending span.
fn validate_no_nested_negative_labels(
    t: &TypeExpr,
    at_top_level_allowed_position: bool,
) -> Result<(), TypeError> {
    match t {
        TypeExpr::NegativeLabelled(inner, _, span) => {
            if !at_top_level_allowed_position {
                return Err(TypeError {
                    message: String::from(
                        "negative information-flow labels (`!Label`) are admissible only at the top level of a boundary position: function parameter or return type, or data field type. Nested positions inside tuples, arrays, options, or other composite types reject them",
                    ),
                    span: *span,
                });
            }
            validate_no_nested_negative_labels(inner, false)
        }
        TypeExpr::Labelled(inner, _, _) => validate_no_nested_negative_labels(inner, false),
        TypeExpr::Multiword(_, _, _) => Ok(()),
        TypeExpr::Tuple(items, _) => {
            for item in items {
                validate_no_nested_negative_labels(item, false)?;
            }
            Ok(())
        }
        TypeExpr::Array(elem, _, _) | TypeExpr::Option(elem, _) => {
            validate_no_nested_negative_labels(elem, false)
        }
        TypeExpr::Named(_, args, _, _) => {
            for arg in args {
                validate_no_nested_negative_labels(arg, false)?;
            }
            Ok(())
        }
        TypeExpr::Prim(_, _) | TypeExpr::Unit(_) => Ok(()),
    }
}

/// Return the label set carried by a type. The empty set is
/// returned for types without a `Labelled` wrapper.
fn labels_of(t: &Type) -> BTreeSet<String> {
    match t {
        Type::Labelled(_, labels) => labels.clone(),
        _ => BTreeSet::new(),
    }
}

/// Wrap `t` in `Type::Labelled` with the supplied label set when
/// the set is non-empty; return `t` unchanged when the set is
/// empty. Used by arithmetic and branching propagation to
/// re-apply the union of operand labels to the structural
/// result without introducing redundant wrappers around pure
/// values.
fn apply_labels(t: Type, labels: &BTreeSet<String>) -> Type {
    if labels.is_empty() {
        t
    } else {
        match t {
            Type::Labelled(inner, existing) => {
                let union: BTreeSet<String> = existing.union(labels).cloned().collect();
                Type::Labelled(inner, union)
            }
            other => Type::Labelled(Box::new(other), labels.clone()),
        }
    }
}

fn type_head_name(t: &Type) -> Option<String> {
    use alloc::string::ToString;
    match t {
        Type::Byte => Some("Byte".to_string()),
        Type::Word => Some("Word".to_string()),
        Type::Fixed(_) => Some("Fixed".to_string()),
        Type::Float => Some("Float".to_string()),
        Type::Bool => Some("bool".to_string()),
        Type::Unit => Some("()".to_string()),
        Type::Str => Some("Text".to_string()),
        Type::Tuple(_) => Some("tuple".to_string()),
        Type::Array(_, _) => Some("array".to_string()),
        Type::Multiword(_, _) => Some("Multiword".to_string()),
        Type::Option(_) => Some("Option".to_string()),
        Type::Struct(name, _) | Type::Enum(name, _) | Type::Opaque(name) => Some(name.clone()),
        Type::Newtype(name) => Some(name.clone()),
        Type::Labelled(inner, _) => type_head_name(inner),
        Type::Var(_) => None,
    }
}

/// Function signature derived from an AST function definition.
///
/// Generic functions record their type parameters, the trait bounds
/// declared on each parameter, and the `Type::Var` identifiers
/// assigned to each one at signature construction time. Call-site
/// instantiation generates a fresh substitution from the recorded
/// variables to fresh per-call variables and applies it to the
/// parameter and return types before unifying against actual
/// arguments. After unification, each bounded parameter is checked
/// against the trait `impls` registry.
#[derive(Debug, Clone)]
struct FnSig {
    /// Generic type parameter names in declaration order. Empty for
    /// non-generic functions.
    type_params: Vec<String>,
    /// Generic const parameter names in declaration order. Empty for
    /// non-const-generic functions. A call site must supply exactly this
    /// many const arguments through a turbofish (B40).
    const_params: Vec<String>,
    /// `Type::Var` allocated for each type parameter at signature
    /// construction. Indexed in the same order as `type_params`. Used
    /// for monomorphic checking of the function body and as the
    /// abstract variables that call-site instantiation substitutes.
    type_param_vars: Vec<Type>,
    /// Trait bounds declared on each type parameter, indexed in the
    /// same order as `type_params`. Each inner `Vec<String>` lists
    /// the trait names the parameter must satisfy. Empty for
    /// unconstrained parameters.
    type_param_bounds: Vec<Vec<String>>,
    params: Vec<Type>,
    return_type: Type,
    /// Negative information-flow labels at each parameter type,
    /// indexed in the same order as [`Self::params`]. Each inner
    /// `BTreeSet<String>` carries the labels declared via
    /// `T@!Label` or `T@{!N1, !N2}` on that parameter. Empty set
    /// when the parameter type declares no negatives. The type
    /// checker rejects a call site whose argument's positive
    /// labels intersect a parameter's negative-label set.
    param_negative_labels: Vec<BTreeSet<String>>,
    /// Negative information-flow labels at the return type. Same
    /// semantics as [`Self::param_negative_labels`] applied at the
    /// return boundary: every `return expr` and `yield expr` in
    /// the function body whose value's positive labels intersect
    /// this set is rejected.
    return_negative_labels: BTreeSet<String>,
}

/// A type-check error with source location.
#[derive(Debug, Clone)]
pub struct TypeError {
    /// Human-readable diagnostic message.
    pub message: String,
    /// Source span of the offending construct.
    pub span: Span,
}

impl TypeError {
    fn new(message: String, span: Span) -> Self {
        Self { message, span }
    }
}

/// The typing context tracks declarations, the local scope chain, and
/// the Hindley-Milner inference state (fresh variable generator and
/// active substitution).
struct Ctx {
    types: BTreeMap<String, TypeKind>,
    structs: BTreeMap<String, BTreeMap<String, Type>>,
    /// Abstract `Type::Var` ids assigned to each generic struct's
    /// type parameters in declaration order. Empty vector for
    /// non-generic structs. Used at struct construction and field
    /// access sites to substitute the per-instance type arguments.
    struct_type_param_vars: BTreeMap<String, Vec<Type>>,
    enums: BTreeMap<String, BTreeMap<String, Vec<Type>>>,
    /// Abstract `Type::Var` ids assigned to each generic enum's type
    /// parameters in declaration order.
    enum_type_param_vars: BTreeMap<String, Vec<Type>>,
    /// Set of trait names declared in the program.
    traits: BTreeMap<String, Vec<TraitMethodSig>>,
    /// Map from trait name to the set of types that implement it. Each
    /// implementing type is recorded as the head of its `Type::*`
    /// representation: `i64` for `Type::Word`, `Pair` for
    /// `Type::Struct("Pair", _)`, and so on.
    impls: BTreeMap<String, BTreeSet<String>>,
    functions: BTreeMap<String, FnSig>,
    /// Newtype declarations keyed by name. Stores the resolved
    /// underlying type for use at construction sites (where the
    /// argument must match the underlying) and at extraction sites
    /// (where the value's runtime representation is the underlying's).
    newtypes: BTreeMap<String, Type>,
    /// Names of newtypes that carry a refinement predicate. A
    /// refined newtype's construction is partial (the predicate may
    /// reject the underlying value), so the newtype-construction
    /// construct's `invalid_newtype` arm is admissible only for these
    /// (B35 P5). A non-refined newtype's construction is total.
    refined_newtypes: BTreeSet<String>,
    /// Newtype maximum-saturation contracts. When a checked-
    /// overflow construct's expected output type is a newtype
    /// listed here, the `saturate_max` keyword inside an arm body
    /// resolves to the stored value rather than the underlying
    /// type's `MAX`. Populated from `newtype Name = T with
    /// saturate_max = N;` declarations.
    newtype_saturate_max: BTreeMap<String, i64>,
    /// Newtype minimum-saturation contracts. Same semantics for
    /// `saturate_min`.
    newtype_saturate_min: BTreeMap<String, i64>,
    /// Bidirectional-inference stack of expected types. Sites
    /// that know the type of an upcoming expression position
    /// push the expected type before checking the expression and
    /// pop afterwards. Context-sensitive resolution sites
    /// (currently `Expr::SaturateMax` and `Expr::SaturateMin`)
    /// consult the top of the stack to determine the type the
    /// expression's value should fit. When the stack is empty,
    /// resolution falls back to the default (Word).
    expected_type_stack: Vec<Type>,
    /// Native function names imported via `use` declarations. Calls
    /// to these names are accepted with any argument types when the
    /// `use` declaration does not carry a signature; declarations
    /// that do carry a signature also populate [`Self::native_signatures`]
    /// and are checked accordingly.
    natives: BTreeSet<String>,
    /// Declared native signatures, keyed by the fully qualified
    /// native name as it would appear in [`Self::natives`]. Populated
    /// at signature-collection time from `use path::name(T, ...) -> R`
    /// declarations. Call sites that find a match enforce the
    /// declared parameter arity, parameter types, and return type.
    /// Native calls without a declared signature continue to fall
    /// through the permissive path that accepts any argument types
    /// and assigns a fresh type variable to the result.
    native_signatures: BTreeMap<String, FnSig>,
    /// Data block field types, keyed by data name then field name.
    data: BTreeMap<String, BTreeMap<String, Type>>,
    /// Per-field negative-label set on data block fields, keyed by
    /// data name then field name. Populated from the field's
    /// `TypeExpr::NegativeLabelled` wrapper at data-decl-pass time.
    /// A field with no negative-label wrapper carries an empty set.
    /// Consulted at every script-side write to the field (assignment
    /// or indexed assignment): the source value's positive labels
    /// must be disjoint from the field's negative set, mirroring the
    /// existing parameter and return negative-label discipline.
    /// Script-side reads return the inner type with no labels; the
    /// negative wrapper does not propagate through the value lattice.
    data_negative_labels: BTreeMap<String, BTreeMap<String, BTreeSet<String>>>,
    /// Stack of local variable scopes. Inner scopes shadow outer.
    locals: Vec<BTreeMap<String, Type>>,
    /// Return type of the function currently being checked.
    current_return: Option<Type>,
    /// Return-side negative-label set of the function currently
    /// being checked. Populated by `check_function` from the
    /// active function's `FnSig::return_negative_labels` and
    /// consulted by the body's tail-return check and by every
    /// `Expr::Yield` visited inside the body. Empty between
    /// function checks.
    current_return_negative_labels: BTreeSet<String>,
    /// Const parameter names of the function currently being checked.
    /// Populated by `check_function`; consulted so a const argument that
    /// references a name is confirmed to be a const parameter rather than
    /// a runtime local (a const argument must be a compile-time constant).
    /// Empty between function checks (B40).
    current_const_params: BTreeSet<String>,
    /// Fresh type variable allocator for the Hindley-Milner pipeline.
    vargen: VarGen,
    /// Active substitution accumulating constraints solved so far.
    subst: Subst,
    /// Fraction-bit count substituted for the surface form `Fixed`
    /// (no explicit `<N>` argument). Defaults to
    /// [`DEFAULT_FIXED_FRAC_BITS`]; the target-aware entry point
    /// [`check_with_target`] overrides it with the supplied target's
    /// [`crate::target::Target::fixed_default_frac_bits`] before
    /// running the passes.
    fixed_default_frac_bits: u8,
    /// When true, the per-function expression-type recording pass is active
    /// (B28 P3 item 5). Set by the recording entry point used for the
    /// post-monomorphization check; the generic pre-monomorphization check
    /// leaves it false.
    record_types: bool,
    /// Per-function buffer of resolved expression types, keyed by span and
    /// reset at each function. A span that receives two different concrete
    /// types is recorded in `fn_type_conflicts` and omitted, preserving the
    /// accurate-or-None guarantee.
    current_fn_types: BTreeMap<crate::token::Span, TypeExpr>,
    /// Spans in the current function that received conflicting concrete types
    /// and are therefore excluded from the authoritative table.
    fn_type_conflicts: BTreeSet<crate::token::Span>,
    /// Accumulated authoritative tables, keyed by function name. Moved into
    /// `Program::fn_expr_types` at the end of the recording check.
    fn_tables: BTreeMap<String, BTreeMap<crate::token::Span, TypeExpr>>,
}

impl Ctx {
    fn new() -> Self {
        Self {
            types: BTreeMap::new(),
            structs: BTreeMap::new(),
            struct_type_param_vars: BTreeMap::new(),
            enums: BTreeMap::new(),
            enum_type_param_vars: BTreeMap::new(),
            traits: BTreeMap::new(),
            impls: BTreeMap::new(),
            functions: BTreeMap::new(),
            natives: BTreeSet::new(),
            native_signatures: BTreeMap::new(),
            newtypes: BTreeMap::new(),
            refined_newtypes: BTreeSet::new(),
            newtype_saturate_max: BTreeMap::new(),
            newtype_saturate_min: BTreeMap::new(),
            expected_type_stack: Vec::new(),
            data: BTreeMap::new(),
            data_negative_labels: BTreeMap::new(),
            locals: Vec::new(),
            current_return: None,
            current_return_negative_labels: BTreeSet::new(),
            current_const_params: BTreeSet::new(),
            vargen: VarGen::default(),
            subst: Subst::new(),
            fixed_default_frac_bits: DEFAULT_FIXED_FRAC_BITS,
            record_types: false,
            current_fn_types: BTreeMap::new(),
            fn_type_conflicts: BTreeSet::new(),
            fn_tables: BTreeMap::new(),
        }
    }

    /// Resolve a [`TypeExpr`] to a [`Type`] using the context's
    /// type-kind table and its target-resolved Fixed default. Use
    /// this rather than [`Type::from_expr`] from inside the type
    /// checker so the default carries through to cross-compilation
    /// targets.
    fn resolve_type(&self, expr: &TypeExpr) -> Type {
        Type::from_expr_with_params_and_frac(
            expr,
            &self.types,
            &BTreeMap::new(),
            self.fixed_default_frac_bits,
        )
    }

    /// Generic-parameter-aware variant of [`Self::resolve_type`].
    /// Equivalent to [`Type::from_expr_with_params`] except the
    /// Fixed default comes from the context.
    fn resolve_type_with_params(
        &self,
        expr: &TypeExpr,
        type_params: &BTreeMap<String, Type>,
    ) -> Type {
        Type::from_expr_with_params_and_frac(
            expr,
            &self.types,
            type_params,
            self.fixed_default_frac_bits,
        )
    }

    /// Push an expected type onto the bidirectional-inference
    /// stack. The caller must pair this with a matching
    /// [`Self::pop_expected`] when leaving the position.
    fn push_expected(&mut self, ty: Type) {
        self.expected_type_stack.push(ty);
    }

    fn pop_expected(&mut self) {
        self.expected_type_stack.pop();
    }

    /// Top of the expected-type stack, with the current
    /// substitution applied. Returns `None` when the stack is
    /// empty (the position has no known expected type and
    /// resolution should fall back to defaults).
    fn expected_type(&self) -> Option<Type> {
        self.expected_type_stack
            .last()
            .map(|t| t.apply(&self.subst))
    }

    fn push_scope(&mut self) {
        self.locals.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        self.locals.pop();
    }

    fn add_local(&mut self, name: String, ty: Type) {
        if let Some(scope) = self.locals.last_mut() {
            scope.insert(name, ty);
        }
    }

    fn lookup_local(&self, name: &str) -> Option<&Type> {
        for scope in self.locals.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Allocate a fresh type variable for an inferred position.
    fn fresh(&mut self) -> Type {
        self.vargen.fresh()
    }
}

/// Build a fresh per-instantiation substitution for a list of
/// abstract `Type::Var` ids.
///
/// For each abstract variable in `abstract_vars`, allocate a fresh
/// `Type::Var` and bind the abstract id to the fresh variable in the
/// returned substitution. Returns the substitution and the ordered
/// list of fresh variables suitable for storing as the per-instance
/// type arguments on a `Type::Struct` or `Type::Enum`.
fn build_instance_subst(ctx: &mut Ctx, abstract_vars: &[Type]) -> (Subst, Vec<Type>) {
    let mut inst = Subst::new();
    let mut fresh_args: Vec<Type> = Vec::with_capacity(abstract_vars.len());
    for var in abstract_vars {
        if let Type::Var(v) = var {
            let fresh = ctx.vargen.fresh();
            inst.insert(*v, fresh.clone());
            fresh_args.push(fresh);
        }
    }
    (inst, fresh_args)
}

/// Instantiate a generic function signature with fresh per-call type
/// variables.
///
/// For each abstract type parameter variable in the signature,
/// allocate a fresh `Type::Var` and build a substitution mapping the
/// abstract variable to the fresh one. Apply this substitution to the
/// parameter and return types before unification with the call's
/// actual argument types. The result includes the instantiated
/// parameter and return types, and the per-call fresh variables in
/// the same order as `sig.type_param_vars`. The fresh variables are
/// retained so the caller can resolve them through the active
/// substitution after unification, which is how trait bound checks
/// recover the concrete argument type for each type parameter.
fn instantiate_sig(ctx: &mut Ctx, sig: &FnSig) -> (Vec<Type>, Type, Vec<Type>) {
    if sig.type_params.is_empty() {
        return (sig.params.clone(), sig.return_type.clone(), Vec::new());
    }
    let mut inst = Subst::new();
    let mut fresh_vars: Vec<Type> = Vec::with_capacity(sig.type_param_vars.len());
    for var in &sig.type_param_vars {
        if let Type::Var(v) = var {
            let fresh = ctx.vargen.fresh();
            inst.insert(*v, fresh.clone());
            fresh_vars.push(fresh);
        }
    }
    let params: Vec<Type> = sig.params.iter().map(|t| t.apply(&inst)).collect();
    let return_type = sig.return_type.apply(&inst);
    (params, return_type, fresh_vars)
}

/// Check that two types unify under the current substitution.
///
/// Records the unification in the context's substitution. Returns
/// false if the unification fails. The caller is responsible for
/// converting that into a [`TypeError`] with an appropriate message.
///
/// Type variables are routed through `unify` so the constraint is
/// recorded in the substitution; distinct generic instantiations
/// thereby fail to unify when their inferred types diverge.
fn types_compatible(ctx: &mut Ctx, a: &Type, b: &Type) -> bool {
    // Information-flow check is recursive: per-position label
    // subset rule applies at every composite layer. See
    // `flow_admissible`.
    if !flow_admissible(a, b) {
        return false;
    }
    // Structural unification. `unify` strips outer labels at
    // entry so nested labels do not interfere with the
    // structural match.
    unify(a, b, &mut ctx.subst).is_ok()
}

/// Recursive information-flow admissibility check. Returns true
/// when the source's labels at every position are a subset of
/// the target's labels at the corresponding position. Composite
/// structures (tuples, arrays, options) recurse element-wise.
///
/// The check is the soundness layer above structural unification:
/// two types may unify structurally and still be incompatible
/// because the source carries labels the target does not
/// authorise.
fn flow_admissible(source: &Type, target: &Type) -> bool {
    let source_labels = labels_of(source);
    let target_labels = labels_of(target);
    if !source_labels.is_subset(&target_labels) {
        return false;
    }
    let s = strip_labels(source.clone());
    let t = strip_labels(target.clone());
    match (s, t) {
        (Type::Tuple(ss), Type::Tuple(ts)) if ss.len() == ts.len() => ss
            .iter()
            .zip(ts.iter())
            .all(|(se, te)| flow_admissible(se, te)),
        (Type::Array(s_elem, _), Type::Array(t_elem, _)) => flow_admissible(&s_elem, &t_elem),
        (Type::Option(s_inner), Type::Option(t_inner)) => flow_admissible(&s_inner, &t_inner),
        _ => true,
    }
}

/// Top-level type check entry point.
///
/// Walks the program in two passes. The first pass collects type
/// definitions, struct and enum field signatures, data block field
/// types, and function signatures. The second pass checks each
/// function body against its declared signature.
/// Convert a resolved [`Type`] into a [`TypeExpr`] using the given span.
///
/// Used by [`check`] to fill in parameter type annotations that the
/// programmer omitted but the type checker has inferred. Returns
/// `None` for types that cannot be expressed in the surface syntax
/// (inference variables, abstract type-parameter slots, opaque
/// names that the parser would not accept here, and types that the
/// compiler does not need precise tag information for). The
/// compiler treats a missing `type_expr` as `TypeTag::Composite`,
/// so leaving unconvertible types as `None` preserves the existing
/// permissive behaviour for runtime argument validation while
/// primitive inferences are written back precisely.
fn type_to_expr(ty: &Type, span: crate::token::Span) -> Option<TypeExpr> {
    match ty {
        Type::Byte => Some(TypeExpr::Prim(PrimType::Byte, span)),
        Type::Word => Some(TypeExpr::Prim(PrimType::Word, span)),
        Type::Fixed(n) => Some(TypeExpr::Prim(PrimType::Fixed(Some(*n)), span)),
        Type::Float => Some(TypeExpr::Prim(PrimType::Float, span)),
        Type::Bool => Some(TypeExpr::Prim(PrimType::Bool, span)),
        Type::Unit => Some(TypeExpr::Unit(span)),
        Type::Str => Some(TypeExpr::Prim(PrimType::Text, span)),
        _ => None,
    }
}

/// Convert a fully-resolved [`Type`] to a [`TypeExpr`], including composites,
/// for the authoritative per-function expression-type table (B28 P3 item 5).
///
/// Unlike [`type_to_expr`] (primitives only, used for parameter writeback),
/// this also converts tuples, arrays, options, structs, enums, newtypes, and
/// opaque names, which is exactly what the compiler needs to bake flat access
/// and field-wise equality. It strips information-flow labels (the runtime
/// representation matches the underlying) and returns `None` for any type that
/// still contains an unresolved inference variable, so a span whose type did
/// not fully resolve is simply omitted and the compiler falls back to its
/// structural inference. A struct/enum name is emitted with no type arguments
/// because after monomorphization the name is the concrete mangled name, which
/// is how the compiler keys its type tables (mirroring `infer_expr_type`).
fn type_to_expr_full(ty: &Type, span: crate::token::Span) -> Option<TypeExpr> {
    Some(match ty {
        Type::Byte => TypeExpr::Prim(PrimType::Byte, span),
        Type::Word => TypeExpr::Prim(PrimType::Word, span),
        Type::Fixed(n) => TypeExpr::Prim(PrimType::Fixed(Some(*n)), span),
        Type::Float => TypeExpr::Prim(PrimType::Float, span),
        Type::Bool => TypeExpr::Prim(PrimType::Bool, span),
        Type::Unit => TypeExpr::Unit(span),
        Type::Str => TypeExpr::Prim(PrimType::Text, span),
        Type::Tuple(ts) => {
            let elems = ts
                .iter()
                .map(|t| type_to_expr_full(t, span))
                .collect::<Option<Vec<_>>>()?;
            TypeExpr::Tuple(elems, span)
        }
        Type::Array(elem, n) => TypeExpr::array_lit(
            Box::new(type_to_expr_full(elem, span)?),
            n.known().unwrap_or(0),
            span,
        ),
        Type::Multiword(n, f) => TypeExpr::multiword_lit(
            n.known().unwrap_or(0) as u16,
            f.known().unwrap_or(0) as u16,
            span,
        ),
        Type::Option(inner) => TypeExpr::Option(Box::new(type_to_expr_full(inner, span)?), span),
        Type::Struct(name, _) | Type::Enum(name, _) | Type::Newtype(name) | Type::Opaque(name) => {
            TypeExpr::Named(name.clone(), Vec::new(), Vec::new(), span)
        }
        Type::Labelled(inner, _) => return type_to_expr_full(inner, span),
        Type::Var(_) => return None,
    })
}

/// Target-aware type-check entry point. Identical to [`check`]
/// except that the surface form `Fixed` without `<N>` resolves
/// to the target's
/// [`crate::target::Target::fixed_default_frac_bits`] (lower half
/// of the target word width). Cross-compilation to a 32-bit or
/// 16-bit target therefore picks up Q15.16 or Q7.8 without the
/// programmer needing to write `Fixed<16>` or `Fixed<8>`
/// explicitly.
pub fn check_with_target(
    program: &mut Program,
    target: crate::target::Target,
) -> Result<(), TypeError> {
    let mut ctx = Ctx::new();
    ctx.fixed_default_frac_bits = target.fixed_default_frac_bits();
    run_check(program, ctx)
}

/// Type-check, additionally recording the authoritative per-function
/// expression-type table into `program.fn_expr_types` (B28 P3 item 5).
///
/// Intended for the compiler's post-monomorphization check, where every
/// function is a concrete specialization, so the recorded types are concrete
/// and the per-function span keys do not collide across specializations. The
/// pre-monomorphization check uses the non-recording [`check_with_target`].
pub fn check_with_target_recording(
    program: &mut Program,
    target: crate::target::Target,
) -> Result<(), TypeError> {
    let mut ctx = Ctx::new();
    ctx.fixed_default_frac_bits = target.fixed_default_frac_bits();
    ctx.record_types = true;
    run_check(program, ctx)
}

/// Type-check a call to a native function whose declared signature
/// is in scope. Validates parameter count and per-parameter types
/// against `sig.params`, then returns `sig.return_type`.
///
/// Used at native call sites where the `use` declaration carries a
/// parenthesised signature: `use host::name(T1, T2, ...) -> R`.
/// Native calls without a declared signature take the permissive
/// path inline.
fn check_native_call_with_signature(
    ctx: &mut Ctx,
    name: &str,
    args: &mut [Expr],
    span: &crate::token::Span,
    sig: &FnSig,
) -> Result<Type, TypeError> {
    if args.len() != sig.params.len() {
        return Err(TypeError::new(
            alloc::format!(
                "native `{}` expects {} argument(s), got {}",
                name,
                sig.params.len(),
                args.len()
            ),
            *span,
        ));
    }
    for (i, (arg, expected)) in args.iter_mut().zip(sig.params.iter()).enumerate() {
        let arg_ty = type_of_expr(ctx, arg)?;
        let has_negatives = sig
            .param_negative_labels
            .get(i)
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        // When the parameter declares negative labels, the
        // positive-label upper-bound rule is relaxed: the
        // structural compatibility check ignores the argument's
        // positive labels (which are admissible up to the
        // negative-disjoint constraint checked separately
        // below). When the parameter has no negative labels,
        // the existing positive-label rule applies as-is.
        let (structural_arg_ty, structural_expected) = if has_negatives {
            (strip_labels(arg_ty.clone()), strip_labels(expected.clone()))
        } else {
            (arg_ty.clone(), expected.clone())
        };
        // The runtime auto-widens a Word argument to Float at the
        // native call boundary, so the typechecker accepts Word
        // where the signature declares Float. The widening is
        // top-level only; nested positions inside composite types
        // are not coerced because the marshalling layer does not
        // reach into them.
        let widened_compatible = matches!(
            (&structural_arg_ty, &structural_expected),
            (Type::Word, Type::Float)
        );
        if !widened_compatible && !types_compatible(ctx, &structural_arg_ty, &structural_expected) {
            return Err(TypeError::new(
                alloc::format!(
                    "native `{}` argument {} expects {}, got {}",
                    name,
                    i,
                    expected.display(),
                    arg_ty.display()
                ),
                arg.span(),
            ));
        }
        check_negative_labels_against_arg(
            name,
            i,
            &arg_ty,
            &sig.param_negative_labels,
            arg.span(),
        )?;
    }
    Ok(sig.return_type.clone())
}

/// Type-check `program` against a fresh context and return the
/// first error if any constraint is violated. On success the AST
/// is updated in place with inferred types written back into
/// originally unannotated positions.
pub fn check(program: &mut Program) -> Result<(), TypeError> {
    let ctx = Ctx::new();
    run_check(program, ctx)
}

fn run_check(program: &mut Program, mut ctx: Ctx) -> Result<(), TypeError> {
    // Pass 1a. Collect type kinds (struct, enum, newtype) so name
    // resolution works while reading field signatures and newtype
    // underlying types.
    for type_def in &program.types {
        match type_def {
            TypeDef::Struct(s) => {
                ctx.types.insert(s.name.clone(), TypeKind::Struct);
            }
            TypeDef::Enum(e) => {
                ctx.types.insert(e.name.clone(), TypeKind::Enum);
            }
            TypeDef::Newtype(n) => {
                ctx.types.insert(n.name.clone(), TypeKind::Newtype);
            }
        }
    }

    // Pass 1a'. Resolve newtype underlying types. Newtypes are
    // resolved after every name has been registered in pass 1a so
    // a newtype may reference structs, enums, or other newtypes
    // declared in any order. Cycles between newtypes are not
    // currently detected; a future check should reject them.
    //
    // Newtypes that carry a refinement predicate also have the
    // predicate's signature validated here once the function
    // signatures have been built in pass 1c. The signature check
    // is deferred to a later pass; the underlying type is
    // resolved now so refinement-augmented newtype types are
    // available at construction sites in pass 2.
    for type_def in &program.types {
        if let TypeDef::Newtype(n) = type_def {
            let underlying = ctx.resolve_type(&n.underlying);
            ctx.newtypes.insert(n.name.clone(), underlying);
            if let Some(v) = n.saturate_max {
                ctx.newtype_saturate_max.insert(n.name.clone(), v);
            }
            if let Some(v) = n.saturate_min {
                ctx.newtype_saturate_min.insert(n.name.clone(), v);
            }
        }
    }

    // Pass 1a''. Detect newtype dependency cycles. A newtype
    // chain `newtype A = B; newtype B = A;` would otherwise
    // produce a well-formed type system entry whose underlying
    // dispatch loops forever. The detection walks the newtype
    // graph and rejects any cycle with a diagnostic naming the
    // participating newtypes.
    {
        let mut newtype_decls: BTreeMap<String, crate::token::Span> = BTreeMap::new();
        for type_def in &program.types {
            if let TypeDef::Newtype(n) = type_def {
                newtype_decls.insert(n.name.clone(), n.span);
            }
        }
        for (name, span) in &newtype_decls {
            let mut visited: BTreeSet<String> = BTreeSet::new();
            let mut current = name.clone();
            loop {
                if !visited.insert(current.clone()) {
                    return Err(TypeError::new(
                        alloc::format!("newtype `{}` participates in a definition cycle", name),
                        *span,
                    ));
                }
                // Walk to the next newtype if the current's
                // underlying is itself a newtype.
                match ctx.newtypes.get(&current) {
                    Some(Type::Newtype(next)) => current = next.clone(),
                    _ => break,
                }
            }
        }
    }

    // Pass 1b. Build struct field types, enum variant types, and data
    // block field types. Generic types allocate a fresh `Type::Var`
    // per declared type parameter and resolve field/variant type
    // expressions through `from_expr_with_params` so the recorded
    // declarations carry abstract type variables. Construction at use
    // sites instantiates these abstract variables with fresh per-call
    // variables and applies the substitution to declared field types
    // before unifying with provided values.
    for type_def in &program.types {
        match type_def {
            TypeDef::Struct(s) => {
                let mut tp_map: BTreeMap<String, Type> = BTreeMap::new();
                let mut tp_vars: Vec<Type> = Vec::new();
                for tp in &s.type_params {
                    let v = ctx.fresh();
                    tp_map.insert(tp.name.clone(), v.clone());
                    tp_vars.push(v);
                }
                let mut fields = BTreeMap::new();
                for f in &s.fields {
                    fields.insert(
                        f.name.clone(),
                        ctx.resolve_type_with_params(&f.type_expr, &tp_map),
                    );
                }
                ctx.structs.insert(s.name.clone(), fields);
                ctx.struct_type_param_vars.insert(s.name.clone(), tp_vars);
            }
            TypeDef::Enum(e) => {
                let mut tp_map: BTreeMap<String, Type> = BTreeMap::new();
                let mut tp_vars: Vec<Type> = Vec::new();
                for tp in &e.type_params {
                    let v = ctx.fresh();
                    tp_map.insert(tp.name.clone(), v.clone());
                    tp_vars.push(v);
                }
                let mut variants = BTreeMap::new();
                for v in &e.variants {
                    let payload: Vec<Type> = v
                        .fields
                        .iter()
                        .map(|t| ctx.resolve_type_with_params(t, &tp_map))
                        .collect();
                    variants.insert(v.name.clone(), payload);
                }
                ctx.enums.insert(e.name.clone(), variants);
                ctx.enum_type_param_vars.insert(e.name.clone(), tp_vars);
            }
            TypeDef::Newtype(_) => {
                // Newtype underlying types were resolved in
                // pass 1a' above. Pass 1b's field/variant
                // construction does not apply.
            }
        }
    }

    for data in &program.data_decls {
        let mut fields = BTreeMap::new();
        let mut field_negatives: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for f in &data.fields {
            // Reject nested negative labels (positions deeper than
            // the top-level wrapper). The top-level wrapper itself
            // is admissible on data field types per the boundary
            // semantics: a `shared` data field is the host-script
            // channel; a `private` data field crosses the
            // yield-resume boundary. Both are boundary positions in
            // the same sense as a function parameter.
            validate_no_nested_negative_labels(&f.type_expr, true)?;
            field_negatives.insert(f.name.clone(), top_level_negative_labels(&f.type_expr));
            fields.insert(f.name.clone(), ctx.resolve_type(&f.type_expr));
        }
        ctx.data.insert(data.name.clone(), fields);
        ctx.data_negative_labels
            .insert(data.name.clone(), field_negatives);
    }

    // Pass 1c0. Collect native names from `use` declarations.
    // Names take the form `path::name` or just `name` for use without
    // path. Wildcard imports cannot be resolved at compile time and
    // are treated leniently elsewhere. Declarations carrying a
    // parenthesised signature (`use host::name(T1, T2, ...) -> R`)
    // also populate `ctx.native_signatures` for call-site validation.
    for use_decl in &program.uses {
        if let ImportItem::Name(name) = &use_decl.import {
            let full = if use_decl.path.is_empty() {
                name.clone()
            } else {
                let mut full = String::new();
                for (i, seg) in use_decl.path.iter().enumerate() {
                    if i > 0 {
                        full.push_str("::");
                    }
                    full.push_str(seg);
                }
                full.push_str("::");
                full.push_str(name);
                full
            };
            if let Some(sig) = &use_decl.signature {
                for p in &sig.params {
                    validate_no_nested_negative_labels(p, true)?;
                }
                validate_no_nested_negative_labels(&sig.return_type, true)?;
                let param_negative_labels: Vec<BTreeSet<String>> =
                    sig.params.iter().map(top_level_negative_labels).collect();
                let return_negative_labels = top_level_negative_labels(&sig.return_type);
                let params: Vec<Type> = sig.params.iter().map(|t| ctx.resolve_type(t)).collect();
                let return_type = ctx.resolve_type(&sig.return_type);
                ctx.native_signatures.insert(
                    full.clone(),
                    FnSig {
                        type_params: Vec::new(),
                        const_params: Vec::new(),
                        type_param_vars: Vec::new(),
                        type_param_bounds: Vec::new(),
                        params,
                        return_type,
                        param_negative_labels,
                        return_negative_labels,
                    },
                );
            }
            ctx.natives.insert(full);
        }
    }

    // Pass 1c. Build function signatures.
    //
    // Generic functions allocate a fresh `Type::Var` for each
    // declared type parameter. Parameter and return type expressions
    // resolve type parameter names through this mapping so the
    // signature reflects the abstract bindings. Call-site
    // instantiation later substitutes these abstract variables with
    // fresh per-call variables before unifying with actual argument
    // types.
    for func in &program.functions {
        let mut tp_map: BTreeMap<String, Type> = BTreeMap::new();
        let mut tp_vars: Vec<Type> = Vec::new();
        let mut tp_names: Vec<String> = Vec::new();
        let mut tp_bounds: Vec<Vec<String>> = Vec::new();
        for tp in &func.type_params {
            let v = ctx.fresh();
            tp_map.insert(tp.name.clone(), v.clone());
            tp_vars.push(v);
            tp_names.push(tp.name.clone());
            tp_bounds.push(tp.bounds.clone());
        }
        // Validate that no negative-label wrapper appears at a
        // nested position inside the function's signature. Only
        // the top-level wrapper on each parameter and on the
        // return type is admissible in V0.2.0.
        for p in &func.params {
            if let Some(t) = &p.type_expr {
                validate_no_nested_negative_labels(t, true)?;
            }
        }
        validate_no_nested_negative_labels(&func.return_type, true)?;
        let param_negative_labels: Vec<BTreeSet<String>> = func
            .params
            .iter()
            .map(|p| match &p.type_expr {
                Some(t) => top_level_negative_labels(t),
                None => BTreeSet::new(),
            })
            .collect();
        let return_negative_labels = top_level_negative_labels(&func.return_type);
        let params: Vec<Type> = func
            .params
            .iter()
            .map(|p| match &p.type_expr {
                Some(t) => ctx.resolve_type_with_params(t, &tp_map),
                None => ctx.fresh(),
            })
            .collect();
        let return_type = ctx.resolve_type_with_params(&func.return_type, &tp_map);
        ctx.functions.insert(
            func.name.clone(),
            FnSig {
                type_params: tp_names,
                const_params: func.const_params.iter().map(|c| c.name.clone()).collect(),
                type_param_vars: tp_vars,
                type_param_bounds: tp_bounds,
                params,
                return_type,
                param_negative_labels,
                return_negative_labels,
            },
        );
    }

    // Pass 1c'. Validate refinement predicates on newtype
    // declarations. Each `newtype Name = Underlying where pred`
    // requires that `pred` is declared in the same program with
    // signature `fn(Underlying) -> Bool` and category Atomic. The
    // category check is enforced through `FnSig` already
    // restricting to function-shaped signatures; the parameter
    // and return type checks are explicit here.
    for type_def in &program.types {
        if let TypeDef::Newtype(n) = type_def
            && let Some(pred_name) = &n.refinement
        {
            ctx.refined_newtypes.insert(n.name.clone());
            let underlying = match ctx.newtypes.get(&n.name).cloned() {
                Some(ty) => ty,
                None => ctx.fresh(),
            };
            // Clone the signature to release the borrow on `ctx`
            // before calling `types_compatible`, which requires
            // `&mut ctx`.
            let sig = ctx.functions.get(pred_name).cloned().ok_or_else(|| {
                TypeError::new(
                    alloc::format!(
                        "refinement predicate `{}` on newtype `{}` is not declared in this program",
                        pred_name,
                        n.name
                    ),
                    n.span,
                )
            })?;
            if sig.params.len() != 1 {
                return Err(TypeError::new(
                    alloc::format!(
                        "refinement predicate `{}` must take exactly 1 argument, takes {}",
                        pred_name,
                        sig.params.len()
                    ),
                    n.span,
                ));
            }
            let sig_param = sig.params[0].clone();
            let sig_return = sig.return_type.clone();
            if !types_compatible(&mut ctx, &sig_param, &underlying) {
                return Err(TypeError::new(
                    alloc::format!(
                        "refinement predicate `{}` parameter type {} does not match newtype `{}` underlying {}",
                        pred_name,
                        sig_param.display(),
                        n.name,
                        underlying.display()
                    ),
                    n.span,
                ));
            }
            if !matches!(sig_return, Type::Bool) {
                return Err(TypeError::new(
                    alloc::format!(
                        "refinement predicate `{}` must return Bool, returns {}",
                        pred_name,
                        sig_return.display()
                    ),
                    n.span,
                ));
            }
        }
    }

    // Pass 1d. Register trait declarations and implementation blocks.
    // Traits collect their declared method signatures. Impls record
    // the (trait, type) pair in `ctx.impls` so call-site bound
    // checking can verify that a generic function's `T: Trait`
    // constraint is satisfied by the actual argument type.
    for trait_def in &program.traits {
        ctx.traits
            .insert(trait_def.name.clone(), trait_def.methods.clone());
    }
    for impl_block in &program.impls {
        let head = match strip_labels(ctx.resolve_type(&impl_block.for_type)) {
            Type::Byte => "Byte".to_string(),
            Type::Word => "Word".to_string(),
            Type::Fixed(n) => alloc::format!("Fixed<{}>", n),
            Type::Float => "Float".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Unit => "()".to_string(),
            Type::Str => "Text".to_string(),
            Type::Tuple(_) => "tuple".to_string(),
            Type::Array(_, _) => "array".to_string(),
            Type::Multiword(n, f) => alloc::format!("Multiword<{}, {}>", n, f),
            Type::Option(_) => "Option".to_string(),
            Type::Struct(name, _) | Type::Enum(name, _) | Type::Opaque(name) => name,
            Type::Newtype(name) => name,
            Type::Labelled(_, _) => unreachable!("strip_labels removed Labelled"),
            Type::Var(_) => continue,
        };
        ctx.impls
            .entry(impl_block.trait_name.clone())
            .or_default()
            .insert(head.clone());

        // Register each impl method under a mangled name so it is
        // callable from user code through the existing `Expr::Call`
        // path. The mangling is `TraitName::TypeName::methodName`.
        // Generic-receiver method dispatch uses these mangled names
        // after monomorphization specializes the call site.
        for method in &impl_block.methods {
            let mangled = format!("{}::{}::{}", impl_block.trait_name, head, method.name);
            let mut tp_map: BTreeMap<String, Type> = BTreeMap::new();
            let mut tp_vars: Vec<Type> = Vec::new();
            let mut tp_names: Vec<String> = Vec::new();
            let mut tp_bounds: Vec<Vec<String>> = Vec::new();
            // The impl block's own type parameters (the `T` in
            // `impl<T> Trait for Cell<T>`) are generic parameters of every
            // method, so a fresh variable must stand for each and the
            // receiver type `Cell<T>` must resolve through it. Without this
            // the receiver would resolve to a rigid `T` that never unifies
            // with a concrete `Cell<Word>` at a call site.
            for tp in impl_block
                .type_params
                .iter()
                .chain(method.type_params.iter())
            {
                let v = ctx.fresh();
                tp_map.insert(tp.name.clone(), v.clone());
                tp_vars.push(v);
                tp_names.push(tp.name.clone());
                tp_bounds.push(tp.bounds.clone());
            }
            for p in &method.params {
                if let Some(t) = &p.type_expr {
                    validate_no_nested_negative_labels(t, true)?;
                }
            }
            validate_no_nested_negative_labels(&method.return_type, true)?;
            let param_negative_labels: Vec<BTreeSet<String>> = method
                .params
                .iter()
                .map(|p| match &p.type_expr {
                    Some(t) => top_level_negative_labels(t),
                    None => BTreeSet::new(),
                })
                .collect();
            let return_negative_labels = top_level_negative_labels(&method.return_type);
            let params: Vec<Type> = method
                .params
                .iter()
                .map(|p| match &p.type_expr {
                    Some(t) => ctx.resolve_type_with_params(t, &tp_map),
                    None => ctx.fresh(),
                })
                .collect();
            let return_type = ctx.resolve_type_with_params(&method.return_type, &tp_map);
            let const_param_names: Vec<String> = impl_block
                .const_params
                .iter()
                .chain(method.const_params.iter())
                .map(|c| c.name.clone())
                .collect();
            ctx.functions.insert(
                mangled,
                FnSig {
                    type_params: tp_names,
                    const_params: const_param_names,
                    type_param_vars: tp_vars,
                    type_param_bounds: tp_bounds,
                    params,
                    return_type,
                    param_negative_labels,
                    return_negative_labels,
                },
            );
        }
    }

    // Pass 1e. Validate impl method signatures against the trait's
    // declared signatures. Each impl method must match the trait's
    // method by name, parameter types, and return type. Self in the
    // trait declaration is not yet a distinguished type and is
    // treated as the implementing type by name match.
    for impl_block in &program.impls {
        let trait_methods = match ctx.traits.get(&impl_block.trait_name) {
            Some(m) => m.clone(),
            None => {
                return Err(TypeError::new(
                    format!("impl references unknown trait `{}`", impl_block.trait_name),
                    impl_block.span,
                ));
            }
        };
        for impl_method in &impl_block.methods {
            let trait_sig = trait_methods.iter().find(|m| m.name == impl_method.name);
            let trait_sig = match trait_sig {
                Some(s) => s,
                None => {
                    return Err(TypeError::new(
                        format!(
                            "impl for trait `{}` provides method `{}` that is not in the trait",
                            impl_block.trait_name, impl_method.name
                        ),
                        impl_method.span,
                    ));
                }
            };
            if trait_sig.params.len() != impl_method.params.len() {
                return Err(TypeError::new(
                    format!(
                        "impl method `{}::{}` has {} parameter(s), trait declares {}",
                        impl_block.trait_name,
                        impl_method.name,
                        impl_method.params.len(),
                        trait_sig.params.len()
                    ),
                    impl_method.span,
                ));
            }
            // Validate that each parameter type and the return type
            // agree between the impl method and the trait declaration.
            // Both sides resolve through `from_expr` against the same
            // `ctx.types` registry. Type parameters of the trait or
            // impl are not yet resolved here; the validation matches
            // by structural equality. Self is not yet a distinguished
            // type so trait declarations that mention `Self` use the
            // implementing type's name explicitly. Future work: allow
            // `Self` in trait declarations and substitute the impl's
            // for_type at validation time.
            for (idx, (impl_param, trait_param)) in impl_method
                .params
                .iter()
                .zip(trait_sig.params.iter())
                .enumerate()
            {
                let impl_ty = match &impl_param.type_expr {
                    Some(t) => ctx.resolve_type(t),
                    None => continue,
                };
                let trait_ty = match &trait_param.type_expr {
                    Some(t) => ctx.resolve_type(t),
                    None => continue,
                };
                if impl_ty != trait_ty {
                    return Err(TypeError::new(
                        format!(
                            "impl method `{}::{}` parameter {} has type {} but trait declares {}",
                            impl_block.trait_name,
                            impl_method.name,
                            idx,
                            impl_ty.display(),
                            trait_ty.display()
                        ),
                        impl_param.span,
                    ));
                }
            }
            let impl_ret = ctx.resolve_type(&impl_method.return_type);
            let trait_ret = ctx.resolve_type(&trait_sig.return_type);
            if impl_ret != trait_ret {
                return Err(TypeError::new(
                    format!(
                        "impl method `{}::{}` returns {} but trait declares {}",
                        impl_block.trait_name,
                        impl_method.name,
                        impl_ret.display(),
                        trait_ret.display()
                    ),
                    impl_method.span,
                ));
            }
        }
    }

    // Pass 2. Check each function body. After each body is checked,
    // any parameter type that the programmer omitted but inference
    // resolved to a concrete primitive is written back into the AST
    // so downstream passes (monomorphizer, compiler, runtime call
    // validator) see the inferred type without having to consult the
    // typecheck context.
    for func in &mut program.functions {
        check_function(&mut ctx, func)?;
        if let Some(sig) = ctx.functions.get(&func.name) {
            let resolved: Vec<Type> = sig.params.clone();
            for (param, ty) in func.params.iter_mut().zip(resolved.iter()) {
                if param.type_expr.is_none()
                    && matches!(
                        param.pattern,
                        Pattern::Variable(_, _) | Pattern::Wildcard(_)
                    )
                    && let Some(expr) = type_to_expr(ty, param.span)
                {
                    param.type_expr = Some(expr);
                }
            }
        }
    }
    // Also check impl method bodies. The bodies are checked under
    // their mangled names so the parameter and return type lookups
    // resolve through the same FnSig that was registered in pass 1d.
    for impl_block in &program.impls {
        let head = match strip_labels(ctx.resolve_type(&impl_block.for_type)) {
            Type::Byte => "Byte".to_string(),
            Type::Word => "Word".to_string(),
            Type::Fixed(n) => alloc::format!("Fixed<{}>", n),
            Type::Float => "Float".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Unit => "()".to_string(),
            Type::Str => "Text".to_string(),
            Type::Tuple(_) => "tuple".to_string(),
            Type::Array(_, _) => "array".to_string(),
            Type::Multiword(n, f) => alloc::format!("Multiword<{}, {}>", n, f),
            Type::Option(_) => "Option".to_string(),
            Type::Struct(name, _) | Type::Enum(name, _) | Type::Opaque(name) => name,
            Type::Newtype(name) => name,
            Type::Labelled(_, _) => unreachable!("strip_labels removed Labelled"),
            Type::Var(_) => continue,
        };
        for method in &impl_block.methods {
            let mut renamed = method.clone();
            renamed.name = format!("{}::{}::{}", impl_block.trait_name, head, method.name);
            // The method body resolves the impl block's generic parameters
            // (`T`, `n`), so prepend them to the method's own parameter
            // lists before checking the body. Without this a method body
            // that mentions `T` or uses `n` as a value fails to resolve.
            let mut tps = impl_block.type_params.clone();
            tps.extend(renamed.type_params.clone());
            renamed.type_params = tps;
            let mut cps = impl_block.const_params.clone();
            cps.extend(renamed.const_params.clone());
            renamed.const_params = cps;
            check_function(&mut ctx, &mut renamed)?;
        }
    }

    // Publish the authoritative per-function expression-type tables built by
    // the recording pass into the program for the compiler to consult (B28 P3
    // item 5). Empty when recording was not requested.
    program.fn_expr_types = core::mem::take(&mut ctx.fn_tables);
    Ok(())
}

fn check_function(ctx: &mut Ctx, func: &mut FunctionDef) -> Result<(), TypeError> {
    // The `ephemeral` modifier is only meaningful on the entry
    // point. The verifier's ephemerality proof is a whole-module
    // property; attaching the modifier to a helper function is a
    // category error. Reject here so the compile pipeline surfaces
    // the mistake at the source span.
    if func.ephemeral && func.name != "main" {
        return Err(TypeError::new(
            alloc::format!(
                "`ephemeral` modifier only permitted on the entry point `main`; remove it from `{}`",
                func.name
            ),
            func.span,
        ));
    }
    // Snapshot the substitution at function entry so the per-function
    // resolution does not pollute later functions with this function's
    // local type variables. The vargen counter continues monotonically
    // across functions because variable identifiers are unique even
    // after substitution snapshots.
    let subst_snapshot = ctx.subst.clone();
    // Reset the per-function expression-type recording buffers (B28 P3
    // item 5). Spans are unique within one function, so each function gets a
    // fresh table.
    if ctx.record_types {
        ctx.current_fn_types.clear();
        ctx.fn_type_conflicts.clear();
    }
    ctx.push_scope();
    let return_type = ctx
        .functions
        .get(&func.name)
        .map(|s| s.return_type.clone())
        .unwrap_or_else(|| ctx.fresh());
    ctx.current_return = Some(return_type.clone());
    // Bind parameters.
    let sig_params = ctx
        .functions
        .get(&func.name)
        .map(|s| s.params.clone())
        .unwrap_or_default();
    for (param, param_type) in func.params.iter().zip(sig_params.iter()) {
        bind_pattern(ctx, &param.pattern, param_type.clone());
    }
    // Bind const parameters as `Word` values in the function scope, so a
    // body may use `n` as an integer (`for i in 0..n`). A local binding
    // in a nested scope shadows the const parameter, since `lookup_local`
    // searches inner scopes first (B40).
    for cp in &func.const_params {
        ctx.add_local(cp.name.clone(), Type::Word);
    }
    ctx.current_const_params = func.const_params.iter().map(|c| c.name.clone()).collect();
    // Check body. The block's tail expression must match the return
    // type when the return type is not Unit. For Unit-returning
    // functions, an absent tail is admissible. The declared return
    // type is pushed as the expected type so refinement-driven
    // saturate keywords inside the tail can consult it.
    // Stash the function's return-side negative-label set on the
    // context so `Expr::Yield` and the body's tail return can
    // consult it without re-resolving the function's name. The
    // outer scope's value is restored on exit.
    let prev_return_negatives = ctx.current_return_negative_labels.clone();
    ctx.current_return_negative_labels = ctx
        .functions
        .get(&func.name)
        .map(|s| s.return_negative_labels.clone())
        .unwrap_or_default();
    ctx.push_expected(return_type.clone());
    let body_result = type_of_block(ctx, &mut func.body);
    ctx.pop_expected();
    let body_type = body_result?;
    // When the function's return type declares negative labels,
    // the positive-label upper-bound rule is relaxed: any
    // positive labels on the body's return value are admissible
    // up to the negative-disjoint clause checked separately
    // below.
    let has_return_negatives = !ctx.current_return_negative_labels.is_empty();
    let (struct_body, struct_return) = if has_return_negatives {
        (
            strip_labels(body_type.clone()),
            strip_labels(return_type.clone()),
        )
    } else {
        (body_type.clone(), return_type.clone())
    };
    if !types_compatible(ctx, &struct_body, &struct_return) {
        ctx.pop_scope();
        ctx.current_return = None;
        ctx.current_return_negative_labels = prev_return_negatives;
        // Display the body type with the latest substitution applied so
        // the user sees the most-resolved form.
        let body_resolved = body_type.apply(&ctx.subst);
        let return_resolved = return_type.apply(&ctx.subst);
        return Err(TypeError::new(
            format!(
                "function `{}` returns {} but body produces {}",
                func.name,
                return_resolved.display(),
                body_resolved.display()
            ),
            func.body.span,
        ));
    }
    // Return-side negative-label boundary clause. The function's
    // declared `return-type@!Label` set rejects body tails whose
    // value carries any of those labels.
    if let Err(e) = check_negative_labels_against_return(
        &alloc::format!("function `{}` body return value", func.name),
        &body_type,
        &ctx.current_return_negative_labels,
        func.body.span,
    ) {
        ctx.pop_scope();
        ctx.current_return = None;
        ctx.current_return_negative_labels = prev_return_negatives;
        return Err(e);
    }
    ctx.pop_scope();
    ctx.current_return = None;
    ctx.current_return_negative_labels = prev_return_negatives;
    // Apply the substitution accumulated during this function check
    // back to the function's parameter types so the global FnSig
    // entry reflects any inference performed against unannotated
    // positions. This makes the resolved types visible to call-site
    // checks of subsequent functions in the same module.
    if let Some(sig) = ctx.functions.get_mut(&func.name) {
        sig.return_type = sig.return_type.apply(&ctx.subst);
        for p in sig.params.iter_mut() {
            *p = p.apply(&ctx.subst);
        }
    }
    // Finalize this function's authoritative expression-type table before the
    // substitution is rolled back, so any types still recorded with this
    // function's local variables are resolved (B28 P3 item 5). The buffer
    // already holds resolved `TypeExpr`s; move them under the function name.
    // Re-resolving with the now-complete substitution is unnecessary because
    // each entry was converted only when fully resolved, but conflicting spans
    // were already excluded.
    if ctx.record_types && !ctx.current_fn_types.is_empty() {
        let table = core::mem::take(&mut ctx.current_fn_types);
        ctx.fn_tables.insert(func.name.clone(), table);
    }
    // Roll back the substitution to the snapshot so type variables
    // local to this function do not leak into the next function's
    // checking. The Hindley-Milner discipline is per-function in
    // monomorphic Keleusma; cross-function generalization is the
    // domain of B2 generic parameters.
    ctx.subst = subst_snapshot;
    Ok(())
}

/// Bind a pattern's variables into the current scope at the given type.
/// Returns true if every pattern in a `CheckedArmKind` is a
/// catch-all (wildcard or bare variable). A catch-all arm matches
/// every runtime value for its outcome class.
fn checked_arm_is_catchall(kind: &crate::ast::CheckedArmKind) -> bool {
    use crate::ast::CheckedArmKind;
    let is_catchall_pat = |p: &Pattern| matches!(p, Pattern::Wildcard(_) | Pattern::Variable(_, _));
    match kind {
        CheckedArmKind::Ok(p)
        | CheckedArmKind::ZeroDivisor(p)
        | CheckedArmKind::Nan(p)
        | CheckedArmKind::InvalidIndex(p)
        | CheckedArmKind::InvalidNewtype(p)
        | CheckedArmKind::PayloadDiscriminant(p)
        | CheckedArmKind::InvalidDiscriminant(p)
        | CheckedArmKind::Error(p) => is_catchall_pat(p),
        CheckedArmKind::Overflow(h, l) | CheckedArmKind::Underflow(h, l) => {
            // A `None` second pattern (the Byte single-pattern form)
            // covers its slot unconditionally.
            is_catchall_pat(h) && l.as_ref().is_none_or(is_catchall_pat)
        }
    }
}

/// Type-check the indexing construct `array[index] { ok(v) => ...,
/// invalid_index(i) => ... }` (B35 P4). The admissible arms are `ok`,
/// binding the element type, and `invalid_index`, binding the
/// offending index `Word`. The `ok` class must have an unguarded
/// catch-all; `invalid_index` is optional and an unhandled
/// out-of-bounds index traps at runtime. The arithmetic outcome arms
/// (`overflow`, `underflow`, `zero_divisor`, `nan`) are inadmissible.
fn check_checked_index(
    ctx: &mut Ctx,
    op_expr: &mut Expr,
    arms: &mut [crate::ast::CheckedArm],
    span: &Span,
) -> Result<Type, TypeError> {
    use crate::ast::CheckedArmKind;
    // The element type is the type of the array-index expression.
    let elem_ty = type_of_expr(ctx, op_expr)?;

    // Vocabulary: only `ok` and `invalid_index` are admissible.
    for arm in arms.iter() {
        let inadmissible = match &arm.kind {
            CheckedArmKind::Ok(_) | CheckedArmKind::InvalidIndex(_) => None,
            CheckedArmKind::Overflow(_, _) => Some("overflow"),
            CheckedArmKind::Underflow(_, _) => Some("underflow"),
            CheckedArmKind::ZeroDivisor(_) => Some("zero_divisor"),
            CheckedArmKind::Nan(_) => Some("nan"),
            CheckedArmKind::InvalidNewtype(_) => Some("invalid_newtype"),
            CheckedArmKind::PayloadDiscriminant(_) => Some("payload_discriminant"),
            CheckedArmKind::InvalidDiscriminant(_) => Some("invalid_discriminant"),
            CheckedArmKind::Error(_) => Some("error"),
        };
        if let Some(name) = inadmissible {
            return Err(TypeError::new(
                alloc::format!(
                    "the `{}` arm is not admissible for array indexing; only `ok` and `invalid_index` are admissible",
                    name
                ),
                arm.span,
            ));
        }
    }

    // Unreachable-arm check per outcome class.
    let mut ok_catchall_seen = false;
    let mut invalid_catchall_seen = false;
    for arm in arms.iter() {
        let class_catchall_seen = match &arm.kind {
            CheckedArmKind::Ok(_) => ok_catchall_seen,
            CheckedArmKind::InvalidIndex(_) => invalid_catchall_seen,
            _ => false,
        };
        if class_catchall_seen {
            return Err(TypeError::new(
                alloc::string::String::from(
                    "indexing-construct arm is unreachable: a prior catch-all arm in the same outcome class already covers it",
                ),
                arm.span,
            ));
        }
        if arm.guard.is_none() && checked_arm_is_catchall(&arm.kind) {
            match &arm.kind {
                CheckedArmKind::Ok(_) => ok_catchall_seen = true,
                CheckedArmKind::InvalidIndex(_) => invalid_catchall_seen = true,
                _ => {}
            }
        }
    }
    if !ok_catchall_seen {
        return Err(TypeError::new(
            alloc::string::String::from(
                "indexing construct is non-exhaustive on `ok`: the last `ok` arm must be an unguarded catch-all (bare variable or wildcard)",
            ),
            *span,
        ));
    }
    // `invalid_index` is optional; an unhandled out-of-bounds index
    // traps. The flag remains in use only for the unreachable check.
    let _ = invalid_catchall_seen;

    // Type-check arm bodies. `ok` binds the element type; an
    // `invalid_index` arm binds the offending index `Word`.
    let result_ty = ctx.fresh();
    for arm in arms.iter_mut() {
        ctx.push_scope();
        match &arm.kind {
            CheckedArmKind::Ok(p) => bind_checked_pattern(ctx, p, elem_ty.clone()),
            CheckedArmKind::InvalidIndex(p) => bind_checked_pattern(ctx, p, Type::Word),
            _ => {}
        }
        if let Some(guard) = arm.guard.as_mut() {
            let guard_ty = type_of_expr(ctx, guard)?;
            if !types_compatible(ctx, &strip_labels(guard_ty.clone()), &Type::Bool) {
                ctx.pop_scope();
                return Err(TypeError::new(
                    alloc::format!(
                        "indexing-construct arm guard must be Bool, got {}",
                        guard_ty.display()
                    ),
                    arm.span,
                ));
            }
        }
        let body_ty = type_of_expr(ctx, &mut arm.body)?;
        ctx.pop_scope();
        if !types_compatible(ctx, &body_ty, &result_ty) {
            return Err(TypeError::new(
                alloc::format!(
                    "indexing-construct arm produces {} which does not unify with the construct's result type {}",
                    body_ty.display(),
                    result_ty.apply(&ctx.subst).display()
                ),
                arm.span,
            ));
        }
    }
    Ok(result_ty.apply(&ctx.subst))
}

/// Type-check the newtype-construction construct `Name(value) {
/// ok(v) => ..., invalid_newtype(x) => ... }` (B35 P5). The
/// admissible arms are `ok`, binding the constructed newtype, and
/// `invalid_newtype`, binding the underlying value the refinement
/// predicate rejected. The `ok` class must have an unguarded
/// catch-all; `invalid_newtype` is optional and an unhandled failure
/// traps. `invalid_newtype` is admissible only when the newtype
/// carries a refinement predicate, because a non-refined newtype's
/// construction is total.
fn check_checked_newtype(
    ctx: &mut Ctx,
    op_expr: &mut Expr,
    newtype_name: &str,
    arms: &mut [crate::ast::CheckedArm],
    span: &Span,
) -> Result<Type, TypeError> {
    use crate::ast::CheckedArmKind;
    // Type-check the constructor call (validates the argument against
    // the underlying type) and yield the newtype type for `ok`.
    let newtype_ty = type_of_expr(ctx, op_expr)?;
    let underlying = ctx
        .newtypes
        .get(newtype_name)
        .cloned()
        .unwrap_or_else(|| ctx.fresh());
    let is_refined = ctx.refined_newtypes.contains(newtype_name);

    // Vocabulary: only `ok` and `invalid_newtype`.
    for arm in arms.iter() {
        let inadmissible = match &arm.kind {
            CheckedArmKind::Ok(_) => None,
            CheckedArmKind::InvalidNewtype(_) => {
                if is_refined {
                    None
                } else {
                    return Err(TypeError::new(
                        alloc::format!(
                            "the `invalid_newtype` arm is not admissible: newtype `{}` has no refinement predicate, so its construction cannot fail",
                            newtype_name
                        ),
                        arm.span,
                    ));
                }
            }
            CheckedArmKind::Overflow(_, _) => Some("overflow"),
            CheckedArmKind::Underflow(_, _) => Some("underflow"),
            CheckedArmKind::ZeroDivisor(_) => Some("zero_divisor"),
            CheckedArmKind::Nan(_) => Some("nan"),
            CheckedArmKind::InvalidIndex(_) => Some("invalid_index"),
            CheckedArmKind::PayloadDiscriminant(_) => Some("payload_discriminant"),
            CheckedArmKind::InvalidDiscriminant(_) => Some("invalid_discriminant"),
            CheckedArmKind::Error(_) => Some("error"),
        };
        if let Some(name) = inadmissible {
            return Err(TypeError::new(
                alloc::format!(
                    "the `{}` arm is not admissible for newtype construction; only `ok` and `invalid_newtype` are admissible",
                    name
                ),
                arm.span,
            ));
        }
    }

    // Unreachable-arm check per outcome class.
    let mut ok_catchall_seen = false;
    let mut invalid_catchall_seen = false;
    for arm in arms.iter() {
        let class_catchall_seen = match &arm.kind {
            CheckedArmKind::Ok(_) => ok_catchall_seen,
            CheckedArmKind::InvalidNewtype(_) => invalid_catchall_seen,
            _ => false,
        };
        if class_catchall_seen {
            return Err(TypeError::new(
                alloc::string::String::from(
                    "newtype-construction arm is unreachable: a prior catch-all arm in the same outcome class already covers it",
                ),
                arm.span,
            ));
        }
        if arm.guard.is_none() && checked_arm_is_catchall(&arm.kind) {
            match &arm.kind {
                CheckedArmKind::Ok(_) => ok_catchall_seen = true,
                CheckedArmKind::InvalidNewtype(_) => invalid_catchall_seen = true,
                _ => {}
            }
        }
    }
    if !ok_catchall_seen {
        return Err(TypeError::new(
            alloc::string::String::from(
                "newtype-construction construct is non-exhaustive on `ok`: the last `ok` arm must be an unguarded catch-all (bare variable or wildcard)",
            ),
            *span,
        ));
    }
    let _ = invalid_catchall_seen;

    // Type-check arm bodies. `ok` binds the newtype; `invalid_newtype`
    // binds the underlying value.
    let result_ty = ctx.fresh();
    for arm in arms.iter_mut() {
        ctx.push_scope();
        match &arm.kind {
            CheckedArmKind::Ok(p) => bind_checked_pattern(ctx, p, newtype_ty.clone()),
            CheckedArmKind::InvalidNewtype(p) => bind_checked_pattern(ctx, p, underlying.clone()),
            _ => {}
        }
        if let Some(guard) = arm.guard.as_mut() {
            let guard_ty = type_of_expr(ctx, guard)?;
            if !types_compatible(ctx, &strip_labels(guard_ty.clone()), &Type::Bool) {
                ctx.pop_scope();
                return Err(TypeError::new(
                    alloc::format!(
                        "newtype-construction arm guard must be Bool, got {}",
                        guard_ty.display()
                    ),
                    arm.span,
                ));
            }
        }
        let body_ty = type_of_expr(ctx, &mut arm.body)?;
        ctx.pop_scope();
        if !types_compatible(ctx, &body_ty, &result_ty) {
            return Err(TypeError::new(
                alloc::format!(
                    "newtype-construction arm produces {} which does not unify with the construct's result type {}",
                    body_ty.display(),
                    result_ty.apply(&ctx.subst).display()
                ),
                arm.span,
            ));
        }
    }
    Ok(result_ty.apply(&ctx.subst))
}

/// Whether a checked-arm pattern names an enum variant: an
/// upper-case `Variable`. The discriminant-to-enum construct (B35 P6)
/// uses upper-case identifiers for `ok` and `payload_discriminant`
/// variant names and lower-case identifiers (or `_`) for binders and
/// catch-alls.
fn checked_arm_variant_name(p: &Pattern) -> Option<&str> {
    if let Pattern::Variable(name, _) = p
        && name.chars().next().is_some_and(|c| c.is_uppercase())
    {
        Some(name)
    } else {
        None
    }
}

/// Type-check the discriminant-to-enum construct `discriminant as
/// EnumType { ok(Variant) => ..., payload_discriminant(Variant) =>
/// ..., invalid_discriminant(raw) => ... }` (B35 P6). The source must
/// be a `Word`. An `ok(Variant)` arm overrides a unit variant; a
/// generic `ok(v)`/`ok(_)` arm post-processes any unit variant; a
/// `payload_discriminant(Variant)` arm supplies a payload variant's
/// payload; and `invalid_discriminant(raw)` catches an unmapped
/// discriminant. Coverage of every payload-bearing variant is
/// mandatory. Unit variants convert to themselves when no arm covers
/// them; an unhandled invalid discriminant traps.
fn check_checked_discriminant(
    ctx: &mut Ctx,
    op_expr: &mut Expr,
    enum_name: &str,
    arms: &mut [crate::ast::CheckedArm],
    span: &Span,
) -> Result<Type, TypeError> {
    use crate::ast::CheckedArmKind;
    // The source of the cast must be a `Word`.
    let inner_ty = match op_expr {
        Expr::Cast { expr, .. } => type_of_expr(ctx, expr)?,
        _ => {
            return Err(TypeError::new(
                alloc::string::String::from(
                    "internal error: discriminant-to-enum construct on a non-cast operation",
                ),
                *span,
            ));
        }
    };
    if !types_compatible(ctx, &strip_labels(inner_ty.clone()), &Type::Word) {
        return Err(TypeError::new(
            alloc::format!(
                "discriminant-to-enum conversion requires a Word source, got {}",
                inner_ty.display()
            ),
            *span,
        ));
    }
    let enum_ty = match op_expr {
        Expr::Cast { target, .. } => strip_labels(ctx.resolve_type(target)),
        _ => unreachable!(),
    };

    // Variant classification. `ctx.enums` maps each variant to its
    // payload field types; an empty list is a unit variant.
    let variants = match ctx.enums.get(enum_name).cloned() {
        Some(v) => v,
        None => {
            return Err(TypeError::new(
                alloc::format!("`{}` is not a declared enum", enum_name),
                *span,
            ));
        }
    };
    let payload_variants: BTreeSet<String> = variants
        .iter()
        .filter(|(_, fields)| !fields.is_empty())
        .map(|(name, _)| name.clone())
        .collect();

    // Validate the arm vocabulary and per-variant admissibility,
    // collecting the payload variants covered for the mandatory-
    // coverage check.
    let mut payload_covered: BTreeSet<String> = BTreeSet::new();
    let mut payload_catchall = false;
    for arm in arms.iter() {
        match &arm.kind {
            CheckedArmKind::Ok(p) => {
                if let Some(vname) = checked_arm_variant_name(p) {
                    match variants.get(vname) {
                        None => {
                            return Err(TypeError::new(
                                alloc::format!(
                                    "`ok` names `{}`, which is not a variant of enum `{}`",
                                    vname,
                                    enum_name
                                ),
                                arm.span,
                            ));
                        }
                        Some(fields) if !fields.is_empty() => {
                            return Err(TypeError::new(
                                alloc::format!(
                                    "`ok` names the payload-bearing variant `{}`; use `payload_discriminant` for it",
                                    vname
                                ),
                                arm.span,
                            ));
                        }
                        Some(_) => {}
                    }
                }
                // A lower-case `Variable` or `_` is a generic blanket
                // `ok` over unit variants; no further checks.
            }
            CheckedArmKind::PayloadDiscriminant(p) => {
                if let Some(vname) = checked_arm_variant_name(p) {
                    if !payload_variants.contains(vname) {
                        return Err(TypeError::new(
                            alloc::format!(
                                "`payload_discriminant` names `{}`, which is not a payload-bearing variant of enum `{}`",
                                vname,
                                enum_name
                            ),
                            arm.span,
                        ));
                    }
                    payload_covered.insert(alloc::string::String::from(vname));
                } else if matches!(p, Pattern::Wildcard(_)) {
                    payload_catchall = true;
                } else {
                    return Err(TypeError::new(
                        alloc::string::String::from(
                            "`payload_discriminant` takes a variant name or `_`",
                        ),
                        arm.span,
                    ));
                }
            }
            CheckedArmKind::InvalidDiscriminant(p) => {
                if checked_arm_variant_name(p).is_some() {
                    return Err(TypeError::new(
                        alloc::string::String::from(
                            "`invalid_discriminant` takes a binder or `_`, not a variant name",
                        ),
                        arm.span,
                    ));
                }
            }
            other => {
                let n = match other {
                    CheckedArmKind::Overflow(_, _) => "overflow",
                    CheckedArmKind::Underflow(_, _) => "underflow",
                    CheckedArmKind::ZeroDivisor(_) => "zero_divisor",
                    CheckedArmKind::Nan(_) => "nan",
                    CheckedArmKind::InvalidIndex(_) => "invalid_index",
                    CheckedArmKind::InvalidNewtype(_) => "invalid_newtype",
                    _ => "this",
                };
                return Err(TypeError::new(
                    alloc::format!(
                        "the `{}` arm is not admissible for a discriminant-to-enum conversion; only `ok`, `payload_discriminant`, and `invalid_discriminant` are admissible",
                        n
                    ),
                    arm.span,
                ));
            }
        }
    }

    // Mandatory coverage: every payload-bearing variant must be
    // covered, either specifically or through a `_` catch-all,
    // because the discriminant alone cannot reconstruct the payload.
    if !payload_catchall {
        let missing: Vec<String> = payload_variants
            .iter()
            .filter(|v| !payload_covered.contains(*v))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Err(TypeError::new(
                alloc::format!(
                    "discriminant-to-enum conversion does not cover the payload-bearing variant(s) {}; add a `payload_discriminant` arm for each, or a `payload_discriminant(_)` catch-all",
                    missing.join(", ")
                ),
                *span,
            ));
        }
    }

    // Type-check arm bodies. An `ok` generic binder binds the
    // converted unit-variant value at the enum type; an
    // `invalid_discriminant` binder binds the raw `Word`. Specific
    // `ok` and `payload_discriminant` arms bind nothing. Every body
    // yields the enum type.
    for arm in arms.iter_mut() {
        ctx.push_scope();
        match &arm.kind {
            CheckedArmKind::Ok(p) if checked_arm_variant_name(p).is_none() => {
                bind_checked_pattern(ctx, p, enum_ty.clone());
            }
            CheckedArmKind::InvalidDiscriminant(p) => {
                bind_checked_pattern(ctx, p, Type::Word);
            }
            _ => {}
        }
        if let Some(guard) = arm.guard.as_mut() {
            let guard_ty = type_of_expr(ctx, guard)?;
            if !types_compatible(ctx, &strip_labels(guard_ty.clone()), &Type::Bool) {
                ctx.pop_scope();
                return Err(TypeError::new(
                    alloc::format!(
                        "discriminant-to-enum arm guard must be Bool, got {}",
                        guard_ty.display()
                    ),
                    arm.span,
                ));
            }
        }
        let body_ty = type_of_expr(ctx, &mut arm.body)?;
        ctx.pop_scope();
        if !types_compatible(ctx, &body_ty, &enum_ty) {
            return Err(TypeError::new(
                alloc::format!(
                    "discriminant-to-enum arm produces {} which does not unify with the target enum type {}",
                    body_ty.display(),
                    enum_ty.display()
                ),
                arm.span,
            ));
        }
    }
    Ok(enum_ty)
}

/// Type-check the native-error construct `native(args) { ok(v) =>
/// ..., error(code) => ... }` (B35 P7). The admissible arms are `ok`,
/// binding the native's success value, and `error`, binding the
/// `Word` error code a fallible native reported. The `ok` class must
/// have an unguarded catch-all; `error` is optional, and an unhandled
/// native error propagates as it would without the construct. `error`
/// is admissible on any native call, since fallibility is not tracked
/// at compile time; on an infallible native the arm is simply never
/// taken.
fn check_checked_native(
    ctx: &mut Ctx,
    op_expr: &mut Expr,
    native_name: &str,
    arms: &mut [crate::ast::CheckedArm],
    span: &Span,
) -> Result<Type, TypeError> {
    use crate::ast::CheckedArmKind;
    // Type-check the call itself; its type is the success value's type.
    let ok_ty = type_of_expr(ctx, op_expr)?;
    let _ = native_name;

    // Vocabulary: only `ok` and `error`.
    for arm in arms.iter() {
        let inadmissible = match &arm.kind {
            CheckedArmKind::Ok(_) | CheckedArmKind::Error(_) => None,
            CheckedArmKind::Overflow(_, _) => Some("overflow"),
            CheckedArmKind::Underflow(_, _) => Some("underflow"),
            CheckedArmKind::ZeroDivisor(_) => Some("zero_divisor"),
            CheckedArmKind::Nan(_) => Some("nan"),
            CheckedArmKind::InvalidIndex(_) => Some("invalid_index"),
            CheckedArmKind::InvalidNewtype(_) => Some("invalid_newtype"),
            CheckedArmKind::PayloadDiscriminant(_) => Some("payload_discriminant"),
            CheckedArmKind::InvalidDiscriminant(_) => Some("invalid_discriminant"),
        };
        if let Some(name) = inadmissible {
            return Err(TypeError::new(
                alloc::format!(
                    "the `{}` arm is not admissible for a native call; only `ok` and `error` are admissible",
                    name
                ),
                arm.span,
            ));
        }
    }

    // Unreachable-arm check per outcome class.
    let mut ok_catchall_seen = false;
    let mut error_catchall_seen = false;
    for arm in arms.iter() {
        let class_catchall_seen = match &arm.kind {
            CheckedArmKind::Ok(_) => ok_catchall_seen,
            CheckedArmKind::Error(_) => error_catchall_seen,
            _ => false,
        };
        if class_catchall_seen {
            return Err(TypeError::new(
                alloc::string::String::from(
                    "native-call arm is unreachable: a prior catch-all arm in the same outcome class already covers it",
                ),
                arm.span,
            ));
        }
        if arm.guard.is_none() && checked_arm_is_catchall(&arm.kind) {
            match &arm.kind {
                CheckedArmKind::Ok(_) => ok_catchall_seen = true,
                CheckedArmKind::Error(_) => error_catchall_seen = true,
                _ => {}
            }
        }
    }
    if !ok_catchall_seen {
        return Err(TypeError::new(
            alloc::string::String::from(
                "native-call construct is non-exhaustive on `ok`: the last `ok` arm must be an unguarded catch-all (bare variable or wildcard)",
            ),
            *span,
        ));
    }
    let _ = error_catchall_seen;

    // Type-check arm bodies. `ok` binds the success value; `error`
    // binds the `Word` error code.
    let result_ty = ctx.fresh();
    for arm in arms.iter_mut() {
        ctx.push_scope();
        match &arm.kind {
            CheckedArmKind::Ok(p) => bind_checked_pattern(ctx, p, ok_ty.clone()),
            CheckedArmKind::Error(p) => bind_checked_pattern(ctx, p, Type::Word),
            _ => {}
        }
        if let Some(guard) = arm.guard.as_mut() {
            let guard_ty = type_of_expr(ctx, guard)?;
            if !types_compatible(ctx, &strip_labels(guard_ty.clone()), &Type::Bool) {
                ctx.pop_scope();
                return Err(TypeError::new(
                    alloc::format!(
                        "native-call arm guard must be Bool, got {}",
                        guard_ty.display()
                    ),
                    arm.span,
                ));
            }
        }
        let body_ty = type_of_expr(ctx, &mut arm.body)?;
        ctx.pop_scope();
        if !types_compatible(ctx, &body_ty, &result_ty) {
            return Err(TypeError::new(
                alloc::format!(
                    "native-call arm produces {} which does not unify with the construct's result type {}",
                    body_ty.display(),
                    result_ty.apply(&ctx.subst).display()
                ),
                arm.span,
            ));
        }
    }
    Ok(result_ty.apply(&ctx.subst))
}

/// Bind a checked-arm pattern's variables into the current scope at
/// `bind_ty`, the operand type of the construct (`Word` or `Byte`);
/// wildcards and literals introduce no bindings. The compiler relies
/// on the type checker to reject pattern shapes that cannot match
/// (e.g. a string literal pattern would fail unification at the test
/// site, not here).
fn bind_checked_pattern(ctx: &mut Ctx, pattern: &Pattern, bind_ty: Type) {
    match pattern {
        Pattern::Variable(name, _) => ctx.add_local(name.clone(), bind_ty),
        Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
        // Other pattern shapes are rejected at parse time by
        // `parse_checked_arm_pattern`; if one slips through, fall
        // back to no binding so the body type check fails on
        // missing-identifier rather than on a panic here.
        _ => {}
    }
}

/// Coerce a bare integer-literal expression to match the
/// counterpart's narrower numeric type. When `expr` is an
/// `Expr::Literal::Int(n)` with type `Word` and the counterpart
/// is `Byte` or `Fixed<N>`, mutate `expr` to wrap the literal in
/// a cast and return the coerced type. For Byte, the literal
/// must fit in `[0, 255]`; out-of-range values fall through
/// unchanged. For Fixed, any integer literal coerces (the cast
/// op shifts to the fraction-bit representation at runtime).
///
/// The mutation is a one-shot rewrite; on subsequent calls the
/// expression is no longer a bare literal, so the helper is a
/// no-op. This preserves idempotence under the surrounding
/// binop's left- and right-side coercion calls.
fn coerce_integer_literal(
    _ctx: &mut Ctx,
    expr: &mut Expr,
    expr_ty: Type,
    counterpart: &Type,
    span: Span,
) -> Type {
    let counterpart_bare = strip_labels(counterpart.clone());
    if !matches!(strip_labels(expr_ty.clone()), Type::Word) {
        return expr_ty;
    }
    let lit_value = match expr {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => *n,
        _ => return expr_ty,
    };
    match counterpart_bare {
        Type::Byte => {
            if !(0..=255).contains(&lit_value) {
                return expr_ty;
            }
            let inner = expr.clone();
            *expr = Expr::Cast {
                expr: Box::new(inner),
                target: TypeExpr::Prim(PrimType::Byte, span),
                span,
            };
            Type::Byte
        }
        Type::Fixed(n) => {
            let inner = expr.clone();
            *expr = Expr::Cast {
                expr: Box::new(inner),
                target: TypeExpr::Prim(PrimType::Fixed(Some(n)), span),
                span,
            };
            Type::Fixed(n)
        }
        _ => expr_ty,
    }
}

/// Validate a const argument: a literal is always admissible; a
/// parameter reference must name an in-scope const parameter (not a
/// runtime local), so a const argument is a compile-time constant; a
/// binary const expression validates its operands (B40).
fn check_const_arg(ctx: &Ctx, ca: &crate::ast::ConstExpr) -> Result<(), TypeError> {
    use crate::ast::ConstExpr;
    match ca {
        ConstExpr::Lit(_, _) => Ok(()),
        ConstExpr::Param(name, span) => {
            if ctx.current_const_params.contains(name) {
                Ok(())
            } else {
                Err(TypeError::new(
                    alloc::format!(
                        "const argument `{}` is not an in-scope const parameter; a const argument must be a compile-time constant",
                        name
                    ),
                    *span,
                ))
            }
        }
        ConstExpr::Bin(_, l, r, _) => {
            check_const_arg(ctx, l)?;
            check_const_arg(ctx, r)
        }
    }
}

fn bind_pattern(ctx: &mut Ctx, pattern: &Pattern, ty: Type) {
    match pattern {
        Pattern::Variable(name, _) => ctx.add_local(name.clone(), ty),
        Pattern::Wildcard(_) => {}
        Pattern::Tuple(parts, _) => {
            if let Type::Tuple(part_tys) = ty {
                for (pat, pty) in parts.iter().zip(part_tys) {
                    bind_pattern(ctx, pat, pty);
                }
            } else {
                for pat in parts {
                    {
                        let fresh = ctx.fresh();
                        bind_pattern(ctx, pat, fresh);
                    }
                }
            }
        }
        Pattern::Enum(enum_name, variant, sub_pats, _) => {
            // Look up the variant's payload types.
            let payload = ctx
                .enums
                .get(enum_name)
                .and_then(|vs| vs.get(variant))
                .cloned();
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                let sub_ty = payload
                    .as_ref()
                    .and_then(|tys| tys.get(i).cloned())
                    .unwrap_or_else(|| ctx.fresh());
                bind_pattern(ctx, sub_pat, sub_ty);
            }
        }
        Pattern::Struct(struct_name, field_pats, _) => {
            // Look up field types from the struct definition.
            let struct_fields = ctx.structs.get(struct_name).cloned();
            for field_pat in field_pats {
                let field_ty = struct_fields
                    .as_ref()
                    .and_then(|fields| fields.get(&field_pat.name).cloned())
                    .unwrap_or_else(|| ctx.fresh());
                if let Some(pat) = &field_pat.pattern {
                    bind_pattern(ctx, pat, field_ty);
                } else {
                    // Shorthand: `Name { field }` binds field to a
                    // local of the same name at the field's type.
                    ctx.add_local(field_pat.name.clone(), field_ty);
                }
            }
        }
        Pattern::Literal(_, _) => {}
    }
}

/// Check that a pattern's shape matches the scrutinee's static type.
///
/// Surfaces shape mismatches such as a tuple pattern against a
/// non-tuple scrutinee, an enum variant pattern against a non-enum
/// scrutinee, an unknown variant name, or a wrong number of payload
/// elements. Variables and wildcards always succeed.
fn check_pattern_against_type(
    ctx: &mut Ctx,
    pattern: &Pattern,
    scrutinee_ty: &Type,
) -> Result<(), TypeError> {
    match pattern {
        Pattern::Wildcard(_) | Pattern::Variable(_, _) => Ok(()),
        Pattern::Literal(lit, span) => {
            let lit_ty = match lit {
                Literal::Int(_) => Type::Word,
                Literal::Float(_) => Type::Float,
                Literal::Byte(_) => Type::Byte,
                Literal::Fixed { frac_bits, .. } => Type::Fixed(*frac_bits),
                Literal::String(_) => Type::Str,
                Literal::Bool(_) => Type::Bool,
                Literal::Unit => Type::Unit,
            };
            if !types_compatible(ctx, &lit_ty, scrutinee_ty) {
                return Err(TypeError::new(
                    format!(
                        "literal pattern of type {} does not match scrutinee type {}",
                        lit_ty.display(),
                        scrutinee_ty.display()
                    ),
                    *span,
                ));
            }
            Ok(())
        }
        Pattern::Tuple(parts, span) => match scrutinee_ty {
            Type::Tuple(elem_types) => {
                if parts.len() != elem_types.len() {
                    return Err(TypeError::new(
                        format!(
                            "tuple pattern of {} elements does not match scrutinee {} of {} elements",
                            parts.len(),
                            scrutinee_ty.display(),
                            elem_types.len()
                        ),
                        *span,
                    ));
                }
                for (pat, elem_ty) in parts.iter().zip(elem_types.iter()) {
                    check_pattern_against_type(ctx, pat, elem_ty)?;
                }
                Ok(())
            }
            Type::Var(_) => Ok(()),
            _ => Err(TypeError::new(
                format!(
                    "tuple pattern does not match scrutinee type {}",
                    scrutinee_ty.display()
                ),
                *span,
            )),
        },
        Pattern::Enum(enum_name, variant, sub_pats, span) => {
            // Option patterns match against `Type::Option(inner)`
            // rather than `Type::Enum`. The single sub-pattern for
            // `Some(p)` checks against the inner type; `None` has
            // no sub-pattern. Variants other than `Some` and `None`
            // are rejected.
            if enum_name == "Option" {
                if let Type::Option(inner_ty) = scrutinee_ty {
                    match variant.as_str() {
                        "None" => {
                            if !sub_pats.is_empty() {
                                return Err(TypeError::new(
                                    format!(
                                        "Option::None pattern has {} sub-patterns, expected zero",
                                        sub_pats.len()
                                    ),
                                    *span,
                                ));
                            }
                            return Ok(());
                        }
                        "Some" => {
                            if sub_pats.len() != 1 {
                                return Err(TypeError::new(
                                    format!(
                                        "Option::Some pattern has {} sub-patterns, expected one",
                                        sub_pats.len()
                                    ),
                                    *span,
                                ));
                            }
                            check_pattern_against_type(ctx, &sub_pats[0], inner_ty)?;
                            return Ok(());
                        }
                        other => {
                            return Err(TypeError::new(
                                format!("Option has no variant `{}`", other),
                                *span,
                            ));
                        }
                    }
                }
                if matches!(scrutinee_ty, Type::Var(_)) {
                    return Ok(());
                }
                return Err(TypeError::new(
                    format!(
                        "Option::{} pattern does not match scrutinee type {}",
                        variant,
                        scrutinee_ty.display()
                    ),
                    *span,
                ));
            }

            // Check enum name matches scrutinee.
            match scrutinee_ty {
                Type::Enum(scrutinee_name, _) if scrutinee_name == enum_name => {}
                Type::Var(_) => return Ok(()),
                _ => {
                    return Err(TypeError::new(
                        format!(
                            "enum pattern `{}::{}` does not match scrutinee type {}",
                            enum_name,
                            variant,
                            scrutinee_ty.display()
                        ),
                        *span,
                    ));
                }
            }
            // Check variant exists and arity matches.
            let payload = ctx
                .enums
                .get(enum_name)
                .and_then(|vs| vs.get(variant))
                .cloned()
                .ok_or_else(|| {
                    TypeError::new(
                        format!("enum `{}` has no variant `{}`", enum_name, variant),
                        *span,
                    )
                })?;
            if sub_pats.len() != payload.len() {
                return Err(TypeError::new(
                    format!(
                        "variant `{}::{}` expects {} payload elements, pattern has {}",
                        enum_name,
                        variant,
                        payload.len(),
                        sub_pats.len()
                    ),
                    *span,
                ));
            }
            for (sub_pat, payload_ty) in sub_pats.iter().zip(payload.iter()) {
                check_pattern_against_type(ctx, sub_pat, payload_ty)?;
            }
            Ok(())
        }
        Pattern::Struct(name, field_pats, span) => {
            match scrutinee_ty {
                Type::Struct(scrutinee_name, _) if scrutinee_name == name => {}
                Type::Var(_) => return Ok(()),
                _ => {
                    return Err(TypeError::new(
                        format!(
                            "struct pattern `{}` does not match scrutinee type {}",
                            name,
                            scrutinee_ty.display()
                        ),
                        *span,
                    ));
                }
            }
            let fields = ctx
                .structs
                .get(name)
                .cloned()
                .ok_or_else(|| TypeError::new(format!("unknown struct `{}`", name), *span))?;
            for field_pat in field_pats {
                let field_ty = fields.get(&field_pat.name).ok_or_else(|| {
                    TypeError::new(
                        format!("struct `{}` has no field `{}`", name, field_pat.name),
                        field_pat.span,
                    )
                })?;
                if let Some(pat) = &field_pat.pattern {
                    check_pattern_against_type(ctx, pat, field_ty)?;
                }
            }
            Ok(())
        }
    }
}

/// Check that a sequence of match arms exhaustively covers a
/// scrutinee's type. A wildcard or unbound variable arm satisfies
/// any type. For enum scrutinees, every variant must be covered.
/// For bool scrutinees, both true and false must be covered. For
/// other types, a wildcard or variable arm is required.
fn check_exhaustiveness(
    ctx: &Ctx,
    arms: &[MatchArm],
    scrutinee_ty: &Type,
    span: Span,
) -> Result<(), TypeError> {
    // A wildcard or variable pattern is a catch-all only when it is
    // unguarded; a guarded arm cannot prove coverage statically
    // because the guard's runtime value is not analysed here.
    let has_catchall = arms.iter().any(|arm| {
        arm.guard.is_none() && matches!(arm.pattern, Pattern::Wildcard(_) | Pattern::Variable(_, _))
    });
    if has_catchall {
        return Ok(());
    }
    match scrutinee_ty {
        Type::Bool => {
            let mut has_true = false;
            let mut has_false = false;
            for arm in arms {
                if arm.guard.is_some() {
                    continue;
                }
                if let Pattern::Literal(Literal::Bool(b), _) = &arm.pattern {
                    if *b {
                        has_true = true;
                    } else {
                        has_false = true;
                    }
                }
            }
            if !has_true || !has_false {
                return Err(TypeError::new(
                    String::from(
                        "non-exhaustive match on bool: needs both true and false arms or a wildcard",
                    ),
                    span,
                ));
            }
            Ok(())
        }
        Type::Enum(enum_name, _) => {
            let variants = ctx
                .enums
                .get(enum_name)
                .ok_or_else(|| TypeError::new(format!("unknown enum `{}`", enum_name), span))?;
            let mut covered: BTreeSet<String> = BTreeSet::new();
            for arm in arms {
                if arm.guard.is_some() {
                    continue;
                }
                if let Pattern::Enum(en, variant, _, _) = &arm.pattern
                    && en == enum_name
                {
                    covered.insert(variant.clone());
                }
            }
            let missing: Vec<&String> = variants.keys().filter(|k| !covered.contains(*k)).collect();
            if !missing.is_empty() {
                let names: Vec<String> = missing.iter().map(|s| (*s).clone()).collect();
                return Err(TypeError::new(
                    format!(
                        "non-exhaustive match on enum `{}`: missing variant(s) {}",
                        enum_name,
                        names.join(", ")
                    ),
                    span,
                ));
            }
            Ok(())
        }
        Type::Unit => {
            // Unit has only one value. A literal Unit pattern or a
            // variable/wildcard arm covers it. We checked for
            // catchall above, so check for a Unit literal arm.
            let has_unit_lit = arms.iter().any(|arm| {
                arm.guard.is_none() && matches!(arm.pattern, Pattern::Literal(Literal::Unit, _))
            });
            if has_unit_lit {
                Ok(())
            } else {
                Err(TypeError::new(
                    String::from("non-exhaustive match on (): requires `()` or wildcard arm"),
                    span,
                ))
            }
        }
        Type::Option(_) => {
            // Option has exactly two variants: Some and None.
            // The match is exhaustive when both are covered.
            let mut has_some = false;
            let mut has_none = false;
            for arm in arms {
                if arm.guard.is_some() {
                    continue;
                }
                if let Pattern::Enum(name, variant, _, _) = &arm.pattern
                    && name == "Option"
                {
                    match variant.as_str() {
                        "Some" => has_some = true,
                        "None" => has_none = true,
                        _ => {}
                    }
                }
            }
            if !has_some || !has_none {
                let mut missing: Vec<&'static str> = Vec::new();
                if !has_some {
                    missing.push("Some");
                }
                if !has_none {
                    missing.push("None");
                }
                return Err(TypeError::new(
                    format!(
                        "non-exhaustive match on Option: missing variant(s) {}",
                        missing.join(", ")
                    ),
                    span,
                ));
            }
            Ok(())
        }
        Type::Var(_) => Ok(()),
        other => Err(TypeError::new(
            format!(
                "non-exhaustive match on {}: requires a wildcard arm",
                other.display()
            ),
            span,
        )),
    }
}

fn type_of_block(ctx: &mut Ctx, block: &mut Block) -> Result<Type, TypeError> {
    ctx.push_scope();
    for stmt in block.stmts.iter_mut() {
        check_stmt(ctx, stmt)?;
    }
    let ty = match block.tail_expr.as_mut() {
        Some(e) => type_of_expr(ctx, e)?,
        None => Type::Unit,
    };
    ctx.pop_scope();
    Ok(ty)
}

fn check_stmt(ctx: &mut Ctx, stmt: &mut Stmt) -> Result<(), TypeError> {
    match stmt {
        Stmt::Let(let_stmt) => {
            // Self-referential closure binding: `let f = |...| ... f(...) ...`.
            // The closure body may reference the binding name. Register a
            // fresh type variable for the binding before checking the
            // value so the body's reference resolves rather than failing
            // with "undefined function". The hoist pass later detects the
            // self-reference and synthesizes a recursive ClosureRef.
            if let Pattern::Variable(name, _) = &let_stmt.pattern
                && let Expr::Closure { .. } = &let_stmt.value
                && let Some(scope) = ctx.locals.last_mut()
            {
                let pre_ty = ctx.vargen.fresh();
                scope.insert(name.clone(), pre_ty);
            }
            // Let-binding type annotations forbid the negative-
            // label wrapper at any depth. Negatives are admissible
            // only at the top level of function parameter and
            // return type positions; a `let x: Word@!Secret = ...`
            // is rejected at the annotation span.
            if let Some(t) = &let_stmt.type_expr {
                validate_no_nested_negative_labels(t, false)?;
            }
            // Bidirectional check: if the binding has a type annotation,
            // resolve it and push onto the expected-type stack so that
            // refinement-driven keywords (saturate_max / saturate_min)
            // inside the value can consult it. Pop after the value is
            // checked regardless of outcome.
            let declared_pre: Option<Type> =
                let_stmt.type_expr.as_ref().map(|t| ctx.resolve_type(t));
            if let Some(d) = &declared_pre {
                ctx.push_expected(d.clone());
            }
            let value_result = type_of_expr(ctx, &mut let_stmt.value);
            if declared_pre.is_some() {
                ctx.pop_expected();
            }
            let value_ty = value_result?;
            let bound_ty = match declared_pre {
                Some(declared) => {
                    if !types_compatible(ctx, &declared, &value_ty) {
                        return Err(TypeError::new(
                            format!(
                                "let binding declared as {} but value has type {}",
                                declared.display(),
                                value_ty.display()
                            ),
                            let_stmt.span,
                        ));
                    }
                    declared
                }
                None => value_ty,
            };
            bind_pattern(ctx, &let_stmt.pattern, bound_ty);
            Ok(())
        }
        Stmt::For(for_stmt) => {
            let elem_ty = match &mut for_stmt.iterable {
                Iterable::Range(start, end) => {
                    let s = type_of_expr(ctx, start)?;
                    let e = type_of_expr(ctx, end)?;
                    if !types_compatible(ctx, &s, &Type::Word)
                        || !types_compatible(ctx, &e, &Type::Word)
                    {
                        return Err(TypeError::new(
                            format!(
                                "for-range bounds must be Word, got {} and {}",
                                s.display(),
                                e.display()
                            ),
                            for_stmt.span,
                        ));
                    }
                    Type::Word
                }
                Iterable::Expr(e) => match type_of_expr(ctx, e)? {
                    Type::Array(inner, _) => *inner,
                    other => {
                        return Err(TypeError::new(
                            format!("for-in expects an array, got {}", other.display()),
                            for_stmt.span,
                        ));
                    }
                },
            };
            ctx.push_scope();
            ctx.add_local(for_stmt.var.clone(), elem_ty);
            let _ = type_of_block(ctx, &mut for_stmt.body)?;
            ctx.pop_scope();
            Ok(())
        }
        Stmt::Break(_) => Ok(()),
        Stmt::DataFieldAssign {
            data_name,
            field,
            value,
            span,
        } => {
            let data_fields = ctx.data.get(data_name).ok_or_else(|| {
                TypeError::new(format!("unknown data block `{}`", data_name), *span)
            })?;
            let declared = data_fields.get(field).ok_or_else(|| {
                TypeError::new(
                    format!("unknown field `{}` on data block `{}`", field, data_name),
                    *span,
                )
            })?;
            let declared = declared.clone();
            let negatives = ctx
                .data_negative_labels
                .get(data_name)
                .and_then(|m| m.get(field))
                .cloned()
                .unwrap_or_default();
            let value_ty = type_of_expr(ctx, value)?;
            if !types_compatible(ctx, &declared, &value_ty) {
                return Err(TypeError::new(
                    format!(
                        "assignment to `{}.{}` expects {}, got {}",
                        data_name,
                        field,
                        declared.display(),
                        value_ty.display()
                    ),
                    *span,
                ));
            }
            check_negative_labels_against_data_write(
                data_name, field, &value_ty, &negatives, *span,
            )?;
            Ok(())
        }
        Stmt::DataFieldIndexAssign {
            data_name,
            field,
            indices,
            value,
            span,
        } => {
            let data_fields = ctx.data.get(data_name).ok_or_else(|| {
                TypeError::new(format!("unknown data block `{}`", data_name), *span)
            })?;
            let declared = data_fields
                .get(field)
                .ok_or_else(|| {
                    TypeError::new(
                        format!("unknown field `{}` on data block `{}`", field, data_name),
                        *span,
                    )
                })?
                .clone();
            // Peel one Array layer per index, validating each index
            // as a Word and ensuring the final type is a scalar.
            let mut current = declared;
            for idx in indices.iter_mut() {
                let idx_ty = type_of_expr(ctx, idx)?;
                if !types_compatible(ctx, &idx_ty, &Type::Word) {
                    return Err(TypeError::new(
                        format!("data array index must be Word, got {}", idx_ty.display()),
                        *span,
                    ));
                }
                current = match current {
                    Type::Array(elem, _) => *elem,
                    other => {
                        return Err(TypeError::new(
                            format!(
                                "indexed access on non-array data field `{}.{}` (type {})",
                                data_name,
                                field,
                                other.display()
                            ),
                            *span,
                        ));
                    }
                };
            }
            if let Type::Array(_, _) = current {
                return Err(TypeError::new(
                    format!(
                        "indexed assignment to `{}.{}` does not descend to a scalar; \
                         provide one index per array level",
                        data_name, field
                    ),
                    *span,
                ));
            }
            let value_ty = type_of_expr(ctx, value)?;
            if !types_compatible(ctx, &current, &value_ty) {
                return Err(TypeError::new(
                    format!(
                        "indexed assignment to `{}.{}` expects {}, got {}",
                        data_name,
                        field,
                        current.display(),
                        value_ty.display()
                    ),
                    *span,
                ));
            }
            // Negative-label boundary check at the indexed-write
            // site. The field's negative-label set is declared on
            // the outer array type; per the no-nested-negative-
            // labels rule it cannot appear on inner array element
            // positions, so the check uses the field-level set.
            let negatives = ctx
                .data_negative_labels
                .get(data_name)
                .and_then(|m| m.get(field))
                .cloned()
                .unwrap_or_default();
            check_negative_labels_against_data_write(
                data_name, field, &value_ty, &negatives, *span,
            )?;
            Ok(())
        }
        Stmt::Expr(e) => {
            let _ = type_of_expr(ctx, e)?;
            Ok(())
        }
        Stmt::Assert { cond, span, .. } => {
            let cond_ty = type_of_expr(ctx, cond)?;
            if !types_compatible(ctx, &strip_labels(cond_ty.clone()), &Type::Bool) {
                return Err(TypeError::new(
                    alloc::format!("assert condition must be bool, got {}", cond_ty.display()),
                    *span,
                ));
            }
            Ok(())
        }
    }
}

/// Infer an expression's type, and (when the recording pass is active)
/// record its resolved type into the current function's authoritative table
/// keyed by the expression's span (B28 P3 item 5).
///
/// Recursive calls go through this wrapper, so every sub-expression is
/// recorded. The type is resolved with the current substitution and converted
/// to a `TypeExpr`; a type that does not fully resolve is skipped. If two
/// distinct expressions share a span (a synthetic node aliasing a source span)
/// and produce different concrete types, the span is marked conflicting and
/// excluded, so the table never hands the compiler a wrong type.
fn type_of_expr(ctx: &mut Ctx, expr: &mut Expr) -> Result<Type, TypeError> {
    let ty = type_of_expr_inner(ctx, expr)?;
    if ctx.record_types {
        let span = expr.span();
        let resolved = ty.apply(&ctx.subst);
        if let Some(te) = type_to_expr_full(&resolved, span)
            && !ctx.fn_type_conflicts.contains(&span)
        {
            match ctx.current_fn_types.get(&span) {
                Some(existing) if *existing != te => {
                    ctx.current_fn_types.remove(&span);
                    ctx.fn_type_conflicts.insert(span);
                }
                _ => {
                    ctx.current_fn_types.insert(span, te);
                }
            }
        }
    }
    Ok(ty)
}

fn type_of_expr_inner(ctx: &mut Ctx, expr: &mut Expr) -> Result<Type, TypeError> {
    match expr {
        Expr::Literal { value, .. } => Ok(match value {
            Literal::Int(_) => Type::Word,
            Literal::Float(_) => Type::Float,
            Literal::Byte(_) => Type::Byte,
            Literal::Fixed { frac_bits, .. } => Type::Fixed(*frac_bits),
            Literal::String(_) => Type::Str,
            Literal::Bool(_) => Type::Bool,
            Literal::Unit => Type::Unit,
        }),
        Expr::Ident { name, span } => {
            // Local variable shadows function name lookup.
            if let Some(ty) = ctx.lookup_local(name) {
                return Ok(ty.clone());
            }
            // Data block field access uses dotted form, parsed as Ident
            // for `data_name` followed by a FieldAccess. So a bare
            // ident matching a data block name is admissible only as
            // the receiver of subsequent field access. Surface it as a
            // synthetic Struct type whose name is the data block name.
            if ctx.data.contains_key(name) {
                return Ok(Type::Struct(name.clone(), Vec::new()));
            }
            // Bare function name reference. Report unknown.
            Err(TypeError::new(
                format!("undefined identifier `{}`", name),
                *span,
            ))
        }
        Expr::BinOp {
            op,
            left,
            right,
            span,
        } => {
            let lt_raw = type_of_expr(ctx, left)?;
            let rt_raw = type_of_expr(ctx, right)?;
            // Integer-literal coercion. When one operand is a
            // bare integer literal typed as `Word` and the other
            // is `Byte` or `Fixed<N>`, wrap the literal in a cast
            // so the operator's existing same-type dispatch sees
            // matching types. The mutation preserves the source's
            // intent; the compiler emits a cast opcode (a no-op
            // for Word-to-Byte truncation in range and an integer-
            // to-fixed shift for Fixed targets). Out-of-range
            // literals fall through to the regular type-error
            // path.
            // A shift is asymmetric: the amount is always a `Word`, so the
            // operands are not coerced to a common type. Coercing the
            // amount literal to a `Byte` value's type would wrongly reject
            // a `Byte` shift by an integer literal.
            let is_shift = matches!(op, BinOp::Shl | BinOp::AShl | BinOp::ShrA | BinOp::ShrL);
            let lt_raw = if is_shift {
                lt_raw
            } else {
                coerce_integer_literal(ctx, left, lt_raw, &rt_raw, *span)
            };
            let rt_raw = if is_shift {
                rt_raw
            } else {
                coerce_integer_literal(ctx, right, rt_raw, &lt_raw, *span)
            };
            // Information-flow label propagation. The operands'
            // labels are unioned to form the result's labels;
            // arithmetic on a labeled value taints the result.
            // Structural dispatch below operates on label-stripped
            // types so the existing match arms see through
            // `Labelled` wrappers.
            let combined_labels: BTreeSet<String> = labels_of(&lt_raw)
                .union(&labels_of(&rt_raw))
                .cloned()
                .collect();
            let lt = strip_labels(lt_raw);
            let rt = strip_labels(rt_raw);
            // Shift operators. The value being shifted is a Word, Byte, or
            // Multiword<N, F>, and the shift amount is a Word, so the two
            // operands are asymmetric and the shift is handled before the
            // same-type arithmetic below. The result has the value's type.
            if matches!(op, BinOp::Shl | BinOp::AShl | BinOp::ShrA | BinOp::ShrL) {
                let value_ok = matches!(
                    lt,
                    Type::Word | Type::Byte | Type::Multiword(_, _) | Type::Var(_)
                );
                let amount_ok = matches!(rt, Type::Word | Type::Var(_));
                if value_ok && amount_ok {
                    return Ok(apply_labels(lt, &combined_labels));
                }
                return Err(TypeError::new(
                    format!(
                        "cannot shift {} by {}; the value must be a Word or Multiword and the amount a Word",
                        lt.display(),
                        rt.display()
                    ),
                    *span,
                ));
            }
            // Multiword<N, F> arithmetic and comparison. Add, Sub, and the
            // six comparisons are implemented (phase 2); integer multiply
            // (F = 0) is implemented (phase 3a). Fixed-point multiply
            // (F > 0), divide, modulo, and the shifts are later phases
            // (B19). Both operands must share N and F.
            if let (Type::Multiword(ln, lf), Type::Multiword(rn, rf)) = (&lt, &rt) {
                if !const_dims_compatible(ln, rn) || !const_dims_compatible(lf, rf) {
                    return Err(TypeError::new(
                        format!(
                            "cannot apply operator to {} and {}",
                            lt.display(),
                            rt.display()
                        ),
                        *span,
                    ));
                }
                let bare = match op {
                    BinOp::Add | BinOp::Sub => Type::Multiword(ln.clone(), lf.clone()),
                    // Multiply is scale-preserving in the type: the integer
                    // case (F = 0) truncates to N words and the fixed-point
                    // case (F > 0) shifts the double-width product right by
                    // F, both yielding a Multiword<N, F>.
                    BinOp::Mul => Type::Multiword(ln.clone(), lf.clone()),
                    // Divide and modulo are scale-preserving in the type.
                    // Integer divide and modulo (F = 0) and the fixed-point
                    // divide (F > 0, which pre-shifts the dividend by F) and
                    // modulo (F > 0, the raw remainder, scale-preserving)
                    // all yield a Multiword<N, F>.
                    BinOp::Div | BinOp::Mod => Type::Multiword(ln.clone(), lf.clone()),
                    // Bitwise operations act per word and preserve the type.
                    BinOp::Band | BinOp::Bor | BinOp::Bxor => {
                        Type::Multiword(ln.clone(), lf.clone())
                    }
                    BinOp::Eq
                    | BinOp::NotEq
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::LtEq
                    | BinOp::GtEq => Type::Bool,
                    _ => {
                        return Err(TypeError::new(
                            format!("{} does not yet support this operator", lt.display()),
                            *span,
                        ));
                    }
                };
                return Ok(apply_labels(bare, &combined_labels));
            }
            let bare_result = match op {
                BinOp::Add => {
                    if matches!(lt, Type::Word) && matches!(rt, Type::Word) {
                        Ok(Type::Word)
                    } else if matches!(lt, Type::Byte) && matches!(rt, Type::Byte) {
                        Ok(Type::Byte)
                    } else if let (Type::Fixed(ln), Type::Fixed(rn)) = (&lt, &rt) {
                        if ln == rn {
                            Ok(Type::Fixed(*ln))
                        } else {
                            Err(TypeError::new(
                                format!("cannot add {} and {}", lt.display(), rt.display()),
                                *span,
                            ))
                        }
                    } else if matches!(lt, Type::Float) && matches!(rt, Type::Float) {
                        Ok(Type::Float)
                    } else if matches!(lt, Type::Str) && matches!(rt, Type::Str) {
                        Ok(Type::Str)
                    } else if matches!(lt, Type::Var(_)) || matches!(rt, Type::Var(_)) {
                        Ok(ctx.fresh())
                    } else {
                        Err(TypeError::new(
                            format!("cannot add {} and {}", lt.display(), rt.display()),
                            *span,
                        ))
                    }
                }
                BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    if matches!(lt, Type::Word) && matches!(rt, Type::Word) {
                        Ok(Type::Word)
                    } else if matches!(lt, Type::Byte) && matches!(rt, Type::Byte) {
                        Ok(Type::Byte)
                    } else if let (Type::Fixed(ln), Type::Fixed(rn)) = (&lt, &rt) {
                        if ln == rn {
                            Ok(Type::Fixed(*ln))
                        } else {
                            Err(TypeError::new(
                                format!(
                                    "arithmetic on incompatible types {} and {}",
                                    lt.display(),
                                    rt.display()
                                ),
                                *span,
                            ))
                        }
                    } else if matches!(lt, Type::Float) && matches!(rt, Type::Float) {
                        Ok(Type::Float)
                    } else if matches!(lt, Type::Var(_)) || matches!(rt, Type::Var(_)) {
                        Ok(ctx.fresh())
                    } else {
                        Err(TypeError::new(
                            format!(
                                "arithmetic on incompatible types {} and {}",
                                lt.display(),
                                rt.display()
                            ),
                            *span,
                        ))
                    }
                }
                BinOp::Eq | BinOp::NotEq => {
                    if !types_compatible(ctx, &lt, &rt) {
                        return Err(TypeError::new(
                            format!("cannot compare {} and {}", lt.display(), rt.display()),
                            *span,
                        ));
                    }
                    Ok(Type::Bool)
                }
                BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                    if !types_compatible(ctx, &lt, &rt) {
                        return Err(TypeError::new(
                            format!("cannot order {} and {}", lt.display(), rt.display()),
                            *span,
                        ));
                    }
                    Ok(Type::Bool)
                }
                BinOp::And | BinOp::Or | BinOp::Xor | BinOp::Andalso | BinOp::Orelse => {
                    if !types_compatible(ctx, &lt, &Type::Bool)
                        || !types_compatible(ctx, &rt, &Type::Bool)
                    {
                        return Err(TypeError::new(
                            format!(
                                "logical operator requires bool operands, got {} and {}",
                                lt.display(),
                                rt.display()
                            ),
                            *span,
                        ));
                    }
                    Ok(Type::Bool)
                }
                BinOp::Band | BinOp::Bor | BinOp::Bxor => {
                    if matches!(lt, Type::Word) && matches!(rt, Type::Word) {
                        Ok(Type::Word)
                    } else if matches!(lt, Type::Byte) && matches!(rt, Type::Byte) {
                        Ok(Type::Byte)
                    } else if matches!(lt, Type::Var(_)) || matches!(rt, Type::Var(_)) {
                        Ok(ctx.fresh())
                    } else {
                        Err(TypeError::new(
                            format!(
                                "bitwise operator requires two Word, Byte, or Multiword operands, got {} and {}",
                                lt.display(),
                                rt.display()
                            ),
                            *span,
                        ))
                    }
                }
                BinOp::Shl | BinOp::AShl | BinOp::ShrA | BinOp::ShrL => {
                    unreachable!("shift operators are handled before the same-type arithmetic")
                }
            };
            // Re-apply the union of operand labels to the
            // structural result. If both operands were unlabeled,
            // the union is empty and the result is unchanged.
            bare_result.map(|t| apply_labels(t, &combined_labels))
        }
        Expr::UnaryOp { op, operand, span } => {
            let ty_raw = type_of_expr(ctx, operand)?;
            let labels = labels_of(&ty_raw);
            let ty = strip_labels(ty_raw);
            let bare_result = match op {
                UnaryOp::Neg => match ty {
                    Type::Word | Type::Byte | Type::Fixed(_) | Type::Float | Type::Var(_) => Ok(ty),
                    other => Err(TypeError::new(
                        format!("cannot negate {}", other.display()),
                        *span,
                    )),
                },
                UnaryOp::Not => {
                    if !types_compatible(ctx, &ty, &Type::Bool) {
                        return Err(TypeError::new(
                            format!("`not` requires bool, got {}", ty.display()),
                            *span,
                        ));
                    }
                    Ok(Type::Bool)
                }
                UnaryOp::Bnot => match ty {
                    Type::Word | Type::Byte | Type::Multiword(_, _) | Type::Var(_) => Ok(ty),
                    other => Err(TypeError::new(
                        format!(
                            "`bnot` requires a Word, Byte, or Multiword, got {}",
                            other.display()
                        ),
                        *span,
                    )),
                },
            };
            bare_result.map(|t| apply_labels(t, &labels))
        }
        Expr::Call {
            name,
            args,
            span,
            const_args,
        } => {
            // Const-generic arity: a call to a function with const
            // parameters must supply exactly that many const arguments
            // through a turbofish, and a turbofish is admissible only on
            // such a function. Each const argument must be a compile-time
            // constant (a literal or an in-scope const parameter) (B40).
            match ctx.functions.get(name).map(|s| s.const_params.len()) {
                Some(k) if k > 0 || !const_args.is_empty() => {
                    if const_args.len() != k {
                        return Err(TypeError::new(
                            alloc::format!(
                                "function `{}` takes {} const argument(s) but {} were supplied",
                                name,
                                k,
                                const_args.len()
                            ),
                            *span,
                        ));
                    }
                }
                _ if !const_args.is_empty() => {
                    return Err(TypeError::new(
                        alloc::format!(
                            "`{}` is not a const-generic function; a const turbofish `::<...>` is not admissible here",
                            name
                        ),
                        *span,
                    ));
                }
                _ => {}
            }
            for ca in const_args.iter() {
                check_const_arg(ctx, ca)?;
            }
            // If `name` resolves to a local first, this is an indirect
            // call through a function value (closure). Type-check the
            // arguments and return a fresh type for the result. The
            // compiler emits `Op::CallIndirect`.
            if ctx.lookup_local(name).is_some() {
                for arg in args {
                    type_of_expr(ctx, arg)?;
                }
                return Ok(ctx.fresh());
            }
            // Native functions are registered at runtime. Names
            // declared in `use` or qualified with `::` are treated as
            // natives. If the `use` declaration carries a signature
            // (`use host::name(T1, T2, ...) -> R`), the call site
            // enforces the declared parameter arity, parameter types,
            // and assigns the declared return type. Native names
            // without a declared signature continue to be accepted
            // with any argument types and a fresh return-type
            // variable; this is the legacy permissive path.
            // Newtype construction. `Name(value)` where `Name` is a
            // declared newtype constructs a value of the newtype.
            // The argument must match the newtype's underlying type;
            // the resulting expression has type `Newtype(name,
            // underlying)`. The bytecode emitted at the compiler
            // layer is just the inner expression's value, because
            // newtypes are transparent at the runtime level.
            if let Some(TypeKind::Newtype) = ctx.types.get(name) {
                if args.len() != 1 {
                    return Err(TypeError::new(
                        alloc::format!(
                            "newtype `{}` constructor expects 1 argument, got {}",
                            name,
                            args.len()
                        ),
                        *span,
                    ));
                }
                let underlying = match ctx.newtypes.get(name).cloned() {
                    Some(ty) => ty,
                    None => ctx.fresh(),
                };
                let arg_ty = type_of_expr(ctx, &mut args[0])?;
                if !types_compatible(ctx, &arg_ty, &underlying) {
                    return Err(TypeError::new(
                        alloc::format!(
                            "newtype `{}` constructor expects {}, got {}",
                            name,
                            underlying.display(),
                            arg_ty.display()
                        ),
                        args[0].span(),
                    ));
                }
                let _ = underlying;
                return Ok(Type::Newtype(name.clone()));
            }
            let sig = match ctx.functions.get(name).cloned() {
                Some(s) => s,
                None => {
                    if let Some(nsig) = ctx.native_signatures.get(name).cloned() {
                        return check_native_call_with_signature(ctx, name, args, span, &nsig);
                    }
                    if ctx.natives.contains(name) || name.contains("::") {
                        // Type-check arguments for syntax errors but
                        // do not enforce signatures.
                        for arg in args {
                            type_of_expr(ctx, arg)?;
                        }
                        return Ok(ctx.fresh());
                    }
                    return Err(TypeError::new(
                        format!("undefined function `{}`", name),
                        *span,
                    ));
                }
            };
            if args.len() != sig.params.len() {
                return Err(TypeError::new(
                    format!(
                        "function `{}` expects {} arguments, got {}",
                        name,
                        sig.params.len(),
                        args.len()
                    ),
                    *span,
                ));
            }
            // Instantiate generic type parameters with fresh per-call
            // type variables before unifying with actual argument
            // types. For non-generic functions this is a no-op clone.
            let (inst_params, inst_return, fresh_vars) = instantiate_sig(ctx, &sig);
            for (i, (arg, param_ty)) in args.iter_mut().zip(inst_params.iter()).enumerate() {
                let arg_ty = type_of_expr(ctx, arg)?;
                let has_negatives = sig
                    .param_negative_labels
                    .get(i)
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                // When the parameter declares negative labels,
                // the positive-label upper-bound rule is relaxed
                // (any positive labels except the explicitly
                // forbidden ones are admissible). The
                // negative-disjoint clause runs separately below.
                let (structural_arg, structural_param) = if has_negatives {
                    (strip_labels(arg_ty.clone()), strip_labels(param_ty.clone()))
                } else {
                    (arg_ty.clone(), param_ty.clone())
                };
                if !types_compatible(ctx, &structural_arg, &structural_param) {
                    return Err(TypeError::new(
                        format!(
                            "argument to `{}` expects {}, got {}",
                            name,
                            param_ty.display(),
                            arg_ty.display()
                        ),
                        arg.span(),
                    ));
                }
                // Negative-label boundary clause. The parameter's
                // declared `!Label` set forbids the argument from
                // carrying any of those labels in its positive set.
                check_negative_labels_against_arg(
                    name,
                    i,
                    &arg_ty,
                    &sig.param_negative_labels,
                    arg.span(),
                )?;
            }
            // Validate trait bounds on type parameters now that the
            // substitution has been recorded by argument unification.
            // For each bounded type parameter, resolve the per-call
            // fresh variable through the active substitution and
            // check that the resulting head type implements every
            // required trait. Unresolved variables (the parameter
            // could not be inferred from arguments) skip the check;
            // concrete types are validated.
            for (var, bounds) in fresh_vars.iter().zip(sig.type_param_bounds.iter()) {
                if bounds.is_empty() {
                    continue;
                }
                let resolved = var.apply(&ctx.subst);
                let head = match type_head_name(&resolved) {
                    Some(h) => h,
                    None => continue,
                };
                for bound in bounds {
                    let satisfies = ctx
                        .impls
                        .get(bound)
                        .map(|set| set.contains(&head))
                        .unwrap_or(false);
                    if !satisfies {
                        return Err(TypeError::new(
                            format!(
                                "type `{}` does not implement trait `{}` required by `{}`",
                                head, bound, name
                            ),
                            *span,
                        ));
                    }
                }
            }
            Ok(inst_return)
        }
        Expr::Pipeline {
            left,
            func,
            args,
            span,
        } => {
            let left_ty = type_of_expr(ctx, left)?;
            // A pipeline desugars to func(left, args...). For the
            // checker we look up func by name and validate args+1.
            if let Some(sig) = ctx.functions.get(func).cloned() {
                if sig.params.len() != args.len() + 1 {
                    return Err(TypeError::new(
                        format!(
                            "pipeline target `{}` expects {} arguments, got {}",
                            func,
                            sig.params.len(),
                            args.len() + 1
                        ),
                        expr.span(),
                    ));
                }
                if let Some(first_param) = sig.params.first()
                    && !types_compatible(ctx, &left_ty, first_param)
                {
                    return Err(TypeError::new(
                        format!(
                            "pipeline left side has type {} but `{}` expects {}",
                            left_ty.display(),
                            func,
                            first_param.display()
                        ),
                        expr.span(),
                    ));
                }
                for (arg, param_ty) in args.iter_mut().zip(sig.params.iter().skip(1)) {
                    let arg_ty = type_of_expr(ctx, arg)?;
                    if !types_compatible(ctx, &arg_ty, param_ty) {
                        return Err(TypeError::new(
                            format!(
                                "argument to `{}` expects {}, got {}",
                                func,
                                param_ty.display(),
                                arg_ty.display()
                            ),
                            arg.span(),
                        ));
                    }
                }
                Ok(sig.return_type)
            } else if ctx.natives.contains(func) || func.contains("::") {
                // Native pipeline target. Accept arguments without
                // signature check.
                for arg in args {
                    let _ = type_of_expr(ctx, arg)?;
                }
                Ok(ctx.fresh())
            } else {
                Err(TypeError::new(
                    format!("undefined function `{}`", func),
                    *span,
                ))
            }
        }
        Expr::Yield { value, span } => {
            let value_ty = type_of_expr(ctx, value)?;
            // The yielded value crosses the script-to-host
            // boundary as an output. The active function's
            // return-side negative-label set applies: a yielded
            // value cannot carry a label the function declares it
            // never returns.
            let return_negatives = ctx.current_return_negative_labels.clone();
            check_negative_labels_against_return(
                "yielded value",
                &value_ty,
                &return_negatives,
                *span,
            )?;
            // Yield's expression value (received from host on resume)
            // cannot be statically typed without dialogue annotations.
            Ok(ctx.fresh())
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            span,
        } => {
            let cond_ty_raw = type_of_expr(ctx, condition)?;
            let cond_labels = labels_of(&cond_ty_raw);
            let cond_ty = strip_labels(cond_ty_raw);
            if !types_compatible(ctx, &cond_ty, &Type::Bool) {
                return Err(TypeError::new(
                    format!("if condition must be bool, got {}", cond_ty.display()),
                    *span,
                ));
            }
            let then_ty = type_of_block(ctx, then_block)?;
            let result_ty = match else_block {
                Some(b) => {
                    let else_ty = type_of_block(ctx, b)?;
                    // Branches must unify structurally; their
                    // labels are unioned to form the result's
                    // labels. The structural check ignores
                    // labels because two branches with the same
                    // underlying type but different labels are
                    // legitimately combinable into a labeled
                    // result.
                    let then_bare = strip_labels(then_ty.clone());
                    let else_bare = strip_labels(else_ty.clone());
                    if unify(&then_bare, &else_bare, &mut ctx.subst).is_err() {
                        return Err(TypeError::new(
                            format!(
                                "if branches have differing types {} and {}",
                                then_ty.display(),
                                else_ty.display()
                            ),
                            *span,
                        ));
                    }
                    let branch_labels: BTreeSet<String> = labels_of(&then_ty)
                        .union(&labels_of(&else_ty))
                        .cloned()
                        .collect();
                    apply_labels(then_bare, &branch_labels)
                }
                None => Type::Unit,
            };
            // Branching condition taint. The condition's labels
            // propagate to the result because an observer of the
            // result can infer information about the condition
            // (which arm fired).
            Ok(apply_labels(result_ty, &cond_labels))
        }
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => {
            let scrutinee_ty_raw = type_of_expr(ctx, scrutinee)?;
            let scrutinee_labels = labels_of(&scrutinee_ty_raw);
            let scrutinee_ty = strip_labels(scrutinee_ty_raw);
            // Type the body of each arm. The arm bodies must agree.
            let mut common: Option<Type> = None;
            let mut arm_labels: BTreeSet<String> = BTreeSet::new();
            for arm in arms.iter_mut() {
                check_pattern_against_type(ctx, &arm.pattern, &scrutinee_ty)?;
                ctx.push_scope();
                bind_pattern(ctx, &arm.pattern, scrutinee_ty.clone());
                // Guard expression must evaluate to Bool. The guard
                // is checked in the scope of the pattern's bindings
                // so it can refer to bound names.
                if let Some(guard) = arm.guard.as_mut() {
                    let guard_ty = type_of_expr(ctx, guard)?;
                    if !types_compatible(ctx, &strip_labels(guard_ty.clone()), &Type::Bool) {
                        ctx.pop_scope();
                        return Err(TypeError::new(
                            alloc::format!(
                                "match-arm guard must be Bool, got {}",
                                guard_ty.display()
                            ),
                            arm.span,
                        ));
                    }
                }
                let arm_ty = type_of_expr(ctx, &mut arm.expr)?;
                ctx.pop_scope();
                arm_labels = arm_labels.union(&labels_of(&arm_ty)).cloned().collect();
                let arm_ty_bare = strip_labels(arm_ty);
                match &common {
                    None => common = Some(arm_ty_bare),
                    Some(c) => {
                        // Arms unify structurally; labels are
                        // aggregated above into `arm_labels`.
                        if unify(c, &arm_ty_bare, &mut ctx.subst).is_err() {
                            return Err(TypeError::new(
                                format!(
                                    "match arms have differing types {} and {}",
                                    c.display(),
                                    arm_ty_bare.display()
                                ),
                                arm.span,
                            ));
                        }
                    }
                }
            }
            check_exhaustiveness(ctx, arms, &scrutinee_ty, *span)?;
            let bare = common.unwrap_or(Type::Unit);
            // The result carries the union of all arm labels
            // (any arm could fire) plus the scrutinee labels
            // (observing which arm fired discloses information
            // about the scrutinee).
            let combined: BTreeSet<String> = arm_labels.union(&scrutinee_labels).cloned().collect();
            Ok(apply_labels(bare, &combined))
        }
        Expr::Loop { body, .. } => {
            let _ = type_of_block(ctx, body)?;
            Ok(Type::Unit)
        }
        Expr::FieldAccess {
            object,
            field,
            span,
        } => {
            let obj_ty = type_of_expr(ctx, object)?;
            match obj_ty {
                Type::Struct(ref name, ref args) => {
                    // Apply the per-instance type argument substitution
                    // to the field's declared type so generic structs
                    // reflect the concrete instantiation at this access.
                    let abstract_vars = ctx
                        .struct_type_param_vars
                        .get(name)
                        .cloned()
                        .unwrap_or_default();
                    let mut inst = Subst::new();
                    for (abstract_var, concrete) in abstract_vars.iter().zip(args.iter()) {
                        if let Type::Var(v) = abstract_var {
                            inst.insert(*v, concrete.clone());
                        }
                    }
                    if let Some(fields) = ctx.structs.get(name)
                        && let Some(t) = fields.get(field)
                    {
                        return Ok(t.apply(&inst));
                    }
                    if let Some(fields) = ctx.data.get(name)
                        && let Some(t) = fields.get(field)
                    {
                        return Ok(t.clone());
                    }
                    Err(TypeError::new(
                        format!("type {} has no field `{}`", obj_ty.display(), field),
                        *span,
                    ))
                }
                Type::Var(_) => Ok(ctx.fresh()),
                other => Err(TypeError::new(
                    format!("field access on non-struct type {}", other.display()),
                    *span,
                )),
            }
        }
        Expr::TupleIndex {
            object,
            index,
            span,
        } => {
            let obj_ty = type_of_expr(ctx, object)?;
            match obj_ty {
                Type::Tuple(elems) => {
                    if (*index as usize) < elems.len() {
                        Ok(elems[*index as usize].clone())
                    } else {
                        Err(TypeError::new(
                            format!(
                                "tuple index {} out of bounds for {}",
                                index,
                                Type::Tuple(elems).display()
                            ),
                            *span,
                        ))
                    }
                }
                other => Err(TypeError::new(
                    format!("tuple index on non-tuple type {}", other.display()),
                    *span,
                )),
            }
        }
        Expr::ArrayIndex {
            object,
            index,
            span,
        } => {
            let obj_ty = type_of_expr(ctx, object)?;
            let idx_ty = type_of_expr(ctx, index)?;
            if !types_compatible(ctx, &idx_ty, &Type::Word) {
                return Err(TypeError::new(
                    format!("array index must be Word, got {}", idx_ty.display()),
                    *span,
                ));
            }
            match obj_ty {
                Type::Array(inner, _) => Ok(*inner),
                // Indexing a Multiword<N> yields the i-th Word digit,
                // little-endian, with the same runtime bounds check as
                // an array (B19).
                Type::Multiword(_, _) => Ok(Type::Word),
                other => Err(TypeError::new(
                    format!("array index on non-array type {}", other.display()),
                    *span,
                )),
            }
        }
        Expr::StructInit {
            name,
            fields,
            const_args,
            span,
        } => {
            for ca in const_args.iter() {
                check_const_arg(ctx, ca)?;
            }
            let declared_fields = ctx
                .structs
                .get(name)
                .cloned()
                .ok_or_else(|| TypeError::new(format!("unknown struct `{}`", name), *span))?;
            if fields.len() != declared_fields.len() {
                return Err(TypeError::new(
                    format!(
                        "struct `{}` expects {} fields, got {}",
                        name,
                        declared_fields.len(),
                        fields.len()
                    ),
                    *span,
                ));
            }
            // Instantiate generic type parameters with fresh per-call
            // variables. The fresh variables become the type arguments
            // on the resulting `Type::Struct` and the substitution is
            // applied to each declared field type before unifying with
            // the value's type. Non-generic structs go through the
            // same path with an empty substitution.
            let abstract_vars = ctx
                .struct_type_param_vars
                .get(name)
                .cloned()
                .unwrap_or_default();
            let (inst, type_args) = build_instance_subst(ctx, &abstract_vars);
            for init in fields.iter_mut() {
                let declared = declared_fields.get(&init.name).ok_or_else(|| {
                    TypeError::new(
                        format!("struct `{}` has no field `{}`", name, init.name),
                        init.span,
                    )
                })?;
                let declared_inst = declared.apply(&inst);
                let value_ty = type_of_expr(ctx, &mut init.value)?;
                if !types_compatible(ctx, &value_ty, &declared_inst) {
                    return Err(TypeError::new(
                        format!(
                            "field `{}.{}` expects {}, got {}",
                            name,
                            init.name,
                            declared_inst.display(),
                            value_ty.display()
                        ),
                        init.span,
                    ));
                }
            }
            Ok(Type::Struct(name.clone(), type_args))
        }
        Expr::EnumVariant {
            enum_name,
            variant,
            args,
            const_args,
            span,
        } => {
            for ca in const_args.iter() {
                check_const_arg(ctx, ca)?;
            }
            let payload_types = ctx
                .enums
                .get(enum_name)
                .and_then(|vs| vs.get(variant))
                .cloned();
            match payload_types {
                Some(types) => {
                    if types.len() != args.len() {
                        return Err(TypeError::new(
                            format!(
                                "enum `{}::{}` expects {} arguments, got {}",
                                enum_name,
                                variant,
                                types.len(),
                                args.len()
                            ),
                            *span,
                        ));
                    }
                    // Instantiate generic type parameters with fresh
                    // per-call variables. The fresh variables become
                    // the type arguments on the resulting
                    // `Type::Enum` and the substitution is applied to
                    // each declared payload type before unifying with
                    // the argument's type.
                    let abstract_vars = ctx
                        .enum_type_param_vars
                        .get(enum_name)
                        .cloned()
                        .unwrap_or_default();
                    let (inst, type_args) = build_instance_subst(ctx, &abstract_vars);
                    for (arg, expected) in args.iter_mut().zip(types.iter()) {
                        let expected_inst = expected.apply(&inst);
                        let arg_ty = type_of_expr(ctx, arg)?;
                        if !types_compatible(ctx, &arg_ty, &expected_inst) {
                            return Err(TypeError::new(
                                format!(
                                    "enum payload expects {}, got {}",
                                    expected_inst.display(),
                                    arg_ty.display()
                                ),
                                arg.span(),
                            ));
                        }
                    }
                    Ok(Type::Enum(enum_name.clone(), type_args))
                }
                None => {
                    if enum_name == "Option" {
                        // `Option` is built-in and is not registered in
                        // the user-declared `enums` map; the two
                        // variants are handled inline. `Some(t)` takes
                        // the payload's type as the inner; `None` takes
                        // a fresh type variable so the surrounding
                        // context (function return type, let-binding
                        // annotation, match-arm sibling, function-call
                        // argument position) can unify it. The previous
                        // implementation returned `Option<Unknown>`
                        // unconditionally, which left `Option::None`
                        // unable to unify against any concrete
                        // `Option<T>` because the unifier does not
                        // narrow `Unknown` through `Option`'s recursive
                        // arm.
                        match variant.as_str() {
                            "Some" => {
                                if args.len() != 1 {
                                    return Err(TypeError::new(
                                        format!(
                                            "`Option::Some` expects 1 argument, got {}",
                                            args.len()
                                        ),
                                        *span,
                                    ));
                                }
                                let inner = type_of_expr(ctx, &mut args[0])?;
                                return Ok(Type::Option(Box::new(inner)));
                            }
                            "None" => {
                                if !args.is_empty() {
                                    return Err(TypeError::new(
                                        format!(
                                            "`Option::None` expects 0 arguments, got {}",
                                            args.len()
                                        ),
                                        *span,
                                    ));
                                }
                                return Ok(Type::Option(Box::new(ctx.fresh())));
                            }
                            _ => {
                                return Err(TypeError::new(
                                    format!(
                                        "unknown variant `Option::{}`; expected `Some` or `None`",
                                        variant
                                    ),
                                    *span,
                                ));
                            }
                        }
                    }
                    Err(TypeError::new(
                        format!("unknown enum variant `{}::{}`", enum_name, variant),
                        *span,
                    ))
                }
            }
        }
        Expr::ArrayLiteral { elements, span } => {
            let mut elem_ty: Option<Type> = None;
            for e in elements.iter_mut() {
                let t = type_of_expr(ctx, e)?;
                match &elem_ty {
                    None => elem_ty = Some(t),
                    Some(et) => {
                        if !types_compatible(ctx, et, &t) {
                            return Err(TypeError::new(
                                format!(
                                    "array elements have differing types {} and {}",
                                    et.display(),
                                    t.display()
                                ),
                                *span,
                            ));
                        }
                    }
                }
            }
            Ok(Type::Array(
                Box::new(elem_ty.unwrap_or_else(|| ctx.fresh())),
                ConstDim::Known(elements.len() as i64),
            ))
        }
        Expr::TupleLiteral { elements, .. } => {
            let mut tys = Vec::with_capacity(elements.len());
            for e in elements {
                tys.push(type_of_expr(ctx, e)?);
            }
            Ok(Type::Tuple(tys))
        }
        Expr::Cast { expr, target, span } => {
            let from_ty_raw = type_of_expr(ctx, expr)?;
            let to_ty_raw = ctx.resolve_type(target);
            // Strip information-flow labels before the cast
            // dispatch. The cast operates on the underlying type;
            // labels propagate through the cast unchanged (the
            // labels on the source flow to the target).
            let from_labels = labels_of(&from_ty_raw);
            let from_ty = strip_labels(from_ty_raw);
            let to_ty = strip_labels(to_ty_raw);
            // Newtype <-> underlying extraction. A newtype value
            // can be cast to its underlying type (extraction); a
            // value of the underlying type can be cast to the
            // newtype (construction). Both are identity at the
            // bytecode level because newtypes are transparent.
            // The underlying type lives in `ctx.newtypes`; the
            // placeholder on `Type::Newtype` is not authoritative.
            let from_underlying = if let Type::Newtype(name) = &from_ty {
                ctx.newtypes.get(name).cloned()
            } else {
                None
            };
            let to_underlying = if let Type::Newtype(name) = &to_ty {
                ctx.newtypes.get(name).cloned()
            } else {
                None
            };
            let result_ty = match (&from_ty, &to_ty) {
                (Type::Newtype(_), other) if from_underlying.as_ref() == Some(other) => {
                    other.clone()
                }
                (other, Type::Newtype(_)) if to_underlying.as_ref() == Some(other) => to_ty.clone(),
                (Type::Newtype(_), _) if from_underlying.is_some() => {
                    return Err(TypeError::new(
                        format!(
                            "cannot cast {} to {}; newtypes only cast to their underlying type {}",
                            from_ty.display(),
                            to_ty.display(),
                            from_underlying.unwrap().display()
                        ),
                        *span,
                    ));
                }
                (Type::Word, Type::Float) | (Type::Float, Type::Word) => to_ty.clone(),
                // Byte conversions. Word→Byte truncates to the low
                // eight bits; Byte→Word zero-extends. Both are
                // explicit casts; implicit narrowing or widening is
                // not permitted because the boundary at which a
                // value is reinterpreted should be visible at the
                // call site.
                (Type::Word, Type::Byte) | (Type::Byte, Type::Word) => to_ty.clone(),
                // Fixed conversions. Word→Fixed left-shifts by the
                // target fraction-bit count; Fixed→Word arithmetic-
                // right-shifts. Both are explicit casts.
                (Type::Word, Type::Fixed(_)) | (Type::Fixed(_), Type::Word) => to_ty.clone(),
                // Enum-to-Word produces the variant's discriminant
                // value. The compiler emits a chain of IsEnum
                // tests; the runtime selects the matching
                // discriminant.
                (Type::Enum(_, _), Type::Word) => to_ty.clone(),
                // Tuple-to-Multiword construction. A tuple of exactly N
                // Word elements casts to Multiword<N>. Both share the
                // flat little-endian N-word layout, so the cast repacks
                // the tuple elements as the multiword digit array (B19).
                (Type::Tuple(elems), Type::Multiword(n, _))
                    if n.known() == Some(elems.len() as i64)
                        && elems
                            .iter()
                            .all(|e| matches!(strip_labels(e.clone()), Type::Word)) =>
                {
                    to_ty.clone()
                }
                // Type variables on either side of a cast pass
                // through (the unifier will narrow them when
                // possible). Casts between fully-resolved types
                // require structural equality unless they hit one
                // of the conversion arms above.
                (Type::Var(_), _) | (_, Type::Var(_)) => to_ty.clone(),
                (a, b) if a == b => to_ty.clone(),
                _ => {
                    return Err(TypeError::new(
                        format!("cannot cast {} to {}", from_ty.display(), to_ty.display()),
                        *span,
                    ));
                }
            };
            // Labels follow the value through the cast.
            Ok(apply_labels(result_ty, &from_labels))
        }
        Expr::Placeholder { .. } => Ok(ctx.fresh()),
        Expr::ClosureRef { span, .. } => {
            // `ClosureRef` is produced by the compiler's closure-
            // hoisting pass. V0.2.0 retires that pass alongside the
            // closure opcodes, so a `ClosureRef` reaching the type
            // checker is a compiler-internal error rather than a
            // user-facing one.
            Err(TypeError::new(
                alloc::string::String::from(
                    "internal: ClosureRef reached the type checker; \
                     V0.2.0 retired the closure-hoisting pass",
                ),
                *span,
            ))
        }
        Expr::Closure { span, .. } => {
            // Closures are rejected under the V0.2.0 conservative-
            // verification stance. The four closure opcodes
            // (`Op::PushFunc`, `Op::MakeClosure`, `Op::MakeRecursiveClosure`,
            // `Op::CallIndirect`) were removed in Phase 4 of the V0.2.0
            // ISA reset (B20); first-class functions and closures
            // cannot be lowered to bytecode. Rewrite the surface
            // expression as a direct call to a named top-level
            // function or trait method.
            Err(TypeError::new(
                alloc::string::String::from(
                    "closures are not supported; V0.2.0 admits only direct \
                     calls and trait dispatch under the conservative-\
                     verification stance. Rewrite as a top-level `fn` or \
                     trait method.",
                ),
                *span,
            ))
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
            span,
        } => {
            // Method call resolution by receiver type.
            //
            // Compute the receiver's type, take its head name, and look
            // up the mangled function `Trait::Head::method` for any
            // trait that has an impl for the receiver's type. The
            // receiver is passed as the first argument.
            //
            // For unresolved receiver types (Type::Var), the
            // resolution is deferred. The current session emits a
            // fresh return type and skips bound checking; B2.4
            // monomorphization will resolve the call by substituting
            // the concrete instantiation.
            let receiver_ty = type_of_expr(ctx, receiver)?;
            let receiver_resolved = receiver_ty.apply(&ctx.subst);
            let head = match type_head_name(&receiver_resolved) {
                Some(h) => h,
                None => return Ok(ctx.fresh()),
            };
            // Search for any trait that has an impl for the head with
            // a method matching the called name.
            let mut resolved: Option<String> = None;
            for trait_name in ctx.traits.keys() {
                let candidate = format!("{}::{}::{}", trait_name, head, method);
                if ctx.functions.contains_key(&candidate) {
                    resolved = Some(candidate);
                    break;
                }
            }
            let mangled = resolved.unwrap_or_default();
            let sig = match ctx.functions.get(&mangled).cloned() {
                Some(s) => s,
                None => {
                    return Err(TypeError::new(
                        format!(
                            "type `{}` has no method `{}` from any trait in scope",
                            head, method
                        ),
                        *span,
                    ));
                }
            };
            // The receiver is implicitly the first argument. Total
            // argument count must match params.len(). Check that
            // params.len() >= 1 to account for the receiver.
            let expected = sig.params.len();
            let actual = args.len() + 1;
            if expected != actual {
                return Err(TypeError::new(
                    format!(
                        "method `{}` expects {} arguments (including receiver), got {}",
                        method, expected, actual
                    ),
                    *span,
                ));
            }
            // Instantiate generic type parameters.
            let (inst_params, inst_return, _fresh_vars) = instantiate_sig(ctx, &sig);
            // Unify receiver with the first parameter.
            if let Some(first) = inst_params.first()
                && !types_compatible(ctx, &receiver_resolved, first)
            {
                return Err(TypeError::new(
                    format!(
                        "method `{}` receiver expects {}, got {}",
                        method,
                        first.display(),
                        receiver_resolved.display()
                    ),
                    receiver.span(),
                ));
            }
            // Unify remaining args with the rest of the parameters.
            for (arg, param_ty) in args.iter_mut().zip(inst_params.iter().skip(1)) {
                let arg_ty = type_of_expr(ctx, arg)?;
                if !types_compatible(ctx, &arg_ty, param_ty) {
                    return Err(TypeError::new(
                        format!(
                            "method `{}` argument expects {}, got {}",
                            method,
                            param_ty.display(),
                            arg_ty.display()
                        ),
                        arg.span(),
                    ));
                }
            }
            Ok(inst_return)
        }
        Expr::Checked {
            op_expr,
            arms,
            span,
        } => {
            // The indexing construct (B35 P4) is a sibling of the
            // arithmetic construct sharing the same node: when the
            // guarded operation is an array index, the admissible arms
            // are `ok` (binding the element) and `invalid_index`
            // (binding the offending index `Word`).
            if matches!(op_expr.as_ref(), Expr::ArrayIndex { .. }) {
                return check_checked_index(ctx, op_expr, arms, span);
            }
            // The newtype-construction construct (B35 P5) is another
            // sibling: when the guarded operation constructs a
            // newtype, the admissible arms are `ok` (binding the
            // newtype) and `invalid_newtype` (binding the underlying
            // value the refinement rejected).
            if let Expr::Call { name, .. } = op_expr.as_ref()
                && matches!(ctx.types.get(name), Some(TypeKind::Newtype))
            {
                let name = name.clone();
                return check_checked_newtype(ctx, op_expr, &name, arms, span);
            }
            // The discriminant-to-enum construct (B35 P6): when the
            // guarded operation is a `Word as Enum` cast, the
            // admissible arms are `ok` (override a unit variant),
            // `payload_discriminant` (supply a payload variant's
            // payload), and `invalid_discriminant` (catch an unmapped
            // discriminant).
            if let Expr::Cast { target, .. } = op_expr.as_ref()
                && let Type::Enum(enum_name, _) = strip_labels(ctx.resolve_type(target))
            {
                let enum_name = enum_name.clone();
                return check_checked_discriminant(ctx, op_expr, &enum_name, arms, span);
            }
            // The native-error construct (B35 P7): when the guarded
            // operation is a native call, the admissible arms are
            // `ok` (the success value) and `error` (the Word error
            // code a fallible native reported).
            if let Expr::Call { name, .. } = op_expr.as_ref()
                && (ctx.natives.contains(name) || name.contains("::"))
            {
                let name = name.clone();
                return check_checked_native(ctx, op_expr, &name, arms, span);
            }
            // The guarded operation must be a single arithmetic
            // operation on Word operands. V0.2 supports the four
            // standard binary ops plus unary negation; other
            // operand types are reserved for later iterations.
            let supported = matches!(
                op_expr.as_ref(),
                Expr::BinOp {
                    op: BinOp::Add
                        | BinOp::Sub
                        | BinOp::Mul
                        | BinOp::Div
                        | BinOp::Mod
                        | BinOp::AShl,
                    ..
                } | Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    ..
                }
            );
            if !supported {
                return Err(TypeError::new(
                    alloc::string::String::from(
                        "checked-overflow construct currently guards only `+`, `-`, `*`, `/`, `%`, and unary `-` on Word operands",
                    ),
                    *span,
                ));
            }
            let inner_ty = type_of_expr(ctx, op_expr)?;
            let is_word = types_compatible(ctx, &inner_ty, &Type::Word);
            let is_byte = !is_word && types_compatible(ctx, &inner_ty, &Type::Byte);
            let is_float = !is_word && !is_byte && types_compatible(ctx, &inner_ty, &Type::Float);
            // `Fixed` carries a fraction-bit count, so it cannot be
            // matched against a fixed `Type` constant; resolve the
            // operand type through the substitution and match it
            // structurally (B35 P3d-iii). `Fixed` is signed, so its
            // outcome admissibility mirrors `Word`; the distinction
            // is that its arms bind a single result like `Byte` and
            // `Float`.
            let fixed_frac_bits: Option<u8> = if !is_word && !is_byte && !is_float {
                match strip_labels(inner_ty.apply(&ctx.subst)) {
                    Type::Fixed(n) => Some(n),
                    _ => None,
                }
            } else {
                None
            };
            let is_fixed = fixed_frac_bits.is_some();
            if !is_word && !is_byte && !is_float && !is_fixed {
                return Err(TypeError::new(
                    alloc::format!(
                        "checked-overflow construct expects a Word, Byte, Float, or Fixed arithmetic operation, got {}",
                        inner_ty.display()
                    ),
                    *span,
                ));
            }
            let operand_ty = if is_byte {
                Type::Byte
            } else if is_float {
                Type::Float
            } else if let Some(n) = fixed_frac_bits {
                Type::Fixed(n)
            } else {
                Type::Word
            };
            // Validate arm structure. The `ok` class must have an
            // unguarded catch-all arm; `overflow` and `underflow` are
            // optional and default to wrapping (B35 P3a); a
            // `zero_divisor` arm handles a zero divisor on `/` and `%`
            // (B35 P3b). The last covering arm per class must be an
            // unguarded catch-all. Patterns are restricted to
            // wildcard, variable, and integer literal; type
            // unification on the arm scope catches mismatches in the
            // literal case.
            use crate::ast::CheckedArmKind;

            // The guarded operator, used for admissibility and arity.
            #[derive(Clone, Copy, PartialEq)]
            enum CheckedOp {
                Add,
                Sub,
                Mul,
                Div,
                Mod,
                AShl,
                Neg,
            }
            let (cop, op_desc) = match op_expr.as_ref() {
                Expr::BinOp { op: BinOp::Add, .. } => (CheckedOp::Add, "`+`"),
                Expr::BinOp { op: BinOp::Sub, .. } => (CheckedOp::Sub, "`-`"),
                Expr::BinOp { op: BinOp::Mul, .. } => (CheckedOp::Mul, "`*`"),
                Expr::BinOp { op: BinOp::Div, .. } => (CheckedOp::Div, "`/`"),
                Expr::BinOp { op: BinOp::Mod, .. } => (CheckedOp::Mod, "`%`"),
                Expr::BinOp {
                    op: BinOp::AShl, ..
                } => (CheckedOp::AShl, "arithmetic left shift `<<<`"),
                // The `supported` check above guarantees unary `-`.
                _ => (CheckedOp::Neg, "unary `-`"),
            };
            // The arithmetic left shift is signed-only and Word-only, the
            // same as multiply-by-a-power-of-two, which is how it lowers.
            if cop == CheckedOp::AShl && !is_word {
                return Err(TypeError::new(
                    alloc::format!(
                        "the arithmetic left shift `<<<` in a checked construct requires Word operands, got {}",
                        inner_ty.display()
                    ),
                    *span,
                ));
            }

            // Unary negation is signed-only; reject it on Byte.
            if is_byte && cop == CheckedOp::Neg {
                return Err(TypeError::new(
                    alloc::string::String::from(
                        "unary `-` is not supported on Byte operands in a checked construct",
                    ),
                    *span,
                ));
            }
            // Float has no modulo and no checked negation.
            if is_float && matches!(cop, CheckedOp::Mod | CheckedOp::Neg) {
                return Err(TypeError::new(
                    alloc::string::String::from(
                        "modulo and unary `-` are not supported on Float operands in a checked construct",
                    ),
                    *span,
                ));
            }

            // Per-operand-type admissibility (B35 P3c, P3d). For the
            // signed `Word` type: `+`, `-`, `*` admit `overflow` and
            // `underflow`; unary `-` admits `overflow`; `/` admits
            // `overflow` and `zero_divisor`; `%` admits `zero_divisor`.
            // For the unsigned `Byte` type: `+` and `*` admit
            // `overflow`; `-` admits `underflow`; `/` and `%` admit
            // `zero_divisor`. The signed `Fixed` type mirrors `Word`
            // (it is signed and `Q`-format arithmetic can overflow or
            // underflow in either direction), differing only in that
            // its arms bind a single result; it therefore reuses the
            // `Word` admissibility branch below. `ok` is admissible for
            // every operator. An arm whose outcome cannot arise is a
            // compile error.
            let type_name = if is_byte {
                "Byte"
            } else if is_float {
                "Float"
            } else if is_fixed {
                "Fixed"
            } else {
                "Word"
            };
            for arm in arms.iter() {
                let (admissible, arm_name) = match &arm.kind {
                    CheckedArmKind::Ok(_) => (true, "ok"),
                    CheckedArmKind::Overflow(_, _) => {
                        let ok = if is_byte {
                            matches!(cop, CheckedOp::Add | CheckedOp::Mul)
                        } else if is_float {
                            // Float `+`, `-`, `*`, `/` can yield +inf.
                            matches!(
                                cop,
                                CheckedOp::Add | CheckedOp::Sub | CheckedOp::Mul | CheckedOp::Div
                            )
                        } else {
                            matches!(
                                cop,
                                CheckedOp::Add
                                    | CheckedOp::Sub
                                    | CheckedOp::Mul
                                    | CheckedOp::AShl
                                    | CheckedOp::Neg
                                    | CheckedOp::Div
                            )
                        };
                        (ok, "overflow")
                    }
                    CheckedArmKind::Underflow(_, _) => {
                        let ok = if is_byte {
                            cop == CheckedOp::Sub
                        } else if is_float {
                            // Float `+`, `-`, `*`, `/` can yield -inf.
                            matches!(
                                cop,
                                CheckedOp::Add | CheckedOp::Sub | CheckedOp::Mul | CheckedOp::Div
                            )
                        } else {
                            matches!(
                                cop,
                                CheckedOp::Add | CheckedOp::Sub | CheckedOp::Mul | CheckedOp::AShl
                            )
                        };
                        (ok, "underflow")
                    }
                    CheckedArmKind::ZeroDivisor(_) => (
                        // Integer-only: a float division by zero yields
                        // an infinity or NaN, not a trap.
                        !is_float && matches!(cop, CheckedOp::Div | CheckedOp::Mod),
                        "zero_divisor",
                    ),
                    // `nan` arises only on Float, where every supported
                    // operator can produce a NaN (e.g. inf - inf, 0/0).
                    CheckedArmKind::Nan(_) => (is_float, "nan"),
                    // `invalid_index` and `invalid_newtype` are the
                    // indexing and newtype constructs' arms, never
                    // admissible on an arithmetic operation. Those
                    // constructs route away before reaching this path.
                    CheckedArmKind::InvalidIndex(_) => (false, "invalid_index"),
                    CheckedArmKind::InvalidNewtype(_) => (false, "invalid_newtype"),
                    CheckedArmKind::PayloadDiscriminant(_) => (false, "payload_discriminant"),
                    CheckedArmKind::InvalidDiscriminant(_) => (false, "invalid_discriminant"),
                    CheckedArmKind::Error(_) => (false, "error"),
                };
                if !admissible {
                    return Err(TypeError::new(
                        alloc::format!(
                            "the `{}` arm is not admissible for the {} operation on {}: that outcome cannot arise",
                            arm_name,
                            op_desc,
                            type_name
                        ),
                        arm.span,
                    ));
                }
                // Arity: a `Word` `overflow`/`underflow` arm binds the
                // high and low halves `(h, l)`; a `Byte` or `Float` arm
                // binds a single result.
                if let CheckedArmKind::Overflow(_, l) | CheckedArmKind::Underflow(_, l) = &arm.kind
                {
                    // `Byte`, `Float`, and `Fixed` arms bind a single
                    // result; only the signed `Word` arm binds the
                    // high and low halves `(h, l)`.
                    let wants_single = is_byte || is_float || is_fixed;
                    if wants_single && l.is_some() {
                        return Err(TypeError::new(
                            alloc::format!(
                                "a {} `{}` arm binds a single result; write `{}(v)`",
                                type_name,
                                arm_name,
                                arm_name
                            ),
                            arm.span,
                        ));
                    }
                    if !wants_single && l.is_none() {
                        return Err(TypeError::new(
                            alloc::format!(
                                "a Word `{}` arm binds the high and low halves; write `{}(h, l)`",
                                arm_name,
                                arm_name
                            ),
                            arm.span,
                        ));
                    }
                }
            }
            let mut ok_catchall_seen = false;
            let mut overflow_catchall_seen = false;
            let mut underflow_catchall_seen = false;
            let mut zero_divisor_catchall_seen = false;
            let mut nan_catchall_seen = false;
            for arm in arms.iter() {
                // Outcomes whose catchall has already been seen
                // cannot have further arms; subsequent arms in the
                // same outcome are dead code.
                let class_catchall_seen = match &arm.kind {
                    CheckedArmKind::Ok(_) => ok_catchall_seen,
                    CheckedArmKind::Overflow(_, _) => overflow_catchall_seen,
                    CheckedArmKind::Underflow(_, _) => underflow_catchall_seen,
                    CheckedArmKind::ZeroDivisor(_) => zero_divisor_catchall_seen,
                    CheckedArmKind::Nan(_) => nan_catchall_seen,
                    // Unreachable on arithmetic; indexing, newtype
                    // construction, discriminant conversion, and native
                    // calls route earlier.
                    CheckedArmKind::InvalidIndex(_)
                    | CheckedArmKind::InvalidNewtype(_)
                    | CheckedArmKind::PayloadDiscriminant(_)
                    | CheckedArmKind::InvalidDiscriminant(_)
                    | CheckedArmKind::Error(_) => false,
                };
                if class_catchall_seen {
                    return Err(TypeError::new(
                        alloc::string::String::from(
                            "checked-overflow arm is unreachable: a prior catch-all arm in the same outcome class already covers it",
                        ),
                        arm.span,
                    ));
                }
                let is_catchall = arm.guard.is_none() && checked_arm_is_catchall(&arm.kind);
                if is_catchall {
                    match &arm.kind {
                        CheckedArmKind::Ok(_) => ok_catchall_seen = true,
                        CheckedArmKind::Overflow(_, _) => overflow_catchall_seen = true,
                        CheckedArmKind::Underflow(_, _) => underflow_catchall_seen = true,
                        CheckedArmKind::ZeroDivisor(_) => zero_divisor_catchall_seen = true,
                        CheckedArmKind::Nan(_) => nan_catchall_seen = true,
                        CheckedArmKind::InvalidIndex(_)
                        | CheckedArmKind::InvalidNewtype(_)
                        | CheckedArmKind::PayloadDiscriminant(_)
                        | CheckedArmKind::InvalidDiscriminant(_)
                        | CheckedArmKind::Error(_) => {}
                    }
                }
            }
            if !ok_catchall_seen {
                return Err(TypeError::new(
                    alloc::string::String::from(
                        "checked-overflow construct is non-exhaustive on `ok`: the last `ok` arm must be an unguarded catch-all (bare variable or wildcard)",
                    ),
                    *span,
                ));
            }
            // The `overflow` and `underflow` classes are optional
            // (B35 P3). An omitted or non-exhaustive class defaults to
            // two's-complement wrapping, which the compiler supplies.
            // The catch-all-seen flags above remain in use only for
            // the unreachable-arm check; they no longer gate
            // exhaustiveness.
            let _ = (
                overflow_catchall_seen,
                underflow_catchall_seen,
                zero_divisor_catchall_seen,
                nan_catchall_seen,
            );
            // Type-check arm bodies. Each arm scope binds the
            // pattern variables to `Word`. The guard expression (if
            // present) is checked in the same scope and must be
            // Bool. All arm bodies unify against the construct's
            // result type.
            let result_ty = ctx.fresh();
            for arm in arms.iter_mut() {
                ctx.push_scope();
                match &arm.kind {
                    CheckedArmKind::Ok(p)
                    | CheckedArmKind::ZeroDivisor(p)
                    | CheckedArmKind::Nan(p)
                    | CheckedArmKind::InvalidIndex(p)
                    | CheckedArmKind::InvalidNewtype(p)
                    | CheckedArmKind::PayloadDiscriminant(p)
                    | CheckedArmKind::InvalidDiscriminant(p)
                    | CheckedArmKind::Error(p) => {
                        bind_checked_pattern(ctx, p, operand_ty.clone());
                    }
                    CheckedArmKind::Overflow(h, l) | CheckedArmKind::Underflow(h, l) => {
                        bind_checked_pattern(ctx, h, operand_ty.clone());
                        if let Some(l) = l {
                            bind_checked_pattern(ctx, l, operand_ty.clone());
                        }
                    }
                }
                if let Some(guard) = arm.guard.as_mut() {
                    let guard_ty = type_of_expr(ctx, guard)?;
                    if !types_compatible(ctx, &strip_labels(guard_ty.clone()), &Type::Bool) {
                        ctx.pop_scope();
                        return Err(TypeError::new(
                            alloc::format!(
                                "checked-overflow arm guard must be Bool, got {}",
                                guard_ty.display()
                            ),
                            arm.span,
                        ));
                    }
                }
                let body_ty = type_of_expr(ctx, &mut arm.body)?;
                ctx.pop_scope();
                if !types_compatible(ctx, &body_ty, &result_ty) {
                    return Err(TypeError::new(
                        alloc::format!(
                            "checked-overflow arm produces {} which does not unify with the construct's result type {}",
                            body_ty.display(),
                            result_ty.apply(&ctx.subst).display()
                        ),
                        arm.span,
                    ));
                }
            }
            Ok(result_ty.apply(&ctx.subst))
        }
        Expr::SaturateMax { span } | Expr::SaturateMin { span } => {
            // `saturate_max` / `saturate_min` have a context-determined
            // type, tied to the surrounding construct's expected type.
            // The node is rewritten in place to a concrete typed
            // literal so the compiler emits the right bound; the
            // construct is otherwise transparent at the bytecode layer.
            // `Word` keeps the bare keyword, which the compiler lowers
            // to the runtime `Word` bound. `Byte`, `Float`, and
            // `Fixed<N>` rewrite to that type's saturating bound (B35
            // P3d extended checked arithmetic to those operand types).
            // A refined newtype that declared a `with saturate_max` /
            // `saturate_min` value rewrites to a constructor call on
            // that literal, predicate-checked at runtime.
            let span_copy = *span;
            let is_max = matches!(expr, Expr::SaturateMax { .. });
            if let Some(exp_ty) = ctx.expected_type() {
                match strip_labels(exp_ty) {
                    Type::Newtype(name) => {
                        let resolved = if is_max {
                            ctx.newtype_saturate_max.get(&name).copied()
                        } else {
                            ctx.newtype_saturate_min.get(&name).copied()
                        };
                        if let Some(value) = resolved {
                            *expr = Expr::Call {
                                name: name.clone(),
                                args: alloc::vec![Expr::Literal {
                                    value: Literal::Int(value),
                                    span: span_copy,
                                }],
                                const_args: Vec::new(),
                                span: span_copy,
                            };
                            return Ok(Type::Newtype(name));
                        }
                    }
                    Type::Byte => {
                        // Unsigned Byte bounds: 255 and 0.
                        let value = if is_max { 255 } else { 0 };
                        *expr = Expr::Literal {
                            value: Literal::Byte(value),
                            span: span_copy,
                        };
                        return Ok(Type::Byte);
                    }
                    Type::Float => {
                        // The largest and most-negative finite Float.
                        let value = if is_max { f64::MAX } else { f64::MIN };
                        *expr = Expr::Literal {
                            value: Literal::Float(value),
                            span: span_copy,
                        };
                        return Ok(Type::Float);
                    }
                    Type::Fixed(n) => {
                        // The extremal Q-format raw bit patterns.
                        let raw = if is_max { i64::MAX } else { i64::MIN };
                        *expr = Expr::Literal {
                            value: Literal::Fixed { raw, frac_bits: n },
                            span: span_copy,
                        };
                        return Ok(Type::Fixed(n));
                    }
                    _ => {}
                }
            }
            Ok(Type::Word)
        }
        Expr::Classify { value, labels, .. } => {
            // Classify adds labels to the value's label set.
            // The underlying type is unchanged. Always admitted;
            // adding labels only tightens flow restrictions.
            let value_ty = type_of_expr(ctx, value)?;
            let (underlying, mut current_labels) = match value_ty {
                Type::Labelled(inner, ls) => (*inner, ls),
                other => (other, BTreeSet::new()),
            };
            for l in labels {
                current_labels.insert(l.clone());
            }
            if current_labels.is_empty() {
                Ok(underlying)
            } else {
                Ok(Type::Labelled(Box::new(underlying), current_labels))
            }
        }
        Expr::Declassify { value, labels, .. } => {
            // Declassify removes labels from the value's label
            // set. The underlying type is unchanged. The
            // operation is always admitted but constitutes an
            // explicit information disclosure audit point that
            // a future iteration may record for review.
            let value_ty = type_of_expr(ctx, value)?;
            let (underlying, mut current_labels) = match value_ty {
                Type::Labelled(inner, ls) => (*inner, ls),
                other => (other, BTreeSet::new()),
            };
            for l in labels {
                current_labels.remove(l);
            }
            if current_labels.is_empty() {
                Ok(underlying)
            } else {
                Ok(Type::Labelled(Box::new(underlying), current_labels))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check_src(src: &str) -> Result<(), TypeError> {
        let tokens = tokenize(src).expect("lex");
        let mut program = parse(&tokens).expect("parse");
        check(&mut program)
    }

    #[test]
    fn native_signature_accepts_well_typed_call() {
        // The `use host::log_event(Word, Word) -> ()` declaration
        // pins the parameter and return types at the type-checker
        // level. A call with two `Word` arguments and a discarded
        // unit result type-checks without error.
        check_src(
            "use host::log_event(Word, Word) -> ()\n\
             fn main() -> Word { host::log_event(1, 2); 0 }",
        )
        .expect("well-typed native call should pass");
    }

    #[test]
    fn native_signature_rejects_wrong_argument_count() {
        // Declared as 2-argument; call site supplies 1.
        let err = check_src(
            "use host::log_event(Word, Word) -> ()\n\
             fn main() -> Word { host::log_event(1); 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("expects 2 argument(s), got 1"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn native_signature_rejects_wrong_argument_type() {
        // Declared as `(Word) -> Word`; call passes `Bool`.
        let err = check_src(
            "use host::increment(Word) -> Word\n\
             fn main() -> Word { host::increment(true) }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("argument 0 expects Word, got bool"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn native_without_signature_remains_permissive() {
        // No signature on the `use` declaration; the call site
        // accepts any argument types (legacy behaviour).
        check_src(
            "use host::log_event\n\
             fn main() -> Word { host::log_event(1, true); 0 }",
        )
        .expect("permissive path should admit native without signature");
    }

    #[test]
    fn native_signature_assigns_declared_return_type() {
        // The declared return type drives the surrounding binding's
        // inferred type. With `-> Word`, the let binding is Word.
        check_src(
            "use host::clock_now() -> Word\n\
             fn main() -> Word { let t: Word = host::clock_now(); t }",
        )
        .expect("declared return type should unify with annotated binding");
        // A mismatch on the same call site is rejected.
        let err = check_src(
            "use host::clock_now() -> Word\n\
             fn main() -> Bool { host::clock_now() }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("returns Bool but body produces Word"),
            "unexpected error: {}",
            err.message
        );
    }

    /// End-to-end compile helper. Some tests need to validate that
    /// monomorphization specializes a generic call site, which only
    /// surfaces during the full compile pipeline rather than the
    /// type-check pass alone. Returns the message of any compile
    /// error so the test can assert against the failure mode.
    fn compile_src(src: &str) -> Result<(), alloc::string::String> {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        crate::compiler::compile(&program)
            .map(|_| ())
            .map_err(|e| e.message)
    }

    #[test]
    fn newtype_cycle_rejected() {
        // `newtype A = B; newtype B = A;` is a definition cycle
        // that the pass 1a'' check rejects.
        let err = check_src(
            "newtype A = B;\n\
             newtype B = A;\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("definition cycle"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn newtype_extraction_via_as_cast() {
        // The `as` cast from a newtype to its underlying type
        // is the surface form for extracting the wrapped value.
        // The cast is identity at the bytecode level because the
        // newtype is transparent.
        let result = compile_src(
            "newtype LocalMs = Word;\n\
             fn main() -> Word {\n\
                 let t: LocalMs = LocalMs(42);\n\
                 t as Word\n\
             }",
        );
        // The extraction may not yet be supported; record the
        // observed behaviour so the gap is visible.
        match result {
            Ok(()) => {}
            Err(msg) => panic!("newtype extraction via `as` should compile: {}", msg),
        }
    }

    #[test]
    fn newtype_with_refinement_composition() {
        // The `newtype Name = T where predicate;` form composes
        // nominal-type distinctness with runtime range checking.
        // The composition is exercised end-to-end through the
        // type checker.
        check_src(
            "fn in_range(x: Word) -> bool { x >= 0 and x <= 100 }\n\
             newtype Percent = Word where in_range;\n\
             fn main() -> Percent { Percent(50) }",
        )
        .expect("newtype with refinement composes");
    }

    #[test]
    fn newtype_chain_admitted() {
        // `newtype A = Word; newtype B = A; newtype C = B;` is a
        // legitimate chain with no cycle. The cycle check admits
        // it.
        check_src(
            "newtype A = Word;\n\
             newtype B = A;\n\
             newtype C = B;\n\
             fn main() -> C { C(B(A(42))) }",
        )
        .expect("non-cyclic newtype chain admitted");
    }

    #[test]
    fn newtype_construction_accepts_underlying() {
        // `newtype LocalMs = Word;` introduces a distinct nominal
        // type whose underlying is Word. Construction with a Word
        // argument type-checks.
        check_src(
            "newtype LocalMs = Word;\n\
             fn main() -> LocalMs { LocalMs(42) }",
        )
        .expect("newtype construction with matching underlying should type-check");
    }

    #[test]
    fn newtype_construction_rejects_wrong_underlying() {
        // The newtype's underlying is Word; passing a Bool should
        // be rejected.
        let err = check_src(
            "newtype LocalMs = Word;\n\
             fn main() -> LocalMs { LocalMs(true) }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("expects Word"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn newtype_distinct_from_underlying() {
        // A newtype is not assignable to its underlying type
        // without explicit construction or extraction. A function
        // declared to return Word cannot return a LocalMs value.
        let err = check_src(
            "newtype LocalMs = Word;\n\
             fn main() -> Word { LocalMs(42) }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("returns Word") && err.message.contains("LocalMs"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn newtype_distinct_from_other_newtype() {
        // Two newtypes over the same underlying are not
        // interchangeable. A function declared to return
        // OriginFrameMs cannot return a LocalMs.
        let err = check_src(
            "newtype LocalMs = Word;\n\
             newtype OriginFrameMs = Word;\n\
             fn main() -> OriginFrameMs { LocalMs(42) }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("LocalMs") && err.message.contains("OriginFrameMs"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn newtype_compiles_to_underlying() {
        // The newtype is transparent at the bytecode level. The
        // compiled program runs and returns the underlying value;
        // accessing the wrapped Word through the newtype layer is
        // a no-op at runtime.
        compile_src(
            "newtype LocalMs = Word;\n\
             fn main() -> LocalMs { LocalMs(7 + 35) }",
        )
        .expect("newtype-wrapping program should compile");
    }

    #[test]
    fn qif_open_flows_into_labeled() {
        // A value with no labels flows into a slot expecting
        // labels because the empty label set is a subset of any
        // label set (classify is implicit at the target).
        check_src("fn main() -> Word@Secret { 42 }").expect("Word flows into Word@Secret silently");
    }

    #[test]
    fn qif_labeled_to_open_requires_declassify() {
        // A labeled value cannot flow into an unlabeled slot.
        let err = check_src(
            "fn produce() -> Word@Secret { 42 }\n\
             fn main() -> Word { produce() }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("Word@Secret") && err.message.contains("Word"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn qif_classify_adds_labels() {
        // The result type is the value's underlying with the
        // named labels added.
        check_src(
            "fn produce() -> Word@Secret { 42 }\n\
             fn main() -> Word@{Mission, Secret} { classify produce()@Mission }",
        )
        .expect("classify adds Mission to existing Secret");
    }

    #[test]
    fn qif_declassify_removes_labels() {
        // declassify is the explicit way to lower restrictions.
        // The result type is the value's underlying with the
        // named labels removed.
        check_src(
            "fn produce() -> Word@Secret { 42 }\n\
             fn main() -> Word { declassify produce()@Secret }",
        )
        .expect("declassify removes Secret, value flows to Word");
    }

    #[test]
    fn qif_multi_label_subset_rule() {
        // {Mission} ⊆ {Mission, Secret}: source flows into a
        // target that accepts more labels.
        check_src(
            "fn produce() -> Word@Mission { 42 }\n\
             fn main() -> Word@{Mission, Secret} { produce() }",
        )
        .expect("source with fewer labels flows into target with more");
    }

    #[test]
    fn qif_tuple_preserves_per_element_labels() {
        // A tuple expression preserves labels on each element
        // because `Type::Tuple` holds a vector of types and each
        // element's type can independently be `Labelled`.
        let tokens = tokenize(
            "fn secret() -> Word@Secret { 1 }\n\
             fn main() -> (Word@Secret, Word) {\n\
                 (secret(), 2)\n\
             }",
        )
        .expect("lex");
        let mut program = parse(&tokens).expect("parse");
        let result = check(&mut program);
        if let Err(e) = &result {
            panic!(
                "tuple preserves per-element labels: {} at span {:?}",
                e.message, e.span
            );
        }
    }

    #[test]
    fn qif_tuple_destructure_preserves_labels() {
        // Destructuring the tuple binds each component with its
        // own label.
        check_src(
            "fn secret() -> Word@Secret { 1 }\n\
             fn main() -> Word@Secret {\n\
                 let t = (secret(), 2);\n\
                 t.0\n\
             }",
        )
        .expect("tuple index preserves label");
    }

    #[test]
    fn qif_tuple_blocks_leak_through_unlabeled_field() {
        // Reading a labeled tuple field into an unlabeled slot
        // is rejected.
        let err = check_src(
            "fn secret() -> Word@Secret { 1 }\n\
             fn main() -> Word {\n\
                 let t = (secret(), 2);\n\
                 t.0\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("Word@Secret") && err.message.contains("Word"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn qif_arithmetic_taints_result_label() {
        // `secret + 0` should produce a Word@Secret. Previously
        // the BinOp arm stripped labels and returned a pure
        // Word, which was the soundness gap. The result now
        // inherits the union of operand labels.
        check_src(
            "fn produce() -> Word@Secret { 42 }\n\
             fn main() -> Word@Secret { produce() + 1 }",
        )
        .expect("arithmetic taints result with operand label");
    }

    #[test]
    fn qif_arithmetic_taint_blocks_leak() {
        // The tainted result cannot flow to an unlabeled slot
        // without declassify; this confirms the propagation is
        // actually checked at the function-return boundary.
        let err = check_src(
            "fn produce() -> Word@Secret { 42 }\n\
             fn main() -> Word { produce() + 1 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("Word@Secret") && err.message.contains("Word"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn qif_arithmetic_unions_operand_labels() {
        // `a@Mission + b@Sensor` produces `Word@{Mission,Sensor}`.
        check_src(
            "fn a() -> Word@Mission { 1 }\n\
             fn b() -> Word@Sensor { 2 }\n\
             fn main() -> Word@{Mission, Sensor} { a() + b() }",
        )
        .expect("label union on arithmetic");
    }

    #[test]
    fn qif_if_taints_with_condition_labels() {
        // The condition's labels propagate to the result; the
        // observer of the result can infer information about
        // the condition.
        check_src(
            "fn cond() -> bool@Secret { true }\n\
             fn main() -> Word@Secret { if cond() { 1 } else { 2 } }",
        )
        .expect("if condition labels taint result");
    }

    #[test]
    fn qif_if_branch_labels_join_with_condition() {
        // Branch arms may carry their own labels; the result
        // is the join of the condition's labels and the arms'
        // labels.
        check_src(
            "fn cond() -> bool@Secret { true }\n\
             fn make() -> Word@Mission { 1 }\n\
             fn main() -> Word@{Secret, Mission} { if cond() { make() } else { 0 } }",
        )
        .expect("if combines condition and branch labels");
    }

    #[test]
    fn qif_native_signature_with_labels() {
        // Native function signatures admit labels. A native
        // declared as `host::transmit(p: Word@Open) -> bool`
        // rejects calls that pass a labeled value without
        // declassify.
        let err = check_src(
            "use host::transmit(Word@Open) -> bool\n\
             fn produce() -> Word@Secret { 42 }\n\
             fn main() -> bool { host::transmit(produce()) }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("argument 0 expects"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn qif_native_signature_admits_declassified() {
        // The same call site with explicit declassify is
        // admitted.
        check_src(
            "use host::transmit(Word@Open) -> bool\n\
             fn produce() -> Word@Secret { 42 }\n\
             fn main() -> bool { host::transmit(declassify produce()@Secret) }",
        )
        .expect("declassified value flows into Open native parameter");
    }

    #[test]
    fn qif_classify_is_not_a_keyword() {
        // `classify` must remain usable as a function name so
        // existing scripts that defined a `classify` helper are
        // not broken by the QIF extension.
        check_src(
            "fn classify(x: Word) -> Word { x }\n\
             fn main() -> Word { classify(42) }",
        )
        .expect("classify usable as a function name");
    }

    #[test]
    fn checked_overflow_requires_ok_arm() {
        let err = check_src(
            "fn main() -> Word {\n\
                let y = 1 + 2 {\n\
                    overflow(_, _) => 0,\n\
                    underflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("non-exhaustive on `ok`"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn checked_overflow_arm_is_optional() {
        // B35 P3: the `overflow` class is optional. Omitting it
        // typechecks; the missing class defaults to wrapping.
        check_src(
            "fn main() -> Word {\n\
                let y = 1 + 2 {\n\
                    underflow(_, _) => 0,\n\
                    ok(v) => v,\n\
                };\n\
                y\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn checked_underflow_arm_is_optional() {
        // B35 P3: the `underflow` class is optional, likewise.
        check_src(
            "fn main() -> Word {\n\
                let y = 1 + 2 {\n\
                    overflow(_, _) => 0,\n\
                    ok(v) => v,\n\
                };\n\
                y\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn checked_overflow_and_underflow_both_optional() {
        // Only the `ok` class is required; both exceptional classes
        // may be omitted and default to wrapping.
        check_src(
            "fn main() -> Word {\n\
                let y = 1 + 2 {\n\
                    ok(v) => v,\n\
                };\n\
                y\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn checked_overflow_admits_multiplication() {
        // Multiplication is supported by the construct.
        check_src(
            "fn main() -> Word {\n\
                let y = 7 * 6 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => 0,\n\
                    underflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
        )
        .expect("checked construct admits `*`");
    }

    #[test]
    fn checked_overflow_admits_unary_neg() {
        // Unary negation is supported by the construct.
        check_src(
            "fn main() -> Word {\n\
                let y = -1 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
        )
        .expect("checked construct admits unary `-`");
    }

    #[test]
    fn checked_underflow_arm_rejected_on_division() {
        // B35 P3c: division cannot underflow, so an `underflow` arm
        // on `/` is a compile error.
        let err = check_src(
            "fn main() -> Word { let y = 10 / 2 { ok(v) => v, underflow(_, _) => 0 }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("underflow"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_overflow_arm_rejected_on_modulo() {
        // Modulo never overflows, so an `overflow` arm on `%` is a
        // compile error.
        let err = check_src(
            "fn main() -> Word { let y = 10 % 2 { ok(v) => v, overflow(_, _) => 0 }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("overflow"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_zero_divisor_arm_rejected_on_addition() {
        // Addition has no zero divisor, so a `zero_divisor` arm on
        // `+` is a compile error.
        let err = check_src(
            "fn main() -> Word { let y = 1 + 2 { ok(v) => v, zero_divisor(_) => 0 }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("zero_divisor"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_byte_overflow_single_pattern_typechecks() {
        // B35 P3d-i: a Byte overflow arm binds a single wrapped value.
        check_src(
            "fn main() -> Byte { let y = 200Byte + 100Byte { ok(v) => v, overflow(w) => w }; y }",
        )
        .expect("Byte checked overflow with single pattern should typecheck");
    }

    #[test]
    fn checked_byte_overflow_two_patterns_rejected() {
        // The two-pattern (h, l) form is the Word shape; Byte rejects it.
        let err = check_src(
            "fn main() -> Byte { let y = 200Byte + 100Byte { ok(v) => v, overflow(h, l) => h }; y }",
        )
        .unwrap_err();
        assert!(err.message.contains("single result"), "{}", err.message);
    }

    #[test]
    fn checked_byte_underflow_rejected_on_addition() {
        // Byte addition cannot underflow.
        let err = check_src(
            "fn main() -> Byte { let y = 1Byte + 2Byte { ok(v) => v, underflow(w) => w }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("underflow"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_word_overflow_single_pattern_rejected() {
        // The single-pattern form is the Byte shape; Word requires (h, l).
        let err =
            check_src("fn main() -> Word { let y = 1 + 2 { ok(v) => v, overflow(w) => w }; y }")
                .unwrap_err();
        assert!(
            err.message.contains("high and low halves"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_unary_neg_rejected_on_byte() {
        // Unary negation is signed-only.
        let err =
            check_src("fn main() -> Byte { let y = -(5Byte) { ok(v) => v }; y }").unwrap_err();
        assert!(
            err.message.contains("unary `-`") && err.message.contains("Byte"),
            "{}",
            err.message
        );
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_float_all_outcomes_typecheck() {
        // B35 P3d-ii: a Float construct admits ok, overflow, underflow,
        // and nan, each binding a single result.
        check_src(
            "fn main() -> Float { let y = 1.0Float / 2.0Float { ok(v) => v, overflow(i) => i, underflow(i) => i, nan(n) => n }; y }",
        )
        .expect("Float checked construct should typecheck");
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_nan_arm_rejected_on_word() {
        // Integer arithmetic never produces NaN.
        let err = check_src("fn main() -> Word { let y = 1 + 2 { ok(v) => v, nan(_) => 0 }; y }")
            .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("nan"),
            "{}",
            err.message
        );
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_zero_divisor_arm_rejected_on_float() {
        // A float division by zero is an infinity or NaN, not a trap.
        let err = check_src(
            "fn main() -> Float { let y = 1.0Float / 2.0Float { ok(v) => v, zero_divisor(_) => 0.0Float }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("zero_divisor"),
            "{}",
            err.message
        );
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_modulo_rejected_on_float() {
        let err = check_src("fn main() -> Float { let y = 5.0Float % 2.0Float { ok(v) => v }; y }")
            .unwrap_err();
        assert!(
            err.message.contains("modulo") && err.message.contains("Float"),
            "{}",
            err.message
        );
    }

    // B35 P3d-iii: Fixed checked arithmetic. Fixed is signed, so it
    // admits the same outcomes as Word, but binds a single result.

    #[test]
    fn checked_fixed_div_zero_divisor_typechecks() {
        check_src(
            "fn main() -> Fixed<16> { let y = 6Fixed<16> / 0Fixed<16> { ok(q) => q, zero_divisor(n) => n }; y }",
        )
        .expect("Fixed checked division with a zero_divisor arm should typecheck");
    }

    #[test]
    fn checked_fixed_overflow_single_pattern_typechecks() {
        check_src(
            "fn main() -> Fixed<16> { let y = 3Fixed<16> * 4Fixed<16> { ok(v) => v, overflow(w) => w }; y }",
        )
        .expect("Fixed checked multiply with a single-pattern overflow arm should typecheck");
    }

    #[test]
    fn checked_fixed_overflow_two_patterns_rejected() {
        // The two-pattern (h, l) form is the Word shape; Fixed binds a
        // single result.
        let err = check_src(
            "fn main() -> Fixed<16> { let y = 3Fixed<16> * 4Fixed<16> { ok(v) => v, overflow(h, l) => h }; y }",
        )
        .unwrap_err();
        assert!(err.message.contains("single result"), "{}", err.message);
    }

    #[test]
    fn checked_fixed_nan_arm_rejected() {
        // Fixed arithmetic never produces NaN.
        let err = check_src(
            "fn main() -> Fixed<16> { let y = 6Fixed<16> / 2Fixed<16> { ok(q) => q, nan(_) => 0Fixed<16> }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible")
                && err.message.contains("nan")
                && err.message.contains("Fixed"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_fixed_zero_divisor_rejected_on_addition() {
        // Addition has no zero divisor.
        let err = check_src(
            "fn main() -> Fixed<16> { let y = 3Fixed<16> + 4Fixed<16> { ok(v) => v, zero_divisor(_) => 0Fixed<16> }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible")
                && err.message.contains("zero_divisor")
                && err.message.contains("Fixed"),
            "{}",
            err.message
        );
    }

    // B35 P4: the indexing construct.

    #[test]
    fn checked_index_ok_and_invalid_index_typecheck() {
        check_src(
            "fn main() -> Word { let a = [10, 20, 30]; let y = a[1] { ok(v) => v, invalid_index(i) => i }; y }",
        )
        .expect("indexing construct with ok and invalid_index should typecheck");
    }

    #[test]
    fn checked_index_ok_only_typechecks() {
        // invalid_index is optional; ok alone is exhaustive.
        check_src("fn main() -> Word { let a = [10, 20, 30]; let y = a[1] { ok(v) => v }; y }")
            .expect("indexing construct with only ok should typecheck");
    }

    #[test]
    fn checked_index_arithmetic_arm_rejected() {
        let err = check_src(
            "fn main() -> Word { let a = [10, 20, 30]; let y = a[1] { ok(v) => v, overflow(w) => w }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("array indexing"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_invalid_index_arm_rejected_on_arithmetic() {
        let err = check_src(
            "fn main() -> Word { let y = 1 + 2 { ok(v) => v, invalid_index(i) => i }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("invalid_index"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_index_non_exhaustive_ok_rejected() {
        let err = check_src(
            "fn main() -> Word { let a = [10, 20, 30]; let y = a[1] { invalid_index(i) => i }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("non-exhaustive on `ok`"),
            "{}",
            err.message
        );
    }

    // B35 P5: the newtype-construction construct.

    const REFINED_NT_PRELUDE: &str =
        "fn is_pos(x: Word) -> bool { x > 0 }\nnewtype Positive = Word where is_pos;\n";

    #[test]
    fn checked_newtype_ok_and_invalid_typecheck() {
        let src = alloc::format!(
            "{}fn main() -> Word {{ let y = Positive(5) {{ ok(p) => 1, invalid_newtype(x) => x }}; y }}",
            REFINED_NT_PRELUDE
        );
        check_src(&src).expect("refined newtype construct should typecheck");
    }

    #[test]
    fn checked_newtype_invalid_arm_rejected_when_unrefined() {
        // A non-refined newtype's construction is total, so
        // invalid_newtype cannot arise.
        let err = check_src(
            "newtype Meters = Word;\nfn main() -> Word { let y = Meters(5) { ok(m) => 1, invalid_newtype(x) => x }; y }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("no refinement predicate"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_newtype_arithmetic_arm_rejected() {
        let src = alloc::format!(
            "{}fn main() -> Word {{ let y = Positive(5) {{ ok(p) => 1, overflow(w) => w }}; y }}",
            REFINED_NT_PRELUDE
        );
        let err = check_src(&src).unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("newtype construction"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_newtype_non_exhaustive_ok_rejected() {
        let src = alloc::format!(
            "{}fn main() -> Word {{ let y = Positive(5) {{ invalid_newtype(x) => x }}; y }}",
            REFINED_NT_PRELUDE
        );
        let err = check_src(&src).unwrap_err();
        assert!(
            err.message.contains("non-exhaustive on `ok`"),
            "{}",
            err.message
        );
    }

    // B35 P6: the discriminant-to-enum construct.

    const DISC_ENUM_PRELUDE: &str = "enum Color { Red = 0, Green = 1, Custom(Word) = 2 }\n";

    #[test]
    fn checked_discriminant_typechecks() {
        let src = alloc::format!(
            "{}fn main() -> Color {{ 0 as Color {{ ok(Red) => Color::Green, payload_discriminant(Custom) => Color::Custom(0), invalid_discriminant(r) => Color::Red }} }}",
            DISC_ENUM_PRELUDE
        );
        check_src(&src).expect("discriminant-to-enum construct should typecheck");
    }

    #[test]
    fn checked_discriminant_payload_in_ok_rejected() {
        let src = alloc::format!(
            "{}fn main() -> Color {{ 0 as Color {{ ok(Custom) => Color::Red, payload_discriminant(_) => Color::Red }} }}",
            DISC_ENUM_PRELUDE
        );
        let err = check_src(&src).unwrap_err();
        assert!(
            err.message.contains("payload-bearing variant `Custom`"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_discriminant_unit_in_payload_rejected() {
        let src = alloc::format!(
            "{}fn main() -> Color {{ 0 as Color {{ payload_discriminant(Red) => Color::Red, payload_discriminant(Custom) => Color::Custom(0) }} }}",
            DISC_ENUM_PRELUDE
        );
        let err = check_src(&src).unwrap_err();
        assert!(
            err.message.contains("not a payload-bearing variant"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_discriminant_uncovered_payload_rejected() {
        let src = alloc::format!(
            "{}fn main() -> Color {{ 0 as Color {{ ok(Red) => Color::Green }} }}",
            DISC_ENUM_PRELUDE
        );
        let err = check_src(&src).unwrap_err();
        assert!(
            err.message
                .contains("does not cover the payload-bearing variant"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_discriminant_non_word_source_rejected() {
        // The source of the conversion must be a Word. A Byte source
        // is rejected (Byte is available without the floats feature).
        let src = alloc::format!(
            "{}fn main() -> Color {{ 5Byte as Color {{ payload_discriminant(_) => Color::Red }} }}",
            DISC_ENUM_PRELUDE
        );
        let err = check_src(&src).unwrap_err();
        assert!(
            err.message.contains("requires a Word source"),
            "{}",
            err.message
        );
    }

    // B35 P7: the native-error construct.

    #[test]
    fn checked_native_ok_and_error_typecheck() {
        check_src(
            "use host::f\nfn main() -> Word { host::f(1) { ok(v) => v, error(code) => code } }",
        )
        .expect("native-error construct should typecheck");
    }

    #[test]
    fn checked_native_arithmetic_arm_rejected() {
        let err = check_src(
            "use host::f\nfn main() -> Word { host::f(1) { ok(v) => v, overflow(w) => w } }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("not admissible") && err.message.contains("native call"),
            "{}",
            err.message
        );
    }

    #[test]
    fn checked_native_non_exhaustive_ok_rejected() {
        let err =
            check_src("use host::f\nfn main() -> Word { host::f(1) { error(code) => code } }")
                .unwrap_err();
        assert!(
            err.message.contains("non-exhaustive on `ok`"),
            "{}",
            err.message
        );
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_float_overflow_two_patterns_rejected() {
        // Float overflow binds a single result, not two halves.
        let err = check_src(
            "fn main() -> Float { let y = 1.0Float + 2.0Float { ok(v) => v, overflow(h, l) => h }; y }",
        )
        .unwrap_err();
        assert!(err.message.contains("single result"), "{}", err.message);
    }

    #[test]
    fn checked_overflow_rejects_non_arithmetic_op() {
        // The construct rejects non-arithmetic operations such
        // as comparison and logical ops.
        let err = check_src(
            "fn main() -> bool {\n\
                let y = 1 == 2 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => false,\n\
                    underflow(_, _) => false,\n\
                };\n\
                y\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.message
                .contains("only `+`, `-`, `*`, `/`, `%`, and unary `-`"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_signature_validated() {
        // The predicate must be a declared function taking the
        // newtype's underlying type and returning Bool.
        check_src(
            "fn in_range(x: Word) -> bool { x >= 0 and x <= 100 }\n\
             newtype Percent = Word where in_range;\n\
             fn main() -> Percent { Percent(50) }",
        )
        .expect("well-formed refinement should type-check");
    }

    #[test]
    fn refinement_predicate_must_exist() {
        let err = check_src(
            "newtype Percent = Word where in_range;\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("is not declared"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_must_return_bool() {
        let err = check_src(
            "fn returns_word(x: Word) -> Word { x }\n\
             newtype Percent = Word where returns_word;\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("must return Bool"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_param_must_match_underlying() {
        let err = check_src(
            "fn from_bool(x: bool) -> bool { x }\n\
             newtype Percent = Word where from_bool;\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("does not match newtype"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn refinement_compiles_to_call_trap_pair() {
        // Verifies the end-to-end compile path. The runtime check
        // is tested through actual VM execution in vm.rs.
        compile_src(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn main() -> Counter { Counter(42) }",
        )
        .expect("refined newtype with passing argument should compile");
    }

    #[test]
    fn simple_function_type_checks() {
        check_src("fn main() -> Word { 1 + 2 }").unwrap();
    }

    #[test]
    fn return_type_mismatch_rejected() {
        let err = check_src("fn main() -> Word { true }").unwrap_err();
        assert!(err.message.contains("returns Word"));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn arithmetic_type_mismatch_rejected() {
        let err = check_src("fn main() -> Word { 1 + 2.0 }").unwrap_err();
        assert!(err.message.contains("cannot add"));
    }

    #[test]
    fn function_call_arg_count_checked() {
        let err =
            check_src("fn add(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { add(1) }")
                .unwrap_err();
        assert!(err.message.contains("expects 2"));
    }

    #[test]
    fn function_call_arg_type_checked() {
        let err =
            check_src("fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(true) }")
                .unwrap_err();
        assert!(err.message.contains("expects Word"));
    }

    #[test]
    fn let_binding_type_mismatch_rejected() {
        let err = check_src("fn main() -> Word { let x: Word = true; 0 }").unwrap_err();
        assert!(err.message.contains("declared as Word"));
    }

    #[test]
    fn let_binding_inferred_from_value() {
        check_src("fn main() -> Word { let x = 1; x + 1 }").unwrap();
    }

    #[test]
    fn numeric_suffix_word_typechecks() {
        check_src("fn main() -> Word { 5Word }").unwrap();
    }

    #[test]
    fn numeric_suffix_byte_checked_against_expected_type() {
        // The `Byte` suffix pins the literal's type, so using it where
        // a `Word` is expected is a type error.
        let err = check_src("fn main() -> Word { let x: Word = 5Byte; x }").unwrap_err();
        assert!(
            err.message.contains("Word") && err.message.contains("Byte"),
            "{}",
            err.message
        );
    }

    #[test]
    fn numeric_suffix_fixed_carries_fraction_bits_in_type() {
        // `Fixed<16>` and `Fixed<32>` are distinct types, so a
        // `Fixed<16>` literal does not satisfy a `Fixed<32>` binding.
        let err = check_src("fn main() -> Word { let x: Fixed<32> = 1Fixed<16>; 0 }").unwrap_err();
        assert!(err.message.contains("Fixed"), "{}", err.message);
    }

    #[test]
    fn if_branch_mismatch_rejected() {
        let err = check_src("fn main() -> Word { if true { 1 } else { false } }").unwrap_err();
        assert!(err.message.contains("if branches"));
    }

    #[test]
    fn struct_field_access_checks() {
        check_src(
            "struct P { x: Word, y: Word }\nfn main() -> Word { let p = P { x: 1, y: 2 }; p.x }",
        )
        .unwrap();
    }

    #[test]
    fn struct_unknown_field_rejected() {
        let err = check_src("struct P { x: Word }\nfn main() -> Word { let p = P { x: 1 }; p.y }")
            .unwrap_err();
        assert!(err.message.contains("no field"));
    }

    #[test]
    fn cast_int_to_float_admissible() {
        check_src("fn main() -> Float { let x: Word = 1; x as Float }").unwrap();
    }

    #[test]
    fn cast_bool_to_int_rejected() {
        let err = check_src("fn main() -> Word { true as Word }").unwrap_err();
        assert!(err.message.contains("cannot cast"));
    }

    #[test]
    fn undefined_identifier_rejected() {
        let err = check_src("fn main() -> Word { x }").unwrap_err();
        assert!(err.message.contains("undefined"));
    }

    // -- #13 Native function call types --

    #[test]
    fn undefined_function_rejected() {
        let err = check_src("fn main() -> Word { foo() }").unwrap_err();
        assert!(err.message.contains("undefined function `foo`"));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn used_native_accepted() {
        check_src("use math::sqrt\nfn main() -> Float { math::sqrt(9.0) }").unwrap();
    }

    #[test]
    #[cfg(feature = "floats")]
    fn qualified_call_treated_as_native() {
        // Qualified names with `::` are treated as natives even
        // without an explicit `use` declaration.
        check_src("fn main() -> () { audio::do_thing(1, 2.0) }").unwrap();
    }

    // -- #12 Pattern type checking against scrutinee --

    #[test]
    fn enum_pattern_unknown_variant_rejected() {
        let err = check_src(
            "enum Color { Red, Green }\n\
             fn main() -> Word { match Color::Red() { Color::Blue() => 1, _ => 0 } }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("no variant `Blue`")
                || err.message.contains("unknown enum variant"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn enum_pattern_wrong_arity_rejected() {
        let err = check_src(
            "enum Shape { Square(Word), Circle(Word) }\n\
             fn main() -> Word { match Shape::Square(1) { Shape::Square(a, b) => 0, _ => 1 } }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("payload elements"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn tuple_pattern_wrong_arity_rejected() {
        let err =
            check_src("fn main() -> Word { match (1, 2) { (a, b, c) => 0, _ => 1 } }").unwrap_err();
        assert!(err.message.contains("tuple pattern"));
    }

    #[test]
    fn tuple_pattern_against_non_tuple_rejected() {
        let err = check_src("fn main() -> Word { match 5 { (a, b) => 0, _ => 1 } }").unwrap_err();
        assert!(err.message.contains("tuple pattern"));
    }

    #[test]
    fn literal_pattern_type_mismatch_rejected() {
        let err = check_src("fn main() -> Word { match 5 { true => 1, _ => 0 } }").unwrap_err();
        assert!(err.message.contains("literal pattern"));
    }

    // -- #11 Match arm exhaustiveness --

    #[test]
    fn enum_match_missing_variant_rejected() {
        let err = check_src(
            "enum Color { Red, Green, Blue }\n\
             fn main() -> Word { match Color::Red() { Color::Red() => 0, Color::Green() => 1 } }",
        )
        .unwrap_err();
        assert!(err.message.contains("non-exhaustive match"));
        assert!(err.message.contains("Blue"));
    }

    #[test]
    fn enum_match_with_wildcard_accepted() {
        check_src(
            "enum Color { Red, Green, Blue }\n\
             fn main() -> Word { match Color::Red() { Color::Red() => 0, _ => 1 } }",
        )
        .unwrap();
    }

    #[test]
    fn enum_match_with_all_variants_accepted() {
        check_src(
            "enum Color { Red, Green }\n\
             fn main() -> Word { match Color::Red() { Color::Red() => 0, Color::Green() => 1 } }",
        )
        .unwrap();
    }

    #[test]
    fn bool_match_missing_arm_rejected() {
        let err = check_src("fn main() -> Word { match true { true => 1 } }").unwrap_err();
        assert!(err.message.contains("non-exhaustive match"));
    }

    #[test]
    fn bool_match_complete_accepted() {
        check_src("fn main() -> Word { match true { true => 1, false => 0 } }").unwrap();
    }

    #[test]
    fn i64_match_without_wildcard_rejected() {
        let err = check_src("fn main() -> Word { match 1 { 1 => 1, 2 => 2 } }").unwrap_err();
        assert!(err.message.contains("non-exhaustive match"));
    }

    #[test]
    fn i64_match_with_wildcard_accepted() {
        check_src("fn main() -> Word { match 1 { 1 => 1, _ => 0 } }").unwrap();
    }

    // -- Hindley-Milner foundation primitives --

    #[test]
    fn vargen_allocates_fresh_variables() {
        let mut g = VarGen::default();
        let a = g.fresh();
        let b = g.fresh();
        match (a, b) {
            (Type::Var(0), Type::Var(1)) => {}
            other => panic!("expected fresh variables 0 and 1, got {:?}", other),
        }
        assert_eq!(g.count(), 2);
    }

    #[test]
    fn unify_identical_primitives() {
        let mut s = Subst::new();
        unify(&Type::Word, &Type::Word, &mut s).unwrap();
        unify(&Type::Bool, &Type::Bool, &mut s).unwrap();
        unify(&Type::Unit, &Type::Unit, &mut s).unwrap();
        unify(&Type::Str, &Type::Str, &mut s).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn unify_distinct_primitives_fails() {
        let mut s = Subst::new();
        let err = unify(&Type::Word, &Type::Float, &mut s).unwrap_err();
        match err {
            UnifyError::Mismatch { left, right } => {
                assert_eq!(left, Type::Word);
                assert_eq!(right, Type::Float);
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn unify_var_with_concrete_binds() {
        let mut s = Subst::new();
        unify(&Type::Var(0), &Type::Word, &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::Word));
    }

    #[test]
    fn unify_concrete_with_var_binds() {
        let mut s = Subst::new();
        unify(&Type::Word, &Type::Var(0), &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::Word));
    }

    #[test]
    fn unify_var_with_var_binds_one_to_other() {
        let mut s = Subst::new();
        unify(&Type::Var(0), &Type::Var(1), &mut s).unwrap();
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn unify_same_var_succeeds_with_no_binding() {
        let mut s = Subst::new();
        unify(&Type::Var(0), &Type::Var(0), &mut s).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn unify_tuple_pairwise() {
        let mut s = Subst::new();
        let t1 = Type::Tuple(alloc::vec![Type::Var(0), Type::Bool]);
        let t2 = Type::Tuple(alloc::vec![Type::Word, Type::Var(1)]);
        unify(&t1, &t2, &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::Word));
        assert_eq!(s.get(1), Some(&Type::Bool));
    }

    #[test]
    fn unify_tuple_arity_mismatch() {
        let mut s = Subst::new();
        let t1 = Type::Tuple(alloc::vec![Type::Word, Type::Bool]);
        let t2 = Type::Tuple(alloc::vec![Type::Word]);
        let err = unify(&t1, &t2, &mut s).unwrap_err();
        match err {
            UnifyError::TupleArityMismatch { left, right } => {
                assert_eq!(left, 2);
                assert_eq!(right, 1);
            }
            other => panic!("expected TupleArityMismatch, got {:?}", other),
        }
    }

    #[test]
    fn unify_array_length_mismatch() {
        let mut s = Subst::new();
        let t1 = Type::Array(Box::new(Type::Word), ConstDim::Known(3));
        let t2 = Type::Array(Box::new(Type::Word), ConstDim::Known(4));
        let err = unify(&t1, &t2, &mut s).unwrap_err();
        match err {
            UnifyError::ArrayLengthMismatch { left, right } => {
                assert_eq!(left, "3");
                assert_eq!(right, "4");
            }
            other => panic!("expected ArrayLengthMismatch, got {:?}", other),
        }
    }

    #[test]
    fn unify_array_element_types_unify() {
        let mut s = Subst::new();
        let t1 = Type::Array(Box::new(Type::Var(0)), ConstDim::Known(3));
        let t2 = Type::Array(Box::new(Type::Word), ConstDim::Known(3));
        unify(&t1, &t2, &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::Word));
    }

    #[test]
    fn unify_option_inner_types_unify() {
        let mut s = Subst::new();
        let t1 = Type::Option(Box::new(Type::Var(0)));
        let t2 = Type::Option(Box::new(Type::Bool));
        unify(&t1, &t2, &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::Bool));
    }

    #[test]
    fn unify_named_struct_same_name_succeeds() {
        let mut s = Subst::new();
        unify(
            &Type::Struct("Point".to_string(), Vec::new()),
            &Type::Struct("Point".to_string(), Vec::new()),
            &mut s,
        )
        .unwrap();
    }

    #[test]
    fn unify_named_struct_different_name_fails() {
        let mut s = Subst::new();
        let err = unify(
            &Type::Struct("Point".to_string(), Vec::new()),
            &Type::Struct("Square".to_string(), Vec::new()),
            &mut s,
        )
        .unwrap_err();
        assert!(matches!(err, UnifyError::Mismatch { .. }));
    }

    #[test]
    fn unify_occurs_check_rejects_self_reference() {
        let mut s = Subst::new();
        // ?T0 ~ Tuple(?T0, i64) would create an infinite type.
        let t1 = Type::Var(0);
        let t2 = Type::Tuple(alloc::vec![Type::Var(0), Type::Word]);
        let err = unify(&t1, &t2, &mut s).unwrap_err();
        assert!(matches!(err, UnifyError::OccursCheck { .. }));
    }

    #[test]
    fn apply_substitution_resolves_variable() {
        let mut s = Subst::new();
        s.insert(0, Type::Word);
        let t = Type::Tuple(alloc::vec![Type::Var(0), Type::Bool]);
        let resolved = t.apply(&s);
        assert_eq!(resolved, Type::Tuple(alloc::vec![Type::Word, Type::Bool]));
    }

    #[test]
    fn apply_substitution_resolves_chain() {
        // ?T0 -> ?T1 -> Bool. Applying once should follow the chain.
        let mut s = Subst::new();
        s.insert(0, Type::Var(1));
        s.insert(1, Type::Bool);
        let resolved = Type::Var(0).apply(&s);
        assert_eq!(resolved, Type::Bool);
    }

    #[test]
    fn unify_propagates_through_existing_substitution() {
        // After ?T0 ~ i64, unifying ?T0 ~ ?T1 should bind ?T1 to i64.
        let mut s = Subst::new();
        unify(&Type::Var(0), &Type::Word, &mut s).unwrap();
        unify(&Type::Var(0), &Type::Var(1), &mut s).unwrap();
        let resolved = Type::Var(1).apply(&s);
        assert_eq!(resolved, Type::Word);
    }

    // -- B2 generic function checks --

    #[test]
    fn generic_identity_function_typechecks() {
        check_src("fn id<T>(x: T) -> T { x }\nfn main() -> Word { id(42) }").unwrap();
    }

    #[test]
    fn generic_function_called_with_two_types_separately() {
        // Two distinct call sites instantiate the type parameter
        // separately, so the same generic function flows through
        // both i64 and bool.
        check_src(
            "fn id<T>(x: T) -> T { x }\n\
             fn main() -> Word {\n\
                let a = id(1);\n\
                let b = id(true);\n\
                a\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn generic_function_with_two_type_params() {
        check_src(
            "fn first<T, U>(a: T, b: U) -> T { a }\n\
             fn main() -> Word { first(1, true) }",
        )
        .unwrap();
    }

    #[test]
    fn generic_function_arity_mismatch_rejected() {
        let err =
            check_src("fn id<T>(x: T) -> T { x }\nfn main() -> Word { id(1, 2) }").unwrap_err();
        assert!(err.message.contains("expects 1 arguments"));
    }

    // -- B2.2 generic struct and enum checks --

    #[test]
    fn generic_struct_with_one_param_typechecks() {
        check_src(
            "struct Cell<T> { value: T }\n\
             fn main() -> Word {\n\
                let c = Cell { value: 42 };\n\
                c.value\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn generic_struct_with_two_params_typechecks() {
        check_src(
            "struct Pair<T, U> { a: T, b: U }\n\
             fn main() -> Word {\n\
                let p = Pair { a: 1, b: true };\n\
                p.a\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn generic_struct_field_access_uses_instantiation() {
        check_src(
            "struct Cell<T> { value: T }\n\
             fn main() -> Word {\n\
                let p = Cell { value: 1 };\n\
                let q = Cell { value: true };\n\
                let _ = q.value;\n\
                p.value\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn generic_enum_construction_typechecks() {
        check_src(
            "enum Maybe<T> { Just(T), Nothing }\n\
             fn main() -> Word {\n\
                let m = Maybe::Just(42);\n\
                0\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn generic_struct_pattern_match_on_enum() {
        // Pattern matching on a generic enum binds the payload to
        // the instantiated type. The match arm's expression returns
        // the instantiated type, which unifies with the surrounding
        // function's return type.
        check_src(
            "enum Maybe<T> { Just(T), Nothing }\n\
             fn main() -> Word {\n\
                let m = Maybe::Just(42);\n\
                match m {\n\
                    Maybe::Just(x) => x,\n\
                    Maybe::Nothing => 0,\n\
                }\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn generic_struct_referenced_by_field_type() {
        // A generic struct used as a field type expression in another
        // struct. `inner: Cell<T>` parses as a generic instantiation
        // and resolves under the outer struct's type parameter scope.
        check_src(
            "struct Cell<T> { value: T }\n\
             struct Wrap<T> { inner: Cell<T> }\n\
             fn main() -> Word {\n\
                let w = Wrap { inner: Cell { value: 7 } };\n\
                w.inner.value\n\
             }",
        )
        .unwrap();
    }

    // -- B2.3 trait declarations and bounds --

    #[test]
    fn trait_declaration_parses_and_typechecks() {
        check_src(
            "trait Numeric { fn one() -> Word; }\n\
             impl Numeric for Word { fn one() -> Word { 1 } }\n\
             fn use_it<T: Numeric>(x: T) -> T { x }\n\
             fn main() -> Word { use_it(7) }",
        )
        .unwrap();
    }

    #[test]
    fn trait_bound_satisfied_by_impl() {
        // When the bound's required impl exists for the call's
        // argument type, the call type-checks.
        check_src(
            "trait Tag { fn tag() -> Word; }\n\
             impl Tag for bool { fn tag() -> Word { 1 } }\n\
             fn use_tag<T: Tag>(x: T) -> Word { 0 }\n\
             fn main() -> Word { use_tag(true) }",
        )
        .unwrap();
    }

    #[test]
    fn trait_bound_unsatisfied_rejects_call() {
        // With an impl for `bool` only, calling with an `i64`
        // argument should fail bound validation because no `Tag`
        // impl exists for `i64`.
        let err = check_src(
            "trait Tag { fn tag() -> Word; }\n\
             impl Tag for bool { fn tag() -> Word { 1 } }\n\
             fn use_tag<T: Tag>(x: T) -> Word { 0 }\n\
             fn main() -> Word { use_tag(7) }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("does not implement"),
            "unexpected error: {}",
            err.message,
        );
    }

    #[test]
    fn unbounded_type_param_admits_any_type() {
        // Without a trait bound, any concrete argument is accepted.
        check_src(
            "fn id<T>(x: T) -> T { x }\n\
             fn main() -> Word { id(42) }",
        )
        .unwrap();
    }

    #[test]
    fn multiple_trait_bounds_on_one_param() {
        check_src(
            "trait A { fn a() -> Word; }\n\
             trait B { fn b() -> Word; }\n\
             impl A for Word { fn a() -> Word { 1 } }\n\
             impl B for Word { fn b() -> Word { 2 } }\n\
             fn use_both<T: A + B>(x: T) -> Word { 0 }\n\
             fn main() -> Word { use_both(7) }",
        )
        .unwrap();
    }

    #[test]
    fn impl_method_with_extra_method_rejected() {
        // The trait does not declare `extra`, so the impl is invalid.
        let err = check_src(
            "trait T { fn one() -> Word; }\n\
             impl T for Word {\n\
                fn one() -> Word { 1 }\n\
                fn extra() -> Word { 2 }\n\
             }\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(err.message.contains("not in the trait"));
    }

    #[test]
    fn impl_method_arity_mismatch_rejected() {
        // The trait declares `fn one() -> i64` (arity zero); the impl
        // supplies `fn one(x: i64) -> i64` (arity one). Arity mismatch.
        let err = check_src(
            "trait T { fn one() -> Word; }\n\
             impl T for Word { fn one(x: Word) -> Word { x } }\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(err.message.contains("parameter"));
    }

    #[test]
    fn monomorphize_generic_method_dispatch() {
        // Inside a generic function body, the receiver's type is the
        // abstract type parameter. Monomorphization specializes the
        // function per concrete call-site type, after which the
        // method call resolves to the impl's mangled function.
        check_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }\n\
             fn main() -> Word { use_doubler(21) }",
        )
        .unwrap();
    }

    #[test]
    fn method_call_resolves_to_impl() {
        check_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn main() -> Word {\n\
                let n: Word = 21;\n\
                n.double()\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn method_call_unknown_method_rejected() {
        let err = check_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn main() -> Word {\n\
                let n: Word = 21;\n\
                n.triple()\n\
             }",
        )
        .unwrap_err();
        assert!(err.message.contains("no method"));
    }

    #[test]
    fn closure_rejected_at_typecheck() {
        // V0.2.0 Phase 4 retired the closure family. The type
        // checker now rejects `Expr::Closure` directly rather than
        // accepting the program and relying on the verifier to
        // reject the resulting bytecode.
        let err = check_src(
            "fn main() -> Word {\n\
                let f = |x: Word| x + 1;\n\
                f(41)\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("closures are not supported"),
            "unexpected error: {}",
            err.message,
        );
    }

    #[test]
    fn monomorphize_inference_through_function_call() {
        // The generic call site uses a function call as argument.
        // Inference reach now resolves the call's return type and
        // specializes the generic function for the resulting type.
        check_src(
            "fn make42() -> Word { 42 }\n\
             fn id<T>(x: T) -> T { x }\n\
             fn main() -> Word { id(make42()) }",
        )
        .unwrap();
    }

    #[test]
    #[cfg(feature = "verify")]
    fn recursive_closure_rejected_at_typecheck() {
        // V0.2.0 Phase 4 retired the closure family. The type
        // checker rejects the closure expression directly; the
        // compile pipeline never sees `MakeRecursiveClosure` or
        // `CallIndirect` bytecode because the opcodes themselves
        // are gone.
        let err = compile_src(
            "fn main() -> Word {\n\
                let fact = |n: Word| if n <= 1 { 1 } else { n * fact(n - 1) };\n\
                fact(5)\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.contains("closures are not supported"),
            "unexpected error: {}",
            err,
        );
    }

    #[test]
    #[cfg(feature = "verify")]
    fn recursive_closure_with_capture_rejected_at_typecheck() {
        // A recursive closure that also captures an outer-function
        // local is rejected at the same stage.
        let err = compile_src(
            "fn main() -> Word {\n\
                let base: Word = 1000;\n\
                let fact = |n: Word| if n <= 1 { base } else { n * fact(n - 1) };\n\
                fact(3)\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.contains("closures are not supported"),
            "unexpected error: {}",
            err,
        );
    }

    #[test]
    fn monomorphize_inference_through_field_access() {
        // A generic call site whose argument is a field access on a
        // local-typed struct should specialize the call. The full
        // compile pipeline must succeed because the receiver
        // resolves to a concrete type only after monomorphization.
        compile_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             struct Holder { value: Word }\n\
             fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }\n\
             fn main() -> Word {\n\
                 let h = Holder { value: 21 };\n\
                 use_doubler(h.value)\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn monomorphize_enum_specialization_round_trip() {
        // Generic enum specialization mirrors struct specialization.
        // Construction with a concrete payload value generates a
        // specialized EnumDef with the payload type substituted.
        check_src(
            "enum Maybe<T> { Just(T), Nothing }\n\
             fn main() -> Word {\n\
                let m = Maybe::Just(42);\n\
                match m {\n\
                    Maybe::Just(x) => x,\n\
                    Maybe::Nothing => 0,\n\
                }\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn monomorphize_struct_field_method_dispatch() {
        // Method dispatch on a generic struct's field. Generic
        // struct specialization gives the field a concrete type so
        // the method call resolves to the impl.
        check_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             struct Cell<T> { value: T }\n\
             fn main() -> Word {\n\
                let c = Cell { value: 21 };\n\
                c.value.double()\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn closure_passed_as_argument_rejected() {
        // V0.2.0 Phase 4: closures are rejected by the type
        // checker. The function-as-parameter pattern requires
        // a closure-shaped expression at the call site, which
        // is no longer permitted.
        let err = check_src(
            "fn apply<F>(f: F, x: Word) -> Word { f(x) }\n\
             fn main() -> Word {\n\
                let g = |x: Word| x + 1;\n\
                apply(g, 41)\n\
             }",
        )
        .unwrap_err();
        assert!(err.message.contains("closures are not supported"));
    }

    #[test]
    fn closure_captures_outer_local_rejected() {
        let err = check_src(
            "fn main() -> Word {\n\
                let n: Word = 10;\n\
                let f = |x: Word| x + n;\n\
                f(5)\n\
             }",
        )
        .unwrap_err();
        assert!(err.message.contains("closures are not supported"));
    }

    #[test]
    fn closure_no_param_rejected() {
        let err = check_src(
            "fn main() -> Word {\n\
                let f = || 42;\n\
                f()\n\
             }",
        )
        .unwrap_err();
        assert!(err.message.contains("closures are not supported"));
    }

    #[test]
    fn closure_nested_inside_closure_rejected() {
        // The outer closure is rejected before the inner one is
        // inspected; the type checker errors at the first closure
        // expression encountered.
        let err = check_src(
            "fn main() -> Word {\n\
                let outer = |x: Word| {\n\
                    let inner = |y: Word| x + y;\n\
                    inner(5)\n\
                };\n\
                outer(7)\n\
             }",
        )
        .unwrap_err();
        assert!(err.message.contains("closures are not supported"));
    }

    #[test]
    fn monomorphize_inference_through_tuple_index() {
        // The argument is a tuple-index expression. The monomorphize
        // pass infers the type from the indexed tuple's element list,
        // which must succeed for the receiver's method dispatch to
        // resolve in the specialized body.
        compile_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }\n\
             fn main() -> Word {\n\
                 let t = (21, true);\n\
                 use_doubler(t.0)\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn monomorphize_inference_through_method_call() {
        // The argument is a method call. The monomorphize pass infers
        // the result type from the impl method's declared return
        // type, looked up under the `<head>::<method>` key in the
        // function-return map populated from program.impls.
        compile_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }\n\
             fn main() -> Word {\n\
                 use_doubler((21).double())\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn monomorphize_inference_through_unary_op() {
        // The argument is `-n` where `n: i64`. UnaryOp::Neg preserves
        // the operand type.
        compile_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }\n\
             fn main() -> Word {\n\
                 let n: Word = 21;\n\
                 use_doubler(-n)\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn monomorphize_inference_through_bin_op() {
        // The argument is `n + 11`. Arithmetic BinOps preserve operand
        // type, which the analysis takes from the left operand.
        compile_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }\n\
             fn main() -> Word {\n\
                 let n: Word = 10;\n\
                 use_doubler(n + 11)\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn monomorphize_inference_through_array_index() {
        // The argument is an array index expression. The monomorphize
        // pass infers the element type from the array's declared
        // element type. The full compile pipeline must succeed.
        compile_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> Word { x + x } }\n\
             fn use_doubler<T: Doubler>(x: T) -> Word { x.double() }\n\
             fn main() -> Word {\n\
                 let a: [Word; 2] = [21, 42];\n\
                 use_doubler(a[0])\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn closure_nested_capturing_outer_local_rejected() {
        let err = check_src(
            "fn main() -> Word {\n\
                let base: Word = 100;\n\
                let outer = |x: Word| {\n\
                    let inner = |y: Word| base + x + y;\n\
                    inner(3)\n\
                };\n\
                outer(7)\n\
             }",
        )
        .unwrap_err();
        assert!(err.message.contains("closures are not supported"));
    }

    #[test]
    fn impl_method_param_type_mismatch_rejected() {
        // Trait declares `fn double(x: i64) -> i64` but the impl
        // supplies `fn double(x: bool) -> i64`. Parameter type
        // mismatch must be rejected.
        let err = check_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: bool) -> Word { 0 } }\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("parameter") || err.message.contains("type"),
            "unexpected error: {}",
            err.message,
        );
    }

    #[test]
    fn impl_method_return_type_mismatch_rejected() {
        // Trait declares `fn double(x: i64) -> i64` but the impl
        // returns `bool`. Return type mismatch must be rejected.
        let err = check_src(
            "trait Doubler { fn double(x: Word) -> Word; }\n\
             impl Doubler for Word { fn double(x: Word) -> bool { true } }\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(err.message.contains("returns"));
    }

    #[test]
    fn impl_for_unknown_trait_rejected() {
        let err = check_src(
            "impl Nonexistent for Word { fn x() -> Word { 0 } }\n\
             fn main() -> Word { 0 }",
        )
        .unwrap_err();
        assert!(err.message.contains("unknown trait"));
    }

    #[test]
    fn missing_one_of_multiple_bounds_rejected() {
        // i64 implements A but not B, so a call requiring T: A + B
        // with i64 must fail.
        let err = check_src(
            "trait A { fn a() -> Word; }\n\
             trait B { fn b() -> Word; }\n\
             impl A for Word { fn a() -> Word { 1 } }\n\
             fn use_both<T: A + B>(x: T) -> Word { 0 }\n\
             fn main() -> Word { use_both(7) }",
        )
        .unwrap_err();
        assert!(err.message.contains("does not implement"));
    }

    #[test]
    fn generic_struct_same_type_param_constraint() {
        // The struct expects both fields to share the same T. With the
        // unifier in place, providing inconsistent types unifies T
        // against incompatible values and surfaces a type error.
        let err = check_src(
            "struct SamePair<T> { a: T, b: T }\n\
             fn main() -> Word {\n\
                let p = SamePair { a: 1, b: true };\n\
                0\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("expects") || err.message.contains("type"),
            "unexpected error message: {}",
            err.message,
        );
    }

    #[test]
    fn ephemeral_modifier_on_non_entry_function_rejected() {
        // The `ephemeral` modifier is a whole-module property and
        // belongs on the entry point only. Attaching it to a helper
        // function is a category error.
        let err = check_src(
            "ephemeral fn helper() -> Word { 0 }\n\
             fn main() -> Word { helper() }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("ephemeral") && err.message.contains("main"),
            "unexpected error message: {}",
            err.message,
        );
    }

    #[test]
    fn ephemeral_modifier_on_main_accepted() {
        // The entry point may carry the modifier. The type checker
        // accepts; the verifier (in a later phase) will check the
        // ephemerality proof.
        check_src("ephemeral fn main() -> Word { 0 }").expect("typecheck accepts ephemeral main");
    }

    // --- Negative IFC labels (R43) -------------------------

    #[test]
    fn negative_label_single_shorthand_parses_and_admits_unlabelled_arg() {
        // `Word@!Secret` on a native parameter admits any source
        // labels except Secret. The bare `0` literal carries no
        // labels and flows in cleanly.
        check_src(
            "use host::transmit(Word@!Secret) -> ()\n\
             fn main() -> Word { host::transmit(0); 0 }",
        )
        .expect("unlabelled arg admitted by negative-label param");
    }

    #[test]
    fn negative_label_braced_form_admits_unlabelled_arg() {
        check_src(
            "use host::transmit(Word@{!Secret}) -> ()\n\
             fn main() -> Word { host::transmit(0); 0 }",
        )
        .expect("unlabelled arg admitted by negative-label param");
    }

    #[test]
    fn negative_label_multi_braced_admits_unlabelled_arg() {
        check_src(
            "use host::transmit(Word@{!Secret, !Internal}) -> ()\n\
             fn main() -> Word { host::transmit(0); 0 }",
        )
        .expect("unlabelled arg admitted by multi-negative param");
    }

    #[test]
    fn negative_label_rejects_call_carrying_forbidden_label() {
        // The argument is explicitly classified with `Secret`;
        // the parameter forbids it.
        let err = check_src(
            "use host::transmit(Word@!Secret) -> ()\n\
             fn main() -> Word { host::transmit(classify 0@Secret); 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("Secret") && err.message.contains("forbids"),
            "expected negative-label rejection diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn negative_label_admits_unrelated_label() {
        // The parameter forbids Secret. An argument labelled
        // `Open` (a different label) flows in cleanly.
        check_src(
            "use host::transmit(Word@!Secret) -> ()\n\
             fn main() -> Word { host::transmit(classify 0@Open); 0 }",
        )
        .expect("unrelated label admitted by negative param");
    }

    #[test]
    fn mixed_positive_and_negative_in_set_is_parse_error() {
        let tokens = tokenize(
            "use host::transmit(Word@{Signed, !Secret}) -> ()\n\
             fn main() -> Word { host::transmit(0); 0 }",
        )
        .expect("lex");
        let err = crate::parser::parse(&tokens).expect_err("mixed set must parse-fail");
        assert!(
            err.message.contains("mixed")
                && err.message.contains("positive")
                && err.message.contains("negative"),
            "expected mixed-set diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn negative_label_at_let_binding_position_rejected() {
        // V0.2.0 admits negatives only at parameter and return
        // top-level positions. A `let` binding with a
        // negative-label annotation must be rejected.
        let err = check_src("fn main() -> Word { let x: Word@!Secret = 0; x }").unwrap_err();
        assert!(
            err.message.contains("negative information-flow labels"),
            "expected nested-negative rejection diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn negative_label_inside_tuple_at_param_position_rejected() {
        // A negative-label wrapper inside a tuple at a parameter
        // position is rejected because the wrapper is nested.
        let err = check_src(
            "use host::pair_in((Word@!Secret, Word)) -> ()\n\
             fn main() -> Word { host::pair_in((0, 0)); 0 }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("negative information-flow labels"),
            "expected nested-negative rejection diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn negative_label_on_return_type_rejects_yielded_secret() {
        // The function declares its return type as `!Secret`.
        // Yielding a Secret-labelled value must be rejected at
        // the yield site.
        let err = check_src(
            "loop main(input: Word) -> Word@!Secret {\n\
                let _ = yield classify 0@Secret;\n\
                0\n\
             }",
        )
        .unwrap_err();
        assert!(
            err.message.contains("yielded value") && err.message.contains("forbids"),
            "expected yield-side negative rejection, got: {}",
            err.message
        );
    }

    #[test]
    fn negative_label_on_return_type_rejects_returned_secret() {
        // The function declares `-> Word@!Secret` and the body
        // tail produces a Secret-labelled value.
        let err = check_src("fn main() -> Word@!Secret { classify 0@Secret }").unwrap_err();
        assert!(
            err.message.contains("body return value") && err.message.contains("forbids"),
            "expected return-side negative rejection, got: {}",
            err.message
        );
    }

    #[test]
    fn negative_label_in_classify_rejected_at_parse() {
        let tokens = tokenize("fn main() -> Word { let x = classify 0@!Secret; x }").expect("lex");
        let err = crate::parser::parse(&tokens).expect_err("classify+negative parse-rejected");
        assert!(
            err.message.contains("classify"),
            "expected classify-rejection diagnostic, got: {}",
            err.message
        );
    }

    /// Negative labels on `shared data` fields are admitted. An
    /// unlabelled write into a `!Secret` field type-checks.
    #[test]
    fn negative_label_on_shared_data_admits_unlabelled_write() {
        let src = "
            shared data state { forbidden: Word @ !Secret }
            fn main() -> Word { state.forbidden = 42; state.forbidden }
        ";
        let tokens = tokenize(src).expect("lex");
        let mut program = crate::parser::parse(&tokens).expect("parse");
        check(&mut program).expect("typecheck accepts unlabelled write");
    }

    /// Negative labels on `private data` fields are admitted. The
    /// yield-resume boundary is the negative-label clause's scope
    /// for private fields.
    #[test]
    fn negative_label_on_private_data_admits_unlabelled_write() {
        let src = "
            private data state { forbidden: Word @ !Secret }
            fn main() -> Word { state.forbidden = 42; state.forbidden }
        ";
        let tokens = tokenize(src).expect("lex");
        let mut program = crate::parser::parse(&tokens).expect("parse");
        check(&mut program).expect("typecheck accepts unlabelled write");
    }

    /// Assigning a labelled value into a `!Label` data field
    /// rejects with the boundary-violation diagnostic.
    #[test]
    fn negative_label_on_data_rejects_labelled_write() {
        let src = "
            shared data state { forbidden: Word @ !Secret }
            fn mk() -> Word @ Secret { classify 1 @ Secret }
            fn main() -> Word { state.forbidden = mk(); state.forbidden }
        ";
        let tokens = tokenize(src).expect("lex");
        let mut program = crate::parser::parse(&tokens).expect("parse");
        let err = check(&mut program).expect_err("typecheck rejects labelled write");
        assert!(
            err.message.contains("forbidden")
                && err.message.contains("Secret")
                && err.message.contains("`!`-prefix declaration forbids"),
            "expected boundary-violation diagnostic on data field, got: {}",
            err.message
        );
    }

    /// Negative labels at nested positions on data fields (inside
    /// a tuple, array, or option) are rejected. The boundary
    /// clause is a top-level-only construct.
    #[test]
    fn negative_label_nested_on_data_field_rejected() {
        let src = "
            shared data state { nested: (Word, Word @ !Secret) }
            fn main() -> Word { state.nested.0 }
        ";
        let tokens = tokenize(src).expect("lex");
        let mut program = crate::parser::parse(&tokens).expect("parse");
        let err = check(&mut program).expect_err("nested negative label on data field rejected");
        assert!(
            err.message.contains("admissible only at the top level"),
            "expected nested-position rejection, got: {}",
            err.message
        );
    }

    /// Reading a `!Label` data field produces a value of the
    /// inner type with no labels. The boundary clause does not
    /// propagate as a value-side label, so the read result can
    /// flow into any context that accepts the inner type
    /// regardless of label discipline.
    #[test]
    fn negative_label_on_data_read_is_inner_type() {
        let src = "
            shared data state { forbidden: Word @ !Secret }
            fn requires_secret(x: Word @ Secret) -> Word { declassify x @ Secret }
            fn main() -> Word {
                let raw: Word = state.forbidden;
                requires_secret(classify raw @ Secret)
            }
        ";
        let tokens = tokenize(src).expect("lex");
        let mut program = crate::parser::parse(&tokens).expect("parse");
        check(&mut program).expect("read produces inner type with no labels");
    }

    /// `const data` fields admit negative labels too. The
    /// initialiser literal carries no labels and satisfies the
    /// boundary.
    #[test]
    fn negative_label_on_const_data_admits_unlabelled_initialiser() {
        let src = "
            const data state { forbidden: Word @ !Secret = 7 }
            fn main() -> Word { state.forbidden }
        ";
        let tokens = tokenize(src).expect("lex");
        let mut program = crate::parser::parse(&tokens).expect("parse");
        check(&mut program).expect("const data with negative label compiles");
    }
}
