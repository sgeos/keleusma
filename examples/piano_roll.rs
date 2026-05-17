//! Eight-channel SDL3 audio piano roll driven by a Keleusma tick
//! control loop, with hot code reload between two songs.
//!
//! # Architecture
//!
//! Two Keleusma scripts (`piano_roll.kel` and `piano_roll_2.kel`)
//! are precompiled at startup. The currently-active script runs on
//! the main thread at one yield per 16th-note tick (125 ms at
//! 120 BPM). At each tick the script emits per-voice setup natives
//! (waveform, duty cycle, ADSR, enable) on the first iteration and
//! `host::play(channel, midi)` / `host::silence(channel)` natives
//! on note boundaries. These natives update voice state shared
//! with the SDL3 audio callback, which renders samples on the
//! audio thread.
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
//! Pressing `s` followed by Enter swaps the currently-running
//! script for the other one at the next reset boundary. Both
//! scripts share the same data-segment schema; the swap
//! reinitialises all slots to zero so the new song's `init`
//! block runs again and reconfigures the channels.
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
const TICK_MS: u64 = 125; // 16th note at 120 BPM
const NUM_VOICES: usize = 8;

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
    volume: f32,
    attack_secs: f32,
    decay_secs: f32,
    sustain_level: f32,
    release_secs: f32,
}

impl Voice {
    const fn silent_default(volume: f32) -> Self {
        Self {
            enabled: false,
            gate: false,
            freq: 0.0,
            waveform: Waveform::Square,
            duty: 0.5,
            volume,
            attack_secs: 0.005,
            decay_secs: 0.080,
            sustain_level: 0.70,
            release_secs: 0.150,
        }
    }
}

// Per-channel default volume preserves the V0.1 three-voice mix
// for the active voices and uses a conservative value for the
// remaining (disabled) voices.
const DEFAULT_VOLUMES: [f32; NUM_VOICES] = [0.22, 0.18, 0.18, 0.15, 0.15, 0.15, 0.15, 0.15];

fn default_voices() -> [Voice; NUM_VOICES] {
    let mut voices = [Voice::silent_default(0.0); NUM_VOICES];
    for (i, slot) in voices.iter_mut().enumerate() {
        *slot = Voice::silent_default(DEFAULT_VOLUMES[i]);
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
}

impl EnvState {
    const fn idle() -> Self {
        Self {
            stage: EnvStage::Idle,
            level: 0.0,
            time_in_stage: 0.0,
            last_gate: false,
            release_start_level: 0.0,
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

        for sample in self.buffer.iter_mut() {
            let mut acc = 0.0f32;
            for (ch, &v) in snapshot.iter().enumerate() {
                let env = &mut self.envs[ch];

                if !v.enabled {
                    *env = EnvState::idle();
                    continue;
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

                let phase_inc = v.freq / self.sample_rate;
                self.phases[ch] = (self.phases[ch] + phase_inc) % 1.0;
                let sample_val =
                    waveform_sample(v.waveform, self.phases[ch], v.duty, &mut self.noise_state);
                acc += v.volume * env.level * sample_val;
            }
            *sample = acc;
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

const SCRIPT_A: &str = include_str!("piano_roll.kel");
const SCRIPT_B: &str = include_str!("piano_roll_2.kel");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let module_a = build_module(SCRIPT_A)?;
    let module_b = build_module(SCRIPT_B)?;

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module_a.clone(), &arena).map_err(|e| format!("verify: {:?}", e))?;

    init_data(&mut vm)?;

    let voices: SharedVoices = Arc::new(Mutex::new(default_voices()));

    register_natives(&mut vm, &voices);

    let sdl_context = sdl3::init()?;
    let audio_subsystem = sdl_context.audio()?;
    let desired_spec = AudioSpec {
        freq: Some(SAMPLE_RATE as i32),
        channels: Some(1),
        format: Some(AudioFormat::f32_sys()),
    };
    let device = audio_subsystem.open_playback_stream(
        &desired_spec,
        Mixer {
            voices: voices.clone(),
            phases: [0.0; NUM_VOICES],
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

    println!("Keleusma piano roll. 120 BPM, eight voices, three active.");
    println!("[ Press 's' + Enter to swap song. Press Enter alone to quit. ]");

    match vm
        .call(&[Value::Int(0)])
        .map_err(|e| format!("vm call: {:?}", e))?
    {
        VmState::Yielded(_) => {}
        s => return Err(format!("script did not yield on first call: {:?}", s).into()),
    }

    let interval = Duration::from_millis(TICK_MS);
    let mut next_tick = Instant::now() + interval;
    let mut tick: i64 = 1;
    let mut active_is_a = true;
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
        next_tick += interval;

        loop {
            let state = vm
                .resume(Value::Int(tick))
                .map_err(|e| format!("vm resume: {:?}", e))?;
            match state {
                VmState::Yielded(_) => break,
                VmState::Reset => {
                    if swap_pending {
                        let next_module = if active_is_a {
                            module_b.clone()
                        } else {
                            module_a.clone()
                        };
                        vm.replace_module(next_module, fresh_data())
                            .map_err(|e| format!("replace_module: {:?}", e))?;
                        active_is_a = !active_is_a;
                        swap_pending = false;
                        let song_index = if active_is_a { 1 } else { 2 };
                        println!("[ swapped to song {} ]", song_index);
                        // Reset voice state to default-disabled so
                        // the new song's init block decides which
                        // channels to enable and with what
                        // parameters.
                        reset_voices(&voices);
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

fn register_natives(vm: &mut Vm, voices: &SharedVoices) {
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
}

fn as_i64(v: &Value) -> Result<i64, VmError> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(VmError::TypeError(format!("expected Int, got {:?}", other))),
    }
}
