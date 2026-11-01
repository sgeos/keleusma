//! Data operands are translated by codegen.kel using parser-produced address maps.
use super::executor::{StageExecutor, stage_vm};
use super::{ParsedFn, SelfHostError};
use crate::bytecode::{Module, Value};
use crate::selfhost_host::{PARSE_DATA_FIELD_BASE_TAG, PARSE_FIELDS_CAP};
use crate::vm::{DEFAULT_ARENA_CAPACITY, VmState, required_persistent_capacity_for};
use alloc::boxed::Box;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

fn error(message: impl core::fmt::Debug) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("data relocation: {message:?}"),
    }
}

pub(super) struct Plan {
    lengths: Vec<i64>,
    targets: Vec<i64>,
}
impl Plan {
    pub(super) fn from_records(records: &[(i64, i64)]) -> Result<Self, SelfHostError> {
        let mut lengths = Vec::new();
        let mut targets = Vec::new();
        let mut visibility = 2;
        for &(kind, value) in records {
            match kind {
                9 => visibility = value % 4,
                4 if visibility < 2 => lengths.push(1),
                8 if visibility < 2 => {
                    *lengths
                        .last_mut()
                        .ok_or_else(|| error("length without field"))? = value
                }
                PARSE_DATA_FIELD_BASE_TAG => {
                    if value < 0 {
                        return Err(error("negative address record"));
                    }
                    let index = usize::try_from(value / 16777216).map_err(error)?;
                    let base = value % 16777216;
                    if index == targets.len() {
                        targets.push(base);
                    } else {
                        *targets
                            .get_mut(index)
                            .ok_or_else(|| error("address record gap"))? = base;
                    }
                }
                _ => {}
            }
        }
        if targets.len() != lengths.len() || lengths.len() > PARSE_FIELDS_CAP {
            return Err(error("declarations and final address table disagree"));
        }
        Ok(Self { lengths, targets })
    }
    pub(super) fn relocate(&self, function: &mut ParsedFn) -> Result<(), SelfHostError> {
        let count = function.data_bases.len();
        let targets = self
            .targets
            .get(..count)
            .ok_or_else(|| error("snapshot exceeds declarations"))?;
        if function.data_bases.as_slice() == targets {
            return Ok(());
        }
        let module = stage();
        let persistent = required_persistent_capacity_for(&module);
        let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).map_err(error)?;
        let mut machine = Machine::new(
            module,
            &arena,
            &function.data_bases,
            &self.lengths[..count],
            targets,
        )?;
        for records in [&mut function.guard, &mut function.body] {
            relocate_records(&mut machine, records)?;
        }
        function.data_bases = Arc::new(targets.to_vec());
        Ok(())
    }
}

/// A handled indexed write's root record carries its offset slot plus one at
/// this radix, above the data operand and its kind lane. A bare write's record
/// 36 carries the slot alone, below it, and holds no data address.
const STORE_ROOT_SLOT: i64 = 1 << 51;

fn relocate_records(
    machine: &mut Machine<'_>,
    records: &mut [(i64, i64)],
) -> Result<(), SelfHostError> {
    for (kind, value) in records {
        if (11..=14).contains(kind) {
            *value = machine.translate(*kind, *value)?;
        } else if *kind == 36 && *value >= STORE_ROOT_SLOT {
            let operand = *value % STORE_ROOT_SLOT;
            *value = *value - operand + machine.translate(13, operand)?;
        }
    }
    Ok(())
}

fn stage() -> Module {
    static MODULE: std::sync::OnceLock<Module> = std::sync::OnceLock::new();
    MODULE
        .get_or_init(|| super::compile_src(include_str!("kel/codegen.kel")))
        .clone()
}

