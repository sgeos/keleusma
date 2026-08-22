//! The self-hosted compile pipeline as a reusable library (gated by the `self-host`
//! feature).
//!
//! This drives the Keleusma stages over a source with the host only moving data between
//! stages; the compile logic is Keleusma end to end. `kel/lexer.kel` tokenizes,
//! `kel/parse.kel` emits a postorder record stream, `kel/reconstruct.kel` folds that
//! into the (kind, arg, lhs, rhs) node forest, `kel/codegen.kel` emits each chunk's ops,
//! and `kel/analyze.kel` supplies the declared WCET/WCMU header; the `kel/verify_*.kel`
//! family provides the self-hosted verifier drivers. The [`crate::selfhost::self_host_compile`] entry
//! splices only the self-hosted chunk ops onto the reference scaffold, whereas
//! [`crate::selfhost::self_host_compile_scratch`] assembles the whole module (data layout, enum-layout
//! table, chunk signatures, schema hash, WCET/WCMU header) from the pipeline output, so
//! for the loop-free stage sources its serialized module is byte-identical to the
//! reference without borrowing any field from it.
//!
//! [`crate::selfhost::self_hosted_compile`] is the shipping entry: it host-guards the target and maps any
//! out-of-subset panic to a clean [`crate::selfhost::SelfHostError`] (the `keleusma-cli` `--compiler
//! self-hosted` backend). All ten Rust-read stage sources are embedded via `include_str!`
//! from `src/selfhost/kel/` (this crate is their canonical home; the detached `compiler/`
//! subproject re-exports this module and its `main.rs`/tests drive it). `prelude.kel` is
//! not read by the Rust host and stays in `compiler/kel/`.
//!
//! `src/selfhost/kel/` also holds **`wire.kel`, which is not one of those ten**. It is the
//! wire format written in Keleusma (step 6 of the wire-format programme), and it IS in the
//! `read_stage` table: it joined when the driver gained a path that emits through it, which
//! is `wire_names_via_kel` below. This paragraph said the opposite — "deliberately absent
//! from the `read_stage` table" — for as long as the entry existed fifty lines below it, so
//! the file contradicted itself and a reader could not tell which half to trust. See
//! `docs/decisions/WIRE_FORMAT_SELFHOST_PLAN.md`.

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::ops::ControlFlow;

use crate::bytecode::{
    ArrayElem, ConstValue, EnumField, Module, NewCompositeOperand, Op, StructField, TupleField,
    Value,
};
use crate::value_layout::{CompositeKind, ScalarKind};
use crate::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
use crate::{Arena, compiler::compile, lexer::tokenize, parser::parse};

/// Read a stage source by basename, resolving the two ownership families from either
/// the subproject directory or the workspace root.
///
/// The four pipeline stages moved into the parent crate at `src/selfhost/kel/`; the
/// subproject-only sources stay in `compiler/kel/`. `rel` is a `kel/<name>.kel` path;
/// only its basename is significant. Each candidate is tried in order; the first that
/// reads wins.
fn read_stage(rel: &str) -> String {
    // All ten Rust-read stage sources are embedded in the crate at `src/selfhost/kel/`
    // (beside this module) via `include_str!`, so the driver needs no filesystem access
    // and works in an installed binary. `rel` is a `kel/<name>.kel` path; only the
    // basename is significant.
    let base = rel.rsplit('/').next().unwrap_or(rel);
    let s: &str = match base {
        "lexer.kel" => include_str!("kel/lexer.kel"),
        "parse.kel" => include_str!("kel/parse.kel"),
        "reconstruct.kel" => include_str!("kel/reconstruct.kel"),
        "codegen.kel" => include_str!("kel/codegen.kel"),
        "analyze.kel" => include_str!("kel/analyze.kel"),
        "verify_structural.kel" => include_str!("kel/verify_structural.kel"),
        "verify_yield.kel" => include_str!("kel/verify_yield.kel"),
        "verify_depth.kel" => include_str!("kel/verify_depth.kel"),
        "verify_typed.kel" => include_str!("kel/verify_typed.kel"),
        "verify_datalayout.kel" => include_str!("kel/verify_datalayout.kel"),
        // JOINED THE TABLE when the driver gained a path that emits through it
        // (`wire_names_via_kel` below). The criterion this module has carried
        // since step 6 was "when it produces bytes rather than a checksum"; it
        // produced bytes in the tests some time before the DRIVER could ask it
        // to, and an entry added then would have recorded a capability the
        // system did not have.
        "wire.kel" => include_str!("kel/wire.kel"),
        other => panic!("unknown embedded stage source `{other}`"),
    };
    s.to_string()
}

/// Compile a stage or program source with the Rust-hosted reference compiler.
pub fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// A reconstructed AST node in the flat `(kind, arg, lhs, rhs)` forest that
/// `reconstruct.kel` emits and `codegen.kel` consumes. Public so the detached
/// `compiler/` subproject (which re-exports this module) can drive the stages.
pub struct Node {
    kind: i64,
    arg: i64,
    lhs: i64,
    rhs: i64,
}

/// A flattened function body: the flattening context and the body's block root.
pub struct Body {
    nodes: Vec<Node>,
    call_args: Vec<i64>,
    for_parts: Vec<i64>,
    match_parts: Vec<i64>,
    limit_parts: Vec<i64>,
    head_parts: Vec<i64>,
    category: i64,
    root: i64,
}

/// A parsed function from the record stream: parser category (1 fn, 2 yield, 3 loop),
/// name id, value-parameter count, and the postorder records of its `when` guard (empty
/// when unguarded) and its body.
pub struct ParsedFn {
    cat: i64,
    name: i64,
    params: usize,
    // The type-name id of each value parameter (from the header PTYPE records) and of
    // the return (the RETTYPE record), for the driver's own chunk-signature assembly.
    param_types: Vec<i64>,
    return_type: i64,
    guard: Vec<(i64, i64)>,
    body: Vec<(i64, i64)>,
    /// Each value parameter's interned name id, in declaration order.
    ///
    /// **The record already carried this and the driver threw it away**: the header
    /// emits `4 + name * 64`, and the arm read the code and discarded the payload
    /// because a count was all anything needed. Parameters are half the bindings a
    /// type check has to resolve, so the name is kept now rather than recovered.
    param_names: Vec<i64>,
    /// Each `let` binding's `(frame slot, interned name id)`, in fold order.
    ///
    /// **Kept OUT of `body`.** The record stream `body` holds is what
    /// `reconstruct.kel` consumes and what the parse tests pin against the
    /// reference, so a new record kind inside it would change both. Diverting the
    /// binding-name record here leaves the node stream byte-for-byte as it was.
    let_names: Vec<(i64, i64)>,
}

impl ParsedFn {
    /// The declaration category (`fn`, `yield`, and so on) as `reconstruct.kel`
    /// consumes it.
    pub fn category(&self) -> i64 {
        self.cat
    }

    /// The number of value parameters this head declares.
    pub fn param_count(&self) -> usize {
        self.params
    }

    /// The head's `when` guard as a record stream, empty when it has none.
    pub fn guard_records(&self) -> &[(i64, i64)] {
        &self.guard
    }

    /// The head's body as a record stream.
    ///
    /// **Accessors rather than `pub` fields, deliberately.** The `v0.3.0` line
    /// needs a record stream to call [`seed_reconstruct_shared`] and asked us to
    /// choose between opening the fields and exposing a reader. Opening them
    /// would freeze the parse representation as public API, and this stage's
    /// input shape has already changed once. A reader hands out the same bytes
    /// while leaving the layout ours to change.
    ///
    /// **[`seed_reconstruct_multihead_shared`] never needed this.** It takes
    /// `&[&ParsedFn]`, and [`parse_functions`] is public and returns
    /// `Vec<ParsedFn>`, so that accessor was reachable from outside the crate all
    /// along — measured, not assumed. The report that both entry points were
    /// blocked was wrong in that half.
    pub fn body_records(&self) -> &[(i64, i64)] {
        &self.body
    }
}

/// Whether a group of same-named heads compiles as a multiheaded guard dispatch.
///
/// **The decision is a property of the heads, not of the declaration keyword.** This
/// asked `cat == 2` (a `yield` declaration) until 2026-08-12, which was wrong in both
/// directions and silent in both:
///
/// - A multiheaded `fn` took the single-body path, which reads `group[0].body` and
///   discards every later head together with every `when` guard. The reference admits
///   and lowers a multiheaded `fn`, so the two compilers emitted different programs
///   with no diagnostic from either.
/// - A single-headed `yield` took the dispatch path and gained a parameter copy and a
///   `Trap(NoMatchingHead)` the reference never emits.
///
/// **Neither was reachable from the corpus**, which is why the whole-stage
/// byte-identity self-compiles never reported it: the ten stage sources contain
/// exactly one multiheaded function, `codegen.kel`'s nine-headed `emit_next`, and it
/// was declared `yield`. The keyword and the head count agreed on every input the
/// oracle had ever seen. That agreement is a fact about the corpus rather than about
/// the predicate.
///
/// A lone GUARDED head still needs the dispatch: its guard can fail, and the reference
/// emits the trap for that path. A lone guarded `yield` head is inadmissible — it is
/// not always-yielding, so structural verification rejects any `loop` that delegates
/// productivity to it — so this predicate never has to route one.
fn is_multihead_group(group: &[&ParsedFn]) -> bool {
    debug_assert!(!group.is_empty(), "a head group is never empty");
    group.len() > 1 || group.first().is_some_and(|h| !h.guard.is_empty())
}

const KINDS: usize = 1;

const ARGS: usize = 1 + 1024;

const LHS: usize = 1 + 1024 * 2;

const RHS: usize = 1 + 1024 * 3;

const CALL_ARGS: usize = 1 + 1024 * 4;

const FOR_PARTS: usize = 1 + 1024 * 4 + 256;

const MATCH_PARTS: usize = 1 + 1024 * 4 + 256 * 2;

const LIMIT_PARTS: usize = 1 + 1024 * 4 + 256 * 3;

const HEAD_PARTS: usize = 1 + 1024 * 4 + 256 * 4;

const PARAM_COUNT: usize = 1 + 1024 * 4 + 256 * 5;

const CATEGORY: usize = 1 + 1024 * 4 + 256 * 5 + 1;

// The shared-slot layout for `parse.kel` lives in `crate::selfhost_host`, which is
// gated on `compile + verify` like the harnesses that need it, rather than on the
// narrower `self-host` feature this module carries.
use crate::selfhost_host::{
    BR_LEX_ICOUNT, BR_LEX_ILEN, BR_LEX_ISTART, BR_P_AND_ID, BR_P_AT, BR_P_BASE, BR_P_BOOL_ID,
    BR_P_BYTE_ID, BR_P_CHUNK_COUNT, BR_P_CHUNKS, BR_P_FALSE_ID, BR_P_LEN, BR_P_LIMIT_ID,
    BR_P_OR_ID, BR_P_PACKED, BR_P_REQUIRE_ID, BR_P_TRUE_ID, BR_P_WORD_ID, PARSE_CHUNK_CAP,
    PARSE_LET_NAME_TAG, PARSE_TOKEN_CAP,
};

fn br_shared_word(vm: &Vm<'_, '_>, buf: &[u8], slot: usize) -> i64 {
    match vm.get_shared(buf, slot).expect("get_shared") {
        Value::Int(n) => n,
        other => panic!("expected Int at slot {slot}, got {other:?}"),
    }
}

/// Resume until the next yielded word, skipping the loop RESET. Used for the raw
/// pool metadata, where a yielded 0 is a real value, not a PENDING marker.
fn next_word(vm: &mut Vm<'_, '_>, shared: &mut [u8]) -> i64 {
    loop {
        match vm
            .resume_with_shared(shared, Value::Int(0))
            .expect("resume")
        {
            VmState::Yielded(Value::Int(w)) => return w,
            VmState::Reset => continue,
            other => panic!("unexpected {other:?}"),
        }
    }
}

/// Tokenize `src` with lexer.kel; return its `(tok, payload)` stream (no EOF) and
/// the id-to-spelling table recovered from the exposed intern table.
fn br_lex(src: &str) -> (Vec<(i64, i64)>, Vec<String>) {
    let bytes = src.as_bytes();
    let m = compile_src(&read_stage("kel/lexer.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify lexer.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(bytes.len() as i64))
        .unwrap();
    for (i, &b) in bytes.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b)).unwrap();
    }
    let mut toks = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(bytes.len() * 4 + 16) {
        if let VmState::Yielded(Value::Int(t)) = st {
            if t == 63 {
            } else if t == 62 {
                break;
            } else {
                toks.push((t.rem_euclid(256), t.div_euclid(256)));
            }
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    let icount = br_shared_word(&vm, &shared, BR_LEX_ICOUNT) as usize;
    let mut names = Vec::with_capacity(icount);
    for id in 0..icount {
        let start = br_shared_word(&vm, &shared, BR_LEX_ISTART + id) as usize;
        let len = br_shared_word(&vm, &shared, BR_LEX_ILEN + id) as usize;
        names.push(String::from_utf8(bytes[start..start + len].to_vec()).unwrap());
    }
    (toks, names)
}

// The flat-scalar and composite-variant tag decoders, shared by the composite/enum `decode_op` arms.
// Identical mapping to the differential-oracle decoder in `tests/selfhost_codegen.rs`.
fn scalar_kind_from_tag(t: i64) -> ScalarKind {
    match t {
        0 => ScalarKind::Unit,
        1 => ScalarKind::Bool,
        2 => ScalarKind::Byte,
        3 => ScalarKind::Int,
        4 => ScalarKind::Fixed,
        5 => ScalarKind::Float,
        6 => ScalarKind::Text,
        7 => ScalarKind::Opaque,
        other => panic!("bad scalar kind tag {other}"),
    }
}

fn composite_kind_from_tag(t: i64) -> CompositeKind {
    match t {
        0 => CompositeKind::Tuple,
        1 => CompositeKind::Array,
        2 => CompositeKind::Struct,
        3 => CompositeKind::Enum,
        other => panic!("bad composite variant tag {other}"),
    }
}

fn decode_op(w: i64) -> Op {
    // P11 Option E: the op-word radix is 8 bits (256); the tag is `w % 256`, the operand `w / 256`.
    let (tag, operand) = (w % 256, w / 256);
    match tag {
        1 => Op::Const(operand as u16),
        2 => Op::Return,
        3 => Op::GetLocal(operand as u16),
        4 => Op::CheckedMul(operand as u8),
        5 => Op::CheckedAdd,
        6 => Op::PopN(operand as u8),
        7 => Op::SetLocal(operand as u16),
        8 => Op::CheckedSub,
        9 => Op::Div,
        10 => Op::Mod,
        11 => Op::CmpEq,
        12 => Op::CmpNe,
        13 => Op::CmpLt,
        14 => Op::CmpGt,
        15 => Op::CmpLe,
        16 => Op::CmpGe,
        17 => Op::If(operand as u16),
        18 => Op::Else(operand as u16),
        19 => Op::EndIf,
        20 => Op::Not,
        21 => Op::Call((operand % 65536) as u16, (operand / 65536) as u8),
        22 => Op::Dup,
        23 => Op::CheckedNeg,
        24 => Op::BitAnd,
        25 => Op::BitOr,
        26 => Op::BitXor,
        27 => Op::GetData(operand as u32),
        28 => Op::SetData(operand as u32),
        // Base and length pack with a 2^24 radix so a data slot or length beyond
        // 65535 (a shared segment over 64 KB) does not spill base into length.
        29 => Op::GetDataIndexed((operand % 16777216) as u32, (operand / 16777216) as u32),
        30 => Op::SetDataIndexed((operand % 16777216) as u32, (operand / 16777216) as u32),
        31 => Op::Loop(operand as u16),
        32 => Op::BreakIf(operand as u16),
        33 => Op::EndLoop(operand as u16),
        34 => Op::PushImmediate(operand as u8),
        // Match control flow. The stage gives these their own op-word tags so
        // `emit_op` can backpatch a bare `If`/`EndIf` and multiple unconditional
        // `Break`s, but they decode to the same reference ops as the structured
        // forms (an `mif` is an `Op::If`, an `mloop` an `Op::Loop`, and so on).
        35 => Op::Break(operand as u16),
        36 => Op::Trap(operand as u16),
        37 => Op::If(operand as u16),
        38 => Op::EndIf,
        39 => Op::Loop(operand as u16),
        40 => Op::EndLoop(operand as u16),
        // The `for ... limit` counter header is a conditional break to the loop
        // exit; its own stage tag lets `emit_op` set its exit target while
        // preserving the `BreakIf` decode.
        41 => Op::BreakIf(operand as u16),
        // The `yield`/`loop` machinery.
        42 => Op::Yield,
        43 => Op::Stream,
        44 => Op::Reset,
        45 => Op::ByteToWord,
        // Op tags 46..=63 -- the composite/enum family and the scalar arithmetic/shift ops. These are
        // ported verbatim from the differential-oracle decoder in `tests/selfhost_codegen.rs::decode_op`
        // (the main workspace), which is the proven-correct reference. Kept in lockstep with that
        // decoder; the `all_wire_ops_decode` regression test below guards against drift, and the
        // `verify.sh` merge gate exercises this whole subproject so a future op cannot ride in undecoded.
        58 => Op::WordToByte,
        59 => Op::Add,
        60 => Op::Sub,
        61 => Op::Mul,
        62 => Op::Shl,
        63 => Op::Shr,
        // A flat struct construction: the operand packs count + byte_size*65536.
        46 => Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Struct,
            count: (operand % 65536) as u16,
            byte_size: (operand / 65536) as u16,
        }),
        // A flat array literal: same packing, Array composite kind.
        50 => Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Array,
            count: (operand % 65536) as u16,
            byte_size: (operand / 65536) as u16,
        }),
        // A flat enum construction: same packing, Enum composite kind.
        51 => Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Enum,
            count: (operand % 65536) as u16,
            byte_size: (operand / 65536) as u16,
        }),
        // A flat tuple construction: same packing, Tuple composite kind.
        52 => Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Tuple,
            count: (operand % 65536) as u16,
            byte_size: (operand / 65536) as u16,
        }),
        // A flat struct field read: the operand packs offset + kind_tag*65536.
        47 => Op::GetField(StructField::Flat {
            offset: (operand % 65536) as u16,
            kind: scalar_kind_from_tag(operand / 65536),
        }),
        // A flat-nested struct field read: operand packs offset + size*65536 + variant*2^32.
        48 => Op::GetField(StructField::FlatNested {
            offset: (operand % 65536) as u16,
            size: ((operand / 65536) % 65536) as u16,
            variant: composite_kind_from_tag(operand / 4294967296),
        }),
        // A flat array-element read: operand is the element ScalarKind tag.
        49 => Op::GetIndex(ArrayElem::Flat {
            kind: scalar_kind_from_tag(operand),
        }),
        // A flat-nested array-element read: operand packs size + variant*65536.
        56 => Op::GetIndex(ArrayElem::FlatNested {
            size: (operand % 65536) as u16,
            variant: composite_kind_from_tag(operand / 65536),
        }),
        // **TAG 53 HAS TWO FORMS AND THIS DRIVER DECODED ONLY ONE.**
        //
        // A tuple field read. The FLAT-NESTED form -- a nested composite tuple element
        // extracted and re-wrapped -- packs `offset + size*65536 + variant*2^32`; the FLAT
        // form packs `offset + kind_tag*65536`, which is under 2^20. They are disambiguated
        // by operand magnitude, exactly as `tests/selfhost_codegen.rs` has always done.
        //
        // Without the first arm a struct-typed tuple element fell into the second and its
        // packed word was read as a scalar-kind tag: for `size = 8, variant = Struct(2)` the
        // operand is 8,590,458,880, and `operand / 65536` is 131,080 -- the "bad scalar kind
        // tag 131080" that six `Ok`-recorded boundary cases faulted with. A LOUD fault rather
        // than a wrong op, but a fault on a construct the boundary reports as supported.
        //
        // The sibling arms for `GetField` (47/48) and `GetIndex` (49/56) use distinct op tags
        // for the two forms, so only the tuple read needs the magnitude guard.
        53 if operand >= 4294967296 => Op::GetTupleField(TupleField::FlatNested {
            offset: (operand % 65536) as u16,
            size: ((operand / 65536) % 65536) as u16,
            variant: composite_kind_from_tag(operand / 4294967296),
        }),
        // A flat tuple field read: operand packs offset + kind_tag*65536.
        53 => Op::GetTupleField(TupleField::Flat {
            offset: (operand % 65536) as u16,
            kind: scalar_kind_from_tag(operand / 65536),
        }),
        // An IsEnum test: operand packs ename_pool + vname_pool*1024 + disc_pool*1024*1024.
        54 => Op::IsEnum(
            (operand % 1024) as u16,
            ((operand / 1024) % 1024) as u16,
            (operand / 1048576) as u16,
        ),
        // A flat-nested enum-payload field read: operand packs offset + size*65536 + variant*2^32.
        57 => Op::GetEnumField(EnumField::FlatNested {
            offset: (operand % 65536) as u16,
            size: ((operand / 65536) % 65536) as u16,
            variant: composite_kind_from_tag(operand / 4294967296),
        }),
        // A flat enum-payload field read: operand packs offset + kind_tag*65536.
        55 => Op::GetEnumField(EnumField::Flat {
            offset: (operand % 65536) as u16,
            kind: scalar_kind_from_tag(operand / 65536),
        }),
        other => panic!("unknown op tag {other} (word {w})"),
    }
}

/// Drive the codegen; return its emitted ops, the constant pool it built, and the
/// local-frame size (`local_count`) it computed.
fn run_codegen(body: &Body, param_count: usize) -> (Vec<Op>, Vec<(i64, i64)>, i64) {
    let src = read_stage("kel/codegen.kel");
    let m = compile_src(&src);
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify codegen.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(body.root))
        .expect("root");
    for (i, n) in body.nodes.iter().enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(n.kind))
            .expect("kind");
        vm.set_shared(&mut shared, ARGS + i, Value::Int(n.arg))
            .expect("arg");
        vm.set_shared(&mut shared, LHS + i, Value::Int(n.lhs))
            .expect("lhs");
        vm.set_shared(&mut shared, RHS + i, Value::Int(n.rhs))
            .expect("rhs");
    }
    for (k, &node) in body.call_args.iter().enumerate() {
        vm.set_shared(&mut shared, CALL_ARGS + k, Value::Int(node))
            .expect("call_arg");
    }
    for (k, &part) in body.for_parts.iter().enumerate() {
        vm.set_shared(&mut shared, FOR_PARTS + k, Value::Int(part))
            .expect("for_part");
    }
    for (k, &part) in body.match_parts.iter().enumerate() {
        vm.set_shared(&mut shared, MATCH_PARTS + k, Value::Int(part))
            .expect("match_part");
    }
    for (k, &part) in body.limit_parts.iter().enumerate() {
        vm.set_shared(&mut shared, LIMIT_PARTS + k, Value::Int(part))
            .expect("limit_part");
    }
    for (k, &part) in body.head_parts.iter().enumerate() {
        vm.set_shared(&mut shared, HEAD_PARTS + k, Value::Int(part))
            .expect("head_part");
    }
    vm.set_shared(&mut shared, PARAM_COUNT, Value::Int(param_count as i64))
        .expect("param_count");
    vm.set_shared(&mut shared, CATEGORY, Value::Int(body.category))
        .expect("category");

    // Phase 1: ops until Return.
    let mut ops = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..65536 {
        match st {
            VmState::Yielded(Value::Int(w)) => {
                if w != 0 {
                    let op = decode_op(w);
                    // The op stream's terminator depends on the function category:
                    // an `fn` ends in `Return`, a `loop` in `Reset`, and a
                    // multiheaded dispatch in `Trap(NoMatchingHead=1)`. A
                    // multihead's per-head `Return`s are interior ops, so it must
                    // read past them to the final trap.
                    let done = match body.category {
                        2 => op == Op::Reset,
                        3 => op == Op::Trap(1),
                        _ => op == Op::Return,
                    };
                    ops.push(op);
                    if done {
                        break;
                    }
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected VM state {other:?}"),
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }

    // Phase 2: the pool the stage built. Size, then that many raw values, then that many raw
    // tags. **THE TAG IS PART OF THE VALUE AND MUST BE CARRIED OUT OF HERE.**
    //
    // `codegen.kel` interns three tags in three separate functions with tag-aware dedup:
    // 0 `Int` (`intern_int`), 1 `StaticStr` (`intern_str`, the value being the LEXER INTERN
    // ID rather than the bytes), and 2 `Bool` (`intern_bool`, reached from `push_struct_eq`).
    //
    // This loop used to read the tags into `let _tag` and drop them, on the stated grounds
    // that "the stage sources are all-Int". That was a true statement about the CORPUS and a
    // false one about the CONTRACT: the byte-identity oracle compiles the stage sources, none
    // of which contains a string literal or a struct equality, so neither tag was ever
    // observed. Every pool entry was then rebuilt as `ConstValue::Int`, turning a `StaticStr`
    // into the integer of its intern id. See `docs/decisions/POOL_TAG_RESIDENCY_BRIEF.md`.
    let count = next_word(&mut vm, &mut shared);
    let values: Vec<i64> = (0..count)
        .map(|_| next_word(&mut vm, &mut shared))
        .collect();
    let pool = values
        .into_iter()
        .map(|v| (v, next_word(&mut vm, &mut shared)))
        .collect();
    // Phase 3: the local-frame size the stage computed.
    let local_count = next_word(&mut vm, &mut shared);
    (ops, pool, local_count)
}

/// Derive the call-resolution chunk table (the interned name id of each chunk, in the
/// module's chunk order) from lexer.kel's token stream, with no reference borrow of the
/// user program. The reference orders chunks by function name and folds a multiheaded
/// function's several same-named heads into one chunk, so the table is the deduplicated,
/// lexicographically sorted set of the program's function names. Each `fn` (tok 0),
/// `yield` (tok 5), or `loop` (tok 6) keyword is immediately followed by the name
/// identifier (tok 1, whose payload is the interned id), so the function-name set is a
/// direct token scan. Returns each chunk name's interned id in the sorted order.
fn chunk_table_from_tokens(tokens: &[(i64, i64)], names: &[String]) -> Vec<i64> {
    // Track brace-nesting depth (LBrace = 2, RBrace = 3) so a `yield` keyword used as a
    // yield *statement* inside a body (depth > 0), or any keyword occurrence below the
    // top level, is not mistaken for a function head. A function head appears only at
    // depth 0, before its body's opening brace, immediately followed by the name Ident.
    let mut ids: Vec<i64> = Vec::new();
    let mut depth: i64 = 0;
    for w in tokens.windows(2) {
        let (kw, _) = w[0];
        let (tok, payload) = w[1];
        if depth == 0 && (kw == 0 || kw == 5 || kw == 6) && tok == 1 {
            ids.push(payload);
        }
        match kw {
            2 => depth += 1,
            3 => depth -= 1,
            _ => {}
        }
    }
    // The final token is not covered by `windows(2)` as a `w[0]`, but a function head is
    // never the last token (its body follows), so no head is missed by that omission.
    // Deduplicate by name (a multiheaded function is one chunk) and order by chunk name,
    // matching the reference's name-keyed `BTreeMap` chunk order.
    ids.sort_by(|&a, &b| names[a as usize].cmp(&names[b as usize]));
    ids.dedup_by(|&mut a, &mut b| names[a as usize] == names[b as usize]);
    ids
}

/// Pull the next token from a live `lexer.kel`, or `None` at end of input.
///
/// The lexer's protocol: 63 is PENDING (a step that consumed a byte without
/// completing a token), 62 is end of input, anything else is a token packed as
/// `kind + payload * 256`.
fn lex_next(vm: &mut Vm<'_, '_>, shared: &mut [u8]) -> Option<i64> {
    // Bounded rather than `loop`: a lexer that only ever reported PENDING would
    // otherwise hang the driver, and a total language should not need a host that
    // can. Four bytes of pending per token is far past any real run.
    for _ in 0..(4 * 393_216) {
        // A coroutine must be RESUMED, not re-called -- but the FIRST entry has to
        // be a call, and the machine exposes no predicate for which it wants.
        // `NotSuspended` is that predicate, and taking it here keeps the caller
        // from having to track whether it has started the lexer yet.
        let st = match vm.resume_with_shared(shared, Value::Int(0)) {
            Err(crate::vm::VmError::NotSuspended) => vm
                .call_with_shared(shared, &[Value::Int(0)])
                .expect("start lexer.kel"),
            other => other.expect("resume lexer.kel"),
        };
        match st {
            VmState::Yielded(Value::Int(62)) => return None,
            VmState::Yielded(Value::Int(63)) | VmState::Reset => {}
            VmState::Yielded(Value::Int(t)) => return Some(t),
            other => panic!("lexer.kel yielded {other:?}"),
        }
    }
    panic!("lexer.kel reported PENDING without end of input")
}

/// Build a live `lexer.kel` over `src`, returning the machine and its shared buffer.
///
/// Separated from driving so the same construction serves the collecting pass and
/// the fused one, and so a caller can hold TWO stages live at once.
/// The compiled lexer, so a caller can size its arena before opening one.
///
/// Split out because the persistent region must be resized to the MODULE's
/// requirement before `Vm::new`, and the requirement is not known until the module
/// exists. A version of `lex_open` that both compiled and opened could not do that
/// without guessing a capacity.
fn lex_module() -> Module {
    compile_src(&read_stage("kel/lexer.kel"))
}

fn lex_open<'a>(arena: &'a Arena, m: Module, src: &str) -> (Vm<'a, 'a>, Vec<u8>) {
    let vm = Vm::new(m, arena).expect("verify lexer.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let bytes = src.as_bytes();
    vm.set_shared(&mut shared, 0, Value::Int(bytes.len() as i64))
        .unwrap();
    for (i, &b) in bytes.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b)).unwrap();
    }
    (vm, shared)
}

/// The intern table, read out of a lexer that has reached end of input.
fn lex_names(vm: &Vm<'_, '_>, shared: &[u8], src: &str) -> Vec<String> {
    let bytes = src.as_bytes();
    let icount = br_shared_word(vm, shared, BR_LEX_ICOUNT) as usize;
    (0..icount)
        .map(|id| {
            let start = br_shared_word(vm, shared, BR_LEX_ISTART + id) as usize;
            let len = br_shared_word(vm, shared, BR_LEX_ILEN + id) as usize;
            String::from_utf8(bytes[start..start + len].to_vec()).unwrap()
        })
        .collect()
}

/// What the first pass over the source establishes, all of it bounded.
struct FirstPass {
    /// The intern table. Complete only at end of input, which is why it is a
    /// whole-input fact rather than something a token can carry.
    names: Vec<String>,
    /// The call-resolution chunk table, in the module's chunk order.
    chunks: Vec<i64>,
    /// How many tokens the source lexes to. `parse.kel` compares its cursor
    /// against this to find end of input, so a windowed feed must know it up
    /// front -- which is the one thing a single forward pass cannot supply.
    token_count: usize,
}

