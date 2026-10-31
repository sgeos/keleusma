//! The bundled text operations over `Text<N>`, compiler intrinsics behind the
//! `text` cargo feature.
//!
//! # What is pinned
//!
//! Each operation is run end to end under strict verification and its result
//! compared with a model computed in Rust from the specification in
//! `docs/decisions/TEXT_CAPACITY_TYPE.md`. The model never reads compiler
//! output, so agreement is evidence about the lowering and not a restatement
//! of it. The inputs are an exhaustive product over a fixed list of strings
//! and capacities rather than a sample, so a failure reproduces exactly.
//!
//! The cost claims are pinned too. Every module must pass `verify()`, the
//! worst-case execution time must be finite and must grow with the capacity,
//! and no operation may emit `Op::Len` or a native call.
//!
//! # What is not pinned here
//!
//! The refusal of the operations in a build without the feature lives in
//! `tests/text_ops_without_feature.rs`, which compiles only in that build.

#![cfg(all(feature = "compile", feature = "verify", feature = "text"))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, TupleBody, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

/// Byte width of this build's word, derived from the crate rather than
/// restated, so a narrow-word selector does not make the expectations wrong.
fn word_bytes() -> usize {
    keleusma::target::Target::host().word_bits() as usize / 8
}

/// The flat body of a `Text<cap>` holding `bytes`, built from the
/// specification and not from compiler output.
fn body(bytes: &[u8], cap: usize) -> Vec<u8> {
    assert!(bytes.len() <= cap, "a model text must fit its capacity");
    let wb = word_bytes();
    let mut out = (bytes.len() as u64).to_le_bytes()[..wb].to_vec();
    out.extend_from_slice(bytes);
    out.resize(wb + cap, 0);
    out
}

