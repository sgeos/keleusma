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
//! The pass uses Robinson-style unification through the [`unify`]
//! function and the [`Subst`] type. Inferred positions allocate fresh
//! type variables through [`Ctx::fresh`]. Unannotated let bindings,
//! unannotated function parameters, and recursive expression types
//! receive [`Type::Var`] placeholders that are resolved through
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

/// A computed type. The internal representation is independent of the
/// `TypeExpr` AST node so the checker can reason about types without
/// surface-syntax detail.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// 64-bit signed integer.
    I64,
    /// 64-bit floating-point.
    F64,
    /// Boolean.
    Bool,
    /// Unit `()`.
    Unit,
    /// Static string.
    Str,
    /// Tuple of types.
    Tuple(Vec<Type>),
    /// Fixed-length array.
    Array(Box<Type>, i64),
    /// Option of a type.
    Option(Box<Type>),
    /// Named struct with optional generic type arguments. Empty
    /// `Vec<Type>` for non-generic structs.
    Struct(String, Vec<Type>),
    /// Named enum with optional generic type arguments. Empty
    /// `Vec<Type>` for non-generic enums.
    Enum(String, Vec<Type>),
    /// Opaque type referenced by name.
    Opaque(String),
    /// Type variable for Hindley-Milner inference. Allocated by the
    /// checker for expressions whose type is constrained but not yet
    /// solved. Resolved through unification against the constraint
    /// set; a final pass applies the substitution and reports any
    /// unresolved variable as an inference failure.
    Var(u32),
    /// Sentinel for an expression whose type cannot be determined
    /// without inference (e.g., unannotated let bound to a `match`
    /// expression returning a variable). Treated as compatible with
    /// anything in this MVP pass. Retained for backwards compatibility
    /// with the narrow ad-hoc inference; the HM pipeline produces
    /// `Type::Var` instead.
    Unknown,
}

impl Type {
    fn from_expr(expr: &TypeExpr, defined_types: &BTreeMap<String, TypeKind>) -> Type {
        Type::from_expr_with_params(expr, defined_types, &BTreeMap::new())
    }

    /// Resolve a [`TypeExpr`] under a generic type parameter mapping.
    ///
    /// Names that match a key in `type_params` resolve to the mapped
    /// [`Type`], typically a [`Type::Var`] allocated at signature
    /// construction. Names that are not type parameters fall back to
    /// the existing struct/enum/opaque resolution.
    fn from_expr_with_params(
        expr: &TypeExpr,
        defined_types: &BTreeMap<String, TypeKind>,
        type_params: &BTreeMap<String, Type>,
    ) -> Type {
        match expr {
            TypeExpr::Prim(p, _) => match p {
                PrimType::I64 => Type::I64,
                PrimType::F64 => Type::F64,
                PrimType::Bool => Type::Bool,
                PrimType::KString => Type::Str,
            },
            TypeExpr::Unit(_) => Type::Unit,
            TypeExpr::Tuple(ts, _) => Type::Tuple(
                ts.iter()
                    .map(|t| Type::from_expr_with_params(t, defined_types, type_params))
                    .collect(),
            ),
            TypeExpr::Array(elem, len, _) => Type::Array(
                Box::new(Type::from_expr_with_params(
                    elem,
                    defined_types,
                    type_params,
                )),
                *len,
            ),
            TypeExpr::Option(inner, _) => Type::Option(Box::new(Type::from_expr_with_params(
                inner,
                defined_types,
                type_params,
            ))),
            TypeExpr::Named(name, args, _) => {
                if let Some(t) = type_params.get(name) {
                    return t.clone();
                }
                let resolved_args: Vec<Type> = args
                    .iter()
                    .map(|a| Type::from_expr_with_params(a, defined_types, type_params))
                    .collect();
                match defined_types.get(name) {
                    Some(TypeKind::Struct) => Type::Struct(name.clone(), resolved_args),
                    Some(TypeKind::Enum) => Type::Enum(name.clone(), resolved_args),
                    None => Type::Opaque(name.clone()),
                }
            }
        }
    }

