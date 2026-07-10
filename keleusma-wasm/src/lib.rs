//! WebAssembly bindings for the Keleusma compiler, powering the browser
//! playground.
//!
//! - [`check`] runs the full static pipeline (`tokenize` → `parse` → `compile`
//!   → `verify`) and reports the per-chunk worst-case-execution-time and
//!   worst-case-memory-usage bounds — the definitive bounds no other language's
//!   playground shows.
//! - [`keywords`] exposes the authoritative keyword list for the highlighter.
//! - [`Session`] *runs* the program step by step, surfacing the
//!   `Yielded`/`Reset`/`Finished` state and value at each step so the page can
//!   drive a Resume button and a debugger-style indicator.
//!
//! Programs that call host-native functions cannot run here; a pure program can.

use keleusma::Arena;
use keleusma::bytecode::{BlockType, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::Span;
use keleusma::verify::{
    verify, wcet_stream_iteration, wcet_whole_chunk, wcmu_stream_iteration, wcmu_whole_chunk,
};
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, GenericVmState, Vm, required_persistent_capacity_for};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Diagnostic {
    /// 1-based line of the offending construct.
    line: u32,
    /// 1-based column of the offending construct.
    column: u32,
    message: String,
}

impl Diagnostic {
    fn at(span: &Span, message: String) -> Self {
        Diagnostic {
            line: span.line,
            column: span.column,
            message,
        }
    }
}

#[derive(Serialize)]
struct Bound {
    chunk: String,
    /// What the bounds are measured over: `"iteration"` for a `loop` (stream)
    /// chunk, `"call"` for a `fn`, or `"resume"` for a `yield` chunk.
    basis: &'static str,
    /// Worst-case execution time in pipelined cycles, under the nominal cost
    /// model. `null` if not computable for this chunk.
    wcet_cycles: Option<u32>,
    /// Worst-case operand-stack bytes. `null` if not computable.
    wcmu_stack_bytes: Option<u32>,
    /// Worst-case heap (arena) bytes. `null` if not computable.
    wcmu_heap_bytes: Option<u32>,
}

#[derive(Serialize)]
struct CheckResult {
    /// True when the program lexes, parses, compiles, and verifies.
    ok: bool,
    diagnostics: Vec<Diagnostic>,
    bounds: Vec<Bound>,
}

fn analyze(src: &str) -> CheckResult {
    let tokens = match tokenize(src) {
        Ok(t) => t,
        Err(e) => return fail(Diagnostic::at(&e.span, e.message)),
    };
    let program = match parse(&tokens) {
        Ok(p) => p,
        Err(e) => return fail(Diagnostic::at(&e.span, e.message)),
    };
    let module = match compile(&program) {
        Ok(m) => m,
        Err(e) => return fail(Diagnostic::at(&e.span, e.message)),
    };
    if let Err(e) = verify(&module) {
        return fail(Diagnostic {
            line: 1,
            column: 1,
            message: format!("verifier rejected `{}`: {}", e.chunk_name, e.message),
        });
    }
    let bounds = module
        .chunks
        .iter()
        .map(|chunk| {
            // A stream (`loop`) chunk's bounds are per stream-to-reset iteration;
            // a `fn` or `yield` chunk's are for the whole chunk (per call / per
            // resume). Calling the wrong one returns Err, so pick by block type.
            let (basis, wcet, wcmu) = match chunk.block_type {
                BlockType::Stream => (
                    "iteration",
                    wcet_stream_iteration(chunk).ok(),
                    wcmu_stream_iteration(chunk).ok(),
                ),
                BlockType::Func => (
                    "call",
                    wcet_whole_chunk(chunk).ok(),
                    wcmu_whole_chunk(chunk).ok(),
                ),
                BlockType::Reentrant => (
                    "resume",
                    wcet_whole_chunk(chunk).ok(),
                    wcmu_whole_chunk(chunk).ok(),
                ),
            };
            let (stack, heap) = match wcmu {
                Some((s, h)) => (Some(s), Some(h)),
                None => (None, None),
            };
            Bound {
                chunk: chunk.name.clone(),
                basis,
                wcet_cycles: wcet,
                wcmu_stack_bytes: stack,
                wcmu_heap_bytes: heap,
            }
        })
        .collect();
    CheckResult {
        ok: true,
        diagnostics: Vec::new(),
        bounds,
    }
}

