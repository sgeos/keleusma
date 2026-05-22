---
name: Bug report
about: Report a defect in the runtime, compiler, verifier, or tooling
title: ''
labels: bug
assignees: ''
---

## Summary

<!--
One or two sentences describing the bug. State the expected
behaviour and the observed behaviour.
-->

## Version

- Keleusma crate version (`keleusma = "X.Y.Z"` from `Cargo.toml`):
- `keleusma-cli` version, if relevant (`keleusma --version`):
- Rust toolchain version (`rustc --version`):
- Host platform (operating system, architecture):

## Minimal reproducer

<!--
Provide the smallest Keleusma source and, when relevant, the
Rust host code that exhibits the bug. If the bug surfaces only
through the embedding API, both pieces help.
-->

```keleusma
// the Keleusma script
fn main() -> Word {
    42
}
```

```rust
// the Rust host code, if applicable
```

## Steps to reproduce

1.
2.
3.

## Expected behaviour

<!-- What you expected to happen. -->

## Observed behaviour

<!--
What actually happened. If the runtime emitted an error,
include the full diagnostic message and the call stack.
-->

## Static guarantee affected (if applicable)

If you believe the bug violates one of the five static guarantees, name it:

- [ ] Totality (programs admitted by the verifier always terminate at every yield or return)
- [ ] Productivity (every `loop` iteration yields)
- [ ] Bounded-step (every Stream-to-Reset slice executes within the declared WCET)
- [ ] Bounded-memory (every Stream-to-Reset slice fits within the declared WCMU)
- [ ] Safe-swap (hot-swap preserves the data-segment schema)
- [ ] Not applicable; this is a tooling or documentation bug

## Additional context

<!--
Anything else: workarounds you tried, related issues, links to
upstream discussion, profiling data, etc.
-->
