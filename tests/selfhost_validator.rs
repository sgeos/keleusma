//! Whole-validator obligations, with malformed modules never executed.
#![cfg(feature = "self-host")]

use keleusma::bytecode::Op;
use keleusma::selfhost::{compile_src, structural_reject_module_via_kel};

#[test]
fn ninth_call_argument_is_checked_against_its_declared_shape() {
    let source = "fn take(a: Word, b: Word, c: Word, d: Word, e: Word, f: Word, g: Word, h: Word, i: [Word; 1]) -> Word { i[0] } fn main() -> Word { take(1, 2, 3, 4, 5, 6, 7, 8, [9]) }";
    let mut module = compile_src(source);
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(!structural_reject_module_via_kel(&module));
    let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
    let construction = main
        .ops
        .iter_mut()
        .find(|op| matches!(op, Op::NewComposite(_)))
        .unwrap();
    *construction = Op::PopN(0);
    assert!(keleusma::verify::verify(&module).is_err());
    assert!(structural_reject_module_via_kel(&module));
}

#[cfg(feature = "floats")]
#[test]
fn float_constant_is_a_known_scalar_for_flat_access_rejection() {
    use keleusma::bytecode::{ConstValue, StructField};
    use keleusma::value_layout::ScalarKind;
    let mut module = compile_src("fn main() -> Word { 0 }");
    module.chunks[0].constants = vec![ConstValue::Float(1.0)];
    module.chunks[0].ops = vec![
        Op::Const(0),
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&module).is_err());
    assert!(structural_reject_module_via_kel(&module));
}

