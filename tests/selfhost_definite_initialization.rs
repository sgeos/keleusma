//! Differential controls for the self-hosted twin of the definite-initialization
//! rule, `keleusma::verify_init`, which `verify_depth.kel` decides in the same run
//! as the operand-stack depth check.
//!
//! # What is established
//!
//! The reference and the stage agree on each module below, on the verdict and on
//! the chunk and instruction of the first refused read. The modules are the
//! mutations of `tests/definite_initialization.rs`, two arms of an if that differ
//! only in which arm writes, the compiled handled indexed writes, and a corpus of mutations that each remove one local
//! write or redirect one local read. Each case counts what it reached, so that a
//! corpus that stops producing refusals, or stops producing admissions, fails
//! rather than passing on nothing. The whole-module validator reports a refusal
//! as the initialization stage.
//!
//! Rule 2, a private data slot whose load-time initializer is `Unit` read before
//! a write on some path, is compared on a `()` slot, on composite slots given
//! `Unit` as an older compiler would, on a write in one arm of an if, on a write
//! made by a callee, and on a corpus that removes one `SetData` or resets one
//! initializer to `Unit` at a time.
//!
//! # What is not established
//!
//! Agreement on arbitrary bytecode. The stage computes the same greatest fixpoint as
//! the reference by a different iteration, which is the argument for agreement in
//! general, and these cases are evidence for it, not a proof.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{ConstValue, Module, Op};
use keleusma::selfhost::{
    VerificationVerdict, compile_src, initialization_rejects_via_kel, verify_module_via_kel,
};
use keleusma::verify_init::verify_definite_initialization;

/// The reference's first refusal, as a chunk index and instruction, or `None`.
fn reference(module: &Module) -> Option<(usize, usize)> {
    let error = verify_definite_initialization(module).err()?;
    let chunk = module
        .chunks
        .iter()
        .position(|c| c.name == error.chunk_name)
        .expect("refused chunk is in the module");
    // A local refusal reads "GetLocal(s) at IP ...", a private data refusal
    // "instruction IP may read private data slot ...".
    let after = match error.message.strip_prefix("instruction ") {
        Some(rest) => Some(rest),
        None => error.message.split(" at ").nth(1),
    };
    let ip = after
        .and_then(|rest| rest.split(' ').next())
        .and_then(|n| n.parse().ok())
        .expect("refusal names its instruction");
    Some((chunk, ip))
}

/// The two verdicts agree, and the outcome is returned.
fn agree(module: &Module, what: &str) -> Option<(usize, usize)> {
    let expected = reference(module);
    let got = initialization_rejects_via_kel(module).unwrap_or_else(|e| panic!("{what}: {e}"));
    assert_eq!(got, expected, "{what}");
    expected
}

fn main_ops(module: &mut Module) -> &mut Vec<Op> {
    &mut module
        .chunks
        .iter_mut()
        .find(|c| c.name == "main")
        .expect("main")
        .ops
}

#[test]
fn compiled_programs_are_admitted_by_both() {
    for src in CORPUS {
        let module = compile_src(src);
        assert_eq!(agree(&module, src), None, "{src}");
    }
}

/// The mutations of `tests/definite_initialization.rs`, through the whole
/// validator as well as the rule alone.
#[test]
fn the_reference_mutations_are_refused_alike() {
    let mut never_written = compile_src("fn main(n: Word) -> Word { let a = n + 1; a }");
    assert_eq!(
        verify_module_via_kel(&never_written).unwrap(),
        VerificationVerdict::Accepted
    );
    let ops = main_ops(&mut never_written);
    let write = ops.iter().position(|op| *op == Op::SetLocal(1)).unwrap();
    ops[write] = Op::PopN(1);
    assert!(agree(&never_written, "never written").is_some());
    assert!(keleusma::verify::verify(&never_written).is_err());
    assert_eq!(
        verify_module_via_kel(&never_written).unwrap(),
        VerificationVerdict::Rejected {
            stage: "initialization",
            chunk: Some(0),
        }
    );

    // A stream local written only before `Stream`.
    let mut stream = compile_src("loop main(e: Word) -> Word { let n = yield e; yield n }");
    let ops = main_ops(&mut stream);
    let at = ops.iter().position(|op| *op == Op::Stream).unwrap();
    let read = ops
        .iter()
        .skip(at)
        .find_map(|op| match op {
            Op::GetLocal(i) if *i >= 1 => Some(*i),
            _ => None,
        })
        .unwrap();
    ops.insert(at, Op::SetLocal(read));
    ops.insert(at, Op::PushImmediate(3));
    for op in ops.iter_mut().skip(at + 3) {
        if *op == Op::SetLocal(read) {
            *op = Op::PopN(1);
        }
    }
    for op in ops.iter_mut() {
        match op {
            Op::If(t)
            | Op::Else(t)
            | Op::Loop(t)
            | Op::EndLoop(t)
            | Op::Break(t)
            | Op::BreakIf(t)
                if (*t as usize) >= at =>
            {
                *t += 2
            }
            _ => {}
        }
    }
    assert!(agree(&stream, "stream").is_some());
    assert!(matches!(
        verify_module_via_kel(&stream).unwrap(),
        VerificationVerdict::Rejected {
            stage: "initialization",
            ..
        }
    ));
}

