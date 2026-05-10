//! Compile-time monomorphization for generic functions.
//!
//! After type checking and before compilation, this pass walks the
//! program's call graph and generates a specialized
//! [`FunctionDef`] per `(function, type_args)` pair encountered.
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
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::ast::*;

/// Apply monomorphization to a program. Returns a new program with
/// specialized functions added and call sites rewritten.
pub fn monomorphize(program: Program) -> Program {
    let mut program = program;

    // Build a map from function name to FunctionDef for lookup.
    // Generic functions remain in this map; specialization clones
    // them.
    let generics: BTreeMap<String, FunctionDef> = program
        .functions
        .iter()
        .filter(|f| !f.type_params.is_empty())
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
            if func.type_params.is_empty() {
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
            for new_fn in &new_functions[len_before..] {
                // The synthetic name is `origin__type_args`. Recover
                // the origin by splitting on the first `__`.
                let origin = new_fn
                    .name
                    .split("__")
                    .next()
                    .unwrap_or(&new_fn.name)
                    .to_string();
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
    program
        .functions
        .retain(|f| !specialized_origins.contains(&f.name));

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
    program
}

/// Generic enum specialization pass. See [`specialize_structs`] for
/// the analogous struct pass. The mechanics mirror struct
/// specialization, with variant payload types in place of struct
/// field types.
fn specialize_enums(mut program: Program, fn_returns: &BTreeMap<String, TypeExpr>) -> Program {
    use crate::visitor::MutVisitor;
    let generic_enums: BTreeMap<String, EnumDef> = program
        .types
        .iter()
        .filter_map(|td| match td {
            TypeDef::Enum(e) if !e.type_params.is_empty() => Some((e.name.clone(), e.clone())),
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
        // Then check this node for a generic EnumVariant to specialize.
        let Expr::EnumVariant {
            enum_name,
            variant,
            args,
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
        let mut type_args: Vec<TypeExpr> = Vec::new();
        for tp in &enum_def.type_params {
            let mut inferred: Option<TypeExpr> = None;
            for (i, decl_ty) in decl_variant.fields.iter().enumerate() {
                if let TypeExpr::Named(n, _, _) = decl_ty
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
        let key_args: Vec<String> = type_args.iter().map(type_arg_canonical).collect();
        let canonical = key_args.join(",");
        let cache_key = (enum_name.clone(), canonical);
        let spec_name = if let Some(existing) = self.specs.get(&cache_key) {
            existing.clone()
        } else {
            let spec_name = mangle_struct(enum_name, &type_args);
            let specialized = specialize_enum(enum_def, &type_args, spec_name.clone());
            self.specs.insert(cache_key, spec_name.clone());
            self.new_enums.push(specialized);
            spec_name
        };
        if let Expr::EnumVariant { enum_name, .. } = expr {
            *enum_name = spec_name;
        }
    }
}

fn specialize_enum(enum_def: &EnumDef, type_args: &[TypeExpr], spec_name: String) -> EnumDef {
    let mut subst: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for (tp, arg) in enum_def.type_params.iter().zip(type_args.iter()) {
        subst.insert(tp.name.clone(), arg.clone());
    }
    let variants: Vec<VariantDecl> = enum_def
        .variants
        .iter()
        .map(|v| VariantDecl {
            name: v.name.clone(),
            fields: v
                .fields
                .iter()
                .map(|t| subst_type_expr(t, &subst))
                .collect(),
            span: v.span,
        })
        .collect();
    EnumDef {
        name: spec_name,
        type_params: Vec::new(),
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
            TypeDef::Struct(s) if !s.type_params.is_empty() => Some((s.name.clone(), s.clone())),
            _ => None,
        })
        .collect();
    if generic_structs.is_empty() {
        return program;
    }
    let mut struct_specs: BTreeMap<(String, String), String> = BTreeMap::new();
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
            new_structs: &mut new_structs,
            fn_returns,
        };
        visitor.visit_block(&mut func.body);
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
        let Expr::StructInit { name, fields, .. } = expr else {
            return;
        };
        let Some(struct_def) = self.generic_structs.get(name) else {
            return;
        };
        let mut type_args: Vec<TypeExpr> = Vec::new();
        for tp in &struct_def.type_params {
            let mut inferred: Option<TypeExpr> = None;
            for decl_field in &struct_def.fields {
                if let TypeExpr::Named(n, _, _) = &decl_field.type_expr
                    && *n == tp.name
                    && let Some(init) = fields.iter().find(|f| f.name == decl_field.name)
                    && let Some(t) = infer_arg_type(&init.value, self.locals, self.fn_returns, None)
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
        if type_args.len() != struct_def.type_params.len() {
            return;
        }
        let key_args: Vec<String> = type_args.iter().map(type_arg_canonical).collect();
        let canonical = key_args.join(",");
        let cache_key = (name.clone(), canonical);
        let spec_name = if let Some(existing) = self.specs.get(&cache_key) {
            existing.clone()
        } else {
            let spec_name = mangle_struct(name, &type_args);
            let specialized = specialize_struct(struct_def, &type_args, spec_name.clone());
            self.specs.insert(cache_key, spec_name.clone());
            self.new_structs.push(specialized);
            spec_name
        };
        if let Expr::StructInit { name, .. } = expr {
            *name = spec_name;
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

fn specialize_struct(
    struct_def: &StructDef,
    type_args: &[TypeExpr],
    spec_name: String,
) -> StructDef {
    let mut subst: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for (tp, arg) in struct_def.type_params.iter().zip(type_args.iter()) {
        subst.insert(tp.name.clone(), arg.clone());
    }
    let fields: Vec<FieldDecl> = struct_def
        .fields
        .iter()
        .map(|f| FieldDecl {
            name: f.name.clone(),
            type_expr: subst_type_expr(&f.type_expr, &subst),
            span: f.span,
        })
        .collect();
    StructDef {
        name: spec_name,
        type_params: Vec::new(),
        fields,
        span: struct_def.span,
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

/// Canonical short string representation for a type argument used in
/// mangling. Uses the head name only, which is sufficient because
/// monomorphization happens after type checking has validated the
/// concrete type.
fn type_arg_canonical(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Prim(p, _) => match p {
            PrimType::I64 => "i64".to_string(),
            PrimType::F64 => "f64".to_string(),
            PrimType::Bool => "bool".to_string(),
            PrimType::KString => "String".to_string(),
        },
        TypeExpr::Unit(_) => "unit".to_string(),
        TypeExpr::Named(n, args, _) => {
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
        TypeExpr::Option(inner, _) => format!("opt_{}", type_arg_canonical(inner)),
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
            Literal::Int(_) => TypeExpr::Prim(PrimType::I64, *span),
            Literal::Float(_) => TypeExpr::Prim(PrimType::F64, *span),
            Literal::Bool(_) => TypeExpr::Prim(PrimType::Bool, *span),
            Literal::String(_) => TypeExpr::Prim(PrimType::KString, *span),
            Literal::Unit => TypeExpr::Unit(*span),
        }),
        Expr::Ident { name, .. } => locals.get(name).cloned(),
        Expr::StructInit { name, span, .. } => {
            Some(TypeExpr::Named(name.clone(), Vec::new(), *span))
        }
        Expr::EnumVariant {
            enum_name, span, ..
        } => Some(TypeExpr::Named(enum_name.clone(), Vec::new(), *span)),
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
            Some(TypeExpr::Array(
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
                TypeExpr::Named(name, args, _) => (name, args),
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
                if let TypeExpr::Named(field_name, field_args, _) = &field_decl.type_expr
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
                UnaryOp::Neg => infer_arg_type(operand, locals, fn_returns, structs),
                UnaryOp::Not => Some(TypeExpr::Prim(PrimType::Bool, operand.span())),
            }
        }
        Expr::BinOp { op, left, span, .. } => {
            // Arithmetic operators preserve the operand types under
            // the type checker's existing same-type unification.
            // Comparison and logical operators return Bool.
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    infer_arg_type(left, locals, fn_returns, structs)
                }
                BinOp::Eq
                | BinOp::NotEq
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::LtEq
                | BinOp::GtEq
                | BinOp::And
                | BinOp::Or => Some(TypeExpr::Prim(PrimType::Bool, *span)),
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
            PrimType::I64 => "i64".to_string(),
            PrimType::F64 => "f64".to_string(),
            PrimType::Bool => "bool".to_string(),
            PrimType::KString => "String".to_string(),
        },
        TypeExpr::Unit(_) => "()".to_string(),
        TypeExpr::Named(name, _, _) => name.clone(),
        TypeExpr::Tuple(_, _) => "tuple".to_string(),
        TypeExpr::Array(_, _, _) => "array".to_string(),
        TypeExpr::Option(_, _) => "Option".to_string(),
    }
}

/// Substitute type-parameter names with concrete type expressions
/// inside a `TypeExpr`. Type parameters not present in `subst` are
/// preserved. Matches by the `Named` form's identifier; primitives
/// and structural types recurse.
fn subst_type_expr(t: &TypeExpr, subst: &BTreeMap<String, TypeExpr>) -> TypeExpr {
    match t {
        TypeExpr::Prim(_, _) | TypeExpr::Unit(_) => t.clone(),
        TypeExpr::Named(name, args, span) => {
            if args.is_empty()
                && let Some(replacement) = subst.get(name)
            {
                return replacement.clone();
            }
            TypeExpr::Named(
                name.clone(),
                args.iter().map(|a| subst_type_expr(a, subst)).collect(),
                *span,
            )
        }
        TypeExpr::Tuple(items, span) => TypeExpr::Tuple(
            items.iter().map(|t| subst_type_expr(t, subst)).collect(),
            *span,
        ),
        TypeExpr::Array(elem, n, span) => {
            TypeExpr::Array(Box::new(subst_type_expr(elem, subst)), *n, *span)
        }
        TypeExpr::Option(inner, span) => {
            TypeExpr::Option(Box::new(subst_type_expr(inner, subst)), *span)
        }
    }
}

/// Specialize a generic function with concrete type arguments. Clones
/// the function and substitutes type parameters in param types,
/// return type, and the body.
fn specialize_function(
    func: &FunctionDef,
    type_args: &[TypeExpr],
    spec_name: String,
) -> FunctionDef {
    let mut subst: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for (tp, arg) in func.type_params.iter().zip(type_args.iter()) {
        subst.insert(tp.name.clone(), arg.clone());
    }
    let params: Vec<Param> = func
        .params
        .iter()
        .map(|p| Param {
            pattern: p.pattern.clone(),
            type_expr: p.type_expr.as_ref().map(|t| subst_type_expr(t, &subst)),
            span: p.span,
        })
        .collect();
    let return_type = subst_type_expr(&func.return_type, &subst);
    let body = subst_in_block(&func.body, &subst);
    FunctionDef {
        category: func.category,
        name: spec_name,
        type_params: Vec::new(),
        params,
        return_type,
        guard: func.guard.clone(),
        body,
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
        Stmt::Expr(e) => Stmt::Expr(subst_in_expr(e, subst)),
    }
}

fn subst_in_iterable(it: &Iterable, subst: &BTreeMap<String, TypeExpr>) -> Iterable {
    match it {
        Iterable::Range(start, end) => Iterable::Range(
            Box::new(subst_in_expr(start, subst)),
            Box::new(subst_in_expr(end, subst)),
        ),
        Iterable::Expr(e) => Iterable::Expr(subst_in_expr(e, subst)),
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
        Expr::Call { name, args, span } => Expr::Call {
            name: name.clone(),
            args: args.iter().map(|a| subst_in_expr(a, subst)).collect(),
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
        Expr::StructInit { name, fields, span } => Expr::StructInit {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|f| FieldInit {
                    name: f.name.clone(),
                    value: subst_in_expr(&f.value, subst),
                    span: f.span,
                })
                .collect(),
            span: *span,
        },
        Expr::EnumVariant {
            enum_name,
            variant,
            args,
            span,
        } => Expr::EnumVariant {
            enum_name: enum_name.clone(),
            variant: variant.clone(),
            args: args.iter().map(|a| subst_in_expr(a, subst)).collect(),
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
        let Expr::Call { name, args, .. } = expr else {
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
                if let Some(TypeExpr::Named(n, _, _)) = &param.type_expr
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
        let key_args: Vec<String> = type_args.iter().map(type_arg_canonical).collect();
        let canonical = key_args.join(",");
        let cache_key = (name.clone(), canonical);
        let spec_name = if let Some(existing) = self.specs.get(&cache_key) {
            existing.clone()
        } else {
            let spec_name = mangle(name, &type_args);
            let specialized = specialize_function(generic_func, &type_args, spec_name.clone());
            self.specs.insert(cache_key, spec_name.clone());
            self.new_functions.push(specialized);
            spec_name
        };
        if let Expr::Call { name, .. } = expr {
            *name = spec_name;
        }
    }
}
