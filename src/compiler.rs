extern crate alloc;
use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use crate::ast::*;
use crate::bytecode::*;
use crate::layout_pass::LayoutContext;
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
    /// Struct name to ordered (field name, declared type) list, in
    /// declaration order. The flat-byte struct layout (B28 P2) packs and
    /// reads fields in this canonical order, which is needed because the
    /// `structs` map above is keyed by a `BTreeMap` (alphabetical) and the
    /// type checker admits struct literals with fields in any order.
    struct_field_order: BTreeMap<String, Vec<(String, TypeExpr)>>,
    /// Enum name to (variant name to payload field types).
    enums: BTreeMap<String, BTreeMap<String, Vec<TypeExpr>>>,
    /// Enum name to ordered (variant name, discriminant) list.
    /// Used by the enum-to-Word cast and any other site that
    /// needs to walk the variants in declaration order.
    enum_variant_order: BTreeMap<String, Vec<(String, i64)>>,
    /// Struct and enum definitions, keyed by name, for building a
    /// [`crate::layout_pass::LayoutContext`]. The flat layout arithmetic
    /// (B28 P2) is computed once in `LayoutContext`/`LayoutDescriptor` and
    /// consulted here rather than reimplemented, so the compiler's access
    /// baking and the runtime construction agree by sharing the predicate.
    struct_defs: BTreeMap<String, StructDef>,
    /// Enum definitions, keyed by name. See [`TypeInfo::struct_defs`].
    enum_defs: BTreeMap<String, EnumDef>,
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
    /// Module-declared word width in bytes, taken from the compile
    /// target (B28 P2). The flat-tuple layout computes baked field
    /// offsets at this width, the same one written into the module
    /// header, so the runtime reads at the offsets the compiler baked.
    /// `Default` yields `0`; the real value is set in
    /// `compile_with_options` where the target is in scope.
    word_bytes: usize,
    /// Module-declared float width in bytes, the float companion of
    /// [`TypeInfo::word_bytes`].
    float_bytes: usize,
}

impl TypeInfo {
    /// Build a [`LayoutContext`] over this module's struct and enum
    /// definitions at the module widths (B28 P2). The flat layout
    /// arithmetic the access baking needs is computed through this context
    /// and [`crate::value_layout::LayoutDescriptor`] rather than
    /// reimplemented, so the compiler and the runtime construction agree by
    /// sharing one predicate.
    fn layout_context(&self) -> LayoutContext<'_> {
        // Opaque fallback is enabled because the compiler runs after the
        // type checker: a bare `Named` type that is not a struct, enum, or
        // newtype is an opaque host reference, which is flat-eligible as a
        // `word_bytes` registry index (B28 P3).
        LayoutContext::new(
            &self.struct_defs,
            &self.enum_defs,
            self.word_bytes,
            self.float_bytes,
        )
        .with_opaque_fallback(&self.newtype_names)
    }
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
    /// Authoritative resolved types for this function's expressions, keyed by
    /// source span, recorded by the post-monomorphization type-check pass
    /// (B28 P3 item 5). `infer_expr_type` consults this first and falls back to
    /// its structural inference, so a present entry is always correct and an
    /// absent one is handled exactly as before. Empty for a function the
    /// recording pass did not table (for example an overloaded head whose name
    /// another head's table overwrote), which is safe because the lookup then
    /// misses and the structural path runs.
    expr_types: BTreeMap<crate::token::Span, TypeExpr>,
    /// Private composite data slots that store their body in the arena
    /// persistent region, mapping the unified slot index to its fixed body
    /// offset within the persistent composite pool (B28 P3 item 5, item 3a). A
    /// write to a slot in this map compiles to [`Op::SetDataComposite`] with the
    /// baked offset; a slot absent from it uses [`Op::SetData`] as before. This
    /// is the same map across all functions in the module (the persistent
    /// layout is module-wide).
    persistent_composite_offsets: BTreeMap<u16, u16>,
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
    /// When true, the compiler records strippable debug metadata (B29)
    /// while emitting this chunk and assembles it into the chunk's
    /// `debug_pool` in [`FuncCompiler::finish`]. Off by default.
    emit_debug: bool,
    /// Strippable debug annotations (B29) gathered while `emit_debug`
    /// is on, drained into the chunk's `debug_pool` at finish time.
    pending_debug: Vec<PendingDebug>,
    /// Set when this chunk emits a flat `NewComposite` that could intern a
    /// host opaque into the ephemeral registry: a flat struct or enum whose
    /// layout has an `Opaque` leaf, or a value-driven tuple or array with an
    /// element that is opaque-typed or whose type the compiler cannot
    /// recover (an unsignatured native result could be opaque at runtime).
    /// Drives the opaque-registry arena bound: a module that never sets this
    /// reserves no registry (`aux_arena_bytes = 0`); one that does falls back
    /// to the sound heap-derived bound (B28 P3 item 5 registry tightening).
    may_intern_opaque: bool,
}

/// A debug annotation captured during codegen, pending assembly into a
/// `debug_meta::DebugRecord` once the chunk is finished. Kept as an
/// intermediate so string interning and span-pool construction happen
/// once, in `build_debug_pool`.
enum PendingDebug {
    /// A call instruction at `op_index` originating from the call
    /// expression covering `span`.
    CallSite {
        op_index: usize,
        span: crate::token::Span,
    },
    /// The first op of a statement covering `span`.
    SourceSpan {
        op_index: usize,
        span: crate::token::Span,
    },
    /// The source line of the statement beginning at `op_index`.
    LineNumber { op_index: usize, line: u32 },
    /// A statement-boundary position at which a breakpoint may be set,
    /// at `op_index` covering `span`. A debugger reads these to present
    /// breakpoint choices; arming one inserts `op_index` into the VM's
    /// breakpoint position list (the runtime mechanism is future work).
    BreakpointCandidate {
        op_index: usize,
        span: crate::token::Span,
    },
    /// A local slot declared with the given human-readable name, in
    /// scope from `op_index`.
    VariableName {
        op_index: usize,
        slot: u16,
        name: String,
    },
    /// A named compiler optimisation applied to the region beginning at
    /// `op_index` (for example refinement-elision, where a refinement
    /// check was proven at compile time and no runtime check emitted).
    Optimisation { op_index: usize, name: String },
    /// A debug `assert` whose trap is at `op_index`, covering `span`
    /// with an optional failure `message`.
    Assert {
        op_index: usize,
        span: crate::token::Span,
        message: Option<String>,
    },
    /// This chunk was generated by monomorphizing `origin` at the given
    /// canonical `type_args`. Keyed at the chunk entry (op 0).
    GenericInstantiation { origin: String, type_args: String },
    /// Information-flow labels applied by a classify or declassify
    /// operation at `op_index`, for the IFC audit trail.
    IfcLabel {
        op_index: usize,
        labels: Vec<String>,
    },
    /// The declared or inferred type of local `slot`, in scope from
    /// `op_index`, rendered as a string-form `TypeRepr`.
    TypeAnnotation {
        op_index: usize,
        slot: u16,
        type_repr: String,
    },
}

/// Append a span-pool entry and return its index.
fn push_debug_span(
    pool: &mut crate::debug_meta::DebugPool,
    file_idx: u16,
    span: &crate::token::Span,
) -> u16 {
    let idx = pool.span_pool.len() as u16;
    let length = span.end.saturating_sub(span.start) as u32;
    pool.span_pool.push((file_idx, span.start as u32, length));
    idx
}

/// Intern a string into the pool's string sub-pool, returning its
/// index. Deduplicates so repeated names share one entry.
fn intern_debug_string(pool: &mut crate::debug_meta::DebugPool, s: &str) -> u16 {
    if let Some(i) = pool.string_pool.iter().position(|x| x == s) {
        return i as u16;
    }
    let i = pool.string_pool.len() as u16;
    pool.string_pool.push(String::from(s));
    i
}

/// Intern a type-representation blob into the pool's type sub-pool,
/// returning its index. Deduplicates so repeated types share one entry.
fn intern_debug_type(pool: &mut crate::debug_meta::DebugPool, bytes: &[u8]) -> u16 {
    if let Some(i) = pool.type_pool.iter().position(|x| x.as_slice() == bytes) {
        return i as u16;
    }
    let i = pool.type_pool.len() as u16;
    pool.type_pool.push(bytes.to_vec());
    i
}

/// Render a `TypeExpr` to a compact human-readable string. This is the
/// version-1 `TypeRepr` carried in a `TypeAnnotation` record's type
/// sub-pool entry: a debugger can display it directly. A structured
/// binary encoding can replace it later without changing the record
/// shape, since the entry is an opaque blob.
fn render_type_expr(ty: &TypeExpr) -> String {
    use crate::ast::PrimType;
    match ty {
        TypeExpr::Prim(p, _) => match p {
            PrimType::Byte => String::from("Byte"),
            PrimType::Word => String::from("Word"),
            PrimType::Float => String::from("Float"),
            PrimType::Bool => String::from("bool"),
            PrimType::Text => String::from("Text"),
            PrimType::Fixed(None) => String::from("Fixed"),
            PrimType::Fixed(Some(n)) => alloc::format!("Fixed<{}>", n),
        },
        TypeExpr::Multiword(n, f, _) => {
            if f.as_lit() == Some(0) {
                alloc::format!("Multiword<{}>", n)
            } else {
                alloc::format!("Multiword<{}, {}>", n, f)
            }
        }
        TypeExpr::Named(name, args, _, _) => {
            if args.is_empty() {
                name.clone()
            } else {
                let inner: Vec<String> = args.iter().map(render_type_expr).collect();
                alloc::format!("{}<{}>", name, inner.join(", "))
            }
        }
        TypeExpr::Tuple(elems, _) => {
            let inner: Vec<String> = elems.iter().map(render_type_expr).collect();
            alloc::format!("({})", inner.join(", "))
        }
        TypeExpr::Array(inner, n, _) => alloc::format!("[{}; {}]", render_type_expr(inner), n),
        TypeExpr::Option(inner, _) => alloc::format!("Option<{}>", render_type_expr(inner)),
        TypeExpr::Unit(_) => String::from("()"),
        TypeExpr::Labelled(inner, labels, _) => {
            alloc::format!("{}@{{{}}}", render_type_expr(inner), labels.join(", "))
        }
        TypeExpr::NegativeLabelled(inner, labels, _) => {
            let neg: Vec<String> = labels.iter().map(|l| alloc::format!("!{}", l)).collect();
            alloc::format!("{}@{{{}}}", render_type_expr(inner), neg.join(", "))
        }
    }
}

/// Assemble the captured annotations into a `DebugPool`. String index 0
/// is a placeholder source-file name (the compiler does not know the
/// path); spans reference it. A host may rewrite index 0 downstream.
fn build_debug_pool(pending: &[PendingDebug]) -> crate::debug_meta::DebugPool {
    use crate::debug_meta::{DebugPool, DebugRecord, DebugRecordKind};
    let mut pool = DebugPool::default();
    pool.string_pool.push(String::new());
    let file_idx: u16 = 0;
    for item in pending {
        match item {
            PendingDebug::CallSite { op_index, span } => {
                let span_idx = push_debug_span(&mut pool, file_idx, span);
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::CallSite,
                    operands: alloc::vec![span_idx],
                });
            }
            PendingDebug::SourceSpan { op_index, span } => {
                let span_idx = push_debug_span(&mut pool, file_idx, span);
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::SourceSpan,
                    operands: alloc::vec![span_idx],
                });
            }
            PendingDebug::BreakpointCandidate { op_index, span } => {
                let span_idx = push_debug_span(&mut pool, file_idx, span);
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::BreakpointCandidate,
                    operands: alloc::vec![span_idx],
                });
            }
            PendingDebug::LineNumber { op_index, line } => {
                let line_u16 = (*line).min(u32::from(u16::MAX)) as u16;
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::LineNumber,
                    operands: alloc::vec![line_u16],
                });
            }
            PendingDebug::VariableName {
                op_index,
                slot,
                name,
            } => {
                let name_idx = intern_debug_string(&mut pool, name);
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::VariableName,
                    operands: alloc::vec![*slot, name_idx],
                });
            }
            PendingDebug::Optimisation { op_index, name } => {
                let name_idx = intern_debug_string(&mut pool, name);
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::OptimisationMarker,
                    operands: alloc::vec![name_idx],
                });
            }
            PendingDebug::Assert {
                op_index,
                span,
                message,
            } => {
                let span_idx = push_debug_span(&mut pool, file_idx, span);
                let mut operands = alloc::vec![span_idx];
                if let Some(message) = message {
                    operands.push(intern_debug_string(&mut pool, message));
                }
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::AssertionContext,
                    operands,
                });
            }
            PendingDebug::GenericInstantiation { origin, type_args } => {
                let origin_idx = intern_debug_string(&mut pool, origin);
                let type_args_idx = intern_debug_string(&mut pool, type_args);
                pool.records.push(DebugRecord {
                    op_index: 0,
                    kind: DebugRecordKind::GenericInstantiation,
                    operands: alloc::vec![origin_idx, type_args_idx],
                });
            }
            PendingDebug::IfcLabel { op_index, labels } => {
                let mut operands = alloc::vec::Vec::with_capacity(labels.len());
                for label in labels {
                    operands.push(intern_debug_string(&mut pool, label));
                }
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::IfcLabelAnnotation,
                    operands,
                });
            }
            PendingDebug::TypeAnnotation {
                op_index,
                slot,
                type_repr,
            } => {
                let type_idx = intern_debug_type(&mut pool, type_repr.as_bytes());
                pool.records.push(DebugRecord {
                    op_index: *op_index as u32,
                    kind: DebugRecordKind::TypeAnnotation,
                    operands: alloc::vec![*slot, type_idx],
                });
            }
        }
    }
    pool
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
        expr_types: BTreeMap<crate::token::Span, TypeExpr>,
        persistent_composite_offsets: BTreeMap<u16, u16>,
        emit_debug: bool,
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
                debug_pool: None,
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
            expr_types,
            persistent_composite_offsets,
            local_const_values: BTreeMap::new(),
            local_ranges: BTreeMap::new(),
            emit_debug,
            pending_debug: Vec::new(),
            may_intern_opaque: false,
        }
    }

    /// Record a `CallSite` debug annotation for the call instruction
    /// most recently emitted, when debug emission is on. Records only
    /// when the chunk's last op is an actual call instruction, so it is
    /// safe to invoke after compiling any call-shaped expression: a
    /// newtype construction or refinement-elided call that emitted no
    /// call op is correctly skipped.
    fn record_call_site_last_op(&mut self, span: &crate::token::Span) {
        if !self.emit_debug {
            return;
        }
        if let Some(idx) = self.chunk.ops.len().checked_sub(1)
            && matches!(
                self.chunk.ops[idx],
                Op::Call(..) | Op::CallVerifiedNative(..) | Op::CallExternalNative(..)
            )
        {
            self.pending_debug.push(PendingDebug::CallSite {
                op_index: idx,
                span: *span,
            });
        }
    }

    /// Record a `SourceSpan` and a `BreakpointCandidate` at the last
    /// emitted op, when debug emission is on, for a trap-bearing
    /// operator (B29 items 2 and 3). The `SourceSpan` lets a runtime
    /// fault at that op resolve exactly to the operator's sub-expression
    /// span rather than to the enclosing statement; the candidate is an
    /// operator-level breakpoint position. Intended to be called
    /// immediately after emitting a partial operation such as `Op::Div`
    /// or `Op::Mod`, so the last op is the operator.
    fn record_operator_site(&mut self, span: &crate::token::Span) {
        if !self.emit_debug {
            return;
        }
        if let Some(idx) = self.chunk.ops.len().checked_sub(1) {
            self.pending_debug.push(PendingDebug::SourceSpan {
                op_index: idx,
                span: *span,
            });
            self.pending_debug.push(PendingDebug::BreakpointCandidate {
                op_index: idx,
                span: *span,
            });
        }
    }

    /// Record a `SourceSpan` and a `LineNumber` annotation for a
    /// statement whose code begins at `op_index`, when debug emission
    /// is on. No-op when no ops were emitted for the statement.
    fn record_statement(&mut self, op_index: usize, span: &crate::token::Span) {
        if !self.emit_debug || op_index >= self.chunk.ops.len() {
            return;
        }
        self.pending_debug.push(PendingDebug::SourceSpan {
            op_index,
            span: *span,
        });
        self.pending_debug.push(PendingDebug::LineNumber {
            op_index,
            line: span.line,
        });
        // Each statement boundary is a candidate breakpoint position.
        self.pending_debug.push(PendingDebug::BreakpointCandidate {
            op_index,
            span: *span,
        });
    }

    /// Record a function-entry `BreakpointCandidate` at op 0 (B29), when
    /// debug emission is on and the chunk has at least one op. This is
    /// the function-entry breakpoint granularity, distinct from the
    /// per-statement candidates `record_statement` emits.
    fn record_function_entry(&mut self, span: &crate::token::Span) {
        if self.emit_debug && !self.chunk.ops.is_empty() {
            self.pending_debug.push(PendingDebug::BreakpointCandidate {
                op_index: 0,
                span: *span,
            });
        }
    }

    /// Record an `AssertionContext` for a debug assert whose trap is at
    /// `op_index`, when debug emission is on.
    fn record_assert(&mut self, op_index: usize, span: &crate::token::Span, message: Option<&str>) {
        if self.emit_debug {
            self.pending_debug.push(PendingDebug::Assert {
                op_index,
                span: *span,
                message: message.map(String::from),
            });
        }
    }

    /// Record an `OptimisationMarker` naming a compiler optimisation
    /// applied at `op_index`, when debug emission is on.
    fn record_optimisation(&mut self, op_index: usize, name: &str) {
        if self.emit_debug {
            self.pending_debug.push(PendingDebug::Optimisation {
                op_index,
                name: String::from(name),
            });
        }
    }

    /// Record a `GenericInstantiation` for this chunk, naming the
    /// generic `origin` and its canonical `type_args`, when debug
    /// emission is on.
    fn record_generic_instantiation(&mut self, origin: &str, type_args: &str) {
        if self.emit_debug {
            self.pending_debug.push(PendingDebug::GenericInstantiation {
                origin: String::from(origin),
                type_args: String::from(type_args),
            });
        }
    }

    /// Record an `IfcLabelAnnotation` for a classify or declassify
    /// operation at `op_index`, when debug emission is on. A no-op for
    /// an empty label set.
    fn record_ifc_labels(&mut self, op_index: usize, labels: &[String]) {
        if self.emit_debug && !labels.is_empty() {
            self.pending_debug.push(PendingDebug::IfcLabel {
                op_index,
                labels: labels.to_vec(),
            });
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
                if let TypeExpr::Named(struct_name, _, _, _) = ty {
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
                if let TypeExpr::Named(struct_name, _, _, _) = field_ty {
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
    fn add_const_value(&mut self, mut cv: ConstValue) -> u16 {
        // Fill in enum discriminants from the type tables so a constant
        // enum materialises into the flat body that matches the baked
        // access (B28 P2). This is the single point every constant value
        // passes through with the type tables available.
        resolve_const_enum_discriminants(&mut cv, &self.type_info.enum_variant_order);
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
        // Render the local's type before `ty` is moved into `Local`,
        // for a TypeAnnotation record under debug emission.
        let type_repr = if self.emit_debug {
            ty.as_ref().map(render_type_expr)
        } else {
            None
        };
        self.locals.push(Local {
            name: String::from(name),
            slot,
            depth: self.scope_depth,
            ty,
        });
        if self.emit_debug {
            let op_index = self.chunk.ops.len();
            self.pending_debug.push(PendingDebug::VariableName {
                op_index,
                slot,
                name: String::from(name),
            });
            if let Some(type_repr) = type_repr {
                self.pending_debug.push(PendingDebug::TypeAnnotation {
                    op_index,
                    slot,
                    type_repr,
                });
            }
        }
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
        if self.emit_debug && !self.pending_debug.is_empty() {
            self.chunk.debug_pool = Some(build_debug_pool(&self.pending_debug));
        }
        self.chunk
    }
}

/// Extract the length of an array type expression. Returns `Some(N)`
/// for `[T; N]` and `None` for other shapes.
fn array_length_of_type(t: &TypeExpr) -> Option<i64> {
    match t {
        TypeExpr::Array(_, n, _) => n.as_lit(),
        _ => None,
    }
}

/// Extract the element type of an array type expression. Returns
/// `Some(T)` for `[T; N]` and `None` for other shapes.
fn element_type_of(t: &TypeExpr) -> Option<TypeExpr> {
    match t {
        TypeExpr::Array(elem, _, _) => Some((**elem).clone()),
        // A Multiword<N> indexes to a Word digit; its representation is
        // a flat N-word array (B19).
        TypeExpr::Multiword(_, _, _) => Some(TypeExpr::Prim(PrimType::Word, Span::default())),
        _ => None,
    }
}

/// Extract a compile-time-constant, non-negative shift amount bounded by
/// `max_bits`. Used by the overflow-checked arithmetic left shift, which
/// lowers `x asl k` to a checked multiply by the constant `2^k`; that
/// multiplier cannot be formed for a runtime amount, so the checked form
/// requires a literal. The bare shift operators admit a variable amount
/// through `classify_shift_amount` instead.
fn const_shift_amount(right: &Expr, max_bits: i64) -> Result<i64, CompileError> {
    match right {
        Expr::Literal {
            value: crate::ast::Literal::Int(k),
            span,
        } => {
            if *k < 0 || *k >= max_bits {
                Err(CompileError {
                    message: alloc::format!(
                        "shift amount {} is out of range [0, {}) for the value width",
                        k,
                        max_bits
                    ),
                    span: *span,
                })
            } else {
                Ok(*k)
            }
        }
        _ => Err(CompileError {
            message: String::from(
                "the overflow-checked arithmetic left shift requires a compile-time-constant amount; a variable amount is admissible only for the bare shift operators",
            ),
            span: right.span(),
        }),
    }
}

/// Classify a shift's right operand as either a compile-time-constant
/// amount in `[0, max_bits)` or a runtime-variable amount. A literal
/// outside the range is a compile error; any non-literal is a variable
/// amount, which is admissible because the VM masks the runtime count to
/// the word width (`Op::Shl`/`Op::Shr` take `count & (word_bits - 1)`).
fn classify_shift_amount(right: &Expr, max_bits: i64) -> Result<Option<i64>, CompileError> {
    match right {
        Expr::Literal {
            value: crate::ast::Literal::Int(k),
            span,
        } => {
            if *k < 0 || *k >= max_bits {
                Err(CompileError {
                    message: alloc::format!(
                        "shift amount {} is out of range [0, {}) for the value width",
                        k,
                        max_bits
                    ),
                    span: *span,
                })
            } else {
                Ok(Some(*k))
            }
        }
        _ => Ok(None),
    }
}

/// Compile a scalar `Word` or `Byte` shift. The amount is either a
/// compile-time-constant literal or a runtime-variable `Word` (B19). A
/// `Byte` value is promoted to `Word` with `Op::ByteToWord`, shifted at
/// the word width, and truncated back with `Op::WordToByte`, which also
/// performs the left-shift masking; a `Byte` is unsigned, so its
/// arithmetic and logical right shifts coincide. The left shift and the
/// arithmetic right shift map to `Op::Shl` and the arithmetic `Op::Shr`;
/// the logical right shift clears the sign-extended high bits.
fn compile_scalar_shift(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    left: &Expr,
    right: &Expr,
) -> Result<(), CompileError> {
    let word_bits = (fc.type_info.word_bytes * 8) as i64;
    let is_byte = matches!(
        infer_expr_type(fc, left),
        Some(TypeExpr::Prim(PrimType::Byte, _))
    );
    // The constant-amount range is the value's own bit width: eight bits
    // for a `Byte`, the word width for a `Word`. A variable amount is
    // masked to the word width by the VM at runtime.
    let value_bits = if is_byte { 8 } else { word_bits };
    let amount = classify_shift_amount(right, value_bits)?;
    compile_expr(fc, left)?;
    if is_byte {
        fc.emit(Op::ByteToWord);
    }
    match amount {
        Some(k) => compile_scalar_shift_const(fc, op, k, word_bits),
        None => compile_scalar_shift_variable(fc, op, right, word_bits)?,
    }
    if is_byte {
        fc.emit(Op::WordToByte);
    }
    Ok(())
}

/// Emit a constant-amount scalar shift by `k` over the `Word`-typed value
/// already on the operand stack. At `k = 0` the logical-right mask is all
/// ones and the shift is the identity.
fn compile_scalar_shift_const(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    k: i64,
    word_bits: i64,
) {
    use crate::ast::BinOp;
    let k_c = fc.add_constant(Value::Int(k));
    fc.emit(Op::Const(k_c));
    match op {
        // The bare arithmetic left shift wraps, producing the same value
        // as the logical left shift; overflow capture is handled by the
        // checked-arithmetic construct, not here.
        BinOp::Shl | BinOp::AShl => {
            fc.emit(Op::Shl);
        }
        BinOp::ShrA => {
            fc.emit(Op::Shr);
        }
        BinOp::ShrL => {
            fc.emit(Op::Shr);
            let mask = ((1i128 << (word_bits - k)) - 1) as i64;
            let mask_c = fc.add_constant(Value::Int(mask));
            fc.emit(Op::Const(mask_c));
            fc.emit(Op::BitAnd);
        }
        _ => unreachable!("compile_scalar_shift_const called with a non-shift operator"),
    }
}

/// Emit a variable-amount scalar shift over the `Word`-typed value
/// already on the operand stack, consuming the runtime count expression
/// `right`. The left shift and arithmetic right shift lower to `Op::Shl`
/// and `Op::Shr`, whose VM dispatch masks the count to the word width.
/// The logical right shift is the arithmetic shift with the
/// sign-extended high bits masked away; because the mask
/// `(1 << (word_bits - c)) - 1` is all ones at `c = 0` (where the VM's
/// count masking would otherwise collapse `1 << word_bits` to `1`), the
/// `c = 0` case is handled by an explicit identity branch.
fn compile_scalar_shift_variable(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    right: &Expr,
    word_bits: i64,
) -> Result<(), CompileError> {
    use crate::ast::BinOp;
    match op {
        BinOp::Shl | BinOp::AShl => {
            compile_expr(fc, right)?;
            fc.emit(Op::Shl);
        }
        BinOp::ShrA => {
            compile_expr(fc, right)?;
            fc.emit(Op::Shr);
        }
        BinOp::ShrL => {
            let lv = fc.declare_local("__shr_v");
            fc.emit(Op::SetLocal(lv));
            compile_expr(fc, right)?;
            let lc = fc.declare_local("__shr_c");
            fc.emit(Op::SetLocal(lc));
            let zero = fc.add_constant(Value::Int(0));
            let one = fc.add_constant(Value::Int(1));
            let wb_c = fc.add_constant(Value::Int(word_bits));
            // if c == 0 { v } else { (v asr c) band ((1 lsl (word_bits - c)) - 1) }
            fc.emit(Op::GetLocal(lc));
            fc.emit(Op::Const(zero));
            fc.emit(Op::CmpEq);
            let if_addr = fc.emit_jump(Op::If(0));
            fc.emit(Op::GetLocal(lv));
            let else_addr = fc.emit_jump(Op::Else(0));
            fc.patch_jump(if_addr);
            fc.emit(Op::GetLocal(lv));
            fc.emit(Op::GetLocal(lc));
            fc.emit(Op::Shr); // v asr c
            fc.emit(Op::Const(one));
            fc.emit(Op::Const(wb_c));
            fc.emit(Op::GetLocal(lc));
            // Word subtraction routes through the checked opcode; the low
            // word of the (high, low, flag) triple is the wrapping result
            // and the high/flag are discarded (Consolidation B removed the
            // unchecked `Op::Sub` Int arm).
            fc.emit(Op::CheckedSub);
            fc.emit(Op::PopN(2)); // word_bits - c
            fc.emit(Op::Shl); // 1 lsl (word_bits - c)
            fc.emit(Op::Const(one));
            fc.emit(Op::CheckedSub);
            fc.emit(Op::PopN(2)); // mask = (1 lsl (word_bits - c)) - 1
            fc.emit(Op::BitAnd);
            fc.patch_jump(else_addr);
            fc.emit(Op::EndIf);
        }
        _ => unreachable!("compile_scalar_shift_variable called with a non-shift operator"),
    }
    Ok(())
}

/// Compile a scalar `Byte` bitwise operation (`band`/`bor`/`bxor`). Each
/// operand is promoted to `Word` with `Op::ByteToWord`, combined with the
/// word-width bitwise opcode, and truncated back to `Byte` with
/// `Op::WordToByte`. The word-width high bits produced by the operation
/// are cleared by the truncation, so the result is the byte-width
/// combination.
fn compile_byte_bitwise(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    left: &Expr,
    right: &Expr,
) -> Result<(), CompileError> {
    use crate::ast::BinOp;
    compile_expr(fc, left)?;
    fc.emit(Op::ByteToWord);
    compile_expr(fc, right)?;
    fc.emit(Op::ByteToWord);
    match op {
        BinOp::Band => {
            fc.emit(Op::BitAnd);
        }
        BinOp::Bor => {
            fc.emit(Op::BitOr);
        }
        BinOp::Bxor => {
            fc.emit(Op::BitXor);
        }
        _ => unreachable!("compile_byte_bitwise called with a non-bitwise operator"),
    }
    fc.emit(Op::WordToByte);
    Ok(())
}

/// Compile a `Multiword<N, F>` shift with a compile-time-constant amount
/// (B19 phase 5). The shift splits into a word offset q and a bit offset
/// r. A left shift fills zero at the bottom and truncates (wraps) at the
/// top; a right shift fills the vacated top with the sign word for the
/// arithmetic `>>` and with zero for the logical `>>>`. Because `Op::Shr`
/// is arithmetic, the intra-word logical part is an arithmetic shift
/// masked to the low `word_bits - r` bits.
fn compile_multiword_shift(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    left: &Expr,
    right: &Expr,
    n: u16,
) -> Result<(), CompileError> {
    use crate::ast::BinOp;
    let word_bits = (fc.type_info.word_bytes * 8) as i64;
    let total_bits = n as i64 * word_bits;
    // A runtime-variable amount takes the unrolled-with-runtime-index path.
    let k = match classify_shift_amount(right, total_bits)? {
        Some(k) => k,
        None => return compile_multiword_variable_shift(fc, op, left, right, n, word_bits),
    };
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let zero = fc.add_constant(Value::Int(0));
    let shift_c = fc.add_constant(Value::Int(word_bits - 1));
    let av = fc.declare_local("__mw_a");
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    let mut wv: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        let wi = fc.declare_local("__mw_w");
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(wi));
        wv.push(wi);
    }
    let q = (k / word_bits) as usize;
    let r = k % word_bits;
    let nn = n as usize;
    let mut rwords: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    if matches!(op, BinOp::Shl | BinOp::AShl) {
        // result[j] = (value[j-q] << r) | top_r_bits(value[j-q-1])
        let (r_c, wr_c, carry_mask) = if r != 0 {
            let cm = ((1i128 << r) - 1) as i64;
            (
                Some(fc.add_constant(Value::Int(r))),
                Some(fc.add_constant(Value::Int(word_bits - r))),
                Some(fc.add_constant(Value::Int(cm))),
            )
        } else {
            (None, None, None)
        };
        for j in 0..nn {
            let rk = fc.declare_local("__mw_r");
            if j >= q {
                fc.emit(Op::GetLocal(wv[j - q]));
                if r != 0 {
                    fc.emit(Op::Const(r_c.unwrap()));
                    fc.emit(Op::Shl);
                }
            } else {
                fc.emit(Op::Const(zero));
            }
            if r != 0 && j > q {
                fc.emit(Op::GetLocal(wv[j - q - 1]));
                fc.emit(Op::Const(wr_c.unwrap()));
                fc.emit(Op::Shr);
                fc.emit(Op::Const(carry_mask.unwrap()));
                fc.emit(Op::BitAnd);
                fc.emit(Op::BitOr);
            }
            fc.emit(Op::SetLocal(rk));
            rwords.push(rk);
        }
    } else {
        // Right shift. The fill for the vacated top is the sign word for
        // the arithmetic shift and zero for the logical shift.
        let fill = fc.declare_local("__mw_fill");
        if matches!(op, BinOp::ShrA) {
            fc.emit(Op::GetLocal(wv[nn - 1]));
            fc.emit(Op::Const(shift_c));
            fc.emit(Op::Shr);
            fc.emit(Op::SetLocal(fill));
        } else {
            fc.emit(Op::Const(zero));
            fc.emit(Op::SetLocal(fill));
        }
        let vword = |m: usize| -> u16 { if m < nn { wv[m] } else { fill } };
        let (r_c, wr_c, logmask) = if r != 0 {
            let lm = ((1i128 << (word_bits - r)) - 1) as i64;
            (
                Some(fc.add_constant(Value::Int(r))),
                Some(fc.add_constant(Value::Int(word_bits - r))),
                Some(fc.add_constant(Value::Int(lm))),
            )
        } else {
            (None, None, None)
        };
        for j in 0..nn {
            let rk = fc.declare_local("__mw_r");
            if r == 0 {
                fc.emit(Op::GetLocal(vword(j + q)));
            } else {
                fc.emit(Op::GetLocal(vword(j + q)));
                fc.emit(Op::Const(r_c.unwrap()));
                fc.emit(Op::Shr);
                fc.emit(Op::Const(logmask.unwrap()));
                fc.emit(Op::BitAnd);
                fc.emit(Op::GetLocal(vword(j + q + 1)));
                fc.emit(Op::Const(wr_c.unwrap()));
                fc.emit(Op::Shl);
                fc.emit(Op::BitOr);
            }
            fc.emit(Op::SetLocal(rk));
            rwords.push(rk);
        }
    }
    for &rk in &rwords {
        fc.emit(Op::GetLocal(rk));
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    Ok(())
}

/// Emit `iv := base + delta * q` for a runtime word offset `q`
/// (`delta` is `+1` to add the offset for a right shift, `-1` to subtract
/// it for a left shift). Word arithmetic routes through the checked
/// opcodes; the low word of the triple is the wrapping result.
fn emit_mw_index(fc: &mut FuncCompiler, base: i64, qv: u16, add: bool, iv: u16) {
    let base_c = fc.add_constant(Value::Int(base));
    fc.emit(Op::Const(base_c));
    fc.emit(Op::GetLocal(qv));
    if add {
        fc.emit(Op::CheckedAdd);
    } else {
        fc.emit(Op::CheckedSub);
    }
    fc.emit(Op::PopN(2));
    fc.emit(Op::SetLocal(iv));
}

/// Push the `iv`-indexed limb of the source array `av`, guarded so an
/// index outside `[0, n)` yields `fillv` rather than trapping. The guard
/// is branch-free: `in_mask` is all ones exactly when `0 <= iv < n`, the
/// index is clamped to zero when out of range so `GetIndex` never traps,
/// and the fetched word is blended with the fill by the mask.
#[allow(clippy::too_many_arguments)]
fn emit_mw_guarded(
    fc: &mut FuncCompiler,
    av: u16,
    iv: u16,
    fillv: u16,
    imv: u16,
    siv: u16,
    elem_op: crate::bytecode::ArrayElem,
    neg1: u16,
    wbm1_c: u16,
    n_c: u16,
) {
    // ge0 = bnot(iv asr (word_bits - 1)) : all ones when iv >= 0.
    fc.emit(Op::GetLocal(iv));
    fc.emit(Op::Const(wbm1_c));
    fc.emit(Op::Shr);
    fc.emit(Op::Const(neg1));
    fc.emit(Op::BitXor);
    // ltn = (iv - n) asr (word_bits - 1) : all ones when iv < n.
    fc.emit(Op::GetLocal(iv));
    fc.emit(Op::Const(n_c));
    fc.emit(Op::CheckedSub);
    fc.emit(Op::PopN(2));
    fc.emit(Op::Const(wbm1_c));
    fc.emit(Op::Shr);
    fc.emit(Op::BitAnd); // in_mask = ge0 band ltn
    fc.emit(Op::SetLocal(imv));
    // safe_idx = iv band in_mask (0 when out of range, so no trap).
    fc.emit(Op::GetLocal(iv));
    fc.emit(Op::GetLocal(imv));
    fc.emit(Op::BitAnd);
    fc.emit(Op::SetLocal(siv));
    // (a[safe_idx] band in_mask) bor (fill band bnot(in_mask)).
    fc.emit(Op::GetLocal(av));
    fc.emit(Op::GetLocal(siv));
    fc.emit(Op::GetIndex(elem_op));
    fc.emit(Op::GetLocal(imv));
    fc.emit(Op::BitAnd);
    fc.emit(Op::GetLocal(fillv));
    fc.emit(Op::GetLocal(imv));
    fc.emit(Op::Const(neg1));
    fc.emit(Op::BitXor);
    fc.emit(Op::BitAnd);
    fc.emit(Op::BitOr);
}

/// Compile a `Multiword<N, F>` shift by a runtime-variable amount. The
/// value stays a flat N-word array; the word offset `q = c >> log2(wb)`
/// and bit offset `r = c & (wb - 1)` are computed at runtime and each of
/// the N result limbs is built from runtime-indexed, bounds-guarded
/// source limbs. The construction is unrolled over N (a compile-time
/// constant), so there is no runtime loop and the worst-case bounds stay
/// exactly as the verifier already accounts opcodes. An out-of-range or
/// over-large count shifts every bit out through the index guards (zero
/// for a left or logical shift, the sign word for an arithmetic right
/// shift), matching the constant lowering. `asl` equals `lsl` here, since
/// a multi-word value has no overflow-capture construct.
fn compile_multiword_variable_shift(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    left: &Expr,
    right: &Expr,
    n: u16,
    word_bits: i64,
) -> Result<(), CompileError> {
    use crate::ast::BinOp;
    let nn = n as usize;
    let log2_wb = word_bits.trailing_zeros() as i64;
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let is_left = matches!(op, BinOp::Shl | BinOp::AShl);

    let zero = fc.add_constant(Value::Int(0));
    let one = fc.add_constant(Value::Int(1));
    let neg1 = fc.add_constant(Value::Int(-1));
    let wbm1_c = fc.add_constant(Value::Int(word_bits - 1));
    let log2_c = fc.add_constant(Value::Int(log2_wb));
    let wb_c = fc.add_constant(Value::Int(word_bits));
    let n_c = fc.add_constant(Value::Int(n as i64));

    let av = fc.declare_local("__mwv_a");
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    let cv = fc.declare_local("__mwv_c");
    compile_expr(fc, right)?;
    fc.emit(Op::SetLocal(cv));
    // q = c >> log2(wb) (arithmetic; a non-negative count divides cleanly).
    let qv = fc.declare_local("__mwv_q");
    fc.emit(Op::GetLocal(cv));
    fc.emit(Op::Const(log2_c));
    fc.emit(Op::Shr);
    fc.emit(Op::SetLocal(qv));
    // r = c band (wb - 1).
    let rv = fc.declare_local("__mwv_r");
    fc.emit(Op::GetLocal(cv));
    fc.emit(Op::Const(wbm1_c));
    fc.emit(Op::BitAnd);
    fc.emit(Op::SetLocal(rv));
    // The vacated fill: the sign word for an arithmetic right shift, zero
    // otherwise.
    let fillv = fc.declare_local("__mwv_fill");
    if matches!(op, BinOp::ShrA) {
        let top_c = fc.add_constant(Value::Int((nn - 1) as i64));
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(top_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::Const(wbm1_c));
        fc.emit(Op::Shr);
        fc.emit(Op::SetLocal(fillv));
    } else {
        fc.emit(Op::Const(zero));
        fc.emit(Op::SetLocal(fillv));
    }
    let iv = fc.declare_local("__mwv_i");
    let imv = fc.declare_local("__mwv_im");
    let siv = fc.declare_local("__mwv_si");

    // if r == 0 { pure word shift } else { word + bit combine }
    fc.emit(Op::GetLocal(rv));
    fc.emit(Op::Const(zero));
    fc.emit(Op::CmpEq);
    let if_addr = fc.emit_jump(Op::If(0));
    // r == 0: each result limb is the guarded source limb at the shifted
    // word position.
    for j in 0..nn {
        emit_mw_index(fc, j as i64, qv, !is_left, iv);
        emit_mw_guarded(fc, av, iv, fillv, imv, siv, elem_op, neg1, wbm1_c, n_c);
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    let else_addr = fc.emit_jump(Op::Else(0));
    fc.patch_jump(if_addr);
    // r != 0: word + bit combine. wbr = wb - r.
    let wbrv = fc.declare_local("__mwv_wbr");
    fc.emit(Op::Const(wb_c));
    fc.emit(Op::GetLocal(rv));
    fc.emit(Op::CheckedSub);
    fc.emit(Op::PopN(2));
    fc.emit(Op::SetLocal(wbrv));
    if is_left {
        // result[j] = (guarded(j-q) lsl r) bor ((guarded(j-q-1) asr wbr) band ((1 lsl r) - 1))
        let maskr = fc.declare_local("__mwv_mr");
        fc.emit(Op::Const(one));
        fc.emit(Op::GetLocal(rv));
        fc.emit(Op::Shl);
        fc.emit(Op::Const(one));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(maskr));
        for j in 0..nn {
            emit_mw_index(fc, j as i64, qv, false, iv);
            emit_mw_guarded(fc, av, iv, fillv, imv, siv, elem_op, neg1, wbm1_c, n_c);
            fc.emit(Op::GetLocal(rv));
            fc.emit(Op::Shl);
            emit_mw_index(fc, j as i64 - 1, qv, false, iv);
            emit_mw_guarded(fc, av, iv, fillv, imv, siv, elem_op, neg1, wbm1_c, n_c);
            fc.emit(Op::GetLocal(wbrv));
            fc.emit(Op::Shr);
            fc.emit(Op::GetLocal(maskr));
            fc.emit(Op::BitAnd);
            fc.emit(Op::BitOr);
        }
    } else {
        // result[j] = ((guarded(j+q) asr r) band ((1 lsl wbr) - 1)) bor (guarded(j+q+1) lsl wbr)
        let lomask = fc.declare_local("__mwv_lm");
        fc.emit(Op::Const(one));
        fc.emit(Op::GetLocal(wbrv));
        fc.emit(Op::Shl);
        fc.emit(Op::Const(one));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(lomask));
        for j in 0..nn {
            emit_mw_index(fc, j as i64, qv, true, iv);
            emit_mw_guarded(fc, av, iv, fillv, imv, siv, elem_op, neg1, wbm1_c, n_c);
            fc.emit(Op::GetLocal(rv));
            fc.emit(Op::Shr);
            fc.emit(Op::GetLocal(lomask));
            fc.emit(Op::BitAnd);
            emit_mw_index(fc, j as i64 + 1, qv, true, iv);
            emit_mw_guarded(fc, av, iv, fillv, imv, siv, elem_op, neg1, wbm1_c, n_c);
            fc.emit(Op::GetLocal(wbrv));
            fc.emit(Op::Shl);
            fc.emit(Op::BitOr);
        }
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    fc.patch_jump(else_addr);
    fc.emit(Op::EndIf);
    Ok(())
}

/// Compile a per-word `Multiword<N>` bitwise operation (`band`, `bor`,
/// `bxor`). Each result limb is the scalar bitwise combination of the
/// two operand limbs; there is no cross-limb interaction, so the
/// lowering is a flat unrolled loop over the existing bitwise opcodes.
fn compile_multiword_bitwise(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    left: &Expr,
    right: &Expr,
    n: u16,
) -> Result<(), CompileError> {
    use crate::ast::BinOp;
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let bit_op = match op {
        BinOp::Band => Op::BitAnd,
        BinOp::Bor => Op::BitOr,
        BinOp::Bxor => Op::BitXor,
        _ => unreachable!("compile_multiword_bitwise called with a non-bitwise operator"),
    };
    let av = fc.declare_local("__mw_a");
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    let bv = fc.declare_local("__mw_b");
    compile_expr(fc, right)?;
    fc.emit(Op::SetLocal(bv));
    let mut rwords: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        let rk = fc.declare_local("__mw_r");
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::GetLocal(bv));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(bit_op);
        fc.emit(Op::SetLocal(rk));
        rwords.push(rk);
    }
    for &rk in &rwords {
        fc.emit(Op::GetLocal(rk));
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    Ok(())
}

/// Compile a `Multiword<N>` bitwise complement (`bnot`) as a per-word
/// exclusive-or against the all-ones word. Each limb is extracted, XORed
/// with `-1` (all ones under two's-complement), and stored back; the
/// upper bits beyond the module word width are discarded on store, so
/// the complement of the retained low bits is correct at narrow widths.
fn compile_multiword_bnot(
    fc: &mut FuncCompiler,
    operand: &Expr,
    n: u16,
) -> Result<(), CompileError> {
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let neg1 = fc.add_constant(Value::Int(-1));
    let av = fc.declare_local("__mw_a");
    compile_expr(fc, operand)?;
    fc.emit(Op::SetLocal(av));
    let mut rwords: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        let rk = fc.declare_local("__mw_r");
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::Const(neg1));
        fc.emit(Op::BitXor);
        fc.emit(Op::SetLocal(rk));
        rwords.push(rk);
    }
    for &rk in &rwords {
        fc.emit(Op::GetLocal(rk));
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    Ok(())
}

