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

/// `(opcode, probe function name, source)`. The probe function isolates the
/// opcode; `main` exists only so the module has an entry.
const PROBES: &[(&str, &str, &str)] = &[
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
        match refusals.iter().find(|(chunk, _)| chunk == func) {
            Some((_, err)) => refused.push(format!("{opcode}  ({err:?})")),
            None => supported.push(opcode),
        }
    }

    println!("\n================ BACKEND OPCODE SUPPORT");
    println!("  probes            : {}", PROBES.len());
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
