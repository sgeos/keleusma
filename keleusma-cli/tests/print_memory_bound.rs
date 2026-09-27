//! **`run --print-memory` REPORTS A COMPUTED BOUND OR NO BOUND, NEVER A DEFAULT.**
//!
//! The flag's documented purpose is to turn the static worst-case-memory bound into an
//! operational figure, so an operator can size or qualify a host before deploying. It used to
//! fabricate that figure: the sizing call ended in `unwrap_or(DEFAULT_ARENA_CAPACITY)`, so a
//! program whose worst-case memory usage cannot be bounded printed the default arena as
//! though it were the program's bound, and exited zero.
//!
//! Measured 2026-09-27 on the recursive example from `book/src/19_why_rejected.md`:
//! `arena: 65536 bytes total (persistent 0, transient 65536)`, exit 0 — for a program the
//! runtime refuses on load with `recursive call detected during WCMU topological sort`.
//! Provisioning a host from that figure means provisioning for a program that cannot run.
//!
//! The crate's value proposition is a DEFINITIVE worst-case bound, and its own instructions
//! forbid implying completeness where verification is incomplete. The one tool whose entire
//! output is that bound is the sharpest place for that to go wrong.
//!
//! **THE CONTROLS ARE WHY THE REFUSALS MEAN ANYTHING.** A test asserting only that the
//! unbounded program is refused would pass against a binary that refused everything,
//! including one that failed to parse its own arguments. Each refusal here is paired with a
//! program that must still report.
//!
//! **NO EXPECTED BYTE COUNT IS ASSERTED.** This session removed two stale byte-count claims
//! from the guide for exactly that reason: the size of a compiled artifact, and the exact
//! arena figure, move with the compiler. What is asserted is that a figure was produced, and
//! that a refusal names the verifier's own cause.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_keleusma");

/// Recursion makes the worst-case memory usage unprovable, so no bound exists. Taken from
/// the guide's worked rejection, which keeps the fixture and the documentation in step.
const UNBOUNDED: &str = "fn count_down(n: Word) -> Word {\n    if n <= 0 { 0 } else { count_down(n - 1) }\n}\n\nfn main() -> Word {\n    count_down(5)\n}\n";

/// An atomic program whose bound is computable.
const BOUNDED: &str = "fn main() -> Word { 60 + 7 }\n";

/// A stream program, which the runner drives forever and whose bound is also computable.
/// Included because the reporting path is the only way to size one without running it.
const STREAM: &str = "loop main(input: Word) -> Word {\n    let _ = yield input;\n    0\n}\n";

struct TmpDir(PathBuf);

impl TmpDir {
    fn new(tag: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("keleusma_printmem_{}_{}", std::process::id(), tag));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TmpDir(dir)
    }
    fn write(&self, name: &str, body: &str) -> PathBuf {
        let p = self.0.join(name);
        std::fs::write(&p, body).expect("write fixture");
        p
    }
}

impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run `keleusma run <path> --print-memory` and return (exit-ok, combined output).
fn print_memory(path: &PathBuf) -> (bool, String) {
    let out = Command::new(BIN)
        .arg("run")
        .arg(path)
        .arg("--print-memory")
        .output()
        .expect("run the CLI");
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), text)
}

/// **AN UNBOUNDABLE PROGRAM GETS A REFUSAL NAMING ITS CAUSE, NOT A FIGURE.**
#[test]
fn a_program_with_no_memory_bound_is_refused_rather_than_given_a_default_figure() {
    let dir = TmpDir::new("unbounded");
    let src = dir.write("rec.kel", UNBOUNDED);
    let (ok, text) = print_memory(&src);

    assert!(
        !ok,
        "reporting a worst-case bound for a program that has none exited successfully. \
         Output was: {text}"
    );
    assert!(
        !text.contains("arena:"),
        "a figure was printed for a program with no bound, which is the defect itself: {text}"
    );
    // The verifier's own reason, not a generic failure. A message that said only "failed"
    // would leave an operator unable to tell an unbounded program from a broken tool.
    assert!(
        text.contains("recursive call detected"),
        "the refusal does not name the verifier's cause, so it cannot be acted on: {text}"
    );
}

/// **THE CONTROL: A BOUNDED PROGRAM STILL REPORTS.**
///
/// Without this, the assertion above is satisfied by a binary that refuses everything.
#[test]
fn a_bounded_program_still_reports_its_arena_figure() {
    let dir = TmpDir::new("bounded");
    for (name, body) in [("tune.kel", BOUNDED), ("pulse.kel", STREAM)] {
        let src = dir.write(name, body);
        let (ok, text) = print_memory(&src);
        assert!(ok, "{name} should report a figure and did not: {text}");
        assert!(
            text.contains("arena:") && text.contains("transient"),
            "{name} produced no arena figure: {text}"
        );
        // A figure, not a particular figure. The exact byte count moves with the compiler,
        // and pinning it is the mistake this session removed from two book chapters.
        let digits = text.chars().filter(char::is_ascii_digit).count();
        assert!(digits > 0, "{name} reported no number at all: {text}");
    }
}

/// **THE SAME HOLDS WHEN THE INPUT IS COMPILED BYTECODE.**
///
/// `run` accepts a `.kel` source or a `.bin` artifact, and the reporting path is reached
/// separately in each. The defect was present on both, so both are pinned; a fix applied to
/// one would otherwise read as covering the other.
#[test]
fn the_bytecode_input_path_behaves_the_same_in_both_directions() {
    let dir = TmpDir::new("bytecode");
    let unb = dir.write("rec.kel", UNBOUNDED);
    let bnd = dir.write("tune.kel", BOUNDED);

    let mut artifacts = Vec::new();
    for (src, out) in [(&unb, "rec.bin"), (&bnd, "tune.bin")] {
        let outp = dir.0.join(out);
        let st = Command::new(BIN)
            .arg("compile")
            .arg(src)
            .arg("-o")
            .arg(&outp)
            .output()
            .expect("compile the fixture");
        // MEASURED, NOT ASSUMED: `compile` writes an artifact for the unbounded program as
        // well. It does not run the resource-bound verification, so it emits a module that
        // no host can load. That is a separate question from this file's subject and is
        // recorded rather than changed here; the assertion is only that the fixture built,
        // so a failure to build cannot masquerade as a passing refusal below.
        assert!(
            st.status.success(),
            "the fixture {out} did not compile: {}",
            String::from_utf8_lossy(&st.stderr)
        );
        artifacts.push(outp);
    }

    let (ok_unb, t_unb) = print_memory(&artifacts[0]);
    assert!(
        !ok_unb,
        "the bytecode path reported a bound that does not exist: {t_unb}"
    );
    assert!(
        t_unb.contains("recursive call detected") && !t_unb.contains("arena:"),
        "the bytecode path's refusal is wrong in shape: {t_unb}"
    );

    let (ok_bnd, t_bnd) = print_memory(&artifacts[1]);
    assert!(
        ok_bnd,
        "the bytecode path refused a bounded program: {t_bnd}"
    );
    assert!(
        t_bnd.contains("arena:"),
        "no figure from the bytecode path: {t_bnd}"
    );
}
