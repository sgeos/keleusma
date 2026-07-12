//! Driver and bootstrap harness for the self-hosted Keleusma compiler (V0.3.0).
//!
//! This is scaffolding. The three pipeline stages live in Keleusma source under
//! `kel/`. The codegen and parser stages are implemented (the parser as the merged
//! `parse.kel`, which parses a whole declaration including its body in one pass); this
//! driver establishes the structure that will register the `compiler::` natives, drive
//! the yield/resume pipeline, run the bootstrap phases, and validate byte-identical
//! output against the Rust-hosted reference compiler. See `README.md` and `MILESTONES.md`;
//! the authoritative design is `docs/roadmap/V0_3_0_SELF_HOSTING.md`.

/// The bytecode format the self-hosted compiler must emit. Sourced from the parent
/// runtime so the two compilers cannot drift on the wire format.
const TARGET_BYTECODE_VERSION: u16 = keleusma::bytecode::BYTECODE_VERSION;

/// The three pipeline stages, in migration order (roadmap Steps 1, 2, 3). The parser
/// stage is the merged `parse.kel`, which parses a whole top-level declaration including
/// its function body in one pass; the earlier split `parser.kel` and body `body.kel`
/// loops are retained as the reference implementations and broader test coverage until
/// `parse.kel` is composed into an end-to-end pipeline with matching coverage.
const STAGES: &[(&str, &str)] = &[
    ("lexer", "kel/lexer.kel"),
    ("parser", "kel/parse.kel"),
    ("codegen", "kel/codegen.kel"),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("status");
    match cmd {
        "status" => status(),
        "bootstrap" => not_yet(
            "bootstrap",
            "cross-compile (Phase A), self-compile to fixed point (Phases B, C)",
        ),
        "verify-corpus" => not_yet(
            "verify-corpus",
            "compile the regression corpus under kelc.1 and assert byte-identity with the Rust-hosted compiler",
        ),
        "lex" => match args.get(1) {
            Some(path) => run_lexer(path),
            None => {
                eprintln!("usage: keleusma-selfhost lex <file>");
                std::process::exit(2);
            }
        },
        "parse" => match args.get(1) {
            Some(path) => run_parse_pipeline(path),
            None => {
                eprintln!("usage: keleusma-selfhost parse <file>");
                std::process::exit(2);
            }
        },
        "compile" => not_yet(
            cmd,
            "run the whole lexer/parser/codegen pipeline to bytecode",
        ),
        "-h" | "--help" | "help" => help(),
        other => {
            eprintln!("unknown command: {other}");
            help();
            std::process::exit(2);
        }
    }
}

fn status() {
    println!("keleusma-selfhost — the self-hosted Keleusma compiler (V0.3.0 goal)");
    println!("targets BYTECODE_VERSION = {TARGET_BYTECODE_VERSION}");
    println!();
    println!("pipeline stages (migration order):");
    for (i, (name, path)) in STAGES.iter().enumerate() {
        // A stage is a "skeleton" while every non-blank line is a comment; it is
        // "in progress" once it holds real Keleusma code.
        let state = match std::fs::read_to_string(path) {
            Err(_) => "absent",
            Ok(text) => {
                let has_code = text
                    .lines()
                    .map(str::trim)
                    .any(|l| !l.is_empty() && !l.starts_with("//"));
                if has_code { "in progress" } else { "skeleton" }
            }
        };
        println!("  step {}: {name:<8} {path:<20} [{state}]", i + 1);
    }
    println!();
    println!("status: porting backward, codegen first. Codegen increment 32, a");
    println!("recursion-free work-stack walk (a `loop` that delegates its yield to a");
    println!("multiheaded guarded `yield` phase machine over per-kind subroutines)");
    println!("that compiles blocks of `let`, data-field assignments, and range");
    println!("`for` loops (including the bounded-runtime-range `for .. limit` form)");
    println!("over the binary integer arithmetic set (+ - * / %), the six");
    println!("comparison operators, unary `not` and `-`, the bitwise band/bor/bxor,");
    println!("short-circuit `andalso`/`orelse`, `if`/`else` structured control flow");
    println!("with stage-resolved jump targets, `match` over an integer scrutinee,");
    println!("function calls, and scalar and");
    println!("indexed data-segment reads and writes, into an op buffer it streams with its own");
    println!("deduplicating constant pool and counted local-frame size, and lexer");
    println!("increment 5, a streaming tokenizer whose output is now the parser's input");
    println!("stream directly -- every token carries its unified Tok code, with maximal");
    println!("munch over the two-byte operators, keyword classification, identifier");
    println!("interning to stable ids, and the single-byte punctuation, `->` arrow, and");
    println!("lone `_` mapped to their Tok codes -- both compile, verify, and run (see");
    println!("tests/selfhost_codegen.rs and `lex <file>`). The codegen stage is now");
    println!("FULLY SELF-HOSTING: all 34 of its functions, including the multiheaded");
    println!("`yield emit_next` dispatch and the `loop main`, compile themselves");
    println!("byte-identically. The parser stage (Step 2) is now the merged parse.kel,");
    println!("which parses a whole top-level declaration -- a function with its full");
    println!("body, a data block, an enum, or a use import -- in a single pass, folding");
    println!("body.kel's node-forest walk into parser.kel's declaration scan and");
    println!("resolving data fields and enums by accumulating their tables as it parses");
    println!("(see tests/selfhost_parse.rs). The lexer and parser stages now COMPOSE: the");
    println!("`parse <file>` command tokenizes with lexer.kel, recovers the parser's");
    println!("inputs from the lexer's exposed intern table and a scan of its token stream,");
    println!("drives parse.kel, and prints the declarations, with no runtime-tokenizer");
    println!("adapter in the path (see tests/selfhost_pipeline.rs). The remaining pipeline");
    println!("work is composing the parser's node forest into codegen.kel to emit");
    println!("bytecode. V0.3.0 ships when the bootstrap reaches a fixed point.");
}