/// A local written on one arm of an if only is refused, whichever arm it is,
/// and both name the same read. The strengthened rule is a must-analysis, so
/// the join intersects the arms. Both orientations are pinned, since the visit
/// order of the arms once decided a verdict.
#[test]
fn a_write_on_either_arm_is_refused_alike() {
    for src in [
        "fn main(n: Word) -> Word { let a = 7; let r = if n > 0 { let b = n * 2 + 3 * n - 1; b } else { 5 }; r + a }",
        "fn main(n: Word) -> Word { let a = 7; let r = if n > 0 { 5 } else { let b = n * 2 + 3 * n - 1; b }; r + a }",
        "fn main(n: Word) -> Word { let a = 7; let r = if n > 0 { let b = 5; b } else { n * 2 + 3 * n - 1 }; r + a }",
    ] {
        let mut module = compile_src(src);
        let ops = main_ops(&mut module);
        // Remove the write of local 1, then move the arm's local onto it, so
        // that local 1 is written on that arm only and read after the join.
        let write = ops.iter().position(|op| *op == Op::SetLocal(1)).unwrap();
        ops[write] = Op::PopN(1);
        for op in ops.iter_mut() {
            match op {
                Op::SetLocal(2) => *op = Op::SetLocal(1),
                Op::GetLocal(2) => *op = Op::GetLocal(1),
                _ => {}
            }
        }
        assert!(agree(&module, src).is_some(), "{src}");
    }
}

/// Programs whose mutations reach branches, loops with breaks, streams, calls
/// and parameters.
const CORPUS: &[&str] = &[
    "fn main(n: Word) -> Word { let a = n + 1; let b = if a > 2 { let c = a * 2; c } else { a }; b + a }",
    "fn main(n: Word) -> Word { let t = n + 3; for i in 0..4 { let u = i + t; if u > 5 { break; } } t }",
    "fn main(n: Word) -> Word { let t = n * 2; for i in 0..3 { let u = i + t; for j in 0..2 { let v = j + u; v } u } t }",
    "loop main(e: Word) -> Word { let n = yield e; let m = n + e; let k = yield m; yield k + m }",
    "fn f(x: Word) -> Word { let y = x * 2; let z = y + x; z }\nfn main(n: Word) -> Word { let z = f(n); let w = z + n; w }",
    "fn main(a: Word, b: Word) -> Word { let c = a + b; let d = if c > 0 { c } else { let e = a - b; e }; d * c }",
    "private data d { x: [Word; 2] } fn main(i: Word) -> Word { d.x[i] = 7 { ok(o, n) => o * 100 + n, invalid_index(j) => j } }",
    "private data d { x: [Byte; 2] } fn main(i: Word) -> Byte { d.x[i] = 7 as Byte { ok(o, _) => o, invalid_index(_) => 0 as Byte } }",
];

