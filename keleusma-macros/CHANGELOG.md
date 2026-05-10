# Changelog

All notable changes to `keleusma-macros` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-05-10

Initial release.

### Added

- `KeleusmaType` derive macro generating `KeleusmaType` trait implementations for host-defined structs and enums whose field and payload types are admissible interop types. Named-field structs, unit-style enums, tuple-style enums, and struct-style enums are all supported.
- The macro's expansion references types and traits defined in the [`keleusma`](https://crates.io/crates/keleusma) crate and is not standalone-useful.

### Notes

- Implementation-detail crate. Depend on `keleusma` directly and consume the derive through `keleusma::KeleusmaType`. The `keleusma` crate re-exports the macro at the top level, so users do not need to add this crate as an explicit dependency. Publication as a separate crate is required only because Cargo restricts proc-macro implementations to their own libraries.
- The crate version is locked one-to-one with the major-minor of `keleusma`. Breaking changes in either are coordinated.

### Licensed

- BSD Zero Clause License (`0BSD`).
