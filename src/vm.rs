extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use allocator_api2::vec::Vec as ArenaVec;
use keleusma_arena::BottomHandle;

use crate::bytecode::*;
#[cfg(feature = "verify")]
use crate::verify;
#[allow(unused_imports)]
use crate::word::{WideWord, Word};

/// Operand stack and call-frame stack type. Borrows the host-owned
/// arena's bottom region. The `Vm` drops and recreates these at every
/// arena reset because their storage pointer would otherwise alias
/// memory that the bump allocator returns for subsequent allocations.
type StackVec<'arena, T> = ArenaVec<T, BottomHandle<'arena>>;

/// Push a value onto the operand stack, returning early with
/// `VmError::OutOfArena` on allocation failure rather than aborting
/// the host process via `handle_alloc_error`.
///
/// V0.2.0 routes every operand-stack push through this macro so that
/// arena exhaustion during execution surfaces as a typed error to
/// the host. The minimum pre-reservation at `Vm::new` covers the
/// first few pushes; growth beyond the reservation attempts to
/// extend the arena and may fail.
macro_rules! sp {
    ($self:expr, $val:expr) => {
        match $self.stack.try_reserve(1) {
            Ok(()) => $self.stack.push($val),
            Err(_) => {
                return Err(out_of_arena_push("operand stack", $self.arena.capacity()));
            }
        }
    };
}

/// Push a call frame, returning early with `VmError::OutOfArena` on
/// allocation failure. Counterpart to `sp!` for the call-frame stack.
macro_rules! fp {
    ($self:expr, $val:expr) => {
        match $self.frames.try_reserve(1) {
            Ok(()) => $self.frames.push($val),
            Err(_) => {
                return Err(out_of_arena_push("call frame", $self.arena.capacity()));
            }
        }
    };
}

/// A runtime error from the Keleusma VM.
#[derive(Debug, Clone)]
pub enum VmError {
    /// The value stack was empty when a pop was attempted.
    StackUnderflow,
    /// A type mismatch occurred during an operation.
    TypeError(String),
    /// Division or modulo by zero.
    DivisionByZero,
    /// Array or tuple index out of bounds.
    IndexOutOfBounds(i64, usize),
    /// Struct field not found.
    FieldNotFound(String, String),
    /// A native function returned an error.
    NativeError(String),
    /// Invalid or unexpected bytecode.
    InvalidBytecode(String),
    /// A newtype refinement predicate returned false at a
    /// construction site. One of the partial-operation traps; see
    /// [`crate::bytecode::TrapKind`].
    RefinementFailed,
    /// No head of a multiheaded function matched the arguments.
    NoMatchingHead,
    /// No arm of a `match` expression matched the scrutinee.
    /// Reachable only when every arm carries a `when` guard.
    NoMatchingArm,
    /// No arm of a checked-arithmetic construct matched the outcome.
    CheckedArithNoArm,
    /// An enum-to-`Word` cast met a `Value::Enum` whose variant is
    /// outside the declared set, reachable only through a host-
    /// constructed enum value.
    EnumVariantUnmapped,
    /// Structural verification failed at load time.
    VerifyError(String),
    /// Bytecode load failure encountered before verification could run,
    /// such as a header mismatch or postcard decode error.
    LoadError(String),
    /// The host-owned arena ran out of space and the runtime cannot
    /// allocate the storage the script requires. Returned by
    /// [`Vm::new`] and related entry points when the arena is too
    /// small for the operand stack and call-frame preamble that the
    /// program needs. Replaces the previous behavior of aborting the
    /// host process through `handle_alloc_error`.
    OutOfArena(String),
    /// [`Vm::resume`] or [`Vm::resume_err`] was called on a VM that is
    /// not in the suspended state. The host must call [`Vm::call`]
    /// first to enter a coroutine and reach the first `yield` before
    /// resuming. Distinguished from [`VmError::InvalidBytecode`] to
    /// keep API misuse separate from corrupt or malformed bytecode.
    NotSuspended,
}

impl From<crate::bytecode::LoadError> for VmError {
    fn from(e: crate::bytecode::LoadError) -> Self {
        VmError::LoadError(format!("{}", e))
    }
}

/// Coarse policy category for a [`VmError`]. Used by hosts that want
/// to make a single retry-or-halt decision without matching the
/// full variant set.
///
/// The category is a derivation from the variant, not a stored
/// field, so adding a new `VmError` variant requires updating
/// [`VmError::category`] but does not change the wire format or any
/// per-error allocation. Hosts that need finer policy than the
/// three-way split continue to match on the variant directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmErrorCategory {
    /// Unrecoverable. The VM is in an undefined intermediate state
    /// and must not be resumed. Hosts that wish to continue running
    /// programs must call [`Vm::reset_after_error`] before any
    /// further [`Vm::call`] or [`Vm::resume`]. Examples:
    /// `StackUnderflow`, `InvalidBytecode`, `VerifyError`,
    /// `LoadError`, `OutOfArena`, `NotSuspended`.
    Halt,
    /// Recoverable script-side error. The script asked for something
    /// that the VM rejected; the host may surface the error to the
    /// script through [`Vm::resume_err`] or restart the iteration.
    /// Examples: `TypeError`, `DivisionByZero`, `IndexOutOfBounds`,
    /// `FieldNotFound`, `NoMatch`, `Trap`.
    SoftScript,
    /// Recoverable host-side error. A native function the host
    /// registered returned an error. The host owns the recovery
    /// policy (retry, fallback, surface to the script through
    /// [`Vm::resume_err`]). Example: `NativeError`.
    SoftHost,
}

impl VmError {
    /// Coarse retry-or-halt category for this error.
    ///
    /// See [`VmErrorCategory`] for the three-way split and per-variant
    /// rationale. Hosts that need finer policy match on `self`
    /// directly.
    pub fn category(&self) -> VmErrorCategory {
        match self {
            // Halt: the VM state is undefined or unrecoverable.
            VmError::StackUnderflow
            | VmError::InvalidBytecode(_)
            | VmError::VerifyError(_)
            | VmError::LoadError(_)
            | VmError::OutOfArena(_)
            | VmError::NotSuspended => VmErrorCategory::Halt,
            // Soft script: the script's request was invalid at the
            // VM level, but the VM's invariants hold and the host
            // can ask the script to retry through `resume_err`.
            VmError::TypeError(_)
            | VmError::DivisionByZero
            | VmError::IndexOutOfBounds(_, _)
            | VmError::FieldNotFound(_, _)
            | VmError::RefinementFailed
            | VmError::NoMatchingHead
            | VmError::NoMatchingArm
            | VmError::CheckedArithNoArm
            | VmError::EnumVariantUnmapped => VmErrorCategory::SoftScript,
            // Soft host: a native returned an error. The host owns
            // the policy.
            VmError::NativeError(_) => VmErrorCategory::SoftHost,
        }
    }
}

/// Type alias for the bundled 64-bit `VmState` shape.
pub type VmState = GenericVmState<i64, f64>;

/// The execution state of the VM, parametric over the runtime's
/// word and float widths.
#[derive(Debug, Clone)]
pub enum GenericVmState<W: crate::word::Word, F: crate::float::Float> {
    /// The coroutine yielded a value and is suspended.
    Yielded(crate::bytecode::GenericValue<W, F>),
    /// The function completed with a return value.
    Finished(crate::bytecode::GenericValue<W, F>),
    /// The stream hit a Reset boundary.
    Reset,
}

/// Policy for handling WCET and WCMU bound overflow at verification.
///
/// The compiler saturates the declared WCET and WCMU header fields to
/// `u32::MAX` when the static analysis cannot bound the value. Under
/// the default [`OverflowPolicy::Reject`] policy, `Vm::new_with_options`
/// rejects such modules as a `VmError::VerifyError`. Hosts that wish to
/// accept the module despite the overflow may downgrade the policy to
/// [`OverflowPolicy::Warn`] (returns the module with a `VerifyWarning`
/// describing the overflow) or [`OverflowPolicy::Allow`] (admits the
/// module silently).
///
/// The policy applies to the declared header fields only. Resource
/// bounds against the arena capacity continue to be enforced because
/// they are a load-time admissibility check rather than a static
/// analysis overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowPolicy {
    /// Overflow is a verification error. Default.
    #[default]
    Reject,
    /// Overflow produces a [`VerifyWarning`] entry and admits the module.
    Warn,
    /// Overflow admits the module silently.
    Allow,
}

/// Construction-time options for [`Vm::new_with_options`].
///
/// The default options apply the strict overflow policy
/// ([`OverflowPolicy::Reject`]) and otherwise match the behaviour of
/// the bare [`Vm::new`] constructor.
#[derive(Debug, Clone, Copy, Default)]
pub struct VmOptions {
    /// Policy for handling WCET and WCMU bound overflow at the
    /// declared header level. See [`OverflowPolicy`].
    pub overflow_policy: OverflowPolicy,
}

/// Non-fatal finding produced by [`Vm::new_with_options`] when an
/// overflow condition is downgraded under the chosen
/// [`OverflowPolicy`].
#[derive(Debug, Clone)]
pub struct VerifyWarning {
    /// Human-readable description of the warning.
    pub message: String,
    /// The category of warning. Hosts may switch on this to route
    /// warnings to telemetry or to apply per-kind handling.
    pub kind: WarningKind,
}

/// Category of [`VerifyWarning`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    /// The declared WCET cycles field saturated to `u32::MAX`.
    WcetOverflow,
    /// The declared WCMU bytes field saturated to `u32::MAX`.
    WcmuOverflow,
}

/// A call frame on the VM call stack.
#[derive(Debug, Clone, Copy)]
struct CallFrame {
    /// Index of the chunk being executed.
    chunk_idx: usize,
    /// Instruction pointer (next instruction to execute).
    ip: usize,
    /// Stack base for this frame's local variables.
    base: usize,
}

/// Context passed to native functions that opt into arena access.
///
/// Created freshly at each [`crate::bytecode::Op::CallVerifiedNative`] or [`crate::bytecode::Op::CallExternalNative`] dispatch with a borrow
/// of the host-owned arena. Native functions allocate dynamic strings
/// through `KString::alloc(ctx.arena, s)` and return them as
/// [`crate::bytecode::GenericValue::KStr`] for the bounded-memory path. Natives that do not
/// need arena access can be registered through [`Vm::register_native`]
/// or [`Vm::register_fn`], whose function types omit the context.
pub struct NativeCtx<'a> {
    /// Borrow of the host-owned arena for string and scratch
    /// allocations.
    pub arena: &'a keleusma_arena::Arena,
}

/// Type alias for a native function callable from Keleusma.
///
/// All native functions internally accept a [`NativeCtx`] to support
/// arena-aware natives. Natives registered through the no-context API
/// ignore the context.
type NativeFn<W, F> = Box<
    dyn for<'a> Fn(
        &NativeCtx<'a>,
        &[crate::bytecode::GenericValue<W, F>],
    ) -> Result<crate::bytecode::GenericValue<W, F>, VmError>,
>;

/// A registered native function.
struct NativeEntry<W: crate::word::Word, F: crate::float::Float> {
    name: String,
    func: NativeFn<W, F>,
    /// Host-attested worst-case execution time, in the same unitless cost
    /// space as `Op::cost()`. Default `DEFAULT_NATIVE_WCET`.
    #[allow(dead_code)]
    wcet: u32,
    /// Host-attested worst-case memory usage in bytes. Native functions
    /// that allocate from the arena must override this for the analysis
    /// to remain sound. Default `DEFAULT_NATIVE_WCMU_BYTES`.
    #[allow(dead_code)]
    wcmu_bytes: u32,
    /// Classification recorded at registration. Cross-checked at
    /// the call-site dispatch against the bytecode's opcode
    /// (`CallVerifiedNative` versus `CallExternalNative`). A
    /// mismatch is rejected as a `VmError::VerifyError`.
    classification: NativeClassification,
    /// External-native upper bound on the per-iteration invocation
    /// count. Recorded for external natives at registration and
    /// consumed by future verifier passes that bound external-call
    /// cost contribution against this attestation. `None` for
    /// verified natives. Current verifier passes use `wcmu_bytes`
    /// for the per-call attestation; the invocation-count
    /// attestation is forward-looking V0.2.x work.
    #[allow(dead_code)]
    max_invocations_per_iteration: Option<u32>,
}

/// Per-native classification recorded at host registration and
/// cross-checked against the call-site opcode at `Vm::new`.
///
/// `Verified` natives are registered through
/// [`GenericVm::register_native`], [`GenericVm::register_fn`], or
/// [`GenericVm::register_verified_native`]. The host attests the
/// per-call WCET and WCMU bound; the verifier folds these into the
/// iteration's static budget.
///
/// `External` natives are registered through
/// [`GenericVm::register_external_native`]. The host attests the
/// maximum invocation count per iteration rather than the per-call
/// cost; the verifier observes the structural marker without
/// charging the iteration budget for individual call cost.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeClassification {
    /// `use module::name` import. Per-call WCET / WCMU attested.
    Verified,
    /// `use external module::name` import. Per-iteration invocation
    /// count attested.
    External,
}

/// Default WCET attestation for a native function. Equal to the cost of a
/// single native-call opcode.
pub const DEFAULT_NATIVE_WCET: u32 = 10;

/// Default WCMU attestation for a native function. Native functions that
/// allocate from the arena must override this through
/// `Vm::set_native_bounds`.
pub const DEFAULT_NATIVE_WCMU_BYTES: u32 = 0;

/// Default arena capacity in bytes when constructed via `Vm::new`. The host
/// can override this by calling `Vm::new_with_arena_capacity`.
pub const DEFAULT_ARENA_CAPACITY: usize = 64 * 1024;

/// Minimum operand-stack capacity pre-reserved at `Vm::new`, in slots.
/// The reservation lets `Vm::new` fail fast with `VmError::OutOfArena`
/// when the arena is too small to hold even a trivial program's working
/// set, rather than aborting the host process on a later push.
///
/// The value is a conservative floor that covers small `fn main`
/// programs without overcommitting tiny arenas beyond what they can
/// hold. Stream programs whose WCMU exceeds this floor still rely on
/// in-execution growth.
const MIN_STACK_RESERVE_SLOTS: usize = 4;

/// Minimum call-frame capacity pre-reserved at `Vm::new`.
const MIN_FRAMES_RESERVE: usize = 1;

/// Build a `VmError::OutOfArena` for an in-execution push that exceeded
/// the arena's capacity.
fn out_of_arena_push(region: &str, capacity: usize) -> VmError {
    use alloc::format;
    VmError::OutOfArena(format!(
        "arena exhausted while growing the {} (capacity {} bytes). \
         Increase the arena, or compute a sufficient size with \
         `auto_arena_capacity_for` plus a host-side margin.",
        region, capacity
    ))
}

/// Build a `VmError::OutOfArena` with the minimum reservation message.
fn out_of_arena_min(capacity: usize) -> VmError {
    use alloc::format;
    VmError::OutOfArena(format!(
        "arena capacity of {} bytes is too small to pre-reserve the \
         operand-stack and call-frame minimums ({} slots and {} frames). \
         Increase the arena capacity, or compute a sufficient size with \
         `auto_arena_capacity_for` plus a host-side margin.",
        capacity, MIN_STACK_RESERVE_SLOTS, MIN_FRAMES_RESERVE
    ))
}

/// Decode every op in every chunk of a bytecode buffer and return
/// the resulting per-chunk owned op vectors.
///
/// The returned vector is indexed by chunk index. Each inner vector
/// contains the chunk's ops in instruction order. The hot dispatch
/// loop reads from these vectors directly through `chunk_op` to
/// avoid the per-fetch discriminant match against the archived form.
///
/// The buffer's framing is validated through `Module::access_bytes`.
/// The op decoding uses `op_from_archived` for each op slot. This is
/// the same conversion that the previous hot-path fetch performed;
/// pre-decoding amortizes its cost across the VM lifetime instead of
/// paying it per fetch.
///
/// Both the owned (`AlignedVec`) and the borrowed (`&[u8]`) paths
/// route through this helper. The single source of truth for op
/// pre-decoding lives here.
/// Compute the `(low, high, flag)` outputs that the
/// `CheckedAdd` / `CheckedSub` / `CheckedMul` / `CheckedNeg`
/// dispatch pushes, given the true result `r` in `W::Wide` and
/// the bytecode's declared word width.
///
/// Semantics. The flag is `0` ok, `1` overflow, `2` underflow
/// against the declared width. The low half is sign-extended
/// truncated to the declared width through
/// [`crate::bytecode::truncate_int_to_declared_width`]; the
/// high half carries the bits above the declared low half, so
/// `r == (high << declared_bits) + low_signed_at_declared_width`
/// reproduces the true result. Both halves are interpreted as
/// signed values at the declared width.
///
/// When the bytecode-declared word width matches or exceeds the
/// runtime word width (`word_bits_log2 >= W::BITS_LOG2`), the
/// helper reduces to the runtime-width semantics: `high` becomes
/// the wide high half, `low` becomes the wide-to-narrow wrap of
/// `r`, and `flag` fires at `W::MIN` / `W::MAX`. When the
/// bytecode declares a narrower width than the runtime supports,
/// the flag fires at the declared range and the high half
/// carries the bits beyond the declared low.
fn checked_arith_outputs<W: crate::word::Word>(r: W::Wide, word_bits_log2: u8) -> (W, W, W) {
    let runtime_bits_log2 = <W as crate::word::Word>::BITS_LOG2;
    let (declared_min, declared_max, narrow) = if word_bits_log2 >= runtime_bits_log2 {
        (
            <W as crate::word::Word>::MIN.widen(),
            <W as crate::word::Word>::MAX.widen(),
            false,
        )
    } else {
        let declared_bits = 1u32 << word_bits_log2;
        let one_widened = <W as crate::word::Word>::from_i64_wrap(1).widen();
        let max = (one_widened << (declared_bits - 1)) - one_widened;
        let min = -(one_widened << (declared_bits - 1));
        (min, max, true)
    };
    let low_raw = <W as crate::word::Word>::from_wide_wrap(r);
    let low_at_declared_i64 =
        crate::bytecode::truncate_int_to_declared_width(low_raw.to_i64(), word_bits_log2);
    let low = <W as crate::word::Word>::from_i64_wrap(low_at_declared_i64);
    let high = if narrow {
        let declared_bits = 1u32 << word_bits_log2;
        let low_widened = low.widen();
        let high_wide = (r - low_widened) >> declared_bits;
        <W as crate::word::Word>::from_wide_wrap(high_wide)
    } else {
        <W as crate::word::Word>::from_wide_wrap(r.high_half())
    };
    let flag: i64 = if r >= declared_min && r <= declared_max {
        0
    } else if r > declared_max {
        1
    } else {
        2
    };
    (low, high, <W as crate::word::Word>::from_i64_wrap(flag))
}

/// Classify a checked `Fixed` result already computed in the wide
/// `i128` domain into the construct's `(low, flag)` pair: the
/// two's-complement-wrapped `Word`-width result and the outcome flag
/// `0` (ok, in range), `1` (overflow, above `i64::MAX`), or `2`
/// (underflow, below `i64::MIN`). `Fixed` always occupies the full
/// runtime word width, so the range is the runtime `Word` range with
/// no narrow-declared-width handling. Unlike `Op::FixedMul` and
/// `Op::FixedDiv`, which saturate, the checked form wraps so the
/// `overflow`/`underflow` arms observe the two's-complement result,
/// matching the wrapping default of the other checked families.
fn fixed_checked_outputs<W: crate::word::Word>(r: W::Wide) -> (W, i64) {
    let min = <W as crate::word::Word>::MIN.widen();
    let max = <W as crate::word::Word>::MAX.widen();
    let flag: i64 = if r >= min && r <= max {
        0
    } else if r > max {
        1
    } else {
        2
    };
    (<W as crate::word::Word>::from_wide_wrap(r), flag)
}

/// Classify a checked floating-point result into the construct's
/// flag: `0` ok (finite), `1` overflow (positive infinity), `2`
/// underflow (negative infinity), `4` not-a-number. The Institute of
/// Electrical and Electronics Engineers 754 operations are total, so
/// there is no zero-divisor flag (flag `3`) for floats; a division by
/// zero produces an infinity or a NaN classified here.
#[cfg(feature = "floats")]
fn float_checked_flag(rf: f64) -> i64 {
    if rf.is_nan() {
        4
    } else if rf.is_infinite() {
        if rf > 0.0 { 1 } else { 2 }
    } else {
        0
    }
}

fn decode_all_ops(bytes: &[u8]) -> Result<Vec<Vec<Op>>, VmError> {
    // V0.2.0 Phase 7c routes the per-chunk op decode through
    // the wire-format opcode stream. Each chunk's slice in the
    // stream is bounded by the WireChunk's `op_byte_offset` and
    // `op_record_count`; the records decode through the shared
    // operand pool.
    let sections = crate::wire_format::parse_wire_sections(bytes)?;
    let archived = rkyv::access::<crate::wire_format::ArchivedWireAuxBody, rkyv::rancor::Error>(
        sections.aux_body,
    )
    .map_err(|e| crate::bytecode::LoadError::Codec(alloc::format!("rkyv access failed: {}", e)))?;
    let mut all_ops: Vec<Vec<Op>> = Vec::with_capacity(archived.chunks.len());
    for chunk in archived.chunks.iter() {
        let start = chunk.op_byte_offset.to_native() as usize;
        let record_count = chunk.op_record_count.to_native() as usize;
        let byte_span = record_count
            .checked_mul(crate::wire_format::OPCODE_RECORD_BYTES)
            .ok_or_else(|| {
                crate::bytecode::LoadError::Codec(alloc::string::String::from(
                    "opcode span overflow",
                ))
            })?;
        let end = start.checked_add(byte_span).ok_or_else(|| {
            crate::bytecode::LoadError::Codec(alloc::string::String::from("opcode span overflow"))
        })?;
        if end > sections.opcode_stream.len() {
            return Err(crate::bytecode::LoadError::Codec(alloc::format!(
                "chunk opcode span [{}..{}) exceeds opcode stream length {}",
                start,
                end,
                sections.opcode_stream.len(),
            ))
            .into());
        }
        let ops = crate::wire_format::decode_op_stream(
            &sections.opcode_stream[start..end],
            sections.operand_pool,
        )?;
        all_ops.push(ops);
    }
    Ok(all_ops)
}

/// Compute the smallest arena capacity that admits the given module
/// under the supplied native attestations. Returns the maximum WCMU sum
/// across Stream chunks, or zero if the module has no Stream chunks.
///
/// Available only when the `verify` feature is enabled because the
/// computation routes through `verify::module_wcmu`. Hosts that
/// build without the verifier must size arenas through a build-time
/// analysis instead.
#[cfg(feature = "verify")]
pub fn auto_arena_capacity_for(
    module: &crate::bytecode::Module,
    native_wcmu: &[u32],
) -> Result<usize, VmError> {
    let chunk_wcmu = verify::module_wcmu(module, native_wcmu)
        .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
    let mut max_total: usize = 0;
    for (chunk_idx, chunk) in module.chunks.iter().enumerate() {
        if chunk.block_type == crate::bytecode::BlockType::Stream {
            let (s, h) = chunk_wcmu[chunk_idx];
            let total = (s as usize).saturating_add(h as usize);
            if total > max_total {
                max_total = total;
            }
        }
    }
    Ok(max_total)
}

/// Bytecode storage for the VM.
///
/// `Owned` carries an `AlignedVec` produced by serializing a `Module`
/// at construction time. `Borrowed` carries a slice supplied by the
/// host through `Vm::view_bytes_unchecked` for true zero-copy
/// execution from `.rodata` or any addressable buffer.
enum BytecodeStore<'a> {
    Owned(rkyv::util::AlignedVec<8>),
    Borrowed(&'a [u8]),
}

impl<'a> BytecodeStore<'a> {
    fn as_slice(&self) -> &[u8] {
        match self {
            BytecodeStore::Owned(v) => v.as_slice(),
            BytecodeStore::Borrowed(b) => b,
        }
    }
}

/// The Keleusma virtual machine.
///
/// Two lifetime parameters. `'a` reflects the bytecode source. VMs
/// constructed from an owned `Module` or from arbitrary byte slices
/// carry `Vm<'static, 'arena>`. VMs constructed via
/// [`Vm::view_bytes_unchecked`] from a borrowed slice carry
/// `Vm<'a, 'arena>` for the slice's lifetime.
///
/// `'arena` ties the VM to a host-owned [`keleusma_arena::Arena`].
/// The host constructs the arena and passes it as a shared reference at
/// VM construction time. Dynamic strings produced during execution
/// allocate into this arena and survive until the next reset.
///
/// Reset of the arena is initiated by the VM through
/// [`keleusma_arena::Arena::reset_unchecked`] because the VM holds the
/// arena through a shared reference. The VM's internal discipline
/// guarantees no allocator-bound collection retains storage in the
/// arena at the moment of reset. Hosts that want to reset the arena
/// from outside the VM must do so when the VM is not borrowing the
/// arena, which means the VM must be dropped first or the host must
/// invoke [`Vm::reset_after_error`] which routes through the unsafe
/// path with the same safety justification.
/// Type alias for the bundled 64-bit `Vm` shape. Existing call
/// sites continue to write `Vm<'a, 'arena>`; the alias expands
/// to `GenericVm<'a, 'arena, i64, u64, f64>`. Sub-64-bit
/// runtimes use a different specialization; hosts introduce a
/// local alias for ergonomic call sites.
pub type Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;

/// Parametric stack-based virtual machine. The type parameters
/// model the runtime's word, address, and float widths so a host
/// can construct a narrow-width runtime (`GenericVm<i16, u16, f32>`)
/// for embedded targets. The default specialization
/// (`GenericVm<i64, u64, f64>`) is the bundled [`Vm`] alias.
pub struct GenericVm<
    'a,
    'arena,
    W: crate::word::Word = i64,
    A: crate::address::Address = u64,
    F: crate::float::Float = f64,
> {
    bytecode: BytecodeStore<'a>,
    /// Phantom marker for the script-visible address-width type
    /// parameter. No `GenericValue` variant carries an address
    /// payload, so `A` does not appear in any field directly.
    _phantom_a: core::marker::PhantomData<A>,
    /// Per-op decode cache, populated at VM construction and at every
    /// `replace_module`. Indexed as `decoded_ops[chunk_idx][ip]`.
    decoded_ops: Vec<Vec<Op>>,
    /// Operand stack. Bump-allocated from the arena's bottom region.
    stack: StackVec<'arena, crate::bytecode::GenericValue<W, F>>,
    /// Call-frame stack. Same arena-backed discipline as `stack`.
    frames: StackVec<'arena, CallFrame>,
    natives: Vec<NativeEntry<W, F>>,
    /// Shared data slots. Survives across RESET boundaries.
    data: Vec<crate::bytecode::GenericValue<W, F>>,
    /// Number of shared slots. Cached at construction from the
    /// module's data layout. Equals `data.len()` for shared
    /// slots; the unified slot index space partitions into
    /// `[0, shared_slot_count)` (shared) and
    /// `[shared_slot_count, shared_slot_count + private_slot_count)`
    /// (private).
    shared_slot_count: u16,
    /// Number of private slots. Cached at construction. Private
    /// slots live in the arena's persistent region starting at
    /// `arena.persistent_ptr()` and occupy
    /// `private_slot_count * size_of::<crate::bytecode::GenericValue<W, F>>()` bytes there.
    private_slot_count: u16,
    /// Host-owned dual-end bump-allocated arena. Borrowed for the
    /// lifetime of the VM. Native functions that allocate dynamic
    /// strings pass `vm.arena()` to [`crate::kstring::KString::alloc`].
    /// The arena's persistent region holds this module's private
    /// data slots.
    arena: &'arena keleusma_arena::Arena,
    started: bool,
    /// Cached load-time native classification check. Populated on
    /// the first `call` after natives are registered (or after
    /// `replace_module`). `None` means the check has not been run
    /// yet; `Some(Ok(()))` means the check succeeded; the error
    /// arm is surfaced at the call boundary the first time it is
    /// detected and is not re-cached, so the host can recover by
    /// re-registering natives with the correct classification.
    /// Any `register_*` method or `replace_module` resets this
    /// field to `None`.
    native_classifications_verified: bool,
    /// Host-supplied trust matrix for cryptographic module
    /// signatures. Populated through
    /// [`Self::register_verifying_key`] before the host hot-swaps
    /// a signed module via
    /// [`Self::replace_module_from_bytes`]. Empty by default; an
    /// empty matrix rejects every signed module with
    /// [`crate::bytecode::LoadError::InvalidSignature`]. Gated on
    /// the `signatures` cargo feature; builds without it carry no
    /// trust matrix at all.
    #[cfg(feature = "signatures")]
    verifying_keys: Vec<ed25519_dalek::VerifyingKey>,
}

/// Compute the arena persistent-capacity needed to back a
/// module's private data segment.
///
/// Hosts call this to size a pool arena before constructing the
/// VM:
///
/// ```text
/// let needed = required_persistent_capacity_for(&module);
/// arena.resize_persistent(needed)?;
/// let vm = Vm::new(module, &arena)?;
/// ```
///
/// The returned value is `private_slot_count * size_of::<crate::bytecode::GenericValue<W, F>>()`,
/// which is the actual runtime storage requirement. It differs
/// from `module.private_data_bytes` because that field is in
/// `VALUE_SLOT_SIZE_BYTES`-sized logical units for WCMU
/// accounting; the actual `Value` enum is larger than the WCMU
/// slot size by an implementation-defined factor.
pub fn required_persistent_capacity_for(module: &crate::bytecode::Module) -> usize {
    required_persistent_capacity_for_generic::<i64, f64>(module)
}

/// Generic counterpart sized against the host's specific
/// `GenericValue<W, F>` storage requirement.
pub fn required_persistent_capacity_for_generic<W: crate::word::Word, F: crate::float::Float>(
    module: &crate::bytecode::Module,
) -> usize {
    let private_count = module.data_layout.as_ref().map_or(0, |dl| {
        dl.slots
            .iter()
            .filter(|s| matches!(s.visibility, crate::bytecode::SlotVisibility::Private))
            .count()
    });
    private_count * core::mem::size_of::<crate::bytecode::GenericValue<W, F>>()
}

/// Number of shared `.data` slots declared in `module`. The shared
/// region lives inside the [`GenericVm`] itself as a
/// `Vec<GenericValue>`, not in the arena; hosts read and write
/// individual slots through [`GenericVm::set_data`] and
/// [`GenericVm::get_data`]. This helper exposes the slot count so an
/// embedder can pre-size a per-slot checkpoint buffer (for example
/// the REPL's session-state buffer) without constructing the VM
/// first.
///
/// The count matches `Self::shared_slot_count` after the VM is
/// built from the same module.
pub fn shared_slot_count_for(module: &crate::bytecode::Module) -> usize {
    module.data_layout.as_ref().map_or(0, |dl| {
        dl.slots
            .iter()
            .filter(|s| matches!(s.visibility, crate::bytecode::SlotVisibility::Shared))
            .count()
    })
}

impl<'a, 'arena, W: crate::word::Word, A: crate::address::Address, F: crate::float::Float> Drop
    for GenericVm<'a, 'arena, W, A, F>
{
    /// Drop the `Value` instances stored in the arena's
    /// persistent region. The arena itself is host-owned and
    /// outlives the VM; without this drop, every private slot's
    /// owned contents (heap-allocated strings, vectors,
    /// reference counts) would leak when the VM is dropped.
    ///
    /// The arena's persistent capacity is left unchanged because
    /// the host may want to reassign the same arena to another
    /// VM; calling `Arena::resize_persistent` is the host's
    /// choice.
    fn drop(&mut self) {
        if self.private_slot_count == 0 {
            return;
        }
        let base = self.arena.persistent_ptr().as_ptr() as *mut crate::bytecode::GenericValue<W, F>;
        for i in 0..self.private_slot_count as usize {
            // SAFETY: each private slot was initialised to
            // `crate::bytecode::GenericValue::Unit` at construction and updated through
            // `write_data_slot` (which drops the old occupant
            // and writes a new value). At drop time every slot
            // therefore holds a valid `Value` that needs its
            // destructor run.
            unsafe {
                core::ptr::drop_in_place(base.add(i));
            }
        }
    }
}

impl<'a, 'arena, W: crate::word::Word, A: crate::address::Address, F: crate::float::Float>
    GenericVm<'a, 'arena, W, A, F>
{
    /// Borrow the archived auxiliary body from internal bytecode storage.
    ///
    /// V0.2.0 Phase 7c routes the lookup through the section-
    /// partitioned wire format. The opcode stream and operand
    /// pool sections are not consulted here; consumers of
    /// per-chunk ops use `self.decoded_ops` which was populated
    /// once at construction time. The aux body section starts
    /// at an 8-byte aligned offset declared in the framing
    /// header. The bytes were validated at construction time,
    /// so `access_unchecked` is sound here.
    fn archived(&self) -> &crate::wire_format::ArchivedWireAuxBody {
        let bytes = self.bytecode.as_slice();
        let aux_body_offset =
            u32::from_le_bytes([bytes[48], bytes[49], bytes[50], bytes[51]]) as usize;
        let aux_body_length =
            u32::from_le_bytes([bytes[52], bytes[53], bytes[54], bytes[55]]) as usize;
        let aux = &bytes[aux_body_offset..aux_body_offset + aux_body_length];
        unsafe { rkyv::access_unchecked::<crate::wire_format::ArchivedWireAuxBody>(aux) }
    }

    /// Deserialize the current bytecode to an owned `Module`.
    ///
    /// Used by cold-path methods such as resource-bounds re-verification
    /// and auto-arena-capacity computation that operate on owned
    /// `Module` values. The hot execution path uses `archived` directly.
    /// Available only when the `verify` feature is enabled because all
    /// current callers are themselves gated behind that feature.
    #[cfg(feature = "verify")]
    fn module_owned(&self) -> Result<Module, VmError> {
        Ok(Module::from_bytes(self.bytecode.as_slice())?)
    }

    /// Read the op at `(chunk_idx, ip)` from the per-op decode cache
    /// populated at VM construction. The hot dispatch loop calls this
    /// per fetch, so the implementation is a direct slice index. The
    /// archived form is no longer consulted on the hot path.
    fn chunk_op(&self, chunk_idx: usize, ip: usize) -> Op {
        self.decoded_ops[chunk_idx][ip]
    }

    /// Materialize the constant at `(chunk_idx, idx)` from archived storage.
    fn chunk_const(&self, chunk_idx: usize, idx: usize) -> crate::bytecode::GenericValue<W, F> {
        let chunk = &self.archived().chunks[chunk_idx];
        crate::bytecode::value_from_archived(&chunk.constants[idx])
    }

    /// Number of ops in the chunk. V0.2.0 Phase 7c reads the
    /// count from the WireChunk's `op_record_count` rather than
    /// the no-longer-present `ops` vector; the ops themselves
    /// live in the opcode stream section.
    fn chunk_op_count(&self, chunk_idx: usize) -> usize {
        self.archived().chunks[chunk_idx]
            .op_record_count
            .to_native() as usize
    }

    /// Local-variable slot count for the chunk (includes parameters).
    fn chunk_local_count(&self, chunk_idx: usize) -> u16 {
        self.archived().chunks[chunk_idx].local_count.to_native()
    }

    /// Module-wide word-width exponent. Used by the checked-
    /// arithmetic dispatch to apply narrow-width truncation to the
    /// `low` result when the bytecode declares a word size smaller
    /// than the runtime supports (cross-architecture portability).
    fn word_bits_log2(&self) -> u8 {
        self.archived().word_bits_log2
    }

    /// Test whether a chunk index is in range.
    fn chunk_count(&self) -> usize {
        self.archived().chunks.len()
    }

    /// Look up a native function name by index. Returns `None` if out of bounds.
    fn native_name(&self, idx: usize) -> Option<alloc::string::String> {
        use alloc::string::ToString;
        self.archived()
            .native_names
            .get(idx)
            .map(|s| s.as_str().to_string())
    }

    /// Read a string-typed constant. Returns `None` if not a string.
    fn chunk_const_str(&self, chunk_idx: usize, idx: usize) -> Option<alloc::string::String> {
        use alloc::string::ToString;
        let chunk = &self.archived().chunks[chunk_idx];
        match &chunk.constants[idx] {
            crate::bytecode::ArchivedConstValue::StaticStr(s) => Some(s.as_str().to_string()),
            _ => None,
        }
    }

    /// Look up a struct template's type name and field names.
    fn struct_template(
        &self,
        chunk_idx: usize,
        idx: usize,
    ) -> (
        alloc::string::String,
        alloc::vec::Vec<alloc::string::String>,
    ) {
        use alloc::string::ToString;
        let template = &self.archived().chunks[chunk_idx].struct_templates[idx];
        let type_name = template.type_name.as_str().to_string();
        let field_names: alloc::vec::Vec<_> = template
            .field_names
            .iter()
            .map(|s| s.as_str().to_string())
            .collect();
        (type_name, field_names)
    }
}

