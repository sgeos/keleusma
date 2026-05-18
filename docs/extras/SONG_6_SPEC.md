# Song 6 specification: Quadrameter Canon

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document is the implementation specification for
`examples/scripts/piano_roll/piano_roll_6.kel` (pending implementation), the
first polyphonic-counterpoint piece in the piano-roll roster.
Song 6 demonstrates a four-voice canon in which each voice
carries the same melodic subject on a different metric grid.
The four meters are 4/4, 3/4, 5/4, and 7/4. The voices enter at
staggered times so the listener can track each entry, and they
proceed simultaneously thereafter at their own metric strides.
The piece is contrapuntal in the strict sense: each voice is a
fully independent melodic agent and the harmonic content
emerges from the alignment of the four voices rather than from
a separate accompaniment layer.

See the long-form manual at
[`docs/guide/PIANO_ROLL.md`](../guide/PIANO_ROLL.md) for the
broader piano-roll context, and the prior specs at
[`SONG_3_SPEC.md`](./SONG_3_SPEC.md),
[`SONG_4_SPEC.md`](./SONG_4_SPEC.md), and
[`SONG_5_SPEC.md`](./SONG_5_SPEC.md) for the other long-form
demonstrations.

## Role in the roster

Index 6 in `SONG_SOURCES`. Joins the existing roster behind the
`sdl3-example` and `text` cargo features. Accessible at runtime
through the `s` (cycle), `r` (restart), and `6` (direct select)
input commands. The input watcher requires no host change.

## High-level brief

Four canonic voices on the same four-note melodic subject. Each
voice advances through the subject at a different tick rate
corresponding to its meter's beat duration. Voice 1 in 4/4
advances one subject position every 4 ticks (one quarter
note). Voice 2 in 3/4 advances every 3 ticks. Voice 3 in 5/4
advances every 5 ticks. Voice 4 in 7/4 advances every 7 ticks.
The voices proceed at genuinely different tempos relative to
one another, producing the polymetric phase relationships that
make this a canon in the twentieth-century polymetric sense
rather than a unison-with-different-accents.

The least common multiple of the four per-voice subject
durations is 1680 ticks. Voice 1 completes one subject in 16
ticks (4 positions × 4 ticks); voice 2 in 12 ticks; voice 3 in
20 ticks; voice 4 in 28 ticks. The LCM of (16, 12, 20, 28) is
1680 ticks, which is the metric superperiod and the loop
length of the piece. At this superperiod boundary, all four
voices simultaneously return to subject position 0 and the
canon repeats.

The composition is in G Dorian. The subject is a four-note
ascending tetrachord (G, A, B-flat, C) that exhibits the
mode's characteristic stepwise modal motion. The subject is
short by design; with four voices each advancing at a
different rate, a short subject produces frequent canonic
relationships (every voice rotates through every position
many times per superperiod), maximising the harmonic
information per unit time.

The aesthetic target is rigorous polymetric counterpoint in the
Nancarrow-influenced twentieth-century tradition. The piece is
meant to read as a four-voice canon in which each voice
proceeds at its own tempo, producing continuously-evolving
vertical harmonic alignments that resolve to unison at the
superperiod boundary.

Approximate listening lengths.

- One full superperiod cycle (after all four voices have entered): approximately 210 seconds at 120 BPM.
- First iteration including the staggered voice entries: approximately 216 seconds.
- The composition loops indefinitely.

## Design intent and psychoacoustic strategy

### The contrapuntal contract

Songs 3 and 4 are homophonic at heart. They carry a lead voice
with arpeggio decoration and accompaniment. Song 5 is texture
without melody. Song 6 is the first piece in the roster where
each channel is melodically equal to the others. The four
canonic voices share the same melodic identity and the piece's
musical content is the relationship between them.

A canon in strict counterpoint must satisfy three constraints.
First, every voice must play the same subject. Second, the
voices must enter at predetermined intervals. Third, the
harmonic content of the simultaneity at every moment must be
acceptable; the four voices must align such that no moment
produces a dissonance the composition's harmonic language does
not permit. The third constraint is what distinguishes a canon
from a round (which is a canon at the unison) and from
arbitrary counterpoint (which has no requirement of subject
repetition).

