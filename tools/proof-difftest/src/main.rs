//! The differential test between increment A's extracted Rocq model and the shipping Rust
//! pipeline. The relations it checks are stated in `README.md` and in [`Relation`].
//!
//! The usage is this.
//!
//! ```text
//! keleusma-proof-difftest --model PATH --out DIR --seed S --cases N --min-both M
//!     [--break accept-recursion|rust-cost|swap-branches]
//! ```
//!
//! It exits with status 0 when every disagreement is explained and no stage collapsed,
//! and with status 1 otherwise. It prints the count of every stage and category.

mod core;
mod model;
mod rng;
mod rust_side;

use crate::core::CoreModule;
use crate::model::ModelResult;
use crate::rust_side::{Run, Stage};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

/// Stream cycles run per host.
const CYCLES: usize = 3;
/// The length of every reply string.
const REPLY_LEN: usize = 4096;
/// Generation limits per cycle, which keep the unary model fast and the replies sufficient.
const MAX_CYCLE_OPS: u64 = 2500;
const MAX_CYCLE_YIELDS: u64 = 200;
/// Attempts to draw a module within the limits before the harness gives up.
const MAX_DRAWS: usize = 64;

/// The relations. Each disagreement is counted under exactly one.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Relation {
    /// R1. Acceptance agrees, or differs only by a stated rule.
    Acceptance,
    /// R1b. The shipping verdict is what its documented rules predict.
    ShippingRules,
    /// R2. A module both sides accept has a finite shipping figure.
    Boundedness,
    /// R3. Every measured iteration of the Stream chunk is within the shipping figure.
    ShippingBound,
    /// R4. The two programs run in lockstep, and the model never charges more.
    Lockstep,
    /// R5. The model's own runs are within its figures.
    ModelBound,
    /// R6. For a main block without calls, the model's figures are within the shipping
    /// figure, each segment once and the junction twice.
    Figures,
}

impl Relation {
    const ALL: [Relation; 7] = [
        Relation::Acceptance,
        Relation::ShippingRules,
        Relation::Boundedness,
        Relation::ShippingBound,
        Relation::Lockstep,
        Relation::ModelBound,
        Relation::Figures,
    ];

    fn label(self) -> &'static str {
        match self {
            Relation::Acceptance => "R1 acceptance",
            Relation::ShippingRules => "R1b shipping-rules",
            Relation::Boundedness => "R2 boundedness",
            Relation::ShippingBound => "R3 shipping-bound",
            Relation::Lockstep => "R4 lockstep",
            Relation::ModelBound => "R5 model-bound",
            Relation::Figures => "R6 figure-relation",
        }
    }
}

#[derive(Default)]
struct Tally {
    counts: BTreeMap<String, u64>,
    unexplained: BTreeMap<Relation, Vec<String>>,
    checks: BTreeMap<Relation, u64>,
}

impl Tally {
    fn bump(&mut self, key: &str) {
        self.add(key, 1);
    }
    fn add(&mut self, key: &str, n: u64) {
        *self.counts.entry(key.to_string()).or_default() += n;
    }
    fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }
    fn check(&mut self, r: Relation, ok: bool, detail: impl FnOnce() -> String) {
        *self.checks.entry(r).or_default() += 1;
        if !ok {
            self.unexplained.entry(r).or_default().push(detail());
        }
    }
}

struct Args {
    model: PathBuf,
    out: PathBuf,
    seed: u64,
    cases: usize,
    min_both: u64,
    breaking: String,
}

