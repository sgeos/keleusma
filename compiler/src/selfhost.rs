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
//! The [`self_host_compile`] entry splices only the self-hosted chunk ops onto the
//! reference scaffold, whereas [`self_host_compile_full`] additionally assembles the
//! module scaffold (data layout, enum-layout table, chunk signatures, schema hash, and
//! declared WCET/WCMU header) from the pipeline output, so for the loop-free stage
//! sources its serialized module is byte-identical to the reference (see
//! `tests/scaffold.rs`) without borrowing any field from it.
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
    // The type-name id of each value parameter (from the header PTYPE records) and of
    // the return (the RETTYPE record), for the driver's own chunk-signature assembly.
    param_types: Vec<i64>,
    return_type: i64,
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

/// Drive lexer.kel then parse.kel over `src` and return every function it yields, each
/// with its guard and body records, plus the interned-name table. Multiheaded functions
/// appear as several same-named entries in declaration order.
// The 4-tuple return carries the parsed functions, name table, and the raw data and
// enum record streams; factoring each into a `type` alias would only scatter it, so
// allow the complexity lint here as the root test file does file-wide.
#[allow(clippy::type_complexity)]
pub fn parse_functions(
    src: &str,
) -> (Vec<ParsedFn>, Vec<String>, Vec<(i64, i64)>, Vec<(i64, i64)>) {
    let (tokens, names) = br_lex(src);
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
    // Every data block's header records (DSTART, then PARAM/PTYPE/ASIZE per field, then
    // END), concatenated in declaration order, for the driver's own data-layout assembly.
    let mut data_records: Vec<(i64, i64)> = Vec::new();
    // Every enum's header records (ENUMSTART, then EVARIANT/EDISC per variant, then END),
    // for the driver's own enum-layout assembly.
    let mut enum_records: Vec<(i64, i64)> = Vec::new();
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
                        })
                    }
                    4 => cur.as_mut().unwrap().params += 1,
                    6 => cur.as_mut().unwrap().param_types.push(val),
                    7 => cur.as_mut().unwrap().return_type = val,
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
                    5 => fns.push(cur.take().unwrap()),
                    15 => return (fns, names, data_records, enum_records),
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
    let (fns, names, _data_records, _enum_records) = parse_functions(src);
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
const WA_CLASS: usize = 8 + 1024;
const WA_ARG: usize = 8 + 1024 * 2;
const WA_GROWTH: usize = 8 + 1024 * 3;
const WA_SHRINK: usize = 8 + 1024 * 4;
const WA_HEAP: usize = 8 + 1024 * 5;
const WA_OPK: usize = 8 + 1024 * 6;
const WA_SLOT: usize = 8 + 1024 * 7;
const WA_CVAL: usize = 8 + 1024 * 8;
const WA_CINT: usize = 8 + 1024 * 9;
const WA_CALLEE_SLOTS: usize = 8 + 1024 * 10;
const WA_CALLEE_HEAP: usize = 8 + 1024 * 11;
const WA_OUT_WCET: usize = 8 + 1024 * 12;
const WA_OUT_STACK: usize = 8 + 1024 * 12 + 1;
const WA_OUT_HEAP: usize = 8 + 1024 * 12 + 2;
const WA_OUT_REJECT: usize = 8 + 1024 * 12 + 3;
const WA_OUT_VALID: usize = 8 + 1024 * 12 + 4;

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
fn analyze_class(op: &keleusma::bytecode::Op) -> (i64, i64) {
    use keleusma::bytecode::Op;
    match op {
        Op::If(t) => (1, *t as i64),
        Op::Else(e) => (2, *e as i64),
        Op::EndIf => (3, 0),
        Op::Loop(x) => (4, *x as i64),
        Op::EndLoop(t) => (5, *t as i64),
        Op::Break(t) => (6, *t as i64),
        Op::BreakIf(t) => (7, *t as i64),
        Op::Trap(_) => (8, 0),
        Op::Call(_, _) => (9, 0),
        _ => (0, 0),
    }
}

