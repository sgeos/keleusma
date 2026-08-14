//! **Step one of the `sret` repair: what does per-CALL-SITE reservation cost?**
//!
//! `sret` reserves a return slot per call site, not per live value: two sites
//! calling composite-returning chunks get two slots even when their lifetimes
//! never overlap. `NATIVE_COMPOSITE_RETURN_ABI.md` made measuring this step one
//! and it has never been taken.
//!
//! The operator authorised the SHAPE, not an unbounded cost. If the figure blows
//! up, that is a reason to revisit before building.
use keleusma::bytecode::{Module, Op, WireShape};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

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

/// Bytes `sret` would add: one slot per call site whose CALLEE returns a flat
/// composite, sized by that callee's declared return shape.
fn sret_bytes(m: &Module) -> (usize, usize) {
    let mut bytes = 0usize;
    let mut sites = 0usize;
    for c in &m.chunks {
        for op in &c.ops {
            if let Op::Call(idx, _) = op
                && let Some(sig) = m.signatures.get(usize::from(*idx))
                && let WireShape::Flat { size, .. } = sig.ret
            {
                bytes += size as usize;
                sites += 1;
            }
        }
    }
    (bytes, sites)
}

#[test]
fn probe_what_sret_costs_in_region_bytes() {
    let mut rows = Vec::new();
    let (mut tot_now, mut tot_add, mut tot_sites) = (0usize, 0usize, 0usize);
    let mut worst: Option<(String, usize, usize)> = None;

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
        let now: usize = m
            .chunks
            .iter()
            .map(|c| keleusma_native::region::plan_chunk_region(c).bytes as usize)
            .sum();
        let (add, sites) = sret_bytes(&m);
        tot_now += now;
        tot_add += add;
        tot_sites += sites;
        if add > 0 {
            rows.push((name.clone(), now, add, sites));
            let ratio = if now == 0 {
                usize::MAX
            } else {
                (now + add) * 100 / now.max(1)
            };
            if worst.as_ref().is_none_or(|(_, _, w)| ratio > *w) {
                worst = Some((name, add, ratio));
            }
        }
    }

    println!("================ SRET COST: per-call-site return slots");
    println!(
        "  {:28} {:>9} {:>9} {:>7}",
        "module", "region", "+sret", "sites"
    );
    for (n, now, add, s) in &rows {
        println!("  {n:28} {now:>9} {add:>9} {s:>7}");
    }
    println!("\n  modules needing any sret slot : {}", rows.len());
    println!("  composite-returning call sites: {tot_sites}");
    println!("  region bytes today            : {tot_now}");
    println!("  region bytes added by sret    : {tot_add}");
    if tot_now > 0 {
        println!(
            "  growth                        : {:.1}%",
            100.0 * tot_add as f64 / tot_now as f64
        );
    }
    if let Some((n, add, r)) = &worst {
        println!("  worst single module           : {n} (+{add} bytes, {r}% of its current)");
    }
    println!("\n  A slot is reserved per SITE, not per live value, so this is an");
    println!("  upper bound on what liveness-aware reuse could achieve.");
    println!("================");
    assert!(
        tot_now > 0,
        "no region bytes measured at all; the probe is vacuous"
    );
}
