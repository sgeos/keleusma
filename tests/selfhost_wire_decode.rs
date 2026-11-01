//! Semantic opcode decoding is compared against the runtime decoder.
#![cfg(feature = "self-host")]

use keleusma::bytecode::{ArrayElem, EnumField, NewCompositeOperand, Op, StructField, TupleField};
use keleusma::selfhost::{wire_decode_opcode_sections_via_kel, wire_opcode_sections_via_kel};
use keleusma::value_layout::{CompositeKind, ScalarKind};
use keleusma::wire_format::{
    OpcodeId, OpcodeRecord, OperandPoolEntry, decode_op, decode_op_stream,
};

#[test]
fn every_opcode_and_access_form_round_trips_through_keleusma() {
    let mut ops = Vec::new();
    for id in 0..=127 {
        let pool = match id {
            5 | 6 => vec![OperandPoolEntry::from_u24_u24(0, 0)],
            43 => vec![OperandPoolEntry::from_u16_u16_u16(0, 0, 0)],
            _ => vec![],
        };
        if let Ok(op) = decode_op(
            OpcodeRecord::from_id_and_operand(OpcodeId(id), [0; 3]),
            &pool,
        ) {
            ops.push(op);
        }
    }
    assert_eq!(ops.len(), 66);
    for kind in (0..=7).filter_map(ScalarKind::from_tag) {
        ops.extend([
            Op::GetField(StructField::Flat {
                offset: 65535,
                kind,
            }),
            Op::GetTupleField(TupleField::Flat {
                offset: 65535,
                kind,
            }),
            Op::GetEnumField(EnumField::Flat {
                offset: 65535,
                kind,
            }),
            Op::GetIndex(ArrayElem::Flat { kind }),
        ]);
    }
    for variant in (0..=3).filter_map(CompositeKind::from_tag) {
        ops.extend([
            Op::GetField(StructField::FlatNested {
                offset: 65535,
                size: 65535,
                variant,
            }),
            Op::GetTupleField(TupleField::FlatNested {
                offset: 65535,
                size: 65535,
                variant,
            }),
            Op::GetEnumField(EnumField::FlatNested {
                offset: 65535,
                size: 65535,
                variant,
            }),
            Op::GetIndex(ArrayElem::FlatNested {
                size: 65535,
                variant,
            }),
        ]);
        for count in [0, 62, 63, 65535] {
            ops.push(Op::NewComposite(NewCompositeOperand::Flat {
                kind: variant,
                count,
                byte_size: 65535,
            }));
            ops.push(Op::NewComposite(NewCompositeOperand::Boxed {
                kind: variant,
                count,
                meta: 65535,
            }));
        }
    }
    ops.extend([
        Op::GetField(StructField::Boxed { name_const: 65535 }),
        Op::GetTupleField(TupleField::Boxed { index: 255 }),
        Op::GetEnumField(EnumField::Boxed { index: 255 }),
        Op::GetIndex(ArrayElem::Boxed),
        Op::GetData(0xFFFFFF),
        Op::SetData(0xFFFFFF),
        Op::GetDataIndexed(0xFFFFFF, 0xFFFFFF),
        Op::IsEnum(65535, 65535, 65535),
        Op::Call(65535, 255),
        Op::PushImmediate(255),
    ]);
    let ast =
        keleusma::parser::parse(&keleusma::lexer::tokenize("fn main() -> Word { 7 }").unwrap())
            .unwrap();
    let mut module = keleusma::compiler::compile(&ast).unwrap();
    module.chunks[0].ops = ops.clone();
    let sections = wire_opcode_sections_via_kel(&module).unwrap();
    assert_eq!(
        wire_decode_opcode_sections_via_kel(&sections.opcodes, &sections.operands).unwrap(),
        ops
    );
    assert_eq!(
        decode_op_stream(&sections.opcodes, &sections.operands).unwrap(),
        ops
    );
}

#[test]
fn malformed_lengths_and_unused_pool_corruption_are_refused() {
    for (ops, pool) in [
        (vec![0; 1], vec![]),
        (vec![0; 3], vec![]),
        (vec![], vec![0; 7]),
        (vec![], vec![0; 9]),
    ] {
        assert!(wire_decode_opcode_sections_via_kel(&ops, &pool).is_err());
        assert!(decode_op_stream(&ops, &pool).is_err());
    }
    let mut pool = OperandPoolEntry::from_u16_u16(0, 0).0;
    assert!(
        wire_decode_opcode_sections_via_kel(&[], &pool)
            .unwrap()
            .is_empty()
    );
    pool[7] ^= 1;
    assert!(wire_decode_opcode_sections_via_kel(&[], &pool).is_err());
    assert!(decode_op_stream(&[], &pool).is_err());
}
