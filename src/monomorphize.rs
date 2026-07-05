//! Compile-time monomorphization for generic functions.
//!
//! After type checking and before compilation, this pass walks the
//! program's call graph and generates a specialized
//! [`FunctionDef`](crate::ast::FunctionDef) per `(function, type_args)` pair encountered.
//! Each specialization clones the generic function's body and
//! substitutes the abstract type-parameter names with the concrete
//! types throughout. Call sites are rewritten to reference the
//! specialization by its mangled name.
//!
//! The benefit. Generic function bodies that contain method calls on
//! type parameters cannot resolve the method at compile time because
//! the receiver's type is abstract. Monomorphization replaces the
//! type parameter with a concrete type, so the receiver's type is
//! known and the method call resolves to the impl's mangled function.
//!
//! Scope. This MVP handles direct `Expr::Call` to generic functions
//! whose argument types can be inferred from literal arguments,
//! identifiers with declared types, or nested calls whose return
//! type is concrete. It does not yet handle generic structs or
//! enums, polymorphic recursion, or type arguments inferred only
//! through unification of complex constraints. These cases continue
//! to use runtime tag dispatch and produce a compile error if a
//! method call inside their body cannot resolve. Future iterations
//! extend the inference reach.

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::ast::*;

/// Apply monomorphization to a program. Returns a new program with
/// specialized functions added and call sites rewritten.
pub fn monomorphize(program: Program) -> Program {
    monomorphize_with_provenance(program).0
}

/// As [`monomorphize`], additionally returning a provenance map from
/// each specialized function's mangled name to its `(origin, type_args)`
/// pair. The compiler consumes this for B29 `GenericInstantiation`
/// debug records; non-debug builds discard it through the
/// [`monomorphize`] wrapper.
pub fn monomorphize_with_provenance(
    program: Program,
) -> (Program, BTreeMap<String, (String, String)>) {
    let mut program = program;

    // Build a map from function name to FunctionDef for lookup.
    // Generic functions remain in this map; specialization clones
    // them.
    let generics: BTreeMap<String, FunctionDef> = program
        .functions
        .iter()
        .filter(|f| !f.type_params.is_empty() || !f.const_params.is_empty())
        .map(|f| (f.name.clone(), f.clone()))
        .collect();

    // Function-return-type map for argument-type inference. Used by
    // `infer_arg_type` to resolve types of nested function calls
    // and method calls appearing in generic call arguments.
    //
    // Top-level functions are keyed on their bare name. Impl method
    // returns are also folded into this map under a `<head>::<method>`
    // mangled key so the `MethodCall` arm of `infer_arg_type` can
    // resolve a method call's return type from its receiver's head
    // and method name without threading a separate impl-method map
    // through the rewrite chain. The mangling form differs from the
    // compiler's `Trait::<head>::<method>` chunk-folding mangling so
    // the two namespaces remain disjoint.
    let mut fn_returns: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for f in &program.functions {
        fn_returns.insert(f.name.clone(), f.return_type.clone());
    }
    for impl_block in &program.impls {
        let head = type_head_for_impl(&impl_block.for_type);
        for method in &impl_block.methods {
            fn_returns.insert(
                alloc::format!("{}::{}", head, method.name),
                method.return_type.clone(),
            );
        }
    }

    // Struct-definition map for field-access type inference. Used by
    // `infer_arg_type` to resolve `o.field` against the struct's
    // declared field types when `o`'s nominal type is known.
    let struct_table: BTreeMap<String, StructDef> = program
        .types
        .iter()
        .filter_map(|td| match td {
            TypeDef::Struct(s) => Some((s.name.clone(), s.clone())),
            _ => None,
        })
        .collect();

    // Local-type information for argument-type inference.
    let mut local_types: BTreeMap<String, TypeExpr> = BTreeMap::new();
    // Specializations generated. Keyed on (function, type_args
    // canonical encoding). Value is the mangled specialized name.
    let mut specs: BTreeMap<(String, String), String> = BTreeMap::new();
    // New specialized functions to add to the program.
    let mut new_functions: Vec<FunctionDef> = Vec::new();

    {
        use crate::visitor::MutVisitor;
        for func in &mut program.functions {
            if func.type_params.is_empty() && func.const_params.is_empty() {
                local_types.clear();
                for param in &func.params {
                    if let Some(t) = &param.type_expr
                        && let Pattern::Variable(name, _) = &param.pattern
                    {
                        local_types.insert(name.clone(), t.clone());
                    }
                }
                let mut visitor = CallSpecializer {
                    generics: &generics,
                    locals: &mut local_types,
                    specs: &mut specs,
                    new_functions: &mut new_functions,
                    fn_returns: &fn_returns,
                    struct_table: &struct_table,
                };
                visitor.visit_block(&mut func.body);
            }
        }
    }

    // Polymorphic recursion guards. The fixed-point loop below
    // bounds two ways. The global SPECIALIZATION_LIMIT bounds the
    // total number of specializations. The per-function limit
    // detects cycles where a single generic function generates an
    // unbounded family of specializations through self-recursion
    // with growing type arguments. Both bounds are conservative;
    // legitimate programs reach a fixed point well below them.
    const SPECIALIZATION_LIMIT: usize = 1024;
    const PER_FUNCTION_LIMIT: usize = 64;
    let mut per_fn_counts: BTreeMap<String, usize> = BTreeMap::new();
    for (origin, _) in specs.keys() {
        *per_fn_counts.entry(origin.clone()).or_insert(0) += 1;
    }

    // Also rewrite calls inside specialized functions. Specialization
    // can introduce new calls that themselves point at generic
    // functions, so iterate to a fixed point.
    let mut idx = 0;
    while idx < new_functions.len() {
        if new_functions.len() > SPECIALIZATION_LIMIT {
            // Bail out: the program likely contains polymorphic
            // recursion that would grow specializations
            // unboundedly. Subsequent compilation will fail when
            // the bytecode chunk count exceeds VM limits, which is
            // the documented behavior. Returning the program
            // with the partial specializations gives the user a
            // clearer error path than infinite loop.
            break;
        }
        // Per-function cycle detection. If any single generic
        // function has produced more than PER_FUNCTION_LIMIT
        // specializations, the call graph is consuming the budget
        // through polymorphic recursion. Abort the loop early.
        let mut max_count = 0;
        for &count in per_fn_counts.values() {
            if count > max_count {
                max_count = count;
            }
        }
        if max_count > PER_FUNCTION_LIMIT {
            break;
        }
        local_types.clear();
        for param in &new_functions[idx].params {
            if let Some(t) = &param.type_expr
                && let Pattern::Variable(name, _) = &param.pattern
            {
                local_types.insert(name.clone(), t.clone());
            }
        }
        // Take ownership of the function temporarily so we can borrow
        // new_functions mutably for inserting nested specializations.
        let len_before = new_functions.len();
        let mut body_clone = new_functions[idx].body.clone();
        {
            use crate::visitor::MutVisitor;
            let mut visitor = CallSpecializer {
                generics: &generics,
                locals: &mut local_types,
                specs: &mut specs,
                new_functions: &mut new_functions,
                fn_returns: &fn_returns,
                struct_table: &struct_table,
            };
            visitor.visit_block(&mut body_clone);
        }
        new_functions[idx].body = body_clone;
        // Update per-function counts for any specializations
        // introduced by rewriting this function's body.
        if new_functions.len() > len_before {
            // Recover each new specialization's origin from `specs`
            // (the authoritative `(origin, type_args) -> mangled-name`
            // map) rather than by splitting the mangled name on `__`.
            // The `origin__type_args` shape is ambiguous when the origin
            // itself contains `__`, so the split miscounted the origin
            // and weakened the per-function recursion guard (audit
            // finding 26).
            let name_to_origin: BTreeMap<&str, &str> = specs
                .iter()
                .map(|((origin, _), mangled)| (mangled.as_str(), origin.as_str()))
                .collect();
            for new_fn in &new_functions[len_before..] {
                let origin = name_to_origin
                    .get(new_fn.name.as_str())
                    .map(|s| (*s).to_string())
                    .unwrap_or_else(|| new_fn.name.clone());
                *per_fn_counts.entry(origin).or_insert(0) += 1;
            }
        }
        idx += 1;
    }

    program.functions.extend(new_functions);
    // Drop generic functions that have at least one specialization
    // generated. Generics with no specializations remain in the
    // program because some call sites may not have been reachable
    // for inference and the original generic chunks still execute
    // correctly through runtime tag dispatch. The polymorphic
    // representation continues to be sound for closure-typed
    // arguments and other shapes whose concrete types cannot be
    // statically inferred.
    let specialized_origins: alloc::collections::BTreeSet<String> =
        specs.keys().map(|(name, _)| name.clone()).collect();
    // A const-generic function is dropped unconditionally: unlike a
    // type-generic function (which can remain and dispatch on runtime
    // tags), it cannot be compiled with a symbolic const, so every
    // instance must be a concrete specialization. An un-instantiated
    // const-generic function is dead code and is removed (B40).
    program
        .functions
        .retain(|f| !specialized_origins.contains(&f.name) && f.const_params.is_empty());

    // Provenance for B29 GenericInstantiation records: invert `specs`
    // so each specialized function's mangled name maps to its origin
    // and the canonical type-argument encoding.
    let provenance: BTreeMap<String, (String, String)> = specs
        .into_iter()
        .map(|((origin, type_args), mangled)| (mangled, (origin, type_args)))
        .collect();

    // Generic struct specialization. Walk the program once more
    // looking for `Expr::StructInit` whose target is a generic
    // struct. Infer the struct's type arguments from the provided
    // field values, generate a specialized `StructDef` with
    // concrete field types, and rewrite the `StructInit`'s name to
    // the specialized name. Subsequent compilation sees the
    // specialized struct as a regular non-generic struct, which
    // lets compile-time field-type inference resolve method
    // dispatch on field-typed receivers.
    program = specialize_structs(program, &fn_returns);

    // Generic enum specialization mirrors the struct pass for
    // `Expr::EnumVariant` whose target enum has type parameters.
    // The payload values' inferred types determine the type
    // arguments, and the pass emits a specialized `EnumDef` with
    // payload types substituted.
    program = specialize_enums(program, &fn_returns);
    (program, provenance)
}

