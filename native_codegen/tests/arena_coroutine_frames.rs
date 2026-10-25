//! **Workstream C: coroutine frames resident in the host arena.**
//!
//! The roadmap's Order 2 gate is "coroutine-driven native functions callable
//! from a host, frames arena-resident". Workstream B made the coroutines
//! callable through stable slots. This file checks the arena half: each frame's
//! size is a static bound derived from the lowering, and slots of that size come
//! from a fixed pool in a `keleusma::Arena`, returned to its free list on
//! release, with no further arena use while instances start, resume and end.
//!
//! Every buffer an instance receives also comes from the same arena, sized by
//! the published contract, so a whole multi-instance run is one arena's bytes.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::Arena;
use keleusma_native::coroutine::pool::{PoolError, SlotPool};
use keleusma_native::coroutine::{self, HostContracts};
use keleusma_native::region;

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    status: u64,
    value: i64,
}
type HandleStart = unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8, *mut u8) -> Outcome;
type HandleResume = unsafe extern "C" fn(*mut u8, i64) -> Outcome;
type HandleRelease = unsafe extern "C" fn(*mut u8);

/// Streams and reentrants with state live across suspension, so the frame
/// bound is not trivially the header alone.
const SUBJECTS: &[(&str, i64, &[i64])] = &[
    (
        "loop main(a: Word) -> Word { let keep = a + 100; let r = 10 + (yield a); yield r + keep }",
        5,
        &[7, 20, 3, 40, 1, 0],
    ),
    (
        "yield helper(a: Word) -> Word { let r = yield a + 1; r + a } \
         loop main(a: Word) -> Word { let v = helper(a + 2); let w = yield v; yield w + a }",
        3,
        &[13, 4, 9, 2, 6, 1],
    ),
];

#[test]
fn the_static_frame_bound_is_the_smallest_reservation_that_lowers() {
    let tm = machine();
    for (src, _, _) in SUBJECTS {
        let program = common::build(src);
        let bound = coroutine::minimum_frame_bytes(&program, &tm, &HostContracts::default())
            .expect("a frame bound");
        assert_eq!(bound % coroutine::FRAME_ALIGN, 0, "the bound is aligned");
        let ctx = Context::create();
        coroutine::lower(&ctx, &program, &tm, bound).expect("the bound itself lowers");
        if bound > coroutine::FRAME_ALIGN {
            let ctx = Context::create();
            let below = coroutine::lower(&ctx, &program, &tm, bound - coroutine::FRAME_ALIGN);
            let refusal = match below {
                Ok(_) => panic!("{src}: one step below the {bound}-byte bound still lowers"),
                Err(e) => format!("{e:?}"),
            };
            assert!(
                refusal.contains("exceeds the caller reservation") || refusal.contains("too small"),
                "the refusal below the bound names the reservation: {refusal}"
            );
        }
        assert!(
            bound < 4096,
            "a {bound}-byte frame for {src}; the 4096 the other harnesses reserve was an over-estimate, \
             and this subject should be far below it"
        );
        println!("frame bound {bound} bytes for {src}");
    }
}

/// An aligned buffer of `len` bytes from the bottom of `arena`.
fn arena_buffer(arena: &Arena, len: usize) -> *mut u8 {
    let raw = arena.alloc_bottom_bytes(len + 8).expect("arena buffer");
    let p = raw.as_ptr() as *mut u8;
    let p = unsafe { p.add(p.align_offset(8)) };
    unsafe { std::ptr::write_bytes(p, 0, len) };
    p
}