fn fail(diagnostic: Diagnostic) -> CheckResult {
    CheckResult {
        ok: false,
        diagnostics: vec![diagnostic],
        bounds: Vec::new(),
    }
}

/// Compile and verify `src`, returning a JSON string:
/// `{ ok, diagnostics: [{line, column, message}], bounds: [{chunk, wcet_cycles,
/// wcmu_stack_bytes, wcmu_heap_bytes}] }`.
#[wasm_bindgen]
pub fn check(src: &str) -> String {
    serde_json::to_string(&analyze(src)).unwrap_or_else(|_| {
        r#"{"ok":false,"diagnostics":[{"line":1,"column":1,"message":"internal serialization error"}],"bounds":[]}"#.to_string()
    })
}

/// The authoritative keyword list, for the playground's syntax highlighter.
/// Sourced from `keleusma::token::KEYWORDS` so it cannot drift from the lexer.
#[wasm_bindgen]
pub fn keywords() -> Vec<String> {
    keleusma::token::KEYWORDS
        .iter()
        .map(|s| (*s).to_string())
        .collect()
}

/// One step of a run: the coroutine state and, for a scalar `Word`, its value.
#[derive(Serialize)]
struct StepResult {
    /// `"yielded"`, `"reset"`, `"finished"`, or `"error"`.
    state: &'static str,
    /// The `Word` value yielded or returned, when it is a scalar integer.
    value: Option<i64>,
    /// A human-readable detail: an error, or the debug form of a non-scalar value.
    detail: Option<String>,
    /// How many inputs have been fed (the initial call plus each resume).
    step: usize,
}

fn err_step(detail: String) -> StepResult {
    StepResult {
        state: "error",
        value: None,
        detail: Some(detail),
        step: 0,
    }
}

/// A stateful run of a program, driven one step at a time from the page.
///
/// Execution is deterministic replay: the session holds the source and the input
/// history and, on each step, replays the whole history through a fresh VM. The
/// VM and its arena are local to the call, so no VM borrow escapes. A program
/// that reads private data before writing it currently fails at runtime (private
/// data is not yet default-initialized); pure programs and stateless loops run.
#[wasm_bindgen]
pub struct Session {
    src: String,
    inputs: Vec<i64>,
}

#[wasm_bindgen]
impl Session {
    /// Create a run for `src`. Nothing executes until the first step.
    #[wasm_bindgen(constructor)]
    pub fn new(src: String) -> Session {
        Session {
            src,
            inputs: Vec::new(),
        }
    }

    /// Discard the run so the next step starts a fresh call from the top.
    pub fn reset(&mut self) {
        self.inputs.clear();
    }

    /// Feed the next input (the `main` parameter / resume value) and advance to
    /// the next coroutine stop. Returns a [`StepResult`] as JSON.
    pub fn step(&mut self, input: i64) -> String {
        self.inputs.push(input);
        serde_json::to_string(&self.replay()).unwrap_or_else(|_| {
            r#"{"state":"error","value":null,"detail":"serialization error","step":0}"#.to_string()
        })
    }

    fn replay(&self) -> StepResult {
        let tokens = match tokenize(&self.src) {
            Ok(t) => t,
            Err(e) => return err_step(e.message),
        };
        let program = match parse(&tokens) {
            Ok(p) => p,
            Err(e) => return err_step(e.message),
        };
        // The entry `main` may take zero parameters (e.g. `fn main()`) or one
        // (e.g. `loop main(value: Word)`); the initial call must match its arity,
        // or the VM rejects the argument count.
        let main_arity = program
            .functions
            .iter()
            .find(|f| f.name == "main")
            .map_or(0, |f| f.params.len());
        let module = match compile(&program) {
            Ok(m) => m,
            Err(e) => return err_step(e.message),
        };
        // A local arena sized for the module's private (persistent) data, dropped
        // together with the VM at the end of this call so no VM borrow escapes.
        let need = required_persistent_capacity_for(&module);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
        if let Err(e) = arena.resize_persistent(need) {
            return err_step(format!("{e:?}"));
        }
        let mut vm = match Vm::new(module, &arena) {
            Ok(vm) => vm,
            Err(e) => return err_step(format!("{e:?}")),
        };
        let mut last = None;
        for (i, &input) in self.inputs.iter().enumerate() {
            let result = if i == 0 {
                if main_arity == 0 {
                    vm.call(&[])
                } else {
                    vm.call(&[Value::Int(input)])
                }
            } else {
                vm.resume(Value::Int(input))
            };
            match result {
                Ok(state) => last = Some(state),
                Err(e) => return err_step(format!("{e:?}")),
            }
        }
        step_result(last, self.inputs.len())
    }
}

