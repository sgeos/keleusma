//! The worst-case-memory bound on chunks whose paths leave without falling
//! through, and the understatement that came from discarding what they consumed.
//!
//! # What was wrong
//!
//! `verify::wcmu_region` returned `Option<McuResult>`, in which `None` meant
//! "no path reaches the end of this region" AND carried no resources at all.
//! Four separate sites therefore threw away an accumulated operand peak and
//! arena heap total: the `Trap` arm, the `If` arm when both branches exited, the
//! `Loop` arm when the body never fell through, and every top-level caller's
//! `unwrap_or(McuResult::empty())`.
//!
//! **Resources are monotone along a path; control flow is not.** A region that
//! exits via `Trap` still reached the depth it reached and allocated what it
//! allocated on the way there, and a bound that omits that is understated.
//!
//! # How far it reached
//!
//! A multiheaded function compiles to a chain of guarded heads with a trailing
//! dispatch `Trap` on no-match, so **every multiheaded function in the language
//! was affected**. Measured across the example corpus before the repair: six of
//! sixty-four non-Stream chunks reported a body peak of exactly zero, and the
//! split against the trailing `Trap` was total — every chunk ending in `Trap`
//! reported zero and no other chunk did. The affected chunks included one of
//! 3905 operations.
//!
//! This was reported by the native-code-generation line as `classify` and
//! `corpse_fill` reporting a bound of 2 where both operand-stack models and the
//! native emitter said 3. The bound of 2 was the local frame alone; the entire
//! body contribution was missing.
//!
//! # What these tests do NOT establish
//!
//! - They pin two named chunks and a corpus-wide invariant. They are not
//!   evidence about the operand-stack models themselves, which are checked
//!   against each other over the whole opcode set in `verify`'s own tests.
//! - The corpus invariant is a property of the example corpus, which is a case
//!   list. `a_chunk_that_only_traps_still_reports_what_it_consumed` is the part
//!   that does not depend on any corpus.
//! - Agreement at 3 between the repaired bound, the two peak models and the
//!   emitter is agreement among four readers of the same instruction stream. It
//!   is not a measurement of the virtual machine's actual stack use.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::bytecode::{Chunk, Module, Op};

fn compile_source(src: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(src).expect("lex");
    let mut program = keleusma::parser::parse(&tokens).expect("parse");
    keleusma::typecheck::check(&mut program).expect("typecheck");
    let program = keleusma::monomorphize::monomorphize(program);
    keleusma::compiler::compile(&program).expect("compile")
}

fn compile_file(path: &str) -> Module {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    compile_source(&src)
}

/// Total operand slots the bound reports, and the part above the local frame.
fn bound_slots(chunk: &Chunk) -> (i32, i32) {
    let slot = keleusma::bytecode::VALUE_SLOT_SIZE_BYTES;
    let (stack_bytes, _heap) = keleusma::verify::wcmu_whole_chunk(chunk).expect("wcmu");
    let total = (stack_bytes / slot) as i32;
    (total, total - chunk.local_count as i32)
}

fn chunk_named<'m>(module: &'m Module, name: &str) -> &'m Chunk {
    module
        .chunks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| {
            panic!(
                "no chunk named {name}; present: {:?}",
                module.chunks.iter().map(|c| &c.name).collect::<Vec<_>>()
            )
        })
}

/// The two chunks the understatement was reported on, pinned by name and value.
///
/// Both are multiheaded dispatchers ending in a no-match `Trap`. The body peak
/// of 3 is what the native line measured both in-tree peak models to give when
/// walked branch-aware, and what its emitter allocates. Before the repair both
/// reported a body peak of 0.
///
/// Pinned as an exact value rather than a lower bound so that a later change in
/// EITHER direction fails here. An understatement is unsound; an overstatement
/// silently wastes arena and would otherwise pass unnoticed.
#[test]
fn the_two_reported_chunks_report_the_bound_the_emitter_allocates() {
    let cases: &[(&str, &str, i32, i32)] = &[
        // (source, chunk, expected total slots, expected body peak)
        ("examples/scripts/06_multiheaded.kel", "classify", 5, 3),
        (
            "examples/scripts/rogue/rogue_bestiary.kel",
            "corpse_fill",
            5,
            3,
        ),
    ];

    let mut checked = 0;
    for (path, name, want_total, want_body) in cases {
        let module = compile_file(path);
        let chunk = chunk_named(&module, name);

        assert!(
            matches!(chunk.ops.last(), Some(Op::Trap(_))),
            "{name}: expected a multiheaded dispatcher ending in a no-match Trap, \
             but the last op is {:?}. This test is named for a construct; if the \
             construct is gone the test is vacuous rather than passing.",
            chunk.ops.last()
        );

        let (total, body) = bound_slots(chunk);
        assert_eq!(
            body, *want_body,
            "{name}: body peak {body}, expected {want_body}. A value of 0 means the \
             exit-path resource discard has returned."
        );
        assert_eq!(total, *want_total, "{name}: total slots");
        checked += 1;
    }
    assert_eq!(checked, cases.len(), "not every case was checked");
}

