# Standard Library

> **Navigation**: [Design](./README.md) | [Documentation Root](../README.md)

## Overview

Keleusma has no built-in standard library. All domain functionality is provided by host-registered native functions. The runtime crate ships several host-registerable bundles in the `keleusma::stddsl` module. Each bundle is a unit struct implementing the `Library` trait and is registered through `Vm::register_library`.

The available bundles are `Math`, `Audio`, `Text`, and `Shell`. Bundle membership is partitioned by namespace. The `Math` bundle owns the `math::` namespace, and the `Audio` bundle owns the `audio::` namespace. A host script that needs both math and audio helpers should register both bundles.

## Registration API

```rust
use keleusma::stddsl;
vm.register_library(stddsl::Math);
vm.register_library(stddsl::Audio);
vm.register_library(stddsl::Text);
```

Individual native registration is also possible.

```rust
vm.register_fn("custom::scale", |x: f64, k: f64| -> f64 { x * k });
vm.register_native_closure("custom::lookup", Box::new(move |_, args| { Ok(args[0].clone()) }));
```

The `register_fn` and `register_fn_fallible` methods accept ordinary Rust closures with automatic argument and return-value marshalling. The `register_native_closure` method accepts a boxed closure for natives that capture external state.

## Math Bundle

The `Math` bundle registers entries under the `math::` namespace. All routines are pure, deterministic, and backed by the `libm` crate so the bundle works in `no_std` builds. Named constants are exposed as zero-argument functions sourced from `core::f64::consts`.

### Algebraic and rounding routines

| Function | Signature | Description |
|---|---|---|
| `math::sqrt` | `(Float) -> Float` | Square root. |
| `math::pow` | `(Float, Float) -> Float` | Raise the first argument to the power of the second. |
| `math::abs` | `(Float) -> Float` | Absolute value. |
| `math::sign` | `(Float) -> Float` | Returns -1, 0, or 1. Propagates NaN. |
| `math::floor` | `(Float) -> Float` | Round toward negative infinity. |
| `math::ceil` | `(Float) -> Float` | Round toward positive infinity. |
| `math::round` | `(Float) -> Float` | Round to the nearest integer, ties away from zero. |
| `math::trunc` | `(Float) -> Float` | Round toward zero. |
| `math::fmod` | `(Float, Float) -> Float` | Floating-point remainder. Traps when the divisor is zero. |
| `math::hypot` | `(Float, Float) -> Float` | Numerically stable square root of the sum of squares. |
| `math::min` | `(Float, Float) -> Float` | Smaller of two values. |
| `math::max` | `(Float, Float) -> Float` | Larger of two values. |
| `math::clamp` | `(Float, Float, Float) -> Float` | Clamp the first argument to the closed interval defined by the second and third. |
| `math::lerp` | `(Float, Float, Float) -> Float` | Linear interpolation computed as `a + (b - a) * t`. |

### Trigonometric routines

| Function | Signature | Description |
|---|---|---|
| `math::sin` | `(Float) -> Float` | Sine of an angle in radians. |
| `math::cos` | `(Float) -> Float` | Cosine of an angle in radians. |
| `math::tan` | `(Float) -> Float` | Tangent of an angle in radians. |
| `math::asin` | `(Float) -> Float` | Inverse sine. Traps when the argument lies outside the interval from negative one to one. |
| `math::acos` | `(Float) -> Float` | Inverse cosine. Traps when the argument lies outside the interval from negative one to one. |
| `math::atan` | `(Float) -> Float` | Inverse tangent. |
| `math::atan2` | `(Float, Float) -> Float` | Two-argument inverse tangent computed as `atan2(y, x)`. |
| `math::tanh` | `(Float) -> Float` | Hyperbolic tangent. Useful as a soft-clip primitive in audio waveshapers. |

### Exponential and logarithmic routines

| Function | Signature | Description |
|---|---|---|
| `math::exp` | `(Float) -> Float` | Natural exponential. |
| `math::ln` | `(Float) -> Float` | Natural logarithm. Traps when the argument is not strictly positive. |
| `math::log10` | `(Float) -> Float` | Common logarithm. Traps when the argument is not strictly positive. |
| `math::log2` | `(Float) -> Float` | Binary logarithm. Traps when the argument is not strictly positive. |

### Named constants

Constants are exposed as zero-argument functions returning the corresponding `Float` value. The pi, tau, and e accessors cover the canonical universal constants. The remaining accessors support common change-of-base and diagonal-norm idioms.

| Function | Returns |
|---|---|
| `math::pi` | The ratio of a circle's circumference to its diameter. |
| `math::tau` | Two times pi. |
| `math::e` | The base of the natural logarithm. |
| `math::sqrt_2` | The square root of two. |
| `math::ln_2` | The natural logarithm of two. |
| `math::ln_10` | The natural logarithm of ten. |

## Audio Bundle

The `Audio` bundle registers entries under the `audio::` namespace. All routines are pure, deterministic, and backed by the `libm` crate. The bundle does not register entries under the `math::` namespace. A host that needs math helpers alongside audio helpers should also register `Math`.

### Pitch conversion

