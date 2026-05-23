# Chapter 35. Sizing the Arena and Reading the Bounds

> Part IX, Embedding Keleusma in a Rust Program. Chapter 35 of 40.
> Previous: [Chapter 34, The Coroutine Protocol from the Host Side](./34_coroutine_protocol.md).
> Next: [Chapter 36, Loading Precompiled and Signed Bytecode](./36_loading_bytecode.md).

## Goal

By the end of this chapter you will be able to choose an arena capacity
deliberately rather than relying on the default.

## What the arena holds

The arena is a single contiguous block of memory the VM borrows. The
operand stack grows from one end and dynamic strings grow from the other.
The total used during one Stream-to-Reset iteration is bounded by the
worst-case memory usage, the WCMU analysis of Chapter 20. The host
chooses the arena's capacity at construction. There are three options.

## Option A: the default capacity

`DEFAULT_ARENA_CAPACITY` is sixty-four kilobytes, sufficient for most
scripts:

````rust
let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
````

This is the right starting point. Move to a computed or fixed size only
when there is a reason.

## Option B: compute the capacity from the module

`auto_arena_capacity_for` walks a module and returns the capacity its
WCMU bound requires:

````rust
let cap   = keleusma::vm::auto_arena_capacity_for(&module, &[])?;
let arena = Arena::with_capacity(cap);
let vm    = Vm::new(module, &arena)?;
````

The second argument is a slice of per-native heap attestations; pass an
empty slice when the script's natives do not allocate into the arena, and
attested values otherwise. This option sizes the arena to exactly what
the program needs, no more.

## Option C: a static buffer

On an embedded target without a heap, the arena can run from a host-owned
buffer placed in static memory:

````rust
static mut ARENA_BUFFER: [u8; 16 * 1024] = [0; 16 * 1024];
let arena = unsafe {
    Arena::from_static_buffer(core::ptr::addr_of_mut!(ARENA_BUFFER))
};
````

This is the pattern for a `no_std` deployment, where the arena is a fixed
region of `.bss` rather than a heap allocation.

## When the arena is too small

If the chosen capacity is below the module's analyzed WCMU, `Vm::new`
returns `VmError::VerifyError`. The error is surfaced at construction,
before any code runs. An undersized arena is therefore not a runtime
hazard. It is a construction-time rejection, the same kind of rejection
as any other the verifier produces.

This is the memory budget of Chapter 20 seen from the host side. The
verifier proved a WCMU bound; the host must provide an arena at least
that large; and the check that the two agree happens at `Vm::new`.

## What you now know

- The arena is bounded working memory; its size is the host's choice.
- `DEFAULT_ARENA_CAPACITY` is the default; `auto_arena_capacity_for`
  computes an exact size; `Arena::from_static_buffer` runs from static
  memory.
- An arena smaller than the module's WCMU is rejected at `Vm::new`,
  before any code runs.

The next chapter loads bytecode that was compiled ahead of time.
