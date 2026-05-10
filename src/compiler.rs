extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::ast::*;
use crate::bytecode::*;
use crate::token::Span;

/// A compile-time error with a message and source location.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub message: String,
    pub span: Span,
}

/// Tracks a local variable during compilation.
struct Local {
    name: String,
    slot: u16,
    depth: u32,
    /// Declared or inferred type expression. Used by selected
    /// optimizations such as for-in iteration bound inference (P2)
    /// to resolve field access on locals to typed arrays.
    ty: Option<TypeExpr>,
}

/// Static type information collected from the AST and used by the
/// compiler for selected optimizations such as the for-in iteration
/// bound (P2). Independent of the type checker; the compiler queries
/// only the subset of declarations it needs.
#[derive(Default, Clone)]
struct TypeInfo {
    /// Struct name to (field name to declared field type).
    structs: BTreeMap<String, BTreeMap<String, TypeExpr>>,
    /// Enum name to (variant name to payload field types).
    enums: BTreeMap<String, BTreeMap<String, Vec<TypeExpr>>>,
    /// Function name to declared return type.
    function_returns: BTreeMap<String, TypeExpr>,
    /// Data block name to (field name to declared field type).
    data_field_types: BTreeMap<String, BTreeMap<String, TypeExpr>>,
}

/// State for compiling a single function chunk.
struct FuncCompiler {
    chunk: Chunk,
    locals: Vec<Local>,
    scope_depth: u32,
    next_slot: u16,
    /// Stack of loop contexts: each entry holds pending break jump addresses.
    loop_breaks: Vec<Vec<usize>>,
    /// Map from function name to chunk index (shared across all functions).
    function_map: BTreeMap<String, u16>,
    /// Map from native function name to native registry index.
    native_map: BTreeMap<String, u16>,
    /// Map from data block name to a list of (field_name, slot_index) pairs.
    data_fields: BTreeMap<String, Vec<(String, u16)>>,
    /// Static type information used by the for-in iteration bound
    /// inference and similar narrow optimizations.
    type_info: TypeInfo,
}

impl FuncCompiler {
    fn new(
        name: &str,
        block_type: BlockType,
        function_map: BTreeMap<String, u16>,
        native_map: BTreeMap<String, u16>,
        data_fields: BTreeMap<String, Vec<(String, u16)>>,
        type_info: TypeInfo,
    ) -> Self {
        Self {
            chunk: Chunk {
                name: String::from(name),
                ops: Vec::new(),
                constants: Vec::new(),
                struct_templates: Vec::new(),
                local_count: 0,
                param_count: 0,
                block_type,
            },
            locals: Vec::new(),
            scope_depth: 0,
            next_slot: 0,
            loop_breaks: Vec::new(),
            function_map,
            native_map,
            data_fields,
            type_info,
        }
    }

    /// Infer the static array length of an expression used as a
    /// for-in source. Returns `Some(N)` when the expression's static
    /// type is `[T; N]`. Used to emit a `Const(N)` end bound that the
    /// strict-mode WCMU verifier accepts. Falls back to `None` for
    /// expressions whose array length is not statically known.
    fn static_for_in_length(&self, expr: &Expr) -> Option<i64> {
        match expr {
            Expr::ArrayLiteral { elements, .. } => Some(elements.len() as i64),
            Expr::Call { name, .. } => {
                let return_type = self.type_info.function_returns.get(name)?;
                array_length_of_type(return_type)
            }
            Expr::FieldAccess { object, field, .. } => {
                let owner = self.struct_name_of(object)?;
                let field_type = self
                    .type_info
                    .structs
                    .get(&owner)
                    .or_else(|| self.type_info.data_field_types.get(&owner))
                    .and_then(|fields| fields.get(field))?;
                array_length_of_type(field_type)
            }
            Expr::Ident { name, .. } => {
                let ty = self.local_type(name)?;
                array_length_of_type(ty)
            }
            Expr::ArrayIndex { object, .. } => {
                // The result of indexing is the element type. For
                // `for x in matrix[0]` where matrix is [[T; N]; M],
                // the indexed expression has type [T; N].
                let object_ty = infer_expr_type(self, object)?;
                let elem_ty = element_type_of(&object_ty)?;
                array_length_of_type(&elem_ty)
            }
            Expr::Match { arms, .. } => {
                // All arms must agree on type (enforced by the type
                // checker P1). The iteration bound is the array length
                // of the first arm's expression.
                let first = arms.first()?;
                let arm_ty = infer_expr_type(self, &first.expr)?;
                array_length_of_type(&arm_ty)
            }
            _ => None,
        }
    }

    /// Return the struct or data block name for an expression that
    /// resolves to a named composite. Used by `static_for_in_length`
    /// to look up field types. Consults data block names first, then
    /// the type recorded for the local variable.
    fn struct_name_of(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident { name, .. } => {
                if self.data_fields.contains_key(name) {
                    return Some(name.clone());
                }
                let ty = self.local_type(name)?;
                if let TypeExpr::Named(struct_name, _, _) = ty {
                    return Some(struct_name.clone());
                }
                None
            }
            _ => None,
        }
    }

    fn emit(&mut self, op: Op) -> usize {
        let idx = self.chunk.ops.len();
        self.chunk.ops.push(op);
        idx
    }

    fn emit_jump(&mut self, placeholder: Op) -> usize {
        self.emit(placeholder)
    }

    fn patch_jump(&mut self, addr: usize) {
        let target = self.chunk.ops.len() as u32;
        match &mut self.chunk.ops[addr] {
            Op::If(a)
            | Op::Else(a)
            | Op::Loop(a)
            | Op::EndLoop(a)
            | Op::Break(a)
            | Op::BreakIf(a) => *a = target,
            _ => {}
        }
    }

    fn add_constant(&mut self, value: Value) -> u16 {
        // The compiler emits compile-time constants only. The
        // conversion below rejects runtime-only variants by panicking
        // because reaching this with a `DynStr` or `KStr` would be a
        // compiler bug rather than user-visible.
        let cv = ConstValue::try_from_value(value).expect("compile-time constant only");
        for (i, c) in self.chunk.constants.iter().enumerate() {
            if *c == cv {
                return i as u16;
            }
        }
        let idx = self.chunk.constants.len() as u16;
        self.chunk.constants.push(cv);
        idx
    }

    fn add_string_constant(&mut self, s: &str) -> u16 {
        self.add_constant(Value::StaticStr(String::from(s)))
    }

    fn add_struct_template(&mut self, type_name: &str, field_names: Vec<String>) -> u16 {
        let idx = self.chunk.struct_templates.len() as u16;
        self.chunk.struct_templates.push(StructTemplate {
            type_name: String::from(type_name),
            field_names,
        });
        idx
    }

    fn resolve_local(&self, name: &str) -> Option<u16> {
        for local in self.locals.iter().rev() {
            if local.name == name {
                return Some(local.slot);
            }
        }
        Option::None
    }

    /// Check if a name refers to a data block.
    fn is_data_block(&self, name: &str) -> bool {
        self.data_fields.contains_key(name)
    }

    /// Resolve a data block field to its slot index.
    fn resolve_data_field(&self, data_name: &str, field: &str) -> Option<u16> {
        self.data_fields.get(data_name).and_then(|fields| {
            fields
                .iter()
                .find(|(name, _)| name == field)
                .map(|(_, slot)| *slot)
        })
    }

    fn declare_local(&mut self, name: &str) -> u16 {
        self.declare_local_typed(name, None)
    }

    fn declare_local_typed(&mut self, name: &str, ty: Option<TypeExpr>) -> u16 {
        let slot = self.next_slot;
        self.next_slot += 1;
        if self.next_slot > self.chunk.local_count {
            self.chunk.local_count = self.next_slot;
        }
        self.locals.push(Local {
            name: String::from(name),
            slot,
            depth: self.scope_depth,
            ty,
        });
        slot
    }

    /// Look up the declared or inferred type of a local by name.
    fn local_type(&self, name: &str) -> Option<&TypeExpr> {
        for local in self.locals.iter().rev() {
            if local.name == name {
                return local.ty.as_ref();
            }
        }
        None
    }

    fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    fn end_scope(&mut self) {
        while let Some(local) = self.locals.last() {
            if local.depth < self.scope_depth {
                break;
            }
            self.locals.pop();
        }
        self.scope_depth -= 1;
    }

    fn enter_loop(&mut self) {
        self.loop_breaks.push(Vec::new());
    }

    fn exit_loop(&mut self) {
        if let Some(breaks) = self.loop_breaks.pop() {
            for addr in breaks {
                self.patch_jump(addr);
            }
        }
    }

    fn finish(mut self) -> Chunk {
        self.chunk.local_count = self.next_slot;
        self.chunk
    }
}

/// Extract the length of an array type expression. Returns `Some(N)`
/// for `[T; N]` and `None` for other shapes.
fn array_length_of_type(t: &TypeExpr) -> Option<i64> {
    match t {
        TypeExpr::Array(_, n, _) => Some(*n),
        _ => None,
    }
}

/// Extract the element type of an array type expression. Returns
/// `Some(T)` for `[T; N]` and `None` for other shapes.
fn element_type_of(t: &TypeExpr) -> Option<TypeExpr> {
    match t {
        TypeExpr::Array(elem, _, _) => Some((**elem).clone()),
        _ => None,
    }
}

/// Compile a parsed Keleusma program into a bytecode module.
///
/// Runs the static type checker before bytecode emission. Type errors
/// are surfaced as `CompileError` with the offending span. The
/// checker catches argument-count and argument-type mismatches,
/// return-type mismatches, let-binding annotation mismatches, and
/// arithmetic on incompatible types. Native function calls are not
/// type-checked at compile time because natives are registered at
/// runtime through `Vm::register_*`. See `crate::typecheck` for the
/// full coverage list.
pub fn compile(program: &Program) -> Result<Module, CompileError> {
    compile_with_target(program, &crate::target::Target::host())
}

