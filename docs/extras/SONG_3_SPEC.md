# Song 3 specification: 16-bit-style intro plus loop

> **Navigation**: [Extras](./README.md) | [Documentation Root](../README.md)

This document is the implementation specification for
[`examples/scripts/piano_roll/piano_roll_3.kel`](../../examples/scripts/piano_roll/piano_roll_3.kel),
the long-form eight-channel boss-theme stress test in the
piano-roll example. The spec was constructed iteratively across
several rounds of collaboration between an implementation engine
and an external musical-design session. The structure has
solidified, the eight-channel layout is final, and the concrete
chord matrix, arpeggio interval-pattern vectors, percussion mask,
and lead-melody pitches for the structurally critical sections
are committed. Sections marked as derivable in the script-author
pass are filled at implementation time from the chord matrix and
the per-section character notes.

See the long-form manual at
[`book/src/PIANO_ROLL.md`](../../book/src/PIANO_ROLL.md) for the
broader piano-roll context.

## Role in the roster

Index 3 in `SONG_SOURCES`. Joins the existing roster behind the
`sdl3-example` and `text` cargo features. Accessible at runtime
through the `s` (cycle), `r` (restart), and `3` (direct select)
input commands.

## High-level brief

A full-length 16-bit-style chiptune piece patterned spiritually
on the boss-theme idiom of mid-1990s Japanese role-playing-game
soundtracks. The script is
intentionally not short. The goal is a complete musical
experience that demonstrates dynamic use of every host native,
not a minimal feature checklist.

Structure: 192-tick unlooped intro followed by a 768-tick looped
main body. Total first-iteration length is 960 ticks,
approximately eighty-eight seconds. Each subsequent loop is
approximately sixty-six seconds. The intro plays once when the
song loads; the main body loops until the user swaps out or
quits.

The composition spans approximately five and three-quarter
octaves of register simultaneously by allocating each of five
active channels to a distinct pitch band from sub-bass (MIDI 24,
C1) to high lead (MIDI 93, A6). The tempo travels 90 BPM at
the opening to 250 BPM at the climax, with multiple intra-loop
shifts.

Spiritual reference targets: a 16-bit role-playing-game golden-
age boss theme with tempo and register dramatics that match the
genre. Final-boss "desperation phase" framing is explicit in
the structure (the Limit Break, Chaos, and Crash sections).

## Design constraints

- All host features in use somewhere. Feature inventory in the
  module docstring at the top of
  [`examples/piano_roll.rs`](../../examples/piano_roll.rs).
  "In use" means called by the script, not merely defaulted by
  the host.
- At least one dynamic event per dynamic-capable feature. BPM
  changes mid-piece. Master volume tapers somewhere. Per-channel
  parameters (waveform, ADSR, vibrato, LPF, detune, velocity)
  vary across sections.
- Variable tempo with user-directed trajectory. Intro starts at
  90 BPM and ramps to 150 BPM. The main loop cruises at 150 BPM
  with a hyper-accelerando section ramping to 250 BPM, a brief
  hold at 250 BPM, and a return to 150 BPM. This profile
  supersedes the earlier 140 to 180 BPM working range. Direct
  user requirement added in round 4.
- Per-tick BPM interpolation during ramps. Every ramp must
  update the tempo on each tick with an intermediate value
  rather than stepping from start to end. The user phrases this
  as the "NES strategy of updating registers every tick" and
  states explicitly that "ramp does not mean 90 then 150; there
  needs to be intermediate tempo values." Direct user
  requirement added after round 4.
- Both ramp-down and direct snap from 250 BPM to 150 BPM. The
  composition uses two distinct returns from peak tempo: one
  ramped (the Crash) and one instantaneous (a snap). The loop
  body therefore visits 250 BPM twice with different exits.
  Direct user requirement added after round 4. Concrete tick
  layout pending external structural expansion.
- Length flexibility. The composition may exceed typical
  16-bit-era length. The round 3 "60 to 90 second loop" target
  is relaxed. Direct user requirement added after round 4.
- All eight channels active somewhere in the composition. The
  spec must allocate channels 6 and 7 with concrete roles.
  Direct user requirement added after round 5.
- Every host feature exercised in active, inactive, and dynamic
  states across the composition. The three-state framework is
  the strict version of round 5's "on/off both used" idea.
  Active means a non-default value with audible effect.
  Inactive means the default or off value. Dynamic means
  modulated over time. Features without a clear off state
  (waveform, duty cycle, attack-decay-sustain-release) have
  active and dynamic states only. Direct user requirement
  added after round 5.
- Aggressive channel repurposing across sections. A single
  physical channel takes on multiple virtual-instrument roles
  during the composition through waveform, attack-decay-
  sustain-release, low-pass-filter, and retrigger changes at
  section boundaries. The user frames this as approximately
  thirty virtual channels surfaced through eight actual
  channels. The spec records the framing as a rough target
  rather than a hard count. Direct user requirement added
  after round 5.
- Up to all eight voices. The host caps at eight and the existing
  songs cap at five active to avoid clipping. Song 3 may use more
  if the spec accounts for the per-channel volume sum staying
  below the master-volume soft-clip ceiling. Pre-clip arithmetic
  example for budgeting volumes is in the host's audio callback
  in `examples/piano_roll.rs`.
- Loop integrity. After the intro plays once, the main body must
  loop seamlessly without state corruption. The script uses
  `state.loop_count` to count loops and `state.section` to track
  position within the main body.
- Explicit scale changes between sections. The composition is
  required to modulate across at least two distinct scales
  during the loop body. The script tracks the active scale per
  section so the harmonic motion reads as a key change rather
  than a chromatic chord substitution. Direct user requirement
  added after round 3, not surfaced through the external
  session.
- Massive octave range. The composition spans at least five
  octaves simultaneously by allocating different channels to
  different registers. The brief expects channels spread from
  sub-bass into the upper register.

## Section structure (round 4 proposal, supersedes round 3)

Round 4 expands the timeline to 960 first-iteration ticks: a
192-tick unlooped intro split into an accelerando phase and a
pregnant-pause phase, followed by a 768-tick main loop
containing the verse, a hyper-accelerando ramp, a brief peak at
250 BPM, and a crash back to 150 BPM closing the loop.

First-iteration tick spans below. The runtime mapping is:

- If `input < 192`, play intro material.
- Otherwise, compute `loop_tick = (input - 192) mod 768` and
  dispatch on `loop_tick` for the four loop phases with
  boundaries at loop-tick 0, 384, 512, and 576.

| Section | Ticks (first iteration) | Tempo | Scale or mode | Harmonic motion | Notes |
|---------|-------------------------|-------|---------------|------------------|-------|
| Intro Accelerando | 0..128 | 90 BPM ramping linearly to 150 BPM | D natural minor with harmonic-minor pivot on V | Pedal D in bass; upper voices ascend bar by bar | 128-tick tempo ramp. Eight bars at sixteen ticks per bar. |
| Intro Pregnant Pause | 128..192 | 150 BPM (locked) | D natural minor | Driving instruments cut; solo high-register arpeggio cascades down through four octaves | Four bars. Channel 2 carries the cascade; channels 0 and 4 are silenced. |
| Loop Verse | 192..576 | 150 BPM | D natural minor with harmonic-minor pivot on V | Galloping octave bass ostinato; contrapuntal lead-plus-arpeggiator over the i — VI — VII — V skeleton | Twenty-four bars. Loop entry point on subsequent iterations. |
| Loop Limit Break Ramp | 576..704 | 150 BPM ramping linearly to 250 BPM | Modulation arc TBD; candidate is moving toward F major or chromatic ascent | Filter sweeps upward across all voices; per-tick BPM increment | Eight bars. Hyper-accelerando. |
| Loop Chaos | 704..768 | 250 BPM (locked) | F major (carrying forward round 3's "glimmer of hope" placement) or aggressive modal alternative; TBD | Trills, neoclassical shredding; velocity boost into `tanhf` saturation | Four bars at peak tempo. Approximately three point eight seconds wall-clock. |
| Loop Crash | 768..960 | 250 BPM ramping linearly to 150 BPM | Cycle-of-fifths arc returning to D natural minor: Dm → Gm → C → F → B-flat → E diminished → A → D minor | High channels fall silent; low pulse-width-modulation drone via dynamic `host::set_duty` carries the descent | Twelve bars (192 ticks). Closes the loop seamlessly back to the Verse at loop-tick 0. |

Note on the round 4 segment arithmetic. The external session's
ASCII timeline labels the Crash segment as "128 Ticks" while
also placing its endpoints at first-iteration tick 768 and 960,
which is 192 ticks. The total loop must be 768 ticks
(192 + 768 = 960) for the math to close, so the Crash span is
192 ticks. Either the segment label "128" is a typo for "192"
or the "8 bars" prose count is a typo for "12 bars". The spec
records 192 ticks per the endpoint arithmetic. The external
session should confirm or revise.

Note on tempo trajectory. The 90 → 150 → 250 → 150 BPM profile
is a direct user requirement from round 4 and supersedes the
earlier 140 to 180 BPM working range. The 250 BPM peak is
spiritually consistent with final-boss desperation themes and
exercises the host's clock at the tight end of practical
operation (approximately sixty milliseconds per tick).

Wall-clock timing estimates.

- Intro Accelerando (90 → 150 BPM, average around 120 BPM, 128 ticks): approximately sixteen seconds.
- Intro Pregnant Pause (150 BPM, 64 ticks): approximately six point four seconds.
- Loop Verse (150 BPM, 384 ticks): approximately thirty-eight point four seconds.
- Loop Limit Break Ramp (150 → 250 BPM, average around 200 BPM, 128 ticks): approximately nine point six seconds.
- Loop Chaos (250 BPM, 64 ticks): approximately three point eight seconds.
- Loop Crash (250 → 150 BPM, average around 200 BPM, 192 ticks): approximately fourteen point four seconds.
- First iteration total: approximately eighty-eight seconds.
- Each subsequent loop iteration: approximately sixty-six seconds.

Structural expansion (round 7 final, master-session accepted).
The composition is now structured as a 128-tick unlooped intro
followed by a 1034-tick loop body containing nine sections
including two passes through the 250 BPM peak. The first Chaos
visit exits via a one-bar whole-tone gesture into a snap from
250 BPM to 150 BPM. The second Chaos visit exits via the
ramp-down Crash. The Intro Pregnant Pause from the prior spec
is collapsed into the Intro Accelerando (round 7 alignment);
the Breakdown is merged into the start of Verse B (round 7
alignment).

Final section table (round 8, master-session corrected).

The Breakdown is restored as a distinct 8-bar section between
the Pre-Snap and Verse B. Round 8's bar numbering (which starts
counting at Verse A bar 1) clarifies that bars 29-36 are the
Breakdown and bars 37-52 are Verse B proper, totaling 24 bars
between the snap-down and the second Limit Break Ramp.

| # | Section | Ticks (first iteration) | Bar range (round 8 numbering) | Bars | Tempo | Scale | Notes |
|---|---------|-------------------------|-------------------------------|------|-------|-------|-------|
| 1 | Intro | 0..128 | (pre-bar 1) | 8 (4/4) | 90 BPM ramping to 150 BPM | D natural minor | One-shot opener. Per-tick BPM updates. Ascending minor-scale lead. |
| 2 | Verse A | 128..384 | 1-16 | 16 (4/4) | 150 BPM | D natural minor with harmonic-minor pivot on V | Galloping octave bass ostinato. Stereo unison detune pair active on channels 6 and 7. |
| 3 | Limit Break Ramp A | 384..510 | 17-24 | 1 (7/8) + 7 (4/4) | 150 BPM ramping to 250 BPM | Begins in D minor, pivots to F major at bar 3 | Per-tick BPM updates. Section is 126 ticks due to the 7/8 pivot at bar 1. Filter sweep on channels 0 and 2. |
| 4 | Chaos Phase A | 510..574 | 25-28 | 4 (4/4) | 250 BPM | D Phrygian dominant | Two-chord D5/E-flat-5 thrashing. Arpeggiator in triplet polyrhythm (0.66 ticks per step). Channel 1 staccato laser trill. |
| 5 | Pre-Snap Whole-Tone | 574..586 | (uncounted) | 1 (3/4) | 250 BPM at entry; one-tick `host::set_bpm(150)` snap at tick 586 | D whole-tone | 12-tick descending whole-tone cascade. Dive-bomb gesture activates at the snap moment. |
| 6 | Breakdown | 586..714 | 29-36 | 8 (4/4) | 150 BPM | D minor (dominant-pedal modulation toward F major across the section) | Bars 29-32: ominous bass pads on D pedal; channels 1, 5, 6, 7 silenced; channel 2 low-passed arpeggio at 400 Hz. Bars 33-36: channel 5 re-enters with hi-hat; channel 1 wakes with distorted Square plus detune lead playing the melodic-entry phrase. Channel 1 dive-bomb recovery (linear ramp from -600 cents to 0) covers the first 16 ticks of this section. |
| 7 | Verse B | 714..970 | 37-52 | 16 (4/4) | F major (relative major modulation) | Heroic second-wind character. Displaced bassline pattern (inverted octave gallop). Parallel-interval harmonization on channels 6 and 7 (scale-aware diatonic thirds and sixths). |
| 8 | Limit Break Ramp B | 970..1098 | 53-60 | 8 (4/4) | 150 BPM ramping to 250 BPM | D harmonic minor | Per-tick BPM updates. Mirrors Ramp A's filter-and-vibrato escalation. |
| 9 | Chaos Phase B | 1098..1162 | 61-64 | 4 (4/4) | 250 BPM | D Phrygian dominant return | Mirrors Chaos A; channel 1 plays a descending sweep instead of trills. |
| 10 | Crash-Down and Taper | 1162..1290 | 65-72 | 8 (4/4) | 250 BPM ramping to 150 BPM | Cycle-of-fifths descent: A → D → G → C → F → B-flat → E-diminished → A | Master volume tapers approximately 85 percent per bar. Instruments drop out one by one. |

