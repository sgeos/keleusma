//! The cryptographic framing of the wire format written in Keleusma, with host-native
//! primitives (Order 5, roadmap Workstream E, approach 1).
//!
//! The framing program is `frame.kel` beside this file. It decides the signed and encrypted
//! header layouts, the signature and encryption metadata blocks, which bytes a signature covers,
//! the order and verdicts of the framing checks, the verify-before-decrypt ordering, the
//! comparison of the recipient fingerprint, the reconstruction of the plaintext view and the CRC
//! trailer. This driver supplies the primitives as natives and moves bytes. See
//! `docs/decisions/ORDER5_CRYPTO_FRAMING_PLAN.md` for the scope and the gate.
//!
//! **Trust boundary.** No key material enters the Keleusma program. The signing key, the trusted
//! verifying keys, the ephemeral seed and the local private key stay in `Host`, and a native
//! refers to a verifying key only by its index. Ed25519 signing and strict verification, X25519
//! agreement, HKDF-SHA-256, AES-256-GCM and SHA-256 run in this file through the crates the
//! reference uses and through `crate::encryption`. The plaintext body of a module does pass
//! through the program, as it does through the reference encoder.
//!
//! **Faults.** The natives refuse an index outside a buffer, a byte outside `[0, 255]` and a
//! write to the input, the sealed parameters or either fingerprint, and the virtual machine
//! reports the refusal as an error. The message, signature, plaintext and ciphertext buffers are
//! writable because the program supplies them to a primitive on one path and reads a primitive's
//! output from them on another, so the program is trusted not to alter a primitive's output.
//! Such an error is a defect of `frame.kel` or of this driver, never a property of the artifact,
//! so it is [`FrameError::Stage`](crate::crypto_frame::FrameError::Stage) and never a [`Refusal`](crate::crypto_frame::Refusal).

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

use crate::Arena;
use crate::bytecode::{Module, Value};
use crate::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState, required_persistent_capacity_for};
use crate::wire_format::{SigningSections, signing_sections};

/// The framing program's source, embedded so the driver needs no filesystem access.
pub const FRAME_SOURCE: &str = include_str!("frame.kel");

/// Units of work per step, the literal in `step` in `frame.kel`.
pub const UNITS_PER_STEP: usize = 4096;

/// The host buffers, numbered as in `frame.kel`.
mod buf {
    pub const IN: usize = 0;
    pub const OUT: usize = 1;
    pub const MSG: usize = 2;
    pub const SIG: usize = 3;
    pub const PT: usize = 4;
    pub const CT: usize = 5;
    #[cfg(feature = "encryption")]
    pub const SEALED: usize = 6;
    #[cfg(feature = "encryption")]
    pub const RID: usize = 7;
    #[cfg(feature = "encryption")]
    pub const LID: usize = 8;
    pub const OPENP: usize = 9;
    pub const COUNT: usize = 10;
    /// Buffers the program may write. The others are written by the host or a primitive.
    pub const WRITABLE: [usize; 6] = [OUT, MSG, SIG, PT, CT, OPENP];
}

/// The commands of `frame.kel`'s protocol.
mod cmd {
    pub const STEP: i64 = 2;
    pub const SIGN: i64 = 10;
    pub const VERIFY: i64 = 11;
    #[cfg(feature = "encryption")]
    pub const ENCRYPT: i64 = 12;
    #[cfg(feature = "encryption")]
    pub const DECRYPT: i64 = 13;
}