| Function | Signature | Description |
|---|---|---|
| `audio::midi_to_freq` | `(Word) -> Float` | Convert a MIDI note number to frequency in hertz using A4 equal to 440 Hz. |
| `audio::freq_to_midi` | `(Float) -> Word` | Convert a frequency in hertz to the nearest MIDI note number. Traps when the input is not positive. |
| `audio::cents_to_ratio` | `(Float) -> Float` | Convert cents to a frequency ratio. One thousand two hundred cents equals one octave. |
| `audio::ratio_to_cents` | `(Float) -> Float` | Convert a frequency ratio to cents. Traps when the input is not strictly positive. |
| `audio::semitones_to_ratio` | `(Float) -> Float` | Convert semitones to a frequency ratio. Twelve semitones equals one octave. |
| `audio::ratio_to_semitones` | `(Float) -> Float` | Convert a frequency ratio to semitones. Traps when the input is not strictly positive. |

### Amplitude conversion

| Function | Signature | Description |
|---|---|---|
| `audio::db_to_linear` | `(Float) -> Float` | Convert a decibel value to linear amplitude using the formula `10^(db/20)`. |
| `audio::linear_to_db` | `(Float) -> Float` | Convert a linear amplitude to decibels using the formula `20 * log10(linear)`. Traps when the input is not strictly positive. |

### Time conversion

| Function | Signature | Description |
|---|---|---|
| `audio::ms_to_samples` | `(Float, Float) -> Float` | Convert a duration in milliseconds to a sample count at the given sample rate. Traps when the sample rate is not strictly positive. |
| `audio::samples_to_ms` | `(Float, Float) -> Float` | Convert a sample count at the given sample rate to a duration in milliseconds. Traps when the sample rate is not strictly positive. |

### Filter coefficient helpers

| Function | Signature | Description |
|---|---|---|
| `audio::onepole_lpf_alpha` | `(Float, Float) -> Float` | Compute the one-pole low-pass coefficient `1 - exp(-2*pi*cutoff/sample_rate)` for the recurrence `y = y_prev + alpha * (x - y_prev)`. Traps when the sample rate is not strictly positive or the cutoff is negative. |
| `audio::onepole_hpf_alpha` | `(Float, Float) -> Float` | Compute the complementary one-pole high-pass decay coefficient `exp(-2*pi*cutoff/sample_rate)`. The high-pass output is `x - lpf` where `lpf` is the underlying one-pole's output. Same trap conditions as `onepole_lpf_alpha`. |

### Spatial helper

| Function | Signature | Description |
|---|---|---|
| `audio::pan_law` | `(Float) -> (Float, Float)` | Equal-power pan law. Position in the closed interval from negative one through positive one maps to a left-and-right gain pair whose sum of squares equals one. Out-of-range positions are clamped. Negative one returns full left, zero returns equal gains of one over the square root of two, positive one returns full right. |

## Text Bundle

The `Text` bundle registers utility natives for string handling. It depends on the `text` cargo feature and is a no-op when the feature is disabled.

| Function | Signature | Description |
|---|---|---|
| `to_string` | `(T) -> Text` | Render any value to its string representation. The result is allocated in the host arena's top region and becomes stale on the next reset. |
| `length` | `(T) -> Word` | Length of an array, tuple, or string. String length is counted in Unicode code points. |
| `concat` | `(Text, Text) -> Text` | Concatenate two strings. Result allocated in the arena. |
| `slice` | `(Text, Word, Word) -> Text` | Extract a substring from `start` inclusive to `end` exclusive. Bounds are character indices. Result allocated in the arena. |
| `println` | `(T) -> Unit` | Debug print. No-op in `no_std` builds; hosts may override through `register_native_closure` to obtain output. |

The math routines that previously rode along with this bundle have been consolidated under the `Math` bundle. A host that registered `Text` historically and depended on `math::sqrt`, `math::floor`, `math::ceil`, `math::round`, or `math::log2` being present must now also register `Math`.

## Shell Bundle

The `Shell` bundle registers shell-script utilities. It depends on the `shell` cargo feature, which adds a `std` dependency, and is incompatible with the `no_std` build profile.

| Function | Signature | Description |
|---|---|---|
| `shell::getenv` | `(Text) -> Option<Text>` | Read an environment variable. |
| `shell::run` | `(Text) -> (Word, Text)` | Execute a command, returning exit code and combined output. |
| `shell::run_checked` | `(Text) -> Text` | Execute a command. Traps on a non-zero exit. |
| `shell::exit` | `(Word) -> Unit` | Terminate the host process with the given exit code. |

## Type Flexibility

All math and audio functions accept both `Word` and `Float` arguments where a `Float` parameter is declared. When an integer argument is provided where a floating-point parameter is expected, the native function boundary performs automatic widening from `Word` to `Float`. This allows scripts to call `math::sin(1)` and `audio::cents_to_ratio(100)` without an explicit cast.

This widening behavior is specific to the native function boundary and does not affect the language type system itself, which remains strict about implicit coercion in all other contexts.

## Implementation Notes

All math and audio operations use the `libm` crate to provide portable floating-point math functions without depending on the Rust standard library. This ensures compatibility with `no_std+alloc` environments.

All functions in the audio and math namespaces are pure. They produce no side effects and return deterministic results for the same inputs. The host declares these functions as pure at registration time, allowing the compiler to treat them accordingly in future optimization passes.

Fallible natives report domain errors as `VmError::NativeError`. The error message names the offending native and the violated precondition. Hosts that wish to surface a different policy can register a wrapping closure that catches the error before it returns to the script side.
