//! Semantic auxiliary decoding driven by the dedicated schema stage.

use super::wire_directory::decode_directory;
use super::wire_framing::{get_word, set_word, with_compiled_wire};
use super::wire_records::decode_records;
use super::*;
use crate::bytecode::{ChunkSignature, WireShape};
use crate::wire_schema::{ShapeRecord, SignatureRecord, kind};

pub(super) const ARG: usize = 0;
pub(super) const FIN: usize = 8;
const BYTES: usize = FIN + 16;
const NAME_WINDOW: usize = 1024;

/// Signature-family results with the optional-table decision preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireDecodedSignatures {
    /// Whether the shared shape/signature view passed its parsing checks.
    /// False represents the reference auxiliary decoder's optional-table policy.
    pub parse_valid: bool,
    /// Per-chunk signature values.
    pub signatures: Vec<ChunkSignature>,
    /// Native return shapes, independent of the native-name vector length.
    pub native_return_shapes: Vec<WireShape>,
}

fn error(detail: impl core::fmt::Display) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire semantics: {detail}"),
    }
}

pub(super) fn command(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    cmd: i64,
) -> Result<usize, SelfHostError> {
    let result = enter_wire(vm, shared, cmd)?;
    usize::try_from(result).map_err(|_| error(format!("command {cmd} refused with {result}")))
}

pub(super) fn fields(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    values: &[i64],
) -> Result<(), SelfHostError> {
    if values.len() > 16 {
        return Err(error("field transport exceeds schema window"));
    }
    for (i, &value) in values.iter().enumerate() {
        set_word(vm, shared, FIN + i, value)?;
    }
    Ok(())
}

fn signature(record: &WireSchemaRecord) -> Result<&SignatureRecord, SelfHostError> {
    match record {
        WireSchemaRecord::SignatureRecord(record) => Ok(record),
        _ => Err(error("unexpected signature record kind")),
    }
}

fn send_signature(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    record: &SignatureRecord,
) -> Result<(), SelfHostError> {
    fields(
        vm,
        shared,
        &[
            i64::from(record.params_first),
            i64::from(record.params_count),
            i64::from(record.ret),
            i64::from(record.resume),
        ],
    )
}

fn materialize_shape(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    record: &ShapeRecord,
) -> Result<WireShape, SelfHostError> {
    fields(
        vm,
        shared,
        &[
            i64::from(record.tag),
            i64::from(record.kind),
            i64::from(record.reserved),
            i64::from(record.size),
        ],
    )?;
    let form = command(vm, shared, 3)?;
    let kind =
        u8::try_from(get_word(vm, shared, FIN)?).map_err(|_| error("shape kind exceeds byte"))?;
    let size = u32::try_from(get_word(vm, shared, FIN + 1)?)
        .map_err(|_| error("shape size exceeds u32"))?;
    match form {
        0 => Ok(WireShape::Top),
        1 => Ok(WireShape::Scalar { kind }),
        2 => Ok(WireShape::Flat { kind, size }),
        _ => Err(error("unknown semantic shape form")),
    }
}

fn requested_shape(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    shapes: &[WireSchemaRecord],
) -> Result<WireShape, SelfHostError> {
    let index = get_word(vm, shared, FIN)?;
    match shapes.get(index) {
        Some(WireSchemaRecord::ShapeRecord(record)) => materialize_shape(vm, shared, record),
        _ => Err(error("shape request leaves decoded table")),
    }
}

fn decode_signature(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    record: &SignatureRecord,
    shapes: &[WireSchemaRecord],
) -> Result<ChunkSignature, SelfHostError> {
    send_signature(vm, shared, record)?;
    if command(vm, shared, 4)? != 0 {
        return Err(error("unexpected signature start"));
    }
    let count = get_word(vm, shared, FIN + 1)?;
    let limit = count
        .checked_add(3)
        .ok_or_else(|| error("signature request count overflow"))?;
    let mut params = Vec::new();
    params
        .try_reserve_exact(count)
        .map_err(|_| error("parameter allocation failed"))?;
    let mut ret = None;
    let mut resume = None;
    let mut completed = false;
    for _ in 0..limit {
        match command(vm, shared, 5)? {
            0 => {
                completed = true;
                break;
            }
            1 => {}
            _ => return Err(error("unknown signature request")),
        }
        let role = get_word(vm, shared, FIN + 1)?;
        let shape = requested_shape(vm, shared, shapes)?;
        match role {
            0 if params.len() < count => params.push(shape),
            1 if ret.is_none() => ret = Some(shape),
            2 if resume.is_none() => resume = Some(shape),
            _ => return Err(error("invalid or repeated signature role")),
        }
    }
    if !completed || params.len() != count {
        return Err(error("incomplete signature"));
    }
    Ok(ChunkSignature {
        params,
        ret: ret.ok_or_else(|| error("missing return shape"))?,
        resume: resume.ok_or_else(|| error("missing resume shape"))?,
    })
}