/// Compile a program against an explicit target descriptor.
///
/// The target's word/address/float widths are baked into the
/// resulting module's wire-format header. The compiler validates
/// that the program does not use features unsupported by the target
/// (such as floating-point operations on a no-float target) and
/// rejects offending programs at compile time. The current 64-bit
/// runtime accepts bytecode with widths at most its own; emitting
/// for a narrower target produces bytecode the runtime can still
/// load, with integer arithmetic masked to the declared width via
/// `truncate_int`.
///
/// See `crate::target::Target` for available presets and the
/// portability story.
pub fn compile_with_target(
    program: &Program,
    target: &crate::target::Target,
) -> Result<Module, CompileError> {
    target.validate_against_runtime()?;
    crate::target::validate_program_for_target(program, target)?;
    crate::typecheck::check(program).map_err(|e| CompileError {
        message: format!("type error: {}", e.message),
        span: e.span,
    })?;

    // Monomorphize generic functions before compilation. The pass
    // walks the call graph and generates a specialized FunctionDef
    // per `(function, type_args)` pair. Call sites are rewritten to
    // reference the specialized name. Generic functions for which
    // no concrete instantiation could be inferred remain unchanged
    // and rely on runtime polymorphism through the Value tag.
    let owned = crate::monomorphize::monomorphize(program.clone());
    // Re-typecheck the monomorphized program so specialized bodies
    // benefit from concrete-type method resolution.
    crate::typecheck::check(&owned).map_err(|e| CompileError {
        message: format!("type error after monomorphization: {}", e.message),
        span: e.span,
    })?;
    // Hoist closure literals to top-level functions. Each
    // `Expr::Closure` becomes `Expr::Ident { name: "__closure_<n>" }`
    // and a synthetic `FunctionDef` is added to the program. The
    // closure's evaluation at runtime emits `Op::PushFunc(idx)` where
    // `idx` is the synthetic function's chunk index.
    let owned = hoist_closures(owned);
    let program = &owned;

    let mut native_names: Vec<String> = Vec::new();
    let mut native_map: BTreeMap<String, u16> = BTreeMap::new();

    // Collect native function names from use declarations.
    for use_decl in &program.uses {
        match &use_decl.import {
            ImportItem::Name(name) => {
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
                let idx = native_names.len() as u16;
                native_map.insert(full.clone(), idx);
                native_names.push(full);
            }
            ImportItem::Wildcard => {
                // Wildcard imports cannot be resolved at compile time.
                // The VM must resolve them at runtime. For now, skip.
            }
        }
    }

    // R28: at most one data block per program.
    if program.data_decls.len() > 1 {
        return Err(CompileError {
            message: format!(
                "at most one data block per program (R28), found {}",
                program.data_decls.len()
            ),
            span: program.data_decls[1].span,
        });
    }

    // Build data layout from data declarations. Validate that each field
    // type has a statically known fixed size before assigning a slot.
    let mut data_fields: BTreeMap<String, Vec<(String, u16)>> = BTreeMap::new();
    let mut data_layout_slots: Vec<DataSlot> = Vec::new();
    let mut data_slot_idx: u16 = 0;
    for decl in &program.data_decls {
        let mut fields = Vec::new();
        for field in &decl.fields {
            let mut visiting: BTreeSet<String> = BTreeSet::new();
            validate_data_field_type(&field.type_expr, &program.types, &mut visiting)?;
            fields.push((field.name.clone(), data_slot_idx));
            data_layout_slots.push(DataSlot {
                name: format!("{}.{}", decl.name, field.name),
            });
            data_slot_idx += 1;
        }
        data_fields.insert(decl.name.clone(), fields);
    }
    let data_layout = if data_layout_slots.is_empty() {
        None
    } else {
        Some(DataLayout {
            slots: data_layout_slots,
        })
    };

    // Group function definitions by name. Impl method definitions
    // are folded in under their mangled name `Trait::Type::method`
    // so the compiler treats them as regular callable functions.
    // Owned synthetic FunctionDefs hold the renamed methods because
    // the existing impl methods are borrowed.
    let mut synth_impl_methods: Vec<FunctionDef> = Vec::new();
    for impl_block in &program.impls {
        let head = match &impl_block.for_type {
            TypeExpr::Prim(p, _) => match p {
                PrimType::I64 => String::from("i64"),
                PrimType::F64 => String::from("f64"),
                PrimType::Bool => String::from("bool"),
                PrimType::KString => String::from("String"),
            },
            TypeExpr::Unit(_) => String::from("()"),
            TypeExpr::Named(name, _, _) => name.clone(),
            TypeExpr::Tuple(_, _) => String::from("tuple"),
            TypeExpr::Array(_, _, _) => String::from("array"),
            TypeExpr::Option(_, _) => String::from("Option"),
        };
        for method in &impl_block.methods {
            let mut renamed = method.clone();
            renamed.name = format!("{}::{}::{}", impl_block.trait_name, head, method.name);
            synth_impl_methods.push(renamed);
        }
    }
    let mut groups: BTreeMap<String, Vec<&FunctionDef>> = BTreeMap::new();
    for func in &program.functions {
        groups.entry(func.name.clone()).or_default().push(func);
    }
    for func in &synth_impl_methods {
        groups.entry(func.name.clone()).or_default().push(func);
    }

    // Build function name -> chunk index map.
    let mut function_map: BTreeMap<String, u16> = BTreeMap::new();
    for (chunk_idx, name) in groups.keys().enumerate() {
        function_map.insert(name.clone(), chunk_idx as u16);
    }

    // Build type info for the compiler's static analyses.
    let mut type_info = TypeInfo::default();
    for type_def in &program.types {
        match type_def {
            TypeDef::Struct(s) => {
                let mut fields = BTreeMap::new();
                for f in &s.fields {
                    fields.insert(f.name.clone(), f.type_expr.clone());
                }
                type_info.structs.insert(s.name.clone(), fields);
            }
            TypeDef::Enum(e) => {
                let mut variants = BTreeMap::new();
                for v in &e.variants {
                    variants.insert(v.name.clone(), v.fields.clone());
                }
                type_info.enums.insert(e.name.clone(), variants);
            }
        }
    }
    for func in &program.functions {
        type_info
            .function_returns
            .insert(func.name.clone(), func.return_type.clone());
    }
    for decl in &program.data_decls {
        let mut fields = BTreeMap::new();
        for f in &decl.fields {
            fields.insert(f.name.clone(), f.type_expr.clone());
        }
        type_info.data_field_types.insert(decl.name.clone(), fields);
    }

    // Compile each function group.
    let mut chunks: Vec<Chunk> = Vec::new();
    for (name, defs) in &groups {
        let chunk = compile_function_group(
            name,
            defs,
            &function_map,
            &native_map,
            &data_fields,
            &type_info,
        )?;
        chunks.push(chunk);
    }

    let entry_point = function_map.get("main").map(|&idx| idx as usize);

    Ok(Module {
        chunks,
        native_names,
        entry_point,
        data_layout,
        word_bits_log2: target.word_bits_log2,
        addr_bits_log2: target.addr_bits_log2,
        float_bits_log2: target.float_bits_log2,
    })
}

/// Validate that a data segment field type has a statically known fixed size.
///
/// Admissible: i64, f64, bool, (), tuples, fixed-length arrays, Option of
/// admissible, named structs of admissible fields, named enums whose variants
/// all have admissible payloads. Rejected: String, opaque named types,
/// recursive types.
fn validate_data_field_type(
    type_expr: &TypeExpr,
    types: &[TypeDef],
    visiting: &mut BTreeSet<String>,
) -> Result<(), CompileError> {
    match type_expr {
        TypeExpr::Prim(prim, span) => match prim {
            PrimType::I64 | PrimType::F64 | PrimType::Bool => Ok(()),
            PrimType::KString => Err(CompileError {
                message: String::from(
                    "data field type String is not admissible: variable-length \
                     types cannot be inlined into the data segment",
                ),
                span: *span,
            }),
        },
        TypeExpr::Unit(_) => Ok(()),
        TypeExpr::Tuple(elems, _) => {
            for elem in elems {
                validate_data_field_type(elem, types, visiting)?;
            }
            Ok(())
        }
        TypeExpr::Array(elem, _len, _) => validate_data_field_type(elem, types, visiting),
        TypeExpr::Option(inner, _) => validate_data_field_type(inner, types, visiting),
        TypeExpr::Named(name, _args, span) => {
            if visiting.contains(name) {
                return Err(CompileError {
                    message: format!(
                        "recursive type {} cannot appear in a data segment field: \
                         the data segment requires statically known fixed size",
                        name
                    ),
                    span: *span,
                });
            }
            let type_def = types.iter().find(|td| match td {
                TypeDef::Struct(s) => &s.name == name,
                TypeDef::Enum(e) => &e.name == name,
            });
            match type_def {
                Some(TypeDef::Struct(s)) => {
                    visiting.insert(name.clone());
                    for field in &s.fields {
                        validate_data_field_type(&field.type_expr, types, visiting)?;
                    }
                    visiting.remove(name);
                    Ok(())
                }
                Some(TypeDef::Enum(e)) => {
                    visiting.insert(name.clone());
                    for variant in &e.variants {
                        for ftype in &variant.fields {
                            validate_data_field_type(ftype, types, visiting)?;
                        }
                    }
                    visiting.remove(name);
                    Ok(())
                }
                None => Err(CompileError {
                    message: format!(
                        "data field type {} is not a struct or enum: opaque types \
                         are not yet admissible in data segment fields",
                        name
                    ),
                    span: *span,
                }),
            }
        }
    }
}

/// Compile a group of function definitions with the same name into one chunk.
fn compile_function_group(
    name: &str,
    defs: &[&FunctionDef],
    function_map: &BTreeMap<String, u16>,
    native_map: &BTreeMap<String, u16>,
    data_fields: &BTreeMap<String, Vec<(String, u16)>>,
    type_info: &TypeInfo,
) -> Result<Chunk, CompileError> {
    let first = defs[0];
    let block_type = match first.category {
        FunctionCategory::Fn => BlockType::Func,
        FunctionCategory::Yield => BlockType::Reentrant,
        FunctionCategory::Loop => BlockType::Stream,
    };
    let param_count = first.params.len() as u8;

    let mut fc = FuncCompiler::new(
        name,
        block_type,
        function_map.clone(),
        native_map.clone(),
        data_fields.clone(),
        type_info.clone(),
    );
    fc.chunk.param_count = param_count;

    // Declare parameter slots. For multiheaded functions, use positional names.
    let mut param_slots = Vec::new();
    for i in 0..param_count {
        let slot = fc.declare_local(&format!("__param{}", i));
        param_slots.push(slot);
    }

    if defs.len() == 1 && !has_non_trivial_pattern(&first.params) && first.guard.is_none() {
        // Single-headed, simple parameters: bind parameter names directly.
        for (i, param) in first.params.iter().enumerate() {
            bind_param_pattern(
                &mut fc,
                &param.pattern,
                param_slots[i],
                param.type_expr.clone(),
            );
        }

        if block_type == BlockType::Stream {
            // Stream function: wrap body in Stream...Reset.
            fc.emit(Op::Stream);
            compile_block(&mut fc, &first.body)?;
            fc.emit(Op::Pop); // Discard body value before Reset.
            fc.emit(Op::Reset);
        } else {
            compile_block(&mut fc, &first.body)?;
            fc.emit(Op::Return);
        }
    } else {
        // Multiheaded or pattern-matched parameters: dispatch.
        if block_type == BlockType::Stream {
            return Err(CompileError {
                message: String::from("multiheaded stream (loop) functions are not supported"),
                span: first.params.first().map_or(
                    Span {
                        start: 0,
                        end: 0,
                        line: 0,
                        column: 0,
                    },
                    |p| p.span,
                ),
            });
        }

        let mut fail_jumps: Vec<usize> = Vec::new();

        for def in defs {
            // Close previous arm's If blocks: emit EndIf for each fail_jump in reverse.
            for addr in fail_jumps.drain(..).rev() {
                fc.patch_jump(addr);
                fc.emit(Op::EndIf);
            }

            fc.begin_scope();

            // Test each parameter against the head's pattern.
            for (i, param) in def.params.iter().enumerate() {
                let fail = compile_pattern_test(&mut fc, &param.pattern, param_slots[i])?;
                fail_jumps.extend(fail);
            }

            // Bind pattern variables before guard (guard may reference
            // them). Pass the parameter's declared type so the bound
            // variables carry type information for downstream
            // optimizations.
            for (i, param) in def.params.iter().enumerate() {
                compile_pattern_bind_typed(
                    &mut fc,
                    &param.pattern,
                    param_slots[i],
                    param.type_expr.clone(),
                )?;
            }

            // Test guard clause if present.
            if let Some(guard) = &def.guard {
                compile_expr(&mut fc, guard)?;
                let fail = fc.emit_jump(Op::If(0));
                fail_jumps.push(fail);
            }

            compile_block(&mut fc, &def.body)?;
            fc.emit(Op::Return);

            fc.end_scope();
        }

        // Close final arm's If blocks.
        for addr in fail_jumps.drain(..).rev() {
            fc.patch_jump(addr);
            fc.emit(Op::EndIf);
        }

        // No head matched: emit trap.
        let msg = fc.add_string_constant(&format!("no matching head for {}", name));
        fc.emit(Op::Trap(msg));
    }

    Ok(fc.finish())
}

