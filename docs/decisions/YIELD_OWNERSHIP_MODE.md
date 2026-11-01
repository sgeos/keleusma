# Decision — `ref` / `out` on a yielding function's return signature

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

**Status**: **ACCEPTED IN PRINCIPLE, 2026-08-24, by the operator. Not scheduled, not implemented,
and not V0.2.x work.** The keyword pair and its position are settled; the open questions in the
last section are not.

**Scope**: a surface-language change to the `->` position of `yield` and `loop` declarations, plus
the host-facing driving interface. It is **V0.3.0 or later**.

**Constraints carried from the line**: no new opcode, no `BYTECODE_VERSION` change without operator
authorization, and worst-case execution time and memory usage must both stay derivable.

---

## The two models

A yielded composite is handed across the machine-host boundary. There are two conventions for
whose storage it lives in, and today the language offers only the first.

This C illustration is the operator's, and it is exact:

````c
struct payload { int x; };

/* this lives in the arena */
static struct payload machine_store = { .x = 0 };

struct payload *set_value_machine(int new_value) {
    machine_store.x = new_value;
    return &machine_store;                       /* callee storage, caller borrows */
}

void set_value_host(struct payload *hidden_host_owned_return, int new_value) {
    hidden_host_owned_return->x = new_value;     /* caller storage, callee writes */
}
````

**`machine_store` being `static` is not incidental — it IS the reuse hazard.** Run the program with
a second machine-model call and the first pointer reads the second call's value:

````
Machine: 7
Host: 5
after second machine call -- a: 9  b: 9  same object: 1
````

That is the composite-region-reuse hazard in four lines, and it is what
[`COMPOSITE_REGION_EVIDENCE.md`](./COMPOSITE_REGION_EVIDENCE.md) records against the runtime: a
yielded composite is an epoch-guarded arena handle, and **an overwrite in place advances no epoch**,
so a reused slot returns the wrong iteration's bytes with `resolve` succeeding.

## The decision

A `yield` or `loop` declaration states its convention in the return position, and **both forms are
mandatory**:

````
loop main(ack: Word) -> ref Reading     // a reference to machine storage; expires at RESET
loop main(ack: Word) -> out Reading     // written into storage the host supplies
````

- **`ref`** — today's semantics. The host receives a handle into the arena, must consume it before
  resuming, and it goes stale at `RESET`.
- **`out`** — the host supplies the destination and the machine constructs into it. The machine
  retains no reference afterwards.

### Why the return signature rather than the yield site

The site was considered first and rejected, for three reasons.

1. **The signature is the contract.** The host reads declarations; yield sites are internal to the
   module and the host never sees them.
2. **The host must know before the first call.** Under `out` it supplies the destination, so it
   needs the convention and the size before calling — from the declaration, not from a site it has
   not reached.
3. **Per-site placement forces a host-visible protocol change.** If sites within one stream could
   disagree, the host could not know which convention the next yield uses, and `VmState` would need
   a second yielded variant. Signature placement is uniform per entry point and needs none.

The position is already contractual in this language: `->` carries information-flow labels, and
`ephemeral`, `signed` and `pure` already qualify a declaration. `-> out Reading@Sensor` composes.

### Why `ref` and `out`

Surveyed vocabulary splits into two families.

| framing | languages | keywords |
|---|---|---|
| **direction** — who supplies the storage | Ada, C#, D, COM/MIDL | `in`, `out`, `in out`, `ref`; MIDL adds `[ref]`/`[unique]`/`[ptr]` |
| **ownership** — the receiver's relationship | Mojo, Vala, Swift, Objective-C ARC, Pony | `borrowed`/`owned`, `owned`/`unowned`, `borrowing`/`consuming`, `strong`/`weak`/`copy`, `iso`/`val`/`ref`/`box`/`tag` |

**Ada's pair was proposed and does not survive the move to a return position.** `out` transfers
cleanly — *delivered into storage you supply*. **`in` does not transfer at all**, because `in` means
input-to-callee and a return is never input.

Ada does distinguish these two models, but **structurally rather than by keyword**:
`function F return Payload_Access` against `procedure F (Result : out Payload)`. So the honest
Ada-derived pair is `access`/`out`, not `in`/`out`. `access` was rejected as heavier and carrying
Ada-specific baggage.

`ref` was taken from the same family instead. Both halves read correctly in a return position, both
are established parameter modes in C#, D and Ada-adjacent practice, and **neither is deictic**,
which matters for the bilingual book — see the rejected alternative below.

**Both forms are mandatory rather than defaulting to `ref`.** Leaving today's behaviour unmarked
would make the case that carries obligations — expiring handle, consume-before-resume, ineligible
for slot reuse — the silent one. That is the wrong way round for a language whose value proposition
is explicit static guarantees. It makes this a breaking change, which is proper V0.3.0 material.

### The alternative that was rejected, and why it was close

`yield your value;` / `yield my value;` — possessive pronouns naming the party.