pub(super) fn byte_count(bytes: Option<&[u8]>) -> Result<i64, SelfHostError> {
    bytes.map_or(Ok(-1), |bytes| {
        i64::try_from(bytes.len()).map_err(|_| error("table exceeds word range"))
    })
}

fn decode_metadata(bytes: &[u8], strict: bool) -> Result<WireDecodedSignatures, SelfHostError> {
    let codec = compile_src(&read_stage("kel/wire.kel"));
    let semantic = compile_src(&read_stage("kel/wire_schema.kel"));
    with_compiled_wire(codec, |codec, codec_shared| {
        let directory = decode_directory(codec, codec_shared, bytes)?;
        let region = |kind| {
            directory
                .regions
                .iter()
                .find(|r| r.kind == kind)
                .map(|r| r.bytes)
        };
        let shapes = region(kind::SHAPES);
        let signatures = region(kind::SIGNATURES);
        // The standalone signature reader does not inspect native-return tables.
        let natives = if strict {
            None
        } else {
            region(kind::NATIVE_RETURNS)
        };
        with_compiled_wire(semantic, |vm, shared| {
            for (i, value) in [
                byte_count(shapes)?,
                byte_count(signatures)?,
                byte_count(natives)?,
                i64::from(strict),
            ]
            .into_iter()
            .enumerate()
            {
                set_word(vm, shared, ARG + i, value)?;
            }
            let mut result = WireDecodedSignatures {
                parse_valid: false,
                signatures: Vec::new(),
                native_return_shapes: Vec::new(),
            };
            match command(vm, shared, 0)? {
                0 => return Ok(result),
                1 => {}
                _ => return Err(error("unknown table parsing state")),
            }
            let signature_count = get_word(vm, shared, FIN + 1)?;
            let native_count = get_word(vm, shared, FIN + 2)?;
            let signatures = match signatures {
                Some(bytes) => decode_records(codec, codec_shared, kind::SIGNATURES, bytes)?,
                None => Vec::new(),
            };
            if signatures.len() != signature_count {
                return Err(error("signature count disagreement"));
            }
            for record in &signatures {
                send_signature(vm, shared, signature(record)?)?;
                if command(vm, shared, 1)? != 0 {
                    return Err(error("unexpected signature validation response"));
                }
            }
            match command(vm, shared, 2)? {
                0 => return Ok(result),
                1 => result.parse_valid = true,
                _ => return Err(error("unknown table validation state")),
            }
            let shapes = decode_records(
                codec,
                codec_shared,
                kind::SHAPES,
                shapes.ok_or_else(|| error("parsed view has no shape table"))?,
            )?;
            result
                .signatures
                .try_reserve_exact(signature_count)
                .map_err(|_| error("signature allocation failed"))?;
            for record in &signatures {
                result
                    .signatures
                    .push(decode_signature(vm, shared, signature(record)?, &shapes)?);
            }
            let natives = match natives {
                Some(bytes) => decode_records(codec, codec_shared, kind::NATIVE_RETURNS, bytes)?,
                None => Vec::new(),
            };
            if natives.len() != native_count {
                return Err(error("native return count disagreement"));
            }
            result
                .native_return_shapes
                .try_reserve_exact(native_count)
                .map_err(|_| error("native shape allocation failed"))?;
            for record in &natives {
                let WireSchemaRecord::NativeReturnRecord(record) = record else {
                    return Err(error("unexpected native return record"));
                };
                set_word(vm, shared, ARG, i64::from(record.shape))?;
                if command(vm, shared, 6)? != 1 {
                    return Err(error("unexpected native shape request"));
                }
                result
                    .native_return_shapes
                    .push(requested_shape(vm, shared, &shapes)?);
            }
            if command(vm, shared, 7)? != 0 {
                return Err(error("incomplete schema transaction"));
            }
            Ok(result)
        })
    })
}

/// Decode signatures with the standalone schema reader's required-table policy.
///
/// Keleusma checks every signature range before requesting any shape and owns
/// shape-tag interpretation and parameter/return/resume selection. Unused invalid
/// shape tags remain unobserved, as in the reference. Kind bytes are retained for
/// later verifier checks. Native-return tables are outside this interface.
///
/// # Errors
/// Refuses a missing or malformed shape/signature view, invalid referenced shape
/// tags, transport failures and allocation failures.
pub fn wire_decode_signatures_via_kel(bytes: &[u8]) -> Result<Vec<ChunkSignature>, SelfHostError> {
    Ok(decode_metadata(bytes, true)?.signatures)
}