#[test]
fn packed_call_shapes_cover_wide_and_repeated_calls() {
    for count in [1usize, 8, 9, 32, 255] {
        let parameters = (0..count)
            .map(|i| {
                format!(
                    "p{i}: {}",
                    if i + 1 == count { "[Word; 1]" } else { "Word" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let arguments = (0..count)
            .map(|i| if i + 1 == count { "[9]" } else { "1" })
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!(
            "fn take({parameters}) -> Word {{ p{}[0] }} fn main() -> Word {{ let a = take({arguments}); take({arguments}) + a }}",
            count - 1
        );
        let mut module = compile_src(&source);
        assert!(keleusma::verify::verify(&module).is_ok(), "{count}");
        assert!(!structural_reject_module_via_kel(&module), "{count}");
        let main = module.chunks.iter_mut().find(|c| c.name == "main").unwrap();
        *main
            .ops
            .iter_mut()
            .rfind(|op| matches!(op, Op::NewComposite(_)))
            .unwrap() = Op::PopN(0);
        assert!(keleusma::verify::verify(&module).is_err(), "{count}");
        assert!(structural_reject_module_via_kel(&module), "{count}");
    }
}

#[test]
fn enum_layouts_beyond_the_old_prefix_remain_admissible() {
    for count in [64, 65, 256] {
        let mut source = String::new();
        for i in 0..count - 1 {
            source.push_str(&format!("enum E{i:03} {{ A([Word; 2]) }}\n"));
        }
        source.push_str("enum ZLast { A(Word) } fn main() -> ZLast { ZLast::A(7) }");
        let module = compile_src(&source);
        assert_eq!(module.enum_layouts.len(), count);
        assert!(keleusma::verify::verify(&module).is_ok());
        assert!(!structural_reject_module_via_kel(&module), "{count}");
        if count == 256 {
            let mut too_many = module.clone();
            too_many.enum_layouts.push(module.enum_layouts[0].clone());
            assert!(
                std::panic::catch_unwind(|| structural_reject_module_via_kel(&too_many)).is_err()
            );
        }
    }
}

#[test]
fn absent_caller_signature_does_not_hide_callee_parameter_shapes() {
    let mut module = compile_src(
        "fn aa(x: [Word; 1]) -> Word { x[0] } fn zz() -> Word { aa([7]) } fn main() -> Word { zz() }",
    );
    assert_eq!(module.chunks[0].name, "aa");
    module.signatures.truncate(1);
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(!structural_reject_module_via_kel(&module));
    let caller = module.chunks.iter_mut().find(|c| c.name == "zz").unwrap();
    *caller
        .ops
        .iter_mut()
        .find(|op| matches!(op, Op::NewComposite(_)))
        .unwrap() = Op::PopN(0);
    assert!(keleusma::verify::verify(&module).is_err());
    assert!(structural_reject_module_via_kel(&module));
}

#[test]
fn absent_chunk_signatures_do_not_disable_enum_layout_validation() {
    use keleusma::bytecode::NewCompositeOperand;
    let mut module = compile_src("enum E { A(Word) } fn main() -> E { E::A(7) }");
    module.signatures.clear();
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(!structural_reject_module_via_kel(&module));
    let construction = module.chunks[0]
        .ops
        .iter_mut()
        .find(|op| matches!(op, Op::NewComposite(_)))
        .unwrap();
    if let Op::NewComposite(NewCompositeOperand::Flat { byte_size, .. }) = construction {
        *byte_size += 8;
    } else {
        panic!("expected flat enum");
    }
    assert!(keleusma::verify::verify(&module).is_err());
    assert!(structural_reject_module_via_kel(&module));
}

#[test]
fn absent_caller_signature_preserves_native_return_shapes() {
    use keleusma::bytecode::{StructField, WireShape};
    use keleusma::value_layout::{CompositeKind, ScalarKind};
    let mut module = compile_src("fn main() -> Word { 0 }");
    module.signatures.clear();
    module.native_names = vec!["host_value".into()];
    module.native_return_shapes = vec![WireShape::Flat {
        kind: CompositeKind::Struct.to_tag(),
        size: 8,
    }];
    module.chunks[0].ops = vec![
        Op::CallVerifiedNative(0, 0),
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(!structural_reject_module_via_kel(&module));
    module.chunks[0].ops[1] = Op::GetField(StructField::Flat {
        offset: 8,
        kind: ScalarKind::Int,
    });
    assert!(keleusma::verify::verify(&module).is_err());
    assert!(structural_reject_module_via_kel(&module));
}

/// C5 RESIDUAL 1, REPAIRED. After a loop the reference sets each local the body writes, and
/// each operand entry above the loop's entry height, to the join of the break states of its
/// loop-head fixpoint. Until this repair the typed stage forgot them, so a field read the
/// reference refuses after the loop was deferred by the stage and accepted. Measured on
/// `ff48e35c`, before the repair, the stage accepted the refused module of each case below.
/// See `docs/decisions/C5_SELFHOSTED_VALIDATOR_REVIEW.md`.
#[test]
fn c5_a_local_written_in_a_loop_keeps_the_join_of_its_break_states() {
    use keleusma::bytecode::StructField;
    use keleusma::selfhost::{VerificationVerdict, verify_module_via_kel};
    use keleusma::value_layout::ScalarKind;
    let mut module = compile_src("fn main(n: Word) -> Word { let a = n; a }");
    assert!(module.chunks[0].local_count >= 2);
    // The review's witness. Local 1 is a scalar on every break, so both refuse the field read.
    module.chunks[0].ops = vec![
        Op::GetLocal(0),
        Op::SetLocal(1),
        Op::Loop(7),
        Op::GetLocal(0),
        Op::SetLocal(1),
        Op::Break(7),
        Op::EndLoop(3),
        Op::GetLocal(1),
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    let reference = keleusma::verify::verify(&module).expect_err("the reference refuses it");
    assert!(
        reference.message.contains("ExpectedComposite"),
        "{}",
        reference.message
    );
    assert!(matches!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected { stage: "types", .. }
    ));
    // The control. The same read with no loop before it is refused by both.
    module.chunks[0].ops = vec![
        Op::GetLocal(0),
        Op::SetLocal(1),
        Op::GetLocal(1),
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&module).is_err());
    assert!(matches!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected { stage: "types", .. }
    ));
}

/// Both validators agree on each loop shape of the residual-one corpus modules, at a field
/// offset inside the body that is known after the loop and at one past it. The local case
/// writes a flat body on every iteration, the widening case writes a scalar over it so that the
/// fixpoint's second pass forgets it, and the operand case leaves a flat body on the stack at
/// its break. The widening case is the one a join of the first pass's breaks would refuse,
/// since the first pass alone still sees the flat body.
#[test]
fn c5_a_loop_exit_agrees_with_the_reference_fixpoint() {
    use c5_corpus::{LoopShape, loop_module};
    use keleusma::selfhost::{VerificationVerdict, verify_module_via_kel};
    for (shape, offset, reference_accepts) in [
        (LoopShape::Local, 0, true),
        (LoopShape::Local, 8, false),
        (LoopShape::Widening, 0, true),
        (LoopShape::Widening, 8, true),
        (LoopShape::Operand, 0, true),
        (LoopShape::Operand, 8, false),
    ] {
        let module = loop_module(shape, offset);
        let reference = keleusma::verify::verify(&module);
        assert_eq!(
            reference.is_ok(),
            reference_accepts,
            "{shape:?} at {offset}: {reference:?}"
        );
        let verdict = verify_module_via_kel(&module).unwrap();
        if reference_accepts {
            assert_eq!(
                verdict,
                VerificationVerdict::Accepted,
                "{shape:?} at {offset}"
            );
        } else {
            assert!(
                matches!(
                    verdict,
                    VerificationVerdict::Rejected { stage: "types", .. }
                ),
                "{shape:?} at {offset}: {verdict:?}"
            );
        }
    }
}

/// The `Text` field read of a body with `fields` text fields, rewritten to read the leading
/// `Word` field as text, which the reference refuses with `TextFieldMismatch`.
fn text_field_module(fields: usize, read_text: bool) -> keleusma::bytecode::Module {
    use keleusma::bytecode::StructField;
    use keleusma::value_layout::ScalarKind;
    let decl: Vec<String> = (0..fields).map(|i| format!("t{i}: Text")).collect();
    let init: Vec<String> = (0..fields).map(|i| format!("t{i}: \"x\"")).collect();
    let source = format!(
        "struct S {{ n: Word, m: Word, {} }} fn main() -> Text {{ let s = S {{ n: 1, m: 2, {} }}; s.t{} }}",
        decl.join(", "),
        init.join(", "),
        fields - 1
    );
    let mut module = compile_src(&source);
    assert!(keleusma::verify::verify(&module).is_ok());
    if !read_text {
        let read = module.chunks[0]
            .ops
            .iter_mut()
            .find(|op| matches!(op, Op::GetField(StructField::Flat { .. })))
            .expect("the read of the last text field");
        *read = Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Text,
        });
    }
    module
}

/// C5 RESIDUAL 2, REPAIRED. The typed stage recorded at most four `Text` offsets of a body's
/// layout and treated a larger layout as unknown, so a `Text` read at an offset holding no text
/// was refused by the reference and deferred by the stage. Measured on `ff48e35c`, before the
/// repair, four fields agreed and five disagreed. Layouts are now variable-length records in a
/// heap, so every width below agrees, including a last text field past the old fourth offset.
#[test]
fn c5_text_layouts_of_any_width_agree_with_the_reference() {
    use keleusma::selfhost::{VerificationVerdict, verify_module_via_kel};
    for fields in [1usize, 4, 5, 16, 40] {
        let good = text_field_module(fields, true);
        assert_eq!(
            verify_module_via_kel(&good).unwrap(),
            VerificationVerdict::Accepted,
            "{fields}"
        );
        let bad = text_field_module(fields, false);
        let message = keleusma::verify::verify(&bad).unwrap_err().message;
        assert!(message.contains("TextFieldMismatch"), "{fields}: {message}");
        assert!(
            matches!(
                verify_module_via_kel(&bad).unwrap(),
                VerificationVerdict::Rejected { stage: "types", .. }
            ),
            "{fields}"
        );
    }
}

/// The reference gives an element read from an array of flat bodies the layout every element
/// shares, `element_text_layout`. Until this repair the stage left it unknown, so a `Text` read
/// of the element's `Word` field was refused by the reference and accepted by the stage, which
/// was measured on `ff48e35c`.
#[test]
fn c5_an_array_element_keeps_the_layout_its_elements_share() {
    use keleusma::bytecode::StructField;
    use keleusma::selfhost::{VerificationVerdict, verify_module_via_kel};
    use keleusma::value_layout::ScalarKind;
    let mut module = compile_src(c5_corpus::TEXT_ARRAY);
    assert!(keleusma::verify::verify(&module).is_ok());
    assert_eq!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Accepted
    );
    let read = module.chunks[0]
        .ops
        .iter_mut()
        .rfind(|op| matches!(op, Op::GetField(StructField::Flat { offset: 0, .. })))
        .expect("the read of field n");
    *read = Op::GetField(StructField::Flat {
        offset: 0,
        kind: ScalarKind::Text,
    });
    let message = keleusma::verify::verify(&module).unwrap_err().message;
    assert!(message.contains("TextFieldMismatch"), "{message}");
    assert!(matches!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected { stage: "types", .. }
    ));
}

