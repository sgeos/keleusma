- **The manifests no longer set `homepage` equal to `repository`.** Cargo 1.100 warns
  about the duplicate on every build that depends on `keleusma`, `keleusma-macros`,
  `keleusma-cli` or `keleusma-bench`, and a downstream host crate reported the warning.
  The registry still links the repository. The crates whose `homepage` names their own
  subdirectory keep it.