/// Each corpus program mutated by removing one local write, or by redirecting
/// one local read to another local, compared on every mutation.
#[test]
fn single_write_and_read_mutations_agree() {
    let (mut refused, mut admitted) = (0usize, 0usize);
    for src in CORPUS {
        let base = compile_src(src);
        let mut variants = Vec::new();
        for (c, chunk) in base.chunks.iter().enumerate() {
            for (ip, op) in chunk.ops.iter().enumerate() {
                match op {
                    Op::SetLocal(_) => {
                        let mut m = base.clone();
                        m.chunks[c].ops[ip] = Op::PopN(1);
                        variants.push(m);
                    }
                    Op::GetLocal(i) => {
                        for j in 0..chunk.local_count {
                            if j != *i {
                                let mut m = base.clone();
                                m.chunks[c].ops[ip] = Op::GetLocal(j);
                                variants.push(m);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        for (k, module) in variants.iter().enumerate() {
            match agree(module, &format!("{src} variant {k}")) {
                Some(_) => refused += 1,
                None => admitted += 1,
            }
        }
    }
    eprintln!("{refused} mutations refused and {admitted} admitted by both");
    // Both outcomes must be reached, or the corpus proves nothing about one.
    assert!(refused >= 20, "only {refused} refusals reached");
    assert!(admitted >= 20, "only {admitted} admissions reached");
}

/// Programs with private data whose mutations reach rule 2. Every one compiles
/// and is admitted by both as written. Every slot each reads is written first
/// on every path, by `SetData` except in the array program, whose elements are
/// written by indexed writes.
const DATA_CORPUS: &[&str] = &[
    // A `()` slot keeps `Unit`, its only value, so rule 2 tracks it as compiled.
    "private data d { u: (), n: Word }\nfn main(k: Word) -> Word { d.u = (); d.n = k; let v = d.u; d.n + k }",
    // A write on both arms of an if, and a read after the join.
    "struct P { x: Word, y: Word }\nprivate data d { p: P, n: Word }\n\
     fn main(k: Word) -> Word { if k > 0 { d.p = P { x: 1, y: 2 }; } else { d.p = P { x: 3, y: 4 }; } d.n = k; let q = d.p; q.x + d.n }",
    // A write made by a callee on every path, read by its caller.
    "struct P { x: Word, y: Word }\nprivate data d { p: P, n: Word }\n\
     fn init(k: Word) -> Word { d.p = P { x: k, y: 2 }; d.n = k; 0 }\n\
     fn main(k: Word) -> Word { let z = init(k); let q = d.p; q.x + z + d.n }",
    // A callee that writes on one path only, and a caller that writes itself.
    "struct P { x: Word, y: Word }\nprivate data d { p: P, n: Word }\n\
     fn maybe(k: Word) -> Word { if k > 0 { d.p = P { x: k, y: 2 }; } 0 }\n\
     fn main(k: Word) -> Word { let z = maybe(k); d.p = P { x: 1, y: 1 }; let q = d.p; q.y + z }",
    // An indexed read over an array of composites, written whole first.
    "struct P { x: Word, y: Word }\nprivate data d { a: [P; 2], n: Word }\n\
     fn main(i: Word) -> Word { d.a[0] = P { x: 1, y: 2 }; d.a[1] = P { x: 3, y: 4 }; d.n = i; let q = d.a[i]; q.x + d.n }",
    // A stream, whose writes persist across `Reset`.
    "private data d { u: (), n: Word }\nloop main(e: Word) -> Word { d.u = (); d.n = e; let v = d.u; let r = yield e + d.n; yield r }",
];

/// Every private initializer of the module set to `Unit`, as a module from an
/// older compiler carries it, returning how many were changed.
fn unit_initializers(module: &mut Module) -> usize {
    let Some(layout) = module.data_layout.as_mut() else {
        return 0;
    };
    let mut changed = 0;
    for value in &mut layout.private_init {
        if *value != ConstValue::Unit {
            *value = ConstValue::Unit;
            changed += 1;
        }
    }
    changed
}

/// The corpus as compiled is admitted by both. With every initializer `Unit`
/// both admit every program but the array one and refuse that one alike.
#[test]
fn private_data_controls_are_admitted_by_both() {
    for src in DATA_CORPUS {
        let mut module = compile_src(src);
        assert_eq!(agree(&module, src), None, "{src}");
        assert_eq!(
            verify_module_via_kel(&module).unwrap(),
            VerificationVerdict::Accepted,
            "{src}"
        );
        unit_initializers(&mut module);
        // An indexed write definitely writes no element, since its index is
        // known only at run time, so the array program's read is refused by both
        // once its elements start as `Unit`. Every other program writes each slot
        // it reads first.
        let indexed = src.contains("d.a[0] =");
        assert_eq!(
            agree(&module, src).is_some(),
            indexed,
            "{src} with Unit initializers"
        );
    }
}

/// The index of the first `SetData` in chunk `c`.
fn first_data_write(module: &Module, c: usize) -> usize {
    module.chunks[c]
        .ops
        .iter()
        .position(|op| matches!(op, Op::SetData(_)))
        .expect("a SetData")
}

fn chunk_named(module: &Module, name: &str) -> usize {
    module
        .chunks
        .iter()
        .position(|c| c.name == name)
        .expect("chunk")
}

/// The three shapes the rule exists for, each refused alike and reported by
/// the whole validator as the initialization stage.
#[test]
fn private_data_refusals_are_refused_alike() {
    // A `()` slot read with its write removed.
    let mut unit = compile_src(DATA_CORPUS[0]);
    let main = chunk_named(&unit, "main");
    let write = first_data_write(&unit, main);
    unit.chunks[main].ops[write] = Op::PopN(1);
    let refused = agree(&unit, "() slot never written").expect("refused");
    assert!(keleusma::verify::verify(&unit).is_err());
    assert_eq!(
        verify_module_via_kel(&unit).unwrap(),
        VerificationVerdict::Rejected {
            stage: "initialization",
            chunk: Some(refused.0),
        }
    );

    // A write in one arm only, from the program that writes on both arms with
    // either arm's write removed in turn.
    let base = compile_src(DATA_CORPUS[1]);
    let main = chunk_named(&base, "main");
    let writes: Vec<usize> = base.chunks[main]
        .ops
        .iter()
        .enumerate()
        .filter(|(_, op)| matches!(op, Op::SetData(_)))
        .map(|(ip, _)| ip)
        .collect();
    // One write in each arm, then the write of `d.n` after the join.
    assert_eq!(writes.len(), 3, "{writes:?}");
    for &ip in &writes[..2] {
        let mut module = base.clone();
        unit_initializers(&mut module);
        module.chunks[main].ops[ip] = Op::PopN(1);
        assert!(agree(&module, "one arm writes").is_some());
        assert!(matches!(
            verify_module_via_kel(&module).unwrap(),
            VerificationVerdict::Rejected {
                stage: "initialization",
                ..
            }
        ));
    }

    // A callee that writes on one path only does not protect its caller's read.
    let mut maybe = compile_src(DATA_CORPUS[3]);
    unit_initializers(&mut maybe);
    let main = chunk_named(&maybe, "main");
    let write = first_data_write(&maybe, main);
    maybe.chunks[main].ops[write] = Op::PopN(1);
    assert!(agree(&maybe, "callee writes on one path").is_some());

    // A callee that writes on every path does, and removing its write refuses
    // the caller's read.
    let mut init = compile_src(DATA_CORPUS[2]);
    unit_initializers(&mut init);
    assert_eq!(agree(&init, "callee writes"), None);
    let callee = chunk_named(&init, "init");
    let write = first_data_write(&init, callee);
    init.chunks[callee].ops[write] = Op::PopN(1);
    let refused = agree(&init, "callee write removed").expect("refused");
    assert_eq!(refused.0, chunk_named(&init, "main"), "the caller's read");
}

/// Each data corpus program with every initializer `Unit`, mutated by removing
/// one `SetData`, and as compiled with one initializer set to `Unit` and one
/// `SetData` removed, compared on every mutation.
#[test]
fn private_data_mutations_agree() {
    let (mut refused, mut admitted) = (0usize, 0usize);
    for src in DATA_CORPUS {
        let base = compile_src(src);
        let mut all_unit = base.clone();
        unit_initializers(&mut all_unit);
        let writes: Vec<(usize, usize)> = base
            .chunks
            .iter()
            .enumerate()
            .flat_map(|(c, chunk)| {
                chunk
                    .ops
                    .iter()
                    .enumerate()
                    .filter(|(_, op)| matches!(op, Op::SetData(_)))
                    .map(move |(ip, _)| (c, ip))
            })
            .collect();
        let mut variants = Vec::new();
        for &(c, ip) in &writes {
            let mut m = all_unit.clone();
            m.chunks[c].ops[ip] = Op::PopN(1);
            variants.push(m);
        }
        let inits = base
            .data_layout
            .as_ref()
            .map_or(0, |d| d.private_init.len());
        for k in 0..inits {
            for &(c, ip) in &writes {
                let mut m = base.clone();
                m.data_layout.as_mut().unwrap().private_init[k] = ConstValue::Unit;
                m.chunks[c].ops[ip] = Op::PopN(1);
                variants.push(m);
            }
        }
        for (k, module) in variants.iter().enumerate() {
            match agree(module, &format!("{src} variant {k}")) {
                Some(_) => refused += 1,
                None => admitted += 1,
            }
        }
    }
    eprintln!("{refused} private data mutations refused and {admitted} admitted by both");
    // Both outcomes must be reached, or the corpus proves nothing about one.
    assert!(refused >= 20, "only {refused} refusals reached");
    assert!(admitted >= 10, "only {admitted} admissions reached");
}