Loop boundary mapping. The runtime computes
`loop_tick = (input - 128) mod 1162` for `input >= 128`. At
`input == 1290` the formula gives `loop_tick = 0`, returning to
Verse A. The Intro plays once on first iteration; subsequent
iterations enter directly at Verse A.

Wall-clock estimates.

- Intro: approximately sixteen seconds.
- Verse A: approximately twenty-five point six seconds.
- Limit Break Ramp A: approximately nine point five seconds.
- Chaos Phase A: approximately three point eight seconds.
- Pre-Snap Whole-Tone: approximately point seven seconds.
- Breakdown: approximately twelve point eight seconds.
- Verse B: approximately twenty-five point six seconds.
- Limit Break Ramp B: approximately nine point six seconds.
- Chaos Phase B: approximately three point eight seconds.
- Crash-Down and Taper: approximately nine point six seconds.
- First iteration total: approximately one hundred seventeen seconds.
- Each subsequent loop iteration: approximately one hundred one seconds.

The composition's "longer than typical 16-bit-era" target is met.

## Channel assignments (round 5 proposal, supersedes round 3)

Round 5 carries the round 3 register allocation forward and
adds channel 5 for percussion. Total active channels: six.

| Channel | Register | Role | Initial waveform | Dynamic profile | Notes |
|---------|----------|------|------------------|-----------------|-------|
| 0 | Sub-bass, C1..A2 (MIDI 24..45) | Bass Machine: driving gallop ostinato | Sawtooth (code 2) | At Limit Break Ramp onset switches to Sine (code 3) with LPF cutoff dropped to 120 Hz, transitioning from "biting metal groove" to "heavy sub-bass drone". Retrigger off. ADSR initially (5, 40, 800, 100). Velocity modulated on downbeats. Pitch-bend rise via `host::set_detune(0, ramp)` during the last four ticks of each Verse leads into the Limit Break Ramp. | Octave-jump 16th-note pattern between root and octave. Detune-doubling target with channel 4 (see Extended Dynamics Catalog). |
| 1 | Melody core, C5..A5 (MIDI 72..81) | Neoclassical Lead Voice | Sawtooth (code 2) | At Chaos onset (first-iteration tick 704) switches to Square (code 0), retrigger flips from off to on, ADSR sustain drops to 0 and decay to 40 ms. Lead morphs from legato violin-style line to staccato laser trill. | Demonstrates retrigger in both on and off states. Demonstrates dynamic ADSR. |
| 2 | High lead, C6..A6 (MIDI 84..93) | Interlocking Arpeggiator | Pulse (code 4) | Retrigger permanently on. Continuous per-tick PWM via `host::set_duty(2, dynamic_pulse)` driven by a sine offset. Filter-envelope simulation: on each new chord-hit, `host::set_lpf(2, 3500)` then per-tick decrement of cutoff toward 600 Hz until the next chord. | Heavy parametric modulation. Two per-tick continuous modulations active in parallel. |
| 3 | Mid harmony, C4..A4 (MIDI 60..69) | Counterpoint / harmony chord roots | Triangle (code 1) | At Crash onset (first-iteration tick 768) switches to Pulse (code 4) with continuous detune oscillation between -25 and +25 cents via per-tick `host::set_detune(3, cent_offset)`. | Triangle warm-pad to detuned-pulse contrast across the loop. Round 5 calls the detune oscillation a "chorus" effect; technically a single-channel detune oscillation is a tremolo-via-detune, not a chorus. |
| 4 | Low harmony, C3..A3 (MIDI 48..57) | Power-chord roots in Verse A; detuned-doubling partner of channel 0 across Verse A and Limit Break Ramp A; silenced during Pregnant Pause and Crash | Filtered Pulse (code 4) with LPF in primary role; mirrors channel 0's waveform during detuned-doubling passages | Plays the bassline an octave above channel 0 with +5 cents detune for the detuned-doubling effect. Centred mix (`host::set_volume(4, 500, 500)`). Retrigger toggles state with section: on for Verse galloping ostinato, off for Limit Break Ramp drone. | Dual-role: power-chord roots when channel 0 is on the galloping bass, detuned-doubling partner reinforcing channel 0 when channel 0 morphs to sub-bass sine. See Extended Dynamics Catalog for the detuned-doubling placement. |
| 5 | Percussion | Percussion Engine: hi-hat, snare, kick | Noise (code 5) | Dynamic ADSR per tick. Hi-hat uses (1, 20, 0, 10). Snare uses (2, 90, 0, 40) with `host::set_velocity(5, 950)`. Kick variant (extrapolated): (1, 60, 0, 30) at lower midi for thump. | Single channel simulates three percussion roles through per-tick ADSR swapping. Demonstrates dynamic attack-decay-sustain-release as a parametric control. |
| 6 | Dual-mode left twin: stereo unison in Verse A, parallel-third harmonizer in Verse B | Mirrors channel 1's waveform in both modes | In Verse A: same melody as channel 1, `host::set_detune(6, +7)` for stereo unison, hard-left pan via `host::set_volume(6, 1000, 0)`. In Verse B: melody at parallel third above channel 1 (scale-aware, three or four semitones depending on F-major scale degree), hard-left pan unchanged, detune reset to zero. Vibrato on at 6 Hz / 25 cents during sustained Verse A notes (slight rate offset from channel 1's 5 Hz produces ensemble shimmer). | Demonstrates two distinct stereo techniques in active state. See Extended Dynamics Catalog for the technique justifications. |
| 7 | Dual-mode right twin: stereo unison in Verse A, parallel-sixth harmonizer in Verse B | Mirrors channel 1's waveform in both modes | In Verse A: same melody as channel 1, `host::set_detune(7, -7)` for stereo unison, hard-right pan via `host::set_volume(7, 0, 1000)`. In Verse B: melody at parallel sixth above channel 1 (scale-aware, eight or nine semitones depending on F-major scale degree), hard-right pan unchanged, detune reset to zero. Vibrato on at 4 Hz / 25 cents during sustained Verse A notes. | Completes the dual-mode pair. The +7 / -7 cent detune unison covers Verse A; the parallel-third / parallel-sixth harmonization covers Verse B. |

Technique note on the stereo unison pair. The user described
"dedicated right and left speaker channels that play slightly
different notes that sound good together". The matching
technique under 16-bit-era sound design is stereo unison with
detune. Two voices carry the same pitch as the lead with small
opposing detune offsets and are hard-panned to opposite
speakers. The pair widens the stereo image and adds chorused
thickness without any host-side reverberation or chorus
processing.

Alternate interpretation. If the user intended parallel-interval
harmonization rather than detuned doubling, channel 6 would
carry the lead a minor or major third above (an extra plus
three or plus four semitones) and channel 7 would carry the
lead a perfect fifth or major sixth above (plus seven or plus
nine semitones), each hard-panned. This is a different effect.
The detune approach widens a single voice; the harmonization
approach builds chords from parallel motion. The user can pick
which to spec; the table above commits to the detuned-unison
reading because it matches "slightly different notes" most
literally.

Active channel count after extrapolation. Channels 0 through 7
inclusive. The "use all eight channels" constraint is met.

The total register span from channel 0 low (MIDI 24, C1, about
32.7 Hz) to channel 2 high (MIDI 93, A6, about 1760 Hz) covers
approximately five octaves and ten semitones. The "massive
octave range" constraint is satisfied.

Round 5 channel-count framing. The external session again
writes "6 available host channels" while describing five
channels by name (0, 1, 2, 3, 5). The host exposes eight
channels. The spec preserves round 4's channel 4 role since
round 5 does not contradict it. Total active channels under the
combined round 4 plus round 5 reading: six (0, 1, 2, 3, 4, 5).

## Mid-song event schedule (round 4 proposal, supersedes round 3)

First-iteration ticks below.

| Tick | Native called | Effect |
|------|---------------|--------|
| Init block | `host::song_name("Keleusma Project: Cyberforge Suite (0BSD)")` | Song-name announcement. The title combines the project prefix, the working artistic title, and a license tag reflecting the repo's 0BSD licensing. |
| Init block | `host::set_bpm(90)` | Establish opening tempo at 90 BPM. |
| Init block | Per-channel `host::set_waveform`, `host::set_adsr`, `host::set_volume`, `host::set_velocity`, `host::set_retrigger`, `host::set_enable` for channels 0..4 | Channel 0 sawtooth bass, channel 1 sawtooth legato lead, channel 2 pulse arpeggiator with retrigger on, channel 3 mid-harmony, channel 4 filtered-pulse power roots. |
| Each tick during 0..128 (Intro Accelerando) | `host::set_bpm(90 + t * 60 / 128)` | Linear BPM ramp from 90 to 150 across the Intro Accelerando. Integer division terminates at 150 BPM. |
| At tick 128 (Pregnant Pause onset) | `host::set_enable(0, 0)`, `host::set_enable(4, 0)` and similar | Driving instruments cut. Channel 2 takes over with the descending arpeggio cascade. |
| At tick 192 (Verse onset) | `host::set_enable(0, 1)`, `host::set_enable(4, 1)` | Driving instruments re-engage at the loop entry. |
| On bass downbeats during the Verse (first-iteration ticks 192..576) | `host::set_velocity(0, accent_q1000)` | Velocity modulation on bassline downbeats for the pulsing pump (round 2 origin, carried forward). |
| On V-chord ticks of the Verse | `host::set_lpf(2, open_hz)` and later `host::set_lpf(2, closed_hz)` | Arpeggiator low-pass-filter opens wide on the V chord, closes elsewhere. |
| Each tick during 576..704 (Limit Break Ramp) | `host::set_bpm(150 + (t - 576) * 100 / 128)` | Linear BPM ramp from 150 to 250 across the Limit Break Ramp. |
| Each tick during 576..704 | `host::set_lpf(<ch>, sweep_hz)` for one or more channels | Filter sweep upward in parallel with the tempo ramp; the proposal frames this as "the entire mix sounds brighter, harsher". |
| Each tick during 704..768 (Chaos) | `host::set_velocity(<ch>, boosted_q1000)` for active channels | Maximum overdrive into the `tanhf` soft-clipper. The "saturation moment" from rounds 1 through 3 lives here. |
| At tick 768 (Crash onset) | `host::set_enable(2, 0)` and similar | High channels fall silent. |
| Each tick during 768..960 (Crash) | `host::set_bpm(250 - (t - 768) * 100 / 192)` | Linear BPM ramp from 250 to 150 across the Crash. Integer division terminates at 150 BPM. |
| Each tick during 768..960 | `host::set_duty(<ch>, modulated_q1000)` on a low channel | Pulse-width-modulation drone gives the descent its menacing character. |
| At tick 960 (loop boundary) | Re-enable channels silenced during the Crash | Restore the Verse-time channel configuration. The subsequent loop iteration enters at loop-tick 0 (first-iteration equivalent tick 192). |

Round 5 parametric additions (overlay on top of the round 4
schedule above).

| Tick span | Native called | Effect |
|-----------|---------------|--------|
| Init block | Channel 5 setup: `host::set_waveform(5, 5)`, `host::set_volume`, `host::set_retrigger`, `host::set_enable` | Percussion engine on the Noise waveform. |
| Every Verse tick on a hi-hat schedule | `host::set_adsr(5, 1, 20, 0, 10)` then `host::play(5, midi)` | Tiny metallic click for hi-hat. |
| Every Verse tick on a snare schedule | `host::set_adsr(5, 2, 90, 0, 40)`, `host::set_velocity(5, 950)`, then `host::play(5, midi)` | Heavy blast for snare. |
| Every tick during 0..959 | `host::set_duty(2, dynamic_pulse)` | Continuous PWM sweep on channel 2 arpeggiator driven by a sine offset. Round 5 frames this as the arpeggiator "sounding alive". |
| On each new chord-hit during the Verse | `host::set_lpf(2, 3500)` | Filter-envelope simulation: open filter at chord onset. |
| Per tick between chord-hits during the Verse | `host::set_lpf(2, decreasing_hz)` | Manual filter-envelope decay from 3500 Hz toward 600 Hz. |
| At Limit Break Ramp onset (first-iteration tick 576) | `host::set_waveform(0, 3)`, `host::set_lpf(0, 120)` | Channel 0 transitions from biting sawtooth to sub-bass sine drone. |
| At Chaos onset (first-iteration tick 704) | `host::set_waveform(1, 0)`, `host::set_retrigger(1, 1)`, `host::set_adsr(1, attack, 40, 0, release)` | Channel 1 morphs from legato lead to staccato laser trill. Retrigger flips off-to-on. ADSR sustain collapses to zero. |
| Every tick during 768..959 (Crash) | `host::set_waveform(3, 4)`, `host::set_detune(3, oscillating_cents)` | Channel 3 switches from Triangle to Pulse with continuous detune oscillation between -25 and +25 cents. |

