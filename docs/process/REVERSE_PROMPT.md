# Reverse Prompt

> **Navigation**: [Process](./README.md) | [Documentation Root](../README.md)

AI to Human communication channel. This is the **bounded latest-state handoff**,
overwritten each session per [COMMUNICATION.md](./COMMUNICATION.md). The append-only
increment-by-increment reasoning lives in [DESIGN_JOURNAL.md](./DESIGN_JOURNAL.md).

---

## Last Updated

**Date**: 2026-08-31 (session 59) — `Text<N>` authorized and designed, four relayed rulings
confirmed, and one refactor committed but NOT verified

## FIRST: THERE IS UNPUSHED, UNVERIFIED WORK

**`feat/opaque-address-width` is committed, unpushed, and lives in a worktree** at
`../keleusma-worktrees/opaque-address-width`. One commit, `32d058b8`. It compiles — library, derive
macro, every test target — but **its workspace test run was never completed, so it is not verified**
and the commit message says so.

Finishing that run is the highest-value first action. Session 57 stranded a branch and session 58
had to find it; this is the same shape, recorded on purpose rather than left to be discovered.

`feat/text-capacity-design` is pushed (`5194047d`, six commits) and has no pull request yet.

## A LIVE DEFECT THAT NEEDS YOUR DECISION, REPORTED BY THE OTHER LINE AND VERIFIED HERE

**Under `narrow-float-32` the module declares a four-byte float and the bundled virtual machine
computes in `f64`.** The `v0.3.0` line measured it; I verified the mechanism, which is unambiguous:

```rust
pub type Vm<'a, 'arena> = GenericVm<'a, 'arena, i64, u64, f64>;   // no #[cfg]
```

The alias is UNCONDITIONALLY `f64`, while `RUNTIME_FLOAT_BITS_LOG2` drops to 5 under the feature.
Their witness: `fn main(x: Float) -> Float { x * 2.0 + 1.0 }` at `x = 1e10` returns the f64 answer
exactly, and `Value::Float(1.0e10_f64)` compiles in that build, which it should not if the runtime
float were `f32`.

**MY "narrow-float-32 is green" CLAIM IS TRUE AND INSUFFICIENT.** The tests pass at 2610 / 0 and the
configuration is still incoherent underneath. I repaired tests that pinned a wide float; I did not
touch the arithmetic width, and nothing I did would have revealed this.

The feature's own documentation says its purpose is "rejecting f64 bytecode at the framing level",
so the arithmetic width was never wired to it. **Whether the bundled alias should become `f32` under
the feature is a semantic change to a public type and is YOURS**, not mine, and not theirs.

They are leaving their one failing backend test RED with the cause named rather than making it pass,
because the alternatives are to compute in `f64` against the float ruling or to pick values where
the widths agree, which is the cheating you ruled out. That is the same call I made on the stranded
exponential witness and I think it is right.

**Scope note from them, worth having**: `float_bits_log2` can already encode 16- and 8-bit floats.
If the arithmetic width does not track the declared width at 32 bits, it will not at 16 or 8, so
this is a precondition for that ladder rather than a separate item.

## WHAT YOU AUTHORIZED AND WHERE IT STANDS

**Dynamic `Text<N>`, first-party in session.** The design is SETTLED and recorded in
[`../decisions/TEXT_CAPACITY_TYPE.md`](../decisions/TEXT_CAPACITY_TYPE.md). Nothing is implemented;
the ground was cleared.

Static text is a `.rodata` pointer with no capacity in its type. Dynamic text is `Text<N>`, a flat
composite carrying no handle. A literal is STATIC and contributes its known length, so `"ab" + "cd"`
is `Text<4>` because a concatenation result cannot live in `.rodata`. `N` counts content bytes with
no terminator. A statically-too-narrow assignment is a compile error; runtime overflow truncates by
default with an optional arm, following the existing `CheckedArmKind` shape. Locals are ordinary
ephemeral arena values, just larger.

**Your `limit`-loop analogy is the governing one** and survived every case: a runtime length under a
static cap, space instead of iterations.

**Four relayed rulings confirmed and binding**: static `Text` as one pointer, `Opaque` by
`addr_bits_log2` with its trust boundary recorded, `narrow-float-32` going green, `confine` becoming
load-bearing.

## DONE

- **`narrow-float-32` is GREEN**: 2605 passed / 5 failed → **2610 / 0**, committed. Four tests pinned
  the float incidentally and now derive it from `RUNTIME_FLOAT_BITS_LOG2`; the fifth has the float AS
  its subject, so deriving would have left nothing wider to reject and it keeps a fixed 6.
- **`Text + Text` is REFUSED at compile time.** It used to compile, pass the verifier and always
  fault — a program the verifier admits and cannot run, which is what `verify()` exists to exclude.
- **`Opaque` sizes by the address width** (committed, unverified — see above).

## WHAT I GOT WRONG, RECORDED BECAUSE THE CORRECTIONS ARE LOAD-BEARING

- **I designed `Text<N>` with an arena handle.** The `v0.3.0` line's R2 corrected it to a flat
  composite, and their reason is better than mine: a handle implies unbounded lifetime, which is why
  it needs an epoch, which is what puts worst-case memory beyond static reach.
- **I said the cross-yield prohibition dissolves because nothing would be arena-resident.** You
  corrected me: an epoch-tagged value already crosses safely, so residence was never the barrier.
  The barrier is that the ephemeral region is write-once.
- **I nearly sent the peer a wrong correction.** I claimed their `narrow-float-32` diagnosis had two
  causes; `Target::wasm32()` declares a 64-bit float, so it fails the float check while asserting
  about the word width. Their report was right.
- **I published a wrong test figure** in #329's commit message and body — "179 binaries and 2904
  tests" for a pass that is 113 and 2708. An ad-hoc total across a multi-pass gate log.
  `scripts/gate-summary.sh` now exists so nobody writes that one-liner again.

## THE QUEUE

1. **Verify and push `feat/opaque-address-width`.** Unfinished, unverified, unpushed.
2. **Open a pull request for `feat/text-capacity-design`.** Pushed, gated locally, no PR.
3. **Implement `Text<N>`** per the settled design. The open question is the write-once ephemeral
   region against a mutable buffer; the peer flagged it and it is ours.
4. The earlier queue is unchanged and documented: the discard-arm reachability census
   (`DISCARD_ARM_REACHABILITY_BRIEF.md`), and `DATA_INIT` for the one stage that does not elide.

## FOR YOU

The `v0.3.0` line's coordination was better than mine throughout. They flagged their relay AS a
relay, told me to confirm before recording, caught a real error in my design, verified my citations
before conceding one of theirs, and **corrected a warning they had given me in the safe direction**
once they measured that a wrong confinement verdict costs a missed refusal rather than a
miscompile. Worth knowing when you weigh what each line reports.

One decision I deliberately did not make: having discovered that adding ONE width cascades through
roughly sixty sites and eleven public signatures, bundling the three widths into a single value
would make the next addition free. **Now is the cheapest moment that will exist to do it.** I did
not, because it exceeds what was authorized and mid-refactor redesign is how correctness is lost.
