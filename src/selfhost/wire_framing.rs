//! Keleusma owns framing layout and incremental integrity computation.
//! Section production is a separate obligation. The host transports opaque bytes.

use super::*;

const WINDOW: usize = 65536;
const HEADER: usize = 64;
const TRAILER: usize = 4;

fn error(detail: impl core::fmt::Display) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire framing: {detail}"),
    }
}

fn with_wire<T>(
    work: impl FnOnce(&mut Vm<'_, '_>, &mut [u8]) -> Result<T, SelfHostError>,
) -> Result<T, SelfHostError> {
    let module = compile_src(&read_stage("kel/wire.kel"));
    with_compiled_wire(module, work)
}

pub(super) fn with_compiled_wire<T>(
    module: Module,
    work: impl FnOnce(&mut Vm<'_, '_>, &mut [u8]) -> Result<T, SelfHostError>,
) -> Result<T, SelfHostError> {
    let need = required_persistent_capacity_for(&module);
    let capacity = DEFAULT_ARENA_CAPACITY
        .checked_add(need)
        .ok_or_else(|| error("arena size overflow"))?;
    let mut arena = Arena::with_capacity(capacity);
    arena
        .resize_persistent(need)
        .map_err(|e| error(format!("arena: {e:?}")))?;
    let mut vm =
        Vm::new(module, &arena).map_err(|e| error(format!("stage verification: {e:?}")))?;
    let mut shared = vec![0; vm.shared_data_bytes()];
    work(&mut vm, &mut shared)
}

pub(super) fn set_word(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    slot: usize,
    value: i64,
) -> Result<(), SelfHostError> {
    vm.set_shared(shared, slot, Value::Int(value))
        .map_err(|e| error(format!("word transport: {e:?}")))
}

pub(super) fn get_word(
    vm: &Vm<'_, '_>,
    shared: &[u8],
    slot: usize,
) -> Result<usize, SelfHostError> {
    match vm
        .get_shared(shared, slot)
        .map_err(|e| error(format!("word transport: {e:?}")))?
    {
        Value::Int(n) => usize::try_from(n).map_err(|_| error("invalid output size")),
        _ => Err(error("expected an integer layout field")),
    }
}

fn write_window(vm: &mut Vm<'_, '_>, shared: &mut [u8], bytes: &[u8]) -> Result<(), SelfHostError> {
    if bytes.len() > WINDOW {
        return Err(error("input exceeds the transport window"));
    }
    for (i, &byte) in bytes.iter().enumerate() {
        vm.set_shared(shared, wire_slots::BYTES + i, Value::Byte(byte))
            .map_err(|e| error(format!("byte transport: {e:?}")))?;
    }
    Ok(())
}

pub(super) fn read_window(
    vm: &Vm<'_, '_>,
    shared: &[u8],
    bytes: &mut [u8],
) -> Result<(), SelfHostError> {
    if bytes.len() > WINDOW {
        return Err(error("output exceeds the transport window"));
    }
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = match vm
            .get_shared(shared, wire_slots::BYTES + i)
            .map_err(|e| error(format!("byte transport: {e:?}")))?
        {
            Value::Byte(byte) => byte,
            _ => return Err(error("expected an output byte")),
        };
    }
    Ok(())
}

pub(super) fn command(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    cmd: i64,
) -> Result<usize, SelfHostError> {
    let result = enter_wire(vm, shared, cmd)?;
    usize::try_from(result).map_err(|_| error(format!("command {cmd} refused with {result}")))
}

fn feed(vm: &mut Vm<'_, '_>, shared: &mut [u8], bytes: &[u8]) -> Result<(), SelfHostError> {
    let mut consumed = 0;
    for window in bytes.chunks(WINDOW) {
        write_window(vm, shared, window)?;
        set_word(vm, shared, wire_slots::WARG, window.len() as i64)?;
        consumed += window.len();
        if command(vm, shared, 189)? != consumed {
            return Err(error("stream cursor disagrees with transport"));
        }
    }
    Ok(())
}

