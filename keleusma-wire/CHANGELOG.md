# Changelog

All notable changes to `keleusma-wire` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Note on the first publication

This crate has never been published. Its `0.1.0` is therefore a first release rather
than a change from a previous one, and there is no prior version to describe a delta
against. The entry below records what `0.1.0` ships, so a reader arriving from
crates.io is not met with an empty file.

The crate exists because the V0.2.3 line replaced the auxiliary-body encoding wholesale
with a word-oriented container, and that container has no dependency on Keleusma's
schema layer. Keeping it separate makes the format usable without the runtime, and
makes its reader testable with no allocator at all.

### Added

- The wire-format v2 container: a triplicated prologue and region directory read by
  majority-of-three vote, fixed-stride record tables, byte-addressed pools, CRC-32, and
  an optional (72,64) SECDED parity plane held parallel to the data.
- `WireBuilder`, the encoder, and `WireView`, the reader. The reader performs in-place
  reads and works with no allocator; `alloc` is a default feature the reader does not
  require.
- An off-by-default `derive` feature providing `#[derive(WireRecord)]` for fixed-size
  records, backed by the `keleusma-wire-derive` crate. Depend on this crate with that
  feature rather than on the derive crate directly.