/// Stream the lexer once, keeping only what the second pass needs.
///
/// **This is the pre-pass, and it is bounded.** It holds an intern table, a list
/// of function-name ids, one previous token and a depth counter -- not the token
/// stream. `chunk_table_from_tokens` walks `windows(2)`, so the same scan runs
/// incrementally with a single token of lookbehind.
///
/// Running the lexer twice is how a single-pass compiler has always handled a
/// forward reference it cannot settle on first sight, and it is cheap: the lexer
/// is the fastest stage and the source is the smallest representation in the
/// pipeline.
fn first_pass(src: &str) -> FirstPass {
    let m = lex_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let (mut vm, mut shared) = lex_open(&arena, m, src);
    let mut ids: Vec<i64> = Vec::new();
    let mut depth: i64 = 0;
    let mut prev: Option<(i64, i64)> = None;
    let mut token_count = 0usize;
    while let Some(w) = lex_next(&mut vm, &mut shared) {
        let cur = (w.rem_euclid(256), w.div_euclid(256));
        token_count += 1;
        if let Some((kw, _)) = prev {
            if depth == 0 && (kw == 0 || kw == 5 || kw == 6) && cur.0 == 1 {
                ids.push(cur.1);
            }
            match kw {
                2 => depth += 1,
                3 => depth -= 1,
                _ => {}
            }
        }
        prev = Some(cur);
    }
    let names = lex_names(&vm, &shared, src);
    // Ordering needs the names, which exist only now. A multiheaded function is
    // one chunk, and the reference keys its chunk map by name, so the table is the
    // deduplicated lexicographically sorted set.
    ids.sort_by(|&a, &b| names[a as usize].cmp(&names[b as usize]));
    ids.dedup_by(|&mut a, &mut b| names[a as usize] == names[b as usize]);
    FirstPass {
        names,
        chunks: ids,
        token_count,
    }
}

/// Drive lexer.kel then parse.kel over `src`, collecting the token stream first.
///
/// The original path: every token materialised into a `Vec` and seeded into a
/// 40,960-word shared array. Kept because it is the reference the fused path is
/// checked against.
// The 4-tuple carries the parsed functions, name table, and the raw data and enum
// record streams; factoring each into a `type` alias would only scatter it.
/// **NO PRODUCTION PATH CALLS THIS ANY MORE. It is the fusion oracle.**
///
/// Every compile entry point — [`self_host_compile`], [`self_host_compile_full`],
/// [`self_host_compile_scratch`] and [`binding_rows_from_pipeline`] — moved to
/// [`parse_functions_fused`] on 2026-08-19, under the operator's direction to
/// retire the token residency.
///
/// # Why it is retained rather than deleted
///
/// This feed is what proves the fused one correct.
/// `the_fused_parse_agrees_with_the_collecting_one` and
/// `the_fused_compile_agrees_with_the_collecting_one` compare the two, and a
/// differential oracle with one side removed is not an oracle. Deleting it would
/// leave fusion checked only against the Rust reference, which is a weaker claim
/// about the FEED specifically: the reference agrees with a whole-program compile,
/// not with a particular token-delivery order.
///
/// # What it costs, and what happens next
///
/// It is the reason `toks.packed` is still 40,960 words: this feed seeds the whole
/// token stream, so the array cannot shrink while any caller does. **The next
/// increment shrinks the array to a buffer sized for the equivalence corpus**, at
/// which point this entry point keeps working for every source those two tests
/// use and stops working for a large one — which is acceptable precisely because
/// nothing but those tests calls it.
#[allow(clippy::type_complexity)]
pub fn parse_functions(
    src: &str,
) -> (Vec<ParsedFn>, Vec<String>, Vec<(i64, i64)>, Vec<(i64, i64)>) {
    let mut fns = Vec::new();
    let (names, data_records, enum_records) =
        parse_functions_impl(src, false, &mut |_, f| fns.push(f), &mut |_, _| {});
    (fns, names, data_records, enum_records)
}

/// The same, with `lexer.kel` driven INTO `parse.kel` and no token stream
/// materialised.
///
/// # The two passes, and why there are two
///
/// `parse.kel` needs the chunk table before its first token, because a resolved
/// call index must match the module's chunk order -- and that table is a property
/// of the WHOLE token stream. No single forward pass supplies it, so the lexer
/// runs twice: `first_pass` establishes the bounded facts, then the second run
/// is fused into the parser. That is the classical answer to a forward reference,
/// and it is what a pipeline cut at this boundary would do with a sidecar file.
///
/// The token COUNT comes from the first pass for the same reason: `parse.kel`
/// finds end of input by comparing its cursor against `toks.len`, which a windowed
/// feed cannot leave as "however many arrive".
///
/// # What it does not change
///
/// The output is what [`parse_functions`] returns, and
/// `the_fused_parse_agrees_with_the_collecting_one` asserts they are equal on real
/// sources. Fusion changes WHEN a token is produced, not what it is.
#[allow(clippy::type_complexity)]
pub fn parse_functions_fused(
    src: &str,
) -> (Vec<ParsedFn>, Vec<String>, Vec<(i64, i64)>, Vec<(i64, i64)>) {
    let mut fns = Vec::new();
    let (names, data_records, enum_records) =
        parse_functions_impl(src, true, &mut |_, f| fns.push(f), &mut |_, _| {});
    (fns, names, data_records, enum_records)
}

/// How many tokens `lexer.kel` produces for `src` — **the count `PARSE_TOKEN_CAP` is
/// measured against**.
///
/// Public because it is NOT the reference tokenizer's count and the difference matters. The
/// two disagree by one on every source measured, the reference emitting a terminator the
/// stage's stream does not carry. A caller sizing an input against the cap with
/// `keleusma::lexer::tokenize(...).len()` is measuring the wrong quantity — the same class
/// of error as sizing a guard against the wrong array.
pub fn lex_token_count(src: &str) -> usize {
    first_pass(src).token_count
}

/// `Node::LetIn`, the record whose payload carries the frame slot.
const NODE_LET_IN: i64 = 5;

/// `Node::Literal`. Always an INTEGER: `codegen.kel`'s `push_literal` interns
/// through `intern_int`, so no other scalar reaches this kind.
const NODE_LITERAL: i64 = 1;

/// `Node::Unit`, whose payload is the `PushImmediate` operand — `0` Unit, `1`
/// true, `2` false. Boolean literals ride this kind rather than a new one.
const NODE_UNIT: i64 = 20;

/// The declaration a record belongs to, or a diagnostic naming what arrived instead.
///
/// **SIX BARE `unwrap()`s USED TO SIT HERE, AND THEY ALL FIRE FOR ONE REASON**: a record
/// arrived while no declaration was open. Measured cause: a top-level `struct` declaration.
/// `parse.kel` has no struct handling at all — its declaration record codes are 1..3
/// (`fn`/`yield`/`loop`), 9 (`data`), 10 (`use`) and 12 (`enum`), with no struct code — so
/// its tokens are parsed as something else and the records land here with nothing open.
///
/// The old failure was `called Option::unwrap() on a None value`, which names neither the
/// record nor the declaration form that produced it. **A user writing an ordinary
/// declaration got a Rust panic with no indication the form was unsupported.**
///
/// This does not decide whether the form should be supported; it reports what happened.
fn open_decl(cur: &mut Option<ParsedFn>, code: i64, val: i64) -> &mut ParsedFn {
    cur.as_mut().unwrap_or_else(|| {
        panic!(
            "parse.kel emitted record ({code}, {val}) with no declaration open. The usual \
             cause is a top-level declaration form the stage does not recognise: it handles \
             `fn`, `yield`, `loop`, `data`, `use` and `enum`, and a `struct` declaration is \
             NOT among them."
        )
    })
}

/// How many tokens the fused feed keeps resident.
///
/// Three would suffice: the parser reads at its cursor and pushes back by at most
/// one, so `[cursor - 1, cursor + 1]` covers every access, measured by
/// `the_parser_never_jumps_more_than_one_token`. Eight is used for margin, and the
/// margin is the point -- a window sized exactly to the measured bound would turn
/// any future widening of the parser's reach into a silently wrong parse rather
/// than an obvious one.
const FUSED_WINDOW: usize = 8;

/// The token cursor after each resume of `parse.kel`, for measuring how far the
/// parser actually reaches into its input.
///
/// # Why this exists
///
/// `toks.packed` is 40,960 words and the driver seeds the whole token stream into
/// it. That residency is the LAST one in the pipeline, and it is the driver's
/// rather than the parser's: every one of `parse.kel`'s cursor moves is plus or
/// minus one, so it is a one-token lookahead scanner with single-token pushback.
///
/// Reading the cursor back is what turns that from a claim about the source into
/// a measurement. A host feeding a sliding window needs to know the parser cannot
/// jump; this returns the evidence rather than the assurance.
///
/// The stage writes `toks.at` on every token read, and a step consumes at most one
/// token, so consecutive entries differ by at most one in either direction.
#[must_use]
pub fn parse_cursor_trace(src: &str) -> Vec<i64> {
    let (tokens, names) = br_lex(src);
    let id_of = |s: &str| {
        names
            .iter()
            .position(|n| n == s)
            .map(|i| i as i64)
            .unwrap_or(-1)
    };
    let chunks: Vec<i64> = chunk_table_from_tokens(&tokens, &names);
    let module = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| compile_src(&read_stage("kel/parse.kel")))
        .expect("spawn")
        .join()
        .expect("join");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parse.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, BR_P_LEN, Value::Int(tokens.len() as i64))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_LIMIT_ID, Value::Int(id_of("limit")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_REQUIRE_ID, Value::Int(id_of("require")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_WORD_ID, Value::Int(id_of("Word")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_BYTE_ID, Value::Int(id_of("Byte")))
        .unwrap();
    // **THE EAGER BOOLEAN OPERATORS, WHICH THIS COMMENT USED TO CLAIM WERE ALREADY HERE.**
    // The sentence below said the boolean literals are "seeded like the eager `and`/`or`
    // ids" -- true of `tests/selfhost_codegen.rs`, which seeds all four, and false of this
    // file, which seeded neither. The comment was copied along with the literals and
    // described the sibling's state.
    //
    // `parse.kel` guards its `and`/`or` recognition on `and_id > 0` so an unseeded host
    // keeps the old behaviour. The old behaviour is that the operator and its RIGHT OPERAND
    // are dropped: `a and b` compiled to `[GetLocal(0), Return]`, which is `a`. A silent
    // miscompile, and `true and false` returned `true`.
    vm.set_shared(&mut shared, BR_P_AND_ID, Value::Int(id_of("and")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_OR_ID, Value::Int(id_of("or")))
        .unwrap();
    // THE BOOLEAN LITERALS, seeded like the eager `and`/`or` ids above and for the same
    // reason: the Tok space is full, so `true` and `false` arrive as identifiers.
    // Without these the stage resolved them as variable references and emitted
    // `GetLocal` where the reference emits `PushImmediate`.
    vm.set_shared(&mut shared, BR_P_TRUE_ID, Value::Int(id_of("true")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_FALSE_ID, Value::Int(id_of("false")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_BOOL_ID, Value::Int(id_of("Bool")))
        .unwrap();
    vm.set_shared(
        &mut shared,
        BR_P_CHUNK_COUNT,
        Value::Int(chunks.len() as i64),
    )
    .unwrap();
    // THE CHUNK TABLE HAS A CAP AND OVERFLOWING IT REPORTS THE WRONG THING.
    //
    // `toks.chunks` holds `PARSE_CHUNK_CAP` entries, and eight load-bearing fields sit
    // immediately after it in the same shared block: the keyword and type ids, the
    // eager-operator ids, and the token window's own `base` and `at`. One entry past
    // the end lands on `require_id`.
    //
    // Measured, because the failure mode was not what it looked like. Overflowing by
    // one does NOT silently corrupt -- it panics -- but it panics with
    // `LoopLimitExceeded` from inside `parse.kel`, naming neither the chunk table nor
    // its cap. A caller reading that would look at loop bounds, not at the function
    // count of their program.
    //
    // THIS COMMENT WAS STALE IN FOUR WAYS AND THE DIAGNOSTIC WITH IT. It said the array
    // was 256, that a 257th entry overflowed, that `wire.kel` hit the cap at 475 chunks,
    // and that raising the array was "the real fix and NOT done here" -- after the array
    // had been raised to 1,024, `wire.kel` measured at 486 chunks, and the raise had in
    // fact been done. **Every number was left behind by the change that moved it**, and
    // the message told a caller with 1,025 functions about a 257th entry. The counts now
    // come from `PARSE_CHUNK_CAP` so they cannot drift again.
    assert!(
        chunks.len() <= PARSE_CHUNK_CAP,
        "this program has {} functions and `toks.chunks` in parse.kel holds {}; one entry \
         past the end overwrites `require_id` and the seven fields after it, including the \
         token window's `base`. Overflowing surfaces as `LoopLimitExceeded` from inside the \
         parser, which names neither this table nor its cap.",
        chunks.len(),
        PARSE_CHUNK_CAP
    );
    for (i, &c) in chunks.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_CHUNKS + i, Value::Int(c))
            .unwrap();
    }
    // THE WINDOW BASE, SET EXPLICITLY RATHER THAN LEFT TO ZERO-INITIALISATION.
    // The whole stream is seeded here, so `packed[0]` really is token zero; but
    // this file's own rule is that an emitter relying on its buffer starting
    // zeroed breaks the day it writes into a reused one, and a driver relying on
    // the same is no different. Stating it also makes the contract visible to a
    // later windowed driver, which sets a base that is not zero.
    vm.set_shared(&mut shared, BR_P_BASE, Value::Int(0))
        .unwrap();
    for (i, &(k, v)) in tokens.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_PACKED + i, Value::Int(k + v * 256))
            .unwrap();
    }

    let mut trace = Vec::new();
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(tokens.len() * 16 + 256) {
        trace.push(br_shared_word(&vm, &shared, BR_P_AT));
        match state {
            VmState::Finished(_) => break,
            _ => {
                state = match vm.resume_with_shared(&mut shared, Value::Int(0)) {
                    Ok(st) => st,
                    Err(_) => break,
                }
            }
        }
    }
    trace
}

/// Drive lexer.kel then parse.kel over `src` and return every function it yields, each
/// with its guard and body records, plus the interned-name table. Multiheaded functions
/// appear as several same-named entries in declaration order.
// The 4-tuple return carries the parsed functions, name table, and the raw data and
// enum record streams; factoring each into a `type` alias would only scatter it, so
// allow the complexity lint here as the root test file does file-wide.
#[allow(clippy::type_complexity)]
/// What the record walk returns beside the functions: the interned name table,
/// the data-block records and the enum records.
///
/// Named because clippy asks at this depth and the ask is right -- three `Vec`s
/// of pairs in a signature tell a reader less than the name does.
type ParseSideTables = (Vec<String>, Vec<(i64, i64)>, Vec<(i64, i64)>);

/// Every `(code, value)` record `parse.kel` emits for `src`, in order, beside the
/// interned name table.
///
/// # Why this exists as public surface
///
/// **A defect in this driver was diagnosed three times without it and stopped
/// short of a cause each time.** The declaration-mis-naming bug pinned by
/// `tests/selfhost_chunk_names.rs` was narrowed to a four-line reproduction and a
/// confirmed behavioural rule, and the code site stayed unknown, because the
/// record stream the driver consumes was not observable from outside it.
///
/// `thread_local!` is unavailable here (`no_std`), so a hook cannot be smuggled
/// in from a test. The sink is threaded through `parse_functions_impl` instead,
/// and this is its one public reader.
///
/// # What it is not
///
/// **Not part of the compile path**, and nothing in it should be. Every other
/// caller passes a sink that discards, so the trace costs a branch on a closure
/// that does nothing. It is a diagnostic instrument, and the reason it is public
/// rather than `#[doc(hidden)]` is that a hidden instrument is one the next
/// person does not know exists — which is how this defect survived three
/// diagnoses.
///
/// # Reading the codes
///
/// A declaration header is 1..=3 (`fn`/`yield`/`loop`) and carries the name id;
/// 4 is a parameter name, 6 a parameter type, 7 the return type, 5 closes a
/// declaration, 9 opens a data block, 16 opens a body, 15 ends one. Resolve a
/// name id through the returned table.
#[must_use]
pub fn parse_record_trace(src: &str) -> (Vec<String>, Vec<(i64, i64)>) {
    let mut records = Vec::new();
    let (names, ..) = parse_functions_impl(src, false, &mut |_, _| {}, &mut |code, val| {
        records.push((code, val))
    });
    (names, records)
}

/// [`parse_functions_impl`] for the two callers that want the name table in their
/// callback and no record trace.
fn parse_functions_impl_named(
    src: &str,
    on_function: &mut dyn FnMut(&[String], ParsedFn),
) -> ParseSideTables {
    parse_functions_impl(src, false, on_function, &mut |_, _| {})
}

fn parse_functions_impl(
    src: &str,
    fused: bool,
    on_function: &mut dyn FnMut(&[String], ParsedFn),
    on_record: &mut dyn FnMut(i64, i64),
) -> ParseSideTables {
    // ONE IMPLEMENTATION, TWO FEEDS. The record handling below is long and
    // stateful, and a second copy of it in a fused driver is precisely the drift
    // `selfhost_host` already records paying for once. Only the token FEED
    // differs: collected and seeded whole, or windowed and slid.
    let first = first_pass(src);
    let names = first.names.clone();
    let token_count = first.token_count;
    // The collecting feed still needs the tokens themselves.
    let tokens: Vec<(i64, i64)> = if fused { Vec::new() } else { br_lex(src).0 };
    let id_of = |s: &str| {
        names
            .iter()
            .position(|n| n == s)
            .map(|i| i as i64)
            .unwrap_or(-1)
    };
    // The chunk table must be in the module's actual chunk order so a resolved call index
    // matches the assembled module. The Rust compiler orders chunks by name (a `BTreeMap`
    // keyed by function name), not by declaration order, and groups same-named heads into
    // one chunk. The same order is the deduplicated, lexicographically sorted set of the
    // program's function names, which is derived from the token stream itself (each
    // `fn`/`yield`/`loop` keyword is immediately followed by the name identifier), so the
    // resolution table needs no reference borrow of the user program.
    let chunks: Vec<i64> = first.chunks.clone();
    let module = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| compile_src(&read_stage("kel/parse.kel")))
        .expect("spawn")
        .join()
        .expect("join");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parse.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    // THE TOKEN ARRAY HAS A CAP AND OVERFLOWING IT REPORTS ONE OF TWO WRONG THINGS.
    //
    // `toks.packed` holds `PARSE_TOKEN_CAP` tokens. Measured on a program that is one long
    // left-associative sum, so nothing else binds first:
    //
    //   41,015 tokens -> `IndexOutOfBounds(40960, 40960)` from inside the stage
    //   42,015 tokens -> a shared-slot range error from the seeding loop BELOW, which walks
    //                    off the end of the whole block
    //
    // Which one a caller sees depends on how far over they are, and neither names the token
    // array or a limit the caller controls. Refused here, before any seeding, with both
    // numbers. `parse.kel` itself is 32,907 tokens, 80% of this.
    //
    // **THE CAP BINDS THE COLLECTING FEED ONLY, AS OF 2026-08-19.** It was
    // unconditional, which meant the FUSED feed carried a bound it does not need:
    // fusion writes a window of `FUSED_WINDOW` slots and slides it, so the size of
    // `packed` says nothing about how long an input it can accept. Every
    // production entry point is fused, so this refusal no longer reaches any
    // compile a user can start. It still guards the collecting feed, which really
    // does seed the whole stream and really does overrun the array.
    //
    // Pinned by `the_token_cap_binds_only_the_collecting_feed`, which compiles a
    // source past the cap through the fused feed and refuses it through the other.
    assert!(
        fused || token_count <= PARSE_TOKEN_CAP,
        "this program lexes to {token_count} tokens and `toks.packed` in parse.kel holds \
         {PARSE_TOKEN_CAP}. Split the source, or use the fused feed, which is windowed."
    );
    vm.set_shared(&mut shared, BR_P_LEN, Value::Int(token_count as i64))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_LIMIT_ID, Value::Int(id_of("limit")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_REQUIRE_ID, Value::Int(id_of("require")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_WORD_ID, Value::Int(id_of("Word")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_BYTE_ID, Value::Int(id_of("Byte")))
        .unwrap();
    // **THE EAGER BOOLEAN OPERATORS, WHICH THIS COMMENT USED TO CLAIM WERE ALREADY HERE.**
    // The sentence below said the boolean literals are "seeded like the eager `and`/`or`
    // ids" -- true of `tests/selfhost_codegen.rs`, which seeds all four, and false of this
    // file, which seeded neither. The comment was copied along with the literals and
    // described the sibling's state.
    //
    // `parse.kel` guards its `and`/`or` recognition on `and_id > 0` so an unseeded host
    // keeps the old behaviour. The old behaviour is that the operator and its RIGHT OPERAND
    // are dropped: `a and b` compiled to `[GetLocal(0), Return]`, which is `a`. A silent
    // miscompile, and `true and false` returned `true`.
    vm.set_shared(&mut shared, BR_P_AND_ID, Value::Int(id_of("and")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_OR_ID, Value::Int(id_of("or")))
        .unwrap();
    // THE BOOLEAN LITERALS, seeded like the eager `and`/`or` ids above and for the same
    // reason: the Tok space is full, so `true` and `false` arrive as identifiers.
    // Without these the stage resolved them as variable references and emitted
    // `GetLocal` where the reference emits `PushImmediate`.
    vm.set_shared(&mut shared, BR_P_TRUE_ID, Value::Int(id_of("true")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_FALSE_ID, Value::Int(id_of("false")))
        .unwrap();
    vm.set_shared(&mut shared, BR_P_BOOL_ID, Value::Int(id_of("Bool")))
        .unwrap();
    vm.set_shared(
        &mut shared,
        BR_P_CHUNK_COUNT,
        Value::Int(chunks.len() as i64),
    )
    .unwrap();
    // THE CHUNK TABLE HAS A CAP AND OVERFLOWING IT REPORTS THE WRONG THING.
    //
    // `toks.chunks` holds `PARSE_CHUNK_CAP` entries, and eight load-bearing fields sit
    // immediately after it in the same shared block: the keyword and type ids, the
    // eager-operator ids, and the token window's own `base` and `at`. One entry past
    // the end lands on `require_id`.
    //
    // Measured, because the failure mode was not what it looked like. Overflowing by
    // one does NOT silently corrupt -- it panics -- but it panics with
    // `LoopLimitExceeded` from inside `parse.kel`, naming neither the chunk table nor
    // its cap. A caller reading that would look at loop bounds, not at the function
    // count of their program.
    //
    // THIS COMMENT WAS STALE IN FOUR WAYS AND THE DIAGNOSTIC WITH IT. It said the array
    // was 256, that a 257th entry overflowed, that `wire.kel` hit the cap at 475 chunks,
    // and that raising the array was "the real fix and NOT done here" -- after the array
    // had been raised to 1,024, `wire.kel` measured at 486 chunks, and the raise had in
    // fact been done. **Every number was left behind by the change that moved it**, and
    // the message told a caller with 1,025 functions about a 257th entry. The counts now
    // come from `PARSE_CHUNK_CAP` so they cannot drift again.
    assert!(
        chunks.len() <= PARSE_CHUNK_CAP,
        "this program has {} functions and `toks.chunks` in parse.kel holds {}; one entry \
         past the end overwrites `require_id` and the seven fields after it, including the \
         token window's `base`. Overflowing surfaces as `LoopLimitExceeded` from inside the \
         parser, which names neither this table nor its cap.",
        chunks.len(),
        PARSE_CHUNK_CAP
    );
    for (i, &c) in chunks.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_CHUNKS + i, Value::Int(c))
            .unwrap();
    }
    // THE WINDOW BASE, SET EXPLICITLY RATHER THAN LEFT TO ZERO-INITIALISATION.
    // The whole stream is seeded here, so `packed[0]` really is token zero; but
    // this file's own rule is that an emitter relying on its buffer starting
    // zeroed breaks the day it writes into a reused one, and a driver relying on
    // the same is no different. Stating it also makes the contract visible to a
    // later windowed driver, which sets a base that is not zero.
    vm.set_shared(&mut shared, BR_P_BASE, Value::Int(0))
        .unwrap();
    for (i, &(k, v)) in tokens.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_PACKED + i, Value::Int(k + v * 256))
            .unwrap();
    }
    // THE FUSED FEED. A live lexer and a window of `FUSED_WINDOW` tokens, instead
    // of the whole stream seeded above. Built here rather than in a separate
    // driver so the record handling below has exactly one implementation.
    let lex_m = lex_module();
    let lex_need = required_persistent_capacity_for(&lex_m);
    let mut lex_arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + lex_need);
    lex_arena.resize_persistent(lex_need).expect("resize");
    let mut lex: Option<(Vm<'_, '_>, Vec<u8>)> = if fused {
        Some(lex_open(&lex_arena, lex_m, src))
    } else {
        None
    };
    let mut win: alloc::collections::VecDeque<i64> = alloc::collections::VecDeque::new();
    let mut base: i64 = 0;
    let mut eof = false;

    // NO VECTOR OF FUNCTIONS HERE. A completed function goes straight to the sink, so
    // the collecting API owns the accumulation and a fusing caller does not have to.
    let mut cur: Option<ParsedFn> = None;
    // Every data block's header records (DSTART, then PARAM/PTYPE/ASIZE per field, then
    // END), concatenated in declaration order, for the driver's own data-layout assembly.
    let mut data_records: Vec<(i64, i64)> = Vec::new();
    // Every enum's header records (ENUMSTART, then EVARIANT/EDISC per variant, then END),
    // for the driver's own enum-layout assembly.
    let mut enum_records: Vec<(i64, i64)> = Vec::new();
    // **A STRUCT, TRAIT OR IMPL DECLARATION IS SKIPPED TO ITS `END`, NOT LEFT TO FALL
    // THROUGH.** `parse.kel` emits STRUCTSTART 18, TRAITSTART 19 and IMPLSTART 20 followed
    // by the declaration's own PARAM/PTYPE records. Without this state those records reach
    // the `match` below with no function open and `open_decl` panics by name — which is what
    // the shipping driver did for every program containing a `struct`, while
    // `tests/selfhost_codegen.rs`'s copy of this loop carried the skip and compiled them.
    // See `docs/decisions/POOL_TAG_RESIDENCY_BRIEF.md`.
    //
    // The declaration contributes no chunk and no scaffold record here: a struct's layout
    // reaches codegen through the BODY record stream (the StructEqField/GetField families),
    // not through its declaration.
    let mut in_skip_decl = false;
    let (mut in_body, mut in_guard, mut in_data, mut in_enum, mut in_use) =
        (false, false, false, false, false);
    // PRIME THE WINDOW BEFORE THE INITIAL CALL, not just before resumes.
    //
    // `drive_parse_records_with` runs the hook before every RESUME, but the first
    // step of the parser happens inside this CALL. Left unprimed, that read lands
    // on a zeroed `packed` and the parser is fed token zero -- which surfaces far
    // downstream as `IndexOutOfBounds(-1, 64)` on the OPERATOR STACK, not on the
    // token array, because the wrong token drives the shunting yard into draining
    // an empty stack. The index that faults names neither the array at fault nor
    // the cause.
    if fused {
        if let Some((lex_vm, lex_shared)) = lex.as_mut() {
            while !eof && (win.len() as i64) < FUSED_WINDOW as i64 {
                match lex_next(lex_vm, lex_shared) {
                    Some(w) => win.push_back(w),
                    None => eof = true,
                }
            }
        }
        vm.set_shared(&mut shared, BR_P_BASE, Value::Int(base))
            .unwrap();
        for (i, &w) in win.iter().enumerate() {
            vm.set_shared(&mut shared, BR_P_PACKED + i, Value::Int(w))
                .unwrap();
        }
    }

    let state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    let budget = token_count * 16 + 256;
    // SLIDE BEFORE THE RESUME, NEVER AFTER. The parser reads at its cursor the
    // moment it runs, so a window corrected afterwards is corrected too late.
    // One token behind the cursor is kept resident, because the parser pushes back
    // by one and that read must still land inside the window.
    let before_resume = |vm: &mut Vm<'_, '_>, shared: &mut [u8]| {
        let Some((lex_vm, lex_shared)) = lex.as_mut() else {
            return;
        };
        let at = match vm.get_shared(shared, BR_P_AT).expect("at") {
            Value::Int(n) => n,
            other => panic!("expected Int at BR_P_AT, got {other:?}"),
        };
        while !eof && (base + win.len() as i64) < at + FUSED_WINDOW as i64 {
            match lex_next(lex_vm, lex_shared) {
                Some(w) => win.push_back(w),
                None => eof = true,
            }
        }
        // ONE TOKEN OF LOOKBEHIND, AND IT IS PROVEN RATHER THAN CHOSEN.
        //
        // `toks.at` is written BEFORE the cursor advances, so it names the index
        // just read: after a read at C the cursor is C+1. With k pushbacks the
        // next read is at C+1-k, so the trace step is 1-k. Every step is within
        // plus or minus one, asserted over five sources including a whole real
        // stage by `the_parser_never_jumps_more_than_one_token`, so k is at most
        // two and the lowest index ever read is `at - 1`.
        //
        // AN EARLIER REVISION USED HALF THE WINDOW AND JUSTIFIED IT WITH A CLAIM
        // THAT IS FALSE -- that the cursor could sit "several tokens" behind `at`.
        // It cannot. That widening was a misdiagnosis of `IndexOutOfBounds(-1, 64)`,
        // whose real cause was an unprimed window at the initial call; the
        // widening did not fix it and was kept anyway.
        //
        // The tight bound is also the more diagnostic one. `base` is a true
        // absolute index, so a read below it lands negative and faults LOUDLY --
        // slack would only delay the report of an assumption breaking, never
        // prevent a wrong parse.
        let lookbehind = 1i64;
        while base < at - lookbehind && win.len() > 1 {
            win.pop_front();
            base += 1;
        }
        vm.set_shared(shared, BR_P_BASE, Value::Int(base)).unwrap();
        for (i, &w) in win.iter().enumerate() {
            vm.set_shared(shared, BR_P_PACKED + i, Value::Int(w))
                .unwrap();
        }
    };

    crate::selfhost_host::drive_parse_records_with(
        &mut vm,
        &mut shared,
        state,
        budget,
        |code, val| {
            on_record(code, val);
            if in_body {
                match code {
                    0 => {}
                    15 => in_body = false,
                    // THE BINDING-NAME RECORD IS DIVERTED, NOT APPENDED.
                    //
                    // `parse.kel` emits it immediately before the `LetIn` it belongs
                    // to, so the slot is the next record's payload. Pairing them here
                    // is what lets a type-check extraction join a forest of SLOTS to a
                    // binding table of NAMES, which nothing in the stream could do
                    // before: slots are reused across scopes and names are not.
                    c if c == PARSE_LET_NAME_TAG => {
                        let f = open_decl(&mut cur, code, val);
                        f.let_names.push((-1, val));
                    }
                    _ => {
                        let f = open_decl(&mut cur, code, val);
                        // The slot arrives with the `LetIn` that follows the name.
                        if code == NODE_LET_IN
                            && let Some(last) = f.let_names.last_mut()
                            && last.0 == -1
                        {
                            last.0 = val;
                        }
                        f.body.push((code, val));
                    }
                }
            } else if in_guard {
                match code {
                    0 => {}
                    15 => in_guard = false,
                    _ => open_decl(&mut cur, code, val).guard.push((code, val)),
                }
            } else if in_data {
                if code == 5 {
                    data_records.push((5, 0));
                    in_data = false;
                } else if code != 0 {
                    data_records.push((code, val));
                }
            } else if in_enum {
                if code == 5 {
                    enum_records.push((5, 0));
                    in_enum = false;
                } else if code != 0 {
                    enum_records.push((code, val));
                }
            } else if in_use {
                in_use = code != 5;
            } else if in_skip_decl {
                in_skip_decl = code != 5;
            } else {
                match code {
                    1..=3 => {
                        cur = Some(ParsedFn {
                            cat: code,
                            name: val,
                            params: 0,
                            param_types: Vec::new(),
                            return_type: 0,
                            guard: Vec::new(),
                            body: Vec::new(),
                            param_names: Vec::new(),
                            let_names: Vec::new(),
                        })
                    }
                    4 => {
                        let f = open_decl(&mut cur, code, val);
                        f.params += 1;
                        f.param_names.push(val);
                    }
                    6 => open_decl(&mut cur, code, val).param_types.push(val),
                    7 => open_decl(&mut cur, code, val).return_type = val,
                    9 => {
                        in_data = true;
                        data_records.push((9, val));
                    }
                    10 => in_use = true,
                    12 => {
                        in_enum = true;
                        enum_records.push((12, val));
                    }
                    16 => in_body = true,
                    17 => in_guard = true,
                    18..=20 => in_skip_decl = true, // struct/trait/impl declaration
                    5 => {
                        open_decl(&mut cur, code, val);
                        on_function(&names, cur.take().unwrap())
                    }
                    15 => return ControlFlow::Break(()),
                    _ => {}
                }
            }
            ControlFlow::Continue(())
        },
        before_resume,
    );
    (names, data_records, enum_records)
}