/// Check if any parameter has a non-trivial pattern (not a simple variable).
fn has_non_trivial_pattern(params: &[Param]) -> bool {
    params
        .iter()
        .any(|p| !matches!(p.pattern, Pattern::Variable(_, _)))
}

/// Bind a simple variable pattern to a parameter slot (alias).
///
/// The parameter's declared type, when present, is recorded on the
/// resulting local so that downstream optimizations such as the
/// for-in iteration bound inference can consult it.
fn bind_param_pattern(fc: &mut FuncCompiler, pattern: &Pattern, slot: u16, ty: Option<TypeExpr>) {
    if let Pattern::Variable(name, _) = pattern {
        // Create a named local that aliases the parameter slot.
        fc.locals.push(Local {
            name: name.clone(),
            slot,
            depth: fc.scope_depth,
            ty,
        });
    }
    // Wildcards and other patterns in simple mode are ignored.
}

/// Compile a block of statements with optional tail expression.
fn compile_block(fc: &mut FuncCompiler, block: &Block) -> Result<(), CompileError> {
    fc.begin_scope();
    for stmt in &block.stmts {
        compile_stmt(fc, stmt)?;
    }
    if let Some(tail) = &block.tail_expr {
        compile_expr(fc, tail)?;
    } else {
        fc.emit(Op::PushUnit);
    }
    fc.end_scope();
    Ok(())
}

/// Compile a data field assignment: `data_name.field = expr;`.
fn compile_data_field_assign(
    fc: &mut FuncCompiler,
    data_name: &str,
    field: &str,
    value: &Expr,
    span: Span,
) -> Result<(), CompileError> {
    let slot = fc
        .resolve_data_field(data_name, field)
        .ok_or_else(|| CompileError {
            message: format!("unknown data field: {}.{}", data_name, field),
            span,
        })?;
    compile_expr(fc, value)?;
    fc.emit(Op::SetData(slot));
    Ok(())
}

/// Compile a single statement.
fn compile_stmt(fc: &mut FuncCompiler, stmt: &Stmt) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let(let_stmt) => {
            // Determine the binding's type. If annotated, use the
            // annotation. Otherwise infer from the value expression
            // through a narrow set of patterns sufficient for the
            // for-in optimization (P2).
            let ty = let_stmt
                .type_expr
                .clone()
                .or_else(|| infer_expr_type(fc, &let_stmt.value));
            compile_expr(fc, &let_stmt.value)?;
            compile_let_pattern_typed(fc, &let_stmt.pattern, ty)?;
        }
        Stmt::For(for_stmt) => {
            compile_for(fc, for_stmt)?;
        }
        Stmt::Break(span) => {
            if fc.loop_breaks.is_empty() {
                return Err(CompileError {
                    message: String::from("break outside of loop"),
                    span: *span,
                });
            }
            let addr = fc.emit_jump(Op::Break(0));
            if let Some(breaks) = fc.loop_breaks.last_mut() {
                breaks.push(addr);
            }
        }
        Stmt::DataFieldAssign {
            data_name,
            field,
            value,
            span,
        } => {
            compile_data_field_assign(fc, data_name, field, value, *span)?;
        }
        Stmt::Expr(expr) => {
            compile_expr(fc, expr)?;
            fc.emit(Op::Pop);
        }
    }
    Ok(())
}

/// Infer a type expression for a let binding's right-hand side.
///
/// Returns `Some(TypeExpr)` when the value's type is determinable
/// from a narrow set of patterns:
/// - Struct construction. `Type { ... }` has type `Type`.
/// - Function call. The function's declared return type.
/// - Identifier. The local's recorded type.
/// - Field access. The struct or data field's declared type.
/// - Array literal with elements of inferable type.
/// - Literal value.
///
/// Used by the let-binding compile path so that locals carry type
/// information for downstream optimizations such as for-in iteration
/// bound inference. Returns `None` for expressions whose type is not
/// determinable through this narrow set of patterns.
fn infer_expr_type(fc: &FuncCompiler, expr: &Expr) -> Option<TypeExpr> {
    match expr {
        Expr::StructInit { name, span, .. } => {
            Some(TypeExpr::Named(name.clone(), Vec::new(), *span))
        }
        Expr::Call { name, .. } => fc.type_info.function_returns.get(name).cloned(),
        Expr::Ident { name, .. } => fc.local_type(name).cloned(),
        Expr::FieldAccess { object, field, .. } => {
            let owner = fc.struct_name_of(object)?;
            let field_type = fc
                .type_info
                .structs
                .get(&owner)
                .or_else(|| fc.type_info.data_field_types.get(&owner))
                .and_then(|fields| fields.get(field))?;
            Some(field_type.clone())
        }
        Expr::ArrayLiteral { elements, span } => {
            let elem_ty = elements.first().and_then(|e| infer_expr_type(fc, e))?;
            Some(TypeExpr::Array(
                Box::new(elem_ty),
                elements.len() as i64,
                *span,
            ))
        }
        Expr::ArrayIndex { object, .. } => {
            let object_ty = infer_expr_type(fc, object)?;
            element_type_of(&object_ty)
        }
        Expr::Match { arms, .. } => {
            let first = arms.first()?;
            infer_expr_type(fc, &first.expr)
        }
        Expr::Literal { value, span } => Some(match value {
            Literal::Int(_) => TypeExpr::Prim(PrimType::I64, *span),
            Literal::Float(_) => TypeExpr::Prim(PrimType::F64, *span),
            Literal::Bool(_) => TypeExpr::Prim(PrimType::Bool, *span),
            Literal::String(_) => TypeExpr::Prim(PrimType::KString, *span),
            Literal::Unit => TypeExpr::Unit(*span),
        }),
        _ => None,
    }
}

/// Compute the head name of a type expression for method dispatch
/// resolution. Mirrors the typecheck `type_head_name` helper.
fn type_expr_head(ty: &TypeExpr) -> Option<String> {
    use alloc::string::ToString;
    match ty {
        TypeExpr::Prim(p, _) => Some(
            match p {
                PrimType::I64 => "i64",
                PrimType::F64 => "f64",
                PrimType::Bool => "bool",
                PrimType::KString => "String",
            }
            .to_string(),
        ),
        TypeExpr::Unit(_) => Some("()".to_string()),
        TypeExpr::Tuple(_, _) => Some("tuple".to_string()),
        TypeExpr::Array(_, _, _) => Some("array".to_string()),
        TypeExpr::Option(_, _) => Some("Option".to_string()),
        TypeExpr::Named(name, _, _) => Some(name.clone()),
    }
}

/// Compile a let binding pattern (allocate locals and store values).
fn compile_let_pattern(fc: &mut FuncCompiler, pattern: &Pattern) -> Result<(), CompileError> {
    compile_let_pattern_typed(fc, pattern, None)
}

/// Compile a let binding pattern with an associated type expression.
///
/// The type, when present, is recorded on the resulting local for
/// downstream optimization passes. Compound patterns destructure the
/// type along with the value.
fn compile_let_pattern_typed(
    fc: &mut FuncCompiler,
    pattern: &Pattern,
    ty: Option<TypeExpr>,
) -> Result<(), CompileError> {
    match pattern {
        Pattern::Variable(name, _) => {
            let slot = fc.declare_local_typed(name, ty);
            fc.emit(Op::SetLocal(slot));
        }
        Pattern::Wildcard(_) => {
            fc.emit(Op::Pop);
        }
        Pattern::Tuple(pats, _) => {
            // Value is on stack. Store in temp, then extract fields.
            let temp = fc.declare_local("__let_tmp");
            fc.emit(Op::SetLocal(temp));
            for (i, pat) in pats.iter().enumerate() {
                fc.emit(Op::GetLocal(temp));
                fc.emit(Op::GetTupleField(i as u8));
                compile_let_pattern(fc, pat)?;
            }
        }
        _ => {
            // For other patterns in let, just bind as a single variable.
            let slot = fc.declare_local("_");
            fc.emit(Op::SetLocal(slot));
        }
    }
    Ok(())
}

