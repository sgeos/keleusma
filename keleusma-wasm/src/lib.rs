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

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::Span;
use keleusma::verify::{verify, wcet_stream_iteration, wcmu_stream_iteration};
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
    /// Worst-case execution time, in pipelined cycles per stream-to-reset slice,
    /// under the nominal cost model. `null` if not computable for this chunk.
    wcet_cycles: Option<u32>,
    /// Worst-case operand-stack bytes per slice. `null` if not computable.
    wcmu_stack_bytes: Option<u32>,
    /// Worst-case heap (arena) bytes per slice. `null` if not computable.
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
            let (stack, heap) = match wcmu_stream_iteration(chunk) {
                Ok((s, h)) => (Some(s), Some(h)),
                Err(_) => (None, None),
            };
            Bound {
                chunk: chunk.name.clone(),
                wcet_cycles: wcet_stream_iteration(chunk).ok(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_program_reports_ok_with_bounds() {
        let json = check("fn main() -> Word { 1 }\n");
        assert!(json.contains("\"ok\":true"), "{json}");
        assert!(json.contains("\"bounds\""));
    }

    #[test]
    fn broken_program_reports_a_diagnostic() {
        let json = check("fn (");
        assert!(json.contains("\"ok\":false"), "{json}");
        assert!(json.contains("\"message\""));
    }
}
