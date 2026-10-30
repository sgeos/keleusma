//! Per-target harness bodies shared by the cargo-fuzz targets and the corpus replay test.
//!
//! # Why this file is shared by path
//!
//! The fuzz targets under `fuzz/fuzz_targets/` and the ordinary test
//! `tests/fuzz_corpus_replay.rs` both include this file with `#[path]`. A replay that
//! called different code from the fuzzer would replay nothing the fuzzer found. Sharing
//! the source keeps the two in agreement without adding a test-support surface to the
//! library's public interface.
//!
//! # The contract under test
//!
//! Every function here accepts arbitrary bytes and returns. A panic, an abort, or a
//! failure to terminate is the defect being searched for. No function asserts that an
//! input is accepted or rejected, because a hostile input is entitled to either outcome.
//! Each function reports the deepest stage the input reached as a [`Reach`], so that the
//! replay test can fail when a corpus stops reaching the stages it was chosen to reach.
//!
//! # How each run is bounded
//!
//! The virtual machine has no step or fuel budget. Inputs are therefore capped in length
//! before any work is done, and bytecode is executed only when the module has a single
//! chunk, which cannot call another script chunk, and the verifier's own worst-case
//! execution time for that chunk is at most [`MAX_EXECUTED_WCET`]. A run that then fails
//! to finish is itself a finding, since it would contradict the verified bound.
//!
//! # Trust boundary
//!
//! All input is untrusted. The fixed signing key below is a test key whose secret is
//! public in this file. It authenticates nothing outside the fuzz harness.

use keleusma::bytecode::{BlockType, Chunk, ConstValue, Module, NOMINAL_COST_MODEL};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};
use keleusma::wire_format::{OpcodeId, OpcodeRecord, decode_op};
use keleusma::{Arena, verify};

/// The longest bytecode input any target examines. Longer inputs are truncated.
pub const MAX_BYTECODE_LEN: usize = 64 * 1024;

/// The longest source input the front-end target examines. Longer inputs are truncated.
pub const MAX_SOURCE_LEN: usize = 16 * 1024;

/// The most instructions a synthesized chunk carries.
pub const MAX_SYNTH_OPS: usize = 1024;

/// The largest verified worst-case execution time, in the nominal cost model's cycles, of
/// a chunk the loader target will execute.
pub const MAX_EXECUTED_WCET: u32 = 1_000_000;

/// One past the highest opcode identifier the synthesizer emits. It deliberately exceeds
/// the instruction set's 66 opcodes so that unknown identifiers reach the decoder.
const SYNTH_OPCODE_SPACE: u8 = 72;

/// The fixed test signing key seed. Public by construction, see the module documentation.
pub const TEST_SIGNING_SEED: [u8; 32] = [0x4B; 32];

/// The deepest stage one input reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reach {
    /// Refused before any decoding, or by the encoder for a synthesized module.
    Refused,
    /// Decoded into a module or a token stream and then refused.
    Decoded,
    /// Accepted by the structural verifier, or parsed into a program.
    Verified,
    /// Constructed into a virtual machine.
    Loaded,
    /// Constructed and executed for one call.
    Executed,
}

/// Truncates `data` to at most `cap` bytes.
fn capped(data: &[u8], cap: usize) -> &[u8] {
    &data[..data.len().min(cap)]
}

/// Returns a copy of `bytes` whose whole-artifact cyclic redundancy check trailer is
/// recomputed, when the declared total length makes that possible.
///
/// A byte-level mutation of an artifact is otherwise refused by the trailer check before
/// any decoder runs, so the fuzzer would measure the checksum and nothing else. An
/// attacker computes a valid trailer as easily as the compiler does, so resealing is the
/// faithful model. The function assumes the artifact carries no shebang prefix, which
/// holds for every committed seed. Inputs that do carry one are still examined unsealed.
pub fn resealed(bytes: &[u8]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    if out.len() < 12 {
        return out;
    }
    let total = u32::from_le_bytes([out[8], out[9], out[10], out[11]]) as usize;
    if total < 16 || total > out.len() {
        return out;
    }
    let crc = keleusma_wire::crc32(&out[..total - 4]);
    out[total - 4..total].copy_from_slice(&crc.to_le_bytes());
    out
}

