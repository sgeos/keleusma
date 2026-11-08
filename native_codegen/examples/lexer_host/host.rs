//! A Rust host linking the self-hosted lexer as a static library.
//!
//! Usage: `host <out-dir> <turns> <source-file>...`
//!
//! Each source file gets its own lexer instance in one slot of a pool the host
//! reserves itself. The instances run in interleaved turns. Each yield is printed
//! as `Y <instance> <turn> <value>`, and each instance's shared buffer is
//! written to `<out-dir>/shared_<instance>.bin` when the run ends.
//!
//! The last line is `S <bytes>`, the deepest machine stack any start or resume
//! call used, measured by painting the stack below the stack pointer before the
//! call and finding the lowest byte the call changed. Both steps are inline
//! assembly, so they touch only memory Rust does not own. A byte the call
//! writes with the paint value is missed, which is why the test runs this host
//! with two paint values and keeps the larger figure. `KEL_STACK_PAINT` selects
//! the value. On a target without the assembly the line reads `S unmeasured`.
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

/// Guard words after every buffer, which the library must never write.
const GUARD_WORDS: usize = 8;
const GUARD: u64 = 0xa5a5_a5a5_a5a5_a5a5;

/// A zeroed buffer aligned to eight bytes, which covers `BUFFER_ALIGN`,
/// followed by guard words that show whether the library wrote past the size
/// its contract exported.
struct Buffer(Vec<u64>, usize);

impl Buffer {
    fn new(bytes: u64) -> Self {
        let words = (bytes as usize).div_ceil(8).max(1);
        let mut v = vec![0; words + GUARD_WORDS];
        v[words..].fill(GUARD);
        Self(v, words)
    }