The following natives remain absent from the round 5 proposal.
Pending external contribution. The implementation-expert
extrapolation below fills these gaps.

- `host::set_vibrato(ch, rate_centihz, depth_cents)`. Untouched
  across all five rounds.
- `host::set_volume(ch, l_q1000, r_q1000)`. The six-channel
  octave allocation now invites distinct stereo positions per
  register. Untouched across all five rounds.
- `host::set_master_volume(q1000)`. Untouched across all five
  rounds.

## Extrapolated event schedule (implementation-expert pass)

The user asked the implementation expert to extrapolate the
spirit of the request. The extrapolation below fills the three
gaps left after round 5 by placing each unaddressed native in
active, inactive, and dynamic states across the composition.
The schedule does not invalidate the round 5 parametric
overlay; it sits alongside.

| Tick span | Native called | Effect | State framing |
|-----------|---------------|--------|---------------|
| Init block | `host::set_master_volume(700)` | Open the composition at approximately seventy percent of unity. Leaves headroom for the climax. | Master volume: active (non-default). |
| Init block | `host::set_volume(0, 500, 500)`, `host::set_volume(1, 500, 500)`, `host::set_volume(2, 400, 600)`, `host::set_volume(3, 600, 400)`, `host::set_volume(4, 500, 500)`, `host::set_volume(5, 500, 500)`, `host::set_volume(6, 1000, 0)`, `host::set_volume(7, 0, 1000)` | Per-channel stereo image: bass centred, lead centred, arpeggiator slightly right, mid harmony slightly left, power roots centred, percussion centred, stereo unison twins hard-panned. | Per-channel stereo volume: active. |
| Init block | `host::set_vibrato(1, 0, 0)` and similar zero calls on other channels | Vibrato inactive at song start. | Vibrato: inactive (default). |
| Each tick during 0..128 (Intro Accelerando) | `host::set_master_volume(700 + t * 300 / 128)` | Linear master-volume ramp from 700 to 1000 across the Intro Accelerando. | Master volume: dynamic. |
| Each tick during 128..192 (Intro Pregnant Pause) | `host::set_volume(2, sweep_left, sweep_right)` | Pan the descending arpeggio cascade across the stereo field, from hard left at the top of the cascade to hard right at the bottom (or the reverse). | Per-channel stereo volume: dynamic. |
| At tick 192 (Verse onset) | `host::set_vibrato(1, 500, 30)` | Five-Hertz vibrato at thirty cents depth on the lead. Subtle expressive movement. | Vibrato: active. |
| At tick 192 (Verse onset) | `host::set_enable(6, 1)`, `host::set_enable(7, 1)` | Stereo unison twins engage with the Verse. | Channel enable: dynamic. |
| Each tick during 576..704 (Limit Break Ramp) | `host::set_vibrato(1, 500 + (t - 576) * 700 / 128, 30 + (t - 576) * 70 / 128)` | Vibrato depth ramps from 30 cents to 100 cents and rate ramps from 5 Hz to 12 Hz in parallel with the tempo ramp. | Vibrato: dynamic. |
| At tick 704 (Chaos onset) | `host::set_vibrato(1, 0, 0)` | Vibrato off during the staccato laser trill. Vibrato is incompatible with the short envelopes. | Vibrato: inactive again (state transition demonstrated). |
| At tick 704 (Chaos onset) | `host::set_enable(6, 0)`, `host::set_enable(7, 0)` | Stereo unison twins silence during Chaos. The laser trill should read as thin and aggressive, not chorused. | Channel enable: dynamic. |
| Each tick during 768..960 (Crash) | `host::set_master_volume(1000 - (t - 768) * 300 / 192)` | Linear master-volume taper from unity back to seventy percent across the Crash. Creates a "running out of energy" decay. | Master volume: dynamic. |
| At loop boundary (tick 960) | `host::set_master_volume(700)` | Snap back to the opening master volume for the next loop iteration. The subsequent loop iteration restarts the ramp at the Verse rather than the Intro. | Master volume: confirms loop seamless restart. |

Coverage matrix after extrapolation.

| Native | Active state | Inactive state | Dynamic state |
|--------|--------------|----------------|---------------|
| `host::set_enable` | Channels 0..7 enabled during the Verse | Channels 6, 7 disabled during Chaos; channels 0, 4 disabled during Pregnant Pause | Per-section enable toggles |
| `host::set_waveform` | All channels carry non-default waveforms | Not applicable (no off state) | Channels 0, 1, 3 morph at section boundaries |
| `host::set_duty` | Pulse channels carry non-default duty | Not applicable (no off state) | Continuous PWM sweep on channel 2 |
| `host::set_adsr` | All channels carry non-default ADSR | Not applicable (no off state) | Channel 5 swaps profiles per tick; channel 1 collapses sustain at Chaos |
| `host::set_volume` | Per-channel panning during the Verse | Equal L/R at startup defaults | Pregnant Pause cascade pan automation |
| `host::set_vibrato` | Verse vibrato on channel 1 at 5 Hz / 30 cents | Vibrato zero during Intro, Pregnant Pause, Chaos, Crash | Limit Break Ramp parallel depth and rate ramp |
| `host::set_lpf` | Channel 0 sub-bass mode at 120 Hz; channel 2 filter envelope opens at 3500 Hz | Channel 1 LPF zero (bypassed) for most of the composition | Channel 2 per-tick filter envelope decay; channel-wide sweep during Limit Break Ramp |
| `host::set_retrigger` | Channel 2 retrigger on; channel 1 retrigger on during Chaos | Channel 1 retrigger off during Verse for legato | Channel 1 off-to-on flip at Chaos onset |
| `host::set_detune` | Channels 6 and 7 carry +7 and -7 cents for stereo unison; channel 3 oscillates during Crash | Most channels at zero detune outside their active passages | Channel 3 continuous oscillation between -25 and +25 cents during Crash |
| `host::set_velocity` | Per-channel base velocity during init | Not applicable (1000 is unity, not "off") | Channel 0 downbeat accents; channel 5 hi-hat/snare velocity swap; Chaos overdrive boost |
| `host::set_master_volume` | 700 at opening; 1000 at Verse | 1000 is unity default | Intro Accelerando ramp 700 to 1000; Crash taper 1000 to 700 |
| `host::set_bpm` | 150 BPM at Verse | 250 BPM at Chaos peak | Per-tick interpolation across Intro Accelerando, Limit Break Ramp, Crash; one-tick snap at the snap-down exit (pending structural expansion) |
| `host::song_name` | Called once in init with the song title | Not applicable (no off state) | Not applicable (one-shot convention) |

## Data-segment usage

Schema is fixed at twenty-three slots. Song-3-specific intent
for each slot:

| Slot | Field | Song-3 role |
|------|-------|-------------|
| 0 | `init` | One-shot setup gate (host convention) |
| 1 | `loop_count` | Distinguishes first iteration (plays intro) from later iterations |
| 2 | `section` | 0 = Intro Accelerando, 1 = Intro Pregnant Pause, 2 = Loop Verse, 3 = Loop Limit Break Ramp, 4 = Loop Chaos, 5 = Loop Crash (round 4 layout). |
| 3 | `user0` | Reserved. The host passes the absolute tick as `input` every tick, so a separate counter is not required. |
| 4 | `user1` | Reserved (candidate use: scale index for the active section, indexing into a scale table). |
| 5 | `user2` | Reserved (candidate use: chord-matrix index for the active section). |
| 6 | `user3` | Reserved (candidate use: arpeggio step-pattern position). |
| 7..14 | `idx[0..7]` | Per-channel note-position counters |
| 15..22 | `rem[0..7]` | Per-channel remaining-ticks counters |

The proposal sketches a custom `struct SongState` with named
fields `tick_counter` and `current_bpm`. The Keleusma data
segment is fixed at the twenty-three-slot schema. The proposal's
`tick_counter` is the host's `input` parameter, which the script
already receives every tick. Caching the current BPM in `user1`
is also redundant because section position determines the BPM
deterministically.

## Note tables (round 7 external session content, master-session accepted with overrides)

The external session committed concrete chord matrix, arpeggio
interval-pattern vectors, percussion mask, lead melody pitches
for several sections, and bassline pitches for the Intro. The
content below lands as the canonical reference with three
master-session overrides identified inline. The script-author
pass translates this into Keleusma source.

### Chord-type encoding (round 7 final)

Round 7 simplifies the encoding to chord-quality identifiers
indexing into arpeggio interval-pattern vectors. The chord root
travels in a separate "Roots" array per section. This is a
cleaner separation than round 6's combined identifier.

| Identifier | Chord quality | Arpeggio vector |
|------------|---------------|-----------------|
| 0 | Minor triad (natural minor / Aeolian) | `[0, 3, 7, 12, 15, 12, 7, 3]` |
| 1 | Minor triad (harmonic minor; major-7 handled by scale-root shift) | `[0, 3, 7, 12, 15, 12, 7, 3]` |
| 2 | Major triad | `[0, 4, 7, 12, 16, 12, 7, 4]` |
| 3 | Dominant 7th / Phrygian dominant (root, major-3, perfect-5, flat-7, flat-9) | `[0, 4, 7, 10, 13, 10, 7, 4]` |
| 4 | Diminished 7th / Whole-tone tension | `[0, 3, 6, 9, 12, 9, 6, 3]` |

The arpeggiator advances through the vector at sixteenth-note
resolution (one tick per step under the polyrhythm-off mode;
alternating five-tick and six-tick step intervals during
Chaos sections under polyrhythm-on mode).

### Chord sequence and roots per bar (round 7 final, master-session accepted)

One bar equals sixteen ticks under 4/4 except where the time
signature pivots (Limit Break Ramp A bar 1 at 14 ticks for 7/8;
Pre-Snap section at 12 ticks for 3/4). MIDI conventions:
D1 = 26, D2 = 38, D3 = 50, D4 = 62, D5 = 74.

Section 1 (Intro, 8 bars in D natural minor).

- Chord matrix: Dm, Dm, B-flat, C, Dm, Dm, B-flat, A.
- Roots: [38, 38, 34, 36, 38, 38, 34, 33].
- Arp types: [0, 0, 2, 2, 0, 0, 2, 2].
- Bassline: static low pedal point on D1 (26) for bars 1 through 6, stepping down to B-flat-0 (22) and A0 (21) on bars 7 and 8.

Section 2 (Verse A, 16 bars in D minor with harmonic-minor pivot on V).

- Bars 1 through 4: Dm, Dm, B-flat, A. Roots [38, 38, 34, 33]. Arp types [0, 0, 2, 1].
- Bars 5 through 8: Dm, Dm (harmonic pivot), B-flat, A7. Roots [38, 38, 34, 33]. Arp types [0, 1, 2, 3].
- Bars 9 through 12: Gm, C, F, B-flat. Roots [31, 36, 29, 34]. Arp types [0, 2, 2, 2]. Cycle of fourths in D minor.
- Bars 13 through 16: A7-flat-9, A, Dm, Dm. Roots [33, 33, 38, 38]. Arp types [4, 1, 0, 0]. Master-session override: round 7 labelled bar 13 "Em7-flat-5" but the root array gives MIDI 33 (A1). The chord at root A with the half-diminished arpeggio shape (vector index 4) is A7-flat-9, the V7-flat-9 of D minor harmonic, which is the harmonically functional sonority at this location.

Verse A to Limit Break Ramp A transition (deceptive cadence).

Bar 16 ends on A major (V of D minor). The expected resolution
is to D minor (i) on bar 17. Instead the track lands on F major
(III), a classic deceptive cadence. Channel 1 carries the
modulation across the bar line with an ascending scalar run:

- Bar 16 final four ticks: `[69, 71, 73, 74]` — A4, B4, C-sharp-5, D5.
- Bar 17 first tick: `77` — F5, landing as the chord shifts to F major beneath.

The C-sharp-5 in bar 16 is the raised seventh of D harmonic
minor reinforcing the dominant function before the deceptive
turn.

Section 3 (Limit Break Ramp A, 1 bar 7/8 plus 7 bars 4/4, pivoting D minor to F major at bar 3).

- Chord matrix: Dm (7/8), G, F, C, B-flat, C, A, A.
- Roots: [38, 31, 29, 36, 34, 36, 33, 33].
- Arp types: [0, 0, 2, 2, 2, 2, 1, 1].
- Total section ticks: 126 (14 + 7×16).