#[test]
fn coroutine_frames_live_in_pooled_arena_slots_and_agree_with_the_vm() {
    const INSTANCES: u32 = 4;
    let tm = machine();
    for (src, first, replies) in SUBJECTS {
        let program = common::build(src);
        let bound = coroutine::minimum_frame_bytes(&program, &tm, &HostContracts::default())
            .expect("frame bound");
        let ctx = Context::create();
        let module = coroutine::lower(&ctx, &program, &tm, bound).expect("lower");
        let engine = module
            .create_jit_execution_engine(OptimizationLevel::None)
            .unwrap();
        let entry = program.entry_point.unwrap();
        let start = unsafe {
            engine
                .get_function::<HandleStart>(&format!("kel_coroutine_{entry}_start"))
                .unwrap()
        };
        let resume = unsafe {
            engine
                .get_function::<HandleResume>(&format!("kel_coroutine_{entry}_resume"))
                .unwrap()
        };
        let release = unsafe {
            engine
                .get_function::<HandleRelease>(&format!("kel_coroutine_{entry}_release"))
                .unwrap()
        };

        let shared_len = program.shared_data_bytes as usize;
        let private_len = keleusma::vm::required_persistent_capacity_for(&program)
            + region::persistent_supplement_bytes(&program) as usize;
        let region_len = region::host_arena_supplement_bytes(&program) as usize;
        let arena = Arena::with_capacity(1 << 20);
        let buffers: Vec<[*mut u8; 3]> = (0..INSTANCES)
            .map(|_| {
                let b = [
                    arena_buffer(&arena, shared_len),
                    arena_buffer(&arena, private_len),
                    arena_buffer(&arena, region_len),
                ];
                common::install_private_init_bytes(&program, unsafe {
                    std::slice::from_raw_parts_mut(b[1], private_len)
                });
                b
            })
            .collect();
        let before_pool = arena.bottom_used();
        let mut pool = SlotPool::new(&arena, bound, INSTANCES).expect("pool");
        assert_eq!(
            arena.bottom_used() - before_pool,
            pool.reserved_bytes(),
            "the pool's reservation is exactly what the arena gave up"
        );
        let settled = arena.bottom_used();

        // Each instance starts from its own argument so the interleaving would
        // show any sharing of state between frames.
        let firsts: Vec<i64> = (0..INSTANCES as i64).map(|i| first + 10 * i).collect();
        let mut slots = Vec::new();
        let mut seen = vec![Vec::new(); INSTANCES as usize];
        for (i, f) in firsts.iter().enumerate() {
            let slot = pool.acquire().expect("a free slot");
            assert!(
                arena.addr_is_ephemeral(slot.as_ptr() as usize),
                "slot is in the arena"
            );
            assert_eq!(slot.as_ptr() as usize % coroutine::FRAME_ALIGN as usize, 0);
            let [s, p, r] = buffers[i];
            let out = unsafe { start.call(*f, s, p, r, slot.as_ptr()) };
            assert_eq!(out.status, 1, "a stream yields at start");
            seen[i].push(out.value);
            slots.push(slot);
        }
        assert!(
            pool.acquire().is_none(),
            "the pool is exhausted at its capacity"
        );
        // Round-robin resumes: every instance advances one step per round.
        for reply in replies.iter().take(replies.len() - 1) {
            for (i, slot) in slots.iter().enumerate() {
                let out = unsafe { resume.call(slot.as_ptr(), *reply + i as i64) };
                assert_eq!(out.status, 1);
                seen[i].push(out.value);
            }
            assert_eq!(arena.bottom_used(), settled, "a resume used arena memory");
        }
        for (i, f) in firsts.iter().enumerate() {
            let per: Vec<i64> = replies.iter().map(|r| r + i as i64).collect();
            let vm = common::general_vm_sequence(src, *f, &per);
            assert_eq!(seen[i], vm[..seen[i].len()], "instance {i} of {src}");
        }
        for slot in slots {
            let slot = match pool.release(slot) {
                Err((slot, PoolError::Live)) => slot,
                other => panic!("a live slot was accepted back: {other:?}"),
            };
            unsafe { release.call(slot.as_ptr()) };
            pool.release(slot)
                .expect("a released slot returns to the pool");
        }
        assert_eq!(pool.available(), INSTANCES);
        assert_eq!(arena.bottom_used(), settled, "release used arena memory");
    }
}

