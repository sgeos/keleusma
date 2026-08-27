//! `reconstruct.kel`'s failure modes, named.
//!
//! # Why this file exists
//!
//! The stage declares **twenty-six** arrays in **six** size classes. An unguarded
//! out-of-range index reports `IndexOutOfBounds(i, N)`, where `N` identifies a size class
//! and not an array, so **twenty-five of the twenty-six share a message with at least one
//! sibling**. That is the defect `parse.kel` carried until thirteen causes were given
//! their own names, and tracing one such failure there cost seven increments.
//!
//! The message that provoked this work, `IndexOutOfBounds(-1, 1024)`, is the worst case in
//! the class: it reads as a capacity bound and means the opposite. `pop()` decremented
//! before indexing, so an empty stack read slot `-1`. The `1024` is the array's size and
//! has nothing to do with the cause.
//!
//! # Scope, stated so the gap is visible rather than implied
//!
//! Guards cover the **1024-wide class** — the seven arrays the observed failure lives in.
//! The other nineteen arrays, in five size classes, are **unguarded**, and
//! [`the_unguarded_arrays_are_named`] lists them so the remainder is a register rather
//! than an absence.

// This file drives the self-hosted `reconstruct.kel` stage, so it exists only when the
// stage does. Without this the three feature sets continuous integration runs that lack
// `self-host` fail to COMPILE it, which is how the first push of this file went red on
// four jobs while a local `--features self-host` run was green on all three signals.
#![cfg(all(feature = "self-host", feature = "compile"))]

use keleusma::bytecode::Value;
use keleusma::selfhost::{reconstruct_kel_module, seed_reconstruct_shared};
use keleusma::selfhost_host::{
    RECONSTRUCT_DIAG_TAG_BASE, RECONSTRUCT_NODE_CAP, describe_reconstruct_diagnostic,
};
use keleusma::vm::{Vm, VmState, required_persistent_capacity_for};
use keleusma_arena::Arena;

const STAGE: &str = include_str!("../src/selfhost/kel/reconstruct.kel");

/// The appended diagnostic slots. Derived from the block layout rather than restated, so
/// a slot inserted above them moves these too.
const RC_AST_BASE: usize = 3 + 1024 * 2;
const RC_HEAD_COUNT: usize = RC_AST_BASE + 1 + 1024 * 4 + 256 * 5 + 2;
const RC_ERR_CODE: usize = RC_HEAD_COUNT + 1 + 16 * 4;
const RC_ERR_DETAIL: usize = RC_ERR_CODE + 1;

