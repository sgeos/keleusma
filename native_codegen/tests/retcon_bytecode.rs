//! Bytecode, rather than handwritten LLVM fragments, drives these coroutines.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma_native::{coroutine, region};

fn machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).unwrap();
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .unwrap()
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Step {
    next: *const (),
    value: i64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    live: u64,
    value: i64,
}
impl Outcome {
    fn step(self, slot: *mut u8) -> Step {
        assert!(self.live <= 1);
        Step {
            next: if self.live == 0 {
                std::ptr::null()
            } else {
                slot.cast()
            },
            value: self.value,
        }
    }
}
type HandleStart = unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
type HandleFloatStart =
    unsafe extern "C" fn(NativeFloat, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
type HandleResume = unsafe extern "C" fn(*mut u8, i64) -> Outcome;
type HandleRelease = unsafe extern "C" fn(*mut u8);

type Resume = unsafe extern "C" fn(*mut u8, bool) -> Step;
type Start = unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) -> Step;

#[cfg(feature = "narrow-float-32")]
type NativeFloat = f32;
#[cfg(not(feature = "narrow-float-32"))]
type NativeFloat = f64;
type FloatStart =
    unsafe extern "C" fn(NativeFloat, *mut u8, *mut u8, *mut u8, *mut u8, *mut u8) -> Step;

#[cfg(feature = "narrow-float-32")]
fn wide(value: NativeFloat) -> f64 {
    f64::from(value)
}
#[cfg(not(feature = "narrow-float-32"))]
fn wide(value: NativeFloat) -> f64 {
    value
}

/// Exact boundaries, including zero-length regions, have adjacent sentinels.
/// Each reservation comes from the arena and is stable until release.
struct Guarded {
    arena: keleusma_arena::Arena,
    allocation: std::ptr::NonNull<[u8]>,
    len: usize,
}
impl Guarded {
    fn new(len: usize) -> Self {
        let arena = keleusma_arena::Arena::with_capacity(len + 128);
        let mut allocation = arena.alloc_bottom_bytes(len + 32).unwrap();
        unsafe {
            allocation.as_mut().fill(0xa5);
            allocation.as_mut()[16..16 + len].fill(0);
        }
        let this = Self {
            arena,
            allocation,
            len,
        };
        assert_eq!(this.ptr() as usize % 8, 0);
        this
    }
    fn ptr(&self) -> *mut u8 {
        unsafe { (self.allocation.as_ptr() as *mut u8).add(16) }
    }
    fn check(&self) {
        let bytes = unsafe { self.allocation.as_ref() };
        assert!(bytes[..16].iter().all(|&b| b == 0xa5), "prefix overwritten");
        assert!(
            bytes[16 + self.len..].iter().all(|&b| b == 0xa5),
            "suffix overwritten"
        );
    }
}

fn native(src: &str, first: i64, replies: &[i64], optimize: bool) -> Vec<i64> {
    let program = common::build(src);
    let seed = vec![0; program.shared_data_bytes as usize];
    native_module(&program, &seed, first, replies, optimize).0
}

fn native_module(
    program: &keleusma::bytecode::Module,
    seed: &[u8],
    first: i64,
    replies: &[i64],
    optimize: bool,
) -> (Vec<i64>, Vec<u8>) {
    let raw = native_module_read(
        program,
        seed,
        first,
        replies,
        optimize,
        false,
        |bits, _, _, _| bits,
    );
    let stable = native_module_read(
        program,
        seed,
        first,
        replies,
        optimize,
        true,
        |bits, _, _, _| bits,
    );
    assert_eq!(
        raw, stable,
        "stable handle must preserve the raw retcon sequence"
    );
    raw
}

fn native_module_read<T>(
    program: &keleusma::bytecode::Module,
    seed: &[u8],
    first: i64,
    replies: &[i64],
    optimize: bool,
    stable: bool,
    mut read: impl FnMut(i64, &Guarded, &Guarded, &Guarded) -> T,
) -> (Vec<T>, Vec<u8>) {
    assert!(!replies.is_empty());
    let ctx = Context::create();
    let tm = machine();
    let module = coroutine::lower(&ctx, program, &tm, 4096).expect("retcon lower");
    if optimize {
        module
            .run_passes(
                "default<O2>",
                &tm,
                inkwell::passes::PassBuilderOptions::create(),
            )
            .unwrap();
        module.verify().unwrap();
    }
    let ir = module.print_to_string().to_string();
    assert!(ir.contains(".resume"), "LLVM must produce continuations");
    assert!(!ir.contains("call i1 (...) @llvm.coro.suspend"));
    let entry = format!("kel_chunk_{}", program.entry_point.unwrap());
    let f = module.get_function(&entry).unwrap();
    assert_eq!(f.count_params(), 6);
    assert_eq!(
        f.get_type()
            .get_return_type()
            .unwrap()
            .into_struct_type()
            .count_fields(),
        2
    );
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    let float_input = f.get_nth_param(0).unwrap().is_float_value();
    if float_input {
        assert_eq!(
            f.get_nth_param(0)
                .unwrap()
                .into_float_value()
                .get_type()
                .get_bit_width(),
            (std::mem::size_of::<NativeFloat>() * 8) as u32
        );
    } else {
        assert_eq!(
            f.get_nth_param(0)
                .unwrap()
                .into_int_value()
                .get_type()
                .get_bit_width(),
            64
        );
    }
    let frame = Guarded::new(if stable {
        coroutine::slot_bytes(4096).unwrap() as usize
    } else {
        4096
    });
    let reply = Guarded::new(if stable { 0 } else { 8 });
    let handle = format!("kel_coroutine_{}", program.entry_point.unwrap());
    if stable {
        assert_eq!(
            module
                .get_function(&format!("{handle}_start"))
                .unwrap()
                .count_params(),
            5
        );
        assert_eq!(
            module
                .get_function(&format!("{handle}_resume"))
                .unwrap()
                .count_params(),
            2
        );
        assert_eq!(
            module
                .get_function(&format!("{handle}_release"))
                .unwrap()
                .count_params(),
            1
        );
    }
    let shared = Guarded::new(program.shared_data_bytes as usize);
    unsafe { std::slice::from_raw_parts_mut(shared.ptr(), shared.len) }.copy_from_slice(seed);
    let private = Guarded::new(
        keleusma::vm::required_persistent_capacity_for(program)
            + region::persistent_supplement_bytes(program) as usize,
    );
    let region = Guarded::new(region::host_arena_supplement_bytes(program) as usize);
    common::install_private_init_bytes(program, unsafe {
        std::slice::from_raw_parts_mut(private.ptr(), private.len)
    });
    let used = frame.arena.bottom_used();
    let mut result = Vec::new();
    let mut step = unsafe {
        if stable {
            let started = if float_input {
                engine
                    .get_function::<HandleFloatStart>(&format!("{handle}_start"))
                    .unwrap()
                    .call(
                        NativeFloat::from_bits(first as _),
                        shared.ptr(),
                        private.ptr(),
                        region.ptr(),
                        frame.ptr(),
                    )
            } else {
                engine
                    .get_function::<HandleStart>(&format!("{handle}_start"))
                    .unwrap()
                    .call(
                        first,
                        shared.ptr(),
                        private.ptr(),
                        region.ptr(),
                        frame.ptr(),
                    )
            };
            started.step(frame.ptr())
        } else if float_input {
            engine.get_function::<FloatStart>(&entry).unwrap().call(
                NativeFloat::from_bits(first as _),
                shared.ptr(),
                private.ptr(),
                region.ptr(),
                frame.ptr(),
                reply.ptr(),
            )
        } else {
            engine.get_function::<Start>(&entry).unwrap().call(
                first,
                shared.ptr(),
                private.ptr(),
                region.ptr(),
                frame.ptr(),
                reply.ptr(),
            )
        }
    };
    for (i, &value) in replies.iter().enumerate() {
        assert!(!step.next.is_null());
        result.push(read(step.value, &region, &private, &frame));
        for storage in [&frame, &reply, &shared, &private, &region] {
            storage.check();
        }
        if i + 1 < replies.len() {
            if stable {
                step = unsafe {
                    engine
                        .get_function::<HandleResume>(&format!("{handle}_resume"))
                        .unwrap()
                        .call(frame.ptr(), value)
                }
                .step(frame.ptr());
            } else {
                unsafe {
                    reply.ptr().cast::<i64>().write(value);
                }
                let resume: Resume = unsafe { std::mem::transmute(step.next) };
                step = unsafe { resume(frame.ptr(), false) };
            }
        }
    }
    if stable {
        unsafe {
            engine
                .get_function::<HandleRelease>(&format!("{handle}_release"))
                .unwrap()
                .call(frame.ptr());
        }
        let after = unsafe {
            engine
                .get_function::<HandleResume>(&format!("{handle}_resume"))
                .unwrap()
                .call(frame.ptr(), 99)
        };
        assert_eq!(after, Outcome { live: 0, value: 0 });
    } else {
        let release: Resume = unsafe { std::mem::transmute(step.next) };
        let end = unsafe { release(frame.ptr(), true) };
        assert!(end.next.is_null());
    }
    assert_eq!(frame.arena.bottom_used(), used);
    for storage in [&frame, &reply, &shared, &private, &region] {
        storage.check();
    }
    (
        result,
        unsafe { std::slice::from_raw_parts(shared.ptr(), shared.len) }.to_vec(),
    )
}

#[test]
fn non_tail_yield_preserves_locals_operands_and_reply() {
    let src =
        "loop main(a: Word) -> Word { let keep = a + 100; let r = 10 + (yield a); yield r + keep }";
    let replies = [7, 20, 3, 40, 1, 0];
    let expected = [5, 122, 20, 133, 40, 151];
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(src, 5, &replies, optimize), expected);
    }
}

#[test]
fn nested_branches_and_private_state_match_the_vm() {
    let src = "private data st { cursor: Word }\nloop main(a: Word) -> Word { st.cursor = st.cursor + 1; if a > 10 { if a > 100 { yield st.cursor * 1000 + a } else { yield st.cursor * 100 + a } } else { yield st.cursor * 10 + a } }";
    let replies = [200, 75, 25, 1, 200, 0];
    let expected = [15, 2200, 375, 425, 51, 6200];
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    assert_eq!(native(src, 5, &replies, false), expected);
}

#[test]
fn undersized_frame_is_a_compile_time_refusal() {
    let p = common::build(
        "loop main(a: Word) -> Word { let keep = a + 100; let r = yield 1; yield r + keep }",
    );
    let ctx = Context::create();
    let err = coroutine::lower(&ctx, &p, &machine(), 8).unwrap_err();
    assert!(err.to_string().contains("frame exceeds"), "{err}");
}

#[test]
fn branch_joins_loop_back_edges_and_calls_resume_correctly() {
    let shapes = [
        "loop main(t: Word) -> Word { if t > 0 { let a = yield t; yield a } else { yield 0 } }",
        "loop main(t: Word) -> Word { let r = yield 1; yield r + t }",
        "loop main(t: Word) -> Word { (yield t) + (yield t + 1) }",
        "fn plus(a: Word) -> Word { a + 19 } loop main(t: Word) -> Word { let a = plus(t); let r = yield a; yield plus(r + a) }",
        "loop main(t: Word) -> Word { for i in 0..3 { yield i + t; } yield t + 100 }",
    ];
    let replies: Vec<_> = (0..80).map(|i| if i % 3 == 0 { -i } else { i }).collect();
    for src in shapes {
        let expected = common::general_vm_sequence(src, 7, &replies);
        for optimize in [false, true] {
            assert_eq!(native(src, 7, &replies, optimize), expected, "{src}");
        }
    }
}

