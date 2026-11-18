//! Memory bound tightness over a fixed corpus.
//!
//! For each program in [`CORPUS`] this runner compares the memory bounds the
//! verifier proves with the memory a run actually touches, and writes the
//! figures to `target/metrics/tightness.json` for the metrics collector. A
//! ratio of measured to bound near one means the bound is tight. A measured
//! figure above its bound is a soundness defect and fails the run.
//!
//! Run: `cargo run --release --example tightness`, optionally with
//! `-- --out <path>`.
//!
//! # What is paired with what
//!
//! The arena has a bottom end and a top end, and the two bounds govern
//! different ends, so each measured peak is paired with the bound of its own
//! end.
//!
//! - **Bottom end, the stack.** `Vm::new` reserves, in this order, the operand
//!   stack, the call frames and the opaque registry, each sized from the
//!   verifier's footprint. The arena's own `bottom_peak` therefore equals the
//!   reservation from the start and measures nothing. This runner paints the
//!   ephemeral region with a byte pattern before construction and, after the
//!   run, finds the highest byte the run changed in each reserved buffer. That
//!   is the stack painting technique of embedded systems. The figure is
//!   rounded up to whole slots or whole frames, because a write of a value may
//!   leave padding bytes unchanged. Each program runs twice with two
//!   different patterns and the larger figure is kept, so a value that happens
//!   to equal the pattern is not mistaken for an untouched byte.
//! - **Top end, the heap.** Allocations on the top end are not reserved in
//!   advance, so the arena's `top_peak` is the measured figure. It is compared
//!   with the top-end terms of `vm_arena_terms_for`, the seeded private
//!   initializers plus the larger of the seeding transient and one run's heap
//!   and inputs.
//!
//! The bound fields are the verifier's figures before the virtual machine
//! applies its reservation minimums, so a small program is not reported as
//! tighter than its analysis is.
//!
//! # Output
//!
//! An object keyed by corpus path. Each value carries `bound_stack_bytes`,
//! `measured_stack_peak_bytes`, `bound_heap_bytes`,
//! `measured_heap_peak_bytes` and `iterations`, which are the fields the
//! collector reads. The stack figures are the operand stack and the call
//! frames together. Four further fields split them, `bound_operand_bytes`,
//! `measured_operand_bytes`, `bound_frame_bytes` and `measured_frame_bytes`.
//! `iterations` is the number of stream iterations completed, one for a
//! program whose entry is a `fn`.
//!
//! # Limits
//!
//! - The figures hold for the drive this runner applies, a scalar argument per
//!   entry parameter and a scalar resume value per yield, for at most
//!   [`DRIVE_STEPS`] steps. Another input can reach a different peak.
//! - A program whose module reserves an opaque registry is skipped with a
//!   reason, because the runner locates the frames by the end of the bottom
//!   reservation and the registry follows them.
//! - The runner depends on the reservation order inside `Vm::new`. It checks
//!   that the observed bottom reservation equals the operand stack and the
//!   frames, up to alignment at its start, and refuses to report when it does
//!   not, rather than guessing.
//! - Natives are not registered, so a program that calls one is skipped.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{
    Vm, VmState, required_persistent_capacity_for, shared_data_bytes_for, vm_arena_terms_for,
};

/// The corpus, relative to the repository root. `11_signed.kel` is left out
/// because it is loaded through the signed path, as in
/// `tests/native_region_supplement.rs`.
const CORPUS: [&str; 15] = [
    "examples/scripts/01_arithmetic.kel",
    "examples/scripts/02_struct_field.kel",
    "examples/scripts/03_enum_match.kel",
    "examples/scripts/04_for_in.kel",
    "examples/scripts/05_pipeline.kel",
    "examples/scripts/06_multiheaded.kel",
    "examples/scripts/07_refinement.kel",
    "examples/scripts/08_method_dispatch.kel",
    "examples/scripts/09_big_numbers.kel",
    "examples/scripts/10_multbyte.kel",
    "examples/scripts/12_sensor_window.kel",
    "examples/scripts/13_telemetry_stream.kel",
    "examples/scripts/14_frame_log.kel",
    "examples/scripts/15_pixel_blend.kel",
    "src/hosted/interp.kel",
];

