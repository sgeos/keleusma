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
    // appearing in generic call arguments.
    let mut fn_returns: BTreeMap<String, TypeExpr> = BTreeMap::new();
    for f in &program.functions {
        fn_returns.insert(f.name.clone(), f.return_type.clone());
    }

    // Local-type information for argument-type inference.
    let mut local_types: BTreeMap<String, TypeExpr> = BTreeMap::new();
    // Specializations generated. Keyed on (function, type_args
    // canonical encoding). Value is the mangled specialized name.
    let mut specs: BTreeMap<(String, String), String> = BTreeMap::new();
    // New specialized functions to add to the program.
    let mut new_functions: Vec<FunctionDef> = Vec::new();

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
            rewrite_block(
                &mut func.body,
                &generics,
                &mut local_types,
                &mut specs,
                &mut new_functions,
                &fn_returns,
            );
        }
    }

    // Polymorphic recursion guard. The fixed-point loop below
    // bounds the number of specializations to prevent unbounded
    // expansion when a generic function calls itself with type
    // arguments derived from its own type parameters in a way that
    // grows. The bound is generous; legitimate programs reach a
    // fixed point well below it.
    const SPECIALIZATION_LIMIT: usize = 1024;

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
        let mut body_clone = new_functions[idx].body.clone();
        rewrite_block(
            &mut body_clone,
            &generics,
            &mut local_types,
            &mut specs,
            &mut new_functions,
            &fn_returns,
        );
        new_functions[idx].body = body_clone;
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
    program
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
fn infer_arg_type(
    expr: &Expr,
    locals: &BTreeMap<String, TypeExpr>,
    fn_returns: &BTreeMap<String, TypeExpr>,
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
                .map(|e| infer_arg_type(e, locals, fn_returns))
                .collect();
            parts.map(|p| TypeExpr::Tuple(p, *span))
        }
        Expr::ArrayLiteral { elements, span } => {
            let elem = elements.first()?;
            let elem_ty = infer_arg_type(elem, locals, fn_returns)?;
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
            infer_arg_type(tail, locals, fn_returns).or_else(|| {
                else_block
                    .as_ref()
                    .and_then(|b| b.tail_expr.as_ref())
                    .and_then(|e| infer_arg_type(e, locals, fn_returns))
            })
        }
        Expr::Match { arms, .. } => {
            // All arms agree on type per the type checker. Use the
            // first arm's expression type.
            let first = arms.first()?;
            infer_arg_type(&first.expr, locals, fn_returns)
        }
        Expr::FieldAccess { object, field, .. } => {
            // For a known local of a known struct, the field's type
            // could be looked up. The current `locals` map only
            // records the struct's nominal type. Without struct
            // field tables threaded here, return None for now.
            let _ = (object, field);
            None
        }
        _ => None,
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
            span,
        } => Expr::ClosureRef {
            name: name.clone(),
            captures: captures.clone(),
            span: *span,
        },
    }
}

/// Walk a block looking for generic call sites and rewrite them in
/// place to reference specialized functions. Records local variable
/// types as it descends through let bindings.
fn rewrite_block(
    block: &mut Block,
    generics: &BTreeMap<String, FunctionDef>,
    locals: &mut BTreeMap<String, TypeExpr>,
    specs: &mut BTreeMap<(String, String), String>,
    new_functions: &mut Vec<FunctionDef>,
    fn_returns: &BTreeMap<String, TypeExpr>,
) {
    for stmt in &mut block.stmts {
        rewrite_stmt(stmt, generics, locals, specs, new_functions, fn_returns);
    }
    if let Some(e) = block.tail_expr.as_mut() {
        rewrite_expr(e, generics, locals, specs, new_functions, fn_returns);
    }
}