    /// Human-readable type name for diagnostics.
    pub fn display(&self) -> String {
        match self {
            Type::I64 => "i64".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Unit => "()".to_string(),
            Type::Str => "String".to_string(),
            Type::Tuple(ts) => {
                let inner: Vec<String> = ts.iter().map(|t| t.display()).collect();
                format!("({})", inner.join(", "))
            }
            Type::Array(elem, n) => format!("[{}; {}]", elem.display(), n),
            Type::Option(inner) => format!("Option<{}>", inner.display()),
            Type::Struct(name, args) | Type::Enum(name, args) => {
                if args.is_empty() {
                    name.clone()
                } else {
                    let inner: Vec<String> = args.iter().map(|t| t.display()).collect();
                    format!("{}<{}>", name, inner.join(", "))
                }
            }
            Type::Opaque(name) => name.clone(),
            Type::Var(n) => format!("?T{}", n),
            Type::Unknown => "<unknown>".to_string(),
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
            Type::Array(elem, n) => Type::Array(Box::new(elem.apply(subst)), *n),
            Type::Option(inner) => Type::Option(Box::new(inner.apply(subst))),
            Type::Struct(name, args) => {
                Type::Struct(name.clone(), args.iter().map(|t| t.apply(subst)).collect())
            }
            Type::Enum(name, args) => {
                Type::Enum(name.clone(), args.iter().map(|t| t.apply(subst)).collect())
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
    Mismatch { left: Type, right: Type },
    /// A type variable would refer to itself through a chain of
    /// constraints, producing an infinite type.
    OccursCheck { var: u32, ty: Type },
    /// Two arrays have different declared lengths.
    ArrayLengthMismatch { left: i64, right: i64 },
    /// Two tuples have different arity.
    TupleArityMismatch { left: usize, right: usize },
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
    let a = a.apply(subst);
    let b = b.apply(subst);
    match (a, b) {
        (Type::I64, Type::I64)
        | (Type::F64, Type::F64)
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
            if ln != rn {
                return Err(UnifyError::ArrayLengthMismatch {
                    left: ln,
                    right: rn,
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
}

/// Return the canonical head name of a type, used as the implementing
/// type's identity in the trait `impls` map. Primitive types have
/// stable lower-case names; named types use their declaration name.
/// Type variables and unresolved `Type::Unknown` are treated as
/// matching any implementation; the caller decides how to handle
/// these cases.
fn type_head_name(t: &Type) -> Option<String> {
    use alloc::string::ToString;
    match t {
        Type::I64 => Some("i64".to_string()),
        Type::F64 => Some("f64".to_string()),
        Type::Bool => Some("bool".to_string()),
        Type::Unit => Some("()".to_string()),
        Type::Str => Some("String".to_string()),
        Type::Tuple(_) => Some("tuple".to_string()),
        Type::Array(_, _) => Some("array".to_string()),
        Type::Option(_) => Some("Option".to_string()),
        Type::Struct(name, _) | Type::Enum(name, _) | Type::Opaque(name) => Some(name.clone()),
        Type::Var(_) | Type::Unknown => None,
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
}

/// A type-check error with source location.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
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
    /// representation: `i64` for `Type::I64`, `Pair` for
    /// `Type::Struct("Pair", _)`, and so on.
    impls: BTreeMap<String, BTreeSet<String>>,
    functions: BTreeMap<String, FnSig>,
    /// Native function names imported via `use` declarations. Calls
    /// to these names are accepted with any argument types because
    /// native signatures are not declared at compile time.
    natives: BTreeSet<String>,
    /// Data block field types, keyed by data name then field name.
    data: BTreeMap<String, BTreeMap<String, Type>>,
    /// Stack of local variable scopes. Inner scopes shadow outer.
    locals: Vec<BTreeMap<String, Type>>,
    /// Return type of the function currently being checked.
    current_return: Option<Type>,
    /// Fresh type variable allocator for the Hindley-Milner pipeline.
    vargen: VarGen,
    /// Active substitution accumulating constraints solved so far.
    subst: Subst,
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
            data: BTreeMap::new(),
            locals: Vec::new(),
            current_return: None,
            vargen: VarGen::default(),
            subst: Subst::new(),
        }
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
/// `Type::Unknown` is treated as compatible with anything for backwards
/// compatibility with the narrow ad-hoc inference; positions that are
/// unannotated should prefer fresh type variables through
/// [`Ctx::fresh`] over `Unknown` so that constraints can propagate.
fn types_compatible(ctx: &mut Ctx, a: &Type, b: &Type) -> bool {
    // The legacy `Type::Unknown` sentinel is compatible with anything
    // because the narrow inference produces it when no constraint can
    // be derived. `Type::Var` is NOT short-circuited here; it must go
    // through `unify` so the constraint is recorded in the
    // substitution. Otherwise distinct generic instantiations would
    // appear compatible regardless of their actual types.
    if matches!(a, Type::Unknown) || matches!(b, Type::Unknown) {
        return true;
    }
    unify(a, b, &mut ctx.subst).is_ok()
}

/// Top-level type check entry point.
///
/// Walks the program in two passes. The first pass collects type
/// definitions, struct and enum field signatures, data block field
/// types, and function signatures. The second pass checks each
/// function body against its declared signature.
pub fn check(program: &Program) -> Result<(), TypeError> {
    let mut ctx = Ctx::new();

    // Pass 1a. Collect type kinds (struct vs enum) so name resolution
    // works while reading field signatures.
    for type_def in &program.types {
        match type_def {
            TypeDef::Struct(s) => {
                ctx.types.insert(s.name.clone(), TypeKind::Struct);
            }
            TypeDef::Enum(e) => {
                ctx.types.insert(e.name.clone(), TypeKind::Enum);
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
                        Type::from_expr_with_params(&f.type_expr, &ctx.types, &tp_map),
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
                        .map(|t| Type::from_expr_with_params(t, &ctx.types, &tp_map))
                        .collect();
                    variants.insert(v.name.clone(), payload);
                }
                ctx.enums.insert(e.name.clone(), variants);
                ctx.enum_type_param_vars.insert(e.name.clone(), tp_vars);
            }
        }
    }

    for data in &program.data_decls {
        let mut fields = BTreeMap::new();
        for f in &data.fields {
            fields.insert(f.name.clone(), Type::from_expr(&f.type_expr, &ctx.types));
        }
        ctx.data.insert(data.name.clone(), fields);
    }

    // Pass 1c0. Collect native names from `use` declarations.
    // Names take the form `path::name` or just `name` for use without
    // path. Wildcard imports cannot be resolved at compile time and
    // are treated leniently elsewhere.
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
        let params: Vec<Type> = func
            .params
            .iter()
            .map(|p| match &p.type_expr {
                Some(t) => Type::from_expr_with_params(t, &ctx.types, &tp_map),
                None => ctx.fresh(),
            })
            .collect();
        let return_type = Type::from_expr_with_params(&func.return_type, &ctx.types, &tp_map);
        ctx.functions.insert(
            func.name.clone(),
            FnSig {
                type_params: tp_names,
                type_param_vars: tp_vars,
                type_param_bounds: tp_bounds,
                params,
                return_type,
            },
        );
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
        let head = match Type::from_expr(&impl_block.for_type, &ctx.types) {
            Type::I64 => "i64".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Unit => "()".to_string(),
            Type::Str => "String".to_string(),
            Type::Tuple(_) => "tuple".to_string(),
            Type::Array(_, _) => "array".to_string(),
            Type::Option(_) => "Option".to_string(),
            Type::Struct(name, _) | Type::Enum(name, _) | Type::Opaque(name) => name,
            Type::Var(_) | Type::Unknown => continue,
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
            for tp in &method.type_params {
                let v = ctx.fresh();
                tp_map.insert(tp.name.clone(), v.clone());
                tp_vars.push(v);
                tp_names.push(tp.name.clone());
                tp_bounds.push(tp.bounds.clone());
            }
            let params: Vec<Type> = method
                .params
                .iter()
                .map(|p| match &p.type_expr {
                    Some(t) => Type::from_expr_with_params(t, &ctx.types, &tp_map),
                    None => ctx.fresh(),
                })
                .collect();
            let return_type = Type::from_expr_with_params(&method.return_type, &ctx.types, &tp_map);
            ctx.functions.insert(
                mangled,
                FnSig {
                    type_params: tp_names,
                    type_param_vars: tp_vars,
                    type_param_bounds: tp_bounds,
                    params,
                    return_type,
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
                    Some(t) => Type::from_expr(t, &ctx.types),
                    None => continue,
                };
                let trait_ty = match &trait_param.type_expr {
                    Some(t) => Type::from_expr(t, &ctx.types),
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
            let impl_ret = Type::from_expr(&impl_method.return_type, &ctx.types);
            let trait_ret = Type::from_expr(&trait_sig.return_type, &ctx.types);
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

    // Pass 2. Check each function body.
    for func in &program.functions {
        check_function(&mut ctx, func)?;
    }
    // Also check impl method bodies. The bodies are checked under
    // their mangled names so the parameter and return type lookups
    // resolve through the same FnSig that was registered in pass 1d.
    for impl_block in &program.impls {
        let head = match Type::from_expr(&impl_block.for_type, &ctx.types) {
            Type::I64 => "i64".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Bool => "bool".to_string(),
            Type::Unit => "()".to_string(),
            Type::Str => "String".to_string(),
            Type::Tuple(_) => "tuple".to_string(),
            Type::Array(_, _) => "array".to_string(),
            Type::Option(_) => "Option".to_string(),
            Type::Struct(name, _) | Type::Enum(name, _) | Type::Opaque(name) => name,
            Type::Var(_) | Type::Unknown => continue,
        };
        for method in &impl_block.methods {
            let mut renamed = method.clone();
            renamed.name = format!("{}::{}::{}", impl_block.trait_name, head, method.name);
            check_function(&mut ctx, &renamed)?;
        }
    }

    Ok(())
}

fn check_function(ctx: &mut Ctx, func: &FunctionDef) -> Result<(), TypeError> {
    // Snapshot the substitution at function entry so the per-function
    // resolution does not pollute later functions with this function's
    // local type variables. The vargen counter continues monotonically
    // across functions because variable identifiers are unique even
    // after substitution snapshots.
    let subst_snapshot = ctx.subst.clone();
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
    // Check body. The block's tail expression must match the return
    // type when the return type is not Unit. For Unit-returning
    // functions, an absent tail is admissible.
    let body_type = type_of_block(ctx, &func.body)?;
    if !types_compatible(ctx, &body_type, &return_type) {
        ctx.pop_scope();
        ctx.current_return = None;
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
    ctx.pop_scope();
    ctx.current_return = None;
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
    // Roll back the substitution to the snapshot so type variables
    // local to this function do not leak into the next function's
    // checking. The Hindley-Milner discipline is per-function in
    // monomorphic Keleusma; cross-function generalization is the
    // domain of B2 generic parameters.
    ctx.subst = subst_snapshot;
    Ok(())
}

/// Bind a pattern's variables into the current scope at the given type.
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
                Literal::Int(_) => Type::I64,
                Literal::Float(_) => Type::F64,
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
            Type::Unknown | Type::Var(_) => Ok(()),
            _ => Err(TypeError::new(
                format!(
                    "tuple pattern does not match scrutinee type {}",
                    scrutinee_ty.display()
                ),
                *span,
            )),
        },
        Pattern::Enum(enum_name, variant, sub_pats, span) => {
            // Check enum name matches scrutinee.
            match scrutinee_ty {
                Type::Enum(scrutinee_name, _) if scrutinee_name == enum_name => {}
                Type::Unknown | Type::Var(_) => return Ok(()),
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
                Type::Unknown | Type::Var(_) => return Ok(()),
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
    let has_catchall = arms
        .iter()
        .any(|arm| matches!(arm.pattern, Pattern::Wildcard(_) | Pattern::Variable(_, _)));
    if has_catchall {
        return Ok(());
    }
    match scrutinee_ty {
        Type::Bool => {
            let mut has_true = false;
            let mut has_false = false;
            for arm in arms {
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
            let has_unit_lit = arms
                .iter()
                .any(|arm| matches!(arm.pattern, Pattern::Literal(Literal::Unit, _)));
            if has_unit_lit {
                Ok(())
            } else {
                Err(TypeError::new(
                    String::from("non-exhaustive match on (): requires `()` or wildcard arm"),
                    span,
                ))
            }
        }
        Type::Unknown | Type::Var(_) => Ok(()),
        other => Err(TypeError::new(
            format!(
                "non-exhaustive match on {}: requires a wildcard arm",
                other.display()
            ),
            span,
        )),
    }
}

fn type_of_block(ctx: &mut Ctx, block: &Block) -> Result<Type, TypeError> {
    ctx.push_scope();
    for stmt in &block.stmts {
        check_stmt(ctx, stmt)?;
    }
    let ty = match &block.tail_expr {
        Some(e) => type_of_expr(ctx, e)?,
        None => Type::Unit,
    };
    ctx.pop_scope();
    Ok(ty)
}

fn check_stmt(ctx: &mut Ctx, stmt: &Stmt) -> Result<(), TypeError> {
    match stmt {
        Stmt::Let(let_stmt) => {
            let value_ty = type_of_expr(ctx, &let_stmt.value)?;
            let bound_ty = match &let_stmt.type_expr {
                Some(t) => {
                    let declared = Type::from_expr(t, &ctx.types);
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
            let elem_ty = match &for_stmt.iterable {
                Iterable::Range(start, end) => {
                    let s = type_of_expr(ctx, start)?;
                    let e = type_of_expr(ctx, end)?;
                    if !types_compatible(ctx, &s, &Type::I64)
                        || !types_compatible(ctx, &e, &Type::I64)
                    {
                        return Err(TypeError::new(
                            format!(
                                "for-range bounds must be i64, got {} and {}",
                                s.display(),
                                e.display()
                            ),
                            for_stmt.span,
                        ));
                    }
                    Type::I64
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
            let _ = type_of_block(ctx, &for_stmt.body)?;
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
            Ok(())
        }
        Stmt::Expr(e) => {
            let _ = type_of_expr(ctx, e)?;
            Ok(())
        }
    }
}

fn type_of_expr(ctx: &mut Ctx, expr: &Expr) -> Result<Type, TypeError> {
    match expr {
        Expr::Literal { value, .. } => Ok(match value {
            Literal::Int(_) => Type::I64,
            Literal::Float(_) => Type::F64,
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
            let lt = type_of_expr(ctx, left)?;
            let rt = type_of_expr(ctx, right)?;
            match op {
                BinOp::Add => {
                    if matches!(lt, Type::I64) && matches!(rt, Type::I64) {
                        Ok(Type::I64)
                    } else if matches!(lt, Type::F64) && matches!(rt, Type::F64) {
                        Ok(Type::F64)
                    } else if matches!(lt, Type::Str) && matches!(rt, Type::Str) {
                        Ok(Type::Str)
                    } else if matches!(lt, Type::Unknown | Type::Var(_))
                        || matches!(rt, Type::Unknown | Type::Var(_))
                    {
                        Ok(ctx.fresh())
                    } else {
                        Err(TypeError::new(
                            format!("cannot add {} and {}", lt.display(), rt.display()),
                            *span,
                        ))
                    }
                }
                BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    if matches!(lt, Type::I64) && matches!(rt, Type::I64) {
                        Ok(Type::I64)
                    } else if matches!(lt, Type::F64) && matches!(rt, Type::F64) {
                        Ok(Type::F64)
                    } else if matches!(lt, Type::Unknown | Type::Var(_))
                        || matches!(rt, Type::Unknown | Type::Var(_))
                    {
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
                BinOp::And | BinOp::Or => {
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
            }
        }
        Expr::UnaryOp { op, operand, span } => {
            let ty = type_of_expr(ctx, operand)?;
            match op {
                UnaryOp::Neg => match ty {
                    Type::I64 | Type::F64 | Type::Unknown | Type::Var(_) => Ok(ty),
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
            }
        }
        Expr::Call { name, args, span } => {
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
            // Native functions are registered at runtime and have no
            // compile-time signature. Names declared in `use` or
            // qualified with `::` are treated as natives and accept
            // any argument types. Other unknown names are rejected
            // as undefined.
            let sig = match ctx.functions.get(name).cloned() {
                Some(s) => s,
                None => {
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
            for (arg, param_ty) in args.iter().zip(inst_params.iter()) {
                let arg_ty = type_of_expr(ctx, arg)?;
                if !types_compatible(ctx, &arg_ty, param_ty) {
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
                for (arg, param_ty) in args.iter().zip(sig.params.iter().skip(1)) {
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
        Expr::Yield { value, .. } => {
            let _ = type_of_expr(ctx, value)?;
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
            let cond_ty = type_of_expr(ctx, condition)?;
            if !types_compatible(ctx, &cond_ty, &Type::Bool) {
                return Err(TypeError::new(
                    format!("if condition must be bool, got {}", cond_ty.display()),
                    *span,
                ));
            }
            let then_ty = type_of_block(ctx, then_block)?;
            match else_block {
                Some(b) => {
                    let else_ty = type_of_block(ctx, b)?;
                    if !types_compatible(ctx, &then_ty, &else_ty) {
                        return Err(TypeError::new(
                            format!(
                                "if branches have differing types {} and {}",
                                then_ty.display(),
                                else_ty.display()
                            ),
                            *span,
                        ));
                    }
                    Ok(then_ty)
                }
                None => Ok(Type::Unit),
            }
        }
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => {
            let scrutinee_ty = type_of_expr(ctx, scrutinee)?;
            // Type the body of each arm. The arm bodies must agree.
            let mut common: Option<Type> = None;
            for arm in arms {
                check_pattern_against_type(ctx, &arm.pattern, &scrutinee_ty)?;
                ctx.push_scope();
                bind_pattern(ctx, &arm.pattern, scrutinee_ty.clone());
                let arm_ty = type_of_expr(ctx, &arm.expr)?;
                ctx.pop_scope();
                match &common {
                    None => common = Some(arm_ty),
                    Some(c) => {
                        if !types_compatible(ctx, c, &arm_ty) {
                            return Err(TypeError::new(
                                format!(
                                    "match arms have differing types {} and {}",
                                    c.display(),
                                    arm_ty.display()
                                ),
                                arm.span,
                            ));
                        }
                    }
                }
            }
            check_exhaustiveness(ctx, arms, &scrutinee_ty, *span)?;
            Ok(common.unwrap_or(Type::Unit))
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
                Type::Unknown | Type::Var(_) => Ok(ctx.fresh()),
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
                Type::Unknown => Ok(ctx.fresh()),
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
            if !types_compatible(ctx, &idx_ty, &Type::I64) {
                return Err(TypeError::new(
                    format!("array index must be i64, got {}", idx_ty.display()),
                    *span,
                ));
            }
            match obj_ty {
                Type::Array(inner, _) => Ok(*inner),
                Type::Unknown => Ok(ctx.fresh()),
                other => Err(TypeError::new(
                    format!("array index on non-array type {}", other.display()),
                    *span,
                )),
            }
        }
        Expr::StructInit { name, fields, span } => {
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
            for init in fields {
                let declared = declared_fields.get(&init.name).ok_or_else(|| {
                    TypeError::new(
                        format!("struct `{}` has no field `{}`", name, init.name),
                        init.span,
                    )
                })?;
                let declared_inst = declared.apply(&inst);
                let value_ty = type_of_expr(ctx, &init.value)?;
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
            span,
        } => {
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
                    for (arg, expected) in args.iter().zip(types.iter()) {
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
                        // Option::Some(t) and Option::None handled here.
                        for arg in args {
                            let _ = type_of_expr(ctx, arg)?;
                        }
                        return Ok(Type::Option(Box::new(Type::Unknown)));
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
            for e in elements {
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
                elements.len() as i64,
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
            let from_ty = type_of_expr(ctx, expr)?;
            let to_ty = Type::from_expr(target, &ctx.types);
            match (&from_ty, &to_ty) {
                (Type::I64, Type::F64) | (Type::F64, Type::I64) => Ok(to_ty),
                (Type::Unknown, _) | (_, Type::Unknown) => Ok(to_ty),
                (a, b) if a == b => Ok(to_ty),
                _ => Err(TypeError::new(
                    format!("cannot cast {} to {}", from_ty.display(), to_ty.display()),
                    *span,
                )),
            }
        }
        Expr::Placeholder { .. } => Ok(ctx.fresh()),
        Expr::ClosureRef { .. } => Ok(ctx.fresh()),
        Expr::Closure { params, body, .. } => {
            // Type-check the closure body in a fresh scope where the
            // parameters are bound to fresh type variables (or their
            // declared types). The closure's surface type is left as
            // a fresh type variable for now; first-class function
            // types are tracked under future B3 follow-on work.
            ctx.push_scope();
            for param in params {
                let t = match &param.type_expr {
                    Some(t) => Type::from_expr(t, &ctx.types),
                    None => ctx.fresh(),
                };
                bind_pattern(ctx, &param.pattern, t);
            }
            let _body_ty = type_of_block(ctx, body)?;
            ctx.pop_scope();
            Ok(ctx.fresh())
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
            // For unresolved receiver types (Type::Var or
            // Type::Unknown), the resolution is deferred. The current
            // session emits a fresh return type and skips bound
            // checking; B2.4 monomorphization will resolve the call
            // by substituting the concrete instantiation.
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
            for (arg, param_ty) in args.iter().zip(inst_params.iter().skip(1)) {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check_src(src: &str) -> Result<(), TypeError> {
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        check(&program)
    }

    #[test]
    fn simple_function_type_checks() {
        check_src("fn main() -> i64 { 1 + 2 }").unwrap();
    }

    #[test]
    fn return_type_mismatch_rejected() {
        let err = check_src("fn main() -> i64 { true }").unwrap_err();
        assert!(err.message.contains("returns i64"));
    }

    #[test]
    fn arithmetic_type_mismatch_rejected() {
        let err = check_src("fn main() -> i64 { 1 + 2.0 }").unwrap_err();
        assert!(err.message.contains("cannot add"));
    }

    #[test]
    fn function_call_arg_count_checked() {
        let err = check_src("fn add(a: i64, b: i64) -> i64 { a + b }\nfn main() -> i64 { add(1) }")
            .unwrap_err();
        assert!(err.message.contains("expects 2"));
    }

    #[test]
    fn function_call_arg_type_checked() {
        let err =
            check_src("fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(true) }")
                .unwrap_err();
        assert!(err.message.contains("expects i64"));
    }

    #[test]
    fn let_binding_type_mismatch_rejected() {
        let err = check_src("fn main() -> i64 { let x: i64 = true; 0 }").unwrap_err();
        assert!(err.message.contains("declared as i64"));
    }

    #[test]
    fn let_binding_inferred_from_value() {
        check_src("fn main() -> i64 { let x = 1; x + 1 }").unwrap();
    }

    #[test]
    fn if_branch_mismatch_rejected() {
        let err = check_src("fn main() -> i64 { if true { 1 } else { false } }").unwrap_err();
        assert!(err.message.contains("if branches"));
    }

    #[test]
    fn struct_field_access_checks() {
        check_src(
            "struct P { x: i64, y: i64 }\nfn main() -> i64 { let p = P { x: 1, y: 2 }; p.x }",
        )
        .unwrap();
    }

    #[test]
    fn struct_unknown_field_rejected() {
        let err = check_src("struct P { x: i64 }\nfn main() -> i64 { let p = P { x: 1 }; p.y }")
            .unwrap_err();
        assert!(err.message.contains("no field"));
    }

    #[test]
    fn cast_int_to_float_admissible() {
        check_src("fn main() -> f64 { let x: i64 = 1; x as f64 }").unwrap();
    }

    #[test]
    fn cast_bool_to_int_rejected() {
        let err = check_src("fn main() -> i64 { true as i64 }").unwrap_err();
        assert!(err.message.contains("cannot cast"));
    }

    #[test]
    fn undefined_identifier_rejected() {
        let err = check_src("fn main() -> i64 { x }").unwrap_err();
        assert!(err.message.contains("undefined"));
    }

    // -- #13 Native function call types --

    #[test]
    fn undefined_function_rejected() {
        let err = check_src("fn main() -> i64 { foo() }").unwrap_err();
        assert!(err.message.contains("undefined function `foo`"));
    }

    #[test]
    fn used_native_accepted() {
        check_src("use math::sqrt\nfn main() -> f64 { math::sqrt(9.0) }").unwrap();
    }

    #[test]
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
             fn main() -> i64 { match Color::Red() { Color::Blue() => 1, _ => 0 } }",
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
            "enum Shape { Square(i64), Circle(i64) }\n\
             fn main() -> i64 { match Shape::Square(1) { Shape::Square(a, b) => 0, _ => 1 } }",
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
            check_src("fn main() -> i64 { match (1, 2) { (a, b, c) => 0, _ => 1 } }").unwrap_err();
        assert!(err.message.contains("tuple pattern"));
    }

    #[test]
    fn tuple_pattern_against_non_tuple_rejected() {
        let err = check_src("fn main() -> i64 { match 5 { (a, b) => 0, _ => 1 } }").unwrap_err();
        assert!(err.message.contains("tuple pattern"));
    }

    #[test]
    fn literal_pattern_type_mismatch_rejected() {
        let err = check_src("fn main() -> i64 { match 5 { true => 1, _ => 0 } }").unwrap_err();
        assert!(err.message.contains("literal pattern"));
    }

    // -- #11 Match arm exhaustiveness --

    #[test]
    fn enum_match_missing_variant_rejected() {
        let err = check_src(
            "enum Color { Red, Green, Blue }\n\
             fn main() -> i64 { match Color::Red() { Color::Red() => 0, Color::Green() => 1 } }",
        )
        .unwrap_err();
        assert!(err.message.contains("non-exhaustive match"));
        assert!(err.message.contains("Blue"));
    }

    #[test]
    fn enum_match_with_wildcard_accepted() {
        check_src(
            "enum Color { Red, Green, Blue }\n\
             fn main() -> i64 { match Color::Red() { Color::Red() => 0, _ => 1 } }",
        )
        .unwrap();
    }

    #[test]
    fn enum_match_with_all_variants_accepted() {
        check_src(
            "enum Color { Red, Green }\n\
             fn main() -> i64 { match Color::Red() { Color::Red() => 0, Color::Green() => 1 } }",
        )
        .unwrap();
    }

    #[test]
    fn bool_match_missing_arm_rejected() {
        let err = check_src("fn main() -> i64 { match true { true => 1 } }").unwrap_err();
        assert!(err.message.contains("non-exhaustive match"));
    }

    #[test]
    fn bool_match_complete_accepted() {
        check_src("fn main() -> i64 { match true { true => 1, false => 0 } }").unwrap();
    }

    #[test]
    fn i64_match_without_wildcard_rejected() {
        let err = check_src("fn main() -> i64 { match 1 { 1 => 1, 2 => 2 } }").unwrap_err();
        assert!(err.message.contains("non-exhaustive match"));
    }

    #[test]
    fn i64_match_with_wildcard_accepted() {
        check_src("fn main() -> i64 { match 1 { 1 => 1, _ => 0 } }").unwrap();
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
        unify(&Type::I64, &Type::I64, &mut s).unwrap();
        unify(&Type::Bool, &Type::Bool, &mut s).unwrap();
        unify(&Type::Unit, &Type::Unit, &mut s).unwrap();
        unify(&Type::Str, &Type::Str, &mut s).unwrap();
        assert!(s.is_empty());
    }

    #[test]
    fn unify_distinct_primitives_fails() {
        let mut s = Subst::new();
        let err = unify(&Type::I64, &Type::F64, &mut s).unwrap_err();
        match err {
            UnifyError::Mismatch { left, right } => {
                assert_eq!(left, Type::I64);
                assert_eq!(right, Type::F64);
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn unify_var_with_concrete_binds() {
        let mut s = Subst::new();
        unify(&Type::Var(0), &Type::I64, &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::I64));
    }

    #[test]
    fn unify_concrete_with_var_binds() {
        let mut s = Subst::new();
        unify(&Type::I64, &Type::Var(0), &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::I64));
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
        let t2 = Type::Tuple(alloc::vec![Type::I64, Type::Var(1)]);
        unify(&t1, &t2, &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::I64));
        assert_eq!(s.get(1), Some(&Type::Bool));
    }

    #[test]
    fn unify_tuple_arity_mismatch() {
        let mut s = Subst::new();
        let t1 = Type::Tuple(alloc::vec![Type::I64, Type::Bool]);
        let t2 = Type::Tuple(alloc::vec![Type::I64]);
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
        let t1 = Type::Array(Box::new(Type::I64), 3);
        let t2 = Type::Array(Box::new(Type::I64), 4);
        let err = unify(&t1, &t2, &mut s).unwrap_err();
        match err {
            UnifyError::ArrayLengthMismatch { left, right } => {
                assert_eq!(left, 3);
                assert_eq!(right, 4);
            }
            other => panic!("expected ArrayLengthMismatch, got {:?}", other),
        }
    }

    #[test]
    fn unify_array_element_types_unify() {
        let mut s = Subst::new();
        let t1 = Type::Array(Box::new(Type::Var(0)), 3);
        let t2 = Type::Array(Box::new(Type::I64), 3);
        unify(&t1, &t2, &mut s).unwrap();
        assert_eq!(s.get(0), Some(&Type::I64));
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
        let t2 = Type::Tuple(alloc::vec![Type::Var(0), Type::I64]);
        let err = unify(&t1, &t2, &mut s).unwrap_err();
        assert!(matches!(err, UnifyError::OccursCheck { .. }));
    }

    #[test]
    fn apply_substitution_resolves_variable() {
        let mut s = Subst::new();
        s.insert(0, Type::I64);
        let t = Type::Tuple(alloc::vec![Type::Var(0), Type::Bool]);
        let resolved = t.apply(&s);
        assert_eq!(resolved, Type::Tuple(alloc::vec![Type::I64, Type::Bool]));
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
        unify(&Type::Var(0), &Type::I64, &mut s).unwrap();
        unify(&Type::Var(0), &Type::Var(1), &mut s).unwrap();
        let resolved = Type::Var(1).apply(&s);
        assert_eq!(resolved, Type::I64);
    }

    // -- B2 generic function checks --

    #[test]
    fn generic_identity_function_typechecks() {
        check_src("fn id<T>(x: T) -> T { x }\nfn main() -> i64 { id(42) }").unwrap();
    }

    #[test]
    fn generic_function_called_with_two_types_separately() {
        // Two distinct call sites instantiate the type parameter
        // separately, so the same generic function flows through
        // both i64 and bool.
        check_src(
            "fn id<T>(x: T) -> T { x }\n\
             fn main() -> i64 {\n\
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
             fn main() -> i64 { first(1, true) }",
        )
        .unwrap();
    }

    #[test]
    fn generic_function_arity_mismatch_rejected() {
        let err =
            check_src("fn id<T>(x: T) -> T { x }\nfn main() -> i64 { id(1, 2) }").unwrap_err();
        assert!(err.message.contains("expects 1 arguments"));
    }

    // -- B2.2 generic struct and enum checks --

    #[test]
    fn generic_struct_with_one_param_typechecks() {
        check_src(
            "struct Cell<T> { value: T }\n\
             fn main() -> i64 {\n\
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
             fn main() -> i64 {\n\
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
             fn main() -> i64 {\n\
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
             fn main() -> i64 {\n\
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
             fn main() -> i64 {\n\
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
             fn main() -> i64 {\n\
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
            "trait Numeric { fn one() -> i64; }\n\
             impl Numeric for i64 { fn one() -> i64 { 1 } }\n\
             fn use_it<T: Numeric>(x: T) -> T { x }\n\
             fn main() -> i64 { use_it(7) }",
        )
        .unwrap();
    }

    #[test]
    fn trait_bound_satisfied_by_impl() {
        // When the bound's required impl exists for the call's
        // argument type, the call type-checks.
        check_src(
            "trait Tag { fn tag() -> i64; }\n\
             impl Tag for bool { fn tag() -> i64 { 1 } }\n\
             fn use_tag<T: Tag>(x: T) -> i64 { 0 }\n\
             fn main() -> i64 { use_tag(true) }",
        )
        .unwrap();
    }

    #[test]
    fn trait_bound_unsatisfied_rejects_call() {
        // With an impl for `bool` only, calling with an `i64`
        // argument should fail bound validation because no `Tag`
        // impl exists for `i64`.
        let err = check_src(
            "trait Tag { fn tag() -> i64; }\n\
             impl Tag for bool { fn tag() -> i64 { 1 } }\n\
             fn use_tag<T: Tag>(x: T) -> i64 { 0 }\n\
             fn main() -> i64 { use_tag(7) }",
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
             fn main() -> i64 { id(42) }",
        )
        .unwrap();
    }

    #[test]
    fn multiple_trait_bounds_on_one_param() {
        check_src(
            "trait A { fn a() -> i64; }\n\
             trait B { fn b() -> i64; }\n\
             impl A for i64 { fn a() -> i64 { 1 } }\n\
             impl B for i64 { fn b() -> i64 { 2 } }\n\
             fn use_both<T: A + B>(x: T) -> i64 { 0 }\n\
             fn main() -> i64 { use_both(7) }",
        )
        .unwrap();
    }

    #[test]
    fn impl_method_with_extra_method_rejected() {
        // The trait does not declare `extra`, so the impl is invalid.
        let err = check_src(
            "trait T { fn one() -> i64; }\n\
             impl T for i64 {\n\
                fn one() -> i64 { 1 }\n\
                fn extra() -> i64 { 2 }\n\
             }\n\
             fn main() -> i64 { 0 }",
        )
        .unwrap_err();
        assert!(err.message.contains("not in the trait"));
    }

    #[test]
    fn impl_method_arity_mismatch_rejected() {
        // The trait declares `fn one() -> i64` (arity zero); the impl
        // supplies `fn one(x: i64) -> i64` (arity one). Arity mismatch.
        let err = check_src(
            "trait T { fn one() -> i64; }\n\
             impl T for i64 { fn one(x: i64) -> i64 { x } }\n\
             fn main() -> i64 { 0 }",
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
            "trait Doubler { fn double(x: i64) -> i64; }\n\
             impl Doubler for i64 { fn double(x: i64) -> i64 { x + x } }\n\
             fn use_doubler<T: Doubler>(x: T) -> i64 { x.double() }\n\
             fn main() -> i64 { use_doubler(21) }",
        )
        .unwrap();
    }

    #[test]
    fn method_call_resolves_to_impl() {
        check_src(
            "trait Doubler { fn double(x: i64) -> i64; }\n\
             impl Doubler for i64 { fn double(x: i64) -> i64 { x + x } }\n\
             fn main() -> i64 {\n\
                let n: i64 = 21;\n\
                n.double()\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn method_call_unknown_method_rejected() {
        let err = check_src(
            "trait Doubler { fn double(x: i64) -> i64; }\n\
             impl Doubler for i64 { fn double(x: i64) -> i64 { x + x } }\n\
             fn main() -> i64 {\n\
                let n: i64 = 21;\n\
                n.triple()\n\
             }",
        )
        .unwrap_err();
        assert!(err.message.contains("no method"));
    }

    #[test]
    fn closure_executes_end_to_end() {
        // The compile pipeline includes hoisting; the type checker
        // accepts the closure expression and the indirect call site.
        check_src(
            "fn main() -> i64 {\n\
                let f = |x: i64| x + 1;\n\
                f(41)\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn monomorphize_inference_through_function_call() {
        // The generic call site uses a function call as argument.
        // Inference reach now resolves the call's return type and
        // specializes the generic function for the resulting type.
        check_src(
            "fn make42() -> i64 { 42 }\n\
             fn id<T>(x: T) -> T { x }\n\
             fn main() -> i64 { id(make42()) }",
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
             fn main() -> i64 {\n\
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
            "trait Doubler { fn double(x: i64) -> i64; }\n\
             impl Doubler for i64 { fn double(x: i64) -> i64 { x + x } }\n\
             struct Cell<T> { value: T }\n\
             fn main() -> i64 {\n\
                let c = Cell { value: 21 };\n\
                c.value.double()\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn closure_passed_as_argument() {
        // A generic function takes a closure as an argument and
        // invokes it. The body uses the parameter as a callable
        // through indirect dispatch.
        check_src(
            "fn apply<F>(f: F, x: i64) -> i64 { f(x) }\n\
             fn main() -> i64 {\n\
                let g = |x: i64| x + 1;\n\
                apply(g, 41)\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn closure_captures_outer_local() {
        check_src(
            "fn main() -> i64 {\n\
                let n: i64 = 10;\n\
                let f = |x: i64| x + n;\n\
                f(5)\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn closure_no_param_callable() {
        check_src(
            "fn main() -> i64 {\n\
                let f = || 42;\n\
                f()\n\
             }",
        )
        .unwrap();
    }

    #[test]
    fn impl_method_param_type_mismatch_rejected() {
        // Trait declares `fn double(x: i64) -> i64` but the impl
        // supplies `fn double(x: bool) -> i64`. Parameter type
        // mismatch must be rejected.
        let err = check_src(
            "trait Doubler { fn double(x: i64) -> i64; }\n\
             impl Doubler for i64 { fn double(x: bool) -> i64 { 0 } }\n\
             fn main() -> i64 { 0 }",
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
            "trait Doubler { fn double(x: i64) -> i64; }\n\
             impl Doubler for i64 { fn double(x: i64) -> bool { true } }\n\
             fn main() -> i64 { 0 }",
        )
        .unwrap_err();
        assert!(err.message.contains("returns"));
    }

    #[test]
    fn impl_for_unknown_trait_rejected() {
        let err = check_src(
            "impl Nonexistent for i64 { fn x() -> i64 { 0 } }\n\
             fn main() -> i64 { 0 }",
        )
        .unwrap_err();
        assert!(err.message.contains("unknown trait"));
    }

    #[test]
    fn missing_one_of_multiple_bounds_rejected() {
        // i64 implements A but not B, so a call requiring T: A + B
        // with i64 must fail.
        let err = check_src(
            "trait A { fn a() -> i64; }\n\
             trait B { fn b() -> i64; }\n\
             impl A for i64 { fn a() -> i64 { 1 } }\n\
             fn use_both<T: A + B>(x: T) -> i64 { 0 }\n\
             fn main() -> i64 { use_both(7) }",
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
             fn main() -> i64 {\n\
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
}
