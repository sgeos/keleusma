//! **WHICH OPCODES CAN THE BACKEND ACTUALLY LOWER?**
//!
//! `examples/scripts/opcode_witness.kel` produced three `UnsupportedOp`
//! refusals, and **three is a floor rather than a count**: `module_refusals`
//! reports the FIRST failure per chunk, so anything after `Add` inside
//! `byte_mix` is invisible. Workstream A's milestone is that the whole language
//! lowers, and the size of that job was unknown.
//!
//! # One opcode per FUNCTION, which is what makes this a partition
//!
//! A refusal names its chunk. Put each construct in its own function and one
//! refusal can no longer mask another. The witness file demonstrated this
//! without being designed for it: `grid_at` and `checked_ratio` did not refuse,
//! which is positive evidence that `BoundsCheck`, `CheckedDiv` and `CheckedMod`
//! lower.
//!
//! # A "supported" verdict is only as good as the emission behind it
//!
//! A snippet that quietly compiles to something else — or to nothing — would sit
//! in the supported column having proved nothing. So every case asserts the
//! opcode is PRESENT in the compiled module before its refusal status is read,
//! and a case whose opcode is absent is reported as a BROKEN PROBE rather than
//! as support.
//!
//! # What a pass here does NOT mean
//!
//! That the backend accepted the opcode. **Not that the emitted code is
//! correct** — `lower_module` returning `Ok` is a fact about the compiler, not
//! about the program. Correctness is the differential's job.
use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, module_refusals};
use std::collections::BTreeSet;

/// Does the lowering actually REACH `opcode` in this module?
///
/// The difference between "accepted" and "never seen" is invisible to a refusal
/// check, and collapsing them puts unvisited opcodes in the supported column.
fn visits_opcode(m: &Module, opcode: &str) -> bool {
    let (_, visits) = keleusma_native::module_lowered_op_indices(m, LowerOptions::default());
    m.chunks.iter().enumerate().any(|(ci, c)| {
        let Some(seen) = visits.get(ci).and_then(|v| v.as_ref()) else {
            return false;
        };
        c.ops
            .iter()
            .enumerate()
            .any(|(i, o)| format!("{o:?}").starts_with(opcode) && seen.contains(&i))
    })
}

/// `(opcode, probe function name, source)`. The probe function isolates the
/// opcode; `main` exists only so the module has an entry.
const PROBES: &[(&str, &str, &str)] = &[
    // **The last opcode neither census could classify.** It lands in NEVER
    // VISITED rather than in either verdict column, which is the honest answer:
    // the corpus emits it only inside chunks that refuse on something else, and
    // a probe emits it in a position the lowering steps over.
    (
        "Reset",
        "p",
        "loop p(resume: Word) -> Word { yield 1 }\nfn main() -> Word { 0 }",
    ),
    (
        "Add",
        "p",
        "fn p(a: Byte, b: Byte) -> Byte { a + b }\nfn main() -> Word { 0 }",
    ),
    (
        "Sub",
        "p",
        "fn p(a: Byte, b: Byte) -> Byte { a - b }\nfn main() -> Word { 0 }",
    ),
    (
        "Mul",
        "p",
        "fn p(a: Byte, b: Byte) -> Byte { a * b }\nfn main() -> Word { 0 }",
    ),
    (
        "Neg",
        "p",
        "fn p(a: Byte) -> Byte { -a }\nfn main() -> Word { 0 }",
    ),
    (
        "IntToFloat",
        "p",
        "fn p(w: Word) -> Float { w as Float }\nfn main() -> Word { 0 }",
    ),
    (
        "FloatToInt",
        "p",
        "fn p(f: Float) -> Word { f as Word }\nfn main() -> Word { 0 }",
    ),
    (
        "WordToFixed",
        "p",
        "fn p(w: Word) -> Fixed<16> { w as Fixed<16> }\nfn main() -> Word { 0 }",
    ),
    (
        "FixedToWord",
        "p",
        "fn p(a: Fixed<16>) -> Word { a as Word }\nfn main() -> Word { 0 }",
    ),
    (
        "FixedMul",
        "p",
        "fn p(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a * b }\nfn main() -> Word { 0 }",
    ),
    (
        "FixedDiv",
        "p",
        "fn p(a: Fixed<16>, b: Fixed<16>) -> Fixed<16> { a / b }\nfn main() -> Word { 0 }",
    ),
    (
        "CheckedDiv",
        "p",
        "fn p(a: Word, b: Word) -> Word { a / b { ok(v) => v, zero_divisor(n) => 0, } }\nfn main() -> Word { 0 }",
    ),
    (
        "CheckedMod",
        "p",
        "fn p(a: Word, b: Word) -> Word { a % b { ok(v) => v, zero_divisor(n) => 0, } }\nfn main() -> Word { 0 }",
    ),
    (
        "BoundsCheck",
        "p",
        "shared data g { c: [[Word; 4]; 4] }\nfn p(i: Word, j: Word) -> Word { g.c[i][j] }\nfn main() -> Word { 0 }",
    ),
    (
        "CallExternalNative",
        "p",
        "use external host::t\nfn p(w: Word) -> Word { host::t(w) }\nfn main() -> Word { 0 }",
    ),
    // A CONTROL that must land in the supported column: plain Word addition is
    // lowered by every module in the corpus. If this reports unsupported the
    // instrument is broken, not the backend.
    (
        "CheckedAdd",
        "p",
        "fn p(a: Word, b: Word) -> Word { a + b }\nfn main() -> Word { 0 }",
    ),
];

