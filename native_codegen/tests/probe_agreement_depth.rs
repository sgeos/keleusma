//! **How much is each agreeing module actually agreeing about?**
//!
//! `corpus_differential` counts 34 modules as executed and agreeing. Six more are
//! pinned as vacuous, all of them self-hosted stages found by
//! `probe_stage_vacuity`. That search had a blind spot, and this file measures it.
//!
//! `is_vacuous` returns false immediately for any module with fewer than two
//! results. That is right for a single call site — one result is not evidence
//! either way — but it means **every single-call module inside the 34 is
//! unexamined**. The nine stages were caught agreeing on nothing because the
//! stages are where I looked. The same thin agreement could sit in the numbered
//! examples and the rogue scripts, and nothing has asked.
//!
//! # This file REPORTS and does not classify
//!
//! The distribution is printed before any threshold is chosen, deliberately.
//! Picking the cut after seeing which modules it moves is how a measurement turns
//! into an argument for a conclusion already held. Whatever classifier change
//! follows belongs in `corpus_differential.rs` and should cite this output.
//!
//! # Why the virtual machine alone is enough here
//!
//! Depth of agreement is a property of how much work the run does, and the two
//! sides already agree on these modules — that is what makes them the 34. So the
//! virtual machine's own observables measure the shared quantity, and the JIT
//! adds cost without adding information.
use keleusma::bytecode::{BlockType, Module, Op, Value, WireShape};
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::LowerOptions;
use std::cell::RefCell;

/// Matches `corpus_differential::TICKS`, so this measures the same run.
const TICKS: i64 = 60;
/// Matches `corpus_differential::STUBS`.
const STUBS: usize = 48;

thread_local! {
    static CALLS: RefCell<usize> = const { RefCell::new(0) };
    static SAW_REF_ARG: RefCell<bool> = const { RefCell::new(false) };
}

/// Byte-for-byte the value `corpus_differential` returns from every stub.
///
/// Reproduced rather than shared because the two files are separate test
/// binaries. A DIFFERENT value here would drive control flow down a different
/// path and this probe would be measuring a run the differential never makes.
fn stub_value(idx: usize, args: &[i64]) -> i64 {
    let mut acc = (idx as i64 + 1) * 7;
    for (i, a) in args.iter().enumerate() {
        acc = acc
            .wrapping_mul(31)
            .wrapping_add(a.wrapping_mul(i as i64 + 1));
    }
    acc % 1024
}

fn sources() -> Vec<std::path::PathBuf> {
    let root = std::path::Path::new("..");
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ]
    .iter()
    .map(|d| root.join(d))
    .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            out.push(p);
        }
    }
    out.sort();
    out
}

fn native_table(m: &Module) -> Vec<(String, usize)> {
    let mut argc: Vec<Option<usize>> = vec![None; m.native_names.len()];
    for c in &m.chunks {
        for op in &c.ops {
            if let Op::CallVerifiedNative(i, n) | Op::CallExternalNative(i, n) = op {
                argc[usize::from(*i)] = Some(usize::from(n & 0x7F));
            }
        }
    }
    m.native_names
        .iter()
        .zip(argc)
        .map(|(n, a)| (n.clone(), a.unwrap_or(0)))
        .collect()
}

fn params_are_scalar(m: &Module, entry: usize) -> bool {
    match m.signatures.get(entry) {
        Some(sig) => sig
            .params
            .iter()
            .all(|p| matches!(p, WireShape::Scalar { .. })),
        None => m.chunks[entry].param_count == 0,
    }
}

struct Depth {
    results: usize,
    distinct: usize,
    calls: usize,
    shared_touched: usize,
    shared_total: usize,
    is_stream: bool,
    /// Does the observable output CHANGE when the entry arguments change?
    ///
    /// `None` when the entry takes no arguments, so there is nothing to vary.
    ///
    /// **This is the measure that matters for a single-call module**, and the
    /// first version of this probe did not have it. Counting "one result" as
    /// trivial conflates a module that returns a constant with one that returns a
    /// computed value, and `10_multbyte.kel` is the standing refutation: one
    /// result, no host calls, no shared segment — and that single integer is what
    /// caught the composite-return aliasing defect, `vm 7` against `native 8`.
    ///
    /// A run whose output does not depend on its input is exercising a fixed
    /// path. A run whose output tracks its input is carrying information through
    /// the emitter, however few values it emits.
    responds_to_input: Option<bool>,
}

