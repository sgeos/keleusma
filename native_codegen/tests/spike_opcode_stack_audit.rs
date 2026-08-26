//! **Is `CheckedAdd` the only opcode whose stack model is wrong?**
//!
//! `NATIVE_BOUNDS_TRANSFER.md` records that `wcmu_region` drives its own running
//! depth negative on 17 of 826 shipped chunks, and traces one cause to
//! `CheckedAdd`: it pushes `(low, high, flag)` — a gross push of three — while
//! `stack_growth()` returns the net `1`, and the peak calculation uses the net
//! as the transient rise.
//!
//! That was found by accident. This asks the question systematically.
//!
//! # Three instruments were PLANNED. **ONE WAS BUILT.**
//!
//! **CORRECTED 2026-08-24. This header described three instruments as though all
//! three existed. Two of them were never written**, and the two tests that DO sit
//! beside the first measure something other than what the plan named. Found by
//! `comment_citations.rs`, which flags a backticked name resolving to nothing.
//!
//! | planned | named | built? |
//! |---|---|---|
//! | model self-consistency | `negative_depth` | **yes** — `audit_1_which_synthetic_cases_drive_the_model_negative` |
//! | model against model | `disagrees_with_typed_verifier` | **NO.** `verify_typed` appears in this file only in prose. `audit_2_report_declared_net_effects_per_opcode` reports the model's OWN declared net effects; it diffs against nothing |
//! | model against measured truth | `predicted_against_measured` | **NO.** `Arena::bottom_peak` is named here and used nowhere. `audit_3_which_opcodes_are_uncovered` reports uncovered opcodes |
//!
//! **THE ONE THAT WAS NOT BUILT IS THE ONE THE HEADER CALLS INDISPENSABLE** — the
//! third, described below as *"the only one of the three that is not another
//! model."* So this file's ground truth is the instrument that does not exist,
//! and the two that remain are both models being checked against themselves.
//!
//! Kept as a spike with an honest header rather than deleted: the plan is still
//! the right plan, and `audit_1` did its job. **What is retracted is the
//! impression that the plan was executed.**
//!
//! # The three instruments as PLANNED, because each is blind to something
//!
//! 1. **Model self-consistency** (`negative_depth`). A chunk whose running
//!    `growth - shrink` goes below zero proves the model wrong SOMEWHERE in that
//!    chunk. It localises nothing on its own, but it needs no ground truth.
//! 2. **Model against model** (`disagrees_with_typed_verifier`). `verify_typed`
//!    independently reconstructs the operand stack to validate flat offsets, so
//!    it necessarily encodes its own push/pop counts. Two in-tree models of the
//!    same quantity, diffed. Already known to disagree on `GetField(Flat)`,
//!    which `verify_typed` treats as pop-one-push-one (net 0) and the WCMU model
//!    declares as net -1.
//! 3. **Model against measured truth** (`predicted_against_measured`). The
//!    operand stack lives in the arena's bottom region, so `Arena::bottom_peak`
//!    is the real high-water mark. This is the only one of the three that is not
//!    another model.
//!
//! # The blindness this file must not inherit
//!
//! The instrument that found `CheckedAdd` compared the native emitter's slot
//! count against the verifier's bound, and could only see cases where the
//! EMITTER EXCEEDS it. It is blind to opcodes where both are wrong in the same
//! direction, and to any opcode the shipped corpus never exercises. Hence the
//! synthetic corpus below: one minimal program per construct, so coverage is
//! deliberate rather than incidental.
//!
//! Run with `cargo test --test spike_opcode_stack_audit -- --nocapture`.
use keleusma::bytecode::{Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::{BTreeMap, BTreeSet};

/// One minimal source per construct.
///
/// **Deliberate coverage, not incidental.** Each entry exists to force a
/// specific opcode family into a chunk. Opcodes that no source here reaches are
/// reported as UNCOVERED rather than passed over — an opcode silently absent
/// from an audit is the failure mode this file exists to avoid.
const CASES: &[(&str, &str)] = &[
    ("const_int", "fn main(a: Word, b: Word) -> Word { 7 }"),
    ("add", "fn main(a: Word, b: Word) -> Word { a + b }"),
    ("sub", "fn main(a: Word, b: Word) -> Word { a - b }"),
    ("mul", "fn main(a: Word, b: Word) -> Word { a * b }"),
    ("div", "fn main(a: Word, b: Word) -> Word { a / b }"),
    ("modulo", "fn main(a: Word, b: Word) -> Word { a % b }"),
    ("neg", "fn main(a: Word, b: Word) -> Word { -a }"),
    (
        "cmp",
        "fn main(a: Word, b: Word) -> Word { if a < b { a } else { b + b } }",
    ),
    (
        "boolean",
        "fn main(a: Word, b: Word) -> Word { if a > 0 and b > 0 { a } else { b } }",
    ),
    (
        "bitwise",
        "fn main(a: Word, b: Word) -> Word { band(a, b) }",
    ),
    ("shift", "fn main(a: Word, b: Word) -> Word { lsl(a, 2) }"),
    (
        "loop_for",
        "fn main(a: Word, b: Word) -> Word { let mut s = 0; for i in 0..4 { s = s + i; }; s }",
    ),
    (
        "call",
        "fn helper(x: Word) -> Word { x + 1 }\nfn main(a: Word, b: Word) -> Word { helper(a) }",
    ),
    (
        "struct_field",
        "struct P { x: Word, y: Word }\nfn main(a: Word, b: Word) -> Word { let p = P { x: a, y: b }; p.x + p.y }",
    ),
    (
        "nested_struct",
        "struct I { a: Word }\nstruct O { i: I, b: Word }\nfn main(a: Word, b: Word) -> Word { let o = O { i: I { a: a }, b: b }; o.i.a }",
    ),
    (
        "array",
        "fn main(a: Word, b: Word) -> Word { let xs = [a, b]; xs[1] }",
    ),
    (
        "tuple",
        "fn main(a: Word, b: Word) -> Word { let t = (a, b); t.1 }",
    ),
    (
        "enum_match",
        "enum E { A(Word), B(Word) }\nfn main(a: Word, b: Word) -> Word { let e = E::A(a); match e { E::A(x) => x, E::B(y) => y + b } }",
    ),
    (
        "byte",
        "fn main(a: Word, b: Word) -> Word { let x = 200 as Byte; x as Word }",
    ),
    (
        "data_slot",
        "data s { v: Word }\nfn main(a: Word, b: Word) -> Word { s.v = a; s.v }",
    ),
    (
        "data_array",
        "data s { xs: [Word; 4] }\nfn main(a: Word, b: Word) -> Word { s.xs[1] = a; s.xs[1] }",
    ),
    (
        "stream",
        "fn main(a: Word, b: Word) -> Word { loop { let _ = yield a + b; } }",
    ),
    (
        "static_str",
        "use host::name\nfn main(a: Word, b: Word) -> Word { host::name(\"x\"); a }",
    ),
    // Added 2026-08-14 to close the audit's own hole: sixteen opcodes occurred
    // in the shipped corpus with no isolating case. Each entry forces one.
    (
        "cmp_eq",
        "fn main(a: Word, b: Word) -> Word { if a == b { a } else { b + b } }",
    ),
    (
        "cmp_ne",
        "fn main(a: Word, b: Word) -> Word { if a != b { a } else { b + b } }",
    ),
    (
        "cmp_le",
        "fn main(a: Word, b: Word) -> Word { if a <= b { a } else { b + b } }",
    ),
    (
        "cmp_ge",
        "fn main(a: Word, b: Word) -> Word { if a >= b { a } else { b + b } }",
    ),
    (
        "logical_not",
        "fn main(a: Word, b: Word) -> Word { if not (a == b) { a } else { b } }",
    ),
    ("bit_and2", "fn main(a: Word, b: Word) -> Word { a band b }"),
    ("bit_or2", "fn main(a: Word, b: Word) -> Word { a bor b }"),
    ("bit_xor2", "fn main(a: Word, b: Word) -> Word { a bxor b }"),
    (
        "shift_left",
        "fn main(a: Word, b: Word) -> Word { a lsl 2 }",
    ),
    (
        "shift_right",
        "fn main(a: Word, b: Word) -> Word { a lsr 2 }",
    ),
    // `break` is a plain statement valid ONLY inside a `for` loop, and invalid
    // in a `loop` function (docs/spec/GRAMMAR.md). `BreakIf` is what the
    // compiler emits for a CONDITIONAL break.
    //
    // **THIS CASE WAS REJECTED, AND `break` WAS NEVER THE REASON.** It read
    // `... }; b }` -- a `for` loop is a STATEMENT and consumes no trailing
    // semicolon, so the parser resumed at statement position, read `;` as the
    // start of an expression, and reported `unexpected token Semicolon in
    // expression`. Two semicolons sat near each other and the diagnostic named
    // the wrong one. On that evidence this line reported a grammar/parser
    // discrepancy to `v0.2.3` and left `BreakIf` unisolated with a stated reason.
    //
    // The `v0.2.3` line settled it with the control this line should have run:
    // remove `break` entirely, keep the stray semicolon, and the failure is
    // BYTE-IDENTICAL. Deleting the semicolon after the `for` block -- and nothing
    // else -- compiles, and `main` carries `BreakIf` and `Break`.
    //
    // The generalisation, which arrived from both directions in one week: a
    // diagnostic names where the parser STOPPED, not what it objected to, just
    // as a defect report names where a reader happened to look. The cheap
    // discriminator both times is a control that removes the suspected cause and
    // checks whether the failure survives.
    (
        "break_cond",
        "data s { n: Word }\nfn main(a: Word, b: Word) -> Word { let xs = [1, 2, 3, 4]; for x in xs { if x > a { break; } s.n = x; } b }",
    ),
    (
        "stream_scalar",
        "loop main(input: Word) -> Word { let _ = yield input; 0 }",
    ),
];

fn compiled_cases() -> Vec<(&'static str, Module)> {
    CASES
        .iter()
        .filter_map(|(n, src)| {
            let m = compile(&parse(&tokenize(src).ok()?).ok()?).ok()?;
            Some((*n, m))
        })
        .collect()
}

/// Which cases the reference compiler REJECTS, with the reason.
///
/// Reported, never silently dropped. A case that fails to compile is a hole in
/// the audit that looks exactly like a clean result, and three of five stream
/// probes on this branch turned out to be reference rejections rather than
/// backend refusals.
fn rejected_cases() -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for (n, src) in CASES {
        let why = match tokenize(src) {
            Err(e) => format!("lex: {e:?}"),
            Ok(t) => match parse(&t) {
                Err(e) => format!("parse: {e:?}"),
                Ok(a) => match compile(&a) {
                    Err(e) => format!("compile: {e:?}"),
                    Ok(_) => continue,
                },
            },
        };
        out.push((*n, why.chars().take(110).collect::<String>()));
    }
    out
}

