//! A cast to the operand's own primitive type is identity.
//!
//! The reference compiler lowered `Word as Word` to `FloatToInt`, `Float as Float` to
//! `IntToFloat`, `Byte as Byte` to `WordToByte` and `Fixed as Fixed` to `WordToFixed`.
//! Each conversion opcode faults on a value already of its result type, so all four
//! programs compiled and verified and then failed at run time with a `TypeError`. The
//! operator ruled on 2026-10-28 that this is a reference defect to be fixed.
//!
//! # What is established
//!
//! Each identity cast runs to the operand's value and emits no conversion opcode. The
//! genuine conversions keep their opcodes, which is the negative control. Under the
//! `self-host` feature the self-hosted compiler produces the reference's bytes for the
//! Word and Byte forms, the only cast targets its subset admits.
//!
//! # What is NOT established
//!
//! A source whose type the reference cannot infer still takes the conversion the type
//! checker validated, so `<unknown> as Word` keeps `FloatToInt`. No case here reaches it.

extern crate alloc;

use keleusma::Arena;
use keleusma::bytecode::{Module, Op, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

fn reference(src: &str) -> Module {
    let toks = tokenize(src).unwrap_or_else(|e| panic!("{src}\nlex: {e:?}"));
    let prog = parse(&toks).unwrap_or_else(|e| panic!("{src}\nparse: {e:?}"));
    compile(&prog).unwrap_or_else(|e| panic!("{src}\ncompile: {e:?}"))
}

fn run(src: &str) -> Value {
    let module = reference(src);
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("arena");
    let mut vm = Vm::new(module, &arena).unwrap_or_else(|e| panic!("{src}\nverify: {e:?}"));
    let mut shared = alloc::vec![0; vm.shared_data_bytes()];
    match vm.call_with_shared(&mut shared, &[]) {
        Ok(VmState::Finished(v)) => v,
        other => panic!("{src}\nunexpected {other:?}"),
    }
}

fn is_conversion(op: &Op) -> bool {
    matches!(
        op,
        Op::FloatToInt
            | Op::IntToFloat
            | Op::WordToByte
            | Op::ByteToWord
            | Op::WordToFixed(_)
            | Op::FixedToWord(_)
    )
}

fn conversions(src: &str) -> Vec<Op> {
    reference(src)
        .chunks
        .iter()
        .flat_map(|c| c.ops.iter())
        .filter(|op| is_conversion(op))
        .cloned()
        .collect()
}

/// Each identity cast runs, and emits only the conversions its other casts need.
#[test]
fn identity_casts_run_to_the_operand_value() {
    let cases: [(&str, Value, usize); 5] = [
        (
            "fn main() -> Word { let w: Word = 7; w as Word }",
            Value::Int(7),
            0,
        ),
        (
            "fn main() -> Float { let f: Float = 2.5; f as Float }",
            Value::Float(2.5),
            0,
        ),
        // One `WordToByte` builds the Byte operand, and the identity cast adds none.
        (
            "fn main() -> Byte { let b: Byte = 3 as Byte; b as Byte }",
            Value::Byte(3),
            1,
        ),
        // One `WordToFixed` builds the operand and one `FixedToWord` reads it back.
        (
            "fn main() -> Word { let a: Fixed = 3 as Fixed; let b = a as Fixed; b as Word }",
            Value::Int(3),
            2,
        ),
        (
            "fn main() -> Word { let a: Fixed<8> = 5 as Fixed<8>; let b = a as Fixed<8>; b as Word }",
            Value::Int(5),
            2,
        ),
    ];
    for (src, expected, count) in cases {
        assert_eq!(run(src), expected, "{src}");
        assert_eq!(
            conversions(src).len(),
            count,
            "{src}: {:?}",
            conversions(src)
        );
    }
}

/// Negative control. A cast that changes type keeps its conversion opcode.
#[test]
fn genuine_conversions_keep_their_opcodes() {
    let cases: [(&str, Op); 4] = [
        (
            "fn main() -> Word { let f: Float = 2.5; f as Word }",
            Op::FloatToInt,
        ),
        (
            "fn main() -> Float { let w: Word = 2; w as Float }",
            Op::IntToFloat,
        ),
        (
            "fn main() -> Byte { let w: Word = 2; w as Byte }",
            Op::WordToByte,
        ),
        (
            "fn main() -> Word { let b: Byte = 2 as Byte; b as Word }",
            Op::ByteToWord,
        ),
    ];
    for (src, op) in cases {
        assert!(
            conversions(src).contains(&op),
            "{src}: {:?}",
            conversions(src)
        );
    }
}

/// The self-hosted compiler gives the reference's bytes for the identity casts it admits.
#[cfg(feature = "self-host")]
#[test]
fn self_hosted_identity_casts_match_the_reference() {
    for src in [
        "fn main() -> Word { let w: Word = 7; w as Word }",
        "fn main() -> Word { 7 as Word }",
        "fn main() -> Byte { let b: Byte = 3 as Byte; b as Byte }",
        "fn main() -> Word { let b: Byte = 3 as Byte; (b as Word) + 1 }",
        // A literal does not clear the parser's Byte flag, so the second cast here must
        // still narrow. The first form of the self-hosted rule dropped it.
        "fn main() -> Byte { (255 as Byte) + (1 as Byte) { ok(v) => v, overflow(v) => v } }",
        "fn f(x: Word) -> Word { x }\nfn main() -> Word { f(2) as Word }",
        "fn main() -> Word { let w: Word = 7; let b: Byte = w as Byte; let c: Byte = b as Byte; (c as Word) as Word }",
    ] {
        let expected = reference(src).to_bytes().unwrap();
        let actual =
            keleusma::selfhost::self_hosted_compile(src, &keleusma::target::Target::host())
                .unwrap_or_else(|error| panic!("{src}\n{error}"));
        assert_eq!(actual.to_bytes().unwrap(), expected, "{src}");
    }
}