/// Compile a for loop.
fn compile_for(fc: &mut FuncCompiler, for_stmt: &ForStmt) -> Result<(), CompileError> {
    match &for_stmt.iterable {
        Iterable::Range(start, end) => {
            // Compile range bounds.
            compile_expr(fc, start)?;
            let var_slot = fc.declare_local(&for_stmt.var);
            fc.emit(Op::SetLocal(var_slot));

            compile_expr(fc, end)?;
            let end_slot = fc.declare_local("__for_end");
            fc.emit(Op::SetLocal(end_slot));

            let loop_addr = fc.emit(Op::Loop(0)); // Placeholder, patched to past EndLoop.
            fc.enter_loop();

            // Condition: break if var >= end.
            fc.emit(Op::GetLocal(var_slot));
            fc.emit(Op::GetLocal(end_slot));
            fc.emit(Op::CmpGe);
            let break_addr = fc.emit(Op::BreakIf(0)); // Placeholder.

            // Body.
            fc.begin_scope();
            compile_block(fc, &for_stmt.body)?;
            fc.emit(Op::Pop); // Discard block value.
            fc.end_scope();

            // Increment.
            fc.emit(Op::GetLocal(var_slot));
            let one_const = fc.add_constant(Value::Int(1));
            fc.emit(Op::Const(one_const));
            fc.emit(Op::Add);
            fc.emit(Op::SetLocal(var_slot));

            let endloop_addr = fc.emit(Op::EndLoop(0)); // Placeholder, patched to after Loop.

            // Patch Loop and BreakIf to point past EndLoop.
            let after_endloop = fc.chunk.ops.len() as u32;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            if let Op::BreakIf(a) = &mut fc.chunk.ops[break_addr] {
                *a = after_endloop;
            }
            // Patch EndLoop back-edge to instruction after Loop.
            let after_loop = (loop_addr + 1) as u32;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            fc.exit_loop(); // Patches Break addresses to after_endloop.
        }
        Iterable::Expr(expr) => {
            // Determine the static array length if the source's type is
            // statically known. Used to emit a `Const(N)` end bound that
            // the strict-mode WCMU verifier accepts. Falls back to
            // `Op::Len` for sources whose length is not statically
            // known. The fall-back is admissible at the bytecode level
            // but may be rejected by the verifier in strict mode.
            let static_length = fc.static_for_in_length(expr);
            // Determine the iteration variable's type from the source
            // type's element type. Recorded on the iteration variable's
            // local so that nested for-in loops can resolve their own
            // iteration bounds through this binding.
            let element_ty = infer_expr_type(fc, expr).and_then(|t| element_type_of(&t));

            // Compile the iterable expression (array) and store it.
            compile_expr(fc, expr)?;
            let arr_slot = fc.declare_local("__for_arr");
            fc.emit(Op::SetLocal(arr_slot));

            // Compute the end bound.
            let end_slot = fc.declare_local("__for_end");
            if let Some(n) = static_length {
                let n_const = fc.add_constant(Value::Int(n));
                fc.emit(Op::Const(n_const));
                fc.emit(Op::SetLocal(end_slot));
            } else {
                fc.emit(Op::GetLocal(arr_slot));
                fc.emit(Op::Len);
                fc.emit(Op::SetLocal(end_slot));
            }

            // Initialize index to 0.
            let zero_const = fc.add_constant(Value::Int(0));
            fc.emit(Op::Const(zero_const));
            let idx_slot = fc.declare_local("__for_idx");
            fc.emit(Op::SetLocal(idx_slot));

            let loop_addr = fc.emit(Op::Loop(0));
            fc.enter_loop();

            // Condition: break if index >= length.
            fc.emit(Op::GetLocal(idx_slot));
            fc.emit(Op::GetLocal(end_slot));
            fc.emit(Op::CmpGe);
            let break_addr = fc.emit(Op::BreakIf(0));

            // Extract element at current index.
            fc.emit(Op::GetLocal(arr_slot));
            fc.emit(Op::GetLocal(idx_slot));
            fc.emit(Op::GetIndex);
            let var_slot = fc.declare_local_typed(&for_stmt.var, element_ty);
            fc.emit(Op::SetLocal(var_slot));

            // Body.
            fc.begin_scope();
            compile_block(fc, &for_stmt.body)?;
            fc.emit(Op::Pop);
            fc.end_scope();

            // Increment index.
            fc.emit(Op::GetLocal(idx_slot));
            let one_const = fc.add_constant(Value::Int(1));
            fc.emit(Op::Const(one_const));
            fc.emit(Op::Add);
            fc.emit(Op::SetLocal(idx_slot));

            let endloop_addr = fc.emit(Op::EndLoop(0));

            // Patch jumps.
            let after_endloop = fc.chunk.ops.len() as u32;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            if let Op::BreakIf(a) = &mut fc.chunk.ops[break_addr] {
                *a = after_endloop;
            }
            let after_loop = (loop_addr + 1) as u32;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            fc.exit_loop();
        }
    }
    Ok(())
}

/// Collect the variable names bound by a pattern. Tuple, struct,
/// and enum sub-patterns recurse. Wildcards bind nothing.
fn collect_pattern_names(
    pattern: &Pattern,
    out: &mut alloc::collections::BTreeSet<alloc::string::String>,
) {
    match pattern {
        Pattern::Variable(name, _) => {
            out.insert(name.clone());
        }
        Pattern::Wildcard(_) | Pattern::Literal(_, _) => {}
        Pattern::Tuple(parts, _) => {
            for p in parts {
                collect_pattern_names(p, out);
            }
        }
        Pattern::Enum(_, _, sub_pats, _) => {
            for p in sub_pats {
                collect_pattern_names(p, out);
            }
        }
        Pattern::Struct(_, field_pats, _) => {
            for fp in field_pats {
                if let Some(p) = &fp.pattern {
                    collect_pattern_names(p, out);
                } else {
                    out.insert(fp.name.clone());
                }
            }
        }
    }
}

/// Collect free identifiers in a block. A free identifier is one that
/// is referenced but not in the current `bound` set. Matching let
/// bindings and for-loop variables extend `bound` for the rest of
/// the block. Closures within the block are NOT recursed into here
/// because they have their own free-variable analysis.
fn collect_free_in_block(
    block: &Block,
    bound: &alloc::collections::BTreeSet<alloc::string::String>,
    out: &mut Vec<alloc::string::String>,
) {
    let mut local_bound = bound.clone();
    for stmt in &block.stmts {
        collect_free_in_stmt(stmt, &mut local_bound, out);
    }
    if let Some(e) = &block.tail_expr {
        collect_free_in_expr(e, &local_bound, out);
    }
}

fn collect_free_in_stmt(
    stmt: &Stmt,
    bound: &mut alloc::collections::BTreeSet<alloc::string::String>,
    out: &mut Vec<alloc::string::String>,
) {
    match stmt {
        Stmt::Let(l) => {
            collect_free_in_expr(&l.value, bound, out);
            collect_pattern_names(&l.pattern, bound);
        }
        Stmt::For(f) => {
            match &f.iterable {
                Iterable::Range(s, e) => {
                    collect_free_in_expr(s, bound, out);
                    collect_free_in_expr(e, bound, out);
                }
                Iterable::Expr(e) => collect_free_in_expr(e, bound, out),
            }
            let mut inner_bound = bound.clone();
            inner_bound.insert(f.var.clone());
            collect_free_in_block(&f.body, &inner_bound, out);
        }
        Stmt::Break(_) => {}
        Stmt::DataFieldAssign { value, .. } => collect_free_in_expr(value, bound, out),
        Stmt::Expr(e) => collect_free_in_expr(e, bound, out),
    }
}

fn collect_free_in_expr(
    expr: &Expr,
    bound: &alloc::collections::BTreeSet<alloc::string::String>,
    out: &mut Vec<alloc::string::String>,
) {
    match expr {
        Expr::Literal { .. } | Expr::Placeholder { .. } => {}
        Expr::Ident { name, .. } => {
            if !bound.contains(name) && !out.contains(name) {
                out.push(name.clone());
            }
        }
        Expr::BinOp { left, right, .. } => {
            collect_free_in_expr(left, bound, out);
            collect_free_in_expr(right, bound, out);
        }
        Expr::UnaryOp { operand, .. } => collect_free_in_expr(operand, bound, out),
        Expr::Call { name, args, .. } => {
            // Only treat the call name as a free variable if it
            // resolves to a local (not a function). The caller
            // doesn't know which is which here, so we conservatively
            // include the name. The compile site filters non-locals
            // out by checking against function_map first when
            // capturing.
            if !bound.contains(name) && !out.contains(name) {
                out.push(name.clone());
            }
            for a in args {
                collect_free_in_expr(a, bound, out);
            }
        }
        Expr::Pipeline { left, args, .. } => {
            collect_free_in_expr(left, bound, out);
            for a in args {
                collect_free_in_expr(a, bound, out);
            }
        }
        Expr::Yield { value, .. } => collect_free_in_expr(value, bound, out),
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            collect_free_in_expr(condition, bound, out);
            collect_free_in_block(then_block, bound, out);
            if let Some(b) = else_block {
                collect_free_in_block(b, bound, out);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            collect_free_in_expr(scrutinee, bound, out);
            for arm in arms {
                let mut arm_bound = bound.clone();
                collect_pattern_names(&arm.pattern, &mut arm_bound);
                collect_free_in_expr(&arm.expr, &arm_bound, out);
            }
        }
        Expr::Loop { body, .. } => collect_free_in_block(body, bound, out),
        Expr::FieldAccess { object, .. } => collect_free_in_expr(object, bound, out),
        Expr::MethodCall { receiver, args, .. } => {
            collect_free_in_expr(receiver, bound, out);
            for a in args {
                collect_free_in_expr(a, bound, out);
            }
        }
        Expr::TupleIndex { object, .. } => collect_free_in_expr(object, bound, out),
        Expr::ArrayIndex { object, index, .. } => {
            collect_free_in_expr(object, bound, out);
            collect_free_in_expr(index, bound, out);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields {
                collect_free_in_expr(&f.value, bound, out);
            }
        }
        Expr::EnumVariant { args, .. } => {
            for a in args {
                collect_free_in_expr(a, bound, out);
            }
        }
        Expr::ArrayLiteral { elements, .. } | Expr::TupleLiteral { elements, .. } => {
            for e in elements {
                collect_free_in_expr(e, bound, out);
            }
        }
        Expr::Cast { expr, .. } => collect_free_in_expr(expr, bound, out),
        Expr::Closure { .. } => {
            // Should not occur after the hoist pass replaces every
            // closure literal with a `ClosureRef`. Conservatively do
            // nothing if it does appear; the surrounding pass will
            // hoist it on the next walk.
        }
        Expr::ClosureRef { captures, .. } => {
            // The inner closure has already been hoisted and its free
            // variables recorded as captures. Each inner capture that
            // is not bound in the outer scope is itself a free
            // variable of the outer expression. Propagating these
            // names lets nested closures transitively capture from
            // enclosing scopes through the surrounding closure's
            // synthetic chunk.
            for capture in captures {
                if !bound.contains(capture) && !out.contains(capture) {
                    out.push(capture.clone());
                }
            }
        }
    }
}

