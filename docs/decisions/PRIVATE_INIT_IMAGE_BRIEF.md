# BRIEF — the private data initial image

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-11, after the measurement.

## The defect, measured before it was described

```
private data log { count: Word = 7 }
fn main(t: Word) -> Word { if t < 0 { log.count = 99; } log.count + 1 }
```

| path | reference | this backend |
|---|---|---|
| the write does not happen | **8** — the declared initializer plus one | **1** |
| the write happens | 100 | 100 |

**A silently wrong value, and no fault this time.** `private_init` carries `Int(7)`; the runtime
applies it when the module is loaded. **This backend never applies it at all**, so a host handing it a
zeroed buffer gets zeros where the language promises the declared literal.

## How it was found, which is the part worth keeping

Not by accident. The four defects of the day had one thing in common: **data that outlives something**.
Two censuses now cover address formation and value movement; the axis neither covers is **memory the
emitter READS but did not write**, and that is the axis the previous defect sat on.

Enumerating the 16 read sites and asking of each *what guarantees the contents* put four on the
host-provided boundary — the resume-state word, the initialisation flags, shared data, and **the
private slot array, whose guarantee is a table this backend had never looked at**.

## What the fix is, and what it is not

**Publish the initial image; do not invent a load step.** There is no native "load" — a host supplies
the buffer — so the backend's obligation is to state what must be in it, exactly as
`persistent_supplement_bytes` states how big it must be. That figure's own documentation already
concedes the weaker guarantee: *"a figure the host must remember to add is not one the runtime's
sizing function includes."* The same applies here and must be said in the same voice.

## The wrong turns

1. **Do not initialise composite slots.** Their `private_init` is `Unit` by design, and the
   initialisation word added in the previous increment makes a read of an unwritten one FAULT. Writing
   zeros into a composite's pool bytes and leaving the flag clear is right; setting the flag would
   undo the fix that just landed.
2. **Do not write the image at every call.** It is a load-time act. A per-call install would reset a
   slot that a previous call legitimately wrote, which is the opposite defect and would break
   `14_frame_log.kel`, whose count accumulates across cycles.
3. **Do not guess a width.** A `Byte`, a `Bool` and a `Word` are different widths, and a `Float` is
   four bytes under `narrow-float-32`. Take each from where the system states it, and refuse rather
   than pack a variant this backend cannot place.
4. **Do not fix only the harness.** Making the differential pass by installing the image in the test
   helper, without publishing it for real hosts, converts a defect into a defect nobody can see.
5. **Do not claim the shared segment is covered.** It is the host's to initialise by contract, and the
   census row should say so rather than implying this change touches it.

## What done looks like

The unwritten path agrees with the reference, the written path still agrees, a host has a published
way to obtain the bytes, the composite slots keep faulting rather than reading zeros, and the read
census exists so the next question of this kind is asked deliberately rather than stumbled on.
