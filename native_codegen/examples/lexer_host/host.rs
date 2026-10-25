//! A Rust host linking the self-hosted lexer as a static library.
//!
//! Usage: `host <out-dir> <turns> <source-file>...`
//!
//! Each source file gets its own lexer instance in one slot of a pool the host
//! reserves itself. The instances run in interleaved turns. Each yield is printed
//! as `Y <instance> <turn> <value>`, and each instance's shared buffer is
//! written to `<out-dir>/shared_<instance>.bin` when the run ends.
//!
//! Every size comes from the generated bindings, and the host first checks that
//! the archive it linked defines the same figures. A mismatch means the
//! bindings and the archive came from different builds, and the host stops
//! rather than run with the wrong reservation.
//!
//! Built by `run.sh`, and by `tests/static_library_linkage.rs`, which compares
//! this host's output with the virtual machine.

include!(env!("KEL_LIBRARY_BINDINGS"));

use kel_lexer as lib;
use std::io::Write;
use std::process::ExitCode;

/// A zeroed buffer aligned to eight bytes, which covers `BUFFER_ALIGN`.
struct Buffer(Vec<u64>);

impl Buffer {
    fn new(bytes: u64) -> Self {
        Self(vec![0; (bytes as usize).div_ceil(8).max(1)])
    }

    fn ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr().cast()
    }

    fn bytes_mut(&mut self, len: u64) -> &mut [u8] {
        // SAFETY: the vector holds at least `len` bytes by construction.
        unsafe { std::slice::from_raw_parts_mut(self.ptr(), len as usize) }
    }
}

fn check_contract() -> Result<(), String> {
    // SAFETY: the statics are immutable constants defined by the archive.
    let linked = unsafe {
        [
            ("contract_version", lib::kel_lexer_contract_version, lib::CONTRACT_VERSION),
            ("frame_bytes", lib::kel_lexer_frame_bytes, lib::FRAME_BYTES),
            ("slot_bytes", lib::kel_lexer_slot_bytes, lib::SLOT_BYTES),
            ("buffer_align", lib::kel_lexer_buffer_align, lib::BUFFER_ALIGN),
            ("shared_bytes", lib::kel_lexer_shared_bytes, lib::SHARED_BYTES),
            ("private_bytes", lib::kel_lexer_private_bytes, lib::PRIVATE_BYTES),
            ("region_bytes", lib::kel_lexer_region_bytes, lib::REGION_BYTES),
            (
                "private_init_bytes",
                lib::kel_lexer_private_init_bytes,
                lib::PRIVATE_INIT_BYTES,
            ),
        ]
    };
    for (name, object, bindings) in linked {
        if object != bindings {
            return Err(format!(
                "contract mismatch on {name}: the archive says {object}, the bindings {bindings}"
            ));
        }
    }
    // SAFETY: as above.
    let verdict = unsafe { lib::kel_lexer_trap_verdict };
    if verdict != lib::TRAP_VERDICT {
        return Err(format!("contract mismatch on trap_verdict: {verdict}"));
    }
    if lib::BUFFER_ALIGN > 8 {
        return Err(format!("buffers must align to {}", lib::BUFFER_ALIGN));
    }
    if !cfg!(target_endian = "little") {
        return Err(String::from("this host seeds Word slots little-endian"));
    }
    Ok(())
}

struct Instance {
    shared: Buffer,
    private: Buffer,
    region: Buffer,
}

impl Instance {
    fn new(text: &[u8]) -> Result<Self, String> {
        use lib::shared::{SRC_BYTES_COUNT, SRC_BYTES_OFFSET, SRC_LEN_OFFSET};
        if text.len() as u64 > SRC_BYTES_COUNT {
            return Err(format!("source longer than {SRC_BYTES_COUNT} bytes"));
        }
        let mut shared = Buffer::new(lib::SHARED_BYTES);
        let view = shared.bytes_mut(lib::SHARED_BYTES);
        let len = SRC_LEN_OFFSET as usize;
        view[len..len + 8].copy_from_slice(&(text.len() as i64).to_le_bytes());
        let bytes = SRC_BYTES_OFFSET as usize;
        view[bytes..bytes + text.len()].copy_from_slice(text);
        let mut private = Buffer::new(lib::PRIVATE_BYTES);
        // SAFETY: the image is an immutable array defined by the archive.
        let image = unsafe { &lib::kel_lexer_private_init };
        private.bytes_mut(lib::PRIVATE_BYTES)[..image.len()].copy_from_slice(image);
        Ok(Self {
            shared,
            private,
            region: Buffer::new(lib::REGION_BYTES),
        })
    }
}

fn run() -> Result<(), String> {
    check_contract()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, turns, sources @ ..] = args.as_slice() else {
        return Err(String::from("usage: host <out-dir> <turns> <source-file>..."));
    };
    let turns: i64 = turns.parse().map_err(|e| format!("turns: {e}"))?;
    if turns < 1 || sources.is_empty() {
        return Err(String::from("need at least one turn and one source"));
    }
    let mut instances = sources
        .iter()
        .map(|p| {
            let text = std::fs::read(p).map_err(|e| format!("{p}: {e}"))?;
            Instance::new(&text)
        })
        .collect::<Result<Vec<_>, _>>()?;
    // The host's own pool, one reservation for every slot.
    let stride = lib::SLOT_BYTES as usize;
    let mut pool = Buffer::new(lib::SLOT_BYTES * instances.len() as u64);
    let base = pool.ptr();
    // SAFETY: slot `i` lies inside the pool, which outlives every instance.
    let slot = |i: usize| unsafe { base.add(i * stride) };

    let stdout = std::io::stdout();
    let mut log = std::io::BufWriter::new(stdout.lock());
    let mut emit = |i: usize, t: i64, o: lib::Outcome| -> Result<(), String> {
        if o.status != 1 {
            return Err(format!("instance {i} turn {t}: status {}", o.status));
        }
        writeln!(log, "Y {i} {t} {}", o.value).map_err(|e| e.to_string())
    };
    for (i, inst) in instances.iter_mut().enumerate() {
        // SAFETY: the buffers and slot follow the exported contract and stay
        // allocated and unmoved until release.
        let o = unsafe {
            lib::kel_lexer_start(
                0,
                inst.shared.ptr(),
                inst.private.ptr(),
                inst.region.ptr(),
                slot(i),
            )
        };
        emit(i, 0, o)?;
    }
    for t in 1..turns {
        for i in 0..instances.len() {
            // SAFETY: the instance is live and is resumed by one thread.
            let o = unsafe { lib::kel_lexer_resume(slot(i), t) };
            emit(i, t, o)?;
        }
    }
    for i in 0..instances.len() {
        // SAFETY: release is idempotent, and a released slot resumes to zeros.
        let after = unsafe {
            lib::kel_lexer_release(slot(i));
            lib::kel_lexer_resume(slot(i), 0)
        };
        if after.status != 0 || after.value != 0 {
            return Err(format!("instance {i} resumed after release: {after:?}"));
        }
    }
    log.flush().map_err(|e| e.to_string())?;
    drop(log);
    for (i, inst) in instances.iter_mut().enumerate() {
        let path = std::path::Path::new(out).join(format!("shared_{i}.bin"));
        std::fs::write(&path, inst.shared.bytes_mut(lib::SHARED_BYTES))
            .map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("host: {e}");
            ExitCode::FAILURE
        }
    }
}