/// Generic enum specialization pass. See [`specialize_structs`] for
/// the analogous struct pass. The mechanics mirror struct
/// specialization, with variant payload types in place of struct
/// field types.
/// Explicit upper bound on the number of distinct generic struct or enum
/// specializations a single pass may mint (audit finding 27). The pass already
/// runs once over the function bodies and deduplicates through its `specs` map,
/// so the count is bounded by the source size; this cap makes that bound
/// explicit and auditable, mirroring the function-specialization loop's
/// `SPECIALIZATION_LIMIT`. Beyond the cap a construction is left generic, so
/// later type checking reports a clear error rather than the pass emitting an
/// unbounded family of types.
const TYPE_SPECIALIZATION_LIMIT: usize = 1024;

fn specialize_enums(mut program: Program, fn_returns: &BTreeMap<String, TypeExpr>) -> Program {
    use crate::visitor::MutVisitor;
    let generic_enums: BTreeMap<String, EnumDef> = program
        .types
        .iter()
        .filter_map(|td| match td {
            TypeDef::Enum(e) if !e.type_params.is_empty() || !e.const_params.is_empty() => {
                Some((e.name.clone(), e.clone()))
            }
            _ => None,
        })
        .collect();
    if generic_enums.is_empty() {
        return program;
    }
    let mut enum_specs: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut new_enums: Vec<EnumDef> = Vec::new();
    let mut local_types: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for func in &mut program.functions {
        local_types.clear();
        for param in &func.params {
            if let Some(t) = &param.type_expr
                && let Pattern::Variable(name, _) = &param.pattern
            {
                local_types.insert(name.clone(), t.clone());
            }
        }
        let mut visitor = EnumSpecializer {
            generic_enums: &generic_enums,
            locals: &mut local_types,
            specs: &mut enum_specs,
            new_enums: &mut new_enums,
            fn_returns,
        };
        visitor.visit_block(&mut func.body);
    }
    // Rewrite enum type references in function signatures to their
    // specialized names, mirroring the struct pass: a const-generic
    // reference `Maybe<8>` in a signature resolves to `Maybe__c8`,
    // matching the specialized construction (B40).
    for func in &mut program.functions {
        for param in &mut func.params {
            if let Some(t) = &param.type_expr {
                param.type_expr = Some(resolve_generic_type_to_spec(t, &enum_specs));
            }
        }
        func.return_type = resolve_generic_type_to_spec(&func.return_type, &enum_specs);
    }
    program
        .types
        .extend(new_enums.into_iter().map(TypeDef::Enum));
    program
}

/// AST visitor that rewrites generic `Expr::EnumVariant` constructions
/// to specialized enum names, emitting a fresh `EnumDef` per
/// concrete instantiation.
struct EnumSpecializer<'a> {
    generic_enums: &'a BTreeMap<String, EnumDef>,
    locals: &'a mut BTreeMap<String, TypeExpr>,
    specs: &'a mut BTreeMap<(String, String), String>,
    new_enums: &'a mut Vec<EnumDef>,
    fn_returns: &'a BTreeMap<String, TypeExpr>,
}

impl crate::visitor::MutVisitor for EnumSpecializer<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Let(l) = stmt {
            self.visit_expr(&mut l.value);
            if let Pattern::Variable(name, _) = &l.pattern
                && let Some(t) = l
                    .type_expr
                    .clone()
                    .or_else(|| infer_arg_type(&l.value, self.locals, self.fn_returns, None))
            {
                self.locals.insert(name.clone(), t);
            }
            return;
        }
        self.walk_stmt(stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        // Recurse into children first so nested EnumVariant
        // constructions are specialized bottom-up.
        self.walk_expr(expr);

        // Handle `Expr::Match`: after the scrutinee has been
        // specialized through walk_expr, the scrutinee's inferred
        // type may now reference a specialized enum (e.g.,
        // `Maybe__Word`). The match arms' patterns retain the
        // original generic enum name (`Maybe`) and need to be
        // rewritten so subsequent type checking matches the
        // monomorphized scrutinee.
        if let Expr::Match {
            scrutinee, arms, ..
        } = expr
        {
            if let Some(scrutinee_ty) =
                infer_arg_type(scrutinee, self.locals, self.fn_returns, None)
                && let TypeExpr::Named(ty_name, ty_args, ty_const_args, _) = &scrutinee_ty
            {
                if let Some(original) = find_original_for_spec(self.specs, ty_name) {
                    // The scrutinee already infers to a specialization
                    // name (e.g. a locally constructed `Maybe__Word`).
                    let spec_name = ty_name.clone();
                    for arm in arms.iter_mut() {
                        rewrite_pattern_enum_name(&mut arm.pattern, &original, &spec_name);
                    }
                } else if self.generic_enums.contains_key(ty_name) {
                    // The scrutinee infers to a generic enum reference
                    // with concrete arguments (e.g. a parameter typed
                    // `Buf<3>`). Mint or reuse the specialization and
                    // rewrite the arm patterns from the generic name to
                    // the specialized name (B40 const-generic enums).
                    let const_values: Option<Vec<i64>> =
                        ty_const_args.iter().map(|c| c.as_lit()).collect();
                    if let Some(const_values) = const_values {
                        let original = ty_name.clone();
                        let type_args = ty_args.clone();
                        if let Some(spec_name) =
                            self.get_or_mint_enum_spec(&original, &type_args, &const_values)
                        {
                            for arm in arms.iter_mut() {
                                rewrite_pattern_enum_name(&mut arm.pattern, &original, &spec_name);
                            }
                        }
                    }
                }
            }
            return;
        }

        // Then check this node for a generic EnumVariant to specialize.
        let Expr::EnumVariant {
            enum_name,
            variant,
            args,
            const_args,
            ..
        } = expr
        else {
            return;
        };
        let Some(enum_def) = self.generic_enums.get(enum_name) else {
            return;
        };
        let Some(decl_variant) = enum_def.variants.iter().find(|v| v.name == *variant) else {
            return;
        };
        // Explicit const arguments from the construction turbofish
        // `Opt::<8>::Some(...)`, evaluated (post-substitution they are
        // ground). A missing or still-symbolic const argument defers.
        let empty: BTreeMap<String, i64> = BTreeMap::new();
        let mut const_values: Vec<i64> = Vec::new();
        for ca in const_args.iter() {
            match eval_const_expr(ca, &empty) {
                Some(v) => const_values.push(v),
                None => return,
            }
        }
        if const_values.len() != enum_def.const_params.len() {
            return;
        }
        let mut type_args: Vec<TypeExpr> = Vec::new();
        for tp in &enum_def.type_params {
            let mut inferred: Option<TypeExpr> = None;
            for (i, decl_ty) in decl_variant.fields.iter().enumerate() {
                if let TypeExpr::Named(n, _, _, _) = decl_ty
                    && *n == tp.name
                    && let Some(arg) = args.get(i)
                    && let Some(t) = infer_arg_type(arg, self.locals, self.fn_returns, None)
                {
                    inferred = Some(t);
                    break;
                }
            }
            match inferred {
                Some(t) => type_args.push(t),
                None => return,
            }
        }
        if type_args.len() != enum_def.type_params.len() {
            return;
        }
        let Some(spec_name) =
            self.get_or_mint_enum_spec(&enum_name.clone(), &type_args, &const_values)
        else {
            // Not a generic enum, or the specialization cap was reached
            // (audit finding 27); leave the construction generic for a
            // clean later error.
            return;
        };
        if let Expr::EnumVariant {
            enum_name,
            const_args,
            ..
        } = expr
        {
            *enum_name = spec_name;
            const_args.clear();
        }
    }
}