/// Closure hoisting pass.
///
/// Walks every function body and impl method body in the program,
/// replacing `Expr::Closure` expressions with `Expr::Ident {
/// name: "__closure_<n>" }` references and emitting a synthetic
/// `FunctionDef` per closure. The synthetic functions go to the
/// end of `program.functions` so they receive chunk indices in the
/// expected order. Surrounding compilation logic resolves the
/// `Expr::Ident` to a chunk-emitting `Op::PushFunc(idx)` because the
/// synthetic name is in `function_map` but never appears as a
/// local.
fn hoist_closures(mut program: Program) -> Program {
    // Build the set of native names declared via `use` so the
    // free-variable analysis does not treat them as captures.
    // Closures that reference native functions resolve through the
    // synthetic chunk's normal call path at compile time.
    let mut natives: alloc::collections::BTreeSet<alloc::string::String> =
        alloc::collections::BTreeSet::new();
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
            natives.insert(full);
            natives.insert(name.clone());
        }
    }
    let mut counter: usize = 0;
    let mut new_functions: Vec<FunctionDef> = Vec::new();
    for func in program.functions.iter_mut() {
        hoist_in_block(&mut func.body, &mut counter, &mut new_functions, &natives);
    }
    for impl_block in program.impls.iter_mut() {
        for method in impl_block.methods.iter_mut() {
            hoist_in_block(&mut method.body, &mut counter, &mut new_functions, &natives);
        }
    }
    program.functions.extend(new_functions);
    program
}

/// Hoist a closure expression that is the value of a `let` binding,
/// detecting self-reference. When the closure body's free variables
/// include the let-binding name, the closure is treated as recursive.
/// The synthetic chunk's parameter list places the binding name
/// after the other captures, the runtime fills that slot with the
/// closure value at each invocation, and the compiler emits
/// `Op::MakeRecursiveClosure` at the construction site.
fn hoist_let_bound_closure(
    let_name: alloc::string::String,
    expr: &mut Expr,
    counter: &mut usize,
    out: &mut Vec<FunctionDef>,
    natives: &alloc::collections::BTreeSet<alloc::string::String>,
) {
    let (params, return_type, body, span) = match expr {
        Expr::Closure {
            params,
            return_type,
            body,
            span,
        } => (params, return_type, body, *span),
        _ => return,
    };
    // Hoist any nested closures inside the body first.
    hoist_in_block(body, counter, out, natives);
    let mut bound: alloc::collections::BTreeSet<alloc::string::String> =
        alloc::collections::BTreeSet::new();
    for p in params.iter() {
        collect_pattern_names(&p.pattern, &mut bound);
    }
    let mut free: Vec<alloc::string::String> = Vec::new();
    collect_free_in_block(body, &bound, &mut free);
    let recursive = free.iter().any(|n| n == &let_name);
    if !recursive {
        // Defer to the regular closure-hoist path if the body does
        // not actually reference the let-binding name. This keeps
        // the non-recursive cases identical to the previous flow.
        hoist_in_expr(expr, counter, out, natives);
        return;
    }
    // Filter out the self-reference from the regular captures list.
    free.retain(|n| n != &let_name);
    let name = format!("__closure_{}", *counter);
    *counter += 1;
    let mut all_params: Vec<Param> = Vec::with_capacity(free.len() + 1 + params.len());
    for capture in &free {
        all_params.push(Param {
            pattern: Pattern::Variable(capture.clone(), span),
            type_expr: None,
            span,
        });
    }
    // The self parameter sits between captures and explicit params.
    all_params.push(Param {
        pattern: Pattern::Variable(let_name.clone(), span),
        type_expr: None,
        span,
    });
    all_params.extend(params.iter().cloned());
    let body_owned = core::mem::replace(
        body,
        Block {
            stmts: Vec::new(),
            tail_expr: None,
            span,
        },
    );
    let synth = FunctionDef {
        category: FunctionCategory::Fn,
        name: name.clone(),
        type_params: Vec::new(),
        params: all_params,
        return_type: return_type.clone().unwrap_or(TypeExpr::Unit(span)),
        guard: None,
        body: body_owned,
        span,
    };
    out.push(synth);
    *expr = Expr::ClosureRef {
        name,
        captures: free,
        recursive: true,
        span,
    };
}

fn hoist_in_block(
    block: &mut Block,
    counter: &mut usize,
    out: &mut Vec<FunctionDef>,
    natives: &alloc::collections::BTreeSet<alloc::string::String>,
) {
    for stmt in block.stmts.iter_mut() {
        hoist_in_stmt(stmt, counter, out, natives);
    }
    if let Some(e) = block.tail_expr.as_mut() {
        hoist_in_expr(e, counter, out, natives);
    }
}

fn hoist_in_stmt(
    stmt: &mut Stmt,
    counter: &mut usize,
    out: &mut Vec<FunctionDef>,
    natives: &alloc::collections::BTreeSet<alloc::string::String>,
) {
    match stmt {
        Stmt::Let(l) => {
            // Special-case `let f = |...| ...` where the closure body
            // references the let-binding name. The hoist pass treats
            // the binding name as a self-reference and produces a
            // recursive ClosureRef. The synthetic chunk's parameter
            // list is laid out as
            //   (other_captures..., self_param, explicit_params...)
            // so references to the binding name inside the body
            // resolve to the implicit self parameter and dispatch
            // through `Op::CallIndirect`. The compiler emits
            // `Op::MakeRecursiveClosure` at the construction site and
            // the runtime pushes the closure value into the self
            // slot at each invocation.
            if let Pattern::Variable(let_name, _) = &l.pattern
                && let Expr::Closure { .. } = &l.value
            {
                hoist_let_bound_closure(let_name.clone(), &mut l.value, counter, out, natives);
            } else {
                hoist_in_expr(&mut l.value, counter, out, natives);
            }
        }
        Stmt::For(f) => {
            match &mut f.iterable {
                Iterable::Range(s, e) => {
                    hoist_in_expr(s, counter, out, natives);
                    hoist_in_expr(e, counter, out, natives);
                }
                Iterable::Expr(e) => hoist_in_expr(e, counter, out, natives),
            }
            hoist_in_block(&mut f.body, counter, out, natives);
        }
        Stmt::Break(_) => {}
        Stmt::DataFieldAssign { value, .. } => hoist_in_expr(value, counter, out, natives),
        Stmt::Expr(e) => hoist_in_expr(e, counter, out, natives),
    }
}

fn hoist_in_expr(
    expr: &mut Expr,
    counter: &mut usize,
    out: &mut Vec<FunctionDef>,
    natives: &alloc::collections::BTreeSet<alloc::string::String>,
) {
    match expr {
        Expr::BinOp { left, right, .. } => {
            hoist_in_expr(left, counter, out, natives);
            hoist_in_expr(right, counter, out, natives);
        }
        Expr::UnaryOp { operand, .. } => hoist_in_expr(operand, counter, out, natives),
        Expr::Call { args, .. } => {
            for a in args.iter_mut() {
                hoist_in_expr(a, counter, out, natives);
            }
        }
        Expr::Pipeline { left, args, .. } => {
            hoist_in_expr(left, counter, out, natives);
            for a in args.iter_mut() {
                hoist_in_expr(a, counter, out, natives);
            }
        }
        Expr::Yield { value, .. } => hoist_in_expr(value, counter, out, natives),
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            hoist_in_expr(condition, counter, out, natives);
            hoist_in_block(then_block, counter, out, natives);
            if let Some(b) = else_block.as_mut() {
                hoist_in_block(b, counter, out, natives);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            hoist_in_expr(scrutinee, counter, out, natives);
            for arm in arms.iter_mut() {
                hoist_in_expr(&mut arm.expr, counter, out, natives);
            }
        }
        Expr::Loop { body, .. } => hoist_in_block(body, counter, out, natives),
        Expr::FieldAccess { object, .. } => hoist_in_expr(object, counter, out, natives),
        Expr::MethodCall { receiver, args, .. } => {
            hoist_in_expr(receiver, counter, out, natives);
            for a in args.iter_mut() {
                hoist_in_expr(a, counter, out, natives);
            }
        }
        Expr::TupleIndex { object, .. } => hoist_in_expr(object, counter, out, natives),
        Expr::ArrayIndex { object, index, .. } => {
            hoist_in_expr(object, counter, out, natives);
            hoist_in_expr(index, counter, out, natives);
        }
        Expr::StructInit { fields, .. } => {
            for f in fields.iter_mut() {
                hoist_in_expr(&mut f.value, counter, out, natives);
            }
        }
        Expr::EnumVariant { args, .. } => {
            for a in args.iter_mut() {
                hoist_in_expr(a, counter, out, natives);
            }
        }
        Expr::ArrayLiteral { elements, .. } | Expr::TupleLiteral { elements, .. } => {
            for e in elements.iter_mut() {
                hoist_in_expr(e, counter, out, natives);
            }
        }
        Expr::Cast { expr, .. } => hoist_in_expr(expr, counter, out, natives),
        Expr::Closure {
            params,
            return_type,
            body,
            span,
        } => {
            // First descend into the body so nested closures are
            // hoisted too.
            hoist_in_block(body, counter, out, natives);
            // Compute the set of names bound by the closure's own
            // parameters. Identifiers in the body that are not in
            // this set are free variables and become captures.
            let mut bound: alloc::collections::BTreeSet<alloc::string::String> =
                alloc::collections::BTreeSet::new();
            for p in params.iter() {
                collect_pattern_names(&p.pattern, &mut bound);
            }
            let mut free: Vec<alloc::string::String> = Vec::new();
            collect_free_in_block(body, &bound, &mut free);
            // Allocate a synthetic name and assemble the synthetic
            // function definition. Captured names become parameters
            // prepended to the user's parameters, with `Type::Unknown`-
            // equivalent type expressions because their concrete
            // types flow from the call site.
            let name = format!("__closure_{}", *counter);
            *counter += 1;
            let mut all_params: Vec<Param> = Vec::with_capacity(free.len() + params.len());
            for capture in &free {
                all_params.push(Param {
                    pattern: Pattern::Variable(capture.clone(), *span),
                    type_expr: None,
                    span: *span,
                });
            }
            all_params.extend(params.iter().cloned());
            let body_owned = core::mem::replace(
                body,
                Block {
                    stmts: Vec::new(),
                    tail_expr: None,
                    span: *span,
                },
            );
            let synth = FunctionDef {
                category: FunctionCategory::Fn,
                name: name.clone(),
                type_params: Vec::new(),
                params: all_params,
                return_type: return_type.clone().unwrap_or(TypeExpr::Unit(*span)),
                guard: None,
                body: body_owned,
                span: *span,
            };
            out.push(synth);
            // Replace the closure expression with a ClosureRef that
            // carries the captured names. The compiler emits a
            // `MakeClosure` (or `PushFunc` when there are no
            // captures) plus `GetLocal` for each capture.
            *expr = Expr::ClosureRef {
                name,
                captures: free,
                recursive: false,
                span: *span,
            };
        }
        Expr::ClosureRef { .. } => {
            // Already hoisted; nothing further to do at this pass.
        }
        Expr::Literal { .. } | Expr::Ident { .. } | Expr::Placeholder { .. } => {}
    }
}