/// Run Stage 1 (the self-hosted lexer) over `path` and print the token stream.
///
/// This drives `kel/lexer.kel` on the current runtime: it compiles the lexer,
/// places the input source in the loop's `shared data` byte array, and resumes
/// it, decoding the unified `tok + payload*64` token wire (increment 5), where
/// `tok` is the parser's Tok discriminant and 63/62 are the PENDING/EOF markers.
/// It is the first end-to-end proof that the host can drive a self-hosted pipeline
/// stage, and the point at which the lexer's output becomes the parser's input.
fn run_lexer(path: &str) {
    use keleusma::Arena;
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

    // The lexer source lives beside this binary's package; try the package-local
    // path first, then the repo-root path, so the command works from either.
    let lexer_src = std::fs::read_to_string("kel/lexer.kel")
        .or_else(|_| std::fs::read_to_string("compiler/kel/lexer.kel"))
        .unwrap_or_else(|e| {
            eprintln!("cannot read kel/lexer.kel: {e}");
            std::process::exit(1);
        });
    let input = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1);
    });

    let module = keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(&lexer_src).expect("lex lexer.kel"))
            .expect("parse lexer.kel"),
    )
    .expect("compile lexer.kel");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize arena");
    let mut vm = Vm::new(module, &arena).expect("verify lexer.kel");

    let cap = vm.shared_data_bytes();
    // The increment-1 lexer holds up to 4096 source bytes in its shared array.
    if input.len() > 4096 {
        eprintln!(
            "input is {} bytes; the increment-1 lexer caps source at 4096",
            input.len()
        );
        std::process::exit(1);
    }
    let mut shared = vec![0u8; cap];
    vm.set_shared(&mut shared, 0, Value::Int(input.len() as i64))
        .expect("set len");
    for (i, &byte) in input.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(byte))
            .expect("set byte");
    }

    println!("tokens for {path} ({} bytes):", input.len());
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    // A generous per-byte iteration budget; the loop is productive-divergent.
    for _ in 0..(input.len() * 4 + 16) {
        match state {
            VmState::Yielded(Value::Int(t)) => {
                // Unified wire (increment 5): `tok + payload*64`, with 63 PENDING
                // (skipped) and 62 EOF above the Tok range.
                if t == 63 {
                    // PENDING; the host skips it.
                } else if t == 62 {
                    println!("  EOF");
                    return;
                } else {
                    let (tok, payload) = (t.rem_euclid(64), t.div_euclid(64));
                    let name = tok_name(tok);
                    if tok == 1 {
                        println!("  {name:<9} id {payload}");
                    } else if tok == 12 {
                        println!("  {name:<9} {payload}");
                    } else {
                        println!("  {name}");
                    }
                }
            }
            VmState::Reset => {}
            other => {
                eprintln!("unexpected VM state: {other:?}");
                std::process::exit(1);
            }
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    eprintln!("iteration budget exhausted before EOF");
    std::process::exit(1);
}

/// Name a parser `Tok` discriminant for the `lex` command's human-readable dump.
/// Mirrors the `Tok` enum in `kel/parse.kel`; 4 is the catch-all that absorbs the
/// `->` arrow and any unrecognized punctuation.
fn tok_name(tok: i64) -> &'static str {
    match tok {
        0 => "fn",
        1 => "Ident",
        2 => "LBrace",
        3 => "RBrace",
        4 => "catchall",
        5 => "yield",
        6 => "loop",
        7 => "LParen",
        8 => "RParen",
        9 => "Colon",
        10 => "Comma",
        12 => "IntLit",
        13 => "data",
        14 => "shared",
        15 => "private",
        16 => "const",
        17 => "Eq",
        19 => "use",
        21 => "Plus",
        22 => "Minus",
        23 => "Star",
        24 => "Slash",
        25 => "Percent",
        26 => "EqEq",
        27 => "NotEq",
        28 => "Lt",
        29 => "Gt",
        30 => "LtEq",
        31 => "GtEq",
        32 => "not",
        33 => "band",
        34 => "bor",
        35 => "bxor",
        36 => "andalso",
        37 => "orelse",
        38 => "let",
        39 => "Semi",
        40 => "Dot",
        41 => "LBracket",
        42 => "RBracket",
        43 => "if",
        44 => "else",
        45 => "for",
        46 => "in",
        47 => "DotDot",
        48 => "match",
        49 => "FatArrow",
        50 => "Underscore",
        51 => "ColCol",
        52 => "as",
        53 => "enum",
        _ => "?tok",
    }
}

