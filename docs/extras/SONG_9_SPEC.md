# Song 9 specification: Sixteen Sunrises

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document is the implementation specification for
`examples/scripts/piano_roll/piano_roll_9.kel` (pending implementation), the
roster's culminating composition. Song 9 fuses the
mainstream-pop construction of song 8 with the structural
experimentation of songs 3 through 7, producing a long-form
loop composition with sixteen iteration variations. Each
iteration is a self-contained mini-song with verse, chorus,
and bridge structure, presented in a different scale and
timbre. Tempo travels from 60 beats-per-minute to 300
beats-per-minute through both segmented ramps and continuous
sine modulation, with one "confusion zone" per iteration that
destabilises the texture before resolving back into clarity.

See the long-form manual at
[`book/src/PIANO_ROLL.md`](../../book/src/PIANO_ROLL.md) for the
broader piano-roll context. Song 9 builds on techniques from
all prior specs in the extras directory.

## Role in the roster

Index 9 in `SONG_SOURCES`. Joins the existing roster behind the
`sdl3-example` and `text` cargo features. Accessible at runtime
through the `s` (cycle), `r` (restart), and `9` (direct select)
input commands. The input watcher requires no host change.

## High-level brief

A semi-experimental loop composition with a textbook chiptune
core. The piece begins with a twenty-second intro that ramps
from 60 BPM to 180 BPM, then enters the loop body. The loop
body is approximately three minutes long and consists of twelve
sections organised in the manner of a conventional pop song
(intro mini, verse, pre-chorus, chorus, confusion zone, verse,
pre-chorus, chorus, bridge, modulation, final chorus, outro).
The loop iterates sixteen times before the meta-loop resets
and the cycle restarts.

Each iteration uses the same chord skeleton and the same
melodic outline but realises them in a different scale and a
different lead-voice timbre. The sixteen iterations form a
four-by-four matrix of scale (C major, A minor, D Dorian, D
Phrygian dominant) and lead waveform (Sawtooth, Square, Pulse,
Triangle). The listener hears the underlying composition
sixteen times in sixteen different sonic guises.

Tempo varies dynamically. The mainstream sections (verses,
pre-choruses, choruses) use segmented ramps in the song 3
style, with per-tick BPM updates between section boundaries.
The "confusion zone" section in each iteration uses continuous
sine modulation in the song 4 style, producing a deliberate
texture destabilisation before the next verse restores
clarity.

Approximate listening length: approximately three minutes per
iteration, sixteen iterations totaling approximately fifty
minutes for the full meta-loop. The composition is suitable
for extended ambient listening; the listener experiences the
same musical content repeatedly through shifting harmonic and
timbral lenses.

## Design intent and the experimental-pop balance

### Why this composition synthesises the prior songs

Songs 3 through 7 each push a single dimension of the
implementation engine to a deliberate extreme. Songs 3 and 4
exercise maximalist feature stacking. Song 5 explores precision
through phase drift. Song 6 explores polyphony through canonic
counterpoint. Song 7 explores microtonal pitch. Each is
deliberately experimental and rewards close listening rather
than casual reception.

Song 8 sits at the other end of the spectrum. It demonstrates
that the engine handles mainstream commercial pop with the
same facility as it handles experimentation. The piece is
deliberately conventional.

Song 9 deliberately occupies the space between these poles. It
takes the conventional pop-song construction of song 8 and
applies experimental dimensions across sixteen iterations.
Each iteration is recognisably pop-shaped, with verse, chorus,
and bridge that any listener can follow. The experimentation
emerges through the iteration-level mutations: scale changes
that defamiliarise the melody, lead-voice timbre shifts that
recolour the texture, and the per-iteration confusion zone
that destabilises before restoring coherence.

The synthesis is structural. Each iteration's experimental
elements are bounded and the listener always returns to
familiar pop territory after each excursion. The composition
is therefore experimental in its iteration design while
remaining popular in its moment-to-moment listening
experience.

### The coherence delta principle

A listener's enjoyment of "clear and predictable" musical
passages is proportional to the contrast against neighbouring
"confused or unpredictable" passages. A song that is entirely
predictable becomes monotonous; a song that is entirely
unpredictable becomes incoherent. The composition that
oscillates between predictable and unpredictable provides
both moment-to-moment surprise and structural orientation.

Song 9 implements this principle through the confusion zone.
Each iteration contains an eight-bar passage where the tempo
modulates continuously (song 4 style) and the scale temporarily
shifts to dissonant material (whole-tone, locrian, or extended
chromatic). The listener experiences disorientation for a brief
period. The verse that follows is therefore more rewarding
than it would be on its own; the listener arrives at clarity
as relief from preceding confusion.