/// Compile an expression, leaving the result on the stack.
fn compile_expr(fc: &mut FuncCompiler, expr: &Expr) -> Result<(), CompileError> {
    match expr {
        Expr::Literal { value, .. } => match value {
            Literal::Int(v) => {
                let idx = fc.add_constant(Value::Int(*v));
                fc.emit(Op::Const(idx));
            }
            Literal::Float(v) => {
                let idx = fc.add_constant(Value::Float(*v));
                fc.emit(Op::Const(idx));
            }
            Literal::String(s) => {
                let idx = fc.add_constant(Value::StaticStr(s.clone()));
                fc.emit(Op::Const(idx));
            }
            Literal::Bool(true) => {
                fc.emit(Op::PushTrue);
            }
            Literal::Bool(false) => {
                fc.emit(Op::PushFalse);
            }
            Literal::Unit => {
                fc.emit(Op::PushUnit);
            }
        },

        Expr::Ident { name, span } => {
            if let Some(slot) = fc.resolve_local(name) {
                fc.emit(Op::GetLocal(slot));
            } else if let Some(&idx) = fc.function_map.get(name) {
                // The name resolves to a top-level function (or a
                // hoisted closure). Emit a `Func` value that the
                // runtime can invoke through `Op::CallIndirect`.
                fc.emit(Op::PushFunc(idx));
            } else if fc.is_data_block(name) {
                return Err(CompileError {
                    message: format!(
                        "data block '{}' cannot be used as a value; access individual fields with {}.field_name",
                        name, name
                    ),
                    span: *span,
                });
            } else {
                return Err(CompileError {
                    message: format!("undefined variable: {}", name),
                    span: *span,
                });
            }
        }

        Expr::BinOp {
            op, left, right, ..
        } => {
            // Short-circuit for logical operators using block-structured control.
            match op {
                BinOp::And => {
                    // a && b: if a is false, result is false; else result is b.
                    compile_expr(fc, left)?;
                    fc.emit(Op::Dup);
                    let if_addr = fc.emit_jump(Op::If(0));
                    // a was true: discard dup, evaluate b.
                    fc.emit(Op::Pop);
                    compile_expr(fc, right)?;
                    let else_addr = fc.emit_jump(Op::Else(0));
                    fc.patch_jump(if_addr);
                    // a was false: duplicated false is on stack as result.
                    fc.patch_jump(else_addr);
                    fc.emit(Op::EndIf);
                    return Ok(());
                }
                BinOp::Or => {
                    // a || b: if a is true, result is true; else result is b.
                    compile_expr(fc, left)?;
                    fc.emit(Op::Dup);
                    fc.emit(Op::Not);
                    let if_addr = fc.emit_jump(Op::If(0));
                    // a was false (Not gave true, If continued): discard dup, evaluate b.
                    fc.emit(Op::Pop);
                    compile_expr(fc, right)?;
                    let else_addr = fc.emit_jump(Op::Else(0));
                    fc.patch_jump(if_addr);
                    // a was true (Not gave false, If skipped): duplicated true is on stack.
                    fc.patch_jump(else_addr);
                    fc.emit(Op::EndIf);
                    return Ok(());
                }
                _ => {}
            }
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            match op {
                BinOp::Add => {
                    fc.emit(Op::Add);
                }
                BinOp::Sub => {
                    fc.emit(Op::Sub);
                }
                BinOp::Mul => {
                    fc.emit(Op::Mul);
                }
                BinOp::Div => {
                    fc.emit(Op::Div);
                }
                BinOp::Mod => {
                    fc.emit(Op::Mod);
                }
                BinOp::Eq => {
                    fc.emit(Op::CmpEq);
                }
                BinOp::NotEq => {
                    fc.emit(Op::CmpNe);
                }
                BinOp::Lt => {
                    fc.emit(Op::CmpLt);
                }
                BinOp::Gt => {
                    fc.emit(Op::CmpGt);
                }
                BinOp::LtEq => {
                    fc.emit(Op::CmpLe);
                }
                BinOp::GtEq => {
                    fc.emit(Op::CmpGe);
                }
                BinOp::And | BinOp::Or => unreachable!(),
            }
        }

        Expr::UnaryOp { op, operand, .. } => {
            compile_expr(fc, operand)?;
            match op {
                UnaryOp::Neg => {
                    fc.emit(Op::Neg);
                }
                UnaryOp::Not => {
                    fc.emit(Op::Not);
                }
            }
        }

        Expr::Call { name, args, span } => {
            compile_call(fc, name, args, span)?;
        }

        Expr::MethodCall {
            receiver,
            method,
            args,
            span,
        } => {
            // Resolve the method by inferring the receiver's type
            // head and finding a registered impl method with the
            // mangled name `Trait::Head::method` in the function map.
            // The receiver is passed as the first argument to the
            // resolved chunk.
            let head = match infer_expr_type(fc, receiver).and_then(|t| type_expr_head(&t)) {
                Some(h) => h,
                None => {
                    return Err(CompileError {
                        message: format!(
                            "method `{}` receiver type cannot be statically resolved; \
                             this currently requires monomorphization (B2.4)",
                            method
                        ),
                        span: *span,
                    });
                }
            };
            // Search the function map for any `*::Head::method` entry.
            let suffix = format!("::{}::{}", head, method);
            let resolved = fc
                .function_map
                .iter()
                .find(|(k, _)| k.ends_with(&suffix))
                .map(|(k, &idx)| (k.clone(), idx));
            let (mangled, chunk_idx) = match resolved {
                Some(p) => p,
                None => {
                    return Err(CompileError {
                        message: format!(
                            "type `{}` has no method `{}` from any trait in scope",
                            head, method
                        ),
                        span: *span,
                    });
                }
            };
            let _ = mangled;
            // Push the receiver and remaining arguments, then call.
            compile_expr(fc, receiver)?;
            for arg in args {
                compile_expr(fc, arg)?;
            }
            let arg_count = (args.len() + 1) as u8;
            fc.emit(Op::Call(chunk_idx, arg_count));
        }

        Expr::Pipeline {
            left,
            func,
            args,
            span,
        } => {
            // Desugar pipeline: insert left as first arg, or at placeholder position.
            let mut call_args: Vec<&Expr> = Vec::new();
            let mut placeholder_found = false;
            for arg in args {
                if matches!(arg, Expr::Placeholder { .. }) {
                    call_args.push(left);
                    placeholder_found = true;
                } else {
                    call_args.push(arg);
                }
            }
            if !placeholder_found {
                // Prepend left as first argument.
                let mut new_args = vec![left.as_ref()];
                for arg in args {
                    new_args.push(arg);
                }
                call_args = new_args;
            }

            for arg in &call_args {
                compile_expr(fc, arg)?;
            }

            let arg_count = call_args.len() as u8;
            if let Some(&idx) = fc.function_map.get(func.as_str()) {
                fc.emit(Op::Call(idx, arg_count));
            } else if let Some(&idx) = fc.native_map.get(func.as_str()) {
                fc.emit(Op::CallNative(idx, arg_count));
            } else {
                return Err(CompileError {
                    message: format!("undefined function: {}", func),
                    span: *span,
                });
            }
        }

        Expr::Yield { value, .. } => {
            compile_expr(fc, value)?;
            fc.emit(Op::Yield);
        }

        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            compile_expr(fc, condition)?;
            let if_addr = fc.emit_jump(Op::If(0));
            compile_block(fc, then_block)?;
            if let Some(else_blk) = else_block {
                let else_addr = fc.emit_jump(Op::Else(0));
                fc.patch_jump(if_addr);
                compile_block(fc, else_blk)?;
                fc.patch_jump(else_addr);
                fc.emit(Op::EndIf);
            } else {
                let else_addr = fc.emit_jump(Op::Else(0));
                fc.patch_jump(if_addr);
                fc.emit(Op::PushUnit);
                fc.patch_jump(else_addr);
                fc.emit(Op::EndIf);
            }
        }

        Expr::Match {
            scrutinee, arms, ..
        } => {
            compile_expr(fc, scrutinee)?;
            let temp = fc.declare_local("__match");
            fc.emit(Op::SetLocal(temp));

            // Wrap match in a virtual Loop so arms can Break to exit.
            let loop_addr = fc.emit(Op::Loop(0));
            fc.enter_loop();

            for arm in arms {
                fc.begin_scope();

                let fail_addrs = compile_pattern_test(fc, &arm.pattern, temp)?;
                compile_pattern_bind(fc, &arm.pattern, temp)?;
                compile_expr(fc, &arm.expr)?;

                // Break out of virtual loop (arm matched, result on stack).
                let break_addr = fc.emit(Op::Break(0));
                if let Some(breaks) = fc.loop_breaks.last_mut() {
                    breaks.push(break_addr);
                }

                fc.end_scope();

                // Close If blocks from pattern tests in reverse order.
                for addr in fail_addrs.into_iter().rev() {
                    fc.patch_jump(addr);
                    fc.emit(Op::EndIf);
                }
            }

            // No arm matched.
            let msg = fc.add_string_constant("no matching arm in match expression");
            fc.emit(Op::Trap(msg));

            let endloop_addr = fc.emit(Op::EndLoop(0));

            // Patch EndLoop back-edge to instruction after Loop.
            let after_loop = (loop_addr + 1) as u32;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            // Patch Loop to past EndLoop, and patch all Break addresses.
            let after_endloop = fc.chunk.ops.len() as u32;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            fc.exit_loop();
        }

        Expr::Loop { body, .. } => {
            let loop_addr = fc.emit(Op::Loop(0));
            fc.enter_loop();

            compile_block(fc, body)?;
            fc.emit(Op::Pop); // Discard block value.

            let endloop_addr = fc.emit(Op::EndLoop(0));

            // Patch EndLoop back-edge to instruction after Loop.
            let after_loop = (loop_addr + 1) as u32;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            // Patch Loop to past EndLoop, and patch all Break addresses.
            let after_endloop = fc.chunk.ops.len() as u32;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            fc.exit_loop();

            // Loop expression evaluates to Unit after break.
            fc.emit(Op::PushUnit);
        }

        Expr::FieldAccess {
            object,
            field,
            span,
        } => {
            // Check if this is a data block field access.
            if let Expr::Ident { name, .. } = object.as_ref()
                && fc.is_data_block(name)
            {
                let slot = fc
                    .resolve_data_field(name, field)
                    .ok_or_else(|| CompileError {
                        message: format!("unknown data field: {}.{}", name, field),
                        span: *span,
                    })?;
                fc.emit(Op::GetData(slot));
                return Ok(());
            }
            compile_expr(fc, object)?;
            let name_const = fc.add_string_constant(field);
            fc.emit(Op::GetField(name_const));
        }

        Expr::TupleIndex { object, index, .. } => {
            compile_expr(fc, object)?;
            fc.emit(Op::GetTupleField(*index as u8));
        }

        Expr::ArrayIndex { object, index, .. } => {
            compile_expr(fc, object)?;
            compile_expr(fc, index)?;
            fc.emit(Op::GetIndex);
        }

        Expr::StructInit { name, fields, .. } => {
            let field_names: Vec<String> = fields.iter().map(|f| f.name.clone()).collect();
            let template_idx = fc.add_struct_template(name, field_names);
            for field in fields {
                compile_expr(fc, &field.value)?;
            }
            fc.emit(Op::NewStruct(template_idx));
        }

        Expr::EnumVariant {
            enum_name,
            variant,
            args,
            ..
        } => {
            for arg in args {
                compile_expr(fc, arg)?;
            }
            let enum_const = fc.add_string_constant(enum_name);
            let var_const = fc.add_string_constant(variant);
            fc.emit(Op::NewEnum(enum_const, var_const, args.len() as u8));
        }

        Expr::ArrayLiteral { elements, .. } => {
            for elem in elements {
                compile_expr(fc, elem)?;
            }
            fc.emit(Op::NewArray(elements.len() as u16));
        }

        Expr::TupleLiteral { elements, .. } => {
            for elem in elements {
                compile_expr(fc, elem)?;
            }
            fc.emit(Op::NewTuple(elements.len() as u8));
        }

        Expr::Cast {
            expr: inner,
            target,
            ..
        } => {
            compile_expr(fc, inner)?;
            match target {
                TypeExpr::Prim(PrimType::F64, _) => {
                    fc.emit(Op::IntToFloat);
                }
                TypeExpr::Prim(PrimType::I64, _) => {
                    fc.emit(Op::FloatToInt);
                }
                _ => {
                    // Other casts are identity at runtime.
                }
            }
        }

        Expr::Placeholder { span } => {
            return Err(CompileError {
                message: String::from("placeholder _ outside of pipeline"),
                span: *span,
            });
        }
        Expr::Closure { span, .. } => {
            return Err(CompileError {
                message: String::from("internal: closure expression encountered after hoist pass"),
                span: *span,
            });
        }
        Expr::ClosureRef {
            name,
            captures,
            recursive,
            span,
        } => {
            let chunk_idx = match fc.function_map.get(name) {
                Some(&i) => i,
                None => {
                    return Err(CompileError {
                        message: format!("internal: synthetic closure name {} not found", name),
                        span: *span,
                    });
                }
            };
            for capture in captures {
                if let Some(slot) = fc.resolve_local(capture) {
                    fc.emit(Op::GetLocal(slot));
                } else if let Some(&fn_idx) = fc.function_map.get(capture) {
                    // The capture refers to a top-level function (or
                    // an already-hoisted synthetic chunk) rather than
                    // a local. Push a non-recursive function value
                    // for it. This case arises when an inner closure
                    // captures an outer closure's binding name.
                    fc.emit(Op::PushFunc(fn_idx));
                } else {
                    return Err(CompileError {
                        message: format!(
                            "closure captures `{}` which is not a local in the enclosing scope",
                            capture
                        ),
                        span: *span,
                    });
                }
            }
            let n = captures.len() as u8;
            if *recursive {
                // For recursive closures, the synthetic chunk's first
                // implicit parameter after the captures is the self
                // reference, populated at each indirect-call by the
                // runtime. The compiler emits the recursive variant
                // even when the captures list is empty so the runtime
                // knows to push the closure value into the self slot.
                fc.emit(Op::MakeRecursiveClosure(chunk_idx, n));
            } else if n == 0 {
                fc.emit(Op::PushFunc(chunk_idx));
            } else {
                fc.emit(Op::MakeClosure(chunk_idx, n));
            }
        }
    }
    Ok(())
}