#[test]
fn lexer_retcon_matches_seeded_vm_tokens_and_shared_writes() {
    use keleusma::bytecode::{SlotVisibility, Value};
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    let program = common::build(&std::fs::read_to_string("../src/selfhost/kel/lexer.kel").unwrap());
    let text = b"fn add(x: Word, y: Word) -> Word { x + y } loop main(t: Word) -> Word { if t > 10 { yield add(t, 2) } else { yield 0 } }";
    let layout = program.data_layout.as_ref().unwrap();
    let offset = |suffix: &str| {
        let index = layout
            .slots
            .iter()
            .filter(|s| s.visibility == SlotVisibility::Shared)
            .position(|s| {
                s.name.ends_with(&format!(".{suffix}"))
                    || s.name.ends_with(&format!(".{suffix}[0]"))
                    || s.name == suffix
            })
            .unwrap();
        layout.shared_layout[index].offset as usize
    };
    let mut seed = vec![0; program.shared_data_bytes as usize];
    let len = offset("len");
    let bytes = offset("bytes");
    seed[len..len + 8].copy_from_slice(&(text.len() as i64).to_le_bytes());
    seed[bytes..bytes + text.len()].copy_from_slice(text);
    let need = required_persistent_capacity_for(&program);
    let mut arena = keleusma_arena::Arena::with_capacity(
        auto_arena_capacity_for(&program, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    let mut shared = seed.clone();
    let mut state = vm.call_with_shared(&mut shared, &[Value::Int(0)]).unwrap();
    let replies: Vec<i64> = (1..401).collect();
    let mut expected = Vec::new();
    for (index, &r) in replies.iter().enumerate() {
        let VmState::Yielded(Value::Int(v)) = state else {
            panic!("unexpected {state:?}")
        };
        expected.push(v);
        if index + 1 < replies.len() {
            state = vm.resume_with_shared(&mut shared, Value::Int(r)).unwrap();
            if matches!(state, VmState::Reset) {
                state = vm.resume_with_shared(&mut shared, Value::Int(r)).unwrap();
            }
        }
    }
    let codes: std::collections::BTreeSet<_> = expected.iter().map(|v| v & 255).collect();
    assert!(codes.len() >= 10, "vacuous lexer drive: {codes:?}");
    for optimize in [false, true] {
        let (got, written) = native_module(&program, &seed, 0, &replies, optimize);
        assert_eq!(got, expected);
        assert_eq!(written, shared);
    }
}

#[test]
fn two_instances_release_independently_and_reuse_a_frame() {
    let program = common::build(
        "loop main(a: Word) -> Word { let keep = a + 100; let r = yield a; yield r + keep }",
    );
    let ctx = Context::create();
    let module = coroutine::lower(&ctx, &program, &machine(), 128).unwrap();
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    let start = unsafe {
        engine.get_function::<Start>(&format!("kel_chunk_{}", program.entry_point.unwrap()))
    }
    .unwrap();
    // These programs never touch the ordinary data regions. Non-null zero-size
    // guarded buffers catch an accidental dependency on the old dispatch state.
    let unused = Guarded::new(0);
    let frames = [Guarded::new(128), Guarded::new(128)];
    let replies = [Guarded::new(8), Guarded::new(8)];
    let launch = |i: usize, first| unsafe {
        start.call(
            first,
            unused.ptr(),
            unused.ptr(),
            unused.ptr(),
            frames[i].ptr(),
            replies[i].ptr(),
        )
    };
    let mut a = launch(0, 5);
    let mut b = launch(1, 99);
    assert_eq!((a.value, b.value), (5, 99));
    unsafe {
        replies[1].ptr().cast::<i64>().write(22);
    }
    let resume_b: Resume = unsafe { std::mem::transmute(b.next) };
    b = unsafe { resume_b(frames[1].ptr(), false) };
    assert_eq!(b.value, 221);
    let release_b: Resume = unsafe { std::mem::transmute(b.next) };
    assert!(unsafe { release_b(frames[1].ptr(), true) }.next.is_null());
    unsafe {
        replies[0].ptr().cast::<i64>().write(11);
    }
    let resume_a: Resume = unsafe { std::mem::transmute(a.next) };
    a = unsafe { resume_a(frames[0].ptr(), false) };
    assert_eq!(a.value, 116);
    // Reuse released bytes without clearing them. Start must initialise its own
    // frame rather than inheriting the previous continuation or live locals.
    b = launch(1, 33);
    assert_eq!(b.value, 33);
    for (i, step) in [a, b].into_iter().enumerate() {
        let release: Resume = unsafe { std::mem::transmute(step.next) };
        assert!(unsafe { release(frames[i].ptr(), true) }.next.is_null());
        frames[i].check();
        replies[i].check();
    }
    unused.check();
}

#[test]
fn invalid_bytecode_and_composite_escape_remain_refused() {
    let ctx = Context::create();
    let tm = machine();
    let mut invalid = common::build("loop main(a: Word) -> Word { yield a }");
    let entry = invalid.entry_point.unwrap();
    invalid.chunks[entry].ops[1] = keleusma::bytecode::Op::GetLocal(u16::MAX);
    assert!(
        coroutine::lower(&ctx, &invalid, &tm, 4096)
            .unwrap_err()
            .to_string()
            .contains("verification")
    );
    let escaping = common::build(
        &std::fs::read_to_string("../examples/scripts/13_telemetry_stream.kel").unwrap(),
    );
    let baseline = keleusma_native::module_refusals(&escaping, Default::default());
    assert!(!baseline.is_empty());
    let error = coroutine::lower(&ctx, &escaping, &tm, 4096).unwrap_err();
    assert!(
        baseline
            .iter()
            .any(|(_, original)| original.to_string() == error.to_string()),
        "{error}"
    );
    // An unrelated reentrant yield must neither add a boundary requirement
    // nor bypass the reachable composite escape refusal.
    let source = std::fs::read_to_string("../examples/scripts/13_telemetry_stream.kel").unwrap();
    let with_delegate = common::build(&format!(
        "{source}\nyield unused(a: Word) -> Word {{ yield a }}"
    ));
    let delegated_error = coroutine::lower(&ctx, &with_delegate, &tm, 4096).unwrap_err();
    assert_eq!(delegated_error.to_string(), error.to_string());
}

#[test]
fn bytecode_coroutine_links_and_runs_from_a_c_host() {
    use std::process::Command;
    let program = common::build(
        "loop main(a: Word) -> Word { let keep = a + 100; let r = yield a; yield r + keep }",
    );
    let ctx = Context::create();
    let tm = machine();
    let module = coroutine::lower(&ctx, &program, &tm, 128).unwrap();
    let dir =
        std::path::PathBuf::from("../tmp").join(format!("retcon-bytecode-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("coroutine.o");
    tm.write_to_file(&module, inkwell::targets::FileType::Object, &object)
        .unwrap();
    let host = dir.join("host.c");
    std::fs::write(
        &host,
        format!(
            r#"
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdalign.h>
struct step {{ void *next; int64_t value; }};
typedef struct step (*resume_fn)(void *, bool);
extern struct step kel_chunk_{entry}(int64_t, void *, void *, void *, void *, void *);
int main(void) {{
    alignas(8) unsigned char frame[128] = {{0}};
    int64_t reply = 0;
    struct step s = kel_chunk_{entry}(5, 0, 0, 0, frame, &reply);
    assert(s.next && s.value == 5);
    reply = 11;
    s = ((resume_fn)s.next)(frame, false);
    assert(s.next && s.value == 116);
    s = ((resume_fn)s.next)(frame, true);
    assert(!s.next);
    return 0;
}}
"#,
            entry = program.entry_point.unwrap()
        ),
    )
    .unwrap();
    let executable = dir.join("host");
    let linked = Command::new("cc")
        .arg("-std=c11")
        .arg(&host)
        .arg(&object)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let run = Command::new(&executable).output().unwrap();
    assert!(
        run.status.success(),
        "{:?}: {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn scalar_reply_widths_and_kinds_match_the_vm() {
    use keleusma::bytecode::{TypeTag, Value};
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    let cases = [
        "loop main(t: ()) -> () { yield t; yield t }",
        "loop main(t: Byte) -> Byte { let q: Byte = t + (1 as Byte); let r = yield t; yield q + r }",
        "loop main(t: bool) -> bool { let r = yield t; yield (r and t) }",
        "loop main(t: Fixed) -> Fixed { let r = yield t; yield (r * t) }",
        "loop main(t: Float) -> Float { let keep = -t; let r = yield t; yield (r + keep) }",
        "loop main(t: Float) -> Word { let r = yield (t as Word); yield (r as Word) }",
        "yield emit(a: ()) -> () { yield a; yield a } loop main(t: ()) -> () { emit(t) }",
        "yield emit(a: ()) -> () { yield (); yield () } loop main(t: ()) -> () { emit(t) }",
        "loop child(a: ()) -> () { yield a; yield a } loop main(t: ()) -> () { child(t) }",
        "private data st { value: Float = 0.5 } loop child() -> Float { st.value = st.value + 0.25; yield st.value } loop main(t: Float) -> Float { child() }",
        "private data st { value: Fixed } loop child() -> Fixed { st.value = st.value + (1 as Fixed); yield st.value } loop main(t: Fixed) -> Fixed { child() }",
        "yield emit(a: Byte) -> Byte { let q: Byte = a + (1 as Byte); let r = yield a; yield q + r + a } loop main(t: Byte) -> Byte { emit(t) }",
        "yield emit(a: bool) -> bool { let r = yield a; yield (r and a) } loop main(t: bool) -> bool { emit(t) }",
        "yield emit(a: Fixed) -> Fixed { let r = yield a; yield (r * a) } loop main(t: Fixed) -> Fixed { emit(t) }",
        "yield emit(a: Float) -> Float { let r = yield a; yield (r + a) } loop main(t: Float) -> Float { emit(t) }",
    ];
    for src in cases {
        let p = common::build(src);
        let tag = p.chunks[p.entry_point.unwrap()].param_types[0];
        let encode = |v: i64| match tag {
            TypeTag::Unit => 0,
            TypeTag::Bool => i64::from(v != 0),
            TypeTag::Fixed => v * 65536,
            TypeTag::Float => (v as NativeFloat / 4.0).to_bits() as i64,
            _ => v,
        };
        let value = |bits: i64| match tag {
            TypeTag::Unit => Value::Unit,
            TypeTag::Byte => Value::Byte(bits as u8),
            TypeTag::Bool => Value::Bool(bits != 0),
            TypeTag::Fixed => Value::Fixed(bits),
            TypeTag::Float => Value::Float(wide(NativeFloat::from_bits(bits as _))),
            _ => unreachable!(),
        };
        let first = encode(2);
        let replies: Vec<_> = [1, 3, 0, 2, 1, 0].into_iter().map(encode).collect();
        let need = required_persistent_capacity_for(&p);
        let mut arena = keleusma_arena::Arena::with_capacity(
            auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
        );
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(p, &arena).unwrap();
        let mut state = vm.call(&[value(first)]).unwrap();
        let mut expected = Vec::new();
        for (i, &reply) in replies.iter().enumerate() {
            let VmState::Yielded(yielded) = state.clone() else {
                panic!("{state:?}")
            };
            expected.push(match yielded {
                Value::Unit => 0,
                Value::Int(v) => v,
                Value::Byte(v) => i64::from(v),
                Value::Bool(v) => i64::from(v),
                Value::Fixed(v) => v,
                Value::Float(v) => {
                    assert_eq!(
                        wide(v as NativeFloat),
                        v,
                        "fixture must be exact at both widths"
                    );
                    (v as NativeFloat).to_bits() as i64
                }
                other => panic!("{other:?}"),
            });
            if i + 1 < replies.len() {
                state = vm.resume(value(reply)).unwrap();
                if matches!(state, VmState::Reset) {
                    state = vm.resume(value(reply)).unwrap();
                }
            }
        }
        for optimize in [false, true] {
            assert_eq!(native(src, first, &replies, optimize), expected, "{src}");
        }
    }
    // The VM can expose a callee's distinct scalar tag despite the entry's
    // return signature. The untagged native payload must refuse that mismatch,
    // including Byte versus Bool, which have equal native widths.
    for (src, expected) in [
        (
            "loop child() -> bool { yield true } loop main(t: Word) -> Byte { child(); yield (t as Byte) }",
            Value::Bool(true),
        ),
        (
            "yield emit() -> Float { yield 0.5 } loop main(t: Word) -> Word { emit(); yield t }",
            Value::Float(0.5),
        ),
    ] {
        let p = common::build(src);
        keleusma::verify::verify(&p).unwrap();
        let ctx = Context::create();
        let error = coroutine::lower(&ctx, &p, &machine(), 4096).unwrap_err();
        assert!(
            error.to_string().contains("scalar types are not proven"),
            "{error}"
        );
        let need = required_persistent_capacity_for(&p);
        let mut arena = keleusma_arena::Arena::with_capacity(
            auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
        );
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(p, &arena).unwrap();
        let VmState::Yielded(actual) = vm.call(&[Value::Int(0)]).unwrap() else {
            panic!("callee must yield its distinct scalar type");
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn delegated_yields_preserve_callee_locals_and_update_only_the_entry_parameter() {
    let src = "yield emit(a: Word) -> Word { let keep = a + 100; let x = yield a; yield keep + x }\nloop main(t: Word) -> Word { let keep = t + 1000; let v = emit(t + 1); yield keep + v + t }";
    let replies = [7, 20, 3, 11, 30, 9, 0];
    let expected = [6, 113, 1045, 4, 115, 1063, 10];
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(src, 5, &replies, optimize), expected);
    }
    let src = "yield emit(a: Word) -> Word { yield a } loop main(t: Word) -> Word { yield (t + 100) + emit(t) }";
    let replies = [7, 11, 13, 17];
    let expected = [5, 112, 11, 124];
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(src, 5, &replies, optimize), expected);
    }
}

#[test]
fn nested_delegation_preserves_each_call_frame() {
    let src = "yield inner(a: Word) -> Word { let keep = a + 100; let r = yield a; yield keep + r + a }\nyield outer(a: Word) -> Word { let keep = a * 10; let v = inner(a + 2); yield keep + v + a }\nloop main(t: Word) -> Word { let v = outer(t + 1); yield v + t }";
    let replies = [7, 11, 13, 17, 19, 23, 29, 31];
    let expected = [5, 117, 44, 26, 20, 159, 221, 58];
    assert_eq!(common::general_vm_sequence(src, 2, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(src, 2, &replies, optimize), expected);
    }
}

#[test]
fn release_inside_a_callee_does_not_execute_its_remaining_effects() {
    let p = common::build(
        "shared data st { n: Word }\nyield emit(a: Word) -> Word { st.n = st.n + 1; yield a; st.n = st.n + 100; yield a + 1 }\nloop main(t: Word) -> Word { emit(t) }",
    );
    let seed = vec![0; p.shared_data_bytes as usize];
    for optimize in [false, true] {
        for (last, expected_shared) in [1, 101, 102, 202].into_iter().enumerate() {
            let replies = [7, 11, 13, 17];
            let (values, shared) = native_module(&p, &seed, 5, &replies[..=last], optimize);
            assert_eq!(values, [5, 6, 11, 12][..=last]);
            assert_eq!(
                i64::from_le_bytes(shared[..8].try_into().unwrap()),
                expected_shared
            );
        }
    }
}

#[test]
fn delegated_composite_yields_remain_refused() {
    let p = common::build(
        "yield emit(a: Word) -> (Word, Word) { yield (a, a) } loop main(t: Word) -> Word { emit(t); yield t }",
    );
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(
        error.to_string().contains("scalar types are not proven"),
        "{error}"
    );
    // A live body operand is now admitted. Only the signature mismatch above
    // remains a refusal, so preserve the former negative as an executed control.
    let src = "fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } yield emit(a: Word) -> Word { yield a } loop main(t: Word) -> Word { yield pick((t, t), emit(t)) }";
    let replies = [7, 20, 3, 40, 1, 0];
    let expected = [5, 12, 20, 23, 40, 41];
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(src, 5, &replies, optimize), expected);
    }
}

#[test]
fn guarded_heads_dispatch_across_suspensions() {
    let cases = [
        "yield emit(a: Word) -> Word when a == 0 { yield 10 } yield emit(a: Word) -> Word when a > 5 { yield a + 100 } yield emit(a: Word) -> Word { yield a + 20 } loop main(t: Word) -> Word { emit(t) }",
        "loop main(t: Word) -> Word when t == 0 { yield 10 } loop main(t: Word) -> Word when t > 5 { yield t + 100 } loop main(t: Word) -> Word { yield t + 20 }",
    ];
    let replies = [7, 2, 0, 9, 3, 0];
    for src in cases {
        let expected = common::general_vm_sequence(src, 0, &replies);
        assert_eq!(expected, [10, 107, 22, 10, 109, 23]);
        for optimize in [false, true] {
            assert_eq!(native(src, 0, &replies, optimize), expected);
        }
    }
}

#[test]
fn nested_stream_calls_preserve_state_across_callee_resets() {
    let cases = [
        (
            "loop child() -> Word { yield 7 } loop main(t: Word) -> Word { child() }",
            vec![7; 6],
        ),
        (
            "loop child(ignored: Word) -> Word { yield 7 } loop main(t: Word) -> Word { child(t) }",
            vec![7; 6],
        ),
        (
            "private data st { n: Word } loop child() -> Word { let keep = st.n + 100; st.n = st.n + 1; yield st.n; yield keep + st.n } loop main(t: Word) -> Word { child() }",
            vec![1, 101, 2, 103, 3, 105],
        ),
        (
            "yield emit(a: Word) -> Word { let keep = a + 100; let r = yield a; yield keep + r } loop child() -> Word { emit(5) } loop middle() -> Word { child() } loop main(t: Word) -> Word { middle() }",
            vec![5, 112, 5, 108, 5, 135],
        ),
    ];
    let cases = cases.into_iter().chain(std::iter::once((
        "private data st { xs: [Word; 2] } loop child() -> Word { st.xs[0] = st.xs[0] + 1; st.xs[1] = st.xs[1] + 10; for i in 0..2 { yield st.xs[i] }; yield st.xs[0] } loop main(t: Word) -> Word { child() }",
        vec![1, 10, 1, 2, 20, 2],
    )));
    let replies = [7, 20, 3, 11, 30, 9];
    for (src, expected) in cases {
        assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
        for optimize in [false, true] {
            assert_eq!(native(src, 5, &replies, optimize), expected);
        }
    }
}

#[test]
fn nested_stream_parameter_reads_do_not_silently_turn_unit_into_zero() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    let p = common::build(
        "loop child(a: Word) -> Word { yield a + 1 } loop main(t: Word) -> Word { child(t) }",
    );
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("non-Unit parameter cleared by Reset")
    );
    let need = required_persistent_capacity_for(&p);
    let mut arena = keleusma_arena::Arena::with_capacity(
        auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(p, &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(5)]).unwrap(),
        VmState::Yielded(Value::Int(6))
    ));
    assert!(matches!(vm.resume(Value::Int(7)).unwrap(), VmState::Reset));
    let keleusma::vm::VmError::TypeError(fault) = vm.resume(Value::Int(7)).unwrap_err() else {
        panic!("the cleared parameter must produce a type error");
    };
    assert!(fault.contains("Unit") && fault.contains("Int"), "{fault}");
}

#[test]
fn nested_stream_release_stops_before_the_next_iteration() {
    let src = "shared data st { n: Word } loop child() -> Word { st.n = st.n + 1; yield st.n } loop main(t: Word) -> Word { child() }";
    let p = common::build(src);
    assert_eq!(common::general_vm_sequence(src, 5, &[9; 4]), [1, 2, 3, 4]);
    let seed = vec![0; p.shared_data_bytes as usize];
    for optimize in [false, true] {
        for count in 1..=4 {
            let (values, shared) = native_module(&p, &seed, 5, &vec![9; count], optimize);
            assert_eq!(values, (1..=count as i64).collect::<Vec<_>>());
            assert_eq!(
                i64::from_le_bytes(shared[..8].try_into().unwrap()),
                count as i64
            );
        }
    }
}

#[test]
fn coroutine_private_scalar_admission_checks_writes_and_indexed_ranges() {
    use keleusma::bytecode::ConstValue;
    let ctx = Context::create();
    let tm = machine();
    let mut changed = common::build(
        "private data st { n: Word } loop main(t: Word) -> Word { st.n = t; yield st.n }",
    );
    // Verified bytecode can change a slot's category after load. The native
    // scalar inference must decline that case instead of trusting the first
    // value's category for every later read.
    changed.data_layout.as_mut().unwrap().private_init[0] = ConstValue::Float(0.0);
    keleusma::verify::verify(&changed).unwrap();
    let error = coroutine::lower(&ctx, &changed, &tm, 4096).unwrap_err();
    assert!(
        error.to_string().contains("coroutine scalar types"),
        "{error}"
    );

    let mut indexed = common::build(
        "private data st { xs: [Word; 2] } loop main(t: Word) -> Word { st.xs[0] = t; yield st.xs[t % 2] }",
    );
    indexed.data_layout.as_mut().unwrap().private_init[1] = ConstValue::Float(0.0);
    keleusma::verify::verify(&indexed).unwrap();
    let error = coroutine::lower(&ctx, &indexed, &tm, 4096).unwrap_err();
    assert!(
        error.to_string().contains("coroutine scalar types"),
        "{error}"
    );
}

#[test]
fn mixed_reply_arithmetic_is_refused_where_the_vm_faults() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{
        Vm, VmError, VmState, auto_arena_capacity_for, required_persistent_capacity_for,
    };
    for (tag, input) in [
        ("Byte", Value::Byte(5)),
        ("bool", Value::Bool(true)),
        ("Fixed", Value::Fixed(65536)),
        ("Float", Value::Float(5.25)),
        ("()", Value::Unit),
    ] {
        for expression in [
            "r + 1", "r - 1", "r * 2", "r / 2", "r % 2", "r band 1", "r bor 1", "r bxor 1",
            "r lsl 1",
        ] {
            for delegated in [false, true] {
                let src = if delegated {
                    format!(
                        "yield emit() -> Word {{ let r = yield 5; yield {expression} }} loop main(t: {tag}) -> Word {{ emit() }}"
                    )
                } else {
                    format!("loop main(t: {tag}) -> Word {{ let r = yield 5; yield {expression} }}")
                };
                let p = common::build(&src);
                keleusma::verify::verify(&p).unwrap();
                let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
                assert!(
                    error.to_string().contains("coroutine scalar types")
                        || error.to_string().contains("consume a float")
                        || error.to_string().contains("one side is a float"),
                    "{src}: {error}"
                );
                let need = required_persistent_capacity_for(&p);
                let mut arena = keleusma_arena::Arena::with_capacity(
                    auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
                );
                arena.resize_persistent(need).unwrap();
                let mut vm = Vm::new(p, &arena).unwrap();
                assert!(matches!(
                    vm.call(std::slice::from_ref(&input)).unwrap(),
                    VmState::Yielded(Value::Int(5))
                ));
                let VmError::TypeError(message) = vm.resume(input.clone()).unwrap_err() else {
                    panic!("expected mixed-type arithmetic fault");
                };
                assert!(!message.is_empty(), "{src}");
            }
        }
    }
}

#[test]
fn reply_tags_are_checked_through_calls_and_branch_joins() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    for src in [
        "loop main(t: Byte) -> Word { let r = yield 5; yield r }",
        "yield emit() -> Word { yield 5 } loop main(t: Byte) -> Word { let r = emit(); yield r }",
        "loop main(t: Byte) -> Word { let r = yield 5; let v = if t > (0 as Byte) { r } else { 0 }; yield v }",
    ] {
        let p = common::build(src);
        keleusma::verify::verify(&p).unwrap();
        let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
        assert!(
            error.to_string().contains("coroutine scalar types"),
            "{error}"
        );
        let need = required_persistent_capacity_for(&p);
        let mut arena = keleusma_arena::Arena::with_capacity(
            auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
        );
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(p, &arena).unwrap();
        assert!(matches!(
            vm.call(&[Value::Byte(5)]).unwrap(),
            VmState::Yielded(Value::Int(5))
        ));
        assert!(matches!(
            vm.resume(Value::Byte(7)).unwrap(),
            VmState::Yielded(Value::Byte(7))
        ));
    }
    // A matching byte reply remains usable through the same branch shape.
    let src = "loop main(t: Byte) -> Byte { let r = yield t; let v = if t > (0 as Byte) { r } else { 0 as Byte }; yield v + (1 as Byte) }";
    for optimize in [false, true] {
        assert_eq!(
            native(src, 5, &[7, 9, 0, 10, 2, 0], optimize),
            [5, 8, 9, 1, 10, 3]
        );
    }
}

