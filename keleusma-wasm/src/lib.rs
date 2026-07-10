//! WebAssembly bindings for the Keleusma compiler.
//!
//! Exposes a single `check` entry point that the browser playground calls: it
//! runs the full static pipeline (`tokenize` → `parse` → `compile` → `verify`)
//! and, on success, reports the per-chunk worst-case-execution-time and
//! worst-case-memory-usage bounds. Surfacing definitive resource bounds in a
//! playground is the feature no other language's playground offers.
//!
//! This is static analysis only; it does not execute the program (running would
//! require host-native registration and output capture, a later addition).

use keleusma::bytecode::BlockType;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::Span;
use keleusma::verify::{
    verify, wcet_stream_iteration, wcet_whole_chunk, wcmu_stream_iteration, wcmu_whole_chunk,
};
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
}