fn module_of(src: &str) -> Result<Module, String> {
    let t = tokenize(src).map_err(|e| format!("lex {e:?}"))?;
    let a = parse(&t).map_err(|e| format!("parse {e:?}"))?;
    compile(&a).map_err(|e| format!("compile {e:?}"))
}

fn ops_in(m: &Module) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in &m.chunks {
        for op in &c.ops {
            let d = format!("{op:?}");
            s.insert(d.split('(').next().unwrap_or(&d).to_string());
        }
    }
    s
}

#[test]
fn which_opcodes_does_the_backend_refuse() {
    let mut supported: Vec<&str> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    let mut broken: Vec<String> = Vec::new();

    let mut never_visited: Vec<&str> = Vec::new();
    for (opcode, func, src) in PROBES {
        let m = match module_of(src) {
            Ok(m) => m,
            Err(e) => {
                broken.push(format!("{opcode}: probe does not compile: {e}"));
                continue;
            }
        };
        // **The emission check comes FIRST.** Without it a snippet that compiles
        // to something else sits in the supported column having proved nothing.
        if !ops_in(&m).contains(*opcode) {
            broken.push(format!(
                "{opcode}: probe compiles but does NOT emit it; emitted {:?}",
                ops_in(&m)
            ));
            continue;
        }
        let refusals = module_refusals(&m, LowerOptions::default());
        // **A MODULE-LEVEL REFUSAL COUNTS.** Matching only the probe's chunk
        // name missed refusals reported against `MODULE_LEVEL_REFUSAL`, and the
        // miss produced a FALSE SUPPORTED verdict -- the flattering direction.
        // Measured when the float signature guard landed: `IntToFloat` and
        // `FloatToInt` moved from refused to "lowers" while the backend had
        // gained no float support whatever.
        let hit = refusals
            .iter()
            .find(|(chunk, _)| chunk == func || chunk == keleusma_native::MODULE_LEVEL_REFUSAL);
        match hit {
            Some((where_, err)) => refused.push(format!("{opcode}  ({where_}: {err:?})")),
            // **NO REFUSAL IS NOT SUPPORT.** An opcode the lowering never VISITS
            // raises no refusal either, and under the old rule landed here having
            // proved nothing. `Op::Reset` is exactly that shape: the
            // degenerate-stream transform reaches `Stream` and steps over
            // `Reset`, so a stream probe emits it and the backend never sees it.
            //
            // **This file had already been burned in the same direction**: its
            // comment above records `IntToFloat` and `FloatToInt` moving "from
            // refused to lowers while the backend had gained no float support
            // whatever". Different cause, same flattering column.
            None if !visits_opcode(&m, opcode) => never_visited.push(*opcode),
            None => supported.push(opcode),
        }
    }

    println!("\n================ BACKEND OPCODE SUPPORT");
    println!("  probes            : {}", PROBES.len());
    if !never_visited.is_empty() {
        println!(
            "  NEVER VISITED     : {}  <- emitted by the probe, never reached by",
            never_visited.len()
        );
        println!("                       the lowering, so NO verdict is available. These");
        println!("                       are NOT supported and NOT refused.");
        for o in &never_visited {
            println!("     {o}");
        }
    }
    println!("  LOWERS            : {}", supported.len());
    for s in &supported {
        println!("     {s}");
    }
    println!("  REFUSED           : {}", refused.len());
    for r in &refused {
        println!("     {r}");
    }
    if !broken.is_empty() {
        println!("  BROKEN PROBES     : {}", broken.len());
        for b in &broken {
            println!("     {b}");
        }
    }
    println!(
        "\n  A LOWERS verdict means the backend ACCEPTED the opcode. It does NOT\n  \
         mean the emitted code is correct -- `lower_module` returning Ok is a\n  \
         fact about the compiler, not the program. Correctness is the\n  \
         differential's job.\n  \
         \n  \
         Reported, not pinned: this partition moves as the backend gains support."
    );
    println!("================\n");

    assert!(
        broken.is_empty(),
        "some probes do not emit the opcode they claim, so their column is \
         meaningless:\n  {}",
        broken.join("\n  ")
    );
    // The control. If plain Word addition does not lower, the instrument is
    // wrong and every other verdict here is worthless.
    assert!(
        supported.contains(&"CheckedAdd"),
        "the control opcode does not lower, so this instrument is broken rather \
         than the backend"
    );
}

