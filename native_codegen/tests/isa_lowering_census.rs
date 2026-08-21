//! **HOW MANY OF THE 66 OPCODES DOES THE BACKEND ACTUALLY LOWER?**
//!
//! Workstream A's milestone is that the whole language lowers, and **this line
//! has never had a denominator for it.** Three measurements exist and not one
//! answers the question:
//!
//! * `isa_coverage_census` reports which opcodes the CORPUS EMITS (64 of 66).
//!   That is a fact about the reference compiler, not about this backend.
//! * `backend_support_census` partitions **fifteen hand-picked probes**. Fifteen
//!   was chosen as the set that looked hard, so its denominator is a judgement
//!   rather than the instruction set.
//! * `spike_corpus_coverage` reports every corpus opcode INSTANCE lowering,
//!   which the coverage census already warns says nothing about an opcode the
//!   corpus never emits.
//!
//! This census reads the instruction set out of `src/bytecode.rs` and asks, of
//! every declared opcode, whether the backend was ever OBSERVED to lower it.
//!
//! # The evidence is POSITIVE, per-OP, and taken from the lowering itself
//!
//! It would be far easier to ask whether a chunk containing the opcode lowered
//! cleanly. **That inference is unsound, and the unsoundness is not theoretical**
//! — see `how_much_of_the_corpus_does_the_lowering_step_over` below, which
//! measures it. `lower_chunk_body` skips every op in code no edge reaches, and
//! the reference compiler emits such code routinely, so an opcode occurring only
//! in a dead region of an otherwise clean chunk was never lowered at all. A
//! chunk-level census would report it as supported.
//!
//! So the backend records what it visited, and [`module_lowered_op_indices`]
//! hands back those indices per chunk. Nothing here infers.
//!
//! # What a LOWERS verdict does NOT mean
//!
//! **Not that the emitted code is correct.** It means the backend produced
//! something for that opcode. Correctness is `corpus_differential`'s question,
//! and the distinction is not pedantry on this line: `FixedMul` sat in the
//! supported column for a whole increment while its saturating clamp was
//! unreached by any program.
//!
//! # Why the answer is a FOUR-way partition
//!
//! An opcode inside a chunk that refused proves nothing, because **the chunk may
//! have refused on a different opcode entirely**. Calling that "refused" is
//! precisely the mistake this line already made once, when a first-failure
//! report was read as a count and three refusals turned out to be ten.
//!
//! * **LOWERS** — the lowering visited it and emitted code.
//! * **NAMED REFUSED** — a refusal message names it. Positive evidence.
//! * **UNPROVEN** — emitted by the corpus, but never visited and never named.
//!   **Neither supported nor refused**, under its own heading so it cannot be
//!   read as either.
//! * **NO CORPUS WITNESS** — not emitted anywhere. `isa_coverage_census` owns
//!   this set; it is echoed so the columns sum to the instruction set.
//!
//! The first two columns can in principle OVERLAP, because the operand type is
//! invisible at the lowering site: one opcode may be supported for one
//! representation and refused for another. The overlap is reported rather than
//! collapsed into whichever column reads better.
//!
//! **It is currently EMPTY, and that refutes a plausible guess.** `Add` looks
//! like the generic addition that lowers for `Word` and refuses for `Byte`. It
//! is not — a `Word` addition compiles to `CheckedAdd`, and `Op::Add` is emitted
//! for `Byte` and `Float` only, so it is refused outright with no supported
//! representation. The overlap column is what made that visible, and a control
//! in this file was written on the wrong guess before the column corrected it.
//!
//! # On reading a refusal message for an opcode name
//!
//! The catch-all arm refuses with the op's own `Debug` rendering, so its leading
//! identifier is the variant name. Several hand-written refusals also open with
//! the opcode they could not lower. Both are counted, and **they do not mean the
//! same thing**: the first says no lowering arm exists, the second says this
//! INSTANCE carried something the arm could not handle. The overlap column is
//! where that difference becomes visible, and the printout says so.
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerError, LowerOptions, module_lowered_op_indices};
use std::collections::BTreeSet;

