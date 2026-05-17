//! Eight-channel SDL3 audio piano roll driven by a Keleusma tick
//! control loop, with hot code reload between two songs.
//!
//! The long-form companion manual is
//! `docs/guide/PIANO_ROLL.md`. The manual narrates the example
//! for three reader populations (song authors, host developers
//! lifting the example into another application, and host
//! developers studying the example as an architectural reference
//! for other control-loop domains). This module-level docstring
//! is the authoritative catalog of host native functions,
//! parameter ranges, defaults, waveform codes, and data-segment
//! slot layout. Where the manual narrates around the surface, the
//! comment below lists the surface itself.
//!
//! # Architecture
//!
//! A roster of Keleusma scripts named `piano_roll_<N>.kel`
//! (currently `piano_roll_0.kel`, `piano_roll_1.kel`, and
//! `piano_roll_2.kel`) is precompiled at startup and
//! registered in the `SONG_SOURCES` slice in this file. The currently-active script runs on the
//! main thread at one yield per 16th-note tick (125 ms at 120
//! BPM by default; the script can call `host::set_bpm` to
//! change the tempo mid-playback). At each tick the script
//! emits per-voice setup natives (waveform, duty cycle, ADSR,
//! enable, etc.) on the first iteration and
//! `host::play(channel, midi)` / `host::silence(channel)`
//! natives on note boundaries. These natives update voice state
//! shared with the SDL3 audio callback, which renders samples
//! on the audio thread.
//!
//! Adding a new song is just dropping a `piano_roll_<N>.kel`
//! file next to this one and appending a matching `include_str!`
//! line to `SONG_SOURCES`. The swap action cycles through the
//! slice in order.
//!
//! - **Audio thread (SDL3 callback)**: receives a sample buffer
//!   to fill at sample rate (48 kHz), reads the current voice
//!   state from a `Mutex<[Voice; 8]>`, advances per-voice phase
//!   and ADSR envelope, sums per-voice waveforms. Never invokes
//!   the Keleusma VM.
//! - **Main thread (Keleusma)**: runs `loop main` once per tick.
//!   Each iteration calls zero or more native side-effects to
//!   update shared voice state, then yields. The host sleeps
//!   until the next tick boundary. Between tick body iterations
//!   the VM transits a `VmState::Reset`; this is the safe
//!   boundary for `Vm::replace_module`.
//! - **Stdin thread**: blocks on `read_line` and forwards user
//!   commands to the main thread through an `mpsc` channel. The
//!   string `"s"` requests a song swap; any other input quits.
//!
//! # Per-channel instrument control
//!
//! The host registers eight voice slots and starts every voice
//! disabled. The script enables and configures voices through the
//! following natives:
//!
//! - `host::set_enable(ch, on)`            — `on` is 0 / non-zero.
//! - `host::set_waveform(ch, code)`        — see waveform codes
//!   below.
//! - `host::set_duty(ch, q1000)`           — duty cycle for
//!   `Waveform::Pulse`, `q1000` in `[0, 1000]` maps to
//!   `[0.0, 1.0]`.
//! - `host::set_adsr(ch, a_ms, d_ms, s_q1000, r_ms)` — attack,
//!   decay, release in milliseconds; sustain level in `q1000`.
//! - `host::play(ch, midi)`                — set pitch and gate
//!   on; triggers an envelope attack.
//! - `host::silence(ch)`                   — gate off; triggers
//!   an envelope release.
//! - `host::set_bpm(bpm)`                  — change the tick
//!   rate. Applies on the next tick boundary, so mid-playback
//!   tempo changes are sample-accurate to one 16th.
//! - `host::set_volume(ch, l_q1000, r_q1000)` — per-speaker
//!   volume. Equal values produce a centred stereo image;
//!   unequal values position the voice in the stereo field.
//! - `host::set_vibrato(ch, rate_centihz, depth_cents)` —
//!   pitch LFO. `rate_centihz` is `100 * Hz` (so 500 means
//!   5 Hz); `depth_cents` is one hundredth of a semitone.
//!   Either parameter at zero disables the LFO with no CPU
//!   cost.
//! - `host::set_lpf(ch, cutoff_hz)` — one-pole low-pass
//!   filter. `cutoff_hz` of zero (or at or above the Nyquist
//!   limit) bypasses the filter at no CPU cost.
//! - `host::set_retrigger(ch, on)` — when set, every
//!   `host::play` retriggers the envelope from Attack even if
//!   the gate is already open. Cleared by default for legato
//!   behaviour.
//! - `host::set_detune(ch, cents)` — static pitch offset in
//!   cents. Combines multiplicatively with vibrato so the
//!   script can detune a voice for a chorused effect or
//!   pitch-bend it by automating this parameter directly. Zero
//!   is the default and is a no-op.
//! - `host::set_velocity(ch, q1000)` — per-voice loudness
//!   scalar applied after the envelope. `1000` is unity gain
//!   (the default); lower values attenuate the voice's mixed
//!   signal.
//! - `host::set_master_volume(q1000)` — global output gain
//!   applied before the soft-clip stage. `1000` is unity (the
//!   default). The mix passes through `tanh` before the
//!   stream output so peaks round off smoothly rather than
//!   hard-clipping.
//! - `host::song_name(name)` — announce the song's name to
//!   stdout. The host dedupes by tracking the last name seen,
//!   so a script that calls this every tick still prints only
//!   once per distinct name. The dedup state resets on every
//!   song swap.
//!
//! Waveform codes:
//!
//! | Code | Waveform | Notes |
//! |------|----------|-------|
//! | 0    | Square   | Fixed 50% duty. |
//! | 1    | Triangle | |
//! | 2    | Sawtooth | |
//! | 3    | Sine     | |
//! | 4    | Pulse    | Duty controlled by `host::set_duty`. |
//! | 5    | Noise    | White noise via xorshift32. |
//!
//! # Hot code swap
//!
//! Pressing `s` followed by Enter cycles to the next song in
//! `SONG_SOURCES` at the next reset boundary. All scripts share
//! the same data-segment schema; the swap reinitialises all
//! data slots to zero so the new song's `init` block runs
//! again and reconfigures the channels.
//!
//! Before loading the next module the host calls
//! `reset_voices` to restore every per-voice parameter
//! (waveform, duty, ADSR, vibrato, LPF, retrigger, per-speaker
//! volume, enable) to its default-disabled value. Scripts
//! therefore only need to set the parameters they care about;
//! they do not need to defensively turn features off in case a
//! previous song left them on. The same reset happens
//! implicitly at startup because the shared voice state is
//! initialised through `default_voices`.
//!
//! # Run
//!
//! ```text
//! cargo run --release --example piano_roll --features sdl3-example,text
//! ```
//!
//! The `text` cargo feature is required because the bundled
//! songs pass string literals (song names) to host natives.
//!
//! SDL3 builds from source through the `build-from-source-static`
//! feature on first build. CMake is required.
//!
//! Input commands (line-buffered through stdin):
//!
//! - `s` — cycle to the next song.
//! - `r` — restart the current song from its beginning.
//! - `<N>` — jump directly to song `N` (e.g. `0` or `1`).
//! - Enter alone — quit.
//!
//! # Possible enhancements left as an exercise for the reader
//!
//! The current example demonstrates the breadth of a tracker-
//! shaped host. Several features were considered and deliberately
//! left out so the code stays an example rather than a
//! product. Rough Rust-side LOC estimates are given where
//! useful.
//!
//! - Tremolo (amplitude LFO, the symmetric counterpart to
//!   vibrato). ~35 LOC.
//! - Filter envelope (a second ADSR driving the LPF cutoff,
//!   not just the amplitude). ~80 LOC.
//! - Delay line (per-voice or master, with feedback). ~50
//!   LOC.
//! - Reverb (Schroeder or Freeverb topology). ~150 LOC for a
//!   credible implementation.
//! - Arpeggio (rapid cycling through a tuple of pitch
//!   offsets at the sample rate). ~20 LOC.
//! - Polyphonic voice allocation (auto-stealing pool with
//!   per-note voice assignment). ~150 LOC.
//! - Sample playback (PCM table plus per-voice cursor with
//!   rate scaling for pitch). ~150 LOC plus a sample-loader
//!   entry point.
//! - FM synthesis (two-operator with carrier and modulator).
//!   ~80 LOC.
//! - Wavetable synthesis (lerp between adjacent table
//!   entries). ~80 LOC.
//! - Real-time visualiser (oscilloscope or spectrum display
//!   through a second SDL3 window). ~250 LOC.
//! - MIDI file import or live MIDI input.
//!
//! # Code-style notes for the reader
//!
//! Several places in this file are deliberately verbose for
//! the sake of a first-time reader; a production codebase
//! would tighten them at the cost of immediate readability.
//! Each is its own potential exercise.
//!
//! - The native registration block contains many almost-
//!   identical `let voices_X = voices.clone(); vm.
//!   register_native_closure(...)` patterns. A small macro
//!   would compress them but trade explicit per-native bodies
//!   for a layer of indirection.
//! - `run` is roughly two hundred lines. A reader follows it
//!   linearly. Production code would factor out helpers (SDL3
//!   audio setup, the stdin watcher thread, the tick loop)
//!   so each function has a single responsibility. Stay
//!   linear here so the data-flow stays visible on one
//!   scroll.
//! - The voice state is a single `Mutex<[Voice; 8]>` snapshot
//!   per audio callback. At eight voices times forty-eight
//!   kilohertz, contention is invisible. A production engine
//!   reaching hundreds of voices would switch to per-voice
//!   atomics or a lock-free ring buffer; the lock pattern is
//!   chosen here so the data-flow is immediately obvious.
//! - All numeric parameter ranges go through a `q1000`
//!   convention (centi-percent of the unit interval) so the
//!   Keleusma side only deals in `Word`. A real host might
//!   expose floating-point natives directly once Keleusma's
//!   marshalling layer covers the relevant width.
//!
//! Press `s`, `r`, or a digit then Enter; or Enter alone to quit.