It was the strongest option **at a yield site**: shortest possible, unambiguous about *by whom*
where `borrowed`/`owned` is not, reading as English, and `my` carries the obligation ("this is mine,
do not keep it") where a factual keyword states only the fact.

Two things defeated it.

**Moving to the signature removes its anchor.** `yield my value;` is an utterance with the machine
unmistakably speaking. `-> my Reading` is a declaration, and a declaration has no speaker in the
same way. The possessive draws its meaning from the act of yielding, and the signature removes the
act. The audience changes too: a signature is read by the host implementer, for whom `ref`/`out` is
immediately legible and a pronoun convention is not.

**Deixis has already cost this project an operator escalation.** `HANDOFF.md` records that "they"
and "their" resolved against whoever held the document and produced *the exact inversion* across two
lines' mailboxes. First and second person pronouns are the paradigm case. In code the anchor holds,
because only the machine writes `yield` — but **in prose about the code it does not**: an embedder
documenting their Rust will write "the script yields my value here", where "my" now means the host.
The Japanese translation makes it worse, since Japanese routinely drops pronouns, and this book has
already carried a translation stating the opposite of its source for nine days.

`borrowed`/`owned` was also proposed and withdrawn: it names the relationship but not the party, so
"an owned `Reading`" does not say owned by whom. Recorded because **Mojo uses exactly that pair**,
so it is proven legible in practice and the withdrawal is a judgment rather than a defect.

## What it buys, measured rather than asserted

Under `out` the machine **constructs directly into host storage** — there is no arena region for
that site at all. That is materially better than the escape-copy discipline of Theorem B2, which constructs in the
arena *and then copies*. That proof is on the `proofs` lineage and has not yet merged into this
line, so it is cited by name rather than by link. As of 2026-08-24 it sits on the FEATURE
branch and not yet on the line branch:
`git show origin/proof/composite-region-reuse:docs/proofs/COMPOSITE_REGION_REUSE_PROOF.md`.
**Checked, because the obvious guess is wrong**: `origin/proofs` carries the mailbox and the
evidence guard, but no `docs/proofs/` yet.

Applied to the program measured at heap **112 bytes** under today's no-reuse model — a 16-byte
array, a 24-byte struct built twice in a loop, two further 24-byte structs:

| regime | arena term | copy term | total |
|---|---|---|---|
| today, `ref` everywhere | 112 | — | **112** |
| B2 escape-copy, machine-owned copy store | 88 | 72 | **160** |
| B2 escape-copy, host-owned copy store | 88 | 0 | **88** |
| **`out` on the yielding declaration** | **88** | **none — the construction IS the write** | **88** |

**B2 with a machine-owned copy store is worse than doing nothing.** The `out` form reaches the good
figure without paying a copy, because it never allocates the arena region.

The arithmetic for the B2 rows is Corollary B2a applied to a measured figure; the `112` is measured,
the rest is derived. **Nothing here has been run against an implementation, because there is none.**

**And the soundness property is the real prize.** Under `out` the machine retains no reference to
host storage after the write, so there is no lifetime question, no epoch question, and slot reuse
behind such a declaration is safe **by construction** — no confinement analysis required for those
sites. It closes the `v0.3.0` planner's yield hazard for them outright.

## What already exists, so the change is smaller than it looks

**`shared data` is already host-owned storage that the machine copies into**, and it already accepts
a composite field. Measured:

````
shared data out { latest: Reading, seq: Word }   ->  shared_data_bytes = 24
````

The host reads those bytes directly and they survive `RESET`. So a program can obtain host-owned
semantics **today** by writing to a `shared data` block and yielding a scalar acknowledgement.

**What is missing is that `yield` cannot express it**, so programs must restructure. This decision
buys ergonomics and accounting, not new capability, and **no new opcode is required** — `SetData`
already copies into host storage.

## Open questions, which this decision does NOT settle

1. **How the host learns the buffer size.** `shared_data_bytes()` is the precedent; the analogue is
   a per-entry-point query for the `out` payload size. Unspecified.
2. **Depth.** A flat composite writes as bytes. One containing `Text` or an opaque handle cannot —
   the handle would point back into the arena. `out` likely needs a transitively-scalar restriction
   at first. This is the same boundary the proof flags for its clause 5 and its boxed path.
3. **The consumption contract under `ref`.** Several yields per cycle mean the host must consume
   before resuming. That is already the shared-data contract, but it should be written down rather
   than inherited.
4. **Whether `out` and `shared data` should be unified** rather than kept as two ways to reach
   host-owned storage.
5. **Whether a per-site override is ever wanted.** Deliberately deferred; it can be added later
   without disturbing the signature form, at the cost of the protocol change described above.
6. **The worst-case-execution-time accounting.** Constructing into host storage is not free, and
   `Op::cost()` currently disagrees with measurement on 50 of 66 opcodes. That reconciliation is
   already ruled to follow Order 1, and this decision should not be scheduled ahead of it.

## Provenance

The two models and the C illustration are the operator's, as is the ruling that the return signature
is the correct position because it is the caller-callee contract. The measured figures are from the
`v0.2.3` line; `112` and the `shared data` composite result are executed, the comparison rows are
derived from Corollary B2a. The keyword survey is analysis. Theorem B2 and its corollaries are on
the `proofs` lineage and are **unruled in either direction** as of this date — this decision neither
adopts nor declines them.
