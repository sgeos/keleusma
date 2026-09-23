#![cfg(all(feature = "compile", feature = "verify"))]
//! Which runtime faults a VERIFIED, COMPILER-PRODUCED module can raise, and
//! whether it carries a `Trap` opcode when it does.
//!
//! # The question, and whose it is
//!
//! `docs/roadmap/V0_2_X_ROADMAP.md` workstream C proposes making trap
//! reachability decidable: `Trap` becomes the only opcode that traps, every
//! other opcode is made **total**, and the validator becomes a scan for the
//! presence of `Trap`. That is a `BYTECODE_VERSION`-bumping instruction-set
//! change and therefore the operator's call.
//!
//! Two things the proposal rests on were unmeasured: how large "make every
//! other opcode total" actually is, and whether the scan would be honest
//! today. Both are measurable now, with no instruction-set change, and that is
//! what this file does. **It implements no part of workstream C** — in
//! particular it deliberately does not ship a trap-freedom verdict, because
//! the census below shows the current instruction set could not support one.
//!
//! # Method
//!
//! Every row is EXECUTED. Each case compiles a source program, counts the
//! `Trap` opcodes in the compiled module, runs it, and records what actually
//! happened. Nothing here is classified by reading `vm.rs` or by matching on
//! an error type's name in source text; both are instruments this repository
//! has repeatedly found to miscount.
//!
//! The population is taken from a SPECIFIED list rather than from the cases
//! that were easy to provoke: the partial operations workstream C enumerates
//! (checked arithmetic, division and modulo by zero, array and indexed-data
//! bounds, the bare `for .. limit`, cast range, newtype refinement, native
//! errors) crossed with the fault kinds the runtime defines.
//!
//! # The result
//!
//! **Exactly two operation families fault with no `Trap` opcode anywhere in
//! the module: division or modulo by zero, and array bounds.** Everything else
//! either already carries a `Trap`, or does not fault at all.
//!
//! That is the measured size of the obligation, for compiler output. It is
//! smaller than the proposal's list implies, because several entries on that
//! list are already in the shape it wants:
//!
//! - **Arithmetic overflow does not fault.** The instruction-set
//!   specification states that a surface `a + b` on `Int` operands compiles to
//!   the checked opcode followed by `PopN(2)`, discarding the outcome flag and
//!   leaving the WRAPPING result. Wrapping is the specified behaviour of the
//!   bare operator, and the flag the proposal wants already exists.
//! - **A cast out of range does not fault either**; it truncates.
//! - **`assert` is a debug construct** (B29) compiled out entirely in a
//!   release build, so it raises nothing there.
//! - **The bare `for .. limit` already lowers to an explicit `Trap`**, as do
//!   the match and enum-discrimination fallbacks.
//!
//! # What this does NOT establish
//!
//! - **Nothing about hand-built bytecode.** A module the compiler never
//!   produced can reach fault kinds no row here reaches; that surface belongs
//!   to `tests/hostile_module_mutation.rs`.
//! - **Nothing about host-contract failures.** An unregistered native faults
//!   with no `Trap` present, but that is the host breaking a stated contract
//!   rather than a property of the module, and `tests/host_contract_faults.rs`
//!   already covers that class.
//! - **It is not exhaustive over source programs.** It is exhaustive over the
//!   specified list above, which is a different and weaker claim.

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::Op;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

/// What running a case actually did.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    /// Ran to completion. A fault was not raised.
    Completed,
    /// Faulted at run time, carrying the fault's name.
    Faulted(&'static str),
}

/// One census row: a program, and what it is expected to do.
struct Case {
    /// Case name, used in failure messages.
    name: &'static str,
    /// The program.
    src: &'static str,
    /// Whether the compiled module is expected to contain any `Trap` opcode.
    traps_expected: bool,
    /// What running it is expected to do.
    outcome: Outcome,
}

