//! **GENERATED FLOAT STREAMS, COMPARED AGAINST THE REFERENCE SEQUENCE BY SEQUENCE.**
//!
//! # Why this exists
//!
//! `Op::Yield` became float-aware for a general stream on 2026-09-18. That arm is
//! **the newest code in the backend**, and it shipped resting on **four
//! hand-written subjects — all chosen by the person who had just written the
//! arm.** `generated_floats.rs` covers the straight-line float surface; this
//! covers the one where a value crosses a suspension.
//!
//! The stream path is where it matters most. Everything beneath a yielded value
//! is spilled to an ephemeral slice as `(Width, OperandKind)` pairs and restored
//! at the resume point. **That width-and-kind interaction across a suspension is
//! exactly where this line found six width arm-groups silently refusing, and
//! where a `lower_module` panic sat unnoticed until 2026-09-17.**
//!
//! # ⚠ A STREAM FEEDS ITS OWN OUTPUT BACK, AND THAT BREAKS THE SIBLING'S BOUND
//!
//! `generated_floats.rs` keeps every value four-byte exact with a single
//! argument: an expression tree of depth *d* over leaves bounded by *M* cannot
//! exceed `M^(2^d)`. **That argument does not survive feedback.**
//!
//! If a generated body computed the next yield from the reply under a
//! multiplication, the bound at tick *n* would be the bound at tick *n-1* raised
//! to the same power — **doubly exponential in the tick count**, leaving the
//! `2^24` exact-integer range of an `f32` almost immediately. Copying the
//! sibling's reasoning here is the obvious move and it is unsound.
//!
//! # The bound that does hold, and why it is stable rather than growing
//!
//! Two facts close it:
//!
//! 1. **The reply enters only ADDITIVELY.** The generator emits `r + E` or
//!    `r - E` and never `r * E`, so a reply is never a multiplicand.
//! 2. **`t` is only ever the initial argument or a reply.** On `Op::Reset` the
//!    runtime writes the resume value into slot 0, so the loop's next iteration
//!    sees a reply as `t`. Both are bounded by [`REPLY_MAX`].
//!
//! So the inner expression `E`, over `t` and literals with depth [`EXPR_DEPTH`],
//! is bounded by `max(REPLY_MAX, LITERAL_MAX) ^ (2 ^ EXPR_DEPTH)`, and a yielded
//! value by `REPLY_MAX +` that. **The bound does not grow with the tick count**,
//! which is the property feedback threatened. `the_feedback_bound_is_arithmetic`
//! asserts it, and every value is checked on both sides at every tick anyway.
//!
//! # What this excludes, stated where a reader meets it
//!
//! No division or modulo — they leave the integers and the exactness invariant
//! with them. No composites beneath a yield, which the emitter refuses
//! deliberately. Two yields per body, matching the shape the hand-written
//! drivers were built for.
//!
//! # What a green run may claim
//!
//! No divergence over the generated streams, at this commit, within these
//! bounds. **Not** that the float yield arm is correct.

mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Value;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use std::collections::BTreeSet;

/// The float width this backend lowers to.
#[cfg(feature = "narrow-float-32")]
type Flt = f32;
#[cfg(not(feature = "narrow-float-32"))]
type Flt = f64;

/// Widen this configuration's float to `f64`.
///
/// **Split by configuration rather than written once**, because `f64::from` is
/// required at four bytes and is a `useless_conversion` lint error at eight.
#[cfg(feature = "narrow-float-32")]
fn wide(x: Flt) -> f64 {
    f64::from(x)
}
#[cfg(not(feature = "narrow-float-32"))]
fn wide(x: Flt) -> f64 {
    x
}

/// The largest integer a four-byte float represents exactly.
const F32_EXACT_INT_LIMIT: f64 = 16_777_216.0;
/// Replies, and therefore `t` after a reset, are integers in `1..=REPLY_MAX`.
const REPLY_MAX: u64 = 5;
/// Literal leaves are integers in `0..=LITERAL_MAX`.
const LITERAL_MAX: u64 = 3;
/// Depth of the inner expression over `t` and literals.
const EXPR_DEPTH: u32 = 2;

/// The largest magnitude any generated stream can yield. See the header.
fn worst_case_yield() -> f64 {
    let leaf = REPLY_MAX.max(LITERAL_MAX) as f64;
    REPLY_MAX as f64 + leaf.powi(1 << EXPR_DEPTH)
}

/// Fail loudly on a value the differential must not be comparing.
fn assert_exact(what: &str, src: &str, v: f64) {
    assert!(
        f64::from(v as f32) == v,
        "{what} {v} for `{src}` is not exact in four bytes. The reference runs at \
         f64 in both configurations and this backend lowers at the configured \
         width, so comparing this value would report rounding as divergence."
    );
    assert!(
        v.abs() <= F32_EXACT_INT_LIMIT,
        "{what} {v} for `{src}` exceeds the four-byte exact-integer limit. The \
         feedback bound this file rests on has been broken."
    );
}

