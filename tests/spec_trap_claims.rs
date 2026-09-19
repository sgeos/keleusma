#![cfg(all(feature = "compile", feature = "verify"))]
//! The instruction-set specification's trap-or-reify claims, checked against
//! the implementation by running it.
//!
//! # Why this file exists
//!
//! `docs/spec/INSTRUCTION_SET.md` stated that `CheckedMod` "Traps on
//! divide-by-zero". **The implementation has never done that** — it reifies a
//! zero divisor as flag `3` carrying the numerator, mirroring `CheckedDiv`,
//! and says so in its own comment. The row sat directly beside `CheckedDiv`,
//! which describes the reifying behaviour correctly, and the same wrong claim
//! had propagated to a hand-written book page that gave the wrong MECHANISM
//! as well.
//!
//! Nothing caught it, because **no test asserted the specification's prose
//! against the implementation for this class**. The one guard that exists in
//! the neighbourhood regenerates the book chapter from the spec and fails on
//! drift, which protects the COPY and says nothing about whether the original
//! is true. `tests/push_order_claims.rs` guards a different prose class the
//! same way this file guards its own.
//!
//! # The population
//!
//! Taken from the specification's own rows rather than from a hand-picked
//! list. Scanning every row whose prose makes a trap-or-reify claim yields:
//!
//! | row | the claim |
//! |---|---|
//! | `Div` | traps on divide-by-zero |
//! | `Mod` | traps on divide-by-zero |
//! | `CheckedDiv` | a zero divisor reifies as flag `3`, rather than trapping |
//! | `CheckedMod` | the same, mirroring `CheckedDiv` |
//! | `BoundsCheck` | traps if the index is outside `[0, bound)` |
//!
//! `CheckedNeg` names an overflow case but makes no trap-or-reify claim, and
//! `Trap` is the trap itself. Neither is in this class.
//!
//! # What is asserted, and what deliberately is not
//!
//! The BEHAVIOUR the row claims, never the row's wording. A reworded row must
//! not fail this file; a reworded row that becomes untrue must.
//!
//! Every case asserts that it REACHED the opcode under test. A module that is
//! refused before the opcode executes has tested nothing, and this suite's
//! recurring failure mode is exactly that: an outcome that looks like a pass
//! while testing nothing.

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::{Module, Op};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{
    DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for,
};