/// Compile a binary operator whose operands are `Multiword<N>` (B19).
/// Add, subtract, comparisons, multiply, divide, modulo, and shifts all
/// lower to unrolled cascades over the existing scalar opcodes.
fn compile_multiword_binop(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    left: &Expr,
    right: &Expr,
    n: u16,
    f: u16,
) -> Result<(), CompileError> {
    use crate::ast::BinOp;
    match op {
        BinOp::Add => compile_multiword_add_sub(fc, left, right, n, false),
        BinOp::Sub => compile_multiword_add_sub(fc, left, right, n, true),
        // Integer multiply (F = 0) truncates to N words; the fixed-point
        // multiply (F > 0) forms the full product and shifts it right by F.
        BinOp::Mul if f == 0 => compile_multiword_mul(fc, left, right, n),
        BinOp::Mul => compile_multiword_fixed_mul(fc, left, right, n, f),
        // Divide and modulo at every scale. The fixed-point divide pre-
        // shifts the dividend by F; the modulo needs no shift.
        BinOp::Div => compile_multiword_div(fc, left, right, n, f, false),
        BinOp::Mod => compile_multiword_div(fc, left, right, n, f, true),
        BinOp::Shl | BinOp::AShl | BinOp::ShrA | BinOp::ShrL => {
            compile_multiword_shift(fc, op, left, right, n)
        }
        BinOp::Band | BinOp::Bor | BinOp::Bxor => compile_multiword_bitwise(fc, op, left, right, n),
        BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Gt | BinOp::LtEq | BinOp::GtEq => {
            compile_multiword_compare(fc, op, left, right, n)
        }
        _ => Err(CompileError {
            message: alloc::format!(
                "Multiword<{}> does not yet support this operator (later phase)",
                n
            ),
            span: left.span(),
        }),
    }
}

/// Emit one limb's unsigned carry (add) or borrow (sub) as a 0/1 word
/// left on the operand stack. Given operand limbs `x` and `y` and the
/// wrapping result limb `s`, all in locals:
///   add carry  = top_bit((x & y) | ((x ^ y) & ~s))
///   sub borrow = top_bit((~x & y) | (~(x ^ y) & s))
/// The top bit is extracted by an arithmetic right shift of word_bits-1
/// followed by a mask of 1. This is the correct two's-complement
/// multi-word carry, not the signed-overflow flag of the checked op.
#[allow(clippy::too_many_arguments)]
fn emit_limb_carry(
    fc: &mut FuncCompiler,
    is_sub: bool,
    x: u16,
    y: u16,
    s: u16,
    neg1: u16,
    shift_c: u16,
    one: u16,
) {
    if is_sub {
        fc.emit(Op::GetLocal(x));
        fc.emit(Op::Const(neg1));
        fc.emit(Op::BitXor); // ~x
        fc.emit(Op::GetLocal(y));
        fc.emit(Op::BitAnd); // ~x & y
        fc.emit(Op::GetLocal(x));
        fc.emit(Op::GetLocal(y));
        fc.emit(Op::BitXor);
        fc.emit(Op::Const(neg1));
        fc.emit(Op::BitXor); // ~(x ^ y)
        fc.emit(Op::GetLocal(s));
        fc.emit(Op::BitAnd); // ~(x ^ y) & s
        fc.emit(Op::BitOr);
    } else {
        fc.emit(Op::GetLocal(x));
        fc.emit(Op::GetLocal(y));
        fc.emit(Op::BitAnd); // x & y
        fc.emit(Op::GetLocal(x));
        fc.emit(Op::GetLocal(y));
        fc.emit(Op::BitXor); // x ^ y
        fc.emit(Op::GetLocal(s));
        fc.emit(Op::Const(neg1));
        fc.emit(Op::BitXor); // ~s
        fc.emit(Op::BitAnd); // (x ^ y) & ~s
        fc.emit(Op::BitOr);
    }
    fc.emit(Op::Const(shift_c));
    fc.emit(Op::Shr);
    fc.emit(Op::Const(one));
    fc.emit(Op::BitAnd);
}

/// Compile `Multiword<N>` addition or subtraction as an unrolled
/// per-limb carry or borrow cascade over the existing checked-word and
/// bitwise opcodes, producing a fresh N-word array (B19).
fn compile_multiword_add_sub(
    fc: &mut FuncCompiler,
    left: &Expr,
    right: &Expr,
    n: u16,
    is_sub: bool,
) -> Result<(), CompileError> {
    let word_bits = (fc.type_info.word_bytes * 8) as i64;
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let neg1 = fc.add_constant(Value::Int(-1));
    let one = fc.add_constant(Value::Int(1));
    let zero = fc.add_constant(Value::Int(0));
    let shift_c = fc.add_constant(Value::Int(word_bits - 1));
    let av = fc.declare_local("__mw_a");
    let bv = fc.declare_local("__mw_b");
    let cv = fc.declare_local("__mw_carry");
    let xi = fc.declare_local("__mw_x");
    let yi = fc.declare_local("__mw_y");
    let s1 = fc.declare_local("__mw_s1");
    let c1 = fc.declare_local("__mw_c1");
    let s2 = fc.declare_local("__mw_s2");
    let c2 = fc.declare_local("__mw_c2");
    // Evaluate both operands into locals.
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    compile_expr(fc, right)?;
    fc.emit(Op::SetLocal(bv));
    // The running carry or borrow starts at zero.
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(cv));
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        // x = a[i]
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(xi));
        // y = b[i]
        fc.emit(Op::GetLocal(bv));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(yi));
        // s1 = x (op) y, wrapping; discard the high word and flag.
        fc.emit(Op::GetLocal(xi));
        fc.emit(Op::GetLocal(yi));
        if is_sub {
            fc.emit(Op::CheckedSub);
        } else {
            fc.emit(Op::CheckedAdd);
        }
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(s1));
        // c1 = carry or borrow out of that step.
        emit_limb_carry(fc, is_sub, xi, yi, s1, neg1, shift_c, one);
        fc.emit(Op::SetLocal(c1));
        // s2 = s1 (op) incoming carry, wrapping.
        fc.emit(Op::GetLocal(s1));
        fc.emit(Op::GetLocal(cv));
        if is_sub {
            fc.emit(Op::CheckedSub);
        } else {
            fc.emit(Op::CheckedAdd);
        }
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(s2));
        // c2 = carry or borrow out of folding the incoming carry.
        emit_limb_carry(fc, is_sub, s1, cv, s2, neg1, shift_c, one);
        fc.emit(Op::SetLocal(c2));
        // Outgoing carry is c1 | c2; at most one is set.
        fc.emit(Op::GetLocal(c1));
        fc.emit(Op::GetLocal(c2));
        fc.emit(Op::BitOr);
        fc.emit(Op::SetLocal(cv));
        // Push the result limb for the array.
        fc.emit(Op::GetLocal(s2));
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    Ok(())
}

/// Compile a `Multiword<N>` comparison (`==`, `!=`, `<`, `>`, `<=`, `>=`)
/// as a branch-free limb-wise fold that yields a `Bool` (B19). The value
/// is little-endian two's complement, so the ordering is decided by the
/// most significant differing limb, the top limb read signed and the
/// lower limbs unsigned.
///
/// The whole fold stays in the `Word` domain because the bitwise opcodes
/// require `Word` operands and the comparison opcodes yield `Bool`. Two
/// running accumulators, `lt` and `gt`, hold whether the value seen so
/// far compares less or greater. Folding from the least significant limb
/// upward, each limb that differs overrides the accumulators, so the most
/// significant differing limb wins:
///   lt := li | (lt & eq_i)   gt := gi | (gt & eq_i)
/// where `li` is `a[i] <u b[i]` and `gi` is `b[i] <u a[i]`. An unsigned
/// limb less-than is exactly the borrow out of the limb subtraction, so
/// [`emit_limb_carry`] computes both. The most significant limb is read
/// signed by XOR-ing both limbs with the sign bit before the same
/// unsigned comparison, which maps the signed order onto the unsigned
/// order. The final `Bool` is produced by comparing the accumulators to
/// zero, so no comparison opcode operates on a `Multiword` directly.
fn compile_multiword_compare(
    fc: &mut FuncCompiler,
    op: crate::ast::BinOp,
    left: &Expr,
    right: &Expr,
    n: u16,
) -> Result<(), CompileError> {
    use crate::ast::BinOp;
    let word_bits = (fc.type_info.word_bytes * 8) as i64;
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let neg1 = fc.add_constant(Value::Int(-1));
    let one = fc.add_constant(Value::Int(1));
    let zero = fc.add_constant(Value::Int(0));
    let shift_c = fc.add_constant(Value::Int(word_bits - 1));
    // The sign bit of a single word; XOR-ing both operands with it maps
    // the signed ordering of the top limb onto the unsigned ordering the
    // borrow computes.
    let sign = fc.add_constant(Value::Int(1i64 << (word_bits - 1)));
    let av = fc.declare_local("__mw_a");
    let bv = fc.declare_local("__mw_b");
    let xi = fc.declare_local("__mw_x");
    let yi = fc.declare_local("__mw_y");
    let s = fc.declare_local("__mw_s");
    let li = fc.declare_local("__mw_lt_i");
    let gi = fc.declare_local("__mw_gt_i");
    let eqi = fc.declare_local("__mw_eq_i");
    let lt = fc.declare_local("__mw_lt");
    let gt = fc.declare_local("__mw_gt");
    // Evaluate both operands into locals.
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    compile_expr(fc, right)?;
    fc.emit(Op::SetLocal(bv));
    // Accumulators start at "equal so far".
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(lt));
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(gt));
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        let is_top = i == n - 1;
        // x = a[i], sign-flipped for the signed top limb.
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        if is_top {
            fc.emit(Op::Const(sign));
            fc.emit(Op::BitXor);
        }
        fc.emit(Op::SetLocal(xi));
        // y = b[i], sign-flipped for the signed top limb.
        fc.emit(Op::GetLocal(bv));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        if is_top {
            fc.emit(Op::Const(sign));
            fc.emit(Op::BitXor);
        }
        fc.emit(Op::SetLocal(yi));
        // li = (x <u y) = borrow out of x - y.
        fc.emit(Op::GetLocal(xi));
        fc.emit(Op::GetLocal(yi));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(s));
        emit_limb_carry(fc, true, xi, yi, s, neg1, shift_c, one);
        fc.emit(Op::SetLocal(li));
        // gi = (y <u x) = borrow out of y - x.
        fc.emit(Op::GetLocal(yi));
        fc.emit(Op::GetLocal(xi));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(s));
        emit_limb_carry(fc, true, yi, xi, s, neg1, shift_c, one);
        fc.emit(Op::SetLocal(gi));
        // eq_i = 1 - (li | gi); exactly one of li, gi, eq_i is set.
        fc.emit(Op::GetLocal(li));
        fc.emit(Op::GetLocal(gi));
        fc.emit(Op::BitOr);
        fc.emit(Op::Const(one));
        fc.emit(Op::BitXor);
        fc.emit(Op::SetLocal(eqi));
        // lt = li | (lt & eq_i)
        fc.emit(Op::GetLocal(lt));
        fc.emit(Op::GetLocal(eqi));
        fc.emit(Op::BitAnd);
        fc.emit(Op::GetLocal(li));
        fc.emit(Op::BitOr);
        fc.emit(Op::SetLocal(lt));
        // gt = gi | (gt & eq_i)
        fc.emit(Op::GetLocal(gt));
        fc.emit(Op::GetLocal(eqi));
        fc.emit(Op::BitAnd);
        fc.emit(Op::GetLocal(gi));
        fc.emit(Op::BitOr);
        fc.emit(Op::SetLocal(gt));
    }
    // Reduce the accumulators to the requested Bool. `<` and `>` are the
    // accumulators themselves; `<=` and `>=` are the negations of the
    // opposite strict order; `==` and `!=` test that neither strict order
    // holds.
    match op {
        BinOp::Lt => {
            fc.emit(Op::GetLocal(lt));
            fc.emit(Op::Const(zero));
            fc.emit(Op::CmpNe);
        }
        BinOp::Gt => {
            fc.emit(Op::GetLocal(gt));
            fc.emit(Op::Const(zero));
            fc.emit(Op::CmpNe);
        }
        BinOp::LtEq => {
            fc.emit(Op::GetLocal(gt));
            fc.emit(Op::Const(zero));
            fc.emit(Op::CmpEq);
        }
        BinOp::GtEq => {
            fc.emit(Op::GetLocal(lt));
            fc.emit(Op::Const(zero));
            fc.emit(Op::CmpEq);
        }
        BinOp::Eq => {
            fc.emit(Op::GetLocal(lt));
            fc.emit(Op::GetLocal(gt));
            fc.emit(Op::BitOr);
            fc.emit(Op::Const(zero));
            fc.emit(Op::CmpEq);
        }
        BinOp::NotEq => {
            fc.emit(Op::GetLocal(lt));
            fc.emit(Op::GetLocal(gt));
            fc.emit(Op::BitOr);
            fc.emit(Op::Const(zero));
            fc.emit(Op::CmpNe);
        }
        _ => unreachable!("compile_multiword_compare called with a non-comparison operator"),
    }
    Ok(())
}

