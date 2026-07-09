# Song 4 specification: Pendulum Chaconne in D minor

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document is the implementation specification for
`examples/scripts/piano_roll/piano_roll_4.kel` (pending implementation), the
second long-form full-matrix demonstration in the piano-roll
example. Song 4 differs from song 3 in three fundamental ways:
the tempo is under continuous sine-wave modulation across the
entire composition rather than under segmented ramps, the
composition uses an explicit loop-iteration mutation state
machine in the manner of a baroque chaconne, and the aesthetic
target is high-energy gothic-classical-metal rather than
generic 16-bit boss theme.

See the long-form manual at
[`book/src/PIANO_ROLL.md`](../../book/src/PIANO_ROLL.md) for the
broader piano-roll context, and the song 3 spec at
[`SONG_3_SPEC.md`](./SONG_3_SPEC.md) for the prior full-matrix
specification that song 4 builds on.

## Role in the roster

Index 4 in `SONG_SOURCES`. Joins the existing roster behind the
`sdl3-example` and `text` cargo features. Accessible at runtime
through the `s` (cycle), `r` (restart), and `4` (direct select)
input commands. The input watcher requires no host change.

## High-level brief

A composition that runs the tempo under continuous sine-wave
modulation between 60 beats-per-minute and 300 beats-per-minute,
producing a perceptual "elastic" effect where the listener
feels gravitational pull at the inflection points and slack at
the peaks and troughs. The tempo trajectory is the primary
expressive vector; the harmonic and rhythmic content is the
secondary vector.

Structurally the composition is an algorithmic chaconne. A
fixed 64-bar loop body (1024 ticks) repeats indefinitely. The
loop iteration count modulates the upper voices, the waveforms,
the scale system, and the percussion intensity. The bassline
and the chord skeleton remain near-constant across iterations,
matching the chaconne convention. Four iteration variations
cycle: Awakening, Descent, Malfunction, and Apocalypse. The
cycle resets every four iterations.

The aesthetic target is unsettling high-energy gothic-classical
metal. At 60 beats-per-minute the music reads as a funeral
march of church-organ pads stacked with vibrato. At 300
beats-per-minute the music reads as hyper-speed neoclassical
shred against a wall of double-bass percussion. At the sine
inflection points (180 beats-per-minute) the music passes
through a perceptual middle ground where neither stasis nor
frenzy dominates.

Approximate listening lengths.

- Iteration 0 only (accelerating from 180 BPM through 300 BPM and back to 180 BPM): approximately 30 to 45 seconds.
- Iteration 1 only (decelerating from 180 BPM through 60 BPM and back to 180 BPM): approximately 90 to 120 seconds because the trough at 60 BPM stretches the perceived bar length.
- One full meta-loop (all four iterations): approximately 4 to 6 minutes.
- The composition loops indefinitely until the user swaps or quits.

## Design intent and psychoacoustic strategy

This section articulates the unifying goal of the composition.
The mechanisms documented in the sections that follow are
individually justifiable on musical grounds, but the goal that
ties them together is a specific psychoacoustic effect. The
goal is structured disorientation rather than chaos. Every
mechanism in the spec serves either as a destabilising vector
or as a stabilising anchor, and the interaction between the
two is the source of the piece's character.

### The stable and the destabilising

The composition operates on two axes simultaneously.

Stable axes. The chord skeleton across all four iterations is
invariant. The root sequence of the bassline, the harmonic
function of each chord, the four-section structural layout
(Loop A, Loop B, Loop A-prime, Loop C), the 64-bar loop length,
and the time signature (4/4 throughout) are constants. The
listener who tracks these elements has a reliable foothold in
the piece. After a few iterations the ear locks onto the
harmonic progression and can anticipate chord arrivals.

Destabilising axes. The tempo is under continuous sine-wave
modulation and never settles. The active scale changes per
iteration through a sequence of progressively less stable mode
systems. The lead voice changes waveform and ADSR character
per iteration. The detune state of channels 4, 6, and 7 ranges
from narrow chorus thickening to wide microtonal collapse. The
percussion intensity escalates within iteration 3. These
elements work against the listener's ability to predict the
surface of the music.

The chaconne form is the carrier of this design. A chaconne
historically keeps the bassline constant while the upper
voices vary. The form was chosen here because it permits
maximal surface variation against a stable foundation. Without
the foundation the piece would dissolve into pure texture and
no anchor would remain.

### Why these specific scales

The four iterations rotate through D natural minor, D Phrygian
dominant, D whole-tone, and D Locrian. The choice is not
arbitrary. The scales are arranged in order of decreasing
tonic stability.

D natural minor (iteration 0). The most stable. A perfect
fifth above the tonic, a clear leading tone available through
the harmonic-minor pivot, and a standard cadential function on
V. The listener can locate the tonic immediately.

D Phrygian dominant (iteration 1). The flat-2 (E-flat) and
raised-3 (F-sharp) create exotic intervals against the tonic.
The mode retains a perfect fifth above the tonic (A) so a
sense of root remains, but the modal colour is unsettled.

D whole-tone (iteration 2). All intervals from the tonic are
augmented. There is no perfect fifth above D. There is no
leading tone. There is no tonic function in the traditional
sense because every note of the scale is equidistant from
every other note. The scale is rotationally symmetric, which
means any of its six pitches could serve as the tonic. The
listener loses the ability to identify a centre.

D Locrian (iteration 3). The flat-5 (A-flat) replaces the
perfect fifth above the tonic with a tritone. Locrian is the
only diatonic mode without a stable tonic function in
traditional tonal practice because the V chord is diminished.
There is no traditional resolution. The mode is mathematically
diatonic but tonally unstable.

