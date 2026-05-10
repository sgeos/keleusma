//! Three-channel SDL3 audio piano roll driven by a Keleusma tick
//! control loop.
//!
//! # Architecture
//!
//! The Keleusma script `piano_roll.kel` runs on the main thread at
//! one yield per 16th-note tick (125 ms at 120 BPM). At each tick
//! the script emits `host::play(channel, midi)` or
//! `host::silence(channel)` native calls. These natives update
//! voice state shared with the SDL3 audio callback, which renders
//! samples on the audio thread.
//!
//! - **Audio thread (SDL3 callback)**: receives a sample buffer to
//!   fill at sample rate (48 kHz), reads the current voice state
//!   from a `Mutex<[Voice; 3]>`, advances per-voice phase, sums
//!   per-voice waveforms. Never invokes the Keleusma VM.
//! - **Main thread (Keleusma)**: runs `loop main` once per tick.
//!   Each iteration calls zero or more native side-effects to
//!   update shared voice state, then yields. The host sleeps
//!   until the next tick boundary.
//! - **Stdin thread**: blocks on `read_line` and flips an
//!   `AtomicBool` to signal quit on Enter.
//!
//! # Song
//!
//! Four-bar progression in C major: `C - Am - F - G` (I-vi-IV-V),
//! sixty-four 16th-note ticks total, auto-looping. Channel 0 is a
//! square-wave melody, channel 1 a triangle-wave bass, channel 2
//! a square-wave harmony.
//!
//! # Editing
//!
//! Per-channel instrument parameters (waveform, volume) are
//! constants near the top of this file. Song notes are in
//! `examples/piano_roll.kel` as match-on-index functions returning
//! `(Pitch, octave, duration_in_16ths)` tuples.
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
//! Press Enter in the terminal to quit.

use std::io::{self, BufRead};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::{Arena, Value};

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream};

// ---------------------------------------------------------------
// Tunable instrument parameters. Edit these to change the sound
// without touching `piano_roll.kel`.
// ---------------------------------------------------------------

const SAMPLE_RATE: u32 = 48_000;
const TICK_MS: u64 = 125; // 16th note at 120 BPM
const NUM_VOICES: usize = 3;

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum Waveform {
    Square,
    Triangle,
    Sawtooth,
    Sine,
}

// Channel 0 (melody), 1 (bass), 2 (harmony).
const WAVEFORMS: [Waveform; NUM_VOICES] = [
    Waveform::Square,   // melody
    Waveform::Triangle, // bass
    Waveform::Square,   // harmony
];

// Per-channel mix amplitude. Sum should stay below 1.0 to avoid
// clipping when all three channels are simultaneously active.
const VOLUMES: [f32; NUM_VOICES] = [0.22, 0.18, 0.18];

// ---------------------------------------------------------------
// Voice state shared between main thread and audio thread.
// ---------------------------------------------------------------

#[derive(Clone, Copy)]
struct Voice {
    freq: f32,
    gate: bool,
}

impl Voice {
    const fn silent() -> Self {
        Self {
            freq: 0.0,
            gate: false,
        }
    }
}

type SharedVoices = Arc<Mutex<[Voice; NUM_VOICES]>>;

// ---------------------------------------------------------------
// Audio callback. Sums three voices into the output buffer using
// the per-channel waveform table. Phase state is owned by the
// audio thread; only `Voice` (freq, gate) crosses the lock.
// ---------------------------------------------------------------

struct Mixer {
    voices: SharedVoices,
    phases: [f32; NUM_VOICES],
    sample_rate: f32,
    buffer: Vec<f32>,
}

impl AudioCallback<f32> for Mixer {
    fn callback(&mut self, stream: &mut AudioStream, requested: i32) {
        self.buffer.resize(requested as usize, 0.0);
        let snapshot = *self.voices.lock().unwrap();

        for sample in self.buffer.iter_mut() {
            let mut acc = 0.0f32;
            for ch in 0..NUM_VOICES {
                let v = snapshot[ch];
                if !v.gate || v.freq <= 0.0 {
                    continue;
                }
                let phase_inc = v.freq / self.sample_rate;
                self.phases[ch] = (self.phases[ch] + phase_inc) % 1.0;
                acc += VOLUMES[ch] * waveform(WAVEFORMS[ch], self.phases[ch]);
            }
            *sample = acc;
        }

        stream.put_data_f32(&self.buffer).unwrap();
    }
}