The confusion zones are placed strategically. They occur
after the first chorus statement in each iteration, between
the chorus and the second verse. The placement allows the
listener to first hear the chorus in its clear form, then
encounter destabilisation, then receive the second verse as
restoration. By the time the second chorus arrives, the
listener is primed to experience it as reward.

### Why sixteen iterations

Sixteen is enough to traverse a structured space of variations
without becoming endless. Each iteration is approximately
three minutes; the full sixteen-iteration cycle is roughly
fifty minutes, suitable for ambient extended listening but
not so long that the listener loses orientation.

The sixteen iterations are organised as a four-by-four matrix
of scale (four scales) and lead waveform (four waveforms).
The matrix is traversed row-major: all four waveforms in the
first scale, then all four in the second scale, and so on.
This produces a listening arc that travels from most familiar
(C major Sawtooth) to most exotic (D Phrygian dominant
Triangle) over the meta-loop.

### Why a chiptune core

The chiptune aesthetic uses simple waveforms (square, pulse,
triangle, sawtooth) at modest polyphony with strict tempo and
deterministic timing. These properties match what the
implementation engine provides natively. The chiptune
tradition also has a strong J-pop connection through the
shared aesthetic vocabulary of melodic catchiness, modal
colour, and clean stereo imaging. A song with a chiptune
core can adopt the J-pop techniques of half-step modulation,
royal road progressions, and stereo-doubled backing vocals
while remaining sonically distinct from a "pop band" mix.

### Contrast with songs 3 through 8

Songs 3 through 7 explore experimentation at the cost of
broad accessibility. Song 8 explores accessibility at the cost
of experimentation. Song 9 attempts both. The implementation
engine's full feature matrix is exercised, the J-pop
conventions are honoured, the chiptune aesthetic is
maintained, the structural variation is rich, and the
listening experience is intended to be pleasant for a broad
audience.

## Design constraints

- Intro followed by infinite loop. Intro is approximately twenty seconds (192 ticks at average 90 BPM ramp). Loop body is approximately three minutes per iteration (2304 ticks at average 180 BPM).
- Sixteen iterations cycling on `state.loop_count`. After iteration 15, count resets to 0 and the cycle restarts.
- Each iteration is a complete song-form with twelve sections: mini-intro, verse, pre-chorus, chorus, confusion zone, verse 2, pre-chorus 2, chorus 2, bridge, modulation, final chorus, mini-outro.
- Tempo ranges 60 BPM (slow points) to 300 BPM (peaks) with 180 BPM as the standard cruise. Tempo modulates through segmented ramps in song 3 style at most section boundaries, and through continuous sine modulation in the confusion zone at song 4 style.
- Eight channels in chiptune-pop arrangement: bass, lead vocal stand-in, lead arpeggio, harmony pad, rhythm pad, drums (noise), backing vocal harmony 1, backing vocal harmony 2.
- Fully stereo. Lead centred, backing harmonies hard-panned, arpeggios slightly panned, drums and bass centred, pads spread.
- Phrase variety: the lead melody varies between sections within an iteration. Each iteration's confusion zone uses dissonant material to create the coherence delta.
- Full host matrix coverage: every host native is used in active, inactive, and dynamic states across the sixteen iterations.
- J-pop and chiptune techniques: half-step modulation at final chorus, royal road progressions, stereo-doubled backing vocals, modal pivots between iterations.
- Pop-grounded experimentation: experimental elements (confusion zones, scale shifts, timbre mutations) are bounded and resolve back to coherent pop material. The listener always returns to familiar territory.

## Total length and section structure

### Overall composition

The composition consists of an unlooped intro followed by a
sixteen-iteration loop body. First-iteration tick mapping
below.

