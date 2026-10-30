//! **ORDER 5, WORKSTREAM H: A NATIVE LIBRARY IS REPLACED WHILE ITS HOST RUNS.**
//!
//! The virtual machine hot-swaps through `Vm::replace_module`. The host keeps
//! its shared buffer, supplies the new module's private data, and the next call
//! starts the new module's entry. A swap is refused when the new module's
//! `schema_hash` differs, because the shared buffer would then mean something
//! else to the new code.
//!
//! This file does the same natively. Each version of a program is exported by
//! `linkage::build` under one prefix and linked as its own shared object. A C
//! host loads the first with `dlopen`, runs it, and at a step boundary loads the
//! second, compares the two libraries' exported `schema_hash` and
//! `shared_bytes`, releases the old instance, unloads the old library and starts
//! the new one on the same shared buffer. The yields and the final shared bytes
//! must equal the virtual machine's across the same swap, and a version whose
//! schema differs must be refused by both.
//!
//! # What this does not establish
//!
//! The swap is of one whole library at a step boundary. A live instance is
//! released, not migrated, as in the virtual machine. Only the dynamic loader
//! of the executed target is exercised, `dlopen` on `aarch64-apple-darwin`.
//! Cross-library inlining, the strategy's concern for native hot replacement,
//! does not arise, because each version is one library and calls only itself.
mod common;

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target, TargetMachine};
use keleusma::bytecode::{Module, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma_native::coroutine::HostContracts;
use keleusma_native::linkage;
use std::path::{Path, PathBuf};
use std::process::Command;

fn machine() -> TargetMachine {
    Target::initialize_native(&InitializationConfig::default()).unwrap();
    let triple = TargetMachine::get_default_triple();
    Target::from_triple(&triple)
        .unwrap()
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .unwrap()
}

const V1: &str = "shared data io { total: Word } private data st { n: Word } \
                  loop main(t: Word) -> Word { st.n = st.n + 1; io.total = io.total + t; \
                  yield st.n * 1000 + io.total }";
/// The same data layout with a different body.
const V2: &str = "shared data io { total: Word } private data st { n: Word } \
                  loop main(t: Word) -> Word { st.n = st.n + 2; io.total = io.total * 2 - t; \
                  yield st.n * 100 + io.total + 7 }";
/// A renamed shared slot, which changes the schema.
const V3: &str = "shared data io { sum: Word } private data st { n: Word } \
                  loop main(t: Word) -> Word { st.n = st.n + 1; io.sum = io.sum + t; yield io.sum + st.n }";
/// The layout of [`V1`] with one shared slot's type changed and its name
/// kept. Since absorption 119 the schema hash covers each slot's declared
/// type, under operator question 8, so this alone changes the schema.
const V4: &str = "shared data io { total: Fixed } private data st { n: Word } \
                  loop main(t: Word) -> Word { st.n = st.n + 1; yield st.n * 1000 + t }";

const FIRST: i64 = 5;
const BEFORE: &[i64] = &[3, 4, 6];
const RESTART: i64 = 9;
const AFTER: &[i64] = &[1, 2, 8, 10];

/// The virtual machine's yields across a swap from [`V1`] to `second` and its
/// final shared bytes.
fn reference(second: &str) -> (Vec<i64>, Vec<u8>) {
    let v1 = common::build(V1);
    let v2 = common::build(second);
    assert_eq!(
        v1.schema_hash, v2.schema_hash,
        "the two versions share a schema"
    );
    let need = required_persistent_capacity_for(&v1).max(required_persistent_capacity_for(&v2));
    let cap = auto_arena_capacity_for(&v1, &[])
        .unwrap()
        .max(auto_arena_capacity_for(&v2, &[]).unwrap())
        + need
        + (1 << 16);
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).unwrap();
    let mut vm = Vm::new(v1, &arena).unwrap();
    let mut shared = vec![0u8; vm_shared_bytes(&v2)];
    let mut yields = Vec::new();
    let mut step = |vm: &mut Vm<'_, '_>, shared: &mut Vec<u8>, first: i64, replies: &[i64]| {
        let mut state = vm.call_with_shared(shared, &[Value::Int(first)]).unwrap();
        let mut replies = replies.iter();
        let mut last = first;
        loop {
            match state {
                VmState::Yielded(Value::Int(v)) => {
                    yields.push(v);
                    let Some(&r) = replies.next() else { break };
                    last = r;
                    state = vm.resume_with_shared(shared, Value::Int(r)).unwrap();
                }
                VmState::Reset => state = vm.resume_with_shared(shared, Value::Int(last)).unwrap(),
                other => panic!("{other:?}"),
            }
        }
    };
    step(&mut vm, &mut shared, FIRST, BEFORE);
    vm.replace_module(v2, vec![Value::Int(0)]).unwrap();
    step(&mut vm, &mut shared, RESTART, AFTER);
    (yields, shared)
}

