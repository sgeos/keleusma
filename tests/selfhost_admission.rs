//! Fallible verifier results must separate malformed modules from unavailable analysis.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{Module, Op};
use keleusma::selfhost::{
    VerificationError, VerificationVerdict, compile_src, verify_module_via_kel,
};

fn base() -> Module {
    compile_src("fn main() -> Word { 7 }")
}
fn accepted(module: &Module) {
    assert!(keleusma::verify::verify(module).is_ok());
    assert_eq!(
        verify_module_via_kel(module).unwrap(),
        VerificationVerdict::Accepted
    );
}

#[test]
fn valid_modules_complete_all_structural_and_typed_passes() {
    for source in [
        "fn main() -> Word { 7 }",
        "yield main(n: Word) -> Word { yield n }",
        "loop main(n: Word) -> Word { yield n }",
        "shared data d { n: Word } fn main() -> Word { d.n = 7; d.n }",
        "enum E { A(Word), B } fn main() -> Word { match E::A(7) { E::A(n) => n, E::B => 0 } }",
    ] {
        accepted(&compile_src(source));
    }
}

#[test]
fn malformed_targets_are_rejected_before_region_walks() {
    for op in [
        Op::If(0),
        Op::Else(0),
        Op::Loop(0),
        Op::If(u16::MAX),
        Op::Loop(u16::MAX),
    ] {
        let mut module = base();
        module.chunks[0].ops[0] = op;
        assert!(keleusma::verify::verify(&module).is_err());
        assert_eq!(
            verify_module_via_kel(&module).unwrap(),
            VerificationVerdict::Rejected {
                stage: "structure",
                chunk: Some(0)
            }
        );
    }
}

#[test]
fn table_capacity_and_bad_operands_are_distinct() {
    let mut module = base();
    module.chunks[0].ops = vec![Op::Return; keleusma::selfhost_host::VERIFY_OP_CAP + 1];
    assert!(matches!(
        verify_module_via_kel(&module),
        Err(VerificationError::Capacity {
            resource: "opcodes",
            ..
        })
    ));
    let mut module = base();
    module.chunks[0].local_count = 257;
    assert!(matches!(
        verify_module_via_kel(&module),
        Err(VerificationError::Capacity {
            resource: "locals",
            ..
        })
    ));
    let mut module = base();
    module.chunks[0].ops[0] = Op::GetLocal(0);
    assert_eq!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected {
            stage: "structure",
            chunk: Some(0)
        }
    );
}

#[test]
fn operand_storage_exhaustion_cannot_be_reported_as_acceptance() {
    for count in [256, 257] {
        let mut module = base();
        module.chunks[0].ops = vec![Op::Const(0); count];
        module.chunks[0].ops.push(Op::Return);
        assert!(keleusma::verify::verify(&module).is_ok());
        if count == 256 {
            accepted(&module);
        } else {
            assert!(matches!(
                verify_module_via_kel(&module),
                Err(VerificationError::Incomplete {
                    stage: "types",
                    cause: 1
                })
            ));
        }
    }
}

#[test]
fn nesting_capacity_preserves_the_declared_boundary() {
    for count in [32, 33] {
        let mut module = base();
        let mut ops = Vec::new();
        for _ in 0..count {
            ops.push(Op::PushImmediate(1));
            ops.push(Op::If(0));
        }
        for i in (0..count).rev() {
            ops[i * 2 + 1] = Op::If(ops.len() as u16);
            ops.push(Op::EndIf);
        }
        ops.extend([Op::Const(0), Op::Return]);
        module.chunks[0].ops = ops;
        assert!(keleusma::verify::verify(&module).is_ok());
        if count == 32 {
            accepted(&module);
        } else {
            assert!(matches!(
                verify_module_via_kel(&module),
                Err(VerificationError::Incomplete {
                    stage: "depth",
                    cause: 1
                })
            ));
        }
    }
}

#[test]
fn layout_failures_are_stage_decisions() {
    let mut module = compile_src("shared data d { n: Word } fn main() -> Word { d.n = 7; d.n }");
    accepted(&module);
    module.data_layout.as_mut().unwrap().shared_layout[0].offset = module.shared_data_bytes;
    assert!(keleusma::verify::verify(&module).is_err());
    assert_eq!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected {
            stage: "data layout",
            chunk: None
        }
    );
}

#[test]
fn loop_body_cannot_consume_values_below_its_entry_floor() {
    let mut module = base();
    module.chunks[0].ops = vec![
        Op::Const(0),
        Op::Loop(6),
        Op::Const(0),
        Op::PopN(1),
        Op::Break(6),
        Op::EndLoop(2),
        Op::Return,
    ];
    accepted(&module);
    module.chunks[0].ops.swap(2, 3);
    assert!(keleusma::verify::verify(&module).is_err());
    assert_eq!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected {
            stage: "types",
            chunk: Some(0)
        }
    );
}

