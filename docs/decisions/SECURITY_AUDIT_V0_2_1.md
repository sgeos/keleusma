# Security and Soundness Audit (V0.2.1)

> **Navigation**: [Decisions](./README.md) | [Documentation Root](../README.md)

Record of an external adversarial code review of the Keleusma verifiable
control kernel, conducted during V0.2.1 development. This document preserves the
findings so they can be tracked and remediated. It is a record, not a set of
applied fixes.

## Provenance and method

The review was an adversarial multi-agent static analysis. Fifteen module
reviewers raised thirty-two findings; each was handed to an independent skeptic
agent that reopened the cited code and attempted to refute it. Two findings were
refuted and discarded. Thirty findings survived.

The review is static. The agents read source and reasoned about reachability;
they did not compile the crate or run the test suite. Confidence markers are the
skeptic verdicts, not certainties of exploitability. Line numbers are as of the
audited tree and may drift as the code changes.

## Handling status

Remediation is **deferred until the B28 flat-byte runtime work is complete**, by
operator decision. This document is the durable record; the fixes are scheduled
work, not yet started.

Two reproduction tests left by the audit, `tests/poc_newarray_underflow.rs`
(finding 7) and `tests/zz_call_underflow_repro.rs` (findings 4 and 16), together
with the probes `poc_const_oob_accepted_by_verifier` and
`poc_wordtofixed_overshift_accepted_by_verifier`, currently live on the
`feat-flat-memory-model` flat-byte feature branch rather than on `v0.2.1`. They
are diagnostic: two of them panic on the unfixed bugs, so the feature branch's
test suite is red until the bugs are addressed or the probes are marked. A
standing arrangement re-audits the delta against this baseline when the
implementation reaches a clean checkpoint.

### Remediation status (assessed 2026-06-29)

The "not yet started" note above is the original record and is now superseded.
B28 is complete, and remediation is under way on `feat-audit-remediation`. As of
the latest commits, the status is **30 fixed, 0 partial, 0 open**. All twelve High
findings are now fixed. Status was assessed by reading the cited code and the
`fix(audit)` commit record. The three memory-unsafety items the audit flagged for
dynamic confirmation (5, 8, 15) are fixed; the shebang/zero-copy fix is
additionally exercised under Miri (Tree Borrows) over the zero-copy path. The
flat-read cluster (6, 10, 11, 12, 14, 19, 20, 21) is closed by the scalar-codec
totality (`24df9dd`), the marshall/derive slice bounds-checks (`af1b381`), and the
verifier-completeness checks (`e35e816`); the gate is green on default, signatures,
and all-features, with `clippy --all-targets -D warnings` clean.

