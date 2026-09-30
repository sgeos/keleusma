//! Returned-continuation lowering of bytecode streams and reentrant entries.
//!
//! The caller owns the frame, reply cell, and the ordinary shared, private and
//! composite regions as disjoint reservations for the entire lifetime of a suspended instance. Start
//! returns a continuation and yielded scalar bits or a flat body pointer. Before each resume the caller
//! writes scalar bits or a flat body pointer to the reply cell, then calls the continuation with
//! `(frame, false)`. Release calls it with `(frame, true)` and invalidates the
//! instance. Float bits occupy the low 32 or all 64 bits according to the
//! module width. Byte and Boolean inputs must be in their declared ranges.
//! Read a yielded flat body before the next resume or release. Its declared
//! byte size is carried by its dialogue contract, and its storage belongs to the
//! instance regions, including the coroutine frame for copied host values.
//! A flat argument is an i64 pointer to its signature's exact packed body.
//! A flat reply uses the selected dialogue's body extent. The host supplies
//! readable bytes with the declared layout and valid field values at each call. The buffer may be reused
//! immediately after that call returns. It must be disjoint from instance
//! storage. Host-derived values are copied into bounded storage when transferred;
//! ordinary region and private-data references preserve VM aliasing semantics.
//! The start argument retains its normal native type. A released frame may be reused; a live frame may not be moved.
//!
//! LLVM manages the continuation and captures live allocas. The emitted module
//! is accepted only if splitting eliminates every overflow allocator use.
//! This API admits scalar or flat arguments to a Stream or Reentrant entry,
//! with ordinary, reentrant or stream callees. Flat-input modules inline all
//! bytecode callees before splitting. [`lower`] chooses a uniform dialogue from
//! the entry return shape and first argument shape, or Unit without arguments.
//! [`lower_with_dialogues`] accepts independent output and reply shapes at each
//! reachable yield site, including delegated sites. Unused chunks do not impose
//! dialogue contracts or emit bodies. Original chunk and instruction indices
//! remain the host site identifiers. Whole-module verification and resource
//! admission still apply, as they do when constructing a VM. The host queries the suspended site
//! before interpreting its output or supplying a reply. These are native host
//! contracts, not restrictions imposed by the source language on reentrants.
//! Replies update only a Stream entry's first parameter, and must match its
//! declared shape. Reentrant entries retain their original arguments. A nested
//! stream clears its own locals on Reset. A consuming operation checks a
//! possibly cleared value before using its otherwise proven kind. Enum inspection
//! and scalar equality retain the VM's valid Unit behavior. Admission follows
//! actual reply tags through control flow and refuses unresolved type mixtures.
//! Composite construction packs actual operand sizes within a proven reservation.
//! Tuple and array lengths follow those values. Struct and enum slack is zeroed.
//! Field reads and host transfers check variable extents before access. Internal
//! calls carry bits, kinds and actual extents. Private body slots retain view
//! lengths and Unit values. Private scalar kinds persist with their payloads,
//! including across frame release. Zero a fresh private region before installing
//! its private initialization image. Ordering selects the actual numeric kind
//! and traps on unequal kinds. Arithmetic still requires one proven kind.
//! [`lower_with_contracts`] admits native composite results
//! with an explicit [`NativeBodyReturn`] contract. Snapshot results are copied
//! into bounded storage; instance borrows retain their aliases. Without a
//! contract a called native body result remains refused, including modules with
//! only scalar host values. Native implementations must uphold these lifetimes.
//!
//! The provisional start symbol is `kel_chunk_<entry_point>` with the scalar
//! arguments or i64 body pointers followed by shared, private, composite-region, frame and reply
//! pointers. It returns `{ptr, i64}`. Continuations take `(ptr, i1)` and return
//! that same pair. The value field is meaningful only with a non-null pointer.
//! Normal Reentrant completion returns a null continuation and writes its
//! final value into the reply cell. Owned flat results occupy a reserved tail
//! inside frame_bytes, outside LLVM's continuation storage. Borrowed results
//! retain their original region aliases. Read completed bodies before release
//! or the next start, while all supplied regions remain allocated and unchanged.
//! Keep the emitted code and instance buffers alive until release. Initialise the
//! ordinary data regions using their existing contracts before start. LLVM
//! initialises the frame itself; its previous bytes need not be cleared.
//! Never resume a released instance or invoke one continuation concurrently.
//!
//! Stable arena handles use `kel_coroutine_<entry>_start`, `_resume` and `_release`.
//! Reserve [`slot_bytes`] bytes aligned to [`FRAME_ALIGN`]. Start takes the normal
//! scalar arguments or body pointers, shared, private and composite-region pointers, then the slot.
//! Resume takes the same slot and an i64 reply bit pattern or body pointer. Both return
//! `{i64 status, i64 value}`. Status one marks a yielded value. Status two
//! reports normal Reentrant completion exactly once, with its final value.
//! Status zero returns a zero payload for an inactive instance. Release takes
//! the slot and returns nothing. It clears
//! the continuation, is idempotent, and makes subsequent resume return zeros.
//! Start may reuse a released slot but must never overwrite a live one. The slot
//! needs no prior initialization before start and must remain allocated and
//! unmoved until release. These provisional entry points use the same code and
//! ordinary-region lifetime contract as the raw interface above. They manage
//! continuation state, not allocation or ownership of the caller's arena pool.

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::Module as LlvmModule;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{BasicValue, CallSiteValue, FunctionValue, InstructionOpcode, PointerValue};
use keleusma::bytecode::{BlockType, Module, Op, TypeTag};