| Stage | Ticks | Duration at 180 BPM cruise | Description |
|-------|-------|----------------------------|-------------|
| Intro | 0..192 | approximately 20 seconds | One-shot opener. Tempo ramps 60 BPM to 180 BPM. Establishes the chord skeleton and the lead melody outline. Plays once on song load. |
| Loop iteration 0 | 192..2496 | approximately 3 minutes | First iteration, C major Sawtooth. |
| Loop iteration 1 | 2496..4800 | approximately 3 minutes | C major Square. |
| Loop iteration 2 | 4800..7104 | approximately 3 minutes | C major Pulse. |
| Loop iteration 3 | 7104..9408 | approximately 3 minutes | C major Triangle. |
| Loop iteration 4 | 9408..11712 | approximately 3 minutes | A minor Sawtooth. |
| Loop iteration 5 | 11712..14016 | approximately 3 minutes | A minor Square. |
| Loop iteration 6 | 14016..16320 | approximately 3 minutes | A minor Pulse. |
| Loop iteration 7 | 16320..18624 | approximately 3 minutes | A minor Triangle. |
| Loop iteration 8 | 18624..20928 | approximately 3 minutes | D Dorian Sawtooth. |
| Loop iteration 9 | 20928..23232 | approximately 3 minutes | D Dorian Square. |
| Loop iteration 10 | 23232..25536 | approximately 3 minutes | D Dorian Pulse. |
| Loop iteration 11 | 25536..27840 | approximately 3 minutes | D Dorian Triangle. |
| Loop iteration 12 | 27840..30144 | approximately 3 minutes | D Phrygian dominant Sawtooth. |
| Loop iteration 13 | 30144..32448 | approximately 3 minutes | D Phrygian dominant Square. |
| Loop iteration 14 | 32448..34752 | approximately 3 minutes | D Phrygian dominant Pulse. |
| Loop iteration 15 | 34752..37056 | approximately 3 minutes | D Phrygian dominant Triangle. |

After iteration 15, the meta-loop resets. The intro plays only
on first song load; subsequent meta-loops (after the sixteenth
iteration) enter directly at iteration 0 with no intro.

Total first-iteration length: 37056 ticks, approximately
fifty-one minutes at average 180 BPM. The actual wall-clock
length varies because the per-iteration tempo profile spends
time at both 60 BPM (slower than average) and 300 BPM (faster
than average); average over each iteration is roughly 180 BPM.

### Per-iteration section structure

Each iteration occupies 2304 ticks divided into twelve
sections. The structure is analogous to song 8's twelve-section
pop layout but with adjusted proportions to accommodate the
confusion zone.

| Section | Bars | Ticks | Within-iteration range | Tempo profile | Description |
|---------|------|-------|------------------------|---------------|-------------|
| 0 Mini-intro | 4 | 64 | 0..64 | 60 BPM ramp to 180 BPM | Iteration onset. Reset master volume, reconfigure waveforms for the new iteration's scale and timbre. Bass and pad enter. |
| 1 Verse | 16 | 256 | 64..320 | 180 BPM (stable) | Lead melody states the verse theme. Backing vocals silent. |
| 2 Pre-chorus | 8 | 128 | 320..448 | 180 BPM ramp to 240 BPM | Royal road progression. Lead melody ascends. Build into chorus. |
| 3 Chorus | 24 | 384 | 448..832 | 240 BPM | Hook melody. Backing vocals at thirds and sixths above lead, hard-panned. Full arrangement. |
| 4 Confusion zone | 8 | 128 | 832..960 | Sine modulation 60 BPM to 300 BPM and back | Scale temporarily shifts to dissonant material. Detune oscillation on rhythm channels. Texture destabilises. |
| 5 Verse 2 | 16 | 256 | 960..1216 | 180 BPM (stable) | Verse theme returns with melodic variation. Resolution after confusion. |
| 6 Pre-chorus 2 | 8 | 128 | 1216..1344 | 180 BPM ramp to 250 BPM | Royal road again, slight build. |
| 7 Chorus 2 | 16 | 256 | 1344..1600 | 250 BPM | Hook melody. Slightly shorter than first chorus. |
| 8 Bridge | 12 | 192 | 1600..1792 | 250 BPM ramp to 90 BPM | New melodic material in the iteration's relative-key. Tempo cools down. |
| 9 Modulation | 4 | 64 | 1792..1856 | 90 BPM (held) | Pivot passage. Dominant pedal sets up the final chorus's half-step-up modulation. |
| 10 Final chorus | 24 | 384 | 1856..2240 | 90 BPM ramp to 300 BPM | Final statement of the hook melody, transposed up one semitone. Peak intensity. |
| 11 Mini-outro | 4 | 64 | 2240..2304 | 300 BPM ramp to 60 BPM | Cool-down. Voices drop out one by one. Tempo decelerates to set up the next iteration's mini-intro. |

The mini-intro of one iteration overlaps the mini-outro of the
previous iteration in tempo terms: the previous iteration ends
at 60 BPM, the next iteration starts at 60 BPM. The transition
is musically continuous in tempo even as the harmonic content
shifts to the new iteration's scale and timbre.

### Tempo profile across one iteration

Per-tick BPM updates throughout. The script computes the
current BPM from the current section and the position within
the section. Each section has its own formula.