#[test]
fn the_pool_is_bounded_reuses_released_slots_and_refuses_misuse() {
    let arena = Arena::with_capacity(1 << 16);
    assert_eq!(
        SlotPool::new(&arena, 64, 0).err(),
        Some(PoolError::ZeroCapacity)
    );
    assert_eq!(
        SlotPool::new(&arena, 4, 2).err(),
        Some(PoolError::FrameSize)
    );
    assert_eq!(
        SlotPool::new(&arena, 64, 1 << 20).err(),
        Some(PoolError::ArenaExhausted),
        "a pool larger than the arena is refused rather than truncated"
    );

    let mut pool = SlotPool::new(&arena, 64, 3).expect("pool");
    assert_eq!(
        pool.stride(),
        80,
        "64 frame bytes, the 16-byte header, aligned"
    );
    let a = pool.acquire().unwrap();
    let b = pool.acquire().unwrap();
    let c = pool.acquire().unwrap();
    assert!(pool.acquire().is_none());
    let addresses = [a.as_ptr(), b.as_ptr(), c.as_ptr()];
    assert_eq!(addresses[1] as usize - addresses[0] as usize, pool.stride());
    // Acquisition clears the continuation word, so a slot never started
    // returns even though arena memory is not zeroed.
    unsafe { c.as_ptr().cast::<usize>().write(1) };
    pool.release(a).expect("an unstarted slot returns");
    let again = pool.acquire().unwrap();
    assert_eq!(again.as_ptr(), addresses[0], "the released slot is reused");
    // A non-null continuation marks a live instance.
    unsafe { b.as_ptr().cast::<usize>().write(1) };
    let (b, why) = pool.release(b).unwrap_err();
    assert_eq!(why, PoolError::Live);
    unsafe { b.as_ptr().cast::<usize>().write(0) };
    pool.release(b).unwrap();

    // A slot from another pool is refused, whatever its index.
    let other_arena = Arena::with_capacity(1 << 12);
    let mut other = SlotPool::new(&other_arena, 64, 3).unwrap();
    let foreign = other.acquire().unwrap();
    let (_, why) = pool.release(foreign).unwrap_err();
    assert_eq!(why, PoolError::Foreign);
    let (c, why) = pool.release(c).unwrap_err();
    assert_eq!(why, PoolError::Live);
    unsafe { c.as_ptr().cast::<usize>().write(0) };
    for slot in [again, c] {
        pool.release(slot).unwrap();
    }
    assert_eq!(pool.available(), pool.capacity(), "every slot is back");
}

#[test]
fn a_completed_reentrant_instance_returns_its_slot_without_a_release() {
    let src = "yield main(a: Word) -> Word { let keep = a + 100; let b = yield a; keep + b }";
    let tm = machine();
    let program = common::build(src);
    let bound = coroutine::minimum_frame_bytes(&program, &tm, &HostContracts::default()).unwrap();
    let ctx = Context::create();
    let module = coroutine::lower(&ctx, &program, &tm, bound).unwrap();
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    let entry = program.entry_point.unwrap();
    let start = unsafe {
        engine
            .get_function::<HandleStart>(&format!("kel_coroutine_{entry}_start"))
            .unwrap()
    };
    let resume = unsafe {
        engine
            .get_function::<HandleResume>(&format!("kel_coroutine_{entry}_resume"))
            .unwrap()
    };
    let arena = Arena::with_capacity(1 << 16);
    let private_len = keleusma::vm::required_persistent_capacity_for(&program)
        + region::persistent_supplement_bytes(&program) as usize;
    let s = arena_buffer(&arena, program.shared_data_bytes as usize);
    let p = arena_buffer(&arena, private_len);
    let r = arena_buffer(
        &arena,
        region::host_arena_supplement_bytes(&program) as usize,
    );
    let mut pool = SlotPool::new(&arena, bound, 1).unwrap();
    let slot = pool.acquire().unwrap();
    let first = unsafe { start.call(5, s, p, r, slot.as_ptr()) };
    assert_eq!(
        first,
        Outcome {
            status: 1,
            value: 5
        }
    );
    let done = unsafe { resume.call(slot.as_ptr(), 7) };
    assert_eq!(
        done,
        Outcome {
            status: 2,
            value: 112
        },
        "keep + b, as the VM computes"
    );
    pool.release(slot)
        .expect("a completed instance needs no release");
    assert_eq!(pool.available(), 1);
}