/// Decode the signature family using the auxiliary decoder's optional-table policy.
///
/// Malformed shape/signature views produce an empty result only when no native
/// return needs that view. Invalid tags reached through a valid view remain errors.
/// Other auxiliary families are not validated by this component.
///
/// # Errors
/// Refuses malformed native-return records, native references through an invalid
/// view, invalid referenced shape tags, and transport or allocation failures.
pub fn wire_decode_signature_metadata_via_kel(
    bytes: &[u8],
) -> Result<WireDecodedSignatures, SelfHostError> {
    decode_metadata(bytes, false)
}

// Both returned spans are supplied by the schema stage. The host only checks
// them against the borrowed buffer before transport or typed construction.
fn requested_bytes<'a>(
    vm: &Vm<'_, '_>,
    shared: &[u8],
    bytes: &'a [u8],
) -> Result<&'a [u8], SelfHostError> {
    let start = get_word(vm, shared, FIN)?;
    let length = get_word(vm, shared, FIN + 1)?;
    let end = start
        .checked_add(length)
        .ok_or_else(|| error("name span overflow"))?;
    bytes
        .get(start..end)
        .ok_or_else(|| error("name request leaves borrowed region"))
}

fn validate_name_bytes(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    bytes: &[u8],
) -> Result<(), SelfHostError> {
    for chunk in bytes.chunks(NAME_WINDOW) {
        for (i, &byte) in chunk.iter().enumerate() {
            vm.set_shared(shared, BYTES + i, Value::Byte(byte))
                .map_err(|_| error("name byte transport failed"))?;
        }
        set_word(vm, shared, ARG, chunk.len() as i64)?;
        if command(vm, shared, 10)? != 0 {
            return Err(error("unexpected name feed response"));
        }
    }
    if command(vm, shared, 11)? != 0 {
        return Err(error("name validation did not finish"));
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct NameTables<'a> {
    pub(super) names: Option<&'a [u8]>,
    pub(super) pool: Option<&'a [u8]>,
}

pub(super) fn name_lookup<'a>(
    codec: &mut Vm<'_, '_>,
    codec_shared: &mut [u8],
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    tables: NameTables<'a>,
    index: u32,
) -> Result<&'a str, SelfHostError> {
    for (i, value) in [
        byte_count(tables.names)?,
        byte_count(tables.pool)?,
        i64::from(index),
    ]
    .into_iter()
    .enumerate()
    {
        set_word(vm, shared, ARG + i, value)?;
    }
    if command(vm, shared, 8)? != 1 {
        return Err(error("unexpected name record request"));
    }
    let name_bytes = requested_bytes(
        vm,
        shared,
        tables
            .names
            .ok_or_else(|| error("name table absent after validation"))?,
    )?;
    let mut records = decode_records(codec, codec_shared, kind::NAMES, name_bytes)?;
    if records.len() != 1 {
        return Err(error("name request did not select one record"));
    }
    let Some(WireSchemaRecord::NameRef(record)) = records.pop() else {
        return Err(error("unexpected name record"));
    };
    fields(
        vm,
        shared,
        &[i64::from(record.offset), i64::from(record.length)],
    )?;
    if command(vm, shared, 9)? != 1 {
        return Err(error("unexpected name pool request"));
    }
    let value = requested_bytes(
        vm,
        shared,
        tables
            .pool
            .ok_or_else(|| error("string pool absent after validation"))?,
    )?;
    validate_name_bytes(vm, shared, value)?;
    // Checked Rust construction remains a defensive boundary. Acceptance has
    // already been decided by the stage, which is tested without this check.
    core::str::from_utf8(value).map_err(|_| error("schema stage returned invalid text"))
}

