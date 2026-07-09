# Changelog

All notable changes to `keleusma-macros` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-07-08

Published to crates.io as `keleusma-macros` 0.2.1.

The crate version tracks the major-minor of `keleusma`; the 0.2.x line evolved the
`KeleusmaType` derive for the V0.2.x flat-byte composite representation (B28) and
added a second derive. These changes were not recorded at the 0.2.0 cut and are
consolidated here for the 0.2.x line.

### Added

- **`KeleusmaError` derive.** Maps a fieldless (discriminant-only) host error enum to the `Word` error codes that the script-side `error(code)` construct binds (B35). The error code is the variant discriminant. Deriving it on an enum with fields is a compile error.
- **Flat-byte composite marshalling on the `KeleusmaType` derive.** For a struct or enum whose fields and payloads are all flat-eligible, the expansion now generates `flat_byte_size`, `from_flat_bytes` and `from_flat_bytes_ctx` (reading a composite from a flat byte body), and `to_flat_bytes` (writing one, the mirror used by `Vm::marshal_shared_into`/`unmarshal_shared`, B34). This matches the flat body the V0.2.x runtime and compiler use, replacing the boxed representation.

### Changed

- Enum marshalling uses the padded uniformly-flat body, and a text or opaque field is decoded at the host boundary through a `RefContext`. Every composite-body slice the derive emits is bounds-checked, so a short or attacker-shaped body yields a clean `VmError` rather than a panic (audit finding 10).

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
