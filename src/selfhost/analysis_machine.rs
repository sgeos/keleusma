//! Shared transport for stateful analyzer protocols.
use super::executor::{StageExecutor, stage_vm};
use super::{Module, Value, VerificationError, VmState};
use alloc::boxed::Box;
use alloc::{format, vec, vec::Vec};

pub(super) struct AnalysisMachine<'a, const N: usize> {
    vm: Box<dyn StageExecutor + 'a>,
    shared: Vec<u8>,
    slots: [usize; N],
    started: bool,
}
pub(super) fn error(detail: impl core::fmt::Debug) -> VerificationError {
    VerificationError::Protocol {
        stage: "analysis",
        detail: format!("{detail:?}"),
    }
}
impl<'a, const N: usize> AnalysisMachine<'a, N> {
    pub(super) fn new(
        module: Module,
        arena: &'a crate::Arena,
        fields: &[&str; N],
    ) -> Result<Self, VerificationError> {
        let layout = module
            .data_layout
            .as_ref()
            .ok_or_else(|| error("missing layout"))?;
        let mut slots = [0; N];
        for (i, name) in fields.iter().enumerate() {
            slots[i] = layout
                .slots
                .iter()
                .position(|s| s.name == *name)
                .ok_or_else(|| error(name))?;
        }
        let vm = stage_vm(module, arena).map_err(|e| VerificationError::Execution {
            stage: "analysis",
            detail: format!("{e:?}"),
        })?;
        let shared = vec![0; vm.shared_data_bytes()];
        Ok(Self {
            vm,
            shared,
            slots,
            started: false,
        })
    }
    pub(super) fn set(&mut self, field: usize, value: i64) -> Result<(), VerificationError> {
        self.set_element(field, 0, value)
    }
    pub(super) fn set_element(
        &mut self,
        field: usize,
        index: usize,
        value: i64,
    ) -> Result<(), VerificationError> {
        self.vm
            .set_shared(
                &mut self.shared,
                self.slots[field] + index,
                Value::Int(value),
            )
            .map_err(error)
    }
    pub(super) fn get(&self, field: usize) -> Result<i64, VerificationError> {
        self.get_element(field, 0)
    }
    pub(super) fn get_element(&self, field: usize, index: usize) -> Result<i64, VerificationError> {
        match self
            .vm
            .get_shared(&self.shared, self.slots[field] + index)
            .map_err(error)?
        {
            Value::Int(v) => Ok(v),
            _ => Err(error("non-word response")),
        }
    }
    pub(super) fn command(&mut self, command: i64) -> Result<i64, VerificationError> {
        self.set(0, command)?;
        let mut state = if self.started {
            self.vm.resume_with_shared(&mut self.shared, Value::Int(0))
        } else {
            self.started = true;
            self.vm.call_with_shared(&mut self.shared, &[Value::Int(0)])
        }
        .map_err(|e| VerificationError::Execution {
            stage: "analysis",
            detail: format!("{e:?}"),
        })?;
        for _ in 0..3 {
            match state {
                VmState::Yielded(Value::Int(v)) => return Ok(v),
                VmState::Reset => {
                    state = self
                        .vm
                        .resume_with_shared(&mut self.shared, Value::Int(0))
                        .map_err(|e| VerificationError::Execution {
                            stage: "analysis",
                            detail: format!("{e:?}"),
                        })?
                }
                other => return Err(error(other)),
            }
        }
        Err(error("command did not yield"))
    }
}
