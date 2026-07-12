//! The self-hosted compile pipeline as a reusable library.
//!
//! This ports the reconstruction bridge and stage drivers that prove the
//! self-hosted stages self-compile (`../tests/selfhost_codegen.rs` in the parent
//! `keleusma` crate) into the compiler subproject, so the `compile` command and the
//! driver-level fixed-point test share one implementation. It drives
//! `kel/lexer.kel` and `kel/parse.kel` over a source, reconstructs each function's
//! (kind, arg, lhs, rhs) node forest from parse.kel's postorder record stream, and
//! drives `kel/codegen.kel` to emit each chunk's ops, splicing them into a module
//! whose data layout and chunk table come from the Rust-hosted reference compiler
//! (`compile_src`). Two limitations remain on the road to full self-hosting, both
//! documented in `MILESTONES.md`: the reconstruction is host-side Rust rather than a
//! Keleusma stage, and the module scaffold (data layout, constant-pool metadata,
//! auxiliary body) is taken from the reference rather than assembled from the stages.
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

/// Rebuild the codegen node forest from parse.kel's postorder record stream with a
/// node-index stack, the inverse of the reference flatten. A leaf (Literal 1,
/// Local 2, Unit 20) pushes itself. A binary node pops its right then left child:
/// the operators (BinOp 3, Andalso 8, Orelse 9), and the block-fold statements
/// LetIn (5, whose `arg` is the local slot) and ExprStmt (21), whose left child is
/// the value and right child the continuation. A unary operator (Not 6, Neg 10)
/// pops its single operand into `lhs`. An If (4) pops its else, then, and cond
/// children and takes the cond node index as its `arg` (the record's `arg` is 0).
/// The root is the one node left on the stack. The data-access kinds fit the same
/// groups: a scalar DataRead (11) is a leaf carrying its slot; a DataAssign (12)
/// folds like a LetIn carrying its slot; and an IndexRead (13) is unary over the
/// index, carrying `base + len*2^24` in `arg`. A Call (7) packs `chunk + count*256`
/// in its record `arg`; it pops its `count` argument nodes, reverses them to source
/// order into the `call_args` side array, and takes that slice's start as `lhs` and
/// the count as `rhs`. An indexed write pairs an IndexStore signal (36), which
/// folds the value and index into a kind-15 node (value in lhs, index in rhs),
/// with an IndexAssign (14) that folds like a LetIn carrying `base + len*2^24`.
/// A `for .. limit` loop streams its start, end, body, and the four literals cap/
/// 0/1/2 as nodes, then five SlotRecords (32) carrying its frame slots, then a
/// ForBuild (33) that assembles the 12-word limit_parts entry (the five slots then
/// the start, end, body, cap, 0, 1, 2 node indices); the ForLimit statement (23)
/// folds into the block popping only the continuation. A `match` streams its
/// scrutinee, a (literal, result) pair per literal arm, and the wildcard result,
/// then a MatchBuild (34) packing `temp*1024 + lit_count`, which assembles the
/// match_parts entry (temp, wildcard, then the pairs) and builds the MatchIn node.
/// A YieldExpr (24) is unary, holding the yielded expression in `lhs`. This
/// increment covers the arithmetic, comparison, bitwise, unary, short-circuit,
/// block, if, scalar and indexed data-access, call, for-limit, integer-match, and
/// yield kinds; a record of any other kind is rejected until a later increment adds
/// it (the multiheaded dispatch and its head_parts remain).
fn reconstruct_body(records: &[(i64, i64)], category: i64) -> Body {
    let mut nodes: Vec<Node> = Vec::new();
    let mut call_args: Vec<i64> = Vec::new();
    let mut limit_parts: Vec<i64> = Vec::new();
    let mut match_parts: Vec<i64> = Vec::new();
    let root = reconstruct_into(
        records,
        &mut nodes,
        &mut call_args,
        &mut limit_parts,
        &mut match_parts,
    );
    Body {
        nodes,
        call_args,
        for_parts: Vec::new(),
        match_parts,
        limit_parts,
        head_parts: Vec::new(),
        category,
        root,
    }
}