struct Machine<'a> {
    vm: Box<dyn StageExecutor + 'a>,
    shared: Vec<u8>,
    kind: usize,
    argument: usize,
    started: bool,
}
impl<'a> Machine<'a> {
    fn new(
        module: Module,
        arena: &'a crate::Arena,
        sources: &[i64],
        lengths: &[i64],
        targets: &[i64],
    ) -> Result<Self, SelfHostError> {
        if sources.len() != lengths.len()
            || sources.len() != targets.len()
            || sources.len() > PARSE_FIELDS_CAP
        {
            return Err(error("relocation table lengths disagree"));
        }
        let layout = module
            .data_layout
            .as_ref()
            .ok_or_else(|| error("missing stage layout"))?;
        let find = |name: &str| {
            layout
                .slots
                .iter()
                .position(|s| s.name == name)
                .ok_or_else(|| error(name))
        };
        let slots = [
            find("ast.relocate_mode")?,
            find("ast.relocate_count")?,
            find("ast.relocate_kind")?,
            find("ast.relocate_arg")?,
            find("ast.relocate_source")?,
            find("ast.relocate_length")?,
            find("ast.relocate_target")?,
        ];
        let vm = stage_vm(module, arena).map_err(error)?;
        let mut shared = vec![0; vm.shared_data_bytes()];
        vm.set_shared(&mut shared, slots[0], Value::Int(1))
            .map_err(error)?;
        vm.set_shared(
            &mut shared,
            slots[1],
            Value::Int(i64::try_from(sources.len()).map_err(error)?),
        )
        .map_err(error)?;
        for (base, values) in [
            (slots[4], sources),
            (slots[5], lengths),
            (slots[6], targets),
        ] {
            for (i, &value) in values.iter().enumerate() {
                vm.set_shared(&mut shared, base + i, Value::Int(value))
                    .map_err(error)?;
            }
        }
        Ok(Self {
            vm,
            shared,
            kind: slots[2],
            argument: slots[3],
            started: false,
        })
    }
    fn translate(&mut self, kind: i64, argument: i64) -> Result<i64, SelfHostError> {
        self.vm
            .set_shared(&mut self.shared, self.kind, Value::Int(kind))
            .map_err(error)?;
        self.vm
            .set_shared(&mut self.shared, self.argument, Value::Int(argument))
            .map_err(error)?;
        let mut state = if self.started {
            self.vm.resume_with_shared(&mut self.shared, Value::Int(0))
        } else {
            self.started = true;
            self.vm.call_with_shared(&mut self.shared, &[Value::Int(0)])
        }
        .map_err(error)?;
        for _ in 0..3 {
            match state {
                VmState::Yielded(Value::Int(value)) if value >= 0 => return Ok(value),
                VmState::Reset => {
                    state = self
                        .vm
                        .resume_with_shared(&mut self.shared, Value::Int(0))
                        .map_err(error)?
                }
                other => {
                    return Err(error(format!(
                        "operand {kind}/{argument} refused with {other:?}"
                    )));
                }
            }
        }
        Err(error("translation did not yield"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPACE: i64 = 16777216;

    fn with_machine(
        sources: &[i64],
        lengths: &[i64],
        targets: &[i64],
        check: impl FnOnce(&mut Machine<'_>),
    ) {
        let module = stage();
        let persistent = required_persistent_capacity_for(&module);
        let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).unwrap();
        let mut machine = Machine::new(module, &arena, sources, lengths, targets).unwrap();
        check(&mut machine);
    }

    #[test]
    fn scalar_and_indexed_operands_preserve_offsets_and_counts() {
        with_machine(&[0, 4], &[4, 2], &[2, 0], |machine| {
            for kind in [11, 12] {
                assert_eq!(machine.translate(kind, 0).unwrap(), 2);
                assert_eq!(machine.translate(kind, 3).unwrap(), 5);
                assert_eq!(machine.translate(kind, 5).unwrap(), 1);
            }
            for kind in [13, 14] {
                assert_eq!(machine.translate(kind, 4 * SPACE).unwrap(), 4 * SPACE + 2);
                assert_eq!(
                    machine.translate(kind, 2 * SPACE + 1).unwrap(),
                    2 * SPACE + 3
                );
                assert_eq!(machine.translate(kind, SPACE + 5).unwrap(), SPACE + 1);
            }
        });
        with_machine(&[0], &[SPACE], &[0], |machine| {
            assert_eq!(machine.translate(11, SPACE - 1).unwrap(), SPACE - 1);
            for kind in [13, 14] {
                assert_eq!(
                    machine.translate(kind, (SPACE - 1) * SPACE).unwrap(),
                    (SPACE - 1) * SPACE
                );
                assert!(machine.translate(kind, SPACE * SPACE).is_err());
            }
        });
    }

    #[test]
    fn indexed_read_kinds_survive_relocation_and_reserved_lanes_refuse() {
        // Stores gained the read's kind lane with the index-first write of
        // 2026-10-21, whose probe needs the element's fallback. Both kinds keep
        // lanes 0, 2 and 3 and refuse the rest rather than mask them.
        with_machine(&[0], &[4], &[8], |machine| {
            for kind in [13, 14] {
                for scalar in [0, 2, 3] {
                    let arg = scalar * SPACE * SPACE + 2 * SPACE + 1;
                    assert_eq!(machine.translate(kind, arg).unwrap(), arg + 8);
                }
                for scalar in [1, 4, 255] {
                    let arg = scalar * SPACE * SPACE + 2 * SPACE + 1;
                    assert!(machine.translate(kind, arg).is_err());
                    assert_eq!(machine.translate(11, 1).unwrap(), 9);
                }
            }
        });
    }

    #[test]
    fn a_handled_write_root_relocates_its_operand_and_keeps_its_slot() {
        with_machine(&[0], &[4], &[8], |machine| {
            let operand = 3 * SPACE * SPACE + 2 * SPACE + 1;
            let mut records = vec![(36, operand + 6 * STORE_ROOT_SLOT), (36, 5)];
            relocate_records(machine, &mut records).unwrap();
            assert_eq!(
                records,
                vec![(36, operand + 8 + 6 * STORE_ROOT_SLOT), (36, 5)]
            );
        });
    }

    #[test]
    fn invalid_operands_refuse_and_the_next_request_recovers() {
        with_machine(&[1], &[3], &[SPACE - 3], |machine| {
            for (kind, argument) in [
                (10, 1),
                (15, 1),
                (11, -1),
                (11, SPACE),
                (11, 0),
                (12, 4),
                (13, 1),
                (13, 4 * SPACE + 1),
                (14, 2 * SPACE + 3),
                (13, i64::MAX),
            ] {
                assert!(
                    machine.translate(kind, argument).is_err(),
                    "{kind}/{argument}"
                );
                assert_eq!(machine.translate(11, 1).unwrap(), SPACE - 3);
            }
        });
    }

    #[test]
    fn invalid_or_ambiguous_maps_refuse() {
        for (sources, lengths, targets) in [
            (vec![], vec![], vec![]),
            (vec![0, 0], vec![1, 1], vec![1, 2]),
            (vec![-1], vec![1], vec![0]),
            (vec![SPACE], vec![1], vec![0]),
            (vec![0], vec![0], vec![0]),
            (vec![0], vec![-1], vec![0]),
            (vec![0], vec![SPACE + 1], vec![0]),
            (vec![0], vec![2], vec![SPACE - 1]),
            (vec![0], vec![1], vec![-1]),
            (vec![0], vec![1], vec![SPACE]),
        ] {
            with_machine(&sources, &lengths, &targets, |machine| {
                assert!(machine.translate(11, 0).is_err());
            });
        }
    }

    #[test]
    fn address_records_require_contiguous_runtime_field_ordinals() {
        for records in [
            vec![(PARSE_DATA_FIELD_BASE_TAG, -1)],
            vec![(PARSE_DATA_FIELD_BASE_TAG, SPACE)],
            vec![(9, 0), (8, 2)],
            vec![(9, 0), (4, 0)],
            vec![(PARSE_DATA_FIELD_BASE_TAG, 0)],
        ] {
            assert!(Plan::from_records(&records).is_err());
        }
        let plan = Plan::from_records(&[
            (9, 0),
            (4, 0),
            (8, 2),
            (PARSE_DATA_FIELD_BASE_TAG, 0),
            (9, 2),
            (4, 0), // A constant is not a runtime slot.
            (9, 1),
            (4, 0),
            (PARSE_DATA_FIELD_BASE_TAG, 1),
            (PARSE_DATA_FIELD_BASE_TAG, SPACE),
        ])
        .unwrap();
        assert_eq!(plan.lengths, [2, 1]);
        assert_eq!(plan.targets, [1, 0]);
    }
}