fn vm_shared_bytes(m: &Module) -> usize {
    keleusma::vm::shared_data_bytes_for(m)
}

/// Build `src` under the prefix the C host looks up and link it as a shared
/// object at `dir/name`.
fn shared_object(src: &str, dir: &Path, name: &str) -> PathBuf {
    let tm = machine();
    let ctx = Context::create();
    let program = common::build(src);
    let lib = linkage::build(&ctx, &program, &tm, "kel_app", &HostContracts::default())
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
    assert_eq!(lib.schema_hash, u64::from(program.schema_hash));
    let object = dir.join(format!("{name}.o"));
    lib.write_object(&tm, &object).unwrap();
    let out = dir.join(format!("lib{name}.dylib"));
    let flag = if cfg!(target_os = "macos") {
        "-dynamiclib"
    } else {
        "-shared"
    };
    let linked = Command::new("cc")
        .arg(flag)
        .arg(&object)
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        linked.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&linked.stderr)
    );
    out
}

/// The C host. Arguments are the first library, the second library, and the
/// expected outcome of the swap, `swap` or `refuse`.
const HOST: &str = r#"
#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct outcome { uint64_t status; int64_t value; };
typedef struct outcome (*start_fn)(int64_t, unsigned char *, unsigned char *, unsigned char *, unsigned char *);
typedef struct outcome (*resume_fn)(unsigned char *, int64_t);
typedef void (*release_fn)(unsigned char *);

struct lib {
    void *handle;
    start_fn start; resume_fn resume; release_fn release;
    uint64_t slot_bytes, private_bytes, region_bytes, shared_bytes, schema_hash, init_bytes;
    const unsigned char *init;
};

static void *need(void *h, const char *name) {
    void *p = dlsym(h, name);
    if (!p) { fprintf(stderr, "missing %s: %s\n", name, dlerror()); exit(3); }
    return p;
}

static struct lib open_lib(const char *path) {
    struct lib l;
    l.handle = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (!l.handle) { fprintf(stderr, "dlopen %s: %s\n", path, dlerror()); exit(3); }
    l.start = (start_fn)need(l.handle, "kel_app_start");
    l.resume = (resume_fn)need(l.handle, "kel_app_resume");
    l.release = (release_fn)need(l.handle, "kel_app_release");
    l.slot_bytes = *(const uint64_t *)need(l.handle, "kel_app_slot_bytes");
    l.private_bytes = *(const uint64_t *)need(l.handle, "kel_app_private_bytes");
    l.region_bytes = *(const uint64_t *)need(l.handle, "kel_app_region_bytes");
    l.shared_bytes = *(const uint64_t *)need(l.handle, "kel_app_shared_bytes");
    l.schema_hash = *(const uint64_t *)need(l.handle, "kel_app_schema_hash");
    l.init_bytes = *(const uint64_t *)need(l.handle, "kel_app_private_init_bytes");
    l.init = l.init_bytes ? (const unsigned char *)need(l.handle, "kel_app_private_init") : 0;
    return l;
}

