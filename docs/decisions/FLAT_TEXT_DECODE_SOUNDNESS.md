# Flat Text decode soundness

> **Navigation** [Decisions](./README.md) | [Runtime faults](../spec/RUNTIME_FAULTS.md)

## Finding

The pykeleusma line reported on 2026-10-25, against keleusma 0.2.2, that a
program both `compile` and `Vm::new` accept could make the safe `Vm::decode`
read an integer as a pointer and end the process with SIGSEGV. V0.2.X
reproduced it on v0.2.3 at `ec536b28` the same day. This is undefined behaviour
reached through safe interfaces, which the operator ruling of 2026-10-20
rejects.

Two defects combined.

1. **The type checker did not check a yield.** `Expr::Yield` applied the
   negative-label rule to the yielded value and never unified its type with the
   declared return type. The body tail and `return` were checked. A yield of
   `(Word, Text)` in a function declared `-> (Text, Word)` was therefore
   admitted. The two tuples have the same flat size, so no later length check
   noticed.
2. **`Vm::decode` trusted a flat `Text` field's two words.** It rebuilt them as
   a pointer and a length through `KString::from_raw_parts` without checking
   them. Any mismatch between the body's layout and the host's chosen type
   therefore read a scalar as an address. The second defect needs no type
   checker gap. Decoding a well-typed `(Word, Text)` body as `(String, i64)`
   crashed the same way.

The typed verifier cannot catch either case. Its abstract value for a flat
composite records the kind and byte size and not the field layout, so
`(Word, Text)` and `(Text, Word)` are the same value to it.

## Repair

1. A yielded value must have the declared return type, with the label handling
   the body tail check uses. The message is "yielded value has type T but the
   function returns R".
2. A flat `Text` field read at the host boundary is checked by
   `marshall::checked_flat_text` before any `str` is built. Its bytes must lie
   wholly inside the VM's arena buffer or the VM's bytecode image, which are
   the only places a flat `Text` field points to, and must be valid UTF-8. Both
   regions are initialised memory the VM owns, so reading them is defined. A
   failed check is a `TypeError`. The epoch check for a stale ephemeral
   reference is unchanged.
3. `RefContext` and `NativeCtx` carry the bytecode image as a new public field
   `rodata`. A context built without the image passes an empty slice, which
   rejects constant strings instead of trusting them. This is a source-breaking
   change for code that builds either struct with a literal.

Host decode already copied the string, so the UTF-8 check adds a constant factor
to a cost that was already proportional to the length.

## Evidence

`tests/flat_text_decode_soundness.rs` holds the reported program and three
other mistyped yields, well-typed controls, the mismatched-host-type decode,
and must-succeed decodes of a constant string and of a host-passed string. With
the type checker change reverted, the rejection test fails, so it fires on the
defect. The unit tests `marshall::flat_text_tests` cover range edges, overflow,
an allocation the VM does not own, invalid UTF-8 and a split multi-byte
character, in configurations with and without floats.

The full workspace suite in release passed 3512 tests with 2 skipped on
2026-10-25. `--no-default-features` passed 338 and `--features signatures,shell`
passed 2517. Remote continuous integration covers the remaining configurations.

## Residual limits

- **VM-internal field reads.** `read_flat_scalar` rebuilds a `Text` field the
  same way during `GetField`, `GetIndex` and the related reads. The compiler
  bakes the correct kinds, so compiled code does not reach it with a wrong
  layout. A hand-built module the typed verifier admits could, because that
  pass does not know field layouts. A range check there is constant cost. A
  UTF-8 check would make that read's time bound depend on the string's length.
  This needs an operator ruling between a layout-aware typed pass, a range check
  alone, or a recorded trust assumption.
- **A resume value inside a composite literal is refused.** On 2026-10-25
  `loop main(e: Word) -> (Text, Word) { let n = yield ("send", e); ("x", n) }`
  failed verification with `NewCompositeSizeMismatch`, both before and after
  this repair. It is conservative, a refusal of a valid program. The type
  checker gives the resume value a fresh type, which is a likely but unverified
  cause.
- **The published 0.2.2 crate keeps both defects.** A patch release is an
  outward-facing action that needs operator authorization.
