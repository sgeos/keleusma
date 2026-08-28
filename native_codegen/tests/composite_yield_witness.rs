//! What does the native lowering hand the host when a composite is yielded?
//!
//! A composite yielded in **tail position** lowers, and nothing in the tree
//! executes it: the suspension differential's subjects all yield `Word`. That is
//! an arm that exists and has never run, which is the class this line has been
//! closing.
//!
//! It is **not** the cross-iteration escape hazard — a tail-yielded composite is
//! built once and no later iteration overwrites it. It is the marshalling of a
//! composite across the yield boundary, unexercised.
//!
//! The host receives a yield through `kel_yield(v: i64)`. A composite is not an
//! integer, so that `i64` is an encoding of something. **This establishes what,
//! by running such a program** — not by reading the lowering.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use keleusma::bytecode::Module;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for};
use keleusma_native::{LowerOptions, lower_module, region};
use std::sync::Mutex;

mod common;

static YIELDED: Mutex<Vec<i64>> = Mutex::new(Vec::new());

extern "C" fn kel_yield(v: i64) -> i64 {
    YIELDED.lock().unwrap().push(v);
    0
}

fn compile_src(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}

const SUBJECT: &str = "struct P { a: Word, b: Word }\n\
                       loop main(t: Word) -> P { yield P { a: t + 1, b: t + 2 } }";

/// Where a yielded value sits relative to the buffers the host provided.
#[derive(Debug, PartialEq, Eq)]
enum Placement {
    /// Inside the composite region the host passed in.
    InRegion { offset: usize },
    /// A small integer, i.e. not a pointer into anything the host owns.
    SmallInteger,
    /// Neither — an address the host cannot account for.
    Unaccounted,
}

fn classify(v: i64, region_base: usize, region_len: usize) -> Placement {
    let u = v as usize;
    if u >= region_base && u < region_base + region_len {
        Placement::InRegion {
            offset: u - region_base,
        }
    } else if v.unsigned_abs() < 1 << 20 {
        Placement::SmallInteger
    } else {
        Placement::Unaccounted
    }
}

/// **The classifier must be able to report each of its three answers**, or a
/// single observed answer says nothing about what it can distinguish.
#[test]
fn the_placement_classifier_reports_each_of_its_answers() {
    let base = 0x1000_0000usize;
    assert_eq!(
        classify((base + 24) as i64, base, 64),
        Placement::InRegion { offset: 24 }
    );
    assert_eq!(classify(7, base, 64), Placement::SmallInteger);
    assert_eq!(
        classify((base + 4096) as i64, base, 64),
        Placement::Unaccounted
    );
}

