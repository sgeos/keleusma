# keleusma-piano-roll

Three-channel piano-roll example. SDL3 audio host driven by a Keleusma tick-based control loop. The Rust file synthesizes audio on the SDL3 audio thread; the Keleusma script sequences the music on the main thread at 120 BPM.

The example is intended as a small but realistic demonstration of the synchronous-reactive split that the Keleusma language is designed for: musical logic at tick rate, sample synthesis at sample rate, with a thread-safe handoff in between.

## What It Does

Plays a four-bar progression in C major (`C - Am - F - G`) on three voices.

- **Channel 0 (melody)**: square wave, quarter notes outlining each chord triad.
- **Channel 1 (bass)**: triangle wave, half notes on the chord roots.
- **Channel 2 (harmony)**: square wave, quarter notes on chord thirds and fifths.

The progression auto-loops indefinitely.

## Run

```sh
cargo run --release -p keleusma-piano-roll
```

The first build compiles SDL3 from source through the `build-from-source-static` feature. CMake is required. Subsequent builds are fast.

Press Enter in the terminal to quit.

## Files

| File | Role |
|------|------|
| [`song.kel`](./song.kel) | Keleusma script. Three hard-coded note arrays (one per channel), per-channel position state in the data segment, one tick per yield. |
| [`src/main.rs`](./src/main.rs) | SDL3 host. Compiles the script, registers `host::play` and `host::silence` natives that update shared voice state, runs the tick loop on the main thread while SDL3 renders samples on the audio thread. |

## Editing the Song

The note arrays are in `song.kel`. Each note is a tuple `(Pitch, octave, duration_in_16ths)`. Examples.

- `(Pitch::C, 4, 4)` is C4 quarter note (60 in MIDI).
- `(Pitch::Fs, 5, 8)` is F-sharp 5 half note.
- `(Pitch::Rest, 0, 4)` is a quarter rest.

The duration unit is sixteenth notes: 1 = sixteenth, 2 = eighth, 4 = quarter, 8 = half, 16 = whole.

The functions `melody_note`, `bass_note`, and `harmony_note` are the entry points. The corresponding length functions `melody_len`, `bass_len`, `harmony_len` declare how many entries each channel has. Edit the match arms and update the length functions to match.

## Editing the Instruments

Per-channel instrument parameters are in `src/main.rs`, near the top of the file as plain `const` arrays.

```rust
const WAVEFORMS: [Waveform; NUM_VOICES] = [
    Waveform::Square,   // melody
    Waveform::Triangle, // bass
    Waveform::Square,   // harmony
];

const VOLUMES: [f32; NUM_VOICES] = [0.22, 0.18, 0.18];
```

Available waveforms: `Square`, `Triangle`, `Sawtooth`, `Sine`. The `VOLUMES` array sets per-channel mix amplitudes; their sum should stay below 1.0 to avoid clipping.

`SAMPLE_RATE` and `TICK_MS` are also constants in the same file. The default 125 ms tick at 120 BPM gives sixteenth-note resolution; halve it for thirty-second notes or double for eighth-note resolution.

## Architecture

The example demonstrates the canonical embedded-scripting split.

- **Audio thread (SDL3 callback)**: receives a sample buffer to fill, reads the current voice state from a `Mutex<[Voice; 3]>`, advances per-voice phase, sums waveforms. The audio thread never calls into Keleusma.
- **Main thread (Keleusma)**: runs the script's `loop main` body once per tick. Each iteration emits zero or more `host::play(channel, midi)` or `host::silence(channel)` native calls that update the shared voice state. After the body's single `yield`, the host sleeps until the next tick boundary.
- **Stdin thread**: blocks on `read_line` and flips an `AtomicBool` to signal quit.

The Keleusma data segment holds per-channel position state (`idx_n`, `rem_n`) so the script can resume cleanly across many ticks without re-allocating its bookkeeping every iteration.

## Why This Example

Three points the example illustrates that smaller examples do not.

1. **Real time-critical workload**. Audio output is the canonical real-time deadline domain. A missed deadline produces an audible glitch. The example shows that a verifier-accepted Keleusma script meets the deadline budget reliably without garbage-collection pauses or unbounded recursion.
2. **Shared state across threads**. The host wires Keleusma side-effects to a thread-safe handoff into the audio engine. This is the embedding pattern most production scripting hosts need, and it is the load-bearing piece that "single-threaded toy" examples leave out.
3. **Multi-voice control flow**. Three independently-sequenced channels with different note durations exercise the data segment as the only mutable region the script can address, which is the key Keleusma constraint relative to typical scripting languages.

## License

0BSD. Same as Keleusma.