/// Compile an integer `Multiword<N>` multiply (fraction-bit count F = 0)
/// as an unrolled schoolbook product truncated to N words (B19 phase 3a).
///
/// The result is `(a * b) mod 2^(N*word_bits)`, the low N words of the
/// full product. Two's-complement multiplication truncated to the low N
/// words equals the unsigned multiplication of the same bit patterns
/// truncated to N words, so the digits are treated as unsigned magnitudes
/// throughout and the top word's sign takes care of itself.
///
/// Each word-by-word partial product needs the unsigned double-word
/// result. `Op::CheckedMul` computes the signed widening product and
/// returns the signed high word, so the high word is corrected to the
/// unsigned high word by the identity
///   unsigned_high = signed_high + (x < 0 ? y : 0) + (y < 0 ? x : 0)
/// evaluated mod 2^word_bits, where the conditional add is done
/// branch-free with the arithmetic-shift sign mask `w >> (word_bits - 1)`.
/// The low word is the same for the signed and unsigned interpretations.
///
/// Partial products are summed by column (the Comba scheme): for output
/// word k, every low word at digit position k and every corrected high
/// word at position k - 1 is added into a two-word accumulator, the low
/// word of which becomes result word k before the accumulator is shifted
/// down one word for the next column. A column sum is at most
/// `(2N + 1) * 2^word_bits`, so the two-word accumulator is exact only
/// while `2N + 1 < 2^word_bits`. For every word width of seventeen bits
/// or more this admits the full word-count range (N up to 65535), so the
/// bound is a real constraint only on eight- and sixteen-bit words, and
/// there it excludes only word counts so large that the N-squared
/// unrolling would be impractical regardless. A word count that would
/// overflow the accumulator is rejected here rather than lowered to a
/// silently wrong product.
fn compile_multiword_mul(
    fc: &mut FuncCompiler,
    left: &Expr,
    right: &Expr,
    n: u16,
) -> Result<(), CompileError> {
    let word_bits = (fc.type_info.word_bytes * 8) as i64;
    // Guard the two-word column accumulator against overflow. Computed in
    // u128 so the shift is exact for every admitted word width up to 64.
    let column_modulus = 1u128 << (word_bits as u32);
    if 2u128 * n as u128 + 1 >= column_modulus {
        return Err(CompileError {
            message: alloc::format!(
                "Multiword<{}> multiply exceeds the multi-word accumulator capacity at a {}-bit word; reduce the word count",
                n,
                word_bits
            ),
            span: left.span(),
        });
    }
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let neg1 = fc.add_constant(Value::Int(-1));
    let one = fc.add_constant(Value::Int(1));
    let zero = fc.add_constant(Value::Int(0));
    let shift_c = fc.add_constant(Value::Int(word_bits - 1));
    let av = fc.declare_local("__mw_a");
    let bv = fc.declare_local("__mw_b");
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    compile_expr(fc, right)?;
    fc.emit(Op::SetLocal(bv));
    // Load every operand word into its own local.
    let mut adig: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    let mut bdig: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        let ai = fc.declare_local("__mw_ai");
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(ai));
        adig.push(ai);
        let bj = fc.declare_local("__mw_bj");
        fc.emit(Op::GetLocal(bv));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(bj));
        bdig.push(bj);
    }
    let hs = fc.declare_local("__mw_hs");
    // Unsigned digit products (low, corrected high) for every pair whose
    // low word lands within the truncated result, that is i + j <= N - 1.
    // Stored as (i, j, lo_local, hi_local).
    let mut prods: alloc::vec::Vec<(u16, u16, u16, u16)> = alloc::vec::Vec::new();
    for i in 0..n {
        for j in 0..n {
            if i as u32 + j as u32 > (n - 1) as u32 {
                continue;
            }
            let lo = fc.declare_local("__mw_plo");
            let hi = fc.declare_local("__mw_phi");
            // (low, signed_high, flag) = a[i] * b[j]; drop the flag.
            fc.emit(Op::GetLocal(adig[i as usize]));
            fc.emit(Op::GetLocal(bdig[j as usize]));
            fc.emit(Op::CheckedMul(0));
            fc.emit(Op::PopN(1));
            fc.emit(Op::SetLocal(hs));
            fc.emit(Op::SetLocal(lo));
            // uhi = signed_high + (a[i] & signmask(b[j])) + (b[j] & signmask(a[i]))
            fc.emit(Op::GetLocal(hs));
            fc.emit(Op::GetLocal(adig[i as usize]));
            fc.emit(Op::GetLocal(bdig[j as usize]));
            fc.emit(Op::Const(shift_c));
            fc.emit(Op::Shr);
            fc.emit(Op::BitAnd);
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::GetLocal(bdig[j as usize]));
            fc.emit(Op::GetLocal(adig[i as usize]));
            fc.emit(Op::Const(shift_c));
            fc.emit(Op::Shr);
            fc.emit(Op::BitAnd);
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(hi));
            prods.push((i, j, lo, hi));
        }
    }
    // Column accumulation with a two-word accumulator.
    let acc_lo = fc.declare_local("__mw_acc_lo");
    let acc_hi = fc.declare_local("__mw_acc_hi");
    let tmp_s = fc.declare_local("__mw_tmp_s");
    let carry = fc.declare_local("__mw_carry");
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(acc_lo));
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(acc_hi));
    let mut rwords: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for k in 0..n {
        // The terms for column k are every low word at digit position k
        // and every corrected high word at position k - 1.
        let mut terms: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
        for &(i, j, lo, hi) in &prods {
            if i as u32 + j as u32 == k as u32 {
                terms.push(lo);
            }
            if k > 0 && i as u32 + j as u32 == (k - 1) as u32 {
                terms.push(hi);
            }
        }
        for v in terms {
            // tmp_s = acc_lo + v, wrapping.
            fc.emit(Op::GetLocal(acc_lo));
            fc.emit(Op::GetLocal(v));
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(tmp_s));
            // carry = unsigned carry out of that add (acc_lo still old).
            emit_limb_carry(fc, false, acc_lo, v, tmp_s, neg1, shift_c, one);
            fc.emit(Op::SetLocal(carry));
            fc.emit(Op::GetLocal(tmp_s));
            fc.emit(Op::SetLocal(acc_lo));
            // acc_hi += carry, wrapping (acc_hi never overflows for
            // admitted N, so the carry out here is discarded).
            fc.emit(Op::GetLocal(acc_hi));
            fc.emit(Op::GetLocal(carry));
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(acc_hi));
        }
        // Result word k is the accumulator low word.
        let rk = fc.declare_local("__mw_r");
        fc.emit(Op::GetLocal(acc_lo));
        fc.emit(Op::SetLocal(rk));
        rwords.push(rk);
        // Shift the accumulator down one word for the next column.
        fc.emit(Op::GetLocal(acc_hi));
        fc.emit(Op::SetLocal(acc_lo));
        fc.emit(Op::Const(zero));
        fc.emit(Op::SetLocal(acc_hi));
    }
    for &rk in &rwords {
        fc.emit(Op::GetLocal(rk));
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    Ok(())
}

/// Subtract the N-word vector `dig[i] & mask` from the high N words
/// `pwords[N + i]` in place, propagating an unsigned borrow through the
/// high half (B19 phase 3b). When `mask` is all zeros the subtraction is
/// a no-op, so a caller gates a conditional correction by passing the
/// arithmetic-shift sign mask of an operand. A borrow out of the top
/// word is dropped, which is correct because the product-level
/// correction only ever cancels the spurious high bits of the unsigned
/// product.
#[allow(clippy::too_many_arguments)]
fn emit_multiword_highword_subtract(
    fc: &mut FuncCompiler,
    pwords: &[u16],
    dig: &[u16],
    mask: u16,
    n: u16,
    neg1: u16,
    shift_c: u16,
    one: u16,
) {
    let zero = fc.add_constant(Value::Int(0));
    let borrow = fc.declare_local("__mw_hb");
    let term = fc.declare_local("__mw_ht");
    let s1 = fc.declare_local("__mw_hs1");
    let s2 = fc.declare_local("__mw_hs2");
    let bo1 = fc.declare_local("__mw_hbo1");
    let bo2 = fc.declare_local("__mw_hbo2");
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(borrow));
    for i in 0..n as usize {
        let hi_word = pwords[n as usize + i];
        // term = dig[i] & mask
        fc.emit(Op::GetLocal(dig[i]));
        fc.emit(Op::GetLocal(mask));
        fc.emit(Op::BitAnd);
        fc.emit(Op::SetLocal(term));
        // s1 = hi_word - term, wrapping; bo1 = borrow out.
        fc.emit(Op::GetLocal(hi_word));
        fc.emit(Op::GetLocal(term));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(s1));
        emit_limb_carry(fc, true, hi_word, term, s1, neg1, shift_c, one);
        fc.emit(Op::SetLocal(bo1));
        // s2 = s1 - borrow, wrapping; bo2 = borrow out.
        fc.emit(Op::GetLocal(s1));
        fc.emit(Op::GetLocal(borrow));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(s2));
        emit_limb_carry(fc, true, s1, borrow, s2, neg1, shift_c, one);
        fc.emit(Op::SetLocal(bo2));
        // hi_word = s2; borrow = bo1 | bo2 (at most one is set).
        fc.emit(Op::GetLocal(s2));
        fc.emit(Op::SetLocal(hi_word));
        fc.emit(Op::GetLocal(bo1));
        fc.emit(Op::GetLocal(bo2));
        fc.emit(Op::BitOr);
        fc.emit(Op::SetLocal(borrow));
    }
}

/// Compile a fixed-point `Multiword<N, F>` multiply (F > 0) as the full
/// 2N-word product shifted right by F, truncated to N words (B19 phase
/// 3b). Two same-scale operands `a` and `b` represent `a / 2^F` and
/// `b / 2^F`, so their product represents `a * b / 2^(2F)`, and the raw
/// result that represents `(a / 2^F) * (b / 2^F)` is `(a * b) >> F`.
///
/// The full product is formed in three steps. First the unsigned 2N-word
/// product of the two operands' bit patterns is accumulated by column
/// (the same Comba scheme and per-digit unsigned-high correction as the
/// integer multiply, but over all 2N columns rather than the low N).
/// Second the unsigned product is corrected to the signed product by
/// `S = U - 2^(N*word_bits) * (a * [b < 0] + b * [a < 0])`, realised as
/// two conditional in-place subtractions from the high N words gated by
/// each operand's sign mask. Third `S` is shifted right by F with an
/// arithmetic (sign-extending) shift and the low N words are taken.
///
/// The shift splits F into a word offset `q = F / word_bits` and a bit
/// offset `r = F % word_bits`. For `r == 0` each result word is a shifted
/// word of `S`; otherwise result word k is
/// `logical_shr(S[k+q], r) | (S[k+q+1] << (word_bits - r))`, the logical
/// right shift synthesised as an arithmetic shift masked to the low
/// `word_bits - r` bits since `Op::Shr` is arithmetic. Words at or beyond
/// index 2N read the arithmetic sign-extension of the top product word.
fn compile_multiword_fixed_mul(
    fc: &mut FuncCompiler,
    left: &Expr,
    right: &Expr,
    n: u16,
    f: u16,
) -> Result<(), CompileError> {
    let word_bits = (fc.type_info.word_bytes * 8) as i64;
    // Fraction-bit bound: F must fit within the N-word value.
    let total_bits = n as u128 * word_bits as u128;
    if f as u128 > total_bits {
        return Err(CompileError {
            message: alloc::format!(
                "Multiword<{}, {}> declares more fraction bits than the {}-bit N-word width holds",
                n,
                f,
                total_bits
            ),
            span: left.span(),
        });
    }
    // Guard the two-word column accumulator (see the integer multiply).
    let column_modulus = 1u128 << (word_bits as u32);
    if 2u128 * n as u128 + 1 >= column_modulus {
        return Err(CompileError {
            message: alloc::format!(
                "Multiword<{}> multiply exceeds the multi-word accumulator capacity at a {}-bit word; reduce the word count",
                n,
                word_bits
            ),
            span: left.span(),
        });
    }
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let neg1 = fc.add_constant(Value::Int(-1));
    let one = fc.add_constant(Value::Int(1));
    let zero = fc.add_constant(Value::Int(0));
    let shift_c = fc.add_constant(Value::Int(word_bits - 1));
    let av = fc.declare_local("__mw_a");
    let bv = fc.declare_local("__mw_b");
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    compile_expr(fc, right)?;
    fc.emit(Op::SetLocal(bv));
    // Load every operand word into its own local.
    let mut adig: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    let mut bdig: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        let ai = fc.declare_local("__mw_ai");
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(ai));
        adig.push(ai);
        let bj = fc.declare_local("__mw_bj");
        fc.emit(Op::GetLocal(bv));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(bj));
        bdig.push(bj);
    }
    let hs = fc.declare_local("__mw_hs");
    // Unsigned digit products (low, corrected high) for every pair; the
    // full product keeps all N*N of them.
    let mut prods: alloc::vec::Vec<(u16, u16, u16, u16)> = alloc::vec::Vec::new();
    for i in 0..n {
        for j in 0..n {
            let lo = fc.declare_local("__mw_plo");
            let hi = fc.declare_local("__mw_phi");
            fc.emit(Op::GetLocal(adig[i as usize]));
            fc.emit(Op::GetLocal(bdig[j as usize]));
            fc.emit(Op::CheckedMul(0));
            fc.emit(Op::PopN(1));
            fc.emit(Op::SetLocal(hs));
            fc.emit(Op::SetLocal(lo));
            // uhi = signed_high + (a[i] & signmask(b[j])) + (b[j] & signmask(a[i]))
            fc.emit(Op::GetLocal(hs));
            fc.emit(Op::GetLocal(adig[i as usize]));
            fc.emit(Op::GetLocal(bdig[j as usize]));
            fc.emit(Op::Const(shift_c));
            fc.emit(Op::Shr);
            fc.emit(Op::BitAnd);
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::GetLocal(bdig[j as usize]));
            fc.emit(Op::GetLocal(adig[i as usize]));
            fc.emit(Op::Const(shift_c));
            fc.emit(Op::Shr);
            fc.emit(Op::BitAnd);
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(hi));
            prods.push((i, j, lo, hi));
        }
    }
    // Accumulate the full 2N-word unsigned product by column.
    let two_n = 2 * n as usize;
    let acc_lo = fc.declare_local("__mw_acc_lo");
    let acc_hi = fc.declare_local("__mw_acc_hi");
    let tmp_s = fc.declare_local("__mw_tmp_s");
    let carry = fc.declare_local("__mw_carry");
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(acc_lo));
    fc.emit(Op::Const(zero));
    fc.emit(Op::SetLocal(acc_hi));
    let mut pwords: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for k in 0..two_n {
        let mut terms: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
        for &(i, j, lo, hi) in &prods {
            if i as usize + j as usize == k {
                terms.push(lo);
            }
            if k > 0 && i as usize + j as usize == k - 1 {
                terms.push(hi);
            }
        }
        for v in terms {
            fc.emit(Op::GetLocal(acc_lo));
            fc.emit(Op::GetLocal(v));
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(tmp_s));
            emit_limb_carry(fc, false, acc_lo, v, tmp_s, neg1, shift_c, one);
            fc.emit(Op::SetLocal(carry));
            fc.emit(Op::GetLocal(tmp_s));
            fc.emit(Op::SetLocal(acc_lo));
            fc.emit(Op::GetLocal(acc_hi));
            fc.emit(Op::GetLocal(carry));
            fc.emit(Op::CheckedAdd);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(acc_hi));
        }
        let pk = fc.declare_local("__mw_p");
        fc.emit(Op::GetLocal(acc_lo));
        fc.emit(Op::SetLocal(pk));
        pwords.push(pk);
        fc.emit(Op::GetLocal(acc_hi));
        fc.emit(Op::SetLocal(acc_lo));
        fc.emit(Op::Const(zero));
        fc.emit(Op::SetLocal(acc_hi));
    }
    // Correct the unsigned product to the signed product: subtract a from
    // the high words when b is negative, and b when a is negative.
    let a_neg = fc.declare_local("__mw_aneg");
    let b_neg = fc.declare_local("__mw_bneg");
    fc.emit(Op::GetLocal(adig[(n - 1) as usize]));
    fc.emit(Op::Const(shift_c));
    fc.emit(Op::Shr);
    fc.emit(Op::SetLocal(a_neg));
    fc.emit(Op::GetLocal(bdig[(n - 1) as usize]));
    fc.emit(Op::Const(shift_c));
    fc.emit(Op::Shr);
    fc.emit(Op::SetLocal(b_neg));
    emit_multiword_highword_subtract(fc, &pwords, &adig, b_neg, n, neg1, shift_c, one);
    emit_multiword_highword_subtract(fc, &pwords, &bdig, a_neg, n, neg1, shift_c, one);
    // Arithmetic-shift the signed 2N-word product right by F and take the
    // low N words. Because F <= N * word_bits, the highest product bit the
    // shift reads is F + N * word_bits - 1, at most 2N * word_bits - 1, the
    // top bit of the 2N-word product; every word access therefore stays
    // within the product and no sign-extension word beyond index 2N is
    // ever needed. The shift is arithmetic (floor toward negative
    // infinity), so a negative product rounds down rather than toward
    // zero, and a result that does not fit in N words wraps, matching the
    // wrapping default of the other multi-word operations.
    let q = (f as usize) / (word_bits as usize);
    let r = (f as usize) % (word_bits as usize);
    debug_assert!(q + n as usize <= two_n, "shift window escapes the product");
    let mut rwords: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    let (r_c, wr_c, mask_c) = if r != 0 {
        let mask = ((1i128 << (word_bits as i128 - r as i128)) - 1) as i64;
        (
            Some(fc.add_constant(Value::Int(r as i64))),
            Some(fc.add_constant(Value::Int(word_bits - r as i64))),
            Some(fc.add_constant(Value::Int(mask))),
        )
    } else {
        (None, None, None)
    };
    for k in 0..n as usize {
        let rk = fc.declare_local("__mw_r");
        if r == 0 {
            fc.emit(Op::GetLocal(pwords[k + q]));
        } else {
            // logical_shr(S[k+q], r) = (S[k+q] >>a r) & mask
            fc.emit(Op::GetLocal(pwords[k + q]));
            fc.emit(Op::Const(r_c.unwrap()));
            fc.emit(Op::Shr);
            fc.emit(Op::Const(mask_c.unwrap()));
            fc.emit(Op::BitAnd);
            // | (S[k+q+1] << (word_bits - r))
            fc.emit(Op::GetLocal(pwords[k + q + 1]));
            fc.emit(Op::Const(wr_c.unwrap()));
            fc.emit(Op::Shl);
            fc.emit(Op::BitOr);
        }
        fc.emit(Op::SetLocal(rk));
        rwords.push(rk);
    }
    for &rk in &rwords {
        fc.emit(Op::GetLocal(rk));
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    Ok(())
}

/// Conditionally two's-complement negate the N-word value in `words` in
/// place (B19 phase 4). The `mask` local is all ones to negate and zero
/// to leave the value unchanged, so a caller passes a sign mask to make a
/// negate-if-negative. Negation is `~x + 1`, so the words are XORed with
/// the mask and then `mask & 1` is added through with carry.
#[allow(clippy::too_many_arguments)]
fn emit_multiword_cond_negate(
    fc: &mut FuncCompiler,
    words: &[u16],
    mask: u16,
    n: u16,
    neg1: u16,
    shift_c: u16,
    one: u16,
) {
    let carry = fc.declare_local("__mw_cn_c");
    let t = fc.declare_local("__mw_cn_t");
    let s = fc.declare_local("__mw_cn_s");
    let bo = fc.declare_local("__mw_cn_bo");
    fc.emit(Op::GetLocal(mask));
    fc.emit(Op::Const(one));
    fc.emit(Op::BitAnd);
    fc.emit(Op::SetLocal(carry));
    for &w in words.iter().take(n as usize) {
        fc.emit(Op::GetLocal(w));
        fc.emit(Op::GetLocal(mask));
        fc.emit(Op::BitXor);
        fc.emit(Op::SetLocal(t));
        fc.emit(Op::GetLocal(t));
        fc.emit(Op::GetLocal(carry));
        fc.emit(Op::CheckedAdd);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(s));
        emit_limb_carry(fc, false, t, carry, s, neg1, shift_c, one);
        fc.emit(Op::SetLocal(bo));
        fc.emit(Op::GetLocal(s));
        fc.emit(Op::SetLocal(w));
        fc.emit(Op::GetLocal(bo));
        fc.emit(Op::SetLocal(carry));
    }
}

/// Compile a `Multiword<N, F>` divide (`is_mod` false) or modulo
/// (`is_mod` true). Integer divide and modulo (F = 0, B19 phase 4a) and
/// the fixed-point divide and modulo (F > 0, B19 phase 4b) are all
/// handled here.
///
/// Division is signed with truncation toward zero, matching the scalar
/// `Word` division: the quotient takes the sign of the operand-sign
/// exclusive-or and the remainder takes the sign of the dividend. The
/// operands are reduced to their magnitudes, an unsigned division runs,
/// and the sign is reapplied. A zero divisor traps as a division by
/// zero, reusing the scalar `Op::Div` trap.
///
/// The fixed-point divide pre-shifts the dividend left by F, because two
/// same-scale operands a and b represent a/2^F and b/2^F, so the raw
/// quotient that represents (a/2^F)/(b/2^F) is (a << F) / b. The shift is
/// folded into the bit loop: dividend bit i is bit i - F of the
/// magnitude, the loop runs over the widened dividend, and only the low N
/// words of the quotient are stored, the higher bits being the overflow.
/// The fixed-point modulo needs no shift, because a same-scale remainder
/// keeps the scale, so raw_a mod raw_b is already the fixed-point
/// remainder, identical to the integer modulo.
///
/// The unsigned core is branchless binary long division. The constrained
/// instruction set has no mutable array indexing, so every word and bit
/// index must be a compile-time constant, which the bit loop is unrolled
/// to provide. Each bit-step shifts the running remainder left by one and
/// injects the next dividend bit, tentatively subtracts the divisor, and
/// uses the subtraction's final borrow both as the `remainder < divisor`
/// test and as the mask that either keeps the old remainder (borrow) or
/// takes the difference (no borrow) and sets the corresponding quotient
/// bit. Because the loop is unrolled, a dividend whose bit total is
/// impractically large is rejected.
fn compile_multiword_div(
    fc: &mut FuncCompiler,
    left: &Expr,
    right: &Expr,
    n: u16,
    f: u16,
    is_mod: bool,
) -> Result<(), CompileError> {
    let word_bits = (fc.type_info.word_bytes * 8) as i64;
    let n_bits = n as u128 * word_bits as u128;
    // The fixed-point divide widens the dividend by F bits; the integer
    // divide and the modulo (at any scale) do not.
    let shift = if !is_mod && f > 0 { f as u128 } else { 0 };
    // A fraction-bit count wider than the value is malformed.
    if shift > n_bits {
        return Err(CompileError {
            message: alloc::format!(
                "Multiword<{}, {}> declares more fraction bits than the {}-bit N-word width holds",
                n,
                f,
                n_bits
            ),
            span: left.span(),
        });
    }
    // The bit loop unrolls to dividend_bits steps; bound it so a single
    // division cannot emit an unreasonable amount of code. The limit
    // admits the practical word counts and rejects only values so wide
    // the unrolling would be impractical.
    let dividend_bits = n_bits + shift;
    if dividend_bits > 512 {
        return Err(CompileError {
            message: alloc::format!(
                "Multiword<{}> division at a {}-bit word unrolls to {} bit-steps, beyond the compiler's limit; reduce the word count",
                n,
                word_bits,
                dividend_bits
            ),
            span: left.span(),
        });
    }
    let byte_size = flat_alloc_bytes(
        &TypeExpr::multiword_lit(n, 0, Span::default()),
        &fc.type_info,
    )
    .unwrap_or_else(|| conservative_alloc_bytes(n));
    let word_ty = TypeExpr::Prim(PrimType::Word, Span::default());
    let elem_op = array_elem_operand(Some(&word_ty), &fc.type_info);
    let neg1 = fc.add_constant(Value::Int(-1));
    let one = fc.add_constant(Value::Int(1));
    let zero = fc.add_constant(Value::Int(0));
    let shift_c = fc.add_constant(Value::Int(word_bits - 1));
    let av = fc.declare_local("__mw_a");
    let bv = fc.declare_local("__mw_b");
    compile_expr(fc, left)?;
    fc.emit(Op::SetLocal(av));
    compile_expr(fc, right)?;
    fc.emit(Op::SetLocal(bv));
    // Load operand words. `adig` becomes |a| and `bdig` becomes |b| below.
    let mut adig: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    let mut bdig: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for i in 0..n {
        let idx_c = fc.add_constant(Value::Int(i as i64));
        let ai = fc.declare_local("__mw_ai");
        fc.emit(Op::GetLocal(av));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(ai));
        adig.push(ai);
        let bj = fc.declare_local("__mw_bj");
        fc.emit(Op::GetLocal(bv));
        fc.emit(Op::Const(idx_c));
        fc.emit(Op::GetIndex(elem_op));
        fc.emit(Op::SetLocal(bj));
        bdig.push(bj);
    }
    // Divide-by-zero guard: 1 / (OR of divisor words) traps exactly when
    // the divisor is all zeros, reusing the scalar zero-divisor trap.
    let b_or = fc.declare_local("__mw_bor");
    fc.emit(Op::GetLocal(bdig[0]));
    for w in bdig.iter().skip(1) {
        fc.emit(Op::GetLocal(*w));
        fc.emit(Op::BitOr);
    }
    fc.emit(Op::SetLocal(b_or));
    fc.emit(Op::Const(one));
    fc.emit(Op::GetLocal(b_or));
    fc.emit(Op::Div);
    fc.emit(Op::PopN(1));
    // Sign masks (all ones when negative) and magnitudes.
    let sign_a = fc.declare_local("__mw_sga");
    let sign_b = fc.declare_local("__mw_sgb");
    fc.emit(Op::GetLocal(adig[(n - 1) as usize]));
    fc.emit(Op::Const(shift_c));
    fc.emit(Op::Shr);
    fc.emit(Op::SetLocal(sign_a));
    fc.emit(Op::GetLocal(bdig[(n - 1) as usize]));
    fc.emit(Op::Const(shift_c));
    fc.emit(Op::Shr);
    fc.emit(Op::SetLocal(sign_b));
    emit_multiword_cond_negate(fc, &adig, sign_a, n, neg1, shift_c, one);
    emit_multiword_cond_negate(fc, &bdig, sign_b, n, neg1, shift_c, one);
    // Remainder R, quotient Q, and reused scratch temporaries.
    let mut rword: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    let mut qword: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    let mut tent: alloc::vec::Vec<u16> = alloc::vec::Vec::new();
    for _ in 0..n {
        let rk = fc.declare_local("__mw_rr");
        fc.emit(Op::Const(zero));
        fc.emit(Op::SetLocal(rk));
        rword.push(rk);
        let qk = fc.declare_local("__mw_qq");
        fc.emit(Op::Const(zero));
        fc.emit(Op::SetLocal(qk));
        qword.push(qk);
        tent.push(fc.declare_local("__mw_tt"));
    }
    let s1 = fc.declare_local("__mw_ds1");
    let s2 = fc.declare_local("__mw_ds2");
    let bo1 = fc.declare_local("__mw_dbo1");
    let bo2 = fc.declare_local("__mw_dbo2");
    let borrow = fc.declare_local("__mw_dbor");
    let mask = fc.declare_local("__mw_dmask");
    let notmask = fc.declare_local("__mw_dnmask");
    // Bit loop from the most significant dividend bit down.
    for i in (0..dividend_bits).rev() {
        // Shift R left by one, injecting dividend bit i into bit 0.
        for j in (1..n as usize).rev() {
            fc.emit(Op::GetLocal(rword[j]));
            fc.emit(Op::Const(one));
            fc.emit(Op::Shl);
            fc.emit(Op::GetLocal(rword[j - 1]));
            fc.emit(Op::Const(shift_c));
            fc.emit(Op::Shr);
            fc.emit(Op::Const(one));
            fc.emit(Op::BitAnd);
            fc.emit(Op::BitOr);
            fc.emit(Op::SetLocal(rword[j]));
        }
        fc.emit(Op::GetLocal(rword[0]));
        fc.emit(Op::Const(one));
        fc.emit(Op::Shl);
        // Dividend bit i is bit (i - shift) of the magnitude; when that
        // index falls outside the magnitude the injected bit is zero, so
        // R is left as R << 1 with no bit set.
        let src = i as i128 - shift as i128;
        if src >= 0 && (src as u128) < n_bits {
            let src = src as u128;
            let swi = (src / word_bits as u128) as usize;
            let sbp = fc.add_constant(Value::Int((src % word_bits as u128) as i64));
            fc.emit(Op::GetLocal(adig[swi]));
            fc.emit(Op::Const(sbp));
            fc.emit(Op::Shr);
            fc.emit(Op::Const(one));
            fc.emit(Op::BitAnd);
            fc.emit(Op::BitOr);
        }
        fc.emit(Op::SetLocal(rword[0]));
        // tentative = R - |b|, tracking the final borrow.
        fc.emit(Op::Const(zero));
        fc.emit(Op::SetLocal(borrow));
        for j in 0..n as usize {
            fc.emit(Op::GetLocal(rword[j]));
            fc.emit(Op::GetLocal(bdig[j]));
            fc.emit(Op::CheckedSub);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(s1));
            emit_limb_carry(fc, true, rword[j], bdig[j], s1, neg1, shift_c, one);
            fc.emit(Op::SetLocal(bo1));
            fc.emit(Op::GetLocal(s1));
            fc.emit(Op::GetLocal(borrow));
            fc.emit(Op::CheckedSub);
            fc.emit(Op::PopN(2));
            fc.emit(Op::SetLocal(s2));
            emit_limb_carry(fc, true, s1, borrow, s2, neg1, shift_c, one);
            fc.emit(Op::SetLocal(bo2));
            fc.emit(Op::GetLocal(s2));
            fc.emit(Op::SetLocal(tent[j]));
            fc.emit(Op::GetLocal(bo1));
            fc.emit(Op::GetLocal(bo2));
            fc.emit(Op::BitOr);
            fc.emit(Op::SetLocal(borrow));
        }
        // mask = 0 - borrow (all ones when R < |b|); notmask = ~mask.
        fc.emit(Op::Const(zero));
        fc.emit(Op::GetLocal(borrow));
        fc.emit(Op::CheckedSub);
        fc.emit(Op::PopN(2));
        fc.emit(Op::SetLocal(mask));
        fc.emit(Op::GetLocal(mask));
        fc.emit(Op::Const(neg1));
        fc.emit(Op::BitXor);
        fc.emit(Op::SetLocal(notmask));
        // R = borrow ? R : tentative.
        for j in 0..n as usize {
            fc.emit(Op::GetLocal(rword[j]));
            fc.emit(Op::GetLocal(mask));
            fc.emit(Op::BitAnd);
            fc.emit(Op::GetLocal(tent[j]));
            fc.emit(Op::GetLocal(notmask));
            fc.emit(Op::BitAnd);
            fc.emit(Op::BitOr);
            fc.emit(Op::SetLocal(rword[j]));
        }
        // Quotient bit i is 1 when the subtraction succeeded (no borrow).
        // Only the low N words are kept; higher bits are the fixed-divide
        // overflow and are discarded.
        if i < n_bits {
            let qwi = (i / word_bits as u128) as usize;
            let qbp = fc.add_constant(Value::Int((i % word_bits as u128) as i64));
            fc.emit(Op::GetLocal(qword[qwi]));
            fc.emit(Op::GetLocal(borrow));
            fc.emit(Op::Const(one));
            fc.emit(Op::BitXor);
            fc.emit(Op::Const(qbp));
            fc.emit(Op::Shl);
            fc.emit(Op::BitOr);
            fc.emit(Op::SetLocal(qword[qwi]));
        }
    }
    // Reapply the sign and select the quotient or the remainder.
    let result = if is_mod {
        emit_multiword_cond_negate(fc, &rword, sign_a, n, neg1, shift_c, one);
        rword
    } else {
        let qsign = fc.declare_local("__mw_qsg");
        fc.emit(Op::GetLocal(sign_a));
        fc.emit(Op::GetLocal(sign_b));
        fc.emit(Op::BitXor);
        fc.emit(Op::SetLocal(qsign));
        emit_multiword_cond_negate(fc, &qword, qsign, n, neg1, shift_c, one);
        qword
    };
    for &w in &result {
        fc.emit(Op::GetLocal(w));
    }
    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
        kind: crate::value_layout::CompositeKind::Array,
        count: n,
        byte_size,
    }));
    Ok(())
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
    compile_with_options(program, target, &CompileOptions::default())
}

/// Compilation options. Extends the default pipeline with opt-in
/// behaviour while leaving the existing entry points unchanged.
#[derive(Debug, Clone, Default)]
pub struct CompileOptions {
    /// When true, the compiler emits strippable debug metadata (B29)
    /// into each chunk's `debug_pool`. The current implementation emits
    /// `CallSite` records that map each call instruction to the source
    /// span of the call expression. Default false, so release builds
    /// carry no debug metadata and produce byte-identical output to the
    /// pre-B29 compiler.
    pub emit_debug: bool,
}

