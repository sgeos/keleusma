//! Signature numbering is derived in Keleusma from semantic shape fields.

use super::wire_framing::{command, read_window, set_word, with_compiled_wire};
use super::*;
use crate::bytecode::{ChunkSignature, WireShape};

/// Shape and signature records with Keleusma-derived table ordinals.
pub struct WireSignatureSections {
    /// Shape records in first-append order, including unshared parameter runs.
    pub shapes: Vec<u8>,
    /// Signature records referring to those shape records.
    pub signatures: Vec<u8>,
}

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire signatures: {detail}"),
    }
}

fn append(
    vm: &Vm<'_, '_>,
    shared: &[u8],
    count: usize,
    output: &mut Vec<u8>,
) -> Result<(), SelfHostError> {
    let at = output.len();
    let end = at
        .checked_add(count)
        .ok_or_else(|| error("size overflow"))?;
    output
        .try_reserve(count)
        .map_err(|_| error("allocation failed"))?;
    output.resize(end, 0);
    read_window(vm, shared, &mut output[at..])
}

/// Derive shape indices and parameter ranges through the Keleusma wire stage.
///
/// The host transports semantic variants and previously emitted records. The
/// stage appends parameters without sharing and finds the earliest identical
/// return or resume shape. Searches use bounded windows, so the stage need not
/// hold the complete table. The reference signature-table helper is not called.
///
/// # Errors
/// Refuses field or table overflow, stage execution errors, unexpected protocol
/// output and allocation failures.
pub fn wire_signature_sections_via_kel(
    signatures: &[ChunkSignature],
) -> Result<WireSignatureSections, SelfHostError> {
    signature_and_native_sections(signatures, &[]).map(|(sections, _)| sections)
}

fn emit_shape(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    shape: &WireShape,
    parameter: bool,
    shapes: &mut Vec<u8>,
) -> Result<(), SelfHostError> {
    let fields = match shape {
        WireShape::Top => [0, 0, 0],
        WireShape::Scalar { kind } => [1, i64::from(*kind), 0],
        WireShape::Flat { kind, size } => [2, i64::from(*kind), i64::from(*size)],
    };
    for (j, value) in fields.into_iter().enumerate() {
        set_word(vm, shared, wire_slots::FIN + j, value)?;
    }
    let mut wrote = command(vm, shared, 197)?;
    if parameter {
        if wrote != 8 {
            return Err(error("parameter was not appended"));
        }
    } else {
        if wrote != 0 {
            return Err(error("unexpected search start"));
        }
        for window in shapes.chunks(49152) {
            for (j, &byte) in window.iter().enumerate() {
                vm.set_shared(shared, wire_slots::BIN + j, Value::Byte(byte))
                    .map_err(|_| error("record transport failed"))?;
            }
            set_word(vm, shared, wire_slots::WARG, window.len() as i64)?;
            if command(vm, shared, 198)? != 0 {
                return Err(error("unexpected search result"));
            }
        }
        wrote = command(vm, shared, 199)?;
        if !matches!(wrote, 0 | 8) {
            return Err(error("unexpected intern result"));
        }
    }
    append(vm, shared, wrote, shapes)
}

