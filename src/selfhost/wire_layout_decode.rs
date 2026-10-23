//! Template and enum layout semantics, including optional-family rejection.

use super::wire_directory::decode_directory;
use super::wire_framing::{get_word, set_word, with_compiled_wire};
use super::wire_records::decode_records;
use super::wire_semantics::{
    ARG, FIN, NameReadError, NameTables, byte_count, command, fields, name_lookup_decision,
};
use super::*;
use crate::bytecode::{EnumLayout, EnumVariantDisc, StructTemplate};
use crate::wire_schema::kind;

/// Independently decoded optional layout families.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireDecodedLayouts {
    /// None when template decoding rejects malformed data.
    pub templates: Option<Vec<StructTemplate>>,
    /// None when enum-layout decoding rejects malformed data.
    pub enum_layouts: Option<Vec<EnumLayout>>,
}

#[derive(Default)]
struct Family {
    templates: Vec<StructTemplate>,
    enums: Vec<EnumLayout>,
}
fn error(detail: &str) -> SelfHostError {
    SelfHostError::Unsupported {
        detail: format!("wire layouts: {detail}"),
    }
}
fn copy_name(name: &str) -> Result<String, SelfHostError> {
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| error("name allocation failed"))?;
    owned.push_str(name);
    Ok(owned)
}
fn row_fields(record: &WireSchemaRecord) -> Result<[i64; 4], SelfHostError> {
    Ok(match record {
        WireSchemaRecord::StructTemplateRecord(r) => [
            i64::from(r.type_name),
            i64::from(r.field_names_first),
            i64::from(r.field_count),
            i64::from(r.reserved),
        ],
        WireSchemaRecord::EnumLayoutRecord(r) => [
            i64::from(r.type_name),
            i64::from(r.variants_first),
            i64::from(r.variants_count),
            i64::from(r.min_payload),
        ],
        _ => return Err(error("unexpected layout record")),
    })
}
fn accepted(vm: &mut Vm<'_, '_>, shared: &mut [u8], cmd: i64) -> Result<bool, SelfHostError> {
    match command(vm, shared, cmd)? {
        0 => Ok(true),
        8 => Ok(false),
        _ => Err(error("unexpected layout response")),
    }
}
fn signed(vm: &Vm<'_, '_>, shared: &[u8], slot: usize) -> Result<i64, SelfHostError> {
    match vm
        .get_shared(shared, slot)
        .map_err(|_| error("discriminant transport failed"))?
    {
        Value::Int(value) => Ok(value),
        _ => Err(error("non-integer discriminant")),
    }
}

fn name_decision<'a>(
    vm: &mut Vm<'_, '_>,
    shared: &mut [u8],
    value: Result<&'a str, NameReadError>,
) -> Result<Option<&'a str>, SelfHostError> {
    match value {
        Ok(name) => Ok(Some(name)),
        Err(NameReadError::Data(_)) => {
            set_word(vm, shared, ARG, 0)?;
            if accepted(vm, shared, 28)? {
                return Err(error("name rejection was accepted"));
            }
            Ok(None)
        }
        Err(NameReadError::Failure(error)) => Err(error),
    }
}

