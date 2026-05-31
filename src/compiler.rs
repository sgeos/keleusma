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
    /// Human-readable diagnostic message.
    pub message: String,
    /// Source span of the offending construct.
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
    /// Enum name to ordered (variant name, discriminant) list.
    /// Used by the enum-to-Word cast and any other site that
    /// needs to walk the variants in declaration order.
    enum_variant_order: BTreeMap<String, Vec<(String, i64)>>,
    /// Function name to declared return type.
    function_returns: BTreeMap<String, TypeExpr>,
    /// Data block name to (field name to declared field type).
    data_field_types: BTreeMap<String, BTreeMap<String, TypeExpr>>,
    /// Names of declared newtypes. The compiler treats a call
    /// expression whose function name is a newtype as a transparent
    /// construction: the inner expression is compiled in place,
    /// without emitting `Op::Call`. The newtype wrapper exists only
    /// at the type-checker level.
    newtype_names: alloc::collections::BTreeSet<String>,
    /// Newtypes with refinement predicates. Maps the newtype name
    /// to the predicate function name. The compiler emits a call
    /// to the predicate at every newtype construction site and
    /// traps if the predicate returns false.
    newtype_refinements: BTreeMap<String, String>,
    /// Cached predicate bodies for the literal-argument elision
    /// pass. Maps the predicate function name to the
    /// `(parameter_name, body_expr)` pair. When a refined newtype
    /// constructor is called with an integer literal that can be
    /// statically evaluated against the predicate body, the
    /// compile-time evaluator decides the outcome and the
    /// runtime predicate call is elided. Predicates without a
    /// single integer parameter or with non-evaluable bodies are
    /// not cached and fall through to the runtime path.
    refinement_bodies: BTreeMap<String, (String, Expr)>,
    /// Per-function return-range summaries used by the
    /// refinement-elision pass. Maps a function name to the
    /// `IntervalSet` covering every value the function might
    /// return. Computed by a fixed-point pass over the function
    /// table at the top of `compile`. Functions whose body
    /// cannot be reduced to an `IntervalSet` under the param-
    /// range substitution are absent; the constructor-emit site
    /// falls through to the runtime check when looking up a
    /// missing summary.
    function_return_ranges: BTreeMap<String, crate::interval::IntervalSet>,
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
    /// Per-native verified-versus-external classification from its
    /// `use` declaration. `true` means the import was declared
    /// `use external module::name` and call sites compile to
    /// `Op::CallExternalNative`; `false` (the default) means a bare
    /// `use module::name` and call sites compile to
    /// `Op::CallVerifiedNative`.
    native_externals: BTreeMap<String, bool>,
    /// Map from data block name to a list of (field_name, slot_index) pairs.
    /// Holds entries for shared and private data only. Const data
    /// fields do not consume runtime slots and are tracked through
    /// `const_fields` instead.
    data_fields: BTreeMap<String, Vec<(String, u16)>>,
    /// Map from data block name to a map from field name to the
    /// compile-time `ConstValue` for `const data` fields. Field
    /// reads resolve through this map first and compile to
    /// `Op::Const`. Field writes against any entry here are
    /// compile errors.
    const_fields: BTreeMap<String, BTreeMap<String, crate::bytecode::ConstValue>>,
    /// Static type information used by the for-in iteration bound
    /// inference and similar narrow optimizations.
    type_info: TypeInfo,
    /// Map from local slot to its compile-time integer value, for
    /// the subset of let-bound locals whose value expression
    /// constant-folds to an integer. Used by the refinement-
    /// elision pass to resolve identifier references in a
    /// constructor's argument. Entries are never removed because
    /// Keleusma's `let` bindings are immutable; a slot's mapping
    /// remains valid for the lifetime of the binding.
    local_const_values: BTreeMap<u16, i64>,
    /// Map from local slot to the inferred range of the value
    /// bound to that slot. Populated for function parameters
    /// whose declared type is a refined newtype: the parameter's
    /// range is the predicate's true set, derived by the
    /// predicate decompiler. The refinement-elision pass uses
    /// this map to admit `Counter(p)` where `p: Counter` (the
    /// argument's range is a subset of the predicate's true set
    /// by construction). Ranges are stored as `IntervalSet` so
    /// non-convex true sets (e.g. predicates with `or`, `!=`, or
    /// `not (x < N)` over a bounded range) compose cleanly.
    local_ranges: BTreeMap<u16, crate::interval::IntervalSet>,
}

impl FuncCompiler {
    #[allow(clippy::too_many_arguments)]
    fn new(
        name: &str,
        block_type: BlockType,
        function_map: BTreeMap<String, u16>,
        native_map: BTreeMap<String, u16>,
        native_externals: BTreeMap<String, bool>,
        data_fields: BTreeMap<String, Vec<(String, u16)>>,
        const_fields: BTreeMap<String, BTreeMap<String, crate::bytecode::ConstValue>>,
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
                param_types: Vec::new(),
            },
            locals: Vec::new(),
            scope_depth: 0,
            next_slot: 0,
            loop_breaks: Vec::new(),
            function_map,
            native_map,
            native_externals,
            data_fields,
            const_fields,
            type_info,
            local_const_values: BTreeMap::new(),
            local_ranges: BTreeMap::new(),
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
                if self.data_fields.contains_key(name) || self.const_fields.contains_key(name) {
                    return Some(name.clone());
                }
                let ty = self.local_type(name)?;
                if let TypeExpr::Named(struct_name, _, _) = ty {
                    return Some(struct_name.clone());
                }
                None
            }
            // Chained field access: resolve the outer owner via the
            // inner field's type. This lets `infer_expr_type` walk
            // expressions like `origin.pt.x` where the field path
            // descends through one struct into another.
            Expr::FieldAccess { object, field, .. } => {
                let owner = self.struct_name_of(object)?;
                let field_ty = self
                    .type_info
                    .structs
                    .get(&owner)
                    .or_else(|| self.type_info.data_field_types.get(&owner))
                    .and_then(|fields| fields.get(field))?;
                if let TypeExpr::Named(struct_name, _, _) = field_ty {
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
        // Cast to `u16` is safe under the post-chunk
        // `enforce_chunk_size_limit` check, which rejects chunks
        // whose op count exceeds `u16::MAX`. Targets within an
        // admissible chunk always fit in `u16`; a truncation here
        // would only fire on a chunk that the post-check
        // immediately rejects.
        let target = self.chunk.ops.len() as u16;
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
        // because reaching this with a `KStr` would be a compiler
        // bug rather than user-visible.
        let cv = ConstValue::try_from_value(value).expect("compile-time constant only");
        self.add_const_value(cv)
    }

    /// Append a [`ConstValue`] to the per-chunk constant pool
    /// with structural deduplication. Returns the index for
    /// `Op::Const`. Used by the const-data field-access path
    /// where the compiler already has the typed `ConstValue`
    /// rather than a runtime `Value`.
    fn add_const_value(&mut self, cv: ConstValue) -> u16 {
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

    /// Resolve a bare-identifier name to its compile-time
    /// integer value if the binding was recorded as a constant
    /// during let-stmt emission. Returns `None` when the name
    /// does not resolve to a known local or when the local's
    /// value did not fold to an integer.
    fn local_const_lookup(&self, name: &str) -> Option<EvalValue> {
        let slot = self.resolve_local(name)?;
        self.local_const_values
            .get(&slot)
            .copied()
            .map(EvalValue::Int)
    }

    /// Check if a name refers to a data block. Returns true for
    /// shared, private, and const data blocks.
    fn is_data_block(&self, name: &str) -> bool {
        self.data_fields.contains_key(name) || self.const_fields.contains_key(name)
    }

    /// Check if a name refers to a const data block.
    fn is_const_data_block(&self, name: &str) -> bool {
        self.const_fields.contains_key(name)
    }

    /// Look up a const data field's compile-time value.
    fn const_data_field_value(
        &self,
        data_name: &str,
        field_name: &str,
    ) -> Option<crate::bytecode::ConstValue> {
        self.const_fields
            .get(data_name)
            .and_then(|m| m.get(field_name))
            .cloned()
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

/// Compute a [`crate::bytecode::TypeTag`] for a function parameter
/// based on its declared type expression. The tag is used by the
/// runtime to validate calls and resumes before any bytecode runs.
///
/// Primitive types map to their corresponding tag. Composite types
/// (struct, enum, tuple, array, option, opaque named types, unit)
/// collapse to [`TypeTag::Composite`] which the runtime accepts
/// without further checking.
fn type_tag_for_param(param: &Param) -> crate::bytecode::TypeTag {
    use crate::bytecode::TypeTag;
    let Some(type_expr) = &param.type_expr else {
        return TypeTag::Composite;
    };
    match type_expr {
        TypeExpr::Prim(PrimType::Byte, _) => TypeTag::Byte,
        TypeExpr::Prim(PrimType::Word, _) => TypeTag::Word,
        TypeExpr::Prim(PrimType::Fixed(_), _) => TypeTag::Fixed,
        TypeExpr::Prim(PrimType::Float, _) => TypeTag::Float,
        TypeExpr::Prim(PrimType::Bool, _) => TypeTag::Bool,
        TypeExpr::Prim(PrimType::Text, _) => TypeTag::Text,
        TypeExpr::Unit(_) => TypeTag::Unit,
        _ => TypeTag::Composite,
    }
}

/// Non-fatal finding produced by [`compile_with_warnings`] that
/// admits the module but signals a potential issue worth surfacing
/// to the operator.
///
/// V0.2.0 Phase 6 narrowed the control-flow operand width from
/// `u32` to `u16`. Chunks are now capped at `CHUNK_SIZE_HARD_LIMIT`
/// ops; chunks crossing `CHUNK_SIZE_SOFT_WARN_THRESHOLD` produce
/// one warning each prompting decomposition into helper functions.
/// The hard limit is a [`CompileError`]; the soft threshold is a
/// `CompileWarning`.
#[derive(Debug, Clone)]
pub struct CompileWarning {
    /// Human-readable description of the warning.
    pub message: String,
    /// Name of the chunk that triggered the warning.
    pub chunk_name: String,
}

/// Maximum number of operations in a single chunk. V0.2.0 Phase 6
/// narrowed the control-flow operand width from `u32` to `u16`;
/// chunks cannot exceed `u16::MAX` ops because the jump targets
/// would not fit.
pub const CHUNK_SIZE_HARD_LIMIT: usize = u16::MAX as usize;

/// Soft warning threshold for a chunk's op count. Chunks crossing
/// this threshold are admissible but the compiler emits a
/// [`CompileWarning`] prompting decomposition. Set to 80% of
/// [`CHUNK_SIZE_HARD_LIMIT`].
pub const CHUNK_SIZE_SOFT_WARN_THRESHOLD: usize = (CHUNK_SIZE_HARD_LIMIT * 80) / 100;

/// Enforce the V0.2.0 Phase 6 chunk-size limits against the
/// supplied chunk. Returns a hard `CompileError` if the chunk's
/// op count exceeds [`CHUNK_SIZE_HARD_LIMIT`]; appends a
/// [`CompileWarning`] to `warnings` if it crosses
/// [`CHUNK_SIZE_SOFT_WARN_THRESHOLD`]; returns `Ok(())` if the
/// chunk fits.
///
/// Extracted from the inline check in [`compile_with_warnings`]
/// so the threshold logic is testable in isolation through a
/// hand-constructed `Chunk`. Live coverage of the soft-warning
/// path through the surface compile pipeline would require a
/// source program with more than 52,428 ops, which is
/// impractical at test time; the unit test in
/// `compiler::tests::soft_warning_fires_on_long_chunk` exercises
/// the helper against a synthetic chunk.
pub fn check_chunk_size_against_limits(
    chunk: &Chunk,
    span: crate::token::Span,
    warnings: &mut Vec<CompileWarning>,
) -> Result<(), CompileError> {
    let op_count = chunk.ops.len();
    if op_count > CHUNK_SIZE_HARD_LIMIT {
        return Err(CompileError {
            message: format!(
                "chunk `{}` emitted {} ops, exceeding the V0.2.0 limit of {} \
                 (u16 control-flow target width); decompose the function into helpers",
                chunk.name, op_count, CHUNK_SIZE_HARD_LIMIT,
            ),
            span,
        });
    }
    if op_count > CHUNK_SIZE_SOFT_WARN_THRESHOLD {
        warnings.push(CompileWarning {
            message: format!(
                "chunk `{}` has {} ops, crossing the 80% soft-warning threshold of \
                 {} against the {} cap; consider decomposing the function into helpers",
                chunk.name, op_count, CHUNK_SIZE_SOFT_WARN_THRESHOLD, CHUNK_SIZE_HARD_LIMIT,
            ),
            chunk_name: chunk.name.clone(),
        });
    }
    Ok(())
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
/// load.
///
/// Soft warnings produced during compilation are discarded by this
/// entry point; hosts that wish to surface them call
/// [`compile_with_warnings`] instead.
///
/// See `crate::target::Target` for available presets and the
/// portability story.
pub fn compile_with_target(
    program: &Program,
    target: &crate::target::Target,
) -> Result<Module, CompileError> {
    compile_with_warnings(program, target).map(|(module, _warnings)| module)
}

/// Compile a program with both the resulting module and any
/// soft warnings produced during emission.
///
/// Same admissibility checks and return shape as
/// [`compile_with_target`] for the module. Additionally returns a
/// vector of [`CompileWarning`]s, one per chunk that crossed
/// [`CHUNK_SIZE_SOFT_WARN_THRESHOLD`]. The vector is empty in the
/// typical case; hosts can route the entries to deployment-time
/// telemetry or surface them to the operator at the build step.
pub fn compile_with_warnings(
    program: &Program,
    target: &crate::target::Target,
) -> Result<(Module, Vec<CompileWarning>), CompileError> {
    let mut warnings: Vec<CompileWarning> = Vec::new();
    target.validate_against_runtime()?;
    crate::target::validate_program_for_target(program, target)?;
    let mut owned = program.clone();
    // Resolve the surface form `Fixed` (no explicit `<N>`) to the
    // target's Q-format default before the type checker observes
    // the program. The compiler downstream reads the resolved
    // immediate from the AST `PrimType::Fixed(Some(n))` to emit
    // `Op::WordToFixed(n)`, `Op::FixedMul(n)`, and friends; the
    // unresolved `Fixed(None)` form would silently fall back to
    // `DEFAULT_FIXED_FRAC_BITS` (Q31.32) regardless of target.
    normalize_fixed_defaults(&mut owned, target.fixed_default_frac_bits());
    crate::typecheck::check_with_target(&mut owned, *target).map_err(|e| CompileError {
        message: format!("type error: {}", e.message),
        span: e.span,
    })?;

    // Monomorphize generic functions before compilation. The pass
    // walks the call graph and generates a specialized FunctionDef
    // per `(function, type_args)` pair. Call sites are rewritten to
    // reference the specialized name. Generic functions for which
    // no concrete instantiation could be inferred remain unchanged
    // and rely on runtime polymorphism through the Value tag.
    let mut owned = crate::monomorphize::monomorphize(owned);
    // Re-typecheck the monomorphized program so specialized bodies
    // benefit from concrete-type method resolution.
    crate::typecheck::check_with_target(&mut owned, *target).map_err(|e| CompileError {
        message: format!("type error after monomorphization: {}", e.message),
        span: e.span,
    })?;
    // V0.2.0 Consolidation Phase 4 retired the closure-hoisting
    // pass and the four closure opcodes (`Op::PushFunc`,
    // `Op::MakeClosure`, `Op::MakeRecursiveClosure`, and
    // `Op::CallIndirect`). The type checker rejects
    // `Expr::Closure` before the program reaches this point, so
    // no closure-shaped expressions survive into compilation.
    let program = &owned;

    let mut native_names: Vec<String> = Vec::new();
    let mut native_map: BTreeMap<String, u16> = BTreeMap::new();

    // Collect native function names from use declarations.
    // Parallel `native_externals` map records each native's
    // verified-versus-external classification from its `use`
    // declaration. The compiler consults this map at call sites
    // to pick between `Op::CallVerifiedNative` and
    // `Op::CallExternalNative`.
    let mut native_externals: BTreeMap<String, bool> = BTreeMap::new();
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
                native_externals.insert(full.clone(), use_decl.is_external);
                native_names.push(full);
            }
            ImportItem::Wildcard => {
                // Wildcard imports cannot be resolved at compile time.
                // The VM must resolve them at runtime. For now, skip.
            }
        }
    }

    // R28: at most one data block per visibility class. The
    // original rule said "at most one data block per program";
    // with shared, private, and const visibility introduced in
    // V0.2.x a program may declare one of each. Two blocks of
    // the same visibility remain rejected.
    let mut seen_shared_span: Option<crate::token::Span> = None;
    let mut seen_private_span: Option<crate::token::Span> = None;
    let mut seen_const_span: Option<crate::token::Span> = None;
    for decl in &program.data_decls {
        let dup_slot = match decl.visibility {
            DataVisibility::Shared => &mut seen_shared_span,
            DataVisibility::Private => &mut seen_private_span,
            DataVisibility::Const => &mut seen_const_span,
        };
        if dup_slot.is_some() {
            return Err(CompileError {
                message: format!(
                    "at most one {} data block per program (R28); found duplicate `{}`",
                    match decl.visibility {
                        DataVisibility::Shared => "shared",
                        DataVisibility::Private => "private",
                        DataVisibility::Const => "const",
                    },
                    decl.name
                ),
                span: decl.span,
            });
        }
        *dup_slot = Some(decl.span);
    }

    // Const data validation. Every const data field must carry a
    // literal initializer; shared and private data fields must
    // not. The literal's type must match the declared field type.
    // Const data fields are baked into a nested lookup table:
    // data block name -> field name -> compile-time value. The
    // field-access codegen consults this table; field reads
    // compile to Op::Const loads from the per-chunk constant
    // pool. Writes are rejected at codegen time.
    let mut const_fields: BTreeMap<String, BTreeMap<String, crate::bytecode::ConstValue>> =
        BTreeMap::new();
    for decl in &program.data_decls {
        match decl.visibility {
            DataVisibility::Const => {
                let mut block: BTreeMap<String, crate::bytecode::ConstValue> = BTreeMap::new();
                for field in &decl.fields {
                    let lit = field.initializer.as_ref().ok_or_else(|| CompileError {
                        message: format!(
                            "const data field `{}.{}` is missing an initializer; const data fields require `= literal` initializers",
                            decl.name, field.name
                        ),
                        span: field.span,
                    })?;
                    let cv = const_value_from_literal_for_field(
                        lit,
                        &field.type_expr,
                        decl.name.as_str(),
                        field.name.as_str(),
                        field.span,
                    )?;
                    block.insert(field.name.clone(), cv);
                }
                const_fields.insert(decl.name.clone(), block);
            }
            DataVisibility::Shared | DataVisibility::Private => {
                for field in &decl.fields {
                    if field.initializer.is_some() {
                        return Err(CompileError {
                            message: format!(
                                "{} data field `{}.{}` has an initializer; only `const data` fields admit initializers",
                                match decl.visibility {
                                    DataVisibility::Shared => "shared",
                                    DataVisibility::Private => "private",
                                    DataVisibility::Const => unreachable!(),
                                },
                                decl.name,
                                field.name
                            ),
                            span: field.span,
                        });
                    }
                }
            }
        }
    }

    // Build data layout from data declarations. Validate that each field
    // type has a statically known fixed size before assigning a slot.
    //
    // Slot indices are partitioned: shared slots occupy the low
    // range [0, shared_count) and private slots occupy
    // [shared_count, shared_count + private_count). The runtime
    // uses this partition to enforce the host-API boundary on
    // `Vm::set_data`/`Vm::get_data`. The order within each
    // partition matches the source declaration order so error
    // messages and slot names remain predictable.
    let mut data_fields: BTreeMap<String, Vec<(String, u16)>> = BTreeMap::new();
    let mut shared_slots: Vec<DataSlot> = Vec::new();
    let mut private_slots: Vec<DataSlot> = Vec::new();
    let mut data_slot_idx: u16 = 0;
    // Two-pass loop. Pass 0 processes shared declarations; pass 1
    // processes private declarations. The slot index counter is
    // continuous across both passes.
    for pass_visibility in [DataVisibility::Shared, DataVisibility::Private] {
        for decl in &program.data_decls {
            if decl.visibility != pass_visibility {
                continue;
            }
            let mut fields = Vec::new();
            for field in &decl.fields {
                let mut visiting: BTreeSet<String> = BTreeSet::new();
                validate_data_field_type(&field.type_expr, &program.types, &mut visiting)?;
                fields.push((field.name.clone(), data_slot_idx));
                // Array-typed fields expand into consecutive slots, one
                // per scalar element. Scalar and other composite types
                // continue to occupy a single slot whose `Value`
                // representation carries the structure. Multi-
                // dimensional arrays (nested `Array` types) flatten into
                // a single contiguous slab; the runtime indexes through
                // the flat region after the compiler emits the per-level
                // stride arithmetic.
                let n_slots = slots_for_data_type(&field.type_expr);
                let visibility = match decl.visibility {
                    DataVisibility::Shared => crate::bytecode::SlotVisibility::Shared,
                    DataVisibility::Private => crate::bytecode::SlotVisibility::Private,
                    DataVisibility::Const => unreachable!(
                        "const data does not produce runtime slots; const fields handled separately"
                    ),
                };
                let target = match pass_visibility {
                    DataVisibility::Shared => &mut shared_slots,
                    DataVisibility::Private => &mut private_slots,
                    DataVisibility::Const => unreachable!(),
                };
                if n_slots == 1 {
                    target.push(DataSlot {
                        name: format!("{}.{}", decl.name, field.name),
                        visibility,
                    });
                } else {
                    for k in 0..n_slots {
                        target.push(DataSlot {
                            name: format!("{}.{}[{}]", decl.name, field.name, k),
                            visibility,
                        });
                    }
                }
                data_slot_idx = data_slot_idx
                    .checked_add(n_slots)
                    .ok_or_else(|| CompileError {
                        message: format!(
                            "data segment field `{}.{}` overflows the 16-bit slot index space",
                            decl.name, field.name
                        ),
                        span: field.span,
                    })?;
            }
            data_fields.insert(decl.name.clone(), fields);
        }
    }
    // Compose the final slot table: shared slots first, then
    // private slots. Slot indices in `data_fields` already
    // reference the unified space.
    let shared_count = shared_slots.len() as u32;
    let private_count = private_slots.len() as u32;
    let mut data_layout_slots: Vec<DataSlot> = shared_slots;
    data_layout_slots.append(&mut private_slots);
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
        let head = type_expr_head_name(&impl_block.for_type);
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
                let mut ordered: Vec<(String, i64)> = Vec::new();
                for v in &e.variants {
                    variants.insert(v.name.clone(), v.fields.clone());
                    ordered.push((v.name.clone(), v.discriminant_value));
                }
                type_info.enums.insert(e.name.clone(), variants);
                type_info.enum_variant_order.insert(e.name.clone(), ordered);
            }
            TypeDef::Newtype(n) => {
                type_info.newtype_names.insert(n.name.clone());
                if let Some(pred) = &n.refinement {
                    type_info
                        .newtype_refinements
                        .insert(n.name.clone(), pred.clone());
                }
            }
        }
    }
    for func in &program.functions {
        type_info
            .function_returns
            .insert(func.name.clone(), func.return_type.clone());
    }

    // Cache predicate bodies for refinement-elision lookup. A
    // candidate predicate has exactly one parameter bound by a
    // bare variable pattern (so we know the substitution name),
    // a single-tail-expression body (so the evaluator can walk
    // the expression without analysing statements), and no
    // function-call dependencies. The candidate filter is
    // intentionally conservative; the evaluator returns None for
    // anything outside its handled subset, so a missed candidate
    // simply falls through to the runtime check.
    for func in &program.functions {
        if !type_info
            .newtype_refinements
            .values()
            .any(|p| p == &func.name)
        {
            continue;
        }
        if func.params.len() != 1 {
            continue;
        }
        let param_name = match &func.params[0].pattern {
            crate::ast::Pattern::Variable(name, _) => name.clone(),
            _ => continue,
        };
        if !func.body.stmts.is_empty() {
            continue;
        }
        let Some(tail) = func.body.tail_expr.as_ref() else {
            continue;
        };
        type_info
            .refinement_bodies
            .insert(func.name.clone(), (param_name, (**tail).clone()));
    }
    for decl in &program.data_decls {
        let mut fields = BTreeMap::new();
        for f in &decl.fields {
            fields.insert(f.name.clone(), f.type_expr.clone());
        }
        type_info.data_field_types.insert(decl.name.clone(), fields);
    }

    // Compute per-function return-range summaries through a
    // fixed-point pass. Each iteration tries to add a summary
    // for any function whose body's range becomes inferable
    // under the current map of known summaries. We stop when
    // no new summaries are added in a full sweep. The fixed-
    // point converges in O(N^2) worst-case steps where N is the
    // function count; in practice nearly all summaries land in
    // one or two passes. Functions whose bodies remain
    // indeterminate after the loop are left absent, and call
    // sites referencing them fall through to the runtime path.
    let function_summaries = compute_function_return_ranges(program, &type_info);
    type_info.function_return_ranges = function_summaries;

    // Compile each function group. After emission, enforce the
    // V0.2.0 Phase 6 chunk-size limit: any chunk whose op count
    // exceeds `CHUNK_SIZE_HARD_LIMIT` is rejected as a
    // `CompileError` because the control-flow opcodes carry `u16`
    // jump targets that would not fit. Chunks that cross
    // `CHUNK_SIZE_SOFT_WARN_THRESHOLD` are admissible but produce
    // a `CompileWarning` prompting decomposition into helpers.
    let mut chunks: Vec<Chunk> = Vec::new();
    for (name, defs) in &groups {
        let chunk = compile_function_group(
            name,
            defs,
            &function_map,
            &native_map,
            &native_externals,
            &data_fields,
            &const_fields,
            &type_info,
        )?;
        let span = defs
            .first()
            .map(|d| d.span)
            .unwrap_or_else(crate::token::Span::default);
        check_chunk_size_against_limits(&chunk, span, &mut warnings)?;
        chunks.push(chunk);
    }

    let entry_point = function_map.get("main").map(|&idx| idx as usize);

    // `mut` is only consumed inside the `verify`-gated block that
    // populates the WCET and WCMU header fields. Suppress the
    // unused-mut warning when the verify feature is off so the
    // single declaration covers both feature combinations.
    // Reject private data blocks where no slot is ever written.
    // An unmutated private slot is wasted memory: the verifier's
    // ephemerality rule rules it out of contributing to the
    // module's behaviour, and the programmer almost certainly
    // meant `const data` instead. The diagnostic recommends the
    // rewrite. Const data blocks are exempt because their values
    // are compile-time and never written.
    if shared_count + private_count > 0 {
        let mut private_slot_indices: Vec<u16> = Vec::new();
        for decl in &program.data_decls {
            if decl.visibility != DataVisibility::Private {
                continue;
            }
            for field in &decl.fields {
                if let Some(field_list) = data_fields.get(&decl.name)
                    && let Some((_, slot_idx)) =
                        field_list.iter().find(|(name, _)| name == &field.name)
                {
                    let n = slots_for_data_type(&field.type_expr);
                    for k in 0..n {
                        private_slot_indices.push(slot_idx.saturating_add(k));
                    }
                }
            }
        }
        if !private_slot_indices.is_empty() {
            let mut written: alloc::collections::BTreeSet<u16> =
                alloc::collections::BTreeSet::new();
            for chunk in &chunks {
                for op in &chunk.ops {
                    match op {
                        crate::bytecode::Op::SetData(slot) => {
                            written.insert(*slot);
                        }
                        crate::bytecode::Op::SetDataIndexed(base, len) => {
                            for k in 0..*len {
                                written.insert(base.saturating_add(k));
                            }
                        }
                        _ => {}
                    }
                }
            }
            let all_unmutated = private_slot_indices
                .iter()
                .all(|slot| !written.contains(slot));
            if all_unmutated {
                let private_block_name = program
                    .data_decls
                    .iter()
                    .find(|d| d.visibility == DataVisibility::Private)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| String::from("<unknown>"));
                let private_span = program
                    .data_decls
                    .iter()
                    .find(|d| d.visibility == DataVisibility::Private)
                    .map(|d| d.span)
                    .unwrap_or_else(crate::token::Span::default);
                return Err(CompileError {
                    message: format!(
                        "private data block `{}` is never mutated; declare it as `const data` with literal initializers instead (private data carries runtime cost that immutable fields do not need)",
                        private_block_name
                    ),
                    span: private_span,
                });
            }
        }
    }

    #[cfg_attr(not(feature = "verify"), allow(unused_mut))]
    let mut module = Module {
        schema_hash: crate::bytecode::compute_schema_hash(data_layout.as_ref()),
        chunks,
        native_names,
        entry_point,
        data_layout,
        word_bits_log2: target.word_bits_log2,
        addr_bits_log2: target.addr_bits_log2,
        float_bits_log2: target.float_bits_log2,
        // Populated below after structural verification succeeds.
        wcet_cycles: 0,
        wcmu_bytes: 0,
        // Flags is populated by the verifier (under `verify`
        // feature) at end of compile_with_target. The shared
        // and private byte counts mirror the partition computed
        // above; their sum equals the total data segment size in
        // bytes (one Value-sized slot per slot).
        flags: 0,
        shared_data_bytes: shared_count.saturating_mul(crate::bytecode::VALUE_SLOT_SIZE_BYTES),
        private_data_bytes: private_count.saturating_mul(crate::bytecode::VALUE_SLOT_SIZE_BYTES),
    };

    // Compile-time defense-in-depth for the WCET and WCMU contract.
    // The conservative-verification stance requires that programs
    // whose bound cannot be proved are rejected at both compilation
    // and loading. The structural verifier and the unbounded-
    // construct scan run here so build pipelines that emit bytecode
    // without an immediate VM construction surface these rejections
    // at the build step rather than deferring them to load.
    //
    // Structural verification (block nesting, jump offsets,
    // block-type constraints, break containment, productivity
    // rule) runs on every chunk at compile time. V0.2.0 Phase 4
    // dropped the closure family, so the previous unbounded-
    // construct scan (`Op::CallIndirect` / `Op::MakeRecursiveClosure`)
    // is no longer needed; the type checker rejects closures
    // before any bytecode is generated.
    //
    // The full WCMU and WCET computation including loop iteration
    // bound extraction and arena-capacity check remain deferred to
    // `Vm::new` because they require the host's arena capacity and
    // because some Func chunks have parameter-dependent loops whose
    // bounds the present analysis cannot extract; those chunks are
    // legitimate when never reached from a Stream chunk's call graph.
    // See LANGUAGE_DESIGN.md Conservative Verification for the
    // design statement.
    // Build a name -> span lookup from the original function defs so
    // structural-verification and unbounded-construct rejections can
    // point at the offending declaration in source. Multi-headed
    // functions reuse the span of the first head, which matches the
    // single chunk produced for the group. Synthetic impl-method
    // chunks fall back to the impl block's span.
    // Compile-time structural verification and resource-bound
    // analysis. Gated behind the `verify` feature so that
    // `--no-default-features --features compile` produces a
    // compile pipeline that emits bytecode without the safety
    // checks built in. In that mode the bytecode header's WCET
    // and WCMU fields stay at 0 (auto); load-time verification,
    // if enabled in the consuming runtime, populates them.
    #[cfg(feature = "verify")]
    {
        let mut chunk_spans: BTreeMap<String, crate::token::Span> = BTreeMap::new();
        for func in &program.functions {
            chunk_spans.entry(func.name.clone()).or_insert(func.span);
        }
        for func in &synth_impl_methods {
            chunk_spans.entry(func.name.clone()).or_insert(func.span);
        }
        let span_for = |name: &str| -> crate::token::Span {
            chunk_spans
                .get(name)
                .copied()
                .unwrap_or_else(crate::token::Span::default)
        };

        crate::verify::verify(&module).map_err(|e| CompileError {
            message: format!("structural verification: {}: {}", e.chunk_name, e.message),
            span: span_for(&e.chunk_name),
        })?;

        // Populate the declared WCET and WCMU fields in the framing
        // header. The compile-time bounds use the bundled nominal cost
        // model and zero native attestations, since native attestations
        // are host-supplied at load time. The runtime re-runs the
        // analysis at `Vm::new` against the host's actual cost model and
        // attestations and may surface a tighter or looser bound.
        //
        // For atomic-total programs (no Stream chunks), the values stay
        // at 0 (auto). For Stream programs, the values are the maximum
        // across Stream chunks. Saturation to `u32::MAX` signals that
        // computation overflowed; safe `Vm::new` rejects on `u32::MAX`.
        let mut max_wcet: u32 = 0;
        let mut max_wcmu: u32 = 0;
        let mut wcet_overflow = false;
        let mut wcmu_overflow = false;
        for chunk in &module.chunks {
            if matches!(chunk.block_type, crate::bytecode::BlockType::Stream) {
                match crate::verify::wcet_stream_iteration(chunk) {
                    Ok(c) => {
                        max_wcet = max_wcet.max(c);
                    }
                    Err(_) => {
                        wcet_overflow = true;
                    }
                }
                match crate::verify::wcmu_stream_iteration(chunk) {
                    Ok((stack, heap)) => {
                        let total = stack.saturating_add(heap);
                        if total == u32::MAX {
                            wcmu_overflow = true;
                        } else {
                            max_wcmu = max_wcmu.max(total);
                        }
                    }
                    Err(_) => {
                        wcmu_overflow = true;
                    }
                }
            }
        }
        module.wcet_cycles = if wcet_overflow { u32::MAX } else { max_wcet };
        module.wcmu_bytes = if wcmu_overflow { u32::MAX } else { max_wcmu };

        // Ephemerality analysis. A module is ephemeral when it has no
        // private data and no value crossing the host-VM boundary at
        // runtime is arena-resident (i.e. text-typed under V0.2).
        //
        // The check combines two pieces of evidence:
        //
        // 1. **Parameter usage.** A `Text`-typed parameter that the
        //    function body never references cannot carry text across
        //    the host-VM boundary, so it does not disqualify
        //    ephemerality. An AST walk on the entry function's body
        //    identifies unused text parameters.
        //
        // 2. **Per-yield arena dataflow.** Even when the entry's
        //    declared return or yield type carries `Text`, every
        //    concrete `Op::Return` and `Op::Yield` in the compiled
        //    entry chunk may peek at a non-text value. The text-size
        //    abstract interpretation pass propagates per-callee
        //    text-ness through the call graph in topological order
        //    and reports whether any boundary-crossing op actually
        //    leaves a text-typed value on top of the abstract stack.
        //    A negative result is a sufficient proof that the entry's
        //    declared text return is never produced at runtime, so
        //    the module is admissible as ephemeral.
        //
        // The dataflow analysis falls back to the conservative
        // signature-only result if the call graph cannot be topologi-
        // cally ordered (e.g. cycle from a future op that the WCMU
        // pass would also reject). Recursion is already rejected
        // earlier in the pipeline, so the fallback path is unreach-
        // able for well-formed modules but is included for defence
        // in depth.
        let entry_name = "main";
        let entry_decl = program.functions.iter().find(|f| f.name == entry_name);
        // Compute the per-chunk text-flow analysis once. The entry
        // chunk's analysis tells us whether any concrete return/yield
        // path crosses the boundary carrying text.
        let entry_chunk_idx = module
            .chunks
            .iter()
            .position(|c| c.name == entry_name)
            .or(module.entry_point);
        let entry_boundary_carries_text = match (
            entry_chunk_idx,
            crate::verify::module_chunk_text_analyses(&module),
        ) {
            (Some(idx), Ok(analyses)) => analyses
                .get(idx)
                .map(|a| a.returns_text || a.yields_text)
                .unwrap_or(true),
            // Conservative fallback: assume the boundary carries text
            // if we cannot pinpoint the entry chunk or the topological
            // pass errored. The signature-level check below will still
            // gate the decision on the declared return type.
            _ => true,
        };
        let signature_uses_text = entry_decl
            .map(|f| {
                let unused_text_params: alloc::collections::BTreeSet<String> = f
                    .params
                    .iter()
                    .filter(|p| {
                        p.type_expr
                            .as_ref()
                            .map(type_expr_carries_text)
                            .unwrap_or(false)
                    })
                    .filter_map(|p| param_binding_name(&p.pattern))
                    .filter(|name| !param_name_is_used(&f.body, name))
                    .collect();
                let any_used_text_param = f.params.iter().any(|p| {
                    let text = p
                        .type_expr
                        .as_ref()
                        .map(type_expr_carries_text)
                        .unwrap_or(false);
                    if !text {
                        return false;
                    }
                    match param_binding_name(&p.pattern) {
                        Some(name) => !unused_text_params.contains(&name),
                        None => true,
                    }
                });
                // The declared return type only disqualifies
                // ephemerality when the entry chunk's compiled body
                // actually leaves a text value on the abstract stack
                // at a boundary-crossing op. The dataflow result is a
                // sound upper bound: a `false` result means no path
                // through the chunk carries text out, so the declared
                // `Text` return is unreachable in practice.
                let declared_return_carries_text =
                    type_expr_carries_text(&f.return_type) && entry_boundary_carries_text;
                any_used_text_param || declared_return_carries_text
            })
            .unwrap_or(false);
        let provably_ephemeral = module.private_data_bytes == 0 && !signature_uses_text;
        if provably_ephemeral {
            module.flags |= crate::bytecode::FLAG_EPHEMERAL;
        }
        // Enforce explicit `ephemeral` declarations.
        if let Some(decl) = entry_decl
            && decl.ephemeral
            && !provably_ephemeral
        {
            let mut reason = alloc::string::String::new();
            if module.private_data_bytes != 0 {
                reason.push_str("module declares `private data` which persists across resets");
            } else if signature_uses_text {
                if !reason.is_empty() {
                    reason.push_str(" and ");
                }
                reason.push_str(
                    "entry function signature carries `Text` which is arena-resident at runtime",
                );
            }
            return Err(CompileError {
                message: alloc::format!(
                    "`ephemeral` modifier on `{}` is not provable: {}",
                    decl.name,
                    reason
                ),
                span: decl.span,
            });
        }
    }

    // Surface-level `signed` modifier on the entry function sets
    // FLAG_REQUIRES_SIGNATURE on the module header. The flag is a
    // contract with the load-time runtime: the module must be
    // accompanied by a cryptographic signature that verifies
    // against the host's trust matrix. Signing itself is a
    // toolchain step independent of the compiler.
    //
    // The modifier is permitted only on the entry function. The
    // compiler rejects `signed` on any helper function with a
    // diagnostic that names the offending declaration.
    {
        let entry_name = "main";
        for func in &program.functions {
            if func.signed && func.name != entry_name {
                return Err(CompileError {
                    message: alloc::format!(
                        "`signed` modifier on `{}` is invalid: the modifier is admissible only on the module's entry function (`main`)",
                        func.name,
                    ),
                    span: func.span,
                });
            }
        }
        let entry_signed = program
            .functions
            .iter()
            .find(|f| f.name == entry_name)
            .map(|f| f.signed)
            .unwrap_or(false);
        if entry_signed {
            module.flags |= crate::wire_format::FLAG_REQUIRES_SIGNATURE;
        }
    }

    Ok((module, warnings))
}