fn parse_args() -> Result<Args, String> {
    let mut a = std::env::args().skip(1);
    let (mut model, mut out, mut seed, mut cases, mut min_both, mut breaking) =
        (None, None, None, None, None, String::new());
    while let Some(k) = a.next() {
        let mut v = || a.next().ok_or_else(|| format!("{k} needs a value"));
        match k.as_str() {
            "--model" => model = Some(PathBuf::from(v()?)),
            "--out" => out = Some(PathBuf::from(v()?)),
            "--seed" => seed = Some(v()?.parse().map_err(|e| format!("--seed: {e}"))?),
            "--cases" => cases = Some(v()?.parse().map_err(|e| format!("--cases: {e}"))?),
            "--min-both" => min_both = Some(v()?.parse().map_err(|e| format!("--min-both: {e}"))?),
            "--break" => breaking = v()?,
            _ => return Err(format!("unknown argument {k}")),
        }
    }
    if !["", "accept-recursion", "rust-cost", "swap-branches"].contains(&breaking.as_str()) {
        return Err(format!("unknown break mode {breaking}"));
    }
    let cases: usize = cases.ok_or("--cases is required")?;
    if cases == 0 {
        return Err(String::from("--cases must be positive"));
    }
    Ok(Args {
        model: model.ok_or("--model is required")?,
        out: out.ok_or("--out is required")?,
        seed: seed.ok_or("--seed is required")?,
        cases,
        min_both: min_both.ok_or("--min-both is required")?,
        breaking,
    })
}

struct Case {
    id: String,
    module: CoreModule,
    hosts: Vec<Vec<u8>>,
    topo: Option<CoreModule>,
}

fn replies(rng: &mut rng::Rng) -> Vec<Vec<u8>> {
    let zeros = vec![b'0'; REPLY_LEN];
    let ones = vec![b'1'; REPLY_LEN];
    let alt: Vec<u8> = (0..REPLY_LEN)
        .map(|i| if i % 2 == 0 { b'1' } else { b'0' })
        .collect();
    let random: Vec<u8> = (0..REPLY_LEN)
        .map(|_| if rng.chance(50) { b'1' } else { b'0' })
        .collect();
    vec![zeros, ones, alt, random]
}

fn case_text(id: &str, m: &CoreModule, hosts: &[Vec<u8>], out: &mut String) {
    let _ = writeln!(out, "case {id}\ncycles {CYCLES}");
    for h in hosts {
        let _ = writeln!(out, "host {}", String::from_utf8_lossy(h));
    }
    out.push_str(&core::lower(m));
    out.push_str("end\n");
}

fn generate(args: &Args) -> Result<(Vec<Case>, u64), String> {
    let mut rng = rng::Rng::new(args.seed);
    let mut cases = Vec::with_capacity(args.cases);
    let mut redraws = 0u64;
    for i in 0..args.cases {
        let mut drawn = None;
        for _ in 0..MAX_DRAWS {
            let m = core::generate(&mut rng);
            let (ops, ys) = core::cycle_size(&m);
            if ops <= MAX_CYCLE_OPS && ys <= MAX_CYCLE_YIELDS {
                drawn = Some(m);
                break;
            }
            redraws += 1;
        }
        let module = drawn.ok_or_else(|| format!("case {i}: no module within limits"))?;
        let topo = if core::has_forward_call(&module) {
            core::topo_reordered(&module)
        } else {
            None
        };
        cases.push(Case {
            id: format!("c{i}"),
            module,
            hosts: replies(&mut rng),
            topo,
        });
    }
    Ok((cases, redraws))
}

/// The lockstep relation and the model-bound relation for one run pair.
fn lockstep(
    t: &mut Tally,
    id: &str,
    h: usize,
    mr: &model::ModelRun,
    fig: &model::Figures,
    vr: &Run,
) {
    let ctx = || format!("{id} host {h}");
    let model_ok = mr.status == "ok" && mr.selfcheck == "ok";
    t.check(Relation::ModelBound, model_ok, || {
        format!(
            "{}: model run {} selfcheck {}",
            ctx(),
            mr.status,
            mr.selfcheck
        )
    });
    if !model_ok {
        return;
    }
    let within =
        mr.stream.iter().all(|&s| s <= fig.wcet) && mr.cycles.iter().all(|c| c.0 <= fig.wcmu);
    t.check(Relation::ModelBound, within, || {
        format!(
            "{}: model stream {:?} above wcet {}",
            ctx(),
            mr.stream,
            fig.wcet
        )
    });
    t.check(Relation::Lockstep, mr.outs == vr.outs, || {
        format!(
            "{}: outputs differ, model {:?} virtual machine {:?}",
            ctx(),
            mr.outs,
            vr.outs
        )
    });
    let model_yields: Vec<u64> = mr.cycles.iter().map(|c| c.1.len() as u64 - 1).collect();
    t.check(Relation::Lockstep, model_yields == vr.iter_yields, || {
        format!(
            "{}: yields per cycle differ, model {:?} virtual machine {:?}",
            ctx(),
            model_yields,
            vr.iter_yields
        )
    });
    let dominated =
        mr.stream.len() == vr.segs.len() && mr.stream.iter().zip(&vr.segs).all(|(m, v)| m <= v);
    t.check(Relation::Lockstep, dominated, || {
        format!(
            "{}: model segments {:?} not dominated by virtual machine segments {:?}",
            ctx(),
            mr.stream,
            vr.segs
        )
    });
    if dominated {
        t.bump("lockstep segments compared");
        t.add("lockstep segment pairs", mr.stream.len() as u64);
    }
}

