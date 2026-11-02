- **The virtual machine's arena figure counts the private composite initializers.** Construction
  and a hot swap copy each private composite initializer through the arena top region into the
  persistent pool, and the copy stays until the first `Reset`. The figure omitted it, so a module
  with a private composite slot ran out of arena on its first call at the figure. Measured on
  `examples/scripts/14_frame_log.kel`, the virtual machine's part of the figure was 656 bytes
  and the first call needed 680. The combined figure of 2026-10-31 hid the shortfall, because
  its native region was larger. The figure now adds the initializers' pool extents and the
  packing transient of a nesting initializer. The memory bound's invariant, that a verified run
  never exceeds the figure, is restored for such modules. Pinned by
  `a_seeded_private_composite_is_counted_and_the_figure_is_tight` in
  `tests/native_region_supplement.rs`.
