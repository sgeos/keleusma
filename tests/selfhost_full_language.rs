//! The Order 6 full-language boundary over every Keleusma source in the repository.
//!
//! Order 6 widens the self-hosted compiler and validator from the self-hosting subset to
//! the whole language. `self_hosted_construct_support_boundary` in `selfhost_codegen.rs`
//! measures enumerated constructs. This file measures the programs people actually wrote,
//! through the shipping entry `self_hosted_compile`, so a slice that widens a construct
//! moves a pinned count here and a slice that breaks a program is caught here.
//!
//! **Every outcome is pinned per file, and the discovered file set must equal the table.**
//! A new source must be classified deliberately, and a removed one cannot shrink the
//! corpus silently. `Identical` means the self-hosted module equals the reference, because
//! `self_hosted_compile` returns a module only after that comparison passes.
//!
//! The classes are distinct because they mean different things. `Diverges` is a
//! self-hosted output that differs from the reference, caught by the shipping cross-check
//! and refused. It says the two disagree, not which is wrong. `StagePanic` is a stage
//! failure recovered into a refusal. `NoVerdict` is a type stage that could not run.
//! `PipelineRejects` is the self-hosted type stage rejecting the source before the
//! reference sees it, which is a false rejection whenever the reference accepts.
//! See `docs/decisions/ORDER6_FULL_LANGUAGE_PLAN.md`.

#![cfg(feature = "self-host")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use keleusma::selfhost::{
    SelfHostError, VerificationVerdict, compile_src, self_hosted_compile, verify_module_via_kel,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Class {
    Identical,
    Unsupported,
    Diverges,
    StagePanic,
    NoVerdict,
    PipelineRejects,
    RefRejects,
}
use Class::*;

/// Directories searched for `.kel` sources, not recursively.
const DIRS: &[&str] = &[
    "examples",
    "examples/scripts",
    "examples/scripts/rogue",
    "examples/scripts/piano_roll",
    "examples/rtos/scripts",
    "compiler/kel",
    "keleusma-cli/examples",
    "scripts",
    "src/crypto_frame",
    "src/hosted",
    "src/selfhost/kel",
];

/// The pinned outcome of the shipping self-hosted compile for every source.
const CORPUS: &[(&str, Class)] = &[
    ("examples/scripts/01_arithmetic.kel", Identical),
    ("examples/scripts/02_struct_field.kel", Identical),
    ("examples/scripts/03_enum_match.kel", Identical),
    ("examples/scripts/04_for_in.kel", Unsupported),
    ("examples/scripts/05_pipeline.kel", Unsupported),
    ("examples/scripts/06_multiheaded.kel", Unsupported),
    ("examples/scripts/07_refinement.kel", Unsupported),
    ("examples/scripts/08_method_dispatch.kel", Unsupported),
    ("examples/scripts/09_big_numbers.kel", Unsupported),
    ("examples/scripts/10_multbyte.kel", Unsupported),
    ("examples/scripts/11_signed.kel", Unsupported),
    ("examples/scripts/12_sensor_window.kel", Identical),
    ("examples/scripts/13_telemetry_stream.kel", Unsupported),
    ("examples/scripts/14_frame_log.kel", Unsupported),
    ("examples/scripts/15_pixel_blend.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_boss.kel", Unsupported),
    ("examples/scripts/rogue/rogue_ai_chaser.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_fast.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_hunter.kel", Unsupported),
    ("examples/scripts/rogue/rogue_ai_idle.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_ranged.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_sleeper.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_smart.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_tracker.kel", Unsupported),
    ("examples/scripts/rogue/rogue_ai_wander.kel", Identical),
    ("examples/scripts/rogue/rogue_bestiary.kel", Unsupported),
    ("examples/scripts/rogue/rogue_book_keeping.kel", Identical),
    ("examples/scripts/rogue/rogue_combat.kel", Identical),
    ("examples/scripts/rogue/rogue_consume.kel", Identical),
    ("examples/scripts/rogue/rogue_descend.kel", Identical),
    ("examples/scripts/rogue/rogue_dungen.kel", NoVerdict),
    ("examples/scripts/rogue/rogue_game.kel", Unsupported),
    ("examples/scripts/rogue/rogue_gear.kel", Unsupported),
    ("examples/scripts/rogue/rogue_item_potion.kel", Identical),
    ("examples/scripts/rogue/rogue_item_scroll.kel", Identical),
    ("examples/scripts/rogue/rogue_move_resolve.kel", Identical),
    ("examples/scripts/rogue/rogue_pickup.kel", Identical),
    ("examples/scripts/rogue/rogue_player_ai.kel", Identical),
    ("examples/scripts/rogue/rogue_scroll_apply.kel", Identical),
    ("examples/scripts/piano_roll/piano_roll_0.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_1.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_2.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_3.kel", NoVerdict),
    ("examples/scripts/piano_roll/piano_roll_4.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_5.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_6.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_7.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_8.kel", Unsupported),
    ("examples/scripts/piano_roll/piano_roll_9.kel", Unsupported),
    ("examples/rtos/scripts/event_listener.kel", RefRejects),
    ("examples/rtos/scripts/faulty.kel", RefRejects),
    ("examples/rtos/scripts/heartbeat.kel", PipelineRejects),
    ("examples/rtos/scripts/led.kel", RefRejects),
    ("examples/rtos/scripts/prelude.kel", Unsupported),
    ("examples/rtos/scripts/sensor.kel", RefRejects),
    ("examples/zero_copy_demo.kel", Identical),
    ("compiler/kel/prelude.kel", Identical),
    ("keleusma-cli/examples/hello.kel", Identical),
    ("scripts/check-md-links.kel", Unsupported),
    ("src/crypto_frame/frame.kel", Unsupported),
    ("src/hosted/interp.kel", Unsupported),
    ("src/selfhost/kel/analyze.kel", Identical),
    ("src/selfhost/kel/codegen.kel", Identical),
    ("src/selfhost/kel/lexer.kel", Identical),
    ("src/selfhost/kel/parse.kel", Identical),
    ("src/selfhost/kel/reconstruct.kel", Identical),
    ("src/selfhost/kel/verify_datalayout.kel", Identical),
    ("src/selfhost/kel/verify_depth.kel", Identical),
    ("src/selfhost/kel/verify_structural.kel", Identical),
    ("src/selfhost/kel/verify_typed.kel", Identical),
    ("src/selfhost/kel/verify_types.kel", Identical),
    ("src/selfhost/kel/verify_yield.kel", Identical),
    ("src/selfhost/kel/wire.kel", Identical),
    ("src/selfhost/kel/wire_debug.kel", Identical),
    ("src/selfhost/kel/wire_schema.kel", Identical),
];