Section 4 (Chaos Phase A, 4 bars in D Phrygian dominant).

- Chord matrix: D5, E-flat-5, D5, E-flat-5. Relentless two-chord thrashing at the bar level.
- Roots (per-bar harmonic anchor): [38, 39, 38, 39].
- Arp types: [3, 3, 3, 3]. Triplet polyrhythm engaged.
- Bassline: relentless sixteenth-note pedal on D1 (26) alternating with E-flat-1 (27) on the E-flat bars. No octave jumps; pure speed.

Chaos Phase A intra-bar pitch ascent (round 8 detail).

Rather than four bars of static thrashing, the section steps
through an ascending register progression while remaining in
the Phrygian dominant tonality. Each bar shifts the lead trill
and arpeggio anchor upward:

- Bar 25: lead trills `D5 / E-flat-5` (74 / 75). Arpeggiator root on D (50).
- Bar 26: lead trills `G5 / A-flat-5` (79 / 80). Arpeggiator root on G (55).
- Bar 27: lead trills `B-flat-5 / B5` (82 / 83). Arpeggiator root on B-flat (58).
- Bar 28: lead screams `E-flat-6 / E6` (86 / 87). Arpeggiator root on A (57), forcing a diminished-dominant resolution into the Pre-Snap section.

The lead pitches `[74, 75, 74, 75, ..., 86, 87, 86, 87]` from
round 7 represent the lower-register interpretation; the
round 8 progression supersedes with the four-bar register
ascent for stronger structural drama.

Section 5 (Pre-Snap Whole-Tone, 1 bar 3/4 in D whole-tone).

- Chord matrix: D-diminished. Root [38]. Arp type [4].
- Total section ticks: 12.
- The snap-down `host::set_bpm(150)` fires at tick 12 of the section (the last tick before the next section begins). Channel 1 dive-bomb activates concurrently.

Section 6 (Breakdown, 8 bars in D minor pivoting to F major).

- Bars 29-32 ("Ominous Pads"): channels 1, 5, 6, 7 silenced; channel 0 holds long D pedal; channel 2 arpeggiator low-passed to 400 Hz murmurs the D minor triad. Bassline roots `[38, 38, 38, 38]`.
- Bars 33-36 ("Melodic Entry"): channel 5 hi-hat clicks re-enter; channel 1 wakes with distorted Square-plus-detune profile; bass continues D-centered transition. The first 16 ticks of this section carry the dive-bomb recovery on channel 1 (linear ramp from -600 cents to 0 cents).

Section 7 (Verse B, 16 bars in F major).

- Chord matrix: F, F, C, C, Dm, Dm, B-flat, C, F, F, Am, Am, B-flat, C, Dm, C.
- Roots: [29, 29, 36, 36, 38, 38, 34, 36, 29, 29, 33, 33, 34, 36, 38, 36].
- Arp types: per chord quality (2 major, 0 minor).
- Parallel-interval harmonization on channels 6 and 7 (scale-aware diatonic) tracks channel 1.

Section 8 (Limit Break Ramp B, 8 bars in D harmonic minor).

- Chord matrix: Dm, Gm, A, Dm, B-flat, Gm, A7, A7.
- Roots: [38, 31, 33, 38, 34, 31, 33, 33].
- Channel 0 LPF cutoff opens from 150 Hz to 4000 Hz across the section.

Section 9 (Chaos Phase B, 4 bars in D Phrygian dominant).

- Musical content mirrors Chaos Phase A.
- Channel 1 plays a descending sweep instead of the trills, setting up the Crash.

Section 10 (Crash-Down and Taper, 8 bars in cycle-of-fifths descent).

- Roots: [33, 38, 31, 36, 29, 34, 28, 33] — A, D, G, C, F, B-flat, E-diminished, A.
- Returns to D minor at the loop boundary.
- Master volume tapers approximately 85 percent per bar (15 percent drop per bar) until only channel 2's muted low-passed arpeggio remains.

### Lead melody pitches (channel 1)

Concrete melody content delivered by round 7.

Section 1 (Intro, 8 bars, one note per bar).

`[62, 65, 69, 70, 74, 77, 81, 80]` — D4, F4, A4, B-flat-4, D5, F5, A5, A-flat-5.

Note on the final A-flat (MIDI 80). The note is not in
D natural minor. Retained as written per round 7 because it
serves as a chromatic approach to A5 reading as the leading
tone of A natural minor. If diatonic correction is wanted,
change 80 to 81 in Section 1 bar 8.

Section 2 (Verse A, 16 bars, four notes per bar).

- Bars 1 through 4: `[62, 65, 64, 62, 69, 67, 65, 64, 65, 69, 67, 65, 64, 67, 65, 64]`. D4, F4, E4, D4, A4, G4, F4, E4, F4, A4, G4, F4, E4, G4, F4, E4.
- Bars 5 through 8: same motif as bars 1 through 4 but the final bar alters to hit the C-sharp leading tone: `[61, 64, 67, 69]`. C-sharp-4, E4, G4, A4.
- Bars 9 through 16: round 7 did not commit specific pitches. The script-author pass derives melody from the chord matrix using chord-tone selection and the Verse A motif rhythm.

Section 3 (Limit Break Ramp A, bars 5 through 8 only).

`[69, 71, 72, 74, 76, 77, 79, 81, 83, 85, 86, 88, 89, 91, 93, 95]` —
ascending sixteenth-note scale run from A4 through B5. Bars 1
through 4 not specified by round 7; script-author pass derives.

Section 4 (Chaos Phase A, 16 notes covering 4 bars at 4 notes per bar).

`[74, 75, 74, 75, 74, 75, 74, 75, 86, 87, 86, 87, 86, 87, 86, 87]` —
frantic chromatic trills alternating D5 and E-flat-5 for the
first two bars, then jumping up an octave to D6 and E-flat-6
trills for the second two bars.

Section 5 (Pre-Snap Whole-Tone, 12 notes covering 1 bar).

`[86, 84, 82, 80, 78, 76, 74, 72, 70, 68, 66, 64]` — descending
whole-tone cascade from D6 down through E4.

Section 6 Breakdown lead melody (round 8).

- Bars 29-32 (Ominous Pads): channel 1 silent (the bassline and the low-passed arpeggio carry the section).
- Bars 33-36 (Melodic Entry, 4 notes per bar): `[65, 69, 72, 77, 76, 74, 72, 69, 70, 74, 77, 81, 79, 77, 76, 72]` — F4, A4, C5, F5, E5, D5, C5, A4, B-flat-4, D5, F5, A5, G5, F5, E5, C5. Channel 1 plays this on the distorted Square plus detune profile that round 8 specifies as "permanently" engaged for the remainder of the loop iteration.

Section 7 Verse B lead melody (round 8, 8 notes per bar eighth-note rhythm except where noted).

- Bars 37-38 (F Major Core): `[77, 76, 74, 77, 81, 84, 82, 81, 79, 81, 82, 79, 76, 74, 72, 74]`.
- Bars 39-40 (C Major Core): `[76, 74, 72, 76, 79, 82, 81, 79, 77, 79, 81, 77, 74, 72, 69, 72]`.
- Bars 41-42 (Dm to B-flat): `[74, 77, 81, 79, 77, 81, 86, 84, 82, 81, 79, 77, 76, 74, 76, 77]`.
- Bars 43-44 (Am Core): `[76, 79, 82, 81, 79, 82, 88, 86, 84, 83, 81, 79, 77, 76, 77, 79]`.
- Bars 45-48 (Cycle Descent, 4 notes per bar): `[81, 77, 74, 77, 79, 76, 72, 76, 77, 74, 70, 74, 76, 72, 69, 72]`.
- Bars 49-52 (Cadence to Ramp, 4 notes per bar ascending run): `[74, 76, 77, 79, 81, 82, 84, 86, 88, 89, 91, 93, 95, 96, 98, 100]` — sweeping ascent from D5 to E7 (note: MIDI 100 is E7) preparing the second Limit Break Ramp.

Sections 8 through 10: lead melody pitches not committed by
the external session. Script-author pass derives from the chord
matrix and the per-section character notes. Chaos Phase B uses
a descending sweep on channel 1 (round 7 specification) which
the implementation pass can derive as a Phrygian-dominant
descending scale run analogous to the bars 49-52 ascent in
reverse.

### Bassline root matrix (round 8 final)

Channel 0 carries the bassline root per bar. Channel 4 tracks
one octave below channel 0 with a static -10 cent detune offset
(round 8 detune-doubling specification supersedes the prior +5
cent value). Rhythmic interpretation per section: galloping
octave ostinato in Verse A and both Limit Break Ramps;
displaced syncopated off-beat pattern in Verse B; static pedal
in Breakdown bars 29-32; relentless sixteenth-note pedal in
Chaos sections.

- Verse A (bars 1-16): `[38, 38, 34, 33, 38, 38, 34, 33, 31, 36, 29, 34, 33, 33, 38, 38]`.
- Limit Break Ramp A (bars 17-24): `[38, 31, 29, 36, 34, 36, 33, 33]`. Bar 17 is the 7/8 bar (14 ticks instead of 16).
- Chaos Phase A (bars 25-28): pedal pattern alternating D1 (26) and E-flat-1 (27) per the per-bar chord-root array `[38, 39, 38, 39]` (sub-octave duplication).
- Pre-Snap Whole-Tone (1 bar): bassline contour follows the descending whole-tone cascade on channel 1 at a sub-octave (round-8 derived; no explicit pitches given).
- Breakdown bars 29-32 (Ominous Pads): held long notes `[38, 38, 38, 38]`. Channel 4 sub-bass continues the detune-doubling at octave-down with -10 cents.
- Breakdown bars 33-36 (transition into Verse B): bass continues D-centered with subtle preparation for the F major modulation. Round 8 does not commit specific pitches for these four bars; the implementation pass holds D pedal or steps toward F.
- Verse B (bars 37-52): off-beat groove pattern `[29, 29, 36, 36, 38, 38, 34, 36, 29, 29, 33, 33, 34, 36, 38, 36]`. The rhythmic pattern hits a rest on tick 0 and throws the heavy low-octave note onto off-beats for syncopated character.
- Limit Break Ramp B (bars 53-60): `[38, 31, 33, 38, 34, 31, 33, 33]`.
- Chaos Phase B (bars 61-64): pedal pattern mirroring Chaos Phase A.
- Crash-Down (bars 65-72): cycle-of-fifths descent `[33, 38, 31, 36, 29, 34, 28, 33]`.

### Mid-harmony block layout (round 8, channel 3)

Channel 3 carries sustained harmony guide-tones focusing on
thirds and sevenths. Two-note intervals (notation `lower/upper`)
are interpreted as sub-bar alternation rather than simultaneous
polyphony, since channel 3 is monophonic. The script alternates
between the two pitches at an eight-tick sub-bar rate by
default. Waveform is Triangle (code 1) by default, switching to
Pulse (code 4) with dynamic vibrato during Verse B.

- Intro and Verse A (24 bars, 12 intervals at 2 bars per interval): `[57/60, 57/60, 58/62, 57/61, 57/60, 57/60, 58/62, 55/59, 55/58, 55/59, 53/57, 53/57]`. Lower-voice pitches sit around A3 to G3 (MIDI 55..58), upper-voice pitches around C4 to E4 (MIDI 59..62). Voice-led across the chord changes.
- Limit Break Ramp A (8 bars, 8 intervals on the beat): `[50/57, 55/62, 53/60, 55/62, 50/57, 55/62, 52/59, 52/59]`. Open-fifth stabs reinforcing the accelerando.
- Chaos Phase A and Chaos Phase B (4 bars each, single-note staccato chops on off-beats): `[50, 50, 51, 51, 50, 50, 51, 51]`. D2 and E-flat-2 pattern mirroring the bassline thrashing one octave above.
- Pre-Snap Whole-Tone: channel 3 silent or follows the channel 1 cascade an octave below.
- Breakdown bars 29-32: low ominous drone on A2 (MIDI 45). The A pedal functions as V of D minor preparing the modulation to F major (where A is the major third of the tonic).
- Verse B (16 bars, 8 intervals at 2 bars per interval, lush slow-swirling pad with dynamic vibrato): `[53/57, 53/57, 55/59, 55/59, 57/60, 57/60, 53/57, 55/59]`. Tracks the F major harmonic motion.
- Limit Break Ramp B: contents not committed; script-author pass mirrors Limit Break Ramp A pattern.
- Crash-Down: contents not committed; fades out in parallel with the master taper.

### Arpeggio step rate (round 8 final)

The arpeggiator advances at one tick per step in the default
mode. The eight-element vectors complete two full cycles per
sixteen-tick 4/4 bar.

Polyrhythm mode (active during Chaos Phase A and Chaos Phase
B). The arpeggiator pointer advances every 0.66 ticks (two
thirds of a tick), producing three steps per two sixteenth-note
ticks. This is the 3-against-4 cross-rhythm from the Extended
Dynamics Catalog. The integer-tick implementation alternates
between two-tick and one-tick step intervals to approximate the
0.66 step rate, completing three steps every two ticks on
average.

