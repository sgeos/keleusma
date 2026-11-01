- **`auto_arena_capacity_for` now includes the native region supplement.** The figure adds
  the region a native compilation of the module reserves, reported by the new
  `keleusma::native_region::native_region_supplement_bytes`, so that one capacity works on
  the virtual machine and as native code. The formula reimplements the V0.3.X line's native
  backend, which stays the authority, and is pinned against five figures that line measured.
  A host that runs only the virtual machine is over-provisioned by exactly the supplement, so
  the figure is no longer the virtual machine's byte-exact minimum. A host that formerly added
  the native figure itself must stop, or it counts the region twice. On the operator's ruling
  on question 22, confirmed first-hand on 2026-10-31. Pinned by
  `tests/native_region_supplement.rs`.