The progression from iteration 0 to iteration 3 walks the
listener from tonal certainty to tonal collapse.

### Why these specific detune behaviours

Each iteration's detune configuration corresponds to a state
on the same axis.

Iteration 0 narrow stereo unison at plus or minus 15 cents.
The detune is below the threshold at which the ear perceives
intonation error. It reads as ensemble thickness, not as
out-of-tune playing.

Iteration 1 narrow stereo unison at plus or minus 15 cents.
Carried forward from iteration 0 unchanged. The pitch axis is
stable while the rhythmic and waveform axes destabilise.

Iteration 2 wide stereo unison at plus or minus 30 cents
combined with per-tick detune oscillation on channel 4 ranging
plus or minus 50 cents. The 30-cent detune is at the threshold
where the ear begins to perceive detuning as separate pitches
rather than as chorus. The oscillating detune on channel 4
produces a mechanically distressed bass that audibly tears
against itself.

Iteration 3 asymmetric detune at plus 50 cents on the left
twin and minus 50 cents on the right twin combined with a slow
linear ramp on channel 4 from 0 to minus 100 cents across the
iteration. The 50-cent offsets are clearly out of tune. The
channel 4 ramp is a slow tuning collapse that takes the bass
half a semitone flat by the end of the iteration. The ear
cannot resolve the resulting harmonic field into a single
tonic.

### The orthogonality of tempo phase and content cycle

The sine wave period is two loop iterations and the content
mutation cycle is four loop iterations. The two periods are
deliberately incommensurate over short windows. The
consequence is that any given musical event recurs in four
distinct physical-delivery contexts before the meta-loop
returns to its start.

Iteration 0 plays the Loop A material at accelerating tempo
peaking near 300 BPM. Iteration 2 plays the same material with
the same harmonic skeleton at accelerating tempo, but with
whole-tone scale, Pulse waveform, and blast-beat percussion.
The two iterations share the same tempo trajectory and the
same chord roots but differ on every other axis. The listener
recognises the bass progression but not the surface.

Conversely, iteration 0 and iteration 2 share the rising tempo
phase; iterations 1 and 3 share the falling tempo phase.
Iteration 1 plays the Loop A material at decelerating tempo
with Phrygian dominant scale, Square waveform, and double-bass
percussion. The listener who locked onto the chord progression
during iteration 0 will recognise the chords in iteration 1
but encounter them with inverted physical delivery. The
sensation is of hearing the same phrase pulled backward
through molasses.

The four iteration-and-tempo combinations form a two-by-two
matrix that the listener cannot easily collapse into a single
abstract performance of the piece.

### Contrast with song 3

Song 3 and song 4 share several mechanisms (sine arithmetic
notwithstanding) including the eight-channel layout, three
doubling techniques, dynamic ADSR on the percussion channel,
and per-tick BPM updates during tempo ramps. The two songs
differ in how they handle the listener's ability to anticipate
musical events.

Song 3 partitions disorienting passages into named sections
with hard boundaries. The Chaos phases announce themselves
through tempo snaps, scale changes, and instrumentation morphs.
The Pre-Snap whole-tone gesture is bracketed by the dive bomb
on the lead voice. The listener understands when the piece
enters disorienting territory and when it exits. Song 3
respects the convention that aggressive material lives in
clearly demarcated zones.

Song 4 refuses that convention. There are no tempo snaps; the
tempo is in continuous motion. The iteration boundaries are
bar-aligned events embedded inside the modulating timeline,
not sectional articulations. A first-time listener will likely
perceive song 4 as a continuous wave of mutating material
rather than as a four-part variation cycle. The recognition of
the iteration structure requires repeated listening or
analytical reading of the spec.

This is the central distinction. Song 3 is structured as a
journey with named stations. Song 4 is structured as a single
unbroken phenomenon under continuous transformation, anchored
only by the invariant chord skeleton.

### Listener experience summary

The intended experience is that the piece feels both
inexorable and inscrutable. Inexorable because the underlying
mathematics never stops; the sine wave never lands on a final
tempo, the iteration counter never freezes on a final scale,
and the chord progression never resolves to a final tonic in
the sense of stopping. Inscrutable because the surface
mutations exceed the listener's predictive capacity within a
single hearing.

The chaconne foundation prevents this from becoming
unlistenable. The listener has the chord progression to hold
onto. The rest of the piece swirls around that anchor.

## Design constraints

- Full host-native matrix in active, inactive, and dynamic
  states across the composition. This includes
  `host::set_vibrato`, `host::set_volume` per-speaker stereo,
  `host::set_master_volume`, and the Noise waveform. Coverage
  matrix at the end of this document audits the assignment.
- All eight host channels active somewhere in the composition.
- Intro followed by infinite loop. The intro plays once on song
  load; subsequent iterations enter directly at the loop body's
  first tick. The host's tick-reset behaviour (see the song 3
  spec) ensures this works through hot-swap as well as cold
  start.
- Sine-wave tempo modulation between 60 and 300 BPM with center
  at 180 BPM and amplitude 120 BPM. Per-tick `host::set_bpm`
  updates following the "NES strategy of updating registers
  every tick" already in force for song 3.
- Loop-iteration mutation. The script reads `state.loop_count`
  on every tick and dispatches to one of four iteration
  variations. The same chord skeleton is used in all four; the
  variations affect waveforms, scales, lead-voice character,
  and percussion intensity.
- Sine period locked to a multiple of the loop length so the
  tempo phase aligns with the loop boundary deterministically.
  The chosen period is twice the loop length, so even-numbered
  iterations enter the loop in the rising phase (acceleration
  into 300 BPM) and odd-numbered iterations enter in the
  falling phase (deceleration into 60 BPM). Iteration content
  variation runs at four-iteration period, orthogonal to the
  two-iteration tempo phase, giving four distinct
  emotion-and-tempo combinations.
