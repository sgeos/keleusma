//! Standard DSL libraries packaged as host-registerable bundles.
//!
//! Each bundle is a unit struct implementing the [`Library`]
//! trait. Hosts register a bundle through
//! [`crate::vm::Vm::register_library`]:
//!
//! ```ignore
//! use keleusma::stddsl;
//! vm.register_library(stddsl::Math);
//! vm.register_library(stddsl::Audio);
//! vm.register_library(stddsl::Text);
//! ```
//!
//! ## Available libraries
//!
//! - [`Math`] - pure floating-point math routines (`math::sqrt`,
//!   `math::floor`, `math::ceil`, `math::round`, `math::log2`).
//! - [`Audio`] - DSP utilities (`audio::midi_to_freq`,
//!   `audio::freq_to_midi`, `audio::db_to_linear`,
//!   `audio::linear_to_db`, `audio::clamp`).
//! - [`Text`] - text utilities (`to_string`, `length`, `concat`,
//!   `slice`, `println`). Requires the `text` cargo feature on the
//!   library; the script-side surface for text is also gated on
//!   the same feature.
//! - [`Shell`] - shell-script utilities (`shell::getenv`,
//!   `shell::run`, `shell::run_checked`, `shell::exit`). Requires
//!   the `shell` cargo feature, which adds a `std` dependency.
//!   `shell` is incompatible with the no_std build profile.
//!
//! ## Single-file scripts
//!
//! Keleusma scripts are necessarily single-file. There is no
//! `import` or `mod` mechanism inside the language; cross-script
//! reuse is intentionally outside the scope of the V0.2 surface.
//! If your application's needs grow to where you find yourself
//! wishing for modularisation, the recommended path is to roll a
//! custom DSL library: implement [`Library`] on a host-side unit
//! struct that registers the natives your scripts call, and let
//! every script consume the same vocabulary through `use`
//! declarations. The host-side library is the unit of reuse,
//! not the script.

extern crate alloc;

use crate::vm::Vm;

/// Host-registerable bundle of native functions.
///
/// A `Library` registers a related set of native functions on a
/// VM. Implementors are typically unit structs in the `stddsl`
/// module or in host crates that want to ship their own bundles.
/// The trait method takes `self` by value so unit structs can be
/// dropped after registration.
///
/// Hosts call [`Vm::register_library`] which delegates to
/// [`Library::register`]; the trait is the extensibility surface
/// for third-party bundles.
pub trait Library {
    /// Register every native function in this bundle on `vm`.
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>);
}

/// Pure floating-point math routines.
///
/// Registers `math::sqrt`, `math::floor`, `math::ceil`,
/// `math::round`, `math::log2`. Backed by `libm` so the bundle
/// works in `no_std` builds.
pub struct Math;

/// Digital signal processing utilities for audio applications.
///
/// Registers MIDI/frequency conversion, decibel/linear amplitude
/// conversion, and a clamp helper. Backed by `libm`.
pub struct Audio;

/// Text utilities. Requires the `text` cargo feature.
///
/// Registers `to_string`, `length`, `concat`, `slice`, `println`.
/// The Text bundle is a no-op when the `text` feature is
/// disabled; scripts that compile without the feature cannot
/// produce text values for these natives to operate on.
pub struct Text;

/// Shell-script utilities. Requires the `shell` cargo feature.
///
/// Registers `shell::getenv` (returns `Option<Text>`),
/// `shell::run` (returns `(Word, Text)`), `shell::run_checked`
/// (returns `Text`, traps on non-zero exit), and `shell::exit`
/// (terminates the host process).
///
/// The Shell bundle is unavailable when the `shell` feature is
/// disabled because it depends on `std::process` and `std::env`.
/// The Vm cannot be constructed in `no_std` mode with the
/// `shell` feature enabled.
pub struct Shell;

impl Library for Math {
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>) {
        math::register(vm);
    }
}

impl Library for Audio {
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>) {
        crate::audio_natives::register_audio_natives(vm);
    }
}

impl Library for Text {
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>) {
        text::register(vm);
    }
}

#[cfg(feature = "shell")]
impl Library for Shell {
    fn register<'a, 'arena>(self, vm: &mut Vm<'a, 'arena>) {
        shell::register(vm);
    }
}

mod math {
    use crate::vm::Vm;
    pub fn register<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
        vm.register_fn("math::sqrt", |x: f64| -> f64 { libm::sqrt(x) });
        vm.register_fn("math::floor", |x: f64| -> f64 { libm::floor(x) });
        vm.register_fn("math::ceil", |x: f64| -> f64 { libm::ceil(x) });
        vm.register_fn("math::round", |x: f64| -> f64 { libm::round(x) });
        vm.register_fn("math::log2", |x: f64| -> f64 { libm::log2(x) });
    }
}

mod text {
    use crate::vm::Vm;
    pub fn register<'a, 'arena>(vm: &mut Vm<'a, 'arena>) {
        // Delegate to the existing utility_natives bundle which
        // registers the arena-aware `to_string`, `concat`,
        // `slice`, `length`, and `println`. The math::* natives in
        // that bundle are duplicated by `stddsl::Math`; the second
        // registration is a no-op because `register_fn` overwrites
        // a prior registration of the same name with the same
        // body. A future refinement could split `utility_natives`
        // into text-only and math-only halves to avoid the
        // double-register, but the current arrangement keeps the
        // dependency layout unchanged.
        crate::utility_natives::register_utility_natives(vm);
    }
}

#[cfg(feature = "shell")]
pub mod shell;
