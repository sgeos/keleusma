//! Static type checker for Keleusma source programs.
//!
//! Runs after parsing and before bytecode emission. Catches type errors
//! at compile time that would otherwise surface at runtime through
//! [`crate::vm::VmError::TypeError`]. The pass is deliberately narrow.
//! It checks declared signatures and explicit annotations against
//! computed expression types. It does not perform Hindley-Milner
//! inference (B1) or check against native function signatures because
//! natives are registered at runtime through `Vm::register_*`.
//!
//! Coverage. The pass currently catches the following at compile time.
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
//!
//! Out of scope for this pass.
//!
//! - Match arm exhaustiveness. The runtime detects nonexhaustive
//!   matches through the `NoMatch` error.
//! - Detailed enum variant field validation beyond variant existence.
//! - Native function call types.
//! - Pattern type checking against the scrutinee. Patterns are accepted
//!   structurally and the runtime detects mismatches.

extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
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
    /// Sentinel for an expression whose type cannot be determined
    /// without inference (e.g., unannotated let bound to a `match`
    /// expression returning a variable). Treated as compatible with
    /// anything in this MVP pass.
    Unknown,
}

impl Type {
    fn from_expr(expr: &TypeExpr, defined_types: &BTreeMap<String, TypeKind>) -> Type {
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
                    .map(|t| Type::from_expr(t, defined_types))
                    .collect(),
            ),
            TypeExpr::Array(elem, len, _) => {
                Type::Array(Box::new(Type::from_expr(elem, defined_types)), *len)
            }
            TypeExpr::Option(inner, _) => {
                Type::Option(Box::new(Type::from_expr(inner, defined_types)))
            }
            TypeExpr::Named(name, _) => match defined_types.get(name) {
                Some(TypeKind::Struct) => Type::Struct(name.clone()),
                Some(TypeKind::Enum) => Type::Enum(name.clone()),
                None => Type::Opaque(name.clone()),
            },
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
            Type::Unknown => "<unknown>".to_string(),
        }
    }
}

/// What kind of type a name refers to.
#[derive(Debug, Clone, Copy)]
enum TypeKind {
    Struct,
    Enum,
}

