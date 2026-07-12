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
        "parse" | "compile" => not_yet(cmd, "run a single stage over an input stream"),
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
    println!("(see tests/selfhost_parse.rs). V0.3.0 ships when the bootstrap reaches a");
    println!("fixed point.");
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
    println!("  parse|compile   run a single stage over an input stream (planned)");
    println!("  bootstrap       cross-compile, self-compile, reach fixed point (planned)");
    println!(
        "  verify-corpus   assert byte-identical output vs the Rust-hosted compiler (planned)"
    );
}
