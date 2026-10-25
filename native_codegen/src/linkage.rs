//! Static-library export of one coroutine (V0.3.x Workstream D).
//!
//! [`build`] lowers a verified module through [`crate::coroutine`] at its
//! static frame bound and makes the result a self-describing object. The stable
//! handles are renamed under a prefix the caller chooses, every other definition
//! becomes internal, and the contract a host must honour is added beside the
//! code as exported constants. [`Library::c_header`] and
//! [`Library::rust_bindings`] render that contract for the two host languages
//! from the same values the object carries, so neither can drift from it.
//!
//! # Exported symbols
//!
//! The object defines `<prefix>_start`, `<prefix>_resume` and
//! `<prefix>_release`, and `<prefix>_yield_site` when dialogue contracts were
//! supplied, with the signatures and lifetimes documented in
//! [`crate::coroutine`]. It also defines these constants, each an unsigned
//! 64-bit word unless stated.
//!
//! - `<prefix>_contract_version`, [`CONTRACT_VERSION`].
//! - `<prefix>_frame_bytes`, the static frame bound, and `<prefix>_slot_bytes`,
//!   the slot reservation that bound needs.
//! - `<prefix>_buffer_align`, the alignment of every buffer the host passes.
//! - `<prefix>_shared_bytes`, `<prefix>_private_bytes` and
//!   `<prefix>_region_bytes`, the three ordinary buffers.
//! - `<prefix>_private_init_bytes`, and `<prefix>_private_init` when that is
//!   not zero, the image to copy into the start of a zeroed private buffer
//!   before start.
//! - `<prefix>_trap_verdict`, an unsigned byte in the [`TrapExport`] encoding.
//!
//! The code's own undefined symbols are the host natives, `kel_native_*`, and
//! the compiler runtime and C library routines the code generator calls, which
//! `LINKAGE_SYMBOL_CENSUS.md` records. [`build`] refuses any other import.
//!
//! # Why no execution-time figure is exported
//!
//! The verifier's cost is counted in virtual-machine cost units and does not
//! include script calls, so it bounds neither native time nor the whole program.
//! A symbol carrying it would be read as a bound it is not. Native timing is
//! Workstream E. See `NATIVE_WCET_ASYMMETRY.md`.
//!
//! # Symbol names
//!
//! The prefix is the whole naming scheme. The mangling recommended for V0.4.0,
//! which encodes module path, function and type arguments, is not implemented
//! here. Two libraries link into one program when their prefixes differ, since
//! nothing else they define is external. Natives are named by the host and are
//! shared by every library that calls them.
//!
//! # Trust boundary
//!
//! The figures are sound on the Keleusma side. The host must provision them,
//! honour the lifetimes, and define each native with its declared signature.
//! Nothing in the object can check the host.

use inkwell::context::Context;
use inkwell::module::{Linkage, Module as LlvmModule};
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{FileType, TargetMachine};
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, FunctionType};
use inkwell::values::{BasicValue, FunctionValue};
use keleusma::bytecode::{Module, SHARED_SLOT_COMPOSITE_FLAG, SlotVisibility};
use keleusma::trap_analysis::{TrapVerdict, classify_traps};
use keleusma::value_layout::ScalarKind;

use crate::coroutine::{self, FRAME_ALIGN, HostContracts};
use crate::{LowerError, NATIVE_SYMBOL_PREFIX, region};

/// Version of the exported contract. A change to any exported name, type or
/// meaning increments it.
pub const CONTRACT_VERSION: u64 = 1;

/// Alignment of every buffer and slot the host passes. Private storage is read
/// in words and the slot needs [`FRAME_ALIGN`], so one figure covers all four.
pub const BUFFER_ALIGN: u64 = FRAME_ALIGN as u64;

/// Longest admitted prefix. Linkers accept far longer names; the limit keeps
/// generated identifiers readable and leaves room for the suffixes.
pub const PREFIX_LIMIT: usize = 48;

