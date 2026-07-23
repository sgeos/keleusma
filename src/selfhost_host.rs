//! Shared host-side driver for the self-hosted `.kel` stages.
//!
//! The loop that drives `parse.kel` and reads its emitted records was historically copied
//! into every consumer — the self-host integration tests and the detached `compiler/`
//! subproject — six times over. That duplication is a drift hazard: it is the same class
//! that let the `compiler/src/selfhost.rs` decoder fall behind and ship `unknown op tag 62`
//! into v0.2.3 (process audit item 4). This module holds ONE copy of the record-reading
//! loop, so a change to the parse-to-host record transport — notably the P11 Option E
//! two-word `(tag, payload)` encoding — is a single edit here rather than six lockstep ones.
//!
//! Gated behind `compile` + `verify` (the features the self-host path already requires); it
//! is a harness helper for the self-host tooling, not part of the stable runtime API.

use crate::bytecode::Value;
use crate::vm::{Vm, VmState};
use core::ops::ControlFlow;

/// Drive a running `parse.kel` coroutine, invoking `on_record(code, val)` for each record it
/// emits until the callback returns [`ControlFlow::Break`] — which the caller signals on the
/// DONE record — or until `budget` iterations elapse (a divergence guard; `parse.kel` must
/// reach DONE first).
///
/// `state` is the coroutine state immediately after the caller's `call_with_shared`. This
/// function owns the record transport: today each record is one yielded word `code + val*64`;
/// the P11 Option E change to a two-word `(tag, payload)` pair lands here and nowhere else.
///
/// The caller keeps ownership of its accumulator state by capturing it in `on_record`; this
/// driver borrows only `vm` and `shared`, so a callback must not itself touch them.
pub fn drive_parse_records<F>(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    state: VmState,
    budget: usize,
    mut on_record: F,
) where
    F: FnMut(i64, i64) -> ControlFlow<()>,
{
    let mut state = state;
    for _ in 0..budget {
        if let VmState::Yielded(Value::Int(w)) = state {
            let (code, val) = (w.rem_euclid(64), w.div_euclid(64));
            if on_record(code, val).is_break() {
                return;
            }
        }
        state = vm
            .resume_with_shared(shared, Value::Int(0))
            .expect("resume parse.kel");
    }
    panic!("parse.kel did not reach DONE within its iteration budget");
}
