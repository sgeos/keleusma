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
    DEFAULT_ARENA_CAPACITY, Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for,
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

#[test]
fn an_element_above_slot_65535_survives_reset_at_its_own_offset() {
    let src = "struct P { x: Word, y: Word }\n\
               private data d { a: [P; 70000] }\n\
               loop main(seed: Word) -> Word { \
                   if seed == 0 { \
                       d.a[69999] = P { x: 3, y: 4 }; \
                       d.a[65536] = P { x: 5, y: 6 }; \
                   }; \
                   let _ = yield d.a[69999].x + d.a[69999].y + d.a[65536].x + d.a[65536].y; \
                   0 \
               }";
    let m = compile(src).unwrap_or_else(|e| panic!("{e}"));
    let need = required_persistent_capacity_for(&m);
    // Loading materialises each element's zero in the working region before
    // persisting it, and `auto_arena_capacity_for` does not count that
    // load-time work, measured on 2026-11-02 to exhaust 3,425,536 bytes for
    // seventy thousand two-word composites. The region is therefore sized here
    // with room for every element. That gap is recorded as a residual in the
    // array wire-slot design record rather than closed by this test.
    let working = auto_arena_capacity_for(&m, &[])
        .expect("arena bound")
        .max(DEFAULT_ARENA_CAPACITY)
        + 70_000 * 128;
    let mut arena = Arena::with_capacity(working + need);
    arena.resize_persistent(need).expect("resize_persistent");
    let mut vm = Vm::new(m, &arena).expect("verify");
    let want = 3 + 4 + 5 + 6;
    match vm.call(&[Value::Int(0)]).expect("call") {
        VmState::Yielded(Value::Int(n)) => assert_eq!(n, want),
        other => panic!("iteration 1 expected Yielded({want}), got {other:?}"),
    }
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Reset => {}
        other => panic!("expected Reset at the loop body end, got {other:?}"),
    }
    // No rewrite on this iteration, so the reads observe the bodies that
    // survived the RESET in the persistent pool.
    match vm.resume(Value::Int(1)).expect("resume") {
        VmState::Yielded(Value::Int(n)) => {
            assert_eq!(n, want, "elements above slot 65,535 must survive RESET")
        }
        other => panic!("iteration 2 expected Yielded({want}), got {other:?}"),
    }
}