/// As [`compile_with_warnings`], with explicit [`CompileOptions`]. This
/// is the workhorse; the other entry points delegate here with default
/// options.
pub fn compile_with_options(
    program: &Program,
    target: &crate::target::Target,
    options: &CompileOptions,
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
    let (mut owned, generic_provenance) = crate::monomorphize::monomorphize_with_provenance(owned);
    // Re-typecheck the monomorphized program so specialized bodies
    // benefit from concrete-type method resolution. This pass also records
    // the authoritative per-function expression-type table into
    // `owned.fn_expr_types`, which the compiler consults for flat-access
    // baking (B28 P3 item 5). It runs post-monomorphization so every function
    // is a concrete specialization and the per-function span keys are
    // collision-free.
    crate::typecheck::check_with_target_recording(&mut owned, *target).map_err(|e| {
        CompileError {
            message: format!("type error after monomorphization: {}", e.message),
            span: e.span,
        }
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
                validate_data_field_type(
                    &field.type_expr,
                    &program.types,
                    pass_visibility,
                    &mut visiting,
                )?;
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
    // `shared_layout` is filled in below, once `type_info` is available to
    // compute per-shared-slot byte offsets and kinds (B28 item 2).
    let mut data_layout = if data_layout_slots.is_empty() {
        None
    } else {
        Some(DataLayout {
            slots: data_layout_slots,
            shared_layout: Vec::new(),
            private_composite_layout: Vec::new(),
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

    // Build type info for the compiler's static analyses. The target's
    // scalar widths are recorded so the flat-tuple layout (B28 P2) bakes
    // offsets at the same width the module header declares.
    let mut type_info = TypeInfo {
        word_bytes: (1usize << target.word_bits_log2) / 8,
        float_bytes: (1usize << target.float_bits_log2) / 8,
        ..TypeInfo::default()
    };
    for type_def in &program.types {
        match type_def {
            TypeDef::Struct(s) => {
                let mut fields = BTreeMap::new();
                let mut order = Vec::with_capacity(s.fields.len());
                for f in &s.fields {
                    fields.insert(f.name.clone(), f.type_expr.clone());
                    order.push((f.name.clone(), f.type_expr.clone()));
                }
                type_info.structs.insert(s.name.clone(), fields);
                type_info.struct_field_order.insert(s.name.clone(), order);
                type_info.struct_defs.insert(s.name.clone(), s.clone());
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
                type_info.enum_defs.insert(e.name.clone(), e.clone());
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
    // A native `use` declaration that carries a signature contributes its
    // return type, so `infer_expr_type` can recover a native call's type
    // (B28 P3 item 3). The flat tuple/array access baking needs reliable
    // element types; a native whose result feeds a composite must be
    // typeable for the composite to be flat rather than boxed. The entry is
    // keyed by both the bare imported name and the fully qualified path so
    // either call form resolves; a script function of the same name (already
    // inserted) takes precedence. A native without a signature is left
    // absent, so a composite holding its result falls back to boxed.
    for use_decl in &program.uses {
        if let (ImportItem::Name(name), Some(sig)) = (&use_decl.import, &use_decl.signature) {
            type_info
                .function_returns
                .entry(name.clone())
                .or_insert_with(|| sig.return_type.clone());
            if !use_decl.path.is_empty() {
                let mut full = use_decl.path.join("::");
                full.push_str("::");
                full.push_str(name);
                type_info
                    .function_returns
                    .entry(full)
                    .or_insert_with(|| sig.return_type.clone());
            }
        }
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

    // Lay out the persistent composite body pool and record each single
    // composite private slot's fixed body offset (B28 P3 item 5, item 3a).
    // Private slots take unified indices starting at `shared_count`, in the
    // same declaration order the slot table used, so the unified index and the
    // cumulative body offset are produced by one walk. The total sizes the
    // pool, declared in the module header; the offset map drives the
    // `SetDataComposite` emission below. A slot whose offset would exceed the
    // sixteen-bit op operand is omitted and falls back to the inline path.
    // Arrays are deferred (zero body bytes), so they neither size the pool nor
    // enter the map. Computed before the codegen loop so the map is available
    // to emission.
    // Two cooperating placements drive the persistent composite pool, both
    // copying a flat composite body into the pool through
    // `persist_composite_body` so no private composite write needs a
    // global-heap owned body (B28 item 2 step 6A). `persistent_composite_offsets`
    // maps a single composite field whose offset fits the sixteen-bit
    // `SetDataComposite` operand and drives that op's emission (item 3a, the
    // common small case). `private_composite_layout` carries every other private
    // composite slot, an array-of-composite element slot or an offset-overflow
    // single slot, keyed by slot index with a `u32` offset the runtime reads at
    // a flat-composite write through the plain `SetData`/`SetDataIndexed` path,
    // with no baked operand. Both share one running `total` so the offsets never
    // collide, and `total` sizes the pool declared in the module header. This is
    // the linker-style fixed-address placement of program state: every private
    // composite slot, array elements included, has a statically baked pool
    // address. Computed before the codegen loop so the offset map is available
    // to `SetDataComposite` emission.
    let (persistent_composite_bytes, persistent_composite_offsets, private_composite_layout) = {
        let mut total: u32 = 0;
        let mut offsets: BTreeMap<u16, u16> = BTreeMap::new();
        let mut table: Vec<crate::bytecode::PrivateCompositeSlot> = Vec::new();
        let mut slot_idx: u32 = shared_count;
        for decl in &program.data_decls {
            if !matches!(decl.visibility, crate::ast::DataVisibility::Private) {
                continue;
            }
            for field in &decl.fields {
                let n = slots_for_data_type(&field.type_expr) as u32;
                // The flat composite body size of the leaf element. A single
                // composite field is its own leaf (`n == 1`); an
                // array-of-composite field has `n` element slots each of the
                // innermost element's body size. A scalar or array-of-scalar
                // leaf is `0` and reserves no pool, its slots storing scalars.
                let leaf = innermost_non_array_type(&field.type_expr);
                let body = data_field_pool_bytes(leaf, &type_info);
                if body > 0 {
                    if n == 1 {
                        // A single composite field: prefer the sixteen-bit
                        // `SetDataComposite` operand path; fall back to the
                        // table when the offset would overflow `u16`.
                        match (u16::try_from(slot_idx), u16::try_from(total)) {
                            (Ok(slot_u16), Ok(off_u16)) => {
                                offsets.insert(slot_u16, off_u16);
                            }
                            (Ok(slot_u16), Err(_)) => {
                                table.push(crate::bytecode::PrivateCompositeSlot {
                                    slot: slot_u16,
                                    offset: total,
                                });
                            }
                            _ => {}
                        }
                        total = total.saturating_add(body as u32);
                    } else {
                        // An array-of-composite field: every element slot is a
                        // composite of `body` bytes at a distinct pool offset,
                        // carried in the table and resolved by slot at the
                        // `SetDataIndexed` write.
                        for k in 0..n {
                            if let Ok(slot_u16) = u16::try_from(slot_idx + k) {
                                table.push(crate::bytecode::PrivateCompositeSlot {
                                    slot: slot_u16,
                                    offset: total,
                                });
                            }
                            total = total.saturating_add(body as u32);
                        }
                    }
                }
                slot_idx = slot_idx.saturating_add(n);
            }
        }
        (total, offsets, table)
    };

    // The shared data segment's flat byte total and its per-shared-slot layout
    // table (B28 item 2 shared-data re-architecture). `shared_data_bytes` is the
    // size of the borrowed host-owned buffer the embedder lends at
    // `call`/`resume`, laid out as a flat struct at the module's scalar widths.
    // The layout table gives each shared slot its byte offset and kind so the
    // existing `GetData`/`SetData` reach the buffer with no new opcode (the
    // rad-hard minimal-ISA choice); it is filled into `data_layout` below. No
    // runtime path reads either yet (the slot model is still active in this
    // step). The shared-field walk order matches the slot-assignment walk
    // above, so the table is indexed by shared slot index.
    let (shared_data_flat_bytes, shared_slot_layout): (
        u32,
        Vec<crate::bytecode::SharedSlotLayout>,
    ) = {
        let mut entries: Vec<crate::bytecode::SharedSlotLayout> = Vec::new();
        let mut offset: u16 = 0;
        for decl in &program.data_decls {
            if !matches!(decl.visibility, crate::ast::DataVisibility::Shared) {
                continue;
            }
            for field in &decl.fields {
                let consumed = push_shared_slot_layout(
                    &field.type_expr,
                    &type_info,
                    offset,
                    field.span,
                    &mut entries,
                )?;
                offset = offset.checked_add(consumed).ok_or_else(|| CompileError {
                    message: String::from(
                        "shared data segment exceeds the 64KB flat host-buffer limit",
                    ),
                    span: field.span,
                })?;
            }
        }
        (offset as u32, entries)
    };
    if let Some(dl) = data_layout.as_mut() {
        dl.shared_layout = shared_slot_layout;
        dl.private_composite_layout = private_composite_layout;
    }

    // Compile each function group. After emission, enforce the
    // V0.2.0 Phase 6 chunk-size limit: any chunk whose op count
    // exceeds `CHUNK_SIZE_HARD_LIMIT` is rejected as a
    // `CompileError` because the control-flow opcodes carry `u16`
    // jump targets that would not fit. Chunks that cross
    // `CHUNK_SIZE_SOFT_WARN_THRESHOLD` are admissible but produce
    // a `CompileWarning` prompting decomposition into helpers.
    let mut chunks: Vec<Chunk> = Vec::new();
    // Set when any chunk emits a flat composite that could intern a host
    // opaque; gates the opaque-registry arena bound below (B28 P3 item 5).
    // Read only by the verify-gated WCMU aux-arena computation, so a no-verify
    // build assigns but never reads it (matching the `unused_mut` allow at the
    // chunk loop above).
    #[cfg_attr(not(feature = "verify"), allow(unused_assignments, unused_variables))]
    let mut may_intern_opaque = false;
    for (name, defs) in &groups {
        let generic_origin = generic_provenance
            .get(name)
            .map(|(origin, type_args)| (origin.as_str(), type_args.as_str()));
        let (chunk, chunk_may_intern_opaque) = compile_function_group(
            name,
            defs,
            &function_map,
            &native_map,
            &native_externals,
            &data_fields,
            &const_fields,
            &type_info,
            &program.fn_expr_types,
            &persistent_composite_offsets,
            options.emit_debug,
            generic_origin,
        )?;
        #[cfg_attr(not(feature = "verify"), allow(unused_assignments))]
        {
            may_intern_opaque |= chunk_may_intern_opaque;
        }
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
                        crate::bytecode::Op::SetDataComposite(slot, _) => {
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

    // Sum the persistent flat-composite body storage the private `.data`
    // slots need (B28 P3 item 5, item 3a). A private slot holding a flat
    // composite stores its body in the arena's persistent region so it
    // survives RESET in place; a scalar slot stores its value inline and needs
    // no body. `program` here is the monomorphized program, so field types are
    // concrete and `type_info` resolves their layouts.
    #[cfg_attr(not(feature = "verify"), allow(unused_mut))]
    let mut module = Module {
        schema_hash: crate::bytecode::compute_schema_hash(data_layout.as_ref()),
        enum_layouts: build_enum_layouts(&type_info),
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
        // The runtime ephemeral tracking-list pre-size figure is computed
        // when the relocation of those lists into the arena lands (B28 P3
        // item 5, Phase C); zero until then.
        aux_arena_bytes: 0,
        // Persistent flat-composite body pool for private `.data` slots (B28
        // P3 item 5, item 3a). Summed over private fields below.
        persistent_composite_bytes,
        // Flags is populated by the verifier (under `verify`
        // feature) at end of compile_with_target. `shared_data_bytes` is the
        // true flat byte size of the borrowed host-owned shared buffer (B28
        // item 2): the sum of each shared field's flat size at the module's
        // scalar widths. `private_data_bytes` still mirrors the slot partition
        // in `VALUE_SLOT_SIZE_BYTES` units, because private slots remain a
        // `Value` array in the arena persistent region.
        flags: 0,
        shared_data_bytes: shared_data_flat_bytes,
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
        // Peak per-iteration arena heap (top-region) bytes across Stream
        // chunks, used to bound the opaque registry's arena footprint
        // (B28 P3 item 5, Phase C2): every distinct interned opaque has its
        // word-sized index stored in a live flat composite body in the heap
        // region (not reclaimed until RESET), so the number of interns is at
        // most `heap_bytes / word_bytes`.
        let mut max_heap: u32 = 0;
        let mut wcet_overflow = false;
        let mut wcmu_overflow = false;
        // Compute the structural verification trace per chunk before the
        // mutable loop below, since `chunk_verification_obligations`
        // borrows the whole module (for the data layout) and the loop
        // borrows `module.chunks` mutably. `verify(&module)` above
        // returned Ok, so each chunk's trace is complete (not truncated
        // at a failing check).
        let per_chunk_obligations: alloc::vec::Vec<
            alloc::vec::Vec<crate::verify::VerificationObligation>,
        > = if options.emit_debug {
            module
                .chunks
                .iter()
                .map(|c| crate::verify::chunk_verification_obligations(c, &module))
                .collect()
        } else {
            alloc::vec::Vec::new()
        };
        for (chunk_idx, chunk) in module.chunks.iter_mut().enumerate() {
            // Under debug emission, record the chunk's verification
            // trace as one VerifierWitness per discharged obligation,
            // keyed to the op position it concerns. Each record's
            // operands are [pass, property] string indices. This is a
            // verifier-stage record, appended to the built chunk rather
            // than emitted during codegen.
            if options.emit_debug {
                let obligations = &per_chunk_obligations[chunk_idx];
                let pool = chunk
                    .debug_pool
                    .get_or_insert_with(crate::debug_meta::DebugPool::default);
                for ob in obligations {
                    let pass = intern_debug_string(pool, ob.pass);
                    let property = intern_debug_string(pool, ob.property);
                    pool.records.push(crate::debug_meta::DebugRecord {
                        op_index: ob.op_index,
                        kind: crate::debug_meta::DebugRecordKind::VerifierWitness,
                        operands: alloc::vec![pass, property],
                    });
                }
            }
            if matches!(chunk.block_type, crate::bytecode::BlockType::Stream) {
                match crate::verify::wcet_stream_iteration(chunk) {
                    Ok(c) => {
                        max_wcet = max_wcet.max(c);
                        // Under debug emission, record the chunk's
                        // declared per-iteration WCET as a WcetMarker so
                        // runtime telemetry can compare measured cost
                        // against the declared bound. This is a
                        // verifier-stage record: it is appended to the
                        // already-built chunk after the WCET pass rather
                        // than emitted during codegen. The u32 cycle
                        // count is carried as two u16 operands (low,
                        // high) following the block id (0 = whole chunk).
                        if options.emit_debug {
                            let pool = chunk
                                .debug_pool
                                .get_or_insert_with(crate::debug_meta::DebugPool::default);
                            pool.records.push(crate::debug_meta::DebugRecord {
                                op_index: 0,
                                kind: crate::debug_meta::DebugRecordKind::WcetMarker,
                                operands: alloc::vec![0u16, (c & 0xFFFF) as u16, (c >> 16) as u16],
                            });
                            // A resource-bound VerifierWitness obligation
                            // citing the proof just discharged: the Ok
                            // arm of wcet_stream_iteration is exactly the
                            // proof that a finite per-iteration WCET bound
                            // exists for this chunk under the bundled
                            // nominal cost model. The cycle count itself
                            // is in the WcetMarker above. Emitted only on
                            // the Ok arm, so overflow records no witness.
                            let pass = intern_debug_string(pool, "resource-bounds");
                            let property =
                                intern_debug_string(pool, "wcet-per-iteration-bound-proven");
                            pool.records.push(crate::debug_meta::DebugRecord {
                                op_index: 0,
                                kind: crate::debug_meta::DebugRecordKind::VerifierWitness,
                                operands: alloc::vec![pass, property],
                            });
                        }
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
                            max_heap = max_heap.max(heap);
                            // A resource-bound VerifierWitness obligation:
                            // the Ok arm with a non-saturating total is
                            // the proof that a finite per-iteration WCMU
                            // bound exists for this chunk. This is the
                            // per-iteration bound only; admission against
                            // a host arena capacity is a separate
                            // load-time check (verify_resource_bounds)
                            // not run here, so the witness does not claim
                            // it. Emitted only when the bound is finite.
                            if options.emit_debug {
                                let pool = chunk
                                    .debug_pool
                                    .get_or_insert_with(crate::debug_meta::DebugPool::default);
                                let pass = intern_debug_string(pool, "resource-bounds");
                                let property =
                                    intern_debug_string(pool, "wcmu-per-iteration-bound-proven");
                                pool.records.push(crate::debug_meta::DebugRecord {
                                    op_index: 0,
                                    kind: crate::debug_meta::DebugRecordKind::VerifierWitness,
                                    operands: alloc::vec![pass, property],
                                });
                            }
                        }
                    }
                    Err(_) => {
                        wcmu_overflow = true;
                    }
                }
            } else if options.emit_debug
                && matches!(
                    chunk.block_type,
                    crate::bytecode::BlockType::Func | crate::bytecode::BlockType::Reentrant
                )
            {
                // Func and Reentrant chunks: emit per-chunk resource-bound
                // obligations for the witness. These attest that a finite
                // whole-body WCET and WCMU bound was proven under the
                // nominal cost model. For a Func chunk the WCET is the
                // per-call bound; for a Reentrant chunk it is the
                // cumulative-across-resumptions bound (a sound upper bound
                // on any single resume) while the WCMU is the persistent
                // peak (the coroutine frame survives yields). Unlike the
                // Stream arm above, neither is folded into
                // `module.wcet_cycles`/`module.wcmu_bytes`, which remain
                // the per-iteration maximum across Stream chunks. The
                // bound is shallow with respect to calls, like the Stream
                // WCET. Each obligation is emitted only on the proof arm,
                // so a chunk whose bound does not prove (e.g. a loop with
                // no statically extractable iteration count) records none.
                if crate::verify::wcet_whole_chunk(chunk).is_ok() {
                    let pool = chunk
                        .debug_pool
                        .get_or_insert_with(crate::debug_meta::DebugPool::default);
                    let pass = intern_debug_string(pool, "resource-bounds");
                    let property = intern_debug_string(pool, "wcet-per-chunk-bound-proven");
                    pool.records.push(crate::debug_meta::DebugRecord {
                        op_index: 0,
                        kind: crate::debug_meta::DebugRecordKind::VerifierWitness,
                        operands: alloc::vec![pass, property],
                    });
                }
                if let Ok((stack, heap)) = crate::verify::wcmu_whole_chunk(chunk)
                    && stack.saturating_add(heap) != u32::MAX
                {
                    let pool = chunk
                        .debug_pool
                        .get_or_insert_with(crate::debug_meta::DebugPool::default);
                    let pass = intern_debug_string(pool, "resource-bounds");
                    let property = intern_debug_string(pool, "wcmu-per-chunk-bound-proven");
                    pool.records.push(crate::debug_meta::DebugRecord {
                        op_index: 0,
                        kind: crate::debug_meta::DebugRecordKind::VerifierWitness,
                        operands: alloc::vec![pass, property],
                    });
                }
            }
        }
        module.wcet_cycles = if wcet_overflow { u32::MAX } else { max_wcet };
        module.wcmu_bytes = if wcmu_overflow { u32::MAX } else { max_wcmu };
        // Bound the opaque registry's arena footprint (B28 P3 item 5). The
        // registry lives in the arena bottom region and is `clear`ed each
        // iteration; its per-iteration peak is at most one `Arc` per distinct
        // interned opaque, and each such opaque has its word-sized index
        // stored in a live flat-composite body, so the count is at most
        // `heap_bytes / word_bytes`. That heap-derived figure is
        // representation-independent, which matters because an unsignatured
        // native can return an opaque the compiler cannot type and a
        // position-based count would undercount it. The registry tightening
        // gates that bound on `may_intern_opaque`: a module that never
        // constructs a flat composite able to intern an opaque (the dominant
        // case) needs no registry and reserves zero, while one that might
        // falls back to the sound heap-derived figure. The flag is set
        // conservatively at the flat `NewComposite` emission sites, including
        // for value-driven tuples/arrays whose element types the compiler
        // cannot recover, so a zero here soundly means no opaque can be
        // interned. `auto_arena_capacity_for` adds this so a host that
        // autosizes provisions for the registry.
        let word_bytes = (1usize << module.word_bits_log2) / 8;
        let arc_bytes = core::mem::size_of::<alloc::sync::Arc<dyn crate::opaque::HostOpaque>>();
        module.aux_arena_bytes = if wcmu_overflow || word_bytes == 0 || !may_intern_opaque {
            // A module that never constructs a flat composite able to intern a
            // host opaque needs no registry: the dominant case (opaque-free
            // programs) now reserves zero rather than the heap-derived bound.
            // The flag is set conservatively, including for value-driven
            // tuples/arrays with untypeable elements, so a zero here soundly
            // means no opaque can be interned (B28 P3 item 5 registry
            // tightening).
            0
        } else {
            let max_interns = (max_heap as usize).div_ceil(word_bytes);
            max_interns.saturating_mul(arc_bytes).min(u32::MAX as usize) as u32
        };

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
        TypeExpr::Multiword(_, _, _) => String::from("Multiword"),
        TypeExpr::Named(name, _, _, _) => name.clone(),
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
                Stmt::Assert { cond, .. } => {
                    if expr_uses(cond, name) {
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
/// Fill in unresolved enum discriminants in a constant value from the
/// enum variant-order table, recursively (B28 P2). Const-evaluation builds
/// `ConstValue::Enum` with `discriminant: None` because it has no type
/// tables; this resolves them so the constant materialises into the flat
/// body the access ops bake against.
fn resolve_const_enum_discriminants(
    cv: &mut crate::bytecode::ConstValue,
    order: &BTreeMap<String, Vec<(String, i64)>>,
) {
    use crate::bytecode::ConstValue;
    match cv {
        ConstValue::Enum {
            type_name,
            variant,
            discriminant,
            fields,
        } => {
            if discriminant.is_none() {
                *discriminant = order
                    .get(type_name)
                    .and_then(|vs| vs.iter().find(|(n, _)| n == variant).map(|(_, d)| *d));
            }
            for f in fields {
                resolve_const_enum_discriminants(f, order);
            }
        }
        ConstValue::Tuple(items) | ConstValue::Array(items) => {
            for i in items {
                resolve_const_enum_discriminants(i, order);
            }
        }
        ConstValue::Struct { fields, .. } => {
            for (_, f) in fields {
                resolve_const_enum_discriminants(f, order);
            }
        }
        _ => {}
    }
}

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
                discriminant: None,
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
            if elements.len() != len.as_lit().unwrap_or(-1) as usize {
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
        (ConstInitializer::Struct { name, fields }, TypeExpr::Named(decl_name, _, _, _)) => {
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
            TypeExpr::Named(decl_name, _, _, _),
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
                discriminant: None,
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
            let total = elem_slots.saturating_mul(len.as_lit().unwrap_or(0) as u32);
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
            TypeExpr::Array(elem, len, _) => {
                let len = len.as_lit().ok_or_else(|| CompileError {
                    message: alloc::format!("data array size `{}` is not a resolved constant", len),
                    span,
                })?;
                (*elem, len)
            }
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
        // Track the element type as one array dimension is peeled per
        // index level, so each `GetIndex` bakes the flat-or-boxed access
        // form that matches the body the constant materialised into
        // (B28 P2). A const scalar array is flat; a nested-array element
        // is a composite and stays boxed.
        let mut cur_ty = fc
            .type_info
            .data_field_types
            .get(chain.data_name)
            .and_then(|m| m.get(chain.field))
            .cloned();
        for index_expr in chain.indices {
            compile_expr(fc, index_expr)?;
            let elem_ty = cur_ty.as_ref().and_then(element_type_of);
            fc.emit(Op::GetIndex(array_elem_operand(
                elem_ty.as_ref(),
                &fc.type_info,
            )));
            cur_ty = elem_ty;
            // Each level's index can trap out-of-bounds; record the
            // operator site so the fault resolves exactly (B29 item 2).
            fc.record_operator_site(&span);
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
    // A dynamic data-array index can trap out-of-bounds; record the
    // operator site so the fault resolves exactly (B29 item 2).
    fc.record_operator_site(&span);
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
    // A dynamic data-array index can trap out-of-bounds; record the
    // operator site so the fault resolves exactly (B29 item 2).
    fc.record_operator_site(&span);
    Ok(())
}

fn validate_data_field_type(
    type_expr: &TypeExpr,
    types: &[TypeDef],
    visibility: DataVisibility,
    visiting: &mut BTreeSet<String>,
) -> Result<(), CompileError> {
    match type_expr {
        TypeExpr::Multiword(_, _, _) => Ok(()),
        TypeExpr::Prim(prim, span) => match prim {
            PrimType::Byte
            | PrimType::Word
            | PrimType::Fixed(_)
            | PrimType::Float
            | PrimType::Bool => Ok(()),
            // A flat `Text` field is a fixed two-word `(ptr, len)` handle, so
            // it has a statically known size in the body and is admissible in
            // a `private` data segment (B28 P3 item 4). A static-string field
            // points at immortal rodata and survives RESET in place; a dynamic
            // string field points at the ephemeral arena and resolves cleanly
            // stale after RESET through the epoch backstop, so it is never a
            // dangling read. A `shared` data field is the host-script boundary
            // and keeps the prior rejection: a host-owned text slot is not yet
            // wired through the marshalling boundary.
            // `Const` is not reached through this validator (the caller
            // iterates only the `Shared` and `Private` passes; const data is
            // validated with literal initializers elsewhere), but const text
            // is a rodata literal and is grouped with `Private` for a
            // meaningful, exhaustive match.
            PrimType::Text => match visibility {
                DataVisibility::Private | DataVisibility::Const => Ok(()),
                DataVisibility::Shared => Err(CompileError {
                    message: String::from(
                        "data field type Text is not admissible in a `shared` data \
                         segment: a host-owned text slot is not yet supported",
                    ),
                    span: *span,
                }),
            },
        },
        TypeExpr::Unit(_) => Ok(()),
        TypeExpr::Tuple(elems, _) => {
            for elem in elems {
                validate_data_field_type(elem, types, visibility, visiting)?;
            }
            Ok(())
        }
        TypeExpr::Array(elem, _len, _) => {
            validate_data_field_type(elem, types, visibility, visiting)
        }
        TypeExpr::Option(inner, _) => validate_data_field_type(inner, types, visibility, visiting),
        TypeExpr::Labelled(inner, _, _) => {
            validate_data_field_type(inner, types, visibility, visiting)
        }
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
        TypeExpr::NegativeLabelled(inner, _, _) => {
            validate_data_field_type(inner, types, visibility, visiting)
        }
        TypeExpr::Named(name, _args, _, span) => {
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
                        validate_data_field_type(&field.type_expr, types, visibility, visiting)?;
                    }
                    visiting.remove(name);
                    Ok(())
                }
                Some(TypeDef::Enum(e)) => {
                    visiting.insert(name.clone());
                    for variant in &e.variants {
                        for ftype in &variant.fields {
                            validate_data_field_type(ftype, types, visibility, visiting)?;
                        }
                    }
                    visiting.remove(name);
                    Ok(())
                }
                Some(TypeDef::Newtype(n)) => {
                    visiting.insert(name.clone());
                    validate_data_field_type(&n.underlying, types, visibility, visiting)?;
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
    fn_expr_types: &BTreeMap<String, BTreeMap<crate::token::Span, TypeExpr>>,
    persistent_composite_offsets: &BTreeMap<u16, u16>,
    emit_debug: bool,
    generic_origin: Option<(&str, &str)>,
) -> Result<(Chunk, bool), CompileError> {
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
        // This group's authoritative expression-type table (B28 P3 item 5),
        // keyed by the group name (the mangled specialization name after
        // monomorphization). Absent for a group the recording pass did not
        // table, in which case the structural inference path runs.
        fn_expr_types.get(name).cloned().unwrap_or_default(),
        // The module-wide private-composite persistent layout (B28 P3 item 5,
        // item 3a); the same map for every function.
        persistent_composite_offsets.clone(),
        emit_debug,
    );
    fc.chunk.param_count = param_count;
    fc.chunk.param_types = first.params.iter().map(type_tag_for_param).collect();
    if let Some((origin, type_args)) = generic_origin {
        fc.record_generic_instantiation(origin, type_args);
    }

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
                if let Some(crate::ast::TypeExpr::Named(type_name, _, _, _)) = &param.type_expr
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
                let fail = compile_pattern_test(
                    &mut fc,
                    &param.pattern,
                    param_slots[i],
                    param.type_expr.as_ref(),
                )?;
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

    // Function entry is a breakpoint candidate at op 0 (B29), distinct
    // from the per-statement candidates: a debugger arms it to break
    // when the function is entered, before any statement runs.
    fc.record_function_entry(&first.span);

    let may_intern_opaque = fc.may_intern_opaque;
    Ok((fc.finish(), may_intern_opaque))
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
        // The block's tail expression carries no `Stmt`, so without this
        // it would have no `SourceSpan` and a fault inside it would
        // resolve only to an outer statement (or, for a function whose
        // whole body is one expression, to nothing). Recording the tail
        // expression's span gives fault localization a tighter enclosing
        // span (B29, item 2). Captured before codegen so the span is
        // keyed to the tail's first op.
        let tail_start_op = fc.chunk.ops.len();
        compile_expr(fc, tail)?;
        fc.record_statement(tail_start_op, &tail.span());
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
    // A private slot that holds a flat composite stores its body in the arena
    // persistent region at a compiler-assigned fixed offset (B28 P3 item 5,
    // item 3a); other slots use the inline `SetData` path.
    if let Some(&rel_offset) = fc.persistent_composite_offsets.get(&slot) {
        fc.emit(Op::SetDataComposite(slot, rel_offset));
    } else {
        fc.emit(Op::SetData(slot));
    }
    Ok(())
}

/// Compile a single statement.
/// The source span of a statement, used for B29 `SourceSpan` and
/// `LineNumber` debug records.
fn stmt_span(stmt: &Stmt) -> crate::token::Span {
    match stmt {
        Stmt::Let(l) => l.span,
        Stmt::For(f) => f.span,
        Stmt::Break(s) => *s,
        Stmt::DataFieldAssign { span, .. } => *span,
        Stmt::DataFieldIndexAssign { span, .. } => *span,
        Stmt::Expr(e) => e.span(),
        Stmt::Assert { span, .. } => *span,
    }
}

fn compile_stmt(fc: &mut FuncCompiler, stmt: &Stmt) -> Result<(), CompileError> {
    let stmt_start_op = fc.chunk.ops.len();
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
        Stmt::Assert {
            cond,
            message,
            span,
        } => {
            // Debug assert (B29). Compiled out entirely in a release
            // build; under a debug build, a runtime check that traps
            // when the condition is false, plus a strippable
            // AssertionContext record for the source span and message.
            //
            // Codegen: `<cond>; Not; If(end); Trap(AssertionFailed);
            // EndIf`. `If` pops `!cond` and, when it is false (cond
            // true), skips the trap; when true (cond false), the trap
            // fires.
            if fc.emit_debug {
                compile_expr(fc, cond)?;
                fc.emit(Op::Not);
                let skip = fc.emit_jump(Op::If(0));
                let trap_op = fc.emit(Op::Trap(crate::bytecode::TrapKind::AssertionFailed.code()));
                fc.patch_jump(skip);
                fc.emit(Op::EndIf);
                fc.record_assert(trap_op, span, message.as_deref());
            }
        }
    }
    fc.record_statement(stmt_start_op, &stmt_span(stmt));
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
/// Collect the variable names bound by a checked-arm kind whose
/// bindings carry exactly the operation's operand type (B28 P2).
///
/// Only `ok`, `overflow`, `underflow`, and `zero_divisor` qualify: on
/// `Word` operands their patterns bind `Word` halves or results, and
/// on `Byte` operands a single `Byte`, so the binding type equals the
/// operand type the caller infers from the guarded operation. Other
/// arm kinds bind values of unrelated types (a native error code, a
/// float NaN result) and are intentionally excluded so the caller does
/// not infer an incorrect type, which would bake a wrong flat offset
/// and corrupt the read. Their absence yields `None`, a safe boxed
/// fallback.
fn collect_checked_arm_bindings(kind: &crate::ast::CheckedArmKind, out: &mut Vec<String>) {
    use crate::ast::CheckedArmKind as K;
    let mut push = |p: &Pattern| {
        if let Pattern::Variable(name, _) = p {
            out.push(name.clone());
        }
    };
    match kind {
        K::Ok(p) | K::ZeroDivisor(p) => push(p),
        K::Overflow(h, l) | K::Underflow(h, l) => {
            push(h);
            if let Some(l) = l {
                push(l);
            }
        }
        // Other kinds bind values whose type is not the operand type;
        // excluded to keep inference accurate-or-None.
        _ => {}
    }
}

/// Infer a checked construct's arm-body type, resolving arm-binding
/// idents to the operation's operand type (B28 P2).
///
/// Tuple-literal bodies recurse so a scalar tuple body infers fully;
/// any other body falls back to ordinary inference, which is itself
/// accurate-or-`None`. The result therefore never names an incorrect
/// type, so a baked flat access can only be correct or absent.
fn infer_arm_body_type(
    fc: &FuncCompiler,
    body: &Expr,
    bound: &[String],
    operand_ty: &TypeExpr,
) -> Option<TypeExpr> {
    match body {
        Expr::TupleLiteral { elements, span } => {
            let mut tys = Vec::with_capacity(elements.len());
            for e in elements {
                tys.push(infer_arm_body_type(fc, e, bound, operand_ty)?);
            }
            Some(TypeExpr::Tuple(tys, *span))
        }
        Expr::Ident { name, .. } if bound.iter().any(|b| b == name) => Some(operand_ty.clone()),
        _ => infer_expr_type(fc, body),
    }
}

fn infer_expr_type(fc: &FuncCompiler, expr: &Expr) -> Option<TypeExpr> {
    // Consult the authoritative per-function type table first (B28 P3 item 5).
    // It is recorded by the post-monomorphization type-check pass, so an entry
    // is the concrete resolved type for this exact expression and is preferred
    // over the structural inference below. A missing entry (an expression the
    // pass could not fully resolve, or a span excluded as ambiguous) falls
    // through to the structural path, preserving the accurate-or-None
    // behaviour.
    if let Some(ty) = fc.expr_types.get(&expr.span()) {
        return Some(ty.clone());
    }
    match expr {
        Expr::StructInit { name, span, .. } => {
            Some(TypeExpr::Named(name.clone(), Vec::new(), Vec::new(), *span))
        }
        Expr::EnumVariant {
            enum_name,
            variant,
            args,
            span,
            ..
        } => {
            // `Option::Some(x)` infers as `Option<T>` so a later flat access
            // (a `match` scrutinee binding) recovers the payload type `T`
            // (B28 P3 item 5 C4); `Option` is generic and absent from the
            // type tables, so `Named("Option")` would drop `T`.
            if enum_name == "Option" && variant == "Some" {
                let payload = args.first().and_then(|a| infer_expr_type(fc, a))?;
                Some(TypeExpr::Option(Box::new(payload), *span))
            } else {
                Some(TypeExpr::Named(
                    enum_name.clone(),
                    Vec::new(),
                    Vec::new(),
                    *span,
                ))
            }
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
            Some(TypeExpr::array_lit(
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
                BinOp::Add
                | BinOp::Sub
                | BinOp::Mul
                | BinOp::Div
                | BinOp::Mod
                | BinOp::Band
                | BinOp::Bor
                | BinOp::Bxor => infer_expr_type(fc, left).or_else(|| infer_expr_type(fc, right)),
                // A shift preserves the shifted value's type; the shift
                // amount (right) is a Word and must not be inferred from.
                BinOp::Shl | BinOp::AShl | BinOp::ShrA | BinOp::ShrL => infer_expr_type(fc, left),
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
        Expr::UnaryOp { operand, .. } => infer_expr_type(fc, operand),
        // The discriminant-to-enum construct (B35 P6) yields the
        // target enum type, so a `let`-bound result carries the enum
        // type for subsequent type-directed operations (e.g. a later
        // `as Word` cast).
        Expr::Checked { op_expr, arms, .. } => match op_expr.as_ref() {
            Expr::Cast {
                target: TypeExpr::Named(n, args, _, sp),
                ..
            } if fc.type_info.enum_variant_order.contains_key(n) => {
                Some(TypeExpr::Named(n.clone(), args.clone(), Vec::new(), *sp))
            }
            // The construct's result type is its arms' common body type.
            // The arm bindings (ok(v), overflow(h, l), …) carry the
            // guarded operation's operand type, which is not in scope
            // here, so the first arm's body is inferred with those names
            // bound to the operand type. This lets a let-destructure of
            // a checked construct yielding a scalar tuple recover the
            // tuple type and bake flat field access (B28 P2). Inference
            // stays accurate-or-None, so a wrong type is never baked.
            _ => {
                let operand_ty = infer_expr_type(fc, op_expr)?;
                let first = arms.first()?;
                let mut bound: Vec<String> = Vec::new();
                collect_checked_arm_bindings(&first.kind, &mut bound);
                infer_arm_body_type(fc, &first.body, &bound, &operand_ty)
            }
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
        TypeExpr::Multiword(_, _, _) => Some("Multiword".to_string()),
        TypeExpr::Tuple(_, _) => Some("tuple".to_string()),
        TypeExpr::Array(_, _, _) => Some("array".to_string()),
        TypeExpr::Option(_, _) => Some("Option".to_string()),
        TypeExpr::Named(name, _, _, _) => Some(name.clone()),
        TypeExpr::Labelled(inner, _, _) => type_expr_head(inner),
        TypeExpr::NegativeLabelled(inner, _, _) => type_expr_head(inner),
    }
}

/// Compile a let binding pattern with an associated type expression.
///
/// The type, when present, is recorded on the resulting local for
/// downstream optimization passes. Compound patterns destructure the
/// type along with the value.
/// Total flat byte size of a type at the module's widths, or `None` when
/// the type is not transitively flat (B28 P2 nested inlining).
///
/// The flat layout arithmetic lives once in the compile-time layout pass
/// ([`crate::value_layout::LayoutDescriptor::flat_byte_size`], built from a
/// [`LayoutContext`] over the module's type definitions). This function and
/// the runtime construction choke points consult that one predicate, so a
/// baked nested access always agrees with the body the construction builds.
/// A type the layout pass cannot resolve (an unknown name, an
/// unsubstituted generic) is treated as non-flat.
fn type_flat_size(ty: &TypeExpr, ti: &TypeInfo) -> Option<usize> {
    if ti.word_bytes == 0 {
        return None;
    }
    ti.layout_context()
        .layout_for(ty)
        .ok()?
        .flat_byte_size(ti.word_bytes, ti.float_bytes)
}

/// Build the per-enum-type layout descriptors the runtime uses to make an
/// enum's flat body type-driven (B37 / audit finding 25 follow-up).
///
/// For each enum the module declares, records the variant discriminants (from
/// the type checker's `enum_variant_order`) and the padded-body payload size at
/// the module's widths. `min_payload` is `word + payload_max - word` for a
/// uniformly-flat enum, taken from the same `flat_byte_size` predicate the
/// construction and access paths use, so a runtime-corrected body agrees with
/// the compiler's baked flat access. A non-flat enum records `min_payload = 0`:
/// it stays boxed and is matched by name, so it needs no padding hint. The
/// program is monomorphized here, so every enum name is concrete.
fn build_enum_layouts(ti: &TypeInfo) -> Vec<crate::bytecode::EnumLayout> {
    use crate::bytecode::{EnumLayout, EnumVariantDisc};
    ti.enum_variant_order
        .iter()
        .map(|(enum_name, variants)| {
            let ty = TypeExpr::Named(enum_name.clone(), Vec::new(), Vec::new(), Span::default());
            let min_payload = type_flat_size(&ty, ti)
                .and_then(|total| total.checked_sub(ti.word_bytes))
                .and_then(|payload_max| u32::try_from(payload_max).ok())
                .unwrap_or(0);
            EnumLayout {
                type_name: enum_name.clone(),
                variants: variants
                    .iter()
                    .map(|(name, disc)| EnumVariantDisc {
                        name: name.clone(),
                        disc: *disc,
                    })
                    .collect(),
                min_payload,
            }
        })
        .collect()
}

/// Bytes of persistent flat-composite body storage a private `.data` field of
/// the given type requires (B28 P3 item 5, item 3a).
///
/// A scalar field stores its value inline in the slot's `Value` cell and needs
/// no separate body, so it contributes zero. A single composite field (struct,
/// tuple, enum, option) that flattens contributes its flat body size; a
/// non-flat (reference-bearing) composite contributes zero because it is not
/// stored as a flat body in the persistent pool. An array field is deferred in
/// this increment and contributes zero: an array of composites keeps the prior
/// per-element behaviour, so it is not yet placed in the persistent pool. The
/// pool total and the per-slot offset map are computed from this function, so
/// they cover exactly the single-composite slots and stay consistent.
/// Peel array layers to the innermost non-array element type (B28 item 2 step
/// 6A). `[[Point; 2]; 3]` yields `Point`; a non-array type is returned
/// unchanged. Used to find the leaf element of a data field so its flat
/// composite body size sizes the persistent pool for every array element slot.
fn innermost_non_array_type(ty: &TypeExpr) -> &TypeExpr {
    let mut cur = ty;
    while let TypeExpr::Array(elem, _, _) = cur {
        cur = elem;
    }
    cur
}

fn data_field_pool_bytes(ty: &TypeExpr, ti: &TypeInfo) -> usize {
    match ty {
        // Callers pass the leaf element type (via `innermost_non_array_type`),
        // so an array never reaches here; a defensive `0` keeps the function
        // total over a bare array type (B28 item 2 step 6A generalised the pool
        // to array-of-composite element slots).
        TypeExpr::Array(_, _, _) => 0,
        _ => match ti.layout_context().layout_for(ty) {
            Ok(crate::value_layout::LayoutDescriptor::Scalar(_)) => 0,
            Ok(layout) => layout
                .flat_byte_size(ti.word_bytes, ti.float_bytes)
                .unwrap_or(0),
            Err(_) => 0,
        },
    }
}

/// Append the shared-buffer layout entries for one shared field's type, in slot
/// order, accumulating from `base_offset`, and return the bytes the field
/// consumes (B28 item 2 shared-data re-architecture).
///
/// A scalar contributes one `Scalar` entry; an array expands to one entry per
/// element slot (so `Op::GetDataIndexed` resolves an element slot whose entry
/// the runtime reads); a flat composite contributes one `Composite` entry
/// covering its whole flat body. A reference scalar (`Text` or opaque) or a
/// composite that transitively carries a reference cannot live in a flat host
/// buffer and is a compile error.
fn push_shared_slot_layout(
    ty: &TypeExpr,
    ti: &TypeInfo,
    base_offset: u16,
    span: crate::token::Span,
    out: &mut Vec<crate::bytecode::SharedSlotLayout>,
) -> Result<u16, CompileError> {
    let layout = ti
        .layout_context()
        .layout_for(ty)
        .map_err(|_| CompileError {
            message: String::from(
                "shared data field has no statically known flat layout for the host buffer",
            ),
            span,
        })?;
    push_shared_layout_desc(&layout, ti, base_offset, span, out)
}

/// Recursive worker for [`push_shared_slot_layout`] over a resolved layout
/// descriptor.
fn push_shared_layout_desc(
    layout: &crate::value_layout::LayoutDescriptor,
    ti: &TypeInfo,
    base_offset: u16,
    span: crate::token::Span,
    out: &mut Vec<crate::bytecode::SharedSlotLayout>,
) -> Result<u16, CompileError> {
    use crate::value_layout::{CompositeKind, LayoutDescriptor as LD, ScalarKind};
    let overflow = || CompileError {
        message: String::from("shared data segment exceeds the 64KB flat host-buffer limit"),
        span,
    };
    match layout {
        LD::Scalar(kind) => {
            if matches!(kind, ScalarKind::Text | ScalarKind::Opaque) {
                return Err(CompileError {
                    message: String::from(
                        "shared data field of reference type (Text or opaque) cannot live in a \
                         flat host buffer",
                    ),
                    span,
                });
            }
            let size = u16::try_from(kind.size_in_bytes(ti.word_bytes, ti.float_bytes))
                .map_err(|_| overflow())?;
            out.push(crate::bytecode::SharedSlotLayout {
                offset: base_offset,
                kind: kind.to_tag(),
                len: 0,
            });
            Ok(size)
        }
        LD::Array { element, count } => {
            let mut off = base_offset;
            let mut total: u16 = 0;
            for _ in 0..*count {
                let consumed = push_shared_layout_desc(element, ti, off, span, out)?;
                off = off.checked_add(consumed).ok_or_else(overflow)?;
                total = total.checked_add(consumed).ok_or_else(overflow)?;
            }
            Ok(total)
        }
        // A composite field (tuple, struct, enum) is a single slot whose body
        // is its whole flat byte range. The composite kind is stored so the
        // runtime re-wraps a copied-out shared composite correctly, which the
        // kind-sensitive flat access ops require. `flat_byte_size` is `None`
        // when the composite transitively carries a reference leaf, which the
        // host buffer cannot hold.
        LD::Tuple(_) | LD::Struct { .. } | LD::Enum { .. } => {
            let composite_kind = match layout {
                LD::Tuple(_) => CompositeKind::Tuple,
                LD::Struct { .. } => CompositeKind::Struct,
                LD::Enum { .. } => CompositeKind::Enum,
                _ => unreachable!("matched Tuple/Struct/Enum above"),
            };
            let len = layout
                .flat_byte_size(ti.word_bytes, ti.float_bytes)
                .ok_or_else(|| CompileError {
                    message: String::from(
                        "shared data composite field is not flat (it carries a reference); not \
                         admissible in a flat host buffer",
                    ),
                    span,
                })?;
            let len_u16 = u16::try_from(len).map_err(|_| overflow())?;
            out.push(crate::bytecode::SharedSlotLayout {
                offset: base_offset,
                kind: crate::bytecode::SHARED_SLOT_COMPOSITE_FLAG | composite_kind.to_tag(),
                len: len_u16,
            });
            Ok(len_u16)
        }
    }
}

/// The flat allocation byte size of a composite type for the
/// [`Op::NewComposite`] operand (B28 P4), or `None` when the type is not
/// flat (the boxed form) or its size exceeds the sixteen-bit operand. This
/// is the explicit allocation the worst-case-memory-usage verifier sums; it
/// equals the type's flat layout size, which the runtime packer reproduces
/// and the baked access offsets agree with.
fn flat_alloc_bytes(ty: &TypeExpr, ti: &TypeInfo) -> Option<u16> {
    let size = type_flat_size(ty, ti)?;
    if size <= u16::MAX as usize {
        Some(size as u16)
    } else {
        None
    }
}

/// A sound conservative flat allocation bound for `count` fields when the
/// exact size is unknown (B28 P4). Each field occupies at most
/// `VALUE_SLOT_SIZE_BYTES`, so `count * VALUE_SLOT_SIZE_BYTES` over-bounds
/// the actual flat layout; it is the verifier annotation for a value-driven
/// tuple or array whose element types the compiler could not recover.
/// Clamped to the sixteen-bit operand.
fn conservative_alloc_bytes(count: u16) -> u16 {
    (count as u32 * crate::bytecode::VALUE_SLOT_SIZE_BYTES).min(u16::MAX as u32) as u16
}

/// Classify a composite field as a flat scalar, a flat nested composite, or
/// not flat (B28 P2). The access-baking functions use this to choose
/// between the `Flat`, `FlatNested`, and boxed operand forms.
enum FlatFieldForm {
    /// A flat-eligible scalar field of the given kind.
    Scalar(crate::value_layout::ScalarKind),
    /// A nested flat composite field of the given byte size and kind.
    Nested(u16, crate::value_layout::CompositeKind),
    /// Not flat-eligible; the whole composite falls back to boxed.
    NotFlat,
}

/// Classify `ty` as a flat field form at the module's widths (B28 P2),
/// from its layout descriptor. A scalar yields the `Flat` form, a
/// transitively-flat composite the `FlatNested` form (with its byte size
/// and composite kind), and anything else `NotFlat`. A nested composite
/// whose size exceeds the sixteen-bit operand bound is reported `NotFlat`
/// so the whole composite falls back to boxed.
fn classify_flat_field(ty: &TypeExpr, ti: &TypeInfo) -> FlatFieldForm {
    if ti.word_bytes == 0 {
        return FlatFieldForm::NotFlat;
    }
    let Ok(descriptor) = ti.layout_context().layout_for(ty) else {
        return FlatFieldForm::NotFlat;
    };
    if let Some(kind) = descriptor.flat_scalar_kind() {
        // A flat `Text` field holds a host data pointer; it is flat only
        // when the word slot is at least the host pointer width, matching
        // the runtime gate in `LayoutDescriptor::flat_byte_size`. A
        // narrow-word build keeps `Text` boxed (B28 P3).
        if matches!(kind, crate::value_layout::ScalarKind::Text)
            && ti.word_bytes < core::mem::size_of::<usize>()
        {
            return FlatFieldForm::NotFlat;
        }
        return FlatFieldForm::Scalar(kind);
    }
    if let (Some(size), Some(variant)) = (
        descriptor.flat_byte_size(ti.word_bytes, ti.float_bytes),
        descriptor.flat_composite_kind(),
    ) && size <= u16::MAX as usize
    {
        return FlatFieldForm::Nested(size as u16, variant);
    }
    FlatFieldForm::NotFlat
}

/// Whether a layout has an `Opaque` scalar leaf, i.e. a field that, when the
/// composite is constructed flat, interns a host `Arc` into the ephemeral
/// opaque registry (B28 P3 item 5 registry tightening). Recurses through
/// every composite kind; an opaque inside a boxed sub-part never reaches
/// here because the caller only consults this on the flat construction path.
fn layout_has_opaque_leaf(d: &crate::value_layout::LayoutDescriptor) -> bool {
    use crate::value_layout::{LayoutDescriptor as L, ScalarKind};
    match d {
        L::Scalar(ScalarKind::Opaque) => true,
        L::Scalar(_) => false,
        L::Struct { fields, .. } => fields.iter().any(|(_, f)| layout_has_opaque_leaf(f)),
        L::Enum { variants, .. } => variants
            .iter()
            .any(|(_, ps)| ps.iter().any(layout_has_opaque_leaf)),
        L::Tuple(elems) => elems.iter().any(layout_has_opaque_leaf),
        L::Array { element, .. } => layout_has_opaque_leaf(element),
    }
}

/// Whether constructing a value of `ty` flat could intern a host opaque
/// (B28 P3 item 5 registry tightening). True when the type's flat layout
/// has an `Opaque` leaf, and conservatively true when the layout cannot be
/// computed, since an untypeable value (an unsignatured native result) could
/// be an opaque at runtime. Used to set [`FuncCompiler::may_intern_opaque`]
/// at flat `NewComposite` emission sites.
fn type_may_intern_opaque(ty: &TypeExpr, ti: &TypeInfo) -> bool {
    ti.layout_context()
        .layout_for(ty)
        .map(|d| layout_has_opaque_leaf(&d))
        .unwrap_or(true)
}

/// Whether a value-driven tuple or array element could intern an opaque at
/// runtime: true when its type cannot be recovered (an unsignatured native
/// result could be an opaque) or its type's flat layout has an `Opaque` leaf
/// (B28 P3 item 5 registry tightening).
fn elem_may_intern_opaque(fc: &FuncCompiler, elem: &Expr) -> bool {
    match infer_expr_type(fc, elem) {
        None => true,
        Some(ty) => type_may_intern_opaque(&ty, &fc.type_info),
    }
}

/// Resolve the baked [`ArrayElem`] operand for indexing an array whose
/// element type is `elem_ty` (B28 P2, references added in B28 P3 item 3). A
/// flat-eligible scalar element type (including the reference kinds `Opaque`
/// and `Text`) bakes `Flat { kind }` and a nested flat composite element
/// bakes `FlatNested { size, kind }`, matching the flat body the
/// construction handler builds for a transitively-flat array; any other
/// element type (float, non-flat composite, or `Text` on a narrow-word build
/// where `classify_flat_field` reports `NotFlat`) or an unrecoverable type
/// bakes the boxed positional form, matching the boxed body. The decision
/// mirrors the runtime's value-based eligibility in `array_with_widths`, so a
/// well-typed program's access form always agrees with the body.
fn array_elem_operand(elem_ty: Option<&TypeExpr>, ti: &TypeInfo) -> ArrayElem {
    match elem_ty.map(|ty| classify_flat_field(ty, ti)) {
        // A text element flattens to a two-word handle like a struct's text
        // field (B28 P3 item 5 C4); `classify_flat_field` already applies the
        // narrow-word gate, reporting `NotFlat` (so the array boxes) when the
        // word is narrower than a host pointer.
        Some(FlatFieldForm::Scalar(kind)) => ArrayElem::Flat { kind },
        Some(FlatFieldForm::Nested(size, variant)) => ArrayElem::FlatNested { size, variant },
        _ => ArrayElem::Boxed,
    }
}

/// Resolve the baked [`TupleField`] for accessing element `index` of a
/// tuple whose element types are `elem_types` (B28 P2).
///
/// Returns the `Flat` form carrying the packed little-endian byte
/// offset and the element's scalar kind when every element is
/// flat-eligible and the offset fits the sixteen-bit operand;
/// otherwise the `Boxed` positional form. The all-or-nothing
/// eligibility, the packed layout, and the target width source mirror
/// the VM's `pack_flat_tuple`, so the access form always matches the
/// body the construct handler builds. A `None` element type (the
/// compiler could not recover it) or unset target widths force the
/// boxed form.
fn tuple_field_access(
    fc: &FuncCompiler,
    elem_types: &[Option<TypeExpr>],
    index: usize,
) -> TupleField {
    let boxed = TupleField::Boxed { index: index as u8 };
    let wb = fc.type_info.word_bytes;
    if wb == 0 {
        return boxed;
    }
    let mut offset = 0usize;
    let mut field_at = None;
    for (i, ty) in elem_types.iter().enumerate() {
        let Some(ty) = ty else { return boxed };
        // Any non-flat element forces the whole tuple boxed; a nested flat
        // composite contributes its full size to following offsets (B28 P2).
        let Some(size) = type_flat_size(ty, &fc.type_info) else {
            return boxed;
        };
        // A text element flattens to a two-word handle like a struct's text
        // field (B28 P3 item 5 C4); `classify_flat_field` reports it as a
        // `Scalar(Text)` and applies the narrow-word gate (returning
        // `NotFlat`, so the tuple boxes, when the word is narrower than a
        // host pointer).
        if i == index {
            field_at = Some((offset, classify_flat_field(ty, &fc.type_info)));
        }
        offset += size;
    }
    if offset > u16::MAX as usize {
        return boxed;
    }
    match field_at {
        Some((off, FlatFieldForm::Scalar(kind))) if off <= u16::MAX as usize => TupleField::Flat {
            offset: off as u16,
            kind,
        },
        Some((off, FlatFieldForm::Nested(size, variant))) if off <= u16::MAX as usize => {
            TupleField::FlatNested {
                offset: off as u16,
                size,
                variant,
            }
        }
        _ => boxed,
    }
}

/// Decompose a tuple type into per-element types of the given arity,
/// unwrapping information-flow label wrappers (B28 P2). Yields a vector
/// of `None` when the type is absent or is not a matching tuple, which
/// `tuple_field_access` then resolves to the boxed form.
fn tuple_elem_types_of(ty: Option<&TypeExpr>, arity: usize) -> Vec<Option<TypeExpr>> {
    match ty {
        Some(TypeExpr::Tuple(ts, _)) if ts.len() == arity => ts.iter().cloned().map(Some).collect(),
        Some(TypeExpr::Labelled(inner, _, _)) | Some(TypeExpr::NegativeLabelled(inner, _, _)) => {
            tuple_elem_types_of(Some(inner), arity)
        }
        _ => core::iter::repeat_with(|| None).take(arity).collect(),
    }
}

/// Resolve the baked [`StructField`] for accessing `field_name` of a struct
/// of type `type_name` (B28 P2). Returns the `Flat` form carrying the
/// packed little-endian byte offset and the field's scalar kind when every
/// field of the struct is a flat-eligible scalar and the packed size fits
/// the sixteen-bit operand; otherwise the `Boxed` form carrying the
/// field-name constant index. The declaration-order layout, the
/// all-or-nothing eligibility, and the target width source mirror
/// `struct_with_widths`, so the access form always matches the body the
/// construct handler builds. An unknown struct, an unset target width, or
/// a non-flat field forces the boxed form.
fn struct_field_access(fc: &mut FuncCompiler, type_name: &str, field_name: &str) -> StructField {
    let wb = fc.type_info.word_bytes;
    // Clone the field order to release the borrow on `type_info`, so the
    // recursive size/classification helpers can borrow it again (B28 P2).
    let order = fc.type_info.struct_field_order.get(type_name).cloned();
    let flat = order.and_then(|order| {
        if wb == 0 {
            return None;
        }
        let mut offset = 0usize;
        let mut field_at = None;
        for (fname, ty) in &order {
            // Any non-flat field forces the whole struct boxed; a nested
            // flat composite contributes its full size (B28 P2).
            let size = type_flat_size(ty, &fc.type_info)?;
            if fname == field_name {
                field_at = Some((offset, classify_flat_field(ty, &fc.type_info)));
            }
            offset += size;
        }
        if offset > u16::MAX as usize {
            return None;
        }
        match field_at {
            Some((off, form)) if off <= u16::MAX as usize => Some((off as u16, form)),
            _ => None,
        }
    });
    match flat {
        Some((offset, FlatFieldForm::Scalar(kind))) => StructField::Flat { offset, kind },
        Some((offset, FlatFieldForm::Nested(size, variant))) => StructField::FlatNested {
            offset,
            size,
            variant,
        },
        _ => StructField::Boxed {
            name_const: fc.add_string_constant(field_name),
        },
    }
}

/// Strip information-flow label wrappers from a type expression, yielding
/// the underlying type (B28 P3 item 5). Used by the field-wise equality
/// emitter to recover the composite shape behind a `@Label` annotation.
fn strip_type_labels(ty: &TypeExpr) -> TypeExpr {
    match ty {
        TypeExpr::Labelled(inner, _, _) | TypeExpr::NegativeLabelled(inner, _, _) => {
            strip_type_labels(inner)
        }
        other => other.clone(),
    }
}

/// Whether equality of a value of type `ty` is compiled field-wise (B28 P3
/// item 5).
///
/// Every composite the compiler can name — a tuple, an array, or a declared
/// struct or enum — is compared field by field rather than by a raw-byte
/// `CmpEq` over its flat body. This is required for a float-bearing composite
/// (a byte-blob compare diverges from IEEE on `+0.0`/`-0.0` and `NaN`) and is
/// applied uniformly to all composites so that a flat composite body never
/// reaches the runtime `CmpEq`: the VM `CmpEq`/`CmpNe` handlers trap on a flat
/// composite operand, which then catches exactly the case the compiler could
/// not name (an unsignatured native's composite result) as a clear fault
/// rather than a silent byte-blob comparison. The per-field extraction works
/// on both flat and boxed bodies.
///
/// `Option`, scalars, and any type the compiler cannot resolve to a tabled
/// struct or enum fall through to `CmpEq`: `Option` and other boxed composites
/// compare correctly with the derived comparison, scalars compare directly,
/// and an unresolved composite traps in the VM (see above) rather than risk a
/// byte-blob compare. The dispatch keys on the declared type tables rather
/// than the layout descriptor so it never selects a composite the field-wise
/// emitter cannot build accessors for (notably the built-in `Option`).
fn composite_needs_fieldwise_eq(ty: &TypeExpr, ti: &TypeInfo) -> bool {
    match strip_type_labels(ty) {
        TypeExpr::Tuple(_, _) | TypeExpr::Array(_, _, _) => true,
        // `Option<T>` flattens its `Some` payload (B28 P3 item 5 C4), so two
        // `Some` bodies must be compared field-wise rather than by raw bytes.
        TypeExpr::Option(_, _) => true,
        TypeExpr::Named(name, _, _, _) => {
            ti.struct_field_order.contains_key(&name) || ti.enum_variant_order.contains_key(&name)
        }
        _ => false,
    }
}

/// A baked field-access operand paired with the local slot the access
/// reads from, used by the field-wise equality emitter (B28 P3 item 5).
enum FieldAccessOp {
    Struct(crate::bytecode::StructField),
    Tuple(crate::bytecode::TupleField),
    Array {
        elem: crate::bytecode::ArrayElem,
        index_const: u16,
    },
}

/// Emit the ops that push the addressed field of the composite in local
/// `slot` onto the stack (B28 P3 item 5). Mirrors the access the compiler
/// bakes for an ordinary field read, so the offsets agree with the
/// constructed body (flat or boxed).
fn emit_field_extract(fc: &mut FuncCompiler, slot: u16, access: &FieldAccessOp) {
    fc.emit(Op::GetLocal(slot));
    match access {
        FieldAccessOp::Struct(f) => {
            fc.emit(Op::GetField(*f));
        }
        FieldAccessOp::Tuple(f) => {
            fc.emit(Op::GetTupleField(*f));
        }
        FieldAccessOp::Array { elem, index_const } => {
            // `GetIndex` pops the index then the container, so push the
            // container (via `GetLocal` above) then the index constant.
            fc.emit(Op::Const(*index_const));
            fc.emit(Op::GetIndex(*elem));
        }
    }
}

/// Resolve the ordered `(field access, field type)` list for the composite
/// `ty` (B28 P3 item 5). Structs follow declaration order; tuples and arrays
/// follow positional order. The access operands reuse the same baking the
/// ordinary field reads use, so the offsets match the constructed body.
fn composite_field_accessors(
    fc: &mut FuncCompiler,
    ty: &TypeExpr,
) -> Result<Vec<(FieldAccessOp, TypeExpr)>, CompileError> {
    match strip_type_labels(ty) {
        TypeExpr::Named(name, _, _, span) => {
            let order = fc
                .type_info
                .struct_field_order
                .get(&name)
                .cloned()
                .ok_or_else(|| CompileError {
                    message: alloc::format!(
                        "field-wise equality requires a struct layout for `{}`",
                        name
                    ),
                    span,
                })?;
            let mut out = Vec::with_capacity(order.len());
            for (fname, fty) in &order {
                let access = FieldAccessOp::Struct(struct_field_access(fc, &name, fname));
                out.push((access, fty.clone()));
            }
            Ok(out)
        }
        TypeExpr::Tuple(elems, _) => {
            let opt_types: Vec<Option<TypeExpr>> = elems.iter().cloned().map(Some).collect();
            let mut out = Vec::with_capacity(elems.len());
            for (i, ety) in elems.iter().enumerate() {
                let access = FieldAccessOp::Tuple(tuple_field_access(fc, &opt_types, i));
                out.push((access, ety.clone()));
            }
            Ok(out)
        }
        TypeExpr::Array(elem, count, span) => {
            let count = count.as_lit().ok_or_else(|| CompileError {
                message: alloc::format!("array size `{}` is not a resolved constant", count),
                span,
            })?;
            if count < 0 {
                return Err(CompileError {
                    message: String::from("field-wise equality on a negative-length array"),
                    span,
                });
            }
            let mut out = Vec::with_capacity(count as usize);
            for i in 0..count {
                let elem_op = array_elem_operand(Some(&elem), &fc.type_info);
                let index_const = fc.add_constant(Value::Int(i));
                out.push((
                    FieldAccessOp::Array {
                        elem: elem_op,
                        index_const,
                    },
                    (*elem).clone(),
                ));
            }
            Ok(out)
        }
        other => Err(CompileError {
            message: alloc::format!(
                "field-wise equality on an unsupported composite type {:?}",
                other
            ),
            span: Span::default(),
        }),
    }
}

/// Emit a field-wise equality comparison of the two composites held in
/// locals `ltmp` and `rtmp`, leaving a single `Bool` on the stack (B28 P3
/// item 5, Phase A).
///
/// Each field is extracted from both operands and compared by kind: a scalar
/// field (including `Float`, which `CmpEq` compares with IEEE semantics on
/// the extracted value) by `CmpEq`, a float-bearing nested composite field
/// by recursion, and any other nested composite by `CmpEq` (its body carries
/// no float, so the comparison is correct). The fields are combined with a
/// short-circuiting logical AND expressed as a virtual loop: the first
/// unequal field breaks out with `false`, and reaching the end pushes `true`.
/// This reuses the `Loop`/`Break` idiom of [`compile_enum_to_word`] and needs
/// no new opcode. Termination is static: the field list is finite and
/// recursion follows the finite, non-recursive composite type structure.
fn emit_composite_fieldwise_eq(
    fc: &mut FuncCompiler,
    ty: &TypeExpr,
    ltmp: u16,
    rtmp: u16,
) -> Result<(), CompileError> {
    // `Option<T>` is the built-in generic enum: it is hybrid (scalar `None`,
    // flat `Some`) and untabled, so it has its own emitter (B28 P3 item 5 C4).
    if matches!(strip_type_labels(ty), TypeExpr::Option(_, _)) {
        return emit_option_fieldwise_eq(fc, ty, ltmp, rtmp);
    }
    // An enum is compared by variant dispatch, not the uniform field loop:
    // the active variant selects which payload fields to compare.
    if let TypeExpr::Named(name, _, _, _) = &strip_type_labels(ty)
        && fc.type_info.enum_variant_order.contains_key(name)
    {
        return emit_enum_fieldwise_eq(fc, name, ltmp, rtmp);
    }
    let fields = composite_field_accessors(fc, ty)?;
    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();
    for (access, fty) in fields {
        fc.begin_scope();
        emit_field_extract(fc, ltmp, &access);
        emit_field_extract(fc, rtmp, &access);
        if composite_needs_fieldwise_eq(&fty, &fc.type_info) {
            let r2 = fc.declare_local("__eqf_r");
            let l2 = fc.declare_local("__eqf_l");
            fc.emit(Op::SetLocal(r2));
            fc.emit(Op::SetLocal(l2));
            emit_composite_fieldwise_eq(fc, &fty, l2, r2)?;
        } else {
            fc.emit(Op::CmpEq);
        }
        // Stack: a `Bool` that is true when this field is equal. Break out
        // with `false` when it is unequal; otherwise continue to the next
        // field. `If` jumps on false, so compare `Not` (unequal) and let the
        // unequal case fall through to the break.
        fc.emit(Op::Not);
        let skip = fc.emit_jump(Op::If(0));
        let f_const = fc.add_constant(Value::Bool(false));
        fc.emit(Op::Const(f_const));
        let br = fc.emit_jump(Op::Break(0));
        fc.loop_breaks
            .last_mut()
            .expect("inside field-wise eq loop")
            .push(br);
        fc.patch_jump(skip);
        fc.emit(Op::EndIf);
        fc.end_scope();
    }
    // Every field compared equal.
    let t_const = fc.add_constant(Value::Bool(true));
    fc.emit(Op::Const(t_const));
    let br = fc.emit_jump(Op::Break(0));
    fc.loop_breaks
        .last_mut()
        .expect("inside field-wise eq loop")
        .push(br);
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

/// Emit a variant-dispatched field-wise equality of the two enums held in
/// locals `ltmp` and `rtmp`, leaving a single `Bool` on the stack (B28 P3
/// item 5, Phase A).
///
/// Two enums are equal iff they are the same variant and that variant's
/// payload fields are field-wise equal. For each declared variant `V`: if the
/// left value is `V`, then if the right value is also `V` compare `V`'s
/// payload fields (short-circuiting), otherwise the values differ; if the left
/// value is not `V`, try the next variant. The dispatch reuses the `IsEnum`
/// check and `GetEnumField` extraction the compiler already bakes, so it is
/// correct on both flat and boxed enum bodies, and the whole comparison is
/// wrapped in the `Loop`/`Break` virtual block (the [`compile_enum_to_word`]
/// idiom) so each outcome breaks out with its `Bool`.
///
/// `IsEnum` peeks (it leaves the inspected enum on the stack and pushes the
/// Bool), so each `GetLocal`/`IsEnum`/`If` is followed by a `PopN(1)` that
/// discards the peeked copy on the path that keeps it. A well-typed value
/// always matches one declared variant; the post-loop `Trap` guards a
/// host-built enum carrying an undeclared variant.
/// Emit field-wise equality of two `Option<T>` values in `ltmp`/`rtmp`,
/// leaving a single `Bool` on the stack (B28 P3 item 5 C4).
///
/// `Option` is hybrid at runtime: `None` is the scalar `Value::None` (also
/// what host natives return) and `Some(x)` is a flat enum `[disc=1][x]`. The
/// only case the derived `PartialEq` mishandles is two `Some` bodies, where a
/// raw-byte compare is IEEE-unsafe for a float payload, so this special-cases
/// it: if both values are `Some`, compare the extracted payloads field-wise;
/// otherwise fall back to `CmpEq`, which is correct and fault-free for any
/// pairing with at least one `None` (the relaxed `reject_untyped_flat_
/// composite_cmp` does not fire unless both operands are flat). Reuses the
/// `Loop`/`Break` idiom and the `IsEnum`-peek/`PopN` discipline of
/// [`emit_enum_fieldwise_eq`]. `ty` is the static `Option<T>` type, the source
/// of the payload type `T` since `Option` is generic and untabled.
fn emit_option_fieldwise_eq(
    fc: &mut FuncCompiler,
    ty: &TypeExpr,
    ltmp: u16,
    rtmp: u16,
) -> Result<(), CompileError> {
    let e_const = fc.add_string_constant("Option");
    let v_const = fc.add_string_constant("Some");
    let some_disc = fc.add_constant(Value::Int(1));
    let opt_ty = Some(strip_type_labels(ty));
    let payload_ty = option_inner(opt_ty.as_ref());
    let some_field = option_some_field(fc, opt_ty.as_ref(), 0);

    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();
    fc.begin_scope();

    // Is the left value `Some`? (`IsEnum` is false for `Value::None`.)
    fc.emit(Op::GetLocal(ltmp));
    fc.emit(Op::IsEnum(e_const, v_const, some_disc));
    let a_not_some = fc.emit_jump(Op::If(0));
    fc.emit(Op::PopN(1)); // left is Some; discard the peeked copy
    // Is the right value also `Some`?
    fc.emit(Op::GetLocal(rtmp));
    fc.emit(Op::IsEnum(e_const, v_const, some_disc));
    let b_not_some = fc.emit_jump(Op::If(0));
    fc.emit(Op::PopN(1)); // both Some; discard the peeked copy
    // Both Some: compare the payloads.
    fc.emit(Op::GetLocal(ltmp));
    fc.emit(Op::GetEnumField(some_field));
    fc.emit(Op::GetLocal(rtmp));
    fc.emit(Op::GetEnumField(some_field));
    match &payload_ty {
        Some(t) if composite_needs_fieldwise_eq(t, &fc.type_info) => {
            let r2 = fc.declare_local("__eqo_r");
            let l2 = fc.declare_local("__eqo_l");
            fc.emit(Op::SetLocal(r2));
            fc.emit(Op::SetLocal(l2));
            emit_composite_fieldwise_eq(fc, t, l2, r2)?;
        }
        _ => {
            fc.emit(Op::CmpEq);
        }
    }
    let br_both = fc.emit_jump(Op::Break(0));
    fc.loop_breaks
        .last_mut()
        .expect("inside option eq loop")
        .push(br_both);
    // Left Some, right not Some: fall back to `CmpEq` (yields false).
    fc.patch_jump(b_not_some);
    fc.emit(Op::EndIf);
    fc.emit(Op::PopN(1)); // discard the peeked right value
    fc.emit(Op::GetLocal(ltmp));
    fc.emit(Op::GetLocal(rtmp));
    fc.emit(Op::CmpEq);
    let br_anb = fc.emit_jump(Op::Break(0));
    fc.loop_breaks
        .last_mut()
        .expect("inside option eq loop")
        .push(br_anb);
    // Left not Some (it is `None`): `CmpEq` handles None==None and None==Some.
    fc.patch_jump(a_not_some);
    fc.emit(Op::EndIf);
    fc.emit(Op::PopN(1)); // discard the peeked left value
    fc.emit(Op::GetLocal(ltmp));
    fc.emit(Op::GetLocal(rtmp));
    fc.emit(Op::CmpEq);
    let br_an = fc.emit_jump(Op::Break(0));
    fc.loop_breaks
        .last_mut()
        .expect("inside option eq loop")
        .push(br_an);

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

fn emit_enum_fieldwise_eq(
    fc: &mut FuncCompiler,
    enum_name: &str,
    ltmp: u16,
    rtmp: u16,
) -> Result<(), CompileError> {
    let variants: Vec<(String, i64)> = fc
        .type_info
        .enum_variant_order
        .get(enum_name)
        .cloned()
        .unwrap_or_default();
    let e_const = fc.add_string_constant(enum_name);
    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();
    for (variant, disc) in &variants {
        fc.begin_scope();
        let v_const = fc.add_string_constant(variant);
        let d_const = fc.add_constant(Value::Int(*disc));
        // Is the left value variant `V`? `IsEnum` peeks, so discard the peeked
        // copy on each path below.
        fc.emit(Op::GetLocal(ltmp));
        fc.emit(Op::IsEnum(e_const, v_const, d_const));
        let l_not_v = fc.emit_jump(Op::If(0)); // false (not V) -> skip this variant
        fc.emit(Op::PopN(1)); // discard the peeked left enum (left is V)
        // Is the right value also `V`?
        fc.emit(Op::GetLocal(rtmp));
        fc.emit(Op::IsEnum(e_const, v_const, d_const));
        let r_not_v = fc.emit_jump(Op::If(0)); // false (not V) -> different variants
        fc.emit(Op::PopN(1)); // discard the peeked right enum (both are V)
        // Both are `V`: compare the payload fields field-wise.
        let payload: Vec<TypeExpr> = fc
            .type_info
            .enums
            .get(enum_name)
            .and_then(|vs| vs.get(variant))
            .cloned()
            .unwrap_or_default();
        for (i, fty) in payload.iter().enumerate() {
            let access = enum_field_access(fc, enum_name, variant, i);
            fc.emit(Op::GetLocal(ltmp));
            fc.emit(Op::GetEnumField(access));
            fc.emit(Op::GetLocal(rtmp));
            fc.emit(Op::GetEnumField(access));
            if composite_needs_fieldwise_eq(fty, &fc.type_info) {
                let r2 = fc.declare_local("__eqe_r");
                let l2 = fc.declare_local("__eqe_l");
                fc.emit(Op::SetLocal(r2));
                fc.emit(Op::SetLocal(l2));
                emit_composite_fieldwise_eq(fc, fty, l2, r2)?;
            } else {
                fc.emit(Op::CmpEq);
            }
            // A `Bool` that is true when this payload field is equal. Break
            // with `false` when it is unequal; otherwise continue.
            fc.emit(Op::Not);
            let field_eq = fc.emit_jump(Op::If(0));
            let f_const = fc.add_constant(Value::Bool(false));
            fc.emit(Op::Const(f_const));
            let br = fc.emit_jump(Op::Break(0));
            fc.loop_breaks
                .last_mut()
                .expect("inside enum eq loop")
                .push(br);
            fc.patch_jump(field_eq);
            fc.emit(Op::EndIf);
        }
        // Every payload field compared equal: the enums are equal.
        let t_const = fc.add_constant(Value::Bool(true));
        fc.emit(Op::Const(t_const));
        let br = fc.emit_jump(Op::Break(0));
        fc.loop_breaks
            .last_mut()
            .expect("inside enum eq loop")
            .push(br);
        // Right is not `V` while left is `V`: different variants, unequal.
        fc.patch_jump(r_not_v);
        fc.emit(Op::EndIf);
        fc.emit(Op::PopN(1)); // discard the peeked right enum
        let f_const = fc.add_constant(Value::Bool(false));
        fc.emit(Op::Const(f_const));
        let br = fc.emit_jump(Op::Break(0));
        fc.loop_breaks
            .last_mut()
            .expect("inside enum eq loop")
            .push(br);
        // Left is not `V`: discard the peeked left enum and try the next
        // variant.
        fc.patch_jump(l_not_v);
        fc.emit(Op::EndIf);
        fc.emit(Op::PopN(1));
        fc.end_scope();
    }
    // A well-typed value matched one variant; reaching here means a
    // host-constructed enum carried an undeclared variant.
    fc.emit(Op::Trap(
        crate::bytecode::TrapKind::EnumVariantUnmapped.code(),
    ));
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

/// The name of a named (struct, enum, or opaque) type expression,
/// unwrapping information-flow label wrappers. `None` for any other type.
fn named_type_name(ty: Option<&TypeExpr>) -> Option<&str> {
    match ty {
        Some(TypeExpr::Named(name, _, _, _)) => Some(name),
        Some(TypeExpr::Labelled(inner, _, _)) | Some(TypeExpr::NegativeLabelled(inner, _, _)) => {
            named_type_name(Some(inner))
        }
        _ => None,
    }
}

/// Resolve the baked [`EnumField`] for accessing payload field `index` of
/// `variant` of enum `type_name` (B28 P2). Returns the `Flat` form carrying
/// the packed byte offset (past the leading discriminant word) and the
/// field kind when the variant's whole payload is flat-eligible scalars and
/// the offset fits the sixteen-bit operand; otherwise the boxed positional
/// form. Mirrors `enum_with_widths`, so the access form matches the body.
/// The payload type `T` of an `Option<T>` scrutinee, or `None` if `ty` is not
/// an `Option` (B28 P3 item 5 C4).
fn option_inner(ty: Option<&TypeExpr>) -> Option<TypeExpr> {
    match ty {
        Some(TypeExpr::Option(inner, _)) => Some((**inner).clone()),
        _ => None,
    }
}

/// The baked [`EnumField`] for extracting `Option::Some`'s single payload
/// (B28 P3 item 5 C4). A flat `Option<T>` body is `[disc word][T]`, so the
/// payload sits at offset `word_bytes`; the kind comes from `T`, recovered
/// from the scrutinee type since `Option` is generic and absent from the type
/// tables. A non-flat `T` (or an unrecoverable scrutinee type) yields the
/// boxed form, which agrees with the boxed construction fallback.
fn option_some_field(fc: &FuncCompiler, ty: Option<&TypeExpr>, index: usize) -> EnumField {
    let boxed = EnumField::Boxed { index: index as u8 };
    let wb = fc.type_info.word_bytes;
    if wb == 0 || index != 0 || wb > u16::MAX as usize {
        return boxed;
    }
    let Some(t) = option_inner(ty) else {
        return boxed;
    };
    match classify_flat_field(&t, &fc.type_info) {
        FlatFieldForm::Scalar(kind) => EnumField::Flat {
            offset: wb as u16,
            kind,
        },
        FlatFieldForm::Nested(size, variant) => EnumField::FlatNested {
            offset: wb as u16,
            size,
            variant,
        },
        FlatFieldForm::NotFlat => boxed,
    }
}

fn enum_field_access(
    fc: &mut FuncCompiler,
    type_name: &str,
    variant: &str,
    index: usize,
) -> EnumField {
    let wb = fc.type_info.word_bytes;
    // Clone the payload types to release the borrow on `type_info` (B28 P2).
    let payload = fc
        .type_info
        .enums
        .get(type_name)
        .and_then(|vs| vs.get(variant))
        .cloned();
    let flat = payload.and_then(|payload| {
        if wb == 0 {
            return None;
        }
        // The flat enum body begins with the discriminant word.
        let mut offset = wb;
        let mut field_at = None;
        for (i, ty) in payload.iter().enumerate() {
            // A non-flat payload field forces the boxed body; a nested flat
            // composite payload field contributes its full size (B28 P2).
            let size = type_flat_size(ty, &fc.type_info)?;
            if i == index {
                field_at = Some((offset, classify_flat_field(ty, &fc.type_info)));
            }
            offset += size;
        }
        if offset > u16::MAX as usize {
            return None;
        }
        match field_at {
            Some((off, form)) if off <= u16::MAX as usize => Some((off as u16, form)),
            _ => None,
        }
    });
    match flat {
        Some((offset, FlatFieldForm::Scalar(kind))) => EnumField::Flat { offset, kind },
        Some((offset, FlatFieldForm::Nested(size, variant))) => EnumField::FlatNested {
            offset,
            size,
            variant,
        },
        _ => EnumField::Boxed { index: index as u8 },
    }
}

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
                fc.emit(Op::GetTupleField(tuple_field_access(fc, &elem_types, i)));
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
            if let Expr::FieldAccess { object, field, .. } = expr.as_ref()
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
                    let len = len.as_lit().unwrap_or(0);
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
            fc.emit(Op::GetIndex(array_elem_operand(
                element_ty.as_ref(),
                &fc.type_info,
            )));
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
            TypeExpr::Prim(_, _) | TypeExpr::Unit(_) | TypeExpr::Multiword(_, _, _) => {}
            TypeExpr::Tuple(parts, _) => {
                for p in parts.iter_mut() {
                    fix_type(p, frac_bits);
                }
            }
            TypeExpr::Array(elem, _, _) => fix_type(elem, frac_bits),
            TypeExpr::Option(inner, _) => fix_type(inner, frac_bits),
            TypeExpr::Named(_, args, _, _) => {
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
            Stmt::Assert { cond, .. } => fix_expr(cond, frac_bits),
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
        let d_const = fc.add_constant(Value::Int(*discriminant));
        fc.emit(Op::GetLocal(temp));
        fc.emit(Op::IsEnum(e_const, v_const, d_const));
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
                // The eager and short-circuit operators fold to the same
                // value on constant operands; the difference is only in
                // runtime evaluation order, which is moot for constants.
                (
                    crate::ast::BinOp::And | crate::ast::BinOp::Andalso,
                    EvalValue::Bool(a),
                    EvalValue::Bool(b),
                ) => Some(EvalValue::Bool(a && b)),
                (
                    crate::ast::BinOp::Or | crate::ast::BinOp::Orelse,
                    EvalValue::Bool(a),
                    EvalValue::Bool(b),
                ) => Some(EvalValue::Bool(a || b)),
                (crate::ast::BinOp::Xor, EvalValue::Bool(a), EvalValue::Bool(b)) => {
                    Some(EvalValue::Bool(a != b))
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
            } else if matches!(op, crate::ast::BinOp::And | crate::ast::BinOp::Andalso) {
                let l = predicate_true_set(left, param)?;
                let r = predicate_true_set(right, param)?;
                Some(l.intersect(&r))
            } else if matches!(op, crate::ast::BinOp::Or | crate::ast::BinOp::Orelse) {
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
    if let Some(crate::ast::TypeExpr::Named(type_name, _, _, _)) = t
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
            op,
            left,
            right,
            span,
        } => {
            // The boolean operator family. `andalso` / `orelse` are the
            // short-circuit control forms: the right operand is skipped
            // when the left already decides the result. `and` / `or` /
            // `xor` are eager: both operands are always evaluated, which
            // is more predictable for worst-case-execution-time analysis
            // (no data-dependent branch) and lets a native side effect on
            // the right run unconditionally. The eager forms evaluate the
            // left into a scratch local and the right onto the stack, then
            // branch only to select, so no operand is evaluated twice or
            // skipped.
            match op {
                BinOp::Andalso => {
                    // a andalso b: if a is false, result is false (b is
                    // skipped); else result is b.
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
                BinOp::Orelse => {
                    // a orelse b: if a is true, result is true (b is
                    // skipped); else result is b.
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
                BinOp::And => {
                    // Eager and: evaluate both, then select b when a is
                    // true and false otherwise.
                    let la = fc.declare_local("__and_l");
                    compile_expr(fc, left)?;
                    fc.emit(Op::SetLocal(la));
                    compile_expr(fc, right)?; // stack: [b]
                    fc.emit(Op::GetLocal(la)); // stack: [b, a]
                    let if_addr = fc.emit_jump(Op::If(0));
                    // a true: result is b, already on the stack.
                    let else_addr = fc.emit_jump(Op::Else(0));
                    fc.patch_jump(if_addr);
                    // a false: discard b, push false.
                    fc.emit(Op::PopN(1));
                    let false_c = fc.add_constant(Value::Bool(false));
                    fc.emit(Op::Const(false_c));
                    fc.patch_jump(else_addr);
                    fc.emit(Op::EndIf);
                    return Ok(());
                }
                BinOp::Or => {
                    // Eager or: evaluate both, then select b when a is
                    // false and true otherwise.
                    let la = fc.declare_local("__or_l");
                    compile_expr(fc, left)?;
                    fc.emit(Op::SetLocal(la));
                    compile_expr(fc, right)?; // stack: [b]
                    fc.emit(Op::GetLocal(la)); // stack: [b, a]
                    fc.emit(Op::Not); // stack: [b, !a]
                    let if_addr = fc.emit_jump(Op::If(0));
                    // !a true (a false): result is b, already on the stack.
                    let else_addr = fc.emit_jump(Op::Else(0));
                    fc.patch_jump(if_addr);
                    // a true: discard b, push true.
                    fc.emit(Op::PopN(1));
                    let true_c = fc.add_constant(Value::Bool(true));
                    fc.emit(Op::Const(true_c));
                    fc.patch_jump(else_addr);
                    fc.emit(Op::EndIf);
                    return Ok(());
                }
                BinOp::Xor => {
                    // Eager exclusive-or of two booleans is inequality;
                    // both operands are evaluated with no branch.
                    compile_expr(fc, left)?;
                    compile_expr(fc, right)?;
                    fc.emit(Op::CmpNe);
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
            // Multiword<N> operators lower to unrolled per-limb cascades
            // over the existing checked-word and bitwise opcodes (B19).
            if let Some((n, f)) = operand_ty.as_ref().and_then(|t| t.as_multiword_lit()) {
                return compile_multiword_binop(fc, *op, left, right, n, f);
            }
            // Scalar `Word`/`Byte` shifts, constant or variable amount. The
            // Multiword case is handled above through compile_multiword_binop.
            if matches!(op, BinOp::Shl | BinOp::AShl | BinOp::ShrA | BinOp::ShrL) {
                return compile_scalar_shift(fc, *op, left, right);
            }
            // Byte bitwise operations promote each operand to `Word`,
            // combine, and truncate back to `Byte`; the generic scalar
            // lowering below assumes `Word` operands.
            if matches!(op, BinOp::Band | BinOp::Bor | BinOp::Bxor)
                && matches!(&operand_ty, Some(TypeExpr::Prim(PrimType::Byte, _)))
            {
                return compile_byte_bitwise(fc, *op, left, right);
            }
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
                    // Division can trap on a zero divisor; record the
                    // operator site so the fault resolves exactly here
                    // (B29 items 2 and 3).
                    fc.record_operator_site(span);
                }
                BinOp::Mod => {
                    fc.emit(Op::Mod);
                    // Modulo by zero traps; record the operator site.
                    fc.record_operator_site(span);
                }
                BinOp::Eq | BinOp::NotEq => {
                    // A float-bearing composite must compare field-wise so the
                    // IEEE `+0.0`/`-0.0` and `NaN` semantics survive a flat
                    // body (B28 P3 item 5); a raw-byte `CmpEq` would diverge.
                    // The operands are already on the stack, so stash them in
                    // scratch locals, emit the per-field comparison, then
                    // negate for `!=`. Non-float composites and scalars keep
                    // the direct compare.
                    let fieldwise = operand_ty
                        .as_ref()
                        .map(|ty| composite_needs_fieldwise_eq(ty, &fc.type_info))
                        .unwrap_or(false);
                    if fieldwise {
                        let ty = operand_ty.clone().expect("fieldwise implies Some");
                        fc.begin_scope();
                        let rtmp = fc.declare_local("__eq_r");
                        let ltmp = fc.declare_local("__eq_l");
                        fc.emit(Op::SetLocal(rtmp));
                        fc.emit(Op::SetLocal(ltmp));
                        emit_composite_fieldwise_eq(fc, &ty, ltmp, rtmp)?;
                        if matches!(op, BinOp::NotEq) {
                            fc.emit(Op::Not);
                        }
                        fc.end_scope();
                    } else if matches!(op, BinOp::Eq) {
                        fc.emit(Op::CmpEq);
                    } else {
                        fc.emit(Op::CmpNe);
                    }
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
                BinOp::And | BinOp::Or | BinOp::Xor | BinOp::Andalso | BinOp::Orelse => {
                    unreachable!("boolean operators are lowered above with an early return")
                }
                // Scalar bitwise operations lower directly to the bitwise
                // opcodes; the Multiword case routes to compile_multiword_binop.
                BinOp::Band => {
                    fc.emit(Op::BitAnd);
                }
                BinOp::Bor => {
                    fc.emit(Op::BitOr);
                }
                BinOp::Bxor => {
                    fc.emit(Op::BitXor);
                }
                BinOp::Shl | BinOp::AShl | BinOp::ShrA | BinOp::ShrL => {
                    unreachable!(
                        "shifts are dispatched to compile_scalar_shift / compile_multiword_shift"
                    )
                }
            }
        }

        Expr::UnaryOp { op, operand, .. } => {
            // A bitwise complement of a `Multiword<N>` lowers to a
            // per-limb complement, mirroring the Multiword binary path.
            if matches!(op, UnaryOp::Bnot)
                && let Some((n, _)) =
                    infer_expr_type(fc, operand).and_then(|t| t.as_multiword_lit())
            {
                return compile_multiword_bnot(fc, operand, n);
            }
            // Mirrors the binary-op type-specialization from
            // Consolidation B: operands inferred or defaulted to
            // `Int` route through `CheckedNeg` followed by
            // `PopN(2)` so the unchecked negate opcode does not
            // need an `Int` arm in the VM dispatch. Operands whose
            // type is explicitly `Byte`, `Fixed`, or `Float`
            // continue to use `Op::Neg` whose VM dispatch retains
            // those three arms. Unknown-type operands default to
            // `Int` for the same reason as the binary path.
            let inferred = infer_expr_type(fc, operand);
            let operand_is_int = matches!(inferred, None | Some(TypeExpr::Prim(PrimType::Word, _)));
            let operand_is_byte = matches!(inferred, Some(TypeExpr::Prim(PrimType::Byte, _)));
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
                UnaryOp::Bnot => {
                    // Scalar bitwise complement is XOR against the
                    // all-ones word; the Multiword case is dispatched
                    // above before the operand is compiled. A `Byte` is
                    // promoted to `Word`, complemented, and truncated back,
                    // so the complement is taken within the byte width.
                    if operand_is_byte {
                        fc.emit(Op::ByteToWord);
                    }
                    let neg1 = fc.add_constant(Value::Int(-1));
                    fc.emit(Op::Const(neg1));
                    fc.emit(Op::BitXor);
                    if operand_is_byte {
                        fc.emit(Op::WordToByte);
                    }
                }
            }
        }

        Expr::Call {
            name, args, span, ..
        } => {
            compile_call(fc, name, args, span)?;
            fc.record_call_site_last_op(span);
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
            fc.record_call_site_last_op(span);
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
            fc.record_call_site_last_op(span);
        }

        Expr::Yield { value, .. } => {
            // A flat (struct/enum) Text field may cross the yield boundary
            // under the read-before-resume contract (B28 P3 item 4). The host
            // decodes a yielded composite before the next `resume()`, which is
            // the RESET point, so the text is read while it is still valid. A
            // static (rodata) field is immortal and reads correctly even after
            // a RESET; a dynamic (ephemeral) field is valid in-iteration, and a
            // contract-violating read after the RESET resolves to a clean stale
            // fault through the epoch backstop rather than a dangling read. The
            // earlier compile-time rejection of flat-text composites is
            // therefore lifted. The runtime `contains_dynstr` walk still
            // governs bare and boxed (tuple/array/Option) dynamic strings,
            // whose lifecycle differs; it is blind to flat bytes by design, so
            // a flat-text composite passes it.
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
            // The scrutinee's type lets the pattern test and bind bake
            // flat tuple-field access when the matched tuple is scalar
            // (B28 P2); an un-inferable scrutinee falls back to boxed.
            let scrutinee_ty = infer_expr_type(fc, scrutinee);
            compile_expr(fc, scrutinee)?;
            let temp = fc.declare_local("__match");
            fc.emit(Op::SetLocal(temp));

            // Wrap match in a virtual Loop so arms can Break to exit.
            let loop_addr = fc.emit(Op::Loop(0));
            fc.enter_loop();

            for arm in arms {
                fc.begin_scope();

                let mut fail_addrs =
                    compile_pattern_test(fc, &arm.pattern, temp, scrutinee_ty.as_ref())?;
                compile_pattern_bind_typed(fc, &arm.pattern, temp, scrutinee_ty.clone())?;
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
            // Bake the flat field access when the object's struct type is
            // recoverable and all its fields are flat scalars; otherwise
            // the boxed by-name form (B28 P2).
            let obj_ty = infer_expr_type(fc, object);
            compile_expr(fc, object)?;
            let field_op = match named_type_name(obj_ty.as_ref()) {
                Some(tn) => struct_field_access(fc, tn, field),
                None => StructField::Boxed {
                    name_const: fc.add_string_constant(field),
                },
            };
            fc.emit(Op::GetField(field_op));
        }

        Expr::TupleIndex { object, index, .. } => {
            // Recover the tuple's element types to bake a flat access
            // when eligible; an un-inferable object stays boxed (B28 P2).
            let elem_types: Vec<Option<TypeExpr>> = match infer_expr_type(fc, object) {
                Some(TypeExpr::Tuple(ts, _)) => ts.into_iter().map(Some).collect(),
                _ => Vec::new(),
            };
            compile_expr(fc, object)?;
            fc.emit(Op::GetTupleField(tuple_field_access(
                fc,
                &elem_types,
                *index as usize,
            )));
        }

        Expr::ArrayIndex {
            object,
            index,
            span,
        } => {
            // Detect indexed access against a data-segment field and
            // emit `Op::GetDataIndexed` plus the per-level
            // `Op::BoundsCheck` / stride arithmetic. Stack-resident
            // arrays continue to use `Op::GetIndex`. The chain is a
            // data access only when its base identifier is actually a
            // data block; a `local.field[i]` where `local` is a struct
            // or enum value (its field an array) falls through to the
            // general `GetIndex` path below, the same lowering as
            // binding the field to a local first and indexing that.
            if let Some(chain) = data_indexed_chain(object, index)
                && fc.is_data_block(chain.data_name)
            {
                emit_data_indexed_read(fc, chain, *span)?;
                return Ok(());
            }
            let elem_ty = infer_expr_type(fc, object).and_then(|t| element_type_of(&t));
            compile_expr(fc, object)?;
            compile_expr(fc, index)?;
            fc.emit(Op::GetIndex(array_elem_operand(
                elem_ty.as_ref(),
                &fc.type_info,
            )));
            // Indexing can trap on an out-of-bounds index; record the
            // operator site so the fault resolves exactly (B29 item 2).
            fc.record_operator_site(span);
        }

        Expr::StructInit { name, fields, .. } => {
            // Pack fields in declaration order so the flat-byte layout
            // (B28 P2) is canonical regardless of the literal field order,
            // which the type checker admits in any order. The struct
            // template and the value pushes therefore both follow the
            // declared order, the order `GetField` bakes offsets against.
            // An unknown struct (rejected earlier by the type checker)
            // falls back to literal order.
            let decl_order = fc.type_info.struct_field_order.get(name).cloned();
            let ordered: Vec<_> = match &decl_order {
                Some(order) => order
                    .iter()
                    .map(|(fname, _)| {
                        fields
                            .iter()
                            .find(|f| &f.name == fname)
                            .expect("type checker guarantees the field is present")
                    })
                    .collect(),
                None => fields.iter().collect(),
            };
            let field_names: Vec<String> = ordered.iter().map(|f| f.name.clone()).collect();
            // The flat allocation size comes from the struct type; a
            // non-flat (reference-bearing) struct bakes the boxed form with
            // a template for by-name access (B28 P4).
            let ty = TypeExpr::Named(name.clone(), Vec::new(), Vec::new(), Span::default());
            let byte_size = flat_alloc_bytes(&ty, &fc.type_info);
            let count = ordered.len() as u16;
            for field in &ordered {
                compile_expr(fc, &field.value)?;
            }
            match byte_size {
                Some(byte_size) => {
                    if type_may_intern_opaque(&ty, &fc.type_info) {
                        fc.may_intern_opaque = true;
                    }
                    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
                        kind: crate::value_layout::CompositeKind::Struct,
                        count,
                        byte_size,
                    }));
                }
                None => {
                    let meta = fc.add_struct_template(name, field_names);
                    fc.emit(Op::NewComposite(NewCompositeOperand::Boxed {
                        kind: crate::value_layout::CompositeKind::Struct,
                        count,
                        meta,
                    }));
                }
            }
        }

        Expr::EnumVariant {
            enum_name,
            variant,
            args,
            ..
        } => {
            // A uniformly-flat enum bakes the flat form: the discriminant is
            // the first packed value (the leading word) and `byte_size` is
            // `word + payload_max`, which carries the padding the verifier
            // sums and retires the old `min_payload`-via-stack push (B28 P4).
            // A non-uniform enum bakes the boxed form with a template holding
            // the type and variant names.
            //
            // `Option::Some(x)` flattens like any enum (B28 P3 item 5 C4).
            // `Option` is generic and absent from the type tables, so build
            // the concrete `Option<T>` type from the payload's inferred type
            // to drive the flat layout (`Named("Option")` would not resolve),
            // with the fixed discriminant `Some = 1`. If the payload type is
            // not recoverable the boxed fallback (`byte_size == None`) applies.
            // `Option::None` is not handled here: it has no payload and
            // materialises to the scalar `Value::None`.
            let opt_some_payload = if enum_name == "Option" && variant == "Some" {
                args.first().and_then(|a| infer_expr_type(fc, a))
            } else {
                None
            };
            let (ty, disc_override) = match opt_some_payload {
                Some(payload_ty) => (
                    TypeExpr::Option(Box::new(payload_ty), Span::default()),
                    Some(1i64),
                ),
                None => (
                    TypeExpr::Named(
                        enum_name.to_string(),
                        Vec::new(),
                        Vec::new(),
                        Span::default(),
                    ),
                    None,
                ),
            };
            let byte_size = flat_alloc_bytes(&ty, &fc.type_info);
            let disc = disc_override.unwrap_or_else(|| {
                fc.type_info
                    .enum_variant_order
                    .get(enum_name)
                    .and_then(|vs| vs.iter().find(|(n, _)| n == variant).map(|(_, d)| *d))
                    .unwrap_or(0)
            });
            match byte_size {
                Some(byte_size) => {
                    if type_may_intern_opaque(&ty, &fc.type_info) {
                        fc.may_intern_opaque = true;
                    }
                    let d_const = fc.add_constant(Value::Int(disc));
                    fc.emit(Op::Const(d_const));
                    for arg in args {
                        compile_expr(fc, arg)?;
                    }
                    fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
                        kind: crate::value_layout::CompositeKind::Enum,
                        count: args.len() as u16 + 1,
                        byte_size,
                    }));
                }
                None => {
                    for arg in args {
                        compile_expr(fc, arg)?;
                    }
                    let meta = fc.add_struct_template(enum_name, alloc::vec![variant.clone()]);
                    fc.emit(Op::NewComposite(NewCompositeOperand::Boxed {
                        kind: crate::value_layout::CompositeKind::Enum,
                        count: args.len() as u16,
                        meta,
                    }));
                }
            }
        }

        Expr::ArrayLiteral { elements, .. } => {
            // A homogeneous array's flat size is `count * element_size`,
            // taken from the inferred element type (B28 P4). An empty array
            // is zero bytes; an unrecoverable element type bakes a
            // conservative bound.
            let byte_size = if elements.is_empty() {
                Some(0u16)
            } else {
                infer_expr_type(fc, &elements[0]).and_then(|elem_ty| {
                    flat_alloc_bytes(
                        &TypeExpr::array_lit(
                            Box::new(elem_ty),
                            elements.len() as i64,
                            Span::default(),
                        ),
                        &fc.type_info,
                    )
                })
            };
            let count = elements.len() as u16;
            // Tuple and array construction is value-driven at run time (the
            // VM decides flat-or-boxed from the values, which agrees with the
            // type-driven access), so the operand is always `Flat` and
            // `byte_size` is only the verifier annotation: the exact flat size
            // when the element type is known, else a sound conservative bound
            // (B28 P4). The value-driven runtime flattens a scalar tuple even
            // when the literal's element types are not statically recoverable,
            // which an operand-driven decision here would wrongly box and
            // disagree with the inferable access. The opaque reference kind is
            // flat in this value-driven form too (B28 P3 item 3).
            let byte_size = byte_size.unwrap_or_else(|| conservative_alloc_bytes(count));
            // Value-driven flat construction interns an opaque element at
            // runtime (B28 P3 item 3), so flag the registry bound when an
            // element could be opaque or is untypeable (B28 P3 item 5).
            if elements.iter().any(|e| elem_may_intern_opaque(fc, e)) {
                fc.may_intern_opaque = true;
            }
            for elem in elements {
                compile_expr(fc, elem)?;
            }
            fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
                kind: crate::value_layout::CompositeKind::Array,
                count,
                byte_size,
            }));
        }

        Expr::TupleLiteral { elements, .. } => {
            // The tuple's flat size is the sum of its inferred element sizes
            // (B28 P4); see the array case for the value-driven rationale.
            let elem_types: Option<Vec<TypeExpr>> =
                elements.iter().map(|e| infer_expr_type(fc, e)).collect();
            let byte_size = elem_types.and_then(|types| {
                flat_alloc_bytes(&TypeExpr::Tuple(types, Span::default()), &fc.type_info)
            });
            let count = elements.len() as u16;
            let byte_size = byte_size.unwrap_or_else(|| conservative_alloc_bytes(count));
            // Value-driven flat construction interns an opaque element at
            // runtime (B28 P3 item 3), so flag the registry bound when an
            // element could be opaque or is untypeable (B28 P3 item 5).
            if elements.iter().any(|e| elem_may_intern_opaque(fc, e)) {
                fc.may_intern_opaque = true;
            }
            for elem in elements {
                compile_expr(fc, elem)?;
            }
            fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
                kind: crate::value_layout::CompositeKind::Tuple,
                count,
                byte_size,
            }));
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
            // Multiword construction. A tuple of N words casts to
            // Multiword<N>, represented as a flat N-word array. The
            // documented construction form is a tuple literal, compiled
            // directly to the digit array (B19).
            if let Some((n, _)) = target.as_multiword_lit() {
                let byte_size = flat_alloc_bytes(
                    &TypeExpr::multiword_lit(n, 0, Span::default()),
                    &fc.type_info,
                )
                .unwrap_or_else(|| conservative_alloc_bytes(n));
                if let Expr::TupleLiteral { elements, .. } = inner.as_ref() {
                    // Fast path: compile the digit expressions directly
                    // into the array, with no intermediate tuple value.
                    for e in elements {
                        compile_expr(fc, e)?;
                    }
                } else {
                    // General tuple source: compile the tuple, stash it
                    // in a scratch local, and extract each Word digit in
                    // order into the array (B19).
                    compile_expr(fc, inner)?;
                    let tmp = fc.declare_local("__mw_src");
                    fc.emit(Op::SetLocal(tmp));
                    let elem_types = alloc::vec![
                        Some(TypeExpr::Prim(PrimType::Word, Span::default()));
                        n as usize
                    ];
                    for i in 0..n as usize {
                        fc.emit(Op::GetLocal(tmp));
                        let operand = tuple_field_access(fc, &elem_types, i);
                        fc.emit(Op::GetTupleField(operand));
                    }
                }
                fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
                    kind: crate::value_layout::CompositeKind::Array,
                    count: n,
                    byte_size,
                }));
                return Ok(());
            }
            let source = infer_expr_type(fc, inner);
            // Enum-to-Word special case. The source must be an
            // enum type the compiler knows about; the cast emits
            // a chain of `IsEnum` tests, one per variant, that
            // each push the variant's discriminant on a match.
            if matches!(target, TypeExpr::Prim(PrimType::Word, _))
                && let Some(TypeExpr::Named(enum_name, _, _, _)) = source.as_ref()
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
                Some(TypeExpr::Named(name, _, _, _)) => fc.type_info.newtype_names.contains(name),
                _ => false,
            };
            let target_is_newtype = match target {
                TypeExpr::Named(name, _, _, _) => fc.type_info.newtype_names.contains(name),
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
        Expr::Classify { value, labels, .. } | Expr::Declassify { value, labels, .. } => {
            // classify / declassify are compile-time-only
            // information-flow operations. The bytecode emitted is
            // the inner expression's bytecode unchanged. Label
            // tracking and declassification audit happen entirely
            // at the type-checker layer. Under debug emission we also
            // record an IfcLabelAnnotation creating an audit trail of
            // the label operation at the value's position.
            let label_op = fc.chunk.ops.len();
            compile_expr(fc, value)?;
            fc.record_ifc_labels(label_op, labels);
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
        && let TypeExpr::Named(enum_name, _, _, _) = target
        && fc.type_info.enum_variant_order.contains_key(enum_name)
    {
        let enum_name = enum_name.clone();
        return compile_checked_discriminant(fc, inner, &enum_name, arms, span);
    }
    // The native-error construct (B35 P7): a native call with outcome
    // arms reifies a soft host failure into an `error(code)` arm.
    if let Expr::Call { name, args, .. } = op_expr
        && fc.native_map.contains_key(name)
    {
        return compile_checked_native(fc, op_expr, name, args, arms, span);
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
        Expr::BinOp {
            op: BinOp::AShl,
            left,
            right,
            ..
        } => {
            // `x <<< k` is `x * 2^k`, so the checked arithmetic left shift
            // is a checked multiply by the constant 2^k. The multiplier
            // must be a positive Word, so the amount is bounded to
            // word_bits - 2 here; a larger shift overflows for all but the
            // degenerate inputs and is out of range for the checked form.
            let word_bits = (fc.type_info.word_bytes * 8) as i64;
            let k = const_shift_amount(right, word_bits - 1)?;
            let mul = fc.add_constant(Value::Int(1i64 << k));
            compile_expr(fc, left)?;
            fc.emit(Op::Const(mul));
            fc.emit(Op::CheckedMul(0));
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
                    "checked-overflow construct currently supports only the operators `+`, `-`, `*`, `/`, `%`, `<<<`, and unary `-`",
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
            | CheckedArmKind::InvalidDiscriminant(_)
            | CheckedArmKind::Error(_) => {
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
    // len_slot = the array length. Array length is a fixed-size,
    // compile-time constant, so fold it to a literal rather than emit
    // `Op::Len`, which a flat array body cannot answer (B28 P2). Fall back
    // to `Op::Len` only for a boxed array whose length the compiler could
    // not recover statically.
    let arr_len = infer_expr_type(fc, object)
        .as_ref()
        .and_then(array_length_of_type);
    if let Some(n) = arr_len {
        let n_const = fc.add_constant(Value::Int(n));
        fc.emit(Op::Const(n_const));
    } else {
        fc.emit(Op::GetLocal(arr_slot));
        fc.emit(Op::Len);
    }
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
    fc.emit(Op::GetIndex(array_elem_operand(
        Some(&elem_ty),
        &fc.type_info,
    )));
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
    fc.emit(Op::GetIndex(array_elem_operand(
        Some(&elem_ty),
        &fc.type_info,
    )));
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
        TypeExpr::Named(
            alloc::string::String::from(newtype_name),
            Vec::new(),
            Vec::new(),
            *span,
        )
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
    // The refinement-failure trap is a partial-operation fault; record
    // the operator site so it resolves exactly to the construction
    // (B29 item 2).
    fc.record_operator_site(span);
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
                let disc = fc
                    .type_info
                    .enum_variant_order
                    .get(enum_name)
                    .and_then(|vs| {
                        vs.iter()
                            .find(|(n, _)| n.as_str() == vname.as_str())
                            .map(|(_, d)| *d)
                    })
                    .unwrap_or(0);
                // Unit-variant construction (B28 P4). A flat enum bakes the
                // discriminant as its only packed value; a non-uniform enum
                // bakes the boxed form with a type/variant template.
                let ty = TypeExpr::Named(
                    enum_name.to_string(),
                    Vec::new(),
                    Vec::new(),
                    Span::default(),
                );
                match flat_alloc_bytes(&ty, &fc.type_info) {
                    Some(byte_size) => {
                        let d_const = fc.add_constant(Value::Int(disc));
                        fc.emit(Op::Const(d_const));
                        fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
                            kind: crate::value_layout::CompositeKind::Enum,
                            count: 1,
                            byte_size,
                        }));
                    }
                    None => {
                        let meta = fc.add_struct_template(enum_name, alloc::vec![vname.clone()]);
                        fc.emit(Op::NewComposite(NewCompositeOperand::Boxed {
                            kind: crate::value_layout::CompositeKind::Enum,
                            count: 0,
                            meta,
                        }));
                    }
                }
                match &arm.kind {
                    CheckedArmKind::Ok(Pattern::Variable(bind, _)) => {
                        let slot = fc.declare_local_typed(
                            bind,
                            Some(TypeExpr::Named(
                                alloc::string::String::from(enum_name),
                                Vec::new(),
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
                let disc = fc
                    .type_info
                    .enum_variant_order
                    .get(enum_name)
                    .and_then(|vs| {
                        vs.iter()
                            .find(|(n, _)| n.as_str() == vname.as_str())
                            .map(|(_, d)| *d)
                    })
                    .unwrap_or(0);
                // Unit-variant construction (B28 P4). A flat enum bakes the
                // discriminant as its only packed value; a non-uniform enum
                // bakes the boxed form with a type/variant template.
                let ty = TypeExpr::Named(
                    enum_name.to_string(),
                    Vec::new(),
                    Vec::new(),
                    Span::default(),
                );
                match flat_alloc_bytes(&ty, &fc.type_info) {
                    Some(byte_size) => {
                        let d_const = fc.add_constant(Value::Int(disc));
                        fc.emit(Op::Const(d_const));
                        fc.emit(Op::NewComposite(NewCompositeOperand::Flat {
                            kind: crate::value_layout::CompositeKind::Enum,
                            count: 1,
                            byte_size,
                        }));
                    }
                    None => {
                        let meta = fc.add_struct_template(enum_name, alloc::vec![vname.clone()]);
                        fc.emit(Op::NewComposite(NewCompositeOperand::Boxed {
                            kind: crate::value_layout::CompositeKind::Enum,
                            count: 0,
                            meta,
                        }));
                    }
                }
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

/// Compile the native-error construct `native(args) { ok(v) => ...,
/// error(code) => ... }` (B35 P7). The lowering needs no new opcode:
/// when an `error` arm is present it sets the error-reify flag (the
/// high bit of the call opcode's argument-count byte) so the VM
/// pushes `(value, flag)` instead of propagating a soft host failure;
/// the construct then dispatches `ok` (flag 0, the success value) and
/// `error` (flag 1, the `Word` error code). Without an `error` arm the
/// call is unmodified and a native error propagates as it would
/// without the construct.
fn compile_checked_native(
    fc: &mut FuncCompiler,
    op_expr: &Expr,
    native_name: &str,
    args: &[Expr],
    arms: &[crate::ast::CheckedArm],
    span: &Span,
) -> Result<(), CompileError> {
    use crate::ast::{CheckedArmKind, Pattern};

    let ok_ty = infer_expr_type(fc, op_expr).unwrap_or(TypeExpr::Prim(PrimType::Word, *span));
    let word_ty = TypeExpr::Prim(PrimType::Word, *span);
    let has_error_arm = arms
        .iter()
        .any(|a| matches!(a.kind, CheckedArmKind::Error(_)));

    if args.len() >= 0x80 {
        return Err(CompileError {
            message: alloc::string::String::from(
                "a native call in an error-handling construct may take at most 127 arguments",
            ),
            span: *span,
        });
    }

    // Emit the arguments and the native call. The reify flag is set in
    // the high bit of the argument count only when an `error` arm is
    // present.
    for arg in args {
        compile_expr(fc, arg)?;
    }
    let mut argc = args.len() as u8;
    if has_error_arm {
        argc |= 0x80;
    }
    let idx = *fc.native_map.get(native_name).ok_or_else(|| CompileError {
        message: alloc::format!("unregistered native: {}", native_name),
        span: *span,
    })?;
    let is_external = fc
        .native_externals
        .get(native_name)
        .copied()
        .unwrap_or(false);
    if is_external {
        fc.emit(Op::CallExternalNative(idx, argc));
    } else {
        fc.emit(Op::CallVerifiedNative(idx, argc));
    }

    let suffix = span.start;
    let value_slot = fc.declare_local(&alloc::format!("__nat_val_{}", suffix));
    let flag_slot = fc.declare_local(&alloc::format!("__nat_flag_{}", suffix));
    if has_error_arm {
        // Reified call left (value, flag) on the stack; flag is on top.
        fc.emit(Op::SetLocal(flag_slot));
        fc.emit(Op::SetLocal(value_slot));
    } else {
        // Plain call left the single result; the flag is always ok.
        fc.emit(Op::SetLocal(value_slot));
        let zero_idx = fc.add_constant(Value::Int(0));
        fc.emit(Op::Const(zero_idx));
        fc.emit(Op::SetLocal(flag_slot));
    }

    let loop_addr = fc.emit(Op::Loop(0));
    fc.enter_loop();
    for arm in arms {
        fc.begin_scope();
        let mut fail_addrs: Vec<usize> = Vec::new();
        let (class_flag, pat, bind_ty): (i64, &Pattern, &TypeExpr) = match &arm.kind {
            CheckedArmKind::Ok(p) => (0, p, &ok_ty),
            CheckedArmKind::Error(p) => (1, p, &word_ty),
            _ => {
                return Err(CompileError {
                    message: alloc::string::String::from(
                        "internal error: non-native-call arm in a native-call checked construct",
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

    // No arm matched. The `ok` class is a mandatory catch-all, so this
    // is reached only when an `error` arm with a guard or literal
    // failed to match a reified error. Trap with the generic
    // no-matching-arm fault. The trailing default break keeps the loop
    // structurally closed.
    fc.emit(Op::GetLocal(flag_slot));
    let one_idx = fc.add_constant(Value::Int(1));
    fc.emit(Op::Const(one_idx));
    fc.emit(Op::CmpEq);
    let not_error = fc.emit_jump(Op::If(0));
    fc.emit(Op::Trap(crate::bytecode::TrapKind::NoMatchingArm.code()));
    fc.patch_jump(not_error);
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
                        let opt_op = fc.chunk.ops.len();
                        compile_expr(fc, &args[0])?;
                        fc.record_optimisation(opt_op, "refinement-elision");
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
                    let opt_op = fc.chunk.ops.len();
                    compile_expr(fc, &args[0])?;
                    fc.record_optimisation(opt_op, "refinement-elision");
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

/// Discard the scrutinee copy that a peeking refutable test (`IsEnum`,
/// `IsStruct`) leaves beneath its Boolean result, so the operand stack is
/// balanced on both the match-continue and fail paths of the test's `If`.
///
/// The peeking ops read `stack.last()` and push a `Bool` on top, leaving
/// `[scrutinee, bool]`. Discarding the scrutinee copy here (its authoritative
/// value lives in `value_slot` and is re-fetched for field extraction) means
/// the subsequent `If` pops only the `Bool` and both arms of the branch leave
/// the same height. Without this, a failed arm leaks the peeked scrutinee onto
/// the stack, which accumulates across arms into a stack-imbalanced `Break`
/// (audit finding B5). `IsEnum`/`IsStruct` remain peeking for
/// `emit_enum_fieldwise_eq`, which reuses the peeked value; this helper is the
/// pattern-test-local consume convention.
fn emit_consume_peeked_scrutinee(fc: &mut FuncCompiler) {
    // Stash the `Bool` result, drop the peeked scrutinee copy, restore the
    // `Bool`. Net effect: `[scrutinee, bool]` -> `[bool]`.
    let scratch = fc.declare_local("__pat_test");
    fc.emit(Op::SetLocal(scratch));
    fc.emit(Op::PopN(1));
    fc.emit(Op::GetLocal(scratch));
}

/// Compile a pattern test. Returns addresses of If instructions that need
/// patching to the "fail" destination (EndIf at the next arm or error).
fn compile_pattern_test(
    fc: &mut FuncCompiler,
    pattern: &Pattern,
    value_slot: u16,
    // The matched value's static type, threaded so the tuple arm can
    // bake a flat field access when the elements are scalar (B28 P2).
    // `None` forces the boxed access form, which a flat-bodied value
    // would reject; callers therefore supply the type wherever a tuple
    // pattern can meet a flat tuple.
    ty: Option<&TypeExpr>,
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
            // Both `Option` variants go through `IsEnum`. `Option::Some(x)`
            // flattens (or boxes) as an enum body with discriminant 1;
            // `Option::None` is discriminant 0. `IsEnum` matches a flat `[disc]`
            // body, a boxed enum by name, and -- for `Option::None` specifically
            // -- the scalar `Value::None` a top-level or host-returned None uses
            // (the VM's `IsEnum` handler special-cases it). Routing `None`
            // through `IsEnum` rather than the old scalar `Value::None`
            // comparison is what lets a nested flat `[disc=0]` Option payload,
            // extracted from a flat parent, match `Option::None`.
            fc.emit(Op::GetLocal(value_slot));
            let e_const = fc.add_string_constant(enum_name);
            let v_const = fc.add_string_constant(variant);
            // The variant discriminant lets the test compare a flat enum's
            // leading discriminant word (B28 P2); the boxed body still
            // compares the names.
            // `Option::Some` flattens (B28 P3 item 5 C4) with the fixed
            // discriminant 1; for a flat body `IsEnum` compares the leading
            // discriminant word, so it must match construction.
            let is_option_some = enum_name == "Option" && variant == "Some";
            let disc = if is_option_some {
                1
            } else if enum_name == "Option" && variant == "None" {
                // `Option::None` flattens with discriminant 0 (mirrors the
                // `Some == 1` convention); the `IsEnum` test above then matches a
                // flat `[disc=0]` body, and the VM additionally matches a scalar
                // `Value::None` against `Option::None`.
                0
            } else {
                fc.type_info
                    .enum_variant_order
                    .get(enum_name)
                    .and_then(|vs| vs.iter().find(|(n, _)| n == variant).map(|(_, d)| *d))
                    .unwrap_or(0)
            };
            let d_const = fc.add_constant(Value::Int(disc));
            fc.emit(Op::IsEnum(e_const, v_const, d_const));
            // `IsEnum` peeks; consume the scrutinee copy on both branches so a
            // failed arm does not leak it onto the stack (B5).
            emit_consume_peeked_scrutinee(fc);
            fail_addrs.push(fc.emit_jump(Op::If(0)));

            // Test sub-patterns on extracted fields.
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                if matches!(sub_pat, Pattern::Variable(_, _) | Pattern::Wildcard(_)) {
                    continue; // Always matches; will bind during bind phase.
                }
                let temp = fc.declare_local(&format!("__enum_field{}", i));
                fc.emit(Op::GetLocal(value_slot));
                // `Option::Some`'s payload type comes from the scrutinee
                // `Option<T>` (it is not in the type tables); other enums look
                // up their variant payloads.
                let efield = if is_option_some {
                    option_some_field(fc, ty, i)
                } else {
                    enum_field_access(fc, enum_name, variant, i)
                };
                fc.emit(Op::GetEnumField(efield));
                fc.emit(Op::SetLocal(temp));
                let sub_ty = if is_option_some {
                    option_inner(ty)
                } else {
                    fc.type_info
                        .enums
                        .get(enum_name)
                        .and_then(|m| m.get(variant))
                        .and_then(|payloads| payloads.get(i))
                        .cloned()
                };
                let sub_fails = compile_pattern_test(fc, sub_pat, temp, sub_ty.as_ref())?;
                fail_addrs.extend(sub_fails);
            }
        }
        Pattern::Struct(type_name, field_pats, _) => {
            // The scrutinee of a struct pattern is statically the struct
            // type, so the type test is irrefutable; fold it out when the
            // type is confirmed (this also keeps a flat struct, which
            // carries no type name, away from `Op::IsStruct`). Fall back to
            // the runtime test only when the type is not statically known,
            // where the boxed body answers it.
            if named_type_name(ty) != Some(type_name.as_str()) {
                fc.emit(Op::GetLocal(value_slot));
                let t_const = fc.add_string_constant(type_name);
                fc.emit(Op::IsStruct(t_const));
                // `IsStruct` peeks; consume the scrutinee copy on both branches
                // so a failed arm does not leak it onto the stack (B5).
                emit_consume_peeked_scrutinee(fc);
                fail_addrs.push(fc.emit_jump(Op::If(0)));
            }

            for field_pat in field_pats {
                if let Some(pat) = &field_pat.pattern {
                    if matches!(pat, Pattern::Variable(_, _) | Pattern::Wildcard(_)) {
                        continue;
                    }
                    let temp = fc.declare_local(&format!("__struct_{}", field_pat.name));
                    fc.emit(Op::GetLocal(value_slot));
                    let field_op = struct_field_access(fc, type_name, &field_pat.name);
                    fc.emit(Op::GetField(field_op));
                    fc.emit(Op::SetLocal(temp));
                    let sub_ty = fc
                        .type_info
                        .structs
                        .get(type_name)
                        .and_then(|m| m.get(&field_pat.name))
                        .cloned();
                    let sub_fails = compile_pattern_test(fc, pat, temp, sub_ty.as_ref())?;
                    fail_addrs.extend(sub_fails);
                }
            }
        }
        Pattern::Tuple(pats, _) => {
            let elem_types = tuple_elem_types_of(ty, pats.len());
            for (i, pat) in pats.iter().enumerate() {
                if matches!(pat, Pattern::Variable(_, _) | Pattern::Wildcard(_)) {
                    continue;
                }
                let temp = fc.declare_local(&format!("__tuple_{}", i));
                fc.emit(Op::GetLocal(value_slot));
                fc.emit(Op::GetTupleField(tuple_field_access(fc, &elem_types, i)));
                fc.emit(Op::SetLocal(temp));
                let sub_ty = elem_types.get(i).and_then(|o| o.as_ref());
                let sub_fails = compile_pattern_test(fc, pat, temp, sub_ty)?;
                fail_addrs.extend(sub_fails);
            }
        }
    }

    Ok(fail_addrs)
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
            // `Option::Some` flattens (B28 P3 item 5 C4); its payload type
            // comes from the scrutinee `Option<T>` since `Option` is generic
            // and absent from the type tables.
            let is_option_some = enum_name == "Option" && variant == "Some";
            // For enum sub-pattern bindings, look up the variant's
            // payload types from the type info when available.
            let payload_types: Vec<Option<TypeExpr>> = if is_option_some {
                alloc::vec![option_inner(ty.as_ref())]
            } else {
                fc.type_info
                    .enums
                    .get(enum_name)
                    .and_then(|variants| variants.get(variant))
                    .map(|tys| tys.iter().cloned().map(Some).collect())
                    .unwrap_or_else(|| sub_pats.iter().map(|_| None).collect())
            };
            for (i, sub_pat) in sub_pats.iter().enumerate() {
                if matches!(sub_pat, Pattern::Wildcard(_) | Pattern::Literal(_, _)) {
                    continue;
                }
                fc.emit(Op::GetLocal(value_slot));
                let efield = if is_option_some {
                    option_some_field(fc, ty.as_ref(), i)
                } else {
                    enum_field_access(fc, enum_name, variant, i)
                };
                fc.emit(Op::GetEnumField(efield));
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
                fc.emit(Op::GetLocal(value_slot));
                let field_op = struct_field_access(fc, struct_name, &field_pat.name);
                fc.emit(Op::GetField(field_op));
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
                fc.emit(Op::GetTupleField(tuple_field_access(fc, &elem_types, i)));
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

    fn compile_str_debug(src: &str) -> Module {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        compile_with_options(
            &program,
            &crate::target::Target::host(),
            &CompileOptions { emit_debug: true },
        )
        .expect("compile")
        .0
    }

    #[test]
    fn debug_emission_records_call_site_at_the_call_op() {
        let src = "fn helper() -> Word { 1 }\nfn main() -> Word { helper() }";
        let module = compile_str_debug(src);
        let main_chunk = module
            .chunks
            .iter()
            .find(|c| c.ops.iter().any(|op| matches!(op, Op::Call(..))))
            .expect("a chunk containing a call");
        let pool = main_chunk
            .debug_pool
            .as_ref()
            .expect("debug build attaches a debug pool to the calling chunk");

        let call_idx = main_chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Call(..)))
            .unwrap();
        let call_records: alloc::vec::Vec<_> = pool
            .records
            .iter()
            .filter(|r| r.kind == crate::debug_meta::DebugRecordKind::CallSite)
            .collect();
        assert_eq!(call_records.len(), 1, "one CallSite for the single call");
        assert_eq!(
            call_records[0].op_index as usize, call_idx,
            "the CallSite must key on the call instruction's op index"
        );

        // The referenced span covers the `helper()` call in the source.
        let span_idx = call_records[0].operands[0] as usize;
        let (_file, start, len) = pool.span_pool[span_idx];
        let snippet = &src[start as usize..(start + len) as usize];
        assert!(
            snippet.contains("helper"),
            "span should cover the call expression, got {:?}",
            snippet
        );
    }

    #[test]
    fn debug_read_path_resolves_call_to_source() {
        // End-to-end through the public read API: compile with debug,
        // look up the record at the call op, and resolve it back to the
        // source text via source_location.
        let src = "fn helper() -> Word { 1 }\nfn main() -> Word { helper() }";
        let module = compile_str_debug(src);
        let main_chunk = module
            .chunks
            .iter()
            .find(|c| c.ops.iter().any(|op| matches!(op, Op::Call(..))))
            .expect("a chunk containing a call");
        let pool = main_chunk.debug_pool.as_ref().expect("debug pool present");
        let call_idx = main_chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Call(..)))
            .unwrap() as u32;
        let rec = pool
            .records_at(call_idx)
            .next()
            .expect("a record at the call op");
        let loc = pool
            .source_location(rec)
            .expect("the call-site record resolves to a source location");
        let snippet = &src[loc.byte_offset as usize..(loc.byte_offset + loc.byte_length) as usize];
        assert!(
            snippet.contains("helper"),
            "resolved location should cover the call expression, got {:?}",
            snippet
        );
    }

    #[test]
    fn debug_emission_covers_the_emittable_catalogue() {
        // A program with a call and a local binding exercises all four
        // emittable record kinds: CallSite, SourceSpan, LineNumber,
        // VariableName.
        let src = "fn helper() -> Word { 1 }\nfn main() -> Word { let x = helper(); x }";
        let module = compile_str_debug(src);
        let main_chunk = module
            .chunks
            .iter()
            .find(|c| c.ops.iter().any(|op| matches!(op, Op::Call(..))))
            .expect("a chunk containing a call");
        let pool = main_chunk.debug_pool.as_ref().expect("debug pool present");

        use crate::debug_meta::DebugRecordKind;
        let present = |k: DebugRecordKind| pool.records.iter().any(|r| r.kind == k);
        assert!(present(DebugRecordKind::CallSite), "CallSite emitted");
        assert!(present(DebugRecordKind::SourceSpan), "SourceSpan emitted");
        assert!(present(DebugRecordKind::LineNumber), "LineNumber emitted");
        assert!(
            present(DebugRecordKind::VariableName),
            "VariableName emitted"
        );

        // The VariableName record resolves its slot to the source name.
        let var = pool
            .records
            .iter()
            .find(|r| r.kind == DebugRecordKind::VariableName)
            .unwrap();
        let name_idx = var.operands[1];
        assert_eq!(pool.string(name_idx), Some("x"));
    }

    #[test]
    fn debug_emission_records_refinement_elision_as_optimisation() {
        // Counter(5) constant-folds: the compiler proves the predicate
        // at compile time and elides the runtime check, recording a
        // refinement-elision OptimisationMarker.
        let src = "fn nonneg(x: Word) -> bool { x >= 0 }\n\
                   newtype Counter = Word where nonneg;\n\
                   fn main() -> Word { let c = Counter(5); c as Word }";
        let module = compile_str_debug(src);
        let found = module.chunks.iter().any(|c| {
            c.debug_pool.as_ref().is_some_and(|p| {
                p.records.iter().any(|r| {
                    r.kind == crate::debug_meta::DebugRecordKind::OptimisationMarker
                        && p.string(r.operands[0]) == Some("refinement-elision")
                })
            })
        });
        assert!(
            found,
            "a constant-folded refinement construction should record a refinement-elision OptimisationMarker"
        );
    }

    #[test]
    fn debug_emission_records_generic_instantiation() {
        // id(5) instantiates the generic id<Word>; the specialized
        // chunk records a GenericInstantiation naming its origin.
        let src = "fn id<T>(x: T) -> T { x }\nfn main() -> Word { id(5) }";
        let module = compile_str_debug(src);
        let found = module.chunks.iter().any(|c| {
            c.debug_pool.as_ref().is_some_and(|p| {
                p.records.iter().any(|r| {
                    r.kind == crate::debug_meta::DebugRecordKind::GenericInstantiation
                        && p.string(r.operands[0]) == Some("id")
                })
            })
        });
        assert!(
            found,
            "the monomorphized id<Word> chunk should record a GenericInstantiation with origin `id`"
        );
    }

    #[test]
    fn debug_emission_records_ifc_label_annotation() {
        // classify and declassify @Secret each record an
        // IfcLabelAnnotation carrying the label set, for the audit trail.
        let src = "fn secret() -> Word@Secret { classify 5@Secret }\n\
                   fn main() -> Word { declassify secret()@Secret }";
        let module = compile_str_debug(src);
        let found = module.chunks.iter().any(|c| {
            c.debug_pool.as_ref().is_some_and(|p| {
                p.records.iter().any(|r| {
                    r.kind == crate::debug_meta::DebugRecordKind::IfcLabelAnnotation
                        && r.operands.iter().any(|&op| p.string(op) == Some("Secret"))
                })
            })
        });
        assert!(
            found,
            "classify/declassify @Secret should record an IfcLabelAnnotation with label Secret"
        );
    }

    #[test]
    fn debug_emission_records_type_annotation_for_locals() {
        // A typed local records a TypeAnnotation mapping its slot to a
        // string-form TypeRepr.
        let src = "fn main() -> Word { let x: Word = 5; x }";
        let module = compile_str_debug(src);
        let found = module.chunks.iter().any(|c| {
            c.debug_pool.as_ref().is_some_and(|p| {
                p.records.iter().any(|r| {
                    r.kind == crate::debug_meta::DebugRecordKind::TypeAnnotation
                        && p.type_blob(r.operands[1]) == Some("Word".as_bytes())
                })
            })
        });
        assert!(
            found,
            "a typed local should record a TypeAnnotation with TypeRepr `Word`"
        );
    }

    #[cfg(feature = "verify")]
    #[test]
    fn debug_emission_records_wcet_marker_for_stream_chunks() {
        // A loop-main program has a Stream chunk whose per-iteration
        // WCET the verifier computes; under --debug it carries a
        // WcetMarker whose two u16 operands reconstruct that u32 cost.
        let src = "loop main(tick: Word) -> Word { let r = yield tick; r }";
        let module = compile_str_debug(src);
        let stream = module
            .chunks
            .iter()
            .find(|c| matches!(c.block_type, crate::bytecode::BlockType::Stream))
            .expect("a Stream chunk");
        let pool = stream
            .debug_pool
            .as_ref()
            .expect("Stream chunk carries a debug pool under --debug");
        let rec = pool
            .records
            .iter()
            .find(|r| r.kind == crate::debug_meta::DebugRecordKind::WcetMarker)
            .expect("a WcetMarker record");
        let reconstructed = rec.operands[1] as u32 | ((rec.operands[2] as u32) << 16);
        let expected = crate::verify::wcet_stream_iteration(stream).unwrap();
        assert_eq!(
            reconstructed, expected,
            "the WcetMarker cycle count should match the verifier's WCET"
        );
    }

    #[test]
    fn assert_emits_trap_and_context_under_debug() {
        let module = compile_str_debug("fn main() -> Word { assert false, \"boom\"; 0 }");
        let assert_code = crate::bytecode::TrapKind::AssertionFailed.code();
        let chunk = module
            .chunks
            .iter()
            .find(|c| {
                c.ops
                    .iter()
                    .any(|op| matches!(op, Op::Trap(c) if *c == assert_code))
            })
            .expect("debug build emits an AssertionFailed trap");
        let pool = chunk.debug_pool.as_ref().expect("debug pool present");
        let rec = pool
            .records
            .iter()
            .find(|r| r.kind == crate::debug_meta::DebugRecordKind::AssertionContext)
            .expect("an AssertionContext record");
        // operand[0] resolves to the source span; operand[1] is the message.
        assert!(pool.source_location(rec).is_some());
        assert_eq!(pool.string(rec.operands[1]), Some("boom"));
    }

    #[test]
    fn assert_is_compiled_out_in_release() {
        let module = compile_str("fn main() -> Word { assert false; 0 }").expect("compile");
        let assert_code = crate::bytecode::TrapKind::AssertionFailed.code();
        for c in &module.chunks {
            assert!(
                !c.ops
                    .iter()
                    .any(|op| matches!(op, Op::Trap(code) if *code == assert_code)),
                "a release build must compile the assert out (no trap op)"
            );
            assert!(
                c.debug_pool.is_none(),
                "release build carries no debug pool"
            );
        }
    }

    #[test]
    fn assert_non_bool_condition_is_type_error() {
        let err = compile_str("fn main() -> Word { assert 5; 0 }").expect_err("type error");
        assert!(err.message.contains("bool"), "got: {}", err.message);
    }

    #[test]
    fn assert_call_form_remains_a_function_call() {
        // `assert(...)` with a parenthesis is a call to a user function
        // named `assert`, not the assertion statement.
        let module =
            compile_str_debug("fn assert(x: Word) -> Word { x }\nfn main() -> Word { assert(7) }");
        let assert_code = crate::bytecode::TrapKind::AssertionFailed.code();
        assert!(
            !module.chunks.iter().any(|c| c
                .ops
                .iter()
                .any(|op| matches!(op, Op::Trap(c) if *c == assert_code))),
            "assert(7) is a call, not an assertion statement"
        );
    }

    #[test]
    fn assert_strip_keeps_check_drops_context() {
        // Stripping a debug build removes the AssertionContext record
        // but leaves the check ops, so a stripped build still traps
        // (generically) and is not identical to a release build, which
        // has no check at all.
        use crate::debug_meta::DebugRecordKind;
        let assert_code = crate::bytecode::TrapKind::AssertionFailed.code();
        let has_trap = |m: &Module| {
            m.chunks.iter().any(|c| {
                c.ops
                    .iter()
                    .any(|op| matches!(op, Op::Trap(c) if *c == assert_code))
            })
        };
        let mut debug = compile_str_debug("fn main() -> Word { assert false; 0 }");
        assert!(has_trap(&debug), "debug build emits the assert check");
        assert!(
            debug
                .chunks
                .iter()
                .any(|c| c.debug_pool.as_ref().is_some_and(|p| p
                    .records
                    .iter()
                    .any(|r| r.kind == DebugRecordKind::AssertionContext))),
            "debug build carries an AssertionContext record"
        );
        for c in &mut debug.chunks {
            c.debug_pool = None;
        }
        assert!(has_trap(&debug), "strip keeps the assert check ops");
        let release = compile_str("fn main() -> Word { assert false; 0 }").expect("compile");
        assert!(!has_trap(&release), "release build has no assert check");
    }

    #[cfg(feature = "verify")]
    #[test]
    fn assert_in_nested_loop_body_verifies_and_passes() {
        use crate::vm::{Vm, VmState};
        let module = compile_str_debug(
            "fn main() -> Word { for i in 0..3 { assert i < 10, \"small\"; } 0 }",
        );
        let arena = crate::Arena::with_capacity(crate::vm::DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify accepts assert in a loop body");
        match vm.call(&[]).expect("call") {
            VmState::Finished(crate::Value::Int(0)) => {}
            other => panic!("expected Finished(Int(0)), got {:?}", other),
        }
    }

    #[cfg(feature = "verify")]
    #[test]
    fn assert_in_loop_main_stream_chunk_verifies() {
        use crate::vm::Vm;
        // Vm::new runs structural and resource verification; success
        // proves the assert trap is well-formed inside a Stream chunk.
        // The productive-divergent loop itself is not driven here.
        let module = compile_str_debug(
            "loop main(tick: Word) -> Word { assert tick >= 0, \"t\"; let r = yield tick; r }",
        );
        let arena = crate::Arena::with_capacity(crate::vm::DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify accepts assert in a Stream chunk");
    }

    #[cfg(feature = "verify")]
    #[test]
    fn assert_false_traps_at_runtime_under_debug() {
        use crate::vm::Vm;
        let module = compile_str_debug("fn main() -> Word { assert false, \"nope\"; 0 }");
        let arena = crate::Arena::with_capacity(crate::vm::DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]) {
            Err(crate::vm::VmError::AssertionFailed) => {}
            other => panic!("expected AssertionFailed, got {:?}", other),
        }
    }

    #[cfg(feature = "verify")]
    #[test]
    fn assert_true_passes_at_runtime_under_debug() {
        use crate::vm::{Vm, VmState};
        let module = compile_str_debug("fn main() -> Word { assert true; 0 }");
        let arena = crate::Arena::with_capacity(crate::vm::DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(crate::Value::Int(0)) => {}
            other => panic!("expected Finished(Int(0)), got {:?}", other),
        }
    }

    #[test]
    fn debug_emission_records_breakpoint_candidates() {
        // Every statement boundary in a debug build is a breakpoint
        // candidate that resolves to a source span.
        let src = "fn main() -> Word { let x = 1; let y = 2; x }";
        let module = compile_str_debug(src);
        let pool = module
            .chunks
            .iter()
            .find_map(|c| c.debug_pool.as_ref())
            .expect("debug pool present");
        let candidates: alloc::vec::Vec<_> = pool
            .records
            .iter()
            .filter(|r| r.kind == crate::debug_meta::DebugRecordKind::BreakpointCandidate)
            .collect();
        assert!(
            !candidates.is_empty(),
            "a debug build records breakpoint candidates"
        );
        for c in &candidates {
            assert!(
                pool.source_location(c).is_some(),
                "a breakpoint candidate resolves to a source location"
            );
        }
        let release = compile_str(src).expect("compile");
        assert!(release.chunks.iter().all(|c| c.debug_pool.is_none()));
    }

    /// Collect the (pass, property) string pairs of every
    /// VerifierWitness record in a pool, for assertions below.
    #[cfg(feature = "verify")]
    fn witness_pairs(pool: &crate::debug_meta::DebugPool) -> alloc::vec::Vec<(&str, &str)> {
        use crate::debug_meta::DebugRecordKind;
        pool.records
            .iter()
            .filter(|r| r.kind == DebugRecordKind::VerifierWitness)
            .filter_map(|r| {
                let pass = pool.string(*r.operands.first()?)?;
                let property = pool.string(*r.operands.get(1)?)?;
                Some((pass, property))
            })
            .collect()
    }

    #[cfg(feature = "verify")]
    #[test]
    fn debug_emission_records_verifier_witness() {
        // A debug build records a VerifierWitness per discharged
        // obligation, naming the pass and the property it established.
        let module = compile_str_debug("fn main() -> Word { 1 }");
        let pool = module
            .chunks
            .iter()
            .find_map(|c| c.debug_pool.as_ref())
            .expect("debug pool present");
        let pairs = witness_pairs(pool);
        let passes: alloc::vec::Vec<&str> = pairs.iter().map(|(p, _)| *p).collect();
        assert!(passes.contains(&"block-nesting-and-offsets"));
        assert!(passes.contains(&"block-type-constraints"));
        // A Func chunk is not productively divergent.
        assert!(!passes.contains(&"productive-divergence"));
        // The chunk-level obligations are present.
        assert!(pairs.contains(&("block-nesting-and-offsets", "all-blocks-closed")));
        assert!(pairs.contains(&("block-type-constraints", "func-has-no-yield")));
        assert!(pairs.contains(&("block-type-constraints", "func-has-no-stream")));
        assert!(pairs.contains(&("block-type-constraints", "func-has-no-reset")));
        // A Func chunk records per-chunk resource-bound obligations.
        assert!(pairs.contains(&("resource-bounds", "wcet-per-chunk-bound-proven")));
        assert!(pairs.contains(&("resource-bounds", "wcmu-per-chunk-bound-proven")));

        let release = compile_str("fn main() -> Word { 1 }").expect("compile");
        assert!(release.chunks.iter().all(|c| c.debug_pool.is_none()));
    }

    #[cfg(feature = "verify")]
    #[test]
    fn func_resource_witness_does_not_set_module_wcet_header() {
        // The Func per-chunk resource-bound obligations are a witness
        // fact only: an atomic-total (Stream-free) module still declares
        // no WCET/WCMU header (the header is the per-iteration maximum
        // across Stream chunks, of which there are none here).
        let module = compile_str_debug("fn main() -> Word { 1 + 2 }");
        assert_eq!(module.wcet_cycles, 0, "no Stream chunk: header stays auto");
        assert_eq!(module.wcmu_bytes, 0, "no Stream chunk: header stays auto");
    }

    #[cfg(feature = "verify")]
    #[test]
    fn verifier_witness_records_reentrant_resource_bounds() {
        // A Reentrant chunk (a yield function) records the per-chunk
        // resource-bound obligations: a whole-body WCET (cumulative
        // across resumptions) and the persistent WCMU peak.
        let module = compile_str_debug("yield process(input: Word) -> Word { yield input * 2 }");
        let chunk = module
            .chunks
            .iter()
            .find(|c| matches!(c.block_type, crate::bytecode::BlockType::Reentrant))
            .expect("a Reentrant chunk");
        let pool = chunk.debug_pool.as_ref().expect("debug pool present");
        let pairs = witness_pairs(pool);
        assert!(pairs.contains(&("resource-bounds", "wcet-per-chunk-bound-proven")));
        assert!(pairs.contains(&("resource-bounds", "wcmu-per-chunk-bound-proven")));
    }

    #[test]
    fn debug_emission_records_operator_site_at_division() {
        // A division emits a SourceSpan and a BreakpointCandidate at the
        // Div op (B29 items 2/3), so a zero-divisor fault resolves
        // exactly and a debugger can break before the division.
        use crate::bytecode::Op;
        use crate::debug_meta::DebugRecordKind;
        let module = compile_str_debug("fn main(d: Word) -> Word { let x = 1 / d; x }");
        let chunk = &module.chunks[0];
        let div_op = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::Div))
            .expect("a Div op") as u32;
        let pool = chunk.debug_pool.as_ref().expect("debug pool present");
        let kinds: alloc::vec::Vec<_> = pool.records_at(div_op).map(|r| r.kind).collect();
        assert!(
            kinds.contains(&DebugRecordKind::SourceSpan),
            "the Div op has an exact SourceSpan, found {kinds:?}"
        );
        assert!(
            kinds.contains(&DebugRecordKind::BreakpointCandidate),
            "the Div op is an operator-level breakpoint candidate"
        );
    }

    #[test]
    fn debug_emission_records_function_entry_breakpoint() {
        // Op 0 of a chunk is a function-entry breakpoint candidate,
        // distinct from the per-statement candidates. A Stream function
        // isolates it: op 0 is the Stream op and the first statement
        // begins later, so the only candidate at op 0 is function entry.
        use crate::debug_meta::DebugRecordKind;
        let module = compile_str_debug("loop main(tick: Word) -> Word { let r = yield tick; r }");
        let chunk = module
            .chunks
            .iter()
            .find(|c| matches!(c.block_type, crate::bytecode::BlockType::Stream))
            .expect("a Stream chunk");
        let pool = chunk.debug_pool.as_ref().expect("debug pool present");
        assert!(
            pool.records_at(0)
                .any(|r| r.kind == DebugRecordKind::BreakpointCandidate),
            "function entry (op 0) is a breakpoint candidate"
        );
    }

    #[cfg(feature = "verify")]
    #[test]
    fn verifier_witness_marks_stream_productive_divergence() {
        let module = compile_str_debug("loop main(tick: Word) -> Word { let r = yield tick; r }");
        let stream = module
            .chunks
            .iter()
            .find(|c| matches!(c.block_type, crate::bytecode::BlockType::Stream))
            .expect("a Stream chunk");
        let pool = stream.debug_pool.as_ref().expect("debug pool present");
        let pairs = witness_pairs(pool);
        assert!(
            pairs.contains(&("productive-divergence", "every-stream-to-reset-path-yields")),
            "a Stream chunk's witness records the productive-divergence proof"
        );
        // The Stream block-type obligations are present too.
        assert!(pairs.contains(&("block-type-constraints", "stream-has-yield")));
        assert!(pairs.contains(&("block-type-constraints", "stream-has-exactly-one-stream")));
    }

    #[cfg(feature = "verify")]
    #[test]
    fn verifier_witness_records_resource_bounds() {
        // A Stream chunk's witness records the per-iteration WCET and
        // WCMU bound proofs discharged at the verifier stage, distinct
        // from the structural passes.
        let module = compile_str_debug("loop main(tick: Word) -> Word { let r = yield tick; r }");
        let stream = module
            .chunks
            .iter()
            .find(|c| matches!(c.block_type, crate::bytecode::BlockType::Stream))
            .expect("a Stream chunk");
        let pool = stream.debug_pool.as_ref().expect("debug pool present");
        let pairs = witness_pairs(pool);
        assert!(
            pairs.contains(&("resource-bounds", "wcet-per-iteration-bound-proven")),
            "the WCET bound proof is recorded, found {pairs:?}"
        );
        assert!(
            pairs.contains(&("resource-bounds", "wcmu-per-iteration-bound-proven")),
            "the WCMU bound proof is recorded, found {pairs:?}"
        );
        // A Func chunk records the per-chunk resource-bound variant,
        // not the per-iteration one (which is Stream-specific).
        let func = compile_str_debug("fn main() -> Word { 1 }");
        let fpool = func
            .chunks
            .iter()
            .find_map(|c| c.debug_pool.as_ref())
            .expect("debug pool present");
        let fpairs = witness_pairs(fpool);
        assert!(fpairs.contains(&("resource-bounds", "wcet-per-chunk-bound-proven")));
        assert!(fpairs.contains(&("resource-bounds", "wcmu-per-chunk-bound-proven")));
        assert!(
            !fpairs
                .iter()
                .any(|(_, prop)| prop.ends_with("per-iteration-bound-proven")),
            "a Func chunk does not record the Stream per-iteration variant"
        );
    }

    #[cfg(feature = "verify")]
    #[test]
    fn verifier_witness_keys_obligations_to_construct_positions() {
        use crate::bytecode::Op;
        use crate::debug_meta::DebugRecordKind;
        // A function with an `if` produces a per-construct pass-1
        // obligation keyed to the If op's position, not to op 0.
        let module = compile_str_debug("fn main() -> Word { if true { 1 } else { 2 } }");
        let chunk = module
            .chunks
            .iter()
            .find(|c| c.name == "main")
            .expect("main chunk");
        let if_pos = chunk
            .ops
            .iter()
            .position(|op| matches!(op, Op::If(_)))
            .expect("an If op") as u32;
        let pool = chunk.debug_pool.as_ref().expect("debug pool present");
        // The If's obligation is keyed to the If position.
        let at_if: alloc::vec::Vec<&str> = pool
            .records_at(if_pos)
            .filter(|r| r.kind == DebugRecordKind::VerifierWitness)
            .filter_map(|r| pool.string(*r.operands.get(1)?))
            .collect();
        assert!(
            at_if.contains(&"if-branch-target-in-bounds"),
            "the If obligation is keyed to op {if_pos}, found {at_if:?}"
        );
        // And it is not the chunk-level position.
        assert_ne!(if_pos, 0);
    }

    #[test]
    fn release_build_emits_no_debug_pool() {
        let src = "fn helper() -> Word { 1 }\nfn main() -> Word { helper() }";
        let module = compile_str(src).expect("compile");
        assert!(
            module.chunks.iter().all(|c| c.debug_pool.is_none()),
            "a default (release) build must carry no debug metadata"
        );
    }

    #[test]
    fn debug_build_strips_to_release_bytes() {
        // The debug build, with its debug pools dropped, must encode to
        // exactly the release bytes (B29 invariant 5 end to end).
        let src = "fn helper() -> Word { 1 }\nfn main() -> Word { helper() }";
        let release = compile_str(src).expect("compile");
        let mut debug = compile_str_debug(src);
        assert!(
            debug.chunks.iter().any(|c| c.debug_pool.is_some()),
            "debug build should have emitted at least one pool"
        );
        for c in &mut debug.chunks {
            c.debug_pool = None;
        }
        assert_eq!(
            release.to_bytes().expect("encode release"),
            debug.to_bytes().expect("encode stripped"),
            "stripped debug build must be byte-identical to the release build"
        );
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
                .any(|op| matches!(op, Op::NewComposite(o) if o.kind() == crate::value_layout::CompositeKind::Enum))
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
            .filter(|op| matches!(op, Op::IsEnum(_, _, _)))
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
                .any(|op| matches!(op, Op::NewComposite(o) if o.kind() == crate::value_layout::CompositeKind::Struct))
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
                .any(|op| matches!(op, Op::GetIndex(_)))
        );
    }

    #[test]
    fn compile_array_index_bakes_flat_access() {
        // A scalar array indexes through the flat access form, matching
        // the flat body the construction handler builds (B28 P2).
        let module = compile_str("fn main() -> Word { let a = [10, 20, 30]; a[1] }").unwrap();
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::GetIndex(crate::bytecode::ArrayElem::Flat { .. })))
        );
    }

    #[test]
    fn compile_checked_index_folds_length_no_op_len() {
        // The checked-index bounds length is a compile-time constant the
        // compiler folds to a literal; it never emits `Op::Len` on a
        // (flat) array, and the element read bakes the flat form (B28 P2).
        let module = compile_str(
            "fn main() -> Word { let a = [10, 20, 30]; a[1] { ok(v) => v, invalid_index(i) => i } }",
        )
        .unwrap();
        let ops = &module.chunks[0].ops;
        assert!(!ops.iter().any(|op| matches!(op, Op::Len)));
        assert!(
            ops.iter()
                .any(|op| matches!(op, Op::GetIndex(crate::bytecode::ArrayElem::Flat { .. })))
        );
    }

    #[test]
    fn compile_struct_access_bakes_flat_field() {
        // A scalar struct's field access bakes the flat form, matching the
        // flat body the construction handler builds (B28 P2).
        let module = compile_str(
            "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 1, b: 2 }; p.a + p.b }",
        )
        .unwrap();
        assert!(
            module.chunks[0]
                .ops
                .iter()
                .any(|op| matches!(op, Op::GetField(crate::bytecode::StructField::Flat { .. })))
        );
    }

    #[test]
    fn compile_struct_pattern_folds_is_struct() {
        // A struct pattern's type test is irrefutable (the scrutinee type
        // is statically known), so it is folded out; no `Op::IsStruct` is
        // emitted, and a flat struct never reaches that op (B28 P2).
        let module = compile_str(
            "struct P { a: Word, b: Word }\nfn main() -> Word { let p = P { a: 1, b: 2 }; match p { P { a, b } => a + b, _ => 0 } }",
        )
        .unwrap();
        let ops = &module.chunks[0].ops;
        assert!(!ops.iter().any(|op| matches!(op, Op::IsStruct(_))));
        assert!(
            ops.iter()
                .any(|op| matches!(op, Op::GetField(crate::bytecode::StructField::Flat { .. })))
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
                .any(|op| matches!(op, Op::NewComposite(o) if o.kind() == crate::value_layout::CompositeKind::Tuple && o.count() == 3))
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
            debug_pool: None,
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
