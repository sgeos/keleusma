#![cfg(feature = "self-host")]
//! The hosted runtime against the Rust virtual machine (Order 4).
//!
//! Every corpus case is compiled by the shipping self-hosted compiler, admitted or refused by
//! the hosted driver, and, when admitted, run under both runtimes over a fixed argument set.
//! The two must agree on the outcome and the value. The harness counts how many cases reached
//! each stage and fails when a count collapses, because an empty agreement is not evidence.
//! See docs/decisions/ORDER4_HOSTED_RUNTIME_PLAN.md.

#[path = "hosted_runtime/corpus.rs"]
mod corpus;

use keleusma::ast::{ConstExpr, PrimType, Program, TypeDef, TypeExpr};
use keleusma::bytecode::{ArrayBody, BoxedEnum, EnumBody, Module, Op, StructBody, TupleBody};
use keleusma::hosted::{
    Admitted, HostedValue, Interpreter, Outcome, Refusal, admit, interpreter_module,
    interpreter_persistent_bytes, reference_outcome,
};
use keleusma::target::Target;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm};
use keleusma::{Arena, Value};

/// Ticks a run may use before it counts as not ending. Far above any corpus case.
const MAX_TICKS: u64 = 100_000;

fn interpreter_arena() -> (Module, Arena) {
    let m = interpreter_module().expect("interp.kel compiles");
    let need = interpreter_persistent_bytes(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    (m, arena)
}

/// Sample argument values built from declared types, for both runtimes. Scalars come from a
/// fixed set that includes the extremes. A composite is built from its element samples, rotated
/// so successive samples differ in every position, and an enum takes every variant. Each is a
/// boxed Rust value: the virtual machine canonicalises it at entry, and the harness gives the
/// interpreter the same canonical body.
struct Samples<'p> {
    program: &'p Program,
    module: &'p Module,
}

impl Samples<'_> {
    /// At most `per` samples of `ty`, or `None` for a type the first pass does not generate.
    fn of(&self, ty: &TypeExpr, per: usize) -> Option<Vec<Value>> {
        let vals: Vec<Value> = match ty {
            TypeExpr::Prim(PrimType::Word, _) => [0, 1, -1, 7, -9, i64::MAX, i64::MIN]
                .into_iter()
                .map(Value::Int)
                .collect(),
            TypeExpr::Prim(PrimType::Byte, _) => {
                [0u8, 1, 7, 128, 255].into_iter().map(Value::Byte).collect()
            }
            TypeExpr::Prim(PrimType::Bool, _) => vec![Value::Bool(false), Value::Bool(true)],
            TypeExpr::Tuple(elems, _) => self
                .rows(elems, per)?
                .into_iter()
                .map(|r| Value::Tuple(TupleBody::boxed(r)))
                .collect(),
            TypeExpr::Array(elem, ConstExpr::Lit(n, _), _) => {
                let elems = vec![(**elem).clone(); usize::try_from(*n).ok()?];
                self.rows(&elems, per)?
                    .into_iter()
                    .map(|r| Value::Array(ArrayBody::boxed(r)))
                    .collect()
            }
            TypeExpr::Named(name, targs, cargs, _) if targs.is_empty() && cargs.is_empty() => {
                self.named(name, per)?
            }
            _ => return None,
        };
        Some(vals.into_iter().take(per).collect())
    }

    /// Rows of one sample per element type, rotated so row `r` takes sample `r + j` of element
    /// `j`.
    fn rows(&self, elems: &[TypeExpr], per: usize) -> Option<Vec<Vec<Value>>> {
        let pools: Vec<Vec<Value>> = elems
            .iter()
            .map(|e| self.of(e, per))
            .collect::<Option<_>>()?;
        let n = pools
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(1)
            .min(per)
            .max(1);
        Some(
            (0..n)
                .map(|r| {
                    pools
                        .iter()
                        .enumerate()
                        .map(|(j, p)| p[(r + j) % p.len()].clone())
                        .collect()
                })
                .collect(),
        )
    }

    fn named(&self, name: &str, per: usize) -> Option<Vec<Value>> {
        for td in &self.program.types {
            match td {
                TypeDef::Struct(sd) if sd.name == name => {
                    let tys: Vec<TypeExpr> =
                        sd.fields.iter().map(|f| f.type_expr.clone()).collect();
                    return Some(
                        self.rows(&tys, per)?
                            .into_iter()
                            .map(|r| {
                                let fields =
                                    sd.fields.iter().map(|f| f.name.clone()).zip(r).collect();
                                Value::Struct(StructBody::boxed(name.to_string(), fields))
                            })
                            .collect(),
                    );
                }
                TypeDef::Enum(ed) if ed.name == name => {
                    let layout = self
                        .module
                        .enum_layouts
                        .iter()
                        .find(|l| l.type_name == name)?;
                    let mut out = Vec::new();
                    for v in &ed.variants {
                        let payloads = if v.fields.is_empty() {
                            vec![Vec::new()]
                        } else {
                            self.rows(&v.fields, 2)?
                        };
                        for fields in payloads {
                            out.push(Value::Enum(EnumBody::Boxed(Box::new(BoxedEnum {
                                type_name: name.to_string(),
                                variant: v.name.clone(),
                                disc: v.discriminant_value,
                                min_payload: layout.min_payload as usize,
                                fields,
                            }))));
                        }
                    }
                    return Some(out);
                }
                _ => {}
            }
        }
        None
    }
}