/// A chunk whose every path leaves via Trap still reports what it consumed.
///
/// Independent of the example corpus and of the multihead construct: it states
/// the property directly. `f` pushes two operands and performs a checked
/// multiply before its dispatch trap, so its body cannot need zero slots.
#[test]
fn a_chunk_that_only_traps_still_reports_what_it_consumed() {
    let module = compile_source(
        "fn f(0) -> Word { 0 }\n\
         fn f(n: Word) -> Word { n * 10 }\n\
         fn main() -> Word { f(1) }\n",
    );
    let chunk = chunk_named(&module, "f");
    assert!(
        matches!(chunk.ops.last(), Some(Op::Trap(_))),
        "subject must end in Trap or it does not exercise the exit path"
    );
    let (_total, body) = bound_slots(chunk);
    assert!(
        body > 0,
        "a chunk that pushes operands before trapping reported a body peak of \
         {body}; the resources consumed before the exit are being discarded"
    );
}

/// THE CORPUS INVARIANT: no chunk that does real work reports a zero body peak.
///
/// Before the repair this reported six offenders, every one of them a chunk
/// ending in a dispatch `Trap`. The threshold on op count keeps trivially small
/// chunks, which can legitimately need no operand slots, from being counted.
#[test]
fn no_substantial_chunk_reports_a_zero_body_peak() {
    let mut paths: Vec<std::path::PathBuf> = Vec::new();
    for dir in ["examples/scripts", "examples/scripts/rogue"] {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().map(|x| x == "kel").unwrap_or(false) {
                    paths.push(p);
                }
            }
        }
    }
    paths.sort();
    assert!(
        paths.len() > 10,
        "expected a corpus of scripts, found {}; this test would pass vacuously",
        paths.len()
    );

    let mut examined = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for path in &paths {
        let Ok(src) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(tokens) = keleusma::lexer::tokenize(&src) else {
            continue;
        };
        let Ok(mut program) = keleusma::parser::parse(&tokens) else {
            continue;
        };
        if keleusma::typecheck::check(&mut program).is_err() {
            continue;
        }
        let program = keleusma::monomorphize::monomorphize(program);
        let Ok(module) = keleusma::compiler::compile(&program) else {
            continue;
        };
        for chunk in &module.chunks {
            if chunk.block_type == keleusma::bytecode::BlockType::Stream || chunk.ops.len() <= 4 {
                continue;
            }
            let Ok((stack_bytes, _)) = keleusma::verify::wcmu_whole_chunk(chunk) else {
                continue;
            };
            examined += 1;
            let slot = keleusma::bytecode::VALUE_SLOT_SIZE_BYTES;
            let body = (stack_bytes / slot) as i32 - chunk.local_count as i32;
            if body == 0 {
                offenders.push(format!(
                    "{}::{} ({} ops, ends_in_trap={})",
                    path.file_name().unwrap().to_string_lossy(),
                    chunk.name,
                    chunk.ops.len(),
                    matches!(chunk.ops.last(), Some(Op::Trap(_)))
                ));
            }
        }
    }

    assert!(
        examined > 40,
        "only {examined} chunks were examined; the corpus walk is not reaching them"
    );
    assert!(
        offenders.is_empty(),
        "{} chunk(s) of {examined} report a body peak of zero despite doing real work:\n  {}",
        offenders.len(),
        offenders.join("\n  ")
    );
}