/// A verdict of the framing program. The codes are the negative values `frame.kel` yields, in
/// the reference's order of checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i64)]
pub enum Refusal {
    /// The buffer or the declared total is shorter than the framing requires.
    Truncated = -1,
    /// The artifact does not begin with the bytecode magic.
    BadMagic = -2,
    /// The header is the unsigned 64-byte header.
    NotSigned = -3,
    /// The header is longer than 64 bytes and shorter than the signature metadata.
    HeaderBelowMinimum = -4,
    /// A reserved field of the signature metadata is nonzero.
    ReservedNonzero = -5,
    /// The signature scheme is 0, which is reserved.
    SchemeZero = -6,
    /// The signature scheme is not Ed25519.
    UnsupportedScheme = -7,
    /// The declared signature length is not 64.
    SignatureLength = -8,
    /// The header length does not match the signed or encrypted layout.
    HeaderLengthMismatch = -9,
    /// No trusted key verified the signature.
    InvalidSignature = -10,
    /// Decryption was asked of an artifact without the encryption flag.
    NotEncrypted = -11,
    /// The encryption flag is set without the signature flag.
    EncryptedUnsigned = -12,
    /// The encryption metadata names another scheme or length.
    EncryptionMetadata = -13,
    /// The artifact is addressed to another recipient.
    WrongRecipient = -14,
    /// The ephemeral public key gives a non-contributory agreement.
    NonContributory = -15,
    /// The nonce does not match the one the agreement derives.
    NonceMismatch = -16,
    /// The ciphertext did not authenticate.
    AuthenticationFailed = -17,
    /// A key the operation needs was not supplied to the host.
    MissingKeyMaterial = -18,
    /// A framing input is out of range.
    InvalidInput = -19,
    /// A command arrived out of the protocol's order.
    OutOfOrder = -20,
    /// The sealing primitive failed.
    SealFailed = -21,
}

impl Refusal {
    fn from_code(code: i64) -> Option<Self> {
        use Refusal::*;
        let all = [
            Truncated,
            BadMagic,
            NotSigned,
            HeaderBelowMinimum,
            ReservedNonzero,
            SchemeZero,
            UnsupportedScheme,
            SignatureLength,
            HeaderLengthMismatch,
            InvalidSignature,
            NotEncrypted,
            EncryptedUnsigned,
            EncryptionMetadata,
            WrongRecipient,
            NonContributory,
            NonceMismatch,
            AuthenticationFailed,
            MissingKeyMaterial,
            InvalidInput,
            OutOfOrder,
            SealFailed,
        ];
        all.into_iter().find(|r| *r as i64 == code)
    }
}

/// Why a framing operation did not produce its result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// The framing program refused the artifact or the request.
    Refused(Refusal),
    /// The framing program or this driver failed. A defect, never a property of the input.
    Stage(String),
}

impl From<VmError> for FrameError {
    fn from(e: VmError) -> Self {
        FrameError::Stage(format!("frame.kel: {e:?}"))
    }
}

/// The host side of the natives. It owns the buffers and every key.
#[derive(Default)]
struct Host {
    bufs: [Vec<u8>; buf::COUNT],
    signing: Option<SigningKey>,
    verifying: Vec<VerifyingKey>,
    #[cfg(feature = "encryption")]
    recipient: Option<[u8; 32]>,
    #[cfg(feature = "encryption")]
    seed: Option<[u8; 32]>,
    #[cfg(feature = "encryption")]
    local: Option<[u8; 32]>,
}

type Shared = Rc<RefCell<Host>>;

fn native_error(detail: String) -> VmError {
    VmError::NativeError(format!("crypto_frame: {detail}"))
}

fn word(args: &[Value], i: usize) -> Result<i64, VmError> {
    match args.get(i) {
        Some(Value::Int(n)) => Ok(*n),
        other => Err(native_error(format!("argument {i} is {other:?}"))),
    }
}

fn buffer_index(args: &[Value], i: usize) -> Result<usize, VmError> {
    let b = word(args, i)?;
    usize::try_from(b)
        .ok()
        .filter(|&b| b < buf::COUNT)
        .ok_or_else(|| native_error(format!("buffer {b}")))
}

fn writable(b: usize) -> Result<usize, VmError> {
    if buf::WRITABLE.contains(&b) {
        Ok(b)
    } else {
        Err(native_error(format!("buffer {b} is not writable")))
    }
}

fn byte(v: i64) -> Result<u8, VmError> {
    u8::try_from(v).map_err(|_| native_error(format!("byte {v}")))
}

fn position(args: &[Value], i: usize, len: usize) -> Result<usize, VmError> {
    let at = word(args, i)?;
    usize::try_from(at)
        .ok()
        .filter(|&at| at < len)
        .ok_or_else(|| native_error(format!("index {at} outside length {len}")))
}

