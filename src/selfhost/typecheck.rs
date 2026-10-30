//! Production transport for scoped source facts and the Keleusma rejection stage.
//! The host interns identities and copies rows. Type resolution remains in the stage.

use super::executor::{StageExecutor, stage_vm};
use super::{CallTypeBatch, CallTypeInputs, ExprOperand};
use crate::Arena;
use crate::bytecode::{Module, SlotVisibility, Value};
use crate::selfhost_host::ParseRecordError;
use crate::vm::{DEFAULT_ARENA_CAPACITY, VmError, VmState, required_persistent_capacity_for};
use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

/// Unavailable type evidence must never be interpreted as source rejection.
#[derive(Debug)]
pub enum TypeCheckError {
    /// A parser limitation or transport failure, retaining its diagnostic classification.
    Parse(ParseRecordError),
    /// An input column exceeds the capacity declared by the stage.
    Capacity {
        /// Shared input column whose bound was exceeded.
        table: String,
        /// Number of rows supplied by the caller.
        actual: usize,
        /// Number of rows declared by the compiled stage.
        maximum: usize,
    },
    /// Invalid fact encoding or an unexpected stage response.
    Protocol(String),
    /// Stage loading or execution failed.
    Execution(VmError),
    /// The embedded rejection stage could not be compiled for bootstrap.
    Bootstrap(String),
    /// An upstream extraction path still panics instead of returning a diagnostic.
    ExtractionPanic(String),
}

impl core::fmt::Display for TypeCheckError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "{error}"),
            Self::Capacity {
                table,
                actual,
                maximum,
            } => write!(
                f,
                "type table {table} has {actual} rows, exceeding {maximum}"
            ),
            Self::Protocol(detail) => write!(f, "type transport protocol error ({detail})"),
            Self::Execution(error) => write!(f, "type stage execution failed ({error:?})"),
            Self::Bootstrap(detail) => write!(f, "type stage bootstrap failed ({detail})"),
            Self::ExtractionPanic(detail) => {
                write!(f, "type input extraction unavailable ({detail})")
            }
        }
    }
}

impl std::error::Error for TypeCheckError {}

/// Check source facts without invoking the reference compiler on the user program.
/// False is an explicit parser source diagnostic or a rejection-stage verdict.
/// Composite coverage remains incomplete. True is not a complete language admission proof.
pub fn type_check_source_via_kel(source: &str) -> Result<bool, TypeCheckError> {
    let extracted = std::panic::catch_unwind(|| super::try_call_type_inputs_from_pipeline(source))
        .map_err(|payload| {
            TypeCheckError::ExtractionPanic(
                payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_else(|| "unknown extraction panic".into()),
            )
        })?;
    match extracted {
        Ok(inputs) => type_check_inputs_via_kel(&inputs),
        Err(error) if error.is_source_rejection() => Ok(false),
        Err(error) => Err(TypeCheckError::Parse(error)),
    }
}