impl Depth {
    fn trivial_calls(&self) -> bool {
        self.calls == 0
    }
    fn trivial_shared(&self) -> bool {
        self.shared_touched == 0
    }
    /// Is the RESULT channel carrying information?
    ///
    /// For a stream, more than one distinct value across the run. For a single
    /// call, a result that moves when the input moves. A zero-argument single
    /// call has neither signal available and is reported as unknown rather than
    /// assumed either way.
    fn trivial_results(&self) -> Option<bool> {
        if self.is_stream || self.results > 1 {
            Some(self.distinct <= 1)
        } else {
            self.responds_to_input.map(|r| !r)
        }
    }
    /// How many of the three observables carry no information, where that is
    /// known. A module whose result channel is unknown is counted on the two
    /// channels that are known, and flagged in the table.
    fn trivial_count(&self) -> usize {
        usize::from(self.trivial_results().unwrap_or(false))
            + usize::from(self.trivial_calls())
            + usize::from(self.trivial_shared())
    }
    fn known(&self) -> bool {
        self.trivial_results().is_some()
    }
}

/// Run the module once and return `(results, calls, shared_touched, shared_total)`.
///
/// `seed` perturbs the entry arguments. Seed 0 reproduces `corpus_differential`'s
/// own `args_for`, so the reported figures describe the differential's run; a
/// nonzero seed exists only to answer whether the output responds to its input.
fn run_once(m: &Module, seed: i64) -> Result<(Vec<i64>, usize, usize, usize), String> {
    CALLS.with(|c| *c.borrow_mut() = 0);
    SAW_REF_ARG.with(|f| *f.borrow_mut() = false);

    const HOST_MARGIN: usize = 4 << 20;
    let need = required_persistent_capacity_for(m);
    let cap =
        auto_arena_capacity_for(m, &[]).map_err(|e| format!("arena: {e:?}"))? + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena
        .resize_persistent(need)
        .map_err(|e| format!("persistent: {e:?}"))?;
    let mut vm = Vm::new(m.clone(), &arena).map_err(|e| format!("load: {e:?}"))?;

    let table = native_table(m);
    if table.len() > STUBS {
        return Err(format!("{} natives exceeds {STUBS} stubs", table.len()));
    }
    for (idx, (name, argc)) in table.iter().enumerate() {
        if name.is_empty() {
            continue;
        }
        let ac = *argc;
        vm.register_native_closure(name, move |args: &[Value]| {
            let vals: Vec<i64> = args
                .iter()
                .take(ac)
                .map(|v| match v {
                    Value::Int(x) => *x,
                    Value::Byte(b) => i64::from(*b),
                    Value::Bool(b) => i64::from(*b),
                    _ => {
                        SAW_REF_ARG.with(|f| *f.borrow_mut() = true);
                        0
                    }
                })
                .collect();
            CALLS.with(|c| *c.borrow_mut() += 1);
            Ok(Value::Int(stub_value(idx, &vals)))
        });
    }

    let entry = m.entry_point.ok_or("no entry point")?;
    let n = m.chunks[entry].param_count as usize;
    let is_stream = m.chunks[entry].block_type == BlockType::Stream;
    let vals: Vec<Value> = if is_stream && n == 1 {
        vec![Value::Int(0)]
    } else {
        (0..n)
            .map(|i| Value::Int((i as i64 + 1) * 3 + 1 + seed * 23))
            .collect()
    };

    let mut shared = vec![0u8; shared_data_bytes_for(m)];
    let shared_total = shared.len();
    let mut results = Vec::new();

    let first = vm
        .call_with_shared(&mut shared, &vals)
        .map_err(|e| format!("run: {e:?}"))?;
    results.push(scalar_of(&first));
    if is_stream {
        for t in 1..TICKS {
            let mut st = vm
                .resume_with_shared(&mut shared, Value::Int(t))
                .map_err(|e| format!("resume: {e:?}"))?;
            if matches!(st, VmState::Reset) {
                st = vm
                    .resume_with_shared(&mut shared, Value::Int(t))
                    .map_err(|e| format!("resume: {e:?}"))?;
            }
            results.push(scalar_of(&st));
        }
    }
    if SAW_REF_ARG.with(|f| *f.borrow()) {
        return Err("reference argument (exempt in the differential too)".into());
    }

    Ok((
        results,
        CALLS.with(|c| *c.borrow()),
        // The segment starts all zero, so a nonzero byte is one the run wrote.
        shared.iter().filter(|b| **b != 0).count(),
        shared_total,
    ))
}

