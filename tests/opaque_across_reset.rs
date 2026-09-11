#![cfg(all(feature = "compile", feature = "verify"))]
//! A HOST-SUPPLIED OPAQUE HANDLE HELD ACROSS A RESET.
//!
//! # The question, and where it comes from
//!
//! `docs/decisions/INVALID_BYTECODE_CENSUS.md` group G covers arena staleness
//! after reset, three sites, verdict "no witness found (1 of 3 probed)". Its
//! own text says the other two "concern host-supplied opaque handles going
//! stale, which is a different question and untested here", and the closing
//! section names group G among what remains.
//!
//! This asks that question. An opaque is a registry index inside a composite
//! body; a private `data` slot's body survives RESET in the persistent region.
//! So a composite bearing an opaque, written on one iteration and read on the
//! next, crosses a RESET with an index in it. Either the index still resolves,
//! or it does not and the runtime says so, or the compiler refuses to let the
//! situation arise.
//!
//! **All three outcomes are informative and none is assumed here.** What would
//! NOT be informative is a test written to confirm a guess.

extern crate alloc;

use alloc::string::String;
use keleusma::bytecode::{GenericValue, Value};
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, HostOpaque, KeleusmaType, host_arc};

struct Handle {
    /// Read by nothing here; kept so the handle is distinguishable in a
    /// debugger from a default-constructed one.
    #[allow(dead_code)]
    label: String,
}
impl HostOpaque for Handle {
    fn type_name(&self) -> &'static str {
        "Handle"
    }
}

/// A composite bearing an opaque, stored in a persistent slot on the first
/// iteration and read back on the second, after the loop body's RESET.
const ACROSS_RESET: &str = "use make_handle() -> Handle\n\
                            use handle_val(Handle) -> Word\n\
                            struct P { h: Handle, n: Word }\n\
                            private data d { p: P }\n\
                            loop main(seed: Word) -> Word { \
                                if seed == 0 { d.p = P { h: make_handle(), n: 7 }; }; \
                                let _ = yield handle_val(d.p.h) + d.p.n; \
                                0 \
                            }";

/// **MEASURED: THE ROUTE IS CLOSED AT COMPILE TIME.**
///
/// The outcome was not guessed. The probe admitted three shapes -- resolve,
/// fault, or compile refusal -- and the answer is the third: an opaque cannot
/// be a field of a data segment at all, so no opaque index ever reaches the
/// persistent region and none can cross a RESET there.
///
/// The message is asserted, not merely the refusal, because "refused" alone
/// would also be satisfied by a refusal for some unrelated reason, and this
/// test would then report a closed route that is open.
#[test]
fn an_opaque_cannot_be_a_persistent_data_field_at_all() {
    let parsed = parse(&tokenize(ACROSS_RESET).expect("lex")).expect("parse");
    let err = compile(&parsed).expect_err(
        "an opaque in a `private data` field now COMPILES. That opens a route by which an \
         opaque index reaches the persistent region and crosses a RESET, which is exactly \
         group G's remaining unprobed question. The rest of this file assumes the route is \
         closed; re-open the probe before trusting it",
    );
    assert!(
        err.message
            .contains("opaque types are not yet admissible in data segment fields"),
        "the refusal is for a different reason than the one that closes this route, so the \
         route may be open for the shapes that reason does not cover: {}",
        err.message
    );
}

/// The OTHER route, and the one the runtime documents rather than forbids.
///
/// A yielded value stays arena-resident, and `src/vm.rs` states that the host
/// must decode it before the next `resume()`, which RESETs the arena, because
/// "a read afterward resolves to a clean stale error".
///
/// **That sentence is a claim about a host-facing contract, and nothing checked
/// it.** Here the host does exactly the forbidden thing: yields a composite
/// bearing an opaque, resumes, and only then decodes. What matters is not that
/// it fails but HOW: a clean error is the documented behaviour, whereas a wrong
/// value or a resolve to some other handle would be a silent hazard in a
/// contract the host is expected to honour by reading the documentation.
#[test]
fn decoding_a_yielded_opaque_after_the_next_resume() {
    const YIELDS_OPAQUE: &str = "use make_handle() -> Handle\n\
                                 struct P { h: Handle, n: Word }\n\
                                 loop main(seed: Word) -> P { \
                                     let _ = yield P { h: make_handle(), n: 7 }; \
                                     P { h: make_handle(), n: 0 } \
                                 }";
    let parsed = parse(&tokenize(YIELDS_OPAQUE).expect("lex")).expect("parse");
    let Ok(m) = compile(&parsed) else {
        // Recorded, not worked around: if this shape is refused too, the whole
        // host-facing route is closed by the compiler and group G's remaining
        // sites are unreachable from a supported producer.
        let e = compile(&parsed).unwrap_err();
        panic!(
            "yielding a composite bearing an opaque is REFUSED at compile time: {}. That is a \
             stronger statement than this file records; update the census",
            e.message
        );
    };
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(m, &arena).expect("verify");
    vm.register_native("make_handle", |_args| {
        Ok(GenericValue::Opaque(host_arc(Handle {
            label: String::from("yielded"),
        })))
    });

    let held = match vm.call(&[Value::Int(0)]).expect("first call") {
        VmState::Yielded(v) => v,
        other => panic!("expected a yield, got {other:?}"),
    };

    // The host does NOT decode here, which is what the contract forbids.
    let _ = vm.resume(Value::Int(1)).expect("resume past the yield");

    // Now read the value it was supposed to have consumed already.
    let late: Result<HeldP, _> = vm.decode(&held);
    match late {
        Err(e) => {
            // **THE VARIANT IS THE CENSUS-RELEVANT PART.** Measured, this is a
            // `TypeError` naming the read-before-resume contract, NOT an
            // `InvalidBytecode`. That matters because group G is a group of
            // `InvalidBytecode` sites: a clean contract error here is evidence
            // that this route does not reach them, where a bare "it failed"
            // would have been evidence of nothing.
            assert!(
                !matches!(e, keleusma::vm::VmError::InvalidBytecode(_)),
                "the late read raised InvalidBytecode, which is a WITNESS for group G's \
                 remaining two sites. Record it in the census rather than adjusting this test: \
                 {e:?}"
            );
            assert!(
                alloc::format!("{e:?}").contains("read-before-resume"),
                "the late read failed for some reason other than the documented contract, so \
                 this test no longer establishes what it claims: {e:?}"
            );
        }
        Ok(p) => panic!(
            "decoding after the reset SUCCEEDED, returning n = {}. The runtime documents this \
             read as resolving to a clean stale error, so either the contract is stricter than \
             it needs to be or this read is returning memory the reset released",
            p.n
        ),
    }
}

#[derive(KeleusmaType)]
struct HeldP {
    h: alloc::sync::Arc<dyn HostOpaque>,
    n: i64,
}