/// Assemble ordinary auxiliary regions with Keleusma-derived directory offsets,
/// word padding, triplicated headers and prologue checksum. Payload bytes remain
/// opaque. Their production and optional error-correction planes are separate.
///
/// # Errors
/// Rejects duplicate kinds, more than 1,024 regions, sizes beyond the framing
/// limit, allocation failures and invalid stage output.
pub fn wire_container_from_regions_via_kel(
    regions: &[(u16, &[u8])],
) -> Result<Vec<u8>, SelfHostError> {
    const REGION_CAP: usize = 1024;
    if regions.len() > REGION_CAP {
        return Err(error("too many auxiliary regions"));
    }
    let mut kinds = alloc::collections::BTreeSet::new();
    // This conservative upper bound checks resource use, not artifact layout.
    // Actual offsets, padded lengths and total size come from the stage.
    let mut upper = 48u64 + 55 * regions.len() as u64;
    for &(kind, bytes) in regions {
        if !kinds.insert(kind) {
            return Err(error("duplicate auxiliary region kind"));
        }
        upper = upper
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| error("container size overflow"))?;
    }
    if upper > u64::from(u32::MAX) {
        return Err(error("container exceeds framing size limit"));
    }
    with_wire(|vm, shared| {
        set_word(vm, shared, wire_slots::NREGIONS, regions.len() as i64)?;
        for (i, &(kind, bytes)) in regions.iter().enumerate() {
            for (column, value) in [i64::from(kind), 0, bytes.len() as i64, 0]
                .into_iter()
                .enumerate()
            {
                set_word(
                    vm,
                    shared,
                    wire_slots::REGION_INPUTS + column * REGION_CAP + i,
                    value,
                )?;
            }
        }
        let header_len = command(vm, shared, 10)?;
        if header_len > WINDOW {
            return Err(error("container header exceeds transport window"));
        }
        let mut positions = Vec::with_capacity(regions.len());
        let mut total = header_len;
        for (i, &(_, bytes)) in regions.iter().enumerate() {
            set_word(vm, shared, wire_slots::WARG, i as i64)?;
            let at = command(vm, shared, 11)?;
            let padded = command(vm, shared, 12)?;
            if at != total || padded < bytes.len() {
                return Err(error("noncontiguous auxiliary layout"));
            }
            total = at
                .checked_add(padded)
                .filter(|&n| n as u64 <= upper)
                .ok_or_else(|| error("invalid auxiliary extent"))?;
            positions.push(at);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(total)
            .map_err(|_| error("container allocation failed"))?;
        output.resize(total, 0);
        read_window(
            vm,
            shared,
            output
                .get_mut(..header_len)
                .ok_or_else(|| error("invalid container header"))?,
        )?;
        for ((_, bytes), at) in regions.iter().zip(positions) {
            let end = at
                .checked_add(bytes.len())
                .ok_or_else(|| error("auxiliary range overflow"))?;
            output
                .get_mut(at..end)
                .ok_or_else(|| error("auxiliary range outside container"))?
                .copy_from_slice(bytes);
        }
        Ok(output)
    })
}

/// Frame already produced sections using Keleusma-derived offsets, padding and checksum.
/// No reference serializer, layout or checksum routine is called here. This function
/// does not establish how the supplied opcode, pool or auxiliary bytes were produced.
/// Signed and encrypted extensions are separate integrations.
///
/// # Errors
/// Returns a stage or transport error for invalid lengths, layout, capacity or execution.
pub fn wire_artifact_from_sections_via_kel(
    module: &Module,
    opcodes: &[u8],
    operands: &[u8],
    auxiliary: &[u8],
) -> Result<Vec<u8>, SelfHostError> {
    let length = |bytes: &[u8]| {
        u32::try_from(bytes.len())
            .map(i64::from)
            .map_err(|_| error("section exceeds u32"))
    };
    let fields = [
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
        length(opcodes)?,
        length(operands)?,
        length(auxiliary)?,
    ];
    with_wire(|vm, shared| {
        for (i, field) in fields.iter().enumerate() {
            set_word(vm, shared, wire_slots::FIN + i, *field)?;
        }
        if command(vm, shared, 188)? != HEADER {
            return Err(error("unexpected header length"));
        }
        let total = get_word(vm, shared, wire_slots::FIN)?;
        let pool_at = get_word(vm, shared, wire_slots::FIN + 1)?;
        let aux_at = get_word(vm, shared, wire_slots::FIN + 2)?;
        let payload_len = total
            .checked_sub(TRAILER)
            .filter(|&n| n >= HEADER)
            .ok_or_else(|| error("invalid total length"))?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(total)
            .map_err(|_| error("artifact allocation failed"))?;
        output.resize(total, 0);
        read_window(vm, shared, &mut output[..HEADER])?;
        for (at, bytes) in [(HEADER, opcodes), (pool_at, operands), (aux_at, auxiliary)] {
            let end = at
                .checked_add(bytes.len())
                .filter(|&n| n <= payload_len)
                .ok_or_else(|| error("section outside artifact"))?;
            output
                .get_mut(at..end)
                .ok_or_else(|| error("invalid section range"))?
                .copy_from_slice(bytes);
        }
        feed(vm, shared, &output[..payload_len])?;
        if command(vm, shared, 190)? != TRAILER {
            return Err(error("unexpected trailer length"));
        }
        read_window(vm, shared, &mut output[payload_len..])?;
        Ok(output)
    })
}

