//! RESEARCH SPIKE: what does composite lowering actually COST?
//!
//! The blocker ranking says composites free 34.5 percent of the corpus. It says
//! nothing about the work required, and a priority claim with a delivery figure
//! and no cost figure is half an argument.
//!
//! The question is decided by which FORMS the corpus uses. Every composite
//! opcode carries a compiler-baked operand with two or three variants, and they
//! differ enormously in what a backend must build:
//!
//! - `Flat` is a byte offset and a scalar kind, resolved at compile time. That is
//!   the same shape `GetData` already lowers: base pointer, constant offset,
//!   typed load.
//! - `FlatNested` extracts a byte range and re-wraps it as a composite value,
//!   which needs a representation for a composite ON THE OPERAND STACK.
//! - `Boxed` is the pre-flat form with a metadata table and a heap body.
//!
//! Run with `cargo test --test spike_composite_cost -- --nocapture --test-threads=1`.

use keleusma::bytecode::{
    ArrayElem, EnumField, Module, NewCompositeOperand, Op, StructField, TupleField,
};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use std::collections::BTreeMap;

fn corpus() -> Vec<Module> {
    let root = std::path::Path::new("..");
    let mut out = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = ["examples/scripts", "src/selfhost/kel"]
        .iter()
        .map(|d| root.join(d))
        .collect();
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                stack.extend(rd.filter_map(|e| e.ok()).map(|e| e.path()));
            }
        } else if p.extension().is_some_and(|x| x == "kel")
            && let Ok(src) = std::fs::read_to_string(&p)
            && let Ok(t) = tokenize(&src)
            && let Ok(a) = parse(&t)
            && let Ok(m) = compile(&a)
        {
            out.push(m);
        }
    }
    out
}

/// THE COST QUESTION: which forms does the corpus actually emit?
#[test]
fn spike_report_composite_forms() {
    let mut f: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut sizes: Vec<u16> = Vec::new();
    let mut counts: Vec<u16> = Vec::new();

    for m in corpus() {
        for c in &m.chunks {
            for op in &c.ops {
                match op {
                    Op::NewComposite(NewCompositeOperand::Flat {
                        count, byte_size, ..
                    }) => {
                        *f.entry("NewComposite::Flat").or_default() += 1;
                        sizes.push(*byte_size);
                        counts.push(*count);
                    }
                    Op::NewComposite(NewCompositeOperand::Boxed { .. }) => {
                        *f.entry("NewComposite::Boxed").or_default() += 1;
                    }
                    Op::GetField(StructField::Flat { .. }) => {
                        *f.entry("GetField::Flat").or_default() += 1;
                    }
                    Op::GetField(StructField::FlatNested { .. }) => {
                        *f.entry("GetField::FlatNested").or_default() += 1;
                    }
                    Op::GetField(StructField::Boxed { .. }) => {
                        *f.entry("GetField::Boxed").or_default() += 1;
                    }
                    Op::GetTupleField(TupleField::Flat { .. }) => {
                        *f.entry("GetTupleField::Flat").or_default() += 1;
                    }
                    Op::GetTupleField(TupleField::FlatNested { .. }) => {
                        *f.entry("GetTupleField::FlatNested").or_default() += 1;
                    }
                    Op::GetTupleField(TupleField::Boxed { .. }) => {
                        *f.entry("GetTupleField::Boxed").or_default() += 1;
                    }
                    Op::GetIndex(ArrayElem::Flat { .. }) => {
                        *f.entry("GetIndex::Flat").or_default() += 1;
                    }
                    Op::GetIndex(ArrayElem::FlatNested { .. }) => {
                        *f.entry("GetIndex::FlatNested").or_default() += 1;
                    }
                    Op::GetIndex(ArrayElem::Boxed) => {
                        *f.entry("GetIndex::Boxed").or_default() += 1;
                    }
                    Op::GetEnumField(EnumField::Flat { .. }) => {
                        *f.entry("GetEnumField::Flat").or_default() += 1;
                    }
                    Op::GetEnumField(EnumField::FlatNested { .. }) => {
                        *f.entry("GetEnumField::FlatNested").or_default() += 1;
                    }
                    Op::GetEnumField(EnumField::Boxed { .. }) => {
                        *f.entry("GetEnumField::Boxed").or_default() += 1;
                    }
                    Op::IsEnum(..) => *f.entry("IsEnum").or_default() += 1,
                    Op::IsStruct(..) => *f.entry("IsStruct").or_default() += 1,
                    _ => {}
                }
            }
        }
    }

    println!("\n================ COMPOSITE FORMS IN THE CORPUS");
    let total: usize = f.values().sum();
    for (k, n) in &f {
        println!(
            "  {k:28} {n:5}  ({:5.1}%)",
            100.0 * *n as f64 / total as f64
        );
    }
    println!("  {:28} {total:5}", "TOTAL");

    let flat: usize = f
        .iter()
        .filter(|(k, _)| k.ends_with("::Flat"))
        .map(|(_, n)| *n)
        .sum();
    let nested: usize = f
        .iter()
        .filter(|(k, _)| k.ends_with("::FlatNested"))
        .map(|(_, n)| *n)
        .sum();
    let boxed: usize = f
        .iter()
        .filter(|(k, _)| k.ends_with("::Boxed"))
        .map(|(_, n)| *n)
        .sum();
    println!("\n  Flat (baked offset + scalar kind) : {flat}");
    println!("  FlatNested (needs a composite on the stack) : {nested}");
    println!("  Boxed (pre-flat, metadata table)  : {boxed}");

    if !sizes.is_empty() {
        sizes.sort_unstable();
        counts.sort_unstable();
        println!(
            "\n  NewComposite byte_size: min {} median {} max {}",
            sizes[0],
            sizes[sizes.len() / 2],
            sizes[sizes.len() - 1]
        );
        println!(
            "  NewComposite count    : min {} median {} max {}",
            counts[0],
            counts[counts.len() / 2],
            counts[counts.len() - 1]
        );
    }
    println!("\n  -> If Boxed is 0 the pre-flat path is dead and need not be built.");
    println!("  -> If FlatNested is 0 no composite ever reaches the operand stack");
    println!("     as a value, and the whole job is pointer arithmetic.");
    println!("================\n");
}