impl EnumSpecializer<'_> {
    /// Return the specialization name for a generic enum instantiated
    /// with the given type and const arguments, minting a fresh
    /// `EnumDef` on first use and caching it. Shared by the
    /// `EnumVariant` construction path and the match-arm pattern
    /// rewrite, so a construction `Buf::<3>::Tag(...)` and a scrutinee
    /// typed `Buf<3>` agree on `Buf__c3`. Returns `None` if the name is
    /// not a generic enum or the specialization cap is reached (audit
    /// finding 27), in which case the caller leaves the site generic
    /// for a clean later error (B40).
    fn get_or_mint_enum_spec(
        &mut self,
        enum_name: &str,
        type_args: &[TypeExpr],
        const_values: &[i64],
    ) -> Option<String> {
        let canonical = generic_cache_canonical(type_args, const_values);
        let cache_key = (enum_name.to_string(), canonical);
        if let Some(existing) = self.specs.get(&cache_key) {
            return Some(existing.clone());
        }
        // Copy the shared reference out so the immutable borrow of the
        // enum table does not overlap the mutation of `specs`/`new_enums`.
        let generic_enums: &BTreeMap<String, EnumDef> = self.generic_enums;
        let enum_def = generic_enums.get(enum_name)?;
        if self.new_enums.len() >= TYPE_SPECIALIZATION_LIMIT {
            return None;
        }
        let spec_name = mangle_struct_with_consts(enum_name, type_args, const_values);
        let specialized = specialize_enum(enum_def, type_args, const_values, spec_name.clone());
        self.specs.insert(cache_key, spec_name.clone());
        self.new_enums.push(specialized);
        Some(spec_name)
    }
}

/// Reverse-lookup the generic enum name that produced a given
/// specialization. Returns `None` if `spec_name` does not appear in
/// the specs map (i.e. the type is already monomorphic or is not an
/// enum specialization).
fn find_original_for_spec(
    specs: &BTreeMap<(String, String), String>,
    spec_name: &str,
) -> Option<String> {
    specs
        .iter()
        .find(|(_, v)| v.as_str() == spec_name)
        .map(|((orig, _), _)| orig.clone())
}

/// Rewrite every `Pattern::Enum(enum_name, ...)` inside `pattern`
/// whose `enum_name` matches `original_enum_name`, replacing it
/// with `spec_enum_name`. Recurses into composite patterns
/// (`Tuple`, `Struct`, and the variant payloads of `Enum` itself)
/// so nested matches against the same generic enum are also
/// rewritten.
fn rewrite_pattern_enum_name(
    pattern: &mut Pattern,
    original_enum_name: &str,
    spec_enum_name: &str,
) {
    match pattern {
        Pattern::Enum(enum_name, _variant, sub_patterns, _span) => {
            if enum_name == original_enum_name {
                *enum_name = spec_enum_name.to_string();
            }
            for sub in sub_patterns.iter_mut() {
                rewrite_pattern_enum_name(sub, original_enum_name, spec_enum_name);
            }
        }
        Pattern::Tuple(sub_patterns, _span) => {
            for sub in sub_patterns.iter_mut() {
                rewrite_pattern_enum_name(sub, original_enum_name, spec_enum_name);
            }
        }
        Pattern::Struct(_name, field_patterns, _span) => {
            for fp in field_patterns.iter_mut() {
                if let Some(p) = fp.pattern.as_mut() {
                    rewrite_pattern_enum_name(p, original_enum_name, spec_enum_name);
                }
            }
        }
        Pattern::Literal(_, _) | Pattern::Wildcard(_) | Pattern::Variable(_, _) => {}
    }
}

fn specialize_enum(
    enum_def: &EnumDef,
    type_args: &[TypeExpr],
    const_values: &[i64],
    spec_name: String,
) -> EnumDef {
    let mut subst: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for (tp, arg) in enum_def.type_params.iter().zip(type_args.iter()) {
        subst.insert(tp.name.clone(), arg.clone());
    }
    let const_subst: BTreeMap<String, i64> = enum_def
        .const_params
        .iter()
        .zip(const_values.iter())
        .map(|(cp, v)| (cp.name.clone(), *v))
        .collect();
    let variants: Vec<VariantDecl> = enum_def
        .variants
        .iter()
        .map(|v| VariantDecl {
            name: v.name.clone(),
            fields: v
                .fields
                .iter()
                .map(|t| subst_const_dims_in_type(&subst_type_expr(t, &subst), &const_subst))
                .collect(),
            explicit_discriminant: v.explicit_discriminant,
            discriminant_value: v.discriminant_value,
            span: v.span,
        })
        .collect();
    EnumDef {
        name: spec_name,
        type_params: Vec::new(),
        const_params: Vec::new(),
        variants,
        span: enum_def.span,
    }
}

/// Generic struct specialization pass.
///
/// Walks the program for `Expr::StructInit` expressions whose target
/// struct has type parameters. For each, infers the struct's type
/// arguments by matching declared field types against the provided
/// field values' types. When all type arguments can be inferred,
/// emits a specialized `StructDef` with concrete field types and
/// rewrites the `StructInit`'s name to a mangled form. The original
/// generic struct is retained alongside the specialization.
fn specialize_structs(mut program: Program, fn_returns: &BTreeMap<String, TypeExpr>) -> Program {
    use crate::visitor::MutVisitor;
    let generic_structs: BTreeMap<String, StructDef> = program
        .types
        .iter()
        .filter_map(|td| match td {
            TypeDef::Struct(s) if !s.type_params.is_empty() || !s.const_params.is_empty() => {
                Some((s.name.clone(), s.clone()))
            }
            _ => None,
        })
        .collect();
    if generic_structs.is_empty() {
        return program;
    }
    let mut struct_specs: BTreeMap<(String, String), String> = BTreeMap::new();
    let mut reverse_specs: BTreeMap<String, (String, Vec<TypeExpr>)> = BTreeMap::new();
    let mut new_structs: Vec<StructDef> = Vec::new();
    let mut local_types: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for func in &mut program.functions {
        local_types.clear();
        for param in &func.params {
            if let Some(t) = &param.type_expr
                && let Pattern::Variable(name, _) = &param.pattern
            {
                local_types.insert(name.clone(), t.clone());
            }
        }
        let mut visitor = StructSpecializer {
            generic_structs: &generic_structs,
            locals: &mut local_types,
            specs: &mut struct_specs,
            reverse_specs: &mut reverse_specs,
            new_structs: &mut new_structs,
            fn_returns,
        };
        visitor.visit_block(&mut func.body);
    }
    // Rewrite struct type references in function signatures (parameters
    // and return types) to their specialized names, now that every
    // specialization has been collected. A const-generic type reference
    // `Buf<8>` in a signature resolves to `Buf__c8`, matching the
    // specialized construction (B40).
    for func in &mut program.functions {
        for param in &mut func.params {
            if let Some(t) = &param.type_expr {
                param.type_expr = Some(resolve_generic_type_to_spec(t, &struct_specs));
            }
        }
        func.return_type = resolve_generic_type_to_spec(&func.return_type, &struct_specs);
    }
    program
        .types
        .extend(new_structs.into_iter().map(TypeDef::Struct));
    program
}

/// AST visitor that rewrites generic `Expr::StructInit` constructions
/// to specialized struct names, emitting a fresh `StructDef` per
/// concrete instantiation.
struct StructSpecializer<'a> {
    generic_structs: &'a BTreeMap<String, StructDef>,
    locals: &'a mut BTreeMap<String, TypeExpr>,
    specs: &'a mut BTreeMap<(String, String), String>,
    /// Reverse lookup mapping each emitted specialization name back
    /// to its original generic name and the concrete type arguments
    /// used to produce it. Used by the inference loop to recover
    /// type arguments when a field's declared type is itself a
    /// generic instantiation (`inner: Cell<T>` where the init
    /// produces `Cell__Word`).
    reverse_specs: &'a mut BTreeMap<String, (String, Vec<TypeExpr>)>,
    new_structs: &'a mut Vec<StructDef>,
    fn_returns: &'a BTreeMap<String, TypeExpr>,
}

