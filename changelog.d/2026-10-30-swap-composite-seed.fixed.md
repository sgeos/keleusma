- **A hot swap seeds a composite private value.** `replace_module` cleared the persistent
  composite pool after writing `initial_data`, so a seeded struct was left boxed and the new
  module's field reads refused it. Pinned by `tests/hot_swap_composite_seed.rs`. A
  pykeleusma finding.