fn ok(n: i64) -> Result<Value, VmError> {
    Ok(Value::Int(n))
}

/// Register the buffer natives and the primitive natives on `vm`.
fn register(vm: &mut Vm<'_, '_>, host: &Shared) {
    let h = host.clone();
    vm.register_native_closure("host::blen", move |args: &[Value]| {
        let b = buffer_index(args, 0)?;
        ok(h.borrow().bufs[b].len() as i64)
    });
    let h = host.clone();
    vm.register_native_closure("host::bget", move |args: &[Value]| {
        let b = buffer_index(args, 0)?;
        let host = h.borrow();
        let at = position(args, 1, host.bufs[b].len())?;
        ok(i64::from(host.bufs[b][at]))
    });
    let h = host.clone();
    vm.register_native_closure("host::bput", move |args: &[Value]| {
        let b = writable(buffer_index(args, 0)?)?;
        let v = byte(word(args, 2)?)?;
        let mut host = h.borrow_mut();
        let at = position(args, 1, host.bufs[b].len())?;
        host.bufs[b][at] = v;
        ok(0)
    });
    let h = host.clone();
    vm.register_native_closure("host::bpush", move |args: &[Value]| {
        let b = writable(buffer_index(args, 0)?)?;
        let v = byte(word(args, 1)?)?;
        let mut host = h.borrow_mut();
        if host.bufs[b].len() >= u32::MAX as usize {
            return Err(native_error(format!("buffer {b} is full")));
        }
        host.bufs[b]
            .try_reserve(1)
            .map_err(|_| native_error(String::from("allocation failed")))?;
        host.bufs[b].push(v);
        ok(0)
    });
    let h = host.clone();
    vm.register_native_closure("host::bclear", move |args: &[Value]| {
        let b = writable(buffer_index(args, 0)?)?;
        let n = word(args, 1)?;
        let n = usize::try_from(n)
            .ok()
            .filter(|&n| n <= u32::MAX as usize)
            .ok_or_else(|| native_error(format!("length {n}")))?;
        let mut host = h.borrow_mut();
        let target = &mut host.bufs[b];
        target.clear();
        target
            .try_reserve_exact(n)
            .map_err(|_| native_error(String::from("allocation failed")))?;
        target.resize(n, 0);
        ok(0)
    });
    let h = host.clone();
    vm.register_native_closure("host::sign", move |_args: &[Value]| {
        let mut host = h.borrow_mut();
        let Some(key) = host.signing.clone() else {
            return ok(0);
        };
        let signature = key.sign(&host.bufs[buf::MSG]).to_bytes();
        host.bufs[buf::SIG] = signature.to_vec();
        ok(1)
    });
    let h = host.clone();
    vm.register_native_closure("host::nkeys", move |_args: &[Value]| {
        ok(h.borrow().verifying.len() as i64)
    });
    let h = host.clone();
    vm.register_native_closure("host::verify", move |args: &[Value]| {
        let host = h.borrow();
        let k = position(args, 0, host.verifying.len())?;
        let bytes: [u8; 64] = host.bufs[buf::SIG]
            .as_slice()
            .try_into()
            .map_err(|_| native_error(String::from("signature buffer is not 64 bytes")))?;
        let signature = Signature::from_bytes(&bytes);
        // `verify_strict`, as the reference uses, refuses the malleable forms.
        let verified = host.verifying[k]
            .verify_strict(&host.bufs[buf::MSG], &signature)
            .is_ok();
        ok(i64::from(verified))
    });
    let h = host.clone();
    vm.register_native_closure("host::seal", move |_args: &[Value]| seal(&h));
    let h = host.clone();
    vm.register_native_closure("host::open", move |_args: &[Value]| open(&h));
}

#[cfg(feature = "encryption")]
fn seal(h: &Shared) -> Result<Value, VmError> {
    let mut host = h.borrow_mut();
    // The seed is consumed. A second seal on the same machine would otherwise reuse the
    // ephemeral key, and with it the derived AES-GCM key and nonce, over another plaintext.
    let (Some(recipient), Some(seed)) = (host.recipient, host.seed.take()) else {
        return ok(0);
    };
    match crate::encryption::encrypt_to_recipient(&host.bufs[buf::PT], &recipient, &seed) {
        Ok((metadata, ciphertext)) => {
            let mut sealed = metadata.ephemeral_public_key.to_vec();
            sealed.extend_from_slice(&metadata.aes_gcm_nonce);
            host.bufs[buf::SEALED] = sealed;
            host.bufs[buf::CT] = ciphertext;
            ok(1)
        }
        Err(_) => ok(-1),
    }
}