/// The most `call` and `resume` steps one drive takes.
const DRIVE_STEPS: usize = 200;

/// The two paint patterns. Each run keeps the larger of the two figures.
const PAINTS: [u8; 2] = [0xA5, 0x5A];

/// One program's figures, in bytes except for `iterations`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Tightness {
    bound_operand_bytes: usize,
    measured_operand_bytes: usize,
    bound_frame_bytes: usize,
    measured_frame_bytes: usize,
    bound_heap_bytes: usize,
    measured_heap_peak_bytes: usize,
    iterations: usize,
}

impl Tightness {
    fn bound_stack_bytes(&self) -> usize {
        self.bound_operand_bytes + self.bound_frame_bytes
    }

    fn measured_stack_peak_bytes(&self) -> usize {
        self.measured_operand_bytes + self.measured_frame_bytes
    }

    /// The figures of two runs of the same program combined, the larger of
    /// each measured figure. The bounds and the iteration count must agree.
    fn merge(self, other: Tightness) -> Result<Tightness, String> {
        if self.bound_operand_bytes != other.bound_operand_bytes
            || self.bound_frame_bytes != other.bound_frame_bytes
            || self.bound_heap_bytes != other.bound_heap_bytes
            || self.iterations != other.iterations
        {
            return Err(format!(
                "two runs disagree outside the measured figures: {self:?} and {other:?}"
            ));
        }
        Ok(Tightness {
            measured_operand_bytes: self.measured_operand_bytes.max(other.measured_operand_bytes),
            measured_frame_bytes: self.measured_frame_bytes.max(other.measured_frame_bytes),
            measured_heap_peak_bytes: self
                .measured_heap_peak_bytes
                .max(other.measured_heap_peak_bytes),
            ..self
        })
    }

    /// The measured figures that exceed their bound, empty when the bounds
    /// held.
    fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (name, measured, bound) in [
            (
                "operand stack",
                self.measured_operand_bytes,
                self.bound_operand_bytes,
            ),
            (
                "call frames",
                self.measured_frame_bytes,
                self.bound_frame_bytes,
            ),
            ("heap", self.measured_heap_peak_bytes, self.bound_heap_bytes),
        ] {
            if measured > bound {
                out.push(format!("{name} measured {measured} above its bound {bound}"));
            }
        }
        out
    }
}

/// The highest offset in `bytes` that differs from `paint`, plus one, rounded
/// up to a multiple of `unit`. Zero when every byte still holds the paint.
fn touched_extent(bytes: &[u8], paint: u8, unit: usize) -> usize {
    debug_assert!(unit > 0, "the rounding unit is positive");
    match bytes.iter().rposition(|&b| b != paint) {
        Some(last) => (last + 1).div_ceil(unit) * unit,
        None => 0,
    }
}

fn compile(root: &Path, path: &str) -> Result<Module, String> {
    let source = std::fs::read_to_string(root.join(path)).map_err(|e| format!("read: {e}"))?;
    let tokens = keleusma::lexer::tokenize(&source).map_err(|e| format!("lex: {e:?}"))?;
    let program = keleusma::parser::parse(&tokens).map_err(|e| format!("parse: {e:?}"))?;
    keleusma::compiler::compile(&program).map_err(|e| format!("compile: {}", e.message))
}