/// Compile a function call by name.
fn compile_call(
    fc: &mut FuncCompiler,
    name: &str,
    args: &[Expr],
    span: &Span,
) -> Result<(), CompileError> {
    // Indirect call through a local variable holding a `Value::Func`.
    // The compiler pushes the function value, then the explicit
    // arguments, then emits `Op::CallIndirect(arg_count)`. The
    // runtime pops the args plus the func value before invoking.
    if let Some(slot) = fc.resolve_local(name) {
        fc.emit(Op::GetLocal(slot));
        for arg in args {
            compile_expr(fc, arg)?;
        }
        fc.emit(Op::CallIndirect(args.len() as u8));
        return Ok(());
    }
    for arg in args {
        compile_expr(fc, arg)?;
    }
    let arg_count = args.len() as u8;

    if let Some(&idx) = fc.function_map.get(name) {
        fc.emit(Op::Call(idx, arg_count));
    } else if let Some(&idx) = fc.native_map.get(name) {
        fc.emit(Op::CallNative(idx, arg_count));
    } else {
        return Err(CompileError {
            message: format!("undefined function: {}", name),
            span: *span,
        });
    }
    Ok(())
}

/// Compile a pattern test. Returns addresses of If instructions that need
/// patching to the "fail" destination (EndIf at the next arm or error).
fn compile_pattern_test(
    fc: &mut FuncCompiler,
    pattern: &Pattern,
    value_slot: u16,
) -> Result<Vec<usize>, CompileError> {
    let mut fail_addrs = Vec::new();

    match pattern {
        Pattern::Variable(_, _) | Pattern::Wildcard(_) => {
            // Always matches.
        }
        Pattern::Literal(lit, _) => {
            fc.emit(Op::GetLocal(value_slot));
            match lit {
                Literal::Int(v) => {
                    let idx = fc.add_constant(Value::Int(*v));
                    fc.emit(Op::Const(idx));
                }
                Literal::Float(v) => {
                    let idx = fc.add_constant(Value::Float(*v));
                    fc.emit(Op::Const(idx));
                }
                Literal::String(s) => {
                    let idx = fc.add_constant(Value::StaticStr(s.clone()));
                    fc.emit(Op::Const(idx));
                }
                Literal::Bool(true) => {
                    fc.emit(Op::PushTrue);
                }
                Literal::Bool(false) => {
                    fc.emit(Op::PushFalse);
                }
                Literal::Unit => {
                    fc.emit(Op::PushUnit);
                }
            }
            fc.emit(Op::CmpEq);
            fail_addrs.push(fc.emit_jump(Op::If(0)));
        }
        Pattern::Enum(enum_name, variant, sub_pats, _) => {
            fc.emit(Op::GetLocal(value_slot));
            let e_const = fc.add_string_constant(enum_name);
            let v_const = fc.add_string_constant(variant);
            fc.emit(Op::IsEnum(e_const, v_const));
            fail_addrs.push(fc.emit_jump(Op::If(0)));
            fc.emit(Op::Pop); // Discard the peeked value.

            // Test sub-patterns on extracted fields.
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                if matches!(sub_pat, Pattern::Variable(_, _) | Pattern::Wildcard(_)) {
                    continue; // Always matches; will bind during bind phase.
                }
                let temp = fc.declare_local(&format!("__enum_field{}", i));
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetEnumField(i as u8));
                fc.emit(Op::SetLocal(temp));
                let sub_fails = compile_pattern_test(fc, sub_pat, temp)?;
                fail_addrs.extend(sub_fails);
            }
        }
        Pattern::Struct(type_name, field_pats, _) => {
            fc.emit(Op::GetLocal(value_slot));
            let t_const = fc.add_string_constant(type_name);
            fc.emit(Op::IsStruct(t_const));
            fail_addrs.push(fc.emit_jump(Op::If(0)));
            fc.emit(Op::Pop);

            for field_pat in field_pats {
                if let Some(pat) = &field_pat.pattern {
                    if matches!(pat, Pattern::Variable(_, _) | Pattern::Wildcard(_)) {
                        continue;
                    }
                    let temp = fc.declare_local(&format!("__struct_{}", field_pat.name));
                    fc.emit(Op::GetLocal(value_slot));
                    let name_const = fc.add_string_constant(&field_pat.name);
                    fc.emit(Op::GetField(name_const));
                    fc.emit(Op::SetLocal(temp));
                    let sub_fails = compile_pattern_test(fc, pat, temp)?;
                    fail_addrs.extend(sub_fails);
                }
            }
        }
        Pattern::Tuple(pats, _) => {
            for (i, pat) in pats.iter().enumerate() {
                if matches!(pat, Pattern::Variable(_, _) | Pattern::Wildcard(_)) {
                    continue;
                }
                let temp = fc.declare_local(&format!("__tuple_{}", i));
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetTupleField(i as u8));
                fc.emit(Op::SetLocal(temp));
                let sub_fails = compile_pattern_test(fc, pat, temp)?;
                fail_addrs.extend(sub_fails);
            }
        }
    }

    Ok(fail_addrs)
}

/// Compile pattern bindings: extract values and store in local variables.
fn compile_pattern_bind(
    fc: &mut FuncCompiler,
    pattern: &Pattern,
    value_slot: u16,
) -> Result<(), CompileError> {
    compile_pattern_bind_typed(fc, pattern, value_slot, None)
}

