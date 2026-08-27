# Brief: Hexadecimal and Binary Literals in `lexer.kel`

**Drafted**: 2026-08-26 (session 54, iteration 2)

## The goal, and why it is now the right one

`wire.kel` is the largest stage at 486 chunks and the only one outside the byte-identity
corpus. Its failure now names its cause, and the cause is smaller and more specific than
anything previously recorded for it.

**Measured, not inferred.** `keleusma::selfhost::lex_token_trace` shows `0xFF` lexing as:

| token | meaning |
|---|---|
| `(12, 0)` | the number literal **zero** |
| `(1, 2)` | an **identifier** interned under the name `xFF` |

`0b1010` behaves identically, interning `b1010`. Decimal is unaffected. So `lexer.kel`
consumes the leading `0`, stops, and treats the radix prefix and digits as a name.

`crc_begin` in `wire.kel` is `crc.acc = 0xFFFFFFFF; crc.acc`, which is why that chunk is
the one the stage refuses on.

## The exposure, stated with proportionality

**State this every time.** `self_hosted_compile` cross-checks against the reference and
refuses on divergence, so a CLI user got a loud error, never a wrong artifact. Exposure was
to direct callers of the `self_host_compile*` entry points. Before the range-arity guard
landed, those callers received a **silently wrong module**: an undefined name where a
constant belonged.

## Why it went unmeasured

**The construct-support boundary contains no hexadecimal or binary literal.** Ninety-six
cases and not one. This is the fourth recorded instance of the same class — the boolean
literal, the `Byte` cast, the bare `for` form, and now the radix prefix. *Any construct the
corpus does not contain is unverified by construction*, and the corpus is the twelve stage
sources plus whatever the boundary happens to list.

**Add the boundary case before the fix**, so the table records the gap in the failing
direction and the repair is what flips it.

## Prior failures this work must not repeat

- **Confirm the reference accepts a generated probe** before concluding anything about the
  stage. Five probes in this repository measured something other than what was intended.
- **A guard that cannot fire is worse than none.** Two were written this session that could
  not fire as first drafted, and only running them showed it.
- **Three independent signals over one feature set are still one feature set.** The first
  push this session was green locally on three signals and red on four continuous
  integration jobs, because a new test file lacked its feature attribute.
- **Do not read a number in a message as if it identified a cause.** That error is what
  made this defect take three sessions to reach.
- **`highest_command()` is a real guard**; a new command returns `0 - 99` until it moves.

## The specific wrong turns to avoid

1. **Do not add a new opcode.** Rad-hard minimal ISA; unconditional.
2. **Do not assume the reference's lexer rules.** Read `src/lexer.rs`. **Already read, and
   one rule would certainly have been guessed wrong:**

   | form | reference behaviour |
   |---|---|
   | `0x` / `0X` + hex digits | hexadecimal |
   | `0x` with no hex digit | **error**, "expected hex digits after '0x'" |
   | `0b` lowercase | **always** binary; `0b` alone is an error |
   | `0B` + `0` or `1` | binary |
   | `0B` otherwise | **not binary.** The `B` begins the `Byte` suffix, so `0Byte` is the byte literal zero |

   That last row is the trap. A stage treating `0B` as a binary prefix unconditionally
   would mis-lex `0Byte`, a form the corpus does use. **The stage must match the
   reference, not a reasonable guess at it**; a divergence here is a miscompile, not a
   missing feature. Both radices also reject a value that does not fit an `i64`.
3. **Do not stop at `0x`.** Binary (`0b`) fails identically and is in the same code path.
   Whether octal exists in the reference is a question to answer by reading, not to assume
   in either direction.
4. **Do not let the interner keep the junk name.** `xFF` is currently interned as a real
   name. A fix that lexes the number correctly but leaves the name table polluted will
   pass a byte-identity test on ops while diverging on `NAMES`.
5. **Do not claim `wire.kel` self-compiles** on any partial result. That false claim was
   invented once already and reached a doc comment, a pull-request body and three channels.
   `wire.kel` has other constructs; fixing the lexer may reveal the next one.
6. **Expect the next failure and say so.** The honest prediction is that `wire.kel` fails
   again on something else. That is progress, not regression, and the message will name it.

---

## THE NEXT BLOCKER, MEASURED AND PARTLY ELIMINATED

With radix literals working, `crc_begin` compiles and `wire.kel` fails at chunk `put_u64`
with a **different named cause**: a pop from an empty work stack. That is progress, and the
message named it in one reading.

