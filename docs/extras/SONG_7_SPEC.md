# Song 7 specification: Harmonic Garden

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document is the implementation specification for
`examples/scripts/piano_roll/piano_roll_7.kel` (pending implementation), the
first microtonal piece in the piano-roll roster. Song 7
demonstrates just-intonation pitches as continuous integer
cent offsets layered onto twelve-tone-equal-temperament MIDI
pitches. The piece builds an eight-voice stack of pure
harmonics on a single tonic (A) by activating one voice at a
time across eight sections, then resets at the loop boundary
and rebuilds. The aesthetic target is meditative drone music
in the spectral and microtonal traditions of the late
twentieth century.

See the long-form manual at
[`book/src/PIANO_ROLL.md`](../../book/src/PIANO_ROLL.md) for the
broader piano-roll context, and the prior specs at
[`SONG_3_SPEC.md`](./SONG_3_SPEC.md) through
[`SONG_6_SPEC.md`](./SONG_6_SPEC.md) for the other long-form
demonstrations.

## Role in the roster

Index 7 in `SONG_SOURCES`. Joins the existing roster behind the
`sdl3-example` and `text` cargo features. Accessible at runtime
through the `s` (cycle), `r` (restart), and `7` (direct select)
input commands. The input watcher requires no host change.

## High-level brief

Eight voices, eight pitches, one tonic. Each voice is assigned
a specific harmonic of the tonic frequency. Voice 0 plays the
fundamental (the tonic itself). Voices 1 through 7 play
harmonics 2, 3, 5, 7, 9, 11, 13 of the fundamental, expressed
as just-intonation intervals above the tonic and realised
through twelve-tone-equal-temperament MIDI pitches with
calculated cents-of-detune offsets.

The piece is structurally an additive stack. Section 0 carries
only the fundamental. Each subsequent section adds one voice
to the active set. By section 7 all eight voices are active
and the piece presents a complete eight-partial harmonic
spectrum sustained on the tonic. At the loop boundary all
non-fundamental voices disable simultaneously and the cycle
restarts with the fundamental alone.

Tempo is slow (60 beats-per-minute). Each section is 256 ticks
(approximately 64 seconds wall-clock). The full 2048-tick loop
body is approximately 8 minutes and 32 seconds. The piece
loops indefinitely. The slow pace allows the listener time to
hear each new harmonic enter the stack and to perceive how the
just-intonation pitch differs from its twelve-tone-equal-
temperament neighbour.

The composition is in A. The tonic frequency is A2 at 110 Hz
in 12-TET terms. The just-intonation pitches are constructed
as integer-ratio multiples of this fundamental and realised by
playing the nearest 12-TET pitch and applying a constant
cents-of-detune offset.

## Design intent and psychoacoustic strategy

### Just intonation and its difference from 12-TET

Twelve-tone equal temperament divides the octave into twelve
equal semitones, each measuring exactly 100 cents. The system
is a compromise; it permits modulation through all twelve
keys at the cost of mistuning every interval except the
octave. The perfect fifth is two cents flat, the major third
is fourteen cents sharp, the minor third sixteen cents flat,
and so on.

Just intonation derives intervals from integer ratios of
frequencies. The perfect fifth is exactly 3/2 (702 cents).
The major third is exactly 5/4 (386 cents). The harmonic
seventh is exactly 7/4 (969 cents). These intervals are
psychoacoustically "pure" because the partial frequencies of
two notes align without beating. The listener perceives
just-intonation chords as more consonant than tempered chords;
the difference is small but real.

This piece presents pure intervals directly, layered to form
a harmonic-series stack on a single tonic. The listener hears
each just interval as a clean, beat-free sonority. The
deliberate slowness of the piece gives the ear time to settle
on each interval and to notice how it differs from the
tempered version.

### Why this fits Keleusma