// Native return shapes continue the same interner after all chunk signatures.
// Their vector length is independent of the native-name vector length.
pub(super) fn signature_and_native_sections(
    signatures: &[ChunkSignature],
    native_returns: &[WireShape],
) -> Result<(WireSignatureSections, Vec<u8>), SelfHostError> {
    let mut result = WireSignatureSections {
        shapes: Vec::new(),
        signatures: Vec::new(),
    };
    let mut natives = Vec::new();
    with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
        if command(vm, shared, 195)? != 0 {
            return Err(error("unexpected reset result"));
        }
        for sig in signatures {
            let count =
                u32::try_from(sig.params.len()).map_err(|_| error("too many parameters"))?;
            set_word(vm, shared, wire_slots::WARG, i64::from(count))?;
            if command(vm, shared, 196)? != 0 {
                return Err(error("unexpected begin result"));
            }
            for (i, shape) in sig.params.iter().chain([&sig.ret, &sig.resume]).enumerate() {
                emit_shape(vm, shared, shape, i < sig.params.len(), &mut result.shapes)?;
            }
            if command(vm, shared, 200)? != 16 {
                return Err(error("unexpected signature result"));
            }
            append(vm, shared, 16, &mut result.signatures)?;
        }
        for shape in native_returns {
            if command(vm, shared, 203)? != 0 {
                return Err(error("unexpected native start"));
            }
            emit_shape(vm, shared, shape, false, &mut result.shapes)?;
            if command(vm, shared, 204)? != 8 {
                return Err(error("unexpected native record"));
            }
            append(vm, shared, 8, &mut natives)?;
        }
        Ok((result, natives))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_return_protocol_requires_completed_interning() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            assert!(command(vm, shared, 203).is_err());
            assert!(command(vm, shared, 204).is_err());
            command(vm, shared, 195)?;
            command(vm, shared, 203)?;
            assert!(command(vm, shared, 204).is_err());
            let mut shapes = Vec::new();
            emit_shape(vm, shared, &WireShape::Top, false, &mut shapes)?;
            for cmd in [
                196, 197, 198, 199, 200, 201, 202, 203, 205, 206, 207, 208, 209, 210, 211, 212,
                213, 214, 215, 216, 217, 218, 219,
            ] {
                assert!(command(vm, shared, cmd).is_err(), "command {cmd}");
            }
            assert_eq!(command(vm, shared, 204)?, 8);
            let mut native = [1; 8];
            read_window(vm, shared, &mut native)?;
            assert_eq!(native, [0; 8]);
            assert!(command(vm, shared, 204).is_err());
            set_word(vm, shared, wire_slots::WARG, 0)?;
            command(vm, shared, 196)?;
            emit_shape(vm, shared, &WireShape::Top, false, &mut shapes)?;
            assert!(command(vm, shared, 204).is_err());
            emit_shape(vm, shared, &WireShape::Top, false, &mut shapes)?;
            assert_eq!(command(vm, shared, 200)?, 16);
            assert_eq!(shapes.len(), 8);
            Ok(())
        })
        .unwrap();
    }

    fn fields(vm: &mut Vm<'_, '_>, shared: &mut [u8], values: [i64; 3]) {
        for (i, value) in values.into_iter().enumerate() {
            set_word(vm, shared, wire_slots::FIN + i, value).unwrap();
        }
    }

    #[test]
    fn signature_refusals_preserve_pending_record_and_position() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            assert!(command(vm, shared, 196).is_err());
            command(vm, shared, 195)?;
            for count in [-1, 0x1_0000_0000] {
                set_word(vm, shared, wire_slots::WARG, count)?;
                assert!(command(vm, shared, 196).is_err());
            }
            set_word(vm, shared, wire_slots::WARG, 1)?;
            command(vm, shared, 196)?;
            // No records have been accepted. An early finish must not commit.
            assert!(command(vm, shared, 200).is_err());
            for malformed in [
                [-1, 0, 0],
                [3, 0, 0],
                [0, 1, 0],
                [0, 0, 1],
                [1, -1, 0],
                [1, 256, 0],
                [1, 1, 1],
                [2, 1, -1],
                [2, 1, 0x1_0000_0000],
            ] {
                fields(vm, shared, malformed);
                assert!(command(vm, shared, 197).is_err());
            }
            fields(vm, shared, [1, 1, 0]);
            assert_eq!(command(vm, shared, 197)?, 8);
            let mut parameter = [0; 8];
            read_window(vm, shared, &mut parameter)?;
            // Return matches the parameter. Reject incomplete and excessive scans.
            fields(vm, shared, [1, 1, 0]);
            assert_eq!(command(vm, shared, 197)?, 0);
            assert!(command(vm, shared, 199).is_err());
            for count in [-1, 1, 16, 49153] {
                set_word(vm, shared, wire_slots::WARG, count)?;
                assert!(command(vm, shared, 198).is_err());
            }
            for cmd in [0, 179, 188, 193, 194, 196, 197, 200] {
                assert!(command(vm, shared, cmd).is_err(), "command {cmd}");
            }
            let mut unchanged = [0; 8];
            read_window(vm, shared, &mut unchanged)?;
            assert_eq!(unchanged, parameter);
            for (i, byte) in parameter.into_iter().enumerate() {
                vm.set_shared(shared, wire_slots::BIN + i, Value::Byte(byte))
                    .unwrap();
            }
            set_word(vm, shared, wire_slots::WARG, 8)?;
            command(vm, shared, 198)?;
            assert!(command(vm, shared, 198).is_err());
            assert_eq!(command(vm, shared, 199)?, 0);
            fields(vm, shared, [0, 0, 0]);
            command(vm, shared, 197)?;
            command(vm, shared, 198)?;
            assert_eq!(command(vm, shared, 199)?, 8);
            assert_eq!(command(vm, shared, 200)?, 16);
            let mut record = [0; 16];
            read_window(vm, shared, &mut record)?;
            assert_eq!(record, [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0]);
            assert!(command(vm, shared, 200).is_err());
            // Explicit reset discards an unfinished signature and numbering.
            set_word(vm, shared, wire_slots::WARG, 0)?;
            command(vm, shared, 196)?;
            command(vm, shared, 195)?;
            command(vm, shared, 196)?;
            Ok(())
        })
        .unwrap();
    }
}
