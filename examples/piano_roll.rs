//! Three-channel SDL3 audio piano roll driven by a Keleusma tick
//! control loop, with hot code reload between two songs.
//!
//! # Architecture
//!
//! Two Keleusma scripts (`piano_roll.kel` and `piano_roll_2.kel`)
//! are precompiled at startup. The currently-active script runs on
//! the main thread at one yield per 16th-note tick (125 ms at
//! 120 BPM). At each tick the script emits `host::play(channel,
//! midi)` or `host::silence(channel)` native calls. These natives
//! update voice state shared with the SDL3 audio callback, which
//! renders samples on the audio thread.
//!
//! - **Audio thread (SDL3 callback)**: receives a sample buffer to
//!   fill at sample rate (48 kHz), reads the current voice state
//!   from a `Mutex<[Voice; 3]>`, advances per-voice phase, sums
//!   per-voice waveforms. Never invokes the Keleusma VM.
//! - **Main thread (Keleusma)**: runs `loop main` once per tick.
//!   Each iteration calls zero or more native side-effects to
//!   update shared voice state, then yields. The host sleeps until
//!   the next tick boundary. Between tick body iterations the VM
//!   transits a `VmState::Reset`; this is the safe boundary for
//!   `Vm::replace_module`.
//! - **Stdin thread**: blocks on `read_line` and forwards user
//!   commands to the main thread through an `mpsc` channel. The
//!   string `"s"` requests a song swap; any other input quits.
//!
//! # Hot code swap
//!
//! Pressing `s` followed by Enter swaps the currently-running
//! script for the other one at the next reset boundary. Both
//! scripts share the same data-segment schema (six `i64` slots);
//! the swap reinitialises all slots to zero so the new song
//! starts at the beginning of its phrase. The audio thread keeps
//! rendering across the swap, so any voices left gated by the
//! outgoing script continue to play until the incoming script
//! either retriggers them on a new pitch or silences them.
//!
//! Per-channel waveforms and volumes (host instrument parameters)
//! do not change across the swap. The two songs route different
//! musical roles to the same fixed channels, so the swap also
//! produces a deliberate timbral shift: in song 1 channel 1 is
//! bass on a triangle wave; in song 2 channel 1 is harmony on the
//! same triangle wave. This is intentional and shows that the
//! script controls musical assignment while the host owns
//! synthesis.
//!
//! # Editing
//!
//! Per-channel instrument parameters (waveform, volume) are
//! constants near the top of this file. Song notes are in
//! `examples/piano_roll.kel` and `examples/piano_roll_2.kel` as
//! match-on-index functions returning `(Pitch, octave,
//! duration_in_16ths)` tuples.
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
use keleusma::bytecode::Module;
use keleusma::{Arena, Value};

use sdl3::audio::{AudioCallback, AudioFormat, AudioSpec, AudioStream};

// ---------------------------------------------------------------
// Tunable instrument parameters. Edit these to change the sound
// without touching `piano_roll.kel` or `piano_roll_2.kel`.
// ---------------------------------------------------------------

const SAMPLE_RATE: u32 = 48_000;
const TICK_MS: u64 = 125; // 16th note at 120 BPM
const NUM_VOICES: usize = 3;
const NUM_DATA_SLOTS: usize = 6;

#[derive(Clone, Copy)]
#[allow(dead_code)]
enum Waveform {
    Square,
    Triangle,
    Sawtooth,
    Sine,
}

// Per-channel waveform, fixed across the song swap.
const WAVEFORMS: [Waveform; NUM_VOICES] = [
    Waveform::Square,   // channel 0 (melody in both songs)
    Waveform::Triangle, // channel 1 (bass in song 1, harmony in song 2)
    Waveform::Square,   // channel 2 (harmony in song 1, bass in song 2)
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
    // Pre-compile both scripts. Both are kept available so a swap
    // can be triggered at any time without re-running the
    // compile pipeline.
    let module_a = build_module(SCRIPT_A)?;
    let module_b = build_module(SCRIPT_B)?;

    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm =
        Vm::new(module_a.clone(), &arena).map_err(|e| format!("verify: {:?}", e))?;

    init_data(&mut vm)?;

    // Shared voice state.
    let voices: SharedVoices = Arc::new(Mutex::new([Voice::silent(); NUM_VOICES]));

    // Register host natives. The closures capture `Arc` clones so
    // the same registrations apply across the swap.
    register_natives(&mut vm, &voices);

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

    // Stdin watcher: parse each line, send `Swap` for "s",
    // otherwise send `Quit` and exit.
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

    println!("Keleusma piano roll. 120 BPM, three voices.");
    println!("[ Press 's' + Enter to swap song. Press Enter alone to quit. ]");

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
    let mut active_is_a = true;
    let mut swap_pending = false;

    loop {
        // Drain any pending stdin commands without blocking.
        match cmd_rx.try_recv() {
            Ok(Command::Quit) => break,
            Ok(Command::Swap) => {
                swap_pending = true;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        // Sleep until the next tick boundary.
        let now = Instant::now();
        if now < next_tick {
            thread::sleep(next_tick - now);
        }
        next_tick += interval;

        // Drive the script forward until the next yield. Treat the
        // intermediate `Reset` as the swap point.
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
                        // Silence outgoing voices so the new song
                        // does not inherit pitches it never set.
                        silence_all(&voices);
                        // Start the new module. The first call
                        // drives to the first yield.
                        match vm
                            .call(&[Value::Int(tick)])
                            .map_err(|e| format!("vm call after swap: {:?}", e))?
                        {
                            VmState::Yielded(_) => break,
                            other => {
                                return Err(format!(
                                    "swapped module did not yield: {:?}",
                                    other
                                )
                                .into());
                            }
                        }
                    }
                    // No swap pending: the loop body finished and
                    // will run again on the next resume.
                    continue;
                }
                other => {
                    return Err(format!("unexpected vm state: {:?}", other).into());
                }
            }
        }
        tick = tick.wrapping_add(1);
    }

    silence_all(&voices);
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

fn silence_all(voices: &SharedVoices) {
    let mut v = voices.lock().unwrap();
    for slot in v.iter_mut() {
        slot.gate = false;
    }
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
