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

## Supported Input Shapes

The derive accepts the following Rust input shapes.

- **Named-field structs**, for example `struct Point { x: f64, y: f64 }`. Each field type must itself implement `KeleusmaType`.
- **Enums with unit variants**, for example `enum Color { Red, Green, Blue }`.
- **Enums with tuple variants**, for example `enum Shape { Circle(f64), Rect(f64, f64) }`. Each payload type must implement `KeleusmaType`.
- **Enums with struct-style variants**, for example `enum Event { Click { x: i64, y: i64 } }`. Each field type must implement `KeleusmaType`.

The following inputs produce a compile error.

- **Tuple structs** such as `struct Wrapper(i64);` are rejected. Use a named-field struct or the bare tuple type.
- **Unit structs** such as `struct Marker;` are rejected. Use the unit type `()`.
- **Unions** are rejected. Unions cannot be safely projected into the runtime `Value` enum.

The full trait contract, including the list of admissible interop field types, lives in the [`keleusma::KeleusmaType`](https://docs.rs/keleusma/latest/keleusma/trait.KeleusmaType.html) trait documentation.

## Stability

This crate's public API is the macros themselves and their generated output's interface contract with the `keleusma` crate. The crate version is locked one-to-one with `keleusma`; breaking changes in either are coordinated. See [CHANGELOG.md](./CHANGELOG.md) for version history.

## License

BSD Zero Clause License (`0BSD`). Same as Keleusma.