- Three doubling techniques exercised, each at a distinct
  placement: stereo unison with detune, detuned doubling at the
  octave for the bass stack, and parallel-interval
  harmonization in the lead voice during specific iterations.
  Same techniques as song 3, in different musical contexts.

## Section structure

Intro plus 1024-tick loop body. Loop body subdivides into four
16-bar sub-sections of 256 ticks each: A, B, A-prime, and C.
First-iteration ticks below.

| Section | Ticks (first iteration) | Bars | Description |
|---------|-------------------------|------|-------------|
| Intro | 0..128 | 8 (4/4) | One-shot opener. Bassline establishes D minor pedal. Sine tempo not yet engaged; tempo holds at 90 BPM ramping to 180 BPM by the loop entry. |
| Loop A | 128..384 | 16 (4/4) | Primary thematic material. Chord skeleton: i, VI, VII, V cycle in D minor. |
| Loop B | 384..640 | 16 (4/4) | Contrasting harmonic motion: ii-half-diminished, V, VI, iv. Gothic suspensions. |
| Loop A' | 640..896 | 16 (4/4) | A material returns, transposed up a perfect fourth (to G minor area) for harmonic motion. Returns to D minor at bar end. |
| Loop C | 896..1152 | 16 (4/4) | Cadential climax. Cycle of fifths arc returning to D minor. |

Loop boundary mapping. The runtime computes
`loop_tick = (input - 128) mod 1024` for `input >= 128`. At
`input == 1152` the formula gives `loop_tick = 0`, returning to
Loop A. The intro plays once on first iteration only.

## Tempo modulation engine

### The pendulum

A sine wave with period 2048 ticks (two loop iterations)
modulates the tempo between 60 and 300 beats-per-minute.

Center: 180 BPM. Amplitude: 120 BPM. The formula is

```
bpm(t) = 180 + 120 * sin(2 * pi * (t - 128) / 2048)
```

where `t` is the absolute input tick. The intro at `t < 128`
uses a separate accelerando described below.

### Sine approximation

Keleusma has no native trigonometric functions. The script
approximates `sin` using a parabolic curve indexed by the phase
position. The approximation is shape-correct (smooth, peaks at
the right phase, zero-crossings at the right phase) but is not
a true sine; the audible difference for tempo modulation at
this rate is negligible.

For phase `p` in `[0, 2048)`:

```
half = p mod 1024
sign = if p < 1024 { +1 } else { -1 }
sine_q1000 = sign * half * (1024 - half) / 262
```

The constant 262 is chosen so the peak value
`(512 * 512) / 262 = 1000` produces a unit-scale output. The
output range is `[-1000, +1000]`, interpreted as `q1000`.

Tempo formula:

```
bpm = 180 + (sine_q1000 * 120) / 1000
```

Integer arithmetic produces output in `[60, 300]` BPM. Per-tick
update via `host::set_bpm(bpm)`.

### Phase alignment with the loop

Loop length is 1024 ticks. Sine period is 2048 ticks. The
relationship is:

- Loop iteration 0 entry: sine phase = 0. Tempo = 180 BPM. The first quarter of iteration 0 accelerates toward 300 BPM at phase π/2 (loop-tick 512). The second half decelerates back to 180 BPM by phase π (loop-tick 1024).
- Loop iteration 1 entry: sine phase = π. Tempo = 180 BPM. The first quarter decelerates toward 60 BPM at phase 3π/2 (loop-tick 512 within iteration 1, absolute phase 1536). The second half accelerates back to 180 BPM.
- Loop iteration 2 entry: sine phase = 2π = 0. Same as iteration 0.
- Loop iteration 3 entry: same as iteration 1.

The sine phase pattern is therefore even-iterations-accelerate
and odd-iterations-decelerate.

### Intro tempo

The intro is not on the sine modulation. It uses a linear
accelerando from 90 BPM at tick 0 to 180 BPM at tick 128. This
matches the tempo at the start of the loop body, so the
transition into the sine pendulum is seamless.

Formula:

```
bpm_intro(t) = 90 + t * 90 / 128
```

### Tempo extremes and listening effects

- 60 BPM (sine trough, loop iteration 1 mid-point): tick rate
  is 60 milliseconds * 4 / 60 = 1000 milliseconds per beat
  divided by 4 sixteenth ticks per beat = 250 milliseconds per
  tick. Sixteenth notes are heard as deliberate, monumental
  events. Long-decay pads (the church-organ stack on channel
  3) breathe across multi-second envelopes.
- 180 BPM (sine zero-crossings, inflection points): standard
  rock tempo. The acceleration is at its maximum rate here.
- 300 BPM (sine peak, loop iteration 0 mid-point): tick rate is
  50 milliseconds per tick. Sixteenth-note arpeggios fire at
  20 Hz, which is the lower bound of pitched hearing. Rhythmic
  arpeggio sequences smear into low-frequency timbres at this
  rate, producing a buzzing wall-of-sound effect. As the tempo
  descends from 300 back through 240 BPM (16 Hz arpeggio
  rate), the rhythmic identity resolves out of the timbre.

## Loop-iteration mutation matrix

Four iterations cycle on `state.loop_count` modulo 4. Each
iteration has a distinct character defined by its waveforms,
its scale system, and its percussion intensity. The chord
skeleton and bassline are near-constant across all four to
preserve the chaconne identity.

