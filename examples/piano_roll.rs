//! Eight-channel SDL3 audio piano roll driven by a Keleusma tick
//! control loop, with hot code reload between two songs.
//!
//! # Architecture
//!
//! A roster of Keleusma scripts named `piano_roll_<N>.kel`
//! (currently `piano_roll_0.kel` and `piano_roll_1.kel`) is
//! precompiled at startup and registered in the `SONG_SOURCES`
//! slice in this file. The currently-active script runs on the
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
//! cargo run --release --example piano_roll --features sdl3-example
//! ```
//!
//! SDL3 builds from source through the `build-from-source-static`
//! feature on first build. CMake is required.
//!
//! Press `s` then Enter to swap songs. Press Enter alone to quit.

use std::io::{self, BufRead};
use std::sync::atomic::{AtomicU64, Ordering};
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

// Data-segment layout. The first slot is the `init` flag the
// script uses to gate the one-time channel-setup block. The
// remaining six slots are the (idx, rem) pair for each of the
// three currently-used channels.
const NUM_DATA_SLOTS: usize = 7;

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
                let pitch_mul = if v.vibrato_depth_cents > 0.0 && v.vibrato_rate_hz > 0.0 {
                    let lfo = libm::sinf(self.vibrato_phases[ch] * core::f32::consts::TAU);
                    libm::powf(2.0, (v.vibrato_depth_cents / 1200.0) * lfo)
                } else {
                    1.0
                };

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

                let weighted = env.level * filtered;
                acc_l += v.volume_left * weighted;
                acc_r += v.volume_right * weighted;
            }
            frame[0] = acc_l;
            frame[1] = acc_r;
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
    Swap,
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

    register_natives(&mut vm, &voices, &tick_us);

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
            if line.eq_ignore_ascii_case("s") {
                if cmd_tx.send(Command::Swap).is_err() {
                    return;
                }
            } else {
                let _ = cmd_tx.send(Command::Quit);
                return;
            }
        }
    });

    println!(
        "Keleusma piano roll. 120 BPM, eight voices, three active. {} song(s) loaded.",
        modules.len()
    );
    println!("[ Press 's' + Enter to swap song. Press Enter alone to quit. ]");
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
    let mut swap_pending = false;

    loop {
        match cmd_rx.try_recv() {
            Ok(Command::Quit) => break,
            Ok(Command::Swap) => {
                swap_pending = true;
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
                    if swap_pending {
                        active_song = (active_song + 1) % modules.len();
                        // Reset host-side voice state BEFORE loading
                        // the next module. The new song's init block
                        // is responsible only for the parameters it
                        // cares about; every other field (vibrato,
                        // LPF, retrigger, ADSR, waveform, duty, per-
                        // speaker volume) is restored to the off /
                        // default value by `reset_voices`. Scripts
                        // therefore never need to defensively turn
                        // features off "in case" a previous song
                        // turned them on.
                        reset_voices(&voices);
                        vm.replace_module(modules[active_song].clone(), fresh_data())
                            .map_err(|e| format!("replace_module: {:?}", e))?;
                        swap_pending = false;
                        println!("[ swapped to song {} ]", active_song);
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

fn register_natives(vm: &mut Vm, voices: &SharedVoices, tick_us: &Arc<AtomicU64>) {
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
}

fn as_i64(v: &Value) -> Result<i64, VmError> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(VmError::TypeError(format!("expected Int, got {:?}", other))),
    }
}