/// Run Stages 1 and 2 composed: tokenize `path` with the self-hosted lexer, build
/// the parser's inputs from the lexer's output alone (the exposed intern table and
/// a brace-depth scan of the token stream), drive the self-hosted parser, and print
/// the declarations it yields. This is the first actual composition of two
/// self-hosted stages in the driver; the host only orchestrates the yield/resume
/// loops, with no runtime-tokenizer adapter in the path. Correctness is guarded by
/// `tests/selfhost_pipeline.rs`, which checks this composition against the reference.
fn run_parse_pipeline(path: &str) {
    use keleusma::Arena;
    use keleusma::bytecode::Value;
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

    // Lexer `src` block slot layout: len(1) + bytes(4096) then the intern table.
    const LEX_ISTART: usize = 1 + 4096;
    const LEX_ILEN: usize = 1 + 4096 + 512;
    const LEX_ICOUNT: usize = 1 + 4096 + 512 + 512;
    // Parser `toks` block slot layout.
    const P_LEN: usize = 0;
    const P_KINDS: usize = 1;
    const P_VALS: usize = 1 + 2048;
    const P_LIMIT_ID: usize = 1 + 2048 + 2048;
    const P_CHUNK_COUNT: usize = 1 + 2048 + 2048 + 1;
    const P_CHUNKS: usize = 1 + 2048 + 2048 + 2;
    const P_REQUIRE_ID: usize = 1 + 2048 + 2048 + 2 + 256;

    let input = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("cannot read {path}: {e}");
        std::process::exit(1);
    });
    if input.len() > 4096 {
        eprintln!(
            "input is {} bytes; the increment-1 lexer caps source at 4096",
            input.len()
        );
        std::process::exit(1);
    }

    // Stage 1: lex the source and recover the interned-name table.
    let lexer_src = read_stage("kel/lexer.kel");
    let lexer = compile_stage(&lexer_src, "lexer.kel", false);
    let need = required_persistent_capacity_for(&lexer);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut lvm = Vm::new(lexer, &arena).expect("verify lexer.kel");
    let mut lshared = vec![0u8; lvm.shared_data_bytes()];
    lvm.set_shared(&mut lshared, 0, Value::Int(input.len() as i64))
        .expect("len");
    for (i, &b) in input.iter().enumerate() {
        lvm.set_shared(&mut lshared, 1 + i, Value::Byte(b))
            .expect("byte");
    }
    let read_word = |vm: &Vm, buf: &[u8], slot: usize| -> i64 {
        match vm.get_shared(buf, slot).expect("get_shared") {
            Value::Int(n) => n,
            other => panic!("expected Int at slot {slot}, got {other:?}"),
        }
    };
    let mut tokens: Vec<(i64, i64)> = Vec::new();
    let mut st = lvm
        .call_with_shared(&mut lshared, &[Value::Int(0)])
        .expect("call lexer");
    for _ in 0..(input.len() * 4 + 16) {
        if let VmState::Yielded(Value::Int(t)) = st {
            if t == 63 {
                // PENDING
            } else if t == 62 {
                break; // EOF
            } else {
                tokens.push((t.rem_euclid(64), t.div_euclid(64)));
            }
        }
        st = lvm
            .resume_with_shared(&mut lshared, Value::Int(0))
            .expect("resume lexer");
    }
    let icount = read_word(&lvm, &lshared, LEX_ICOUNT) as usize;
    let mut names: Vec<String> = Vec::with_capacity(icount);
    for id in 0..icount {
        let start = read_word(&lvm, &lshared, LEX_ISTART + id) as usize;
        let len = read_word(&lvm, &lshared, LEX_ILEN + id) as usize;
        names.push(String::from_utf8_lossy(&input[start..start + len]).into_owned());
    }
    if tokens.len() > 2048 {
        eprintln!(
            "{} tokens; the parser stage caps input at 2048",
            tokens.len()
        );
        std::process::exit(1);
    }

    // Recover the parser's non-token inputs from the lexer's output.
    let id_of = |s: &str| names.iter().position(|n| n == s).map(|i| i as i64);
    let limit_id = id_of("limit").unwrap_or(-1);
    let require_id = id_of("require").unwrap_or(-1);
    let chunks = chunk_ids_from_tokens(&tokens);

    // Stage 2: drive the parser with the lexer-recovered inputs.
    let parse_src = read_stage("kel/parse.kel");
    let parser = compile_stage(&parse_src, "parse.kel", true);
    let need = required_persistent_capacity_for(&parser);
    let mut parena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    parena.resize_persistent(need).expect("resize");
    let mut pvm = Vm::new(parser, &parena).expect("verify parse.kel");
    let mut pshared = vec![0u8; pvm.shared_data_bytes()];
    pvm.set_shared(&mut pshared, P_LEN, Value::Int(tokens.len() as i64))
        .unwrap();
    pvm.set_shared(&mut pshared, P_LIMIT_ID, Value::Int(limit_id))
        .unwrap();
    pvm.set_shared(&mut pshared, P_REQUIRE_ID, Value::Int(require_id))
        .unwrap();
    pvm.set_shared(&mut pshared, P_CHUNK_COUNT, Value::Int(chunks.len() as i64))
        .unwrap();
    for (i, &c) in chunks.iter().enumerate() {
        pvm.set_shared(&mut pshared, P_CHUNKS + i, Value::Int(c))
            .unwrap();
    }
    for (i, &(k, v)) in tokens.iter().enumerate() {
        pvm.set_shared(&mut pshared, P_KINDS + i, Value::Int(k))
            .unwrap();
        pvm.set_shared(&mut pshared, P_VALS + i, Value::Int(v))
            .unwrap();
    }

    println!(
        "parsing {path} ({} bytes, {} tokens, {} identifiers):",
        input.len(),
        tokens.len(),
        names.len()
    );
    let name_of = |id: i64| -> String {
        usize::try_from(id)
            .ok()
            .and_then(|i| names.get(i))
            .cloned()
            .unwrap_or_else(|| format!("#{id}"))
    };

    // Decode the record stream and print one line per declaration.
    let (mut in_body, mut in_data, mut in_enum, mut in_use) = (false, false, false, false);
    let mut in_guard = false;
    let (mut cat, mut nm, mut params, mut body) = (0i64, 0i64, 0i64, 0i64);
    let (mut nfuncs, mut ndata, mut nenum, mut nuse) = (0, 0, 0, 0);
    let mut state = pvm
        .call_with_shared(&mut pshared, &[Value::Int(0)])
        .expect("call parser");
    for _ in 0..(tokens.len() * 4 + 64) {
        if let VmState::Yielded(Value::Int(w)) = state {
            let (code, val) = (w.rem_euclid(64), w.div_euclid(64));
            if in_body {
                match code {
                    0 => {}
                    15 => in_body = false,
                    _ => body += 1,
                }
            } else if in_guard {
                if code == 15 {
                    in_guard = false; // the `when` guard forest's Done
                }
            } else if in_data {
                if code == 5 {
                    in_data = false;
                }
            } else if in_enum {
                if code == 5 {
                    in_enum = false;
                }
            } else if in_use {
                if code == 5 {
                    in_use = false;
                }
            } else {
                match code {
                    0 => {}
                    1..=3 => {
                        cat = code;
                        nm = val;
                        params = 0;
                        body = 0;
                    }
                    4 => params += 1,
                    6..=8 => {}
                    9 => {
                        in_data = true;
                        ndata += 1;
                    }
                    10 => {
                        in_use = true;
                        nuse += 1;
                    }
                    12 => {
                        in_enum = true;
                        nenum += 1;
                    }
                    16 => in_body = true,
                    17 => in_guard = true, // GSTART: a `when` guard forest, skipped in the summary
                    5 => {
                        let kw = match cat {
                            1 => "fn",
                            2 => "yield",
                            _ => "loop",
                        };
                        nfuncs += 1;
                        println!(
                            "  {kw:<6} {:<20} params {params}  body {body} nodes",
                            name_of(nm)
                        );
                    }
                    15 => {
                        println!(
                            "{nfuncs} functions, {ndata} data blocks, {nenum} enums, {nuse} use imports"
                        );
                        return;
                    }
                    other => {
                        eprintln!("unexpected declaration record {other}");
                        std::process::exit(1);
                    }
                }
            }
        }
        state = pvm
            .resume_with_shared(&mut pshared, Value::Int(0))
            .expect("resume parser");
    }
    eprintln!("parser did not reach DONE within the iteration budget");
    std::process::exit(1);
}

