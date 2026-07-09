# Song 8 specification: Sunlit Promise

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document is the implementation specification for
`examples/scripts/piano_roll/piano_roll_8.kel` (pending implementation), the
first mainstream-pop piece in the piano-roll roster. Song 8
demonstrates the conventional structural, harmonic, and
arrangement techniques of textbook Japanese pop songwriting.
The composition uses proven mainstream techniques throughout
and is deliberately unexperimental. The aesthetic target is
bright, accessible, and pleasant to a broad audience.

See the long-form manual at
[`book/src/PIANO_ROLL.md`](../../book/src/PIANO_ROLL.md) for the
broader piano-roll context, and the prior specs at
[`SONG_3_SPEC.md`](./SONG_3_SPEC.md) through
[`SONG_7_SPEC.md`](./SONG_7_SPEC.md) for the experimental
counterparts.

## Role in the roster

Index 8 in `SONG_SOURCES`. Joins the existing roster behind the
`sdl3-example` and `text` cargo features. Accessible at runtime
through the `s` (cycle), `r` (restart), and `8` (direct select)
input commands. The input watcher requires no host change.

## High-level brief

A textbook example of Japanese pop songwriting. The composition
is in C major for the verses, pre-choruses, and the first two
choruses; modulates to A minor (the relative minor) for the
bridge; and modulates up a half-step to D-flat major for the
final chorus, a technique that is virtually obligatory in
mainstream Japanese pop. The total length is approximately
three minutes and forty-two seconds at 108 beats-per-minute,
which is a conventional tempo and length for a radio-friendly
single.

The arrangement uses eight channels in a conventional pop-band
distribution: bass, lead vocal stand-in, lead arpeggio,
harmony pad, rhythm pad, drums, and a stereo-doubled
backing-vocal pair. The mix is fully stereo with the lead
centred and the harmonies spread across the field.

The composition adheres to proven mainstream techniques
throughout. There are no time-signature pivots, no microtonal
intervals, no continuous tempo modulation, no polymetric
canon, no minimalist process, no exotic scales. The piece's
distinguishing feature is its conventionality, which is itself
the spec's point: the piano-roll example should be able to
express mainstream pop with the same facility as it expresses
the experimental directions.

Approximate listening length: 3 minutes 42 seconds at 108 BPM.

## Design intent and accessibility strategy

### The conventionality contract

Songs 3 through 7 demonstrate the implementation engine
through experimental composition. Song 8 demonstrates the
engine through unexperimental composition. The piece is
deliberately conventional. It uses chord progressions a
broad audience recognises, melodic shapes that resolve as
expected, and an arrangement that mirrors commercial pop-band
practice.

The accessibility is the design. A listener encountering the
piano-roll example for the first time should hear at least one
song that sounds like the popular music they already know. The
experimental songs are valuable but they are a difficult
gateway to the example. Song 8 provides the easy gateway.

### Why Japanese pop specifically

The Japanese pop tradition has converged on a specific set of
conventions that distinguish it from Western pop while
remaining accessible. Three of these conventions are
prominently featured in song 8.

First, the modulation up a half-step or whole-step at the
final chorus. This technique is so prevalent in mainstream
Japanese pop that its absence in a song is more notable than
its presence. The modulation injects a lift of emotional
intensity at the song's climax without requiring new melodic
material; the existing chorus melody simply transposes one
semitone or two semitones higher.

Second, the use of the "royal road" chord progression
(IV-V-iii-vi) in pre-chorus passages. The descending bass
motion through the fourth, fifth, third, and sixth scale
degrees produces an emotional intensity that is recognisable
across the genre. The progression appears in a substantial
fraction of mainstream Japanese pop songs from the 1990s
onward.

Third, the staggered backing-vocal harmonies. Lead vocal in
the centre, third-above-lead and sixth-above-lead harmonies
panned left and right. This stereo arrangement is virtually
universal in produced Japanese pop.

### Why the title is in English

Many Japanese pop songs use single-word or short-phrase
English titles, often slightly poetic. "Promise", "Memory",
"Colour", "Light", "Smile", "Forever", and similar one-word
titles are widely used in the genre. The title "Sunlit
Promise" follows this convention. The two-word English title
evokes the bright, optimistic, slightly nostalgic mood that
is characteristic of mainstream Japanese pop in major-key
emotional songs.

### Contrast with the experimental songs

Songs 3 through 7 each push one or more dimensions of the
engine to a deliberate extreme. Song 8 sits in the middle of
every dimension. The tempo is moderate. The harmony is
mainstream major. The structure is conventional. The
arrangement is pop-band-standard. The aesthetic is friendly.

A first-time listener should be able to skip directly to song
8 and hear something that sounds immediately familiar. The
listener who has spent time with songs 3 through 7 will
appreciate song 8 as the demonstration that the engine handles
convention with the same facility as it handles experiment.

## Design constraints