The host's `set_detune` native takes cents as a continuous
integer parameter. To realise a just-intonation pitch the
script calls `host::play` with the nearest twelve-tone-equal-
temperament MIDI value and then `host::set_detune` with the
cents offset required to land on the just pitch exactly. The
combination is mathematically precise. The audio thread
applies the detune as a multiplicative pitch factor and the
resulting frequency matches the just interval to within the
floating-point precision of the audio path.

Previous songs in the roster used `set_detune` only for
chorus thickness (songs 3 and 4 stereo unison pairs) and for
pitch-bend gestures (song 3 dive bomb). Song 7 uses
`set_detune` for primary pitch identity. Every voice's
fundamental frequency is determined jointly by its MIDI value
and its detune offset; neither alone is sufficient.

### Why this fits the roster

Songs 3 and 4 demonstrate the engine through maximalist
feature stacking. Song 5 demonstrates precision through phase
drift. Song 6 demonstrates polyphony through canonic
counterpoint. Song 7 demonstrates the engine's full-spectrum
pitch control through just intonation. The four pieces span
the engine's expressive dimensions: dynamics, precision,
polyphony, and pitch.

### Contrast with twelve-tone-equal-temperament repertoire

A listener accustomed to 12-TET will hear the just-intonation
intervals as slightly out of tune. The major third sounds
flat (386 cents instead of 400). The harmonic seventh sounds
very flat (969 instead of 1000 or 1100). The 11/8 interval
sounds neither sharp 4 nor flat 5 but something between them
(551 cents). The 13/8 interval sounds neither sharp 5 nor
flat 6 (841 cents).

The "out of tune" perception is in fact the listener's
12-TET-trained ear failing to recognise pure intervals. With
sustained tones and careful listening, the just intervals
reveal themselves as more stable than their tempered
counterparts; the beats that mark tempered intonation are
absent. The piece's slow tempo and long held notes are
deliberate; they give the listener time to make this
psychoacoustic adjustment.

### Listener experience

The intended experience is contemplative. The piece is not
melodic in the traditional sense. There is no theme, no
development, no resolution. There is only the stack of
harmonics building up across the loop and dissolving at the
boundary. The listener attends to the sonority itself, not to
musical events.

A listener who follows the additive structure will hear the
piece as a slow climb up the harmonic series. A listener who
does not will hear a slowly-thickening drone that periodically
resets to its fundamental and rebuilds. Both readings are
valid.

The piece is the slowest in the roster. It is also the
quietest in the sense of musical event density. The composition
trades surface activity for spectral depth.

## Design constraints

- Eight voices, eight pitches. Each voice is assigned a single just-intonation pitch and holds that pitch for the duration of its active region.
- Just-intonation tuning derived from integer ratios with the fundamental at A2 (12-TET MIDI 45, approximately 110 Hz).
- Continuous detune offsets. Each voice's pitch is realised through a 12-TET MIDI value plus a fixed cents-of-detune offset that places the voice's frequency on the just interval.
- Additive stack. Each section adds one voice to the active set. The piece progresses through eight sections, each enabling one additional voice. The fundamental (channel 0) plays in all sections.
- Loop reset. At the loop boundary (tick 2048), all non-fundamental voices disable simultaneously. The fundamental drone continues unbroken across the boundary. The next loop iteration rebuilds the stack from section 0.
- Slow tempo. 60 beats-per-minute throughout. The piece is contemplative and requires sustained tones for the just-intonation intervals to be perceptually clear.
- Long envelopes. All voices have long attack and release. Section transitions therefore feel like fade-ins rather than abrupt entries. The loop boundary feels like a fade-out and rebuilds.

## Just-intonation pitch table

The fundamental is A2 (12-TET MIDI 45, 110 Hz). Each voice
plays a specific harmonic of this fundamental. The just ratio
is converted to cents, the cents value is compared against the
nearest 12-TET pitch, and the difference becomes the detune
offset.

