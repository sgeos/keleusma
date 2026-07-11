//! Driver and bootstrap harness for the self-hosted Keleusma compiler (V0.3.0).
//!
//! This is scaffolding. The three pipeline stages live in Keleusma source under
//! `kel/` and are not yet implemented; this driver establishes the structure that
//! will register the `compiler::` natives, drive the yield/resume pipeline, run the
//! bootstrap phases, and validate byte-identical output against the Rust-hosted
//! reference compiler. See `README.md` and `MILESTONES.md`; the authoritative design
//! is `docs/roadmap/V0_3_0_SELF_HOSTING.md`.

/// The bytecode format the self-hosted compiler must emit. Sourced from the parent
/// runtime so the two compilers cannot drift on the wire format.
const TARGET_BYTECODE_VERSION: u16 = keleusma::bytecode::BYTECODE_VERSION;

/// The three pipeline stages, in migration order (roadmap Steps 1, 2, 3).
const STAGES: &[(&str, &str)] = &[
    ("lexer", "kel/lexer.kel"),
    ("parser", "kel/parser.kel"),
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
    println!("status: porting backward, codegen first. Codegen increment 24, a");
    println!("recursion-free work-stack walk (a `loop` that delegates its yield to a");
    println!("multiheaded guarded `yield` phase machine over per-kind subroutines)");
    println!("that compiles blocks of `let` and data-field assignments");
    println!("over the binary integer arithmetic set (+ - * / %), the six");
    println!("comparison operators, unary `not` and `-`, the bitwise band/bor/bxor,");
    println!("short-circuit `andalso`/`orelse`, `if`/`else` structured control flow");
    println!("with stage-resolved jump targets, function calls, and scalar and");
    println!("indexed data-segment reads and writes, into an op buffer it streams with its own");
    println!("deduplicating constant pool and counted local-frame size, and lexer");
    println!("increment 1, a streaming tokenizer, both compile, verify, and run (see");
    println!("tests/selfhost_codegen.rs and `lex <file>`). V0.3.0 ships when the");
    println!("bootstrap reaches a fixed point.");
}

/// Run Stage 1 (the self-hosted lexer) over `path` and print the token stream.
///
/// This drives `kel/lexer.kel` on the current runtime: it compiles the lexer,
/// places the input source in the loop's `shared data` byte array, and resumes
/// it, decoding the increment-1 `kind + value*16` token wire. It is the first
/// end-to-end proof that the host can drive a self-hosted pipeline stage.
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
                let (kind, value) = (t.rem_euclid(16), t.div_euclid(16));
                match kind {
                    0 => {} // PENDING; the host skips it
                    1 => {
                        println!("  EOF");
                        return;
                    }
                    2 => println!("  IDENT   len {value}"),
                    3 => println!("  INT     {value}"),
                    4 => println!("  PUNCT   {:?}", value as u8 as char),
                    other => println!("  ?kind {other} value {value}"),
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
