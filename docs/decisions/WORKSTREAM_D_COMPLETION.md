# Workstream D completion requirements

Workstream D in the [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md) covers
static-library linkage, the host application binary interface (ABI), and object
files that link into foreign projects. Order 3 is this workstream alone, under
the gate "a native static library links against a Rust host and runs a real
program". The operator set completing Order 3 as this line's goal on
2026-10-24. No earlier record scoped Workstream D, so the requirements below are
taken from the roadmap's own list. Each is marked implemented, met in part, or
deferred, with the reason.

## The gate

**Met on 2026-10-25.** `keleusma_native::linkage` lowers the self-hosted lexer,
`src/selfhost/kel/lexer.kel`, as a coroutine at its static frame bound of 1216
bytes. The test archives the object as `libkel_lexer.a` with the platform `ar`.
It then compiles
[`examples/lexer_host/host.rs`](../../native_codegen/examples/lexer_host/host.rs)
with `rustc` against the generated bindings, and runs the binary as a separate
process. Three lexer instances in one host-reserved pool lex three different
sources over 300 interleaved turns. Every yield and every byte of each
555,024-byte shared buffer equal the virtual machine's.
`a_rust_host_links_the_lexer_static_library_and_agrees_with_the_vm` in
[`static_library_linkage.rs`](../../native_codegen/tests/static_library_linkage.rs)
is the evidence. It also requires at least eight distinct token codes per
instance and different yields for different sources, so an agreement on empty
output cannot pass.

## Acceptance matrix

Tests are in
[`static_library_linkage.rs`](../../native_codegen/tests/static_library_linkage.rs)
unless another file is named.

| Requirement | Executable evidence | Disposition |
|---|---|---|
| Links as a static library against a Rust host | The gate test above | Implemented |
| Exposes sub-coroutine entry points | The object exports `<prefix>_start`, `<prefix>_resume` and `<prefix>_release`, and `<prefix>_yield_site` under dialogue contracts. The signatures are those `coroutine.rs` documents | Implemented |
| The ABI is specified, including the native marshalling boundary | `coroutine.rs` states the handle signatures, the buffer order, the slot lifetime and the reply encoding. `linkage.rs` states the exported constants. The generated header and bindings render both | Implemented for scalars and flat bodies. Text and opaque values remain refused, as Workstream B records |
| Links into a project in another language | `a_c_host_links_the_same_archive_and_agrees_with_the_vm` builds a C host from the generated header with `-Wall -Werror` and compares its yields with the virtual machine | Met in part. C and Rust are exercised. Ada is not |
| A stable C ABI, freestanding | `the_archive_exports_exactly_the_api_and_the_contract` reads the archive with `nm`. Its external definitions are exactly the handles and constants, and every import is a host native or a C library routine. The lexer imports nothing. No Keleusma runtime or virtual machine is linked | Implemented |
| Symbol mangling fixed per the V0.4.0 resolution | The prefix is the whole scheme. `malformed_prefixes_are_refused` refuses a prefix that is not a C identifier, is too long, or lies in the native namespace | Deferred. The recommended mangling encodes module path, function and type arguments, and matters once objects reference each other. No object does yet |
| Two libraries in one program | `two_libraries_link_into_one_rust_host_only_after_export` links two programs whose entries share chunk index zero. The unexported objects fail to link on `kel_coroutine_0_start`. Exported under two prefixes they link, run and agree with the virtual machine | Implemented |
| The arena requirement exported as a symbol | `<prefix>_frame_bytes`, `<prefix>_slot_bytes`, `<prefix>_buffer_align`, `<prefix>_shared_bytes`, `<prefix>_private_bytes`, `<prefix>_region_bytes` and the private image. `the_generated_contract_agrees_with_the_module` checks the frame figure against `minimum_frame_bytes` | Implemented |
| The trap-free verdict exported | `<prefix>_trap_verdict` encodes the first-pass classification as 0 not established, 1 trap free, 2 may trap. The same test checks it against `classify_traps`. The lexer reports 2, because it contains an explicit `Trap` | Implemented |
| The WCET bound exported | None | Deferred to Workstream E. See below |
| The shared region as a C-representable layout | The header and bindings give each shared field an offset, an element size and a count. An array is one field. The gate test seeds `src.len` and `src.bytes` through them, and the reference reads the same offsets from the module independently | Implemented, with the region supplied per instance rather than placed at a fixed symbol. See below |
| A machine-readable contract beside the object | The constants are in the object. The header and bindings state the same figures as compile-time constants. `a_host_refuses_bindings_from_another_build` shows the host refusing to run when the two differ | Implemented |
| A native's signature is checked on the host side | The bindings give each native a function pointer type. The two-library test coerces the host's definition to it, and a wrong-arity definition fails to compile with `mismatched types` | Implemented for Rust. The C header declares prototypes, which `host_contract_header.rs` already covers |

## Two departures from the roadmap's wording

**No execution-time figure is exported.** The verifier's per-chunk cost is
counted in virtual-machine cost units. It does not include the cost of calls
between scripts, so it bounds neither native execution time nor the whole
program. A symbol carrying it would be read as a bound it is not.
[`NATIVE_WCET_ASYMMETRY.md`](./NATIVE_WCET_ASYMMETRY.md) records the same
asymmetry for the motor policy example. Carrying bounds onto native code is
Order 4 and Workstream E, and the roadmap's open decision 1 asks whether native
WCET is hard or best effort. That decision belongs to the operator.

**The shared region is not placed at a fixed symbol.** The handles take the
shared buffer as a parameter, so one library serves several live instances with
separate buffers, as the gate test does. A region at a fixed symbol would admit
one instance per program. The layout is still C-representable, and its size is
an exported constant.

## Limits

- One target was executed, `aarch64-apple-darwin`, which is also the hosted
  runner's. The `nm` reader handles ELF naming, but no ELF archive was linked
  or run.
- The host reserves its own slots. `coroutine::pool::SlotPool` is Rust in this
  package, and a linked host would have to depend on it. A C host gets the slot
  contract but not the pool, as Workstream C records.
- Dialogue shapes and native body lifetimes are accepted by `linkage::build`
  through `HostContracts`. The generated header and bindings do not render
  them. A host using either relies on `coroutine.rs` for the obligations.
- The object is written after the `default<O2>` middle end. The JIT
  differentials run unoptimised, so the gate test is also the lexer's first
  differential through the optimised pipeline.
- The frame bound is found by bisection over complete lowerings, as Workstream C
  records, so building the lexer library takes several seconds.
- An interrupt handler object file, the roadmap's canonical foreign example,
  belongs to the V0.4.X low-level tier and is not attempted here.