use crate::{LowerError, LowerOptions};

pub(crate) mod dialogue;
pub use dialogue::{Dialogue, HostContracts, NativeBodyReturn, YieldSite};
mod host;
pub(crate) mod kinds;
pub(crate) mod ownership;
pub(crate) mod packing;
pub(crate) mod private;
pub(crate) mod types;

/// Alignment required for the caller-provided coroutine frame.
pub const FRAME_ALIGN: u32 = 8;

/// Bytes for a stable host slot, including its continuation and reply header.
/// The slot requires [`FRAME_ALIGN`] alignment and remains reserved until release.
pub fn slot_bytes(frame_bytes: u32) -> Result<u32, LowerError> {
    if !(FRAME_ALIGN..=i32::MAX as u32).contains(&frame_bytes) {
        return Err(error(
            "coroutine frame size must be between 8 and i32::MAX bytes",
        ));
    }
    Ok(frame_bytes + host::HEADER_BYTES)
}

fn error(message: impl ToString) -> LowerError {
    LowerError::UnsupportedShape(message.to_string())
}

/// Emit and split a verified, resource-bounded coroutine for `machine`.
///
/// `frame_bytes` is the exact caller reservation, not an estimate of LLVM's
/// internal frame. A flat Reentrant result reserves its declared byte size,
/// or one byte for an empty body, at the tail of this reservation. LLVM may use
/// only the remaining bytes. An insufficient reservation is a lowering error. Buffer
/// bounds for shared, private and composite storage remain those published by
/// [`crate::region`]. They are additional to this frame and the eight-byte
/// reply cell. The returned module owns all emitted functions. Do not execute
/// partial output following an error.
pub fn lower<'ctx>(
    ctx: &'ctx Context,
    program: &Module,
    machine: &TargetMachine,
    frame_bytes: u32,
) -> Result<LlvmModule<'ctx>, LowerError> {
    lower_impl(
        ctx,
        program,
        machine,
        frame_bytes,
        None,
        &Default::default(),
    )
}