fn measure(m: &Module) -> Result<Depth, String> {
    let entry = m.entry_point.ok_or("no entry point")?;
    let is_stream = m.chunks[entry].block_type == BlockType::Stream;
    let n = m.chunks[entry].param_count as usize;

    let (results, calls, shared_touched, shared_total) = run_once(m, 0)?;

    // Only meaningful for a single call with at least one argument. For a stream
    // the entry parameter is the tick, which the driver already varies, and the
    // distinct-value count is the measure.
    let responds_to_input = if is_stream || n == 0 {
        None
    } else {
        match run_once(m, 1) {
            Ok((other, other_calls, other_shared, _)) => {
                Some(other != results || other_calls != calls || other_shared != shared_touched)
            }
            // A perturbed input that traps is itself evidence the run depends on
            // its input, but it is not the same observation, so it is reported as
            // unknown rather than folded in.
            Err(_) => None,
        }
    };

    let mut d = results.clone();
    d.sort_unstable();
    d.dedup();
    Ok(Depth {
        results: results.len(),
        distinct: d.len(),
        calls,
        shared_touched,
        shared_total,
        is_stream,
        responds_to_input,
    })
}

fn scalar_of(st: &VmState) -> i64 {
    match st {
        VmState::Yielded(Value::Int(v)) | VmState::Finished(Value::Int(v)) => *v,
        VmState::Yielded(Value::Unit) | VmState::Finished(Value::Unit) => 0,
        VmState::Reset => i64::MIN + 1,
        _ => i64::MIN,
    }
}

#[test]
fn how_deep_is_each_agreeing_modules_agreement() {
    let mut rows: Vec<(String, Depth)> = Vec::new();
    let mut skipped = 0usize;

    for p in sources() {
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            skipped += 1;
            continue;
        };
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            skipped += 1;
            continue;
        }
        let Some(entry) = m.entry_point else {
            skipped += 1;
            continue;
        };
        if !params_are_scalar(&m, entry) {
            skipped += 1;
            continue;
        }
        match measure(&m) {
            Ok(d) => rows.push((name, d)),
            Err(_) => skipped += 1,
        }
    }

    println!("\n================ DEPTH OF AGREEMENT, per module the differential counts");
    println!("  `trivial` counts how many of the THREE compared observables carry no");
    println!("  information: one repeated result, no host calls, an untouched segment.\n");
    println!(
        "  {:<26} {:>5} {:>9} {:>7} {:>16} {:>9}  trivial",
        "module", "runs", "distinct", "calls", "shared touched", "responds"
    );

    rows.sort_by_key(|(n, d)| (std::cmp::Reverse(d.trivial_count()), n.clone()));
    for (name, d) in &rows {
        let responds = match d.responds_to_input {
            Some(true) => "yes",
            Some(false) => "NO",
            None => "-",
        };
        println!(
            "  {:<26} {:>5} {:>9} {:>7} {:>7}/{:<8} {responds:>9}  {}/3{}{}",
            name,
            d.results,
            d.distinct,
            d.calls,
            d.shared_touched,
            d.shared_total,
            d.trivial_count(),
            if d.is_stream { " (stream)" } else { "" },
            if d.known() {
                ""
            } else {
                " [result channel UNKNOWN]"
            }
        );
    }

    // THE DISTRIBUTION. This is the output the classifier decision must cite.
    let mut hist = [0usize; 4];
    let mut single_call_trivial3 = 0usize;
    let mut stream_trivial3 = 0usize;
    for (_, d) in &rows {
        hist[d.trivial_count()] += 1;
        if d.trivial_count() == 3 {
            if d.is_stream {
                stream_trivial3 += 1;
            } else {
                single_call_trivial3 += 1;
            }
        }
    }
    println!("\n  DISTRIBUTION over {} measured modules:", rows.len());
    for (k, n) in hist.iter().enumerate() {
        println!("    {k} of 3 observables trivial : {n}");
    }
    println!(
        "\n  Of the {} modules where ALL THREE are trivial, {} are single-call and {} are\n  \
         streams. The streams are the ones `is_vacuous` already catches; the single-call\n  \
         ones are the blind spot this probe exists to size.",
        hist[3], single_call_trivial3, stream_trivial3
    );
    println!("  {skipped} sources were not measured (refused, no entry, or exempt).");
    println!("================");

    assert!(
        rows.len() > 20,
        "only {} modules measured; this probe is not covering the corpus and its \
         distribution would be misleading",
        rows.len()
    );
}

