# BRIEF — the four sites I counted and then refused to judge

## The goal

Correcting the mis-compilation class from three sites to seven added four that carry **no
reachability verdict**:

```
GetField      operand form does not match struct body
GetIndex      operand form does not match array body
GetTupleField operand form does not match tuple body
GetEnumField  operand form does not match enum body
```

I wrote that they *"require a form mismatch between construction and access, which the compiler
chooses by static type — that suggests unreachability, and suggestion is not evidence."* That is the
right thing to have written and it leaves the work undone.

## Why there is a specific hypothesis worth testing, not just a fishing trip

**The `Op::IsStruct` root cause was exactly a one-sided rewrite.** `rewrite_pattern_enum_name`
rewrote a pattern's ENUM name on specialization and left a struct pattern's own name alone, so a
generic parameter had its TYPE rewritten to `P__Word` while its PATTERN still named `P`. Construction
and access disagreed because monomorphization touched one and not the other.

**These four sites are the same shape one level down**: construction bakes a body form, access bakes
an operand form, and they agree only because both are derived from the same static type. **Anything
that rewrites one side and not the other reopens it.** Generics are the obvious candidate, and the
`v0.2.3` line has just fixed one instance of precisely that.

So the probe set is not "try constructs" — it is **"find places where the form could be decided
twice"**, and generics are where a type is decided twice by construction.

## Prior failures and specific wrong turns to avoid

- **Do not guess syntax.** Three recorded instances of inventing grammar from memory, most recently
  six of eleven probes failing to compile. **Derive probes from the corpus or from the emission
  condition**, and always report the compiling denominator.
- **Read how the form is CHOSEN before probing.** The `IsStruct` falsification worked because I read
  the condition and enumerated what satisfies it; the eleven-guess attempt before it did not.
- **A negative here is weak by construction.** These sites need construction and access to disagree,
  and the compiler derives both from one type. Failing to break that is close to expected, so **do
  not dress it up**: say what was searched, and never write "unreachable".
- **Do not report a suggestion as a result.** That is the exact sentence I already wrote about these
  four; the increment exists to replace it with a measurement or with a stated bounded search.
- **`src/compiler.rs` and `src/vm.rs` are the `v0.2.3` line's.** Read, never written. A finding is
  reported to them, as the `IsStruct` one was.
- **If a mismatch IS reachable, it is a defect report and it goes to them before anything else.** It
  would mean a program can verify, load, and die on a body-form disagreement — the same load-time
  hole class, in four more places.

## What a good outcome looks like

Each of the four sites has either a demonstrated reachability, or a bounded search recorded with its
probes listed and its denominator stated — replacing the current "suggests unreachability" with
something a reader can audit.

**A clean negative is a fine outcome here** and is worth recording precisely because the previous
sentence was a guess. What is not acceptable is leaving the guess in place while the class count has
already been corrected around it.