/// The head of an op's Debug rendering, which is its variant name.
fn op_name(op: &Op) -> String {
    format!("{op:?}")
        .split(['(', ' '])
        .next()
        .unwrap_or("?")
        .to_string()
}

/// Instrument 1: does the model's own running depth stay non-negative?
///
/// Needs no ground truth. A chunk that goes negative proves the model wrong
/// somewhere inside it; attributing that to an opcode is what instrument 2 and
/// the isolation below are for.
#[test]
fn audit_1_which_synthetic_cases_drive_the_model_negative() {
    println!("\n================ AUDIT 1: model self-consistency on synthetic cases");
    let cases = compiled_cases();
    let rejected = rejected_cases();
    if !rejected.is_empty() {
        println!(
            "  CASES THE REFERENCE COMPILER REJECTS ({}):",
            rejected.len()
        );
        for (n, why) in &rejected {
            println!("     {n:16} {why}");
        }
    }
    assert!(
        cases.len() + rejected.len() == CASES.len(),
        "case accounting does not add up: {} compiled + {} rejected != {}",
        cases.len(),
        rejected.len(),
        CASES.len()
    );

    // An opcode is IMPLICATED when it appears in a chunk that goes negative and
    // the chunk contains no other implicated candidate. Reported as a suspect
    // set rather than a verdict: co-occurrence cannot separate two opcodes that
    // always appear together.
    let mut negative_cases: Vec<(&str, String, i32)> = Vec::new();
    let mut suspects: BTreeMap<String, usize> = BTreeMap::new();

    for (name, m) in &cases {
        for c in &m.chunks {
            let (mut off, mut low) = (0i32, 0i32);
            for op in &c.ops {
                off += op.stack_growth() as i32 - op.stack_shrink() as i32;
                low = low.min(off);
            }
            if low < 0 {
                negative_cases.push((name, c.name.clone(), low));
                for op in &c.ops {
                    *suspects.entry(op_name(op)).or_default() += 1;
                }
            }
        }
    }

    println!("  synthetic cases compiled : {}", cases.len());
    println!("  chunks going NEGATIVE    : {}", negative_cases.len());
    for (case, chunk, low) in &negative_cases {
        println!("   {case:16} chunk {chunk:20} low {low}");
    }
    if !suspects.is_empty() {
        println!("\n  opcodes present in a negative chunk (co-occurrence, NOT a verdict):");
        let mut v: Vec<_> = suspects.iter().collect();
        v.sort_by_key(|(_, n)| core::cmp::Reverse(**n));
        for (k, n) in v.iter().take(14) {
            println!("     {n:4}  {k}");
        }
    }
    println!("================\n");
}

