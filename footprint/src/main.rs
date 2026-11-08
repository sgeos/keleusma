//! The footprint probe. See Cargo.toml for what it measures and how.
//!
//! The baseline and every probe configuration share this skeleton: the cortex-m-rt
//! entry point, a bump allocator over a static region and a panic handler that loops.
//! Both allocate, so the allocator is in the baseline and the measured difference is
//! the runtime alone. `black_box` keeps the module bytes and every result opaque to the
//! optimizer, so nothing the runtime would execute is folded away at build time.
//!
//! The figures are what one embedder's call pattern links, loading a module and calling
//! its entry point. A host that also registers natives, resumes streams or swaps
//! modules links more. The probe never runs. It is built and measured only.
#![no_std]
#![no_main]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::hint::black_box;
use core::panic::PanicInfo;
use cortex_m_rt::entry;

/// Heap size for the bump allocator. It must cover the arena and the module.
const HEAP_BYTES: usize = 64 * 1024;
/// Arena capacity handed to the virtual machine.
#[cfg(feature = "probe")]
const ARENA_BYTES: usize = 16 * 1024;

/// A bump allocator that never frees. The probe allocates a bounded amount once, and
/// the image is never run, so freeing would only add code to both sides of the
/// difference.
struct Bump {
    heap: UnsafeCell<[u8; HEAP_BYTES]>,
    next: UnsafeCell<usize>,
}

// The probe is single-threaded and has no interrupt handlers that allocate.
unsafe impl Sync for Bump {}

unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: one thread of execution, as the Sync impl above states.
        let next = unsafe { &mut *self.next.get() };
        let start = next.next_multiple_of(layout.align());
        match start.checked_add(layout.size()) {
            Some(end) if end <= HEAP_BYTES => {
                *next = end;
                // SAFETY: start + size is within the heap, checked just above.
                unsafe { self.heap.get().cast::<u8>().add(start) }
            }
            _ => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

#[global_allocator]
static HEAP: Bump = Bump {
    heap: UnsafeCell::new([0; HEAP_BYTES]),
    next: UnsafeCell::new(0),
};

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// A serialized module. Its contents do not affect the code linked, since the bytes
/// pass through `black_box`, but a real module keeps the probe honest if it is run.
static MODULE: &[u8] = include_bytes!("../../fuzz/corpus/load_bytes/seed-01_arithmetic.kbc");

#[entry]
fn main() -> ! {
    let bytes: &[u8] = black_box(MODULE);
    run(bytes);
    loop {
        core::hint::spin_loop();
    }
}

/// The baseline: allocate as the probe does, and touch the bytes.
#[cfg(not(feature = "probe"))]
fn run(bytes: &[u8]) {
    let copy: alloc::vec::Vec<u8> = bytes.to_vec();
    black_box(copy);
}

/// Load the module and call its entry point. With `verify`, loading runs the
/// load-time verifier, as a host that accepts untrusted bytecode must. Without it, the
/// module is loaded under trust, as a host of precompiled and signed-off bytecode may.
#[cfg(feature = "probe")]
fn run(bytes: &[u8]) {
    use keleusma::vm::Vm;
    use keleusma_arena::Arena;

    let arena: &'static Arena =
        alloc::boxed::Box::leak(alloc::boxed::Box::new(Arena::with_capacity(ARENA_BYTES)));
    #[cfg(feature = "verify")]
    let vm = Vm::load_bytes(bytes, arena).ok();
    #[cfg(not(feature = "verify"))]
    let vm = keleusma::bytecode::Module::from_bytes(bytes)
        .ok()
        // SAFETY: never run. A host that loads under trust takes on this obligation.
        .and_then(|module| unsafe { Vm::new_unchecked(module, arena) }.ok());
    if let Some(mut vm) = vm {
        black_box(vm.call(&[]).is_ok());
    }
}