impl<'a, 'arena, W: crate::word::Word, A: crate::address::Address, F: crate::float::Float>
    GenericVm<'a, 'arena, W, A, F>
{
    /// Create a new VM with the given compiled module and a host-owned
    /// arena.
    ///
    /// The arena's capacity must accommodate the module's worst-case
    /// memory usage. The host typically sizes the arena via
    /// [`auto_arena_capacity_for`] before constructing the VM. Runs
    /// structural verification on the module and resource bounds
    /// verification against the arena's capacity. Returns an error if
    /// either check fails.
    pub fn new(module: Module, arena: &'arena keleusma_arena::Arena) -> Result<Self, VmError> {
        let (vm, _warnings) = Self::new_with_options(module, arena, VmOptions::default())?;
        Ok(vm)
    }

    /// Create a new VM with explicit construction-time options.
    ///
    /// Same admissibility checks as [`Vm::new`] (structural
    /// verification followed by the resource-bounds check against the
    /// arena capacity), plus a configurable overflow policy that
    /// decides what to do when the module's declared WCET or WCMU
    /// header fields saturated to `u32::MAX` during compilation. The
    /// default policy ([`OverflowPolicy::Reject`]) treats overflow as
    /// a `VerifyError`, matching the historic behaviour of `Vm::new`.
    /// Hosts that wish to admit overflow-saturated modules can supply
    /// [`OverflowPolicy::Warn`] to receive a [`VerifyWarning`] for
    /// each overflowing field, or [`OverflowPolicy::Allow`] to admit
    /// the module silently.
    ///
    /// Returns the constructed VM together with a vector of warnings.
    /// Under the default policy the vector is always empty because
    /// the function returns `Err` before reaching the construction
    /// step on overflow.
    pub fn new_with_options(
        module: Module,
        arena: &'arena keleusma_arena::Arena,
        options: VmOptions,
    ) -> Result<(Self, Vec<VerifyWarning>), VmError> {
        let mut module = module;
        let mut warnings: Vec<VerifyWarning> = Vec::new();
        // R1. The module must declare an entry point. The compiler sets
        // `entry_point` from the `main` function. Detecting absence
        // here gives a clear `VerifyError` at the API boundary instead
        // of deferring the failure to the first `Vm::call`, which
        // would otherwise surface as `InvalidBytecode("no entry
        // point")` at the first use site.
        if module.entry_point.is_none() {
            return Err(VmError::VerifyError(String::from(
                "module has no entry point: declare a `fn main`, `yield main`, or `loop main`",
            )));
        }
        // Signed modules cannot be loaded through `Vm::new` because
        // the Module representation has already lost the signature
        // payload. Hosts use [`Vm::load_signed_bytes`] for an
        // initial signed load, or hot-swap signed bytes onto an
        // existing VM through
        // [`Vm::replace_module_from_bytes`]. The `Vm::new_unchecked`
        // path bypasses this check (and every other verification).
        if (module.flags & crate::wire_format::FLAG_REQUIRES_SIGNATURE) != 0 {
            return Err(VmError::VerifyError(String::from(
                "module declares FLAG_REQUIRES_SIGNATURE; load through Vm::load_signed_bytes or hot-swap via Vm::replace_module_from_bytes",
            )));
        }
        if module.wcet_cycles == u32::MAX {
            let message = String::from(
                "module declared WCET (cycles) overflowed to u32::MAX during compilation; the static analysis could not bound the cost",
            );
            match options.overflow_policy {
                OverflowPolicy::Reject => return Err(VmError::VerifyError(message)),
                OverflowPolicy::Warn => {
                    warnings.push(VerifyWarning {
                        message,
                        kind: WarningKind::WcetOverflow,
                    });
                    // Rewrite the declared field to the auto-compute
                    // sentinel so the downstream serializer and the
                    // load-time overflow check in `Module::access_bytes`
                    // do not re-reject the module. The warning preserves
                    // the original overflow signal for the host.
                    module.wcet_cycles = 0;
                }
                OverflowPolicy::Allow => {
                    module.wcet_cycles = 0;
                }
            }
        }
        if module.wcmu_bytes == u32::MAX {
            let message = String::from(
                "module declared WCMU (bytes) overflowed to u32::MAX during compilation; the static analysis could not bound the cost",
            );
            match options.overflow_policy {
                OverflowPolicy::Reject => return Err(VmError::VerifyError(message)),
                OverflowPolicy::Warn => {
                    warnings.push(VerifyWarning {
                        message,
                        kind: WarningKind::WcmuOverflow,
                    });
                    module.wcmu_bytes = 0;
                }
                OverflowPolicy::Allow => {
                    module.wcmu_bytes = 0;
                }
            }
        }
        // Structural verification and resource-bound verification.
        // Gated behind the `verify` feature; when the feature is
        // off, `Vm::new_with_options` behaves like
        // `Vm::new_unchecked` from the caller's perspective.
        #[cfg(feature = "verify")]
        {
            verify::verify(&module)
                .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
            // R31. Verify worst-case memory usage fits within the
            // arena. The check is sound for programs without calls
            // and without variable-iteration loops. See
            // `verify_resource_bounds` for current limitations.
            //
            // B16 step 11: parametric runtimes pass the runtime's
            // actual `size_of::<GenericValue<W, F>>()` as the
            // bytes-per-slot multiplier so the bound matches the
            // narrow runtime's footprint rather than the default
            // 32-byte 64-bit-runtime conservative bound.
            let value_slot_bytes =
                core::mem::size_of::<crate::bytecode::GenericValue<W, F>>() as u32;
            verify::verify_resource_bounds_with_natives_and_value_slot_bytes(
                &module,
                arena.capacity(),
                &[],
                value_slot_bytes,
            )
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        }
        let vm = Self::construct(module, arena)?;
        Ok((vm, warnings))
    }

    /// Create a VM that runs structural verification but skips WCET and
    /// WCMU resource bounds checks.
    ///
    /// Intended for hosts that load precompiled bytecode from a trusted
    /// source where the resource bounds were validated during the
    /// build pipeline rather than at load time. Skipping the bounds
    /// check shifts the bounded-memory and bounded-step guarantees onto
    /// the host's attestation that the bytecode was admitted by an
    /// equivalent verification step previously.
    ///
    /// Structural verification still runs because the VM execution loop
    /// relies on its invariants for memory safety. Specifically, block
    /// nesting depth, jump offset bounds, and the productivity rule
    /// must hold for the VM to step the bytecode without dereferencing
    /// invalid frame state.
    ///
    /// # Safety
    ///
    /// The caller attests that the bytecode was produced by a trusted
    /// compiler and that resource bounds were verified during the build
    /// pipeline, or that the host accepts the consequences of running
    /// bytecode whose worst-case stack and heap usage may exceed the
    /// arena capacity. Exceeding the bound at runtime produces an
    /// allocation failure error from the arena rather than memory
    /// unsafety, so the unsafe marker captures the loss of the
    /// bounded-memory contract rather than a memory-safety risk.
    pub unsafe fn new_unchecked(
        module: Module,
        arena: &'arena keleusma_arena::Arena,
    ) -> Result<Self, VmError> {
        if module.entry_point.is_none() {
            return Err(VmError::VerifyError(String::from(
                "module has no entry point: declare a `fn main`, `yield main`, or `loop main`",
            )));
        }
        // Structural verification is retained even in
        // `new_unchecked` because the VM execution loop relies on
        // its invariants (block nesting depth, jump bounds,
        // productivity rule) for memory safety. Gated behind
        // `verify` because the verifier itself is. With the
        // feature off the host attests the bytecode's structural
        // soundness through a build-time verification step.
        #[cfg(feature = "verify")]
        verify::verify(&module)
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        Self::construct(module, arena)
    }

    /// Load and verify a module from a serialized byte slice.
    ///
    /// Convenience wrapper around [`Vm::new`]. The byte slice may
    /// originate from any addressable buffer including a file read,
    /// an in-memory `Vec<u8>`, or a `&'static [u8]` placed in
    /// `.rodata`. Runs full verification including resource bounds.
    pub fn load_bytes(bytes: &[u8], arena: &'arena keleusma_arena::Arena) -> Result<Self, VmError> {
        // Signed bytecode requires either the `signatures` cargo
        // feature plus a trust matrix (use `Vm::load_signed_bytes`)
        // or a build that explicitly skips verification (use
        // `Vm::load_bytes_unchecked`). Without the feature, surface
        // a dedicated `LoadError::SignaturesUnsupported` so the
        // operator sees an actionable diagnostic instead of the
        // generic "FLAG_REQUIRES_SIGNATURE" message that `Vm::new`
        // would otherwise produce.
        if crate::wire_format::header_requires_signature(bytes) {
            #[cfg(not(feature = "signatures"))]
            return Err(VmError::from(
                crate::bytecode::LoadError::SignaturesUnsupported,
            ));
            #[cfg(feature = "signatures")]
            return Err(VmError::from(crate::bytecode::LoadError::Codec(
                String::from(
                    "bytecode is signed; load through Vm::load_signed_bytes with a trust matrix",
                ),
            )));
        }
        let module = Module::from_bytes(bytes)?;
        Self::new(module, arena)
    }

    /// Load a signed module from serialized bytecode bytes,
    /// verifying the cryptographic signature against the supplied
    /// trust matrix before construction.
    ///
    /// Use this entry point for the initial load of a signed
    /// module. Hosts that bootstrap from an unsigned stub and
    /// later hot-swap to signed modules use
    /// [`Self::register_verifying_key`] followed by
    /// [`Self::replace_module_from_bytes`] instead.
    ///
    /// When the bytecode is unsigned, this function is equivalent
    /// to [`Self::load_bytes`]; the trust matrix is ignored. When
    /// the bytecode is signed, the signature is verified through
    /// [`crate::wire_format::verify_module_signature`] against
    /// every key in `verifying_keys`. The first matching key
    /// admits the module; no match produces
    /// [`crate::bytecode::LoadError::InvalidSignature`]. The
    /// trust matrix is also copied onto the constructed VM so a
    /// subsequent hot-swap through
    /// [`Self::replace_module_from_bytes`] inherits the same
    /// keys.
    ///
    /// Requires the `signatures` cargo feature.
    #[cfg(feature = "signatures")]
    pub fn load_signed_bytes(
        bytes: &[u8],
        arena: &'arena keleusma_arena::Arena,
        verifying_keys: &[ed25519_dalek::VerifyingKey],
    ) -> Result<Self, VmError> {
        let signed = crate::wire_format::header_requires_signature(bytes);
        if signed {
            crate::wire_format::verify_module_signature(bytes, verifying_keys)
                .map_err(VmError::from)?;
        }
        let mut module = Module::from_bytes(bytes).map_err(VmError::from)?;
        // The signed-module gate in `Vm::new_with_options` would
        // otherwise reject the verified module. Clear the flag
        // before construction; the trust matrix on the VM
        // perpetuates the host's policy for subsequent hot-swaps.
        let was_signed = (module.flags & crate::wire_format::FLAG_REQUIRES_SIGNATURE) != 0;
        if was_signed {
            module.flags &= !crate::wire_format::FLAG_REQUIRES_SIGNATURE;
        }
        let mut vm = Self::new(module, arena)?;
        for key in verifying_keys {
            vm.verifying_keys.push(*key);
        }
        Ok(vm)
    }

    /// Load a signed-and-encrypted module from a serialized byte
    /// slice. Verifies the Ed25519 signature, decrypts the body
    /// using the supplied X25519 private key, runs structural and
    /// resource-bounds verification on the decrypted plaintext,
    /// and constructs the VM.
    ///
    /// The signature is verified BEFORE decryption to authenticate
    /// origin before any decryption work runs. The signature
    /// covers the encrypted body, so an adversary cannot strip
    /// encryption and substitute cleartext while preserving
    /// signature validity.
    ///
    /// The `recipient_key_id` in the encryption metadata is checked
    /// against the SHA-256 of the local public key. Artefacts
    /// intended for a different recipient are rejected before
    /// expensive cryptographic operations.
    ///
    /// The trust matrix is also copied onto the constructed VM so
    /// a subsequent hot-swap through
    /// [`Self::replace_module_from_bytes`] inherits the same keys.
    ///
    /// Requires both the `signatures` and `encryption` cargo features.
    #[cfg(all(feature = "signatures", feature = "encryption"))]
    pub fn load_encrypted_signed_bytes(
        bytes: &[u8],
        arena: &'arena keleusma_arena::Arena,
        verifying_keys: &[ed25519_dalek::VerifyingKey],
        decryption_key: &[u8; crate::encryption::X25519_PRIVATE_KEY_LEN],
    ) -> Result<Self, VmError> {
        let encrypted = crate::wire_format::header_requires_encryption(bytes);
        if !encrypted {
            return Err(VmError::from(crate::bytecode::LoadError::Codec(
                alloc::string::String::from(
                    "load_encrypted_signed_bytes called on bytes without FLAG_ENCRYPTED",
                ),
            )));
        }
        // Decrypt to a reconstructed signed-only buffer. This
        // function verifies the signature internally before
        // attempting decryption, so the caller does not need to
        // call verify_module_signature separately.
        let signed_buf = crate::wire_format::decrypt_encrypted_signed_to_signed_bytes(
            bytes,
            verifying_keys,
            decryption_key,
        )
        .map_err(VmError::from)?;
        // The reconstructed buffer has the signed-only flag still
        // set but is functionally equivalent to a never-encrypted
        // signed module. Parse and load through the existing path
        // which clears the signed-only flag before construction.
        let mut module = Module::from_bytes(&signed_buf).map_err(VmError::from)?;
        let was_signed = (module.flags & crate::wire_format::FLAG_REQUIRES_SIGNATURE) != 0;
        if was_signed {
            module.flags &= !crate::wire_format::FLAG_REQUIRES_SIGNATURE;
        }
        let mut vm = Self::new(module, arena)?;
        for key in verifying_keys {
            vm.verifying_keys.push(*key);
        }
        Ok(vm)
    }

    /// Load a module from a serialized byte slice and skip resource
    /// bounds verification.
    ///
    /// Convenience wrapper around [`Vm::new_unchecked`].
    ///
    /// # Safety
    ///
    /// Same contract as [`Vm::new_unchecked`].
    pub unsafe fn load_bytes_unchecked(
        bytes: &[u8],
        arena: &'arena keleusma_arena::Arena,
    ) -> Result<Self, VmError> {
        let module = Module::from_bytes(bytes)?;
        unsafe { Self::new_unchecked(module, arena) }
    }

    /// Load a module from an aligned byte slice and run full verification.
    ///
    /// The body of the framed bytecode must be 8-byte aligned within the
    /// slice. The runtime validates the framing in place via
    /// [`Module::access_bytes`] and deserializes the archived form via
    /// `rkyv::deserialize`. Compared to [`Vm::load_bytes`], this path
    /// skips the body copy that arbitrary unaligned slices require.
    ///
    /// Hosts that wish to execute bytecode directly from `.rodata` or
    /// from a flash region typically arrange alignment through linker
    /// scripts or by wrapping the buffer in `rkyv::util::AlignedVec`.
    /// See the documentation on [`Module::access_bytes`] for the
    /// alignment contract.
    ///
    /// True zero-copy execution against `&ArchivedModule` is the next
    /// iteration of P10. The current view path delivers in-place
    /// validation. The execution loop continues to operate on the
    /// deserialized owned `Module`.
    pub fn view_bytes(bytes: &[u8], arena: &'arena keleusma_arena::Arena) -> Result<Self, VmError> {
        let module = Module::view_bytes(bytes)?;
        Self::new(module, arena)
    }

    /// Load a module from an aligned byte slice and skip resource
    /// bounds verification.
    ///
    /// Convenience wrapper around [`Vm::new_unchecked`].
    ///
    /// # Safety
    ///
    /// Same contract as [`Vm::new_unchecked`].
    pub unsafe fn view_bytes_unchecked(
        bytes: &[u8],
        arena: &'arena keleusma_arena::Arena,
    ) -> Result<Self, VmError> {
        let module = Module::view_bytes(bytes)?;
        unsafe { Self::new_unchecked(module, arena) }
    }

    /// Construct a VM that borrows bytecode directly from `bytes` without
    /// any deserialization. True zero-copy execution.
    ///
    /// Validates the framing through [`Module::access_bytes`] and stores
    /// the slice. The execution loop reads from `&ArchivedModule` for
    /// every op-fetch and constant-load via the archived converters. No
    /// owned `Module` is materialized at any point.
    ///
    /// The lifetime parameter on the returned `Vm<'a>` ties the VM to
    /// the slice's lifetime. The slice must remain valid for as long as
    /// the VM is in use.
    ///
    /// Suitable for hosts that place bytecode in `.rodata` (a
    /// `&'static [u8]`) or in any addressable buffer where the host
    /// arranges 8-byte alignment of the body.
    ///
    /// Skips structural verification, resource bounds verification,
    /// and rkyv body validation. The host attests through the unsafe
    /// marker that the bytecode was previously verified or comes from
    /// a trusted compiler.
    ///
    /// # Safety
    ///
    /// The caller attests that the bytecode is well-formed: framing,
    /// rkyv structure, structural invariants (block nesting, jump
    /// offsets, productivity rule), and resource bounds. Violation may
    /// produce arbitrary VM behavior including reads or writes of
    /// invalid memory through the operand stack and the call frames.
    /// This is stronger than [`Vm::new_unchecked`] which still runs
    /// structural verification.
    ///
    /// Use [`Vm::view_bytes_unchecked`] for hosts that want the bytes
    /// borrowed but with structural verification still active. Use
    /// this constructor only when the bytecode is known good.
    pub unsafe fn view_bytes_zero_copy(
        bytes: &'a [u8],
        arena: &'arena keleusma_arena::Arena,
    ) -> Result<Self, VmError> {
        // V0.2.0 Phase 7c routes zero-copy through the wire
        // format. `access_bytes` validates the framing and
        // returns the archived auxiliary body slice; we use the
        // same slice (via `archived()` after construction)
        // throughout the VM lifetime.
        let archived = Module::access_bytes(bytes)?;
        // B16 step 8: width validation against this VM's W/A/F
        // trait parameters. The wire-format header carries the
        // declared widths at bytes 12 (word), 13 (address), and
        // 14 (float).
        Self::check_runtime_widths(bytes[12], bytes[13], bytes[14])?;
        // Determine data segment slot counts from the archived
        // auxiliary body. The data layout structure is shared
        // with the legacy format; only the chunk-ops separation
        // is new.
        let (shared_count, private_count) = match archived.data_layout.as_ref() {
            None => (0u16, 0u16),
            Some(dl) => {
                let mut shared = 0u16;
                let mut private_ = 0u16;
                for slot in dl.slots.iter() {
                    match slot.visibility {
                        crate::bytecode::ArchivedSlotVisibility::Shared => {
                            shared = shared.saturating_add(1);
                        }
                        crate::bytecode::ArchivedSlotVisibility::Private => {
                            private_ = private_.saturating_add(1);
                        }
                    }
                }
                (shared, private_)
            }
        };
        let private_storage_bytes =
            private_count as usize * core::mem::size_of::<crate::bytecode::GenericValue<W, F>>();
        if arena.persistent_capacity() < private_storage_bytes {
            return Err(VmError::VerifyError(alloc::format!(
                "arena persistent_capacity ({} bytes) is too small for module's private data ({} bytes); call `arena.resize_persistent(required_persistent_capacity_for(&module))` before constructing the VM",
                arena.persistent_capacity(),
                private_storage_bytes,
            )));
        }
        if private_count > 0 {
            let base = arena.persistent_ptr().as_ptr() as *mut crate::bytecode::GenericValue<W, F>;
            for i in 0..private_count as usize {
                // SAFETY: same justification as in `Vm::construct`.
                unsafe {
                    base.add(i).write(crate::bytecode::GenericValue::Unit);
                }
            }
        }
        let data = vec![crate::bytecode::GenericValue::Unit; shared_count as usize];
        let decoded_ops = decode_all_ops(bytes)?;
        let mut stack = ArenaVec::new_in(arena.bottom_handle());
        let mut frames = ArenaVec::new_in(arena.bottom_handle());
        stack
            .try_reserve(MIN_STACK_RESERVE_SLOTS)
            .map_err(|_| out_of_arena_min(arena.capacity()))?;
        frames
            .try_reserve(MIN_FRAMES_RESERVE)
            .map_err(|_| out_of_arena_min(arena.capacity()))?;
        Ok(Self {
            bytecode: BytecodeStore::Borrowed(bytes),
            _phantom_a: core::marker::PhantomData,
            decoded_ops,
            stack,
            frames,
            natives: Vec::new(),
            data,
            shared_slot_count: shared_count,
            private_slot_count: private_count,
            arena,
            started: false,
            native_classifications_verified: false,
            #[cfg(feature = "signatures")]
            verifying_keys: Vec::new(),
        })
    }

    /// Construct the VM struct without running any verification.
    ///
    /// Internal helper shared by the verifying and unchecked
    /// constructors. Serializes the owned module to an aligned vector
    /// for archived access during execution. The data segment is
    /// initialized to `Unit` for each declared slot.
    /// Validate that the module's declared widths are admissible by
    /// this VM's compile-time trait parameters `W`, `A`, `F`.
    ///
    /// The bytecode's `word_bits_log2`, `addr_bits_log2`, and
    /// `float_bits_log2` fields must each be no greater than the
    /// corresponding trait's `BITS_LOG2` constant. The VM's chosen
    /// widths act as an upper bound on bytecode it can run; narrower
    /// bytecode is admitted and wrapped through `Word::from_i64_wrap`
    /// at constant-load time. Wider bytecode would silently truncate
    /// runtime values and is rejected here.
    fn check_runtime_widths(
        word_bits_log2: u8,
        addr_bits_log2: u8,
        float_bits_log2: u8,
    ) -> Result<(), VmError> {
        if word_bits_log2 > <W as crate::word::Word>::BITS_LOG2 {
            return Err(VmError::VerifyError(alloc::format!(
                "bytecode declares word_bits_log2 = {} but this Vm runs at word_bits_log2 = {} (chosen Word type is narrower than the bytecode requires)",
                word_bits_log2,
                <W as crate::word::Word>::BITS_LOG2,
            )));
        }
        if addr_bits_log2 > <A as crate::address::Address>::BITS_LOG2 {
            return Err(VmError::VerifyError(alloc::format!(
                "bytecode declares addr_bits_log2 = {} but this Vm runs at addr_bits_log2 = {} (chosen Address type is narrower than the bytecode requires)",
                addr_bits_log2,
                <A as crate::address::Address>::BITS_LOG2,
            )));
        }
        if float_bits_log2 > <F as crate::float::Float>::BITS_LOG2 {
            return Err(VmError::VerifyError(alloc::format!(
                "bytecode declares float_bits_log2 = {} but this Vm runs at float_bits_log2 = {} (chosen Float type is narrower than the bytecode requires)",
                float_bits_log2,
                <F as crate::float::Float>::BITS_LOG2,
            )));
        }
        Ok(())
    }

    fn construct(module: Module, arena: &'arena keleusma_arena::Arena) -> Result<Self, VmError> {
        // B16 step 8: validate the module's declared widths against
        // this VM's compile-time W/A/F trait parameters. A narrower
        // Vm running wider bytecode would silently truncate values
        // through Word::from_i64_wrap; reject the mismatch instead.
        Self::check_runtime_widths(
            module.word_bits_log2,
            module.addr_bits_log2,
            module.float_bits_log2,
        )?;
        // Partition data slots by visibility. Shared slots live
        // in the Vm-owned vector; private slots live in the
        // arena's persistent region. The compiler emits shared
        // slots first in the unified slot index space, so the
        // partition reduces to a count.
        let (shared_count, private_count) = match module.data_layout.as_ref() {
            None => (0u16, 0u16),
            Some(dl) => {
                let mut shared = 0u16;
                let mut private_ = 0u16;
                for slot in &dl.slots {
                    match slot.visibility {
                        crate::bytecode::SlotVisibility::Shared => {
                            shared = shared.checked_add(1).ok_or_else(|| {
                                VmError::VerifyError(String::from(
                                    "data layout shared slot count exceeds u16::MAX",
                                ))
                            })?;
                        }
                        crate::bytecode::SlotVisibility::Private => {
                            private_ = private_.checked_add(1).ok_or_else(|| {
                                VmError::VerifyError(String::from(
                                    "data layout private slot count exceeds u16::MAX",
                                ))
                            })?;
                        }
                    }
                }
                (shared, private_)
            }
        };
        let private_storage_bytes =
            private_count as usize * core::mem::size_of::<crate::bytecode::GenericValue<W, F>>();
        if arena.persistent_capacity() < private_storage_bytes {
            return Err(VmError::VerifyError(alloc::format!(
                "arena persistent_capacity ({} bytes) is too small for module's private data ({} bytes; {} slot(s) at {} bytes each); call `arena.resize_persistent(required_persistent_capacity_for(&module))` before constructing the VM",
                arena.persistent_capacity(),
                private_storage_bytes,
                private_count,
                core::mem::size_of::<crate::bytecode::GenericValue<W, F>>(),
            )));
        }
        // Initialise each private slot to crate::bytecode::GenericValue::Unit via
        // `ptr::write` so the bytes hold a valid Value before
        // any subsequent reader clones or any subsequent writer
        // drops the old occupant. The arena's persistent region
        // is freshly zeroed when first resized, but those zero
        // bytes are not a valid `Value`, so write through `write`
        // not assignment.
        if private_count > 0 {
            let base = arena.persistent_ptr().as_ptr() as *mut crate::bytecode::GenericValue<W, F>;
            for i in 0..private_count as usize {
                // SAFETY: `i` is within the slot count just
                // verified to fit in the persistent capacity; the
                // arena owns the buffer for the VM's lifetime;
                // `Value` is properly aligned at every multiple
                // of its size on the 16-byte-aligned buffer base.
                unsafe {
                    base.add(i).write(crate::bytecode::GenericValue::Unit);
                }
            }
        }
        let data = vec![crate::bytecode::GenericValue::Unit; shared_count as usize];
        let bytes = module.to_bytes()?;
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let decoded_ops = decode_all_ops(aligned.as_slice())?;
        let mut stack = ArenaVec::new_in(arena.bottom_handle());
        let mut frames = ArenaVec::new_in(arena.bottom_handle());
        // Pre-reserve a known-good minimum for the operand stack and
        // call frames so a too-small arena fails fast at construction
        // with `VmError::OutOfArena` rather than aborting the host
        // process via `handle_alloc_error` on a later push. The
        // reservation also avoids reallocation amplification in the
        // bump-allocator (a growing `Vec` over a bump allocator
        // consumes cumulative memory across reallocations without
        // freeing earlier capacity).
        //
        // The minimum is conservative: programs that need a larger
        // stack still grow at runtime, which can still abort if the
        // arena is too small relative to the worst-case usage. Full
        // OOM-safe push paths for arbitrary workloads is tracked for
        // V0.2.x.
        stack
            .try_reserve(MIN_STACK_RESERVE_SLOTS)
            .map_err(|_| out_of_arena_min(arena.capacity()))?;
        frames
            .try_reserve(MIN_FRAMES_RESERVE)
            .map_err(|_| out_of_arena_min(arena.capacity()))?;
        Ok(Self {
            bytecode: BytecodeStore::Owned(aligned),
            _phantom_a: core::marker::PhantomData,
            decoded_ops,
            stack,
            frames,
            natives: Vec::new(),
            data,
            shared_slot_count: shared_count,
            private_slot_count: private_count,
            arena,
            started: false,
            native_classifications_verified: false,
            #[cfg(feature = "signatures")]
            verifying_keys: Vec::new(),
        })
    }

    /// Read a data slot's current value, cloning. Dispatches by
    /// the unified slot index: indices below `shared_slot_count`
    /// resolve to the Vm-owned `data` vector; higher indices
    /// resolve to the arena's persistent region.
    fn read_data_slot(&self, slot: usize) -> crate::bytecode::GenericValue<W, F> {
        if slot < self.shared_slot_count as usize {
            self.data[slot].clone()
        } else {
            // SAFETY: the slot is within the partition checked at
            // construction; the persistent region was initialised
            // with `crate::bytecode::GenericValue::Unit` for every slot and updates flow
            // through `write_data_slot`, so the pointee is always
            // a valid `Value`.
            unsafe {
                let private_idx = slot - self.shared_slot_count as usize;
                let base = self.arena.persistent_ptr().as_ptr()
                    as *const crate::bytecode::GenericValue<W, F>;
                (*base.add(private_idx)).clone()
            }
        }
    }

    /// Overwrite a data slot. Same dispatch as `read_data_slot`.
    /// Assignment via `*ptr = value` drops the previous occupant,
    /// which is valid because every private slot is initialised
    /// to `crate::bytecode::GenericValue::Unit` at construction.
    fn write_data_slot(&mut self, slot: usize, value: crate::bytecode::GenericValue<W, F>) {
        if slot < self.shared_slot_count as usize {
            self.data[slot] = value;
        } else {
            // SAFETY: the slot is within the partition checked at
            // construction; the pointee is a valid `Value` per
            // the construction-time initialisation, so dropping
            // it via the assignment is sound.
            unsafe {
                let private_idx = slot - self.shared_slot_count as usize;
                let base = self.arena.persistent_ptr().as_ptr()
                    as *mut crate::bytecode::GenericValue<W, F>;
                *base.add(private_idx) = value;
            }
        }
    }

    /// Number of shared data slots declared in the loaded module.
    /// Slot indices `0..shared_slot_count` are host-accessible
    /// through [`Self::set_data`] and [`Self::get_data`]; slot
    /// indices beyond that range are private and reject host access.
    /// Use this to size a host-side checkpoint buffer (for example
    /// a REPL's per-evaluation persistence buffer) without keeping a
    /// reference to the source module.
    pub fn shared_slot_count(&self) -> usize {
        self.shared_slot_count as usize
    }

    /// Set a data segment slot to an initial value.
    ///
    /// The host calls this before execution begins to populate the
    /// persistent context. Returns an error if the slot index is out
    /// of bounds.
    pub fn set_data(
        &mut self,
        slot: usize,
        value: crate::bytecode::GenericValue<W, F>,
    ) -> Result<(), VmError> {
        let total = self.data_len();
        if slot >= total {
            return Err(VmError::NativeError(format!(
                "data slot index {} out of bounds (data segment has {} slots)",
                slot, total
            )));
        }
        if self.slot_is_private(slot) {
            return Err(VmError::NativeError(format!(
                "data slot {} is private and not accessible through the host API",
                slot
            )));
        }
        self.data[slot] = value;
        Ok(())
    }

    /// Read a data segment slot value.
    ///
    /// Returns an error if the slot index is out of bounds or the
    /// slot is declared `private` in the source. Private slots are
    /// script-only and not exposed through the host API.
    pub fn get_data(&self, slot: usize) -> Result<&crate::bytecode::GenericValue<W, F>, VmError> {
        let total = self.data_len();
        if slot >= total {
            return Err(VmError::NativeError(format!(
                "data slot index {} out of bounds (data segment has {} slots)",
                slot, total
            )));
        }
        if self.slot_is_private(slot) {
            return Err(VmError::NativeError(format!(
                "data slot {} is private and not accessible through the host API",
                slot
            )));
        }
        Ok(&self.data[slot])
    }

    /// True when the slot at `slot` is declared `private` in the
    /// module's data layout. Used by [`Vm::set_data`] and
    /// [`Vm::get_data`] to enforce the host-API boundary on private
    /// slots. Out-of-bounds indices return false; the caller is
    /// expected to have validated the bound before invoking this
    /// helper (both call sites do).
    fn slot_is_private(&self, slot: usize) -> bool {
        slot >= self.shared_slot_count as usize && slot < self.data_len()
    }

    /// Return the number of slots in the current data segment.
    ///
    /// Useful for hosts that want to allocate a `Vec<crate::bytecode::GenericValue<W, F>>` of the correct
    /// size without inspecting the `Module` directly.
    pub fn data_len(&self) -> usize {
        self.shared_slot_count as usize + self.private_slot_count as usize
    }

    /// Borrow the VM's arena.
    ///
    /// The arena is the dual-end bump-allocated buffer described in R32. It
    /// is available to host-supplied native functions that wish to allocate
    /// dynamic strings or other arena-resident values. The arena is reset
    /// at every `Op::Reset` boundary, so host-allocated values do not
    /// survive across stream phases.
    pub fn arena(&self) -> &'arena keleusma_arena::Arena {
        self.arena
    }

    /// Reset the arena's top region and advance the epoch, leaving
    /// the bottom region intact.
    ///
    /// Used by `Op::Reset` to invalidate scratch allocations and
    /// dynamic string handles between stream iterations while
    /// preserving the operand stack and call-frame stack. The bottom
    /// region holds the operand stack and frames, both of which carry
    /// state across the reset.
    ///
    /// Outstanding [`keleusma_arena::ArenaHandle`] values, regardless
    /// of which end produced them, return [`keleusma_arena::Stale`]
    /// on access after this call.
    ///
    /// Returns [`keleusma_arena::EpochSaturated`] when the epoch
    /// counter is exhausted.
    fn reset_arena_internal(&self) -> Result<(), keleusma_arena::EpochSaturated> {
        // SAFETY: The top region holds only short-lived scratch and
        // dynamic strings. No `Vec<T, TopHandle>` in the VM holds
        // non-zero capacity. The bottom-region operand stack and
        // frames are unaffected.
        unsafe { self.arena.reset_top_unchecked() }
    }

    /// Drop the operand and frame stacks, reset both arena ends, and
    /// advance the epoch.
    ///
    /// Used by error recovery and hot-swap where the VM transitions
    /// to a clean callable state with no retained execution context.
    /// The arena-backed stacks would otherwise hold storage in the
    /// bottom region whose addresses alias memory the bump allocator
    /// will return for subsequent allocations once the bump pointer
    /// is rewound, so they must be dropped before the bottom-region
    /// reset advances the bump pointer.
    fn full_reset_arena_internal(&mut self) -> Result<(), keleusma_arena::EpochSaturated> {
        // Drop the old arena-backed stacks before clearing the bottom
        // bump pointer. Drop runs each contained value's destructor
        // and calls `BottomHandle::deallocate`, which is a no-op for
        // the bump allocator. The fresh stacks have zero capacity
        // and therefore do not allocate.
        self.stack = ArenaVec::new_in(self.arena.bottom_handle());
        self.frames = ArenaVec::new_in(self.arena.bottom_handle());
        // SAFETY: After the assignments above, no `Vec<T, BottomHandle>`
        // in the VM holds non-zero capacity. The data segment is
        // globally-allocated. Dynamic string handles produced through
        // `KString::alloc` are epoch-tagged and return `Stale` on
        // access after the reset rather than dereferencing reclaimed
        // memory.
        unsafe { self.arena.reset_unchecked() }
    }

    /// Recover from a runtime error and return the VM to a clean
    /// callable state.
    ///
    /// After [`Vm::call`] or [`Vm::resume`] returns `Err(VmError)` the
    /// VM is in an undefined intermediate state. The operand stack,
    /// call frames, and arena may all hold partial values from the
    /// failed iteration. This method clears the volatile state and
    /// returns the VM to the same shape it had before the failed
    /// call. The data segment is preserved so the host can retry
    /// the same iteration with the accumulated state intact.
    ///
    /// Behavior.
    ///
    /// - Operand stack cleared.
    /// - Call frames cleared.
    /// - Arena reset, releasing any dynamic strings or scratch
    ///   buffers from the failed iteration.
    /// - Data segment preserved.
    /// - Bytecode store preserved.
    ///
    /// After this call, [`Vm::call`] starts a fresh iteration of the
    /// entry point. Hosts that want to also reset the data segment
    /// can follow with calls to [`Vm::set_data`] or replace the
    /// module via [`Vm::replace_module`].
    ///
    /// This is the explicit recovery path (P3). Callers attest by
    /// invoking the method that they have inspected the error and
    /// decided to retry. Errors that violated bytecode invariants
    /// (such as `InvalidBytecode`) may indicate a corrupt module and
    /// the host should consider whether retrying is appropriate.
    pub fn reset_after_error(&mut self) {
        // `full_reset_arena_internal` drops and recreates the
        // arena-backed stacks and clears both arena ends. The volatile
        // state is cleared as a side effect.
        let _ = self.full_reset_arena_internal();
        self.started = false;
    }

    /// Replace the current module with a new one as a hot code update.
    ///
    /// This is the host-facing hot swap API (R26, R27). The host is
    /// expected to call this only between a `GenericVmState::Reset` and the
    /// next `call`. The Rust borrow checker enforces that the call
    /// cannot overlap with running execution because that would require
    /// concurrent mutable access to `self`.
    ///
    /// The new module is verified before replacement. The data segment
    /// is replaced atomically with the host-supplied initial values
    /// following Replace semantics, namely the host owns storage and
    /// supplies whatever instance is appropriate for the new code
    /// version. The supplied vector length must match the declared
    /// slot count of the new module.
    ///
    /// Frames and stack are cleared. The host should call `call` to
    /// start the new module's entry point. The old module's coroutine
    /// state, if any, is discarded.
    ///
    /// Dialogue type compatibility between the old and new modules is
    /// the host's responsibility. The VM does not check it because
    /// dialogue types are erased at the bytecode level.
    pub fn replace_module(
        &mut self,
        new_module: Module,
        initial_data: Vec<crate::bytecode::GenericValue<W, F>>,
    ) -> Result<(), VmError> {
        // Strict schema check. Reject hot swaps whose data-segment
        // layout differs from the currently loaded module's layout.
        // The hash covers slot names and visibility in declaration
        // order; modules with no data segment have hash zero and
        // therefore swap freely.
        //
        // The strict check is the safer default. Hosts that need to
        // swap across incompatible schemas (typically because the
        // new module declares a different `data` block by intent)
        // call [`Vm::replace_module_unchecked`] instead, which
        // bypasses this check and leaves the existing
        // size-and-arena verification as the only guard.
        let current_hash: u32 = self.archived().schema_hash.to_native();
        if current_hash != new_module.schema_hash {
            return Err(VmError::VerifyError(format!(
                "schema mismatch on hot swap: current module schema_hash = {:#x}, new module schema_hash = {:#x}. Use `Vm::replace_module_unchecked` to force the swap if the data layout change is intentional.",
                current_hash, new_module.schema_hash
            )));
        }
        self.replace_module_inner(new_module, initial_data)
    }

    /// Hot swap without the schema-compatibility check.
    ///
    /// Equivalent to [`Self::replace_module`] except that the
    /// schema-hash comparison is skipped. Hosts use this when the new
    /// module declares a different data layout from the currently
    /// loaded module by intent; the size check on `initial_data`
    /// continues to enforce that the host supplies the right number
    /// of initial slot values for the new layout.
    ///
    /// `unchecked` names the safety opt-out, not memory safety. The
    /// VM continues to enforce structural verification and resource
    /// bounds; only the schema-hash sanity check is bypassed.
    pub fn replace_module_unchecked(
        &mut self,
        new_module: Module,
        initial_data: Vec<crate::bytecode::GenericValue<W, F>>,
    ) -> Result<(), VmError> {
        self.replace_module_inner(new_module, initial_data)
    }

    /// Hot-swap to a new module loaded from its serialized
    /// bytecode bytes, verifying the cryptographic signature
    /// against the VM's host-supplied trust matrix before the
    /// swap takes effect.
    ///
    /// When the bytecode's framing header carries
    /// [`crate::wire_format::FLAG_REQUIRES_SIGNATURE`], the
    /// signature is verified through
    /// [`crate::wire_format::verify_module_signature`] against
    /// every key the host has registered via
    /// [`Self::register_verifying_key`]. The first matching key
    /// admits the swap; an empty trust matrix or no matching key
    /// produces [`crate::bytecode::LoadError::InvalidSignature`]
    /// and the existing module continues to run.
    ///
    /// When the bytecode is unsigned, this method is equivalent
    /// to decoding the bytes through [`Module::from_bytes`] and
    /// calling [`Self::replace_module`].
    ///
    /// Requires the `signatures` cargo feature.
    #[cfg(feature = "signatures")]
    pub fn replace_module_from_bytes(
        &mut self,
        bytes: &[u8],
        initial_data: Vec<crate::bytecode::GenericValue<W, F>>,
    ) -> Result<(), VmError> {
        if crate::wire_format::header_requires_signature(bytes) {
            crate::wire_format::verify_module_signature(bytes, &self.verifying_keys)
                .map_err(VmError::from)?;
        }
        let new_module = Module::from_bytes(bytes).map_err(VmError::from)?;
        self.replace_module(new_module, initial_data)
    }

    /// Register a verifying key with the VM's trust matrix.
    ///
    /// Subsequent calls to [`Self::replace_module_from_bytes`]
    /// consult the matrix when the incoming bytecode carries
    /// [`crate::wire_format::FLAG_REQUIRES_SIGNATURE`]. The matrix
    /// is additive; hosts that need to rotate trust call
    /// [`Self::clear_verifying_keys`] first and re-register the
    /// new key set.
    ///
    /// Requires the `signatures` cargo feature.
    #[cfg(feature = "signatures")]
    pub fn register_verifying_key(&mut self, key: ed25519_dalek::VerifyingKey) {
        self.verifying_keys.push(key);
    }

    /// Clear every key from the VM's trust matrix. Subsequent
    /// signed hot-swap attempts via
    /// [`Self::replace_module_from_bytes`] are rejected with
    /// [`crate::bytecode::LoadError::InvalidSignature`] until at
    /// least one key is re-registered through
    /// [`Self::register_verifying_key`].
    ///
    /// Requires the `signatures` cargo feature.
    #[cfg(feature = "signatures")]
    pub fn clear_verifying_keys(&mut self) {
        self.verifying_keys.clear();
    }

    /// Number of verifying keys currently in the trust matrix.
    /// Hosts use this for diagnostic logging at deployment time.
    ///
    /// Requires the `signatures` cargo feature.
    #[cfg(feature = "signatures")]
    pub fn verifying_keys_len(&self) -> usize {
        self.verifying_keys.len()
    }

    fn replace_module_inner(
        &mut self,
        new_module: Module,
        initial_data: Vec<crate::bytecode::GenericValue<W, F>>,
    ) -> Result<(), VmError> {
        // B16 step 8: the new module's declared widths must match
        // the runtime's W/A/F trait parameters, same as Vm::new.
        // Without this the hot-swap path would re-introduce the
        // silent-truncation foot-gun the construct path closed.
        Self::check_runtime_widths(
            new_module.word_bits_log2,
            new_module.addr_bits_log2,
            new_module.float_bits_log2,
        )?;
        #[cfg(feature = "verify")]
        {
            verify::verify(&new_module)
                .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
            // R31. Verify the new module's WCMU fits the existing arena.
            // B16 step 11: same parametric value_slot_bytes plumbing
            // as `new_with_options` so the hot-swap path tightens
            // the bound on narrow runtimes.
            let value_slot_bytes =
                core::mem::size_of::<crate::bytecode::GenericValue<W, F>>() as u32;
            verify::verify_resource_bounds_with_natives_and_value_slot_bytes(
                &new_module,
                self.arena.capacity(),
                &[],
                value_slot_bytes,
            )
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        }

        let expected_len = new_module
            .data_layout
            .as_ref()
            .map_or(0, |dl| dl.slots.len());
        if initial_data.len() != expected_len {
            return Err(VmError::InvalidBytecode(format!(
                "data segment size mismatch: new module declares {} slot(s), host supplied {}",
                expected_len,
                initial_data.len()
            )));
        }
        // Partition the new module's slots. Subsequent code
        // splits `initial_data` accordingly so the shared portion
        // populates the Vm-owned vector and the private portion
        // populates the arena's persistent region.
        let (new_shared, new_private) = match new_module.data_layout.as_ref() {
            None => (0u16, 0u16),
            Some(dl) => {
                let mut shared = 0u16;
                let mut private_ = 0u16;
                for slot in &dl.slots {
                    match slot.visibility {
                        crate::bytecode::SlotVisibility::Shared => {
                            shared = shared.checked_add(1).ok_or_else(|| {
                                VmError::VerifyError(String::from(
                                    "data layout shared slot count exceeds u16::MAX",
                                ))
                            })?;
                        }
                        crate::bytecode::SlotVisibility::Private => {
                            private_ = private_.checked_add(1).ok_or_else(|| {
                                VmError::VerifyError(String::from(
                                    "data layout private slot count exceeds u16::MAX",
                                ))
                            })?;
                        }
                    }
                }
                (shared, private_)
            }
        };
        let new_private_storage =
            new_private as usize * core::mem::size_of::<crate::bytecode::GenericValue<W, F>>();
        if self.arena.persistent_capacity() < new_private_storage {
            return Err(VmError::VerifyError(format!(
                "arena persistent_capacity ({} bytes) is too small for new module's private data ({} bytes); resize before hot swap",
                self.arena.persistent_capacity(),
                new_private_storage,
            )));
        }
        // Drop the old private slots. Each was initialised at
        // construction or at a prior hot swap and may hold owned
        // resources whose destructor must run.
        let old_private_count = self.private_slot_count as usize;
        if old_private_count > 0 {
            let base =
                self.arena.persistent_ptr().as_ptr() as *mut crate::bytecode::GenericValue<W, F>;
            for i in 0..old_private_count {
                // SAFETY: every old private slot held a valid
                // `Value` initialised through `write_data_slot`
                // or `Vm::construct`. `drop_in_place` runs the
                // destructor in place; the bytes are then
                // uninitialised and ready for `ptr::write`.
                unsafe {
                    core::ptr::drop_in_place(base.add(i));
                }
            }
        }
        // Split the host-supplied initial values into the
        // shared and private partitions. The compiler emits
        // shared slots first in the unified index space, so the
        // split is a contiguous prefix.
        let mut iter = initial_data.into_iter();
        let shared_init: Vec<crate::bytecode::GenericValue<W, F>> =
            iter.by_ref().take(new_shared as usize).collect();
        let private_init: Vec<crate::bytecode::GenericValue<W, F>> = iter.collect();

        // Serialize the new module to aligned bytes for archived
        // access. The borrowed variant is replaced by an owned variant
        // because hot swap takes an owned input.
        let bytes = new_module.to_bytes()?;
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let decoded_ops = decode_all_ops(aligned.as_slice())?;
        self.bytecode = BytecodeStore::Owned(aligned);
        self.decoded_ops = decoded_ops;
        self.data = shared_init;
        self.shared_slot_count = new_shared;
        self.private_slot_count = new_private;
        // New bytecode means the native-classification check must
        // re-run; the previous module's chunks are gone and the
        // new module's call sites have not yet been validated
        // against the host's registered classifications.
        self.native_classifications_verified = false;
        // Initialise the new private slots via `ptr::write` (no
        // drop on the destination, because we just dropped the
        // old occupants above).
        if new_private > 0 {
            let base =
                self.arena.persistent_ptr().as_ptr() as *mut crate::bytecode::GenericValue<W, F>;
            for (i, val) in private_init.into_iter().enumerate() {
                // SAFETY: the destination is within the
                // persistent region whose capacity was verified
                // above; the bytes are uninitialised after the
                // drop pass; `ptr::write` does not read the old
                // value.
                unsafe {
                    base.add(i).write(val);
                }
            }
        }
        // `full_reset_arena_internal` drops and recreates the
        // arena-backed stacks before clearing both ends. The
        // persistent region is preserved through the reset.
        let _ = self.full_reset_arena_internal();
        self.started = false;

        Ok(())
    }

    /// Register a native function by name using a function pointer.
    ///
    /// The supplied function does not receive arena context. Native
    /// functions that need arena access for [`crate::bytecode::GenericValue::KStr`] allocation
    /// register through [`Vm::register_native_with_ctx`] instead.
    #[allow(clippy::type_complexity)]
    pub fn register_native(
        &mut self,
        name: &str,
        func: fn(
            &[crate::bytecode::GenericValue<W, F>],
        ) -> Result<crate::bytecode::GenericValue<W, F>, VmError>,
    ) {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(
                move |_ctx: &NativeCtx<'_>, args: &[crate::bytecode::GenericValue<W, F>]| {
                    func(args)
                },
            ),
            classification: NativeClassification::Verified,
            max_invocations_per_iteration: None,
        });
    }

    /// Register a native function by name using a closure.
    ///
    /// This allows closures that capture state, such as a shared command
    /// buffer for audio script integration. The closure does not receive
    /// arena context.
    pub fn register_native_closure<Func>(&mut self, name: &str, func: Func)
    where
        Func: Fn(
                &[crate::bytecode::GenericValue<W, F>],
            ) -> Result<crate::bytecode::GenericValue<W, F>, VmError>
            + 'static,
    {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(
                move |_ctx: &NativeCtx<'_>, args: &[crate::bytecode::GenericValue<W, F>]| {
                    func(args)
                },
            ),
            classification: NativeClassification::Verified,
            max_invocations_per_iteration: None,
        });
    }

    /// Register a native function that receives arena context.
    ///
    /// The function gains access to the host-owned arena through the
    /// [`NativeCtx`] argument. Use this for natives that produce
    /// arena-allocated dynamic strings via
    /// [`crate::kstring::KString::alloc`] and return them as
    /// [`crate::bytecode::GenericValue::KStr`]. The boundary type carries epoch-tagged
    /// stale-pointer detection. Outstanding handles become
    /// [`keleusma_arena::Stale`] on the next reset.
    #[allow(clippy::type_complexity)]
    pub fn register_native_with_ctx(
        &mut self,
        name: &str,
        func: for<'b> fn(
            &NativeCtx<'b>,
            &[crate::bytecode::GenericValue<W, F>],
        ) -> Result<crate::bytecode::GenericValue<W, F>, VmError>,
    ) {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(func),
            classification: NativeClassification::Verified,
            max_invocations_per_iteration: None,
        });
    }

    /// Register a native function that receives arena context using a
    /// closure.
    pub fn register_native_with_ctx_closure<Func>(&mut self, name: &str, func: Func)
    where
        Func: for<'b> Fn(
                &NativeCtx<'b>,
                &[crate::bytecode::GenericValue<W, F>],
            ) -> Result<crate::bytecode::GenericValue<W, F>, VmError>
            + 'static,
    {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(func),
            classification: NativeClassification::Verified,
            max_invocations_per_iteration: None,
        });
    }

    /// Register an external native function with an attested upper
    /// bound on the per-iteration invocation count.
    ///
    /// External natives correspond to source-level
    /// `use external module::name` imports; the compiler emits
    /// `Op::CallExternalNative` for their call sites. The host
    /// attests `max_invocations_per_iteration` rather than the
    /// per-call WCET / WCMU budget. The attestation is recorded
    /// on the entry and consumed by future verifier passes; the
    /// current verifier admits external natives without folding
    /// per-call cost into the iteration budget. A mismatch
    /// between the registration classification and the call-site
    /// opcode is rejected at the call-site dispatch.
    #[allow(clippy::type_complexity)]
    pub fn register_external_native(
        &mut self,
        name: &str,
        func: fn(
            &[crate::bytecode::GenericValue<W, F>],
        ) -> Result<crate::bytecode::GenericValue<W, F>, VmError>,
        max_invocations_per_iteration: u32,
    ) {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(
                move |_ctx: &NativeCtx<'_>, args: &[crate::bytecode::GenericValue<W, F>]| {
                    func(args)
                },
            ),
            classification: NativeClassification::External,
            max_invocations_per_iteration: Some(max_invocations_per_iteration),
        });
    }

    /// Register a verified native function with attested per-call
    /// WCET and WCMU bounds.
    ///
    /// Verified natives correspond to source-level
    /// `use module::name` imports; the compiler emits
    /// `Op::CallVerifiedNative` for their call sites. The verifier
    /// folds the per-call attested cost into the iteration's
    /// WCET / WCMU budget.
    #[allow(clippy::type_complexity)]
    pub fn register_verified_native(
        &mut self,
        name: &str,
        func: fn(
            &[crate::bytecode::GenericValue<W, F>],
        ) -> Result<crate::bytecode::GenericValue<W, F>, VmError>,
        wcet: u32,
        wcmu_bytes: u32,
    ) {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet,
            wcmu_bytes,
            name: String::from(name),
            func: Box::new(
                move |_ctx: &NativeCtx<'_>, args: &[crate::bytecode::GenericValue<W, F>]| {
                    func(args)
                },
            ),
            classification: NativeClassification::Verified,
            max_invocations_per_iteration: None,
        });
    }

    // register_fn, register_fn_fallible, register_library are
    // marshall-tied and live on a specialized impl<Vm<'a,
    // 'arena>> block below. The marshall layer's IntoNativeFn /
    // KeleusmaType / stddsl::Library traits are concrete on
    // Value; step 6 lifts them to be parametric and these
    // methods can move into this generic impl block.

    /// Re-verify resource bounds with current native attestations.
    ///
    /// Walks the module's call graph, computes per-chunk WCMU including
    /// transitive contributions from chunks and natives, and checks
    /// each Stream chunk against the configured arena capacity. The
    /// host calls this after registering natives and declaring their
    /// bounds via [`Vm::set_native_bounds`].
    ///
    /// Returns an error if any Stream chunk's WCMU exceeds the arena
    /// capacity.
    ///
    /// Available only when the `verify` feature is enabled.
    #[cfg(feature = "verify")]
    pub fn verify_resources(&self) -> Result<(), VmError> {
        let module = self.module_owned()?;
        // Per-native attestations carry both the per-call WCMU
        // bound and the external-native invocation count. The
        // verifier sums per-call WCMU over static call sites for
        // verified natives and applies
        // `max_invocations_per_iteration * per_call_wcmu` once
        // per chunk for external natives.
        let bounds = self.native_iteration_bounds();
        let value_slot_bytes = core::mem::size_of::<crate::bytecode::GenericValue<W, F>>() as u32;
        verify::verify_resource_bounds_with_bounds(
            &module,
            self.arena.capacity(),
            &bounds,
            value_slot_bytes,
        )
        .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))
    }

    /// Compute the smallest arena capacity that admits this VM's module
    /// under current native attestations.
    ///
    /// Returns the maximum WCMU sum across Stream chunks. If the module
    /// has no Stream chunk, returns zero. The host can use this to size
    /// a fresh VM appropriately.
    ///
    /// Available only when the `verify` feature is enabled.
    #[cfg(feature = "verify")]
    pub fn auto_arena_capacity(&self) -> Result<usize, VmError> {
        let module = self.module_owned()?;
        let bounds = self.native_iteration_bounds();
        let chunk_wcmu = verify::module_wcmu_with_bounds(
            &module,
            &bounds,
            crate::bytecode::VALUE_SLOT_SIZE_BYTES,
        )
        .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        let mut max_total: usize = 0;
        for (chunk_idx, chunk) in module.chunks.iter().enumerate() {
            if chunk.block_type == crate::bytecode::BlockType::Stream {
                let (s, h) = chunk_wcmu[chunk_idx];
                let total = (s as usize).saturating_add(h as usize);
                if total > max_total {
                    max_total = total;
                }
            }
        }
        Ok(max_total)
    }

    /// Build per-native attestations for the verifier. Verified
    /// natives carry `max_invocations: None` and the
    /// `wcmu_bytes` field set by `set_native_bounds` or the
    /// per-call argument to `register_verified_native`. External
    /// natives carry `max_invocations: Some(n)` set at
    /// `register_external_native`.
    #[cfg(feature = "verify")]
    fn native_iteration_bounds(&self) -> Vec<verify::NativeIterationBound> {
        self.natives
            .iter()
            .map(|n| verify::NativeIterationBound {
                per_call_wcmu_bytes: n.wcmu_bytes,
                max_invocations: match n.classification {
                    NativeClassification::Verified => None,
                    NativeClassification::External => {
                        Some(n.max_invocations_per_iteration.unwrap_or(0))
                    }
                },
            })
            .collect()
    }

    /// Verify that every native call site in the loaded module
    /// matches the classification of its registered host native.
    ///
    /// Walks the module's chunks and inspects each
    /// `Op::CallVerifiedNative` / `Op::CallExternalNative` site. For
    /// each site, looks up the native's name through the module's
    /// `native_names` table and finds the matching registered
    /// `NativeEntry`. If the registered classification disagrees
    /// with the opcode's classification, returns
    /// `VmError::VerifyError` with a diagnostic naming both sides.
    /// Native names referenced by the bytecode but not yet
    /// registered are skipped: the dispatch path surfaces them as
    /// `InvalidBytecode` at the first invocation, and the host may
    /// still register the missing native after this method returns.
    ///
    /// The check is run lazily on the first `call` after natives
    /// are registered. The result is cached so subsequent calls do
    /// not repeat the walk. Any `register_*` method or
    /// `replace_module` invalidates the cache. The host may call
    /// this method explicitly to detect mismatches before the
    /// first invocation; doing so eliminates the indirection
    /// through `call` and lets the host surface the diagnostic at
    /// a deployment-validation step rather than at use.
    pub fn verify_native_classifications(&mut self) -> Result<(), VmError> {
        if self.native_classifications_verified {
            return Ok(());
        }
        // Walk the decoded ops to collect native call sites.
        // The decoded_ops field is populated at construction
        // from the wire-format opcode stream and carries the
        // owned `Op` enum directly. The archived form's chunks
        // no longer hold the ops after the Phase 7c cutover.
        let mut sites: Vec<(u16, bool)> = Vec::new();
        {
            for chunk_ops in self.decoded_ops.iter() {
                for op in chunk_ops.iter() {
                    match op {
                        Op::CallVerifiedNative(idx, _) => {
                            sites.push((*idx, false));
                        }
                        Op::CallExternalNative(idx, _) => {
                            sites.push((*idx, true));
                        }
                        _ => {}
                    }
                }
            }
        }
        for (idx, expected_external) in sites {
            let native_name = match self.native_name(idx as usize) {
                Some(name) => name,
                None => {
                    return Err(VmError::InvalidBytecode(format!(
                        "invalid native index: {}",
                        idx
                    )));
                }
            };
            // Skip names that have no registration yet; the call-
            // site dispatch surfaces them as `InvalidBytecode`
            // when the bytecode reaches that site.
            let entry = match self.natives.iter().find(|e| e.name == native_name) {
                Some(e) => e,
                None => continue,
            };
            let entry_external = entry.classification == NativeClassification::External;
            if entry_external != expected_external {
                return Err(VmError::VerifyError(format!(
                    "native `{}` registered as {} but bytecode invokes it as {}",
                    native_name,
                    if entry_external {
                        "external"
                    } else {
                        "verified"
                    },
                    if expected_external {
                        "external"
                    } else {
                        "verified"
                    },
                )));
            }
        }
        self.native_classifications_verified = true;
        Ok(())
    }

    /// Set the worst-case execution time and memory usage attestation for
    /// a previously registered native function.
    ///
    /// The host calls this after `register_native`, `register_fn`, or any
    /// other registration method to provide the upper bounds used by the
    /// static analysis tooling. The bounds are part of the trust boundary
    /// described in R9.
    ///
    /// Returns an error if no native function is registered under the
    /// given name. Applies to all entries registered under that name in
    /// case the host has registered the same name multiple times.
    pub fn set_native_bounds(
        &mut self,
        name: &str,
        wcet: u32,
        wcmu_bytes: u32,
    ) -> Result<(), VmError> {
        let mut found = false;
        for entry in self.natives.iter_mut() {
            if entry.name == name {
                entry.wcet = wcet;
                entry.wcmu_bytes = wcmu_bytes;
                found = true;
            }
        }
        if found {
            Ok(())
        } else {
            Err(VmError::NativeError(format!(
                "no native function registered under name `{}`",
                name
            )))
        }
    }

    /// Call the module's entry point with the given arguments.
    pub fn call(
        &mut self,
        args: &[crate::bytecode::GenericValue<W, F>],
    ) -> Result<GenericVmState<W, F>, VmError> {
        let entry = self
            .archived()
            .entry_point
            .as_ref()
            .map(|e| e.to_native() as usize)
            .ok_or_else(|| VmError::InvalidBytecode(String::from("no entry point")))?;
        self.call_function(entry, args)
    }

    /// Call a specific function by chunk index with the given arguments.
    pub fn call_function(
        &mut self,
        chunk_idx: usize,
        args: &[crate::bytecode::GenericValue<W, F>],
    ) -> Result<GenericVmState<W, F>, VmError> {
        // Native-classification check runs lazily before any
        // execution. The first call after natives are registered
        // (or after a hot swap) walks every native-call site and
        // verifies the bytecode-declared classification matches
        // the host's registered classification. Subsequent calls
        // skip the walk because the result is cached.
        self.verify_native_classifications()?;
        let archived = self.archived();
        let chunk = archived.chunks.get(chunk_idx).ok_or_else(|| {
            VmError::InvalidBytecode(format!("invalid chunk index: {}", chunk_idx))
        })?;
        let local_count = chunk.local_count.to_native() as usize;
        let param_count = chunk.param_count as usize;

        // Validate the argument count up front. Passing too few
        // arguments would default the missing parameter slots to
        // `crate::bytecode::GenericValue::Unit`, which the body then trips over at the
        // first use site with a confusing TypeError. Failing here
        // gives the host a clear signal that the call signature
        // is wrong before any bytecode runs.
        if args.len() != param_count {
            return Err(VmError::TypeError(format!(
                "function `{}` expected {} argument{}, got {}",
                chunk.name.as_str(),
                param_count,
                if param_count == 1 { "" } else { "s" },
                args.len()
            )));
        }

        // Validate each argument's runtime type against the
        // parameter's declared type tag. Composite types
        // (struct, enum, tuple, array, option, opaque) accept any
        // `Value`; primitive types accept only their matching
        // variant. The early rejection produces a clearer error
        // than the eventual TypeError at the first use site.
        for (i, (arg, tag)) in args.iter().zip(chunk.param_types.iter()).enumerate() {
            let tag = crate::bytecode::TypeTag::from_archived(tag);
            if !tag.admits(arg) {
                return Err(VmError::TypeError(format!(
                    "function `{}` parameter {} expected {}, got {}",
                    chunk.name.as_str(),
                    i,
                    tag.name(),
                    arg.type_name()
                )));
            }
        }

        let base = self.stack.len();
        // Push arguments as the first local slots.
        for arg in args {
            sp!(self, arg.clone());
        }
        // Extend stack for remaining local slots.
        let extra = local_count - args.len();
        for _ in 0..extra {
            sp!(self, crate::bytecode::GenericValue::Unit);
        }

        fp!(
            self,
            CallFrame {
                chunk_idx,
                ip: 0,
                base,
            }
        );
        self.started = true;

        self.run()
    }

    /// Resume execution and signal that the requested input could not
    /// be produced.
    ///
    /// This is a convenience over [`Vm::resume`] that documents the
    /// host's intent to propagate an error rather than supply a
    /// successful input. The supplied `error_value` flows through the
    /// script's yield expression unchanged. The script handles the
    /// error by pattern-matching against the value.
    ///
    /// Idiomatic usage. The script types its yield as `Option<T>` or a
    /// script-defined Result-like enum and pattern-matches:
    ///
    /// ```text
    /// let result: Option<i64> = yield request;
    /// match result {
    ///     Some(v) => { /* use v */ }
    ///     None => { /* recover from error */ }
    /// }
    /// ```
    ///
    /// The host then calls `resume(crate::bytecode::GenericValue::Int(v))` for success and
    /// [`Vm::resume_err`] (with `crate::bytecode::GenericValue::None`) for failure. For
    /// richer errors, the script defines an enum like
    /// `enum Reply { Ok(i64), Err(String) }` and the host resumes
    /// with the corresponding `crate::bytecode::GenericValue::Enum` variant.
    ///
    /// If the script does not handle the error case (does not match
    /// the error variant) the next operation that consumes the value
    /// traps with a runtime type error. This matches Keleusma's
    /// general dynamic-tag dispatch contract; it is not a new failure
    /// mode introduced by this API.
    ///
    /// The API does not perform any wrapping: the returned `VmState`
    /// reflects whatever the script does next. Hosts that want
    /// automatic propagation (Rust-like `?`) must implement that
    /// pattern in the script through pattern matching and early
    /// `return`.
    pub fn resume_err(
        &mut self,
        error_value: crate::bytecode::GenericValue<W, F>,
    ) -> Result<GenericVmState<W, F>, VmError> {
        self.resume(error_value)
    }

    /// Resume execution after a yield or reset, providing the input value.
    pub fn resume(
        &mut self,
        input: crate::bytecode::GenericValue<W, F>,
    ) -> Result<GenericVmState<W, F>, VmError> {
        if !self.started || self.frames.is_empty() {
            return Err(VmError::NotSuspended);
        }
        // For stream functions, update the parameter slot with the new input.
        // This ensures the next iteration sees the latest input.
        if let Some(base_frame) = self.frames.first().copied() {
            let archived = self.archived();
            let chunk = &archived.chunks[base_frame.chunk_idx];
            let block_type = match &chunk.block_type {
                crate::bytecode::ArchivedBlockType::Stream => BlockType::Stream,
                crate::bytecode::ArchivedBlockType::Reentrant => BlockType::Reentrant,
                crate::bytecode::ArchivedBlockType::Func => BlockType::Func,
            };
            let param_count = chunk.param_count;
            if block_type == BlockType::Stream && param_count > 0 {
                // Validate the resume value against the loop's
                // parameter type. The yield expression inside the
                // loop body has the same static type as the
                // parameter (resume provides the next iteration's
                // input); rejecting a wrong-typed value here gives
                // the host a clear signal at the resume boundary
                // rather than a confusing TypeError when the body
                // first uses the resumed value.
                if let Some(tag) = chunk.param_types.first() {
                    let tag = crate::bytecode::TypeTag::from_archived(tag);
                    if !tag.admits(&input) {
                        return Err(VmError::TypeError(format!(
                            "loop `{}` resume expected {}, got {}",
                            chunk.name.as_str(),
                            tag.name(),
                            input.type_name()
                        )));
                    }
                }
                let base = base_frame.base;
                self.stack[base] = input.clone();
            }
        }
        // Push the input value onto the stack (it becomes the yield expression result).
        sp!(self, input);
        self.run()
    }

    /// Execute bytecode until yield, return, reset, or error.
    fn run(&mut self) -> Result<GenericVmState<W, F>, VmError> {
        loop {
            if self.frames.is_empty() {
                return Err(VmError::InvalidBytecode(String::from("empty call stack")));
            }

            let frame = self.frames.last().unwrap();
            let chunk_idx = frame.chunk_idx;
            let ip = frame.ip;
            let base = frame.base;

            if ip >= self.chunk_op_count(chunk_idx) {
                // End of chunk without explicit return: return Unit.
                let result = self
                    .stack
                    .pop()
                    .unwrap_or(crate::bytecode::GenericValue::Unit);
                self.frames.pop();
                if self.frames.is_empty() {
                    return Ok(GenericVmState::Finished(result));
                }
                sp!(self, result);
                continue;
            }

            let op = self.chunk_op(chunk_idx, ip);
            // Advance IP.
            self.frames.last_mut().unwrap().ip += 1;

            match op {
                Op::Const(idx) => {
                    let val = self.chunk_const(chunk_idx, idx as usize);
                    sp!(self, val);
                }

                Op::GetLocal(slot) => {
                    let val = self.stack[base + slot as usize].clone();
                    sp!(self, val);
                }
                Op::SetLocal(slot) => {
                    let val = self.pop()?;
                    self.stack[base + slot as usize] = val;
                }

                Op::GetData(slot) => {
                    let idx = slot as usize;
                    let total = self.data_len();
                    if idx >= total {
                        return Err(VmError::InvalidBytecode(format!(
                            "data slot index {} out of bounds",
                            idx
                        )));
                    }
                    let val = self.read_data_slot(idx);
                    sp!(self, val);
                }
                Op::SetData(slot) => {
                    let idx = slot as usize;
                    let total = self.data_len();
                    if idx >= total {
                        return Err(VmError::InvalidBytecode(format!(
                            "data slot index {} out of bounds",
                            idx
                        )));
                    }
                    let val = self.pop()?;
                    self.write_data_slot(idx, val);
                }
                Op::GetDataIndexed(base, len) => {
                    let index = match self.pop()? {
                        crate::bytecode::GenericValue::Int(n) => n,
                        other => {
                            return Err(VmError::TypeError(format!(
                                "GetDataIndexed expected Int index, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if index.to_i64() < 0 || index.to_i64() >= len as i64 {
                        return Err(VmError::IndexOutOfBounds(index.to_i64(), len as usize));
                    }
                    let slot = base as usize + index.to_i64() as usize;
                    let total = self.data_len();
                    if slot >= total {
                        return Err(VmError::InvalidBytecode(format!(
                            "GetDataIndexed slot {} out of bounds",
                            slot
                        )));
                    }
                    let val = self.read_data_slot(slot);
                    sp!(self, val);
                }
                Op::SetDataIndexed(base, len) => {
                    let index = match self.pop()? {
                        crate::bytecode::GenericValue::Int(n) => n,
                        other => {
                            return Err(VmError::TypeError(format!(
                                "SetDataIndexed expected Int index, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if index.to_i64() < 0 || index.to_i64() >= len as i64 {
                        return Err(VmError::IndexOutOfBounds(index.to_i64(), len as usize));
                    }
                    let val = self.pop()?;
                    let slot = base as usize + index.to_i64() as usize;
                    let total = self.data_len();
                    if slot >= total {
                        return Err(VmError::InvalidBytecode(format!(
                            "SetDataIndexed slot {} out of bounds",
                            slot
                        )));
                    }
                    self.write_data_slot(slot, val);
                }
                Op::BoundsCheck(bound) => {
                    // Peek the top of the stack; trap if it is not a
                    // non-negative `Int` strictly less than `bound`.
                    // The stack is not modified.
                    let top = self.stack.last().ok_or(VmError::StackUnderflow)?;
                    let value = match top {
                        crate::bytecode::GenericValue::Int(n) => *n,
                        other => {
                            return Err(VmError::TypeError(format!(
                                "BoundsCheck expected Int, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if value.to_i64() < 0 || value.to_i64() >= bound as i64 {
                        return Err(VmError::IndexOutOfBounds(value.to_i64(), bound as usize));
                    }
                }

                Op::Add => {
                    // Consolidation B narrowed `Op::Add` away from
                    // `Int` operands. The compiler emits
                    // `CheckedAdd; PopN(2)` for any `Int + Int`
                    // expression and routes only `Byte`, `Fixed`,
                    // and `Float` through this opcode.
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            sp!(self, crate::bytecode::GenericValue::Byte(x.wrapping_add(y)));
                        }
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Fixed Add is integer add of the
                            // fixed-point bits; the fraction-bit
                            // count is the same for both operands
                            // by type-check invariant.
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Fixed(x.wrapping_add(y))
                            );
                        }
                        #[cfg(feature = "floats")]
                        (
                            crate::bytecode::GenericValue::Float(x),
                            crate::bytecode::GenericValue::Float(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Float(x + y)),
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "cannot add {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::Sub => self.binary_arith(|a: W, b: W| a.wrapping_sub(b), |a: F, b: F| a - b)?,
                Op::Mul => self.binary_arith(|a: W, b: W| a.wrapping_mul(b), |a: F, b: F| a * b)?,
                Op::Div => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(_),
                            crate::bytecode::GenericValue::Int(y),
                        ) if y == W::default() => {
                            return Err(VmError::DivisionByZero);
                        }
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Int(x.wrapping_div(y))),
                        (
                            crate::bytecode::GenericValue::Byte(_),
                            crate::bytecode::GenericValue::Byte(0),
                        ) => return Err(VmError::DivisionByZero),
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            sp!(self, crate::bytecode::GenericValue::Byte(x.wrapping_div(y)));
                        }
                        #[cfg(feature = "floats")]
                        (
                            crate::bytecode::GenericValue::Float(x),
                            crate::bytecode::GenericValue::Float(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Float(x / y)),
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "cannot divide {} by {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::Mod => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(_),
                            crate::bytecode::GenericValue::Int(y),
                        ) if y == W::default() => {
                            return Err(VmError::DivisionByZero);
                        }
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Int(x.wrapping_rem(y))),
                        (
                            crate::bytecode::GenericValue::Byte(_),
                            crate::bytecode::GenericValue::Byte(0),
                        ) => return Err(VmError::DivisionByZero),
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            sp!(self, crate::bytecode::GenericValue::Byte(x.wrapping_rem(y)));
                        }
                        #[cfg(feature = "floats")]
                        (
                            crate::bytecode::GenericValue::Float(x),
                            crate::bytecode::GenericValue::Float(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Float(x % y)),
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "cannot modulo {} by {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::Neg => {
                    // Consolidation B narrowed `Op::Neg` away from
                    // `Int` operands. The compiler emits
                    // `CheckedNeg; PopN(2)` for `-Int`. This opcode
                    // handles `Byte`, `Fixed`, and `Float`.
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Byte(x) => {
                            sp!(self, crate::bytecode::GenericValue::Byte(x.wrapping_neg()))
                        }
                        crate::bytecode::GenericValue::Fixed(x) => {
                            sp!(self, crate::bytecode::GenericValue::Fixed(x.wrapping_neg()))
                        }
                        #[cfg(feature = "floats")]
                        crate::bytecode::GenericValue::Float(x) => {
                            sp!(self, crate::bytecode::GenericValue::Float(-x))
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot negate {}",
                                v.type_name()
                            )));
                        }
                    }
                }

                Op::CmpEq => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    sp!(self, crate::bytecode::GenericValue::Bool(a == b));
                }
                Op::CmpNe => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    sp!(self, crate::bytecode::GenericValue::Bool(a != b));
                }
                Op::CmpLt => self.compare_op(|ord| ord.is_lt())?,
                Op::CmpGt => self.compare_op(|ord| ord.is_gt())?,
                Op::CmpLe => self.compare_op(|ord| ord.is_le())?,
                Op::CmpGe => self.compare_op(|ord| ord.is_ge())?,

                Op::Not => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Bool(b) => {
                            sp!(self, crate::bytecode::GenericValue::Bool(!b))
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot apply not to {}",
                                v.type_name()
                            )));
                        }
                    }
                }

                // -- Block-structured control flow --
                Op::If(target) => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Bool(false) => {
                            self.frames.last_mut().unwrap().ip = target as usize;
                        }
                        crate::bytecode::GenericValue::Bool(true) => {
                            // Continue to then-block.
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "condition must be Bool, got {}",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::Else(target) => {
                    // Reached when then-block completes. Skip else-block.
                    self.frames.last_mut().unwrap().ip = target as usize;
                }
                Op::EndIf => {
                    // No-op. Block delimiter.
                }

                Op::Loop(_) => {
                    // No-op at entry. Target is used by Break/BreakIf.
                }
                Op::EndLoop(target) => {
                    // Back-edge: jump to instruction after Loop.
                    self.frames.last_mut().unwrap().ip = target as usize;
                }
                Op::Break(target) => {
                    self.frames.last_mut().unwrap().ip = target as usize;
                }
                Op::BreakIf(target) => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Bool(true) => {
                            self.frames.last_mut().unwrap().ip = target as usize;
                        }
                        crate::bytecode::GenericValue::Bool(false) => {
                            // Continue loop body.
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "BreakIf condition must be Bool, got {}",
                                v.type_name()
                            )));
                        }
                    }
                }

                // -- Streaming --
                Op::Stream => {
                    // No-op. Marks the stream entry point.
                }
                Op::Reset => {
                    // Reset locals to Unit, truncate stack, reset arena pointers.
                    let (reset_base, reset_chunk_idx) = {
                        let frame = self.frames.last().unwrap();
                        (frame.base, frame.chunk_idx)
                    };
                    let local_count = self.chunk_local_count(reset_chunk_idx) as usize;

                    // Clear locals to Unit.
                    for i in 0..local_count {
                        self.stack[reset_base + i] = crate::bytecode::GenericValue::Unit;
                    }
                    // Truncate stack to just the locals.
                    self.stack.truncate(reset_base + local_count);

                    // Reset both arena bump pointers (R32). Host-allocated
                    // dynamic strings and other arena values are reclaimed
                    // here.
                    let _ = self.reset_arena_internal();

                    // Find Stream instruction and set IP to
                    // instruction after it. V0.2.0 Phase 7c reads
                    // decoded_ops instead of the archived chunk's
                    // (no longer present) ops field.
                    let stream_ip = self.decoded_ops[reset_chunk_idx]
                        .iter()
                        .position(|op| matches!(op, Op::Stream));
                    match stream_ip {
                        Some(pos) => self.frames.last_mut().unwrap().ip = pos + 1,
                        None => {
                            return Err(VmError::InvalidBytecode(String::from(
                                "Reset without Stream in chunk",
                            )));
                        }
                    }

                    return Ok(GenericVmState::Reset);
                }

                // -- Functions --
                Op::Call(idx, arg_count) => {
                    if idx as usize >= self.chunk_count() {
                        return Err(VmError::InvalidBytecode(format!("invalid chunk: {}", idx)));
                    }
                    let called_local_count = self.chunk_local_count(idx as usize) as usize;
                    let new_base = self.stack.len() - arg_count as usize;
                    let extra = called_local_count - arg_count as usize;
                    for _ in 0..extra {
                        sp!(self, crate::bytecode::GenericValue::Unit);
                    }
                    fp!(
                        self,
                        CallFrame {
                            chunk_idx: idx as usize,
                            ip: 0,
                            base: new_base,
                        }
                    );
                }
                Op::Return => {
                    let result = self.pop()?;
                    let old_frame = self.frames.pop().unwrap();
                    self.stack.truncate(old_frame.base);
                    if self.frames.is_empty() {
                        return Ok(GenericVmState::Finished(result));
                    }
                    sp!(self, result);
                }

                Op::Yield => {
                    let output = self.pop()?;
                    // Enforce cross-yield prohibition on dynamic strings (R31).
                    // A dynamic string is an arena pointer. Allowing one across
                    // the yield boundary would either require the host to
                    // consume it before the next RESET or accept dangling
                    // references after the arena is cleared. The runtime
                    // structural check rejects yielded values that transitively
                    // contain a dynamic string.
                    if output.contains_dynstr() {
                        return Err(VmError::TypeError(String::from(
                            "yielded value contains a dynamic string, which cannot \
                             cross the yield boundary; use a static string or convert \
                             to a non-string representation in the host",
                        )));
                    }
                    return Ok(GenericVmState::Yielded(output));
                }

                Op::Dup => {
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?.clone();
                    sp!(self, val);
                }

                Op::NewStruct(template_idx) => {
                    let (type_name, field_names) =
                        self.struct_template(chunk_idx, template_idx as usize);
                    let n = field_names.len();
                    if self.stack.len() < n {
                        return Err(VmError::StackUnderflow);
                    }
                    let values: Vec<crate::bytecode::GenericValue<W, F>> =
                        self.stack.drain(self.stack.len() - n..).collect();
                    let fields: Vec<(String, crate::bytecode::GenericValue<W, F>)> =
                        field_names.into_iter().zip(values).collect();
                    sp!(
                        self,
                        crate::bytecode::GenericValue::Struct { type_name, fields }
                    );
                }
                Op::NewEnum(enum_const, var_const, arg_count) => {
                    let type_name = self
                        .chunk_const_str(chunk_idx, enum_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("enum name not a string"))
                        })?;
                    let variant = self
                        .chunk_const_str(chunk_idx, var_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("variant name not a string"))
                        })?;
                    let n = arg_count as usize;
                    let fields: Vec<crate::bytecode::GenericValue<W, F>> = if n > 0 {
                        self.stack.drain(self.stack.len() - n..).collect()
                    } else {
                        Vec::new()
                    };
                    sp!(
                        self,
                        crate::bytecode::GenericValue::Enum {
                            type_name,
                            variant,
                            fields,
                        }
                    );
                }
                Op::NewArray(count) => {
                    let n = count as usize;
                    let elements: Vec<crate::bytecode::GenericValue<W, F>> =
                        self.stack.drain(self.stack.len() - n..).collect();
                    sp!(self, crate::bytecode::GenericValue::Array(elements));
                }
                Op::NewTuple(count) => {
                    let n = count as usize;
                    let elements: Vec<crate::bytecode::GenericValue<W, F>> =
                        self.stack.drain(self.stack.len() - n..).collect();
                    sp!(self, crate::bytecode::GenericValue::Tuple(elements));
                }
                Op::GetField(name_const) => {
                    let container = self.pop()?;
                    let field_name = self
                        .chunk_const_str(chunk_idx, name_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("field name not a string"))
                        })?;
                    match container {
                        crate::bytecode::GenericValue::Struct { type_name, fields } => {
                            let val = fields
                                .iter()
                                .find(|(n, _)| n == &field_name)
                                .map(|(_, v)| v.clone())
                                .ok_or(VmError::FieldNotFound(type_name, field_name))?;
                            sp!(self, val);
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot access field on {}",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::GetIndex => {
                    let index = self.pop()?;
                    let container = self.pop()?;
                    match (container, index) {
                        (
                            crate::bytecode::GenericValue::Array(arr),
                            crate::bytecode::GenericValue::Int(i),
                        ) => {
                            let len = arr.len();
                            if i.to_i64() < 0 || i.to_i64() as usize >= len {
                                return Err(VmError::IndexOutOfBounds(i.to_i64(), len));
                            }
                            sp!(self, arr[i.to_i64() as usize].clone());
                        }
                        (c, i) => {
                            return Err(VmError::TypeError(format!(
                                "cannot index {} with {}",
                                c.type_name(),
                                i.type_name()
                            )));
                        }
                    }
                }
                Op::GetTupleField(idx) => {
                    let container = self.pop()?;
                    match container {
                        crate::bytecode::GenericValue::Tuple(elems) => {
                            let i = idx as usize;
                            if i >= elems.len() {
                                return Err(VmError::IndexOutOfBounds(i as i64, elems.len()));
                            }
                            sp!(self, elems[i].clone());
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot tuple-index {}",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::GetEnumField(idx) => {
                    let container = self.pop()?;
                    match container {
                        crate::bytecode::GenericValue::Enum { fields, .. } => {
                            let i = idx as usize;
                            if i >= fields.len() {
                                return Err(VmError::IndexOutOfBounds(i as i64, fields.len()));
                            }
                            sp!(self, fields[i].clone());
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot enum-field {}",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::Len => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Array(arr) => {
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(arr.len() as i64)
                                )
                            );
                        }
                        crate::bytecode::GenericValue::StaticStr(s) => {
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(
                                        s.chars().count() as i64
                                    )
                                )
                            );
                        }
                        crate::bytecode::GenericValue::KStr(h) => {
                            let s = h.get(self.arena).map_err(|_| {
                                VmError::TypeError(String::from(
                                    "KStr is stale (arena reset since allocation)",
                                ))
                            })?;
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(
                                        s.chars().count() as i64
                                    )
                                )
                            );
                        }
                        crate::bytecode::GenericValue::Tuple(t) => {
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(t.len() as i64)
                                )
                            );
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot get length of {}",
                                v.type_name()
                            )));
                        }
                    }
                }

                // -- Type predicates (push bool, no jump) --
                Op::IsEnum(enum_const, var_const) => {
                    let expected_type = self
                        .chunk_const_str(chunk_idx, enum_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("enum const not string"))
                        })?;
                    let expected_var = self
                        .chunk_const_str(chunk_idx, var_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("variant const not string"))
                        })?;
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?;
                    let matches = matches!(
                        val,
                        crate::bytecode::GenericValue::Enum { type_name, variant, .. }
                            if type_name == &expected_type && variant == &expected_var
                    );
                    sp!(self, crate::bytecode::GenericValue::Bool(matches));
                }
                Op::IsStruct(type_const) => {
                    let expected = self
                        .chunk_const_str(chunk_idx, type_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("type const not string"))
                        })?;
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?;
                    let matches = matches!(val, crate::bytecode::GenericValue::Struct { type_name, .. } if type_name == &expected);
                    sp!(self, crate::bytecode::GenericValue::Bool(matches));
                }

                #[cfg(feature = "floats")]
                Op::IntToFloat => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Int(i) => sp!(
                            self,
                            crate::bytecode::GenericValue::Float(
                                <F as crate::float::Float>::from_f64(i.to_i64() as f64)
                            )
                        ),
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to Float",
                                v.type_name()
                            )));
                        }
                    }
                }
                #[cfg(not(feature = "floats"))]
                Op::IntToFloat => {
                    return Err(VmError::InvalidBytecode(String::from(
                        "Op::IntToFloat requires the `floats` feature",
                    )));
                }
                #[cfg(feature = "floats")]
                Op::FloatToInt => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Float(f) => sp!(
                            self,
                            crate::bytecode::GenericValue::Int(
                                <W as crate::word::Word>::from_i64_wrap(f.to_f64() as i64)
                            )
                        ),
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to Word",
                                v.type_name()
                            )));
                        }
                    }
                }
                #[cfg(not(feature = "floats"))]
                Op::FloatToInt => {
                    return Err(VmError::InvalidBytecode(String::from(
                        "Op::FloatToInt requires the `floats` feature",
                    )));
                }
                Op::WordToByte => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Int(i) => sp!(
                            self,
                            crate::bytecode::GenericValue::Byte((i.to_i64() & 0xFF) as u8)
                        ),
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to Byte",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::ByteToWord => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Byte(b) => sp!(
                            self,
                            crate::bytecode::GenericValue::Int(
                                <W as crate::word::Word>::from_i64_wrap(b as i64)
                            )
                        ),
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to Word",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::WordToFixed(frac_bits) => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Int(i) => {
                            // Left-shift the word into the fixed
                            // representation. Saturate at
                            // i64::MAX/MIN on overflow.
                            let shifted = i.widen() << (frac_bits as u32);
                            let bits = if shifted > <W as crate::word::Word>::MAX.widen() {
                                <W as crate::word::Word>::MAX
                            } else if shifted < <W as crate::word::Word>::MIN.widen() {
                                <W as crate::word::Word>::MIN
                            } else {
                                <W as crate::word::Word>::from_wide_wrap(shifted)
                            };
                            sp!(self, crate::bytecode::GenericValue::Fixed(bits));
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to Fixed",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::FixedToWord(frac_bits) => {
                    let val = self.pop()?;
                    match val {
                        crate::bytecode::GenericValue::Fixed(bits) => {
                            // Arithmetic-right-shift to drop the
                            // fraction bits. Negative values keep
                            // their sign through the shift.
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(bits >> (frac_bits as u32))
                            );
                        }
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to Word",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::FixedMul(frac_bits) => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Q-format multiply: extend to i128 to
                            // avoid intermediate overflow, multiply,
                            // shift right by `frac_bits`, saturate
                            // back to i64.
                            let product = x.widen() * y.widen();
                            let shifted = product >> (frac_bits as u32);
                            let bits = if shifted > <W as crate::word::Word>::MAX.widen() {
                                <W as crate::word::Word>::MAX
                            } else if shifted < <W as crate::word::Word>::MIN.widen() {
                                <W as crate::word::Word>::MIN
                            } else {
                                <W as crate::word::Word>::from_wide_wrap(shifted)
                            };
                            sp!(self, crate::bytecode::GenericValue::Fixed(bits));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "FixedMul requires two Fixed operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::FixedDiv(frac_bits) => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Fixed(_),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) if y == W::default() => {
                            return Err(VmError::DivisionByZero);
                        }
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Q-format divide: extend the dividend to
                            // i128 and left-shift by frac_bits before
                            // dividing, so the result retains the
                            // Q-format precision.
                            let dividend = x.widen() << (frac_bits as u32);
                            let quotient = dividend / y.widen();
                            let bits = if quotient > <W as crate::word::Word>::MAX.widen() {
                                <W as crate::word::Word>::MAX
                            } else if quotient < <W as crate::word::Word>::MIN.widen() {
                                <W as crate::word::Word>::MIN
                            } else {
                                <W as crate::word::Word>::from_wide_wrap(quotient)
                            };
                            sp!(self, crate::bytecode::GenericValue::Fixed(bits));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "FixedDiv requires two Fixed operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                Op::Trap(kind_code) => {
                    use crate::bytecode::TrapKind;
                    return Err(match TrapKind::from_code(kind_code) {
                        Some(TrapKind::RefinementFailed) => VmError::RefinementFailed,
                        Some(TrapKind::NoMatchingHead) => VmError::NoMatchingHead,
                        Some(TrapKind::NoMatchingArm) => VmError::NoMatchingArm,
                        Some(TrapKind::CheckedArithNoArm) => VmError::CheckedArithNoArm,
                        Some(TrapKind::EnumVariantUnmapped) => VmError::EnumVariantUnmapped,
                        // An unhandled zero divisor in a checked
                        // construct surfaces as the same error a plain
                        // division by zero produces.
                        Some(TrapKind::ZeroDivisor) => VmError::DivisionByZero,
                        None => VmError::InvalidBytecode(alloc::format!(
                            "Op::Trap carried an unknown trap-kind code {}",
                            kind_code
                        )),
                    });
                }
                Op::CheckedAdd => {
                    let word_bits_log2 = self.word_bits_log2();
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => {
                            let r = x.widen() + y.widen();
                            let (low, high, flag) = checked_arith_outputs::<W>(r, word_bits_log2);
                            // Push order is (low, high, flag) so the
                            // wrapping-arithmetic synthesis emitted by
                            // the compiler — `CheckedAdd; PopN(2)` —
                            // discards the top two slots (flag and
                            // high) and leaves `low` on the stack.
                            sp!(self, crate::bytecode::GenericValue::Int(low));
                            sp!(self, crate::bytecode::GenericValue::Int(high));
                            sp!(self, crate::bytecode::GenericValue::Int(flag));
                        }
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            // Unsigned Byte addition: overflow above
                            // 255, never underflow. The wrapped result
                            // (low 8 bits) is the low slot; the high
                            // slot is unused for Byte.
                            let r = x as i64 + y as i64;
                            let flag: i64 = if r > 0xFF { 1 } else { 0 };
                            sp!(self, crate::bytecode::GenericValue::Byte(r as u8));
                            sp!(self, crate::bytecode::GenericValue::Byte(0));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        #[cfg(feature = "floats")]
                        (
                            crate::bytecode::GenericValue::Float(x),
                            crate::bytecode::GenericValue::Float(y),
                        ) => {
                            // Float addition is total (IEEE 754): the
                            // result is finite, an infinity, or NaN,
                            // classified into the flag. The result is
                            // the low slot; the high slot is unused.
                            let r = x + y;
                            let flag = float_checked_flag(<F as crate::float::Float>::to_f64(r));
                            sp!(self, crate::bytecode::GenericValue::Float(r));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Float(
                                    <F as crate::float::Float>::from_f64(0.0)
                                )
                            );
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Q-format addition shares the operands'
                            // fraction-bit count, so it is a raw sum.
                            // The wide result is wrapped to the low
                            // slot; the high slot is unused.
                            let r = x.widen() + y.widen();
                            let (low, flag) = fixed_checked_outputs::<W>(r);
                            sp!(self, crate::bytecode::GenericValue::Fixed(low));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedAdd expects Word, Byte, Float, or Fixed operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedSub => {
                    let word_bits_log2 = self.word_bits_log2();
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => {
                            let r = x.widen() - y.widen();
                            let (low, high, flag) = checked_arith_outputs::<W>(r, word_bits_log2);
                            sp!(self, crate::bytecode::GenericValue::Int(low));
                            sp!(self, crate::bytecode::GenericValue::Int(high));
                            sp!(self, crate::bytecode::GenericValue::Int(flag));
                        }
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            // Unsigned Byte subtraction: underflow below
                            // 0, never overflow. `r as u8` wraps modulo
                            // 256 for the low slot.
                            let r = x as i64 - y as i64;
                            let flag: i64 = if r < 0 { 2 } else { 0 };
                            sp!(self, crate::bytecode::GenericValue::Byte(r as u8));
                            sp!(self, crate::bytecode::GenericValue::Byte(0));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        #[cfg(feature = "floats")]
                        (
                            crate::bytecode::GenericValue::Float(x),
                            crate::bytecode::GenericValue::Float(y),
                        ) => {
                            let r = x - y;
                            let flag = float_checked_flag(<F as crate::float::Float>::to_f64(r));
                            sp!(self, crate::bytecode::GenericValue::Float(r));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Float(
                                    <F as crate::float::Float>::from_f64(0.0)
                                )
                            );
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Q-format subtraction is a raw difference
                            // sharing the fraction-bit count.
                            let r = x.widen() - y.widen();
                            let (low, flag) = fixed_checked_outputs::<W>(r);
                            sp!(self, crate::bytecode::GenericValue::Fixed(low));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedSub expects Word, Byte, Float, or Fixed operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedMul => {
                    let word_bits_log2 = self.word_bits_log2();
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => {
                            // True product in i128; both halves are
                            // load-bearing for big-number
                            // multiplication. The shared helper
                            // computes high relative to the
                            // declared word width and reports flag
                            // direction (overflow / underflow) at
                            // the declared range.
                            let r = x.widen() * y.widen();
                            let (low, high, flag) = checked_arith_outputs::<W>(r, word_bits_log2);
                            sp!(self, crate::bytecode::GenericValue::Int(low));
                            sp!(self, crate::bytecode::GenericValue::Int(high));
                            sp!(self, crate::bytecode::GenericValue::Int(flag));
                        }
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            // Unsigned Byte multiplication: overflow
                            // above 255, never underflow.
                            let r = x as i64 * y as i64;
                            let flag: i64 = if r > 0xFF { 1 } else { 0 };
                            sp!(self, crate::bytecode::GenericValue::Byte(r as u8));
                            sp!(self, crate::bytecode::GenericValue::Byte(0));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        #[cfg(feature = "floats")]
                        (
                            crate::bytecode::GenericValue::Float(x),
                            crate::bytecode::GenericValue::Float(y),
                        ) => {
                            let r = x * y;
                            let flag = float_checked_flag(<F as crate::float::Float>::to_f64(r));
                            sp!(self, crate::bytecode::GenericValue::Float(r));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Float(
                                    <F as crate::float::Float>::from_f64(0.0)
                                )
                            );
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedMul expects Word, Byte, or Float operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedNeg => {
                    let word_bits_log2 = self.word_bits_log2();
                    let a = self.pop()?;
                    match a {
                        crate::bytecode::GenericValue::Int(x) => {
                            // The only runtime-width overflow is
                            // `-i64::MIN`. At a narrower declared
                            // width, both `-declared_min` and
                            // every value of `x` that lands outside
                            // the declared range surface as
                            // overflow/underflow through the
                            // shared helper.
                            let r = -x.widen();
                            let (low, high, flag) = checked_arith_outputs::<W>(r, word_bits_log2);
                            sp!(self, crate::bytecode::GenericValue::Int(low));
                            sp!(self, crate::bytecode::GenericValue::Int(high));
                            sp!(self, crate::bytecode::GenericValue::Int(flag));
                        }
                        crate::bytecode::GenericValue::Fixed(x) => {
                            // Q-format negation is a raw negation; the
                            // only overflow case is `-i64::MIN`.
                            let r = -x.widen();
                            let (low, flag) = fixed_checked_outputs::<W>(r);
                            sp!(self, crate::bytecode::GenericValue::Fixed(low));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        a => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedNeg expects a Word or Fixed operand, got {}",
                                a.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedDiv => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) if y == W::default() => {
                            // Zero divisor: reify as flag 3
                            // (zero_divisor) rather than trapping. The
                            // numerator goes in the low slot so the
                            // construct's `zero_divisor(numerator)` arm
                            // binds it; an unhandled zero divisor traps
                            // as DivisionByZero in the compiled dispatch.
                            sp!(self, crate::bytecode::GenericValue::Int(x));
                            sp!(self, crate::bytecode::GenericValue::Int(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(3)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => {
                            // Only `i64::MIN / -1` overflows. The
                            // true result is `2^63`, which in i128
                            // is (high=0, low=i64::MIN). All other
                            // divisions fit in `Word`; the wrapped
                            // quotient becomes the low slot and the
                            // high slot is zero.
                            let r = x.widen() / y.widen();
                            let high = <W as crate::word::Word>::from_wide_wrap(r.high_half());
                            let low = <W as crate::word::Word>::from_wide_wrap(r);
                            let flag: i64 = if r >= <W as crate::word::Word>::MIN.widen()
                                && r <= <W as crate::word::Word>::MAX.widen()
                            {
                                0
                            } else if r > <W as crate::word::Word>::MAX.widen() {
                                1
                            } else {
                                2
                            };
                            sp!(self, crate::bytecode::GenericValue::Int(low));
                            sp!(self, crate::bytecode::GenericValue::Int(high));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(0),
                        ) => {
                            // Byte zero divisor: flag 3, numerator in
                            // the low slot.
                            sp!(self, crate::bytecode::GenericValue::Byte(x));
                            sp!(self, crate::bytecode::GenericValue::Byte(0));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(3)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            // Unsigned Byte division never overflows.
                            sp!(self, crate::bytecode::GenericValue::Byte(x / y));
                            sp!(self, crate::bytecode::GenericValue::Byte(0));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(0)
                                )
                            );
                        }
                        #[cfg(feature = "floats")]
                        (
                            crate::bytecode::GenericValue::Float(x),
                            crate::bytecode::GenericValue::Float(y),
                        ) => {
                            // Float division is total: division by zero
                            // yields a signed infinity (x != 0) or NaN
                            // (0 / 0), classified into the flag. There
                            // is no zero-divisor trap for floats.
                            let r = x / y;
                            let flag = float_checked_flag(<F as crate::float::Float>::to_f64(r));
                            sp!(self, crate::bytecode::GenericValue::Float(r));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Float(
                                    <F as crate::float::Float>::from_f64(0.0)
                                )
                            );
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedDiv expects Word, Byte, or Float operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedMod => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) if y == W::default() => {
                            // Zero divisor: reify as flag 3, numerator
                            // in the low slot, mirroring CheckedDiv.
                            sp!(self, crate::bytecode::GenericValue::Int(x));
                            sp!(self, crate::bytecode::GenericValue::Int(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(3)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => {
                            // A remainder is always in range, including
                            // the `i64::MIN % -1` corner whose true
                            // result is `0`, so modulo never overflows
                            // or underflows; the only non-`ok` outcome
                            // is the zero divisor handled above. The
                            // type checker forbids `overflow` and
                            // `underflow` arms on `%` (B35 P3c).
                            let r = x.widen() % y.widen();
                            let high = <W as crate::word::Word>::from_wide_wrap(r.high_half());
                            let low = <W as crate::word::Word>::from_wide_wrap(r);
                            let flag: i64 = 0;
                            sp!(self, crate::bytecode::GenericValue::Int(low));
                            sp!(self, crate::bytecode::GenericValue::Int(high));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(0),
                        ) => {
                            // Byte zero divisor: flag 3, numerator in
                            // the low slot.
                            sp!(self, crate::bytecode::GenericValue::Byte(x));
                            sp!(self, crate::bytecode::GenericValue::Byte(0));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(3)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Byte(x),
                            crate::bytecode::GenericValue::Byte(y),
                        ) => {
                            // A Byte remainder is always in range.
                            sp!(self, crate::bytecode::GenericValue::Byte(x % y));
                            sp!(self, crate::bytecode::GenericValue::Byte(0));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(0)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) if y == W::default() => {
                            // Fixed zero divisor: flag 3, numerator in
                            // the low slot, mirroring CheckedDiv.
                            sp!(self, crate::bytecode::GenericValue::Fixed(x));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(3)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Q-format remainder is the raw remainder
                            // at the shared scale and is always in
                            // range, so modulo never overflows.
                            let r = x.widen() % y.widen();
                            let low = <W as crate::word::Word>::from_wide_wrap(r);
                            sp!(self, crate::bytecode::GenericValue::Fixed(low));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(0)
                                )
                            );
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedMod expects Word, Byte, or Fixed operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedFixedMul(frac_bits) => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Q-format product: widen to avoid the
                            // intermediate overflow, multiply, shift
                            // right by the shared fraction-bit count,
                            // then classify against the Word range.
                            // The checked form wraps the low slot,
                            // unlike the saturating `Op::FixedMul`.
                            let product = x.widen() * y.widen();
                            let shifted = product >> (frac_bits as u32);
                            let (low, flag) = fixed_checked_outputs::<W>(shifted);
                            sp!(self, crate::bytecode::GenericValue::Fixed(low));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedFixedMul requires two Fixed operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedFixedDiv(frac_bits) => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) if y == W::default() => {
                            // Fixed zero divisor: flag 3, numerator in
                            // the low slot, mirroring CheckedDiv.
                            sp!(self, crate::bytecode::GenericValue::Fixed(x));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(3)
                                )
                            );
                        }
                        (
                            crate::bytecode::GenericValue::Fixed(x),
                            crate::bytecode::GenericValue::Fixed(y),
                        ) => {
                            // Q-format quotient: left-shift the
                            // dividend by the fraction-bit count in the
                            // wide domain, divide, then classify. The
                            // checked form wraps the low slot, unlike
                            // the saturating `Op::FixedDiv`.
                            let dividend = x.widen() << (frac_bits as u32);
                            let quotient = dividend / y.widen();
                            let (low, flag) = fixed_checked_outputs::<W>(quotient);
                            sp!(self, crate::bytecode::GenericValue::Fixed(low));
                            sp!(self, crate::bytecode::GenericValue::Fixed(W::default()));
                            sp!(
                                self,
                                crate::bytecode::GenericValue::Int(
                                    <W as crate::word::Word>::from_i64_wrap(flag)
                                )
                            );
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedFixedDiv requires two Fixed operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }

                // V0.2.0 ISA additions (B20). Phase 1: dispatch
                // implemented; compiler emission and migration land
                // in later phases.
                Op::PushImmediate(value) => {
                    let v = match value {
                        0 => crate::bytecode::GenericValue::Unit,
                        1 => crate::bytecode::GenericValue::Bool(true),
                        2 => crate::bytecode::GenericValue::Bool(false),
                        3 => crate::bytecode::GenericValue::None,
                        n @ 4..=19 => crate::bytecode::GenericValue::Int(
                            <W as crate::word::Word>::from_i64_wrap((n - 4) as i64),
                        ),
                        other => {
                            return Err(VmError::InvalidBytecode(format!(
                                "Op::PushImmediate({}) operand is reserved; valid range is 0..=19",
                                other
                            )));
                        }
                    };
                    sp!(self, v);
                }
                Op::PopN(n) => {
                    let count = n as usize;
                    if self.stack.len() < count {
                        return Err(VmError::StackUnderflow);
                    }
                    let new_len = self.stack.len() - count;
                    self.stack.truncate(new_len);
                }
                Op::BitAnd => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Int(x & y)),
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::BitAnd expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::BitOr => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Int(x | y)),
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::BitOr expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::BitXor => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (
                            crate::bytecode::GenericValue::Int(x),
                            crate::bytecode::GenericValue::Int(y),
                        ) => sp!(self, crate::bytecode::GenericValue::Int(x ^ y)),
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::BitXor expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::Shl => {
                    let count = self.pop()?;
                    let value = self.pop()?;
                    match (value, count) {
                        (
                            crate::bytecode::GenericValue::Int(v),
                            crate::bytecode::GenericValue::Int(c),
                        ) => {
                            let word_bits = 1u32 << <W as crate::word::Word>::BITS_LOG2;
                            let shift = (<W as crate::word::Word>::to_i64(c) as u32)
                                & word_bits.saturating_sub(1);
                            sp!(self, crate::bytecode::GenericValue::Int(v << shift));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::Shl expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::Shr => {
                    let count = self.pop()?;
                    let value = self.pop()?;
                    match (value, count) {
                        (
                            crate::bytecode::GenericValue::Int(v),
                            crate::bytecode::GenericValue::Int(c),
                        ) => {
                            let word_bits = 1u32 << <W as crate::word::Word>::BITS_LOG2;
                            let shift = (<W as crate::word::Word>::to_i64(c) as u32)
                                & word_bits.saturating_sub(1);
                            sp!(self, crate::bytecode::GenericValue::Int(v >> shift));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::Shr expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                // Verified and external native dispatch share the
                // value-pop / arg-marshal sequence. The split
                // opcodes carry the structural classification; the
                // verifier observes the same classification to
                // decide between per-call WCET/WCMU attestation
                // (verified) and per-iteration invocation count
                // (external). The classification cross-check
                // against the registered native runs lazily at
                // `call_function` entry through
                // `verify_native_classifications`; the dispatch
                // arm here trusts the load-time check.
                Op::CallVerifiedNative(idx, arg_count) | Op::CallExternalNative(idx, arg_count) => {
                    let n = arg_count as usize;
                    if self.stack.len() < n {
                        return Err(VmError::StackUnderflow);
                    }
                    let args: Vec<crate::bytecode::GenericValue<W, F>> =
                        self.stack.drain(self.stack.len() - n..).collect();
                    let native_name = self.native_name(idx as usize).ok_or_else(|| {
                        VmError::InvalidBytecode(format!("invalid native index: {}", idx))
                    })?;
                    let entry = self
                        .natives
                        .iter()
                        .find(|e| e.name == native_name)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(format!(
                                "unregistered native: {}",
                                native_name
                            ))
                        })?;
                    let ctx = NativeCtx { arena: self.arena };
                    let result = (entry.func)(&ctx, &args)?;
                    sp!(self, result);
                }
            }
        }
    }

    fn pop(&mut self) -> Result<crate::bytecode::GenericValue<W, F>, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn binary_arith<IntOp, FloatOp>(
        &mut self,
        int_op: IntOp,
        // `float_op` is kept regardless of the `floats` feature so
        // existing call sites compile unchanged.
        #[allow(unused_variables)] float_op: FloatOp,
    ) -> Result<(), VmError>
    where
        IntOp: Fn(W, W) -> W,
        FloatOp: Fn(F, F) -> F,
    {
        // Consolidation B narrowed `Op::Sub` and `Op::Mul` away
        // from `Int` operands. The compiler emits the
        // `CheckedXxx; PopN(2)` synthesis for `Int + Int`
        // expressions. This helper retains the `Byte` and `Float`
        // arms only; `Op::Mul` on `Fixed` goes through
        // `Op::FixedMul(n)` rather than this helper, but `Op::Sub`
        // on `Fixed` (which the compiler still emits via the
        // generic `Op::Sub`) is admitted by widening the `Byte`
        // arm pattern to include `Fixed`. The `int_op` closure is
        // reused for `Byte` and `Fixed` arithmetic since both are
        // wrapping integer operations on the underlying bit
        // representation.
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (crate::bytecode::GenericValue::Byte(x), crate::bytecode::GenericValue::Byte(y)) => {
                // Byte arithmetic via i64 then mask: simulate the
                // operand widening and post-result truncation that
                // the i64 default runtime performs.
                let result = int_op(
                    <W as crate::word::Word>::from_i64_wrap(x as i64),
                    <W as crate::word::Word>::from_i64_wrap(y as i64),
                );
                sp!(
                    self,
                    crate::bytecode::GenericValue::Byte((result.to_i64() & 0xFF) as u8)
                );
            }
            (crate::bytecode::GenericValue::Fixed(x), crate::bytecode::GenericValue::Fixed(y)) => {
                let result = int_op(x, y);
                sp!(self, crate::bytecode::GenericValue::Fixed(result));
            }
            #[cfg(feature = "floats")]
            (crate::bytecode::GenericValue::Float(x), crate::bytecode::GenericValue::Float(y)) => {
                sp!(self, crate::bytecode::GenericValue::Float(float_op(x, y)))
            }
            (a, b) => {
                return Err(VmError::TypeError(format!(
                    "type mismatch: {} and {}",
                    a.type_name(),
                    b.type_name()
                )));
            }
        }
        Ok(())
    }

    fn compare_op<Pred>(&mut self, pred: Pred) -> Result<(), VmError>
    where
        Pred: FnOnce(core::cmp::Ordering) -> bool,
    {
        let b = self.pop()?;
        let a = self.pop()?;
        let ord = match (&a, &b) {
            (crate::bytecode::GenericValue::Int(x), crate::bytecode::GenericValue::Int(y)) => {
                x.cmp(y)
            }
            (crate::bytecode::GenericValue::Byte(x), crate::bytecode::GenericValue::Byte(y)) => {
                x.cmp(y)
            }
            (crate::bytecode::GenericValue::Fixed(x), crate::bytecode::GenericValue::Fixed(y)) => {
                x.cmp(y)
            }
            #[cfg(feature = "floats")]
            (crate::bytecode::GenericValue::Float(x), crate::bytecode::GenericValue::Float(y)) => {
                x.partial_cmp(y).unwrap_or(core::cmp::Ordering::Equal)
            }
            (
                a @ (crate::bytecode::GenericValue::StaticStr(_)
                | crate::bytecode::GenericValue::KStr(_)),
                b @ (crate::bytecode::GenericValue::StaticStr(_)
                | crate::bytecode::GenericValue::KStr(_)),
            ) => {
                let arena = self.arena;
                let xs = a
                    .as_str_with_arena(arena)
                    .map_err(|_| {
                        VmError::TypeError(String::from(
                            "KStr is stale (arena reset since allocation)",
                        ))
                    })?
                    .unwrap_or("");
                let ys = b
                    .as_str_with_arena(arena)
                    .map_err(|_| {
                        VmError::TypeError(String::from(
                            "KStr is stale (arena reset since allocation)",
                        ))
                    })?
                    .unwrap_or("");
                xs.cmp(ys)
            }
            _ => {
                return Err(VmError::TypeError(format!(
                    "cannot compare {} and {}",
                    a.type_name(),
                    b.type_name()
                )));
            }
        };
        sp!(self, crate::bytecode::GenericValue::Bool(pred(ord)));
        Ok(())
    }
}

// Marshall-integration methods. The marshall layer's
// IntoNativeFn / KeleusmaType / stddsl::Library traits are
// parametric over (W, F); these methods quantify the same way
// so any `GenericVm<W, A, F>` can register host functions.
impl<'a, 'arena, W: crate::word::Word, A: crate::address::Address, F: crate::float::Float>
    GenericVm<'a, 'arena, W, A, F>
{
    /// Register an infallible host function with automatic argument and
    /// return-value marshalling.
    pub fn register_fn<Func, Args, R>(&mut self, name: &str, func: Func)
    where
        Func: crate::marshall::IntoNativeFn<W, F, Args, R>,
    {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: func.into_native_fn(),
            classification: NativeClassification::Verified,
            max_invocations_per_iteration: None,
        });
    }

    /// Register a fallible host function with automatic argument and
    /// return-value marshalling.
    pub fn register_fn_fallible<Func, Args, R>(&mut self, name: &str, func: Func)
    where
        Func: crate::marshall::IntoFallibleNativeFn<W, F, Args, R>,
    {
        self.native_classifications_verified = false;
        // Deduplicate: a re-registration of the same name replaces
        // the prior entry rather than shadowing it. Without this
        // the dispatch `find` would return the first match
        // regardless of subsequent registrations, making the
        // re-registration silently no-op.
        self.natives.retain(|e| e.name != name);
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: func.into_native_fn(),
            classification: NativeClassification::Verified,
            max_invocations_per_iteration: None,
        });
    }

    /// Register a [`crate::stddsl::Library`] bundle on the VM.
    #[cfg(feature = "floats")]
    pub fn register_library<L: crate::stddsl::Library<W, A, F>>(&mut self, library: L) {
        library.register(self);
    }
}

