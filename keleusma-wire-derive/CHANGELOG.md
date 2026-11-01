# Changelog

All notable changes to `keleusma-wire-derive` will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Note on the first publication

This crate has never been published, so `0.1.0` is a first release with no prior
version to describe a delta against.

It is an **implementation detail of `keleusma-wire`** and is published only because a
proc-macro crate cannot be vendored into the crate that uses it. Depend on
`keleusma-wire` with its `derive` feature rather than on this crate directly; its
surface is not intended to be stable independently of that crate.

### Added

- `#[derive(WireRecord)]`, generating fixed-size record encode and decode for the
  `keleusma-wire` container.