/// Lower with a contract for every Yield in chunks reachable from the entry.
/// Contracts for unused chunks are permitted but do not affect this instance.
/// The host reads `kel_coroutine_<entry>_yield_site(slot)` after status one to
/// select the output and reply shapes. Its result is YieldSite::id(), or
/// u64::MAX for an inactive slot. Query only an allocated slot initialized by start.
/// The raw interface stores this identifier in the final aligned eight bytes
/// of frame_bytes. This storage is excluded from LLVM's continuation frame.
pub fn lower_with_dialogues<'ctx>(
    ctx: &'ctx Context,
    program: &Module,
    machine: &TargetMachine,
    frame_bytes: u32,
    dialogues: &std::collections::BTreeMap<YieldSite, Dialogue>,
) -> Result<LlvmModule<'ctx>, LowerError> {
    lower_impl(
        ctx,
        program,
        machine,
        frame_bytes,
        Some(dialogues),
        &Default::default(),
    )
}

/// Lower with explicit dialogue and native body lifetime contracts.
/// Snapshot native results use the same bounded ownership transfers as host
/// replies. Instance borrows retain their original region aliases. The supplied
/// native functions must uphold the documented NativeBodyReturn obligations.
pub fn lower_with_contracts<'ctx>(
    ctx: &'ctx Context,
    program: &Module,
    machine: &TargetMachine,
    frame_bytes: u32,
    contracts: &HostContracts,
) -> Result<LlvmModule<'ctx>, LowerError> {
    lower_impl(
        ctx,
        program,
        machine,
        frame_bytes,
        contracts.dialogues.as_ref(),
        &contracts.native_body_returns,
    )
}

