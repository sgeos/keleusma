//! Resource decisions are compared without executing subject bytecode.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{ConstValue, Op};
use keleusma::selfhost::{compile_src, text_bounds_via_kel};

#[test]
fn text_lattice_matches_the_reference_over_control_flow_and_calls() {
    let base = compile_src("fn main() -> Word { 0 }");
    let cases = vec![
        vec![Op::Const(0), Op::Return],
        vec![Op::Const(0), Op::Const(1), Op::Add, Op::Return],
        vec![
            Op::Const(0),
            Op::SetLocal(0),
            Op::GetLocal(0),
            Op::Dup,
            Op::Add,
            Op::Yield,
        ],
        vec![
            Op::If(4),
            Op::Const(0),
            Op::Const(1),
            Op::Add,
            Op::EndIf,
            Op::Return,
        ],
        vec![
            Op::Loop(5),
            Op::Const(0),
            Op::Const(1),
            Op::Add,
            Op::EndLoop(1),
            Op::Return,
        ],
        vec![Op::Call(0, 0), Op::Const(0), Op::Add, Op::Return],
        vec![Op::Call(1, 0), Op::Const(0), Op::Add, Op::Return],
        vec![Op::Call(2, 0), Op::Const(2), Op::Add, Op::Return],
        vec![Op::CallVerifiedNative(0, 0), Op::Dup, Op::Add, Op::Return],
        vec![
            Op::CallExternalNative(0, 128),
            Op::Const(2),
            Op::Add,
            Op::Return,
        ],
        vec![
            Op::GetLocal(99),
            Op::SetLocal(99),
            Op::PopN(20),
            Op::Dup,
            Op::Return,
        ],
    ];
    for ops in cases {
        let mut chunk = base.chunks[0].clone();
        chunk.local_count = 1;
        chunk.constants = vec![
            ConstValue::StaticStr("ab".into()),
            ConstValue::StaticStr("c".into()),
            ConstValue::Int(7),
        ];
        chunk.ops = ops;
        let expected = keleusma::text_size::analyze_chunk_text(&chunk, &[true, false]);
        let actual = text_bounds_via_kel(&chunk, &[true, false]).unwrap();
        assert_eq!(
            (actual.heap_bytes, actual.returns_text, actual.yields_text),
            (
                expected.heap_alloc,
                expected.returns_text,
                expected.yields_text
            ),
            "{:?}",
            chunk.ops
        );
    }
}

#[test]
fn text_heap_saturates_and_stops_at_the_same_instruction() {
    let mut chunk = compile_src("fn main() -> Word { 0 }").chunks.remove(0);
    chunk.constants = vec![ConstValue::StaticStr("x".into())];
    chunk.ops = vec![Op::Const(0)];
    for _ in 0..33 {
        chunk.ops.extend([Op::Dup, Op::Add]);
    }
    chunk.ops.push(Op::Return);
    let expected = keleusma::text_size::analyze_chunk_text(&chunk, &[]);
    assert_eq!(expected.heap_alloc, u32::MAX);
    let actual = text_bounds_via_kel(&chunk, &[]).unwrap();
    assert_eq!(
        (actual.heap_bytes, actual.returns_text, actual.yields_text),
        (
            expected.heap_alloc,
            expected.returns_text,
            expected.yields_text
        )
    );
}

fn assert_admission_boundary(module: &keleusma::bytecode::Module) {
    use keleusma::bytecode::BlockType;
    use keleusma::selfhost::{VerificationVerdict, admit_module_via_kel};
    assert!(keleusma::verify::verify(module).is_ok());
    let bounds = keleusma::verify::module_wcmu(module, &[]).unwrap();
    let limit = module
        .chunks
        .iter()
        .zip(bounds)
        .filter(|(c, _)| c.block_type == BlockType::Stream)
        .map(|(_, (s, h))| s as usize + h as usize)
        .max()
        .unwrap();
    for capacity in [limit.saturating_sub(1), limit, limit + 1] {
        let reference = keleusma::verify::verify_resource_bounds(module, capacity).is_ok();
        let actual = admit_module_via_kel(module, capacity).unwrap();
        assert_eq!(
            matches!(actual, VerificationVerdict::Accepted),
            reference,
            "capacity {capacity}, verdict {actual:?}"
        );
    }
}