/// True when the type expression contains `Text` anywhere within
/// its structure (including inside tuples and arrays). Used by
/// the ephemerality verifier pass to detect dialogue types that
/// would carry arena-resident values across the host-VM boundary.
#[cfg(feature = "verify")]
fn type_expr_carries_text(t: &TypeExpr) -> bool {
    match t {
        TypeExpr::Prim(PrimType::Text, _) => true,
        TypeExpr::Tuple(parts, _) => parts.iter().any(type_expr_carries_text),
        TypeExpr::Array(elem, _, _) => type_expr_carries_text(elem),
        TypeExpr::Option(inner, _) => type_expr_carries_text(inner),
        TypeExpr::Labelled(inner, _, _) => type_expr_carries_text(inner),
        TypeExpr::NegativeLabelled(inner, _, _) => type_expr_carries_text(inner),
        _ => false,
    }
}

/// Identity string for a type expression. Used at impl-block
/// dispatch and at sites that need a string-tagged head for
/// method resolution. Information-flow labels are not part of
/// the identity; the wrapper is unwrapped recursively.
fn type_expr_head_name(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Prim(p, _) => match p {
            PrimType::Byte => String::from("Byte"),
            PrimType::Word => String::from("Word"),
            PrimType::Fixed(_) => String::from("Fixed"),
            PrimType::Float => String::from("Float"),
            PrimType::Bool => String::from("bool"),
            PrimType::Text => String::from("Text"),
        },
        TypeExpr::Unit(_) => String::from("()"),
        TypeExpr::Named(name, _, _) => name.clone(),
        TypeExpr::Tuple(_, _) => String::from("tuple"),
        TypeExpr::Array(_, _, _) => String::from("array"),
        TypeExpr::Option(_, _) => String::from("Option"),
        TypeExpr::Labelled(inner, _, _) => type_expr_head_name(inner),
        TypeExpr::NegativeLabelled(inner, _, _) => type_expr_head_name(inner),
    }
}

/// Extract the binding name from a parameter pattern when it
/// is a simple identifier. Used by the ephemerality refinement
/// to look up the parameter in the body and decide whether it
/// is referenced. Patterns with destructuring return `None`
/// (the analysis falls back to "used" in that case for
/// soundness).
#[cfg(feature = "verify")]
fn param_binding_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Variable(name, _) => Some(name.clone()),
        _ => None,
    }
}

/// True when an identifier of the given name appears in the
/// expression tree, recursing through nested expressions,
/// blocks, statements, match arms, and closures. Used by the
/// ephemerality refinement to detect parameters that the body
/// never references.
#[cfg(feature = "verify")]
fn param_name_is_used(body: &Block, name: &str) -> bool {
    fn expr_uses(expr: &Expr, name: &str) -> bool {
        match expr {
            Expr::Ident { name: n, .. } => n == name,
            Expr::Literal { .. } | Expr::Placeholder { .. } => false,
            Expr::BinOp { left, right, .. } => expr_uses(left, name) || expr_uses(right, name),
            Expr::UnaryOp { operand, .. } => expr_uses(operand, name),
            Expr::Call { args, .. } => args.iter().any(|e| expr_uses(e, name)),
            Expr::Pipeline { left, args, .. } => {
                expr_uses(left, name) || args.iter().any(|e| expr_uses(e, name))
            }
            Expr::MethodCall { receiver, args, .. } => {
                expr_uses(receiver, name) || args.iter().any(|e| expr_uses(e, name))
            }
            Expr::FieldAccess { object, .. } => expr_uses(object, name),
            Expr::TupleIndex { object, .. } => expr_uses(object, name),
            Expr::ArrayIndex { object, index, .. } => {
                expr_uses(object, name) || expr_uses(index, name)
            }
            Expr::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                expr_uses(condition, name)
                    || block_uses(then_block, name)
                    || else_block
                        .as_ref()
                        .map(|b| block_uses(b, name))
                        .unwrap_or(false)
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                expr_uses(scrutinee, name)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .map(|g| expr_uses(g, name))
                            .unwrap_or(false)
                            || expr_uses(&arm.expr, name)
                    })
            }
            Expr::TupleLiteral { elements, .. } => elements.iter().any(|e| expr_uses(e, name)),
            Expr::ArrayLiteral { elements, .. } => elements.iter().any(|e| expr_uses(e, name)),
            Expr::StructInit { fields, .. } => fields.iter().any(|f| expr_uses(&f.value, name)),
            Expr::EnumVariant { args, .. } => args.iter().any(|e| expr_uses(e, name)),
            Expr::Yield { value, .. } => expr_uses(value, name),
            Expr::Cast { expr: inner, .. } => expr_uses(inner, name),
            Expr::Loop { body, .. } => block_uses(body, name),
            Expr::Closure { body, .. } => block_uses(body, name),
            Expr::ClosureRef { captures, .. } => captures.iter().any(|c| c == name),
            Expr::Checked { op_expr, arms, .. } => {
                expr_uses(op_expr, name) || arms.iter().any(|arm| expr_uses(&arm.body, name))
            }
            Expr::SaturateMax { .. } | Expr::SaturateMin { .. } => false,
            Expr::Classify { value, .. } | Expr::Declassify { value, .. } => expr_uses(value, name),
        }
    }
    fn block_uses(block: &Block, name: &str) -> bool {
        for stmt in &block.stmts {
            match stmt {
                Stmt::Let(l) => {
                    if expr_uses(&l.value, name) {
                        return true;
                    }
                }
                Stmt::For(f) => {
                    let iter_uses = match &f.iterable {
                        Iterable::Expr(e) => expr_uses(e, name),
                        Iterable::Range(lo, hi) => expr_uses(lo, name) || expr_uses(hi, name),
                    };
                    if iter_uses || block_uses(&f.body, name) {
                        return true;
                    }
                }
                Stmt::Break(_) => {}
                Stmt::DataFieldAssign { value, .. } => {
                    if expr_uses(value, name) {
                        return true;
                    }
                }
                Stmt::DataFieldIndexAssign { indices, value, .. } => {
                    if indices.iter().any(|e| expr_uses(e, name)) || expr_uses(value, name) {
                        return true;
                    }
                }
                Stmt::Expr(e) => {
                    if expr_uses(e, name) {
                        return true;
                    }
                }
            }
        }
        if let Some(tail) = &block.tail_expr {
            return expr_uses(tail, name);
        }
        false
    }
    block_uses(body, name)
}

/// Validate that a data segment field type has a statically known fixed size.
///
/// Admissible: i64, f64, bool, (), tuples, fixed-length arrays, Option of
/// admissible, named structs of admissible fields, named enums whose variants
/// all have admissible payloads. Rejected: String, opaque named types,
/// recursive types.
/// Total number of data-segment slots a field of the given type
/// occupies. Array fields flatten into the product of their lengths
/// multiplied by the underlying scalar slot count of the leaf type.
/// Every other field type (scalar, tuple, option, struct, enum)
/// uses a single slot whose `Value` representation carries the
/// internal structure.
/// Convert a const initializer into a `ConstValue` without
/// validating against a declared field type. Used recursively
/// when the outer context cannot supply a precise inner type
/// (struct fields and enum payloads). Scalar literals carry
/// enough information to choose the right `ConstValue` variant;
/// composite forms recurse.
fn const_value_any(init: &ConstInitializer) -> crate::bytecode::ConstValue {
    use crate::bytecode::ConstValue;
    match init {
        ConstInitializer::Scalar(Literal::Int(n)) => ConstValue::Int(*n),
        ConstInitializer::Scalar(Literal::Byte(b)) => ConstValue::Byte(*b),
        ConstInitializer::Scalar(Literal::Fixed { raw, .. }) => ConstValue::Fixed(*raw),
        #[cfg(feature = "floats")]
        ConstInitializer::Scalar(Literal::Float(f)) => ConstValue::Float(*f),
        #[cfg(not(feature = "floats"))]
        ConstInitializer::Scalar(Literal::Float(_)) => {
            unreachable!("float literals are rejected at lex time when the `floats` feature is off")
        }
        ConstInitializer::Scalar(Literal::Bool(b)) => ConstValue::Bool(*b),
        ConstInitializer::Scalar(Literal::String(s)) => ConstValue::StaticStr(s.clone()),
        ConstInitializer::Scalar(Literal::Unit) => ConstValue::Unit,
        ConstInitializer::Tuple(elements) => {
            let out: Vec<ConstValue> = elements.iter().map(const_value_any).collect();
            ConstValue::Tuple(out)
        }
        ConstInitializer::Array(elements) => {
            let out: Vec<ConstValue> = elements.iter().map(const_value_any).collect();
            ConstValue::Array(out)
        }
        ConstInitializer::Struct { name, fields } => {
            let out: Vec<(String, ConstValue)> = fields
                .iter()
                .map(|(fname, finit)| (fname.clone(), const_value_any(finit)))
                .collect();
            ConstValue::Struct {
                type_name: name.clone(),
                fields: out,
            }
        }
        ConstInitializer::Enum {
            enum_name,
            variant,
            args,
        } => {
            let out: Vec<ConstValue> = args.iter().map(const_value_any).collect();
            ConstValue::Enum {
                type_name: enum_name.clone(),
                variant: variant.clone(),
                fields: out,
            }
        }
    }
}

