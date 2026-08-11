// ---------------------------------------------------------------------------
// PREPARED WHILE THE OTHER SESSION'S GATE HELD THE MACHINE. **UNCOMPILED.**
//
// The degenerate stream lowering: the admissibility predicate, and the tests
// that settle it. Install the predicate into `native_codegen/src/lib.rs` and the
// test module into `native_codegen/tests/yield_sequence.rs`, which already
// carries the observational-equivalence oracle this needs.
//
// WHY THE PREDICATE IS SEPARATED FROM THE EMISSION.
//
// The emission is mechanical: skip `Stream`, lower the body, turn `Yield` into
// `ret`, and drop the unreachable tail. The predicate is where soundness lives,
// because everything the emission omits is justified by a condition the predicate
// checked. Writing it as one function over `&Chunk`, with no LLVM types in its
// signature, makes it testable without a context and reviewable without reading
// the emitter.
//
// EVERY CONDITION HERE HAS A FAILURE CONSEQUENCE, and none is present for
// symmetry. The derivations are in `NATIVE_LOWERING_INVENTORY.md` under "THE
// DEGENERATE FORM NEEDS ONE ENTRY POINT, NOT TWO".
// ---------------------------------------------------------------------------

// ======================= install into src/lib.rs =======================

/// Is this chunk a **degenerate stream**, lowerable as a plain function?
///
/// A degenerate stream is `Stream ; <body> ; Yield ; PopN(1) ; Reset` with the
/// `Yield` at nesting depth zero and nothing else able to observe a suspension.
/// It lowers to a single entry point:
///
/// ```text
/// kel_chunk_N_step(resume: i64) -> i64
/// ```
///
/// # Why one entry point and not two
///
/// `Vm::resume_after_enter` writes the resume value into local slot 0 and then
/// pushes it as the suspended `Yield`'s result. The parameter reproduces the
/// first; the second is discarded by the `PopN(1)`, because the `yield` is the
/// body's tail expression. Iteration zero takes its value from the `call`
/// argument and iteration k from the k-th `resume`, so `step(a)`, `step(r1)`,
/// `step(r2)` reproduces the whole sequence with **no distinguished first call**.
///
/// # Why this needs the module and not just the chunk
///
/// **A DELEGATED suspension is invisible in the chunk's own ops**, and admitting
/// one silently produces a wrong resume value. `Vm::resume_after_enter` writes
/// slot 0 of `self.frames.first()`, the ENTRY chunk, whenever that entry is a
/// `Stream` — *regardless of which frame actually suspended*. So when a nested
/// `yield fn` callee suspends and the host resumes, the VM updates the stream
/// chunk's `resume` parameter as well as handing the value to the callee.
///
/// Natively the callee's suspension goes through the `kel_yield` callback, whose
/// return value reaches the callee's operand stack and **nothing else**. The
/// stream chunk's `resume` parameter is never updated, so the next iteration
/// reads a stale value. The chunk's own op vector shows no sign of this, which is
/// why a chunk-local predicate cannot see it.
///
/// # Why NO call-graph walk is needed, and no `compute_always_yielding`
///
/// The type checker's `category_can_call` enforces `Fn => matches!(callee, Fn)`:
/// **an atomic total may call only atomic totals.** Its own comment gives the
/// reason — it keeps a `fn` from transitively yielding through a `yield` callee,
/// which the virtual machine would propagate as a suspension.
///
/// So the transitive closure of a `Func` chunk contains only `Func` chunks, and a
/// stream chunk can delegate a suspension **if and only if it directly calls a
/// non-`Func` chunk**. One direct check over the call sites is not a conservative
/// approximation of the walk; it is exact.
///
/// That also removes a dependency that does not work. `compute_always_yielding`
/// lives behind `#[cfg(feature = "verify")]`, and `native_codegen` depends on
/// `keleusma` with `features = ["compile"]` only, so it is **not reachable** as
/// the package is configured. It is additionally `#[doc(hidden)]` and documented
/// as outside the stable API. An earlier draft of this predicate called it.
///
/// The condition is stated positively — every callee must be `Func` — because
/// that is the property actually required, namely that no callee can suspend. A
/// `Stream` callee is refused by the same clause without needing its own case.
///
/// # Returns
///
/// The index of the `Op::Yield` that becomes the return, or `None` with the
/// chunk left for the general Workstream B case.
fn degenerate_stream_yield(chunk: &Chunk, module: &Module) -> Option<usize> {
    if chunk.block_type != BlockType::Stream {
        return None;
    }
    let ops = &chunk.ops;

    // No delegated suspension. See the section above; this is the condition that
    // is invisible in the chunk and wrong in a way no local reading reveals.
    //
    // EVERY callee must be `Func`, which by `category_can_call` means it cannot
    // suspend, directly or transitively. An unresolvable index is refused rather
    // than skipped: a `None` from `chunks.get` means the module disagrees with
    // the op stream, and admitting on missing evidence is the wrong default.
    for op in ops {
        if let Op::Call(idx, _) = op
            && module.chunks.get(*idx as usize).map(|c| c.block_type) != Some(BlockType::Func)
        {
            return None;
        }
    }

    // The prologue must be EMPTY. `Reset` rewinds to just after `Stream`, so any
    // op before `Stream` runs exactly once in the VM but on every call in the
    // native form. This is the condition most likely to be read as tidiness; it
    // is the one that silently changes behaviour.
    if !matches!(ops.first(), Some(Op::Stream)) {
        return None;
    }

    // `Reset` must be last. A tail after it is unreachable in the VM, which
    // rewinds rather than falling through, and would be reachable natively.
    if !matches!(ops.last(), Some(Op::Reset)) {
        return None;
    }

    // Slot 0 is the resume parameter. A second parameter has no native source,
    // because `resume` supplies exactly one value.
    if chunk.param_count > 1 {
        return None;
    }

    // Exactly one `Yield`, at nesting depth zero.
    //
    // The block-opening set is `If` and `Loop`. `Else` opens nothing, being a
    // jump inside an already-open `If`, and `Break`/`BreakIf` transfer control
    // without nesting. That set was checked against the full block-structured
    // opcode list rather than assumed: a missed opener would report a nested
    // yield as top level and admit a chunk this transformation is wrong for.
    //
    // A MULTIHEADED stream chunk wraps its dispatch in `Loop`/`EndLoop`, so its
    // yields are nested by construction and it can never reach the tail check
    // below. That is a consequence of this rule rather than a separate one.
    let mut depth: i32 = 0;
    let mut found: Option<usize> = None;
    for (ip, op) in ops.iter().enumerate() {
        match op {
            Op::If(_) | Op::Loop(_) => depth += 1,
            Op::EndIf | Op::EndLoop(_) => depth -= 1,
            Op::Yield => {
                if depth != 0 || found.is_some() {
                    return None;
                }
                found = Some(ip);
            }
            _ => {}
        }
    }
    let y = found?;

    // Between the `Yield` and the `Reset` there must be exactly `PopN(1)`.
    //
    // This is what proves the resumed value is DISCARDED. Anything else consumes
    // it, and `step` does not supply it: the parameter carries the resume value
    // for the next iteration, not the result of the suspended `yield`. A body
    // written `let x = yield v; ...` fails here, which is correct and is the
    // shape `a_divergent_loop_function_is_refused` already pins.
    let tail = &ops[y + 1..ops.len() - 1];
    if !matches!(tail, [Op::PopN(1)]) {
        return None;
    }

    Some(y)
}