/// A Keleusma string literal for `s`, escaping what the lexer requires.
fn lit(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn compile_src(src: &str) -> Module {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    compile(&program).unwrap_or_else(|e| panic!("compile failed for {src}: {}", e.message))
}

/// Compile end to end and return the message. A lex or parse failure panics,
/// so that a fixture with a syntax error cannot pass as a refusal.
fn compile_err(src: &str) -> String {
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    match compile(&program) {
        Ok(_) => panic!("expected a refusal, but this compiled: {src}"),
        Err(e) => e.message,
    }
}

/// Run `main` under strict verification and return its finished value, with a
/// flat tuple result copied out while the arena is alive.
enum Outcome {
    Body(Vec<u8>),
    Word(i64),
    Bool(bool),
}

fn run(src: &str) -> Outcome {
    let module = compile_src(src);
    keleusma::verify::verify(&module).unwrap_or_else(|e| panic!("verify() refused {src}: {e:?}"));
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("strict construction must verify");
    match vm
        .call(&[])
        .unwrap_or_else(|e| panic!("main must run for {src}: {e:?}"))
    {
        VmState::Finished(Value::Tuple(TupleBody::Flat(fc))) => {
            Outcome::Body(fc.resolve(&arena).expect("live body").to_vec())
        }
        VmState::Finished(Value::Int(n)) => Outcome::Word(n),
        VmState::Finished(Value::Bool(b)) => Outcome::Bool(b),
        other => panic!("unexpected result for {src}: {other:?}"),
    }
}

fn run_body(src: &str) -> Vec<u8> {
    match run(src) {
        Outcome::Body(b) => b,
        _ => panic!("expected a flat text body for {src}"),
    }
}

fn run_word(src: &str) -> i64 {
    match run(src) {
        Outcome::Word(n) => n,
        _ => panic!("expected a Word for {src}"),
    }
}

fn run_bool(src: &str) -> bool {
    match run(src) {
        Outcome::Bool(b) => b,
        _ => panic!("expected a Bool for {src}"),
    }
}

/// Inputs for the exhaustive products. The list covers the empty text, one
/// byte, an interior NUL, a two-byte UTF-8 character, and a text that fills a
/// small capacity exactly.
const SAMPLES: &[&str] = &["", "a", "ab", "a\0b", "\u{e9}", "abcd"];

/// The smallest capacity a sample fits, and one with slack beyond it.
fn caps_for(s: &str) -> [usize; 2] {
    let n = s.len().max(1);
    [n, n + 3]
}

// --- Concatenation ---

#[test]
fn concat_matches_the_model_over_every_pair_and_capacity() {
    let mut reached = 0usize;
    for a in SAMPLES {
        for b in SAMPLES {
            for ca in caps_for(a) {
                for cb in caps_for(b) {
                    let src = format!(
                        "fn main() -> Text<{}> {{ text::concat({} as Text<{ca}>, {} as Text<{cb}>) }}",
                        ca + cb,
                        lit(a),
                        lit(b)
                    );
                    let joined = [a.as_bytes(), b.as_bytes()].concat();
                    assert_eq!(run_body(&src), body(&joined, ca + cb), "{src}");
                    reached += 1;
                }
            }
        }
    }
    // Counted, so that an emptied sample list cannot pass as a clean run.
    assert_eq!(reached, SAMPLES.len() * SAMPLES.len() * 4);
}

#[test]
fn concat_nests_and_its_capacity_is_the_sum() {
    let src = "fn main() -> Text<9> { \
               let a = \"ab\" as Text<2>; \
               let b = \"c\" as Text<3>; \
               text::concat(text::concat(a, b), \"de\" as Text<4>) }";
    assert_eq!(run_body(src), body(b"abcde", 9));
    // An unannotated binding of a result keeps its capacity for later use.
    let src = "fn main() -> Word { \
               let r = text::concat(\"ab\" as Text<2>, \"c\" as Text<3>); \
               let s = text::slice(r, 1, 9); \
               text::len(s) + text::capacity(s) }";
    assert_eq!(run_word(src), 2 + 5);
    let msg =
        compile_err("fn main() -> Text<4> { text::concat(\"ab\" as Text<2>, \"cd\" as Text<3>) }");
    assert!(msg.contains("Text<4>") && msg.contains("Text<5>"), "{msg}");
}

#[test]
fn a_const_generic_capacity_is_resolved_by_monomorphization() {
    let src = "fn twice<const n: Word>(t: Text<n>) -> Text<n + n> { text::concat(t, t) }\n\
               fn main() -> Text<6> { twice::<3>(\"xy\" as Text<3>) }";
    assert_eq!(run_body(src), body(b"xyxy", 6));
}

// --- Length and capacity ---

#[test]
fn len_is_the_live_byte_count_and_capacity_is_the_type() {
    for s in SAMPLES {
        for cap in caps_for(s) {
            let t = format!("{} as Text<{cap}>", lit(s));
            assert_eq!(
                run_word(&format!("fn main() -> Word {{ text::len({t}) }}")),
                s.len() as i64,
                "{s:?}"
            );
            assert_eq!(
                run_word(&format!("fn main() -> Word {{ text::capacity({t}) }}")),
                cap as i64,
                "{s:?}"
            );
        }
    }
    // Large capacities do not fit an immediate and go through the pool.
    assert_eq!(
        run_word("fn main() -> Word { text::capacity(\"a\" as Text<40>) }"),
        40
    );
}

// --- Equality ---

#[test]
fn eq_matches_the_model_over_every_pair_and_capacity() {
    let mut reached = 0usize;
    let mut equal = 0usize;
    for a in SAMPLES {
        for b in SAMPLES {
            for ca in caps_for(a) {
                for cb in caps_for(b) {
                    let src = format!(
                        "fn main() -> bool {{ text::eq({} as Text<{ca}>, {} as Text<{cb}>) }}",
                        lit(a),
                        lit(b)
                    );
                    let want = a == b;
                    assert_eq!(run_bool(&src), want, "{src}");
                    reached += 1;
                    equal += usize::from(want);
                }
            }
        }
    }
    assert_eq!(reached, SAMPLES.len() * SAMPLES.len() * 4);
    // Both outcomes were reached, so a lowering that always answers one value
    // cannot pass.
    assert_eq!(equal, SAMPLES.len() * 4);
}

#[test]
fn eq_distinguishes_a_prefix_and_a_trailing_nul() {
    for (a, b) in [("ab", "abc"), ("a", "a\0"), ("\0", "")] {
        let src = format!(
            "fn main() -> bool {{ text::eq({} as Text<4>, {} as Text<4>) }}",
            lit(a),
            lit(b)
        );
        assert!(!run_bool(&src), "{a:?} and {b:?} must differ");
    }
}

// --- Slicing ---

#[test]
fn slice_matches_the_clamped_model() {
    let model = |s: &[u8], start: i64, end: i64| -> Vec<u8> {
        let len = s.len() as i64;
        let st = start.clamp(0, len);
        let en = end.clamp(st, len);
        s[st as usize..en as usize].to_vec()
    };
    let bounds: &[i64] = &[i64::MIN + 1, -1, 0, 1, 2, 3, 4, 5, 100, i64::MAX];
    let mut reached = 0usize;
    for s in ["", "abcd", "a\0b"] {
        let cap = s.len() + 2;
        for &start in bounds {
            for &end in bounds {
                let src = format!(
                    "fn main() -> Text<{cap}> {{ text::slice({} as Text<{cap}>, {start}, {end}) }}",
                    lit(s)
                );
                assert_eq!(
                    run_body(&src),
                    body(&model(s.as_bytes(), start, end), cap),
                    "{src}"
                );
                reached += 1;
            }
        }
    }
    assert_eq!(reached, 3 * bounds.len() * bounds.len());
}

// --- Composition with the rest of the language ---

#[test]
fn a_result_crosses_a_yield_and_lives_in_a_struct() {
    let src = "struct S { n: Word, t: Text<5> }\n\
               fn main() -> Text<5> { \
               let s = S { n: 1, t: text::concat(\"ab\" as Text<2>, \"c\" as Text<3>) }; \
               s.t }";
    assert_eq!(run_body(src), body(b"abc", 5));
    let module = compile_src(
        "loop main(e: Word) -> Text<4> { \
         let n = yield text::concat(\"h\" as Text<2>, \"i\" as Text<2>); n }",
    );
    keleusma::verify::verify(&module).expect("verify");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).expect("strict construction must verify");
    match vm.call(&[Value::Int(0)]).expect("run") {
        VmState::Yielded(Value::Tuple(TupleBody::Flat(fc))) => {
            assert_eq!(fc.resolve(&arena).expect("live"), body(b"hi", 4));
        }
        other => panic!("expected a yielded flat tuple, got {other:?}"),
    }
}