use std::io::{self, BufRead};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::{Arena, Module, Value};

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream};

// ---------------------------------------------------------------
// Top-level constants.
// ---------------------------------------------------------------

const SAMPLE_RATE: u32 = 48_000;
const DEFAULT_BPM: u32 = 120;
const NUM_VOICES: usize = 8;

/// Microseconds per 16th-note tick at the given BPM. Four
/// sixteenths per beat; sixty seconds per minute.
const fn tick_us_for_bpm(bpm: u32) -> u64 {
    60_000_000u64 / (bpm as u64 * 4)
}

// Data-segment layout. The host treats the data segment
// opaquely; the semantics are owned by the script. The layout
// below documents the convention every bundled song follows so
// that the slot count, ordering, and reset behaviour are
// consistent across the roster.
//
//   slot 0      `init`          one-shot setup gate
//   slot 1      `loop_count`    bumped by the script when its
//                               progression wraps; lets a song
//                               distinguish first-loop intro
//                               material from subsequent loops,
//                               schedule fade-outs, or transpose
//                               on each repeat
//   slot 2      `section`       current-section pointer
//                               (intro = 0, verse = 1, ...);
//                               subsumes the intro-flag use case
//   slots 3..6  `user0..3`      song-defined general-purpose
//                               slots (transposition offset,
//                               random seed, mute mask, fill
//                               pattern selector, ...)
//   slots 7..14 `idx: [Word; 8]` per-channel position counters
//                                for the full eight-voice
//                                channel count, addressable
//                                from the script through
//                                `state.idx[ch]`
//   slots 15..22 `rem: [Word; 8]` per-channel remaining-ticks
//                                counters paired with the
//                                position counters above,
//                                addressable through
//                                `state.rem[ch]`
//
// `fresh_data` zeros every slot before each `replace_module`
// and `init_data` does the same at startup, so the new song's
// `init` block always sees a clean slate and unused fields stay
// at zero.
const NUM_DATA_SLOTS: usize = 23;