/// Resolve and validate one name through Keleusma, retaining the input's storage.
///
/// Only the requested name is checked. Unused malformed name records and pool
/// bytes do not change acceptance. Keleusma validates the name index, pool span
/// and UTF-8 bytes through bounded windows. Rust returns a borrowed string.
///
/// # Errors
/// Refuses missing regions, invalid indices or spans, malformed UTF-8, and
/// transport or allocation failures.
pub fn wire_name_via_kel(bytes: &[u8], index: u32) -> Result<&str, SelfHostError> {
    with_compiled_wire(
        compile_src(&read_stage("kel/wire.kel")),
        |codec, codec_shared| {
            let directory = decode_directory(codec, codec_shared, bytes)?;
            let region = |kind| {
                directory
                    .regions
                    .iter()
                    .find(|r| r.kind == kind)
                    .map(|r| r.bytes)
            };
            let tables = NameTables {
                names: region(kind::NAMES),
                pool: region(kind::STRING_POOL),
            };
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| name_lookup(codec, codec_shared, vm, shared, tables, index),
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending_return(vm: &mut Vm<'_, '_>, shared: &mut [u8]) -> Result<(), SelfHostError> {
        for (i, value) in [8, 16, -1, 1].into_iter().enumerate() {
            set_word(vm, shared, ARG + i, value)?;
        }
        assert_eq!(command(vm, shared, 0)?, 1);
        fields(vm, shared, &[0, 0, 0, 0])?;
        assert_eq!(command(vm, shared, 1)?, 0);
        assert_eq!(command(vm, shared, 2)?, 1);
        assert_eq!(command(vm, shared, 4)?, 0);
        assert_eq!(command(vm, shared, 5)?, 1);
        assert_eq!(get_word(vm, shared, FIN)?, 0);
        assert_eq!(get_word(vm, shared, FIN + 1)?, 1);
        Ok(())
    }

    #[test]
    fn every_shape_tag_and_kind_matches_reference_value_semantics() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                let mut accepted = 0;
                let mut refused = 0;
                for tag in 0..=u16::MAX {
                    if tag <= 4 {
                        pending_return(vm, shared)?;
                    }
                    let record = ShapeRecord {
                        tag,
                        kind: 255,
                        reserved: 255,
                        size: u32::MAX,
                    };
                    match (materialize_shape(vm, shared, &record), record.to_shape()) {
                        (Ok(a), Some(b)) => {
                            assert_eq!(a, b);
                            accepted += 1;
                        }
                        (Err(_), None) => refused += 1,
                        (a, b) => panic!("tag {tag}: {a:?} versus {b:?}"),
                    }
                }
                assert_eq!((accepted, refused), (3, 65533));
                let mut kinds = 0;
                for tag in 1..=3 {
                    for kind in 0..=u8::MAX {
                        for size in [0, 1, u32::MAX] {
                            pending_return(vm, shared)?;
                            let record = ShapeRecord {
                                tag,
                                kind,
                                reserved: 255,
                                size,
                            };
                            assert_eq!(
                                materialize_shape(vm, shared, &record)?,
                                record.to_shape().unwrap()
                            );
                            if tag == 1 {
                                assert_eq!(get_word(vm, shared, FIN)?, 0);
                            }
                            if tag != 3 {
                                assert_eq!(get_word(vm, shared, FIN + 1)?, 0);
                            }
                            kinds += 1;
                        }
                    }
                }
                assert_eq!(kinds, 2304);
                std::eprintln!(
                    "shape tags {accepted} accepted, {refused} refused; {kinds} kind/size controls"
                );
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn schema_protocol_requires_complete_requests_and_preserves_failed_reply() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                for cmd in 1..=7 {
                    assert!(command(vm, shared, cmd).is_err());
                }
                pending_return(vm, shared)?;
                for cmd in [1, 2, 4, 5, 6, 7] {
                    assert!(command(vm, shared, cmd).is_err());
                }
                let bad = ShapeRecord {
                    tag: 0,
                    kind: 0,
                    reserved: 0,
                    size: 0,
                };
                assert!(materialize_shape(vm, shared, &bad).is_err());
                let good = ShapeRecord {
                    tag: 2,
                    kind: 255,
                    reserved: 0,
                    size: u32::MAX,
                };
                assert_eq!(
                    materialize_shape(vm, shared, &good)?,
                    WireShape::Scalar { kind: 255 }
                );
                assert_eq!(command(vm, shared, 5)?, 1);
                assert_eq!(get_word(vm, shared, FIN + 1)?, 2);
                assert_eq!(
                    materialize_shape(vm, shared, &good)?,
                    WireShape::Scalar { kind: 255 }
                );
                assert_eq!(command(vm, shared, 5)?, 0);
                assert_eq!(command(vm, shared, 7)?, 0);
                assert!(command(vm, shared, 7).is_err());
                pending_return(vm, shared)?;
                set_word(vm, shared, ARG, -2)?;
                assert!(command(vm, shared, 0).is_err());
                assert!(command(vm, shared, 3).is_err());
                Ok(())
            },
        )
        .unwrap();
    }
    #[test]
    fn native_only_requests_require_valid_replies_and_exact_completion() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                for (i, value) in [8, -1, 8, 0].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                assert_eq!(command(vm, shared, 0)?, 1);
                assert_eq!(command(vm, shared, 2)?, 1);
                assert!(command(vm, shared, 7).is_err());
                set_word(vm, shared, ARG, 1)?;
                assert!(command(vm, shared, 6).is_err());
                set_word(vm, shared, ARG, 0)?;
                assert_eq!(command(vm, shared, 6)?, 1);
                assert!(command(vm, shared, 6).is_err());
                let bad = ShapeRecord {
                    tag: 0,
                    kind: 255,
                    reserved: 255,
                    size: u32::MAX,
                };
                assert!(materialize_shape(vm, shared, &bad).is_err());
                let good = ShapeRecord { tag: 3, ..bad };
                assert_eq!(
                    materialize_shape(vm, shared, &good)?,
                    WireShape::Flat {
                        kind: 255,
                        size: u32::MAX
                    }
                );
                assert!(command(vm, shared, 6).is_err());
                assert_eq!(command(vm, shared, 7)?, 0);
                Ok(())
            },
        )
        .unwrap();
    }
    fn utf8_only(
        vm: &mut Vm<'_, '_>,
        shared: &mut [u8],
        bytes: &[u8],
    ) -> Result<bool, SelfHostError> {
        for (i, value) in [8, bytes.len() as i64, 0].into_iter().enumerate() {
            set_word(vm, shared, ARG + i, value)?;
        }
        assert_eq!(command(vm, shared, 8)?, 1);
        fields(vm, shared, &[0, bytes.len() as i64])?;
        assert_eq!(command(vm, shared, 9)?, 1);
        match validate_name_bytes(vm, shared, bytes) {
            Ok(()) => Ok(true),
            Err(SelfHostError::Unsupported { detail }) if detail.contains("refused with -298") => {
                Ok(false)
            }
            Err(error) => Err(error),
        }
    }

    #[test]
    fn utf8_stage_verdict_matches_rust_without_host_string_construction() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                let mut accepted = 0;
                let mut refused = 0;
                let mut check = |bytes: &[u8]| -> Result<(), SelfHostError> {
                    let actual = utf8_only(vm, shared, bytes)?;
                    assert_eq!(actual, core::str::from_utf8(bytes).is_ok(), "{bytes:?}");
                    if actual {
                        accepted += 1;
                    } else {
                        refused += 1;
                    }
                    Ok(())
                };
                check(&[])?;
                for a in 0..=255u8 {
                    check(&[a])?;
                    for b in 0..=255u8 {
                        check(&[a, b])?;
                        check(&[a, b, 128])?;
                        check(&[a, b, 128, 128])?;
                    }
                    check(&[225, 128, a])?;
                    check(&[241, 128, a, 128])?;
                    check(&[241, 128, 128, a])?;
                }
                for sequence in [
                    &[224, 160, 128][..],
                    &[237, 159, 191],
                    &[237, 160, 128],
                    &[240, 144, 128, 128],
                    &[244, 143, 191, 191],
                    &[244, 144, 128, 128],
                    &[239, 191, 191],
                    &[226, 130],
                ] {
                    for split in 1021..=1025 {
                        let mut bytes = vec![b'x'; split];
                        bytes.extend_from_slice(sequence);
                        check(&bytes)?;
                    }
                }
                assert!(accepted > 1000 && refused > 1000);
                assert_eq!(accepted + refused, 197673);
                std::eprintln!("UTF-8 cases accepted {accepted}, refused {refused}");
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn name_protocol_refusals_and_other_transactions_preserve_state() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                assert!(command(vm, shared, 9).is_err());
                for (i, value) in [8, 4, 0].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                assert_eq!(command(vm, shared, 8)?, 1);
                fields(vm, shared, &[3, 2])?;
                assert!(command(vm, shared, 9).is_err());
                fields(vm, shared, &[0, 4])?;
                assert_eq!(command(vm, shared, 9)?, 1);
                for cmd in [0, 1, 2, 3, 4, 5, 6, 7, 12, 16, 20, 21] {
                    assert!(command(vm, shared, cmd).is_err());
                }
                for len in [-1, 0, 5, 1025] {
                    set_word(vm, shared, ARG, len)?;
                    assert!(command(vm, shared, 10).is_err());
                }
                assert!(command(vm, shared, 11).is_err());
                validate_name_bytes(vm, shared, b"name")?;
                assert!(command(vm, shared, 11).is_err());
                pending_return(vm, shared)?;
                assert!(command(vm, shared, 8).is_err());
                Ok(())
            },
        )
        .unwrap();
    }
}