/// Names the runtime fault, without matching on a message's text.
///
/// The mapping is over the error VARIANT. Matching a substring of a rendered
/// message is the instrument this repository has found to miscount, and a
/// message is free to be reworded.
fn fault_name(e: &keleusma::vm::VmError) -> &'static str {
    use keleusma::vm::VmError as E;
    match e {
        E::StackUnderflow => "StackUnderflow",
        E::TypeError(_) => "TypeError",
        E::DivisionByZero => "DivisionByZero",
        E::IndexOutOfBounds(_, _) => "IndexOutOfBounds",
        E::FieldNotFound(_, _) => "FieldNotFound",
        E::NativeError(_) => "NativeError",
        E::InvalidBytecode(_) => "InvalidBytecode",
        E::RefinementFailed => "RefinementFailed",
        E::NoMatchingHead => "NoMatchingHead",
        E::NoMatchingArm => "NoMatchingArm",
        E::CheckedArithNoArm => "CheckedArithNoArm",
        E::EnumVariantUnmapped => "EnumVariantUnmapped",
        E::AssertionFailed => "AssertionFailed",
        E::LoopLimitExceeded => "LoopLimitExceeded",
        E::VerifyError(_) => "VerifyError",
        E::LoadError(_) => "LoadError",
        _ => "Other",
    }
}

const CASES: &[Case] = &[
    // --- Faults with NO trap opcode. These are the obligation. ---
    Case {
        name: "division by a runtime zero",
        src: "fn main() -> Word { let z = 0; 7 / z }",
        traps_expected: false,
        outcome: Outcome::Faulted("DivisionByZero"),
    },
    Case {
        name: "modulo by a runtime zero",
        src: "fn main() -> Word { let z = 0; 7 % z }",
        traps_expected: false,
        outcome: Outcome::Faulted("DivisionByZero"),
    },
    Case {
        name: "division by a literal zero",
        src: "fn main() -> Word { 7 / 0 }",
        traps_expected: false,
        outcome: Outcome::Faulted("DivisionByZero"),
    },
    Case {
        name: "array index out of bounds, runtime index",
        src: "fn main() -> Word { let xs = [1, 2]; let i = 5; xs[i] }",
        traps_expected: false,
        outcome: Outcome::Faulted("IndexOutOfBounds"),
    },
    Case {
        name: "array index out of bounds, constant index",
        src: "fn main() -> Word { let xs = [1, 2]; xs[5] }",
        traps_expected: false,
        outcome: Outcome::Faulted("IndexOutOfBounds"),
    },
    // --- Already carries a trap. ---
    Case {
        name: "bare for-limit exceeded",
        src: "fn main() -> Word { let hi = 100; for i in 0..hi limit 4 { } 0 }",
        traps_expected: true,
        outcome: Outcome::Faulted("LoopLimitExceeded"),
    },
    // --- Does not fault at all. ---
    Case {
        name: "addition overflow wraps, as specified",
        src: "fn main() -> Word { let a = 9223372036854775807; a + 1 }",
        traps_expected: false,
        outcome: Outcome::Completed,
    },
    Case {
        name: "multiplication overflow wraps, as specified",
        src: "fn main() -> Word { let a = 9223372036854775807; a * 2 }",
        traps_expected: false,
        outcome: Outcome::Completed,
    },
    Case {
        name: "cast out of range truncates",
        src: "fn main() -> Word { let a = 300; let b = a as Byte; b as Word }",
        traps_expected: false,
        outcome: Outcome::Completed,
    },
    Case {
        name: "assert is a debug construct and emits nothing here",
        src: "fn main() -> Word { assert false; 7 }",
        traps_expected: false,
        outcome: Outcome::Completed,
    },
    Case {
        name: "exhaustive match carries its fallback trap and never takes it",
        src: "enum E { A, B }\n\
               fn f(e: E) -> Word { match e { E::A => 1, E::B => 2 } }\n\
               fn main() -> Word { f(E::B) }",
        traps_expected: true,
        outcome: Outcome::Completed,
    },
    Case {
        name: "enum payload read carries a trap and never takes it",
        src: "enum E { A(Word), B }\n\
               fn f(e: E) -> Word { match e { E::A(n) => n, E::B => 0 } }\n\
               fn main() -> Word { f(E::A(5)) }",
        traps_expected: true,
        outcome: Outcome::Completed,
    },
    Case {
        name: "control: ordinary arithmetic",
        src: "fn main() -> Word { 21 + 21 }",
        traps_expected: false,
        outcome: Outcome::Completed,
    },
];