// ---------------------------------------------------------------
// Waveform types.
// ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Waveform {
    Square,
    Triangle,
    Sawtooth,
    Sine,
    Pulse,
    Noise,
}

impl Waveform {
    fn from_code(code: i64) -> Option<Self> {
        Some(match code {
            0 => Waveform::Square,
            1 => Waveform::Triangle,
            2 => Waveform::Sawtooth,
            3 => Waveform::Sine,
            4 => Waveform::Pulse,
            5 => Waveform::Noise,
            _ => return None,
        })
    }
}

// ---------------------------------------------------------------
// Voice state shared between main thread and audio thread.
// ---------------------------------------------------------------

#[derive(Clone, Copy)]
struct Voice {
    enabled: bool,
    gate: bool,
    freq: f32,
    waveform: Waveform,
    duty: f32,
    /// Per-speaker volume. The stereo output sums each voice's
    /// signal weighted by `volume_left` into the left channel and
    /// by `volume_right` into the right channel. A voice positioned
    /// dead-centre uses equal values; hard-panned voices use the
    /// other extreme set to zero.
    volume_left: f32,
    volume_right: f32,
    attack_secs: f32,
    decay_secs: f32,
    sustain_level: f32,
    release_secs: f32,
    /// Vibrato rate in Hertz. Zero disables vibrato (the audio
    /// thread fast-paths and skips the LFO advance and the pitch
    /// multiplier).
    vibrato_rate_hz: f32,
    /// Vibrato depth in cents (one hundredth of a semitone). The
    /// per-sample pitch multiplier is
    /// `2^((depth / 1200) * sin(2π * rate * t))`. Zero disables.
    vibrato_depth_cents: f32,
    /// One-pole low-pass cutoff in Hertz. Zero bypasses the filter
    /// (the audio thread fast-paths and passes the raw waveform
    /// through). Frequencies at or above the Nyquist limit also
    /// effectively bypass.
    lpf_cutoff_hz: f32,
    /// When set, every `host::play` call retriggers the envelope
    /// from the Attack stage even if the gate is already open.
    /// When clear, consecutive `host::play` calls sustain the
    /// envelope (legato).
    retrigger: bool,
    /// Increments on every retriggering `host::play` call. The
    /// audio thread snapshots `last_trigger_seq` per voice and
    /// resets the envelope on observed change.
    trigger_seq: u32,
    /// Static pitch offset in cents (one hundredth of a semitone)
    /// applied on top of the MIDI pitch. Combines multiplicatively
    /// with the vibrato LFO, so the script can detune a voice for
    /// a chorused effect or pitch-bend it by automating this
    /// parameter directly. Zero is the default and is a no-op.
    detune_cents: f32,
    /// Per-note loudness scalar applied to the mixed signal after
    /// the envelope. `host::set_velocity` writes this; `host::play`
    /// reads it implicitly through the audio path. The convention
    /// is `0.0` to `1.0` where `1.0` is the default unattenuated
    /// loudness.
    velocity: f32,
}

impl Voice {
    const fn silent_default(volume_left: f32, volume_right: f32) -> Self {
        Self {
            enabled: false,
            gate: false,
            freq: 0.0,
            waveform: Waveform::Square,
            duty: 0.5,
            volume_left,
            volume_right,
            attack_secs: 0.005,
            decay_secs: 0.080,
            sustain_level: 0.70,
            release_secs: 0.150,
            vibrato_rate_hz: 0.0,
            vibrato_depth_cents: 0.0,
            lpf_cutoff_hz: 0.0,
            retrigger: false,
            trigger_seq: 0,
            detune_cents: 0.0,
            velocity: 1.0,
        }
    }
}