/// Fine-grained op detail for analyze.kel's loop-bound extraction: `(opk, slot, cval, cint)`.
/// `opk` tags the opcode (1 GetLocal, 2 SetLocal, 3 Const, 4 CmpGe, 5 BreakIf, 6 CheckedAdd,
/// 7 PopN, 8 EndLoop, 9 Loop, 0 other); `slot` the GetLocal/SetLocal slot; `cval` the Const
/// integer value or PopN count; `cint` 1 if a Const resolves to an integer.
fn analyze_opk(
    op: &keleusma::bytecode::Op,
    chunk: &keleusma::bytecode::Chunk,
) -> (i64, i64, i64, i64) {
    use keleusma::bytecode::{ConstValue, Op};
    match op {
        Op::GetLocal(s) => (1, *s as i64, 0, 0),
        Op::SetLocal(s) => (2, *s as i64, 0, 0),
        Op::Const(idx) => match chunk.constants.get(*idx as usize) {
            Some(ConstValue::Int(v)) => (3, 0, *v, 1),
            _ => (3, 0, 0, 0),
        },
        Op::CmpGe => (4, 0, 0, 0),
        Op::BreakIf(_) => (5, 0, 0, 0),
        Op::CheckedAdd => (6, 0, 0, 0),
        Op::PopN(n) => (7, 0, *n as i64, 0),
        Op::EndLoop(_) => (8, 0, 0, 0),
        Op::Loop(_) => (9, 0, 0, 0),
        _ => (0, 0, 0, 0),
    }
}