- Section 0 (Mini-intro): linear ramp 60 to 180 BPM over 64 ticks.
- Section 1 (Verse): hold at 180 BPM.
- Section 2 (Pre-chorus): linear ramp 180 to 240 BPM over 128 ticks.
- Section 3 (Chorus): hold at 240 BPM.
- Section 4 (Confusion zone): sine modulation. Center 180 BPM, amplitude 120 BPM, period 128 ticks (one full cycle across the section). The tempo oscillates 60 BPM to 300 BPM and back.
- Section 5 (Verse 2): hold at 180 BPM.
- Section 6 (Pre-chorus 2): linear ramp 180 to 250 BPM over 128 ticks.
- Section 7 (Chorus 2): hold at 250 BPM.
- Section 8 (Bridge): linear ramp 250 to 90 BPM over 192 ticks (gradual cool-down).
- Section 9 (Modulation): hold at 90 BPM.
- Section 10 (Final chorus): linear ramp 90 to 300 BPM over 384 ticks (epic build).
- Section 11 (Mini-outro): linear ramp 300 to 60 BPM over 64 ticks (rapid cool-down).

The final mini-outro tempo of 60 BPM matches the next
iteration's mini-intro start tempo, producing the continuous
tempo across iteration boundaries.

## The sixteen iteration variations

The sixteen iterations cycle through a four-by-four matrix of
scale and lead waveform. The chord skeleton and the melodic
outline are constant across all sixteen iterations; the
realisation varies.

### Scale per iteration

The scales progress from most familiar to most exotic.

| Iterations | Scale | Tonic | Scale degrees from tonic (semitones) |
|------------|-------|-------|---------------------------------------|
| 0..3 | C major | C (MIDI 36 for bass tonic) | 0, 2, 4, 5, 7, 9, 11 |
| 4..7 | A minor | A (MIDI 33 for bass tonic) | 0, 2, 3, 5, 7, 8, 10 |
| 8..11 | D Dorian | D (MIDI 38 for bass tonic) | 0, 2, 3, 5, 7, 9, 10 |
| 12..15 | D Phrygian dominant | D (MIDI 38 for bass tonic) | 0, 1, 4, 5, 7, 8, 10 |

The progression from C major to A minor is the standard
relative-minor modulation. From A minor to D Dorian is a modal
shift to the iv-rooted Dorian mode (D is the 4th of A). From D
Dorian to D Phrygian dominant is a chromatic exoticisation
(retaining D as tonic but introducing flat-2 and raised-3 for
the Phrygian-dominant colour).

### Lead waveform per iteration

The lead waveform cycles within each scale block.

| Iteration within scale block | Lead waveform code | Character |
|------------------------------|--------------------|-----------|
| First (iter 0, 4, 8, 12) | 2 (Sawtooth) | Warm, vocal-like, the standard pop-lead character |
| Second (iter 1, 5, 9, 13) | 0 (Square) | Bright, chiptune-classic, slightly nasal |
| Third (iter 2, 6, 10, 14) | 4 (Pulse) with duty 250 | Thin, narrow, with characteristic pulse-wave harmonic content |
| Fourth (iter 3, 7, 11, 15) | 1 (Triangle) | Mellow, soft, the "flute" or "synth-pad" character |

Each waveform brings a different sonic flavour to the same
underlying melody. The Sawtooth iterations sound the most
"pop-band". The Square iterations sound the most "chiptune-
classic". The Pulse iterations sound thin and crisp. The
Triangle iterations sound smooth and ambient.

### Combined iteration table

| Iter | Scale | Lead waveform | Approximate aesthetic |
|------|-------|---------------|------------------------|
| 0 | C major | Sawtooth | Bright pop, mainstream |
| 1 | C major | Square | Bright chiptune-classic |
| 2 | C major | Pulse | Bright crisp pop |
| 3 | C major | Triangle | Bright ambient pop |
| 4 | A minor | Sawtooth | Introspective pop |
| 5 | A minor | Square | Introspective chiptune |
| 6 | A minor | Pulse | Introspective crisp |
| 7 | A minor | Triangle | Introspective ambient |
| 8 | D Dorian | Sawtooth | Modal folk-pop |
| 9 | D Dorian | Square | Modal chiptune-folk |
| 10 | D Dorian | Pulse | Modal crisp |
| 11 | D Dorian | Triangle | Modal ambient |
| 12 | D Phrygian dominant | Sawtooth | Exotic pop |
| 13 | D Phrygian dominant | Square | Exotic chiptune |
| 14 | D Phrygian dominant | Pulse | Exotic crisp |
| 15 | D Phrygian dominant | Triangle | Exotic ambient |

The listener traverses sixteen distinct aesthetic moments
across the meta-loop while always hearing the same underlying
song-form.