/// Drive the stage over a crafted record stream and return `(yielded, code, detail)`.
///
/// Driving the STAGE rather than the whole compiler is deliberate: a provoking input
/// expressed as Keleusma source would have to survive the lexer, the parser and the
/// record transport first, and a failure anywhere upstream would look like this guard
/// firing.
fn drive(records: &[(i64, i64)]) -> (i64, i64, i64) {
    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity((1 << 22) + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
    let mut shared = seed_reconstruct_shared(&vm, records, 0, 0);
    let yielded = match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call")
    {
        VmState::Yielded(Value::Int(n)) => n,
        other => panic!("unexpected state: {other:?}"),
    };
    let rd = |slot: usize| match vm.get_shared(&shared, slot).unwrap() {
        Value::Int(n) => n,
        o => panic!("expected Int at {slot}, got {o:?}"),
    };
    (yielded, rd(RC_ERR_CODE), rd(RC_ERR_DETAIL))
}

/// Record kind 1 is `Literal`: a leaf that appends one node and pushes it.
const LITERAL: i64 = 1;
/// Record kind 3 is `BinOp`: pops its right child then its left.
const BINOP: i64 = 3;

// ---------------------------------------------------------------------------
// 1. Distinct causes produce distinct messages.
// ---------------------------------------------------------------------------

/// Every code the driver knows renders a message no other code renders.
///
/// **This is the whole point of the exercise**, so it is asserted rather than described.
/// A table whose entries collapse onto one string would leave the stage exactly where it
/// started, with a message that cannot be attributed to a cause.
#[test]
fn every_named_cause_renders_a_distinct_message() {
    let mut seen: Vec<(i64, String)> = Vec::new();
    for code in 1..=5 {
        let tag = RECONSTRUCT_DIAG_TAG_BASE - code;
        let msg = describe_reconstruct_diagnostic(tag, 7);
        for (other, prev) in &seen {
            assert_ne!(
                *prev, msg,
                "codes {other} and {code} render the same message, so the two causes are \
                 still indistinguishable to a reader"
            );
        }
        seen.push((code, msg));
    }
    assert_eq!(seen.len(), 5, "the code table lost an entry");
}

/// An underflow and an exhaustion on the SAME array must not read alike.
///
/// This is the specific confusion that produced `IndexOutOfBounds(-1, 1024)`: one message
/// served both directions, and the array size in it pointed a reader at the wrong one.
#[test]
fn an_underflow_does_not_read_as_an_exhaustion() {
    let full = describe_reconstruct_diagnostic(RECONSTRUCT_DIAG_TAG_BASE - 2, 1025);
    let empty = describe_reconstruct_diagnostic(RECONSTRUCT_DIAG_TAG_BASE - 3, 0);
    assert_ne!(full, empty);
    assert!(
        empty.to_lowercase().contains("underflow"),
        "the empty-stack message must say which direction it is, got: {empty}"
    );
    assert!(
        empty.to_lowercase().contains("not a capacity bound"),
        "the empty-stack message must deny the reading its old form invited, got: {empty}"
    );
}

/// A code the driver was never taught is reported as unknown, not folded onto a known one.
#[test]
fn an_unknown_cause_is_reported_as_unknown() {
    let msg = describe_reconstruct_diagnostic(RECONSTRUCT_DIAG_TAG_BASE - 99, 3);
    assert!(msg.contains("99"), "got: {msg}");
    assert!(msg.contains("does not know"), "got: {msg}");
}

// ---------------------------------------------------------------------------
// 2. The array family is derived from the source, not enumerated by hand.
// ---------------------------------------------------------------------------

/// Every `[Word; N]` declared in the stage, as `(name, size)`.
fn declared_arrays() -> Vec<(String, usize)> {
    let mut out = Vec::new();
    for line in STAGE.lines() {
        let t = line.trim();
        let Some((name, rest)) = t.split_once(": [Word; ") else {
            continue;
        };
        let Some((size, _)) = rest.split_once(']') else {
            continue;
        };
        if name.contains(' ') || name.starts_with("//") {
            continue;
        }
        if let Ok(n) = size.parse::<usize>() {
            out.push((name.to_string(), n));
        }
    }
    out
}

/// The derivation finds arrays at all, and finds the size classes the scope rests on.
///
/// **Asserted non-vacuous deliberately.** Two derivations in this repository passed while
/// finding nothing — one walked past a `[` and reported zero arrays — so a derived set
/// that is never checked for emptiness is a guard that cannot fire.
#[test]
fn the_array_family_is_derived_and_non_vacuous() {
    let arrays = declared_arrays();
    assert!(
        arrays.len() >= 20,
        "the derivation found only {} arrays, so it is matching something other than the \
         declarations it is meant to read",
        arrays.len()
    );
    assert!(
        arrays.iter().any(|(n, _)| n == "stack"),
        "the derivation missed `rs.stack`, the array the observed failure indexed"
    );

    // The message-collision claim this whole file rests on, measured rather than asserted.
    let shared_a_message = arrays
        .iter()
        .filter(|(_, s)| arrays.iter().filter(|(_, o)| o == s).count() > 1)
        .count();
    assert!(
        shared_a_message >= arrays.len() - 1,
        "at most one array should have a size class to itself; {} of {} do",
        arrays.len() - shared_a_message,
        arrays.len()
    );
}

/// The capacity the guards use is the capacity the arrays are declared at.
///
/// A guard whose limit disagrees with its array either refuses a legal program or fails to
/// refuse an illegal one, and neither shows up as a test failure anywhere else.
#[test]
fn the_reconstruct_guard_caps_match_their_arrays() {
    let guarded = [
        "stack", "kinds", "args", "lhs", "rhs", "rec_kind", "rec_arg",
    ];
    let arrays = declared_arrays();
    for name in guarded {
        let (_, size) = arrays
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("`{name}` is no longer declared in the stage"));
        assert_eq!(
            *size, RECONSTRUCT_NODE_CAP,
            "`{name}` is declared at {size} but the guards use {RECONSTRUCT_NODE_CAP}"
        );
    }
}

/// The tag base the driver uses is the tag base the stage yields.
///
/// Restated in two places by necessity — the stage cannot import Rust — so it is checked
/// rather than trusted. A driver that learned the base by observing one would only learn
/// it from a program that had already failed.
#[test]
fn the_reconstruct_diagnostic_tag_base_matches() {
    let want = format!(
        "fn rc_fail_base() -> Word {{ 0 - {} }}",
        -RECONSTRUCT_DIAG_TAG_BASE
    );
    assert!(
        STAGE.contains(&want),
        "the stage's `rc_fail_base` no longer matches RECONSTRUCT_DIAG_TAG_BASE"
    );
}