/// The operand-stack `(growth, shrink)` analyze.kel accounts for `op` under the empty
/// resolver. Identical to `Op::stack_growth()`/`stack_shrink()` except for a native call: the
/// reference WCMU native arm uses `during_peak = offset + 1` and `offset += 1 - n` with `n`
/// the whole argument-count byte (the error-reify high bit included), which the generic
/// accounting reproduces exactly as `growth = 1, shrink = n_full_byte`.
fn analyze_stack_effect(op: &keleusma::bytecode::Op) -> (i64, i64) {
    use keleusma::bytecode::Op;
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
    op: &keleusma::bytecode::Op,
    chunk: &keleusma::bytecode::Chunk,
    shared_layout: &[keleusma::bytecode::SharedSlotLayout],
) -> i64 {
    use keleusma::bytecode::{Op, SHARED_SLOT_COMPOSITE_FLAG};
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
    chunk: &keleusma::bytecode::Chunk,
    arena_capacity: i64,
    chunk_wcmu: &[(i64, i64)],
    shared_layout: &[keleusma::bytecode::SharedSlotLayout],
) -> (i64, i64, i64, bool, bool) {
    use keleusma::bytecode::{BlockType, Op};
    let vsb = keleusma::bytecode::VALUE_SLOT_SIZE_BYTES as i64;
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
    assert!(chunk.ops.len() <= 1024, "analyze.kel op-table capacity");
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
pub fn analyze_stream_chunk(chunk: &keleusma::bytecode::Chunk) -> (i64, i64, i64, bool, bool) {
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
// `calls_ay` (8); the arrays `class` (9..), `arg`, `opb`, `o1`, `o2`, `o3`, `mark` (each 1024
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
const SV_ARG: usize = 9 + 1024;
const SV_OPB: usize = 9 + 1024 * 2;
const SV_O1: usize = 9 + 1024 * 3;
const SV_O2: usize = 9 + 1024 * 4;
const SV_O3: usize = 9 + 1024 * 5;
const SV_MARK: usize = 9 + 1024 * 6;
const SV_OUT_REJECT: usize = 9 + 1024 * 7;

fn verify_structural_kel_module() -> Module {
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
fn structural_opbounds(op: &keleusma::bytecode::Op, module: &Module) -> (i64, i64, i64, i64) {
    use keleusma::bytecode::{NewCompositeOperand, Op, StructField};
    use keleusma::value_layout::CompositeKind;
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
fn structural_marker(op: &keleusma::bytecode::Op) -> i64 {
    use keleusma::bytecode::Op;
    match op {
        Op::Yield => 1,
        Op::Stream => 2,
        Op::Reset => 3,
        _ => 0,
    }
}

/// The block type as the stage's tag: 0 Func, 1 Reentrant, 2 Stream.
fn block_type_tag(chunk: &keleusma::bytecode::Chunk) -> i64 {
    use keleusma::bytecode::BlockType;
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
    chunk: &keleusma::bytecode::Chunk,
    always: &std::collections::BTreeSet<usize>,
) -> bool {
    assert!(
        chunk.ops.len() <= 1024,
        "verify_structural.kel op-table capacity"
    );
    let m = verify_structural_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_structural.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&vm, &mut shared, SV_OP_COUNT, chunk.ops.len() as i64);
    set(&vm, &mut shared, SV_LOCAL_COUNT, chunk.local_count as i64);
    set(
        &vm,
        &mut shared,
        SV_CONST_COUNT,
        chunk.constants.len() as i64,
    );
    set(
        &vm,
        &mut shared,
        SV_TEMPLATE_COUNT,
        chunk.struct_templates.len() as i64,
    );
    set(
        &vm,
        &mut shared,
        SV_DATA_LEN,
        data_layout_slot_count(module),
    );
    set(&vm, &mut shared, SV_NCHUNKS, module.chunks.len() as i64);
    set(
        &vm,
        &mut shared,
        SV_WORD_BITS,
        1i64 << module.word_bits_log2,
    );
    set(&vm, &mut shared, SV_BLOCK_TYPE, block_type_tag(chunk));
    // Whether the chunk delegates its yield to an always-yielding callee (the reference's
    // `calls_always_yielder`). Resolved from the marshalled always-yielding set.
    let calls_ay = chunk.ops.iter().any(
        |op| matches!(op, keleusma::bytecode::Op::Call(g, _) if always.contains(&(*g as usize))),
    );
    set(&vm, &mut shared, SV_CALLS_AY, i64::from(calls_ay));
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        set(&vm, &mut shared, SV_CLASS + i, class);
        set(&vm, &mut shared, SV_ARG + i, arg);
        let (opb, o1, o2, o3) = structural_opbounds(op, module);
        set(&vm, &mut shared, SV_OPB + i, opb);
        set(&vm, &mut shared, SV_O1 + i, o1);
        set(&vm, &mut shared, SV_O2 + i, o2);
        set(&vm, &mut shared, SV_O3 + i, o3);
        set(&vm, &mut shared, SV_MARK + i, structural_marker(op));
    }
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

// --- Self-hosted yield-coverage kernel (verify_yield.kel) and Pass 3 -------------------------
//
// verify_yield.kel decides whether every fall-through path of a chunk region passes through a
// Yield (or a Call delegating to an always-yielding chunk), reproducing the reference
// `analyze_yield_coverage`. Its shared block `yv` lays out `op_count` (0), `region_start` (1),
// `region_end` (2); the arrays `class` (3..), `arg`, `mark`, `cay` (each 1024 wide, where `cay`
// flags a Call to an always-yielding chunk); and the results `out_fell`, `out_hy`. The driver
// runs it in two orchestrations, both self-hosting what was the reference borrow: the
// always-yielding monotone fixpoint (over `[0, op_count)` per chunk) and the Stream productivity
// check (over `[stream_pos + 1, reset_pos)`).

const YV_OP_COUNT: usize = 0;
const YV_REGION_START: usize = 1;
const YV_REGION_END: usize = 2;
const YV_CLASS: usize = 3;
const YV_ARG: usize = 3 + 1024;
const YV_MARK: usize = 3 + 1024 * 2;
const YV_CAY: usize = 3 + 1024 * 3;
const YV_OUT_FELL: usize = 3 + 1024 * 4;
const YV_OUT_HY: usize = 3 + 1024 * 4 + 1;

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
    chunk: &keleusma::bytecode::Chunk,
    start: usize,
    end: usize,
    always: &std::collections::BTreeSet<usize>,
) -> (bool, bool) {
    use keleusma::bytecode::Op;
    assert!(
        chunk.ops.len() <= 1024,
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
    chunk: &keleusma::bytecode::Chunk,
    always: &std::collections::BTreeSet<usize>,
) -> bool {
    use keleusma::bytecode::{BlockType, Op};
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
// `op_count` (0); the arrays `class` (1..), `arg`, `dreq`, `dnet`, `is_term` (each 1024 wide);
// and the verdict `out_reject`. `dreq`/`dnet` are the reference `op_depth_effect` (the actual
// operand consumption, NOT the WCMU stack effect); `is_term` flags Trap/Return.

const DV_OP_COUNT: usize = 0;
const DV_CLASS: usize = 1;
const DV_ARG: usize = 1 + 1024;
const DV_DREQ: usize = 1 + 1024 * 2;
const DV_DNET: usize = 1 + 1024 * 3;
const DV_IS_TERM: usize = 1 + 1024 * 4;
const DV_OUT_REJECT: usize = 1 + 1024 * 5;

fn verify_depth_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/verify_depth.kel")))
        .clone()
}

/// Run verify_depth.kel over one chunk, returning whether any op underflows the operand stack
/// (the reference `verify_stack_depth`). Marshals the control-flow `(class, arg)` table via
/// `analyze_class`, the actual operand consumption `(dreq, dnet)` via `op_depth_effect`, and the
/// Trap/Return terminator flag.
pub fn depth_reject_chunk_via_kel(chunk: &keleusma::bytecode::Chunk) -> bool {
    use keleusma::bytecode::Op;
    assert!(
        chunk.ops.len() <= 1024,
        "verify_depth.kel op-table capacity"
    );
    let m = verify_depth_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_depth.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&vm, &mut shared, DV_OP_COUNT, chunk.ops.len() as i64);
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg) = analyze_class(op);
        let (req, net) = keleusma::verify::op_depth_effect(op, chunk);
        set(&vm, &mut shared, DV_CLASS + i, class);
        set(&vm, &mut shared, DV_ARG + i, arg);
        set(&vm, &mut shared, DV_DREQ + i, i64::from(req.max(0)));
        set(&vm, &mut shared, DV_DNET + i, i64::from(net));
        set(
            &vm,
            &mut shared,
            DV_IS_TERM + i,
            i64::from(matches!(op, Op::Trap(_) | Op::Return)),
        );
    }
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

/// Run the whole self-hosted structural verifier over a module: the block-nesting, branch-target,
/// operand-bounds, and block-type checks per chunk (verify_structural.kel), the productive-
/// divergence check for Stream chunks (verify_yield.kel), and the operand-stack depth-balance
/// check per chunk (verify_depth.kel). Together these are `verify()`'s per-chunk checks -- every
/// pass except the A.2.1 typed operand-stack pass. The module is rejected iff any chunk is,
/// mirroring `verify()`. The always-yielding set is computed by the self-hosted fixpoint.
pub fn structural_reject_module_via_kel(module: &Module) -> bool {
    let always = self_hosted_always_yielding(module);
    module.chunks.iter().any(|chunk| {
        structural_reject_chunk_via_kel(module, chunk, &always)
            || productivity_reject_via_kel(chunk, &always)
            || depth_reject_chunk_via_kel(chunk)
    })
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
// `typed_check_chunk`. Deferred residuals: the Call argument-vs-parameter check, the non-enum
// `NewComposite` packed-size check, and exact composite-kind compatibility. Still without the
// module-level data-layout validation (slice 2c), so not yet wired into
// `structural_reject_module_via_kel`.

const TV_OP_COUNT: usize = 0;
const TV_RESUME_TAG: usize = 1;
const TV_RESUME_SIZE: usize = 2;
const TV_EB_COUNT: usize = 3;
const TV_CLASS: usize = 4;
const TV_ARG: usize = 4 + 1024;
const TV_IS_TERM: usize = 4 + 1024 * 2;
const TV_TK: usize = 4 + 1024 * 3;
const TV_REQ: usize = 4 + 1024 * 4;
const TV_PROD: usize = 4 + 1024 * 5;
const TV_TA: usize = 4 + 1024 * 6;
const TV_TB: usize = 4 + 1024 * 7;
const TV_RET_TAG: usize = 4 + 1024 * 8;
const TV_RET_SIZE: usize = 4 + 1024 * 9;
const TV_SEED_TAG: usize = 4 + 1024 * 10;
const TV_SEED_SIZE: usize = 4 + 1024 * 10 + 256;
const TV_EB_VALS: usize = 4 + 1024 * 10 + 512;
const TV_OUT_REJECT: usize = 4 + 1024 * 10 + 512 + 64;

fn verify_typed_kel_module() -> Module {
    static CACHED: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    CACHED
        .get_or_init(|| compile_src(&read_stage("kel/verify_typed.kel")))
        .clone()
}

/// The byte size of a scalar-shaped constant (`const_abs` restricted to its scalar arms), or
/// `None` for a composite/unknown constant (which the reference leaves `Top`).
fn const_scalar_size(
    cv: Option<&keleusma::bytecode::ConstValue>,
    wb: usize,
    fb: usize,
) -> Option<i64> {
    use keleusma::bytecode::ConstValue;
    use keleusma::value_layout::ScalarKind;
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
    op: &keleusma::bytecode::Op,
    chunk: &keleusma::bytecode::Chunk,
    wb: usize,
    fb: usize,
) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
    use keleusma::bytecode::{
        ArrayElem, EnumField, NewCompositeOperand, Op, StructField, TupleField,
    };
    use keleusma::value_layout::CompositeKind;
    let (class, arg) = analyze_class(op);
    let is_term = i64::from(matches!(op, Op::Return | Op::Trap(_)));
    let (r, net) = keleusma::verify::op_depth_effect(op, chunk);
    let req = i64::from(r.max(0));
    let prod = i64::from((net + r.max(0)).max(0));
    // The shape transfer kind and its operands; everything not listed is generic (tk 0).
    let (tk, ta, tb): (i64, i64, i64) = match op {
        Op::Dup => (1, 0, 0),
        Op::IsEnum(_, _, _) | Op::IsStruct(_) => (2, 1, 0),
        Op::GetLocal(i) => (3, *i as i64, 0),
        Op::SetLocal(i) => (4, *i as i64, 0),
        Op::Yield => (11, 0, 0),
        Op::Call(_, _) | Op::CallVerifiedNative(_, _) | Op::CallExternalNative(_, _) => (12, 0, 0),
        Op::Const(idx) => match const_scalar_size(chunk.constants.get(*idx as usize), wb, fb) {
            Some(sz) => (2, sz, 0),
            None => (0, 0, 0),
        },
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Enum,
            byte_size,
            ..
        }) => (14, 0, *byte_size as i64),
        Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) => (6, 0, *byte_size as i64),
        Op::GetField(StructField::Flat { offset, kind })
        | Op::GetTupleField(TupleField::Flat { offset, kind })
        | Op::GetEnumField(EnumField::Flat { offset, kind }) => {
            (7, *offset as i64, kind.size_in_bytes(wb, fb) as i64)
        }
        Op::GetField(StructField::FlatNested { offset, size, .. })
        | Op::GetTupleField(TupleField::FlatNested { offset, size, .. })
        | Op::GetEnumField(EnumField::FlatNested { offset, size, .. }) => {
            (8, *offset as i64, *size as i64)
        }
        Op::GetIndex(ArrayElem::Flat { kind }) => (9, kind.size_in_bytes(wb, fb) as i64, 0),
        Op::GetIndex(ArrayElem::FlatNested { size, .. }) => (10, *size as i64, 0),
        _ => (0, 0, 0),
    };
    (class, arg, is_term, tk, req, prod, ta, tb)
}

