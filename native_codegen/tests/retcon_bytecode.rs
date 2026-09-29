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
    native_module_read(program, seed, first, replies, optimize, |bits, _, _| bits)
}

fn native_module_read<T>(
    program: &keleusma::bytecode::Module,
    seed: &[u8],
    first: i64,
    replies: &[i64],
    optimize: bool,
    mut read: impl FnMut(i64, &Guarded, &Guarded) -> T,
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
    let frame = Guarded::new(4096);
    let reply = Guarded::new(8);
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
        if float_input {
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
        result.push(read(step.value, &region, &private));
        for storage in [&frame, &reply, &shared, &private, &region] {
            storage.check();
        }
        if i + 1 < replies.len() {
            unsafe {
                reply.ptr().cast::<i64>().write(value);
            }
            let resume: Resume = unsafe { std::mem::transmute(step.next) };
            step = unsafe { resume(frame.ptr(), false) };
        }
    }
    let release: Resume = unsafe { std::mem::transmute(step.next) };
    let end = unsafe { release(frame.ptr(), true) };
    assert!(end.next.is_null());
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
    // An unrelated reentrant yield with a different signature must also
    // remain refused without bypassing coroutine boundary admission.
    let source = std::fs::read_to_string("../examples/scripts/13_telemetry_stream.kel").unwrap();
    let with_delegate = common::build(&format!(
        "{source}\nyield unused(a: Word) -> Word {{ yield a }}"
    ));
    let delegated_error = coroutine::lower(&ctx, &with_delegate, &tm, 4096).unwrap_err();
    assert!(
        delegated_error
            .to_string()
            .contains("yield signature changes")
    );
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
            error.to_string().contains("yield signature changes"),
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
        error.to_string().contains("yield signature changes"),
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
fn mixed_enum_receiver_kinds_remain_an_explicit_boundary() {
    use keleusma::bytecode::Value;
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    let src = "enum E { A, B } loop main(t: Word) -> Word { let r = yield 5; let e = if t > 0 { E::A } else { r }; yield match e { E::A => 1, _ => 0 } }";
    let p = common::build(src);
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("mixed or unknown receiver kinds"),
        "{error}"
    );
    for (reply, expected) in [(7, 1), (-7, 0)] {
        let need = required_persistent_capacity_for(&p);
        let mut arena = keleusma_arena::Arena::with_capacity(
            auto_arena_capacity_for(&p, &[]).unwrap() + need + 65536,
        );
        arena.resize_persistent(need).unwrap();
        let mut vm = Vm::new(p.clone(), &arena).unwrap();
        assert!(matches!(
            vm.call(&[Value::Int(5)]).unwrap(),
            VmState::Yielded(Value::Int(5))
        ));
        assert!(
            matches!(vm.resume(Value::Int(reply)).unwrap(), VmState::Yielded(Value::Int(v)) if v == expected)
        );
    }
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
    for optimize in [false, true] {
        let (actual, _) = native_module_read(
            &p,
            &vec![0; p.shared_data_bytes as usize],
            first,
            replies,
            optimize,
            |bits, region, private| {
                let address = bits as usize;
                let end = address.checked_add(size as usize).unwrap();
                let storage = [region, private]
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
    // Private layout records an extent but no composite kind. Passing its
    // unknown kind into a tuple parameter remains a type-admission boundary.
    let src = "private data st { p: (Word, Word) } fn pick(p: (Word, Word), a: Word) -> Word { p.0 + a } yield emit(t: Word) -> Word { let r: Word = yield t; st.p = (r, r); r } loop main(t: Word) -> Word { st.p = (t, t); yield pick(st.p, emit(t)) }";
    assert_eq!(common::general_vm_sequence(src, 5, &replies), expected);
    let p = common::build(src);
    let error = coroutine::lower(&Context::create(), &p, &machine(), 4096).unwrap_err();
    assert!(
        error.to_string().contains("types are not proven"),
        "{error}"
    );
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
