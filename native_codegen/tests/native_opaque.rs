//! **OPERATOR QUESTION 14: AN OPAQUE HANDLE IS A TYPED HOST POINTER.**
//!
//! The operator ruled on 2026-10-28 for option O1 of
//! `docs/decisions/ORDER_6_DECISION_OPTIONS.md`. Native code carries the
//! host's pointer as a word and never reads through it, and the generated
//! header declares a distinct incomplete structure per opaque type.
//!
//! A probe before this file found that four routes already lowered as
//! untyped words: a result discarded, passed to another native, held across a
//! yield, and received as a parameter. What this file adds is the typed
//! declaration, the refusal of a contract the word cannot carry, and the
//! execution of those routes against the virtual machine.
//!
//! # What this does not establish
//!
//! A struct field and a shared slot holding an opaque value stay refused, as
//! `ORDER_6_RULINGS_PLAN.md` records. The type distinction is a C compile-time
//! property only. Native code cannot detect a stale or wrong pointer, so the
//! differential shows that a valid pointer survives the routes unchanged, not
//! that an invalid one is caught.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for};
use keleusma::{HostOpaque, host_arc};
use keleusma_native::coroutine::{HostContracts, NativeOpaque};
use keleusma_native::linkage::{self, LinkageError};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

/// One handle made from the start argument, held across the yield, and passed
/// to a native twice through a function parameter.
const SOURCE: &str = "use host::make(Word) -> Device\n\
                      use host::poke(Device, Word) -> Word\n\
                      fn twice(d: Device, a: Word) -> Word { host::poke(d, a) + host::poke(d, a) }\n\
                      yield main(x: Word) -> Word { let d = host::make(x); let a = yield x; twice(d, a) }";

fn parse(src: &str) -> keleusma::ast::Program {
    let tokens = keleusma::lexer::tokenize(src).unwrap();
    keleusma::parser::parse(&tokens).unwrap()
}

