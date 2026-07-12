//! The self-hosted compile pipeline as a reusable library.
//!
//! This ports the stage drivers that prove the self-hosted stages self-compile
//! (`../tests/selfhost_codegen.rs` in the parent `keleusma` crate) into the compiler
//! subproject, so the `compile` command and the driver-level fixed-point test share
//! one implementation. It drives all four Keleusma stages over a source:
//! `kel/lexer.kel` tokenizes, `kel/parse.kel` emits a postorder record stream,
//! `kel/reconstruct.kel` folds that into the (kind, arg, lhs, rhs) node forest, and
//! `kel/codegen.kel` emits each chunk's ops, which are spliced into a module. The
//! host only moves data between stages; the compile logic is Keleusma end to end.
//! One limitation remains on the road to full self-hosting, documented in
//! `MILESTONES.md`: the module scaffold (data layout, constant-pool metadata,
//! auxiliary body) is taken from the Rust-hosted reference compiler (`compile_src`)
//! rather than assembled from the stage output.
//!
//! The stage sources are read relative to the current directory, trying the
//! package-local `kel/...` path then the repo-root `compiler/kel/...` path, so the
//! commands work from either the subproject or the workspace root.

use keleusma::bytecode::{ConstValue, Module, Op, Value};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
use keleusma::{Arena, compiler::compile, lexer::tokenize, parser::parse};

/// Read a stage source, trying the package-local then the repo-root path.
fn read_stage(rel: &str) -> String {
    std::fs::read_to_string(rel)
        .or_else(|_| std::fs::read_to_string(format!("compiler/{rel}")))
        .unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

/// Compile a stage or program source with the Rust-hosted reference compiler.
pub fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

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
    guard: Vec<(i64, i64)>,
    body: Vec<(i64, i64)>,
}

const KINDS: usize = 1;

const ARGS: usize = 1 + 1024;

const LHS: usize = 1 + 1024 * 2;

const RHS: usize = 1 + 1024 * 3;

const CALL_ARGS: usize = 1 + 1024 * 4;

const FOR_PARTS: usize = 1 + 1024 * 4 + 64;

const MATCH_PARTS: usize = 1 + 1024 * 4 + 64 * 2;

const LIMIT_PARTS: usize = 1 + 1024 * 4 + 64 * 3;

const HEAD_PARTS: usize = 1 + 1024 * 4 + 64 * 4;

const PARAM_COUNT: usize = 1 + 1024 * 4 + 64 * 5;

const CATEGORY: usize = 1 + 1024 * 4 + 64 * 5 + 1;

const BR_LEX_ISTART: usize = 1 + 73728;

const BR_LEX_ILEN: usize = 1 + 73728 + 1280;

const BR_LEX_ICOUNT: usize = 1 + 73728 + 1280 + 1280;

const BR_P_LEN: usize = 0;

const BR_P_PACKED: usize = 1;

const BR_P_LIMIT_ID: usize = 1 + 12288;

const BR_P_CHUNK_COUNT: usize = 1 + 12288 + 1;

const BR_P_CHUNKS: usize = 1 + 12288 + 2;

const BR_P_REQUIRE_ID: usize = 1 + 12288 + 2 + 256;

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
                toks.push((t.rem_euclid(64), t.div_euclid(64)));
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

fn decode_op(w: i64) -> Op {
    let (tag, operand) = (w % 64, w / 64);
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
        other => panic!("unknown op tag {other} (word {w})"),
    }
}

/// Drive the codegen; return its emitted ops, the constant pool it built, and the
/// local-frame size (`local_count`) it computed.
fn run_codegen(body: &Body, param_count: usize) -> (Vec<Op>, Vec<i64>, i64) {
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

    // Phase 2: the pool the stage built. Size, then that many raw values.
    let count = next_word(&mut vm, &mut shared);
    let pool = (0..count)
        .map(|_| next_word(&mut vm, &mut shared))
        .collect();
    // Phase 3: the local-frame size the stage computed.
    let local_count = next_word(&mut vm, &mut shared);
    (ops, pool, local_count)
}