/// The chunk table the parser needs: function-name ids in declaration order, from a
/// brace-depth scan of the token stream for a category keyword (fn 0, yield 5,
/// loop 6) at depth 0 immediately followed by an Ident (Tok 1).
fn chunk_ids_from_tokens(tokens: &[(i64, i64)]) -> Vec<i64> {
    let mut chunks = Vec::new();
    let mut depth = 0i64;
    for w in tokens.windows(2) {
        match w[0].0 {
            2 => depth += 1,
            3 => depth -= 1,
            0 | 5 | 6 if depth == 0 && w[1].0 == 1 => chunks.push(w[1].1),
            _ => {}
        }
    }
    chunks
}

/// Read a pipeline-stage source, trying the package-local path then the repo root.
fn read_stage(rel: &str) -> String {
    std::fs::read_to_string(rel)
        .or_else(|_| std::fs::read_to_string(format!("compiler/{rel}")))
        .unwrap_or_else(|e| {
            eprintln!("cannot read {rel}: {e}");
            std::process::exit(1);
        })
}

/// Compile a stage source to a module, on a wide-stack thread when `deep` (the
/// parser stage's nesting overflows the default stack in the host's recursive parse).
fn compile_stage(src: &str, name: &str, deep: bool) -> keleusma::bytecode::Module {
    let build = |src: String, name: String| {
        keleusma::compiler::compile(
            &keleusma::parser::parse(&keleusma::lexer::tokenize(&src).expect("lex stage"))
                .expect("parse stage"),
        )
        .unwrap_or_else(|e| {
            eprintln!("cannot compile {name}: {e:?}");
            std::process::exit(1);
        })
    };
    if deep {
        let (src, name) = (src.to_string(), name.to_string());
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(move || build(src, name))
            .expect("spawn")
            .join()
            .expect("join")
    } else {
        build(src.to_string(), name.to_string())
    }
}

fn not_yet(cmd: &str, what: &str) {
    eprintln!("`{cmd}` is not yet implemented (scaffolding).");
    eprintln!("when built, it will: {what}.");
    eprintln!("see MILESTONES.md for the release that lands it.");
    std::process::exit(1);
}

fn help() {
    println!("usage: keleusma-selfhost <command>");
    println!();
    println!("  status          show the pipeline scaffold state (default)");
    println!("  lex <file>      run Stage 1 (the self-hosted lexer) and print tokens");
    println!("  parse <file>    run Stages 1+2 (lexer into parser) and print declarations");
    println!("  compile         run the whole pipeline to bytecode (planned)");
    println!("  bootstrap       cross-compile, self-compile, reach fixed point (planned)");
    println!(
        "  verify-corpus   assert byte-identical output vs the Rust-hosted compiler (planned)"
    );
}
