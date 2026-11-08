//! Kani bounded model checking harnesses for the Keleusma verifier core and
//! for the keleusma-wire load-time trust path.
//!
//! Every harness reaches the code under check through the public application
//! programming interface of `keleusma` or `keleusma-wire` only. Nothing here is
//! compiled outside `cargo kani`, because every item is gated on `cfg(kani)`.
//!
//! The harnesses fall into three groups. The passing harnesses are counted by
//! `check.sh` and must all verify. The finding harnesses reproduce a real
//! defect in the code under check and must all fail. The negative control
//! states a property known to be false and must fail, which shows that a green
//! run of the passing group is evidence and not an artefact of the toolchain.
//! `README.md` lists each harness with its property, bounds and result.

#![no_std]

#[cfg(kani)]
extern crate alloc;

#[cfg(kani)]
mod generators;

#[cfg(kani)]
mod verifier_harnesses;

#[cfg(kani)]
mod wire_harnesses;
