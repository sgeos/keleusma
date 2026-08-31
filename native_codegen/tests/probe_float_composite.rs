//! **WHAT ACTUALLY BLOCKS A FLOAT INSIDE A COMPOSITE?**
//!
//! Written BEFORE the increment that closes it, and before its brief, because
//! this line has now sized work from the component being changed five times and
//! been wrong every time. The requirement lives at the boundary. This probe asks
//! the boundary.
//!
//! It reports rather than asserts, apart from one non-vacuity check.

use keleusma::bytecode::Module;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::LowerOptions;

fn build(src: &str) -> Option<Module> {
    tokenize(src)
        .ok()
        .and_then(|t| parse(&t).ok())
        .and_then(|a| compile(&a).ok())
}

#[test]
fn what_refuses_a_float_inside_a_composite() {
    let cases: &[(&str, &str)] = &[
        (
            "struct field, written and read",
            "struct P { x: Float, n: Word }
             fn main(a: Word, b: Word) -> Word { let p = P { x: a as Float, n: b }; p.n }",
        ),
        (
            "struct field, the float READ back",
            "struct P { x: Float, n: Word }
             fn main(a: Word, b: Word) -> Word { let p = P { x: a as Float, n: b }; (p.x) as Word }",
        ),
        (
            "tuple carrying a float",
            "fn main(a: Word, b: Word) -> Word { let t = (a as Float, b); t.1 }",
        ),
        (
            "array of floats",
            "fn main(a: Word, b: Word) -> Word { let xs = [a as Float, b as Float]; (xs[0]) as Word }",
        ),
        (
            "NESTED: a struct holding a struct that holds a float",
            "struct Inner { x: Float }
             struct Outer { i: Inner, n: Word }
             fn main(a: Word, b: Word) -> Word {
                 let o = Outer { i: Inner { x: a as Float }, n: b }; (o.i.x) as Word
             }",
        ),
        (
            "NESTED: an array of structs each holding a float",
            "struct Inner { x: Float }
             fn main(a: Word, b: Word) -> Word {
                 let xs = [Inner { x: a as Float }, Inner { x: b as Float }];
                 (xs[1].x) as Word
             }",
        ),
        (
            "NESTED control: the same nesting with no float",
            "struct J { x: Word }
             struct K { i: J, n: Word }
             fn main(a: Word, b: Word) -> Word {
                 let k = K { i: J { x: a }, n: b }; k.i.x
             }",
        ),
        (
            "control: the same struct with no float",
            "struct Q { x: Word, n: Word }
             fn main(a: Word, b: Word) -> Word { let q = Q { x: a, n: b }; q.n }",
        ),
    ];

    let mut compiled = 0usize;
    let mut lowered = 0usize;
    println!("\n================ FLOAT INSIDE A COMPOSITE, measured before building");
    for (name, src) in cases {
        match build(src) {
            None => println!("  {name}\n    REFERENCE COMPILER REFUSES IT"),
            Some(m) => {
                compiled += 1;
                let refusals = keleusma_native::module_refusals(&m, LowerOptions::default());
                if refusals.is_empty() {
                    lowered += 1;
                    println!("  {name}\n    LOWERS with no refusal");
                } else {
                    println!("  {name}");
                    for (chunk, e) in refusals.iter().take(3) {
                        println!("    chunk {chunk}: {e}");
                    }
                }
            }
        }
    }
    println!("================\n");

    // **NON-VACUITY.** The control must compile and lower, or a sweep in which
    // everything fails says nothing about floats specifically.
    assert!(
        compiled > 0 && lowered > 0,
        "no case compiled and lowered at all, so this probe is measuring the \
         harness rather than the float question"
    );
}

mod common;

/// **THE CORPUS POPULATION, READ AS DATA.**
///
/// The brief's prediction depends on this and refuses to guess it: if a corpus
/// module carries a float inside a composite, the coverage censuses MOVE when
/// this route opens, and a movement that was not predicted is indistinguishable
/// from a regression.
///
/// Read from the INSTRUCTION STREAM — the field and element kind tags the
/// backend itself dispatches on — rather than from the corpus source text.
#[test]
fn how_many_corpus_modules_carry_a_float_inside_a_composite() {
    use keleusma::bytecode::{ArrayElem, Op, StructField};
    use keleusma::value_layout::ScalarKind as SK;

    let mut modules = 0usize;
    let mut composite_sites = 0usize;
    let mut float_field_reads = 0usize;
    let mut carriers: Vec<String> = Vec::new();

    for p in common::corpus_sources() {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Some(m) = build(&src) else { continue };
        modules += 1;
        let mut hit = false;
        for c in &m.chunks {
            for op in &c.ops {
                match op {
                    Op::NewComposite(_) => composite_sites += 1,
                    Op::GetField(StructField::Flat {
                        kind: SK::Float, ..
                    })
                    | Op::GetIndex(ArrayElem::Flat { kind: SK::Float }) => {
                        float_field_reads += 1;
                        hit = true;
                    }
                    _ => {}
                }
            }
        }
        if hit {
            carriers.push(p.file_name().unwrap().to_string_lossy().to_string());
        }
    }

    println!("\n================ FLOAT INSIDE A COMPOSITE, corpus population");
    println!("  corpus modules compiled   : {modules}");
    println!("  composite construction ops: {composite_sites}");
    println!("  float field/element reads : {float_field_reads}");
    println!(
        "  modules carrying one      : {} {carriers:?}",
        carriers.len()
    );
    println!("================\n");

    // **NON-VACUITY.** A sweep seeing no composite construction at all would
    // report an empty float population for the wrong reason.
    assert!(
        composite_sites > 0,
        "the sweep found no composite construction anywhere in {modules} \
         modules, so its float population says nothing about floats"
    );
}