/// The layout heap is a capacity, and past it the typed stage reports unavailable analysis,
/// never a verdict. A body of 2,048 `Text` fields copied 32 times records more offsets than the
/// heap's 65,536 words hold. The reference accepts the module, so a verdict here would be an
/// acceptance of a module whose layout the stage no longer knows, and the stage must refuse to
/// give one.
#[test]
fn c5_a_layout_heap_past_its_capacity_is_unavailable_analysis() {
    use keleusma::bytecode::{ConstValue, NewCompositeOperand};
    use keleusma::value_layout::CompositeKind;
    let mut module = compile_src("fn main() -> Word { 0 }");
    let chunk = &mut module.chunks[0];
    chunk.constants = vec![ConstValue::StaticStr("x".into()), ConstValue::Int(0)];
    let flat = |count: u16, byte_size: u16| {
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Array,
            count,
            byte_size,
        })
    };
    let build = |copies: usize| {
        let mut ops = vec![Op::Const(0)];
        let mut size: u16 = 16;
        for _ in 0..11 {
            ops.push(Op::Dup);
            size *= 2;
            ops.push(flat(2, size));
        }
        for _ in 0..copies {
            ops.push(Op::Dup);
            ops.push(flat(1, size));
            ops.push(Op::PopN(1));
        }
        ops.push(Op::PopN(1));
        ops.push(Op::Const(1));
        ops.push(Op::Return);
        ops
    };
    // The control. Two copies fit, and both validators accept.
    chunk.ops = build(2);
    assert!(keleusma::verify::verify(&module).is_ok());
    assert_eq!(
        keleusma::selfhost::verify_module_via_kel(&module).unwrap(),
        keleusma::selfhost::VerificationVerdict::Accepted
    );
    module.chunks[0].ops = build(32);
    assert!(keleusma::verify::verify(&module).is_ok());
    assert_eq!(
        keleusma::selfhost::verify_module_via_kel(&module),
        Err(keleusma::selfhost::VerificationError::Incomplete {
            stage: "types",
            cause: 2
        })
    );
}

/// The C5 review's mutation corpus, in its own file so that its generator stays readable. See
/// `docs/decisions/C5_SELFHOSTED_VALIDATOR_REVIEW.md`.
#[path = "selfhost_validator/c5_corpus.rs"]
mod c5_corpus;