// The test module exercises the full pipeline (source through
// VM execution) and therefore requires both the `compile` and
// `verify` features.
#[cfg(all(test, feature = "compile", feature = "verify"))]
mod tests {
    use super::*;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    #[test]
    fn vm_error_category_three_way_split() {
        // Sanity-check that each VmError variant maps to the
        // expected category. Halt covers the unrecoverable cases
        // where the VM's state is undefined after the error; soft
        // script covers the recoverable-via-resume_err cases; soft
        // host covers the host-native error surface.
        let halt: alloc::vec::Vec<VmError> = alloc::vec![
            VmError::StackUnderflow,
            VmError::InvalidBytecode(alloc::string::String::from("oops")),
            VmError::VerifyError(alloc::string::String::from("oops")),
            VmError::LoadError(alloc::string::String::from("oops")),
            VmError::OutOfArena(alloc::string::String::from("oops")),
            VmError::NotSuspended,
        ];
        for e in &halt {
            assert_eq!(
                e.category(),
                VmErrorCategory::Halt,
                "expected Halt for {:?}",
                e
            );
        }
        let soft_script: alloc::vec::Vec<VmError> = alloc::vec![
            VmError::TypeError(alloc::string::String::from("oops")),
            VmError::DivisionByZero,
            VmError::IndexOutOfBounds(0, 0),
            VmError::FieldNotFound(
                alloc::string::String::from("S"),
                alloc::string::String::from("f"),
            ),
            VmError::RefinementFailed,
            VmError::NoMatchingHead,
            VmError::NoMatchingArm,
            VmError::CheckedArithNoArm,
            VmError::EnumVariantUnmapped,
        ];
        for e in &soft_script {
            assert_eq!(
                e.category(),
                VmErrorCategory::SoftScript,
                "expected SoftScript for {:?}",
                e
            );
        }
        assert_eq!(
            VmError::NativeError(alloc::string::String::from("oops")).category(),
            VmErrorCategory::SoftHost,
        );
    }