/// Run `module` once in an arena whose ephemeral region holds `paint`, and
/// measure it.
fn measure(module: &Module, paint: u8) -> Result<Tightness, String> {
    if module.aux_arena_bytes > 0 {
        return Err(String::from(
            "the module reserves an opaque registry, which this runner cannot locate",
        ));
    }
    let terms = vm_arena_terms_for(module, &[], 0).map_err(|e| format!("terms: {e:?}"))?;
    let footprint = keleusma::verify::module_runtime_footprint(module, &[])
        .map_err(|e| format!("footprint: {}", e.message))?;
    let slot = core::mem::size_of::<Value>();
    let depth = footprint.max_frame_depth as usize;
    if depth == 0 || terms.call_frame_bytes % depth != 0 {
        return Err(format!(
            "the frame reservation {} is not a whole number of {depth} frames",
            terms.call_frame_bytes
        ));
    }
    let frame = terms.call_frame_bytes / depth;
    let bound_operand_bytes = footprint.max_operand_slots as usize * slot;
    let bound_frame_bytes = terms.call_frame_bytes;
    let bound_heap_bytes = terms.seeded_private_bytes
        + (terms.iteration_heap_bytes + terms.host_input_bytes).max(terms.seeding_transient_bytes);

    let persistent = required_persistent_capacity_for(module);
    let capacity = terms.total() + persistent;
    let mut arena = Arena::with_capacity(capacity);
    arena
        .resize_persistent(persistent)
        .map_err(|e| format!("resize: {e:?}"))?;
    let base = arena.persistent_ptr().as_ptr();
    // SAFETY: the arena owns `capacity` bytes from `base`, nothing has been
    // allocated in its ephemeral region `[persistent, capacity)`, and no
    // reference into it exists.
    unsafe { core::ptr::write_bytes(base.add(persistent), paint, capacity - persistent) };

    let entry = module.entry_point.ok_or("the module has no entry")?;
    let params = module.signatures[entry].params.len();
    let mut shared = vec![0u8; shared_data_bytes_for(module)];
    let mut iterations = 0usize;
    let (stack_start, frames_start, reserved) = {
        let mut vm = Vm::new(module.clone(), &arena).map_err(|e| format!("new: {e:?}"))?;
        let reserved = arena.bottom_used();
        let frames_start = persistent + reserved - terms.call_frame_bytes;
        let stack_start = frames_start
            .checked_sub(terms.operand_stack_bytes)
            .filter(|&s| s >= persistent && s - persistent < slot)
            .ok_or_else(|| {
                format!(
                    "the bottom reservation of {reserved} bytes is not the operand stack of {} \
                     and the frames of {} up to alignment",
                    terms.operand_stack_bytes, terms.call_frame_bytes
                )
            })?;
        let args: Vec<Value> = (0..params).map(|_| Value::Int(1)).collect();
        let mut state = vm
            .call_with_shared(&mut shared, &args)
            .map_err(|e| format!("call: {e:?}"))?;
        for step in 1..DRIVE_STEPS {
            match state {
                VmState::Finished(_) => {
                    iterations += 1;
                    break;
                }
                VmState::Yielded(_) | VmState::Reset => {
                    if matches!(state, VmState::Reset) {
                        iterations += 1;
                    }
                    let input = if step % 7 == 0 { 3 } else { 2 };
                    state = vm
                        .resume_with_shared(&mut shared, Value::Int(input))
                        .map_err(|e| format!("resume at step {step}: {e:?}"))?;
                }
                VmState::BreakpointHit { .. } => return Err(String::from("a breakpoint hit")),
            }
        }
        (stack_start, frames_start, reserved)
    };
    // SAFETY: the virtual machine is dropped, the bytes lie within the arena's
    // buffer, and they are only read.
    let ephemeral = unsafe { core::slice::from_raw_parts(base, capacity) };
    // A push past the reservation would reallocate the vector beyond the
    // frames, where this scan does not look, so growth is refused outright.
    if arena.bottom_peak() > reserved {
        return Err(format!(
            "the bottom end grew to {} bytes past its reservation of {reserved}",
            arena.bottom_peak()
        ));
    }
    let operand = &ephemeral[stack_start..stack_start + terms.operand_stack_bytes];
    let frames = &ephemeral[frames_start..frames_start + terms.call_frame_bytes];
    Ok(Tightness {
        bound_operand_bytes,
        measured_operand_bytes: touched_extent(operand, paint, slot),
        bound_frame_bytes,
        measured_frame_bytes: touched_extent(frames, paint, frame),
        bound_heap_bytes,
        measured_heap_peak_bytes: arena.top_peak(),
        iterations,
    })
}

