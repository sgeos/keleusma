# A Rust host linking the self-hosted lexer as a static library

```sh
cd native_codegen && examples/lexer_host/run.sh
```

The script lowers `src/selfhost/kel/lexer.kel` as a coroutine at its static frame bound, writes the
object with its C header and Rust bindings, archives the object as `libkel_lexer.a`, and links
`host.rs` against it with `rustc`. The host then lexes a one-line source and prints the token
yields.

## The contract the object carries

`keleusma_native::linkage` names the stable handles under a prefix, here `kel_lexer`, and makes
every other definition internal. The object therefore exports these handles.

- `kel_lexer_start`, `kel_lexer_resume` and `kel_lexer_release`.
- A constant for each figure the host must honour. These are the frame bound, the slot size, the
  buffer alignment, the shared, private and region sizes, the private image, and the trap verdict.

The header and the bindings state the same figures as compile-time constants, which a host needs
in order to size static storage. `host.rs` compares the two before it runs, and refuses when they
differ, because a difference means the bindings and the archive came from different builds.

The shared layout is given as named fields with an offset, an element size and a count. The lexer
reads its source from `src.bytes` and its length from `src.len`.

## What it does not carry

No execution-time bound is exported. The verifier's cost figure is counted in virtual-machine cost
units and leaves out script calls, so it bounds neither native time nor the whole program. See
[`NATIVE_WCET_ASYMMETRY.md`](../../../docs/decisions/NATIVE_WCET_ASYMMETRY.md).

The lexer's trap verdict is 2, which means that it may trap. The lexer contains an explicit `Trap`,
and the first-pass analysis reports where it is.

## How it is kept honest

`tests/static_library_linkage.rs` builds this host from this file and runs three lexer instances in
interleaved turns. It compares every yield and every shared byte with the virtual machine. The
same file links a C host against the same archive. It also links two libraries into one Rust host,
and checks that the archive defines nothing beyond its handles and its contract.
