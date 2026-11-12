#![cfg(all(feature = "compile", feature = "verify"))]
//! The composed time bound charges each call its callee.
//!
//! `wcet_stream_iteration`, `wcet_whole_chunk` and `module_wcet_with_bounds` are shallow. An
//! `Op::Call` contributes its dispatch cycles and not the callee's body, so their figures bound
//! no program that calls a function. The proofs line measured the size of the gap and recorded
//! it in `tools/proof-difftest/README.md` as finding 2. Its example has a declared header of 20
//! and a unit-cost figure of 11 where 872 operations execute in one iteration.
//!
//! `verify::module_wcet_composed_with_bounds` and `Vm::wcet_per_iteration_composed` are the
//! addition these tests pin. The shallow functions and the module header are unchanged, which
//! `the_shallow_figures_and_the_header_are_unchanged` pins.
//!
//! # What is measured, and how
//!
//! **The instrument is the proofs line's own**, from `tools/proof-difftest/src/rust_side.rs`,
//! reached through the public API. Every operation of every chunk is armed as a breakpoint, so
//! the virtual machine stops before each operation it dispatches and the harness counts it. An
//! operation is counted once whatever it does, and the implicit return at the end of a chunk
//! dispatches no operation and is not counted. The bound is taken under a cost model that charges
//! one cycle for every operation and nothing for a text byte, so the figure and the count are in
//! the same unit.
//!
//! One iteration is the span from the host's call, or from the resume that follows a `Reset`, to
//! the next `Reset`. It may contain several yields.
//!
//! # Coverage limits
//!
//! - The corpus is nine hand-written programs, not a generated one. Agreement on it shows the
//!   relation where it was looked for and not elsewhere.
//! - A run executes one path. The bound is over the worst path, which a run may not take, so the
//!   relation tested is that the bound is at least the count, never that it is attained.
//! - Only the unit cost model is compared against execution. The nominal table is compared
//!   against a second computation of the same method, not against time.
//! - No program here calls a host native during a measured run.

use keleusma::bytecode::{BlockType, CostModel, Module, NOMINAL_COST_MODEL, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::verify::{self, NativeIterationBound};
use keleusma::vm::{Vm, VmState};

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse"))
        .unwrap_or_else(|e| panic!("compile {src}: {}", e.message))
}

fn unit_cycles(_: &Op) -> u32 {
    1
}

/// One cycle per operation and nothing per text byte.
const UNIT: CostModel = CostModel {
    value_slot_bytes: NOMINAL_COST_MODEL.value_slot_bytes,
    op_cycles: unit_cycles,
    text_byte_cycles: 0,
};

fn chunk_index(module: &Module, name: &str) -> usize {
    module
        .chunks
        .iter()
        .position(|c| c.name == name)
        .unwrap_or_else(|| panic!("no chunk named {name}"))
}

fn composed_unit(module: &Module) -> Vec<u32> {
    verify::module_wcet_composed_with_bounds(module, &[], &UNIT)
        .unwrap_or_else(|e| panic!("composed bound: {}: {}", e.chunk_name, e.message))
}

fn shallow_unit(module: &Module) -> Vec<u32> {
    verify::module_wcet_with_bounds(module, &[], &UNIT).expect("shallow bound")
}

/// The operations dispatched in each of the first `iterations` iterations, in every chunk, and
/// the chunk that was resetting at the end of each.
fn measure(module: &Module, iterations: usize) -> Vec<(u64, usize)> {
    const STEP_CAP: u64 = 5_000_000;
    let arena = keleusma::Arena::with_capacity(1 << 20);
    let mut vm = Vm::new(module.clone(), &arena).expect("load");
    for (ci, c) in module.chunks.iter().enumerate() {
        for oi in 0..c.ops.len() {
            vm.set_breakpoint(ci, oi);
        }
    }
    let mut spans: Vec<(u64, usize)> = Vec::new();
    let (mut count, mut total, mut last_chunk) = (0u64, 0u64, usize::MAX);
    let mut state = vm.call(&[Value::Int(3)]).expect("call");
    loop {
        state = match state {
            VmState::BreakpointHit { chunk, .. } => {
                count += 1;
                total += 1;
                assert!(total <= STEP_CAP, "runaway, the step cap was reached");
                last_chunk = chunk;
                vm.resume_from_breakpoint().expect("step")
            }
            VmState::Yielded(_) => vm.resume(Value::Int(3)).expect("resume"),
            VmState::Reset => {
                spans.push((count, last_chunk));
                count = 0;
                if spans.len() == iterations {
                    return spans;
                }
                vm.resume(Value::Int(3)).expect("resume after reset")
            }
            other => panic!("unexpected state {other:?}"),
        };
    }
}