fn waveform(kind: Waveform, phase: f32) -> f32 {
    match kind {
        Waveform::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        Waveform::Triangle => {
            // -1 at phase 0, +1 at phase 0.5, -1 at phase 1.
            if phase < 0.5 {
                4.0 * phase - 1.0
            } else {
                3.0 - 4.0 * phase
            }
        }
        Waveform::Sawtooth => 2.0 * phase - 1.0,
        Waveform::Sine => libm::sinf(phase * core::f32::consts::TAU),
    }
}

fn midi_to_freq_hz(midi: i64) -> f32 {
    if midi < 0 {
        return 0.0;
    }
    440.0 * libm::powf(2.0, (midi as f32 - 69.0) / 12.0)
}

// ---------------------------------------------------------------
// Entry point.
// ---------------------------------------------------------------

const SCRIPT: &str = include_str!("piano_roll.kel");

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Compile the Keleusma script.
    let tokens = tokenize(SCRIPT).map_err(|e| format!("lex: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("parse: {:?}", e))?;
    let module = compile(&program).map_err(|e| format!("compile: {:?}", e))?;

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("verify: {:?}", e))?;

    // Initialize all six data slots to zero.
    for slot in 0..6 {
        vm.set_data(slot, Value::Int(0))
            .map_err(|e| format!("set_data: {:?}", e))?;
    }

    // Shared voice state.
    let voices: SharedVoices = Arc::new(Mutex::new([Voice::silent(); NUM_VOICES]));

    // Register host natives that update the shared voices.
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

    // Initialise SDL3 audio.
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
            sample_rate: SAMPLE_RATE as f32,
            buffer: Vec::new(),
        },
    )?;
    device.resume()?;

    // Stdin watcher: any line of input flips the quit flag.
    let quit = Arc::new(AtomicBool::new(false));
    let quit_stdin = quit.clone();
    thread::spawn(move || {
        let stdin = io::stdin();
        let mut buf = String::new();
        let _ = stdin.lock().read_line(&mut buf);
        quit_stdin.store(true, Ordering::Relaxed);
    });

    println!("Keleusma piano roll. 120 BPM, three voices.");
    println!("[ Press Enter to quit. ]");

    // First call to start the loop.
    match vm
        .call(&[Value::Int(0)])
        .map_err(|e| format!("vm call: {:?}", e))?
    {
        VmState::Yielded(_) => {}
        s => return Err(format!("script did not yield on first call: {:?}", s).into()),
    }

    // Tick loop. One yield boundary per 16th-note tick.
    let interval = Duration::from_millis(TICK_MS);
    let mut next_tick = Instant::now() + interval;
    let mut tick: i64 = 1;

    while !quit.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now < next_tick {
            thread::sleep(next_tick - now);
        }
        next_tick += interval;

        // Drive the script forward until the next yield.
        loop {
            match vm
                .resume(Value::Int(tick))
                .map_err(|e| format!("vm resume: {:?}", e))?
            {
                VmState::Yielded(_) => break,
                VmState::Reset => continue, // body finished; loop runs again
                s => {
                    return Err(format!("unexpected vm state: {:?}", s).into());
                }
            }
        }
        tick = tick.wrapping_add(1);
    }

    // Quiet all voices before dropping the device.
    {
        let mut v = voices.lock().unwrap();
        for slot in v.iter_mut() {
            slot.gate = false;
        }
    }
    device.pause()?;
    println!("bye.");
    Ok(())
}

fn as_i64(v: &Value) -> Result<i64, VmError> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(VmError::TypeError(format!(
            "expected Int, got {:?}",
            other
        ))),
    }
}
