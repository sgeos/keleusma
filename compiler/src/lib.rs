//! Library surface for the self-hosted Keleusma compiler driver.
//!
//! The bootstrap pipeline (drive the self-hosted `lexer.kel`, `parse.kel`, and
//! `codegen.kel` stages over a source and assemble a `Module`) lives in
//! [`selfhost`], shared by the `keleusma-selfhost` binary's `compile` command and
//! the driver-level fixed-point tests under `tests/`.

pub mod selfhost;