**THE FAILURE IS CONTEXT-DEPENDENT, WHICH IS THE WHOLE DIFFICULTY.** `put_u64` sits at line
270. A prefix of `wire.kel` through line ~800 compiles; a prefix through line 2000 fails at
`put_u64`. **Something 1,700 lines AFTER the reported chunk changes how that chunk is
handled.**

**ELEVEN GUESSED REPRODUCTIONS ALL PASSED**, including the three real functions verbatim in
file order. This matches the recorded pattern where fourteen guessed constructs failed
before the real one was found. **Stop guessing earlier than feels natural**; the prefix
bisect over the real file found a boundary in one run.

### Hypotheses eliminated, each by measurement rather than reading

| hypothesis | measurement | verdict |
|---|---|---|
| the intern cap (1280 distinct identifiers) | whole file has **667** | **eliminated** |
| the token cap (40,960, collecting feed) | whole file is **~25,700** | **eliminated** |
| `put_u64` in isolation | compiles | **eliminated** |
| the three real writers verbatim | compile | **eliminated** |
| call-as-statement, in and out of a `for` body | compile | **eliminated** |

Both cap eliminations cost no build at all. **Do the arithmetic before running the
experiment** when a hypothesis names a threshold.

### The wrong turn most available here

**DO NOT TRUST THE REPORTED CHUNK NAME AS THE LOCATION.** The driver derives it from
`names[fns[i].name]`, an interned id. A defect that perturbs the name table would make the
name itself wrong, so the name is a LABEL and not yet evidence of a location. **Confirm the
name is stable across prefixes before treating `put_u64` as where the defect lives.** Every
other confident wrong reading in this file's history came from treating a number or a name
in a message as if it identified a cause.

### THE BOUNDARY IS EXACT. THE CAUSE I INFERRED FROM IT WAS WRONG.

Bisected to a single line. `wire.kel` truncated to line **1673** self-compiles; truncated to
line **1675** it fails. Line 1674 is blank, so the trigger is one added declaration:

| prefix | declarations | verdict |
|---|---|---|
| 1673 lines | **256** | compiles |
| 1675 lines | **257** | empty-stack pop |

> **RETRACTED WITHIN THE HOUR, BY THE EXPERIMENT THAT SHOULD HAVE COME FIRST.** I read the
> two counts, saw 256, and wrote "a cap of 256 on the chunk count" into this brief as a
> finding. **A synthetic program of 300 trivial chunks compiles.** So crossing 256 chunks is
> NOT the trigger. The 256/257 pair is a true measurement and the cause inferred from it is
> false.
>
> **This is the same error the whole increment is about**, committed while documenting it:
> *a number in a message read as if it identified a cause.* The number was even in the right
> place this time, which made it more convincing, not less. **What remains established is
> only the bisect**: line 1673 compiles, line 1675 does not, and the difference is one
> declaration. The mechanism is unknown.
>
> The paragraph below still holds as an observation about the guard, and is kept because it
> is independently true — but it is no longer evidence about THIS defect.

**A 256-WIDE CHUNK-INDEXED FAMILY IS A REAL HAZARD, AND THE STAGE HAS A GUARD FOR IT.**
`every_chunk_indexed_array_admits_the_chunk_cap` exists precisely because raising
`toks.chunks` from 256 to 1024 did not admit `wire.kel` — the chunk index also addressed six
`chunkret.ret_*` arrays and bounded two loops. Its own doc says a cap is a FAMILY.

**So why did it miss this one?** Because it derives the family from a **hand-written list of
two index expressions** (`ps.cur_chunk` and `call.call_chunk[`) inside **one file**
(`parse.kel`). That is the recorded meta-defect in its purest form: *a suite whose coverage
is a property of its case list, mistaken for a property of the thing under test.* The guard
is real, it fires correctly for the arrays it knows, and it cannot see an indexer it was not
told about or a stage it does not read.

**Do not widen anything yet: nothing has been shown too small.** The lesson the guard records is that
widening one member of a family moves the wall rather than removing it: an index trap became
a loop-limit trap became a different index trap, "each naming a size and none naming the
cap". Find the family first, and **strengthen the guard so it derives its indexers rather
than listing them** — otherwise the next member is found the same expensive way.

**And the reported chunk name was misleading, as predicted.** The failure names `put_u64` at
line 270, which cannot itself be affected by a declaration 1,400 lines later. The name is a
label produced from an interned id, not a location. Treating it as a location would have
sent the next reader to the wrong end of the file.