| Voice | Channel | Harmonic | Ratio | Cents from A2 | Nearest 12-TET pitch | MIDI | Detune (cents) |
|-------|---------|----------|-------|---------------|----------------------|------|----------------|
| 0 | 0 | 1st (fundamental) | 1/1 | 0 | A2 | 45 | 0 |
| 1 | 1 | 2nd (octave) | 2/1 | 1200 | A3 | 57 | 0 |
| 2 | 2 | 3rd (perfect fifth + octave) | 3/2 above A3 = E4 territory; in this piece simplified to 3/2 from A2 in upper octave | 702 from A2 | E3 | 52 | +2 |
| 3 | 3 | 5th (major third + two octaves) | 5/4 from A3 | 386 from A3 = 1586 from A2; here just 5/4 from A2 | C-sharp-3 | 49 | -14 |
| 4 | 4 | 7th (harmonic seventh) | 7/4 from A2 | 969 from A2 | G3 | 55 | -31 |
| 5 | 5 | 9th (major second above 8th harmonic) | 9/8 from A2 | 204 from A2 | B2 | 47 | +4 |
| 6 | 6 | 11th | 11/8 from A2 | 551 from A2 | D3 | 50 | +51 |
| 7 | 7 | 13th | 13/8 from A2 | 841 from A2 | F3 | 53 | +41 |

The detune offsets are integer cents and stable. The script
sets the detune once per voice in the init block and never
changes it during playback.

### Detune calculation reference

The cents value for ratio `r` is `1200 * log2(r)`. For the
ratios used:

- 1/1: 0 cents
- 2/1: 1200 cents (octave, no detune needed)
- 3/2: 1200 * log2(1.5) = 1200 * 0.5849625 = 701.955 cents (round to 702, 12-TET E is 700, detune +2)
- 5/4: 1200 * log2(1.25) = 1200 * 0.3219281 = 386.314 cents (round to 386, 12-TET C-sharp is 400, detune -14)
- 7/4: 1200 * log2(1.75) = 1200 * 0.8073549 = 968.826 cents (round to 969, 12-TET G is 1000, detune -31)
- 9/8: 1200 * log2(1.125) = 1200 * 0.1699250 = 203.910 cents (round to 204, 12-TET B is 200, detune +4)
- 11/8: 1200 * log2(1.375) = 1200 * 0.4594316 = 551.318 cents (round to 551, 12-TET D is 500, detune +51)
- 13/8: 1200 * log2(1.625) = 1200 * 0.7004397 = 840.528 cents (round to 841, 12-TET F is 800, detune +41)

The cents values round to the nearest integer because
`host::set_detune` takes an integer parameter. The rounding
introduces at most 0.5 cents of imprecision per voice, which
is below the threshold at which most listeners perceive
intonation error.

## Section structure

Loop body is 2048 ticks divided into eight 256-tick sections.
Each section is 64 seconds at 60 BPM. Each section adds one
voice to the active stack.

| Section | Ticks | Active channels | Active partials | Total harmonic content |
|---------|-------|-----------------|------------------|-------------------------|
| 0 | 0..256 | 0 | 1 | Fundamental alone (A2) |
| 1 | 256..512 | 0, 1 | 1, 2 | Octave drone |
| 2 | 512..768 | 0, 1, 2 | 1, 2, 3 | Open fifth chord |
| 3 | 768..1024 | 0, 1, 2, 3 | 1, 2, 3, 5 | Pure major triad |
| 4 | 1024..1280 | 0, 1, 2, 3, 4 | 1, 2, 3, 5, 7 | Harmonic seventh chord |
| 5 | 1280..1536 | 0, 1, 2, 3, 4, 5 | 1, 2, 3, 5, 7, 9 | 9-limit chord |
| 6 | 1536..1792 | 0, 1, 2, 3, 4, 5, 6 | 1, 2, 3, 5, 7, 9, 11 | 11-limit chord |
| 7 | 1792..2048 | 0, 1, 2, 3, 4, 5, 6, 7 | 1, 2, 3, 5, 7, 9, 11, 13 | Full 13-limit harmonic stack |

At the loop boundary (tick 2048), channels 1 through 7 are
disabled and the piece returns to section 0 (fundamental
alone). Channel 0 remains enabled throughout, including across
the loop boundary, so the fundamental drone is unbroken.

