//! Build script. Emits linker flags only when targeting a
//! bare-metal `none` OS so the host-side `three-task-std`
//! binary continues to link with the system default linker
//! invocation. The embassy hello-world reference uses the same
//! pattern as a precedent for split-target crates.

use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();

    // `target_os == "none"` covers all bare-metal targets the
    // crate supports today (thumbv8m.main-none-eabihf for the
    // N6). The host targets (macos, linux, windows) bypass the
    // embedded link arguments entirely.
    if target_os == "none" {
        // `--nmagic` disables page alignment of sections,
        // matching the small AXISRAM regions used by the N6.
        println!("cargo:rustc-link-arg-bins=--nmagic");
        // `link.x` comes from cortex-m-rt; `defmt.x` from defmt.
        println!("cargo:rustc-link-arg-bins=-Tlink.x");
        println!("cargo:rustc-link-arg-bins=-Tdefmt.x");
        // The linker needs to know where to find `memory.x`.
        println!("cargo:rerun-if-changed=memory.x");
    }
}
