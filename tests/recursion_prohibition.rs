#![cfg(all(feature = "compile", feature = "verify"))]
//! The compile pipeline and the verifier refuse a call cycle.
//!
//! `docs/spec/GRAMMAR.md` under "Recursion Prohibition" and
//! `docs/architecture/LANGUAGE_DESIGN.md` state that the compiler rejects any cycle in the call
//! graph with a diagnostic naming the cycle. Until 2026-11-11 neither the type checker nor the
//! compiler nor `verify` did. A recursive program compiled, verified and received a finite
//! worst-case execution time header, and only `Vm::new` refused it, through the topological sort
//! of the memory analysis. The proofs line found this with a differential test, recorded in
//! `tools/proof-difftest/README.md` as finding 1.
//!
//! Three layers now refuse a cycle, and each is pinned here.
//!
//! 1. The type checker, for every call it resolves. That is a plain call, a pipe, a trait method
//!    on a resolved receiver and the implicit call of a refinement predicate. The diagnostic
//!    names the functions of the cycle and carries the span of the call that closes it.
//! 2. `verify`, on the bytecode call graph, which also covers bytecode from another producer and
//!    any call the type checker did not resolve.
//! 3. `Vm::new`, as before.
//!
//! **Coverage limits.** The cases are single programs, not a generated corpus. A cycle that the
//! type checker cannot see and that reaches `verify` only through the compiler is exercised by no
//! case here, because no such source shape was found. The verifier layer is exercised on
//! hand-built modules instead.

use keleusma::bytecode::{Module, Op};
use keleusma::compiler::{CompileError, compile};
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn try_compile(src: &str) -> Result<Module, CompileError> {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
}

/// The refusal of `src`, required to name every function in `names` and to carry a real span.
fn refused(src: &str, names: &[&str]) -> CompileError {
    let err = match try_compile(src) {
        Ok(_) => panic!("a program with a call cycle compiled: {src}"),
        Err(e) => e,
    };
    assert!(
        err.message.contains("recursive call detected"),
        "the refusal does not say that a recursive call was detected: {}",
        err.message
    );
    for name in names {
        assert!(
            err.message.contains(&format!("`{name}`")),
            "the refusal does not name `{name}`: {}",
            err.message
        );
    }
    assert!(
        err.span.line >= 1 && err.span.end > err.span.start,
        "the refusal carries no source span: {:?}",
        err.span
    );
    err
}

#[test]
fn direct_recursion_is_refused_at_compile_time() {
    // The proofs line's minimal reproduction of finding 1.
    let src = "fn f0(a: Word, b: Word) -> Word { f0(a, b) }\n\
               loop main(x: Word) -> Word { yield 1; f0(x, x) }";
    let err = refused(src, &["f0"]);
    // The span is the recursive call, on the first line, not the declaration of `main`.
    assert_eq!(err.span.line, 1, "{:?}", err.span);
    assert!(
        err.message.contains("type error"),
        "the type checker is the stage expected to refuse it: {}",
        err.message
    );
}

#[test]
fn the_type_checker_alone_refuses_direct_recursion() {
    let src = "fn f(a: Word) -> Word { f(a) }\nfn main() -> Word { f(1) }";
    let mut program = parse(&tokenize(src).expect("lex")).expect("parse");
    let err = keleusma::typecheck::check(&mut program).expect_err("a cycle must not type check");
    assert!(
        err.message.contains("recursive call detected") && err.message.contains("`f`"),
        "{}",
        err.message
    );
    assert_eq!(err.span.line, 1);
}

#[test]
fn mutual_recursion_is_refused_and_names_both_functions() {
    let src = "fn a(x: Word) -> Word { b(x) }\n\
               fn b(x: Word) -> Word { a(x) }\n\
               fn main() -> Word { a(1) }";
    let err = refused(src, &["a", "b"]);
    // The cycle is written out in call order and returns to where it started.
    assert!(
        err.message.contains("`a` -> `b` -> `a`") || err.message.contains("`b` -> `a` -> `b`"),
        "{}",
        err.message
    );
}