impl crate::visitor::MutVisitor for StructSpecializer<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Let(l) = stmt {
            self.visit_expr(&mut l.value);
            if let Pattern::Variable(name, _) = &l.pattern
                && let Some(t) = l
                    .type_expr
                    .clone()
                    .or_else(|| infer_arg_type(&l.value, self.locals, self.fn_returns, None))
            {
                self.locals.insert(name.clone(), t);
            }
            return;
        }
        self.walk_stmt(stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        // Recurse into children first so nested StructInit
        // constructions specialize bottom-up.
        self.walk_expr(expr);
        let Expr::StructInit {
            name,
            fields,
            const_args,
            ..
        } = expr
        else {
            return;
        };
        let Some(struct_def) = self.generic_structs.get(name) else {
            return;
        };
        // Explicit const arguments from the construction turbofish
        // `Buf::<8> { ... }`, evaluated (post-substitution they are
        // ground). A missing or still-symbolic const argument defers.
        let empty: BTreeMap<String, i64> = BTreeMap::new();
        let mut const_values: Vec<i64> = Vec::new();
        for ca in const_args.iter() {
            match eval_const_expr(ca, &empty) {
                Some(v) => const_values.push(v),
                None => return,
            }
        }
        if const_values.len() != struct_def.const_params.len() {
            return;
        }
        let mut type_args: Vec<TypeExpr> = Vec::new();
        for tp in &struct_def.type_params {
            let mut inferred: Option<TypeExpr> = None;
            for decl_field in &struct_def.fields {
                let Some(init) = fields.iter().find(|f| f.name == decl_field.name) else {
                    continue;
                };
                let Some(init_ty) = infer_arg_type(&init.value, self.locals, self.fn_returns, None)
                else {
                    continue;
                };
                // Case 1: the field's declared type is the type
                // parameter itself (e.g. `value: T`). The inferred
                // init type is the bound for `T`.
                if let TypeExpr::Named(n, _, _, _) = &decl_field.type_expr
                    && *n == tp.name
                {
                    inferred = Some(init_ty);
                    break;
                }
                // Case 2: the field's declared type is a generic
                // instantiation that mentions `tp.name` among its
                // arguments (e.g. `inner: Cell<T>`). The inferred
                // init type is a specialization that we can reverse
                // through `reverse_specs` to recover the bound for
                // `T`. Single-level only; deeper nesting (e.g.
                // `Cell<Wrap<T>>`) is not yet handled.
                if let TypeExpr::Named(outer_decl, decl_args, _, _) = &decl_field.type_expr
                    && let TypeExpr::Named(spec_name, _, _, _) = &init_ty
                    && let Some((orig, inferred_args)) = self.reverse_specs.get(spec_name)
                    && orig == outer_decl
                {
                    for (decl_arg, inf_arg) in decl_args.iter().zip(inferred_args.iter()) {
                        if let TypeExpr::Named(arg_n, _, _, _) = decl_arg
                            && *arg_n == tp.name
                        {
                            inferred = Some(inf_arg.clone());
                            break;
                        }
                    }
                    if inferred.is_some() {
                        break;
                    }
                }
            }
            match inferred {
                Some(t) => type_args.push(t),
                None => return,
            }
        }
        if type_args.len() != struct_def.type_params.len() {
            return;
        }
        let canonical = generic_cache_canonical(&type_args, &const_values);
        let cache_key = (name.clone(), canonical);
        let spec_name = if let Some(existing) = self.specs.get(&cache_key) {
            existing.clone()
        } else if self.new_structs.len() >= TYPE_SPECIALIZATION_LIMIT {
            // Explicit specialization cap reached (audit finding 27);
            // leave the construction generic for a clean later error.
            return;
        } else {
            let spec_name = mangle_struct_with_consts(name, &type_args, &const_values);
            // Pass the in-progress specs so any nested generic
            // field types (e.g. `inner: Cell<i64>` inside a
            // freshly specialized `Wrap<i64>`) are rewritten to
            // their already-emitted specialization names. The
            // bottom-up walk guarantees inner specializations
            // exist before the outer is emitted.
            let specialized = specialize_struct(
                struct_def,
                &type_args,
                &const_values,
                spec_name.clone(),
                self.specs,
            );
            self.specs.insert(cache_key, spec_name.clone());
            self.reverse_specs
                .insert(spec_name.clone(), (name.clone(), type_args.clone()));
            self.new_structs.push(specialized);
            spec_name
        };
        if let Expr::StructInit {
            name, const_args, ..
        } = expr
        {
            *name = spec_name;
            const_args.clear();
        }
    }
}

fn mangle_struct(name: &str, type_args: &[TypeExpr]) -> String {
    let mut s = name.to_string();
    for arg in type_args {
        s.push_str("__");
        s.push_str(&type_arg_canonical(arg));
    }
    s
}

