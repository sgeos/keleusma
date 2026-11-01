//! Windowed auxiliary emission. Semantic indices are stage decisions; host maps
//! and queues retain storage between bounded stage requests.

use super::wire_framing::{set_word, with_compiled_wire};
use super::wire_semantics::{ARG, BYTES, FIN};
use super::*;
use crate::wire_schema::kind;
use alloc::collections::BTreeMap;

fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("streaming auxiliary: {detail}"),
    }
}
fn size(n: usize) -> Result<i64, SelfHostError> {
    i64::try_from(n).map_err(|_| error("count exceeds transport word"))
}
fn index(n: i64) -> Result<usize, SelfHostError> {
    usize::try_from(n).map_err(|_| error("negative or oversized index"))
}
fn append(out: &mut Vec<u8>, values: &[u8]) -> Result<(), SelfHostError> {
    out.try_reserve(values.len())
        .map_err(|_| error("output allocation failed"))?;
    out.extend_from_slice(values);
    Ok(())
}
pub(super) struct Machine<'a> {
    vm: &'a mut dyn StageExecutor,
    shared: &'a mut [u8],
    arg: usize,
    fin: usize,
    bytes: usize,
    window: usize,
}
impl Machine<'_> {
    pub(super) fn args(&mut self, values: &[i64]) -> Result<(), SelfHostError> {
        if values.len() > 8 {
            return Err(error("argument window exceeded"));
        }
        for (i, &n) in values.iter().enumerate() {
            set_word(self.vm, self.shared, self.arg + i, n)?;
        }
        Ok(())
    }
    pub(super) fn fields(&mut self, values: &[i64]) -> Result<(), SelfHostError> {
        if values.len() > 16 {
            return Err(error("field window exceeded"));
        }
        for (i, &n) in values.iter().enumerate() {
            set_word(self.vm, self.shared, self.fin + i, n)?;
        }
        Ok(())
    }
    pub(super) fn output<const N: usize>(&self) -> Result<[i64; N], SelfHostError> {
        if N > 16 {
            return Err(error("field response window exceeded"));
        }
        let mut result = [0; N];
        for (i, n) in result.iter_mut().enumerate() {
            *n = match self
                .vm
                .get_shared(self.shared, self.fin + i)
                .map_err(|_| error("field read failed"))?
            {
                Value::Int(n) => n,
                _ => return Err(error("non-integer field response")),
            };
        }
        Ok(result)
    }
    pub(super) fn command(&mut self, cmd: i64) -> Result<usize, SelfHostError> {
        let result = enter_wire(self.vm, self.shared, cmd)?;
        usize::try_from(result).map_err(|_| SelfHostError::Unsupported {
            detail: format!("streaming auxiliary command {cmd} refused with {result}"),
        })
    }
    fn accept(&mut self, cmd: i64) -> Result<(), SelfHostError> {
        if self.command(cmd)? != 0 {
            return Err(error("unexpected command response"));
        }
        Ok(())
    }
    pub(super) fn write(&mut self, offset: usize, bytes: &[u8]) -> Result<(), SelfHostError> {
        if offset
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.window)
        {
            return Err(error("byte window exceeded"));
        }
        for (i, &b) in bytes.iter().enumerate() {
            self.vm
                .set_shared(self.shared, self.bytes + offset + i, Value::Byte(b))
                .map_err(|_| error("byte write failed"))?;
        }
        Ok(())
    }
    fn read(&self, count: usize, out: &mut Vec<u8>) -> Result<(), SelfHostError> {
        if count > self.window {
            return Err(error("byte response exceeded window"));
        }
        out.try_reserve(count)
            .map_err(|_| error("byte allocation failed"))?;
        for i in 0..count {
            out.push(
                match self
                    .vm
                    .get_shared(self.shared, self.bytes + i)
                    .map_err(|_| error("byte read failed"))?
                {
                    Value::Byte(b) => b,
                    _ => return Err(error("non-byte response")),
                },
            );
        }
        Ok(())
    }
    fn record(
        &mut self,
        kind: u16,
        fields: &[i64],
        out: &mut Vec<u8>,
    ) -> Result<(), SelfHostError> {
        self.fields(fields)?;
        let command = match kind {
            kind::STRUCT_AUX => {
                self.args(&[0, 1])?;
                128
            }
            kind::ENUM_AUX => {
                self.args(&[0, 1])?;
                129
            }
            _ => {
                self.args(&[kind.into(), 1, 0])?;
                164
            }
        };
        let count = self.command(command)?;
        if count == 0 {
            return Err(error("record stream made no progress"));
        }
        self.read(count, out)
    }
}