- Conventional pop song structure: intro, verse, pre-chorus, chorus, interlude, verse, pre-chorus, chorus, bridge, modulation, final chorus, outro.
- Three to four minute total length. Song 8 targets approximately three minutes forty-two seconds.
- 108 BPM, 4/4 throughout. No tempo modulation, no time-signature pivots.
- C major for the first two-thirds of the song, A minor for the bridge (relative minor modulation), D-flat major for the final chorus (half-step-up modulation).
- Eight channels in pop-band arrangement: bass, lead, lead arpeggio, harmony pad, rhythm pad, drums, two backing-vocal harmonies.
- Fully stereo. Lead centred, backing harmonies hard-panned, arpeggios slightly panned, drums and bass centred, pads spread.
- Phrase variety: the lead melody varies between verse, pre-chorus, chorus, and bridge phrases. The chorus carries a hook that recurs across all chorus statements.
- No microtonal intervals. All pitches are twelve-tone equal temperament. `host::set_detune` is used only for chorus thickness on the backing harmonies (a few cents of detune for stereo widening, not for pitch identity).
- No experimental devices. No polymetric canon, no sine-wave tempo modulation, no phase drift, no whole-tone or Locrian scales. The harmonic language is diatonic with one secondary dominant (the E major chord functioning as V of vi) used in the pre-chorus build.

## Section structure

The composition is one hundred bars long at 16 ticks per bar,
totaling 1600 ticks. At 108 BPM (one tick equals approximately
139 milliseconds), the total length is approximately 222
seconds (3 minutes 42 seconds).

| Section | Bars | Ticks | First-iteration tick range | Key | Description |
|---------|------|-------|----------------------------|-----|-------------|
| Intro | 8 | 128 | 0..128 | C major | Establishes tempo and key. Light arpeggio over chord pad. Drums enter on bar 5. |
| Verse 1 | 8 | 128 | 128..256 | C major | Lead melody states the verse theme. Light arrangement: bass, drums, pad, lead. |
| Pre-chorus 1 | 8 | 128 | 256..384 | C major | Royal road progression (IV-V-iii-vi). Backing harmonies join. Build into chorus. |
| Chorus 1 | 12 | 192 | 384..576 | C major | Full arrangement. Hook melody. Backing vocals at thirds and sixths above lead. |
| Interlude | 4 | 64 | 576..640 | C major | Brief instrumental passage. Lead arpeggio carries the hook melody alone. |
| Verse 2 | 8 | 128 | 640..768 | C major | Verse theme repeats with slight melodic variation. Same arrangement as verse 1. |
| Pre-chorus 2 | 8 | 128 | 768..896 | C major | Royal road again. |
| Chorus 2 | 12 | 192 | 896..1088 | C major | Identical structure to chorus 1. |
| Bridge | 8 | 128 | 1088..1216 | A minor (relative minor) | New melodic material in the relative minor. Less dense arrangement; allows breath. |
| Modulation | 4 | 64 | 1216..1280 | A-flat major as V of D-flat | Pivot passage. Bass and pad sustain on A-flat to set up the half-step modulation. Drums roll into the final chorus. |
| Final chorus | 16 | 256 | 1280..1536 | D-flat major | The chorus material transposed up one semitone. Half-step modulation is the textbook J-pop climax device. Maximum arrangement density. |
| Outro | 4 | 64 | 1536..1600 | D-flat major | Final cadence on D-flat. Voices drop out one by one. Last note is bass D-flat held with pad. |

Loop boundary: the runtime computes `loop_tick = input mod
1600`. At `input == 1600` the formula gives `loop_tick = 0`,
returning to the intro. The intro plays on every loop
iteration because the song is short enough that the intro is
part of the listening experience even on repeat.

## Key, scale, and chord progressions

### C major (sections 0 through 7)

