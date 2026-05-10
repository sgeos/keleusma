# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel.

---

## Last Updated

**Date**: 2026-05-10
**Task**: V0.1-M3-T46 Hot code swap in the SDL3 piano-roll example.
**Status**: Complete. The example now exercises `Vm::replace_module` between two precompiled songs, audio continues across the swap, and the user toggles with the key the user specified.

## Verification

**Commands**:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --example piano_roll --features sdl3-example
cargo clippy --example piano_roll --features sdl3-example -- -D warnings
(sleep 1; printf 's\n'; sleep 1; printf 's\n'; sleep 1; printf '\n') | ./target/release/examples/piano_roll
```

**Results**:

- Workspace build remains SDL3-free; first invocation reports `Finished` with no SDL3 compilation.
- 519 workspace tests pass, unchanged from prior baseline.
- Clippy clean across the workspace and across the feature-gated example.
- The smoke-test sequence prints, in order: the welcome banner, `[ swapped to song 2 ]`, `[ swapped to song 1 ]`, `bye.`. Round-trip swap is verified.

## Summary

The user requested hot code swap in the existing SDL3 piano-roll example, with `s` followed by Enter as the swap key, and provided the second song as Keleusma source. The implementation adds the second song as `examples/piano_roll_2.kel`, precompiles both modules at startup, and threads an `mpsc` command channel from the stdin reader to the tick loop.

### Swap protocol

The protocol mirrors the existing `Vm::replace_module` test cases.

1. The tick loop drains the stdin command channel non-blocking at each tick boundary. A `Command::Swap` flips a `swap_pending` flag; a `Command::Quit` breaks the outer loop.
2. The inner resume loop drives the script until the next `VmState::Yielded`. Between body iterations the script transits `VmState::Reset`. The current code already consumed `Reset` transparently, but now also checks `swap_pending`.
3. On `Reset` with `swap_pending` true, the host calls `vm.replace_module(next_module, fresh_data())`, where `fresh_data` returns six `Value::Int(0)` slots so the incoming song starts at the beginning of its phrase. The data-segment schema is identical between the two songs, so the size check on `replace_module` succeeds.
4. After `replace_module`, the host calls `vm.call(&[Value::Int(tick)])` to start the new module's entry point, drives to the first yield, and counts that as the current tick.
5. Voices are explicitly silenced after swap so the new song does not inherit gate state from the outgoing song.

### Voice continuity across swap

Audio rendering continues without interruption across the swap. The audio thread reads from the same `Mutex<[Voice; 3]>` regardless of which Keleusma module is active. Native registrations are stored on the VM, not on the module, so they persist across `replace_module`. This is the embedding-grade hot-swap property the language is designed to support.

The decision to silence voices on swap rather than let them ring through the swap is conservative. The alternative behavior would let the outgoing song's gated voices continue until the incoming song chose to retrigger or silence them. Both behaviors are defensible; silence-on-swap was chosen because it makes the swap audibly clean and avoids a class of subtle bugs where the new song's first tick produces no `host::play` call on a particular channel and the outgoing song's pitch lingers indefinitely.

### Song 2 design

The user supplied the note tables for song 2. Song 2 uses an eighth-note arpeggiated melody (compared to song 1's quarter-note triadic melody), a quarter-note alternating-harmony line (compared to song 1's quarter-note triadic harmony), and a staccato bass with rests on every other slot (compared to song 1's continuous half-note bass). Both songs share the C major progression I-vi-IV-V (`C - Am - F - G`).

Song 2 also reorders the channel-to-musical-role mapping. In song 1, channel 1 carries the bass and channel 2 carries the harmony. In song 2, channel 1 carries the harmony and channel 2 carries the bass. The host's per-channel waveform table (`[Square, Triangle, Square]`) is fixed across the swap, so this reordering produces a deliberate timbral shift: the bass moves from a triangle voice (song 1) to a square voice (song 2), and the harmony moves the other direction. This emergent property of the swap is intentional and shows that the script controls musical assignment while the host owns synthesis.

### Stdin command channel

The previous implementation used an `AtomicBool` quit flag set by a stdin reader thread. The new implementation uses `std::sync::mpsc::Sender<Command>` / `Receiver<Command>` with a small `Command` enum having `Swap` and `Quit` variants. The stdin thread now runs in a loop: each line is parsed, "s" sends `Swap` and continues, anything else sends `Quit` and exits. The main loop's `try_recv` is non-blocking, so the tick clock is unaffected by stdin idle time.

A line-buffered terminal does not capture single keystrokes without raw-mode support, so "press 's'" is in practice "type s and press Enter". The banner is explicit about this.

## Trade-offs and Properties

The decision to precompile both songs at startup, rather than recompile from source on each swap, reflects the example's pedagogical purpose: hot code swap is the language feature being demonstrated, not script compilation latency. A production embedder might recompile on swap if the script content can change at runtime; for the example, both songs are known at build time and embedded via `include_str!`.

The decision to keep `module_a` and `module_b` as cloneable `Module` values rather than `Rc<Module>` or boxed values is motivated by `replace_module`'s ownership signature, which takes an owned `Module`. `Module` derives `Clone`, so cloning at swap time is cheap relative to the other costs of replacement.

The decision to reset the data segment to all zeros at swap rather than carry it across is consistent with the user's "Replace semantics" choice from prior architecture decisions. The data segment of song 1 holds song-1 channel positions, which would be meaningless to song 2. A future enhancement could let the host migrate selected slots, but for the same-schema case here, fresh-zero is the right default.

The decision to silence voices at swap (`silence_all`) sits inside the swap branch, so the silencing is paired with the module replacement. The audio thread observes a brief moment of total silence before the new song's first tick fires `host::play` calls. The duration of the gap is one tick interval (125 ms) at most, which is below most listeners' threshold for noticing a phrase break.

The example deliberately does not demonstrate same-song reload. The interesting test case is two genuinely different songs, which exercises both verification (the new module re-runs structural and resource verification at swap) and dialogue compatibility (both songs export the same six-slot data schema).

## Files Touched

- **`examples/piano_roll.kel`**. Unchanged from the prior task.
- **`examples/piano_roll_2.kel`** (new). Second song. Pitch enum, MIDI helpers, three note-lookup match functions, length functions, data-segment schema identical to song 1, `loop main` with reordered channel-to-role routing.
- **`examples/piano_roll.rs`**. Doc header rewritten to describe the hot-swap protocol. Imports updated to add `mpsc` and `keleusma::bytecode::Module`. Two `SCRIPT_*` constants. New `Command` enum. Stdin thread parses lines and sends commands instead of flipping a flag. Tick loop drains the command channel and threads `swap_pending` through the inner resume loop. New helpers: `build_module`, `init_data`, `fresh_data`, `silence_all`, `register_natives`. Banner updated.
- **`docs/process/TASKLOG.md`**. New row for V0.1-M3-T46.
- **`docs/process/REVERSE_PROMPT.md`**. This file.

## Remaining Open Priorities

Most refinements from prior tasks still apply. New items raised by this task.

- **Same-song silence gap.** Voices are silenced for one tick interval at swap. A short crossfade in the audio mixer would smooth the transition, at the cost of tracking pre-swap and post-swap voice state. Not blocking; the gap is barely perceptible at 125 ms.
- **Stdin parsing is line-based.** True any-key swap (no Enter required) needs raw-mode terminal support, which would add a `crossterm`-class dependency. For the current line-buffered design, the user types `s` then Enter. The banner is explicit.
- **No persistence of swap history.** The stdin parser sends `Quit` on any non-`s` line, including obvious typos. A future enhancement could allow `quit` or `q` as the explicit quit command and ignore other input. The current behavior is conservative ("don't try to be clever"); any unrecognised input means "stop".
- **No demonstration of dialogue-incompatible swap.** Both songs share the data-segment schema, so the swap path exercises only the value-preserving case. A second example showing a swap with a `replace_module` that fails on schema mismatch would round out the hot-swap coverage but is not core to this example's pedagogical purpose.

## Intended Next Step

Await human prompt before proceeding.

## Session Context

This session added hot code swap to the SDL3 piano-roll example, completing the architectural pattern coverage requested across the recent sessions. The example now demonstrates the principal load-bearing capabilities Keleusma is designed for: bounded-step execution under a real-time deadline (audio rendering), shared state across threads (the `Mutex<[Voice; 3]>` handoff between Keleusma main thread and SDL3 audio thread), multi-voice control flow (three independently-sequenced channels through the data segment), and now hot code swap (two precompiled modules, swap at the reset boundary, audio continuity across the swap). The user's "Press Enter to quit" prior preference is preserved as the quit affordance, with `s` plus Enter added as the swap affordance, both communicated explicitly in the welcome banner.
