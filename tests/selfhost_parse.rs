//! Merged parser stage (`compiler/kel/parse.kel`): one streaming `loop` that parses a
//! whole top-level declaration — a function (its header and full body), a `data` block, an
//! `enum`, a `use` native import, or a `require` machine directive — in a single pass. This
//! covers every declaration form and body construct the compiler stages use. Data fields
//! and enums resolve by strategy-B accumulation; calls against a host-supplied chunk table.
//!
//! A throwaway adapter maps the reference tokenizer into the stage's unified `(kind,
//! value)` token stream. The stage emits header records `dkind + val*64` (1/2/3 START of a
//! fn/yield/loop, 4 PARAM, 5 END, 6 PTYPE, 7 RETTYPE, 8 ASIZE, 15 DONE, 16 BSTART) and,
//! bracketed by a BSTART and the body's terminal Done, body node records `kind + arg*64`
//! in POSTORDER (the `enum Node` codes: 1 Literal, 2 Local, 3 BinOp, 6 Not, 8 Andalso,
//! 9 Orelse, 10 Neg, 15 Done). The host decode keeps an `in_body` flag mirroring the
//! stage's. Each function is checked against the reference parse for its category, name,
//! parameter names, and the postorder body record sequence; two postorder traversals are
//! equal exactly when the node forests are, so the flat sequence is a sound equivalence.
//! Types are covered by the separate parser-stage test. Later increments add statements,
//! the control-flow forms, and the data/enum/use declaration kinds.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
// Throwaway test-driver adapters: the streaming decode state machine uses single-arm matches
// per parser sub-state, documentary `temp * 1024 + offset` slot encodings in expected-record
// literals, and record helpers that thread many positional arguments. Refactoring these would
// only obscure the adapter intent, so allow the corresponding style lints for this test file.
#![allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::single_match,
    clippy::manual_range_patterns,
    clippy::identity_op
)]

use keleusma::Arena;
use keleusma::ast::{
    BinOp, Block, Expr, FunctionCategory, Iterable, Literal, Pattern, PrimType, Stmt, TypeDef,
    TypeExpr, UnaryOp,
};
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `toks` block in parse.kel: the token stream
// is one packed `tok+payload*64` word per token (not two `kinds`/`vals` arrays).
const LEN: usize = 0;
const PACKED: usize = 1;
const LIMIT_ID: usize = 1 + 12288;
const CHUNK_COUNT: usize = 1 + 12288 + 1;
const CHUNKS: usize = 1 + 12288 + 2;
const REQUIRE_ID: usize = 1 + 12288 + 2 + 256;
const WORD_ID: usize = 1 + 12288 + 2 + 256 + 1;

