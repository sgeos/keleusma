# Extras

> **Navigation**: [Documentation Root](../README.md)

This directory holds supplementary documents that are too
specific to a single artefact to live under the architecture,
design, decisions, or process sections. Each document is a
companion reference for a particular example, song, or
extended demonstration in the repository.

For the cooperative-scheduling RTOS microkernel example, the
companion documents live alongside the example crate itself in
[`examples/rtos/`](../../examples/rtos/) because they include
operational detail (build commands, hardware setup, defmt log
interpretation) that benefits from sitting next to the source.

- [`examples/rtos/README.md`](../../examples/rtos/README.md) — overview, quick start, file table.
- [`examples/rtos/MANUAL.md`](../../examples/rtos/MANUAL.md) — operator manual: hardware setup, build matrix, platform protocol, Status protocol, porting guide, troubleshooting.
- [`examples/rtos/SPEC.md`](../../examples/rtos/SPEC.md) — architectural rationale, three-layer split, and roadmap.

## Contents

| Document | Companion to | Description |
|----------|--------------|-------------|
| [ICON_PROMPT.md](./ICON_PROMPT.md) | [`assets/Keleusma_Icon.png`](../../assets/Keleusma_Icon.png) | Recommended image-generation prompt for the project icon, with the design concept (a cosmic binary-sun pair forming an infinity loop that maps the host-to-virtual-machine control loop), the light-fill-with-black-outline branding constraint, the rationale for each prompt choice, optional deltas, and practical regeneration caveats. |
| [SONG_3_SPEC.md](./SONG_3_SPEC.md) | [`examples/scripts/piano_roll/piano_roll_3.kel`](../../examples/scripts/piano_roll/piano_roll_3.kel) | Implementation specification for the long-form eight-channel boss-theme stress test in the piano-roll example. Includes chord matrix, arpeggio vectors, percussion mask, lead-melody pitches, the Extended Dynamics Catalog, and a sheet-music feasibility appendix. |
| [SONG_4_SPEC.md](./SONG_4_SPEC.md) | [`examples/scripts/piano_roll/piano_roll_4.kel`](../../examples/scripts/piano_roll/piano_roll_4.kel) | Specification for a second full-matrix demonstration. Tempo modulates continuously on a sine wave between 60 and 300 beats-per-minute. The composition is an algorithmic chaconne with four-iteration content mutation cycling between Awakening, Descent, Malfunction, and Apocalypse variations over a constant chord skeleton. Aesthetic target is unsettling high-energy gothic-classical-metal. |
| [SONG_5_SPEC.md](./SONG_5_SPEC.md) | [`examples/scripts/piano_roll/piano_roll_5.kel`](../../examples/scripts/piano_roll/piano_roll_5.kel) | Specification for the first minimalist-process piece in the roster. Eight channels play the same twelve-note diatonic pattern in D natural minor at different advance rates so the inter-channel phase relationships drift across timescales from minutes to hours. Aesthetic target is the American minimalist phase-music tradition of the late 1960s and 1970s. |
| [SONG_6_SPEC.md](./SONG_6_SPEC.md) | [`examples/scripts/piano_roll/piano_roll_6.kel`](../../examples/scripts/piano_roll/piano_roll_6.kel) | Specification for the first polyphonic-counterpoint piece in the roster. A four-voice polymetric canon in G Dorian where each voice advances through the same four-note subject at a different tick stride (4, 3, 5, 7 ticks per subject position corresponding to 4/4, 3/4, 5/4, 7/4 meters). The voices proceed at genuinely different tempos and produce continuous canonic motion at the polymetric level. The 1680-tick superperiod realigns all four voices to subject position zero. Demonstrates genuine independent polyphony in the Nancarrow-influenced twentieth-century polymetric tradition. |
| [SONG_7_SPEC.md](./SONG_7_SPEC.md) | [`examples/scripts/piano_roll/piano_roll_7.kel`](../../examples/scripts/piano_roll/piano_roll_7.kel) | Specification for the first microtonal piece in the roster. Eight voices play the harmonic-series partials 1, 2, 3, 5, 7, 9, 11, 13 of A2 as just-intonation intervals realised through 12-TET MIDI pitches plus integer cents-of-detune offsets. The piece builds the eight-voice harmonic stack across eight 256-tick sections and resets at the loop boundary. Aesthetic target is contemplative drone music in the spectral and microtonal traditions of the late twentieth century. |
| [SONG_8_SPEC.md](./SONG_8_SPEC.md) | [`examples/scripts/piano_roll/piano_roll_8.kel`](../../examples/scripts/piano_roll/piano_roll_8.kel) | Specification for the first mainstream-pop piece in the roster. A textbook example of Japanese-pop songwriting at 108 BPM in C major, modulating to A minor for the bridge and up a half-step to D-flat major for the final chorus. Conventional twelve-section pop structure with verse, pre-chorus, chorus, interlude, bridge, modulation, and outro. Eight-channel pop-band arrangement with lead, lead arpeggio, harmony pad, rhythm pad, bass, drums, and a stereo-paired backing-vocal harmony at thirds and sixths. Deliberately unexperimental; grounded in proven mainstream techniques. |
| [SONG_9_SPEC.md](./SONG_9_SPEC.md) | [`examples/scripts/piano_roll/piano_roll_9.kel`](../../examples/scripts/piano_roll/piano_roll_9.kel) | Specification for the roster's culminating composition. Semi-experimental loop with sixteen iteration variations forming a four-by-four matrix of scale (C major, A minor, D Dorian, D Phrygian dominant) and lead waveform (Sawtooth, Square, Pulse, Triangle). Each iteration is a twelve-section pop-form (similar to song 8's structure) with verse, pre-chorus, chorus, confusion zone, bridge, modulation, and final chorus. Tempo ranges 60 BPM to 300 BPM through segmented ramps (song 3 style) plus a per-iteration confusion zone using continuous sine modulation (song 4 style) that destabilises before resolving back to coherent pop material. Roughly three minutes per iteration, fifty minutes per full meta-loop. Full host matrix coverage. |