fn machine(triple: &TargetTriple) -> TargetMachine {
    Target::initialize_all(&InitializationConfig::default());
    Target::from_triple(triple)
        .unwrap()
        .create_target_machine(
            triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

fn host_machine() -> TargetMachine {
    machine(&TargetMachine::get_default_triple())
}

fn contracts(program: &keleusma::ast::Program, module: &Module) -> HostContracts {
    HostContracts {
        opaque_natives: keleusma_native::opaque::opaque_natives(program, module),
        ..HostContracts::default()
    }
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kel-opaque-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_source_names_each_opaque_position_and_nothing_else() {
    let src = "use host::make(Word) -> Device\n\
               use host::poke(Device, Word) -> Word\n\
               use host::area(Shape) -> Word\n\
               use host::bare\n\
               struct Shape { w: Word, h: Word }\n\
               yield main(x: Word) -> Word {\n\
                   let d = host::make(x);\n\
                   let s = Shape { w: x, h: x };\n\
                   let a = yield host::poke(d, host::area(s));\n\
                   host::bare(a)\n\
               }";
    let program = parse(src);
    let module = common::build(src);
    let index = |name: &str| {
        u16::try_from(module.native_names.iter().position(|n| n == name).unwrap()).unwrap()
    };
    let found = keleusma_native::opaque::opaque_natives(&program, &module);
    let device = || Some(String::from("Device"));
    let expected = BTreeMap::from([
        (
            index("host::make"),
            NativeOpaque {
                params: vec![None],
                ret: device(),
            },
        ),
        (
            index("host::poke"),
            NativeOpaque {
                params: vec![device(), None],
                ret: None,
            },
        ),
    ]);
    // `Shape` is declared, so it is not opaque, and the bare `use` declares no
    // signature, so neither native has an entry.
    assert_eq!(found, expected);
}

#[test]
fn without_a_contract_the_header_keeps_its_untyped_words() {
    let module = common::build(SOURCE);
    let ctx = Context::create();
    let lib = linkage::build(
        &ctx,
        &module,
        &host_machine(),
        "kel_dev",
        &HostContracts::default(),
    )
    .unwrap();
    let header = lib.c_header();
    assert!(!header.contains("kel_opaque_"), "{header}");
    let make = keleusma_native::native_symbol("host::make");
    assert!(
        header.contains(&format!("int64_t {make}(int64_t);")),
        "{header}"
    );
}

#[test]
fn the_header_and_bindings_type_each_opaque_position() {
    let module = common::build(SOURCE);
    let ctx = Context::create();
    let lib = linkage::build(
        &ctx,
        &module,
        &host_machine(),
        "kel_dev",
        &contracts(&parse(SOURCE), &module),
    )
    .unwrap();
    let (make, poke) = (
        keleusma_native::native_symbol("host::make"),
        keleusma_native::native_symbol("host::poke"),
    );
    let header = lib.c_header();
    for line in [
        String::from("struct kel_opaque_Device;"),
        format!("struct kel_opaque_Device * {make}(int64_t);"),
        format!("int64_t {poke}(struct kel_opaque_Device *, int64_t);"),
    ] {
        assert!(
            header.contains(&line),
            "the header lacks `{line}`:\n{header}"
        );
    }
    assert_eq!(header.matches("struct kel_opaque_Device;").count(), 1);
    let rust = lib.rust_bindings();
    for line in [
        String::from("pub struct kel_opaque_Device {"),
        format!("pub type {make} = extern \"C\" fn(i64) -> *mut kel_opaque_Device;"),
        format!("pub type {poke} = extern \"C\" fn(*mut kel_opaque_Device, i64) -> i64;"),
    ] {
        assert!(rust.contains(&line), "the bindings lack `{line}`:\n{rust}");
    }
}

/// MUST-FIRE: each contract the word or the header cannot honour is refused.
#[test]
fn a_contract_the_word_cannot_carry_is_refused() {
    let module = common::build(SOURCE);
    let program = parse(SOURCE);
    let good = contracts(&program, &module);
    let refuse = |c: &HostContracts, tm: &TargetMachine, needle: &str| {
        let ctx = Context::create();
        match linkage::build(&ctx, &module, tm, "kel_dev", c) {
            Err(LinkageError::Signature(m)) => assert!(m.contains(needle), "{m}"),
            Err(e) => panic!("refused for another reason: {e:?}"),
            Ok(_) => panic!("accepted a contract containing {needle}"),
        }
    };
    let tm = host_machine();
    let mut missing = good.clone();
    missing.opaque_natives.insert(
        99,
        NativeOpaque {
            params: vec![],
            ret: Some(String::from("Device")),
        },
    );
    refuse(&missing, &tm, "does not exist");
    let mut short = good.clone();
    for entry in short.opaque_natives.values_mut() {
        entry.params.push(None);
    }
    refuse(&short, &tm, "parameters");
    let mut spelled = good.clone();
    for entry in spelled.opaque_natives.values_mut() {
        if entry.ret.is_some() {
            entry.ret = Some(String::from("not-an-ident"));
        }
    }
    refuse(&spelled, &tm, "not a C identifier");
    // A 32-bit target cannot carry a 64-bit host pointer in its word.
    refuse(
        &good,
        &machine(&TargetTriple::create("i686-unknown-linux-gnu")),
        "64-bit addresses and pointers",
    );
}

#[test]
fn the_command_writes_the_typed_header() {
    let dir = scratch("cli");
    let src = dir.join("device.kel");
    std::fs::write(&src, SOURCE).unwrap();
    let out = dir.join("out");
    let ran = Command::new(env!("CARGO_BIN_EXE_kel-native"))
        .arg("build")
        .arg(&src)
        .args(["--prefix", "kel_dev", "--out"])
        .arg(&out)
        .output()
        .unwrap();
    assert!(ran.status.success(), "{ran:?}");
    let header = std::fs::read_to_string(out.join("kel_dev.h")).unwrap();
    assert!(header.contains("struct kel_opaque_Device;"), "{header}");
    let _ = std::fs::remove_dir_all(&dir);
}

struct Device {
    n: i64,
}

impl HostOpaque for Device {
    fn type_name(&self) -> &'static str {
        "Device"
    }
}

/// The virtual machine's outcomes: the yield of `start`, then the result.
fn vm_outcomes(module: &Module, start: i64, reply: i64) -> (i64, i64) {
    let arena =
        keleusma_arena::Arena::with_capacity(auto_arena_capacity_for(module, &[]).unwrap() + 65536);
    let mut vm = Vm::new(module.clone(), &arena).unwrap();
    vm.register_native("host::make", |args| match args {
        [Value::Int(x)] => Ok(Value::Opaque(host_arc(Device { n: x * 3 }))),
        other => panic!("make({other:?})"),
    });
    vm.register_native("host::poke", |args| match args {
        [Value::Opaque(d), Value::Int(a)] => {
            let device = d.as_ref().downcast_ref::<Device>().expect("a Device");
            Ok(Value::Int(device.n + a))
        }
        other => panic!("poke({other:?})"),
    });
    let VmState::Yielded(Value::Int(y)) = vm.call(&[Value::Int(start)]).unwrap() else {
        panic!("the VM did not yield")
    };
    let VmState::Finished(Value::Int(r)) = vm.resume(Value::Int(reply)).unwrap() else {
        panic!("the VM did not finish")
    };
    (y, r)
}

#[test]
fn an_opaque_handle_crosses_a_yield_and_a_call_and_agrees_with_the_vm() {
    if Command::new("cc").arg("--version").output().is_err() {
        assert!(
            std::env::var_os("KEL_REQUIRE_LLVM_TOOLS").is_none()
                && std::env::var_os("CI").is_none(),
            "`cc` is required here and could not be run"
        );
        eprintln!("\nSKIPPED: `cc` was not found, so the opaque differential did not run.\n");
        return;
    }
    let module = common::build(SOURCE);
    let tm = host_machine();
    let ctx = Context::create();
    let lib = linkage::build(
        &ctx,
        &module,
        &tm,
        "kel_dev",
        &contracts(&parse(SOURCE), &module),
    )
    .unwrap();
    let dir = scratch("run");
    lib.write_object(&tm, &dir.join("kel_dev.o")).unwrap();
    std::fs::write(dir.join("kel_dev.h"), lib.c_header()).unwrap();
    let (make, poke) = (
        keleusma_native::native_symbol("host::make"),
        keleusma_native::native_symbol("host::poke"),
    );
    // The host's structure is complete only here. Native code sees a pointer.
    let host = format!(
        r#"#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "kel_dev.h"
struct kel_opaque_Device {{ int64_t n; }};
struct kel_opaque_Device *{make}(int64_t x) {{
    struct kel_opaque_Device *d = malloc(sizeof *d);
    if (!d) exit(4);
    d->n = x * 3;
    return d;
}}
int64_t {poke}(struct kel_opaque_Device *d, int64_t a) {{ return d->n + a; }}
static _Alignas(8) unsigned char slot[KEL_DEV_SLOT_BYTES];
static _Alignas(8) unsigned char shared[KEL_DEV_SHARED_BYTES + 8];
static _Alignas(8) unsigned char priv[KEL_DEV_PRIVATE_BYTES + 8];
static _Alignas(8) unsigned char region[KEL_DEV_REGION_BYTES + 8];
int main(int argc, char **argv) {{
    if (argc != 3) return 64;
    struct kel_dev_outcome o = kel_dev_start(atoll(argv[1]), shared, priv, region, slot);
    printf("%llu %lld\n", (unsigned long long)o.status, (long long)o.value);
    o = kel_dev_resume(slot, atoll(argv[2]));
    printf("%llu %lld\n", (unsigned long long)o.status, (long long)o.value);
    kel_dev_release(slot);
    return 0;
}}
"#
    );
    std::fs::write(dir.join("host.c"), host).unwrap();
    let exe = dir.join("host");
    let linked = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Werror", "-I"])
        .arg(&dir)
        .arg(dir.join("host.c"))
        .arg(dir.join("kel_dev.o"))
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{}",
        String::from_utf8_lossy(&linked.stderr)
    );
    for (start, reply) in [(7, 5), (0, 0), (-4, 100)] {
        let ran = Command::new(&exe)
            .args([start.to_string(), reply.to_string()])
            .output()
            .unwrap();
        assert!(ran.status.success(), "{ran:?}");
        let lines: Vec<String> = String::from_utf8_lossy(&ran.stdout)
            .lines()
            .map(String::from)
            .collect();
        let (y, r) = vm_outcomes(&module, start, reply);
        assert_eq!(
            lines,
            [format!("1 {y}"), format!("2 {r}")],
            "start {start}, reply {reply}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