/// Decodes `bytes` as an artifact, first as given and then resealed.
fn decode_either(bytes: &[u8]) -> Option<Module> {
    Module::from_bytes(bytes)
        .ok()
        .or_else(|| Module::from_bytes(&resealed(bytes)).ok())
}

/// Builds a one-chunk module from raw bytes and passes it through the encoder and the
/// decoder, so that only a module the loader could itself produce is returned.
///
/// The first byte selects the block type, the second the local count and the third the
/// parameter count. Every following group of four bytes is one opcode record, an
/// identifier and three operand bytes, decoded with an empty operand pool. A record the
/// decoder refuses is skipped rather than ending the chunk.
fn synthesized(bytes: &[u8]) -> Option<Module> {
    if bytes.len() < 3 {
        return None;
    }
    let block_type = match bytes[0] % 3 {
        0 => BlockType::Func,
        1 => BlockType::Reentrant,
        _ => BlockType::Stream,
    };
    let mut ops = Vec::new();
    for rec in bytes[3..].as_chunks::<4>().0.iter().take(MAX_SYNTH_OPS) {
        let id = OpcodeId(rec[0] % SYNTH_OPCODE_SPACE);
        let record = OpcodeRecord::from_id_and_operand(id, [rec[1], rec[2], rec[3]]);
        if let Ok(op) = decode_op(record, &[]) {
            ops.push(op);
        }
    }
    let param_count = bytes[2] % 4;
    let chunk = Chunk {
        name: String::from("main"),
        ops,
        constants: vec![
            ConstValue::Int(0),
            ConstValue::Int(1),
            ConstValue::Bool(true),
        ],
        struct_templates: Vec::new(),
        local_count: u16::from(bytes[1] % 16).max(u16::from(param_count)),
        param_count,
        block_type,
        param_types: Vec::new(),
        debug_pool: None,
    };
    let module = Module {
        schema_hash: 0,
        enum_layouts: Vec::new(),
        signatures: Vec::new(),
        native_return_shapes: Vec::new(),
        chunks: vec![chunk],
        native_names: Vec::new(),
        entry_point: Some(0),
        data_layout: None,
        word_bits_log2: keleusma::bytecode::RUNTIME_WORD_BITS_LOG2,
        addr_bits_log2: keleusma::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
        float_bits_log2: keleusma::bytecode::RUNTIME_FLOAT_BITS_LOG2,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        aux_arena_bytes: 0,
        persistent_composite_bytes: 0,
        flags: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
    };
    let encoded = module.to_bytes().ok()?;
    Module::from_bytes(&encoded).ok()
}

/// The module a structured target examines. A decodable artifact is preferred, and any
/// other input is synthesized into a module.
fn module_for(data: &[u8]) -> Option<Module> {
    let data = capped(data, MAX_BYTECODE_LEN);
    decode_either(data).or_else(|| synthesized(data))
}

/// Whether executing `module` once is bounded by a verified bound small enough to run.
fn executable_within_budget(module: &Module) -> bool {
    let [chunk] = module.chunks.as_slice() else {
        return false;
    };
    if module.entry_point != Some(0)
        || chunk.block_type != BlockType::Func
        || chunk.param_count != 0
        || !module.native_names.is_empty()
    {
        return false;
    }
    matches!(
        verify::module_wcet_with_bounds(module, &[], &NOMINAL_COST_MODEL).as_deref(),
        Ok([wcet]) if *wcet <= MAX_EXECUTED_WCET
    )
}

/// Loads one artifact through the unsigned loader and, when bounded, executes it once.
fn load_one(bytes: &[u8]) -> Reach {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let Ok(mut vm) = Vm::load_bytes(bytes, &arena) else {
        return match Module::from_bytes(bytes) {
            Ok(_) => Reach::Decoded,
            Err(_) => Reach::Refused,
        };
    };
    let Ok(module) = Module::from_bytes(bytes) else {
        // The loader accepted bytes the decoder alone refuses. That is not a panic, so
        // it is reported as loaded and left for a reader of the census to notice.
        return Reach::Loaded;
    };
    if !executable_within_budget(&module) {
        return Reach::Loaded;
    }
    let _ = vm.call(&[]);
    Reach::Executed
}