## Loop boundary mapping

The runtime computes `loop_tick = input mod 2048` for `input >=
0`. The script gates voice activations on `loop_tick` rather
than on absolute `input` because all loop iterations are
identical (no "first iteration" stagger as in song 6). The
host's tick-reset on song load also produces this directly.

## Channel assignments

| Channel | Role | Just pitch | Waveform | Stereo position | ADSR (a/d/s/r) |
|---------|------|------------|----------|-----------------|-----------------|
| 0 | Fundamental drone | A2 | Sine (code 3) | Centre (500/500) | (500, 500, 800, 1000) |
| 1 | Octave above | A3 | Sine (code 3) | Centre slight-left (550/450) | (500, 500, 800, 1000) |
| 2 | Just perfect fifth | E3 + 2 cents | Triangle (code 1) | Left (700/300) | (800, 600, 750, 1200) |
| 3 | Just major third | C#3 - 14 cents | Triangle (code 1) | Right (300/700) | (800, 600, 750, 1200) |
| 4 | Harmonic seventh | G3 - 31 cents | Sawtooth (code 2) | Left of centre (650/350) | (1000, 700, 700, 1500) |
| 5 | Just ninth | B2 + 4 cents | Sawtooth (code 2) | Right of centre (350/650) | (1000, 700, 700, 1500) |
| 6 | 11th harmonic | D3 + 51 cents | Pulse with duty 250 (code 4) | Far left (850/150) | (1200, 800, 650, 1800) |
| 7 | 13th harmonic | F3 + 41 cents | Pulse with duty 750 (code 4) | Far right (150/850) | (1200, 800, 650, 1800) |

The ADSR envelopes use long attack and release values so that
voice activations and deactivations fade in and out rather
than starting and stopping abruptly. The attack values range
from 500 milliseconds (drones) to 1200 milliseconds (upper
harmonics). The release values range from 1000 milliseconds
to 1800 milliseconds. Sustain values are high (650 to 800 of
q1000) so the voices remain present once attacked.

The stereo positions place adjacent harmonics on alternating
sides of the field. The fundamental is centred. The octave is
slightly left. The fifth is more left. The major third is
counterpoint-right. The pattern continues outward. The
listener can localise individual harmonics in space, which
helps the ear separate them despite their tight pitch
proximity.

Different waveforms across the channels provide timbral
contrast. The lower harmonics use Sine and Triangle for
warmth. The middle harmonics use Sawtooth for definition. The
upper harmonics (11th and 13th) use Pulse waveforms with
contrasting duty cycles for their more dissonant character.

## Mid-song event schedule

### Init block

