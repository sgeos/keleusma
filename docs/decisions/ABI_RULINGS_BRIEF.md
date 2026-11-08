# BRIEF — record the operator's ABI rulings before they decay

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Why now

The operator has ruled on the ABI questions — **the first substantive input in roughly twelve
increments**, and it arrived because a page I wrote turned out to omit one of the items. Input that is
not written into the decision documents decays into transcript, and the next session starts from the
documents.

## What is settled, and what is not

| item | ruling | status |
|---|---|---|
| **float entry ABI** | **Option A** — a real FP ABI | **settled**, with one flagged assumption |
| **`Float` shared slot** | falls out of the above | **settled** |
| **string ABI** | **Option B** — make the embeddings agree, revisit later | **settled as a decision**, not implementable here |
| **`Fixed`** | "each `N` a different type; the compiler bakes it in" | **ambiguous** — three readings |
| **`Text` slot** | operator supposed it settled by the string ruling | **NOT settled** — different construct |
| **`Opaque`** | "pass-through pointer to host-allocated data" | intent already met by the handle; a literal pointer conflicts with narrow-word builds |
| **`Unit`** | operator asked what it is; that is a question, not a ruling | **open** |

## The flagged assumptions, which must be recorded AS assumptions

- **Float width.** `Float` is `f32` or `f64` under `narrow-float-32`. "Double ABI" is incoherent in a
  build with no `f64`, so the only coherent reading is **the FP type matches the runtime's float
  width**. I stated this to the operator and proceed on it; it is recorded as an assumption, not as a
  ruling.
- **`Unit`.** My inference is a permanent refusal, since a zero-byte slot conveys nothing. **Inference,
  not measurement, and not the operator's words.**

## Wrong turns to avoid

- **Do not record an ambiguous ruling as settled.** `Fixed` has three readings and one of them (encode
  `N` per type at the ABI) contradicts the operator's own "without needing to store". Capture the
  input and the readings; do not pick one.
- **Do not silently convert my inference into their decision.** `Unit` and the float width are mine.
  Attribution matters more than usual here, because a later reader cannot tell them apart from tone.
- **Do not let the corrected supposition vanish.** The operator believed `Text` was covered. Recording
  only the corrected state loses the fact that a reasonable reader drew that conclusion, which is
  exactly the confusion the record should prevent next time.
- **Do not start implementing floats from the ruling alone.** Measure what it unblocks and what it
  touches first. This line has been wrong about where cost sits three times in the last few
  increments, each time from acting on a plausible guess.
- **Do not treat the string ruling as actionable here.** Option B changes marshalling in `src/`, owned
  by the `v0.2.3` line. Recording it is the whole of what this line can do.

## What good looks like

The decision documents state the rulings, their attributions, the assumptions I am proceeding on, and
what remains open — such that a session starting cold from the documents would ask the operator the
same three questions I would ask, and no more.