/// Every argument vector over the entry parameters' samples, capped so a case stays cheap.
/// `None` when a parameter's type has no samples.
fn arg_vectors(program: &Program, module: &Module) -> Option<Vec<Vec<Value>>> {
    let main = program.functions.iter().find(|f| f.name == "main")?;
    let samples = Samples { program, module };
    let mut vectors = vec![Vec::new()];
    for p in &main.params {
        let s = samples.of(p.type_expr.as_ref()?, 4)?;
        vectors = vectors
            .iter()
            .flat_map(|v| {
                s.iter().map(move |x| {
                    let mut w: Vec<Value> = v.clone();
                    w.push(x.clone());
                    w
                })
            })
            .collect();
    }
    vectors.truncate(64);
    Some(vectors)
}

/// The interpreter's form of a Rust argument: its canonical flat body for a composite. `None`
/// when the value does not canonicalise to a flat body.
fn hosted_arg(v: &Value) -> Option<HostedValue> {
    let scratch = Arena::with_capacity(4096);
    let canon = v.clone().into_arena_canonical(8, 8, 8, &scratch).ok()?;
    HostedValue::from_value(&canon, &scratch)
}

fn self_hosted(src: &str) -> Option<Module> {
    keleusma::selfhost::self_hosted_compile(src, &Target::host()).ok()
}

/// A boundary case made runnable. The boundary test compares bytes and never runs its cases, and
/// most name their function `f`, which leaves the module without an entry point that `Vm::new`
/// requires. The rename changes the source, so the shipping self-hosted compile, which errors on
/// any divergence from the reference, re-establishes byte identity for the renamed program.
fn runnable(src: &str) -> String {
    if src.contains("fn main(") || src.contains("loop main(") {
        src.to_string()
    } else {
        src.replacen("fn f(", "fn main(", 1)
    }
}

#[derive(Default, Debug)]
struct Reach {
    cases: usize,
    compiled: usize,
    admitted: usize,
    generated: usize,
    runs: usize,
    finished: usize,
    trapped: usize,
    max_steps: u64,
    composite_arg_runs: usize,
    composite_results: usize,
}

#[test]
fn the_interpreter_is_admitted_by_the_rust_verifier() {
    let (m, arena) = interpreter_arena();
    // The interpreter's own bounds are what the fuel model composes with, so they must exist.
    let main = m.entry_point.expect("interp.kel has an entry");
    let tick = keleusma::verify::wcet_stream_iteration(&m.chunks[main]).expect("a tick bound");
    assert!(tick > 0, "interp.kel has no time bound");
    Interpreter::new(m, &arena).expect("interp.kel verifies");
}