/// Why a library could not be built or written.
#[derive(Debug)]
pub enum LinkageError {
    /// The prefix is not a C identifier of at most [`PREFIX_LIMIT`] bytes, or
    /// it would place the handles in the native namespace.
    Prefix(String),
    /// The coroutine did not lower.
    Lower(LowerError),
    /// The module has a private initializer this backend cannot place.
    PrivateImage,
    /// The shared layout cannot be rendered as named fields.
    Layout(String),
    /// A handle or native has a parameter type with no C rendering here.
    Signature(String),
    /// The code imports a symbol outside the documented set.
    Import(String),
    /// LLVM refused the module or the object could not be written.
    Emit(String),
}

impl From<LowerError> for LinkageError {
    fn from(e: LowerError) -> Self {
        Self::Lower(e)
    }
}

/// A scalar crossing the boundary, as LLVM declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CType {
    /// `i1`.
    Bool,
    /// `i8`, a Keleusma `Byte`.
    U8,
    /// `i16`.
    I16,
    /// `i32`.
    I32,
    /// `i64`, including a flat body passed as an address.
    I64,
    /// `float`.
    F32,
    /// `double`.
    F64,
    /// `ptr`.
    Ptr,
}

impl CType {
    fn of_basic(t: BasicTypeEnum<'_>) -> Option<Self> {
        match t {
            BasicTypeEnum::IntType(i) => match i.get_bit_width() {
                1 => Some(Self::Bool),
                8 => Some(Self::U8),
                16 => Some(Self::I16),
                32 => Some(Self::I32),
                64 => Some(Self::I64),
                _ => None,
            },
            BasicTypeEnum::FloatType(f) => match f.get_bit_width() {
                32 => Some(Self::F32),
                64 => Some(Self::F64),
                _ => None,
            },
            BasicTypeEnum::PointerType(_) => Some(Self::Ptr),
            _ => None,
        }
    }

    fn of_param(t: BasicMetadataTypeEnum<'_>) -> Option<Self> {
        BasicTypeEnum::try_from(t).ok().and_then(Self::of_basic)
    }

    /// The C spelling.
    pub fn c(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U8 => "uint8_t",
            Self::I16 => "int16_t",
            Self::I32 => "int32_t",
            Self::I64 => "int64_t",
            Self::F32 => "float",
            Self::F64 => "double",
            Self::Ptr => "unsigned char *",
        }
    }

    /// The Rust spelling.
    pub fn rust(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Ptr => "*mut u8",
        }
    }
}

/// The trap classification carried by `<prefix>_trap_verdict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrapExport {
    /// Encoded 0. The first-pass analysis did not reach a verdict. The reason
    /// is kept for the generated comments.
    Unestablished(String),
    /// Encoded 1. No supported instruction can raise a language trap.
    TrapFree,
    /// Encoded 2. An explicit `Trap` occurs at this chunk and instruction.
    MayTrap(usize, usize),
}

impl TrapExport {
    /// The exported byte.
    pub fn code(&self) -> u8 {
        match self {
            Self::Unestablished(_) => 0,
            Self::TrapFree => 1,
            Self::MayTrap(..) => 2,
        }
    }

    fn describe(&self) -> String {
        match self {
            Self::Unestablished(why) => format!("0, not established: {why}"),
            Self::TrapFree => String::from("1, trap free"),
            Self::MayTrap(c, i) => format!("2, may trap, first explicit Trap at chunk {c} op {i}"),
        }
    }
}

/// One named run of shared slots. An array is one field with a count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedField {
    /// The slot name as the module declares it.
    pub name: String,
    /// The name rendered as an upper-case identifier.
    pub ident: String,
    /// Byte offset of the first element.
    pub offset: u64,
    /// Bytes per element.
    pub element_bytes: u64,
    /// Number of contiguous elements.
    pub count: u64,
    /// The slot kind byte from the module's layout.
    pub kind: u8,
}