/// Turn `codegen.kel`'s tagged constant pool into the reference compiler's `ConstValue`s.
///
/// **ONE DEFINITION, USED BY EVERY ENTRY POINT.** Three call sites each rebuilt the pool
/// inline as `ConstValue::Int`, which is how the tag came to be dropped in three places at
/// once; a fourth entry point added later would have copied the fourth. The nine-copies
/// shared-layout defect this crate already paid for is the same shape.
///
/// # The tag protocol, which is the stage's and not this function's
///
/// | tag | `ConstValue` | what the value word holds |
/// |---|---|---|
/// | 0 | `Int` | the integer itself |
/// | 1 | `StaticStr` | the **lexer intern id**, resolved here through `names` |
/// | 2 | `Bool` | 0 or 1 |
///
/// A `StaticStr` is the only entry needing `names`, and it is why this cannot be a plain
/// `From` impl on the pair.
///
/// # Failure modes
///
/// An intern id outside `names` and an unrecognised tag both panic by name rather than
/// producing a silently wrong constant. Both are stage/host protocol violations: neither is
/// reachable from a user program, and a wrong constant would be a silent miscompile, which is
/// the class this function exists to close.
fn pool_to_constants(pool: &[(i64, i64)], names: &[String]) -> Vec<ConstValue> {
    pool.iter()
        .map(|&(v, tag)| match tag {
            0 => ConstValue::Int(v),
            1 => {
                let raw = names.get(v as usize).unwrap_or_else(|| {
                    panic!(
                        "codegen.kel interned StaticStr id {v}, which is outside the \
                         {}-entry lexer name table",
                        names.len()
                    )
                });
                ConstValue::StaticStr(unescape_string(raw))
            }
            2 => ConstValue::Bool(v != 0),
            other => panic!(
                "codegen.kel emitted constant-pool tag {other}, which this host does not \
                 know. A new tag needs a `ConstValue` mapping here, not a default"
            ),
        })
        .collect()
}

/// Resolve the escape sequences the reference compiler bakes into a `StaticStr`.
///
/// The lexer's name table holds a string literal's content **as written**, so `\n` is a
/// backslash followed by an `n`. The reference bakes the escaped byte, so comparing the two
/// requires this. Handling the four the reference handles — newline, tab, quote, backslash —
/// and passing anything else through unchanged, which matches the reference's own behaviour
/// on an unknown escape.
fn unescape_string(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push(match bytes[i + 1] {
                b'n' => b'\n',
                b't' => b'\t',
                b'"' => b'"',
                b'\\' => b'\\',
                other => other,
            });
            i += 2;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("a string literal's unescaped content is valid UTF-8")
}

/// Self-host-compile a whole program: drive the pipeline over every function, reconstruct
/// each into its codegen Body (grouping same-named heads into one multihead), run
/// codegen.kel, and splice the self-hosted ops, constant pool, and local_count into the
/// reference module chunk of that name. Native chunks (absent from the source) keep the
/// reference's ops. The result is a runnable module whose every source-defined chunk was
/// emitted by the self-hosted pipeline.
pub fn self_host_compile(src: &str) -> Module {
    let (fns, names, _data_records, _enum_records) = parse_functions_fused(src);
    let mut module = compile_src(src);
    let mut i = 0;
    while i < fns.len() {
        let name = names[fns[i].name as usize].clone();
        // Group consecutive same-named heads (a multiheaded function is one chunk).
        let mut group: Vec<&ParsedFn> = vec![&fns[i]];
        let mut j = i + 1;
        while j < fns.len() && names[fns[j].name as usize] == name {
            group.push(&fns[j]);
            j += 1;
        }
        i = j;
        let pc = group[0].params;
        // More than one head, or one guarded head, compiles as a multihead dispatch;
        // anything else as a single body. See `is_multihead_group` for why this is not
        // decided by the declaration keyword.
        // The reconstruction runs through the self-hosted reconstruct.kel stage, so the
        // whole compile path is Keleusma and the host only moves data between stages.
        let body = if is_multihead_group(&group) {
            reconstruct_via_kel_multihead(&group, pc)
        } else {
            let category = if group[0].cat == 3 { 2 } else { 0 };
            reconstruct_via_kel(&group[0].body, category, pc)
        };
        let (ops, pool, lc) = run_codegen(&body, pc);
        let idx = module
            .chunks
            .iter()
            .position(|c| c.name == name)
            .unwrap_or_else(|| panic!("no chunk named `{name}`"));
        module.chunks[idx].ops = ops;
        module.chunks[idx].constants = pool_to_constants(&pool, &names);
        module.chunks[idx].local_count = lc as u16;
    }
    module
}

/// The chunk NAME for each chunk index, derived from the parsed function stream.
///
/// # Why this has to be derived rather than read off a record
///
/// A `Call` node carries the callee's CHUNK INDEX, not its name -- `reconstruct.kel`
/// packs `chunk + count * 256` into the node's `arg`. The type channel is keyed by
/// NAMES, so an alias row for `let a = g()` needs the index turned back into `g`.
///
/// **IT DELEGATES, BECAUSE THE TABLE ALREADY EXISTED.** `first_pass` computes
/// `chunks` -- the deduplicated, lexicographically sorted set of function name ids,
/// in the module's chunk order -- and `parse.kel` is seeded from it. This is a
/// name lookup over that table and nothing more.
///
/// # I DERIVED IT BY HAND FIRST, GOT IT WRONG TWICE, AND IT WAS ALREADY WRITTEN DOWN
///
/// The first version grouped **consecutive same-named heads in declaration
/// order**, reasoning from the grouping [`self_host_compile_fused`] flushes on.
/// That grouping is real -- a multi-arm function is one chunk -- and it is not the
/// numbering. It produced the right chunk COUNT and the right SET of names in the
/// wrong ORDER, so every `Call` node would have resolved to another function, with
/// nothing about the count or the set looking wrong.
///
/// It also **passed the small multi-arm probe**, because there the two rules
/// coincide. Only the real corpus separated them.
///
/// The second version sorted, and then disagreed on `wire.kel` -- which was
/// recorded as an unexplained divergence and excluded from the corpus test.
/// **That divergence was the hand-rolled derivation inheriting a defect**, not a
/// property of the numbering: this version agrees with the reference on every
/// stage, `wire.kel` included.
///
/// **The rule was documented in three places before any of that**, including on
/// `chunk_table_from_tokens` and beside the seeding in `parse_functions_impl`.
/// Checking for it would have cost one search and saved two wrong derivations and
/// a spurious open finding. That is the sixth instance of this pattern in one
/// session and the first that reached the tree.
///
/// # Still checked, not assumed
///
/// `the_derived_chunk_names_match_the_reference_compiler` compares this against
/// `Module::chunks`' own names over the whole stage corpus, with a non-vacuity
/// guard. Delegation is a reason to expect agreement, not evidence of it.
#[must_use]
pub fn chunk_names_from_pipeline(src: &str) -> Vec<String> {
    let first = first_pass(src);
    first
        .chunks
        .iter()
        .map(|&id| first.names.get(id as usize).cloned().unwrap_or_default())
        .collect()
}

/// The type checker's BINDING ROWS, derived from the self-hosted pipeline.
///
/// Returns `(name, tag, form)` triples with names as STRINGS rather than interned
/// ids, so a caller can compare them against an extraction built over a different id
/// space. `form` matches the stage's `ty.bform`: 0 the value is a tag, 1 it is
/// another name the stage resolves through one alias hop.
///
/// # What this replaces, and why it could not be written until now
///
/// Order 1 records that this input should come from `parse.kel` plus
/// `reconstruct.kel` because "structure is available" there. It was half true.
/// Function names, declared return types and declared parameter types were all
/// reachable; **the names of the things being bound were not**. A `Local` record
/// carries a slot, and the type channel is keyed by names.
///
/// Two records closed that. The parameter's name was **already in the stream** —
/// the header emits `4 + name * 64` and the driver discarded the payload. The
/// `let` binding's name is the record added under the operator's ruling on that
/// fork. Neither invents an encoding, which is what Order 1 asked for.
///
/// # What it does NOT yet cover
///
/// A `let` bound to a literal or a call still has no tag here: the initialiser's
/// SHAPE lives in the body record stream, and reading it means walking the forest
/// rather than the header. That walk is the next slice; this one carries the
/// declared bindings, which are the ones the source states outright.
pub fn binding_rows_from_pipeline(src: &str) -> (Vec<String>, Vec<(String, i64, i64)>) {
    let (fns, names, ..) = parse_functions_fused(src);
    // A type NAME id to the stage's scalar tag. The ids come from the same intern
    // table the records index, so this is a lookup rather than a second convention.
    let tag_of = |type_name_id: i64| -> i64 {
        match names.get(type_name_id as usize).map(String::as_str) {
            Some("Word") => 1,
            // **LOWERCASE, AND IT IS THE ONLY PRIMITIVE THAT IS.** `Word`, `Byte`
            // and `Float` are capitalised; `bool` is not. `Bool` with a capital
            // letter is an ordinary NAMED type, which the reference refuses to add
            // to a `Word`, and an earlier revision of this table mapped it here —
            // telling the type channel that a value of some user type was a boolean.
            Some("bool") => 2,
            Some("Byte") => 3,
            _ => 0,
        }
    };
    let name_of = |id: i64| -> Option<String> { names.get(id as usize).cloned() };

    let mut rows: Vec<(String, i64, i64)> = Vec::new();
    for f in &fns {
        // The function's own name carries its declared return type, which is what a
        // `let a = g()` alias hop resolves through.
        if let Some(n) = name_of(f.name) {
            let t = tag_of(f.return_type);
            if t != 0 {
                rows.push((n, t, 0));
            }
        }
        for (i, &pid) in f.param_names.iter().enumerate() {
            let Some(n) = name_of(pid) else { continue };
            let t = f.param_types.get(i).copied().map_or(0, tag_of);
            if t != 0 {
                rows.push((n, t, 0));
            }
        }

        // --- `let` BINDINGS, VIA THE RECONSTRUCTED FOREST -----------------------
        //
        // **NOT BY LOOKING AT THE RECORD NEXT TO THE `LetIn`.** `LetIn` is BINARY
        // and pops its right child then its left, so the postfix stream for
        // `let a = 7; a` is `[Literal(7), Local(0), LetIn(0)]` and the record
        // immediately before the `LetIn` is the CONTINUATION, not the initialiser.
        // Reasoning from adjacency picks the wrong node every time.
        //
        // The forest gives the right answer directly: `lhs` is the initialiser and
        // `rhs` the continuation. Built by `reconstruct_via_kel`, which is the
        // validated walker — writing a second one here is the mistake the `v0.3.0`
        // line recorded when an independently written walk reported 365 of 386
        // loops disagreeing.
        if f.let_names.is_empty() {
            continue;
        }
        let body = reconstruct_via_kel(&f.body, f.cat, f.params);
        for node in &body.nodes {
            if node.kind != NODE_LET_IN {
                continue;
            }
            // **JOINED BY SLOT, NOT BY POSITION.** `LetIn`'s payload is the frame
            // slot and `let_names` carries `(slot, name)`. Pairing by fold order
            // would be positional and would fail silently on a reordering.
            let Some(&(_, nid)) = f.let_names.iter().find(|(slot, _)| *slot == node.arg) else {
                continue;
            };
            let Some(bound) = name_of(nid) else { continue };
            let Some(init) = body.nodes.get(node.lhs as usize) else {
                continue;
            };
            match init.kind {
                // A literal. `push_literal` interns through `intern_int`, so a
                // kind-1 node is always an integer; booleans are `Unit` carrying
                // the `PushImmediate` operand.
                NODE_LITERAL => rows.push((bound, 1, 0)),
                NODE_UNIT if init.arg == 1 || init.arg == 2 => rows.push((bound, 2, 0)),
                // **A CALL IS NOT EMITTED HERE, AND THE REASON IS THE ROW SHAPE
                // RATHER THAN THE PIPELINE.** `let a = g()` is a form-1 alias whose
                // row carries the TARGET'S NAME ID in the tag position. The two
                // extractions do not share an id space — the reference numbers by
                // insertion order as it walks, this one uses the lexer's intern
                // table — so a form-1 row cannot be compared by name string, which
                // is the discipline that keeps this honest.
                //
                // Emitting one would mean either comparing id spaces (comparing the
                // numbering rather than the content) or changing the row shape to
                // carry a target string. The second is the right answer and it is a
                // slice of its own.
                //
                // Everything else — an operator expression above all — needs the
                // initialiser's NODE INDEX to reach the stage's bounded fixpoint
                // (form 2), which is a further slice again. Leaving no row means the
                // stage accepts, the documented conservative stance.
                _ => {}
            }
        }
    }
    (names, rows)
}

/// Self-host-compile a whole program **without ever holding every function's records**.
///
/// Identical output to [`self_host_compile`], which is the point: the boundary between
/// `parse` and `reconstruct` is cut at FUNCTION granularity, and
/// `the_fused_compile_agrees_with_the_collecting_one` checks the two modules chunk for
/// chunk. Only the residency differs.
///
/// # What is resident, and what it is worth
///
/// [`self_host_compile`] calls [`parse_functions`] first, so every function's postorder
/// records for the whole program are live before the first one is reconstructed. This
/// holds one GROUP -- consecutive same-named heads, which are one chunk -- and drops it
/// as soon as the group is compiled. Measured over the corpus:
///
/// | stage | all records | largest group | ratio |
/// |---|---|---|---|
/// | `wire` | 8,785 | 214 | 41.1x |
/// | `parse` | 12,111 | 931 | 13.0x |
/// | `codegen` | 7,359 | 762 | 9.7x |
/// | `lexer` | 1,415 | 276 | 5.1x |
/// | `analyze` | 1,538 | 324 | 4.7x |
/// | `reconstruct` | 3,222 | 885 | 3.6x |
/// | `verify_typed` | 1,313 | 382 | 3.4x |
///
/// The largest stage benefits most, which is the direction that matters.
///
/// # The group is a one-function lookahead, not a whole-input fact
///
/// A group ends when the next function's name differs, so a completed function cannot be
/// compiled until the following header arrives. That is a bounded lookahead of one
/// function, not a dependency on the whole stream. **In this corpus the largest group is
/// exactly the largest single function in every stage**, so grouping costs no residency
/// at all here -- but that is what the corpus contains, not a bound on what the language
/// admits, and it must not be offered as one.
pub fn self_host_compile_fused(src: &str) -> Module {
    let mut module = compile_src(src);
    // The pending group: consecutive heads sharing a name. Flushed when the name changes
    // and again at end of input, which is the only place the last group can be closed.
    let mut group: Vec<ParsedFn> = Vec::new();
    let mut group_name = String::new();

    // **THE NAME TABLE IS A PARAMETER, NOT A RE-DERIVATION.** A `StaticStr` pool entry
    // carries the lexer's intern id, so resolving it needs the table, and this closure runs
    // INSIDE the streaming callback where the table is what the feed has interned so far.
    // Calling `parse_functions_fused` a second time here to obtain one would discard the
    // bounded record residency this entry point exists to provide.
    let flush = |group: &mut Vec<ParsedFn>, name: &str, module: &mut Module, names: &[String]| {
        if group.is_empty() {
            return;
        }
        let pc = group[0].params;
        let refs: Vec<&ParsedFn> = group.iter().collect();
        let body = if is_multihead_group(&refs) {
            reconstruct_via_kel_multihead(&refs, pc)
        } else {
            let category = if group[0].cat == 3 { 2 } else { 0 };
            reconstruct_via_kel(&group[0].body, category, pc)
        };
        let (ops, pool, lc) = run_codegen(&body, pc);
        let idx = module
            .chunks
            .iter()
            .position(|c| c.name == name)
            .unwrap_or_else(|| panic!("no chunk named `{name}`"));
        module.chunks[idx].ops = ops;
        module.chunks[idx].constants = pool_to_constants(&pool, names);
        module.chunks[idx].local_count = lc as u16;
        group.clear();
    };

    let (names, ..) = parse_functions_impl_named(src, &mut |names, f| {
        let name = names[f.name as usize].clone();
        if !group.is_empty() && name != group_name {
            flush(&mut group, &group_name, &mut module, names);
        }
        group_name = name;
        group.push(f);
    });
    // The trailing group is flushed against the FINAL table, which is a superset of every
    // intermediate one: the interner only grows, and an id it has issued keeps its meaning.
    flush(&mut group, &group_name, &mut module, &names);
    module
}

/// The peak record residency of a fused compile, for the residency assertions.
///
/// Returns `(peak_group_records, total_records)`. Separated from
/// [`self_host_compile_fused`] so the compile path carries no measurement apparatus, and
/// so a test asserting the ratio reads the same numbers the compile would hold.
pub fn fused_compile_residency(src: &str) -> (usize, usize) {
    let mut group: Vec<ParsedFn> = Vec::new();
    let mut group_name = String::new();
    let (mut peak, mut total) = (0usize, 0usize);
    parse_functions_impl_named(src, &mut |names, f| {
        let name = names[f.name as usize].clone();
        if !group.is_empty() && name != group_name {
            group.clear();
        }
        group_name = name;
        let n = f.body.len() + f.guard.len();
        total += n;
        group.push(f);
        peak = peak.max(
            group
                .iter()
                .map(|g| g.body.len() + g.guard.len())
                .sum::<usize>(),
        );
    });
    (peak, total)
}

// -- reconstruct.kel drivers (ported from tests/selfhost_codegen.rs) --------

// Flat shared-slot offsets of reconstruct.kel's single `io` block: the record
// input, then the codegen.kel-mirroring forest output, then the multihead input.
const RC_REC_COUNT: usize = 0;
const RC_IN_CATEGORY: usize = 1;
const RC_IN_PARAM: usize = 2;
const RC_REC_KIND: usize = 3;
const RC_REC_ARG: usize = 3 + 1024;
const RC_AST_BASE: usize = 3 + 1024 * 2;
const RC_AST_ROOT: usize = RC_AST_BASE;
const RC_AST_KINDS: usize = RC_AST_BASE + 1;
const RC_AST_ARGS: usize = RC_AST_BASE + 1 + 1024;
const RC_AST_LHS: usize = RC_AST_BASE + 1 + 1024 * 2;
const RC_AST_RHS: usize = RC_AST_BASE + 1 + 1024 * 3;
const RC_AST_CALL_ARGS: usize = RC_AST_BASE + 1 + 1024 * 4;
const RC_AST_MATCH_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 256 * 2;
const RC_AST_LIMIT_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 256 * 3;
const RC_AST_HEAD_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 256 * 4;
const RC_AST_CATEGORY: usize = RC_AST_BASE + 1 + 1024 * 4 + 256 * 5 + 1;
const RC_HEAD_COUNT: usize = RC_AST_BASE + 1 + 1024 * 4 + 256 * 5 + 2;
const RC_HEAD_GUARD_START: usize = RC_HEAD_COUNT + 1;
const RC_HEAD_GUARD_LEN: usize = RC_HEAD_COUNT + 1 + 16;
const RC_HEAD_BODY_START: usize = RC_HEAD_COUNT + 1 + 16 * 2;
const RC_HEAD_BODY_LEN: usize = RC_HEAD_COUNT + 1 + 16 * 3;

/// Drive reconstruct.kel over one function's postorder records and read back the
/// reconstructed forest as a `Body`. This increment reads only the node arrays and
/// the root/category; the side arrays (call/for/match) arrive with those kinds.
// Compile reconstruct.kel once and clone the module per call: `self_host_compile`
// drives it for every function, so recompiling each time dominates the runtime.
/// The compiled `reconstruct.kel` stage module, cached after the first build.
///
/// Public so an external harness can construct its own [`Vm`] and drive the
/// stage on real input via [`seed_reconstruct_shared`] or
/// [`seed_reconstruct_multihead_shared`].
pub fn reconstruct_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/reconstruct.kel")))
        .clone()
}

/// The shared buffer `reconstruct.kel` consumes for ONE function body, seeded
/// and ready to drive.
///
/// **`reconstruct` has TWO entry points and this covers one of them.** The other
/// is the multihead form, [`seed_reconstruct_multihead_shared`], whose input is a
/// group of heads rather than a record stream. An accessor for this one alone
/// would drive the path that has never been the problem: the multihead dispatch
/// predicate was wrong in both directions and no oracle caught it, because every
/// corpus input agreed on keyword and head count.
pub fn seed_reconstruct_shared(
    vm: &Vm<'_, '_>,
    records: &[(i64, i64)],
    category: i64,
    param_count: usize,
) -> Vec<u8> {
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, RC_REC_COUNT, Value::Int(records.len() as i64))
        .unwrap();
    vm.set_shared(&mut shared, RC_IN_CATEGORY, Value::Int(category))
        .unwrap();
    vm.set_shared(&mut shared, RC_IN_PARAM, Value::Int(param_count as i64))
        .unwrap();
    for (i, &(k, a)) in records.iter().enumerate() {
        vm.set_shared(&mut shared, RC_REC_KIND + i, Value::Int(k))
            .unwrap();
        vm.set_shared(&mut shared, RC_REC_ARG + i, Value::Int(a))
            .unwrap();
    }
    shared
}