/// Compile a pattern bind with the value's known type expression.
///
/// The type, when present, is recorded on `Pattern::Variable` bindings
/// so downstream optimizations such as the for-in iteration bound
/// inference can consult it. For composite patterns (Tuple, Enum,
/// Struct) the type is decomposed structurally where the AST permits.
/// Patterns whose type cannot be statically decomposed propagate
/// `None` to inner binds.
fn compile_pattern_bind_typed(
    fc: &mut FuncCompiler,
    pattern: &Pattern,
    value_slot: u16,
    ty: Option<TypeExpr>,
) -> Result<(), CompileError> {
    match pattern {
        Pattern::Variable(name, _) => {
            fc.emit(Op::GetLocal(value_slot));
            let slot = fc.declare_local_typed(name, ty);
            fc.emit(Op::SetLocal(slot));
        }
        Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
            // Nothing to bind.
        }
        Pattern::Enum(enum_name, variant, sub_pats, _) => {
            // For enum sub-pattern bindings, look up the variant's
            // payload types from the type info when available.
            let payload_types: Vec<Option<TypeExpr>> = fc
                .type_info
                .enums
                .get(enum_name)
                .and_then(|variants| variants.get(variant))
                .map(|tys| tys.iter().cloned().map(Some).collect())
                .unwrap_or_else(|| sub_pats.iter().map(|_| None).collect());
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                if matches!(sub_pat, Pattern::Wildcard(_) | Pattern::Literal(_, _)) {
                    continue;
                }
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetEnumField(i as u8));
                let sub_ty = payload_types.get(i).cloned().unwrap_or(None);
                if let Pattern::Variable(name, _) = sub_pat {
                    let slot = fc.declare_local_typed(name, sub_ty);
                    fc.emit(Op::SetLocal(slot));
                } else {
                    let temp = fc.declare_local(&format!("__bind_tmp{}", i));
                    fc.emit(Op::SetLocal(temp));
                    compile_pattern_bind_typed(fc, sub_pat, temp, sub_ty)?;
                }
            }
        }
        Pattern::Struct(struct_name, field_pats, _) => {
            // Look up field types for nested pattern bindings.
            let field_types: BTreeMap<String, TypeExpr> = fc
                .type_info
                .structs
                .get(struct_name)
                .cloned()
                .unwrap_or_default();
            for field_pat in field_pats {
                let name_const = fc.add_string_constant(&field_pat.name);
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetField(name_const));
                let field_ty = field_types.get(&field_pat.name).cloned();
                if let Some(pat) = &field_pat.pattern {
                    if let Pattern::Variable(vname, _) = pat {
                        let slot = fc.declare_local_typed(vname, field_ty);
                        fc.emit(Op::SetLocal(slot));
                    } else if matches!(pat, Pattern::Wildcard(_)) {
                        fc.emit(Op::Pop);
                    } else {
                        let temp = fc.declare_local(&format!("__sf_{}", field_pat.name));
                        fc.emit(Op::SetLocal(temp));
                        compile_pattern_bind_typed(fc, pat, temp, field_ty)?;
                    }
                } else {
                    let slot = fc.declare_local_typed(&field_pat.name, field_ty);
                    fc.emit(Op::SetLocal(slot));
                }
            }
        }
        Pattern::Tuple(pats, _) => {
            // Decompose the tuple type structurally if present.
            let elem_types: Vec<Option<TypeExpr>> = match &ty {
                Some(TypeExpr::Tuple(ts, _)) if ts.len() == pats.len() => {
                    ts.iter().cloned().map(Some).collect()
                }
                _ => pats.iter().map(|_| None).collect(),
            };
            for (i, pat) in pats.iter().enumerate() {
                if matches!(pat, Pattern::Wildcard(_) | Pattern::Literal(_, _)) {
                    continue;
                }
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetTupleField(i as u8));
                let sub_ty = elem_types.get(i).cloned().unwrap_or(None);
                if let Pattern::Variable(name, _) = pat {
                    let slot = fc.declare_local_typed(name, sub_ty);
                    fc.emit(Op::SetLocal(slot));
                } else {
                    let temp = fc.declare_local(&format!("__tup_bind{}", i));
                    fc.emit(Op::SetLocal(temp));
                    compile_pattern_bind_typed(fc, pat, temp, sub_ty)?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn compile_str(src: &str) -> Result<Module, CompileError> {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        compile(&program)
    }

    #[test]
    fn compile_simple_fn() {
        let module = compile_str("fn add(a: i64, b: i64) -> i64 { a + b }").unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert_eq!(module.chunks[0].name, "add");
        assert_eq!(module.chunks[0].param_count, 2);
    }

    #[test]
    fn compile_literal_fn() {
        let module = compile_str("fn fortytwo() -> i64 { 42 }").unwrap();
        assert_eq!(module.chunks.len(), 1);
        // Should have a Const instruction and Return.
        assert!(module.chunks[0].ops.contains(&Op::Return));
    }

    #[test]
    fn compile_if_else() {
        let module =
            compile_str("fn max(a: i64, b: i64) -> i64 { if a > b { a } else { b } }").unwrap();
        assert_eq!(module.chunks.len(), 1);
        // Should contain If for the condition.
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::If(_)))
        );
    }

    #[test]
    fn compile_let_binding() {
        let module = compile_str("fn double(x: i64) -> i64 { let y = x * 2; y }").unwrap();
        assert_eq!(module.chunks.len(), 1);
    }

    #[test]
    fn compile_for_range() {
        let module = compile_str(
            "fn sum_to(n: i64) -> i64 { let total = 0; for i in 0..n { let x = total + i; } total }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
        // Should contain Loop/EndLoop for the for-range.
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::Loop(_)))
        );
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::EndLoop(_)))
        );
    }

    #[test]
    fn compile_function_call() {
        let module = compile_str(
            "fn double(x: i64) -> i64 { x * 2 }\nfn quad(x: i64) -> i64 { double(double(x)) }",
        )
        .unwrap();
        assert_eq!(module.chunks.len(), 2);
        // quad should contain Call instructions.
        let quad = &module.chunks[1];
        assert!(quad.ops.iter().any(|op| matches!(op, Op::Call(_, 1))));
    }

    #[test]
    fn compile_multiheaded() {
        let module = compile_str(
            "fn classify(0) -> String { \"zero\" }\nfn classify(x: i64) -> String { \"other\" }",
        )
        .unwrap();
        // Two heads compiled into one chunk.
        assert_eq!(module.chunks.len(), 1);
    }

    #[test]
    fn compile_enum_variant() {
        let module = compile_str(
            "enum Color { Red, Green, Blue }\nfn make() -> Color { let x = Color::Red(); x }",
        )
        .unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::NewEnum(_, _, 0)))
        );
    }

    #[test]
    fn compile_struct_init() {
        let module = compile_str(
            "struct Point { x: i64, y: i64 }\nfn make() -> Point { let p = Point { x: 1, y: 2 }; p }",
        )
        .unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::NewStruct(_)))
        );
    }

    #[test]
    fn compile_yield_function() {
        let module = compile_str("yield process(input: i64) -> i64 { yield input * 2 }").unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::Yield))
        );
        assert_eq!(module.chunks[0].block_type, BlockType::Reentrant);
    }

    #[test]
    fn compile_loop_function() {
        let module =
            compile_str("loop main(input: i64) -> i64 { let input = yield input + 1; input }")
                .unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert_eq!(module.chunks[0].block_type, BlockType::Stream);
        // Should contain Stream and Reset instructions.
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::Stream))
        );
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::Reset))
        );
    }

    #[test]
    fn compile_entry_point() {
        let module = compile_str("fn main(x: i64) -> i64 { x }").unwrap();
        assert!(module.entry_point.is_some());
    }

    #[test]
    fn compile_pipeline() {
        let module = compile_str(
            "fn double(x: i64) -> i64 { x * 2 }\nfn apply(x: i64) -> i64 { x |> double() }",
        )
        .unwrap();
        assert_eq!(module.chunks.len(), 2);
    }

    #[test]
    fn error_undefined_variable() {
        let result = compile_str("fn bad() -> i64 { unknown }");
        assert!(result.is_err());
    }

    #[test]
    fn error_undefined_function() {
        let result = compile_str("fn bad() -> i64 { missing(1) }");
        assert!(result.is_err());
    }

    #[test]
    fn error_break_outside_loop() {
        let result = compile_str("fn bad() -> () { break; }");
        assert!(result.is_err());
    }

    #[test]
    fn compile_for_in_array() {
        let module =
            compile_str("fn main() -> i64 { let s = 0; for x in [1, 2, 3] { let s = s + x; } s }")
                .unwrap();
        assert_eq!(module.chunks.len(), 1);
        // Should contain Loop/EndLoop for the for-in.
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::Loop(_)))
        );
        // For an array literal, the for-in iteration bound is known
        // statically (3), so the compiler emits a Const for the end
        // bound rather than `Op::Len`. The strict-mode WCMU verifier
        // accepts this pattern.
        assert!(!module.chunks[0].ops.iter().any(|op| matches!(op, Op::Len)));
        // Should contain GetIndex.
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::GetIndex))
        );
    }

    #[test]
    fn compile_tuple_literal() {
        let module = compile_str("fn main() -> (i64, i64, i64) { let t = (1, 2, 3); t }").unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::NewTuple(3)))
        );
    }

    #[test]
    fn compile_block_structured_control() {
        // Verify no flat jumps are emitted.
        let module = compile_str("fn main() -> i64 { if true { 1 } else { 2 } }").unwrap();
        for op in &module.chunks[0].ops {
            assert!(
                !matches!(
                    op,
                    Op::Loop(_) | Op::EndLoop(_) | Op::Break(_) | Op::BreakIf(_)
                ),
                "unexpected loop instruction in simple if/else"
            );
        }
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::If(_)))
        );
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::Else(_)))
        );
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::EndIf))
        );
    }

    // -- Data segment conformance tests --

    #[test]
    fn data_block_admits_primitives() {
        let src = "data ctx { score: i64, level: i64, ratio: f64, alive: bool }\n\
                   fn main() -> i64 { ctx.score }";
        let module = compile_str(src).unwrap();
        let layout = module.data_layout.expect("expected data layout");
        assert_eq!(layout.slots.len(), 4);
    }

    #[test]
    fn data_block_admits_unit() {
        let src = "data ctx { tick: () }\n\
                   fn main() -> () { ctx.tick }";
        let module = compile_str(src).unwrap();
        assert!(module.data_layout.is_some());
    }

    #[test]
    fn data_block_admits_tuple_of_admissible() {
        let src = "data ctx { pos: (f64, f64) }\n\
                   fn main() -> (f64, f64) { ctx.pos }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_array_of_admissible() {
        let src = "data ctx { samples: [f64; 4] }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_option_of_admissible() {
        let src = "data ctx { last: Option<i64> }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_struct_of_admissible() {
        let src = "struct Point { x: f64, y: f64 }\n\
                   data ctx { origin: Point }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_enum_of_admissible() {
        let src = "enum Status { Idle, Active(i64), Error(i64, i64) }\n\
                   data ctx { state: Status }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_rejects_string() {
        let src = "data ctx { name: String }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("String"));
    }

    #[test]
    fn data_block_rejects_string_in_tuple() {
        let src = "data ctx { pair: (i64, String) }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("String"));
    }

    #[test]
    fn data_block_rejects_string_in_array() {
        let src = "data ctx { names: [String; 4] }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("String"));
    }

    #[test]
    fn data_block_rejects_string_in_option() {
        let src = "data ctx { last: Option<String> }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("String"));
    }

    #[test]
    fn data_block_rejects_string_in_struct() {
        let src = "struct Tag { label: String }\n\
                   data ctx { t: Tag }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("String"));
    }

    #[test]
    fn data_block_rejects_string_in_enum() {
        let src = "enum Tag { Named(String), Unnamed }\n\
                   data ctx { t: Tag }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("String"));
    }

    #[test]
    fn data_block_rejects_unknown_named_type() {
        let src = "data ctx { handle: Mystery }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Mystery") || err.message.contains("opaque"));
    }

    #[test]
    fn multiple_data_blocks_rejected() {
        let src = "data ctx_a { x: i64 }\n\
                   data ctx_b { y: i64 }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("R28") || err.message.contains("one data block"));
    }

    #[test]
    fn no_data_block_compiles() {
        let module = compile_str("fn main() -> i64 { 42 }").unwrap();
        assert!(module.data_layout.is_none());
    }
}
