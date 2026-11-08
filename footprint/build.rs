//! Put memory.x where cortex-m-rt's link.x looks for it.

use std::{env, fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::copy("memory.x", out.join("memory.x")).expect("memory.x is beside Cargo.toml");
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=memory.x");
}
