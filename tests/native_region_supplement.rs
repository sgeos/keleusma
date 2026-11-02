//! The arena sizing functions of each route, and the native region supplement.
//!
//! The operator ruled on question 22, confirmed first-hand on 2026-10-31,
//! that the runtime's sizing figure include the arena bytes a native
//! compilation reserves. `src/native_region.rs` reimplements the formula the
//! V0.3.X line sent as data, and the native backend stays the authority. On
//! the operator's later ruling that `auto_arena_capacity_for` right-size an
//! arena, recorded in `docs/decisions/ARENA_SIZING_ROUTES.md`, that function
//! became the virtual machine's figure alone, the combined figure moved to
//! `auto_arena_capacity_for_vm_and_native`, and a native-only host sizes its
//! buffers from `native_route_buffers_for`.
//!
//! The pinned native figures are the ones V0.3.X measured with
//! `keleusma_native::region::host_arena_supplement_bytes` at its tree
//! `630e68f0`, and its region, private and shared figures of 2026-10-31 on
//! v0.3.0 plus absorption 129. A disagreement here means this copy has drifted
//! from the backend, or that the two lines' compilers emit different chunks
//! for the same source, and either is a finding to report to V0.3.X rather
//! than a pin to move.
//!
//! # What these tests do not establish
//!
//! The corpus drive feeds each entry fixed scalar inputs for a bounded number
//! of steps, so it measures the peak of the paths those inputs reach, not of
//! every path. The verifier's bound is the worst case over paths, so a measured
//! peak below the figure shows the figure is not short on the driven paths and
//! does not show which path attains it.
#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::Arena;
use keleusma::bytecode::{Module, Value};
use keleusma::native_region::{
    NativeRouteBuffers, native_persistent_supplement_bytes, native_region_supplement_bytes,
    native_route_buffers_for,
};
use keleusma::vm::{
    Vm, VmError, VmState, auto_arena_capacity_for, auto_arena_capacity_for_vm_and_native,
    host_input_arena_bytes_for, required_persistent_capacity_for, shared_data_bytes_for,
    vm_arena_terms_for,
};

fn compile_source(source: &str) -> Module {
    let tokens = keleusma::lexer::tokenize(source).expect("lex");
    keleusma::compiler::compile(&keleusma::parser::parse(&tokens).expect("parse"))
        .unwrap_or_else(|e| panic!("{}", e.message))
}

fn module(path: &str) -> Module {
    let full = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
    let source = std::fs::read_to_string(&full).unwrap_or_else(|e| panic!("{path}: {e}"));
    compile_source(&source)
}

/// The five modules V0.3.X measured, with its region, its virtual machine
/// figure without the region, its private buffer and its shared buffer.
const V03X_FIGURES: [(&str, usize, usize, usize, usize); 5] = [
    ("examples/scripts/02_struct_field.kel", 16, 224, 0, 0),
    ("examples/scripts/10_multbyte.kel", 192, 1616, 0, 0),
    ("examples/scripts/12_sensor_window.kel", 80, 712, 0, 0),
    ("examples/scripts/13_telemetry_stream.kel", 720, 664, 8, 0),
    ("src/hosted/interp.kel", 4032, 15800, 80072, 162832),
];

/// The corpus the virtual machine figure is driven against. `11_signed.kel` is
/// left out because it must be loaded through the signed path.
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

/// Steps of `call` and `resume` each corpus drive takes.
const DRIVE_STEPS: usize = 200;

/// The measured arena peak of a run, bottom and top, after the persistent
/// region.
struct Drive {
    steps: usize,
    peak: usize,
}

/// Drive `module` in an arena of `ephemeral` bytes after its persistent
/// region, with a scalar argument for each entry parameter and a scalar resume
/// value for each yield, for at most [`DRIVE_STEPS`] steps.
fn drive(module: &Module, ephemeral: usize) -> Result<Drive, VmError> {
    let persistent = required_persistent_capacity_for(module);
    let mut arena = Arena::with_capacity(ephemeral + persistent);
    arena.resize_persistent(persistent).expect("resize");
    let entry = module.entry_point.expect("an entry");
    let params = module.signatures[entry].params.len();
    let mut shared = vec![0u8; shared_data_bytes_for(module)];
    let mut steps = 1usize;
    {
        let mut vm = Vm::new(module.clone(), &arena)?;
        let args: Vec<Value> = (0..params).map(|_| Value::Int(1)).collect();
        let mut state = vm.call_with_shared(&mut shared, &args)?;
        while steps < DRIVE_STEPS {
            match state {
                VmState::Yielded(_) | VmState::Reset => {
                    let input = if steps.is_multiple_of(7) { 3 } else { 2 };
                    state = vm.resume_with_shared(&mut shared, Value::Int(input))?;
                    steps += 1;
                }
                _ => break,
            }
        }
    }
    Ok(Drive {
        steps,
        peak: arena.bottom_peak() + arena.top_peak(),
    })
}