fn reconstruct_via_kel(records: &[(i64, i64)], category: i64, param_count: usize) -> Body {
    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
    let mut shared = seed_reconstruct_shared(&vm, records, category, param_count);
    let node_count = match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call")
    {
        VmState::Yielded(Value::Int(n)) => n as usize,
        other => panic!("unexpected reconstruct.kel state: {other:?}"),
    };
    let rd = |vm: &Vm<'_, '_>, shared: &[u8], slot: usize| -> i64 {
        match vm.get_shared(shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    let root = rd(&vm, &shared, RC_AST_ROOT);
    let mut nodes = Vec::with_capacity(node_count);
    for i in 0..node_count {
        nodes.push(Node {
            kind: rd(&vm, &shared, RC_AST_KINDS + i),
            arg: rd(&vm, &shared, RC_AST_ARGS + i),
            lhs: rd(&vm, &shared, RC_AST_LHS + i),
            rhs: rd(&vm, &shared, RC_AST_RHS + i),
        });
    }
    // Read each 256-entry side array in full; the caller compares only the prefix the
    // Rust reconstruction populated.
    let read_side = |vm: &Vm<'_, '_>, shared: &[u8], base: usize| -> Vec<i64> {
        (0..256).map(|k| rd(vm, shared, base + k)).collect()
    };
    let call_args = read_side(&vm, &shared, RC_AST_CALL_ARGS);
    let match_parts = read_side(&vm, &shared, RC_AST_MATCH_PARTS);
    let limit_parts = read_side(&vm, &shared, RC_AST_LIMIT_PARTS);
    Body {
        nodes,
        call_args,
        for_parts: Vec::new(),
        match_parts,
        limit_parts,
        head_parts: Vec::new(),
        category: rd(&vm, &shared, RC_AST_CATEGORY),
        root,
    }
}

/// Drive reconstruct.kel over a group of same-named heads (a multiheaded function),
/// feeding each head's guard and body record ranges, and read back the reconstructed
/// multihead `Body`.
/// The shared buffer `reconstruct.kel` consumes for a group of same-named heads
/// (a multiheaded function), seeded and ready to drive.
///
/// **THIS IS THE SECOND `reconstruct` ENTRY POINT AND THE ONE THAT MATTERED.**
/// Its input is a head group, not a record stream, so
/// [`seed_reconstruct_shared`] cannot stand in for it. The `v0.3.0` line asked
/// for this one specifically: the multihead dispatch predicate was once wrong in
/// both directions and no oracle caught it, because every corpus input agreed on
/// keyword and head count.
///
/// The per-head guard and body ranges are computed here rather than passed in,
/// because they are offsets into the concatenation this function performs and
/// have no meaning outside it.
pub fn seed_reconstruct_multihead_shared(
    vm: &Vm<'_, '_>,
    heads: &[&ParsedFn],
    pc: usize,
) -> Vec<u8> {
    // Concatenate every head's guard then body records, tracking the per-head offsets.
    let mut recs: Vec<(i64, i64)> = Vec::new();
    let mut gs = Vec::new();
    let mut gl = Vec::new();
    let mut bs = Vec::new();
    let mut bl = Vec::new();
    for h in heads {
        gs.push(recs.len());
        gl.push(h.guard.len());
        recs.extend_from_slice(&h.guard);
        bs.push(recs.len());
        bl.push(h.body.len());
        recs.extend_from_slice(&h.body);
    }
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(vm, &mut shared, RC_REC_COUNT, recs.len() as i64);
    set(vm, &mut shared, RC_IN_CATEGORY, 3);
    set(vm, &mut shared, RC_IN_PARAM, pc as i64);
    for (i, &(k, a)) in recs.iter().enumerate() {
        set(vm, &mut shared, RC_REC_KIND + i, k);
        set(vm, &mut shared, RC_REC_ARG + i, a);
    }
    set(vm, &mut shared, RC_HEAD_COUNT, heads.len() as i64);
    for h in 0..heads.len() {
        set(vm, &mut shared, RC_HEAD_GUARD_START + h, gs[h] as i64);
        set(vm, &mut shared, RC_HEAD_GUARD_LEN + h, gl[h] as i64);
        set(vm, &mut shared, RC_HEAD_BODY_START + h, bs[h] as i64);
        set(vm, &mut shared, RC_HEAD_BODY_LEN + h, bl[h] as i64);
    }
    shared
}

fn reconstruct_via_kel_multihead(heads: &[&ParsedFn], pc: usize) -> Body {
    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
    let mut shared = seed_reconstruct_multihead_shared(&vm, heads, pc);
    let node_count = match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call")
    {
        VmState::Yielded(Value::Int(n)) => n as usize,
        other => panic!("unexpected reconstruct.kel state: {other:?}"),
    };
    let rd = |vm: &Vm<'_, '_>, shared: &[u8], slot: usize| -> i64 {
        match vm.get_shared(shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    let root = rd(&vm, &shared, RC_AST_ROOT);
    let mut nodes = Vec::with_capacity(node_count);
    for i in 0..node_count {
        nodes.push(Node {
            kind: rd(&vm, &shared, RC_AST_KINDS + i),
            arg: rd(&vm, &shared, RC_AST_ARGS + i),
            lhs: rd(&vm, &shared, RC_AST_LHS + i),
            rhs: rd(&vm, &shared, RC_AST_RHS + i),
        });
    }
    let read_side = |vm: &Vm<'_, '_>, shared: &[u8], base: usize| -> Vec<i64> {
        (0..64).map(|k| rd(vm, shared, base + k)).collect()
    };
    Body {
        nodes,
        call_args: read_side(&vm, &shared, RC_AST_CALL_ARGS),
        for_parts: Vec::new(),
        match_parts: read_side(&vm, &shared, RC_AST_MATCH_PARTS),
        limit_parts: read_side(&vm, &shared, RC_AST_LIMIT_PARTS),
        head_parts: read_side(&vm, &shared, RC_AST_HEAD_PARTS),
        category: rd(&vm, &shared, RC_AST_CATEGORY),
        root,
    }
}

// -- analyze.kel driver (ported from tests/selfhost_codegen.rs) --------------
//
// analyze.kel reformulates the reference verifier's recursive `wcet_region`/`wcmu_region`
// max traversals as one explicit region-frame stack, computing a Stream chunk's
// per-iteration WCET and WCMU from a marshalled op table. Each per-op quantity is the
// authoritative `Op::cost()`/`stack_growth()`/`stack_shrink()`/`heap_alloc()`; the stage
// self-hosts only the control-flow algorithm.

const WA_OP_COUNT: usize = 0;
const WA_STREAM_POS: usize = 1;
const WA_RESET_POS: usize = 2;
const WA_LOCAL_COUNT: usize = 3;
const WA_VSB: usize = 4;
const WA_ARENA_CAPACITY: usize = 5;
const WA_REGION_START: usize = 6;
const WA_REGION_END: usize = 7;
const WA_COST: usize = 8;
const WA_CLASS: usize = 8 + 1536;
const WA_ARG: usize = 8 + 1536 * 2;
const WA_GROWTH: usize = 8 + 1536 * 3;
const WA_SHRINK: usize = 8 + 1536 * 4;
const WA_HEAP: usize = 8 + 1536 * 5;
const WA_OPK: usize = 8 + 1536 * 6;
const WA_SLOT: usize = 8 + 1536 * 7;
const WA_CVAL: usize = 8 + 1536 * 8;
const WA_CINT: usize = 8 + 1536 * 9;
const WA_CALLEE_SLOTS: usize = 8 + 1536 * 10;
const WA_CALLEE_HEAP: usize = 8 + 1536 * 11;
const WA_OUT_WCET: usize = 8 + 1536 * 12;
const WA_OUT_STACK: usize = 8 + 1536 * 12 + 1;
const WA_OUT_HEAP: usize = 8 + 1536 * 12 + 2;
const WA_OUT_REJECT: usize = 8 + 1536 * 12 + 3;
const WA_OUT_VALID: usize = 8 + 1536 * 12 + 4;

fn analyze_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/analyze.kel")))
        .clone()
}

/// Classify an op for analyze.kel and verify_structural.kel as `(class, arg)`. The class tags
/// the control-flow role (0 plain, 1 If, 2 Else, 3 EndIf, 4 Loop, 5 EndLoop, 6 Break, 7 BreakIf,
/// 8 Trap, 9 Call); `arg` carries each control-transfer op's target: the branch/exit target for
/// If and Loop, the matching EndIf position for Else, the back-edge for EndLoop, and the loop
/// exit a Break/BreakIf jumps to. (analyze.kel reads `arg` only for If and Loop; the EndLoop,
/// Break, and BreakIf targets are ignored there and consumed only by verify_structural.kel's
/// target-equality checks, so populating them does not affect the resource analysis.)
/// The op classification tables `analyze.kel` consumes, re-exported from
/// [`crate::selfhost_host`] where they are always available.
///
/// **They live there rather than here so there is ONE table, not two.** This
/// module is gated on `self-host`, and `tests/selfhost_codegen.rs` builds
/// without it, so a consumer that could not reach these was previously obliged
/// to reproduce them — and the copy in that file had already drifted, keeping a
/// `_ => (0, 0)` catch-all after this table was made exhaustive and passing `0`
/// where it passes real branch targets. The differential meant to be the oracle
/// was running against the unrepaired copy.
pub use crate::selfhost_host::{analyze_class, analyze_opk};

/// The operand-stack `(growth, shrink)` analyze.kel accounts for `op` under the empty
/// resolver. Identical to `Op::stack_growth()`/`stack_shrink()` except for a native call: the
/// reference WCMU native arm uses `during_peak = offset + 1` and `offset += 1 - n` with `n`
/// the whole argument-count byte (the error-reify high bit included), which the generic
/// accounting reproduces exactly as `growth = 1, shrink = n_full_byte`.
fn analyze_stack_effect(op: &crate::bytecode::Op) -> (i64, i64) {
    use crate::bytecode::Op;
    match op {
        Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => (1, *n as i64),
        _ => (op.stack_growth() as i64, op.stack_shrink() as i64),
    }
}

/// The per-op arena-heap bytes analyze.kel accounts for `op`: the op's own construction
/// allocation (`Op::heap_alloc`) plus the copy-out a `GetData`/`GetDataIndexed` performs when
/// it reads a flat-composite shared slot (`shared_composite_copyout_bytes`). `shared_layout`
/// is empty for the shallow empty-resolver form (copy-out zero, matching
/// `wcmu_stream_iteration`) and the module's real layout for the transitive validator.
fn analyze_op_heap(
    op: &crate::bytecode::Op,
    chunk: &crate::bytecode::Chunk,
    shared_layout: &[crate::bytecode::SharedSlotLayout],
) -> i64 {
    use crate::bytecode::{Op, SHARED_SLOT_COMPOSITE_FLAG};
    let slot_copyout = |slot: usize| -> i64 {
        shared_layout
            .get(slot)
            .filter(|e| e.kind & SHARED_SLOT_COMPOSITE_FLAG != 0)
            .map_or(0, |e| e.len as i64)
    };
    let copyout = match op {
        Op::GetData(s) => slot_copyout(*s as usize),
        Op::GetDataIndexed(base, len) => (0..*len as usize)
            .map(|i| slot_copyout(*base as usize + i))
            .max()
            .unwrap_or(0),
        _ => 0,
    };
    op.heap_alloc(chunk) as i64 + copyout
}

/// Run analyze.kel over one chunk against `arena_capacity`, returning `(wcet, stack_bytes,
/// heap_bytes, reject, valid)`. The region is the Stream-to-Reset body for a Stream chunk and
/// the whole op range for a Func/Reentrant chunk, matching `compute_chunk_wcmu`. `chunk_wcmu`
/// resolves each `Op::Call` to the callee chunk's already-computed `(stack_bytes, heap_bytes)`
/// (indexed by chunk index); pass `&[]` for the shallow empty-resolver form, where every
/// callee folds in as zero. `shared_layout` sizes composite-shared-read copy-out (empty for
/// the shallow form).
fn run_analyze_kel(
    chunk: &crate::bytecode::Chunk,
    arena_capacity: i64,
    chunk_wcmu: &[(i64, i64)],
    shared_layout: &[crate::bytecode::SharedSlotLayout],
) -> (i64, i64, i64, bool, bool) {
    use crate::bytecode::{BlockType, Op};
    let vsb = crate::bytecode::VALUE_SLOT_SIZE_BYTES as i64;
    // The analysed region and the Stream/Reset positions (used only for a Stream chunk's WCET
    // overhead term; a Func/Reentrant chunk analyses its whole op range).
    let (region_start, region_end, sp, rp) = match chunk.block_type {
        BlockType::Stream => {
            let sp = chunk
                .ops
                .iter()
                .position(|o| matches!(o, Op::Stream))
                .expect("Stream op");
            let rp = chunk
                .ops
                .iter()
                .position(|o| matches!(o, Op::Reset))
                .expect("Reset op");
            (sp + 1, rp, sp, rp)
        }
        BlockType::Func | BlockType::Reentrant => (0, chunk.ops.len(), 0, 0),
    };
    assert!(chunk.ops.len() <= 1536, "analyze.kel op-table capacity");
    let m = analyze_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify analyze.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&vm, &mut shared, WA_OP_COUNT, chunk.ops.len() as i64);
    set(&vm, &mut shared, WA_STREAM_POS, sp as i64);
    set(&vm, &mut shared, WA_RESET_POS, rp as i64);
    set(&vm, &mut shared, WA_LOCAL_COUNT, chunk.local_count as i64);
    set(&vm, &mut shared, WA_VSB, vsb);
    set(&vm, &mut shared, WA_ARENA_CAPACITY, arena_capacity);
    set(&vm, &mut shared, WA_REGION_START, region_start as i64);
    set(&vm, &mut shared, WA_REGION_END, region_end as i64);
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        let (opk, slot, cval, cint) = analyze_opk(op, chunk);
        let (growth, shrink) = analyze_stack_effect(op);
        set(&vm, &mut shared, WA_COST + i, op.cost() as i64);
        set(&vm, &mut shared, WA_CLASS + i, class);
        set(&vm, &mut shared, WA_ARG + i, arg);
        set(&vm, &mut shared, WA_GROWTH + i, growth);
        set(&vm, &mut shared, WA_SHRINK + i, shrink);
        set(
            &vm,
            &mut shared,
            WA_HEAP + i,
            analyze_op_heap(op, chunk, shared_layout),
        );
        set(&vm, &mut shared, WA_OPK + i, opk);
        set(&vm, &mut shared, WA_SLOT + i, slot);
        set(&vm, &mut shared, WA_CVAL + i, cval);
        set(&vm, &mut shared, WA_CINT + i, cint);
        // A Call folds in the callee's transitive WCMU (in slots for the stack term, bytes
        // for the heap term). An unresolved callee (shallow mode) contributes zero.
        if let Op::Call(callee, _) = op {
            let (cs, ch) = chunk_wcmu.get(*callee as usize).copied().unwrap_or((0, 0));
            set(&vm, &mut shared, WA_CALLEE_SLOTS + i, cs / vsb);
            set(&vm, &mut shared, WA_CALLEE_HEAP + i, ch);
        }
    }
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call analyze.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected analyze.kel state: {other:?}"),
    }
    let rd = |slot: usize| -> i64 {
        match vm.get_shared(&shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    (
        rd(WA_OUT_WCET),
        rd(WA_OUT_STACK),
        rd(WA_OUT_HEAP),
        rd(WA_OUT_REJECT) != 0,
        rd(WA_OUT_VALID) != 0,
    )
}

/// Run analyze.kel over one Stream chunk (shallow, unbounded capacity) and report its
/// per-iteration `(wcet, stack_bytes, heap_bytes, reject, valid)`. A thin reporting wrapper
/// over `run_analyze_kel` with the empty resolver and empty layout.
pub fn analyze_stream_chunk(chunk: &crate::bytecode::Chunk) -> (i64, i64, i64, bool, bool) {
    run_analyze_kel(chunk, i64::MAX, &[], &[])
}

// --- Self-hosted structural verifier (verify_structural.kel) ---------------------------------
//
// The block-nesting, branch-target, operand-bounds, and block-type portion of the self-hosted
// structural verifier: `verify.rs`'s first structural pass and its second pass (block-type
// constraints), leaving only the third pass (productive-divergence analysis). It runs over a
// marshalled op table: the control-flow `(class, arg)` table `analyze.kel` also receives (with
// the EndLoop/Break/BreakIf targets populated by `analyze_class`), a parallel operand-bounds
// table `(opb, o1, o2, o3)` from `structural_opbounds`, a block-type marker table `mark` from
// `structural_marker`, and the per-chunk/per-module counts the checks validate against. The
// block-type pass's one inter-procedural input -- whether the chunk Calls an always-yielding
// chunk (`calls_ay`) -- is resolved here from the reference `compute_always_yielding` fixpoint
// (marshalled per chunk), pending that fixpoint's own self-hosting alongside the third pass. The
// shared block `sv` lays out the scalars `op_count` (0), `local_count` (1), `const_count` (2),
// `template_count` (3), `data_len` (4), `nchunks` (5), `word_bits` (6), `block_type` (7),
// `calls_ay` (8); the arrays `class` (9..), `arg`, `opb`, `o1`, `o2`, `o3`, `mark` (each 1536
// wide); and the verdict `out_reject`.

const SV_OP_COUNT: usize = 0;
const SV_LOCAL_COUNT: usize = 1;
const SV_CONST_COUNT: usize = 2;
const SV_TEMPLATE_COUNT: usize = 3;
const SV_DATA_LEN: usize = 4;
const SV_NCHUNKS: usize = 5;
const SV_WORD_BITS: usize = 6;
const SV_BLOCK_TYPE: usize = 7;
const SV_CALLS_AY: usize = 8;
const SV_CLASS: usize = 9;
const SV_ARG: usize = 9 + 1536;
const SV_OPB: usize = 9 + 1536 * 2;
const SV_O1: usize = 9 + 1536 * 3;
const SV_O2: usize = 9 + 1536 * 4;
const SV_O3: usize = 9 + 1536 * 5;
const SV_MARK: usize = 9 + 1536 * 6;
const SV_OUT_REJECT: usize = 9 + 1536 * 7;

/// The compiled `verify_structural.kel` stage module, cached after the first
/// build.
///
/// Public so an external harness can construct its own [`Vm`] and drive the
/// stage on real input via [`seed_verify_structural_shared`].
pub fn verify_structural_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/verify_structural.kel")))
        .clone()
}

/// The number of declared shared/private data slots (`data_layout.slots.len()`), or 0 when the
/// module declares no data layout, matching the reference `GetData`/`SetData` bound.
fn data_layout_slot_count(module: &Module) -> i64 {
    module
        .data_layout
        .as_ref()
        .map_or(0, |dl| dl.slots.len() as i64)
}

/// Classify an op's operand-bounds obligation as `(opb, o1, o2, o3)`, mirroring the reference's
/// operand-index checks (see `verify_structural.kel` for the `opb` tag meanings). For a `Call`,
/// `o3` is the callee chunk's local count resolved here (0 when the callee index is out of
/// range, in which case the callee-in-bounds check rejects first). An op with no operand index
/// to validate yields `(0, 0, 0, 0)`.
fn structural_opbounds(op: &crate::bytecode::Op, module: &Module) -> (i64, i64, i64, i64) {
    use crate::bytecode::{NewCompositeOperand, Op, StructField};
    use crate::value_layout::CompositeKind;
    match op {
        Op::GetData(s) | Op::SetData(s) => (1, *s as i64, 0, 0),
        Op::GetDataIndexed(b, l) | Op::SetDataIndexed(b, l) => (2, *b as i64, *l as i64, 0),
        Op::Const(i) | Op::IsStruct(i) => (3, *i as i64, 0, 0),
        Op::GetField(StructField::Boxed { name_const }) => (3, *name_const as i64, 0, 0),
        Op::IsEnum(e, v, d) => (4, *e as i64, *v as i64, *d as i64),
        Op::Call(callee, nargs) => {
            let callee_locals = module
                .chunks
                .get(*callee as usize)
                .map_or(0, |ch| ch.local_count as i64);
            (5, *callee as i64, *nargs as i64, callee_locals)
        }
        Op::WordToFixed(fb)
        | Op::FixedToWord(fb)
        | Op::FixedMul(fb)
        | Op::FixedDiv(fb)
        | Op::CheckedMul(fb)
        | Op::CheckedDiv(fb) => (6, *fb as i64, 0, 0),
        Op::GetLocal(s) | Op::SetLocal(s) => (7, *s as i64, 0, 0),
        Op::NewComposite(NewCompositeOperand::Boxed {
            kind: CompositeKind::Struct | CompositeKind::Enum,
            meta,
            ..
        }) => (8, *meta as i64, 0, 0),
        _ => (0, 0, 0, 0),
    }
}

/// Tag an op as a block-type marker for the second reference pass: 1 Yield, 2 Stream, 3 Reset,
/// 0 other. The stage counts these to enforce each block type's marker profile.
fn structural_marker(op: &crate::bytecode::Op) -> i64 {
    use crate::bytecode::Op;
    match op {
        Op::Yield => 1,
        Op::Stream => 2,
        Op::Reset => 3,
        _ => 0,
    }
}

/// The block type as the stage's tag: 0 Func, 1 Reentrant, 2 Stream.
fn block_type_tag(chunk: &crate::bytecode::Chunk) -> i64 {
    use crate::bytecode::BlockType;
    match chunk.block_type {
        BlockType::Func => 0,
        BlockType::Reentrant => 1,
        BlockType::Stream => 2,
    }
}

/// Run verify_structural.kel over one chunk of `module`, returning whether it rejects the
/// chunk's block nesting, branch targets, operand-index bounds, or block-type marker profile.
/// Marshals the control-flow `(class, arg)` table via `analyze_class`, the operand-bounds
/// `(opb, o1, o2, o3)` table via `structural_opbounds`, the block-type markers via
/// `structural_marker`, and the per-chunk/per-module counts. `always` is the module's
/// always-yielding chunk set (from `compute_always_yielding`), used to resolve the chunk's
/// delegated-yield flag. No op is executed: a deliberately malformed chunk is classified but
/// never run.
pub fn structural_reject_chunk_via_kel(
    module: &Module,
    chunk: &crate::bytecode::Chunk,
    always: &std::collections::BTreeSet<usize>,
) -> bool {
    let m = verify_structural_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_structural.kel");
    let mut shared = seed_verify_structural_shared(&vm, module, chunk, always);
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call verify_structural.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected verify_structural.kel state: {other:?}"),
    }
    match vm.get_shared(&shared, SV_OUT_REJECT).unwrap() {
        Value::Int(n) => n != 0,
        o => panic!("expected Int at out_reject, got {o:?}"),
    }
}

/// The shared buffer `verify_structural.kel` consumes for one chunk, seeded and
/// ready to drive.
///
/// `always` is the module's always-yielding chunk set; it resolves the chunk's
/// delegated-yield flag and cannot be derived from the chunk alone.
///
/// One encoding, two callers: [`structural_reject_chunk_via_kel`] seeds through
/// this rather than inline, so an external harness feeds the stage exactly what
/// the driver does.
pub fn seed_verify_structural_shared(
    vm: &Vm<'_, '_>,
    module: &Module,
    chunk: &crate::bytecode::Chunk,
    always: &std::collections::BTreeSet<usize>,
) -> Vec<u8> {
    assert!(
        chunk.ops.len() <= 1536,
        "verify_structural.kel op-table capacity"
    );
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(vm, &mut shared, SV_OP_COUNT, chunk.ops.len() as i64);
    set(vm, &mut shared, SV_LOCAL_COUNT, chunk.local_count as i64);
    set(
        vm,
        &mut shared,
        SV_CONST_COUNT,
        chunk.constants.len() as i64,
    );
    set(
        vm,
        &mut shared,
        SV_TEMPLATE_COUNT,
        chunk.struct_templates.len() as i64,
    );
    set(vm, &mut shared, SV_DATA_LEN, data_layout_slot_count(module));
    set(vm, &mut shared, SV_NCHUNKS, module.chunks.len() as i64);
    set(vm, &mut shared, SV_WORD_BITS, 1i64 << module.word_bits_log2);
    set(vm, &mut shared, SV_BLOCK_TYPE, block_type_tag(chunk));
    // Whether the chunk delegates its yield to an always-yielding callee (the reference's
    // `calls_always_yielder`). Resolved from the marshalled always-yielding set.
    let calls_ay = chunk
        .ops
        .iter()
        .any(|op| matches!(op, crate::bytecode::Op::Call(g, _) if always.contains(&(*g as usize))));
    set(vm, &mut shared, SV_CALLS_AY, i64::from(calls_ay));
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        set(vm, &mut shared, SV_CLASS + i, class);
        set(vm, &mut shared, SV_ARG + i, arg);
        let (opb, o1, o2, o3) = structural_opbounds(op, module);
        set(vm, &mut shared, SV_OPB + i, opb);
        set(vm, &mut shared, SV_O1 + i, o1);
        set(vm, &mut shared, SV_O2 + i, o2);
        set(vm, &mut shared, SV_O3 + i, o3);
        set(vm, &mut shared, SV_MARK + i, structural_marker(op));
    }
    shared
}

// --- Self-hosted yield-coverage kernel (verify_yield.kel) and Pass 3 -------------------------
//
// verify_yield.kel decides whether every fall-through path of a chunk region passes through a
// Yield (or a Call delegating to an always-yielding chunk), reproducing the reference
// `analyze_yield_coverage`. Its shared block `yv` lays out `op_count` (0), `region_start` (1),
// `region_end` (2); the arrays `class` (3..), `arg`, `mark`, `cay` (each 1536 wide, where `cay`
// flags a Call to an always-yielding chunk); and the results `out_fell`, `out_hy`. The driver
// runs it in two orchestrations, both self-hosting what was the reference borrow: the
// always-yielding monotone fixpoint (over `[0, op_count)` per chunk) and the Stream productivity
// check (over `[stream_pos + 1, reset_pos)`).

const YV_OP_COUNT: usize = 0;
const YV_REGION_START: usize = 1;
const YV_REGION_END: usize = 2;
const YV_CLASS: usize = 3;
const YV_ARG: usize = 3 + 1536;
const YV_MARK: usize = 3 + 1536 * 2;
const YV_CAY: usize = 3 + 1536 * 3;
const YV_OUT_FELL: usize = 3 + 1536 * 4;
const YV_OUT_HY: usize = 3 + 1536 * 4 + 1;

fn verify_yield_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/verify_yield.kel")))
        .clone()
}

/// Run verify_yield.kel over `chunk`'s region `[start, end)`, returning `(fell, hy)`: whether
/// some path falls through to `end`, and whether every such path yielded. `always` is the
/// current always-yielding chunk set, which flags each `Call`'s delegated yield (`cay`).
fn run_ayc(
    chunk: &crate::bytecode::Chunk,
    start: usize,
    end: usize,
    always: &std::collections::BTreeSet<usize>,
) -> (bool, bool) {
    use crate::bytecode::Op;
    assert!(
        chunk.ops.len() <= 1536,
        "verify_yield.kel op-table capacity"
    );
    let m = verify_yield_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_yield.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&vm, &mut shared, YV_OP_COUNT, chunk.ops.len() as i64);
    set(&vm, &mut shared, YV_REGION_START, start as i64);
    set(&vm, &mut shared, YV_REGION_END, end as i64);
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        set(&vm, &mut shared, YV_CLASS + i, class);
        set(&vm, &mut shared, YV_ARG + i, arg);
        set(&vm, &mut shared, YV_MARK + i, structural_marker(op));
        let cay = matches!(op, Op::Call(g, _) if always.contains(&(*g as usize)));
        set(&vm, &mut shared, YV_CAY + i, i64::from(cay));
    }
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call verify_yield.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected verify_yield.kel state: {other:?}"),
    }
    let rd = |slot: usize| -> i64 {
        match vm.get_shared(&shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    (rd(YV_OUT_FELL) != 0, rd(YV_OUT_HY) != 0)
}

/// The self-hosted always-yielding chunk set: the monotone fixpoint of verify_yield.kel over
/// `[0, op_count)` per chunk (a chunk joins the set when every path of it yields, using the set
/// from prior rounds for the delegated-yield contribution). A drop-in for the reference
/// `compute_always_yielding`, computed entirely by the self-hosted kernel.
pub fn self_hosted_always_yielding(module: &Module) -> std::collections::BTreeSet<usize> {
    let mut always: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    loop {
        let mut changed = false;
        for (i, chunk) in module.chunks.iter().enumerate() {
            if always.contains(&i) {
                continue;
            }
            let (fell, hy) = run_ayc(chunk, 0, chunk.ops.len(), &always);
            if fell && hy {
                always.insert(i);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    always
}

/// Whether `chunk` is an unproductive Stream chunk: some path from its `Stream` to its `Reset`
/// passes no Yield (directly or by delegation). Reproduces the reference Pass 3 via the
/// self-hosted kernel; `always` is the (self-hosted) always-yielding set. Non-Stream chunks and
/// chunks missing a Stream/Reset marker are not rejected here (Pass 2 handles the latter).
fn productivity_reject_via_kel(
    chunk: &crate::bytecode::Chunk,
    always: &std::collections::BTreeSet<usize>,
) -> bool {
    use crate::bytecode::{BlockType, Op};
    if chunk.block_type != BlockType::Stream {
        return false;
    }
    let sp = chunk.ops.iter().position(|o| matches!(o, Op::Stream));
    let rp = chunk.ops.iter().position(|o| matches!(o, Op::Reset));
    if let (Some(s), Some(r)) = (sp, rp) {
        let (fell, hy) = run_ayc(chunk, s + 1, r, always);
        fell && !hy
    } else {
        false
    }
}

// --- Self-hosted operand-stack depth-balance pass (verify_depth.kel) -------------------------
//
// verify_depth.kel reproduces the reference `verify_stack_depth`/`verify_depth_region`: it walks
// a chunk tracking the absolute operand-stack depth through the structured control flow and
// rejects any op that would underflow the operand stack (audit finding 3). Height-only (no
// shapes), it is the frame-stack twin of verify_yield.kel. Its shared block `dv` lays out
// `op_count` (0); the arrays `class` (1..), `arg`, `dreq`, `dnet`, `is_term` (each 1536 wide);
// and the verdict `out_reject`. `dreq`/`dnet` are the reference `op_depth_effect` (the actual
// operand consumption, NOT the WCMU stack effect); `is_term` flags Trap/Return.

const DV_OP_COUNT: usize = 0;
const DV_CLASS: usize = 1;
const DV_ARG: usize = 1 + 1536;
const DV_DREQ: usize = 1 + 1536 * 2;
const DV_DNET: usize = 1 + 1536 * 3;
const DV_IS_TERM: usize = 1 + 1536 * 4;
const DV_OUT_REJECT: usize = 1 + 1536 * 5;
/// Why `verify_depth.kel` returned the verdict it did.
///
/// 0 means the walk completed, so the verdict is a real analysis result. 1 means
/// the chunk nests deeper than the stage's declared cap and the verdict is
/// default-deny. **Only the cause distinguishes a proven defect from an
/// unanalysed program**, and only the cause says whether raising the cap would
/// change the answer.
const DV_OUT_CAUSE: usize = 2 + 1536 * 5;

/// The compiled `verify_depth.kel` stage module, cached after the first build.
///
/// Public so an external harness can construct its own [`Vm`] and drive the
/// stage on real input via [`seed_verify_depth_shared`].
pub fn verify_depth_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/verify_depth.kel")))
        .clone()
}

/// The shared buffer `verify_depth.kel` consumes for one chunk, seeded and
/// ready to drive.
///
/// **One encoding, two callers.** [`depth_reject_chunk_via_kel`] below uses this
/// rather than seeding inline, so an external harness driving the stage feeds it
/// exactly what the driver does. That is the point of the accessor and it is the
/// `v0.3.0` line's stated reason for wanting the `Vm` passed IN: a constructor of
/// its own would be a second encoding, free to drift from this one.
///
/// The `vm` argument supplies the shared layout only. Build it from
/// [`verify_depth_kel_module`].
pub fn seed_verify_depth_shared(vm: &Vm<'_, '_>, chunk: &crate::bytecode::Chunk) -> Vec<u8> {
    use crate::bytecode::Op;
    assert!(
        chunk.ops.len() <= 1536,
        "verify_depth.kel op-table capacity"
    );
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(vm, &mut shared, DV_OP_COUNT, chunk.ops.len() as i64);
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        let (req, net) = crate::verify::op_depth_effect(op, chunk);
        set(vm, &mut shared, DV_CLASS + i, class);
        set(vm, &mut shared, DV_ARG + i, arg);
        set(vm, &mut shared, DV_DREQ + i, i64::from(req.max(0)));
        set(vm, &mut shared, DV_DNET + i, i64::from(net));
        set(
            vm,
            &mut shared,
            DV_IS_TERM + i,
            i64::from(matches!(op, Op::Trap(_) | Op::Return)),
        );
    }
    shared
}

/// Run verify_depth.kel over one chunk, returning whether any op underflows the operand stack
/// (the reference `verify_stack_depth`). Marshals the control-flow `(class, arg)` table via
/// `analyze_class`, the actual operand consumption `(dreq, dnet)` via `op_depth_effect`, and the
/// Trap/Return terminator flag.
///
/// Seeds through [`seed_verify_depth_shared`], so this and any external harness
/// drive the stage from the same encoding.
pub fn depth_reject_chunk_via_kel(chunk: &crate::bytecode::Chunk) -> bool {
    let m = verify_depth_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_depth.kel");
    let mut shared = seed_verify_depth_shared(&vm, chunk);
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call verify_depth.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected verify_depth.kel state: {other:?}"),
    }
    match vm.get_shared(&shared, DV_OUT_REJECT).unwrap() {
        Value::Int(n) => n != 0,
        o => panic!("expected Int at out_reject, got {o:?}"),
    }
}

/// Why `verify_depth.kel` rejected `chunk`, distinguishing a proven operand
/// underflow from a refusal to analyse.
///
/// # Why this exists beside [`depth_reject_chunk_via_kel`]
///
/// The stage declares a nesting cap and refuses a chunk past it, default-deny, in
/// line with this project's conservative-verification stance. That refusal and a
/// proven underflow are the SAME verdict, so a caller reading only the verdict
/// cannot tell a defective program from one the analysis declined. **A caller
/// deciding whether to raise the cap needs this and the verdict does not carry
/// it.**
///
/// Returns [`DepthVerdict::Accept`], [`DepthVerdict::Underflow`], or
/// [`DepthVerdict::OverCap`].
pub fn depth_verdict_chunk_via_kel(chunk: &crate::bytecode::Chunk) -> DepthVerdict {
    let m = verify_depth_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_depth.kel");
    let mut shared = seed_verify_depth_shared(&vm, chunk);
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call verify_depth.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected verify_depth.kel state: {other:?}"),
    }
    let reject = match vm.get_shared(&shared, DV_OUT_REJECT).unwrap() {
        Value::Int(n) => n != 0,
        o => panic!("expected Int at out_reject, got {o:?}"),
    };
    let cause = match vm.get_shared(&shared, DV_OUT_CAUSE).unwrap() {
        Value::Int(n) => n,
        o => panic!("expected Int at out_cause, got {o:?}"),
    };
    match (reject, cause) {
        (false, _) => DepthVerdict::Accept,
        (true, 1) => DepthVerdict::OverCap,
        (true, _) => DepthVerdict::Underflow,
    }
}

/// The outcome of the self-hosted operand-stack depth-balance pass.
///
/// `Underflow` and `OverCap` are both rejections, and keeping them distinct is the
/// point: one is a proven property of the program and the other is a statement
/// about the analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthVerdict {
    /// The walk completed and found no operand-stack underflow.
    Accept,
    /// The walk completed and proved an op would underflow the operand stack.
    Underflow,
    /// The chunk nests deeper than the stage's declared cap, so it was refused
    /// without being analysed. **Not evidence of a defect.**
    OverCap,
}

/// Run the WHOLE self-hosted verifier over a module -- every check `verify()` performs, all
/// self-hosted: per chunk, the block-nesting/branch-target/operand-bounds and block-type checks
/// (verify_structural.kel), the productive-divergence check for Stream chunks (verify_yield.kel),
/// and the operand-stack depth-balance check (verify_depth.kel); then the module-level A.2.1
/// typed operand-stack pass (verify_typed.kel, seeded from the signature table) and its
/// data-layout validation (verify_datalayout.kel, B6/C4). The module is rejected iff any check
/// rejects, mirroring `verify()`. The always-yielding set is computed by the self-hosted fixpoint.
///
/// The typed pass reconstructs shapes with the operator's chosen sound over-approximation (operand
/// shapes reset to Top across control-flow boundaries, so cross-join precision is forgone -- a Top
/// defers to the retained runtime guard, never a false-reject); within that approximation every
/// reference check is reproduced exactly, including the Call argument-vs-parameter check and exact
/// composite-kind compatibility. There are no deferred verifier checks left in the wiring.
pub fn structural_reject_module_via_kel(module: &Module) -> bool {
    let always = self_hosted_always_yielding(module);
    let per_chunk = module.chunks.iter().any(|chunk| {
        structural_reject_chunk_via_kel(module, chunk, &always)
            || productivity_reject_via_kel(chunk, &always)
            || depth_reject_chunk_via_kel(chunk)
    });
    per_chunk || typed_reject_module_via_kel(module) || dl_reject_module_via_kel(module)
}

// --- Self-hosted A.2.1 typed operand-stack verifier, slices 2a+2b (verify_typed.kel) ---------
//
// verify_typed.kel is a frame-stack abstract interpreter over the WHOLE chunk (all control flow).
// It reconstructs each operand-stack entry's flat shape and validates every compiler-baked flat
// field/array offset against the composite's known size (audit B1/B2), the operand-stack
// underflow (finding 3), the if/else branch-height balance (B3/B4), the loop back-edge height
// neutrality (B5), and -- with signature/native/enum seeding (slice 2b) -- the seeded local
// composite-field offsets, `SetLocal` compatibility, resume shape, `Call`/native return shape,
// and enum body size (B8). Per the operator's chosen trade-off it uses the SOUND over-
// approximation the reference itself falls back to: shapes are precise within a basic block and
// reset to Top across every control-flow boundary (a loop also invalidates every local), so it
// never rejects a valid program -- a Top defers to the retained runtime guard -- and forgoes
// only the cross-join shape precision. `typed_run` marshals the per-op descriptor plus, in the
// seeded form, the per-slot seed shapes (from the chunk signature), the resume shape, the per-op
// Call/native return shapes, and the enum body sizes; an unseeded run reproduces the isolation
// `typed_check_chunk`. Every reference check is now reproduced: the non-enum `NewComposite`
// packed-size check, the Call argument-vs-parameter check (per-callee parameter shapes marshalled
// as `cp_*`, up to eight), and exact composite-kind compatibility (a `kind` word threaded through
// every shape). Together with the batched data-layout validation (slice 2c) the typed pass is
// wired into `structural_reject_module_via_kel`; within the sound over-approximation it is exact.

