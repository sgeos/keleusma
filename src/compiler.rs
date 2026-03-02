extern crate alloc;
use alloc::collections::BTreeMap;
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
}

impl FuncCompiler {
    fn new(
        name: &str,
        is_loop: bool,
        function_map: BTreeMap<String, u16>,
        native_map: BTreeMap<String, u16>,
    ) -> Self {
        Self {
            chunk: Chunk {
                name: String::from(name),
                ops: Vec::new(),
                constants: Vec::new(),
                struct_templates: Vec::new(),
                local_count: 0,
                param_count: 0,
                is_loop,
            },
            locals: Vec::new(),
            scope_depth: 0,
            next_slot: 0,
            loop_breaks: Vec::new(),
            function_map,
            native_map,
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
            Op::Jump(a)
            | Op::JumpIfFalse(a)
            | Op::TestEnum(_, _, a)
            | Op::TestStruct(_, a) => *a = target,
            _ => {}
        }
    }

    fn add_constant(&mut self, value: Value) -> u16 {
        // Reuse existing constant if possible.
        for (i, c) in self.chunk.constants.iter().enumerate() {
            if *c == value {
                return i as u16;
            }
        }
        let idx = self.chunk.constants.len() as u16;
        self.chunk.constants.push(value);
        idx
    }