/// A native the host must define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeImport {
    /// The linker symbol.
    pub symbol: String,
    /// Parameter types in order.
    pub params: Vec<CType>,
    /// Return type, `None` for void. For a fallible native this is the type
    /// of the value word.
    pub ret: Option<CType>,
    /// Whether the native returns `kel_native_result`, a value word and a
    /// failure word, as `HostContracts::fallible_natives` declares.
    pub fallible: bool,
}

/// The C spelling of a fallible native's result, shared by every library so
/// that two headers declaring one native agree.
pub const NATIVE_RESULT_C: &str = "kel_native_result";

/// A lowered coroutine ready to write as an object, with its contract.
pub struct Library<'ctx> {
    module: LlvmModule<'ctx>,
    /// The symbol prefix.
    pub prefix: String,
    /// Static frame bound the code was lowered with.
    pub frame_bytes: u64,
    /// Slot reservation for that bound.
    pub slot_bytes: u64,
    /// Shared buffer bytes.
    pub shared_bytes: u64,
    /// Private buffer bytes.
    pub private_bytes: u64,
    /// Composite region bytes.
    pub region_bytes: u64,
    /// Bytes to copy into the start of a zeroed private buffer before start.
    pub private_init: Vec<u8>,
    /// The trap classification.
    pub trap: TrapExport,
    /// The start handle's ordinary parameters, before the buffers and slot.
    pub start_params: Vec<CType>,
    /// Whether `<prefix>_yield_site` is exported.
    pub yield_site: bool,
    /// Natives the host must define.
    pub natives: Vec<NativeImport>,
    /// The shared layout as named fields, in offset order.
    pub shared: Vec<SharedField>,
}

fn check_prefix(prefix: &str) -> Result<(), LinkageError> {
    let mut chars = prefix.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let rest_ok = chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !first_ok || !rest_ok || prefix.len() > PREFIX_LIMIT {
        return Err(LinkageError::Prefix(format!(
            "`{prefix}` is not a C identifier of at most {PREFIX_LIMIT} bytes"
        )));
    }
    if prefix.starts_with(NATIVE_SYMBOL_PREFIX) {
        return Err(LinkageError::Prefix(format!(
            "`{prefix}` lies in the host-native namespace `{NATIVE_SYMBOL_PREFIX}`"
        )));
    }
    Ok(())
}

fn rename_export(function: FunctionValue<'_>, name: &str) -> Result<(), LinkageError> {
    function.as_global_value().set_name(name);
    // LLVM renames on a clash instead of failing, so the name is read back.
    if function.get_name().to_bytes() != name.as_bytes() {
        return Err(LinkageError::Prefix(format!(
            "`{name}` clashes with a symbol already in the module"
        )));
    }
    function.set_linkage(Linkage::External);
    Ok(())
}

fn add_constant<'ctx>(
    module: &LlvmModule<'ctx>,
    name: &str,
    value: inkwell::values::BasicValueEnum<'ctx>,
) -> Result<(), LinkageError> {
    if module.get_global(name).is_some() || module.get_function(name).is_some() {
        return Err(LinkageError::Prefix(format!(
            "`{name}` clashes with a symbol already in the module"
        )));
    }
    let global = module.add_global(value.get_type(), None, name);
    global.set_initializer(&value);
    global.set_constant(true);
    global.set_linkage(Linkage::External);
    Ok(())
}

fn parameters(ty: FunctionType<'_>, what: &str) -> Result<Vec<CType>, LinkageError> {
    ty.get_param_types()
        .into_iter()
        .map(|p| {
            CType::of_param(p).ok_or_else(|| {
                LinkageError::Signature(format!("{what} takes `{}`", p.print_to_string()))
            })
        })
        .collect()
}