/// Drive the reference, passing and receiving `Value::Float`.
fn vm_sequence(src: &str, first: Flt, replies: &[Flt]) -> Vec<f64> {
    let m = common::build(src);
    let need = required_persistent_capacity_for(&m);
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena") + need + (64 << 10);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent");
    let mut vm = Vm::new(m, &arena).expect("vm");
    let mut shared: Vec<u8> = Vec::new();

    let mut out = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Float(wide(first))])
        .expect("vm run");
    while out.len() < replies.len() {
        match st {
            VmState::Yielded(Value::Float(v)) => {
                out.push(v);
                let r = wide(replies[out.len() - 1]);
                st = vm
                    .resume_with_shared(&mut shared, Value::Float(r))
                    .expect("resume");
            }
            VmState::Reset => {
                let r = wide(replies[out.len().saturating_sub(1)]);
                st = vm
                    .resume_with_shared(&mut shared, Value::Float(r))
                    .expect("resume after reset");
            }
            other => panic!("a float stream produced {other:?}"),
        }
    }
    out
}

/// Drive the lowered module through a signature named for THIS configuration.
fn native_sequence(src: &str, first: Flt, replies: &[Flt]) -> Vec<f64> {
    let m = common::build(src);
    let entry = m.entry_point.expect("entry point");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    keleusma_native::lower_module(&ctx, &lm, &m, keleusma_native::LowerOptions::default())
        .expect("lower module");
    lm.verify().expect("LLVM module verification");
    common::maybe_optimize(&lm);
    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");

    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry function");
    // **ASSERTED BEFORE THE CALL.** A wrong signature here is undefined behaviour
    // that surfaces as a SIGBUS inside JIT code with no usable stack, which this
    // package has already paid for once.
    assert_eq!(
        f.count_params(),
        u32::from(m.chunks[entry].param_count) + 3,
        "a resumable stream must carry the three trailing pointers; the call below \
         names that signature by hand and cannot detect a change to it"
    );
    assert!(
        f.get_nth_param(0).expect("a parameter").is_float_value(),
        "parameter 0 is not a float value, so the hand-named floating-point \
         signature below would be wrong and calling through it is undefined \
         behaviour"
    );

    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(Flt, *mut u8, *mut u8, *mut u8) -> Flt>(&sym)
    }
    .expect("entry symbol");

    let persistent = required_persistent_capacity_for(&m)
        + keleusma_native::region::persistent_supplement_bytes(&m) as usize;
    let mut privs = vec![0u8; persistent + 64];
    common::install_private_init_bytes(&m, &mut privs);
    let mut shared = vec![0u8; keleusma::vm::shared_data_bytes_for(&m).max(8)];
    let region_bytes = keleusma_native::region::host_arena_supplement_bytes(&m) as usize + 4096;
    let mut region = vec![0u8; region_bytes];

    let mut out = Vec::new();
    let mut input = first;
    for &r in replies {
        out.push(wide(unsafe {
            callable.call(
                input,
                shared.as_mut_ptr(),
                privs.as_mut_ptr(),
                region.as_mut_ptr(),
            )
        }));
        input = r;
    }
    out
}

/// Xorshift64*, so a failure is reproducible rather than an anecdote.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Operators for the inner expression. **No `/` or `%`**: they leave the
/// integers, and the exactness invariant with them.
const INNER_OPS: &[&str] = &["+", "-", "*"];
/// The reply joins only additively. **Never `*`** — see the header.
const REPLY_OPS: &[&str] = &["+", "-"];

fn leaf(rng: &mut Rng) -> String {
    match rng.below(3) {
        0 => "t".into(),
        _ => format!("{}.0", rng.below(LITERAL_MAX + 1)),
    }
}

/// An expression over `t` and literals only. **The reply never appears here.**
fn inner(rng: &mut Rng, depth: u32) -> String {
    if depth == 0 {
        return leaf(rng);
    }
    let op = INNER_OPS[rng.below(INNER_OPS.len() as u64) as usize];
    format!("({} {op} {})", inner(rng, depth - 1), inner(rng, depth - 1))
}

/// A two-yield stream, the shape the float drivers were built for.
fn program(rng: &mut Rng) -> String {
    let e1 = inner(rng, EXPR_DEPTH);
    let e2 = inner(rng, EXPR_DEPTH);
    let op = REPLY_OPS[rng.below(REPLY_OPS.len() as u64) as usize];
    format!("loop main(t: Float) -> Float {{ let r = yield {e1}; yield (r {op} {e2}) }}")
}

const PROGRAMS: usize = 120;
const TICKS: usize = 6;
const SEEDS: &[u64] = &[0x5F10_0000_0000_0001, 0x5F10_0000_0000_0002];
const DISTINCT_FLOOR: usize = 90;

fn replies(n: usize) -> Vec<Flt> {
    (0..n)
        .map(|i| ((i as u64 % REPLY_MAX) + 1) as Flt)
        .collect()
}