#[cfg(not(feature = "encryption"))]
fn seal(_h: &Shared) -> Result<Value, VmError> {
    ok(0)
}

#[cfg(feature = "encryption")]
fn open(h: &Shared) -> Result<Value, VmError> {
    use crate::encryption::EncryptionError;
    let mut host = h.borrow_mut();
    let Some(local) = host.local else {
        return ok(0);
    };
    let parts = &host.bufs[buf::OPENP];
    if parts.len() != 44 {
        return Err(native_error(String::from(
            "open parameters are not 44 bytes",
        )));
    }
    let mut ephemeral = [0u8; 32];
    ephemeral.copy_from_slice(&parts[..32]);
    let mut nonce = [0u8; 12];
    nonce.copy_from_slice(&parts[32..]);
    match crate::encryption::open_sealed(&ephemeral, &nonce, &host.bufs[buf::CT], &local) {
        Ok(plaintext) => {
            host.bufs[buf::PT] = plaintext;
            ok(1)
        }
        Err(EncryptionError::NonContributoryKey) => ok(-1),
        Err(EncryptionError::DecryptFailed(detail)) if detail.starts_with("nonce in metadata") => {
            ok(-2)
        }
        Err(EncryptionError::DecryptFailed(_)) => ok(-3),
        Err(_) => ok(-4),
    }
}

#[cfg(not(feature = "encryption"))]
fn open(_h: &Shared) -> Result<Value, VmError> {
    ok(0)
}

/// Compile the framing program.
///
/// # Errors
///
/// A [`FrameError::Stage`] when the embedded source does not compile, which is a defect.
pub fn frame_module() -> Result<Module, FrameError> {
    let stage = |e: String| FrameError::Stage(format!("frame.kel: {e}"));
    let tokens = crate::lexer::tokenize(FRAME_SOURCE).map_err(|e| stage(format!("{e:?}")))?;
    let program = crate::parser::parse(&tokens).map_err(|e| stage(format!("{e:?}")))?;
    crate::compiler::compile(&program).map_err(|e| stage(e.message))
}

/// One run of the framing program on a fresh virtual machine.
struct Machine<'arena> {
    vm: Vm<'arena, 'arena>,
    shared: Vec<u8>,
    host: Shared,
}

impl<'arena> Machine<'arena> {
    fn new(module: Module, arena: &'arena Arena, host: Host) -> Result<Self, FrameError> {
        let mut vm = Vm::new(module, arena)?;
        let host = Rc::new(RefCell::new(host));
        register(&mut vm, &host);
        let shared = alloc::vec![0u8; vm.shared_data_bytes()];
        Ok(Machine { vm, shared, host })
    }

    /// Send one command and return the yielded value. The first command enters `loop main`
    /// through a call and every later one through a resume, as in the hosted runtime's driver.
    fn send(&mut self, command: i64) -> Result<i64, FrameError> {
        for _ in 0..2 {
            let state = match self
                .vm
                .resume_with_shared(&mut self.shared, Value::Int(command))
            {
                Err(VmError::NotSuspended) => self
                    .vm
                    .call_with_shared(&mut self.shared, &[Value::Int(command)])?,
                other => other?,
            };
            match state {
                VmState::Yielded(Value::Int(v)) => return Ok(v),
                VmState::Reset => continue,
                other => {
                    return Err(FrameError::Stage(format!(
                        "frame.kel produced {other:?} for command {command}"
                    )));
                }
            }
        }
        Err(FrameError::Stage(format!(
            "frame.kel reset twice without yielding for command {command}"
        )))
    }