#[test]
fn the_supplement_matches_the_backend_figures() {
    for (path, region, _, _, _) in V03X_FIGURES {
        assert_eq!(
            native_region_supplement_bytes(&module(path)) as usize,
            region,
            "{path}"
        );
    }
}

#[test]
fn the_copy_at_the_yield_follows_the_entry_total() {
    // 13_telemetry_stream.kel's entry total is 648 and its copy is four slots
    // of 16 bytes and a cursor word, 72 bytes, so the figure is 720.
    let m = module("examples/scripts/13_telemetry_stream.kel");
    assert_eq!(native_region_supplement_bytes(&m), 648 + 72);
}

#[test]
fn a_module_without_an_entry_needs_no_supplement() {
    let mut m = module("examples/scripts/02_struct_field.kel");
    m.entry_point = None;
    assert_eq!(native_region_supplement_bytes(&m), 0);
    assert_eq!(native_route_buffers_for(&m).region_bytes, 0);
    assert_eq!(
        auto_arena_capacity_for_vm_and_native(&m, &[]).expect("bounded"),
        auto_arena_capacity_for(&m, &[]).expect("bounded"),
        "with no region the combined figure is the virtual machine's"
    );
}

#[test]
fn the_vm_figure_excludes_the_region_and_matches_the_v03x_vm_only_column() {
    // V0.3.X computed its virtual-machine-only column as the combined figure
    // minus the region, so the right-sized function must reproduce it.
    for (path, region, vm_only, _, _) in V03X_FIGURES {
        let m = module(path);
        assert_eq!(
            auto_arena_capacity_for(&m, &[]).expect("bounded"),
            vm_only,
            "{path}"
        );
        assert_eq!(
            auto_arena_capacity_for_vm_and_native(&m, &[]).expect("bounded"),
            vm_only + region,
            "{path}: the combined figure is the former one"
        );
    }
}

#[test]
fn the_combined_figure_adds_the_region_exactly_once() {
    for path in CORPUS {
        let m = module(path);
        let vm = auto_arena_capacity_for(&m, &[]).expect("bounded");
        let both = auto_arena_capacity_for_vm_and_native(&m, &[]).expect("bounded");
        assert_eq!(
            both - vm,
            native_region_supplement_bytes(&m) as usize,
            "{path}"
        );
    }
}

#[test]
fn the_vm_figure_is_the_sum_of_the_verifier_terms() {
    for path in CORPUS {
        let m = module(path);
        let fp = keleusma::verify::module_runtime_footprint(&m, &[]).expect("bounded");
        let terms = vm_arena_terms_for(&m, &[], 0).expect("bounded");
        assert_eq!(
            terms.operand_stack_bytes,
            // Floored at the constructor's minimum of four slots.
            (fp.max_operand_slots as usize).max(4) * std::mem::size_of::<Value>(),
            "{path}"
        );
        assert_eq!(
            terms.iteration_heap_bytes, fp.max_heap_bytes as usize,
            "{path}"
        );
        assert_eq!(
            terms.opaque_registry_bytes, m.aux_arena_bytes as usize,
            "{path}"
        );
        assert_eq!(
            terms.host_input_bytes,
            host_input_arena_bytes_for(&m, 0).expect("bounded"),
            "{path}"
        );
        assert_eq!(
            auto_arena_capacity_for(&m, &[]).expect("bounded"),
            terms.total(),
            "{path}"
        );
    }
}

#[test]
fn the_vm_figure_is_never_below_the_measured_peak() {
    let mut driven = 0usize;
    for path in CORPUS {
        let m = module(path);
        let figure = auto_arena_capacity_for(&m, &[]).expect("bounded");
        let run = drive(&m, figure)
            .unwrap_or_else(|e| panic!("{path}: the figure of {figure} bytes is short, {e:?}"));
        assert!(
            run.peak <= figure,
            "{path}: the measured peak {} exceeds the figure {figure}",
            run.peak
        );
        assert!(run.steps >= 1, "{path}");
        driven += 1;
    }
    assert_eq!(driven, CORPUS.len(), "every corpus module was driven");
}