/// **Is a mutation to a given opcode even reachable in the corpus?**
///
/// A mutation census is only evidence if the mutated opcode occurs in the
/// modules being measured. `CmpLt` lowered as `SLE` leaves the whole corpus
/// differential passing, and that is a finding about coverage ONLY if these
/// modules emit `CmpLt` at all. Otherwise it is a vacuous mutation and says
/// nothing, which is the same error this whole arc is about.
#[test]
fn how_often_does_each_opcode_occur_in_the_measured_corpus() {
    use std::collections::BTreeMap;
    let mut totals: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for p in sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            continue;
        }
        let Some(entry) = m.entry_point else { continue };
        if !params_are_scalar(&m, entry) {
            continue;
        }
        let mut seen_here: BTreeMap<String, usize> = BTreeMap::new();
        for c in &m.chunks {
            for op in &c.ops {
                // The discriminant name alone; operands would fragment the count.
                let name = format!("{op:?}");
                let name = name.split('(').next().unwrap_or(&name).to_string();
                *seen_here.entry(name).or_insert(0) += 1;
            }
        }
        for (k, n) in seen_here {
            let e = totals.entry(k).or_insert((0, 0));
            e.0 += n;
            e.1 += 1;
        }
    }

    println!("\n================ OPCODE OCCURRENCE across the measured corpus");
    println!("  {:<24} {:>10} {:>10}", "opcode", "sites", "modules");
    let mut rows: Vec<_> = totals.iter().collect();
    rows.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    for (k, (n, mods)) in rows {
        println!("  {k:<24} {n:>10} {mods:>10}");
    }
    println!("================");
    println!("  A mutation to an opcode with ZERO sites proves nothing. One with many");
    println!("  sites that the differential still passes is a real coverage hole.");

    assert!(
        !totals.is_empty(),
        "no opcodes counted; this census would make any mutation look vacuous"
    );
}

/// Machine-readable `opcode -> modules containing it`, for the mutation sweep.
///
/// `tools/mutation_sweep.py` reads this to run each mutation only against the
/// modules that actually emit the mutated opcode. That is both faster and more
/// honest: a module with no site for an opcode cannot detect a defect in it, and
/// counting it as "did not detect" would understate the corpus.
#[test]
fn dump_opcode_module_map() {
    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for p in sources() {
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            continue;
        }
        let Some(entry) = m.entry_point else { continue };
        if !params_are_scalar(&m, entry) {
            continue;
        }
        for c in &m.chunks {
            for op in &c.ops {
                let d = format!("{op:?}");
                let d = d.split('(').next().unwrap_or(&d).to_string();
                let e = map.entry(d).or_default();
                if !e.contains(&name) {
                    e.push(name.clone());
                }
            }
        }
    }
    for (k, mods) in &map {
        println!("OPCODEMAP {k} {}", mods.join(" "));
    }
    assert!(
        !map.is_empty(),
        "the map is empty; the sweep would run nothing"
    );
}