/// The proofs line's example for finding 2.
const PROOFS_EXAMPLE: &str = "fn f0(a: Word, b: Word) -> Word { for i in 0..50 { let w = a + 1; }; a }\n\
                              loop main(x: Word) -> Word { yield 1; f0(x, x); x }";

/// The corpus, as a name and a source. Every entry is a `loop main` taking one `Word`.
fn corpus() -> Vec<(&'static str, &'static str)> {
    vec![
        ("proofs_example", PROOFS_EXAMPLE),
        (
            "nested_calls",
            "fn c(x: Word) -> Word { for i in 0..4 { let w = x + 1; }; x }\n\
             fn b(x: Word) -> Word { c(x) + c(x) }\n\
             fn a(x: Word) -> Word { b(x) + 1 }\n\
             loop main(x: Word) -> Word { yield a(x); x }",
        ),
        (
            "call_inside_a_loop",
            "fn g(x: Word) -> Word { let y = x + 1; y + 1 }\n\
             loop main(x: Word) -> Word { for i in 0..7 { let w = g(i); }; yield x; x }",
        ),
        (
            "call_inside_nested_loops",
            "fn g(x: Word) -> Word { for i in 0..3 { let w = x + 1; }; x }\n\
             loop main(x: Word) -> Word { \
             for i in 0..5 { for j in 0..2 { let w = g(j); }; }; yield x; x }",
        ),
        (
            "diamond",
            "fn leaf(x: Word) -> Word { for i in 0..6 { let w = x + 1; }; x + 1 }\n\
             fn left(x: Word) -> Word { leaf(x) + 1 }\n\
             fn right(x: Word) -> Word { leaf(x) + 2 }\n\
             fn top(x: Word) -> Word { left(x) + right(x) }\n\
             loop main(x: Word) -> Word { yield top(x); x }",
        ),
        (
            "call_in_one_branch",
            "fn g(x: Word) -> Word { for i in 0..9 { let w = x + 1; }; x }\n\
             loop main(x: Word) -> Word { let r = if x > 100 { 1 } else { g(x) }; yield r; x }",
        ),
        (
            "yield_callee",
            "yield g(a: Word) -> Word { for i in 0..5 { let w = a + 1; }; yield a; \
             for i in 0..5 { let w = a + 1; }; yield a + 1 }\n\
             loop main(r: Word) -> Word { let x = g(r); yield x }",
        ),
        (
            "yield_callee_inside_a_loop",
            "yield g(a: Word) -> Word { yield a; a + 1 }\n\
             loop main(r: Word) -> Word { for i in 0..3 { let x = g(i); }; yield r }",
        ),
        (
            "call_free",
            "loop main(x: Word) -> Word { for i in 0..4 { let w = x + 1; }; \
             let r = if x > 2 { x } else { x + 1 }; yield r; x }",
        ),
    ]
}

#[test]
fn the_composed_figure_is_at_least_the_operations_executed() {
    let mut reached = 0usize;
    for (name, src) in corpus() {
        let module = compile_src(src);
        let main = chunk_index(&module, "main");
        let bound = composed_unit(&module)[main];
        assert_ne!(bound, u32::MAX, "{name}: the corpus is meant to be bounded");
        let spans = measure(&module, 3);
        for (i, &(count, chunk)) in spans.iter().enumerate() {
            assert_eq!(chunk, main, "{name}: iteration {i} reset in another chunk");
            assert!(
                count <= bound as u64,
                "{name}: iteration {i} executed {count} operations and the composed bound is {bound}"
            );
            reached += 1;
        }
        // The bound is not vacuous. It is within a factor of two of the first iteration on this
        // corpus, measured, apart from the program whose calling branch the run does not take.
        if name != "call_in_one_branch" {
            assert!(
                (bound as u64) < 2 * spans[0].0,
                "{name}: the bound {bound} is more than twice the {} operations executed",
                spans[0].0
            );
        }
    }
    assert_eq!(reached, 27, "every iteration of every program was compared");
}