#[test]
fn a_cycle_of_three_is_refused_and_names_all_three() {
    let src = "fn a(x: Word) -> Word { b(x) }\n\
               fn b(x: Word) -> Word { c(x) }\n\
               fn c(x: Word) -> Word { if x > 0 { a(x - 1) } else { 0 } }\n\
               fn main() -> Word { a(3) }";
    refused(src, &["a", "b", "c"]);
}

#[test]
fn a_terminating_recursion_is_refused_all_the_same() {
    // The guide's worked rejection. It would stop at zero, and the prohibition is syntactic.
    let src = "fn count_down(n: Word) -> Word {\n    if n <= 0 { 0 } else { count_down(n - 1) }\n}\n\n\
               fn main() -> Word {\n    count_down(5)\n}\n";
    let err = refused(src, &["count_down"]);
    assert_eq!(err.span.line, 2);
}

#[test]
fn a_cycle_through_a_trait_method_is_refused() {
    let src = "trait Cap { fn cap(self) -> Word; }\n\
               struct S { c: Word }\n\
               fn helper(s: S) -> Word { s.cap() }\n\
               impl Cap for S { fn cap(s: S) -> Word { helper(s) } }\n\
               fn main() -> Word { let x = S { c: 7 }; x.cap() }";
    // An impl method is named by its trait, its type and its own name.
    refused(src, &["helper", "Cap::S::cap"]);
}

#[test]
fn a_method_that_calls_itself_is_refused() {
    let src = "trait Cap { fn cap(self) -> Word; }\n\
               struct S { c: Word }\n\
               impl Cap for S { fn cap(s: S) -> Word { s.cap() } }\n\
               fn main() -> Word { let x = S { c: 7 }; x.cap() }";
    refused(src, &["Cap::S::cap"]);
}

#[test]
fn a_generic_function_that_calls_itself_is_refused() {
    let src = "fn again<T>(x: T) -> T { again(x) }\nfn main() -> Word { again(1) }";
    refused(src, &["again"]);
}

#[test]
fn a_cycle_visible_only_after_monomorphization_is_refused() {
    // Before monomorphization the receiver of `x.cap()` is a type variable, so the type checker
    // resolves no method and sees no edge from `through` to the impl. The specialization at `S`
    // closes the cycle, and the check after monomorphization or the verifier must refuse it.
    let src = "trait Cap { fn cap(self) -> Word; }\n\
               struct S { c: Word }\n\
               fn through<T: Cap>(x: T) -> Word { x.cap() }\n\
               impl Cap for S { fn cap(s: S) -> Word { through(s) } }\n\
               fn main() -> Word { let x = S { c: 7 }; through(x) }";
    let err = match try_compile(src) {
        Ok(_) => panic!("a cycle through a specialized generic compiled"),
        Err(e) => e,
    };
    assert!(
        err.message.contains("recursive call detected"),
        "{}",
        err.message
    );
    assert!(err.message.contains("cap"), "{}", err.message);
}

#[test]
fn a_cycle_through_a_pipe_is_refused() {
    let src = "fn f(a: Word) -> Word { a |> g() }\n\
               fn g(a: Word) -> Word { a |> f() }\n\
               fn main() -> Word { 1 |> f() }";
    refused(src, &["f", "g"]);
}

#[test]
fn a_cycle_through_a_refinement_predicate_is_refused() {
    // Constructing `P` calls `pos`, which no call expression names.
    let src = "fn pos(x: Word) -> bool { let p = P(x); x > 0 }\n\
               newtype P = Word where pos;\n\
               fn main(a: Word) -> Word { let p = P(a); p as Word }";
    let err = match try_compile(src) {
        Ok(_) => panic!("a cycle through a refinement predicate compiled"),
        Err(e) => e,
    };
    assert!(
        err.message.contains("recursive call detected") && err.message.contains("`pos`"),
        "{}",
        err.message
    );
}

#[test]
fn a_yield_function_that_calls_itself_is_refused() {
    let src = "yield g(a: Word) -> Word { yield a; g(a) }\n\
               loop main(r: Word) -> Word { let x = g(r); yield x }";
    refused(src, &["g"]);
}

