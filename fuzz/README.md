# Fuzz targets

This package holds [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets for the
surface of the runtime that accepts untrusted input. It is a detached package with its own
workspace, like `keleusma-lsp` and `keleusma-wasm`, because libFuzzer needs a nightly
toolchain. Each target asserts only that its entry point returns. A panic, an abort, a
timeout or an exhausted memory limit is a finding.

## Targets

| Target | Entry points |
|--------|--------------|
| `load_bytes` | `Vm::load_bytes`, then one bounded `Vm::call` |
| `load_signed` | `Vm::load_signed_bytes` with an empty and a one-key trust matrix |
| `verify_module` | `verify::verify` on a decoded or synthesized module |
| `module_bounds` | `verify::module_wcet_with_bounds`, `verify::module_wcmu`, `verify::module_runtime_footprint` and `verify::module_call_depth` |
| `front_end` | `lexer::tokenize` and `parser::parse` on UTF-8 text and on arbitrary bytes |

The bodies are in [`harness.rs`](harness.rs). Its module documentation records the design
choices, which are summarized here.

- The bytecode targets examine each input as given and with its checksum trailer
  recomputed. Without the second form almost every mutation is refused by the checksum and
  the decoders behind it are never reached.
- The structured targets decode the input as an artifact. When that fails they synthesize
  a one-chunk module from the bytes and pass it through the encoder and the decoder, so
  that every module examined is one the loader could itself produce.
- The virtual machine has no step budget, so execution is limited to single-chunk modules
  whose verified worst-case execution time is at most `MAX_EXECUTED_WCET`. Inputs are
  capped in length.

## Running

Install the tool once with `cargo install cargo-fuzz`. Then, from this directory, run a
target for a fixed time.

```sh
cargo +nightly fuzz run load_bytes -- -max_total_time=60
cargo +nightly fuzz run load_signed -- -max_total_time=60
cargo +nightly fuzz run verify_module -- -max_total_time=60
cargo +nightly fuzz run module_bounds -- -max_total_time=60
cargo +nightly fuzz run front_end -- -max_total_time=60
```

A run reads and extends `corpus/<target>/`. Only the entries whose names begin with
`seed-` are tracked, so the entries a run adds stay local. A crash is written under
`artifacts/<target>/` with the command that reproduces it. To keep running past a known
crash, add `-fork=2 -ignore_crashes=1` after the `--`.

## Replay in the ordinary suite

`tests/fuzz_corpus_replay.rs` in the parent package includes `harness.rs` by path and
calls the same bodies on every committed corpus file, on the stable toolchain and without
cargo-fuzz. It fails on a panic, on an input that does not finish within its timeout, and
on a corpus whose files no longer reach the depth each target requires, which is how stale
bytecode seeds show up. The seeds are compiled from repository sources. To regenerate them
after a wire-format change, run the following from the repository root.

```sh
cargo test --features signatures --test fuzz_corpus_replay -- --ignored regenerate
```

A reproducer for a finding is added as a test in that file. While the library defect is
open, the test is marked `#[ignore]` with a comment that describes the defect, so that the
suite stays green and the reproducer is kept.