/// **Is an "undetected" mutation actually REACHABLE?**
///
/// Round two of the sweep left six opcodes undetected even with their result
/// replaced by a constant. That has two readings, and only one is a coverage
/// hole: either the corpus never observes the opcode, or the mutation never
/// applied to a site the corpus emits.
///
/// `PushImmediate` is the case that forces the question. Its arm maps an
/// IMMEDIATE INDEX to a value, and the mutation changed the arm for index `1`
/// only. With 1337 sites across 26 modules that sounds broad, but if none of
/// those sites carries index 1 the mutation is vacuous and says nothing at all
/// about coverage. Counting a vacuous mutation as a hole is the same error as
/// counting a vacuous agreement as coverage.
#[test]
fn operand_level_reachability_of_the_undetected_mutations() {
    use std::collections::BTreeMap;
    let mut imm: BTreeMap<u8, usize> = BTreeMap::new();
    let mut owners: BTreeMap<&str, Vec<String>> = BTreeMap::new();

    for p in sources() {
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            continue;
        }
        let Some(entry) = m.entry_point else { continue };
        if !params_are_scalar(&m, entry) {
            continue;
        }
        for c in &m.chunks {
            for op in &c.ops {
                match op {
                    Op::PushImmediate(n) => *imm.entry(*n).or_insert(0) += 1,
                    Op::BitAnd => owners.entry("BitAnd").or_default().push(name.clone()),
                    Op::BitOr => owners.entry("BitOr").or_default().push(name.clone()),
                    Op::Shl => owners.entry("Shl").or_default().push(name.clone()),
                    Op::Shr => owners.entry("Shr").or_default().push(name.clone()),
                    Op::CmpNe => owners.entry("CmpNe").or_default().push(name.clone()),
                    _ => {}
                }
            }
        }
    }

    println!("\n================ REACHABILITY OF THE UNDETECTED MUTATIONS");
    println!("  PushImmediate operand distribution (the mutation touched index 1):");
    for (n, c) in &imm {
        let flag = if *n == 1 {
            "   <- THE MUTATED INDEX"
        } else {
            ""
        };
        println!("    immediate {n:>3} : {c:>6} sites{flag}");
    }
    let idx1 = imm.get(&1).copied().unwrap_or(0);
    println!(
        "\n  => index 1 has {idx1} sites. {}",
        if idx1 == 0 {
            "THE MUTATION WAS VACUOUS -- it is not a coverage hole."
        } else {
            "The mutation was reachable, so the hole is real."
        }
    );

    println!("\n  Modules owning each undetected opcode:");
    for (k, mods) in &owners {
        let mut u: Vec<String> = mods.clone();
        u.sort();
        u.dedup();
        println!("    {k:<8} {:>5} sites in {:?}", mods.len(), u);
    }
    println!("================");
    assert!(
        !imm.is_empty(),
        "no PushImmediate sites; the check is vacuous"
    );
}