/// Convert a source-level const initializer to a `ConstValue`
/// for the constant pool. Validates the initializer's shape and
/// element types against the declared field type. Scalar
/// initializers match primitive types; tuple initializers match
/// tuple types element-wise; array initializers match array
/// types with a length check.
fn const_value_from_literal_for_field(
    init: &ConstInitializer,
    field_type: &TypeExpr,
    data_name: &str,
    field_name: &str,
    span: crate::token::Span,
) -> Result<crate::bytecode::ConstValue, CompileError> {
    use crate::bytecode::ConstValue;
    match (init, field_type) {
        (ConstInitializer::Scalar(lit), TypeExpr::Prim(p, _)) => match (lit, p) {
            (Literal::Int(n), PrimType::Word) => Ok(ConstValue::Int(*n)),
            (Literal::Int(n), PrimType::Byte) => {
                if !(0..=0xFF).contains(n) {
                    return Err(CompileError {
                        message: format!(
                            "const data field `{}.{}` initializer {} does not fit in `Byte` (range 0..=255)",
                            data_name, field_name, n
                        ),
                        span,
                    });
                }
                Ok(ConstValue::Byte(*n as u8))
            }
            #[cfg(feature = "floats")]
            (Literal::Float(f), PrimType::Float) => Ok(ConstValue::Float(*f)),
            (Literal::Bool(b), PrimType::Bool) => Ok(ConstValue::Bool(*b)),
            (Literal::String(s), PrimType::Text) => Ok(ConstValue::StaticStr(s.clone())),
            _ => Err(CompileError {
                message: format!(
                    "const data field `{}.{}` initializer is incompatible with the declared field type",
                    data_name, field_name
                ),
                span,
            }),
        },
        (ConstInitializer::Scalar(Literal::Unit), TypeExpr::Unit(_)) => Ok(ConstValue::Unit),
        (ConstInitializer::Tuple(elements), TypeExpr::Tuple(elem_types, _)) => {
            if elements.len() != elem_types.len() {
                return Err(CompileError {
                    message: format!(
                        "const data field `{}.{}` tuple initializer has {} element(s), expected {}",
                        data_name,
                        field_name,
                        elements.len(),
                        elem_types.len()
                    ),
                    span,
                });
            }
            let mut out: Vec<ConstValue> = Vec::with_capacity(elements.len());
            for (e, t) in elements.iter().zip(elem_types.iter()) {
                out.push(const_value_from_literal_for_field(
                    e, t, data_name, field_name, span,
                )?);
            }
            Ok(ConstValue::Tuple(out))
        }
        (ConstInitializer::Array(elements), TypeExpr::Array(elem_type, len, _)) => {
            if elements.len() != *len as usize {
                return Err(CompileError {
                    message: format!(
                        "const data field `{}.{}` array initializer has {} element(s), expected {}",
                        data_name,
                        field_name,
                        elements.len(),
                        len
                    ),
                    span,
                });
            }
            let mut out: Vec<ConstValue> = Vec::with_capacity(elements.len());
            for e in elements {
                out.push(const_value_from_literal_for_field(
                    e, elem_type, data_name, field_name, span,
                )?);
            }
            Ok(ConstValue::Array(out))
        }
        (ConstInitializer::Struct { name, fields }, TypeExpr::Named(decl_name, _, _)) => {
            if decl_name != name {
                return Err(CompileError {
                    message: format!(
                        "const data field `{}.{}` initializer is `{}` but declared type is `{}`",
                        data_name, field_name, name, decl_name
                    ),
                    span,
                });
            }
            let mut out: Vec<(String, ConstValue)> = Vec::with_capacity(fields.len());
            for (fname, finit) in fields {
                // Element-type lookup against the type registry
                // is delegated to the type checker. The compiler
                // accepts any scalar form here; mismatches will
                // surface at runtime when the constructed value
                // is used in a typed context. A tighter check
                // would consult `program.types` and validate per
                // field; deferred because struct const data is
                // primarily used for fixed lookup tables where
                // the type discipline is enforced by the field
                // names matching the struct declaration.
                let cv = const_value_from_literal_for_field(
                    finit,
                    &TypeExpr::Prim(PrimType::Word, span),
                    data_name,
                    field_name,
                    span,
                )
                .unwrap_or_else(|_| {
                    // Recurse with a permissive inner type by
                    // re-attempting against a synthetic "any"
                    // expectation. The struct case is one place
                    // where the precise inner type cannot be
                    // determined without a type-info lookup; we
                    // accept any well-formed const value here
                    // and rely on later runtime validation if
                    // the script reads the field through a
                    // type-narrowing operation.
                    const_value_any(finit)
                });
                out.push((fname.clone(), cv));
            }
            Ok(ConstValue::Struct {
                type_name: name.clone(),
                fields: out,
            })
        }
        (
            ConstInitializer::Enum {
                enum_name,
                variant,
                args,
            },
            TypeExpr::Named(decl_name, _, _),
        ) => {
            if decl_name != enum_name {
                return Err(CompileError {
                    message: format!(
                        "const data field `{}.{}` initializer is `{}::{}` but declared type is `{}`",
                        data_name, field_name, enum_name, variant, decl_name
                    ),
                    span,
                });
            }
            let out: Vec<ConstValue> = args.iter().map(const_value_any).collect();
            Ok(ConstValue::Enum {
                type_name: enum_name.clone(),
                variant: variant.clone(),
                fields: out,
            })
        }
        _ => Err(CompileError {
            message: format!(
                "const data field `{}.{}` initializer is incompatible with the declared field type",
                data_name, field_name
            ),
            span,
        }),
    }
}

fn slots_for_data_type(type_expr: &TypeExpr) -> u16 {
    match type_expr {
        TypeExpr::Array(elem, len, _) => {
            let elem_slots = slots_for_data_type(elem) as u32;
            let total = elem_slots.saturating_mul(*len as u32);
            total.min(u16::MAX as u32) as u16
        }
        _ => 1,
    }
}

/// A resolved indexed access into a data-segment array field.
/// `data_name` and `field` identify the slot region, `indices`
/// are the source-order indices (outermost-to-innermost), and
/// `field_type` is the field's declared type expression.
struct DataIndexedChain<'a> {
    data_name: &'a str,
    field: &'a str,
    indices: Vec<&'a Expr>,
}

/// If the given `(object, index)` expression pair forms a
/// well-shaped indexed access against a data-segment field,
/// return the chain. The function walks the `object` back
/// through any number of nested `Expr::ArrayIndex` layers until
/// it reaches a `FieldAccess` whose receiver is a bare data-
/// block name. The returned indices are in source order, that
/// is the outermost (leftmost) index first.
fn data_indexed_chain<'a>(object: &'a Expr, last_index: &'a Expr) -> Option<DataIndexedChain<'a>> {
    let mut indices: Vec<&'a Expr> = Vec::new();
    indices.push(last_index);
    let mut current = object;
    loop {
        match current {
            Expr::ArrayIndex { object, index, .. } => {
                indices.push(index);
                current = object.as_ref();
            }
            Expr::FieldAccess { object, field, .. } => {
                if let Expr::Ident { name, .. } = object.as_ref() {
                    indices.reverse();
                    return Some(DataIndexedChain {
                        data_name: name.as_str(),
                        field: field.as_str(),
                        indices,
                    });
                }
                return None;
            }
            _ => return None,
        }
    }
}

/// Emit per-level bounds checks and stride arithmetic for an
/// indexed data-segment access, leaving the flat offset on the
/// operand stack. Returns the total slot count of the field
/// and an error if the indexing depth does not match the
/// field's type structure or if the field is not an array.
fn emit_indexed_offset(
    fc: &mut FuncCompiler,
    field_type: &TypeExpr,
    indices: &[&Expr],
    span: Span,
) -> Result<u16, CompileError> {
    let single_level = indices.len() == 1;
    let mut current_type = field_type.clone();
    let mut emitted_first = false;
    for idx_expr in indices {
        let (elem_type, len) = match current_type {
            TypeExpr::Array(elem, len, _) => (*elem, len),
            _ => {
                return Err(CompileError {
                    message: String::from(
                        "indexed access on a non-array data field; only `[T; N]` data fields admit `field[i]`",
                    ),
                    span,
                });
            }
        };
        if len < 0 {
            return Err(CompileError {
                message: format!("data array length must be non-negative, got {}", len),
                span,
            });
        }
        if len > u16::MAX as i64 {
            return Err(CompileError {
                message: format!(
                    "data array length {} exceeds the 16-bit bound the bytecode supports",
                    len
                ),
                span,
            });
        }
        let len_u16 = len as u16;
        let stride = slots_for_data_type(&elem_type);

        compile_expr(fc, idx_expr)?;
        // Skip the explicit `BoundsCheck` when there is exactly
        // one index. The trailing `Op::GetDataIndexed` or
        // `Op::SetDataIndexed` will perform the same check
        // against the field's total length, which equals this
        // level's length for a single-level access.
        if !single_level {
            fc.emit(Op::BoundsCheck(len_u16));
        }
        if stride != 1 {
            let stride_const = fc.add_constant(Value::Int(stride as i64));
            fc.emit(Op::Const(stride_const));
            // Index-stride product is `Int * Int`. After
            // Consolidation B narrowed `Op::Mul` away from
            // `Int` operands, the compiler synthesizes the
            // wrapping product via `CheckedMul` followed by
            // `PopN(2)` so the overflow flag and high half are
            // discarded. `0` fraction bits selects integer multiply.
            fc.emit(Op::CheckedMul(0));
            fc.emit(Op::PopN(2));
        }
        if emitted_first {
            // Flat offset accumulation is `Int + Int`.
            // Consolidation B routes the wrapping sum through
            // `CheckedAdd; PopN(2)`.
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
        } else {
            emitted_first = true;
        }
        current_type = elem_type;
    }
    if matches!(current_type, TypeExpr::Array(_, _, _)) {
        return Err(CompileError {
            message: String::from(
                "indexed access does not descend to a scalar; provide one index per array level",
            ),
            span,
        });
    }
    Ok(slots_for_data_type(field_type))
}

/// Emit a `state.field[i][j]...` read by computing the flat
/// offset on the stack and issuing `Op::GetDataIndexed`.
fn emit_data_indexed_read(
    fc: &mut FuncCompiler,
    chain: DataIndexedChain<'_>,
    span: Span,
) -> Result<(), CompileError> {
    if !fc.is_data_block(chain.data_name) {
        return Err(CompileError {
            message: format!("unknown data block: {}", chain.data_name),
            span,
        });
    }
    // Indexed read on a const data field. The literal value
    // lives in the constant pool; emit a constant load followed
    // by per-level `Op::GetIndex` for each index expression. The
    // runtime's `Op::GetIndex` pops index and array, pushes the
    // element. For multi-dimensional access, the per-level reads
    // chain naturally.
    if fc.is_const_data_block(chain.data_name) {
        let cv = fc
            .const_data_field_value(chain.data_name, chain.field)
            .ok_or_else(|| CompileError {
                message: format!(
                    "unknown const data field: {}.{}",
                    chain.data_name, chain.field
                ),
                span,
            })?;
        let idx = fc.add_const_value(cv);
        fc.emit(Op::Const(idx));
        for index_expr in chain.indices {
            compile_expr(fc, index_expr)?;
            fc.emit(Op::GetIndex);
        }
        return Ok(());
    }
    let base = fc
        .resolve_data_field(chain.data_name, chain.field)
        .ok_or_else(|| CompileError {
            message: format!("unknown data field: {}.{}", chain.data_name, chain.field),
            span,
        })?;
    let field_type = fc
        .type_info
        .data_field_types
        .get(chain.data_name)
        .and_then(|fields| fields.get(chain.field))
        .cloned()
        .ok_or_else(|| CompileError {
            message: format!(
                "data field {}.{} has no recorded type",
                chain.data_name, chain.field
            ),
            span,
        })?;
    let total = emit_indexed_offset(fc, &field_type, &chain.indices, span)?;
    fc.emit(Op::GetDataIndexed(base, total));
    Ok(())
}

/// Emit a `state.field[i][j]... = value` write. The value is
/// already on the stack from the caller; the helper appends the
/// offset arithmetic and `Op::SetDataIndexed`.
fn emit_data_indexed_write(
    fc: &mut FuncCompiler,
    chain: DataIndexedChain<'_>,
    span: Span,
) -> Result<(), CompileError> {
    if !fc.is_data_block(chain.data_name) {
        return Err(CompileError {
            message: format!("unknown data block: {}", chain.data_name),
            span,
        });
    }
    if fc.is_const_data_block(chain.data_name) {
        return Err(CompileError {
            message: format!(
                "cannot assign to `{}.{}` because `{}` is declared `const data`; const data is immutable",
                chain.data_name, chain.field, chain.data_name
            ),
            span,
        });
    }
    let base = fc
        .resolve_data_field(chain.data_name, chain.field)
        .ok_or_else(|| CompileError {
            message: format!("unknown data field: {}.{}", chain.data_name, chain.field),
            span,
        })?;
    let field_type = fc
        .type_info
        .data_field_types
        .get(chain.data_name)
        .and_then(|fields| fields.get(chain.field))
        .cloned()
        .ok_or_else(|| CompileError {
            message: format!(
                "data field {}.{} has no recorded type",
                chain.data_name, chain.field
            ),
            span,
        })?;
    let total = emit_indexed_offset(fc, &field_type, &chain.indices, span)?;
    fc.emit(Op::SetDataIndexed(base, total));
    Ok(())
}

