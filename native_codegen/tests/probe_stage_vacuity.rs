//! **Do the nine self-hosted stages inside `corpus_differential` actually run?**
//!
//! `corpus_differential` reports 40 modules executed and agreeing, and nine of
//! the ten stage sources under `src/selfhost/kel` are inside that number. It
//! drives every module with an **all-zero shared data segment**.
//!
//! `lexer.kel`'s own header states the input convention:
//!
//! > The host places the source in the `shared data` byte array `src.bytes` and
//! > its length in `src.len`, then drives the loop with resume.
//!
//! A zero segment therefore presents a source of length zero. The suspicion this
//! probe exists to settle is that each stage takes an immediate end-of-input
//! exit, so both sides agree on a path that compiles nothing.
//!
//! # The instrument, and why not the obvious one
//!
//! The first attempt measured CHUNK COVERAGE with the VM's breakpoint facility,
//! arming op 0 of every chunk. **That does not work, and the reason is a defect
//! worth reporting rather than a mistake here**: `resume_from_breakpoint` calls
//! `run()` without rebinding the shared buffer that `call_with_shared` binds and
//! clears around each entry, so the first shared read after a breakpoint stop
//! reaches `read_shared_from_buffer`'s `.expect(...)` and **panics**. Every stage
//! declares shared data, so every stage panics. See
//! `the_breakpoint_facility_panics_on_any_shared_data_module` below, which pins
//! it as a minimal reproducer.
//!
//! What is used instead is the stage's own observable output: **the sequence of
//! yielded values**. That needs nothing from the VM, and for a tokenizer it is a
//! more direct answer than coverage would have been — `lexer.kel` documents 62 as
//! end of source and 63 as "no complete token this iteration", so an immediate
//! exit is legible in the output itself.
//!
//! # This is a REPORT, with two assertions
//!
//! The counts are printed, not asserted, because they will move when the
//! differential is repaired. The two assertions are the properties that would
//! make the measurement meaningless if they failed.
use keleusma::bytecode::{BlockType, Module, SlotVisibility, Value};
use keleusma::vm::{
    Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for, shared_data_bytes_for,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};

/// Matches `corpus_differential::TICKS`, so this measures the same run.
const TICKS: i64 = 60;