| # | Sev | Status | Note |
|---|-----|--------|------|
| 1 | High | Fixed | `21216d1` validates constant-pool indices in `verify_chunk` |
| 2 | High | Fixed | `e35e816` validates local-slot indices against `local_count` |
| 3 | High | Fixed | `87507ed` operand-stack-depth pass |
| 4 | High | Fixed | `21216d1` call-arity check plus `checked_sub` |
| 5 | High | Fixed | `397e512` makes the swap transactional (fallible steps before the drop) |
| 6 | High | Fixed | `e35e816` added the omitted local-slot and struct-template checks; the verifier-completeness set (depth, const, arity, locals, templates) is now in place |
| 7 | High | Fixed | `d9fd075` underflow guards in construct/call ops |
| 8 | High | Fixed | `38e4268` strips the shebang before validating and storing; Miri-clean |
| 9 | High | Fixed | `e30f7a3` enforces signing by host policy (non-empty trust matrix) |
| 10 | High | Fixed | `af1b381` `flat_subslice` bounds-checks every composite-body slice in the marshall decoders and the derive |
| 11 | High | Fixed | `24df9dd` codec total: an unverified offset/kind yields a clean `VmError`, not a panic (the verifier still does not statically validate flat offsets; safety is by codec totality) |
| 12 | High | Fixed | `24df9dd` `read_scalar_le` returns `ReferenceKind` for Text/Opaque instead of panicking |
| 13 | Med | Fixed | `e35e816` validates the boxed `NewComposite` template index |
| 14 | Med | Fixed | `24df9dd` codec total (the flat-tuple field read reaches a clean error, not a panic) |
| 15 | Med | Fixed | `38e4268` (same shebang strip as finding 8) |
| 16 | Med | Fixed | `checked_sub` in `Op::Call` |
| 17 | Med | Fixed | `e35e816` adds the local and template index validation it required |
| 18 | Med | Fixed | `checked_sub` in `Op::Call` |
| 19 | Med | Fixed | `24df9dd` codec total (offset/kind reach a clean error, not a panic) |
| 20 | Med | Fixed | `24df9dd` both return `UnsupportedWidth` on bad kinds/widths |
| 21 | Med | Fixed | `24df9dd` checked slicing returns `OutOfBounds` on short buffers |
| 22 | Low | Fixed | `2ec3d56` gates the safe construction/load family behind `verify`; no-verify builds use the explicit `unsafe` `*_unchecked` family |
| 23 | Low | Fixed | `5282209` `verify_module_signature` uses `verify_strict` |
| 24 | Low | Fixed | `5282209` rejects a non-contributory (low-order) ephemeral key |
| 25 | Low | Fixed | `0c7f5d5`/`ae87165` wrap and flatten `Some` so `Some(None)` no longer collapses (value path and native flat body); the residual nested-`Option` *match* failure is a separately-tracked language-level bug (reproducible with no native), not a marshalling gap |
| 26 | Low | Fixed | `5282209` recovers the origin from the `specs` map, not `split("__")` |
| 27 | Low | Fixed | `5282209` explicit `TYPE_SPECIALIZATION_LIMIT` on the struct/enum passes |
| 28 | Low | Fixed | `5282209` saturating size/offset arithmetic in `value_layout` |
| 29 | Info | Fixed | `5282209` `f64::from_value` errors above the ±2^53 safe-integer range |
| 30 | Info | Fixed | specialization-failure documentation corrected |

No High findings remain open. The done remediation items are the
operand-stack-depth pass (3, 4, 7, 16, 18), the operand-index validation
(1, 2, 13, 17), the transactional hot-swap (5), the signature host-policy (9), the
shebang/zero-copy reconciliation (8, 15), the scalar-codec totality (11, 12, 14,
19, 20, 21), the marshall/derive composite-body slice bounds-checks (10), and the
verifier-completeness checks that close finding 6 (locals and struct-template, on
top of the earlier depth/const/arity work), and the low-severity cleanup
(`verify_strict` 23, the contributory-key check 24, the monomorphizer origin
lookup 26 and specialization cap 27, the saturating layout arithmetic 28, and the
lossy-f64 guard 29), the safe-constructor gating (finding 22, which gates the whole
construction/load family behind `verify` and adds `load_signed_bytes_unchecked` and
`load_encrypted_signed_bytes_unchecked` for the no-verify signed-load path), and the
value-path and native flat-body Option wrapping (finding 25, which stops `Some(None)`
collapsing and makes a native-returned flat-eligible `Option` byte-identical to the
body a script constructs). All thirty findings are now resolved.

The finding-25 investigation surfaced one separate, newly-discovered bug that is **not**
a marshalling or memory-safety issue and is tracked for a follow-up: matching a nested
`Option` whose inner value is `None` fails with a clean `NoMatchingArm`, reproducible in
a pure script with no native involved, because the compiler lowers an `Option::None`
pattern to a scalar `Value::None` check that does not recognize an extracted flat
`[disc=0]` payload. It is a compiler/VM correctness concern (Option's dual scalar/flat
`None` representation not composing under flat nesting), pinned by the
`nested_option_match_is_a_language_limitation` test. Marshalling round-trips
host-to-host correctly regardless.

Pre-re-audit hardening added test coverage the fixes lacked: the finding-22
`load_signed_bytes_unchecked` / `load_encrypted_signed_bytes_unchecked` paths (signature
verification, wrong-key, finding-9 policy, decryption, wrong-recipient) and the
finding-26 underscored-origin specialization guard. The finding-10 flat-read
subslice and the finding-25 Option flat-body (`build_in_arena`) paths were
additionally exercised clean under Miri (Tree Borrows).

## Severity and category distribution

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 12 |
| Medium | 9 |
| Low | 7 |
| Info | 2 |