#[test]
fn every_current_stage_completes_the_fallible_verifier() {
    let mut paths: Vec<_> = std::fs::read_dir("src/selfhost/kel")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|s| s == "kel"))
        .collect();
    paths.sort();
    assert_eq!(paths.len(), 15);
    for path in paths {
        let module = compile_src(&std::fs::read_to_string(&path).unwrap());
        assert!(keleusma::verify::verify(&module).is_ok());
        assert_eq!(
            verify_module_via_kel(&module).unwrap(),
            VerificationVerdict::Accepted,
            "{}",
            path.display()
        );
    }
}

#[test]
fn declared_target_widths_include_float_disabled_modules() {
    use keleusma::target::Target;
    let source = "shared data d { n: Word } fn main() -> Word { d.n = 7; d.n }";
    for target in [
        Target::host(),
        Target::wasm32(),
        Target::embedded_32(),
        Target::embedded_16(),
        Target::embedded_8(),
    ] {
        let program = keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap();
        let module = keleusma::compiler::compile_with_target(&program, &target).unwrap();
        accepted(&module);
    }
    let mut module = base();
    module.word_bits_log2 = u8::MAX;
    assert!(matches!(
        verify_module_via_kel(&module),
        Err(VerificationError::Protocol {
            stage: "module",
            ..
        })
    ));
}

#[test]
fn data_layout_batches_preserve_a_late_failure() {
    let mut module = compile_src(
        "shared data d { xs: [Word; 1025] } fn main() -> Word { d.xs[0] = 7; d.xs[0] }",
    );
    accepted(&module);
    module.data_layout.as_mut().unwrap().shared_layout[1024].offset = module.shared_data_bytes;
    assert!(keleusma::verify::verify(&module).is_err());
    assert_eq!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected {
            stage: "data layout",
            chunk: None
        }
    );
}

#[test]
fn depth_and_productivity_failures_are_completed_rejections() {
    let mut module = base();
    module.chunks[0].ops = vec![Op::PopN(1), Op::Return];
    assert!(keleusma::verify::verify(&module).is_err());
    assert_eq!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected {
            stage: "depth",
            chunk: Some(0)
        }
    );
    let mut module = compile_src("loop main(n: Word) -> Word { yield n }");
    module.chunks[0].ops = vec![
        Op::Stream,
        Op::PushImmediate(1),
        Op::If(6),
        Op::GetLocal(0),
        Op::Yield,
        Op::PopN(1),
        Op::EndIf,
        Op::Reset,
    ];
    assert!(keleusma::verify::verify(&module).is_err());
    assert_eq!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected {
            stage: "productivity",
            chunk: Some(0)
        }
    );
}

#[test]
fn control_flow_widening_does_not_invent_a_scalar_violation() {
    use keleusma::bytecode::{NewCompositeOperand, StructField};
    use keleusma::value_layout::{CompositeKind, ScalarKind};
    let mut module = base();
    module.chunks[0].ops = vec![
        Op::Const(0),
        Op::PushImmediate(1),
        Op::If(7),
        Op::PopN(1),
        Op::Const(0),
        Op::NewComposite(NewCompositeOperand::Flat {
            kind: CompositeKind::Struct,
            count: 1,
            byte_size: 8,
        }),
        Op::Else(8),
        Op::PopN(0),
        Op::EndIf,
        Op::GetField(StructField::Flat {
            offset: 8,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    accepted(&module);
}

#[test]
fn terminated_branches_do_not_make_dead_typed_operations_live() {
    use keleusma::bytecode::StructField;
    use keleusma::value_layout::ScalarKind;
    let mut module = base();
    module.chunks[0].ops = vec![
        Op::PushImmediate(1),
        Op::If(5),
        Op::Const(0),
        Op::Return,
        Op::Else(7),
        Op::Const(0),
        Op::Return,
        Op::EndIf,
        Op::Const(0),
        Op::GetField(StructField::Flat {
            offset: 0,
            kind: ScalarKind::Int,
        }),
        Op::Return,
    ];
    accepted(&module);
}

#[test]
fn incompatible_break_heights_are_rejected() {
    let mut module = base();
    module.chunks[0].ops = vec![
        Op::Loop(9),
        Op::PushImmediate(1),
        Op::If(6),
        Op::Const(0),
        Op::Break(9),
        Op::Else(7),
        Op::Break(9),
        Op::EndIf,
        Op::EndLoop(1),
        Op::Const(0),
        Op::Return,
    ];
    assert!(keleusma::verify::verify(&module).is_err());
    assert!(matches!(
        verify_module_via_kel(&module).unwrap(),
        VerificationVerdict::Rejected { .. }
    ));
}
