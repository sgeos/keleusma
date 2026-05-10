# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T44 SDL3 audio example: keleusma-piano-roll.
**Status**: Complete. New workspace member `keleusma-piano-roll` provides a real-use-case example of Keleusma driving an SDL3 audio host through a tick-based control loop. Three voices play a four-bar progression in C major that auto-loops indefinitely.

## Verification

**Commands**:

```bash
cargo build --release -p keleusma-piano-roll
cargo test --workspace --exclude keleusma-piano-roll
cargo clippy --workspace --all-targets -- -D warnings
(sleep 3; echo) | ./target/release/keleusma-piano-roll
```

**Results**:

- Workspace tests pass. 519 tests across the workspace.
- Clippy clean across `--workspace --all-targets`.
- The piano-roll binary builds (SDL3 from source via `build-from-source-static` feature, CMake-driven, ~60 seconds first build, fast on rebuilds).
- The binary runs end to end. It compiles the Keleusma script, opens the SDL3 audio device, drives the tick loop at 120 BPM, receives the stdin quit signal cleanly, and exits.

## Summary

The user requested a real-use-case example: Rust + SDL3 host with a Keleusma audio control loop driving a three-channel embedded piano roll. Single Rust file plus single Keleusma file. The result is the new workspace member `keleusma-piano-roll`.

### Architecture

The example demonstrates the canonical synchronous-reactive split that Keleusma is designed for.

- **Audio thread (SDL3 callback)**: receives a sample buffer to fill at sample rate (48 kHz), reads the current voice state from a `Mutex<[Voice; 3]>`, advances per-voice phase, sums the per-voice waveform contributions. The audio thread never invokes the Keleusma VM.
- **Main thread (Keleusma)**: runs the script's `loop main` body once per tick at 125 milliseconds (16th-note resolution at 120 BPM). Each iteration emits zero or more `host::play(channel, midi)` or `host::silence(channel)` native calls that update the shared voice state. After the body's single `yield`, the host sleeps until the next tick boundary.
- **Stdin thread**: blocks on `read_line` and flips an `AtomicBool` to signal quit. The main loop polls the flag at each tick.

The script's data segment carries per-channel position state `(idx_n, rem_n)`, allowing the script to resume cleanly across many ticks without re-allocating its bookkeeping every iteration. The audio thread receives only the small `Voice` struct (`freq: f32`, `gate: bool`) per channel, so the lock holds are short relative to the audio buffer period.

### Song

Four-bar progression in C major: `C - Am - F - G` (I-vi-IV-V). Each bar is sixteen 16th-note ticks, total loop length sixty-four ticks. The three channels:

- **Channel 0 (melody)**: square wave, sixteen quarter notes outlining each chord triad.
- **Channel 1 (bass)**: triangle wave, eight half notes on the chord roots.
- **Channel 2 (harmony)**: square wave, sixteen quarter notes on chord thirds and fifths.

The note format is `(Pitch, octave, duration_in_16ths)` where `Pitch` is an enum carrying the twelve chromatic pitch classes plus `Rest`. The format is musically idiomatic: `(Pitch::C, 4, 4)` reads as "C4 quarter note" without translation. The duration unit is sixteenth notes, so 1 = sixteenth, 2 = eighth, 4 = quarter, 8 = half, 16 = whole.

### Editability

Per the user's directive, instrument parameters live in the Rust file as `const` arrays at the top:

```rust
const WAVEFORMS: [Waveform; NUM_VOICES] = [
    Waveform::Square,   // melody
    Waveform::Triangle, // bass
    Waveform::Square,   // harmony
];
const VOLUMES: [f32; NUM_VOICES] = [0.22, 0.18, 0.18];
```

These can be edited without touching the Keleusma script. Available waveforms include square, triangle, sawtooth, and sine; the unused variants are marked `#[allow(dead_code)]` so the file compiles clean while leaving them available.

The song itself is entirely in the Keleusma script through the `melody_note`, `bass_note`, and `harmony_note` match-on-index functions plus the corresponding length functions.

### Build

SDL3 is pulled in through the `sdl3` 0.18 crate with the `build-from-source-static` feature. CMake is required at build time. The first build takes approximately one minute as SDL3 compiles from source; subsequent builds are fast. The trade-off is that the example is self-contained: a user can clone the repository and run `cargo run --release -p keleusma-piano-roll` without installing SDL3 system libraries through Homebrew or a package manager.

### Bug Fixes During Authoring

Three documentation issues surfaced as Keleusma syntax errors while writing the script. All have been corrected at the source.

1. **Data block syntax**. The data block requires a name: `data state { fields }` not `data { fields }`. The `WHY_REJECTED.md` rewrite example for the recursive-factorial case had the wrong syntax and is now corrected. The corrected version also notes that data slots default to `Value::Unit` and the host must initialize them through `vm.set_data` before driving the script.
2. **`use` declaration position**. All `use` declarations must precede every other top-level item (types, functions, data, traits, impl). The script originally placed `use host::play` and `use host::silence` after the `enum Pitch` declaration, which produced a parse error.
3. **If-else statement-position semicolons**. The Keleusma parser requires explicit trailing semicolons after `if-else` expressions when used at statement position. Rust admits the implicit-unit form; Keleusma does not. The script was updated to add semicolons after each top-level if-else and after the inner if-else for next-index advancement.