fn json(results: &[(&str, Tightness)]) -> String {
    let mut out = String::from("{\n");
    for (i, (path, t)) in results.iter().enumerate() {
        let sep = if i + 1 == results.len() { "" } else { "," };
        // The corpus paths are fixed ASCII paths with no character that
        // needs escaping in a JSON string.
        let _ = writeln!(
            out,
            "  \"{path}\": {{\"bound_stack_bytes\": {}, \"measured_stack_peak_bytes\": {}, \
             \"bound_heap_bytes\": {}, \"measured_heap_peak_bytes\": {}, \"iterations\": {}, \
             \"bound_operand_bytes\": {}, \"measured_operand_bytes\": {}, \
             \"bound_frame_bytes\": {}, \"measured_frame_bytes\": {}}}{sep}",
            t.bound_stack_bytes(),
            t.measured_stack_peak_bytes(),
            t.bound_heap_bytes,
            t.measured_heap_peak_bytes,
            t.iterations,
            t.bound_operand_bytes,
            t.measured_operand_bytes,
            t.bound_frame_bytes,
            t.measured_frame_bytes,
        );
    }
    out.push_str("}\n");
    out
}

fn out_path(root: &Path) -> Result<PathBuf, String> {
    let mut args = std::env::args().skip(1);
    match (args.next().as_deref(), args.next()) {
        (None, _) => {
            let target = std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| root.join("target"));
            Ok(target.join("metrics").join("tightness.json"))
        }
        (Some("--out"), Some(p)) if args.next().is_none() => Ok(PathBuf::from(p)),
        _ => Err(String::from("usage: tightness [--out <path>]")),
    }
}

fn main() -> std::process::ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = match out_path(root) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            return std::process::ExitCode::from(2);
        }
    };
    let mut results = Vec::new();
    let mut violations = Vec::new();
    for path in CORPUS {
        let figures = compile(root, path).and_then(|m| {
            PAINTS
                .iter()
                .map(|&p| measure(&m, p))
                .reduce(|a, b| a?.merge(b?))
                .unwrap_or_else(|| Err(String::from("no paint")))
        });
        match figures {
            Ok(t) => {
                violations.extend(t.violations().into_iter().map(|v| format!("{path}: {v}")));
                results.push((path, t));
            }
            Err(e) => eprintln!("skipped {path}: {e}"),
        }
    }
    eprintln!("measured {} of {} programs", results.len(), CORPUS.len());
    if let Some(dir) = out.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        eprintln!("{}: {e}", dir.display());
        return std::process::ExitCode::from(2);
    }
    if let Err(e) = std::fs::write(&out, json(&results)) {
        eprintln!("{}: {e}", out.display());
        return std::process::ExitCode::from(2);
    }
    eprintln!("wrote {}", out.display());
    if violations.is_empty() {
        std::process::ExitCode::SUCCESS
    } else {
        for v in &violations {
            eprintln!("BOUND EXCEEDED {v}");
        }
        std::process::ExitCode::FAILURE
    }
}

#[cfg(test)]
mod tests {
    use super::touched_extent;

    #[test]
    fn an_untouched_buffer_has_no_extent() {
        assert_eq!(touched_extent(&[0xA5; 64], 0xA5, 32), 0);
    }

    #[test]
    fn a_partial_slot_rounds_up_to_the_whole_slot() {
        let mut b = [0xA5u8; 96];
        b[33] = 0;
        assert_eq!(touched_extent(&b, 0xA5, 32), 64);
    }

    #[test]
    fn the_last_byte_counts() {
        let mut b = [0xA5u8; 64];
        b[63] = 1;
        assert_eq!(touched_extent(&b, 0xA5, 32), 64);
    }
}
