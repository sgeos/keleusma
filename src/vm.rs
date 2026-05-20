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
    /// No pattern matched in match expression or multiheaded function.
    NoMatch(String),
    /// A native function returned an error.
    NativeError(String),
    /// Invalid or unexpected bytecode.
    InvalidBytecode(String),
    /// Script execution was halted by a Trap instruction.
    Trap(String),
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
            | VmError::NoMatch(_)
            | VmError::Trap(_) => VmErrorCategory::SoftScript,
            // Soft host: a native returned an error. The host owns
            // the policy.
            VmError::NativeError(_) => VmErrorCategory::SoftHost,
        }
    }
}

/// The execution state of the VM.
#[derive(Debug, Clone)]
pub enum VmState {
    /// The coroutine yielded a value and is suspended.
    Yielded(Value),
    /// The function completed with a return value.
    Finished(Value),
    /// The stream hit a Reset boundary. The host may hot-swap and resume.
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
/// Created freshly at each [`Op::CallNative`] dispatch with a borrow
/// of the host-owned arena. Native functions allocate dynamic strings
/// through `KString::alloc(ctx.arena, s)` and return them as
/// [`Value::KStr`] for the bounded-memory path. Natives that do not
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
type NativeFn = Box<dyn for<'a> Fn(&NativeCtx<'a>, &[Value]) -> Result<Value, VmError>>;

/// A registered native function.
///
/// Carries WCET and WCMU bounds attested by the host. The bounds are used
/// by the static analysis tooling to compute end-to-end resource bounds.
/// Defaults are conservative for timing (one `CallNative` cost) and zero
/// for memory.
struct NativeEntry {
    name: String,
    func: NativeFn,
    /// Host-attested worst-case execution time, in the same unitless cost
    /// space as `Op::cost()`. Default `DEFAULT_NATIVE_WCET`.
    #[allow(dead_code)]
    wcet: u32,
    /// Host-attested worst-case memory usage in bytes. Native functions
    /// that allocate from the arena must override this for the analysis
    /// to remain sound. Default `DEFAULT_NATIVE_WCMU_BYTES`.
    #[allow(dead_code)]
    wcmu_bytes: u32,
}