These three constraints are user-facing, not internal, and are now reflected in the corrected `WHY_REJECTED.md` example. A future enhancement to the parser could relax constraint three, but the current strictness is consistent with the language design's preference for explicit syntactic markers.

## Trade-offs and Properties

The decision to use a `Mutex<[Voice; 3]>` rather than a lock-free ring buffer or per-voice atomics reflects the small lock-hold duration relative to the audio buffer period. With three `(f32, bool)` voices the entire snapshot is around twenty bytes; copying it under the lock takes nanoseconds. A lock-free design would be appropriate for hundreds of voices or for hard real-time deadlines where any unbounded wait is unacceptable; for this example the simplicity outweighs the theoretical benefit.

The decision to have the script declare each note as a function-with-match returning a tuple, rather than as a literal array indexed in the loop body, is a performance optimization that became visible during development. Keleusma admits arrays of tuples and supports indexing them, so the literal-array form is also workable. The match-function form generates one fixed lookup per tick rather than constructing an array on every iteration. Both forms are within the verifier's capability; the match form was preferred for performance and readability.

The decision to place the song in the Keleusma file and the instrument parameters in the Rust file is a clean separation of concerns: the script controls musical decisions, the host controls synthesis decisions. A user editing the song does not need to recompile the host (only the script-compile path runs); a user editing instrument parameters does need a Rust rebuild but can do so without touching the music.

The decision to leave SDL3 as `build-from-source-static` rather than recommending a system-installed SDL3 library trades faster builds for portability. For an example that prioritizes "clone and run" ergonomics, this is the right default. A production deployment would prefer the system library to avoid bundling a copy of SDL3 in every binary.

The example illustrates three points that smaller examples do not: a real time-critical workload (audio with audible deadline), shared state across threads (the host wires Keleusma side-effects to a thread-safe handoff), and multi-voice control flow (three independently-sequenced channels with different note durations). These are the load-bearing pieces that distinguish a "real use case" example from a single-threaded compile-and-run demonstration.

## Files Touched

- **`Cargo.toml`** at workspace root. Added `keleusma-piano-roll` to workspace members.
- **`README.md`** (top-level). Workspace section updated to reflect six crates.
- **`keleusma-piano-roll/Cargo.toml`** (new). Crate metadata; depends on `keleusma`, `keleusma-arena`, `sdl3` with `build-from-source-static` feature, `libm`. License 0BSD.
- **`keleusma-piano-roll/README.md`** (new). Architecture, song description, editability notes, build instructions, "why this example" rationale.
- **`keleusma-piano-roll/song.kel`** (new). Three-channel piano roll. Pitch enum, MIDI conversion helpers, three note-lookup match functions, length functions, data segment for position state, `loop main` driving three channels per tick.
- **`keleusma-piano-roll/src/main.rs`** (new). SDL3 host. Mixer struct implementing `AudioCallback<f32>`, waveform synthesis, MIDI-to-frequency conversion, native registration, stdin reader thread, tick loop with `Instant`-based deadline.
- **`docs/guide/WHY_REJECTED.md`**. Data-block syntax corrected and host-initialization note added.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T44 in the Task Breakdown table and a new History row.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

The example is functional and was smoke-tested for end-to-end execution. Several refinements remain.

- **No ADSR envelopes**. The current synthesis is gate-driven only: a voice is on or off with no attack, decay, sustain, or release. Real instruments fade in and out smoothly. Adding a per-voice envelope state machine in the audio thread would improve listenability without changing the script-host interface.
- **Audio output not directly verifiable in CI**. The smoke test confirms the program runs and exits cleanly but does not verify the actual audio output sounds correct. A CI step that captures the audio device output and compares against a golden waveform would catch regressions; this is non-trivial and is deferred.
- **Single-frequency-per-voice limitation**. The `Voice` struct holds one frequency; rapid note changes within a single tick are not supported. The example does not need this, but a richer use case (chord trills, ornamentation) would.
- **No explicit thread-priority management on the audio callback**. SDL3 manages this internally for the audio thread, but a production embedding would want explicit verification on the deployment platform.
- **Hot code swap not demonstrated**. The user explicitly excluded this from the example as the wrong fit, but documenting the swap pattern in a separate example (load v2 of the script while audio plays v1's voices) would round out the architecture pattern coverage.
- **The `set_native_bounds` attestation pass is not exercised** in this example. The natives are zero-attested by default, which is harmless because the script does not depend on the verifier proving the native heap budget. A production audio embedding would want to attest realistic bounds for the natives.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session built a real-use-case example demonstrating Keleusma in its intended deployment shape: an embedded scripting layer driving real-time audio synthesis. The example exercises the synchronous-reactive split (tick-rate logic, sample-rate audio), the bounded-step guarantee (per-tick budget on a real deadline), the productivity guarantee (yield per tick), the data segment (per-channel position state), and native interop (host functions updating shared voice state through `Mutex`). The presence of the example is significant because it counters the most common skepticism about deeply-restricted scripting languages: "if locals are immutable and recursion is forbidden, can the language do anything useful in production?" The answer here is concretely yes for the audio-engine use case the language was designed for.