| Category | Count |
|----------|-------|
| Memory safety | 16 |
| Soundness | 9 |
| Security | 3 |
| Correctness | 1 |
| Quality | 1 |

## Root cause

One structural gap accounts for eight of the findings. The structural verifier
in `verify_chunk` validates block nesting, branch-target bounds, and data-slot
bounds, but it performs no operand-stack-depth analysis and does not validate
operand indices for the constant pool, local slots, struct templates, call
argument counts, or flat tuple field offsets. The virtual machine assumes all of
these were checked before execution; they are not. Bytecode that passes
`verify::verify` can therefore panic the machine, and in one case silently
corrupt another call frame. Findings 1, 2, 3, 4, 6, 7, 13, and 17 are instances
of this single gap.

A consequence for the marketed property follows directly. The worst-case
memory-usage analysis saturates at zero on underflow, which silently assumes a
never-underflowing operand stack that the verifier never proves, and the
worst-case budget `local_count + body_peak` assumes slot indices stay within
`local_count`, which the verifier never checks. The bounds framework is
analytically sound but conditional on invariants that are not enforced, so the
"safe loader admits only steppable bytecode" claim does not hold for the code as
written. The gap is one of completeness, not design; the remedies are
conventional verifier passes and defense-in-depth checks.

## Findings

Severity shown is the skeptic-corrected severity. Locations are as audited.

| # | Sev | Cat | Location | Title |
|---|-----|-----|----------|-------|
| 1 | High | mem-safety | `verify.rs:1723-1977` | Constant-pool indices unvalidated; `Op::Const`/`NewEnum`/`GetField` panic on out-of-range index |
| 2 | High | mem-safety | `verify.rs:1723-1977` | Local-slot indices unvalidated; `Op::SetLocal` enables OOB write / cross-frame corruption |
| 3 | High | soundness | `verify.rs:1697-2132` | No stack-discipline analysis; drain-based composite ops underflow-panic |
| 4 | High | mem-safety | `verify.rs:1697-2132` | `Op::Call` arg count unchecked vs callee frame size; integer underflow in dispatch |
| 5 | High | mem-safety | `vm.rs:2117-2185` | Hot-swap drops old private slots before fallible decode; double-drop / use-after-free |
| 6 | High | soundness | `verify.rs:1697-2133` | Verifier omits the slot/arg checks the `new_unchecked` safety doc relies on |
| 7 | High | mem-safety | `vm.rs:3385-3391,3466-3489` | `NewTuple`/`NewArray`/`NewEnum`/`Call` underflow the operand stack |
| 8 | High | mem-safety | `vm.rs:879-887,1432-1489` | Shebang desync feeds attacker offsets to `rkyv::access_unchecked` (UB) |
| 9 | High | security | `vm.rs:1244-1269,1983-1995` | Signature check gated on attacker-controlled flag bit; clearing it skips verification |
| 10 | High | mem-safety | `marshall.rs:262-282` | Flat-tuple marshalling indexes script bytes at host offsets with no length check |
| 11 | High | mem-safety | `vm.rs:3562-3573` | `Op::GetTupleField` flat operand unverified; offset/kind reaches `read_scalar_le` panic |
| 12 | High | mem-safety | `bytecode.rs:487-489` | `read_scalar_le` panics on Text/Opaque kinds the wire decoder accepts |
| 13 | Med | mem-safety | `verify.rs:1723-1977` | Struct-template indices unvalidated; `Op::NewStruct` panics on out-of-range index |
| 14 | Med | mem-safety | `vm.rs:3562-3573` | Flat-tuple field read uses unverified byte offset; panics on out-of-range/reference kinds |
| 15 | Med | mem-safety | `vm.rs:879-887,1479-1489` | Zero-copy decode cache strips shebang but `archived()` does not; mis-locates rkyv aux body |
| 16 | Med | soundness | `vm.rs:3385-3403` | `Op::Call` `new_base`/`extra` via unchecked usize subtraction |
| 17 | Med | mem-safety | `vm.rs:905-907,910-920,3007-3017,3438-3441` | Hot-path handlers index cache/constants/locals/templates with no bounds check |
| 18 | Med | correctness | `vm.rs:3385-3402` | Unchecked subtraction in `Op::Call` can wrap to a wild frame base |
| 19 | Med | mem-safety | `bytecode.rs:444-464` | `GetTupleField(Flat)` offset/kind never bounds-checked; drives OOB slice |
| 20 | Med | mem-safety | `bytecode.rs:419-422,478-483` | `read_scalar_le`/`write_scalar_le` panic on reference/composite kinds and bad float widths |
| 21 | Med | mem-safety | `bytecode.rs:447-491` | `read_scalar_le` panics on short buffers; aborts through marshall flat-tuple path |
| 22 | Low | soundness | `vm.rs:1110-1140` | Safe `Vm::new` skips all verification when `verify` feature is off, via a non-unsafe API |
| 23 | Low | security | `wire_format.rs:1539,1577-1582` | Ed25519 uses `verify()` not `verify_strict()`; admits signature malleability |
| 24 | Low | security | `encryption.rs:260-317` | Public `decrypt_from_metadata` accepts attacker ephemeral key; no contributory check |
| 25 | Low | soundness | `marshall.rs:175-199` | Option marshalling collapses `Some(None)`; nested Option round-trips are lossy |
| 26 | Low | soundness | `monomorphize.rs:142-214` | Polymorphic-recursion guard miscounts origins via `split("__")` |
| 27 | Low | soundness | `monomorphize.rs:265-544` | Generic struct/enum specialization passes have no explicit count bound |
| 28 | Low | soundness | `value_layout.rs:220-300` | Composite size/offset arithmetic uses unchecked usize multiply/sum (latent) |
| 29 | Info | soundness | `marshall.rs:116-126` | `f64::from_value` silently coerces Int to float; lossy above 2^53 |
| 30 | Info | quality | `monomorphize.rs:150-161` | Inaccurate failure-mode documentation for the specialization bail-out |

