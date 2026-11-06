//! A private composite slot beyond `u16::MAX` has a pool home, and keeps it
//! across RESET.
//!
//! # The defect, and how Encoding F removed it
//!
//! The data slot index space is 24 bits, and the placement table that Encoding
//! F replaced recorded each private composite element with a `u16` slot. The
//! compiler skipped a slot above `u16::MAX` with no diagnostic, so its writes
//! had no pool offset. Measured on 2026-10-30, a private `[P; 70000]` produced
//! 65,536 records for 70,000 elements. Pull request 652 refused such a module
//! at compile time as a stopgap.
//!
//! Encoding F, which the operator chose on 2026-10-31, places a whole field
//! with one record whose `first_slot` and `count` are thirty-two bits wide, so
//! the cap is gone rather than enforced.
//!
//! # What is established
//!
//! The former boundary compiles on both sides with ONE field record, and an
//! element far above slot 65,535 is written, survives a RESET and reads back
//! at its own pool offset. The second settles the hypothesis the design record
//! left open, at the boundary that hypothesis concerned.
#![cfg(feature = "compile")]

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::vm::{
    Vm, VmError, VmState, auto_arena_capacity_for, required_persistent_capacity_for,
};

fn compile(src: &str) -> Result<keleusma::bytecode::Module, String> {
    let tokens = keleusma::lexer::tokenize(src).map_err(|e| format!("{e:?}"))?;
    let program = keleusma::parser::parse(&tokens).map_err(|e| format!("{e:?}"))?;
    keleusma::compiler::compile(&program).map_err(|e| e.message)
}

fn source(n: u32) -> String {
    format!(
        "struct P {{ x: Word }}\nprivate data d {{ a: [P; {n}] }}\n\
         fn main() -> Word {{ d.a[0] = P {{ x: 1 }}; 0 }}"
    )
}

#[test]
fn the_former_boundary_compiles_with_one_field_record() {
    for n in [65_535u32, 65_536, 65_537, 70_000] {
        let m = compile(&source(n)).unwrap_or_else(|e| panic!("[P; {n}] did not compile: {e}"));
        let layout = m.data_layout.as_ref().expect("a data layout");
        assert_eq!(layout.fields.len(), 1, "one field record for [P; {n}]");
        let f = &layout.fields[0];
        assert_eq!(f.count, n);
        assert_eq!(f.offset, Some(0));
        assert_eq!(f.stride, u32::from(f.elem_len));
        // The last element, above slot 65,535 when n exceeds 65,536, has a
        // pool home at its own offset.
        assert_eq!(
            layout.private_composite_offset(n as usize - 1),
            Some((n - 1) * f.stride)
        );
    }
}

const ABOVE_THE_CAP: &str = "struct P { x: Word, y: Word }\n\
    private data d { a: [P; 70000] }\n\
    loop main(seed: Word) -> Word { \
        if seed == 0 { \
            d.a[69999] = P { x: 3, y: 4 }; \
            d.a[65536] = P { x: 5, y: 6 }; \
        }; \
        let _ = yield d.a[69999].x + d.a[69999].y + d.a[65536].x + d.a[65536].y; \
        0 \
    }";

/// Construct, run two iterations across a RESET, and return the sum each read.
/// `ephemeral` is the working region after the persistent one.
fn run_twice(m: &keleusma::bytecode::Module, ephemeral: usize) -> Result<(i64, i64), VmError> {
    let need = required_persistent_capacity_for(m);
    let mut arena = Arena::with_capacity(ephemeral + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m.clone(), &arena)?;
    let first = match vm.call(&[Value::Int(0)])? {
        VmState::Yielded(Value::Int(n)) => n,
        other => panic!("iteration 1 yielded {other:?}"),
    };
    match vm.resume(Value::Int(1))? {
        VmState::Reset => {}
        other => panic!("expected Reset at the loop body end, got {other:?}"),
    }
    // No rewrite on this iteration, so the reads observe the bodies that
    // survived the RESET in the persistent pool.
    let second = match vm.resume(Value::Int(1))? {
        VmState::Yielded(Value::Int(n)) => n,
        other => panic!("iteration 2 yielded {other:?}"),
    };
    Ok((first, second))
}

#[test]
fn an_element_above_slot_65535_survives_reset_at_its_own_offset() {
    let m = compile(ABOVE_THE_CAP).unwrap_or_else(|e| panic!("{e}"));
    let want = 3 + 4 + 5 + 6;
    // Sized by the module's own bound, which counts the load-time seeding of
    // every element's zero.
    let figure = auto_arena_capacity_for(&m, &[]).expect("arena bound");
    assert_eq!(
        run_twice(&m, figure).expect("the bound suffices"),
        (want, want),
        "elements above slot 65,535 must survive RESET"
    );
}

/// The bound is tight: one byte less runs out of arena. Measured the way
/// `tests/native_region_supplement.rs` measures, by the smallest region that
/// runs and the region one byte smaller.
#[test]
fn the_bound_for_seventy_thousand_seeded_composites_is_tight() {
    let m = compile(ABOVE_THE_CAP).unwrap_or_else(|e| panic!("{e}"));
    let figure = auto_arena_capacity_for(&m, &[]).expect("arena bound");
    let terms = keleusma::vm::vm_arena_terms_for(&m, &[], 0).expect("arena bound");
    assert_eq!(
        terms.seeded_private_bytes,
        70_000 * 16,
        "one 16-byte body per element"
    );
    // Measured on 2026-11-02, 1,120,000 bytes of seeded bodies and 440 of the
    // other terms. The figure runs and one byte less does not.
    assert_eq!(figure, 1_120_440);
    match run_twice(&m, figure - 1) {
        Err(VmError::OutOfArena(_)) => {}
        other => panic!("one byte below the bound must run out, got {other:?}"),
    }
}

/// Packed private storage, S1, keeps a private array of composites in its pool
/// bodies alone, so the persistent figure is the pool. Before packing it was
/// 70,000 `Value` handles of 32 bytes beside the pool, 3,360,000 bytes. The
/// figure runs and one byte less is refused at construction.
#[test]
fn the_persistent_figure_for_seventy_thousand_packed_composites_is_tight() {
    let m = compile(ABOVE_THE_CAP).unwrap_or_else(|e| panic!("{e}"));
    let need = required_persistent_capacity_for(&m);
    assert_eq!(need, 70_000 * 16);
    assert_eq!(m.private_data_bytes, 0, "no Value slot remains");
    let figure = auto_arena_capacity_for(&m, &[]).expect("arena bound");
    let construct = |persistent: usize| {
        let mut arena = Arena::with_capacity(figure + persistent);
        arena
            .resize_persistent(persistent)
            .expect("resize_persistent");
        Vm::new(m.clone(), &arena).map(|_| ())
    };
    assert!(construct(need).is_ok());
    match construct(need - 1) {
        Err(VmError::VerifyError(message)) => {
            assert!(message.contains("persistent_capacity"), "{message}")
        }
        other => panic!("one byte below the persistent figure must refuse, got {other:?}"),
    }
}