#[test]
fn the_shallow_figure_is_below_what_executes_and_the_composed_one_is_not() {
    // Finding 2, with the proofs line's figures. This is why the addition exists.
    let module = compile_src(PROOFS_EXAMPLE);
    let main = chunk_index(&module, "main");
    assert_eq!(module.wcet_cycles, 20, "the declared header, unchanged");
    assert_eq!(
        shallow_unit(&module)[main],
        11,
        "the shallow unit-cost figure"
    );
    let spans = measure(&module, 3);
    let counts: Vec<u64> = spans.iter().map(|s| s.0).collect();
    assert_eq!(counts, vec![872, 871, 871], "the operations executed");
    let composed = composed_unit(&module)[main];
    assert!(composed >= 872, "{composed}");
    // The figure itself, so that a change to the walk is seen. `main` costs 11 at dispatch only,
    // and the call adds the whole of `f0`.
    let f0 = chunk_index(&module, "f0");
    assert_eq!(composed, 11 + composed_unit(&module)[f0]);
    assert_eq!(composed_unit(&module)[f0], shallow_unit(&module)[f0]);
}

#[test]
fn a_call_free_program_has_equal_composed_and_shallow_figures() {
    for src in [
        "loop main(x: Word) -> Word { yield x }",
        "loop main(x: Word) -> Word { for i in 0..4 { let w = x + 1; }; \
         let r = if x > 2 { x } else { x + 1 }; yield r; x }",
        "fn main(a: Word) -> Word { for i in 0..3 { let w = a + 1; }; a }",
    ] {
        let module = compile_src(src);
        assert!(
            !module
                .chunks
                .iter()
                .any(|c| c.ops.iter().any(|op| matches!(op, Op::Call(..)))),
            "{src} is meant to be call-free"
        );
        for model in [&UNIT, &NOMINAL_COST_MODEL] {
            let shallow = verify::module_wcet_with_bounds(&module, &[], model).expect("shallow");
            let composed =
                verify::module_wcet_composed_with_bounds(&module, &[], model).expect("composed");
            assert_eq!(shallow, composed, "{src}");
        }
    }
}

#[test]
fn the_composed_figure_is_never_below_the_shallow_one_for_a_function_or_a_stream() {
    for (name, src) in corpus() {
        let module = compile_src(src);
        for model in [&UNIT, &NOMINAL_COST_MODEL] {
            let shallow = verify::module_wcet_with_bounds(&module, &[], model).expect("shallow");
            let composed =
                verify::module_wcet_composed_with_bounds(&module, &[], model).expect("composed");
            for (i, chunk) in module.chunks.iter().enumerate() {
                assert!(
                    composed[i] >= shallow[i],
                    "{name}: chunk {} has composed {} below shallow {}",
                    chunk.name,
                    composed[i],
                    shallow[i]
                );
            }
        }
    }
}

#[test]
fn a_call_site_in_a_loop_is_charged_once_per_pass() {
    // The same callee called once and called in a loop of seven. Under unit costs the difference
    // between the two mains is exactly six more calls of `g` plus the loop's own instructions,
    // so the callee's figure must appear seven times in the second.
    let once = compile_src(
        "fn g(x: Word) -> Word { let y = x + 1; y + 1 }\n\
         loop main(x: Word) -> Word { let w = g(x); yield x; x }",
    );
    let seven = compile_src(
        "fn g(x: Word) -> Word { let y = x + 1; y + 1 }\n\
         loop main(x: Word) -> Word { for i in 0..7 { let w = g(i); }; yield x; x }",
    );
    let g = composed_unit(&once)[chunk_index(&once, "g")];
    assert_eq!(g, composed_unit(&seven)[chunk_index(&seven, "g")]);
    let shallow_seven = shallow_unit(&seven)[chunk_index(&seven, "main")];
    let composed_seven = composed_unit(&seven)[chunk_index(&seven, "main")];
    assert_eq!(composed_seven, shallow_seven + 7 * g);
    let shallow_once = shallow_unit(&once)[chunk_index(&once, "main")];
    assert_eq!(
        composed_unit(&once)[chunk_index(&once, "main")],
        shallow_once + g
    );
}