## High-severity detail

**1. Constant-pool indices unvalidated.** The verifier Pass-1 loop matches only
control-flow and data-slot ops; `Op::Const`, `Op::NewEnum`, `Op::GetField`,
`Op::IsEnum`, and `Op::IsStruct` fall through, so their constant-pool index
operands are never checked against `chunk.constants.len()`. The VM dereferences
them directly. `Op::Const(65535)` on a one-entry pool passes `verify()` and then
panics. The operand is a `u16`.

**2. Local-slot indices unvalidated.** No arm validates `Op::GetLocal`/`SetLocal`
against `chunk.local_count`, in contrast to `GetData`/`SetData` which do guard.
When `slot` exceeds `local_count` but `base + slot` is within the live stack,
`SetLocal` writes into another call frame without panicking. This is silent
intra-arena corruption from verified bytecode and invalidates the
`local_count + body_peak` memory budget.

**3. No stack-discipline analysis.** The verifier never tracks operand-stack
depth. `Op::NewArray(65535)` with too few operands passes and panics on the
`drain`. `NewStruct` guards with a depth check; `NewArray`/`NewTuple`/`NewEnum`
do not, because the design assumed the verifier proved depth. Remedy: a forward
abstract-interpretation pass computing per-op operand depth from the existing
`stack_growth`/`stack_shrink` tables, requiring balance at control-flow joins and
loop back-edges. This also discharges the precondition the memory analysis
assumes.

**4. Call argument count unchecked.** `Op::Call(callee, 5)` against a callee with
`local_count` 0 computes `extra = 0 - 5`, which panics in debug or wraps near
`usize::MAX` in release, after which the push loop exhausts the arena. The
in-stream call path has no arity check; the entry-call path does, which shows the
invariant was intended.

**5. Hot-swap double-drop / use-after-free.** `replace_module_inner`
unconditionally drops every old private slot via `core::ptr::drop_in_place`
before the fallible `to_bytes()` and `decode_all_ops()` steps, but updates
`private_slot_count` only on success. On error the count still equals the old
count while the slot memory holds logically dropped values, so the VM `Drop`
re-drops them. Genuine host-process memory unsafety on the marketed hot-swap
path. Remedy: make the swap transactional, building and validating the new state
before dropping the old.