The polymetric extension to the classical canon adds a fourth
constraint. The voices proceed at different metric speeds.
This is the twentieth-century innovation; the classical canon
keeps all voices on a single metric grid. The polymetric canon
introduces phase relationships that conventional canon does not
support.

### Why this fits Keleusma

Polymetric canon requires absolutely precise timing. A live
ensemble executing four voices on 4/4, 3/4, 5/4, and 7/4
simultaneously is extraordinarily difficult; performers report
the technique as more demanding than equivalent polyrhythmic
exercises because each performer must keep their own metric
count without losing the shared global time. The conductor's
gesture, normally the anchor for the ensemble, becomes
unreliable when no two performers share the same bar length.

The implementation engine has no such conflict. Each voice's
metric stride is an integer parameter. The voices stay
perfectly aligned at every moment.

### Why this fits the roster

Songs 3 and 4 demonstrate the engine's range through dynamic
feature use and structural complexity. Song 5 demonstrates the
engine's precision through gradual phase drift. Song 6
demonstrates the engine's polyphonic capacity through
simultaneous-but-independent melodic lines. The three together
exhibit the engine across the orthogonal dimensions of
dynamics, precision, and polyphony.

### The listener experience

The piece begins with one voice alone (the 4/4 voice, channel
0). The 3/4 voice enters at the second subject statement. The
5/4 voice enters at the third. The 7/4 voice enters at the
fourth. After all four voices are active, the listener hears
the texture as a flowing four-voice polyphony. Each voice
remains identifiable through its register and timbre.

As the voices proceed on their independent strides, the
alignment between them shifts continuously. Moments where the
voices land on the same beat produce strong harmonic anchors;
moments where they offset produce passing dissonances and
resolutions. The piece is densest at the moments when all four
voices coincide on a downbeat (which happens once per
superperiod) and sparsest at the moments when they are maximally
displaced.

The intended experience is one of contemplative listening to a
piece that rewards close attention. The piece is shorter and
simpler than songs 3 or 4 but more dense in its polyphonic
information per second. A listener who follows the canonic
relationships will hear the piece as a single coherent
contrapuntal motion. A listener who does not track the canon
will hear a constantly-shifting modal-folk texture.

### Contrast with songs 3 through 5

Song 3 is journey-with-named-stations. Song 4 is continuous-
transformation under invariant chord skeleton. Song 5 is
slow-drift-through-identity. Song 6 is parallel-melodic-
agency. The four pieces collectively span the major aesthetic
modes that the implementation engine can express.

## Design constraints

- Four voices in strict canon. Each voice plays the same melodic subject. Channels 0 through 3 are the canonic voices. Channels 4 through 7 carry harmonic and rhythmic support layers that do not participate in the canon.
- Four independent meters. The four canonic voices run on 4/4, 3/4, 5/4, and 7/4 respectively. The metric stride of each voice determines its rate of advance through the subject.
- Staggered entries. The four voices enter at the second, third, and fourth subject statements respectively. The first subject statement is voice 1 alone.
- Modal harmony. The piece is in G Dorian. The harmonic content emerges from the alignment of the four canonic voices rather than from a separate chord progression.
- Constant tempo. The piece runs at 120 BPM throughout. Phase music and polymetric music traditionally have steady tempo; tempo modulation would obscure the metric relationships.
- Stereo positioning. The four canonic voices are spread across the stereo field so each voice is identifiable. The first voice on the left, the second voice slightly left, the third voice slightly right, the fourth voice on the right.

## The subject

The subject is four notes long, an ascending tetrachord through
the first four scale degrees of G Dorian. Each voice traverses
this subject at its own metric stride. The shortness of the
subject is structurally deliberate: with four voices each
proceeding at a different rate, a long subject would produce
slow-changing harmonic textures dominated by repetition.
A four-note subject produces frequent recombinations of the
four pitches across the four voices, giving the piece its
harmonic density.

Pattern in G Dorian, MIDI values:

```
position: 0   1   2   3
pitch:    67  69  70  72
note:     G4  A4  B♭4 C5
```

