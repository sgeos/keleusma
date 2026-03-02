# Standard Library

> **Navigation**: [Design](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma has no built-in standard library. All domain functionality is provided by host-registered native functions. However, the crate includes a convenience module called `audio_natives` that bundles commonly useful audio and math functions. Hosts can register the entire set with a single call or register individual functions selectively.

## Registration API

The `register_audio_natives` function registers all built-in audio and math native functions with a virtual machine instance.

```rust
pub fn register_audio_natives(vm: &mut Vm)
```

Individual registration is also possible using the two native function registration methods on the VM.

```rust
vm.register_native("math::sin", math_sin);
vm.register_native_closure("custom::lookup", Box::new(move |args| { /* ... */ }));
```

The first method accepts a function pointer for stateless functions. The second accepts a boxed closure for functions that capture external state.

## Audio Functions

Audio functions are registered under the `audio::` namespace. They provide standard conversions between musical and signal processing representations.

| Function | Signature | Description |
|----------|-----------|-------------|
| `audio::midi_to_freq` | `(i64) -> f64` | Convert a MIDI note number to a frequency in Hz, using A4 = 440 Hz as the reference pitch |
| `audio::freq_to_midi` | `(f64) -> i64` | Convert a frequency in Hz to the nearest MIDI note number |
| `audio::db_to_linear` | `(f64) -> f64` | Convert a decibel value to linear amplitude using the formula 10^(db/20) |
| `audio::linear_to_db` | `(f64) -> f64` | Convert a linear amplitude to decibels using the formula 20*log10(linear) |

## Math Functions

Math functions are registered under the `math::` namespace. They provide standard mathematical operations for numeric processing.

| Function | Signature | Description |
|----------|-----------|-------------|
| `math::clamp` | `(f64, f64, f64) -> f64` | Clamp a value to the range [min, max] |
| `math::lerp` | `(f64, f64, f64) -> f64` | Linear interpolation computed as a + (b - a) * t |
| `math::sin` | `(f64) -> f64` | Sine of a value in radians |
| `math::cos` | `(f64) -> f64` | Cosine of a value in radians |
| `math::pow` | `(f64, f64) -> f64` | Raise a base to an exponent |
| `math::abs` | `(f64) -> f64` | Absolute value |
| `math::min` | `(f64, f64) -> f64` | Return the smaller of two values |
| `math::max` | `(f64, f64) -> f64` | Return the larger of two values |

## Type Flexibility

All math and audio functions accept both `Int` and `Float` arguments. When an integer argument is provided where a floating-point parameter is expected, the native function boundary performs automatic widening from `i64` to `f64`. This allows scripts to call `math::sin(1)` without an explicit cast, reducing boilerplate in common usage patterns.

This widening behavior is specific to the native function boundary and does not affect the language type system itself, which remains strict about implicit coercion in all other contexts.

## Implementation Notes

All math operations use the `libm` crate to provide portable floating-point math functions without depending on the Rust standard library. This ensures compatibility with `no_std+alloc` environments.

All functions in the audio and math namespaces are pure. They produce no side effects and return deterministic results for the same inputs. The host declares these functions as pure at registration time, allowing the compiler to treat them accordingly in future optimization passes.