fn validate_data_field_type(
    type_expr: &TypeExpr,
    types: &[TypeDef],
    visiting: &mut BTreeSet<String>,
) -> Result<(), CompileError> {
    match type_expr {
        TypeExpr::Prim(prim, span) => match prim {
            PrimType::Byte
            | PrimType::Word
            | PrimType::Fixed(_)
            | PrimType::Float
            | PrimType::Bool => Ok(()),
            PrimType::Text => Err(CompileError {
                message: String::from(
                    "data field type Text is not admissible: variable-length \
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
        TypeExpr::Labelled(inner, _, _) => validate_data_field_type(inner, types, visiting),
        // Negative information-flow labels are admissible on data
        // field types. A `shared data` field is the host-script
        // boundary; a `private data` field is the yield-resume
        // boundary. Both are boundary positions in the same sense
        // as a function parameter or return position. The
        // boundary check fires at every script-side write through
        // the type checker's `check_negative_labels_against_data_write`.
        // The top-level-only rule is enforced by the type
        // checker's `validate_no_nested_negative_labels` invoked
        // on each field's type expression at the data-decl pass.
        TypeExpr::NegativeLabelled(inner, _, _) => validate_data_field_type(inner, types, visiting),
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
                TypeDef::Newtype(n) => &n.name == name,
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
                Some(TypeDef::Newtype(n)) => {
                    visiting.insert(name.clone());
                    validate_data_field_type(&n.underlying, types, visiting)?;
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

/// Returns `true` when two patterns dispatch on the same shape.
///
/// Two patterns share a shape when a value that matches one would
/// also match the other for dispatch purposes. `Variable` and
/// `Wildcard` both catch any value; two `Literal` patterns share
/// a shape only when their literals are equal. Composite patterns
/// (tuples, structs, enums) compare structurally on the same
/// rule. The function ignores `Span` fields so that two heads
/// declared at different source positions compare correctly.
fn pattern_shape_eq(a: &Pattern, b: &Pattern) -> bool {
    match (a, b) {
        (
            Pattern::Wildcard(_) | Pattern::Variable(_, _),
            Pattern::Wildcard(_) | Pattern::Variable(_, _),
        ) => true,
        (Pattern::Literal(la, _), Pattern::Literal(lb, _)) => la == lb,
        (Pattern::Tuple(pa, _), Pattern::Tuple(pb, _)) => {
            pa.len() == pb.len()
                && pa
                    .iter()
                    .zip(pb.iter())
                    .all(|(a, b)| pattern_shape_eq(a, b))
        }
        (Pattern::Enum(ea, va, sa, _), Pattern::Enum(eb, vb, sb, _)) => {
            ea == eb
                && va == vb
                && sa.len() == sb.len()
                && sa
                    .iter()
                    .zip(sb.iter())
                    .all(|(a, b)| pattern_shape_eq(a, b))
        }
        (Pattern::Struct(na, fa, _), Pattern::Struct(nb, fb, _)) => {
            na == nb
                && fa.len() == fb.len()
                && fa.iter().zip(fb.iter()).all(|(a, b)| {
                    a.name == b.name
                        && match (&a.pattern, &b.pattern) {
                            (Some(pa), Some(pb)) => pattern_shape_eq(pa, pb),
                            (None, None) => true,
                            _ => false,
                        }
                })
        }
        _ => false,
    }
}

/// Compile a group of function definitions with the same name into one chunk.
#[allow(clippy::too_many_arguments)]
fn compile_function_group(
    name: &str,
    defs: &[&FunctionDef],
    function_map: &BTreeMap<String, u16>,
    native_map: &BTreeMap<String, u16>,
    native_externals: &BTreeMap<String, bool>,
    data_fields: &BTreeMap<String, Vec<(String, u16)>>,
    const_fields: &BTreeMap<String, BTreeMap<String, crate::bytecode::ConstValue>>,
    type_info: &TypeInfo,
) -> Result<Chunk, CompileError> {
    // Within a group, every head after the first must dispatch on
    // a pattern shape distinct from every earlier head; otherwise
    // the later head is unreachable. Single-head groups skip this
    // loop. The check rejects both literal-pattern duplicates
    // (`fn classify(0)` declared twice) and signature duplicates
    // (`fn main()` declared twice).
    for (i, later) in defs.iter().enumerate().skip(1) {
        for earlier in &defs[..i] {
            if later.params.len() != earlier.params.len() {
                continue;
            }
            let shape_match = later
                .params
                .iter()
                .zip(earlier.params.iter())
                .all(|(l, e)| pattern_shape_eq(&l.pattern, &e.pattern));
            if shape_match && earlier.guard.is_none() && later.guard.is_none() {
                return Err(CompileError {
                    message: alloc::format!(
                        "function head `{}` is dead code: an earlier head with the same pattern shape and no guard already matches every value reaching it",
                        name
                    ),
                    span: later.span,
                });
            }
        }
    }

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
        native_externals.clone(),
        data_fields.clone(),
        const_fields.clone(),
        type_info.clone(),
    );
    fc.chunk.param_count = param_count;
    fc.chunk.param_types = first.params.iter().map(type_tag_for_param).collect();

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
            // Populate the parameter's range when the declared
            // type is a refined newtype with a predicate that
            // decomposes to a single interval. This is the
            // primary source of non-singleton ranges consumed by
            // the refinement-elision lattice pass.
            if let crate::ast::Pattern::Variable(_, _) = &param.pattern {
                if let Some(crate::ast::TypeExpr::Named(type_name, _, _)) = &param.type_expr
                    && let Some(pred_name) =
                        fc.type_info.newtype_refinements.get(type_name).cloned()
                    && let Some((pred_param, body)) =
                        fc.type_info.refinement_bodies.get(&pred_name).cloned()
                    && let Some(range) = predicate_true_set(&body, &pred_param)
                {
                    fc.local_ranges.insert(param_slots[i], range);
                } else if let Some(natural) = natural_range_of_type_expr(&param.type_expr) {
                    // Primitive parameters carry their type's
                    // natural range. The principal customer is
                    // Byte (always in `[0, 255]`); the cast `b as
                    // Word` carries this range through to a
                    // newtype constructor and admits elision when
                    // the newtype's predicate's true set covers
                    // it.
                    fc.local_ranges.insert(param_slots[i], natural);
                }
            }
        }

        if block_type == BlockType::Stream {
            // Stream function: wrap body in Stream...Reset.
            fc.emit(Op::Stream);
            compile_block(&mut fc, &first.body)?;
            fc.emit(Op::PopN(1)); // Discard body value before Reset.
            fc.emit(Op::Reset);
        } else {
            compile_block(&mut fc, &first.body)?;
            fc.emit(Op::Return);
        }
    } else {
        // Multiheaded or pattern-matched parameters: dispatch.
        //
        // For Func and Reentrant chunks, each head's body ends with
        // `Op::Return` and the dispatch falls through to a `Trap` on
        // no-match. For Stream chunks, the Stream...Reset envelope
        // is mandatory and exactly one of each must appear. The
        // dispatch is wrapped in `Op::Loop`/`Op::EndLoop` so each
        // matched head's body can `Op::Pop` its tail value and
        // `Op::Break` out of the loop, falling through to the
        // shared `Op::Reset` epilogue. The `Op::EndLoop` back-edge
        // is structurally required for verifier nesting balance but
        // is dead code because every reachable path either breaks
        // out (matched head) or traps (no head matched).
        let stream_dispatch = block_type == BlockType::Stream;
        let mut loop_marker: Option<usize> = None;
        let mut stream_break_addrs: Vec<usize> = Vec::new();
        if stream_dispatch {
            fc.emit(Op::Stream);
            loop_marker = Some(fc.emit_jump(Op::Loop(0)));
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
            if stream_dispatch {
                fc.emit(Op::PopN(1));
                let break_addr = fc.emit_jump(Op::Break(0));
                stream_break_addrs.push(break_addr);
            } else {
                fc.emit(Op::Return);
            }

            fc.end_scope();
        }

        // Close final arm's If blocks.
        for addr in fail_jumps.drain(..).rev() {
            fc.patch_jump(addr);
            fc.emit(Op::EndIf);
        }

        // No head matched: emit trap.
        fc.emit(Op::Trap(crate::bytecode::TrapKind::NoMatchingHead.code()));

        if stream_dispatch {
            // Emit EndLoop with its back-edge target = loop_ip + 1.
            // The Op::Loop target and each Op::Break(0) target are
            // patched to the position after EndLoop, which is where
            // the Op::Reset epilogue lives.
            let loop_ip = loop_marker.expect("Stream dispatch sets loop_marker");
            fc.emit(Op::EndLoop((loop_ip + 1) as u16));
            fc.patch_jump(loop_ip);
            for addr in &stream_break_addrs {
                fc.patch_jump(*addr);
            }
            fc.emit(Op::Reset);
        }
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
        fc.emit(Op::PushImmediate(0));
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
    if fc.is_const_data_block(data_name) {
        return Err(CompileError {
            message: format!(
                "cannot assign to `{}.{}` because `{}` is declared `const data`; const data is immutable",
                data_name, field, data_name
            ),
            span,
        });
    }
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
            // If the value expression constant-folds to an
            // integer and the binding is a bare variable, record
            // the (slot, value) mapping for the refinement-
            // elision identifier lookup. The fold reuses the
            // existing local-constant map so chains like `let x =
            // 2; let y = x * 21; Counter(y)` resolve through y
            // back to x's compile-time value. Failure to fold is
            // silently fine; the entry is simply not recorded.
            let folded = fold_to_int(&let_stmt.value, &|n| fc.local_const_lookup(n));
            compile_expr(fc, &let_stmt.value)?;
            compile_let_pattern_typed(fc, &let_stmt.pattern, ty)?;
            if let (Some(value), crate::ast::Pattern::Variable(name, _)) =
                (folded, &let_stmt.pattern)
                && let Some(slot) = fc.resolve_local(name.as_str())
            {
                fc.local_const_values.insert(slot, value);
            }
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
        Stmt::DataFieldIndexAssign {
            data_name,
            field,
            indices,
            value,
            span,
        } => {
            // Evaluate the value first so it lands beneath the
            // computed offset on the operand stack. The trailing
            // `Op::SetDataIndexed` then pops the offset and pops
            // the value.
            compile_expr(fc, value)?;
            let index_refs: Vec<&Expr> = indices.iter().collect();
            emit_data_indexed_write(
                fc,
                DataIndexedChain {
                    data_name: data_name.as_str(),
                    field: field.as_str(),
                    indices: index_refs,
                },
                *span,
            )?;
        }
        Stmt::Expr(expr) => {
            compile_expr(fc, expr)?;
            fc.emit(Op::PopN(1));
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
        Expr::EnumVariant {
            enum_name, span, ..
        } => Some(TypeExpr::Named(enum_name.clone(), Vec::new(), *span)),
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
        // Tuple literal inference: per-element inference; one
        // `None` element collapses the result because the let-
        // pattern decomposition requires every component.
        Expr::TupleLiteral { elements, span } => {
            let mut elem_tys: Vec<TypeExpr> = Vec::with_capacity(elements.len());
            for e in elements {
                elem_tys.push(infer_expr_type(fc, e)?);
            }
            Some(TypeExpr::Tuple(elem_tys, *span))
        }
        Expr::ArrayIndex { object, .. } => {
            let object_ty = infer_expr_type(fc, object)?;
            element_type_of(&object_ty)
        }
        // Tuple-index inference: walk the tuple type and project the
        // indexed component. Out-of-range indices are caught by the
        // type checker; here we conservatively report `None` so the
        // arithmetic dispatch falls through to the generic emission.
        Expr::TupleIndex { object, index, .. } => {
            let object_ty = infer_expr_type(fc, object)?;
            match object_ty {
                TypeExpr::Tuple(elems, _) => elems.get(*index as usize).cloned(),
                _ => None,
            }
        }
        Expr::Match { arms, .. } => {
            let first = arms.first()?;
            infer_expr_type(fc, &first.expr)
        }
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
        Expr::Cast { target, .. } => Some(target.clone()),
        Expr::BinOp {
            left,
            right,
            op,
            span,
            ..
        } => {
            // Arithmetic operators preserve the operand type. The
            // type checker rejects mixed-type operands so left's
            // type equals right's; comparisons produce `bool`.
            use crate::ast::BinOp;
            match op {
                BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                    infer_expr_type(fc, left).or_else(|| infer_expr_type(fc, right))
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
        Expr::UnaryOp { operand, .. } => infer_expr_type(fc, operand),
        // The discriminant-to-enum construct (B35 P6) yields the
        // target enum type, so a `let`-bound result carries the enum
        // type for subsequent type-directed operations (e.g. a later
        // `as Word` cast).
        Expr::Checked { op_expr, .. } => match op_expr.as_ref() {
            Expr::Cast {
                target: TypeExpr::Named(n, args, sp),
                ..
            } if fc.type_info.enum_variant_order.contains_key(n) => {
                Some(TypeExpr::Named(n.clone(), args.clone(), *sp))
            }
            _ => None,
        },
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
                PrimType::Byte => "Byte",
                PrimType::Word => "Word",
                PrimType::Fixed(_) => "Fixed",
                PrimType::Float => "Float",
                PrimType::Bool => "bool",
                PrimType::Text => "Text",
            }
            .to_string(),
        ),
        TypeExpr::Unit(_) => Some("()".to_string()),
        TypeExpr::Tuple(_, _) => Some("tuple".to_string()),
        TypeExpr::Array(_, _, _) => Some("array".to_string()),
        TypeExpr::Option(_, _) => Some("Option".to_string()),
        TypeExpr::Named(name, _, _) => Some(name.clone()),
        TypeExpr::Labelled(inner, _, _) => type_expr_head(inner),
        TypeExpr::NegativeLabelled(inner, _, _) => type_expr_head(inner),
    }
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
            fc.emit(Op::PopN(1));
        }
        Pattern::Tuple(pats, _) => {
            // Value is on stack. Store in temp, then extract fields.
            // Decompose the tuple type structurally so each inner
            // pattern's bind carries the element type. Without this,
            // patterns like `let (a, b) = (x, y)` bind `a` and `b`
            // without type information and downstream type-driven
            // dispatch (notably the V0.2.0 Consolidation B arithmetic
            // split) cannot route Int operands to the checked-form
            // emission.
            let elem_types: Vec<Option<TypeExpr>> = match &ty {
                Some(TypeExpr::Tuple(ts, _)) if ts.len() == pats.len() => {
                    ts.iter().cloned().map(Some).collect()
                }
                _ => pats.iter().map(|_| None).collect(),
            };
            let temp = fc.declare_local("__let_tmp");
            fc.emit(Op::SetLocal(temp));
            for (i, pat) in pats.iter().enumerate() {
                fc.emit(Op::GetLocal(temp));
                fc.emit(Op::GetTupleField(i as u8));
                let sub_ty = elem_types.get(i).cloned().unwrap_or(None);
                compile_let_pattern_typed(fc, pat, sub_ty)?;
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
/// Compile `for x in ctx.field { body }` against a data-segment
/// array field. The lowered loop iterates over the array's slot
/// region through `Op::GetDataIndexed`, avoiding the need to
/// materialise the field as a `Value::Array` on the operand stack.
fn compile_for_in_data_array(
    fc: &mut FuncCompiler,
    for_stmt: &ForStmt,
    data_name: &str,
    field: &str,
    elem_type: &TypeExpr,
    len: i64,
) -> Result<(), CompileError> {
    if matches!(elem_type, TypeExpr::Array(_, _, _)) {
        return Err(CompileError {
            message: format!(
                "for-in iteration over multi-dimensional data-segment array `{}.{}` is not supported; iterate the outer dimension by index and the inner explicitly",
                data_name, field
            ),
            span: for_stmt.span,
        });
    }
    if !(0..=u16::MAX as i64).contains(&len) {
        return Err(CompileError {
            message: format!(
                "data array length {} is outside the supported 16-bit bound",
                len
            ),
            span: for_stmt.span,
        });
    }
    let base = fc
        .resolve_data_field(data_name, field)
        .ok_or_else(|| CompileError {
            message: format!("unknown data field: {}.{}", data_name, field),
            span: for_stmt.span,
        })?;
    let total_slots = (len as u16).saturating_mul(slots_for_data_type(elem_type));

    // `_idx = 0`
    let zero_const = fc.add_constant(Value::Int(0));
    fc.emit(Op::Const(zero_const));
    let idx_slot = fc.declare_local("__for_data_idx");
    fc.emit(Op::SetLocal(idx_slot));

    // `_end = len`
    let end_const = fc.add_constant(Value::Int(len));
    fc.emit(Op::Const(end_const));
    let end_slot = fc.declare_local("__for_data_end");
    fc.emit(Op::SetLocal(end_slot));

    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();

    // Break-if when `idx >= end`.
    fc.emit(Op::GetLocal(idx_slot));
    fc.emit(Op::GetLocal(end_slot));
    fc.emit(Op::CmpGe);
    let break_addr = fc.emit(Op::BreakIf(0));

    // Bind the iteration variable to the indexed element.
    fc.emit(Op::GetLocal(idx_slot));
    fc.emit(Op::GetDataIndexed(base, total_slots));
    let var_slot = fc.declare_local_typed(&for_stmt.var, Some(elem_type.clone()));
    fc.emit(Op::SetLocal(var_slot));

    // Body.
    fc.begin_scope();
    compile_block(fc, &for_stmt.body)?;
    fc.emit(Op::PopN(1));
    fc.end_scope();

    // `idx = idx + 1`. Consolidation B routes the wrapping
    // sum through `CheckedAdd; PopN(2)` since the `Int` arm
    // of `Op::Add` was removed from VM dispatch.
    fc.emit(Op::GetLocal(idx_slot));
    let one_const = fc.add_constant(Value::Int(1));
    fc.emit(Op::Const(one_const));
    fc.emit(Op::CheckedAdd);
    fc.emit(Op::PopN(2));
    fc.emit(Op::SetLocal(idx_slot));

    let endloop_addr = fc.emit(Op::EndLoop(0));
    let after_endloop = fc.chunk.ops.len() as u16;
    if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
        *a = after_endloop;
    }
    if let Op::BreakIf(a) = &mut fc.chunk.ops[break_addr] {
        *a = after_endloop;
    }
    let after_loop = (loop_addr + 1) as u16;
    if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
        *a = after_loop;
    }
    fc.exit_loop();

    Ok(())
}

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
            fc.emit(Op::PopN(1)); // Discard block value.
            fc.end_scope();

            // Increment. Consolidation B routes the wrapping
            // sum through `CheckedAdd; PopN(2)`.
            fc.emit(Op::GetLocal(var_slot));
            let one_const = fc.add_constant(Value::Int(1));
            fc.emit(Op::Const(one_const));
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(var_slot));

            let endloop_addr = fc.emit(Op::EndLoop(0)); // Placeholder, patched to after Loop.

            // Patch Loop and BreakIf to point past EndLoop.
            let after_endloop = fc.chunk.ops.len() as u16;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            if let Op::BreakIf(a) = &mut fc.chunk.ops[break_addr] {
                *a = after_endloop;
            }
            // Patch EndLoop back-edge to instruction after Loop.
            let after_loop = (loop_addr + 1) as u16;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            fc.exit_loop(); // Patches Break addresses to after_endloop.
        }
        Iterable::Expr(expr) => {
            // Data-segment array fields are stored as multiple
            // consecutive slots rather than as a single
            // `Value::Array` slot. Naked field access against the
            // data segment is rejected for array fields, so the
            // for-in here must walk the slot region directly through
            // indexed reads. The lowered iteration is a numeric loop
            // from zero to the array length emitting
            // `Op::GetDataIndexed` per element.
            if let Expr::FieldAccess { object, field, .. } = expr
                && let Expr::Ident { name, .. } = object.as_ref()
                && fc.is_data_block(name)
            {
                let field_type = fc
                    .type_info
                    .data_field_types
                    .get(name)
                    .and_then(|f| f.get(field))
                    .cloned();
                if let Some(TypeExpr::Array(elem, len, _)) = field_type {
                    return compile_for_in_data_array(fc, for_stmt, name, field, &elem, len);
                }
            }
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
            fc.emit(Op::PopN(1));
            fc.end_scope();

            // Increment index. Consolidation B routes the
            // wrapping sum through `CheckedAdd; PopN(2)`.
            fc.emit(Op::GetLocal(idx_slot));
            let one_const = fc.add_constant(Value::Int(1));
            fc.emit(Op::Const(one_const));
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(idx_slot));

            let endloop_addr = fc.emit(Op::EndLoop(0));

            // Patch jumps.
            let after_endloop = fc.chunk.ops.len() as u16;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            if let Op::BreakIf(a) = &mut fc.chunk.ops[break_addr] {
                *a = after_endloop;
            }
            let after_loop = (loop_addr + 1) as u16;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            fc.exit_loop();
        }
    }
    Ok(())
}

/// Rewrite every `PrimType::Fixed(None)` in the program's
/// type annotations to `PrimType::Fixed(Some(frac_bits))`.
///
/// The surface form `Fixed` (no explicit `<N>` argument) is a
/// target-relative request for the target's default Q-format.
/// The compiler emits target-aware `Op::WordToFixed`,
/// `Op::FixedToWord`, `Op::FixedMul`, and `Op::FixedDiv`
/// immediates by reading the fraction-bit count from the AST
/// `PrimType::Fixed(Some(n))`; unresolved `Fixed(None)` would
/// silently fall back to the host's `DEFAULT_FIXED_FRAC_BITS`
/// (Q31.32) at the emission site, defeating cross-compilation
/// to a 32-bit or 16-bit target. This pass runs once at the
/// top of `compile_with_target` so every downstream consumer
/// sees the resolved form.
///
/// The walker descends through every place a `TypeExpr` can
/// appear: function parameter and return types, struct field
/// types, enum variant argument types, data field types, let
/// binding annotations, and cast targets. Composite type
/// expressions (`Tuple`, `Array`, `Option`, `Named<T, U, …>`)
/// recurse into their components.
fn normalize_fixed_defaults(program: &mut Program, frac_bits: u8) {
    use crate::ast::*;
    fn fix_type(t: &mut TypeExpr, frac_bits: u8) {
        match t {
            TypeExpr::Prim(PrimType::Fixed(slot), _) => {
                if slot.is_none() {
                    *slot = Some(frac_bits);
                }
            }
            TypeExpr::Prim(_, _) | TypeExpr::Unit(_) => {}
            TypeExpr::Tuple(parts, _) => {
                for p in parts.iter_mut() {
                    fix_type(p, frac_bits);
                }
            }
            TypeExpr::Array(elem, _, _) => fix_type(elem, frac_bits),
            TypeExpr::Option(inner, _) => fix_type(inner, frac_bits),
            TypeExpr::Named(_, args, _) => {
                for a in args.iter_mut() {
                    fix_type(a, frac_bits);
                }
            }
            TypeExpr::Labelled(inner, _, _) => fix_type(inner, frac_bits),
            TypeExpr::NegativeLabelled(inner, _, _) => fix_type(inner, frac_bits),
        }
    }
    fn fix_opt(t: &mut Option<TypeExpr>, frac_bits: u8) {
        if let Some(t) = t.as_mut() {
            fix_type(t, frac_bits);
        }
    }
    fn fix_block(block: &mut Block, frac_bits: u8) {
        for stmt in block.stmts.iter_mut() {
            fix_stmt(stmt, frac_bits);
        }
        if let Some(e) = block.tail_expr.as_mut() {
            fix_expr(e, frac_bits);
        }
    }
    fn fix_stmt(stmt: &mut Stmt, frac_bits: u8) {
        match stmt {
            Stmt::Let(l) => {
                fix_opt(&mut l.type_expr, frac_bits);
                fix_expr(&mut l.value, frac_bits);
            }
            Stmt::For(f) => {
                match &mut f.iterable {
                    Iterable::Range(s, e) => {
                        fix_expr(s, frac_bits);
                        fix_expr(e, frac_bits);
                    }
                    Iterable::Expr(e) => fix_expr(e, frac_bits),
                }
                fix_block(&mut f.body, frac_bits);
            }
            Stmt::Break(_) => {}
            Stmt::DataFieldAssign { value, .. } => fix_expr(value, frac_bits),
            Stmt::DataFieldIndexAssign { indices, value, .. } => {
                for idx in indices.iter_mut() {
                    fix_expr(idx, frac_bits);
                }
                fix_expr(value, frac_bits);
            }
            Stmt::Expr(e) => fix_expr(e, frac_bits),
        }
    }
    fn fix_expr(expr: &mut Expr, frac_bits: u8) {
        match expr {
            Expr::Literal { .. }
            | Expr::Ident { .. }
            | Expr::Placeholder { .. }
            | Expr::ClosureRef { .. } => {}
            Expr::BinOp { left, right, .. } => {
                fix_expr(left, frac_bits);
                fix_expr(right, frac_bits);
            }
            Expr::UnaryOp { operand, .. } => fix_expr(operand, frac_bits),
            Expr::Call { args, .. } => {
                for a in args.iter_mut() {
                    fix_expr(a, frac_bits);
                }
            }
            Expr::Pipeline { left, args, .. } => {
                fix_expr(left, frac_bits);
                for a in args.iter_mut() {
                    fix_expr(a, frac_bits);
                }
            }
            Expr::Yield { value, .. } => fix_expr(value, frac_bits),
            Expr::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                fix_expr(condition, frac_bits);
                fix_block(then_block, frac_bits);
                if let Some(eb) = else_block {
                    fix_block(eb, frac_bits);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                fix_expr(scrutinee, frac_bits);
                for arm in arms.iter_mut() {
                    if let Some(g) = arm.guard.as_mut() {
                        fix_expr(g, frac_bits);
                    }
                    fix_expr(&mut arm.expr, frac_bits);
                }
            }
            Expr::Loop { body, .. } => fix_block(body, frac_bits),
            Expr::Cast {
                expr: inner,
                target,
                ..
            } => {
                fix_expr(inner, frac_bits);
                fix_type(target, frac_bits);
            }
            Expr::TupleLiteral { elements, .. } => {
                for e in elements.iter_mut() {
                    fix_expr(e, frac_bits);
                }
            }
            Expr::ArrayLiteral { elements, .. } => {
                for e in elements.iter_mut() {
                    fix_expr(e, frac_bits);
                }
            }
            Expr::ArrayIndex { object, index, .. } => {
                fix_expr(object, frac_bits);
                fix_expr(index, frac_bits);
            }
            Expr::FieldAccess { object, .. } => fix_expr(object, frac_bits),
            Expr::TupleIndex { object, .. } => fix_expr(object, frac_bits),
            Expr::MethodCall { receiver, args, .. } => {
                fix_expr(receiver, frac_bits);
                for a in args.iter_mut() {
                    fix_expr(a, frac_bits);
                }
            }
            Expr::StructInit { fields, .. } => {
                for f in fields.iter_mut() {
                    fix_expr(&mut f.value, frac_bits);
                }
            }
            Expr::EnumVariant { args, .. } => {
                for a in args.iter_mut() {
                    fix_expr(a, frac_bits);
                }
            }
            Expr::Closure {
                params,
                return_type,
                body,
                ..
            } => {
                for p in params.iter_mut() {
                    fix_opt(&mut p.type_expr, frac_bits);
                }
                fix_opt(return_type, frac_bits);
                fix_block(body, frac_bits);
            }
            Expr::Checked { op_expr, arms, .. } => {
                fix_expr(op_expr, frac_bits);
                for arm in arms.iter_mut() {
                    fix_expr(&mut arm.body, frac_bits);
                }
            }
            Expr::SaturateMax { .. } | Expr::SaturateMin { .. } => {}
            Expr::Classify { value, .. } | Expr::Declassify { value, .. } => {
                fix_expr(value, frac_bits);
            }
        }
    }
    fn fix_function(func: &mut FunctionDef, frac_bits: u8) {
        for p in func.params.iter_mut() {
            fix_opt(&mut p.type_expr, frac_bits);
        }
        fix_type(&mut func.return_type, frac_bits);
        fix_block(&mut func.body, frac_bits);
    }
    for type_def in program.types.iter_mut() {
        match type_def {
            TypeDef::Struct(s) => {
                for f in s.fields.iter_mut() {
                    fix_type(&mut f.type_expr, frac_bits);
                }
            }
            TypeDef::Enum(e) => {
                for v in e.variants.iter_mut() {
                    for t in v.fields.iter_mut() {
                        fix_type(t, frac_bits);
                    }
                }
            }
            TypeDef::Newtype(n) => {
                fix_type(&mut n.underlying, frac_bits);
            }
        }
    }
    for data in program.data_decls.iter_mut() {
        for f in data.fields.iter_mut() {
            fix_type(&mut f.type_expr, frac_bits);
        }
    }
    for func in program.functions.iter_mut() {
        fix_function(func, frac_bits);
    }
    for impl_block in program.impls.iter_mut() {
        for method in impl_block.methods.iter_mut() {
            for p in method.params.iter_mut() {
                fix_opt(&mut p.type_expr, frac_bits);
            }
            fix_type(&mut method.return_type, frac_bits);
            fix_block(&mut method.body, frac_bits);
        }
    }
}

/// Compile an `enum_value as Word` cast into a chain of
/// variant tests. Each variant produces an `IsEnum` check
/// guarded by an `If`; on a match the corresponding
/// discriminant constant is pushed and the chain breaks out of
/// a wrapping virtual loop. The shape mirrors `compile_expr`'s
/// match-expression compilation so the structural verifier
/// accepts the resulting nesting unchanged.
fn compile_enum_to_word(
    fc: &mut FuncCompiler,
    inner: &Expr,
    enum_name: &str,
) -> Result<(), CompileError> {
    // Evaluate the enum value and stash it in a local so each
    // variant test can re-read it without re-evaluating the
    // expression (which may have side effects).
    compile_expr(fc, inner)?;
    let temp = fc.declare_local("__enum_cast");
    fc.emit(Op::SetLocal(temp));

    // Snapshot the variant table; the borrow against
    // `fc.type_info` is released before the loop body runs and
    // emits ops back into `fc`.
    let variants: Vec<(String, i64)> = fc
        .type_info
        .enum_variant_order
        .get(enum_name)
        .cloned()
        .unwrap_or_default();

    // Wrap the dispatch in a virtual loop so each successful
    // arm can break out with its discriminant on the stack.
    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();

    let e_const = fc.add_string_constant(enum_name);
    for (variant_name, discriminant) in &variants {
        fc.begin_scope();
        let v_const = fc.add_string_constant(variant_name);
        fc.emit(Op::GetLocal(temp));
        fc.emit(Op::IsEnum(e_const, v_const));
        let fail_addr = fc.emit_jump(Op::If(0));
        fc.emit(Op::PopN(1)); // Discard the peeked enum value.
        let disc_const = fc.add_constant(Value::Int(*discriminant));
        fc.emit(Op::Const(disc_const));
        let break_addr = fc.emit(Op::Break(0));
        if let Some(breaks) = fc.loop_breaks.last_mut() {
            breaks.push(break_addr);
        }
        fc.patch_jump(fail_addr);
        fc.emit(Op::EndIf);
        fc.end_scope();
    }

    // Defensive trap. The type checker has confirmed the source
    // is an enum of the named type, and the loop covers every
    // declared variant, so the fall-through is unreachable
    // unless host-constructed Value::Enum carries a variant
    // name outside the declaration.
    fc.emit(Op::Trap(
        crate::bytecode::TrapKind::EnumVariantUnmapped.code(),
    ));

    // Close the virtual loop. Pattern copied from
    // `compile_expr`'s `Match` arm.
    let endloop_addr = fc.emit(Op::EndLoop(0));
    let after_loop = (loop_addr + 1) as u16;
    if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
        *a = after_loop;
    }
    let after_endloop = fc.chunk.ops.len() as u16;
    if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
        *a = after_endloop;
    }
    fc.exit_loop();

    Ok(())
}

/// Result domain for the refinement-elision evaluator. The
/// evaluator works in three value kinds: the integer value
/// substituted for the parameter, intermediate integer values
/// produced by arithmetic, and boolean values produced by
/// comparison and logical operators.
#[derive(Debug, Clone, Copy)]
enum EvalValue {
    Int(i64),
    Bool(bool),
}

/// Statically evaluate a refinement predicate's body at the
/// supplied integer argument. Returns `Some(true)` or
/// `Some(false)` when the body is structurally evaluable and
/// produces a boolean; returns `None` otherwise.
fn eval_predicate_at_int(body: &Expr, param_name: &str, value: i64) -> Option<bool> {
    let lookup = |name: &str| -> Option<EvalValue> {
        if name == param_name {
            Some(EvalValue::Int(value))
        } else {
            None
        }
    };
    match eval_expr_with(body, &lookup)? {
        EvalValue::Bool(b) => Some(b),
        EvalValue::Int(_) => None,
    }
}

/// Fold an expression that contains only literals (and any
/// identifiers resolved by `lookup`) to an integer constant.
/// Returns `None` when the expression contains anything outside
/// the constant-folding subset.
fn fold_to_int(expr: &Expr, lookup: &dyn Fn(&str) -> Option<EvalValue>) -> Option<i64> {
    match eval_expr_with(expr, lookup)? {
        EvalValue::Int(n) => Some(n),
        EvalValue::Bool(_) => None,
    }
}

