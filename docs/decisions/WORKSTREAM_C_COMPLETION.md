# Workstream C completion requirements

Workstream C in the [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md) places
coroutine frames in the arena rather than on the C stack, preserving the bounded
worst-case memory model in native code. The authoritative design is
[`V0_4_0_NATIVE_CODEGEN.md`](../roadmap/V0_4_0_NATIVE_CODEGEN.md), section
"Arena-resident coroutine frames". It gives each coroutine a region of the
master arena sized by the coroutine's static bound, returned to a free list on
release. Order 2 pairs this workstream with Workstream B under the gate
"coroutine-driven native functions callable from a host, frames
arena-resident". The operator set completing Order 2 as this line's goal on
2026-10-24. No earlier record scoped Workstream C, so these requirements are
taken from the roadmap and the design.

## Acceptance matrix

Tests are in
[`arena_coroutine_frames.rs`](../../native_codegen/tests/arena_coroutine_frames.rs)
unless another file is named.

| Requirement | Executable evidence | Disposition |
|---|---|---|
| Suspension state never leaves the frame | Lowering refuses a module whose split coroutine still calls the overflow allocator or the deallocator, or keeps a coroutine or dynamic-stack intrinsic. See `coroutine.rs` and [Workstream B](./WORKSTREAM_B_COMPLETION.md) | Implemented in Workstream B and relied on here |
| A static frame bound per coroutine | `coroutine::minimum_frame_bytes`. `the_static_frame_bound_is_the_smallest_reservation_that_lowers` requires the bound to lower and the next aligned size below it to be refused for the reservation | Implemented. 24 bytes for the two constructed subjects |
| Bounds for the self-hosted stages | `every_admitted_stage_source_has_a_static_frame_bound` derives a bound for all 14 stage sources, from 64 to 1216 bytes | Implemented. Every stage is admitted |
| Frames reserved from the master arena | `coroutine::pool::SlotPool` reserves its slots, free stack and flags from a `keleusma::Arena` in one allocation, sized by the bound | Implemented |
| Release returns the region to a free list | `the_pool_is_bounded_reuses_released_slots_and_refuses_misuse` reuses a released slot at the same address | Implemented |
| Bounded memory | The pool refuses an empty pool, an invalid frame size and a reservation the arena cannot supply. Acquisition past capacity returns `None`. Arena use is unchanged across every start, resume and release | Implemented |
| A live frame is never reused | `SlotPool::release` refuses a slot whose continuation word is non-null. Acquisition clears that word, because arena memory is not zeroed, so a slot never started reads as inactive. A slot from another pool is refused | Implemented |
| Completion frees the frame | `a_completed_reentrant_instance_returns_its_slot_without_a_release` | Implemented |
| Correct execution from arena slots | `coroutine_frames_live_in_pooled_arena_slots_and_agree_with_the_vm` runs four interleaved instances of two subjects, one with delegation. `pooled_lexer_instances_agree_with_the_vm_on_different_sources` runs three `lexer.kel` instances on different sources, comparing yields and shared writes with the virtual machine | Implemented |
| The arena cannot be reset under a live frame | `SlotPool` borrows the arena shared, and `Arena::reset` needs it mutably | Enforced by the compiler |

Every buffer an instance receives in these tests comes from the same arena as
its slot, sized by the published contract, so a run occupies one arena.

## How the design is realized

The design names an allocator and a deallocator in the emitted IR. Lowering
instead receives the slot from the host and names two sentinel functions that
must have no use after splitting. The two are equivalent for memory. In both,
the frame occupies one statically sized arena region and nothing else. The host
form keeps allocation out of native code, so a native instance can run without
an allocator symbol. The pool is the allocator and its free list is the
deallocator, called by the host around start and release.

The native composite region keeps the model in
[`region.rs`](../../native_codegen/src/region.rs): construction sites are
placed at fixed offsets within each chunk's region, so a `Reset` reuses them
with no cursor to rewind. That is the bump-and-reset discipline with the bump
resolved at compile time.

## Limits

- The bound is found by search over complete lowerings, not by reading LLVM's
  frame layout. It is exact for the reservation the lowering accepts, and costs
  one lowering per step. The 14 stage sources take about 95 seconds together.
- A call into native code still uses the machine stack while it runs. That use
  is bounded and measured by the stack-size work, not placed in the arena. Only
  state that survives a suspension is arena-resident.
- The pool is host-side Rust in `keleusma-native`. A C host gets the same slot
  contract but not this pool. Host packaging is Workstream D.
- Native execution cost is not attested, and foreign targets are not executed.
  These belong to Workstream E and to the deployment rows of Workstream B.
- Text and opaque host representations remain outside the coroutine route, as
  Workstream B records.