The home key. Diatonic scale: C, D, E, F, G, A, B. All chord
progressions in the verse, pre-chorus, chorus 1, interlude,
verse 2, pre-chorus 2, and chorus 2 draw from this scale plus
one secondary-dominant chromatic note (G-sharp, the major
third of E, used in the E7 secondary dominant resolving to
A minor in the pre-chorus's royal-road progression).

### A minor (bridge)

Relative minor of C major. Shares all notes with C major (no
key signature change). Heard as centred on A. The bridge uses
the A natural minor scale; the harmonic minor's raised seventh
(G-sharp) is used once on the V chord (E major) to produce the
characteristic leading-tone cadence back to the tonic.

### D-flat major (final chorus)

Half-step modulation up from C major. Diatonic scale: D-flat,
E-flat, F, G-flat, A-flat, B-flat, C. All five black keys plus
F and C. The modulation from C major to D-flat major has no
common-tone pivot; it is a direct chromatic shift achieved by
holding the V of D-flat (A-flat major) for the modulation bar
and resolving to D-flat at the start of the final chorus. The
unprepared modulation is the textbook Japanese pop technique.

### Chord progression per section

The chord progressions below use root MIDI values for the bass
voice. All roots are in the C2 to B2 octave (MIDI 36 to 47)
for the C major and A minor sections and in the C2 to B2
octave (MIDI 36 to 47) for the D-flat major section as well.

**Intro (8 bars).** Two cycles of I-IV-V-vi-IV-V-I.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | C | 36 (C2) |
| 2 | F | 41 (F2) |
| 3 | G | 43 (G2) |
| 4 | C | 36 |
| 5 | Am | 45 (A2) |
| 6 | F | 41 |
| 7 | G | 43 |
| 8 | C | 36 |

**Verse 1 and Verse 2 (8 bars each).** Two cycles of I-V-vi-IV.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | C | 36 |
| 2 | G | 43 |
| 3 | Am | 45 |
| 4 | F | 41 |
| 5 | C | 36 |
| 6 | G | 43 |
| 7 | Am | 45 |
| 8 | F | 41 |

**Pre-chorus 1 and Pre-chorus 2 (8 bars each).** Royal road (IV-V-iii-vi) plus turnaround.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | F | 41 |
| 2 | G | 43 |
| 3 | Em | 40 (E2) |
| 4 | Am | 45 |
| 5 | F | 41 |
| 6 | G | 43 |
| 7 | C | 36 |
| 8 | G | 43 |

The E minor in bar 3 functions as the mediant. The descending
bass line F-G-E-A-F-G-C-G produces the emotional lift that
defines the royal-road technique.

**Chorus 1 and Chorus 2 (12 bars each).** Three cycles of I-V-vi-IV.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | C | 36 |
| 2 | G | 43 |
| 3 | Am | 45 |
| 4 | F | 41 |
| 5 | C | 36 |
| 6 | G | 43 |
| 7 | Am | 45 |
| 8 | F | 41 |
| 9 | C | 36 |
| 10 | G | 43 |
| 11 | F | 41 |
| 12 | G | 43 |

The final two bars (F-G) replace the third cycle's Am-F to
provide a stronger setup for the next section.

**Interlude (4 bars).** I-IV-V-I.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | C | 36 |
| 2 | F | 41 |
| 3 | G | 43 |
| 4 | C | 36 |

**Bridge (8 bars in A minor).** Two cycles of i-VI-iv-V with harmonic-minor V.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | Am | 45 |
| 2 | F | 41 |
| 3 | Dm | 38 (D2) |
| 4 | E | 40 (E2) |
| 5 | Am | 45 |
| 6 | F | 41 |
| 7 | Dm | 38 |
| 8 | E | 40 |

The E major chord in bars 4 and 8 contains G-sharp (the
harmonic-minor raised seventh), producing the dominant
function that pulls back to the A minor tonic.

**Modulation (4 bars).** Sustained V of D-flat.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | A-flat | 44 (A♭2) |
| 2 | A-flat | 44 |
| 3 | A-flat | 44 |
| 4 | A-flat7 | 44 |

The four-bar A-flat dominant pedal sets up the half-step-up
modulation. The bar-four chord adds the dominant seventh
(G-flat) to strengthen the resolution to D-flat at the final
chorus onset.

**Final chorus (16 bars in D-flat major).** Four cycles of I-V-vi-IV with cadential extension.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | D♭ | 37 (D♭2) |
| 2 | A♭ | 44 |
| 3 | B♭m | 46 (B♭2) |
| 4 | G♭ | 42 (G♭2) |
| 5 | D♭ | 37 |
| 6 | A♭ | 44 |
| 7 | B♭m | 46 |
| 8 | G♭ | 42 |
| 9 | D♭ | 37 |
| 10 | A♭ | 44 |
| 11 | G♭ | 42 |
| 12 | A♭ | 44 |
| 13 | D♭ | 37 |
| 14 | G♭ | 42 |
| 15 | A♭ | 44 |
| 16 | D♭ | 37 |

The final four bars carry the cadence: I-IV-V-I, the
strongest possible resolution to the new tonic.

**Outro (4 bars).** D-flat with G-flat plagal cadence.

| Bar | Chord | Root MIDI |
|-----|-------|-----------|
| 1 | D♭ | 37 |
| 2 | G♭ | 42 |
| 3 | D♭ | 37 |
| 4 | D♭ | 37 |

The outro fades out across these four bars. Voices drop out
one by one.

## Channel assignments

| Channel | Role | Waveform | Stereo position | Pan rationale |
|---------|------|----------|-----------------|---------------|
| 0 | Bass guitar stand-in | Sawtooth (code 2) with LPF at 1200 Hz | Centre (500/500) | Bass sits centred in pop mixes. |
| 1 | Lead vocal stand-in | Sawtooth (code 2) with LPF at 4000 Hz | Centre (500/500) | Lead vocal centred in pop mixes. |
| 2 | Lead arpeggio | Pulse (code 4) with duty 500 | Slightly left (650/350) | Lead arpeggio counter-melody. |
| 3 | Harmony pad | Triangle (code 1) with vibrato | Slightly right (350/650) | Harmonic warmth without dominating. |
| 4 | Rhythm pad | Pulse (code 4) with duty 250, LPF 2000 | Slight left (600/400) | Rhythm-guitar role, slight stereo movement. |
| 5 | Drums | Noise (code 5) with dynamic ADSR | Centre (500/500) | Drums centred. |
| 6 | Backing vocal harmony 1 (third above) | Sawtooth (code 2) with +3 cents detune | Hard left (1000/0) | Stereo widening. |
| 7 | Backing vocal harmony 2 (sixth above) | Sawtooth (code 2) with -3 cents detune | Hard right (0/1000) | Mirror of channel 6. |

The bass and drums occupy the centre of the stereo field as
they do in nearly all pop mixes. The lead vocal stand-in also
sits centred. The lead arpeggio, harmony pad, and rhythm pad
spread slightly across the field. The two backing-vocal
harmonies are hard-panned to opposite speakers for maximum
stereo width during chorus sections.

The slight detune offsets on channels 6 and 7 (plus and minus
three cents respectively) produce a chorus-like thickening
similar to the song 3 stereo unison pair, but the detune is
much narrower because pop backing vocals are typically
in-tune rather than chorused.

## Lead melody

The lead melody is the song's primary expressive element.
The melody varies by section to provide the phrase variety
that the design constraints require.

### Verse melody (8 bars, 4 notes per bar = 32 notes total)

In C major. Conjunct (stepwise) motion, modest range,
descending overall contour to give the verse a settled
character.

```
Bars 1-2 (C, G):     [72, 76, 79, 76, 74, 77, 71, 67]
                     C5 E5 G5 E5 D5 F5 B4 G4
Bars 3-4 (Am, F):    [69, 72, 76, 72, 65, 69, 72, 69]
                     A4 C5 E5 C5 F4 A4 C5 A4
Bars 5-6 (C, G):     [72, 76, 79, 81, 74, 79, 83, 79]
                     C5 E5 G5 A5 D5 G5 B5 G5
Bars 7-8 (Am, F):    [81, 79, 76, 72, 77, 76, 74, 72]
                     A5 G5 E5 C5 F5 E5 D5 C5
```

Verse 2 uses the same pitches with a small variation on the
final two bars (bars 7-8) to provide melodic interest on
repeat:

```
Bars 7-8 variation:  [81, 79, 76, 74, 72, 74, 76, 79]
                     A5 G5 E5 D5 C5 D5 E5 G5
```

The verse 2 ending ascends back to G5 to lead into pre-chorus
2 rather than descending to C5.

### Pre-chorus melody (8 bars, 4 notes per bar = 32 notes total)

In C major. Ascending overall contour. Builds momentum into
the chorus.

```
Bars 1-2 (F, G):     [77, 79, 81, 79, 79, 81, 83, 81]
                     F5 G5 A5 G5 G5 A5 B5 A5
Bars 3-4 (Em, Am):   [79, 83, 79, 76, 81, 84, 83, 81]
                     G5 B5 G5 E5 A5 C6 B5 A5
Bars 5-6 (F, G):     [77, 81, 84, 81, 79, 83, 86, 83]
                     F5 A5 C6 A5 G5 B5 D6 B5
Bars 7-8 (C, G):     [79, 76, 72, 76, 79, 74, 79, 86]
                     G5 E5 C5 E5 G5 D5 G5 D6
```

The pre-chorus ends on D6, the highest pitch yet, which
prepares the chorus's high range.

### Chorus hook (12 bars, 8 notes per bar = 96 notes total)

In C major. The chorus is the song's most memorable melodic
content. The hook spans 4 bars and is stated three times
across the 12-bar chorus.

4-bar hook (32 notes):

```
Bar 1 (C):    [79, 79, 76, 79, 84, 84, 83, 79]
              G5 G5 E5 G5 C6 C6 B5 G5
Bar 2 (G):    [79, 74, 77, 74, 79, 83, 86, 83]
              G5 D5 F5 D5 G5 B5 D6 B5
Bar 3 (Am):   [81, 81, 79, 76, 81, 84, 83, 81]
              A5 A5 G5 E5 A5 C6 B5 A5
Bar 4 (F):    [77, 81, 79, 77, 76, 77, 79, 81]
              F5 A5 G5 F5 E5 F5 G5 A5
```

The hook is stated three times for the full 12-bar chorus.
The third statement's bar 4 alters slightly to set up the
section transition:

```
Bar 12 variation: [77, 79, 81, 83, 81, 79, 77, 76]
                  F5 G5 A5 B5 A5 G5 F5 E5
```

This descending tail in the final bar resolves to the next
section's opening.

### Interlude melody (4 bars, 8 notes per bar)

The interlude restates the chorus hook on the lead arpeggio
channel (channel 2) rather than on the lead vocal channel.
The lead vocal channel rests during the interlude.

The arpeggio pitches are the chorus hook pitches transposed
up one octave (so G6, E6, C6, etc.) to give the interlude a
brighter, lighter character than the chorus.

```
Bar 1: [91, 91, 88, 91, 96, 96, 95, 91]
Bar 2: [91, 86, 89, 86, 91, 95, 98, 95]
Bar 3: [93, 93, 91, 88, 93, 96, 95, 93]
Bar 4: [89, 93, 91, 89, 88, 89, 91, 93]
```

### Bridge melody (8 bars, 4 notes per bar = 32 notes total)

In A minor. The melody is more conjunct and has a darker
contour than the major-key sections.

```
Bars 1-2 (Am, F):    [69, 72, 76, 72, 65, 69, 72, 69]
                     A4 C5 E5 C5 F4 A4 C5 A4
Bars 3-4 (Dm, E):    [62, 65, 69, 65, 64, 68, 71, 76]
                     D4 F4 A4 F4 E4 G#4 B4 E5
Bars 5-6 (Am, F):    [69, 72, 76, 81, 72, 69, 65, 64]
                     A4 C5 E5 A5 C5 A4 F4 E4
Bars 7-8 (Dm, E):    [65, 69, 74, 72, 71, 68, 64, 64]
                     F4 A4 D5 C5 B4 G#4 E4 E4
```

Bar 4 and bar 8 each include G-sharp (MIDI 68), the harmonic-
minor raised seventh, on the E major chord. The G-sharp
provides the leading-tone resolution back to A.

### Modulation melody (4 bars, 4 notes per bar = 16 notes)

The modulation passage has a simple ascending lead line that
rises into the new key. The melody anticipates the final
chorus's D-flat by ending on A-flat (the V of D-flat).

```
Bar 1 (A♭): [68, 71, 76, 80]    G#4 B4 E5 A♭5
Bar 2 (A♭): [80, 76, 72, 80]    A♭5 E5 C5 A♭5
Bar 3 (A♭): [80, 83, 87, 91]    A♭5 B5 E♭6 A♭6
Bar 4 (A♭7): [91, 87, 83, 80]   A♭6 E♭6 B5 A♭5
```

The lead drops one octave at the end of bar 4 (from A♭6 down
to A♭5) so the final chorus's first note (A♭5 in the
transposed hook) is reachable without a leap.