// Per-channel default per-speaker volume. The first three entries
// preserve the V0.1 three-voice mix as a centred stereo image
// (equal L/R). The remaining entries hold conservative defaults
// for voices that default to disabled.
const DEFAULT_VOLUMES: [(f32, f32); NUM_VOICES] = [
    (0.22, 0.22),
    (0.18, 0.18),
    (0.18, 0.18),
    (0.15, 0.15),
    (0.15, 0.15),
    (0.15, 0.15),
    (0.15, 0.15),
    (0.15, 0.15),
];

fn default_voices() -> [Voice; NUM_VOICES] {
    let mut voices = [Voice::silent_default(0.0, 0.0); NUM_VOICES];
    for (i, slot) in voices.iter_mut().enumerate() {
        let (l, r) = DEFAULT_VOLUMES[i];
        *slot = Voice::silent_default(l, r);
    }
    voices
}

type SharedVoices = Arc<Mutex<[Voice; NUM_VOICES]>>;

// ---------------------------------------------------------------
// Audio-thread-private envelope state.
// ---------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum EnvStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy)]
struct EnvState {
    stage: EnvStage,
    level: f32,
    time_in_stage: f32,
    last_gate: bool,
    release_start_level: f32,
    /// Last `trigger_seq` value observed for this voice. The
    /// audio thread compares the live `voice.trigger_seq` against
    /// this snapshot and forces an Attack on observed change.
    last_trigger_seq: u32,
}

impl EnvState {
    const fn idle() -> Self {
        Self {
            stage: EnvStage::Idle,
            level: 0.0,
            time_in_stage: 0.0,
            last_gate: false,
            release_start_level: 0.0,
            last_trigger_seq: 0,
        }
    }
}

// ---------------------------------------------------------------
// Audio callback. Sums all voices through their per-channel
// waveform and ADSR envelope. Phase, envelope, and noise PRNG
// state are owned by the audio thread; only `Voice` (script-
// controlled parameters) crosses the lock.
// ---------------------------------------------------------------

struct Mixer {
    voices: SharedVoices,
    phases: [f32; NUM_VOICES],
    /// Vibrato LFO phase per voice. Advances at `vibrato_rate_hz`
    /// regardless of whether `vibrato_depth_cents` is zero, so
    /// turning vibrato on does not cause an audible phase pop.
    vibrato_phases: [f32; NUM_VOICES],
    /// One-pole low-pass filter state per voice. Holds the last
    /// output sample so the filter recurrence
    /// `y = y + alpha * (x - y)` continues across callbacks.
    lpf_states: [f32; NUM_VOICES],
    envs: [EnvState; NUM_VOICES],
    noise_state: u32,
    sample_rate: f32,
    buffer: Vec<f32>,
    /// Master output gain, `q1000` in `[0, 1000]` mapping to
    /// `[0.0, 1.0]`. Read by the audio thread on every callback.
    /// Updated by `host::set_master_volume`.
    master_volume: Arc<AtomicU32>,
}

impl AudioCallback<f32> for Mixer {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        self.buffer.resize(requested as usize, 0.0);
        let snapshot = *self.voices.lock().unwrap();
        let dt = 1.0 / self.sample_rate;
        let nyquist = self.sample_rate * 0.5;

        // The output stream is two-channel interleaved L/R. Each
        // frame is a `(left, right)` pair so the buffer is iterated
        // in chunks of two slots per frame.
        for frame in self.buffer.chunks_exact_mut(2) {
            let mut acc_l = 0.0f32;
            let mut acc_r = 0.0f32;
            for (ch, &v) in snapshot.iter().enumerate() {
                let env = &mut self.envs[ch];

                if !v.enabled {
                    *env = EnvState::idle();
                    self.lpf_states[ch] = 0.0;
                    continue;
                }

                // Retrigger: force a fresh Attack on every
                // observed `trigger_seq` change. Suppresses the
                // gate-on edge that would otherwise also fire.
                if v.trigger_seq != env.last_trigger_seq {
                    env.stage = EnvStage::Attack;
                    env.time_in_stage = 0.0;
                    env.last_trigger_seq = v.trigger_seq;
                    env.last_gate = v.gate;
                }

                // Edge detection on gate, then advance the
                // envelope state machine.
                if v.gate && !env.last_gate {
                    env.stage = EnvStage::Attack;
                    env.time_in_stage = 0.0;
                } else if !v.gate && env.last_gate {
                    env.stage = EnvStage::Release;
                    env.time_in_stage = 0.0;
                    env.release_start_level = env.level;
                }
                env.last_gate = v.gate;

                advance_envelope(env, &v, dt);

                if env.stage == EnvStage::Idle || v.freq <= 0.0 {
                    continue;
                }

                // Vibrato. Always advance the LFO phase so toggling
                // vibrato on does not jump. The pitch multiplier
                // collapses to 1.0 when depth is zero, which the
                // fast path below skips.
                self.vibrato_phases[ch] = (self.vibrato_phases[ch] + v.vibrato_rate_hz * dt) % 1.0;
                let vibrato_mul = if v.vibrato_depth_cents > 0.0 && v.vibrato_rate_hz > 0.0 {
                    let lfo = libm::sinf(self.vibrato_phases[ch] * core::f32::consts::TAU);
                    libm::powf(2.0, (v.vibrato_depth_cents / 1200.0) * lfo)
                } else {
                    1.0
                };

                // Static detune. Fast-path zero cents to avoid the
                // powf call when the voice has no detune set.
                let detune_mul = if v.detune_cents != 0.0 {
                    libm::powf(2.0, v.detune_cents / 1200.0)
                } else {
                    1.0
                };
                let pitch_mul = vibrato_mul * detune_mul;

                let phase_inc = (v.freq * pitch_mul) / self.sample_rate;
                self.phases[ch] = (self.phases[ch] + phase_inc) % 1.0;
                let raw_sample =
                    waveform_sample(v.waveform, self.phases[ch], v.duty, &mut self.noise_state);

                // One-pole low-pass filter. Bypass when cutoff is
                // zero or at/above Nyquist; otherwise advance the
                // filter state in place.
                let filtered = if v.lpf_cutoff_hz > 0.0 && v.lpf_cutoff_hz < nyquist {
                    let alpha = 1.0
                        - libm::expf(-core::f32::consts::TAU * v.lpf_cutoff_hz / self.sample_rate);
                    self.lpf_states[ch] += alpha * (raw_sample - self.lpf_states[ch]);
                    self.lpf_states[ch]
                } else {
                    // Reset state when bypassed so re-engaging the
                    // filter does not pop from a stale value.
                    self.lpf_states[ch] = 0.0;
                    raw_sample
                };

                let weighted = env.level * v.velocity * filtered;
                acc_l += v.volume_left * weighted;
                acc_r += v.volume_right * weighted;
            }
            // Master volume and soft clip. The `tanh` curve is a
            // smooth saturation around +/-1.0 so peaks that
            // exceed the linear range round off rather than
            // clipping into hard edges.
            let master = (self.master_volume.load(Ordering::Relaxed) as f32) / 1000.0;
            frame[0] = libm::tanhf(acc_l * master);
            frame[1] = libm::tanhf(acc_r * master);
        }