| Iteration | Name | Tempo phase | Primary scale | Lead waveform | Percussion | Detune state |
|-----------|------|-------------|---------------|---------------|------------|--------------|
| 0 | Awakening | Accelerating | D natural minor with harmonic-minor pivot on V | Sawtooth (clean) | Standard backbeat | Stereo unison ±15 cents |
| 1 | Descent | Decelerating | D Phrygian dominant | Square (distorted chug) | Double-bass kick pattern | Stereo unison ±15 cents |
| 2 | Malfunction | Accelerating | D whole-tone (no tonic) bleeding into chromaticism | Pulse with dynamic duty | Blast beat | Wider detune ±30 cents on stereo pair; channel 4 detune oscillation |
| 3 | Apocalypse | Decelerating | D Locrian (flat 2, flat 5) | Triple-stacked (sawtooth + square + pulse) | Hybrid pattern accelerating with the sine | Asymmetric detune ramping across the iteration |

### Iteration 0: Awakening

The composition opens its first loop iteration in the cleanest
configuration. Channel 1 lead on pristine sawtooth, retrigger
off for legato. Channels 6 and 7 stereo unison detune at the
narrow ±15 cents. Scale is D natural minor; the V chord (A) on
beat 4 of each four-bar phrase pivots to harmonic minor through
the raised seventh (C-sharp), preparing the next bar's tonic.

Channel 2 arpeggiator runs minor and major triad arpeggios
matched to the chord per bar. Channel 3 church organ pad
sustains chord tones at low velocity for textural support.
Channel 5 percussion runs the standard backbeat mask from song 3
(kick on tick 0 and 8, snare on tick 4 and 12, hi-hat
interspersed).

This iteration is the "control" against which the others
deviate.

### Iteration 1: Descent

Channel 1 swaps to Square wave with retrigger on; the legato
melody becomes a distorted chug. ADSR sustain drops to 200
q1000 for a harsher decay. The lead's pitches stay near the
chord but draw from D Phrygian dominant (D, E-flat, F-sharp, G,
A, B-flat, C). The flat-2 (E-flat) and raised-3 (F-sharp)
shift the harmony from minor to exotic and dread-laden.

Channel 5 percussion engages the double-bass kick pattern. Kick
hits land on every two ticks (1, 3, 5, 7, 9, 11, 13, 15 within
each bar) producing a galloping low-end rhythm. Snare anchors
beat 2 and beat 4.

The sine wave is in its decelerating phase: tempo travels from
180 BPM down to 60 BPM at the midpoint, then back up. The
heavy kick chug paired with the slowing tempo produces a sense
of mechanical exhaustion descending into stasis.

### Iteration 2: Malfunction

Scale switches to D whole-tone (D, E, F-sharp, G-sharp, A-sharp,
C) with chromatic deviations on weak beats. The whole-tone
scale has no tonic, no leading tone, and consists of all
augmented intervals. The result is dissonant disorientation.

Channel 1 lead uses Pulse wave with continuous duty
modulation. The duty cycles between 100 and 900 q1000 across
each bar, producing a morphing nasal-to-thin timbre. Channels
6 and 7 widen their detune to ±30 cents, audibly out-of-tune
but not yet collapsed. Channel 4 (power-chord roots, the
detuned-doubling partner to channel 0) oscillates its detune
between -50 and +50 cents per tick, producing a tearing,
mechanically-distressed bass timbre.

Channel 5 percussion runs the blast beat from song 3's Chaos
sections. Kick and snare alternate on every tick.

The sine wave is in its accelerating phase, climbing through
180 BPM into 300 BPM. As the rhythm tightens, the dissonance
intensifies. At 300 BPM the per-second event density combined
with the whole-tone harmony produces maximum perceptual
overload.

### Iteration 3: Apocalypse

The collapse. Scale is D Locrian (D, E-flat, F, G, A-flat,
B-flat, C). Locrian's flat-5 (A-flat) destabilises the V chord
into a tritone substitution. There is no resolution.

Channel 1 lead stacks three waveforms simultaneously by playing
the same melodic line on channels 1, 6, and 7 with channel 6 on
Square and channel 7 on Pulse. The three-waveform stack
produces a wall-of-sound character. Detune across the three
channels is asymmetric: channel 1 stays at zero detune, channel
6 at +50 cents (deliberately out of tune sharp), channel 7 at
-50 cents (deliberately out of tune flat). The wall of sound
sounds destabilising at low tempo and frenzied at high tempo.

Channel 5 percussion runs a hybrid pattern that intensifies
with the absolute tick within the iteration. Early bars use
the standard backbeat; mid bars escalate to double-bass; late
bars escalate to blast beat. The percussion pattern is
synchronised to the tempo descent so that the rhythmic
intensity peaks just as the tempo bottoms out at 60 BPM,
producing a slow-motion blast beat.

After iteration 3, `state.loop_count` resets to zero and
iteration 0 plays again.

## Channel assignments