### Per-iteration tonic and modulation behaviour

Each iteration's tonic determines the chord-root mapping for
that iteration. The chord skeleton is constant across
iterations in terms of scale-degree movement: I-V-vi-IV for
the chorus, IV-V-iii-vi for the pre-chorus royal road, etc.
The actual root MIDI values for each chord depend on the
iteration's tonic and scale.

For example, the chorus's I-V-vi-IV progression realises as:
- In C major (iter 0..3): C-G-Am-F (roots 36, 43, 45, 41)
- In A minor (iter 4..7, treated as i-VII-VI-iv): Am-G-F-Dm (roots 45, 43, 41, 38)
- In D Dorian (iter 8..11, treated as i-VII-vi-iv): Dm-C-Bm♭5-Gm (roots 38, 36, 47, 43)
- In D Phrygian dominant (iter 12..15, treated as I-flat-V-vi-IV): D-A♭-Bm-G (roots 38, 32, 47, 43)

The bass and pad layers render these chord roots. The melody
adjusts to fit the iteration's scale through scale-degree
based pitch derivation.

### Half-step modulation per iteration

Every iteration's final chorus modulates up one semitone from
that iteration's tonic. So iteration 0 modulates from C major
to D-flat major. Iteration 4 modulates from A minor to
B-flat minor. Iteration 8 modulates from D Dorian to E-flat
Dorian. And so on.

The half-step-up modulation is the canonical J-pop final-
chorus device. Applied across all sixteen iterations, the
listener experiences it as a structural marker that signals
"climax approaching" within each iteration, regardless of the
iteration's scale or timbre.

## The confusion zone (per iteration)

The confusion zone occupies 128 ticks (eight bars) immediately
after the first chorus statement. It is the coherence-delta
mechanism described in the design intent section. The zone
exhibits four properties simultaneously.

### Tempo

Continuous sine modulation. Center 180 BPM, amplitude 120 BPM.
Period 128 ticks (one full cycle across the section). The
tempo accelerates from 180 to 300 BPM at the section's quarter
point, returns to 180 BPM at the midpoint, decelerates to 60
BPM at the three-quarter point, and returns to 180 BPM at the
section end.

This produces an immediate audible difference from the
preceding chorus (which was stable at 240 BPM) and from the
following verse (which will be stable at 180 BPM). The
listener cannot establish a tempo expectation during the
confusion zone.

### Scale

The scale temporarily shifts away from the iteration's home
scale. The shift is to D whole-tone (D, E, F♯, G♯, A♯, C) for
iterations whose home scale is major or minor (iter 0..7), or
to D Locrian (D, E♭, F, G, A♭, B♭, C) for iterations whose
home scale is modal or exotic (iter 8..15).

The temporary scale produces dissonance against the bass and
pad (which continue to outline the home-scale chord
progression). The simultaneity is harmonically unstable.

### Detune

Channel 4 (rhythm pad / power-chord) detune oscillates between
-50 and +50 cents per tick. This is the song 4 Malfunction
iteration's mechanical-distress effect. The bass stack
audibly tears against itself.

### Texture

Channels 6 and 7 (backing vocals) remain enabled but their
pitch tracking switches from parallel diatonic intervals to
chromatic intervals (parallel minor thirds and minor sixths,
not scale-aware). The harmonisation becomes harsh.

### Coherence restoration

At the confusion zone's end (tick 960 within iteration), all
four destabilising properties cease simultaneously:
- Tempo snaps to 180 BPM.
- Scale returns to the iteration's home scale.
- Channel 4 detune resets to 0 cents.
- Backing vocals return to scale-aware parallel intervals.

The verse 2 that follows is therefore experienced as a
restoration of clarity. The listener's relief is the reward
that justifies the confusion.

## Channel assignments

| Channel | Role | Initial waveform | Stereo position | Per-iteration variation |
|---------|------|------------------|-----------------|--------------------------|
| 0 | Bass | Sawtooth (code 2) with LPF | Centre (500/500) | Static across iterations. Plays chord root on tick 0 and 8 of each bar. |
| 1 | Lead vocal stand-in | Varies per iteration (Sawtooth, Square, Pulse, Triangle) | Centre (500/500) | Iteration-driven waveform changes at every iteration boundary. |
| 2 | Lead arpeggio | Pulse (code 4) with duty 500 | Slight left (650/350) | Static. Continuous PWM modulation. |
| 3 | Harmony pad | Triangle (code 1) with vibrato | Slight right (350/650) | Static. |
| 4 | Rhythm pad / power chord | Pulse (code 4) with duty 250, LPF 2000 | Slight left (600/400) | Static normally; detune oscillation activates during confusion zone. |
| 5 | Drums | Noise (code 5) with dynamic ADSR | Centre (500/500) | Pattern varies per section: standard backbeat for verses, denser pattern for choruses, blast beat for confusion zone. |
| 6 | Backing harmony 1 (third above lead) | Sawtooth (code 2) with +3 cents detune | Hard left (1000/0) | Active during chorus sections only. Disabled elsewhere. |
| 7 | Backing harmony 2 (sixth above lead) | Sawtooth (code 2) with -3 cents detune | Hard right (0/1000) | Active during chorus sections only. Disabled elsewhere. |

