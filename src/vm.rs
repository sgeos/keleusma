extern crate alloc;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::bytecode::*;
use crate::verify;

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
}

impl From<crate::bytecode::LoadError> for VmError {
    fn from(e: crate::bytecode::LoadError) -> Self {
        VmError::LoadError(format!("{}", e))
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

/// Type alias for a native function callable from Keleusma.
type NativeFn = Box<dyn Fn(&[Value]) -> Result<Value, VmError>>;

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

/// Compute the smallest arena capacity that admits the given module
/// under the supplied native attestations. Returns the maximum WCMU sum
/// across Stream chunks, or zero if the module has no Stream chunks.
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
/// The lifetime parameter `'a` reflects the bytecode source. VMs
/// constructed from an owned `Module` or from arbitrary byte slices
/// carry `Vm<'static>`. VMs constructed via `view_bytes_unchecked`
/// from a borrowed slice carry `Vm<'a>` for the slice's lifetime.
pub struct Vm<'a> {
    bytecode: BytecodeStore<'a>,
    stack: Vec<Value>,
    frames: Vec<CallFrame>,
    natives: Vec<NativeEntry>,
    /// Persistent data segment. Survives across RESET boundaries.
    data: Vec<Value>,
    /// Dual-end bump-allocated arena. Currently exposed for explicit native
    /// function use through `Vm::arena()`. The operand stack and dynamic
    /// string storage do not yet route through the arena. See P7 and P8 in
    /// `docs/decisions/PRIORITY.md` for the integration plan.
    arena: keleusma_arena::Arena,
    started: bool,
}

impl<'a> Vm<'a> {
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
    fn module_owned(&self) -> Result<Module, VmError> {
        Ok(Module::from_bytes(self.bytecode.as_slice())?)
    }