#[test]
fn resource_admission_matches_transitive_boundaries() {
    for source in [
        "loop main(n: Word) -> Word { yield n }",
        "fn f(x: Word) -> Word { x + 1 } fn g(x: Word) -> Word { f(x) + 2 } loop main(n: Word) -> Word { yield g(n) }",
        "struct P { n: Word } shared data d { p: P } loop main(n: Word) -> Word { d.p = P { n: n }; let p = d.p; yield p.n }",
        "loop main(n: Word) -> Word { for i in 0..8 limit 8 { let xs = [1,2,3]; } yield n }",
    ] {
        assert_admission_boundary(&compile_src(source));
    }
}

#[test]
fn resource_admission_includes_text_heap() {
    let mut module = compile_src("loop main(n: Word) -> Word { yield n }");
    module.chunks[0].constants = vec![
        ConstValue::StaticStr("ab".into()),
        ConstValue::StaticStr("c".into()),
    ];
    module.chunks[0].ops = vec![
        Op::Stream,
        Op::Const(0),
        Op::Const(1),
        Op::Add,
        Op::Yield,
        Op::PopN(1),
        Op::Reset,
    ];
    let expected = keleusma::verify::module_wcmu(&module, &[]).unwrap();
    assert_eq!(expected[0].1, 3);
    assert_admission_boundary(&module);
}

#[test]
fn heap_arithmetic_saturates_like_the_reference() {
    let elements = vec!["1"; 256].join(",");
    let source = format!(
        "loop main(n: Word) -> Word {{ for i in 0..3000000 limit 3000000 {{ let xs = [{elements}]; }} yield n }}"
    );
    let module = compile_src(&source);
    assert_eq!(
        keleusma::verify::module_wcmu(&module, &[]).unwrap()[0].1,
        u32::MAX
    );
    assert_admission_boundary(&module);
}

#[test]
fn recursive_modules_do_not_receive_resource_admission() {
    use keleusma::selfhost::{VerificationVerdict, admit_module_via_kel};
    let mut module = compile_src("fn main() -> Word { 0 }");
    module.chunks[0].ops = vec![Op::Call(0, 0), Op::Return];
    assert!(keleusma::verify::verify_resource_bounds(&module, usize::MAX).is_err());
    assert!(matches!(
        admit_module_via_kel(&module, usize::MAX).unwrap(),
        VerificationVerdict::Rejected {
            stage: "resource cycle",
            ..
        }
    ));
}

#[test]
fn host_resource_stage_preserves_declared_subject_target_widths() {
    use keleusma::target::Target;
    let source = "loop main(n: Word) -> Word { yield n }";
    for target in [
        Target::host(),
        Target::wasm32(),
        Target::embedded_32(),
        Target::embedded_16(),
        Target::embedded_8(),
    ] {
        let program = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let module = keleusma::compiler::compile_with_target(&program, &target).unwrap();
        assert_admission_boundary(&module);
    }
    let stage = include_str!("../src/selfhost/kel/analyze.kel");
    let program = keleusma::parser::parse(&keleusma::lexer::tokenize(stage).unwrap()).unwrap();
    assert!(keleusma::compiler::compile_with_target(&program, &Target::host()).is_ok());
    assert!(keleusma::compiler::compile_with_target(&program, &Target::embedded_32()).is_err());
}