/// The nineteen arrays this increment does NOT guard, named with the reason.
///
/// A scope stated in prose is invisible to a reader who greps for coverage. Naming them
/// here makes the remainder a register that can shrink, and makes an array added to the
/// stage without a guard show up as a failure of this test rather than as silence.
#[test]
fn the_unguarded_arrays_are_named() {
    let guarded = [
        "stack", "kinds", "args", "lhs", "rhs", "rec_kind", "rec_arg",
    ];
    let arrays = declared_arrays();
    let unguarded: Vec<&str> = arrays
        .iter()
        .map(|(n, _)| n.as_str())
        .filter(|n| !guarded.contains(n))
        .collect();

    // The register. Every entry is unguarded because it lies outside the 1024-wide class
    // the observed failure occupies, NOT because it was judged safe.
    let register = [
        "call_args",
        "for_parts",
        "match_parts",
        "limit_parts",
        "head_parts",
        "head_guard_start",
        "head_guard_len",
        "head_body_start",
        "head_body_len",
        "pending",
        "epending",
        "bindpending",
        "sqpending",
        "aqe_rem",
        "eqfields",
        "seb",
        "se_nstk_phase",
        "se_nstk_remaining",
        "fp_stack",
    ];
    for name in &unguarded {
        assert!(
            register.contains(name),
            "`{name}` is declared in the stage, is not guarded, and is not in the register. \
             Guard it or record it; do not leave it silent."
        );
    }
    assert_eq!(
        unguarded.len(),
        register.len(),
        "the register has drifted from the stage's declarations"
    );
}

// ---------------------------------------------------------------------------
// 3. Every guard that CAN fire has been made to fire.
// ---------------------------------------------------------------------------

/// Code 5: a range that leaves more than one node.
///
/// Two literals and nothing to fold them means two roots. **Before this cause was named
/// the stage returned slot zero and discarded the second node silently** — a wrong answer,
/// not a trap, and nothing in the tree would have reported it.
#[test]
fn a_range_leaving_two_nodes_names_itself() {
    let (yielded, code, detail) = drive(&[(LITERAL, 1), (LITERAL, 2)]);
    assert_eq!(code, 5, "expected the range-arity cause, got code {code}");
    assert_eq!(detail, 2, "the detail must carry the depth left behind");
    assert_eq!(yielded, RECONSTRUCT_DIAG_TAG_BASE - 5);
    let msg = describe_reconstruct_diagnostic(yielded, detail);
    assert!(msg.contains("exactly one node"), "got: {msg}");
}

/// Code 5 again, in the other direction: a range that leaves none.
///
/// An empty record stream reduces to nothing. The old code returned `stack[0]`, which is
/// whatever the previous range left there — a STALE node index presented as a root.
#[test]
fn an_empty_range_names_itself_rather_than_returning_a_stale_root() {
    let (_, code, detail) = drive(&[]);
    assert_eq!(code, 5, "expected the range-arity cause, got code {code}");
    assert_eq!(detail, 0, "an empty range leaves depth zero");
}

/// Code 3: a pop from an empty work stack.
///
/// A binary operator with no operands beneath it. This is the cause whose unguarded form
/// was `IndexOutOfBounds(-1, 1024)`.
#[test]
fn a_pop_from_an_empty_stack_names_itself() {
    let (yielded, code, _) = drive(&[(BINOP, 1)]);
    assert_eq!(
        code, 3,
        "expected the empty-stack cause, got code {code}; this is the exact failure the \
         unguarded stage reported as an index of -1 into a 1024-wide array"
    );
    let msg = describe_reconstruct_diagnostic(yielded, 0);
    assert!(msg.to_lowercase().contains("underflow"), "got: {msg}");
}

/// Code 4: a record range longer than the input arrays hold.
///
/// **This one replaced a `LoopLimitExceeded` trap**, which is a virtual-machine message
/// naming no cause whatsoever. The walk carries `limit 1024`, so it aborted one iteration
/// before any per-index check could fire; the guard had to move onto the range length.
#[test]
fn a_range_longer_than_the_record_arrays_names_itself() {
    let over = RECONSTRUCT_NODE_CAP as i64 + 6;
    let records: Vec<(i64, i64)> = (0..over).map(|i| (LITERAL, i)).collect();
    let (_, code, detail) = drive(&records);
    assert_eq!(
        code, 4,
        "expected the record-range cause, got code {code}. A LoopLimitExceeded trap here \
         means the guard moved back onto the index and is unreachable again"
    );
    assert_eq!(
        detail, over,
        "the detail must carry the range end attempted"
    );
}