/// **THE FLOAT SIGNATURE ROUTE IS OPEN, and this test has now flipped twice.**
///
/// Its first form asserted that a float-typed function LOWERS, recording that
/// the absence of float miscompiles rested on no float OPERATION being
/// supported. Its second form asserted the opposite: `lower_module` refused any
/// module with a `Float` in a chunk signature, because the lowered entry took
/// `i64` where the C ABI passes a double, so a host would have read an FP
/// register the backend never wrote. Each form's own message directed the next
/// rewrite, and this is the second one.
///
/// The entry ABI now exists for the one float width that is built: a float
/// parameter or return takes a real floating-point position in the declared
/// function type, converted at the four boundary points. So the census claim is
/// that the float identity module produces NO refusal. **Acceptance is not the
/// evidence of correctness** — the convention itself is exercised by calling
/// the JIT-ed symbol with a runtime `f64` in `entry_abi_float.rs`, which is
/// where the register-agreement claim lives.
///
/// What remains refused is a float of a width this lowering does not emit; that
/// guard reads the module's `float_bits_log2`, and this build's `Float` is
/// 8 bytes, so the refusal path is unreachable by compiling a program here.
#[test]
fn an_eight_byte_float_signature_lowers_with_no_refusal() {
    let identity = module_of("fn p(a: Float) -> Float { a }\nfn main() -> Word { 0 }")
        .expect("the float identity compiles");
    assert_eq!(
        1u32 << identity.float_bits_log2 >> 3,
        8,
        "this build's Float is not 8 bytes, so the signature route should be \
         refused and this test's claim does not describe it"
    );
    let refusals = module_refusals(&identity, LowerOptions::default());
    println!("\n  FLOAT SIGNATURE REFUSALS: {refusals:?}");

    assert!(
        refusals.is_empty(),
        "a float-typed function refuses again. Either the entry ABI was removed, \
         in which case this test should return to pinning the refusal, or \
         something else now rejects the module first: {refusals:?}"
    );
}

