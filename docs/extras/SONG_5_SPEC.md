# Song 5 specification: Phase Garden

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document is the implementation specification for
`examples/scripts/piano_roll/piano_roll_5.kel` (pending implementation), the
first minimalist-process piece in the piano-roll roster. Song
5 demonstrates phase-music techniques in the American
minimalist tradition of the late 1960s and 1970s. Two or more
channels play the same short cyclic melodic pattern at very
slightly different advance rates. Over many minutes the
relationships between channels drift through canonical
displacements, producing a constantly-shifting texture that
emerges from the deterministic-clock precision of the
implementation engine.

See the long-form manual at
[`book/src/PIANO_ROLL.md`](../../book/src/PIANO_ROLL.md) for the
broader piano-roll context, and the prior specs at
[`SONG_3_SPEC.md`](./SONG_3_SPEC.md) and
[`SONG_4_SPEC.md`](./SONG_4_SPEC.md) for the maximalist
counterparts that song 5 stands in contrast to.

## Role in the roster

Index 5 in `SONG_SOURCES`. Joins the existing roster behind the
`sdl3-example` and `text` cargo features. Accessible at runtime
through the `s` (cycle), `r` (restart), and `5` (direct select)
input commands. The input watcher requires no host change.

## High-level brief

Eight channels play the same twelve-note diatonic pattern in
D natural minor. The pattern is short, repetitive, and modal
in character. Each channel advances through the pattern at a
slightly different rate. Channel 0 is the reference and never
drifts. Channels 1 through 7 each carry a different drift
rate, so each pairs against the reference at its own pace and
produces its own emergent canonical relationships over time.

The piece has no traditional sections. The structure is the
slow evolution of the inter-channel phase relationships. After
approximately ten minutes of playback the fastest-drifting
channel completes one full pattern displacement against the
reference; after approximately ninety minutes the
slowest-drifting channel completes one full displacement.
The piece is therefore best experienced as continuous
ambient-foreground listening over an extended period, with the
listener attending to the gradual reorganisation of the
texture.

The composition loops indefinitely, but unlike songs 3 and 4
there is no explicit loop boundary in the musical sense. The
script's `state.loop_count` is used for very slow content
mutation that fires only on geological timescales (every
several full pattern-displacement cycles).

## Design intent and psychoacoustic strategy

### The minimalist contract

Songs 3 and 4 are maximalist. They demonstrate the engine by
piling features onto a dense surface. Song 5 demonstrates the
engine by doing almost nothing on the surface and letting the
deterministic clock produce the entire composition through
gradual displacement.

Phase music depends on three properties of the playback
substrate that an analog synthesizer or live ensemble cannot
guarantee. First, the timing must be exact at the
sub-millisecond level. Second, the relative drift between
voices must be mathematically precise rather than humanly
approximate. Third, the two voices must be perceptually
identifiable as the same source so the drift reads as a
displacement of unity rather than as two separate sounds. The
implementation engine satisfies all three properties
trivially.

### Why phase music is hard to perform and easy to compute

A live performer attempting a canonical phase piece from the
minimalist repertoire must listen to their partner and adjust
their tempo by an imperceptible amount per beat to produce a
clean displacement over several minutes. Performers report
the technique as one of the most demanding in the minimalist
literature because the brain resists imprecision while also
resisting synchronisation. The implementation engine has no
such conflict. The drift rate is an integer parameter. The
two voices stay perfectly aligned at every drift offset.

### Why the listener perceives structure

The listener does not consciously track the phase relationship.
The listener tracks the texture. As the drift accumulates, the
texture moves through three perceptual stages.

The unison stage. All channels in lockstep. The texture is a
single emphasised melodic line with octave reinforcement.

The canonical stage. One or more channels are exactly one
pattern position ahead of the reference. The texture reads as
imitative counterpoint, similar to a baroque round.