/// Canonical cache key for a generic instantiation over type and const
/// arguments, shared by the specializer (which populates the cache) and
/// the type-reference resolver (which reads it), so a const-generic type
/// reference resolves to the same specialization (B40).
fn generic_cache_canonical(type_args: &[TypeExpr], const_values: &[i64]) -> String {
    use alloc::string::ToString;
    let mut s = type_args
        .iter()
        .map(type_arg_canonical)
        .collect::<Vec<_>>()
        .join(",");
    if !const_values.is_empty() {
        s.push_str(";c=");
        s.push_str(
            &const_values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    s
}

/// Mangle a specialized struct/enum name including const arguments after
/// the type arguments, mirroring [`mangle_with_consts`] (B40).
fn mangle_struct_with_consts(name: &str, type_args: &[TypeExpr], const_values: &[i64]) -> String {
    use alloc::string::ToString;
    let mut s = mangle_struct(name, type_args);
    for v in const_values {
        s.push_str("__c");
        if *v < 0 {
            s.push('n');
            s.push_str(&v.unsigned_abs().to_string());
        } else {
            s.push_str(&v.to_string());
        }
    }
    s
}

fn specialize_struct(
    struct_def: &StructDef,
    type_args: &[TypeExpr],
    const_values: &[i64],
    spec_name: String,
    existing_specs: &BTreeMap<(String, String), String>,
) -> StructDef {
    let mut subst: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for (tp, arg) in struct_def.type_params.iter().zip(type_args.iter()) {
        subst.insert(tp.name.clone(), arg.clone());
    }
    let const_subst: BTreeMap<String, i64> = struct_def
        .const_params
        .iter()
        .zip(const_values.iter())
        .map(|(cp, v)| (cp.name.clone(), *v))
        .collect();
    let fields: Vec<FieldDecl> = struct_def
        .fields
        .iter()
        .map(|f| {
            let substituted = subst_type_expr(&f.type_expr, &subst);
            let substituted = subst_const_dims_in_type(&substituted, &const_subst);
            FieldDecl {
                name: f.name.clone(),
                type_expr: resolve_generic_type_to_spec(&substituted, existing_specs),
                span: f.span,
            }
        })
        .collect();
    StructDef {
        name: spec_name,
        type_params: Vec::new(),
        const_params: Vec::new(),
        fields,
        span: struct_def.span,
    }
}

/// Walk a `TypeExpr` and rewrite any `Named(outer, [args])` whose
/// `(outer, canonical_args)` pair is in the specs map to the
/// emitted specialization name without type arguments. Used after
/// type-parameter substitution to keep specialized field types in
/// sync with the specialized struct definitions the monomorphizer
/// has emitted.
fn resolve_generic_type_to_spec(
    t: &TypeExpr,
    specs: &BTreeMap<(String, String), String>,
) -> TypeExpr {
    match t {
        TypeExpr::Named(name, args, const_args, span)
            if !args.is_empty() || !const_args.is_empty() =>
        {
            let resolved_args: Vec<TypeExpr> = args
                .iter()
                .map(|a| resolve_generic_type_to_spec(a, specs))
                .collect();
            // A const argument in a type reference is a literal after
            // substitution; a still-symbolic one leaves the reference
            // generic (an internal state the re-typecheck rejects).
            let const_values: Option<Vec<i64>> = const_args.iter().map(|c| c.as_lit()).collect();
            if let Some(const_values) = const_values {
                let canonical = generic_cache_canonical(&resolved_args, &const_values);
                if let Some(spec) = specs.get(&(name.clone(), canonical)) {
                    return TypeExpr::Named(spec.clone(), Vec::new(), Vec::new(), *span);
                }
            }
            TypeExpr::Named(name.clone(), resolved_args, const_args.clone(), *span)
        }
        TypeExpr::Named(name, args, const_args, span) => {
            TypeExpr::Named(name.clone(), args.clone(), const_args.clone(), *span)
        }
        TypeExpr::Tuple(items, span) => TypeExpr::Tuple(
            items
                .iter()
                .map(|i| resolve_generic_type_to_spec(i, specs))
                .collect(),
            *span,
        ),
        TypeExpr::Array(elem, len, span) => TypeExpr::Array(
            alloc::boxed::Box::new(resolve_generic_type_to_spec(elem, specs)),
            len.clone(),
            *span,
        ),
        TypeExpr::Option(inner, span) => TypeExpr::Option(
            alloc::boxed::Box::new(resolve_generic_type_to_spec(inner, specs)),
            *span,
        ),
        other => other.clone(),
    }
}

/// Mangle a function name with its type arguments. The mangling uses
/// double underscores between segments to avoid colliding with
/// existing path-style identifiers and to remain a valid lower-case
/// identifier for the parser's path syntax.
fn mangle(name: &str, type_args: &[TypeExpr]) -> String {
    let mut s = name.to_string();
    for arg in type_args {
        s.push_str("__");
        s.push_str(&type_arg_canonical(arg));
    }
    s
}

/// Mangle a specialized name including const arguments after the type
/// arguments. A negative const value is rendered with an `n` prefix
/// (`c__n3` for `-3`) so the mangled name has no `-` (B40).
fn mangle_with_consts(name: &str, type_args: &[TypeExpr], const_values: &[i64]) -> String {
    let mut s = mangle(name, type_args);
    for v in const_values {
        s.push_str("__c");
        if *v < 0 {
            s.push('n');
            s.push_str(&v.unsigned_abs().to_string());
        } else {
            s.push_str(&v.to_string());
        }
    }
    s
}

/// Canonical short string representation for a type argument used in
/// mangling. Uses the head name only, which is sufficient because
/// monomorphization happens after type checking has validated the
/// concrete type.
fn type_arg_canonical(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Prim(p, _) => match p {
            PrimType::Byte => "Byte".to_string(),
            PrimType::Word => "Word".to_string(),
            PrimType::Fixed(Some(n)) => alloc::format!("Fixed{}", n),
            PrimType::Fixed(None) => "Fixed".to_string(),
            PrimType::Float => "Float".to_string(),
            PrimType::Bool => "bool".to_string(),
            PrimType::Text => "Text".to_string(),
        },
        TypeExpr::Unit(_) => "unit".to_string(),
        TypeExpr::Named(n, args, _, _) => {
            if args.is_empty() {
                n.clone()
            } else {
                let inner: Vec<String> = args.iter().map(type_arg_canonical).collect();
                format!("{}_{}", n, inner.join("_"))
            }
        }
        TypeExpr::Tuple(items, _) => {
            let inner: Vec<String> = items.iter().map(type_arg_canonical).collect();
            format!("tuple_{}", inner.join("_"))
        }
        TypeExpr::Array(elem, n, _) => {
            format!("arr_{}_{}", type_arg_canonical(elem), n)
        }
        TypeExpr::Multiword(n, f, _) => format!("Multiword{}_{}", n, f),
        TypeExpr::Option(inner, _) => format!("opt_{}", type_arg_canonical(inner)),
        TypeExpr::Labelled(inner, _, _) => type_arg_canonical(inner),
        TypeExpr::NegativeLabelled(inner, _, _) => type_arg_canonical(inner),
    }
}

/// Infer the type of an expression for monomorphization purposes.
///
/// Returns the most concrete `TypeExpr` derivable from local
/// information. The pass returns `None` for expressions whose type
/// cannot be determined, in which case the call site is not
/// specialized and the runtime falls back to the generic dispatch.
///
/// The optional `structs` argument enables `FieldAccess` resolution
/// against declared struct field types. Callers that lack a struct
/// table pass `None`, in which case `FieldAccess` returns `None`.
fn infer_arg_type(
    expr: &Expr,
    locals: &BTreeMap<String, TypeExpr>,
    fn_returns: &BTreeMap<String, TypeExpr>,
    structs: Option<&BTreeMap<String, StructDef>>,
) -> Option<TypeExpr> {
    match expr {
        Expr::Literal { value, span } => Some(match value {
            Literal::Int(_) => TypeExpr::Prim(PrimType::Word, *span),
            Literal::Float(_) => TypeExpr::Prim(PrimType::Float, *span),
            Literal::Byte(_) => TypeExpr::Prim(PrimType::Byte, *span),
            Literal::Fixed { frac_bits, .. } => {
                TypeExpr::Prim(PrimType::Fixed(Some(*frac_bits)), *span)
            }
            Literal::Bool(_) => TypeExpr::Prim(PrimType::Bool, *span),
            Literal::String(_) => TypeExpr::Prim(PrimType::Text, *span),
            Literal::Unit => TypeExpr::Unit(*span),
        }),
        Expr::Ident { name, .. } => locals.get(name).cloned(),
        Expr::StructInit { name, span, .. } => {
            Some(TypeExpr::Named(name.clone(), Vec::new(), Vec::new(), *span))
        }
        Expr::EnumVariant {
            enum_name, span, ..
        } => Some(TypeExpr::Named(
            enum_name.clone(),
            Vec::new(),
            Vec::new(),
            *span,
        )),
        Expr::Call { name, .. } => fn_returns.get(name).cloned(),
        Expr::Cast { target, .. } => Some(target.clone()),
        Expr::TupleLiteral { elements, span } => {
            let parts: Option<Vec<TypeExpr>> = elements
                .iter()
                .map(|e| infer_arg_type(e, locals, fn_returns, structs))
                .collect();
            parts.map(|p| TypeExpr::Tuple(p, *span))
        }
        Expr::ArrayLiteral { elements, span } => {
            let elem = elements.first()?;
            let elem_ty = infer_arg_type(elem, locals, fn_returns, structs)?;
            Some(TypeExpr::array_lit(
                Box::new(elem_ty),
                elements.len() as i64,
                *span,
            ))
        }
        Expr::If {
            then_block,
            else_block,
            ..
        } => {
            // Use the type of the then-branch's tail expression.
            let tail = then_block.tail_expr.as_ref()?;
            infer_arg_type(tail, locals, fn_returns, structs).or_else(|| {
                else_block
                    .as_ref()
                    .and_then(|b| b.tail_expr.as_ref())
                    .and_then(|e| infer_arg_type(e, locals, fn_returns, structs))
            })
        }
        Expr::Match { arms, .. } => {
            // All arms agree on type per the type checker. Use the
            // first arm's expression type.
            let first = arms.first()?;
            infer_arg_type(&first.expr, locals, fn_returns, structs)
        }
        Expr::TupleIndex { object, index, .. } => {
            // Resolve the object's type. If it is a tuple type with
            // enough elements, the indexed element's type is the
            // result. Out-of-range indices return None and let the
            // call site fall back to runtime tag dispatch.
            let obj_ty = infer_arg_type(object, locals, fn_returns, structs)?;
            if let TypeExpr::Tuple(elements, _) = obj_ty {
                let idx = *index as usize;
                elements.get(idx).cloned()
            } else {
                None
            }
        }
        Expr::ArrayIndex { object, .. } => {
            // Array element type is the inferred element of the
            // object's array type, regardless of the index value.
            let obj_ty = infer_arg_type(object, locals, fn_returns, structs)?;
            if let TypeExpr::Array(elem, _, _) = obj_ty {
                Some(*elem)
            } else {
                None
            }
        }
        Expr::FieldAccess { object, field, .. } => {
            // Resolve the object's nominal type, look up the struct's
            // declared field type, and apply the per-instance
            // substitution from the struct's type parameters to the
            // instance's type arguments. When the struct table is not
            // available or the lookup fails, return None and let the
            // call site fall back to runtime tag dispatch.
            let structs = structs?;
            let obj_ty = infer_arg_type(object, locals, fn_returns, Some(structs))?;
            let (struct_name, type_args) = match obj_ty {
                TypeExpr::Named(name, args, _, _) => (name, args),
                _ => return None,
            };
            let struct_def = structs.get(&struct_name)?;
            let field_decl = struct_def.fields.iter().find(|f| f.name == *field)?;
            if struct_def.type_params.len() == type_args.len() && !type_args.is_empty() {
                let subst: BTreeMap<String, TypeExpr> = struct_def
                    .type_params
                    .iter()
                    .zip(type_args.iter())
                    .map(|(tp, arg)| (tp.name.clone(), arg.clone()))
                    .collect();
                Some(subst_type_expr(&field_decl.type_expr, &subst))
            } else {
                // No type-arg substitution available. If the field's
                // declared type is exactly one of the struct's type
                // parameters, the inferred type would be abstract
                // and would erroneously propagate as a concrete
                // type argument at the call site. Return None so
                // the call falls back to runtime tag dispatch.
                if let TypeExpr::Named(field_name, field_args, _, _) = &field_decl.type_expr
                    && field_args.is_empty()
                    && struct_def
                        .type_params
                        .iter()
                        .any(|tp| tp.name == *field_name)
                {
                    return None;
                }
                Some(field_decl.type_expr.clone())
            }
        }
        Expr::UnaryOp { op, operand, .. } => {
            // Negation preserves the operand's type. Logical-not
            // returns Bool. Both arms recurse on the operand for
            // type information.
            match op {
                UnaryOp::Neg | UnaryOp::Bnot => {
                    infer_arg_type(operand, locals, fn_returns, structs)
                }
                UnaryOp::Not => Some(TypeExpr::Prim(PrimType::Bool, operand.span())),
            }
        }
        Expr::BinOp { op, left, span, .. } => {
            // Arithmetic operators preserve the operand types under
            // the type checker's existing same-type unification.
            // Comparison and logical operators return Bool.
            match op {
                // Arithmetic and shifts preserve the shifted value's
                // (left operand's) type.
                BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Mod
                | BinOp::Shl
                | BinOp::AShl
                | BinOp::ShrA
                | BinOp::ShrL
                | BinOp::Band
                | BinOp::Bor
                | BinOp::Bxor => infer_arg_type(left, locals, fn_returns, structs),
                BinOp::Eq
                | BinOp::NotEq
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::LtEq
                | BinOp::GtEq
                | BinOp::And
                | BinOp::Or
                | BinOp::Xor
                | BinOp::Andalso
                | BinOp::Orelse => Some(TypeExpr::Prim(PrimType::Bool, *span)),
            }
        }
        Expr::MethodCall {
            receiver, method, ..
        } => {
            // Resolve the receiver's nominal type, take its head, and
            // look up the impl method's return type in fn_returns
            // under the `<head>::<method>` mangling. The map was
            // populated at the top of `monomorphize` from
            // `program.impls`. When the lookup fails (no impl found,
            // or the receiver type is not yet concrete), return None.
            let recv_ty = infer_arg_type(receiver, locals, fn_returns, structs)?;
            let head = type_head_for_impl(&recv_ty);
            let key = alloc::format!("{}::{}", head, method);
            fn_returns.get(&key).cloned()
        }
        _ => None,
    }
}

/// Compute the head string for a `TypeExpr` used to key impl method
/// returns and impl-method chunk lookups. Mirrors the compiler's
/// existing convention so the `<head>::<method>` mangling here is
/// consistent with the compiler's `Trait::<head>::<method>` chunk
/// names.
fn type_head_for_impl(ty: &TypeExpr) -> String {
    use alloc::string::ToString;
    match ty {
        TypeExpr::Prim(p, _) => match p {
            PrimType::Byte => "Byte".to_string(),
            PrimType::Word => "Word".to_string(),
            PrimType::Fixed(Some(n)) => alloc::format!("Fixed{}", n),
            PrimType::Fixed(None) => "Fixed".to_string(),
            PrimType::Float => "Float".to_string(),
            PrimType::Bool => "bool".to_string(),
            PrimType::Text => "Text".to_string(),
        },
        TypeExpr::Unit(_) => "()".to_string(),
        TypeExpr::Named(name, _, _, _) => name.clone(),
        TypeExpr::Tuple(_, _) => "tuple".to_string(),
        TypeExpr::Array(_, _, _) => "array".to_string(),
        TypeExpr::Multiword(_, _, _) => "Multiword".to_string(),
        TypeExpr::Option(_, _) => "Option".to_string(),
        TypeExpr::Labelled(inner, _, _) => type_head_for_impl(inner),
        TypeExpr::NegativeLabelled(inner, _, _) => type_head_for_impl(inner),
    }
}

/// Substitute type-parameter names with concrete type expressions
/// inside a `TypeExpr`. Type parameters not present in `subst` are
/// preserved. Matches by the `Named` form's identifier; primitives
/// and structural types recurse.
fn subst_type_expr(t: &TypeExpr, subst: &BTreeMap<String, TypeExpr>) -> TypeExpr {
    match t {
        TypeExpr::Prim(_, _) | TypeExpr::Unit(_) | TypeExpr::Multiword(_, _, _) => t.clone(),
        TypeExpr::Named(name, args, const_args, span) => {
            if args.is_empty()
                && const_args.is_empty()
                && let Some(replacement) = subst.get(name)
            {
                return replacement.clone();
            }
            TypeExpr::Named(
                name.clone(),
                args.iter().map(|a| subst_type_expr(a, subst)).collect(),
                const_args.clone(),
                *span,
            )
        }
        TypeExpr::Tuple(items, span) => TypeExpr::Tuple(
            items.iter().map(|t| subst_type_expr(t, subst)).collect(),
            *span,
        ),
        TypeExpr::Array(elem, n, span) => {
            TypeExpr::Array(Box::new(subst_type_expr(elem, subst)), n.clone(), *span)
        }
        TypeExpr::Option(inner, span) => {
            TypeExpr::Option(Box::new(subst_type_expr(inner, subst)), *span)
        }
        TypeExpr::Labelled(inner, labels, span) => TypeExpr::Labelled(
            Box::new(subst_type_expr(inner, subst)),
            labels.clone(),
            *span,
        ),
        TypeExpr::NegativeLabelled(inner, labels, span) => TypeExpr::NegativeLabelled(
            Box::new(subst_type_expr(inner, subst)),
            labels.clone(),
            *span,
        ),
    }
}

/// Specialize a generic function with concrete type arguments. Clones
/// the function and substitutes type parameters in param types,
/// return type, and the body.
/// Evaluate a const expression to a concrete integer, resolving const
/// parameters through `subst`. Total over `+`, `-`, `*` (wrapping),
/// which is why const arithmetic excludes division. Returns `None` when
/// a referenced parameter is not in `subst` (an unresolved const
/// parameter), which defers specialization to a later fixed-point pass
/// once the enclosing function is itself specialized (B40).
fn eval_const_expr(e: &crate::ast::ConstExpr, subst: &BTreeMap<String, i64>) -> Option<i64> {
    use crate::ast::{ConstBinOp, ConstExpr};
    match e {
        ConstExpr::Lit(n, _) => Some(*n),
        ConstExpr::Param(name, _) => subst.get(name).copied(),
        ConstExpr::Bin(op, l, r, _) => {
            let a = eval_const_expr(l, subst)?;
            let b = eval_const_expr(r, subst)?;
            Some(match op {
                ConstBinOp::Add => a.wrapping_add(b),
                ConstBinOp::Sub => a.wrapping_sub(b),
                ConstBinOp::Mul => a.wrapping_mul(b),
            })
        }
    }
}

/// Substitute const parameters in a const expression with their values,
/// folding to a literal where both operands are known (B40).
fn subst_const_expr(
    ce: &crate::ast::ConstExpr,
    const_subst: &BTreeMap<String, i64>,
) -> crate::ast::ConstExpr {
    use crate::ast::{ConstBinOp, ConstExpr};
    match ce {
        ConstExpr::Lit(_, _) => ce.clone(),
        ConstExpr::Param(name, span) => match const_subst.get(name) {
            Some(v) => ConstExpr::Lit(*v, *span),
            None => ce.clone(),
        },
        ConstExpr::Bin(op, l, r, span) => {
            let l = subst_const_expr(l, const_subst);
            let r = subst_const_expr(r, const_subst);
            if let (Some(a), Some(b)) = (l.as_lit(), r.as_lit()) {
                let v = match op {
                    ConstBinOp::Add => a.wrapping_add(b),
                    ConstBinOp::Sub => a.wrapping_sub(b),
                    ConstBinOp::Mul => a.wrapping_mul(b),
                };
                ConstExpr::Lit(v, *span)
            } else {
                ConstExpr::Bin(*op, Box::new(l), Box::new(r), *span)
            }
        }
    }
}

/// Substitute const parameters in every const dimension of a type
/// expression (array sizes and `Multiword` N/F), recursing into nested
/// types (B40).
fn subst_const_dims_in_type(te: &TypeExpr, const_subst: &BTreeMap<String, i64>) -> TypeExpr {
    match te {
        TypeExpr::Prim(_, _) | TypeExpr::Unit(_) => te.clone(),
        TypeExpr::Array(elem, n, span) => TypeExpr::Array(
            Box::new(subst_const_dims_in_type(elem, const_subst)),
            subst_const_expr(n, const_subst),
            *span,
        ),
        TypeExpr::Multiword(n, f, span) => TypeExpr::Multiword(
            subst_const_expr(n, const_subst),
            subst_const_expr(f, const_subst),
            *span,
        ),
        TypeExpr::Tuple(items, span) => TypeExpr::Tuple(
            items
                .iter()
                .map(|t| subst_const_dims_in_type(t, const_subst))
                .collect(),
            *span,
        ),
        TypeExpr::Named(name, args, const_args, span) => TypeExpr::Named(
            name.clone(),
            args.iter()
                .map(|a| subst_const_dims_in_type(a, const_subst))
                .collect(),
            const_args
                .iter()
                .map(|c| subst_const_expr(c, const_subst))
                .collect(),
            *span,
        ),
        TypeExpr::Option(inner, span) => TypeExpr::Option(
            Box::new(subst_const_dims_in_type(inner, const_subst)),
            *span,
        ),
        TypeExpr::Labelled(inner, l, span) => TypeExpr::Labelled(
            Box::new(subst_const_dims_in_type(inner, const_subst)),
            l.clone(),
            *span,
        ),
        TypeExpr::NegativeLabelled(inner, l, span) => TypeExpr::NegativeLabelled(
            Box::new(subst_const_dims_in_type(inner, const_subst)),
            l.clone(),
            *span,
        ),
    }
}

/// Collect the variable names a pattern binds, recursing into tuple and
/// enum sub-patterns.
fn collect_pattern_bindings(pattern: &Pattern, out: &mut Vec<String>) {
    match pattern {
        Pattern::Variable(name, _) => out.push(name.clone()),
        Pattern::Enum(_, _, subs, _) | Pattern::Tuple(subs, _) => {
            for s in subs {
                collect_pattern_bindings(s, out);
            }
        }
        Pattern::Struct(_, fields, _) => {
            for f in fields {
                match &f.pattern {
                    // A sub-pattern binds its own names.
                    Some(p) => collect_pattern_bindings(p, out),
                    // Field shorthand binds a local named after the field.
                    None => out.push(f.name.clone()),
                }
            }
        }
        Pattern::Literal(_, _) | Pattern::Wildcard(_) => {}
    }
}

/// Substitute const-parameter value references (`Expr::Ident(n)`) with
/// their concrete literal, honouring local lexical shadowing. A `let`
/// binding, a `for` loop variable, or a match-arm binding that reuses a
/// const parameter's name shadows it within its scope, so an inner
/// reference to that name is left untouched. This is the correctness
/// crux of const-generic value substitution (B40).
struct ConstValueSubstitutor {
    subst: BTreeMap<String, i64>,
    shadowed: Vec<BTreeSet<String>>,
}

impl ConstValueSubstitutor {
    fn is_shadowed(&self, name: &str) -> bool {
        self.shadowed.iter().any(|s| s.contains(name))
    }
    fn shadow_if_const(&mut self, name: &str) {
        if self.subst.contains_key(name)
            && let Some(top) = self.shadowed.last_mut()
        {
            top.insert(name.to_string());
        }
    }
}

impl crate::visitor::MutVisitor for ConstValueSubstitutor {
    fn visit_block(&mut self, block: &mut Block) {
        self.shadowed.push(BTreeSet::new());
        self.walk_block(block);
        self.shadowed.pop();
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Let(l) => {
                // The value is evaluated before the binding takes effect,
                // so it sees the pre-binding scope.
                self.visit_expr(&mut l.value);
                // A `let x: [Word; n] = ...` type annotation carries const
                // dimensions that must be substituted too.
                if let Some(t) = &l.type_expr {
                    l.type_expr = Some(subst_const_dims_in_type(t, &self.subst));
                }
                let mut names = Vec::new();
                collect_pattern_bindings(&l.pattern, &mut names);
                for name in names {
                    self.shadow_if_const(&name);
                }
            }
            Stmt::For(f) => {
                // The iterable is evaluated in the outer scope; the loop
                // variable shadows within the body.
                match &mut f.iterable {
                    Iterable::Expr(e) => self.visit_expr(e),
                    Iterable::Range(a, b) => {
                        self.visit_expr(a);
                        self.visit_expr(b);
                    }
                }
                let var = f.var.clone();
                self.shadowed.push(BTreeSet::new());
                self.shadow_if_const(&var);
                self.visit_block(&mut f.body);
                self.shadowed.pop();
            }
            _ => self.walk_stmt(stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Ident { name, span } => {
                if self.subst.contains_key(name) && !self.is_shadowed(name) {
                    let value = self.subst[name];
                    *expr = Expr::Literal {
                        value: Literal::Int(value),
                        span: *span,
                    };
                }
            }
            Expr::Cast {
                expr: inner,
                target,
                ..
            } => {
                // A cast target such as `... as Multiword<n>` carries
                // const dimensions that must be substituted.
                *target = subst_const_dims_in_type(target, &self.subst);
                self.visit_expr(inner);
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                self.visit_expr(scrutinee);
                for arm in arms.iter_mut() {
                    self.shadowed.push(BTreeSet::new());
                    let mut names = Vec::new();
                    collect_pattern_bindings(&arm.pattern, &mut names);
                    for name in names {
                        self.shadow_if_const(&name);
                    }
                    if let Some(g) = &mut arm.guard {
                        self.visit_expr(g);
                    }
                    self.visit_expr(&mut arm.expr);
                    self.shadowed.pop();
                }
            }
            _ => self.walk_expr(expr),
        }
    }
}

fn specialize_function(
    func: &FunctionDef,
    type_args: &[TypeExpr],
    const_values: &[i64],
    spec_name: String,
) -> FunctionDef {
    let mut subst: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for (tp, arg) in func.type_params.iter().zip(type_args.iter()) {
        subst.insert(tp.name.clone(), arg.clone());
    }
    let const_subst: BTreeMap<String, i64> = func
        .const_params
        .iter()
        .zip(const_values.iter())
        .map(|(cp, val)| (cp.name.clone(), *val))
        .collect();
    let params: Vec<Param> = func
        .params
        .iter()
        .map(|p| Param {
            pattern: p.pattern.clone(),
            type_expr: p
                .type_expr
                .as_ref()
                .map(|t| subst_const_dims_in_type(&subst_type_expr(t, &subst), &const_subst)),
            span: p.span,
        })
        .collect();
    let return_type =
        subst_const_dims_in_type(&subst_type_expr(&func.return_type, &subst), &const_subst);
    let mut body = subst_in_block(&func.body, &subst);
    // Substitute const-parameter value references and const dimensions in
    // the body with their concrete literals (the erasure step). After
    // this, the body carries no symbolic const, so the verifier sees only
    // concrete values.
    if !const_subst.is_empty() {
        use crate::visitor::MutVisitor;
        let mut substitutor = ConstValueSubstitutor {
            subst: const_subst,
            shadowed: Vec::new(),
        };
        substitutor.visit_block(&mut body);
    }
    FunctionDef {
        category: func.category,
        name: spec_name,
        type_params: Vec::new(),
        const_params: Vec::new(),
        params,
        return_type,
        guard: func.guard.clone(),
        body,
        ephemeral: func.ephemeral,
        signed: func.signed,
        span: func.span,
    }
}

/// Substitute type parameters inside a Block. Walks statements and
/// the trailing expression.
fn subst_in_block(block: &Block, subst: &BTreeMap<String, TypeExpr>) -> Block {
    Block {
        stmts: block
            .stmts
            .iter()
            .map(|s| subst_in_stmt(s, subst))
            .collect(),
        tail_expr: block
            .tail_expr
            .as_ref()
            .map(|e| Box::new(subst_in_expr(e, subst))),
        span: block.span,
    }
}

fn subst_in_stmt(stmt: &Stmt, subst: &BTreeMap<String, TypeExpr>) -> Stmt {
    match stmt {
        Stmt::Let(l) => Stmt::Let(LetStmt {
            pattern: l.pattern.clone(),
            type_expr: l.type_expr.as_ref().map(|t| subst_type_expr(t, subst)),
            value: subst_in_expr(&l.value, subst),
            span: l.span,
        }),
        Stmt::For(f) => Stmt::For(ForStmt {
            var: f.var.clone(),
            iterable: subst_in_iterable(&f.iterable, subst),
            body: subst_in_block(&f.body, subst),
            span: f.span,
        }),
        Stmt::Break(span) => Stmt::Break(*span),
        Stmt::DataFieldAssign {
            data_name,
            field,
            value,
            span,
        } => Stmt::DataFieldAssign {
            data_name: data_name.clone(),
            field: field.clone(),
            value: subst_in_expr(value, subst),
            span: *span,
        },
        Stmt::DataFieldIndexAssign {
            data_name,
            field,
            indices,
            value,
            span,
        } => Stmt::DataFieldIndexAssign {
            data_name: data_name.clone(),
            field: field.clone(),
            indices: indices.iter().map(|e| subst_in_expr(e, subst)).collect(),
            value: subst_in_expr(value, subst),
            span: *span,
        },
        Stmt::Expr(e) => Stmt::Expr(subst_in_expr(e, subst)),
        Stmt::Assert {
            cond,
            message,
            span,
        } => Stmt::Assert {
            cond: subst_in_expr(cond, subst),
            message: message.clone(),
            span: *span,
        },
    }
}

fn subst_in_iterable(it: &Iterable, subst: &BTreeMap<String, TypeExpr>) -> Iterable {
    match it {
        Iterable::Range(start, end) => Iterable::Range(
            Box::new(subst_in_expr(start, subst)),
            Box::new(subst_in_expr(end, subst)),
        ),
        Iterable::Expr(e) => Iterable::Expr(Box::new(subst_in_expr(e, subst))),
    }
}

fn subst_in_expr(expr: &Expr, subst: &BTreeMap<String, TypeExpr>) -> Expr {
    match expr {
        Expr::Literal { value, span } => Expr::Literal {
            value: value.clone(),
            span: *span,
        },
        Expr::Ident { name, span } => Expr::Ident {
            name: name.clone(),
            span: *span,
        },
        Expr::BinOp {
            op,
            left,
            right,
            span,
        } => Expr::BinOp {
            op: *op,
            left: Box::new(subst_in_expr(left, subst)),
            right: Box::new(subst_in_expr(right, subst)),
            span: *span,
        },
        Expr::UnaryOp { op, operand, span } => Expr::UnaryOp {
            op: *op,
            operand: Box::new(subst_in_expr(operand, subst)),
            span: *span,
        },
        Expr::Call {
            name,
            args,
            const_args,
            span,
        } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|a| subst_in_expr(a, subst)).collect(),
            const_args: const_args.clone(),
            span: *span,
        },
        Expr::Pipeline {
            left,
            func,
            args,
            span,
        } => Expr::Pipeline {
            left: Box::new(subst_in_expr(left, subst)),
            func: func.clone(),
            args: args.iter().map(|a| subst_in_expr(a, subst)).collect(),
            span: *span,
        },
        Expr::Yield { value, span } => Expr::Yield {
            value: Box::new(subst_in_expr(value, subst)),
            span: *span,
        },
        Expr::If {
            condition,
            then_block,
            else_block,
            span,
        } => Expr::If {
            condition: Box::new(subst_in_expr(condition, subst)),
            then_block: subst_in_block(then_block, subst),
            else_block: else_block.as_ref().map(|b| subst_in_block(b, subst)),
            span: *span,
        },
        Expr::Match {
            scrutinee,
            arms,
            span,
        } => Expr::Match {
            scrutinee: Box::new(subst_in_expr(scrutinee, subst)),
            arms: arms
                .iter()
                .map(|a| MatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.as_ref().map(|g| subst_in_expr(g, subst)),
                    expr: subst_in_expr(&a.expr, subst),
                    span: a.span,
                })
                .collect(),
            span: *span,
        },
        Expr::Loop { body, span } => Expr::Loop {
            body: subst_in_block(body, subst),
            span: *span,
        },
        Expr::FieldAccess {
            object,
            field,
            span,
        } => Expr::FieldAccess {
            object: Box::new(subst_in_expr(object, subst)),
            field: field.clone(),
            span: *span,
        },
        Expr::MethodCall {
            receiver,
            method,
            args,
            span,
        } => Expr::MethodCall {
            receiver: Box::new(subst_in_expr(receiver, subst)),
            method: method.clone(),
            args: args.iter().map(|a| subst_in_expr(a, subst)).collect(),
            span: *span,
        },
        Expr::TupleIndex {
            object,
            index,
            span,
        } => Expr::TupleIndex {
            object: Box::new(subst_in_expr(object, subst)),
            index: *index,
            span: *span,
        },
        Expr::ArrayIndex {
            object,
            index,
            span,
        } => Expr::ArrayIndex {
            object: Box::new(subst_in_expr(object, subst)),
            index: Box::new(subst_in_expr(index, subst)),
            span: *span,
        },
        Expr::StructInit {
            name,
            fields,
            const_args,
            span,
        } => Expr::StructInit {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|f| FieldInit {
                    name: f.name.clone(),
                    value: subst_in_expr(&f.value, subst),
                    span: f.span,
                })
                .collect(),
            const_args: const_args.clone(),
            span: *span,
        },
        Expr::EnumVariant {
            enum_name,
            variant,
            args,
            const_args,
            span,
        } => Expr::EnumVariant {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            args: args.iter().map(|a| subst_in_expr(a, subst)).collect(),
            const_args: const_args.clone(),
            span: *span,
        },
        Expr::ArrayLiteral { elements, span } => Expr::ArrayLiteral {
            elements: elements.iter().map(|e| subst_in_expr(e, subst)).collect(),
            span: *span,
        },
        Expr::TupleLiteral { elements, span } => Expr::TupleLiteral {
            elements: elements.iter().map(|e| subst_in_expr(e, subst)).collect(),
            span: *span,
        },
        Expr::Cast { expr, target, span } => Expr::Cast {
            expr: Box::new(subst_in_expr(expr, subst)),
            target: subst_type_expr(target, subst),
            span: *span,
        },
        Expr::Placeholder { span } => Expr::Placeholder { span: *span },
        Expr::Closure {
            params,
            return_type,
            body,
            span,
        } => Expr::Closure {
            params: params
                .iter()
                .map(|p| Param {
                    pattern: p.pattern.clone(),
                    type_expr: p.type_expr.as_ref().map(|t| subst_type_expr(t, subst)),
                    span: p.span,
                })
                .collect(),
            return_type: return_type.as_ref().map(|t| subst_type_expr(t, subst)),
            body: subst_in_block(body, subst),
            span: *span,
        },
        Expr::ClosureRef {
            name,
            captures,
            recursive,
            span,
        } => Expr::ClosureRef {
            name: name.clone(),
            captures: captures.clone(),
            recursive: *recursive,
            span: *span,
        },
        Expr::Checked {
            op_expr,
            arms,
            span,
        } => Expr::Checked {
            op_expr: Box::new(subst_in_expr(op_expr, subst)),
            arms: arms
                .iter()
                .map(|arm| crate::ast::CheckedArm {
                    kind: arm.kind.clone(),
                    guard: arm.guard.as_ref().map(|g| subst_in_expr(g, subst)),
                    body: subst_in_expr(&arm.body, subst),
                    span: arm.span,
                })
                .collect(),
            span: *span,
        },
        Expr::SaturateMax { span } => Expr::SaturateMax { span: *span },
        Expr::SaturateMin { span } => Expr::SaturateMin { span: *span },
        Expr::Classify {
            value,
            labels,
            span,
        } => Expr::Classify {
            value: Box::new(subst_in_expr(value, subst)),
            labels: labels.clone(),
            span: *span,
        },
        Expr::Declassify {
            value,
            labels,
            span,
        } => Expr::Declassify {
            value: Box::new(subst_in_expr(value, subst)),
            labels: labels.clone(),
            span: *span,
        },
    }
}