fn arena_for(m: &Module) -> keleusma_arena::Arena {
    const HOST_MARGIN: usize = 4 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

/// Byte offset of the shared slot named `suffix`, or of its element zero.
///
/// Slot names are qualified (`src.len`) and **an array slot is expanded to one
/// slot per element** (`src.bytes[0]`, `src.bytes[1]`, ...), which is why a plain
/// suffix match on `bytes` finds nothing. Both forms are accepted here.
///
/// `shared_layout` is parallel to the SHARED-slot prefix of `slots`, so the index
/// has to be counted among shared slots rather than taken from the `slots`
/// position.
fn shared_offset(m: &Module, suffix: &str) -> Option<u32> {
    let dl = m.data_layout.as_ref()?;
    let scalar = format!(".{suffix}");
    let element0 = format!(".{suffix}[0]");
    let mut shared_ix = 0usize;
    for s in &dl.slots {
        if s.visibility != SlotVisibility::Shared {
            continue;
        }
        if s.name == suffix || s.name.ends_with(&scalar) || s.name.ends_with(&element0) {
            return dl.shared_layout.get(shared_ix).map(|l| l.offset);
        }
        shared_ix += 1;
    }
    None
}

/// Write `src` into the module's `bytes` array and its length into `len`.
///
/// Returns false when the module has no such pair, which is how a stage that
/// does not take source input is distinguished from one that does.
fn seed_source(m: &Module, buf: &mut [u8], src: &str) -> bool {
    let (Some(len_off), Some(bytes_off)) = (shared_offset(m, "len"), shared_offset(m, "bytes"))
    else {
        return false;
    };
    let (len_off, bytes_off) = (len_off as usize, bytes_off as usize);
    let n = src.len();
    if bytes_off + n > buf.len() || len_off + 8 > buf.len() {
        return false;
    }
    // A `Word` slot is eight little-endian bytes at the host target.
    buf[len_off..len_off + 8].copy_from_slice(&(n as u64).to_le_bytes());
    buf[bytes_off..bytes_off + n].copy_from_slice(src.as_bytes());
    true
}

struct Run {
    yields: Vec<i64>,
    outcome: String,
}

/// Drive the module exactly as `corpus_differential` does, recording each
/// yielded value rather than only the last.
fn drive(m: &Module, seed: Option<&str>) -> Result<Run, String> {
    let arena = arena_for(m);
    let mut vm = match Vm::new(m.clone(), &arena) {
        Ok(v) => v,
        Err(e) => return Err(format!("VM refuses to load: {e:?}")),
    };

    let entry = m.entry_point.ok_or("no entry point")?;
    let n = m.chunks[entry].param_count as usize;
    let is_stream = m.chunks[entry].block_type == BlockType::Stream;
    let vals: Vec<Value> = if is_stream && n == 1 {
        vec![Value::Int(0)]
    } else {
        (0..n).map(|i| Value::Int((i as i64 + 1) * 3 + 1)).collect()
    };

    let mut shared = vec![0u8; shared_data_bytes_for(m)];
    if let Some(s) = seed
        && !seed_source(m, &mut shared, s)
    {
        // Capped: an array slot expands per element, so a stage's shared slot
        // list runs to hundreds of thousands of names.
        let names: Vec<&str> = m
            .data_layout
            .as_ref()
            .map(|dl| {
                dl.slots
                    .iter()
                    .filter(|s| s.visibility == SlotVisibility::Shared)
                    .map(|s| s.name.as_str())
                    .take(6)
                    .collect()
            })
            .unwrap_or_default();
        return Err(format!("no len/bytes pair; shared slots begin {names:?}"));
    }

    let mut yields = Vec::new();
    let mut state = match vm.call_with_shared(&mut shared, &vals) {
        Ok(s) => s,
        Err(e) => return Err(format!("VM refuses to run: {e:?}")),
    };
    let mut outcome = String::from("ran to the tick limit");

    for t in 0..TICKS {
        match state {
            VmState::Yielded(ref v) => yields.push(scalar(v)),
            VmState::Finished(ref v) => {
                yields.push(scalar(v));
                outcome = format!("FINISHED after {t} ticks");
                break;
            }
            VmState::Reset => {}
            ref other => {
                outcome = format!("unexpected state: {other:?}");
                break;
            }
        }
        if !is_stream {
            outcome = "not a stream; one call".into();
            break;
        }
        state = match vm.resume_with_shared(&mut shared, Value::Int(t + 1)) {
            Ok(s) => s,
            Err(e) => {
                outcome = format!("resume failed at tick {t}: {e:?}");
                break;
            }
        };
    }

    Ok(Run { yields, outcome })
}

fn scalar(v: &Value) -> i64 {
    match v {
        Value::Int(x) => *x,
        Value::Byte(b) => i64::from(*b),
        Value::Bool(b) => i64::from(*b),
        Value::Unit => 0,
        _ => i64::MIN,
    }
}

fn distinct(v: &[i64]) -> usize {
    let mut s: Vec<i64> = v.to_vec();
    s.sort_unstable();
    s.dedup();
    s.len()
}

fn stage_sources() -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new("../src/selfhost/kel");
    let mut out: Vec<_> = std::fs::read_dir(dir)
        .expect("read src/selfhost/kel")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    out.sort();
    out
}

