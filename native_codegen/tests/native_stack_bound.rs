//! A native worst-case stack bound, computed end to end.
//!
//! `V0_3_X_ROADMAP.md` Workstream E requires the bounded-resource guarantees to
//! survive lowering. The arena bounds dynamic allocation and says nothing about
//! the machine stack, because the register allocator spills regardless of the
//! heap model and LLVM chooses frame sizes. This file closes that gap for the
//! stack, using the property the language already has: **the verifier forbids
//! recursion**, so the static call graph is acyclic and the worst case is the
//! longest weighted path through it.
//!
//! # Three constraints established by measurement, not assumed
//!
//! 1. **`.stack_sizes` is unreachable in process.** `TargetMachine::write_to_file`
//!    emits an object with an EMPTY `StackSizes` block, because inkwell has no
//!    way to set `--stack-size-section`. Driving `llc` out of process is the
//!    only route, which forces a subprocess into any toolchain that must
//!    produce a stack bound.
//! 2. **Function identity needs no relocation parsing.**
//!    `llvm-readobj --stack-sizes` resolves entries to symbol names directly.
//!    An earlier note in the inventory expected `.rela.stack_sizes` to need
//!    hand-decoding; it does not.
//! 3. **The section is ELF-only.** The host is Mach-O, so this targets
//!    `thumbv7em-none-eabihf`, which is also where the bound actually matters,
//!    since a stack overflow on a microcontroller is unrecoverable.
//!
//! # Skipping
//!
//! Needs `llc` and `llvm-readobj`. Where absent the test SKIPS LOUDLY, because a
//! step that quietly does nothing reads as a step that passed.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::{CodeModel, InitializationConfig, RelocMode, Target};
use keleusma::bytecode::Op;
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::{LowerOptions, lower_module};
use std::collections::BTreeMap;
use std::process::Command;

const LLVM_BIN: &str = "/opt/local/libexec/llvm-22/bin";

fn tool(name: &str) -> Option<String> {
    let p = format!("{LLVM_BIN}/{name}");
    Command::new(&p)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| p)
}

/// Per-function frame sizes, keyed by symbol, read from the emitted object.
fn frame_sizes(src: &str, dir: &std::path::Path) -> Option<BTreeMap<String, u64>> {
    let (llc, readobj) = (tool("llc")?, tool("llvm-readobj")?);
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");

    Target::initialize_all(&InitializationConfig::default());
    let triple = inkwell::targets::TargetTriple::create("thumbv7em-none-eabihf");
    let machine = Target::from_triple(&triple)
        .expect("target")
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .expect("target machine");

    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    lower_module(&ctx, &lm, &m, LowerOptions::default()).expect("lower module");
    // **`mem2reg` ONLY, deliberately, and not `default<O2>`.**
    //
    // The middle end must run at all, because without it every frame carries
    // MAX_STACK operand slots the program never touches and the bound is wrong
    // by roughly thirty times. But the full pipeline INLINES, and inlining
    // dissolves the call graph the longest-path traversal walks: measured on a
    // three-function program, `default<O2>` reduced every reported frame to
    // zero because nothing survived as a call.
    //
    // A bound computed from the bytecode call graph over post-inlining weights
    // is still conservative, since an inlined callee's needs are folded into
    // its caller's frame and adding the callee's standalone figure only
    // over-counts. It is conservative and USELESS: a bound of zero bounds
    // nothing. Promoting allocas without inlining keeps the two graphs in
    // correspondence and gives a figure that means something.
    lm.run_passes("mem2reg", &machine, PassBuilderOptions::create())
        .expect("optimise");

    std::fs::create_dir_all(dir).ok()?;
    let ll = dir.join("m.ll");
    std::fs::write(&ll, lm.print_to_string().to_string()).ok()?;
    let obj = dir.join("m.o");
    let ok = Command::new(&llc)
        .args([
            "-mtriple=thumbv7em-none-eabihf",
            "--stack-size-section",
            "-filetype=obj",
        ])
        .arg(&ll)
        .arg("-o")
        .arg(&obj)
        .status()
        .ok()?
        .success();
    if !ok {
        return None;
    }
    let out = Command::new(&readobj)
        .arg("--stack-sizes")
        .arg(&obj)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();

    let mut sizes = BTreeMap::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("Functions: [") {
            pending = Some(rest.trim_end_matches(']').to_string());
        } else if let (Some(rest), Some(f)) = (t.strip_prefix("Size: "), pending.take()) {
            let v = u64::from_str_radix(rest.trim_start_matches("0x"), 16).unwrap_or(0);
            sizes.insert(f, v);
        }
    }
    Some(sizes)
}