        stream.put_data_f32(&self.buffer).unwrap();
    }
}

fn advance_envelope(env: &mut EnvState, v: &Voice, dt: f32) {
    match env.stage {
        EnvStage::Idle => {
            env.level = 0.0;
        }
        EnvStage::Attack => {
            if v.attack_secs <= 0.0 {
                env.level = 1.0;
                env.stage = EnvStage::Decay;
                env.time_in_stage = 0.0;
            } else {
                env.level = (env.time_in_stage / v.attack_secs).min(1.0);
                if env.level >= 1.0 {
                    env.level = 1.0;
                    env.stage = EnvStage::Decay;
                    env.time_in_stage = 0.0;
                }
            }
        }
        EnvStage::Decay => {
            if v.decay_secs <= 0.0 {
                env.level = v.sustain_level;
                env.stage = EnvStage::Sustain;
                env.time_in_stage = 0.0;
            } else {
                let t = (env.time_in_stage / v.decay_secs).min(1.0);
                env.level = 1.0 - (1.0 - v.sustain_level) * t;
                if t >= 1.0 {
                    env.level = v.sustain_level;
                    env.stage = EnvStage::Sustain;
                    env.time_in_stage = 0.0;
                }
            }
        }
        EnvStage::Sustain => {
            env.level = v.sustain_level;
        }
        EnvStage::Release => {
            if v.release_secs <= 0.0 {
                env.level = 0.0;
                env.stage = EnvStage::Idle;
            } else {
                let t = (env.time_in_stage / v.release_secs).min(1.0);
                env.level = env.release_start_level * (1.0 - t);
                if t >= 1.0 {
                    env.level = 0.0;
                    env.stage = EnvStage::Idle;
                }
            }
        }
    }
    env.time_in_stage += dt;
}