fn lower_impl<'ctx>(
    ctx: &'ctx Context,
    program: &Module,
    machine: &TargetMachine,
    frame_bytes: u32,
    dialogues: Option<&std::collections::BTreeMap<YieldSite, Dialogue>>,
    native_body_returns: &std::collections::BTreeMap<u16, NativeBodyReturn>,
) -> Result<LlvmModule<'ctx>, LowerError> {
    if frame_bytes < FRAME_ALIGN || frame_bytes > i32::MAX as u32 {
        return Err(error(
            "coroutine frame size must be between 8 and i32::MAX bytes",
        ));
    }
    keleusma::verify::verify(program).map_err(|e| error(format!("verification: {e:?}")))?;
    keleusma::vm::auto_arena_capacity_for(program, &[])
        .map_err(|e| error(format!("resource admission: {e:?}")))?;
    let entry = program
        .entry_point
        .ok_or_else(|| error("coroutine needs an entry point"))?;
    // The shared emitter checks allocation confinement for the coroutine
    // path itself. The legacy preflight cannot use proven unreachable arms.
    let stream = program
        .chunks
        .get(entry)
        .ok_or_else(|| error("coroutine entry is out of range"))?;
    if !matches!(stream.block_type, BlockType::Stream | BlockType::Reentrant)
        || stream.param_types.iter().any(|tag| {
            !matches!(
                tag,
                TypeTag::Word
                    | TypeTag::Byte
                    | TypeTag::Bool
                    | TypeTag::Fixed
                    | TypeTag::Float
                    | TypeTag::Unit
                    | TypeTag::Composite
            )
        })
        || (stream.block_type == BlockType::Stream
            && (!matches!(stream.ops.first(), Some(Op::Stream))
                || !matches!(stream.ops.last(), Some(Op::Reset))
                || stream.ops.iter().any(|op| matches!(op, Op::Return))))
    {
        return Err(error(
            "retcon requires a scalar or flat-input Stream or Reentrant entry; a Stream must end in Reset",
        ));
    }
    // Completion retains the entry result shape. Explicit dialogue outputs
    // and replies are independent of that signature.
    let result_shape = program
        .signatures
        .get(entry)
        .ok_or_else(|| error("coroutine needs an entry signature"))?
        .ret;
    for (tag, shape) in stream
        .param_types
        .iter()
        .zip(&program.signatures[entry].params)
    {
        if *tag == TypeTag::Composite
            && !matches!(
                shape,
                keleusma::bytecode::WireShape::Flat { kind: 0..=3, .. }
            )
        {
            return Err(error(
                "retcon requires a bounded flat composite input signature",
            ));
        }
    }
    let plan = dialogue::Plan::new(program, entry, dialogues, frame_bytes, native_body_returns)?;
    for (index, callee) in program.chunks.iter().enumerate() {
        if !plan.reachable[index] || index == entry || callee.block_type != BlockType::Stream {
            continue;
        }
        if !matches!(callee.ops.first(), Some(Op::Stream))
            || !matches!(callee.ops.last(), Some(Op::Reset))
            || callee.ops.iter().any(|op| matches!(op, Op::Return))
        {
            return Err(error(
                "a nested Stream must begin with Stream and end with Reset",
            ));
        }
    }
    let metadata_offset = plan.metadata_offset;
    // Completion outlives coro.end. Reserve a disjoint tail for owned flat
    // results, outside the bytes LLVM may use for the continuation frame.
    let completion_bytes = match (stream.block_type, result_shape) {
        (BlockType::Reentrant, keleusma::bytecode::WireShape::Flat { size, .. }) => size.max(1),
        _ => 0,
    };
    let continuation_bytes = metadata_offset
        .unwrap_or(frame_bytes)
        .checked_sub(completion_bytes)
        .filter(|bytes| *bytes >= FRAME_ALIGN)
        .ok_or_else(|| error("coroutine frame is too small for its completion reservation"))?;
    let owns_host_values = plan.owns_bodies;
    let analysis = types::check(program, entry, plan)?;
    let mut prepared = program.clone();
    for &(chunk, ip) in &analysis.false_inspections {
        // Both instructions leave the inspected value in place and push Bool.
        // Keeping instruction indices preserves all structured branch targets.
        prepared.chunks[chunk].ops[ip] = Op::PushImmediate(2);
    }
    let program = &prepared;
    let module = ctx.create_module("kel_retcon");
    module.set_triple(&machine.get_triple());
    module.set_data_layout(&machine.get_target_data().get_data_layout());
    crate::lower_module_with(
        ctx,
        &module,
        program,
        LowerOptions::default(),
        None,
        None,
        Some((continuation_bytes, &analysis)),
    )?;
    if !matches!(program.signatures.get(entry).map(|s| &s.ret),
        Some(keleusma::bytecode::WireShape::Scalar { kind }) if *kind <= keleusma::value_layout::ScalarKind::Float.to_tag())
        && !matches!(
            result_shape,
            keleusma::bytecode::WireShape::Flat { kind: 0..=3, .. }
        )
    {
        return Err(error(
            "retcon requires a scalar or flat entry result signature",
        ));
    }
    if owns_host_values {
        // Owned host bodies returned by ordinary callees must not escape a
        // machine-stack alloca. Inline every bytecode call before frame capture.
        for index in 0..program.chunks.len() {
            if index != entry && analysis.plan.reachable[index] {
                mark_delegate(
                    ctx,
                    module.get_function(&format!("kel_chunk_{index}")).unwrap(),
                );
            }
        }
    }
    // All allocations have static extents. Hoist them before inlining too,
    // otherwise LLVM brackets a callee's non-entry allocas with stacksave and
    // stackrestore. Restoring a pre-suspension C stack on resume is invalid.
    for function in module.get_functions() {
        hoist_allocas(ctx, function);
    }
    module.verify().map_err(error)?;
    // The verified call graph is acyclic. Inline suspension-capable callees
    // into the one coroutine before splitting, so LLVM captures each live
    // call frame and abnormal resume bypasses every remaining callee effect.
    module
        .run_passes(
            "always-inline,globaldce",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(error)?;
    if module.get_functions().any(is_delegate) {
        return Err(error("a suspension-capable callee survived inlining"));
    }
    // mem2reg promotes only entry-block allocas. The intrinsic scaffold
    // precedes the bytecode body, so move its fixed scalar slots to that entry.
    let f = module.get_function(&format!("kel_chunk_{entry}")).unwrap();
    let cleanup = f
        .get_basic_blocks()
        .into_iter()
        .find(|b| b.get_name().to_bytes() == b"retcon.cleanup")
        .unwrap();
    // Retcon requires one fallthrough coro.end. Redirect inlined abnormal
    // resume exits to the entry's cleanup, before the coroutine passes run.
    let cleanup_builder = ctx.create_builder();
    for block in f.get_basic_blocks().into_iter().filter(|b| *b != cleanup) {
        let ends: Vec<_> = block
            .get_instructions()
            .filter(|i| {
                CallSiteValue::try_from(*i)
                    .ok()
                    .and_then(|c| c.get_called_fn_value())
                    .is_some_and(|callee| callee.get_name().to_bytes() == b"llvm.coro.end")
            })
            .collect();
        for end in ends {
            let terminal = end
                .get_next_instruction()
                .ok_or_else(|| error("coroutine cleanup has no terminator"))?;
            if terminal.get_opcode() != InstructionOpcode::Unreachable {
                return Err(error("coroutine cleanup has effects after coro.end"));
            }
            terminal.erase_from_basic_block();
            end.erase_from_basic_block();
            cleanup_builder.position_at_end(block);
            cleanup_builder.build_unconditional_branch(cleanup).unwrap();
        }
    }
    hoist_allocas(ctx, f);
    module.verify().map_err(error)?;
    // Promote scalar slots before splitting, so only values actually live at
    // suspension occupy the frame. No handwritten resume dispatch is emitted.
    module
        .run_passes(
            "function(sroa,mem2reg),coro-early,coro-split,coro-cleanup",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(error)?;
    module.verify().map_err(error)?;
    if module
        .get_function("kel_retcon_overflow")
        .is_some_and(|f| f.as_global_value().get_first_use().is_some())
    {
        return Err(error("coroutine frame exceeds the caller reservation"));
    }
    if module
        .get_function("kel_retcon_free")
        .is_some_and(|f| f.as_global_value().get_first_use().is_some())
    {
        return Err(error("coroutine retained an unexpected deallocator use"));
    }
    if module.get_functions().any(|f| {
        (f.get_name().to_bytes().starts_with(b"llvm.coro.")
            || f.get_name().to_bytes().starts_with(b"llvm.stacksave")
            || f.get_name().to_bytes().starts_with(b"llvm.stackrestore"))
            && f.as_global_value().get_first_use().is_some()
    }) {
        return Err(error(
            "coroutine or dynamic-stack intrinsic survived splitting",
        ));
    }
    host::emit(
        ctx,
        &module,
        f,
        entry,
        stream.block_type == BlockType::Reentrant,
        metadata_offset,
    );
    module.verify().map_err(error)?;
    Ok(module)
}

fn hoist_allocas(ctx: &Context, function: FunctionValue<'_>) {
    let Some(header) = function.get_first_basic_block() else {
        return;
    };
    let Some(first) = header
        .get_instructions()
        .find(|i| i.get_opcode() != InstructionOpcode::Alloca)
    else {
        return;
    };
    let builder = ctx.create_builder();
    builder.position_before(&first);
    let allocas: Vec<_> = function
        .get_basic_blocks()
        .iter()
        .flat_map(|b| b.get_instructions())
        .filter(|i| i.get_opcode() == InstructionOpcode::Alloca)
        .collect();
    for alloca in allocas {
        alloca.remove_from_basic_block();
        builder.insert_instruction(&alloca, None);
    }
}

pub(crate) fn declare<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    index: usize,
    params: &[BasicMetadataTypeEnum<'ctx>],
    frame_bytes: u32,
) -> Result<FunctionValue<'ctx>, LowerError> {
    let mut arguments: Vec<_> = params
        .iter()
        .enumerate()
        .map(|(i, ty)| format!("{} %arg{i}", ty.print_to_string().to_string()))
        .collect();
    arguments.push("ptr %buffer".into());
    arguments.push("ptr %reply".into());
    // Tokens are not BasicValueEnum values in inkwell. Parse only the fixed
    // intrinsic scaffold, then emit all bytecode operations through the shared
    // builder. No user strings are interpolated into this fragment.
    let mut ir = format!(
        r#"
declare token @llvm.coro.id.retcon(i32, i32, ptr, ptr, ptr, ptr)
declare ptr @llvm.coro.begin(token, ptr)
declare i1 @llvm.coro.suspend.retcon.i1(...)
declare void @llvm.coro.end(ptr, i1, token)
declare ptr @kel_retcon_overflow(i32)
declare void @kel_retcon_free(ptr)
declare {{ptr, i64}} @kel_retcon_prototype(ptr, i1)
define {{ptr, i64}} @kel_chunk_{index}({args}) presplitcoroutine {{
retcon.header:
  %id = call token @llvm.coro.id.retcon(i32 {frame_bytes}, i32 {FRAME_ALIGN}, ptr %buffer, ptr @kel_retcon_prototype, ptr @kel_retcon_overflow, ptr @kel_retcon_free)
  %hdl = call ptr @llvm.coro.begin(token %id, ptr null)
  br label %retcon.cleanup
retcon.cleanup:
  call void @llvm.coro.end(ptr %hdl, i1 false, token none)
  unreachable
}}
"#,
        args = arguments.join(", ")
    );
    ir.push('\0');
    let buffer = MemoryBuffer::create_from_memory_range_copy(ir.as_bytes(), "retcon scaffold");
    let scaffold = ctx.create_module_from_ir(buffer).map_err(error)?;
    module.link_in_module(scaffold).map_err(error)?;
    Ok(module.get_function(&format!("kel_chunk_{index}")).unwrap())
}