/// The bound over the self-hosted stage sources, the programs Order 1 drives.
/// Each stage's entry is a stream, and every one that the coroutine route
/// admits gets a derived bound. A refusal is reported with its reason rather
/// than skipped, and at least one stage must be admitted for this to say
/// anything.
#[test]
fn every_admitted_stage_source_has_a_static_frame_bound() {
    let tm = machine();
    let mut admitted = 0;
    let mut paths: Vec<_> = std::fs::read_dir("../src/selfhost/kel")
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();
    for path in &paths {
        let src = std::fs::read_to_string(path).unwrap();
        let program = common::build(&src);
        match coroutine::minimum_frame_bytes(&program, &tm, &HostContracts::default()) {
            Ok(bound) => {
                admitted += 1;
                println!(
                    "{:24} frame bound {bound} bytes",
                    path.file_name().unwrap().to_string_lossy()
                );
            }
            Err(e) => println!(
                "{:24} not admitted by the coroutine route: {e:?}",
                path.file_name().unwrap().to_string_lossy()
            ),
        }
    }
    assert_eq!(
        admitted,
        paths.len(),
        "every stage source was admitted when this was written; a refusal printed above is new"
    );
    assert!(
        paths.len() >= 14,
        "only {} stage sources were found",
        paths.len()
    );
}

