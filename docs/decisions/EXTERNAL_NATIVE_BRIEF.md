# BRIEF — the one opcode that is lowered, witnessed, and executed by nothing

## The goal

`CallExternalNative` is counted as **lowered** by the support census and **witnessed** by the
coverage census. **No corpus module executes it.** Measured: exactly one module in the whole corpus
emits it — `external_native_witness.kel`, created this session by splitting the witness corpus — and
that module is EXEMPT:

> *the VM refuses to run it: native `host::tick` registered as verified but bytecode invokes it as
> external*

**That is a harness registration choice, not a backend limit.** `Vm::register_external_native`
exists; the harness has simply always used the verified registration for every native.

This is the sharpest remaining instance of the distinction this whole line turns on: **a lowering
figure is not a correctness figure.** One opcode sits in the supported column with nothing having
ever run it, and the reason is our own configuration.

## Why this one is a CONTRACT and the string mask was not

The reference-argument mask closed ten modules and rests on a RUNTIME observation — what the virtual
machine happened to see, because `Module` records no native parameter types.

**This one is derivable from the artefact.** The bytecode says which natives are called externally:
`Op::CallExternalNative(idx)` versus `Op::CallVerifiedNative(idx)`. Scanning the module's ops for
those operands gives the classification the module itself declares, with no runtime inference.
**Say that difference plainly** — the two closures should not be described as equally well founded.

## The obstacle, and it is real

`register_external_native` takes a bare `fn` pointer, not a closure, so it cannot capture the
native's index or name. The native side already solves exactly this: a family of `kel_stub_NN`
functions that recover their identity from a thread-local table. **Mirror that rather than inventing
a second mechanism**, and reuse the same table.

## Prior failures and specific wrong turns to avoid

- **Do not register a native as external unless the module says so.** A mismatch is rejected at
  call-site dispatch in the other direction too; classifying by guess would trade one exemption for
  a different one.
- **Do not build an unbounded family.** A fixed set with an explicit refusal past its end is honest;
  silently binding the wrong function is not. The native stub family already has this shape.
- **Check the executed count actually moves.** Twice this session a change that should have made a
  module executable did not, and the reason was a different blocker: a corpus split that left an
  external-native mismatch, and a mask patched into one of two registration paths. **If the count
  does not move, the remaining blocker is the finding.**
- **The module must AGREE, not merely run.** A newly-executed module that disagrees is a real result
  and must be reported, not masked.
- **State what this does not establish.** Executing one synthetic witness is not evidence that
  external natives work generally; it is evidence that this opcode has been executed once. The
  corpus contains no real program using one.

## What a good outcome looks like

`external_native_witness.kel` executes and agrees; the exempt count falls by one; `CallExternalNative`
stops being an opcode nobody has run; and the report distinguishes this contract-derived
classification from the runtime-derived reference mask.

**If the effort turns out to exceed the value, say so and stop.** One synthetic module is a small
payoff, and the honest reason to do it is that it removes the last case of "supported but never
executed" — not the module count.