### Final chorus melody (16 bars, 8 notes per bar = 128 notes total)

The final chorus's first 12 bars are the chorus hook
transposed up one semitone (every MIDI value increases by 1).
The final 4 bars carry the cadential climax.

```
Bar 1 (D♭):  [80, 80, 77, 80, 85, 85, 84, 80]
Bar 2 (A♭):  [80, 75, 78, 75, 80, 84, 87, 84]
Bar 3 (B♭m): [82, 82, 80, 77, 82, 85, 84, 82]
Bar 4 (G♭):  [78, 82, 80, 78, 77, 78, 80, 82]
```

The hook is stated three times across bars 1 through 12 of
the final chorus. The fourth statement is the cadential
extension (bars 13 through 16):

```
Bar 13 (D♭): [85, 85, 84, 80, 82, 80, 77, 80]
Bar 14 (G♭): [82, 80, 78, 75, 78, 80, 82, 84]
Bar 15 (A♭): [85, 84, 82, 80, 82, 84, 85, 87]
Bar 16 (D♭): [89, 87, 85, 84, 85, 80, 77, 80]
```

The final bar's last note is A♭5 (MIDI 80) holding into the
outro.

## Backing vocal harmonies (channels 6 and 7)

The backing vocals are active only during chorus 1, chorus 2,
and the final chorus. They are silent during the verses,
pre-choruses, interlude, bridge, and outro.

