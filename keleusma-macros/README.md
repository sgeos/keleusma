# keleusma-macros

Procedural macros for the [Keleusma](https://crates.io/crates/keleusma) scripting language.

## Implementation detail

This crate is the proc-macro backend for the `KeleusmaType` derive. It is published only because Cargo requires proc-macro crates to live in their own library. The macro's expansion references types and traits defined in the `keleusma` crate; using this crate without `keleusma` will produce code that does not compile.

**Depend on the `keleusma` crate, not on this one.** The derive is re-exported as `keleusma::KeleusmaType`:

```rust
use keleusma::KeleusmaType;

#[derive(KeleusmaType)]
struct Point {
    x: f64,
    y: f64,
}
```

The shape mirrors `serde` + `serde_derive` and `tokio` + `tokio-macros`: a thin proc-macro crate paired with a regular crate, where the regular crate is the user-facing API.

## Stability

This crate's public API is the macros themselves and their generated output's interface contract with the `keleusma` crate. The crate version is locked one-to-one with `keleusma`; breaking changes in either are coordinated.

## License

BSD Zero Clause License (`0BSD`). Same as Keleusma.
