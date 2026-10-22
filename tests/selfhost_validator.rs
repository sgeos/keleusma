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