The channel allocation matches song 8's pop-band layout. The
key difference is that channel 1's waveform changes
per-iteration, and the confusion zone temporarily modifies
channels 4, 6, and 7.

## Mid-iteration chord skeleton

The chord skeleton is the same scale-degree progression
across all sixteen iterations. The realisation depends on the
iteration's tonic and scale.

### Verse and Verse 2 (16 bars each)

Two cycles of I-V-vi-IV in C major form. Scale-degree movement:

| Bar | Scale degree progression |
|-----|--------------------------|
| 1, 5, 9, 13 | 1 (tonic) |
| 2, 6, 10, 14 | 5 |
| 3, 7, 11, 15 | 6 (relative minor for major-key iterations, supertonic for minor-key iterations) |
| 4, 8, 12, 16 | 4 |

For iter 0..3 (C major): C-G-Am-F-C-G-Am-F-... (16 bars).
For iter 4..7 (A minor): Am-G-F-Dm-Am-G-F-Dm-... (read as i-VII-VI-iv).
For iter 8..11 (D Dorian): Dm-C-Bm-Gm-Dm-C-Bm-Gm-... (read as i-VII-vi-iv).
For iter 12..15 (D Phrygian dominant): D-A♭-Bdim-G-D-A♭-Bdim-G-... (read as I-V-vi-IV with chromatic alterations).

### Pre-chorus (8 bars each)

Royal road: 4-5-3-6-4-5-1-5.

For C major: F-G-Em-Am-F-G-C-G.
For A minor: Dm-Em-Cmaj-Fmaj-Dm-Em-Am-Em (interpreted in minor-key reading).
For D Dorian: Gm-Am-F-Bm-Gm-Am-Dm-Am.
For D Phrygian dominant: G-A♭-F♯-Bdim-G-A♭-D-A♭.

### Chorus (24 bars first, 16 bars second)

Three cycles of I-V-vi-IV plus turnaround for the 24-bar
chorus. Two cycles plus turnaround for the 16-bar chorus.

### Confusion zone (8 bars)

Chord movement halts. The bass holds the iteration's tonic
throughout the confusion zone while the lead and pad explore
the temporary alien scale. The structural anchor (bass tonic)
remains stable while the surface (lead, pad, harmonies) becomes
unstable. This is the chaconne principle from song 4 applied
locally: an invariant foundation supporting destabilising
surface motion.

### Bridge (12 bars)

In the iteration's relative key. For major-key iterations, the
relative minor (A minor for iter 0..3, etc.). For minor-key
iterations, the relative major (C major for iter 4..7). For
modal iterations, a parallel-minor or parallel-major shift.

The bridge progression: i-VI-iv-V (or relative-key equivalent)
repeated twice. Provides emotional contrast against the verse
and chorus material.

### Modulation (4 bars)

Sustained dominant pedal of the half-step-up target key. For
iter 0 (C major → D-flat major modulation), the modulation
section sustains A-flat (V of D-flat) for four bars.

### Final chorus (24 bars)

The chorus material transposed up one semitone, presenting
in the half-step-up key. The cadential extension at bars 21-24
provides the climactic peak.

## Mid-song event schedule (overview)

The full event schedule is large. The implementation pass
will derive most events from the chord skeleton, the iteration
parameters, and the section structure. The overview below
describes the categories.

### Init block (runs once on song load)

Configure all eight channels with default waveforms, ADSR
envelopes, stereo positions, and vibrato settings. Set tempo
to 60 BPM (intro start). Set master volume to 800. Announce
the song name. Enable channels 0 through 5; leave channels 6
and 7 disabled (they enable on chorus onsets).

### Intro (192 ticks)

Per-tick BPM ramp from 60 to 180 over 192 ticks. Bass plays
chord roots; pad sustains chord tones; arpeggio enters at the
midpoint (tick 96) and plays the chord-tone arpeggio; lead
remains silent. Drums enter at tick 128 with a building
pattern. The intro establishes tempo and key for the first
iteration.