/// AST visitor that rewrites generic function call sites to their
/// specialized monomorphic counterparts. Walks the program in
/// post-order so nested generic calls are specialized bottom-up.
/// Records local variable types as it descends through `let`
/// bindings so subsequent call sites that reference those locals can
/// infer concrete type arguments.
struct CallSpecializer<'a> {
    generics: &'a BTreeMap<String, FunctionDef>,
    locals: &'a mut BTreeMap<String, TypeExpr>,
    specs: &'a mut BTreeMap<(String, String), String>,
    new_functions: &'a mut Vec<FunctionDef>,
    fn_returns: &'a BTreeMap<String, TypeExpr>,
    struct_table: &'a BTreeMap<String, StructDef>,
}

impl crate::visitor::MutVisitor for CallSpecializer<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Let(l) = stmt {
            self.visit_expr(&mut l.value);
            // Record local type for subsequent type inference at call
            // sites that reference this binding.
            if let Pattern::Variable(name, _) = &l.pattern
                && let Some(t) = l.type_expr.clone().or_else(|| {
                    infer_arg_type(
                        &l.value,
                        self.locals,
                        self.fn_returns,
                        Some(self.struct_table),
                    )
                })
            {
                self.locals.insert(name.clone(), t);
            }
            return;
        }
        self.walk_stmt(stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        // Recurse into children first so nested generic calls are
        // specialized bottom-up.
        self.walk_expr(expr);
        // Then check this node for a generic call to specialize.
        let Expr::Call {
            name,
            args,
            const_args,
            ..
        } = expr
        else {
            return;
        };
        let Some(generic_func) = self.generics.get(name) else {
            return;
        };
        // Infer the type arguments from the call's actual argument
        // types. Positional correspondence: the i-th type parameter
        // is bound by the first parameter declared as `Named(tp.name)`.
        let mut type_args: Vec<TypeExpr> = Vec::new();
        for tp in &generic_func.type_params {
            let mut inferred: Option<TypeExpr> = None;
            for (param_idx, param) in generic_func.params.iter().enumerate() {
                if let Some(TypeExpr::Named(n, _, _, _)) = &param.type_expr
                    && *n == tp.name
                    && let Some(arg) = args.get(param_idx)
                    && let Some(t) =
                        infer_arg_type(arg, self.locals, self.fn_returns, Some(self.struct_table))
                {
                    inferred = Some(t);
                    break;
                }
            }
            match inferred {
                Some(t) => type_args.push(t),
                None => return,
            }
        }
        if type_args.len() != generic_func.type_params.len() {
            return;
        }
        // Evaluate the explicit const arguments. Post-substitution they
        // are ground (literals), so an empty parameter map suffices; a
        // still-symbolic const argument defers specialization to a later
        // fixed-point pass once the enclosing function is specialized.
        let empty: BTreeMap<String, i64> = BTreeMap::new();
        let mut const_values: Vec<i64> = Vec::new();
        for ca in const_args.iter() {
            match eval_const_expr(ca, &empty) {
                Some(v) => const_values.push(v),
                None => return,
            }
        }
        if const_values.len() != generic_func.const_params.len() {
            return;
        }
        let key_args: Vec<String> = type_args.iter().map(type_arg_canonical).collect();
        let mut canonical = key_args.join(",");
        if !const_values.is_empty() {
            use alloc::string::ToString;
            let cvs: Vec<String> = const_values.iter().map(|v| v.to_string()).collect();
            canonical.push_str(";c=");
            canonical.push_str(&cvs.join(","));
        }
        let cache_key = (name.clone(), canonical);
        let spec_name = if let Some(existing) = self.specs.get(&cache_key) {
            existing.clone()
        } else {
            let spec_name = mangle_with_consts(name, &type_args, &const_values);
            let specialized =
                specialize_function(generic_func, &type_args, &const_values, spec_name.clone());
            self.specs.insert(cache_key, spec_name.clone());
            self.new_functions.push(specialized);
            spec_name
        };
        if let Expr::Call {
            name, const_args, ..
        } = expr
        {
            *name = spec_name;
            // The const arguments are consumed into the specialization;
            // the rewritten call targets a non-const-generic function.
            const_args.clear();
        }
    }
}
