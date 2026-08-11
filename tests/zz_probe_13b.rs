//! TEMPORARY PROBE — not for commit.
//!
//! Settles the four unmeasured assumptions tabulated in
//! `docs/decisions/WIRE_FORMAT_SELFHOST_PLAN.md` for slice 13b. Each is
//! currently read out of the compiler rather than measured, and the flattener's
//! "needs hand-built constant trees" error came from exactly that substitution.
//!
//!   1. Does `const data k { t: (Text, Word) = ("hi", 1) }` yield
//!      `Tuple[StaticStr, Int]` — a string at a CHILD position?
//!   2. Is `Text` admissible in a `const data` field at all?
//!   3. Does a struct node intern its type name FIRST, so that
//!      `field_names_first` is one past it?
//!   4. Is a child-position `StaticStr` common enough to build cases from?
//!
//! Assumption 1 is load-bearing: if a string only ever appears at a root, then
//! depth-first and breadth-first interning coincide and the slice's central
//! property is unobservable — the same vacuity four of five flattener cases had.

#![cfg(all(feature = "compile", feature = "verify"))]

use keleusma::bytecode::ConstValue;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;

fn corpus_aux_of(module: &keleusma::bytecode::Module) -> keleusma::wire_format::WireAuxBody {
    use keleusma::wire_format::{WireAuxBody, WireChunk};
    WireAuxBody {
        chunks: module
            .chunks
            .iter()
            .map(|c| WireChunk {
                name: c.name.clone(),
                constants: c.constants.clone(),
                struct_templates: c.struct_templates.clone(),
                local_count: c.local_count,
                param_count: c.param_count,
                block_type: c.block_type,
                param_types: c.param_types.clone(),
                op_byte_offset: 0,
                op_record_count: 0,
                debug_pool_bytes: None,
            })
            .collect(),
        signatures: module.signatures.clone(),
        enum_layouts: module.enum_layouts.clone(),
        native_names: module.native_names.clone(),
        native_return_shapes: module.native_return_shapes.clone(),
        data_layout: module.data_layout.clone(),
        entry_point: module.entry_point,
        word_bits_log2: module.word_bits_log2,
        addr_bits_log2: module.addr_bits_log2,
        float_bits_log2: module.float_bits_log2,
        flags: 0,
        wcet_cycles: 0,
        wcmu_bytes: 0,
        shared_data_bytes: 0,
        private_data_bytes: 0,
        schema_hash: 0,
    }
}

fn shape(c: &ConstValue) -> String {
    match c {
        ConstValue::Tuple(v) => format!("Tuple[{}]", v.iter().map(shape).collect::<Vec<_>>().join(",")),
        ConstValue::Array(v) => format!("Array[{}]", v.iter().map(shape).collect::<Vec<_>>().join(",")),
        ConstValue::Struct { type_name, fields } => format!(
            "Struct {type_name}{{{}}}",
            fields.iter().map(|(n, v)| format!("{n}:{}", shape(v))).collect::<Vec<_>>().join(",")
        ),
        ConstValue::Enum { type_name, variant, fields, .. } => format!(
            "Enum {type_name}::{variant}({})",
            fields.iter().map(shape).collect::<Vec<_>>().join(",")
        ),
        ConstValue::StaticStr(s) => format!("Str({s:?})"),
        ConstValue::Int(v) => format!("Int({v})"),
        other => format!("{other:?}"),
    }
}

/// The NAMES entries as strings, in order.
fn names_of(bytes: &[u8]) -> Vec<String> {
    use keleusma::wire_schema::kind;
    let view = keleusma_wire::WireView::parse(bytes).expect("parses");
    let pool = view
        .find_region(kind::STRING_POOL)
        .and_then(|r| view.region_bytes(&r).ok())
        .unwrap_or(&[])
        .to_vec();
    let mut out = Vec::new();
    if let Some(r) = view.find_region(kind::NAMES) {
        let t = view.records(&r, 8).expect("names");
        for i in 0..t.len() {
            let n: keleusma::wire_schema::NameRef = t.get_as(i).expect("rec");
            let (o, l) = (n.offset as usize, n.length as usize);
            out.push(String::from_utf8_lossy(&pool[o..o + l]).into_owned());
        }
    }
    out
}

fn probe(label: &str, src: &str) {
    let m = match tokenize(src)
        .map_err(|e| format!("lex {e:?}"))
        .and_then(|t| parse(&t).map_err(|e| format!("parse {e:?}")))
        .and_then(|a| compile(&a).map_err(|e| format!("compile {e:?}")))
    {
        Ok(m) => m,
        Err(e) => {
            println!("{label:26} REJECTED {}", e.chars().take(110).collect::<String>());
            return;
        }
    };
    let roots: Vec<String> = m
        .chunks
        .iter()
        .flat_map(|c| c.constants.iter())
        .map(shape)
        .collect();
    println!("{label:26} roots={roots:?}");
}

#[test]
fn assumption_1_and_2_can_a_string_sit_inside_a_composite() {
    // Control: a string at ROOT position, which is already known to work.
    probe("str-at-root", "fn main() -> Word { let s = \"hi\"; 42 }");

    // THE LOAD-BEARING CASE.
    probe(
        "const-tuple-text-first",
        "const data k { t: (Text, Word) = (\"hi\", 1) }\nfn main() -> Word { k.t.1 }",
    );
    probe(
        "const-tuple-text-second",
        "const data k { t: (Word, Text) = (1, \"hi\") }\nfn main() -> Word { k.t.0 }",
    );
    probe(
        "const-bare-text",
        "const data k { s: Text = \"hi\" }\nfn main() -> Word { 42 }",
    );
    probe(
        "const-array-of-text",
        "const data k { xs: [Text; 2] = [\"a\", \"bb\"] }\nfn main() -> Word { 42 }",
    );
    probe(
        "const-struct-text-field",
        "struct P { s: Text, n: Word }\nconst data k { p: P = P { s: \"hi\", n: 1 } }\n\
         fn take(v: P) -> Word { v.n }\nfn main() -> Word { take(k.p) }",
    );
}

/// Assumption 3: a struct constant interns its TYPE NAME first, so
/// `field_names_first` is one past the type name's index.
#[test]
fn assumption_3_struct_interns_its_type_name_before_its_fields() {
    use keleusma::wire_schema::kind;
    let src = "struct Zed { alpha: Word, beta: Word }\n\
               const data k { p: Zed = Zed { alpha: 1, beta: 2 } }\n\
               fn take(v: Zed) -> Word { v.alpha }\nfn main() -> Word { take(k.p) }";
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let bytes = keleusma::wire_schema::encode_aux_body(&corpus_aux_of(&m)).expect("encode");
    let names = names_of(&bytes);
    let view = keleusma_wire::WireView::parse(&bytes).expect("parses");
    let r = view
        .find_region(kind::STRUCT_AUX)
        .expect("STRUCT_AUX region");
    let t = view.records(&r, 8).expect("table");
    println!("names = {names:?}");
    for i in 0..t.len() {
        let a: keleusma::wire_schema::StructAux = t.get_as(i).expect("rec");
        let tn = a.type_name as usize;
        let ff = a.field_names_first as usize;
        println!(
            "  struct_aux {i}: type_name={tn} ({:?})  field_names_first={ff} ({:?}, {:?})",
            names.get(tn),
            names.get(ff),
            names.get(ff + 1)
        );
        println!(
            "  -> type name interned BEFORE fields? {}",
            if ff == tn + 1 { "YES (ff == tn + 1)" } else { "NO" }
        );
    }
}
