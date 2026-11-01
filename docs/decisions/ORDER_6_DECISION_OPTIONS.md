# Order 6 decision options, questions 12 to 14

> **Navigation** [Decisions](./README.md) | [Order 6 completion](./ORDER_6_COMPLETION.md) | [Operator questions](../process/handoffs/v0.3.0-QUESTIONS.md)

**Status, proposal of 2026-10-27. Nothing here is decided or implemented.** The
operator asked on 2026-10-27 for the unblocking work this line can do without
operator input. Three of the open items of
[Order 6](./ORDER_6_COMPLETION.md) wait on questions 12, 13 and 14 of
[the operator question list](../process/handoffs/v0.3.0-QUESTIONS.md). This
record turns each question into a choice among concrete options, states what
each costs, and gives a recommendation. A recommendation is this line's
judgment and not a ruling. No code, workflow or toolchain was changed to
prepare it.

## What is already settled, so that it is not reopened

| Item | Settled by | What it settles |
|---|---|---|
| A string passed to a native | [Option B](./STRING_ABI_OPTION_B.md), operator rulings of 2026-08-29 and 2026-08-30 | A native observes a borrowed, length-delimited view of the bytes, valid for the call, under both embeddings, as [the native string ABI](../spec/NATIVE_STRING_ABI.md) specifies |
| Dynamic text with a bound | [`TEXT_CAPACITY_TYPE.md`](./TEXT_CAPACITY_TYPE.md), operator authorization of 2026-08-31 | `Text<N>` is a flat composite of a length and `N` bytes with no reference field. The native backend already packs and reads flat composites. It is authorized and not implemented, and it is built in `src/`, which V0.2.X owns |
| The size of an `Opaque` slot | [`OPAQUE_ADDRESS_WIDTH_BRIEF.md`](./OPAQUE_ADDRESS_WIDTH_BRIEF.md), operator confirmation of 2026-08-31 | An `Opaque` slot is sized by the address width, not the word width |
| The intent for `Opaque` | [`ABI_RULINGS.md`](./ABI_RULINGS.md), operator remark of 2026-08-29 | "a pass through pointer to data that the host allocates for itself". The record notes that the remark was not framed as a ruling between options |

The remaining questions are therefore narrower than their wording suggests.
Question 13 concerns only bare `Text` that is not a literal, because `Text<N>`
needs nothing new natively. Question 14 concerns what an `Opaque` word is at
the native boundary, because its size is settled.

## Question 13, bare `Text` that is not a literal

**The gap, measured.** Native code carries a literal as the address of a
`{ i64 len, bytes }` record it lays out. The virtual machine carries a flat
`Text` as a handle of `2 * word_bytes`, an address or offset and a length or
epoch, with a discriminant between a static literal and an arena string. A
`Text` from a call, a parameter, a native result or a field has no agreed
native form, so `text.rs` refuses its comparison and `NewComposite` refuses a
`Text` field, as [`TEXT_FIELD_VERDICT.md`](./TEXT_FIELD_VERDICT.md) records.
The reference verifier already charges a per-byte term for a text operation
and treats an operation on text of unbounded length as not statically
bounded, so any option inherits that cost rule rather than inventing one.

| Option | Native form of bare `Text` | What it opens | What it costs |
|---|---|---|---|
| **T1, a two-word view** | `{ ptr, len }` carried by value, the same size as the virtual machine's handle. A literal becomes a pointer to its bytes and its length | Comparison by content of any two `Text` values, a `Text` field in a composite, a native returning `Text` | The literal ABI that natives see changes from one pointer to a pointer and a length. The generated header absorbs that, but every existing C host changes. A host-returned `Text` needs a lifetime rule, proposed as valid until the instance's next step, which mirrors the arena rule. The virtual machine's epoch and discriminant have no native counterpart, so a host reading a `Text` shared slot sees different contents under the two embeddings, and only the size agrees |
| **T2, one word everywhere** | The address of a `{ len, bytes }` record for every `Text`, as literals are now | The same as T1, at a smaller change to the literal path | A host returning `Text` must lay out and keep alive a record in this line's format. A `Text` field occupies two words in the module's layout but carries one, so the second word is padding and differs from the virtual machine's handle in content and in meaning |
| **T3, literals only, with `Text<N>` for dynamic text** | Unchanged. Bare `Text` stays literal-only natively, and the refusals stay. Dynamic text uses `Text<N>` once V0.2.X builds it | Everything T1 opens, for programs that use `Text<N>`. Comparison and packing then cost nothing new, because `Text<N>` is a flat composite | A program that takes bare `Text` from a host stays refused natively. The gate item closes only when `Text<N>` lands, which is V0.2.X's work and is not scheduled on its line |

