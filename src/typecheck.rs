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
    /// Named struct.
    Struct(String),
    /// Named enum.
    Enum(String),
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
            TypeExpr::Named(name, _) => {
                if let Some(t) = type_params.get(name) {
                    return t.clone();
                }
                match defined_types.get(name) {
                    Some(TypeKind::Struct) => Type::Struct(name.clone()),
                    Some(TypeKind::Enum) => Type::Enum(name.clone()),
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
            Type::Struct(name) | Type::Enum(name) | Type::Opaque(name) => name.clone(),
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
        (Type::Struct(ln), Type::Struct(rn))
        | (Type::Enum(ln), Type::Enum(rn))
        | (Type::Opaque(ln), Type::Opaque(rn))
            if ln == rn =>
        {
            Ok(())
        }
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

/// Function signature derived from an AST function definition.
///
/// Generic functions record their type parameters and the
/// `Type::Var` identifiers assigned to each one at signature
/// construction time. Call-site instantiation generates a fresh
/// substitution from the recorded variables to fresh per-call
/// variables and applies it to the parameter and return types
/// before unifying against actual arguments.
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
    enums: BTreeMap<String, BTreeMap<String, Vec<Type>>>,
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
            enums: BTreeMap::new(),
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

/// Instantiate a generic function signature with fresh per-call type
/// variables.
///
/// For each abstract type parameter variable in the signature,
/// allocate a fresh `Type::Var` and build a substitution mapping the
/// abstract variable to the fresh one. Apply this substitution to the
/// parameter and return types before unification with the call's
/// actual argument types. The result is a pair of instantiated
/// parameter types and the instantiated return type.
fn instantiate_sig(ctx: &mut Ctx, sig: &FnSig) -> (Vec<Type>, Type) {
    if sig.type_params.is_empty() {
        return (sig.params.clone(), sig.return_type.clone());
    }
    let mut inst = Subst::new();
    for var in &sig.type_param_vars {
        if let Type::Var(v) = var {
            let fresh = ctx.vargen.fresh();
            inst.insert(*v, fresh);
        }
    }
    let params: Vec<Type> = sig.params.iter().map(|t| t.apply(&inst)).collect();
    let return_type = sig.return_type.apply(&inst);
    (params, return_type)
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
    if matches!(a, Type::Unknown | Type::Var(_)) || matches!(b, Type::Unknown | Type::Var(_)) {
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
    // block field types. The `from_expr` resolver consults `ctx.types`.
    for type_def in &program.types {
        match type_def {
            TypeDef::Struct(s) => {
                let mut fields = BTreeMap::new();
                for f in &s.fields {
                    fields.insert(f.name.clone(), Type::from_expr(&f.type_expr, &ctx.types));
                }
                ctx.structs.insert(s.name.clone(), fields);
            }
            TypeDef::Enum(e) => {
                let mut variants = BTreeMap::new();
                for v in &e.variants {
                    let payload: Vec<Type> = v
                        .fields
                        .iter()
                        .map(|t| Type::from_expr(t, &ctx.types))
                        .collect();
                    variants.insert(v.name.clone(), payload);
                }
                ctx.enums.insert(e.name.clone(), variants);
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
        for tp in &func.type_params {
            let v = ctx.fresh();
            tp_map.insert(tp.name.clone(), v.clone());
            tp_vars.push(v);
            tp_names.push(tp.name.clone());
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
                params,
                return_type,
            },
        );
    }

    // Pass 2. Check each function body.
    for func in &program.functions {
        check_function(&mut ctx, func)?;
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
                Type::Enum(scrutinee_name) if scrutinee_name == enum_name => {}
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
                Type::Struct(scrutinee_name) if scrutinee_name == name => {}
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
        Type::Enum(enum_name) => {
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
                return Ok(Type::Struct(name.clone()));
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
            let (inst_params, inst_return) = instantiate_sig(ctx, &sig);
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
                Type::Struct(ref name) => {
                    if let Some(fields) = ctx.structs.get(name)
                        && let Some(t) = fields.get(field)
                    {
                        return Ok(t.clone());
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
            for init in fields {
                let declared = declared_fields.get(&init.name).ok_or_else(|| {
                    TypeError::new(
                        format!("struct `{}` has no field `{}`", name, init.name),
                        init.span,
                    )
                })?;
                let value_ty = type_of_expr(ctx, &init.value)?;
                if !types_compatible(ctx, &value_ty, declared) {
                    return Err(TypeError::new(
                        format!(
                            "field `{}.{}` expects {}, got {}",
                            name,
                            init.name,
                            declared.display(),
                            value_ty.display()
                        ),
                        init.span,
                    ));
                }
            }
            Ok(Type::Struct(name.clone()))
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
                    for (arg, expected) in args.iter().zip(types.iter()) {
                        let arg_ty = type_of_expr(ctx, arg)?;
                        if !types_compatible(ctx, &arg_ty, expected) {
                            return Err(TypeError::new(
                                format!(
                                    "enum payload expects {}, got {}",
                                    expected.display(),
                                    arg_ty.display()
                                ),
                                arg.span(),
                            ));
                        }
                    }
                    Ok(Type::Enum(enum_name.clone()))
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
            &Type::Struct("Point".to_string()),
            &Type::Struct("Point".to_string()),
            &mut s,
        )
        .unwrap();
    }

    #[test]
    fn unify_named_struct_different_name_fails() {
        let mut s = Subst::new();
        let err = unify(
            &Type::Struct("Point".to_string()),
            &Type::Struct("Square".to_string()),
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
}
