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
    ("examples/scripts/04_for_in.kel", Identical),
    ("examples/scripts/05_pipeline.kel", Identical),
    ("examples/scripts/06_multiheaded.kel", Identical),
    ("examples/scripts/07_refinement.kel", Unsupported),
    ("examples/scripts/08_method_dispatch.kel", Identical),
    ("examples/scripts/09_big_numbers.kel", Identical),
    ("examples/scripts/10_multbyte.kel", Identical),
    ("examples/scripts/11_signed.kel", Identical),
    ("examples/scripts/12_sensor_window.kel", Identical),
    ("examples/scripts/13_telemetry_stream.kel", Identical),
    // Unsupported until slice 6 lowered the projection of a struct-typed data field.
    ("examples/scripts/14_frame_log.kel", Identical),
    ("examples/scripts/15_pixel_blend.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_boss.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_chaser.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_fast.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_hunter.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_idle.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_ranged.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_sleeper.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_smart.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_tracker.kel", Identical),
    ("examples/scripts/rogue/rogue_ai_wander.kel", Identical),
    // Identical since the reconstruct, codegen and constant-pool capacities rose, 2026-10-26.
    ("examples/scripts/rogue/rogue_bestiary.kel", Identical),
    ("examples/scripts/rogue/rogue_book_keeping.kel", Identical),
    ("examples/scripts/rogue/rogue_combat.kel", Identical),
    ("examples/scripts/rogue/rogue_consume.kel", Identical),
    ("examples/scripts/rogue/rogue_descend.kel", Identical),
    // Identical since the parser's local capacity rose and native results are typed from
    // their declared scalar return, 2026-10-26.
    ("examples/scripts/rogue/rogue_dungen.kel", Identical),
    ("examples/scripts/rogue/rogue_game.kel", Identical),
    // Identical since the reconstruct, codegen and constant-pool capacities rose, 2026-10-26.
    ("examples/scripts/rogue/rogue_gear.kel", Identical),
    ("examples/scripts/rogue/rogue_item_potion.kel", Identical),
    ("examples/scripts/rogue/rogue_item_scroll.kel", Identical),
    ("examples/scripts/rogue/rogue_move_resolve.kel", Identical),
    ("examples/scripts/rogue/rogue_pickup.kel", Identical),
    ("examples/scripts/rogue/rogue_player_ai.kel", Identical),
    ("examples/scripts/rogue/rogue_scroll_apply.kel", Identical),
    // Identical since the reconstruct, codegen and constant-pool capacities rose, 2026-10-26.
    ("examples/scripts/piano_roll/piano_roll_0.kel", Identical),
    // Identical since the reconstruct, codegen and constant-pool capacities rose, 2026-10-26.
    ("examples/scripts/piano_roll/piano_roll_1.kel", Identical),
    ("examples/scripts/piano_roll/piano_roll_2.kel", Identical),
    // Identical since the parser's local capacity rose and native results are typed from
    // their declared scalar return, 2026-10-26.
    ("examples/scripts/piano_roll/piano_roll_3.kel", Identical),
    // Identical since the reconstruct, codegen and constant-pool capacities rose, 2026-10-26.
    ("examples/scripts/piano_roll/piano_roll_4.kel", Identical),
    ("examples/scripts/piano_roll/piano_roll_5.kel", Identical),
    ("examples/scripts/piano_roll/piano_roll_6.kel", Identical),
    ("examples/scripts/piano_roll/piano_roll_7.kel", Identical),
    // Identical since the reconstruct, codegen and constant-pool capacities rose, 2026-10-26.
    ("examples/scripts/piano_roll/piano_roll_8.kel", Identical),
    // Identical since the reconstruct, codegen and constant-pool capacities rose, 2026-10-26.
    ("examples/scripts/piano_roll/piano_roll_9.kel", Identical),
    ("examples/rtos/scripts/event_listener.kel", RefRejects),
    ("examples/rtos/scripts/faulty.kel", RefRejects),
    // Rejected by the pipeline until slice 5d, which stopped reading `saturate_max` as an
    // undefined name. The remaining rejection is the reference's, since the script calls
    // `host::log_event` without the RTOS prelude that declares it.
    ("examples/rtos/scripts/heartbeat.kel", RefRejects),
    ("examples/rtos/scripts/led.kel", RefRejects),
    // Identical since slice 8 accepted its tuple-returning signatures.
    ("examples/rtos/scripts/prelude.kel", Identical),
    ("examples/rtos/scripts/sensor.kel", RefRejects),
    ("examples/zero_copy_demo.kel", Identical),
    ("compiler/kel/prelude.kel", Identical),
    ("keleusma-cli/examples/hello.kel", Identical),
    // Identical since an untyped native's destructured elements read boxed, 2026-10-26.
    ("scripts/check-md-links.kel", Identical),
    ("src/crypto_frame/frame.kel", Identical),
    ("src/hosted/interp.kel", Identical),
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
/// compiles must receive the reference verdict. Empty since the capacity rose from 2,048 to
/// 8,192 under the operator's ruling of 2026-10-26, which closed the four it held, two rogue
/// and piano-roll sources whose largest chunks reach 2,056 to 4,702 ops.
const VALIDATOR_CAPACITY: &[&str] = &[];

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

/// THE GATE 3 REJECTION CORPUS. Every module the reference compiles from the corpus, each
/// corrupted by every family below at its first applicable instruction. The self-hosted
/// validator must give the reference's verdict on every corrupted module, and the capacity
/// refusals already pinned in `VALIDATOR_CAPACITY` are the only exception. Each family must
/// reach many modules and the reference must reject most of them, or the corpus would
/// measure nothing.
#[test]
fn the_self_hosted_validator_agrees_with_the_reference_on_a_mutated_corpus() {
    use keleusma::bytecode::{Module, Op};
    type Mutation = fn(&mut Module) -> bool;
    fn first(m: &mut Module, f: impl Fn(&mut Op, usize, usize, usize, usize) -> bool) -> bool {
        let chunks = m.chunks.len();
        let natives = m.native_names.len();
        for chunk in &mut m.chunks {
            let locals = chunk.local_count as usize + chunk.param_count as usize;
            let consts = chunk.constants.len();
            let len = chunk.ops.len();
            for op in &mut chunk.ops {
                if f(op, locals, consts, len, chunks.max(natives)) {
                    return true;
                }
            }
        }
        false
    }
    let families: &[(&str, Mutation)] = &[
        ("local out of range", |m| {
            first(m, |op, locals, _, _, _| match op {
                Op::GetLocal(i) => {
                    *i = (locals + 9) as u16;
                    true
                }
                _ => false,
            })
        }),
        ("constant out of range", |m| {
            first(m, |op, _, consts, _, _| match op {
                Op::Const(i) => {
                    *i = (consts + 9) as u16;
                    true
                }
                _ => false,
            })
        }),
        ("branch target out of range", |m| {
            first(m, |op, _, _, len, _| match op {
                Op::If(t) => {
                    *t = (len + 9) as u16;
                    true
                }
                _ => false,
            })
        }),
        ("callee out of range", |m| {
            let n = m.chunks.len();
            first(m, |op, _, _, _, _| match op {
                Op::Call(c, _) => {
                    *c = (n + 9) as u16;
                    true
                }
                _ => false,
            })
        }),
        ("native out of range", |m| {
            let n = m.native_names.len();
            first(m, |op, _, _, _, _| match op {
                Op::CallVerifiedNative(c, _) => {
                    *c = (n + 9) as u16;
                    true
                }
                _ => false,
            })
        }),
        ("data slot out of range", |m| {
            let n = m.data_layout.as_ref().map_or(0, |d| d.slots.len());
            first(m, |op, _, _, _, _| match op {
                Op::GetData(i) | Op::SetData(i) => {
                    *i = (n + 9) as u32;
                    true
                }
                _ => false,
            })
        }),
        ("final return removed", |m| {
            for chunk in &mut m.chunks {
                if matches!(chunk.ops.last(), Some(Op::Return)) {
                    chunk.ops.pop();
                    return true;
                }
            }
            false
        }),
    ];
    let root = root();
    let mut reached: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut disagreements = Vec::new();
    for (file, _) in CORPUS {
        if VALIDATOR_CAPACITY.contains(file) {
            continue;
        }
        let src = std::fs::read_to_string(root.join(file)).expect("readable source");
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let module = std::panic::catch_unwind(|| compile_src(&src));
        std::panic::set_hook(prev);
        let Ok(module) = module else { continue };
        for (family, mutate) in families {
            let mut m = module.clone();
            if !mutate(&mut m) {
                continue;
            }
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|_| {}));
            let reference = std::panic::catch_unwind(|| keleusma::verify::verify(&m).is_ok());
            let staged = std::panic::catch_unwind(|| verify_module_via_kel(&m));
            std::panic::set_hook(prev);
            let Ok(reference) = reference else {
                disagreements.push(format!("{file} {family}: the reference verifier panicked"));
                continue;
            };
            let entry = reached.entry(family).or_default();
            entry.0 += 1;
            if !reference {
                entry.1 += 1;
            }
            match staged {
                Ok(Ok(verdict)) => {
                    let accepted = verdict == VerificationVerdict::Accepted;
                    if accepted != reference {
                        disagreements.push(format!(
                            "{file} {family}: reference {reference}, self-hosted {verdict:?}"
                        ));
                    }
                }
                Ok(Err(error)) => disagreements.push(format!(
                    "{file} {family}: self-hosted analysis unavailable: {error}"
                )),
                Err(_) => {
                    disagreements.push(format!("{file} {family}: self-hosted validator panicked"))
                }
            }
        }
    }
    println!("mutated corpus, per family (modules, reference rejections): {reached:?}");
    assert!(disagreements.is_empty(), "{}", disagreements.join("\n"));
    for (family, _) in families {
        let (modules, rejected) = reached.get(family).copied().unwrap_or((0, 0));
        assert!(modules >= 10, "{family}: reached only {modules} modules");
        // The reference admits a native index past the native table, and so does `Vm::new`,
        // measured 2026-10-26, so that family measures agreement and not rejection. Whether
        // admission should check it is operator question 13.
        if *family == "native out of range" {
            assert_eq!(
                rejected, 0,
                "the reference began rejecting an out-of-range native"
            );
        } else {
            assert!(
                rejected * 2 >= modules,
                "{family}: the reference rejected only {rejected} of {modules}"
            );
        }
    }
}

