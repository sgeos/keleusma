# Chapter 35. Sizing the Arena and Reading the Bounds

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

```rust
let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
```

This is the right starting point. Move to a computed or fixed size only
when there is a reason.

## Option B: compute the capacity from the module

`auto_arena_capacity_for` walks a module and returns the capacity its
WCMU bound requires:

```rust
let cap   = keleusma::vm::auto_arena_capacity_for(&module, &[])?;
let arena = Arena::with_capacity(cap);
let vm    = Vm::new(module, &arena)?;
```

The second argument is a slice of per-native heap attestations; pass an
empty slice when the script's natives do not allocate into the arena, and
attested values otherwise. This option sizes the arena from the module's
own bounds.

The figure includes the composite values the host passes to the entry
and resumes it with, because each is copied into the arena and kept
until the next `Reset`. When such a value carries `Text`, use
`auto_arena_capacity_with_input_text` and pass a bound on the text bytes
in any one value, since the module cannot know the length of a host
string.

The figure also includes the region a native compilation of the module
reserves, so that one capacity works whether the module runs on the
virtual machine or as native code. A host that runs only the virtual
machine is therefore over-provisioned by that region, which
`keleusma::native_region::native_region_supplement_bytes` reports, and a
host that used to add the native figure itself should stop, or it counts
the region twice.

## Option C: a static buffer

On an embedded target without a heap, the arena can run from a host-owned
buffer placed in static memory:

```rust
static mut ARENA_BUFFER: [u8; 16 * 1024] = [0; 16 * 1024];
let arena = unsafe {
    Arena::from_static_buffer(core::ptr::addr_of_mut!(ARENA_BUFFER))
};
```

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