/// **THE COMPARISON, SEQUENCE BY SEQUENCE.**
///
/// Comparing only a final value would pass while every intermediate diverged —
/// and the intermediates are what the spill and restore path actually affects.
#[test]
fn generated_float_streams_agree_with_the_reference() {
    let first: Flt = 2.0;
    let reps = replies(TICKS);
    assert_exact("the first argument", "<arguments>", wide(first));
    for r in &reps {
        assert_exact("a reply", "<arguments>", wide(*r));
    }

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut compared = 0usize;
    let mut yields = 0usize;

    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for i in 0..PROGRAMS {
            let src = program(&mut rng);
            seen.insert(src.clone());

            // **A REFUSAL IS NOT A DIVERGENCE.** If the backend declines one of
            // these, the generator has left its intended set and must be
            // narrowed -- reporting it as a finding would be a statement about
            // the generator dressed as one about the emitter.
            let refusals = keleusma_native::module_refusals(
                &common::build(&src),
                keleusma_native::LowerOptions::default(),
            );
            assert!(
                refusals.is_empty(),
                "generated stream {i} is REFUSED, not divergent:\n  {src}\n  \
                 {refusals:?}\nNarrow the generator and state the exclusion; do \
                 not record this as a backend finding."
            );

            let vm = vm_sequence(&src, first, &reps);
            let native = native_sequence(&src, first, &reps);
            for v in vm.iter().chain(native.iter()) {
                assert_exact("a yielded value", &src, *v);
            }
            yields += vm.len();
            compared += 1;
            assert_eq!(
                vm, native,
                "DIVERGENCE on generated float stream {i}:\n  {src}\n  \
                 reference = {vm:?}\n  native    = {native:?}\n\nEvery value on \
                 both sides is four-byte exact, so this is NOT the f64-versus-f32 \
                 harness artefact. The generator is seeded and deterministic. This \
                 establishes that the two implementations disagree, NOT which is \
                 right."
            );
        }
    }

    assert_eq!(
        compared,
        PROGRAMS * SEEDS.len(),
        "only {compared} of {} streams were compared",
        PROGRAMS * SEEDS.len()
    );
    assert!(
        seen.len() >= DISTINCT_FLOOR * SEEDS.len(),
        "the generator produced only {} distinct streams, so this is a slower \
         spelling of the four hand-written subjects",
        seen.len()
    );
    // **THE STREAMS REALLY SUSPENDED.** A driver that returned an empty sequence
    // for every program would satisfy every count above.
    assert_eq!(
        yields,
        compared * TICKS,
        "the streams yielded {yields} values in total, not the {} expected. A \
         stream that did not suspend is not exercising the yield arm at all.",
        compared * TICKS
    );
}

/// **THE FEEDBACK BOUND IS ARITHMETIC, AND THE ARITHMETIC IS CHECKED.**
///
/// The header argues that the reply entering only additively keeps the magnitude
/// bound STABLE rather than growing with the tick count. **An argument in a
/// comment is not a guard**: raising `EXPR_DEPTH` or `REPLY_MAX` would break the
/// invariant and the comparison above would keep passing until a value happened to
/// land outside the exact range.
#[test]
fn the_feedback_bound_is_arithmetic() {
    assert!(
        worst_case_yield() <= F32_EXACT_INT_LIMIT,
        "the worst-case yield is {}, above the four-byte exact-integer limit of \
         {F32_EXACT_INT_LIMIT}. A bound has been changed without redoing the \
         arithmetic.",
        worst_case_yield()
    );

    // **NON-VACUITY, AND IT IS THE POINT OF THE FILE.** Had the reply been
    // allowed under a multiplication, the bound would compound each tick. Six
    // ticks of that must exceed the limit, or the additive restriction is
    // guarding nothing.
    let leaf = REPLY_MAX.max(LITERAL_MAX) as f64;
    let mut compounding = leaf;
    for _ in 0..TICKS {
        compounding = compounding.powi(1 << EXPR_DEPTH);
        if compounding > F32_EXACT_INT_LIMIT {
            return;
        }
    }
    panic!(
        "a multiplicative reply would stay inside the four-byte limit after \
         {TICKS} ticks, reaching only {compounding}. Then the additive \
         restriction on the reply is not what keeps this file sound, and the \
         header's central argument is wrong."
    );
}

/// **THE GENERATED PROGRAMS REALLY ARE FLOAT STREAMS THAT NEST.**
#[test]
fn the_generated_streams_are_float_and_nested() {
    let mut rng = Rng(0x5F10_0000_0000_00FF);
    let mut nested = 0usize;
    for _ in 0..PROGRAMS {
        let src = program(&mut rng);
        assert!(
            src.starts_with("loop main(t: Float)") && src.contains("yield"),
            "a generated program is not a float stream: {src}"
        );
        assert!(
            !src.contains("r *"),
            "the reply appears under a multiplication, which breaks the feedback \
             bound this file rests on: {src}"
        );
        if src.matches('(').count() > 4 {
            nested += 1;
        }
    }
    assert!(
        nested * 10 >= PROGRAMS * 9,
        "only {nested} of {PROGRAMS} generated streams nest; the composition \
         claim is false"
    );
}