#[test]
fn coroutine_scalar_admission_requires_complete_parameter_signatures() {
    let mut p = common::build(
        "yield emit(a: Word) -> Word { yield a } loop main(t: Word) -> Word { emit(t) }",
    );
    let delegate = p.chunks.iter().position(|c| c.name == "emit").unwrap();
    p.signatures[delegate].params.clear();
    // The verifier permits absent shape metadata. Native coroutine admission
    // cannot interpret that absence as evidence about an untagged value.
    keleusma::verify::verify(&p).unwrap();
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(error.to_string().contains("complete signatures"), "{error}");
    let mut p = common::build("loop main(t: Byte) -> Word { let r = yield 5; yield r + 1 }");
    let entry = p.entry_point.unwrap();
    p.signatures[entry].params[0] = keleusma::bytecode::WireShape::Scalar {
        kind: keleusma::value_layout::ScalarKind::Int.to_tag(),
    };
    keleusma::verify::verify(&p).unwrap();
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(
        error.to_string().contains("parameter signature disagrees"),
        "{error}"
    );
}

#[test]
fn scalar_reply_admission_reaches_a_later_loop_iteration() {
    use keleusma::bytecode::{Op, Value};
    use keleusma::vm::{
        Vm, VmError, VmState, auto_arena_capacity_for, required_persistent_capacity_for,
    };
    let mut p =
        common::build("loop main(t: Word) -> Word { for i in 0..2 { yield t + 1; } yield 0 }");
    let entry = p.entry_point.unwrap();
    let ops = &mut p.chunks[entry].ops;
    let edge = ops
        .iter()
        .position(|op| matches!(op, Op::EndLoop(_)))
        .unwrap()
        - 5;
    // Keep the canonical five-instruction induction step intact so the
    // resource verifier still proves the loop bound.
    assert!(matches!(ops[edge], Op::GetLocal(_)));
    for op in ops.iter_mut() {
        if let Op::If(t)
        | Op::Else(t)
        | Op::Loop(t)
        | Op::EndLoop(t)
        | Op::Break(t)
        | Op::BreakIf(t) = op
            && *t as usize >= edge
        {
            *t += 2;
        }
    }
    ops.splice(edge..edge, [Op::PushImmediate(1), Op::SetLocal(0)]);
    keleusma::verify::verify(&p).unwrap();
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(error.to_string().contains("CheckedAdd"), "{error}");
    let need = required_persistent_capacity_for(&p);
    let mut arena = keleusma_arena::Arena::with_capacity(
        auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(p, &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(5)]).unwrap(),
        VmState::Yielded(Value::Int(6))
    ));
    let VmError::TypeError(message) = vm.resume(Value::Int(7)).unwrap_err() else {
        panic!("later iteration must see the changed parameter type");
    };
    assert!(message.contains("CheckedAdd"), "{message}");
}

#[test]
fn scalar_replies_do_not_become_enum_pointers() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    for (tag, bits, input) in [
        ("Word", 7, Value::Int(7)),
        ("Byte", 7, Value::Byte(7)),
        ("bool", 1, Value::Bool(true)),
        ("()", 0, Value::Unit),
        ("Fixed", 7 * 65536, Value::Fixed(7 * 65536)),
        (
            "Float",
            (7.25 as NativeFloat).to_bits() as i64,
            Value::Float(7.25),
        ),
    ] {
        for (variants, pattern, arm) in [("A, B", "E::A", "1"), ("A(Word), B", "E::A(x)", "x")] {
            for delegated in [false, true] {
                let body = format!(
                    "let r = yield 5; let _ = yield match r {{ {pattern} => {arm}, _ => 0 }};"
                );
                let src = if delegated {
                    format!(
                        "enum E {{ {variants} }} yield inspect() -> Word {{ {body} 0 }} loop main(t: {tag}) -> Word {{ inspect(); yield 99 }}"
                    )
                } else {
                    format!("enum E {{ {variants} }} loop main(t: {tag}) -> Word {{ {body} 0 }}")
                };
                if !delegated && pattern == "E::A(x)" {
                    // The core verifier already knows a direct scalar resume
                    // cannot supply a composite field, even in this dead arm.
                    let ast =
                        keleusma::parser::parse(&keleusma::lexer::tokenize(&src).unwrap()).unwrap();
                    let error = keleusma::compiler::compile(&ast).unwrap_err();
                    assert!(error.message.contains("ExpectedComposite"), "{error:?}");
                    continue;
                }
                let p = common::build(&src);
                let before = format!("{p:?}");
                let expected: &[i64] = if delegated {
                    &[5, 0, 99, 5, 0, 99]
                } else {
                    &[5, 0, 5, 0, 5, 0]
                };
                let need = required_persistent_capacity_for(&p);
                let mut arena = keleusma_arena::Arena::with_capacity(
                    auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
                );
                arena.resize_persistent(need).unwrap();
                let mut vm = Vm::new(p.clone(), &arena).unwrap();
                let mut state = vm.call(std::slice::from_ref(&input)).unwrap();
                for (i, value) in expected.iter().enumerate() {
                    assert!(
                        matches!(state, VmState::Yielded(Value::Int(v)) if v == *value),
                        "{src}: {state:?}"
                    );
                    if i + 1 < expected.len() {
                        state = vm.resume(input.clone()).unwrap();
                        if matches!(state, VmState::Reset) {
                            state = vm.resume(input.clone()).unwrap();
                        }
                    }
                }
                for optimize in [false, true] {
                    let seed = vec![0; p.shared_data_bytes as usize];
                    assert_eq!(
                        native_module(&p, &seed, bits, &vec![bits; expected.len()], optimize).0,
                        expected,
                        "{src}"
                    );
                }
                assert_eq!(
                    format!("{p:?}"),
                    before,
                    "lowering must preserve the input bytecode"
                );
            }
        }
    }
}

#[test]
fn coroutine_inspection_distinguishes_enum_and_struct_bodies() {
    use keleusma::bytecode::{NewCompositeOperand, Op, Value};
    use keleusma::value_layout::CompositeKind;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    let src = "enum E { A, B } loop main(t: Word) -> Word { let e = E::A; let r = yield t; yield match e { E::A => r, _ => 0 } }";
    let p = common::build(src);
    for optimize in [false, true] {
        assert_eq!(native(src, 5, &[7, 9, 1, 0], optimize), [5, 7, 9, 1]);
    }
    let mut changed = p;
    let ops = &mut changed.chunks[changed.entry_point.unwrap()].ops;
    let constructor = ops
        .iter_mut()
        .find(|o| matches!(o, Op::NewComposite(_)))
        .unwrap();
    let Op::NewComposite(NewCompositeOperand::Flat { kind, .. }) = constructor else {
        panic!("flat constructor")
    };
    *kind = CompositeKind::Struct;
    keleusma::verify::verify(&changed).unwrap();
    let need = required_persistent_capacity_for(&changed);
    let mut arena = keleusma_arena::Arena::with_capacity(
        auto_arena_capacity_for(&changed, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(changed.clone(), &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(5)]).unwrap(),
        VmState::Yielded(Value::Int(5))
    ));
    assert!(matches!(
        vm.resume(Value::Int(7)).unwrap(),
        VmState::Yielded(Value::Int(0))
    ));
    for optimize in [false, true] {
        assert_eq!(
            native_module(&changed, &[], 5, &[7, 9, 1, 0], optimize).0,
            [5, 0, 9, 0]
        );
    }
}

#[test]
fn mixed_enum_receiver_kinds_execute_with_guarded_body_inspection() {
    // Scalar bits include null and invalid addresses. The false edge must never
    // interpret them as a body pointer. A successful test also refines payload
    // reads, including the compiler's Boolean temporary and structured match.
    for (declaration, branch, pattern, expected_true, enum_on_positive) in [
        ("A, B", "if t > 0 { E::A } else { r }", "E::A => 1", 1, true),
        (
            "A, B",
            "if t > 0 { r } else { E::A }",
            "E::A => 1",
            1,
            false,
        ),
        (
            "A(Word), B",
            "if t > 0 { E::A(9) } else { r }",
            "E::A(x) => x",
            9,
            true,
        ),
        (
            "A(Word), B",
            "if t > 0 { r } else { E::A(9) }",
            "E::A(x) => x",
            9,
            false,
        ),
    ] {
        let src = format!(
            "enum E {{ {declaration} }} loop main(t: Word) -> Word {{ let r = yield 5; let e = {branch}; yield match e {{ {pattern}, _ => 0 }} }}"
        );
        for reply in [7, -7, 0, i64::MIN, i64::MAX] {
            let expected = if (reply > 0) == enum_on_positive {
                expected_true
            } else {
                0
            };
            let replies = [reply, 3, reply, 3];
            let want = [5, expected, 5, expected];
            assert_eq!(common::general_vm_sequence(&src, 5, &replies), want);
            for optimize in [false, true] {
                assert_eq!(
                    native(&src, 5, &replies, optimize),
                    want,
                    "{src} reply {reply} optimize {optimize}"
                );
            }
        }
    }
}

#[test]
fn mixed_owned_bodies_keep_selected_extents_across_suspension() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, -40, 1]
        .into_iter()
        .map(|a| {
            Value::enum_value(
                "E".into(),
                "Pair".into(),
                0,
                vec![Value::Int(a), Value::Int(a * 10)],
            )
        })
        .collect();
    for other in ["0", "1.5", "(99, 100)"] {
        for (condition, expected) in [
            ("n > 0", [5, 6, 7, 5, 6, 0]),
            ("n < 0", [5, 6, 0, 5, 6, -40]),
        ] {
            let src = format!(
                "enum E {{ Pair(Word, Word) }} loop main(t: E) -> Word {{ let r = yield 5; let n = match t {{ E::Pair(a, b) => a }}; let e = if {condition} {{ r }} else {{ {other} }}; yield 6; yield match e {{ E::Pair(a, b) => a, _ => 0 }} }}"
            );
            flat_inputs(&src, &inputs, &expected);
        }
    }
}

#[test]
fn enum_field_reads_use_the_minimum_extent_without_overcopying() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, -40, 1]
        .into_iter()
        .map(|a| Value::enum_value("E".into(), "A".into(), 0, vec![Value::Int(a)]))
        .collect();
    // Flat enum inspection uses the discriminant, not a boxed nominal name.
    // The two body sizes join, but each copy retains its actual byte count.
    flat_inputs(
        "enum E { A(Word) } enum F { A(Word, Word) } loop main(t: E) -> Word { let r = yield 5; let n = match t { E::A(a) => a }; let e = if n > 0 { r } else { F::A(99, 100) }; yield 6; yield match e { E::A(a) => a, _ => 0 } }",
        &inputs,
        &[5, 6, 7, 5, 6, 99],
    );
}

#[test]
fn mixed_array_extents_use_the_selected_runtime_bound() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, -40, 1]
        .into_iter()
        .map(|a| Value::array(vec![Value::Int(a), Value::Int(a * 10)]))
        .collect();
    flat_inputs(
        "loop main(t: [Word; 2]) -> Word { let r = yield 5; let n = t[0]; let e: [Word; 3] = if n > 0 { r } else { [99, 100, 101] }; let i = if n > 0 { 1 } else { 2 }; yield 6; yield e[i] }",
        &inputs,
        &[5, 6, 70, 5, 6, 101],
    );
}

#[test]
#[cfg(unix)]
fn mixed_array_extents_trap_before_out_of_bounds_access() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState};
    use std::os::unix::process::ExitStatusExt;
    let source = "loop main(t: [Word; 2]) -> Word { let r = yield 5; let n = t[1]; let e: [Word; 3] = if n > 0 { r } else { [99, 100, 101] }; let i = t[0]; yield 6; yield e[i] }";
    let program = common::build(source);
    if let Ok(case) = std::env::var("KEL_COROUTINE_ARRAY_TRAP") {
        let fields: Vec<_> = case.split(',').collect();
        let index: i64 = fields[0].parse().unwrap();
        let host = Guarded::new(16);
        unsafe {
            std::ptr::copy_nonoverlapping([index, 1].as_ptr().cast::<u8>(), host.ptr(), 16);
        }
        let pointer = host.ptr() as i64;
        let mut yielded = 0;
        native_module_read(
            &program,
            &[],
            pointer,
            &[pointer; 3],
            fields[1] == "true",
            fields[2] == "true",
            |value, _, _, _| {
                yielded += 1;
                if yielded == 2 {
                    assert_eq!(value, 6);
                    println!("COROUTINE-ARRAY-RESUMING-INTO-INDEX");
                }
                value
            },
        );
        panic!("native indexing returned where the VM faults");
    }
    for index in [-1, 2] {
        let arena = keleusma_arena::Arena::with_capacity(65536);
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        let input = Value::array(vec![Value::Int(index), Value::Int(1)]);
        assert!(matches!(
            vm.call(std::slice::from_ref(&input)).unwrap(),
            VmState::Yielded(Value::Int(5))
        ));
        assert!(matches!(
            vm.resume(input.clone()).unwrap(),
            VmState::Yielded(Value::Int(6))
        ));
        let error = vm.resume(input).unwrap_err();
        assert!(
            format!("{error:?}").contains("IndexOutOfBounds"),
            "{error:?}"
        );
        for optimized in [false, true] {
            for stable in [false, true] {
                let result = std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "mixed_array_extents_trap_before_out_of_bounds_access",
                        "--nocapture",
                    ])
                    .env(
                        "KEL_COROUTINE_ARRAY_TRAP",
                        format!("{index},{optimized},{stable}"),
                    )
                    .output()
                    .unwrap();
                assert!(
                    String::from_utf8_lossy(&result.stdout)
                        .contains("COROUTINE-ARRAY-RESUMING-INTO-INDEX"),
                    "{result:?}"
                );
                assert_eq!(
                    result.status.signal(),
                    Some(5),
                    "native bounds guard must raise SIGTRAP: {result:?}"
                );
            }
        }
    }
}

#[test]
fn enum_refinement_is_invalidated_when_the_receiver_local_changes() {
    use keleusma::bytecode::{Op, Value};
    use keleusma::vm::{Vm, VmState};
    let source = "enum E { A(Word), B } loop main(t: Word) -> Word { let r = yield 5; let e = if t > 0 { E::A(9) } else { r }; yield match e { E::A(x) => x, _ => 0 } }";
    let mut program = common::build(source);
    let entry = program.entry_point.unwrap();
    let ops = &mut program.chunks[entry].ops;
    let inspection = ops
        .iter()
        .position(|op| matches!(op, Op::IsEnum(..)))
        .unwrap();
    let Op::GetLocal(receiver) = ops[inspection - 1] else {
        panic!("receiver local")
    };
    // The Boolean fact is saved, then the receiver is overwritten before it is
    // tested. Reusing the old fact would turn zero into a payload pointer.
    let insertion = inspection + 3;
    assert!(matches!(ops[insertion - 1], Op::PopN(1)));
    for op in ops.iter_mut() {
        match op {
            Op::If(target)
            | Op::Else(target)
            | Op::Loop(target)
            | Op::EndLoop(target)
            | Op::Break(target)
            | Op::BreakIf(target)
                if *target as usize >= insertion =>
            {
                *target += 2
            }
            _ => {}
        }
    }
    ops.splice(
        insertion..insertion,
        [Op::PushImmediate(0), Op::SetLocal(receiver)],
    );
    keleusma::verify::verify(&program).unwrap();
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(5)]).unwrap(),
        VmState::Yielded(Value::Int(5))
    ));
    assert!(vm.resume(Value::Int(7)).is_err());
    let error = coroutine::lower(&Context::create(), &program, &machine(), 4096).unwrap_err();
    assert!(
        error.to_string().contains("types are not proven"),
        "{error}"
    );
}

#[test]
fn private_body_kind_tracks_the_most_recent_write() {
    use keleusma::bytecode::{NewCompositeOperand, Op, Value};
    use keleusma::value_layout::CompositeKind;
    use keleusma::vm::{Vm, VmState, required_persistent_capacity_for};
    let source = "enum E { A, B } private data st { p: E } loop main(t: Word) -> Word { st.p = if t > 0 { E::A } else { E::A }; yield 5; yield match st.p { E::A => 1, _ => 0 } }";
    let mut program = common::build(source);
    let entry = program.entry_point.unwrap();
    let Op::NewComposite(NewCompositeOperand::Flat { kind, .. }) = program.chunks[entry]
        .ops
        .iter_mut()
        .filter(|op| matches!(op, Op::NewComposite(_)))
        .nth(1)
        .unwrap()
    else {
        panic!("constructor")
    };
    *kind = CompositeKind::Struct;
    keleusma::verify::verify(&program).unwrap();
    let mut arena = keleusma_arena::Arena::with_capacity(65536);
    arena
        .resize_persistent(required_persistent_capacity_for(&program))
        .unwrap();
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    let replies = [7, -5, 7, 3];
    let expected = [5, 1, 5, 0];
    let mut state = vm.call(&[Value::Int(5)]).unwrap();
    for (index, want) in expected.iter().enumerate() {
        assert!(matches!(state, VmState::Yielded(Value::Int(v)) if v == *want));
        if index + 1 < expected.len() {
            state = vm.resume(Value::Int(replies[index])).unwrap();
            if matches!(state, VmState::Reset) {
                state = vm.resume(Value::Int(replies[index])).unwrap();
            }
        }
    }
    for optimize in [false, true] {
        assert_eq!(
            native_module(&program, &[], 5, &replies, optimize).0,
            expected
        );
    }
}

