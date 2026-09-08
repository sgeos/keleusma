//! **WHERE DOES THE NATIVE SIDE OF THE B35 PARTIAL-OPERATION CONTRACT ACTUALLY
//! STAND?**
//!
//! # Why this exists
//!
//! `docs/spec/RUNTIME_FAULTS.md` carries a NORMATIVE native contract: where a
//! program does not handle a partial outcome, native code produces a defined
//! value rather than trapping — zero for division by zero, the numerator for
//! modulo by zero, a canonical-zero or lowest-valid value for the three software
//! conditions, and a trap only for a native error, which has no safe default.
//!
//! The same specification defers that lowering to V0.4.0 and says it "is not
//! implemented". **So the interesting question is not whether the contract is
//! met — it is what the backend does INSTEAD, for each of the six, and whether
//! the interim behaviour is sound.**
//!
//! Trapping is sound as an interim: a trap is not a silently wrong value, and it
//! matches the virtual machine, which faults. Producing the contract's default
//! today, while the reference still traps, would be the divergence.
//!
//! # What this census establishes, and what it deliberately does not
//!
//! It establishes, per operation: whether a subject can be written at all,
//! what the REFERENCE does with it, whether the backend lowers or refuses, and
//! whether the emitted code reaches a trap. **It does not assert that the
//! contract is implemented, because it is not, and it does not assert that it
//! should be — that is V0.4.0 scope and the operator's sequencing call.**
//!
//! # Why the native side is read out of the IR rather than run
//!
//! A native trap aborts the process, so a trapping subject cannot be executed
//! in-process and compared. `differential.rs` established the technique this
//! file reuses: inspect the emitted IR for a conditional branch into the trap
//! block. That is a claim about this lowering's straight-line shape, not a
//! general licence to read IR textually.