/// Code 1: more nodes than the node arrays hold.
///
/// **Reaching this took a correction.** Within a single range the cause is unreachable:
/// a range holds at most `RECONSTRUCT_NODE_CAP` records and every record appends at most
/// one node, so `node_count` cannot pass the cap. It becomes reachable only because
/// `node_count` accumulates ACROSS the ranges of a multiheaded function, and two heads may
/// read the same records. That is what this drives.
#[test]
fn exhausting_the_node_arrays_names_itself() {
    // A range that folds to exactly one node: one literal, then 256 (literal, binop)
    // pairs. 513 records, 513 nodes, depth one at the end. Any other arity would trip the
    // range-arity cause first and this test would pass for the wrong reason.
    let mut records = vec![(LITERAL, 0)];
    for i in 0..256 {
        records.push((LITERAL, i));
        records.push((BINOP, 1));
    }
    assert_eq!(records.len(), 513);

    let m = reconstruct_kel_module();
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity((1 << 22) + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify reconstruct.kel");
    let mut shared = seed_reconstruct_shared(&vm, &records, 0, 0);

    // Two heads over the SAME record range: 2 x 513 = 1026 nodes from 513 records.
    let put = |vm: &Vm<'_, '_>, sh: &mut [u8], slot: usize, v: i64| {
        vm.set_shared(sh, slot, Value::Int(v)).unwrap();
    };
    put(&vm, &mut shared, RC_HEAD_COUNT, 2);
    for h in 0..2 {
        put(&vm, &mut shared, RC_HEAD_COUNT + 1 + h, 0); // guard_start
        put(&vm, &mut shared, RC_HEAD_COUNT + 1 + 16 + h, 0); // guard_len: unguarded
        put(&vm, &mut shared, RC_HEAD_COUNT + 1 + 32 + h, 0); // body_start
        put(&vm, &mut shared, RC_HEAD_COUNT + 1 + 48 + h, 513); // body_len
    }

    match vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call")
    {
        VmState::Yielded(Value::Int(n)) => {
            assert!(n < 0, "expected a diagnostic, got a node count of {n}")
        }
        other => panic!("unexpected state: {other:?}"),
    };
    let rd = |slot: usize| match vm.get_shared(&shared, slot).unwrap() {
        Value::Int(n) => n,
        o => panic!("expected Int at {slot}, got {o:?}"),
    };
    assert_eq!(
        rd(RC_ERR_CODE),
        1,
        "expected the node-array cause, got code {}",
        rd(RC_ERR_CODE)
    );
    assert_eq!(
        rd(RC_ERR_DETAIL),
        RECONSTRUCT_NODE_CAP as i64 + 1,
        "the detail must carry the index attempted"
    );
}

/// Code 2 is UNREACHABLE, and this pins the invariant that makes it so.
///
/// `push` has exactly one caller, inside `emit`, and `emit`'s node guard fires first.
/// Since `sp` never exceeds `node_count`, the work stack cannot fill before the node
/// arrays do. **A guard that cannot fire is worse than none**, so the honest options were
/// to delete it or to pin the reason. Pinning is chosen because a second `push` caller
/// would make the guard live, and this test fails the moment one is added.
#[test]
fn the_work_stack_cannot_overflow_before_the_node_array() {
    let calls = STAGE
        .lines()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("//") && t.contains("push(") && !t.contains("push_fp(")
        })
        .count();
    assert_eq!(
        calls, 2,
        "`push` no longer has exactly one definition and one call site. If a second caller \
         was added, the work-stack guard is now reachable and needs a provoking input"
    );
}

// ---------------------------------------------------------------------------
// 4. The wire.kel failure names its own cause.
// ---------------------------------------------------------------------------

/// `wire.kel` COMPILES. This pin recorded the refusal that used to stand in its way.
///
/// **Re-aimed, not deleted.** What it now guards is that the named-cause machinery still
/// works on a stage that exercises it heavily: `wire.kel` drove three separate named causes
/// during its repair, and a regression that reintroduced any of them should fail here rather
/// than surface as a raw array index again.
///
/// The control comes first for the same reason it always did: without it, a compiler broken
/// on everything would satisfy the assertion and look like a fact about `wire.kel`.
#[test]
fn wire_kel_reports_a_named_cause_rather_than_an_array_index() {
    const WIRE: &str = include_str!("../src/selfhost/kel/wire.kel");
    const LEXER: &str = include_str!("../src/selfhost/kel/lexer.kel");

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let control = std::panic::catch_unwind(|| keleusma::selfhost::self_host_compile(LEXER));
    let subject = std::panic::catch_unwind(|| keleusma::selfhost::self_host_compile(WIRE));
    std::panic::set_hook(prev);

    assert!(
        control.is_ok(),
        "`lexer.kel` no longer self-compiles, so nothing below is about `wire.kel`"
    );

    // If it ever refuses again, the message must NAME a cause. A raw `IndexOutOfBounds` is
    // the state this whole family of guards exists to end.
    if let Err(e) = subject {
        let msg = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        assert!(
            msg.contains("reconstruct.kel refused"),
            "`wire.kel` refuses again and the message does not name its stage or cause. That \
             is the regression this file exists to prevent. Got: {msg}"
        );
        panic!("`wire.kel` no longer compiles; it refuses with: {msg}");
    }
}