/// **WHICH UNEXERCISED ARMS CAN ORDINARY SOURCE REACH AT ALL?**
///
/// The kind-arm census left twenty-six combinations unexercised, and they are
/// not one kind of thing: some are refused by the lowering, some are simply
/// undriven. **Sizing witnesses for them without asking the reference compiler
/// first would be the mistake this line has made six times.** This asks.
#[test]
fn which_narrow_and_fixed_composite_reads_are_reachable_from_source() {
    let cases: &[(&str, &str)] = &[
        (
            "struct x Bool",
            "struct P { b: bool, n: Word }
             fn main(a: Word, b: Word) -> Word { let p = P { b: true, n: b }; if p.b { p.n } else { 0 } }",
        ),
        (
            "array x Bool",
            "fn main(a: Word, b: Word) -> Word { let xs = [true, false]; if xs[0] { b } else { 0 } }",
        ),
        (
            "tuple x Byte",
            "fn main(a: Word, b: Word) -> Word { let t = (200 as Byte, b); (t.0) as Word + t.1 }",
        ),
        (
            "tuple x Fixed",
            "fn main(a: Word, b: Word) -> Word { let t = (a as Fixed<16>, b); ((t.0) as Word) + t.1 }",
        ),
        (
            "array x Fixed",
            "fn main(a: Word, b: Word) -> Word { let xs = [a as Fixed<16>, b as Fixed<16>]; ((xs[1]) as Word) + b }",
        ),
        (
            "enum x Byte",
            "enum E { A(Byte), B }
             fn main(a: Word, b: Word) -> Word { let e = E::A(200 as Byte); match e { E::A(x) => (x as Word) + b, E::B => 0 } }",
        ),
    ];

    println!("\n================ REACHABILITY OF THE UNEXERCISED ARMS");
    let mut compiled = 0usize;
    for (name, src) in cases {
        match build(src) {
            None => println!("  {name}\n    REFERENCE COMPILER REFUSES THIS SOURCE"),
            Some(m) => {
                compiled += 1;
                let r = keleusma_native::module_refusals(&m, LowerOptions::default());
                if r.is_empty() {
                    println!("  {name}\n    compiles and LOWERS");
                } else {
                    println!("  {name}\n    compiles, backend refuses: {}", r[0].1);
                }
            }
        }
    }
    println!("================\n");
    assert!(
        compiled > 0,
        "no case compiled, so this probe measures the harness rather than reachability"
    );
}

/// **WHERE DOES A COMPOSITE OPERAND'S WIDTH COME FROM?** Measured, because the
/// refusal message names an operand position and not an opcode, and reasoning
/// backwards from the packer is the mistake this line keeps making.
#[test]
fn what_pushes_a_composite_operand_for_each_element_kind() {
    for (name, src) in [
        (
            "[1, 2]",
            "fn main(a: Word, b: Word) -> Word { let xs = [1, 2]; xs[0] + b }",
        ),
        (
            "[true, false]",
            "fn main(a: Word, b: Word) -> Word { let xs = [true, false]; if xs[0] { b } else { 0 } }",
        ),
        (
            "[a as Fixed<16>, b as Fixed<16>]",
            "fn main(a: Word, b: Word) -> Word { let xs = [a as Fixed<16>, b as Fixed<16>]; ((xs[1]) as Word) + b }",
        ),
    ] {
        let Some(m) = build(src) else {
            println!("  {name}: reference compiler refuses");
            continue;
        };
        let ops: Vec<String> = m.chunks[0]
            .ops
            .iter()
            .take(8)
            .map(|o| format!("{o:?}").chars().take(28).collect::<String>())
            .collect();
        println!("  {name}\n    {ops:?}");
    }
}