| Channel | Register | Role | Initial waveform | Iteration variations |
|---------|----------|------|------------------|----------------------|
| 0 | Sub-bass, C1..A2 | Driving distorted bass | Sawtooth (code 2) with LPF cutoff at 800 Hz | LPF sweeps to 4000 Hz in iteration 2 (Malfunction). Retrigger on across all iterations. Velocity accents on bass downbeats. |
| 1 | Melody core, C5..A5 | Lead voice; primary expressive layer | Sawtooth (code 2) | Iteration 0: Sawtooth legato. Iteration 1: Square chug with retrigger on. Iteration 2: Pulse with continuous duty modulation. Iteration 3: Sawtooth (with channels 6 and 7 carrying the Square and Pulse stack). |
| 2 | High lead, C6..A6 | Neoclassical arpeggiator and trills | Pulse (code 4) | Continuous duty modulation across all iterations. Filter envelope opens at chord-hit and decays per tick. Step rate is one tick per step normally; triplet polyrhythm during iterations 1 and 2 (the descending and chromatic iterations). |
| 3 | Mid-harmony, C3..A4 | Church organ pad | Triangle (code 1) | Heavy vibrato (rate 4 Hz, depth 40 cents) for the organ character. Sustained chord tones held across bars. ADSR generous (attack 50 ms, decay 200 ms, sustain 700 q1000, release 500 ms). At sine trough (60 BPM) the long-decay envelopes shine; at sine peak (300 BPM) the pads compress into a humming drone. |
| 4 | Low harmony, C3..A3 | Power-chord roots and detuned-doubling partner of channel 0 | Pulse (code 4) with LPF | Detune at -15 cents in iterations 0 and 1 (bass-stack thickening). Detune oscillates between -50 and +50 cents per tick in iteration 2 (Malfunction). In iteration 3 the detune ramps slowly across the iteration, from 0 cents at the start to -100 cents at the end, producing a "tuning collapse" effect. |
| 5 | Percussion | Noise (code 5) drum kit | Noise | ADSR per-tick swap between kick, snare, hi-hat, and tom variants. See per-iteration percussion details below. |
| 6 | Stereo unison or harmonization left twin | Mirrors channel 1's pitch sequence | Mirrors channel 1's waveform per iteration except in iteration 3 where it carries Square independently | Iteration 0: stereo unison at +15 cents, hard-panned left. Iteration 1: stereo unison at +15 cents. Iteration 2: stereo unison widened to +30 cents. Iteration 3: Square waveform stacked with channel 1's Sawtooth, +50 cents detune. |
| 7 | Stereo unison or harmonization right twin | Mirrors channel 1's pitch sequence | Mirrors channel 1's waveform per iteration except in iteration 3 where it carries Pulse independently | Iteration 0: stereo unison at -15 cents, hard-panned right. Iteration 1: stereo unison at -15 cents. Iteration 2: stereo unison widened to -30 cents. Iteration 3: Pulse waveform stacked with channel 1's Sawtooth, -50 cents detune. |

## Chord skeleton

The chord skeleton is constant across all four iterations. The
iteration variation affects scale-degree raise or lower (e.g.,
Phrygian dominant on a V chord in iteration 1, Locrian on a
V chord in iteration 3) but the root sequence does not change.

### Loop A (bars 1-16)

A-section chords. The structure is four phrases of four bars
each, with each phrase reaching a half-cadence on the V chord.

| Bar | Chord (iteration 0) | Root MIDI | Notes |
|-----|---------------------|-----------|-------|
| 1 | Dm | 38 | i in D minor. |
| 2 | Dm | 38 | i held. |
| 3 | B-flat | 34 | VI. |
| 4 | A | 33 | V (harmonic-minor pivot in iteration 0). In iteration 1 this is A in Phrygian dominant context, in iteration 3 it is A-flat (Locrian flat-5). |
| 5 | Dm | 38 | i. |
| 6 | Dm with harmonic-minor pivot | 38 | i with raised seventh on lead. |
| 7 | B-flat | 34 | VI. |
| 8 | A7-flat-9 | 33 | V7-flat-9 (dominant resolution preparation). |
| 9 | Gm | 31 | iv in D minor. |
| 10 | C | 36 | VII. |
| 11 | F | 29 | III (relative major). |
| 12 | B-flat | 34 | VI. |
| 13 | A | 33 | V. |
| 14 | A | 33 | V held. |
| 15 | Dm | 38 | i. |
| 16 | Dm | 38 | i, preparing transition to Loop B. |

### Loop B (bars 17-32)

Gothic-suspension passage. Half-diminished sonorities prevail.
The bass moves more freely than in Loop A.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 17 | E-half-diminished | 28 |
| 18 | A7 | 33 |
| 19 | Dm | 38 |
| 20 | F-sharp-half-diminished | 30 (chromatic descent) |
| 21 | B-half-diminished | 35 |
| 22 | E-half-diminished | 28 |
| 23 | A7 | 33 |
| 24 | Dm | 38 |
| 25 | Gm | 31 |
| 26 | C7 | 36 |
| 27 | F | 29 |
| 28 | B-flat-major-7 | 34 |
| 29 | E-half-diminished | 28 |
| 30 | A7 | 33 |
| 31 | Dm | 38 |
| 32 | A7 | 33 |

### Loop A-prime (bars 33-48)

Loop A transposed up a perfect fourth (effectively into G minor
territory, but always cadencing back to D minor at the end of
the section). The bassline pitches are shifted up five
semitones but the harmonic shape is identical to Loop A.

Bars 33-48 root MIDI sequence:
`[43, 43, 39, 38, 43, 43, 39, 38, 36, 41, 34, 39, 38, 38, 43, 38]`

The transposition is a perfect fourth up from Loop A's root
sequence `[38, 38, 34, 33, 38, 38, 34, 33, 31, 36, 29, 34, 33,
33, 38, 38]`. MIDI 43 is G2 (one fourth above D2). The
transposition places the section in G minor territory. Bar 48
is explicitly set to D minor (MIDI 38) rather than the
transposed G minor (MIDI 43) so the cadence into Loop C
(opening on A major at MIDI 33, the V of D minor) resolves
cleanly through the home tonic.

### Loop C (bars 49-64)

Cadential climax. Cycle of fifths descent returning the tonal
center to D minor. The bassline traces:

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 49 | A | 33 |
| 50 | Dm | 38 |
| 51 | Gm | 31 |
| 52 | C | 36 |
| 53 | F | 29 |
| 54 | B-flat | 34 |
| 55 | E-diminished | 28 |
| 56 | A | 33 |
| 57 | Dm | 38 |
| 58 | Dm | 38 |
| 59 | A | 33 |
| 60 | A | 33 |
| 61 | Dm | 38 |
| 62 | B-flat | 34 |
| 63 | A | 33 |
| 64 | Dm | 38 |