The plateau stage. Channels are spread across the pattern at
non-canonical offsets. The texture reads as a chord progression
or as a single complex line, depending on how the ear chooses
to parse it. The same pitch material can be heard as either
melody or as harmony at this stage.

The piece passes through these three stages cyclically. The
listener experiences the cycle as the piece's structural
narrative even though no part of the script ever signals "we
are now in the canonical stage".

### Contrast with songs 3 and 4

Song 3 is journey-with-named-stations. The listener is told
where they are at each point through tempo snaps and
instrumentation morphs. Song 4 is continuous-transformation
under invariant chord skeleton. The listener follows the
chord progression as the foundation while the surface
mutates. Song 5 is continuous-transformation under invariant
melodic pattern. There is no chord progression to follow.
There is no tempo modulation. There is no waveform mutation.
The only thing that changes is the alignment of the channels
to each other.

This is the simplest piece in the roster. It is also, by
several measures, the most subtle.

## Design constraints

- Eight channels active. Each channel carries the same pattern at a different drift rate. Channel 0 is the reference (no drift). Channels 1 through 7 each have a unique drift rate.
- One pattern. A twelve-note diatonic melodic figure in D natural minor. All channels use the same pattern; only their position within the pattern differs.
- Tempo constant. The piece runs at 120 BPM throughout. Phase music traditionally has a steady tempo; the drift-rate mechanism does not require tempo modulation, and adding tempo modulation would obscure the phase relationships.
- Per-tick BPM update is still active (it just always writes the same value). Consistency with the established songs.
- Stereo positioning critical. The eight channels are spread across the stereo field so the listener can identify individual voices and hear them drift independently. A mono mix would collapse the perceptual structure.
- Channel-pair register allocation. Channels are paired by register (bass, mid-low, mid-high, high) with each pair containing a reference voice and a phaser voice. The pair's reference voice is centred slightly toward one side and the phaser voice toward the other.

## Pattern

The pattern is twelve notes long. Each note plays for two ticks
(eighth-note duration at sixteenth-note tick resolution). One
complete cycle through the pattern occupies 24 ticks, which is
one-and-a-half bars in 4/4. The non-alignment of the pattern
period with the bar grid is deliberate; it produces an extra
layer of cross-rhythm against the standard bar structure even
before phase drift begins.

Pattern in D natural minor, MIDI values:

```
position: 0   1   2   3   4   5   6   7   8   9   10  11
pitch:    62  64  65  67  69  67  65  64  62  64  65  64
note:     D4  E4  F4  G4  A4  G4  F4  E4  D4  E4  F4  E4
```

The pattern climbs through the lower tetrachord of D natural
minor (D, E, F, G, A), pivots at A, returns through the same
tetrachord, and then traces a tighter neighbour-tone figure (D,
E, F, E) to return to the starting position. The shape is
modal and stepwise with no leaps wider than a major second.
This is consistent with the phase-music tradition where
patterns are deliberately small in range so that displacement
produces audible inter-voice relationships rather than chaos.

Each channel transposes the pattern to its target register
through octave shifts. The transposition table is in the
Channel assignments section below.

## Drift-rate engine

Each channel advances through the pattern at its own rate. The
reference channel (channel 0) advances exactly one pattern step
every two ticks. Each other channel advances at the reference
rate plus an additional step per drift period.

Pattern position for channel `c` at absolute tick `t`:

```
base_position = t / 2
extra_position = if drift_period[c] > 0 {
    t / drift_period[c]
} else {
    0
}
total_position = (base_position + extra_position) mod 12
```

The reference channel has `drift_period = 0` (no drift). Other
channels have positive drift periods. A smaller drift period
means faster drift.

### Drift-period assignments

The drift periods are chosen to produce a hierarchy of
phase-cycle lengths. The fastest channel (channel 7) completes
one full pattern displacement in approximately ten minutes.
The slowest channel (channel 1) takes approximately ninety
minutes. The hierarchy ensures the texture is always evolving
at some scale.