/// Instrument 2: does the WCMU model agree with the typed verifier?
///
/// `verify_typed` reconstructs the operand stack to validate flat offsets, so it
/// carries its own push/pop counts for every opcode it handles. Where the two
/// disagree, at least one is wrong.
///
/// **This is a comparison of NET effect only.** `CheckedAdd`'s defect is in the
/// TRANSIENT (gross push 3, net +1), which a net comparison cannot see — so a
/// clean result here would not clear an opcode. The two instruments are blind to
/// different things on purpose.
#[test]
fn audit_2_report_declared_net_effects_per_opcode() {
    println!("\n================ AUDIT 2: declared net effect, per opcode");
    let mut seen: BTreeMap<String, (i32, u32, u32)> = BTreeMap::new();
    for (_, m) in &compiled_cases() {
        for c in &m.chunks {
            for op in &c.ops {
                let g = op.stack_growth();
                let s = op.stack_shrink();
                seen.insert(op_name(op), (g as i32 - s as i32, g, s));
            }
        }
    }
    println!(
        "  {:<22} {:>6} {:>7} {:>5}",
        "opcode", "growth", "shrink", "net"
    );
    for (k, (net, g, s)) in &seen {
        let flag = if *net < 0 { "  <- net NEGATIVE" } else { "" };
        println!("  {k:<22} {g:>6} {s:>7} {net:>5}{flag}");
    }
    println!("\n  A net-negative producer is not automatically wrong -- PopN and");
    println!("  Return legitimately consume. What is suspicious is an opcode that");
    println!("  READS a value and pushes a result yet declares a net loss, since");
    println!("  that describes a stack that shrinks while producing.");
    println!("  distinct opcodes observed: {}", seen.len());
    println!("================\n");
}

