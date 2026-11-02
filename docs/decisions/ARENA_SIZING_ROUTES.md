# Arena sizing by route

> **Navigation** [Decisions](./README.md) | [The native region formula](./NATIVE_REGION_SUPPLEMENT_FORMULA.md)

**Status.** Implemented on 2026-11-01 on `fix/v02-arena-sizing-split`, on the operator's
ruling quoted below. This record states which function each kind of host calls, why
`auto_arena_capacity_for` changed meaning, and the measurements behind the change.

## The ruling

The operator ruled, verbatim, "The goal of `auto_arena_capacity_for` is to right-size an arena
automatically. If the current implementation does not do this, then fix it so it does. This
might mean you need another function or two." The ruling reached this line first-hand in the
V0.2.X session and is recorded in `docs/process/REVERSE_PROMPT.md` under the concerns of that
session.

The ruling on question 22 of 2026-10-31 had folded the native region supplement into
`auto_arena_capacity_for`, so that one figure served both routes. That made the figure loose for
a host that runs only the virtual machine, and it gave a host that runs only native code a figure
made mostly of terms it does not use.

## The functions

| Function | Host | Terms |
|---|---|---|
| `keleusma::vm::auto_arena_capacity_for` | Runs the virtual machine | Operand stack, call frames, opaque registry, per-iteration heap, entry host inputs, seeded private composite initializers |
| `keleusma::vm::auto_arena_capacity_with_input_text` | Runs the virtual machine and passes composites carrying `Text` | As above, with the host's text allowance per input |
| `keleusma::vm::vm_arena_terms_for` | Audits the figure | The same terms one by one, as `VmArenaTerms` |
| `keleusma::vm::auto_arena_capacity_for_vm_and_native` | Keeps one arena for either route | `auto_arena_capacity_for` plus the native region, once |
| `keleusma::native_region::native_route_buffers_for` | Runs only native code | The native region, the private buffer and the shared buffer, as `NativeRouteBuffers` |

None of the virtual machine functions includes the persistent region, which
`required_persistent_capacity_for` sizes, as before.

The native route returns a structure rather than a single capacity. A host therefore cannot pass
it where a virtual machine capacity is expected, and the name says which route it serves. Its
private buffer is `required_persistent_capacity_for` plus `native_persistent_supplement_bytes`,
the backend's one word per `Stream` chunk and one word per private composite pool slot. Its
shared buffer is `shared_data_bytes_for`. The backend's frame slot and machine stack are not
reported, because only the backend computes them, and a host linking its static library reads
them from the library header.

## The choice between the two options

Two options were open. Option (a) makes `auto_arena_capacity_for` the virtual machine's figure
and adds a native-route function. Option (b) leaves it combined and adds two functions.

**Option (a) is implemented.** The function's name and its every caller in this tree, the
command-line tool, the task scheduler, the examples and the book, size a virtual machine. Under
option (b) the function the documentation tells a host to call would remain the one that is not
right-sized, against the ruling.

**The change is host-observable, and it cannot leave a host short.** A host that runs the virtual
machine now gets a smaller figure, which is the figure that host needs. A host that relied on the
combined figure for native code switches to `auto_arena_capacity_for_vm_and_native`, whose value
is never below the former combined value. A host that relied on it and does not switch, and then
runs native code in an arena of the virtual machine's figure, would be short by the region. That
is the one silent breakage the change admits. It is inferred, not observed, because no host in
this tree does it. The changelog fragment states the change so that such a host can find it.

## Measurements

Measured on 2026-11-01 at `7c76d771` plus this change, by driving each module through `Vm::new`,
`call` and up to 200 resumes with scalar inputs in an arena of the figure, and reading the
arena's bottom and top high-water marks. The corpus is `examples/scripts`, leaving out
`11_signed.kel`, which must be loaded through the signed path, and `src/hosted/interp.kel`. The
column "Before" is the virtual machine's part of the former figure, the former value minus the
native region.

