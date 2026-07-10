# keleusma-wasm

WebAssembly bindings for the Keleusma compiler, powering the browser
**playground**. It compiles and verifies a program entirely in the browser and
reports its per-chunk worst-case-execution-time and worst-case-memory-usage
bounds — the definitive resource bounds that are Keleusma's whole point, shown
live in a way no other language's playground offers.

Like `keleusma-lsp`, this is a **detached-workspace crate** that depends on the
core crate by path but ships `wasm-bindgen`/`serde`, so it stays out of the core
`no_std` workspace. The core crate being `no_std + alloc` is what makes it
compile cleanly and compactly to `wasm32-unknown-unknown`.

## What it exposes

One function, `check(src: &str) -> String`, returning JSON:

```json
{
  "ok": true,
  "diagnostics": [],
  "bounds": [
    { "chunk": "main", "wcet_cycles": 12, "wcmu_stack_bytes": 32, "wcmu_heap_bytes": 0 }
  ]
}
```

On failure, `ok` is `false` and `diagnostics` carries `{ line, column, message }`
from whichever stage rejected the program (lex, parse, compile, or verify). It is
static analysis only; the program is not executed.

## Build

`wasm-pack` drives the build. Because the repository's `stable` toolchain may be
unavailable, force a known-good toolchain:

```sh
cd keleusma-wasm
rustup run 1.92 wasm-pack build --target web            # release (small, wasm-opt)
rustup run 1.92 wasm-pack build --target web --dev      # fast, unoptimized
```

Output lands in `keleusma-wasm/pkg/` (git-ignored; it is a build artifact).

## Run the playground locally

Serve the crate directory so the page and the `pkg/` output are both reachable,
then open the playground:

```sh
cd keleusma-wasm
python3 -m http.server 8000
# then browse to http://localhost:8000/www/
```

The page (`www/index.html`) imports `../pkg/keleusma_wasm.js`, calls `check` on
every edit (debounced), and renders either the diagnostics or the WCET/WCMU
bounds table.

## Roadmap

- **Deploy to GitHub Pages** alongside the hosted book (CI, tracked as M5).
- **Run with output capture** — execute the program and show `println` output.
  Needs host-native registration and an output sink; a later addition.
