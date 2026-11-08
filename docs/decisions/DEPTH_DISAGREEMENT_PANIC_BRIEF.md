# BRIEF — the depth-disagreement assertion on a public entry point

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## The suspicion, stated before measuring

`lower_chunk` tracks the operand-stack depth at every branch target and asserts agreement:

```
assert_eq!(prev, d, "operand-stack depth disagreement entering op{t}: ... \
                     The typed verifier guarantees agreement, so this is a lowering bug.");
```

**That is a panic, and `lower_module` is a public entry point that does not require a verified
module.** This line has already spent an increment on exactly that class: 58 panics reachable through
a public entry point, all converted to refusals, with `lowering_robustness.rs` built to keep them
converted.

**The message names the reason to doubt it.** "The typed verifier guarantees agreement" is true of
modules that went through the verifier. Nothing makes a caller run it — `Vm::new_unchecked` exists
precisely because trust-skip is a supported mode — and the mutation sweep exists precisely because a
malformed module can reach this code.

## What would make the suspicion wrong

The sweep is green, so **no mutation it applies today reaches the assertion.** That is evidence about
the sweep's reach, not about the assertion's reachability — the recorded lesson from this line's own
guard work: *a clean guard proves its reach before it proves the tree.*

So the increment is a construction, not a search: build a module whose branch target is reachable at
two different depths and see what `lower_module` does with it.

## The wrong turns

1. **Do not weaken the check.** If it is reachable, the fix is to REFUSE, not to drop the invariant.
   The depth agreement is what makes the emitted blocks well-formed; losing it would trade a panic for
   a miscompilation, which is strictly worse.
2. **Do not assume unreachable because the sweep is green.** The sweep mutates opcodes; a depth
   disagreement needs a JUMP TARGET that arrives from two edges at different depths, which may be
   outside every mutation kind it applies.
3. **Do not hand-write bytecode that the compiler could never produce and call it a defect.** The
   claim must be about what a public entry point does with input it does not reject — `lower_module`
   takes a `Module`, and anything constructible by decoding one is fair.
4. **Do not convert the panic and leave the sweep unchanged.** If a new reachable shape is found, the
   sweep should reach it too, or the next one like it goes unnoticed.
5. **Do not report this as a runtime hazard.** It is a compile-time panic in the backend's own
   lowering; the emitted code is not involved. Overstating it would be the mirror of the accusation
   problem just audited.

## What done looks like

Whether the assertion is reachable from a module `lower_module` accepts is established by
construction. If it is, it becomes a refusal with a named reason and a test that pins the refusal; if
it is not, the reason it is unreachable is written where the assertion is, so the next reader does not
have to re-derive it.

---

## OUTCOME — 2026-09-12

**Reachable, and it panicked.** Retargeting the `If(6)` of `if t > 0 { 1 } else { 2 }` to `If(7)`
makes op 7 arrive from the branch edge at depth 0 and from the then-arm's `Else` at depth 1.
`module_refusals` — a public entry point — died on it. It now refuses, naming the disagreement.

**The suspicion's grounds were right and its target was slightly wrong.** The brief said the message
"names the reason to doubt it", pointing at *"the typed verifier guarantees agreement"*. That is
exactly where the reasoning failed: true of verified modules, and nothing makes a caller verify.

### The sweep could not have found it, which the brief predicted and the fix corrects

Wrong turn 2 said not to assume unreachability from a green sweep, because a disagreement needs a
JUMP TARGET moved rather than an opcode corrupted. **That is precisely why it was green**, and the
sweep now carries a retarget mutation, so the class is covered rather than the instance.

### And the new mutation exposed a race in the sweep itself

Adding it raised the panic count from a handful to forty-nine, and the file began failing **only when
its three tests ran together**. The panic-origin hook wrote to a process-global `Mutex<Option<String>>`
while the harness ran those tests on parallel threads: one test's clear raced another's set, the origin
came back `unknown`, fell outside the `confine.rs` allowance, and was reported as a panic inside this
backend.

**A green or red result that depends on thread scheduling is neither.** The hook runs on the panicking
thread, so the value is now thread-local — not a workaround for the race but the right home for it:
the question is always *where did THIS thread's panic come from*. Three consecutive full-file runs are
green.

> **The race predates this increment and the volume made it certain.** That is the third harness defect
> this session, after two sized a buffer by a literal. The instruments are code, and nothing was
> auditing them as code.
