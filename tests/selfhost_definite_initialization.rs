//! Differential controls for the self-hosted twin of the definite-initialization
//! rule, `keleusma::verify_init`, which `verify_depth.kel` decides in the same run
//! as the operand-stack depth check.
//!
//! # What is established
//!
//! The reference and the stage agree on each module below, on the verdict and on
//! the chunk and instruction of the first refused read. The modules are the
//! mutations of `tests/definite_initialization.rs`, two arms of an if that differ
//! only in which arm writes, and a corpus of mutations that each remove one local
//! write or redirect one local read. Each case counts what it reached, so that a
//! corpus that stops producing refusals, or stops producing admissions, fails
//! rather than passing on nothing. The whole-module validator reports a refusal
//! as the initialization stage.
//!
//! # What is not established
//!
//! Agreement on arbitrary bytecode. The stage computes the same least fixpoint as
//! the reference by a different iteration, which is the argument for agreement in
//! general, and these cases are evidence for it, not a proof.
#![cfg(feature = "self-host")]
use keleusma::bytecode::{Module, Op};
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
    let ip = error
        .message
        .split(" at ")
        .nth(1)
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

/// A local written on one arm of an if only is admitted, whichever arm it is.
/// The reference once checked each read as its worklist popped it, and so
/// refused the program in which the longer arm writes, since the shorter arm
/// reached the join first. Both orientations are pinned.
#[test]
fn a_write_on_either_arm_is_admitted_alike() {
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
        assert_eq!(agree(&module, src), None, "{src}");
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
