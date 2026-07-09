//! Driver and bootstrap harness for the self-hosted Keleusma compiler (V0.3.0).
//!
//! This is scaffolding. The three pipeline stages live in Keleusma source under
//! `kel/` and are not yet implemented; this driver establishes the structure that
//! will register the `compiler::` natives, drive the yield/resume pipeline, run the
//! bootstrap phases, and validate byte-identical output against the Rust-hosted
//! reference compiler. See `README.md` and `MILESTONES.md`; the authoritative design
//! is `docs/roadmap/V0_3_0_SELF_HOSTING.md`.

use std::path::Path;

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
        "bootstrap" => not_yet("bootstrap", "cross-compile (Phase A), self-compile to fixed point (Phases B, C)"),
        "verify-corpus" => not_yet("verify-corpus", "compile the regression corpus under kelc.1 and assert byte-identity with the Rust-hosted compiler"),
        "lex" | "parse" | "compile" => not_yet(cmd, "run a single stage over an input stream"),
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
    println!("status: scaffolding. No stage is implemented. Each V0.2.x release fills");
    println!("in a prerequisite or a stage; V0.3.0 ships when the bootstrap reaches a");
    println!("fixed point. See MILESTONES.md.");
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
    println!("  lex|parse|compile   run a single stage over an input stream (planned)");
    println!("  bootstrap       cross-compile, self-compile, reach fixed point (planned)");
    println!("  verify-corpus   assert byte-identical output vs the Rust-hosted compiler (planned)");
}