#[test]
fn reset_clears_runtime_kinds_of_later_entry_parameters() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState};
    type StartTwo = unsafe extern "C" fn(i64, i64, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
    let source = "enum E { A, B } loop main(a: Word, e: E) -> Word { yield match e { E::A => 1, _ => 0 }; yield 2 }";
    let program = common::build(source);
    let expected = [1, 2, 0, 2, 0, 2];
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    let mut state = vm
        .call(&[
            Value::Int(5),
            Value::enum_value("E".into(), "A".into(), 0, vec![]),
        ])
        .unwrap();
    for (index, want) in expected.iter().enumerate() {
        assert!(matches!(state, VmState::Yielded(Value::Int(value)) if value == *want));
        if index + 1 < expected.len() {
            state = vm.resume(Value::Int(7)).unwrap();
            if matches!(state, VmState::Reset) {
                state = vm.resume(Value::Int(7)).unwrap();
            }
        }
    }
    for optimized in [false, true] {
        let context = Context::create();
        let module = coroutine::lower(&context, &program, &machine(), 4096).unwrap();
        if optimized {
            common::force_optimize(&module);
        }
        let engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .unwrap();
        let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
        assert_eq!(
            module
                .get_function(&format!("{prefix}_start"))
                .unwrap()
                .count_params(),
            6
        );
        let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
        let unused = Guarded::new(0);
        let body = Guarded::new(8);
        unsafe {
            let start = engine
                .get_function::<StartTwo>(&format!("{prefix}_start"))
                .unwrap();
            let resume = engine
                .get_function::<HandleResume>(&format!("{prefix}_resume"))
                .unwrap();
            let mut result = start.call(
                5,
                body.ptr() as i64,
                unused.ptr(),
                unused.ptr(),
                unused.ptr(),
                slot.ptr(),
            );
            for (index, want) in expected.iter().enumerate() {
                assert_eq!(
                    result,
                    Outcome {
                        live: 1,
                        value: *want
                    }
                );
                if index + 1 < expected.len() {
                    result = resume.call(slot.ptr(), 7);
                }
            }
            engine
                .get_function::<HandleRelease>(&format!("{prefix}_release"))
                .unwrap()
                .call(slot.ptr());
        }
        for region in [&slot, &unused, &body] {
            region.check();
        }
    }
}

#[test]
fn guarded_nested_parameter_use_remains_a_lowering_gap() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, required_persistent_capacity_for};
    let source = "private data st { first: bool = true } loop child(a: Word) -> Word { if st.first { st.first = false; yield a } else { yield 0 } } loop main(t: Word) -> Word { child(t) }";
    let program = common::build(source);
    let mut arena = keleusma_arena::Arena::with_capacity(65536);
    arena
        .resize_persistent(required_persistent_capacity_for(&program))
        .unwrap();
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    let mut state = vm.call(&[Value::Int(5)]).unwrap();
    for want in [5, 0, 0, 0, 0, 0] {
        assert!(matches!(state, VmState::Yielded(Value::Int(value)) if value == want));
        state = vm.resume(Value::Int(7)).unwrap();
        if matches!(state, VmState::Reset) {
            state = vm.resume(Value::Int(7)).unwrap();
        }
    }
    // This is a valid source program. The parameter is read only before Reset
    // clears it. Keep the remaining gap explicit until guarded lowering lands.
    let error = coroutine::lower(&Context::create(), &program, &machine(), 4096).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("non-Unit parameter cleared by Reset"),
        "{error}"
    );
}

fn composite_bodies(src: &str, first: i64, replies: &[i64]) -> Vec<Vec<u8>> {
    use keleusma::bytecode::{ArrayBody, EnumBody, StructBody, TupleBody, Value, WireShape};
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    let p = common::build(src);
    let WireShape::Flat { size, .. } = p.signatures[p.entry_point.unwrap()].ret else {
        panic!("a flat output fixture is required");
    };
    let need = required_persistent_capacity_for(&p);
    let mut arena = keleusma_arena::Arena::with_capacity(
        auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(p.clone(), &arena).unwrap();
    let mut state = vm.call(&[Value::Int(first)]).unwrap();
    let mut expected = Vec::new();
    for (i, &reply) in replies.iter().enumerate() {
        let VmState::Yielded(value) = &state else {
            panic!("expected a yielded body, got {state:?}");
        };
        let body = match value {
            Value::Tuple(TupleBody::Flat(body))
            | Value::Array(ArrayBody::Flat(body))
            | Value::Struct(StructBody::Flat(body))
            | Value::Enum(EnumBody::Flat(body)) => body.resolve(&arena).unwrap(),
            other => panic!("expected flat body, got {other:?}"),
        };
        assert_eq!(body.len(), size as usize);
        expected.push(body.to_vec());
        if i + 1 < replies.len() {
            state = vm.resume(Value::Int(reply)).unwrap();
            if matches!(state, VmState::Reset) {
                state = vm.resume(Value::Int(reply)).unwrap();
            }
        }
    }
    for (optimize, stable) in [(false, false), (false, true), (true, false), (true, true)] {
        let (actual, _) = native_module_read(
            &p,
            &vec![0; p.shared_data_bytes as usize],
            first,
            replies,
            optimize,
            stable,
            |bits, region, private, frame| {
                let address = bits as usize;
                let end = address.checked_add(size as usize).unwrap();
                let storage = [region, private, frame]
                    .into_iter()
                    .find(|storage| {
                        address >= storage.ptr() as usize
                            && end <= storage.ptr() as usize + storage.len
                    })
                    .expect("yielded body must fit a declared instance region");
                let offset = address - storage.ptr() as usize;
                // Read through the checked owning region before the next resume.
                unsafe {
                    std::slice::from_raw_parts(storage.ptr(), storage.len)
                        [offset..offset + size as usize]
                        .to_vec()
                }
            },
        );
        assert_eq!(actual, expected, "{src}, optimized={optimize}");
    }
    expected
}

#[test]
fn flat_yields_preserve_bodies_across_resumes_and_resets() {
    let replies = [11, 20, 31, 40, 55, 0];
    let expected: Vec<Vec<u8>> = [
        (7i64, 8i64),
        (11, 13),
        (20, 21),
        (31, 33),
        (40, 41),
        (55, 57),
    ]
    .into_iter()
    .map(|(a, b)| [a.to_le_bytes(), b.to_le_bytes()].concat())
    .collect();
    for src in [
        "struct P { a: Word, b: Word } loop main(t: Word) -> P { let r = yield P { a: t, b: t + 1 }; yield P { a: r, b: r + 2 } }",
        "loop main(t: Word) -> (Word, Word) { let r: Word = yield (t, t + 1); yield (r, r + 2) }",
        "loop main(t: Word) -> [Word; 2] { let r: Word = yield [t, t + 1]; yield [r, r + 2] }",
    ] {
        assert_eq!(composite_bodies(src, 7, &replies), expected);
    }
}

#[test]
fn delegated_flat_yields_preserve_callee_state_and_entry_replies() {
    let replies = [11, 20, 31, 40, 55, 0];
    for src in [
        "struct P { a: Word, b: Word } yield emit(a: Word) -> P { let r = yield P { a: a, b: a + 1 }; yield P { a: r, b: a }; P { a: a, b: a } } loop main(t: Word) -> P { emit(t); yield P { a: t, b: t + 2 } }",
        "struct P { a: Word, b: Word } loop child() -> P { let r = yield P { a: 7, b: 8 }; yield P { a: r, b: r + 2 } } loop main(t: Word) -> P { child(); yield P { a: t, b: t } }",
        "enum E { A(Word), B(Word) } loop main(t: Word) -> E { let r = yield E::A(t); yield E::B(r) }",
        "struct P { a: Word, b: Word } loop main(t: Word) -> P { let p = P { a: t, b: t + 1 }; yield p; yield p }",
        "struct P { a: Byte, b: Word } struct Q { p: P, c: Word } fn make(t: Word) -> Q { Q { p: P { a: (t as Byte), b: t + 1 }, c: t + 2 } } loop main(t: Word) -> Q { let r: Word = yield make(t); yield make(r) }",
        "struct F { a: Float, b: Byte } loop main(t: Word) -> F { yield F { a: 0.5, b: (t as Byte) }; yield F { a: 1.25, b: (t as Byte) } }",
    ] {
        composite_bodies(src, 7, &replies);
    }
}

#[test]
fn coroutine_composite_boundaries_check_declared_extent() {
    use keleusma::bytecode::WireShape;
    for src in [
        "loop main(t: Word) -> (Word, Word) { yield (t, t); yield (t, t) }",
        "fn pair(t: Word) -> (Word, Word) { (t, t) } loop main(t: Word) -> (Word, Word) { yield pair(t); yield pair(t) }",
    ] {
        let mut p = common::build(src);
        for signature in &mut p.signatures {
            if let WireShape::Flat { size, .. } = &mut signature.ret {
                *size += 8;
            }
        }
        // Verification does not establish native body extents from metadata.
        keleusma::verify::verify(&p).unwrap();
        let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
        assert!(error.to_string().contains("composite boundary"), "{error}");
    }
    let mut p = common::build(
        "fn pair(t: Word) -> (Word, Word) { (t, t) } loop main(t: Word) -> (Word, Word) { yield pair(t); yield pair(t) }",
    );
    let pair = p.chunks.iter_mut().find(|c| c.name == "pair").unwrap();
    assert!(matches!(
        pair.ops.pop(),
        Some(keleusma::bytecode::Op::Return)
    ));
    pair.ops.push(keleusma::bytecode::Op::PopN(1));
    // The current verifier already closes implicit-return paths before lowering.
    assert!(keleusma::verify::verify(&p).is_err());
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(error.to_string().contains("verification:"), "{error}");
}

#[test]
fn composite_operands_survive_direct_and_nested_suspension() {
    let replies = [7, 20, 3, 40, 1, 0];
    for src in [
        "fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } loop main(t: Word) -> Word { yield pick((t, t), yield t) }",
        "fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } yield emit(t: Word) -> Word { pick((t, t), yield t) } loop main(t: Word) -> Word { yield emit(t) }",
        "fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } yield emit(t: Word) -> Word { yield t } yield nested(t: Word) -> Word { pick((t, t), emit(t)) } loop main(t: Word) -> Word { yield nested(t) }",
        "fn pair(t: Word) -> (Word, Word) { (t, t) } fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } yield emit(t: Word) -> Word { yield t } loop main(t: Word) -> Word { yield pick(pair(t), emit(t)) }",
    ] {
        let expected = [5, 12, 20, 23, 40, 41];
        assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
        for optimize in [false, true] {
            assert_eq!(native(src, 5, &replies, optimize), expected, "{src}");
        }
    }
    let src = "fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } loop main(t: Word) -> Word { for i in 0..3 { yield pick((t + i, t), yield t); } yield 0 }";
    let expected = [5, 12, 20, 24, 40, 43];
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    // The common allocation gate still conservatively refuses a body produced
    // inside a loop and held at Yield. Retcon must not bypass that boundary.
    let p = common::build(src);
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(
        matches!(
            error,
            keleusma_native::LowerError::YieldEscapingLoopComposite { .. }
        ),
        "{error}"
    );
}

#[test]
fn coroutine_body_values_preserve_private_storage_aliasing() {
    let replies = [7, 20, 3, 40, 1, 0];
    let expected = [5, 14, 20, 6, 40, 2];
    let src = "private data st { p: (Word, Word) } loop main(t: Word) -> Word { st.p = (t, t); let keep = st.p; let r: Word = yield t; st.p = (r, r); yield keep.0 + st.p.0 }";
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(src, 5, &replies, optimize), expected);
    }
    // Slot kinds are inferred from writes, including the delegated write.
    // Passing an alias before suspension must still observe the later write.
    let src = "private data st { p: (Word, Word) } fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } yield emit(t: Word) -> Word { let r: Word = yield t; st.p = (r, r); r } loop main(t: Word) -> Word { st.p = (t, t); yield pick(st.p, emit(t)) }";
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(src, 5, &replies, optimize), expected);
    }
}

#[test]
fn live_body_coroutines_require_frame_space_and_verified_arguments() {
    use keleusma::bytecode::WireShape;
    let src = "fn pick(p: (Word, Word), a: Word) -> Word { p.0 + p.1 + a } loop main(t: Word) -> Word { yield pick((t, t + 1), yield t) }";
    let mut p = common::build(src);
    let ctx = Context::create();
    let tm = machine();
    let error = coroutine::lower(&ctx, &p, &tm, 8).unwrap_err();
    assert!(error.to_string().contains("frame exceeds"), "{error}");
    let pick = p.chunks.iter().position(|c| c.name == "pick").unwrap();
    let WireShape::Flat { size, .. } = &mut p.signatures[pick].params[0] else {
        panic!("flat argument");
    };
    *size += 8;
    assert!(keleusma::verify::verify(&p).is_err());
    let error = coroutine::lower(&ctx, &p, &tm, 4096).unwrap_err();
    assert!(error.to_string().contains("verification:"), "{error}");
}

#[test]
fn stable_handles_release_independently_and_reuse_the_same_slot() {
    let src = "loop main(a: Word) -> Word { let keep = a + 100; let r = yield a; yield r + keep }";
    assert_eq!(
        common::general_vm_sequence(src, 5, &[11, 20, 31, 0]),
        [5, 116, 20, 151]
    );
    assert_eq!(common::general_vm_sequence(src, 99, &[22, 0]), [99, 221]);
    assert_eq!(common::general_vm_sequence(src, 17, &[2, 0]), [17, 119]);
    assert!(coroutine::slot_bytes(7).is_err());
    assert!(coroutine::slot_bytes(u32::MAX).is_err());
    let program = common::build(src);
    for optimize in [false, true] {
        let ctx = Context::create();
        let tm = machine();
        let module = coroutine::lower(&ctx, &program, &tm, 128).unwrap();
        if optimize {
            module
                .run_passes(
                    "default<O2>",
                    &tm,
                    inkwell::passes::PassBuilderOptions::create(),
                )
                .unwrap();
        }
        let engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .unwrap();
        let name = format!("kel_coroutine_{}", program.entry_point.unwrap());
        let start =
            unsafe { engine.get_function::<HandleStart>(&format!("{name}_start")) }.unwrap();
        let resume =
            unsafe { engine.get_function::<HandleResume>(&format!("{name}_resume")) }.unwrap();
        let release =
            unsafe { engine.get_function::<HandleRelease>(&format!("{name}_release")) }.unwrap();
        let slots = [
            Guarded::new(coroutine::slot_bytes(128).unwrap() as usize),
            Guarded::new(coroutine::slot_bytes(128).unwrap() as usize),
        ];
        let used = slots.each_ref().map(|slot| slot.arena.bottom_used());
        let unused = Guarded::new(0);
        let launch = |i: usize, value| unsafe {
            start.call(
                value,
                unused.ptr(),
                unused.ptr(),
                unused.ptr(),
                slots[i].ptr(),
            )
        };
        assert_eq!(launch(0, 5), Outcome { live: 1, value: 5 });
        assert_eq!(launch(1, 99), Outcome { live: 1, value: 99 });
        assert_eq!(
            unsafe { resume.call(slots[1].ptr(), 22) },
            Outcome {
                live: 1,
                value: 221
            }
        );
        unsafe {
            release.call(slots[1].ptr());
            release.call(slots[1].ptr());
        }
        assert_eq!(
            unsafe { resume.call(slots[1].ptr(), 99) },
            Outcome { live: 0, value: 0 }
        );
        assert_eq!(
            unsafe { resume.call(slots[0].ptr(), 11) },
            Outcome {
                live: 1,
                value: 116
            }
        );
        assert_eq!(launch(1, 17), Outcome { live: 1, value: 17 });
        assert_eq!(
            unsafe { resume.call(slots[0].ptr(), 20) },
            Outcome { live: 1, value: 20 }
        );
        assert_eq!(
            unsafe { resume.call(slots[0].ptr(), 31) },
            Outcome {
                live: 1,
                value: 151
            }
        );
        assert_eq!(
            unsafe { resume.call(slots[1].ptr(), 2) },
            Outcome {
                live: 1,
                value: 119
            }
        );
        for (slot, used) in slots.iter().zip(used) {
            unsafe {
                release.call(slot.ptr());
            }
            assert_eq!(slot.arena.bottom_used(), used);
            slot.check();
        }
        unused.check();
    }
}

