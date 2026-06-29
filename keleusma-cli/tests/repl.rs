//! Black-box integration tests for the interactive REPL.
//!
//! These drive the built `keleusma` binary in `repl` mode over stdin and
//! assert on its output, exercising the real command dispatch, the
//! `:run`/`:resume` stepping sub-prompt, and `:save`/`:load` end to end.
//! Driving the binary (rather than calling internal functions) is the
//! faithful way to test code that reads `io::stdin()` in a loop.
//!
//! Input is written to the child's stdin and the pipe is then closed, so a
//! session that omits `:quit` still terminates on the resulting EOF; every
//! test nonetheless ends explicitly to keep intent clear and avoid hangs.

use std::io::Write;
use std::process::{Command, Stdio};

/// Run the REPL with `input` on stdin; return `(stdout, stderr)`.
fn run_repl(input: &str) -> (String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_keleusma"))
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn keleusma repl");
    // Write all input, then drop the handle to close stdin (EOF).
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(input.as_bytes())
        .expect("write to child stdin");
    let out = child.wait_with_output().expect("wait for repl");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn run_steps_a_loop_main_and_evolves_shared_state() {
    // The loop adds the tick to a shared counter and yields it. Stepping with
    // the default tick convention (yield + 1) gives ticks 1, 2, 4, so the
    // counter reads 1, 3, 7 across the first three presentations.
    let input = concat!(
        "shared data state { count: Word }\n",
        "loop main(tick: Word) -> Word { state.count = state.count + tick; yield state.count }\n",
        ":run\n",
        ":resume\n",
        ":resume\n",
        ":shared\n",
        ":stop\n",
        ":quit\n",
    );
    let (out, err) = run_repl(input);
    assert!(out.contains("stepping loop main"), "out: {out}\nerr: {err}");
    assert!(out.contains("yield => 1"), "out: {out}");
    assert!(out.contains("yield => 3"), "out: {out}");
    assert!(out.contains("yield => 7"), "out: {out}");
    // The shared-data renderer names the field and shows the evolved value.
    assert!(out.contains("state.count = 1"), "out: {out}");
    assert!(out.contains("state.count = 7"), "out: {out}");
    assert!(out.contains("stopped stepping"), "out: {out}");
}

#[test]
fn resume_with_explicit_tick_overrides_the_convention() {
    // `:resume 10` injects tick 10 instead of the convention's next tick, so
    // the counter jumps from 1 to 11.
    let input = concat!(
        "shared data state { count: Word }\n",
        "loop main(tick: Word) -> Word { state.count = state.count + tick; yield state.count }\n",
        ":run\n",
        ":resume 10\n",
        ":stop\n",
        ":quit\n",
    );
    let (out, _err) = run_repl(input);
    assert!(out.contains("yield => 1"), "out: {out}");
    assert!(out.contains("yield => 11"), "out: {out}");
    assert!(out.contains("state.count = 11"), "out: {out}");
}

#[test]
fn run_an_atomic_fn_main_prints_its_result_without_stepping() {
    let input = concat!("fn main() -> Word { 6 * 7 }\n", ":run\n", ":quit\n");
    let (out, _err) = run_repl(input);
    assert!(out.contains("=> 42"), "out: {out}");
    // An atomic fn does not enter the stepping sub-prompt.
    assert!(!out.contains("stepping"), "out: {out}");
}

#[test]
fn run_without_a_main_reports_a_helpful_error() {
    let input = concat!(
        "fn double(x: Word) -> Word { x * 2 }\n",
        ":run\n",
        ":quit\n",
    );
    let (_out, err) = run_repl(input);
    assert!(err.contains(":run needs a `main`"), "err: {err}");
}

#[test]
fn save_then_load_round_trips_the_session_program() {
    let path =
        std::env::temp_dir().join(format!("keleusma_repl_saveload_{}.kel", std::process::id()));
    let path_str = path.to_str().expect("utf8 temp path").to_string();
    let _ = std::fs::remove_file(&path);

    let input = format!(
        concat!(
            "fn double(x: Word) -> Word {{ x * 2 }}\n",
            ":save {p}\n",
            ":reset\n",
            ":load {p}\n",
            "double(21)\n",
            ":quit\n",
        ),
        p = path_str
    );
    let (out, err) = run_repl(&input);
    assert!(out.contains("saved session to"), "out: {out}\nerr: {err}");
    assert!(out.contains("loaded"), "out: {out}");
    // The loaded program runs: double(21) == 42.
    assert!(out.contains("42"), "out: {out}");

    // The saved file holds exactly the program that was typed.
    let saved = std::fs::read_to_string(&path).expect("read saved .kel file");
    assert!(
        saved.contains("fn double(x: Word) -> Word { x * 2 }"),
        "saved: {saved}"
    );
    let _ = std::fs::remove_file(&path);
}