/// The corpus directories, matching `isa_coverage_census` and the differential.
const CORPUS_DIRS: [&str; 4] = [
    "examples/scripts",
    "src/selfhost/kel",
    "examples/rtos/scripts",
    "compiler/kel",
];

fn source_for(p: &std::path::Path) -> Option<String> {
    let src = std::fs::read_to_string(p).ok()?;
    let is_rtos = p.components().any(|c| c.as_os_str() == "rtos");
    let is_prelude = p.file_name().is_some_and(|n| n == "prelude.kel");
    if is_rtos && !is_prelude {
        let prelude = std::fs::read_to_string("../examples/rtos/scripts/prelude.kel").ok()?;
        return Some(format!("{prelude}\n{src}"));
    }
    Some(src)
}

fn all_compiling_modules() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = CORPUS_DIRS.iter().map(|d| root.join(d)).collect();
    let mut paths = Vec::new();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel") {
            paths.push(p);
        }
    }
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(src) = source_for(&p) else { continue };
        let Some(m) = compiled(&src) else { continue };
        out.push((name, m));
    }
    out
}

fn compiled(src: &str) -> Option<Module> {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

/// The `Op` variant names, read from the crate source rather than listed here.
///
/// **Restated rather than shared with `isa_coverage_census`.** An integration
/// test binary cannot import another's helpers without a shared module, and the
/// restatement is the choice `delegated_subject_census` documents for its own
/// duplicated constant: if the two copies diverge, two censuses disagree loudly
/// about what the instruction set is, which is the failure worth surfacing. Both
/// assert a plausible floor, so a broken extraction cannot pass quietly in
/// either.
fn declared_isa() -> BTreeSet<String> {
    let src = std::fs::read_to_string("../src/bytecode.rs").expect("read bytecode.rs");
    let start = src.find("pub enum Op {").expect("find `pub enum Op`");
    let body = &src[start..];
    let end = body.find("\n}").expect("find the enum's close");
    let body = &body[..end];
    let mut out = BTreeSet::new();
    for line in body.lines() {
        let t = line.trim_start();
        if line.starts_with("    ")
            && !line.starts_with("     ")
            && t.starts_with(|c: char| c.is_ascii_uppercase())
        {
            let name: String = t
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if !name.is_empty() {
                out.insert(name);
            }
        }
    }
    out
}

/// The leading identifier of a `Debug` rendering, which is the variant name.
fn head(s: &str) -> String {
    s.chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect()
}

/// What one module contributes: opcodes proved LOWERED, opcodes NAMED by a
/// refusal, and the number of ops the lowering stepped over in clean chunks.
///
/// Factored out so the controls below run the identical query against synthetic
/// modules. A control exercising a different code path would validate nothing.
fn evidence_from(
    m: &Module,
    isa: &BTreeSet<String>,
) -> (BTreeSet<String>, BTreeSet<String>, usize) {
    let (refusals, visits) = module_lowered_op_indices(m, LowerOptions::default());

    let mut named_refused = BTreeSet::new();
    for (_, e) in &refusals {
        if let LowerError::UnsupportedOp(msg) = e {
            let h = head(msg);
            if isa.contains(&h) {
                named_refused.insert(h);
            }
        }
    }

    let mut lowered = BTreeSet::new();
    let mut skipped = 0usize;
    for (chunk, seen) in m.chunks.iter().zip(visits.iter()) {
        let Some(seen) = seen else { continue };
        skipped += chunk.ops.len() - seen.len();
        for &i in seen {
            lowered.insert(head(&format!("{:?}", chunk.ops[i])));
        }
    }
    (lowered, named_refused, skipped)
}

/// **CONTROL, must-fire.** The query has to be able to report a refusal.
///
/// A zero from a broken query is indistinguishable from a zero from a clean one,
/// and this line has four recorded instances of a check that passed while it
/// could not have failed. `Add` on two `Byte`s is the refusal
/// `backend_support_census` already pins, reached here through the identical
/// call path the census uses.
#[test]
fn the_query_reports_a_known_refusal() {
    let isa = declared_isa();
    let m = compiled("fn p(a: Byte, b: Byte) -> Byte { a + b }\nfn main() -> Word { 0 }")
        .expect("the probe must compile");
    assert!(
        m.chunks
            .iter()
            .any(|c| c.ops.iter().any(|o| format!("{o:?}").starts_with("Add"))),
        "the probe does not emit Add, so it is a BROKEN PROBE and this control \
         would pass without testing anything"
    );
    let (lowered, named, _) = evidence_from(&m, &isa);
    assert!(
        named.contains("Add"),
        "no refusal named Add, so the NAMED REFUSED column built from this query \
         means nothing. Named: {named:?}"
    );
    assert!(
        !lowered.contains("Add"),
        "Add was reported as LOWERED by a module whose only Add is the refused \
         one, so the positive column is crediting ops the lowering never reached"
    );
}

/// **CONTROL, must-not-fire.** A query that named everything would pass the
/// control above and make the refused column a copy of the instruction set.
#[test]
fn the_query_reports_no_refusal_for_a_module_that_lowers() {
    let isa = declared_isa();
    let m = compiled("fn main() -> Word { 1 }\n").expect("compiles");
    let (lowered, named, _) = evidence_from(&m, &isa);
    assert!(
        named.is_empty(),
        "a module the backend lowers cleanly produced refusals: {named:?}"
    );
    assert!(
        !lowered.is_empty(),
        "a module the backend lowers cleanly produced NO lowered opcodes, so the \
         positive side of this query is broken"
    );
}

/// **CONTROL: the positive evidence is per-CHUNK, not per-module.**
///
/// A module holding one refusing chunk and one clean chunk must record a verdict
/// for EACH. Without this, a query that discarded any module containing any
/// refusal would pass both controls above while silently shrinking the LOWERS
/// column toward the refusal-free modules only.
///
/// **This control was written wrong first, and the measurement said so.** It
/// asserted that the `Word` chunk contributes an `Add`, on the assumption that
/// `Add` is the generic addition and refuses only for `Byte`. It is not: a
/// `Word` addition compiles to `CheckedAdd`, and `Op::Add` is emitted for `Byte`
/// and `Float` ONLY. The census's own overlap column is the evidence — it is
/// EMPTY, where the mistaken reading predicts `Add` in it. The claim is now
/// positional, which needs no assumption about which opcode a source form emits.
#[test]
fn a_clean_chunk_still_counts_inside_a_module_that_has_a_refusing_chunk() {
    let m = compiled(
        "fn bad(a: Byte, b: Byte) -> Byte { a + b }\n\
         fn good(a: Word, b: Word) -> Word { a + b }\n\
         fn main() -> Word { 0 }",
    )
    .expect("compiles");
    let (_, visits) = module_lowered_op_indices(&m, LowerOptions::default());
    assert_eq!(
        visits.len(),
        m.chunks.len(),
        "the per-chunk record is not parallel to the chunks, so no index into it \
         means what it appears to"
    );

    let verdict = |name: &str| {
        m.chunks
            .iter()
            .position(|c| c.name == name)
            .map(|i| visits[i].is_some())
    };
    assert_eq!(
        verdict("bad"),
        Some(false),
        "the Byte chunk did not refuse, so this control is not exercising the \
         mixed case it exists for"
    );
    assert_eq!(
        verdict("good"),
        Some(true),
        "the clean chunk was recorded as refused, so either the whole module was \
         discarded because ONE chunk refused, or the per-chunk record is \
         misaligned"
    );
}

/// **IS THE DEAD-CODE HOLE REAL?** The measurement this census's design rests on.
///
/// The cheap version of this census would ask whether a chunk containing an
/// opcode lowered cleanly. That is unsound *if and only if* the lowering ever
/// steps over ops in a chunk it otherwise completes. This asks how often it
/// does, over the real corpus, and — the part that actually matters — whether
/// any opcode's ONLY appearances are in stepped-over positions.
///
/// **A zero in the second figure would not make the cheap version correct**, only
/// currently indistinguishable, and it would be one `break` away from wrong. The
/// figures are printed rather than pinned for that reason: what is asserted is
/// that the question was asked over a real corpus.
#[test]
fn how_much_of_the_corpus_does_the_lowering_step_over() {
    let isa = declared_isa();
    let corpus = all_compiling_modules();
    assert!(
        corpus.len() > 50,
        "only {} modules compiled, so this is reading the wrong tree",
        corpus.len()
    );

    let mut skipped_total = 0usize;
    let mut clean_ops_total = 0usize;
    let mut chunks_with_skips = 0usize;
    // Opcodes seen in a clean chunk at an index the lowering did NOT visit.
    let mut in_skipped_position: BTreeSet<String> = BTreeSet::new();
    let mut lowered_anywhere: BTreeSet<String> = BTreeSet::new();

    for (_, m) in &corpus {
        let (_, visits) = module_lowered_op_indices(m, LowerOptions::default());
        for (chunk, seen) in m.chunks.iter().zip(visits.iter()) {
            let Some(seen) = seen else { continue };
            let seen_set: BTreeSet<usize> = seen.iter().copied().collect();
            clean_ops_total += chunk.ops.len();
            let skipped = chunk.ops.len() - seen_set.len();
            skipped_total += skipped;
            if skipped > 0 {
                chunks_with_skips += 1;
            }
            for (i, op) in chunk.ops.iter().enumerate() {
                let h = head(&format!("{op:?}"));
                if seen_set.contains(&i) {
                    lowered_anywhere.insert(h);
                } else {
                    in_skipped_position.insert(h);
                }
            }
        }
    }

    // The opcodes a chunk-level census would have wrongly credited.
    let only_in_skipped: Vec<&String> = in_skipped_position
        .iter()
        .filter(|o| !lowered_anywhere.contains(*o) && isa.contains(*o))
        .collect();

    println!("\n================ HOW MUCH DOES THE LOWERING STEP OVER?");
    println!("  ops in cleanly-lowered chunks : {clean_ops_total}");
    println!("  ops STEPPED OVER (unreachable): {skipped_total}");
    println!("  chunks with at least one skip : {chunks_with_skips}");
    println!(
        "  opcodes appearing in a skipped position : {}",
        in_skipped_position.len()
    );
    println!("  opcodes appearing ONLY in skipped positions: {only_in_skipped:?}");
    println!(
        "\n  The last line is the size of the error a CHUNK-LEVEL census would\n  \
         have made. Even at zero the cheap inference stays unsound -- it would\n  \
         be one `break` away from crediting a lowering that does not exist."
    );
    println!("================\n");

    assert!(
        clean_ops_total > 0,
        "no chunk lowered cleanly, so this walk measured nothing"
    );

    // **THE GUARD ON THE INSTRUMENT ITSELF, and it is not decoration.**
    //
    // Mutation-verified: recording the op index BEFORE the dead check instead of
    // after — which is precisely the naive chunk-level claim — takes this figure
    // to 0, moves `Reset` out of UNPROVEN into LOWERS, and reports the backend
    // lowering 57 of 66 rather than 56. **Every test in this file still passed.**
    // Without this assertion a disarmed sink is silent, and the census quietly
    // reverts to the inference it was built to avoid.
    //
    // A FAILURE HERE IS NEWS BEFORE IT IS A DEFECT. Two things reach it: the
    // sink stopped honouring the dead-code skip (a defect, and the likely one),
    // or the reference compiler stopped emitting unreachable code (a real change
    // worth recording). Establish WHICH before editing anything, and do not
    // delete this to make the suite green.
    assert!(
        skipped_total > 0,
        "the lowering stepped over NO ops in {clean_ops_total} lowered ops. \
         Either the visit sink no longer honours the dead-code skip, in which \
         case every LOWERS verdict in this file is now the unsound chunk-level \
         inference, or the reference compiler stopped emitting unreachable code. \
         The last measured value was 441 skipped ops across 140 chunks."
    );
}

/// **WHY `Reset` IS NEVER VISITED, MEASURED RATHER THAN READ OFF THE SOURCE.**
///
/// The corpus walk above reports `Reset` appearing only in stepped-over
/// positions. That is a fact about the corpus. **The mechanism behind it is
/// structural**, and this test pins the mechanism so the finding does not read
/// as an accident of which programs happen to be in the tree.
///
/// `Op::Stream` and `Op::Reset` are REFUSED outright, deliberately: a divergent
/// `loop fn` lowered natively would spin with no way for the host to stop it.
/// The single exception is the degenerate-stream transform, where both lower to
/// nothing. **But that transform is exactly what makes the `Reset` unreachable**
/// — it turns the tail `Yield` into the return, so every op after it, the
/// `Reset` included, is in code no edge reaches.
///
/// So the only configuration in which `Reset` HAS a lowering is the
/// configuration in which it is never REACHED. Its half of that shared match arm
/// is never taken. `Op::Stream`, which shares the arm, is the first op of the
/// chunk and is visited normally — which is what keeps this from being a claim
/// about the arm being dead.
#[test]
fn a_degenerate_stream_visits_its_stream_op_and_never_its_reset() {
    let m = compiled(
        "yield tick(resume: Word) -> Word { yield resume + 1 }\n\
         loop main(resume: Word) -> Word { yield resume + 1 }\n",
    )
    .expect("the degenerate stream must compile");

    let entry = m.entry_point.expect("a loop entry");
    let ops = &m.chunks[entry].ops;
    let ix = |name: &str| {
        ops.iter()
            .position(|o| format!("{o:?}").starts_with(name))
            .unwrap_or_else(|| panic!("the entry emits no {name}; ops are {ops:?}"))
    };
    let (stream_ix, reset_ix) = (ix("Stream"), ix("Reset"));

    let (refusals, visits) = module_lowered_op_indices(&m, LowerOptions::default());
    let seen = visits[entry].as_ref().unwrap_or_else(|| {
        panic!(
            "the degenerate entry REFUSED, so this test is not exercising the \
             transform it exists for: {refusals:?}"
        )
    });

    assert!(
        seen.contains(&stream_ix),
        "the lowering never visited Op::Stream either, so `Reset` being unvisited \
         says nothing specific -- the whole chunk would be unreachable"
    );
    assert!(
        !seen.contains(&reset_ix),
        "THE LOWERING VISITED Op::Reset. That is NEWS: the degenerate transform \
         no longer makes the trailing Reset unreachable, so `Reset` may now be \
         genuinely lowered and the census row saying otherwise needs re-measuring \
         rather than deleting."
    );
}

/// The census.
///
/// **Reported, not pinned, except for the floors and the partition.** The
/// columns move with the corpus and with the instruction set, and pinning them
/// would fail on ordinary growth. What is asserted is that both sides came from
/// real inputs and that the partition is TOTAL — every declared opcode lands in
/// exactly one column, so an opcode cannot fall out of the accounting while the
/// columns still look sane.
#[test]
fn how_many_isa_opcodes_does_the_backend_lower() {
    let isa = declared_isa();
    // **Non-vacuity first.** An extraction that matched nothing would report
    // every opcode unlowered, which reads as a dramatic finding and is a broken
    // parse.
    assert!(
        isa.len() >= 60,
        "extracted only {} Op variants from bytecode.rs; the parse is broken and \
         every figure below is an artefact of it",
        isa.len()
    );

    let corpus = all_compiling_modules();
    assert!(
        corpus.len() > 50,
        "only {} modules compiled, so this census is reading the wrong tree",
        corpus.len()
    );

    let mut emitted: BTreeSet<String> = BTreeSet::new();
    let mut lowered: BTreeSet<String> = BTreeSet::new();
    let mut named_refused: BTreeSet<String> = BTreeSet::new();
    let mut modules_lowering_nothing = 0usize;

    for (_, m) in &corpus {
        for c in &m.chunks {
            for op in &c.ops {
                emitted.insert(head(&format!("{op:?}")));
            }
        }
        let (l, n, _) = evidence_from(m, &isa);
        if l.is_empty() && !m.chunks.is_empty() {
            modules_lowering_nothing += 1;
        }
        lowered.extend(l);
        named_refused.extend(n);
    }

    let no_witness: Vec<&String> = isa.difference(&emitted).collect();
    let both: Vec<&String> = isa
        .iter()
        .filter(|o| lowered.contains(*o) && named_refused.contains(*o))
        .collect();
    let refused_only: Vec<&String> = isa
        .iter()
        .filter(|o| named_refused.contains(*o) && !lowered.contains(*o))
        .collect();
    let lowered_only: Vec<&String> = isa
        .iter()
        .filter(|o| lowered.contains(*o) && !named_refused.contains(*o))
        .collect();
    let unproven: Vec<&String> = isa
        .iter()
        .filter(|o| emitted.contains(*o) && !lowered.contains(*o) && !named_refused.contains(*o))
        .collect();

    println!("\n================ ISA LOWERING CENSUS");
    println!("  corpus directories         : {CORPUS_DIRS:?}");
    println!("  modules compiled           : {}", corpus.len());
    println!("  opcodes declared in the ISA: {}", isa.len());
    println!("  opcodes the corpus emits   : {}", emitted.len());
    println!();
    println!(
        "  THE BACKEND LOWERS         : {} of {}",
        lowered.len(),
        isa.len()
    );
    println!("    lowers, never refused    : {}", lowered_only.len());
    println!("    lowers AND refuses ({})   : {both:?}", both.len());
    if both.is_empty() {
        println!("      -- EMPTY, and that is a finding rather than a formality: no");
        println!("         opcode is supported for one representation and refused for");
        println!("         another. `Add` looks like it should be here and is not --");
        println!("         a Word addition compiles to CheckedAdd, so Op::Add has no");
        println!("         supported representation at all.");
    } else {
        println!("      -- the operand type is invisible at the lowering site, so these");
        println!("         are supported for one representation and refused for another.");
        println!("         Not a contradiction; a property of the instruction set.");
    }
    println!();
    println!(
        "  NAMED REFUSED, never lowers ({}): {refused_only:?}",
        refused_only.len()
    );
    println!(
        "  UNPROVEN ({}) -- emitted, never visited, never named: {unproven:?}",
        unproven.len()
    );
    println!("      -- unproven FROM THE CORPUS, which is the only population this");
    println!("         census reads. An opcode here occurs only inside chunks that");
    println!("         refused on something else, so the corpus never put it to the");
    println!("         backend. `backend_support_census` drives hand-built probes and");
    println!("         reaches some of these; the two are complementary and neither");
    println!("         subsumes the other.");
    println!(
        "  NO CORPUS WITNESS ({})       : {no_witness:?}",
        no_witness.len()
    );
    println!();
    println!("  modules where nothing lowered: {modules_lowering_nothing}");
    println!(
        "\n  A LOWERS VERDICT IS NOT A CORRECTNESS CLAIM. It says the backend\n  \
         emitted code for the opcode, not that the code is right. That is\n  \
         `corpus_differential`'s question, and this line has already shipped an\n  \
         opcode whose saturating clamp no program reached."
    );
    println!("================\n");

    // Non-vacuity in both directions: a walk that read nothing satisfies an
    // emptiness check on either column alone.
    assert!(
        !lowered.is_empty(),
        "no opcode was proved lowered at all, so the positive side read nothing"
    );
    assert!(
        !emitted.is_empty(),
        "no opcode was emitted at all, so the corpus walk read nothing"
    );

    // **THE PARTITION IS TOTAL.** Without this an opcode could drop out of the
    // accounting entirely and every column would still look plausible.
    let accounted = lowered_only.len() + both.len() + refused_only.len() + unproven.len();
    assert_eq!(
        accounted + no_witness.len(),
        isa.len(),
        "the columns do not partition the ISA: {accounted} accounted plus {} \
         unwitnessed against {} declared",
        no_witness.len(),
        isa.len()
    );

    // The extraction must cover what the corpus emits, or NO CORPUS WITNESS is
    // measuring this file's parser rather than the backend.
    let unextracted: Vec<&String> = emitted.difference(&isa).collect();
    assert!(
        unextracted.is_empty(),
        "these opcodes occur in compiled modules but were not extracted from \
         bytecode.rs, so the ISA side is incomplete and no column can be \
         trusted: {unextracted:?}"
    );
}