/// Target `load_bytes`. Runs [`Vm::load_bytes`] on the input as given and resealed.
pub fn load_bytes(data: &[u8]) -> Reach {
    let data = capped(data, MAX_BYTECODE_LEN);
    let raw = load_one(data);
    let sealed = load_one(&resealed(data));
    raw.max(sealed)
}

/// The verifying key matching [`TEST_SIGNING_SEED`].
#[cfg(feature = "signatures")]
pub fn test_verifying_key() -> ed25519_dalek::VerifyingKey {
    ed25519_dalek::SigningKey::from_bytes(&TEST_SIGNING_SEED).verifying_key()
}

/// Loads one artifact through the signed loader with an empty and a one-key trust matrix.
#[cfg(feature = "signatures")]
fn load_signed_one(bytes: &[u8]) -> Reach {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let keys = [test_verifying_key()];
    let empty = Vm::load_signed_bytes(bytes, &arena, &[]).is_ok();
    let keyed = Vm::load_signed_bytes(bytes, &arena, &keys).is_ok();
    if empty || keyed {
        Reach::Loaded
    } else if Module::from_bytes(bytes).is_ok() {
        Reach::Decoded
    } else {
        Reach::Refused
    }
}

/// Target `load_signed`. Runs [`Vm::load_signed_bytes`] on the input as given and
/// resealed. Resealing does not forge a signature, because the signature covers the
/// buffer with the trailer zeroed, so a mutated signed artifact is still expected to be
/// refused at signature verification.
#[cfg(feature = "signatures")]
pub fn load_signed(data: &[u8]) -> Reach {
    let data = capped(data, MAX_BYTECODE_LEN);
    let raw = load_signed_one(data);
    let sealed = load_signed_one(&resealed(data));
    raw.max(sealed)
}

/// Target `verify_module`. Runs [`verify::verify`] on a decoded or synthesized module.
pub fn verify_module(data: &[u8]) -> Reach {
    let Some(module) = module_for(data) else {
        return Reach::Refused;
    };
    match verify::verify(&module) {
        Ok(()) => Reach::Verified,
        Err(_) => Reach::Decoded,
    }
}

/// Target `module_bounds`. Runs the worst-case execution time and memory entries on a
/// decoded or synthesized module.
///
/// The entries run whether or not [`verify::verify`] accepts the module. Each is public
/// and documented to return an error for a chunk it cannot bound, and
/// `tests/verify_hostile_termination.rs` already holds them to termination on unverified
/// input. A panic found only on an unverified module is still reported, with that
/// qualification.
pub fn module_bounds(data: &[u8]) -> Reach {
    let Some(module) = module_for(data) else {
        return Reach::Refused;
    };
    let natives = module.native_names.len();
    let bounds = vec![verify::NativeIterationBound::default(); natives];
    let native_wcmu = vec![0u32; natives];
    let wcet = verify::module_wcet_with_bounds(&module, &bounds, &NOMINAL_COST_MODEL);
    let wcmu = verify::module_wcmu(&module, &native_wcmu);
    let footprint = verify::module_runtime_footprint(&module, &native_wcmu);
    let depth = verify::module_call_depth(&module);
    if wcet.is_ok() && wcmu.is_ok() && footprint.is_ok() && depth.is_ok() {
        Reach::Verified
    } else {
        Reach::Decoded
    }
}

/// Lexes and parses one source text.
fn front_end_one(source: &str) -> Reach {
    let Ok(tokens) = keleusma::lexer::tokenize(source) else {
        return Reach::Refused;
    };
    match keleusma::parser::parse(&tokens) {
        Ok(_) => Reach::Verified,
        Err(_) => Reach::Decoded,
    }
}

/// Target `front_end`. Runs the lexer and the parser on the input when it is valid UTF-8,
/// and on its lossy UTF-8 conversion otherwise, so that arbitrary bytes reach both.
pub fn front_end(data: &[u8]) -> Reach {
    let data = capped(data, MAX_SOURCE_LEN);
    match core::str::from_utf8(data) {
        Ok(source) => front_end_one(source),
        Err(_) => front_end_one(&String::from_utf8_lossy(data)),
    }
}