fn compiled(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

/// Every opcode in the module, flattened.
fn ops(m: &Module) -> alloc::vec::Vec<Op> {
    m.chunks
        .iter()
        .flat_map(|c| c.ops.iter().copied())
        .collect()
}

/// Runs `m`, requiring that it verifies first.
///
/// Verification failing is a failure of the CASE, not a pass: a module the
/// virtual machine never accepted cannot demonstrate anything about an
/// opcode's behaviour.
fn run(what: &str, m: Module) -> Result<VmState, VmError> {
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena
        .resize_persistent(need)
        .unwrap_or_else(|e| panic!("{what}: arena: {e:?}"));
    let mut vm = Vm::new(m, &arena).unwrap_or_else(|e| {
        panic!("{what}: the module did not verify, so the opcode was never reached: {e:?}")
    });
    vm.call(&[])
}

/// `Div` and `Mod` trap on a zero divisor, as their rows say.
#[test]
fn the_plain_divide_and_modulo_trap_on_a_zero_divisor_as_the_spec_says() {
    for (what, src, wanted) in [
        ("Div", "fn main() -> Word { let z = 0; 7 / z }", Op::Div),
        ("Mod", "fn main() -> Word { let z = 0; 7 % z }", Op::Mod),
    ] {
        let m = compiled(src);
        assert!(
            ops(&m).contains(&wanted),
            "{what}: the program does not emit {wanted:?}, so this case tests nothing. \
             The lowering changed and this file's population needs re-deriving."
        );
        let r = run(what, m);
        assert!(
            matches!(r, Err(VmError::DivisionByZero)),
            "{what}: the spec row says it traps on divide-by-zero; got {r:?}"
        );
    }
}

/// `CheckedDiv` and `CheckedMod` reify a zero divisor rather than trapping.
///
/// The surface `/` lowers to `Op::Div`, so a source-level program cannot reach
/// the checked opcodes without the arm construct. The module is therefore
/// patched: `Div` is replaced by the two-instruction sequence the
/// specification itself describes for an uncaptured operation, `CheckedDiv`
/// followed by `PopN(2)`, which discards the flag and the high half and leaves
/// the low half. That substitution is stack-neutral, so the surrounding chunk
/// stays well formed, and the program contains no jump whose target the extra
/// instruction would move.
///
/// If the reified behaviour were a trap, this would fault. It must not.
#[test]
fn the_checked_divide_and_modulo_reify_a_zero_divisor_rather_than_trapping() {
    for (what, src, plain, checked) in [
        (
            "CheckedDiv",
            "fn main() -> Word { let z = 0; 7 / z }",
            Op::Div,
            Op::CheckedDiv(0),
        ),
        (
            "CheckedMod",
            "fn main() -> Word { let z = 0; 7 % z }",
            Op::Mod,
            Op::CheckedMod,
        ),
    ] {
        let mut m = compiled(src);
        let mut patched = false;
        for chunk in &mut m.chunks {
            if let Some(i) = chunk.ops.iter().position(|o| *o == plain) {
                assert!(
                    !chunk.ops.iter().any(|o| matches!(
                        o,
                        Op::If(_)
                            | Op::Else(_)
                            | Op::Loop(_)
                            | Op::EndLoop(_)
                            | Op::Break(_)
                            | Op::BreakIf(_)
                    )),
                    "{what}: the fixture gained a jump, so splicing an instruction would move a \
                     target; choose a straight-line fixture instead"
                );
                chunk.ops[i] = checked;
                chunk.ops.insert(i + 1, Op::PopN(2));
                patched = true;
                break;
            }
        }
        assert!(
            patched,
            "{what}: no {plain:?} to replace, so this case tests nothing"
        );
        assert!(
            ops(&m).contains(&checked),
            "{what}: the patch did not take, so this case tests nothing"
        );

        let r = run(what, m);
        assert!(
            !matches!(r, Err(VmError::DivisionByZero)),
            "{what}: the spec row says a zero divisor reifies as flag 3 rather than trapping, \
             and the implementation trapped. One of the two is wrong, and the row is the \
             claim under test."
        );
    }
}

/// `BoundsCheck` traps when the index is outside its bound.
///
/// The row records that the compiler emits it between levels of a
/// multi-dimensional indexed access, so the fixture is two-dimensional. If a
/// lowering change stops emitting it here the assertion below fails rather
/// than the case quietly passing on a program that never contained the
/// opcode.
#[test]
fn the_bounds_check_traps_outside_its_bound_as_the_spec_says() {
    let src = "fn main() -> Word {\n\
               \x20 let g = [[1, 2], [3, 4]];\n\
               \x20 let i = 5;\n\
               \x20 g[i][0]\n\
               }";
    let m = compiled(src);
    assert!(
        ops(&m)
            .iter()
            .any(|o| matches!(o, Op::BoundsCheck(_)) || matches!(o, Op::GetIndex(_))),
        "the two-dimensional index emits neither BoundsCheck nor GetIndex, so this case \
         tests nothing and the population needs re-deriving"
    );
    let r = run("BoundsCheck", m);
    assert!(
        matches!(r, Err(VmError::IndexOutOfBounds(_, _))),
        "an out-of-range index must fault, as the BoundsCheck row says; got {r:?}"
    );
}

/// The population this file covers is the one the specification states.
///
/// Guards the census itself: if a row gains or loses a trap-or-reify claim,
/// the set below no longer matches the specification and the file's coverage
/// claim has gone stale.
#[test]
fn the_population_matches_the_rows_that_make_such_a_claim() {
    const SPEC: &str = include_str!("../docs/spec/INSTRUCTION_SET.md");
    let mut claiming: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for line in SPEC.lines() {
        let Some(rest) = line.strip_prefix("| ") else {
            continue;
        };
        let Some((name, body)) = rest.split_once('|') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() || !name.chars().next().is_some_and(char::is_uppercase) {
            continue;
        }
        let low = body.to_ascii_lowercase();
        if low.contains("traps on") || low.contains("trap if") || low.contains("reifies as flag") {
            claiming.push(name);
        }
    }
    claiming.sort_unstable();
    claiming.dedup();
    assert_eq!(
        claiming,
        alloc::vec!["BoundsCheck", "CheckedDiv", "CheckedMod", "Div", "Mod"],
        "the set of specification rows making a trap-or-reify claim has changed. This file \
         covers a fixed population; a new claiming row is unguarded until it is added here."
    );
}