/// Map the reference token stream into the stage's unified `(kind, value)` pairs. The
/// operator codes follow the retired body.kel scheme (`Plus` 21 upward); the header
/// keywords and punctuation keep the retired parser.kel codes, which agree with the body vocabulary on every shared
/// token. The trailing `Eof` is dropped.
fn adapt_tokens(src: &str, names: &mut Vec<String>) -> (Vec<i64>, Vec<i64>) {
    let mut intern = |s: &str| -> i64 {
        if let Some(i) = names.iter().position(|n| n == s) {
            i as i64
        } else {
            names.push(s.to_string());
            (names.len() - 1) as i64
        }
    };
    let tokens = tokenize(src).expect("lex");
    let mut kinds = Vec::new();
    let mut vals = Vec::new();
    for tok in &tokens {
        let (kind, val) = match &tok.kind {
            TokenKind::Fn => (0, 0),
            TokenKind::LowerIdent(s) | TokenKind::UpperIdent(s) => (1, intern(s)),
            TokenKind::LBrace => (2, 0),
            TokenKind::RBrace => (3, 0),
            TokenKind::Yield => (5, 0),
            TokenKind::Loop => (6, 0),
            TokenKind::LParen => (7, 0),
            TokenKind::RParen => (8, 0),
            TokenKind::Colon => (9, 0),
            TokenKind::Comma => (10, 0),
            TokenKind::IntLit(n) => (12, *n),
            TokenKind::Data => (13, 0),
            TokenKind::Shared => (14, 0),
            TokenKind::Private => (15, 0),
            TokenKind::Const => (16, 0),
            TokenKind::Eq => (17, 0),
            TokenKind::Use => (19, 0),
            TokenKind::LBracket => (41, 0),
            TokenKind::RBracket => (42, 0),
            TokenKind::Plus => (21, 0),
            TokenKind::Minus => (22, 0),
            TokenKind::Star => (23, 0),
            TokenKind::Slash => (24, 0),
            TokenKind::Percent => (25, 0),
            TokenKind::EqEq => (26, 0),
            TokenKind::NotEq => (27, 0),
            TokenKind::Lt => (28, 0),
            TokenKind::Gt => (29, 0),
            TokenKind::LtEq => (30, 0),
            TokenKind::GtEq => (31, 0),
            TokenKind::Not => (32, 0),
            TokenKind::Band => (33, 0),
            TokenKind::Bor => (34, 0),
            TokenKind::Bxor => (35, 0),
            TokenKind::Andalso => (36, 0),
            TokenKind::Orelse => (37, 0),
            TokenKind::Let => (38, 0),
            TokenKind::Semicolon => (39, 0),
            TokenKind::Dot => (40, 0),
            TokenKind::If => (43, 0),
            TokenKind::Else => (44, 0),
            TokenKind::For => (45, 0),
            TokenKind::In => (46, 0),
            TokenKind::DotDot => (47, 0),
            TokenKind::Match => (48, 0),
            TokenKind::FatArrow => (49, 0),
            TokenKind::Underscore => (50, 0),
            TokenKind::ColonColon => (51, 0),
            TokenKind::As => (52, 0),
            TokenKind::Enum => (53, 0),
            TokenKind::Struct => (54, 0),
            TokenKind::Trait => (55, 0),
            TokenKind::Impl => (56, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    (kinds, vals)
}

/// A parsed function: its category (1 fn, 2 yield, 3 loop), interned name id, its value
/// parameter name ids in order, its body's postorder node record sequence as (kind, arg)
/// pairs, and a `when` guard's postorder node records (empty when the head is unguarded).
type Func = (i64, i64, Vec<i64>, Vec<(i64, i64)>, Vec<(i64, i64)>);
// A captured struct declaration: its name id, then per field the field-name id, the
// type-name id, and the array length (0 for a scalar field).
type StructRec = (i64, Vec<(i64, i64, i64)>);

#[derive(Debug, Default, PartialEq)]
struct Parsed {
    funcs: Vec<Func>,
    structs: Vec<StructRec>,
    // Per trait declaration, its method-signature name ids in source order.
    traits: Vec<Vec<i64>>,
    // Per impl block, its method name ids in source order.
    impls: Vec<Vec<i64>>,
}

/// Compile parse.kel on a 64MB thread; its deeply nested source overflows the default
/// 2MB test-thread stack in the host compiler's recursive-descent parse.
fn compile_parse_stage() -> keleusma::bytecode::Module {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let stage = std::fs::read_to_string("compiler/kel/parse.kel").expect("read parse.kel");
            compile(&parse(&tokenize(&stage).expect("lex parse.kel")).expect("parse parse.kel"))
                .expect("compile parse.kel")
        })
        .expect("spawn")
        .join()
        .expect("join")
}

/// The flat data-slot names in layout order (`data.field` for a scalar, `data.field[i]`
/// per array element), from compiling the program; empty when it has no data blocks.
fn data_slot_names(program: &keleusma::ast::Program) -> Vec<String> {
    if program.data_decls.is_empty() {
        return Vec::new();
    }
    compile(program)
        .expect("compile program")
        .data_layout
        .as_ref()
        .map(|dl| dl.slots.iter().map(|s| s.name.clone()).collect())
        .unwrap_or_default()
}

/// The element-0 slot (base) and length of an array `data.field`, from its per-element
/// slot names `data.field[0]`, `data.field[1]`, ...
fn array_base_len(data_slots: &[String], data: &str, field: &str) -> (i64, i64) {
    let prefix = format!("{data}.{field}[");
    let base = data_slots
        .iter()
        .position(|n| n.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no array data slot with prefix `{prefix}`"))
        as i64;
    let len = data_slots.iter().filter(|n| n.starts_with(&prefix)).count() as i64;
    (base, len)
}

/// The enum table (enum name, variant name, discriminant) from the program's enum
/// declarations, mirroring the discriminant values the compiler resolves.
fn enum_table(program: &keleusma::ast::Program) -> Vec<(String, String, i64)> {
    let mut t = Vec::new();
    for ty in &program.types {
        if let keleusma::ast::TypeDef::Enum(ed) = ty {
            for variant in &ed.variants {
                t.push((
                    ed.name.clone(),
                    variant.name.clone(),
                    variant.discriminant_value,
                ));
            }
        }
    }
    t
}

fn run_parse(src: &str, names: &mut Vec<String>) -> Parsed {
    let (kinds, vals) = adapt_tokens(src, names);
    let module = compile_parse_stage();
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parse.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, LEN, Value::Int(kinds.len() as i64))
        .expect("len");
    // `limit` is a lowercase identifier, not a keyword, so the stage recognizes the
    // `for .. limit` clause by comparing an identifier to the interned id of "limit". It is
    // interned when the clause's `limit` token appears; -1 when there is no `for` loop.
    let limit_id = names
        .iter()
        .position(|n| n == "limit")
        .map(|i| i as i64)
        .unwrap_or(-1);
    vm.set_shared(&mut shared, LIMIT_ID, Value::Int(limit_id))
        .expect("limit_id");
    // `require` is a lowercase identifier, not a keyword; the stage recognizes the machine
    // directive by comparing an identifier to the interned id of "require".
    let require_id = names
        .iter()
        .position(|n| n == "require")
        .map(|i| i as i64)
        .unwrap_or(-1);
    vm.set_shared(&mut shared, REQUIRE_ID, Value::Int(require_id))
        .expect("require_id");
    // The interned id of `Word`, so the stage sizes a struct's Word fields at eight bytes
    // (any other scalar field is one byte) when it sums a struct's flat byte size; -1 when
    // the program has no `Word` token.
    let word_id = names
        .iter()
        .position(|n| n == "Word")
        .map(|i| i as i64)
        .unwrap_or(-1);
    vm.set_shared(&mut shared, WORD_ID, Value::Int(word_id))
        .expect("word_id");
    // The chunk-name table: the function names in declaration order, interned to the same
    // ids the token stream uses. A call resolves its callee against this host-supplied
    // table (resolved-reference data, per the merge plan; forward calls cannot resolve in
    // a single pass, so it stays host-supplied).
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let chunk_ids: Vec<i64> = program
        .functions
        .iter()
        .map(|f| {
            names
                .iter()
                .position(|n| n == &f.name)
                .unwrap_or_else(|| panic!("function name {} not interned", f.name))
                as i64
        })
        .collect();
    vm.set_shared(&mut shared, CHUNK_COUNT, Value::Int(chunk_ids.len() as i64))
        .expect("chunk_count");
    for (i, &id) in chunk_ids.iter().enumerate() {
        vm.set_shared(&mut shared, CHUNKS + i, Value::Int(id))
            .expect("chunk");
    }
    for (i, (&k, &v)) in kinds.iter().zip(vals.iter()).enumerate() {
        vm.set_shared(&mut shared, PACKED + i, Value::Int(k + v * 64))
            .expect("token");
    }

    let mut parsed = Parsed::default();
    // The declaration under construction: (category, name, params, body records, guard
    // records).
    let mut cur: Option<(i64, i64, Vec<i64>, Vec<(i64, i64)>, Vec<(i64, i64)>)> = None;
    let mut in_body = false;
    let mut in_guard = false;
    // A data block is skipped this increment: DSTART opens it and its END closes it; its
    // fields are not compared, only that the block is consumed without breaking the stream.
    let mut in_data = false;
    let mut in_enum = false;
    let mut in_use = false;
    // A struct declaration is captured: STRUCTSTART (18) opens it with the struct name, each
    // PARAM/PTYPE/ASIZE builds a field record, and END closes it. The captured fields are
    // compared against the reference StructDef.
    let mut in_struct = false;
    let mut cur_struct: Option<StructRec> = None;
    // A trait (TRAITSTART 19) captures each method signature's name (MNAME 21) into a list,
    // validated against the reference TraitDef. An impl (IMPLSTART 20) is skipped whole (its
    // methods are a later increment). Both emit END at the brace-matched close.
    let mut in_trait = false;
    let mut cur_trait: Option<Vec<i64>> = None;
    let mut in_impl = false;
    let mut cur_impl: Option<Vec<i64>> = None;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(kinds.len() * 4 + 16) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let code = w.rem_euclid(64);
                let val = w.div_euclid(64);
                if in_body {
                    match code {
                        0 => {}                // PENDING (a shunting-yard push, no record)
                        15 => in_body = false, // the body's Done ends body mode
                        _ => cur
                            .as_mut()
                            .expect("body node before START")
                            .3
                            .push((code, val)),
                    }
                } else if in_guard {
                    match code {
                        0 => {}                 // PENDING
                        15 => in_guard = false, // the guard's Done ends guard mode
                        _ => cur
                            .as_mut()
                            .expect("guard node before START")
                            .4
                            .push((code, val)),
                    }
                } else if in_data {
                    match code {
                        5 => in_data = false, // the data block's END
                        _ => {}               // its fields, skipped this increment
                    }
                } else if in_enum {
                    match code {
                        5 => in_enum = false, // the enum's END
                        _ => {}               // its variants, skipped this increment
                    }
                } else if in_struct {
                    match code {
                        // PARAM: a new field name; its type and array length follow.
                        4 => cur_struct
                            .as_mut()
                            .expect("struct field before STRUCTSTART")
                            .1
                            .push((val, 0, 0)),
                        // PTYPE: the current field's type name.
                        6 => {
                            cur_struct
                                .as_mut()
                                .expect("PTYPE before STRUCTSTART")
                                .1
                                .last_mut()
                                .expect("PTYPE before PARAM")
                                .1 = val
                        }
                        // ASIZE: the current field is an array of this length.
                        8 => {
                            cur_struct
                                .as_mut()
                                .expect("ASIZE before STRUCTSTART")
                                .1
                                .last_mut()
                                .expect("ASIZE before PARAM")
                                .2 = val
                        }
                        5 => {
                            parsed
                                .structs
                                .push(cur_struct.take().expect("struct END without STRUCTSTART"));
                            in_struct = false;
                        }
                        _ => {} // PENDING and any other intra-struct record
                    }
                } else if in_trait {
                    match code {
                        // MNAME: a method signature's name.
                        21 => cur_trait
                            .as_mut()
                            .expect("method name before TRAITSTART")
                            .push(val),
                        5 => {
                            parsed
                                .traits
                                .push(cur_trait.take().expect("trait END without TRAITSTART"));
                            in_trait = false;
                        }
                        _ => {} // signature params/return, skipped this increment
                    }
                } else if in_impl {
                    match code {
                        21 => cur_impl
                            .as_mut()
                            .expect("method name before IMPLSTART")
                            .push(val), // MNAME: an impl method's name
                        5 => {
                            parsed
                                .impls
                                .push(cur_impl.take().expect("impl END without IMPLSTART"));
                            in_impl = false;
                        }
                        _ => {} // method params/return/body, skipped this increment
                    }
                } else if in_use {
                    match code {
                        5 => in_use = false, // the use import's END
                        _ => {}              // its path segments, skipped this increment
                    }
                } else {
                    match code {
                        0 => {} // PENDING
                        1..=3 => cur = Some((code, val, Vec::new(), Vec::new(), Vec::new())),
                        4 => cur.as_mut().expect("PARAM before START").2.push(val),
                        6 | 7 | 8 => {} // PTYPE/RETTYPE/ASIZE: not checked this increment
                        9 => in_data = true, // DSTART: a data block, skipped this increment
                        10 => in_use = true, // USTART: a use import, skipped this increment
                        12 => in_enum = true, // ENUMSTART: an enum, skipped this increment
                        18 => {
                            // STRUCTSTART: open a struct capture carrying its name id.
                            in_struct = true;
                            cur_struct = Some((val, Vec::new()));
                        }
                        19 => {
                            // TRAITSTART: open a trait capture; MNAMEs fill its method list.
                            in_trait = true;
                            cur_trait = Some(Vec::new());
                        }
                        20 => {
                            // IMPLSTART: open an impl capture; MNAMEs fill its method list.
                            in_impl = true;
                            cur_impl = Some(Vec::new());
                        }
                        16 => in_body = true,  // BSTART: a body forest follows
                        17 => in_guard = true, // GSTART: a `when` guard forest follows
                        5 => {
                            let f = cur.take().expect("END before START");
                            parsed.funcs.push(f);
                        }
                        15 => {
                            assert!(cur.is_none(), "DONE mid-declaration");
                            return parsed;
                        }
                        other => panic!("unexpected declaration kind {other}"),
                    }
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected VM state {other:?}"),
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("parser did not reach DONE within the iteration budget");
}

/// Flatten an expression into its postorder node record sequence, mirroring the stage's
/// shunting-yard emission: operands before operators, a BinOp (kind 3) carrying its
/// operator code, the short-circuit booleans their own kinds (8 Andalso, 9 Orelse), and
/// the unary prefixes 6 Not / 10 Neg. Parenthesised grouping is structural in the AST, so
/// it contributes no node, exactly as the stage discards its parenthesis marker.
fn flatten(
    e: &Expr,
    scope: &[(String, i64)],
    next_slot: &mut i64,
    forlim: &mut i64,
    data_slots: &[String],
    chunk_names: &[String],
    enum_table: &[(String, String, i64)],
    out: &mut Vec<(i64, i64)>,
) {
    match e {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => out.push((1, *n)),
        Expr::Ident { name, .. } => {
            // The scope maps each bound name to its frame slot; the most recent binding of
            // a name wins (a `let` shadows a parameter).
            let slot = scope
                .iter()
                .rev()
                .find(|(n, _)| n == name)
                .map(|(_, s)| *s)
                .unwrap_or_else(|| panic!("identifier {name} is not in scope"));
            out.push((2, slot));
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            flatten(
                left,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            flatten(
                right,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            let (kind, code) = match op {
                BinOp::Add => (3, 1),
                BinOp::Mul => (3, 2),
                BinOp::Sub => (3, 3),
                BinOp::Div => (3, 4),
                BinOp::Mod => (3, 5),
                BinOp::Eq => (3, 6),
                BinOp::NotEq => (3, 7),
                BinOp::Lt => (3, 8),
                BinOp::Gt => (3, 9),
                BinOp::LtEq => (3, 10),
                BinOp::GtEq => (3, 11),
                BinOp::Band => (3, 12),
                BinOp::Bor => (3, 13),
                BinOp::Bxor => (3, 14),
                BinOp::Andalso => (8, 0),
                BinOp::Orelse => (9, 0),
                other => panic!("increment does not handle operator {other:?}"),
            };
            out.push((kind, code));
        }
        Expr::UnaryOp { op, operand, .. } => {
            flatten(
                operand,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            let kind = match op {
                UnaryOp::Not => 6,
                UnaryOp::Neg => 10,
                other => panic!("increment handles only `-` and `not`, got {other:?}"),
            };
            out.push((kind, 0));
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            // Postorder: the condition, the then branch (a folded block), the else branch
            // (a folded block, or a synthesized Unit when absent), then the If node
            // (kind 4). Slots are monotonic across branches, so `next_slot` is threaded.
            flatten(
                condition,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            flatten_block(
                then_block,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            match else_block {
                Some(eb) => flatten_block(
                    eb,
                    scope,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                ),
                None => out.push((20, 0)), // synthesized empty else: a Unit
            }
            out.push((4, 0));
        }
        Expr::Yield { value, .. } => {
            // `yield e` is a unary YieldExpr (kind 24) over its operand.
            flatten(
                value,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            out.push((24, 0));
        }
        Expr::FieldAccess { object, field, .. } => {
            // A scalar `data.field` read is a DataRead (kind 11) over the field's element-0
            // slot in the flat data layout. The stage resolves it from its own accumulated
            // table; the reference resolves it from the compiler's `data_layout.slots`.
            let data = match object.as_ref() {
                Expr::Ident { name, .. } => name.as_str(),
                other => panic!("a data read's object must be a block name, got {other:?}"),
            };
            let name = format!("{data}.{field}");
            let slot = data_slots
                .iter()
                .position(|n| n == &name)
                .unwrap_or_else(|| panic!("no data slot named `{name}`"))
                as i64;
            out.push((11, slot));
        }
        Expr::ArrayIndex { object, index, .. } => {
            // A `data.field[i]` read is an IndexRead (kind 13, arg = base + len*2^24) over
            // the index expression. The object is a `data.field` access.
            let (data, field) = match object.as_ref() {
                Expr::FieldAccess {
                    object: inner,
                    field,
                    ..
                } => match inner.as_ref() {
                    Expr::Ident { name, .. } => (name.as_str(), field.as_str()),
                    other => panic!("a data index object must be a block name, got {other:?}"),
                },
                other => panic!("a data index object must be a field access, got {other:?}"),
            };
            let (base, len) = array_base_len(data_slots, data, field);
            flatten(
                index,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            out.push((13, base + len * 16_777_216));
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            // Postorder: the scrutinee, then per integer-literal arm a Literal node and its
            // result, then the trailing wildcard's result, then a MatchBuild (kind 34)
            // packing the scrutinee temp slot and the literal-arm count as
            // `temp * 1024 + count`. The temp slot is reserved right after the scrutinee.
            flatten(
                scrutinee,
                scope,
                next_slot,
                forlim,
                data_slots,
                chunk_names,
                enum_table,
                out,
            );
            let temp = *next_slot;
            *next_slot += 1;
            let mut lit_count = 0i64;
            for arm in arms {
                assert!(
                    arm.guard.is_none(),
                    "increment 5 handles unguarded arms only"
                );
                match &arm.pattern {
                    Pattern::Literal(Literal::Int(v), _) => {
                        out.push((1, *v));
                        flatten(
                            &arm.expr,
                            scope,
                            next_slot,
                            forlim,
                            data_slots,
                            chunk_names,
                            enum_table,
                            out,
                        );
                        lit_count += 1;
                    }
                    Pattern::Enum(enum_name, variant, sub_pats, _) if sub_pats.is_empty() => {
                        // A unit enum-variant pattern folds to the variant's discriminant
                        // literal, the same arm an integer literal forms.
                        let disc = enum_table
                            .iter()
                            .find(|(e, v, _)| e == enum_name && v == variant)
                            .map(|(_, _, d)| *d)
                            .unwrap_or_else(|| {
                                panic!("no discriminant for {enum_name}::{variant}")
                            });
                        out.push((1, disc));
                        flatten(
                            &arm.expr,
                            scope,
                            next_slot,
                            forlim,
                            data_slots,
                            chunk_names,
                            enum_table,
                            out,
                        );
                        lit_count += 1;
                    }
                    Pattern::Wildcard(_) => {
                        flatten(
                            &arm.expr,
                            scope,
                            next_slot,
                            forlim,
                            data_slots,
                            chunk_names,
                            enum_table,
                            out,
                        );
                    }
                    other => panic!("increment 5 handles integer and wildcard arms, got {other:?}"),
                }
            }
            out.push((34, temp * 1024 + lit_count));
        }
        Expr::Call { name, args, .. } => {
            // A call flattens its arguments left to right, then a Call (kind 7) packing the
            // callee chunk index and the argument count as chunk + count*256. The chunk is
            // the callee's position in the function declaration order (the last, matching
            // the stage's scan, so a repeated name resolves identically).
            for arg in args {
                flatten(
                    arg,
                    scope,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
            }
            let chunk = chunk_names
                .iter()
                .rposition(|n| n == name)
                .unwrap_or_else(|| panic!("no chunk named `{name}`"))
                as i64;
            out.push((7, chunk + args.len() as i64 * 256));
        }
        Expr::StructInit { fields, .. } => {
            // A struct construction flattens its field values in source order (which equals
            // declaration order this increment), then a StructInit (kind 27) packing the
            // struct's flat byte size and field count as byte_size * 1024 + count. The
            // parse-level structs here are all-Word, so the byte size is count * word_bytes
            // (eight); the stage sums it per field from the declaration, so the two agree.
            for f in fields {
                flatten(
                    &f.value,
                    scope,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
            }
            let count = fields.len() as i64;
            out.push((27, count * 8 * 1024 + count));
        }
        Expr::Cast { expr: inner, .. } => match inner.as_ref() {
            // A `Enum::Variant() as Word` cast of a no-payload variant folds to the
            // variant's discriminant literal (kind 1).
            Expr::EnumVariant {
                enum_name,
                variant,
                args,
                ..
            } if args.is_empty() => {
                let disc = enum_table
                    .iter()
                    .find(|(e, v, _)| e == enum_name && v == variant)
                    .map(|(_, _, d)| *d)
                    .unwrap_or_else(|| panic!("no discriminant for {enum_name}::{variant}"));
                out.push((1, disc));
            }
            other => panic!("increment 10 handles `Enum::Variant() as Word` casts, got {other:?}"),
        },
        other => panic!("increment does not handle expression {other:?}"),
    }
}

/// Flatten a block: each statement's value in source order, then the tail (or a Unit for a
/// statement-only block), then each statement's node (LetIn for a `let`, ExprStmt for a
/// bare or block-form expression) last to first, mirroring the stage's block fold. A `let`
/// binding claims the next monotonic frame slot and joins the scope. `next_slot` is the
/// monotonic frame-slot counter, threaded so a nested branch's slots never collide with an
/// enclosing block's.
fn flatten_block(
    block: &Block,
    scope: &[(String, i64)],
    next_slot: &mut i64,
    forlim: &mut i64,
    data_slots: &[String],
    chunk_names: &[String],
    enum_table: &[(String, String, i64)],
    out: &mut Vec<(i64, i64)>,
) {
    let mut local = scope.to_vec();
    let mut stmt_nodes = Vec::new();
    for st in &block.stmts {
        match st {
            Stmt::Let(l) => {
                flatten(
                    &l.value,
                    &local,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                let name = match &l.pattern {
                    Pattern::Variable(n, _) => n.clone(),
                    other => panic!("test uses simple let patterns only, got {other:?}"),
                };
                let slot = *next_slot;
                *next_slot += 1;
                local.push((name, slot));
                stmt_nodes.push((5, slot)); // LetIn
            }
            Stmt::Expr(e) => {
                flatten(
                    e,
                    &local,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                stmt_nodes.push((21, 0)); // ExprStmt
            }
            Stmt::DataFieldAssign {
                data_name,
                field,
                value,
                ..
            } => {
                // `d.f = e` — the value, then a DataAssignIn (kind 12) carrying the field's
                // slot, folded in like a LetIn.
                flatten(
                    value,
                    &local,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                let name = format!("{data_name}.{field}");
                let slot = data_slots
                    .iter()
                    .position(|n| n == &name)
                    .unwrap_or_else(|| panic!("no data slot named `{name}`"))
                    as i64;
                stmt_nodes.push((12, slot));
            }
            Stmt::DataFieldIndexAssign {
                data_name,
                field,
                indices,
                value,
                ..
            } => {
                // `d.f[i] = e` — the index, the value, an IndexStore signal (kind 36), then
                // an IndexAssignIn (kind 14, arg = base + len*2^24) folded around the block.
                assert_eq!(indices.len(), 1, "single-dimension array assignment only");
                flatten(
                    &indices[0],
                    &local,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                flatten(
                    value,
                    &local,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                let (base, len) = array_base_len(data_slots, data_name, field);
                out.push((36, 0));
                stmt_nodes.push((14, base + len * 16_777_216));
            }
            Stmt::For(fs) if fs.limit.is_some() => {
                // A `for v in lo..hi limit CAP { body }`. The loop variable, the high slot,
                // the counter, the cap slot, and the outcome are five consecutive monotonic
                // frame slots (the variable allocated after the low bound and before the
                // high). The stage then streams the four literal nodes (cap, 0, 1, 2), the
                // five SlotRecords, and a ForBuild, and records a ForLimit statement.
                let (low, high) = match &fs.iterable {
                    Iterable::Range(s, e) => (s.as_ref(), e.as_ref()),
                    other => panic!("a `limit` clause requires a range, got {other:?}"),
                };
                let cap = match fs.limit.as_ref() {
                    Some(Expr::Literal {
                        value: Literal::Int(n),
                        ..
                    }) => *n,
                    other => panic!("the stage requires a literal `limit`, got {other:?}"),
                };
                flatten(
                    low,
                    &local,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                let vslot = *next_slot;
                *next_slot += 1;
                flatten(
                    high,
                    &local,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                let end_slot = *next_slot;
                *next_slot += 1;
                let ctr = *next_slot;
                *next_slot += 1;
                let cap_slot = *next_slot;
                *next_slot += 1;
                let oc = *next_slot;
                *next_slot += 1;
                let mut body_scope = local.clone();
                body_scope.push((fs.var.clone(), vslot));
                flatten_block(
                    &fs.body,
                    &body_scope,
                    next_slot,
                    forlim,
                    data_slots,
                    chunk_names,
                    enum_table,
                    out,
                );
                out.push((1, cap));
                out.push((1, 0));
                out.push((1, 1));
                out.push((1, 2));
                out.push((32, vslot));
                out.push((32, end_slot));
                out.push((32, ctr));
                out.push((32, cap_slot));
                out.push((32, oc));
                out.push((33, 0));
                stmt_nodes.push((23, 12 * *forlim)); // ForLimit
                *forlim += 1;
            }
            other => panic!(
                "increment 5 handles let, expression, and for-limit statements, got {other:?}"
            ),
        }
    }
    match &block.tail_expr {
        Some(tail) => flatten(
            tail,
            &local,
            next_slot,
            forlim,
            data_slots,
            chunk_names,
            enum_table,
            out,
        ),
        None => out.push((20, 0)), // statement-only block: implicit Unit
    }
    for (kind, arg) in stmt_nodes.iter().rev() {
        out.push((*kind, *arg));
    }
}

/// Build the same `Parsed` from the reference parse, interning names to match the stage's
/// ids.
fn reference(src: &str, names: &[String]) -> Parsed {
    let id = |s: &str| -> i64 {
        names
            .iter()
            .position(|n| n == s)
            .unwrap_or_else(|| panic!("name {s} not interned")) as i64
    };
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let data_slots = data_slot_names(&program);
    let chunk_names: Vec<String> = program.functions.iter().map(|f| f.name.clone()).collect();
    let enum_table = enum_table(&program);
    let mut funcs = Vec::new();
    for f in &program.functions {
        let cat = match f.category {
            FunctionCategory::Fn => 1,
            FunctionCategory::Yield => 2,
            FunctionCategory::Loop => 3,
        };
        let param_names: Vec<&str> = f
            .params
            .iter()
            .map(|p| match &p.pattern {
                Pattern::Variable(n, _) => n.as_str(),
                other => panic!("test uses simple parameter patterns only, got {other:?}"),
            })
            .collect();
        let params: Vec<i64> = param_names.iter().map(|n| id(n)).collect();
        // The frame scope: each parameter at its positional slot.
        let param_scope: Vec<(String, i64)> = param_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), i as i64))
            .collect();
        // A `when` guard is a single expression sharing the parameter frame; the stage
        // resets the frame between the guard and the body, so the guard flattens with its
        // own slot counter starting after the parameters, independent of the body's.
        let mut guard = Vec::new();
        if let Some(g) = &f.guard {
            let mut gslot = param_names.len() as i64;
            let mut gforlim = 0i64;
            flatten(
                g,
                &param_scope,
                &mut gslot,
                &mut gforlim,
                &data_slots,
                &chunk_names,
                &enum_table,
                &mut guard,
            );
        }
        let mut body = Vec::new();
        let mut next_slot = param_names.len() as i64;
        let mut forlim = 0i64;
        flatten_block(
            &f.body,
            &param_scope,
            &mut next_slot,
            &mut forlim,
            &data_slots,
            &chunk_names,
            &enum_table,
            &mut body,
        );
        funcs.push((cat, id(&f.name), params, body, guard));
    }
    // Struct declarations, in source order, each as (name, fields) where a field is
    // (field name, type name, array length 0 for a scalar) -- mirroring the stage's
    // STRUCTSTART / PARAM / PTYPE / ASIZE record capture.
    // The source spelling of a scalar type, so its interned id matches the type-name token
    // the stage emits as PTYPE. A primitive (`Word`, `Byte`, ...) is a `Prim`, not a `Named`.
    fn type_name(te: &TypeExpr) -> &str {
        match te {
            TypeExpr::Named(n, _, _, _) => n.as_str(),
            TypeExpr::Prim(pt, _) => match pt {
                PrimType::Byte => "Byte",
                PrimType::Word => "Word",
                PrimType::Float => "Float",
                PrimType::Bool => "Bool",
                PrimType::Text => "Text",
                PrimType::Fixed(_) => "Fixed",
            },
            other => panic!("struct test uses named or primitive field types only, got {other:?}"),
        }
    }
    let mut structs = Vec::new();
    for ty in &program.types {
        if let TypeDef::Struct(sd) = ty {
            let mut fields = Vec::new();
            for fd in &sd.fields {
                let (tn, size): (&str, i64) = match &fd.type_expr {
                    TypeExpr::Array(elem, len, _) => (
                        type_name(elem),
                        len.as_lit().expect("array length must be a literal"),
                    ),
                    other => (type_name(other), 0),
                };
                fields.push((id(&fd.name), id(tn), size));
            }
            structs.push((id(&sd.name), fields));
        }
    }
    // Trait declarations, in source order; each as the list of its method-signature name
    // ids -- mirroring the stage's per-trait MNAME capture.
    let traits: Vec<Vec<i64>> = program
        .traits
        .iter()
        .map(|td| td.methods.iter().map(|m| id(&m.name)).collect())
        .collect();
    // Impl blocks, in source order; each as the list of its method name ids.
    let impls: Vec<Vec<i64>> = program
        .impls
        .iter()
        .map(|ib| ib.methods.iter().map(|m| id(&m.name)).collect())
        .collect();
    Parsed {
        funcs,
        structs,
        traits,
        impls,
    }
}

// An atomic literal body (the increment-1 case) still parses: one Literal record.
#[test]
fn an_atomic_literal_body_still_parses() {
    let src = "fn answer() -> Word { 42 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(1, 42)]);
}

// A single binary operator over two parameters is postorder Local, Local, BinOp.
#[test]
fn a_binary_operator_body_parses() {
    let src = "fn add(a: Word, b: Word) -> Word { a + b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 1), (3, 1)]); // a, b, BinOp(Add)
}

// Precedence binds `*` tighter than `+`: `a + b * c` is a, b, c, Mul, Add.
#[test]
fn precedence_is_respected() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a + b * c }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 1), (2, 2), (3, 2), (3, 1)]);
}

// Parentheses override precedence: `(a + b) * c` is a, b, Add, c, Mul.
#[test]
fn parentheses_override_precedence() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { (a + b) * c }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 1), (3, 1), (2, 2), (3, 2)]);
}

// The unary prefixes and a comparison: `not a == b` and `-a`.
#[test]
fn unary_and_comparison_parse() {
    let src = "fn f(a: Word, b: Word) -> Word { -a + b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// The bitwise and short-circuit operators parse to their node kinds.
#[test]
fn bitwise_and_short_circuit_parse() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a band b orelse c }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, b, BinOp(Band), c, Orelse.
    assert_eq!(
        got.funcs[0].3,
        vec![(2, 0), (2, 1), (3, 12), (2, 2), (9, 0)]
    );
}

// Two functions with expression bodies parse in sequence, proving the handoff.
#[test]
fn two_expression_bodies_in_sequence() {
    let src = "fn f(a: Word) -> Word { a + a } fn g(b: Word) -> Word { b * b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2);
}

// A single `let` binding followed by a tail: the value, the tail, then a LetIn wrapping
// them. The binding's slot is the first after the parameters.
#[test]
fn a_let_binding_folds_into_a_letin() {
    let src = "fn f(a: Word) -> Word { let x = a + a; x }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, a, BinOp(Add) [the value], Local(x, slot 1) [tail], LetIn(slot 1).
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 0), (3, 1), (2, 1), (5, 1)]);
}

// Two `let` bindings: the second may reference the first; the fold wraps last to first.
#[test]
fn two_let_bindings_fold_last_to_first() {
    let src = "fn f(a: Word) -> Word { let x = a + a; let y = x + a; y }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // x value (a,a,Add), y value (x=slot1, a=slot0, Add), tail y=slot2,
    // then LetIn(y=slot2), LetIn(x=slot1).
    assert_eq!(
        got.funcs[0].3,
        vec![
            (2, 0),
            (2, 0),
            (3, 1),
            (2, 1),
            (2, 0),
            (3, 1),
            (2, 2),
            (5, 2),
            (5, 1)
        ]
    );
}

// A statement-only block (a `let` with no tail) has the implicit Unit value.
#[test]
fn a_statement_only_block_has_a_unit_tail() {
    let src = "fn f(a: Word) -> Word { let x = a; }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a [value], Unit [tail], LetIn(slot 1).
    assert_eq!(got.funcs[0].3, vec![(2, 0), (20, 0), (5, 1)]);
}

// An `if`/`else` as the block tail: condition, then branch, else branch, then the If node.
#[test]
fn an_if_else_tail_is_an_if_node() {
    let src = "fn f(a: Word) -> Word { if a > 0 { a } else { 0 } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, 0, BinOp(Gt) [cond]; a [then]; 0 [else]; If.
    assert_eq!(
        got.funcs[0].3,
        vec![(2, 0), (1, 0), (3, 9), (2, 0), (1, 0), (4, 0)]
    );
}

// Nested statement blocks in the branches, each with its own `let`, use monotonic slots.
#[test]
fn if_branches_are_statement_blocks() {
    let src = "fn f(a: Word) -> Word { if a > 0 { let x = a; x } else { let y = a; y } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// An `if` without `else` synthesizes a Unit else branch.
#[test]
fn an_if_without_else_synthesizes_a_unit() {
    let src = "fn f(a: Word) -> Word { if a > 0 { a } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // cond; a [then]; Unit [synth else]; If.
    assert_eq!(
        got.funcs[0].3,
        vec![(2, 0), (1, 0), (3, 9), (2, 0), (20, 0), (4, 0)]
    );
}

// A block-form `if` statement followed by a tail is committed as an ExprStmt.
#[test]
fn a_block_form_if_statement_is_an_expr_stmt() {
    let src = "fn f(a: Word) -> Word { if a > 0 { a } else { a } a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // cond, then, else, If [the statement value]; a [tail]; ExprStmt.
    assert_eq!(
        got.funcs[0].3,
        vec![
            (2, 0),
            (1, 0),
            (3, 9),
            (2, 0),
            (2, 0),
            (4, 0),
            (2, 0),
            (21, 0)
        ]
    );
}

// A `yield e` tail is a YieldExpr over its operand; the operand may be a whole expression
// since `yield` binds loosest.
#[test]
fn a_yield_expression_binds_loosest() {
    let src = "yield gen(a: Word) -> Word { yield a + a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, a, BinOp(Add), YieldExpr.
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 0), (3, 1), (24, 0)]);
    assert_eq!(got.funcs[0].0, 2); // yield category
}