### Loop-iteration variation schedule (round 8 final)

The script reads `state.loop_count` and varies behaviour on
subsequent iterations.

- `loop_count == 0` (first pass): standard full-density playback per the specification above.
- `loop_count == 1` (texture thinning): the first four bars of Verse A drop channel 4 (sub-bass stack) entirely, leaving channel 0 alone to carry the low end. Bars 5 through 16 restore channel 4. The hollow, exposed character marks the second iteration.
- `loop_count == 2` (overdrive pass): channel 5 percussion velocity is scaled upward by five percent across the board. The master mix drives harder into the `tanhf` soft-clipping for a grittier, more compressed flavor.
- `loop_count >= 3`: the script resets `state.loop_count` to zero. This guarantees deterministic, long-term stability across arbitrary listening durations.

### Percussion mask and ADSR profiles (round 7 final)

Channel 5 carries the percussion engine on the Noise waveform.
The script reads the 16-tick bitmask per bar and swaps ADSR
plus velocity registers on each fire to produce the requested
sound.

Standard 16-tick mask (Verse A and Verse B).

| Tick | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 |
|------|---|---|---|---|---|---|---|---|---|---|----|----|----|----|----|----|
| Sound | K | . | H | . | S | . | H | . | K | H | S  | .  | H  | K  | S  | H  |

Three rhythm-state ADSR profiles.

| State | Native sequence | Character |
|-------|-----------------|-----------|
| K (kick) | `host::set_adsr(5, 2, 45, 0, 10)`, `host::set_velocity(5, 900)`, low-pass filter cutoff at approximately 400 Hz | Heavy thump. |
| S (snare) | `host::set_adsr(5, 1, 110, 0, 30)`, `host::set_velocity(5, 980)`, low-pass filter open (cutoff zero or above Nyquist) | Loud white-noise backbeat crack. |
| H (hi-hat) | `host::set_adsr(5, 1, 15, 0, 5)`, `host::set_velocity(5, 650)`, low-pass filter open | Tight metallic tick. |
| . (rest) | No `host::play` fires this tick | Silence. |

Special boundary overrides.

- Pre-snap snare roll. Round 7 places this at "Section 3, Bar 8" which is the final bar of Limit Break Ramp A. The roll overrides the mask to fire S on every even tick (0, 2, 4, 6, 8, 10, 12, 14) with velocity ramping from 500 up to 1000 across the bar. The roll precedes the Chaos Phase A onset and provides percussive momentum into the peak tempo.
- Chaos blast beat. Sections 4 and 8 override the standard mask with the double-time pattern `[K, H, S, H, K, H, S, H, K, H, S, H, K, H, S, H]`. Kick on every fourth tick starting at 0, snare on every fourth tick starting at 2, hi-hat on alternating ticks. The pattern produces the aggressive wall-of-percussion character of the Chaos sections.
- Crash taper. The standard mask continues but velocity scales down approximately 85 percent per bar in parallel with the master-volume taper.

### Pitch-bend gestures (revised per round 6)

- Bass rise into Limit Break (channel 0). At Limit Break Ramp
  onset, `host::set_detune(0, ...)` ramps from 0 to +200 cents
  across the first eight ticks of the Limit Break Ramp. Replaces
  the earlier "last four ticks of Verse" placement.
- Dive bomb at snap-down (channel 1). At the snap-down moment,
  `host::set_detune(1, -600)` fires instantaneously. Over the
  subsequent sixteen ticks of the Breakdown, the detune ramps
  linearly back from -600 cents to 0. Replaces the earlier "ramp
  to -1200 cents before the snap" placement. The revised
  gesture is "drop and recover" rather than "fall in".

### Remaining open content (script-author pass derives)

The external session has delivered substantial content over
eight rounds. The remaining gaps below are now small enough to
fill in the implementation pass by deriving from the chord
matrix, the channel role descriptions, and the per-section
character notes.

- Lead melody for Verse A bars 9-16. Bars 1-8 are specified
  (with the alteration to bars 5-8); bars 9-16 derive from the
  Gm-C-F-B-flat cycle of fourths and the A7-flat-9 / Dm
  cadence using chord-tone selection in the Verse A motif
  rhythm.
- Lead melody for Limit Break Ramp A bars 1-4 (the 7/8 bar
  and the first three 4/4 bars). Bars 5-8 are specified;
  bars 1-4 derive from the D minor opening with the chromatic
  modulation toward F major.
- Lead melody for Limit Break Ramp B (8 bars in D harmonic
  minor) and Crash-Down (8 bars cycle-of-fifths descent).
- Chaos Phase B descending sweep pitches (round 7 frames as
  analogous to the bars 49-52 ascending run in reverse).
- Mid-harmony for Limit Break Ramp B and Crash-Down. Round 8
  did not commit; derives from the section harmonic motion.
- Breakdown bars 33-36 bassline content. Held D pedal or
  stepping toward F is the natural derivation.
- Pre-Snap Whole-Tone bassline contour. Round 8 implies the
  bass follows the channel 1 descending cascade sub-octave;
  specific pitches derive directly from the cascade pitches.

## Audio mix budgeting