/// The largest stage frame, `lexer.kel`'s, in pooled slots: three instances lex
/// three different sources in interleaved turns, each against its own run on
/// the virtual machine, yields and shared writes both.
#[test]
fn pooled_lexer_instances_agree_with_the_vm_on_different_sources() {
    use keleusma::bytecode::{SlotVisibility, Value};
    use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
    const TEXTS: &[&[u8]] = &[
        b"fn add(x: Word, y: Word) -> Word { x + y }",
        b"loop main(t: Word) -> Word { if t > 10 { yield t * 2 } else { yield 0 } } // c",
        b"struct P { a: Word } fn f(p: P) -> Word { p.a >= 3 }",
    ];
    const TURNS: i64 = 300;
    let tm = machine();
    let program = common::build(&std::fs::read_to_string("../src/selfhost/kel/lexer.kel").unwrap());
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
    let shared_len = program.shared_data_bytes as usize;
    let seed = |text: &[u8]| {
        let mut seed = vec![0u8; shared_len];
        let (len, bytes) = (offset("len"), offset("bytes"));
        seed[len..len + 8].copy_from_slice(&(text.len() as i64).to_le_bytes());
        seed[bytes..bytes + text.len()].copy_from_slice(text);
        seed
    };

    // The reference, one virtual machine per source.
    let need = required_persistent_capacity_for(&program);
    let expected: Vec<(Vec<i64>, Vec<u8>)> = TEXTS
        .iter()
        .map(|text| {
            let mut arena = keleusma::Arena::with_capacity(
                auto_arena_capacity_for(&program, &[]).unwrap() + need + 65536,
            );
            arena.resize_persistent(need).unwrap();
            let mut vm = Vm::new(program.clone(), &arena).unwrap();
            let mut shared = seed(text);
            let mut st = vm.call_with_shared(&mut shared, &[Value::Int(0)]).unwrap();
            let mut ys = Vec::new();
            for t in 1..=TURNS {
                let VmState::Yielded(Value::Int(v)) = st else {
                    panic!("{st:?}")
                };
                ys.push(v);
                if t < TURNS {
                    st = vm.resume_with_shared(&mut shared, Value::Int(t)).unwrap();
                    if matches!(st, VmState::Reset) {
                        st = vm.resume_with_shared(&mut shared, Value::Int(t)).unwrap();
                    }
                }
            }
            (ys, shared)
        })
        .collect();

    let bound = coroutine::minimum_frame_bytes(&program, &tm, &HostContracts::default()).unwrap();
    let ctx = Context::create();
    let module = coroutine::lower(&ctx, &program, &tm, bound).unwrap();
    let engine = module
        .create_jit_execution_engine(OptimizationLevel::None)
        .unwrap();
    let entry = program.entry_point.unwrap();
    let start =
        unsafe { engine.get_function::<HandleStart>(&format!("kel_coroutine_{entry}_start")) }
            .unwrap();
    let resume =
        unsafe { engine.get_function::<HandleResume>(&format!("kel_coroutine_{entry}_resume")) }
            .unwrap();
    let release =
        unsafe { engine.get_function::<HandleRelease>(&format!("kel_coroutine_{entry}_release")) }
            .unwrap();

    let private_len = need + region::persistent_supplement_bytes(&program) as usize;
    let region_len = region::host_arena_supplement_bytes(&program) as usize;
    let arena = Arena::with_capacity(
        TEXTS.len() * (shared_len + private_len + region_len + 64) + (1 << 16),
    );
    let buffers: Vec<[*mut u8; 3]> = TEXTS
        .iter()
        .map(|text| {
            let b = [
                arena_buffer(&arena, shared_len),
                arena_buffer(&arena, private_len),
                arena_buffer(&arena, region_len),
            ];
            unsafe { std::slice::from_raw_parts_mut(b[0], shared_len) }
                .copy_from_slice(&seed(text));
            common::install_private_init_bytes(&program, unsafe {
                std::slice::from_raw_parts_mut(b[1], private_len)
            });
            b
        })
        .collect();
    let mut pool = SlotPool::new(&arena, bound, TEXTS.len() as u32).unwrap();
    let settled = arena.bottom_used();
    let mut seen = vec![Vec::new(); TEXTS.len()];
    let slots: Vec<_> = buffers
        .iter()
        .enumerate()
        .map(|(i, [s, p, r])| {
            let slot = pool.acquire().unwrap();
            let out = unsafe { start.call(0, *s, *p, *r, slot.as_ptr()) };
            assert_eq!(out.status, 1);
            seen[i].push(out.value);
            slot
        })
        .collect();
    for t in 1..TURNS {
        for (i, slot) in slots.iter().enumerate() {
            let out = unsafe { resume.call(slot.as_ptr(), t) };
            assert_eq!(out.status, 1);
            seen[i].push(out.value);
        }
    }
    assert_eq!(
        arena.bottom_used(),
        settled,
        "the lexer instances used arena memory after start"
    );
    for (i, (ys, shared)) in expected.iter().enumerate() {
        assert_eq!(&seen[i], ys, "instance {i}: token yields");
        let written = unsafe { std::slice::from_raw_parts(buffers[i][0], shared_len) };
        assert_eq!(written, &shared[..], "instance {i}: shared writes");
        let codes: std::collections::BTreeSet<_> = ys.iter().map(|v| v & 255).collect();
        assert!(codes.len() >= 8, "instance {i} lexed vacuously: {codes:?}");
    }
    assert_ne!(
        expected[0].0, expected[1].0,
        "the sources must lex differently"
    );
    for slot in slots {
        unsafe { release.call(slot.as_ptr()) };
        pool.release(slot).unwrap();
    }
    println!(
        "lexer.kel: {} pooled instances at a {bound}-byte frame bound",
        TEXTS.len()
    );
}