/// Function signature derived from an AST function definition.
#[derive(Debug, Clone)]
struct FnSig {
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

/// The typing context tracks declarations and the local scope chain.
struct Ctx {
    types: BTreeMap<String, TypeKind>,
    structs: BTreeMap<String, BTreeMap<String, Type>>,
    enums: BTreeMap<String, BTreeMap<String, Vec<Type>>>,
    functions: BTreeMap<String, FnSig>,
    /// Data block field types, keyed by data name then field name.
    data: BTreeMap<String, BTreeMap<String, Type>>,
    /// Stack of local variable scopes. Inner scopes shadow outer.
    locals: Vec<BTreeMap<String, Type>>,
    /// Return type of the function currently being checked.
    current_return: Option<Type>,
}

impl Ctx {
    fn new() -> Self {
        Self {
            types: BTreeMap::new(),
            structs: BTreeMap::new(),
            enums: BTreeMap::new(),
            functions: BTreeMap::new(),
            data: BTreeMap::new(),
            locals: Vec::new(),
            current_return: None,
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
}

/// Check that two types are compatible. The MVP rule is structural
/// equality with `Unknown` matching anything and Opaque types matching
/// only themselves by name.
fn types_compatible(a: &Type, b: &Type) -> bool {
    if matches!(a, Type::Unknown) || matches!(b, Type::Unknown) {
        return true;
    }
    a == b
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

    // Pass 1c. Build function signatures.
    for func in &program.functions {
        let params: Vec<Type> = func
            .params
            .iter()
            .map(|p| match &p.type_expr {
                Some(t) => Type::from_expr(t, &ctx.types),
                None => Type::Unknown,
            })
            .collect();
        let return_type = Type::from_expr(&func.return_type, &ctx.types);
        ctx.functions.insert(
            func.name.clone(),
            FnSig {
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
    ctx.push_scope();
    let return_type = ctx
        .functions
        .get(&func.name)
        .map(|s| s.return_type.clone())
        .unwrap_or(Type::Unknown);
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
    if !types_compatible(&body_type, &return_type) {
        ctx.pop_scope();
        ctx.current_return = None;
        return Err(TypeError::new(
            format!(
                "function `{}` returns {} but body produces {}",
                func.name,
                return_type.display(),
                body_type.display()
            ),
            func.body.span,
        ));
    }
    ctx.pop_scope();
    ctx.current_return = None;
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
                    bind_pattern(ctx, pat, Type::Unknown);
                }
            }
        }
        // For literal, struct, and enum patterns we do not introduce
        // bindings here. Variables nested inside struct or enum
        // patterns are bound during match-arm checking.
        _ => {}
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
                    if !types_compatible(&declared, &value_ty) {
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
                    if !types_compatible(&s, &Type::I64) || !types_compatible(&e, &Type::I64) {
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
            if !types_compatible(&declared, &value_ty) {
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
                    } else if matches!(lt, Type::Unknown) || matches!(rt, Type::Unknown) {
                        Ok(Type::Unknown)
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
                    } else if matches!(lt, Type::Unknown) || matches!(rt, Type::Unknown) {
                        Ok(Type::Unknown)
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
                    if !types_compatible(&lt, &rt) {
                        return Err(TypeError::new(
                            format!("cannot compare {} and {}", lt.display(), rt.display()),
                            *span,
                        ));
                    }
                    Ok(Type::Bool)
                }
                BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
                    if !types_compatible(&lt, &rt) {
                        return Err(TypeError::new(
                            format!("cannot order {} and {}", lt.display(), rt.display()),
                            *span,
                        ));
                    }
                    Ok(Type::Bool)
                }
                BinOp::And | BinOp::Or => {
                    if !types_compatible(&lt, &Type::Bool) || !types_compatible(&rt, &Type::Bool) {
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
                    Type::I64 | Type::F64 | Type::Unknown => Ok(ty),
                    other => Err(TypeError::new(
                        format!("cannot negate {}", other.display()),
                        *span,
                    )),
                },
                UnaryOp::Not => {
                    if !types_compatible(&ty, &Type::Bool) {
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
            // compile-time signature in this MVP. Treat unknown names
            // as natives and accept any argument types.
            let sig = match ctx.functions.get(name).cloned() {
                Some(s) => s,
                None => return Ok(Type::Unknown),
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
            for (arg, param_ty) in args.iter().zip(sig.params.iter()) {
                let arg_ty = type_of_expr(ctx, arg)?;
                if !types_compatible(&arg_ty, param_ty) {
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
            Ok(sig.return_type)
        }
        Expr::Pipeline {
            left, func, args, ..
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
                    && !types_compatible(&left_ty, first_param)
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
                    if !types_compatible(&arg_ty, param_ty) {
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
            } else {
                // Native pipeline target. Accept.
                for arg in args {
                    let _ = type_of_expr(ctx, arg)?;
                }
                Ok(Type::Unknown)
            }
        }
        Expr::Yield { value, .. } => {
            let _ = type_of_expr(ctx, value)?;
            // Yield's expression value (received from host on resume)
            // cannot be statically typed without dialogue annotations.
            Ok(Type::Unknown)
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            span,
        } => {
            let cond_ty = type_of_expr(ctx, condition)?;
            if !types_compatible(&cond_ty, &Type::Bool) {
                return Err(TypeError::new(
                    format!("if condition must be bool, got {}", cond_ty.display()),
                    *span,
                ));
            }
            let then_ty = type_of_block(ctx, then_block)?;
            match else_block {
                Some(b) => {
                    let else_ty = type_of_block(ctx, b)?;
                    if !types_compatible(&then_ty, &else_ty) {
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
            scrutinee, arms, ..
        } => {
            let _ = type_of_expr(ctx, scrutinee)?;
            // Type the body of each arm. The arm bodies must agree.
            let mut common: Option<Type> = None;
            for arm in arms {
                ctx.push_scope();
                // Bind any variables introduced by the pattern. Without
                // detailed pattern checking we bind to Unknown.
                bind_pattern(ctx, &arm.pattern, Type::Unknown);
                let arm_ty = type_of_expr(ctx, &arm.expr)?;
                ctx.pop_scope();
                match &common {
                    None => common = Some(arm_ty),
                    Some(c) => {
                        if !types_compatible(c, &arm_ty) {
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
                Type::Unknown => Ok(Type::Unknown),
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
                Type::Unknown => Ok(Type::Unknown),
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
            if !types_compatible(&idx_ty, &Type::I64) {
                return Err(TypeError::new(
                    format!("array index must be i64, got {}", idx_ty.display()),
                    *span,
                ));
            }
            match obj_ty {
                Type::Array(inner, _) => Ok(*inner),
                Type::Unknown => Ok(Type::Unknown),
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
                if !types_compatible(&value_ty, declared) {
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
                        if !types_compatible(&arg_ty, expected) {
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
                        if !types_compatible(et, &t) {
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
                Box::new(elem_ty.unwrap_or(Type::Unknown)),
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
        Expr::Placeholder { .. } => Ok(Type::Unknown),
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
}