/// Sources the reference verifier accepts and the self-hosted validator cannot yet
/// analyse, each for the per-chunk opcode capacity. Every other source the reference
/// compiles must receive the reference verdict.
const VALIDATOR_CAPACITY: &[&str] = &[
    "examples/scripts/piano_roll/piano_roll_3.kel",
    "examples/scripts/piano_roll/piano_roll_8.kel",
    "examples/scripts/piano_roll/piano_roll_9.kel",
    "examples/scripts/rogue/rogue_bestiary.kel",
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn discovered() -> Vec<String> {
    let root = root();
    let mut out = Vec::new();
    for dir in DIRS {
        let entries = std::fs::read_dir(root.join(dir)).expect("corpus directory is readable");
        for entry in entries {
            let path = entry.expect("directory entry").path();
            if path.extension().is_some_and(|x| x == "kel") {
                out.push(relative(&root, &path));
            }
        }
    }
    out.sort();
    out
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("under the root")
        .display()
        .to_string()
}

fn classify(src: &str) -> (Class, String) {
    let target = keleusma::target::Target::host();
    match self_hosted_compile(src, &target) {
        Ok(_) => (Identical, String::new()),
        Err(error) => {
            let detail = format!("{error:?}");
            let class = match &error {
                SelfHostError::Unsupported { detail } if detail.contains(" diverges (") => Diverges,
                SelfHostError::Unsupported { detail }
                    if detail.contains("called `Result::unwrap()`") =>
                {
                    StagePanic
                }
                SelfHostError::Unsupported { .. } => Unsupported,
                SelfHostError::TypeCheckUnavailable { .. } => NoVerdict,
                SelfHostError::PipelineRejected { .. } => PipelineRejects,
                SelfHostError::ReferenceRejected { .. } => RefRejects,
                other => panic!("unexpected self-hosted outcome {other:?}"),
            };
            (class, detail)
        }
    }
}

#[test]
fn the_corpus_table_names_every_source_once() {
    let mut named: Vec<String> = CORPUS.iter().map(|(f, _)| (*f).to_string()).collect();
    named.sort();
    let before = named.len();
    named.dedup();
    assert_eq!(before, named.len(), "a source is named twice");
    assert_eq!(
        named,
        discovered(),
        "the table and the repository disagree. Classify a new source deliberately and \
         remove a deleted one, rather than letting the corpus change size silently"
    );
}

#[test]
fn every_source_has_its_pinned_self_hosted_outcome() {
    let root = root();
    let mut mismatches = Vec::new();
    let mut counts: BTreeMap<Class, usize> = BTreeMap::new();
    for (file, expected) in CORPUS {
        let src = std::fs::read_to_string(root.join(file)).expect("readable source");
        let (actual, detail) = classify(&src);
        *counts.entry(actual).or_default() += 1;
        if actual != *expected {
            mismatches.push(format!(
                "{file}: expected {expected:?}, got {actual:?} {detail}"
            ));
        }
    }
    assert!(
        mismatches.is_empty(),
        "self-hosted outcomes moved. Update the table deliberately when a slice widens \
         support, and treat any move away from Identical as a regression:\n{}",
        mismatches.join("\n")
    );
    // Non-vacuity. A classifier that called everything one class would fail above, and
    // this keeps the population each class claims from collapsing unnoticed.
    assert!(
        counts.get(&Identical).copied().unwrap_or(0) >= 30,
        "{counts:?}"
    );
    println!("full-language boundary {counts:?}");
}

#[test]
fn the_self_hosted_validator_agrees_with_the_reference_on_every_compiled_source() {
    let root = root();
    let mut checked = 0usize;
    let mut capacity = Vec::new();
    let mut disagreements = Vec::new();
    for (file, _) in CORPUS {
        let src = std::fs::read_to_string(root.join(file)).expect("readable source");
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let module = std::panic::catch_unwind(|| compile_src(&src));
        std::panic::set_hook(prev);
        let Ok(module) = module else { continue };
        let reference = keleusma::verify::verify(&module).is_ok();
        match verify_module_via_kel(&module) {
            Ok(verdict) => {
                checked += 1;
                let accepted = verdict == VerificationVerdict::Accepted;
                if accepted != reference {
                    disagreements.push(format!(
                        "{file}: reference {reference}, self-hosted {verdict:?}"
                    ));
                }
            }
            Err(error) => {
                let text = error.to_string();
                assert!(
                    text.contains("opcodes capacity"),
                    "{file}: unavailable analysis other than the opcode capacity: {text}"
                );
                capacity.push((*file).to_string());
            }
        }
    }
    assert!(disagreements.is_empty(), "{}", disagreements.join("\n"));
    let pinned: Vec<String> = VALIDATOR_CAPACITY
        .iter()
        .map(|f| (*f).to_string())
        .collect();
    capacity.sort();
    assert_eq!(capacity, pinned, "the validator's capacity refusals moved");
    assert!(checked >= 60, "only {checked} sources reached a verdict");
}

/// Slice 1 of Order 6. Each source here was a defect rather than a language gap, a token
/// that the pipeline skipped, and each now has a pinned outcome. The expected classes were
/// measured before the repair. A literal head became a head with no parameter (Diverges),
/// a pipe became a comparison (PipelineRejects), a trailing comma or a bare unit variant
/// hung or faulted the parser (NoVerdict), and a data-field projection became an undefined
/// identifier (PipelineRejects).
#[test]
fn skipped_tokens_are_refused_or_lowered_rather_than_dropped() {
    let cases: &[(&str, Class)] = &[
        // Parameter patterns are refused. The controls keep plain parameters identical.
        (
            "fn c(0) -> Word { 0 } fn c(n: Word) -> Word { n } fn main() -> Word { c(3) }",
            Unsupported,
        ),
        (
            "fn c(true) -> Word { 0 } fn c(b: bool) -> Word { 1 } fn main() -> Word { c(true) }",
            Unsupported,
        ),
        (
            "fn c(_) -> Word { 0 } fn main() -> Word { c(3) }",
            Unsupported,
        ),
        (
            "fn c((a, b): (Word, Word)) -> Word { a } fn main() -> Word { c((1, 2)) }",
            Unsupported,
        ),
        (
            "enum E { A, B } fn c(E::A) -> Word { 0 } fn c(e: E) -> Word { 1 } fn main() -> Word { c(E::B) }",
            Unsupported,
        ),
        (
            "fn c(a: Word, b: Word) -> Word { a + b } fn main() -> Word { c(1, 2) }",
            Identical,
        ),
        ("fn c() -> Word { 1 } fn main() -> Word { c() }", Identical),
        // An unrecognized byte is refused rather than read as an arrow.
        (
            "fn d(x: Word) -> Word { x } fn main() -> Word { 6 |> d() }",
            Unsupported,
        ),
        ("fn main() -> Word { if 6 > 1 { 1 } else { 0 } }", Identical),
        // A trailing comma closes a struct literal, in any field order and nested.
        (
            "struct S { a: Word, b: Word } fn main() -> Word { let t = S { a: 1, b: 2, }; t.a }",
            Identical,
        ),
        (
            "struct S { a: Word, b: Word } fn g(x: Word) -> Word { x } fn main() -> Word { let t = S { a: 1, b: g(2), }; t.a }",
            Identical,
        ),
        (
            "struct S { a: Word, b: Word } fn main() -> Word { let t = S { b: 2, a: 1, }; t.a }",
            Identical,
        ),
        (
            "struct S { a: Word, b: Word } fn main() -> Word { let t = S { a 1, b: 2 }; t.a }",
            RefRejects,
        ),
        // A bare unit variant is the same construction as `E::V()`.
        (
            "enum P { A, B } fn f() -> P { P::A } fn main() -> Word { match f() { P::A => 1, P::B => 2 } }",
            Identical,
        ),
        (
            "enum P { A, B } fn main() -> Word { P::B as Word }",
            Identical,
        ),
        (
            "enum P { A, B } fn g(p: P) -> Word { 1 } fn main() -> Word { g(P::A) + g(P::B) }",
            Identical,
        ),
        (
            "enum P { A, B } fn f(i: Word) -> (P, Word) { match i { 0 => (P::A, 4), 1 => (P::A, 4), 2 => (P::A, 4), \
          3 => (P::A, 4), 4 => (P::A, 4), 5 => (P::A, 4), 6 => (P::A, 4), 7 => (P::A, 4), 8 => (P::A, 4), \
          _ => (P::B, 1) } } fn main() -> Word { 1 }",
            Identical,
        ),
        (
            "enum P { A(Word), B } fn main() -> Word { let x = P::A; 1 }",
            PipelineRejects,
        ),
        // A data-field projection is refused rather than misread.
        (
            "struct F { id: Word, len: Word } private data log { latest: F, count: Word } \
          loop main(tick: Word) -> Word { log.latest = F { id: 1, len: 2 }; log.count = log.count + 1; \
          let _a = yield log.latest.len + log.count; 0 }",
            Unsupported,
        ),
        (
            "private data log { count: Word } loop main(tick: Word) -> Word { log.count = log.count + 1; \
          let _a = yield log.count; 0 }",
            Identical,
        ),
    ];
    let mut mismatches = Vec::new();
    for (src, expected) in cases {
        let (actual, detail) = classify(src);
        if actual != *expected {
            mismatches.push(format!(
                "{src}\n  expected {expected:?}, got {actual:?} {detail}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
    // A pipeline rejection is honest only when the reference rejects too.
    for (src, expected) in cases {
        if *expected == PipelineRejects {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let compiled = std::panic::catch_unwind(|| compile_src(src)).is_ok();
            std::panic::set_hook(prev);
            assert!(
                !compiled,
                "the pipeline rejects a source the reference compiles: {src}"
            );
        }
    }
}

/// Slice 2 of Order 6, native imports and qualified native calls. A two-segment `use` of a
/// host function and a call of it compile byte-identically, which covers the native index
/// order, the error-reify flag, the default `HostError` trap and the `Top` return shape. The
/// refused forms are refused rather than skipped, and each is pinned with its reason.
#[test]
fn native_imports_and_qualified_calls_compile_or_are_refused() {
    let cases: &[(&str, Class)] = &[
        ("use host::f\nfn main() -> Word { host::f(1) }", Identical),
        (
            "use host::f\nuse host::g\nfn main() -> Word { host::g(1, 2) + host::f(3) }",
            Identical,
        ),
        (
            "use host::f\nfn main() -> Word { let x = host::f(1); x + 1 }",
            Identical,
        ),
        (
            "use host::f\nuse host::g\nfn main() -> Word { host::g(host::f(1), 2) }",
            Identical,
        ),
        (
            "use host::f\nuse host::g\nfn k(a: Word) -> Word { host::g(a, 7) }\nfn main() -> Word { k(host::f(0)) }",
            Identical,
        ),
        ("use host::f\nfn main() -> Word { host::f() }", Identical),
        // An import that is never called still takes its index.
        (
            "use host::a\nuse host::f\nfn main() -> Word { host::f(1) }",
            Identical,
        ),
        // A wildcard is indistinguishable from a single-segment import in the record stream.
        (
            "use host::*\nuse host::f\nfn main() -> Word { host::f(1) }",
            Unsupported,
        ),
        // A single-segment import is called unqualified, which this path does not resolve.
        ("use f\nfn main() -> Word { f(1) }", Unsupported),
        // A signature is slice 2b.
        (
            "use host::f(Word) -> Word\nfn main() -> Word { host::f(1) }",
            Unsupported,
        ),
        // `external` has no keyword in the self-hosted lexer.
        (
            "use external host::f\nfn main() -> Word { host::f(1) }",
            Unsupported,
        ),
        // A call of an undeclared native is the reference's to reject.
        (
            "use host::f\nfn main() -> Word { host::nope(1) }",
            RefRejects,
        ),
    ];
    let mut mismatches = Vec::new();
    for (src, expected) in cases {
        let (actual, detail) = classify(src);
        if actual != *expected {
            mismatches.push(format!(
                "{src}\n  expected {expected:?}, got {actual:?} {detail}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