// --- agreement with the native route's method ------------------------------------------------

thread_local! {
    static CALLEE: std::cell::RefCell<Vec<u32>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `native_codegen/src/cost.rs` composes by giving `Op::Call` the cost of its dispatch plus the
/// callee's composed bound and then running the shallow walk. This is that method, restated.
fn dispatch_plus_callee(op: &Op) -> u32 {
    let own = (NOMINAL_COST_MODEL.op_cycles)(op);
    match op {
        Op::Call(target, _) => CALLEE.with(|c| own.saturating_add(c.borrow()[*target as usize])),
        _ => own,
    }
}

#[test]
fn the_composed_figure_agrees_with_the_native_routes_method_where_that_has_one() {
    // The native route refuses a chunk that calls a suspending chunk or a host native, so the
    // comparison is over the programs whose callees are all `fn`. It is a second computation of
    // the same definition by another mechanism, a cost model in place of per-instruction extras.
    let model = CostModel {
        op_cycles: dispatch_plus_callee,
        ..NOMINAL_COST_MODEL
    };
    let mut compared = 0usize;
    for (name, src) in corpus() {
        let module = compile_src(src);
        let suspending_callee = module.chunks.iter().any(|c| {
            c.ops.iter().any(|op| {
                matches!(op, Op::Call(t, _) if module.chunks[*t as usize].block_type != BlockType::Func)
            })
        });
        if suspending_callee {
            continue;
        }
        let returns_text = verify::module_chunk_returns_text(&module).expect("text-ness");
        // Callees first. A chunk's callees have lower figures recorded before it is reached,
        // because the loop repeats until no chunk remains whose callees are all done.
        let n = module.chunks.len();
        let mut done: Vec<Option<u32>> = vec![None; n];
        for _ in 0..=n {
            for i in 0..n {
                if done[i].is_some() {
                    continue;
                }
                let chunk = &module.chunks[i];
                let ready = chunk.ops.iter().all(|op| match op {
                    Op::Call(t, _) => done[*t as usize].is_some(),
                    _ => true,
                });
                if !ready {
                    continue;
                }
                CALLEE.with(|c| *c.borrow_mut() = done.iter().map(|d| d.unwrap_or(0)).collect());
                let figure = match chunk.block_type {
                    BlockType::Stream => {
                        verify::wcet_stream_iteration_with_cost_model(chunk, &model, &[])
                    }
                    _ => verify::wcet_whole_chunk_with_callees(chunk, &model, &[], &returns_text),
                }
                .expect("the native method's figure");
                done[i] = Some(figure);
            }
        }
        let expected: Vec<u32> = done.into_iter().map(|d| d.expect("acyclic")).collect();
        let composed = verify::module_wcet_composed_with_bounds(&module, &[], &NOMINAL_COST_MODEL)
            .expect("composed");
        assert_eq!(composed, expected, "{name}");
        compared += 1;
    }
    assert_eq!(compared, 7, "the programs without a suspending callee");
}

/// The hosted runtime composes its admission bound the same way, in `hosted::transitive_bound`,
/// by charging each call as a verified native attested at its callee's bound. It is a third
/// computation of the definition, written before this one, and it agrees wherever it has a
/// figure. It refuses a function that adds or compares two call results, because it bounds a
/// function without its callees' return shapes and so reads such a result as possibly text of
/// unbounded length. Those chunks and their callers are counted and not compared.
#[cfg(feature = "self-host")]
#[test]
fn the_composed_figure_agrees_with_the_hosted_runtimes_transitive_bound() {
    let mut compared = 0usize;
    let mut refused = 0usize;
    for (name, src) in corpus() {
        let module = compile_src(src);
        let suspending_callee = module.chunks.iter().any(|c| {
            c.ops.iter().any(|op| {
                matches!(op, Op::Call(t, _) if module.chunks[*t as usize].block_type != BlockType::Func)
            })
        });
        if suspending_callee {
            continue;
        }
        let composed = verify::module_wcet_composed_with_bounds(&module, &[], &NOMINAL_COST_MODEL)
            .expect("composed");
        for (i, chunk) in module.chunks.iter().enumerate() {
            match keleusma::hosted::transitive_bound(&module, i) {
                Ok(hosted) => {
                    assert_eq!(hosted, u64::from(composed[i]), "{name}: {}", chunk.name);
                    compared += 1;
                }
                Err(_) => refused += 1,
            }
        }
    }
    assert!(
        compared >= 8,
        "only {compared} chunks were compared, and {refused} were refused"
    );
}

// --- suspending callees ------------------------------------------------------------------------

#[test]
fn a_yield_callee_is_charged_its_whole_body_not_one_resume() {
    // The shallow figure of a `yield` function is the cost of its longest resume. A resume slice
    // can span the caller and the callee, so the composed figure charges the whole call, and it
    // is documented as a bound per whole call.
    let module = compile_src(
        "yield g(a: Word) -> Word { for i in 0..5 { let w = a + 1; }; yield a; \
         for i in 0..5 { let w = a + 1; }; yield a + 1 }\n\
         loop main(r: Word) -> Word { let x = g(r); yield x }",
    );
    let g = chunk_index(&module, "g");
    assert_eq!(module.chunks[g].block_type, BlockType::Reentrant);
    let per_resume = shallow_unit(&module)[g];
    let whole_call = composed_unit(&module)[g];
    assert!(
        whole_call > per_resume,
        "whole call {whole_call}, longest resume {per_resume}"
    );
    // Every operation of `g` runs once, so the whole-call figure of this body is at least its
    // two loops, which the per-resume figure counts one of.
    assert!(
        whole_call >= 2 * (per_resume - 4),
        "{whole_call} {per_resume}"
    );
    let main = chunk_index(&module, "main");
    assert_eq!(
        composed_unit(&module)[main],
        shallow_unit(&module)[main] + whole_call
    );
}

#[test]
fn a_delegated_loop_is_bounded_by_its_own_figure_after_the_first_iteration() {
    // `main` delegates to `child`, which never returns. Every reset is the child's. The first
    // iteration also runs the prologue of `main`, so it is bounded by the figure of `main`,
    // which charges the delegation the child's first iteration. Later iterations are bounded by
    // the child's own figure.
    let module = compile_src(
        "fn work(x: Word) -> Word { for i in 0..6 { let w = x + 1; }; x }\n\
         loop child(a: Word) -> Word { let b = work(a); yield b; a }\n\
         loop main(t: Word) -> Word { let u = work(t); let v = work(u); child(v) }",
    );
    let (main, child) = (chunk_index(&module, "main"), chunk_index(&module, "child"));
    let composed = composed_unit(&module);
    let spans = measure(&module, 4);
    for (i, &(_, chunk)) in spans.iter().enumerate() {
        assert_eq!(chunk, child, "iteration {i} is the child's");
    }
    assert!(
        spans[0].0 > composed[child] as u64,
        "the first iteration, {}, exceeds the child's own figure, {}, which is why the caller's \
         figure is the one that bounds it",
        spans[0].0,
        composed[child]
    );
    assert!(
        spans[0].0 <= composed[main] as u64,
        "{} {}",
        spans[0].0,
        composed[main]
    );
    for &(count, _) in &spans[1..] {
        assert!(
            count <= composed[child] as u64,
            "{count} {}",
            composed[child]
        );
    }
    // The virtual machine's figure is the largest over the loops, so it bounds every iteration.
    let arena = keleusma::Arena::with_capacity(1 << 20);
    let vm = Vm::new(module.clone(), &arena).expect("load");
    let nominal = vm.wcet_per_iteration_composed().expect("a composed figure");
    let per_chunk =
        verify::module_wcet_composed_with_bounds(&module, &[], &NOMINAL_COST_MODEL).unwrap();
    assert_eq!(nominal, per_chunk[main].max(per_chunk[child]));
    assert!(nominal >= vm.wcet_per_iteration().expect("the shallow figure"));
}

// --- the virtual machine's accessor ------------------------------------------------------------

#[test]
fn the_virtual_machine_reports_the_composed_figure_beside_the_shallow_one() {
    let arena = keleusma::Arena::with_capacity(1 << 20);
    let module = compile_src(PROOFS_EXAMPLE);
    let vm = Vm::new(module.clone(), &arena).expect("load");
    let shallow = vm.wcet_per_iteration().expect("shallow");
    let composed = vm.wcet_per_iteration_composed().expect("composed");
    assert_eq!(shallow, 20, "the existing accessor is unchanged");
    let per_chunk =
        verify::module_wcet_composed_with_bounds(&module, &[], &NOMINAL_COST_MODEL).unwrap();
    assert_eq!(composed, per_chunk[chunk_index(&module, "main")]);
    assert!(
        composed > 872,
        "nominal costs are at least one per operation: {composed}"
    );

    let call_free = compile_src("loop main(x: Word) -> Word { yield x }");
    let vm = Vm::new(call_free, &arena).expect("load");
    assert_eq!(
        vm.wcet_per_iteration_composed().unwrap(),
        vm.wcet_per_iteration().unwrap()
    );

    // A module with no loop has no iteration, and both accessors say zero.
    let atomic = compile_src("fn main() -> Word { 1 }");
    let vm = Vm::new(atomic, &arena).expect("load");
    assert_eq!(vm.wcet_per_iteration_composed().unwrap(), 0);
}

#[test]
fn the_shallow_figures_and_the_header_are_unchanged() {
    // The figures the proofs line recorded before the addition, and one more taken from
    // `tests/wcet_marker_charging.rs`.
    let module = compile_src(PROOFS_EXAMPLE);
    assert_eq!(module.wcet_cycles, 20);
    let main = &module.chunks[chunk_index(&module, "main")];
    assert_eq!(verify::wcet_stream_iteration(main).unwrap(), 20);
    assert_eq!(
        verify::wcet_stream_iteration_with_cost_model(main, &UNIT, &[]).unwrap(),
        11
    );
    let atomic = compile_src("fn main(n: Word) -> Word { for i in 0..3 { } n }");
    assert_eq!(verify::wcet_whole_chunk(&atomic.chunks[0]).unwrap(), 55);
    assert_eq!(
        verify::module_wcet_with_bounds(&atomic, &[], &NOMINAL_COST_MODEL).unwrap(),
        vec![55]
    );
}

// --- natives, saturation and refusals ----------------------------------------------------------

#[test]
fn an_attested_native_inside_a_callee_is_scaled_by_the_callers_loop() {
    let module = compile_src(
        "use host::probe\n\
         fn g(x: Word) -> Word { host::probe(x) }\n\
         loop main(x: Word) -> Word { for i in 0..4 { let w = g(i); }; yield x; x }",
    );
    assert_eq!(module.native_names.len(), 1);
    let main = chunk_index(&module, "main");
    let attested = |cycles: u32| {
        let bounds = [NativeIterationBound {
            per_call_wcmu_bytes: 0,
            per_call_wcet_cycles: cycles,
            max_invocations: None,
        }];
        verify::module_wcet_composed_with_bounds(&module, &bounds, &NOMINAL_COST_MODEL)
            .expect("composed")[main]
    };
    // The native runs once per call of `g`, and `g` runs four times.
    assert_eq!(attested(1000) - attested(0), 4 * 1000);
    // The shallow figure of `main` does not see the native at all, since it is in the callee.
    let shallow = |cycles: u32| {
        let bounds = [NativeIterationBound {
            per_call_wcmu_bytes: 0,
            per_call_wcet_cycles: cycles,
            max_invocations: None,
        }];
        verify::module_wcet_with_bounds(&module, &bounds, &NOMINAL_COST_MODEL).unwrap()[main]
    };
    assert_eq!(shallow(1000), shallow(0));
}

#[test]
fn a_saturated_callee_saturates_every_caller_and_the_accessor_refuses() {
    // The callee alone exceeds the range. A caller that added a saturated figure to its own
    // would stay saturated, and one that multiplied a clamped figure could fall back below the
    // range, so the saturation is carried explicitly.
    let module = compile_src(
        "fn big(n: Word) -> Word { \
         for i in 0..3000000 limit 3000000 { \
         for j in 0..3000000 limit 3000000 { \
         for k in 0..3000000 limit 3000000 { let w = n + 1; } } } n }\n\
         fn mid(n: Word) -> Word { big(n) }\n\
         loop main(n: Word) -> Word { let w = mid(n); yield n }",
    );
    for model in [&UNIT, &NOMINAL_COST_MODEL] {
        let composed =
            verify::module_wcet_composed_with_bounds(&module, &[], model).expect("figures");
        for name in ["big", "mid", "main"] {
            assert_eq!(composed[chunk_index(&module, name)], u32::MAX, "{name}");
        }
    }
    // The header is the shallow figure and is finite, so the module loads. The composed
    // accessor refuses to report a saturated figure as a bound.
    assert_ne!(module.wcet_cycles, u32::MAX);
    let arena = keleusma::Arena::with_capacity(1 << 20);
    let vm = Vm::new(module, &arena).expect("the module loads on its shallow header");
    let err = vm
        .wcet_per_iteration_composed()
        .expect_err("a saturated figure is not a bound");
    assert!(format!("{err:?}").contains("saturated"), "{err:?}");
}

#[test]
fn a_zero_cost_call_does_not_hide_a_saturated_callee() {
    // Under a model that charges nothing for anything but one instruction kind, the caller's own
    // instructions add nothing, so only an explicit carry keeps the caller saturated.
    fn only_add(op: &Op) -> u32 {
        match op {
            Op::Add | Op::CheckedAdd => u32::MAX / 2 + 1,
            _ => 0,
        }
    }
    let model = CostModel {
        op_cycles: only_add,
        ..UNIT
    };
    let module = compile_src(
        "fn two(n: Word) -> Word { let a = n + 1; a + 1 }\n\
         fn mid(n: Word) -> Word { two(n) }\n\
         loop main(n: Word) -> Word { yield mid(n); n }",
    );
    let composed = verify::module_wcet_composed_with_bounds(&module, &[], &model).expect("figures");
    assert_eq!(composed[chunk_index(&module, "two")], u32::MAX);
    assert_eq!(composed[chunk_index(&module, "mid")], u32::MAX);
    assert_eq!(composed[chunk_index(&module, "main")], u32::MAX);
}

#[test]
fn a_call_cycle_and_a_missing_callee_are_refused() {
    let base = compile_src("fn a() -> Word { 1 }\nfn main() -> Word { a() }");
    let (a, main) = (chunk_index(&base, "a"), chunk_index(&base, "main"));
    verify::module_wcet_composed_with_bounds(&base, &[], &UNIT).expect("the control");

    let mut cyclic = base.clone();
    cyclic.chunks[a].ops = vec![Op::Call(main as u16, 0), Op::Return];
    let err = verify::module_wcet_composed_with_bounds(&cyclic, &[], &UNIT)
        .expect_err("a cycle has no composed bound");
    assert!(
        err.message.contains("recursive call detected"),
        "{}",
        err.message
    );

    let mut dangling = base.clone();
    dangling.chunks[main].ops = vec![Op::Call(999, 0), Op::Return];
    let err = verify::module_wcet_composed_with_bounds(&dangling, &[], &UNIT)
        .expect_err("a call to a chunk that does not exist has no composed bound");
    assert!(err.message.contains("does not exist"), "{}", err.message);
}

#[test]
fn an_unbounded_loop_in_a_callee_refuses_the_whole_query() {
    // The conservative stance. A callee whose loop has no extractable bound has no figure, and
    // neither has anything that calls it.
    let module = compile_src(
        "fn g(n: Word) -> Word { for i in 0..n { let w = i + 1; }; n }\n\
         loop main(x: Word) -> Word { yield g(x); x }",
    );
    assert!(verify::module_wcet_composed_with_bounds(&module, &[], &UNIT).is_err());
}