#[derive(Clone, Copy)]
struct NameIndex(i64);
#[derive(Default)]
struct Names<'a> {
    // Bucket keys come exclusively from Keleusma. Hashes only narrow candidates;
    // Keleusma compares bytes and selects the last equal entry, including fresh ones.
    buckets: BTreeMap<u32, Vec<usize>>,
    strings: Vec<&'a str>,
    records: Vec<u8>,
    pool: Vec<u8>,
}
impl<'a> Names<'a> {
    fn prefix(
        &mut self,
        schema: &mut Machine<'_>,
        codec: &mut Machine<'_>,
        text: &'a str,
        expected: (i64, usize, usize),
        variants: usize,
    ) -> Result<NameIndex, SelfHostError> {
        if schema.command(86)? != 1 {
            return Err(error("name prefix ended before its input"));
        }
        let request = schema.output::<3>()?;
        if request != [expected.0, size(expected.1)?, size(expected.2)?] {
            return Err(error("name input disagrees with stage traversal order"));
        }
        let name = self.intern(schema, codec, text, request[0])?;
        schema.args(&[size(variants)?])?;
        schema.accept(87)?;
        Ok(name)
    }

    fn intern(
        &mut self,
        schema: &mut Machine<'_>,
        codec: &mut Machine<'_>,
        text: &'a str,
        role: i64,
    ) -> Result<NameIndex, SelfHostError> {
        schema.args(&[size(text.len())?, role])?;
        schema.accept(65)?;
        for bytes in text.as_bytes().chunks(1024) {
            schema.write(0, bytes)?;
            schema.args(&[size(bytes.len())?])?;
            schema.accept(66)?;
        }
        schema.accept(67)?;
        let hash =
            u32::try_from(schema.output::<1>()?[0]).map_err(|_| error("invalid hash response"))?;
        let candidates = self.buckets.get(&hash).map(Vec::as_slice).unwrap_or(&[]);
        let budget = candidates
            .len()
            .checked_mul(text.len() / 512 + 3)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(|| error("name request budget overflow"))?;
        schema.args(&[size(candidates.len())?])?;
        let mut event = schema.command(68)?;
        let mut candidate = None;
        for _ in 0..budget {
            match event {
                0 => {
                    let p = schema.output::<5>()?;
                    if u32::try_from(p[4]).ok() != Some(hash) || index(p[2])? != text.len() {
                        return Err(error("name result disagrees with request"));
                    }
                    let selected = index(p[0])?;
                    match p[3] {
                        0 => {
                            self.strings
                                .get(selected)
                                .ok_or_else(|| error("unknown shared name"))?;
                        }
                        1 => {
                            if selected != self.strings.len() || index(p[1])? != self.pool.len() {
                                return Err(error("name append coordinates disagree"));
                            }
                            codec.record(kind::NAMES, &[p[1], p[2]], &mut self.records)?;
                            append(&mut self.pool, text.as_bytes())?;
                            self.strings
                                .try_reserve(1)
                                .map_err(|_| error("name storage allocation failed"))?;
                            self.strings.push(text);
                            let bucket = self.buckets.entry(hash).or_default();
                            bucket
                                .try_reserve(1)
                                .map_err(|_| error("name bucket allocation failed"))?;
                            bucket.push(selected);
                        }
                        _ => return Err(error("unknown name append decision")),
                    }
                    return Ok(NameIndex(p[0]));
                }
                1 => {
                    let at = index(schema.output::<1>()?[0])?;
                    let id = *candidates
                        .get(at)
                        .ok_or_else(|| error("unknown hash-bucket position"))?;
                    let value = *self
                        .strings
                        .get(id)
                        .ok_or_else(|| error("missing name candidate"))?;
                    candidate = Some(value);
                    schema.args(&[size(id)?, size(value.len())?])?;
                    event = schema.command(69)?;
                }
                2 => {
                    let [start, count] = schema.output::<2>()?;
                    let start = index(start)?;
                    let count = index(count)?;
                    let end = start
                        .checked_add(count)
                        .ok_or_else(|| error("name span overflow"))?;
                    let source = text
                        .as_bytes()
                        .get(start..end)
                        .ok_or_else(|| error("invalid input-name window"))?;
                    let previous = candidate
                        .ok_or_else(|| error("comparison without candidate"))?
                        .as_bytes()
                        .get(start..end)
                        .ok_or_else(|| error("invalid candidate window"))?;
                    schema.write(0, source)?;
                    schema.write(512, previous)?;
                    schema.args(&[size(count)?])?;
                    event = schema.command(70)?;
                }
                _ => return Err(error("unknown name request")),
            }
        }
        Err(error("name request budget exhausted"))
    }
}

