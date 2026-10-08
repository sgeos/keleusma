//! Module-driven auxiliary assembly without reference bytes or region positions.
//! Scalar metadata remains host supplied. Keleusma owns signature numbering,
//! record encoding, name interning and container layout.

use super::wire_framing::{command, read_window, set_word, with_compiled_wire};
use super::*;

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire auxiliary: {detail}"),
    }
}

// The historical packer uses infallible u16 conversions. Validate every count
// and name it will visit before calling it. Constants have already been checked
// to be scalar, so their root count is also their node count.
fn validate_input(module: &Module) -> Result<(), SelfHostError> {
    let fits_name = |name: &str| name.len() <= u16::MAX as usize;
    let mut events = module
        .chunks
        .len()
        .checked_add(module.native_names.len())
        .ok_or_else(|| error("name count overflow"))?;
    if module.native_names.iter().any(|n| !fits_name(n)) {
        return Err(error("native name exceeds the input field width"));
    }
    let mut nodes = 0usize;
    for chunk in &module.chunks {
        if !fits_name(&chunk.name) {
            return Err(error("chunk name exceeds the input field width"));
        }
        nodes = nodes
            .checked_add(chunk.constants.len())
            .ok_or_else(|| error("node count overflow"))?;
    }
    if nodes > u16::MAX as usize || module.enum_layouts.len() > 1024 {
        return Err(error("module table exceeds the input field width"));
    }
    for layout in &module.enum_layouts {
        if !fits_name(&layout.type_name) || layout.variants.iter().any(|v| !fits_name(&v.name)) {
            return Err(error("enum name exceeds the input field width"));
        }
        events = events
            .checked_add(layout.variants.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| error("name count overflow"))?;
    }
    if let Some(layout) = &module.data_layout {
        if layout.slots.iter().any(|s| !fits_name(&s.name)) {
            return Err(error("slot name exceeds the input field width"));
        }
        let runs = usize::from(!layout.slots.is_empty())
            + layout
                .slots
                .windows(2)
                .filter(|pair| {
                    pair[0].name != pair[1].name || pair[0].visibility != pair[1].visibility
                })
                .count();
        events = events
            .checked_add(runs)
            .ok_or_else(|| error("name count overflow"))?;
    }
    if events > 1024 {
        return Err(error("name events exceed the stage capacity"));
    }
    Ok(())
}

struct Emitter {
    stage: Module,
    blob: Vec<u8>,
    header: [i64; 11],
    chunks: usize,
    native_at: Option<usize>,
}

impl Emitter {
    fn run<T>(
        &self,
        work: impl FnOnce(&mut Vm<'_, '_>, &mut [u8]) -> Result<T, SelfHostError>,
    ) -> Result<T, SelfHostError> {
        with_compiled_wire(self.stage.clone(), |vm, shared| {
            for (i, &value) in self.header.iter().enumerate() {
                set_word(vm, shared, wire_slots::FIN + HEADER_FIELD_BASE + i, value)?;
            }
            for (i, &byte) in self.blob.iter().enumerate() {
                vm.set_shared(shared, wire_slots::BIN + i, Value::Byte(byte))
                    .map_err(|_| error("input byte transport failed"))?;
            }
            if let Some(at) = self.native_at {
                set_word(vm, shared, wire_slots::WARG, at as i64)?;
                set_word(vm, shared, wire_slots::WARG + 1, self.blob.len() as i64)?;
                if command(vm, shared, 201)? != 0 {
                    return Err(error("native input setup failed"));
                }
            }
            set_word(vm, shared, wire_slots::WARG, self.chunks as i64)?;
            work(vm, shared)
        })
    }

    fn append(
        vm: &mut Vm<'_, '_>,
        shared: &mut [u8],
        cmd: i64,
        output: &mut Vec<u8>,
    ) -> Result<(), SelfHostError> {
        let wrote = command(vm, shared, cmd)?;
        if wrote > STAGE_BUFFER_BYTES {
            return Err(error("record output exceeds the transport window"));
        }
        let at = output.len();
        let end = at
            .checked_add(wrote)
            .ok_or_else(|| error("size overflow"))?;
        output
            .try_reserve(wrote)
            .map_err(|_| error("allocation failed"))?;
        output.resize(end, 0);
        read_window(vm, shared, &mut output[at..])
    }

    fn single(&self, cmd: i64) -> Result<Vec<u8>, SelfHostError> {
        self.run(|vm, shared| {
            let mut bytes = Vec::new();
            Self::append(vm, shared, cmd, &mut bytes)?;
            Ok(bytes)
        })
    }