#[test]
fn stable_handle_entry_points_link_and_run_from_c() {
    use std::process::Command;
    let program = common::build(
        "loop main(a: Word) -> Word { let keep = a + 100; let r = yield a; yield r + keep }",
    );
    let ctx = Context::create();
    let tm = machine();
    let module = coroutine::lower(&ctx, &program, &tm, 128).unwrap();
    let dir =
        std::path::PathBuf::from("../tmp").join(format!("stable-handle-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let object = dir.join("coroutine.o");
    tm.write_to_file(&module, inkwell::targets::FileType::Object, &object)
        .unwrap();
    let host = dir.join("host.c");
    std::fs::write(
        &host,
        format!(
            r#"
#include <assert.h>
#include <stdint.h>
#include <stdalign.h>
struct outcome {{ uint64_t live; int64_t value; }};
extern struct outcome kel_coroutine_{entry}_start(int64_t, void *, void *, void *, void *);
extern struct outcome kel_coroutine_{entry}_resume(void *, int64_t);
extern void kel_coroutine_{entry}_release(void *);
int main(void) {{
    alignas(8) unsigned char slot[{bytes}];
    struct outcome s = kel_coroutine_{entry}_start(5, 0, 0, 0, slot);
    assert(s.live == 1 && s.value == 5);
    s = kel_coroutine_{entry}_resume(slot, 11);
    assert(s.live == 1 && s.value == 116);
    kel_coroutine_{entry}_release(slot);
    kel_coroutine_{entry}_release(slot);
    s = kel_coroutine_{entry}_resume(slot, 99);
    assert(s.live == 0 && s.value == 0);
    s = kel_coroutine_{entry}_start(17, 0, 0, 0, slot);
    assert(s.live == 1 && s.value == 17);
    s = kel_coroutine_{entry}_resume(slot, 2);
    assert(s.live == 1 && s.value == 119);
    kel_coroutine_{entry}_release(slot);
    return 0;
}}
"#,
            entry = program.entry_point.unwrap(),
            bytes = coroutine::slot_bytes(128).unwrap()
        ),
    )
    .unwrap();
    let executable = dir.join("host");
    let linked = Command::new("cc")
        .arg("-std=c11")
        .arg(&host)
        .arg(&object)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    let run = Command::new(&executable).output().unwrap();
    assert!(
        run.status.success(),
        "{:?}: {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// Reuse one exact-sized host buffer for every incoming body. Older values
/// must survive overwrite, while private-storage references still alias.
fn flat_inputs(src: &str, inputs: &[keleusma::bytecode::Value], expected: &[i64]) {
    flat_inputs_read(
        src,
        inputs,
        expected,
        |value, _| match value {
            keleusma::bytecode::Value::Int(v) => *v,
            other => panic!("expected Word, got {other:?}"),
        },
        |bits, _, _, _| bits,
    );
}

fn flat_inputs_read<T: std::fmt::Debug + PartialEq>(
    src: &str,
    inputs: &[keleusma::bytecode::Value],
    expected: &[T],
    vm_read: impl Fn(&keleusma::bytecode::Value, &keleusma_arena::Arena) -> T,
    read: impl Fn(i64, &Guarded, &Guarded, &Guarded) -> T,
) {
    use keleusma::bytecode::{ArrayBody, EnumBody, StructBody, TupleBody, Value, WireShape};
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    let program = common::build(src);
    let WireShape::Flat { size, .. } = program.signatures[program.entry_point.unwrap()].params[0]
    else {
        panic!("flat input fixture required");
    };
    assert_eq!(inputs.len(), expected.len());
    let need = required_persistent_capacity_for(&program);
    let mut arena = keleusma_arena::Arena::with_capacity(
        auto_arena_capacity_for(&program, &[]).unwrap() + need + 65536,
    );
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    let mut state = vm.call(&inputs[..1]).unwrap();
    for (index, value) in expected.iter().enumerate() {
        let VmState::Yielded(ref actual) = state else {
            panic!("VM step {index}: {state:?}");
        };
        assert_eq!(&vm_read(actual, &arena), value, "VM step {index}");
        if index + 1 < inputs.len() {
            state = vm.resume(inputs[index + 1].clone()).unwrap();
            if matches!(state, VmState::Reset) {
                state = vm.resume(inputs[index + 1].clone()).unwrap();
            }
        }
    }
    let packing = keleusma_arena::Arena::with_capacity(65536);
    let bodies: Vec<_> = inputs
        .iter()
        .map(|value| {
            let flat = value
                .clone()
                .into_arena_canonical(8, std::mem::size_of::<NativeFloat>(), 8, &packing)
                .unwrap();
            let bytes = match &flat {
                Value::Tuple(TupleBody::Flat(body))
                | Value::Array(ArrayBody::Flat(body))
                | Value::Struct(StructBody::Flat(body))
                | Value::Enum(EnumBody::Flat(body)) => body.resolve(&packing).unwrap(),
                other => panic!("not a flat host body: {other:?}"),
            };
            assert_eq!(bytes.len(), size as usize);
            bytes.to_vec()
        })
        .collect();
    for (optimize, stable) in [(false, false), (false, true), (true, false), (true, true)] {
        let host = Guarded::new(size as usize);
        unsafe { std::slice::from_raw_parts_mut(host.ptr(), host.len) }.copy_from_slice(&bodies[0]);
        let pointer = host.ptr() as i64;
        let mut index = 0;
        let (actual, _) = native_module_read(
            &program,
            &vec![0; program.shared_data_bytes as usize],
            pointer,
            &vec![pointer; inputs.len()],
            optimize,
            stable,
            |value, region, private, frame| {
                let output = read(value, region, private, frame);
                index += 1;
                let bytes = unsafe { std::slice::from_raw_parts_mut(host.ptr(), host.len) };
                // Poison even the final body before release, so release cannot
                // depend on a still-valid host input after the last call.
                bytes.fill(0xdd);
                if index < bodies.len() {
                    bytes.copy_from_slice(&bodies[index]);
                }
                host.check();
                output
            },
        );
        assert_eq!(
            actual, expected,
            "optimized={optimize}, stable={stable}: {src}"
        );
        host.check();
    }
}

#[test]
fn flat_host_inputs_survive_reused_buffers_and_ordinary_calls() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, 40, 1, 2, 4]
        .into_iter()
        .map(|a| Value::tuple(vec![Value::Int(a), Value::Int(a * 10)]))
        .collect();
    flat_inputs(
        "fn same(p: (Word, Word)) -> (Word, Word) { p } loop main(t: (Word, Word)) -> Word { let keep = same(t); let r: (Word, Word) = yield t.0; yield keep.0 + r.1 + t.0 }",
        &inputs,
        &[5, 82, 20, 53, 40, 51, 2, 46],
    );
    flat_inputs(
        "fn pick(p: (Word, Word), r: (Word, Word)) -> Word { p.0 + r.1 } loop main(t: (Word, Word)) -> Word { yield pick(t, yield t.0) }",
        &inputs,
        &[5, 75, 20, 50, 40, 50, 2, 42],
    );
    flat_inputs(
        "fn choose(p: (Word, Word)) -> (Word, Word) { if p.0 > 10 { p } else { (99, 100) } } loop main(t: (Word, Word)) -> Word { let keep = choose(t); let r: (Word, Word) = yield t.0; yield keep.0 + r.0 }",
        &inputs,
        &[5, 106, 20, 23, 40, 41, 2, 103],
    );
}

#[test]
fn flat_host_replies_cross_delegated_suspension() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, 40, 1]
        .into_iter()
        .map(|a| Value::tuple(vec![Value::Int(a), Value::Int(a * 10)]))
        .collect();
    flat_inputs(
        "yield emit(p: (Word, Word)) -> Word { let r: (Word, Word) = yield p.0; yield p.0 + r.1; 0 } loop main(t: (Word, Word)) -> Word { emit(t); yield t.0 }",
        &inputs,
        &[5, 75, 20, 3, 403, 1],
    );
}

#[test]
fn flat_host_ownership_preserves_private_aliases_in_the_same_frame() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, 40, 1]
        .into_iter()
        .map(|a| Value::tuple(vec![Value::Int(a), Value::Int(a * 10)]))
        .collect();
    flat_inputs(
        "private data st { p: (Word, Word) } loop main(t: (Word, Word)) -> Word { st.p = t; let alias = st.p; let owned = t; let r = yield t.0; st.p = r; yield alias.0 + owned.0 + st.p.0 }",
        &inputs,
        &[5, 19, 20, 26, 40, 42],
    );
}

#[test]
fn owned_flat_yields_live_inside_the_bounded_instance() {
    use keleusma::bytecode::{TupleBody, Value};
    let inputs: Vec<_> = [5, 7, 20, 3, 40, 1]
        .into_iter()
        .map(|a| Value::tuple(vec![Value::Int(a), Value::Int(a * 10)]))
        .collect();
    let expected: Vec<_> = [5i64, 5, 20, 20, 40, 40]
        .into_iter()
        .map(|a| [a.to_le_bytes(), (a * 10).to_le_bytes()].concat())
        .collect();
    flat_inputs_read(
        "loop main(t: (Word, Word)) -> (Word, Word) { let keep = t; yield t; yield keep }",
        &inputs,
        &expected,
        |value, arena| match value {
            Value::Tuple(TupleBody::Flat(body)) => body.resolve(arena).unwrap().to_vec(),
            other => panic!("expected tuple, got {other:?}"),
        },
        |bits, region, private, frame| {
            let address = bits as usize;
            let end = address.checked_add(16).unwrap();
            let storage = [region, private, frame]
                .into_iter()
                .find(|storage| {
                    address >= storage.ptr() as usize && end <= storage.ptr() as usize + storage.len
                })
                .expect("yielded body must fit a bounded instance reservation");
            let offset = address - storage.ptr() as usize;
            (unsafe { std::slice::from_raw_parts(storage.ptr(), storage.len) })[offset..offset + 16]
                .to_vec()
        },
    );
}

#[test]
fn nested_flat_host_inputs_preserve_packed_fields_and_array_views() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, 40, 1]
        .into_iter()
        .map(|a| {
            Value::array(vec![
                Value::tuple(vec![
                    Value::Byte(2),
                    Value::Float(a as f64),
                    Value::Int(a * 10),
                ]),
                Value::tuple(vec![Value::Byte(3), Value::Float(1.0), Value::Int(a)]),
            ])
        })
        .collect();
    flat_inputs(
        "loop main(t: [(Byte, Float, Word); 2]) -> Word { let keep = t[0]; let r: [(Byte, Float, Word); 2] = yield t[1].2; yield (keep.1 as Word) + (r[0].0 as Word) + r[0].2 }",
        &inputs,
        &[5, 77, 20, 52, 40, 52],
    );
}

#[test]
fn host_struct_and_enum_bodies_survive_later_replies() {
    use keleusma::bytecode::Value;
    let structs: Vec<_> = [5, 7, 20, 3, 40, 1]
        .into_iter()
        .map(|a| {
            Value::struct_value(
                "P".into(),
                vec![
                    ("a".into(), Value::Int(a)),
                    ("b".into(), Value::Int(a * 10)),
                ],
            )
        })
        .collect();
    flat_inputs(
        "struct P { a: Word, b: Word } loop main(t: P) -> Word { let keep = t; let r: P = yield t.a; yield keep.a + r.b }",
        &structs,
        &[5, 75, 20, 50, 40, 50],
    );
    let enums: Vec<_> = [5, 7, 20, 3, 40, 1]
        .into_iter()
        .map(|a| {
            Value::enum_value(
                "E".into(),
                "Pair".into(),
                0,
                vec![Value::Int(a), Value::Int(a * 10)],
            )
        })
        .collect();
    flat_inputs(
        "enum E { Pair(Word, Word) } fn first(e: E) -> Word { match e { E::Pair(a, b) => a } } fn last(e: E) -> Word { match e { E::Pair(a, b) => b } } loop main(t: E) -> Word { let keep = t; let r: E = yield first(t); yield first(keep) + last(r) }",
        &enums,
        &[5, 75, 20, 50, 40, 50],
    );
}

#[test]
fn flat_host_values_survive_repeated_loop_suspension() {
    use keleusma::bytecode::Value;
    let inputs: Vec<_> = [5, 7, 20, 3, 40, 1, 2, 4]
        .into_iter()
        .map(|a| Value::tuple(vec![Value::Int(a), Value::Int(a * 10)]))
        .collect();
    flat_inputs(
        "loop main(t: (Word, Word)) -> Word { let keep = t; for i in 0..3 { let r: (Word, Word) = yield keep.0 + i; yield r.1 + keep.0; } yield t.0 }",
        &inputs,
        &[5, 75, 6, 35, 7, 15, 2, 4],
    );
}

#[test]
fn flat_host_copies_require_bounded_signatures_and_frame_space() {
    use keleusma::bytecode::WireShape;
    let mut p = common::build(
        "loop main(t: (Word, Word)) -> Word { let keep = t; yield t.0; yield keep.1 }",
    );
    let error = coroutine::lower(&Context::create(), &p, &machine(), 8).unwrap_err();
    assert!(error.to_string().contains("frame exceeds"), "{error}");
    p.signatures[p.entry_point.unwrap()].params[0] = WireShape::Top;
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(
        error.to_string().contains("flat composite input signature"),
        "{error}"
    );

    let mut native = common::build(
        "use host::identity\nloop main(t: (Word, Word)) -> Word { let keep: (Word, Word) = host::identity(t); yield keep.0; yield t.0 }",
    );
    native.native_return_shapes[0] = native.signatures[native.entry_point.unwrap()].params[0];
    keleusma::verify::verify(&native).unwrap();
    let error = coroutine::lower(&Context::create(), &native, &machine(), 4096).unwrap_err();
    assert!(error.to_string().contains("ownership contract"), "{error}");
}

#[test]
fn native_body_lifetimes_require_a_contract_even_with_scalar_host_values() {
    use keleusma::bytecode::WireShape;
    let mut program = common::build(
        "use host::pair\nloop main(t: Word) -> Word { let p: (Word, Word) = host::pair(t); yield p.0; yield p.1 }",
    );
    program.native_return_shapes[0] = WireShape::Flat { kind: 0, size: 16 };
    keleusma::verify::verify(&program).unwrap();
    let error = coroutine::lower(&Context::create(), &program, &machine(), 4096).unwrap_err();
    assert!(error.to_string().contains("ownership contract"), "{error}");
}