### Per-iteration onset (every 2304 ticks of loop body)

Reconfigure channel 1 waveform based on iteration index modulo
4. Reset master volume to 800. Reset all detune offsets to
zero. Update the iteration-local tonic and scale tables.

### Section transitions within each iteration

Twelve section-onset points trigger appropriate
reconfigurations. The confusion zone onset activates the
destabilising properties; the confusion zone exit restores
them. Backing vocals enable at chorus onsets and disable at
chorus exits.

### Per-tick events

The standard per-tick body fires bass, lead, arpeggio, pad,
drums, and backing-vocal events based on the current section
and the iteration parameters. The tempo updates per tick
through the section-specific formula.

Per the lesson from backlog item B12, the per-tick body
inlines arithmetic and uses small match-based helper
functions rather than nested helper chains.

## Full host matrix coverage

Song 9 exercises every host native in active, inactive, and
dynamic states across the sixteen iterations and the
within-iteration section structure.

| Native | Active | Inactive | Dynamic |
|--------|--------|----------|---------|
| `host::set_enable` | Channels 0..5 active at init | Channels 6, 7 disabled outside choruses | Per-section toggles for backing vocals |
| `host::set_waveform` | All channels carry non-default waveforms | Not applicable | Channel 1 changes waveform at every iteration boundary (cycling Sawtooth, Square, Pulse, Triangle) |
| `host::set_duty` | Pulse channels (2, 4) at non-default duty | Not applicable | Continuous PWM modulation on channel 2 throughout |
| `host::set_adsr` | Per-channel envelopes at init | Not applicable | Channel 5 percussion swaps per beat between kick, snare, hi-hat profiles |
| `host::set_volume` | Per-channel stereo positions at init | Centered on bass, drums, lead | Section-onset pan adjustments on rhythm pad during the confusion zone |
| `host::set_vibrato` | Lead vocal vibrato active during verses and choruses | Lead vibrato off during confusion zone | Vibrato depth ramps in pre-chorus build sections |
| `host::set_lpf` | Bass LPF, rhythm pad LPF active | Lead LPF zero (bypassed) for most of composition | Confusion zone filter sweep on rhythm pad |
| `host::set_retrigger` | Bass, arpeggio, drums retrigger on | Lead retrigger off for legato | Lead retrigger flips on during Square iterations (iter 1, 5, 9, 13) for chiptune-classic staccato |
| `host::set_detune` | Backing harmonies at ±3 cents | Most channels at zero detune | Channel 4 per-tick oscillation -50 to +50 cents during confusion zone |
| `host::set_velocity` | Per-channel base velocities at init | 1000 is unity (neutral) | Per-tick bass downbeat accents; percussion velocity per beat; chorus climax boost |
| `host::set_master_volume` | 800 at init, 950 at final chorus | 1000 unity | Linear ramp 800 to 950 across the final chorus; reset to 800 at iteration boundary |
| `host::set_bpm` | 180 BPM cruise, 240 chorus, 250 chorus 2 | Not applicable | Per-tick ramps in mini-intro, pre-choruses, bridge, modulation, final chorus, mini-outro. Per-tick sine modulation in confusion zone. |
| `host::song_name` | Called once in init | Not applicable | Not applicable (one-shot) |
| `host::play` / `host::silence` | Per-tick note events on all active channels | Channel silence during rests | Per-tick on each active channel |

All natives are exercised. All six waveforms (Square,
Triangle, Sawtooth, Sine, Pulse, Noise) are used across the
sixteen iterations and the support layers.

## Verification checklist

The song is complete when:

- Compiles via `cargo run -p keleusma-cli -- compile examples/scripts/piano_roll/piano_roll_9.kel`.
- Loads through `Vm::new` against the default arena without a `VerifyError`.
- A headless probe shows the expected sixteen iteration transitions at the appropriate tick offsets. Each transition produces a reconfiguration of channel 1's waveform per the iteration's lead-timbre assignment.
- Per-iteration tempo profiles produce the expected ramps and sine modulation. The probe verifies that the BPM at the start of each section matches the specification (60, 180, 240, 250, 90, 300, 60 at the relevant section transitions).
- The confusion zone in each iteration produces audible destabilisation: tempo modulates continuously, scale shifts to whole-tone (iter 0..7) or Locrian (iter 8..15), channel 4 detune oscillates, and backing vocals harmonise chromatically rather than diatonically.
- Coherence restoration at the verse 2 onset: tempo snaps to 180 BPM, scale returns to home, detune resets, backing vocals return to scale-aware harmonisation.
- The half-step modulation at the final chorus onset is musically convincing within each iteration.
- The meta-loop boundary at iteration 15 to iteration 0 produces clean reconfiguration. The intro plays only on first song load.
- Workspace tests, clippy, fmt, release build all clean.

