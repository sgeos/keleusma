# BRIEF — the blind spot the last audit named

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## Present goals

| # | Goal | Actionable here? |
|---|---|---|
| 1 | Audit the capability-claim names — the class the last audit could not see | yes |
| 2 | Report the rate, and state what remains unaudited | yes |
| 3 | Keep the gate green and the branch published | yes |
| 4 | `Fixed` shared-slot ABI, float entry ABI, git topology | **no — operator** |

## Rationale

The previous audit selected `#[test]` names opening with a universal or negative quantifier and found
**2 of 11 overclaimed**. It recorded its own blind spot: **the rule sees leading quantifiers only**,
and *"the region canary can fire"* — the defect fixed earlier the same day, whose body never made the
canary fire — would not have been caught, because that name begins with "the".

**That class is now countable.** Names asserting a capability — *can*, *cannot*, *must*, *able* —
number **11 of 325**, the same size as the last set and the one the canary belonged to.

**A capability claim has a specific failure mode**, which is why it deserves its own pass: the body
can establish that a thing *is so* without ever establishing that the mechanism *can report it*. That
is exactly the canary: the region demanded more than a word, which is true and says nothing about
whether the canary fires.

**Two classes remain after this**: mid-name quantifiers, measured at **36**, and everything with no
syntactic marker at all. The second is unbounded and is why no rate here can be called complete.

## Prior failures to avoid repeating

1. **Four increments running, the finding has been a name asserting more than its body proves** — a
   canary, a guard's population three times, a measured property, and two test names.
2. **A rate without its blind spot reads as complete.** The previous audit said so explicitly; this
   one must too.
3. **Weakening the name is the cheap repair** and silently reduces what the suite proves. Choose per
   case.
4. **An edit that silently did nothing** because its assertion was omitted. Assert on every
   substitution.
5. **Fifteen recorded premises found false or overstated in consecutive increments.** Write the
   prediction: at least one of the eleven overclaims.

## Specific wrong turns to avoid

- **Do not count a capability name as sound because the property holds.** The question is whether the
  body demonstrates the capability, not whether the claim is true. A test can prove the right thing
  and still not prove what its name says.
- **Do not treat "the test passes" as evidence either way.** That was the entire defect last time.
- **Do not audit the mid-name-quantifier class halfway.** Either take it as a whole in a later
  increment or leave it, and say which; a partial sweep produces a rate that describes nothing.
- **Do not rename a test whose body genuinely demonstrates the capability** just because the pattern
  matched it. The deliverable is a classification, and "sound" is a legitimate outcome for all
  eleven.
- **Do not touch** `src/verify.rs`, `src/bytecode.rs`, `src/vm.rs`, `src/wire_schema.rs`,
  `src/selfhost/`, `src/confine.rs`, `.github/workflows/`.