const TV_OP_COUNT: usize = 0;
const TV_RESUME_TAG: usize = 1;
const TV_RESUME_SIZE: usize = 2;
const TV_RESUME_KIND: usize = 3;
const TV_EB_COUNT: usize = 4;
const TV_CLASS: usize = 5;
const TV_ARG: usize = 5 + 1536;
const TV_IS_TERM: usize = 5 + 1536 * 2;
const TV_TK: usize = 5 + 1536 * 3;
const TV_REQ: usize = 5 + 1536 * 4;
const TV_PROD: usize = 5 + 1536 * 5;
const TV_TA: usize = 5 + 1536 * 6;
const TV_TB: usize = 5 + 1536 * 7;
const TV_TC: usize = 5 + 1536 * 8;
const TV_RET_TAG: usize = 5 + 1536 * 9;
const TV_RET_SIZE: usize = 5 + 1536 * 10;
const TV_RET_KIND: usize = 5 + 1536 * 11;
const TV_SEED_TAG: usize = 5 + 1536 * 12;
const TV_SEED_SIZE: usize = 5 + 1536 * 12 + 256;
const TV_SEED_KIND: usize = 5 + 1536 * 12 + 512;
const TV_EB_VALS: usize = 5 + 1536 * 12 + 768;
const TV_CP_TAG: usize = 5 + 1536 * 12 + 768 + 64;
const TV_CP_SIZE: usize = 5 + 1536 * 12 + 768 + 64 + 12288;
const TV_CP_KIND: usize = 5 + 1536 * 12 + 768 + 64 + 12288 * 2;
const TV_OUT_REJECT: usize = 5 + 1536 * 12 + 768 + 64 + 12288 * 3;
const TV_CP_STRIDE: usize = 8;

/// The compiled `verify_typed.kel` stage module, cached after the first build.
///
/// Public so an external harness can construct its own [`Vm`] and drive the
/// stage on real input via [`seed_verify_typed_shared`].
pub fn verify_typed_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/verify_typed.kel")))
        .clone()
}

/// The byte size of a scalar-shaped constant (`const_abs` restricted to its scalar arms), or
/// `None` for a composite/unknown constant (which the reference leaves `Top`).
fn const_scalar_size(
    cv: Option<&crate::bytecode::ConstValue>,
    wb: usize,
    fb: usize,
) -> Option<i64> {
    use crate::bytecode::ConstValue;
    use crate::value_layout::ScalarKind;
    let sz = |k: ScalarKind| k.size_in_bytes(wb, fb) as i64;
    match cv {
        Some(ConstValue::Unit) => Some(sz(ScalarKind::Unit)),
        Some(ConstValue::Bool(_)) => Some(sz(ScalarKind::Bool)),
        Some(ConstValue::Int(_)) => Some(sz(ScalarKind::Int)),
        Some(ConstValue::Byte(_)) => Some(sz(ScalarKind::Byte)),
        Some(ConstValue::Fixed(_)) => Some(sz(ScalarKind::Fixed)),
        Some(ConstValue::StaticStr(_)) => Some(sz(ScalarKind::Text)),
        // A `Float` constant (present only under the parent crate's `floats` feature, which the
        // subproject cannot gate on) falls through to `None` -> Top: a sound defer, since a
        // float const is never a composite operand a flat access would need sized.
        _ => None,
    }
}

/// The typed op descriptor verify_typed.kel applies for `op`:
/// `(class, arg, is_term, tk, req, prod, ta, tb)`. `(class, arg)` is the control-flow role/target
/// (via `analyze_class`); `is_term` flags Trap/Return; `(req, prod)` is the actual operand
/// consumption and generic push count from the reference `op_depth_effect` (NOT the WCMU
/// `stack_growth`/`shrink`, which mis-state ops like `Yield`); `tk`/`ta`/`tb` is the shape
/// transfer (see verify_typed.kel), 0 generic for every op except the shape producers/consumers.
fn typed_desc(
    op: &crate::bytecode::Op,
    chunk: &crate::bytecode::Chunk,
    wb: usize,
    fb: usize,
) -> (i64, i64, i64, i64, i64, i64, i64, i64, i64) {
    use crate::bytecode::{ArrayElem, EnumField, NewCompositeOperand, Op, StructField, TupleField};
    use crate::value_layout::CompositeKind;
    let (class, arg) = analyze_class(op);
    let is_term = i64::from(matches!(op, Op::Return | Op::Trap(_)));
    let (r, net) = crate::verify::op_depth_effect(op, chunk);
    let req = i64::from(r.max(0));
    let prod = i64::from((net + r.max(0)).max(0));
    // The shape transfer kind, its operands, and (for flat composites) the composite kind `tc`
    // (the reference's `CompositeKind::to_tag`); everything not listed is generic (tk 0).
    let (tk, ta, tb, tc): (i64, i64, i64, i64) = match op {
        Op::Dup => (1, 0, 0, 0),
        Op::IsEnum(_, _, _) | Op::IsStruct(_) => (2, 1, 0, 0),
        Op::GetLocal(i) => (3, *i as i64, 0, 0),
        Op::SetLocal(i) => (4, *i as i64, 0, 0),
        Op::Yield => (11, 0, 0, 0),
        Op::Call(_, _) | Op::CallVerifiedNative(_, _) | Op::CallExternalNative(_, _) => {
            (12, 0, 0, 0)
        }
        Op::Const(idx) => match const_scalar_size(chunk.constants.get(*idx as usize), wb, fb) {
            Some(sz) => (2, sz, 0, 0),
            None => (0, 0, 0, 0),
        },
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Enum,
            byte_size,
            ..
        }) => (
            14,
            0,
            *byte_size as i64,
            i64::from(CompositeKind::Enum.to_tag()),
        ),
        Op::NewComposite(NewCompositeOperand::Flat {
            kind, byte_size, ..
        }) => (6, 0, *byte_size as i64, i64::from(kind.to_tag())),
        Op::GetField(StructField::Flat { offset, kind })
        | Op::GetTupleField(TupleField::Flat { offset, kind })
        | Op::GetEnumField(EnumField::Flat { offset, kind }) => {
            (7, *offset as i64, kind.size_in_bytes(wb, fb) as i64, 0)
        }
        Op::GetField(StructField::FlatNested {
            offset,
            size,
            variant,
        })
        | Op::GetTupleField(TupleField::FlatNested {
            offset,
            size,
            variant,
        })
        | Op::GetEnumField(EnumField::FlatNested {
            offset,
            size,
            variant,
        }) => (8, *offset as i64, *size as i64, i64::from(variant.to_tag())),
        Op::GetIndex(ArrayElem::Flat { kind }) => (9, kind.size_in_bytes(wb, fb) as i64, 0, 0),
        Op::GetIndex(ArrayElem::FlatNested { size, variant }) => {
            (10, *size as i64, 0, i64::from(variant.to_tag()))
        }
        _ => (0, 0, 0, 0),
    };
    (class, arg, is_term, tk, req, prod, ta, tb, tc)
}

/// Lift a wire signature shape into the stage's `(tag, size, kind)` lattice, mirroring
/// `AbsVal::from_wire`: Top -> (0,0,0); a decodable scalar -> (1, byte size, 0); a decodable flat
/// composite -> (2, byte size, composite-kind tag); an undecodable tag -> Top.
fn abs_from_wire(shape: &crate::bytecode::WireShape, wb: usize, fb: usize) -> (i64, i64, i64) {
    use crate::bytecode::WireShape;
    use crate::value_layout::{CompositeKind, ScalarKind};
    match shape {
        WireShape::Top => (0, 0, 0),
        WireShape::Scalar { kind } => match ScalarKind::from_tag(*kind) {
            Some(k) => (1, k.size_in_bytes(wb, fb) as i64, 0),
            None => (0, 0, 0),
        },
        WireShape::Flat { kind, size } => match CompositeKind::from_tag(*kind) {
            Some(_) => (2, *size as i64, i64::from(*kind)),
            None => (0, 0, 0),
        },
    }
}

/// Run verify_typed.kel over one chunk. `sig` seeds the local frame and resume shape (Phase 2b);
/// `None` is the isolation check (all Top). When `sig` is `Some` the module tables are also used
/// to seed each `Call`/native return shape and the enum body sizes (B8); the isolation form
/// leaves them empty, reproducing `typed_check_chunk`. Returns whether the interpreter rejects a
/// flat offset, an underflow, a height imbalance, a `SetLocal` shape mismatch, or an enum body
/// size mismatch.
fn typed_run(
    module: &Module,
    chunk: &crate::bytecode::Chunk,
    sig: Option<&crate::bytecode::ChunkSignature>,
    wb: usize,
    fb: usize,
) -> bool {
    let m = verify_typed_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_typed.kel");
    let mut shared = seed_verify_typed_shared(&vm, module, chunk, sig, wb, fb);
    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call verify_typed.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected verify_typed.kel state: {other:?}"),
    }
    match vm.get_shared(&shared, TV_OUT_REJECT).unwrap() {
        Value::Int(n) => n != 0,
        o => panic!("expected Int at out_reject, got {o:?}"),
    }
}

/// The shared buffer `verify_typed.kel` consumes for one chunk, seeded and ready
/// to drive.
///
/// `sig` is the chunk's entry in the module signature table, which seeds the
/// initial operand shapes; `wb`/`fb` are the module's word and float byte
/// widths. A chunk driven without its signature is a different, weaker check,
/// so the argument is required rather than defaulted.
///
/// One encoding, two callers: `typed_run` seeds through this rather than inline,
/// so an external harness feeds the stage exactly what the driver does.
pub fn seed_verify_typed_shared(
    vm: &Vm<'_, '_>,
    module: &Module,
    chunk: &crate::bytecode::Chunk,
    sig: Option<&crate::bytecode::ChunkSignature>,
    wb: usize,
    fb: usize,
) -> Vec<u8> {
    use crate::bytecode::Op;
    assert!(
        chunk.ops.len() <= 1536,
        "verify_typed.kel op-table capacity"
    );
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(vm, &mut shared, TV_OP_COUNT, chunk.ops.len() as i64);
    // Seed the local frame from the signature's parameters (leading slots) and the resume shape.
    if let Some(sig) = sig {
        for (i, param) in sig.params.iter().enumerate().take(256) {
            let (tag, size, kind) = abs_from_wire(param, wb, fb);
            set(vm, &mut shared, TV_SEED_TAG + i, tag);
            set(vm, &mut shared, TV_SEED_SIZE + i, size);
            set(vm, &mut shared, TV_SEED_KIND + i, kind);
        }
        let (rtag, rsize, rkind) = abs_from_wire(&sig.resume, wb, fb);
        set(vm, &mut shared, TV_RESUME_TAG, rtag);
        set(vm, &mut shared, TV_RESUME_SIZE, rsize);
        set(vm, &mut shared, TV_RESUME_KIND, rkind);
        // The declared flat enum body sizes (`word_bytes + min_payload`), for the B8 cross-check.
        for (i, el) in module.enum_layouts.iter().enumerate().take(64) {
            set(
                vm,
                &mut shared,
                TV_EB_VALS + i,
                wb as i64 + el.min_payload as i64,
            );
        }
        set(
            vm,
            &mut shared,
            TV_EB_COUNT,
            module.enum_layouts.len().min(64) as i64,
        );
    }
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg, is_term, tk, req, prod, ta, tb, tc) = typed_desc(op, chunk, wb, fb);
        assert!(prod <= 4, "verify_typed.kel push_tops unroll bound");
        set(vm, &mut shared, TV_CLASS + i, class);
        set(vm, &mut shared, TV_ARG + i, arg);
        set(vm, &mut shared, TV_IS_TERM + i, is_term);
        set(vm, &mut shared, TV_TK + i, tk);
        set(vm, &mut shared, TV_REQ + i, req);
        set(vm, &mut shared, TV_PROD + i, prod);
        set(vm, &mut shared, TV_TA + i, ta);
        set(vm, &mut shared, TV_TB + i, tb);
        set(vm, &mut shared, TV_TC + i, tc);
        // In the seeded form, a Call's return shape and the callee's parameter shapes (for the
        // argument check), or a native's return shape; isolation leaves them Top, matching
        // `typed_check_chunk`'s empty tables. An unmarshalled parameter slot (beyond the callee's
        // count, or for a native) stays Top and defers.
        if sig.is_some() {
            match op {
                Op::Call(callee, _) => {
                    if let Some(cs) = module.signatures.get(*callee as usize) {
                        let (tag, size, kind) = abs_from_wire(&cs.ret, wb, fb);
                        set(vm, &mut shared, TV_RET_TAG + i, tag);
                        set(vm, &mut shared, TV_RET_SIZE + i, size);
                        set(vm, &mut shared, TV_RET_KIND + i, kind);
                        for (p, param) in cs.params.iter().enumerate().take(TV_CP_STRIDE) {
                            let (ptag, psize, pkind) = abs_from_wire(param, wb, fb);
                            set(vm, &mut shared, TV_CP_TAG + i * TV_CP_STRIDE + p, ptag);
                            set(vm, &mut shared, TV_CP_SIZE + i * TV_CP_STRIDE + p, psize);
                            set(vm, &mut shared, TV_CP_KIND + i * TV_CP_STRIDE + p, pkind);
                        }
                    }
                }
                Op::CallVerifiedNative(idx, _) | Op::CallExternalNative(idx, _) => {
                    if let Some(w) = module.native_return_shapes.get(*idx as usize) {
                        let (tag, size, kind) = abs_from_wire(w, wb, fb);
                        set(vm, &mut shared, TV_RET_TAG + i, tag);
                        set(vm, &mut shared, TV_RET_SIZE + i, size);
                        set(vm, &mut shared, TV_RET_KIND + i, kind);
                    }
                }
                _ => {}
            }
        }
    }
    shared
}

/// Run verify_typed.kel over one chunk in isolation (no seeding), the drop-in for
/// `typed_check_chunk`.
pub fn typed_reject_chunk_via_kel(module: &Module, chunk: &crate::bytecode::Chunk) -> bool {
    let wb = (1usize << module.word_bits_log2) / 8;
    let fb = (1usize << module.float_bits_log2) / 8;
    typed_run(module, chunk, None, wb, fb)
}

/// Run verify_typed.kel over every chunk of a module, seeding each from the module's per-chunk
/// signature table (Phase 2b), the drop-in for `typed_check_module`. The module is rejected iff
/// any chunk is.
pub fn typed_reject_module_via_kel(module: &Module) -> bool {
    let wb = (1usize << module.word_bits_log2) / 8;
    let fb = (1usize << module.float_bits_log2) / 8;
    module
        .chunks
        .iter()
        .enumerate()
        .any(|(i, chunk)| typed_run(module, chunk, module.signatures.get(i), wb, fb))
}

// --- Self-hosted A.2.1 data-layout validation, slice 2c (verify_datalayout.kel) --------------
//
// verify_datalayout.kel reproduces the reference `validate_data_layout` (B6/C4): the shared-slot
// reconcile (contiguity/count), the shared-slot buffer bounds, and the private-composite
// monotonicity. It is BATCHED so it scales to the self-hosted stages, whose layouts expand to
// tens of thousands of slots (one per array element). The driver feeds each table 1024 entries at
// a time by resuming the coroutine, threading the running state through the shared block `dl`:
// scalars `phase` (0), `count` (1), `n_slots` (2), `buffer` (3), `pool` (4); the running state
// `io_prefix` (5) `io_total` `io_still` `io_prev_slot` `io_prev_off` `io_first` `io_bad` (11); and
// the batch arrays `a0` (12..), `a1`, `a2` (each 1024 wide), reused per phase. After the last
// batch the driver reads `io_bad`/`io_prefix`/`io_total` and finalises the contiguity/count
// comparison (`prefix == total`, `n_shared == total`).

const DL_PHASE: usize = 0;
const DL_COUNT: usize = 1;
const DL_N_SLOTS: usize = 2;
const DL_BUFFER: usize = 3;
const DL_POOL: usize = 4;
const DL_IO_PREFIX: usize = 5;
const DL_IO_TOTAL: usize = 6;
const DL_IO_STILL: usize = 7;
const DL_IO_PREV_SLOT: usize = 8;
const DL_IO_PREV_OFF: usize = 9;
const DL_IO_FIRST: usize = 10;
const DL_IO_BAD: usize = 11;
const DL_A0: usize = 12;
const DL_A1: usize = 12 + 1024;
const DL_A2: usize = 12 + 1024 * 2;

const DL_BATCH: usize = 1024;

fn verify_datalayout_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/verify_datalayout.kel")))
        .clone()
}

