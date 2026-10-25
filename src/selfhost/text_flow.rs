//! Text boundary decisions run in analyze.kel. Rust stores the abstract values and
//! supplies opcode facts, stack effects, and callee results in dependency order.
use super::{SelfHostError, analyze_kel_module};
use crate::bytecode::{Chunk, ConstValue, Module, Op, Value};
use crate::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

const FIELDS: [&str; 17] = [
    "wa.tf_command",
    "wa.tf_kind",
    "wa.tf_required",
    "wa.tf_delta",
    "wa.tf_top",
    "wa.tf_second",
    "wa.tf_local",
    "wa.tf_const",
    "wa.tf_callee",
    "wa.tf_callee_known",
    "wa.tf_index",
    "wa.tf_locals",
    "wa.tf_pops",
    "wa.tf_pushes",
    "wa.tf_value",
    "wa.tf_write",
    "wa.tf_write_value",
];
fn error(detail: impl core::fmt::Debug) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("text boundary analysis: {detail:?}"),
    }
}
struct Machine<'a> {
    vm: Vm<'static, 'a>,
    shared: Vec<u8>,
    slots: [usize; 17],
    started: bool,
}
impl<'a> Machine<'a> {
    fn new(module: Module, arena: &'a crate::Arena) -> Result<Self, SelfHostError> {
        let layout = module
            .data_layout
            .as_ref()
            .ok_or_else(|| error("missing stage layout"))?;
        let mut slots = [0; 17];
        for (i, field) in FIELDS.iter().enumerate() {
            slots[i] = layout
                .slots
                .iter()
                .position(|s| s.name == *field)
                .ok_or_else(|| error(field))?;
        }
        let vm = Vm::new(module, arena).map_err(error)?;
        let shared = vec![0; vm.shared_data_bytes()];
        Ok(Self {
            vm,
            shared,
            slots,
            started: false,
        })
    }
    fn set(&mut self, field: usize, value: i64) -> Result<(), SelfHostError> {
        self.vm
            .set_shared(&mut self.shared, self.slots[field], Value::Int(value))
            .map_err(error)
    }
    fn get(&self, field: usize) -> Result<i64, SelfHostError> {
        match self
            .vm
            .get_shared(&self.shared, self.slots[field])
            .map_err(error)?
        {
            Value::Int(value) => Ok(value),
            _ => Err(error("non-word response")),
        }
    }
    fn command(&mut self, command: i64) -> Result<i64, SelfHostError> {
        self.set(0, command)?;
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
                        "command {command} refused or stopped with {other:?}"
                    )));
                }
            }
        }
        Err(error("command did not reach its yield"))
    }
    fn chunk(&mut self, chunk: &Chunk, callees: &[bool]) -> Result<(bool, bool), SelfHostError> {
        self.command(1)?;
        let mut locals = Vec::new();
        locals
            .try_reserve_exact(usize::from(chunk.local_count))
            .map_err(error)?;
        locals.resize(usize::from(chunk.local_count), 0i64);
        let mut stack: Vec<i64> = Vec::new();
        self.set(11, i64::from(chunk.local_count))?;
        for op in &chunk.ops {
            let (kind, index) = match op {
                Op::Const(i) => (1, usize::from(*i)),
                Op::GetLocal(i) => (2, usize::from(*i)),
                Op::SetLocal(i) => (3, usize::from(*i)),
                Op::Dup => (4, 0),
                Op::Add => (5, 0),
                Op::Call(i, _) => (6, usize::from(*i)),
                Op::CallVerifiedNative(_, n) | Op::CallExternalNative(_, n) => {
                    (7, usize::from(*n & 0x7F))
                }
                Op::Return => (8, 0),
                Op::Yield => (9, 0),
                Op::Trap(_) => (10, 0),
                _ => (0, 0),
            };
            let (required, delta) = crate::verify::op_depth_effect(op, chunk);
            let constant = matches!(chunk.constants.get(index), Some(ConstValue::StaticStr(_)));
            let summary = callees.get(index).copied();
            for (field, value) in [
                (1, kind),
                (2, i64::from(required)),
                (3, i64::from(delta)),
                (4, stack.last().copied().unwrap_or(0)),
                (
                    5,
                    stack
                        .len()
                        .checked_sub(2)
                        .and_then(|i| stack.get(i))
                        .copied()
                        .unwrap_or(0),
                ),
                (6, locals.get(index).copied().unwrap_or(0)),
                (7, i64::from(constant)),
                (8, i64::from(summary.unwrap_or(false))),
                (9, i64::from(summary.is_some())),
                (10, i64::try_from(index).map_err(error)?),
            ] {
                self.set(field, value)?;
            }
            self.command(2)?;
            let pops = usize::try_from(self.get(12)?).map_err(error)?;
            let pushes = usize::try_from(self.get(13)?).map_err(error)?;
            let value = self.get(14)?;
            if !(0..=1).contains(&value) {
                return Err(error("invalid abstract value"));
            }
            match self.get(15)? {
                0 => {}
                1 => {
                    let value = self.get(16)?;
                    if !(0..=1).contains(&value) {
                        return Err(error("invalid local value"));
                    }
                    *locals
                        .get_mut(index)
                        .ok_or_else(|| error("invalid local write"))? = value;
                }
                _ => return Err(error("invalid write flag")),
            }
            stack.truncate(stack.len().saturating_sub(pops));
            stack.try_reserve(pushes).map_err(error)?;
            let length = stack
                .len()
                .checked_add(pushes)
                .ok_or_else(|| error("abstract stack overflow"))?;
            stack.resize(length, value);
        }
        self.set(4, stack.last().copied().unwrap_or(0))?;
        let flags = self.command(3)?;
        if flags > 3 {
            return Err(error("invalid boundary result"));
        }
        Ok((flags & 1 != 0, flags & 2 != 0))
    }
    fn module(&mut self, module: &Module) -> Result<Option<Vec<(bool, bool)>>, SelfHostError> {
        // Scheduling only. No reference text-flow result is consumed.
        let Some(order) = super::call_order_via_kel(module)? else {
            return Ok(None);
        };
        let mut returns = vec![false; module.chunks.len()];
        let mut results = vec![(false, false); module.chunks.len()];
        for index in order {
            let result = self.chunk(&module.chunks[index], &returns)?;
            returns[index] = result.0;
            results[index] = result;
        }
        Ok(Some(results))
    }
}