#[test]
fn repeated_flat_local_assignments_keep_their_proven_extent() {
    use keleusma::bytecode::{TupleBody, Value};
    use keleusma::vm::{Vm, VmState};
    // Both writes have the same extent. The saved value must remain independent
    // of the host input buffer after that buffer is reused for a reply.
    let mut p = common::build(
        "loop main(t: (Word, Word)) -> Word { let keep = t; let replacement = t; yield 0; yield keep.0 }",
    );
    let entry = p.entry_point.unwrap();
    let writes: Vec<_> = p.chunks[entry]
        .ops
        .iter()
        .enumerate()
        .filter_map(|(i, op)| {
            if let keleusma::bytecode::Op::SetLocal(slot) = op {
                Some((i, *slot))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(writes.len(), 2);
    p.chunks[entry].ops[writes[1].0] = keleusma::bytecode::Op::SetLocal(writes[0].1);
    keleusma::verify::verify(&p).unwrap();
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = Vm::new(p.clone(), &arena).unwrap();
    let pair = |a, b| {
        Value::Tuple(TupleBody::Boxed(Box::new(vec![
            Value::Int(a),
            Value::Int(b),
        ])))
    };
    assert!(matches!(
        vm.call(&[pair(5, 6)]).unwrap(),
        VmState::Yielded(Value::Int(0))
    ));
    assert!(matches!(
        vm.resume(pair(7, 8)).unwrap(),
        VmState::Yielded(Value::Int(5))
    ));
    for optimized in [false, true] {
        for stable in [false, true] {
            let host = Guarded::new(16);
            unsafe {
                std::ptr::copy_nonoverlapping([5i64, 6].as_ptr().cast::<u8>(), host.ptr(), 16);
            }
            let pointer = host.ptr() as i64;
            let (observed, _) = native_module_read(
                &p,
                &vec![0; p.shared_data_bytes as usize],
                pointer,
                &[pointer, pointer],
                optimized,
                stable,
                |bits, _, _, _| {
                    unsafe {
                        std::ptr::copy_nonoverlapping(
                            [7i64, 8].as_ptr().cast::<u8>(),
                            host.ptr(),
                            16,
                        );
                    }
                    bits
                },
            );
            assert_eq!(observed, [0, 5]);
            host.check();
        }
    }
}

#[test]
fn reentrant_entries_report_completion_once_and_reuse_the_slot() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState};
    let cases = [
        (
            "yield main(a: Word) -> Word { let keep = a + 100; let b = yield a; let c = yield keep + b; keep + c + a }",
            vec![7, 11],
            vec![(1, 5), (1, 112), (2, 121)],
        ),
        (
            "yield main(a: Word) -> Word { if a > 0 { a + 3 } else { yield a } }",
            vec![],
            vec![(2, 8)],
        ),
        (
            "yield helper(a: Word) -> Word { let r = yield a + 1; r + a } yield main(a: Word) -> Word { let v = helper(a + 2); v + a }",
            vec![13],
            vec![(1, 8), (2, 25)],
        ),
    ];
    for (source, replies, expected) in cases {
        let program = common::build(source);
        let arena = keleusma_arena::Arena::with_capacity(65536);
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        let mut state = vm.call(&[Value::Int(5)]).unwrap();
        let mut observed = Vec::new();
        for reply in replies.iter().copied().map(Some).chain([None]) {
            match state {
                VmState::Yielded(Value::Int(value)) => observed.push((1, value)),
                VmState::Finished(Value::Int(value)) => observed.push((2, value)),
                other => panic!("unexpected VM outcome {other:?}"),
            }
            if let Some(reply) = reply {
                state = vm.resume(Value::Int(reply)).unwrap();
            } else {
                break;
            }
        }
        assert_eq!(observed, expected, "independent completion oracle");
        for optimized in [false, true] {
            let context = Context::create();
            let module = coroutine::lower(&context, &program, &machine(), 4096).unwrap();
            if optimized {
                common::force_optimize(&module);
            }
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
            let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
            let shared = Guarded::new(program.shared_data_bytes as usize);
            let private = Guarded::new(
                keleusma::vm::required_persistent_capacity_for(&program)
                    + region::persistent_supplement_bytes(&program) as usize,
            );
            let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
            unsafe {
                let start = engine
                    .get_function::<HandleStart>(&format!("{prefix}_start"))
                    .unwrap();
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                let release = engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap();
                for _ in 0..2 {
                    let initial =
                        start.call(5, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr());
                    let mut observed = vec![(initial.live, initial.value)];
                    for &reply in &replies {
                        let next = resume.call(slot.ptr(), reply);
                        observed.push((next.live, next.value));
                    }
                    assert_eq!(observed, expected, "optimized={optimized}");
                    assert_eq!(resume.call(slot.ptr(), 99), Outcome { live: 0, value: 0 });
                    release.call(slot.ptr());
                    release.call(slot.ptr());
                }
            }
            for reservation in [&slot, &shared, &private, &bodies] {
                reservation.check();
            }
        }
    }
}

#[test]
fn completed_host_bodies_outlive_the_continuation_frame() {
    use keleusma::bytecode::{TupleBody, Value};
    use keleusma::vm::{Vm, VmState};
    type StartTwo = unsafe extern "C" fn(i64, i64, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
    for multiple in [false, true] {
        let program = if multiple {
            common::build(
                "yield main(n: Word, a: (Word, Word)) -> (Word, Word) { let old = a; yield a; old }",
            )
        } else {
            common::build(
                "yield main(a: (Word, Word)) -> (Word, Word) { let old = a; let reply: (Word, Word) = yield a; old }",
            )
        };
        let arena = keleusma_arena::Arena::with_capacity(65536);
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        let tuple = |a, b| {
            Value::Tuple(TupleBody::Boxed(Box::new(vec![
                Value::Int(a),
                Value::Int(b),
            ])))
        };
        let read_vm = |value: Value| {
            let Value::Tuple(TupleBody::Flat(body)) = value else {
                panic!("expected flat tuple")
            };
            let bytes = body.resolve(&arena).unwrap();
            [
                i64::from_le_bytes(bytes[..8].try_into().unwrap()),
                i64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            ]
        };
        let arguments = if multiple {
            vec![Value::Int(9), tuple(5, 6)]
        } else {
            vec![tuple(5, 6)]
        };
        let VmState::Yielded(value) = vm.call(&arguments).unwrap() else {
            panic!("expected yield")
        };
        assert_eq!(read_vm(value), [5, 6]);
        let VmState::Finished(value) = vm
            .resume(if multiple {
                Value::Int(21)
            } else {
                tuple(21, 22)
            })
            .unwrap()
        else {
            panic!("expected completion")
        };
        assert_eq!(read_vm(value), [5, 6]);
        for (optimized, explicit) in [(false, false), (true, false), (false, true), (true, true)] {
            let context = Context::create();
            let module = if explicit {
                let entry = program.entry_point.unwrap();
                let contracts = program
                    .chunks
                    .iter()
                    .enumerate()
                    .flat_map(|(ci, chunk)| {
                        let signature = &program.signatures[entry];
                        chunk.ops.iter().enumerate().filter_map(move |(ip, op)| {
                            matches!(op, keleusma::bytecode::Op::Yield).then_some((
                                coroutine::YieldSite {
                                    chunk: ci as u32,
                                    instruction: ip as u32,
                                },
                                coroutine::Dialogue {
                                    yielded: signature.ret,
                                    reply: signature.params[0],
                                },
                            ))
                        })
                    })
                    .collect();
                coroutine::lower_with_dialogues(&context, &program, &machine(), 4096, &contracts)
                    .unwrap()
            } else {
                coroutine::lower(&context, &program, &machine(), 4096).unwrap()
            };
            let continuation_bytes = 4096 - 16 - if explicit { 8 } else { 0 };
            if optimized {
                common::force_optimize(&module);
            }
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
            let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
            let shared = Guarded::new(program.shared_data_bytes as usize);
            let private = Guarded::new(
                keleusma::vm::required_persistent_capacity_for(&program)
                    + region::persistent_supplement_bytes(&program) as usize,
            );
            let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
            let input = Guarded::new(16);
            unsafe {
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                let release = engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap();
                for _ in 0..2 {
                    let source = std::slice::from_raw_parts_mut(input.ptr(), 16);
                    source[..8].copy_from_slice(&5_i64.to_le_bytes());
                    source[8..].copy_from_slice(&6_i64.to_le_bytes());
                    let yielded = if multiple {
                        engine
                            .get_function::<StartTwo>(&format!("{prefix}_start"))
                            .unwrap()
                            .call(
                                9,
                                input.ptr() as i64,
                                shared.ptr(),
                                private.ptr(),
                                bodies.ptr(),
                                slot.ptr(),
                            )
                    } else {
                        engine
                            .get_function::<HandleStart>(&format!("{prefix}_start"))
                            .unwrap()
                            .call(
                                input.ptr() as i64,
                                shared.ptr(),
                                private.ptr(),
                                bodies.ptr(),
                                slot.ptr(),
                            )
                    };
                    assert_eq!(yielded.live, 1);
                    source[..8].copy_from_slice(&21_i64.to_le_bytes());
                    source[8..].copy_from_slice(&22_i64.to_le_bytes());
                    let finished =
                        resume.call(slot.ptr(), if multiple { 21 } else { input.ptr() as i64 });
                    assert_eq!(finished.live, 2);
                    // The result must survive both host input reuse and destruction
                    // of every byte LLVM could use for the continuation frame.
                    source.fill(0xdd);
                    let result = finished.value as *const u8;
                    assert_eq!(result, slot.ptr().add(16 + continuation_bytes));
                    std::slice::from_raw_parts_mut(slot.ptr().add(16), continuation_bytes)
                        .fill(0xaa);
                    let bytes = std::slice::from_raw_parts(result, 16);
                    assert_eq!(i64::from_le_bytes(bytes[..8].try_into().unwrap()), 5);
                    assert_eq!(i64::from_le_bytes(bytes[8..].try_into().unwrap()), 6);
                    assert_eq!(resume.call(slot.ptr(), 0), Outcome { live: 0, value: 0 });
                    release.call(slot.ptr());
                }
            }
            for reservation in [&slot, &shared, &private, &bodies, &input] {
                reservation.check();
            }
        }
        let error = coroutine::lower(&Context::create(), &program, &machine(), 16).unwrap_err();
        assert!(
            error.to_string().contains("completion reservation"),
            "{error}"
        );
    }
}

#[test]
fn zero_and_multiple_entry_arguments_preserve_host_contracts() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState};
    type StartZero = unsafe extern "C" fn(*mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
    type StartTwo = unsafe extern "C" fn(i64, i64, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
    let cases = [
        (
            "yield main() -> Word { yield 42; 7 }",
            vec![],
            Value::Unit,
            [(1, 42), (2, 7)],
        ),
        (
            "loop main() -> Word { yield 42; yield 43 }",
            vec![],
            Value::Unit,
            [(1, 42), (1, 43)],
        ),
        (
            "yield main(a: Word, b: Word) -> Word { let r = yield a + b; a + b + r }",
            vec![Value::Int(5), Value::Int(7)],
            Value::Int(11),
            [(1, 12), (2, 23)],
        ),
        (
            "loop main(a: Word, b: Word) -> Word { yield a; yield a }",
            vec![Value::Int(5), Value::Int(7)],
            Value::Int(11),
            [(1, 5), (1, 11)],
        ),
    ];
    for (source, arguments, reply, expected) in cases {
        let program = common::build(source);
        let arena = keleusma_arena::Arena::with_capacity(65536);
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        let first = vm.call(&arguments).unwrap();
        let second = vm.resume(reply.clone()).unwrap();
        let observe = |state| match state {
            VmState::Yielded(Value::Int(v)) => (1, v),
            VmState::Finished(Value::Int(v)) => (2, v),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!([observe(first), observe(second)], expected);
        for optimized in [false, true] {
            let context = Context::create();
            let module = coroutine::lower(&context, &program, &machine(), 4096).unwrap();
            if optimized {
                common::force_optimize(&module);
            }
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
            let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
            let shared = Guarded::new(program.shared_data_bytes as usize);
            let private = Guarded::new(
                keleusma::vm::required_persistent_capacity_for(&program)
                    + region::persistent_supplement_bytes(&program) as usize,
            );
            let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
            unsafe {
                let first = if arguments.is_empty() {
                    engine
                        .get_function::<StartZero>(&format!("{prefix}_start"))
                        .unwrap()
                        .call(shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr())
                } else {
                    engine
                        .get_function::<StartTwo>(&format!("{prefix}_start"))
                        .unwrap()
                        .call(5, 7, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr())
                };
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                let bits = if arguments.is_empty() { 0 } else { 11 };
                let second = resume.call(slot.ptr(), bits);
                assert_eq!(
                    [(first.live, first.value), (second.live, second.value)],
                    expected,
                    "{source}, optimized={optimized}"
                );
                if arguments.is_empty() && second.live == 1 {
                    assert_eq!(resume.call(slot.ptr(), 0), Outcome { live: 1, value: 42 });
                }
                engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap()
                    .call(slot.ptr());
            }
            for reservation in [&slot, &shared, &private, &bodies] {
                reservation.check();
            }
        }
    }
}

#[test]
fn stream_reset_does_not_replenish_later_parameters() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState};
    let program = common::build("loop main(a: Word, b: Word) -> Word { yield a + b; yield a + b }");
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(5), Value::Int(7)]).unwrap(),
        VmState::Yielded(Value::Int(12))
    ));
    assert!(matches!(
        vm.resume(Value::Int(11)).unwrap(),
        VmState::Yielded(Value::Int(18))
    ));
    assert!(matches!(vm.resume(Value::Int(13)).unwrap(), VmState::Reset));
    assert!(
        vm.resume(Value::Int(13)).is_err(),
        "Reset cleared parameter b to Unit"
    );
    let error = coroutine::lower(&Context::create(), &program, &machine(), 4096).unwrap_err();
    assert!(
        error.to_string().contains("scalar types are not proven"),
        "{error}"
    );
}

#[test]
fn implicit_reentrant_completion_cannot_bypass_body_extent_checks() {
    use keleusma::bytecode::{Op, WireShape};
    let mut program =
        common::build("yield main(a: (Word, Word)) -> (Word, Word) { if false { yield a; } a }");
    let entry = program.entry_point.unwrap();
    assert!(matches!(program.chunks[entry].ops.pop(), Some(Op::Return)));
    let WireShape::Flat { size, .. } = &mut program.signatures[entry].ret else {
        panic!("flat fixture required")
    };
    *size += 8;
    let verified = keleusma::verify::verify(&program);
    let error = coroutine::lower(&Context::create(), &program, &machine(), 4096).unwrap_err();
    assert!(verified.is_err());
    assert!(error.to_string().contains("verification:"), "{error}");
}

#[test]
fn per_site_dialogues_distinguish_yields_replies_and_completion() {
    use coroutine::{Dialogue, YieldSite};
    use keleusma::bytecode::{Op, Value, WireShape};
    use keleusma::vm::{Vm, VmState};
    use std::collections::BTreeMap;
    type Site = unsafe extern "C" fn(*mut u8) -> u64;
    let program = common::build(
        "yield helper(a: Word) -> Word { let r: Word = yield true; a + r } yield main(a: Word) -> Word { let b: Byte = yield a; let n = helper(a + (b as Word)); let d: Float = yield (n as Byte); n + (d as Word) }",
    );
    let mut contracts = BTreeMap::new();
    let mut main_sites = Vec::new();
    let mut helper_site = None;
    for (ci, chunk) in program.chunks.iter().enumerate() {
        for (ip, op) in chunk.ops.iter().enumerate() {
            if !matches!(op, Op::Yield) {
                continue;
            }
            let site = YieldSite {
                chunk: ci as u32,
                instruction: ip as u32,
            };
            let (yielded, reply) = if chunk.name == "helper" {
                helper_site = Some(site);
                (1, 3)
            } else {
                let first = main_sites.is_empty();
                main_sites.push(site);
                if first { (3, 2) } else { (2, 5) }
            };
            contracts.insert(
                site,
                Dialogue {
                    yielded: WireShape::Scalar { kind: yielded },
                    reply: WireShape::Scalar { kind: reply },
                },
            );
        }
    }
    let sites = [main_sites[0], helper_site.unwrap(), main_sites[1]];
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(5)]).unwrap(),
        VmState::Yielded(Value::Int(5))
    ));
    assert!(matches!(
        vm.resume(Value::Byte(2)).unwrap(),
        VmState::Yielded(Value::Bool(true))
    ));
    assert!(matches!(
        vm.resume(Value::Int(7)).unwrap(),
        VmState::Yielded(Value::Byte(14))
    ));
    assert!(matches!(
        vm.resume(Value::Float(3.0)).unwrap(),
        VmState::Finished(Value::Int(17))
    ));
    for optimized in [false, true] {
        let context = Context::create();
        let module =
            coroutine::lower_with_dialogues(&context, &program, &machine(), 4096, &contracts)
                .unwrap();
        if optimized {
            common::force_optimize(&module);
        }
        let engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .unwrap();
        let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
        let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
        let shared = Guarded::new(program.shared_data_bytes as usize);
        let private = Guarded::new(
            keleusma::vm::required_persistent_capacity_for(&program)
                + region::persistent_supplement_bytes(&program) as usize,
        );
        let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
        unsafe {
            let start = engine
                .get_function::<HandleStart>(&format!("{prefix}_start"))
                .unwrap();
            let resume = engine
                .get_function::<HandleResume>(&format!("{prefix}_resume"))
                .unwrap();
            let site = engine
                .get_function::<Site>(&format!("{prefix}_yield_site"))
                .unwrap();
            let first = start.call(5, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr());
            assert_eq!(first, Outcome { live: 1, value: 5 });
            assert_eq!(site.call(slot.ptr()), sites[0].id());
            assert_eq!(resume.call(slot.ptr(), 2), Outcome { live: 1, value: 1 });
            assert_eq!(site.call(slot.ptr()), sites[1].id());
            assert_eq!(resume.call(slot.ptr(), 7), Outcome { live: 1, value: 14 });
            assert_eq!(site.call(slot.ptr()), sites[2].id());
            assert_eq!(
                resume.call(slot.ptr(), NativeFloat::to_bits(3.0) as i64),
                Outcome { live: 2, value: 17 }
            );
            assert_eq!(site.call(slot.ptr()), u64::MAX);
            engine
                .get_function::<HandleRelease>(&format!("{prefix}_release"))
                .unwrap()
                .call(slot.ptr());
        }
        for reservation in [&slot, &shared, &private, &bodies] {
            reservation.check();
        }
    }
    let mut incomplete = contracts.clone();
    incomplete.remove(&sites[0]);
    assert!(
        coroutine::lower_with_dialogues(
            &Context::create(),
            &program,
            &machine(),
            4096,
            &incomplete
        )
        .unwrap_err()
        .to_string()
        .contains("missing dialogue")
    );
    let mut wrong = contracts.clone();
    wrong.get_mut(&sites[1]).unwrap().yielded = WireShape::Scalar { kind: 2 };
    assert!(
        coroutine::lower_with_dialogues(&Context::create(), &program, &machine(), 4096, &wrong)
            .unwrap_err()
            .to_string()
            .contains("scalar types are not proven")
    );
    let mut extra = contracts.clone();
    extra.insert(
        YieldSite {
            chunk: 0,
            instruction: u32::MAX,
        },
        contracts[&sites[0]],
    );
    assert!(
        coroutine::lower_with_dialogues(&Context::create(), &program, &machine(), 4096, &extra)
            .unwrap_err()
            .to_string()
            .contains("non-yield site")
    );
}

fn reassign_yield_reply_to_first_local(program: &mut keleusma::bytecode::Module) {
    use keleusma::bytecode::Op;
    let entry = program.entry_point.unwrap();
    let ops = &mut program.chunks[entry].ops;
    let destination = ops
        .iter()
        .find_map(|op| match op {
            Op::SetLocal(slot) => Some(*slot),
            _ => None,
        })
        .unwrap();
    let reply = ops.iter().position(|op| matches!(op, Op::Yield)).unwrap() + 1;
    assert!(matches!(ops[reply], Op::SetLocal(_)));
    ops[reply] = Op::SetLocal(destination);
    keleusma::verify::verify(program).unwrap();
}

