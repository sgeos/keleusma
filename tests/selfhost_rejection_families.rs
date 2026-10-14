//! Differential controls for typed rejection families beyond the stage corpus.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{
    ArrayElem, ConstValue, EnumField, Module, Op, StructField, TupleField, WireShape,
};
use keleusma::selfhost::{
    VerificationVerdict, compile_src, typed_reject_module_via_kel, verify_module_via_kel,
};
use keleusma::value_layout::{CompositeKind, ScalarKind};
use keleusma::verify_typed::{TypedError, typed_check_module};

fn accepted(module: &Module) {
    assert!(keleusma::verify::verify(module).is_ok());
    assert!(!typed_reject_module_via_kel(module));
    assert_eq!(
        verify_module_via_kel(module).unwrap(),
        VerificationVerdict::Accepted
    );
}
fn rejected(module: &Module, expected: fn(&TypedError) -> bool) {
    let error = typed_check_module(module, 8, 8).unwrap_err();
    assert!(expected(&error), "unexpected reference family {error:?}");
    if matches!(
        error,
        TypedError::SharedSlotOutOfBounds { .. }
            | TypedError::PrivateCompositeLayoutInvalid { .. }
            | TypedError::SharedLayoutCountMismatch { .. }
    ) {
        assert!(keleusma::selfhost::dl_reject_module_via_kel(module));
    } else {
        assert!(typed_reject_module_via_kel(module));
    }
    assert!(matches!(
        verify_module_via_kel(module).unwrap(),
        VerificationVerdict::Rejected { .. }
    ));
}
fn seeded(size: u32) -> Module {
    let mut m = compile_src("fn main(n: Word) -> Word { n }");
    m.chunks[0].constants = vec![ConstValue::Int(7)];
    m.signatures[0].params[0] = WireShape::Flat {
        kind: CompositeKind::Struct.to_tag(),
        size,
    };
    m
}
#[test]
fn scalar_and_nested_array_strides_reach_the_typed_stage() {
    for element in [
        ArrayElem::Flat {
            kind: ScalarKind::Int,
        },
        ArrayElem::FlatNested {
            size: 8,
            variant: CompositeKind::Struct,
        },
    ] {
        let mut m = seeded(16);
        m.chunks[0].ops = vec![
            Op::GetLocal(0),
            Op::Const(0),
            Op::GetIndex(element),
            Op::Return,
        ];
        accepted(&m);
        m.signatures[0].params[0] = WireShape::Flat {
            kind: CompositeKind::Array.to_tag(),
            size: 12,
        };
        rejected(&m, |e| matches!(e, TypedError::ArrayStrideMismatch { .. }));
    }
    let mut m = seeded(0);
    m.chunks[0].ops = vec![
        Op::GetLocal(0),
        Op::Const(0),
        Op::GetIndex(ArrayElem::FlatNested {
            size: 0,
            variant: CompositeKind::Struct,
        }),
        Op::Return,
    ];
    accepted(&m);
}
#[test]
fn declared_local_assignment_checks_shape() {
    let mut m = seeded(8);
    m.chunks[0].ops = vec![Op::GetLocal(0), Op::SetLocal(0), Op::Const(0), Op::Return];
    accepted(&m);
    m.chunks[0].ops[0] = Op::Const(0);
    rejected(&m, |e| matches!(e, TypedError::LocalTypeMismatch { .. }));
}
#[test]
fn yield_resume_shape_controls_field_admission() {
    let mut m = compile_src("yield main(n: Word) -> Word { yield n }");
    m.chunks[0].constants = vec![ConstValue::Int(7)];
    m.signatures[0].resume = WireShape::Flat {
        kind: CompositeKind::Struct.to_tag(),
        size: 8,
    };
    m.chunks[0].ops = vec![
        Op::Const(0),
        Op::Yield,
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    accepted(&m);
    m.chunks[0].ops[2] = Op::GetField(StructField::Flat {
        offset: 8,
        kind: ScalarKind::Int,
    });
    rejected(&m, |e| matches!(e, TypedError::OffsetOutOfBounds { .. }));
}
#[test]
fn every_flat_projection_family_checks_the_extent() {
    for offset in [0, 16] {
        for op in [
            Op::GetField(StructField::Flat {
                offset,
                kind: ScalarKind::Int,
            }),
            Op::GetField(StructField::FlatNested {
                offset,
                size: 8,
                variant: CompositeKind::Struct,
            }),
            Op::GetTupleField(TupleField::Flat {
                offset,
                kind: ScalarKind::Int,
            }),
            Op::GetTupleField(TupleField::FlatNested {
                offset,
                size: 8,
                variant: CompositeKind::Struct,
            }),
            Op::GetEnumField(EnumField::Flat {
                offset,
                kind: ScalarKind::Int,
            }),
            Op::GetEnumField(EnumField::FlatNested {
                offset,
                size: 8,
                variant: CompositeKind::Struct,
            }),
        ] {
            let mut m = seeded(16);
            m.chunks[0].ops = vec![Op::GetLocal(0), op, Op::Return];
            if offset == 0 {
                accepted(&m);
            } else {
                rejected(&m, |e| matches!(e, TypedError::OffsetOutOfBounds { .. }));
            }
        }
    }
}
#[test]
fn loop_back_edge_height_must_be_neutral() {
    let mut m = compile_src("fn main() -> Word { 7 }");
    m.chunks[0].ops = vec![
        Op::Loop(4),
        Op::Const(0),
        Op::PopN(1),
        Op::EndLoop(1),
        Op::Const(0),
        Op::Return,
    ];
    accepted(&m);
    m.chunks[0].ops[2] = Op::PopN(0);
    rejected(&m, |e| matches!(e, TypedError::LoopNotNeutral { .. }));
}
#[test]
fn layout_reconciliation_checks_prefix_count_and_private_order() {
    use keleusma::bytecode::SlotVisibility;
    let m = compile_src(
        "struct P { n: Word } shared data s { a: Word, b: Word } private data p { a: P, b: P } fn main() -> Word { p.a = P { n: 1 }; p.b = P { n: 2 }; 7 }",
    );
    accepted(&m);
    let mut count = m.clone();
    count.data_layout.as_mut().unwrap().shared_layout.pop();
    rejected(&count, |e| {
        matches!(e, TypedError::SharedLayoutCountMismatch { .. })
    });
    let mut prefix = m.clone();
    prefix.data_layout.as_mut().unwrap().slots[0].visibility = SlotVisibility::Private;
    rejected(&prefix, |e| {
        matches!(e, TypedError::SharedLayoutCountMismatch { .. })
    });
    let mut order = m.clone();
    let private = &mut order.data_layout.as_mut().unwrap().private_composite_layout;
    assert!(private.len() >= 2);
    private.swap(0, 1);
    rejected(&order, |e| {
        matches!(e, TypedError::PrivateCompositeLayoutInvalid { .. })
    });
}

#[test]
fn packed_call_shape_pool_accepts_its_exact_capacity() {
    use keleusma::selfhost::VerificationError;
    let parameters = (0..128)
        .map(|i| format!("p{i}: Word"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut module = compile_src(&format!(
        "fn f({parameters}) -> Word {{ p0 }} fn main() -> Word {{ 7 }}"
    ));
    let callee = module.chunks.iter().position(|c| c.name == "f").unwrap();
    let main = module.chunks.iter().position(|c| c.name == "main").unwrap();
    // Dead calls still need metadata transport. This isolates the call-shape
    // pool from the independent operand-stack and opcode capacities.
    assert!(matches!(module.chunks[main].ops.last(), Some(Op::Return)));
    module.chunks[main]
        .ops
        .extend(std::iter::repeat_n(Op::Call(callee as u16, 128), 96));
    accepted(&module);
    module.chunks[main].ops.push(Op::Call(callee as u16, 128));
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(matches!(
        verify_module_via_kel(&module),
        Err(VerificationError::Capacity {
            stage: "types",
            resource: "call shapes",
            limit: 12288,
            actual: 12416
        })
    ));
}

#[test]
fn loop_termination_preserves_the_reference_post_loop_checks() {
    let mut module = compile_src("fn main() -> Word { 7 }");
    module.chunks[0].ops = vec![
        Op::Loop(4),
        Op::Const(0),
        Op::Return,
        Op::EndLoop(1),
        Op::Const(0),
        Op::Return,
    ];
    accepted(&module);
    assert_eq!(
        keleusma::selfhost::admit_module_via_kel(&module, 65536).unwrap(),
        VerificationVerdict::Accepted
    );
    // The reference deliberately uses the loop-entry state when neither a body
    // fall-through nor a break state exists. Match its checks after that join.
    module.chunks[0].ops.insert(
        5,
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
    );
    let actual = keleusma::selfhost::admit_module_via_kel(&module, 65536).unwrap();
    assert!(
        matches!(actual, VerificationVerdict::Rejected { .. }),
        "{actual:?}"
    );
    rejected(&module, |error| {
        matches!(error, TypedError::ExpectedComposite { .. })
    });
}