/// Hidden context belongs to the entry instance, never to a callee's locals.
/// Scalar replacement removes these pointer slots before frame splitting.
pub(crate) fn delegate_context<'ctx>(
    ctx: &'ctx Context,
    builder: &Builder<'ctx>,
    reply: PointerValue<'ctx>,
    entry_parameter: PointerValue<'ctx>,
    latest_reply: PointerValue<'ctx>,
    entry_owned: Option<PointerValue<'ctx>>,
    site: Option<PointerValue<'ctx>>,
) -> PointerValue<'ctx> {
    let ptr = ctx.ptr_type(inkwell::AddressSpace::default());
    let ty = ctx.struct_type(&[ptr.into(); 5], false);
    let context = builder.build_alloca(ty, "delegate_context").unwrap();
    for (index, value) in [
        reply,
        entry_parameter,
        latest_reply,
        entry_owned.unwrap_or_else(|| {
            builder
                .build_alloca(ctx.i64_type(), "unused_entry_ownership")
                .unwrap()
        }),
        site.unwrap_or(ptr.const_null()),
    ]
    .into_iter()
    .enumerate()
    {
        let field = builder
            .build_struct_gep(ty, context, index as u32, "context_field")
            .unwrap();
        builder.build_store(field, value).unwrap();
    }
    context
}

