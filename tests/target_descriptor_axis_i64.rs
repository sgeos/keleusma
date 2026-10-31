#![cfg(all(feature = "compile", feature = "verify"))]
//! The `i64` row of the target-descriptor runtime grid, and the sweep on the
//! build's default runtime.
//!
//! Every target descriptor the compiler accepts, compiled against the corpus in
//! `tests/target_descriptor_axis/harness.rs` and loaded on the eight runtimes
//! whose word is `i64`, asking whether any cell reaches a run-time
//! `InvalidBytecode`. The harness module documents the axis, the cell outcomes
//! and the reason the grid is split across four binaries.
//!
//! The default-runtime sweep lives here because the build's default `Vm` has an
//! `i64` word at the default build, so its instantiation adds nothing this
//! binary does not already compile there.

extern crate alloc;

#[path = "target_descriptor_axis/harness.rs"]
mod harness;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use harness::{
    Cell, Handle, PRELUDE, Shape, Tally, WORD_FLOOR, assert_every_grid_file_is_present,
    assert_grid_row, corpus, sweep_word, targets,
};
use keleusma::bytecode::GenericValue;
use keleusma::compiler::compile_with_target;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::target::Target;
use keleusma::vm::{VmError, VmState};
use keleusma::{Arena, host_arc};

fn run_cell(target: &Target, shape: &Shape) -> Cell {
    let full = format!("{PRELUDE}{}", shape.src);
    let Ok(tokens) = tokenize(&full) else {
        return Cell::CompileRefused;
    };
    let Ok(program) = parse(&tokens) else {
        return Cell::CompileRefused;
    };
    let Ok(module) = compile_with_target(&program, target) else {
        return Cell::CompileRefused;
    };
    let arena = Arena::with_capacity(8192);
    let Ok(mut vm) = keleusma::vm::Vm::new(module, &arena) else {
        return Cell::LoadRefused;
    };
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle)))
    });
    vm.register_native("handle_val", |_args| Ok(GenericValue::Int(100)));
    match vm.call(&[]) {
        Ok(VmState::Finished(GenericValue::Int(n))) => {
            // `Word::to_i64` rather than `i64::from`: the runtime word is `i64`
            // at the default build and NARROWER at every `narrow-word-*` one, so
            // the `From` form is a useless conversion in one configuration and
            // the only correct one in the others. The trait method is right in
            // all of them and needs no lint exemption to say so.
            let got = keleusma::word::Word::to_i64(n);
            if got == shape.expect {
                Cell::Ran
            } else {
                Cell::WrongAnswer(got)
            }
        }
        Ok(other) => Cell::OtherFault(format!("unexpected final state: {other:?}")),
        Err(VmError::InvalidBytecode(m)) => Cell::InvalidBytecodeAtRuntime(m),
        Err(e) => Cell::OtherFault(format!("{e:?}")),
    }
}