/// Every relation for one case.
fn judge_case(
    t: &mut Tally,
    args: &Args,
    c: &Case,
    models: &BTreeMap<String, ModelResult>,
    log: &mut String,
) {
    let src = core::translate(&c.module, args.breaking == "swap-branches");
    t.bump("stage translated");
    let Some(mres) = models.get(&c.id) else {
        t.bump("model result missing");
        return;
    };
    t.bump("stage model-checked");
    let topo_res = models.get(&format!("{}.topo", c.id));
    let verdict = rust_side::judge(&src);
    if verdict.stage >= Stage::Load {
        t.bump("stage compiled");
    }
    let rust_ok = verdict.stage == Stage::Accepted;
    if rust_ok {
        t.bump("stage verified");
    }
    let cyclic = core::call_graph_cyclic(&c.module);
    let strict = core::strict_rules(&c.module);
    let detail = |what: &str| {
        format!(
            "{}: {what}\n  rust: {:?} {}\n  source: {}",
            c.id,
            verdict.stage,
            verdict.message,
            src.trim_end()
        )
    };

    // R1b. The shipping verdict is what its documented rules predict.
    let spec_ok = !cyclic && strict.is_empty();
    t.check(Relation::ShippingRules, spec_ok == rust_ok, || {
        detail(&format!(
            "documented rules predict accept={spec_ok}, strict rules {strict:?}, cyclic {cyclic}"
        ))
    });
    // A shipping rejection must carry the diagnostic of a rule predicted to fire.
    if !rust_ok {
        let mut fired: Vec<&str> = strict
            .iter()
            .flat_map(|r| r.diagnostics().iter().copied())
            .collect();
        if cyclic {
            fired.push("recursive call detected");
        }
        let reason = fired.iter().find(|d| verdict.message.contains(*d));
        t.check(Relation::ShippingRules, reason.is_some(), || {
            detail(&format!(
                "rejection reason not predicted, strict rules {strict:?}, cyclic {cyclic}"
            ))
        });
        if let Some(d) = reason {
            t.bump(&format!("shipping rejection by \"{d}\""));
        }
    }
    if cyclic {
        t.bump(match verdict.stage {
            Stage::Load => "recursion rejected at load only",
            Stage::Accepted => "recursion accepted",
            _ => "recursion rejected before load",
        });
    }

    // R1. Acceptance.
    let category = match (mres.accept, rust_ok) {
        (true, true) => "agree accept".to_string(),
        (false, false) => "agree reject".to_string(),
        (false, true) => {
            let topo_ok = topo_res.is_some_and(|r| r.accept);
            if !cyclic && core::has_forward_call(&c.module) && topo_ok {
                "explained model-only: index-order call rule".to_string()
            } else {
                "unexplained".to_string()
            }
        }
        (true, false) => {
            match strict
                .iter()
                .find(|r| r.diagnostics().iter().any(|d| verdict.message.contains(d)))
            {
                Some(r) => format!("explained shipping-only: {}", r.name()),
                None => "unexplained".to_string(),
            }
        }
    };
    t.check(Relation::Acceptance, category != "unexplained", || {
        detail(&format!(
            "model accept={} rust accept={rust_ok}, strict rules {strict:?}",
            mres.accept
        ))
    });
    t.bump(&format!("acceptance {category}"));
    if category.starts_with("explained") && log.len() < 200_000 {
        let _ = writeln!(
            log,
            "{category}\n  {}: rust {:?} {}\n  {}",
            c.id,
            verdict.stage,
            verdict.message,
            src.trim_end()
        );
    }

    if !rust_ok {
        return;
    }
    let module = verdict
        .module
        .as_ref()
        .expect("an accepted verdict carries its module");

    // The model result that describes this program, the reordered one when the order rule
    // alone made the model reject.
    let mview = if mres.accept {
        Some(mres)
    } else {
        topo_res.filter(|r| r.accept)
    };

    // R2. Boundedness, for a module the model judges bounded.
    let figure = rust_side::wcet(module, args.breaking == "rust-cost");
    if mview.is_some() {
        t.check(Relation::Boundedness, figure.is_ok(), || {
            detail(&format!("no shipping figure: {figure:?}"))
        });
    }
    let Ok(r) = figure else {
        t.bump("shipping figure missing");
        return;
    };
    let r = u64::from(r);
    t.bump("stage compared");
    if mview.is_some() {
        for f in core::features(&c.module) {
            t.bump(&format!("coverage, both accept: {f}"));
        }
    }

    // R6. The figure relation, for a main block without calls.
    if let Some(fig) = mview.and_then(|m| m.figures) {
        let holds = fig.first <= r && fig.middle <= r && fig.last < r && fig.wcet <= 2 * r;
        if core::main_calls(&c.module) {
            t.bump(if holds {
                "figure relation holds, main with calls"
            } else {
                "figure relation fails, main with calls (shallow calls)"
            });
        } else {
            t.check(Relation::Figures, holds, || {
                detail(&format!(
                    "model f={} m={} l={} wcet={} against shipping figure {r}",
                    fig.first, fig.middle, fig.last, fig.wcet
                ))
            });
        }
    }

    // R3 and R4 on every host.
    let mut most = 0u64;
    for (h, replies) in c.hosts.iter().enumerate() {
        let run = match rust_side::measure(module, CYCLES, replies) {
            Ok(run) => run,
            Err(e) => {
                t.check(Relation::ShippingBound, false, || {
                    detail(&format!("host {h}: run failed: {e}"))
                });
                continue;
            }
        };
        t.bump("runs measured");
        most = most.max(run.iter_shallow.iter().copied().max().unwrap_or(0));
        for (k, &s) in run.iter_shallow.iter().enumerate() {
            t.check(Relation::ShippingBound, s <= r, || {
                detail(&format!(
                    "host {h} iteration {k}: {s} operations in the Stream chunk, figure {r}"
                ))
            });
        }
        for &d in &run.iter_deep {
            t.bump(if d <= r {
                "iterations within figure counting callees"
            } else {
                "iterations above figure counting callees (shallow calls)"
            });
        }
        if let Some(m) = mview {
            let fig = m.figures.expect("an accepted model result carries figures");
            match m.runs.get(h) {
                Some(mr) => lockstep(t, &c.id, h, mr, &fig, &run),
                None => t.check(Relation::Lockstep, false, || {
                    detail(&format!("host {h}: no model run"))
                }),
            }
        }
    }
    // Tightness, reported and not checked. The four hosts need not reach the worst path.
    t.bump(if most == r {
        "tightness: some run attains the shipping figure"
    } else if 2 * most >= r {
        "tightness: best run within half of the shipping figure"
    } else {
        "tightness: every run below half of the shipping figure"
    });
}

