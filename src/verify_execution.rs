//! Private lower-bound certificate for eliding operand-stack guards.
//!
//! The caller first validates the module and admits its total-instruction profile.
//! Unlike the ordinary verifier's upper-bound walk, this walk propagates minima
//! across actual control-flow edges, including reset-to-stream reentry.
use crate::bytecode::{Chunk, Module, Op};
use crate::verify::{VerifyError, op_depth_effect};
use alloc::collections::VecDeque;
use alloc::{format, string::String, vec};

fn error(chunk: &Chunk, message: impl Into<String>) -> VerifyError {
    VerifyError {
        chunk_name: chunk.name.clone(),
        message: message.into(),
    }
}
fn enqueue(
    chunk: &Chunk,
    depths: &mut [Option<i64>],
    queued: &mut [bool],
    work: &mut VecDeque<usize>,
    ip: usize,
    depth: i64,
) -> Result<(), VerifyError> {
    let slot = depths
        .get_mut(ip)
        .ok_or_else(|| error(chunk, "proof edge leaves instruction array"))?;
    if slot.is_none_or(|old| depth < old) {
        *slot = Some(depth);
        if !queued[ip] {
            queued[ip] = true;
            work.push_back(ip);
        }
    }
    Ok(())
}

fn prove_chunk(chunk: &Chunk) -> Result<(), VerifyError> {
    let n = chunk.ops.len();
    if n > crate::trap_analysis::TRAP_OPCODE_CAP {
        return Err(error(chunk, "operand proof instruction capacity exceeded"));
    }
    if u16::from(chunk.param_count) > chunk.local_count {
        return Err(error(chunk, "parameter count exceeds local storage"));
    }
    // A first arrival follows a simple path, with at most n positive effects.
    // Reset introduces at most one operand. Each later update strictly lowers
    // a nonnegative integer. This bounds successful updates and hence dequeues.
    let growth = chunk
        .ops
        .iter()
        .map(|op| op_depth_effect(op, chunk).1.max(0) as usize)
        .max()
        .unwrap_or(0);
    let max_depth = n
        .checked_mul(growth)
        .and_then(|v| v.checked_add(1))
        .ok_or_else(|| error(chunk, "proof depth bound overflow"))?;
    let budget = (n + 1)
        .checked_mul(max_depth + 1)
        .ok_or_else(|| error(chunk, "proof work bound overflow"))?;
    let stream = chunk
        .ops
        .iter()
        .position(|op| matches!(op, Op::Stream))
        .map(|ip| ip + 1);
    let mut depths = vec![None; n + 1];
    let mut queued = vec![false; n + 1];
    let mut work = VecDeque::new();
    enqueue(chunk, &mut depths, &mut queued, &mut work, 0, 0)?;
    for _ in 0..budget {
        let Some(ip) = work.pop_front() else {
            return Ok(());
        };
        queued[ip] = false;
        if ip == n {
            return Err(error(
                chunk,
                "operand proof requires explicit path termination",
            ));
        }
        let available = depths[ip].ok_or_else(|| error(chunk, "missing proof depth"))?;
        let op = chunk.ops[ip];
        let (required, delta) = op_depth_effect(&op, chunk);
        // Return actually pops one value. The ordinary model's terminal effect
        // does not prove that value exists and is insufficient for this mode.
        let required = if matches!(op, Op::Return) {
            1
        } else {
            required
        };
        if available < i64::from(required) {
            return Err(error(
                chunk,
                format!("operand proof at {ip} requires {required}, lower bound is {available}"),
            ));
        }
        let next = available
            .checked_add(i64::from(delta))
            .ok_or_else(|| error(chunk, "proof arithmetic overflow"))?;
        let mut edge =
            |target, depth| enqueue(chunk, &mut depths, &mut queued, &mut work, target, depth);
        match op {
            Op::Return | Op::Trap(_) => {}
            // Reset truncates to locals. The only admitted next entry is resume,
            // which supplies one operand before restarting after Stream.
            Op::Reset => edge(
                stream.ok_or_else(|| error(chunk, "Reset has no Stream"))?,
                1,
            )?,
            Op::If(target) | Op::BreakIf(target) => {
                edge(ip + 1, next)?;
                edge(target as usize, next)?;
            }
            Op::Else(target) | Op::EndLoop(target) | Op::Break(target) => {
                edge(target as usize, next)?
            }
            // Yield's net-zero effect includes exactly one future resume input.
            // The runtime protocol forbids breakpoint-resume on that state.
            _ => edge(ip + 1, next)?,
        }
    }
    if work.is_empty() {
        Ok(())
    } else {
        Err(error(chunk, "bounded operand proof did not converge"))
    }
}

pub(crate) fn verify_pop_elision(module: &Module) -> Result<(), VerifyError> {
    for chunk in &module.chunks {
        prove_chunk(chunk)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bytecode::BlockType;
    use alloc::vec::Vec;
    fn proof(ops: Vec<Op>) -> Result<(), VerifyError> {
        prove_chunk(&Chunk {
            name: String::from("proof"),
            ops,
            constants: vec![],
            struct_templates: vec![],
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: vec![],
            debug_pool: None,
        })
    }
    #[test]
    fn proof_elision_uses_the_lower_branch_depth() {
        assert!(
            proof(vec![
                Op::PushImmediate(1),
                Op::If(4),
                Op::PushImmediate(11),
                Op::Else(4),
                Op::EndIf,
                Op::Return
            ])
            .is_err()
        );
        assert!(
            proof(vec![
                Op::PushImmediate(1),
                Op::If(4),
                Op::PushImmediate(11),
                Op::Else(5),
                Op::PushImmediate(12),
                Op::EndIf,
                Op::Return
            ])
            .is_ok()
        );
    }
    #[test]
    fn proof_elision_refuses_a_consuming_back_edge() {
        assert!(
            proof(vec![
                Op::PushImmediate(11),
                Op::Loop(4),
                Op::PopN(1),
                Op::EndLoop(2),
                Op::Return
            ])
            .is_err()
        );
    }
    #[test]
    fn proof_elision_uses_the_lower_break_exit_depth() {
        assert!(
            proof(vec![
                Op::PushImmediate(1),
                Op::Loop(7),
                Op::Dup,
                Op::BreakIf(7),
                Op::PopN(1),
                Op::Break(7),
                Op::EndLoop(2),
                Op::Return
            ])
            .is_err()
        );
    }
    #[test]
    fn proof_elision_checks_reset_reentry_not_only_the_initial_path() {
        assert!(
            proof(vec![
                Op::PushImmediate(11),
                Op::PushImmediate(12),
                Op::Stream,
                Op::PopN(2),
                Op::PushImmediate(11),
                Op::Yield,
                Op::PopN(1),
                Op::Reset
            ])
            .is_err()
        );
        assert!(
            proof(vec![
                Op::PushImmediate(11),
                Op::Stream,
                Op::PopN(1),
                Op::PushImmediate(11),
                Op::Yield,
                Op::PopN(1),
                Op::Reset
            ])
            .is_ok()
        );
    }
}