fn decode_family(bytes: &[u8], enums: bool, strict: bool) -> Result<Option<Family>, SelfHostError> {
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
            let names = NameTables {
                names: region(kind::NAMES),
                pool: region(kind::STRING_POOL),
            };
            let templates = region(kind::STRUCT_TEMPLATES);
            let variants = region(kind::ENUM_VARIANTS);
            let layouts = region(kind::ENUM_LAYOUTS);
            with_compiled_wire(
                compile_src(&read_stage("kel/wire_schema.kel")),
                |vm, shared| {
                    for (i, value) in [
                        byte_count(names.names)?,
                        byte_count(names.pool)?,
                        byte_count(templates)?,
                        byte_count(variants)?,
                        byte_count(layouts)?,
                        i64::from(enums),
                        i64::from(strict),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        set_word(vm, shared, ARG + i, value)?;
                    }
                    if !accepted(vm, shared, 22)? {
                        return Ok(None);
                    }
                    let count = get_word(vm, shared, FIN + usize::from(enums))?;
                    let templates = match templates {
                        Some(bytes) => {
                            decode_records(codec, codec_shared, kind::STRUCT_TEMPLATES, bytes)?
                        }
                        None => Vec::new(),
                    };
                    let layouts = match layouts {
                        Some(bytes) => {
                            decode_records(codec, codec_shared, kind::ENUM_LAYOUTS, bytes)?
                        }
                        None => Vec::new(),
                    };
                    for record in &templates {
                        fields(vm, shared, &row_fields(record)?)?;
                        if !accepted(vm, shared, 23)? {
                            return Ok(None);
                        }
                    }
                    for record in &layouts {
                        fields(vm, shared, &row_fields(record)?)?;
                        if !accepted(vm, shared, 24)? {
                            return Ok(None);
                        }
                    }
                    if !accepted(vm, shared, 25)? {
                        return Err(error("validation finish discarded a completed view"));
                    }
                    let variants = if enums {
                        match variants {
                            Some(bytes) => {
                                decode_records(codec, codec_shared, kind::ENUM_VARIANTS, bytes)?
                            }
                            None => Vec::new(),
                        }
                    } else {
                        Vec::new()
                    };
                    let selected = if enums { &layouts } else { &templates };
                    if selected.len() != count {
                        return Err(error("layout count disagreement"));
                    }
                    let mut result = Family::default();
                    if enums {
                        result
                            .enums
                            .try_reserve_exact(count)
                            .map_err(|_| error("enum allocation failed"))?;
                    } else {
                        result
                            .templates
                            .try_reserve_exact(count)
                            .map_err(|_| error("template allocation failed"))?;
                    }
                    for _ in 0..count {
                        if command(vm, shared, 27)? != 1 {
                            return Err(error("missing layout record request"));
                        }
                        let index = get_word(vm, shared, FIN)?;
                        fields(
                            vm,
                            shared,
                            &row_fields(
                                selected
                                    .get(index)
                                    .ok_or_else(|| error("layout request out of bounds"))?,
                            )?,
                        )?;
                        if !accepted(vm, shared, 26)? {
                            return Ok(None);
                        }
                        let members = get_word(vm, shared, FIN)?;
                        let limit = members
                            .checked_mul(2)
                            .and_then(|n| n.checked_add(3))
                            .ok_or_else(|| error("layout request budget overflow"))?;
                        let mut type_name = None;
                        let mut field_names = Vec::new();
                        let mut enum_variants = Vec::new();
                        if enums {
                            enum_variants
                                .try_reserve_exact(members)
                                .map_err(|_| error("variant allocation failed"))?;
                        } else {
                            field_names
                                .try_reserve_exact(members)
                                .map_err(|_| error("field allocation failed"))?;
                        }
                        let mut finished = false;
                        for _ in 0..limit {
                            match command(vm, shared, 27)? {
                                2 => {
                                    let index = u32::try_from(get_word(vm, shared, FIN)?)
                                        .map_err(|_| error("name index exceeds u32"))?;
                                    let role = get_word(vm, shared, FIN + 1)?;
                                    let disc = if role == 2 {
                                        signed(vm, shared, FIN + 2)?
                                    } else {
                                        0
                                    };
                                    let decision = name_lookup_decision(
                                        codec,
                                        codec_shared,
                                        vm,
                                        shared,
                                        names,
                                        index,
                                    );
                                    let Some(name) = name_decision(vm, shared, decision)? else {
                                        return Ok(None);
                                    };
                                    let name = copy_name(name)?;
                                    match role {
                                        0 if type_name.is_none() => type_name = Some(name),
                                        1 if !enums && field_names.len() < members => {
                                            field_names.push(name)
                                        }
                                        2 if enums && enum_variants.len() < members => {
                                            enum_variants.push(EnumVariantDisc { name, disc })
                                        }
                                        _ => return Err(error("invalid layout name role")),
                                    }
                                    set_word(vm, shared, ARG, 1)?;
                                    if !accepted(vm, shared, 28)? {
                                        return Err(error("valid name was discarded"));
                                    }
                                }
                                3 if enums => {
                                    let index = get_word(vm, shared, FIN)?;
                                    let Some(WireSchemaRecord::EnumVariantRecord(r)) =
                                        variants.get(index)
                                    else {
                                        return Err(error("variant request out of bounds"));
                                    };
                                    fields(vm, shared, &[i64::from(r.name), r.disc])?;
                                    if !accepted(vm, shared, 29)? {
                                        return Ok(None);
                                    }
                                }
                                4 => {
                                    let name = type_name
                                        .take()
                                        .ok_or_else(|| error("missing layout type name"))?;
                                    if enums {
                                        if enum_variants.len() != members {
                                            return Err(error("incomplete variants"));
                                        }
                                        let min_payload = u32::try_from(get_word(vm, shared, FIN)?)
                                            .map_err(|_| error("payload exceeds u32"))?;
                                        result.enums.push(EnumLayout {
                                            type_name: name,
                                            variants: enum_variants,
                                            min_payload,
                                        });
                                    } else {
                                        if field_names.len() != members {
                                            return Err(error("incomplete fields"));
                                        }
                                        result.templates.push(StructTemplate {
                                            type_name: name,
                                            field_names,
                                        });
                                    }
                                    if !accepted(vm, shared, 28)? {
                                        return Err(error("completed record was discarded"));
                                    }
                                    finished = true;
                                    break;
                                }
                                8 => return Ok(None),
                                _ => return Err(error("unknown layout request")),
                            }
                        }
                        if !finished {
                            return Err(error("layout request budget exhausted"));
                        }
                    }
                    if command(vm, shared, 27)? != 0 {
                        return Err(error("layout stream did not finish"));
                    }
                    Ok(Some(result))
                },
            )
        },
    )
}