The backing vocals follow the lead melody at fixed diatonic
intervals using scale-aware harmonization, similar to song 3's
parallel-interval technique. The intervals depend on the key:

In C major (chorus 1 and chorus 2):
- Channel 6: third above lead. Major or minor third depending on the lead's scale degree, computed by adding 4 or 3 semitones so the harmonization stays diatonic.
- Channel 7: sixth above lead. Major or minor sixth (9 or 8 semitones above), also scale-aware.

In D-flat major (final chorus):
- Same intervals applied. The diatonic intervals in D-flat major are computed by adding the same scale-degree offsets.

The exact diatonic-third and diatonic-sixth pattern in C major:

| Lead pitch class | Third above (semitones) | Sixth above (semitones) |
|------------------|-------------------------|--------------------------|
| C (0) | +4 (E) | +9 (A) |
| D (2) | +3 (F) | +9 (B) |
| E (4) | +3 (G) | +8 (C) |
| F (5) | +4 (A) | +9 (D) |
| G (7) | +4 (B) | +9 (E) |
| A (9) | +3 (C) | +8 (F) |
| B (11) | +3 (D) | +8 (G) |

In D-flat major, all pitch classes shift up by 1 but the
interval pattern (in terms of scale degrees rather than
semitones) is identical. The implementation can apply a
constant +1 semitone offset to channel 6 and channel 7
during the final chorus.