fn rewrite_stmt(
    stmt: &mut Stmt,
    generics: &BTreeMap<String, FunctionDef>,
    locals: &mut BTreeMap<String, TypeExpr>,
    specs: &mut BTreeMap<(String, String), String>,
    new_functions: &mut Vec<FunctionDef>,
    fn_returns: &BTreeMap<String, TypeExpr>,
) {
    match stmt {
        Stmt::Let(l) => {
            rewrite_expr(
                &mut l.value,
                generics,
                locals,
                specs,
                new_functions,
                fn_returns,
            );
            // Record local type for subsequent type inference at call
            // sites that reference this binding.
            if let Pattern::Variable(name, _) = &l.pattern
                && let Some(t) = l
                    .type_expr
                    .clone()
                    .or_else(|| infer_arg_type(&l.value, locals, fn_returns))
            {
                locals.insert(name.clone(), t);
            }
        }
        Stmt::For(f) => {
            rewrite_iterable(
                &mut f.iterable,
                generics,
                locals,
                specs,
                new_functions,
                fn_returns,
            );
            rewrite_block(
                &mut f.body,
                generics,
                locals,
                specs,
                new_functions,
                fn_returns,
            );
        }
        Stmt::Break(_) => {}
        Stmt::DataFieldAssign { value, .. } => {
            rewrite_expr(value, generics, locals, specs, new_functions, fn_returns);
        }
        Stmt::Expr(e) => rewrite_expr(e, generics, locals, specs, new_functions, fn_returns),
    }
}

fn rewrite_iterable(
    it: &mut Iterable,
    generics: &BTreeMap<String, FunctionDef>,
    locals: &mut BTreeMap<String, TypeExpr>,
    specs: &mut BTreeMap<(String, String), String>,
    new_functions: &mut Vec<FunctionDef>,
    fn_returns: &BTreeMap<String, TypeExpr>,
) {
    match it {
        Iterable::Range(start, end) => {
            rewrite_expr(start, generics, locals, specs, new_functions, fn_returns);
            rewrite_expr(end, generics, locals, specs, new_functions, fn_returns);
        }
        Iterable::Expr(e) => rewrite_expr(e, generics, locals, specs, new_functions, fn_returns),
    }
}