## Sheet music feasibility

### Overview

Sheet music for a sixteen-iteration variation cycle is
unusual but feasible. The standard approach is to publish the
underlying chord skeleton and melodic outline once, then
provide an iteration table listing the parameter changes for
each iteration. A performer realises each iteration by
applying the iteration's parameters to the shared template.

### Notation solution

The score consists of three parts.

Part 1: the shared composition. Twelve sections notated as a
standard pop-song lead sheet with chord symbols, melodic
outline, and arrangement notes. The chord symbols use
scale-degree notation rather than letter-name notation so they
generalise across the sixteen iterations. The lead-vocal staff
shows the melody in scale degrees (1, 3, 5, etc.) rather than
specific pitches.

Part 2: the iteration table. Sixteen rows, one per iteration,
listing the tonic, scale, lead waveform, and any
iteration-specific deviations. The performer reads the
relevant row when performing each iteration.

Part 3: the confusion-zone notation. A graphic-notation panel
showing the continuous tempo modulation curve and the
temporary scale shift. Standard staff notation cannot
straightforwardly represent the continuous tempo curve; the
score uses a tempo graph similar to song 4's master-score
addition.

### Master-score layout

| Staff | Channel | Role |
|-------|---------|------|
| (tempo graph) | --- | BPM curve across each section |
| 1 | Channel 1 | Lead vocal stand-in (notes in scale degrees) |
| 2 | Channel 6 | Backing harmony 1 |
| 3 | Channel 7 | Backing harmony 2 |
| 4 | Channel 2 | Lead arpeggio |
| 5 | Channel 3 | Harmony pad |
| 6 | Channel 4 | Rhythm pad |
| 7 | Channel 0 | Bass |
| 8 | Channel 5 | Drum kit |

### Verdict

Sheet music is feasible but the score functions more as
analytical documentation than as performance instruction. A
live ensemble could perform a single iteration of the piece
in conventional pop-band fashion; performing all sixteen
iterations as a fifty-minute set would require careful
preparation and the click-track support that the implementation
engine renders trivial. The implementation engine remains the
most authentic realisation.

## Pending implementation

The script `examples/scripts/piano_roll/piano_roll_9.kel` is not yet implemented.
The specification above provides the structural and musical
content required for the script-author pass.

The implementation effort is estimated at approximately 1500
to 2500 lines of Keleusma source. This is the largest
implementation in the roster because of the combination of
twelve-section per-iteration structure, sixteen-iteration
variation matrix, full per-tick tempo modulation, and the
confusion-zone destabilising properties.

The script will:

- Add `include_str!("piano_roll_9.kel")` to `SONG_SOURCES` at index 9 in `examples/piano_roll.rs`.
- Implement the iteration dispatcher computing iteration index from absolute tick.
- Implement the section dispatcher computing section index from within-iteration tick.
- Implement the per-iteration tonic and scale tables.
- Implement the chord-skeleton lookup that takes section, bar within section, and iteration tonic to return the bass root.
- Implement the lead-melody lookup using scale-degree-based pitches scaled by the iteration's scale to produce MIDI values.
- Implement the per-tick tempo computation supporting linear ramps, holds, and the sine modulation in the confusion zone.
- Implement the confusion-zone destabilising properties (scale shift, detune oscillation, chromatic backing harmonisation).
- Implement the per-iteration waveform reconfiguration on channel 1.
- Implement the half-step modulation at each iteration's final chorus.
- Implement the drum patterns per section.
- Update `book/src/PIANO_ROLL.md` to mention song 9.
- Update the module docstring in `examples/piano_roll.rs` to reflect the ten-song roster.
- Verify via headless probe, lib tests, clippy, fmt, and release build per the established discipline.

Per the lesson from backlog item B12, the per-tick body
should inline arithmetic directly and use small match-based
helper functions rather than nested helper-call chains. The
verifier will reject overly-nested helper structures.

## Working title and song-name string

The composition's working title is "Sixteen Sunrises". The
title reflects the sixteen-iteration structure where each
iteration is a fresh emergence of the same underlying
composition in a new sonic light. The metaphor is the
recurring dawn over a fixed landscape; the listener returns to
familiar territory sixteen times but each return is in
different light.

The host song-name string is
`"Keleusma Project: Sixteen Sunrises (0BSD)"`, following the
license-tag convention established by songs 3 through 8.
