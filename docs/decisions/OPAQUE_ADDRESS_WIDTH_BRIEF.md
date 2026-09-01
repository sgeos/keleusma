# Brief: size `Opaque` by the address width, and finish the job this time

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

## What this is

`ScalarKind::Opaque` is a handle to a host-managed reference. It was sized by the **word** width,
which is wrong: a pointer-like handle is an address, and on any target where the two widths differ
the flat layout reserved the wrong number of bytes. The operator confirmed the ruling on
2026-08-31.

## Why it is red, and what that cost

The change adds a third parameter to `size_in_bytes`, which touches roughly sixty call sites and
eleven signatures. A previous session recorded it as compiling against "library, derive macro and
every test target". **It did not.** Two of the three feature sets continuous integration runs fail
it, and the failures are six instances of the same omission.

- Five under `self-host`, in the driver's typed-verifier helpers.
- One under `signatures,shell`, in a test-only `NativeCtx` literal.

The second is the instructive one. `cargo check --features signatures,shell` **passes**, because
the site is inside a test module. Only `--tests` reveals it. A per-feature sweep that varies the
feature set but not the target selection reports the branch clean.

## The specific wrong turn to avoid

**Do not choose each call site's width by what makes it compile.** `word_bytes` and `addr_bytes`
are both `8` on this development host, so a wrong choice passes every check available here and is
wrong only on a 32-bit or 16-bit target, where nothing routinely runs.

Choose by what the value *is*. The three widths derive from the module header by the same rule, so
a helper that already receives the word and float widths should receive the address width from the
same place rather than having one invented at the call site.

## Why it is worth landing now specifically

The format fingerprint hashes the scalar size table at pairwise-distinct reference widths. This
change moves `Opaque` from one of those widths to another, so **the fingerprint must move when this
lands.** That is the mechanism's first evidence against a real change rather than a synthetic one.

If the fingerprint does *not* move when this lands, the fingerprint is broken, not this change.
That is a genuine test and it only works in this order.

## What is not in scope

Bundling the three widths into a single value. It would make the next width addition nearly free
and this is the cheapest moment to do it, but it is a redesign in the middle of a repair, which is
how correctness is lost. Recorded as a deliberate omission, not an oversight.