fn waveform_sample(kind: Waveform, phase: f32, duty: f32, noise_state: &mut u32) -> f32 {
    match kind {
        Waveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Pulse => {
            if phase < duty {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Triangle => {
            if phase < 0.5 {
                4.0 * phase - 1.0
            } else {
                3.0 - 4.0 * phase
            }
        }
        Waveform::Sawtooth => 2.0 * phase - 1.0,
        Waveform::Sine => libm::sinf(phase * core::f32::consts::TAU),
        Waveform::Noise => {
            let r = xorshift32(noise_state);
            (r as f32 / u32::MAX as f32) * 2.0 - 1.0
        }
    }
}

fn xorshift32(state: &mut u32) -> u32 {
    let mut x = *state;
    if x == 0 {
        x = 0x9E37_79B9;
    }
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

fn midi_to_freq_hz(midi: i64) -> f32 {
    if midi < 0 {
        return 0.0;
    }
    440.0 * libm::powf(2.0, (midi as f32 - 69.0) / 12.0)
}

// ---------------------------------------------------------------
// Stdin command channel.
// ---------------------------------------------------------------

enum Command {
    /// Cycle forward to the next song in `SONG_SOURCES`.
    Swap,
    /// Reload the current song from the start.
    Restart,
    /// Jump directly to a specific song by index. Out-of-range
    /// indices are reported and ignored.
    SelectSong(usize),
    /// Stop the host loop.
    Quit,
}

// ---------------------------------------------------------------
// Entry point.
// ---------------------------------------------------------------

// The song roster. Each entry is the source text of one
// `piano_roll_<song>.kel` file. To add song N: drop a new
// `piano_roll_<N>.kel` next to this file and append the
// matching `include_str!` here. The swap action cycles through
// songs in this order, so the song-number indexing in the on-
// screen log matches the index into this slice.
const SONG_SOURCES: &[&str] = &[
    include_str!("piano_roll_0.kel"),
    include_str!("piano_roll_1.kel"),
    include_str!("piano_roll_2.kel"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Application chrome lives here. A real host might parse
    // command-line arguments, set up logging, choose which
    // song roster to load, configure an arena size, or route
    // stdin and stdout differently. The current example takes
    // no arguments, so `main` only delegates.
    //
    // To embed an adapted piano-roll loop into a larger
    // application (a game's audio subsystem, a music editor,
    // a livecoding shell), copy `run` and the helpers it
    // depends on into your host code and call it from wherever
    // your main loop wants to drive the Keleusma VM. The
    // boundary between `main` and `run` is the boundary
    // between application chrome and the embeddable host
    // loop.
    run()
}

/// Run the piano-roll host loop. Builds the song roster,
/// constructs the VM and the SDL3 audio device, registers the
/// host natives, and drives the tick-and-yield loop until the
/// stdin watcher requests quit.
///
/// Returns `Ok(())` on a graceful quit and an error otherwise.
/// Callers that embed this into a larger application can
/// either copy the body wholesale into their own host loop or
/// parameterise this function (additional arguments for the
/// song roster, BPM, arena capacity, native registrations) as
/// their host requires.
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let modules: Vec<Module> = SONG_SOURCES
        .iter()
        .map(|src| build_module(src))
        .collect::<Result<Vec<_>, _>>()?;
    let mut active_song: usize = 0;

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm =
        Vm::new(modules[active_song].clone(), &arena).map_err(|e| format!("verify: {:?}", e))?;

    init_data(&mut vm)?;

    let voices: SharedVoices = Arc::new(Mutex::new(default_voices()));

    // Tick interval shared between main thread and `host::set_bpm`.
    // The script reads the current BPM by calling the native;
    // mid-playback changes apply on the next tick boundary.
    let tick_us: Arc<AtomicU64> = Arc::new(AtomicU64::new(tick_us_for_bpm(DEFAULT_BPM)));

    // Master output gain shared between the script (via
    // `host::set_master_volume`) and the audio thread's Mixer.
    // The default of `1000` corresponds to unity gain.
    let master_volume: Arc<AtomicU32> = Arc::new(AtomicU32::new(1000));

    // Last song name announced through `host::song_name`. The
    // native dedupes by comparing against this value so a
    // script that calls the native every tick still prints
    // only once per distinct name. Reset to `None` on song
    // swap so the next song's first call prints unconditionally.
    let last_song_name: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    register_natives(&mut vm, &voices, &tick_us, &master_volume, &last_song_name);

    let sdl_context = sdl3::init()?;
    let audio_subsystem = sdl_context.audio()?;
    let desired_spec = AudioSpec {
        freq: Some(SAMPLE_RATE as i32),
        channels: Some(2),
        format: Some(AudioFormat::f32_sys()),
    };
    let device = audio_subsystem.open_playback_stream(
        &desired_spec,
        Mixer {
            voices: voices.clone(),
            phases: [0.0; NUM_VOICES],
            vibrato_phases: [0.0; NUM_VOICES],
            lpf_states: [0.0; NUM_VOICES],
            envs: [EnvState::idle(); NUM_VOICES],
            noise_state: 0x9E37_79B9,
            sample_rate: SAMPLE_RATE as f32,
            buffer: Vec::new(),
            master_volume: master_volume.clone(),
        },
    )?;
    device.resume()?;

    let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut buf = String::new();
        loop {
            buf.clear();
            if stdin.lock().read_line(&mut buf).is_err() {
                let _ = cmd_tx.send(Command::Quit);
                return;
            }
            let line = buf.trim();
            let cmd = if line.eq_ignore_ascii_case("s") {
                Command::Swap
            } else if line.eq_ignore_ascii_case("r") {
                Command::Restart
            } else if let Ok(n) = line.parse::<usize>() {
                Command::SelectSong(n)
            } else {
                Command::Quit
            };
            let is_quit = matches!(cmd, Command::Quit);
            if cmd_tx.send(cmd).is_err() {
                return;
            }
            if is_quit {
                return;
            }
        }
    });

    println!(
        "Keleusma piano roll. 120 BPM, eight voices, three active. {} song(s) loaded.",
        modules.len()
    );
    println!("[ Commands: s=next, r=restart, <N>=select song N, Enter=quit ]");
    println!("[ now playing song 0 ]");

    match vm
        .call(&[Value::Int(0)])
        .map_err(|e| format!("vm call: {:?}", e))?
    {
        VmState::Yielded(_) => {}
        s => return Err(format!("script did not yield on first call: {:?}", s).into()),
    }

    let mut next_tick = Instant::now() + Duration::from_micros(tick_us.load(Ordering::Relaxed));
    let mut tick: i64 = 1;
    // Pending song change resolved at the next Reset boundary.
    // `Some(target)` means "load song `target` on the next
    // Reset"; `target == active_song` means restart in place.
    let mut pending_song_change: Option<usize> = None;

    loop {
        match cmd_rx.try_recv() {
            Ok(Command::Quit) => break,
            Ok(Command::Swap) => {
                pending_song_change = Some((active_song + 1) % modules.len());
            }
            Ok(Command::Restart) => {
                pending_song_change = Some(active_song);
            }
            Ok(Command::SelectSong(n)) => {
                if n < modules.len() {
                    pending_song_change = Some(n);
                } else {
                    println!(
                        "[ song {} is out of range; roster has {} song(s) ]",
                        n,
                        modules.len()
                    );
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        let now = Instant::now();
        if now < next_tick {
            thread::sleep(next_tick - now);
        }
        // Reload the interval each tick so `host::set_bpm` calls
        // from the script take effect on the very next tick.
        next_tick += Duration::from_micros(tick_us.load(Ordering::Relaxed));

        loop {
            let state = vm
                .resume(Value::Int(tick))
                .map_err(|e| format!("vm resume: {:?}", e))?;
            match state {
                VmState::Yielded(_) => break,
                VmState::Reset => {
                    if let Some(target) = pending_song_change.take() {
                        let restart_in_place = target == active_song;
                        active_song = target;
                        // Reset host-side voice state BEFORE loading
                        // the next module. The new song's init block
                        // is responsible only for the parameters it
                        // cares about; every other field (vibrato,
                        // LPF, retrigger, ADSR, waveform, duty, per-
                        // speaker volume, detune, velocity) is
                        // restored to the off / default value by
                        // `reset_voices`. Scripts therefore never
                        // need to defensively turn features off "in
                        // case" a previous song turned them on. The
                        // last-printed song name is also cleared so
                        // the incoming song's `host::song_name` call
                        // (if any) announces unconditionally.
                        reset_voices(&voices);
                        *last_song_name.lock().unwrap() = None;
                        vm.replace_module(modules[active_song].clone(), fresh_data())
                            .map_err(|e| format!("replace_module: {:?}", e))?;
                        let label = if restart_in_place {
                            "restarted"
                        } else {
                            "swapped to"
                        };
                        println!("[ {} song {} ]", label, active_song);
                        match vm
                            .call(&[Value::Int(tick)])
                            .map_err(|e| format!("vm call after swap: {:?}", e))?
                        {
                            VmState::Yielded(_) => break,
                            other => {
                                return Err(
                                    format!("swapped module did not yield: {:?}", other).into()
                                );
                            }
                        }
                    }
                    continue;
                }
                other => {
                    return Err(format!("unexpected vm state: {:?}", other).into());
                }
            }
        }
        tick = tick.wrapping_add(1);
    }

    reset_voices(&voices);
    device.pause()?;
    println!("bye.");
    Ok(())
}

fn build_module(src: &str) -> Result<Module, Box<dyn std::error::Error>> {
    let tokens = tokenize(src).map_err(|e| format!("lex: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("parse: {:?}", e))?;
    Ok(compile(&program).map_err(|e| format!("compile: {:?}", e))?)
}

fn init_data(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    for slot in 0..NUM_DATA_SLOTS {
        vm.set_data(slot, Value::Int(0))
            .map_err(|e| format!("set_data: {:?}", e))?;
    }
    Ok(())
}

fn fresh_data() -> Vec<Value> {
    (0..NUM_DATA_SLOTS).map(|_| Value::Int(0)).collect()
}

fn reset_voices(voices: &SharedVoices) {
    let mut v = voices.lock().unwrap();
    *v = default_voices();
}

fn register_natives(
    vm: &mut Vm,
    voices: &SharedVoices,
    tick_us: &Arc<AtomicU64>,
    master_volume: &Arc<AtomicU32>,
    last_song_name: &Arc<Mutex<Option<String>>>,
) {
    let tick_us_for_bpm = tick_us.clone();
    vm.register_native_closure(
        "host::set_bpm",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let bpm = as_i64(&args[0])?;
            if bpm <= 0 {
                return Err(VmError::NativeError(format!(
                    "host::set_bpm expected positive BPM, got {}",
                    bpm
                )));
            }
            let new_us = 60_000_000u64 / (bpm as u64 * 4);
            tick_us_for_bpm.store(new_us, Ordering::Relaxed);
            Ok(Value::Unit)
        }),
    );

    let voices_play = voices.clone();
    vm.register_native_closure(
        "host::play",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let midi = as_i64(&args[1])?;
            if ch < NUM_VOICES {
                let mut v = voices_play.lock().unwrap();
                v[ch].freq = midi_to_freq_hz(midi);
                v[ch].gate = midi >= 0;
                // When the script has opted into retrigger for
                // this voice, bump the trigger sequence so the
                // audio thread restarts the envelope at Attack
                // even if the gate was already open.
                if v[ch].retrigger && midi >= 0 {
                    v[ch].trigger_seq = v[ch].trigger_seq.wrapping_add(1);
                }
            }
            Ok(Value::Unit)
        }),
    );

    let voices_silence = voices.clone();
    vm.register_native_closure(
        "host::silence",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            if ch < NUM_VOICES {
                let mut v = voices_silence.lock().unwrap();
                v[ch].gate = false;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_enable = voices.clone();
    vm.register_native_closure(
        "host::set_enable",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let on = as_i64(&args[1])? != 0;
            if ch < NUM_VOICES {
                let mut v = voices_enable.lock().unwrap();
                v[ch].enabled = on;
                if !on {
                    v[ch].gate = false;
                }
            }
            Ok(Value::Unit)
        }),
    );

    let voices_waveform = voices.clone();
    vm.register_native_closure(
        "host::set_waveform",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let code = as_i64(&args[1])?;
            let kind = Waveform::from_code(code)
                .ok_or_else(|| VmError::NativeError(format!("unknown waveform code {}", code)))?;
            if ch < NUM_VOICES {
                voices_waveform.lock().unwrap()[ch].waveform = kind;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_duty = voices.clone();
    vm.register_native_closure(
        "host::set_duty",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let q1000 = as_i64(&args[1])?;
            if ch < NUM_VOICES {
                voices_duty.lock().unwrap()[ch].duty = (q1000.clamp(0, 1000) as f32) / 1000.0;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_adsr = voices.clone();
    vm.register_native_closure(
        "host::set_adsr",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let a_ms = as_i64(&args[1])?.max(0) as f32;
            let d_ms = as_i64(&args[2])?.max(0) as f32;
            let s_q1000 = as_i64(&args[3])?.clamp(0, 1000) as f32;
            let r_ms = as_i64(&args[4])?.max(0) as f32;
            if ch < NUM_VOICES {
                let mut v = voices_adsr.lock().unwrap();
                v[ch].attack_secs = a_ms / 1000.0;
                v[ch].decay_secs = d_ms / 1000.0;
                v[ch].sustain_level = s_q1000 / 1000.0;
                v[ch].release_secs = r_ms / 1000.0;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_volume = voices.clone();
    vm.register_native_closure(
        "host::set_volume",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let left = as_i64(&args[1])?.clamp(0, 1000) as f32 / 1000.0;
            let right = as_i64(&args[2])?.clamp(0, 1000) as f32 / 1000.0;
            if ch < NUM_VOICES {
                let mut v = voices_volume.lock().unwrap();
                v[ch].volume_left = left;
                v[ch].volume_right = right;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_vibrato = voices.clone();
    vm.register_native_closure(
        "host::set_vibrato",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            // Rate is in centi-Hertz (1 = 0.01 Hz). Five cycles per
            // second is therefore 500. Depth is in cents directly
            // (0..1200 typical, where 1200 cents = one octave).
            let ch = as_i64(&args[0])? as usize;
            let rate_centihz = as_i64(&args[1])?.max(0) as f32;
            let depth_cents = as_i64(&args[2])?.max(0) as f32;
            if ch < NUM_VOICES {
                let mut v = voices_vibrato.lock().unwrap();
                v[ch].vibrato_rate_hz = rate_centihz / 100.0;
                v[ch].vibrato_depth_cents = depth_cents;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_lpf = voices.clone();
    vm.register_native_closure(
        "host::set_lpf",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let cutoff_hz = as_i64(&args[1])?.max(0) as f32;
            if ch < NUM_VOICES {
                voices_lpf.lock().unwrap()[ch].lpf_cutoff_hz = cutoff_hz;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_retrigger = voices.clone();
    vm.register_native_closure(
        "host::set_retrigger",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let on = as_i64(&args[1])? != 0;
            if ch < NUM_VOICES {
                voices_retrigger.lock().unwrap()[ch].retrigger = on;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_detune = voices.clone();
    vm.register_native_closure(
        "host::set_detune",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let cents = as_i64(&args[1])? as f32;
            if ch < NUM_VOICES {
                voices_detune.lock().unwrap()[ch].detune_cents = cents;
            }
            Ok(Value::Unit)
        }),
    );

    let voices_velocity = voices.clone();
    vm.register_native_closure(
        "host::set_velocity",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let ch = as_i64(&args[0])? as usize;
            let q1000 = as_i64(&args[1])?.clamp(0, 1000) as f32;
            if ch < NUM_VOICES {
                voices_velocity.lock().unwrap()[ch].velocity = q1000 / 1000.0;
            }
            Ok(Value::Unit)
        }),
    );

    let master = master_volume.clone();
    vm.register_native_closure(
        "host::set_master_volume",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let q1000 = as_i64(&args[0])?.clamp(0, 1000) as u32;
            master.store(q1000, Ordering::Relaxed);
            Ok(Value::Unit)
        }),
    );

    // Print the song's name to stdout, but only when the
    // argument differs from the last name printed. The host
    // clears the tracked name on every song swap, so the next
    // song's first call always prints. Calls that pass the
    // same name (typically because the script reuses the name
    // literal in code that runs every tick) are silently
    // skipped.
    let name_state = last_song_name.clone();
    vm.register_native_closure(
        "host::song_name",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            let name = args[0].as_str().ok_or_else(|| {
                VmError::TypeError(format!("host::song_name expected Text, got {:?}", args[0]))
            })?;
            let mut last = name_state.lock().unwrap();
            if last.as_deref() != Some(name) {
                println!("[ song name: {} ]", name);
                *last = Some(name.to_string());
            }
            Ok(Value::Unit)
        }),
    );
}

fn as_i64(v: &Value) -> Result<i64, VmError> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(VmError::TypeError(format!("expected Int, got {:?}", other))),
    }
}