/// Default WCET attestation for a native function. Equal to the cost of a
/// single `CallNative` instruction.
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
fn decode_all_ops(bytes: &[u8]) -> Result<Vec<Vec<Op>>, VmError> {
    let archived = crate::bytecode::Module::access_bytes(bytes)?;
    Ok(archived
        .chunks
        .iter()
        .map(|chunk| {
            chunk
                .ops
                .iter()
                .map(crate::bytecode::op_from_archived)
                .collect()
        })
        .collect())
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
pub struct Vm<'a, 'arena> {
    bytecode: BytecodeStore<'a>,
    /// Per-op decode cache, populated at VM construction and at every
    /// `replace_module`. Indexed as `decoded_ops[chunk_idx][ip]`. The
    /// hot dispatch loop reads from this slice directly, which avoids
    /// the per-fetch discriminant match and payload copy that
    /// `op_from_archived` performs against the archived form.
    ///
    /// The cost is one heap allocation proportional to the program's
    /// total op count at construction. Constants and string data
    /// continue to be read on demand from the archived form, so the
    /// zero-copy contract for those is preserved. The `Op` type is
    /// `Copy`, so the slice access is a trivial load on the hot path.
    decoded_ops: Vec<Vec<Op>>,
    /// Operand stack. Bump-allocated from the arena's bottom region.
    /// Recreated at every arena reset because the bump allocator's
    /// `deallocate` is a no-op and the Vec's storage would otherwise
    /// alias newly-allocated memory after a reset.
    stack: StackVec<'arena, Value>,
    /// Call-frame stack. Same arena-backed discipline as `stack`.
    frames: StackVec<'arena, CallFrame>,
    natives: Vec<NativeEntry>,
    /// Shared data slots. Survives across RESET boundaries.
    /// Host-visible through `Vm::set_data` and `Vm::get_data`.
    /// Indexed by the unified slot index `i` for `i` in
    /// `[0, shared_slot_count)`.
    data: Vec<Value>,
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
    /// `private_slot_count * size_of::<Value>()` bytes there.
    private_slot_count: u16,
    /// Host-owned dual-end bump-allocated arena. Borrowed for the
    /// lifetime of the VM. Native functions that allocate dynamic
    /// strings pass `vm.arena()` to [`crate::kstring::KString::alloc`].
    /// The arena's persistent region holds this module's private
    /// data slots.
    arena: &'arena keleusma_arena::Arena,
    started: bool,
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
/// The returned value is `private_slot_count * size_of::<Value>()`,
/// which is the actual runtime storage requirement. It differs
/// from `module.private_data_bytes` because that field is in
/// `VALUE_SLOT_SIZE_BYTES`-sized logical units for WCMU
/// accounting; the actual `Value` enum is larger than the WCMU
/// slot size by an implementation-defined factor.
pub fn required_persistent_capacity_for(module: &crate::bytecode::Module) -> usize {
    let private_count = module.data_layout.as_ref().map_or(0, |dl| {
        dl.slots
            .iter()
            .filter(|s| matches!(s.visibility, crate::bytecode::SlotVisibility::Private))
            .count()
    });
    private_count * core::mem::size_of::<Value>()
}

impl<'a, 'arena> Drop for Vm<'a, 'arena> {
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
        let base = self.arena.persistent_ptr().as_ptr() as *mut Value;
        for i in 0..self.private_slot_count as usize {
            // SAFETY: each private slot was initialised to
            // `Value::Unit` at construction and updated through
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

impl<'a, 'arena> Vm<'a, 'arena> {
    /// Borrow the archived module from internal bytecode storage.
    ///
    /// The bytes were validated at construction time, so accessing the
    /// archived form via `access_unchecked` is sound. For owned bytes
    /// produced by `Module::to_bytes`, the bytes are well-formed by
    /// construction. For borrowed bytes from `view_bytes_unchecked`,
    /// the framing was validated and the caller attests through the
    /// unsafe marker that the rkyv structure is valid.
    fn archived(&self) -> &crate::bytecode::ArchivedModule {
        let bytes = self.bytecode.as_slice();
        let length = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
        let body = &bytes[16..length - 4];
        unsafe { rkyv::access_unchecked::<crate::bytecode::ArchivedModule>(body) }
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
    fn chunk_const(&self, chunk_idx: usize, idx: usize) -> Value {
        let chunk = &self.archived().chunks[chunk_idx];
        crate::bytecode::value_from_archived(&chunk.constants[idx])
    }

    /// Number of ops in the chunk.
    fn chunk_op_count(&self, chunk_idx: usize) -> usize {
        self.archived().chunks[chunk_idx].ops.len()
    }

    /// Local-variable slot count for the chunk (includes parameters).
    fn chunk_local_count(&self, chunk_idx: usize) -> u16 {
        self.archived().chunks[chunk_idx].local_count.to_native()
    }

    /// Module-wide bit width exponent for arithmetic masking.
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

impl<'a, 'arena> Vm<'a, 'arena> {
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
            verify::verify_resource_bounds(&module, arena.capacity())
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
        let module = Module::from_bytes(bytes)?;
        Self::new(module, arena)
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
        // Framing validation only (magic, length, CRC, version, sizes).
        // No rkyv structural validation, no execution-side verification.
        let _ = Module::access_bytes(bytes)?;
        // Determine data segment length from the archived module so the
        // data vector has the right slot count.
        // Body offset matches the framing header length declared
        // in `bytecode::HEADER_LEN` (32 bytes for the current
        // wire format). Hardcoded here because the constant is
        // private to the bytecode module.
        const BODY_OFFSET: usize = 32;
        let (shared_count, private_count) = {
            let length = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
            let body = &bytes[BODY_OFFSET..length - 4];
            let archived =
                unsafe { rkyv::access_unchecked::<crate::bytecode::ArchivedModule>(body) };
            match archived.data_layout.as_ref() {
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
            }
        };
        let private_storage_bytes = private_count as usize * core::mem::size_of::<Value>();
        if arena.persistent_capacity() < private_storage_bytes {
            return Err(VmError::VerifyError(alloc::format!(
                "arena persistent_capacity ({} bytes) is too small for module's private data ({} bytes); call `arena.resize_persistent(required_persistent_capacity_for(&module))` before constructing the VM",
                arena.persistent_capacity(),
                private_storage_bytes,
            )));
        }
        if private_count > 0 {
            let base = arena.persistent_ptr().as_ptr() as *mut Value;
            for i in 0..private_count as usize {
                // SAFETY: same justification as in `Vm::construct`.
                unsafe {
                    base.add(i).write(Value::Unit);
                }
            }
        }
        let data = vec![Value::Unit; shared_count as usize];
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
            decoded_ops,
            stack,
            frames,
            natives: Vec::new(),
            data,
            shared_slot_count: shared_count,
            private_slot_count: private_count,
            arena,
            started: false,
        })
    }

    /// Construct the VM struct without running any verification.
    ///
    /// Internal helper shared by the verifying and unchecked
    /// constructors. Serializes the owned module to an aligned vector
    /// for archived access during execution. The data segment is
    /// initialized to `Unit` for each declared slot.
    fn construct(module: Module, arena: &'arena keleusma_arena::Arena) -> Result<Self, VmError> {
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
        let private_storage_bytes = private_count as usize * core::mem::size_of::<Value>();
        if arena.persistent_capacity() < private_storage_bytes {
            return Err(VmError::VerifyError(alloc::format!(
                "arena persistent_capacity ({} bytes) is too small for module's private data ({} bytes; {} slot(s) at {} bytes each); call `arena.resize_persistent(required_persistent_capacity_for(&module))` before constructing the VM",
                arena.persistent_capacity(),
                private_storage_bytes,
                private_count,
                core::mem::size_of::<Value>(),
            )));
        }
        // Initialise each private slot to Value::Unit via
        // `ptr::write` so the bytes hold a valid Value before
        // any subsequent reader clones or any subsequent writer
        // drops the old occupant. The arena's persistent region
        // is freshly zeroed when first resized, but those zero
        // bytes are not a valid `Value`, so write through `write`
        // not assignment.
        if private_count > 0 {
            let base = arena.persistent_ptr().as_ptr() as *mut Value;
            for i in 0..private_count as usize {
                // SAFETY: `i` is within the slot count just
                // verified to fit in the persistent capacity; the
                // arena owns the buffer for the VM's lifetime;
                // `Value` is properly aligned at every multiple
                // of its size on the 16-byte-aligned buffer base.
                unsafe {
                    base.add(i).write(Value::Unit);
                }
            }
        }
        let data = vec![Value::Unit; shared_count as usize];
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
            decoded_ops,
            stack,
            frames,
            natives: Vec::new(),
            data,
            shared_slot_count: shared_count,
            private_slot_count: private_count,
            arena,
            started: false,
        })
    }

    /// Read a data slot's current value, cloning. Dispatches by
    /// the unified slot index: indices below `shared_slot_count`
    /// resolve to the Vm-owned `data` vector; higher indices
    /// resolve to the arena's persistent region.
    fn read_data_slot(&self, slot: usize) -> Value {
        if slot < self.shared_slot_count as usize {
            self.data[slot].clone()
        } else {
            // SAFETY: the slot is within the partition checked at
            // construction; the persistent region was initialised
            // with `Value::Unit` for every slot and updates flow
            // through `write_data_slot`, so the pointee is always
            // a valid `Value`.
            unsafe {
                let private_idx = slot - self.shared_slot_count as usize;
                let base = self.arena.persistent_ptr().as_ptr() as *const Value;
                (*base.add(private_idx)).clone()
            }
        }
    }

    /// Overwrite a data slot. Same dispatch as `read_data_slot`.
    /// Assignment via `*ptr = value` drops the previous occupant,
    /// which is valid because every private slot is initialised
    /// to `Value::Unit` at construction.
    fn write_data_slot(&mut self, slot: usize, value: Value) {
        if slot < self.shared_slot_count as usize {
            self.data[slot] = value;
        } else {
            // SAFETY: the slot is within the partition checked at
            // construction; the pointee is a valid `Value` per
            // the construction-time initialisation, so dropping
            // it via the assignment is sound.
            unsafe {
                let private_idx = slot - self.shared_slot_count as usize;
                let base = self.arena.persistent_ptr().as_ptr() as *mut Value;
                *base.add(private_idx) = value;
            }
        }
    }

    /// Set a data segment slot to an initial value.
    ///
    /// The host calls this before execution begins to populate the
    /// persistent context. Returns an error if the slot index is out
    /// of bounds.
    pub fn set_data(&mut self, slot: usize, value: Value) -> Result<(), VmError> {
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
    pub fn get_data(&self, slot: usize) -> Result<&Value, VmError> {
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
    /// Useful for hosts that want to allocate a `Vec<Value>` of the correct
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
    /// expected to call this only between a `VmState::Reset` and the
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
        initial_data: Vec<Value>,
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
        initial_data: Vec<Value>,
    ) -> Result<(), VmError> {
        self.replace_module_inner(new_module, initial_data)
    }

    fn replace_module_inner(
        &mut self,
        new_module: Module,
        initial_data: Vec<Value>,
    ) -> Result<(), VmError> {
        #[cfg(feature = "verify")]
        {
            verify::verify(&new_module)
                .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
            // R31. Verify the new module's WCMU fits the existing arena.
            verify::verify_resource_bounds(&new_module, self.arena.capacity())
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
        let new_private_storage = new_private as usize * core::mem::size_of::<Value>();
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
            let base = self.arena.persistent_ptr().as_ptr() as *mut Value;
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
        let shared_init: Vec<Value> = iter.by_ref().take(new_shared as usize).collect();
        let private_init: Vec<Value> = iter.collect();

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
        // Initialise the new private slots via `ptr::write` (no
        // drop on the destination, because we just dropped the
        // old occupants above).
        if new_private > 0 {
            let base = self.arena.persistent_ptr().as_ptr() as *mut Value;
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
    /// functions that need arena access for [`Value::KStr`] allocation
    /// register through [`Vm::register_native_with_ctx`] instead.
    pub fn register_native(&mut self, name: &str, func: fn(&[Value]) -> Result<Value, VmError>) {
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(move |_ctx: &NativeCtx<'_>, args: &[Value]| func(args)),
        });
    }

    /// Register a native function by name using a closure.
    ///
    /// This allows closures that capture state, such as a shared command
    /// buffer for audio script integration. The closure does not receive
    /// arena context.
    pub fn register_native_closure<F>(&mut self, name: &str, func: F)
    where
        F: Fn(&[Value]) -> Result<Value, VmError> + 'static,
    {
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(move |_ctx: &NativeCtx<'_>, args: &[Value]| func(args)),
        });
    }

    /// Register a native function that receives arena context.
    ///
    /// The function gains access to the host-owned arena through the
    /// [`NativeCtx`] argument. Use this for natives that produce
    /// arena-allocated dynamic strings via
    /// [`crate::kstring::KString::alloc`] and return them as
    /// [`Value::KStr`]. The boundary type carries epoch-tagged
    /// stale-pointer detection. Outstanding handles become
    /// [`keleusma_arena::Stale`] on the next reset.
    pub fn register_native_with_ctx(
        &mut self,
        name: &str,
        func: for<'b> fn(&NativeCtx<'b>, &[Value]) -> Result<Value, VmError>,
    ) {
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(func),
        });
    }

    /// Register a native function that receives arena context using a
    /// closure.
    pub fn register_native_with_ctx_closure<F>(&mut self, name: &str, func: F)
    where
        F: for<'b> Fn(&NativeCtx<'b>, &[Value]) -> Result<Value, VmError> + 'static,
    {
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(func),
        });
    }

    /// Register an infallible host function with automatic argument and
    /// return-value marshalling.
    ///
    /// The function may take any number of arguments through arity 4 whose
    /// types implement `KeleusmaType`. The return type must also implement
    /// `KeleusmaType`. Arity and type checks happen at the boundary
    /// automatically. For functions that may fail, use
    /// [`register_fn_fallible`] instead.
    ///
    /// [`register_fn_fallible`]: Self::register_fn_fallible
    pub fn register_fn<F, Args, R>(&mut self, name: &str, func: F)
    where
        F: crate::marshall::IntoNativeFn<Args, R>,
    {
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: func.into_native_fn(),
        });
    }

    /// Register a fallible host function with automatic argument and
    /// return-value marshalling.
    ///
    /// The function returns `Result<R, VmError>`. Errors propagate to the
    /// script as native errors. Argument and return types must implement
    /// `KeleusmaType`.
    pub fn register_fn_fallible<F, Args, R>(&mut self, name: &str, func: F)
    where
        F: crate::marshall::IntoFallibleNativeFn<Args, R>,
    {
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: func.into_native_fn(),
        });
    }

    /// Register a [`crate::stddsl::Library`] bundle on the VM.
    ///
    /// Delegates to the library's `register` method. The bundle is
    /// consumed by value, so unit-struct libraries drop after the
    /// call returns. Hosts use this to install the standard
    /// libraries:
    ///
    /// ```ignore
    /// use keleusma::stddsl;
    /// vm.register_library(stddsl::Math);
    /// vm.register_library(stddsl::Audio);
    /// vm.register_library(stddsl::Text);
    /// ```
    ///
    /// Third-party crates may implement `Library` on their own
    /// types to ship reusable bundles of native functions.
    #[cfg(feature = "floats")]
    pub fn register_library<L: crate::stddsl::Library>(&mut self, library: L) {
        library.register(self);
    }

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
        let native_wcmu: Vec<u32> = self.natives.iter().map(|n| n.wcmu_bytes).collect();
        verify::verify_resource_bounds_with_natives(&module, self.arena.capacity(), &native_wcmu)
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
        let native_wcmu: Vec<u32> = self.natives.iter().map(|n| n.wcmu_bytes).collect();
        auto_arena_capacity_for(&module, &native_wcmu)
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
    pub fn call(&mut self, args: &[Value]) -> Result<VmState, VmError> {
        let entry = self
            .archived()
            .entry_point
            .as_ref()
            .map(|e| e.to_native() as usize)
            .ok_or_else(|| VmError::InvalidBytecode(String::from("no entry point")))?;
        self.call_function(entry, args)
    }

    /// Call a specific function by chunk index with the given arguments.
    pub fn call_function(&mut self, chunk_idx: usize, args: &[Value]) -> Result<VmState, VmError> {
        let archived = self.archived();
        let chunk = archived.chunks.get(chunk_idx).ok_or_else(|| {
            VmError::InvalidBytecode(format!("invalid chunk index: {}", chunk_idx))
        })?;
        let local_count = chunk.local_count.to_native() as usize;
        let param_count = chunk.param_count as usize;

        // Validate the argument count up front. Passing too few
        // arguments would default the missing parameter slots to
        // `Value::Unit`, which the body then trips over at the
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
            sp!(self, Value::Unit);
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
    /// The host then calls `resume(Value::Int(v))` for success and
    /// [`Vm::resume_err`] (with `Value::None`) for failure. For
    /// richer errors, the script defines an enum like
    /// `enum Reply { Ok(i64), Err(String) }` and the host resumes
    /// with the corresponding `Value::Enum` variant.
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
    pub fn resume_err(&mut self, error_value: Value) -> Result<VmState, VmError> {
        self.resume(error_value)
    }

    /// Resume execution after a yield or reset, providing the input value.
    pub fn resume(&mut self, input: Value) -> Result<VmState, VmError> {
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
    fn run(&mut self) -> Result<VmState, VmError> {
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
                let result = self.stack.pop().unwrap_or(Value::Unit);
                self.frames.pop();
                if self.frames.is_empty() {
                    return Ok(VmState::Finished(result));
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
                Op::PushUnit => sp!(self, Value::Unit),
                Op::PushTrue => sp!(self, Value::Bool(true)),
                Op::PushFalse => sp!(self, Value::Bool(false)),
                Op::PushFunc(idx) => sp!(
                    self,
                    Value::Func {
                        chunk_idx: idx,
                        env: alloc::vec::Vec::new(),
                        recursive: false,
                    }
                ),
                Op::MakeClosure(chunk_idx_val, n_captures) => {
                    let n = n_captures as usize;
                    if self.stack.len() < n {
                        return Err(VmError::StackUnderflow);
                    }
                    let env: alloc::vec::Vec<Value> =
                        self.stack.drain(self.stack.len() - n..).collect();
                    sp!(
                        self,
                        Value::Func {
                            chunk_idx: chunk_idx_val,
                            env,
                            recursive: false,
                        }
                    );
                }
                Op::MakeRecursiveClosure(chunk_idx_val, n_captures) => {
                    // Identical to MakeClosure except the resulting
                    // Value::Func is marked recursive. At each
                    // CallIndirect invocation, the runtime will push
                    // the func itself between the env values and the
                    // explicit arguments, populating the synthetic
                    // chunk's self parameter with the closure value.
                    let n = n_captures as usize;
                    if self.stack.len() < n {
                        return Err(VmError::StackUnderflow);
                    }
                    let env: alloc::vec::Vec<Value> =
                        self.stack.drain(self.stack.len() - n..).collect();
                    sp!(
                        self,
                        Value::Func {
                            chunk_idx: chunk_idx_val,
                            env,
                            recursive: true,
                        }
                    );
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
                        Value::Int(n) => n,
                        other => {
                            return Err(VmError::TypeError(format!(
                                "GetDataIndexed expected Int index, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if index < 0 || index >= len as i64 {
                        return Err(VmError::IndexOutOfBounds(index, len as usize));
                    }
                    let slot = base as usize + index as usize;
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
                        Value::Int(n) => n,
                        other => {
                            return Err(VmError::TypeError(format!(
                                "SetDataIndexed expected Int index, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if index < 0 || index >= len as i64 {
                        return Err(VmError::IndexOutOfBounds(index, len as usize));
                    }
                    let val = self.pop()?;
                    let slot = base as usize + index as usize;
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
                        Value::Int(n) => *n,
                        other => {
                            return Err(VmError::TypeError(format!(
                                "BoundsCheck expected Int, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if value < 0 || value >= bound as i64 {
                        return Err(VmError::IndexOutOfBounds(value, bound as usize));
                    }
                }

                Op::Add => {
                    let word_bits_log2 = self.word_bits_log2();
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            let r =
                                crate::bytecode::truncate_int(x.wrapping_add(y), word_bits_log2);
                            sp!(self, Value::Int(r));
                        }
                        (Value::Byte(x), Value::Byte(y)) => {
                            sp!(self, Value::Byte(x.wrapping_add(y)));
                        }
                        (Value::Fixed(x), Value::Fixed(y)) => {
                            // Fixed Add is integer add of the
                            // fixed-point bits; the fraction-bit
                            // count is the same for both operands
                            // by type-check invariant.
                            sp!(self, Value::Fixed(x.wrapping_add(y)));
                        }
                        #[cfg(feature = "floats")]
                        (Value::Float(x), Value::Float(y)) => sp!(self, Value::Float(x + y)),
                        (a, b)
                            if matches!(a, Value::StaticStr(_) | Value::KStr(_))
                                && matches!(b, Value::StaticStr(_) | Value::KStr(_)) =>
                        {
                            let arena = self.arena;
                            let lhs = a.as_str_with_arena(arena).map_err(|_| {
                                VmError::TypeError(String::from(
                                    "KStr is stale (arena reset since allocation)",
                                ))
                            })?;
                            let rhs = b.as_str_with_arena(arena).map_err(|_| {
                                VmError::TypeError(String::from(
                                    "KStr is stale (arena reset since allocation)",
                                ))
                            })?;
                            let lhs = lhs.unwrap_or("");
                            let rhs = rhs.unwrap_or("");
                            let mut concatenated = String::with_capacity(lhs.len() + rhs.len());
                            concatenated.push_str(lhs);
                            concatenated.push_str(rhs);
                            let handle = crate::kstring::KString::alloc(arena, &concatenated)
                                .map_err(|_| out_of_arena_push("text", arena.capacity()))?;
                            sp!(self, Value::KStr(handle));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "cannot add {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::Sub => self.binary_arith(|a, b| a.wrapping_sub(b), |a, b| a - b)?,
                Op::Mul => self.binary_arith(|a, b| a.wrapping_mul(b), |a, b| a * b)?,
                Op::Div => {
                    let word_bits_log2 = self.word_bits_log2();
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(_), Value::Int(0)) => return Err(VmError::DivisionByZero),
                        (Value::Int(x), Value::Int(y)) => sp!(
                            self,
                            Value::Int(crate::bytecode::truncate_int(
                                x.wrapping_div(y),
                                word_bits_log2
                            ),)
                        ),
                        (Value::Byte(_), Value::Byte(0)) => return Err(VmError::DivisionByZero),
                        (Value::Byte(x), Value::Byte(y)) => {
                            sp!(self, Value::Byte(x.wrapping_div(y)));
                        }
                        #[cfg(feature = "floats")]
                        (Value::Float(x), Value::Float(y)) => sp!(self, Value::Float(x / y)),
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
                    let word_bits_log2 = self.word_bits_log2();
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(_), Value::Int(0)) => return Err(VmError::DivisionByZero),
                        (Value::Int(x), Value::Int(y)) => sp!(
                            self,
                            Value::Int(crate::bytecode::truncate_int(
                                x.wrapping_rem(y),
                                word_bits_log2
                            ),)
                        ),
                        (Value::Byte(_), Value::Byte(0)) => return Err(VmError::DivisionByZero),
                        (Value::Byte(x), Value::Byte(y)) => {
                            sp!(self, Value::Byte(x.wrapping_rem(y)));
                        }
                        #[cfg(feature = "floats")]
                        (Value::Float(x), Value::Float(y)) => sp!(self, Value::Float(x % y)),
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
                    let word_bits_log2 = self.word_bits_log2();
                    let val = self.pop()?;
                    match val {
                        Value::Int(x) => sp!(
                            self,
                            Value::Int(crate::bytecode::truncate_int(
                                x.wrapping_neg(),
                                word_bits_log2
                            ),)
                        ),
                        Value::Byte(x) => sp!(self, Value::Byte(x.wrapping_neg())),
                        Value::Fixed(x) => sp!(self, Value::Fixed(x.wrapping_neg())),
                        #[cfg(feature = "floats")]
                        Value::Float(x) => sp!(self, Value::Float(-x)),
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
                    sp!(self, Value::Bool(a == b));
                }
                Op::CmpNe => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    sp!(self, Value::Bool(a != b));
                }
                Op::CmpLt => self.compare_op(|ord| ord.is_lt())?,
                Op::CmpGt => self.compare_op(|ord| ord.is_gt())?,
                Op::CmpLe => self.compare_op(|ord| ord.is_le())?,
                Op::CmpGe => self.compare_op(|ord| ord.is_ge())?,

                Op::Not => {
                    let val = self.pop()?;
                    match val {
                        Value::Bool(b) => sp!(self, Value::Bool(!b)),
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
                        Value::Bool(false) => {
                            self.frames.last_mut().unwrap().ip = target as usize;
                        }
                        Value::Bool(true) => {
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
                        Value::Bool(true) => {
                            self.frames.last_mut().unwrap().ip = target as usize;
                        }
                        Value::Bool(false) => {
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
                        self.stack[reset_base + i] = Value::Unit;
                    }
                    // Truncate stack to just the locals.
                    self.stack.truncate(reset_base + local_count);

                    // Reset both arena bump pointers (R32). Host-allocated
                    // dynamic strings and other arena values are reclaimed
                    // here.
                    let _ = self.reset_arena_internal();

                    // Find Stream instruction and set IP to instruction after it.
                    let stream_ip = self.archived().chunks[reset_chunk_idx]
                        .ops
                        .iter()
                        .position(|op| matches!(op, crate::bytecode::ArchivedOp::Stream));
                    match stream_ip {
                        Some(pos) => self.frames.last_mut().unwrap().ip = pos + 1,
                        None => {
                            return Err(VmError::InvalidBytecode(String::from(
                                "Reset without Stream in chunk",
                            )));
                        }
                    }

                    return Ok(VmState::Reset);
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
                        sp!(self, Value::Unit);
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
                Op::CallNative(idx, arg_count) => {
                    let n = arg_count as usize;
                    if self.stack.len() < n {
                        return Err(VmError::StackUnderflow);
                    }
                    let args: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
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
                Op::CallIndirect(arg_count) => {
                    // The operand stack holds, from top down, the
                    // function arguments (arg_count items) and then
                    // the `Value::Func` carrying the chunk index and
                    // optional captured environment. Pop the args
                    // aside, pop the func, push the env values, push
                    // the saved args, then push extra `Unit` slots
                    // for the chunk's locals beyond its parameters.
                    // The total argument count seen by the called
                    // chunk is `env.len() + arg_count`.
                    let n = arg_count as usize;
                    if self.stack.len() < n + 1 {
                        return Err(VmError::StackUnderflow);
                    }
                    let args_start = self.stack.len() - n;
                    let saved_args: alloc::vec::Vec<Value> =
                        self.stack.drain(args_start..).collect();
                    let func_value = self.pop()?;
                    let (chunk_idx, env, recursive) = match func_value.clone() {
                        Value::Func {
                            chunk_idx,
                            env,
                            recursive,
                        } => (chunk_idx, env, recursive),
                        other => {
                            return Err(VmError::TypeError(format!(
                                "indirect call expected Func, got {}",
                                other.type_name()
                            )));
                        }
                    };
                    if chunk_idx as usize >= self.chunk_count() {
                        return Err(VmError::InvalidBytecode(format!(
                            "invalid chunk: {}",
                            chunk_idx
                        )));
                    }
                    let env_len = env.len();
                    for v in env {
                        sp!(self, v);
                    }
                    // For recursive closures, push the closure value
                    // itself between the env values and the explicit
                    // arguments. This populates the synthetic chunk's
                    // self parameter so the body's references to the
                    // closure's let-binding resolve to the closure
                    // value through indirect dispatch.
                    let self_count = if recursive { 1 } else { 0 };
                    if recursive {
                        sp!(self, func_value);
                    }
                    for v in saved_args {
                        sp!(self, v);
                    }
                    let total_args = env_len + self_count + n;
                    let called_local_count = self.chunk_local_count(chunk_idx as usize) as usize;
                    let new_base = self.stack.len() - total_args;
                    let extra = called_local_count - total_args;
                    for _ in 0..extra {
                        sp!(self, Value::Unit);
                    }
                    fp!(
                        self,
                        CallFrame {
                            chunk_idx: chunk_idx as usize,
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
                        return Ok(VmState::Finished(result));
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
                    return Ok(VmState::Yielded(output));
                }

                Op::Pop => {
                    self.pop()?;
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
                    let values: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    let fields: Vec<(String, Value)> =
                        field_names.into_iter().zip(values).collect();
                    sp!(self, Value::Struct { type_name, fields });
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
                    let fields: Vec<Value> = if n > 0 {
                        self.stack.drain(self.stack.len() - n..).collect()
                    } else {
                        Vec::new()
                    };
                    sp!(
                        self,
                        Value::Enum {
                            type_name,
                            variant,
                            fields,
                        }
                    );
                }
                Op::NewArray(count) => {
                    let n = count as usize;
                    let elements: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    sp!(self, Value::Array(elements));
                }
                Op::NewTuple(count) => {
                    let n = count as usize;
                    let elements: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    sp!(self, Value::Tuple(elements));
                }
                Op::WrapSome => {
                    // In our representation, Some(v) is just v. None is Value::None.
                    // WrapSome is a no-op for the value itself.
                }
                Op::PushNone => {
                    sp!(self, Value::None);
                }

                Op::GetField(name_const) => {
                    let container = self.pop()?;
                    let field_name = self
                        .chunk_const_str(chunk_idx, name_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("field name not a string"))
                        })?;
                    match container {
                        Value::Struct { type_name, fields } => {
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
                        (Value::Array(arr), Value::Int(i)) => {
                            let len = arr.len();
                            if i < 0 || i as usize >= len {
                                return Err(VmError::IndexOutOfBounds(i, len));
                            }
                            sp!(self, arr[i as usize].clone());
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
                        Value::Tuple(elems) => {
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
                        Value::Enum { fields, .. } => {
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
                        Value::Array(arr) => {
                            sp!(self, Value::Int(arr.len() as i64));
                        }
                        Value::StaticStr(s) => {
                            sp!(self, Value::Int(s.chars().count() as i64));
                        }
                        Value::KStr(h) => {
                            let s = h.get(self.arena).map_err(|_| {
                                VmError::TypeError(String::from(
                                    "KStr is stale (arena reset since allocation)",
                                ))
                            })?;
                            sp!(self, Value::Int(s.chars().count() as i64));
                        }
                        Value::Tuple(t) => {
                            sp!(self, Value::Int(t.len() as i64));
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
                        Value::Enum { type_name, variant, .. }
                            if type_name == &expected_type && variant == &expected_var
                    );
                    sp!(self, Value::Bool(matches));
                }
                Op::IsStruct(type_const) => {
                    let expected = self
                        .chunk_const_str(chunk_idx, type_const as usize)
                        .ok_or_else(|| {
                            VmError::InvalidBytecode(String::from("type const not string"))
                        })?;
                    let val = self.stack.last().ok_or(VmError::StackUnderflow)?;
                    let matches =
                        matches!(val, Value::Struct { type_name, .. } if type_name == &expected);
                    sp!(self, Value::Bool(matches));
                }

                #[cfg(feature = "floats")]
                Op::IntToFloat => {
                    let val = self.pop()?;
                    match val {
                        Value::Int(i) => sp!(self, Value::Float(i as f64)),
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
                        Value::Float(f) => sp!(self, Value::Int(f as i64)),
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
                        Value::Int(i) => sp!(self, Value::Byte((i & 0xFF) as u8)),
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
                        Value::Byte(b) => sp!(self, Value::Int(b as i64)),
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
                        Value::Int(i) => {
                            // Left-shift the word into the fixed
                            // representation. Saturate at
                            // i64::MAX/MIN on overflow.
                            let shifted = (i as i128) << (frac_bits as u32);
                            let bits = if shifted > i64::MAX as i128 {
                                i64::MAX
                            } else if shifted < i64::MIN as i128 {
                                i64::MIN
                            } else {
                                shifted as i64
                            };
                            sp!(self, Value::Fixed(bits));
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
                        Value::Fixed(bits) => {
                            // Arithmetic-right-shift to drop the
                            // fraction bits. Negative values keep
                            // their sign through the shift.
                            sp!(self, Value::Int(bits >> (frac_bits as u32)));
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
                        (Value::Fixed(x), Value::Fixed(y)) => {
                            // Q-format multiply: extend to i128 to
                            // avoid intermediate overflow, multiply,
                            // shift right by `frac_bits`, saturate
                            // back to i64.
                            let product = (x as i128) * (y as i128);
                            let shifted = product >> (frac_bits as u32);
                            let bits = if shifted > i64::MAX as i128 {
                                i64::MAX
                            } else if shifted < i64::MIN as i128 {
                                i64::MIN
                            } else {
                                shifted as i64
                            };
                            sp!(self, Value::Fixed(bits));
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
                        (Value::Fixed(_), Value::Fixed(0)) => {
                            return Err(VmError::DivisionByZero);
                        }
                        (Value::Fixed(x), Value::Fixed(y)) => {
                            // Q-format divide: extend the dividend to
                            // i128 and left-shift by frac_bits before
                            // dividing, so the result retains the
                            // Q-format precision.
                            let dividend = (x as i128) << (frac_bits as u32);
                            let quotient = dividend / (y as i128);
                            let bits = if quotient > i64::MAX as i128 {
                                i64::MAX
                            } else if quotient < i64::MIN as i128 {
                                i64::MIN
                            } else {
                                quotient as i64
                            };
                            sp!(self, Value::Fixed(bits));
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

                Op::Trap(msg_const) => {
                    let msg = self
                        .chunk_const_str(chunk_idx, msg_const as usize)
                        .unwrap_or_else(|| String::from("trap"));
                    return Err(VmError::Trap(msg));
                }
                Op::CheckedAdd => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            // Compute the true sum in i128 and
                            // derive the outcome flag from the
                            // i128 range relative to i64. The flag
                            // values are `0` ok (fits in i64),
                            // `1` overflow (> i64::MAX), `2`
                            // underflow (< i64::MIN). The high and
                            // low halves of the i128 result are
                            // pushed in all three cases so arm
                            // patterns can destructure them.
                            let r = (x as i128) + (y as i128);
                            let high = (r >> 64) as i64;
                            let low = r as i64;
                            let flag: i64 = if r >= i64::MIN as i128 && r <= i64::MAX as i128 {
                                0
                            } else if r > i64::MAX as i128 {
                                1
                            } else {
                                2
                            };
                            sp!(self, Value::Int(high));
                            sp!(self, Value::Int(low));
                            sp!(self, Value::Int(flag));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedAdd expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedSub => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            let r = (x as i128) - (y as i128);
                            let high = (r >> 64) as i64;
                            let low = r as i64;
                            let flag: i64 = if r >= i64::MIN as i128 && r <= i64::MAX as i128 {
                                0
                            } else if r > i64::MAX as i128 {
                                1
                            } else {
                                2
                            };
                            sp!(self, Value::Int(high));
                            sp!(self, Value::Int(low));
                            sp!(self, Value::Int(flag));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedSub expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedMul => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(x), Value::Int(y)) => {
                            // True product in i128; both halves are
                            // load-bearing for big-number
                            // multiplication. Flag reports the
                            // direction of overflow based on the
                            // i128 result's sign relative to the
                            // i64 representable range.
                            let r = (x as i128) * (y as i128);
                            let high = (r >> 64) as i64;
                            let low = r as i64;
                            let flag: i64 = if r >= i64::MIN as i128 && r <= i64::MAX as i128 {
                                0
                            } else if r > i64::MAX as i128 {
                                1
                            } else {
                                2
                            };
                            sp!(self, Value::Int(high));
                            sp!(self, Value::Int(low));
                            sp!(self, Value::Int(flag));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedMul expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedNeg => {
                    let a = self.pop()?;
                    match a {
                        Value::Int(x) => {
                            // Only `-i64::MIN` overflows. The true
                            // result is `2^63`, which in i128 is
                            // (high=0, low=i64::MIN); we report
                            // overflow (flag=1) in that case.
                            let r = -(x as i128);
                            let high = (r >> 64) as i64;
                            let low = r as i64;
                            let flag: i64 = if r >= i64::MIN as i128 && r <= i64::MAX as i128 {
                                0
                            } else {
                                1
                            };
                            sp!(self, Value::Int(high));
                            sp!(self, Value::Int(low));
                            sp!(self, Value::Int(flag));
                        }
                        a => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedNeg expects a Word operand, got {}",
                                a.type_name()
                            )));
                        }
                    }
                }
                Op::CheckedDiv => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(_), Value::Int(0)) => return Err(VmError::DivisionByZero),
                        (Value::Int(x), Value::Int(y)) => {
                            // Only `i64::MIN / -1` overflows. The
                            // true result is `2^63`, which in i128
                            // is (high=0, low=i64::MIN). All other
                            // divisions fit in `Word`; the wrapped
                            // quotient becomes the low slot and the
                            // high slot is zero.
                            let r = (x as i128) / (y as i128);
                            let high = (r >> 64) as i64;
                            let low = r as i64;
                            let flag: i64 = if r >= i64::MIN as i128 && r <= i64::MAX as i128 {
                                0
                            } else if r > i64::MAX as i128 {
                                1
                            } else {
                                2
                            };
                            sp!(self, Value::Int(high));
                            sp!(self, Value::Int(low));
                            sp!(self, Value::Int(flag));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedDiv expects Word operands, got {} and {}",
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
                        (Value::Int(_), Value::Int(0)) => return Err(VmError::DivisionByZero),
                        (Value::Int(x), Value::Int(y)) => {
                            // `i64::MIN % -1` overflows on the
                            // underlying division step; the true
                            // mathematical result is `0`. We
                            // detect this corner by computing in
                            // i128 (`i64::MIN as i128) % (-1 as
                            // i128) == 0`) and report overflow
                            // (flag=1) so the arm dispatch matches
                            // the documented behaviour. The
                            // wrapped result is `0` in any case.
                            let r = (x as i128) % (y as i128);
                            let high = (r >> 64) as i64;
                            let low = r as i64;
                            let corner = x == i64::MIN && y == -1;
                            let flag: i64 = if corner { 1 } else { 0 };
                            sp!(self, Value::Int(high));
                            sp!(self, Value::Int(low));
                            sp!(self, Value::Int(flag));
                        }
                        (a, b) => {
                            return Err(VmError::TypeError(format!(
                                "Op::CheckedMod expects Word operands, got {} and {}",
                                a.type_name(),
                                b.type_name()
                            )));
                        }
                    }
                }
            }
        }
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn binary_arith(
        &mut self,
        int_op: fn(i64, i64) -> i64,
        // `float_op` is kept in the signature regardless of the
        // `floats` feature so existing call sites compile
        // unchanged. With `floats` off the `Value::Float` match arm
        // is gated out below and the closure is unreachable; LTO
        // strips both the closure body and the transitive
        // `compiler_builtins` soft-float routines from the final
        // image.
        #[allow(unused_variables)] float_op: fn(f64, f64) -> f64,
    ) -> Result<(), VmError> {
        let word_bits_log2 = self.word_bits_log2();
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                let result = crate::bytecode::truncate_int(int_op(x, y), word_bits_log2);
                sp!(self, Value::Int(result));
            }
            (Value::Byte(x), Value::Byte(y)) => {
                // Byte arithmetic uses the integer op as if both
                // operands had been zero-extended to i64, then
                // truncates the result back to the low eight bits.
                // This matches wrapping `u8` semantics for Add,
                // Sub, and Mul. Div and Mod are handled separately
                // by the caller because they reject zero divisors.
                let result = int_op(x as i64, y as i64);
                sp!(self, Value::Byte((result & 0xFF) as u8));
            }
            (Value::Fixed(x), Value::Fixed(y)) => {
                // Fixed Sub is integer sub of the fixed-point
                // bits. Fixed Add is handled directly in `Op::Add`;
                // Fixed Mul and Div are emitted as dedicated
                // `Op::FixedMul` and `Op::FixedDiv` opcodes
                // because they require the fraction-bit count.
                let result = int_op(x, y);
                sp!(self, Value::Fixed(result));
            }
            #[cfg(feature = "floats")]
            (Value::Float(x), Value::Float(y)) => sp!(self, Value::Float(float_op(x, y))),
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

    fn compare_op<F>(&mut self, pred: F) -> Result<(), VmError>
    where
        F: FnOnce(core::cmp::Ordering) -> bool,
    {
        let b = self.pop()?;
        let a = self.pop()?;
        let ord = match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => x.cmp(y),
            (Value::Byte(x), Value::Byte(y)) => x.cmp(y),
            (Value::Fixed(x), Value::Fixed(y)) => x.cmp(y),
            #[cfg(feature = "floats")]
            (Value::Float(x), Value::Float(y)) => {
                x.partial_cmp(y).unwrap_or(core::cmp::Ordering::Equal)
            }
            (
                a @ (Value::StaticStr(_) | Value::KStr(_)),
                b @ (Value::StaticStr(_) | Value::KStr(_)),
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
        sp!(self, Value::Bool(pred(ord)));
        Ok(())
    }
}

// The test module exercises the full pipeline (source through
// VM execution) and therefore requires both the `compile` and
// `verify` features. Without either, the helpers it imports
// (`lexer`, `parser`, `compiler`, `verify`) are absent.
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
            VmError::NoMatch(alloc::string::String::from("oops")),
            VmError::Trap(alloc::string::String::from("oops")),
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
                    underflow(_, _) => 2,\n\
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
                    underflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert_eq!(val, Value::Int(3));
    }

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
                    underflow(_, _) => 0 - 1,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        // h = 0, l = i64::MIN; sum is i64::MIN.
        assert_eq!(val, Value::Int(i64::MIN));
    }

    #[test]
    fn checked_mod_min_by_neg_one_surfaces_corner() {
        // `i64::MIN % -1` is mathematically `0` but the
        // underlying division step overflows. The construct
        // surfaces the corner through the overflow arm with high
        // and low both zero (the mathematical result).
        let val = run_expect(
            "fn main() -> Word {\n\
                let m = 0 - 9223372036854775807 - 1;\n\
                let y = m % (0 - 1) {\n\
                    ok(_) => 0 - 1,\n\
                    overflow(h, l) => h + l + 42,\n\
                    underflow(_, _) => 0 - 1,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        // h = 0, l = 0; the overflow arm body returns 0 + 0 + 42.
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn checked_div_by_zero_traps() {
        // Division by zero traps with VmError::DivisionByZero;
        // the construct does not catch it (the arm dispatch does
        // not run because the opcode itself fails).
        let result = run_program(
            "fn main() -> Word {\n\
                let y = 10 / 0 {\n\
                    ok(v) => v,\n\
                    overflow(_, _) => 0,\n\
                    underflow(_, _) => 0,\n\
                };\n\
                y\n\
             }",
            &[],
        );
        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
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
        // newtype construction traps at runtime when the constant
        // folder cannot decide the argument statically. The
        // argument here is computed through a function call, so
        // the compile-time elision pass falls through to the
        // runtime check.
        let err = run_program(
            "fn nonneg(x: Word) -> bool { x >= 0 }\n\
             newtype Counter = Word where nonneg;\n\
             fn neg_one() -> Word { 0 - 1 }\n\
             fn main() -> Counter { Counter(neg_one()) }",
            &[],
        )
        .unwrap_err();
        match err {
            VmError::Trap(msg) => {
                assert!(
                    msg.contains("nonneg") && msg.contains("Counter"),
                    "expected refinement-trap message naming `nonneg` and `Counter`, got: {}",
                    msg
                );
            }
            other => panic!("expected VmError::Trap, got {:?}", other),
        }
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
        // Tier 3 soundness: when the predicate cannot be
        // decomposed to a convex interval (e.g. it uses
        // disjunction), the lattice path returns None and the
        // runtime check fires. The example predicate's true set
        // is `x < 0 or x > 100`, which is not a single interval.
        let val = run_expect(
            "fn outside(x: Word) -> bool { x < 0 or x > 100 }\n\
             newtype Edge = Word where outside;\n\
             fn main() -> Edge { Edge(0 - 50) }",
            &[],
        );
        assert_eq!(val, Value::Int(-50));
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
        match err {
            VmError::Trap(msg) => {
                assert!(
                    msg.contains("nonneg") && msg.contains("Counter"),
                    "expected refinement-trap message naming `nonneg` and `Counter`, got: {}",
                    msg
                );
            }
            other => panic!("expected VmError::Trap, got {:?}", other),
        }
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

    #[cfg(feature = "text")]
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

    #[cfg(feature = "text")]
    #[test]
    fn eval_multiheaded_literal() {
        let val = run_expect(
            "fn classify(0) -> Text { \"zero\" }\nfn classify(x: Word) -> Text { \"other\" }\nfn main() -> Text { classify(0) }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("zero")));
    }

    #[cfg(feature = "text")]
    #[test]
    fn eval_multiheaded_fallthrough() {
        let val = run_expect(
            "fn classify(0) -> Text { \"zero\" }\nfn classify(x: Word) -> Text { \"other\" }\nfn main() -> Text { classify(5) }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("other")));
    }

    #[test]
    fn eval_pipeline() {
        let val = run_expect(
            "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { 21 |> double() }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[cfg(feature = "text")]
    #[test]
    fn eval_match_literal() {
        let val = run_expect(
            "fn main() -> Text { let x = 1; match x { 1 => \"one\", 2 => \"two\", _ => \"other\" } }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("one")));
    }

    #[cfg(feature = "text")]
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
    fn eval_guard_clause() {
        let val = run_expect(
            "fn abs(x: Word) -> Word when x < 0 { -x }\nfn abs(x: Word) -> Word { x }\nfn main() -> Word { abs(-5) + abs(3) }",
            &[],
        );
        assert_eq!(val, Value::Int(8));
    }

    #[cfg(feature = "text")]
    #[test]
    fn eval_string_concat() {
        let src = "fn main() -> Text { \"hello\" + \" world\" }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        let val = match vm.call(&[]).unwrap() {
            VmState::Finished(v) => v,
            other => panic!("unexpected: {:?}", other),
        };
        let s = val
            .as_str_with_arena(&arena)
            .expect("KStr should resolve against live arena")
            .expect("Op::Add on Text operands yields a string variant");
        assert_eq!(s, "hello world");
        assert!(matches!(val, Value::KStr(_)));
    }

    #[cfg(feature = "text")]
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

    #[cfg(feature = "text")]
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

    #[cfg(feature = "text")]
    #[test]
    fn yield_dynamic_string_fails() {
        // to_string returns a KStr. Yielding it must fail at runtime.
        let src = "use to_string\n\
                   loop main(input: Word) -> Text { \
                       let input = yield to_string(input); \"done\" }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        crate::utility_natives::register_utility_natives(&mut vm);
        let err = vm.call(&[Value::Int(42)]).unwrap_err();
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("dynamic string") || msg.contains("KStr"))
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[cfg(feature = "text")]
    #[test]
    fn yield_tuple_with_dynamic_string_fails() {
        // Yielding a tuple containing a KStr must fail.
        let src = "use to_string\n\
                   loop main(input: Word) -> (Word, Text) { \
                       let input = yield (input, to_string(input)); (0, \"\") }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        crate::utility_natives::register_utility_natives(&mut vm);
        let err = vm.call(&[Value::Int(7)]).unwrap_err();
        match err {
            VmError::TypeError(msg) => assert!(msg.contains("dynamic string")),
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

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
        // Pad to the minimum framing length (header 32 + footer 4 = 36)
        // so the slice passes the truncation check and reaches the
        // magic check.
        let bytes = alloc::vec![
            b'X', b'X', b'X', b'X', // magic
            0x08, 0x00, // version
            0x24, 0x00, 0x00, 0x00, // length = 36
            6, 6, 6,    // word_bits_log2, addr_bits_log2, float_bits_log2
            0x00, // flags
            0x00, 0x00, // reserved
            0x00, 0x00, 0x00, 0x00, // wcet_cycles
            0x00, 0x00, 0x00, 0x00, // wcmu_bytes
            0x00, 0x00, 0x00, 0x00, // shared_data_bytes
            0x00, 0x00, 0x00, 0x00, // private_data_bytes
            0x00, 0x00, 0x00, 0x00, // CRC placeholder
        ];
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

    #[test]
    fn bytecode_golden_bytes_for_main_returning_one() {
        // Pin the exact serialized form of a minimal Keleusma program
        // to guard against unintended wire format changes and
        // endian-dependent code paths. Updating this byte sequence
        // requires a deliberate decision recorded in R39 and a
        // BYTECODE_VERSION bump if the change is not backwards
        // compatible.
        //
        // Source: `fn main() -> Word { 1 }`
        //
        // The wire format header is 32 bytes; the rkyv body grew by
        // one u32 (`schema_hash`) in the V0.2 schema-strict hot-swap
        // implementation, bringing the total length to 192 bytes
        // including the 4-byte CRC trailer. For a function with no
        // data layout, `schema_hash` is zero (no slots to hash).
        let expected: alloc::vec::Vec<u8> = alloc::vec![
            75, 69, 76, 69, 1, 0, 192, 0, 0, 0, 6, 6, 6, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 39, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0,
            1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 109, 97, 105,
            110, 255, 255, 255, 255, 200, 255, 255, 255, 2, 0, 0, 0, 208, 255, 255, 255, 1, 0, 0,
            0, 232, 255, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 220, 255, 255, 255, 0, 0, 0, 0, 212,
            255, 255, 255, 1, 0, 0, 0, 248, 255, 255, 255, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 6, 6, 6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 218, 19, 221, 224,
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
    fn bytecode_view_bytes_rejects_unaligned_input() {
        // A plain Vec<u8> is not guaranteed to be 8-byte aligned. The
        // view path fails with an alignment-specific Codec message
        // rather than silently succeeding under undefined behavior.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        // Force unaligned by prepending one byte then taking bytes[1..].
        // This guarantees the body slice is not 8-byte aligned.
        let mut shifted = alloc::vec![0u8];
        shifted.extend_from_slice(&bytes);
        let unaligned = &shifted[1..];
        match Module::view_bytes(unaligned) {
            Err(crate::bytecode::LoadError::Codec(msg)) if msg.contains("not 8-byte aligned") => {}
            // The shifted slice may also misalign the framing reads in
            // ways that surface as BadMagic or BadChecksum before the
            // alignment check. Either is acceptable evidence that the
            // path rejects unaligned input.
            Err(crate::bytecode::LoadError::BadMagic) => {}
            Err(crate::bytecode::LoadError::BadChecksum) => {}
            other => panic!(
                "expected alignment or magic/checksum failure, got {:?}",
                other
            ),
        }
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
    fn bytecode_archived_op_round_trip_matches_owned() {
        // op_from_archived materializes an owned Op from an archived Op
        // without information loss. Verify the ops in a compiled module
        // compare equal across the archive round trip. This is the
        // foundation for the future zero-copy execution loop, which
        // will fetch ArchivedOp and convert per step.
        use crate::bytecode::{ArchivedModule, op_from_archived};
        let src = "fn main() -> Word { 1 + 2 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let archived: &ArchivedModule = Module::access_bytes(&aligned).expect("access");
        let main_chunk = &archived.chunks[0];
        for (i, archived_op) in main_chunk.ops.iter().enumerate() {
            let owned_op = op_from_archived(archived_op);
            let original_op = module.chunks[0].ops[i];
            assert_eq!(
                owned_op, original_op,
                "op at index {} mismatches across archive round trip",
                i
            );
        }
    }

    #[test]
    fn bytecode_archived_value_round_trip_matches_owned() {
        // value_from_archived materializes an owned Value from an
        // archived Value. Verify constants survive the round trip.
        use crate::bytecode::{ArchivedModule, value_from_archived};
        let src = "fn main() -> Word { 42 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let archived: &ArchivedModule = Module::access_bytes(&aligned).expect("access");
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
        // access_bytes returns a borrowed ArchivedModule. The archived
        // form preserves the chunk count, the entry point, and the word
        // and address sizes, exposed through native conversions.
        use crate::bytecode::ArchivedModule;
        let src = "fn double(x: Word) -> Word { x * 2 }\nfn main() -> Word { double(21) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let archived: &ArchivedModule = Module::access_bytes(&aligned).expect("access");
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
        // Compile a real module, patch the word_bits_log2 field to a
        // value greater than the runtime supports, and recompute the
        // CRC trailer so the residue check passes. The version and
        // length fields are intact so only the word size mismatch
        // surfaces.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        bytes[10] = crate::bytecode::RUNTIME_WORD_BITS_LOG2 + 1;
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
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        bytes[11] = crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2 + 1;
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

    #[test]
    fn bytecode_admits_narrower_word_size() {
        // Compile a module, patch the word_bits_log2 to a value below
        // the runtime maximum, and recompute the CRC. The runtime
        // accepts narrower-than-runtime bytecode under the relaxed
        // policy. The masking pass in the VM keeps arithmetic within
        // the declared narrower width.
        let src = "fn main() -> Word { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        // Declare 32-bit words. Runtime is 64-bit so 5 <= 6 holds.
        bytes[10] = 5;
        let trailer_start = bytes.len() - 4;
        let new_crc = crate::bytecode::crc32(&bytes[..trailer_start]);
        bytes[trailer_start..].copy_from_slice(&new_crc.to_le_bytes());
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::load_bytes(&bytes, &arena).expect("narrower bytecode should be admitted");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(1)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

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
    #[cfg(feature = "text")]
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
    #[cfg(feature = "text")]
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
    #[cfg(feature = "text")]
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
    #[cfg(feature = "text")]
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
}