fn signature(
    ty: FunctionType<'_>,
    what: &str,
) -> Result<(Vec<CType>, Option<CType>), LinkageError> {
    let params = parameters(ty, what)?;
    let ret = match ty.get_return_type() {
        None => None,
        Some(r) => Some(CType::of_basic(r).ok_or_else(|| {
            LinkageError::Signature(format!("{what} returns `{}`", r.print_to_string()))
        })?),
    };
    Ok((params, ret))
}

fn shared_fields(program: &Module) -> Result<Vec<SharedField>, LinkageError> {
    let Some(dl) = program.data_layout.as_ref() else {
        return Ok(Vec::new());
    };
    let names: Vec<&str> = dl
        .slots
        .iter()
        .filter(|s| s.visibility == SlotVisibility::Shared)
        .map(|s| s.name.as_str())
        .collect();
    if names.len() != dl.shared_layout.len() {
        return Err(LinkageError::Layout(format!(
            "{} shared slots but {} layout entries",
            names.len(),
            dl.shared_layout.len()
        )));
    }
    let width = |log2: u8| (1usize << log2) / 8;
    let (word, float, addr) = (
        width(program.word_bits_log2),
        width(program.float_bits_log2),
        width(program.addr_bits_log2),
    );
    let mut fields: Vec<SharedField> = Vec::new();
    for (name, entry) in names.iter().zip(&dl.shared_layout) {
        let element_bytes = if entry.kind & SHARED_SLOT_COMPOSITE_FLAG != 0 {
            u64::from(entry.len)
        } else {
            ScalarKind::from_tag(entry.kind)
                .ok_or_else(|| {
                    LinkageError::Layout(format!("`{name}` has unknown kind {}", entry.kind))
                })?
                .size_in_bytes(word, float, addr) as u64
        };
        let offset = u64::from(entry.offset);
        if let Some(last) = fields.last_mut()
            && last.name == *name
            && last.kind == entry.kind
            && last.element_bytes == element_bytes
            && last.offset + last.element_bytes * last.count == offset
        {
            last.count += 1;
            continue;
        }
        let ident: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        fields.push(SharedField {
            name: name.to_string(),
            ident,
            offset,
            element_bytes,
            count: 1,
            kind: entry.kind,
        });
    }
    let shared_bytes = keleusma::vm::shared_data_bytes_for(program) as u64;
    let mut idents = std::collections::BTreeSet::new();
    for f in &fields {
        if !idents.insert(f.ident.as_str()) {
            return Err(LinkageError::Layout(format!(
                "`{}` renders as `{}`, which another field already uses",
                f.name, f.ident
            )));
        }
        if f.offset + f.element_bytes * f.count > shared_bytes {
            return Err(LinkageError::Layout(format!(
                "`{}` extends past the {shared_bytes}-byte shared buffer",
                f.name
            )));
        }
    }
    Ok(fields)
}