The subject ascends through the first four scale degrees of G
Dorian: tonic, supertonic, mediant (flat-3), and subdominant
(fourth). All motion is stepwise. There are no leaps. The
subject is deliberately minimal, leaving the polymetric canon
itself to generate the piece's harmonic interest rather than
relying on the subject's internal melodic motion.

G Dorian shares all notes with F major and D natural minor but
is heard as centred on G. The flat-7 (F natural) and major-6
(E natural) are the characteristic Dorian colours; neither is
used in this subject's four notes, but both appear in the
support layer chord-tones.

## Metric stride and entry schedule

### Metric stride

Each voice advances through the subject at one note per beat,
where the beat duration depends on the voice's meter. The
metric stride is the number of ticks between consecutive subject
notes for that voice.

| Channel | Meter | Stride (ticks per subject note) | Subject duration (ticks) |
|---------|-------|----------------------------------|---------------------------|
| 0 | 4/4 | 4 | 16 (4 notes × 4 ticks) |
| 1 | 3/4 | 3 | 12 |
| 2 | 5/4 | 5 | 20 |
| 3 | 7/4 | 7 | 28 |

Voices advance at different tick rates. Voice 1 plays G, A,
B-flat, C across 16 ticks. Voice 2 plays the same notes across
12 ticks (faster). Voice 3 plays them across 20 ticks (slower).
Voice 4 plays them across 28 ticks (slowest). The voices
proceed at genuinely different speeds. At any given tick, the
four voices are at different subject positions and produce
distinct harmonic alignments.

The stride values are deliberately chosen so the LCM of (16,
12, 20, 28) is 1680 ticks, matching the metric superperiod
calculation in the spec. The factorisation is:

```
16 = 2^4
12 = 2^2 × 3
20 = 2^2 × 5
28 = 2^2 × 7
LCM = 2^4 × 3 × 5 × 7 = 1680
```

### Per-voice accent

Each voice emphasises every bar's downbeat through a velocity
boost. The bar duration depends on the voice's meter. The
accent fires when the voice's firing-count modulo the meter
denominator is zero.

| Voice | Meter | Accent on firing count |
|-------|-------|-------------------------|
| 0 | 4/4 | Every 4th firing |
| 1 | 3/4 | Every 3rd firing |
| 2 | 5/4 | Every 5th firing |
| 3 | 7/4 | Every 7th firing |

The first firing of each voice is always accented (firing count
zero satisfies the modulo condition). The accent pattern per
voice through a long sequence diverges from the others' accent
patterns because the four moduli are coprime.

### Entry schedule

| Voice | Channel | Entry tick | Description |
|-------|---------|------------|-------------|
| 1 | 0 | 0 | Plays alone for the first 16 ticks. |
| 2 | 1 | 16 | Enters when voice 1 completes its first subject statement. |
| 3 | 2 | 32 | Enters when voice 1 completes its second subject statement. |
| 4 | 3 | 48 | Enters when voice 1 completes its third subject statement. |

The first 48 ticks of the piece carry the staggered entries.
From tick 48 onward all four voices are active. The voices
have different per-voice strides and are at different subject
positions, producing the polymetric polyphony.

### Subject position per voice

The position of voice `v` in the subject at loop-tick `t` is:

```
firing_count = (t - effective_entry_tick[v]) / stride[v]
subject_position = firing_count mod 4
```

The voice fires only when `(t - effective_entry_tick[v]) mod
stride[v] == 0` and `t >= effective_entry_tick[v]`.

The effective entry tick depends on iteration:

- In the first iteration (`input < 1680`): the entry ticks are the staggered values 0, 16, 32, 48.
- In subsequent iterations: all voices have effective entry tick 0; every voice fires from loop-tick zero.

This produces the staggered entries only on the first listen.
Hot-swapping into song 6 also produces the staggered entries
because the host resets the tick counter on song load.

### Superperiod and loop

The piece loops at tick 1680 in the first iteration and every
1680 ticks thereafter. At the loop boundary all four voices
return to subject position 0 simultaneously (because each
voice's per-stride subject-cycle count divides evenly into
1680). The voices realign and the canon repeats. The
realignment moment is harmonically a strong unison-and-octave
G, the tonic.