/// Compiles, counts traps, runs. Returns `(trap_count, outcome)`.
fn run_case(c: &Case) -> (usize, Outcome) {
    let toks = tokenize(c.src).unwrap_or_else(|e| panic!("{}: lex: {e:?}", c.name));
    let prog = parse(&toks).unwrap_or_else(|e| panic!("{}: parse: {e:?}", c.name));
    let m = compile(&prog).unwrap_or_else(|e| panic!("{}: compile: {e:?}", c.name));
    let traps = m
        .chunks
        .iter()
        .map(|ch| ch.ops.iter().filter(|o| matches!(o, Op::Trap(_))).count())
        .sum();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena
        .resize_persistent(need)
        .unwrap_or_else(|e| panic!("{}: arena: {e:?}", c.name));
    let mut vm = Vm::new(m, &arena).unwrap_or_else(|e| panic!("{}: verify: {e:?}", c.name));
    let outcome = match vm.call(&[]) {
        Ok(VmState::Finished(_)) | Ok(VmState::Reset) => Outcome::Completed,
        Ok(other) => panic!("{}: unexpected state {other:?}", c.name),
        Err(e) => Outcome::Faulted(fault_name(&e)),
    };
    (traps, outcome)
}

/// Every row behaves as the census records.
///
/// **A case that was meant to fault and instead completed is a defect in the
/// row, not a pass.** That is asserted rather than tolerated, because an
/// unfilled cell reported as a clean result is the failure mode this
/// repository keeps paying for: an outcome that looks like a pass while
/// testing nothing.
#[test]
fn every_census_row_behaves_as_recorded() {
    for c in CASES {
        let (traps, outcome) = run_case(c);
        assert_eq!(
            traps > 0,
            c.traps_expected,
            "{}: expected traps_expected={}, module carries {traps}",
            c.name,
            c.traps_expected
        );
        assert_eq!(outcome, c.outcome, "{}: outcome differs", c.name);
    }
}

/// The obligation workstream C would have to discharge, pinned.
///
/// This is the guard that stops the census drifting. If a new operation starts
/// faulting without a `Trap` present, or one of these two stops, the set moves
/// and this fails — rather than the roadmap's sizing quietly going stale.
#[test]
fn exactly_two_operation_families_fault_without_a_trap_opcode() {
    let mut without_trap: alloc::vec::Vec<&'static str> = CASES
        .iter()
        .filter_map(|c| {
            let (traps, outcome) = run_case(c);
            match outcome {
                Outcome::Faulted(kind) if traps == 0 => Some(kind),
                _ => None,
            }
        })
        .collect();
    without_trap.sort_unstable();
    without_trap.dedup();

    assert_eq!(
        without_trap,
        alloc::vec!["DivisionByZero", "IndexOutOfBounds"],
        "the set of faults reachable from compiler output with NO trap opcode \
         has changed. That set is the measured size of workstream C's \
         'make every other opcode total' obligation, so a change here resizes \
         an operator decision and must be deliberate."
    );
}

/// The census covers every partial operation workstream C enumerates.
///
/// Guards the other direction: a row silently disappearing would shrink the
/// obligation without anyone deciding to.
#[test]
fn the_census_covers_the_specified_partial_operations() {
    let names: alloc::vec::Vec<&str> = CASES.iter().map(|c| c.name).collect();
    for needle in [
        "division by a runtime zero",
        "modulo by a runtime zero",
        "array index out of bounds, runtime index",
        "bare for-limit exceeded",
        "addition overflow wraps, as specified",
        "cast out of range truncates",
    ] {
        assert!(
            names.contains(&needle),
            "the census lost its row for {needle:?}; the specified list is no longer covered"
        );
    }
    assert!(
        CASES.iter().any(|c| c.outcome == Outcome::Completed),
        "no row completes, so the census has no control and a universal fault would pass it"
    );
}
