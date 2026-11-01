//! **THE SYMBOL EXAMPLES IN THE EMITTER'S DOC COMMENT MATCH WHAT IT PRODUCES.**
//!
//! # Why this needs a guard rather than care
//!
//! `native_symbol` maps every character outside `[A-Za-z0-9_]` to `_`, one for one,
//! so `host::play` becomes `kel_native_host__play` with TWO underscores. **Its doc
//! comment gave one**, and stated the rule correctly in the next sentence — a
//! comment contradicting itself, where the example is the half a reader copies.
//!
//! **The cost of copying the wrong one is a SEGFAULT, not a compile error.** A host
//! defining `kel_native_host_play` leaves the module's declaration unresolved, and
//! `native_calls.rs` records what the engine does then: it *"resolves the
//! declaration to nothing and jumps to it."*
//!
//! The working stubs in that file — `kel_native_host__one`, `__two`, `__three` —
//! were double all along, so only the prose was ever wrong. **That is precisely the
//! kind of drift a guard catches and care does not**, which is why this package
//! already applies the idiom to comment citations.

use keleusma_native::native_symbol;

/// **THE EXAMPLES ARE THE ASSERTIONS.**
#[test]
fn the_native_symbol_examples_match_the_code() {
    assert_eq!(
        native_symbol("host::play"),
        "kel_native_host__play",
        "the doc comment's example must be what the function returns; it gave a \
         single underscore until 2026-09-24"
    );

    // **THE COLLISION PAIR IS AN EXAMPLE TOO**, and the one the comment gave did
    // not collide under the real rule.
    assert_eq!(
        native_symbol("host::play"),
        native_symbol("host__play"),
        "the documented collision pair must actually collide"
    );
    assert_ne!(
        native_symbol("host::play"),
        native_symbol("host_play"),
        "`host::play` and `host_play` do NOT collide under the one-for-one rule, \
         and the comment claimed they did until 2026-09-24"
    );
}

/// **THE RULE ITSELF, so the examples above cannot be satisfied by an accident.**
#[test]
fn every_non_identifier_character_becomes_one_underscore() {
    assert_eq!(native_symbol("a"), "kel_native_a");
    assert_eq!(
        native_symbol("a_b"),
        "kel_native_a_b",
        "an underscore survives"
    );
    assert_eq!(native_symbol("a9"), "kel_native_a9", "a digit survives");
    assert_eq!(
        native_symbol("a::b::c"),
        "kel_native_a__b__c",
        "each separator character contributes its own underscore"
    );
    assert_eq!(
        native_symbol("a-b.c"),
        "kel_native_a_b_c",
        "any non-identifier character, not only a colon"
    );
}
