- **A hot swap refuses a module whose working set the arena cannot hold.** `Vm::new` refuses an
  arena too small for the module's operand stack, call frames and opaque registry, but
  `replace_module` admitted such a module and failed on its first push past the arena. It now
  refuses the swap before changing anything, and the running module is kept. Pinned by
  `tests/hot_swap_working_set.rs`. A pykeleusma finding.