| Channel | Role | Drift period (ticks) | Time to complete one phase cycle |
|---------|------|----------------------|-----------------------------------|
| 0 | Reference (no drift) | 0 | Infinite (never drifts) |
| 1 | Slowest phaser | 7200 | Approximately 90 minutes |
| 2 | Slow phaser | 3600 | Approximately 45 minutes |
| 3 | Mid-slow phaser | 1800 | Approximately 22.5 minutes |
| 4 | Mid phaser | 1200 | Approximately 15 minutes |
| 5 | Mid-fast phaser | 900 | Approximately 11.25 minutes |
| 6 | Fast phaser | 600 | Approximately 7.5 minutes |
| 7 | Fastest phaser | 480 | Approximately 6 minutes |

Time-to-complete computed at 120 BPM where one tick equals
0.125 seconds and one full phase cycle requires 12 extra
advances at the drift rate. For channel 7 with drift period
480 ticks, completing 12 extras requires 5760 ticks which is
720 seconds or 12 minutes; revised estimate below.

Corrected timing for one complete twelve-step phase cycle:

| Channel | Drift period (ticks) | Ticks for 12 extras | Wall-clock at 120 BPM |
|---------|----------------------|---------------------|------------------------|
| 0 | 0 | --- | Never |
| 1 | 7200 | 86400 | Approximately 3 hours |
| 2 | 3600 | 43200 | Approximately 1.5 hours |
| 3 | 1800 | 21600 | Approximately 45 minutes |
| 4 | 1200 | 14400 | Approximately 30 minutes |
| 5 | 900 | 10800 | Approximately 22.5 minutes |
| 6 | 600 | 7200 | Approximately 15 minutes |
| 7 | 480 | 5760 | Approximately 12 minutes |

These timescales are deliberately long. A listener who plays
the piece for a few minutes will hear a slowly-evolving
texture without recognising the underlying mechanism. A
listener who plays the piece for an hour or more will hear the
fastest channel return to alignment with the reference and the
texture pass through perceptible canonical relationships.

## Channel assignments

| Channel | Role | Register transposition (semitones from pattern base) | Waveform | Pan | Notes |
|---------|------|------------------------------------------------------|----------|-----|-------|
| 0 | Reference bass | -24 (two octaves down) | Sawtooth (code 2) | Centre (500/500) | The anchor voice. No drift. The pattern descends into the bass register here. |
| 1 | Slowest phaser, sub-bass | -24 | Sine (code 3) | Centre with slight left lean (600/400) | Pairs against channel 0 for the deepest phase relationship. |
| 2 | Slow phaser, mid-low | -12 (one octave down) | Triangle (code 1) | Left of centre (700/300) | Mid-low register voice. |
| 3 | Mid-slow phaser, mid-low | -12 | Triangle | Right of centre (300/700) | Pairs against channel 2 across the stereo field. |
| 4 | Mid phaser, mid | 0 (no transposition, base register) | Sawtooth | Left (800/200) | Carries the pattern in its base register. |
| 5 | Mid-fast phaser, mid | 0 | Sawtooth | Right (200/800) | Pairs against channel 4. |
| 6 | Fast phaser, high | +12 (one octave up) | Pulse with duty 250 (code 4) | Hard left (1000/0) | High-register voice for clarity. |
| 7 | Fastest phaser, high | +12 | Pulse with duty 750 (code 4) | Hard right (0/1000) | Pairs against channel 6. The two pulse duties give complementary timbres in the stereo field. |

The four register pairs (bass, mid-low, mid, high) each contain
a reference voice and a phaser voice positioned on opposite
sides of the stereo image. The listener hears each pair's
phase relationship localised in space, which separates the
four simultaneous drift processes perceptually.

## Mid-song event schedule

### Init block