/// **`module_refusals` REPORTS MODULE-LEVEL REFUSALS, and did not before.**
///
/// It collects per-CHUNK refusals through a sink and rejected the whole module
/// by RETURNING; the return value was discarded. So a module the backend cannot
/// lower at all produced an EMPTY vector — indistinguishable from a module it
/// lowers perfectly. Callers, including the corpus differential's exemption
/// classification, read that emptiness as acceptance.
///
/// Two guards had this shape before the float one: the native-symbol collision
/// check and the word-width check.
///
/// **The subject changed once.** This test originally used a float signature,
/// which was the newest module-level refusal at the time; the entry ABI then
/// opened that route, so the subject is now the word-width guard. It is the one
/// module-level refusal reachable without a float, and it cannot be reached by
/// compiling a program in this build — the compiler stamps the build's own
/// width — so the module's declared width is overwritten after compilation.
#[test]
fn a_module_level_refusal_is_visible_to_module_refusals() {
    let mut m = module_of("fn main() -> Word { 0 }").expect("compiles");
    assert_eq!(
        m.word_bits_log2, 6,
        "this build's Word is not 64 bits, so the width mutation below would be \
         a no-op and this test would measure nothing"
    );
    m.word_bits_log2 = 5;
    let reported = module_refusals(&m, LowerOptions::default());
    assert!(
        reported
            .iter()
            .any(|(chunk, _)| chunk == keleusma_native::MODULE_LEVEL_REFUSAL),
        "a module-level refusal is not reported by `module_refusals`, so callers \
         reading its emptiness will treat an unlowerable module as accepted: \
         {reported:?}"
    );
}

/// **THE HAZARD, DEMONSTRATED BEFORE IT IS FIXED.**
///
/// This census decides by asking *"was there a refusal?"*. Emission is checked
/// first, so a probe that does not emit its opcode is caught. **But an opcode the
/// lowering never VISITS raises no refusal either**, and would land in the
/// supported column having proved nothing.
///
/// `Op::Reset` is exactly that shape: `isa_lowering_census` records it as
/// appearing ONLY in skipped positions, and the degenerate-stream transform
/// reaches `Stream` and steps over `Reset`.
///
/// **This is not hypothetical for this file.** Its own comment records
/// `IntToFloat` and `FloatToInt` moving *"from refused to lowers while the
/// backend had gained no float support whatever"*, because module-level refusals
/// were not being matched. **Same flattering direction, different cause.**
#[test]
fn an_emitted_but_never_visited_opcode_would_land_in_the_supported_column() {
    const STREAM: &str = "loop p(resume: Word) -> Word { yield 1 }\nfn main() -> Word { 0 }";
    let m = module_of(STREAM).expect("the stream probe must compile");

    // The probe really does emit it -- the check this census already makes.
    assert!(
        ops_in(&m).contains("Reset"),
        "the stream probe does not emit Reset, so it cannot demonstrate anything; \
         emitted {:?}",
        ops_in(&m)
    );

    // No refusal is raised for it...
    let refusals = module_refusals(&m, LowerOptions::default());
    let named = refusals
        .iter()
        .any(|(_, e)| format!("{e:?}").contains("Reset"));

    // ...and the lowering never VISITS it. Those two facts together are the
    // hazard: the existing verdict rule reads the first and cannot see the
    // second.
    let (_, visits) = keleusma_native::module_lowered_op_indices(&m, LowerOptions::default());
    let visited_reset = m.chunks.iter().enumerate().any(|(ci, c)| {
        let Some(seen) = visits.get(ci).and_then(|v| v.as_ref()) else {
            return false;
        };
        c.ops
            .iter()
            .enumerate()
            .any(|(i, o)| matches!(o, keleusma::bytecode::Op::Reset) && seen.contains(&i))
    });

    println!("\n================ THE FALSE-SUPPORTED HAZARD, MEASURED");
    println!("  probe emits Reset      : true");
    println!("  a refusal names Reset  : {named}");
    println!("  the lowering VISITS it : {visited_reset}");
    if !named && !visited_reset {
        println!("  => Under the rule 'no refusal means supported', this probe would");
        println!("     report Reset as SUPPORTED while the backend never saw the opcode.");
        println!("     THE HAZARD IS REAL, not hypothetical.");
    } else if visited_reset {
        println!("  => The lowering DOES visit Reset, so a probe verdict is meaningful");
        println!("     and the opcode's status can be read directly.");
    }
    println!("================\n");

    assert!(
        !named,
        "a refusal now names Reset, so the opcode has a verdict and this \
         demonstration is describing a state that no longer exists -- re-derive it"
    );
}