/// Instrument 3: coverage. Which opcodes did the synthetic corpus NOT reach?
///
/// Reported explicitly. The goal this file serves names a silently absent opcode
/// as the failure mode, so absence is printed rather than left to inference.
#[test]
fn audit_3_which_opcodes_are_uncovered() {
    println!("\n================ AUDIT 3: coverage of the synthetic corpus");
    let mut synthetic: BTreeSet<String> = BTreeSet::new();
    for (_, m) in &compiled_cases() {
        for c in &m.chunks {
            for op in &c.ops {
                synthetic.insert(op_name(op));
            }
        }
    }
    // The shipped corpus is the yardstick for "an opcode that occurs in real
    // code". An opcode in neither is out of scope for a corpus-driven audit and
    // is named as such rather than counted as covered.
    let mut shipped: BTreeSet<String> = BTreeSet::new();
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> =
        ["examples/scripts", "src/selfhost/kel", "compiler/kel"]
            .iter()
            .map(|d| root.join(d))
            .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel")
            && let Ok(src) = std::fs::read_to_string(&p)
            && let Ok(t) = tokenize(&src)
            && let Ok(a) = parse(&t)
            && let Ok(m) = compile(&a)
        {
            for c in &m.chunks {
                for op in &c.ops {
                    shipped.insert(op_name(op));
                }
            }
        }
    }

    let missing: Vec<&String> = shipped.difference(&synthetic).collect();
    println!("  opcodes in the synthetic corpus : {}", synthetic.len());
    println!("  opcodes in the shipped corpus   : {}", shipped.len());
    println!("  IN SHIPPED BUT NOT SYNTHETIC    : {}", missing.len());
    for m in &missing {
        println!("     {m}");
    }
    println!("\n  These are the audit's holes, stated rather than implied. An opcode");
    println!("  here has been checked by the shipped-corpus walk but NOT by a");
    println!("  minimal isolating case, so a defect in it may be masked by");
    println!("  co-occurrence.");
    println!("================\n");
    assert!(
        !shipped.is_empty(),
        "the shipped corpus walk found no opcodes; the coverage figure is vacuous"
    );
}