The final Dm at bar 64 is the loop-boundary chord. Loop A's
bar 1 Dm follows immediately on iteration N+1.

## Arpeggio interval-pattern vectors

Channel 2 advances through these eight-step vectors at one
tick per step under normal mode and at the triplet polyrhythm
rate (three steps per two ticks on average) during iterations
1 and 2.

| Chord quality | Vector | Notes |
|---------------|--------|-------|
| Minor triad | `[0, 3, 7, 12, 15, 12, 7, 3]` | Used for D minor, G minor, A minor neighborhoods. |
| Major triad | `[0, 4, 7, 12, 16, 12, 7, 4]` | Used for B-flat, C, F neighborhoods. |
| Dominant 7 / Phrygian dominant | `[0, 4, 7, 10, 13, 10, 7, 4]` | Used for A7, A7-flat-9, V-chord neighborhoods. |
| Half-diminished 7 | `[0, 3, 6, 10, 12, 10, 6, 3]` | New for Loop B's gothic-suspension chords. |
| Whole-tone | `[0, 2, 4, 6, 8, 6, 4, 2]` | Iteration 2 (Malfunction) on every chord, producing dissonant overlay. |
| Locrian (root, flat-2, flat-3, flat-5, octave) | `[0, 1, 3, 6, 12, 6, 3, 1]` | Iteration 3 (Apocalypse) on every chord. |

The arpeggio type per bar is selected from a per-iteration
override table. Iterations 0 and 1 use the chord-matched
vectors above (minor for minor chords, major for major chords,
dominant for V chords, half-diminished for the Loop B
sonorities). Iteration 2 overrides every chord to use the
whole-tone vector. Iteration 3 overrides every chord to use
the Locrian vector.

## Percussion patterns

Five distinct mask states layered on the standard 16-tick bar.

### Iteration 0 (Awakening): Standard backbeat

`[K . H . S . H . K H S . H K S H]`

ADSR profiles identical to song 3 (kick at 2/45/0/10, snare at
1/110/0/30, hi-hat at 1/15/0/5).

### Iteration 1 (Descent): Double-bass gallop

`[K H K S K H K H K S K H K S K H]`

Kick on every odd-numbered tick within the bar (1, 3, 5, 7, 9,
11, 13, 15). Snare anchors beats 2 and 4 (ticks 4 and 12).
Hi-hat on remaining ticks. ADSR kick uses 1/30/0/5 (tighter
than iteration 0) for the rapid-fire feel.

### Iteration 2 (Malfunction): Blast beat

`[K S K S K S K S K S K S K S K S]`

Alternating kick and snare on every tick. The pattern produces
the wall-of-percussion character of the Malfunction iteration.

### Iteration 3 (Apocalypse): Tempo-synchronised escalation

The pattern starts as iteration 0 (standard backbeat) at bar 1
of the iteration, transitions to iteration 1 (double-bass) by
bar 17, reaches iteration 2 (blast) by bar 33, and stays at
blast through bar 64. The transition points are bar-aligned
but the pattern within each phase is the iteration's standard
mask. This produces the impression of percussion intensifying
in lockstep with the apocalyptic narrative.

## Mid-song event schedule

### Init block

Initial configuration at song load. The init block runs on
every fresh load (cold start or hot swap).

```
host::song_name("Keleusma Project: Pendulum Chaconne (0BSD)");
host::set_bpm(90);
host::set_master_volume(700);

// Channel 0: sawtooth bass with LPF
host::set_waveform(0, 2);
host::set_adsr(0, 5, 40, 800, 100);
host::set_volume(0, 500, 500);
host::set_velocity(0, 1000);
host::set_retrigger(0, 1);
host::set_lpf(0, 800);
host::set_detune(0, 0);
host::set_enable(0, 1);

// Channel 1: clean sawtooth lead
host::set_waveform(1, 2);
host::set_adsr(1, 10, 120, 800, 200);
host::set_volume(1, 500, 500);
host::set_velocity(1, 1000);
host::set_retrigger(1, 0);
host::set_vibrato(1, 0, 0);
host::set_lpf(1, 0);
host::set_detune(1, 0);
host::set_enable(1, 1);

// Channel 2: arpeggiator
host::set_waveform(2, 4);
host::set_duty(2, 500);
host::set_adsr(2, 2, 60, 400, 80);
host::set_volume(2, 400, 600);
host::set_velocity(2, 800);
host::set_retrigger(2, 1);
host::set_lpf(2, 3500);
host::set_enable(2, 1);

// Channel 3: church-organ pad
host::set_waveform(3, 1);
host::set_adsr(3, 50, 200, 700, 500);
host::set_volume(3, 600, 400);
host::set_velocity(3, 700);
host::set_retrigger(3, 0);
host::set_vibrato(3, 400, 40);
host::set_enable(3, 1);

// Channel 4: power-chord roots with detune
host::set_waveform(4, 4);
host::set_duty(4, 250);
host::set_adsr(4, 5, 50, 700, 150);
host::set_volume(4, 500, 500);
host::set_velocity(4, 750);
host::set_retrigger(4, 1);
host::set_lpf(4, 1500);
host::set_detune(4, -15);
host::set_enable(4, 1);

// Channel 5: percussion
host::set_waveform(5, 5);
host::set_volume(5, 500, 500);
host::set_velocity(5, 800);
host::set_retrigger(5, 1);
host::set_enable(5, 1);

// Channels 6 and 7: stereo unison pair
host::set_waveform(6, 2);
host::set_adsr(6, 10, 120, 700, 200);
host::set_volume(6, 1000, 0);
host::set_velocity(6, 700);
host::set_retrigger(6, 0);
host::set_detune(6, 15);
host::set_vibrato(6, 600, 20);
host::set_enable(6, 1);

host::set_waveform(7, 2);
host::set_adsr(7, 10, 120, 700, 200);
host::set_volume(7, 0, 1000);
host::set_velocity(7, 700);
host::set_retrigger(7, 0);
host::set_detune(7, -15);
host::set_vibrato(7, 400, 20);
host::set_enable(7, 1);
```