fn child_count(value: &ConstValue) -> usize {
    match value {
        ConstValue::Tuple(v) | ConstValue::Array(v) | ConstValue::Enum { fields: v, .. } => v.len(),
        ConstValue::Struct { fields, .. } => fields.len(),
        _ => 0,
    }
}
fn append_children<'a>(
    queue: &mut Vec<&'a ConstValue>,
    value: &'a ConstValue,
) -> Result<(), SelfHostError> {
    queue
        .try_reserve(child_count(value))
        .map_err(|_| error("node queue allocation failed"))?;
    match value {
        ConstValue::Tuple(v) | ConstValue::Array(v) | ConstValue::Enum { fields: v, .. } => {
            queue.extend(v.iter())
        }
        ConstValue::Struct { fields, .. } => queue.extend(fields.iter().map(|(_, v)| v)),
        _ => {}
    }
    Ok(())
}
struct Forest {
    constants: Vec<u8>,
    structs: Vec<u8>,
    enums: Vec<u8>,
}
fn forest<'a>(
    schema: &mut Machine<'_>,
    codec: &mut Machine<'_>,
    names: &mut Names<'a>,
    module: &'a Module,
    include_initializers: bool,
) -> Result<Forest, SelfHostError> {
    let roots = module.chunks.iter().flat_map(|c| &c.constants).chain(
        module
            .data_layout
            .iter()
            .flat_map(|d| &d.private_init)
            .take(if include_initializers { usize::MAX } else { 0 }),
    );
    let count = roots.clone().count();
    schema.args(&[size(count)?])?;
    schema.accept(71)?;
    let mut queue = Vec::new();
    queue
        .try_reserve_exact(count)
        .map_err(|_| error("root allocation failed"))?;
    queue.extend(roots);
    let mut result = Forest {
        constants: Vec::new(),
        structs: Vec::new(),
        enums: Vec::new(),
    };
    for ordinal in 0..=u32::MAX {
        match schema.command(72)? {
            0 => return Ok(result),
            1 => {}
            _ => return Err(error("unknown node request")),
        }
        let at = index(schema.output::<1>()?[0])?;
        if at != ordinal as usize {
            return Err(error("node queue did not advance monotonically"));
        }
        let value = *queue
            .get(at)
            .ok_or_else(|| error("unavailable node request"))?;
        let (tag, payload) = const_tag_and_name(value);
        let (flags, disc) = const_flags_and_discriminant(value);
        schema.args(&[tag.into(), payload, size(child_count(value))?, flags, disc])?;
        schema.accept(73)?;
        let contributed = blob_node_names(value);
        for _ in 0..=contributed.len() {
            match schema.command(74)? {
                0 => break,
                1 => {}
                _ => return Err(error("unknown node-name request")),
            }
            let [name, role] = schema.output::<2>()?;
            names.intern(
                schema,
                codec,
                contributed
                    .get(index(name)?)
                    .copied()
                    .ok_or_else(|| error("missing node name"))?,
                role,
            )?;
            schema.accept(75)?;
        }
        schema.accept(76)?;
        let p = schema.output::<10>()?;
        codec.record(kind::CONSTS, &p[..4], &mut result.constants)?;
        match p[4] {
            0 => {}
            1 => codec.record(kind::STRUCT_AUX, &p[5..7], &mut result.structs)?,
            2 => codec.record(kind::ENUM_AUX, &p[5..8], &mut result.enums)?,
            _ => return Err(error("unknown auxiliary node kind")),
        }
        if index(p[8])? != queue.len() || index(p[9])? != child_count(value) {
            return Err(error("child append coordinates disagree"));
        }
        append_children(&mut queue, value)?;
        schema.args(&[size(queue.len())?])?;
        schema.accept(77)?;
    }
    Err(error("node queue exceeds wire index range"))
}
struct Initializer {
    elided: bool,
    record: Vec<u8>,
}
fn initializer(
    codec: &mut Machine<'_>,
    module: &Module,
) -> Result<Option<Initializer>, SelfHostError> {
    let Some(layout) = &module.data_layout else {
        return Ok(None);
    };
    codec.args(&[size(module.chunks.len())?, size(layout.private_init.len())?])?;
    codec.accept(209)?;
    for chunk in &module.chunks {
        codec.args(&[size(chunk.constants.len())?])?;
        codec.accept(210)?;
    }
    // Eight scalar facts fit the generic sixteen-word transport wrapper.
    for batch in layout.private_init.chunks(8) {
        let fields: Vec<_> = batch
            .iter()
            .flat_map(|v| {
                let (t, p) = const_tag_and_name(v);
                [i64::from(t), p]
            })
            .collect();
        codec.fields(&fields)?;
        codec.args(&[size(batch.len())?])?;
        codec.accept(211)?;
    }
    if codec.command(212)? != 8 {
        return Err(error("initializer record length disagrees"));
    }
    let elided = match codec.output::<1>()?[0] {
        0 => false,
        1 => true,
        _ => return Err(error("invalid initializer decision")),
    };
    let mut record = Vec::new();
    codec.read(8, &mut record)?;
    Ok(Some(Initializer { elided, record }))
}
fn parameter_code(value: crate::bytecode::TypeTag) -> i64 {
    use crate::bytecode::TypeTag::*;
    match value {
        Composite => 0,
        Byte => 1,
        Word => 2,
        Fixed => 3,
        Float => 4,
        Bool => 5,
        Unit => 6,
        Text => 7,
    }
}