fn module_of(p: &std::path::Path) -> Option<Module> {
    let src = std::fs::read_to_string(p).ok()?;
    tokenize(&src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

/// A source that exercises several token classes, so a real run cannot produce
/// one repeated value the way an empty one does.
const SEED: &str = "fn add(a: Word, b: Word) -> Word { let c = a + b; c }";

#[test]
fn how_far_does_each_stage_get_on_the_differentials_own_input() {
    println!("\n================ STAGE OUTPUT: all-zero shared segment vs. seeded source");
    println!("  the differential drives the left column. {TICKS} ticks each.\n");
    println!(
        "  {:<24} {:>7} {:>26}   {:>7} {:>26}",
        "stage", "distinct", "first yields (zero input)", "distinct", "first yields (seeded)"
    );

    let mut measured = 0usize;
    let mut single_valued = 0usize;
    for p in stage_sources() {
        let name = p.file_name().unwrap().to_str().unwrap().to_string();
        let Some(m) = module_of(&p) else {
            println!("  {name:<24} rejected by the REFERENCE compiler");
            continue;
        };
        let zero = drive(&m, None);
        let seeded = drive(&m, Some(SEED));

        let render = |r: &Result<Run, String>| match r {
            Ok(run) => {
                let head: Vec<String> = run.yields.iter().take(6).map(|v| v.to_string()).collect();
                (
                    format!("{}", distinct(&run.yields)),
                    format!(
                        "{} {}",
                        head.join(","),
                        if run.yields.len() > 6 { ".." } else { "" }
                    ),
                )
            }
            Err(_) => ("-".into(), "see below".into()),
        };
        let (dz, hz) = render(&zero);
        let (ds, hs) = render(&seeded);
        println!("  {name:<24} {dz:>7} {hz:>26}   {ds:>7} {hs:>26}");
        if let Err(e) = &seeded {
            println!("        seeded: {e}");
        }

        if let Ok(run) = &zero {
            measured += 1;
            if distinct(&run.yields) <= 1 {
                single_valued += 1;
            }
            if let Ok(s) = &seeded
                && run.yields == s.yields
            {
                println!("        ^ seeding changed NOTHING: this stage ignores the source input");
            }
        }
        if let Ok(run) = &zero
            && run.outcome != "ran to the tick limit"
        {
            println!("        zero-input outcome: {}", run.outcome);
        }
    }

    println!(
        "\n  {single_valued} of {measured} stages yield a SINGLE repeated value on the \
         differential's input."
    );
    println!("  A tokenizer emitting one value for sixty ticks has reached end of source");
    println!("  immediately. `lexer.kel` documents 62 as EOF and 63 as PENDING.");
    assert!(
        measured > 0,
        "no stage was measured; the probe asserts nothing"
    );
}

/// The property that would invalidate the comparison above.
///
/// `corpus_differential` returns an asymmetric `stub_value` from every native;
/// this probe registers none at all. If a stage declared a native, the two
/// harnesses could drive it down different paths and what is measured here would
/// not be what the differential runs. None does, so they cannot diverge.
#[test]
fn no_stage_declares_a_native_so_the_two_harnesses_cannot_diverge() {
    for p in stage_sources() {
        let name = p.file_name().unwrap().to_str().unwrap();
        let Some(m) = module_of(&p) else { continue };
        let declared: Vec<&String> = m.native_names.iter().filter(|n| !n.is_empty()).collect();
        assert!(
            declared.is_empty(),
            "{name} declares natives {declared:?}; this probe registers no natives while \
             `corpus_differential` returns `stub_value` from each, so the two harnesses \
             could drive it down different paths"
        );
    }
}

/// **A `v0.2.3` defect, pinned here because it blocked the instrument above.**
///
/// `Vm::set_breakpoint` is documented as working on any module. It does not work
/// on a module that declares shared data: `resume_from_breakpoint` calls `run()`
/// without rebinding the buffer that `call_with_shared` binds and clears around
/// each entry, so the first shared read after the stop hits
/// `read_shared_from_buffer`'s `.expect("... called with an active buffer")` and
/// **panics**. A panic is not a `VmError`, so a host cannot handle it.
///
/// `src/vm.rs` belongs to the `v0.2.3` line and is untouched. This test asserts
/// the CURRENT behaviour so that a repair there fails it loudly rather than
/// leaving a stale claim here.
#[test]
fn the_breakpoint_facility_panics_on_any_shared_data_module() {
    let src = "\
shared data s { n: Word }
loop main(resume: Word) -> Word {
  s.n = s.n + 1;
  yield s.n
}
";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    assert!(
        shared_data_bytes_for(&m) > 0,
        "the reproducer must declare shared data or it tests nothing"
    );

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let arena = arena_for(&m);
        let mut vm = Vm::new(m.clone(), &arena).expect("vm");
        // Chunk 0 op 0 is entered on the first call, so this stops immediately.
        vm.set_breakpoint(0, 0);
        let mut shared = vec![0u8; shared_data_bytes_for(&m)];
        let st = vm
            .call_with_shared(&mut shared, &[Value::Int(0)])
            .expect("call");
        assert!(
            matches!(st, VmState::BreakpointHit { .. }),
            "expected a breakpoint stop, got {st:?}"
        );
        vm.resume_from_breakpoint()
    }));

    assert!(
        outcome.is_err(),
        "`resume_from_breakpoint` no longer panics on a shared-data module. If the \
         `v0.2.3` line has repaired the missing buffer rebind, DELETE this test and \
         restore the chunk-coverage instrument in this file, which is strictly better \
         than the yield-sequence one it was replaced by."
    );
}