#[test]
fn what_the_native_side_yields_for_a_composite() {
    YIELDED.lock().unwrap().clear();
    let m = compile_src(SUBJECT);
    let entry = m.entry_point.expect("entry");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("the subject lowers");
    lm.verify().expect("IR valid");
    common::maybe_optimize(&lm);

    // **WHAT THE MODULE ACTUALLY DECLARES, BEFORE ASSUMING HOW IT SUSPENDS.**
    // The first version of this probe demanded a `kel_yield` hook and failed on
    // its own expect message, which had anticipated exactly this: "if it does
    // not, the lowering suspends some other way and this probe is aimed
    // wrongly".
    let declared: Vec<String> = lm
        .get_functions()
        .map(|f| f.get_name().to_string_lossy().to_string())
        .collect();
    println!("\n================ WHAT A TAIL COMPOSITE YIELD LOWERS TO");
    println!("  functions in the lowered module: {declared:?}");
    let suspends_through_host = declared.iter().any(|n| n == "kel_yield");
    println!("  declares a kel_yield hook: {suspends_through_host}");

    let ee = lm
        .create_jit_execution_engine(OptimizationLevel::None)
        .expect("jit");
    if suspends_through_host {
        let hook = lm.get_function("kel_yield").expect("just observed");
        ee.add_global_mapping(&hook, kel_yield as *const () as usize);
    }

    const CANARY: u64 = 0xDEAD_BEEF_FEED_FACE;
    let n_region = region::region_total_bytes(&m, entry, 0) as usize;
    let mut region_buf = vec![0u64; n_region.div_ceil(8) + 1];
    let canary_at = n_region.div_ceil(8);
    region_buf[canary_at] = CANARY;
    let mut shared = CANARY.to_le_bytes().to_vec();
    let mut privs = vec![CANARY; 1];

    let sym = format!("kel_chunk_{entry}");
    let f = lm.get_function(&sym).expect("entry");
    let pc = u32::from(m.chunks[entry].param_count);
    assert_eq!(
        f.count_params(),
        pc + 3,
        "this subject builds a composite, so the entry must carry the three \
         trailing pointers"
    );
    let callable = unsafe {
        ee.get_function::<unsafe extern "C" fn(i64, *mut u8, *mut u8, *mut u8) -> i64>(&sym)
    }
    .expect("entry symbol");

    let region_base = region_buf.as_ptr() as usize;
    let region_len = n_region;
    let ret = unsafe {
        callable.call(
            5,
            shared.as_mut_ptr(),
            privs.as_mut_ptr() as *mut u8,
            region_buf.as_mut_ptr() as *mut u8,
        )
    };

    assert_eq!(
        region_buf[canary_at], CANARY,
        "the lowering wrote past the composite region"
    );

    let seen = YIELDED.lock().unwrap().clone();
    println!("  region base 0x{region_base:x}, {region_len} bytes");
    println!(
        "  entry returned {ret} -> {:?}",
        classify(ret, region_base, region_len)
    );
    println!("  yields captured through the host hook: {}", seen.len());
    for v in &seen {
        println!(
            "    raw {v} (0x{:x}) -> {:?}",
            *v as usize,
            classify(*v, region_base, region_len)
        );
    }
    println!(
        "\n  A SINGLE TAIL YIELD IS NOT A SUSPENSION HERE. With nothing after it,\n  \
         the value the host would receive is the value the entry returns, so the\n  \
         lowering hands it back through the RETURN rather than through a hook.\n  \
         That means the marshalling exercised is the composite-RETURN ABI, which\n  \
         `composite_return_aliasing.rs` already covers, and not a distinct\n  \
         yield-boundary path.\n================\n"
    );

    assert!(
        !suspends_through_host,
        "the subject now suspends through a host hook, so a tail composite yield \
         is no longer lowered as a return and the reasoning above needs redoing"
    );
    assert!(
        seen.is_empty(),
        "the host hook was called {} time(s) despite not being declared",
        seen.len()
    );
    assert_ne!(
        classify(ret, region_base, region_len),
        Placement::Unaccounted,
        "the returned value is neither a small integer nor inside the region the \
         host provided, so the host cannot account for what it was handed"
    );

    // **THE BODY, NOT JUST THE HANDLE.** Comparing an address to an address
    // would prove nothing about marshalling. The two fields are words at the
    // start of the region, and `t + 1`, `t + 2` with `t = 5` makes them 6 and 7.
    let a = region_buf[0] as i64;
    let b = region_buf[1] as i64;
    println!("  body read back from the region: a={a}, b={b}");

    // The reference, run on the same module, for the same input.
    let cap = auto_arena_capacity_for(&m, &[]).expect("arena capacity");
    let arena = keleusma_arena::Arena::with_capacity(cap);
    let mut vm = Vm::new(m.clone(), &arena).expect("vm");
    let state = vm.call(&[Value::Int(5)]).expect("vm run");
    assert!(
        matches!(state, VmState::Yielded(_)),
        "the reference must SUSPEND here, which is what makes the native side \
         returning instead the finding: {state:?}"
    );

    // **RESOLVE THE HANDLE, DO NOT PRINT IT.** A first attempt compared the
    // reference's `Debug` text for the field values; that text shows the arena
    // handle and not the body, so the check failed for the right reason. The
    // bytes have to be read through the arena.
    use keleusma::bytecode::StructBody;
    let reference_bytes = match &state {
        VmState::Yielded(Value::Struct(StructBody::Flat(fc))) => {
            fc.resolve(&arena).expect("a live flat body").to_vec()
        }
        other => panic!("expected a yielded flat struct, got {other:?}"),
    };
    let native_bytes: Vec<u8> = region_buf[..2]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();
    println!("  reference body: {reference_bytes:?}");
    println!("  native body   : {native_bytes:?}");
    println!(
        "\n  THE BODIES ARE COMPARED, NOT THE HANDLES. Comparing an address to an\n  \
         address would prove nothing about marshalling.\n================\n"
    );

    assert_eq!(
        (a, b),
        (6, 7),
        "the native body is not what the source computes"
    );
    assert_eq!(
        native_bytes, reference_bytes,
        "the native and reference composite bodies differ"
    );
}