```
host::song_name("Keleusma Project: Phase Garden (0BSD)");
host::set_bpm(120);
host::set_master_volume(800);

// All channels share the same ADSR for unity of texture.
// Generous decay produces overlapping notes that mask
// individual attack transients and emphasise the continuous-
// texture character of the piece.

// Channel 0: reference bass, sawtooth.
host::set_waveform(0, 2);
host::set_adsr(0, 20, 300, 600, 400);
host::set_volume(0, 500, 500);
host::set_velocity(0, 800);
host::set_retrigger(0, 0);
host::set_lpf(0, 1500);
host::set_enable(0, 1);

// Channel 1: slowest phaser, sine.
host::set_waveform(1, 3);
host::set_adsr(1, 30, 300, 600, 400);
host::set_volume(1, 600, 400);
host::set_velocity(1, 700);
host::set_retrigger(1, 0);
host::set_lpf(1, 0);
host::set_enable(1, 1);

// Channel 2: slow phaser, triangle.
host::set_waveform(2, 1);
host::set_adsr(2, 20, 250, 700, 350);
host::set_volume(2, 700, 300);
host::set_velocity(2, 700);
host::set_retrigger(2, 0);
host::set_lpf(2, 0);
host::set_enable(2, 1);

// Channel 3: mid-slow phaser, triangle.
host::set_waveform(3, 1);
host::set_adsr(3, 20, 250, 700, 350);
host::set_volume(3, 300, 700);
host::set_velocity(3, 700);
host::set_retrigger(3, 0);
host::set_lpf(3, 0);
host::set_enable(3, 1);

// Channel 4: mid phaser, sawtooth.
host::set_waveform(4, 2);
host::set_adsr(4, 10, 200, 700, 300);
host::set_volume(4, 800, 200);
host::set_velocity(4, 700);
host::set_retrigger(4, 0);
host::set_lpf(4, 2500);
host::set_enable(4, 1);

// Channel 5: mid-fast phaser, sawtooth.
host::set_waveform(5, 2);
host::set_adsr(5, 10, 200, 700, 300);
host::set_volume(5, 200, 800);
host::set_velocity(5, 700);
host::set_retrigger(5, 0);
host::set_lpf(5, 2500);
host::set_enable(5, 1);

// Channel 6: fast phaser, pulse duty 250.
host::set_waveform(6, 4);
host::set_duty(6, 250);
host::set_adsr(6, 5, 150, 600, 200);
host::set_volume(6, 1000, 0);
host::set_velocity(6, 600);
host::set_retrigger(6, 0);
host::set_lpf(6, 0);
host::set_enable(6, 1);

// Channel 7: fastest phaser, pulse duty 750.
host::set_waveform(7, 4);
host::set_duty(7, 750);
host::set_adsr(7, 5, 150, 600, 200);
host::set_volume(7, 0, 1000);
host::set_velocity(7, 600);
host::set_retrigger(7, 0);
host::set_lpf(7, 0);
host::set_enable(7, 1);
```

### Per-tick events

```
// Constant tempo. Per-tick call for consistency with prior
// songs even though the value never changes.
host::set_bpm(120);

// Per-channel pattern advance.
for ch in 0..8 {
    let dp = drift_period(ch);
    let base = input / 2;
    let extra = if dp > 0 { input / dp } else { 0 };
    let position = (base + extra) mod 12;
    let pitch = pattern_pitch(position) + register_offset(ch);

    // Fire the pitch on every even tick within the channel's
    // pattern advance cycle. The pattern step is two ticks long;
    // play on tick 0 of each two-tick window.
    if input mod 2 == 0 {
        host::play(ch, pitch);
    }
}
```

The actual implementation unrolls the per-channel dispatch
into eight sequential statements rather than expressing it as
a `for` loop. A `for` loop over `0..8` is rejected by the
verifier because the WCMU analysis cannot bound the per-
iteration top-of-arena allocation across the loop body.
Unrolled, the body becomes straight-line code and the
`pattern_position`, `register_offset`, and `drift_period`
helper functions each verify cleanly through the resolution
of backlog item B12 in the WCMU text-size analysis. The
unrolled form is documented in `examples/scripts/piano_roll/piano_roll_5.kel`
with one explicit per-channel block.