/// THE GATE 4 INSTRUMENT. The trap analysis in Keleusma and its Rust reference over every
/// source the reference compiles. Agreement is on the verdict, `TrapFree` against `MayTrap`
/// with the same first location, and on the class of a refusal, `InvalidModule`,
/// `Unsupported` at the same instruction, or `Capacity`. `Unavailable` on the Keleusma side
/// is pinned by name, since it is evidence about the analysis rather than the subject.
#[test]
fn the_trap_analysis_agrees_with_the_reference_on_every_compiled_source() {
    use keleusma::trap_analysis::{TrapAnalysisError, TrapVerdict, classify_traps};
    fn class(r: &Result<TrapVerdict, TrapAnalysisError>) -> String {
        match r {
            Ok(TrapVerdict::TrapFree) => "TrapFree".into(),
            Ok(TrapVerdict::MayTrap { first }) => format!("MayTrap {first:?}"),
            Err(TrapAnalysisError::InvalidModule { .. }) => "InvalidModule".into(),
            Err(TrapAnalysisError::Unsupported { location, .. }) => {
                format!("Unsupported {location:?}")
            }
            Err(TrapAnalysisError::Capacity { chunk, .. }) => format!("Capacity {chunk}"),
            Err(TrapAnalysisError::Unavailable { .. }) => "Unavailable".into(),
        }
    }
    let root = root();
    let mut compared = 0usize;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut unavailable = Vec::new();
    let mut refused = Vec::new();
    let mut disagreements = Vec::new();
    for (file, _) in CORPUS {
        let src = std::fs::read_to_string(root.join(file)).expect("readable source");
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let module = std::panic::catch_unwind(|| compile_src(&src));
        std::panic::set_hook(prev);
        let Ok(module) = module else { continue };
        let reference = class(&classify_traps(&module));
        let staged = class(&keleusma::selfhost::classify_traps_via_kel(&module));
        let kind = reference.split(' ').next().unwrap_or("").to_string();
        if kind == "Capacity" || kind == "Unsupported" {
            refused.push(format!("{file} {kind}"));
        }
        *counts.entry(kind).or_default() += 1;
        if staged == "Unavailable" {
            unavailable.push((*file).to_string());
        } else if staged != reference {
            disagreements.push(format!("{file}: reference {reference}, Keleusma {staged}"));
        } else {
            compared += 1;
        }
    }
    println!("trap census {counts:?}, compared {compared}, unavailable {unavailable:?}");
    println!("trap refusals {refused:?}");
    let pinned_refusals: Vec<String> = TRAP_REFUSALS
        .iter()
        .map(|(file, kind)| format!("{file} {kind}"))
        .collect();
    assert_eq!(
        refused, pinned_refusals,
        "the sources both trap analyses refuse moved"
    );
    assert!(disagreements.is_empty(), "{}", disagreements.join("\n"));
    let pinned: Vec<String> = TRAP_UNAVAILABLE.iter().map(|f| (*f).to_string()).collect();
    unavailable.sort();
    assert_eq!(
        unavailable, pinned,
        "the trap analysis's unavailable sources moved"
    );
    assert!(
        compared >= 60,
        "only {compared} sources reached a compared verdict"
    );
}

/// Sources the Keleusma trap analysis cannot finish on, measured, so a change in either
/// direction is announced.
const TRAP_UNAVAILABLE: &[&str] = &[];