fn rewrite_expr(
    expr: &mut Expr,
    generics: &BTreeMap<String, FunctionDef>,
    locals: &mut BTreeMap<String, TypeExpr>,
    specs: &mut BTreeMap<(String, String), String>,
    new_functions: &mut Vec<FunctionDef>,
    fn_returns: &BTreeMap<String, TypeExpr>,
) {
    // First descend so nested calls are rewritten before this one.
    match expr {
        Expr::BinOp { left, right, .. } => {
            rewrite_expr(left, generics, locals, specs, new_functions, fn_returns);
            rewrite_expr(right, generics, locals, specs, new_functions, fn_returns);
        }
        Expr::UnaryOp { operand, .. } => {
            rewrite_expr(operand, generics, locals, specs, new_functions, fn_returns);
        }
        Expr::Call { args, .. } => {
            for a in args.iter_mut() {
                rewrite_expr(a, generics, locals, specs, new_functions, fn_returns);
            }
        }
        Expr::Pipeline { left, args, .. } => {
            rewrite_expr(left, generics, locals, specs, new_functions, fn_returns);
            for a in args.iter_mut() {
                rewrite_expr(a, generics, locals, specs, new_functions, fn_returns);
            }
        }
        Expr::Yield { value, .. } => {
            rewrite_expr(value, generics, locals, specs, new_functions, fn_returns)
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            rewrite_expr(
                condition,
                generics,
                locals,
                specs,
                new_functions,
                fn_returns,
            );
            rewrite_block(
                then_block,
                generics,
                locals,
                specs,
                new_functions,
                fn_returns,
            );
            if let Some(b) = else_block.as_mut() {
                rewrite_block(b, generics, locals, specs, new_functions, fn_returns);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            rewrite_expr(
                scrutinee,
                generics,
                locals,
                specs,
                new_functions,
                fn_returns,
            );
            for arm in arms.iter_mut() {
                rewrite_expr(
                    &mut arm.expr,
                    generics,
                    locals,
                    specs,
                    new_functions,
                    fn_returns,
                );
            }
        }
        Expr::Loop { body, .. } => {
            rewrite_block(body, generics, locals, specs, new_functions, fn_returns)
        }
        Expr::FieldAccess { object, .. } => {
            rewrite_expr(object, generics, locals, specs, new_functions, fn_returns);
        }
        Expr::MethodCall { receiver, args, .. } => {
            rewrite_expr(receiver, generics, locals, specs, new_functions, fn_returns);
            for a in args.iter_mut() {
                rewrite_expr(a, generics, locals, specs, new_functions, fn_returns);
            }
        }
        Expr::TupleIndex { object, .. } => {
            rewrite_expr(object, generics, locals, specs, new_functions, fn_returns);
        }
        Expr::ArrayIndex { object, index, .. } => {
            rewrite_expr(object, generics, locals, specs, new_functions, fn_returns);
            rewrite_expr(index, generics, locals, specs, new_functions, fn_returns);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields.iter_mut() {
                rewrite_expr(
                    &mut f.value,
                    generics,
                    locals,
                    specs,
                    new_functions,
                    fn_returns,
                );
            }
        }
        Expr::EnumVariant { args, .. } => {
            for a in args.iter_mut() {
                rewrite_expr(a, generics, locals, specs, new_functions, fn_returns);
            }
        }
        Expr::ArrayLiteral { elements, .. } | Expr::TupleLiteral { elements, .. } => {
            for e in elements.iter_mut() {
                rewrite_expr(e, generics, locals, specs, new_functions, fn_returns);
            }
        }
        Expr::Cast { expr, .. } => {
            rewrite_expr(expr, generics, locals, specs, new_functions, fn_returns)
        }
        Expr::Closure { body, .. } => {
            rewrite_block(body, generics, locals, specs, new_functions, fn_returns);
        }
        Expr::ClosureRef { .. } => {}
        Expr::Literal { .. } | Expr::Ident { .. } | Expr::Placeholder { .. } => {}
    }

    // Now inspect this node for a generic call to specialize.
    if let Expr::Call {
        name,
        args,
        span: _,
    } = expr
        && let Some(generic_func) = generics.get(name)
    {
        // Infer the type arguments from the call's actual argument
        // types. The mapping uses positional correspondence: the
        // i-th type parameter is bound by the i-th argument's type.
        // This is a minimum-viable inference; full inference would
        // unify against all parameter positions and propagate
        // constraints. For now, handle cases where the i-th
        // parameter's declared type is exactly the i-th type
        // parameter.
        let mut type_args: Vec<TypeExpr> = Vec::new();
        for tp in &generic_func.type_params {
            // Find the first parameter whose declared type is
            // `Named(tp.name)` and infer from the corresponding
            // argument.
            let mut inferred: Option<TypeExpr> = None;
            for (param_idx, param) in generic_func.params.iter().enumerate() {
                if let Some(TypeExpr::Named(n, _, _)) = &param.type_expr
                    && *n == tp.name
                    && let Some(arg) = args.get(param_idx)
                    && let Some(t) = infer_arg_type(arg, locals, fn_returns)
                {
                    inferred = Some(t);
                    break;
                }
            }
            match inferred {
                Some(t) => type_args.push(t),
                None => return, // give up; leave the call generic
            }
        }
        if type_args.len() != generic_func.type_params.len() {
            return;
        }
        // Compute the canonical key for this specialization.
        let key_args: Vec<String> = type_args.iter().map(type_arg_canonical).collect();
        let canonical = key_args.join(",");
        let cache_key = (name.clone(), canonical);
        let spec_name = if let Some(existing) = specs.get(&cache_key) {
            existing.clone()
        } else {
            let spec_name = mangle(name, &type_args);
            let specialized = specialize_function(generic_func, &type_args, spec_name.clone());
            specs.insert(cache_key, spec_name.clone());
            new_functions.push(specialized);
            spec_name
        };
        // Rewrite the call's name to the specialized name.
        if let Expr::Call { name, .. } = expr {
            *name = spec_name;
        }
    }
}