Constraint that applies regardless of musical choice. At any
given sample, the sum across all active voices of
`volume_left * env.level * velocity` must stay below
approximately 0.85 to avoid pushing the master `tanh` soft-clip
into mutual saturation. The same holds for `volume_right`. The
existing songs ship with a comfortable margin. Song 3 with more
active voices needs to budget per-channel volume accordingly,
either by lowering individual `host::set_volume` values, by
keeping sustain low (`host::set_adsr`'s fourth argument), or by
lowering the master with `host::set_master_volume`.

## Verification checklist

The song is complete when:

- Compiles via `keleusma compile examples/scripts/piano_roll/piano_roll_3.kel`.
- Loads through `Vm::new` against the default arena without a
  `VerifyError`.
- A headless probe of the first one to two minutes of playback
  shows the expected per-tick `host::play` sequence, the
  expected init-block native calls, and the expected mid-song
  events at their scheduled ticks.
- Loops cleanly: after the intro plays once, subsequent
  iterations skip the intro and replay the main body.
- Hot swap in (other song -> song 3) and out (song 3 -> other
  song) both work; the data segment resets correctly and the
  host-side voice state is restored to defaults across the swap.
- Every host native listed in the module docstring at the top
  of [`examples/piano_roll.rs`](../../examples/piano_roll.rs) is
  called at least once, and every dynamic-capable native is
  called more than once (proving the dynamic capability is
  exercised).
- Workspace tests, clippy, fmt, release build all clean.

## Pending external contributions (after round 5)

Items still to fill in.

- Structural expansion to include both ramp-down and direct
  snap. User-direct requirement: the loop must visit 250 BPM
  twice with different exits, one ramped (Crash) and one
  instantaneous snap. Concrete tick layout for the second loop
  arm (a Verse Reprise, a second Limit Break Ramp, a second
  Chaos, and the one-tick snap exit) is pending. Estimated
  expansion is approximately 448 ticks bringing the loop body
  to roughly 1216 ticks.
- Stop using `host::set_tick_interval`. Round 5 used it for the
  fourth time. The actual native is `host::set_bpm(bpm)`.
- Drop sample-rate math from the script side. The script does
  not need to compute samples per tick. The host runs at
  forty-eight kilohertz.
- Confirm whether channel 4 stays active. Round 4 assigned it a
  power-chord-roots role; round 5 listed channels 0, 1, 2, 3, 5
  without mentioning 4. The spec keeps channel 4 active per
  round 4 unless the external session asks to drop it.
- Confirm whether channel 5 percussion replaces round 4's
  channel 4 power-chord roots or sits alongside it. Round 5's
  "6 available channels" framing reads as alongside.
- Confirmed song-name string for `host::song_name`. Open across
  all five rounds.
- Schedule for `host::set_vibrato`. Untouched across all five
  rounds. The lead melody on channel 1 is the natural candidate
  on sustained notes.
- Schedule for `host::set_volume` per-channel stereo positions.
  Untouched across all five rounds. The six-channel octave
  allocation now invites distinct stereo positions per register.
- Schedule for `host::set_master_volume`. Untouched across all
  five rounds.
- Confirm scale assignments per section. The scale-change
  requirement is user-direct (added after round 3). External
  session has not addressed it in rounds 4 or 5.
- Music-theory array sizing. Round 5 proposes a 64-element
  master chord timeline for a 768-tick loop, which places chord
  changes every 12 ticks (three beats). More natural would be
  48 elements at 16 ticks each (one bar per chord) or 96
  elements at 8 ticks each (half-bar per chord). The 64-element
  count likely needs revisiting once the structural expansion
  to roughly 1216 loop-ticks is committed.
- Concrete chord matrices per section.
- Concrete lead melody pitches per section.
- Concrete bassline pitches for the octave-jump ostinato.
- Concrete arpeggio step-pattern arrays per section, including
  the descending cascade in the Intro Pregnant Pause.
- Concrete percussion rhythm mask (per-tick hi-hat versus
  snare). Round 5 proposes a bitmask but does not commit to a
  specific pattern.
- Per-channel velocity values for the bass downbeat accent and
  the Chaos overdrive target.
- Decision on whether to assign roles to channels 6 and 7 to
  add stereo-paired voices or a true dual-oscillator chorus.

## Validation notes (round 1)

Findings recorded from the first external prompt-and-response
pair so that future rounds do not repeat the same gaps.

- API correction. The proposal calls
  `host::set_tick_interval(bpm_to_interval(...))`. That native
  does not exist. The actual host native is
  `host::set_bpm(bpm)`, which takes the desired BPM directly and
  performs the conversion to a tick interval inside the host.
- Coverage gaps. The proposal does not exercise
  `host::set_vibrato`, `host::set_volume` for stereo panning,
  `host::set_master_volume`, or `host::song_name`. The brief
  requires every host feature.
- Loop structure ambiguity. The proposal describes a linear arc
  from Part A through the Transition to Part B without
  identifying the intro-plus-loop boundary. The conservative
  reading is that Part A and the Transition are intro and only
  Part B loops, but the external session has not confirmed.
- Sonic concern on channel 0. A pure sine has no harmonic
  content, so a 150 Hz low-pass filter is audibly inert.
- Sonic concern on Part B. The proposal recommends driving
  velocity to maximum to demonstrate the master `tanhf`
  soft-clip. The existing songs budget per-channel velocity
  below the saturation ceiling deliberately. Driving into
  saturation produces audible distortion that may not match the
  "warm analog-style" framing.
- Pseudocode style. The proposal mixes Rust-style identifiers
  with the actual Keleusma surface. The implementation will use
  the established Keleusma style. All identifiers are lowercase
  and the data segment uses the fixed twenty-three-slot schema
  through `data state { ... }`, not a custom struct.
- Tempo placement. The proposal cruises at 140 BPM. The brief
  targets 140 to 180. Stronger contrast against the 90 BPM Part
  A would call for pushing the cruise higher.

## Validation notes (round 2)

- Direction shift. Round 2 abandons the "Cyberpunk Suite"
  framing in favor of a 16-bit boss theme inspired by the
  mid-1990s Japanese role-playing-game boss-theme idiom. The
  harmonic theory (Aeolian / harmonic-minor pivot on
  V, galloping octave bass ostinato, contrapuntal lead and
  arpeggiator) is idiomatic for the genre and constructable from
  theory without copying melodies. The round 2 framing is more
  musically concrete than round 1.
- API surface. Every native named in round 2 exists. Waveform
  codes (0 Square, 2 Sawtooth, 4 Pulse) match the host catalog.
- Channel count reduction. Round 1 used five channels; round 2
  uses three (bass, melody, arpeggiator). The noise-percussion
  channel from round 1 was dropped. Three channels are
  defensible for a chamber-style boss theme but leave five of
  the eight voices idle. The brief allowed up to eight.
- Coverage gaps persist. Round 2 still does not exercise
  `host::set_vibrato`, `host::set_volume` for stereo,
  `host::set_master_volume`, or `host::song_name`. The brief
  requires every feature.
- Musical theory error on detune. The proposal states "Channel 1
  with heavy `host::set_detune` to simulate a dual-oscillator
  synth lead." A single voice with a single detune offset is an
  out-of-tune voice, not a chorus. A true dual-oscillator
  chorused effect requires two voices playing the same pitch
  with different detune offsets mixed together. Either dedicate
  a second channel for the Bridge to achieve a true chorus, or
  reframe the detune use as a deliberate pitch shift.
- Saturation moment. Round 1 proposed sustained saturation
  across Part B; round 2 narrows it to a single climax moment
  before the loop point. The narrowing is defensible as an
  opt-in moment. Specific velocity values still determine
  whether the result is "warm rounding" or harsh distortion.
- Tempo placement. Round 2 cruises at 145 BPM after a ramp from
  110. The brief targets 140 to 180. 145 is just inside the
  target range and still near the floor.
- Persistence of script-side tick counter. Round 2 again
  proposes `state.tick_counter` as a 512-modulo counter. The
  host already passes the absolute tick as `input` every tick.
  The script can compute the loop position as `input % 512`
  directly without a persistent counter slot.
- Loop boundary ambiguity. Round 2 says the song plays
  infinitely "like a game boss loop" with a 512-tick frame but
  does not state whether the 64-tick intro plays only on the
  first iteration or on every loop iteration. The conservative
  intro-plus-loop reading is that ticks 0..64 are one-shot.

## Validation notes (round 3)

- Length and loop boundary now explicit. Round 3 resolves the
  ambiguity from rounds 1 and 2 by committing to a 128-tick
  one-shot intro followed by a 512-tick loop. The total length
  estimate of approximately sixty-four seconds for the first
  iteration and approximately fifty seconds for each
  subsequent loop matches the brief's "intro-plus-loop"
  framing and the external session's stated target of 60 to 90
  seconds per loop with a 5 to 15 second intro.
- Tempo placement improved. Round 3 cruises at 160 BPM with
  intra-loop variation to 140 and 165. Rounds 1 and 2 sat at
  the floor of the target range (140, 145); round 3 lands
  centrally within the brief's 140 to 180 target.
- Octave range made explicit. Round 3 commits each channel to
  a register, spanning roughly MIDI 24 (C1) to MIDI 93 (A6),
  about five and three-quarters octaves. This satisfies the
  user's "massive octave range" requirement explicitly.
- Channel count expanded. Round 1 used five channels with
  percussion. Round 2 used three. Round 3 uses five with no
  percussion (channels 0, 1, 2, 3, 4 active). Channels 5, 6,
  and 7 remain idle. The dropped percussion is a coverage gap
  for the Noise waveform (code 5).
- Phase arithmetic error. Round 3 states three loop phases of
  128 ticks each in a 512-tick loop, which sums to 384, not
  512. The spec extends Phase 3 to 256 ticks (loop-ticks
  256..512, first-iteration ticks 385..640) to close the gap.
  The external session should confirm or revise.
- Channel count typo. Round 3 wrote "6 available channels"
  while listing five (channels 0 through 4). The host exposes
  eight channels through the registered natives.
- Coverage gaps regressed. Round 3 dropped explicit mention of
  `host::set_detune`, `host::set_duty`, and `host::set_velocity`
  that were present in round 2. The text references "driving
  the master mix directly into `tanhf` saturation" without
  naming the native that performs the drive.
- Persistent state counters proposed again. Round 3 reintroduces
  `state.global_tick` and `state.loop_tick` as named fields. The
  host already passes the absolute tick as `input` every tick;
  at 160 BPM the `input` value would need approximately thirty
  million years to overflow `i64`. The script can compute the
  loop position as `(input - 128) mod 512` directly without
  persistent counters.
- Pseudocode style. Round 3 again uses Rust-style
  `struct SongState { ... }` instead of the Keleusma
  `data state { ... }` block. The implementation must use the
  fixed twenty-three-slot schema with the canonical field
  names.

## User-direct addition after round 3

The composition should change scale mid-piece across compositional
sections. This is a first-class structural requirement rather
than an emergent property of chord substitution. The current
spec assigns (now updated for the round 4 section names):

- Intro Accelerando and Intro Pregnant Pause: D natural minor
  with harmonic-minor pivot on the V chord.
- Loop Verse: D natural minor with harmonic-minor pivot on V.
- Loop Limit Break Ramp: modulation arc from D minor toward F
  major (chromatic ascent candidate, TBD).
- Loop Chaos: F major (carrying forward the round 3 "glimmer of
  hope" placement).
- Loop Crash: cycle-of-fifths arc Dm → Gm → C → F → B-flat → E
  diminished → A → D minor returning to D minor.

These scale assignments should be explicit in the script's
note-table dispatch. The script reads `state.section` to choose
the active scale, then derives pitches from the active scale
plus a chord-degree index. The user has not yet specified
whether additional scale changes beyond these are wanted.

## Validation notes (round 4)

- Timeline expansion. Round 4 grows the total first-iteration
  length from 640 ticks (round 3) to 960 ticks: a 192-tick
  unlooped intro followed by a 768-tick loop. The first
  iteration is approximately eighty-eight seconds wall-clock;
  each subsequent loop iteration is approximately sixty-six
  seconds. The lengths match the external session's stated
  target of 60 to 90 seconds per loop with a 5 to 15 second
  intro, except the intro at twenty-two seconds slightly
  exceeds the upper end of that target. The user explicitly
  asked for a slower intro, so the excess is intentional.
- Tempo trajectory now user-directed. The 90 → 150 → 250 → 150
  BPM profile is a direct user requirement and supersedes the
  earlier 140 to 180 BPM working range. Brief and design
  constraints updated.
- API regression: `host::set_tick_interval` reintroduced. Round
  4 again uses this name despite the round 1 correction. The
  actual native is `host::set_bpm(bpm)`. The spec records
  `host::set_bpm` throughout. The external session has not
  internalized this correction across four rounds.
- Sample-rate factual error. The proposal computes
  `samples_per_tick = 44100 * 60 / (BPM * 4)`. The host runs at
  forty-eight kilohertz, not forty-four point one kilohertz
  (see `examples/piano_roll.rs:237`). Both numbers are moot for
  the script because the host performs the BPM-to-interval
  conversion internally; the script calls `host::set_bpm(bpm)`
  and stops.
- WCET framing. At 250 BPM the per-tick budget is sixty
  milliseconds (`60_000_000 / (250 * 4) = 60_000` microseconds).
  The existing songs' per-tick bodies complete in microseconds.
  There is no realistic deadline-miss risk at 250 BPM. The
  external session's "razor-thin" framing is dramatic but not
  tight in practice.
- Crash-phase arithmetic. The ASCII diagram labels the Crash
  segment "128 Ticks" and the prose says "Over 8 bars" while
  the endpoint arithmetic produces 192 ticks (12 bars).
  Endpoints govern in the spec. The 768-tick loop math only
  closes with a 192-tick Crash. The external session should
  reconcile the label and prose with the endpoints.
- Persistent counters proposed again. Round 4 reintroduces
  `state.global_tick` and `state.current_bpm` as named fields.
  The host already passes the absolute tick as `input` every
  tick. The script can branch on `input < 192` for intro and
  compute `loop_tick = (input - 192) mod 768` for the loop
  body. No persistent counters are required.
- Pseudocode style. Round 4 again uses Rust-style
  `struct CompositionState { ... }` instead of the Keleusma
  `data state { ... }` block.
- Feature coverage partial recovery. Round 4 reintroduces
  explicit `host::set_velocity` use (Chaos overdrive) and
  `host::set_duty` use (Crash pulse-width-modulation drone)
  that round 3 had dropped. Coverage of `host::set_lpf` is
  expanded to a parallel filter sweep across the Limit Break
  Ramp. `host::set_vibrato`, `host::set_volume` (stereo),
  `host::set_master_volume`, `host::set_detune`, and the Noise
  waveform remain unaddressed.
- Scale-change requirement still pending external
  contribution. The external session has not been asked about
  scale changes yet; round 4 was focused on tempo and length.
  The current scale assignments in the spec are inferred from
  round 3's harmonic content extended to the new section
  layout.

## Validation notes (round 5)

- Parametric modulation introduced explicitly. Round 5 adds
  three continuous per-tick modulations: PWM sweep on the
  channel 2 arpeggiator, filter-envelope simulation on the same
  channel via per-tick LPF decrement, and detune oscillation on
  channel 3 during the Crash. This matches the user's emphasis
  on parametric features and the "NES strategy of updating
  registers every tick".
- Dynamic ADSR demonstrated. Round 5 swaps ADSR profiles per
  tick on channel 5 (hi-hat versus snare) and collapses
  channel 1 sustain at Chaos onset. ADSR is no longer a static
  init-only parameter.
- Retrigger demonstrated in both states. Channel 1 starts with
  retrigger off (legato) and flips to retrigger on at Chaos
  onset (staccato). The "use features in both on and off
  states" intent is satisfied for retrigger.
- Noise waveform restored. Round 1 used Noise (code 5) for
  percussion on channel 5; rounds 2 through 4 dropped it.
  Round 5 restores it through the percussion engine. The Noise
  waveform is no longer a coverage gap.
- Detune restored. Round 2 used detune on channel 1 during the
  Bridge; rounds 3 and 4 dropped it. Round 5 restores it on
  channel 3 during the Crash, this time as continuous
  oscillation rather than a static offset. The round 2
  musical-theory concern about single-channel detune
  simulating a chorus is mitigated by reframing as a tremolo
  effect, though the proposal still calls it "Chorus/Detune
  effect" which conflates two distinct phenomena.
- API regression: `host::set_tick_interval` for the fourth
  time. The actual native is `host::set_bpm(bpm)`.
- Channel 4 dropped without comment. Round 4 placed power-chord
  roots on channel 4; round 5's channel manifest lists 0, 1,
  2, 3, 5 with no mention of 4. The spec preserves channel 4
  per round 4 unless the external session asks otherwise.
- Channel-count framing still says "6 available host channels"
  while naming only five (0, 1, 2, 3, 5). The host exposes
  eight. With channel 4 preserved, the active count is six,
  matching the framing if not the named list.
- Music-theory array sizing. The proposed 64-element master
  chord timeline maps to 12 ticks per chord under the round 4
  768-tick loop, which is three beats per chord and does not
  align with the 16-tick bar grid. The mismatch likely
  resolves naturally once the structural expansion to
  approximately 1216 loop-ticks lands.
- Coverage gaps remaining. `host::set_vibrato`,
  `host::set_volume` per-channel stereo positions, and
  `host::set_master_volume` are still unaddressed across five
  rounds.

## User-direct additions after round 4

The user's bullet list issued alongside the round 5 quote
established three new requirements not present in any external
session response.

- Per-tick BPM interpolation. Every tempo ramp must update on
  each tick with intermediate values. The user phrases this as
  the "NES strategy of updating registers every tick" and
  states that "ramp does not mean 90 then 150; there needs to
  be intermediate tempo values". The round 4 spec already
  encodes per-tick BPM updates through formulas like
  `host::set_bpm(90 + t * 60 / 128)`; the requirement
  formalises this and prevents regression to step-function
  tempo changes.
- Both ramp-down and direct snap from 250 BPM to 150 BPM. The
  loop body must visit 250 BPM twice with different exits: one
  ramped (the existing Crash) and one instantaneous one-tick
  snap. The single-pass round 4 structure does not satisfy
  this; structural expansion is required. The spec records the
  expected expansion as adding approximately 448 ticks for a
  second loop arm (Verse Reprise, second Limit Break Ramp,
  second Chaos exited by snap).
- Length flexibility. The composition may exceed typical
  16-bit-era length. The round 3 "60 to 90 second loop" target
  is relaxed.

## Extended Dynamics Catalog

This catalog enumerates every dynamic technique committed to the
composition, with section placement and music-theory
justification for each. The catalog is the canonical reference
for the script-author pass.

### Doubling and harmonization techniques

#### Stereo unison with detune

Channels involved. Channel 1 (lead), channel 6 (left twin),
channel 7 (right twin).

Configuration. Channels 6 and 7 mirror channel 1's pitch and
waveform. Channel 6 carries +7 cents detune and is hard-panned
left via `host::set_volume(6, 1000, 0)`. Channel 7 carries -7
cents detune and is hard-panned right via
`host::set_volume(7, 0, 1000)`.

Placement. Active during Loop Verse A. Disabled at the
transition into Loop Limit Break Ramp A (the buildup is more
effective with a thinner texture). Inactive in Intro,
Pregnant Pause, Chaos passages, and Crash.

Music-theory justification. Doubling a melodic voice at unison
across two hard-panned voices with small opposing detune
offsets produces a wide stereo image without altering harmonic
content. The 14-cent total spread (plus seven on one side,
minus seven on the other) is below the threshold at which the
ear perceives intonation error but above the threshold at
which it perceives chorus thickness. The technique adapts the
analog-synth "unison stack" idiom common to 1980s polyphonic
analog synthesizers to a two-channel hardware budget. In the
D-minor Verse A, the lead carries expressive vibrato and
sustained intervals; the unison thickening matches the
expressive register without obscuring melodic identity.

#### Detuned doubling

Channels involved. Channel 0 (bass root), channel 4 (octave-
above bass doubler).

Configuration. Channel 4 plays the bassline an octave above
channel 0 with +5 cents detune. Both centered via
`host::set_volume(0, 500, 500)` and
`host::set_volume(4, 500, 500)`. Channel 4 mirrors channel
0's waveform across the Verse-to-Limit-Break-Ramp transition
(sawtooth during the gallop, sine during the sub-bass drone
mode).

Placement. Active during Loop Verse A and Loop Limit Break
Ramp A. Disabled during Intro Pregnant Pause (bass channels
silenced for the cascade), during Verse B (channel 4 swaps to
its power-chord-roots role), and during Crash (bass thins
out for the descent).

Music-theory justification. Octave doubling reinforces the
fundamental by aligning the doubling voice with the first
overtone of the bass partial series. The 5-cent detune offset
on the octave-up doubler creates slow beating against the
natural harmonic that the ear interprets as bass thickness
rather than detuned pitch. This is the "fat bass" technique
from synth-bass programming, widely adopted on 8-bit and
16-bit hardware. Distinct from stereo unison because the
doubler sits
at the octave rather than the unison and is mixed centrally
rather than hard-panned.

#### Parallel-interval harmonization

Channels involved. Channel 1 (lead), channel 6 (third above),
channel 7 (sixth above).

Configuration. Channel 6 plays the lead a major or minor third
above (three or four semitones, scale-aware in F major).
Channel 7 plays the lead a major or minor sixth above (eight or
nine semitones, scale-aware). Detune offsets reset to zero on
both channels; hard-panning unchanged from Verse A
configuration.

Placement. Active during Loop Verse B (Reprise) in F major.
Disabled during Loop Limit Break Ramp B (texture thins
into the buildup). Inactive elsewhere.

Music-theory justification. Parallel thirds and parallel
sixths are the cornerstones of imperfect-consonance
counterpoint in baroque and classical writing. In a major
key, parallel-third harmonization above the lead produces a
diatonic harmony that requires no chromatic adjustment.
Scale-degree 1 harmonized at a third becomes scale-degrees 1
and 3 (in F major, F and A) forming the tonic chord's root
and third; scale-degree 2 becomes 2 and 4 (G and B-flat)
producing a passing suspension that resolves naturally; and
so on through the scale. Parallel sixths produce a
complementary harmonic line that doubles the lead an octave
plus a sixth above, creating a fuller voicing without
aggressive dissonance. The technique is idiomatic for
triumphant, pastoral, or heroic passages in major-mode tonal
music. Placing this in the F-major Verse Reprise marks the
section as a heroic second wind against the D-minor Verse
A's first encounter.

### Pitch-bend gestures via detune ramp

Channels involved. Channel 1 (lead dive bomb at snap-down),
channel 0 (bass rise into Limit Break Ramp).

Configuration. The dive-bomb gesture ramps
`host::set_detune(1, ...)` from 0 cents to -1200 cents (one
octave down) across the last eight ticks before the snap-down
moment at tick 640. The bass-rise gesture ramps
`host::set_detune(0, ...)` from 0 to +200 cents (whole tone up)
across the last four ticks of each Verse before its Limit
Break Ramp.

Placement. Dive bomb fires once per loop iteration at the
Chaos A to snap-down boundary (loop-ticks 440..448
approximately, just before the snap at loop-tick 448). The
bass rise fires twice per loop, at the Verse-to-Ramp
transitions on both sides of the loop body.

Music-theory justification. The dive-bomb gesture is a
glissando spanning one octave downward, a punctuating effect
borrowed from synth and electric-guitar idiom. In the
context of a 250 BPM peak collapsing to 150 BPM, the
descending pitch gesture musically reinforces the tempo
collapse, presenting the snap as a deliberate musical event
rather than a discontinuity. The bass-rise gesture is a
short leading-tone-style ascent that pulls the ear into the
following section's higher tempo, an aural analogue of a
pickup measure.

### Vibrato on the stereo unison pair

Channels involved. Channel 1 (lead), channel 6, channel 7.

Configuration. Channel 1 vibrato at 5 Hz / 30 cents during
Verse A sustained notes (`host::set_vibrato(1, 500, 30)`).
Channel 6 vibrato at 6 Hz / 25 cents
(`host::set_vibrato(6, 600, 25)`). Channel 7 vibrato at
4 Hz / 25 cents (`host::set_vibrato(7, 400, 25)`).

Placement. Active during Loop Verse A sustained notes only.
Disabled during fast passages within Verse A to avoid
muddying. Inactive elsewhere.

Music-theory justification. Ensemble strings and chorused
synth pads naturally exhibit slight rate variations between
voices, producing the "human" ensemble sound rather than the
synchronized vibrato of a single voice. Offsetting the unison
twins' vibrato rates by approximately one Hertz from the
lead's rate produces this ensemble shimmer. The depth offset
(25 versus 30 cents) keeps the twins slightly less prominent
than the lead, preserving melodic clarity.

### Resonant low-pass-filter sweep on channel 0

Channels involved. Channel 0 (bass).

Configuration. During Loop Limit Break Ramp A, `host::set_lpf(0,
cutoff)` ramps from 200 Hz to 2000 Hz across 128 ticks. The
sweep runs in parallel with the BPM ramp from 150 to 250.
During Loop Limit Break Ramp B, the same sweep repeats
beginning from the channel 0 sub-bass-drone state.

Placement. Active during both Limit Break Ramps. Static at low
cutoff during the sub-bass-drone passages flanking the ramps.

Music-theory justification. Filter automation transforms a
single instrument from muted-and-controlled to bright-and-
aggressive across the buildup, contributing dynamic intensity
in parallel with the tempo and vibrato ramps. The resonant
low-pass-filter sweep is the canonical buildup gesture in
electronic dance music and was a staple of late 16-bit-era
bass programming.

### Stereo motion on lead and mid-harmony

Channels involved. Channel 1 (lead), channel 3 (mid harmony).

Configuration. Channel 1 panning follows the melodic contour:
when the lead's MIDI pitch is above its scale center, the
panning leans slightly right (`host::set_volume(1, 400, 600)`);
when below center, leans slightly left
(`host::set_volume(1, 600, 400)`). Updated on each note onset
during Verse A. Channel 3 panning swirls slowly across the
stereo field via per-tick `host::set_volume(3, sweep_l,
sweep_r)` driven by a slow LFO completing one cycle every
eight bars during Verse passages.

Placement. Channel 1 contour panning active in Verse A and
Verse B. Channel 3 swirl active in both Verses.

Music-theory justification. Pairing pitch height with stereo
position aligns with the standard convention that high notes
sit to the right and low notes to the left, derived from
piano-reading orientation. The subtle panning shift (±100
from center, not hard-panning) preserves the lead's
prominence while adding spatial life. The channel 3 swirl is
a slow ambient stereo motion that fills the space between
the hard-panned unison twins and the centered bass without
distracting from the lead.

### Crescendo and decrescendo via velocity automation

Channels involved. Channel 1 (lead) for the build; all
channels for the final decrescendo.

Configuration. Crescendo: during the second half of Verse A
(last eight bars), `host::set_velocity(1, ...)` ramps from
700 to 1000 across 128 ticks. Decrescendo: during the last
eight ticks of the Crash, all channels' velocities ramp from
their current values down to 500 via per-tick
`host::set_velocity` calls.

Placement. Crescendo runs in the build-up to Limit Break Ramp
A. Decrescendo punctuates the end of the Crash before the loop
boundary.

Music-theory justification. A four-bar crescendo built into
the second half of a section is the canonical "build" gesture
in popular music and orchestral writing, signaling impending
section change without explicit instrumental cue. The
decrescendo at the end of the loop body provides
psychoacoustic separation between the loop-end and the
next-iteration loop-start, helping the seamless loop read as
"breath and restart" rather than "abrupt restart".

### Retrigger toggles beyond channel 1

Channels involved. Channel 0 (bass), channel 3 (mid harmony).

Configuration. Channel 0: retrigger on during Verse A and Verse
B (for sharp octave-jump attacks in the gallop); retrigger off
during sub-bass-drone passages of the Limit Break Ramps (for
legato sustained drone). Channel 3: retrigger off during
Triangle pad sustain of Verse A and Verse B; retrigger on
during the Pulse detune drone of the Crash.

Placement. State flips at the boundaries identified.

Music-theory justification. Retrigger as a per-section attack
characteristic. On legato passages (drones, sustained pads),
retrigger off keeps the envelope from restarting on each note,
producing the smooth held-tone character. On staccato or
percussive passages (gallop, drone with movement), retrigger
on ensures each note onset has a fresh attack transient.

### Scale-change schedule

Sections and scales.

- Intro Accelerando: D natural minor with harmonic-minor pivot
  on V. The pivot raises the seventh (C natural to C sharp) on
  V-chord beats, giving the V chord a major-quality dominant
  function. This is the standard 16-bit harmonic-minor pivot.
- Intro Pregnant Pause: D natural minor (no pivot, descending
  arpeggio over a static D minor harmony).
- Loop Verse A: D natural minor with harmonic-minor pivot on V.
- Loop Limit Break Ramp A: D natural minor moving through
  chromatic alteration. Flat second (E flat) and raised third
  (F sharp) are introduced on the last four ticks to prepare
  the Phrygian dominant of Chaos A.
- Loop Chaos A: D Phrygian dominant (D, E flat, F sharp, G, A,
  B flat, C). Exotic mode with flat second and raised third.
  The mode carries Middle Eastern and flamenco association,
  used in metal and progressive rock for tense or "evil"
  passages. Maximum tension for the peak-tempo Chaos.
- Snap-Down: brief whole-tone gesture (D, E, F sharp, G sharp,
  A sharp, C). The whole-tone scale has no tonic feel and
  produces a moment of disorientation that musically mirrors
  the tempo discontinuity.
- Loop Verse B (Reprise): F major (relative major modulation).
  Heroic second-wind character. F major shares all notes with
  D natural minor except via the harmonic-minor pivot, so the
  modulation is structurally close yet emotionally distant.
- Loop Limit Break Ramp B: F major modulating chromatically
  toward Phrygian dominant return. Mirrors Ramp A's chromatic
  ascent in the new tonal center.
- Loop Chaos B: D Phrygian dominant return (mirrors Chaos A).
  Symmetry between the two peaks emphasizes the structural
  parallel of the loop's two halves.
- Loop Crash: cycle of fifths starting on A (the V of D minor):
  A major → D minor → G minor → C major → F major → B-flat
  major → E diminished → A major → D minor. The cycle returns
  to the loop's tonic and prepares re-entry to Verse A.

Music-theory justification (composite). The composition
travels through five distinct scales or modes plus the
cycle-of-fifths arc, satisfying the user's explicit scale-
change requirement. The progression follows a structural
arch: D minor (familiar tension) → Phrygian dominant (exotic
peak) → F major (heroic respite) → Phrygian dominant return
(symmetrical peak) → cycle-of-fifths (return). The choice of
modes at peaks (Phrygian dominant) and respite (F major)
reflects 16-bit boss-theme convention where the relative
major appears for melodic "glimpses of hope" within a
predominantly minor piece.

### Time-signature pivots

Locations.

- One 7/8 bar at the Limit Break Ramp A onset (tick 448). The
  bar occupies 14 ticks instead of 16, displacing the
  following bar's downbeat. The next bar resumes 4/4.
- One 3/4 bar at the snap-down preceding tick 640. The bar
  occupies 12 ticks instead of 16, truncating the bar before
  the snap.

Music-theory justification. The 7/8 bar at the Limit Break
Ramp A onset disrupts the 4/4 grid by removing one eighth-
note from the expected sixteen-tick bar, creating asymmetric
momentum that drives forward. 7/8 is a standard modal-rock
and progressive-rock device, used effectively by 16-bit-era
composers in moments of urgency. The
3/4 bar before the snap-down truncates the bar by one quarter
note, producing the "pulled rug" feel that complements the
abrupt tempo snap. Both pivots are brief (one bar each) and
return to 4/4 immediately, preserving the dance-like
regularity of the surrounding sections.

### Polyrhythm in the arpeggiator

Channels involved. Channel 2 (arpeggiator) against channel 0
(bass) and the percussion grid.

Configuration. During Loop Chaos A and Loop Chaos B, the
channel 2 arpeggiator switches from straight 16th-note
subdivision to triplet subdivision. Implementation
approximates triplets on the 16th-note tick grid by
alternating five-tick and six-tick step intervals (mean
5.33 ticks per step, two triplets every sixteen ticks).

Placement. Active in both Chaos sections only. Off elsewhere.

Music-theory justification. Three-against-four polyrhythm is a
classic tension device used by 16-bit composers in
culmination moments. The triplet feel on the arpeggiator
against the duple feel of the bass produces cross-rhythm
that the ear perceives as accelerating intensity even though
the underlying tempo is constant. The effect is most
prominent when isolated to brief peak sections, where the
ear has not adjusted to the triplet feel as the new norm.

### Drum fills at section boundaries

Channels involved. Channel 5 (percussion).

Configuration. Pre-snap fill: four 16th-note snare hits on
ticks 636 through 639 (immediately before the snap at tick
640). Pre-crash fill: two-tick descending tom roll on ticks
1086 and 1087 (immediately before the Crash onset at tick
1088), implemented by playing channel 5 with decreasing pitch
on each tick to simulate a "tom" tuning via noise.

Placement. Pre-snap fill at the Chaos A exit. Pre-crash fill
at the Chaos B to Crash boundary.

Music-theory justification. A drum fill before a section
change is a universal punctuation mark, present in rock,
electronic music, and 16-bit game soundtracks. Fills give the
listener advance notice of an impending structural event and
provide rhythmic momentum across the transition. The snare
fill before the snap is the more dramatic of the two,
matching the snap's abruptness; the tom roll before the
Crash matches the Crash's longer descending character.

### Bassline pattern variation between Verses

Channels involved. Channel 0 (bass), channel 4 (detuned
doubler).

Configuration. Verse A bassline: galloping ostinato between
root and octave-up (D2, D3, D2, D3, ... for D minor).
Verse B bassline: displaced ostinato with off-beat emphasis
on the major third (F2, A2, F3, A2, ... for F major). The
displaced pattern shifts the rhythmic accent from the
downbeat to the off-beat, marking the section as "past the
first peak".

Placement. Verse A and Verse B exclusively.

Music-theory justification. The second Verse should signal
structural advancement without abandoning the bass-driven
character of the first. A displaced ostinato pattern keeps
the rhythmic identity (continuous 16th-note bass motion)
while changing the melodic identity (root-plus-octave to
root-plus-third). The technique is common in dance music
remixes where the same bass groove is given new melodic
content for variation.

### Pickup notes (anacrusis)

Channels involved. Channel 1 (lead).

Configuration. Two-tick pickup into Verse A: channel 1 plays
a brief upward gesture (e.g., A4 then C5 in D minor) on
ticks 190 and 191, landing on D5 at the Verse A downbeat at
tick 192. Two-tick pickup into Verse B: channel 1 plays an
analogous gesture in F major (e.g., D5 then E5) landing on
F5 at the Verse B downbeat.

Placement. Once per loop iteration at each Verse onset.

Music-theory justification. An anacrusis (upbeat pickup) into
a section gives the downbeat structural weight and is
idiomatic in classical, folk, and popular music. The pickup
gives the ear a melodic clue that something significant is
arriving, marking the section onset more strongly than a
silent approach.

### Loop-iteration variation via state.loop_count

Channels and natives involved. Multiple.

Configuration. The script reads `state.loop_count` and varies
behaviour on subsequent iterations.

- Iteration 0 (first loop): as specced above.
- Iteration 1 onward: the channel 6 and channel 7 stereo
  unison pair drops to one voice (channel 7 silenced) for a
  thinner texture during Verse A. The texture rebuilds at
  Verse B.
- Iteration 2 onward: Verse B transposes up by a whole step
  (from F major to G major).
- Iteration 3 onward: per-channel velocities reduce by ten
  percent each iteration to keep extended listening from
  becoming fatiguing, until iteration 8 where they reset.

Music-theory justification. Extended listening benefits from
variation that keeps the brain engaged. The progressive
thinning of the unison pair on iteration 1, the transposition
on iteration 2, and the rolling velocity reduction together
provide variation across many loop cycles without ever
fundamentally changing the composition. The variations are
constructed so that the listener experiences each loop as
"the same song with a small new detail" rather than "a
different song". This is the Keleusma-native pattern for
extended-listening variation, made possible by `loop_count`
in the data segment.

## User-direct additions after round 5

The user issued three additional requirements after round 5,
along with an instruction to extrapolate the spec to fill the
remaining gaps. The role-split clarified at this point is that
the implementation expert (this spec author) handles the
technical extrapolation while the external music-design session
remains a value-add advisor for the musical-design domain.

- All eight host channels must be active somewhere in the
  composition. Round 5 reached six channels (0 through 5). The
  extrapolation adds channels 6 and 7 as a stereo unison pair
  doubling the lead with opposing detune offsets and hard-left
  versus hard-right panning. The user describes this technique
  as "dedicated right and left speaker channels that play
  slightly different notes that sound good together" and notes
  the precise name is forgotten. The implementation-expert
  reading commits to stereo unison with detune; parallel-
  interval harmonization is recorded as an alternate
  interpretation in the channel-assignments table.
- Every host feature must be exercised in active, inactive,
  and dynamic states across the composition. The extrapolated
  event schedule above places `host::set_vibrato`,
  `host::set_volume` per-speaker stereo, and
  `host::set_master_volume` (the three persistently unaddressed
  natives) in all three states. The coverage matrix in the same
  section traces every native through all applicable states.
  Features without a clear off state (waveform, duty,
  attack-decay-sustain-release) carry active and dynamic states
  only.
- Channels are repurposed across sections so the eight physical
  channels surface approximately twenty to thirty virtual
  instrument roles. The user frames this as a rough target
  rather than a hard count. The spec carries forward existing
  channel-role morphs (channels 0, 1, 3 morph waveform and
  envelope at section boundaries) and notes that channel 4 and
  channel 5 also carry multiple roles. Channel 4 spans power-
  chord roots, harmonic doubler, and low drone across sections.
  Channel 5 spans hi-hat, snare, and kick through
  attack-decay-sustain-release swapping. The implementation-
  expert extrapolation does not enumerate every per-section
  role for every channel; the principle is documented and the
  concrete per-section role table is open work for the
  script-author pass.

## Resolved conflicts (round 7 delivered)

The three open conflicts from round 6 are resolved in round 7
with master-session overrides where required.

1. Loop length and dual-peak structure. Resolved. The structure
   now contains both peaks with different exits: Chaos Phase A
   exits via the Pre-Snap whole-tone gesture followed by the
   one-tick snap from 250 BPM to 150 BPM; Chaos Phase B exits
   via the ramp-down Crash from 250 BPM to 150 BPM across 8
   bars. The user-direct requirement is satisfied. Loop body
   is 1034 ticks; first iteration is 1162 ticks.

2. All three doubling techniques. Resolved with implementation-
   expert placements carrying. Stereo unison with detune sits
   on channels 6 and 7 during Verse A (round 6 commitment).
   Parallel-interval harmonization sits on channels 6 and 7
   during Verse B with master-session override for diatonic
   scale-aware intervals (round 7 proposed fixed +4 and +9
   semitones which produce non-diatonic results on half the
   scale degrees; correct diatonic harmonization alternates
   between +3 and +4 semitones for thirds and between +8 and
   +9 semitones for sixths). Detuned doubling sits on channels
   0 and 4: round 8 finalizes the channel 4 detune offset at
   -10 cents (octave below channel 0). Round 8 also confirms
   the stereo unison pair on Verse A uses ±12 cents detune on
   channels 6 and 7 (Verse A and Chaos A active windows).

3. F major placement. Resolved. Round 7 places F major as the
   tonal center of Verse B (matching implementation-expert
   intent) with a brief pivot to F major mid-way through Limit
   Break Ramp A (round 7 addition). The original
   implementation-expert reading of "F major as second-wind
   Verse Reprise" stands; the Ramp A pivot is a useful musical
   added element bridging the two tonal centers.

## Master-session overrides (round 7)

The following deviations from round 7's literal content are
recorded for the script-author pass.

- Parallel-interval harmonization on channels 6 and 7 in Verse
  B is scale-aware diatonic, not fixed +4 and +9 semitones.
  Channel 6 plays the major or minor third above channel 1
  depending on the lead's scale degree; channel 7 plays the
  major or minor sixth above. The script computes the offset
  from the active scale (F major) and the lead's scale degree.
  Justification: fixed +4 semitones in F major produces
  C-sharp, F-sharp, and G-sharp on scale degrees 3, 6, and 7
  respectively, none of which are diatonic to F major. The
  result would be chromatic dissonance on roughly half the
  bars rather than the intended "glorious high-fantasy
  ensemble" character.
- Verse A bar 13 chord is A7-flat-9, not Em7-flat-5. Round 7's
  chord matrix labels bar 13 "Em7-flat-5" but the root array
  gives MIDI 33 (A1) which is the root of A, not E. A7-flat-9
  functions as V7-flat-9 in D minor harmonic and shares the A
  root, producing the V-tension-resolution feel that the
  surrounding harmony implies.
- Section 1 bar 8 lead melody pitch MIDI 80 (A-flat-5 or
  G-sharp-5) is retained as written. The note is not in
  D natural minor but reads as a chromatic approach to A5.
  Implementation may either preserve as written for chromatic
  color or substitute MIDI 81 (A5) for diatonic correctness.
  Master-session preference: preserve as written.

## Sheet music feasibility

### Overview

Sheet music for this composition is feasible. Standard classical
notation will, however, struggle to capture the hyper-automated
data-driven character of the piece. A traditional copyist or a
sight-reading musician would find the score an avant-garde
technical challenge. A functional score would adopt the approach
taken by 20th-century composers working in electroacoustic and
spectral music. It would combine traditional staff notation with
a detailed performance manual and a graphic-notation overlay.

The remainder of this section enumerates the specific techniques
used in the composition, identifies the friction points each
presents for notation, and describes how a professional engraver
would solve them.

### Per-tick tempo ramping

Traditional sheet music handles tempo changes through terms such
as accelerando or ritardando, sometimes accompanied by a dashed
line.

Friction point. In this composition the tempo is not a vague
"speeding up" but a strict mathematically linear calculation
updated tens of times per second.

Notation solution. A conductor or performer cannot dynamically
compute a linear equation in real time. The sheet music would
state the starting and ending beats-per-minute over a bracketed
number of bars and add the explicit instruction "Sempre
accelerando, strictly linear." For a live ensemble a conductor
would either require a click track or simulate the ramp with a
practiced steady widening of their gesture.

### Asymmetric 7/8 pivot bar

This is the easiest technique to capture in notation.

Notation solution. Sheet music handles time-signature changes
effortlessly. Bar 17 of the loop body would carry a prominent
7/8 time-signature block, followed immediately by a 4/4 block on
bar 18. The engraver would group the eighth notes as 2-plus-2-
plus-3 or 3-plus-2-plus-2 through beam groupings, showing the
performers where the missing sixteenth was cut.

### Triplet polyrhythm in the Chaos sections

Writing a straight sixteenth-note bass and drum pattern alongside
an arpeggiator in triplet feel is standard fare for complex
polyphonic music.

Notation solution. The arpeggiator staff would use tuplets,
specifically sixteenth-note triplets or nested tuplets, spanning
the standard quarter-note beats. The visual result is dense and
comparable to a nocturne cascade or a math-rock transcription.
Any percussionist or advanced pianist trained in polymetric
playing could execute it.

### Continuous parametric automation

The script automates pulse-width and low-pass-filter cutoff on a
per-tick basis. Musicians do not have a "filter knob" on a violin
but they do have physical equivalents.

Notation solution for the channel 2 low-pass-filter decay. The
engraver uses a specialised expression lane below the staff,
similar to a dynamics lane, with a wedge or custom line element
labelled "LPF 3500 Hz to 600 Hz". For acoustic instruments this
maps to a transition from sul ponticello (bright glassy playing
near the bridge) to sul tasto (warm muffled playing over the
fingerboard).

Notation solution for the channel 1 dive-bomb and linear recovery
at the snap moment. The engraver writes a large glissando line
plunging down an octave, with a dotted line indicating a strict
un-metered linear slide back up to pitch over exactly one bar.

### Stereo unison, detune, and parallel harmonization

Channels 6 and 7 are alter-egos of channel 1 that shift between
plus-or-minus seven cents detuned unison in Verse A and parallel
diatonic thirds and sixths in Verse B.

Notation solution. On a grand conductor's score the three voices
appear on three separate staves labelled Lead, Voice II, and
Voice III, bracketed together in the manner of a string section's
first violin, second violin, and viola.

During Verse A the three staves carry identical pitches. The
plus-or-minus seven cent detune is captured as a performance
note at the top of the system reading "Voice II tuned slightly
sharp, Voice III tuned slightly flat to create an ensemble chorus
effect."

During Verse B the staves visually split into cascading
counterpoint as channels 6 and 7 take on their parallel-
harmonization roles.

### Master-score layout

A publication-grade score is an eight-staff master score. The
staff-to-channel mapping below preserves the channel allocation
documented elsewhere in this specification.

| Staff | Channel | Role | Notation idiom |
|-------|---------|------|----------------|
| 1 | Channel 1 | Neoclassical lead (sawtooth into square) | Standard treble staff |
| 2 | Channel 6 | Left twin (unison or parallel third) | Standard treble staff |
| 3 | Channel 7 | Right twin (unison or parallel sixth) | Standard treble staff |
| 4 | Channel 2 | Interlocking arpeggiator (pulse) | Heavy tuplet brackets in the Chaos sections |
| 5 | Channel 3 | Mid-harmony (triangle into pulse pad) | Sustained chords or double-stops |
| 6 | Channel 0 | Driving bass line (sawtooth into sine) | Explicit gallop rhythmic notation |
| 7 | Channel 4 | Sub-bass octave stack (pulse) | Standard bass staff |
| 8 | Channel 5 | Percussion engine (noise) | Multi-line drum staff |

### Verdict

Sheet music is feasible. The result would resemble a hybrid of a
baroque concerto grosso and a modern math-rock chart. A human
ensemble would introduce natural organic imperfections that the
script bypasses. The score on paper would highlight how
structurally sound, contrapuntally tight, and deeply classical
the underlying composition is.