/// Sources both trap analyses refuse in agreement, measured 2026-10-26. The four capacity
/// refusals the first measurement held closed when the per-chunk opcode capacity rose from
/// 2,048 to 8,192, and both analyses agree on all four. The unsupported one is outside the
/// first-pass proof profile, a boxed field read. Gate 4 asks for it to close.
const TRAP_REFUSALS: &[(&str, &str)] = &[("scripts/check-md-links.kel", "Unsupported")];

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
            // A literal head was refused here in slice 1 and is lowered since slice 3b.
            "fn c(0) -> Word { 0 } fn c(n: Word) -> Word { n } fn main() -> Word { c(3) }",
            Identical,
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
            // The pipe was refused here in slice 1 and is lowered since slice 3b. The point
            // of the slice 1 case stands: it no longer reads as a comparison.
            "fn d(x: Word) -> Word { x } fn main() -> Word { 6 |> d() }",
            Identical,
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
        // A data-field projection was refused rather than misread until slice 6 lowered it.
        (
            "struct F { id: Word, len: Word } private data log { latest: F, count: Word } \
          loop main(tick: Word) -> Word { log.latest = F { id: 1, len: 2 }; log.count = log.count + 1; \
          let _a = yield log.latest.len + log.count; 0 }",
            Identical,
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
        // A signature, refused in slice 2, is read since slice 2b.
        (
            "use host::f(Word) -> Word\nfn main() -> Word { host::f(1) }",
            Identical,
        ),
        // `use external`, refused in slice 2, is lowered since slice 3e.
        (
            "use external host::f\nfn main() -> Word { host::f(1) }",
            Identical,
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

/// Slice 3 of Order 6. A flat tuple pattern in a `let`, and a wildcard `let`, reach the
/// parser as the plain statements the reference lowers them to, through the lexer's
/// destructuring filter. The stale-flag cases are a defect slice 3 exposed: a whole-value
/// equality flag left by an earlier statement or branch made a later scalar `==` fold as an
/// array or tuple equality. Each was measured to diverge before the repair.
#[test]
fn destructuring_and_wildcard_lets_compile_and_stale_equality_flags_are_cleared() {
    let cases: &[(&str, Class)] = &[
        (
            "fn f() -> (Word, Word) { (1, 2) }\nfn main() -> Word { let (a, b) = f(); a + b }",
            Identical,
        ),
        (
            "fn f() -> (Word, Word) { (1, 2) }\nfn main() -> Word { let (a, _) = f(); a }",
            Identical,
        ),
        ("fn main() -> Word { let (a, b) = (3, 4); b }", Identical),
        (
            "fn f() -> (Word, Word, Word) { (1, 2, 3) }\nfn main() -> Word { let (_, b, c) = f(); b + c }",
            Identical,
        ),
        (
            "fn main() -> Word { let t = (3, 4); let (a, b) = t; a + b }",
            Identical,
        ),
        (
            "fn f(a: Word) -> (Word, Word) { (a, a) }\nfn main() -> Word { let (a, b) = f(if 1 > 0 { 2 } else { 3 }); a }",
            Identical,
        ),
        (
            "fn f(x: Word) -> Word { x }\nfn main() -> Word { let _ = f(1); 2 }",
            Identical,
        ),
        (
            "loop main(r: Word) -> Word { let _ = yield 0; 1 }",
            Identical,
        ),
        // Refused: a nested pattern and an annotated wildcard.
        (
            "fn f() -> ((Word, Word), Word) { ((1, 2), 3) }\nfn main() -> Word { let ((a, b), c) = f(); a }",
            Unsupported,
        ),
        ("fn main() -> Word { let _: Word = 3; 2 }", Unsupported),
        // Stale whole-value flags.
        ("fn main() -> bool { let a = [1, 2]; 1 == a[0] }", Identical),
        (
            "fn main() -> bool { let a = [1, 2]; a[0] == a[1] }",
            Identical,
        ),
        (
            "fn main() -> Word { let a: [Word; 2] = [1, 2]; if a[0] == 1 and a[1] == 2 { 1 } else { 0 } }",
            Identical,
        ),
        (
            "fn main() -> bool { let a = [1, 2]; let b = [1, 2]; let c = a == b; c }",
            Identical,
        ),
        (
            "fn main(c: bool) -> (Word, Word) { if c { (1, 2) } else { let x = 3; if x == 0 { (0, 0) } else { (x, x) } } }",
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
}

/// Slice 3b of Order 6, the pipe operator and literal parameter heads. `x |> f(a)` is the
/// reference's `f(x, a)`, and a single literal parameter `fn f(0)` is lowered as the
/// reference's pattern test, before any copy and with no copy of its own. A literal beside
/// another parameter, a literal with a `when` guard, and the pipe's `_` placeholder are
/// refused.
#[test]
fn the_pipe_and_literal_heads_compile_or_are_refused() {
    let cases: &[(&str, Class)] = &[
        (
            "fn d(x: Word) -> Word { x + x }\nfn a(x: Word, y: Word) -> Word { x + y }\nfn main() -> Word { 6 |> d() |> a(1) }",
            Identical,
        ),
        (
            "fn d(x: Word) -> Word { x + x }\nfn main() -> Word { 6 + 1 |> d() }",
            Identical,
        ),
        (
            "fn d(x: Word) -> Word { x + x }\nfn main() -> Word { let v = 3 |> d(); v }",
            Identical,
        ),
        (
            "fn d(x: Word) -> Word { x + x }\nfn a(x: Word, y: Word) -> Word { x + y }\nfn main() -> Word { a(2 |> d(), 5 |> d()) }",
            Identical,
        ),
        (
            "use host::f\nfn main() -> Word { 2 |> host::f() }",
            Identical,
        ),
        (
            "fn d(x: Word) -> Word { x + x }\nfn a(x: Word, y: Word) -> Word { x + y }\nfn main() -> Word { 6 |> a(_, 1) }",
            Unsupported,
        ),
        (
            "fn c(0) -> Word { 0 }\nfn c(1) -> Word { 1 }\nfn c(n: Word) -> Word { n + 10 }\nfn main() -> Word { c(3) }",
            Identical,
        ),
        (
            "fn w(0) -> Word { 2 }\nfn w(1) -> Word { 4 }\nfn main() -> Word { w(1) }",
            Identical,
        ),
        (
            "fn c(0) -> Word { let x = 5; x + 1 }\nfn c(n: Word) -> Word { let y = n; y }\nfn main() -> Word { c(3) }",
            Identical,
        ),
        (
            "fn f(0) -> Word { 1 }\nfn main() -> Word { f(0) }",
            Identical,
        ),
        (
            "yield g(0) -> Word { yield 1 }\nyield g(n: Word) -> Word { yield n }\nloop main(r: Word) -> Word { g(r) }",
            Identical,
        ),
        (
            "fn c(0, b: Word) -> Word { b }\nfn c(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { c(1, 2) }",
            Unsupported,
        ),
        (
            "fn c(0) -> Word when true { 0 }\nfn c(n: Word) -> Word { n }\nfn main() -> Word { c(3) }",
            Unsupported,
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

/// Three more skipped tokens, found while widening toward the Order 5 framing program. A
/// `use` path joined any two consecutive identifiers, so `use host::open` followed by
/// `require word >= 64;` became one path. A leading shebang line was lexed as source. And
/// the operator dispatch's last arm returned without a record for any token it did not
/// recognise, which dropped a `.` after an untyped value.
#[test]
fn use_paths_need_separators_shebangs_are_skipped_and_unclaimed_tokens_are_refused() {
    let cases: &[(&str, Class)] = &[
        (
            "use host::f\nrequire word >= 64;\nfn main() -> Word { host::f(1) }",
            Identical,
        ),
        (
            "#!/usr/bin/env keleusma\nfn main() -> Word { 7 }",
            Identical,
        ),
        // Identical since an untyped native's destructured elements read boxed, 2026-10-26.
        (
            "use host::f\nfn main() -> Word { let (a, b) = host::f(); a }",
            Identical,
        ),
        (
            "use host::f\nfn main() -> Word { let t = host::f(); t.0 }",
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

/// Slice 3d of Order 6, `for x in xs` over a static scalar array. The reference copies the
/// array into a fresh local, stores its length and then a zero counter, and binds `x` to
/// the element at the counter before the body. The stage writes that as a hidden `let`,
/// a bare range loop initialised end first, and a body opening with `let x = tmp[ctr]`.
#[test]
fn for_over_a_static_array_compiles() {
    let cases: &[(&str, Class)] = &[
        (
            "fn main() -> Word { let xs = [1, 2, 3]; for x in xs { let y = x; } 7 }",
            Identical,
        ),
        (
            "fn main() -> Word { let xs = [10, 20, 30, 40]; for x in xs { let _d = x * 2; } xs[3] }",
            Identical,
        ),
        (
            "fn f(a: [Word; 3]) -> Word { for x in a { let y = x; } 1 }\nfn main() -> Word { f([1, 2, 3]) }",
            Identical,
        ),
        (
            "fn main() -> Word { let xs = [1, 2]; for x in xs { for y in xs { let z = x + y; } } 0 }",
            Identical,
        ),
        (
            "fn main() -> Word { let xs: [Byte; 2] = [1 as Byte, 2 as Byte]; for x in xs { let y = x; } 0 }",
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
}

/// Slice 3e of Order 6, the function and import modifiers. `signed` on the entry sets the
/// module's requires-signature flag, `pure` and `ephemeral` change nothing the reference
/// emits for a program it accepts, and `use external` lowers its calls to
/// CallExternalNative. The reference keeps every rejection these modifiers carry.
#[test]
fn modifiers_compile_or_are_left_to_the_reference() {
    let cases: &[(&str, Class)] = &[
        ("signed fn main() -> Word { 21 + 21 }", Identical),
        (
            "fn helper() -> Word { 1 }\nsigned fn main() -> Word { helper() }",
            Identical,
        ),
        (
            "signed loop main(r: Word) -> Word { let _ = yield 1; 0 }",
            Identical,
        ),
        (
            "signed fn helper() -> Word { 1 }\nfn main() -> Word { helper() }",
            RefRejects,
        ),
        (
            "pure fn sq(x: Word) -> Word { x * x }\nfn main() -> Word { sq(3) }",
            Identical,
        ),
        ("ephemeral fn main() -> Word { 7 }", Identical),
        (
            "private data d { n: Word }\nephemeral fn main() -> Word { d.n = d.n + 1; d.n }",
            RefRejects,
        ),
        (
            "fn helper() -> Word { 1 }\nephemeral signed fn main() -> Word { helper() }",
            Identical,
        ),
        (
            "use host::g\nuse external host::f\nfn main() -> Word { host::f(1) + host::g(2) }",
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
}

/// Slice 4 of Order 6, trait impls and method calls. `impl Trait for Head { fn m .. }`
/// compiles each method as the chunk `Trait::Head::m`, sorted among the plain names, and
/// `recv.m(args)` calls the first such chunk for the receiver's type head with the receiver
/// as its first argument, as the reference resolves it. An impl whose methods are never
/// called compiles too, since the reference emits their chunks regardless.
#[test]
fn trait_impls_and_method_calls_compile() {
    let cases: &[(&str, Class)] = &[
        (
            "trait D { fn d(x: Word) -> Word; }\nimpl D for Word { fn d(x: Word) -> Word { x + 1 } }\nfn main(a: Word) -> Word { a.d() }",
            Identical,
        ),
        (
            "struct P { x: Word }\ntrait Get { fn get(p: P) -> Word; }\nimpl Get for P { fn get(p: P) -> Word { p.x } }\nfn main() -> Word { let p = P { x: 7 }; p.get() }",
            Identical,
        ),
        (
            "trait A { fn f(x: Word, y: Word) -> Word; }\nimpl A for Word { fn f(x: Word, y: Word) -> Word { x + y } }\nfn main() -> Word { let n: Word = 1; n.f(2) }",
            Identical,
        ),
        (
            "trait A { fn f(x: Word) -> Word; }\nimpl A for Word { fn f(x: Word) -> Word { x } }\nfn main() -> Word { 3 }",
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
}

/// Slice 2b of Order 6, `use` signatures. A signature's parameter types change nothing the
/// call lowers to, and the reference checks them. Its return type fixes the native's return
/// shape, `Scalar` for `Word`, `Byte`, `bool` and `()`, as the reference records it. A
/// tuple or other composite return is refused.
#[test]
fn use_signatures_set_scalar_return_shapes() {
    let cases: &[(&str, Class)] = &[
        (
            "use host::f(Word) -> Word\nfn main() -> Word { host::f(1) }",
            Identical,
        ),
        (
            "use host::g() -> ()\nfn main() -> Word { host::g(); 1 }",
            Identical,
        ),
        (
            "use host::b(Word) -> bool\nfn main() -> bool { host::b(1) }",
            Identical,
        ),
        (
            "use host::y(Byte) -> Byte\nfn main() -> Byte { host::y(1 as Byte) }",
            Identical,
        ),
        (
            "use host::f(Word, Word) -> Word\nuse host::g\nfn main() -> Word { host::f(1, 2) + host::g(3) }",
            Identical,
        ),
        (
            "use host::t(Word) -> (Word, Word)\nfn main() -> Word { let (a, b) = host::t(1); a }",
            Unsupported,
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

/// Slice 5 of Order 6, arithmetic on Float operands. The reference lowers `+`, `-` and `*`
/// on two Floats to the unchecked Add, Sub and Mul, and `/` and `%` to Div and Mod followed
/// by `PopN(2)`. The parser marks such an operation on the Byte arithmetic record, flagged
/// so its result stays a Float. A float literal, slice 5b, is lexed as its interned text,
/// which the host converts with the reference lexer's own conversion.
#[test]
fn float_arithmetic_on_typed_operands_compiles() {
    let cases: &[(&str, Class)] = &[
        ("fn main(a: Float, b: Float) -> Float { a + b }", Identical),
        (
            "fn main(a: Float, b: Float) -> Float { a * b - a / b }",
            Identical,
        ),
        ("fn main(a: Float, b: Float) -> Float { a % b }", Identical),
        (
            "fn f(a: Float) -> Float { a + a }\nfn main(x: Float) -> Float { let y = f(x); y * x }",
            Identical,
        ),
        (
            "fn main(a: Float, b: Float) -> bool { a + b < b }",
            Identical,
        ),
        ("fn main(a: Byte, b: Byte) -> Byte { a + b }", Identical),
        ("fn main() -> Float { 1.5 }", Identical),
        ("fn main(a: Float) -> Float { a + 2.25 }", Identical),
        ("fn main() -> Float { 1.5 + 1.5 * 2.0 }", Identical),
        ("fn main(a: Float) -> bool { a < 0.5 }", Identical),
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

/// Irrefutable destructuring arms, `(a, b) => ...` and `P { x, y } => ...`, which the reference
/// lowers as a wildcard arm that reads each bound field into a fresh slot in source order. A
/// literal sub-pattern, a nested composite field and a field rename stay refused, and the
/// match still needs its own wildcard arm, as the reference requires.
#[test]
fn destructuring_arms_compile() {
    const P: &str = "struct P { x: Word, y: Word, b: Byte }\n";
    let cases: &[(String, Class)] = &[
        (
            String::from(
                "fn main(t: (Word, Word)) -> Word { match t { (a, b) => a + b, _ => 0 } }",
            ),
            Identical,
        ),
        (
            String::from(
                "fn main(t: (Word, Word, Word)) -> Word { match t { (_, b, c) => b * c, _ => 0 } }",
            ),
            Identical,
        ),
        (
            String::from(
                "fn main(t: (Word, Byte)) -> Byte { match t { (_, b) => b + b, _ => 0 as Byte } }",
            ),
            Identical,
        ),
        (
            format!("{P}fn main(p: P) -> Word {{ match p {{ P {{ x, y }} => x + y, _ => 0 }} }}"),
            Identical,
        ),
        (
            format!("{P}fn main(p: P) -> Word {{ match p {{ P {{ y }} => y, _ => 0 }} }}"),
            Identical,
        ),
        (
            format!(
                "{P}fn main(p: P) -> Byte {{ match p {{ P {{ b }} => b + b, _ => 0 as Byte }} }}"
            ),
            Identical,
        ),
        (
            String::from("fn main(t: (Word, Word)) -> Word { match t { (1, b) => b, _ => 0 } }"),
            Unsupported,
        ),
        (
            format!("{P}fn main(p: P) -> Word {{ match p {{ P {{ x: 1 }} => 1, _ => 0 }} }}"),
            Unsupported,
        ),
        (
            format!("{P}fn main(p: P) -> Word {{ match p {{ P {{ x: a }} => a, _ => 0 }} }}"),
            Unsupported,
        ),
        (
            // Both reject a tuple match without a wildcard arm, and the pipeline's own coverage
            // check speaks first.
            String::from("fn main(t: (Word, Word)) -> Word { match t { (a, b) => a } }"),
            PipelineRejects,
        ),
        (
            String::from("fn main(a: Byte) -> Byte { match a { v => v + v } }"),
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
}

/// A variable arm on a scalar match, `v => ...`, which the reference lowers as a wildcard arm
/// that first copies the scrutinee into a fresh slot. A guarded variable arm and a variable
/// pattern on an enum scrutinee stay refused.
#[test]
fn variable_match_arms_compile() {
    let cases: &[(&str, Class)] = &[
        ("fn main(a: Word) -> Word { match a { v => v } }", Identical),
        (
            "fn main(a: Word) -> Word { match a { 1 => 2, v => v + 1 } }",
            Identical,
        ),
        ("fn main(a: Word) -> Word { match a { v => 3 } }", Identical),
        (
            "fn main(a: Word) -> Word { match a + 1 { 0 => 9, v => v * 2 } }",
            Identical,
        ),
        (
            "fn main(a: Word) -> Word { let x = match a { 0 => 1, v => v + a }; x + match a { w => w } }",
            Identical,
        ),
        (
            "fn main(a: Word) -> Word { match a { 1 => 2, v when v > 3 => v, _ => 0 } }",
            Unsupported,
        ),
        (
            "enum E { A, B }\nfn main(e: E) -> Word { match e { E::A => 1, x => 2 } }",
            Unsupported,
        ),
        // `true` and `false` reach the parser as identifiers. As patterns they are literals,
        // so they must not be taken for bindings.
        (
            "fn main(c: bool) -> Word { match c { true => 1, false => 0 } }",
            Unsupported,
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

/// Wildcard payloads in a variant pattern, `E::V(_)`, which the reference reads nothing for,
/// so only the field position advances and a later named payload still reads its own offset.
/// Also a named payload the arm never reads, which used to panic type-input extraction because
/// its slot had no identity.
#[test]
fn wildcard_and_unused_payloads_compile() {
    const E: &str =
        "enum S { Ok, Err(Word), P(Word, Word) }\nfn g(a: Word) -> S { S::P(a, a + 1) }\n";
    let cases: &[(String, Class)] = &[
        (
            format!(
                "{E}fn main() -> Word {{ match g(2) {{ S::Ok => 1, S::Err(_) => 0, S::P(a, b) => a }} }}"
            ),
            Identical,
        ),
        (
            format!(
                "{E}fn main() -> Word {{ match g(2) {{ S::Ok => 1, S::Err(c) => 0, S::P(a, b) => b }} }}"
            ),
            Identical,
        ),
        (
            format!(
                "{E}fn main() -> Word {{ match g(2) {{ S::Ok => 1, S::Err(c) => c, S::P(_, b) => b }} }}"
            ),
            Identical,
        ),
        (
            format!(
                "{E}fn main() -> Word {{ match g(2) {{ S::Ok => 1, S::Err(c) => c, S::P(a, _) => a }} }}"
            ),
            Identical,
        ),
        (
            format!(
                "{E}fn main() -> Word {{ match g(2) {{ S::Ok => 1, S::Err(_) => 0, S::P(_, _) => 2 }} }}"
            ),
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
}

/// A `break` in a bare `for`, one without `limit`. The parser took it for a `limit` loop's
/// break, so it stored an outcome code into slot 0, which held the first parameter, and its
/// `Break` was never patched and kept target 0. The shipping cross-check refused every such
/// program. A bare loop's `break` is now the `Break` alone, patched by the loop's `EndLoop`.
/// The nested cases pin that each break exits its own loop.
#[test]
fn a_break_in_a_bare_for_compiles() {
    let cases: &[(&str, Class)] = &[
        (
            "fn main(r: Word) -> Word { for i in 0..3 { if i == r { break; } } r }",
            Identical,
        ),
        (
            "fn main(r: Word) -> Word { for i in 0..3 { if i == r { break; } else { let y = i; } } r }",
            Identical,
        ),
        (
            "fn main(r: Word) -> Word { for i in 0..3 { for j in 0..2 { if j == r { break; } } if i == r { break; } } r }",
            Identical,
        ),
        (
            "fn main(r: Word) -> Word { for i in 0..3 limit 3 { for j in 0..2 { if j == r { break; } } if i == r { break; } } r }",
            Identical,
        ),
        (
            "fn main(r: Word) -> Word { for i in 0..3 { for j in 0..2 limit 2 { if j == r { break; } } if i == r { break; } } r }",
            Identical,
        ),
        (
            "loop main(r: Word) -> (Word, Word) { for i in 0..5 { if i == r { break; } } let _ = yield (0, 1); (0, 0) }",
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
}

/// An enum `match` in a stream chunk lowers to a loop whose every arm breaks and whose
/// no-match path traps. The reference charges such a single-pass loop the larger of that pass
/// and its costliest break, and `analyze.kel` charged the break alone, an execution bound 21
/// cycles below the reference's for these programs. The whole-module comparison includes the
/// bound, so Identical here pins the two analyses' agreement. A scalar `match`, which lowers
/// to `if` chains, is the control that always agreed.
#[test]
fn an_enum_match_in_a_stream_has_the_reference_execution_bound() {
    const TAIL: &str = "let _ = yield (0, 1); (0, 0) }";
    let cases: &[(String, Class)] = &[
        (
            format!(
                "enum S {{ A, B }}\nfn g(r: Word) -> S {{ if r == 0 {{ S::A }} else {{ S::B }} }}\n\
                 loop main(r: Word) -> (Word, Word) {{ let x = match g(r) {{ S::A => 1, S::B => 2 }}; {TAIL}"
            ),
            Identical,
        ),
        (
            format!(
                "enum S {{ A, B(Word) }}\nfn g(r: Word) -> S {{ if r == 0 {{ S::A }} else {{ S::B(r) }} }}\n\
                 loop main(r: Word) -> (Word, Word) {{ let x = match g(r) {{ S::A => 1, S::B(c) => c }}; {TAIL}"
            ),
            Identical,
        ),
        (
            format!(
                "loop main(r: Word) -> (Word, Word) {{ let x = match r {{ 0 => 1, _ => 2 }}; {TAIL}"
            ),
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
}

/// The RTOS task scripts as the RTOS crate compiles them, the prelude, a newline, then the
/// script, which is `build_module` in `examples/rtos/src/setup.rs`. Alone, a task script
/// names host functions only the prelude declares, so the corpus table above measures the
/// reference rejecting it. Joined, they are the sources gate 1 is about.
#[test]
fn rtos_scripts_joined_with_the_prelude_have_their_pinned_outcomes() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/rtos/scripts/");
    let prelude = std::fs::read_to_string(format!("{dir}prelude.kel")).expect("the prelude");
    let pinned: &[(&str, Class)] = &[
        // Identical since slice 8 accepted the prelude's tuple-returning signatures.
        ("event_listener.kel", Identical),
        ("faulty.kel", Identical),
        ("heartbeat.kel", Identical),
        // Identical since `analyze.kel` charges the trap path of a single-pass loop, which an
        // enum `match` lowers to. Its execution bound was 93 against the reference's 114.
        ("led.kel", Identical),
        ("sensor.kel", Identical),
    ];
    let mut tasks: Vec<String> = std::fs::read_dir(dir)
        .expect("the RTOS script directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|n| n.ends_with(".kel") && n != "prelude.kel")
        .collect();
    tasks.sort();
    let named: Vec<&str> = pinned.iter().map(|(n, _)| *n).collect();
    assert_eq!(
        tasks, named,
        "the RTOS task scripts changed, so update this table"
    );
    let mut mismatches = Vec::new();
    for (name, expected) in pinned {
        let task = std::fs::read_to_string(format!("{dir}{name}")).expect("a task script");
        let (actual, detail) = classify(&format!("{prelude}\n{task}"));
        println!("rtos {name} {actual:?} {detail}");
        if actual != *expected {
            mismatches.push(format!(
                "{name}: expected {expected:?}, got {actual:?} {detail}"
            ));
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}

/// `assert`, which the reference compiles out of a release build. The self-hosted parser
/// commits the statement as kind 83, which codegen lowers to its continuation alone, while the
/// condition's nodes still reach the type stage, so a condition naming an undefined binding is
/// still rejected. The message form is not lowered here and stays refused.
#[test]
fn assert_statements_compile_out() {
    let cases: &[(&str, Class)] = &[
        ("fn main(a: Word) -> Word { assert a > 0; a }", Identical),
        (
            "fn main(a: Word) -> Word { assert not (a == 3); assert a >= 0; a + 1 }",
            Identical,
        ),
        (
            "fn main(a: Word) -> Word { let b = a + 1; assert b > a; b }",
            Identical,
        ),
        (
            "fn main(a: Word) -> Word { if a > 2 { assert a > 1; a } else { 0 } }",
            Identical,
        ),
        (
            "fn main(a: Word) -> Word { assert missing > 0; a }",
            PipelineRejects,
        ),
        (
            "fn main(a: Word) -> Word { assert a > 0, \"positive\"; a }",
            Unsupported,
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

/// Destructuring an untyped library native's result. The reference reads each element
/// boxed, `GetTupleField(Boxed { index })`, because the value has no known layout, and the
/// self-hosted parser now does so for the destructuring filter's hidden binding. A hand-written
/// `t.0` on an untyped value is a reference rejection, the control that the boxed read is
/// confined to the form the reference admits.
#[test]
fn untyped_native_destructuring_reads_boxed() {
    let cases: &[(&str, Class)] = &[
        (
            "use shell::run\nfn main() -> Word { let (code, details) = shell::run(\"x\"); code }",
            Identical,
        ),
        (
            "use shell::run\nfn main() -> Word { let (code, _) = shell::run(\"x\"); code + 1 }",
            Identical,
        ),
        (
            "use shell::run\nfn main() -> Word { let t = shell::run(\"x\"); t.0 }",
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

/// A native call's value takes its declared scalar return, `Word`, `Byte` or `bool`, as the
/// reference types it from the `use` signature. Untyped, a tuple of native results had no
/// layout, which refused `rogue_dungen.kel`, and a Byte result could not select the Byte
/// operators.
#[test]
fn native_results_take_their_declared_scalar_type() {
    let cases: &[(&str, Class)] = &[
        (
            "use host::w() -> Word\nfn main() -> Word { let a = host::w(); let b = host::w(); let t = (a, b); t.1 }",
            Identical,
        ),
        (
            "use host::b() -> Byte\nfn main() -> Byte { host::b() + host::b() }",
            Identical,
        ),
        (
            "use host::b() -> Byte\nfn main() -> Byte { let x = host::b(); x band x }",
            Identical,
        ),
        (
            "use host::f() -> bool\nfn main() -> Word { if host::f() { 1 } else { 0 } }",
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
}

/// Slice 7 of Order 6, natives that return an enum. The host gives such a native the
/// reference's flat return shape, and the parser types the call's result as the enum, so a
/// `match` on it binds the variant payload. The reference requires every `use` before the
/// type definitions, so the enum is always declared after the `use`, as the RTOS prelude
/// declares `Status`. A tuple or struct return stays refused.
#[test]
fn natives_returning_an_enum_compile() {
    const ENUM: &str = "enum Status { Ok = 0, Err(Word) = 1 }\n";
    const USE: &str = "use host::gpio_set(Word, Word) -> Status\n";
    let cases: &[(String, Class)] = &[
        (
            format!(
                "{USE}{ENUM}fn main() -> Word {{ match host::gpio_set(1, 2) {{ Status::Ok => 0, Status::Err(c) => c }} }}"
            ),
            Identical,
        ),
        (
            format!(
                "{USE}{ENUM}fn main() -> Word {{ let s = host::gpio_set(1, 2); match s {{ Status::Ok => 0, Status::Err(c) => c }} }}"
            ),
            Identical,
        ),
        (
            format!(
                "{USE}use host::clock_now() -> Word\n{ENUM}fn main() -> Word {{ let t = host::clock_now(); match host::gpio_set(t, 2) {{ Status::Ok => t, Status::Err(c) => c }} }}"
            ),
            Identical,
        ),
        (
            format!(
                "use host::adc_read(Word) -> (Status, Word)\n{ENUM}fn main() -> Word {{ let (s, v) = host::adc_read(1); v }}"
            ),
            Unsupported,
        ),
        // Slice 8. A tuple-returning native that is declared and not called no longer blocks
        // the program, which is the RTOS prelude's situation.
        (
            format!(
                "use host::adc_read(Word) -> (Status, Word)\nuse host::spi_read(Word) -> (Status, Word)\n{USE}{ENUM}fn main() -> Word {{ match host::gpio_set(1, 2) {{ Status::Ok => 0, Status::Err(c) => c }} }}"
            ),
            Identical,
        ),
        (
            String::from(
                "use host::point() -> P\nstruct P { x: Word }\nfn main() -> Word { let p = host::point(); p.x }",
            ),
            Unsupported,
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

/// Slice 6 of Order 6, struct-typed data fields. Such a field is one data slot holding the
/// whole composite, which the reference reads with `GetData` and projects exactly as a struct
/// local. A struct declared after the data block is not yet known when the field is read, so
/// its projection stays refused.
#[test]
fn struct_typed_data_fields_compile() {
    const DECLS: &str = "struct G { x: Word }\nstruct F { a: Word, g: G, b: Byte }\n\
                         private data d { n: Word, f: F, c: Byte, e: Byte }\n";
    let cases: &[(String, Class)] = &[
        (
            format!("{DECLS}fn main() -> Word {{ d.n = 3; d.f.a + d.n }}"),
            Identical,
        ),
        (
            format!("{DECLS}fn main() -> Word {{ d.n = 3; d.f.g.x }}"),
            Identical,
        ),
        (
            format!("{DECLS}fn main() -> Byte {{ d.n = 3; d.f.b }}"),
            Identical,
        ),
        (
            format!("{DECLS}fn main() -> Word {{ d.n = 3; let s = d.f; s.a + s.g.x }}"),
            Identical,
        ),
        // A cast to Byte before a data read used to leave the Byte flag set, so the next
        // Word addition lowered to the unchecked Byte Add.
        (
            format!(
                "{DECLS}fn main() -> Word {{ d.c = 4 as Byte; d.f = F {{ a: 1, g: G {{ x: 5 }}, b: 2 as Byte }}; d.f.a + d.n }}"
            ),
            Identical,
        ),
        (
            format!("{DECLS}fn main() -> Byte {{ d.c = 4 as Byte; d.e = 5 as Byte; d.c + d.e }}"),
            Identical,
        ),
        (
            String::from(
                "private data d { n: Word, f: F }\nstruct F { a: Word }\nfn main() -> Word { d.n = 3; d.f.a }",
            ),
            Unsupported,
        ),
        // Two private blocks, so the pool places three composites at running offsets.
        (
            String::from(
                "struct F { a: Word, b: Word }\nprivate data d { n: Word, f: F, h: F }\n\
                 private data e { k: F }\nfn main() -> Word { d.n = 3; d.h.b + e.k.a }",
            ),
            Identical,
        ),
        // Shared slots precede the private ones, so a private composite's slot counts them,
        // a shared array taking one slot per element.
        (
            String::from(
                "struct F { a: Word }\nshared data s { m: Word, xs: [Word; 3] }\n\
                 private data d { n: Word, f: F }\nfn main() -> Word { d.n = s.m; d.f.a + s.xs[1] }",
            ),
            Identical,
        ),
        // A shared struct field takes a composite shared-layout entry, which the host does not
        // assemble, so its projection stays refused.
        (
            String::from(
                "struct F { a: Word, b: Word }\nshared data s { n: Word, f: F }\nfn main() -> Word { s.f.a }",
            ),
            Unsupported,
        ),
        // An array of structs carries no pool size record, so the cross-check refuses it.
        (
            String::from(
                "struct F { a: Word }\nprivate data d { n: Word, fs: [F; 2] }\nfn main() -> Word { d.n = 3; d.n }",
            ),
            Unsupported,
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

/// Slice 5c of Order 6, Float negation, the unchecked `Neg` in the reference, through a new
/// transport tag, 69, for that existing instruction. A Word negation keeps the checked form.
#[test]
fn float_negation_compiles() {
    let cases: &[(&str, Class)] = &[
        ("fn main(a: Float) -> Float { -a }", Identical),
        ("fn main(a: Float) -> Float { -a + 1.0 }", Identical),
        (
            "fn main(a: Float, b: Float) -> Float { -(a * b) + 1.0 }",
            Identical,
        ),
        ("fn main(a: Word) -> Word { -a }", Identical),
        // The reference lexes the saturation words as keywords, so outside a checked arm they
        // are not bindings either. This pins that a name of that spelling is not admitted.
        (
            "fn main() -> Word { let saturate_max = 3; saturate_max }",
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

/// THE GATE 2 CENSUS. One program, or more, for every form of the reference AST, with the
/// shipping outcome it measures pinned beside it. A form whose outcome is not Identical is
/// a gap Order 6 still owes, named here so it cannot be forgotten. Forms the reference
/// itself rejects in this release are pinned RefRejects, since they are not the self-hosted
/// compiler's to support.
///
/// The labels name `ast.rs` variants. `the_census_names_every_ast_variant` checks the list
/// against the enums in `src/ast.rs`, so a variant added there must gain a row here.
const AST_CENSUS: &[(&str, &str, Class)] = &[
    // --- Stmt ---
    ("Stmt::Let", "fn main() -> Word { let x = 1; x }", Identical),
    (
        "Stmt::For",
        "fn main() -> Word { for i in 0..3 { let y = i; } 0 }",
        Identical,
    ),
    (
        "Stmt::Break",
        "fn main() -> Word { for i in 0..3 limit 3 { break; } 0 }",
        Identical,
    ),
    (
        "Stmt::Break::BareLoop",
        "fn main(r: Word) -> Word { for i in 0..3 { if i == r { break; } } r }",
        Identical,
    ),
    (
        "Stmt::DataFieldAssign",
        "private data d { n: Word }\nfn main() -> Word { d.n = 3; d.n }",
        Identical,
    ),
    (
        "Stmt::DataFieldIndexAssign",
        "private data d { xs: [Word; 2] }\nfn main() -> Word { d.xs[0] = 3; d.xs[0] }",
        Identical,
    ),
    (
        "Stmt::Expr",
        "fn g(x: Word) -> Word { x }\nfn main() -> Word { g(1); 0 }",
        Identical,
    ),
    (
        "Stmt::Assert",
        "fn main(a: Word) -> Word { assert a > 0; a }",
        Identical,
    ),
    // --- Iterable ---
    (
        "Iterable::Range",
        "fn main() -> Word { for i in 0..3 { let y = i; } 0 }",
        Identical,
    ),
    (
        "Iterable::Expr",
        "fn main() -> Word { let xs = [1, 2]; for x in xs { let y = x; } 0 }",
        Identical,
    ),
    // --- Expr ---
    ("Expr::Literal", "fn main() -> Word { 7 }", Identical),
    ("Expr::Ident", "fn main(a: Word) -> Word { a }", Identical),
    (
        "Expr::BinOp",
        "fn main(a: Word) -> Word { a + 1 }",
        Identical,
    ),
    (
        "Expr::UnaryOp",
        "fn main(a: Word) -> Word { -a }",
        Identical,
    ),
    (
        "Expr::Call",
        "fn g(x: Word) -> Word { x }\nfn main() -> Word { g(1) }",
        Identical,
    ),
    (
        "Expr::Pipeline",
        "fn g(x: Word) -> Word { x }\nfn main() -> Word { 1 |> g() }",
        Identical,
    ),
    (
        "Expr::Yield",
        "loop main(r: Word) -> Word { let _ = yield 1; 0 }",
        Identical,
    ),
    (
        "Expr::If",
        "fn main(c: bool) -> Word { if c { 1 } else { 0 } }",
        Identical,
    ),
    (
        "Expr::Match",
        "fn main(a: Word) -> Word { match a { 1 => 2, _ => 3 } }",
        Identical,
    ),
    (
        "Expr::Loop",
        "loop main(r: Word) -> Word { let _ = yield r; 0 }",
        Identical,
    ),
    (
        "Expr::FieldAccess",
        "struct P { x: Word }\nfn main(p: P) -> Word { p.x }",
        Identical,
    ),
    (
        "Expr::MethodCall",
        "trait D { fn d(x: Word) -> Word; }\nimpl D for Word { fn d(x: Word) -> Word { x } }\nfn main(a: Word) -> Word { a.d() }",
        Identical,
    ),
    (
        "Expr::TupleIndex",
        "fn main(t: (Word, Word)) -> Word { t.1 }",
        Identical,
    ),
    (
        "Expr::ArrayIndex",
        "fn main() -> Word { let a = [1, 2]; a[0] }",
        Identical,
    ),
    (
        "Expr::StructInit",
        "struct P { x: Word }\nfn main() -> Word { let p = P { x: 1 }; p.x }",
        Identical,
    ),
    (
        "Expr::EnumVariant",
        "enum E { A, B }\nfn main() -> Word { E::B as Word }",
        Identical,
    ),
    (
        "Expr::ArrayLiteral",
        "fn main() -> [Word; 2] { [1, 2] }",
        Identical,
    ),
    (
        "Expr::TupleLiteral",
        "fn main() -> (Word, Word) { (1, 2) }",
        Identical,
    ),
    (
        "Expr::Cast",
        "fn main(a: Byte) -> Word { a as Word }",
        Identical,
    ),
    (
        "Expr::Placeholder",
        "fn g(x: Word, y: Word) -> Word { x }\nfn main() -> Word { 1 |> g(_, 2) }",
        Unsupported,
    ),
    (
        "Expr::Closure",
        "fn main() -> Word { let f = |x| x; 1 }",
        RefRejects,
    ),
    (
        "Expr::Checked",
        "fn main(a: Word) -> Word { a + 1 { ok(v) => v, overflow(_, l) => l, underflow(_, l) => l } }",
        Identical,
    ),
    (
        "Expr::DataIndexStore",
        "private data d { xs: [Word; 2] }\nfn main(i: Word) -> Word { d.xs[i] = 3; 0 }",
        Identical,
    ),
    (
        // The arm words read as undefined names, a false source rejection, until slice 5d
        // lowered them to the Word bound constants the reference emits.
        "Expr::SaturateMax",
        "private data d { n: Word }\nfn main() -> Word { d.n = d.n + 1 { ok(v) => v, overflow(_, _) => saturate_max, underflow(_, _) => saturate_min }; d.n }",
        Identical,
    ),
    (
        "Expr::Classify",
        "fn main(a: Word) -> Word { let b = classify a @Secret; declassify b @Secret }",
        Unsupported,
    ),
    (
        "Expr::Declassify",
        "fn main(a: Word) -> Word { let b = classify a @Secret; declassify b @Secret }",
        Unsupported,
    ),
    (
        "Expr::SaturateMin",
        "private data d { n: Word }\nfn main() -> Word { d.n = d.n - 1 { ok(v) => v, overflow(_, _) => saturate_max, underflow(_, _) => saturate_min }; d.n }",
        Identical,
    ),
    // `ClosureRef` is synthesized by the hoist pass from a closure and has no surface form
    // of its own, so its row is the closure program, which the reference rejects today.
    (
        "Expr::ClosureRef",
        "fn main() -> Word { let f = |x| x; 1 }",
        RefRejects,
    ),
    // --- Pattern ---
    (
        "Pattern::Literal",
        "fn main(a: Word) -> Word { match a { 1 => 2, _ => 3 } }",
        Identical,
    ),
    (
        "Pattern::Wildcard",
        "fn main(a: Word) -> Word { match a { _ => 3 } }",
        Identical,
    ),
    (
        "Pattern::Variable",
        "fn main(a: Word) -> Word { match a { v => v } }",
        Identical,
    ),
    (
        "Pattern::Enum",
        "enum E { A, B }\nfn main(e: E) -> Word { match e { E::A => 1, E::B => 2 } }",
        Identical,
    ),
    // A wildcard in a variant's payload position. Found while lowering enum-returning natives,
    // where the RTOS scripts write `Status::Err(_)`.
    (
        "Pattern::Enum::WildcardPayload",
        "enum S { Ok, Err(Word) }\nfn g(a: Word) -> S { S::Err(a) }\nfn main() -> Word { match g(2) { S::Ok => 1, S::Err(_) => 0 } }",
        Identical,
    ),
    (
        "Pattern::Struct",
        "struct P { x: Word }\nfn main(p: P) -> Word { match p { P { x } => x, _ => 0 } }",
        Identical,
    ),
    (
        "Pattern::Tuple",
        "fn main(t: (Word, Word)) -> Word { match t { (a, b) => a, _ => 0 } }",
        Identical,
    ),
    // --- TypeExpr ---
    (
        "TypeExpr::Prim::Word",
        "fn main(a: Word) -> Word { a }",
        Identical,
    ),
    (
        "TypeExpr::Prim::Byte",
        "fn main(a: Byte) -> Byte { a }",
        Identical,
    ),
    (
        "TypeExpr::Prim::Bool",
        "fn main(a: bool) -> bool { a }",
        Identical,
    ),
    (
        "TypeExpr::Prim::Float",
        "fn main(a: Float) -> Float { a }",
        Identical,
    ),
    (
        "TypeExpr::Prim::Fixed",
        "fn main(a: Fixed<8>) -> Fixed<8> { a }",
        Identical,
    ),
    (
        "TypeExpr::Prim::Text",
        "fn main(a: Text) -> Word { 1 }",
        Identical,
    ),
    (
        "TypeExpr::Multiword",
        "fn main(a: Multiword<2>) -> Multiword<2> { a }",
        Unsupported,
    ),
    (
        "TypeExpr::TextN",
        "fn main(a: Text<8>) -> Word { 1 }",
        RefRejects,
    ),
    (
        "TypeExpr::Labelled",
        "fn main(a: Word@Secret) -> Word@Secret { a }",
        Unsupported,
    ),
    (
        "TypeExpr::NegativeLabelled",
        "fn main(a: Word@!Secret) -> Word { a }",
        Unsupported,
    ),
    (
        "TypeExpr::Named",
        "struct P { x: Word }\nfn main(p: P) -> Word { p.x }",
        Identical,
    ),
    (
        "TypeExpr::Tuple",
        "fn main(t: (Word, Word)) -> Word { t.0 }",
        Identical,
    ),
    (
        "TypeExpr::Array",
        "fn main(a: [Word; 2]) -> Word { a[0] }",
        Identical,
    ),
    ("TypeExpr::Unit", "fn main() -> () { () }", Identical),
    (
        "TypeExpr::Option",
        "fn main(a: Option<Word>) -> Word { 1 }",
        Unsupported,
    ),
    // --- Declarations ---
    (
        "Decl::Struct",
        "struct P { x: Word }\nfn main() -> Word { 1 }",
        Identical,
    ),
    (
        "Decl::Enum",
        "enum E { A, B }\nfn main() -> Word { 1 }",
        Identical,
    ),
    (
        "Decl::EnumPayload",
        "enum E { A(Word), B }\nfn main() -> Word { let e = E::A(3); match e { E::A(v) => v, E::B => 0 } }",
        Identical,
    ),
    (
        "Decl::Newtype",
        "newtype C = Word;\nfn main() -> Word { let c = C(5); c as Word }",
        Unsupported,
    ),
    (
        "Decl::Trait",
        "trait T { fn f(x: Word) -> Word; }\nfn main() -> Word { 1 }",
        Identical,
    ),
    (
        "Decl::Impl",
        "trait T { fn f(x: Word) -> Word; }\nimpl T for Word { fn f(x: Word) -> Word { x } }\nfn main() -> Word { 1 }",
        Identical,
    ),
    (
        "Decl::Data",
        "shared data d { n: Word }\nfn main() -> Word { d.n }",
        Identical,
    ),
    (
        "Decl::ConstData",
        "const data c { n: Word = 4 }\nfn main() -> Word { c.n }",
        Identical,
    ),
    (
        "Decl::Use",
        "use host::f\nfn main() -> Word { host::f(1) }",
        Identical,
    ),
    (
        "Decl::Require",
        "require word >= 64;\nfn main() -> Word { 1 }",
        Identical,
    ),
    (
        "Decl::Generic",
        "fn id<T>(x: T) -> T { x }\nfn main() -> Word { id(1) }",
        Unsupported,
    ),
    (
        "Decl::Multihead",
        "fn c(0) -> Word { 0 }\nfn c(n: Word) -> Word { n }\nfn main() -> Word { c(1) }",
        Identical,
    ),
];

/// The enums of `src/ast.rs` the census covers, with the label prefix each variant takes.
/// `PrimType` is reached through `TypeExpr::Prim`, so its rows read `TypeExpr::Prim::Word`.
const CENSUS_ENUMS: &[(&str, &str)] = &[
    ("Stmt", "Stmt"),
    ("Iterable", "Iterable"),
    ("Expr", "Expr"),
    ("Pattern", "Pattern"),
    ("TypeExpr", "TypeExpr"),
    ("PrimType", "TypeExpr::Prim"),
];

/// The variant names of `pub enum <name>` in `src/ast.rs`, read as the four-space-indented
/// identifiers between the header and the closing brace. An enum that is not found, or that
/// yields no variant, fails here, so a renamed enum or a reformatted file cannot pass by
/// reading nothing.
fn ast_variants(ast: &str, name: &str) -> Vec<String> {
    let header = format!("pub enum {name} ");
    let start = ast
        .lines()
        .position(|l| l.starts_with(&header))
        .unwrap_or_else(|| panic!("`{header}` is not in src/ast.rs"));
    let mut out = Vec::new();
    for line in ast.lines().skip(start + 1) {
        if line.starts_with('}') {
            break;
        }
        let Some(rest) = line.strip_prefix("    ") else {
            continue;
        };
        let ident: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if ident.starts_with(|c: char| c.is_ascii_uppercase()) {
            out.push(ident);
        }
    }
    assert!(!out.is_empty(), "read no variant of {name} from src/ast.rs");
    out
}

#[test]
fn the_census_names_every_ast_variant() {
    let ast = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/ast.rs"))
        .expect("src/ast.rs is readable");
    let mut missing = Vec::new();
    let mut seen = 0;
    for (name, prefix) in CENSUS_ENUMS {
        for variant in ast_variants(&ast, name) {
            seen += 1;
            let label = format!("{prefix}::{variant}");
            let nested = format!("{label}::");
            if !AST_CENSUS
                .iter()
                .any(|(l, _, _)| *l == label || l.starts_with(&nested))
            {
                missing.push(label);
            }
        }
    }
    // Non-vacuity. The six enums held 59 variants when this check was written.
    assert!(seen >= 59, "read only {seen} variants from src/ast.rs");
    assert!(
        missing.is_empty(),
        "AST variants without a census row: {missing:?}"
    );
}

#[test]
fn the_ast_census_has_its_pinned_outcomes() {
    let mut mismatches = Vec::new();
    let mut counts: BTreeMap<Class, usize> = BTreeMap::new();
    for (label, src, expected) in AST_CENSUS {
        let (actual, detail) = classify(src);
        *counts.entry(actual).or_default() += 1;
        if actual != *expected {
            mismatches.push(format!(
                "{label}: expected {expected:?}, got {actual:?} {detail}"
            ));
        }
    }
    println!("ast census {counts:?}");
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