/// Run verify_datalayout.kel over a module's data layout, returning whether it rejects a B6
/// shared-slot reconcile/bounds or a C4 private-composite monotonicity violation (the reference
/// `validate_data_layout`). A module with no data layout is accepted. The three tables are fed one
/// 1024-entry batch at a time (resuming the coroutine, the running state persisting in the shared
/// buffer), so it scales to the self-hosted stages' tens-of-thousands-of-slot layouts.
pub fn dl_reject_module_via_kel(module: &Module) -> bool {
    use crate::bytecode::{SHARED_SLOT_COMPOSITE_FLAG, SlotVisibility};
    use crate::value_layout::ScalarKind;
    let Some(layout) = module.data_layout.as_ref() else {
        return false;
    };
    let n_slots = layout.slots.len();
    let n_shared = layout.shared_layout.len();
    let wb = (1usize << module.word_bits_log2) / 8;
    let fb = (1usize << module.float_bits_log2) / 8;
    let m = verify_datalayout_kel_module();
    let need = required_persistent_capacity_for(&m);
    // The working region must hold verify_datalayout.kel's per-batch frames while it walks a stage's
    // data layout; the self-hosted stages expand a large shared byte array to per-element slots (the
    // lexer `src.bytes` buffer alone is hundreds of thousands of entries), so give a generous margin
    // over the 64 KB default. This is a host-side test-harness arena, not a WCMU claim.
    let mut arena = Arena::with_capacity(4 * 1024 * 1024 + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_datalayout.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    // Running state and the per-run scalars, initialised once (they persist across resumes).
    set(&vm, &mut shared, DL_IO_PREFIX, 0);
    set(&vm, &mut shared, DL_IO_TOTAL, 0);
    set(&vm, &mut shared, DL_IO_STILL, 1);
    set(&vm, &mut shared, DL_IO_PREV_SLOT, 0);
    set(&vm, &mut shared, DL_IO_PREV_OFF, 0);
    set(&vm, &mut shared, DL_IO_FIRST, 1);
    set(&vm, &mut shared, DL_IO_BAD, 0);
    set(&vm, &mut shared, DL_N_SLOTS, n_slots as i64);
    set(&vm, &mut shared, DL_BUFFER, module.shared_data_bytes as i64);
    set(
        &vm,
        &mut shared,
        DL_POOL,
        module.persistent_composite_bytes as i64,
    );

    // Process one prepared batch (`phase`/`count`/arrays already set). Each batch is a fresh
    // `call_with_shared` -- it re-enters `loop main` from the entry, runs `run()` once, and yields
    // -- so the batch's `run()` always executes (a resume can land on the Stream/Reset cycle
    // boundary, which does not run the body). The running state `io_*` persists in the shared
    // buffer across calls because the buffer is retained, not re-zeroed.
    let run_batch = |vm: &mut Vm<'_, '_>, shared: &mut [u8]| match vm
        .call_with_shared(shared, &[Value::Int(0)])
        .expect("call verify_datalayout.kel")
    {
        VmState::Yielded(Value::Int(_)) => {}
        other => panic!("unexpected verify_datalayout.kel state: {other:?}"),
    };

    // Phase 1: the slot visibilities (shared prefix/total counts).
    for batch in layout.slots.chunks(DL_BATCH) {
        set(&vm, &mut shared, DL_PHASE, 1);
        set(&vm, &mut shared, DL_COUNT, batch.len() as i64);
        for (j, s) in batch.iter().enumerate() {
            let is_shared = i64::from(matches!(s.visibility, SlotVisibility::Shared));
            set(&vm, &mut shared, DL_A0 + j, is_shared);
        }
        run_batch(&mut vm, &mut shared);
    }
    // Phase 2: the shared-slot layout (offset, precomputed size, undecodable flag).
    for batch in layout.shared_layout.chunks(DL_BATCH) {
        set(&vm, &mut shared, DL_PHASE, 2);
        set(&vm, &mut shared, DL_COUNT, batch.len() as i64);
        for (j, sl) in batch.iter().enumerate() {
            set(&vm, &mut shared, DL_A0 + j, sl.offset as i64);
            let (size, bad) = if sl.kind & SHARED_SLOT_COMPOSITE_FLAG != 0 {
                (sl.len as i64, 0)
            } else {
                match ScalarKind::from_tag(sl.kind) {
                    Some(k) => (k.size_in_bytes(wb, fb) as i64, 0),
                    None => (0, 1),
                }
            };
            set(&vm, &mut shared, DL_A1 + j, size);
            set(&vm, &mut shared, DL_A2 + j, bad);
        }
        run_batch(&mut vm, &mut shared);
    }
    // Phase 3: the private-composite layout (slot, pool offset).
    for batch in layout.private_composite_layout.chunks(DL_BATCH) {
        set(&vm, &mut shared, DL_PHASE, 3);
        set(&vm, &mut shared, DL_COUNT, batch.len() as i64);
        for (j, e) in batch.iter().enumerate() {
            set(&vm, &mut shared, DL_A0 + j, e.slot as i64);
            set(&vm, &mut shared, DL_A1 + j, e.offset as i64);
        }
        run_batch(&mut vm, &mut shared);
    }

    let rd = |slot: usize| -> i64 {
        match vm.get_shared(&shared, slot).unwrap() {
            Value::Int(n) => n,
            o => panic!("expected Int at {slot}, got {o:?}"),
        }
    };
    // Reject on any per-entry violation, or the B6 contiguity/count mismatch. When there were no
    // batches at all (an empty layout), the running state is its init value and this is `false`.
    rd(DL_IO_BAD) != 0
        || rd(DL_IO_PREFIX) != rd(DL_IO_TOTAL)
        || (n_shared as i64) != rd(DL_IO_TOTAL)
}

/// The self-hosted drop-in replacement for `verify_resource_bounds`: analyze.kel decides each
/// chunk's WCMU transitively (callee bodies folded at every `Op::Call`, resolved in
/// topological order so callees precede callers), and the module is admitted iff no chunk has
/// an inextractable bound and every Stream chunk's budget fits `arena_capacity`.
pub fn validate_module_via_kel(module: &Module, arena_capacity: i64) -> bool {
    use crate::bytecode::{BlockType, Op};
    let n = module.chunks.len();
    // Topological order over the call graph (callees before callers), rejecting recursion --
    // the same DFS postorder `topological_call_order` computes.
    let mut visited = vec![0u8; n]; // 0 unseen, 1 on-stack, 2 done
    let mut order = Vec::new();
    fn visit(module: &Module, i: usize, visited: &mut [u8], order: &mut Vec<usize>) -> bool {
        if visited[i] == 1 {
            return false; // cycle
        }
        if visited[i] == 2 {
            return true;
        }
        visited[i] = 1;
        for op in &module.chunks[i].ops {
            if let Op::Call(callee, _) = op {
                let c = *callee as usize;
                if c < module.chunks.len() && !visit(module, c, visited, order) {
                    return false;
                }
            }
        }
        visited[i] = 2;
        order.push(i);
        true
    }
    for i in 0..n {
        if visited[i] != 2 && !visit(module, i, &mut visited, &mut order) {
            return false; // a recursive call graph is inadmissible
        }
    }
    // Resolve each chunk's transitive WCMU in topological order, then admit.
    let shared_layout = module
        .data_layout
        .as_ref()
        .map_or(&[][..], |dl| &dl.shared_layout);
    let mut chunk_wcmu = vec![(0i64, 0i64); n];
    let mut valid = true;
    for &idx in &order {
        let chunk = &module.chunks[idx];
        let (_wcet, stack, heap, reject, chunk_valid) =
            run_analyze_kel(chunk, arena_capacity, &chunk_wcmu, shared_layout);
        if reject {
            valid = false; // an inextractable bound anywhere fails module_wcmu
        }
        if chunk.block_type == BlockType::Stream && !chunk_valid {
            valid = false; // a Stream chunk whose transitive budget exceeds the capacity
        }
        chunk_wcmu[idx] = (stack, heap);
    }
    valid
}

// -- scaffold assembly from parse.kel's records (ported from tests/selfhost_codegen.rs) --
//
// The data layout, enum-layout table, chunk signatures, schema hash, and declared
// WCET/WCMU header are assembled from the pipeline output (parse.kel's record stream and
// analyze.kel's per-chunk verdict), rather than borrowed from the Rust reference. Each
// assembly mirrors the corresponding Rust compiler pass, so the serialized module is
// byte-identical to the reference for the loop-free stage sources.

/// Assemble the data-slot table from parse.kel's data-block record stream, mapping the
/// interned block, field, and (unused here) type ids through the name table.
fn assemble_data_slots(
    data_records: &[(i64, i64)],
    names: &[String],
) -> Vec<crate::bytecode::DataSlot> {
    use crate::bytecode::{DataSlot, SlotVisibility};
    struct Blk {
        name_id: i64,
        vis: i64,
        fields: Vec<(i64, i64)>, // (field name id, element count)
    }
    let mut blocks: Vec<Blk> = Vec::new();
    for &(code, val) in data_records {
        match code {
            9 => blocks.push(Blk {
                name_id: val / 4,
                vis: val % 4,
                fields: Vec::new(),
            }),
            4 => blocks.last_mut().unwrap().fields.push((val, 1)),
            8 => blocks.last_mut().unwrap().fields.last_mut().unwrap().1 = val,
            // 6 (PTYPE) is not needed for the slot names; 5 (END) is a boundary.
            _ => {}
        }
    }
    let mut slots = Vec::new();
    // Pass 0 shared, pass 1 private; visibility 2 (const) yields no runtime slots.
    for pass_vis in [0i64, 1i64] {
        let visibility = if pass_vis == 0 {
            SlotVisibility::Shared
        } else {
            SlotVisibility::Private
        };
        for b in blocks.iter().filter(|b| b.vis == pass_vis) {
            let bname = &names[b.name_id as usize];
            for &(fid, count) in &b.fields {
                let fname = &names[fid as usize];
                if count == 1 {
                    slots.push(DataSlot {
                        name: format!("{bname}.{fname}"),
                        visibility,
                    });
                } else {
                    // ONE NAME FOR THE WHOLE ARRAY, mirroring `compiler.rs`.
                    //
                    // This driver builds the data layout from the self-hosted
                    // parser's data-block records, and its output is compared
                    // against the reference compiler's byte for byte -- including
                    // `schema_hash`, which covers the layout. A distinct
                    // `field[k]` per element defeated the name interner's dedup
                    // and made the string pool scale with the element count; the
                    // reference stopped doing it, so this must too or the two
                    // compilers disagree.
                    let array_name = format!("{bname}.{fname}");
                    for _ in 0..count {
                        slots.push(DataSlot {
                            name: array_name.clone(),
                            visibility,
                        });
                    }
                }
            }
        }
    }
    slots
}

/// Assemble the per-shared-slot byte layout (offset, kind tag, len) from parse.kel's
/// data-block records. The shared segment is the single shared block's fields expanded
/// to one entry per element at consecutive byte offsets; the stage data fields are all
/// `Word` (ScalarKind::Int, tag 3, eight bytes at the 64-bit reference width) or `Byte`
/// (tag 2, one byte), and each is a scalar (len 0).
fn assemble_shared_layout(
    data_records: &[(i64, i64)],
    names: &[String],
) -> Vec<crate::bytecode::SharedSlotLayout> {
    use crate::bytecode::SharedSlotLayout;
    struct Blk {
        vis: i64,
        fields: Vec<(i64, i64)>, // (type name id, element count)
    }
    let mut blocks: Vec<Blk> = Vec::new();
    for &(code, val) in data_records {
        match code {
            9 => blocks.push(Blk {
                vis: val % 4,
                fields: Vec::new(),
            }),
            4 => blocks.last_mut().unwrap().fields.push((0, 1)),
            6 => blocks.last_mut().unwrap().fields.last_mut().unwrap().0 = val,
            8 => blocks.last_mut().unwrap().fields.last_mut().unwrap().1 = val,
            _ => {}
        }
    }
    let scalar = |type_id: i64| -> (u8, u32) {
        match names[type_id as usize].as_str() {
            "Word" => (3, 8),
            "Byte" => (2, 1),
            other => panic!("unhandled shared field type `{other}`"),
        }
    };
    let mut layout = Vec::new();
    let mut offset: u32 = 0;
    for b in blocks.iter().filter(|b| b.vis == 0) {
        for &(tid, count) in &b.fields {
            let (tag, size) = scalar(tid);
            for _ in 0..count {
                layout.push(SharedSlotLayout {
                    offset,
                    kind: tag,
                    len: 0,
                });
                offset += size;
            }
        }
    }
    layout
}

/// Assemble the per-private-slot load-time initial values from parse.kel's records:
/// one entry per private slot (arrays expanded), the type's zero -- `Int(0)` for a
/// `Word` slot, `Byte(0)` for a `Byte` slot. The stage private fields carry no
/// `= literal` initializer, so every entry is a zero.
fn assemble_private_init(
    data_records: &[(i64, i64)],
    names: &[String],
) -> Vec<crate::bytecode::ConstValue> {
    use crate::bytecode::ConstValue;
    struct Blk {
        vis: i64,
        fields: Vec<(i64, i64)>, // (type name id, element count)
    }
    let mut blocks: Vec<Blk> = Vec::new();
    for &(code, val) in data_records {
        match code {
            9 => blocks.push(Blk {
                vis: val % 4,
                fields: Vec::new(),
            }),
            4 => blocks.last_mut().unwrap().fields.push((0, 1)),
            6 => blocks.last_mut().unwrap().fields.last_mut().unwrap().0 = val,
            8 => blocks.last_mut().unwrap().fields.last_mut().unwrap().1 = val,
            _ => {}
        }
    }
    let mut init = Vec::new();
    for b in blocks.iter().filter(|b| b.vis == 1) {
        for &(tid, count) in &b.fields {
            let zero = match names[tid as usize].as_str() {
                "Word" => ConstValue::Int(0),
                "Byte" => ConstValue::Byte(0),
                other => panic!("unhandled private field type `{other}`"),
            };
            for _ in 0..count {
                init.push(zero.clone());
            }
        }
    }
    init
}

/// Assemble a whole `DataLayout` from parse.kel's data-block records. The stages have
/// no private composite fields, so `private_composite_layout` is empty.
fn assemble_data_layout(
    data_records: &[(i64, i64)],
    names: &[String],
) -> crate::bytecode::DataLayout {
    crate::bytecode::DataLayout {
        slots: assemble_data_slots(data_records, names),
        shared_layout: assemble_shared_layout(data_records, names),
        private_composite_layout: Vec::new(),
        private_init: assemble_private_init(data_records, names),
    }
}

/// Assemble the enum-layout table from parse.kel's enum record stream.
fn assemble_enum_layouts(
    enum_records: &[(i64, i64)],
    names: &[String],
) -> Vec<crate::bytecode::EnumLayout> {
    use crate::bytecode::{EnumLayout, EnumVariantDisc};
    let mut layouts: Vec<EnumLayout> = Vec::new();
    let mut running = 0i64;
    for &(code, val) in enum_records {
        match code {
            12 => {
                layouts.push(EnumLayout {
                    type_name: names[val as usize].clone(),
                    variants: Vec::new(),
                    min_payload: 0,
                });
                running = 0;
            }
            13 => {
                layouts.last_mut().unwrap().variants.push(EnumVariantDisc {
                    name: names[val as usize].clone(),
                    disc: running,
                });
                running += 1;
            }
            14 => {
                let vs = &mut layouts.last_mut().unwrap().variants;
                vs.last_mut().unwrap().disc = val;
                running = val + 1;
            }
            _ => {}
        }
    }
    // The reference orders the enum-layout table by type name, not declaration order.
    layouts.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    layouts
}

/// The flat shape of a stage boundary type: `Word` -> Int scalar (tag 3), `Byte` ->
/// Byte scalar (tag 2), anything else the conservative `Top` the reference records for
/// an unresolvable type.
fn wire_shape_of(type_id: i64, names: &[String]) -> crate::bytecode::WireShape {
    use crate::bytecode::WireShape;
    match names.get(type_id as usize).map(String::as_str) {
        Some("Word") => WireShape::Scalar { kind: 3 },
        Some("Byte") => WireShape::Scalar { kind: 2 },
        _ => WireShape::Top,
    }
}

/// Assemble the per-chunk signature table from the parsed functions, grouping
/// same-named heads into one chunk and ordering by chunk name to match the module.
fn assemble_signatures(fns: &[ParsedFn], names: &[String]) -> Vec<crate::bytecode::ChunkSignature> {
    use crate::bytecode::{ChunkSignature, WireShape};
    let mut chunks: Vec<(String, ChunkSignature)> = Vec::new();
    let mut i = 0;
    while i < fns.len() {
        let name = names[fns[i].name as usize].clone();
        let first = &fns[i];
        // Skip the rest of this head group.
        let mut j = i + 1;
        while j < fns.len() && names[fns[j].name as usize] == name {
            j += 1;
        }
        i = j;
        let params: Vec<WireShape> = first
            .param_types
            .iter()
            .map(|&t| wire_shape_of(t, names))
            .collect();
        let ret = wire_shape_of(first.return_type, names);
        // Only a `loop` (category 3, a Stream chunk) resumes with its first parameter.
        let resume = if first.cat == 3 {
            params.first().copied().unwrap_or(WireShape::Top)
        } else {
            WireShape::Top
        };
        chunks.push((
            name,
            ChunkSignature {
                params,
                ret,
                resume,
            },
        ));
    }
    chunks.sort_by(|a, b| a.0.cmp(&b.0));
    chunks.into_iter().map(|(_, s)| s).collect()
}

/// Set a module's declared WCET/WCMU header from the self-hosted analyze.kel stage: the
/// per-iteration maximum across the module's Stream chunks, mirroring the reference
/// compiler's fold (`compiler.rs` sets `wcet_cycles`/`wcmu_bytes` to that maximum).
fn assemble_resource_bounds(module: &mut Module) {
    let mut max_wcet = 0i64;
    let mut max_wcmu = 0i64;
    for c in &module.chunks {
        if c.block_type != crate::bytecode::BlockType::Stream {
            continue;
        }
        // The self-hosted analyze.kel driver already ported into this file: the shallow
        // empty-resolver form matches `wcet_stream_iteration`/`wcmu_stream_iteration`.
        let (wcet, stack, heap, reject, _valid) = run_analyze_kel(c, i64::MAX, &[], &[]);
        assert!(!reject, "analyze.kel rejected a stage Stream chunk");
        max_wcet = max_wcet.max(wcet);
        max_wcmu = max_wcmu.max(stack + heap);
    }
    module.wcet_cycles = max_wcet as u32;
    module.wcmu_bytes = max_wcmu as u32;
}

/// Each chunk's `(name, param_count, block_type, param_types)` assembled from the parsed
/// functions, in chunk-name order. The block type comes from the declaration category (fn ->
/// Func, yield -> Reentrant, loop -> Stream); the parameter type tags map `Word`/`Byte` to
/// their [`crate::bytecode::TypeTag`] (a stage boundary carries only scalar parameters). A
/// multiheaded function is one chunk described by its first head.
#[allow(clippy::type_complexity)]
fn assemble_chunk_metadata(
    fns: &[ParsedFn],
    names: &[String],
) -> Vec<(
    String,
    u8,
    crate::bytecode::BlockType,
    Vec<crate::bytecode::TypeTag>,
)> {
    use crate::bytecode::{BlockType, TypeTag};
    let tag_of = |type_id: i64| -> TypeTag {
        match names.get(type_id as usize).map(String::as_str) {
            Some("Word") => TypeTag::Word,
            Some("Byte") => TypeTag::Byte,
            _ => TypeTag::Composite,
        }
    };
    let mut chunks = Vec::new();
    let mut i = 0;
    while i < fns.len() {
        let name = names[fns[i].name as usize].clone();
        let first = &fns[i];
        let mut j = i + 1;
        while j < fns.len() && names[fns[j].name as usize] == name {
            j += 1;
        }
        i = j;
        let block_type = match first.cat {
            1 => BlockType::Func,
            2 => BlockType::Reentrant,
            _ => BlockType::Stream,
        };
        let param_types: Vec<TypeTag> = first.param_types.iter().map(|&t| tag_of(t)).collect();
        chunks.push((name, first.params as u8, block_type, param_types));
    }
    chunks.sort_by(|a, b| a.0.cmp(&b.0));
    chunks
}

/// Self-host-compile a whole program with a from-scratch module scaffold: the
/// self-hosted chunk ops (via [`crate::selfhost::self_host_compile`]) plus a data layout, schema hash,
/// enum-layout table, chunk signatures, per-chunk metadata, and declared WCET/WCMU header all
/// assembled from the pipeline output (parse.kel's record stream and analyze.kel's verdict)
/// rather than borrowed from the Rust reference. For the loop-free stage sources the
/// serialized module is byte-identical to the reference; the reference is used only as the
/// oracle in `tests/scaffold.rs`.
///
/// What still rides the reference base: the chunk table's names and order, the (absent, for
/// the stages) native chunks, and the module bookkeeping metadata (`aux_arena_bytes`,
/// `persist_composite_bytes`, `flags`, and the target bit-widths). The last three are
/// program-analysis-derived (opaque-intern reachability, private-composite persistence, entry
/// modifiers) and self-hosting them is a distinct increment.
pub fn self_host_compile_full(src: &str) -> Module {
    let mut module = self_host_compile(src);
    let (fns, names, data_records, enum_records) = parse_functions_fused(src);
    let dl = assemble_data_layout(&data_records, &names);
    module.schema_hash = crate::bytecode::compute_schema_hash(Some(&dl));
    module.data_layout = Some(dl);
    module.enum_layouts = assemble_enum_layouts(&enum_records, &names);
    module.signatures = assemble_signatures(&fns, &names);
    // Self-assemble each source chunk's param_count, block_type, and param_types (the last
    // per-chunk scaffold field). A native chunk (absent for the stages) is not in `meta` and
    // keeps the reference's metadata.
    let meta = assemble_chunk_metadata(&fns, &names);
    for chunk in &mut module.chunks {
        if let Some((_, pc, bt, pts)) = meta.iter().find(|(n, _, _, _)| n == &chunk.name) {
            chunk.param_count = *pc;
            chunk.block_type = *bt;
            chunk.param_types = pts.clone();
        }
    }
    assemble_resource_bounds(&mut module);
    self_host_module_bookkeeping(&mut module);
    module
}

/// Compute the module bookkeeping fields the reference derives by program analysis --
/// `aux_arena_bytes`, `persistent_composite_bytes`, and `flags` -- from the pipeline output,
/// rather than inheriting them from the reference base.
///
/// For the self-hosting subset (scalar `Word`/`Byte` data, no `Text`, no opaque types, no
/// `signed` entry) these reduce to closed forms:
///
/// - **`aux_arena_bytes` is 0.** The field reserves an opaque-registry arena slice sized by
///   `may_intern_opaque`, which is true only when the program constructs a flat composite able
///   to hold a host opaque. A scalar-only program never does, so the field is provably 0. The
///   general opaque-reachability analysis is a future extension gated on opaque-type support.
/// - **`persistent_composite_bytes` is 0.** The field is the summed flat-body size of private
///   data slots that hold a composite; the subset's private data is scalar or array-of-scalar,
///   so the assembled private-composite layout is empty. The general body-size accounting is a
///   future extension gated on composite-in-`data` support (a `debug_assert` guards the
///   assumption).
/// - **`FLAG_EPHEMERAL` iff `private_data_bytes == 0`.** The reference sets it when the module
///   has no private data and no arena-resident (`Text`) value crosses the host boundary; the
///   subset has no `Text`, so the text condition is vacuously satisfied and the flag reduces to
///   the private-data test. `FLAG_REQUIRES_SIGNATURE` is never set (the subset has no `signed`
///   entry).
///
/// The byte-identity oracle (`tests/scaffold.rs`) confirms these match the reference for the
/// five stages (all zero) and a private-data-free program (which sets `FLAG_EPHEMERAL`).
fn self_host_module_bookkeeping(module: &mut Module) {
    module.aux_arena_bytes = 0;
    debug_assert!(
        module
            .data_layout
            .as_ref()
            .is_none_or(|dl| dl.private_composite_layout.is_empty()),
        "self-hosted persistent_composite_bytes = 0 assumes no private composite data (subset)"
    );
    module.persistent_composite_bytes = 0;
    module.flags = if module.private_data_bytes == 0 {
        crate::bytecode::FLAG_EPHEMERAL
    } else {
        0
    };
}

/// The shared segment's flat byte total, derived from the assembled shared-slot layout the
/// same way the reference accumulates it (`compile_with_target`'s `shared_data_flat_bytes`):
/// one entry per shared slot at consecutive byte offsets, so the total is the byte past the
/// last entry. The per-slot size is fixed by the scalar kind tag the layout records: a `Word`
/// (tag 3) is eight bytes and a `Byte` (tag 2) is one byte at the 64-bit reference width, the
/// same `(tag, size)` mapping `assemble_shared_layout` uses. Zero for a module with no shared
/// slots.
fn shared_data_bytes_of(shared_layout: &[crate::bytecode::SharedSlotLayout]) -> u32 {
    shared_layout
        .iter()
        .map(|e| {
            let size = match e.kind {
                3 => 8u32,
                2 => 1u32,
                other => panic!("unhandled shared slot kind tag {other}"),
            };
            e.offset + size
        })
        .max()
        .unwrap_or(0)
}

/// Self-host-compile a whole program building the emitted [`Module`] entirely from the
/// pipeline output, with no reference-compiler borrow of the user program. Unlike
/// [`self_host_compile_full`] (which starts from `compile_src(src)` and overwrites fields),
/// every one of the module's eighteen fields is assembled here from parse.kel's record
/// stream, reconstruct.kel's forest, codegen.kel's ops, and analyze.kel's verdict; the
/// reference is used only as the byte-identity oracle in `tests/scaffold.rs`.
///
/// The chunk order matches the reference's name-keyed `BTreeMap` order (chunks sorted
/// lexicographically by name, same-named heads folded into one chunk). The entry point is the
/// name-sorted index of `main`. The scalar bit widths are the host target's. The
/// shared/private data byte totals are derived from the assembled `DataLayout` exactly as the
/// reference derives them (the shared segment's flat byte span, and the private-slot count
/// times `VALUE_SLOT_SIZE_BYTES`). The stages declare no natives, so `native_names` and
/// `native_return_shapes` are empty.
pub fn self_host_compile_scratch(src: &str) -> Module {
    use crate::bytecode::{Chunk, SlotVisibility};
    let (fns, names, data_records, enum_records) = parse_functions_fused(src);

    // Build each source chunk from the pipeline output. Group consecutive same-named heads
    // (a multiheaded function is one chunk), mirroring `self_host_compile`, but emit a fresh
    // `Chunk` rather than splicing into a reference base.
    let meta = assemble_chunk_metadata(&fns, &names);
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut i = 0;
    while i < fns.len() {
        let name = names[fns[i].name as usize].clone();
        let mut group: Vec<&ParsedFn> = vec![&fns[i]];
        let mut j = i + 1;
        while j < fns.len() && names[fns[j].name as usize] == name {
            group.push(&fns[j]);
            j += 1;
        }
        i = j;
        let pc = group[0].params;
        let body = if is_multihead_group(&group) {
            reconstruct_via_kel_multihead(&group, pc)
        } else {
            let category = if group[0].cat == 3 { 2 } else { 0 };
            reconstruct_via_kel(&group[0].body, category, pc)
        };
        let (ops, pool, lc) = run_codegen(&body, pc);
        // The metadata table (already name-sorted) supplies param_count, block_type, and
        // param_types; look this chunk's entry up by name.
        let (_, param_count, block_type, param_types) = meta
            .iter()
            .find(|(n, _, _, _)| n == &name)
            .unwrap_or_else(|| panic!("no metadata for chunk `{name}`"));
        chunks.push(Chunk {
            name,
            ops,
            constants: pool_to_constants(&pool, &names),
            struct_templates: Vec::new(),
            local_count: lc as u16,
            param_count: *param_count,
            block_type: *block_type,
            param_types: param_types.clone(),
            debug_pool: None,
        });
    }
    // Order chunks by name to match the reference's name-keyed chunk order. The stage
    // sources have no native chunks, so every chunk is source-defined.
    chunks.sort_by(|a, b| a.name.cmp(&b.name));

    // The entry point is the name-sorted index of the module's `main`.
    let entry_point = chunks.iter().position(|c| c.name == "main");

    // The native names come from the `use` declarations; the stages declare none, so this is
    // empty and `native_return_shapes` is correspondingly empty.
    let native_names: Vec<String> = Vec::new();
    let native_return_shapes: Vec<crate::bytecode::WireShape> = Vec::new();

    // The data layout, assembled from parse.kel's data-block records, drives both the shared
    // and private byte totals the reference derives from the same layout.
    let dl = assemble_data_layout(&data_records, &names);
    let shared_data_bytes = shared_data_bytes_of(&dl.shared_layout);
    let private_slot_count = dl
        .slots
        .iter()
        .filter(|s| s.visibility == SlotVisibility::Private)
        .count() as u32;
    let private_data_bytes =
        private_slot_count.saturating_mul(crate::bytecode::VALUE_SLOT_SIZE_BYTES);
    let schema_hash = crate::bytecode::compute_schema_hash(Some(&dl));

    let enum_layouts = assemble_enum_layouts(&enum_records, &names);
    let signatures = assemble_signatures(&fns, &names);

    // The scalar bit widths are the host target's, matching the reference `compile`, which
    // compiles with `Target::host()`.
    let target = crate::target::Target::host();

    let mut module = Module {
        chunks,
        native_names,
        entry_point,
        data_layout: Some(dl),
        word_bits_log2: target.word_bits_log2,
        addr_bits_log2: target.addr_bits_log2,
        float_bits_log2: target.float_bits_log2,
        // Assembled from analyze.kel below.
        wcet_cycles: 0,
        wcmu_bytes: 0,
        // Assembled from the program-analysis bookkeeping below.
        aux_arena_bytes: 0,
        persistent_composite_bytes: 0,
        flags: 0,
        shared_data_bytes,
        private_data_bytes,
        schema_hash,
        enum_layouts,
        signatures,
        native_return_shapes,
    };
    assemble_resource_bounds(&mut module);
    self_host_module_bookkeeping(&mut module);
    module
}

/// Why a self-hosted compile could not proceed. The self-hosted pipeline is not a
/// general-purpose compiler: it accepts the self-hosted language subset at the host
/// target width. `keleusma-cli`'s `--compiler self-hosted` maps these to a clear error
/// suggesting `--compiler rust`.
#[derive(Debug, Clone)]
pub enum SelfHostError {
    /// The requested target differs from the host width. The self-hosted pipeline is only
    /// validated byte-identical to the reference at host word/address/float widths, so a
    /// cross-width compile is refused rather than emitting an unverified module.
    NonHostTarget,
    /// The program does not compile under the *reference* compiler either: a genuine source
    /// error (a lex, parse, or type error), not a self-hosted-subset limitation. Retrying
    /// with `--compiler rust` reports the identical error, so the CLI does NOT suggest it.
    ReferenceRejected {
        /// The reference compiler's own diagnostic message.
        detail: String,
    },
    /// The program is within the reference language but outside the self-hosted subset (for
    /// example floats, generics, `Text`, or native calls). This surfaces either as a
    /// recovered pipeline panic or as a divergence from the reference output; `detail` names
    /// the diverging chunk and the first differing op where a divergence is the cause.
    /// Retrying with `--compiler rust` compiles the program, so the CLI suggests it.
    Unsupported {
        /// A best-effort description: for a divergence, the diverging chunk and the first
        /// differing op or dimension; for a stage panic, the recovered panic payload.
        detail: String,
    },
}

impl SelfHostError {
    /// Whether retrying the compile with the Rust reference backend (`--compiler rust`)
    /// would help. True for a self-hosted-subset limitation ([`Unsupported`]) and for a
    /// cross-width target ([`NonHostTarget`], which the reference compiler supports). False
    /// for a genuine source error ([`ReferenceRejected`]), where the reference reports the
    /// identical error and the hint would only mislead.
    ///
    /// [`Unsupported`]: SelfHostError::Unsupported
    /// [`NonHostTarget`]: SelfHostError::NonHostTarget
    /// [`ReferenceRejected`]: SelfHostError::ReferenceRejected
    pub fn rust_backend_would_help(&self) -> bool {
        !matches!(self, SelfHostError::ReferenceRejected { .. })
    }
}

impl core::fmt::Display for SelfHostError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SelfHostError::NonHostTarget => write!(
                f,
                "the self-hosted compiler supports only the host target width"
            ),
            SelfHostError::ReferenceRejected { detail } => {
                write!(f, "the program does not compile ({detail})")
            }
            SelfHostError::Unsupported { detail } => write!(
                f,
                "the self-hosted compiler does not support this program ({detail})"
            ),
        }
    }
}

impl std::error::Error for SelfHostError {}

/// The shared-slot layout `wire.kel` declares, derived once from its `shared data
/// wire` block.
///
/// # Why this is a module and not three sets of local constants
///
/// It was three. `wire_names_from_input`, `wire_regions_from_input` and
/// `wire_chunks_from_input` each restated `1 + 65536 + 1 + 1024 * 4`, and they
/// agreed -- which is the state a drifted copy starts in. This tree has already
/// paid for the same shape twice: nine copies of `parse.kel`'s layout collapsed
/// into `crate::selfhost_host`, and four silent miscompiles in one day from the
/// shipping driver disagreeing with a copy of itself that the boundary measured
/// instead.
///
/// # The derivation, field by field
///
/// A slot index is a WORD index for a `Word` field and a BYTE index inside a
/// `[Byte; N]` field, because `set_shared` addresses the declared slots. Reading
/// down the block:
///
/// | field | width | first slot |
/// |---|---|---|
/// | `len` | 1 | 0 |
/// | `bytes` | 65,536 | 1 |
/// | `nregions` | 1 | 65,537 |
/// | `rkind`, `rflags`, `rlen`, `rcovers` | 1,024 each | 65,538 |
/// | `warg` .. `warg5` | 5 | 69,634 |
/// | `fin` | 1,024 | 69,639 |
/// | `bin` | 49,152 | 70,663 |
///
/// **APPEND TO A SLOT-ADDRESSED BLOCK, NEVER INSERT.** Every constant here is an
/// offset from the fields above it, so a field inserted rather than appended
/// moves every slot after it and every one of these becomes wrong at once.
pub mod wire_slots {
    /// `wire.len`, the directory byte length.
    pub const LEN: usize = 0;
    /// First byte of `wire.bytes`, the artifact window.
    pub const BYTES: usize = LEN + 1;
    /// `wire.nregions`.
    pub const NREGIONS: usize = BYTES + 65536;
    /// First slot of the four per-region input arrays.
    pub const REGION_INPUTS: usize = NREGIONS + 1;
    /// `wire.warg`, first of the five general argument slots.
    pub const WARG: usize = REGION_INPUTS + 1024 * 4;
    /// First slot of `wire.fin`, the record-field batch buffer.
    pub const FIN: usize = WARG + 5;
    /// First slot of `wire.bin`, the byte-pool batch buffer.
    pub const BIN: usize = FIN + 1024;
}

/// Emit a module's `NAMES` and `STRING_POOL` regions through `wire.kel`.
///
/// **This is the first thing the DRIVER asks `wire.kel` to do**, and it is what
/// makes `wire.kel`'s place in the stage table honest. Until now the wire format
/// was exercised only from the test harness; the driver ran pipeline stages and
/// produced no auxiliary body at all.
///
/// The module reaches Keleusma as a length-prefixed blob and nothing else: no
/// Every constant root in a module, in chunk order.
fn const_roots_of(module: &Module) -> Vec<ConstValue> {
    let mut roots = Vec::new();
    for c in &module.chunks {
        roots.extend(c.constants.iter().cloned());
    }
    roots
}

/// The wire tag and payload for a constant node.
///
/// Composite tags are RETURNED rather than rejected, so a refusal comes from the
/// stage. A host that quietly dropped them would make the stage's guard
/// untestable, which is the same reason the blob carries a zero enum count
/// rather than omitting the section.
/// A constant's child nodes, in the order the wire format numbers them.
///
/// Extracted from `push_blob_node`, which is no longer its only caller: the
/// `CONSTS` streaming emitter walks the same structure into a different encoding.
/// Two walks of one shape is how a composite comes to be counted one way and
/// emitted another.
fn const_children(c: &ConstValue) -> Vec<&ConstValue> {
    use ConstValue as K;
    match c {
        K::Tuple(v) | K::Array(v) => v.iter().collect(),
        K::Struct { fields, .. } => fields.iter().map(|(_, v)| v).collect(),
        K::Enum { fields, .. } => fields.iter().collect(),
        _ => Vec::new(),
    }
}

/// A constant's `(flags, discriminant)` pair.
///
/// Only a resolved enum discriminant sets a flag, and the bit is
/// [`crate::wire_schema::FLAG_HAS_DISCRIMINANT`] rather than a literal `1`, for
/// the same reason the tags below are named: the flag layout is the wire
/// contract, not a number that happens to match it.
fn const_flags_and_discriminant(c: &ConstValue) -> (i64, i64) {
    use ConstValue as K;
    match c {
        K::Enum {
            discriminant: Some(d),
            ..
        } => (i64::from(crate::wire_schema::FLAG_HAS_DISCRIMINANT), *d),
        _ => (0, 0),
    }
}

fn const_tag_and_name(c: &ConstValue) -> (u16, i64) {
    use crate::wire_schema::tag;
    use ConstValue as K;
    // THE TAGS ARE THE ENCODER'S OWN CONSTANTS, NOT NUMBERS THAT MATCH THEM.
    // This arm list carried the literals 1..12 and they agreed with
    // `wire_schema::tag` by coincidence rather than by construction: the tag
    // numbering is the wire contract, so a renumbering there would leave this
    // function silently emitting the old contract. That is the shipping-driver
    // versus test-copy shape that produced four silent miscompiles on
    // 2026-08-21, one layer down.
    match c {
        K::Unit => (tag::UNIT, 0),
        K::Bool(b) => (tag::BOOL, i64::from(*b)),
        K::Int(v) => (tag::INT, *v),
        K::Byte(v) => (tag::BYTE, i64::from(*v)),
        K::Fixed(v) => (tag::FIXED, *v),
        K::None => (tag::NONE, 0),
        K::StaticStr(_) => (tag::STATIC_STR, 0),
        K::Tuple(_) => (tag::TUPLE, 0),
        K::Array(_) => (tag::ARRAY, 0),
        K::Struct { .. } => (tag::STRUCT, 0),
        K::Enum { .. } => (tag::ENUM, 0),
        other => panic!("const_tag_and_name has no tag for {other:?}"),
    }
}

/// Total nodes in a constant forest, counting every descendant.
fn count_blob_nodes(roots: &[ConstValue]) -> usize {
    use ConstValue as K;
    fn go(c: &K) -> usize {
        1 + match c {
            K::Tuple(v) | K::Array(v) => v.iter().map(go).sum::<usize>(),
            K::Struct { fields, .. } => fields.iter().map(|(_, v)| go(v)).sum::<usize>(),
            K::Enum { fields, .. } => fields.iter().map(go).sum::<usize>(),
            _ => 0,
        }
    }
    roots.iter().map(go).sum()
}

/// The names a constant node contributes, in the order the reference interns
/// them.
fn blob_node_names(c: &ConstValue) -> Vec<&str> {
    use ConstValue as K;
    match c {
        K::Struct { type_name, fields } => {
            let mut n = vec![type_name.as_str()];
            n.extend(fields.iter().map(|(f, _)| f.as_str()));
            n
        }
        K::Enum {
            type_name, variant, ..
        } => vec![type_name.as_str(), variant.as_str()],
        K::StaticStr(s) => vec![s.as_str()],
        _ => Vec::new(),
    }
}

/// One constant node in PREORDER: tag, payload, child count, flags,
/// discriminant, its interned names, then its children.
///
/// The order is the reference's rather than a fresh choice: the reference
/// flattener pushes a node and then descends, so both the node table and the
/// name sequence are depth-first preorder. Writing the blob in that order is
/// what lets the stage reproduce both with a linear scan.
fn push_blob_node(c: &ConstValue, out: &mut Vec<u8>, names: &mut usize) {
    let node_names = blob_node_names(c);
    let children = const_children(c);
    let (tag, payload) = const_tag_and_name(c);
    let (flags, disc) = const_flags_and_discriminant(c);
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&payload.to_le_bytes());
    out.extend_from_slice(
        &u16::try_from(children.len())
            .expect("kids fit u16")
            .to_le_bytes(),
    );
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&disc.to_le_bytes());
    out.extend_from_slice(
        &u16::try_from(node_names.len())
            .expect("names fit u16")
            .to_le_bytes(),
    );
    for n in node_names {
        let b = n.as_bytes();
        out.extend_from_slice(&u16::try_from(b.len()).expect("len fits u16").to_le_bytes());
        out.extend_from_slice(b);
        *names += 1;
    }
    for ch in children {
        push_blob_node(ch, out, names);
    }
}

/// The module-input blob and an UPPER BOUND on the names it interns.
///
/// **The two are returned together deliberately.** They are the same walk, and
/// until this moved out of the test harness the caller supplied the count
/// separately — from a model that omitted the data-slot contributor entirely,
/// reporting 252 for `parse` where the module really interns 627. A count that
/// disagrees with the blob it describes is the "one model with two readers"
/// shape that produced the understated worst-case-memory bound, so there is one
/// walk and no second opinion.
///
/// **It is a BOUND rather than the record count, and the direction matters.**
/// The count returned here is the number of interning EVENTS. The reference
/// dedups, so its `NAMES` record count can be lower — and by how much is
/// order-dependent, because `Names::intern_fresh` records its entry in the
/// index so a later `intern` can share it. Reproducing the exact count
/// host-side would mean replicating the reference's interning ORDER, which is
/// precisely what `wire.kel`'s `mi_*` producers already do and what the
/// byte-identity oracle already checks. Duplicating it here would be a second
/// model of the thing under test.
///
/// The bound's only consumer is the cap check in [`wire_names_via_kel`], where
/// over-counting refuses slightly early and under-counting would admit a module
/// that overruns the interner. The old caller under-counted. On all ten corpus
/// stages the bound is exact; an enum constant makes it loose, and both facts
/// are pinned by test.
///
/// Sections, in the order `wire.kel`'s `mi_*` producers read them: chunk names,
/// the enum layouts, the data-slot runs, then the constant forest as a tail.
/// **Every count is written even when it is zero.** Inferring an absent section
/// from the blob ENDING cannot distinguish empty from truncated, and it would
/// have passed here by accident, because `bin` is zero-filled past the blob so a
/// reader would find a zero count and be right for a reason that is not the
/// encoding.
pub fn module_input(module: &Module) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut names = 0usize;
    fn push_name(out: &mut Vec<u8>, s: &str, names: &mut usize) {
        let b = s.as_bytes();
        let l = u16::try_from(b.len()).expect("name length fits u16");
        out.extend_from_slice(&l.to_le_bytes());
        out.extend_from_slice(b);
        *names += 1;
    }

    let n = u16::try_from(module.chunks.len()).expect("chunk count fits u16");
    out.extend_from_slice(&n.to_le_bytes());
    for c in &module.chunks {
        push_name(&mut out, &c.name, &mut names);
    }

    // The constant section rides at the END, so the sections the stage reads
    // first are at fixed offsets from the start.
    let consts = const_roots_of(module);
    let mut nodes = Vec::new();
    let mut const_names = 0usize;
    for c in &consts {
        push_blob_node(c, &mut nodes, &mut const_names);
    }
    let cn = u16::try_from(count_blob_nodes(&consts)).expect("node count fits u16");
    let mut tail = Vec::new();
    tail.extend_from_slice(&cn.to_le_bytes());
    tail.extend_from_slice(&nodes);

    let e = u16::try_from(module.enum_layouts.len()).expect("enum count fits u16");
    out.extend_from_slice(&e.to_le_bytes());
    for l in &module.enum_layouts {
        push_name(&mut out, &l.type_name, &mut names);
        let v = u16::try_from(l.variants.len()).expect("variant count fits u16");
        out.extend_from_slice(&v.to_le_bytes());
        for var in &l.variants {
            push_name(&mut out, &var.name, &mut names);
        }
    }

    // The DATA-SLOT section, one name per RUN. Consecutive slots sharing a name
    // and visibility collapse into one record in the reference, and the name is
    // interned once per run; interning per SLOT would emit one name per array
    // element, which is how the pre-run-length-encoding artifact reached tens of
    // thousands of names and produced the 395,804 figure that outlived it.
    let mut runs: Vec<&str> = Vec::new();
    if let Some(dl) = &module.data_layout {
        let mut i = 0usize;
        while i < dl.slots.len() {
            let s = &dl.slots[i];
            let mut n = 1usize;
            while i + n < dl.slots.len()
                && dl.slots[i + n].name == s.name
                && dl.slots[i + n].visibility == s.visibility
            {
                n += 1;
            }
            runs.push(s.name.as_str());
            i += n;
        }
    }
    let sn = u16::try_from(runs.len()).expect("slot run count fits u16");
    out.extend_from_slice(&sn.to_le_bytes());
    for r in &runs {
        push_name(&mut out, r, &mut names);
    }

    out.extend_from_slice(&tail);
    (out, names + const_names)
}

/// name lengths, no offsets, no interning sequence. Keleusma recovers all three
/// and runs the interner and the emitters in ONE call, because shared data is
/// re-seeded on every call and the sequence would not survive a return.
///
/// **Bounded, and it says so.** The interner admits at most `NAME_CAP` names per
/// call and the blob must fit `wire.kel`'s `bin`. Both bounds now cover every
/// stage in the corpus: the largest, `parse`, interns 627 names from a
/// 33,395-byte blob. A module past either bound is refused rather than
/// truncated, because a silent partial artifact would be byte-identical for a
/// prefix and wrong after it.
///
/// The figure previously named here — "a real stage's 395,804" — described no
/// name count. It was a REGION RECORD count belonging to `CONSTS`, carried over
/// from the pre-run-length-encoding representation, and it made a 2.5x problem
/// look like a 1500x one. Measured: the largest `NAMES` region is 627 records.
///
/// **THE DRIVER IS WIRED TO THE MODULE, NOT TO A MODEL.** It builds the blob and
/// its name count with [`module_input`] rather than accepting them. Until this
/// changed the function took a pre-built blob and opened with `let _ = module;`
/// — the module was in the signature and unused, and the only producer of the
/// blob was a Rust function in the test harness. A path that cannot be driven
/// from a `Module` is not a compile path, however byte-identical its output.
///
/// [`wire_names_from_input`] remains for tests that must inject a blob or a
/// count the encoder would never produce, which is the only way to reach the
/// two cap refusals.
pub fn wire_names_via_kel(
    module: &Module,
    directory: &[u8],
    regions: usize,
) -> Result<Vec<u8>, SelfHostError> {
    let (blob, names) = module_input(module);
    wire_names_from_input(&blob, names, directory, regions)
}