// A `yield` as a `let` value composes as an ordinary operand.
#[test]
fn a_yield_may_be_a_let_value() {
    let src = "yield gen(a: Word) -> Word { let x = yield a; x }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // a, YieldExpr [value]; x=slot1 [tail]; LetIn(slot1).
    assert_eq!(got.funcs[0].3, vec![(2, 0), (24, 0), (2, 1), (5, 1)]);
}

// A `for v in lo..hi limit CAP { body }` accumulator loop over a data-free body: the loop
// is a ForLimit statement, its parts streamed after the body.
#[test]
fn a_for_limit_loop_parses() {
    let src = "fn f(n: Word) -> Word { for i in 0..n limit 8 { let x = i; } 0 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// The loop body may reference the loop variable, bound to its frame slot.
#[test]
fn the_loop_variable_is_in_scope() {
    let src = "fn f(n: Word) -> Word { for i in 0..n limit 8 { let x = i + i; } 0 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // low 0; high n(slot0); body: i(slot1)+i(slot1) Add, LetIn(slot6); then the parts.
    // Slots: var=1, end=2, ctr=3, cap=4, oc=5, then body let x=6.
    assert_eq!(
        got.funcs[0].3,
        vec![
            (1, 0),  // low 0
            (2, 0),  // high n
            (2, 1),  // i
            (2, 1),  // i
            (3, 1),  // Add
            (20, 0), // body Unit tail
            (5, 6),  // LetIn x (slot 6)
            (1, 8),  // cap
            (1, 0),  // 0
            (1, 1),  // 1
            (1, 2),  // 2
            (32, 1), // SlotRecord var
            (32, 2), // end
            (32, 3), // ctr
            (32, 4), // cap slot
            (32, 5), // oc
            (33, 0), // ForBuild
            (1, 0),  // tail 0
            (23, 0)  // ForLimit statement
        ]
    );
}

// A `match` over integer-literal arms with a trailing wildcard is a MatchBuild signal
// packing the scrutinee temp slot and the literal-arm count.
#[test]
fn a_match_expression_parses() {
    let src = "fn f(n: Word) -> Word { match n { 1 => n, 2 => n, _ => n } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // scrut n(0); Lit 1, n(0); Lit 2, n(0); wildcard n(0); MatchBuild(temp 1 * 1024 + 2).
    assert_eq!(
        got.funcs[0].3,
        vec![
            (2, 0),
            (1, 1),
            (2, 0),
            (1, 2),
            (2, 0),
            (2, 0),
            (34, 1 * 1024 + 2)
        ]
    );
}

// A `match` may be a `let` value; the temp slot follows the binding's own slots.
#[test]
fn a_match_may_be_a_let_value() {
    let src = "fn f(n: Word) -> Word { let r = match n { 0 => n, _ => n }; r }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A block-form `match` statement followed by a tail is committed as an ExprStmt.
#[test]
fn a_block_form_match_statement_is_an_expr_stmt() {
    let src = "fn f(n: Word) -> Word { match n { 1 => n, _ => n } n }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // scrut, Lit 1, n, wildcard n, MatchBuild, n [tail], ExprStmt.
    assert_eq!(
        got.funcs[0].3,
        vec![
            (2, 0),
            (1, 1),
            (2, 0),
            (2, 0),
            (34, 1 * 1024 + 1),
            (2, 0),
            (21, 0)
        ]
    );
}

// A `shared data` block before a function: the block is consumed by the header and the
// function still parses correctly.
#[test]
fn a_data_block_before_a_function_parses() {
    let src = "shared data d { a: Word, b: Word } fn f(n: Word) -> Word { n + n }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 1);
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 0), (3, 1)]);
}