    /// Begin an operation and step it to completion. The step budget follows from the units
    /// each phase spends, at most eight per byte of the larger buffer plus one per key, so
    /// exceeding it means the program stopped making progress.
    fn run(&mut self, begin: i64) -> Result<(), FrameError> {
        let started = self.send(begin)?;
        if started < 0 {
            return Err(refusal(started));
        }
        let (bytes, keys) = {
            let host = self.host.borrow();
            let bytes = host.bufs[buf::IN].len().max(host.bufs[buf::OUT].len());
            (bytes, host.verifying.len())
        };
        let units = bytes
            .saturating_add(1024)
            .saturating_mul(8)
            .saturating_add(keys);
        let steps = units / UNITS_PER_STEP + 8;
        for _ in 0..steps {
            match self.send(cmd::STEP)? {
                0 => {}
                1 => return Ok(()),
                code if code < 0 => return Err(refusal(code)),
                other => {
                    return Err(FrameError::Stage(format!("frame.kel step yielded {other}")));
                }
            }
        }
        Err(FrameError::Stage(String::from(
            "frame.kel exceeded its step budget",
        )))
    }

    fn set_field(&mut self, slot: usize, value: i64) -> Result<(), FrameError> {
        self.vm
            .set_shared(&mut self.shared, slot, Value::Int(value))
            .map_err(FrameError::from)
    }

    fn output(&self) -> Vec<u8> {
        self.host.borrow().bufs[buf::OUT].clone()
    }
}

fn refusal(code: i64) -> FrameError {
    match Refusal::from_code(code) {
        Some(r) => FrameError::Refused(r),
        None => FrameError::Stage(format!("frame.kel yielded unknown verdict {code}")),
    }
}