/// Check the unsigned envelope layout and checksum through Keleusma across windows.
/// This does not validate opcode, operand-pool or auxiliary-record semantics.
///
/// # Errors
/// Returns transport or execution errors. Malformed envelopes and checksum mismatches return false.
pub fn wire_framing_is_intact_via_kel(bytes: &[u8]) -> Result<bool, SelfHostError> {
    if bytes.len() < HEADER + TRAILER || u32::try_from(bytes.len()).is_err() {
        return Ok(false);
    }
    with_wire(|vm, shared| {
        write_window(vm, shared, &bytes[..HEADER])?;
        set_word(vm, shared, wire_slots::WARG, bytes.len() as i64)?;
        match command(vm, shared, 191)? {
            0 => return Ok(false),
            1 => {}
            _ => return Err(error("unexpected envelope verdict")),
        }
        feed(vm, shared, bytes)?;
        match command(vm, shared, 192)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(error("unexpected checksum verdict")),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn begin(vm: &mut Vm<'_, '_>, shared: &mut [u8], fields: [i64; 13]) -> i64 {
        for (i, value) in fields.into_iter().enumerate() {
            set_word(vm, shared, wire_slots::FIN + i, value).unwrap();
        }
        enter_wire(vm, shared, 188).unwrap()
    }

    #[test]
    fn framing_fields_are_checked_before_header_writes() {
        with_wire(|vm, shared| {
            for index in 0..13 {
                for value in [-1, if index < 4 { 256 } else { 0x1_0000_0000 }] {
                    write_window(vm, shared, &[0xA5; HEADER])?;
                    let mut fields = [0; 13];
                    fields[index] = value;
                    assert_eq!(
                        begin(vm, shared, fields),
                        -263,
                        "field {index}, value {value}"
                    );
                    let mut actual = [0; HEADER];
                    read_window(vm, shared, &mut actual)?;
                    assert_eq!(actual, [0xA5; HEADER]);
                    assert_eq!(enter_wire(vm, shared, 190)?, -260);
                }
            }
            let mut fields = [0; 13];
            fields[12] = 0xFFFF_FFFF - 68;
            assert_eq!(begin(vm, shared, fields), 64);
            assert_eq!(get_word(vm, shared, wire_slots::FIN)?, 0xFFFF_FFFF);
            fields[12] += 1;
            assert_eq!(begin(vm, shared, fields), -264);
            assert_eq!(enter_wire(vm, shared, 190)?, -260);
            fields = [0; 13];
            fields[10] = 0xFFFF_FFFC;
            assert_eq!(begin(vm, shared, fields), -264);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn stream_protocol_requires_exact_length_and_preserves_state_on_refusal() {
        with_wire(|vm, shared| {
            assert_eq!(enter_wire(vm, shared, 189)?, -260);
            assert_eq!(begin(vm, shared, [0; 13]), 64);
            for length in [-1, 0, 65, 65537] {
                set_word(vm, shared, wire_slots::WARG, length)?;
                assert_eq!(enter_wire(vm, shared, 189)?, -261);
            }
            set_word(vm, shared, wire_slots::WARG, 32)?;
            assert_eq!(enter_wire(vm, shared, 189)?, 32);
            assert_eq!(enter_wire(vm, shared, 190)?, -262);
            assert_eq!(enter_wire(vm, shared, 192)?, -260);
            // The legacy checksum command would replace the live accumulator.
            assert_eq!(enter_wire(vm, shared, 0)?, -260);
            assert_eq!(enter_wire(vm, shared, 193)?, -260);
            assert_eq!(enter_wire(vm, shared, 194)?, -260);
            for cmd in 195..=217 {
                assert_eq!(enter_wire(vm, shared, cmd)?, -260);
            }
            assert_eq!(enter_wire(vm, shared, 189)?, 64);
            assert_eq!(enter_wire(vm, shared, 190)?, 4);
            assert_eq!(enter_wire(vm, shared, 190)?, -260);

            let mut fields = [0; 13];
            fields[12] = WINDOW as i64;
            assert_eq!(begin(vm, shared, fields), 64);
            let bytes = vec![0xA5; WINDOW + HEADER];
            feed(vm, shared, &bytes)?;
            assert_eq!(enter_wire(vm, shared, 190)?, 4);
            let mut trailer = [0; 4];
            read_window(vm, shared, &mut trailer)?;
            assert_eq!(u32::from_le_bytes(trailer), keleusma_wire::crc32(&bytes));
            Ok(())
        })
        .unwrap();
    }
}