// ==================== install into tests/yield_sequence.rs ====================

/// Observational equivalence for the degenerate form, which is the ONLY thing
/// that settles it.
///
/// `assert_sequences_agree` compares the whole yielded sequence and the final
/// result between the VM and native code, so a transformation that produced the
/// right values in the wrong order fails here and nowhere else. The inventory has
/// carried "equivalence is unproven" as the load-bearing gap since the rotation
/// was first written; these cases are what close it, or fail to.
///
/// The replies differ from each other and from the arguments on purpose. Equal
/// values would let a form that returns the argument instead of the resumed value
/// pass, which is exactly the confusion the two delivery paths invite.
#[test]
fn the_degenerate_stream_agrees_in_sequence_and_result() {
    // The bare shape: the yield IS the body.
    assert_sequences_agree(
        "loop main(a: Word) -> Word { yield a }",
        &[10],
        &[21, 32, 43],
    );
    // A body that computes before yielding, so the yielded value is not simply
    // the parameter and a form that confuses the two is visible.
    assert_sequences_agree(
        "loop main(a: Word) -> Word { yield a * 2 + 1 }",
        &[10],
        &[21, 32, 43],
    );
    // A branch before the yield, so the body is not straight-line and the
    // depth-zero rule is exercised against a chunk that really has an `If`.
    assert_sequences_agree(
        "loop main(a: Word) -> Word { yield if a > 20 { a - 20 } else { a } }",
        &[10],
        &[21, 32, 43],
    );
    // A call, since eight of the ten self-hosted stages are `yield run()` and a
    // call is the shape that actually ships.
    assert_sequences_agree(
        "fn double(x: Word) -> Word { x * 2 }\n\
         loop main(a: Word) -> Word { yield double(a) }",
        &[10],
        &[21, 32, 43],
    );
}

