//! Instruction transport and orchestration for the Keleusma trap classifier.
use super::analysis_machine::AnalysisMachine;
use super::*;
use crate::trap_analysis::{
    TrapAnalysisError, TrapLocation, TrapVerdict, UnsupportedTrapOperation,
};

type Result<T> = core::result::Result<T, TrapAnalysisError>;
fn unavailable(error: impl core::fmt::Debug) -> TrapAnalysisError {
    TrapAnalysisError::Unavailable {
        detail: format!("{error:?}"),
    }
}

// Raw operand-form transport. This deliberately does not label an operation safe.
fn operands(op: &Op) -> (i64, i64) {
    match op {
        Op::Trap(kind) => (i64::from(*kind), 0),
        Op::PushImmediate(kind) => (i64::from(*kind), 0),
        Op::GetIndex(ArrayElem::Flat { kind })
        | Op::GetField(StructField::Flat { kind, .. })
        | Op::GetTupleField(TupleField::Flat { kind, .. })
        | Op::GetEnumField(EnumField::Flat { kind, .. }) => (1, i64::from(kind.to_tag())),
        Op::GetIndex(ArrayElem::FlatNested { .. })
        | Op::GetField(StructField::FlatNested { .. })
        | Op::GetTupleField(TupleField::FlatNested { .. })
        | Op::GetEnumField(EnumField::FlatNested { .. }) => (2, 0),
        Op::GetIndex(ArrayElem::Boxed)
        | Op::GetField(StructField::Boxed { .. })
        | Op::GetTupleField(TupleField::Boxed { .. })
        | Op::GetEnumField(EnumField::Boxed { .. }) => (3, 0),
        _ => (0, 0),
    }
}

/// Validate through Keleusma and classify every chunk without executing subject code.
/// The stage receives wire tags and operand forms and owns profile admission and
/// trap detection. Runtime contract guards remain required after a trap-free result.
pub fn classify_traps_via_kel(module: &Module) -> Result<TrapVerdict> {
    classify_with_stage(module, verify_structural_kel_module())
}