**7. Composite constructors underflow the operand stack.** Runtime counterpart to
finding 3. `NewArray`/`NewTuple`/`NewEnum`/`Call` compute `drain` ranges and frame
bases by raw subtraction with no depth guard, while `NewStruct` and `PopN` guard.
Remedy: explicit guards plus the verifier depth pass.

**8. Shebang desync reaches `rkyv::access_unchecked`.** All wire validation runs
against a shebang-stripped slice, but the zero-copy constructor
`view_bytes_zero_copy` stores the raw unstripped slice. `archived()` then reads
the aux-body offset and length from the unstripped bytes and calls
`access_unchecked` over a mis-located region. Because the archived type contains
relative-pointer `Vec` and `String` fields, this is undefined behavior reachable
from a documented, supported input form.

**9. Signature check gated on an attacker-controlled flag.** `load_signed_bytes`
and `replace_module_from_bytes` gate Ed25519 verification on
`header_requires_signature`, which reads bit `0x02` of byte 15 directly from
untrusted bytecode. An adversary clears the bit, recomputes the four-byte CRC32
trailer, and the runtime loads arbitrary bytecode with the signature never
checked. The command-line frontend is saved by a separate strict-mode gate that
library embedders do not inherit. Remedy: make enforcement a property of host
policy, rejecting unsigned modules whenever the trust matrix is non-empty, rather
than trusting a self-asserted flag.

**10, 11, 12. Flat-tuple and scalar-read panics.** `read_scalar_le` performs
unchecked slice indexing and panics outright on Text and Opaque scalar kinds,
which the wire decoder accepts. The flat-tuple marshaller reads host-derived
field widths against a script-controlled buffer with no length check.
`Op::GetTupleField` passes an attacker offset and kind straight into
`read_scalar_le`. Common remedy: make `read_scalar_le` return a `Result` with
checked slicing, and validate flat offsets at the verifier.

## Security assessment

- Finding 9 is the most serious security item. Authenticity is gated on an
  attacker-mutable flag rather than host policy.
- Findings 8 and 15 reach `rkyv::access_unchecked` over attacker-influenced
  offsets, which is undefined behavior. The inconsistency is internal to the
  zero-copy constructor, so a valid but shebang-prefixed module breaks rather
  than being rejected.
- Findings 23 and 24 are cryptographic hygiene: non-strict Ed25519 admitting
  malleability, and an ephemeral-key path with no contributory check. Both are
  low severity in the current wire path, where signature verification precedes
  decryption, but worth correcting for defense in depth.

## Prioritized remediation

1. Add the operand-stack-depth abstract-interpretation pass to `verify_chunk`.
   Closes findings 3, 4, 7, 16, 18, and discharges the memory-bound precondition.
2. Add verifier operand-index validation for the constant pool, local slots,
   struct templates, and flat tuple offsets. Closes findings 1, 2, 13, 14, 17, 19.
3. Make `read_scalar_le` and `write_scalar_le` total by returning `Result` with
   checked slicing. Closes findings 10, 11, 12, 20, 21.
4. Make hot-swap transactional to remove the double-drop. Closes finding 5.
5. Make signature enforcement a host-policy decision and reconcile the shebang
   handling in the zero-copy path. Closes findings 8, 9, 15.
6. Replace unchecked subtraction in `Op::Call` with `checked_sub`. Hardens 16, 18.
7. Address the lower-severity soundness, cryptographic hygiene, and documentation
   items, findings 22 through 30, as a cleanup pass.

Two verifier passes (items 1 and 2) plus making `read_scalar_le` total (item 3)
close roughly twenty of the thirty findings.

## Coverage note

No finding was a test gap, but the conclusion follows from the findings: the
existing suite validates correct programs and round-trips and does not exercise
the adversarial rejection paths the high-assurance claim depends on. The most
valuable additions are a hostile-bytecode corpus driven through the full safe
`Vm::new`, a wire-format fuzz target, a signature-bypass regression that clears
the flag and recomputes the CRC, and a sanitizer run of the hot-swap failure
path for finding 5.

## Caveats

The review is static, so reachability is reasoned rather than executed. A dynamic
pass, `cargo test`, `cargo clippy`, and a Miri or sanitizer run on the hot-swap
path, would corroborate the three memory-unsafety findings (5, 8, 15) before
they are treated as confirmed exploitable.