#[test]
fn native_attestations_preserve_per_site_and_per_chunk_charging() {
    use keleusma::bytecode::{BlockType, WireShape};
    use keleusma::selfhost::{VerificationVerdict, admit_module_with_bounds_via_kel};
    use keleusma::value_layout::ScalarKind;
    use keleusma::verify::NativeIterationBound;
    let original = compile_src(
        "fn stub() -> Word { 7 } loop main(n: Word) -> Word { for i in 0..3 limit 3 { let a = stub(); let b = stub(); } yield n }",
    );
    for external in [false, true] {
        let mut module = original.clone();
        module.native_names = vec!["host_stub".into()];
        module.native_return_shapes = vec![WireShape::Scalar {
            kind: ScalarKind::Int.to_tag(),
        }];
        for chunk in &mut module.chunks {
            for op in &mut chunk.ops {
                if let Op::Call(_, n) = op {
                    *op = if external {
                        Op::CallExternalNative(0, *n)
                    } else {
                        Op::CallVerifiedNative(0, *n)
                    };
                }
            }
        }
        assert!(keleusma::verify::verify(&module).is_ok());
        for maximum in [None, Some(5), Some(u32::MAX)] {
            let bounds = [NativeIterationBound {
                per_call_wcmu_bytes: 13,
                per_call_wcet_cycles: 1,
                max_invocations: maximum,
            }];
            for width in [32, 64] {
                let expected =
                    keleusma::verify::module_wcmu_with_bounds(&module, &bounds, width).unwrap();
                let limit = module
                    .chunks
                    .iter()
                    .zip(expected)
                    .filter(|(c, _)| c.block_type == BlockType::Stream)
                    .map(|(_, (s, h))| s as usize + h as usize)
                    .max()
                    .unwrap();
                for capacity in [limit - 1, limit, limit + 1] {
                    let reference = keleusma::verify::verify_resource_bounds_with_bounds(
                        &module, capacity, &bounds, width,
                    )
                    .is_ok();
                    let actual =
                        admit_module_with_bounds_via_kel(&module, capacity, &bounds, width)
                            .unwrap();
                    assert_eq!(
                        matches!(actual, VerificationVerdict::Accepted),
                        reference,
                        "external {external}, maximum {maximum:?}, width {width}, capacity {capacity}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_stage_matches_complete_resource_admission() {
    let mut paths: Vec<_> = std::fs::read_dir("src/selfhost/kel")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|s| s == "kel"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 15);
    for path in paths {
        let module = compile_src(&std::fs::read_to_string(&path).unwrap());
        assert_admission_boundary(&module);
    }
}

#[test]
fn iteration_counts_obey_the_reference_u32_boundary() {
    use keleusma::selfhost::{VerificationVerdict, admit_module_via_kel};
    for count in [i64::from(u32::MAX), i64::from(u32::MAX) + 1, i64::MAX] {
        let mut module =
            compile_src("loop main(n: Word) -> Word { for i in 0..8 limit 8 {} yield n }");
        for chunk in &mut module.chunks {
            for value in &mut chunk.constants {
                if matches!(value, ConstValue::Int(8)) {
                    *value = ConstValue::Int(count);
                }
            }
        }
        let expected = keleusma::verify::verify_resource_bounds(&module, 65536).is_ok();
        assert_eq!(expected, count == i64::from(u32::MAX));
        assert_eq!(
            matches!(
                admit_module_via_kel(&module, 65536).unwrap(),
                VerificationVerdict::Accepted
            ),
            expected
        );
    }
    let mut module = compile_src("loop main(n: Word) -> Word { for i in 0..8 limit 8 {} yield n }");
    for chunk in &mut module.chunks {
        for value in &mut chunk.constants {
            match value {
                ConstValue::Int(8) => *value = ConstValue::Int(i64::MAX),
                ConstValue::Int(0) => *value = ConstValue::Int(i64::MIN),
                _ => {}
            }
        }
    }
    assert!(keleusma::verify::verify_resource_bounds(&module, 65536).is_err());
    assert!(matches!(
        admit_module_via_kel(&module, 65536).unwrap(),
        VerificationVerdict::Rejected { .. }
    ));
}

#[test]
fn literal_array_loop_bounds_follow_backward_aliases() {
    for source in [
        "loop main(n: Word) -> Word { for x in [1,2,3] { let a = [x,x]; } yield n }",
        "loop main(n: Word) -> Word { let a = [1,2,3]; let b = a; let c = b; for x in c { let v = [x,x]; } yield n }",
    ] {
        assert_admission_boundary(&compile_src(source));
    }
    let module = compile_src(
        "fn make() -> [Word; 3] { [1,2,3] } loop main(n: Word) -> Word { let a = make(); for x in a { let b = [x,x]; } yield n }",
    );
    // The reference compiler can derive this length from the declared return type.
    assert_admission_boundary(&module);
}

#[test]
fn raw_array_length_pattern_requires_a_literal_origin() {
    use keleusma::bytecode::{NewCompositeOperand, WireShape};
    use keleusma::selfhost::{VerificationVerdict, admit_module_via_kel};
    use keleusma::value_layout::CompositeKind;
    let mut module = compile_src("fn main() -> Word { 7 }");
    module.chunks[0].local_count = 3;
    module.chunks[0].constants = vec![ConstValue::Int(0), ConstValue::Int(1), ConstValue::Int(7)];
    module.chunks[0].ops = vec![
        Op::Const(2),
        Op::Const(2),
        Op::Const(2),
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Array,
            count: 3,
            byte_size: 24,
        }),
        Op::SetLocal(2),
        Op::GetLocal(2),
        Op::Len,
        Op::SetLocal(1),
        Op::Const(0),
        Op::SetLocal(0),
        Op::Loop(23),
        Op::GetLocal(0),
        Op::GetLocal(1),
        Op::CmpGe,
        Op::BreakIf(23),
        Op::PushImmediate(0),
        Op::PopN(1),
        Op::GetLocal(0),
        Op::Const(1),
        Op::CheckedAdd,
        Op::PopN(2),
        Op::SetLocal(0),
        Op::EndLoop(11),
        Op::Const(2),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(keleusma::verify::verify_resource_bounds(&module, 65536).is_ok());
    assert_eq!(
        admit_module_via_kel(&module, 65536).unwrap(),
        VerificationVerdict::Accepted
    );
    module.native_names = vec!["array_source".into()];
    module.native_return_shapes = vec![WireShape::Flat {
        kind: CompositeKind::Array.to_tag(),
        size: 24,
    }];
    module.chunks[0].ops[0] = Op::CallVerifiedNative(0, 0);
    for i in 1..4 {
        module.chunks[0].ops[i] = Op::PopN(0);
    }
    assert!(keleusma::verify::verify(&module).is_ok());
    assert!(keleusma::verify::verify_resource_bounds(&module, 65536).is_err());
    assert!(matches!(
        admit_module_via_kel(&module, 65536).unwrap(),
        VerificationVerdict::Rejected {
            stage: "resources",
            ..
        }
    ));
}

#[test]
fn diagnostic_outputs_preserve_text_below_their_operands() {
    for (prefix, operation) in [
        (vec![Op::Const(1)], Op::BoundsCheck(2)),
        (vec![Op::Const(1), Op::Const(1)], Op::SetDataIndexed(0, 1)),
    ] {
        let mut module = compile_src("fn main() -> Word { 0 }");
        let chunk = &mut module.chunks[0];
        chunk.constants = vec![
            keleusma::bytecode::ConstValue::StaticStr("retained".into()),
            keleusma::bytecode::ConstValue::Int(0),
        ];
        chunk.ops = vec![Op::Const(0)];
        chunk.ops.extend(prefix);
        chunk.ops.extend([operation, Op::PopN(6), Op::Return]);
        let expected = keleusma::text_size::analyze_chunk_text(chunk, &[]);
        assert!(expected.returns_text);
        let actual = text_bounds_via_kel(chunk, &[]).unwrap();
        assert_eq!(
            (actual.heap_bytes, actual.returns_text, actual.yields_text),
            (
                expected.heap_alloc,
                expected.returns_text,
                expected.yields_text
            )
        );
    }
}