pub(super) fn ephemeral_flag(module: &Module, source_flags: i64) -> Result<u8, SelfHostError> {
    let stage = analyze_kel_module();
    let persistent = required_persistent_capacity_for(&stage);
    let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
    arena.resize_persistent(persistent).map_err(error)?;
    let mut machine = Machine::new(stage, &arena)?;
    let boundaries = machine.module(module)?;
    let entry = module
        .entry_point
        .and_then(|i| boundaries.as_ref().and_then(|v| v.get(i)));
    machine.set(4, i64::from(entry.is_some_and(|&(r, y)| r || y)))?;
    machine.set(5, source_flags & 1)?;
    machine.set(6, (source_flags / 2) & 1)?;
    machine.set(9, i64::from(entry.is_some()))?;
    machine.set(10, i64::from(module.private_data_bytes))?;
    let flag = machine.command(4)?;
    u8::try_from(flag)
        .ok()
        .filter(|v| *v <= 1)
        .ok_or_else(|| error("invalid module flag"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_boundaries_match_the_reference_over_calls_locals_and_control_flow() {
        for source in [
            "fn main() -> Text { \"x\" }",
            "fn f() -> Text { \"x\" } fn main() -> Text { f() }",
            "fn f(x: Text) -> Text { x } fn main() -> Text { f(\"x\") }",
            "fn main(b: bool) -> Text { if b { \"a\" } else { \"b\" } }",
            "fn main() -> Word { let s = \"a\"; let b = s == \"b\"; if b { 1 } else { 2 } }",
            "yield main() -> Text { yield \"x\"; \"x\" }",
        ] {
            let module = super::super::compile_src(source);
            let expected = crate::verify::module_chunk_text_analyses(&module).unwrap();
            let stage = analyze_kel_module();
            let persistent = required_persistent_capacity_for(&stage);
            let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
            arena.resize_persistent(persistent).unwrap();
            let mut machine = Machine::new(stage, &arena).unwrap();
            let actual = machine.module(&module).unwrap().unwrap();
            let expected: Vec<_> = expected
                .iter()
                .map(|a| (a.returns_text, a.yields_text))
                .collect();
            assert_eq!(actual, expected, "{source}");
        }
    }

    #[test]
    fn protocol_refuses_malformed_fields_and_recovers() {
        let stage = analyze_kel_module();
        let persistent = required_persistent_capacity_for(&stage);
        let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).unwrap();
        let mut machine = Machine::new(stage, &arena).unwrap();
        machine.command(1).unwrap();
        assert!(machine.command(99).is_err());
        machine.set(1, 99).unwrap();
        assert!(machine.command(2).is_err());
        machine.set(1, 1).unwrap();
        machine.set(2, 0).unwrap();
        machine.set(3, 1).unwrap();
        machine.set(7, 2).unwrap();
        assert!(machine.command(2).is_err());
        machine.set(7, 1).unwrap();
        machine.command(2).unwrap();
        assert_eq!(machine.get(14).unwrap(), 1);
        machine.set(4, 1).unwrap();
        assert_eq!(machine.command(3).unwrap(), 1);
        machine.set(4, 0).unwrap();
        machine.set(5, 0).unwrap();
        machine.set(6, 1).unwrap();
        machine.set(9, 0).unwrap();
        machine.set(10, 0).unwrap();
        assert_eq!(machine.command(4).unwrap(), 0);
        machine.set(9, 1).unwrap();
        assert_eq!(machine.command(4).unwrap(), 1);
        machine.set(10, 1).unwrap();
        assert_eq!(machine.command(4).unwrap(), 0);
    }

    #[test]
    fn cycle_fallback_and_native_operand_conventions_match_reference() {
        let mut module = super::super::compile_src("fn main() -> Text { \"x\" }");
        module.chunks[0].ops = vec![Op::Call(0, 0), Op::Return];
        assert!(crate::verify::module_chunk_text_analyses(&module).is_err());
        assert_eq!(ephemeral_flag(&module, 2).unwrap(), 0);
        module.chunks[0].ops = vec![
            Op::Const(0),
            Op::CallExternalNative(0, 128),
            Op::PopN(1),
            Op::Return,
        ];
        let expected = crate::verify::module_chunk_text_analyses(&module).unwrap();
        let stage = analyze_kel_module();
        let persistent = required_persistent_capacity_for(&stage);
        let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
        arena.resize_persistent(persistent).unwrap();
        let mut machine = Machine::new(stage, &arena).unwrap();
        let actual = machine.module(&module).unwrap().unwrap();
        assert_eq!(
            actual[0],
            (expected[0].returns_text, expected[0].yields_text)
        );
    }
}