/// Run `work` on a machine for the framing program with an arena sized for its private state.
fn with_machine<T>(
    host: Host,
    work: impl FnOnce(&mut Machine<'_>) -> Result<T, FrameError>,
) -> Result<T, FrameError> {
    let module = frame_module()?;
    let need = required_persistent_capacity_for(&module);
    let capacity = DEFAULT_ARENA_CAPACITY
        .checked_add(need)
        .ok_or_else(|| FrameError::Stage(String::from("arena size overflow")))?;
    let mut arena = Arena::with_capacity(capacity);
    arena
        .resize_persistent(need)
        .map_err(|e| FrameError::Stage(format!("arena: {e:?}")))?;
    let mut machine = Machine::new(module, &arena, host)?;
    work(&mut machine)
}

/// The thirteen framing fields `frame.kel` takes, in its order.
fn framing_fields(module: &Module, sections: &SigningSections) -> [i64; 13] {
    let len = |b: &[u8]| b.len() as i64;
    [
        i64::from(module.word_bits_log2),
        i64::from(module.addr_bits_log2),
        i64::from(module.float_bits_log2),
        i64::from(module.flags),
        i64::from(module.wcet_cycles),
        i64::from(module.wcmu_bytes),
        i64::from(module.shared_data_bytes),
        i64::from(module.private_data_bytes),
        i64::from(module.aux_arena_bytes),
        i64::from(module.persistent_composite_bytes),
        len(&sections.opcodes),
        len(&sections.operands),
        len(&sections.auxiliary),
    ]
}

fn concatenated(sections: &SigningSections) -> Vec<u8> {
    let mut input = sections.opcodes.clone();
    input.extend_from_slice(&sections.operands);
    input.extend_from_slice(&sections.auxiliary);
    input
}

/// Begin and run a signing on `machine`, whose host already holds the signing key.
fn sign_on(
    machine: &mut Machine<'_>,
    module: &Module,
    sections: &SigningSections,
) -> Result<(), FrameError> {
    for (slot, value) in framing_fields(module, sections).into_iter().enumerate() {
        machine.set_field(slot, value)?;
    }
    machine.host.borrow_mut().bufs[buf::IN] = concatenated(sections);
    machine.run(cmd::SIGN)
}

/// Frame already produced sections as a signed artifact through `frame.kel`.
///
/// The sections are opaque here, as in the unsigned framing stage. Their production is a
/// separate obligation. The result is byte-identical to [`crate::wire_format::module_to_signed_wire_bytes`]
/// when the sections are those [`signing_sections`] produces, which the Order 5 harness checks.
///
/// # Errors
///
/// [`Refusal::InvalidInput`] for a field or total out of range, and [`FrameError::Stage`] for a
/// defect.
pub fn sign_sections(
    module: &Module,
    sections: &SigningSections,
    signing_key: &SigningKey,
) -> Result<Vec<u8>, FrameError> {
    let host = Host {
        signing: Some(signing_key.clone()),
        ..Host::default()
    };
    with_machine(host, |machine| {
        sign_on(machine, module, sections)?;
        Ok(machine.output())
    })
}

/// Encode and sign `module` with the framing written in Keleusma. The sections come from
/// [`signing_sections`]. With `ecc` each auxiliary region carries a parity plane.
///
/// # Errors
///
/// As [`sign_sections`], and [`FrameError::Stage`] when the sections cannot be encoded.
pub fn signed_wire_bytes(
    module: &Module,
    signing_key: &SigningKey,
    ecc: bool,
) -> Result<Vec<u8>, FrameError> {
    let sections =
        signing_sections(module, ecc).map_err(|e| FrameError::Stage(format!("sections: {e:?}")))?;
    sign_sections(module, &sections, signing_key)
}

/// Check the framing and the signature of `bytes` against `keys` through `frame.kel`, with the
/// verdicts of [`crate::wire_format::verify_module_signature`].
///
/// # Errors
///
/// The framing program's [`Refusal`], and [`FrameError::Stage`] for a defect.
pub fn verify_signature(bytes: &[u8], keys: &[VerifyingKey]) -> Result<(), FrameError> {
    let mut host = Host {
        verifying: keys.to_vec(),
        ..Host::default()
    };
    host.bufs[buf::IN] = bytes.to_vec();
    with_machine(host, |machine| machine.run(cmd::VERIFY))
}

/// Encode, sign and encrypt `module` with the framing written in Keleusma, as
/// [`crate::wire_format::module_to_encrypted_signed_wire_bytes`] does. The program signs the
/// module, seals the signed body to the recipient, lays out the encrypted header and signs the
/// result.
///
/// # Errors
///
/// As [`signed_wire_bytes`], and [`Refusal::SealFailed`] when sealing fails.
#[cfg(feature = "encryption")]
pub fn encrypted_signed_wire_bytes(
    module: &Module,
    signing_key: &SigningKey,
    recipient_public_key: &[u8; 32],
    ephemeral_seed: &[u8; 32],
) -> Result<Vec<u8>, FrameError> {
    let sections = signing_sections(module, false)
        .map_err(|e| FrameError::Stage(format!("sections: {e:?}")))?;
    encrypt_sections(
        module,
        &sections,
        signing_key,
        recipient_public_key,
        ephemeral_seed,
    )
}

/// Sign and encrypt already produced sections through `frame.kel`.
///
/// # Errors
///
/// As [`encrypted_signed_wire_bytes`].
#[cfg(feature = "encryption")]
pub fn encrypt_sections(
    module: &Module,
    sections: &SigningSections,
    signing_key: &SigningKey,
    recipient_public_key: &[u8; 32],
    ephemeral_seed: &[u8; 32],
) -> Result<Vec<u8>, FrameError> {
    let mut host = Host {
        signing: Some(signing_key.clone()),
        recipient: Some(*recipient_public_key),
        seed: Some(*ephemeral_seed),
        ..Host::default()
    };
    host.bufs[buf::RID] = crate::encryption::recipient_key_id(recipient_public_key).to_vec();
    with_machine(host, |machine| {
        sign_on(machine, module, sections)?;
        machine.run(cmd::ENCRYPT)?;
        Ok(machine.output())
    })
}

/// Verify and decrypt an encrypted artifact through `frame.kel`, returning the plaintext view
/// [`crate::wire_format::decrypt_encrypted_signed_to_signed_bytes`] returns.
///
/// # Errors
///
/// The framing program's [`Refusal`], and [`FrameError::Stage`] for a defect.
#[cfg(feature = "encryption")]
pub fn decrypt_to_signed_bytes(
    bytes: &[u8],
    keys: &[VerifyingKey],
    local_private_key: &[u8; 32],
) -> Result<Vec<u8>, FrameError> {
    let local_public = crate::encryption::public_key_from_private(local_private_key);
    let mut host = Host {
        verifying: keys.to_vec(),
        local: Some(*local_private_key),
        ..Host::default()
    };
    host.bufs[buf::IN] = bytes.to_vec();
    host.bufs[buf::LID] = crate::encryption::recipient_key_id(&local_public).to_vec();
    with_machine(host, |machine| {
        machine.run(cmd::DECRYPT)?;
        Ok(machine.output())
    })
}

/// The sections of a signed module produced by the self-hosted wire stages of Order 1, with no
/// reference encoder. The opcode stream and operand pool come from
/// [`crate::selfhost::wire_opcode_sections_via_kel`] and the auxiliary body, whose flags carry
/// `FLAG_REQUIRES_SIGNATURE`, from [`crate::selfhost::wire_auxiliary_from_module_via_kel`].
/// Error-correcting planes are not produced on this path.
///
/// # Errors
///
/// [`FrameError::Stage`] when a stage refuses the module or fails.
#[cfg(feature = "self-host")]
pub fn signing_sections_via_kel(module: &Module) -> Result<SigningSections, FrameError> {
    let mut flagged = module.clone();
    flagged.flags |= crate::wire_format::FLAG_REQUIRES_SIGNATURE;
    let stage = |e: crate::selfhost::SelfHostError| FrameError::Stage(format!("sections: {e}"));
    let code = crate::selfhost::wire_opcode_sections_via_kel(&flagged).map_err(stage)?;
    let auxiliary = crate::selfhost::wire_auxiliary_from_module_via_kel(&flagged).map_err(stage)?;
    Ok(SigningSections {
        opcodes: code.opcodes,
        operands: code.operands,
        auxiliary,
    })
}

/// Protocol probes for the harness. Each sends `commands` in order to a fresh machine with no
/// keys and returns what each yielded.
///
/// # Errors
///
/// [`FrameError::Stage`] for a defect.
pub fn probe_protocol(commands: &[i64]) -> Result<Vec<i64>, FrameError> {
    with_machine(Host::default(), |machine| {
        commands.iter().map(|&c| machine.send(c)).collect()
    })
}

/// A protocol probe of a signing whose thirteen framing fields are `fields`, over an empty
/// section input, followed by `commands`.
///
/// # Errors
///
/// [`FrameError::Stage`] for a defect.
pub fn probe_signing_fields(fields: [i64; 13], commands: &[i64]) -> Result<Vec<i64>, FrameError> {
    let host = Host {
        signing: Some(SigningKey::from_bytes(&[0; 32])),
        ..Host::default()
    };
    with_machine(host, |machine| {
        for (slot, value) in fields.into_iter().enumerate() {
            machine.set_field(slot, value)?;
        }
        commands.iter().map(|&c| machine.send(c)).collect()
    })
}

/// A protocol probe after a completed signing of `module` with `signing_key`. With
/// `recipient`, the host also holds that recipient public key and one ephemeral seed.
///
/// # Errors
///
/// The signing's refusal, and [`FrameError::Stage`] for a defect.
pub fn probe_after_signing(
    module: &Module,
    signing_key: &SigningKey,
    #[cfg_attr(not(feature = "encryption"), allow(unused_variables))] recipient: Option<(
        [u8; 32],
        [u8; 32],
    )>,
    commands: &[i64],
) -> Result<Vec<i64>, FrameError> {
    let sections =
        signing_sections(module, false).map_err(|e| FrameError::Stage(format!("{e:?}")))?;
    #[cfg_attr(not(feature = "encryption"), allow(unused_mut))]
    let mut host = Host {
        signing: Some(signing_key.clone()),
        ..Host::default()
    };
    #[cfg(feature = "encryption")]
    if let Some((public, seed)) = recipient {
        host.recipient = Some(public);
        host.seed = Some(seed);
        host.bufs[buf::RID] = crate::encryption::recipient_key_id(&public).to_vec();
    }
    with_machine(host, |machine| {
        sign_on(machine, module, &sections)?;
        commands.iter().map(|&c| machine.send(c)).collect()
    })
}