## Drum pattern

The drum pattern is standard 4/4 pop drumming. Channel 5
fires the kick, snare, and hi-hat through dynamic ADSR
swapping (the same pattern used in songs 3, 4, and 6).

**Verse and pre-chorus drum pattern (per bar, 16 ticks):**

| Tick | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|------|---|---|---|---|---|---|---|---|---|---|----|----|----|----|----|----|
| Hit | K | . | H | . | S | . | H | . | K | . | H | . | S | . | H | . |

**Chorus drum pattern (per bar):**

| Tick | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|------|---|---|---|---|---|---|---|---|---|---|----|----|----|----|----|----|
| Hit | K | . | H | . | S | . | H | H | K | K | H | . | S | . | H | H |

The chorus pattern adds a kick on tick 9 and hi-hats on ticks
7 and 15 for additional drive.

**Bridge drum pattern (per bar):**

Simpler, less dense pattern to allow the bridge to breathe:

| Tick | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|------|---|---|---|---|---|---|---|---|---|---|----|----|----|----|----|----|
| Hit | K | . | . | . | S | . | . | . | K | . | . | . | S | . | . | . |

**Final chorus drum pattern:**

Same as the regular chorus pattern. Drum fills mark the
section boundaries (last bar of chorus 2, last bar of
modulation, final bar of song).

**Drum fill (last bar of modulation, bar 4 of section):**

Override the bar's pattern with an accelerating snare roll:

| Tick | 0 | 2 | 4 | 6 | 8 | 10 | 12 | 14 |
|------|---|---|---|---|---|----|----|----|
| Hit | S | S | S | S | S | S | S | S |

Velocity ramps from 500 to 1000 across the 8 hits.

## Mid-song event schedule

### Init block

```
host::song_name("Keleusma Project: Sunlit Promise (0BSD)");
host::set_bpm(108);
host::set_master_volume(850);

// Channel 0: Bass guitar stand-in.
host::set_waveform(0, 2);
host::set_adsr(0, 5, 80, 700, 150);
host::set_volume(0, 500, 500);
host::set_velocity(0, 800);
host::set_retrigger(0, 1);
host::set_vibrato(0, 0, 0);
host::set_lpf(0, 1200);
host::set_detune(0, 0);
host::set_enable(0, 1);

// Channel 1: Lead vocal stand-in.
host::set_waveform(1, 2);
host::set_adsr(1, 10, 100, 800, 200);
host::set_volume(1, 500, 500);
host::set_velocity(1, 900);
host::set_retrigger(1, 0);
host::set_vibrato(1, 500, 25);
host::set_lpf(1, 4000);
host::set_detune(1, 0);
host::set_enable(1, 1);

// Channel 2: Lead arpeggio.
host::set_waveform(2, 4);
host::set_duty(2, 500);
host::set_adsr(2, 2, 60, 500, 100);
host::set_volume(2, 650, 350);
host::set_velocity(2, 700);
host::set_retrigger(2, 1);
host::set_vibrato(2, 0, 0);
host::set_lpf(2, 0);
host::set_detune(2, 0);
host::set_enable(2, 1);

// Channel 3: Harmony pad.
host::set_waveform(3, 1);
host::set_adsr(3, 50, 200, 700, 400);
host::set_volume(3, 350, 650);
host::set_velocity(3, 600);
host::set_retrigger(3, 0);
host::set_vibrato(3, 400, 30);
host::set_lpf(3, 0);
host::set_detune(3, 0);
host::set_enable(3, 1);

// Channel 4: Rhythm pad.
host::set_waveform(4, 4);
host::set_duty(4, 250);
host::set_adsr(4, 30, 150, 600, 250);
host::set_volume(4, 600, 400);
host::set_velocity(4, 600);
host::set_retrigger(4, 0);
host::set_vibrato(4, 0, 0);
host::set_lpf(4, 2000);
host::set_detune(4, 0);
host::set_enable(4, 1);

// Channel 5: Drums.
host::set_waveform(5, 5);
host::set_volume(5, 500, 500);
host::set_velocity(5, 800);
host::set_retrigger(5, 1);
host::set_enable(5, 1);

// Channel 6: Backing harmony 1 (third above lead).
host::set_waveform(6, 2);
host::set_adsr(6, 10, 100, 750, 200);
host::set_volume(6, 1000, 0);
host::set_velocity(6, 650);
host::set_retrigger(6, 0);
host::set_detune(6, 3);
host::set_vibrato(6, 500, 20);
host::set_lpf(6, 0);
host::set_enable(6, 0);

// Channel 7: Backing harmony 2 (sixth above lead).
host::set_waveform(7, 2);
host::set_adsr(7, 10, 100, 750, 200);
host::set_volume(7, 0, 1000);
host::set_velocity(7, 650);
host::set_retrigger(7, 0);
host::set_detune(7, -3);
host::set_vibrato(7, 450, 20);
host::set_lpf(7, 0);
host::set_enable(7, 0);
```