/// **THE SWEEP.**
///
/// The assertion is narrow on purpose: a run-time `InvalidBytecode` is a
/// finding, and every other outcome is reported but permitted. Refusal is the
/// correct and expected majority outcome, and a sweep that treated it as a
/// defect would be wrong about its own subject.
#[test]
fn no_accepted_target_lets_an_ordinary_program_reach_invalid_bytecode() {
    let mut findings: Vec<String> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    let mut not_run: Vec<String> = Vec::new();
    let (mut ran, mut compile_refused, mut load_refused, mut other) = (0, 0, 0, 0);

    for (label, target) in targets() {
        for shape in corpus() {
            match run_cell(&target, &shape) {
                Cell::Ran => ran += 1,
                Cell::CompileRefused => {
                    compile_refused += 1;
                    // **NAME THE SHAPE, NOT JUST THE COUNT.** A shape refused
                    // under every descriptor contributes no evidence, and a bare
                    // total cannot distinguish that from one refused nowhere.
                    not_run.push(format!("{label} / {} (compile)", shape.name));
                }
                Cell::LoadRefused => {
                    load_refused += 1;
                    not_run.push(format!("{label} / {} (load)", shape.name));
                }
                Cell::OtherFault(m) => {
                    other += 1;
                    not_run.push(format!("{label} / {}: {m}", shape.name));
                }
                Cell::WrongAnswer(got) => wrong.push(format!(
                    "{label} / {}: expected {}, got {got}",
                    shape.name, shape.expect
                )),
                Cell::InvalidBytecodeAtRuntime(m) => {
                    findings.push(format!("{label} / {}: {m}", shape.name))
                }
            }
        }
    }

    // **NON-VACUITY.** A sweep in which nothing ran proves nothing about the
    // runtime, and one whose descriptor set has collapsed to a single point is
    // not varying what it claims to vary.
    //
    // The first draft ALSO required at least one refusal, on the assumption that
    // some admissible descriptor would be rejected for these programs. **That was
    // wrong, and the sweep said so on its first run**: every descriptor the
    // compiler accepts compiles and loads every shape in this corpus. Requiring a
    // refusal asserted a property that had not been measured, in a test written
    // to measure properties -- so the check now constrains the SWEEP's spread,
    // which is what non-vacuity actually needs, and the refusal counts are
    // reported rather than demanded.
    assert!(
        ran > 0,
        "no cell ran a program to completion, so this sweep measured nothing about the runtime"
    );
    let spread = targets();
    assert!(
        spread.len() > 1
            && spread.iter().any(|(_, t)| t.word_bits_log2 == WORD_FLOOR)
            && spread
                .iter()
                .any(|(_, t)| t.word_bits_log2 == keleusma::bytecode::RUNTIME_WORD_BITS_LOG2),
        "the descriptor set does not span the runtime's width range, so the sweep is not \
         varying the axis it names ({} descriptors)",
        spread.len()
    );

    assert!(
        findings.is_empty(),
        "a module that COMPILED and LOADED raised InvalidBytecode, which is the class verify() \
         exists to exclude:\n{}\n(ran {ran}, compile-refused {compile_refused}, load-refused \
         {load_refused}, other {other})",
        findings.join("\n")
    );
    assert!(
        wrong.is_empty(),
        "a module that compiled and loaded returned the WRONG ANSWER, which is worse than a \
         fault because nothing reports it:\n{}",
        wrong.join("\n")
    );

    // **THE RESULT, PINNED RATHER THAN DESCRIBED.** Measured 2026-09-10 at the default build:
    // every one of the 288 cells RAN and returned the expected value. Nothing was refused at
    // either boundary and nothing faulted.
    //
    // A refusal is a SAFE outcome and this is not asserting that refusals are wrong. It asserts
    // that the sweep's shape has not changed silently: if a descriptor starts being refused, or a
    // shape stops running, that is worth a reader's attention even though neither is a defect.
    // The figure is derived from the corpus and descriptor set rather than written as a literal,
    // so adding either moves it.
    // **THE EXPECTED TOTAL IS NOW PER CELL, NOT A PRODUCT.** It was
    // `descriptors * shapes` while every shape could run under every
    // descriptor. A float-using shape cannot run where the descriptor declares
    // no floats, and its refusal is CORRECT; a product would report that as the
    // sweep having changed shape.
    let expected_to_run: usize = targets()
        .iter()
        .map(|(_, t)| {
            corpus()
                .iter()
                .filter(|s| !s.needs_floats || t.has_floats)
                .count()
        })
        .sum();
    assert_eq!(
        ran,
        expected_to_run,
        "not every accepted descriptor ran every shape: {ran} of {} cells ran ({compile_refused} \
         compile-refused, {load_refused} load-refused, {other} other). None of those outcomes is \
         unsafe, but the sweep's shape has changed and the document describing it is now stale. \
         The cells that did not run:\n{}",
        expected_to_run,
        not_run.join("\n")
    );
}

/// The `i64` row of the grid, paired with every implemented address on both
/// float runtimes.
#[test]
fn no_runtime_and_module_width_pair_reaches_invalid_bytecode() {
    let mut t = Tally::default();
    sweep_word::<i64>(&mut t);
    assert_grid_row(&t, "i64", false);
}

/// The other three rows still exist and still sweep their word types.
#[test]
fn every_word_row_of_the_grid_is_present() {
    assert_every_grid_file_is_present();
}