/// Reconstruct one postorder record forest into the shared `nodes` array (and the
/// shared side arrays), returning the root node index. Node indices are absolute in
/// `nodes`, so several forests -- a multihead's per-head guards and bodies -- can be
/// reconstructed into one array and referenced by `head_parts`.
fn reconstruct_into(
    records: &[(i64, i64)],
    nodes: &mut Vec<Node>,
    call_args: &mut Vec<i64>,
    limit_parts: &mut Vec<i64>,
    match_parts: &mut Vec<i64>,
) -> i64 {
    let mut stack: Vec<i64> = Vec::new();
    let mut pending_slots: Vec<i64> = Vec::new();
    for &(kind, arg) in records {
        // A SlotRecord (32) carries a frame slot for the pending `for` loop, and a
        // ForBuild (33) assembles the 12-word limit_parts entry from the five
        // collected slots and the seven loop nodes on the stack. Neither is a node.
        if kind == 32 {
            pending_slots.push(arg);
            continue;
        }
        if kind == 33 {
            let two = stack.pop().expect("for two");
            let one = stack.pop().expect("for one");
            let zero = stack.pop().expect("for zero");
            let cap = stack.pop().expect("for cap");
            let body = stack.pop().expect("for body");
            let end = stack.pop().expect("for end");
            let start = stack.pop().expect("for start");
            assert_eq!(pending_slots.len(), 5, "a for loop has five SlotRecords");
            limit_parts.extend([
                pending_slots[0],
                pending_slots[1],
                pending_slots[2],
                pending_slots[3],
                pending_slots[4],
                start,
                end,
                body,
                cap,
                zero,
                one,
                two,
            ]);
            pending_slots.clear();
            continue;
        }
        let idx = match kind {
            1 | 2 | 11 | 20 => {
                nodes.push(Node {
                    kind,
                    arg,
                    lhs: 0,
                    rhs: 0,
                });
                (nodes.len() - 1) as i64
            }
            3 | 5 | 8 | 9 | 12 | 14 | 21 => {
                let r = stack.pop().expect("binary rhs");
                let l = stack.pop().expect("binary lhs");
                nodes.push(Node {
                    kind,
                    arg,
                    lhs: l,
                    rhs: r,
                });
                (nodes.len() - 1) as i64
            }
            36 => {
                // IndexStore signal: fold the value and index already on the stack
                // into a kind-15 IndexStore node (value in lhs, index in rhs). The
                // record kind is 36 because 15 collides with the record DONE marker;
                // the node it produces is a real kind-15.
                let value = stack.pop().expect("index-store value");
                let index = stack.pop().expect("index-store index");
                nodes.push(Node {
                    kind: 15,
                    arg: 0,
                    lhs: value,
                    rhs: index,
                });
                (nodes.len() - 1) as i64
            }
            23 => {
                // ForLimit statement: arg is the limit_parts entry start (already
                // assembled at the ForBuild). It folds into the block popping only
                // the continuation; its lhs is unused.
                let cont = stack.pop().expect("forlimit continuation");
                nodes.push(Node {
                    kind: 23,
                    arg,
                    lhs: 0,
                    rhs: cont,
                });
                (nodes.len() - 1) as i64
            }
            34 => {
                // MatchBuild signal: arg packs `temp*1024 + lit_count`. The stack
                // holds the scrutinee, then a (literal, result) pair per literal arm,
                // then the wildcard result on top. Assemble the match_parts entry
                // (temp, wildcard, then the literal/result pairs) and build the
                // MatchIn node (kind 22): arg = entry start, lhs = scrutinee, rhs =
                // literal-arm count. Unlike the for loop, the build signal itself
                // produces the value node.
                let temp = arg.div_euclid(1024);
                let lit_count = arg.rem_euclid(1024);
                let wc = stack.pop().expect("match wildcard result");
                let mut pairs: Vec<(i64, i64)> = Vec::new();
                for _ in 0..lit_count {
                    let res = stack.pop().expect("match arm result");
                    let lit = stack.pop().expect("match arm literal");
                    pairs.push((lit, res));
                }
                pairs.reverse();
                let scrut = stack.pop().expect("match scrutinee");
                let base = match_parts.len() as i64;
                match_parts.push(temp);
                match_parts.push(wc);
                for (lit, res) in &pairs {
                    match_parts.push(*lit);
                    match_parts.push(*res);
                }
                nodes.push(Node {
                    kind: 22,
                    arg: base,
                    lhs: scrut,
                    rhs: lit_count,
                });
                (nodes.len() - 1) as i64
            }
            6 | 10 | 13 | 24 | 26 => {
                let c = stack.pop().expect("unary operand");
                nodes.push(Node {
                    kind,
                    arg,
                    lhs: c,
                    rhs: 0,
                });
                (nodes.len() - 1) as i64
            }
            4 => {
                let el = stack.pop().expect("if else");
                let th = stack.pop().expect("if then");
                let cond = stack.pop().expect("if cond");
                nodes.push(Node {
                    kind: 4,
                    arg: cond,
                    lhs: th,
                    rhs: el,
                });
                (nodes.len() - 1) as i64
            }
            7 => {
                // Call: arg packs `chunk + count*256`. Pop the count argument node
                // indices (last-pushed is the last argument), reverse them to source
                // order, and append them to the packed call_args side array. The Call
                // node's lhs is that slice's start and rhs the argument count.
                let chunk = arg.rem_euclid(256);
                let count = arg.div_euclid(256);
                let mut popped: Vec<i64> =
                    (0..count).map(|_| stack.pop().expect("call arg")).collect();
                popped.reverse();
                let args_start = call_args.len() as i64;
                call_args.extend(popped);
                nodes.push(Node {
                    kind: 7,
                    arg: chunk,
                    lhs: args_start,
                    rhs: count,
                });
                (nodes.len() - 1) as i64
            }
            other => panic!("reconstruct_body: unsupported node kind {other}"),
        };
        stack.push(idx);
    }
    assert_eq!(stack.len(), 1, "exactly one root node remains");
    stack[0]
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

/// Combine the heads of a multiheaded function into the codegen Body codegen.kel's
/// multihead dispatch consumes. Each head's guard and body are reconstructed into one
/// shared node array, its frame slots remapped by the head's `param_start` -- the heads'
/// parameter copies occupy `(i+1)*pc .. (i+2)*pc` (this covers heads whose only frame
/// slots are the parameters; heads with their own lets or loops need the additional
/// per-head temp offset, a later refinement). Each head contributes a four-word
/// head_parts entry (param_start, guarded, guard node, body node), and a MultiHead node
/// (kind 25, rhs = head count) is the root; the category is 3.
fn build_multihead_bridge(heads: &[&ParsedFn], pc: usize) -> Body {
    let mut nodes: Vec<Node> = Vec::new();
    let mut call_args: Vec<i64> = Vec::new();
    let mut limit_parts: Vec<i64> = Vec::new();
    let mut match_parts: Vec<i64> = Vec::new();
    let mut head_parts: Vec<i64> = Vec::new();
    let mut entries: Vec<(i64, i64, i64, i64)> = Vec::new();
    for (i, h) in heads.iter().enumerate() {
        let base = ((i + 1) * pc) as i64;
        // Remap frame-slot references (Local) into this head's parameter window.
        let remap = |recs: &[(i64, i64)]| -> Vec<(i64, i64)> {
            recs.iter()
                .map(|&(k, a)| if k == 2 { (k, a + base) } else { (k, a) })
                .collect()
        };
        let (guarded, guard_node) = if h.guard.is_empty() {
            (0i64, 0i64)
        } else {
            let g = remap(&h.guard);
            let root = reconstruct_into(
                &g,
                &mut nodes,
                &mut call_args,
                &mut limit_parts,
                &mut match_parts,
            );
            (1i64, root)
        };
        let b = remap(&h.body);
        let body_node = reconstruct_into(
            &b,
            &mut nodes,
            &mut call_args,
            &mut limit_parts,
            &mut match_parts,
        );
        entries.push((base, guarded, guard_node, body_node));
    }
    for (ps, g, gn, bn) in &entries {
        head_parts.push(*ps);
        head_parts.push(*g);
        head_parts.push(*gn);
        head_parts.push(*bn);
    }
    nodes.push(Node {
        kind: 25,
        arg: 0,
        lhs: 0,
        rhs: heads.len() as i64,
    });
    let root = (nodes.len() - 1) as i64;
    Body {
        nodes,
        call_args,
        for_parts: Vec::new(),
        match_parts,
        limit_parts,
        head_parts,
        category: 3,
        root,
    }
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
        let body = if group[0].cat == 2 {
            build_multihead_bridge(&group, pc)
        } else {
            let category = if group[0].cat == 3 { 2 } else { 0 };
            reconstruct_body(&group[0].body, category)
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