```
host::song_name("Keleusma Project: Harmonic Garden (0BSD)");
host::set_bpm(60);
host::set_master_volume(750);

// Channel 0: Fundamental A2 drone. Centre.
host::set_waveform(0, 3);
host::set_adsr(0, 500, 500, 800, 1000);
host::set_volume(0, 500, 500);
host::set_velocity(0, 700);
host::set_retrigger(0, 0);
host::set_vibrato(0, 0, 0);
host::set_lpf(0, 0);
host::set_detune(0, 0);
host::set_enable(0, 1);

// Channel 1: Octave A3.
host::set_waveform(1, 3);
host::set_adsr(1, 500, 500, 800, 1000);
host::set_volume(1, 550, 450);
host::set_velocity(1, 650);
host::set_retrigger(1, 0);
host::set_vibrato(1, 0, 0);
host::set_lpf(1, 0);
host::set_detune(1, 0);
host::set_enable(1, 0);

// Channel 2: Just perfect fifth E3 (+2 cents).
host::set_waveform(2, 1);
host::set_adsr(2, 800, 600, 750, 1200);
host::set_volume(2, 700, 300);
host::set_velocity(2, 600);
host::set_retrigger(2, 0);
host::set_vibrato(2, 0, 0);
host::set_lpf(2, 0);
host::set_detune(2, 2);
host::set_enable(2, 0);

// Channel 3: Just major third C#3 (-14 cents).
host::set_waveform(3, 1);
host::set_adsr(3, 800, 600, 750, 1200);
host::set_volume(3, 300, 700);
host::set_velocity(3, 600);
host::set_retrigger(3, 0);
host::set_vibrato(3, 0, 0);
host::set_lpf(3, 0);
host::set_detune(3, -14);
host::set_enable(3, 0);

// Channel 4: Harmonic seventh G3 (-31 cents).
host::set_waveform(4, 2);
host::set_adsr(4, 1000, 700, 700, 1500);
host::set_volume(4, 650, 350);
host::set_velocity(4, 550);
host::set_retrigger(4, 0);
host::set_vibrato(4, 0, 0);
host::set_lpf(4, 2000);
host::set_detune(4, -31);
host::set_enable(4, 0);

// Channel 5: Just ninth B2 (+4 cents).
host::set_waveform(5, 2);
host::set_adsr(5, 1000, 700, 700, 1500);
host::set_volume(5, 350, 650);
host::set_velocity(5, 550);
host::set_retrigger(5, 0);
host::set_vibrato(5, 0, 0);
host::set_lpf(5, 2000);
host::set_detune(5, 4);
host::set_enable(5, 0);

// Channel 6: 11th harmonic D3 (+51 cents).
host::set_waveform(6, 4);
host::set_duty(6, 250);
host::set_adsr(6, 1200, 800, 650, 1800);
host::set_volume(6, 850, 150);
host::set_velocity(6, 500);
host::set_retrigger(6, 0);
host::set_vibrato(6, 0, 0);
host::set_lpf(6, 2500);
host::set_detune(6, 51);
host::set_enable(6, 0);

// Channel 7: 13th harmonic F3 (+41 cents).
host::set_waveform(7, 4);
host::set_duty(7, 750);
host::set_adsr(7, 1200, 800, 650, 1800);
host::set_volume(7, 150, 850);
host::set_velocity(7, 500);
host::set_retrigger(7, 0);
host::set_vibrato(7, 0, 0);
host::set_lpf(7, 2500);
host::set_detune(7, 41);
host::set_enable(7, 0);

// Fire the fundamental drone immediately. All other voices
// fire when their section's enable transition happens.
host::play(0, 45);
```

### Per-tick events

```
host::set_bpm(60);

let loop_tick = input mod 2048;

// Section transitions: enable a voice at the start of each
// section and fire its play. The enable and play happen on a
// single tick once per loop.

if loop_tick == 256 {
    host::set_enable(1, 1);
    host::play(1, 57);
}
if loop_tick == 512 {
    host::set_enable(2, 1);
    host::play(2, 52);
}
if loop_tick == 768 {
    host::set_enable(3, 1);
    host::play(3, 49);
}
if loop_tick == 1024 {
    host::set_enable(4, 1);
    host::play(4, 55);
}
if loop_tick == 1280 {
    host::set_enable(5, 1);
    host::play(5, 47);
}
if loop_tick == 1536 {
    host::set_enable(6, 1);
    host::play(6, 50);
}
if loop_tick == 1792 {
    host::set_enable(7, 1);
    host::play(7, 53);
}

// Loop boundary: disable channels 1 through 7. Channel 0
// continues unbroken.
if loop_tick == 0 and input > 0 {
    host::set_enable(1, 0);
    host::set_enable(2, 0);
    host::set_enable(3, 0);
    host::set_enable(4, 0);
    host::set_enable(5, 0);
    host::set_enable(6, 0);
    host::set_enable(7, 0);
}
```

The per-tick body is extremely small. Most ticks fire no
voice events at all. Only the eight section-onset ticks and
the loop-boundary tick produce native calls beyond
`host::set_bpm`. This is the simplest per-tick body in the
roster.

Per the lesson from backlog item B12, the section-transition
gates use direct equality comparisons against constants rather
than helper-function dispatch, so the WCMU analysis can bound
the per-tick top-of-arena tightly.

## Coverage matrix