/// Lower `program`'s entry as a coroutine library exporting under `prefix`.
///
/// The frame is reserved at [`coroutine::minimum_frame_bytes`], the static
/// bound. Verification and resource admission apply as for any coroutine.
pub fn build<'ctx>(
    ctx: &'ctx Context,
    program: &Module,
    machine: &TargetMachine,
    prefix: &str,
    contracts: &HostContracts,
) -> Result<Library<'ctx>, LinkageError> {
    check_prefix(prefix)?;
    let frame = coroutine::minimum_frame_bytes(program, machine, contracts)?;
    let slot = coroutine::slot_bytes(frame)?;
    let module = coroutine::lower_with_contracts(ctx, program, machine, frame, contracts)?;
    let entry = program
        .entry_point
        .ok_or_else(|| LinkageError::Emit(String::from("the module has no entry point")))?;
    let handle = |suffix: &str| module.get_function(&format!("kel_coroutine_{entry}_{suffix}"));
    let missing =
        |what: &str| LinkageError::Emit(format!("the lowering emitted no `{what}` handle"));
    let start = handle("start").ok_or_else(|| missing("start"))?;
    let resume = handle("resume").ok_or_else(|| missing("resume"))?;
    let release = handle("release").ok_or_else(|| missing("release"))?;
    let yield_site = handle("yield_site");

    // Everything defined becomes internal first, then the handles are named
    // and exported. A definition the scheme does not know about therefore
    // stays private rather than leaking a name another library could share.
    for f in module.get_functions() {
        if !f.as_global_value().is_declaration() {
            f.set_linkage(Linkage::Internal);
        }
    }
    for g in module.get_globals() {
        if !g.is_declaration() && g.get_linkage() != Linkage::Private {
            g.set_linkage(Linkage::Internal);
        }
    }
    rename_export(start, &format!("{prefix}_start"))?;
    rename_export(resume, &format!("{prefix}_resume"))?;
    rename_export(release, &format!("{prefix}_release"))?;
    if let Some(f) = yield_site {
        rename_export(f, &format!("{prefix}_yield_site"))?;
    }

    let shared_bytes = keleusma::vm::shared_data_bytes_for(program) as u64;
    let private_bytes = (keleusma::vm::required_persistent_capacity_for(program)
        + region::persistent_supplement_bytes(program) as usize) as u64;
    let region_bytes = u64::from(region::host_arena_supplement_bytes(program));
    // `private_init_image` answers `None` both for a module without data and
    // for an initializer it cannot place. Only the second is a refusal.
    let private_init = match (
        program.data_layout.is_some(),
        region::private_init_image(program),
    ) {
        (false, _) => Vec::new(),
        (true, Some(image)) => image,
        (true, None) => return Err(LinkageError::PrivateImage),
    };
    if private_init.len() as u64 > private_bytes {
        return Err(LinkageError::Layout(format!(
            "the {}-byte private image exceeds the {private_bytes}-byte private buffer",
            private_init.len()
        )));
    }
    let trap = match classify_traps(program) {
        Ok(TrapVerdict::TrapFree) => TrapExport::TrapFree,
        Ok(TrapVerdict::MayTrap { first }) => TrapExport::MayTrap(first.chunk, first.instruction),
        Err(e) => TrapExport::Unestablished(format!("{e}")),
    };

    let word = ctx.i64_type();
    for (suffix, value) in [
        ("contract_version", CONTRACT_VERSION),
        ("frame_bytes", u64::from(frame)),
        ("slot_bytes", u64::from(slot)),
        ("buffer_align", BUFFER_ALIGN),
        ("shared_bytes", shared_bytes),
        ("private_bytes", private_bytes),
        ("region_bytes", region_bytes),
        ("private_init_bytes", private_init.len() as u64),
    ] {
        add_constant(
            &module,
            &format!("{prefix}_{suffix}"),
            word.const_int(value, false).into(),
        )?;
    }
    add_constant(
        &module,
        &format!("{prefix}_trap_verdict"),
        ctx.i8_type()
            .const_int(u64::from(trap.code()), false)
            .into(),
    )?;
    if !private_init.is_empty() {
        add_constant(
            &module,
            &format!("{prefix}_private_init"),
            ctx.const_string(&private_init, false).into(),
        )?;
    }

    // **THE OBJECT IS OPTIMISED HERE, ONCE.** The figures this library
    // exports must describe the code it writes, and the machine stack bound
    // depends on the optimisation level. Writing therefore applies no further
    // pass, so a later write cannot change the code under its exported figures.
    module
        .run_passes(
            "globaldce,default<O2>",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(|e| LinkageError::Emit(e.to_string()))?;
    module
        .verify()
        .map_err(|e| LinkageError::Emit(e.to_string()))?;

    let mut natives = Vec::new();
    for f in module.get_functions() {
        if !f.as_global_value().is_declaration() || f.as_global_value().get_first_use().is_none() {
            continue;
        }
        let name = f.get_name().to_string_lossy().into_owned();
        if name.starts_with("llvm.") {
            continue;
        }
        if !name.starts_with(NATIVE_SYMBOL_PREFIX) {
            return Err(LinkageError::Import(name));
        }
        let ty = f.get_type();
        // A fallible native returns the two-word aggregate. Anything else
        // with a struct return is refused by `signature` as before.
        let two_words = ty.get_return_type().is_some_and(|r| {
            r.is_struct_type()
                && r.into_struct_type().get_field_types() == [word.into(), word.into()]
        });
        let (params, ret) = if two_words {
            (parameters(ty, &name)?, Some(CType::I64))
        } else {
            signature(ty, &name)?
        };
        natives.push(NativeImport {
            symbol: name,
            params,
            ret,
            fallible: two_words,
        });
    }
    if let Some(g) = module
        .get_globals()
        .find(|g| g.is_declaration() && g.as_pointer_value().get_first_use().is_some())
    {
        return Err(LinkageError::Import(
            g.get_name().to_string_lossy().into_owned(),
        ));
    }

    let start = module
        .get_function(&format!("{prefix}_start"))
        .ok_or_else(|| missing("start"))?;
    // The handle returns the two-word outcome struct, rendered by hand below.
    let params = parameters(start.get_type(), "start")?;
    // Shared, private, composite region and slot follow the ordinary
    // parameters, as `coroutine` documents.
    let ordinary = params.len().checked_sub(4).ok_or_else(|| {
        LinkageError::Signature(String::from("start has fewer than its four buffers"))
    })?;
    if params[ordinary..].iter().any(|p| *p != CType::Ptr) {
        return Err(LinkageError::Signature(String::from(
            "start's trailing parameters are not the four buffer pointers",
        )));
    }

    Ok(Library {
        module,
        prefix: prefix.to_string(),
        frame_bytes: u64::from(frame),
        slot_bytes: u64::from(slot),
        shared_bytes,
        private_bytes,
        region_bytes,
        private_init,
        trap,
        start_params: params[..ordinary].to_vec(),
        yield_site: yield_site.is_some(),
        natives,
        shared: shared_fields(program)?,
    })
}

