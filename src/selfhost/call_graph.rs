//! Keleusma owns call-graph traversal and cycle detection. Rust transports opcode facts.
use super::executor::{StageExecutor, stage_vm};
use super::{SelfHostError, analyze_kel_module};
use crate::bytecode::{Module, Op, Value};
use crate::vm::{DEFAULT_ARENA_CAPACITY, VmState, required_persistent_capacity_for};
use alloc::{format, vec, vec::Vec};

const FIELDS: [&str; 7] = [
    "wa.tf_command",
    "wa.cg_count",
    "wa.cg_node",
    "wa.cg_index",
    "wa.cg_size",
    "wa.cg_callee",
    "wa.cg_output",
];
fn unavailable(detail: impl core::fmt::Debug) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("call-graph stage unavailable: {detail:?}"),
    }
}

/// Callees precede callers in declaration/opcode DFS order. `None` means a cycle.
/// Capacity and protocol failures are errors, never cycle or source-rejection verdicts.
/// The first-pass stage admits 1,024 chunks and 65,535 opcodes per chunk.
pub fn call_order_via_kel(module: &Module) -> Result<Option<Vec<usize>>, SelfHostError> {
    call_order_with_stage(module, analyze_kel_module())
}

fn call_order_with_stage(
    module: &Module,
    stage: Module,
) -> Result<Option<Vec<usize>>, SelfHostError> {
    let layout = stage
        .data_layout
        .as_ref()
        .ok_or_else(|| unavailable("missing layout"))?;
    let mut slots = [0; 7];
    for (index, name) in FIELDS.iter().enumerate() {
        slots[index] = layout
            .slots
            .iter()
            .position(|slot| slot.name == *name)
            .ok_or_else(|| unavailable(name))?;
    }
    let persistent = required_persistent_capacity_for(&stage);
    let mut arena = crate::Arena::with_capacity(DEFAULT_ARENA_CAPACITY + persistent);
    arena.resize_persistent(persistent).map_err(unavailable)?;
    let mut vm = stage_vm(stage, &arena).map_err(unavailable)?;
    let mut shared = vec![0; vm.shared_data_bytes()];
    let set = |vm: &dyn StageExecutor, shared: &mut [u8], field: usize, value: usize| {
        let value = i64::try_from(value).map_err(unavailable)?;
        vm.set_shared(shared, slots[field], Value::Int(value))
            .map_err(unavailable)
    };
    let read = |vm: &dyn StageExecutor, shared: &[u8], field: usize| match vm
        .get_shared(shared, slots[field])
        .map_err(unavailable)?
    {
        Value::Int(value) => usize::try_from(value).map_err(unavailable),
        other => Err(unavailable(other)),
    };
    set(&vm, &mut shared, 0, 10)?;
    set(&vm, &mut shared, 1, module.chunks.len())?;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .map_err(unavailable)?;
    let mut order = Vec::new();
    order
        .try_reserve_exact(module.chunks.len())
        .map_err(unavailable)?;
    let mut seen = vec![false; module.chunks.len()];
    let budget = module.chunks.iter().try_fold(
        module
            .chunks
            .len()
            .checked_mul(3)
            .and_then(|n| n.checked_add(8))
            .ok_or_else(|| unavailable("budget overflow"))?,
        |total, chunk| {
            total
                .checked_add(chunk.ops.len())
                .ok_or_else(|| unavailable("budget overflow"))
        },
    )?;
    for _ in 0..budget {
        // Each command crosses at most one coroutine reset before its next yield.
        for _ in 0..3 {
            if !matches!(state, VmState::Reset) {
                break;
            }
            state = vm
                .resume_with_shared(&mut shared, Value::Int(0))
                .map_err(unavailable)?;
        }
        let code = match state {
            VmState::Yielded(Value::Int(code)) => code,
            other => return Err(unavailable(other)),
        };
        match code {
            0 => {
                if order.len() != module.chunks.len() {
                    return Err(unavailable("incomplete traversal"));
                }
                return Ok(Some(order));
            }
            -1 => return Ok(None),
            -3 => return Err(unavailable("capacity exceeds 1024 chunks or 65535 opcodes")),
            1 => {
                let node = read(&vm, &shared, 2)?;
                let chunk = module
                    .chunks
                    .get(node)
                    .ok_or_else(|| unavailable("invalid chunk request"))?;
                set(&vm, &mut shared, 4, chunk.ops.len())?;
            }
            2 => {
                let node = read(&vm, &shared, 2)?;
                let index = read(&vm, &shared, 3)?;
                let op = module
                    .chunks
                    .get(node)
                    .and_then(|chunk| chunk.ops.get(index))
                    .ok_or_else(|| unavailable("invalid opcode request"))?;
                let callee = match op {
                    Op::Call(callee, _) => i64::from(*callee),
                    _ => -1,
                };
                vm.set_shared(&mut shared, slots[5], Value::Int(callee))
                    .map_err(unavailable)?;
            }
            3 => {}
            4 => {
                let node = read(&vm, &shared, 6)?;
                let emitted = seen
                    .get_mut(node)
                    .ok_or_else(|| unavailable("invalid completion"))?;
                if *emitted {
                    return Err(unavailable("duplicate completion"));
                }
                *emitted = true;
                order.push(node);
            }
            other => return Err(unavailable(other)),
        }
        set(&vm, &mut shared, 0, 11)?;
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .map_err(unavailable)?;
    }
    Err(unavailable("command budget exhausted"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(edges: &[Vec<usize>]) -> Module {
        let mut module = super::super::compile_src("fn main() -> Word { 0 }");
        let template = module.chunks[0].clone();
        module.chunks = edges
            .iter()
            .enumerate()
            .map(|(i, callees)| {
                let mut chunk = template.clone();
                chunk.name = format!("node_{i}");
                chunk.ops = callees
                    .iter()
                    .map(|&c| Op::Call(u16::try_from(c).unwrap(), 0))
                    .collect();
                chunk.ops.push(Op::Return);
                chunk
            })
            .collect();
        module
    }

    #[test]
    fn stage_order_and_cycle_verdict_match_the_reference() {
        for edges in [
            vec![],
            vec![vec![]],
            vec![vec![], vec![], vec![]],
            vec![vec![1], vec![2], vec![]],
            vec![vec![2, 1, 2], vec![3], vec![3], vec![]],
            vec![vec![0]],
            vec![vec![1], vec![0]],
            vec![vec![], vec![2], vec![1]],
            vec![vec![65535]],
        ] {
            let module = graph(&edges);
            let expected = crate::verify::topological_call_order(&module).ok();
            assert_eq!(call_order_via_kel(&module).unwrap(), expected, "{edges:?}");
        }
    }

    #[test]
    fn graph_capacity_is_an_unavailable_result_and_the_last_slot_works() {
        let edges: Vec<_> = (0..1024)
            .map(|i| if i + 1 < 1024 { vec![i + 1] } else { vec![] })
            .collect();
        let module = graph(&edges);
        assert_eq!(
            call_order_via_kel(&module).unwrap().unwrap(),
            (0..1024).rev().collect::<Vec<_>>()
        );
        let too_many = graph(&vec![vec![]; 1025]);
        let error = call_order_via_kel(&too_many).unwrap_err();
        assert!(
            matches!(error, SelfHostError::Unsupported { detail } if detail.contains("capacity"))
        );
    }

    #[test]
    fn cycle_verdict_depends_on_the_stage_with_identical_inputs() {
        let source = super::super::read_stage("kel/analyze.kel");
        let original = "if cg.color[wa.cg_callee] == 1 { cg.bad = 0 - 1; }";
        assert_eq!(source.matches(original).count(), 1);
        let changed = source.replace(original, "if cg.color[wa.cg_callee] == 1 { cg.bad = 0; }");
        let without_cycle_rejection = super::super::compile_src(&changed);
        let cyclic = graph(&[vec![0]]);
        assert_eq!(call_order_via_kel(&cyclic).unwrap(), None);
        assert_eq!(
            call_order_with_stage(&cyclic, without_cycle_rejection.clone()).unwrap(),
            Some(vec![0])
        );
        let acyclic = graph(&[vec![1], vec![]]);
        assert_eq!(
            call_order_with_stage(&acyclic, without_cycle_rejection).unwrap(),
            call_order_via_kel(&acyclic).unwrap()
        );
    }

    #[test]
    fn opcode_capacity_distinguishes_the_last_admitted_count() {
        let mut module = graph(&[vec![]]);
        module.chunks[0].ops = vec![Op::Return; 65535];
        assert_eq!(call_order_via_kel(&module).unwrap(), Some(vec![0]));
        module.chunks[0].ops.push(Op::Return);
        assert!(
            matches!(call_order_via_kel(&module), Err(SelfHostError::Unsupported { detail }) if detail.contains("capacity"))
        );
    }

    #[test]
    fn current_stage_graphs_preserve_reference_order() {
        let mut paths: Vec<_> = std::fs::read_dir("src/selfhost/kel")
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|s| s == "kel"))
            .collect();
        paths.sort();
        assert_eq!(paths.len(), 14);
        for path in paths {
            let module = super::super::compile_src(&std::fs::read_to_string(&path).unwrap());
            let expected = crate::verify::topological_call_order(&module).unwrap();
            assert_eq!(
                call_order_via_kel(&module).unwrap().unwrap(),
                expected,
                "{}",
                path.display()
            );
        }
    }
}
