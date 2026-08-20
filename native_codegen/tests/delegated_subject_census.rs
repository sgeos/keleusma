//! **Does the corpus contain a real delegated-suspension subject?**
//!
//! Delegated suspension is the case where a `Stream` entry hands its whole body
//! to a callee that yields, rather than yielding itself. `resume_after_enter`
//! writes slot 0 of the ENTRY frame whatever frame actually suspended, so the
//! native lowering has to model a suspension that is invisible in the entry's
//! own ops. The transform for it is implemented and sits behind
//! `LowerOptions::delegated_suspension`, off by default.
//!
//! **It has had no real subject since `aaa87a01`.** `codegen.kel` was the only
//! one; the `v0.2.3` line refactored it at this line's request, `emit_next`
//! becoming a plain `fn`, and since then the only witness has been a synthetic
//! module. The handoff has carried "whether that changed is a MEASUREMENT nobody
//! has made" ever since.
//!
//! **The reason to make it now** is that the precondition moved. All twelve
//! stage sources carry a `loop main` entry on this tree, where ten of twelve did
//! before the `v0.2.3` absorption, and `wire.kel` and `verify_types.kel` are the
//! two that changed shape.
//!
//! # This file asks; it does not enable
//!
//! Every query here goes through `delegated_suspension_subject`, which is a view
//! of the predicate `lower_module` itself uses. Nothing here sets the flag, and a
//! module reported as a subject is not thereby lowered any differently.
//!
//! # Why the control is not optional
//!
//! The expected answer is zero, and **a zero from a broken query is
//! indistinguishable from a zero from a clean one**. This line has four recorded
//! instances of a check that passed while it could not have failed, including a
//! `Trap` observable that passed while none of its three subjects emitted the
//! opcode. So `the_census_query_finds_a_known_subject` runs the same query
//! against a module built to qualify, and the census asserts it examined a
//! non-empty corpus. Without both, the census below establishes nothing.
use keleusma::bytecode::{Module, Op};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::delegated_suspension_subject;

/// The same shape `probe_delegated_suspension.rs` uses: a `Stream` entry whose
/// whole body is a tail call to a `Reentrant` chunk yielding in tail position.
///
/// **Restated here deliberately rather than shared.** If the two copies ever
/// diverge, two tests disagree loudly about what a subject is, which is the
/// failure this file is designed to make visible. A shared constant would let a
/// silent edit disarm the control in both places at once.
const KNOWN_SUBJECT: &str = "\
private data st { n: Word }

fn step() -> Word {
  st.n = st.n + 1;
  st.n
}

yield emit(resume: Word) -> Word {
  yield step()
}

loop main(resume: Word) -> Word {
  emit(resume)
}
";

fn module_of_source(src: &str) -> Option<Module> {
    compile(&parse(&tokenize(src).ok()?).ok()?).ok()
}

/// Mirrors `probe_nesting_and_breaks::source_for`, which mirrors the
/// differential's. The rtos scripts do not compile without their prelude.
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

/// The four-directory corpus, the one the differential and the nesting probe
/// both walk. Named here because a census over a different set answers a
/// different question, and the last increment published a figure that confused
/// exactly these two populations.
fn corpus() -> Vec<(String, Module)> {
    let root = std::path::Path::new("..");
    let mut stack: Vec<std::path::PathBuf> = [
        "examples/scripts",
        "src/selfhost/kel",
        "examples/rtos/scripts",
        "compiler/kel",
    ]
    .iter()
    .map(|d| root.join(d))
    .collect();
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
        let Some(m) = module_of_source(&src) else {
            continue;
        };
        out.push((name, m));
    }
    out
}

/// **THE CONTROL. Read this before believing the census.**
///
/// The query must be able to return a subject. If this fails, the census's zero
/// means nothing whatever, and no amount of green elsewhere repairs that.
#[test]
fn the_census_query_finds_a_known_subject() {
    let m = module_of_source(KNOWN_SUBJECT).expect("the known subject must compile");
    let got = delegated_suspension_subject(&m);
    assert!(
        got.is_some(),
        "the query did not recognise a module built to qualify, so any zero it \
         reports over the corpus is meaningless. Entry ops: {:?}",
        m.entry_point.map(|e| &m.chunks[e].ops)
    );
    let (entry, callee, call_ix) = got.unwrap();
    // The shape, not merely the verdict: a verdict-only control passes for a
    // predicate that returns Some unconditionally.
    assert_eq!(
        m.entry_point,
        Some(entry),
        "the plan named a non-entry chunk"
    );
    assert_ne!(entry, callee, "the entry cannot delegate to itself");
    assert!(
        matches!(m.chunks[entry].ops.get(call_ix), Some(Op::Call(_, _))),
        "the plan's call index does not point at a Call"
    );
    assert!(
        m.chunks[callee].ops.iter().any(|o| matches!(o, Op::Yield)),
        "the named callee does not yield, so it cannot be what suspends"
    );
}

/// **A must-NOT-fire companion.** A predicate that says yes to everything would
/// pass the control above and make the census a list of every module.
#[test]
fn the_census_query_rejects_a_plain_function_module() {
    let m = module_of_source("fn main() -> Word { 1 }\n").expect("compiles");
    assert!(
        delegated_suspension_subject(&m).is_none(),
        "a module with no stream entry was reported as delegating a suspension"
    );
}

/// The census itself. Prints every module and asserts only that the walk was not
/// vacuous; the COUNT is reported rather than pinned, because pinning it would
/// make a future real subject look like a regression.
#[test]
fn which_corpus_modules_delegate_a_suspension() {
    let corpus = corpus();
    println!("\n================ DELEGATED-SUSPENSION CENSUS");
    println!("  modules examined : {}", corpus.len());

    let mut subjects = Vec::new();
    let mut stream_entries = 0usize;
    for (name, m) in &corpus {
        let is_stream = m
            .entry_point
            .is_some_and(|e| m.chunks[e].block_type == keleusma::bytecode::BlockType::Stream);
        if is_stream {
            stream_entries += 1;
        }
        if let Some((entry, callee, call_ix)) = delegated_suspension_subject(m) {
            subjects.push(format!(
                "{name} (entry {entry} `{}` -> callee {callee} `{}` at op {call_ix})",
                m.chunks[entry].name, m.chunks[callee].name
            ));
        }
    }

    // The denominator matters: a census over modules that mostly have no stream
    // entry cannot find a delegated suspension and would report zero either way.
    println!("  with a Stream entry : {stream_entries}");
    println!("  DELEGATING SUBJECTS : {}", subjects.len());
    for s in &subjects {
        println!("     {s}");
    }
    if subjects.is_empty() {
        println!(
            "\n  ZERO. Validated by `the_census_query_finds_a_known_subject`, which\n  \
             runs this same query against a module built to qualify and finds it.\n  \
             So the corpus still routes around delegated suspension, and the\n  \
             transform behind LowerOptions::delegated_suspension still has no\n  \
             witness but the synthetic one."
        );
    }

    assert!(
        corpus.len() > 50,
        "only {} modules were examined, so this census is reading the wrong tree \
         and would report zero vacuously",
        corpus.len()
    );
    assert!(
        stream_entries > 0,
        "no module in the corpus has a Stream entry, so a delegated suspension \
         was impossible by construction and the zero says nothing"
    );
}