## Channel assignments

| Channel | Role | Register | Waveform | Pan | Notes |
|---------|------|----------|----------|-----|-------|
| 0 | Voice 1 canon, 4/4 | Base register (G4..D5) | Sawtooth (code 2) | Hard left (1000/0) | The reference voice. Enters first. |
| 1 | Voice 2 canon, 3/4 | One octave down (G3..D4) | Triangle (code 1) | Slight left (700/300) | Enters second. The lowest of the four canonic voices. |
| 2 | Voice 3 canon, 5/4 | Base register (G4..D5) | Pulse with duty 250 (code 4) | Slight right (300/700) | Enters third. Same register as voice 1 but a different waveform; the stereo separation keeps them distinct. |
| 3 | Voice 4 canon, 7/4 | One octave up (G5..D6) | Pulse with duty 750 (code 4) | Hard right (0/1000) | Enters fourth. The highest of the four canonic voices. |
| 4 | Sub-bass pedal | Two octaves down (G2 and D2) | Sine (code 3) | Centre (500/500) | Provides a slow-changing harmonic foundation under the canon. Alternates between G2 and D2 at the bar level. Not part of the canon. |
| 5 | Percussion | Noise (code 5) | Centre (500/500) | Light cymbal-and-block pattern marking the global beat. Per-tick ADSR for hi-hat and rim variants. |
| 6 | Pad layer | Base register | Triangle (code 1) | Slight left (600/400) | Sustained chord-tone pad. Plays the tonic G note for the duration of the piece, with light vibrato. Provides ambient warmth. |
| 7 | Pad layer mirror | Base register | Triangle (code 1) | Slight right (400/600) | Pairs with channel 6. Plays the fifth (D) for the duration of the piece. Together channels 6 and 7 provide a continuous G-and-D drone. |

Channels 0 through 3 are the canonic voices. Channels 4
through 7 are the support layer: a sub-bass pedal, light
percussion, and a sustained drone. The support layer is
intentionally minimal so the four canonic voices remain the
focus.

## Subject lookup function

```keleusma
fn subject_pitch(position: Word) -> Word {
    match position {
        0 => 67,  // G4
        1 => 69,  // A4
        2 => 70,  // B-flat-4
        _ => 72,  // C5
    }
}
```

Register transposition per voice is applied at the call site:
voice 1 plays `subject_pitch(pos)` directly (G4..C5), voice 2
plays `subject_pitch(pos) - 12` (G3..C4, one octave down),
voice 3 plays `subject_pitch(pos)` (same register as voice 1),
voice 4 plays `subject_pitch(pos) + 12` (G5..C6, one octave up).

## Mid-song event schedule

### Init block