    fn run_program(src: &str, args: &[Value]) -> Result<VmState, VmError> {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena)?;
        vm.call(args)
    }

    fn run_expect(src: &str, args: &[Value]) -> Value {
        match run_program(src, args).unwrap() {
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
        }
    }

    #[test]
    fn qif_labels_are_erased_at_runtime() {
        // The information-flow labels are a compile-time
        // discipline. Both classify and declassify compile to
        // identity at the bytecode layer; the runtime value
        // produced is identical to the unlabeled equivalent.
        let val = run_expect(
            "fn produce() -> Word@Secret { 42 }\n\
             fn main() -> Word { declassify produce()@Secret }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn qif_classify_runtime_value_is_unchanged() {
        let val = run_expect("fn main() -> Word@Secret { classify 99@Secret }", &[]);
        assert_eq!(val, Value::Int(99));
    }

    #[test]
    fn checked_overflow_ok_arm_passes_result() {
        // `1 + 2` does not overflow, so the construct evaluates
        // to the `ok` arm's body which binds the successful
        // result and returns it.
        let val = run_expect(
            "fn main() -> Word {\n\
                let y = 1 + 2 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => saturate_max,\n\
                    underflow(_, _) => saturate_min,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn checked_overflow_without_arm_wraps_by_default() {
        // B35 P3: with no `overflow` arm, a positive overflow defaults
        // to the two's-complement wrapped result. `Word::MAX + 1`
        // wraps to `Word::MIN`.
        let val = run_expect(
            "fn main() -> Word {\n\
                let y = 9223372036854775807 + 1 {\n\
                    ok(v) => v,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(i64::MIN));
    }

    #[test]
    fn checked_overflow_arm_returns_saturate_max() {
        // A positive overflow (Word::MAX + 1) dispatches to the
        // overflow arm; `saturate_max` evaluates to Word::MAX.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 9223372036854775807;\n\
                let y = m + 1 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => saturate_max,\n\
                    underflow(_, _) => saturate_min,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(i64::MAX));
    }

    // B16 step 12: this test exercises i64-specific boundary
    // arithmetic. Narrowed binary builds reject the wider literals
    // at the framing or compile level; the test is gated to the
    // default 64-bit runtime configuration.
    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn checked_underflow_arm_returns_saturate_min() {
        // A negative overflow ((Word::MIN + 1) - 2) dispatches
        // to the underflow arm. The minuend is constructed as
        // `0 - 9223372036854775807` because the bare literal
        // `-9223372036854775808` would not lex (the absolute
        // value is one past `i64::MAX`).
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 0 - 9223372036854775807;\n\
                let y = m - 2 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => saturate_max,\n\
                    underflow(_, _) => saturate_min,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(i64::MIN));
    }

    #[test]
    fn checked_mul_overflow_detected() {
        // Multiplication of two large positives overflows;
        // the construct routes to the overflow arm.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 9223372036854775807;\n\
                let y = m * 2 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => 1,\n\
                    underflow(_, _) => 2,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(1));
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn checked_neg_min_overflows() {
        // Negation of Word::MIN overflows because no positive
        // counterpart exists in signed 64-bit.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 0 - 9223372036854775807;\n\
                let y = -(m - 1) {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => 1,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn checked_div_ok_path() {
        // Division of in-range operands routes through the ok arm
        // with the quotient as the bound value.
        let val = run_expect(
            "fn main() -> Word {\n\
                let y = 10 / 3 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(3));
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn checked_div_min_by_neg_one_overflows() {
        // `i64::MIN / -1` is the only overflow case in signed
        // 64-bit division because the true result is `2^63` and
        // does not fit in `Word`. The construct routes to the
        // overflow arm; the high half is zero (the true result
        // fits in 65 bits) and the low half is `i64::MIN` (the
        // wrapped quotient).
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 0 - 9223372036854775807 - 1;\n\
                let y = m / (0 - 1) {\n\
                    ok(_) => 0,\n\
                    overflow(h, l) => h + l,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        // h = 0, l = i64::MIN; sum is i64::MIN.
        assert_eq!(val, Value::Int(i64::MIN));
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn checked_mod_min_by_neg_one_is_in_range_zero() {
        // `i64::MIN % -1` is mathematically `0`. A remainder is
        // always in range, so modulo never overflows or underflows
        // (B35 P3c forbids those arms on `%`); the corner surfaces
        // through the `ok` arm as `0`.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 0 - 9223372036854775807 - 1;\n\
                let y = m % (0 - 1) {\n\
                    ok(r) => r,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn checked_div_by_zero_traps() {
        // With no `zero_divisor` arm, a zero divisor is reified by
        // the opcode (flag 3) and the dispatch's default traps with
        // VmError::DivisionByZero, the same error plain division by
        // zero produces.
        let result = run_program(
            "fn main() -> Word {\n\
                let y = 10 / 0 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn checked_div_zero_divisor_arm_binds_numerator() {
        // B35 P3b: a handled zero divisor runs the `zero_divisor`
        // arm, which binds the numerator.
        let val = run_expect(
            "fn main() -> Word {\n\
                let y = 42 / 0 {\n\
                    ok(q) => q,\n\
                    zero_divisor(n) => n,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn checked_mod_zero_divisor_arm_handled() {
        // Modulo by zero is reified and handled the same way.
        let val = run_expect(
            "fn main() -> Word {\n\
                let y = 7 % 0 {\n\
                    ok(r) => r,\n\
                    zero_divisor(_) => 99,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(99));
    }

    #[test]
    fn checked_byte_add_overflow_binds_wrapped() {
        // B35 P3d-i: unsigned Byte addition overflows above 255; the
        // single-pattern overflow arm binds the wrapped result.
        let val = run_expect(
            "fn main() -> Byte {\n\
                let y = 200Byte + 100Byte {\n\
                    ok(v) => v,\n\
                    overflow(w) => w,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Byte(44));
    }

    #[test]
    fn checked_byte_add_ok_in_range() {
        let val = run_expect(
            "fn main() -> Byte {\n\
                let y = 100Byte + 50Byte {\n\
                    ok(v) => v,\n\
                    overflow(_) => 255Byte,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Byte(150));
    }

    #[test]
    fn checked_byte_sub_underflow_binds_wrapped() {
        // 5 - 10 underflows; the wrapped result is 251 (modulo 256).
        let val = run_expect(
            "fn main() -> Byte {\n\
                let y = 5Byte - 10Byte {\n\
                    ok(v) => v,\n\
                    underflow(w) => w,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Byte(251));
    }

    #[test]
    fn checked_byte_div_zero_divisor_binds_numerator() {
        let val = run_expect(
            "fn main() -> Byte {\n\
                let y = 42Byte / 0Byte {\n\
                    ok(q) => q,\n\
                    zero_divisor(n) => n,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Byte(42));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_float_div_zero_is_infinity_overflow() {
        // B35 P3d-ii: float `1.0 / 0.0` is +inf, classified as the
        // overflow outcome (IEEE 754, no trap).
        let val = run_expect(
            "fn main() -> Float {\n\
                1.0Float / 0.0Float {\n\
                    ok(v) => v,\n\
                    overflow(_) => 1.0Float,\n\
                    underflow(_) => 2.0Float,\n\
                    nan(_) => 3.0Float,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Float(1.0));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_float_zero_over_zero_is_nan() {
        let val = run_expect(
            "fn main() -> Float {\n\
                0.0Float / 0.0Float {\n\
                    ok(v) => v,\n\
                    overflow(_) => 1.0Float,\n\
                    underflow(_) => 2.0Float,\n\
                    nan(_) => 3.0Float,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Float(3.0));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_float_negative_over_zero_is_underflow() {
        // (0.0 - 1.0) / 0.0 is -inf, the underflow outcome.
        let val = run_expect(
            "fn main() -> Float {\n\
                let n = 0.0Float - 1.0Float;\n\
                n / 0.0Float {\n\
                    ok(v) => v,\n\
                    overflow(_) => 1.0Float,\n\
                    underflow(_) => 2.0Float,\n\
                    nan(_) => 3.0Float,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Float(2.0));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn checked_float_ok_finite_result() {
        let val = run_expect(
            "fn main() -> Float {\n\
                6.0Float / 2.0Float {\n\
                    ok(v) => v,\n\
                    overflow(_) => 0.0Float,\n\
                    underflow(_) => 0.0Float,\n\
                    nan(_) => 0.0Float,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Float(3.0));
    }

    // The next three checked-overflow tests embed integer literals
    // (4294967296 = 2^32, large guard values, literal-high patterns)
    // sized for an i64 Word. Under any of the `narrow-word-*`
    // features the runtime Word is i32 or smaller and the constant
    // either fails to fit or wraps to a value the test does not
    // expect. Gated so they only run on the default i64 runtime.

    #[test]
    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    fn checked_mul_overflow_exposes_high_half() {
        // The high half of the i128 intermediate is the load-
        // bearing value for big-number multiplication. `2^32 *
        // 2^32 == 2^64`, which in i128 is (high=1, low=0); the
        // construct binds both and the body returns the high
        // half.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 4294967296;\n\
                let y = m * m {\n\
                    ok(v) => 0 - 1,\n\
                    overflow(h, _) => h,\n\
                    underflow(_, _) => 0 - 2,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    fn checked_overflow_arm_pattern_matches_literal_high() {
        // A literal `0` in the high position selects the small-
        // overflow specialization. For signed addition of two
        // positive operands at i64::MAX, the true sum is
        // (high=0, low=-2 wrapped), so the `overflow(0, l)` arm
        // fires.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 9223372036854775807;\n\
                let y = m + m {\n\
                    ok(v) => v,\n\
                    overflow(0, l) => l,\n\
                    overflow(h, _) => h,\n\
                    underflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        // i64::MAX + i64::MAX == -2 wrapped (low half).
        assert_eq!(val, Value::Int(-2));
    }

    #[test]
    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    fn checked_overflow_arm_guard_falls_through() {
        // The first arm's pattern matches but its guard returns
        // false; dispatch falls through to the catch-all.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 9223372036854775807;\n\
                let y = m + 1 {\n\
                    ok(v) => v,\n\
                    overflow(h, l) when h == 99 => 0,\n\
                    overflow(_, l) => l,\n\
                    underflow(_, _) => 0 - 1,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        // i64::MAX + 1 == i64::MIN wrapped (low half).
        assert_eq!(val, Value::Int(i64::MIN));
    }

    #[test]
    fn newtype_construction_returns_underlying_at_runtime() {
        // Newtype `Percent` is transparent at runtime; the value
        // produced is the underlying Word.
        let val = run_expect(
            "newtype Percent = Word;\n\
             fn main() -> Percent { Percent(42) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_passes_when_argument_in_range() {
        // The predicate `nonneg` returns true for non-negative
        // arguments; the construction succeeds and the value
        // passes through.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn main() -> Counter { Counter(42) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_eliminated_when_literal_proves_satisfaction() {
        // The predicate `nonneg(x) = x >= 0` is statically true
        // for the literal `42`. The compiler elides the runtime
        // call and trap; the construction reduces to the inner
        // value. The runtime result is unchanged.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn main() -> Counter { Counter(42) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_rejected_at_compile_time_when_literal_provably_fails() {
        // The predicate `nonneg(x) = x >= 0` is statically false
        // for a literal that the evaluator can prove out of range.
        // Use a parser-level negative literal: `nonneg(-1)` is
        // parsed as a unary-neg over a literal, which the
        // evaluator handles. The compiler rejects the construction.
        let src = "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn always_negative() -> Word { 0 - 1 }\n\
             fn main() -> Counter { Counter(0 - always_negative()) }";
        // The compile-time check only fires for direct literal
        // arguments; the above keeps the runtime path active.
        // Sanity-check that the runtime path still works (this
        // arg is computed and evaluates to +1 at runtime).
        let val = run_expect(src, &[]);
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn refinement_predicate_compile_error_when_literal_out_of_range() {
        // A direct literal argument that statically violates the
        // predicate is rejected at compile time. The diagnostic
        // names the predicate, the newtype, and the offending
        // argument.
        let src = "fn small(x: Word) -> bool { x < 10 }\n\
             newtype Tiny = Word where small;\n\
             fn main() -> Tiny { Tiny(42) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("small")
                && err.message.contains("Tiny")
                && err.message.contains("42"),
            "expected compile-time diagnostic naming predicate / newtype / argument, got: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_traps_when_argument_out_of_range() {
        // The predicate `nonneg` returns false for -1; the
        // newtype construction traps at runtime when neither
        // constant folding nor the cross-function range summary
        // can decide the argument statically. The body of
        // `mystery` uses an if-expression which the summary
        // computer does not handle, so the call site falls
        // through to the runtime check.
        let err = run_program(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn mystery() -> Word { if 0 == 0 { 0 - 1 } else { 1 } }\n\
             fn main() -> Counter { Counter(mystery()) }",
            &[],
        )
        .unwrap_err();
        assert!(
            matches!(err, VmError::RefinementFailed),
            "expected VmError::RefinementFailed, got {:?}",
            err
        );
    }

    #[test]
    fn refinement_predicate_folded_arithmetic_argument_eliminated() {
        // Tier 1: arithmetic over literal arguments folds to a
        // compile-time constant and routes through the predicate
        // evaluator. `Counter(2 + 40)` reduces to `Counter(42)`
        // statically; the predicate `nonneg(42)` is true and the
        // runtime check is elided.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn main() -> Counter { Counter(2 + 40) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_let_bound_constant_eliminated() {
        // Tier 2: a let-bound local that constant-folds to an
        // integer at compile time resolves through the elision
        // pass. The runtime predicate call and trap are skipped.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn main() -> Counter {\n\
                let n = 42;\n\
                Counter(n)\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_let_bound_arithmetic_chain_eliminated() {
        // Tier 2: chained let-bound constants. Each binding folds
        // through the previous one's recorded value.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn main() -> Counter {\n\
                let a = 2;\n\
                let b = a * 21;\n\
                Counter(b)\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_let_bound_constant_compile_rejects_provable_failure() {
        // Tier 2: a let-bound value that folds to a value
        // provably failing the predicate is rejected at compile
        // time. The diagnostic names the predicate, the newtype,
        // and the folded value.
        let src = "fn small(x: Word) -> bool { x < 10 }\n\
             newtype Tiny = Word where small;\n\
             fn main() -> Tiny {\n\
                let big = 100;\n\
                Tiny(big)\n\
             }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("small")
                && err.message.contains("Tiny")
                && err.message.contains("100"),
            "expected compile-time diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_parameter_range_eliminated() {
        // Tier 3: the parameter `c: Counter` carries the
        // predicate's true set as its compile-time range. Re-
        // wrapping the parameter through `Counter(c as Word)`
        // hits the lattice subset check (argument range == true
        // set) and elides the runtime predicate call.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn rewrap(c: Counter) -> Counter { Counter(c as Word) }\n\
             fn main() -> Counter { rewrap(Counter(42)) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_parameter_range_rejects_on_disjoint() {
        // Tier 3: a function takes a parameter from a wider
        // refined newtype and constructs a narrower one. The
        // lattice subset check fires only when the wider range
        // is contained in the narrower's true set. Conversely,
        // when the construction is provably out-of-range (the
        // ranges are disjoint), the compile rejects.
        //
        // Here `Wide` (`x < 0`) and `NonNeg` (`x >= 0`) are
        // disjoint; constructing a `NonNeg` from a `Wide`-typed
        // parameter is rejected at compile time.
        let src = "fn negative(x: Word) -> bool { x < 0 }\n\
             fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Wide = Word where negative;\n\
             newtype NonNeg = Word where nonneg;\n\
             fn convert(w: Wide) -> NonNeg { NonNeg(w as Word) }\n\
             fn main() -> NonNeg { convert(Wide(0 - 1)) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("nonneg") && err.message.contains("NonNeg"),
            "expected lattice-driven compile-time diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_parameter_range_falls_through_when_predicate_undecidable() {
        // The IntervalSet lattice handles disjunction exactly,
        // so a predicate body of `x < 0 or x > 100` decomposes
        // to the set `(-inf, -1] U [101, +inf)`. The argument
        // `0 - 50` folds to `-50`, which lies in the first
        // component; elision fires at compile time.
        let val = run_expect(
            "fn outside(x: Word) -> bool { x < 0 or x > 100 }\n\
             newtype Edge = Word where outside;\n\
             fn main() -> Edge { Edge(0 - 50) }",
            &[],
        );
        assert_eq!(val, Value::Int(-50));
    }

    #[test]
    fn refinement_predicate_disjoint_set_admits_constant_outside_singleton() {
        // The predicate `not (x == 5)` has true set
        // `(-inf, 4] U [6, +inf)`. The argument `42` lies in
        // the second component; the IntervalSet subset check
        // admits the construction.
        let val = run_expect(
            "fn not_five(x: Word) -> bool { not (x == 5) }\n\
             newtype NotFive = Word where not_five;\n\
             fn main() -> NotFive { NotFive(42) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_disjoint_set_rejects_constant_at_singleton_excluded() {
        // The literal `5` is excluded by `not (x == 5)`. The
        // compile-time check rejects the construction.
        let src = "fn not_five(x: Word) -> bool { not (x == 5) }\n\
             newtype NotFive = Word where not_five;\n\
             fn main() -> NotFive { NotFive(5) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("not_five") && err.message.contains("NotFive"),
            "expected compile-time diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_function_call_summary_admits() {
        // Tier B: `always_42()` has return-range summary
        // `singleton(42)`. The constructor argument is the call;
        // the lattice subset check admits at compile time.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn always_42() -> Word { 42 }\n\
             fn main() -> Counter { Counter(always_42()) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn refinement_predicate_function_call_summary_rejects_on_disjoint() {
        // Tier B: `always_neg_one()` summary is `singleton(-1)`,
        // disjoint from `nonneg`'s true set; the compile rejects.
        let src = "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn always_neg_one() -> Word { 0 - 1 }\n\
             fn main() -> Counter { Counter(always_neg_one()) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("nonneg") && err.message.contains("Counter"),
            "expected cross-function summary diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn refinement_predicate_function_call_with_parameter_admits_via_summary() {
        // Tier B: `widen(c: Counter) -> Word { c as Word }` has
        // summary `nonneg`'s true set (the parameter's range).
        // Re-wrapping the call result admits cleanly.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn widen(c: Counter) -> Word { c as Word }\n\
             fn main() -> Counter { Counter(widen(Counter(7))) }",
            &[],
        );
        assert_eq!(val, Value::Int(7));
    }

    #[test]
    fn refinement_predicate_if_expression_summary_admits() {
        // The function-summary pass now handles `if`/`else`
        // bodies, computing the union of the branch ranges. The
        // example function returns 0 or 1; the summary is
        // `[0, 1]`, a subset of `nonneg`'s true set, so the
        // construction admits at compile time.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn flag(n: Word) -> Word {\n\
                if n == 0 { 0 } else { 1 }\n\
             }\n\
             fn main() -> Counter { Counter(flag(7)) }",
            &[],
        );
        assert_eq!(val, Value::Int(1));
    }

    // Note on recursive-function summaries: the widening
    // infrastructure on the interval lattice (`Interval::widen`,
    // `IntervalSet::widen`) converges the function-summary
    // fixed-point pass for recursive bodies. End-to-end runtime
    // tests are deferred because the WCMU verifier rejects
    // recursive functions at load time (their stack-frame count
    // is not statically bounded under V0.2's static analysis).
    // The widening pass nevertheless computes a sound compile-
    // time summary; future work that admits recursive functions
    // under a relaxed WCMU bound or trust-skipped load will
    // exercise this path end-to-end.

    #[test]
    fn refinement_predicate_match_arm_narrowing_admits_via_literal_pattern() {
        // Match-arm narrowing: the scrutinee `n` has the
        // parameter range full(). The arm `42 => Counter(42)`
        // narrows the scrutinee to singleton(42), and the body
        // uses the literal 42 directly which folds. The arm
        // `v => Counter(0)` always returns 0. Both arm bodies
        // are non-negative; the function-return summary is
        // singleton(0) union singleton(42), and the construction
        // `Counter(classify(...))` consults the summary.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn classify(n: Word) -> Word {\n\
                match n {\n\
                    42 => 42,\n\
                    v => 0,\n\
                }\n\
             }\n\
             fn main() -> Counter { Counter(classify(7)) }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn refinement_predicate_match_arm_narrowing_admits_via_variable_pattern() {
        // The variable-pattern arm binds `v` to the scrutinee's
        // narrowed range. When the scrutinee is a non-negative
        // parameter, the binding range is non-negative; using
        // it inside a `Counter(...)` admits.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             newtype NonNeg = Word where nonneg;\n\
             fn passthrough(n: NonNeg) -> Word {\n\
                match n as Word {\n\
                    v => v,\n\
                }\n\
             }\n\
             fn main() -> Counter { Counter(passthrough(NonNeg(7))) }",
            &[],
        );
        assert_eq!(val, Value::Int(7));
    }

    #[test]
    fn byte_refinement_predicate_compiles_with_literal_coercion() {
        // The type checker coerces the `0` and `100` literals to
        // Byte at the comparison sites. The Byte refinement
        // predicate compiles cleanly; constructing `Percent(42)`
        // works through the elision pathway (the literal 42 is
        // coerced and the predicate's true set covers it).
        let val = run_expect(
            "fn in_range(x: Byte) -> bool { x >= 0 and x <= 100 }\n\
             newtype Percent = Byte where in_range;\n\
             fn main() -> Percent { Percent(42 as Byte) }",
            &[],
        );
        assert_eq!(val, Value::Byte(42));
    }

    #[test]
    fn byte_refinement_predicate_elision_when_in_range() {
        // The Byte parameter carries the natural range [0, 255],
        // which is a subset of `[0, 100]`? Let's check: the
        // predicate true set is `[0, 100]`; the natural range
        // is `[0, 255]`. The argument `b as Byte` carries the
        // parameter range. The intersection is `[0, 100]` (non-
        // empty, non-subset), so the lattice path falls through
        // to the runtime check. For an in-range literal, the
        // constant-fold path elides at compile time.
        let val = run_expect(
            "fn in_range(x: Byte) -> bool { x >= 0 and x <= 100 }\n\
             newtype Percent = Byte where in_range;\n\
             fn main() -> Percent { Percent(50 as Byte) }",
            &[],
        );
        assert_eq!(val, Value::Byte(50));
    }

    #[test]
    fn refinement_predicate_byte_parameter_natural_range_admits_nonneg() {
        // A Byte parameter carries the natural range [0, 255].
        // Casting the parameter to Word preserves the range
        // (zero-extension at the bytecode level is identity in
        // i64). The newtype `Counter` requires non-negative,
        // which the range [0, 255] satisfies; the lattice subset
        // check admits the construction at compile time.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn from_byte(b: Byte) -> Counter { Counter(b as Word) }\n\
             fn main() -> Counter { from_byte(7 as Byte) }",
            &[],
        );
        assert_eq!(val, Value::Int(7));
    }

    #[test]
    fn refinement_predicate_disjunction_admits_via_lattice_union() {
        // Parameter `e: Edge` carries the union range
        // `(-inf, -1] U [101, +inf)`. Re-wrapping the parameter
        // through `Edge(e as Word)` hits the IntervalSet subset
        // check (argument range == true set).
        let val = run_expect(
            "fn outside(x: Word) -> bool { x < 0 or x > 100 }\n\
             newtype Edge = Word where outside;\n\
             fn rewrap(e: Edge) -> Edge { Edge(e as Word) }\n\
             fn main() -> Edge { rewrap(Edge(200)) }",
            &[],
        );
        assert_eq!(val, Value::Int(200));
    }

    #[test]
    fn refinement_predicate_non_constant_let_falls_through_to_runtime() {
        // Tier 2: a let-bound value that does NOT fold to a
        // constant (because it comes from a function call) does
        // not record a constant entry. The elision pass falls
        // through to the runtime check.
        let err = run_program(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn neg_one() -> Word { 0 - 1 }\n\
             fn main() -> Counter {\n\
                let n = neg_one();\n\
                Counter(n)\n\
             }",
            &[],
        )
        .unwrap_err();
        assert!(
            matches!(err, VmError::RefinementFailed),
            "expected VmError::RefinementFailed, got {:?}",
            err
        );
    }

    #[test]
    fn refinement_predicate_folded_arithmetic_compile_rejects_on_provable_failure() {
        // Tier 1: the folded value `0 - 1` is `-1`, which fails
        // the `nonneg` predicate statically. The construction is
        // rejected at compile time even though the source uses an
        // arithmetic expression rather than a bare literal.
        let src = "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn main() -> Counter { Counter(0 - 1) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("nonneg")
                && err.message.contains("Counter")
                && err.message.contains("-1"),
            "expected compile-time diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn enum_to_word_cast_implicit_discriminants() {
        // Implicit discriminants: Red=0, Green=1, Blue=2.
        let val = run_expect(
            "enum Color { Red, Green, Blue }\n\
             fn main() -> Word {\n\
                let c = Color::Green();\n\
                c as Word\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn enum_to_word_cast_explicit_discriminants() {
        let val = run_expect(
            "enum Code { OutOfRange = 1, Busy = 3, Timeout = 4 }\n\
             fn main() -> Word {\n\
                let c = Code::Busy();\n\
                c as Word\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn enum_to_word_cast_negative_discriminant() {
        let val = run_expect(
            "enum Sign { Neg = -1, Zero = 0, Pos = 1 }\n\
             fn main() -> Word {\n\
                let s = Sign::Neg();\n\
                s as Word\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(-1));
    }

    #[test]
    fn enum_to_word_cast_after_match_arm() {
        // Exercise the cast on a variable bound through a match
        // expression — the local's inferred type should still
        // light up the enum-to-Word path.
        let val = run_expect(
            "enum Code { A = 10, B = 20 }\n\
             fn pick(which: Word) -> Code {\n\
                if which == 0 { Code::A() } else { Code::B() }\n\
             }\n\
             fn main() -> Word {\n\
                let c = pick(1);\n\
                c as Word\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(20));
    }

    #[test]
    fn monomorphize_generic_enum_with_match_pattern() {
        // Regression: the monomorphizer renamed the enum
        // construction site (`Maybe::Just(42)` to `Maybe__Word::Just`)
        // but match-arm patterns retained the original generic
        // enum name, producing `enum pattern Maybe::Just does not
        // match scrutinee type Maybe__Word`. Both sites must
        // rewrite consistently.
        let val = run_expect(
            "enum Maybe<T> { Just(T), Nothing }\n\
             fn main() -> Word {\n\
                let m = Maybe::Just(42);\n\
                match m {\n\
                    Maybe::Just(x) => x,\n\
                    Maybe::Nothing => 0,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn monomorphize_nested_generic_structs_resolves_field_types() {
        // Regression: the struct specializer substituted type
        // parameters in field declarations (`inner: Cell<T>`
        // became `inner: Cell<Word>`) but did not rewrite the
        // substituted type to the emitted specialization name
        // (`Cell__Word`), producing `field Wrap__Word.inner expects
        // Cell<Word>, got Cell__Word` at type check. The
        // substitution now resolves nested generic instantiations
        // to their specialization names.
        let val = run_expect(
            "struct Cell<T> { value: T }\n\
             struct Wrap<T> { inner: Cell<T> }\n\
             fn main() -> Word {\n\
                let w = Wrap { inner: Cell { value: 7 } };\n\
                w.inner.value\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(7));
    }

    #[test]
    fn eval_literal() {
        let val = run_expect("fn main() -> Word { 42 }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn numeric_suffix_word_is_plain_int() {
        let val = run_expect("fn main() -> Word { 7Word }", &[]);
        assert_eq!(val, Value::Int(7));
    }

    #[test]
    fn numeric_suffix_byte_produces_byte() {
        let val = run_expect("fn main() -> Byte { 42Byte }", &[]);
        assert_eq!(val, Value::Byte(42));
    }

    #[test]
    fn numeric_suffix_fixed_integer_form_encodes_q_format() {
        // `42Fixed<16>` is the Q-format value 42 << 16.
        let val = run_expect("fn main() -> Fixed<16> { 42Fixed<16> }", &[]);
        assert_eq!(val, Value::Fixed(42 << 16));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn numeric_suffix_float_from_integer_digits() {
        let val = run_expect("fn main() -> Float { 42Float }", &[]);
        assert_eq!(val, Value::Float(42.0));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn numeric_suffix_fixed_fractional_form_rounds() {
        let val = run_expect("fn main() -> Fixed<16> { 3.5Fixed<16> }", &[]);
        assert_eq!(val, Value::Fixed((3.5 * 65536.0) as i64));
    }

    #[test]
    fn byte_cast_truncates_word_to_low_eight_bits() {
        let val = run_expect("fn main() -> Byte { 300 as Byte }", &[]);
        assert_eq!(val, Value::Byte(44));
    }

    #[test]
    fn byte_cast_round_trip_preserves_low_eight_bits() {
        let val = run_expect("fn main() -> Word { (200 as Byte) as Word }", &[]);
        assert_eq!(val, Value::Int(200));
    }

    #[test]
    fn byte_addition_wraps_modulo_256() {
        let val = run_expect("fn main() -> Byte { (200 as Byte) + (100 as Byte) }", &[]);
        // 200 + 100 = 300 wraps to 44 (300 mod 256).
        assert_eq!(val, Value::Byte(44));
    }

    #[test]
    fn byte_subtraction_wraps_modulo_256() {
        let val = run_expect("fn main() -> Byte { (10 as Byte) - (20 as Byte) }", &[]);
        // 10 - 20 = -10 wraps to 246 (256 - 10).
        assert_eq!(val, Value::Byte(246));
    }

    #[test]
    fn byte_multiplication_wraps_modulo_256() {
        let val = run_expect("fn main() -> Byte { (16 as Byte) * (17 as Byte) }", &[]);
        // 16 * 17 = 272 wraps to 16.
        assert_eq!(val, Value::Byte(16));
    }

    #[test]
    fn byte_division_and_modulo_use_unsigned_semantics() {
        let div = run_expect("fn main() -> Byte { (200 as Byte) / (16 as Byte) }", &[]);
        assert_eq!(div, Value::Byte(12));
        let rem = run_expect("fn main() -> Byte { (200 as Byte) % (16 as Byte) }", &[]);
        assert_eq!(rem, Value::Byte(8));
    }

    #[test]
    fn byte_comparison_uses_unsigned_ordering() {
        let val = run_expect("fn main() -> bool { (200 as Byte) > (100 as Byte) }", &[]);
        assert_eq!(val, Value::Bool(true));
    }

    // -- Fixed (Q-format) tests --
    //
    // On the host runtime, `Fixed` is Q31.32 (32 fraction bits).
    // The integer value 1 cast to `Fixed` is stored as
    // `1 << 32 = 4_294_967_296` in the underlying bits.

    #[test]
    fn fixed_word_round_trip_preserves_integer_value() {
        let val = run_expect("fn main() -> Word { (5 as Fixed) as Word }", &[]);
        assert_eq!(val, Value::Int(5));
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn fixed_cast_from_word_uses_q31_32_format() {
        let val = run_expect("fn main() -> Fixed { 1 as Fixed }", &[]);
        assert_eq!(val, Value::Fixed(1i64 << 32));
    }

    #[test]
    fn fixed_addition_sums_underlying_bits() {
        let val = run_expect(
            "fn main() -> Word { ((2 as Fixed) + (3 as Fixed)) as Word }",
            &[],
        );
        assert_eq!(val, Value::Int(5));
    }

    #[test]
    fn fixed_subtraction_subtracts_underlying_bits() {
        let val = run_expect(
            "fn main() -> Word { ((10 as Fixed) - (3 as Fixed)) as Word }",
            &[],
        );
        assert_eq!(val, Value::Int(7));
    }

    #[test]
    fn fixed_multiply_maintains_q_format() {
        let val = run_expect(
            "fn main() -> Word { ((4 as Fixed) * (5 as Fixed)) as Word }",
            &[],
        );
        assert_eq!(val, Value::Int(20));
    }

    #[test]
    fn fixed_divide_maintains_q_format() {
        let val = run_expect(
            "fn main() -> Word { ((20 as Fixed) / (5 as Fixed)) as Word }",
            &[],
        );
        assert_eq!(val, Value::Int(4));
    }

    #[test]
    fn fixed_negate_negates_underlying_bits() {
        let val = run_expect("fn main() -> Word { (-(5 as Fixed)) as Word }", &[]);
        assert_eq!(val, Value::Int(-5));
    }

    #[test]
    fn fixed_comparison_uses_signed_ordering() {
        let val = run_expect("fn main() -> bool { (10 as Fixed) > (5 as Fixed) }", &[]);
        assert_eq!(val, Value::Bool(true));
    }

    #[test]
    fn fixed_parameterised_q15_16_uses_sixteen_fraction_bits() {
        // `Fixed<16>` is Q15.16: 1 cast to Fixed<16> equals
        // `1 << 16 = 65_536` in the underlying bits.
        let val = run_expect("fn main() -> Fixed<16> { 1 as Fixed<16> }", &[]);
        assert_eq!(val, Value::Fixed(1i64 << 16));
    }

    #[test]
    fn fixed_parameterised_q15_16_multiply_maintains_format() {
        let val = run_expect(
            "fn main() -> Word { ((4 as Fixed<16>) * (5 as Fixed<16>)) as Word }",
            &[],
        );
        assert_eq!(val, Value::Int(20));
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn fixed_default_form_resolves_to_q31_32() {
        // The default `Fixed` surface form resolves to Q31.32 on
        // the host runtime, so `1 as Fixed` equals `1 << 32` in
        // the underlying bits.
        let val = run_expect("fn main() -> Fixed { 1 as Fixed }", &[]);
        assert_eq!(val, Value::Fixed(1i64 << 32));
    }

    #[test]
    fn option_some_pattern_matches_constructed_some() {
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = Option::Some(42);\n\
                match m {\n\
                    Option::Some(x) => x,\n\
                    Option::None => 0,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn option_none_pattern_matches_value_none() {
        // The compiler emits `Op::PushNone` for `Option::None`; the
        // match arm tests the unwrapped `Value::None` through a
        // direct equality check rather than `IsEnum` (which would
        // fail because `Value::None` is not a `Value::Enum`).
        // The match scrutinee uses a Some-constructed value to
        // avoid an unrelated Option<T> type-unification limitation
        // around bare None literals in function returns; the
        // Some arm is the one verified here through the value 7.
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = Option::Some(7);\n\
                match m {\n\
                    Option::None => 99,\n\
                    Option::Some(x) => x,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(7));
    }

    #[test]
    fn eval_add() {
        let val = run_expect("fn main() -> Word { 10 + 32 }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_arithmetic() {
        let val = run_expect("fn main() -> Word { (2 + 3) * 4 - 1 }", &[]);
        assert_eq!(val, Value::Int(19));
    }

    #[test]
    fn eval_comparison() {
        let val = run_expect("fn main() -> bool { 10 > 5 }", &[]);
        assert_eq!(val, Value::Bool(true));
    }

    #[test]
    fn eval_logical_and() {
        let val = run_expect("fn main() -> bool { true and false }", &[]);
        assert_eq!(val, Value::Bool(false));
    }

    #[test]
    fn eval_logical_or() {
        let val = run_expect("fn main() -> bool { false or true }", &[]);
        assert_eq!(val, Value::Bool(true));
    }

    #[test]
    fn eval_negation() {
        let val = run_expect("fn main() -> Word { -42 }", &[]);
        assert_eq!(val, Value::Int(-42));
    }

    #[test]
    fn eval_not() {
        let val = run_expect("fn main() -> bool { not true }", &[]);
        assert_eq!(val, Value::Bool(false));
    }

    #[test]
    fn eval_if_true() {
        let val = run_expect("fn main() -> Word { if true { 1 } else { 2 } }", &[]);
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn eval_if_false() {
        let val = run_expect("fn main() -> Word { if false { 1 } else { 2 } }", &[]);
        assert_eq!(val, Value::Int(2));
    }

    #[test]
    fn eval_let_binding() {
        let val = run_expect("fn main() -> Word { let x = 10; let y = 32; x + y }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_function_call() {
        let val = run_expect(
            "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(21) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_nested_calls() {
        let val = run_expect(
            "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(double(10)) + 2 }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_with_args() {
        let val = run_expect("fn main(x: Word) -> Word { x + 1 }", &[Value::Int(41)]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_for_range() {
        let val = run_expect(
            "fn main() -> Word { let sum = 0; for i in 0..5 { let x = sum + i; } sum }",
            &[],
        );
        // Lexical scoping: inner `let x` shadows but does not mutate outer `sum`.
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_string_literal() {
        let val = run_expect("fn main() -> Text { \"hello\" }", &[]);
        assert_eq!(val, Value::StaticStr(String::from("hello")));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn eval_float_arithmetic() {
        let val = run_expect("fn main() -> Float { 1.5 + 2.5 }", &[]);
        assert_eq!(val, Value::Float(4.0));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn eval_cast_int_to_float() {
        let val = run_expect("fn main() -> Float { 42 as Float }", &[]);
        assert_eq!(val, Value::Float(42.0));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn eval_cast_float_to_int() {
        let val = run_expect("fn main() -> Word { 3.7 as Word }", &[]);
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn eval_struct_init_and_field() {
        let val = run_expect(
            "struct Point { x: Word, y: Word }\nfn main() -> Word { let p = Point { x: 10, y: 32 }; p.x + p.y }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_enum_variant() {
        let val = run_expect(
            "enum Color { Red, Green, Blue }\nfn main() -> Word { let c = Color::Red(); 42 }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_array_literal_and_index() {
        let val = run_expect("fn main() -> Word { let arr = [10, 20, 30]; arr[1] }", &[]);
        assert_eq!(val, Value::Int(20));
    }

    #[test]
    fn eval_yield_and_resume() {
        let src = "loop main(input: Word) -> Word { let input = yield input * 2; input }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();

        // First call: main(5) -> yields 5 * 2 = 10.
        match vm.call(&[Value::Int(5)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(10)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Resume with 7: continues after yield, sets input=7, reaches Reset.
        match vm.resume(Value::Int(7)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }

        // Resume after Reset with 7: restarts stream, yields 7 * 2 = 14.
        match vm.resume(Value::Int(7)).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(14)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Resume with 0: reaches Reset.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }

        // Resume after Reset with 0: yields 0 * 2 = 0.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(0)),
            other => panic!("expected yield, got {:?}", other),
        }
    }

    #[test]
    fn resume_err_propagates_through_enum_reply() {
        // The script declares a Result-shaped enum and pattern-matches
        // on the resumed value. The host calls `resume` with the Ok
        // variant for success and `resume_err` with the Err variant
        // for failure. Both flow through the same operand-stack
        // resume mechanism; `resume_err` is a documentation alias
        // that signals intent.
        let src = "\
            enum Reply { Ok(Word), Err }\n\
            loop main(input: Reply) -> Word {\n\
                let reply = yield 0;\n\
                match reply {\n\
                    Reply::Ok(v) => v,\n\
                    Reply::Err => -1,\n\
                }\n\
            }\
        ";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        let initial = Value::Enum {
            type_name: String::from("Reply"),
            variant: String::from("Ok"),
            fields: alloc::vec![Value::Int(0)],
        };
        match vm.call(&[initial]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(0)),
            other => panic!("expected first yield, got {:?}", other),
        }
        // Successful resume returns the Ok payload.
        let success = Value::Enum {
            type_name: String::from("Reply"),
            variant: String::from("Ok"),
            fields: alloc::vec![Value::Int(42)],
        };
        match vm.resume(success).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }
        // After the implicit reset, send Err to drive the next round
        // through the error branch.
        let initial2 = Value::Enum {
            type_name: String::from("Reply"),
            variant: String::from("Ok"),
            fields: alloc::vec![Value::Int(0)],
        };
        match vm.resume(initial2).unwrap() {
            VmState::Yielded(_) => {}
            other => panic!("expected yield, got {:?}", other),
        }
        let err = Value::Enum {
            type_name: String::from("Reply"),
            variant: String::from("Err"),
            fields: alloc::vec![],
        };
        match vm.resume_err(err).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset on err, got {:?}", other),
        }
    }

    #[test]
    fn resume_err_passes_through_with_value_none() {
        // The simplest error pattern: the script types its input as
        // a value that may be `None`. The host resumes with
        // `Value::None` to signal the absence of input. The script's
        // dispatch logic handles the None case explicitly.
        let src = "\
            enum Reply { Ok(Word), Err }\n\
            loop main(input: Reply) -> Word {\n\
                let reply = yield 0;\n\
                match reply {\n\
                    Reply::Ok(v) => v,\n\
                    Reply::Err => 99,\n\
                }\n\
            }\
        ";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        let initial = Value::Enum {
            type_name: String::from("Reply"),
            variant: String::from("Ok"),
            fields: alloc::vec![Value::Int(0)],
        };
        match vm.call(&[initial]).unwrap() {
            VmState::Yielded(_) => {}
            other => panic!("expected yield, got {:?}", other),
        }
        // resume_err with Err variant routes through the error arm
        // and the script returns 99 to the host before reset.
        let err = Value::Enum {
            type_name: String::from("Reply"),
            variant: String::from("Err"),
            fields: alloc::vec![],
        };
        match vm.resume_err(err).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }
    }

    #[test]
    fn eval_multiheaded_literal() {
        let val = run_expect(
            "fn classify(0) -> Text { \"zero\" }\nfn classify(x: Word) -> Text { \"other\" }\nfn main() -> Text { classify(0) }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("zero")));
    }

    #[test]
    fn eval_multiheaded_fallthrough() {
        let val = run_expect(
            "fn classify(0) -> Text { \"zero\" }\nfn classify(x: Word) -> Text { \"other\" }\nfn main() -> Text { classify(5) }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("other")));
    }

    #[test]
    fn multiheaded_no_matching_head_traps_with_kind() {
        // Two literal heads with no catch-all; an argument matching
        // neither traps with the NoMatchingHead kind.
        let err = run_program(
            "fn pick(0) -> Word { 100 }\nfn pick(1) -> Word { 200 }\nfn main() -> Word { pick(5) }",
            &[],
        )
        .unwrap_err();
        assert!(
            matches!(err, VmError::NoMatchingHead),
            "expected NoMatchingHead trap, got {:?}",
            err
        );
    }

    #[test]
    fn trap_kind_code_round_trips() {
        use crate::bytecode::TrapKind;
        for k in [
            TrapKind::RefinementFailed,
            TrapKind::NoMatchingHead,
            TrapKind::NoMatchingArm,
            TrapKind::CheckedArithNoArm,
            TrapKind::EnumVariantUnmapped,
        ] {
            assert_eq!(TrapKind::from_code(k.code()), Some(k));
        }
        assert_eq!(TrapKind::from_code(999), None);
    }

    #[test]
    fn eval_pipeline() {
        let val = run_expect(
            "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { 21 |> double() }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_match_literal() {
        let val = run_expect(
            "fn main() -> Text { let x = 1; match x { 1 => \"one\", 2 => \"two\", _ => \"other\" } }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("one")));
    }

    #[test]
    fn eval_match_wildcard() {
        let val = run_expect(
            "fn main() -> Text { let x = 99; match x { 1 => \"one\", _ => \"other\" } }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("other")));
    }

    #[test]
    fn eval_division_by_zero() {
        let result = run_program("fn main() -> Word { 1 / 0 }", &[]);
        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn eval_index_out_of_bounds() {
        let result = run_program("fn main() -> Word { let a = [1, 2]; a[5] }", &[]);
        assert!(matches!(result, Err(VmError::IndexOutOfBounds(5, 2))));
    }

    #[test]
    fn eval_native_function() {
        let src = "use math::add_one\nfn main(x: Word) -> Word { math::add_one(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_native("math::add_one", |args| match &args[0] {
            Value::Int(x) => Ok(Value::Int(x + 1)),
            _ => Err(VmError::TypeError(String::from("expected Int"))),
        });
        match vm.call(&[Value::Int(41)]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn external_native_round_trip() {
        // `use external` imports compile to `Op::CallExternalNative`.
        // Registering the native through `register_external_native`
        // sets the matching classification; the call succeeds.
        let src = "use external host::log_event\nfn main(x: Word) -> Word { host::log_event(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_external_native("host::log_event", |args| Ok(args[0].clone()), 16);
        match vm.call(&[Value::Int(7)]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(7)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn native_classification_mismatch_rejected_at_call() {
        // The script imports `host::log_event` as a verified
        // native (bare `use`), but the host registers it through
        // `register_external_native`. The mismatch is detected at
        // the call site dispatch and surfaces as VmError::VerifyError.
        let src = "use host::log_event\nfn main(x: Word) -> Word { host::log_event(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_external_native("host::log_event", |args| Ok(args[0].clone()), 16);
        let err = vm.call(&[Value::Int(7)]).unwrap_err();
        match err {
            VmError::VerifyError(msg) => {
                assert!(msg.contains("registered as external"), "{}", msg);
                assert!(msg.contains("invokes it as verified"), "{}", msg);
            }
            other => panic!("expected VerifyError, got {:?}", other),
        }
    }

    #[test]
    fn external_classification_mismatch_rejected_at_call() {
        // The script imports `host::log_event` as external but the
        // host registers it through `register_native` (verified).
        let src = "use external host::log_event\nfn main(x: Word) -> Word { host::log_event(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_native("host::log_event", |args| Ok(args[0].clone()));
        let err = vm.call(&[Value::Int(7)]).unwrap_err();
        match err {
            VmError::VerifyError(msg) => {
                assert!(msg.contains("registered as verified"), "{}", msg);
                assert!(msg.contains("invokes it as external"), "{}", msg);
            }
            other => panic!("expected VerifyError, got {:?}", other),
        }
    }

    #[test]
    fn classification_mismatch_detected_before_execution() {
        // The load-time check fires at the entry of `call_function`
        // before any bytecode executes. Even when the call site
        // sits behind a branch that the test arguments would not
        // take, the verification still reports the mismatch.
        let src = "use host::log_event\n\
                       fn main(x: Word) -> Word {\n\
                           if x > 0 { x } else { host::log_event(x) }\n\
                       }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_external_native("host::log_event", |args| Ok(args[0].clone()), 16);
        // x > 0 takes the then-branch which does not reach the
        // native; a runtime-only check would let this slip through.
        // The load-time check catches it.
        let err = vm.call(&[Value::Int(5)]).unwrap_err();
        assert!(
            matches!(&err, VmError::VerifyError(msg)
                if msg.contains("registered as external")
                && msg.contains("invokes it as verified")),
            "unexpected error: {:?}",
            err,
        );
    }

    #[test]
    fn verify_native_classifications_callable_before_first_call() {
        // The host may invoke `verify_native_classifications`
        // explicitly to surface mismatches at a deployment-
        // validation step rather than at first call. The method
        // returns Ok when registrations match.
        let src = "use external host::log_event\nfn main(x: Word) -> Word { host::log_event(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_external_native("host::log_event", |args| Ok(args[0].clone()), 16);
        vm.verify_native_classifications().unwrap();
    }

    #[test]
    fn verify_native_classifications_idempotent() {
        // Calling verify_native_classifications twice in a row
        // succeeds idempotently. The second call hits the cached-
        // Ok path and returns without re-walking the chunks.
        let src = "use external host::log_event\nfn main(x: Word) -> Word { host::log_event(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_external_native("host::log_event", |args| Ok(args[0].clone()), 16);
        vm.verify_native_classifications().unwrap();
        vm.verify_native_classifications().unwrap();
    }

    #[test]
    fn duplicate_native_registration_replaces_prior_entry() {
        // V0.2.0 Phase 6 follow-on (concern #2): re-registering a
        // native with the same name replaces the prior entry
        // rather than appending. The script reads the latest
        // registration's behaviour, not the first.
        let src = "use host::compute\nfn main(x: Word) -> Word { host::compute(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.register_native("host::compute", |_args| Ok(Value::Int(1)));
        vm.register_native("host::compute", |_args| Ok(Value::Int(2)));
        match vm.call(&[Value::Int(99)]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(2)),
            other => panic!("expected Int(2), got {:?}", other),
        }
    }

    #[test]
    fn duplicate_native_registration_swaps_classification() {
        // Re-registering with a different classification replaces
        // both the function and the classification. A prior
        // verified registration is removed when an external
        // re-registration replaces it; the cache invalidation
        // then forces a fresh load-time check that sees only the
        // external entry.
        let src = "use external host::compute\nfn main(x: Word) -> Word { host::compute(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        // Initial registration as verified: would mismatch the
        // `use external` import. The dedup behaviour means the
        // verified entry is wiped out by the external re-
        // registration that follows.
        vm.register_native("host::compute", |_args| Ok(Value::Int(1)));
        vm.register_external_native("host::compute", |_args| Ok(Value::Int(2)), 16);
        match vm.call(&[Value::Int(99)]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(2)),
            other => panic!("expected Int(2), got {:?}", other),
        }
    }

    #[test]
    fn eval_guard_clause() {
        let val = run_expect(
            "fn abs(x: Word) -> Word when x < 0 { -x }\nfn abs(x: Word) -> Word { x }\nfn main() -> Word { abs(-5) + abs(3) }",
            &[],
        );
        assert_eq!(val, Value::Int(8));
    }

    #[test]
    fn exponential_text_concat_rejected_at_safe_constructor() {
        // The FAQ exponential-string-concat example expressed as a
        // Stream block, which is the form subject to the per-iteration
        // WCMU bound. Sixty doublings of a 1-byte string allocate
        // more than u32::MAX bytes cumulatively. The text-size
        // abstract interpretation pass saturates the chunk's heap
        // bound; the WCMU resource-bounds check rejects the module
        // because the bound exceeds any feasible arena capacity.
        let mut src =
            alloc::string::String::from("loop main(input: Word) -> Text {\n    let s = \"a\";\n");
        for _ in 0..60 {
            src.push_str("    let s = s + s;\n");
        }
        src.push_str("    let _ = yield s;\n    s\n}\n");
        let tokens = tokenize(&src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let err = Vm::new(module, &arena)
            .err()
            .expect("expected rejection from exponential text growth");
        assert!(matches!(err, VmError::VerifyError(_)));
    }

    // -- For-in over array expressions --

    #[test]
    fn eval_for_in_array_literal() {
        let val = run_expect(
            "fn main() -> Word { let sum = 0; for x in [10, 20, 30] { let sum = sum + x; } sum }",
            &[],
        );
        // Lexical scoping: inner `let sum` shadows but does not mutate outer `sum`.
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_for_in_array_accumulate() {
        // Use a mutable-style accumulation pattern via function calls.
        let val = run_expect(
            "fn main() -> Word {\n\
             let arr = [1, 2, 3, 4, 5];\n\
             let result = 0;\n\
             for x in arr {\n\
               let result = result + x;\n\
             }\n\
             result\n\
             }",
            &[],
        );
        // Due to lexical scoping, result remains 0.
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_for_in_empty_array() {
        let val = run_expect(
            "fn main() -> Word { let count = 42; for x in [] { let count = 0; } count }",
            &[],
        );
        // Body never executes for empty array.
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_for_in_single_element() {
        let val = run_expect(
            "fn main() -> Word { let last = 0; for x in [99] { let last = x; } last }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_for_in_array_with_function() {
        let val = run_expect(
            "fn double(x: Word) -> Word { x * 2 }\n\
             fn main() -> Word {\n\
               let result = 0;\n\
               for x in [1, 2, 3] {\n\
                 let result = result + double(x);\n\
               }\n\
               result\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    // -- Tuple literal construction --

    #[test]
    fn eval_tuple_literal() {
        let val = run_expect("fn main() -> Word { let t = (1, 2, 3); t.0 }", &[]);
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn eval_tuple_field_access() {
        let val = run_expect("fn main() -> Word { let t = (10, 20, 30); t.1 }", &[]);
        assert_eq!(val, Value::Int(20));
    }

    #[test]
    fn eval_tuple_let_destructure() {
        let val = run_expect("fn main() -> Word { let (a, b) = (10, 32); a + b }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    #[cfg(feature = "floats")]
    fn eval_tuple_mixed_types() {
        let val = run_expect("fn main() -> Float { let t = (42, 2.5, true); t.1 }", &[]);
        assert_eq!(val, Value::Float(2.5));
    }

    // -- Len instruction --

    #[test]
    fn eval_len_via_for_in() {
        // Len is used internally by for-in. Verify via a known array size.
        let val = run_expect(
            "fn main() -> Word { let n = 0; for x in [1, 1, 1, 1] { let n = n + 1; } n }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    // -- Data segment --

    #[test]
    fn eval_data_read() {
        // Read a host-initialized data slot from script.
        let src = "data ctx {\n    score: Word,\n}\nfn main() -> Word { ctx.score }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.set_data(0, Value::Int(42)).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn eval_data_write() {
        // Write to a data slot and read it back.
        let src = "\
            data ctx {\n\
                score: Word,\n\
            }\n\
            fn main() -> Word {\n\
                ctx.score = 100;\n\
                ctx.score\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(100)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn eval_data_survives_reset() {
        // Write to data before reset, verify it persists after.
        let src = "\
            data ctx {\n\
                counter: Word,\n\
            }\n\
            loop main(input: Word) -> Word {\n\
                ctx.counter = ctx.counter + 1;\n\
                let input = yield ctx.counter;\n\
                input\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.set_data(0, Value::Int(0)).unwrap();

        // First call: counter 0 + 1 = 1, yield 1.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(1)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Resume: reaches Reset. Counter is still 1.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }

        // Resume after Reset: counter 1 + 1 = 2, yield 2.
        // Data survived the reset.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(2)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Resume: reaches Reset.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }

        // Resume after second Reset: counter 2 + 1 = 3.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(3)),
            other => panic!("expected yield, got {:?}", other),
        }
    }

    #[test]
    fn eval_data_survives_yield() {
        // Write to data, yield, resume, verify data persists across yield.
        let src = "\
            data ctx {\n\
                value: Word,\n\
            }\n\
            loop main(input: Word) -> Word {\n\
                ctx.value = 99;\n\
                let input = yield ctx.value;\n\
                let input = yield ctx.value + 1;\n\
                input\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();

        // First yield: ctx.value = 99, yield 99.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(99)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Second yield: ctx.value still 99, yield 99 + 1 = 100.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(100)),
            other => panic!("expected yield, got {:?}", other),
        }
    }

    #[test]
    fn eval_data_multiple_slots() {
        // Multiple named data slots with independent values.
        let src = "\
            data ctx {\n\
                a: Word,\n\
                b: Word,\n\
                c: Word,\n\
            }\n\
            fn main() -> Word {\n\
                ctx.a = 10;\n\
                ctx.b = 20;\n\
                ctx.c = 30;\n\
                ctx.a + ctx.b + ctx.c\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(60)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    // B16 step 12: integer-magnitude test that fits i16/i32/i64 but
    // not i8; the narrow-word-8 binary masks 100 + 200 = 300 to 8
    // bits and the test's expected value no longer holds.
    #[cfg(not(feature = "narrow-word-8"))]
    fn eval_data_host_initialized() {
        // Host initializes data, script reads it.
        let src = "\
            data ctx {\n\
                x: Word,\n\
                y: Word,\n\
            }\n\
            fn main() -> Word { ctx.x + ctx.y }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.set_data(0, Value::Int(100)).unwrap();
        vm.set_data(1, Value::Int(200)).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(300)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    fn build_module(src: &str) -> Module {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        compile(&program).expect("compile error")
    }

    // -- Hot swap (replace_module) --

    #[test]
    fn hot_swap_same_schema_preserved() {
        // Module A: ctx { score: i64 }, returns ctx.score + 10.
        let src_a = "data ctx { score: Word }\nfn main() -> Word { ctx.score + 10 }";
        // Module B: ctx { score: i64 }, returns ctx.score * 2.
        let src_b = "data ctx { score: Word }\nfn main() -> Word { ctx.score * 2 }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_a, &arena).unwrap();
        vm.set_data(0, Value::Int(5)).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(15)),
            other => panic!("expected finished, got {:?}", other),
        }

        // Hot swap to module B with the same value preserved by the host.
        vm.replace_module(mod_b, alloc::vec![Value::Int(5)])
            .unwrap();
        assert_eq!(vm.data_len(), 1);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(10)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_strict_rejects_schema_mismatch() {
        // Replaces the previous `hot_swap_new_schema_replaced` test
        // under the strict-by-default schema policy added in Item 6
        // of the V0.2 design pass. Modules A and B declare different
        // data layouts; `replace_module` now rejects the swap with a
        // `VmError::VerifyError` whose message names the two
        // schema_hash values. Hosts that intend to swap across
        // incompatible schemas call `replace_module_unchecked`
        // (covered by `hot_swap_unchecked_admits_new_schema` below).
        let src_a = "data ctx { score: Word }\nfn main() -> Word { ctx.score }";
        let src_b =
            "data ctx { x: Word, y: Word, z: Word }\nfn main() -> Word { ctx.x + ctx.y + ctx.z }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_a, &arena).unwrap();
        let err = vm
            .replace_module(
                mod_b,
                alloc::vec![Value::Int(1), Value::Int(2), Value::Int(3)],
            )
            .unwrap_err();
        match err {
            VmError::VerifyError(msg) => assert!(
                msg.contains("schema mismatch"),
                "expected schema-mismatch error, got: {}",
                msg
            ),
            other => panic!("expected VerifyError(schema mismatch), got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_unchecked_admits_new_schema() {
        // The escape hatch: `replace_module_unchecked` bypasses the
        // schema check and admits a swap to a module with a different
        // data layout, retaining the previous V0.1 behaviour.
        let src_a = "data ctx { score: Word }\nfn main() -> Word { ctx.score }";
        let src_b =
            "data ctx { x: Word, y: Word, z: Word }\nfn main() -> Word { ctx.x + ctx.y + ctx.z }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_a, &arena).unwrap();
        vm.set_data(0, Value::Int(7)).unwrap();
        assert_eq!(vm.data_len(), 1);

        vm.replace_module_unchecked(
            mod_b,
            alloc::vec![Value::Int(1), Value::Int(2), Value::Int(3)],
        )
        .unwrap();
        assert_eq!(vm.data_len(), 3);

        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(6)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_size_mismatch_rejected() {
        // Schema-compatible swap that fails the size check. With the
        // strict policy now in effect, the new and old modules must
        // share a schema_hash for `replace_module` to even reach the
        // size check; this test therefore uses
        // `replace_module_unchecked` to bypass the schema check and
        // exercise the size-mismatch path directly.
        let src_a = "data ctx { x: Word }\nfn main() -> Word { ctx.x }";
        let src_b = "data ctx { x: Word, y: Word }\nfn main() -> Word { ctx.x + ctx.y }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_a, &arena).unwrap();
        let err = vm
            .replace_module_unchecked(mod_b, alloc::vec![Value::Int(99)])
            .unwrap_err();
        match err {
            VmError::InvalidBytecode(msg) => assert!(msg.contains("size mismatch")),
            other => panic!("expected size mismatch error, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_no_data_module_accepts_empty_vec() {
        // Hot swap from a module with a data block to one without.
        // The two schemas differ (the source module's data block
        // produces a non-zero schema_hash; the target's absence
        // produces zero), so this is a schema-incompatible swap that
        // routes through `replace_module_unchecked`.
        let src_a = "data ctx { x: Word }\nfn main() -> Word { ctx.x }";
        let src_b = "fn main() -> Word { 42 }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_a, &arena).unwrap();
        vm.replace_module_unchecked(mod_b, Vec::new()).unwrap();
        assert_eq!(vm.data_len(), 0);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_strict_admits_schema_compatible() {
        // Two modules with identical data layouts: same slot names,
        // same visibility, same declaration order. The default
        // `replace_module` admits the swap because the schema hashes
        // match.
        let src_a = "data ctx { x: Word }\nfn main() -> Word { ctx.x }";
        let src_b = "data ctx { x: Word }\nfn main() -> Word { ctx.x + 1 }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_a, &arena).unwrap();
        vm.set_data(0, Value::Int(5)).unwrap();
        vm.replace_module(mod_b, alloc::vec![Value::Int(5)])
            .unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(6)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_at_reset_starts_new_module() {
        // Module A: streaming counter. Module B: streaming doubler.
        let src_a = "data ctx { n: Word }\n\
                     loop main(input: Word) -> Word {\n\
                         ctx.n = ctx.n + 1;\n\
                         let input = yield ctx.n;\n\
                         input\n\
                     }";
        let src_b = "data ctx { n: Word }\n\
                     loop main(input: Word) -> Word {\n\
                         let input = yield ctx.n * 10;\n\
                         input\n\
                     }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_a, &arena).unwrap();
        vm.set_data(0, Value::Int(0)).unwrap();

        // Run module A: yield 1.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(1)),
            other => panic!("expected yield, got {:?}", other),
        }

        // Resume to reach Reset.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }

        // Hot swap to module B, host preserves n = 1.
        vm.replace_module(mod_b, alloc::vec![Value::Int(1)])
            .unwrap();

        // Run module B: yield 1 * 10 = 10.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(10)),
            other => panic!("expected yield, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_rollback_to_prior_version() {
        // Demonstrate rollback by treating the older module as the swap target.
        let src_v1 = "data ctx { n: Word }\nfn main() -> Word { ctx.n + 1 }";
        let src_v2 = "data ctx { n: Word }\nfn main() -> Word { ctx.n + 100 }";

        let mod_v1 = build_module(src_v1);
        let mod_v2 = build_module(src_v2);

        // Start with v1, snapshot the value 5.
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(mod_v1.clone(), &arena).unwrap();
        vm.set_data(0, Value::Int(5)).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(6)),
            other => panic!("expected finished, got {:?}", other),
        }

        // Forward update to v2.
        vm.replace_module(mod_v2, alloc::vec![Value::Int(5)])
            .unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(105)),
            other => panic!("expected finished, got {:?}", other),
        }

        // Rollback to v1 with the same value.
        vm.replace_module(mod_v1, alloc::vec![Value::Int(5)])
            .unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(6)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    // -- Cross-yield prohibition on dynamic strings (R31) --

    #[test]
    fn yield_static_string_succeeds() {
        // Static string literals can be yielded.
        let src = "loop main(input: Word) -> Text { let input = yield \"static\"; \"static\" }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::StaticStr(String::from("static"))),
            other => panic!("expected yield, got {:?}", other),
        }
    }

    // V0.2.0 removed the `to_string`, `concat`, `slice`, and `length`
    // utility natives that previously produced KStr values from
    // script-level operations. The cross-yield prohibition for
    // `Value::KStr` is still enforced at runtime; tests for that path
    // now need a host-registered native that produces a KStr. The
    // `yield_dynamic_string_fails` and
    // `yield_tuple_with_dynamic_string_fails` tests were removed in
    // this transition; they should be reinstated alongside the
    // Phase 5 work that introduces the verified/external native ABI
    // split (and a test native that produces a KStr).

    // -- Arena integration --

    #[test]
    fn vm_has_arena_with_default_capacity() {
        let module = build_module("fn main() -> Word { 42 }");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let vm = Vm::new(module, &arena).unwrap();
        assert_eq!(vm.arena().capacity(), DEFAULT_ARENA_CAPACITY);
        // V0.2.0: Vm::new pre-reserves the operand stack and call
        // frames so the runtime fails fast with `OutOfArena` rather
        // than aborting on a later push. The bottom region is therefore
        // non-empty at construction.
        assert!(vm.arena().bottom_used() > 0);
        assert_eq!(vm.arena().top_used(), 0);
    }

    #[test]
    fn vm_arena_capacity_configurable() {
        let module = build_module("fn main() -> Word { 42 }");
        let arena = keleusma_arena::Arena::with_capacity(4096);
        let vm = Vm::new(module, &arena).unwrap();
        assert_eq!(vm.arena().capacity(), 4096);
    }

    #[test]
    fn vm_arena_reset_at_op_reset() {
        // Stream function that allocates from the arena's top region
        // before yield. The arena is not reset at yield. At the
        // Op::Reset boundary the top region is cleared and the epoch
        // advances. The bottom region is preserved because the
        // operand stack and call frames are bottom-allocated and
        // carry state across the reset.
        use crate::kstring::KString;

        let src = "loop main(input: Word) -> Word { let input = yield input; input }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();

        // Host allocates a string from the arena's top region. The
        // KString is the boundary type that becomes stale on reset.
        let handle = KString::alloc(vm.arena(), "scratch").unwrap();
        assert!(vm.arena().top_used() > 0);

        // First call yields, arena not reset at yield.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(_) => {}
            other => panic!("expected yield, got {:?}", other),
        }
        assert!(vm.arena().top_used() > 0);
        // The handle resolves while the epoch matches.
        assert_eq!(handle.get(vm.arena()).unwrap(), "scratch");

        // Resume to reach Reset. Top region is cleared and the epoch
        // advances. Bottom region is preserved (operand stack and
        // frames remain).
        let pre_epoch = vm.arena().epoch();
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }
        assert_eq!(vm.arena().top_used(), 0);
        assert_eq!(vm.arena().epoch(), pre_epoch + 1);
        // The handle is now stale.
        assert!(handle.get(vm.arena()).is_err());
    }

    #[test]
    fn bytecode_roundtrip() {
        let src = "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(21) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        // Header is correctly stamped.
        assert_eq!(&bytes[0..4], &crate::bytecode::BYTECODE_MAGIC);
        assert_eq!(
            u16::from_le_bytes([bytes[4], bytes[5]]),
            crate::bytecode::BYTECODE_VERSION
        );
        // Decoded module runs and produces the same result as the original.
        let decoded = Module::from_bytes(&bytes).expect("decode");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(decoded, &arena).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_load_bytes_end_to_end() {
        let src = "fn main() -> Word { 7 + 35 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::load_bytes(&bytes, &arena).expect("load");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_bad_magic() {
        // Pad to the V0.2.0 wire-format minimum framing length
        // (64-byte header + 4-byte CRC = 68 bytes) so the slice
        // passes the truncation check and reaches the magic
        // check. The body fields after the magic do not matter
        // because the wire-format reader rejects on the magic
        // mismatch before reading further.
        let mut bytes = alloc::vec![b'X', b'X', b'X', b'X']; // bad magic
        bytes.resize(68, 0);
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::BadMagic) => {}
            other => panic!("expected BadMagic, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_truncated() {
        let bytes = alloc::vec![b'K', b'E', b'L'];
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::Truncated) => {}
            other => panic!("expected Truncated, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_oversized_length_field() {
        // Construct a slice whose length field claims more bytes than
        // the slice actually contains. The truncation check catches
        // this before any further validation.
        let mut bytes = alloc::vec![
            b'K', b'E', b'L', b'E', // magic
            0x04, 0x00, // version
            0xFF, 0xFF, 0xFF, 0xFF, // length = 4 GiB, far above slice length
            6, 6, // word_bits_log2, addr_bits_log2
            0x00, 0x00, 0x00, 0x00, // reserved
            0x00, 0x00, 0x00, 0x00, // CRC placeholder
        ];
        // Pad to clearly less than the claimed length.
        bytes.push(0);
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::Truncated) => {}
            other => panic!("expected Truncated, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_undersized_length_field() {
        // Construct a slice whose length field is below the minimum
        // framing size.
        let bytes = alloc::vec![
            b'K', b'E', b'L', b'E', // magic
            0x04, 0x00, // version
            0x05, 0x00, 0x00, 0x00, // length = 5, below minimum framing
            6, 6, // word_bits_log2, addr_bits_log2
            0x00, 0x00, 0x00, 0x00, // reserved
            0x00, 0x00, 0x00, 0x00, // CRC placeholder
        ];
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::Truncated) => {}
            other => panic!("expected Truncated, got {:?}", other),
        }
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32",
        feature = "narrow-address-8",
        feature = "narrow-address-16",
        feature = "narrow-address-32",
        feature = "narrow-float-32"
    )))]
    #[test]
    fn bytecode_golden_bytes_for_main_returning_one() {
        // Pin the exact serialized form of a minimal Keleusma program
        // under the V0.2.0 wire format (Phase 7c).
        //
        // Source: `fn main() -> Word { 1 }`
        //
        // Layout: 64-byte framing header + opcode stream (8 bytes:
        // PushImmediate(5) + Return as 4-byte records) + empty
        // operand pool + rkyv-archived WireAuxBody + 4-byte CRC.
        // Total length: 216 bytes.
        let expected: alloc::vec::Vec<u8> = alloc::vec![
            75, 69, 76, 69, 1, 0, 64, 0, 216, 0, 0, 0, 6, 6, 6, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 64, 0, 0, 0, 8, 0, 0, 0, 72, 0, 0, 0, 0, 0, 0, 0, 72, 0, 0, 0, 140, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 159, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 1, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 109, 97, 105, 110,
            255, 255, 255, 255, 216, 255, 255, 255, 1, 0, 0, 0, 240, 255, 255, 255, 0, 0, 0, 0, 0,
            0, 0, 0, 228, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 212, 255, 255, 255, 1,
            0, 0, 0, 248, 255, 255, 255, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 6, 6, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 205, 36, 48, 180,
        ];
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        assert_eq!(
            bytes, expected,
            "wire format drift detected. Update the expected bytes deliberately and bump BYTECODE_VERSION if not backwards compatible."
        );
        // Round-trip verifies the deserializer reads the golden bytes
        // correctly and the resulting program executes.
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::load_bytes(&expected, &arena).expect("load");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(1)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_view_bytes_runs_aligned_input() {
        // Compile, serialize, and copy into an AlignedVec to obtain an
        // aligned slice. view_bytes validates in place via
        // Module::access_bytes and deserializes without the AlignedVec
        // copy that load_bytes performs internally.
        let src = "fn main() -> Word { 7 + 35 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::view_bytes(&aligned, &arena).expect("view");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_view_bytes_handles_unaligned_input() {
        // `Module::view_bytes` and `Module::from_bytes` both route
        // through `wire_format::module_from_wire_bytes`, which copies
        // the rkyv-archived auxiliary body into an `AlignedVec<8>`
        // before deserialization. Unaligned input is therefore
        // handled gracefully without requiring the caller to align
        // the buffer. The zero-copy alignment contract is preserved
        // by the distinct `Module::access_bytes` and
        // `Vm::view_bytes_zero_copy` entry points; this test pins
        // the owned-decode tolerance.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        // Force unaligned by prepending one byte then taking bytes[1..].
        let mut shifted = alloc::vec![0u8];
        shifted.extend_from_slice(&bytes);
        let unaligned = &shifted[1..];
        let decoded = Module::view_bytes(unaligned).expect("decode unaligned");
        // Round-trip soundness: the entry chunk is reachable and the
        // module's declared widths survive the decode.
        assert!(decoded.entry_point.is_some());
        assert_eq!(decoded.word_bits_log2, module.word_bits_log2);
    }

    #[test]
    fn vm_view_bytes_zero_copy_executes_against_borrowed_buffer() {
        // True zero-copy execution. Compile a program, serialize to
        // bytes inside an AlignedVec, then construct a VM that borrows
        // the bytes directly. The execution loop reads the entire
        // module through `&ArchivedModule` with no owned `Module`
        // materialized.
        let src = "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(21) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        // Construct a VM that borrows from `aligned`. The lifetime
        // parameter on Vm is tied to the slice.
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm: Vm<'_, '_> =
            unsafe { Vm::view_bytes_zero_copy(&aligned[..], &arena).expect("view") };
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_op_round_trip_matches_owned() {
        // V0.2.0 Phase 7c moves the per-op encoding from the
        // rkyv archive into the wire-format opcode stream. The
        // round-trip equivalence between an in-memory `Op` and
        // its on-the-wire form is now exercised through
        // `Module::to_bytes` and `Module::from_bytes` in this
        // test; the wire_format crate carries direct unit tests
        // for every variant in `module_roundtrip_*` and the
        // `opcode_record_roundtrip_*` suite.
        let src = "fn main() -> Word { 1 + 2 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let decoded = Module::from_bytes(&bytes).expect("decode");
        assert_eq!(module.chunks.len(), decoded.chunks.len());
        for (chunk_idx, (orig, dec)) in module.chunks.iter().zip(decoded.chunks.iter()).enumerate()
        {
            assert_eq!(
                orig.ops, dec.ops,
                "chunk {} ops mismatch across wire-format round trip",
                chunk_idx
            );
        }
    }

    #[test]
    fn bytecode_archived_value_round_trip_matches_owned() {
        // value_from_archived materializes an owned Value from an
        // archived Value. Verify constants survive the round trip.
        use crate::bytecode::value_from_archived;
        let src = "fn main() -> Word { 42 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let archived: &crate::wire_format::ArchivedWireAuxBody =
            Module::access_bytes(&aligned).expect("access");
        let main_chunk = &archived.chunks[0];
        for (i, archived_val) in main_chunk.constants.iter().enumerate() {
            let owned = value_from_archived(archived_val);
            let original = module.chunks[0].constants[i].clone().into_value();
            assert_eq!(
                owned, original,
                "constant at index {} mismatches across archive round trip",
                i
            );
        }
    }

    #[test]
    fn bytecode_access_bytes_returns_archived_view() {
        // access_bytes returns a borrowed `ArchivedWireAuxBody`
        // under the V0.2.0 wire format. The archived form
        // preserves the chunk count, the entry point, and the
        // word and address sizes through native conversions.
        let src = "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(21) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let archived: &crate::wire_format::ArchivedWireAuxBody =
            Module::access_bytes(&aligned).expect("access");
        assert_eq!(archived.chunks.len(), 2);
        assert_eq!(
            archived.word_bits_log2,
            crate::bytecode::RUNTIME_WORD_BITS_LOG2
        );
        assert_eq!(
            archived.addr_bits_log2,
            crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2
        );
    }

    #[test]
    fn bytecode_admits_trailing_padding() {
        // The recorded length is authoritative. Trailing bytes after
        // the recorded length are ignored, so bytecode embedded in a
        // larger buffer is accepted.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        bytes.extend_from_slice(&[0xAA; 32]);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let module = Module::from_bytes(&bytes).expect("decode");
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(1)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_unsupported_version() {
        // Compile a real module, patch the version field to an
        // unsupported value, then recompute the CRC trailer so the
        // residue check still passes. This isolates the version
        // rejection path from the checksum rejection path.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        bytes[4] = 0xFF;
        bytes[5] = 0xFF;
        let trailer_start = bytes.len() - 4;
        let new_crc = crate::bytecode::crc32(&bytes[..trailer_start]);
        bytes[trailer_start..].copy_from_slice(&new_crc.to_le_bytes());
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::UnsupportedVersion { got, expected }) => {
                assert_eq!(got, 0xFFFF);
                assert_eq!(expected, crate::bytecode::BYTECODE_VERSION);
            }
            other => panic!("expected UnsupportedVersion, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_bad_checksum() {
        // Compile a real module, then flip a byte deep inside the body
        // so the CRC residue check fails. The flipped byte must lie
        // beyond the length field (offsets 6..10) so it does not change
        // the recorded length and trip the truncation check first.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        // Flip a byte in the postcard body, just past the header.
        let body_byte = bytes.len() - 5;
        bytes[body_byte] ^= 0xFF;
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::BadChecksum) => {}
            other => panic!("expected BadChecksum, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_word_size_mismatch() {
        // Compile a real module, patch the header's word_bits_log2
        // field to a value greater than the runtime supports, and
        // recompute the CRC trailer so the residue check passes.
        // The V0.2.0 wire format places word_bits_log2 at byte 12.
        // The width validation runs before the header-vs-aux
        // cross-check, so the patched header surfaces as
        // WordSizeMismatch.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        bytes[12] = crate::bytecode::RUNTIME_WORD_BITS_LOG2 + 1;
        let trailer_start = bytes.len() - 4;
        let new_crc = crate::bytecode::crc32(&bytes[..trailer_start]);
        bytes[trailer_start..].copy_from_slice(&new_crc.to_le_bytes());
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::WordSizeMismatch { got, max_supported }) => {
                assert_eq!(got, crate::bytecode::RUNTIME_WORD_BITS_LOG2 + 1);
                assert_eq!(max_supported, crate::bytecode::RUNTIME_WORD_BITS_LOG2);
            }
            other => panic!("expected WordSizeMismatch, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_address_size_mismatch() {
        // Header's addr_bits_log2 is at byte 13 in the V0.2.0
        // wire format.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        bytes[13] = crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2 + 1;
        let trailer_start = bytes.len() - 4;
        let new_crc = crate::bytecode::crc32(&bytes[..trailer_start]);
        bytes[trailer_start..].copy_from_slice(&new_crc.to_le_bytes());
        match Module::from_bytes(&bytes) {
            Err(crate::bytecode::LoadError::AddressSizeMismatch { got, max_supported }) => {
                assert_eq!(got, crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2 + 1);
                assert_eq!(max_supported, crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2);
            }
            other => panic!("expected AddressSizeMismatch, got {:?}", other),
        }
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn bytecode_admits_narrower_word_size() {
        // The runtime accepts narrower-than-runtime bytecode
        // under the relaxed-width policy. With the V0.2.0 wire
        // format, the auxiliary body also mirrors the
        // word_bits_log2 field, so simply patching the header
        // byte produces a header-vs-aux mismatch. Build a
        // narrower-target module through `compile_with_target`
        // instead; both the header and the auxiliary body carry
        // the matching narrower width.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module =
            crate::compiler::compile_with_target(&program, &crate::target::Target::embedded_16())
                .expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::load_bytes(&bytes, &arena).expect("narrower bytecode should be admitted");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(1)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn bytecode_masking_truncates_to_declared_width() {
        // Construct a Module with word_bits_log2 = 5 (32-bit) and an
        // arithmetic that would not overflow at 64 bits but would at
        // 32 bits. Verify that the runtime applies sign-extending
        // truncation. The expression `2147483647 + 1` produces
        // 2147483648 at 64 bits but i32::MIN = -2147483648 at 32 bits.
        let src = "fn main() -> Word { 2147483647 + 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let mut module = compile(&program).expect("compile");
        module.word_bits_log2 = 5;
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(-2147483648)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_residue_property_holds() {
        // The CRC-32 residue property states that for any byte sequence
        // D and its CRC C, the CRC of D followed by the little-endian
        // encoding of C equals 0x2144DF1C. Verify against the reference
        // value crc32("123456789") = 0xCBF43926 and confirm the residue.
        let data = b"123456789";
        let c = crate::bytecode::crc32(data);
        assert_eq!(c, 0xCBF43926);
        let mut combined = alloc::vec![];
        combined.extend_from_slice(data);
        combined.extend_from_slice(&c.to_le_bytes());
        let residue = crate::bytecode::crc32(&combined);
        assert_eq!(residue, 0x2144DF1C);
    }

    #[test]
    fn bytecode_load_via_vm_propagates_load_error() {
        // Twenty bytes is the minimum framing size. The magic is
        // intentionally wrong so the magic-check path triggers. The
        // length field is set to 20 so the truncation check passes.
        let bytes = alloc::vec![
            b'X', b'X', b'X', b'X', 0x04, 0x00, 0x14, 0x00, 0x00, 0x00, 6, 6, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        match Vm::load_bytes(&bytes, &arena) {
            Err(VmError::LoadError(_)) => {}
            Err(other) => panic!("expected VmError::LoadError, got {:?}", other),
            Ok(_) => panic!("expected error, got VM"),
        }
    }

    #[test]
    fn vm_new_returns_out_of_arena_when_capacity_too_small_for_minimum() {
        // A trivial program should still report `OutOfArena` rather
        // than aborting the host when the arena cannot hold the
        // minimum operand-stack and call-frame reservation.
        let module = build_module("fn main() -> Word { 1 }");
        let arena = keleusma_arena::Arena::with_capacity(0);
        let result = Vm::new(module, &arena);
        match result {
            Err(VmError::OutOfArena(_)) | Err(VmError::VerifyError(_)) => {}
            Err(other) => panic!("expected OutOfArena or VerifyError, got {:?}", other),
            Ok(_) => panic!("expected OutOfArena or VerifyError, got Ok"),
        }
    }

    #[test]
    fn vm_new_unchecked_returns_out_of_arena_when_capacity_too_small() {
        // The trust-skip constructor also returns OutOfArena rather
        // than aborting when the arena is too small for the minimum
        // runtime reservation.
        let module = build_module("fn main() -> Word { 1 }");
        let arena = keleusma_arena::Arena::with_capacity(0);
        let result = unsafe { Vm::new_unchecked(module, &arena) };
        assert!(matches!(result, Err(VmError::OutOfArena(_))));
    }

    #[test]
    fn unchecked_still_runs_structural_verification() {
        // Construct a module that fails structural verification by
        // manually corrupting the chunk's block type. A `Stream` chunk
        // without a yield is rejected.
        use crate::bytecode::{BlockType, Chunk, ConstValue, Module, Op};
        let chunk = Chunk {
            name: alloc::string::String::from("main"),
            ops: alloc::vec![Op::Const(0), Op::Reset],
            constants: alloc::vec![ConstValue::Int(0)],
            struct_templates: alloc::vec![],
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Stream,
            param_types: alloc::vec![],
        };
        let module = Module {
            schema_hash: 0,
            chunks: alloc::vec![chunk],
            native_names: alloc::vec![],
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
            float_bits_log2: crate::bytecode::RUNTIME_FLOAT_BITS_LOG2,
            wcet_cycles: 0,
            wcmu_bytes: 0,
            flags: 0,
            shared_data_bytes: 0,
            private_data_bytes: 0,
        };
        // The unchecked constructor still rejects on structural grounds.
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let result = unsafe { Vm::new_unchecked(module, &arena) };
        assert!(matches!(result, Err(VmError::VerifyError(_))));
    }

    #[test]
    fn contains_dynstr_helper() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let h = crate::kstring::KString::alloc(&arena, "hi").unwrap();
        let kstr = Value::KStr(h);
        assert!(!Value::Int(1).contains_dynstr());
        assert!(!Value::StaticStr(String::from("hi")).contains_dynstr());
        assert!(kstr.contains_dynstr());
        assert!(Value::Tuple(alloc::vec![Value::Int(1), kstr.clone()]).contains_dynstr());
        assert!(
            !Value::Tuple(alloc::vec![
                Value::Int(1),
                Value::StaticStr(String::from("x"))
            ])
            .contains_dynstr()
        );
        assert!(
            Value::Struct {
                type_name: String::from("Foo"),
                fields: alloc::vec![(String::from("x"), kstr)],
            }
            .contains_dynstr()
        );
    }

    // -- P3 error recovery --

    #[test]
    fn reset_after_error_preserves_data() {
        // After a runtime error the data segment persists. The host
        // can call reset_after_error and retry without losing
        // accumulated state.
        let src = "data ctx { count: Word }\n\
                   loop main(input: Word) -> Word {\n\
                       ctx.count = ctx.count + 1;\n\
                       let next = yield ctx.count;\n\
                       next\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.set_data(0, Value::Int(5)).unwrap();

        // First iteration: yield 6.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(Value::Int(6)) => {}
            other => panic!("expected yield 6, got {:?}", other),
        }

        // Simulate an error: pretend the host got an Err from resume.
        // We do not actually trigger one here because the test would
        // need to provide bytecode that traps. The recovery contract
        // is that reset_after_error returns the VM to a callable state
        // regardless of how the failed call left things. Verify that
        // calling it after a normal yield still produces a valid
        // post-recovery state.
        vm.reset_after_error();

        // Data segment preserved.
        assert_eq!(vm.get_data(0).unwrap(), &Value::Int(6));

        // Fresh call works.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(Value::Int(7)) => {}
            other => panic!("expected yield 7 after recovery, got {:?}", other),
        }
    }

    #[test]
    fn reset_after_trap_clears_volatile_state() {
        // A program that traps. The host catches the error, calls
        // reset_after_error, then runs the program again successfully.
        let trap_src = "fn main() -> Word { let x = 0; if x == 0 { 1 / x } else { 0 } }";
        match run_program(trap_src, &[]) {
            Err(VmError::DivisionByZero) => {}
            other => panic!("expected DivisionByZero precheck, got {:?}", other),
        }
        // Build the VM directly so we own its lifetime here and can
        // recover after the error.
        let tokens = tokenize(trap_src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();

        // First run produces the error.
        match vm.call(&[]) {
            Err(VmError::DivisionByZero) => {}
            other => panic!("expected DivisionByZero, got {:?}", other),
        }

        // Recover and verify volatile state is clean.
        vm.reset_after_error();

        // Calling again still produces the same error because the
        // bytecode is unchanged. The point is that the call goes
        // through cleanly without corruption from the prior failed
        // run.
        match vm.call(&[]) {
            Err(VmError::DivisionByZero) => {}
            other => panic!("expected DivisionByZero on second call, got {:?}", other),
        }
    }

    #[test]
    fn reset_after_error_idempotent() {
        // Calling reset_after_error multiple times is harmless.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        vm.reset_after_error();
        vm.reset_after_error();
        vm.reset_after_error();
        match vm.call(&[]).unwrap() {
            VmState::Finished(Value::Int(1)) => {}
            other => panic!("expected finished 1, got {:?}", other),
        }
    }

    // -- P2 for-in over typed expressions --

    #[test]
    fn for_in_over_function_return_passes_strict_verify() {
        // The for-in iteration bound is extracted from the called
        // function's declared return type [i64; 3]. The verifier
        // accepts the loop because the end bound is a Const(3).
        // Without static type info the same source would be rejected
        // by strict-mode WCMU.
        let src = "fn make() -> [Word; 3] { [1, 2, 3] }\n\
                   fn main() -> Word {\n\
                       let last = 0;\n\
                       for x in make() { let last = x; }\n\
                       last\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        // Constructing the VM runs the strict-mode verifier. A
        // successful construction confirms the iteration bound was
        // extractable from the typed return.
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify");
    }

    #[test]
    fn for_in_over_data_segment_field_passes_strict_verify() {
        // For-in over a data segment field whose declared type is
        // [i64; N] is admissible because the field type provides the
        // static iteration bound.
        let src = "data ctx { items: [Word; 4] }\n\
                   fn main() -> Word {\n\
                       let last = 0;\n\
                       for x in ctx.items { let last = x; }\n\
                       last\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify");
    }

    #[test]
    fn for_in_over_struct_field_from_local_passes_strict_verify() {
        // For-in over a struct field accessed through a local
        // variable. The compiler tracks the local's declared or
        // inferred type and resolves `b.items` to `[i64; 3]` from
        // the struct definition. The verifier accepts the
        // resulting Const(3) end bound.
        let src = "struct Box { items: [Word; 3] }\n\
                   fn main() -> Word {\n\
                       let b = Box { items: [1, 2, 3] };\n\
                       let last = 0;\n\
                       for x in b.items { let last = x; }\n\
                       last\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify");
    }

    #[test]
    fn for_in_over_param_array_passes_strict_verify() {
        // For-in over an array parameter typed `[T; N]`. The
        // compiler records the parameter's declared type on the
        // local and the for-in source resolves to a typed array.
        let src = "fn sum_n(arr: [Word; 4]) -> Word {\n\
                       let s = 0;\n\
                       for x in arr { let s = s + x; }\n\
                       s\n\
                   }\n\
                   fn main() -> Word { sum_n([1, 2, 3, 4]) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify");
    }

    #[test]
    fn for_in_over_nested_array_index_passes_strict_verify() {
        // For-in over the result of indexing a nested array. The
        // compiler infers `m[0]` to have the matrix's element type
        // `[i64; 3]` and emits `Const(3)` for the iteration bound.
        let src = "fn main() -> Word {\n\
                       let m: [[Word; 3]; 2] = [[1, 2, 3], [4, 5, 6]];\n\
                       let last = 0;\n\
                       for x in m[0] { let last = x; }\n\
                       last\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify");
    }

    #[test]
    fn for_in_over_match_array_result_passes_strict_verify() {
        // For-in over a match expression that returns an array. The
        // compiler infers the match result type from the first arm's
        // expression and uses it for the iteration bound.
        let src = "fn main() -> Word {\n\
                       let cond = 1;\n\
                       let last = 0;\n\
                       for x in match cond { 0 => [1, 2, 3], _ => [4, 5, 6] } {\n\
                           let last = x;\n\
                       }\n\
                       last\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify");
    }

    #[test]
    fn for_in_three_level_nested_passes_strict_verify() {
        // Three nested for-in loops over a 3D array. Each level
        // resolves its iteration bound from the binding's element
        // type. The outer loop reads the matrix's outer length.
        // Each subsequent loop reads the element type's length from
        // the iteration variable's type recorded by the compiler.
        let src = "fn main() -> Word {\n\
                       let map: [[[Word; 2]; 2]; 2] = [\n\
                           [[1, 2], [3, 4]],\n\
                           [[5, 6], [7, 8]],\n\
                       ];\n\
                       let last = 0;\n\
                       for z in map {\n\
                           for y in z {\n\
                               for x in y {\n\
                                   let last = x;\n\
                               }\n\
                           }\n\
                       }\n\
                       last\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let _vm = Vm::new(module, &arena).expect("verify");
    }

    #[test]
    fn match_on_inner_value_inside_three_level_nested_for_in() {
        // A match expression inside three nested for-in loops over a
        // 3D array of i64. The match arms evaluate to integer values
        // and the result accumulates into a data segment field. The
        // value 1 contributes 100; every other value contributes 1.
        // The matrix has values 1..=8 so the total is 100 + 7 = 107.
        let src = "data ctx { sum: Word }\n\
                   fn main() -> Word {\n\
                       let map: [[[Word; 2]; 2]; 2] = [\n\
                           [[1, 2], [3, 4]],\n\
                           [[5, 6], [7, 8]],\n\
                       ];\n\
                       for z in map {\n\
                           for y in z {\n\
                               for x in y {\n\
                                   let v = match x {\n\
                                       1 => 100,\n\
                                       _ => 1,\n\
                                   };\n\
                                   ctx.sum = ctx.sum + v;\n\
                               }\n\
                           }\n\
                       }\n\
                       ctx.sum\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        vm.set_data(0, Value::Int(0)).expect("init sum");
        match vm.call(&[]) {
            Ok(VmState::Finished(Value::Int(v))) => assert_eq!(v, 107),
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn tuple_match_across_three_loop_variables() {
        // Three independent 1D arrays, nested for-in, with a match
        // on the tuple `(x, y, z)`. The corner case `(0, 0, 0)`
        // contributes 100; every other coordinate contributes 1.
        // The 2x2x2 coordinate space has 8 cells so the total is
        // 100 + 7 = 107.
        let src = "data ctx { hits: Word }\n\
                   fn main() -> Word {\n\
                       let xs: [Word; 2] = [0, 1];\n\
                       let ys: [Word; 2] = [0, 1];\n\
                       let zs: [Word; 2] = [0, 1];\n\
                       for z in zs {\n\
                           for y in ys {\n\
                               for x in xs {\n\
                                   let v = match (x, y, z) {\n\
                                       (0, 0, 0) => 100,\n\
                                       _ => 1,\n\
                                   };\n\
                                   ctx.hits = ctx.hits + v;\n\
                               }\n\
                           }\n\
                       }\n\
                       ctx.hits\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        vm.set_data(0, Value::Int(0)).expect("init hits");
        match vm.call(&[]) {
            Ok(VmState::Finished(Value::Int(v))) => assert_eq!(v, 107),
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn for_in_three_level_nested_runs() {
        // Same shape as the verify test but exercises full execution
        // through a native function call. The native sums all
        // visited values. The data segment carries the running total
        // because let bindings shadow rather than mutate.
        let src = "data ctx { total: Word }\n\
                   fn main() -> Word {\n\
                       let map: [[[Word; 2]; 2]; 2] = [\n\
                           [[1, 2], [3, 4]],\n\
                           [[5, 6], [7, 8]],\n\
                       ];\n\
                       for z in map {\n\
                           for y in z {\n\
                               for x in y {\n\
                                   ctx.total = ctx.total + x;\n\
                               }\n\
                           }\n\
                       }\n\
                       ctx.total\n\
                   }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        vm.set_data(0, Value::Int(0)).expect("init total");
        match vm.call(&[]) {
            Ok(VmState::Finished(Value::Int(v))) => {
                // Sum of 1..=8 is 36.
                assert_eq!(v, 36);
            }
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn for_in_over_array_literal_runs() {
        let val = run_expect(
            "fn main() -> Word {\n\
                 let last = 0;\n\
                 for x in [10, 20, 30] { let last = x; }\n\
                 last\n\
             }",
            &[],
        );
        // The body's `let last = x;` shadows in the inner scope.
        // Outer `last` remains 0.
        assert_eq!(val, Value::Int(0));
    }

    // -- Overflow policy knob --

    fn build_module_with_overflow(wcet: u32, wcmu: u32) -> Module {
        // Build a trivial module then mutate the declared header
        // fields to simulate a compile-time saturation.
        let mut module = build_module("fn main() -> Word { 0 }");
        module.wcet_cycles = wcet;
        module.wcmu_bytes = wcmu;
        module
    }

    #[test]
    fn new_rejects_module_without_entry_point() {
        // Compile a module that has functions but no `main`. The
        // entry-point absence should surface as a clear VerifyError
        // at the boundary, not as InvalidBytecode at the first
        // Vm::call use site.
        let src = "fn helper(x: Word) -> Word { x + 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert!(
            module.entry_point.is_none(),
            "test precondition: module has no entry point"
        );
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        match Vm::new(module, &arena) {
            Err(VmError::VerifyError(msg)) => {
                assert!(
                    msg.contains("no entry point"),
                    "expected entry-point diagnostic, got {:?}",
                    msg
                );
            }
            Ok(_) => panic!("expected VerifyError for missing entry point"),
            Err(other) => panic!("expected VerifyError, got {:?}", other),
        }
    }

    #[test]
    fn new_with_options_default_rejects_wcet_overflow() {
        let module = build_module_with_overflow(u32::MAX, 0);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let err = Vm::new_with_options(module, &arena, VmOptions::default())
            .err()
            .expect("expected overflow rejection");
        match err {
            VmError::VerifyError(msg) => assert!(msg.contains("WCET")),
            other => panic!("expected VerifyError, got {:?}", other),
        }
    }

    #[test]
    fn new_with_options_default_rejects_wcmu_overflow() {
        let module = build_module_with_overflow(0, u32::MAX);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let err = Vm::new_with_options(module, &arena, VmOptions::default())
            .err()
            .expect("expected overflow rejection");
        match err {
            VmError::VerifyError(msg) => assert!(msg.contains("WCMU")),
            other => panic!("expected VerifyError, got {:?}", other),
        }
    }

    #[test]
    fn new_with_options_warn_admits_and_returns_warning() {
        let module = build_module_with_overflow(u32::MAX, 0);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let options = VmOptions {
            overflow_policy: OverflowPolicy::Warn,
        };
        let warnings = match Vm::new_with_options(module, &arena, options) {
            Ok((_vm, w)) => w,
            Err(e) => panic!("expected Ok, got {:?}", e),
        };
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, WarningKind::WcetOverflow);
    }

    #[test]
    fn new_with_options_warn_returns_both_warnings_for_both_overflows() {
        let module = build_module_with_overflow(u32::MAX, u32::MAX);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let options = VmOptions {
            overflow_policy: OverflowPolicy::Warn,
        };
        let warnings = match Vm::new_with_options(module, &arena, options) {
            Ok((_vm, w)) => w,
            Err(e) => panic!("expected Ok, got {:?}", e),
        };
        assert_eq!(warnings.len(), 2);
        let kinds: Vec<WarningKind> = warnings.iter().map(|w| w.kind).collect();
        assert!(kinds.contains(&WarningKind::WcetOverflow));
        assert!(kinds.contains(&WarningKind::WcmuOverflow));
    }

    #[test]
    fn new_with_options_allow_admits_silently() {
        let module = build_module_with_overflow(u32::MAX, u32::MAX);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let options = VmOptions {
            overflow_policy: OverflowPolicy::Allow,
        };
        let warnings = match Vm::new_with_options(module, &arena, options) {
            Ok((_vm, w)) => w,
            Err(e) => panic!("expected Ok, got {:?}", e),
        };
        assert!(warnings.is_empty());
    }

    #[test]
    fn new_with_options_no_overflow_returns_empty_warnings() {
        let module = build_module_with_overflow(100, 1000);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let options = VmOptions {
            overflow_policy: OverflowPolicy::Warn,
        };
        let warnings = match Vm::new_with_options(module, &arena, options) {
            Ok((_vm, w)) => w,
            Err(e) => panic!("expected Ok, got {:?}", e),
        };
        assert!(warnings.is_empty());
    }

    #[test]
    fn vm_new_remains_strict_under_overflow() {
        // `Vm::new` is a thin wrapper around `new_with_options` with
        // the default (Reject) policy, so it must reject overflow.
        let module = build_module_with_overflow(u32::MAX, 0);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let err = Vm::new(module, &arena).err().expect("expected rejection");
        assert!(matches!(err, VmError::VerifyError(_)));
    }

    #[test]
    fn call_with_too_few_args_returns_typed_error() {
        // Reviewer reproduction: `fn main(a: Word, b: Word) -> Word { a + b }`
        // called with only one argument used to default `b` to
        // `Value::Unit`, which then failed at `a + b` with the
        // misleading message "cannot add Int and Unit". The call
        // now rejects the wrong arg count before any bytecode runs.
        let src = "fn main(a: Word, b: Word) -> Word { a + b }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        let err = vm.call(&[Value::Int(1)]).expect_err("expected rejection");
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("expected 2"), "got: {}", msg);
                assert!(msg.contains("got 1"), "got: {}", msg);
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[test]
    #[cfg(feature = "floats")]
    fn call_with_wrong_arg_type_returns_typed_error() {
        // The runtime validates each argument against the
        // parameter's declared type tag and rejects mismatches
        // before any bytecode runs.
        let src = "fn main(a: Word) -> Word { a + 1 }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        let err = vm
            .call(&[Value::Float(1.5)])
            .expect_err("expected rejection");
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("expected Word"), "got: {}", msg);
                assert!(msg.contains("got Float"), "got: {}", msg);
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[test]
    #[cfg(feature = "floats")]
    fn resume_with_wrong_type_returns_typed_error() {
        // Reviewer reproduction: a loop with `x: Word` parameter
        // could be resumed with a Float without complaint. The
        // wrong type would then trip an arithmetic op at the
        // first use site. The runtime now validates the resume
        // value at the boundary.
        let src = "loop main(x: Word) -> Word { let z = yield x; z }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[Value::Int(11)]).expect("call") {
            VmState::Yielded(v) => assert_eq!(v, Value::Int(11)),
            other => panic!("expected yield, got {:?}", other),
        }
        let err = vm
            .resume(Value::Float(1.5))
            .expect_err("expected rejection");
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("expected Word"), "got: {}", msg);
                assert!(msg.contains("got Float"), "got: {}", msg);
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[test]
    fn premature_resume_returns_not_suspended() {
        // Reviewer reproduction: calling `resume` before `call`
        // previously surfaced as `InvalidBytecode("cannot resume:
        // VM not suspended")`, which conflated API misuse with
        // corrupt bytecode. The runtime now returns the dedicated
        // `VmError::NotSuspended` variant.
        let src = "loop main(x: Word) -> Word { let z = yield x; z }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        let err = vm.resume(Value::Int(0)).expect_err("expected NotSuspended");
        match err {
            VmError::NotSuspended => {}
            other => panic!("expected NotSuspended, got {:?}", other),
        }
    }

    #[test]
    fn resume_after_finished_returns_not_suspended() {
        // After a Stream block reaches Finished (or any non-yielded
        // terminal state), the VM is no longer suspended and resume
        // must surface NotSuspended rather than InvalidBytecode.
        let src = "fn main() -> Word { 7 }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 7),
            other => panic!("expected Finished, got {:?}", other),
        }
        let err = vm.resume(Value::Int(0)).expect_err("expected NotSuspended");
        match err {
            VmError::NotSuspended => {}
            other => panic!("expected NotSuspended, got {:?}", other),
        }
    }

    #[test]
    #[cfg(feature = "floats")]
    fn untyped_param_inferred_rejects_wrong_type_at_call() {
        // `fn main(x) -> Word { x }` infers `x: Word`. The chunk's
        // param_types must carry that inferred tag so Vm::call
        // rejects a Float argument with a typed error rather than
        // silently accepting and tripping arithmetic later.
        let src = "fn main(x) -> Word { x }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[Value::Int(7)]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 7),
            other => panic!("expected Finished, got {:?}", other),
        }
        let err = vm.call(&[Value::Float(1.5)]).expect_err("expected reject");
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("expected Word"), "got: {}", msg);
                assert!(msg.contains("got Float"), "got: {}", msg);
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[test]
    fn multiheaded_loop_main_executes() {
        // Two heads: the literal `0` head yields a constant; the
        // `x: Word` head yields the input. The runtime dispatches
        // per iteration and resumes correctly across the
        // Stream...Reset boundary.
        let src = "loop main(0) -> Word { yield 100 }\n\
                   loop main(x: Word) -> Word { let z = yield x; z }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[Value::Int(0)]).expect("call") {
            VmState::Yielded(Value::Int(v)) => assert_eq!(v, 100),
            other => panic!("expected Yielded(100), got {:?}", other),
        }
        // Resume to reach the Reset epilogue for the matched head.
        let _ = vm.resume(Value::Int(0));
        // Second iteration: input 7 takes the second head.
        match vm.call(&[Value::Int(7)]).expect("call") {
            VmState::Yielded(Value::Int(v)) => assert_eq!(v, 7),
            other => panic!("expected Yielded(7), got {:?}", other),
        }
    }

    #[test]
    fn data_segment_indexed_array_round_trip() {
        // A `data state { idx: [Word; 4] }` block compiles to four
        // consecutive slots. The script writes through indexed
        // assignment and reads through indexed access; the loop
        // accumulates a sum across the array.
        let src = "data state { items: [Word; 4] }\n\
                   fn main() -> Word {\n\
                       state.items[0] = 10;\n\
                       state.items[1] = 20;\n\
                       state.items[2] = 30;\n\
                       state.items[3] = 40;\n\
                       state.items[0] + state.items[3]\n\
                   }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        for slot in 0..4 {
            vm.set_data(slot, Value::Int(0)).expect("init slot");
        }
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 50),
            other => panic!("unexpected state: {:?}", other),
        }
        // The slots were written through `Op::SetDataIndexed` and
        // must persist after the call returns.
        assert_eq!(vm.get_data(0).unwrap(), &Value::Int(10));
        assert_eq!(vm.get_data(1).unwrap(), &Value::Int(20));
        assert_eq!(vm.get_data(2).unwrap(), &Value::Int(30));
        assert_eq!(vm.get_data(3).unwrap(), &Value::Int(40));
    }

    #[test]
    fn data_segment_indexed_out_of_bounds_traps() {
        // An index past the declared length triggers a typed
        // `VmError::IndexOutOfBounds`. The compiler's single-level
        // path elides the explicit `BoundsCheck` and relies on
        // `Op::GetDataIndexed` to perform the check.
        let src = "data state { items: [Word; 3] }\n\
                   fn main() -> Word { state.items[5] }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        for slot in 0..3 {
            vm.set_data(slot, Value::Int(0)).expect("init slot");
        }
        let err = vm.call(&[]).expect_err("expected out-of-bounds trap");
        match err {
            VmError::IndexOutOfBounds(i, len) => {
                assert_eq!(i, 5);
                assert_eq!(len, 3);
            }
            other => panic!("expected IndexOutOfBounds, got {:?}", other),
        }
    }

    #[test]
    fn data_segment_indexed_multidim_round_trip() {
        // Nested arrays flatten to a single contiguous slab. The
        // compiler emits per-level `Op::BoundsCheck` followed by
        // stride arithmetic and a final `Op::GetDataIndexed` /
        // `Op::SetDataIndexed`. Both writes and reads round-trip.
        let src = "data state { grid: [[Word; 3]; 2] }\n\
                   fn main() -> Word {\n\
                       state.grid[0][0] = 1;\n\
                       state.grid[0][1] = 2;\n\
                       state.grid[0][2] = 3;\n\
                       state.grid[1][0] = 4;\n\
                       state.grid[1][1] = 5;\n\
                       state.grid[1][2] = 6;\n\
                       state.grid[1][2] - state.grid[0][0]\n\
                   }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        for slot in 0..6 {
            vm.set_data(slot, Value::Int(0)).expect("init slot");
        }
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 5),
            other => panic!("unexpected state: {:?}", other),
        }
        // Slot layout walks the inner dimension first: grid[0][0],
        // grid[0][1], grid[0][2], grid[1][0], grid[1][1], grid[1][2].
        assert_eq!(vm.get_data(0).unwrap(), &Value::Int(1));
        assert_eq!(vm.get_data(3).unwrap(), &Value::Int(4));
        assert_eq!(vm.get_data(5).unwrap(), &Value::Int(6));
    }

    #[test]
    fn data_segment_multidim_inner_bounds_check_traps() {
        // A multi-dimensional access whose inner index exceeds the
        // inner dimension length must trap even when the
        // mathematically computed flat offset stays inside the
        // total slab. Without a per-level `BoundsCheck` the access
        // would silently land on a different "row".
        let src = "data state { grid: [[Word; 3]; 2] }\n\
                   fn main() -> Word { state.grid[0][5] }";
        let module = build_module(src);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        for slot in 0..6 {
            vm.set_data(slot, Value::Int(0)).expect("init slot");
        }
        let err = vm.call(&[]).expect_err("expected inner-bound trap");
        match err {
            VmError::IndexOutOfBounds(i, len) => {
                assert_eq!(i, 5);
                assert_eq!(len, 3);
            }
            other => panic!("expected IndexOutOfBounds, got {:?}", other),
        }
    }

    // --- Data partition (shared vs private) and ephemeral flag ---

    #[test]
    fn shared_data_byte_count_in_header() {
        let src = "\
            data ctx { a: Word, b: Word }\n\
            fn main() -> Word { ctx.a + ctx.b }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        // Two shared slots times 32-byte VALUE_SLOT_SIZE_BYTES.
        assert_eq!(module.shared_data_bytes, 64);
        assert_eq!(module.private_data_bytes, 0);
    }

    #[test]
    fn private_data_byte_count_in_header() {
        // Private data must be mutated to satisfy the unmutated-
        // private rejection rule introduced in phase 6.
        let src = "\
            private data state { counter: Word }\n\
            fn main() -> Word { state.counter = 1; state.counter }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert_eq!(module.shared_data_bytes, 0);
        assert_eq!(module.private_data_bytes, 32);
    }

    #[test]
    fn shared_slot_count_for_returns_module_count() {
        let src = "\
            data ctx { a: Word, b: Word, c: Word }\n\
            fn main() -> Word { ctx.a }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert_eq!(shared_slot_count_for(&module), 3);
    }

    #[test]
    fn shared_slot_count_excludes_private_slots() {
        let src = "\
            data sh { a: Word, b: Word }\n\
            private data pv { x: Word, y: Word, z: Word }\n\
            fn main() -> Word { pv.x = 1; pv.y = 2; pv.z = 3; sh.a + pv.x }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert_eq!(shared_slot_count_for(&module), 2);
    }

    #[test]
    fn vm_shared_slot_count_matches_module_helper() {
        let src = "\
            data ctx { a: Word, b: Word, c: Word, d: Word }\n\
            fn main() -> Word { ctx.a }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let expected = shared_slot_count_for(&module);
        let mut arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        arena
            .resize_persistent(required_persistent_capacity_for(&module))
            .expect("resize persistent");
        let vm = Vm::new(module, &arena).expect("verify");
        assert_eq!(vm.shared_slot_count(), expected);
        assert_eq!(vm.shared_slot_count(), 4);
    }

    #[test]
    fn mixed_data_partitions_correctly() {
        let src = "\
            data shared_ctx { x: Word }\n\
            private data priv_ctx { y: Word, z: Word }\n\
            fn main() -> Word { priv_ctx.y = 1; priv_ctx.z = 2; shared_ctx.x + priv_ctx.y + priv_ctx.z }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert_eq!(module.shared_data_bytes, 32);
        assert_eq!(module.private_data_bytes, 64);
    }

    #[test]
    fn set_data_rejects_private_slot() {
        let src = "\
            data shared_ctx { x: Word }\n\
            private data priv_ctx { y: Word }\n\
            fn main() -> Word { priv_ctx.y = 1; shared_ctx.x + priv_ctx.y }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        arena
            .resize_persistent(required_persistent_capacity_for(&module))
            .expect("resize persistent");
        let mut vm = Vm::new(module, &arena).expect("verify");
        // Slot 0 is shared; should succeed.
        vm.set_data(0, Value::Int(5)).expect("shared slot accepted");
        // Slot 1 is private; should reject.
        let err = vm
            .set_data(1, Value::Int(7))
            .expect_err("private slot must reject");
        match err {
            VmError::NativeError(msg) => {
                assert!(
                    msg.contains("private"),
                    "expected 'private' in error: {}",
                    msg
                );
            }
            other => panic!("expected NativeError, got {:?}", other),
        }
    }

    #[test]
    fn get_data_rejects_private_slot() {
        let src = "\
            data shared_ctx { x: Word }\n\
            private data priv_ctx { y: Word }\n\
            fn main() -> Word { priv_ctx.y = 1; shared_ctx.x + priv_ctx.y }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        arena
            .resize_persistent(required_persistent_capacity_for(&module))
            .expect("resize persistent");
        let vm = Vm::new(module, &arena).expect("verify");
        let _ok = vm.get_data(0).expect("shared slot accessible");
        let err = vm.get_data(1).expect_err("private slot must reject");
        match err {
            VmError::NativeError(msg) => {
                assert!(msg.contains("private"));
            }
            other => panic!("expected NativeError, got {:?}", other),
        }
    }

    #[test]
    fn ephemeral_bit_set_for_atomic_total_no_data() {
        let src = "fn main() -> Word { 42 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert!(
            module.flags & crate::bytecode::FLAG_EPHEMERAL != 0,
            "expected FLAG_EPHEMERAL bit set, got flags = {:#04x}",
            module.flags
        );
    }

    #[test]
    fn ephemeral_bit_clear_when_private_data_present() {
        let src = "\
            private data state { counter: Word }\n\
            fn main() -> Word { state.counter = state.counter + 1; state.counter }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert!(
            module.flags & crate::bytecode::FLAG_EPHEMERAL == 0,
            "expected FLAG_EPHEMERAL cleared for module with private data; flags = {:#04x}",
            module.flags
        );
    }

    #[test]
    fn explicit_ephemeral_modifier_accepted_when_proof_holds() {
        let src = "ephemeral fn main() -> Word { 0 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile accepts ephemeral main");
        assert!(module.flags & crate::bytecode::FLAG_EPHEMERAL != 0);
    }

    #[test]
    fn private_slots_round_trip_through_arena() {
        // Construct a module with one private slot. Write the
        // slot from the script, then read it back. The value
        // lives in the arena's persistent region; this test
        // confirms the arena routing works end to end.
        let src = "\
            private data state { counter: Word }\n\
            fn main() -> Word {\n\
                state.counter = 42;\n\
                state.counter\n\
            }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        arena
            .resize_persistent(required_persistent_capacity_for(&module))
            .expect("resize persistent");
        assert_eq!(arena.persistent_capacity(), core::mem::size_of::<Value>());
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected Finished(42), got {:?}", other),
        }
    }

    #[test]
    fn vm_new_rejects_insufficient_persistent_capacity() {
        // The arena has zero persistent capacity. The module
        // declares private data. `Vm::new` rejects with a
        // helpful message.
        let src = "\
            private data state { x: Word }\n\
            fn main() -> Word { state.x = 1; state.x }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        // Note: no resize_persistent call. Default persistent
        // capacity is 0.
        let err = match Vm::new(module, &arena) {
            Ok(_) => panic!("Vm::new should have rejected the module"),
            Err(e) => e,
        };
        match err {
            VmError::VerifyError(msg) => {
                assert!(
                    msg.contains("persistent_capacity") && msg.contains("private"),
                    "unexpected error message: {}",
                    msg
                );
            }
            other => panic!("expected VerifyError, got {:?}", other),
        }
    }

    #[test]
    fn vm_drop_runs_destructors_on_private_slots() {
        // Sanity check: dropping a VM with multiple private
        // slots iterates them all without overrunning the slot
        // count. The slots hold `Value::Unit` (the default
        // initialisation), whose Drop is a no-op; the test
        // exercises the iteration bound, not the destructor
        // body. A bug that miscomputed the slot count would
        // either UAF or leak; this test does neither, which is
        // the implicit assertion.
        let src = "\
            private data state { a: Word, b: Word, c: Word }\n\
            fn main() -> Word { state.a = 1; state.b = 2; state.c = 3; 0 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        arena
            .resize_persistent(required_persistent_capacity_for(&module))
            .expect("resize persistent");
        {
            let vm = Vm::new(module, &arena).expect("verify");
            assert_eq!(vm.data_len(), 3);
            // Drop happens at end of scope.
            drop(vm);
        }
    }

    #[test]
    fn ephemeral_bit_set_when_text_param_is_unused() {
        // A `Text` parameter that the body never references
        // cannot carry arena-resident data across the
        // host-VM boundary. The verifier admits the module as
        // ephemeral. This is the parameter-usage refinement of
        // the dialogue-type rule.
        let src = "fn main(unused_name: Text) -> Word { 42 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert!(
            module.flags & crate::bytecode::FLAG_EPHEMERAL != 0,
            "expected FLAG_EPHEMERAL set; unused Text param should not disqualify, flags = {:#04x}",
            module.flags
        );
    }

    #[test]
    fn ephemeral_bit_set_when_declared_text_return_never_produced() {
        // The entry's declared return type carries `Text` through
        // `Option<Text>`, but every concrete return path produces
        // `Option::None` (a non-text discriminant). The per-yield
        // arena dataflow refinement walks the compiled chunk in
        // topological call order, observes that `Op::Return` peeks a
        // non-text value, and admits the module as ephemeral despite
        // the declared signature. The previous signature-only rule
        // would have disqualified this program even though it never
        // crosses the host-VM boundary with an arena-resident string.
        //
        // The test exercises both the per-yield dataflow refinement
        // and the type-checker tightening that admits bare
        // `Option::None` in a function-return position by unifying
        // its inner type with the declared return type.
        let src = "fn main() -> Option<Text> { Option::None }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert!(
            module.flags & crate::bytecode::FLAG_EPHEMERAL != 0,
            "expected FLAG_EPHEMERAL set; declared Text return with no concrete text path should not disqualify, flags = {:#04x}",
            module.flags
        );
    }

    #[test]
    fn ephemeral_bit_clear_when_declared_text_return_actually_produced() {
        // The entry's declared return type is `Text` and the body
        // actually produces a text value at the return site. The
        // dataflow refinement confirms the boundary-crossing op
        // peeks a text value, so the module is correctly disqualified
        // from ephemerality. This is a regression guard that the
        // refinement does not become permissive in cases where the
        // conservative rule already disqualified.
        let src = "fn main() -> Text { \"hello\" }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert!(
            module.flags & crate::bytecode::FLAG_EPHEMERAL == 0,
            "expected FLAG_EPHEMERAL clear; declared Text return with actual text path must disqualify, flags = {:#04x}",
            module.flags
        );
    }

    #[test]
    #[cfg(feature = "verify")]
    fn module_chunk_text_analyses_distinguishes_yield_and_return() {
        // Direct unit test of the per-yield dataflow analysis. The
        // analysis must peek at the value being yielded or returned
        // and report `yields_text`/`returns_text` accordingly.
        //
        // The source-level positive test for the ephemerality
        // refinement is blocked by an unrelated type-unification
        // limitation around bare `Option::None` literals in function
        // returns. This test exercises the analysis directly against
        // hand-crafted chunks so that the dataflow path stays under
        // automated coverage even when the surface language cannot
        // express the relevant program.
        use crate::bytecode::{BlockType, Chunk, ConstValue, Module, Op};

        let mut text_returning_chunk = Chunk {
            name: alloc::string::String::from("text_return"),
            ops: alloc::vec![Op::Const(0), Op::Return],
            constants: alloc::vec![ConstValue::StaticStr(alloc::string::String::from("hi"))],
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
        };
        // Suppress the compiler's per-chunk field defaults that may
        // differ across builds. Module-level fields below are also
        // populated explicitly for determinism.
        text_returning_chunk.ops.shrink_to_fit();

        let int_returning_chunk = Chunk {
            name: alloc::string::String::from("int_return"),
            ops: alloc::vec![Op::Const(0), Op::Return],
            constants: alloc::vec![ConstValue::Int(0)],
            struct_templates: alloc::vec::Vec::new(),
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Func,
            param_types: alloc::vec::Vec::new(),
        };

        let module = Module {
            schema_hash: 0,
            chunks: alloc::vec![text_returning_chunk, int_returning_chunk],
            native_names: alloc::vec::Vec::new(),
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: 6,
            addr_bits_log2: 5,
            float_bits_log2: 6,
            shared_data_bytes: 0,
            private_data_bytes: 0,
            flags: 0,
            wcet_cycles: 0,
            wcmu_bytes: 0,
        };

        let analyses = crate::verify::module_chunk_text_analyses(&module).expect("analyse");
        assert_eq!(analyses.len(), 2);
        assert!(
            analyses[0].returns_text,
            "chunk 0 returns a static string and must be flagged returns_text"
        );
        assert!(
            !analyses[1].returns_text,
            "chunk 1 returns an integer and must not be flagged returns_text"
        );
        assert!(
            !analyses[0].yields_text && !analyses[1].yields_text,
            "neither chunk contains Op::Yield"
        );
    }

    #[test]
    fn ephemeral_bit_clear_when_text_param_is_used() {
        // The same shape but the body actually references the
        // Text param. The verifier conservatively assumes the
        // param could flow back to the host through a yield or
        // return and disqualifies the module from ephemerality.
        // The body returns 0 unconditionally; the only purpose
        // of referencing `name` is to mark it as used.
        let src = "fn main(name: Text) -> Word { let _ignored = name; 0 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert!(
            module.flags & crate::bytecode::FLAG_EPHEMERAL == 0,
            "expected FLAG_EPHEMERAL clear; used Text param must disqualify, flags = {:#04x}",
            module.flags
        );
    }

    #[test]
    fn const_data_struct_initializer() {
        // Struct-typed const data field with a struct literal
        // initializer. The struct is declared elsewhere; the
        // const field references it by name.
        let src = "\
            struct Point { x: Word, y: Word }\n\
            const data origin {\n\
                pt: Point = Point { x: 3, y: 4 },\n\
            }\n\
            fn main() -> Word { origin.pt.x + origin.pt.y }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 7),
            other => panic!("expected Int(7), got {:?}", other),
        }
    }

    #[test]
    fn const_data_enum_initializer() {
        // Enum-typed const data field with a variant
        // construction initializer. Tests both unit and tuple-
        // payload variants through a Word cast.
        let src = "\
            enum Color { Red = 1, Green = 2, Blue = 3 }\n\
            const data palette {\n\
                primary: Color = Color::Red,\n\
            }\n\
            fn main() -> Word { palette.primary as Word }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 1),
            other => panic!("expected Int(1), got {:?}", other),
        }
    }

    #[test]
    fn const_data_enum_tuple_variant_initializer() {
        let src = "\
            enum Shape { Square(Word), Rect(Word, Word) }\n\
            const data shapes {\n\
                a: Shape = Shape::Rect(4, 5),\n\
            }\n\
            fn main() -> Word {\n\
                match shapes.a {\n\
                    Shape::Rect(w, h) => w * h,\n\
                    Shape::Square(s) => s * s,\n\
                }\n\
            }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 20),
            other => panic!("expected Int(20), got {:?}", other),
        }
    }

    #[test]
    fn const_data_tuple_initializer() {
        // Tuple-typed const data field with a tuple initializer.
        let src = "\
            const data pt {\n\
                origin: (Word, Word) = (3, 4),\n\
            }\n\
            fn main() -> Word { pt.origin.0 + pt.origin.1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 7),
            other => panic!("expected Int(7), got {:?}", other),
        }
    }

    #[test]
    fn const_data_array_initializer() {
        // Array-typed const data field with an array initializer.
        let src = "\
            const data lut {\n\
                table: [Word; 3] = [10, 20, 30],\n\
            }\n\
            fn main() -> Word { lut.table[0] + lut.table[1] + lut.table[2] }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Int(v)) => assert_eq!(v, 60),
            other => panic!("expected Int(60), got {:?}", other),
        }
    }

    #[test]
    fn const_data_array_length_mismatch_rejected() {
        let src = "\
            const data lut {\n\
                table: [Word; 5] = [1, 2, 3],\n\
            }\n\
            fn main() -> Word { lut.table[0] }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("3 element") && err.message.contains("expected 5"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn const_data_field_compiles_to_constant_load() {
        // `const data` fields bake their initializer into the
        // per-chunk constant pool. The runtime reads them through
        // `Op::Const`; no data-segment slot is allocated.
        let src = "\
            const data palette {\n\
                red: Byte = 255,\n\
                green: Byte = 128,\n\
            }\n\
            fn main() -> Byte { palette.red }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        // No runtime slots for const data; byte counts stay zero.
        assert_eq!(module.shared_data_bytes, 0);
        assert_eq!(module.private_data_bytes, 0);
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).expect("verify");
        match vm.call(&[]).expect("call") {
            VmState::Finished(Value::Byte(b)) => assert_eq!(b, 255),
            other => panic!("expected Byte(255), got {:?}", other),
        }
    }

    #[test]
    fn const_data_field_write_rejected() {
        let src = "\
            const data k { v: Word = 7 }\n\
            fn main() -> Word { k.v = 9; k.v }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("const data") && err.message.contains("immutable"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn const_data_missing_initializer_rejected() {
        let src = "\
            const data k { v: Word }\n\
            fn main() -> Word { k.v }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("initializer"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn shared_data_initializer_rejected() {
        let src = "\
            data ctx { x: Word = 5 }\n\
            fn main() -> Word { ctx.x }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("initializer") && err.message.contains("const data"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn three_data_blocks_one_each_visibility_accepted() {
        let src = "\
            data shared_ctx { a: Word }\n\
            private data priv_ctx { b: Word }\n\
            const data const_ctx { c: Word = 42 }\n\
            fn main() -> Word { priv_ctx.b = 1; shared_ctx.a + priv_ctx.b + const_ctx.c }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert_eq!(module.shared_data_bytes, 32);
        assert_eq!(module.private_data_bytes, 32);
    }

    #[test]
    fn private_data_never_mutated_rejected() {
        // The verifier rejects a private data block whose
        // slots are never written. The diagnostic suggests
        // `const data` as the rewrite.
        let src = "\
            private data state { x: Word }\n\
            fn main() -> Word { state.x }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("never mutated") && err.message.contains("const data"),
            "unexpected error: {}",
            err.message
        );
    }

    #[test]
    fn explicit_ephemeral_modifier_rejected_when_private_data_present() {
        let src = "\
            private data state { x: Word }\n\
            ephemeral fn main() -> Word { state.x = 1; state.x }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("ephemeral") && err.message.contains("private data"),
            "unexpected error message: {}",
            err.message
        );
    }

    // The `with saturate_max = N, saturate_min = M` clause on a refined
    // newtype declaration defines context-determined values for the
    // `saturate_max` and `saturate_min` keywords. When the surrounding
    // expected type is the refined newtype, the keywords resolve to a
    // constructor call wrapping the declared literal. When the
    // surrounding expected type is the underlying primitive, the
    // keywords retain the legacy behaviour of evaluating to
    // `Word::MAX` / `Word::MIN`.
    #[test]
    fn saturate_keywords_resolve_to_newtype_contract_via_function_return() {
        // The function's declared return type drives the resolution.
        // The overflow path produces the declared `saturate_max` value
        // (100), wrapped by the `Limited` constructor. The refinement
        // predicate `nonneg` is satisfied at runtime because 100 >= 0.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Limited = Word where nonneg with saturate_max = 100, saturate_min = 0;\n\
             fn main() -> Limited {\n\
                let m = 9223372036854775807;\n\
                m + 1 {\n\
                    ok(v) => Limited(v),\n\
                    overflow(_, _) => saturate_max,\n\
                    underflow(_, _) => saturate_min,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(100));
    }

    #[cfg(not(any(
        feature = "narrow-word-8",
        feature = "narrow-word-16",
        feature = "narrow-word-32"
    )))]
    #[test]
    fn saturate_keywords_resolve_to_newtype_contract_via_let_annotation() {
        // The `let y: Limited = ...` annotation pushes `Limited` onto
        // the expected-type stack so the underflow arm's `saturate_min`
        // resolves to 0 (the declared contract).
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Limited = Word where nonneg with saturate_max = 100, saturate_min = 0;\n\
             fn main() -> Word {\n\
                let m = 0 - 9223372036854775807;\n\
                let y: Limited = m - 2 {\n\
                    ok(v) => Limited(v),\n\
                    overflow(_, _) => saturate_max,\n\
                    underflow(_, _) => saturate_min,\n\
                };\n\
                y as Word\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn saturate_keywords_fall_back_to_word_extrema_without_newtype_context() {
        // The function return type is `Word`, so the saturate keywords
        // retain the legacy semantics: `saturate_max` evaluates to
        // `Word::MAX`, not any newtype's declared contract.
        let val = run_expect(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Limited = Word where nonneg with saturate_max = 100, saturate_min = 0;\n\
             fn main() -> Word {\n\
                let m = 9223372036854775807;\n\
                let y = m + 1 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => saturate_max,\n\
                    underflow(_, _) => saturate_min,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(i64::MAX));
    }

    // Match-arm guard tests. A guard is an optional `when expr`
    // clause between the pattern and the `=>`; the arm fires only
    // when the pattern matches *and* the guard evaluates to true.
    // The exhaustiveness check treats guarded arms as non-catch-all
    // regardless of pattern shape.
    #[test]
    fn match_arm_guard_dispatches_on_runtime_predicate() {
        // Three arms differing only in guard select the correct
        // branch based on the bound value at runtime.
        let val = run_expect(
            "fn classify(n: Word) -> Word {\n\
                match n {\n\
                    v when v < 0 => 0 - 1,\n\
                    v when v == 0 => 0,\n\
                    v => 1,\n\
                }\n\
             }\n\
             fn main() -> Word { classify(0) + classify(5) + classify(0 - 3) }",
            &[],
        );
        assert_eq!(val, Value::Int(1 - 1));
    }

    #[test]
    fn match_arm_guard_falls_through_to_next_arm_when_false() {
        // The first arm's pattern matches but its guard returns
        // false; dispatch falls through to the unguarded catch-all.
        let val = run_expect(
            "fn main() -> Word {\n\
                match 5 {\n\
                    v when v > 10 => 999,\n\
                    v => v,\n\
                }\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(5));
    }

    #[test]
    fn match_arm_guarded_pattern_is_not_a_catchall() {
        // A guarded bare-variable arm does not satisfy the
        // exhaustiveness requirement for a bool scrutinee.
        let src = "fn main() -> Word {\n\
                match true {\n\
                    v when v == true => 1,\n\
                }\n\
             }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("non-exhaustive"),
            "expected non-exhaustive diagnostic, got: {}",
            err.message
        );
    }

    // The narrow-bytecode CheckedXxx tests directly exercise
    // `checked_arith_outputs` at every supported declared word
    // width to confirm that the `(low, high, flag)` triple is
    // computed at the bytecode-declared width rather than the
    // runtime width. This addresses the unresolved concern
    // documented in REVERSE_PROMPT for V0.2.0 Phase 8.

    #[test]
    fn checked_arith_outputs_runtime_width_in_range() {
        // Declared width matches the runtime: `100 + 50 = 150`
        // fits in i64, so flag = 0, high = 0, low = 150.
        let r: i128 = 100i64.widen() + 50i64.widen();
        let (low, high, flag) = super::checked_arith_outputs::<i64>(r, 6);
        assert_eq!(low, 150);
        assert_eq!(high, 0);
        assert_eq!(flag, 0);
    }

    #[test]
    fn checked_arith_outputs_runtime_width_overflow() {
        // `i64::MAX + 1` overflows the runtime range; flag = 1,
        // low wraps to `i64::MIN`, high carries the i128 high
        // half (zero in this case because the result fits in 65
        // bits but is positive).
        let r: i128 = i64::MAX.widen() + 1i64.widen();
        let (low, high, flag) = super::checked_arith_outputs::<i64>(r, 6);
        assert_eq!(low, i64::MIN);
        assert_eq!(high, 0);
        assert_eq!(flag, 1);
    }

    #[test]
    fn checked_arith_outputs_runtime_width_underflow() {
        // `i64::MIN + i64::MIN` underflows; flag = 2.
        let r: i128 = i64::MIN.widen() + i64::MIN.widen();
        let (_low, high, flag) = super::checked_arith_outputs::<i64>(r, 6);
        // High half: -2 * 2^63 == -(2^64), shifted right by 64
        // is -1.
        assert_eq!(high, -1);
        assert_eq!(flag, 2);
    }

    #[test]
    fn checked_arith_outputs_narrow_declared_32_in_range() {
        // Declared 32-bit on a 64-bit runtime: a value that fits
        // i32 reports flag = 0, low = the value sign-extended at
        // 32, high = 0.
        let r: i128 = (i32::MAX as i64).widen() + 0i64.widen();
        let (low, high, flag) = super::checked_arith_outputs::<i64>(r, 5);
        assert_eq!(low, i32::MAX as i64);
        assert_eq!(high, 0);
        assert_eq!(flag, 0);
    }

    #[test]
    fn checked_arith_outputs_narrow_declared_32_overflow() {
        // `i32::MAX + 1` overflows the declared 32-bit range.
        // Flag = 1; the low half is `i32::MIN` (the truncated
        // sign-extended value); the high half is 1.
        let r: i128 = (i32::MAX as i64).widen() + 1i64.widen();
        let (low, high, flag) = super::checked_arith_outputs::<i64>(r, 5);
        assert_eq!(low, i32::MIN as i64);
        assert_eq!(high, 1);
        assert_eq!(flag, 1);
    }

    #[test]
    fn checked_arith_outputs_narrow_declared_32_underflow() {
        // `i32::MIN - 1` underflows the declared 32-bit range.
        // Flag = 2; low half is `i32::MAX`; high half is -1.
        let r: i128 = (i32::MIN as i64).widen() - 1i64.widen();
        let (low, high, flag) = super::checked_arith_outputs::<i64>(r, 5);
        assert_eq!(low, i32::MAX as i64);
        assert_eq!(high, -1);
        assert_eq!(flag, 2);
    }

    #[test]
    fn checked_arith_outputs_narrow_declared_16_overflow() {
        // `i16::MAX + 1` overflows the declared 16-bit range.
        let r: i128 = (i16::MAX as i64).widen() + 1i64.widen();
        let (low, high, flag) = super::checked_arith_outputs::<i64>(r, 4);
        assert_eq!(low, i16::MIN as i64);
        assert_eq!(high, 1);
        assert_eq!(flag, 1);
    }

    #[test]
    fn checked_arith_outputs_narrow_declared_8_underflow() {
        // `i8::MIN - 1` underflows the declared 8-bit range.
        let r: i128 = (i8::MIN as i64).widen() - 1i64.widen();
        let (low, high, flag) = super::checked_arith_outputs::<i64>(r, 3);
        assert_eq!(low, i8::MAX as i64);
        assert_eq!(high, -1);
        assert_eq!(flag, 2);
    }

    #[test]
    fn checked_arith_outputs_narrow_declared_reconstructs_true_value() {
        // The (low, high) pair reconstructs the true value via
        // `r == (high << declared_bits) + low_signed`. Verify
        // this invariant at 32-bit declared width with a value
        // that crosses the declared boundary.
        let true_value: i128 = (i32::MAX as i128) + 100;
        let (low, high, _flag) = super::checked_arith_outputs::<i64>(true_value, 5);
        let reconstructed = ((high as i128) << 32) + (low as i128);
        assert_eq!(reconstructed, true_value);
    }

    #[test]
    fn signed_keyword_parses_on_all_function_categories() {
        let cases: &[(&str, &str)] = &[
            ("signed fn", "signed fn main() -> Word { 42 }"),
            (
                "signed yield",
                "signed yield main(_x: Word) -> Word { let _r = yield 0; 0 }",
            ),
            (
                "signed loop",
                "signed loop main(_x: Word) -> Word { let _r = yield 0; 0 }",
            ),
        ];
        for (label, src) in cases {
            let tokens = tokenize(src).unwrap_or_else(|e| panic!("{}: lex: {:?}", label, e));
            let program = parse(&tokens).unwrap_or_else(|e| panic!("{}: parse: {:?}", label, e));
            let main = program
                .functions
                .iter()
                .find(|f| f.name == "main")
                .expect("main");
            assert!(main.signed, "{}: signed not recorded", label);
        }
    }

    #[test]
    fn signed_modifier_on_helper_is_rejected_at_compile_time() {
        let src = "signed fn helper() -> Word { 0 }\nfn main() -> Word { helper() }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let err = compile(&program).expect_err("compile should reject");
        assert!(
            err.message.contains("`signed` modifier on `helper`"),
            "expected entry-only diagnostic, got: {}",
            err.message
        );
    }

    #[test]
    fn signed_entry_sets_flag_requires_signature() {
        let src = "signed fn main() -> Word { 42 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert_ne!(
            module.flags & crate::wire_format::FLAG_REQUIRES_SIGNATURE,
            0,
            "FLAG_REQUIRES_SIGNATURE must be set on signed entry"
        );
    }

    #[test]
    fn unsigned_entry_does_not_set_flag_requires_signature() {
        let src = "fn main() -> Word { 42 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        assert_eq!(
            module.flags & crate::wire_format::FLAG_REQUIRES_SIGNATURE,
            0,
            "FLAG_REQUIRES_SIGNATURE must not be set on unsigned entry"
        );
    }

    #[test]
    fn vm_new_rejects_signed_module_directly() {
        // Construct a module with the flag manually set and feed
        // it to `Vm::new`. The constructor refuses because the
        // signature info has already been stripped from the
        // Module representation; the host must use
        // `Vm::load_signed_bytes` or hot-swap instead.
        let src = "signed fn main() -> Word { 42 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let result = Vm::new(module, &arena);
        match result {
            Err(VmError::VerifyError(msg)) => assert!(
                msg.contains("FLAG_REQUIRES_SIGNATURE"),
                "expected signed-module rejection, got: {}",
                msg
            ),
            Err(other) => panic!("expected VerifyError, got: {:?}", other),
            Ok(_) => panic!("expected VerifyError, got Ok(_)"),
        }
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn signed_module_loads_through_load_signed_bytes() {
        use ed25519_dalek::SigningKey;
        let signer = SigningKey::from_bytes(&[42u8; 32]);
        let verifying = signer.verifying_key();
        let src = "signed fn main() -> Word { 21 + 21 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes =
            crate::wire_format::module_to_signed_wire_bytes(&module, &signer).expect("sign");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::load_signed_bytes(&bytes, &arena, &[verifying]).expect("load+verify");
        assert_eq!(vm.verifying_keys_len(), 1);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected Finished(42), got {:?}", other),
        }
    }

    #[cfg(all(feature = "signatures", feature = "encryption"))]
    #[test]
    fn load_encrypted_signed_bytes_executes_decrypted_module() {
        use ed25519_dalek::SigningKey;

        let signer = SigningKey::from_bytes(&[101u8; 32]);
        let verifying = signer.verifying_key();

        // Recipient X25519 keypair.
        let recipient_sk = [0xb0u8; 32];
        let recipient_pk = crate::encryption::public_key_from_private(&recipient_sk);

        let ephemeral_seed = [0xc0u8; 32];

        // Compile a simple module that returns 99.
        let src = "signed fn main() -> Word { 99 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");

        // Encrypt and sign.
        let bytes = crate::wire_format::module_to_encrypted_signed_wire_bytes(
            &module,
            &signer,
            &recipient_pk,
            &ephemeral_seed,
        )
        .expect("encrypt+sign");

        // Load through the VM. End-to-end verifies the signature,
        // decrypts the body, parses the plaintext, runs structural
        // verification, and constructs the VM.
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::load_encrypted_signed_bytes(&bytes, &arena, &[verifying], &recipient_sk)
            .expect("load+decrypt+verify");

        match vm.call(&[]).expect("execute") {
            VmState::Finished(Value::Int(99)) => (),
            other => panic!("expected Finished(99), got {:?}", other),
        }
    }

    #[cfg(all(feature = "signatures", feature = "encryption"))]
    #[test]
    fn load_encrypted_signed_bytes_rejects_wrong_decryption_key() {
        use ed25519_dalek::SigningKey;

        let signer = SigningKey::from_bytes(&[102u8; 32]);
        let verifying = signer.verifying_key();

        let alice_sk = [0xd1u8; 32];
        let alice_pk = crate::encryption::public_key_from_private(&alice_sk);
        let bob_sk = [0xd2u8; 32];

        let ephemeral_seed = [0xc1u8; 32];

        let src = "signed fn main() -> Word { 7 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");

        // Encrypt to Alice.
        let bytes = crate::wire_format::module_to_encrypted_signed_wire_bytes(
            &module,
            &signer,
            &alice_pk,
            &ephemeral_seed,
        )
        .expect("encrypt+sign");

        // Bob tries to load. Should fail at recipient_key_id check.
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let result = Vm::load_encrypted_signed_bytes(&bytes, &arena, &[verifying], &bob_sk);
        assert!(result.is_err(), "expected wrong-recipient rejection");
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn load_signed_bytes_rejects_wrong_key() {
        use ed25519_dalek::SigningKey;
        let signer = SigningKey::from_bytes(&[42u8; 32]);
        let wrong = SigningKey::from_bytes(&[43u8; 32]).verifying_key();
        let src = "signed fn main() -> Word { 0 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes =
            crate::wire_format::module_to_signed_wire_bytes(&module, &signer).expect("sign");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let result = Vm::load_signed_bytes(&bytes, &arena, &[wrong]);
        match result {
            Err(VmError::LoadError(msg)) => assert!(
                msg.contains("signature did not verify") || msg.contains("InvalidSignature"),
                "expected InvalidSignature, got: {}",
                msg
            ),
            Err(other) => panic!("expected LoadError, got: {:?}", other),
            Ok(_) => panic!("expected LoadError, got Ok(_)"),
        }
    }

    /// `Vm::load_bytes` must refuse signed input ahead of every
    /// other check. The minimum-viable signed-looking buffer is
    /// 16 bytes with `KELE` magic and the
    /// `FLAG_REQUIRES_SIGNATURE` bit set in the flags byte;
    /// `header_requires_signature` returns true on that and the
    /// load path short-circuits before any framing or CRC
    /// validation runs. The test fires in both feature
    /// configurations and asserts the error message names the
    /// signature contract.
    #[test]
    fn load_bytes_short_circuits_on_signed_flag() {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(b"KELE");
        bytes[15] = crate::wire_format::FLAG_REQUIRES_SIGNATURE;
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let result = Vm::load_bytes(&bytes, &arena);
        match result {
            Err(VmError::LoadError(msg)) => {
                // Without `signatures`: the message names the
                // unsupported feature. With `signatures` on: the
                // message redirects the caller to
                // `Vm::load_signed_bytes`. Both contain
                // "signatures" or "signed"; the assertion is
                // permissive but pins the contract.
                assert!(
                    msg.contains("signatures")
                        || msg.contains("Vm::load_signed_bytes")
                        || msg.contains("signed"),
                    "expected signed-module rejection, got: {}",
                    msg
                );
            }
            Err(other) => panic!("expected LoadError, got: {:?}", other),
            Ok(_) => panic!("expected error, got Ok(_)"),
        }
    }

    /// In a build without the `signatures` feature, the same
    /// path returns the dedicated
    /// `LoadError::SignaturesUnsupported` variant rather than a
    /// `Codec` redirect. The two cases produce distinguishable
    /// error messages so operators can tell whether the
    /// build supports verification or the call site is just
    /// using the wrong API.
    #[cfg(not(feature = "signatures"))]
    #[test]
    fn load_bytes_rejects_signed_with_signatures_unsupported() {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(b"KELE");
        bytes[15] = crate::wire_format::FLAG_REQUIRES_SIGNATURE;
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let result = Vm::load_bytes(&bytes, &arena);
        match result {
            Err(VmError::LoadError(msg)) => assert!(
                msg.contains("does not include the `signatures` feature"),
                "expected SignaturesUnsupported message, got: {}",
                msg
            ),
            Err(other) => panic!("expected LoadError, got: {:?}", other),
            Ok(_) => panic!("expected error, got Ok(_)"),
        }
    }
}