/// MUST-NOT-FIRE for the predicate: shapes it has to REFUSE.
///
/// A predicate verified only in the admitting direction is the vacuous-control
/// failure this project keeps catching. Each case below is refused for a
/// different one of the six conditions, so a predicate that lost any single
/// condition still fails this test.
///
/// Refusal is observed through `lower_module`, not by calling the predicate
/// directly, because that is the boundary a consumer meets. A predicate that
/// returns `None` while the emitter lowers the chunk anyway would pass a direct
/// test and ship a wrong module.
#[test]
fn shapes_outside_the_degenerate_class_are_still_refused() {
    // The resumed value is CONSUMED, so the tail is not `[PopN(1)]`. This is the
    // case `a_divergent_loop_function_is_refused` already pins; asserted here
    // too because it is the condition most likely to be relaxed by someone who
    // reads `PopN(1)` as bookkeeping.
    assert_refused("loop main(a: Word) -> Word { let x = yield a; x }");

    // TWO top-level yields: a real partition, which the degenerate form does not
    // have. This is the multi-segment case that still needs the rotation.
    assert_refused("loop main(a: Word) -> Word { yield a; yield a + 1 }");

    // A NESTED yield. `lexer.kel` is this shape, and it is the general case.
    assert_refused("loop main(a: Word) -> Word { if a > 0 { yield a } else { yield 0 } }");

    // A DELEGATED suspension, which is the case no chunk-local reading catches.
    //
    // The op vector of `main` here looks degenerate: one top-level `Yield`, tail
    // exactly `PopN(1)`, `Stream` first and `Reset` last. It is NOT degenerate,
    // because `helper` suspends too, and on that suspension the VM overwrites
    // `main`'s resume parameter while native code does not.
    //
    // If this case is ever DROPPED because it looks redundant next to the others,
    // the predicate silently starts miscompiling a shape the corpus contains:
    // `codegen.kel` delegates its entire body this way.
    assert_refused(
        "yield helper(x: Word) -> Word { let r = yield x; r }\n\
         loop main(a: Word) -> Word { yield helper(a) }",
    );
}

/// Lower `src` and assert the module is REFUSED.
///
/// Deliberately does not match on the refusal text. The reason a chunk is
/// outside the degenerate class is not a stable interface, and asserting on it
/// would make this test fail when a message improves rather than when behaviour
/// regresses.
fn assert_refused(src: &str) {
    let m = compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile");
    let ctx = Context::create();
    let lm = ctx.create_module("kel");
    assert!(
        lower_module(&ctx, &lm, &m, LowerOptions::default()).is_err(),
        "this shape is outside the degenerate class and must be refused, not \
         lowered as though the resumed value were discarded:\n{src}"
    );
}

// ---------------------------------------------------------------------------
// KNOWN RISKS, written down rather than discovered.
//
//  1. RESOLVED by reading rather than left as a hedge. `Op` derives
//     `Debug, Clone, Copy, PartialEq` and `PopN` is `PopN(u8)`, so the slice
//     pattern `[Op::PopN(1)]` is valid and `1` is a literal `u8` pattern.
//  2. `ops[y + 1..ops.len() - 1]` assumes `Reset` is the last op, which the
//     check above guarantees, and that `y + 1 <= ops.len() - 1`, which holds
//     because `Yield` cannot BE the last op when `Reset` is.
//  3. RESOLVED. `assert_sequences_agree(src: &str, args: &[i64], replies: &[i64])`,
//     read from its definition and three call sites rather than assumed.
//     `lower_module`, `LowerOptions` and `Context` are already imported in
//     `yield_sequence.rs`, so `assert_refused` adds no imports.
//  4. The emission is NOT in this file. Nothing here lowers anything, so
//     installing the predicate alone changes no behaviour and the tests above
//     will fail until the emitter consults it. Install both or neither.
//  5. RESOLVED, and the earlier draft was BROKEN. It called
//     `keleusma::verify::compute_always_yielding`, which sits behind
//     `#[cfg(feature = "verify")]` while this package depends on `keleusma` with
//     `features = ["compile"]` only. It would not have compiled. The replacement
//     needs no feature and no walk: `category_can_call` enforces that an atomic
//     total may call only atomic totals, so requiring every callee to be `Func`
//     is exact rather than conservative.
//  6. RESOLVED by reading the declaration. `Op::Call(u16, u8)` is documented as
//     "Call compiled function by chunk index with N arguments", and the shipped
//     lowering resolves it as `callees.get(*idx as usize)`.
//  7. The degenerate emission must keep `Op::Yield`'s TWO lowerings apart. In a
//     `Reentrant` chunk it stays the `kel_yield` callback; in a degenerate
//     `Stream` chunk it becomes `ret`. The op loop therefore needs the mode in
//     scope, and a single shared `Op::Yield` arm would silently pick one.
//
// AND ONE EDIT THAT IS NOT OPTIONAL, easy to forget because it is in a passing
// test. `a_divergent_loop_function_is_refused` carries a comment reading
// "`Stream` and `Reset` are refused DELIBERATELY, not by omission." **That stops
// being true when this lands** — they will be refused only OUTSIDE the degenerate
// class. The test itself stays green, because its shape consumes the resumed
// value and is still refused, so nothing forces the comment to be updated. This
// branch has reported the same defect class to the `v0.2.3` session three times:
// a comment that outlives the code it describes. Update it in the same change.
// ---------------------------------------------------------------------------