/// [`wire_names_via_kel`] with the module input supplied directly.
///
/// Separate so the cap refusals are reachable: both bounds cover every stage in
/// the corpus, so no real module can exercise them and a negative test must
/// inject the input.
pub fn wire_names_from_input(
    blob: &[u8],
    names: usize,
    directory: &[u8],
    regions: usize,
) -> Result<Vec<u8>, SelfHostError> {
    /// The interner's per-call name bound, mirrored from `wire.kel`'s
    /// `nm_max_names`. Named rather than spelled inline so a widening there is
    /// a compile-time mismatch here rather than a silent overrun.
    const NAME_CAP: usize = 1024;
    /// The blob buffer's size, mirrored from `wire.kel`'s `bin_capacity`.
    ///
    /// Checked HERE because the stage cannot check it: the host seeds `bin` by
    /// slot before the call, and a blob longer than the array writes past it or
    /// is silently truncated depending on the seeding loop. `bin_capacity` is
    /// consulted inside `wire.kel` only against the interner's cursor, which is
    /// a different quantity.
    const BLOB_CAP: usize = 49152;
    if names > NAME_CAP {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "wire.kel interns at most {NAME_CAP} names per call and this module has \
                 {names}; staging is not implemented"
            ),
        });
    }
    if blob.len() > BLOB_CAP {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "wire.kel's blob buffer holds {BLOB_CAP} bytes and this module's blob is \
                 {}; staging is not implemented",
                blob.len()
            ),
        });
    }
    let m = compile_src(&read_stage("kel/wire.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify wire.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    // Slot layout mirrors `wire.kel`'s shared block: `len`, then `bytes`, then
    // the region inputs. The directory is seeded so `dir_find` can locate
    // NAMES and STRING_POOL; the blob rides `bin`.
    use wire_slots::{BIN as BIN_SLOT, NREGIONS as NREGIONS_SLOT};
    const CMD_JOIN: i64 = 167;
    vm.set_shared(&mut shared, 0, Value::Int(directory.len() as i64))
        .expect("len");
    // `dir_find` walks `0..wire.nregions`, so a directory seeded without its
    // COUNT is a directory the stage cannot see: the first attempt refused with
    // -233, which reads as "no NAMES region" and means "no regions at all".
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(regions as i64))
        .expect("nregions");
    for (i, &b) in directory.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b))
            .expect("seed");
    }
    for (i, &b) in blob.iter().enumerate() {
        vm.set_shared(&mut shared, BIN_SLOT + i, Value::Byte(b))
            .expect("blob");
    }
    let st = vm
        .call_with_shared(&mut shared, &[Value::Int(CMD_JOIN)])
        .expect("run wire.kel");
    match st {
        crate::vm::VmState::Yielded(Value::Int(v)) if v >= 0 => {}
        other => {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel refused the join: {other:?}"),
            });
        }
    }
    let mut out = vec![0u8; directory.len()];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = match vm.get_shared(&shared, 1 + i).expect("read") {
            Value::Byte(b) => b,
            other => panic!("shared byte slot held {other:?}"),
        };
    }
    Ok(out)
}

/// The eleven `HEADER` record fields a module contributes, in the order
/// `emit_header_record_at` reads them from `fin`.
///
/// Host-side because they are scalar module properties rather than anything the
/// blob walk derives. See [`wire_regions_via_kel`] for what that means for the
/// coverage claim.
fn header_fields_of(module: &Module) -> [i64; 11] {
    [
        // `ABSENT` for a module with no entry point, matching the reference's
        // own sentinel rather than inventing one.
        module
            .entry_point
            .map_or(i64::from(crate::wire_schema::ABSENT), |e| e as i64),
        module.word_bits_log2 as i64,
        module.addr_bits_log2 as i64,
        module.float_bits_log2 as i64,
        0, // flags
        0, // wcet_cycles
        0, // wcmu_bytes
        0, // shared_data_bytes
        0, // private_data_bytes
        0, // schema_hash
        0, // reserved
    ]
}

/// Emit `NAMES`, `STRING_POOL` and the `HEADER` record for `module` through
/// `wire.kel`, returning the artifact bytes.
///
/// # What this covers, stated precisely because the distinction is the point
///
/// - `NAMES` and `STRING_POOL` are **computed**: the stage walks the module blob
///   built by [`module_input`], interns the names itself, and derives every byte
///   of both regions from that walk.
/// - `HEADER` is **encoded but not derived**: the eleven scalar field values are
///   read off the `Module` here and seeded into `fin`, and the stage decides the
///   record's offsets, widths and endianness. Keleusma owns the ENCODING, the
///   host owns the NUMBERS.
///
/// Neither region's payload comes from the reference artifact, so both are
/// module-driven; but only the first two are self-hosted end to end, and a
/// reader should not read the third as more than it is.
///
/// # Why `CONSTS` is not here, which is the largest region but no longer a dominant one
///
/// `CONSTS` is **37,152 bytes across the eleven stages, 33.9% of the
/// 109,552-byte corpus auxiliary body**, against 25,752 for `NAMES` and
/// `STRING_POOL` together. It is still the largest single region and still the
/// obvious next target. **The share is of the BODY**, which is what the 90.5%
/// it replaces was a share of; against the sum of region payloads alone the same
/// corpus reads 37.5%, and quoting the two interchangeably is how a figure stops
/// meaning anything.
///
/// **THIS PARAGRAPH SAID 645,312 BYTES AND 90.5% OF 712,936, AND CLAIMED EVERY
/// FIGURE IN IT WAS DERIVED BY A TEST.** Neither was true. Those numbers predate
/// the all-default elision, which removed the 38,087 wholly-default private-slot
/// initialisers that made up the bulk of the region, and no test asserted them —
/// so the correction was available to anyone who measured and to nobody who
/// read. That is the seventh stale-figure incident on this line and the third in
/// this doc comment specifically; an earlier revision quoted 663,120 and 34,960
/// from a measurement nothing re-ran.
///
/// The share is now pinned by `the_recorded_region_magnitude_is_the_one_the_tree_produces`
/// in `tests/consts_region_composition.rs`, as a band rather than an exact
/// figure: an exact pin fails on every stage edit, which trains its reader to
/// re-baseline it rather than to read it.
///
/// Two obstacles were recorded. **Only the first is real for this corpus**, and
/// the correction matters because the second is the one that looked like a
/// design decision:
///
/// 1. **The producer and the consumer use different arrays.** `mi_put_node_full`
///    writes the constant node table into `wire.bytes` at byte zero, which is
///    where the artifact lives, while the flattener reads its nodes from
///    `wire.fin`. Running the node walk inside a join that also emits would
///    overwrite the directory, which is the same failure this file already
///    records for the seventh chunk onward. This one stands.
/// 2. **The two paths intern in different orders** — the module walk in preorder
///    by linear scan, the flattener breadth-first, with the difference
///    observable in `NAMES`. True of the general case and **unreachable here**.
///    The flattener interns only for `StaticStr`, `Struct` and `Enum` nodes, and
///    all 40,332 constants across the eleven stages are `Int`. Pinned by
///    `the_flattener_interns_no_name_for_any_stage`, which asserts both the node
///    census and the observation that clearing every constant leaves the string
///    pool byte-identical.
///
/// # What actually blocks it, which is neither of the two above
///
/// The emit machinery is **already driven from real modules and already
/// byte-identical**: `keleusma_flattens_a_constant_forest_breadth_first` in
/// `tests/selfhost_wire.rs` compiles a source, seeds its constants into
/// `wire.fin` as nodes, and compares the emitted `CONSTS` region against the
/// reference. What excludes the eleven stages is a CAPACITY BOUND, not a
/// disagreement:
///
/// * **The 170-node flattener cap.** `wire.fin` is 1,024 words and a node costs
///   six, so the flattener walks at most 170 nodes per call. `parse` needs
///   **857**, so it needs six calls rather than one. **This figure was recorded
///   as 17,391 and that was a forest nothing emits**: it counted the
///   wholly-default initialisers the encoder elides, overstating the margin
///   twenty-six-fold. The bound still excludes the stages and the magnitude was
///   wrong, which is why both are stated.
/// * **This is not the only node cap and the two are easy to conflate.** The
///   MODULE-INPUT walk separately refuses past 1,024 NODES (`nm_max_names`,
///   error `-240`), which is what `wire.kel` hits at 1,194 chunk constants.
///   Different walks, different bounds, and only the first is derived from a
///   word count.
/// * **What the emitted set actually is.** `keleusma::wire_schema::constant_roots`
///   is the one definition of it: every chunk's constants in chunk order, then
///   `DataLayout::private_init` unless the encoder elides it. `const_roots_of`
///   below is the narrower BLOB model and is not that set. For this corpus the
///   two coincide, because every stage's private pool is wholly default and so
///   contributes nothing — but they coincide by measurement rather than by
///   construction, and a stage that gained one non-zero initialiser would part
///   them silently.
///
/// # Why enlarging `wire.fin` cannot be the answer
///
/// A stage's private data array is itself initialised, one `Int(0)` per word, so
/// widening `fin` to hold N nodes adds 6N records to `wire.kel`'s **own**
/// `CONSTS` region. Walking `parse`'s 857 nodes needs `fin` at 5,142 words,
/// which is 82,272 bytes of `CONSTS` in the walking stage — six times the
/// 13,712-byte region it is trying to emit. **The ratio is the node width and is
/// what makes this a non-answer**; it does not depend on the forest size, which
/// is why the correction to that size leaves the conclusion standing. The
/// stage's capacity to describe a data segment is paid for out of a data segment
/// described the same way, so the approach diverges rather than converging.
/// Batching across calls is the route, and for a scalar-only forest it is sound
/// with no carried state, because each record depends on nothing outside
/// itself.
///
/// `STRUCT_AUX` and `ENUM_AUX` are deliberately not candidates either: measured
/// across the eleven stages, both regions are EMPTY in all of them, so a byte
/// identity for either would pass without emitting anything. That is the same
/// fact as the census above, seen from the other side — both regions are written
/// only for `Struct` and `Enum` constants, and there are none.
pub fn wire_regions_via_kel(
    module: &Module,
    directory: &[u8],
    regions: usize,
) -> Result<Vec<u8>, SelfHostError> {
    let (blob, names) = module_input(module);
    wire_regions_from_input(&blob, names, directory, regions, &header_fields_of(module))
}

/// [`wire_regions_via_kel`] with the module input and header fields supplied
/// directly, so the cap refusals stay reachable.
pub fn wire_regions_from_input(
    blob: &[u8],
    names: usize,
    directory: &[u8],
    regions: usize,
    header: &[i64; 11],
) -> Result<Vec<u8>, SelfHostError> {
    const NAME_CAP: usize = 1024;
    const BLOB_CAP: usize = 49152;
    if names > NAME_CAP {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "wire.kel interns at most {NAME_CAP} names per call and this module has \
                 {names}; staging is not implemented"
            ),
        });
    }
    if blob.len() > BLOB_CAP {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "wire.kel's blob buffer holds {BLOB_CAP} bytes and this module's blob is \
                 {}; staging is not implemented",
                blob.len()
            ),
        });
    }
    let m = compile_src(&read_stage("kel/wire.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify wire.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    use wire_slots::{BIN as BIN_SLOT, FIN as FIN_SLOT, NREGIONS as NREGIONS_SLOT};
    /// `mi_join_header`. Mirrored from `wire.kel`'s dispatch, where
    /// `highest_command` is a real guard: a command above it returns `0 - 99`.
    const CMD_JOIN_HEADER: i64 = 168;
    vm.set_shared(&mut shared, 0, Value::Int(directory.len() as i64))
        .expect("len");
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(regions as i64))
        .expect("nregions");
    for (i, &b) in directory.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b))
            .expect("seed");
    }
    for (i, &v) in header.iter().enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + i, Value::Int(v))
            .expect("header field");
    }
    for (i, &b) in blob.iter().enumerate() {
        vm.set_shared(&mut shared, BIN_SLOT + i, Value::Byte(b))
            .expect("blob");
    }
    let st = vm
        .call_with_shared(&mut shared, &[Value::Int(CMD_JOIN_HEADER)])
        .expect("run wire.kel");
    match st {
        crate::vm::VmState::Yielded(Value::Int(v)) if v >= 0 => {}
        other => {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel refused the widened join: {other:?}"),
            });
        }
    }
    let mut out = vec![0u8; directory.len()];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = match vm.get_shared(&shared, 1 + i).expect("read") {
            Value::Byte(b) => b,
            other => panic!("shared byte slot held {other:?}"),
        };
    }
    Ok(out)
}

/// The ten host-supplied fields of each chunk record, in the order
/// `emit_chunks_batch` reads them from `fin` after slot zero.
///
/// Slot zero is left at zero deliberately: the stage overwrites it with the
/// interner's own index, so the host cannot supply a name index the `NAMES`
/// region would disagree with.
fn chunk_fields_of(module: &Module) -> Vec<i64> {
    use crate::bytecode::BlockType;
    let mut out = Vec::with_capacity(module.chunks.len() * 11);
    for c in &module.chunks {
        out.push(0); // name: the stage fills this from the interner
        out.push(c.constants.len() as i64);
        out.push(c.struct_templates.len() as i64);
        out.push(c.param_types.len() as i64);
        // No debug pool in the auxiliary body this path builds, so the span is
        // ABSENT rather than an empty range. The reference uses the sentinel to
        // distinguish `None` from `Some(empty)`.
        out.push(i64::from(crate::wire_schema::ABSENT));
        out.push(0);
        out.push(0); // op_byte_offset
        out.push(0); // op_record_count
        out.push(c.local_count as i64);
        out.push(c.param_count as i64);
        out.push(match c.block_type {
            BlockType::Func => i64::from(crate::wire_schema::block_tag::FUNC),
            BlockType::Reentrant => i64::from(crate::wire_schema::block_tag::REENTRANT),
            BlockType::Stream => i64::from(crate::wire_schema::block_tag::STREAM),
        });
    }
    out
}

/// `wire.kel`'s single-batch chunk cap, mirrored so a module past it is refused
/// here rather than truncated there.
const CHUNK_BATCH_CAP: usize = 90;

/// `wire.bytes`, the stage's artifact buffer. Mirrored for the same reason.
const STAGE_BUFFER_BYTES: usize = 65536;

/// Emit `NAMES`, `STRING_POOL`, the `HEADER` record and the `CHUNKS` region for
/// `module`, returning the artifact bytes.
///
/// # What each region owes to whom
///
/// - `NAMES` and `STRING_POOL` are **computed**: the stage walks the module blob
///   and derives every byte.
/// - `HEADER` is **encoded but not derived**: eleven scalars come from the host.
/// - `CHUNKS` is **mixed, per field**. The stage computes the name index, taking
///   it from the interner that produced `NAMES` rather than from the host, and it
///   computes the three running range cursors `consts_first`, `templates_first`
///   and `param_types_first` by accumulation across records. The other ten fields
///   per record are host-supplied.
///
/// # Which modules this serves, and why not all of them
///
/// Two limits, both mirrored from `wire.kel` and both measured against the eleven
/// stage sources:
///
/// - **90 chunk records per call.** `wire.fin` is 1024 words at eleven per chunk.
///   `wire.kel` itself has 466 chunks and `parse.kel` has 94.
/// - **65,536 artifact bytes.** The emitter writes at absolute artifact offsets
///   into `wire.bytes`. `parse.kel` produces 304,432 bytes, `codegen.kel` 111,864
///   and `verify_structural.kel` 102,256.
///
/// Seven of the eleven stages satisfy both. The four that do not are refused with
/// a stated reason rather than truncated. `ck_emit_window` exists for the
/// oversize case and is not driven from a module yet.
pub fn wire_chunks_via_kel(
    module: &Module,
    directory: &[u8],
    regions: usize,
) -> Result<Vec<u8>, SelfHostError> {
    if module.chunks.len() > CHUNK_BATCH_CAP {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "wire.kel emits at most {CHUNK_BATCH_CAP} chunk records per call and this \
                 module has {}; the windowed path is not driven from a module yet",
                module.chunks.len()
            ),
        });
    }
    // NO ARTIFACT-SIZE GUARD HERE, AND THE FIRST VERSION HAD ONE THAT COULD NEVER
    // FIRE. It compared `directory.len()` against the buffer, but that length is
    // the SHARED ARRAY's size, not the artifact's -- it is 65,536 for every
    // module, so the comparison was false by construction. A guard that cannot
    // fire is worse than none: it reads as coverage.
    //
    // The real limit is enforced where it is actually known. The stage writes at
    // absolute artifact offsets and FAILS CLOSED with an out-of-bounds fault
    // naming the offset and the bound, which the call below turns into a refusal.
    let (blob, names) = module_input(module);
    wire_chunks_from_input(
        &blob,
        names,
        directory,
        regions,
        &header_fields_of(module),
        &chunk_fields_of(module),
        module.chunks.len(),
    )
}

/// [`wire_chunks_via_kel`] with every input supplied directly.
#[allow(clippy::too_many_arguments)]
pub fn wire_chunks_from_input(
    blob: &[u8],
    names: usize,
    directory: &[u8],
    regions: usize,
    header: &[i64; 11],
    chunk_fields: &[i64],
    chunk_count: usize,
) -> Result<Vec<u8>, SelfHostError> {
    const NAME_CAP: usize = 1024;
    const BLOB_CAP: usize = 49152;
    if names > NAME_CAP {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "wire.kel interns at most {NAME_CAP} names per call and this module has {names}"
            ),
        });
    }
    if blob.len() > BLOB_CAP {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "wire.kel's blob buffer holds {BLOB_CAP} bytes and this module's blob is {}",
                blob.len()
            ),
        });
    }
    let m = compile_src(&read_stage("kel/wire.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify wire.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    use wire_slots::{
        BIN as BIN_SLOT, FIN as FIN_SLOT, NREGIONS as NREGIONS_SLOT, WARG as WARG_SLOT,
    };
    /// `mi_join_chunks`, mirrored from `wire.kel`'s dispatch where
    /// `highest_command` is a real guard.
    const CMD_JOIN_CHUNKS: i64 = 169;
    vm.set_shared(&mut shared, 0, Value::Int(directory.len() as i64))
        .expect("len");
    vm.set_shared(&mut shared, NREGIONS_SLOT, Value::Int(regions as i64))
        .expect("nregions");
    // `warg` carries the chunk-record count the join emits.
    vm.set_shared(&mut shared, WARG_SLOT, Value::Int(chunk_count as i64))
        .expect("warg");
    for (i, &b) in directory.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b))
            .expect("seed");
    }
    // Chunk records take `fin[0..990]` at eleven words each; the header rides
    // `fin[990..1001]`. **Disjoint on purpose.** Every region of one artifact is
    // emitted in a single call, because shared data is re-seeded on every call,
    // so a header at slot zero would be read out of chunk data. The first version
    // of this function did exactly that.
    const HEADER_FIELD_BASE: usize = 990;
    for (i, &v) in header.iter().enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + HEADER_FIELD_BASE + i, Value::Int(v))
            .expect("header field");
    }
    for (i, &v) in chunk_fields.iter().enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + i, Value::Int(v))
            .expect("chunk field");
    }
    for (i, &b) in blob.iter().enumerate() {
        vm.set_shared(&mut shared, BIN_SLOT + i, Value::Byte(b))
            .expect("blob");
    }
    let st = match vm.call_with_shared(&mut shared, &[Value::Int(CMD_JOIN_CHUNKS)]) {
        Ok(st) => st,
        Err(e) => {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!(
                    "wire.kel writes at absolute artifact offsets into a \
                     {STAGE_BUFFER_BYTES}-byte buffer and this artifact reaches past it; \
                     the windowed path is not driven from a module yet ({e:?})"
                ),
            });
        }
    };
    match st {
        crate::vm::VmState::Yielded(Value::Int(v)) if v >= 0 => {}
        other => {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel refused the chunk join: {other:?}"),
            });
        }
    }
    let mut out = vec![0u8; directory.len()];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = match vm.get_shared(&shared, 1 + i).expect("read") {
            Value::Byte(b) => b,
            other => panic!("shared byte slot held {other:?}"),
        };
    }
    Ok(out)
}

/// Emit `NAMES`, `STRING_POOL`, `HEADER` and, where the chunk count allows,
/// `CHUNKS` for `module`, assembling the artifact from one window per region.
///
/// # Why this exists when [`wire_chunks_via_kel`] already emits those regions
///
/// That entry writes at ABSOLUTE artifact offsets into a 65,536-byte buffer, so
/// it serves only a module whose artifact fits. **The ceiling was never about
/// region size.** Measured across the eleven stage sources, the largest of these
/// four payloads is `wire.kel`'s `CHUNKS` at 22,512 bytes, a third of the buffer;
/// what overflows is the OFFSET, and `parse.kel` puts `NAMES` at byte 299,416 of
/// a 304,432-byte artifact.
///
/// So each region is emitted at window offset zero and copied to its true base
/// here. Every stage is reachable for the three regions that need no batching.
///
/// # What each region owes to whom
///
/// - `NAMES` and `STRING_POOL` are **computed**: the stage walks the module blob
///   and derives every byte.
/// - `CHUNKS` is **mixed per field** — the stage computes the name index from its
///   own interner and the three range cursors by accumulation; ten fields per
///   record come from the host.
/// - `HEADER` is **encoded but not derived**: eleven scalars come from the host
///   and the stage owns the record layout.
///
/// # The one limit that remains
///
/// `CHUNKS` is a single batch of at most 90 records. `wire.kel` has 469 chunks
/// and `parse.kel` 94, so both are emitted without it and say so through
/// [`WindowedArtifact::chunks_emitted`]. The batching carries exist in the stage
/// and are exercised from harness inputs; driving them from a module is a further
/// step.
///
/// # Cost, stated because it looks wasteful and is not
///
/// One call per region re-walks the blob and re-interns, because shared data is
/// re-seeded on every call. The interner is a pure function of its input, so the
/// second run is the same answer rather than a second answer. Carrying indices
/// between calls host-side would reproduce the stage's output, which is the drift
/// this crate has now found four times.
pub struct WindowedArtifact {
    /// The assembled artifact bytes.
    pub bytes: Vec<u8>,
    /// Whether the chunk region was emitted. False when the module exceeds the
    /// single-batch cap, in which case those bytes are left zero.
    pub chunks_emitted: bool,
}

/// Emit one region into the window and return the bytes written.
fn window_emit(
    blob: &[u8],
    names: usize,
    header: &[i64; 11],
    chunk_fields: &[i64],
    chunk_count: usize,
    cmd: i64,
    want: usize,
) -> Result<Vec<u8>, SelfHostError> {
    let m = compile_src(&read_stage("kel/wire.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify wire.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, WARG_SLOT, Value::Int(chunk_count as i64))
        .expect("warg");
    for (i, &v) in header.iter().enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + HEADER_FIELD_BASE + i, Value::Int(v))
            .expect("header field");
    }
    // CAPPED AT THE BATCH BOUND, WHICH IS EXACTLY WHERE THE HEADER STARTS.
    // `fin` is 1024 words and a chunk costs eleven, so 90 records fill slots
    // 0..990 and the header rides 990..1001. `parse.kel` has 94 chunks, whose
    // 1034 fields overran the header and silently rewrote it -- the emitted
    // header then differed from the reference for that stage alone, which is how
    // this was found. A module past the cap does not have its chunks emitted at
    // all, so truncating here loses nothing that would otherwise be written.
    const CHUNK_FIELD_LIMIT: usize = CHUNK_BATCH_CAP * 11;
    for (i, &v) in chunk_fields.iter().take(CHUNK_FIELD_LIMIT).enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + i, Value::Int(v))
            .expect("chunk field");
    }
    for (i, &b) in blob.iter().enumerate() {
        vm.set_shared(&mut shared, BIN_SLOT + i, Value::Byte(b))
            .expect("blob");
    }
    // **`names` IS ACCEPTED AND DELIBERATELY UNUSED, AND THAT IS NOT AN OVERSIGHT.**
    //
    // Investigated 2026-08-21 after a sweep for discarded stage inputs flagged it, because a
    // parameter read into a discard is the exact shape of four defects repaired that day. This
    // one is not: `wire.kel` derives the name count itself -- `name_count()` is a function of the
    // blob it was given, returned by command 18 -- so a host-supplied count would be a SECOND
    // answer to a question the stage already answers, which is the drift this file records
    // paying for repeatedly. The only host-supplied argument in this block is `chunk_count`, at
    // `WARG_SLOT`.
    //
    // Kept in the signature rather than removed so every windowed emitter takes the same
    // arguments; removing it is a safe cleanup, not a repair.
    let _ = names;
    let st = match vm.call_with_shared(&mut shared, &[Value::Int(cmd)]) {
        Ok(st) => st,
        Err(e) => {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel faulted emitting into the window: {e:?}"),
            });
        }
    };
    match st {
        crate::vm::VmState::Yielded(Value::Int(v)) if v >= 0 => {}
        other => {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel refused the windowed emit: {other:?}"),
            });
        }
    }
    let mut out = vec![0u8; want];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = match vm.get_shared(&shared, 1 + i).expect("read") {
            Value::Byte(b) => b,
            other => panic!("shared byte slot held {other:?}"),
        };
    }
    Ok(out)
}

// `wire.kel`'s shared-block slot map, for every windowed emitter here.
//
// **THIS COMMENT USED TO SIT ON A SECOND COPY OF THE ARITHMETIC.** It was hoisted
// out of `window_emit` when a second emitter needed it, and it says, correctly,
// that two copies of a slot map is a drift this file's history already records --
// while being one of four. The three `wire_*_from_input` entry points each
// restated it as well. The reasoning was right and the remedy was applied one
// scope too narrowly, which is the more common failure than not knowing the rule.
//
// The definition is `wire_slots`. These are re-exports so the emitters below read
// unchanged; the shared block is addressed BY SLOT, so a constant that disagrees
// with its twin shifts every field after it and produces a WRONG artifact rather
// than a refused one.
use wire_slots::{BIN as BIN_SLOT, FIN as FIN_SLOT, WARG as WARG_SLOT};
/// Where the eleven header scalars ride in `fin`, above the chunk-field area.
const HEADER_FIELD_BASE: usize = 990;

/// Send one command to a suspended `wire.kel` and return the word it yields.
///
/// # Why the RESET skip is bounded rather than a loop
///
/// The stage's `loop main(...)` reports its RESET between iterations, so a
/// yielded value is not always the next state. Skipping resets without a bound
/// would let a stage that only ever resets hang the driver instead of reporting
/// it; four is well past the one reset a well-formed stage produces.
///
/// # Why `NotSuspended` falls back to a call
///
/// The first command on a fresh virtual machine has nothing to resume. Every
/// later one must resume, because CALLING a suspended coroutine stacks another
/// activation rather than replacing it — a several-hundred-record region
/// exhausts the arena that way, and the failure surfaces as an operand-stack
/// error naming neither the call pattern nor the record count.
///
/// # Errors
///
/// [`SelfHostError::Unsupported`] when the stage faults, returns a non-integer
/// state, or resets without ever yielding.
fn enter_wire(vm: &mut Vm<'_, '_>, shared: &mut [u8], cmd: i64) -> Result<i64, SelfHostError> {
    let mut st = match vm.resume_with_shared(shared, Value::Int(cmd)) {
        Err(crate::vm::VmError::NotSuspended) => vm
            .call_with_shared(shared, &[Value::Int(cmd)])
            .map_err(|e| SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel faulted on command {cmd}: {e:?}"),
            })?,
        other => other.map_err(|e| SelfHostError::Unsupported {
            detail: alloc::format!("wire.kel faulted on command {cmd}: {e:?}"),
        })?,
    };
    for _ in 0..4 {
        match st {
            crate::vm::VmState::Yielded(Value::Int(v)) => return Ok(v),
            crate::vm::VmState::Reset => {}
            other => {
                return Err(SelfHostError::Unsupported {
                    detail: alloc::format!("wire.kel returned {other:?} for command {cmd}"),
                });
            }
        }
        st = vm
            .resume_with_shared(shared, Value::Int(cmd))
            .map_err(|e| SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel faulted resuming command {cmd}: {e:?}"),
            })?;
    }
    Err(SelfHostError::Unsupported {
        detail: alloc::format!("wire.kel reset repeatedly without yielding for {cmd}"),
    })
}

/// Emit the whole `CHUNKS` region by STREAMING, one record per call.
///
/// `window_emit` seeds a batch into `fin` and takes one region back, which is why
/// it inherits the batch's 90-record cap: `fin` is 1,024 words at eleven fields a
/// chunk. This drives commands 174 and 175 instead — begin once, then one record
/// per call — so the host holds the artifact and the stage holds one record.
///
/// **The three running range cursors are NOT relayed here, and that is the
/// point.** The batch path takes `first`, `c0`, `t0` and `p0` because a function
/// entered fresh cannot remember; the stage is a coroutine and carries them in a
/// private block across the loop's RESET. A host that recomputed them would be
/// asserting an answer it has no way to check.
///
/// The virtual machine is built ONCE and resumed, never re-called. Calling a
/// suspended coroutine stacks another activation instead of replacing it, and a
/// module with several hundred chunks exhausts the arena that way — the failure
/// arrives as an operand-stack error naming neither the call pattern nor the
/// chunk count.
fn window_emit_chunks(
    blob: &[u8],
    header: &[i64; 11],
    chunk_fields: &[i64],
    want: usize,
) -> Result<Vec<u8>, SelfHostError> {
    const CMD_BEGIN: i64 = 174;
    const CMD_STEP: i64 = 175;
    const FIELDS: usize = 11;

    let m = compile_src(&read_stage("kel/wire.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify wire.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];

    let records = chunk_fields.len() / FIELDS;
    vm.set_shared(&mut shared, WARG_SLOT, Value::Int(records as i64))
        .expect("chunk count");
    for (i, &v) in header.iter().enumerate() {
        vm.set_shared(&mut shared, FIN_SLOT + HEADER_FIELD_BASE + i, Value::Int(v))
            .expect("header field");
    }
    for (i, &b) in blob.iter().enumerate() {
        vm.set_shared(&mut shared, BIN_SLOT + i, Value::Byte(b))
            .expect("blob");
    }

    let began = enter_wire(&mut vm, &mut shared, CMD_BEGIN)?;
    if began < 0 {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!("wire.kel refused the chunk stream with {began}"),
        });
    }

    let mut out = vec![0u8; want];
    // `as_chunks` rather than `chunks_exact`, so `row` is a `&[i64; FIELDS]` with
    // its length known to the type system rather than a slice that happens to be
    // that long. Clippy's `chunks_exact_to_as_chunks` requires it for a CONSTANT
    // chunk size, and the lint is MSRV-gated: `as_chunks` stabilised at 1.88, which
    // is this crate's `rust-version` exactly. Verified against a real 1.88
    // toolchain, not inferred from the lint firing.
    for (j, row) in chunk_fields.as_chunks::<FIELDS>().0.iter().enumerate() {
        for (f, &v) in row.iter().enumerate() {
            vm.set_shared(&mut shared, FIN_SLOT + f, Value::Int(v))
                .expect("chunk field");
        }
        let wrote = enter_wire(&mut vm, &mut shared, CMD_STEP)?;
        if wrote <= 0 {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel refused chunk record {j} with {wrote}"),
            });
        }
        let stride = wrote as usize;
        let at = j * stride;
        if at + stride > want {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!(
                    "chunk record {j} would end at {} in a {want}-byte region",
                    at + stride
                ),
            });
        }
        for k in 0..stride {
            out[at + k] = match vm.get_shared(&shared, 1 + k).expect("read") {
                Value::Byte(b) => b,
                other => panic!("shared byte slot held {other:?}"),
            };
        }
    }
    Ok(out)
}

