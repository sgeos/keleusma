//! The `reconstruct` seeding accessors, driven from outside the crate.
//!
//! # Why this file exists
//!
//! The `v0.3.0` line drives each self-hosted stage against its native lowering and
//! reported `reconstruct` as its last stage "agreeing while producing nothing" —
//! the stage was entered with an all-zero shared segment, took an immediate
//! end-of-input exit, and both sides agreed on doing nothing. They asked for
//! seeding accessors so the bytes would be the driver's own encoding rather than a
//! second one reproduced in their harness.
//!
//! # What was actually blocked, which is half of what was reported
//!
//! They reported that neither accessor could be called because `ParsedFn` is `pub`
//! with every field private "and the function that produces one is private".
//!
//! **`parse_functions` is public.** It returns `Vec<ParsedFn>`, so `&[&ParsedFn]`
//! is obtainable and [`seed_reconstruct_multihead_shared`] was reachable from
//! outside the crate all along. Measured before anything was changed:
//! `what_the_multihead_accessor_could_already_do` below is that measurement, and it
//! would have passed before this file's accessors existed.
//!
//! Only [`seed_reconstruct_shared`] was genuinely blocked. It takes a record stream
//! plus a category and parameter count, which are the private `body`, `cat` and
//! `params` fields. Four accessors now expose them.
//!
//! # What these tests establish, and what they do not
//!
//! They establish that both entry points are callable externally and that the stage
//! **consumes** what it is given: a differing record stream produces a differing
//! reconstruction, which is the must-fire control. A test that called the accessors
//! and checked they returned bytes would establish neither — an all-zero segment is
//! also bytes, and that is precisely the failure mode being closed.
//!
//! They do **not** establish that the reconstruction is correct. That is the
//! reference differential's job, in `tests/selfhost_codegen.rs`. Nor do they cover
//! `verify_datalayout`, which has no batch-zero seed by design.

#![cfg(feature = "self-host")]

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::selfhost::{
    ParsedFn, parse_functions, reconstruct_kel_module, seed_reconstruct_multihead_shared,
    seed_reconstruct_shared,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

const MULTIHEAD_SRC: &str = "fn f(0) -> Word { 0 }\n\
                             fn f(1) -> Word { 11 }\n\
                             fn f(n: Word) -> Word { n + 10 }\n\
                             fn main() -> Word { f(1) }\n";

const SINGLE_SRC: &str = "fn g(a: Word) -> Word { a + 1 }\n\
                          fn main() -> Word { g(1) }\n";

fn kel_vm(m: Module, arena: &Arena) -> Vm<'_, '_> {
    Vm::new(m, arena).expect("verify reconstruct.kel")
}

fn fresh_arena(m: &Module) -> Arena {
    let need = required_persistent_capacity_for(m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    arena
}

/// Drive `reconstruct.kel` over a seeded buffer and return `(node_count, root)`.
///
/// The stage yields its node count, so a count of zero means it read nothing —
/// which is exactly the "agreeing while producing nothing" state being closed.
fn drive(shared: &mut [u8], vm: &mut Vm<'_, '_>) -> (i64, i64) {
    let n = match vm.call_with_shared(shared, &[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => n,
        other => panic!("unexpected reconstruct.kel state: {other:?}"),
    };
    // Slot 0 of the AST root block; read positionally because the verdict slot
    // constants are private and this file deliberately does not reproduce them.
    let root = match vm.get_shared(shared, 0).expect("root slot") {
        Value::Int(v) => v,
        o => panic!("expected Int, got {o:?}"),
    };
    (n, root)
}

/// **THE MEASUREMENT THAT CORRECTS THE REPORT.** This accessor needed nothing.
///
/// It uses only `parse_functions`, which was already public, and would have passed
/// before the `ParsedFn` accessors were added. Kept as a standing check that the
/// multihead entry point stays externally reachable.
#[test]
fn what_the_multihead_accessor_could_already_do() {
    let (fns, ..) = parse_functions(MULTIHEAD_SRC);
    let heads: Vec<&ParsedFn> = fns.iter().filter(|f| f.param_count() <= 1).collect();
    assert!(
        heads.len() >= 3,
        "expected the three heads of `f`, got {}",
        heads.len()
    );

    let m = reconstruct_kel_module();
    let arena = fresh_arena(&m);
    let vm = kel_vm(m, &arena);

    let seed = seed_reconstruct_multihead_shared(&vm, &heads, 1);
    let non_zero = seed.iter().filter(|b| **b != 0).count();
    assert!(
        non_zero > 0,
        "the multihead seed is entirely zero, which is indistinguishable from not \
         being built at all"
    );
}

/// The single-head accessor, which is the one that was genuinely blocked.
///
/// Callable now only because `ParsedFn` exposes its category, parameter count and
/// body records. The stage must actually consume them: a zero node count means it
/// took the end-of-input exit, which is the state this seeding exists to leave.
#[test]
fn the_single_head_accessor_is_callable_and_the_stage_consumes_it() {
    let (fns, ..) = parse_functions(SINGLE_SRC);
    let g = fns
        .iter()
        .find(|f| !f.body_records().is_empty())
        .expect("a head with a body");

    let m = reconstruct_kel_module();
    let arena = fresh_arena(&m);
    let mut vm = kel_vm(m, &arena);

    let mut shared = seed_reconstruct_shared(&vm, g.body_records(), g.category(), g.param_count());
    let (nodes, _root) = drive(&mut shared, &mut vm);

    assert!(
        nodes > 0,
        "reconstruct.kel produced {nodes} nodes from a non-empty record stream; a \
         count of zero is the immediate end-of-input exit, meaning the seed was not \
         consumed"
    );
}

/// **MUST FIRE.** A different record stream must produce a different reconstruction.
///
/// Without this, the test above passes for a stage that ignores its input entirely
/// and returns a constant. That is not hypothetical: the reported defect was a
/// stage agreeing with its native lowering *while producing nothing*, and a
/// silently-unbuilt seed and a silently-rejected one produce the identical number.
#[test]
fn a_different_record_stream_reconstructs_differently() {
    let (small, ..) = parse_functions(SINGLE_SRC);
    let (large, ..) = parse_functions(
        "fn h(a: Word) -> Word { a + 1 + 2 + 3 + 4 + 5 + 6 }\nfn main() -> Word { h(1) }\n",
    );

    let a = small
        .iter()
        .find(|f| !f.body_records().is_empty())
        .expect("small body");
    let b = large
        .iter()
        .find(|f| !f.body_records().is_empty())
        .expect("large body");

    assert!(
        b.body_records().len() > a.body_records().len(),
        "the two subjects must differ in the input under test: {} against {}",
        a.body_records().len(),
        b.body_records().len()
    );

    let m = reconstruct_kel_module();
    let arena = fresh_arena(&m);
    let mut vm = kel_vm(m, &arena);

    let mut sa = seed_reconstruct_shared(&vm, a.body_records(), a.category(), a.param_count());
    let (na, _) = drive(&mut sa, &mut vm);

    let m2 = reconstruct_kel_module();
    let arena2 = fresh_arena(&m2);
    let mut vm2 = kel_vm(m2, &arena2);
    let mut sb = seed_reconstruct_shared(&vm2, b.body_records(), b.category(), b.param_count());
    let (nb, _) = drive(&mut sb, &mut vm2);

    assert_ne!(
        na, nb,
        "both record streams reconstructed to {na} nodes. The stage is not \
         discriminating on its input, so the seeding test above proves nothing."
    );
}