### Per-tick events

The per-tick body dispatches on the current section. The
section is determined by the loop-tick position. Each section
has its own chord-progression, melody, and drum-pattern
behaviour.

Per the lesson from B12, the per-tick body uses direct
arithmetic and inlined chord-progression lookups via match
statements. Helper-function calls in the hot path are
minimised.

The section dispatch:

```
let loop_tick = input mod 1600;
let section = if loop_tick < 128 { 0 }       // Intro
              else if loop_tick < 256 { 1 }  // Verse 1
              else if loop_tick < 384 { 2 }  // Pre-chorus 1
              else if loop_tick < 576 { 3 }  // Chorus 1
              else if loop_tick < 640 { 4 }  // Interlude
              else if loop_tick < 768 { 5 }  // Verse 2
              else if loop_tick < 896 { 6 }  // Pre-chorus 2
              else if loop_tick < 1088 { 7 } // Chorus 2
              else if loop_tick < 1216 { 8 } // Bridge
              else if loop_tick < 1280 { 9 } // Modulation
              else if loop_tick < 1536 { 10 } // Final chorus
              else { 11 };                    // Outro
```

The script reads `section` and dispatches the bass, lead,
arpeggio, pad, and drum events per section. Backing harmonies
are enabled when entering chorus 1, chorus 2, and the final
chorus, and disabled when leaving each chorus.

### Section-onset reconfiguration

Each major section transition fires a small number of
one-shot natives to reconfigure channels:

- At Chorus 1 onset (tick 384): enable channels 6 and 7.
- At Chorus 1 end (tick 576): disable channels 6 and 7.
- At Chorus 2 onset (tick 896): enable channels 6 and 7.
- At Chorus 2 end (tick 1088): disable channels 6 and 7.
- At Final Chorus onset (tick 1280): enable channels 6 and 7. Also increase master volume to 950 for the climax.
- At Outro onset (tick 1536): disable channels 6 and 7. Decrease master volume linearly across the outro.

## Coverage matrix

Song 8 has a moderate coverage profile. Most natives are used
in active states; few are used dynamically. The piece is
deliberately conventional rather than feature-demonstrating.

| Native | Coverage |
|--------|----------|
| `host::set_enable` | Channels 0 through 5 active at init. Channels 6 and 7 enabled per-chorus and disabled between choruses. Dynamic at section boundaries. |
| `host::set_waveform` | Four distinct waveforms across channels (Sawtooth, Pulse, Triangle, Noise). Static within the piece. |
| `host::set_duty` | Active on channels 2 and 4 at 500 and 250 respectively. Static. |
| `host::set_adsr` | Per-channel envelopes at init. Channel 5 percussion swaps per beat between kick, snare, and hi-hat profiles. |
| `host::set_volume` | Per-channel stereo positions at init. Static. |
| `host::set_vibrato` | Active on channels 1, 3, 6, 7 for vocal-like expression. Static. |
| `host::set_lpf` | Active on channels 0, 1, 4 at various cutoff values. Static. |
| `host::set_retrigger` | Channel 0, 2, 5 retrigger on for sharp attacks. Channels 1, 3, 4, 6, 7 retrigger off for legato. Static. |
| `host::set_detune` | Channels 6 and 7 carry ±3 cents for stereo widening of the backing harmonies. Static. The final chorus modulation up one semitone is achieved by adjusting MIDI pitches by +1, not by detune. |
| `host::set_velocity` | Per-channel base velocities at init. Per-tick percussion velocity. Dynamic at chorus and final-chorus boundaries (subtle ramps). |
| `host::set_master_volume` | 850 at init, 950 at the final chorus onset, ramps down across the outro. Dynamic at three points. |
| `host::set_bpm` | 108 per tick. Static value. |
| `host::song_name` | Called once in init. |
| `host::play` | Per-note on each active channel. |
| `host::silence` | Not used. Channels are disabled via `set_enable` rather than silenced via `silence`. |

The dynamic feature use is concentrated in the
chorus-and-final-chorus enable transitions and the master
volume changes. The piece is the roster's "mainstream
convention" exhibit, complementary to the experimental songs
that exercise dynamic features intensely.

## Verification checklist

The song is complete when:

- Compiles via `cargo run -p keleusma-cli -- compile examples/scripts/piano_roll/piano_roll_8.kel`.
- Loads through `Vm::new` against the default arena without a `VerifyError`.
- A headless probe shows the expected section transitions at tick 128, 256, 384, 576, 640, 768, 896, 1088, 1216, 1280, and 1536. The probe also shows the channel 6 and 7 enable transitions at chorus boundaries.
- The audible texture begins with the intro arpeggio, builds into the verse, pre-chorus, and chorus, with the chorus carrying clearly the hook melody. The interlude follows. Verse 2 repeats with the slight melodic variation in bars 7-8. The bridge in A minor is audibly different from the C major chorus. The modulation passage's four bars of A-flat dominant set up the half-step modulation. The final chorus is audibly higher than the previous choruses by exactly one semitone. The outro fades to silence.
- The half-step modulation is musically convincing. The transition from A-flat dominant to D-flat resolves cleanly with no perceived dissonance or modulation glitch.
- The backing-vocal harmonies are audible during the choruses, contributing stereo width and harmonic density, and absent during the verses and bridge.
- Workspace tests, clippy, fmt, release build all clean.