/// Execute scoped facts through the embedded stage. Every batch has a fresh arena and
/// buffer, so private solver state and unused columns cannot leak between source heads.
pub fn type_check_inputs_via_kel(inputs: &CallTypeInputs) -> Result<bool, TypeCheckError> {
    let source = include_str!("kel/verify_types.kel");
    let tokens =
        crate::lexer::tokenize(source).map_err(|e| TypeCheckError::Bootstrap(format!("{e:?}")))?;
    let ast =
        crate::parser::parse(&tokens).map_err(|e| TypeCheckError::Bootstrap(format!("{e:?}")))?;
    let module =
        crate::compiler::compile(&ast).map_err(|e| TypeCheckError::Bootstrap(format!("{e:?}")))?;
    let layout = Layout::from_module(&module)?;
    let mut declarations = BTreeMap::new();
    let mut arities = Vec::new();
    for (index, (name, arity, tags)) in inputs.declarations.iter().enumerate() {
        if usize::try_from(*arity).ok() != Some(tags.len()) {
            return Err(protocol("declaration arity does not match parameter facts"));
        }
        declarations.insert(name.as_str(), word(index)?);
        arities.push(*arity);
    }
    layout.check("dparams", arities.len())?;
    for batch in &inputs.batches {
        let encoded = encode(batch, &declarations)?;
        if !run_batch(&module, &layout, &arities, &encoded)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn protocol(detail: &str) -> TypeCheckError {
    TypeCheckError::Protocol(detail.into())
}
fn word(value: usize) -> Result<i64, TypeCheckError> {
    i64::try_from(value).map_err(|_| protocol("row index exceeds Word"))
}

/// Ranges come from compiled shared-slot names, not a second copy of offset arithmetic.
struct Layout(BTreeMap<String, (usize, usize)>);

impl Layout {
    fn from_module(module: &Module) -> Result<Self, TypeCheckError> {
        let data = module
            .data_layout
            .as_ref()
            .ok_or_else(|| protocol("stage has no data layout"))?;
        let mut fields: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for (index, slot) in data.slots.iter().enumerate().take(data.shared_layout.len()) {
            if slot.visibility != SlotVisibility::Shared || data.shared_layout[index].kind != 3 {
                return Err(protocol("type inputs must be shared Word slots"));
            }
            let entry = fields.entry(slot.name.clone()).or_insert((index, 0));
            if entry.0 + entry.1 != index {
                return Err(protocol("noncontiguous stage field"));
            }
            entry.1 += 1;
        }
        Ok(Self(fields))
    }

    fn check(&self, field: &str, count: usize) -> Result<usize, TypeCheckError> {
        let name = format!("ty.{field}");
        let &(start, maximum) = self
            .0
            .get(&name)
            .ok_or_else(|| protocol("missing stage field"))?;
        if count > maximum {
            return Err(TypeCheckError::Capacity {
                table: name,
                actual: count,
                maximum,
            });
        }
        Ok(start)
    }

    fn write(
        &self,
        vm: &dyn StageExecutor,
        shared: &mut [u8],
        field: &str,
        values: &[i64],
    ) -> Result<(), TypeCheckError> {
        let start = self.check(field, values.len())?;
        for (i, &value) in values.iter().enumerate() {
            vm.set_shared(shared, start + i, Value::Int(value))
                .map_err(TypeCheckError::Execution)?;
        }
        Ok(())
    }
}

struct Encoded {
    sites: Vec<(i64, i64)>,
    nodes: Vec<(i64, i64, i64, i64, i64)>,
    bindings: Vec<(i64, i64, i64)>,
}

fn encode(
    batch: &CallTypeBatch,
    declarations: &BTreeMap<&str, i64>,
) -> Result<Encoded, TypeCheckError> {
    let mut names = BTreeMap::new();
    let mut intern = |name: &String| -> Result<(), TypeCheckError> {
        let next = word(names.len())?
            .checked_add(1)
            .ok_or_else(|| protocol("name index overflow"))?;
        names.entry(name.clone()).or_insert(next);
        Ok(())
    };
    for (name, _, form, target) in &batch.bindings {
        intern(name)?;
        if *form == 1 {
            intern(target)?;
        }
    }
    for (_, form, name) in batch
        .arguments
        .iter()
        .map(|(_, operand)| operand)
        .chain(batch.expressions.iter().flat_map(|(_, a, b)| [a, b]))
    {
        if *form == 1 {
            intern(name)?;
        }
    }
    let lookup = |name: &String| {
        names
            .get(name)
            .copied()
            .ok_or_else(|| protocol("uninterned operand name"))
    };
    let operand = |(value, form, name): &ExprOperand| -> Result<(i64, i64), TypeCheckError> {
        match *form {
            0 if *value >= 0 => Ok((*value, 0)),
            1 => Ok((lookup(name)?, 1)),
            3 => Ok((*value, 3)),
            _ => Err(protocol("unsupported operand form")),
        }
    };
    let mut bindings = Vec::new();
    for (name, value, form, target) in &batch.bindings {
        let value = match *form {
            0 if *value >= 0 => *value,
            1 => lookup(target)?,
            2 if usize::try_from(*value).is_ok_and(|v| v < batch.expressions.len()) => *value,
            _ => return Err(protocol("invalid binding form or expression index")),
        };
        bindings.push((lookup(name)?, value, *form));
    }
    let mut nodes = Vec::new();
    for (kind, a, b) in &batch.expressions {
        if !(1..=19).contains(kind) {
            return Err(protocol("unsupported expression kind"));
        }
        let (a, af) = operand(a)?;
        let (b, bf) = operand(b)?;
        nodes.push((*kind, a, af, b, bf));
    }
    for (required, actual) in &batch.arguments {
        if *required < 0 {
            return Err(protocol("negative required type"));
        }
        let (value, form) = operand(actual)?;
        nodes.push((8, *required, 0, value, form));
    }
    let mut sites = Vec::new();
    for (name, count) in &batch.sites {
        if *count < 0 {
            return Err(protocol("negative call arity"));
        }
        let declaration = declarations
            .get(name.as_str())
            .ok_or_else(|| protocol("call has no declaration"))?;
        sites.push((*declaration, *count));
    }
    Ok(Encoded {
        sites,
        nodes,
        bindings,
    })
}

fn constant(
    vm: &mut dyn StageExecutor,
    module: &Module,
    name: &str,
) -> Result<i64, TypeCheckError> {
    let index = module
        .chunks
        .iter()
        .position(|chunk| chunk.name == name)
        .ok_or_else(|| protocol("missing protocol constant function"))?;
    match vm
        .call_function(index, &[])
        .map_err(TypeCheckError::Execution)?
    {
        VmState::Finished(Value::Int(value)) => Ok(value),
        _ => Err(protocol("protocol constant did not return Word")),
    }
}

fn run_batch(
    module: &Module,
    layout: &Layout,
    arities: &[i64],
    batch: &Encoded,
) -> Result<bool, TypeCheckError> {
    let persistent = required_persistent_capacity_for(module);
    let capacity = DEFAULT_ARENA_CAPACITY
        .checked_add(persistent)
        .ok_or_else(|| protocol("arena size overflow"))?;
    let mut arena = Arena::with_capacity(capacity);
    arena
        .resize_persistent(persistent)
        .map_err(|e| TypeCheckError::Protocol(format!("arena allocation failed ({e:?})")))?;
    let mut vm = stage_vm(module.clone(), &arena).map_err(TypeCheckError::Execution)?;
    let mut shared = vec![0; vm.shared_data_bytes()];
    let budget = usize::try_from(constant(&mut vm, module, "ty_max_steps")?)
        .ok()
        .and_then(|v| v.checked_mul(2))
        .and_then(|v| v.checked_add(4))
        .ok_or_else(|| protocol("invalid stage step budget"))?;
    let pending = constant(&mut vm, module, "tyc_pending")?;
    let accept = constant(&mut vm, module, "ty_accept")?;
    let reject = constant(&mut vm, module, "ty_reject")?;
    if pending == accept || pending == reject || accept == reject {
        return Err(protocol("colliding verdict values"));
    }
    for (name, values) in [
        ("dn", vec![word(arities.len())?]),
        ("dparams", arities.to_vec()),
        ("csn", vec![word(batch.sites.len())?]),
        ("csite", batch.sites.iter().map(|r| r.0).collect()),
        ("cargs", batch.sites.iter().map(|r| r.1).collect()),
        ("en", vec![word(batch.nodes.len())?]),
        ("ekind", batch.nodes.iter().map(|r| r.0).collect()),
        ("ea", batch.nodes.iter().map(|r| r.1).collect()),
        ("eaf", batch.nodes.iter().map(|r| r.2).collect()),
        ("eb", batch.nodes.iter().map(|r| r.3).collect()),
        ("ebf", batch.nodes.iter().map(|r| r.4).collect()),
        ("bn", vec![word(batch.bindings.len())?]),
        ("bname", batch.bindings.iter().map(|r| r.0).collect()),
        ("btag", batch.bindings.iter().map(|r| r.1).collect()),
        ("bform", batch.bindings.iter().map(|r| r.2).collect()),
    ] {
        layout.write(&vm, &mut shared, name, &values)?;
    }
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .map_err(TypeCheckError::Execution)?;
    for _ in 0..budget {
        match state {
            VmState::Yielded(Value::Int(value)) if value == accept => return Ok(true),
            VmState::Yielded(Value::Int(value)) if value == reject => return Ok(false),
            VmState::Yielded(Value::Int(value)) if value == pending => {}
            VmState::Reset => {}
            _ => return Err(protocol("unexpected type stage state")),
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .map_err(TypeCheckError::Execution)?;
    }
    Err(protocol("type stage exhausted its declared step budget"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdict(module: &Module, inputs: &CallTypeInputs) -> bool {
        let layout = Layout::from_module(module).unwrap();
        let declarations = inputs
            .declarations
            .iter()
            .enumerate()
            .map(|(index, (name, _, _))| (name.as_str(), word(index).unwrap()))
            .collect();
        let arities: Vec<_> = inputs.declarations.iter().map(|(_, n, _)| *n).collect();
        inputs.batches.iter().all(|batch| {
            let encoded = encode(batch, &declarations).unwrap();
            run_batch(module, &layout, &arities, &encoded).unwrap()
        })
    }

    /// Identical source facts lose a derived rejection when only the stage's
    /// expression-type decision is disabled. Direct rejection still works.
    #[test]
    fn derived_binding_verdict_depends_on_the_keleusma_decision() {
        let source = include_str!("kel/verify_types.kel");
        let begin = source.find("fn tyb_node_tag(e: Word) -> Word {").unwrap();
        let end = source[begin..].find("\nfn ").unwrap() + begin;
        let mut disabled = source.to_string();
        disabled.replace_range(begin..end, "fn tyb_node_tag(e: Word) -> Word { 0 }\n");
        assert_ne!(disabled, source);
        let normal = super::super::compile_src(source);
        let without_derivation = super::super::compile_src(&disabled);
        for (program, expected, disabled_expected) in [
            (
                "fn take(x: bool) -> Word { 7 } fn main() -> Word { let a = 1 + 2; take(a) }",
                false,
                true,
            ),
            (
                "fn take(x: bool) -> Word { 7 } fn main() -> Word { let a = 1 == 2; take(a) }",
                true,
                true,
            ),
            (
                "fn take(x: bool) -> Word { 7 } fn main() -> Word { take(1) }",
                false,
                false,
            ),
        ] {
            let inputs = super::super::call_type_inputs_from_pipeline(program);
            assert_eq!(type_check_inputs_via_kel(&inputs).unwrap(), expected);
            assert_eq!(verdict(&normal, &inputs), expected, "{program}");
            assert_eq!(
                verdict(&without_derivation, &inputs),
                disabled_expected,
                "{program}"
            );
        }
    }
}
