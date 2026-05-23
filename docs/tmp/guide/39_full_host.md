# Chapter 39. A Full Host, End to End

> Part IX, Embedding Keleusma in a Rust Program. Chapter 39 of 40.
> Previous: [Chapter 38, Calibrated WCET and Cost Models](./38_cost_models.md).
> Next: [Chapter 40, Further Reading](./40_further_reading.md).

## Goal

By the end of this chapter you will be able to read the complete piano
roll host and see where each technique from this part appears in it.

## The file

The piano roll host is the single file `examples/piano_roll.rs` in the
repository. It is around thirteen hundred lines, and it is written to be
read. This chapter is a map of it.

## Two kinds of code

The most useful thing to recognize on a first read is that the file
contains two kinds of code, and only one of them is about embedding
Keleusma.

The embedding code is the subject of this part. It is a small fraction of
the file:

- `build_module` runs `tokenize`, `parse`, and `compile`, the phases of
  Chapter 32.
- `run` constructs the `Arena` and the `Vm`, the rest of Chapter 32.
- `register_natives` registers sixteen natives with
  `register_native_closure`, the captured-state route of Chapter 33.
- `init_data` seeds the data segment through `set_data`.
- The main tick loop calls `resume`, matches `VmState`, and handles the
  `Reset` case, the protocol of Chapter 34.
- The `Reset` arm calls `replace_module` to swap songs, the hot swap of
  Chapter 37.

The audio-synthesis code is everything else, and it is not about
Keleusma at all. The `Mixer`, the `AudioCallback` implementation,
`advance_envelope`, `waveform_sample`, the `Voice` and `EnvState`
structs, and the SDL3 device setup are an ordinary software synthesizer.
Any audio program would need code like it. When reading the file to learn
embedding, this code can be skimmed.

## The main and run split

The file separates two concerns. The function `main` carries application
chrome: argument handling and process-level setup. The function `run`
carries the embeddable host loop: build the VM, open the audio device,
register natives, drive the tick-and-yield cycle. The boundary between
them is the boundary between what a different application would discard
and what it would copy. A developer lifting the piano roll into another
program copies the body of `run`.

## Patterns from the cookbook

The repository's `COOKBOOK.md` collects host-side patterns that recur
across applications and that the piano roll touches: sizing the arena
from a module's WCMU, a data-loader pattern for host configuration,
narrow-runtime type aliasing for sub-64-bit targets, signed bytecode
distribution, and calibrated WCET with a measured cost model. Each is a
short recipe building on a technique from this part.

## What you now know

- The piano roll host is one readable file of roughly thirteen hundred
  lines.
- It contains embedding code and audio-synthesis code; only the first is
  about Keleusma, and it is a small fraction of the file.
- Every technique of this part appears in it: construction, native
  registration, the resume protocol, hot swap.
- The `main` and `run` split separates application chrome from the
  embeddable host loop.

That completes Part IX. You have the full host-side surface: construction,
native functions, the coroutine protocol, arena sizing, bytecode loading,
hot swap, and cost models. The final part points to where to go next.