**Recommendation, T3.** It is the only option whose memory and time bounds
are static by construction, which is the ecosystem's stated value. It adds no
lifetime rule at the boundary and no embedder-visible change to the literal
ABI, and it builds on an authorization already given rather than asking for a
new one. Its cost is that Order 6's `Text` item closes on V0.2.X's schedule
and not this line's. If the operator wants the item closed sooner, T1 is the
better of the other two, because it matches the virtual machine's size and
does not require hosts to adopt this line's record format.

**A decision under T3 would also settle** whether bare `Text` from a host is
expected to remain supported natively at all, or is to be retired in favor of
`Text<N>` at the native boundary. This line has no evidence about embedders'
use of bare host-returned `Text` and does not assume either answer.

## Question 14, `Opaque` natively

**The gap, measured.** The virtual machine holds an opaque value as
`Value::Opaque(Arc<dyn HostOpaque>)`, reference-counted and checked by type
when a native downcasts it. Its worst-case memory contribution is zero,
because the host owns the storage, and it may cross a yield. Native code
refuses it on every route, and the emitter's message assigns host handles to
Workstream D, where no design was made.

| Option | Native form of `Opaque` | What it opens | What it costs |
|---|---|---|---|
| **O1, a typed host pointer** | The host's pointer, carried verbatim and never dereferenced by native code, of the address width. The generated header declares a distinct incomplete struct per declared opaque type, for example `struct kel_opaque_Device *` | Opaque parameters, results, fields, shared slots and yields, natively | No reference counting and no type check at run time. A stale or wrong pointer is the host's error, and native code cannot detect it. Type distinction is at C compile time only. The two embeddings differ in how a native body reads the value, so one host body does not serve both, unlike Option B for strings |
| **O2, a registry index** | A word-sized index into a table the host owns, optionally with a generation count | The same as O1, and stale-handle detection when the host checks the generation | The host implements and bounds the table. Each use costs an indirection in host code. The address-width ruling sized the slot for an address, so an index leaves bytes unused, which is harmless but should be stated |
| **O3, stay refused** | Unchanged | Nothing | Order 6's opaque item stays open until a consumer needs it |

**Recommendation, O1.** It is what the operator's remark of 2026-08-29
describes, it matches the settled address-width slot exactly, and it adds no
allocation and no worst-case memory. Native lowering would refuse a module
whose address width differs from the target's pointer width, so the
narrow-word concern recorded in `ABI_RULINGS.md` cannot produce a mispack. The
trust boundary moves to the host, which must keep each pointer valid for as
long as the instance can hold it. That boundary is the substantive part of the
decision and is why this is a question rather than an implementation choice.
O2 is the better choice if the operator weights stale-handle detection above
simplicity.

## Question 12, executing the Linux targets

**The gap, measured.** `tier_one_targets.rs` builds the lexer library for all
four Tier 1 triples. It runs only `darwin_arch` triples, so the two Linux
triples are built and read and never linked or run. This machine has a Docker
client at `/opt/local/bin/docker` with no running daemon, and `Docker.app` and
`UTM.app` are installed. Neither was started.

| Option | What runs Linux | What it costs |
|---|---|---|
| **L1, a hosted CI job** | GitHub's `ubuntu-24.04` and `ubuntu-24.04-arm` runners, one job each | CI time on every native path change, which question 11 already weighs. LLVM 22 must be installed there, presumably from `apt.llvm.org`, which this line has not verified |
| **L2, a local container** | Docker or UTM on this machine, run by hand at each Order 6 checkpoint | Starting a daemon, which needs operator approval. Evidence is local only and does not gate a merge |
| **L3, a release-time check** | Either of the above, run once before V0.4.0 is published | The least running cost. Linux could regress between checks without notice |

Every option needs the same test change. `tier_one_targets.rs` would run a
triple when it equals the host's own triple, in addition to the `darwin_arch`
route, and the Linux host would link it with the system `cc`. That change is
not made here, because it would be code that nothing on this line can
execute.

**Recommendation, L1 for `x86_64` and `aarch64` together, scoped to the
targets test and the command-line test**, `tier_one_targets` and
`kel_native_cli`, rather than the whole native gate. The two targets are the
only Order 6 item that hardware alone blocks, and a scoped job keeps the CI
cost to a fraction of the macOS gate. L3 is the fallback if question 11's CI
load concern outweighs continuous evidence.

## What an answer would let this line do next

| Answer | Next step on this line |
|---|---|
| T3 | Nothing until `Text<N>` lands, then a native differential for `Text<N>` comparison and fields |
| T1 or T2 | A native representation change in `native_codegen/`, with the header, the literal path and the refusals in `text.rs` revised, and a request to V0.2.X if the virtual machine's handle must change to agree |
| O1 or O2 | Lowering of `Opaque` in `native_codegen/` with a generated typed declaration, and a differential against the virtual machine through a host native |
| L1, L2 or L3 | The `tier_one_targets.rs` change above, and the job or the local run |