    fn stream(
        &self,
        fields: &[i64],
        width: usize,
        begin: Option<i64>,
        step: i64,
        kind: Option<u16>,
    ) -> Result<Vec<u8>, SelfHostError> {
        if width == 0 || width > 1024 || !fields.len().is_multiple_of(width) {
            return Err(error("invalid record field grouping"));
        }
        if fields.is_empty() {
            return Ok(Vec::new());
        }
        self.run(|vm, shared| {
            if let Some(cmd) = begin {
                command(vm, shared, cmd)?;
            }
            if let Some(kind) = kind {
                set_word(vm, shared, wire_slots::WARG, i64::from(kind))?;
                set_word(vm, shared, wire_slots::WARG + 1, 1)?;
                set_word(vm, shared, wire_slots::WARG + 2, 0)?;
            }
            let mut bytes = Vec::new();
            for row in fields.chunks_exact(width) {
                for (i, &value) in row.iter().enumerate() {
                    set_word(vm, shared, wire_slots::FIN + i, value)?;
                }
                let before = bytes.len();
                Self::append(vm, shared, step, &mut bytes)?;
                if bytes.len() == before {
                    return Err(error("record stream made no progress"));
                }
            }
            Ok(bytes)
        })
    }
}

/// Encode the auxiliary container from module facts, with no reference artifact.
///
/// This integrates the stage-corpus records and native metadata. Templates,
/// debug pools, composite constants and private composite slots still
/// require integration. They are refused explicitly. Shape numbering is derived
/// in Keleusma, and parameter tags remain opaque bytes.
/// This function is not yet the shipping serializer.
///
/// # Errors
/// Returns an unsupported error for an unintegrated record form, a stage capacity
/// limit, invalid stage output or allocation failure.
pub fn wire_auxiliary_from_module_via_kel(module: &Module) -> Result<Vec<u8>, SelfHostError> {
    use crate::wire_schema::kind;

    if module
        .chunks
        .iter()
        .any(|c| !c.struct_templates.is_empty() || c.debug_pool.is_some())
    {
        return Err(error("template and debug records are not integrated"));
    }
    if module
        .data_layout
        .as_ref()
        .is_some_and(|dl| !dl.private_composite_layout.is_empty())
    {
        return Err(error("private composite records are not integrated"));
    }
    if !crate::wire_schema::constant_roots_of_module(module)
        .iter()
        .all(constant_is_scalar)
    {
        return Err(error("composite and named constants are not integrated"));
    }
    let init = if module.data_layout.is_some() {
        Some(
            data_init_fields(module)
                .ok_or_else(|| error("private initializer pool is not integrated"))?,
        )
    } else {
        None
    };
    validate_input(module)?;
    let (mut blob, names) = module_input(module);
    let native_at = if module.native_names.is_empty() {
        None
    } else {
        let at = blob.len();
        let extra = module
            .native_names
            .iter()
            .try_fold(2usize, |size, name| {
                size.checked_add(2)?.checked_add(name.len())
            })
            .filter(|&size| at.checked_add(size).is_some_and(|end| end <= 49152))
            .ok_or_else(|| error("native input exceeds the stage capacity"))?;
        blob.try_reserve_exact(extra)
            .map_err(|_| error("native input allocation failed"))?;
        let count =
            u16::try_from(module.native_names.len()).map_err(|_| error("native count overflow"))?;
        blob.extend_from_slice(&count.to_le_bytes());
        for name in &module.native_names {
            let len =
                u16::try_from(name.len()).map_err(|_| error("native name length overflow"))?;
            blob.extend_from_slice(&len.to_le_bytes());
            blob.extend_from_slice(name.as_bytes());
        }
        Some(at)
    };
    if blob.len() > 49152 || names > 1024 {
        return Err(error("module input exceeds the stage capacity"));
    }
    let (signatures, native_returns) = super::wire_signatures::signature_and_native_sections(
        &module.signatures,
        &module.native_return_shapes,
    )?;
    let mut header = header_fields_of(module);
    header[4..10].copy_from_slice(&[
        i64::from(module.flags),
        i64::from(module.wcet_cycles),
        i64::from(module.wcmu_bytes),
        i64::from(module.shared_data_bytes),
        i64::from(module.private_data_bytes),
        i64::from(module.schema_hash),
    ]);
    let emitter = Emitter {
        stage: compile_src(&read_stage("kel/wire.kel")),
        blob,
        header,
        chunks: module.chunks.len(),
        native_at,
    };
    let mut chunks = chunk_fields_of(module);
    let mut op_offset = 0u32;
    for (row, chunk) in chunks
        .as_chunks_mut::<11>()
        .0
        .iter_mut()
        .zip(&module.chunks)
    {
        let count = u32::try_from(chunk.ops.len()).map_err(|_| error("opcode count overflow"))?;
        row[6] = i64::from(op_offset);
        row[7] = i64::from(count);
        op_offset = count
            .checked_mul(4)
            .and_then(|n| op_offset.checked_add(n))
            .ok_or_else(|| error("opcode section exceeds u32"))?;
    }
    // Declaration order is part of the format. Even empty mandatory tables
    // receive directory entries, while per-chunk tables require a contributor.
    let mut regions = vec![
        (kind::SIGNATURES, signatures.signatures),
        (
            kind::ENUM_VARIANTS,
            emitter.stream(&enum_variant_fields(module), 3, Some(184), 185, None)?,
        ),
        (
            kind::ENUM_LAYOUTS,
            emitter.stream(&enum_layout_fields(module), 2, Some(186), 187, None)?,
        ),
        (
            kind::NATIVES,
            emitter.stream(
                &(0..module.native_names.len())
                    .map(|i| i as i64)
                    .collect::<Vec<_>>(),
                1,
                Some(182),
                202,
                None,
            )?,
        ),
        (kind::NATIVE_RETURNS, native_returns),
    ];
    if let Some(init) = init {
        regions.extend([
            (
                kind::DATA_SLOTS,
                emitter.stream(&slot_run_fields(module), 4, Some(182), 183, None)?,
            ),
            (
                kind::SHARED_LAYOUT,
                emitter.stream(
                    &shared_slot_fields(module),
                    7,
                    None,
                    164,
                    Some(kind::SHARED_LAYOUT),
                )?,
            ),
            (kind::PRIVATE_COMPOSITE, Vec::new()),
            (
                kind::DATA_INIT,
                emitter.stream(&init, 2, None, 164, Some(kind::DATA_INIT))?,
            ),
        ]);
    }
    regions.push((kind::HEADER, emitter.single(172)?));
    let wants_constants = !module.chunks.is_empty()
        || module
            .data_layout
            .as_ref()
            .is_some_and(|dl| !crate::wire_schema::private_init_is_elided(dl));
    if wants_constants {
        regions.extend([
            (
                kind::CONSTS,
                window_emit_consts(&emitter.stage, &const_node_fields(module))?,
            ),
            (kind::STRUCT_AUX, Vec::new()),
            (kind::ENUM_AUX, Vec::new()),
        ]);
    }
    if !module.chunks.is_empty() {
        regions.push((kind::STRUCT_TEMPLATES, Vec::new()));
        regions.push((kind::PARAM_TYPES, param_type_bytes(module)));
    }
    regions.push((kind::SHAPES, signatures.shapes));
    if !module.chunks.is_empty() {
        regions.push((
            kind::CHUNKS,
            emitter.stream(&chunks, 11, Some(174), 175, None)?,
        ));
    }
    regions.push((kind::STRING_POOL, emitter.single(171)?));
    regions.push((kind::NAMES, emitter.single(170)?));
    let borrowed: Vec<_> = regions
        .iter()
        .map(|(kind, bytes)| (*kind, bytes.as_slice()))
        .collect();
    wire_container_from_regions_via_kel(&borrowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_transport_rejects_incomplete_names_and_invalid_ranges() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            // Four empty legacy sections followed by the optional native section.
            for i in 0..8 {
                vm.set_shared(shared, wire_slots::BIN + i, Value::Byte(0))
                    .unwrap();
            }
            for (start, end) in [
                (-1, 0),
                (i64::MAX, i64::MAX),
                (8, 9),
                (49151, 49152),
                (8, 49153),
            ] {
                set_word(vm, shared, wire_slots::WARG, start)?;
                set_word(vm, shared, wire_slots::WARG + 1, end)?;
                assert!(command(vm, shared, 201).is_err());
            }
            for bytes in [
                &[1, 0][..],
                &[1, 0, 3, 0, b'a'],
                &[0, 0, 0],
                &[255, 255],
                &[1, 0, 1, 1],
            ] {
                for (i, &byte) in bytes.iter().enumerate() {
                    vm.set_shared(shared, wire_slots::BIN + 8 + i, Value::Byte(byte))
                        .unwrap();
                }
                set_word(vm, shared, wire_slots::WARG, 8)?;
                set_word(vm, shared, wire_slots::WARG + 1, (8 + bytes.len()) as i64)?;
                command(vm, shared, 201)?;
                assert!(command(vm, shared, 202).is_err());
                assert!(command(vm, shared, 182).is_err());
                assert!(command(vm, shared, 202).is_err());
            }
            for (i, byte) in [1, 0, 1, 0, b'x'].into_iter().enumerate() {
                vm.set_shared(shared, wire_slots::BIN + 8 + i, Value::Byte(byte))
                    .unwrap();
            }
            set_word(vm, shared, wire_slots::WARG, 8)?;
            set_word(vm, shared, wire_slots::WARG + 1, 13)?;
            command(vm, shared, 201)?;
            assert_eq!(command(vm, shared, 182)?, 1);
            set_word(vm, shared, wire_slots::FIN, 0)?;
            assert_eq!(command(vm, shared, 202)?, 8);
            let mut before = [0; 8];
            read_window(vm, shared, &mut before)?;
            for index in [-1, 1, i64::MAX] {
                set_word(vm, shared, wire_slots::FIN, index)?;
                assert!(command(vm, shared, 202).is_err());
                let mut after = [0; 8];
                read_window(vm, shared, &mut after)?;
                assert_eq!(before, after);
            }
            Ok(())
        })
        .unwrap();
    }
}