/// Emit a module's `CONSTS` region by STREAMING, one constant record per call.
///
/// # Why this path exists at all, when `fl_walk` already emits a byte-identical region
///
/// `fl_walk` is the breadth-first flattener and it is capped at 170 nodes,
/// because the whole forest must sit in `wire.fin` — 1,024 words at six words a
/// node. It needs a QUEUE: a composite's record carries `(first, count)` into
/// children numbered after every node at its own depth, so it cannot write a
/// record until it knows how many nodes precede its children.
///
/// **A forest of scalars has no children.** The queue never grows past the roots,
/// the walk degenerates to a linear scan, and it becomes one node in, one record
/// out with no state but a cursor. That is commands 176 and 177, and it is not
/// bounded by the cap because the stage holds ONE node rather than the forest.
/// Measured 2026-08-22, every constant across all eleven stages is an `Int` and
/// the largest forest is `parse` at 857 nodes — five times the walk's cap and
/// unbounded for this path.
///
/// # The refusal is the point, not a limitation
///
/// The stage refuses a node with children (`-264`), an interning tag (`-265`) and
/// a range-carrying tag (`-266`). A composite reaching this path would be emitted
/// with a zero range and a zero `aux`: structurally valid, silently wrong, and
/// indistinguishable downstream from a correct record. **The refusal is what keeps
/// the gap visible instead of encoding it in the bytes**, so this function relays
/// the code rather than falling back to `fl_walk`.
///
/// # Errors
///
/// [`SelfHostError::Unsupported`] when the stage refuses a node, naming the node
/// index and the refusal code, or when the stage faults.
pub fn wire_consts_via_kel(module: &Module) -> Result<Vec<u8>, SelfHostError> {
    let roots = crate::wire_schema::constant_roots_of_module(module);
    let mut fields = Vec::new();
    for r in &roots {
        push_const_preorder(r, &mut fields);
    }
    window_emit_consts(&fields)
}

/// One constant subtree in depth-first preorder, six words a node.
///
/// The six words are `(tag, payload, child count, names_first, flags,
/// discriminant)`, matching what `wire.kel` reads out of `fin`. **Every word is
/// written even when it is zero**: the stride is what locates the NEXT node, so a
/// short record silently shifts the whole forest rather than failing.
///
/// `names_first` is zero because this path admits no interning tag; the stage
/// refuses `StaticStr`, `Struct` and `Enum` rather than reading it.
fn push_const_preorder(c: &ConstValue, out: &mut Vec<i64>) {
    let (tag, payload) = const_tag_and_name(c);
    let children = const_children(c);
    let (flags, disc) = const_flags_and_discriminant(c);
    out.push(i64::from(tag));
    out.push(payload);
    out.push(children.len() as i64);
    out.push(0);
    out.push(flags);
    out.push(disc);
    for ch in children {
        push_const_preorder(ch, out);
    }
}

/// Drive commands 176 and 177 over a preorder node stream, concatenating the
/// records.
///
/// The same coroutine discipline as [`window_emit_chunks`]: the virtual machine
/// is built ONCE and resumed. Calling a suspended coroutine stacks another
/// activation rather than replacing it, and a stage with several hundred
/// constants exhausts the arena that way, reporting an operand-stack error that
/// names neither the call pattern nor the constant count.
fn window_emit_consts(node_fields: &[i64]) -> Result<Vec<u8>, SelfHostError> {
    const CMD_BEGIN: i64 = 176;
    const CMD_STEP: i64 = 177;
    /// `(tag, payload, children, names_first, flags, discriminant)`.
    const FIELDS: usize = 6;

    if !node_fields.len().is_multiple_of(FIELDS) {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "the preorder stream is {} words, not a multiple of {FIELDS}",
                node_fields.len()
            ),
        });
    }

    let m = compile_src(&read_stage("kel/wire.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify wire.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];

    let began = enter_wire(&mut vm, &mut shared, CMD_BEGIN)?;
    if began < 0 {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!("wire.kel refused the constant stream with {began}"),
        });
    }

    let mut out = Vec::with_capacity(node_fields.len() / FIELDS * 16);
    for (j, row) in node_fields.as_chunks::<FIELDS>().0.iter().enumerate() {
        for (f, &v) in row.iter().enumerate() {
            vm.set_shared(&mut shared, FIN_SLOT + f, Value::Int(v))
                .expect("node field");
        }
        let wrote = enter_wire(&mut vm, &mut shared, CMD_STEP)?;
        if wrote <= 0 {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!(
                    "wire.kel refused constant node {j} with {wrote}; -264 is a node with \
                     children, -265 an interning tag, -266 a range-carrying tag, and each \
                     means this forest is outside the streaming path rather than that the \
                     path is broken"
                ),
            });
        }
        for k in 0..wrote as usize {
            out.push(
                match vm.get_shared(&shared, wire_slots::BYTES + k).expect("read") {
                    Value::Byte(b) => b,
                    other => panic!("shared byte slot held {other:?}"),
                },
            );
        }
    }
    Ok(out)
}

/// Emit a module's `SHAPES` region by STREAMING, one record per call.
///
/// # What this region is, and the standing it has
///
/// `SHAPES` is the deduplicated table of operand shapes a signature's parameters,
/// return and resume value refer to by index. The indices are an **encoder
/// decision** rather than a property of the module, so the host cannot derive
/// them and must take them from the one definition the encoder itself consumes,
/// [`crate::wire_schema::signature_tables`].
///
/// That makes this region **encoded but not derived**, the standing the `HEADER`
/// record has: Keleusma decides every byte of the record's layout and the host
/// decides the values. `wire.kel` says the same thing about its own formatters,
/// and counting them beside `NAMES` -- which the stage computes from the module
/// blob -- would overstate what is self-hosted.
///
/// # Errors
///
/// [`SelfHostError::Unsupported`] when the stage refuses a record or faults.
pub fn wire_shapes_via_kel(module: &Module) -> Result<Vec<u8>, SelfHostError> {
    let (shapes, _) = crate::wire_schema::signature_tables(&module.signatures);
    let mut fields = Vec::with_capacity(shapes.len() * 4);
    for r in &shapes {
        fields.push(i64::from(r.tag));
        fields.push(i64::from(r.kind));
        fields.push(i64::from(r.reserved));
        fields.push(i64::from(r.size));
    }
    window_emit_records(&fields, 4, 179, "SHAPES")
}

/// Emit a module's `SIGNATURES` region by STREAMING, one record per call.
///
/// Same standing as [`wire_shapes_via_kel`]: the parameter run and the two shape
/// indices come from the encoder's own numbering, so this is encoded rather than
/// derived.
///
/// # Errors
///
/// [`SelfHostError::Unsupported`] when the stage refuses a record or faults.
pub fn wire_signatures_via_kel(module: &Module) -> Result<Vec<u8>, SelfHostError> {
    let (_, sigs) = crate::wire_schema::signature_tables(&module.signatures);
    let mut fields = Vec::with_capacity(sigs.len() * 4);
    for r in &sigs {
        fields.push(i64::from(r.params_first));
        fields.push(i64::from(r.params_count));
        fields.push(i64::from(r.ret));
        fields.push(i64::from(r.resume));
    }
    window_emit_records(&fields, 4, 180, "SIGNATURES")
}

/// Drive a stateless record formatter over a field stream, concatenating the
/// records it returns.
///
/// # Why these need no `begin` and the chunk and constant streams do
///
/// `wire.kel` says it in its own comment: the chunk and constant streams carry
/// state between records -- running range cursors, a node cursor -- and these
/// depend on nothing outside themselves. A `begin` here would exist only to look
/// symmetric with its neighbours, and a reader would have to open its body to
/// learn it reset nothing.
///
/// The virtual machine is still built ONCE and resumed. Calling a suspended
/// coroutine stacks another activation rather than replacing it, and `wire.kel`
/// carries 486 signatures, which exhausts the arena that way.
///
/// # Errors
///
/// [`SelfHostError::Unsupported`] when the field stream is not a whole number of
/// records, when the stage refuses one, or when the stage faults.
fn window_emit_records(
    fields: &[i64],
    per_record: usize,
    cmd: i64,
    label: &str,
) -> Result<Vec<u8>, SelfHostError> {
    if !fields.len().is_multiple_of(per_record) {
        return Err(SelfHostError::Unsupported {
            detail: alloc::format!(
                "the {label} field stream is {} words, not a multiple of {per_record}",
                fields.len()
            ),
        });
    }

    let m = compile_src(&read_stage("kel/wire.kel"));
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify wire.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];

    let mut out = Vec::new();
    for (j, row) in fields.chunks(per_record).enumerate() {
        for (f, &v) in row.iter().enumerate() {
            vm.set_shared(&mut shared, FIN_SLOT + f, Value::Int(v))
                .expect("record field");
        }
        let wrote = enter_wire(&mut vm, &mut shared, cmd)?;
        if wrote <= 0 {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!("wire.kel refused {label} record {j} with {wrote}"),
            });
        }
        for k in 0..wrote as usize {
            out.push(
                match vm.get_shared(&shared, wire_slots::BYTES + k).expect("read") {
                    Value::Byte(b) => b,
                    other => panic!("shared byte slot held {other:?}"),
                },
            );
        }
    }
    Ok(out)
}

/// See [`WindowedArtifact`]. `regions` gives each region's `(kind, base, len)` in
/// the artifact the caller is assembling.
pub fn wire_windowed_via_kel(
    module: &Module,
    artifact_len: usize,
    regions: &[(u16, usize, usize)],
) -> Result<WindowedArtifact, SelfHostError> {
    const WINDOW: usize = 65536;
    let (blob, names) = module_input(module);
    let header = header_fields_of(module);
    let fields = chunk_fields_of(module);
    let mut out = vec![0u8; artifact_len];
    for &(kind, base, len) in regions {
        // CHUNKS TAKES THE STREAMING PATH, so the 90-record batch no longer
        // decides whether a stage is reachable. `parse` (94) and `wire` (475)
        // were the two it excluded and both emit now.
        if kind == crate::wire_schema::kind::CHUNKS {
            let win = window_emit_chunks(&blob, &header, &fields, len)?;
            out[base..base + len].copy_from_slice(&win[..len]);
            continue;
        }
        // CONSTS TAKES THE STREAMING PATH TOO, for the same reason CHUNKS does:
        // it emits at window offset zero, one record per call, so the host
        // places the bytes and no cap decides which stages are reachable.
        //
        // **THE LENGTH IS CHECKED RATHER THAN TRUNCATED.** The branches around
        // this one write `&win[..len]`, which silently discards a disagreement
        // between what the stage produced and what the reference reserved. Here
        // a mismatch is the interesting event -- it means the root model and the
        // encoder have parted -- so it is reported rather than trimmed away.
        if kind == crate::wire_schema::kind::CONSTS {
            let win = wire_consts_via_kel(module)?;
            if win.len() != len {
                return Err(SelfHostError::Unsupported {
                    detail: alloc::format!(
                        "the self-hosted CONSTS region is {} bytes and the reference \
                         reserved {len}; the constant-root model and the encoder disagree",
                        win.len()
                    ),
                });
            }
            out[base..base + len].copy_from_slice(&win);
            continue;
        }
        // SHAPES AND SIGNATURES TAKE THE FORMATTER STREAMS, commands 179 and
        // 180. Both exceed a single `fin` batch on `wire.kel` -- 341 and 486
        // records against a 256-record batch -- so the one-record-per-call form
        // is what makes them reachable at all rather than a stylistic choice.
        //
        // The length is CHECKED rather than truncated, on the same ground as
        // CONSTS above: a mismatch means the host's model of the encoder's
        // numbering has parted from the encoder, which is the interesting event.
        if kind == crate::wire_schema::kind::SHAPES || kind == crate::wire_schema::kind::SIGNATURES
        {
            let win = if kind == crate::wire_schema::kind::SHAPES {
                wire_shapes_via_kel(module)?
            } else {
                wire_signatures_via_kel(module)?
            };
            if win.len() != len {
                return Err(SelfHostError::Unsupported {
                    detail: alloc::format!(
                        "the self-hosted region {kind:#06x} is {} bytes and the reference \
                         reserved {len}; the host's model of the encoder's shape numbering \
                         and the encoder disagree",
                        win.len()
                    ),
                });
            }
            out[base..base + len].copy_from_slice(&win);
            continue;
        }
        let cmd = match kind {
            k if k == crate::wire_schema::kind::NAMES => 170,
            k if k == crate::wire_schema::kind::STRING_POOL => 171,
            k if k == crate::wire_schema::kind::HEADER => 172,
            // EVERY OTHER KIND IS LEFT AS ZEROS, and this is the honest name for
            // what the windowed path does NOT cover. The stage has an emitter for
            // all of them -- `emit_in_window` dispatches eighteen kinds -- so what
            // is missing is the DRIVER supplying their fields, not the Keleusma
            // side. `tests/selfhost_region_coverage.rs` measures which kinds land
            // and which are skipped, so the coverage claim is a figure rather
            // than a sentence.
            _ => continue,
        };
        if len > WINDOW {
            return Err(SelfHostError::Unsupported {
                detail: alloc::format!(
                    "region {kind:#06x} is {len} bytes and the stage window holds {WINDOW};                      emitting a region larger than the window needs batching within it"
                ),
            });
        }
        let win = window_emit(
            &blob,
            names,
            &header,
            &fields,
            module.chunks.len(),
            cmd,
            len,
        )?;
        out[base..base + len].copy_from_slice(&win[..len]);
    }
    Ok(WindowedArtifact {
        bytes: out,
        // Always, now. The field is kept rather than removed because callers read
        // it, and because a flag that is permanently true is a smaller lie than a
        // silently deleted one; a future limit would set it false again.
        chunks_emitted: true,
    })
}

/// Compile a whole program with the self-hosted pipeline, returning a self-hosted-built
/// [`Module`] for an in-subset program at the host target.
///
/// This is the shipping entry behind `keleusma-cli --compiler self-hosted`. It:
/// 1. refuses a non-host `target` (the pipeline is only validated at host width);
/// 2. recovers any out-of-subset pipeline panic into [`SelfHostError::Unsupported`]
///    rather than aborting the process; and
/// 3. cross-checks the self-hosted output against the reference compiler and rejects it
///    if they diverge.
///
/// Step 3 is what makes the backend fail loudly on the whole subset boundary rather than
/// only on constructs that crash a stage: the self-hosted pipeline silently mis-compiles
/// some out-of-subset programs (for example float arithmetic) to a valid-but-wrong
/// module, which neither a panic nor a load-time verify would catch. The reference is
/// used ONLY as the correctness oracle — the returned module is the self-hosted one, so
/// the emitted bytecode is genuinely self-hosted; when the two agree (an in-subset
/// program) that agreement is the [construct-support boundary]'s guarantee. On any
/// divergence the CLI prints a clean error suggesting `--compiler rust`.
pub fn self_hosted_compile(
    src: &str,
    target: &crate::target::Target,
) -> Result<Module, SelfHostError> {
    let host = crate::target::Target::host();
    if target.word_bits_log2 != host.word_bits_log2
        || target.addr_bits_log2 != host.addr_bits_log2
        || target.float_bits_log2 != host.float_bits_log2
    {
        return Err(SelfHostError::NonHostTarget);
    }
    // The reference compile is the correctness oracle. If the reference itself rejects the
    // program it is a plain compile error (the self-hosted compiler cannot do better), so
    // surface it; otherwise its chunks are the yardstick for the divergence check below.
    let reference = compile_reference(src)?;
    // The pipeline `panic!`s on some out-of-subset constructs. Recover those into a clean
    // error; suppress the default panic output for the duration.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let owned = src.to_string();
    let result = std::panic::catch_unwind(move || self_host_compile_scratch(&owned));
    std::panic::set_hook(prev);
    let module = result.map_err(|payload| {
        let detail = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unsupported construct".to_string());
        SelfHostError::Unsupported { detail }
    })?;
    // Correctness cross-check: the self-hosted compiled code (each chunk's ops, constant
    // pool, and local count) must match the reference. A divergence means the program is
    // outside the self-hosted subset; reject it rather than emit a wrong module.
    let diverges = module.chunks.len() != reference.chunks.len()
        || module
            .chunks
            .iter()
            .zip(reference.chunks.iter())
            .any(|(m, r)| {
                m.name != r.name
                    || m.ops != r.ops
                    || m.constants != r.constants
                    || m.local_count != r.local_count
            });
    if diverges {
        return Err(SelfHostError::Unsupported {
            detail: describe_divergence(&module, &reference),
        });
    }
    Ok(module)
}

/// Compile `src` with the reference compiler, mapping any lex/parse/compile failure to
/// [`SelfHostError::ReferenceRejected`] with the reference's message. Used as the
/// correctness oracle by [`crate::selfhost::self_hosted_compile`]. A failure here is a genuine source error,
/// distinct from a self-hosted-subset limitation, so it does not carry the `--compiler rust`
/// retry hint.
fn compile_reference(src: &str) -> Result<Module, SelfHostError> {
    let tokens =
        tokenize(src).map_err(|e| SelfHostError::ReferenceRejected { detail: e.message })?;
    let program =
        parse(&tokens).map_err(|e| SelfHostError::ReferenceRejected { detail: e.message })?;
    compile(&program).map_err(|e| SelfHostError::ReferenceRejected { detail: e.message })
}

/// Describe how the self-hosted `module` diverges from the `reference`, naming the first
/// diverging chunk and the specific dimension (chunk count/order, op index, local frame,
/// or constant pool). This turns the backend's out-of-subset rejection from an opaque
/// "diverges from the reference" into an actionable pointer at the offending function.
///
/// Precondition: the two modules are known to differ (the caller checks equality first);
/// the fallthrough message is retained only for total-function safety.
fn describe_divergence(module: &Module, reference: &Module) -> String {
    if module.chunks.len() != reference.chunks.len() {
        return format!(
            "self-hosted output has {} chunk(s), reference has {}",
            module.chunks.len(),
            reference.chunks.len()
        );
    }
    for (m, r) in module.chunks.iter().zip(reference.chunks.iter()) {
        if m.name != r.name {
            return format!(
                "chunk order diverges: `{}` vs reference `{}`",
                m.name, r.name
            );
        }
        if m.ops != r.ops {
            return match m.ops.iter().zip(r.ops.iter()).position(|(a, b)| a != b) {
                Some(i) => format!(
                    "chunk `{}`: op {} diverges ({:?} vs reference {:?})",
                    m.name, i, m.ops[i], r.ops[i]
                ),
                None => format!(
                    "chunk `{}`: op count {} vs reference {}",
                    m.name,
                    m.ops.len(),
                    r.ops.len()
                ),
            };
        }
        if m.local_count != r.local_count {
            return format!(
                "chunk `{}`: local frame size {} vs reference {}",
                m.name, m.local_count, r.local_count
            );
        }
        if m.constants != r.constants {
            return format!(
                "chunk `{}`: constant pool diverges from the reference",
                m.name
            );
        }
    }
    "the self-hosted output diverges from the reference compiler".to_string()
}

#[cfg(test)]
mod decoder_drift_guard {
    use super::decode_op;

    /// Every op tag the codegen can emit -- currently `1..=63` (the `wire` const
    /// block in `kel/codegen.kel` assigns ops 1..=63 with no gap) -- must decode without hitting the
    /// `unknown op tag` catch-all. This is the FAST guard against decoder DRIFT: an op added to the
    /// codegen wire set but not to `decode_op` above fails this test in microseconds, rather than only
    /// surfacing in a ~40-second stage self-compile. It is the standing regression for the
    /// `unknown op tag 62` defect, which rode into the `v0.2.3` release line undetected because the
    /// subproject decoder had fallen behind the emitted op set and nothing gated the subproject.
    ///
    /// Since P11 Option E widened the op-word radix to 8 bits (`tag + operand*256`), a tag >= 64 is now
    /// REPRESENTABLE, but the codegen still assigns only `1..=63`, so the sweep matches the emitted set;
    /// extend the upper bound here when codegen first assigns an op tag >= 64. A future GAP in the
    /// assigned set would make this test slightly over-strict, which fails safe (it prompts a look
    /// rather than letting an undecoded op ship).
    #[test]
    fn all_wire_op_tags_decode() {
        for tag in 1..=63i64 {
            // operand 0 is the minimal representative word for each tag; decode must not panic.
            let _ = decode_op(tag);
        }
    }
}

/// Structural checks on the classification tables `analyze.kel` consumes.
///
/// These are not numbers, so no numeric differential can see them wrong: a
/// misclassified opcode changes the control-flow graph `analyze.kel`
/// reconstructs, and the bound it extracts from a wrong graph can be finite and
/// wrong rather than absent.
#[cfg(test)]
mod classification_tables {
    use super::{analyze_class, analyze_opk};
    use crate::bytecode::{Chunk, Op};

    fn scratch_chunk() -> Chunk {
        let m = crate::compiler::compile(
            &crate::parser::parse(&crate::lexer::tokenize("fn main() -> Word { 1 }").expect("lex"))
                .expect("parse"),
        )
        .expect("compile");
        m.chunks.first().expect("a chunk").clone()
    }

    /// Every control-flow opcode carries its own class, and its ARGUMENT.
    ///
    /// The argument matters as much as the class: `analyze.kel` follows
    /// `If`/`Loop`/`EndLoop`/`Break` targets to rebuild the graph, so a class
    /// that is right with an argument that is wrong reconstructs a graph that
    /// is plausible and not the program's.
    #[test]
    fn every_control_flow_opcode_keeps_its_class_and_its_target() {
        let cases: &[(Op, i64, i64)] = &[
            (Op::If(11), 1, 11),
            (Op::Else(12), 2, 12),
            (Op::EndIf, 3, 0),
            (Op::Loop(13), 4, 13),
            (Op::EndLoop(14), 5, 14),
            (Op::Break(15), 6, 15),
            (Op::BreakIf(16), 7, 16),
            // Class 8 is PATH EXIT, shared by both opcodes that end a path
            // without transferring control to an enclosing loop.
            (Op::Trap(0), 8, 0),
            (Op::Return, 8, 0),
            (Op::Call(17, 2), 9, 0),
        ];
        for (op, class, arg) in cases {
            let got = analyze_class(op);
            assert_eq!(
                got,
                (*class, *arg),
                "{op:?} classified as {got:?}, expected ({class}, {arg})"
            );
        }
    }

    /// Ordinary opcodes must classify as plain, or `analyze.kel` invents an edge.
    #[test]
    fn ordinary_opcodes_classify_as_plain() {
        let cases: &[Op] = &[
            Op::Const(0),
            Op::GetLocal(0),
            Op::SetLocal(0),
            Op::Add,
            Op::Sub,
            Op::Div,
            Op::CmpEq,
            Op::Dup,
            Op::Not,
            // `Op::Return` WAS HERE until 2026-08-16 and is now class 8. It ends
            // the path, and treating it as plain made `analyze.kel` walk a
            // multiheaded dispatch as though every head ran in sequence. This
            // test failing is how that change was confirmed to reach the table.
            Op::Yield,
            Op::Stream,
            Op::Reset,
            Op::Len,
        ];
        for op in cases {
            assert_eq!(
                analyze_class(op),
                (0, 0),
                "{op:?} was given a control-flow class it does not have"
            );
        }
    }

    /// The class table has exactly nine kinds, and the hole this test used to
    /// describe is CLOSED.
    ///
    /// It previously read: "`analyze_class` ends in `_ => (0, 0)`. A
    /// control-flow opcode added later and not added here becomes 'plain'
    /// silently — no panic, no rejection, just a control-flow graph missing an
    /// edge and a bound extracted from it that is finite and wrong. This test
    /// cannot close that hole." That was correct, and the closing move it named
    /// is the one that was taken: `analyze_class` and `analyze_opk` are now
    /// exhaustive over `Op`, so the compiler refuses a new opcode until someone
    /// decides its class. Verified by adding a variant to `Op` and observing
    /// `E0004` at both sites.
    ///
    /// **This test is still worth keeping, and its job has changed.** The
    /// compiler now guarantees every opcode is CLASSIFIED; it cannot guarantee
    /// the classification is RIGHT. Exhaustiveness is satisfied just as well by
    /// mapping a new control-flow opcode to `(0, 0)` in the plain group, which
    /// is exactly the silent-edge defect wearing a different hat. So this pins
    /// the count: a tenth kind fails here, and `analyze.kel` needs a decoder for
    /// it before it means anything.
    #[test]
    fn the_class_table_covers_exactly_nine_kinds() {
        let control: &[Op] = &[
            Op::If(0),
            Op::Else(0),
            Op::EndIf,
            Op::Loop(0),
            Op::EndLoop(0),
            Op::Break(0),
            Op::BreakIf(0),
            Op::Trap(0),
            Op::Call(0, 0),
        ];
        let classes: alloc::collections::BTreeSet<i64> =
            control.iter().map(|op| analyze_class(op).0).collect();
        assert_eq!(
            classes.len(),
            9,
            "the class table no longer has nine distinct kinds; a new one needs a decoder \
             in analyze.kel before it means anything"
        );
        assert!(
            !classes.contains(&0),
            "a control-flow opcode fell through to the plain default"
        );
    }

    /// The loop-bound extraction's opcode tags, which pick out the induction
    /// variable's update. A wrong tag here yields a bound, not an error.
    #[test]
    fn the_bound_extraction_tags_name_the_right_opcodes() {
        let chunk = scratch_chunk();
        let cases: &[(Op, i64)] = &[
            (Op::GetLocal(3), 1),
            (Op::SetLocal(4), 2),
            (Op::CmpGe, 4),
            (Op::BreakIf(0), 5),
            (Op::CheckedAdd, 6),
            (Op::PopN(2), 7),
            (Op::EndLoop(0), 8),
            (Op::Loop(0), 9),
            (Op::Add, 0),
            (Op::Mul, 0),
        ];
        for (op, opk) in cases {
            let got = analyze_opk(op, &chunk).0;
            assert_eq!(got, *opk, "{op:?} tagged {got}, expected {opk}");
        }
        // The slot travels with the tag; a tag without its slot reads the wrong
        // local and extracts a bound for a variable that is not the induction one.
        assert_eq!(analyze_opk(&Op::GetLocal(3), &chunk).1, 3);
        assert_eq!(analyze_opk(&Op::SetLocal(4), &chunk).1, 4);
        assert_eq!(analyze_opk(&Op::PopN(2), &chunk).2, 2);
    }
}

#[cfg(test)]
mod typecheck_input_feasibility {
    use super::*;

    /// **IDENTITY NOW TRAVELS WITH THE STRUCTURE.**
    ///
    /// Order 1 records that the type checker's input should come from `parse.kel`
    /// plus `reconstruct.kel` because "structure is available" there. Measured, that
    /// was only half true: a `Local` record carries a **slot**, `codegen.kel` lowers
    /// it straight to `GetLocal(slot)`, and no body record mentioned a name at all.
    /// The type channel is keyed by interned NAME ids, so a forest of slots could
    /// not be joined to a binding table of names.
    ///
    /// The operator ruled on the fork: a `let` record carries its name id, rather
    /// than the type channel being keyed by slot for locals and by name for
    /// everything else. `parse.kel` already held the name at the emitting site and
    /// the Option E transport had a full word free.
    ///
    /// # Why the pairing is positional, and why that is safe here
    ///
    /// The name record is emitted immediately before the `LetIn` it belongs to, by
    /// the same fold step, so the slot is the very next record's payload. That is a
    /// positional coupling and normally a smell — it is sound here because one fold
    /// step emits exactly the pair and nothing can be interleaved between them.
    /// **This test is what keeps that true**: it checks the slot and name of every
    /// binding, so a reordering that broke the pairing would show up as a wrong
    /// slot rather than as silence.
    #[test]
    fn every_let_binding_carries_its_slot_and_name() {
        // Padded so a name id cannot coincide with a slot or a literal, which would
        // let a wrong pairing pass by arithmetic accident.
        let mut src = String::new();
        for i in 0..12 {
            src.push_str(&alloc::format!("fn pad{i}() -> Word {{ 0 }}\n"));
        }
        src.push_str("fn main() -> Word { let aardvark = 1; let barnacle = 2; aardvark }");
        let (fns, names, ..) = parse_functions(&src);

        let id = |want: &str| {
            names
                .iter()
                .position(|n| n == want)
                .unwrap_or_else(|| panic!("`{want}` is not interned")) as i64
        };
        let main = fns.last().expect("a function");
        let mut got: Vec<(i64, i64)> = main.let_names.clone();
        got.sort();

        // Two bindings, distinct slots, and the NAMES the source spelled. The fold
        // emits statements last to first, so the pairs are sorted rather than
        // compared in source order.
        assert_eq!(got.len(), 2, "expected two bindings, got {got:?}");
        let mut want = alloc::vec![(got[0].0, id("aardvark")), (got[1].0, id("barnacle"))];
        want.sort();
        assert_eq!(
            got,
            want,
            "the binding names did not arrive with their slots. Slots {:?}, names \
             aardvark={} barnacle={}",
            got.iter().map(|(s, _)| *s).collect::<alloc::vec::Vec<_>>(),
            id("aardvark"),
            id("barnacle")
        );
        assert_ne!(got[0].0, got[1].0, "two bindings share a frame slot");
        assert!(
            got.iter().all(|(s, _)| *s >= 0),
            "a binding was never paired with its `LetIn`, so its slot is still the \
             unpaired sentinel: {got:?}"
        );
    }

    /// The driver's tag is the stage's, checked rather than assumed.
    #[test]
    fn the_let_name_tag_matches_the_stage() {
        const STAGE: &str = include_str!("kel/parse.kel");
        let declared: i64 = STAGE
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("fn tag_let_name() -> Word {"))
            .and_then(|l| l.split('{').nth(1))
            .and_then(|t| t.split('}').next())
            .and_then(|n| n.trim().parse().ok())
            .expect("parse.kel declares `fn tag_let_name()`");
        assert_eq!(
            declared, PARSE_LET_NAME_TAG,
            "the stage emits binding names under {declared} and the driver diverts \
             {PARSE_LET_NAME_TAG}. A mismatch puts the record into the node stream, \
             where `reconstruct.kel` would meet a tag it has no arm for."
        );
    }
}
