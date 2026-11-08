- **`auto_arena_capacity_for` is the virtual machine's figure again, and each route has its own
  function.** This is a host-observable change. The function no longer adds the native region
  supplement, so a host that runs the virtual machine gets the smaller figure it needs, the sum
  of the verifier's terms, which the new `keleusma::vm::vm_arena_terms_for` lists one by one. A
  host that keeps one arena for either route calls the new
  `keleusma::vm::auto_arena_capacity_for_vm_and_native`, which returns what
  `auto_arena_capacity_for` returned since 2026-10-31 and never less. A host that runs only
  native code calls the new `keleusma::native_region::native_route_buffers_for`, which reports
  the native composite region, the private buffer with the new
  `native_persistent_supplement_bytes`, and the shared buffer, and none of the virtual machine's
  terms. A host that sized native execution from `auto_arena_capacity_for` and does not switch
  is short by the region. On the operator's ruling that the function right-size an arena,
  recorded in `docs/decisions/ARENA_SIZING_ROUTES.md`. Pinned by
  `tests/native_region_supplement.rs`, which reproduces the five figures the V0.3.X line sent.
