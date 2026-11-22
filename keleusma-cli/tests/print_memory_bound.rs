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
///
/// Since 2026-11-11 the type checker refuses this source, so `run` and `compile` report the
/// recursion as a compile error and never reach the memory analysis with it. The refusal
/// still names `recursive call detected`. The memory analysis is reached with a recursive
/// module through `recursive_module`, and with a source through `RUNTIME_COUNT`.
const UNBOUNDED: &str = "fn count_down(n: Word) -> Word {\n    if n <= 0 { 0 } else { count_down(n - 1) }\n}\n\nfn main() -> Word {\n    count_down(5)\n}\n";

/// A loop whose count arrives at runtime. It compiles, and its worst-case memory usage cannot
/// be bounded, so it is the source that still reaches the memory analysis without a bound.
/// Taken from the same chapter of the guide.
const RUNTIME_COUNT: &str = "fn process(n: Word) -> Word {\n    for i in 0..n {\n        let _step = i;\n    }\n    0\n}\n\nfn main() -> Word {\n    process(3)\n}\n";

/// A recursive module, as bytecode from a producer other than this compiler would be.
///
/// The compiler refuses recursion, so the module is the acyclic twin of `UNBOUNDED` with the
/// body of `count_down` replaced by a call to itself.
fn recursive_module() -> keleusma::bytecode::Module {
    use keleusma::bytecode::Op;
    let twin =
        "fn count_down(n: Word) -> Word {\n    n\n}\n\nfn main() -> Word {\n    count_down(5)\n}\n";
    let tokens = keleusma::lexer::tokenize(twin).expect("lex the fixture");
    let program = keleusma::parser::parse(&tokens).expect("parse the fixture");
    let mut module = keleusma::compiler::compile(&program).expect("compile the fixture");
    let index = module
        .chunks
        .iter()
        .position(|c| c.name == "count_down")
        .expect("the count_down chunk");
    module.chunks[index].ops = vec![Op::GetLocal(0), Op::Call(index as u16, 1), Op::Return];
    module
}

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
    // The reason itself, not a generic failure. A message that said only "failed" would
    // leave an operator unable to tell an unbounded program from a broken tool. Since
    // 2026-11-11 the reason comes from the type checker.
    assert!(
        text.contains("recursive call detected"),
        "the refusal does not name the cause, so it cannot be acted on: {text}"
    );

    // A source the compiler accepts and the memory analysis cannot bound takes the path this
    // test was written for, and is refused with the verifier's own reason.
    let src = dir.write("count.kel", RUNTIME_COUNT);
    let (ok, text) = print_memory(&src);
    assert!(
        !ok && !text.contains("arena:"),
        "a figure was printed for a loop with a runtime count: {text}"
    );
    assert!(
        text.contains("no statically extractable iteration bound"),
        "the refusal does not name the verifier's cause: {text}"
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
    let bnd = dir.write("tune.kel", BOUNDED);

    // Neither `compile` nor the library compiler emits the recursive program, the first
    // since the ruling of 2026-10-30 and the second since the type checker began refusing
    // recursion on 2026-11-11. Its artifact is therefore a module built by hand, as
    // bytecode from another producer would be, so the bytecode path of `run` is still
    // reached with a module that has no bound.
    let rec_bin = dir.0.join("rec.bin");
    let module = recursive_module();
    std::fs::write(&rec_bin, module.to_bytes().expect("serialize the fixture"))
        .expect("write the fixture");

    let tune_bin = dir.0.join("tune.bin");
    let st = Command::new(BIN)
        .arg("compile")
        .arg(&bnd)
        .arg("-o")
        .arg(&tune_bin)
        .output()
        .expect("compile the fixture");
    assert!(
        st.status.success(),
        "the bounded fixture did not compile: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    let artifacts = [rec_bin, tune_bin];

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

/// **`compile` REFUSES A PROGRAM WHOSE MEMORY BOUND CANNOT BE COMPUTED.**
///
/// Such a module can be loaded by no host at any arena capacity, so emitting it only moves
/// the refusal to a later machine. On the operator's ruling of 2026-10-30. The bounded
/// control shows that the refusal is about the bound and not about `compile` failing.
#[test]
fn compile_refuses_a_program_with_no_memory_bound() {
    let dir = TmpDir::new("compile-refusal");
    let unb = dir.write("count.kel", RUNTIME_COUNT);
    let out = dir.0.join("count.bin");
    let st = Command::new(BIN)
        .arg("compile")
        .arg(&unb)
        .arg("-o")
        .arg(&out)
        .output()
        .expect("run compile");
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(
        !st.status.success(),
        "compile emitted an unloadable module: {err}"
    );
    assert!(
        err.contains("cannot be computed")
            && err.contains("no statically extractable iteration bound"),
        "the refusal does not name the bound and its reason: {err}"
    );
    assert!(
        !out.exists(),
        "compile refused but still wrote {}",
        out.display()
    );

    // The recursive program is refused earlier, by the type checker, and writes nothing.
    let rec = dir.write("rec.kel", UNBOUNDED);
    let rec_out = dir.0.join("rec.bin");
    let st = Command::new(BIN)
        .arg("compile")
        .arg(&rec)
        .arg("-o")
        .arg(&rec_out)
        .output()
        .expect("run compile");
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(
        !st.status.success() && err.contains("recursive call detected"),
        "compile did not refuse the recursive program by name: {err}"
    );
    assert!(
        err.contains("`count_down` -> `count_down`"),
        "the refusal does not name the cycle: {err}"
    );
    assert!(!rec_out.exists(), "compile refused but still wrote a file");

    let bnd = dir.write("tune.kel", BOUNDED);
    let st = Command::new(BIN)
        .arg("compile")
        .arg(&bnd)
        .arg("-o")
        .arg(dir.0.join("tune.bin"))
        .output()
        .expect("run compile");
    assert!(
        st.status.success(),
        "the bounded control did not compile: {}",
        String::from_utf8_lossy(&st.stderr)
    );
}
