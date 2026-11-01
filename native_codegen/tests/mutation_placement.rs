//! **EVERY REGISTERED MUTATION MUST STILL MATCH THE EMITTER, AND NINE STOPPED
//! WITHOUT ANYTHING SAYING SO.**
//!
//! # The decay this closes
//!
//! `tools/mutation_sweep.py` holds 53 pre-registered mutations. Each is a literal
//! snippet of `src/lib.rs` plus its replacement, so a mutation only applies while
//! its text still occurs in the emitter **exactly once**.
//!
//! Measured 2026-09-06: **nine had stopped placing** — `Return`, `Div`, `Mod`,
//! `GetData`, `SetData`, `GetDataIndexed`, `SetDataIndexed`, `Yield` and
//! `GetIndex`. Every one was a REFORMATTING or a SIGNATURE change:
//! `resolve_shared_scalar` and `resolve_shared_array` gained a `float_bytes`
//! parameter and their call sites reflowed, `st.b.build_return(Some(&v))` became
//! `build_typed_return(...)`, and `SK::Int => 8` folded into
//! `SK::Int | SK::Fixed => 8`. **No opcode had stopped being lowered.**
//!
//! **Nothing announced any of it.** The sweep was too expensive to run, so those
//! opcodes carried no coverage evidence for weeks with no signal at all. A
//! mutation that does not place is a silent no-op, and it looks exactly like
//! "nothing detected it" — which is worse than useless, because it reads as a
//! HOLE.
//!
//! # Why this is a test and not a step in the sweep
//!
//! **Placement is textual and needs no sweep**, so it costs a fraction of a
//! second and can sit in the ordinary suite. The sweep itself takes tens of
//! minutes and is run deliberately; a decay that only the sweep can see is a
//! decay nobody sees.
//!
//! # WHAT THIS DOES NOT ESTABLISH, SAID PLAINLY
//!
//! **Placing is necessary for a verdict and never sufficient.** A mutation can
//! place and still be worthless — it may abort lowering, or its opcode may have
//! no executing witness in the corpus, which is the state `BitAnd`, `BitOr`,
//! `BitXor` and `Shr` are in. This test says the tables have not rotted. It says
//! nothing about what the sweep would find.

use std::process::Command;

fn run_check(dir: &str) -> std::process::Output {
    Command::new("python3")
        .args(["tools/mutation_sweep.py", "--check-placement"])
        .current_dir(dir)
        .output()
        .expect("run the placement check; python3 must be on PATH")
}

/// **Every registered mutation places exactly once.**
#[test]
fn every_registered_mutation_still_matches_the_emitter() {
    let out = run_check(".");
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    println!("{text}");
    assert!(
        out.status.success(),
        "A REGISTERED MUTATION NO LONGER MATCHES THE EMITTER. This is almost \
         always a REFORMATTING or SIGNATURE change rather than a removed opcode, \
         and it silently retires that opcode's mutation coverage until the text \
         is re-registered. Read the current arm and update the entry, keeping the \
         discriminating shape; do not delete the entry to restore green. Nine \
         entries decayed this way before anything noticed.\n\n{text}"
    );
}

/// **THE MUST-FIRE CONTROL: the checker has to be able to report a miss.**
///
/// Without this, the test above would pass just as happily against a checker that
/// always exits zero, and this line has had three textual censuses falsified by
/// exactly that. A copy of the tool is made in a temporary directory with one
/// registered snippet corrupted, and the checker must fail on it.
///
/// **The real tool and the real emitter are never touched**, which matters
/// because the sweep mutates `src/lib.rs` in place and a test that did the same
/// could race the developer's working tree.
#[test]
fn the_checker_reports_a_mutation_that_does_not_place() {
    let tool = std::fs::read_to_string("tools/mutation_sweep.py").expect("read the tool");
    // Corrupt the FIRST registered snippet so it cannot match the emitter.
    let needle = "Op::CmpEq => IntPredicate::EQ,";
    assert!(
        tool.contains(needle),
        "the control's chosen snippet is not in the tool any more, so this control \
         is not exercising what it claims; pick another registered snippet"
    );
    let corrupted = tool.replacen(needle, "Op::CmpEq => IntPredicate::NEVER_EMITTED,", 1);

    let dir = std::env::temp_dir().join(format!("kel_mutplace_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("tools")).expect("scratch dir");
    std::fs::create_dir_all(dir.join("src")).expect("scratch src");
    std::fs::write(dir.join("tools/mutation_sweep.py"), &corrupted).expect("write tool copy");
    std::fs::copy("src/lib.rs", dir.join("src/lib.rs")).expect("copy the emitter");

    let out = run_check(dir.to_str().expect("path"));
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        !out.status.success(),
        "THE CHECKER PASSED A MUTATION THAT CANNOT MATCH THE EMITTER, so its \
         clean verdict above is vacuous and says nothing about the tables:\n\n{text}"
    );
    assert!(
        text.contains("CmpEq"),
        "the checker failed, but did not NAME the entry that stopped placing, so \
         a real failure would not tell anyone which one to fix:\n\n{text}"
    );
}
