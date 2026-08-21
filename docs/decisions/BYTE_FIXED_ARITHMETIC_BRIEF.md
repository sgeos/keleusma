# BRIEF — the generic arithmetic cluster is NOT blocked on the float decision

## What changed my mind, and it was a contradiction between two of my own records

The handoff says four opcodes wait on **the float representation**, which is the operator's:

> *"`Op::Add` is emitted for `Byte` OR `Float` and the operand type is INVISIBLE at the lowering
> site."*

The lowering's own in-code comment says something different:

> *"`Op::Add` cannot be lowered without knowing whether its operands are `Byte` or **`Fixed`**, and
> the opcode does not say."*

**Two records, two different answers, four opcodes hanging on it.** Measured:

| source | emits |
|---|---|
| `Byte + Byte` | `Add` |
| `Fixed<16> + Fixed<16>` | `Add` |
| `Float + Float` | `Add` |
| `Word + Word` | `CheckedAdd` |
| `Byte * Byte` | `Mul` |
| `Fixed * Fixed` | **`FixedMul(16)`** — already lowered |

**Both records were wrong by omission.** `Op::Add` covers THREE types; each record named two.

## Why this makes the cluster reachable without an operator

`Fixed` addition is a **plain `i64` add** — the format is scale-independent, which the multi-word
work already established. `Byte` addition is the same add **plus a mask**, because the backend holds
the invariant that a `Byte` occupies the low eight bits (that invariant is what makes `ByteToWord` a
no-op, and `WordToByte` already masks). **Only `Float` needs a representation this backend does not
have.**

The backend tracks `Width` on its own operand stack: `Width::Scalar(1)` for a `Byte`,
`Width::Scalar(8)` otherwise. **That separates `Byte` from `{Fixed, Float}` but NOT `Fixed` from
`Float`** — and `Float` is exactly the one that must not become an integer add.

**So the enabling step is a stronger float guard, not a float decision.** Today the guard checks
chunk SIGNATURES only. A float can still enter through a constant, a data slot, or a native's
declared return shape. If the module is refused when a float appears by ANY of those routes, then in
an admitted module `Width::Scalar(8)` at an `Add` means `Fixed`, and the lowering is determined.

`Float` stays refused. **The operator's decision is untouched and still required to SUPPORT floats;
it was never required to lower `Byte` and `Fixed`.**

## Prior failures, and the specific wrong turns to avoid

- **This is arithmetic. A wrong lowering is a silent wrong answer, not a crash.** The differential
  oracle is the check, and `opcode_witness.kel`'s `byte_mix` becomes executable rather than exempt
  — that is the point, and it is also the risk.
- **Do not weaken the float guard to make something lower.** A correct refusal must not be traded
  for coverage; that rule is standing on this line.
- **The guard must be checked for COMPLETENESS, not just added.** Enumerate the routes a float can
  take and assert each is closed, or say which are unchecked. A guard that closes three of four
  routes while reading as total is the same shape as every other error this session.
- **`Byte` needs the mask; `Fixed` must NOT be masked.** Masking a `Fixed` would truncate it to
  eight bits and every later field offset would still look right — the `ByteToWord` relabel trap in
  a new place.
- **Do not trust `Width` to say more than it does.** It distinguishes one byte from eight. It does
  not distinguish `Fixed` from `Float`, and the whole approach rests on the guard making that
  distinction unnecessary rather than on `Width` making it.
- **Verify by EXECUTION, not by "it lowers".** `lower_module` returning `Ok` is a fact about the
  compiler. This line has already shipped an opcode whose saturating clamp no program reached.
- **Mutate the mask.** If breaking the `Byte` mask leaves the suite green, no program exercises the
  wrap, and the coverage claim is empty.

## STATUS UPDATE — the guard has LANDED, and one fact in this brief was refined

The widened float guard is in the tree with a test per route. **One route's boundary was not what
this brief assumed**: a module that DECLARES a float data slot and never reads it LOWERS; only an
ACCESS refuses. That is safe by construction — an unread slot puts no float on the operand stack —
and it is stronger than the "declaration refuses" claim it replaced.

**Exact semantics, measured from the virtual machine rather than assumed:**

| op | `Byte` | `Fixed` |
|---|---|---|
| `Add`, `Sub` | i64 op then `& 0xFF` | plain wrapping i64 |
| `Neg` | `wrapping_neg` on `u8`, so `(-a) & 0xFF` | plain wrapping i64 |
| `Mul` | i64 mul then `& 0xFF` | **never emitted** — `Fixed * Fixed` is `FixedMul` |

So `Op::Mul` is `Byte`-only once `Float` is excluded, and admitting an eight-byte operand for it
would be admitting a case that cannot occur. **Refuse it rather than handle it**: a lowering arm for
an unreachable case is untested code that looks tested.

**Width is the discriminator and it is only trustworthy where the signature says so.** `GetLocal`
carries a signature-derived width ONLY when the chunk never writes that local; anything written is
`Unknown`. So the dispatch must **refuse on `Unknown`** rather than guess — which costs coverage and
cannot mispack, the same trade already made for composite packing.

## What a good outcome looks like

`Add`, `Sub` and `Neg` lower for `Byte` and `Fixed`, verified by the differential oracle actually
executing them rather than by acceptance; `Float` still refused, by a guard shown to close every
route it claims to; and the handoff's "blocked on the operator" row corrected to name what is
actually blocked.

**If the guard cannot be shown complete, lower nothing and say so.** A refusal that is correct is
worth more than four opcodes that are usually right.