// A function interleaved between a private data block with an array field and a const
// data block whose initializers are skipped.
#[test]
fn functions_interleave_with_data_blocks() {
    let src = "shared data ps { xs: [Word; 4], n: Word } \
        fn f(a: Word) -> Word { a } \
        const data k { radix: Word = 64, pack: Word = 65536 } \
        fn g(b: Word) -> Word { b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2);
}

// A scalar `data.field` read resolves to its element-0 slot, accumulated by the header
// and validated against the compiler's flat data layout.
#[test]
fn a_scalar_data_read_resolves_its_slot() {
    let src = "shared data d { a: Word, b: Word } fn f() -> Word { d.b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // d.b is the second field, slot 1.
    assert_eq!(got.funcs[0].3, vec![(11, 1)]);
}

// A data read composes with the operator grammar and preceding array-field widths: the
// base counter is a prefix-sum over field slot counts.
#[test]
fn data_reads_account_for_array_widths() {
    let src = "shared data d { xs: [Word; 3], n: Word } fn f(p: Word) -> Word { d.n + p }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // d.xs occupies slots 0..2, so d.n is slot 3; then p (slot 0), Add.
    assert_eq!(got.funcs[0].3, vec![(11, 3), (2, 0), (3, 1)]);
}

// A const data block carries no runtime slots, so it does not advance the base counter;
// a following shared field starts where the shared layout left off.
#[test]
fn const_blocks_do_not_consume_slots() {
    let src = "const data k { z: Word = 9 } shared data d { a: Word } fn f() -> Word { d.a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs[0].3, vec![(11, 0)]);
}