impl<'ctx> Library<'ctx> {
    /// The lowered module, for inspection.
    pub fn module(&self) -> &LlvmModule<'ctx> {
        &self.module
    }

    /// Every symbol the object defines with external linkage, undecorated.
    pub fn exported_symbols(&self) -> Vec<String> {
        let p = &self.prefix;
        let mut out: Vec<String> = ["start", "resume", "release"]
            .iter()
            .map(|s| format!("{p}_{s}"))
            .collect();
        if self.yield_site {
            out.push(format!("{p}_yield_site"));
        }
        for s in [
            "contract_version",
            "frame_bytes",
            "slot_bytes",
            "buffer_align",
            "shared_bytes",
            "private_bytes",
            "region_bytes",
            "private_init_bytes",
            "trap_verdict",
        ] {
            out.push(format!("{p}_{s}"));
        }
        if !self.private_init.is_empty() {
            out.push(format!("{p}_private_init"));
        }
        out.sort();
        out
    }

    /// Write the object file. [`build`] already ran the `default<O2>`
    /// middle end, so this applies no pass.
    pub fn write_object(
        &self,
        machine: &TargetMachine,
        path: &std::path::Path,
    ) -> Result<(), LinkageError> {
        machine
            .write_to_file(&self.module, FileType::Object, path)
            .map_err(|e| LinkageError::Emit(e.to_string()))
    }

    fn figures(&self) -> [(&'static str, u64); 8] {
        [
            ("CONTRACT_VERSION", CONTRACT_VERSION),
            ("FRAME_BYTES", self.frame_bytes),
            ("SLOT_BYTES", self.slot_bytes),
            ("BUFFER_ALIGN", BUFFER_ALIGN),
            ("SHARED_BYTES", self.shared_bytes),
            ("PRIVATE_BYTES", self.private_bytes),
            ("REGION_BYTES", self.region_bytes),
            ("PRIVATE_INIT_BYTES", self.private_init.len() as u64),
        ]
    }

    fn banner(&self, comment: &str) -> String {
        let lines = [
            format!(
                "GENERATED by keleusma_native::linkage for `{}`. Do not edit.",
                self.prefix
            ),
            String::from("Every figure is read from the values the object defines."),
            String::from("Reserve one slot of SLOT_BYTES per live instance and pass"),
            String::from("shared, private and region buffers of the stated sizes,"),
            String::from("each aligned to BUFFER_ALIGN. Zero the private buffer and copy"),
            String::from("the private image into its start before the first start call."),
            format!("Trap verdict {}.", self.trap.describe()),
            String::from("No execution-time bound is exported. See NATIVE_WCET_ASYMMETRY.md."),
        ];
        lines.iter().map(|l| format!("{comment} {l}\n")).collect()
    }

    /// The contract as a C header.
    pub fn c_header(&self) -> String {
        let p = &self.prefix;
        let up = p.to_ascii_uppercase();
        let mut h = self.banner("//");
        h.push_str(&format!(
            "#ifndef {up}_H\n#define {up}_H\n#include <stdbool.h>\n#include <stdint.h>\n\n"
        ));
        for (name, value) in self.figures() {
            h.push_str(&format!("#define {up}_{name} {value}u\n"));
        }
        h.push_str(&format!(
            "#define {up}_TRAP_VERDICT {}u\n\n",
            self.trap.code()
        ));
        for f in &self.shared {
            h.push_str(&format!(
                "#define {up}_{id}_OFFSET {}u\n#define {up}_{id}_ELEMENT_BYTES {}u\n#define {up}_{id}_COUNT {}u\n",
                f.offset,
                f.element_bytes,
                f.count,
                id = f.ident
            ));
        }
        h.push_str(&format!(
            "\nstruct {p}_outcome {{ uint64_t status; int64_t value; }};\n\n"
        ));
        let mut args: Vec<String> = self
            .start_params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("{} arg{i}", t.c()))
            .collect();
        args.extend(
            ["shared", "private_region", "composite_region", "slot"]
                .iter()
                .map(|n| format!("unsigned char *{n}")),
        );
        h.push_str(&format!(
            "struct {p}_outcome {p}_start({});\n",
            args.join(", ")
        ));
        h.push_str(&format!(
            "struct {p}_outcome {p}_resume(unsigned char *slot, int64_t reply);\n"
        ));
        h.push_str(&format!("void {p}_release(unsigned char *slot);\n"));
        if self.yield_site {
            h.push_str(&format!("uint64_t {p}_yield_site(unsigned char *slot);\n"));
        }
        h.push('\n');
        for (name, _) in self.figures() {
            h.push_str(&format!(
                "extern const uint64_t {p}_{};\n",
                name.to_ascii_lowercase()
            ));
        }
        h.push_str(&format!("extern const uint8_t {p}_trap_verdict;\n"));
        if !self.private_init.is_empty() {
            h.push_str(&format!(
                "extern const unsigned char {p}_private_init[{}];\n",
                self.private_init.len()
            ));
        }
        if self.natives.is_empty() {
            h.push_str("\n// This library calls no host natives.\n");
        } else {
            h.push_str("\n// The host must define each of these natives.\n");
            if self.natives.iter().any(|n| n.fallible) {
                h.push_str(&format!(
                    "// A fallible native reports a soft failure with a nonzero `failed`,\n\
                     // and `value` is then its error code.\n\
                     #ifndef KEL_NATIVE_RESULT_DEFINED\n#define KEL_NATIVE_RESULT_DEFINED\n\
                     typedef struct {{ int64_t value; int64_t failed; }} {NATIVE_RESULT_C};\n#endif\n"
                ));
            }
            for n in &self.natives {
                let ret = if n.fallible {
                    NATIVE_RESULT_C
                } else {
                    n.ret.map_or("void", CType::c)
                };
                let args = if n.params.is_empty() {
                    String::from("void")
                } else {
                    n.params
                        .iter()
                        .map(|t| t.c())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                h.push_str(&format!("{ret} {}({args});\n", n.symbol));
            }
        }
        h.push_str("\n#endif\n");
        h
    }

    /// The contract as a Rust module named after the prefix, linking the
    /// static archive `lib<prefix>.a`.
    ///
    /// Each native appears as a function pointer type in `natives`. A host
    /// that coerces its definition to that type gets a compile error for a
    /// mismatched signature, which the linker would not report.
    pub fn rust_bindings(&self) -> String {
        let p = &self.prefix;
        let mut r = self.banner("//");
        r.push_str(&format!(
            "#[allow(dead_code, non_upper_case_globals, non_camel_case_types)]\npub mod {p} {{\n"
        ));
        for (name, value) in self.figures() {
            r.push_str(&format!("    pub const {name}: u64 = {value};\n"));
        }
        r.push_str(&format!(
            "    pub const TRAP_VERDICT: u8 = {};\n\n",
            self.trap.code()
        ));
        r.push_str("    pub mod shared {\n");
        for f in &self.shared {
            r.push_str(&format!(
                "        pub const {id}_OFFSET: u64 = {};\n        pub const {id}_ELEMENT_BYTES: u64 = {};\n        pub const {id}_COUNT: u64 = {};\n",
                f.offset,
                f.element_bytes,
                f.count,
                id = f.ident
            ));
        }
        r.push_str("    }\n\n    pub mod natives {\n");
        if self.natives.iter().any(|n| n.fallible) {
            r.push_str(
                "        /// A fallible native's result. A nonzero `failed` is a soft failure\n        /// whose error code is `value`.\n        #[repr(C)]\n        #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n        pub struct NativeResult {\n            pub value: i64,\n            pub failed: i64,\n        }\n",
            );
        }
        for n in &self.natives {
            let args: Vec<&str> = n.params.iter().map(|t| t.rust()).collect();
            let ret = if n.fallible {
                String::from(" -> NativeResult")
            } else {
                n.ret.map_or(String::new(), |t| format!(" -> {}", t.rust()))
            };
            r.push_str(&format!(
                "        pub type {} = extern \"C\" fn({}){ret};\n",
                n.symbol,
                args.join(", ")
            ));
        }
        r.push_str("    }\n\n");
        r.push_str(
            "    #[repr(C)]\n    #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n    pub struct Outcome {\n        pub status: u64,\n        pub value: i64,\n    }\n\n",
        );
        r.push_str(&format!(
            "    #[link(name = \"{p}\", kind = \"static\")]\n    unsafe extern \"C\" {{\n"
        ));
        let mut args: Vec<String> = self
            .start_params
            .iter()
            .enumerate()
            .map(|(i, t)| format!("arg{i}: {}", t.rust()))
            .collect();
        args.extend(
            ["shared", "private_region", "composite_region", "slot"]
                .iter()
                .map(|n| format!("{n}: *mut u8")),
        );
        r.push_str(&format!(
            "        pub fn {p}_start({}) -> Outcome;\n",
            args.join(", ")
        ));
        r.push_str(&format!(
            "        pub fn {p}_resume(slot: *mut u8, reply: i64) -> Outcome;\n"
        ));
        r.push_str(&format!("        pub fn {p}_release(slot: *mut u8);\n"));
        if self.yield_site {
            r.push_str(&format!(
                "        pub fn {p}_yield_site(slot: *mut u8) -> u64;\n"
            ));
        }
        for (name, _) in self.figures() {
            r.push_str(&format!(
                "        pub static {p}_{}: u64;\n",
                name.to_ascii_lowercase()
            ));
        }
        r.push_str(&format!("        pub static {p}_trap_verdict: u8;\n"));
        if !self.private_init.is_empty() {
            r.push_str(&format!(
                "        pub static {p}_private_init: [u8; {}];\n",
                self.private_init.len()
            ));
        }
        r.push_str("    }\n}\n");
        r
    }
}
