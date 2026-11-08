# BRIEF — how many of the virtual machine's "should never happen" refusals actually happen?

## The goal, and why it is the project's core guarantee rather than bookkeeping

The virtual machine carries a small class of refusals whose message says the opcode **should never
have been emitted at all** — a mis-compilation rather than a bad program. There are exactly three,
naming two opcodes:

| site | message |
|---|---|
| `Op::Len` on a flat ARRAY | *length is a compile-time constant* |
| `Op::Len` on a flat TUPLE | *arity is a compile-time constant* |
| `Op::IsStruct` on a flat STRUCT | *the type test is a compile-time constant* |

All three raise `InvalidBytecode`, **which is the class `verify()` exists to exclude at load time.**
A program that passes `verify()`, receives a resource bound, loads, and then dies here is a
**load-time hole**: the verifier admitted something it is supposed to reject.

**One of the three is already proven reachable.** Four constructs reach the `IsStruct` site, two of
them dying with `InvalidBytecode`, and one is ordinary generic code. **Nobody has asked about the
other two.**

This bears directly on the conservative-verification stance: the whole claim is that a program the
verifier admits is one the runtime will not refuse as malformed.

## What "reachable" has to mean, or the answer is meaningless

**Not merely "a program emits the opcode".** The `Op::Len` witness emits it and is REFUSED A BOUND —
the loop has no statically extractable iteration count, so it never loads and never reaches the
virtual machine at all. That is the conservative-verification stance working, not a hole.

So reachable means the full chain: **compiles, `verify()` accepts, a resource bound is granted, the
module loads, and execution reaches the site.** Anything short of that is a different result and must
be reported as the different result it is.

## The likely shape of the answer, stated as a hypothesis to test rather than assume

`Op::Len` fires only when a for-in source has no statically known length — and a loop whose trip
count is unknown is exactly what the bound extractor refuses. **The property that reaches the opcode
may be the property that denies the bound**, in which case those two sites are unreachable from any
admissible program for a structural reason rather than by luck.

`Op::IsStruct` has no such coupling, which is why it is reachable.

**Do not assume this.** It is the same shape as the `Op::Len` finding already recorded, which is
suggestive and not evidence.

## Prior failures and specific wrong turns to avoid

- **Do not guess syntax.** Six of eleven probes did not compile in the previous increment, which is
  this line's third recorded instance of inventing grammar from memory. **Derive probes from corpus
  sources or from the emission condition**, and report the compiling denominator, never the attempted
  one.
- **Report "did not compile" separately from "did not reach".** They are different results and only
  one is evidence.
- **"I could not reach it" is not "unreachable".** That distinction is this line's own `Op::Reset`
  lesson and the `v0.2.3` line adopted it for `IsStruct`. Record the search alongside the result, so
  a negative reads as "I looked at these" rather than "I looked".
- **Do not edit `src/vm.rs` or `src/verify.rs`.** Both are the `v0.2.3` line's. This is a
  measurement; any repair is theirs.
- **A reachable site is a finding to report, not to fix here.** The previous increment's finding is
  with them and unmerged; adding to it is useful, pre-empting it is not.

## What a good outcome looks like

Each of the three sites has a stated reachability verdict, with the full chain measured rather than
inferred, and with the compiling denominator beside any negative.

**If all three are reachable, that is a serious result about the verifier.** If two are structurally
unreachable because the bound extractor refuses their precondition, that is a good result and worth
recording as the mechanism rather than the outcome — it would mean the conservative-verification
stance is load-bearing in a place nobody had checked.