fn step_result(state: Option<GenericVmState<i64, f64>>, step: usize) -> StepResult {
    match state {
        Some(GenericVmState::Yielded(v)) => scalar_step("yielded", v, step),
        Some(GenericVmState::Finished(v)) => scalar_step("finished", v, step),
        Some(GenericVmState::Reset) => StepResult {
            state: "reset",
            value: None,
            detail: None,
            step,
        },
        Some(GenericVmState::BreakpointHit { .. }) => StepResult {
            state: "reset",
            value: None,
            detail: Some("breakpoint".into()),
            step,
        },
        None => err_step("no input".into()),
    }
}

fn scalar_step(state: &'static str, value: Value, step: usize) -> StepResult {
    match value {
        Value::Int(n) => StepResult {
            state,
            value: Some(n),
            detail: None,
            step,
        },
        other => StepResult {
            state,
            value: None,
            detail: Some(format!("{other:?}")),
            step,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_program_reports_ok_with_bounds() {
        // `fn main` is a Func chunk; its WCET comes from wcet_whole_chunk (the
        // stream-only path returns Err for a fn), so it must be reported.
        let json = check("fn main() -> Word { 1 }\n");
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["ok"], true, "{json}");
        let bounds = v["bounds"].as_array().unwrap();
        assert!(!bounds.is_empty());
        assert_eq!(bounds[0]["basis"], "call");
        assert!(
            bounds[0]["wcet_cycles"].is_number(),
            "fn chunk must report a WCET: {json}"
        );
    }

    #[test]
    fn helper_fn_and_loop_each_report_a_wcet() {
        // The reported bug: a helper `fn` alongside a `loop` showed no WCET
        // because only the stream chunk was measured. Now every chunk does.
        let src = "private data state { total: Word }\n\
                   fn add(a: Word, b: Word) -> Word { a + b }\n\
                   loop main(value: Word) -> Word {\n\
                     state.total = add(value, state.total);\n\
                     yield state.total\n\
                   }\n";
        let json = check(src);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["ok"], true, "{json}");
        for b in v["bounds"].as_array().unwrap() {
            assert!(
                b["wcet_cycles"].is_number(),
                "chunk {} has no WCET: {json}",
                b["chunk"]
            );
        }
    }

    #[test]
    fn broken_program_reports_a_diagnostic() {
        let json = check("fn (");
        assert!(json.contains("\"ok\":false"), "{json}");
        assert!(json.contains("\"message\""));
    }

    #[test]
    fn keywords_match_the_core_list() {
        let k = keywords();
        assert_eq!(k.len(), keleusma::token::KEYWORDS.len());
        assert!(k.contains(&"loop".to_string()) && k.contains(&"yield".to_string()));
    }

    #[test]
    fn session_runs_a_stateless_loop() {
        let mut s =
            Session::new("loop main(value: Word) -> Word {\n  yield value\n}\n".to_string());
        // call(5) -> yields 5
        let a: serde_json::Value = serde_json::from_str(&s.step(5)).unwrap();
        assert_eq!(a["state"], "yielded");
        assert_eq!(a["value"], 5);
        // resume -> Reset (loop body end), then resume with 9 -> yields 9
        let b: serde_json::Value = serde_json::from_str(&s.step(7)).unwrap();
        assert_eq!(b["state"], "reset");
        let c: serde_json::Value = serde_json::from_str(&s.step(9)).unwrap();
        assert_eq!(c["state"], "yielded");
        assert_eq!(c["value"], 9);
    }

    #[test]
    fn session_runs_a_zero_arg_fn() {
        // `fn main()` takes no parameters; the call must not pass the input.
        let mut s = Session::new("fn main() -> Word { 40 + 2 }\n".to_string());
        let v: serde_json::Value = serde_json::from_str(&s.step(0)).unwrap();
        assert_eq!(v["state"], "finished");
        assert_eq!(v["value"], 42);
    }
}
