# Piano Roll Manual

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

This document is the long-form companion to the `piano_roll` example. The example couples a Keleusma script driving 16th-note ticks against a Rust audio host that synthesizes eight-voice polyphonic output through Simple DirectMedia Layer 3. The example is small enough to read in one sitting and dense enough to exercise the patterns that recur across Keleusma host applications.

## Contents

This document carries three major sections, each addressed to a different reader.

- **[Composing songs](#composing-songs)** is for someone who wants to write a new `.kel` song to play through the example or through an adaptation of it. The reader will learn the mental model, the data segment conventions, the per-tick body structure, and the available host native function calls.
- **[Lifting the example](#lifting-the-example)** is for someone who wants to copy this example into a larger application. Two paths exist. The first embeds the host loop into another program such as a game or a music editor. The second extends the example in place into a more fully featured tool. Both paths are addressed.
- **[Embedding patterns](#embedding-patterns)** is for someone who wants to study the architecture as a pattern for embedding Keleusma in a different control-loop application. The piano roll was chosen as a low-stakes canonical because audio is easy to audit and because the patterns that work here transfer to more demanding domains.

## How this document relates to the source

The module-level documentation comment in [`examples/piano_roll.rs`](../../examples/piano_roll.rs) carries the authoritative catalog of host native functions, parameter ranges, defaults, waveform codes, and data segment slot layout. This document narrates around that catalog. Where the docstring lists what is available, this document explains how to use it and why it was structured that way. A reader trying to look up the argument shape of a specific native should consult the docstring. A reader trying to understand the architecture or to write a new song should read this document.

The bundled roster contains ten songs. Songs 0 and 1 are three-channel chord-progression scripts that introduce the host. Song 2 is a five-channel Bach Prelude arrangement demonstrating the host's native ADSR and retrigger. Song 3 is a long-form eight-channel boss-theme stress test in D minor that exercises every host native in active, inactive, and dynamic states across a ten-section dual-peak loop including a 7/8 time-signature pivot, a whole-tone snap-down gesture, three doubling techniques (stereo unison with detune, detuned octave doubling, parallel-interval harmonization in F major), and per-tick BPM updates ramping between 90 BPM and 250 BPM. Song 4 is a second full-matrix demonstration that runs the tempo under continuous sine-wave modulation between 60 BPM and 300 BPM across a 1024-tick loop body, with four iteration variations (Awakening, Descent, Malfunction, Apocalypse) cycling on the loop count over a constant D-minor chord skeleton in the manner of an algorithmic chaconne. Song 5 is a minimalist process piece in the phase-music tradition where eight channels play the same twelve-note pattern in D natural minor at different advance rates, producing inter-channel canonical relationships that drift across timescales from minutes to hours. Song 6 is a polymetric canon in G Dorian where four canonic voices share the same four-note subject but advance at different tick strides (4, 3, 5, 7 ticks per subject position corresponding to 4/4, 3/4, 5/4, 7/4 meters), producing genuine four-voice polyphonic counterpoint at a 1680-tick metric superperiod. Song 7 is a microtonal drone piece where eight voices play the just-intonation harmonic-series partials 1, 2, 3, 5, 7, 9, 11, 13 of an A2 fundamental, realised through 12-TET MIDI pitches plus integer cents-of-detune offsets, demonstrating the host's set_detune native as a continuous full-spectrum pitch-control mechanism. Song 8 is a textbook mainstream pop song at 108 BPM in C major with relative-minor bridge and half-step modulation to D-flat major for the final chorus, demonstrating that the implementation engine handles conventional commercial-pop songwriting with the same facility as it handles the experimental songs. Song 9 is a semi-experimental loop composition with a chiptune core, presenting sixteen iteration variations across a four-by-four matrix of scale (C major, A minor, D Dorian, D Phrygian dominant) and lead waveform (Sawtooth, Square, Pulse, Triangle), each iteration a twelve-section pop-form with confusion zone, bridge, modulation, and final chorus; tempo travels 60 to 300 BPM through segmented ramps and one continuous-sine confusion zone per iteration; approximately fifty minutes per full meta-loop. See [`docs/extras/SONG_3_SPEC.md`](../extras/SONG_3_SPEC.md), [`docs/extras/SONG_4_SPEC.md`](../extras/SONG_4_SPEC.md), [`docs/extras/SONG_5_SPEC.md`](../extras/SONG_5_SPEC.md), [`docs/extras/SONG_6_SPEC.md`](../extras/SONG_6_SPEC.md), [`docs/extras/SONG_7_SPEC.md`](../extras/SONG_7_SPEC.md), [`docs/extras/SONG_8_SPEC.md`](../extras/SONG_8_SPEC.md), and [`docs/extras/SONG_9_SPEC.md`](../extras/SONG_9_SPEC.md) for the full implementation specifications.

## Meta-note

This document also serves as a concrete documentation example for Keleusma host applications. The shape of its sections, the depth of its prose, and the relationship between manual and source docstring are themselves the patterns. A team building a Keleusma host in another domain can adopt the same shape for their own manual.

Compositional and music theory are out of scope. The closing of the script-author section lists a few durable category names for readers who want to pursue programmatic composition further.

---

## Composing songs

A song is a Keleusma program with the entry point `loop main(input: Word) -> Word`. The host calls `main` once at startup and then calls `Vm::resume` on every 16th-note tick. The script's body runs against the current tick value, calls zero or more host native functions, and yields control. Between iterations the host arena resets, so any per-iteration arena allocations release at no cost to the script author.

### Mental model

The script does not synthesize audio. The script schedules events. The host owns the synthesis state for each voice. The script writes to that state through host native function calls. The audio thread reads from that state and renders samples without ever entering the Keleusma virtual machine.

A song is therefore three distinct pieces of state working together. The data segment carries persistent per-channel position counters and sequencer-level state across ticks. The host voice state carries the instrument parameters such as waveform, envelope, and per-speaker volume. The script body decides at each tick which voices to play, silence, or reconfigure.

### The init block

Every bundled song begins its `loop main` body with a one-shot setup block guarded by a slot named `state.init`. The slot is zero at startup and remains zero across hot swap because the host zeroes the data segment at every song load. The init block calls every host native that configures voice state for the song and then sets `state.init` to one.

The init block is the only place in the script where instrument parameters are configured. Channels start in a disabled state. The init block enables the channels the song uses and configures their waveform, envelope, and volume. Channels not mentioned in the init block remain disabled and produce no sound.

### Data segment conventions

The host reserves twenty-three slots in the data segment. The first seven slots carry sequencer-level state. The remaining sixteen slots are per-channel position and remaining-tick counters for the full eight-voice channel count.

Slot zero, `init`, is the one-shot setup gate described above.

Slot one, `loop_count`, is bumped by the script when its progression wraps. Songs use this to vary their behaviour on subsequent loops. A first-time-through intro section can run only when `loop_count` is zero. A fade-out can begin once `loop_count` reaches a chosen threshold. A transposition can apply on every odd loop.

Slot two, `section`, is a song-section pointer. A song with a multi-part structure uses this to track which section is currently active. The value zero denotes the first section, one denotes the next, and so on. The script reads the value to dispatch to the correct note table.

Slots three through six, `user0` through `user3`, are general-purpose slots for state the host has no opinion about. Suitable uses include a random seed, a transposition offset, a per-channel mute mask, a fill-pattern selector, or anything else the song needs to track.

Slots seven through fourteen carry `idx: [Word; 8]`, the per-channel position counters for the full eight-voice channel count. Slots fifteen through twenty-two carry `rem: [Word; 8]`, the matching per-channel remaining-ticks counters. The script addresses each counter through the indexed-array form `state.idx[ch]` or `state.rem[ch]` where `ch` is a `Word` in `[0, 8)`. The compiler emits a bounds-checked indexed read or write against the underlying flat slot region; out-of-range indices trap rather than silently addressing a different counter. A script that needs to walk every channel can use `for ch in 0..8 { ... state.idx[ch] ... }` and the compiler lowers the iteration to direct indexed slot reads without materialising a `Value::Array`.

The data segment is host-owned at the schema level and script-owned at the semantic level. The host reserves the slots and zeroes them. The script decides what each slot means. The conventions above are followed by every bundled song so that the schema stays consistent across the roster.

### Per-tick body structure

After the init block, each per-channel block follows the same shape. The script checks whether the channel's remaining-ticks counter is zero. If so, it looks up the next note in the channel's note table, calls `host::play` or `host::silence` based on whether the note is a rest, sets the remaining-ticks counter to the note's duration, and advances the channel's position counter. Otherwise the script decrements the remaining-ticks counter.

This pattern keeps the per-tick cost bounded. Each tick performs a constant number of native function calls plus a small amount of data segment arithmetic. The bounded-step guarantee that Keleusma provides at the language level holds throughout.

### Working with sequencer state

A song that uses `loop_count` should bump the slot at the same boundary as the channel zero position counter, because channel zero typically holds the longest part. When channel zero's position counter wraps to zero, the song has completed one full progression. The increment goes immediately after the wrap.

A song that uses `section` should advance the slot at section boundaries the song author defines. Sections might be tied to bar counts, to specific `loop_count` values, or to a manual schedule. The reading of `section` then drives the per-channel note-table lookups so that each section can have its own progression.

### Hot swap and song-name announcement

The host announces the song's title once per load through `host::song_name`. The init block calls the native with a static string literal. Subsequent calls with the same string are silently ignored by the host. On every hot swap the host clears the tracked name so the next song announces unconditionally.

### Resources for programmatic composition

Compositional theory and musical practice are out of scope for this document. Readers who want to pursue programmatic composition further may consult the tracker-module documentation maintained by the chiptune community, surveys of algorithmic composition, and the documentation of Music Macro Language. Each of these traditions has a long history and an active community that can provide depth this document does not attempt to match.

---

## Lifting the example

This section is for the Rust host developer who wants to take this example into their own project. Two paths are addressed. The first embeds the example into a larger application. The second extends the example into a more fully featured tool. The two paths share most of their concerns and are addressed together.

### The main and run split

The example separates application chrome from the embeddable host loop. The function `main` carries command-line argument parsing and other process-level concerns. The function `run` carries the actual host work, building the Keleusma virtual machine, opening the audio device, registering host native functions, and driving the tick-and-yield cycle.

A developer embedding the example into another program copies the body of `run` into their own host code. The function takes no arguments today. Extending it to accept a song roster, an arena capacity, a default tempo, or alternative host native registrations is a localized change.

A developer extending the example into a fuller tool keeps `run` as it is and grows `main`. Command-line flags for choosing the starting song, an alternative tempo, or a different audio device land in `main` without touching `run`. The two functions stay distinct so that an embedder reading the source can recognize which part to copy and which part to discard.

### Native registration boundary

The function `register_natives` carries every host-script crossing the example offers. Each entry is a separate `vm.register_native_closure` call with a closure that captures shared state. The pattern is verbose by design. A reader can trace any native from its name through to its effect in two reads.

A production host will likely shorten this through a macro or through a registration helper. The bundled `register_library` trait described in the embedding guide is the supported abstraction for that step. The example does not use it so that the data flow stays explicit on the page.

### Pointer to exercises

The module-level documentation comment in the example lists ten substantial features that were intentionally left out so the example stays an example rather than a product. The list carries rough Rust-side line-of-code estimates for tremolo, filter envelope, delay, reverb, arpeggio, polyphonic voice allocation, sample playback, frequency modulation synthesis, wavetable synthesis, a real-time visualizer, and Musical Instrument Digital Interface input. A developer extending the example can pick any of these as a starting point. The estimates are rough and meant to scope effort rather than to commit to a precise count.

### Data segment expansion caveats

The `data state` block declared in every song script defines the data-segment schema, and every song in the roster must declare the same schema. The script declares the layout; the host passes an empty `initial_data` vector to `replace_module` so the segment reinitialises to zero on each swap. A song whose `data state` schema differs from the currently loaded one is rejected at hot swap by the `replace_module` schema-hash check unless the host opts into `replace_module_unchecked`.

The recommendation is to settle the data segment schema in advance, before any songs are written. The host author and the song author collaborate on what slots are needed for sequencer state, per-channel counters, and any application-specific bookkeeping. Once the schema is in place, every song targets it.

Mid-project changes happen, however. A host author may need to add slots to support a new sequencer feature. The cost is small for the host author and meaningful for every song already written, because each song's `data state` block must be updated to match the new schema. Mitigation strategies include scheduling schema changes to coincide with broader content revisions, reserving generous `user` slots up front so that schema growth happens within those slots rather than at the schema level, and documenting the schema version somewhere visible. A version stamp comment at the head of every `data state` block is one approach.

### Cargo feature requirements

The piano-roll example requires the `sdl3-example` Cargo feature to build. The feature pulls in the Simple DirectMedia Layer 3 dependency and `cmake`-builds SDL3 from source. The example's `required-features` declaration in `Cargo.toml` lists `compile`, `verify`, and `sdl3-example`; the first two are on by default. The build command is therefore `cargo run --release --example piano_roll --features sdl3-example`.

Static string literals (used by the bundled songs for the `host::song_name` call) are unconditional in V0.2.0. The retired V0.1.x `text` cargo feature is no longer present. A host derived from the example with a different audio backend replaces the `sdl3-example` requirement with whatever its own backend needs.

---

## Embedding patterns

This section is for the developer who wants to study the example as a reference for embedding Keleusma in a different control-loop application. Audio is the chosen domain because audio is easy to audit and because real-time deadline pressure is familiar to most developers. The patterns that work here transfer to other control loops where the cost of a missed deadline or a corrupted state may be substantially higher.

### Why this example was chosen as the canonical

The piano roll exercises the full Keleusma host surface. It uses a Stream block as its entry point. It maintains persistent state across ticks through the data segment. It performs deterministic-step iteration through `loop main`. It survives hot code swap. It coordinates two threads, one running the Keleusma virtual machine and one rendering output at a different rate. It uses host native functions to bridge between the script's logical events and the host's physical state.

None of these patterns are specific to audio. The same architecture serves a control loop running at any rate that schedules events on a regular cadence against host-owned state.

### State separation

The example splits its state into two domains. The host-owned domain carries the audio voices, the master volume, the tick interval, and the song-name dedup cache. This state lives in Rust types behind synchronization primitives. The audio thread reads it. The script writes it through host native function calls.

The script-owned domain carries the per-channel position counters, the loop count, the section pointer, and the application-specific user slots. This state lives in the Keleusma data segment. The script reads and writes it directly. The host zeroes it at every load.

The separation is principled. Host-owned state is everything the host's hot path needs to read without taking a Keleusma virtual machine call. Script-owned state is everything the script needs to reason about across iterations without the host caring about its semantics.

This separation generalizes. Any control loop in which a Keleusma program decides what should happen and a Rust thread enacts the decision should split state the same way. The script's invariants live in the data segment. The host's invariants live in Rust types behind appropriate synchronization. The native function boundary is the only crossing. The crossing is bounded, typed, and auditable, which are the same properties a serious host wants on every other boundary in its application.

### Tick-and-yield boundary

The script yields once per 16th-note tick, not once per audio sample. The decision was deliberate.

A sample-rate yield is too fine. At forty-eight thousand samples per second the script would have a budget of roughly twenty microseconds per yield, which is hard to keep clear of jitter and leaves no margin for the host work that also runs on the main thread.

A very coarse-grained yield is also wrong. It would leave the host with long stretches between opportunities to swap, restart, or reconfigure, and any input the host took during those stretches would land at the next tick boundary instead of the current one. The 16th-note tick at one hundred twenty beats per minute lands at one hundred twenty-five milliseconds between yields. This is a comfortable budget for the script's work and an acceptable latency for hot swap and command processing.

The general rule is straightforward. The tick rate should be the highest meaningful frequency at which the script makes decisions. A control loop that makes decisions every ten milliseconds should yield every ten milliseconds, not every millisecond and not every hundred milliseconds. A host that picks the wrong granularity pays a price either in latency or in budget pressure.

### Hot swap semantics

The host calls `Vm::replace_module` only when the virtual machine is in the `VmState::Reset` state. The Reset state is the boundary between iterations of the Stream block. At that point the script's stack is empty and the data segment is the only live script-owned state.

The host resets the data segment by passing a fresh zero-initialized vector to `replace_module`. The host also resets the host-owned voice state and the song-name dedup cache before issuing the swap. The incoming script's init block therefore runs against a clean slate of both domains.

The relevant principle is that hot code swap is safe only when the application's invariants live in a bounded, host-readable region. Keleusma enforces this by requiring the swap to happen at a Reset boundary. A host application embedding Keleusma in a domain other than music should respect the same constraint. Any state that needs to survive a swap belongs in the data segment, and the host should reset the host-owned domain at the same boundary so that the incoming script does not observe stale state.

### Concurrency choice

The example uses a single `Mutex<[Voice; 8]>` shared between the audio thread and the main thread. The lock is acquired for one snapshot copy per audio callback. The contention window is microseconds.

This choice was made for clarity. A reader follows the data flow on one read. The pattern would not survive a host with hundreds of voices, where the lock would become a contention point. A production host operating in that regime would either move to per-voice atomic types, to a lock-free queue, or to a triple-buffer arrangement.

The general rule is to choose the simplest synchronization primitive that meets the deadline budget. Promotion to a more complex primitive is justifiable when profiling shows contention. It is not justified by abstract scalability concerns alone. The simpler primitive keeps the data-flow visible, which matters more for an example and often matters more in production than is granted at the design stage.

### Native registration

The example registers every native function once at startup. The Keleusma virtual machine accepts native function registrations only before any script is loaded. The registration boundary is therefore the boundary between host initialization and host operation.

A host that wants different scripts to see different native function sets cannot do that within a single virtual machine instance. The available options are to register a superset and let scripts choose which to call, to use multiple virtual machine instances, or to reload the host between script changes. The example takes the first option. Every song sees the full native function surface, and a song uses the subset it needs.

The trade-off is that adding a native function later requires every loaded script to be recompiled if it uses the new function. The trade-off is acceptable for a host whose script roster is known in advance and whose natives stabilize early. Hosts whose native surface is genuinely dynamic should consider the multiple-virtual-machine pattern.

### Reset convention

The host owns the reset. The script does not reset itself. When the host loads a new module, the host clears the data segment, clears host-owned voice state, and clears any other host-side per-load caches. The script's init block then writes the values the script needs.

The convention keeps the script simple. The script author does not write defensive code for the case in which a previous song left a state machine in an unexpected configuration. The host guarantees the clean slate, and the script author can trust the guarantee.

The general principle is that reset is a host responsibility. Pushing it to the script is appropriate only if the host cannot determine which state to clear, which is rare in practice. Most host-side state has a known shape and a known reset value, and the host can clear it directly.

### Closing

The piano roll's specific opcodes do not transfer to other domains. The patterns that surround them do. State separation, tick-and-yield discipline, Reset-bounded hot swap, simple synchronization, host-owned reset, and one-shot script initialization through a flagged init block are all features of the example that an embedder in a different domain can adopt directly. The example was sized and shaped so that a reader can absorb each pattern in isolation and then assemble them into a host that fits a different application.
