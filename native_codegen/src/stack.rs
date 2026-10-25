//! A machine stack bound for emitted code (V0.3.x Workstream E).
//!
//! The verifier bounds the virtual machine's operand stack, which native code
//! does not use. The machine stack native code uses is chosen by LLVM's frame
//! lowering and register allocator. This module reads that choice back and
//! bounds the stack a call into the code can reach.
//!
//! # How the frame sizes are read
//!
//! LLVM's prologue and epilogue insertion reports a function's final stack
//! frame size as a diagnostic when the function's `warn-stack-size` attribute
//! is below it. [`with_frame_sizes`] sets that threshold to zero on every
//! defined function, installs a diagnostic handler on the context, runs one
//! code generation, and collects each report. It then removes the attribute
//! and the handler. The figure is the frame size the object file contains.
//! `native_stack_bound.rs` read the same figure from an ELF `.stack_sizes`
//! section through `llc`, which no Mach-O object carries.
//!
//! # How the bound is formed
//!
//! The verifier rejects recursion and the lowering emits none, so the static
//! call graph of the emitted code is acyclic. [`longest_path`] adds, along the
//! deepest call path from each root, every frame plus [`FRAME_ALLOWANCE`]. An
//! indirect call may reach any function whose address the module takes, which
//! covers the stable handles calling a continuation. A cycle is refused rather
//! than cut.
//!
//! # What the bound does not cover
//!
//! Functions this module does not define are outside it. Those are the host's
//! natives and any compiler runtime or C library routine the code generator
//! calls, such as `memcpy` for a large copy. A function with an allocation of
//! runtime size has no static frame and is refused.

use inkwell::attributes::AttributeLoc;
use inkwell::module::Module as LlvmModule;
use inkwell::values::{AnyValue, BasicValueEnum, FunctionValue, InstructionOpcode};
use llvm_sys::prelude::LLVMContextRef;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, c_void};

/// Bytes added to every frame on a path.
///
/// A reported frame size may leave out what the call instruction itself
/// stores. On x86-64 that is the eight-byte return address, and stack
/// realignment at a call can add eight more. AAPCS64 keeps the return address
/// in a register and includes the frame record in the frame. Sixteen bytes
/// per level covers both conventions, which is an assumption about them and
/// not a measurement. `static_library_linkage.rs` measures the stack a host
/// actually uses on the executed target.
pub const FRAME_ALLOWANCE: u64 = 16;

const ATTRIBUTE: &str = "warn-stack-size";
const REPORT: &str = "stack frame size (";
const FUNCTION: &str = " in function '";

/// Clears the handler however the guarded code generation ends, before the
/// buffer the handler writes into is dropped.
struct HandlerGuard {
    ctx: LLVMContextRef,
}

impl Drop for HandlerGuard {
    fn drop(&mut self) {
        // SAFETY: clearing the handler takes no user pointer.
        unsafe {
            llvm_sys::core::LLVMContextSetDiagnosticHandler(self.ctx, None, std::ptr::null_mut())
        };
    }
}

extern "C" fn collect(info: llvm_sys::prelude::LLVMDiagnosticInfoRef, out: *mut c_void) {
    // SAFETY: LLVM passes a valid diagnostic for the duration of this call and
    // the user pointer is the `Vec` that `with_frame_sizes` keeps alive while
    // the handler is installed.
    unsafe {
        let text = llvm_sys::core::LLVMGetDiagInfoDescription(info);
        if text.is_null() {
            return;
        }
        let line = CStr::from_ptr(text).to_string_lossy().into_owned();
        llvm_sys::core::LLVMDisposeMessage(text);
        (*out.cast::<Vec<String>>()).push(line);
    }
}

/// Parse one frame-size report into its function name and byte count.
fn parse_report(line: &str) -> Option<(String, u64)> {
    let start = line.find(REPORT)? + REPORT.len();
    let size = line[start..].split(')').next()?.trim().parse().ok()?;
    let name_start = line.find(FUNCTION)? + FUNCTION.len();
    let name = line[name_start..].strip_suffix('\'')?;
    Some((name.to_string(), size))
}

/// Run `generate`, a code generation of `module`, and return its result with
/// the frame size of every defined function that has a frame.
///
/// A function absent from the map was reported with no frame, which is a zero
/// frame. Two reports for one function, or an unparsable report, are errors,
/// since either means the figures cannot be trusted.
pub fn with_frame_sizes<R>(
    module: &LlvmModule<'_>,
    generate: impl FnOnce() -> R,
) -> Result<(R, BTreeMap<String, u64>), String> {
    let ctx = module.get_context();
    let defined: Vec<FunctionValue<'_>> = module
        .get_functions()
        .filter(|f| !f.as_global_value().is_declaration())
        .collect();
    for f in &defined {
        f.add_attribute(
            AttributeLoc::Function,
            ctx.create_string_attribute(ATTRIBUTE, "0"),
        );
    }
    let mut reports: Vec<String> = Vec::new();
    let result = {
        let _guard = HandlerGuard { ctx: ctx.raw() };
        // SAFETY: `reports` outlives the guard, which clears the handler.
        unsafe {
            llvm_sys::core::LLVMContextSetDiagnosticHandler(
                ctx.raw(),
                Some(collect),
                (&mut reports as *mut Vec<String>).cast(),
            )
        };
        generate()
    };
    for f in &defined {
        f.remove_string_attribute(AttributeLoc::Function, ATTRIBUTE);
    }
    let mut frames = BTreeMap::new();
    for line in reports.iter().filter(|l| l.contains(REPORT)) {
        let (name, size) =
            parse_report(line).ok_or_else(|| format!("unreadable frame report `{line}`"))?;
        if frames.insert(name.clone(), size).is_some() {
            return Err(format!("two frame reports for `{name}`"));
        }
    }
    Ok((result, frames))
}

