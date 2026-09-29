//! Returned-continuation lowering of bytecode streams.
//!
//! The caller owns the frame, reply cell, and the ordinary shared, private and
//! composite regions as disjoint reservations for the entire lifetime of a suspended instance. Start
//! returns a continuation and a yielded scalar bits. Before each resume the caller
//! writes the next scalar bits to the reply cell, then calls the continuation with
//! `(frame, false)`. Release calls it with `(frame, true)` and invalidates the
//! instance. Float bits occupy the low 32 or all 64 bits according to the
//! module width. Byte and Boolean inputs must be in their declared ranges.
//! The start argument retains its normal native type. A released frame may be reused; a live frame may not be moved.
//!
//! LLVM manages the continuation and captures live allocas. The emitted module
//! is accepted only if splitting eliminates every overflow allocator use.
//! This API currently admits one scalar stream with non-suspending callees.
//!
//! The provisional start symbol is `kel_chunk_<entry_point>` with the scalar
//! argument followed by shared, private, composite-region, frame and reply
//! pointers. It returns `{ptr, i64}`. Continuations take `(ptr, i1)` and return
//! that same pair. The value field is meaningful only with a non-null pointer.
//! Keep the emitted code and all buffers alive until release. Initialise the
//! ordinary data regions using their existing contracts before start. LLVM
//! initialises the frame itself; its previous bytes need not be cleared.
//! Never resume a released instance or invoke one continuation concurrently.

use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use inkwell::module::Module as LlvmModule;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::values::{BasicValue, FunctionValue, InstructionOpcode};
use keleusma::bytecode::{BlockType, Module, Op, TypeTag};

use crate::{LowerError, LowerOptions};

/// Alignment required for the caller-provided coroutine frame.
pub const FRAME_ALIGN: u32 = 8;

fn error(message: impl ToString) -> LowerError {
    LowerError::UnsupportedShape(message.to_string())
}

/// Emit and split a verified, resource-bounded scalar stream for `machine`.
///
/// `frame_bytes` is the exact caller reservation, not an estimate of LLVM's
/// internal frame. An insufficient reservation is a lowering error. Buffer
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
    if frame_bytes < FRAME_ALIGN || frame_bytes > i32::MAX as u32 {
        return Err(error(
            "coroutine frame size must be between 8 and i32::MAX bytes",
        ));
    }
    keleusma::verify::verify(program).map_err(|e| error(format!("verification: {e:?}")))?;
    keleusma::vm::auto_arena_capacity_for(program, &[])
        .map_err(|e| error(format!("resource admission: {e:?}")))?;
    // Reuse the established safety refusals, including composite yield escape.
    // Passing verification alone does not certify fixed-offset native storage.
    let preflight = ctx.create_module("retcon_preflight");
    crate::lower_module(ctx, &preflight, program, LowerOptions::default())?;
    let entry = program
        .entry_point
        .ok_or_else(|| error("coroutine needs an entry point"))?;
    let stream = program
        .chunks
        .get(entry)
        .ok_or_else(|| error("coroutine entry is out of range"))?;
    if stream.block_type != BlockType::Stream
        || !matches!(
            stream.param_types.as_slice(),
            [TypeTag::Word
                | TypeTag::Byte
                | TypeTag::Bool
                | TypeTag::Fixed
                | TypeTag::Float
                | TypeTag::Unit]
        )
        || stream.param_count != 1
        || !matches!(stream.ops.first(), Some(Op::Stream))
        || !matches!(stream.ops.last(), Some(Op::Reset))
        || stream.ops.iter().any(|op| matches!(op, Op::Return))
    {
        return Err(error(
            "retcon currently requires a one-scalar Stream entry ending in Reset",
        ));
    }
    if program.chunks.iter().enumerate().any(|(i, c)| {
        i != entry
            && (c.block_type != BlockType::Func || c.ops.iter().any(|op| matches!(op, Op::Yield)))
    }) {
        return Err(error("retcon currently requires non-suspending callees"));
    }
    if !matches!(program.signatures.get(entry).map(|s| &s.ret),
        Some(keleusma::bytecode::WireShape::Scalar { kind }) if *kind <= keleusma::value_layout::ScalarKind::Float.to_tag())
    {
        return Err(error("retcon currently requires a scalar yield signature"));
    }
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
        Some(frame_bytes),
    )?;
    // mem2reg promotes only entry-block allocas. The intrinsic scaffold
    // precedes the bytecode body, so move its fixed scalar slots to that entry.
    let f = module.get_function(&format!("kel_chunk_{entry}")).unwrap();
    let header = f.get_first_basic_block().unwrap();
    let builder = ctx.create_builder();
    builder.position_before(&header.get_first_instruction().unwrap());
    let allocas: Vec<_> = f
        .get_basic_blocks()
        .iter()
        .flat_map(|b| b.get_instructions())
        .filter(|i| i.get_opcode() == InstructionOpcode::Alloca)
        .collect();
    for alloca in allocas {
        alloca.remove_from_basic_block();
        builder.insert_instruction(&alloca, None);
    }
    module.verify().map_err(error)?;
    // Promote scalar slots before splitting, so only values actually live at
    // suspension occupy the frame. No handwritten resume dispatch is emitted.
    module
        .run_passes(
            "function(mem2reg),coro-early,coro-split,coro-cleanup",
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
        f.get_name().to_bytes().starts_with(b"llvm.coro.")
            && f.as_global_value().get_first_use().is_some()
    }) {
        return Err(error("coroutine intrinsic survived splitting"));
    }
    Ok(module)
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