#[test]
fn explicit_flat_replies_survive_reuse_and_delegated_entry_completion() {
    use coroutine::{Dialogue, YieldSite};
    use keleusma::bytecode::{Op, TupleBody, Value, WireShape};
    use keleusma::vm::{Vm, VmState};
    use std::collections::BTreeMap;
    let pair = WireShape::Flat { kind: 0, size: 16 };
    let triple = WireShape::Flat { kind: 0, size: 24 };
    let cases = [
        (
            "yield main(a: Word) -> Word { let keep = (a, a); for i in 0..2 { let replacement = yield keep.1; }; keep.0 }",
            vec![(WireShape::Scalar { kind: 3 }, pair)],
            vec![vec![5], vec![8]],
            vec![vec![7, 8], vec![9, 10]],
            9,
            true,
        ),
        (
            "yield main(a: Word) -> Word { let r: (Word, Word) = yield (a, a + 1); let s: (Word, Word, Word) = yield r; r.0 + s.2 + a }",
            vec![(pair, pair), (pair, triple)],
            vec![vec![5, 6], vec![11, 12]],
            vec![vec![11, 12], vec![31, 32, 33]],
            49,
            false,
        ),
        (
            "yield helper(a: Word) -> Word { let p: (Word, Word) = yield true; p.0 + p.1 + a } yield main(a: Word) -> Word { helper(a) }",
            vec![(WireShape::Scalar { kind: 1 }, pair)],
            vec![vec![1]],
            vec![vec![11, 12]],
            28,
            false,
        ),
    ];
    for (source, shapes, outputs, replies, expected, reassign) in cases {
        let mut program = common::build(source);
        if reassign {
            reassign_yield_reply_to_first_local(&mut program);
        }
        let mut contracts = BTreeMap::new();
        let mut next = shapes.iter();
        for (ci, chunk) in program.chunks.iter().enumerate() {
            for (ip, op) in chunk.ops.iter().enumerate() {
                if matches!(op, Op::Yield) {
                    let &(yielded, reply) = next.next().unwrap();
                    contracts.insert(
                        YieldSite {
                            chunk: ci as u32,
                            instruction: ip as u32,
                        },
                        Dialogue { yielded, reply },
                    );
                }
            }
        }
        assert!(next.next().is_none());
        let arena = keleusma_arena::Arena::with_capacity(65536);
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        let mut state = vm.call(&[Value::Int(5)]).unwrap();
        for (output, reply) in outputs.iter().zip(&replies) {
            let VmState::Yielded(value) = state else {
                panic!("expected yield")
            };
            let actual: Vec<i64> = match value {
                Value::Bool(value) => vec![i64::from(value)],
                Value::Int(value) => vec![value],
                Value::Tuple(TupleBody::Flat(body)) => body
                    .resolve(&arena)
                    .unwrap()
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|bytes| i64::from_le_bytes(*bytes))
                    .collect(),
                other => panic!("unexpected {other:?}"),
            };
            assert_eq!(&actual, output);
            state = vm
                .resume(Value::Tuple(TupleBody::Boxed(Box::new(
                    reply.iter().copied().map(Value::Int).collect(),
                ))))
                .unwrap();
        }
        assert!(matches!(state, VmState::Finished(Value::Int(value)) if value == expected));
        for optimized in [false, true] {
            let context = Context::create();
            // Odd reservations also test alignment of the metadata tail.
            let module =
                coroutine::lower_with_dialogues(&context, &program, &machine(), 4099, &contracts)
                    .unwrap();
            if optimized {
                common::force_optimize(&module);
            }
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
            let slot = Guarded::new(coroutine::slot_bytes(4099).unwrap() as usize);
            let shared = Guarded::new(program.shared_data_bytes as usize);
            let private = Guarded::new(
                keleusma::vm::required_persistent_capacity_for(&program)
                    + region::persistent_supplement_bytes(&program) as usize,
            );
            let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
            let host = Guarded::new(24);
            unsafe {
                let start = engine
                    .get_function::<HandleStart>(&format!("{prefix}_start"))
                    .unwrap();
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                let release = engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap();
                let mut state =
                    start.call(5, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr());
                for ((output, reply), &(yielded, _)) in
                    outputs.iter().zip(&replies).zip(shapes.iter().cycle())
                {
                    assert_eq!(state.live, 1);
                    let actual: Vec<i64> = if let WireShape::Flat { size, .. } = yielded {
                        std::slice::from_raw_parts(state.value as *const u8, size as usize)
                            .as_chunks::<8>()
                            .0
                            .iter()
                            .map(|bytes| i64::from_le_bytes(*bytes))
                            .collect()
                    } else {
                        vec![state.value]
                    };
                    assert_eq!(&actual, output);
                    let bytes = std::slice::from_raw_parts_mut(host.ptr(), host.len);
                    bytes.fill(0xdd);
                    for (destination, value) in bytes.as_chunks_mut::<8>().0.iter_mut().zip(reply) {
                        destination.copy_from_slice(&value.to_le_bytes());
                    }
                    state = resume.call(slot.ptr(), host.ptr() as i64);
                }
                assert_eq!(
                    state,
                    Outcome {
                        live: 2,
                        value: expected
                    }
                );
                release.call(slot.ptr());
            }
            for reservation in [&slot, &shared, &private, &bodies, &host] {
                reservation.check();
            }
        }
    }
}

#[test]
fn dialogue_body_extents_cover_reads_and_callee_arguments() {
    use coroutine::{Dialogue, YieldSite};
    use keleusma::bytecode::{Op, WireShape};
    use std::collections::BTreeMap;
    for (source, kind, size, reassign) in [
        (
            "yield main(a: Word) -> Word { let keep = (a, a); for i in 0..2 { let replacement = yield keep.1; }; keep.0 }",
            0,
            8,
            true,
        ),
        (
            "yield main(a: Word) -> Word { let p: (Word, Word) = yield a; p.1 }",
            0,
            8,
            false,
        ),
        (
            "fn last(p: (Word, Word)) -> Word { p.1 } yield main(a: Word) -> Word { let p: (Word, Word) = yield a; last(p) }",
            0,
            8,
            false,
        ),
        (
            "struct W { pair: (Word, Word) } yield main(a: Word) -> Word { let p: W = yield a; p.pair.1 }",
            2,
            8,
            false,
        ),
        (
            "enum E { Pair(Word, Word) } yield main(a: Word) -> Word { let p: E = yield a; match p { E::Pair(x, y) => x + y } }",
            3,
            4,
            false,
        ),
    ] {
        let mut program = common::build(source);
        if reassign {
            reassign_yield_reply_to_first_local(&mut program);
        }
        let mut contracts = BTreeMap::new();
        for (ci, chunk) in program.chunks.iter().enumerate() {
            for (ip, op) in chunk.ops.iter().enumerate() {
                if matches!(op, Op::Yield) {
                    contracts.insert(
                        YieldSite {
                            chunk: ci as u32,
                            instruction: ip as u32,
                        },
                        Dialogue {
                            yielded: WireShape::Scalar { kind: 3 },
                            reply: WireShape::Flat { kind, size },
                        },
                    );
                }
            }
        }
        let error = coroutine::lower_with_dialogues(
            &Context::create(),
            &program,
            &machine(),
            4096,
            &contracts,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("body extent"),
            "{source}: {error}"
        );
    }
    // A join must not adopt the last emitted branch's larger body extent.
    let program = common::build(
        "yield main(a: Word) -> Word { let p: (Word, Word) = if a > 0 { yield 1 } else { yield 2 }; p.1 }",
    );
    let entry = program.entry_point.unwrap();
    for sizes in [[8, 16], [16, 8]] {
        let contracts = program.chunks[entry]
            .ops
            .iter()
            .enumerate()
            .filter(|(_, op)| matches!(op, Op::Yield))
            .enumerate()
            .map(|(ordinal, (ip, _))| {
                (
                    YieldSite {
                        chunk: entry as u32,
                        instruction: ip as u32,
                    },
                    Dialogue {
                        yielded: WireShape::Scalar { kind: 3 },
                        reply: WireShape::Flat {
                            kind: 0,
                            size: sizes[ordinal],
                        },
                    },
                )
            })
            .collect();
        let error = coroutine::lower_with_dialogues(
            &Context::create(),
            &program,
            &machine(),
            4096,
            &contracts,
        )
        .unwrap_err();
        assert!(error.to_string().contains("body extent"), "{error}");
    }
}