// A scalar `data.field = e` assignment is a DataAssignIn statement carrying the field's
// slot, folded around the tail.
#[test]
fn a_scalar_data_assignment_parses() {
    let src = "shared data d { a: Word } fn f(x: Word) -> Word { d.a = x; d.a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // value x(slot0); tail d.a (DataRead slot 0); DataAssignIn(slot 0).
    assert_eq!(got.funcs[0].3, vec![(2, 0), (11, 0), (12, 0)]);
}

// A shared and a private data block together: the base counter spans both in declaration
// order, matching the compiler's flat layout. The private block is mutated so it compiles.
#[test]
fn shared_and_private_blocks_share_the_slot_space() {
    let src = "shared data s { a: Word } private data p { b: Word } \
        fn f(x: Word) -> Word { p.b = x; s.a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// An indexed `data.field[i]` read is an IndexRead over the index, arg packing base + len.
#[test]
fn an_indexed_data_read_parses() {
    let src = "shared data d { xs: [Word; 4] } fn f(i: Word) -> Word { d.xs[i] }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // index i (slot 0), IndexRead(base 0 + len 4 * 2^24).
    assert_eq!(got.funcs[0].3, vec![(2, 0), (13, 4 * 16_777_216)]);
}

// An indexed `data.field[i] = e` assignment: index, value, IndexStore signal, then the
// folded IndexAssignIn.
#[test]
fn an_indexed_data_assignment_parses() {
    let src = "shared data d { xs: [Word; 4] } fn f(i: Word, x: Word) -> Word { d.xs[i] = x; 0 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // index i(0), value x(1), IndexStore(36); tail 0; IndexAssignIn(base 0 + len 4*2^24).
    assert_eq!(
        got.funcs[0].3,
        vec![(2, 0), (2, 1), (36, 0), (1, 0), (14, 4 * 16_777_216)]
    );
}

// The index may itself be a data read, exercising the nested idx stack.
#[test]
fn a_data_read_may_index_another() {
    let src = "shared data d { xs: [Word; 4], j: Word } fn f() -> Word { d.xs[d.j] }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A call with two arguments is a Call node packing the callee chunk index and arg count.
#[test]
fn a_call_with_arguments_parses() {
    let src = "fn g(a: Word, b: Word) -> Word { a } fn f(n: Word) -> Word { g(n, n) }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // args n(0), n(0); Call(chunk 0 + count 2*256).
    assert_eq!(got.funcs[1].3, vec![(2, 0), (2, 0), (7, 2 * 256)]);
}

// A zero-argument call and an argument that is an expression.
#[test]
fn calls_with_zero_and_expression_arguments() {
    let src = "fn z() -> Word { 0 } fn h(a: Word) -> Word { a } \
        fn f(n: Word) -> Word { z() + h(n + 1) }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // z() [Call chunk 0, 0 args]; n, 1, Add [h's arg]; h(...) [Call chunk 1, 1 arg]; Add.
    assert_eq!(
        got.funcs[2].3,
        vec![(7, 0), (2, 0), (1, 1), (3, 1), (7, 1 + 256), (3, 1)]
    );
}

// A nested call: an argument is itself a call.
#[test]
fn a_nested_call_parses() {
    let src = "fn g(a: Word) -> Word { a } fn f(n: Word) -> Word { g(g(n)) }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// An `enum` declaration before a function: the enum is consumed by the header (and its
// table accumulated) and the function still parses.
#[test]
fn an_enum_before_a_function_parses() {
    let src = "enum Color { Red, Green, Blue } fn f(n: Word) -> Word { n }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 1);
}

// An enum with explicit discriminants interleaved with functions.
#[test]
fn enums_with_explicit_discriminants_interleave() {
    let src = "fn f(a: Word) -> Word { a } \
        enum Tag { A = 5, B, C = 10 } \
        fn g(b: Word) -> Word { b }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2);
}

// An `Enum::Variant() as Word` cast folds to the variant's discriminant literal.
#[test]
fn an_enum_cast_folds_to_a_discriminant() {
    let src = "enum Op { Add, Mul, Sub } fn f() -> Word { Op::Mul() as Word }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // Op::Mul is discriminant 1.
    assert_eq!(got.funcs[0].3, vec![(1, 1)]);
}

// A cast with an explicit discriminant folds to that value.
#[test]
fn an_enum_cast_uses_explicit_discriminants() {
    let src = "enum Tag { A = 5, B } fn f() -> Word { Tag::B() as Word }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // A = 5, so B = 6.
    assert_eq!(got.funcs[0].3, vec![(1, 6)]);
}

// A unit enum-variant match pattern folds to the same integer-match forest an integer arm
// would, mixing with integer arms.
#[test]
fn an_enum_match_pattern_folds_to_a_discriminant() {
    let src = "enum Tok { Ident, Int, Eq } \
        fn f(k: Word) -> Word { match k { Tok::Ident() => k, 5 => k, _ => k } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // scrut k(0); Tok::Ident disc 0, k(0); Lit 5, k(0); wildcard k(0); MatchBuild(temp*1024+2).
    assert_eq!(
        got.funcs[0].3,
        vec![
            (2, 0),
            (1, 0),
            (2, 0),
            (1, 5),
            (2, 0),
            (2, 0),
            (34, 1024 + 2)
        ]
    );
}

// A `use path::name` native import before a function: the import is consumed and the
// function still parses.
#[test]
fn a_use_import_before_a_function_parses() {
    let src = "use math::sqrt fn f(n: Word) -> Word { n }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 1);
}

// A `use` running to end of input has no closing delimiter, so it is closed before DONE.
// A multi-segment path exercises the `::`-separated scan.
#[test]
fn a_use_import_at_end_of_input_is_closed() {
    let src = "use std::math::floor";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 0);
}

// A `require word >= N;` machine directive at the top of a program is skipped, and the
// declarations after it parse. Every compiler stage source begins with such a directive.
#[test]
fn a_require_directive_is_skipped() {
    let src = "require word >= 32; fn f(n: Word) -> Word { n + n }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 1);
    assert_eq!(got.funcs[0].3, vec![(2, 0), (2, 0), (3, 1)]);
}

// A multiheaded function with a `when` guard: each head is a separate declaration, and the
// guard (between the return type and the body) is now emitted as its own bracketed forest
// (GSTART, the guard's postorder records, its Done), matching the reference.
#[test]
fn a_when_guarded_multihead_parses() {
    let src = "yield step(r: Word) -> Word when r > 0 { yield r } \
        yield step(r: Word) -> Word { yield 0 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2);
    assert_eq!(got.funcs[0].0, 2); // yield category
    assert_eq!(got.funcs[0].3, vec![(2, 0), (24, 0)]); // body: yield r (Local 0, YieldExpr)
    assert_eq!(got.funcs[0].4, vec![(2, 0), (1, 0), (3, 9)]); // guard: r > 0 (Local, Lit 0, Gt)
    assert!(got.funcs[1].4.is_empty()); // the unguarded head has no guard records
}

// A realistic body mixing a data blocks, a let, a data assignment, an indexed read, an
// if statement, a call, and an enum-scrutinee match, as the compiler stages combine them.
#[test]
fn a_realistic_stage_like_body_parses() {
    let src = "enum Kind { A, B } \
        shared data src { buf: [Word; 8] } \
        private data st { pos: Word, acc: Word } \
        fn helper(x: Word) -> Word { x + 1 } \
        fn run(n: Word) -> Word { \
            let x = helper(n); \
            st.pos = x; \
            if n > 0 { st.acc = src.buf[st.pos]; } else { st.acc = 0; } \
            match n { 1 => Kind::A() as Word, _ => st.acc } \
        }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2);
}

// Data reads as call arguments: each read flushes before the `,` or `)` boundary.
#[test]
fn data_reads_as_call_arguments() {
    let src = "fn g(a: Word, b: Word) -> Word { a } shared data d { x: Word, y: Word } \
        fn f() -> Word { g(d.x, d.y) }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A data read as a match scrutinee flushes before the scrutinee's `{`.
#[test]
fn a_data_read_match_scrutinee() {
    let src = "shared data d { sel: Word } fn f() -> Word { match d.sel { 1 => 10, _ => 20 } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A data read inside an `if` condition flushes before the comparison operator.
#[test]
fn a_data_read_in_an_if_condition() {
    let src = "shared data d { flag: Word } fn f() -> Word { if d.flag > 0 { 1 } else { 0 } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// Nested matches: an inner match is an arm result of an outer match.
#[test]
fn nested_matches() {
    let src = "fn f(n: Word) -> Word { match n { 1 => match n { 2 => n, _ => n }, _ => n } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A `for .. limit` body that reads and writes data, indexing an array by the loop var.
#[test]
fn a_for_limit_body_with_data_access() {
    let src = "shared data d { acc: Word, xs: [Word; 4] } \
        fn f(n: Word) -> Word { for i in 0..n limit 4 { d.acc = d.acc + d.xs[i]; } d.acc }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// An enum cast as a call argument flushes before the `)`.
#[test]
fn an_enum_cast_as_call_argument() {
    let src = "enum E { A, B } fn g(a: Word) -> Word { a } \
        fn f() -> Word { g(E::B() as Word) }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A dispatch function shaped like the compiler's own step functions: nested if/else with
// enum-cast conditions, data writes, an indexed read, and a match with a data-read result.
#[test]
fn a_stage_shaped_dispatch_function_parses() {
    let src = "enum Tok { A, B, C } \
        shared data src { kinds: [Word; 8] } \
        private data ps { cursor: Word, state: Word } \
        fn step(k: Word) -> Word { \
            if k == Tok::A() as Word { \
                ps.state = 1; \
                0 \
            } else { \
                if k == Tok::B() as Word { \
                    ps.cursor = ps.cursor + 1; \
                    src.kinds[ps.cursor] \
                } else { \
                    match k { 1 => ps.state, _ => 0 } \
                } \
            } \
        }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A real stage function (parse.kel's own emit_op): a match over enum-variant patterns
// whose arm results are enum casts, with an enum-cast-plus-arithmetic wildcard. This
// combines enum patterns, enum casts in arm results, and arithmetic as the stages do.
#[test]
fn the_emit_op_stage_function_parses() {
    let src = "enum Op { Neg, Not, Andalso, Orelse, YieldMark } \
        enum Nd { BinOp, Neg, Not, Andalso, Orelse, YieldExpr } \
        fn emit_op(op: Word) -> Word { \
            match op { \
                Op::Neg() => Nd::Neg() as Word, \
                Op::Not() => Nd::Not() as Word, \
                Op::Andalso() => Nd::Andalso() as Word, \
                Op::Orelse() => Nd::Orelse() as Word, \
                Op::YieldMark() => Nd::YieldExpr() as Word, \
                _ => Nd::BinOp() as Word + op * 64 \
            } \
        }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A scan function shaped like the parser's own table lookups (resolve_param, step_ident):
// a `for .. limit` loop whose body is an else-less `if` with an indexed data read in the
// condition and data writes in the body, plus a data-read tail.
#[test]
fn a_stage_scan_loop_function_parses() {
    let src = "shared data src { chunks: [Word; 8], chunk_count: Word } \
        private data st { found: Word, flag: Word } \
        fn resolve(v: Word) -> Word { \
            st.found = 0; \
            st.flag = 0; \
            for i in 0..src.chunk_count limit 8 { \
                if src.chunks[i] == v { \
                    st.found = i; \
                    st.flag = 1; \
                } \
            } \
            st.found \
        }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A whole mini-stage program combining every declaration form and body construct at once:
// a require directive, a use import, an enum, shared/private/const data blocks, and
// functions using if/else, a for-limit loop with data access and a call, a when-guarded
// multiheaded yield reading and writing data, and a match mixing integer and enum arms.
// The strongest self-parse test: the closest to a real stage source file.
#[test]
fn a_whole_mini_stage_program_parses() {
    let src = "enum Kind { Lo, Hi } \
        shared data src { buf: [Word; 16], len: Word } \
        private data st { pos: Word, acc: Word } \
        const data cfg { cap: Word = 16 } \
        fn clamp(x: Word) -> Word { if x > 100 { 100 } else { x } } \
        fn scan(target: Word) -> Word { \
            st.pos = 0; \
            st.acc = 0; \
            for i in 0..src.len limit 16 { \
                if src.buf[i] == target { st.acc = st.acc + 1; } \
            } \
            clamp(st.acc) \
        } \
        yield emit(resume: Word) -> Word when st.pos < src.len { \
            st.pos = st.pos + 1; \
            yield src.buf[st.pos] \
        } \
        yield emit(resume: Word) -> Word { yield 0 } \
        fn classify(n: Word) -> Word { \
            match n { 0 => Kind::Lo() as Word, 1 => Kind::Hi() as Word, _ => n } \
        }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 5); // clamp, scan, emit, emit, classify
}

// The BASELINE TORTURE TEST ported from the retired body.kel: an expression statement nested in a `let`'s
// value (`let a = if c { g(n); n } else { 0 };`) must be an ExprStmt without clobbering
// the enclosing `let`, which must still be a LetIn. Getting the pending-statement baseline
// wrong would mis-commit the outer let, a silent valid-but-wrong forest.
#[test]
fn an_expr_statement_nested_in_a_let_value_keeps_the_let() {
    let src = "fn g(x: Word) -> Word { x } \
        fn f(c: Word, n: Word) -> Word { let a = if c > 0 { g(n); n } else { 0 }; a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    // The root is the outer LetIn (kind 5), not an ExprStmt.
    assert_eq!(got.funcs[1].3.last().unwrap().0, 5);
}

// A block-form `if` statement inside a `for` body: the pending-statement baseline is
// pushed for both the for body and the branch blocks.
#[test]
fn a_block_form_if_statement_inside_a_for_body() {
    let src = "shared data d { acc: Word } \
        fn f(n: Word) -> Word { for i in 0..n limit 8 { if i > 0 { d.acc = i; } else { d.acc = 0; } } d.acc }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// A `match` as a `let` value with an enum-variant arm, nested in a further block.
#[test]
fn a_match_let_value_with_an_enum_arm() {
    let src = "enum E { A, B } \
        fn f(n: Word) -> Word { let r = match n { 1 => E::A() as Word, _ => n }; r + 1 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
}

// ACTUAL STAGE SOURCE, VERBATIM. The three real enums parse.kel declares (`Tok`, `Node`,
// `OpCode`) with their explicit, out-of-order discriminants, and three of parse.kel's own
// self-contained functions (`prec_of`, `opcode_of`, `emit_op`) copied verbatim. Unlike the
// paraphrased emit_op test, this feeds the real out-of-order discriminants (Underscore=50
// then LBracket=41, ...), so it exercises parse.kel's explicit-discriminant enum-table
// accumulation against the genuine stage vocabulary: every `Tok::X()`/`OpCode::X()` pattern
// and every `Node::X() as Word` cast must resolve to the discriminant literal the reference
// compiler assigns. This is parse.kel parsing its own source, the strongest self-parse yet.
#[test]
fn parse_kel_parses_its_own_enums_and_operator_functions() {
    let src = "\
        enum Tok { \
            Fn = 0, Ident = 1, LBrace = 2, RBrace = 3, Yield = 5, Loop = 6, LParen = 7, \
            RParen = 8, Colon = 9, Comma = 10, IntLit = 12, Data = 13, Shared = 14, \
            Private = 15, Const = 16, Eq = 17, Use = 19, Plus = 21, Minus = 22, Star = 23, \
            Slash = 24, Percent = 25, EqEq = 26, NotEq = 27, Lt = 28, Gt = 29, LtEq = 30, \
            GtEq = 31, Not = 32, Band = 33, Bor = 34, Bxor = 35, Andalso = 36, Orelse = 37, \
            Let = 38, Semi = 39, Dot = 40, If = 43, Else = 44, For = 45, In = 46, \
            DotDot = 47, Match = 48, FatArrow = 49, Underscore = 50, LBracket = 41, \
            RBracket = 42, ColCol = 51, As = 52, Enum = 53 } \
        enum Node { \
            Literal = 1, Local = 2, BinOp = 3, If = 4, LetIn = 5, Not = 6, Call = 7, \
            DataRead = 11, DataAssignIn = 12, IndexRead = 13, IndexAssignIn = 14, \
            Andalso = 8, Orelse = 9, Neg = 10, Done = 15, Unit = 20, ExprStmt = 21, \
            ForLimit = 23, YieldExpr = 24, SlotRecord = 32, ForBuild = 33, MatchBuild = 34, \
            IndexStore = 36 } \
        enum OpCode { \
            Add = 1, Mul = 2, Sub = 3, Div = 4, Mod = 5, Eq = 6, NotEq = 7, Lt = 8, Gt = 9, \
            LtEq = 10, GtEq = 11, Band = 12, Bor = 13, Bxor = 14, Paren = 15, Neg = 16, \
            Not = 17, Andalso = 18, Orelse = 19, IndexMark = 20, CallMark = 21, \
            YieldMark = 22 } \
        fn prec_of(op: Word) -> Word { \
            match op { \
                OpCode::Neg() => 9, OpCode::Not() => 9, OpCode::Paren() => 0, \
                OpCode::IndexMark() => 0, OpCode::YieldMark() => 0, OpCode::Mul() => 8, \
                OpCode::Div() => 8, OpCode::Mod() => 8, OpCode::Add() => 7, \
                OpCode::Sub() => 7, OpCode::Band() => 6, OpCode::Bxor() => 5, \
                OpCode::Bor() => 4, OpCode::Andalso() => 2, OpCode::Orelse() => 1, _ => 3 } } \
        fn opcode_of(k: Word) -> Word { \
            match k { \
                Tok::Plus() => OpCode::Add() as Word, Tok::Minus() => OpCode::Sub() as Word, \
                Tok::Star() => OpCode::Mul() as Word, Tok::Slash() => OpCode::Div() as Word, \
                Tok::Percent() => OpCode::Mod() as Word, Tok::EqEq() => OpCode::Eq() as Word, \
                Tok::NotEq() => OpCode::NotEq() as Word, Tok::Lt() => OpCode::Lt() as Word, \
                Tok::Gt() => OpCode::Gt() as Word, Tok::LtEq() => OpCode::LtEq() as Word, \
                Tok::GtEq() => OpCode::GtEq() as Word, Tok::Band() => OpCode::Band() as Word, \
                Tok::Bor() => OpCode::Bor() as Word, Tok::Bxor() => OpCode::Bxor() as Word, \
                Tok::Andalso() => OpCode::Andalso() as Word, \
                Tok::Orelse() => OpCode::Orelse() as Word, _ => 0 } } \
        fn emit_op(op: Word) -> Word { \
            match op { \
                OpCode::Neg() => Node::Neg() as Word, OpCode::Not() => Node::Not() as Word, \
                OpCode::Andalso() => Node::Andalso() as Word, \
                OpCode::Orelse() => Node::Orelse() as Word, \
                OpCode::YieldMark() => Node::YieldExpr() as Word, \
                _ => Node::BinOp() as Word + op * 64 } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 3); // prec_of, opcode_of, emit_op
}

// A data block wider than the field table's seed capacity. parse.kel accumulates one
// field-layout entry per data-block field into fixed arrays scanned by fixed-range loops;
// a whole stage file declares far more fields than the original 64-entry sizing, the same
// fixed-capacity class the enum vocabulary hit. This builds a block of 70 scalar fields and
// reads the last, which overflows a 64-entry table at accumulation and traps a 64-cap scan
// at resolution. It must resolve the last field to its slot exactly as the reference does.
#[test]
fn a_data_block_wider_than_the_field_table_seed_capacity() {
    const FIELDS: usize = 70;
    let mut block = String::from("shared data wide { ");
    for i in 0..FIELDS {
        if i > 0 {
            block.push_str(", ");
        }
        block.push_str(&format!("f{i}: Word"));
    }
    block.push_str(" } ");
    let src = format!("{block} fn read_last() -> Word {{ wide.f{} }}", FIELDS - 1);
    let mut names = Vec::new();
    let got = run_parse(&src, &mut names);
    assert_eq!(got, reference(&src, &names));
    assert_eq!(got.funcs[0].3, vec![(11, (FIELDS - 1) as i64)]); // DataRead of slot 69
}

// A `struct` declaration between two functions: its two scalar fields are captured
// (STRUCTSTART then PARAM/PTYPE per field) and validated against the reference StructDef,
// and the surrounding functions parse to the same records the reference produces. The
// struct's fields are not slotted (`commit_field` leaves the data-field table untouched).
#[test]
fn a_struct_declaration_has_its_fields_captured() {
    let src = "fn a() -> Word { 1 } \
               struct P { x: Word, y: Word } \
               fn b(p: Word) -> Word { p }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2, "both functions parsed");
    assert_eq!(got.structs.len(), 1, "one struct captured");
    assert_eq!(got.structs[0].1.len(), 2, "two fields captured");
    assert_eq!(got.structs[0].1[0].2, 0, "x is a scalar field"); // array length 0
    assert_eq!(got.structs[0].1[1].2, 0, "y is a scalar field");
    assert_eq!(got.funcs[0].3, vec![(1, 1)]); // a: Literal 1
    assert_eq!(got.funcs[1].3, vec![(2, 0)]); // b: Local slot 0 (p)
}

// A struct with an array field exercises the `[T; N]` field path (PTYPE + ASIZE): the
// captured field records its element type and length 4. A following function's own
// `data.field` read still resolves, proving the struct's fields did not pollute the
// data-field layout table.
#[test]
fn a_struct_with_an_array_field_captures_its_length() {
    let src = "struct Buf { xs: [Word; 4] } \
               shared data s { a: Word } \
               fn read() -> Word { s.a }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 1, "the function parsed");
    assert_eq!(got.structs.len(), 1, "one struct captured");
    assert_eq!(got.structs[0].1.len(), 1, "one field captured");
    assert_eq!(got.structs[0].1[0].2, 4, "xs is an array of length 4");
    assert_eq!(got.funcs[0].3, vec![(11, 0)]); // read: DataRead of s.a at slot 0
}

// A generic struct needs no extra parser machinery: after the struct name, mode 6 ignores
// every token until the body's `{`, so the `<...>` type parameters are skipped for free.
// Its two fields (typed by the generic parameters) are still captured and validated.
#[test]
fn a_generic_struct_declaration_has_its_fields_captured() {
    let src = "fn a() -> Word { 7 } \
               struct Pair<A, B> { first: A, second: B } \
               fn b() -> Word { 8 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2, "both functions parsed");
    assert_eq!(got.structs.len(), 1, "the generic struct captured");
    assert_eq!(
        got.structs[0].1.len(),
        2,
        "two generic-typed fields captured"
    );
    assert_eq!(got.funcs[0].3, vec![(1, 7)]); // a: Literal 7
    assert_eq!(got.funcs[1].3, vec![(1, 8)]); // b: Literal 8
}

// A struct construction `P { x: 1, y: 2 }` in a body parses to its field values in order
// (Literal 1, Literal 2) then a StructInit node (kind 27) carrying the field count, matching
// the reference flatten. The stage recognises `P` as a struct type name (from its accumulated
// struct table), opens the field scan at `{`, drains each field value at `,`/`}`, and emits
// the StructInit at the closing `}`.
#[test]
fn a_struct_construction_parses_to_a_struct_init() {
    let src = "struct P { x: Word, y: Word }\n\
               fn make() -> P { P { x: 1, y: 2 } }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 1, "make parsed");
    // Postorder: the two field values, then the StructInit packing the flat byte size (16 =
    // two Words) and field count 2 as 16 * 1024 + 2.
    assert_eq!(got.funcs[0].3, vec![(1, 1), (1, 2), (27, 16 * 1024 + 2)]);
}

// A `trait` declaration between two functions: the stage emits TRAITSTART (19), captures
// each method signature's name (MNAME), and emits END; the two method names are validated
// against the reference TraitDef, and the surrounding functions parse to the reference
// records (a trait's methods land in `program.traits`, not `program.functions`).
#[test]
fn a_trait_declaration_captures_its_method_names() {
    let src = "fn a() -> Word { 1 } \
               trait Shape { fn area(self) -> Word; fn name(self) -> Word; } \
               fn b() -> Word { 2 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 2, "both functions parsed");
    assert_eq!(got.traits.len(), 1, "one trait captured");
    assert_eq!(
        got.traits[0].len(),
        2,
        "two method names captured (area, name)"
    );
    assert_eq!(got.funcs[0].3, vec![(1, 1)]); // a: Literal 1
    assert_eq!(got.funcs[1].3, vec![(1, 2)]); // b: Literal 2
}

// An `impl Trait for Type` block has its method NAMES captured (each `fn` at body depth one),
// while its methods' brace-nested bodies are still consumed (the `idepth` counter balances
// them). Its methods land in `program.impls`, not `program.functions`, so the top-level
// function matches and the impl's one method name is validated against the reference. The
// struct and trait it depends on are captured by their own paths.
#[test]
fn an_impl_block_captures_its_method_names() {
    let src = "struct S { c: Word } \
               trait Cap { fn cap(self) -> Word; } \
               impl Cap for S { fn cap(s: S) -> Word { s.c } } \
               fn top() -> Word { 5 }";
    let mut names = Vec::new();
    let got = run_parse(src, &mut names);
    assert_eq!(got, reference(src, &names));
    assert_eq!(got.funcs.len(), 1, "the top-level function parsed");
    assert_eq!(got.impls.len(), 1, "one impl captured");
    assert_eq!(got.impls[0].len(), 1, "its one method name (cap) captured");
    assert_eq!(got.traits.len(), 1, "the trait captured too");
    assert_eq!(got.funcs[0].3, vec![(1, 5)]); // top: Literal 5
}