    /// Whether every guard word is intact.
    fn intact(&self) -> bool {
        self.0[self.1..].iter().all(|&w| w == GUARD)
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
            ("stack_bytes", lib::kel_lexer_stack_bytes, lib::STACK_BYTES),
            ("wcet_cost", lib::kel_lexer_wcet_cost, lib::WCET_COST),
            ("schema_hash", lib::kel_lexer_schema_hash, lib::SCHEMA_HASH),
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
    // SAFETY: as above.
    let status = unsafe { lib::kel_lexer_wcet_status };
    if status != lib::WCET_STATUS {
        return Err(format!("contract mismatch on wcet_status: {status}"));
    }
    if lib::BUFFER_ALIGN > 8 {
        return Err(format!("buffers must align to {}", lib::BUFFER_ALIGN));
    }
    if !cfg!(target_endian = "little") {
        return Err(String::from("this host seeds Word slots little-endian"));
    }
    Ok(())
}

/// Bytes below the stack pointer painted before each probed call. Far above
/// the exported bound, so a bound that is too small shows as a measurement
/// above it rather than as an unmeasured overrun.
const PAINT_BYTES: usize = 64 * 1024;

/// Bytes directly below the stack pointer left unpainted. The System V
/// x86-64 red zone is this size, and the probing function's own code may use it.
const RED_ZONE: usize = 128;

/// Fill `[sp - PAINT_BYTES, sp - RED_ZONE)` with `mark`.
#[inline(always)]
fn paint(mark: u8) {
    #[cfg(target_arch = "aarch64")]
    // SAFETY: the bytes lie in this thread's stack below the stack pointer,
    // which no live Rust frame owns, and well inside its eight-megabyte limit.
    unsafe {
        std::arch::asm!(
            "mov {top}, sp",
            "sub {top}, {top}, {red}",
            "sub {cur}, {top}, {len}",
            "2:",
            "strb {mark:w}, [{cur}], #1",
            "cmp {cur}, {top}",
            "b.lo 2b",
            top = out(reg) _,
            cur = out(reg) _,
            red = in(reg) RED_ZONE,
            len = in(reg) PAINT_BYTES - RED_ZONE,
            mark = in(reg) u32::from(mark),
            options(nostack),
        );
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: as above.
    unsafe {
        std::arch::asm!(
            "mov {top}, rsp",
            "sub {top}, {red}",
            "mov {cur}, {top}",
            "sub {cur}, {len}",
            "2:",
            "mov byte ptr [{cur}], {mark}",
            "inc {cur}",
            "cmp {cur}, {top}",
            "jb 2b",
            top = out(reg) _,
            cur = out(reg) _,
            red = in(reg) RED_ZONE,
            len = in(reg) PAINT_BYTES - RED_ZONE,
            mark = in(reg_byte) mark,
            options(nostack),
        );
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    let _ = mark;
}

/// Bytes between the stack pointer and the lowest painted byte that no longer
/// holds `mark`, or zero when every painted byte still does.
#[inline(always)]
fn used(mark: u8) -> usize {
    let depth: usize;
    #[cfg(target_arch = "aarch64")]
    // SAFETY: reads the bytes `paint` wrote, under the same argument.
    unsafe {
        std::arch::asm!(
            "mov {top}, sp",
            "sub {cur}, {top}, {len}",
            "sub {end}, {top}, {red}",
            "2:",
            "ldrb {byte:w}, [{cur}]",
            "cmp {byte:w}, {mark:w}",
            "b.ne 3f",
            "add {cur}, {cur}, #1",
            "cmp {cur}, {end}",
            "b.lo 2b",
            "mov {cur}, {top}",
            "3:",
            "sub {depth}, {top}, {cur}",
            top = out(reg) _,
            cur = out(reg) _,
            end = out(reg) _,
            byte = out(reg) _,
            depth = lateout(reg) depth,
            red = in(reg) RED_ZONE,
            len = in(reg) PAINT_BYTES,
            mark = in(reg) u32::from(mark),
            options(nostack, readonly),
        );
    }
    #[cfg(target_arch = "x86_64")]
    // SAFETY: as above.
    unsafe {
        std::arch::asm!(
            "mov {top}, rsp",
            "mov {cur}, {top}",
            "sub {cur}, {len}",
            "mov {end}, {top}",
            "sub {end}, {red}",
            "2:",
            "cmp byte ptr [{cur}], {mark}",
            "jne 3f",
            "inc {cur}",
            "cmp {cur}, {end}",
            "jb 2b",
            "mov {cur}, {top}",
            "3:",
            "mov {depth}, {top}",
            "sub {depth}, {cur}",
            top = out(reg) _,
            cur = out(reg) _,
            end = out(reg) _,
            depth = lateout(reg) depth,
            red = in(reg) RED_ZONE,
            len = in(reg) PAINT_BYTES,
            mark = in(reg_byte) mark,
            options(nostack, readonly),
        );
    }
    #[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
    {
        let _ = mark;
        depth = 0;
    }
    depth
}

/// Start an instance and measure the stack the call used.
#[inline(never)]
fn probed_start(mark: u8, inst: &mut Instance, slot: *mut u8) -> (lib::Outcome, usize) {
    let (s, p, r) = (inst.shared.ptr(), inst.private.ptr(), inst.region.ptr());
    paint(mark);
    // SAFETY: the buffers and slot follow the exported contract and stay
    // allocated and unmoved until release.
    let o = unsafe { lib::kel_lexer_start(0, s, p, r, slot) };
    (o, used(mark))
}

/// Resume an instance and measure the stack the call used.
#[inline(never)]
fn probed_resume(mark: u8, slot: *mut u8, reply: i64) -> (lib::Outcome, usize) {
    paint(mark);
    // SAFETY: the instance is live and is resumed by one thread.
    let o = unsafe { lib::kel_lexer_resume(slot, reply) };
    (o, used(mark))
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
    let mark = match std::env::var("KEL_STACK_PAINT") {
        Ok(v) => v.parse::<u8>().map_err(|e| format!("KEL_STACK_PAINT: {e}"))?,
        Err(_) => 0x5a,
    };
    let mut deepest = 0usize;
    for (i, inst) in instances.iter_mut().enumerate() {
        let (o, depth) = probed_start(mark, inst, slot(i));
        deepest = deepest.max(depth);
        emit(i, 0, o)?;
    }
    for t in 1..turns {
        for i in 0..instances.len() {
            let (o, depth) = probed_resume(mark, slot(i), t);
            deepest = deepest.max(depth);
            emit(i, t, o)?;
        }
    }
    if cfg!(any(target_arch = "aarch64", target_arch = "x86_64")) {
        writeln!(log, "S {deepest}").map_err(|e| e.to_string())?;
    } else {
        writeln!(log, "S unmeasured").map_err(|e| e.to_string())?;
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
    // Every buffer was sized from the exported contract, so a written guard is
    // a library writing outside its stated memory.
    let pool_intact = pool.intact();
    for (i, inst) in instances.iter().enumerate() {
        for (name, b) in [
            ("shared", &inst.shared),
            ("private", &inst.private),
            ("region", &inst.region),
        ] {
            if !b.intact() {
                return Err(format!("instance {i} wrote past its {name} buffer"));
            }
        }
    }
    if !pool_intact {
        return Err(String::from("an instance wrote past the slot pool"));
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
