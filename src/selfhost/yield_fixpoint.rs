//! The stage schedules fixpoint rounds and decides membership; Rust transports facts.
use super::analysis_machine::{AnalysisMachine, error};
use super::*;
use alloc::collections::BTreeSet;
const FIELDS: [&str; 6] = [
    "yv.command",
    "yv.module_count",
    "yv.module_node",
    "yv.reply_fell",
    "yv.reply_yielded",
    "yv.query_node",
];

pub(super) fn always(module: &Module) -> Result<BTreeSet<usize>, VerificationError> {
    always_with_stage(module, verify_yield_kel_module())
}
fn always_with_stage(module: &Module, stage: Module) -> Result<BTreeSet<usize>, VerificationError> {
    if module.chunks.len() > 1024 {
        return Err(VerificationError::Capacity {
            stage: "yield fixpoint",
            resource: "chunks",
            limit: 1024,
            actual: module.chunks.len(),
        });
    }
    for chunk in &module.chunks {
        if chunk.ops.len() > crate::selfhost_host::VERIFY_OP_CAP {
            return Err(VerificationError::Capacity {
                stage: "yield fixpoint",
                resource: "opcodes",
                limit: crate::selfhost_host::VERIFY_OP_CAP,
                actual: chunk.ops.len(),
            });
        }
    }
    let persistent = required_persistent_capacity_for(&stage);
    let mut arena = Arena::try_with_capacity(DEFAULT_ARENA_CAPACITY + persistent).map_err(error)?;
    arena.resize_persistent(persistent).map_err(error)?;
    let mut machine = AnalysisMachine::new(stage.clone(), &arena, &FIELDS)?;
    machine.set(1, module.chunks.len() as i64)?;
    let mut code = machine.command(10)?;
    // At most one membership is added per productive round, plus the final round.
    let budget = (module.chunks.len() + 1)
        .checked_mul(module.chunks.len() * 2 + 2)
        .and_then(|n| n.checked_add(4))
        .ok_or_else(|| error("fixpoint budget overflow"))?;
    for _ in 0..budget {
        match code {
            0 => {
                let mut out = BTreeSet::new();
                for i in 0..module.chunks.len() {
                    machine.set(5, i as i64)?;
                    match machine.command(13)? {
                        0 => {}
                        1 => {
                            out.insert(i);
                        }
                        other => return Err(error(other)),
                    }
                }
                return Ok(out);
            }
            1 => {
                let index = usize::try_from(machine.get(2)?).map_err(error)?;
                let chunk = module
                    .chunks
                    .get(index)
                    .ok_or_else(|| error("invalid yield request"))?;
                let (fell, yielded) = super::validation::yield_using_lookup(
                    chunk,
                    0,
                    chunk.ops.len(),
                    |callee| {
                        machine.set(5, callee as i64)?;
                        match machine.command(13)? {
                            0 => Ok(false),
                            1 => Ok(true),
                            other => Err(error(other)),
                        }
                    },
                    stage.clone(),
                )?;
                machine.set(3, i64::from(fell))?;
                machine.set(4, i64::from(yielded))?;
                code = machine.command(12)?;
            }
            3 => code = machine.command(11)?,
            other => return Err(error(other)),
        }
    }
    Err(VerificationError::Incomplete {
        stage: "yield fixpoint",
        cause: 1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn membership_and_convergence_are_stage_owned() {
        let module = compile_src("loop main(n: Word) -> Word { yield n }");
        let expected = crate::verify::compute_always_yielding(&module);
        assert!(!expected.is_empty());
        assert_eq!(always(&module).unwrap(), expected);
        let source = read_stage("kel/verify_yield.kel");
        let marker = "if yv.reply_fell == 1 andalso yv.reply_yielded == 1 {";
        assert_eq!(source.matches(marker).count(), 1);
        let changed = compile_src(&source.replace(
            marker,
            "if yv.reply_fell == 0 andalso yv.reply_yielded == 1 {",
        ));
        assert!(always_with_stage(&module, changed).unwrap().is_empty());
        let control = compile_src("fn main() -> Word { 7 }");
        assert!(always(&control).unwrap().is_empty());
    }
}
