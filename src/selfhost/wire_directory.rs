//! Auxiliary directory interpretation without reference offsets or payload copies.

use super::wire_decode::transport;
use super::wire_framing::{command, get_word, set_word, with_compiled_wire};
use super::*;

/// One region whose location was decoded and checked by Keleusma.
#[derive(Debug, PartialEq, Eq)]
pub struct WireAuxiliaryRegion<'a> {
    /// Schema identifier, including identifiers unknown to the compiler.
    pub kind: u16,
    /// Flags carried without assigning schema meaning.
    pub flags: u16,
    /// Protected region identifier for a parity plane.
    pub covers: u16,
    /// Byte offset from the start of the auxiliary container.
    pub byte_offset: usize,
    /// Original region storage, including any format padding.
    pub bytes: &'a [u8],
}

/// A decoded directory retaining borrowed region payloads.
#[derive(Debug, PartialEq, Eq)]
pub struct WireAuxiliaryDirectory<'a> {
    /// Regions in directory order.
    pub regions: Vec<WireAuxiliaryRegion<'a>>,
    /// Whether any prologue or directory copy disagrees with its majority vote.
    /// This is a header repair signal, not a payload integrity verdict.
    pub needs_scrub: bool,
}

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire directory: {detail}"),
    }
}

fn decode_directory<'a>(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    bytes: &'a [u8],
) -> Result<WireAuxiliaryDirectory<'a>, SelfHostError> {
    let prologue = bytes.get(..48).ok_or_else(|| error("truncated prologue"))?;
    transport(vm, shared, 0, prologue)?;
    set_word(
        vm,
        shared,
        wire_slots::WARG,
        i64::try_from(bytes.len()).map_err(|_| error("container length exceeds word range"))?,
    )?;
    let header = command(vm, shared, 216)?;
    let count = get_word(vm, shared, wire_slots::FIN)?;
    if count > 1024 {
        return Err(error("directory count exceeds format ceiling"));
    }
    transport(
        vm,
        shared,
        0,
        bytes
            .get(..header)
            .ok_or_else(|| error("truncated directory"))?,
    )?;
    let mut regions = Vec::new();
    regions
        .try_reserve_exact(count)
        .map_err(|_| error("directory allocation failed"))?;
    for _ in 0..count {
        if command(vm, shared, 217)? != 1 {
            return Err(error("directory ended before its declared count"));
        }
        let mut fields = [0; 5];
        for (i, field) in fields.iter_mut().enumerate() {
            *field = get_word(vm, shared, wire_slots::FIN + i)?;
        }
        let narrow = |n| u16::try_from(n).map_err(|_| error("region metadata exceeds u16"));
        let [kind, flags, covers, start, length] = fields;
        let end = start
            .checked_add(length)
            .ok_or_else(|| error("region span overflow"))?;
        regions.push(WireAuxiliaryRegion {
            kind: narrow(kind)?,
            flags: narrow(flags)?,
            covers: narrow(covers)?,
            byte_offset: start,
            bytes: bytes
                .get(start..end)
                .ok_or_else(|| error("region leaves container"))?,
        });
    }
    if command(vm, shared, 217)? != 0 {
        return Err(error("directory exceeded its declared count"));
    }
    let needs_scrub = match get_word(vm, shared, wire_slots::FIN)? {
        0 => false,
        1 => true,
        _ => return Err(error("invalid disagreement result")),
    };
    Ok(WireAuxiliaryDirectory {
        regions,
        needs_scrub,
    })
}