#[test]
fn two_loops_that_delegate_to_each_other_are_refused() {
    let src = "loop child(a: Word) -> Word { yield a; main(a) }\n\
               loop main(t: Word) -> Word { yield t; child(t) }";
    refused(src, &["child", "main"]);
}

#[test]
fn a_diamond_is_not_a_cycle_and_still_compiles_and_loads() {
    // `top` reaches `leaf` along two paths. Nothing returns to a caller.
    let src = "fn leaf(x: Word) -> Word { x + 1 }\n\
               fn left(x: Word) -> Word { leaf(x) + 1 }\n\
               fn right(x: Word) -> Word { leaf(x) + 2 }\n\
               fn top(x: Word) -> Word { left(x) + right(x) }\n\
               loop main(x: Word) -> Word { yield top(x); x }";
    let module = try_compile(src).expect("a diamond is acyclic and must compile");
    keleusma::verify::verify(&module).expect("a diamond verifies");
    assert!(keleusma::verify::find_call_cycle(&module).is_none());
    let arena = keleusma::Arena::with_capacity(1 << 16);
    keleusma::vm::Vm::new(module, &arena).expect("a diamond loads");
}

#[test]
fn a_call_chain_and_a_repeated_callee_are_not_cycles() {
    for src in [
        "fn a(x: Word) -> Word { x }\nfn b(x: Word) -> Word { a(a(x)) }\nfn main() -> Word { b(b(1)) }",
        "fn a(x: Word) -> Word { x }\nfn main() -> Word { for i in 0..3 { let w = a(i); } a(1) }",
    ] {
        try_compile(src).unwrap_or_else(|e| panic!("{src} was refused: {}", e.message));
    }
}

// --- the verifier, on bytecode from another producer ------------------------------------

/// A module whose chunks call each other as `edges` says, built from a compiled acyclic one so
/// that everything but the call graph is well formed.
fn rewired(edges: &[(usize, u16)]) -> Module {
    let mut module = try_compile(
        "fn a() -> Word { 1 }\nfn b() -> Word { 2 }\nfn c() -> Word { 3 }\nfn main() -> Word { 0 }",
    )
    .expect("the base compiles");
    for &(caller, callee) in edges {
        module.chunks[caller].ops = vec![Op::Call(callee, 0), Op::Return];
    }
    module
}

fn chunk_index(module: &Module, name: &str) -> usize {
    module
        .chunks
        .iter()
        .position(|c| c.name == name)
        .unwrap_or_else(|| panic!("no chunk named {name}"))
}

#[test]
fn verify_refuses_a_module_whose_chunk_calls_itself() {
    let base = rewired(&[]);
    keleusma::verify::verify(&base).expect("the control verifies");
    let a = chunk_index(&base, "a");
    let module = rewired(&[(a, a as u16)]);
    let err = keleusma::verify::verify(&module).expect_err("verify must refuse a self call");
    assert!(
        err.message.contains("recursive call detected") && err.message.contains("`a` -> `a`"),
        "{}: {}",
        err.chunk_name,
        err.message
    );
    assert_eq!(keleusma::verify::find_call_cycle(&module), Some(vec![a]));
}

#[test]
fn verify_refuses_a_mutual_cycle_and_names_it_in_call_order() {
    let base = rewired(&[]);
    let (a, b, c) = (
        chunk_index(&base, "a"),
        chunk_index(&base, "b"),
        chunk_index(&base, "c"),
    );
    let module = rewired(&[(a, b as u16), (b, c as u16), (c, a as u16)]);
    let err = keleusma::verify::verify(&module).expect_err("verify must refuse a cycle of three");
    assert!(
        err.message.contains("`a` -> `b` -> `c` -> `a`"),
        "{}",
        err.message
    );
    let cycle = keleusma::verify::find_call_cycle(&module).expect("a cycle");
    assert_eq!(cycle, vec![a, b, c]);
    // The memory analysis reports the same cycle and no longer an unnamed one.
    let wcmu = keleusma::verify::module_wcmu(&module, &[]).expect_err("no memory bound");
    assert!(wcmu.message.contains("recursive call detected"));
    assert!(wcmu.message.contains("`a` -> `b` -> `c` -> `a`"));
}

