# A Rust host linking the self-hosted lexer as a static library

```sh
cd native_codegen && examples/lexer_host/run.sh
```

The script lowers `src/selfhost/kel/lexer.kel` as a coroutine at its static frame bound, writes the
object with its C header and Rust bindings, archives the object as `libkel_lexer.a`, and links
`host.rs` against it with `rustc`. The host then lexes a one-line source and prints the token
yields, followed by the deepest machine stack any call used.

## The contract the object carries

`keleusma_native::linkage` names the stable handles under a prefix, here `kel_lexer`, and makes
every other definition internal. The object therefore exports these handles.

- `kel_lexer_start`, `kel_lexer_resume` and `kel_lexer_release`.
- A constant for each figure the host must honour. These are the frame bound, the slot size, the
  buffer alignment, the shared, private and region sizes, the private image, the trap verdict,
  the machine stack bound and the composed cost claim with its status.

The header and the bindings state the same figures as compile-time constants, which a host needs
in order to size static storage. `host.rs` compares the two before it runs, and refuses when they
differ, because a difference means the bindings and the archive came from different builds.

The shared layout is given as named fields with an offset, an element size and a count. The lexer
reads its source from `src.bytes` and its length from `src.len`.

## What it does not carry

No execution-time bound is exported. `kel_lexer_wcet_cost` is a bytecode cost claim, best effort as
a measure of native time, and for the lexer its status is 0. One of its callees compares two
`Word` call results, which the verifier's text analysis cannot bound, so no claim is composed.
See [`ORDER_4_COMPLETION.md`](../../../docs/decisions/ORDER_4_COMPLETION.md).

The lexer's trap verdict is 2, which means that it may trap. The lexer contains an explicit `Trap`,
and the first-pass analysis reports where it is. A trap ends the instance with status 3 and its
trap code instead of ending the host process.

The stack bound covers code in the archive. A host native or a compiler runtime routine the code
calls is outside it, and the lexer calls neither.

## How it is kept honest

The host paints the stack below its stack pointer before each call and reports the deepest byte a
call changed. It also places guard words after every buffer it sized from the contract and refuses
to finish if the library wrote any of them.

`tests/static_library_linkage.rs` builds this host from this file and runs three lexer instances in
interleaved turns. It compares every yield and every shared byte with the virtual machine. The
same file links a C host against the same archive. It also links two libraries into one Rust host,
and checks that the archive defines nothing beyond its handles and its contract. Another test in that file runs the host twice with different paint values and requires
the measured stack to stay within `kel_lexer_stack_bytes`.
