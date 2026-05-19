//! Keleusma RTOS microkernel.
//!
//! Draft scaffold for the cooperative-scheduling, Keleusma-
//! script-driven RTOS described in `tmp/RTOS_MICROKERNEL_SPEC.md`.
//! The kernel core is `no_std + alloc` so it can be lifted onto
//! bare-metal targets unchanged. Per-platform implementations
//! live in `src/platform/<name>.rs`, each gated by a cargo
//! feature.
//!
//! The crate is built against `std` when the `std-platform`
//! feature is active (which transparently re-exports `alloc`
//! through `std`); otherwise it is built against `no_std +
//! alloc`. Embedded targets disable `std-platform` and enable
//! the appropriate per-board feature instead.

#![cfg_attr(not(feature = "std-platform"), no_std)]

extern crate alloc;

pub mod kernel;
pub mod natives;
pub mod platform;
pub mod setup;

pub use kernel::{Kernel, Task, TaskState, WakeReason, YieldReason};
pub use platform::{Platform, PlatformResources};

#[cfg(feature = "std-platform")]
pub use platform::StdPlatform;

#[cfg(feature = "stm32n6570dk-platform")]
pub use platform::Stm32N6570DkPlatform;