/// **Operand distribution for the opcodes the first sweep skipped.**
///
/// The reachability rule, learned the hard way: `PushImmediate` looked like the
/// largest hole in the first sweep and was a vacuous mutation, because all 1337
/// sites carry immediate index 0 and the mutation changed index 1, which has
/// none.
///
/// The 25 skipped opcodes are mostly operand-carrying, and several have variants
/// (`Flat` against `FlatNested` against `Boxed`) reached by different emitter
/// arms. Mutating an arm the corpus never enters would repeat that error at
/// larger scale, so this prints which variants actually occur BEFORE the
/// mutation table for them is written.
#[test]
fn variant_distribution_of_the_skipped_opcodes() {
    use std::collections::BTreeMap;
    let mut var: BTreeMap<String, usize> = BTreeMap::new();
    let mut popn: BTreeMap<u8, usize> = BTreeMap::new();
    let mut callargs: BTreeMap<u8, usize> = BTreeMap::new();

    for p in sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            continue;
        };
        if !keleusma_native::module_refusals(&m, LowerOptions::default()).is_empty() {
            continue;
        }
        let Some(entry) = m.entry_point else { continue };
        if !params_are_scalar(&m, entry) {
            continue;
        }
        for c in &m.chunks {
            for op in &c.ops {
                // The FULL debug rendering, truncated to the variant, so
                // `GetField(Flat { .. })` and `GetField(FlatNested { .. })` are
                // counted apart rather than together.
                let d = format!("{op:?}");
                let key = match op {
                    Op::GetField(_)
                    | Op::GetIndex(_)
                    | Op::GetTupleField(_)
                    | Op::GetEnumField(_)
                    | Op::NewComposite(_) => {
                        let head = d.split('(').next().unwrap_or("").to_string();
                        let variant = d
                            .split_once('(')
                            .map(|(_, r)| r.split_whitespace().next().unwrap_or("?").to_string())
                            .unwrap_or_default();
                        format!("{head}({variant}")
                    }
                    Op::Div | Op::Mod | Op::Trap(_) | Op::IsEnum(_, _, _) => {
                        d.split('(').next().unwrap_or(&d).to_string()
                    }
                    Op::GetData(_)
                    | Op::SetData(_)
                    | Op::GetDataIndexed(_, _)
                    | Op::SetDataIndexed(_, _) => d.split('(').next().unwrap_or(&d).to_string(),
                    Op::PopN(n) => {
                        *popn.entry(*n).or_insert(0) += 1;
                        continue;
                    }
                    Op::Call(_, n) => {
                        *callargs.entry(*n).or_insert(0) += 1;
                        continue;
                    }
                    _ => continue,
                };
                *var.entry(key).or_insert(0) += 1;
            }
        }
    }

    println!("\n================ VARIANT DISTRIBUTION, opcodes skipped by sweep one");
    for (k, n) in &var {
        println!("  {k:<34} {n:>7}");
    }
    println!("\n  PopN operand distribution:");
    for (k, n) in &popn {
        println!("    PopN({k:<3}) {n:>7}");
    }
    println!("\n  Call argument-count distribution:");
    for (k, n) in &callargs {
        println!("    Call(_, {k:<3}) {n:>7}");
    }
    println!("================");
    println!("  A mutation to a variant with ZERO sites is vacuous, whatever the");
    println!("  opcode's headline count says.");
    assert!(!var.is_empty(), "no variants counted; the check is vacuous");
}

/// **What contract can the harness DERIVE for a native's return value?**
///
/// Two exemptions are artefacts of a contractless stub: `rogue_dungen` faults
/// because the stub ignores the range implied by `rng_range(lo, hi)`, and `led`
/// faults because it matches a `Status` enum and the stub returns an integer
/// matching no variant.
///
/// The rule is derive, do not guess. This prints what the bytecode actually
/// records, so the line between the two is drawn from evidence rather than from
/// what would be convenient.
#[test]
fn what_return_contract_does_the_bytecode_record() {
    for path in [
        "../examples/rtos/scripts/led.kel",
        "../examples/scripts/rogue/rogue_dungen.kel",
    ] {
        let Ok(raw) = std::fs::read_to_string(path) else {
            continue;
        };
        // The rtos scripts need the prelude, as the host prepends it.
        let src = if path.contains("/rtos/") {
            let p =
                std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").expect("prelude");
            format!("{p}\n{raw}")
        } else {
            raw
        };
        let Some(m) = tokenize(&src)
            .ok()
            .and_then(|t| parse(&t).ok())
            .and_then(|a| compile(&a).ok())
        else {
            println!("\n{path}: reference compiler rejects it");
            continue;
        };
        println!("\n================ {path}");
        println!("  natives and their RECORDED return shapes:");
        for (i, name) in m.native_names.iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            let shape = m.native_return_shapes.get(i);
            println!("    {i:>2} {name:<28} {shape:?}");
        }
        println!("  enum layouts recorded: {}", m.enum_layouts.len());
    }
    println!("\n================");
    println!("  A `Scalar`/`Flat` shape is derivable and can be stubbed faithfully.");
    println!("  `Top` or absent records NOTHING, and a numeric RANGE is never in the");
    println!("  bytecode at all -- `use host::f(Word, Word) -> Word` carries types only.");
}