/// Emit the complete auxiliary body without corpus-sized resident name or forest tables.
///
/// Keleusma owns contributor order, hash keys and collision equality, fresh/share decisions, name
/// offsets and indices, breadth-first node scheduling, child ranges, auxiliary
/// numbering, template ranges, chunk ranges and parameter-tag encoding. The host
/// retains typed values and indexed storage and transports bounded windows.
/// Data-slot groups, shared strides and record splitting are stage-derived from
/// logical slot facts through bounded metadata and byte-comparison batches.
///
/// # Errors
/// Refuses values outside wire field widths, allocation failures and invalid
/// stage/transport responses. These windows impose no fixed name-length, name-event,
/// forest-node or aggregate module-input cap.
pub fn wire_auxiliary_from_module_via_kel(module: &Module) -> Result<Vec<u8>, SelfHostError> {
    with_compiled_wire(
        compile_src(&read_stage("kel/wire.kel")),
        |wire, wire_shared| {
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |semantic, semantic_shared| {
                    let mut codec = Machine {
                        vm: wire,
                        shared: wire_shared,
                        arg: wire_slots::WARG,
                        fin: wire_slots::FIN,
                        bytes: wire_slots::BYTES,
                        window: 65536,
                    };
                    let mut schema = Machine {
                        vm: semantic,
                        shared: semantic_shared,
                        arg: ARG,
                        fin: FIN,
                        bytes: BYTES,
                        window: 1024,
                    };
                    schema.accept(64)?;
                    let init = initializer(&mut codec, module)?;
                    let include_initializers = init.as_ref().is_some_and(|i| !i.elided);
                    let (runs, shared_rows) = if let Some(layout) = &module.data_layout {
                        (
                            super::wire_run_encode::slot_runs(&mut schema, &layout.slots)?,
                            super::wire_run_encode::shared_runs(
                                &mut schema,
                                &layout.shared_layout,
                            )?,
                        )
                    } else {
                        (Vec::new(), Vec::new())
                    };
                    let template_count = module
                        .chunks
                        .iter()
                        .map(|c| c.struct_templates.len())
                        .try_fold(0usize, |a, b| a.checked_add(b))
                        .ok_or_else(|| error("template count overflow"))?;
                    schema.args(&[
                        size(module.chunks.len())?,
                        size(module.enum_layouts.len())?,
                        size(module.native_names.len())?,
                        size(runs.len())?,
                        size(template_count)?,
                        i64::from(include_initializers),
                    ])?;
                    schema.accept(85)?;
                    let mut names = Names::default();
                    let mut chunk_names = Vec::new();
                    chunk_names
                        .try_reserve_exact(module.chunks.len())
                        .map_err(|_| error("chunk name allocation failed"))?;
                    for (i, c) in module.chunks.iter().enumerate() {
                        chunk_names.push(names.prefix(
                            &mut schema,
                            &mut codec,
                            &c.name,
                            (0, i, 0),
                            0,
                        )?);
                    }
                    let signature = super::wire_signatures::signature_and_native_sections(
                        &module.signatures,
                        &module.native_return_shapes,
                    )?;
                    let mut variants = Vec::new();
                    let mut layouts = Vec::new();
                    for (i, e) in module.enum_layouts.iter().enumerate() {
                        let type_name = names.prefix(
                            &mut schema,
                            &mut codec,
                            &e.type_name,
                            (1, i, 0),
                            e.variants.len(),
                        )?;
                        for (j, v) in e.variants.iter().enumerate() {
                            let name =
                                names.prefix(&mut schema, &mut codec, &v.name, (2, i, j), 0)?;
                            codec.record(
                                kind::ENUM_VARIANTS,
                                &[name.0, 0, v.disc],
                                &mut variants,
                            )?;
                        }
                        schema.fields(&[
                            type_name.0,
                            size(e.variants.len())?,
                            e.min_payload.into(),
                        ])?;
                        schema.accept(80)?;
                        codec.record(kind::ENUM_LAYOUTS, &schema.output::<4>()?, &mut layouts)?;
                    }
                    let mut natives = Vec::new();
                    for (i, n) in module.native_names.iter().enumerate() {
                        let name = names.prefix(&mut schema, &mut codec, n, (3, i, 0), 0)?;
                        codec.record(kind::NATIVES, &[name.0, 0], &mut natives)?;
                    }
                    let mut regions = vec![
                        (kind::SIGNATURES, signature.0.signatures),
                        (kind::ENUM_VARIANTS, variants),
                        (kind::ENUM_LAYOUTS, layouts),
                        (kind::NATIVES, natives),
                        (kind::NATIVE_RETURNS, signature.1),
                    ];
                    if let Some(layout) = &module.data_layout {
                        let mut slots = Vec::new();
                        for (i, run) in runs.iter().enumerate() {
                            let name = names.prefix(
                                &mut schema,
                                &mut codec,
                                &layout
                                    .slots
                                    .get(run.first)
                                    .ok_or_else(|| error("stage selected invalid slot name"))?
                                    .name,
                                (4, i, 0),
                                0,
                            )?;
                            for &take in &run.pieces {
                                codec.record(
                                    kind::DATA_SLOTS,
                                    &[name.0, run.visibility, 0, take],
                                    &mut slots,
                                )?;
                            }
                        }
                        let mut shared = Vec::new();
                        for row in &shared_rows {
                            codec.record(kind::SHARED_LAYOUT, row, &mut shared)?;
                        }
                        let mut placements = Vec::new();
                        for p in &layout.private_composite_layout {
                            codec.record(
                                kind::PRIVATE_COMPOSITE,
                                &[p.slot.into(), 0, p.offset.into()],
                                &mut placements,
                            )?;
                        }
                        regions.extend([
                            (kind::DATA_SLOTS, slots),
                            (kind::SHARED_LAYOUT, shared),
                            (kind::PRIVATE_COMPOSITE, placements),
                            (
                                kind::DATA_INIT,
                                init.as_ref()
                                    .ok_or_else(|| error("initializer plan absent"))?
                                    .record
                                    .clone(),
                            ),
                        ]);
                    }
                    schema.accept(86)?;
                    schema.fields(&[
                        i64::from(module.entry_point.is_some()),
                        size(module.entry_point.unwrap_or(0))?,
                        module.word_bits_log2.into(),
                        module.addr_bits_log2.into(),
                        module.float_bits_log2.into(),
                        module.flags.into(),
                        module.wcet_cycles.into(),
                        module.wcmu_bytes.into(),
                        module.shared_data_bytes.into(),
                        module.private_data_bytes.into(),
                        module.schema_hash.into(),
                        crate::bytecode::FORMAT_FINGERPRINT.into(),
                    ])?;
                    schema.accept(83)?;
                    let mut header = Vec::new();
                    codec.record(kind::HEADER, &schema.output::<11>()?, &mut header)?;
                    regions.push((kind::HEADER, header));
                    if !module.chunks.is_empty() || include_initializers {
                        let f = forest(
                            &mut schema,
                            &mut codec,
                            &mut names,
                            module,
                            include_initializers,
                        )?;
                        regions.extend([
                            (kind::CONSTS, f.constants),
                            (kind::STRUCT_AUX, f.structs),
                            (kind::ENUM_AUX, f.enums),
                        ]);
                    }
                    let mut templates = Vec::new();
                    for t in module.chunks.iter().flat_map(|c| &c.struct_templates) {
                        names.intern(&mut schema, &mut codec, &t.type_name, 10)?;
                        schema.args(&[size(t.field_names.len())?])?;
                        schema.accept(81)?;
                        for f in &t.field_names {
                            names.intern(&mut schema, &mut codec, f, 11)?;
                        }
                        schema.accept(82)?;
                        codec.record(
                            kind::STRUCT_TEMPLATES,
                            &schema.output::<4>()?,
                            &mut templates,
                        )?;
                    }
                    schema.accept(88)?;
                    let mut params = Vec::new();
                    for c in &module.chunks {
                        for batch in c.param_types.chunks(16) {
                            let fields: Vec<_> = batch.iter().map(|&t| parameter_code(t)).collect();
                            schema.fields(&fields)?;
                            schema.args(&[size(batch.len())?])?;
                            let count = schema.command(84)?;
                            if count != batch.len() {
                                return Err(error("parameter count disagrees"));
                            }
                            schema.read(count, &mut params)?;
                        }
                    }
                    if !module.chunks.is_empty() {
                        regions.push((kind::STRUCT_TEMPLATES, templates));
                        regions.push((kind::PARAM_TYPES, params));
                    }
                    regions.push((kind::SHAPES, signature.0.shapes));
                    let mut chunks = Vec::new();
                    let mut debug = Vec::new();
                    let mut has_debug = false;
                    for (c, name) in module.chunks.iter().zip(chunk_names) {
                        let bytes = c
                            .debug_pool
                            .as_ref()
                            .map(wire_debug_pool_via_kel)
                            .transpose()?;
                        let block = match c.block_type {
                            crate::bytecode::BlockType::Func => 0,
                            crate::bytecode::BlockType::Reentrant => 1,
                            crate::bytecode::BlockType::Stream => 2,
                        };
                        schema.fields(&[
                            name.0,
                            size(c.constants.len())?,
                            size(c.struct_templates.len())?,
                            size(c.param_types.len())?,
                            i64::from(bytes.is_some()),
                            size(bytes.as_ref().map_or(0, Vec::len))?,
                            size(c.ops.len())?,
                            c.local_count.into(),
                            c.param_count.into(),
                            block,
                        ])?;
                        schema.accept(79)?;
                        codec.record(kind::CHUNKS, &schema.output::<14>()?, &mut chunks)?;
                        if let Some(bytes) = bytes {
                            has_debug = true;
                            append(&mut debug, &bytes)?;
                        }
                    }
                    if !module.chunks.is_empty() {
                        regions.push((kind::CHUNKS, chunks));
                    }
                    if has_debug {
                        regions.push((kind::DEBUG_POOL, debug));
                    }
                    regions.push((kind::STRING_POOL, names.pool));
                    regions.push((kind::NAMES, names.records));
                    let borrowed: Vec<_> = regions
                        .iter()
                        .map(|(kind, bytes)| (*kind, bytes.as_slice()))
                        .collect();
                    wire_container_from_regions_via_kel(&borrowed)
                },
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_run_protocol_refuses_partial_replies_and_preserves_earliest_boundary() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                let mut s = Machine {
                    vm,
                    shared,
                    arg: ARG,
                    fin: FIN,
                    bytes: BYTES,
                    window: 1024,
                };
                s.args(&[3])?;
                assert_eq!(s.command(89)?, 1);
                assert!(s.command(64).is_err());
                assert!(s.command(94).is_err());
                s.args(&[1, 0])?;
                assert_eq!(s.command(90)?, 2);
                assert_eq!(s.output::<2>()?, [1, 2]);
                s.fields(&[1, 0, 1, 2])?;
                s.args(&[2])?;
                assert!(s.command(91).is_err());
                s.fields(&[1, 0, 1, 0])?;
                assert_eq!(s.command(91)?, 3);
                assert_eq!(s.output::<5>()?, [0, 1, 2, 0, 1]);
                s.write(0, b"a")?;
                s.write(64, b"a")?;
                s.write(128, b"b")?;
                s.args(&[2])?;
                assert!(s.command(92).is_err());
                s.args(&[1])?;
                assert_eq!(s.command(92)?, 4);
                assert_eq!(s.output::<4>()?, [0, 2, 1, 0]);
                assert_eq!(s.command(93)?, 5);
                assert_eq!(s.output::<1>()?, [2]);
                assert_eq!(s.command(93)?, 1);
                assert_eq!(s.output::<1>()?, [2]);
                s.args(&[1, 0])?;
                assert_eq!(s.command(90)?, 4);
                assert_eq!(s.output::<4>()?, [2, 1, 1, 1]);
                assert_eq!(s.command(93)?, 5);
                assert_eq!(s.output::<1>()?, [1]);
                assert_eq!(s.command(93)?, 0);
                assert_eq!(s.output::<1>()?, [2]);
                assert!(s.command(93).is_err());
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn shared_run_protocol_refuses_bad_batches_without_consuming_rows() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                let mut s = Machine {
                    vm,
                    shared,
                    arg: ARG,
                    fin: FIN,
                    bytes: BYTES,
                    window: 1024,
                };
                s.args(&[3])?;
                assert_eq!(s.command(94)?, 1);
                assert!(s.command(89).is_err());
                s.args(&[0, 3, 0])?;
                assert_eq!(s.command(95)?, 2);
                assert_eq!(s.output::<2>()?, [1, 2]);
                s.fields(&[8, 3, 0, 0x1_0000_0000, 3, 0])?;
                s.args(&[2])?;
                assert!(s.command(96).is_err());
                s.fields(&[8, 3, 0, 16, 3, 0])?;
                s.args(&[1])?;
                assert!(s.command(96).is_err());
                s.args(&[2])?;
                assert_eq!(s.command(96)?, 3);
                assert_eq!(s.output::<7>()?, [0, 3, 0, 0, 0, 3, 8]);
                assert!(s.command(96).is_err());
                assert_eq!(s.command(97)?, 0);
                assert!(s.command(97).is_err());
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn equal_length_hash_collisions_and_last_fresh_names_remain_distinct() {
        let long = "λ".repeat(40000);
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            let mut codec = Machine {
                vm,
                shared,
                arg: wire_slots::WARG,
                fin: wire_slots::FIN,
                bytes: wire_slots::BYTES,
                window: 65536,
            };
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    let mut schema = Machine {
                        vm,
                        shared,
                        arg: ARG,
                        fin: FIN,
                        bytes: BYTES,
                        window: 1024,
                    };
                    schema.accept(64)?;
                    let mut names = Names::default();
                    for (value, role, expected) in [
                        ("6a2ef2cd", 0, 0),
                        ("f41fb47a", 0, 1),
                        ("6a2ef2cd", 0, 0),
                        ("6a2ef2cd", 7, 2),
                        ("6a2ef2cd", 0, 2),
                        ("f41fb47a", 0, 1),
                        (long.as_str(), 0, 3),
                        (long.as_str(), 0, 3),
                        ("", 0, 4),
                        ("", 7, 5),
                        ("", 0, 5),
                    ] {
                        assert_eq!(
                            names.intern(&mut schema, &mut codec, value, role)?.0,
                            expected
                        );
                    }
                    assert_eq!(names.buckets.get(&0xec8732b2).unwrap(), &[0, 1, 2]);
                    assert_eq!(names.strings.len(), 6);
                    assert_eq!(names.pool.len(), 24 + long.len());
                    Ok(())
                },
            )
        })
        .unwrap();
    }
    #[test]
    fn stage_orders_name_contributors_and_requires_forest_before_templates() {
        with_compiled_wire(compile_src(&read_stage("kel/wire.kel")), |vm, shared| {
            let mut codec = Machine {
                vm,
                shared,
                arg: wire_slots::WARG,
                fin: wire_slots::FIN,
                bytes: wire_slots::BYTES,
                window: 65536,
            };
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    let mut s = Machine {
                        vm,
                        shared,
                        arg: ARG,
                        fin: FIN,
                        bytes: BYTES,
                        window: 1024,
                    };
                    let mut names = Names::default();
                    s.accept(64)?;
                    s.args(&[1, 1, 1, 1, 1, 0])?;
                    s.accept(85)?;
                    for (role, inner, value, variants) in [
                        (0, 0, "c", 0),
                        (1, 0, "E", 2),
                        (2, 0, "V", 0),
                        (2, 1, "V", 0),
                        (3, 0, "V", 0),
                        (4, 0, "d.s", 0),
                    ] {
                        assert_eq!(s.command(86)?, 1);
                        assert_eq!(s.output::<3>()?, [role, 0, inner]);
                        if role == 0 {
                            s.args(&[1, 3])?;
                            assert!(s.command(65).is_err());
                        }
                        names.intern(&mut s, &mut codec, value, role)?;
                        s.args(&[variants])?;
                        s.accept(87)?;
                    }
                    s.accept(86)?;
                    assert!(names.intern(&mut s, &mut codec, "early", 10).is_err());
                    assert!(s.command(88).is_err());
                    s.args(&[0])?;
                    s.accept(71)?;
                    assert_eq!(s.command(72)?, 0);
                    names.intern(&mut s, &mut codec, "T", 10)?;
                    assert!(names.intern(&mut s, &mut codec, "duplicate", 10).is_err());
                    assert!(s.command(88).is_err());
                    s.args(&[0])?;
                    s.accept(81)?;
                    s.accept(82)?;
                    s.accept(88)?;
                    assert!(names.intern(&mut s, &mut codec, "late", 3).is_err());
                    Ok(())
                },
            )
        })
        .unwrap();
    }

    #[test]
    fn encoder_name_protocol_requires_complete_input_and_matching_roles() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                let mut s = Machine {
                    vm,
                    shared,
                    arg: ARG,
                    fin: FIN,
                    bytes: BYTES,
                    window: 1024,
                };
                s.accept(64)?;
                assert!(s.command(66).is_err());
                s.args(&[1, 0])?;
                s.accept(65)?;
                assert!(s.command(67).is_err());
                assert!(s.command(71).is_err());
                s.write(0, b"x")?;
                s.args(&[2])?;
                assert!(s.command(66).is_err());
                s.args(&[1])?;
                s.accept(66)?;
                s.accept(67)?;
                s.args(&[1])?;
                assert!(s.command(68).is_err());
                s.args(&[0])?;
                s.accept(68)?;
                assert_eq!(s.output::<4>()?, [0, 0, 1, 1]);
                s.fields(&[8])?;
                s.args(&[1])?;
                assert!(s.command(84).is_err());
                s.args(&[1])?;
                s.accept(71)?;
                assert_eq!(s.command(72)?, 1);
                s.args(&[7, 0, 0, 0, 0])?;
                s.accept(73)?;
                assert_eq!(s.command(74)?, 1);
                s.args(&[1, 7])?;
                assert!(s.command(65).is_err());
                assert!(s.command(75).is_err());
                Ok(())
            },
        )
        .unwrap();
    }
}