## Sheet music feasibility

### Overview

Sheet music for a textbook J-pop song is entirely standard
practice. The score uses conventional notation throughout:
treble and bass staves, standard key signatures, standard
chord symbols, and standard tempo markings. The composition
is realisable by a live ensemble of vocals, piano (or
keyboards), bass, drums, and rhythm guitar with no special
techniques required.

### Notation solution

The score is laid out as a piano-vocal-band lead sheet with
the following elements:

- Top of page: title ("Sunlit Promise"), composer (Keleusma Project), tempo ("♩ = 108"), key signature (C major initially), time signature (4/4).
- Vocal staff: melody with lyrics. Lyrics are not part of the implementation engine's output but would be added by the lyricist.
- Chord symbols: standard alphabetic notation (C, Am, F, G, Dm, Em, A♭, D♭, etc.) above the vocal staff.
- Piano accompaniment: two-staff piano notation showing the rhythm pattern and harmonic content.
- Key signature change: at the bridge, no change (relative minor uses the same key signature). At the modulation, key signature changes to D-flat major (five flats) for the final chorus and outro.

### Performance considerations

A live ensemble can perform the piece directly from the lead
sheet. The half-step modulation requires the lead vocalist
to adjust their pitch up one semitone at the modulation
moment; this is a routine technique in pop singing and
requires no special training. The drums and bass adjust their
patterns to the new key by transposing root motion up one
semitone.

The arrangement scales well. A simplified version with just
piano and vocal works. The full arrangement with rhythm
section, lead arpeggio, and backing vocals works. The piece
is structurally simple and the performance demands are
modest.

### Master-score layout

| Staff | Channel | Role |
|-------|---------|------|
| 1 (treble) | Channel 1 | Lead vocal stand-in |
| 2 (treble) | Channel 6 | Backing harmony 1 (third above lead) |
| 3 (treble) | Channel 7 | Backing harmony 2 (sixth above lead) |
| 4 (treble) | Channel 2 | Lead arpeggio |
| 5 (treble) | Channel 3 | Harmony pad |
| 6 (treble) | Channel 4 | Rhythm pad |
| 7 (bass) | Channel 0 | Bass |
| 8 | Channel 5 | Drum kit on a five-line drum staff |

### Verdict

Sheet music is fully feasible and conventional. The score is
publishable through standard pop music publication. A live
performance by an ensemble of three to five musicians is
direct and requires no special accommodation for the
implementation engine.

## Pending implementation

The script `examples/scripts/piano_roll/piano_roll_8.kel` is not yet implemented.
The specification above provides the structural and musical
content required for the script-author pass. The script will:

- Add `include_str!("piano_roll_8.kel")` to `SONG_SOURCES` at index 8 in `examples/piano_roll.rs`.
- Implement the section-dispatch logic via inlined if-else chain or match expression on the section identifier.
- Implement the chord-progression lookups via match expressions returning the root MIDI value per bar.
- Implement the lead melody lookups via match expressions returning the MIDI value per note position.
- Implement the per-section channel reconfiguration for chorus backing-vocal enables and master-volume changes.
- Implement the drum patterns for verse, chorus, bridge, and the modulation drum fill.
- Implement the scale-aware diatonic-third and diatonic-sixth harmonization on channels 6 and 7 (similar to the song 3 parallel-interval implementation).
- Apply the +1 semitone shift to all melody and chord pitches during the final chorus and outro to realise the half-step modulation.
- Update `book/src/PIANO_ROLL.md` to mention song 8.
- Update the module docstring in `examples/piano_roll.rs` to reflect the nine-song roster.
- Verify via headless probe, lib tests, clippy, fmt, and release build per the established discipline.

The implementation effort is estimated at approximately 600 to
800 lines of Keleusma source, comparable in scale to song 6's
implementation. The principal implementation challenges are
the section dispatch (twelve sections, each with its own
content), the scale-aware harmonization on the backing
voices, and the half-step modulation logic that must adjust
the pitches consistently across all active channels at the
final-chorus boundary.

Per the lesson from backlog item B12, the per-tick body
should inline chord-root lookups and melody lookups directly
rather than delegating to helper functions returning values
through multiple levels of indirection.

## Working title and song-name string

The composition's working title is "Sunlit Promise". The
title follows the textbook Japanese-pop convention of a
short English title with bright, optimistic, slightly
nostalgic connotations. The host song-name string is
`"Keleusma Project: Sunlit Promise (0BSD)"`, following the
license-tag convention established by songs 3 through 7.