#[test]
fn labels_flow_through_an_operation() {
    // A labelled operand labels the result, so the labelled length cannot be
    // returned where an unlabelled Word is declared.
    let msg =
        compile_err("fn f(t: Text<4>@Secret) -> Word { text::len(t) }\nfn main() -> Word { 1 }");
    assert!(msg.contains("Secret"), "{msg}");
    compile_src("fn f(t: Text<4>@Secret) -> Word@Secret { text::len(t) }\nfn main() -> Word { 1 }");
}

// --- Cost ---

/// Instruction count of `main` in a module.
fn main_ops(src: &str) -> Vec<Op> {
    let module = compile_src(src);
    let main = module
        .chunks
        .iter()
        .find(|c| c.name == "main")
        .expect("main chunk");
    main.ops.clone()
}

#[test]
fn no_operation_emits_len_or_a_native_call() {
    for src in [
        "fn main() -> Word { text::len(\"ab\" as Text<4>) }",
        "fn main() -> Word { text::capacity(\"ab\" as Text<4>) }",
        "fn main() -> Text<8> { text::concat(\"ab\" as Text<4>, \"c\" as Text<4>) }",
        "fn main() -> bool { text::eq(\"ab\" as Text<4>, \"c\" as Text<4>) }",
        "fn main() -> Text<4> { text::slice(\"ab\" as Text<4>, 0, 1) }",
    ] {
        let ops = main_ops(src);
        assert!(!ops.is_empty(), "{src}");
        for op in &ops {
            assert!(
                !matches!(
                    op,
                    Op::Len
                        | Op::CallVerifiedNative(..)
                        | Op::CallExternalNative(..)
                        | Op::Call(..)
                ),
                "{src} emitted {op:?}"
            );
        }
        let module = compile_src(src);
        assert!(module.native_names.is_empty(), "{src} registered a native");
    }
}

#[test]
fn the_static_cost_is_finite_and_linear_in_the_capacity() {
    let wcet = |src: &str| -> u32 {
        let module = compile_src(src);
        keleusma::verify::verify(&module).expect("verify");
        let main = module
            .chunks
            .iter()
            .find(|c| c.name == "main")
            .expect("main chunk");
        keleusma::verify::wcet_whole_chunk(main).expect("a static WCET")
    };
    for op in ["concat", "eq"] {
        let ret = if op == "eq" {
            "bool".to_string()
        } else {
            String::new()
        };
        let at = |n: usize| {
            let ret = if ret.is_empty() {
                format!("Text<{}>", 2 * n)
            } else {
                ret.clone()
            };
            wcet(&format!(
                "fn main() -> {ret} {{ text::{op}(\"a\" as Text<{n}>, \"b\" as Text<{n}>) }}"
            ))
        };
        let (w4, w8, w16) = (at(4), at(8), at(16));
        assert!(w4 < w8 && w8 < w16, "{op}: {w4} {w8} {w16}");
        // Linear: doubling the capacity adds the same amount each time.
        assert_eq!(w16 - w8, 2 * (w8 - w4), "{op}: {w4} {w8} {w16}");
    }
}