/// Statically evaluate an expression to either an integer or a
/// boolean value. `lookup` resolves bare identifiers; pass a
/// closure that returns `None` for everything to evaluate
/// literal-only expressions.
///
/// Handled forms: integer and boolean literals; identifier
/// references (via `lookup`); unary negation on integers; logical
/// negation on booleans; integer arithmetic (`+`, `-`, `*`, `/`,
/// `%`); comparison (`==`, `!=`, `<`, `<=`, `>`, `>=`); logical
/// operators (`and`, `or`). Anything else returns `None`.
fn eval_expr_with(expr: &Expr, lookup: &dyn Fn(&str) -> Option<EvalValue>) -> Option<EvalValue> {
    match expr {
        Expr::Literal { value: lit, .. } => match lit {
            crate::ast::Literal::Int(n) => Some(EvalValue::Int(*n)),
            crate::ast::Literal::Bool(b) => Some(EvalValue::Bool(*b)),
            _ => None,
        },
        Expr::Ident { name, .. } => lookup(name.as_str()),
        Expr::UnaryOp { op, operand, .. } => {
            let inner = eval_expr_with(operand, lookup)?;
            match (op, inner) {
                (crate::ast::UnaryOp::Neg, EvalValue::Int(n)) => {
                    Some(EvalValue::Int(n.wrapping_neg()))
                }
                (crate::ast::UnaryOp::Not, EvalValue::Bool(b)) => Some(EvalValue::Bool(!b)),
                _ => None,
            }
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            let l = eval_expr_with(left, lookup)?;
            let r = eval_expr_with(right, lookup)?;
            match (op, l, r) {
                (crate::ast::BinOp::Add, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Int(a.wrapping_add(b)))
                }
                (crate::ast::BinOp::Sub, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Int(a.wrapping_sub(b)))
                }
                (crate::ast::BinOp::Mul, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Int(a.wrapping_mul(b)))
                }
                (crate::ast::BinOp::Div, EvalValue::Int(a), EvalValue::Int(b)) if b != 0 => {
                    Some(EvalValue::Int(a.wrapping_div(b)))
                }
                (crate::ast::BinOp::Mod, EvalValue::Int(a), EvalValue::Int(b)) if b != 0 => {
                    Some(EvalValue::Int(a.wrapping_rem(b)))
                }
                (crate::ast::BinOp::Eq, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Bool(a == b))
                }
                (crate::ast::BinOp::Eq, EvalValue::Bool(a), EvalValue::Bool(b)) => {
                    Some(EvalValue::Bool(a == b))
                }
                (crate::ast::BinOp::NotEq, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Bool(a != b))
                }
                (crate::ast::BinOp::NotEq, EvalValue::Bool(a), EvalValue::Bool(b)) => {
                    Some(EvalValue::Bool(a != b))
                }
                (crate::ast::BinOp::Lt, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Bool(a < b))
                }
                (crate::ast::BinOp::LtEq, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Bool(a <= b))
                }
                (crate::ast::BinOp::Gt, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Bool(a > b))
                }
                (crate::ast::BinOp::GtEq, EvalValue::Int(a), EvalValue::Int(b)) => {
                    Some(EvalValue::Bool(a >= b))
                }
                (crate::ast::BinOp::And, EvalValue::Bool(a), EvalValue::Bool(b)) => {
                    Some(EvalValue::Bool(a && b))
                }
                (crate::ast::BinOp::Or, EvalValue::Bool(a), EvalValue::Bool(b)) => {
                    Some(EvalValue::Bool(a || b))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Decompose a refinement predicate body into the set of values
/// for which the predicate returns true. Returns `None` when the
/// body falls outside the decomposer's recognised subset; the
/// elision pass then falls back to the runtime check (sound by
/// construction).
///
/// Handled forms:
/// - `true` / `false` literals (full / empty sets).
/// - Comparison against the parameter on either side: `x op N`
///   or `N op x` with `op` in `<`, `<=`, `==`, `!=`, `>`, `>=`.
/// - `predicate_a and predicate_b` (set intersection).
/// - `predicate_a or predicate_b` (set union).
/// - `not predicate` (set complement) for any handled subexpression.
pub(crate) fn predicate_true_set(body: &Expr, param: &str) -> Option<crate::interval::IntervalSet> {
    use crate::interval::IntervalSet;
    match body {
        Expr::Literal {
            value: crate::ast::Literal::Bool(true),
            ..
        } => Some(IntervalSet::full()),
        Expr::Literal {
            value: crate::ast::Literal::Bool(false),
            ..
        } => Some(IntervalSet::empty()),
        Expr::BinOp {
            op, left, right, ..
        } => {
            if let Some((cmp, n)) = comparison_against_param(*op, left, right, param) {
                comparison_set(cmp, n)
            } else if matches!(op, crate::ast::BinOp::And) {
                let l = predicate_true_set(left, param)?;
                let r = predicate_true_set(right, param)?;
                Some(l.intersect(&r))
            } else if matches!(op, crate::ast::BinOp::Or) {
                let l = predicate_true_set(left, param)?;
                let r = predicate_true_set(right, param)?;
                Some(l.union(&r))
            } else {
                None
            }
        }
        Expr::UnaryOp {
            op: crate::ast::UnaryOp::Not,
            operand,
            ..
        } => {
            let inner = predicate_true_set(operand, param)?;
            Some(inner.complement())
        }
        _ => None,
    }
}

/// Match a comparison `lhs op rhs` against a comparison of the
/// parameter `param` to an integer constant. Returns `(op, n)`
/// in the normalized form `param op n` (with the operator
/// flipped when the parameter appears on the right).
fn comparison_against_param(
    op: crate::ast::BinOp,
    lhs: &Expr,
    rhs: &Expr,
    param: &str,
) -> Option<(crate::ast::BinOp, i64)> {
    if !matches!(
        op,
        crate::ast::BinOp::Lt
            | crate::ast::BinOp::LtEq
            | crate::ast::BinOp::Gt
            | crate::ast::BinOp::GtEq
            | crate::ast::BinOp::Eq
            | crate::ast::BinOp::NotEq
    ) {
        return None;
    }
    let param_on_left = matches!(lhs, Expr::Ident { name, .. } if name == param);
    let param_on_right = matches!(rhs, Expr::Ident { name, .. } if name == param);
    if param_on_left {
        let n = fold_to_int(rhs, &|_| None)?;
        Some((op, n))
    } else if param_on_right {
        let n = fold_to_int(lhs, &|_| None)?;
        Some((flip_cmp(op), n))
    } else {
        None
    }
}

/// Flip a comparison operator left-to-right: `a < b` is the same
/// as `b > a`. Used when the parameter appears on the right of
/// a comparison.
fn flip_cmp(op: crate::ast::BinOp) -> crate::ast::BinOp {
    match op {
        crate::ast::BinOp::Lt => crate::ast::BinOp::Gt,
        crate::ast::BinOp::LtEq => crate::ast::BinOp::GtEq,
        crate::ast::BinOp::Gt => crate::ast::BinOp::Lt,
        crate::ast::BinOp::GtEq => crate::ast::BinOp::LtEq,
        crate::ast::BinOp::Eq => crate::ast::BinOp::Eq,
        crate::ast::BinOp::NotEq => crate::ast::BinOp::NotEq,
        other => other,
    }
}

/// Compute the set of values satisfying `param op n`. Returns
/// the singleton or half-bounded interval for `op` in
/// `< <= == > >=`, and the two-piece complement for `!=`.
fn comparison_set(op: crate::ast::BinOp, n: i64) -> Option<crate::interval::IntervalSet> {
    use crate::interval::{Interval, IntervalSet};
    match op {
        crate::ast::BinOp::Lt => match n.checked_sub(1) {
            Some(m) => Some(IntervalSet::from_interval(Interval::at_most(m))),
            None => Some(IntervalSet::empty()),
        },
        crate::ast::BinOp::LtEq => Some(IntervalSet::from_interval(Interval::at_most(n))),
        crate::ast::BinOp::Gt => match n.checked_add(1) {
            Some(m) => Some(IntervalSet::from_interval(Interval::at_least(m))),
            None => Some(IntervalSet::empty()),
        },
        crate::ast::BinOp::GtEq => Some(IntervalSet::from_interval(Interval::at_least(n))),
        crate::ast::BinOp::Eq => Some(IntervalSet::singleton(n)),
        crate::ast::BinOp::NotEq => Some(IntervalSet::singleton(n).complement()),
        _ => None,
    }
}

/// Compute per-function return-range summaries through a fixed-
/// point loop with Cousot-Cousot widening for recursive
/// functions.
///
/// Each function starts with the empty `IntervalSet` (bottom).
/// Each iteration sweeps the function table and recomputes the
/// new candidate summary using the current map. When the new
/// candidate differs from the existing summary, the entry is
/// updated through a union for the first
/// `WIDEN_AFTER_ITERATIONS` rounds and then through widening,
/// forcing convergence on recursive functions whose body would
/// otherwise expand the range by a constant each iteration.
/// Iteration ends when a sweep produces no changes.
const WIDEN_AFTER_ITERATIONS: usize = 3;
const SUMMARY_PASS_LIMIT: usize = 16;

fn compute_function_return_ranges(
    program: &crate::ast::Program,
    type_info: &TypeInfo,
) -> BTreeMap<String, crate::interval::IntervalSet> {
    use crate::interval::IntervalSet;
    // Seed every function with the empty set so recursive calls
    // through `eval_expr_to_range::Call` find a lookup target.
    // Functions that never produce a non-empty candidate will be
    // dropped at the end so the elision pass sees only useful
    // summaries.
    let mut summaries: BTreeMap<String, IntervalSet> = BTreeMap::new();
    for func in &program.functions {
        summaries.insert(func.name.clone(), IntervalSet::empty());
    }
    for iteration in 0..SUMMARY_PASS_LIMIT {
        let mut changed = false;
        for func in &program.functions {
            let Some(new_range) = compute_function_return_range(func, type_info, &summaries) else {
                continue;
            };
            let old = summaries
                .get(&func.name)
                .cloned()
                .unwrap_or_else(IntervalSet::empty);
            if new_range == old {
                continue;
            }
            let updated = if iteration >= WIDEN_AFTER_ITERATIONS {
                old.widen(&new_range)
            } else {
                old.union(&new_range)
            };
            if updated != old {
                summaries.insert(func.name.clone(), updated);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Drop functions whose summary remained empty; the elision
    // pass treats absence as "no information" and falls through
    // to the runtime check, which is the same behaviour as an
    // empty range under the subset check (empty intersects with
    // anything to empty, so the pass would fail). Removing the
    // empty entries keeps the map size proportional to actual
    // information.
    summaries.retain(|_, range| !range.is_empty());
    summaries
}

/// Compute the return-range summary of a single function under
/// the current `known_summaries` map. Returns `None` when the
/// body is not a single tail expression or when the tail
/// expression cannot be reduced to an `IntervalSet` under the
/// parameter substitution.
fn compute_function_return_range(
    func: &crate::ast::FunctionDef,
    type_info: &TypeInfo,
    known_summaries: &BTreeMap<String, crate::interval::IntervalSet>,
) -> Option<crate::interval::IntervalSet> {
    if !func.body.stmts.is_empty() {
        return None;
    }
    let tail = func.body.tail_expr.as_ref()?;
    let mut params: BTreeMap<String, crate::interval::IntervalSet> = BTreeMap::new();
    for param in &func.params {
        let crate::ast::Pattern::Variable(name, _) = &param.pattern else {
            return None;
        };
        let range = param_range_from_type(&param.type_expr, type_info)?;
        params.insert(name.clone(), range);
    }
    eval_expr_to_range(tail, &params, type_info, known_summaries)
}

/// Resolve a function-parameter type expression to its compile-
/// time range. Refined newtypes carry their predicate's true set;
/// Byte parameters carry `[0, 255]`; Word parameters carry
/// `full()`. Anything else returns `None`.
fn param_range_from_type(
    t: &Option<crate::ast::TypeExpr>,
    type_info: &TypeInfo,
) -> Option<crate::interval::IntervalSet> {
    if let Some(crate::ast::TypeExpr::Named(type_name, _, _)) = t
        && let Some(pred_name) = type_info.newtype_refinements.get(type_name)
        && let Some((pred_param, body)) = type_info.refinement_bodies.get(pred_name)
        && let Some(range) = predicate_true_set(body, pred_param)
    {
        return Some(range);
    }
    natural_range_of_type_expr(t)
}

/// Evaluate an expression to an `IntervalSet` under a parameter
/// substitution map and a set of known function summaries. The
/// recognised shapes mirror [`infer_arg_range`] plus an extra
/// arm for `Expr::Call` that consults `known_summaries`.
fn eval_expr_to_range(
    expr: &Expr,
    params: &BTreeMap<String, crate::interval::IntervalSet>,
    _type_info: &TypeInfo,
    known_summaries: &BTreeMap<String, crate::interval::IntervalSet>,
) -> Option<crate::interval::IntervalSet> {
    use crate::interval::IntervalSet;
    match expr {
        Expr::Literal {
            value: crate::ast::Literal::Int(n),
            ..
        } => Some(IntervalSet::singleton(*n)),
        Expr::Ident { name, .. } => params.get(name).cloned(),
        Expr::UnaryOp {
            op: crate::ast::UnaryOp::Neg,
            operand,
            ..
        } => {
            let inner = eval_expr_to_range(operand, params, _type_info, known_summaries)?;
            Some(inner.neg())
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            let l = eval_expr_to_range(left, params, _type_info, known_summaries)?;
            let r = eval_expr_to_range(right, params, _type_info, known_summaries)?;
            match op {
                crate::ast::BinOp::Add => Some(l.add(&r)),
                crate::ast::BinOp::Sub => Some(l.sub(&r)),
                crate::ast::BinOp::Mul => Some(l.mul(&r)),
                crate::ast::BinOp::Div => Some(l.div(&r)),
                crate::ast::BinOp::Mod => Some(l.rem(&r)),
                _ => None,
            }
        }
        Expr::Cast { expr: inner, .. } => {
            eval_expr_to_range(inner, params, _type_info, known_summaries)
        }
        Expr::Call { name, .. } => known_summaries.get(name).cloned(),
        Expr::If {
            then_block,
            else_block,
            ..
        } => {
            // Both branches must reduce to a single tail
            // expression for the summary pass to apply; otherwise
            // we lose the result value. The union of the branch
            // ranges is a sound bound. A missing else (Unit-
            // returning if) returns None because the if's value
            // type is then Unit, outside the lattice's domain.
            let then_range = block_tail_range(then_block, params, _type_info, known_summaries)?;
            let else_range = match else_block {
                Some(b) => block_tail_range(b, params, _type_info, known_summaries)?,
                None => return None,
            };
            Some(then_range.union(&else_range))
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            // Narrowing match analysis (function-summary edition).
            // Mirrors the in-function narrowing in
            // `infer_arg_range_with`; differs in operand sources
            // (param map and summary map rather than function
            // compiler).
            use crate::interval::IntervalSet;
            let scrut_range = eval_expr_to_range(scrutinee, params, _type_info, known_summaries)?;
            let mut result = IntervalSet::empty();
            for arm in arms {
                let pattern_range = pattern_to_range(&arm.pattern)?;
                let arm_scrut = scrut_range.intersect(&pattern_range);
                if arm_scrut.is_empty() {
                    continue;
                }
                let mut arm_params = params.clone();
                if let Pattern::Variable(name, _) = &arm.pattern {
                    arm_params.insert(name.clone(), arm_scrut.clone());
                }
                let body_range =
                    eval_expr_to_range(&arm.expr, &arm_params, _type_info, known_summaries)?;
                result = result.union(&body_range);
            }
            Some(result)
        }
        _ => None,
    }
}

/// Helper for the summary pass's if/else handling: evaluate a
/// block's tail expression to an `IntervalSet`. Returns `None`
/// when the block has any statements or no tail (the summary
/// pass deals only with expression-bodied blocks).
fn block_tail_range(
    block: &crate::ast::Block,
    params: &BTreeMap<String, crate::interval::IntervalSet>,
    type_info: &TypeInfo,
    known_summaries: &BTreeMap<String, crate::interval::IntervalSet>,
) -> Option<crate::interval::IntervalSet> {
    if !block.stmts.is_empty() {
        return None;
    }
    let tail = block.tail_expr.as_ref()?;
    eval_expr_to_range(tail, params, type_info, known_summaries)
}

/// The natural range of a value of the given type expressed as
/// an [`IntervalSet`](crate::interval::IntervalSet) over the i64 representation that the
/// runtime carries on the operand stack. Returns:
///
/// - Byte: `[0, 255]` (the u8 domain lifted to i64).
/// - Word: `(-inf, +inf)` (no narrowing).
/// - Anything else: `None`; the caller should not record a
///   range. Fixed and other typed values share the i64 carrier
///   but their refinement-predicate surface is not yet wired,
///   so populating a natural range is premature.
fn natural_range_of_type_expr(
    t: &Option<crate::ast::TypeExpr>,
) -> Option<crate::interval::IntervalSet> {
    use crate::interval::{Interval, IntervalSet};
    let Some(crate::ast::TypeExpr::Prim(prim, _)) = t else {
        return None;
    };
    match prim {
        crate::ast::PrimType::Byte => Some(IntervalSet::from_interval(Interval::range(0, 255))),
        crate::ast::PrimType::Word => Some(IntervalSet::full()),
        _ => None,
    }
}

/// Infer the set of values a constructor's argument expression
/// might evaluate to at runtime. Returns `None` when the
/// expression falls outside the inference's recognised subset.
/// Sound: the returned set is a superset of the actual runtime
/// range, so a subset-of-true-set check is conservative.
fn infer_arg_range(expr: &Expr, fc: &FuncCompiler) -> Option<crate::interval::IntervalSet> {
    infer_arg_range_with(expr, fc, &BTreeMap::new())
}

/// Convert a match-arm pattern into the interval set of values
/// that match it. Returns `None` for patterns outside the
/// recognised subset (wildcard, variable, integer literal).
fn pattern_to_range(pattern: &Pattern) -> Option<crate::interval::IntervalSet> {
    use crate::interval::IntervalSet;
    match pattern {
        Pattern::Wildcard(_) | Pattern::Variable(_, _) => Some(IntervalSet::full()),
        Pattern::Literal(Literal::Int(n), _) => Some(IntervalSet::singleton(*n)),
        _ => None,
    }
}

/// Range inference with an identifier shadow map. Bare-variable
/// identifiers resolve through `shadow` first, then fall back to
/// the function compiler's local constant and range tables. The
/// shadow is used by the match-arm narrowing pathway to bind a
/// pattern's variable to the arm's intersected scrutinee range.
fn infer_arg_range_with(
    expr: &Expr,
    fc: &FuncCompiler,
    shadow: &BTreeMap<String, crate::interval::IntervalSet>,
) -> Option<crate::interval::IntervalSet> {
    use crate::interval::IntervalSet;
    match expr {
        Expr::Literal {
            value: crate::ast::Literal::Int(n),
            ..
        } => Some(IntervalSet::singleton(*n)),
        Expr::Ident { name, .. } => {
            if let Some(r) = shadow.get(name) {
                return Some(r.clone());
            }
            let slot = fc.resolve_local(name.as_str())?;
            if let Some(v) = fc.local_const_values.get(&slot) {
                Some(IntervalSet::singleton(*v))
            } else {
                fc.local_ranges.get(&slot).cloned()
            }
        }
        Expr::UnaryOp {
            op: crate::ast::UnaryOp::Neg,
            operand,
            ..
        } => {
            let inner = infer_arg_range_with(operand, fc, shadow)?;
            Some(inner.neg())
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            let l = infer_arg_range_with(left, fc, shadow)?;
            let r = infer_arg_range_with(right, fc, shadow)?;
            match op {
                crate::ast::BinOp::Add => Some(l.add(&r)),
                crate::ast::BinOp::Sub => Some(l.sub(&r)),
                crate::ast::BinOp::Mul => Some(l.mul(&r)),
                crate::ast::BinOp::Div => Some(l.div(&r)),
                crate::ast::BinOp::Mod => Some(l.rem(&r)),
                _ => None,
            }
        }
        Expr::Cast { expr: inner, .. } => infer_arg_range_with(inner, fc, shadow),
        Expr::Call { name, .. } => fc.type_info.function_return_ranges.get(name).cloned(),
        Expr::Match {
            scrutinee, arms, ..
        } => {
            // Narrowed match analysis. Compute the scrutinee's
            // range, then walk each arm: intersect with the
            // pattern's value set to get the arm's effective
            // scrutinee range, bind a variable pattern's name to
            // that range in the shadow, and recurse on the body.
            // Union the arm bodies' ranges. An arm whose pattern
            // is disjoint from the scrutinee range is unreachable
            // and contributes nothing. Guarded arms degrade to
            // the unguarded analysis (the guard might exclude
            // some values, but the analysis returns a sound
            // superset regardless).
            let scrut_range = infer_arg_range_with(scrutinee, fc, shadow)?;
            let mut result = IntervalSet::empty();
            for arm in arms {
                let pattern_range = pattern_to_range(&arm.pattern)?;
                let arm_scrut = scrut_range.intersect(&pattern_range);
                if arm_scrut.is_empty() {
                    continue;
                }
                let mut arm_shadow = shadow.clone();
                if let Pattern::Variable(name, _) = &arm.pattern {
                    arm_shadow.insert(name.clone(), arm_scrut.clone());
                }
                let body_range = infer_arg_range_with(&arm.expr, fc, &arm_shadow)?;
                result = result.union(&body_range);
            }
            Some(result)
        }
        _ => None,
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
            Literal::Byte(v) => {
                let idx = fc.add_constant(Value::Byte(*v));
                fc.emit(Op::Const(idx));
            }
            Literal::Fixed { raw, .. } => {
                let idx = fc.add_constant(Value::Fixed(*raw));
                fc.emit(Op::Const(idx));
            }
            #[cfg(feature = "floats")]
            Literal::Float(v) => {
                let idx = fc.add_constant(Value::Float(*v));
                fc.emit(Op::Const(idx));
            }
            #[cfg(not(feature = "floats"))]
            Literal::Float(_) => {
                unreachable!(
                    "float literals are rejected at lex time when the `floats` feature is off"
                );
            }
            Literal::String(s) => {
                let idx = fc.add_constant(Value::StaticStr(s.clone()));
                fc.emit(Op::Const(idx));
            }
            Literal::Bool(true) => {
                fc.emit(Op::PushImmediate(1));
            }
            Literal::Bool(false) => {
                fc.emit(Op::PushImmediate(2));
            }
            Literal::Unit => {
                fc.emit(Op::PushImmediate(0));
            }
        },

        Expr::Ident { name, span } => {
            if let Some(slot) = fc.resolve_local(name) {
                fc.emit(Op::GetLocal(slot));
            } else if fc.function_map.contains_key(name) {
                // V0.2.0 Phase 4 dropped the `Op::PushFunc` opcode
                // along with the rest of the closure family.
                // First-class function values are no longer
                // representable; the surface expression must be a
                // direct call site rather than a bare reference.
                return Err(CompileError {
                    message: alloc::format!(
                        "first-class function references are not supported in V0.2.0; \
                         rewrite `{}` as a direct call site or as a trait-bounded \
                         generic",
                        name
                    ),
                    span: *span,
                });
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
                    fc.emit(Op::PopN(1));
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
                    fc.emit(Op::PopN(1));
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
            // Infer the operand type before compiling so we can pick
            // the type-specialized arithmetic opcode. The dispatch is
            // a three-way split per V0.2.0 Consolidation B:
            //   * `Fixed` operands → `FixedMul(n)` / `FixedDiv(n)` for
            //     multiply and divide so the Q-format fraction-bit
            //     count is preserved; add and subtract on `Fixed` use
            //     the generic `Op::Add` / `Op::Sub` whose VM dispatch
            //     extends to `Value::Fixed`.
            //   * `Word` (Int) operands → checked-arithmetic
            //     synthesis. The compiler emits the `CheckedXxx`
            //     opcode followed by `PopN(2)` to discard the
            //     `(high, flag)` pair, leaving the wrapping result on
            //     the stack. This is semantically equivalent to the
            //     previous `Op::Add` Int branch but routes the cost
            //     through the checked-arithmetic family so the
            //     overflow-flag results are available to the language
            //     without a separate opcode for the unchecked form.
            //   * `Byte` / `Float` operands → generic `Op::Add` etc.
            //     whose VM dispatch retains the `Byte` and `Float`
            //     arms after Consolidation B narrowed away the `Int`
            //     arm.
            // The `Fixed` fraction-bit count comes from the operand's
            // `PrimType::Fixed(n)`; when `n` is `None` (the default-
            // form `Fixed` surface), it falls back to
            // `DEFAULT_FIXED_FRAC_BITS` which matches the host
            // runtime's Q31.32 format.
            let operand_ty = infer_expr_type(fc, left).or_else(|| infer_expr_type(fc, right));
            let left_fixed_n = match &operand_ty {
                Some(TypeExpr::Prim(PrimType::Fixed(n), _)) => {
                    Some(n.unwrap_or(crate::typecheck::DEFAULT_FIXED_FRAC_BITS))
                }
                _ => None,
            };
            // Treat operands as `Word` (Int) when the compiler can
            // confirm `Word` or when the type cannot be inferred.
            // The compiler's `infer_expr_type` is a partial re-
            // derivation that returns `None` for expressions whose
            // type the type checker accepts but the compiler cannot
            // resolve statically (notably host-native calls without
            // a declared signature, and chained data-segment
            // indexing). The default treats those operands as `Int`
            // because the script's default numeric type is `Word`;
            // `Float`, `Byte`, `Fixed`, and `Text` arithmetic on
            // inference-opaque expressions requires an explicit
            // annotation or a typed `use` declaration.
            let left_is_int = matches!(&operand_ty, None | Some(TypeExpr::Prim(PrimType::Word, _)));
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            match op {
                BinOp::Add => {
                    if left_is_int {
                        fc.emit(Op::CheckedAdd);
                        fc.emit(Op::PopN(2));
                    } else {
                        fc.emit(Op::Add);
                    }
                }
                BinOp::Sub => {
                    if left_is_int {
                        fc.emit(Op::CheckedSub);
                        fc.emit(Op::PopN(2));
                    } else {
                        fc.emit(Op::Sub);
                    }
                }
                BinOp::Mul => {
                    if let Some(n) = left_fixed_n {
                        fc.emit(Op::FixedMul(n));
                    } else if left_is_int {
                        fc.emit(Op::CheckedMul(0));
                        fc.emit(Op::PopN(2));
                    } else {
                        fc.emit(Op::Mul);
                    }
                }
                BinOp::Div => {
                    if let Some(n) = left_fixed_n {
                        fc.emit(Op::FixedDiv(n));
                    } else {
                        fc.emit(Op::Div);
                    }
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
            // Mirrors the binary-op type-specialization from
            // Consolidation B: operands inferred or defaulted to
            // `Int` route through `CheckedNeg` followed by
            // `PopN(2)` so the unchecked negate opcode does not
            // need an `Int` arm in the VM dispatch. Operands whose
            // type is explicitly `Byte`, `Fixed`, or `Float`
            // continue to use `Op::Neg` whose VM dispatch retains
            // those three arms. Unknown-type operands default to
            // `Int` for the same reason as the binary path.
            let operand_is_int = matches!(
                infer_expr_type(fc, operand),
                None | Some(TypeExpr::Prim(PrimType::Word, _))
            );
            compile_expr(fc, operand)?;
            match op {
                UnaryOp::Neg => {
                    if operand_is_int {
                        fc.emit(Op::CheckedNeg);
                        fc.emit(Op::PopN(2));
                    } else {
                        fc.emit(Op::Neg);
                    }
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
                let is_external = fc
                    .native_externals
                    .get(func.as_str())
                    .copied()
                    .unwrap_or(false);
                if is_external {
                    fc.emit(Op::CallExternalNative(idx, arg_count));
                } else {
                    fc.emit(Op::CallVerifiedNative(idx, arg_count));
                }
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
                fc.emit(Op::PushImmediate(0));
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

                let mut fail_addrs = compile_pattern_test(fc, &arm.pattern, temp)?;
                compile_pattern_bind(fc, &arm.pattern, temp)?;
                // Optional guard: evaluate in the scope of the
                // pattern's bindings; on false, fall through to the
                // next arm via the same If/EndIf machinery used by
                // pattern tests.
                if let Some(guard) = &arm.guard {
                    compile_expr(fc, guard)?;
                    let guard_fail = fc.emit_jump(Op::If(0));
                    fail_addrs.push(guard_fail);
                }
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
            fc.emit(Op::Trap(crate::bytecode::TrapKind::NoMatchingArm.code()));

            let endloop_addr = fc.emit(Op::EndLoop(0));

            // Patch EndLoop back-edge to instruction after Loop.
            let after_loop = (loop_addr + 1) as u16;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            // Patch Loop to past EndLoop, and patch all Break addresses.
            let after_endloop = fc.chunk.ops.len() as u16;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            fc.exit_loop();
        }

        Expr::Loop { body, .. } => {
            let loop_addr = fc.emit(Op::Loop(0));
            fc.enter_loop();

            compile_block(fc, body)?;
            fc.emit(Op::PopN(1)); // Discard block value.

            let endloop_addr = fc.emit(Op::EndLoop(0));

            // Patch EndLoop back-edge to instruction after Loop.
            let after_loop = (loop_addr + 1) as u16;
            if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
                *a = after_loop;
            }

            // Patch Loop to past EndLoop, and patch all Break addresses.
            let after_endloop = fc.chunk.ops.len() as u16;
            if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
                *a = after_endloop;
            }
            fc.exit_loop();

            // Loop expression evaluates to Unit after break.
            fc.emit(Op::PushImmediate(0));
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
                // Const data fields resolve to a compile-time
                // literal and emit a constant load. The runtime
                // never allocates a data-segment slot for them.
                if fc.is_const_data_block(name) {
                    let cv =
                        fc.const_data_field_value(name, field)
                            .ok_or_else(|| CompileError {
                                message: format!("unknown const data field: {}.{}", name, field),
                                span: *span,
                            })?;
                    let idx = fc.add_const_value(cv);
                    fc.emit(Op::Const(idx));
                    return Ok(());
                }
                let slot = fc
                    .resolve_data_field(name, field)
                    .ok_or_else(|| CompileError {
                        message: format!("unknown data field: {}.{}", name, field),
                        span: *span,
                    })?;
                let field_type = fc
                    .type_info
                    .data_field_types
                    .get(name)
                    .and_then(|fields| fields.get(field));
                if let Some(t) = field_type
                    && matches!(t, TypeExpr::Array(_, _, _))
                {
                    return Err(CompileError {
                        message: format!(
                            "data field `{}.{}` is an array; index it through `{}.{}[i]`",
                            name, field, name, field
                        ),
                        span: *span,
                    });
                }
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

        Expr::ArrayIndex {
            object,
            index,
            span,
        } => {
            // Detect indexed access against a data-segment field and
            // emit `Op::GetDataIndexed` plus the per-level
            // `Op::BoundsCheck` / stride arithmetic. Stack-resident
            // arrays continue to use `Op::GetIndex`.
            if let Some(chain) = data_indexed_chain(object, index) {
                emit_data_indexed_read(fc, chain, *span)?;
                return Ok(());
            }
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
            // Choose the cast opcode based on the source type
            // (inferred from the inner expression) and the target.
            // The type checker has already validated the cast pair
            // (Word↔Float, Word↔Byte, Word↔Fixed, Enum↔Word, or
            // identity); this dispatch picks the matching opcode.
            //
            // The Fixed fraction-bit count is hard-coded to 32
            // The `Fixed<N>` parameterised form pins the count
            // explicitly; the default `Fixed` form resolves to
            // `crate::typecheck::DEFAULT_FIXED_FRAC_BITS` (32 on
            // the host runtime). Target-scaled defaults for
            // sub-64-bit targets are deferred to a follow-up that
            // threads the target descriptor into the function
            // compiler.
            let source = infer_expr_type(fc, inner);
            // Enum-to-Word special case. The source must be an
            // enum type the compiler knows about; the cast emits
            // a chain of `IsEnum` tests, one per variant, that
            // each push the variant's discriminant on a match.
            if matches!(target, TypeExpr::Prim(PrimType::Word, _))
                && let Some(TypeExpr::Named(enum_name, _, _)) = source.as_ref()
                && fc.type_info.enum_variant_order.contains_key(enum_name)
            {
                let enum_name = enum_name.clone();
                compile_enum_to_word(fc, inner, &enum_name)?;
                return Ok(());
            }
            compile_expr(fc, inner)?;
            // Newtype <-> underlying casts are identity at the
            // bytecode level because newtypes are transparent.
            // Detect both directions and emit nothing.
            let source_is_newtype = match source.as_ref() {
                Some(TypeExpr::Named(name, _, _)) => fc.type_info.newtype_names.contains(name),
                _ => false,
            };
            let target_is_newtype = match target {
                TypeExpr::Named(name, _, _) => fc.type_info.newtype_names.contains(name),
                _ => false,
            };
            if source_is_newtype || target_is_newtype {
                // No opcode emitted; the value flows through
                // unchanged.
            } else {
                match (source.as_ref(), target) {
                    (_, TypeExpr::Prim(PrimType::Float, _)) => {
                        fc.emit(Op::IntToFloat);
                    }
                    (
                        Some(TypeExpr::Prim(PrimType::Byte, _)),
                        TypeExpr::Prim(PrimType::Word, _),
                    ) => {
                        fc.emit(Op::ByteToWord);
                    }
                    (
                        Some(TypeExpr::Prim(PrimType::Fixed(n), _)),
                        TypeExpr::Prim(PrimType::Word, _),
                    ) => {
                        fc.emit(Op::FixedToWord(
                            n.unwrap_or(crate::typecheck::DEFAULT_FIXED_FRAC_BITS),
                        ));
                    }
                    (_, TypeExpr::Prim(PrimType::Word, _)) => {
                        // Default for `as Word`: source is `Float`.
                        // Byte and Fixed sources are caught by the more
                        // specific arms above.
                        fc.emit(Op::FloatToInt);
                    }
                    (_, TypeExpr::Prim(PrimType::Byte, _)) => {
                        fc.emit(Op::WordToByte);
                    }
                    (_, TypeExpr::Prim(PrimType::Fixed(n), _)) => {
                        fc.emit(Op::WordToFixed(
                            n.unwrap_or(crate::typecheck::DEFAULT_FIXED_FRAC_BITS),
                        ));
                    }
                    _ => {
                        // Other casts are identity at runtime.
                    }
                }
            }
        }

        Expr::Placeholder { span } => {
            return Err(CompileError {
                message: String::from("placeholder _ outside of pipeline"),
                span: *span,
            });
        }
        Expr::Closure { span, .. } | Expr::ClosureRef { span, .. } => {
            // V0.2.0 Phase 4 retired the closure family. The type
            // checker rejects `Expr::Closure` before compilation
            // and `Expr::ClosureRef` is no longer synthesized
            // (the closure-hoisting pass was removed). Reaching
            // this arm indicates a compiler-internal bug; report a
            // clear diagnostic rather than panicking so the
            // upstream pass can be located.
            return Err(CompileError {
                message: String::from(
                    "internal: closure expression reached the compiler after V0.2.0 \
                     Phase 4 retired the closure family. This is a compiler bug.",
                ),
                span: *span,
            });
        }
        Expr::Checked {
            op_expr,
            arms,
            span,
        } => {
            compile_checked(fc, op_expr, arms, span)?;
        }
        Expr::SaturateMax { .. } => {
            // Word::MAX. Future iterations refine the value based
            // on the construct's expected type (Byte::MAX,
            // Fixed-specific bound, or a refined-type contract's
            // saturate_max declaration). V0.2 supports only Word.
            let idx = fc.add_constant(Value::Int(i64::MAX));
            fc.emit(Op::Const(idx));
        }
        Expr::SaturateMin { .. } => {
            // Word::MIN.
            let idx = fc.add_constant(Value::Int(i64::MIN));
            fc.emit(Op::Const(idx));
        }
        Expr::Classify { value, .. } | Expr::Declassify { value, .. } => {
            // classify / declassify are compile-time-only
            // information-flow operations. The bytecode emitted is
            // the inner expression's bytecode unchanged. Label
            // tracking and declassification audit happen entirely
            // at the type-checker layer.
            compile_expr(fc, value)?;
        }
    }
    Ok(())
}

/// Compile an overflow-checked construct.
///
/// Surface form: `op_expr { ok(p) => body, overflow(ph, pl) =>
/// body, underflow(ph, pl) => body }` with optional `when guard`
/// clauses. Patterns may be `_`, a bare identifier (binds a
/// `Word`), or an integer literal (matches by equality).
///
/// Lowering shape:
/// 1. Emit the operands and the checked opcode (`CheckedAdd`,
///    `CheckedSub`, `CheckedMul`, `CheckedNeg`, or `Div`/`Mod`
///    with stamped overflow contract). The stack carries
///    `[high, low, flag]` after the opcode.
/// 2. Stash all three slots into temporary locals.
/// 3. Wrap the arms in a virtual `Loop` so the first matching
///    arm can `Break` out with its body's result on the stack.
/// 4. For each arm in declaration order, emit a class-flag check
///    (`flag == 0` for `ok`, `1` for `overflow`, `2` for
///    `underflow`), then literal-pattern equality checks against
///    `high_slot` / `low_slot`, then optional guard evaluation,
///    then bind variable patterns and compile the body.
/// 5. After the last arm, emit a `Trap` for the unreachable case
///    where no catch-all matched (defense-in-depth; the type
///    checker rejects non-exhaustive constructs).
fn compile_checked(
    fc: &mut FuncCompiler,
    op_expr: &Expr,
    arms: &[crate::ast::CheckedArm],
    span: &Span,
) -> Result<(), CompileError> {
    use crate::ast::{CheckedArmKind, Pattern};

    // The indexing construct (B35 P4) shares this node but lowers
    // differently: a bounds check synthesized from existing opcodes,
    // not a checked-arithmetic opcode.
    if let Expr::ArrayIndex { object, index, .. } = op_expr {
        return compile_checked_index(fc, op_expr, object, index, arms, span);
    }
    // The newtype-construction construct (B35 P5): reify the
    // refinement-predicate check into an outcome flag rather than
    // trapping on failure.
    if let Expr::Call { name, args, .. } = op_expr
        && fc.type_info.newtype_names.contains(name)
        && args.len() == 1
    {
        return compile_checked_newtype(fc, op_expr, name, &args[0], arms, span);
    }
    // The discriminant-to-enum construct (B35 P6): a `Word as Enum`
    // cast with outcome arms lowers to a per-variant discriminant
    // dispatch.
    if let Expr::Cast {
        expr: inner,
        target,
        ..
    } = op_expr
        && let TypeExpr::Named(enum_name, _, _) = target
        && fc.type_info.enum_variant_order.contains_key(enum_name)
    {
        let enum_name = enum_name.clone();
        return compile_checked_discriminant(fc, inner, &enum_name, arms, span);
    }

    // Determine the operand expression and, when it is `Fixed`, its
    // fraction-bit count (B35 P3d-iii). The multiply and divide paths
    // pass the count to the unified `Op::CheckedMul` / `Op::CheckedDiv`
    // opcodes, where `0` selects integer arithmetic and a positive
    // count selects the `Q`-format shift. The arm bindings carry the
    // `Fixed` type so the arm-body arithmetic dispatch routes through
    // the `Fixed` opcodes.
    // `+`, `-`, `%`, and unary `-` on `Fixed` need no fraction-bit
    // count and reuse the generic checked opcodes, whose VM dispatch
    // now carries `Fixed` arms. When `n` is `None` (the default-form
    // `Fixed` surface), it falls back to `DEFAULT_FIXED_FRAC_BITS`,
    // matching the plain-arithmetic path; the AST is normalized to a
    // concrete count before compilation in practice.
    let operand = match op_expr {
        Expr::BinOp { left, .. } => Some(left.as_ref()),
        Expr::UnaryOp { operand, .. } => Some(operand.as_ref()),
        _ => None,
    };
    let operand_fixed_n: Option<u8> =
        operand
            .and_then(|e| infer_expr_type(fc, e))
            .and_then(|t| match t {
                TypeExpr::Prim(PrimType::Fixed(n), _) => {
                    Some(n.unwrap_or(crate::typecheck::DEFAULT_FIXED_FRAC_BITS))
                }
                _ => None,
            });

    // Emit the checked operation. Each path leaves [high, low,
    // flag] on the stack.
    match op_expr {
        Expr::BinOp {
            op: BinOp::Add,
            left,
            right,
            ..
        } => {
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            fc.emit(Op::CheckedAdd);
        }
        Expr::BinOp {
            op: BinOp::Sub,
            left,
            right,
            ..
        } => {
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            fc.emit(Op::CheckedSub);
        }
        Expr::BinOp {
            op: BinOp::Mul,
            left,
            right,
            ..
        } => {
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            // The fraction-bit count selects integer (0) or Q-format
            // (>0) multiply on the unified opcode.
            fc.emit(Op::CheckedMul(operand_fixed_n.unwrap_or(0)));
        }
        Expr::BinOp {
            op: BinOp::Div,
            left,
            right,
            ..
        } => {
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            fc.emit(Op::CheckedDiv(operand_fixed_n.unwrap_or(0)));
        }
        Expr::BinOp {
            op: BinOp::Mod,
            left,
            right,
            ..
        } => {
            compile_expr(fc, left)?;
            compile_expr(fc, right)?;
            fc.emit(Op::CheckedMod);
        }
        Expr::UnaryOp {
            op: UnaryOp::Neg,
            operand,
            ..
        } => {
            compile_expr(fc, operand)?;
            fc.emit(Op::CheckedNeg);
        }
        _ => {
            return Err(CompileError {
                message: alloc::string::String::from(
                    "checked-overflow construct currently supports only the operators `+`, `-`, `*`, `/`, `%`, and unary `-`",
                ),
                span: *span,
            });
        }
    }

    // The operand type determines the type bound by the arm
    // patterns: `Word` for a Word construct, `Byte` for a Byte
    // construct, and `Fixed<n>` for a Fixed construct (so the
    // arm-body arithmetic dispatch routes through the Fixed opcodes).
    // The type checker has already constrained the operands to one of
    // these. `operand` was bound above for the Fixed-opcode dispatch.
    let bind_ty = if let Some(n) = operand_fixed_n {
        TypeExpr::Prim(PrimType::Fixed(Some(n)), *span)
    } else {
        match operand.and_then(|e| infer_expr_type(fc, e)) {
            Some(TypeExpr::Prim(PrimType::Byte, _)) => TypeExpr::Prim(PrimType::Byte, *span),
            _ => TypeExpr::Prim(PrimType::Word, *span),
        }
    };

    // Stack: [low, high, flag]. Stash to temporary locals. The
    // local names embed the span's start position so multiple
    // checked constructs in the same function get distinct slots.
    // The push order (low at the bottom, flag on top) lets the
    // wrapping-arithmetic synthesis `CheckedXxx; PopN(2)` discard
    // the top two slots and leave `low` on the stack.
    let suffix = span.start;
    let flag_name = alloc::format!("__checked_flag_{}", suffix);
    let low_name = alloc::format!("__checked_low_{}", suffix);
    let high_name = alloc::format!("__checked_high_{}", suffix);
    let flag_slot = fc.declare_local(&flag_name);
    let low_slot = fc.declare_local(&low_name);
    let high_slot = fc.declare_local(&high_name);
    fc.emit(Op::SetLocal(flag_slot));
    fc.emit(Op::SetLocal(high_slot));
    fc.emit(Op::SetLocal(low_slot));

    // Wrap the arm dispatch in a virtual loop so the first
    // matching arm can break out with its body's result.
    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();

    for arm in arms {
        fc.begin_scope();
        let mut fail_addrs: Vec<usize> = Vec::new();

        // Class-flag check.
        let (class_flag, single_pattern, h_pattern, l_pattern) = match &arm.kind {
            CheckedArmKind::Ok(p) => (0_i64, Some(p), None, None),
            // Word overflow/underflow bind two halves (high, low).
            CheckedArmKind::Overflow(h, Some(l)) => (1_i64, None, Some(h), Some(l)),
            CheckedArmKind::Underflow(h, Some(l)) => (2_i64, None, Some(h), Some(l)),
            // Byte overflow/underflow bind a single wrapped result,
            // which the checked Byte op places in the low slot.
            CheckedArmKind::Overflow(p, None) => (1_i64, Some(p), None, None),
            CheckedArmKind::Underflow(p, None) => (2_i64, Some(p), None, None),
            // Zero divisor: flag 3, the numerator bound through the
            // single pattern against the low slot (where the checked
            // division and modulo place it).
            CheckedArmKind::ZeroDivisor(p) => (3_i64, Some(p), None, None),
            // NaN: flag 4, the result bound through the single pattern
            // against the low slot (where the checked float op places
            // it).
            CheckedArmKind::Nan(p) => (4_i64, Some(p), None, None),
            // `invalid_index` belongs to the indexing construct, which
            // is compiled by `compile_checked_index`; reaching it here
            // means the type checker admitted it on an arithmetic
            // operation, which is an internal inconsistency.
            CheckedArmKind::InvalidIndex(_)
            | CheckedArmKind::InvalidNewtype(_)
            | CheckedArmKind::PayloadDiscriminant(_)
            | CheckedArmKind::InvalidDiscriminant(_) => {
                return Err(CompileError {
                    message: alloc::string::String::from(
                        "internal error: non-arithmetic outcome arm on an arithmetic checked construct",
                    ),
                    span: *span,
                });
            }
        };
        fc.emit(Op::GetLocal(flag_slot));
        let class_idx = fc.add_constant(Value::Int(class_flag));
        fc.emit(Op::Const(class_idx));
        fc.emit(Op::CmpEq);
        let class_fail = fc.emit_jump(Op::If(0));
        fail_addrs.push(class_fail);

        // Literal pattern tests against the high/low slots.
        let test_literal =
            |fc: &mut FuncCompiler, pat: &Pattern, slot: u16, fail_addrs: &mut Vec<usize>| {
                if let Pattern::Literal(crate::ast::Literal::Int(v), _) = pat {
                    fc.emit(Op::GetLocal(slot));
                    let idx = fc.add_constant(Value::Int(*v));
                    fc.emit(Op::Const(idx));
                    fc.emit(Op::CmpEq);
                    let fail = fc.emit_jump(Op::If(0));
                    fail_addrs.push(fail);
                }
            };
        if let Some(p) = single_pattern {
            // For `ok(p)`, the bound value is the low slot (the
            // representable result in the in-range case).
            test_literal(fc, p, low_slot, &mut fail_addrs);
        }
        if let (Some(h), Some(l)) = (h_pattern, l_pattern) {
            test_literal(fc, h, high_slot, &mut fail_addrs);
            test_literal(fc, l, low_slot, &mut fail_addrs);
        }

        // Variable bindings. The bound names carry the operand type
        // (`Word` or `Byte`); typing the binds enables Consolidation
        // B's type-driven arithmetic dispatch to route subsequent
        // `h + l` etc. expressions through the right checked op. The
        // two-pattern `(h, l)` form is only produced for `Word`.
        let bind_var = |fc: &mut FuncCompiler,
                        pat: &Pattern,
                        slot: u16,
                        ty: &TypeExpr|
         -> Result<(), CompileError> {
            if let Pattern::Variable(name, _) = pat {
                let v_slot = fc.declare_local_typed(name, Some(ty.clone()));
                fc.emit(Op::GetLocal(slot));
                fc.emit(Op::SetLocal(v_slot));
            }
            Ok(())
        };
        if let Some(p) = single_pattern {
            bind_var(fc, p, low_slot, &bind_ty)?;
        }
        if let (Some(h), Some(l)) = (h_pattern, l_pattern) {
            bind_var(fc, h, high_slot, &bind_ty)?;
            bind_var(fc, l, low_slot, &bind_ty)?;
        }

        // Guard expression.
        if let Some(guard) = arm.guard.as_ref() {
            compile_expr(fc, guard)?;
            let guard_fail = fc.emit_jump(Op::If(0));
            fail_addrs.push(guard_fail);
        }

        // Arm body, then break out of the virtual loop with the
        // result on the stack.
        compile_expr(fc, &arm.body)?;
        let break_addr = fc.emit(Op::Break(0));
        if let Some(breaks) = fc.loop_breaks.last_mut() {
            breaks.push(break_addr);
        }

        fc.end_scope();

        // Patch failure jumps in reverse to close nested Ifs.
        for addr in fail_addrs.into_iter().rev() {
            fc.patch_jump(addr);
            fc.emit(Op::EndIf);
        }
    }

    // No user arm matched the outcome. An unhandled zero divisor
    // (flag 3) traps as a division by zero, matching the contract
    // that a partial operation with no in-band result traps when
    // unhandled. Any other unhandled outcome wraps: `low` holds the
    // in-range result for `ok` and the two's-complement wrapped
    // result for `overflow` and `underflow`, so pushing it is the
    // wrapping default for the optional `overflow` and `underflow`
    // classes.
    fc.emit(Op::GetLocal(flag_slot));
    let three_idx = fc.add_constant(Value::Int(3));
    fc.emit(Op::Const(three_idx));
    fc.emit(Op::CmpEq);
    let not_zero_divisor = fc.emit_jump(Op::If(0));
    fc.emit(Op::Trap(crate::bytecode::TrapKind::ZeroDivisor.code()));
    fc.patch_jump(not_zero_divisor);
    fc.emit(Op::EndIf);

    fc.emit(Op::GetLocal(low_slot));
    let default_break = fc.emit(Op::Break(0));
    if let Some(breaks) = fc.loop_breaks.last_mut() {
        breaks.push(default_break);
    }

    let endloop_addr = fc.emit(Op::EndLoop(0));
    let after_loop = (loop_addr + 1) as u16;
    if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
        *a = after_loop;
    }
    let after_endloop = fc.chunk.ops.len() as u16;
    if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
        *a = after_endloop;
    }
    fc.exit_loop();

    Ok(())
}

/// Compile the indexing construct `array[index] { ok(v) => ...,
/// invalid_index(i) => ... }` (B35 P4). The lowering needs no new
/// opcode: it synthesizes the bounds check from `Op::Len`, integer
/// comparisons, and `Op::If`, computing an outcome flag (`0` ok, `1`
/// invalid index) and stashing the element (in-bounds) or leaving the
/// index for the `invalid_index` binding. The arm dispatch mirrors
/// `compile_checked`. An unhandled out-of-bounds index re-issues the
/// plain `Op::GetIndex`, which traps with the precise
/// `VmError::IndexOutOfBounds(index, len)`.
fn compile_checked_index(
    fc: &mut FuncCompiler,
    op_expr: &Expr,
    object: &Expr,
    index: &Expr,
    arms: &[crate::ast::CheckedArm],
    span: &Span,
) -> Result<(), CompileError> {
    use crate::ast::{CheckedArmKind, Pattern};

    // The `ok` arm binds the element type; `invalid_index` binds the
    // index `Word`. The element type comes from the array-index
    // expression's inferred type.
    let elem_ty = infer_expr_type(fc, op_expr).unwrap_or(TypeExpr::Prim(PrimType::Word, *span));
    let word_ty = TypeExpr::Prim(PrimType::Word, *span);

    let suffix = span.start;
    let arr_slot = fc.declare_local(&alloc::format!("__idx_arr_{}", suffix));
    let idx_slot = fc.declare_local(&alloc::format!("__idx_i_{}", suffix));
    let len_slot = fc.declare_local(&alloc::format!("__idx_len_{}", suffix));
    let flag_slot = fc.declare_local(&alloc::format!("__idx_flag_{}", suffix));
    let elem_slot = fc.declare_local(&alloc::format!("__idx_elem_{}", suffix));

    // arr_slot = object; idx_slot = index.
    compile_expr(fc, object)?;
    fc.emit(Op::SetLocal(arr_slot));
    compile_expr(fc, index)?;
    fc.emit(Op::SetLocal(idx_slot));
    // len_slot = len(arr).
    fc.emit(Op::GetLocal(arr_slot));
    fc.emit(Op::Len);
    fc.emit(Op::SetLocal(len_slot));
    // flag defaults to 1 (invalid_index); flipped to 0 when in range.
    let one_idx = fc.add_constant(Value::Int(1));
    fc.emit(Op::Const(one_idx));
    fc.emit(Op::SetLocal(flag_slot));
    // if idx >= 0 { if idx < len { elem = arr[idx]; flag = 0 } }
    fc.emit(Op::GetLocal(idx_slot));
    let zero_idx = fc.add_constant(Value::Int(0));
    fc.emit(Op::Const(zero_idx));
    fc.emit(Op::CmpGe);
    let ge_skip = fc.emit_jump(Op::If(0));
    fc.emit(Op::GetLocal(idx_slot));
    fc.emit(Op::GetLocal(len_slot));
    fc.emit(Op::CmpLt);
    let lt_skip = fc.emit_jump(Op::If(0));
    fc.emit(Op::GetLocal(arr_slot));
    fc.emit(Op::GetLocal(idx_slot));
    fc.emit(Op::GetIndex);
    fc.emit(Op::SetLocal(elem_slot));
    let zero_flag_idx = fc.add_constant(Value::Int(0));
    fc.emit(Op::Const(zero_flag_idx));
    fc.emit(Op::SetLocal(flag_slot));
    fc.patch_jump(lt_skip);
    fc.emit(Op::EndIf);
    fc.patch_jump(ge_skip);
    fc.emit(Op::EndIf);

    // Dispatch arms in a virtual loop, mirroring compile_checked.
    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();
    for arm in arms {
        fc.begin_scope();
        let mut fail_addrs: Vec<usize> = Vec::new();
        let (class_flag, pat, bind_slot, bind_ty): (i64, &Pattern, u16, &TypeExpr) = match &arm.kind
        {
            CheckedArmKind::Ok(p) => (0, p, elem_slot, &elem_ty),
            CheckedArmKind::InvalidIndex(p) => (1, p, idx_slot, &word_ty),
            _ => {
                return Err(CompileError {
                    message: alloc::string::String::from(
                        "internal error: non-indexing arm in an indexing checked construct",
                    ),
                    span: *span,
                });
            }
        };
        // Class-flag check.
        fc.emit(Op::GetLocal(flag_slot));
        let cidx = fc.add_constant(Value::Int(class_flag));
        fc.emit(Op::Const(cidx));
        fc.emit(Op::CmpEq);
        let class_fail = fc.emit_jump(Op::If(0));
        fail_addrs.push(class_fail);
        // Literal pattern test against the bound slot.
        if let Pattern::Literal(crate::ast::Literal::Int(v), _) = pat {
            fc.emit(Op::GetLocal(bind_slot));
            let idxc = fc.add_constant(Value::Int(*v));
            fc.emit(Op::Const(idxc));
            fc.emit(Op::CmpEq);
            let fail = fc.emit_jump(Op::If(0));
            fail_addrs.push(fail);
        }
        // Variable binding, typed so arm-body dispatch is correct.
        if let Pattern::Variable(name, _) = pat {
            let v_slot = fc.declare_local_typed(name, Some(bind_ty.clone()));
            fc.emit(Op::GetLocal(bind_slot));
            fc.emit(Op::SetLocal(v_slot));
        }
        // Guard.
        if let Some(guard) = arm.guard.as_ref() {
            compile_expr(fc, guard)?;
            let guard_fail = fc.emit_jump(Op::If(0));
            fail_addrs.push(guard_fail);
        }
        // Body, then break out of the virtual loop with the result.
        compile_expr(fc, &arm.body)?;
        let break_addr = fc.emit(Op::Break(0));
        if let Some(breaks) = fc.loop_breaks.last_mut() {
            breaks.push(break_addr);
        }
        fc.end_scope();
        for addr in fail_addrs.into_iter().rev() {
            fc.patch_jump(addr);
            fc.emit(Op::EndIf);
        }
    }

    // No arm matched. Reaching here implies the index was out of
    // bounds (flag 1) and no `invalid_index` arm covered it, since the
    // `ok` class is a mandatory catch-all. Re-issue the plain index so
    // the runtime traps with the precise `IndexOutOfBounds(index, len)`.
    fc.emit(Op::GetLocal(arr_slot));
    fc.emit(Op::GetLocal(idx_slot));
    fc.emit(Op::GetIndex);
    let default_break = fc.emit(Op::Break(0));
    if let Some(breaks) = fc.loop_breaks.last_mut() {
        breaks.push(default_break);
    }

    let endloop_addr = fc.emit(Op::EndLoop(0));
    let after_loop = (loop_addr + 1) as u16;
    if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
        *a = after_loop;
    }
    let after_endloop = fc.chunk.ops.len() as u16;
    if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
        *a = after_endloop;
    }
    fc.exit_loop();

    Ok(())
}

/// Compile the newtype-construction construct `Name(value) { ok(v)
/// => ..., invalid_newtype(x) => ... }` (B35 P5). The lowering needs
/// no new opcode: it computes the underlying value, runs the
/// refinement predicate when one exists, and branches on the result
/// into an outcome flag (`0` ok, `1` invalid newtype). The arm
/// dispatch mirrors the other constructs. `ok` is a mandatory
/// catch-all; `invalid_newtype` is optional, and an unhandled failure
/// traps with `TrapKind::RefinementFailed`, the same fault a bare
/// construction produces.
fn compile_checked_newtype(
    fc: &mut FuncCompiler,
    op_expr: &Expr,
    newtype_name: &str,
    arg: &Expr,
    arms: &[crate::ast::CheckedArm],
    span: &Span,
) -> Result<(), CompileError> {
    use crate::ast::{CheckedArmKind, Pattern};

    // `ok` binds the newtype; `invalid_newtype` binds the underlying
    // value. Newtypes are transparent at runtime, so both read the
    // same slot; only the bound type differs.
    let newtype_ty = infer_expr_type(fc, op_expr).unwrap_or_else(|| {
        TypeExpr::Named(alloc::string::String::from(newtype_name), Vec::new(), *span)
    });
    let underlying_ty = infer_expr_type(fc, arg).unwrap_or(TypeExpr::Prim(PrimType::Word, *span));

    let suffix = span.start;
    let value_slot = fc.declare_local(&alloc::format!("__nt_val_{}", suffix));
    let flag_slot = fc.declare_local(&alloc::format!("__nt_flag_{}", suffix));

    // value_slot = the underlying value (the constructor is
    // transparent, so this is just the argument).
    compile_expr(fc, arg)?;
    fc.emit(Op::SetLocal(value_slot));

    // Compute the outcome flag. With a refinement predicate, run it
    // and set flag 0 on success, leaving the default 1 on failure.
    // Without a refinement the construction is total, so flag is 0.
    if let Some(pred_name) = fc.type_info.newtype_refinements.get(newtype_name).cloned() {
        let pred_idx = *fc
            .function_map
            .get(pred_name.as_str())
            .ok_or_else(|| CompileError {
                message: alloc::format!(
                    "refinement predicate `{}` for newtype `{}` is not a declared function",
                    pred_name,
                    newtype_name
                ),
                span: *span,
            })?;
        let one_idx = fc.add_constant(Value::Int(1));
        fc.emit(Op::Const(one_idx));
        fc.emit(Op::SetLocal(flag_slot));
        fc.emit(Op::GetLocal(value_slot));
        fc.emit(Op::Call(pred_idx, 1));
        let pred_fail = fc.emit_jump(Op::If(0));
        let zero_idx = fc.add_constant(Value::Int(0));
        fc.emit(Op::Const(zero_idx));
        fc.emit(Op::SetLocal(flag_slot));
        fc.patch_jump(pred_fail);
        fc.emit(Op::EndIf);
    } else {
        let zero_idx = fc.add_constant(Value::Int(0));
        fc.emit(Op::Const(zero_idx));
        fc.emit(Op::SetLocal(flag_slot));
    }

    // Dispatch arms in a virtual loop, mirroring compile_checked.
    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();
    for arm in arms {
        fc.begin_scope();
        let mut fail_addrs: Vec<usize> = Vec::new();
        let (class_flag, pat, bind_ty): (i64, &Pattern, &TypeExpr) = match &arm.kind {
            CheckedArmKind::Ok(p) => (0, p, &newtype_ty),
            CheckedArmKind::InvalidNewtype(p) => (1, p, &underlying_ty),
            _ => {
                return Err(CompileError {
                    message: alloc::string::String::from(
                        "internal error: non-newtype arm in a newtype-construction checked construct",
                    ),
                    span: *span,
                });
            }
        };
        fc.emit(Op::GetLocal(flag_slot));
        let cidx = fc.add_constant(Value::Int(class_flag));
        fc.emit(Op::Const(cidx));
        fc.emit(Op::CmpEq);
        let class_fail = fc.emit_jump(Op::If(0));
        fail_addrs.push(class_fail);
        if let Pattern::Literal(crate::ast::Literal::Int(v), _) = pat {
            fc.emit(Op::GetLocal(value_slot));
            let idxc = fc.add_constant(Value::Int(*v));
            fc.emit(Op::Const(idxc));
            fc.emit(Op::CmpEq);
            let fail = fc.emit_jump(Op::If(0));
            fail_addrs.push(fail);
        }
        if let Pattern::Variable(vname, _) = pat {
            let v_slot = fc.declare_local_typed(vname, Some(bind_ty.clone()));
            fc.emit(Op::GetLocal(value_slot));
            fc.emit(Op::SetLocal(v_slot));
        }
        if let Some(guard) = arm.guard.as_ref() {
            compile_expr(fc, guard)?;
            let guard_fail = fc.emit_jump(Op::If(0));
            fail_addrs.push(guard_fail);
        }
        compile_expr(fc, &arm.body)?;
        let break_addr = fc.emit(Op::Break(0));
        if let Some(breaks) = fc.loop_breaks.last_mut() {
            breaks.push(break_addr);
        }
        fc.end_scope();
        for addr in fail_addrs.into_iter().rev() {
            fc.patch_jump(addr);
            fc.emit(Op::EndIf);
        }
    }

    // No arm matched. The `ok` class is a mandatory catch-all, so the
    // flag is 1 (invalid newtype). Trap with RefinementFailed, the
    // same fault a bare construction produces. The trailing default
    // break keeps the loop structurally closed.
    fc.emit(Op::GetLocal(flag_slot));
    let one_idx = fc.add_constant(Value::Int(1));
    fc.emit(Op::Const(one_idx));
    fc.emit(Op::CmpEq);
    let not_invalid = fc.emit_jump(Op::If(0));
    fc.emit(Op::Trap(crate::bytecode::TrapKind::RefinementFailed.code()));
    fc.patch_jump(not_invalid);
    fc.emit(Op::EndIf);
    fc.emit(Op::GetLocal(value_slot));
    let default_break = fc.emit(Op::Break(0));
    if let Some(breaks) = fc.loop_breaks.last_mut() {
        breaks.push(default_break);
    }

    let endloop_addr = fc.emit(Op::EndLoop(0));
    let after_loop = (loop_addr + 1) as u16;
    if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
        *a = after_loop;
    }
    let after_endloop = fc.chunk.ops.len() as u16;
    if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
        *a = after_endloop;
    }
    fc.exit_loop();

    Ok(())
}

/// Compile the discriminant-to-enum construct `discriminant as Enum {
/// ok(Variant) => ..., payload_discriminant(Variant) => ...,
/// invalid_discriminant(raw) => ... }` (B35 P6). The lowering needs
/// no new opcode: it evaluates the `Word` discriminant once, then for
/// each variant emits `if discriminant == variant_discriminant {
/// <action> }`. A unit variant's action is the matching `ok` arm
/// body, a generic `ok` body binding the variant value, or the
/// variant value itself when no `ok` arm covers it. A payload
/// variant's action is the matching `payload_discriminant` arm body.
/// An unmatched discriminant runs the `invalid_discriminant` arm or
/// traps with `TrapKind::EnumVariantUnmapped`.
fn compile_checked_discriminant(
    fc: &mut FuncCompiler,
    inner: &Expr,
    enum_name: &str,
    arms: &[crate::ast::CheckedArm],
    span: &Span,
) -> Result<(), CompileError> {
    use crate::ast::{CheckedArmKind, Pattern};

    // Variant-name extraction: an upper-case `Variable` names a
    // variant; lower-case or `_` is a binder or catch-all.
    fn variant_name(p: &Pattern) -> Option<&str> {
        match p {
            Pattern::Variable(name, _) if name.chars().next().is_some_and(|c| c.is_uppercase()) => {
                Some(name)
            }
            _ => None,
        }
    }

    // Resolve, for each kind of arm, the body to emit per variant.
    // `ok` specific arms by variant name; the first generic `ok`
    // (lower-case binder or `_`); `payload_discriminant` specific arms
    // by variant name; the first `payload_discriminant(_)` catch-all;
    // and the `invalid_discriminant` arm.
    let mut ok_specific: BTreeMap<&str, &crate::ast::CheckedArm> = BTreeMap::new();
    let mut ok_generic: Option<&crate::ast::CheckedArm> = None;
    let mut payload_specific: BTreeMap<&str, &crate::ast::CheckedArm> = BTreeMap::new();
    let mut payload_catchall: Option<&crate::ast::CheckedArm> = None;
    let mut invalid_arm: Option<&crate::ast::CheckedArm> = None;
    for arm in arms {
        match &arm.kind {
            CheckedArmKind::Ok(p) => match variant_name(p) {
                Some(v) => {
                    ok_specific.insert(v, arm);
                }
                None => {
                    if ok_generic.is_none() {
                        ok_generic = Some(arm);
                    }
                }
            },
            CheckedArmKind::PayloadDiscriminant(p) => match variant_name(p) {
                Some(v) => {
                    payload_specific.insert(v, arm);
                }
                None => {
                    if payload_catchall.is_none() {
                        payload_catchall = Some(arm);
                    }
                }
            },
            CheckedArmKind::InvalidDiscriminant(_) if invalid_arm.is_none() => {
                invalid_arm = Some(arm);
            }
            _ => {}
        }
    }

    // Evaluate the discriminant once into a temporary.
    let suffix = span.start;
    let temp = fc.declare_local(&alloc::format!("__disc_{}", suffix));
    compile_expr(fc, inner)?;
    fc.emit(Op::SetLocal(temp));

    // Snapshot the variant table (name, discriminant) in declaration
    // order, and the per-variant arity from the payload-field map.
    let variants: Vec<(String, i64)> = fc
        .type_info
        .enum_variant_order
        .get(enum_name)
        .cloned()
        .unwrap_or_default();
    let e_const = fc.add_string_constant(enum_name);

    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();

    for (vname, disc) in &variants {
        let arity = fc
            .type_info
            .enums
            .get(enum_name)
            .and_then(|m| m.get(vname))
            .map(|fields| fields.len())
            .unwrap_or(0);

        fc.begin_scope();
        // if discriminant == this variant's discriminant
        fc.emit(Op::GetLocal(temp));
        let disc_const = fc.add_constant(Value::Int(*disc));
        fc.emit(Op::Const(disc_const));
        fc.emit(Op::CmpEq);
        let fail = fc.emit_jump(Op::If(0));

        if arity == 0 {
            // Unit variant. Specific `ok` overrides; else a generic
            // `ok` binds the variant value and runs its body; else the
            // variant converts to itself.
            if let Some(arm) = ok_specific.get(vname.as_str()) {
                compile_expr(fc, &arm.body)?;
            } else if let Some(arm) = ok_generic {
                let v_const = fc.add_string_constant(vname);
                fc.emit(Op::NewEnum(e_const, v_const, 0));
                match &arm.kind {
                    CheckedArmKind::Ok(Pattern::Variable(bind, _)) => {
                        let slot = fc.declare_local_typed(
                            bind,
                            Some(TypeExpr::Named(
                                alloc::string::String::from(enum_name),
                                Vec::new(),
                                *span,
                            )),
                        );
                        fc.emit(Op::SetLocal(slot));
                    }
                    // Wildcard generic `ok`: discard the value.
                    _ => {
                        fc.emit(Op::PopN(1));
                    }
                }
                compile_expr(fc, &arm.body)?;
            } else {
                // No `ok` arm: the unit variant converts to itself.
                let v_const = fc.add_string_constant(vname);
                fc.emit(Op::NewEnum(e_const, v_const, 0));
            }
        } else {
            // Payload variant. The body constructs the value; a
            // specific arm wins over the catch-all. The type checker
            // guarantees one of them exists.
            let arm = payload_specific
                .get(vname.as_str())
                .copied()
                .or(payload_catchall);
            match arm {
                Some(arm) => compile_expr(fc, &arm.body)?,
                None => {
                    return Err(CompileError {
                        message: alloc::format!(
                            "internal error: payload-bearing variant `{}` is uncovered in a discriminant-to-enum conversion",
                            vname
                        ),
                        span: *span,
                    });
                }
            }
        }

        let break_addr = fc.emit(Op::Break(0));
        if let Some(breaks) = fc.loop_breaks.last_mut() {
            breaks.push(break_addr);
        }
        fc.patch_jump(fail);
        fc.emit(Op::EndIf);
        fc.end_scope();
    }

    // No variant matched. Run the `invalid_discriminant` arm (binding
    // the raw `Word`) or trap with EnumVariantUnmapped.
    fc.begin_scope();
    if let Some(arm) = invalid_arm {
        if let CheckedArmKind::InvalidDiscriminant(Pattern::Variable(bind, _)) = &arm.kind {
            let slot = fc.declare_local_typed(bind, Some(TypeExpr::Prim(PrimType::Word, *span)));
            fc.emit(Op::GetLocal(temp));
            fc.emit(Op::SetLocal(slot));
        }
        compile_expr(fc, &arm.body)?;
        let break_addr = fc.emit(Op::Break(0));
        if let Some(breaks) = fc.loop_breaks.last_mut() {
            breaks.push(break_addr);
        }
    } else {
        fc.emit(Op::Trap(
            crate::bytecode::TrapKind::EnumVariantUnmapped.code(),
        ));
    }
    fc.end_scope();

    let endloop_addr = fc.emit(Op::EndLoop(0));
    let after_loop = (loop_addr + 1) as u16;
    if let Op::EndLoop(a) = &mut fc.chunk.ops[endloop_addr] {
        *a = after_loop;
    }
    let after_endloop = fc.chunk.ops.len() as u16;
    if let Op::Loop(a) = &mut fc.chunk.ops[loop_addr] {
        *a = after_endloop;
    }
    fc.exit_loop();

    Ok(())
}

/// Compile a function call by name.
fn compile_call(
    fc: &mut FuncCompiler,
    name: &str,
    args: &[Expr],
    span: &Span,
) -> Result<(), CompileError> {
    // Newtype construction. `Name(value)` where `Name` is a
    // declared newtype emits the inner expression's bytecode
    // directly with no `Op::Call`. Newtypes are transparent at the
    // runtime layer; the wrapper exists only at the type-checker
    // level. The type checker has already validated the argument
    // against the underlying type.
    //
    // If the newtype carries a refinement predicate, the compiled
    // form additionally emits a `Dup`, a `Call(predicate, 1)`, an
    // `If` block, and a `Trap` for the false branch. The
    // predicate is required to be a declared atomic-total function
    // with signature `fn(Underlying) -> Bool`; the type checker
    // has already validated the signature.
    if fc.type_info.newtype_names.contains(name) {
        if args.len() != 1 {
            return Err(CompileError {
                message: alloc::format!(
                    "newtype `{}` constructor expects 1 argument, got {}",
                    name,
                    args.len()
                ),
                span: *span,
            });
        }
        // Refinement-elision pathway. Two layers in priority
        // order. (1) Constant fold: if the argument expression
        // reduces to a single integer (literal, let-bound
        // constant, arithmetic over the above), evaluate the
        // predicate at that integer. (2) Range subset: if the
        // argument expression has an inferable interval (e.g.
        // the argument is a function parameter declared as a
        // refined newtype) and the predicate decomposes to a
        // convex true set, admit when the argument range is a
        // subset of the true set. Both pathways fall through to
        // the runtime check when undecided.
        if let Some(pred_name) = fc.type_info.newtype_refinements.get(name).cloned()
            && let Some((param_name, body)) =
                fc.type_info.refinement_bodies.get(&pred_name).cloned()
        {
            // Layer 1: constant fold.
            if let Some(n) = fold_to_int(&args[0], &|s| fc.local_const_lookup(s)) {
                match eval_predicate_at_int(&body, &param_name, n) {
                    Some(true) => {
                        compile_expr(fc, &args[0])?;
                        return Ok(());
                    }
                    Some(false) => {
                        return Err(CompileError {
                            message: alloc::format!(
                                "refinement check `{}` provably fails for newtype `{}` at compile time on argument {}",
                                pred_name,
                                name,
                                n
                            ),
                            span: *span,
                        });
                    }
                    None => {}
                }
            }
            // Layer 2: range subset via the interval lattice.
            if let Some(arg_range) = infer_arg_range(&args[0], fc)
                && let Some(true_set) = predicate_true_set(&body, &param_name)
            {
                if !arg_range.is_empty() && arg_range.is_subset_of(&true_set) {
                    compile_expr(fc, &args[0])?;
                    return Ok(());
                }
                if !arg_range.is_empty() && arg_range.intersect(&true_set).is_empty() {
                    return Err(CompileError {
                        message: alloc::format!(
                            "refinement check `{}` provably fails for newtype `{}` at compile time; argument range is disjoint from the predicate's true set",
                            pred_name,
                            name
                        ),
                        span: *span,
                    });
                }
            }
        }
        compile_expr(fc, &args[0])?;
        if let Some(pred_name) = fc.type_info.newtype_refinements.get(name).cloned() {
            let pred_idx =
                *fc.function_map
                    .get(pred_name.as_str())
                    .ok_or_else(|| CompileError {
                        message: alloc::format!(
                            "refinement predicate `{}` for newtype `{}` is not a declared function",
                            pred_name,
                            name
                        ),
                        span: *span,
                    })?;
            // Stack: [value]
            fc.emit(Op::Dup);
            // Stack: [value, value]
            fc.emit(Op::Call(pred_idx, 1));
            // Stack: [value, bool] (the call popped 1 arg, pushed 1 result)
            let if_addr = fc.emit_jump(Op::If(0));
            // True branch: predicate succeeded. Stack: [value].
            // No additional work; fall through to Else which
            // jumps to EndIf.
            let else_addr = fc.emit_jump(Op::Else(0));
            fc.patch_jump(if_addr);
            // False branch: predicate failed. Trap with the
            // refinement-failed kind. The host categorizes the fault
            // through `VmError::Trap(TrapKind::RefinementFailed)`.
            fc.emit(Op::Trap(crate::bytecode::TrapKind::RefinementFailed.code()));
            fc.patch_jump(else_addr);
            fc.emit(Op::EndIf);
            // Stack: [value]
        }
        return Ok(());
    }
    // V0.2.0 Phase 4 dropped indirect-call dispatch. A name that
    // resolves to a local in a call position is no longer a
    // `Value::Func` invocation; the type checker rejects the
    // closures and first-class function values that produced
    // such locals. Reaching this point with a local name in a
    // call position means the user invoked something that is not
    // a callable.
    if let Some(_slot) = fc.resolve_local(name) {
        return Err(CompileError {
            message: alloc::format!(
                "`{}` is a local variable, not a callable. \
                 V0.2.0 admits only direct calls to top-level \
                 functions, methods, and host natives.",
                name
            ),
            span: *span,
        });
    }
    for arg in args {
        compile_expr(fc, arg)?;
    }
    let arg_count = args.len() as u8;

    if let Some(&idx) = fc.function_map.get(name) {
        fc.emit(Op::Call(idx, arg_count));
    } else if let Some(&idx) = fc.native_map.get(name) {
        let is_external = fc.native_externals.get(name).copied().unwrap_or(false);
        if is_external {
            fc.emit(Op::CallExternalNative(idx, arg_count));
        } else {
            fc.emit(Op::CallVerifiedNative(idx, arg_count));
        }
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
                Literal::Byte(v) => {
                    let idx = fc.add_constant(Value::Byte(*v));
                    fc.emit(Op::Const(idx));
                }
                Literal::Fixed { raw, .. } => {
                    let idx = fc.add_constant(Value::Fixed(*raw));
                    fc.emit(Op::Const(idx));
                }
                #[cfg(feature = "floats")]
                Literal::Float(v) => {
                    let idx = fc.add_constant(Value::Float(*v));
                    fc.emit(Op::Const(idx));
                }
                #[cfg(not(feature = "floats"))]
                Literal::Float(_) => {
                    unreachable!(
                        "float literals are rejected at lex time when the `floats` feature is off"
                    );
                }
                Literal::String(s) => {
                    let idx = fc.add_constant(Value::StaticStr(s.clone()));
                    fc.emit(Op::Const(idx));
                }
                Literal::Bool(true) => {
                    fc.emit(Op::PushImmediate(1));
                }
                Literal::Bool(false) => {
                    fc.emit(Op::PushImmediate(2));
                }
                Literal::Unit => {
                    fc.emit(Op::PushImmediate(0));
                }
            }
            fc.emit(Op::CmpEq);
            fail_addrs.push(fc.emit_jump(Op::If(0)));
        }
        Pattern::Enum(enum_name, variant, sub_pats, _) => {
            // `Option::None` matches `Value::None` directly rather
            // than going through `IsEnum`, because the compiler
            // emits `Op::PushNone` for `Option::None` constructions
            // and host-side natives return `Value::None` for the
            // None case. The `IsEnum` check would fail against
            // `Value::None` because it is not a `Value::Enum`.
            //
            // `Option::Some(p)` continues to use `IsEnum` because
            // the compiler emits `Op::NewEnum` for `Option::Some(x)`,
            // producing a `Value::Enum { type_name: "Option",
            // variant: "Some", fields: [x] }`. Host-side natives
            // that produce `Option::Some(v)` must construct the
            // same `Value::Enum` shape.
            if enum_name == "Option" && variant == "None" {
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::PushImmediate(3));
                fc.emit(Op::CmpEq);
                fail_addrs.push(fc.emit_jump(Op::If(0)));
                return Ok(fail_addrs);
            }

            fc.emit(Op::GetLocal(value_slot));
            let e_const = fc.add_string_constant(enum_name);
            let v_const = fc.add_string_constant(variant);
            fc.emit(Op::IsEnum(e_const, v_const));
            fail_addrs.push(fc.emit_jump(Op::If(0)));
            fc.emit(Op::PopN(1)); // Discard the peeked value.

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
            fc.emit(Op::PopN(1));

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
                        fc.emit(Op::PopN(1));
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
    use alloc::string::ToString;

    fn compile_str(src: &str) -> Result<Module, CompileError> {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        compile(&program)
    }

    #[test]
    fn compile_simple_fn() {
        let module = compile_str("fn add(a: Word, b: Word) -> Word { a + b }").unwrap();
        assert_eq!(module.chunks.len(), 1);
        assert_eq!(module.chunks[0].name, "add");
        assert_eq!(module.chunks[0].param_count, 2);
    }

    #[test]
    fn compile_literal_fn() {
        let module = compile_str("fn fortytwo() -> Word { 42 }").unwrap();
        assert_eq!(module.chunks.len(), 1);
        // Should have a Const instruction and Return.
        assert!(module.chunks[0].ops.contains(&Op::Return));
    }

    #[test]
    fn compile_if_else() {
        let module =
            compile_str("fn max(a: Word, b: Word) -> Word { if a > b { a } else { b } }").unwrap();
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
        let module = compile_str("fn double(x: Word) -> Word { let y = x * 2; y }").unwrap();
        assert_eq!(module.chunks.len(), 1);
    }

    #[test]
    fn compile_for_range() {
        let module = compile_str(
            "fn sum_to(n: Word) -> Word { let total = 0; for i in 0..n { let x = total + i; } total }"
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
            "fn double(x: Word) -> Word { x * 2 }\nfn quad(x: Word) -> Word { double(double(x)) }",
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
            "fn classify(0) -> Text { \"zero\" }\nfn classify(x: Word) -> Text { \"other\" }",
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
    fn compile_enum_to_word_cast_implicit() {
        // Implicit discriminants 0, 1, 2.
        let module = compile_str(
            "enum Color { Red, Green, Blue }\n\
             fn pick() -> Word { let c = Color::Green(); c as Word }",
        )
        .unwrap();
        assert_eq!(module.chunks.len(), 1);
        // The cast should emit IsEnum and Loop opcodes for the
        // dispatch. The specific Const indices vary with the
        // constant pool layout; we only verify the dispatch
        // shape is present.
        let ops = &module.chunks[0].ops;
        let isenum_count = ops
            .iter()
            .filter(|op| matches!(op, Op::IsEnum(_, _)))
            .count();
        assert!(
            isenum_count >= 3,
            "expected at least one IsEnum per variant, got {}",
            isenum_count
        );
        assert!(ops.iter().any(|op| matches!(op, Op::Loop(_))));
        assert!(ops.iter().any(|op| matches!(op, Op::EndLoop(_))));
    }

    #[test]
    fn compile_enum_to_word_cast_explicit_discriminants() {
        // Explicit discriminants 10, 20, 30.
        let module = compile_str(
            "enum Code { A = 10, B = 20, C = 30 }\n\
             fn pick() -> Word { let c = Code::B(); c as Word }",
        )
        .unwrap();
        // The constant pool must contain the explicit discriminants.
        let consts = &module.chunks[0].constants;
        let int_consts: Vec<i64> = consts
            .iter()
            .filter_map(|c| match c {
                ConstValue::Int(n) => Some(*n),
                _ => None,
            })
            .collect();
        for expected in &[10, 20, 30] {
            assert!(
                int_consts.contains(expected),
                "expected discriminant {} in constant pool, got {:?}",
                expected,
                int_consts
            );
        }
    }

    #[test]
    fn compile_enum_to_word_cast_negative_discriminant() {
        let module = compile_str(
            "enum Sign { Neg = -1, Zero = 0, Pos = 1 }\n\
             fn pick() -> Word { let s = Sign::Neg(); s as Word }",
        )
        .unwrap();
        let consts = &module.chunks[0].constants;
        let int_consts: Vec<i64> = consts
            .iter()
            .filter_map(|c| match c {
                ConstValue::Int(n) => Some(*n),
                _ => None,
            })
            .collect();
        assert!(int_consts.contains(&-1));
        assert!(int_consts.contains(&1));
    }

    #[test]
    fn compile_struct_init() {
        let module = compile_str(
            "struct Point { x: Word, y: Word }\nfn make() -> Point { let p = Point { x: 1, y: 2 }; p }",
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
        let module = compile_str("yield process(input: Word) -> Word { yield input * 2 }").unwrap();
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
            compile_str("loop main(input: Word) -> Word { let input = yield input + 1; input }")
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
        let module = compile_str("fn main(x: Word) -> Word { x }").unwrap();
        assert!(module.entry_point.is_some());
    }

    #[test]
    fn compile_pipeline() {
        let module = compile_str(
            "fn double(x: Word) -> Word { x * 2 }\nfn apply(x: Word) -> Word { x |> double() }",
        )
        .unwrap();
        assert_eq!(module.chunks.len(), 2);
    }

    #[test]
    fn error_undefined_variable() {
        let result = compile_str("fn bad() -> Word { unknown }");
        assert!(result.is_err());
    }

    #[test]
    fn error_undefined_function() {
        let result = compile_str("fn bad() -> Word { missing(1) }");
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
            compile_str("fn main() -> Word { let s = 0; for x in [1, 2, 3] { let s = s + x; } s }")
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
        let module =
            compile_str("fn main() -> (Word, Word, Word) { let t = (1, 2, 3); t }").unwrap();
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
        let module = compile_str("fn main() -> Word { if true { 1 } else { 2 } }").unwrap();
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
        let src = "data ctx { score: Word, level: Word, ratio: Float, alive: bool }\n\
                   fn main() -> Word { ctx.score }";
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
        let src = "data ctx { pos: (Float, Float) }\n\
                   fn main() -> (Float, Float) { ctx.pos }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_array_of_admissible() {
        let src = "data ctx { samples: [Float; 4] }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_option_of_admissible() {
        let src = "data ctx { last: Option<Word> }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_struct_of_admissible() {
        let src = "struct Point { x: Float, y: Float }\n\
                   data ctx { origin: Point }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_admits_enum_of_admissible() {
        let src = "enum Status { Idle, Active(Word), Error(Word, Word) }\n\
                   data ctx { state: Status }\n\
                   fn main() -> () { () }";
        assert!(compile_str(src).is_ok());
    }

    #[test]
    fn data_block_rejects_string() {
        let src = "data ctx { name: Text }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Text"));
    }

    #[test]
    fn data_block_rejects_string_in_tuple() {
        let src = "data ctx { pair: (Word, Text) }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Text"));
    }

    #[test]
    fn data_block_rejects_string_in_array() {
        let src = "data ctx { names: [Text; 4] }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Text"));
    }

    #[test]
    fn data_block_rejects_string_in_option() {
        let src = "data ctx { last: Option<Text> }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Text"));
    }

    #[test]
    fn data_block_rejects_string_in_struct() {
        let src = "struct Tag { label: Text }\n\
                   data ctx { t: Tag }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Text"));
    }

    #[test]
    fn data_block_rejects_string_in_enum() {
        let src = "enum Tag { Named(Text), Unnamed }\n\
                   data ctx { t: Tag }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Text"));
    }

    #[test]
    fn data_block_rejects_unknown_named_type() {
        let src = "data ctx { handle: Mystery }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("Mystery") || err.message.contains("opaque"));
    }

    #[test]
    fn multiple_data_blocks_same_visibility_rejected() {
        // Two shared blocks remain rejected under R28. One shared
        // plus one private would be accepted; see
        // `mixed_data_partitions_correctly` in vm.rs tests.
        let src = "data ctx_a { x: Word }\n\
                   data ctx_b { y: Word }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("R28") || err.message.contains("one"));
    }

    #[test]
    fn two_private_data_blocks_rejected() {
        let src = "private data ctx_a { x: Word }\n\
                   private data ctx_b { y: Word }\n\
                   fn main() -> () { () }";
        let err = compile_str(src).unwrap_err();
        assert!(err.message.contains("R28") || err.message.contains("private"));
    }

    #[test]
    fn no_data_block_compiles() {
        let module = compile_str("fn main() -> Word { 42 }").unwrap();
        assert!(module.data_layout.is_none());
    }

    #[test]
    fn untyped_param_is_inferred_from_return_type() {
        // `fn main(x) -> Word { x }`: the parameter has no
        // annotation but is returned at the body's tail, so
        // inference must resolve `x` to `Word`. The compiler reads
        // the resolved type back from the typechecker's
        // write-back pass and surfaces it through the chunk's
        // `param_types` so `Vm::call` rejects wrong-typed args.
        let module = compile_str("fn main(x) -> Word { x }").expect("compile");
        assert_eq!(module.chunks.len(), 1);
        assert_eq!(module.chunks[0].param_count, 1);
        assert_eq!(
            module.chunks[0].param_types,
            alloc::vec![crate::bytecode::TypeTag::Word],
        );
    }

    #[test]
    fn multiheaded_fn_main_dispatches() {
        let src = "fn main(0) -> Word { 100 }\n\
                   fn main(x: Word) -> Word { x }";
        let module = compile_str(src).expect("compile");
        assert_eq!(module.chunks.len(), 1);
        assert_eq!(module.chunks[0].block_type, BlockType::Func);
        assert!(module.entry_point.is_some());
    }

    #[test]
    fn multiheaded_yield_main_dispatches() {
        let src = "yield main(0) -> Word { yield 100 }\n\
                   yield main(x: Word) -> Word { yield x }";
        let module = compile_str(src).expect("compile");
        assert_eq!(module.chunks.len(), 1);
        assert_eq!(module.chunks[0].block_type, BlockType::Reentrant);
        assert!(module.entry_point.is_some());
    }

    #[test]
    fn multiheaded_loop_main_dispatches() {
        // Multi-headed Stream dispatch is wrapped in Loop/EndLoop
        // so each matched head can Break out to the shared Reset
        // epilogue while the Stream block retains the single
        // Stream and single Reset that the verifier requires.
        let src = "loop main(0) -> Word { yield 100 }\n\
                   loop main(x: Word) -> Word { let z = yield x; z }";
        let module = compile_str(src).expect("compile");
        assert_eq!(module.chunks.len(), 1);
        assert_eq!(module.chunks[0].block_type, BlockType::Stream);
        assert!(module.entry_point.is_some());
        // Exactly one Stream, exactly one Reset (verifier invariant).
        let stream_count = module.chunks[0]
            .ops
            .iter()
            .filter(|op| matches!(op, Op::Stream))
            .count();
        let reset_count = module.chunks[0]
            .ops
            .iter()
            .filter(|op| matches!(op, Op::Reset))
            .count();
        assert_eq!(stream_count, 1);
        assert_eq!(reset_count, 1);
    }

    #[test]
    fn duplicate_fn_main_is_rejected() {
        let err = compile_str(
            "fn main() -> Word { 1 }\n\
             fn main() -> Word { 2 }",
        )
        .expect_err("expected duplicate-head rejection");
        assert!(
            err.message.contains("dead code"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn duplicate_yield_main_is_rejected() {
        let err = compile_str(
            "yield main(x: Word) -> Word { yield x }\n\
             yield main(x: Word) -> Word { yield x + 1 }",
        )
        .expect_err("expected duplicate-head rejection");
        assert!(
            err.message.contains("dead code"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn duplicate_loop_main_is_rejected() {
        let err = compile_str(
            "loop main(x: Word) -> Word { let z = yield x; z }\n\
             loop main(x: Word) -> Word { let z = yield x; z }",
        )
        .expect_err("expected duplicate-head rejection");
        assert!(
            err.message.contains("dead code"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn duplicate_non_entry_function_is_rejected() {
        let err = compile_str(
            "fn helper(x: Word) -> Word { x }\n\
             fn helper(x: Word) -> Word { x + 1 }\n\
             fn main() -> Word { helper(0) }",
        )
        .expect_err("expected duplicate-head rejection");
        assert!(
            err.message.contains("dead code"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    #[cfg(feature = "verify")]
    fn closure_compile_error_carries_source_span() {
        // V0.2.0 Phase 4 retired the closure family. The rejection
        // now fires at the type-checker stage with a real source
        // span pointing at the closure expression rather than the
        // synthetic-chunk default.
        let src = "fn main() -> Word {\n\
                       let fact = |n: Word| if n <= 1 { 1 } else { n * fact(n - 1) };\n\
                       fact(5)\n\
                   }";
        let err = compile_str(src).expect_err("expected rejection");
        assert!(
            err.message.contains("closures are not supported"),
            "unexpected error: {}",
            err.message
        );
        assert_ne!(
            err.span,
            crate::token::Span::default(),
            "expected source span on closure rejection",
        );
    }

    #[test]
    fn chunk_size_thresholds_are_consistent() {
        // The soft threshold sits at 80% of the hard limit, and
        // the hard limit matches the u16 control-flow operand
        // width.
        assert_eq!(CHUNK_SIZE_HARD_LIMIT, u16::MAX as usize);
        assert_eq!(
            CHUNK_SIZE_SOFT_WARN_THRESHOLD,
            (CHUNK_SIZE_HARD_LIMIT * 80) / 100
        );
        // The relation is checked through equality above; the
        // ordering of the two constants is a derived property.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(CHUNK_SIZE_SOFT_WARN_THRESHOLD < CHUNK_SIZE_HARD_LIMIT);
        }
    }

    #[test]
    fn small_chunk_produces_no_warnings() {
        // A minimal program is well below the soft threshold so
        // the warnings vector is empty.
        let tokens = tokenize("fn main() -> Word { 1 + 2 }").expect("lex");
        let program = parse(&tokens).expect("parse");
        let (_module, warnings) =
            compile_with_warnings(&program, &crate::target::Target::host()).unwrap();
        assert!(warnings.is_empty(), "unexpected warnings: {:?}", warnings);
    }

    fn make_chunk_with_ops(name: &str, ops: alloc::vec::Vec<crate::bytecode::Op>) -> Chunk {
        Chunk {
            name: name.into(),
            ops,
            constants: alloc::vec::Vec::new(),
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: crate::bytecode::BlockType::Func,
            param_types: alloc::vec::Vec::new(),
        }
    }

    #[test]
    fn soft_warning_fires_on_long_chunk() {
        // Construct a chunk with `CHUNK_SIZE_SOFT_WARN_THRESHOLD + 1`
        // ops: just over the 80% mark, comfortably below the hard
        // cap. The helper appends one `CompileWarning` and
        // returns Ok.
        let op_count = CHUNK_SIZE_SOFT_WARN_THRESHOLD + 1;
        let ops: alloc::vec::Vec<crate::bytecode::Op> =
            (0..op_count).map(|_| crate::bytecode::Op::Return).collect();
        let chunk = make_chunk_with_ops("long_chunk", ops);
        let mut warnings: alloc::vec::Vec<CompileWarning> = alloc::vec::Vec::new();
        check_chunk_size_against_limits(&chunk, crate::token::Span::default(), &mut warnings)
            .expect("admissible at this threshold");
        assert_eq!(warnings.len(), 1, "expected one soft warning");
        let warning = &warnings[0];
        assert_eq!(warning.chunk_name, "long_chunk");
        assert!(
            warning.message.contains("soft-warning threshold"),
            "unexpected warning message: {}",
            warning.message,
        );
        assert!(
            warning.message.contains(&op_count.to_string()),
            "warning message should name the op count, got: {}",
            warning.message,
        );
    }

    #[test]
    fn hard_cap_rejects_oversize_chunk() {
        // One op past the hard cap. The helper returns
        // CompileError without populating the warnings vec.
        let op_count = CHUNK_SIZE_HARD_LIMIT + 1;
        let ops: alloc::vec::Vec<crate::bytecode::Op> =
            (0..op_count).map(|_| crate::bytecode::Op::Return).collect();
        let chunk = make_chunk_with_ops("oversize_chunk", ops);
        let mut warnings: alloc::vec::Vec<CompileWarning> = alloc::vec::Vec::new();
        let err =
            check_chunk_size_against_limits(&chunk, crate::token::Span::default(), &mut warnings)
                .unwrap_err();
        assert!(
            err.message.contains("oversize_chunk"),
            "diagnostic should name the chunk: {}",
            err.message,
        );
        assert!(err.message.contains(&CHUNK_SIZE_HARD_LIMIT.to_string()));
        assert!(
            warnings.is_empty(),
            "hard cap rejection should not also emit a warning",
        );
    }

    #[test]
    fn boundary_chunk_size_no_warning() {
        // Exactly at the soft threshold (not over it) produces
        // no warning. The threshold check is `>` not `>=`.
        let op_count = CHUNK_SIZE_SOFT_WARN_THRESHOLD;
        let ops: alloc::vec::Vec<crate::bytecode::Op> =
            (0..op_count).map(|_| crate::bytecode::Op::Return).collect();
        let chunk = make_chunk_with_ops("boundary", ops);
        let mut warnings: alloc::vec::Vec<CompileWarning> = alloc::vec::Vec::new();
        check_chunk_size_against_limits(&chunk, crate::token::Span::default(), &mut warnings)
            .unwrap();
        assert!(warnings.is_empty());
    }
}
