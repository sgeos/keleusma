//! Library surface for the self-hosted Keleusma compiler driver.
//!
//! The reusable bootstrap pipeline (drive the self-hosted `lexer.kel`, `parse.kel`,
//! `reconstruct.kel`, `codegen.kel`, and `analyze.kel` stages over a source and assemble
//! a `Module`, plus the `verify_*.kel` drivers) now lives canonically in the parent
//! `keleusma` crate at `keleusma::selfhost` (behind its `self-host` feature), which also
//! embeds the stage sources. This subproject re-exports it so the `keleusma-selfhost`
//! binary's `compile` command and the driver-level tests under `tests/` keep resolving
//! `keleusma_selfhost::selfhost::*` unchanged.

pub mod selfhost {
    pub use keleusma::selfhost::*;
}