Song 7 has the narrowest coverage profile in the roster. The
piece's aesthetic depends on stable pitches and stable
timbres. Dynamic feature use would defeat the contemplative
character.

| Native | Coverage |
|--------|----------|
| `host::set_enable` | Channel 0 active at init. Channels 1 through 7 enabled at their section-onset ticks. All non-fundamental channels disabled at loop boundary. Dynamic per-section. |
| `host::set_waveform` | Four distinct waveforms across the channels (Sine, Triangle, Sawtooth, Pulse). Static within the piece. |
| `host::set_duty` | Active on channels 6 and 7 at 250 and 750. Static. |
| `host::set_adsr` | Per-channel envelopes set at init. Long attack and release values are critical for the fade-in / fade-out character of section transitions. Static. |
| `host::set_volume` | Per-channel stereo positions set at init. Static. The stereo image is critical for perceptual separation of the eight harmonics. |
| `host::set_vibrato` | Not used (all channels at zero). Vibrato would obscure the just-intonation pitch identity. |
| `host::set_lpf` | Active on channels 4 through 7 at progressively higher cutoff values. Static. Filters the upper harmonics to soften their character. |
| `host::set_retrigger` | All channels at zero (legato). The drone aesthetic requires legato. |
| `host::set_detune` | Active on channels 2 through 7 with specific integer cents values (+2, -14, -31, +4, +51, +41). Static. The detune is the primary pitch-identity mechanism for the just-intonation realisation. |
| `host::set_velocity` | Per-channel base velocities at init. Static. |
| `host::set_master_volume` | 750 at init. Static. |
| `host::set_bpm` | 60 per tick. Static value. |
| `host::song_name` | Called once in init. |
| `host::play` | Once per channel at the channel's first activation. The channels are subsequently silent in the host::play sense but continue sustaining through their ADSR envelopes. |
| `host::silence` | Not used. Voices are disabled via set_enable at the loop boundary rather than silenced via host::silence. |

The dynamic feature use is concentrated in the channel-enable
transitions. The piece is the roster's "deep dive on
microtonal pitch identity" exhibit, analogous to song 5's
"deep dive on phase music" and complementary to songs 3 and
4's exhaustive coverage exhibits.

## Verification checklist

The song is complete when:

- Compiles via `cargo run -p keleusma-cli -- compile examples/scripts/piano_roll/piano_roll_7.kel`.
- Loads through `Vm::new` against the default arena without a `VerifyError`.
- A headless probe shows the fundamental drone firing at tick 0 and persisting through the loop boundary. The eight section-onset ticks produce the expected `host::set_enable(N, 1)` and `host::play(N, pitch)` pairs. The loop-boundary tick produces seven `host::set_enable(N, 0)` calls for channels 1 through 7.
- The audible texture begins with the fundamental alone and builds through the eight sections to the full harmonic stack. The just-intonation intervals should sound noticeably different from their 12-TET counterparts when sustained at the slow tempo. Specifically, the harmonic seventh (channel 4) should sound clearly flat compared to a 12-TET seventh, the 11/8 (channel 6) should sound clearly between sharp-4 and flat-5, and the 13/8 (channel 7) should sound clearly between sharp-5 and flat-6.
- The loop boundary produces a clean fade: the seven non-fundamental voices release their envelopes over 1000 to 1800 milliseconds while channel 0 continues unbroken. The next iteration begins with the fundamental alone and rebuilds.
- Workspace tests, clippy, fmt, release build all clean.

## Sheet music feasibility

### Overview

Sheet music for microtonal music has a well-developed
notational convention. The standard approach uses quarter-tone
accidentals (typically a backwards flat sign for quarter-flat,
a half-sharp for quarter-sharp, and double-stroke variants for
three-quarter-tone adjustments) combined with explicit cents
annotations for precise tuning.

For an additive drone piece like song 7, the score is
extremely simple. Each voice has a single sustained pitch.
There is no melody, no rhythm beyond the section transitions,
and no metric structure beyond the steady 60 BPM. A
performance score would consist of eight horizontal staff
lines, one per voice, with each voice's single sustained pitch
notated as a whole note tied across the duration of its active
section. Vertical text labels mark the section boundaries.

