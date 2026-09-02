# Brief — a roadmap caveat that expired, and the file that answered it says it does not run

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Line**: V0.3.X, `native_codegen`. **Drafted 2026-09-02.**

---

## The goal set

| goal | state |
|---|---|
| **G13** correct two expired status claims about Workstream B | **unblocked, and the subject of this brief** |
| `f16`, `Text<N>`, `Opaque`, publication | not mine, or no oracle |
| absorption 46 | nothing unabsorbed |

**The unblocked capability queue is empty** and I am not going to invent an entry for it. What is
available is closing a gap between what the tree knows and what its own documents say.

## The finding

`V0_3_X_ROADMAP.md` describes **Workstream B, sub-coroutine lowering, as "where the risk
concentrates"**, and attaches an epistemic caveat dated 2026-08-10:

> *"**Sufficiency is NOT measured**: `lower_module` refuses on the first unsupported opcode and
> `Op::Stream` is the first op of every stream chunk, so **nothing behind it has been examined** and
> composites may sit there."*

**It has been measured since, and the roadmap does not say so.** `spike_stream_sufficiency.rs` runs
in the package suite — 95 seconds of it — and reports:

```text
  stages freed by the stream work alone : 12
  stages needing more                   : 0
```

So handling `Stream` and `Reset` is **sufficient** for every stage module: no other unsupported
opcode sits behind the refusal. **The caveat is what a reader of the roadmap meets, and it is
steering a judgement about where risk concentrates using a fact that has expired.**

## The second claim, in the file that produced the answer

Its own header reads:

> *"RESEARCH SPIKE, not a regression test. **UNCOMPILED — see `README.md`.** Install as
> `native_codegen/tests/spike_stream_sufficiency.rs`."*

**It is installed at exactly that path, it is compiled, it runs on every suite invocation, and it
asserts.** Checked across the package: **it is the only file making that claim**, so this is a single
stale header rather than a convention being misread.

**Two claims, both true when written, both false now.** Same class as everything else recorded today:
a status that expired while the sentence carrying it did not.

## What to do, and what not to

**Correct both claims to what is measured.** The roadmap gets the answer, dated, with the instrument
named. The header stops describing itself as uncompiled and not-a-test when it is compiled and
asserting.

**Do NOT restate the workstream's risk assessment.** Sufficiency answers *"does one increment unblock
the stages"*. It does **not** answer whether those stages need coroutine intrinsics, which is a
different question the roadmap's source-level reading addresses separately. **Conflating the two
would be exactly the overclaim this correction exists to remove.**

**Do NOT delete the caveat.** It was correct when written and its expiry is the finding. Superseded
text stays visible, as with every other reversal recorded this session.

**Do NOT quietly promote the spike to a regression test without saying what that costs.** It is 95
seconds of a suite whose whole run is a few minutes, spent on something labelled as not a test. If it
stays, that cost is being paid deliberately; say so.

## The wrong turns

**1. Do not claim the roadmap is wrong.** It was right on 2026-08-10 and nothing has corrected it
since. The defect is that no one updated it, not that it was mistaken.

**2. Do not read "freed by stream alone" as "lowers".** It means no *other* unsupported opcode blocks
the module. `Stream` itself is still refused, deliberately, and the coverage residual depends on that.

**3. Do not edit test sources while a suite runs.**

**4. Report the spike's figure with its population.** Twelve stages, from the four corpus roots that
file walks — which is not the same population as the 69-module figure used elsewhere.