    /// Materialize the op at `(chunk_idx, ip)` from archived storage.
    fn chunk_op(&self, chunk_idx: usize, ip: usize) -> Op {
        let chunk = &self.archived().chunks[chunk_idx];
        crate::bytecode::op_from_archived(&chunk.ops[ip])
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
            crate::bytecode::ArchivedValue::StaticStr(s)
            | crate::bytecode::ArchivedValue::DynStr(s) => Some(s.as_str().to_string()),
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

impl<'a> Vm<'a> {
    /// Create a new VM with the given compiled module and the default arena
    /// capacity.
    ///
    /// Runs structural verification on the module. Returns an error if
    /// verification fails.
    pub fn new(module: Module) -> Result<Self, VmError> {
        Self::new_with_arena_capacity(module, DEFAULT_ARENA_CAPACITY)
    }

    /// Construct a VM with an arena auto-sized from the module's
    /// worst-case memory usage.
    ///
    /// The capacity is the sum of the worst-case stack and heap WCMU of
    /// the entry-point Stream chunk, computed with default native
    /// attestations (zero heap). For modules without function calls or
    /// natives, this produces a tight bound. For modules whose natives
    /// allocate from the arena, the host should set native bounds and
    /// call [`Vm::verify_resources`] afterward; if verification fails
    /// because the auto-sized arena is too small, construct a new VM
    /// with [`Vm::new_with_arena_capacity`] using a larger capacity.
    pub fn new_auto(module: Module) -> Result<Self, VmError> {
        let capacity = auto_arena_capacity_for(&module, &[])?;
        Self::new_with_arena_capacity(module, capacity)
    }

    /// Create a new VM with the given compiled module and a host-specified
    /// arena capacity in bytes.
    ///
    /// Runs structural verification on the module. Returns an error if
    /// verification fails.
    pub fn new_with_arena_capacity(module: Module, arena_capacity: usize) -> Result<Self, VmError> {
        verify::verify(&module)
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        // R31. Verify worst-case memory usage fits within the arena. The
        // check is sound for programs without calls and without
        // variable-iteration loops. See `verify_resource_bounds` for
        // current limitations.
        verify::verify_resource_bounds(&module, arena_capacity)
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        Self::construct(module, arena_capacity)
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
    pub unsafe fn new_unchecked(module: Module) -> Result<Self, VmError> {
        unsafe { Self::new_unchecked_with_arena_capacity(module, DEFAULT_ARENA_CAPACITY) }
    }

    /// Create a VM with a host-specified arena capacity that runs
    /// structural verification but skips resource bounds checks.
    ///
    /// See [`Vm::new_unchecked`] for the trust contract.
    ///
    /// # Safety
    ///
    /// Same contract as [`Vm::new_unchecked`].
    pub unsafe fn new_unchecked_with_arena_capacity(
        module: Module,
        arena_capacity: usize,
    ) -> Result<Self, VmError> {
        verify::verify(&module)
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        Self::construct(module, arena_capacity)
    }

    /// Load and verify a module from a serialized byte slice.
    ///
    /// Convenience wrapper. Equivalent to
    /// `Vm::new(Module::from_bytes(bytes)?)`. The byte slice may
    /// originate from any addressable buffer including a file read,
    /// an in-memory `Vec<u8>`, or a `&'static [u8]` placed in
    /// `.rodata`. Runs full verification including resource bounds.
    pub fn load_bytes(bytes: &[u8]) -> Result<Self, VmError> {
        let module = Module::from_bytes(bytes)?;
        Self::new(module)
    }

    /// Load a module from a serialized byte slice and skip resource
    /// bounds verification.
    ///
    /// Convenience wrapper. Equivalent to
    /// `Vm::new_unchecked(Module::from_bytes(bytes)?)`. See
    /// [`Vm::new_unchecked`] for the trust contract.
    ///
    /// # Safety
    ///
    /// Same contract as [`Vm::new_unchecked`].
    pub unsafe fn load_bytes_unchecked(bytes: &[u8]) -> Result<Self, VmError> {
        let module = Module::from_bytes(bytes)?;
        unsafe { Self::new_unchecked(module) }
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
    pub fn view_bytes(bytes: &[u8]) -> Result<Self, VmError> {
        let module = Module::view_bytes(bytes)?;
        Self::new(module)
    }

    /// Load a module from an aligned byte slice and skip resource
    /// bounds verification.
    ///
    /// Convenience wrapper. Equivalent to
    /// `Vm::new_unchecked(Module::view_bytes(bytes)?)`. See
    /// [`Vm::new_unchecked`] for the trust contract and
    /// [`Vm::view_bytes`] for the alignment contract.
    ///
    /// # Safety
    ///
    /// Same contract as [`Vm::new_unchecked`].
    pub unsafe fn view_bytes_unchecked(bytes: &[u8]) -> Result<Self, VmError> {
        let module = Module::view_bytes(bytes)?;
        unsafe { Self::new_unchecked(module) }
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
    pub unsafe fn view_bytes_zero_copy(bytes: &'a [u8]) -> Result<Self, VmError> {
        // Framing validation only (magic, length, CRC, version, sizes).
        // No rkyv structural validation, no execution-side verification.
        let _ = Module::access_bytes(bytes)?;
        // Determine data segment length from the archived module so the
        // data vector has the right slot count.
        let data_len = {
            let length = u32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]) as usize;
            let body = &bytes[16..length - 4];
            let archived =
                unsafe { rkyv::access_unchecked::<crate::bytecode::ArchivedModule>(body) };
            archived.data_layout.as_ref().map_or(0, |dl| dl.slots.len())
        };
        let data = vec![Value::Unit; data_len];
        Ok(Self {
            bytecode: BytecodeStore::Borrowed(bytes),
            stack: Vec::new(),
            frames: Vec::new(),
            natives: Vec::new(),
            data,
            arena: keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY),
            started: false,
        })
    }

    /// Construct the VM struct without running any verification.
    ///
    /// Internal helper shared by the verifying and unchecked
    /// constructors. Serializes the owned module to an aligned vector
    /// for archived access during execution. The data segment is
    /// initialized to `Unit` for each declared slot.
    fn construct(module: Module, arena_capacity: usize) -> Result<Self, VmError> {
        let data_len = module.data_layout.as_ref().map_or(0, |dl| dl.slots.len());
        let data = vec![Value::Unit; data_len];
        let bytes = module.to_bytes()?;
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        Ok(Self {
            bytecode: BytecodeStore::Owned(aligned),
            stack: Vec::new(),
            frames: Vec::new(),
            natives: Vec::new(),
            data,
            arena: keleusma_arena::Arena::with_capacity(arena_capacity),
            started: false,
        })
    }

    /// Set a data segment slot to an initial value.
    ///
    /// The host calls this before execution begins to populate the
    /// persistent context. Returns an error if the slot index is out
    /// of bounds.
    pub fn set_data(&mut self, slot: usize, value: Value) -> Result<(), VmError> {
        if slot >= self.data.len() {
            return Err(VmError::NativeError(format!(
                "data slot index {} out of bounds (data segment has {} slots)",
                slot,
                self.data.len()
            )));
        }
        self.data[slot] = value;
        Ok(())
    }

    /// Read a data segment slot value.
    ///
    /// Returns an error if the slot index is out of bounds.
    pub fn get_data(&self, slot: usize) -> Result<&Value, VmError> {
        self.data.get(slot).ok_or_else(|| {
            VmError::NativeError(format!(
                "data slot index {} out of bounds (data segment has {} slots)",
                slot,
                self.data.len()
            ))
        })
    }

    /// Return the number of slots in the current data segment.
    ///
    /// Useful for hosts that want to allocate a `Vec<Value>` of the correct
    /// size without inspecting the `Module` directly.
    pub fn data_len(&self) -> usize {
        self.data.len()
    }

    /// Borrow the VM's arena.
    ///
    /// The arena is the dual-end bump-allocated buffer described in R32. It
    /// is available to host-supplied native functions that wish to allocate
    /// dynamic strings or other arena-resident values. The arena is reset
    /// at every `Op::Reset` boundary, so host-allocated values do not
    /// survive across stream phases.
    pub fn arena(&self) -> &keleusma_arena::Arena {
        &self.arena
    }

    /// Mutable borrow of the VM's arena.
    pub fn arena_mut(&mut self) -> &mut keleusma_arena::Arena {
        &mut self.arena
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
        verify::verify(&new_module)
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;
        // R31. Verify the new module's WCMU fits the existing arena.
        verify::verify_resource_bounds(&new_module, self.arena.capacity())
            .map_err(|e| VmError::VerifyError(format!("{}: {}", e.chunk_name, e.message)))?;

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

        // Serialize the new module to aligned bytes for archived
        // access. The borrowed variant is replaced by an owned variant
        // because hot swap takes an owned input.
        let bytes = new_module.to_bytes()?;
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        self.bytecode = BytecodeStore::Owned(aligned);
        self.data = initial_data;
        self.frames.clear();
        self.stack.clear();
        self.arena.reset();
        self.started = false;

        Ok(())
    }

    /// Register a native function by name using a function pointer.
    pub fn register_native(&mut self, name: &str, func: fn(&[Value]) -> Result<Value, VmError>) {
        self.natives.push(NativeEntry {
            wcet: DEFAULT_NATIVE_WCET,
            wcmu_bytes: DEFAULT_NATIVE_WCMU_BYTES,
            name: String::from(name),
            func: Box::new(func),
        });
    }

    /// Register a native function by name using a closure.
    ///
    /// This allows closures that capture state, such as a shared command
    /// buffer for audio script integration.
    pub fn register_native_closure<F>(&mut self, name: &str, func: F)
    where
        F: Fn(&[Value]) -> Result<Value, VmError> + 'static,
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

        if args.len() > local_count {
            return Err(VmError::InvalidBytecode(format!(
                "too many arguments: expected at most {}, got {}",
                local_count,
                args.len()
            )));
        }

        let base = self.stack.len();
        // Push arguments as the first local slots.
        for arg in args {
            self.stack.push(arg.clone());
        }
        // Extend stack for remaining local slots.
        let extra = local_count - args.len();
        for _ in 0..extra {
            self.stack.push(Value::Unit);
        }

        self.frames.push(CallFrame {
            chunk_idx,
            ip: 0,
            base,
        });
        self.started = true;

        self.run()
    }

    /// Resume execution after a yield or reset, providing the input value.
    pub fn resume(&mut self, input: Value) -> Result<VmState, VmError> {
        if !self.started || self.frames.is_empty() {
            return Err(VmError::InvalidBytecode(String::from(
                "cannot resume: VM not suspended",
            )));
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
                let base = base_frame.base;
                self.stack[base] = input.clone();
            }
        }
        // Push the input value onto the stack (it becomes the yield expression result).
        self.stack.push(input);
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
                self.stack.push(result);
                continue;
            }