#[test]
fn a_seeded_private_composite_is_counted_and_the_figure_is_tight() {
    // Construction copies the `Frame` initializer of `log.latest` through the
    // top region, and the copy stays until the first `Reset`. Measured on
    // 2026-11-01, a figure without those 24 bytes failed with `OutOfArena` on
    // the first call, and the smallest arena that ran was 680 bytes.
    let m = module("examples/scripts/14_frame_log.kel");
    let terms = vm_arena_terms_for(&m, &[], 0).expect("bounded");
    assert_eq!(terms.seeded_private_bytes, 24);
    assert_eq!(
        terms.seeding_transient_bytes, 0,
        "a flat Frame nests nothing"
    );
    let figure = auto_arena_capacity_for(&m, &[]).expect("bounded");
    assert_eq!(figure, 680);
    drive(&m, figure).expect("the figure runs");
    match drive(&m, figure - 1) {
        Err(VmError::OutOfArena(_)) => {}
        other => panic!(
            "one byte below the figure must run out, got {:?}",
            other.map(|d| d.steps)
        ),
    }
}

#[test]
fn a_nested_private_initializer_is_charged_its_transient() {
    let m = compile_source(
        "struct Inner { a: Word, b: Word }\n\
         struct Outer { inner: Inner, c: Word }\n\
         private data st { o: Outer, n: Word }\n\
         loop main(x: Word) -> Word { st.n = st.n + x; let _r = yield st.o.c + st.n; 0 }",
    );
    let terms = vm_arena_terms_for(&m, &[], 0).expect("bounded");
    assert_eq!(
        terms.seeded_private_bytes,
        m.persistent_composite_bytes as usize
    );
    assert!(terms.seeded_private_bytes > 0);
    assert_eq!(
        terms.seeding_transient_bytes, terms.seeded_private_bytes,
        "the one nested initializer is charged its extent"
    );
    // Measured on 2026-11-01, packing `Outer` holds its 16-byte `Inner` body
    // beside its own 24-byte body, so the top region peaks at 40 bytes and the
    // smallest arena that runs is 224 bytes. The figure is 232, because the
    // transient is charged the whole 24-byte extent rather than the 16 bytes of
    // the nested child, a looseness of the parent's own scalar field.
    let figure = auto_arena_capacity_for(&m, &[]).expect("bounded");
    assert_eq!(figure, 232);
    drive(&m, figure).expect("the figure constructs and runs");
    match drive(&m, figure - terms.seeding_transient_bytes) {
        Err(VmError::OutOfArena(_)) => {}
        other => panic!(
            "without the transient the figure must run out, got {:?}",
            other.map(|d| d.steps)
        ),
    }
}

#[test]
fn a_module_without_composites_or_streams_needs_no_seed_and_no_native_buffers() {
    let m = compile_source("fn main() -> Word { 7 }");
    let terms = vm_arena_terms_for(&m, &[], 0).expect("bounded");
    assert_eq!(
        (
            terms.iteration_heap_bytes,
            terms.host_input_bytes,
            terms.seeded_private_bytes,
            terms.seeding_transient_bytes
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(native_persistent_supplement_bytes(&m), 0);
    assert_eq!(native_route_buffers_for(&m), NativeRouteBuffers::default());
    let figure = auto_arena_capacity_for(&m, &[]).expect("bounded");
    assert_eq!(figure, terms.operand_stack_bytes + terms.call_frame_bytes);
    drive(&m, figure).expect("the figure runs");
}

#[test]
fn the_native_route_reproduces_the_v03x_buffers() {
    for (path, region, _, private, shared) in V03X_FIGURES {
        let m = module(path);
        assert_eq!(
            native_route_buffers_for(&m),
            NativeRouteBuffers {
                region_bytes: region,
                private_bytes: private,
                shared_bytes: shared,
            },
            "{path}"
        );
    }
}

#[test]
fn the_native_persistent_supplement_counts_streams_and_composite_slots() {
    // Derived from the backend's formula, not measured by V0.3.X. The frame
    // log has one stream chunk and one private composite pool slot, each one
    // eight-byte word.
    let m = module("examples/scripts/14_frame_log.kel");
    assert_eq!(native_persistent_supplement_bytes(&m), 16);
    assert_eq!(
        native_route_buffers_for(&m).private_bytes,
        required_persistent_capacity_for(&m) + 16
    );
    // A non-stream module with no private data has no supplement, as V0.3.X
    // measured for the first three of its five modules.
    let m = module("examples/scripts/02_struct_field.kel");
    assert_eq!(native_persistent_supplement_bytes(&m), 0);
}