/// The reproduction mode, `--source FILE`. It judges one Keleusma source, prints the
/// stage, the declared header figure, the unit-cost figure, and the operations of each of
/// three iterations under a host that always replies zero, in the Stream chunk and in all.
fn reproduce(path: &str) -> i32 {
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("difftest: read {path}: {e}");
            return 2;
        }
    };
    let v = rust_side::judge(&src);
    println!("stage {:?} {}", v.stage, v.message);
    let Some(m) = v.module.as_ref() else { return 0 };
    println!("declared header wcet_cycles {}", m.wcet_cycles);
    println!("unit-cost figure {:?}", rust_side::wcet(m, false));
    if v.stage == Stage::Accepted {
        match rust_side::measure(m, CYCLES, &vec![b'0'; REPLY_LEN]) {
            Ok(r) => println!(
                "measured, Stream chunk {:?}, all chunks {:?}",
                r.iter_shallow, r.iter_deep
            ),
            Err(e) => println!("measure failed: {e}"),
        }
    }
    0
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() == 3 && argv[1] == "--source" {
        std::process::exit(reproduce(&argv[2]));
    }
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("difftest: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = std::fs::create_dir_all(&args.out) {
        eprintln!("difftest: create {}: {e}", args.out.display());
        std::process::exit(2);
    }
    let (cases, redraws) = match generate(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("difftest: {e}");
            std::process::exit(1);
        }
    };
    let mut text = String::new();
    for c in &cases {
        case_text(&c.id, &c.module, &c.hosts, &mut text);
        if let Some(t) = &c.topo {
            case_text(&format!("{}.topo", c.id), t, &c.hosts, &mut text);
        }
    }
    let case_file = args.out.join("cases.txt");
    if let Err(e) = std::fs::write(&case_file, &text) {
        eprintln!("difftest: write {}: {e}", case_file.display());
        std::process::exit(2);
    }
    let extra: Vec<&str> = if args.breaking == "accept-recursion" {
        vec!["--break", "accept-recursion"]
    } else {
        vec![]
    };
    let models = match model::run(&args.model, &case_file, &extra) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("difftest: {e}");
            std::process::exit(1);
        }
    };

    let mut t = Tally::default();
    let mut log = String::new();
    for c in &cases {
        t.bump("stage generated");
        if c.topo.is_some() {
            t.bump("reordered variants checked by the model");
        }
        judge_case(&mut t, &args, c, &models, &mut log);
    }
    let _ = std::fs::write(args.out.join("explained.txt"), &log);

    let mode = if args.breaking.is_empty() {
        "none"
    } else {
        args.breaking.as_str()
    };
    println!(
        "difftest: seed {} cases {} break {mode} redraws {redraws}",
        args.seed, args.cases
    );
    for (k, v) in &t.counts {
        println!("count  {v:>7}  {k}");
    }
    let mut failed = false;
    let mut report = String::new();
    for r in Relation::ALL {
        let n = t.unexplained.get(&r).map_or(0, Vec::len);
        let checks = t.checks.get(&r).copied().unwrap_or(0);
        println!("check  {checks:>7}  {}, unexplained {n}", r.label());
        if let Some(v) = t.unexplained.get(&r) {
            failed = true;
            for d in v {
                let _ = writeln!(report, "[{}] {d}", r.label());
            }
            for d in v.iter().take(2) {
                println!("  example [{}] {d}", r.label());
            }
        }
    }
    let _ = std::fs::write(args.out.join("disagreements.txt"), &report);

    // Stage collapse. Every case must reach translation and the model, and enough must be
    // accepted by both sides for the bound relations to mean anything.
    let n = args.cases as u64;
    let floors = [
        ("stage generated", n),
        ("stage translated", n),
        ("stage model-checked", n),
        ("acceptance agree accept", args.min_both),
        ("runs measured", 4 * args.min_both),
        ("lockstep segments compared", 4 * args.min_both),
    ];
    for (k, floor) in floors {
        if t.get(k) < floor {
            println!("collapse  {k}: {} below the floor {floor}", t.get(k));
            failed = true;
        }
    }
    let unexplained: usize = t.unexplained.values().map(Vec::len).sum();
    let verdict = if failed { "FAIL" } else { "PASS" };
    println!("difftest: {unexplained} unexplained disagreement(s), result {verdict}");
    std::process::exit(i32::from(failed));
}