```
host::song_name("Keleusma Project: Quadrameter Canon (0BSD)");
host::set_bpm(120);
host::set_master_volume(800);

// Channel 0: Voice 1 canon, 4/4. Sawtooth, hard left.
host::set_waveform(0, 2);
host::set_adsr(0, 10, 150, 700, 250);
host::set_volume(0, 1000, 0);
host::set_velocity(0, 800);
host::set_retrigger(0, 0);
host::set_vibrato(0, 0, 0);
host::set_lpf(0, 0);
host::set_detune(0, 0);
host::set_enable(0, 1);

// Channel 1: Voice 2 canon, 3/4. Triangle, slight left.
host::set_waveform(1, 1);
host::set_adsr(1, 15, 200, 700, 300);
host::set_volume(1, 700, 300);
host::set_velocity(1, 800);
host::set_retrigger(1, 0);
host::set_vibrato(1, 0, 0);
host::set_lpf(1, 0);
host::set_detune(1, 0);
host::set_enable(1, 1);

// Channel 2: Voice 3 canon, 5/4. Pulse 250, slight right.
host::set_waveform(2, 4);
host::set_duty(2, 250);
host::set_adsr(2, 5, 150, 700, 200);
host::set_volume(2, 300, 700);
host::set_velocity(2, 750);
host::set_retrigger(2, 0);
host::set_vibrato(2, 0, 0);
host::set_lpf(2, 0);
host::set_detune(2, 0);
host::set_enable(2, 0);  // Disabled until entry.

// Channel 3: Voice 4 canon, 7/4. Pulse 750, hard right.
host::set_waveform(3, 4);
host::set_duty(3, 750);
host::set_adsr(3, 5, 150, 700, 200);
host::set_volume(3, 0, 1000);
host::set_velocity(3, 750);
host::set_retrigger(3, 0);
host::set_vibrato(3, 0, 0);
host::set_lpf(3, 0);
host::set_detune(3, 0);
host::set_enable(3, 0);  // Disabled until entry.

// Channel 4: Sub-bass pedal. Sine, centre.
host::set_waveform(4, 3);
host::set_adsr(4, 30, 300, 800, 400);
host::set_volume(4, 500, 500);
host::set_velocity(4, 700);
host::set_retrigger(4, 0);
host::set_lpf(4, 800);
host::set_enable(4, 1);

// Channel 5: Percussion. Noise, centre.
host::set_waveform(5, 5);
host::set_volume(5, 500, 500);
host::set_velocity(5, 500);
host::set_retrigger(5, 1);
host::set_enable(5, 1);

// Channel 6: G drone pad, slight left.
host::set_waveform(6, 1);
host::set_adsr(6, 100, 300, 800, 500);
host::set_volume(6, 600, 400);
host::set_velocity(6, 400);
host::set_retrigger(6, 0);
host::set_vibrato(6, 300, 25);
host::set_lpf(6, 0);
host::set_enable(6, 1);

// Channel 7: D drone pad, slight right.
host::set_waveform(7, 1);
host::set_adsr(7, 100, 300, 800, 500);
host::set_volume(7, 400, 600);
host::set_velocity(7, 400);
host::set_retrigger(7, 0);
host::set_vibrato(7, 350, 25);
host::set_lpf(7, 0);
host::set_enable(7, 1);
```

### Per-tick events

```
host::set_bpm(120);

let loop_tick = input mod 1680;
let in_first = input < 1680;

// Voice 1: 4/4 stride. Fires every 4 ticks. No entry stagger.
if loop_tick mod 4 == 0 {
    let f1 = loop_tick / 4;
    if f1 mod 4 == 0 {
        host::set_velocity(0, 950);  // accent every 4 firings
    } else {
        host::set_velocity(0, 750);
    }
    host::play(0, subject_pitch(f1 mod 4));
}

// Voice 2: 3/4 stride. Entry at tick 16 in first iteration,
// tick 0 in subsequent. Fires every 3 ticks from entry.
let e2 = if in_first { 16 } else { 0 };
if loop_tick >= e2 and (loop_tick - e2) mod 3 == 0 {
    let f2 = (loop_tick - e2) / 3;
    if f2 mod 3 == 0 {
        host::set_velocity(1, 950);
    } else {
        host::set_velocity(1, 750);
    }
    host::play(1, subject_pitch(f2 mod 4) - 12);
}

// Voice 3: 5/4 stride. Entry at tick 32 in first iteration.
let e3 = if in_first { 32 } else { 0 };
if loop_tick >= e3 and (loop_tick - e3) mod 5 == 0 {
    let f3 = (loop_tick - e3) / 5;
    if f3 mod 5 == 0 {
        host::set_velocity(2, 900);
    } else {
        host::set_velocity(2, 700);
    }
    host::play(2, subject_pitch(f3 mod 4));
}

// Voice 4: 7/4 stride. Entry at tick 48 in first iteration.
let e4 = if in_first { 48 } else { 0 };
if loop_tick >= e4 and (loop_tick - e4) mod 7 == 0 {
    let f4 = (loop_tick - e4) / 7;
    if f4 mod 7 == 0 {
        host::set_velocity(3, 900);
    } else {
        host::set_velocity(3, 700);
    }
    host::play(3, subject_pitch(f4 mod 4) + 12);
}

// Sub-bass pedal: alternates G2 and D2 every 32 ticks.
if loop_tick mod 32 == 0 {
    let pedal_pitch = if (loop_tick / 32) mod 2 == 0 { 43 } else { 38 };
    host::play(4, pedal_pitch);
}

// Drone pads: G and D held continuously across the loop body.
if loop_tick == 0 {
    host::play(6, 67);  // G4
    host::play(7, 74);  // D5
}

// Light percussion on every 4 ticks (a global beat reference).
if loop_tick mod 4 == 0 {
    let beat = (loop_tick / 4) mod 4;
    if beat == 0 {
        host::set_adsr(5, 1, 25, 0, 10);
        host::set_velocity(5, 700);
        host::play(5, 70);
    } else {
        host::set_adsr(5, 1, 15, 0, 5);
        host::set_velocity(5, 500);
        host::play(5, 80);
    }
}
```