            let op = self.chunk_op(chunk_idx, ip);
            // Advance IP.
            self.frames.last_mut().unwrap().ip += 1;

            match op {
                Op::Const(idx) => {
                    let val = self.chunk_const(chunk_idx, idx as usize);
                    self.stack.push(val);
                }
                Op::PushUnit => self.stack.push(Value::Unit),
                Op::PushTrue => self.stack.push(Value::Bool(true)),
                Op::PushFalse => self.stack.push(Value::Bool(false)),

                Op::GetLocal(slot) => {
                    let val = self.stack[base + slot as usize].clone();
                    self.stack.push(val);
                }
                Op::SetLocal(slot) => {
                    let val = self.pop()?;
                    self.stack[base + slot as usize] = val;
                }

                Op::GetData(slot) => {
                    let idx = slot as usize;
                    if idx >= self.data.len() {
                        return Err(VmError::InvalidBytecode(format!(
                            "data slot index {} out of bounds",
                            idx
                        )));
                    }
                    let val = self.data[idx].clone();
                    self.stack.push(val);
                }
                Op::SetData(slot) => {
                    let idx = slot as usize;
                    if idx >= self.data.len() {
                        return Err(VmError::InvalidBytecode(format!(
                            "data slot index {} out of bounds",
                            idx
                        )));
                    }
                    let val = self.pop()?;
                    self.data[idx] = val;
                }

                Op::Add => {
                    let word_bits_log2 = self.word_bits_log2();
                    self.binary_op(move |a, b| match (a, b) {
                        (Value::Int(x), Value::Int(y)) => Ok(Value::Int(
                            crate::bytecode::truncate_int(x.wrapping_add(y), word_bits_log2),
                        )),
                        (Value::Float(x), Value::Float(y)) => Ok(Value::Float(x + y)),
                        (a, b) if a.as_str().is_some() && b.as_str().is_some() => {
                            let mut s = match a {
                                Value::StaticStr(s) | Value::DynStr(s) => s,
                                _ => unreachable!(),
                            };
                            let suffix = match b {
                                Value::StaticStr(s) | Value::DynStr(s) => s,
                                _ => unreachable!(),
                            };
                            s.push_str(&suffix);
                            Ok(Value::DynStr(s))
                        }
                        (a, b) => Err(VmError::TypeError(format!(
                            "cannot add {} and {}",
                            a.type_name(),
                            b.type_name()
                        ))),
                    })?
                }
                Op::Sub => self.binary_arith(|a, b| a.wrapping_sub(b), |a, b| a - b)?,
                Op::Mul => self.binary_arith(|a, b| a.wrapping_mul(b), |a, b| a * b)?,
                Op::Div => {
                    let word_bits_log2 = self.word_bits_log2();
                    let b = self.pop()?;
                    let a = self.pop()?;
                    match (a, b) {
                        (Value::Int(_), Value::Int(0)) => return Err(VmError::DivisionByZero),
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(
                            crate::bytecode::truncate_int(x.wrapping_div(y), word_bits_log2),
                        )),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x / y)),
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
                        (Value::Int(x), Value::Int(y)) => self.stack.push(Value::Int(
                            crate::bytecode::truncate_int(x.wrapping_rem(y), word_bits_log2),
                        )),
                        (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(x % y)),
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
                        Value::Int(x) => self.stack.push(Value::Int(
                            crate::bytecode::truncate_int(x.wrapping_neg(), word_bits_log2),
                        )),
                        Value::Float(x) => self.stack.push(Value::Float(-x)),
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
                    self.stack.push(Value::Bool(a == b));
                }
                Op::CmpNe => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.stack.push(Value::Bool(a != b));
                }
                Op::CmpLt => self.compare_op(|ord| ord.is_lt())?,
                Op::CmpGt => self.compare_op(|ord| ord.is_gt())?,
                Op::CmpLe => self.compare_op(|ord| ord.is_le())?,
                Op::CmpGe => self.compare_op(|ord| ord.is_ge())?,

                Op::Not => {
                    let val = self.pop()?;
                    match val {
                        Value::Bool(b) => self.stack.push(Value::Bool(!b)),
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
                    self.arena.reset();

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
                        self.stack.push(Value::Unit);
                    }
                    self.frames.push(CallFrame {
                        chunk_idx: idx as usize,
                        ip: 0,
                        base: new_base,
                    });
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
                    let result = (entry.func)(&args)?;
                    self.stack.push(result);
                }
                Op::Return => {
                    let result = self.pop()?;
                    let old_frame = self.frames.pop().unwrap();
                    self.stack.truncate(old_frame.base);
                    if self.frames.is_empty() {
                        return Ok(VmState::Finished(result));
                    }
                    self.stack.push(result);
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
                    self.stack.push(val);
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
                    self.stack.push(Value::Struct { type_name, fields });
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
                    self.stack.push(Value::Enum {
                        type_name,
                        variant,
                        fields,
                    });
                }
                Op::NewArray(count) => {
                    let n = count as usize;
                    let elements: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    self.stack.push(Value::Array(elements));
                }
                Op::NewTuple(count) => {
                    let n = count as usize;
                    let elements: Vec<Value> = self.stack.drain(self.stack.len() - n..).collect();
                    self.stack.push(Value::Tuple(elements));
                }
                Op::WrapSome => {
                    // In our representation, Some(v) is just v. None is Value::None.
                    // WrapSome is a no-op for the value itself.
                }
                Op::PushNone => {
                    self.stack.push(Value::None);
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
                            self.stack.push(val);
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
                            self.stack.push(arr[i as usize].clone());
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
                            self.stack.push(elems[i].clone());
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
                            self.stack.push(fields[i].clone());
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
                            self.stack.push(Value::Int(arr.len() as i64));
                        }
                        Value::StaticStr(s) | Value::DynStr(s) => {
                            self.stack.push(Value::Int(s.chars().count() as i64));
                        }
                        Value::Tuple(t) => {
                            self.stack.push(Value::Int(t.len() as i64));
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
                    self.stack.push(Value::Bool(matches));
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
                    self.stack.push(Value::Bool(matches));
                }

                Op::IntToFloat => {
                    let val = self.pop()?;
                    match val {
                        Value::Int(i) => self.stack.push(Value::Float(i as f64)),
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to f64",
                                v.type_name()
                            )));
                        }
                    }
                }
                Op::FloatToInt => {
                    let val = self.pop()?;
                    match val {
                        Value::Float(f) => self.stack.push(Value::Int(f as i64)),
                        v => {
                            return Err(VmError::TypeError(format!(
                                "cannot cast {} to i64",
                                v.type_name()
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
            }
        }
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        self.stack.pop().ok_or(VmError::StackUnderflow)
    }

    fn binary_op<F>(&mut self, f: F) -> Result<(), VmError>
    where
        F: FnOnce(Value, Value) -> Result<Value, VmError>,
    {
        let b = self.pop()?;
        let a = self.pop()?;
        let result = f(a, b)?;
        self.stack.push(result);
        Ok(())
    }

    fn binary_arith(
        &mut self,
        int_op: fn(i64, i64) -> i64,
        float_op: fn(f64, f64) -> f64,
    ) -> Result<(), VmError> {
        let word_bits_log2 = self.word_bits_log2();
        let b = self.pop()?;
        let a = self.pop()?;
        match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                let result = crate::bytecode::truncate_int(int_op(x, y), word_bits_log2);
                self.stack.push(Value::Int(result));
            }
            (Value::Float(x), Value::Float(y)) => self.stack.push(Value::Float(float_op(x, y))),
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
            (Value::Float(x), Value::Float(y)) => {
                x.partial_cmp(y).unwrap_or(core::cmp::Ordering::Equal)
            }
            (Value::StaticStr(x) | Value::DynStr(x), Value::StaticStr(y) | Value::DynStr(y)) => {
                x.cmp(y)
            }
            _ => {
                return Err(VmError::TypeError(format!(
                    "cannot compare {} and {}",
                    a.type_name(),
                    b.type_name()
                )));
            }
        };
        self.stack.push(Value::Bool(pred(ord)));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn run_program(src: &str, args: &[Value]) -> Result<VmState, VmError> {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module)?;
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
    fn eval_literal() {
        let val = run_expect("fn main() -> i64 { 42 }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_add() {
        let val = run_expect("fn main() -> i64 { 10 + 32 }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_arithmetic() {
        let val = run_expect("fn main() -> i64 { (2 + 3) * 4 - 1 }", &[]);
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
        let val = run_expect("fn main() -> i64 { -42 }", &[]);
        assert_eq!(val, Value::Int(-42));
    }

    #[test]
    fn eval_not() {
        let val = run_expect("fn main() -> bool { not true }", &[]);
        assert_eq!(val, Value::Bool(false));
    }

    #[test]
    fn eval_if_true() {
        let val = run_expect("fn main() -> i64 { if true { 1 } else { 2 } }", &[]);
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn eval_if_false() {
        let val = run_expect("fn main() -> i64 { if false { 1 } else { 2 } }", &[]);
        assert_eq!(val, Value::Int(2));
    }

    #[test]
    fn eval_let_binding() {
        let val = run_expect("fn main() -> i64 { let x = 10; let y = 32; x + y }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_function_call() {
        let val = run_expect(
            "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(21) }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_nested_calls() {
        let val = run_expect(
            "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(double(10)) + 2 }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_with_args() {
        let val = run_expect("fn main(x: i64) -> i64 { x + 1 }", &[Value::Int(41)]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_for_range() {
        let val = run_expect(
            "fn main() -> i64 { let sum = 0; for i in 0..5 { let x = sum + i; } sum }",
            &[],
        );
        // Lexical scoping: inner `let x` shadows but does not mutate outer `sum`.
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_string_literal() {
        let val = run_expect("fn main() -> String { \"hello\" }", &[]);
        assert_eq!(val, Value::StaticStr(String::from("hello")));
    }

    #[test]
    fn eval_float_arithmetic() {
        let val = run_expect("fn main() -> f64 { 1.5 + 2.5 }", &[]);
        assert_eq!(val, Value::Float(4.0));
    }

    #[test]
    fn eval_cast_int_to_float() {
        let val = run_expect("fn main() -> f64 { 42 as f64 }", &[]);
        assert_eq!(val, Value::Float(42.0));
    }

    #[test]
    fn eval_cast_float_to_int() {
        let val = run_expect("fn main() -> i64 { 3.7 as i64 }", &[]);
        assert_eq!(val, Value::Int(3));
    }

    #[test]
    fn eval_struct_init_and_field() {
        let val = run_expect(
            "struct Point { x: i64, y: i64 }\nfn main() -> i64 { let p = Point { x: 10, y: 32 }; p.x + p.y }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_enum_variant() {
        let val = run_expect(
            "enum Color { Red, Green, Blue }\nfn main() -> i64 { let c = Color::Red(); 42 }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_array_literal_and_index() {
        let val = run_expect("fn main() -> i64 { let arr = [10, 20, 30]; arr[1] }", &[]);
        assert_eq!(val, Value::Int(20));
    }

    #[test]
    fn eval_yield_and_resume() {
        let src = "loop main(input: i64) -> i64 { let input = yield input * 2; input }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();

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
    fn eval_multiheaded_literal() {
        let val = run_expect(
            "fn classify(0) -> String { \"zero\" }\nfn classify(x: i64) -> String { \"other\" }\nfn main() -> String { classify(0) }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("zero")));
    }

    #[test]
    fn eval_multiheaded_fallthrough() {
        let val = run_expect(
            "fn classify(0) -> String { \"zero\" }\nfn classify(x: i64) -> String { \"other\" }\nfn main() -> String { classify(5) }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("other")));
    }

    #[test]
    fn eval_pipeline() {
        let val = run_expect(
            "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { 21 |> double() }",
            &[],
        );
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_match_literal() {
        let val = run_expect(
            "fn main() -> String { let x = 1; match x { 1 => \"one\", 2 => \"two\", _ => \"other\" } }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("one")));
    }

    #[test]
    fn eval_match_wildcard() {
        let val = run_expect(
            "fn main() -> String { let x = 99; match x { 1 => \"one\", _ => \"other\" } }",
            &[],
        );
        assert_eq!(val, Value::StaticStr(String::from("other")));
    }

    #[test]
    fn eval_division_by_zero() {
        let result = run_program("fn main() -> i64 { 1 / 0 }", &[]);
        assert!(matches!(result, Err(VmError::DivisionByZero)));
    }

    #[test]
    fn eval_index_out_of_bounds() {
        let result = run_program("fn main() -> i64 { let a = [1, 2]; a[5] }", &[]);
        assert!(matches!(result, Err(VmError::IndexOutOfBounds(5, 2))));
    }

    #[test]
    fn eval_native_function() {
        let src = "use math::add_one\nfn main(x: i64) -> i64 { math::add_one(x) }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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
            "fn abs(x: i64) -> i64 when x < 0 { -x }\nfn abs(x: i64) -> i64 { x }\nfn main() -> i64 { abs(-5) + abs(3) }",
            &[],
        );
        assert_eq!(val, Value::Int(8));
    }

    #[test]
    fn eval_string_concat() {
        let val = run_expect("fn main() -> String { \"hello\" + \" world\" }", &[]);
        assert_eq!(val, Value::DynStr(String::from("hello world")));
    }

    // -- For-in over array expressions --

    #[test]
    fn eval_for_in_array_literal() {
        let val = run_expect(
            "fn main() -> i64 { let sum = 0; for x in [10, 20, 30] { let sum = sum + x; } sum }",
            &[],
        );
        // Lexical scoping: inner `let sum` shadows but does not mutate outer `sum`.
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_for_in_array_accumulate() {
        // Use a mutable-style accumulation pattern via function calls.
        let val = run_expect(
            "fn main() -> i64 {\n\
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
            "fn main() -> i64 { let count = 42; for x in [] { let count = 0; } count }",
            &[],
        );
        // Body never executes for empty array.
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_for_in_single_element() {
        let val = run_expect(
            "fn main() -> i64 { let last = 0; for x in [99] { let last = x; } last }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    #[test]
    fn eval_for_in_array_with_function() {
        let val = run_expect(
            "fn double(x: i64) -> i64 { x * 2 }\n\
             fn main() -> i64 {\n\
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
        let val = run_expect("fn main() -> i64 { let t = (1, 2, 3); t.0 }", &[]);
        assert_eq!(val, Value::Int(1));
    }

    #[test]
    fn eval_tuple_field_access() {
        let val = run_expect("fn main() -> i64 { let t = (10, 20, 30); t.1 }", &[]);
        assert_eq!(val, Value::Int(20));
    }

    #[test]
    fn eval_tuple_let_destructure() {
        let val = run_expect("fn main() -> i64 { let (a, b) = (10, 32); a + b }", &[]);
        assert_eq!(val, Value::Int(42));
    }

    #[test]
    fn eval_tuple_mixed_types() {
        let val = run_expect("fn main() -> f64 { let t = (42, 2.5, true); t.1 }", &[]);
        assert_eq!(val, Value::Float(2.5));
    }

    // -- Len instruction --

    #[test]
    fn eval_len_via_for_in() {
        // Len is used internally by for-in. Verify via a known array size.
        let val = run_expect(
            "fn main() -> i64 { let n = 0; for x in [1, 1, 1, 1] { let n = n + 1; } n }",
            &[],
        );
        assert_eq!(val, Value::Int(0));
    }

    // -- Data segment --

    #[test]
    fn eval_data_read() {
        // Read a host-initialized data slot from script.
        let src = "data ctx {\n    score: i64,\n}\nfn main() -> i64 { ctx.score }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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
                score: i64,\n\
            }\n\
            fn main() -> i64 {\n\
                ctx.score = 100;\n\
                ctx.score\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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
                counter: i64,\n\
            }\n\
            loop main(input: i64) -> i64 {\n\
                ctx.counter = ctx.counter + 1;\n\
                let input = yield ctx.counter;\n\
                input\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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
                value: i64,\n\
            }\n\
            loop main(input: i64) -> i64 {\n\
                ctx.value = 99;\n\
                let input = yield ctx.value;\n\
                let input = yield ctx.value + 1;\n\
                input\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();

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
                a: i64,\n\
                b: i64,\n\
                c: i64,\n\
            }\n\
            fn main() -> i64 {\n\
                ctx.a = 10;\n\
                ctx.b = 20;\n\
                ctx.c = 30;\n\
                ctx.a + ctx.b + ctx.c\n\
            }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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
                x: i64,\n\
                y: i64,\n\
            }\n\
            fn main() -> i64 { ctx.x + ctx.y }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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
        let src_a = "data ctx { score: i64 }\nfn main() -> i64 { ctx.score + 10 }";
        // Module B: ctx { score: i64 }, returns ctx.score * 2.
        let src_b = "data ctx { score: i64 }\nfn main() -> i64 { ctx.score * 2 }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let mut vm = Vm::new(mod_a).unwrap();
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
    fn hot_swap_new_schema_replaced() {
        // Module A: ctx { score: i64 }, returns ctx.score.
        let src_a = "data ctx { score: i64 }\nfn main() -> i64 { ctx.score }";
        // Module B: ctx { x: i64, y: i64, z: i64 }, returns x + y + z.
        let src_b =
            "data ctx { x: i64, y: i64, z: i64 }\nfn main() -> i64 { ctx.x + ctx.y + ctx.z }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let mut vm = Vm::new(mod_a).unwrap();
        vm.set_data(0, Value::Int(7)).unwrap();
        assert_eq!(vm.data_len(), 1);

        vm.replace_module(
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
        let src_a = "data ctx { x: i64 }\nfn main() -> i64 { ctx.x }";
        let src_b = "data ctx { x: i64, y: i64 }\nfn main() -> i64 { ctx.x + ctx.y }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let mut vm = Vm::new(mod_a).unwrap();
        // Supplying one value when the new module declares two slots must fail.
        let err = vm
            .replace_module(mod_b, alloc::vec![Value::Int(99)])
            .unwrap_err();
        match err {
            VmError::InvalidBytecode(msg) => assert!(msg.contains("size mismatch")),
            other => panic!("expected size mismatch error, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_no_data_module_accepts_empty_vec() {
        let src_a = "data ctx { x: i64 }\nfn main() -> i64 { ctx.x }";
        let src_b = "fn main() -> i64 { 42 }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let mut vm = Vm::new(mod_a).unwrap();
        vm.replace_module(mod_b, Vec::new()).unwrap();
        assert_eq!(vm.data_len(), 0);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn hot_swap_at_reset_starts_new_module() {
        // Module A: streaming counter. Module B: streaming doubler.
        let src_a = "data ctx { n: i64 }\n\
                     loop main(input: i64) -> i64 {\n\
                         ctx.n = ctx.n + 1;\n\
                         let input = yield ctx.n;\n\
                         input\n\
                     }";
        let src_b = "data ctx { n: i64 }\n\
                     loop main(input: i64) -> i64 {\n\
                         let input = yield ctx.n * 10;\n\
                         input\n\
                     }";

        let mod_a = build_module(src_a);
        let mod_b = build_module(src_b);

        let mut vm = Vm::new(mod_a).unwrap();
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
        let src_v1 = "data ctx { n: i64 }\nfn main() -> i64 { ctx.n + 1 }";
        let src_v2 = "data ctx { n: i64 }\nfn main() -> i64 { ctx.n + 100 }";

        let mod_v1 = build_module(src_v1);
        let mod_v2 = build_module(src_v2);

        // Start with v1, snapshot the value 5.
        let mut vm = Vm::new(mod_v1.clone()).unwrap();
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
        let src = "loop main(input: i64) -> String { let input = yield \"static\"; \"static\" }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(v) => assert_eq!(v, Value::StaticStr(String::from("static"))),
            other => panic!("expected yield, got {:?}", other),
        }
    }

    #[test]
    fn yield_dynamic_string_fails() {
        // to_string returns a DynStr. Yielding it must fail at runtime.
        let src = "use to_string\n\
                   loop main(input: i64) -> String { \
                       let input = yield to_string(input); \"done\" }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
        crate::utility_natives::register_utility_natives(&mut vm);
        let err = vm.call(&[Value::Int(42)]).unwrap_err();
        match err {
            VmError::TypeError(msg) => {
                assert!(msg.contains("dynamic string") || msg.contains("DynStr"))
            }
            other => panic!("expected TypeError, got {:?}", other),
        }
    }

    #[test]
    fn yield_tuple_with_dynamic_string_fails() {
        // Yielding a tuple containing a DynStr must fail.
        let src = "use to_string\n\
                   loop main(input: i64) -> (i64, String) { \
                       let input = yield (input, to_string(input)); (0, \"\") }";
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module).unwrap();
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
        let module = build_module("fn main() -> i64 { 42 }");
        let vm = Vm::new(module).unwrap();
        assert_eq!(vm.arena().capacity(), DEFAULT_ARENA_CAPACITY);
        assert_eq!(vm.arena().bottom_used(), 0);
        assert_eq!(vm.arena().top_used(), 0);
    }

    #[test]
    fn vm_arena_capacity_configurable() {
        let module = build_module("fn main() -> i64 { 42 }");
        let vm = Vm::new_with_arena_capacity(module, 4096).unwrap();
        assert_eq!(vm.arena().capacity(), 4096);
    }

    #[test]
    fn vm_arena_reset_at_op_reset() {
        // Stream function that allocates from arena via the arena_mut
        // accessor before yield. The arena is not reset at yield, but is
        // reset at the Op::Reset boundary.
        use core::alloc::Layout;
        use keleusma_arena::Arena;

        let src = "loop main(input: i64) -> i64 { let input = yield input; input }";
        let module = build_module(src);
        let mut vm = Vm::new(module).unwrap();

        // Host allocates from arena before first call.
        {
            let layout = Layout::new::<u64>();
            let handle = vm.arena().bottom_handle();
            let _p = allocator_api2::alloc::Allocator::allocate(&handle, layout).unwrap();
        }
        assert!(vm.arena().bottom_used() > 0);
        let _ = &vm; // fence for readability

        // First call yields, arena not reset at yield.
        match vm.call(&[Value::Int(0)]).unwrap() {
            VmState::Yielded(_) => {}
            other => panic!("expected yield, got {:?}", other),
        }
        assert!(vm.arena().bottom_used() > 0);

        // Resume to reach Reset. Arena is reset.
        match vm.resume(Value::Int(0)).unwrap() {
            VmState::Reset => {}
            other => panic!("expected reset, got {:?}", other),
        }
        assert_eq!(vm.arena().bottom_used(), 0);
        assert_eq!(vm.arena().top_used(), 0);

        // Suppress unused import in this nested context.
        let _: fn(usize) -> Arena = Arena::with_capacity;
    }

    #[test]
    fn bytecode_roundtrip() {
        let src = "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(21) }";
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
        let mut vm = Vm::new(decoded).unwrap();
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_load_bytes_end_to_end() {
        let src = "fn main() -> i64 { 7 + 35 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut vm = Vm::load_bytes(&bytes).expect("load");
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => assert_eq!(v, Value::Int(42)),
            other => panic!("expected finished, got {:?}", other),
        }
    }

    #[test]
    fn bytecode_rejects_bad_magic() {
        // Pad to the minimum framing length (header 16 + footer 4 = 20)
        // so the slice passes the truncation check and reaches the
        // magic check.
        let bytes = alloc::vec![
            b'X', b'X', b'X', b'X', // magic
            0x04, 0x00, // version
            0x14, 0x00, 0x00, 0x00, // length = 20
            6, 6, // word_bits_log2, addr_bits_log2
            0x00, 0x00, 0x00, 0x00, // reserved
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
        // Source: `fn main() -> i64 { 1 }`
        //
        // Layout breakdown:
        //   bytes[0..4]    = b"KELE"               magic
        //   bytes[4..6]    = 0x04 0x00              version 4 (u16 LE)
        //   bytes[6..10]   = 0x90 0x00 0x00 0x00    length 144 (u32 LE)
        //   bytes[10]      = 0x06                   word_bits_log2 = 6 (64-bit)
        //   bytes[11]      = 0x06                   addr_bits_log2 = 6 (64-bit)
        //   bytes[12..16]  = 0x00 0x00 0x00 0x00    reserved
        //   bytes[16..140] = rkyv body
        //   bytes[140..144] = 0xB9 0x9D 0x13 0xFB   CRC-32 (u32 LE)
        let expected: alloc::vec::Vec<u8> = alloc::vec![
            0x4B, 0x45, 0x4C, 0x45, 0x04, 0x00, 0x90, 0x00, 0x00, 0x00, 0x06, 0x06, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6D, 0x61, 0x69, 0x6E, 0xFF, 0xFF,
            0xFF, 0xFF, 0xC8, 0xFF, 0xFF, 0xFF, 0x02, 0x00, 0x00, 0x00, 0xD0, 0xFF, 0xFF, 0xFF,
            0x01, 0x00, 0x00, 0x00, 0xE8, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0xDC, 0xFF, 0xFF, 0xFF, 0x01, 0x00, 0x00, 0x00, 0xF8, 0xFF, 0xFF, 0xFF,
            0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x06, 0x00, 0x00,
            0xB9, 0x9D, 0x13, 0xFB,
        ];
        let src = "fn main() -> i64 { 1 }";
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
        let mut vm = Vm::load_bytes(&expected).expect("load");
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
        let src = "fn main() -> i64 { 7 + 35 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        let mut vm = Vm::view_bytes(&aligned).expect("view");
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
        let src = "fn main() -> i64 { 1 }";
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
        let src = "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(21) }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let bytes = module.to_bytes().expect("encode");
        let mut aligned = rkyv::util::AlignedVec::<8>::with_capacity(bytes.len());
        aligned.extend_from_slice(&bytes);
        // Construct a VM that borrows from `aligned`. The lifetime
        // parameter on Vm is tied to the slice.
        let mut vm: Vm<'_> = unsafe { Vm::view_bytes_zero_copy(&aligned[..]).expect("view") };
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
        let src = "fn main() -> i64 { 1 + 2 }";
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
        let src = "fn main() -> i64 { 42 }";
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
            let original = &module.chunks[0].constants[i];
            assert_eq!(
                &owned, original,
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
        let src = "fn double(x: i64) -> i64 { x * 2 }\nfn main() -> i64 { double(21) }";
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
        let src = "fn main() -> i64 { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        bytes.extend_from_slice(&[0xAA; 32]);
        let mut vm = Module::from_bytes(&bytes)
            .map(Vm::new)
            .expect("decode")
            .expect("verify");
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
        let src = "fn main() -> i64 { 1 }";
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
        let src = "fn main() -> i64 { 1 }";
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
        let src = "fn main() -> i64 { 1 }";
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
        let src = "fn main() -> i64 { 1 }";
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
        let src = "fn main() -> i64 { 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");
        let mut bytes = module.to_bytes().expect("encode");
        // Declare 32-bit words. Runtime is 64-bit so 5 <= 6 holds.
        bytes[10] = 5;
        let trailer_start = bytes.len() - 4;
        let new_crc = crate::bytecode::crc32(&bytes[..trailer_start]);
        bytes[trailer_start..].copy_from_slice(&new_crc.to_le_bytes());
        let mut vm = Vm::load_bytes(&bytes).expect("narrower bytecode should be admitted");
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
        let src = "fn main() -> i64 { 2147483647 + 1 }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let mut module = compile(&program).expect("compile");
        module.word_bits_log2 = 5;
        let mut vm = Vm::new(module).expect("verify");
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
        match Vm::load_bytes(&bytes) {
            Err(VmError::LoadError(_)) => {}
            Err(other) => panic!("expected VmError::LoadError, got {:?}", other),
            Ok(_) => panic!("expected error, got VM"),
        }
    }

    #[test]
    fn unchecked_admits_module_that_fails_bounds() {
        // A loop main that pushes a value yields a tiny but non-zero
        // worst-case stack usage. With a capacity of 1 byte, the
        // bounds check rejects the module. The unchecked path admits
        // it because it skips the bounds check entirely.
        let src = "loop main() -> i64 { let n = yield 0; n }";
        let tokens = tokenize(src).expect("lex");
        let program = parse(&tokens).expect("parse");
        let module = compile(&program).expect("compile");

        // The verifying constructor rejects.
        let rejected = Vm::new_with_arena_capacity(module.clone(), 1);
        assert!(matches!(rejected, Err(VmError::VerifyError(_))));

        // The unchecked constructor admits the same module under the
        // same tiny capacity. Structural verification still runs.
        let admitted = unsafe { Vm::new_unchecked_with_arena_capacity(module, 1) };
        assert!(admitted.is_ok());
    }

    #[test]
    fn unchecked_still_runs_structural_verification() {
        // Construct a module that fails structural verification by
        // manually corrupting the chunk's block type. A `Stream` chunk
        // without a yield is rejected.
        use crate::bytecode::{BlockType, Chunk, Module, Op};
        let chunk = Chunk {
            name: alloc::string::String::from("main"),
            ops: alloc::vec![Op::Const(0), Op::Reset],
            constants: alloc::vec![Value::Int(0)],
            struct_templates: alloc::vec![],
            local_count: 0,
            param_count: 0,
            block_type: BlockType::Stream,
        };
        let module = Module {
            chunks: alloc::vec![chunk],
            native_names: alloc::vec![],
            entry_point: Some(0),
            data_layout: None,
            word_bits_log2: crate::bytecode::RUNTIME_WORD_BITS_LOG2,
            addr_bits_log2: crate::bytecode::RUNTIME_ADDRESS_BITS_LOG2,
        };
        // The unchecked constructor still rejects on structural grounds.
        let result = unsafe { Vm::new_unchecked(module) };
        assert!(matches!(result, Err(VmError::VerifyError(_))));
    }

    #[test]
    fn contains_dynstr_helper() {
        assert!(!Value::Int(1).contains_dynstr());
        assert!(!Value::StaticStr(String::from("hi")).contains_dynstr());
        assert!(Value::DynStr(String::from("hi")).contains_dynstr());
        assert!(
            Value::Tuple(alloc::vec![Value::Int(1), Value::DynStr(String::from("x"))])
                .contains_dynstr()
        );
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
                fields: alloc::vec![(String::from("x"), Value::DynStr(String::from("y")))],
            }
            .contains_dynstr()
        );
    }
}
