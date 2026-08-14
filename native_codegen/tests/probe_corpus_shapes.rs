//! What would it take to EXECUTE every module that lowers?
//!
//! 57 of 58 lower; 14 are executed. This asks what the other 43 require, so the
//! generic harness is built against measured shapes rather than assumed ones.
use keleusma::bytecode::{Op, SlotVisibility};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

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

#[test]
fn probe_what_the_untested_modules_need() {
    let mut rows = Vec::new();
    let mut arity_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut natives_all: BTreeMap<String, std::collections::BTreeSet<u8>> = BTreeMap::new();
    let mut blocked = Vec::new();

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
            blocked.push((name, "reference rejected".to_string()));
            continue;
        };
        if !keleusma_native::module_refusals(&m, keleusma_native::LowerOptions::default())
            .is_empty()
        {
            blocked.push((name, "backend refuses".to_string()));
            continue;
        }
        let Some(e) = m.entry_point else {
            blocked.push((name, "no entry point".into()));
            continue;
        };
        let ec = &m.chunks[e];
        let mut nats: Vec<String> = Vec::new();
        for c in &m.chunks {
            for op in &c.ops {
                if let Op::CallVerifiedNative(i, n) | Op::CallExternalNative(i, n) = op {
                    let nm = m.native_names.get(*i as usize).cloned().unwrap_or_default();
                    natives_all.entry(nm.clone()).or_default().insert(n & 0x7F);
                    nats.push(nm);
                }
            }
        }
        nats.sort();
        nats.dedup();
        let shared = keleusma::vm::shared_data_bytes_for(&m);
        let privs = m
            .data_layout
            .as_ref()
            .map(|d| {
                d.slots
                    .iter()
                    .filter(|s| s.visibility == SlotVisibility::Private)
                    .count()
            })
            .unwrap_or(0);
        let region: usize = m
            .chunks
            .iter()
            .map(|c| keleusma_native::region::plan_chunk_region(c).bytes as usize)
            .sum();
        *arity_hist.entry(ec.param_count as usize).or_default() += 1;
        rows.push((
            name,
            e,
            format!("{:?}", ec.block_type),
            ec.param_count,
            nats.len(),
            shared,
            privs,
            region,
        ));
    }

    println!("================ MODULES THAT LOWER: {}", rows.len());
    println!(
        "  {:26} {:>5} {:>9} {:>4} {:>5} {:>7} {:>5} {:>7}",
        "module", "entry", "type", "par", "nats", "shared", "priv", "region"
    );
    for (n, e, t, p, na, s, pv, r) in &rows {
        println!("  {n:26} {e:>5} {t:>9} {p:>4} {na:>5} {s:>7} {pv:>5} {r:>7}");
    }
    println!("\n  entry source-arity histogram: {arity_hist:?}");
    println!(
        "  DISTINCT NATIVES ACROSS THE CORPUS: {}",
        natives_all.len()
    );
    let multi: Vec<_> = natives_all.iter().filter(|(_, v)| v.len() > 1).collect();
    println!("  natives with MORE THAN ONE arity (harness hazard): {multi:?}");
    println!("\n  NOT LOWERING ({}):", blocked.len());
    for (n, why) in &blocked {
        println!("    {n:26} {why}");
    }
    println!("================");
    assert!(
        rows.len() > 40,
        "only {} modules lower; the probe is too thin",
        rows.len()
    );
}