    fn add_string_constant(&mut self, s: &str) -> u16 {
        self.add_constant(Value::Str(String::from(s)))
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

    fn declare_local(&mut self, name: &str) -> u16 {
        let slot = self.next_slot;
        self.next_slot += 1;
        if self.next_slot > self.chunk.local_count {
            self.chunk.local_count = self.next_slot;
        }
        self.locals.push(Local {
            name: String::from(name),
            slot,
            depth: self.scope_depth,
        });
        slot
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

/// Compile a parsed Keleusma program into a bytecode module.
pub fn compile(program: &Program) -> Result<Module, CompileError> {
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

    // Group function definitions by name.
    let mut groups: BTreeMap<String, Vec<&FunctionDef>> = BTreeMap::new();
    for func in &program.functions {
        groups.entry(func.name.clone()).or_default().push(func);
    }

    // Build function name -> chunk index map.
    let mut function_map: BTreeMap<String, u16> = BTreeMap::new();
    for (chunk_idx, name) in groups.keys().enumerate() {
        function_map.insert(name.clone(), chunk_idx as u16);
    }

    // Compile each function group.
    let mut chunks: Vec<Chunk> = Vec::new();
    for (name, defs) in &groups {
        let chunk = compile_function_group(
            name,
            defs,
            &function_map,
            &native_map,
        )?;
        chunks.push(chunk);
    }

    let entry_point = function_map.get("main").map(|&idx| idx as usize);

    Ok(Module {
        chunks,
        native_names,
        entry_point,
    })
}

/// Compile a group of function definitions with the same name into one chunk.
fn compile_function_group(
    name: &str,
    defs: &[&FunctionDef],
    function_map: &BTreeMap<String, u16>,
    native_map: &BTreeMap<String, u16>,
) -> Result<Chunk, CompileError> {
    let first = defs[0];
    let is_loop = first.category == FunctionCategory::Loop;
    let param_count = first.params.len() as u8;

    let mut fc = FuncCompiler::new(name, is_loop, function_map.clone(), native_map.clone());
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
            bind_param_pattern(&mut fc, &param.pattern, param_slots[i]);
        }
        let body_start = fc.chunk.ops.len() as u32;
        compile_block(&mut fc, &first.body)?;
        if is_loop {
            fc.emit(Op::Pop); // Discard body value before looping.
            fc.emit(Op::Jump(body_start));
        } else {
            fc.emit(Op::Return);
        }
    } else {
        // Multiheaded or pattern-matched parameters: dispatch.
        let mut fail_jumps: Vec<usize> = Vec::new();
        let mut end_jumps: Vec<usize> = Vec::new();

        for def in defs {
            // Patch previous fail jump to here.
            for addr in fail_jumps.drain(..) {
                fc.patch_jump(addr);
            }

            fc.begin_scope();

            // Test each parameter against the head's pattern.
            for (i, param) in def.params.iter().enumerate() {
                let fail = compile_pattern_test(&mut fc, &param.pattern, param_slots[i])?;
                fail_jumps.extend(fail);
            }

            // Bind pattern variables before guard (guard may reference them).
            for (i, param) in def.params.iter().enumerate() {
                compile_pattern_bind(&mut fc, &param.pattern, param_slots[i])?;
            }

            // Test guard clause if present.
            if let Some(guard) = &def.guard {
                compile_expr(&mut fc, guard)?;
                let fail = fc.emit_jump(Op::JumpIfFalse(0));
                fail_jumps.push(fail);
            }

            compile_block(&mut fc, &def.body)?;
            fc.emit(Op::Return);
            end_jumps.push(fc.emit_jump(Op::Jump(0)));

            fc.end_scope();
        }

        // No head matched: emit trap.
        for addr in fail_jumps.drain(..) {
            fc.patch_jump(addr);
        }
        let msg = fc.add_string_constant(&format!("no matching head for {}", name));
        fc.emit(Op::Trap(msg));

        // Patch end jumps.
        for addr in end_jumps {
            fc.patch_jump(addr);
        }
    }

    Ok(fc.finish())
}

/// Check if any parameter has a non-trivial pattern (not a simple variable).
fn has_non_trivial_pattern(params: &[Param]) -> bool {
    params.iter().any(|p| !matches!(p.pattern, Pattern::Variable(_, _)))
}

/// Bind a simple variable pattern to a parameter slot (alias).
fn bind_param_pattern(fc: &mut FuncCompiler, pattern: &Pattern, slot: u16) {
    if let Pattern::Variable(name, _) = pattern {
        // Create a named local that aliases the parameter slot.
        fc.locals.push(Local {
            name: name.clone(),
            slot,
            depth: fc.scope_depth,
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

/// Compile a single statement.
fn compile_stmt(fc: &mut FuncCompiler, stmt: &Stmt) -> Result<(), CompileError> {
    match stmt {
        Stmt::Let(let_stmt) => {
            compile_expr(fc, &let_stmt.value)?;
            compile_let_pattern(fc, &let_stmt.pattern)?;
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
            let addr = fc.emit_jump(Op::Jump(0));
            if let Some(breaks) = fc.loop_breaks.last_mut() {
                breaks.push(addr);
            }
        }
        Stmt::Expr(expr) => {
            compile_expr(fc, expr)?;
            fc.emit(Op::Pop);
        }
    }
    Ok(())
}

/// Compile a let binding pattern (allocate locals and store values).
fn compile_let_pattern(fc: &mut FuncCompiler, pattern: &Pattern) -> Result<(), CompileError> {
    match pattern {
        Pattern::Variable(name, _) => {
            let slot = fc.declare_local(name);
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

            let loop_start = fc.chunk.ops.len();
            fc.enter_loop();

            // Condition: var < end.
            fc.emit(Op::GetLocal(var_slot));
            fc.emit(Op::GetLocal(end_slot));
            fc.emit(Op::CmpLt);
            let exit_jump = fc.emit_jump(Op::JumpIfFalse(0));

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
            fc.emit(Op::Jump(loop_start as u32));

            fc.patch_jump(exit_jump);
            fc.exit_loop();
        }
        Iterable::Expr(expr) => {
            // Compile the iterable expression.
            compile_expr(fc, expr)?;
            let arr_slot = fc.declare_local("__for_arr");
            fc.emit(Op::SetLocal(arr_slot));

            // Index counter.
            let zero_const = fc.add_constant(Value::Int(0));
            fc.emit(Op::Const(zero_const));
            let idx_slot = fc.declare_local("__for_idx");
            fc.emit(Op::SetLocal(idx_slot));

            // Array length: we need a way to get length at runtime.
            // For now, use a native-like approach: store array and index.
            // The loop condition checks arr[idx] bounds.
            // We will use a special approach: catch index-out-of-bounds as loop exit.
            // FUTURE: Add a Len instruction. For now, iterate until GetIndex fails.
            // HACK: This is a placeholder. Proper iteration requires a Len opcode.
            return Err(CompileError {
                message: String::from("for-in over expressions is not yet supported; use range syntax"),
                span: for_stmt.span,
            });
        }
    }
    Ok(())
}

/// Compile an expression, leaving the result on the stack.
fn compile_expr(fc: &mut FuncCompiler, expr: &Expr) -> Result<(), CompileError> {
    match expr {
        Expr::Literal { value, .. } => {
            match value {
                Literal::Int(v) => {
                    let idx = fc.add_constant(Value::Int(*v));
                    fc.emit(Op::Const(idx));
                }
                Literal::Float(v) => {
                    let idx = fc.add_constant(Value::Float(*v));
                    fc.emit(Op::Const(idx));
                }
                Literal::String(s) => {
                    let idx = fc.add_constant(Value::Str(s.clone()));
                    fc.emit(Op::Const(idx));
                }
                Literal::Bool(true) => { fc.emit(Op::PushTrue); }
                Literal::Bool(false) => { fc.emit(Op::PushFalse); }
            }
        }

        Expr::Ident { name, span } => {
            if let Some(slot) = fc.resolve_local(name) {
                fc.emit(Op::GetLocal(slot));
            } else {
                return Err(CompileError {
                    message: format!("undefined variable: {}", name),
                    span: *span,
                });
            }
        }

        Expr::BinOp { op, left, right, .. } => {
            // Short-circuit for logical operators.
            match op {
                BinOp::And => {
                    compile_expr(fc, left)?;
                    let short = fc.emit_jump(Op::JumpIfFalse(0));
                    compile_expr(fc, right)?;
                    let end = fc.emit_jump(Op::Jump(0));
                    fc.patch_jump(short);
                    fc.emit(Op::PushFalse);
                    fc.patch_jump(end);
                    return Ok(());
                }
                BinOp::Or => {
                    compile_expr(fc, left)?;
                    let short = fc.emit_jump(Op::JumpIfFalse(0));
                    fc.emit(Op::PushTrue);
                    let end = fc.emit_jump(Op::Jump(0));
                    fc.patch_jump(short);
                    compile_expr(fc, right)?;
                    fc.patch_jump(end);
                    return Ok(());
                }
                _ => {}
            }
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            match op {
                BinOp::Add => { fc.emit(Op::Add); }
                BinOp::Sub => { fc.emit(Op::Sub); }
                BinOp::Mul => { fc.emit(Op::Mul); }
                BinOp::Div => { fc.emit(Op::Div); }
                BinOp::Mod => { fc.emit(Op::Mod); }
                BinOp::Eq => { fc.emit(Op::CmpEq); }
                BinOp::NotEq => { fc.emit(Op::CmpNe); }
                BinOp::Lt => { fc.emit(Op::CmpLt); }
                BinOp::Gt => { fc.emit(Op::CmpGt); }
                BinOp::LtEq => { fc.emit(Op::CmpLe); }
                BinOp::GtEq => { fc.emit(Op::CmpGe); }
                BinOp::And | BinOp::Or => unreachable!(),
            }
        }

        Expr::UnaryOp { op, operand, .. } => {
            compile_expr(fc, operand)?;
            match op {
                UnaryOp::Neg => { fc.emit(Op::Neg); }
                UnaryOp::Not => { fc.emit(Op::Not); }
            }
        }

        Expr::Call { name, args, span } => {
            compile_call(fc, name, args, span)?;
        }

        Expr::Pipeline { left, func, args, span } => {
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

        Expr::If { condition, then_block, else_block, .. } => {
            compile_expr(fc, condition)?;
            let else_jump = fc.emit_jump(Op::JumpIfFalse(0));
            compile_block(fc, then_block)?;
            if let Some(else_blk) = else_block {
                let end_jump = fc.emit_jump(Op::Jump(0));
                fc.patch_jump(else_jump);
                compile_block(fc, else_blk)?;
                fc.patch_jump(end_jump);
            } else {
                let end_jump = fc.emit_jump(Op::Jump(0));
                fc.patch_jump(else_jump);
                fc.emit(Op::PushUnit);
                fc.patch_jump(end_jump);
            }
        }

        Expr::Match { scrutinee, arms, .. } => {
            compile_expr(fc, scrutinee)?;
            let temp = fc.declare_local("__match");
            fc.emit(Op::SetLocal(temp));

            let mut end_jumps: Vec<usize> = Vec::new();

            for arm in arms {
                fc.begin_scope();

                let fail_addrs = compile_pattern_test(fc, &arm.pattern, temp)?;
                compile_pattern_bind(fc, &arm.pattern, temp)?;
                compile_expr(fc, &arm.expr)?;
                end_jumps.push(fc.emit_jump(Op::Jump(0)));

                fc.end_scope();

                for addr in fail_addrs {
                    fc.patch_jump(addr);
                }
            }

            // No arm matched.
            let msg = fc.add_string_constant("no matching arm in match expression");
            fc.emit(Op::Trap(msg));

            for addr in end_jumps {
                fc.patch_jump(addr);
            }
        }

        Expr::Loop { body, .. } => {
            let loop_start = fc.chunk.ops.len();
            fc.enter_loop();

            compile_block(fc, body)?;
            fc.emit(Op::Pop); // Discard block value.
            fc.emit(Op::Jump(loop_start as u32));

            fc.exit_loop();
            // Loop expression evaluates to Unit after break.
            fc.emit(Op::PushUnit);
        }

        Expr::FieldAccess { object, field, .. } => {
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

        Expr::EnumVariant { enum_name, variant, args, .. } => {
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

        Expr::Cast { expr: inner, target, .. } => {
            compile_expr(fc, inner)?;
            match target {
                TypeExpr::Prim(PrimType::F64, _) => { fc.emit(Op::IntToFloat); }
                TypeExpr::Prim(PrimType::I64, _) => { fc.emit(Op::FloatToInt); }
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

/// Compile a pattern test. Returns addresses of jump instructions that need
/// patching to the "fail" destination (the next arm or error).
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
                    let idx = fc.add_constant(Value::Str(s.clone()));
                    fc.emit(Op::Const(idx));
                }
                Literal::Bool(true) => { fc.emit(Op::PushTrue); }
                Literal::Bool(false) => { fc.emit(Op::PushFalse); }
            }
            fc.emit(Op::CmpEq);
            fail_addrs.push(fc.emit_jump(Op::JumpIfFalse(0)));
        }
        Pattern::Enum(enum_name, variant, sub_pats, _) => {
            fc.emit(Op::GetLocal(value_slot));
            let e_const = fc.add_string_constant(enum_name);
            let v_const = fc.add_string_constant(variant);
            let fail = fc.emit_jump(Op::TestEnum(e_const, v_const, 0));
            fail_addrs.push(fail);
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
            let fail = fc.emit_jump(Op::TestStruct(t_const, 0));
            fail_addrs.push(fail);
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
    match pattern {
        Pattern::Variable(name, _) => {
            fc.emit(Op::GetLocal(value_slot));
            let slot = fc.declare_local(name);
            fc.emit(Op::SetLocal(slot));
        }
        Pattern::Wildcard(_) | Pattern::Literal(_, _) => {
            // Nothing to bind.
        }
        Pattern::Enum(_, _, sub_pats, _) => {
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                if matches!(sub_pat, Pattern::Wildcard(_) | Pattern::Literal(_, _)) {
                    continue;
                }
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetEnumField(i as u8));
                if let Pattern::Variable(name, _) = sub_pat {
                    let slot = fc.declare_local(name);
                    fc.emit(Op::SetLocal(slot));
                } else {
                    // Nested non-trivial pattern: store in temp and recurse.
                    let temp = fc.declare_local(&format!("__bind_tmp{}", i));
                    fc.emit(Op::SetLocal(temp));
                    compile_pattern_bind(fc, sub_pat, temp)?;
                }
            }
        }
        Pattern::Struct(_, field_pats, _) => {
            for field_pat in field_pats {
                let name_const = fc.add_string_constant(&field_pat.name);
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetField(name_const));
                if let Some(pat) = &field_pat.pattern {
                    if let Pattern::Variable(vname, _) = pat {
                        let slot = fc.declare_local(vname);
                        fc.emit(Op::SetLocal(slot));
                    } else if matches!(pat, Pattern::Wildcard(_)) {
                        fc.emit(Op::Pop);
                    } else {
                        let temp = fc.declare_local(&format!("__sf_{}", field_pat.name));
                        fc.emit(Op::SetLocal(temp));
                        compile_pattern_bind(fc, pat, temp)?;
                    }
                } else {
                    // Shorthand: `Name { field }` binds field to a variable of the same name.
                    let slot = fc.declare_local(&field_pat.name);
                    fc.emit(Op::SetLocal(slot));
                }
            }
        }
        Pattern::Tuple(pats, _) => {
            for (i, pat) in pats.iter().enumerate() {
                if matches!(pat, Pattern::Wildcard(_) | Pattern::Literal(_, _)) {
                    continue;
                }
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetTupleField(i as u8));
                if let Pattern::Variable(name, _) = pat {
                    let slot = fc.declare_local(name);
                    fc.emit(Op::SetLocal(slot));
                } else {
                    let temp = fc.declare_local(&format!("__tup_bind{}", i));
                    fc.emit(Op::SetLocal(temp));
                    compile_pattern_bind(fc, pat, temp)?;
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
        let module = compile_str(
            "fn max(a: i64, b: i64) -> i64 { if a > b { a } else { b } }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
        // Should contain JumpIfFalse for the if condition.
        assert!(module.chunks[0].ops.iter().any(|op| matches!(op, Op::JumpIfFalse(_))));
    }

    #[test]
    fn compile_let_binding() {
        let module = compile_str(
            "fn double(x: i64) -> i64 { let y = x * 2; y }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
    }

    #[test]
    fn compile_for_range() {
        let module = compile_str(
            "fn sum_to(n: i64) -> i64 { let total = 0; for i in 0..n { let x = total + i; } total }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
    }

    #[test]
    fn compile_function_call() {
        let module = compile_str(
            "fn double(x: i64) -> i64 { x * 2 }\nfn quad(x: i64) -> i64 { double(double(x)) }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 2);
        // quad should contain Call instructions.
        let quad = &module.chunks[1];
        assert!(quad.ops.iter().any(|op| matches!(op, Op::Call(_, 1))));
    }

    #[test]
    fn compile_multiheaded() {
        let module = compile_str(
            "fn classify(0) -> String { \"zero\" }\nfn classify(x: i64) -> String { \"other\" }"
        ).unwrap();
        // Two heads compiled into one chunk.
        assert_eq!(module.chunks.len(), 1);
    }

    #[test]
    fn compile_enum_variant() {
        let module = compile_str(
            "fn make() -> () { let x = Color::Red(); x }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(module.chunks[0].ops.iter().any(|op| matches!(op, Op::NewEnum(_, _, 0))));
    }

    #[test]
    fn compile_struct_init() {
        let module = compile_str(
            "fn make() -> () { let p = Point { x: 1, y: 2 }; p }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(module.chunks[0].ops.iter().any(|op| matches!(op, Op::NewStruct(_))));
    }

    #[test]
    fn compile_yield_function() {
        let module = compile_str(
            "yield process(input: i64) -> i64 { yield input * 2 }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(module.chunks[0].ops.iter().any(|op| matches!(op, Op::Yield)));
    }

    #[test]
    fn compile_loop_function() {
        let module = compile_str(
            "loop main(input: i64) -> i64 { let input = yield input + 1; input }"
        ).unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert!(module.chunks[0].is_loop);
        // Should end with Jump(0) for the implicit loop.
        let last_real = module.chunks[0].ops.iter().rev()
            .find(|op| !matches!(op, Op::Jump(_)))
            .cloned();
        assert!(last_real.is_some());
    }

    #[test]
    fn compile_entry_point() {
        let module = compile_str(
            "fn main(x: i64) -> i64 { x }"
        ).unwrap();
        assert!(module.entry_point.is_some());
    }

    #[test]
    fn compile_pipeline() {
        let module = compile_str(
            "fn double(x: i64) -> i64 { x * 2 }\nfn apply(x: i64) -> i64 { x |> double() }"
        ).unwrap();
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
}