| Module | Before | Now | Measured peak | Verdict for the virtual machine |
|---|---|---|---|---|
| `01_arithmetic.kel` | 336 | 336 | 336 | Right-sized and tight |
| `02_struct_field.kel` | 224 | 224 | 224 | Right-sized and tight |
| `03_enum_match.kel` | 488 | 488 | 488 | Right-sized and tight |
| `04_for_in.kel` | 600 | 600 | 600 | Right-sized and tight |
| `05_pipeline.kel` | 208 | 208 | 208 | Right-sized and tight |
| `06_multiheaded.kel` | 336 | 336 | 336 | Right-sized and tight |
| `07_refinement.kel` | 328 | 328 | 328 | Right-sized and tight |
| `08_method_dispatch.kel` | 208 | 208 | 208 | Right-sized and tight |
| `09_big_numbers.kel` | 720 | 720 | 656 | Right-sized, 64 bytes of verifier heap slack |
| `10_multbyte.kel` | 1616 | 1616 | 1552 | Right-sized, 64 bytes of verifier heap slack |
| `12_sensor_window.kel` | 712 | 712 | 712 | Right-sized and tight |
| `13_telemetry_stream.kel` | 664 | 664 | 664 | Right-sized and tight |
| `14_frame_log.kel` | 656 | 680 | 680 | **Short by 24 before**, now right-sized and tight |
| `15_pixel_blend.kel` | 688 | 688 | 688 | Right-sized and tight |
| `interp.kel` | 15800 | 15800 | 1976 | Equal to the verifier's bound, loose against the driven paths |

**The former figure was short for the virtual machine on one module.** `14_frame_log.kel` has a
private `Frame` slot. Construction copies the slot's zero initializer through the arena top
region into the persistent pool, and the copy is not reclaimed until the first `Reset`. Measured,
the top region held 24 bytes after `Vm::new`, and the first call failed with `OutOfArena` in an
arena of the former virtual machine figure. The combined figure hid this, because the native
region it added was larger than the shortfall. The new term `seeded_private_bytes` counts each
seeded initializer's pool extent and its static string bytes, and `seeding_transient_bytes`
charges the momentary packing of an initializer that nests a composite. With them the figure is
680, the smallest arena that ran, and 679 bytes fail.

**Where the figure is loose, it is the verifier's bound.** On `09_big_numbers.kel` and
`10_multbyte.kel` the heap bound exceeds the measured top peak by 64 bytes. On `interp.kel` the
heap bound is 13824 bytes and the driven run used none of it. The drive feeds fixed inputs, so it
reaches some paths and not others, and whether any path attains the bound is not verified. The
figure adds nothing beyond the verifier's terms, which `the_vm_figure_is_the_sum_of_the_verifier_terms`
checks term by term.

**One term is conservative by construction.** The seeding transient charges a nesting
initializer its whole extent. Measured on a private `Outer { inner: Inner, c: Word }`, the packing
peak is the parent's 24 bytes and the child's 16, so the smallest arena that ran was 224 bytes
against a figure of 232. Charging the exact child bodies needs the flat size of each nested
constant, which the module does not record, and the 8 bytes are the parent's own scalar field.

## The native route against V0.3.X

V0.3.X sent five figures on 2026-10-31 for a host that runs only native code. All five are
reproduced in `tests/native_region_supplement.rs`.

| Module | Region | Private | Shared | Virtual machine only, V0.3.X | Now |
|---|---|---|---|---|---|
| `02_struct_field.kel` | 16 | 0 | 0 | 224 | 224 |
| `10_multbyte.kel` | 192 | 0 | 0 | 1616 | 1616 |
| `12_sensor_window.kel` | 80 | 0 | 0 | 712 | 712 |
| `13_telemetry_stream.kel` | 720 | 8 | 0 | 664 | 664 |
| `interp.kel` | 4032 | 80072 | 162832 | 15800 | 15800 |

The private figure of `14_frame_log.kel`, 104 bytes, is derived from the backend's formula and
was not measured by V0.3.X.

## Failure modes

- The virtual machine functions fail with `VmError::VerifyError` when the verifier cannot bound
  the footprint or the resumes before a `Reset`, as before. A figure beyond `usize::MAX`
  saturates, and no allocation satisfies it.
- The native route does not fail. It does not check that the backend lowers the module, so a
  module the backend refuses gets figures that size nothing. Its formulas are copies of the
  backend's, which stays the authority.
- The figures assume the bundled `i64` and `f64` runtime and, for the native route, the 64-bit
  host target the backend measured.

## Open question

The seeded copy is dead once it reaches the pool. Construction could reclaim it, which would make
the seeded term unnecessary and lower the figure of every module with a private composite. That
changes when the arena epoch advances at construction and at a hot swap, so it is left to a later
increment and not decided here.
