# BRIEF — every panic-capable site that encodes a module assumption

**Line**: V0.3.X native code generation. **Drafted**: 2026-09-12.

## Why, and the evidence that the class is live

The previous increment found an `assert_eq!` in the emitter reachable from a module that
`lower_module` accepts. **`lower_module` is a public entry point that does not require a verified
module**, and this package has already converted 58 panics on it into refusals.

**That one was found by hand.** The sweep was green because its mutations change opcodes and the
defect needed a jump target moved. The deliberate version asks: **which other panic-capable sites
encode an assumption about the MODULE, and can a malformed module reach them?**

## The population, measured

| construct | `src/lib.rs` | note |
|---|---|---|
| `assert!` / `assert_eq!` | 8 | each encodes an assumption |
| `panic!` / `unreachable!` | 7 | same |
| `.expect(` | 46 | **mixed** — some name module properties, most name inkwell calls that cannot fail |
| `.unwrap()` | 208 | overwhelmingly inkwell builder results |

**The census must not swallow the 208.** A table nobody maintains is worse than none, and this package
has already rejected one widening on exactly that ground — the premise census measured that going
broad took it from 29 rows to 132 and declined.

So the scope is: **assertions, explicit panics, and the `expect`s whose message names a property of
the module rather than of the compiler's own API.**

## The wrong turns

1. **Do not classify by construct.** `.expect("a stream declares its resume parameter")` is a claim
   about the module; `.expect("the builder is positioned")` is not. The MESSAGE decides, not the
   method.
2. **Do not assume unreachable because the sweep is green.** That reasoning was wrong one increment
   ago and the sweep's blind spot was structural, not accidental.
3. **Do not convert every site to a refusal.** Some assumptions are genuinely internal — established
   by the emitter's own preceding code — and turning those into `Result` noise costs clarity and buys
   nothing. The disposition is the deliverable, not a blanket rewrite.
4. **Do not report a count as the result.** The premise census earns its keep by dispositioning each
   entry by what a false premise would COST. A panic census must do the same: what does a caller see
   if this fires?
5. **Do not touch the repository-root `src/` or `tests/`.**

## What done looks like

Every panic-capable site encoding a module assumption is enumerated and dispositioned by whether a
malformed module can reach it and what a caller would see; the enumeration fails when a new one
appears; anything found reachable is either refused or recorded as reachable-and-accepted with a
reason; and the scope exclusion — the 208 infallible unwraps — is stated rather than silently applied.


---

## OUTCOME — 2026-09-12

**22 sites in six families, and no live defect.** The one that was live was fixed the previous
increment; this census is what makes the next one visible.

| family | sites | disposition |
|---|---|---|
| the three trailing pointers are present | 11 | agreement by construction, **and two routes tested**: a data op with no layout, and a `NewComposite` inserted after compilation. Both refuse before the site. |
| a chunk or native returns a value, never void | 3 | about LLVM types this emitter declared |
| a general stream has a loop top and a resume parameter | 3 | `param_count != 1` is refused earlier |
| internal, established by the preceding statement | 2 | — |
| `PRIVATE_SLOT_BYTES` is a power of two | 2 | a `const` |
| the visit vector is parallel to the chunk table | 1 | a `debug_assert_eq!`, so it **panics in the configuration this suite runs** |

### The matcher caught what the probe missed, and the arithmetic caught the rest

A scratch probe reported 21 sites. The committed matcher reports **22**. The difference is
`debug_assert_eq!`: the probe's regex used `\bassert_eq!`, and a word boundary does not match inside
`debug_assert_eq!`. **A debug assertion is panic-capable in exactly the build this suite runs**, so the
probe's blind spot was over the most easily overlooked family of all.

And the first draft's family table summed to **23** against a count of **21** — neither correct.

> **An arithmetic that does not close is the cheapest available signal that a population was not
> actually read.** It fired twice here, once against the probe and once against the table, before
> anything was filed.

### The exclusion is enforced, not just declared

The file carries 208 `.unwrap()` calls on infallible inkwell results. The census asserts its
vocabulary filter keeps **fewer than half** of the panic-capable lines it sees, so a filter that
started admitting them would fail rather than quietly produce a table nobody maintains.