/// Drive lexer.kel then parse.kel over `src` and return every function it yields, each
/// with its guard and body records, plus the interned-name table. Multiheaded functions
/// appear as several same-named entries in declaration order.
pub fn parse_functions(src: &str) -> (Vec<ParsedFn>, Vec<String>) {
    let (tokens, names) = br_lex(src);
    let id_of = |s: &str| {
        names
            .iter()
            .position(|n| n == s)
            .map(|i| i as i64)
            .unwrap_or(-1)
    };
    // The chunk table must be in the module's actual chunk order so a resolved call index
    // matches the assembled module. The Rust compiler orders chunks by name, not by
    // declaration, so the table (host-supplied resolved-reference data) is taken from the
    // reference module's chunk order rather than the token scan's declaration order.
    let reference_chunks = compile_src(src);
    let chunks: Vec<i64> = reference_chunks
        .chunks
        .iter()
        .map(|c| id_of(&c.name))
        .collect();
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
    vm.set_shared(
        &mut shared,
        BR_P_CHUNK_COUNT,
        Value::Int(chunks.len() as i64),
    )
    .unwrap();
    for (i, &c) in chunks.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_CHUNKS + i, Value::Int(c))
            .unwrap();
    }
    for (i, &(k, v)) in tokens.iter().enumerate() {
        vm.set_shared(&mut shared, BR_P_PACKED + i, Value::Int(k + v * 64))
            .unwrap();
    }

    let mut fns: Vec<ParsedFn> = Vec::new();
    let mut cur: Option<ParsedFn> = None;
    let (mut in_body, mut in_guard, mut in_data, mut in_enum, mut in_use) =
        (false, false, false, false, false);
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(tokens.len() * 16 + 256) {
        if let VmState::Yielded(Value::Int(w)) = state {
            let (code, val) = (w.rem_euclid(64), w.div_euclid(64));
            if in_body {
                match code {
                    0 => {}
                    15 => in_body = false,
                    _ => cur.as_mut().unwrap().body.push((code, val)),
                }
            } else if in_guard {
                match code {
                    0 => {}
                    15 => in_guard = false,
                    _ => cur.as_mut().unwrap().guard.push((code, val)),
                }
            } else if in_data {
                in_data = code != 5;
            } else if in_enum {
                in_enum = code != 5;
            } else if in_use {
                in_use = code != 5;
            } else {
                match code {
                    1..=3 => {
                        cur = Some(ParsedFn {
                            cat: code,
                            name: val,
                            params: 0,
                            guard: Vec::new(),
                            body: Vec::new(),
                        })
                    }
                    4 => cur.as_mut().unwrap().params += 1,
                    9 => in_data = true,
                    10 => in_use = true,
                    12 => in_enum = true,
                    16 => in_body = true,
                    17 => in_guard = true,
                    5 => fns.push(cur.take().unwrap()),
                    15 => return (fns, names),
                    _ => {}
                }
            }
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("parse.kel did not reach DONE");
}

/// Self-host-compile a whole program: drive the pipeline over every function, reconstruct
/// each into its codegen Body (grouping same-named heads into one multihead), run
/// codegen.kel, and splice the self-hosted ops, constant pool, and local_count into the
/// reference module chunk of that name. Native chunks (absent from the source) keep the
/// reference's ops. The result is a runnable module whose every source-defined chunk was
/// emitted by the self-hosted pipeline.
pub fn self_host_compile(src: &str) -> Module {
    let (fns, names) = parse_functions(src);
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
        // A yield head compiles as a multihead chunk; a fn or loop as a single body.
        // The reconstruction runs through the self-hosted reconstruct.kel stage, so the
        // whole compile path is Keleusma and the host only moves data between stages.
        let body = if group[0].cat == 2 {
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
        module.chunks[idx].constants = pool.iter().map(|&v| ConstValue::Int(v)).collect();
        module.chunks[idx].local_count = lc as u16;
    }
    module
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
const RC_AST_MATCH_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 2;
const RC_AST_LIMIT_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 3;
const RC_AST_HEAD_PARTS: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 4;
const RC_AST_CATEGORY: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 5 + 1;
const RC_HEAD_COUNT: usize = RC_AST_BASE + 1 + 1024 * 4 + 64 * 5 + 2;
const RC_HEAD_GUARD_START: usize = RC_HEAD_COUNT + 1;
const RC_HEAD_GUARD_LEN: usize = RC_HEAD_COUNT + 1 + 16;
const RC_HEAD_BODY_START: usize = RC_HEAD_COUNT + 1 + 16 * 2;
const RC_HEAD_BODY_LEN: usize = RC_HEAD_COUNT + 1 + 16 * 3;

/// Drive reconstruct.kel over one function's postorder records and read back the
/// reconstructed forest as a `Body`. This increment reads only the node arrays and
/// the root/category; the side arrays (call/for/match) arrive with those kinds.
// Compile reconstruct.kel once and clone the module per call: `self_host_compile`
// drives it for every function, so recompiling each time dominates the runtime.
fn reconstruct_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/reconstruct.kel")))
        .clone()
}

fn reconstruct_via_kel(records: &[(i64, i64)], category: i64, param_count: usize) -> Body {
    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
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
    // Read each 64-entry side array in full; the caller compares only the prefix the
    // Rust reconstruction populated.
    let read_side = |vm: &Vm<'_, '_>, shared: &[u8], base: usize| -> Vec<i64> {
        (0..64).map(|k| rd(vm, shared, base + k)).collect()
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
fn reconstruct_via_kel_multihead(heads: &[&ParsedFn], pc: usize) -> Body {
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

    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &mut Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&mut vm, &mut shared, RC_REC_COUNT, recs.len() as i64);
    set(&mut vm, &mut shared, RC_IN_CATEGORY, 3);
    set(&mut vm, &mut shared, RC_IN_PARAM, pc as i64);
    for (i, &(k, a)) in recs.iter().enumerate() {
        set(&mut vm, &mut shared, RC_REC_KIND + i, k);
        set(&mut vm, &mut shared, RC_REC_ARG + i, a);
    }
    set(&mut vm, &mut shared, RC_HEAD_COUNT, heads.len() as i64);
    for h in 0..heads.len() {
        set(&mut vm, &mut shared, RC_HEAD_GUARD_START + h, gs[h] as i64);
        set(&mut vm, &mut shared, RC_HEAD_GUARD_LEN + h, gl[h] as i64);
        set(&mut vm, &mut shared, RC_HEAD_BODY_START + h, bs[h] as i64);
        set(&mut vm, &mut shared, RC_HEAD_BODY_LEN + h, bl[h] as i64);
    }
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