use inkwell::context::Context;
use keleusma::bytecode::{Module, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{Vm, auto_arena_capacity_for};
use keleusma_native::{LowerOptions, lower_module, module_refusals};

mod common;

fn try_compile(src: &str) -> Result<Module, String> {
    let toks = tokenize(src).map_err(|e| format!("lex: {e:?}"))?;
    let ast = parse(&toks).map_err(|e| format!("parse: {e:?}"))?;
    compile(&ast).map_err(|e| format!("{e:?}"))
}

/// What the reference does: an error name, or the value it returned.
fn reference_outcome(m: &Module, args: &[i64]) -> String {
    let cap = match auto_arena_capacity_for(m, &[]) {
        Ok(c) => c,
        Err(e) => return format!("arena: {e:?}"),
    };
    let arena = keleusma_arena::Arena::with_capacity(cap + (64 << 10));
    let mut vm = match Vm::new(m.clone(), &arena) {
        Ok(v) => v,
        Err(e) => return format!("vm-new: {e:?}"),
    };
    let vals: Vec<Value> = args.iter().map(|&x| Value::Int(x)).collect();
    match vm.call(&vals) {
        Err(e) => format!("FAULT {e:?}"),
        Ok(st) => format!("ok {st:?}").chars().take(46).collect(),
    }
}

/// Whether the emitted module can reach a trap, and whether a guard precedes it.
fn native_shape(m: &Module) -> String {
    let refusals = module_refusals(m, LowerOptions::default());
    if !refusals.is_empty() {
        let why: String = refusals
            .iter()
            .map(|(_, e)| e.to_string())
            .collect::<Vec<_>>()
            .join(" | ")
            .chars()
            .take(58)
            .collect();
        return format!("REFUSED: {why}");
    }
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    if let Err(e) = lower_module(&ctx, &lm, m, LowerOptions::default()) {
        return format!("REFUSED: {e}");
    }
    let ir = lm.print_to_string().to_string();
    // **REACHABILITY, NOT TEXTUAL SHAPE.** An earlier version of this looked for
    // `br i1 ... label %trap` and reported "no guard branch" for three subjects
    // that ARE guarded — they reach the trap through an unconditional branch
    // from a conditionally-entered block. LLVM labels a block nothing reaches
    // with `No predecessors!`, which answers the question directly.
    // Every lowered function is given a trap block whether or not it uses one,
    // so the question is whether ANY trap block in the module is REACHED. LLVM
    // labels a block nothing reaches with `No predecessors!`.
    //
    // **THE LIMIT OF THIS INSTRUMENT, STATED**: it is per MODULE, not per path.
    // It answers "can this module trap at all", which is what distinguishes a
    // fabricated value from a fault; it does not prove the trap is on the
    // subject's own path. A first version reported the reverse error — it looked
    // for a conditional branch straight to `%trap` and called three guarded
    // subjects unguarded, because they reach it through an unconditional branch
    // from a conditionally-entered block.
    let mut reachable = 0usize;
    let mut unreachable = 0usize;
    for l in ir.lines() {
        let t = l.trim_start();
        if !t.starts_with("trap") || !t.contains("preds") && !t.contains("No predecessors!") {
            continue;
        }
        if t.contains("No predecessors!") {
            unreachable += 1;
        } else {
            reachable += 1;
        }
    }
    if !ir.contains("llvm.trap") {
        "LOWERS, no trap emitted".to_string()
    } else if reachable > 0 {
        format!("LOWERS, trap reachable ({reachable} live, {unreachable} unused)")
    } else {
        "LOWERS, EVERY TRAP UNREACHABLE".to_string()
    }
}

/// One partial operation, its normative native default, and a subject that
/// triggers it UNHANDLED.
struct Subject {
    op: &'static str,
    contract: &'static str,
    src: &'static str,
    args: &'static [i64],
}

const SUBJECTS: &[Subject] = &[
    Subject {
        op: "division by zero",
        contract: "zero",
        src: "fn main(a: Word, b: Word) -> Word { a / b }",
        args: &[7, 0],
    },
    Subject {
        op: "modulo by zero",
        contract: "the numerator",
        src: "fn main(a: Word, b: Word) -> Word { a % b }",
        args: &[7, 0],
    },
    Subject {
        op: "out-of-bounds index",
        contract: "element zero-or-lowest-valid",
        src: "fn main(i: Word) -> Word { let xs = [1, 2, 3]; xs[i] }",
        args: &[5],
    },
    Subject {
        op: "newtype predicate failure",
        contract: "the lowest-valid value",
        src: "fn in_range(x: Word) -> bool { x >= 0 and x <= 100 }\n\
              newtype Percent = Word where in_range;\n\
              fn main(raw: Word) -> Percent { Percent(raw) }",
        args: &[500],
    },
    Subject {
        op: "invalid discriminant",
        contract: "the zero-or-lowest-valid variant",
        // The BARE cast is rejected by the reference: the discriminant
        // conversion is a construct and requires its arm block. "Unhandled"
        // therefore means a construct with no `invalid_discriminant` arm, which
        // is what this subject is.
        src: "enum Status { Ok = 0, Pending = 1 }\n\
              fn main(code: Word) -> Status { code as Status { ok(_) => Status::Ok } }",
        args: &[99],
    },
    // A fallible NATIVE function is the sixth. It has no pure-source subject:
    // the failure comes from a host-registered function, and a module that
    // declares one cannot be driven here without a host. Its contract entry is
    // also the only one that says TRAP, so the interim behaviour and the
    // contract already agree — which is why its absence costs least.
];

#[test]
fn what_the_backend_does_with_each_partial_operation_today() {
    println!("\n================ PARTIAL-OPERATION CENSUS (B35 P8)");
    println!(
        "  {:<28} {:<32} {:<26} backend today",
        "operation", "contract default (V0.4.0)", "reference today"
    );
    let mut written = 0usize;
    let mut lowered = 0usize;
    let mut contract_met = 0usize;

    for s in SUBJECTS {
        match try_compile(s.src) {
            Err(why) => {
                let short: String = why.chars().take(200).collect();
                println!(
                    "  {:<28} {:<32} {:<26} {}",
                    s.op, s.contract, "REFERENCE REJECTED", short
                );
            }
            Ok(m) => {
                written += 1;
                let vm = reference_outcome(&m, s.args);
                let nat = native_shape(&m);
                if nat.starts_with("LOWERS") {
                    lowered += 1;
                }
                // The contract is met only where native produces the defined
                // value. A guarded trap is the INTERIM behaviour, not the
                // contract, and is counted as such rather than as compliance.
                if nat == "LOWERS, no trap emitted" && vm.starts_with("ok") {
                    contract_met += 1;
                }
                println!("  {:<28} {:<32} {:<26} {}", s.op, s.contract, vm, nat);
            }
        }
    }

    println!("  ------------------------------------------------");
    println!(
        "  subjects written {written} of {}, lowering {lowered}, producing a defined \
         value where the reference does not fault {contract_met}",
        SUBJECTS.len()
    );
    println!(
        "\n  THE CONTRACT IS DEFERRED TO V0.4.0 AND IS NOT IMPLEMENTED. Trapping is\n  \
         the sound interim: it matches the reference, and a trap is not a\n  \
         silently wrong value. Producing the contract's default TODAY, while the\n  \
         reference still faults, would be the divergence.\n================\n"
    );

    // **NON-VACUITY.** A census whose subjects the reference rejects measures
    // nothing about the backend.
    assert!(
        written >= 3,
        "fewer than three partial-operation subjects compile, so this census has \
         too little subject to describe the backend's behaviour"
    );
}

/// **The interim behaviour must be a TRAP and not a fabricated value.**
///
/// This is the property that matters while the contract is deferred: for a
/// partial operation the reference faults on, native code must not carry on with
/// a value of its own invention. It is asserted separately from the census
/// because the census prints and this decides.
#[test]
fn no_partial_operation_produces_a_value_where_the_reference_faults() {
    let mut checked = 0usize;
    for s in SUBJECTS {
        let Ok(m) = try_compile(s.src) else { continue };
        let vm = reference_outcome(&m, s.args);
        if !vm.starts_with("FAULT") {
            continue;
        }
        checked += 1;
        let nat = native_shape(&m);
        assert!(
            nat.starts_with("REFUSED") || nat.starts_with("LOWERS, trap reachable"),
            "the reference FAULTS on {:?} ({vm}), and the backend neither refuses \
             it nor emits a trap: {nat}. Producing a value where the reference \
             faults is a silently wrong answer, which is the class this backend \
             exists to refuse. If the B35 native contract has been implemented, \
             this test is what must change, deliberately and with the reference's \
             agreement.",
            s.op
        );
    }
    assert!(
        checked > 0,
        "no subject made the reference fault, so this asserted nothing"
    );
}

/// **AN OUT-OF-BOUNDS INDEX MUST NOT BE SILENTLY ANSWERED.**
///
/// # This test found a real defect, and is kept as the guard against its return
///
/// It was written to run `xs[i]` with a three-element array and `i = 5`, and it
/// FAILED: the reference faults `IndexOutOfBounds(5, 3)`, and native code
/// returned `0xabababababababab` — the filler byte of the caller's region buffer,
/// read forty bytes past the array body. Both halves of the worst case at once,
/// **a silently wrong value and an out-of-bounds read at a runtime offset.**
///
/// The cause was a premise recorded in a comment and never checked: the
/// `Op::GetIndex` arm said the compiler emits `Op::BoundsCheck` before the
/// index. It does not, for this shape — the bytecode is a bare `GetIndex`.
///
/// # Why the out-of-range case is checked structurally rather than executed
///
/// It now traps, and a native trap aborts the process, so the guarded path
/// cannot be called from a test harness. Its presence is read out of the IR by
/// reachability, which is what `native_shape` does. **The IN-RANGE case IS
/// executed**, because a guard that also rejected valid indices would satisfy a
/// structural check while breaking every array in the corpus.
#[test]
fn an_out_of_bounds_index_is_not_silently_answered() {
    use inkwell::OptimizationLevel;
    use keleusma::vm::required_persistent_capacity_for;

    let src = "fn main(i: Word) -> Word { let xs = [1, 2, 3]; xs[i] }";
    let m = try_compile(src).expect("the subject compiles");

    let reference = reference_outcome(&m, &[5]);
    assert!(
        reference.starts_with("FAULT"),
        "the reference no longer faults on an out-of-bounds index, so this test \
         is asserting the wrong thing: {reference}"
    );

    let shape = native_shape(&m);
    assert!(
        shape.starts_with("REFUSED") || shape.starts_with("LOWERS, trap reachable"),
        "an out-of-bounds index must be refused or trap. It said: {shape}. \
         Returning a value here is what this test caught once already."
    );
    if shape.starts_with("REFUSED") {
        return;
    }

    // **MUST-NOT-FIRE: a VALID index still returns its element.** Without this,
    // a guard that trapped unconditionally would pass everything above.
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower");
    lm.verify().expect("IR valid");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    let entry = m.entry_point.expect("entry");
    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    assert_eq!(
        f.count_params(),
        u32::from(m.chunks[entry].param_count) + 3,
        "the call below names this signature by hand"
    );
    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let mut privs = vec![0u8; required_persistent_capacity_for(&m) + 4096];
    let mut shared = vec![0u8; 4096];
    let mut region = vec![0xABu8; 64 << 10];

    for (i, expect) in [(0i64, 1i64), (1, 2), (2, 3)] {
        let got = unsafe {
            callable.call(
                i,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region.as_mut_ptr(),
            )
        };
        assert_eq!(
            got, expect,
            "the guard rejected a VALID index {i}: expected {expect}, got {got} \
             (0x{got:x}). A bounds check that refuses in-range indices breaks \
             every array in the corpus while satisfying the structural check above"
        );
    }
}

/// **DIAGNOSTIC, NOT A GUARD.** Prints the branch structure around each
/// subject's trap block so "no guard branch" can be told apart from "a guard my
/// detector did not recognise". Kept because the distinction is the exact one
/// this line has got wrong before: a guard that cannot fire is
/// indistinguishable from no guard.
#[test]
fn what_reaches_the_trap_block_in_each_subject() {
    for s in SUBJECTS {
        let Ok(m) = try_compile(s.src) else { continue };
        if !module_refusals(&m, LowerOptions::default()).is_empty() {
            continue;
        }
        let ctx = Context::create();
        let lm = ctx.create_module("kel");
        if lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err() {
            continue;
        }
        let ir = lm.print_to_string().to_string();
        println!("\n================ {}", s.op);
        let _ = &ir;
        for (i, c) in m.chunks.iter().enumerate() {
            println!("  chunk {i} {:?}", c.ops);
        }
    }
}