#[test]
fn parameterless_dialogues_complete_with_each_scalar_kind() {
    use coroutine::{Dialogue, YieldSite};
    use keleusma::bytecode::{Op, Value, WireShape};
    use keleusma::vm::{Vm, VmState};
    use std::collections::BTreeMap;
    type StartZero = unsafe extern "C" fn(*mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
    for (declaration, kind, value, bits) in [
        ("()", 0, Value::Unit, 0),
        ("bool", 1, Value::Bool(true), 1),
        ("Byte", 2, Value::Byte(9), 9),
        ("Word", 3, Value::Int(-11), -11),
        ("Fixed", 4, Value::Fixed(65536), 65536),
        (
            "Float",
            5,
            Value::Float(7.25),
            NativeFloat::to_bits(7.25) as i64,
        ),
    ] {
        let program = common::build(&format!(
            "yield main() -> {declaration} {{ let r: {declaration} = yield false; r }}"
        ));
        let entry = program.entry_point.unwrap();
        let ip = program.chunks[entry]
            .ops
            .iter()
            .position(|op| matches!(op, Op::Yield))
            .unwrap();
        let contracts = BTreeMap::from([(
            YieldSite {
                chunk: entry as u32,
                instruction: ip as u32,
            },
            Dialogue {
                yielded: WireShape::Scalar { kind: 1 },
                reply: WireShape::Scalar { kind },
            },
        )]);
        let arena = keleusma_arena::Arena::with_capacity(65536);
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        assert!(matches!(
            vm.call(&[]).unwrap(),
            VmState::Yielded(Value::Bool(false))
        ));
        let VmState::Finished(actual) = vm.resume(value.clone()).unwrap() else {
            panic!("expected completion")
        };
        assert_eq!(actual, value);
        for optimized in [false, true] {
            let context = Context::create();
            let module =
                coroutine::lower_with_dialogues(&context, &program, &machine(), 4096, &contracts)
                    .unwrap();
            if optimized {
                common::force_optimize(&module);
            }
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            let prefix = format!("kel_coroutine_{entry}");
            let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
            let unused = Guarded::new(0);
            unsafe {
                let start = engine
                    .get_function::<StartZero>(&format!("{prefix}_start"))
                    .unwrap();
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                assert_eq!(
                    start.call(unused.ptr(), unused.ptr(), unused.ptr(), slot.ptr()),
                    Outcome { live: 1, value: 0 }
                );
                assert_eq!(
                    resume.call(slot.ptr(), bits),
                    Outcome {
                        live: 2,
                        value: bits
                    }
                );
                engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap()
                    .call(slot.ptr());
            }
            slot.check();
            unused.check();
        }
    }
    let program = common::build("loop main(a: Word) -> Word { yield a }");
    let entry = program.entry_point.unwrap();
    let ip = program.chunks[entry]
        .ops
        .iter()
        .position(|op| matches!(op, Op::Yield))
        .unwrap();
    let contracts = BTreeMap::from([(
        YieldSite {
            chunk: entry as u32,
            instruction: ip as u32,
        },
        Dialogue {
            yielded: WireShape::Scalar { kind: 3 },
            reply: WireShape::Scalar { kind: 2 },
        },
    )]);
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    vm.call(&[Value::Int(5)]).unwrap();
    assert!(vm.resume(Value::Byte(7)).is_err());
    assert!(
        coroutine::lower_with_dialogues(&Context::create(), &program, &machine(), 4096, &contracts)
            .unwrap_err()
            .to_string()
            .contains("stream dialogue reply")
    );
}

#[test]
fn composite_reply_kinds_must_match_the_consuming_operation() {
    use coroutine::{Dialogue, YieldSite};
    use keleusma::bytecode::{ArrayBody, Op, TupleBody, Value, WireShape};
    use keleusma::vm::{Vm, VmState};
    use std::collections::BTreeMap;
    for (source, reply_kind) in [
        (
            "yield main(a: Word) -> Word { let p: (Word, Word) = yield a; p.0 }",
            1,
        ),
        (
            "yield main(a: Word) -> Word { let p: [Word; 2] = yield a; p[0] }",
            0,
        ),
        (
            "struct P { x: Word, y: Word } yield main(a: Word) -> Word { let p: P = yield a; p.x }",
            0,
        ),
        (
            "private data st { p: (Word, Word) } yield main(a: Word) -> Word { let p: (Word, Word) = yield a; st.p = p; st.p.0 }",
            1,
        ),
        (
            "private data st { p: (Word, Word), q: (Word, Word) } yield main(a: Word) -> Word { let p: (Word, Word) = yield a; st.p = p; st.q = st.p; st.q.0 }",
            1,
        ),
    ] {
        let program = common::build(source);
        let mut arena = keleusma_arena::Arena::with_capacity(65536);
        arena
            .resize_persistent(keleusma::vm::required_persistent_capacity_for(&program))
            .unwrap();
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        assert!(matches!(
            vm.call(&[Value::Int(5)]).unwrap(),
            VmState::Yielded(Value::Int(5))
        ));
        let fields = Box::new(vec![Value::Int(7), Value::Int(8)]);
        let reply = if reply_kind == 0 {
            Value::Tuple(TupleBody::Boxed(fields))
        } else {
            Value::Array(ArrayBody::Boxed(fields))
        };
        assert!(vm.resume(reply).is_err(), "{source}");
        let entry = program.entry_point.unwrap();
        let ip = program.chunks[entry]
            .ops
            .iter()
            .position(|op| matches!(op, Op::Yield))
            .unwrap();
        let contracts = BTreeMap::from([(
            YieldSite {
                chunk: entry as u32,
                instruction: ip as u32,
            },
            Dialogue {
                yielded: WireShape::Scalar { kind: 3 },
                reply: WireShape::Flat {
                    kind: reply_kind,
                    size: 16,
                },
            },
        )]);
        let error = coroutine::lower_with_dialogues(
            &Context::create(),
            &program,
            &machine(),
            4096,
            &contracts,
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("types are not proven"),
            "{source}: {error}"
        );
    }
    // Chained private writes require more than one pass through the slot-kind
    // equations. Correct inference preserves the storage aliases across yield.
    let source = "private data st { p: (Word, Word), q: (Word, Word) } loop main(a: Word) -> Word { st.p = (a, a + 1); st.q = st.p; let keep = st.q; yield keep.0; yield keep.1 }";
    let replies = [7, 20, 3, 40];
    let expected = [5, 6, 20, 21];
    assert_eq!(common::general_vm_sequence(source, 5, &replies), expected);
    for optimize in [false, true] {
        assert_eq!(native(source, 5, &replies, optimize), expected);
    }
}

thread_local! {
    static NATIVE_PAIR_BUFFER: std::cell::RefCell<[i64; 2]> = const { std::cell::RefCell::new([0; 2]) };
    static NATIVE_PAIR_CALLS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

extern "C" fn native_pair_snapshot(value: i64) -> i64 {
    NATIVE_PAIR_CALLS.with(|calls| calls.set(calls.get() + 1));
    NATIVE_PAIR_BUFFER.with(|buffer| {
        let mut body = buffer.borrow_mut();
        *body = [value, value + 1];
        body.as_ptr() as i64
    })
}

extern "C" fn native_pair_alias(address: i64) -> i64 {
    address
}

#[test]
fn native_snapshot_results_survive_calls_suspension_and_completion() {
    use coroutine::{Dialogue, HostContracts, NativeBodyReturn, YieldSite};
    use keleusma::bytecode::{Op, TupleBody, Value, WireShape};
    use keleusma::vm::{Vm, VmState};
    use std::collections::BTreeMap;
    for (delegated, external) in [(0, false), (1, false), (2, false), (1, true), (2, true)] {
        let source = if delegated == 2 {
            "use host::pair\nyield make(a: Word) -> (Word, Word) { let p: (Word, Word) = host::pair(a); yield p.0; p } yield main(a: Word) -> (Word, Word) { let keep = make(a); let later = make(a + 11); keep }"
        } else if delegated == 1 {
            "use host::pair\nfn make(a: Word) -> (Word, Word) { host::pair(a) } yield main(a: Word) -> (Word, Word) { let keep = make(a); yield keep.0; let later = make(a + 10); yield later.1; keep }"
        } else {
            "use host::pair\nyield main(a: Word) -> (Word, Word) { let keep: (Word, Word) = host::pair(a); yield keep.0; let later: (Word, Word) = host::pair(a + 10); yield later.1; keep }"
        };
        let source = if external {
            source.replace("use host::pair", "use external host::pair")
        } else {
            source.to_owned()
        };
        let mut program = common::build(&source);
        program.native_return_shapes[0] = WireShape::Flat { kind: 0, size: 16 };
        let dialogues = program
            .chunks
            .iter()
            .enumerate()
            .flat_map(|(ci, chunk)| {
                chunk.ops.iter().enumerate().filter_map(move |(ip, op)| {
                    matches!(op, Op::Yield).then_some((
                        YieldSite {
                            chunk: ci as u32,
                            instruction: ip as u32,
                        },
                        Dialogue {
                            yielded: WireShape::Scalar { kind: 3 },
                            reply: WireShape::Scalar { kind: 3 },
                        },
                    ))
                })
            })
            .collect();
        let contracts = HostContracts {
            dialogues: Some(dialogues),
            native_body_returns: BTreeMap::from([(0, NativeBodyReturn::Snapshot)]),
        };
        let arena = keleusma_arena::Arena::with_capacity(65536);
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        if external {
            vm.register_external_native(
                "host::pair",
                |args| {
                    let Value::Int(a) = args[0] else {
                        panic!("expected Word")
                    };
                    Ok(Value::tuple(vec![Value::Int(a), Value::Int(a + 1)]))
                },
                2,
            );
        } else {
            vm.register_fn("host::pair", |a: i64| (a, a + 1));
        }
        assert!(matches!(
            vm.call(&[Value::Int(5)]).unwrap(),
            VmState::Yielded(Value::Int(5))
        ));
        assert!(matches!(
            vm.resume(Value::Int(7)).unwrap(),
            VmState::Yielded(Value::Int(16))
        ));
        let VmState::Finished(Value::Tuple(TupleBody::Flat(body))) =
            vm.resume(Value::Int(0)).unwrap()
        else {
            panic!("expected completed pair")
        };
        let expected: Vec<_> = body
            .resolve(&arena)
            .unwrap()
            .as_chunks::<8>()
            .0
            .iter()
            .map(|bytes| i64::from_le_bytes(*bytes))
            .collect();
        assert_eq!(expected, [5, 6]);
        for optimized in [false, true] {
            let context = Context::create();
            let module =
                coroutine::lower_with_contracts(&context, &program, &machine(), 4096, &contracts)
                    .unwrap();
            if optimized {
                common::force_optimize(&module);
            }
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            let symbol = module
                .get_function(&keleusma_native::native_symbol("host::pair"))
                .unwrap();
            engine.add_global_mapping(&symbol, native_pair_snapshot as *const () as usize);
            let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
            let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
            let shared = Guarded::new(program.shared_data_bytes as usize);
            let private = Guarded::new(
                keleusma::vm::required_persistent_capacity_for(&program)
                    + region::persistent_supplement_bytes(&program) as usize,
            );
            let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
            unsafe {
                let start = engine
                    .get_function::<HandleStart>(&format!("{prefix}_start"))
                    .unwrap();
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                let release = engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap();
                for _ in 0..2 {
                    NATIVE_PAIR_CALLS.with(|calls| calls.set(0));
                    assert_eq!(
                        start.call(5, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr()),
                        Outcome { live: 1, value: 5 }
                    );
                    NATIVE_PAIR_BUFFER.with(|buffer| *buffer.borrow_mut() = [99, 100]);
                    assert_eq!(resume.call(slot.ptr(), 7), Outcome { live: 1, value: 16 });
                    NATIVE_PAIR_BUFFER.with(|buffer| *buffer.borrow_mut() = [199, 200]);
                    let end = resume.call(slot.ptr(), 0);
                    assert_eq!(end.live, 2);
                    // Completion lives after LLVM's frame and before site metadata.
                    assert_eq!(end.value as *mut u8, slot.ptr().add(16 + 4096 - 8 - 16));
                    std::slice::from_raw_parts_mut(slot.ptr().add(16), 4096 - 8 - 16).fill(0xdd);
                    assert_eq!(
                        std::slice::from_raw_parts(end.value as *const i64, 2),
                        &expected
                    );
                    assert_eq!(NATIVE_PAIR_CALLS.with(|calls| calls.get()), 2);
                    release.call(slot.ptr());
                }
                NATIVE_PAIR_CALLS.with(|calls| calls.set(0));
                assert_eq!(
                    start
                        .call(5, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr())
                        .live,
                    1
                );
                release.call(slot.ptr());
                assert_eq!(NATIVE_PAIR_CALLS.with(|calls| calls.get()), 1);
                assert_eq!(resume.call(slot.ptr(), 0), Outcome { live: 0, value: 0 });
            }
            for reservation in [&slot, &shared, &private, &bodies] {
                reservation.check();
            }
        }
        let error = coroutine::lower_with_contracts(
            &Context::create(),
            &program,
            &machine(),
            32,
            &contracts,
        )
        .unwrap_err();
        assert!(error.to_string().contains("frame"), "{error}");
        let mut bad = contracts.clone();
        bad.native_body_returns
            .insert(1, NativeBodyReturn::Snapshot);
        assert!(
            coroutine::lower_with_contracts(&Context::create(), &program, &machine(), 4096, &bad)
                .unwrap_err()
                .to_string()
                .contains("bounded flat return signature")
        );
        program.native_return_shapes[0] = WireShape::Top;
        assert!(
            coroutine::lower_with_contracts(
                &Context::create(),
                &program,
                &machine(),
                4096,
                &contracts
            )
            .unwrap_err()
            .to_string()
            .contains("bounded flat return signature")
        );
    }
}

#[test]
fn native_instance_borrows_preserve_aliases_while_snapshots_copy() {
    use coroutine::{HostContracts, NativeBodyReturn};
    use keleusma::bytecode::{Value, WireShape};
    use keleusma::vm::{Vm, VmState};
    use std::collections::BTreeMap;
    let mut program = common::build(
        "use host::alias\nprivate data st { pair: (Word, Word) } yield main(a: Word) -> Word { st.pair = (a, a + 1); let keep: (Word, Word) = host::alias(st.pair); yield keep.0; st.pair = (7, 8); keep.1 }",
    );
    program.native_return_shapes[0] = WireShape::Flat { kind: 0, size: 16 };
    for (contract, expected) in [
        (NativeBodyReturn::InstanceBorrow, 8),
        (NativeBodyReturn::Snapshot, 6),
    ] {
        let mut arena = keleusma_arena::Arena::with_capacity(65536);
        arena
            .resize_persistent(keleusma::vm::required_persistent_capacity_for(&program))
            .unwrap();
        let mut vm = Vm::new(program.clone(), &arena).unwrap();
        if contract == NativeBodyReturn::InstanceBorrow {
            vm.register_native("host::alias", |args| Ok(args[0].clone()));
        } else {
            vm.register_fn("host::alias", |pair: (i64, i64)| pair);
        }
        assert!(matches!(
            vm.call(&[Value::Int(5)]).unwrap(),
            VmState::Yielded(Value::Int(5))
        ));
        assert!(
            matches!(vm.resume(Value::Int(0)).unwrap(), VmState::Finished(Value::Int(result)) if result == expected)
        );
        let contracts = HostContracts {
            dialogues: None,
            native_body_returns: BTreeMap::from([(0, contract)]),
        };
        for optimized in [false, true] {
            let context = Context::create();
            let module =
                coroutine::lower_with_contracts(&context, &program, &machine(), 4096, &contracts)
                    .unwrap();
            if optimized {
                common::force_optimize(&module);
            }
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            engine.add_global_mapping(
                &module
                    .get_function(&keleusma_native::native_symbol("host::alias"))
                    .unwrap(),
                native_pair_alias as *const () as usize,
            );
            let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
            let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
            let shared = Guarded::new(program.shared_data_bytes as usize);
            let private = Guarded::new(
                keleusma::vm::required_persistent_capacity_for(&program)
                    + region::persistent_supplement_bytes(&program) as usize,
            );
            let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
            unsafe {
                let start = engine
                    .get_function::<HandleStart>(&format!("{prefix}_start"))
                    .unwrap();
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                let release = engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap();
                assert_eq!(
                    start.call(5, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr()),
                    Outcome { live: 1, value: 5 }
                );
                assert_eq!(
                    resume.call(slot.ptr(), 0),
                    Outcome {
                        live: 2,
                        value: expected
                    }
                );
                release.call(slot.ptr());
            }
            for reservation in [&slot, &shared, &private, &bodies] {
                reservation.check();
            }
        }
    }
}

#[test]
fn unused_chunks_do_not_change_coroutine_admission_or_site_identifiers() {
    use coroutine::{Dialogue, YieldSite};
    use keleusma::bytecode::{Op, Value, WireShape};
    use keleusma::vm::{Vm, VmState};
    use std::collections::BTreeMap;
    let source = "use host::unused\nfn unused_native(a: Word) -> Float { host::unused(a) } fn unused_text(x: Text) -> Text { x } loop unused_stream(a: Word) -> Word { yield a } yield unused_yield(a: Byte) -> Byte { yield a } yield helper(a: Word) -> Word { let b = yield a + 1; b + a } yield main(a: Word) -> Word { helper(a) + a }";
    let mut program = common::build(source);
    program.native_return_shapes[0] = WireShape::Scalar { kind: 5 };
    let helper = program
        .chunks
        .iter()
        .position(|chunk| chunk.name == "helper")
        .unwrap();
    let site = YieldSite {
        chunk: helper as u32,
        instruction: program.chunks[helper]
            .ops
            .iter()
            .position(|op| matches!(op, Op::Yield))
            .unwrap() as u32,
    };
    let contract = Dialogue {
        yielded: WireShape::Scalar { kind: 3 },
        reply: WireShape::Scalar { kind: 3 },
    };
    let arena = keleusma_arena::Arena::with_capacity(65536);
    let mut vm = Vm::new(program.clone(), &arena).unwrap();
    assert!(matches!(
        vm.call(&[Value::Int(5)]).unwrap(),
        VmState::Yielded(Value::Int(6))
    ));
    assert!(matches!(
        vm.resume(Value::Int(7)).unwrap(),
        VmState::Finished(Value::Int(17))
    ));
    for optimized in [false, true] {
        for all_sites in [false, true] {
            let mut contracts = BTreeMap::from([(site, contract)]);
            if all_sites {
                for (ci, chunk) in program.chunks.iter().enumerate() {
                    if ci == helper {
                        continue;
                    }
                    for (ip, op) in chunk.ops.iter().enumerate() {
                        if matches!(op, Op::Yield) {
                            contracts.insert(
                                YieldSite {
                                    chunk: ci as u32,
                                    instruction: ip as u32,
                                },
                                Dialogue {
                                    yielded: WireShape::Top,
                                    reply: WireShape::Top,
                                },
                            );
                        }
                    }
                }
            }
            let context = Context::create();
            let module =
                coroutine::lower_with_dialogues(&context, &program, &machine(), 4096, &contracts)
                    .unwrap();
            if optimized {
                common::force_optimize(&module);
            }
            assert!(
                module
                    .get_function(&keleusma_native::native_symbol("host::unused"))
                    .is_none()
            );
            let engine = module
                .create_jit_execution_engine(OptimizationLevel::None)
                .unwrap();
            let prefix = format!("kel_coroutine_{}", program.entry_point.unwrap());
            let slot = Guarded::new(coroutine::slot_bytes(4096).unwrap() as usize);
            let shared = Guarded::new(program.shared_data_bytes as usize);
            let private = Guarded::new(
                keleusma::vm::required_persistent_capacity_for(&program)
                    + region::persistent_supplement_bytes(&program) as usize,
            );
            let bodies = Guarded::new(region::host_arena_supplement_bytes(&program) as usize);
            unsafe {
                let start = engine
                    .get_function::<HandleStart>(&format!("{prefix}_start"))
                    .unwrap();
                let resume = engine
                    .get_function::<HandleResume>(&format!("{prefix}_resume"))
                    .unwrap();
                let query = engine
                    .get_function::<unsafe extern "C" fn(*mut u8) -> u64>(&format!(
                        "{prefix}_yield_site"
                    ))
                    .unwrap();
                assert_eq!(
                    start.call(5, shared.ptr(), private.ptr(), bodies.ptr(), slot.ptr()),
                    Outcome { live: 1, value: 6 }
                );
                assert_eq!(query.call(slot.ptr()), site.id());
                assert_eq!(resume.call(slot.ptr(), 7), Outcome { live: 2, value: 17 });
                engine
                    .get_function::<HandleRelease>(&format!("{prefix}_release"))
                    .unwrap()
                    .call(slot.ptr());
            }
            for reservation in [&slot, &shared, &private, &bodies] {
                reservation.check();
            }
        }
    }
    // Unused signature metadata is unnecessary, but reachable native returns
    // still pass through the backend's native floating-point ABI boundary.
    let unused = program
        .chunks
        .iter()
        .position(|chunk| chunk.name == "unused_text")
        .unwrap();
    program.signatures[unused].params.clear();
    coroutine::lower(&Context::create(), &program, &machine(), 4096).unwrap();
    let mut reachable = common::build(
        "use host::unused\nfn helper(a: Word) -> Float { host::unused(a) } yield main(a: Word) -> Float { let f = helper(a); yield f; f }",
    );
    reachable.native_return_shapes[0] = WireShape::Scalar { kind: 5 };
    assert!(
        coroutine::lower(&Context::create(), &reachable, &machine(), 4096)
            .unwrap_err()
            .to_string()
            .contains("Float RETURN SHAPE")
    );
}

#[test]
fn native_snapshot_completion_links_from_c_and_emits_for_tier_one_targets() {
    use coroutine::{Dialogue, HostContracts, NativeBodyReturn, YieldSite};
    use inkwell::targets::{FileType, TargetTriple};
    use inkwell::values::BasicValue;
    use keleusma::bytecode::{Op, WireShape};
    use std::collections::BTreeMap;
    use std::process::Command;
    let mut program = common::build(
        "use host::pair\nfn make(a: Word) -> (Word, Word) { host::pair(a) } yield main(a: Word) -> (Word, Word) { let p = make(a); yield p.0; p }",
    );
    program.native_return_shapes[0] = WireShape::Flat { kind: 0, size: 16 };
    let entry = program.entry_point.unwrap();
    let site = YieldSite {
        chunk: entry as u32,
        instruction: program.chunks[entry]
            .ops
            .iter()
            .position(|op| matches!(op, Op::Yield))
            .unwrap() as u32,
    };
    let contracts = HostContracts {
        dialogues: Some(BTreeMap::from([(
            site,
            Dialogue {
                yielded: WireShape::Scalar { kind: 3 },
                reply: WireShape::Scalar { kind: 0 },
            },
        )])),
        native_body_returns: BTreeMap::from([(0, NativeBodyReturn::Snapshot)]),
    };
    let directory = std::path::PathBuf::from("../tmp")
        .join(format!("retcon-native-completion-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for optimized in [false, true] {
        let context = Context::create();
        let target = machine();
        let module =
            coroutine::lower_with_contracts(&context, &program, &target, 4096, &contracts).unwrap();
        if optimized {
            common::force_optimize(&module);
        }
        let object = directory.join("coroutine.o");
        target
            .write_to_file(&module, FileType::Object, &object)
            .unwrap();
        let host = directory.join("host.c");
        std::fs::write(
            &host,
            format!(
                r#"
#include <assert.h>
#include <stdint.h>
#include <stdalign.h>
#include <string.h>
struct outcome {{ uint64_t status; int64_t value; }};
extern struct outcome kel_coroutine_{entry}_start(int64_t, void *, void *, void *, void *);
extern struct outcome kel_coroutine_{entry}_resume(void *, int64_t);
extern void kel_coroutine_{entry}_release(void *);
extern uint64_t kel_coroutine_{entry}_yield_site(void *);
static int64_t transfer[2];
int64_t {native}(int64_t a) {{
    transfer[0] = a; transfer[1] = a + 1;
    return (int64_t)(uintptr_t)transfer;
}}
struct guarded {{ uint64_t before; alignas(8) unsigned char slot[4112]; uint64_t after; }};
int main(void) {{
    struct guarded a, b;
    a.before = b.before = a.after = b.after = UINT64_C(0xa5a5a5a5a5a5a5a5);
    struct outcome first = kel_coroutine_{entry}_start(5, 0, 0, 0, a.slot);
    assert(first.status == 1 && first.value == 5);
    assert(kel_coroutine_{entry}_yield_site(a.slot) == UINT64_C({site}));
    struct outcome second = kel_coroutine_{entry}_start(20, 0, 0, 0, b.slot);
    assert(second.status == 1 && second.value == 20);
    transfer[0] = transfer[1] = 999;
    first = kel_coroutine_{entry}_resume(a.slot, 0);
    second = kel_coroutine_{entry}_resume(b.slot, 0);
    assert(first.status == 2 && second.status == 2);
    assert(kel_coroutine_{entry}_yield_site(a.slot) == UINT64_MAX);
    memset(a.slot + 16, 0xdd, 4072);
    memset(b.slot + 16, 0xdd, 4072);
    const int64_t *x = (const int64_t *)(uintptr_t)first.value;
    const int64_t *y = (const int64_t *)(uintptr_t)second.value;
    assert(x[0] == 5 && x[1] == 6 && y[0] == 20 && y[1] == 21);
    kel_coroutine_{entry}_release(a.slot);
    kel_coroutine_{entry}_release(b.slot);
    first = kel_coroutine_{entry}_resume(a.slot, 0);
    assert(first.status == 0 && first.value == 0);
    assert(a.before == UINT64_C(0xa5a5a5a5a5a5a5a5) && a.after == a.before);
    assert(b.before == a.before && b.after == a.before);
    return 0;
}}
"#,
                native = keleusma_native::native_symbol("host::pair"),
                site = site.id()
            ),
        )
        .unwrap();
        let executable = directory.join("host");
        let linked = Command::new("cc")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
            .arg(&host)
            .arg(&object)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            linked.status.success(),
            "{}",
            String::from_utf8_lossy(&linked.stderr)
        );
        let run = Command::new(executable).output().unwrap();
        assert!(
            run.status.success(),
            "{:?}: {}",
            run.status,
            String::from_utf8_lossy(&run.stderr)
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
    // Cross-target object emission is checked separately from host execution.
    // This establishes no execution or timing result on another platform.
    Target::initialize_all(&InitializationConfig::default());
    for name in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ] {
        let triple = TargetTriple::create(name);
        let target = Target::from_triple(&triple)
            .unwrap()
            .create_target_machine(
                &triple,
                "generic",
                "",
                OptimizationLevel::Default,
                RelocMode::PIC,
                CodeModel::Default,
            )
            .unwrap();
        let context = Context::create();
        let module =
            coroutine::lower_with_contracts(&context, &program, &target, 4096, &contracts).unwrap();
        module
            .run_passes(
                "default<O2>",
                &target,
                inkwell::passes::PassBuilderOptions::create(),
            )
            .unwrap();
        module.verify().unwrap();
        let object = target
            .write_to_memory_buffer(&module, FileType::Object)
            .unwrap();
        let bytes = object.as_slice();
        assert!(bytes.len() > 64, "{name}");
        assert!(
            bytes.starts_with(if name.contains("linux") {
                b"\x7fELF"
            } else {
                b"\xcf\xfa\xed\xfe"
            }),
            "{name}"
        );
        if name.contains("linux") {
            assert_eq!(
                u16::from_le_bytes(bytes[18..20].try_into().unwrap()),
                if name.starts_with("x86_64") { 62 } else { 183 },
                "{name}"
            );
        } else {
            assert_eq!(
                u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
                if name.starts_with("x86_64") {
                    0x01000007
                } else {
                    0x0100000c
                },
                "{name}"
            );
        }
        assert!(module.get_function("malloc").is_none());
        assert!(module.get_function("free").is_none());
        assert!(
            module
                .get_function("kel_retcon_overflow")
                .is_none_or(|f| f.as_global_value().get_first_use().is_none())
        );
    }
}

#[test]
fn host_control_routes_are_explicit_and_do_not_fall_back_to_callbacks() {
    use keleusma_native::{LowerOptions, lower_module};
    for source in [
        "fn main(a: Word) -> Word { a + 1 }",
        "loop main(a: Word) -> Word { yield a + 1 }",
        "yield main(a: Word) -> Word { yield a + 1; a + 2 }",
    ] {
        let program = common::build(source);
        let entry = program.entry_point.unwrap();
        let category = program.chunks[entry].block_type;
        let context = Context::create();
        let ordinary = context.create_module("ordinary_control");
        lower_module(&context, &ordinary, &program, LowerOptions::default()).unwrap();
        ordinary.verify().unwrap();
        assert!(
            ordinary
                .get_function(&format!("kel_coroutine_{entry}_start"))
                .is_none()
        );
        let lowered = coroutine::lower(&context, &program, &machine(), 4096);
        if category == keleusma::bytecode::BlockType::Func {
            assert!(
                lowered
                    .unwrap_err()
                    .to_string()
                    .contains("Stream or Reentrant entry")
            );
            assert!(ordinary.get_function("kel_yield").is_none());
        } else {
            let lowered = lowered.unwrap();
            assert!(lowered.get_function("kel_yield").is_none());
            for suffix in ["start", "resume", "release"] {
                assert!(
                    lowered
                        .get_function(&format!("kel_coroutine_{entry}_{suffix}"))
                        .is_some()
                );
            }
            assert_eq!(
                ordinary.get_function("kel_yield").is_some(),
                category == keleusma::bytecode::BlockType::Reentrant
            );
        }
    }
}