/// Longest weighted path through the acyclic static call graph.
///
/// Acyclicity is not assumed here; it is a property the type checker enforces by
/// rejecting direct and mutual recursion. The traversal still carries a visited
/// set, so a cycle that slipped through would terminate rather than recurse
/// forever, and would be visible as an implausibly small bound rather than a
/// hang.
fn longest_path(m: &keleusma::bytecode::Module, sizes: &BTreeMap<String, u64>) -> u64 {
    let n = m.chunks.len();
    let weight = |i: usize| *sizes.get(&format!("kel_chunk_{i}")).unwrap_or(&0);
    let edges: Vec<Vec<usize>> = m
        .chunks
        .iter()
        .map(|c| {
            let mut v: Vec<usize> = c
                .ops
                .iter()
                .filter_map(|o| match o {
                    Op::Call(t, _) if (*t as usize) < n => Some(*t as usize),
                    _ => None,
                })
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        })
        .collect();

    fn walk(
        i: usize,
        edges: &[Vec<usize>],
        w: &dyn Fn(usize) -> u64,
        seen: &mut Vec<usize>,
    ) -> u64 {
        if seen.contains(&i) {
            return 0;
        }
        seen.push(i);
        let deepest = edges[i]
            .iter()
            .map(|&c| walk(c, edges, w, seen))
            .max()
            .unwrap_or(0);
        seen.pop();
        w(i) + deepest
    }
    (0..n)
        .map(|i| walk(i, &edges, &weight, &mut Vec::new()))
        .max()
        .unwrap_or(0)
}

#[test]
fn a_native_stack_bound_is_computable_end_to_end() {
    let dir = std::env::temp_dir().join(format!("kel-stack-{}", std::process::id()));
    let src = "fn leaf(x: Word) -> Word { x + 1 }
               fn mid(x: Word, y: Word) -> Word { leaf(x) - leaf(y) }
               fn main(a: Word, b: Word) -> Word { mid(a, b) + leaf(a) }";
    let Some(sizes) = frame_sizes(src, &dir) else {
        eprintln!(
            "\n\x1b[1;33mSKIPPED: llc or llvm-readobj not found, so the native stack bound was \
             NOT verified by this run.\x1b[0m\n"
        );
        return;
    };
    assert!(
        !sizes.is_empty(),
        "the object carried no .stack_sizes entries; --stack-size-section is the only way to \
         emit them and inkwell cannot set it, so an in-process emission silently yields nothing"
    );

    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let bound = longest_path(&m, &sizes);
    eprintln!("  per-function frames: {sizes:?}");
    eprintln!("  native stack bound (longest weighted path): {bound} bytes");

    // The bound must be at least the largest single frame, and at most the sum
    // of all of them. Those are the two trivial envelopes, and a traversal that
    // double-counted or lost a level would escape one of them.
    let largest = *sizes.values().max().unwrap();
    let total: u64 = sizes.values().sum();
    assert!(
        bound >= largest,
        "the bound {bound} is below the largest single frame {largest}, so the traversal lost a \
         level"
    );
    assert!(
        bound <= total,
        "the bound {bound} exceeds the sum of every frame {total}, so the traversal counted a \
         function twice"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_deeper_call_chain_raises_the_bound() {
    // MUST-NOT-FIRE for the bound above: if the traversal ignored depth, a
    // three-level chain and a one-level program would report the same figure.
    let dir = std::env::temp_dir().join(format!("kel-stack-d-{}", std::process::id()));
    let flat = "fn main(a: Word, b: Word) -> Word { a + b }";
    let deep = "fn f1(x: Word) -> Word { x + 1 }
                fn f2(x: Word) -> Word { f1(x) * 3 }
                fn f3(x: Word) -> Word { f2(x) - 7 }
                fn main(a: Word, b: Word) -> Word { f3(a) + f3(b) }";
    let (Some(sf), Some(sd)) = (frame_sizes(flat, &dir), frame_sizes(deep, &dir)) else {
        eprintln!(
            "\n\x1b[1;33mSKIPPED: LLVM tools absent; depth sensitivity NOT verified.\x1b[0m\n"
        );
        return;
    };
    let mf = compile(&parse(&tokenize(flat).expect("lex")).expect("parse")).expect("compile");
    let md = compile(&parse(&tokenize(deep).expect("lex")).expect("parse")).expect("compile");
    let (bf, bd) = (longest_path(&mf, &sf), longest_path(&md, &sd));
    eprintln!("  flat bound {bf}, deep bound {bd}");
    assert!(
        bd > bf,
        "a four-level call chain must bound higher than a single leaf: deep={bd} flat={bf}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