/// Lift a wire signature shape into the stage's `(tag, size)` lattice, mirroring
/// `AbsVal::from_wire`: Top -> (0,0); a decodable scalar -> (1, byte size); a decodable flat
/// composite -> (2, byte size); an undecodable tag -> Top. The composite kind is not tracked
/// (size-only flat compatibility, a documented residual).
fn abs_from_wire(shape: &keleusma::bytecode::WireShape, wb: usize, fb: usize) -> (i64, i64) {
    use keleusma::bytecode::WireShape;
    use keleusma::value_layout::{CompositeKind, ScalarKind};
    match shape {
        WireShape::Top => (0, 0),
        WireShape::Scalar { kind } => match ScalarKind::from_tag(*kind) {
            Some(k) => (1, k.size_in_bytes(wb, fb) as i64),
            None => (0, 0),
        },
        WireShape::Flat { kind, size } => match CompositeKind::from_tag(*kind) {
            Some(_) => (2, *size as i64),
            None => (0, 0),
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
    chunk: &keleusma::bytecode::Chunk,
    sig: Option<&keleusma::bytecode::ChunkSignature>,
    wb: usize,
    fb: usize,
) -> bool {
    use keleusma::bytecode::Op;
    assert!(
        chunk.ops.len() <= 1024,
        "verify_typed.kel op-table capacity"
    );
    let m = verify_typed_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify verify_typed.kel");
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    let set = |vm: &Vm<'_, '_>, shared: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(shared, slot, Value::Int(v)).unwrap();
    };
    set(&vm, &mut shared, TV_OP_COUNT, chunk.ops.len() as i64);
    // Seed the local frame from the signature's parameters (leading slots) and the resume shape.
    if let Some(sig) = sig {
        for (i, param) in sig.params.iter().enumerate().take(256) {
            let (tag, size) = abs_from_wire(param, wb, fb);
            set(&vm, &mut shared, TV_SEED_TAG + i, tag);
            set(&vm, &mut shared, TV_SEED_SIZE + i, size);
        }
        let (rtag, rsize) = abs_from_wire(&sig.resume, wb, fb);
        set(&vm, &mut shared, TV_RESUME_TAG, rtag);
        set(&vm, &mut shared, TV_RESUME_SIZE, rsize);
        // The declared flat enum body sizes (`word_bytes + min_payload`), for the B8 cross-check.
        for (i, el) in module.enum_layouts.iter().enumerate().take(64) {
            set(
                &vm,
                &mut shared,
                TV_EB_VALS + i,
                wb as i64 + el.min_payload as i64,
            );
        }
        set(
            &vm,
            &mut shared,
            TV_EB_COUNT,
            module.enum_layouts.len().min(64) as i64,
        );
    }
    for (i, op) in chunk.ops.iter().enumerate() {
        let (class, arg, is_term, tk, req, prod, ta, tb) = typed_desc(op, chunk, wb, fb);
        assert!(prod <= 4, "verify_typed.kel push_tops unroll bound");
        set(&vm, &mut shared, TV_CLASS + i, class);
        set(&vm, &mut shared, TV_ARG + i, arg);
        set(&vm, &mut shared, TV_IS_TERM + i, is_term);
        set(&vm, &mut shared, TV_TK + i, tk);
        set(&vm, &mut shared, TV_REQ + i, req);
        set(&vm, &mut shared, TV_PROD + i, prod);
        set(&vm, &mut shared, TV_TA + i, ta);
        set(&vm, &mut shared, TV_TB + i, tb);
        // A Call/native return shape, seeded from the module tables (only in the seeded form;
        // isolation leaves it Top, matching `typed_check_chunk`'s empty tables).
        if sig.is_some() {
            let ret = match op {
                Op::Call(callee, _) => module
                    .signatures
                    .get(*callee as usize)
                    .map(|cs| abs_from_wire(&cs.ret, wb, fb)),
                Op::CallVerifiedNative(idx, _) | Op::CallExternalNative(idx, _) => module
                    .native_return_shapes
                    .get(*idx as usize)
                    .map(|w| abs_from_wire(w, wb, fb)),
                _ => None,
            };
            if let Some((tag, size)) = ret {
                set(&vm, &mut shared, TV_RET_TAG + i, tag);
                set(&vm, &mut shared, TV_RET_SIZE + i, size);
            }
        }
    }
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

/// Run verify_typed.kel over one chunk in isolation (no seeding), the drop-in for
/// `typed_check_chunk`.
pub fn typed_reject_chunk_via_kel(module: &Module, chunk: &keleusma::bytecode::Chunk) -> bool {
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

/// The self-hosted drop-in replacement for `verify_resource_bounds`: analyze.kel decides each
/// chunk's WCMU transitively (callee bodies folded at every `Op::Call`, resolved in
/// topological order so callees precede callers), and the module is admitted iff no chunk has
/// an inextractable bound and every Stream chunk's budget fits `arena_capacity`.
pub fn validate_module_via_kel(module: &Module, arena_capacity: i64) -> bool {
    use keleusma::bytecode::{BlockType, Op};
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
) -> Vec<keleusma::bytecode::DataSlot> {
    use keleusma::bytecode::{DataSlot, SlotVisibility};
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
                    for k in 0..count {
                        slots.push(DataSlot {
                            name: format!("{bname}.{fname}[{k}]"),
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
) -> Vec<keleusma::bytecode::SharedSlotLayout> {
    use keleusma::bytecode::SharedSlotLayout;
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
) -> Vec<keleusma::bytecode::ConstValue> {
    use keleusma::bytecode::ConstValue;
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
) -> keleusma::bytecode::DataLayout {
    keleusma::bytecode::DataLayout {
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
) -> Vec<keleusma::bytecode::EnumLayout> {
    use keleusma::bytecode::{EnumLayout, EnumVariantDisc};
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
fn wire_shape_of(type_id: i64, names: &[String]) -> keleusma::bytecode::WireShape {
    use keleusma::bytecode::WireShape;
    match names.get(type_id as usize).map(String::as_str) {
        Some("Word") => WireShape::Scalar { kind: 3 },
        Some("Byte") => WireShape::Scalar { kind: 2 },
        _ => WireShape::Top,
    }
}

/// Assemble the per-chunk signature table from the parsed functions, grouping
/// same-named heads into one chunk and ordering by chunk name to match the module.
fn assemble_signatures(
    fns: &[ParsedFn],
    names: &[String],
) -> Vec<keleusma::bytecode::ChunkSignature> {
    use keleusma::bytecode::{ChunkSignature, WireShape};
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
        if c.block_type != keleusma::bytecode::BlockType::Stream {
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
/// their [`keleusma::bytecode::TypeTag`] (a stage boundary carries only scalar parameters). A
/// multiheaded function is one chunk described by its first head.
#[allow(clippy::type_complexity)]
fn assemble_chunk_metadata(
    fns: &[ParsedFn],
    names: &[String],
) -> Vec<(
    String,
    u8,
    keleusma::bytecode::BlockType,
    Vec<keleusma::bytecode::TypeTag>,
)> {
    use keleusma::bytecode::{BlockType, TypeTag};
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
/// self-hosted chunk ops (via [`self_host_compile`]) plus a data layout, schema hash,
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
    let (fns, names, data_records, enum_records) = parse_functions(src);
    let dl = assemble_data_layout(&data_records, &names);
    module.schema_hash = keleusma::bytecode::compute_schema_hash(Some(&dl));
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
        keleusma::bytecode::FLAG_EPHEMERAL
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
fn shared_data_bytes_of(shared_layout: &[keleusma::bytecode::SharedSlotLayout]) -> u32 {
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
    use keleusma::bytecode::{Chunk, ConstValue, SlotVisibility};
    let (fns, names, data_records, enum_records) = parse_functions(src);

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
        let body = if group[0].cat == 2 {
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
            constants: pool.iter().map(|&v| ConstValue::Int(v)).collect(),
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
    let native_return_shapes: Vec<keleusma::bytecode::WireShape> = Vec::new();

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
        private_slot_count.saturating_mul(keleusma::bytecode::VALUE_SLOT_SIZE_BYTES);
    let schema_hash = keleusma::bytecode::compute_schema_hash(Some(&dl));

    let enum_layouts = assemble_enum_layouts(&enum_records, &names);
    let signatures = assemble_signatures(&fns, &names);

    // The scalar bit widths are the host target's, matching the reference `compile`, which
    // compiles with `Target::host()`.
    let target = keleusma::target::Target::host();

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