#[test]
fn a_cycle_that_the_entry_never_reaches_is_refused_too() {
    // The prohibition is on the call graph, not on what the entry reaches.
    let base = rewired(&[]);
    let (b, c) = (chunk_index(&base, "b"), chunk_index(&base, "c"));
    let module = rewired(&[(b, c as u16), (c, b as u16)]);
    assert!(keleusma::verify::verify(&module).is_err());
    assert_eq!(keleusma::verify::find_call_cycle(&module), Some(vec![b, c]));
}

#[test]
fn a_long_acyclic_chain_is_walked_without_exhausting_the_stack() {
    // 3000 chunks, each calling the next. The walk is iterative, so its depth costs heap.
    let n = 3000usize;
    let mut module = rewired(&[]);
    let template = module.chunks[chunk_index(&module, "a")].clone();
    module.chunks.clear();
    for i in 0..n {
        let mut chunk = template.clone();
        chunk.name = format!("f{i}");
        if i + 1 < n {
            chunk.ops = vec![Op::Call((i + 1) as u16, 0), Op::Return];
        }
        module.chunks.push(chunk);
    }
    assert!(keleusma::verify::find_call_cycle(&module).is_none());
    assert_eq!(
        keleusma::verify::module_call_depth(&module).unwrap(),
        n as u32
    );
    // Closing the chain makes the longest possible cycle, and it is still found.
    module.chunks[n - 1].ops = vec![Op::Call(0, 0), Op::Return];
    let cycle = keleusma::verify::find_call_cycle(&module).expect("the closed chain is a cycle");
    assert_eq!(cycle.len(), n);
}

// --- the self-hosted compiler -------------------------------------------------------------

/// **THE SELF-HOSTED COMPILER DOES NOT EMIT A MODULE FOR A RECURSIVE PROGRAM.**
///
/// The pin is on the outcome of the shipping entry, `self_hosted_compile`, which is what
/// `keleusma compile --compiler self-hosted` runs. Its own type stage does not look for a call
/// cycle, so the refusal is the reference's, reported as `ReferenceRejected`. That is the
/// cross-check doing the work, and it is the weaker of the two acceptable outcomes. A change that
/// makes the stage refuse first moves this to `PipelineRejected`, and the assertion says so.
#[cfg(feature = "self-host")]
#[test]
fn the_self_hosted_compiler_reports_a_recursive_program_as_refused_by_the_reference() {
    use keleusma::selfhost::{SelfHostError, self_hosted_compile};
    let host = keleusma::target::Target::host();
    for src in [
        "fn f0(a: Word, b: Word) -> Word { f0(a, b) }\nloop main(x: Word) -> Word { yield 1; f0(x, x) }",
        "fn a(x: Word) -> Word { b(x) }\nfn b(x: Word) -> Word { a(x) }\nfn main() -> Word { a(1) }",
    ] {
        match self_hosted_compile(src, &host) {
            Err(SelfHostError::ReferenceRejected { detail }) => assert!(
                detail.contains("recursive call detected"),
                "the reference refused it for another reason: {detail}"
            ),
            Err(SelfHostError::PipelineRejected { .. }) => panic!(
                "the self-hosted stage now refuses recursion before the reference does. That is \
                 the stronger outcome, so move this pin to `PipelineRejected`: {src}"
            ),
            Err(other) => panic!("an unexpected refusal for {src}: {other:?}"),
            Ok(_) => panic!("the self-hosted compiler emitted a recursive module: {src}"),
        }
    }
    // The control. A refusal of everything would satisfy the loop above.
    self_hosted_compile(
        "fn leaf(x: Word) -> Word { x + 1 }\nfn main() -> Word { leaf(1) }",
        &host,
    )
    .expect("the acyclic control compiles under the self-hosted compiler");
}