pub(crate) fn mark_delegate(ctx: &Context, function: FunctionValue<'_>) {
    function.set_linkage(inkwell::module::Linkage::Internal);
    function.add_attribute(
        AttributeLoc::Function,
        ctx.create_enum_attribute(Attribute::get_named_enum_kind_id("alwaysinline"), 0),
    );
    function.add_attribute(
        AttributeLoc::Function,
        ctx.create_string_attribute("kel.retcon.delegate", ""),
    );
}

pub(crate) fn is_delegate(function: FunctionValue<'_>) -> bool {
    function
        .get_string_attribute(AttributeLoc::Function, "kel.retcon.delegate")
        .is_some()
}

pub(crate) fn delegate_yield<'ctx>(
    ctx: &'ctx Context,
    module: &LlvmModule<'ctx>,
    resume_shape: Option<keleusma::bytecode::WireShape>,
    site: u64,
) -> Result<FunctionValue<'ctx>, LowerError> {
    let name = format!("kel_retcon_delegate_yield_{site}");
    if let Some(function) = module.get_function(&name) {
        return Ok(function);
    }
    // After inlining, lower redirects this helper's abnormal exit to the
    // enclosing coroutine's single cleanup block. No helper survives splitting.
    let ir = r#"
declare i1 @llvm.coro.suspend.retcon.i1(...)
declare void @llvm.coro.end(ptr, i1, token)
define i64 @kel_retcon_delegate_yield(i64 %value, ptr %context) alwaysinline {
entry:
  %reply_slot = getelementptr {ptr, ptr, ptr, ptr}, ptr %context, i32 0, i32 0
  %parameter_slot = getelementptr {ptr, ptr, ptr, ptr}, ptr %context, i32 0, i32 1
  %latest_slot = getelementptr {ptr, ptr, ptr, ptr}, ptr %context, i32 0, i32 2
  %reply = load ptr, ptr %reply_slot
  %parameter = load ptr, ptr %parameter_slot
  %latest = load ptr, ptr %latest_slot
  %unwind = call i1 (...) @llvm.coro.suspend.retcon.i1(i64 %value)
  br i1 %unwind, label %cleanup, label %resume
cleanup:
  call void @llvm.coro.end(ptr null, i1 false, token none)
  unreachable
resume:
  %result = load i64, ptr %reply
  store i64 %result, ptr %parameter
  store i64 %result, ptr %latest
  ret i64 %result
}
"#;
    let ir = ir
        .replace("kel_retcon_delegate_yield", &name)
        .replace("{ptr, ptr, ptr, ptr}", "{ptr, ptr, ptr, ptr, ptr}");
    let buffer = MemoryBuffer::create_from_memory_range_copy(
        format!("{ir}\0").as_bytes(),
        "delegate scaffold",
    );
    module
        .link_in_module(ctx.create_module_from_ir(buffer).map_err(error)?)
        .map_err(error)?;
    let function = module.get_function(&name).unwrap();
    if let Some(keleusma::bytecode::WireShape::Flat { size, .. }) = resume_shape {
        let resume = function
            .get_basic_blocks()
            .into_iter()
            .find(|b| b.get_name().to_bytes() == b"resume")
            .unwrap();
        // Replace the scalar stores after the reply load with distinct owned
        // values for the entry parameter and latest-reply slot.
        let result = resume.get_first_instruction().unwrap();
        while let Some(next) = result.get_next_instruction() {
            next.erase_from_basic_block();
        }
        let b = ctx.create_builder();
        b.position_at_end(resume);
        let context = function.get_nth_param(1).unwrap().into_pointer_value();
        let ptr = ctx.ptr_type(inkwell::AddressSpace::default());
        let ty = ctx.struct_type(&[ptr.into(); 5], false);
        let bits = result.try_into().unwrap();
        for index in [1, 2] {
            let slot = b
                .build_struct_gep(ty, context, index, "owned_reply_slot")
                .unwrap();
            let dst = b
                .build_load(ptr, slot, "owned_reply_destination")
                .unwrap()
                .into_pointer_value();
            let value = ownership::copy(
                &b,
                bits,
                crate::Width::Body(size),
                ctx.i64_type().const_int(1, false),
            );
            b.build_store(dst, value).unwrap();
        }
        let slot = b
            .build_struct_gep(ty, context, 3, "entry_ownership_slot")
            .unwrap();
        let dst = b
            .build_load(ptr, slot, "entry_ownership")
            .unwrap()
            .into_pointer_value();
        b.build_store(dst, ctx.i64_type().const_int(1, false))
            .unwrap();
        b.build_return(Some(&bits)).unwrap();
    }
    mark_delegate(ctx, function);
    Ok(function)
}