fn classify_with_stage(module: &Module, stage: Module) -> Result<TrapVerdict> {
    crate::trap_analysis::preflight(module)?;
    match verify_module_via_kel(module).map_err(unavailable)? {
        VerificationVerdict::Accepted => {}
        rejected => {
            return Err(TrapAnalysisError::InvalidModule {
                detail: format!("{rejected:?}"),
            });
        }
    }
    let persistent = required_persistent_capacity_for(&stage);
    let capacity = persistent
        .checked_add(DEFAULT_ARENA_CAPACITY)
        .ok_or_else(|| unavailable("arena capacity overflow"))?;
    let mut arena = Arena::try_with_capacity(capacity).map_err(unavailable)?;
    arena.resize_persistent(persistent).map_err(unavailable)?;
    let mut machine = AnalysisMachine::new(
        stage,
        &arena,
        &[
            "sv.mode",
            "sv.op_count",
            "sv.class",
            "sv.arg",
            "sv.opb",
            "sv.out_reject",
            "sv.o1",
            "sv.o2",
            "sv.o3",
        ],
    )
    .map_err(unavailable)?;
    let mut first = None;
    let mut unsupported = None;
    for (chunk, body) in module.chunks.iter().enumerate() {
        machine
            .set(1, i64::try_from(body.ops.len()).map_err(unavailable)?)
            .map_err(unavailable)?;
        for (ip, op) in body.ops.iter().enumerate() {
            let (form, scalar) = operands(op);
            machine
                .set_element(2, ip, i64::from(crate::wire_format::opcode_id_of(op).0))
                .map_err(unavailable)?;
            machine.set_element(3, ip, form).map_err(unavailable)?;
            machine.set_element(4, ip, scalar).map_err(unavailable)?;
        }
        let verdict = machine.command(2).map_err(unavailable)?;
        if machine.get(5).map_err(unavailable)? != verdict {
            return Err(unavailable("yielded and stored trap verdicts differ"));
        }
        let trap_ip = machine.get(6).map_err(unavailable)?;
        let unsupported_ip = machine.get(7).map_err(unavailable)?;
        let reason = machine.get(8).map_err(unavailable)?;
        let location = |ip: i64| -> Result<TrapLocation> {
            let instruction = usize::try_from(ip).map_err(unavailable)?;
            if instruction >= body.ops.len() {
                return Err(unavailable("stage location outside subject chunk"));
            }
            Ok(TrapLocation { chunk, instruction })
        };
        match verdict {
            0 if trap_ip == -1 && unsupported_ip == -1 && reason == 0 => {}
            1 if unsupported_ip == -1 && reason == 0 => {
                first.get_or_insert(location(trap_ip)?);
            }
            2 => {
                if trap_ip >= 0 {
                    first.get_or_insert(location(trap_ip)?);
                } else if trap_ip != -1 {
                    return Err(unavailable("invalid absent-trap sentinel"));
                }
                let operation = match reason {
                    2 => UnsupportedTrapOperation::IndexedValue,
                    3 => UnsupportedTrapOperation::BoxedField,
                    4 => UnsupportedTrapOperation::NativeCall,
                    _ => return Err(unavailable("unknown unsupported-operation reason")),
                };
                unsupported.get_or_insert(TrapAnalysisError::Unsupported {
                    location: location(unsupported_ip)?,
                    operation,
                });
            }
            3 => {
                location(unsupported_ip)?;
                return Err(TrapAnalysisError::InvalidModule {
                    detail: format!(
                        "trap-profile instruction rejected at chunk {chunk}, instruction {unsupported_ip}"
                    ),
                });
            }
            _ => return Err(unavailable("invalid trap-analysis response")),
        }
    }
    if let Some(error) = unsupported {
        return Err(error);
    }
    Ok(
        first.map_or(TrapVerdict::TrapFree, |first| TrapVerdict::MayTrap {
            first,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_stage_owns_the_trap_decision() {
        let mut subject = compile_src("fn main() -> Word { 7 }");
        subject.chunks[0].ops = vec![Op::Trap(5)];
        assert!(matches!(
            classify_traps_via_kel(&subject).unwrap(),
            TrapVerdict::MayTrap { .. }
        ));
        let source = include_str!("kel/verify_structural.kel");
        let changed = source.replace(
            "if sv.class[ip] == 53 andalso",
            "if sv.class[ip] == 999 andalso",
        );
        assert_ne!(source, changed);
        assert_eq!(
            classify_with_stage(&subject, compile_src(&changed)).unwrap(),
            TrapVerdict::TrapFree
        );
    }
    #[test]
    fn the_stage_owns_profile_admission() {
        let mut subject = compile_src("fn main() -> Word { 7 }");
        // A boxed field access is outside the first-pass profile. Fixed
        // division served as this subject until it became total on 2026-10-22.
        subject.chunks[0].ops = vec![
            Op::PushImmediate(4),
            Op::Return,
            Op::GetField(StructField::Boxed { name_const: 0 }),
        ];
        assert!(matches!(
            classify_traps_via_kel(&subject),
            Err(TrapAnalysisError::Unsupported { .. })
        ));
        let source = include_str!("kel/verify_structural.kel");
        let changed = source.replace(
            "if form == 3 { 3 } else { 0 }",
            "if form == 3 { 0 } else { 0 }",
        );
        assert_ne!(source, changed);
        assert_eq!(
            classify_with_stage(&subject, compile_src(&changed)).unwrap(),
            TrapVerdict::TrapFree
        );
    }

    #[test]
    fn malformed_stage_inputs_refuse_and_the_next_request_recovers() {
        let stage = verify_structural_kel_module();
        let persistent = required_persistent_capacity_for(&stage);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).unwrap();
        let mut machine = AnalysisMachine::new(
            stage,
            &arena,
            &[
                "sv.mode",
                "sv.op_count",
                "sv.class",
                "sv.arg",
                "sv.opb",
                "sv.out_reject",
                "sv.o1",
                "sv.o2",
                "sv.o3",
            ],
        )
        .unwrap();
        for (count, tag, form, scalar, expected) in [
            (-1, 31, 0, 0, 4),
            (2049, 31, 0, 0, 4),
            (1, -1, 0, 0, 4),
            (1, 34, 0, 0, 4),
            (1, 37, 0, 0, 4),
            (1, 70, 0, 0, 4),
            (1, 31, 1, 0, 4),
            (1, 31, 0, 3, 4),
            (1, 39, 0, 3, 4),
            (1, 39, 1, 8, 4),
            (1, 39, 2, 3, 4),
            (1, 53, 9, 0, 3),
            (1, 60, 20, 0, 3),
            (2048, 31, 0, 0, 0),
        ] {
            machine.set(1, count).unwrap();
            machine.set_element(2, 0, tag).unwrap();
            machine.set_element(3, 0, form).unwrap();
            machine.set_element(4, 0, scalar).unwrap();
            assert_eq!(
                machine.command(2).unwrap(),
                expected,
                "{count}/{tag}/{form}/{scalar}"
            );
            machine.set(1, 1).unwrap();
            machine.set_element(2, 0, 31).unwrap();
            machine.set_element(3, 0, 0).unwrap();
            machine.set_element(4, 0, 0).unwrap();
            assert_eq!(machine.command(2).unwrap(), 0);
            assert_eq!(machine.get(6).unwrap(), -1);
            assert_eq!(machine.get(7).unwrap(), -1);
        }
        assert_eq!(
            crate::trap_analysis::TRAP_OPCODE_CAP,
            crate::selfhost_host::VERIFY_OP_CAP
        );
    }

    #[test]
    fn inconsistent_stage_responses_are_unavailable() {
        let subject = compile_src("fn main() -> Word { 7 }");
        let mut stage = verify_structural_kel_module();
        let scan = stage
            .chunks
            .iter_mut()
            .find(|c| c.name == "scan_traps")
            .unwrap();
        // A fabricated return does not populate the stored verdict and location sentinels.
        scan.ops = vec![Op::PushImmediate(4), Op::Return];
        assert!(matches!(
            classify_with_stage(&subject, stage),
            Err(TrapAnalysisError::Unavailable { .. })
        ));
        // An invalid-subject report must identify an instruction. An absent
        // location is a broken analysis response, not a proof of invalidity.
        let source = include_str!("kel/verify_structural.kel");
        let changed = source.replace("sv.out_reject = 0; sv.o1[0]", "sv.out_reject = 3; sv.o1[0]");
        assert_ne!(source, changed);
        assert!(matches!(
            classify_with_stage(&subject, compile_src(&changed)),
            Err(TrapAnalysisError::Unavailable { .. })
        ));
    }
    /// Every current opcode, in every operand form the transport
    /// distinguishes, gets the stage verdict the Rust analysis gives it.
    ///
    /// The expectation is derived from `trap_analysis::classify_instruction`,
    /// the function `classify_traps` itself applies, and the operand forms
    /// come from this file's `operands`, the transport the classifier uses.
    /// Until 2026-10-21 both were written into this test as a table of tags,
    /// so the stage was compared with the table and not with the analysis.
    #[test]
    fn every_current_wire_opcode_has_an_explicit_stage_profile() {
        use crate::trap_analysis::{InstructionTrapClass, classify_instruction};
        use crate::value_layout::ScalarKind;
        let stage = verify_structural_kel_module();
        let persistent = required_persistent_capacity_for(&stage);
        let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).unwrap();
        let mut machine = AnalysisMachine::new(
            stage,
            &arena,
            &[
                "sv.mode",
                "sv.op_count",
                "sv.class",
                "sv.arg",
                "sv.opb",
                "sv.out_reject",
                "sv.o1",
                "sv.o2",
                "sv.o3",
            ],
        )
        .unwrap();
        // One representative per opcode, then the operand forms that change a
        // verdict: each flat index kind, nested and boxed elements and fields,
        // and every trap kind.
        let mut subjects = crate::verify::tests::representative_ops();
        for kind in [ScalarKind::Byte, ScalarKind::Bool] {
            subjects.push(Op::GetIndex(ArrayElem::Flat { kind }));
        }
        let nested = crate::value_layout::CompositeKind::Struct;
        subjects.push(Op::GetIndex(ArrayElem::FlatNested {
            size: 8,
            variant: nested,
        }));
        subjects.push(Op::GetField(StructField::FlatNested {
            offset: 0,
            size: 8,
            variant: nested,
        }));
        subjects.push(Op::GetIndex(ArrayElem::Boxed));
        subjects.push(Op::GetField(StructField::Boxed { name_const: 0 }));
        subjects.push(Op::GetTupleField(TupleField::Boxed { index: 0 }));
        subjects.push(Op::GetEnumField(EnumField::Boxed { index: 0 }));
        for kind in 0..=8 {
            subjects.push(Op::Trap(kind));
        }
        let location = TrapLocation {
            chunk: 0,
            instruction: 0,
        };
        let mut tags = std::collections::BTreeSet::new();
        machine.set(1, 1).unwrap();
        for op in &subjects {
            let tag = crate::wire_format::opcode_id_of(op).0;
            tags.insert(tag);
            let (form, scalar) = operands(op);
            machine.set_element(2, 0, i64::from(tag)).unwrap();
            machine.set_element(3, 0, form).unwrap();
            machine.set_element(4, 0, scalar).unwrap();
            let expected = match classify_instruction(op, location).unwrap() {
                InstructionTrapClass::Clear => 0,
                InstructionTrapClass::MayTrap => 1,
                InstructionTrapClass::Unsupported(_) => 2,
            };
            assert_eq!(machine.command(2).unwrap(), expected, "{op:?}");
            assert_eq!(machine.get(5).unwrap(), expected, "{op:?}");
        }
        // Non-vacuity and completeness: every tag of the wire format's opcode
        // table was reached, and each verdict class occurred.
        let inventory: std::collections::BTreeSet<u8> = crate::wire_format::opcode_table()
            .iter()
            .map(|&(_, tag)| tag)
            .collect();
        assert_eq!(inventory.len(), 66);
        assert_eq!(tags, inventory);
        let classes: std::collections::BTreeSet<u8> = subjects
            .iter()
            .map(|op| match classify_instruction(op, location).unwrap() {
                InstructionTrapClass::Clear => 0,
                InstructionTrapClass::MayTrap => 1,
                InstructionTrapClass::Unsupported(_) => 2,
            })
            .collect();
        assert_eq!(classes.len(), 3);
    }
}