### Notation solution for the just-intonation pitches

Each voice's pitch is annotated with the just-intonation ratio
and the cents-of-detune offset. For example, channel 4
(harmonic seventh) is notated as "G3, harmonic seventh of A2,
7/4 (-31 cents)". The performer (or rather, the implementation
engine) reads the ratio and the cents annotation together; the
ratio is the conceptual identity, the cents is the
realisation.

For a live performance with acoustic instruments capable of
microtonal pitch (string instruments, voice, fretless wind
instruments), the score's cents annotations guide the player.
The 7/4 interval at -31 cents from G is reachable by stringed-
instrument players through deliberately-flat fingering. The
11/8 interval at +51 cents from D is at the midpoint between
D and D-sharp and requires specifically microtonal technique.

### Master-score layout

| Staff | Channel | Pitch (just) | Pitch (12-TET realisation) | Cents detune |
|-------|---------|--------------|----------------------------|--------------|
| 1 | Channel 7 | 13/8 of A2 | F3 | +41 |
| 2 | Channel 6 | 11/8 of A2 | D3 | +51 |
| 3 | Channel 5 | 9/8 of A2 | B2 | +4 |
| 4 | Channel 4 | 7/4 of A2 | G3 | -31 |
| 5 | Channel 3 | 5/4 of A2 | C-sharp-3 | -14 |
| 6 | Channel 2 | 3/2 of A2 | E3 | +2 |
| 7 | Channel 1 | 2/1 of A2 | A3 | 0 |
| 8 | Channel 0 | 1/1 of A2 | A2 | 0 |

The staves are arranged in descending order of pitch position
within the harmonic series (top to bottom: highest harmonic to
fundamental). The fundamental is at the bottom as the visual
foundation.

A "harmonic series staff" notation, sometimes used for
spectral music, would alternatively place each voice on a
diagram of the harmonic series rather than on a traditional
five-line staff. The choice of notational convention depends
on the performer's training; a player from the spectral-music
tradition would find the harmonic-series diagram more
immediate.

### Verdict

Sheet music is feasible and the score is genuinely useful for
analytical study. A live performance by an ensemble of
acoustic microtonal-capable instruments is plausible if not
common. The implementation engine remains the most authentic
realisation because its pitch precision is exact at the
integer-cent level, while a live ensemble would introduce
intonation drift that, for this specific piece, would
compromise the demonstration's clarity.

## Pending implementation

The script `examples/scripts/piano_roll/piano_roll_7.kel` is not yet implemented.
The specification above provides the structural and musical
content required for the script-author pass. The script will:

- Add `include_str!("piano_roll_7.kel")` to `SONG_SOURCES` at index 7 in `examples/piano_roll.rs`.
- Implement the init block per the schedule above, configuring all eight channels with their assigned waveform, ADSR, stereo position, and detune offset.
- Implement the per-tick body with the eight section-onset triggers and the loop-boundary disable.
- Update `book/src/PIANO_ROLL.md` to mention song 7.
- Update the module docstring in `examples/piano_roll.rs` to reflect the eight-song roster.
- Verify via headless probe, lib tests, clippy, fmt, and release build per the established discipline.

The implementation effort is estimated at approximately 200 to
300 lines of Keleusma source, similar in scale to song 5 and
smaller than song 6. The per-tick body is essentially a series
of integer comparisons against constant tick values; no
helper functions are required.

## Working title and song-name string

The composition's working title is "Harmonic Garden". The
title continues the botanical motif from song 5 ("Phase
Garden") and describes the piece's cultivation of the harmonic
series. The garden metaphor is apt because the eight harmonics
grow in place; they do not move; the piece tends to them as a
garden is tended.

The host song-name string is
`"Keleusma Project: Harmonic Garden (0BSD)"`, following the
license-tag convention established by songs 3 through 6.
