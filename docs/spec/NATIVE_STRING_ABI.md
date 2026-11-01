# Native String ABI

> **Navigation**: [Spec](./README.md) | [Documentation Root](../README.md)

The contract a string-taking host native observes. This document specifies the string portion
of the native Application Binary Interface named as a cross-cutting concern in
[`../roadmap/V0_2_X_ROADMAP.md`](../roadmap/V0_2_X_ROADMAP.md). It does not specify the rest of
that boundary, and it does not specify the `Text` shared-slot kind, which is a different
construct.

The governing ruling and its provenance are recorded in
[`../decisions/STRING_ABI_OPTION_B.md`](../decisions/STRING_ABI_OPTION_B.md).

## 1. Scope

This document specifies what a native function observes when it is passed a value of the
surface type `Text`, under both embeddings of the language.

| embedding | where it lives | how a native is supplied |
|---|---|---|
| virtual machine | the shipping `keleusma` crate | `Vm::register_fn` and `Vm::register_fn_fallible` |
| ahead-of-time native | the `v0.3.0` line, `native_codegen/` | a symbol the host links, `kel_native_host__<name>` |

## 2. The agreed representation

A native observes a **borrowed, length-delimited view of the string's bytes**, valid for the
duration of the call and no longer.

Concretely, and identically under both embeddings:

1. The native is given the address of the first byte and the number of bytes.
2. The byte count is authoritative. The view is **not** terminated by a NUL byte, and a NUL
   byte within it is content rather than an end marker.
3. The bytes are the source literal's bytes, or the bytes the producing native supplied, with
   escape sequences already resolved to their single bytes.
4. The bytes are valid UTF-8.
5. The native does not own the bytes and must not retain the view past its return.

### 2.1 How each embedding expresses it

Under the virtual machine the view is a Rust `&str`. The host writes a native taking `&str`,
and `register_fn` supplies a borrow that lives for the call.

```rust
vm.register_fn("host::blen", |s: &str| -> i64 { s.len() as i64 });
```

Under the ahead-of-time native backend the view is the address of a constant block laid out as
`{ i64 len, [n+1 x i8] bytes }`, whose trailing NUL is a convenience for a host written in C
and is **not** part of the length. The host decodes the address into the same borrowed view.

```c
long long kel_native_host__blen(const struct { long long len; char b[]; } *s);
```

The two embeddings therefore agree on what the native observes. A body written against a
borrowed view compiles and behaves identically under both, which is the whole content of the
ruling.

### 2.2 Lifetime and the arena

A `Text` value is one of two runtime representations. A static literal borrows the module
image, which is immortal. A dynamic string borrows the arena. Both reach the native as the
same borrowed view, so a native cannot tell them apart and does not need to.

The borrow is bounded by the call because an arena-resident string is invalidated by the next
`resume` or `RESET`. This is the same use-before-`resume` discipline that already governs
every arena read at the host boundary. Under the virtual machine embedding the Rust lifetime
makes that discipline checked by the compiler rather than merely documented, which the owned
representation could not do.

A dynamic string whose arena region has already been reclaimed resolves to a clean
`VmError::TypeError` naming the staleness, rather than to a dangling read. This is a
default-deny boundary and a secure failure mode, not a recoverable condition.

## 3. The retained owned representation, and why it is not portable

A native may still be declared against an owned `alloc::string::String`. That path is
unchanged and continues to work exactly as before.

It is a **virtual-machine-only convenience and is not part of the agreed ABI.** No
ahead-of-time lowering can produce an owned `String` without allocating and copying, so a
native declared against `String` does not carry to the native embedding. A host that wants one
body to serve both embeddings declares the native against a borrowed view.

The owned path is retained rather than deprecated because deprecating it is an
embedder-visible decision the operator has not made. This document records the position; it
does not create a deprecation.

## 4. Argument positions and arity

A borrowed string argument is admitted in **any** argument position, at every arity the
marshalling layer supports, which is zero through four. Every combination of borrowed and
owned slots is implemented.

The implementation enumerates the combinations explicitly rather than deriving them. Routing
the slot types through an associated type would make the argument shape unrecoverable from a
closure's signature, because an associated type is not injective, and registration would stop
inferring. That was measured before the family was written rather than assumed.

The slot kinds are further wrapped in distinct marker types so that no substitution makes two
shapes equal. Written as bare types, the shapes `(&'static str, B)` and `(A, &'static str)`
unify at `A = B = &'static str`, and the coherence check reports an overlap it cannot
discharge.

## 5. What is not specified here

- **The `Text` shared-slot kind.** A shared-data slot of kind `Text` is a fixed-size handle of
  `2 * word_bytes`, not a literal. Settling the literal ABI does not settle it.
- **A borrowed string field inside a composite argument.** A struct or enum field of type
  `Text` decodes to an owned `String` through the derive macro. Making such a field borrowed
  requires lifetimes on the derived type, which is not part of this specification.
- **Every other native ABI question.** Floats, `Fixed`, `Opaque`, and `Unit` are separate
  items, and the float entry ABI is ruled on the `v0.3.0` line rather than here.

## 6. Verification

Because the two embeddings live on different branches, no single test observes both. Agreement
is established as the conjunction of two one-sided pins over the same four observable
properties. That is weaker than a differential oracle and is stated as such.

| property | pinned here | pinned on the native line |
|---|---|---|
| length-delimited, not NUL-delimited | `an_interior_nul_is_not_truncated` | `an_interior_nul_is_not_truncated` |
| byte length, not character count | `multibyte_utf8_is_counted_in_bytes_not_characters` | `multibyte_utf8_survives` |
| empty is a live view of length zero | `an_empty_literal_is_a_live_view_of_length_zero` | `an_empty_literal_is_length_zero` |
| borrowed and owned carry identical bytes | `the_borrowed_view_and_the_owned_copy_observe_the_same_bytes` | not applicable |

The tests on this line are in [`../../tests/string_abi_borrowed.rs`](../../tests/string_abi_borrowed.rs).
The tests on the native line are in that line's `native_codegen/tests/native_calls.rs`.

Property three of section 2 depends on the lexer baking a literal's source bytes. That was not
true when this specification was written. The lexer scanned bytes and pushed each as a Unicode
scalar, so every byte of a multi-byte character was re-encoded and a six-byte literal became
eleven bytes of well-formed but incorrect text. It is fixed, and pinned by
`a_string_literal_keeps_the_source_bytes_of_a_multibyte_character` in `src/lexer.rs`.