### Per-tick events (all sections)

```
// Tempo modulation: per-tick sine bpm update.
// Intro section (input < 128): linear accelerando 90 to 180 BPM.
if input < 128 {
    host::set_bpm(90 + input * 90 / 128);
} else {
    // Sine pendulum: 60 to 300 BPM, period 2048 ticks.
    let phase = (input - 128) mod 2048;
    let half = phase mod 1024;
    let sign = if phase < 1024 { +1 } else { -1 };
    let sine_q1000 = sign * half * (1024 - half) / 262;
    host::set_bpm(180 + sine_q1000 * 120 / 1000);
}
```

### Per-iteration events (one-shot at iteration onset)

At iteration onset (when `state.loop_count` changes), the
script reconfigures channels per the iteration matrix.

```
// At iteration 1 onset (Descent):
host::set_waveform(1, 0);       // Square chug
host::set_retrigger(1, 1);      // sharp transients
host::set_adsr(1, 1, 40, 200, 60);

// At iteration 2 onset (Malfunction):
host::set_waveform(1, 4);       // Pulse with duty modulation
host::set_duty(1, 500);
host::set_detune(6, 30);
host::set_detune(7, -30);

// At iteration 3 onset (Apocalypse):
host::set_waveform(1, 2);       // back to Sawtooth (stack base)
host::set_waveform(6, 0);       // Square on left twin
host::set_waveform(7, 4);       // Pulse on right twin
host::set_detune(6, 50);
host::set_detune(7, -50);

// At iteration 0 onset (or back to it via reset):
host::set_waveform(1, 2);
host::set_retrigger(1, 0);
host::set_adsr(1, 10, 120, 800, 200);
host::set_waveform(6, 2);
host::set_waveform(7, 2);
host::set_detune(6, 15);
host::set_detune(7, -15);
```

### Master volume dynamics

Master volume tracks the tempo modulation to compensate for
perceptual loudness changes. At slow tempo (60 BPM) the
density of events drops; the script raises master volume to
800 q1000 to keep loudness perceived. At fast tempo (300 BPM)
the density is high; master volume drops to 700 q1000 to
prevent the mix from saturating into the soft-clip.

```
// Inverse-correlation with sine_q1000:
let master = 750 - sine_q1000 * 50 / 1000;
host::set_master_volume(master);
```

Range: `[700, 800]` q1000 as the sine sweeps. Gentle 100-unit
range; the perceptual loudness compensation is subtle.

## Coverage matrix

Per-native exercise across active, inactive, and dynamic
states.

| Native | Active | Inactive | Dynamic |
|--------|--------|----------|---------|
| `host::set_enable` | Channels 0..7 enabled at init | Specific channels disabled during silenced passages | Per-section enable toggles for Loop B half-diminished suspensions (some channels rest) |
| `host::set_waveform` | All channels carry non-default waveforms | Not applicable | Channel 1, 6, 7 morph per iteration |
| `host::set_duty` | Pulse channels at non-default duty | Not applicable | Channel 1 continuous duty modulation in iteration 2; channel 2 PWM continuous always |
| `host::set_adsr` | All channels carry section-specific ADSR | Not applicable | Channel 1 ADSR collapses in iterations 1 through 3; channel 5 swaps per tick |
| `host::set_volume` | Per-channel stereo positions at init | Centered (equal L/R) on channels 0, 4, 5 | Pan automation on channel 2 (arpeggiator swirl) during Loop B |
| `host::set_vibrato` | Channel 3 organ vibrato at 4 Hz / 40 cents | Channel 0, 4, 5 vibrato at zero | Channel 1 vibrato depth ramps during Loop A and disengages during iterations 1 through 3 |
| `host::set_lpf` | Channel 0 LPF at 800 Hz; channel 2 filter envelope at 3500 Hz | Channel 1 LPF at zero (bypass) during clean iterations | Channel 0 LPF sweep across iteration 2; channel 2 per-tick filter envelope decay |
| `host::set_retrigger` | Channel 0, 2, 4, 5 retrigger on | Channel 1 retrigger off during iteration 0 | Channel 1 flips on at iteration 1 onset, stays on through iteration 3 |
| `host::set_detune` | Channels 6, 7 detune at ±15 cents | Channels 0, 1, 2, 3, 5 detune at zero in iteration 0 | Channel 4 detune oscillates per tick during iteration 2; channel 6, 7 detune widens per iteration |
| `host::set_velocity` | Per-channel base velocity at init | 1000 is unity (the neutral state) | Per-tick bass downbeat accents; per-tick percussion velocity swap; iteration-3 escalation |
| `host::set_master_volume` | 750 q1000 at the sine zero-crossings | 1000 is unity default | Per-tick inverse-sine modulation across the full composition |
| `host::set_bpm` | 180 BPM at sine zero-crossings | Not applicable (tempo always exists) | Per-tick sine modulation; intro accelerando |
| `host::song_name` | Called once in init | Not applicable | Not applicable (one-shot convention) |
| `host::play` / `host::silence` | Per-tick note onsets | Channel silence during rests | Per-tick on every active channel |

All natives are exercised. The Noise waveform (code 5) is in
use via channel 5 percussion. All six waveform codes are used
across channels 0 through 7 collectively.