#[test]
fn the_interpreter_agrees_with_the_virtual_machine_on_the_corpus() {
    let (m, arena) = interpreter_arena();
    let mut interp = Interpreter::new(m, &arena).expect("interp.kel verifies");
    let mut reach = Reach::default();
    let mut failures = Vec::new();
    let mut refusals = Vec::new();

    for (label, src) in corpus::BOUNDARY_OK {
        reach.cases += 1;
        let src = &runnable(src);
        let Some(module) = self_hosted(src) else {
            continue;
        };
        reach.compiled += 1;
        let adm: Admitted = match admit(&module) {
            Ok(a) => a,
            Err(r) => {
                refusals.push(format!("{label}: {r:?}"));
                continue;
            }
        };
        reach.admitted += 1;
        let program = keleusma::parser::parse(&keleusma::lexer::tokenize(src).unwrap()).unwrap();
        let Some(vectors) = arg_vectors(&program, &module) else {
            refusals.push(format!("{label}: no argument samples"));
            continue;
        };
        reach.generated += 1;
        for values in vectors {
            let Some(args) = values.iter().map(hosted_arg).collect::<Option<Vec<_>>>() else {
                failures.push(format!("{label}: an argument has no flat body"));
                continue;
            };
            let vm_arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
            let mut vm = Vm::new(module.clone(), &vm_arena).expect("the corpus module verifies");
            let expected = match reference_outcome(vm.call(&values), &vm_arena) {
                Ok(o) => o,
                Err(e) => {
                    failures.push(format!("{label} {args:?}: reference {e}"));
                    continue;
                }
            };
            let run = interp.run(&adm, &args, MAX_TICKS).expect("interp.kel runs");
            reach.runs += 1;
            if args
                .iter()
                .any(|a| matches!(a, HostedValue::Composite { .. }))
            {
                reach.composite_arg_runs += 1;
            }
            match run.outcome {
                Outcome::Finished(HostedValue::Composite { .. }) => {
                    reach.finished += 1;
                    reach.composite_results += 1;
                }
                Outcome::Finished(_) => reach.finished += 1,
                Outcome::Trapped { .. } => reach.trapped += 1,
                _ => {}
            }
            if run.outcome != expected {
                failures.push(format!(
                    "{label} {args:?}: interpreter {:?}, virtual machine {expected:?}",
                    run.outcome
                ));
            }
            // The bound composition. Every instruction costs at least one nominal cycle, so a
            // run executes no more instructions than the module's verified time bound, and
            // takes no more ticks than that bound over the fuel, plus the ending tick.
            if run.steps > adm.instruction_bound() {
                failures.push(format!(
                    "{label} {args:?}: {} steps against a bound of {}",
                    run.steps,
                    adm.instruction_bound()
                ));
            }
            if run.ticks > adm.tick_bound() {
                failures.push(format!(
                    "{label} {args:?}: {} ticks against a bound of {}",
                    run.ticks,
                    adm.tick_bound()
                ));
            }
            reach.max_steps = reach.max_steps.max(run.steps);
        }
    }

    println!("{reach:?}");
    println!("refused: {}", refusals.len());
    for r in &refusals {
        println!("  {r}");
    }
    assert!(
        failures.is_empty(),
        "{} disagreements:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
    // NON-VACUITY. These floors are the counts measured when slice 1 landed. A later slice
    // raises them. A drop means the harness stopped reaching what it used to.
    assert_eq!(reach.cases, 107, "{reach:?}");
    assert!(reach.compiled >= 100, "{reach:?}");
    assert!(reach.admitted >= SLICE_ADMITTED_FLOOR, "{reach:?}");
    assert!(reach.runs >= SLICE_RUNS_FLOOR, "{reach:?}");
    assert!(reach.finished > 0 && reach.trapped > 0, "{reach:?}");
    assert!(
        reach.composite_arg_runs >= SLICE_COMPOSITE_ARG_RUNS_FLOOR,
        "{reach:?}"
    );
    assert!(
        reach.composite_results >= SLICE_COMPOSITE_RESULTS_FLOOR,
        "{reach:?}"
    );
}

/// Measured floors for the current slice. See the non-vacuity note above. Slice 1, scalars and
/// control flow, measured 47 admitted cases and 698 runs on 2026-10-25. Slice 2, flat
/// composites, measured the figures below the same day.
const SLICE_ADMITTED_FLOOR: usize = 101;
const SLICE_RUNS_FLOOR: usize = 971;
const SLICE_COMPOSITE_ARG_RUNS_FLOOR: usize = 615;
const SLICE_COMPOSITE_RESULTS_FLOOR: usize = 4;

#[test]
fn a_module_with_an_unsupported_instruction_is_refused() {
    let src = "fn main(x: Word) -> Word { x }";
    let mut m = self_hosted(src).expect("compiles");
    m.chunks[0].ops.insert(0, Op::Stream);
    assert!(matches!(admit(&m), Err(Refusal::Unsupported(_))));
}

#[test]
fn a_fault_is_reported_and_is_not_a_trap() {
    // A hand-built module the Rust verifier would refuse, reading a local past the frame. The
    // interpreter must report fault 8 rather than read past its stack or trap itself.
    let mut m = self_hosted("fn main(x: Word) -> Word { x }").expect("compiles");
    m.chunks[0].ops = vec![Op::GetLocal(9), Op::Return];
    let adm = admit(&m).expect("admission checks forms, not stack discipline");
    let (im, arena) = interpreter_arena();
    let mut interp = Interpreter::new(im, &arena).expect("interp.kel verifies");
    let run = interp
        .run(&adm, &[HostedValue::Word(3)], MAX_TICKS)
        .expect("interp.kel runs");
    assert_eq!(run.outcome, Outcome::Fault(8));
}

#[test]
fn a_long_run_spans_ticks_and_ends_in_its_bound() {
    // A control: a run longer than one tick must take several ticks and still finish, which
    // separates the tick loop from a single-step interpreter.
    let src = "fn main(n: Word) -> Word { for i in 0..20 { let x = i * 2 + n; } n + 190 }";
    let Some(m) = self_hosted(src) else {
        // The self-hosted compiler may not reach this form; the reference control then runs.
        let p = keleusma::parser::parse(&keleusma::lexer::tokenize(src).unwrap()).unwrap();
        let m = keleusma::compiler::compile(&p).unwrap();
        return long_run(m);
    };
    long_run(m)
}

fn long_run(m: Module) {
    let adm = admit(&m).expect("admitted");
    let (im, arena) = interpreter_arena();
    let mut interp = Interpreter::new(im, &arena).expect("interp.kel verifies");
    let run = interp
        .run(&adm, &[HostedValue::Word(0)], MAX_TICKS)
        .expect("interp.kel runs");
    assert_eq!(run.outcome, Outcome::Finished(HostedValue::Word(190)));
    assert!(run.ticks > 1, "{run:?}");
    assert!(
        run.steps <= adm.instruction_bound(),
        "{run:?} against {adm:?}"
    );
    assert!(run.ticks <= adm.tick_bound(), "{run:?} against {adm:?}");
}