/// Decode and validate an auxiliary container directory through Keleusma.
///
/// Keleusma votes the triplicated headers, checks the prologue, derives region
/// byte spans, and rejects duplicate kinds and out-of-bounds payloads. Unknown
/// flags and kinds, overlapping spans and reserved fields retain reference
/// behavior. Only header bytes enter the stage's existing window. Payloads are
/// borrowed directly from the input, regardless of their size. The host allocates
/// at most 1,024 region descriptors, in addition to the stage runtime.
///
/// This does not interpret schema records or verify payload integrity.
///
/// # Errors
/// Refuses truncated or invalid headers, duplicate kinds, invalid region spans,
/// allocation failures and stage protocol failures.
pub fn wire_auxiliary_directory_via_kel(
    bytes: &[u8],
) -> Result<WireAuxiliaryDirectory<'_>, SelfHostError> {
    with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
        decode_directory(vm, shared, bytes)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use keleusma_wire::{WireBuilder, WireView};

    fn fixture() -> Vec<u8> {
        let mut builder = WireBuilder::new();
        let a = builder.region(10, 0).unwrap();
        builder.push(a, &[1, 2, 3]);
        let b = builder.region(65535, 65535).unwrap();
        builder.push(b, &[4; 16]);
        builder.finish().unwrap()
    }

    fn compare(vm: &mut Vm<'_, '_>, shared: &mut [u8], bytes: &[u8]) -> bool {
        let actual = decode_directory(vm, shared, bytes);
        let reference = WireView::parse(bytes);
        match (actual, reference) {
            (Ok(actual), Ok(reference)) => {
                assert_eq!(actual.needs_scrub, reference.needs_scrub());
                assert_eq!(actual.regions.len(), usize::from(reference.region_count()));
                for (i, region) in actual.regions.iter().enumerate() {
                    let expected = reference.region_at(i as u16).unwrap();
                    assert_eq!(region.kind, expected.kind);
                    assert_eq!(region.flags, expected.flags);
                    assert_eq!(region.covers, expected.covers);
                    assert_eq!(region.byte_offset, expected.byte_offset().unwrap());
                    let expected_bytes = reference.region_bytes(&expected).unwrap();
                    assert_eq!(region.bytes, expected_bytes);
                    assert_eq!(region.bytes.as_ptr(), expected_bytes.as_ptr());
                }
                true
            }
            (Err(_), Err(_)) => false,
            (actual, reference) => panic!("directory disagreement {actual:?} versus {reference:?}"),
        }
    }

    #[test]
    fn directory_votes_bounds_and_accepted_padding_match_reference() {
        let clean = fixture();
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            assert!(compare(vm, shared, &clean));
            let mut accepted = 0;
            let mut refused = 0;
            // Every one-bit header fault is repaired by the majority vote.
            for byte in 0..144 {
                for bit in 0..8 {
                    let mut damaged = clean.clone();
                    damaged[byte] ^= 1 << bit;
                    assert!(compare(vm, shared, &damaged), "byte {byte}, bit {bit}");
                    accepted += 1;
                }
            }
            // Change all three prologues together, with valid checksums.
            for (field, width, value) in [
                (0, 4, 0u32),
                (4, 2, 0xFFFE),
                (6, 2, 1),
                (8, 2, 1025),
                (8, 2, 1024),
                (10, 2, 65535),
            ] {
                let mut changed = clean.clone();
                let mut prologue: [u8; 16] = clean[..16].try_into().unwrap();
                prologue[field..field + width].copy_from_slice(&value.to_le_bytes()[..width]);
                let checksum = keleusma_wire::crc32(&prologue[..12]);
                prologue[12..16].copy_from_slice(&checksum.to_le_bytes());
                for copy in 0..3 {
                    changed[copy * 16..copy * 16 + 16].copy_from_slice(&prologue);
                }
                if compare(vm, shared, &changed) {
                    accepted += 1;
                } else {
                    refused += 1;
                }
            }
            // A voted checksum failure must not be mistaken for repairable disagreement.
            let mut bad_checksum = clean.clone();
            for copy in 0..3 {
                bad_checksum[copy * 16 + 12] ^= 1;
            }
            assert!(!compare(vm, shared, &bad_checksum));
            refused += 1;
            // Identical changes in every directory copy exercise semantic checks.
            for field in [0, 2, 4, 8, 12, 14] {
                for value in [0u32, 1, 10, 18, 65535, u32::MAX] {
                    let mut changed = clean.clone();
                    let width = if field == 4 || field == 8 { 4 } else { 2 };
                    for copy in 0..3 {
                        let at = 48 + copy * 32 + 16 + field;
                        changed[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]);
                    }
                    if compare(vm, shared, &changed) {
                        accepted += 1;
                    } else {
                        refused += 1;
                    }
                }
            }
            for end in 0..clean.len() {
                if compare(vm, shared, &clean[..end]) {
                    accepted += 1;
                } else {
                    refused += 1;
                }
            }
            assert!(accepted >= 1152 && refused > 0);
            std::eprintln!("directory mutations accepted {accepted}, refused {refused}");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn failed_record_retry_and_restart_preserve_directory_state() {
        let clean = fixture();
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            assert!(command(vm, shared, 217).is_err());
            transport(vm, shared, 0, &clean[..48])?;
            set_word(vm, shared, wire_slots::WARG, clean.len() as i64)?;
            assert_eq!(command(vm, shared, 216)?, 144);
            transport(vm, shared, 0, &clean[..144])?;
            for cmd in [0, 2, 188, 195, 203, 209, 213, 214, 215, 218, 219] {
                assert!(command(vm, shared, cmd).is_err(), "command {cmd}");
            }
            assert_eq!(command(vm, shared, 217)?, 1);
            // A new begin discards a partially consumed directory.
            assert_eq!(command(vm, shared, 216)?, 144);
            assert_eq!(command(vm, shared, 217)?, 1);
            assert_eq!(get_word(vm, shared, wire_slots::FIN)?, 10);
            let mut bad = clean.clone();
            for copy in 0..3 {
                bad[48 + copy * 32 + 16] = 10;
                bad[48 + copy * 32 + 17] = 0;
            }
            transport(vm, shared, 0, &bad[..144])?;
            assert!(command(vm, shared, 217).is_err());
            transport(vm, shared, 0, &clean[..144])?;
            assert_eq!(command(vm, shared, 217)?, 1);
            assert_eq!(command(vm, shared, 217)?, 0);
            assert!(command(vm, shared, 217).is_err());
            assert_eq!(command(vm, shared, 216)?, 144);
            set_word(vm, shared, wire_slots::WARG, -1)?;
            assert!(command(vm, shared, 216).is_err());
            assert!(command(vm, shared, 217).is_err());
            assert!(compare(vm, shared, &clean));
            Ok(())
        })
        .unwrap();
    }
}