// --- The self-hosted compiler ---

/// The stages under `kel/` do not mirror the operations. A program using one
/// necessarily names a `Text<N>`, because the literal cast is the only way to
/// make an operand, so the shipping entry refuses it as unsupported before any
/// stage runs. This lives here rather than in a `selfhost_*` binary because the
/// workspace job, the only one that enables `text` together with `self-host`,
/// does not run those binaries.
#[cfg(feature = "self-host")]
#[test]
fn a_bundled_text_operation_is_outside_the_self_hosted_subset() {
    let src = "fn main() -> Word { text::len(text::concat(\"ab\" as Text<2>, \"c\" as Text<2>)) }";
    match keleusma::selfhost::self_hosted_compile(src, &keleusma::target::Target::host()) {
        Err(keleusma::selfhost::SelfHostError::Unsupported { detail }) => {
            assert!(detail.contains("Text<N>"), "{detail}");
        }
        other => panic!("expected an unsupported refusal, got {other:?}"),
    }
}

// --- Refusals ---

#[test]
fn an_operand_that_is_not_a_text_n_is_refused() {
    for (src, needle) in [
        (
            "fn main() -> Word { text::len(\"ab\") }",
            "expects a capacity-bounded text",
        ),
        (
            "fn main() -> Word { text::len(3) }",
            "expects a capacity-bounded text",
        ),
        (
            "fn main() -> Text<4> { text::slice(\"ab\" as Text<4>, true, 1) }",
            "expects Word",
        ),
        (
            "fn main() -> bool { text::eq(\"ab\" as Text<4>, \"ab\") }",
            "argument 1 expects a capacity-bounded text",
        ),
    ] {
        let msg = compile_err(src);
        assert!(msg.contains(needle), "{src}: {msg}");
    }
}

#[test]
fn an_arity_mismatch_or_an_unknown_operation_is_refused() {
    for (src, needle) in [
        (
            "fn main() -> Word { text::len(\"a\" as Text<2>, \"b\" as Text<2>) }",
            "expects 1 argument",
        ),
        (
            "fn main() -> Word { text::length(\"a\" as Text<2>) }",
            "not a bundled text operation",
        ),
        (
            "fn main() -> Word { text::len::<2>(\"a\" as Text<2>) }",
            "takes no const arguments",
        ),
    ] {
        let msg = compile_err(src);
        assert!(msg.contains(needle), "{src}: {msg}");
    }
}

#[test]
fn the_namespace_cannot_be_imported_piped_or_handled() {
    for (src, needle) in [
        (
            "use text::concat\nfn main() -> Word { 1 }",
            "the `text` path names the bundled text operations",
        ),
        (
            "use text::*\nfn main() -> Word { 1 }",
            "the `text` path names the bundled text operations",
        ),
        (
            "fn main() -> Word { (\"a\" as Text<2>) |> text::len() }",
            "is not a pipeline target",
        ),
        (
            "fn main() -> Word { text::len(\"a\" as Text<2>) { ok(v) => v, host_error(c) => c } }",
            "has no outcome arms",
        ),
    ] {
        let msg = compile_err(src);
        assert!(msg.contains(needle), "{src}: {msg}");
    }
}

#[test]
fn the_comparison_operators_stay_refused_on_text_n() {
    // The ruling makes `text::eq` the comparison surface. The operators stay
    // refused, because the general composite rule would admit them and they
    // faulted at runtime when measured.
    for src in [
        "fn main() -> bool { let t = \"a\" as Text<2>; t == t }",
        "fn main() -> bool { let t = \"a\" as Text<2>; t < t }",
    ] {
        compile_err(src);
    }
}

#[test]
fn a_result_capacity_past_the_flat_body_limit_is_refused() {
    // A flat body is at most `u16::MAX` bytes, the length word included.
    let half = (usize::from(u16::MAX) - word_bytes()) / 2 + 1;
    let msg = compile_err(&format!(
        "fn main() -> Text<{}> {{ text::concat(\"\" as Text<{half}>, \"\" as Text<{half}>) }}",
        2 * half
    ));
    assert!(msg.contains("larger than the largest capacity"), "{msg}");
}