## Audio mix budgeting

Same constraint as song 3: at any given sample, the sum across
all active voices of `volume_left * env.level * velocity` must
stay below approximately 0.85 to avoid pushing the master
`tanh` soft-clip into mutual saturation. The same holds for
`volume_right`.

Song 4's eight-channel configuration is denser than song 3's.
Mitigation: each channel's velocity is set conservatively (650
to 800 q1000 in most cases), with the master volume tracking
the sine to compensate for perceived loudness. The iteration 3
three-waveform stack on channels 1, 6, and 7 is the densest
moment; the wide detune offsets cause the three voices to phase
against each other, which reduces peak amplitude through
destructive interference.

## Verification checklist

The song is complete when:

- Compiles via `cargo run -p keleusma-cli -- compile examples/scripts/piano_roll/piano_roll_4.kel`.
- Loads through `Vm::new` against the default arena without a `VerifyError`.
- A headless probe of the first one to two minutes of playback shows the expected per-tick `host::play` sequence and the expected init-block native calls.
- The sine tempo modulation is audibly continuous (no stepping or stuttering at the inflection points). The BPM trace over time approximates a sine wave with peaks at the expected tick offsets.
- The intro plays only on the first iteration after song load. Hot swap to song 4 from another song plays the intro.
- Loop iteration mutation triggers at the loop boundary. `state.loop_count` cycles 0, 1, 2, 3, 0, 1, 2, 3 and the channel reconfigurations fire at each onset.
- Every host native is called somewhere in the composition. Every dynamic-capable native is called multiple times.
- Workspace tests, clippy, fmt, release build all clean.

## Sheet music feasibility

### Overview

Sheet music for this composition is feasible. The continuous
sine-tempo modulation and the four-iteration variation cycle
make standard classical notation strain at the seams. The
notation approach is identical in principle to the song 3
approach: a hybrid of traditional staff notation and a
performance manual or graphic-notation overlay.

### Continuous sine-wave tempo

This is the most challenging element to notate. Standard
notation expresses tempo changes through accelerando and
ritardando markings or through metric modulation.

Notation solution. The score uses a continuously-curved tempo
graph above the system. The Y-axis shows beats-per-minute from
60 to 300; the X-axis shows bars. The curve is the sine pendulum.
Conductors with a click-track or backing-track reference can
follow the curve directly. Conductors without a reference
would need to internalise the sine shape and approximate the
acceleration and deceleration manually, which is impractical
for a human ensemble at this rate of change.

Practical recommendation. Performance of song 4 by a live
ensemble requires a click track. The click track derives from
the same sine formula the script uses.

### Iteration mutation

Each iteration's variation reads as a multi-bar repeat with
performance-direction overlays. The score would contain four
sequential sections labelled "Iteration 0: Awakening",
"Iteration 1: Descent", "Iteration 2: Malfunction", and
"Iteration 3: Apocalypse", each spanning the 64-bar loop body
with the same chord skeleton but distinct performance
directions for the lead and percussion.

A simpler arrangement uses the chaconne-variation convention:
print the chord skeleton and bassline once at the head of the
score, then print four variation pages, each showing only the
lead and percussion content for that iteration.

### Three-waveform stack in iteration 3

The Apocalypse stack of Sawtooth + Square + Pulse on channels
1, 6, and 7 with asymmetric detune translates into three
parallel staves with a performance note: "Voice II detuned plus
50 cents, Voice III detuned minus 50 cents, intentionally out
of tune". The three staves play unison pitches except where
the parallel-interval harmonization mode is engaged.

### Whole-tone and Locrian passages

These exotic scales are notated through standard accidentals.
Iteration 2's whole-tone passage uses sharps and flats freely;
iteration 3's Locrian passage flats the second, third, fifth,
sixth, and seventh degrees of D.

### Master-score layout

Same eight-staff layout as song 3, with the addition of a
continuous tempo graph above the systems.

| Staff | Channel | Role |
|-------|---------|------|
| (tempo graph) | --- | Continuous sine BPM curve (60..300) |
| 1 | Channel 1 | Lead voice (Sawtooth into Square into Pulse depending on iteration) |
| 2 | Channel 6 | Left twin (unison detune or Square stack) |
| 3 | Channel 7 | Right twin (unison detune or Pulse stack) |
| 4 | Channel 2 | Neoclassical arpeggiator |
| 5 | Channel 3 | Church organ pad |
| 6 | Channel 0 | Driving distorted bass |
| 7 | Channel 4 | Power-chord roots / detuned-doubling partner |
| 8 | Channel 5 | Percussion engine |

### Verdict

Sheet music is feasible but the score is intended as a
reference document, not a primary performance vehicle. The
piece is most authentically realised by the script itself; the
score documents the underlying structure for analytical and
educational purposes.

## Pending implementation

The script `examples/scripts/piano_roll/piano_roll_4.kel` is not yet implemented.
This specification provides the structural and musical content
required for a script-author pass. The script will:

- Add `include_str!("piano_roll_4.kel")` to `SONG_SOURCES` at index 4 in `examples/piano_roll.rs`.
- Implement the sine-approximation tempo math, the four-iteration mutation state machine, the per-iteration channel reconfigurations, the chord-matrix lookups, and the percussion-mask swaps.
- Update `book/src/PIANO_ROLL.md` to mention song 4.
- Update the module docstring in `examples/piano_roll.rs` to reflect the four-song-plus roster.
- Verify via headless probe, lib tests, clippy, fmt, and release build per the established discipline.

The implementation effort is estimated at approximately the
same scale as the song 3 implementation, roughly 800 to 1000
lines of Keleusma source, with the sine math and the iteration
state machine being the primary novel elements.