Each canonic voice's accent fires through `host::set_velocity`
immediately before its `host::play`, with the velocity choice
depending on whether the firing-count modulo the voice's meter
equals zero (accent) or not (normal). The accent pattern
distinguishes the voices auditorily even though they share
identical subject pitches across their registers.

## Harmonic content under polymetric canon

The four voices share the same subject pitches (G, A, B-flat,
C) at different octaves. The simultaneity at any given tick
is determined by which subject position each voice currently
occupies. Because the four voices proceed at strides 4, 3, 5,
and 7, the subject positions cycle independently and produce
all 4^3 = 64 distinct three-position combinations across the
non-reference voices (the reference voice contributes the
fourth voice's position as a function of tick). Over the
1680-tick superperiod, the script visits every combination
many times.

The subject's four pitches form an ascending tetrachord of G
Dorian. Any combination of these four pitches lies entirely
within the G Dorian harmonic field. The strongest dissonances
that arise are major-second intervals between adjacent
pitches (G against A, A against B-flat, B-flat against C);
these are mild and idiomatic for modal counterpoint. Sevenths
are absent because the subject does not include F natural or
E. Tritones are absent because the subject does not include
F-sharp or any chromatic alteration.

The harmonic field is therefore safe by construction. Any
simultaneity is either a unison or octave doubling, a fifth
or fourth, a third or sixth, or a mild major-second
dissonance. The four-voice polyphony does not produce
chromatic dissonance and does not require the resolution
discipline that strict species counterpoint demands.

At the superperiod boundary (loop-tick 0), all four voices
simultaneously land on subject position 0 (G). The piece
returns to a strong unison-and-octave tonic and repeats. This
return-to-tonic is the loop's structural punctuation.

## Coverage matrix

Phase 6 has a focused coverage profile similar to song 5. The
piece does not exercise per-tick dynamic feature use; the
aesthetic depends on stable instrument character and stable
metric grids.

| Native | Coverage |
|--------|----------|
| `host::set_enable` | Channels 0, 1, 4, 5, 6, 7 active at init. Channels 2 and 3 enabled at their entry ticks (128 and 192 respectively in the first iteration; tick 0 in subsequent iterations). Dynamic per-iteration. |
| `host::set_waveform` | Four distinct waveforms across the eight channels (Sawtooth, Triangle, Pulse, Sine, Noise). Static within the piece. |
| `host::set_duty` | Active on channels 2 and 3 at 250 and 750 respectively. Static within the piece. |
| `host::set_adsr` | Per-channel envelopes at init. Channel 5 percussion swaps per beat. |
| `host::set_volume` | Per-channel stereo positions at init. Static. Critical for voice identification. |
| `host::set_vibrato` | Active on channels 6 and 7 (drone pads) at slightly different rates for ensemble shimmer. Static. |
| `host::set_lpf` | Active on channel 4 (sub-bass) at 800 Hz. Static. |
| `host::set_retrigger` | Channels 0-4, 6, 7 retrigger off (legato). Channel 5 retrigger on (sharp transients). Static. |
| `host::set_detune` | Not used in active state. The canon is in twelve-tone equal temperament. |
| `host::set_velocity` | Per-voice metric accents. Dynamic per voice per beat. |
| `host::set_master_volume` | 800 at init. Static. |
| `host::set_bpm` | 120 per tick. Static value. |
| `host::song_name` | Called once in init. |
| `host::play` | Per-beat on each canonic voice; per-bar on the sub-bass pedal; per-beat on percussion. |
| `host::silence` | Not used. Voices are continuously active once entered. |

The dynamic feature use is concentrated in the per-voice
metric accent system. The piece does not modulate timbre,
filter, vibrato, or detune within the composition.

## Verification checklist

The song is complete when:

- Compiles via `cargo run -p keleusma-cli -- compile examples/scripts/piano_roll/piano_roll_6.kel`.
- Loads through `Vm::new` against the default arena without a `VerifyError`.
- A headless probe shows the four canonic voices firing on their expected beat positions. Voice 1 fires on every fourth tick from tick 0. Voice 2 fires on every fourth tick from tick 64 in the first iteration and from tick 0 in subsequent iterations. Voices 3 and 4 follow analogously.
- The audible texture begins with one voice and stacks to four voices over the first 192 ticks of the first iteration. Subsequent iterations begin with all four voices.
- The harmonic acceptability audit (described in the Verification section above) confirms no unacceptable simultaneities.
- Workspace tests, clippy, fmt, release build all clean.

## Sheet music feasibility

### Overview

Sheet music for a canon is fully standard notation. Polymetric
canons require one staff per voice with the meter signature
printed at the start of each voice's staff. The four staves are
bracketed together as a system.

### Notation solution

Each voice is printed on its own staff with its meter
signature: voice 1 in 4/4, voice 2 in 3/4, voice 3 in 5/4,
voice 4 in 7/4. The bar lines fall at different places in each
staff, producing the polymetric visual that is the score's
defining character.

A live performance requires either a click track that ticks at
each performer's metric rate (similar to the song 5 solution)
or a conductor who can simultaneously beat 4, 3, 5, and 7
patterns (which is essentially impossible). The implementation
engine bypasses both options.

### Master-score layout

| Staff | Channel | Role | Meter |
|-------|---------|------|-------|
| 1 | Channel 3 | Voice 4 canon | 7/4 |
| 2 | Channel 2 | Voice 3 canon | 5/4 |
| 3 | Channel 0 | Voice 1 canon | 4/4 |
| 4 | Channel 1 | Voice 2 canon | 3/4 |
| 5 | Channel 6 | Pad layer (G drone) | 4/4 |
| 6 | Channel 7 | Pad layer mirror (D drone) | 4/4 |
| 7 | Channel 4 | Sub-bass pedal | 4/4 |
| 8 | Channel 5 | Percussion | 4/4 |

The four canonic voices are placed adjacent to one another in
the order voice 4, voice 3, voice 1, voice 2 (descending by
meter complexity, which roughly matches descending by
register). The support layers are below.

### Verdict

Sheet music is fully feasible and the score is a useful
performance artefact for analytical purposes. Live performance
is impractical without a click track; the implementation engine
is the authentic realisation of the piece.

## Pending implementation

The script `examples/scripts/piano_roll/piano_roll_6.kel` is not yet implemented.
The specification above provides the structural and musical
content required for the script-author pass. The script will:

- Add `include_str!("piano_roll_6.kel")` to `SONG_SOURCES` at index 6 in `examples/piano_roll.rs`.
- Implement the `subject_pitch` lookup function.
- Implement the per-voice subject-position computation with entry-time gating.
- Implement the per-voice metric accent system.
- Implement the sub-bass pedal, drone pads, and percussion support layers.
- Update `docs/guide/PIANO_ROLL.md` to mention song 6.
- Update the module docstring in `examples/piano_roll.rs` to reflect the seven-song roster.
- Verify via headless probe, lib tests, clippy, fmt, and release build per the established discipline.

The implementation effort is estimated at approximately 400 to
600 lines of Keleusma source. The principal implementation
challenge is the per-voice metric accent logic and the entry-
gating that must handle both the first iteration (staggered
entries) and subsequent iterations (all four voices active
from tick 0). Per-voice constants (stride, entry, register
transposition, accent and normal velocities) are exposed
through small helper functions that the resolution of backlog
item B12 in the WCMU text-size analysis admits cleanly.

## Working title and song-name string

The composition's working title is "Quadrameter Canon". The
title names the central structural device (a canon in four
distinct meters). The host song-name string is
`"Keleusma Project: Quadrameter Canon (0BSD)"`, following the
license-tag convention established by songs 3 through 5.