/// Decode struct templates with the reference standalone reader's strict policy.
///
/// # Errors
/// Propagates malformed layout tables, invalid requested names and stage failures.
pub fn wire_decode_struct_templates_via_kel(
    bytes: &[u8],
) -> Result<Vec<StructTemplate>, SelfHostError> {
    Ok(decode_family(bytes, false, true)?
        .ok_or_else(|| error("required template family absent"))?
        .templates)
}
/// Decode enum layouts with the reference standalone reader's strict policy.
///
/// # Errors
/// Propagates malformed layout tables, invalid requested names and stage failures.
pub fn wire_decode_enum_layouts_via_kel(bytes: &[u8]) -> Result<Vec<EnumLayout>, SelfHostError> {
    Ok(decode_family(bytes, true, true)?
        .ok_or_else(|| error("required enum family absent"))?
        .enums)
}
/// Decode both optional layout families without hiding transport or execution failures.
///
/// Each family can be absent because of malformed data independently, matching
/// the auxiliary decoder's two defaulting operations. Other families are not checked.
///
/// # Errors
/// Propagates container, allocation, transport and stage execution failures.
pub fn wire_decode_layout_metadata_via_kel(
    bytes: &[u8],
) -> Result<WireDecodedLayouts, SelfHostError> {
    let templates = decode_family(bytes, false, false)?.map(|f| f.templates);
    let enum_layouts = decode_family(bytes, true, false)?.map(|f| f.enums);
    Ok(WireDecodedLayouts {
        templates,
        enum_layouts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn optional_layout_rejection_does_not_hide_protocol_failure() {
        with_compiled_wire(
            compile_src(&read_stage("kel/wire_schema.kel")),
            |vm, shared| {
                for (i, value) in [8, 1, 16, -1, -1, 0, 0].into_iter().enumerate() {
                    set_word(vm, shared, ARG + i, value)?;
                }
                assert!(accepted(vm, shared, 22)?);
                fields(vm, shared, &[0, 0, 0, 0])?;
                assert!(accepted(vm, shared, 23)?);
                assert!(accepted(vm, shared, 25)?);
                assert_eq!(command(vm, shared, 27)?, 1);
                fields(vm, shared, &[0, 0, 0, 0])?;
                assert!(accepted(vm, shared, 26)?);
                assert_eq!(command(vm, shared, 27)?, 2);
                let failure = command(vm, shared, 27).unwrap_err();
                assert!(name_decision(vm, shared, Err(NameReadError::Failure(failure))).is_err());
                assert!(name_decision(vm, shared, Err(NameReadError::Data(-298)))?.is_none());
                assert!(command(vm, shared, 27).is_err());
                Ok(())
            },
        )
        .unwrap();
    }
}
