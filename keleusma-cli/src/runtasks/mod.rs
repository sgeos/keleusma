//! Multi-script runner implementing `keleusma run-tasks <manifest.toml>`.
//!
//! The runner lifts the cooperative scheduler from `examples/rtos/` onto
//! the desktop. See `docs/architecture/RUN_TASKS.md` for the design
//! contract this module implements. Operators deploying the runner under
//! a service supervisor on Linux, FreeBSD, OpenBSD, macOS, or Windows
//! should consult that document for per-platform recipes.

mod manifest;
mod scheduler;
mod signals;

pub use scheduler::run;