### Optional long-term content variation

The script may read `state.loop_count` and apply a subtle
texture mutation after the first hour of playback. Candidate
mutations:

- Slight increase in channel 1's drift rate (reducing drift period from 7200 to 5400), accelerating the slowest channel's phase cycle so the listener who returns to the piece after a long absence hears a different texture than they would have heard at the same wall-clock offset.
- Introduction of a transposition on channels 4 and 5 (the mid-register pair) by an octave upward, lifting the centre of the texture.

These mutations are optional and may be omitted from the
initial implementation. The piece works without them.

## Pattern lookup function

```keleusma
fn pattern_pitch(position: Word) -> Word {
    match position {
        0 => 62,
        1 => 64,
        2 => 65,
        3 => 67,
        4 => 69,
        5 => 67,
        6 => 65,
        7 => 64,
        8 => 62,
        9 => 64,
        10 => 65,
        _ => 64,
    }
}

fn register_offset(channel: Word) -> Word {
    match channel {
        0 => -24,
        1 => -24,
        2 => -12,
        3 => -12,
        4 => 0,
        5 => 0,
        6 => 12,
        _ => 12,
    }
}

fn drift_period(channel: Word) -> Word {
    match channel {
        0 => 0,
        1 => 7200,
        2 => 3600,
        3 => 1800,
        4 => 1200,
        5 => 900,
        6 => 600,
        _ => 480,
    }
}
```

## Coverage matrix

Phase music does not exercise every native to the same extent
that songs 3 and 4 do. The aesthetic depends on stasis at
every axis except the phase position. Dynamic use of waveforms,
ADSR, vibrato, detune, and so on would defeat the piece's
character. Song 5 therefore presents a deliberate coverage
profile rather than a full-matrix exercise.

| Native | Coverage |
|--------|----------|
| `host::set_enable` | Active on all eight channels at init. No mid-piece enable changes. |
| `host::set_waveform` | Four distinct waveforms across the eight channels (Sawtooth, Sine, Triangle, Pulse). Static within the piece. |
| `host::set_duty` | Active on channels 6 and 7 at 250 and 750 respectively. Static within the piece. |
| `host::set_adsr` | Per-channel envelopes set at init. Static within the piece. |
| `host::set_volume` | Per-channel stereo positions set at init. Static within the piece. The stereo image is critical for the perceptual structure of the piece. |
| `host::set_vibrato` | Not used (left at zero defaults). Vibrato would compete with the slow phase drift for perceptual attention. |
| `host::set_lpf` | Active on channels 0 (1500 Hz) and 4, 5 (2500 Hz) for bass and mid-register clarity. Static within the piece. |
| `host::set_retrigger` | All channels at zero (legato). The overlapping-decay character requires legato playback. |
| `host::set_detune` | Not used in active state (channels are at zero detune). The piece is strictly in twelve-tone equal temperament. |
| `host::set_velocity` | Per-channel base velocities set at init. Static within the piece. |
| `host::set_master_volume` | Set once at init to 800. Static within the piece. |
| `host::set_bpm` | Called per tick with the constant value 120. The per-tick call is preserved for consistency with the established pattern; the value never changes. |
| `host::song_name` | Called once in init. |
| `host::play` | Called per channel per pattern advance. |
| `host::silence` | Not used. All channels are continuously active throughout the piece. |

The coverage is deliberately narrow. Songs 3 and 4 are the
roster's exhaustive-coverage exhibits. Song 5 is the roster's
single-technique-deep-dive exhibit.

## Verification checklist

The song is complete when:

- Compiles via `cargo run -p keleusma-cli -- compile examples/scripts/piano_roll/piano_roll_5.kel`.
- Loads through `Vm::new` against the default arena without a `VerifyError`.
- A headless probe of the first two to three minutes of playback shows each channel firing on its expected pattern positions at the expected ticks. The reference channel (channel 0) advances exactly one pattern position every two ticks. The fastest phaser (channel 7) advances at the reference rate plus one extra position every 480 ticks.
- The audible texture has the unison character at song start (all channels in lockstep), drifts perceptibly within five to ten minutes, and approaches a canonical relationship between channels 6 or 7 and channel 0 after twelve to fifteen minutes.
- Workspace tests, clippy, fmt, release build all clean.

## Sheet music feasibility

### Overview

Sheet music for a phase piece is feasible but somewhat
unusual. The convention for notating phase music is to print
the pattern once, list the per-voice drift rates as
performance directions, and provide a graphic indication of
the phase relationship at chosen sample points within the
piece.

### Notation solution

The pattern is printed once at the head of the score in
standard notation, with the eight-note staff system showing
each voice's transposition. The drift rates are given as a
performance note: "Voice 1 advances by one sixteenth note
every 90 minutes; voice 2 every 45 minutes; voice 3 every 22.5
minutes; voice 4 every 15 minutes; voice 5 every 11.25
minutes; voice 6 every 7.5 minutes; voice 7 every 6 minutes."

A human ensemble cannot execute this directly. The traditional
performance-practice solution is the click-track method, where
each performer wears headphones receiving a separate click
that ticks at the performer's individual rate. The
implementation engine's script is functionally equivalent to
eight perfectly-synchronised click tracks running at the
required rates.

### Graphic indications

Phase-music scores often include a "phase map" diagram
showing the relative positions of the voices at sample times
through the piece. For song 5, the map would show the
pattern position of each channel at the 1-minute, 5-minute,
15-minute, 30-minute, 60-minute, and 90-minute marks. The
diagram visualises the slow reorganisation of the texture and
helps the listener orient themselves in the piece's
extended timescale.

### Master-score layout

Eight staves, one per channel, bracketed together. A
performance-note paragraph at the head of the score gives the
drift rates. A phase-map diagram at the foot of the score
shows sample-time alignments. The pattern is printed once
above the master score.

### Verdict

Sheet music is feasible as a documentation artefact. The
score is not a primary performance vehicle because the piece
is most authentically realised by the implementation engine.
A human realisation through the click-track method is
possible but laborious.

## Pending implementation

The script `examples/scripts/piano_roll/piano_roll_5.kel` is not yet implemented.
The specification above provides the structural and musical
content required for the script-author pass. The script will:

- Add `include_str!("piano_roll_5.kel")` to `SONG_SOURCES` at index 5 in `examples/piano_roll.rs`.
- Implement the pattern-lookup, register-offset, and drift-period helpers.
- Implement the per-channel pattern-advance logic in the main loop body.
- Implement the init block per the spec.
- Update `book/src/PIANO_ROLL.md` to mention song 5.
- Update the module docstring in `examples/piano_roll.rs` to reflect the six-song roster.
- Verify via headless probe, lib tests, clippy, fmt, and release build per the established discipline.

The implementation effort is estimated at approximately 250 to
400 lines of Keleusma source, substantially smaller than songs
3 and 4. The principal implementation challenge is the
per-channel pattern-advance arithmetic, which must be unrolled
in the per-tick body rather than expressed as a `for` loop
because the drift-period and register-offset values vary per
channel.

## Working title and song-name string

The composition's working title is "Phase Garden". The
metaphor is botanical. A garden is a fixed plot in which
plants grow at different rates and reach different stages at
different times. The slowest-growing plants reach maturity in
the same garden as the fastest-growing plants reach senescence.
The phase relationships between the channels function the same
way.

The host song-name string is
`"Keleusma Project: Phase Garden (0BSD)"`, following the
license-tag convention established by songs 3 and 4.
