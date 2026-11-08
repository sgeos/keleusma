//! The model side. The harness writes every case to one file, runs the extracted model
//! once over it, and parses what the model prints. The format is the one
//! `model/model_main.ml` reads and writes.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Stdio};

/// The model's figures for an accepted module.
#[derive(Debug, Clone, Copy)]
pub struct Figures {
    pub wcet: u64,
    pub wcmu: u64,
    /// The first, largest middle and last segment bound of the main block.
    pub first: u64,
    pub middle: u64,
    pub last: u64,
    /// The interpreter fuel the checker computed, which the model driver already used.
    #[allow(dead_code)]
    pub fuel: u64,
}

/// One run of the extracted interpreter.
#[derive(Debug, Default, Clone)]
pub struct ModelRun {
    pub status: String,
    /// Per cycle, the arena and the segments.
    pub cycles: Vec<(u64, Vec<u64>)>,
    pub outs: Vec<u64>,
    pub stream: Vec<u64>,
    pub selfcheck: String,
}

/// The model's result for one case.
#[derive(Debug, Default, Clone)]
pub struct ModelResult {
    pub accept: bool,
    pub figures: Option<Figures>,
    pub runs: Vec<ModelRun>,
}

fn nums(ws: &[&str]) -> Result<Vec<u64>, String> {
    ws.iter()
        .map(|w| w.parse::<u64>().map_err(|e| format!("bad number {w}: {e}")))
        .collect()
}

/// Run the model binary at `model` over the case file `cases`, passing `extra` arguments.
pub fn run(
    model: &Path,
    cases: &Path,
    extra: &[&str],
) -> Result<BTreeMap<String, ModelResult>, String> {
    let input = std::fs::File::open(cases).map_err(|e| format!("open {}: {e}", cases.display()))?;
    let out = Command::new(model)
        .args(extra)
        .stdin(Stdio::from(input))
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("spawn {}: {e}", model.display()))?;
    if !out.status.success() {
        return Err(format!("model exited with {}", out.status));
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

fn parse(text: &str) -> Result<BTreeMap<String, ModelResult>, String> {
    let mut all = BTreeMap::new();
    let mut id: Option<String> = None;
    let mut cur = ModelResult::default();
    for (n, line) in text.lines().enumerate() {
        let ws: Vec<&str> = line.split_whitespace().collect();
        let bad = || format!("model output line {}: {line}", n + 1);
        match ws.as_slice() {
            ["case", i] => {
                id = Some((*i).to_string());
                cur = ModelResult::default();
            }
            ["verdict", "accept"] => cur.accept = true,
            ["verdict", "reject"] => cur.accept = false,
            ["fig", rest @ ..] => {
                let v = nums(rest)?;
                let [wcet, wcmu, first, middle, last, fuel] = v[..] else {
                    return Err(bad());
                };
                cur.figures = Some(Figures {
                    wcet,
                    wcmu,
                    first,
                    middle,
                    last,
                    fuel,
                });
            }
            ["host", _] => cur.runs.push(ModelRun::default()),
            ["run", s] => cur.runs.last_mut().ok_or_else(bad)?.status = (*s).to_string(),
            ["cyc", arena, rest @ ..] => {
                let a = arena.parse::<u64>().map_err(|_| bad())?;
                cur.runs
                    .last_mut()
                    .ok_or_else(bad)?
                    .cycles
                    .push((a, nums(rest)?));
            }
            ["outs", rest @ ..] => cur.runs.last_mut().ok_or_else(bad)?.outs = nums(rest)?,
            ["stream", rest @ ..] => cur.runs.last_mut().ok_or_else(bad)?.stream = nums(rest)?,
            ["selfcheck", s] => cur.runs.last_mut().ok_or_else(bad)?.selfcheck = (*s).to_string(),
            ["end"] => {
                let i = id.take().ok_or_else(bad)?;
                all.insert(i, std::mem::take(&mut cur));
            }
            _ => return Err(bad()),
        }
    }
    Ok(all)
}