/// Refuse a function whose frame is not static.
///
/// An `alloca` whose count is not a constant sizes its frame at run time, and
/// the reported size then excludes it.
pub fn require_static_frames(module: &LlvmModule<'_>) -> Result<(), String> {
    for f in module.get_functions() {
        for block in f.get_basic_blocks() {
            for i in block.get_instructions() {
                if i.get_opcode() != InstructionOpcode::Alloca {
                    continue;
                }
                let constant = i
                    .get_operand(0)
                    .and_then(|o| o.value())
                    .is_some_and(|v| v.is_int_value() && v.into_int_value().is_const());
                if !constant {
                    return Err(format!(
                        "`{}` allocates a runtime-sized stack object",
                        f.get_name().to_string_lossy()
                    ));
                }
            }
        }
    }
    Ok(())
}

/// The call graph among `module`'s defined functions.
///
/// Each function maps to the defined functions it calls directly, plus every
/// address-taken defined function when it makes an indirect call.
fn call_graph<'ctx>(module: &LlvmModule<'ctx>) -> BTreeMap<String, BTreeSet<String>> {
    let defined: BTreeSet<String> = module
        .get_functions()
        .filter(|f| !f.as_global_value().is_declaration())
        .map(|f| f.get_name().to_string_lossy().into_owned())
        .collect();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let mut direct: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut indirect: BTreeSet<String> = BTreeSet::new();
    for f in module
        .get_functions()
        .filter(|f| !f.as_global_value().is_declaration())
    {
        let name = f.get_name().to_string_lossy().into_owned();
        let callees = direct.entry(name.clone()).or_default();
        for block in f.get_basic_blocks() {
            for i in block.get_instructions() {
                let is_call = i.get_opcode() == InstructionOpcode::Call;
                let operands = i.get_num_operands();
                for k in 0..operands {
                    // Matched as a basic value. Converting to `AnyValueEnum`
                    // reclassifies a function constant as a function, which no
                    // pointer pattern then matches, and the graph came out empty.
                    let Some(BasicValueEnum::PointerValue(p)) =
                        i.get_operand(k).and_then(|o| o.value())
                    else {
                        continue;
                    };
                    let target = p.get_name().to_string_lossy().into_owned();
                    if !defined.contains(&target) {
                        continue;
                    }
                    // The callee is a call's last operand. Any other use of
                    // a function's address may reach an indirect call.
                    if is_call && k + 1 == operands {
                        callees.insert(target);
                    } else {
                        taken.insert(target);
                    }
                }
                if is_call {
                    let callee = i
                        .get_operand(operands.saturating_sub(1))
                        .and_then(|o| o.value());
                    let named = callee.is_some_and(|v| {
                        v.is_pointer_value()
                            && module
                                .get_function(&v.into_pointer_value().get_name().to_string_lossy())
                                .is_some()
                    });
                    if !named {
                        indirect.insert(name.clone());
                    }
                }
            }
        }
    }
    for g in module.get_globals() {
        if let Some(init) = g.get_initializer() {
            let text = init.print_to_string().to_string();
            for d in &defined {
                if text.contains(&format!("@{d}")) || text.contains(&format!("@\"{d}\"")) {
                    taken.insert(d.clone());
                }
            }
        }
    }
    for f in indirect {
        direct.get_mut(&f).unwrap().extend(taken.iter().cloned());
    }
    direct
}

/// The deepest stack a call to any of `roots` can reach, in bytes.
///
/// Each frame on the path counts its reported size plus [`FRAME_ALLOWANCE`].
/// The traversal is an explicit post-order over an acyclic graph, and a cycle
/// is an error.
pub fn longest_path(
    module: &LlvmModule<'_>,
    frames: &BTreeMap<String, u64>,
    roots: &[String],
) -> Result<u64, String> {
    let graph = call_graph(module);
    let mut depth: BTreeMap<String, u64> = BTreeMap::new();
    let mut on_path: BTreeSet<String> = BTreeSet::new();
    for root in roots {
        if !graph.contains_key(root) {
            return Err(format!("`{root}` is not defined in the module"));
        }
        // (function, whether its callees have been pushed)
        let mut work: Vec<(String, bool)> = vec![(root.clone(), false)];
        while let Some((f, expanded)) = work.pop() {
            if depth.contains_key(&f) {
                continue;
            }
            if expanded {
                on_path.remove(&f);
                let deepest = graph[&f]
                    .iter()
                    .map(|c| depth.get(c).copied().unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                let own = frames.get(&f).copied().unwrap_or(0) + FRAME_ALLOWANCE;
                depth.insert(f, own + deepest);
                continue;
            }
            if !on_path.insert(f.clone()) {
                return Err(format!("the call graph has a cycle through `{f}`"));
            }
            work.push((f.clone(), true));
            for c in &graph[&f] {
                if on_path.contains(c) {
                    return Err(format!("the call graph has a cycle through `{c}`"));
                }
                if !depth.contains_key(c) {
                    work.push((c.clone(), false));
                }
            }
        }
    }
    Ok(roots.iter().map(|r| depth[r]).max().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    use super::parse_report;

    #[test]
    fn a_frame_report_parses_its_name_and_size() {
        assert_eq!(
            parse_report("<unknown>:0:0: stack frame size (80) exceeds limit (0) in function 'f'"),
            Some((String::from("f"), 80))
        );
        assert_eq!(
            parse_report(
                "<unknown>:0:0: stack frame size (16) exceeds limit (0) in function 'kel_chunk_3.resume.0'"
            ),
            Some((String::from("kel_chunk_3.resume.0"), 16))
        );
        assert_eq!(
            parse_report("<unknown>:0:0: 5 instructions in function"),
            None
        );
    }
}