/* One instance's storage, every buffer sized from its library. */
struct inst { uint64_t *slot, *priv, *region; };

static struct inst new_inst(const struct lib *l) {
    struct inst i;
    i.slot = calloc(l->slot_bytes / 8 + 1, 8);
    i.priv = calloc(l->private_bytes / 8 + 1, 8);
    i.region = calloc(l->region_bytes / 8 + 1, 8);
    if (l->init_bytes) memcpy(i.priv, l->init, l->init_bytes);
    return i;
}

static void run(const struct lib *l, struct inst *i, unsigned char *shared,
                int64_t first, const int64_t *replies, int n) {
    struct outcome o = l->start(first, shared, (unsigned char *)i->priv,
                                (unsigned char *)i->region, (unsigned char *)i->slot);
    for (int k = 0;; k++) {
        if (o.status != 1) { fprintf(stderr, "status %llu\n", (unsigned long long)o.status); exit(4); }
        printf("Y %lld\n", (long long)o.value);
        if (k == n) break;
        o = l->resume((unsigned char *)i->slot, replies[k]);
    }
}

int main(int argc, char **argv) {
    if (argc != 4) return 2;
    static const int64_t before[] = { BEFORE };
    static const int64_t after[] = { AFTER };
    struct lib old = open_lib(argv[1]);
    unsigned char *shared = calloc(old.shared_bytes + 8, 1);
    struct inst a = new_inst(&old);
    run(&old, &a, shared, FIRST, before, sizeof before / sizeof *before);

    struct lib next = open_lib(argv[2]);
    int compatible = next.schema_hash == old.schema_hash && next.shared_bytes == old.shared_bytes;
    if (!compatible) {
        printf("REFUSED %llx %llx\n", (unsigned long long)old.schema_hash,
               (unsigned long long)next.schema_hash);
        return strcmp(argv[3], "refuse") == 0 ? 0 : 5;
    }
    if (strcmp(argv[3], "swap") != 0) return 6;

    /* No instance of the old library may outlive it. Release, confirm that
       the slot is inactive, and only then unload. */
    old.release((unsigned char *)a.slot);
    struct outcome gone = old.resume((unsigned char *)a.slot, 0);
    if (gone.status != 0 || gone.value != 0) return 7;
    if (dlclose(old.handle) != 0) { fprintf(stderr, "dlclose: %s\n", dlerror()); return 8; }
    free(a.slot); free(a.priv); free(a.region);

    struct inst b = new_inst(&next);
    run(&next, &b, shared, RESTART, after, sizeof after / sizeof *after);
    next.release((unsigned char *)b.slot);
    printf("S");
    for (uint64_t k = 0; k < next.shared_bytes; k++) printf(" %u", shared[k]);
    printf("\n");
    return 0;
}
"#;

fn list(xs: &[i64]) -> String {
    xs.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")
}

fn host(dir: &Path) -> PathBuf {
    let src = dir.join("host.c");
    std::fs::write(&src, HOST).unwrap();
    let exe = dir.join("host");
    let built = Command::new("cc")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(format!("-DFIRST={FIRST}"))
        .arg(format!("-DRESTART={RESTART}"))
        .arg(format!("-DBEFORE={}", list(BEFORE)))
        .arg(format!("-DAFTER={}", list(AFTER)))
        .arg(&src)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    exe
}

fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../tmp")
        .join(format!("native-hot-swap-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_loaded_library_is_replaced_and_agrees_with_the_vm_swap() {
    let dir = scratch("swap");
    let (v1, v2) = (shared_object(V1, &dir, "v1"), shared_object(V2, &dir, "v2"));
    let exe = host(&dir);
    let out = Command::new(&exe)
        .arg(&v1)
        .arg(&v2)
        .arg("swap")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8(out.stdout).unwrap();
    let yields: Vec<i64> = text
        .lines()
        .filter_map(|l| l.strip_prefix("Y "))
        .map(|v| v.parse().unwrap())
        .collect();
    let shared: Vec<u8> = text
        .lines()
        .find_map(|l| l.strip_prefix("S"))
        .expect("the host dumps the shared buffer")
        .split_whitespace()
        .map(|b| b.parse().unwrap())
        .collect();
    let (vm_yields, vm_shared) = reference(V2);
    assert_eq!(yields, vm_yields, "yields across the swap");
    assert_eq!(shared, vm_shared, "the shared buffer the host kept");
    // MUST-NOT-FIRE: the second version must change what is yielded, or the
    // test could not tell a swap from a run of the first library throughout.
    let split = BEFORE.len() + 1;
    let (same, _) = reference(V1);
    assert_eq!(
        vm_yields[..split],
        same[..split],
        "the runs agree before the swap"
    );
    assert_ne!(
        vm_yields[split..],
        same[split..],
        "the replacement must behave differently from the original"
    );
    println!("  swapped run: {yields:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_replacement_with_another_schema_is_refused_by_both_shapes() {
    let dir = scratch("refuse");
    let (v1, v3) = (shared_object(V1, &dir, "v1"), shared_object(V3, &dir, "v3"));
    let exe = host(&dir);
    let out = Command::new(&exe)
        .arg(&v1)
        .arg(&v3)
        .arg("refuse")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("REFUSED"),
        "{out:?}"
    );
    let (m1, m3) = (common::build(V1), common::build(V3));
    assert_ne!(m1.schema_hash, m3.schema_hash);
    let mut arena = keleusma_arena::Arena::with_capacity(1 << 20);
    arena
        .resize_persistent(required_persistent_capacity_for(&m1))
        .unwrap();
    let mut vm = Vm::new(m1, &arena).unwrap();
    let error = vm.replace_module(m3, vec![Value::Int(0)]).unwrap_err();
    assert!(
        format!("{error:?}").contains("schema mismatch"),
        "{error:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Operator question 8, the native half. A slot's type alone changes the
/// schema, so a replacement that keeps every name and size is refused by the
/// native host and by the virtual machine alike. The shared sizes are equal, so
/// only the hash can refuse it.
#[test]
fn a_replacement_changing_only_a_slot_type_is_refused_by_both_shapes() {
    let (m1, m4) = (common::build(V1), common::build(V4));
    assert_eq!(
        keleusma::vm::shared_data_bytes_for(&m1),
        keleusma::vm::shared_data_bytes_for(&m4),
        "the layouts must differ in a type only, or the size would refuse it"
    );
    // CONTROL: the untyped hash, names and visibility only, cannot tell the
    // two apart, so the refusals below depend on the typed hash alone.
    assert_eq!(
        keleusma::bytecode::compute_schema_hash(m1.data_layout.as_ref()),
        keleusma::bytecode::compute_schema_hash(m4.data_layout.as_ref()),
        "the two layouts differ in more than a slot's type"
    );
    assert_ne!(
        m1.schema_hash, m4.schema_hash,
        "the schema hash no longer covers a slot's declared type"
    );
    let dir = scratch("refuse-type");
    let (v1, v4) = (shared_object(V1, &dir, "v1"), shared_object(V4, &dir, "v4"));
    let exe = host(&dir);
    let out = Command::new(&exe)
        .arg(&v1)
        .arg(&v4)
        .arg("refuse")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("REFUSED"),
        "the native host accepted a replacement whose slot type changed: {out:?}"
    );
    let mut arena = keleusma_arena::Arena::with_capacity(1 << 20);
    arena
        .resize_persistent(required_persistent_capacity_for(&m1))
        .unwrap();
    let mut vm = Vm::new(m1, &arena).unwrap();
    let error = vm.replace_module(m4, vec![Value::Int(0)]).unwrap_err();
    assert!(
        format!("{error:?}").contains("schema mismatch"),
        "{error:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
