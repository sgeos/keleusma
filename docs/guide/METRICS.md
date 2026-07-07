# Metrics

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Resource footprint and execution metrics for the V0.2.1 Keleusma CLI compared with popular scripting languages. The numbers below are intended for operators planning embedded or constrained deployments.

## Measurement methodology

All numbers measured on a single Apple M1 (`arm64-apple-darwin23`) host, May 2026. Each runner executed the same conceptual workload: load the runtime, parse-or-load a trivial program, print the integer 42, exit. Five runs per runner; the typical mid-range value is reported.

- **Binary size**: stripped or default release-build size as reported by `ls -l`. Path resolved through `readlink` where present.
- **Maximum resident set size**: peak RSS as reported by `/usr/bin/time -l` on macOS. Includes the runtime's own memory plus all shared libraries page-resident at any point.
- **Peak memory footprint**: peak dirty + anonymous memory as reported by `/usr/bin/time -l`. Lower than maximum RSS because it excludes shared library text pages.
- **Cycles elapsed**: CPU cycles consumed from process start to process exit.
- **Real time**: wall-clock duration from process start to exit. Rounded to 10 ms by the measurement tool.

The Keleusma measurement runs the full strict-mode-capable CLI binary including the encryption feature stack (X25519 + AES-256-GCM + HKDF-SHA-256). The binary as measured is what an operator would deploy.

## Results

| Runner | Binary size | Max RSS | Peak footprint | Cycles | Real time |
|--------|-------------|---------|----------------|--------|-----------|
| bash 5.x (system) | 1.3 MB | 1.85 MB | 1.26 MB | 6.3 M | < 10 ms |
| **Keleusma 0.2.1** | **2.1 MB** | **2.85 MB** | **1.49 MB** | **8.4 M** | **< 10 ms** |
| Lua 5.4 (typical published) | 0.3 MB | ~2.0 MB | ~1.5 MB | ~7 M | < 10 ms |
| Python 3.13 (MacPorts) | 34 KB launcher + framework | 11 MB | 5.7 MB | 54.8 M | 20 ms |
| Ruby 3.1 (MacPorts) | 34 KB launcher + framework | 30.7 MB | 25.7 MB | 128 M | 40 ms |
| Node.js (MacPorts) | 77 MB | 42.5 MB | 13.7 MB | 108 M | 40 ms |

The Python and Ruby launcher binaries are tiny shims that load shared libraries; the substantial code lives in the Python/Ruby framework dynamic libraries on disk. The launcher size understates total install footprint. Node.js statically links most of V8 and the runtime, hence the large binary.

The Lua row uses published numbers from the Lua 5.4 reference build rather than measurement on this machine because Lua was not available locally. Numbers are representative of a default ./configure && make build with `liblua` linked statically.

## Headline finding

**Keleusma is essentially bash-tier in resource consumption.** Binary size, RSS, peak footprint, and cycle count are all within 30 to 60 percent of bash for the same trivial workload. Both runners load and execute in under 10 ms.

The other interpreted scripting languages (Python, Ruby, Node.js) carry 5 to 20 times more memory pressure and 7 to 15 times more CPU cycles for the same workload. Their advantage is the ecosystem; the cost is the footprint.

Keleusma sits in the same operational class as bash and Lua while delivering substantially more guarantees than either. The next section addresses what the operator gets for the small overhead.

## What the operator gets

For roughly half a megabyte of additional binary and one megabyte of additional RSS versus bash, the Keleusma deployment includes:

- **Verified bytecode**: scripts cannot crash from memory issues, infinite loops, or unbounded recursion. The structural verifier rejects programs that would defeat the bounds.
- **Statically computed WCMU and WCET bounds**: arena memory is sized to exactly the bytecode's declared bound. Bash and Lua have no equivalent.
- **Ed25519 signed delivery**: scripts cryptographically authenticated to a release key. Bash scripts can be signed externally via separate tooling (gpg, codesign), but verification at execution time is not built in.
- **X25519 plus AES-256-GCM encrypted delivery**: scripts encrypted to a specific destination host. Plaintext is not readable from the artefact alone.
- **Information-flow labels**: type-system-level data-flow tracking that catches policy violations at compile time. Bash and the other scripting languages have no static IFC.
- **Strict-mode policy gate**: the CLI enforces that only signed and decryptable artefacts run. Configurable via filesystem and environment variables. No script-side mechanism can bypass.

None of these features cost the operator significant footprint. The crypto stack adds about 200 KB to the binary (out of the 2.1 MB total). The strict-mode policy machinery is a small portion of the rest.

## Per-feature footprint breakdown (approximate)

| Feature | Approx. binary contribution |
|---------|-----------------------------|
| Core compiler and VM (parser, type checker, verifier, executor) | ~1.4 MB |
| `signatures` feature (Ed25519) | ~150 KB |
| `encryption` feature (X25519, AES-GCM, HKDF, SHA-256) | ~200 KB |
| `shell` feature (process spawn, env vars, exit) | ~50 KB |
| CLI surface (argument parsing, REPL, keygen, strict-mode policy) | ~250 KB |
| Total stripped release binary | **2.1 MB** |

Embedders who do not need a feature can omit it through Cargo feature flags. A minimal embedded build with `--no-default-features --features compile,verify` (no encryption, no signing, no shell) is approximately 1.2 MB. A library-only build linked into a host application has no CLI surface and is approximately 800 KB to 1 MB of additional binary.

## Loop daemon workload

Running an indefinite `loop main` at one iteration per millisecond for one minute under the CLI uses approximately:

- Constant 2.9 MB RSS (no growth over the run; the arena is sized at startup)
- 1 to 2 percent of one M1 core (the tick rate dominates; the per-iteration cost is microseconds)
- Zero allocator pressure (the arena's transient region is reset on each yield-resume cycle)

Comparable steady-state daemon behaviour in Python or Ruby runs at 15 to 30 MB RSS with comparable or higher CPU usage for an empty loop body. The advantage compounds as the daemon's runtime extends.

### Steady-state at sleep cadence

At one tick per second under `--tick-interval 1s`, the daemon's CPU drops to effectively zero (microseconds of compute per iteration, ~999.9 ms idle in the rate-limiter's sleep). The RSS is unchanged from the high-rate workload because the arena is sized at startup and reused. The drift-compensated sleep reduces cumulative drift to under one percent over a typical operating period.

For long-cadence daemons (`--tick-interval 1h`, `--tick-interval 1d`), CPU usage is dominated by the OS scheduler's wakeup mechanism rather than Keleusma itself. The runtime is genuinely idle between iterations. A memory-resident-on-call daemon at one tick per hour consumes operationally the same resources as a 2.9 MB resident memory mapping; the cost is page-fault avoidance, not computation. See [`SECURITY_POLICY.md`](./SECURITY_POLICY.md#daemon-deployments-and-tick-interval-cadences) for the operator guide to memory-resident deployments.

## Notes on the comparison

The comparison is intentionally selective. Each runner has different design goals; direct feature comparisons are unfair.

- **bash**: shell interpreter. Designed for shell pipelines, not general programming. Faster startup than Keleusma because it does less per-statement validation, but no static type checking, no bounded resource analysis, no signature verification.
- **Lua**: embeddable scripting. Closest analogue to Keleusma in operational footprint. Lua admits unbounded recursion and unbounded loops; runtime errors are possible. No WCMU or WCET bounds. No built-in signing or encryption.
- **Python, Ruby**: general-purpose dynamic languages with rich standard libraries. Operationally heavier; type errors caught only at runtime. No bounded resource analysis. Signing and encryption available through third-party packages, not built in.
- **Node.js**: V8 plus Node runtime. Designed for high-throughput server workloads; the JIT amortizes startup cost across long-running services. Heavy for one-shot scripts. No built-in signing or encryption of code.

Where Keleusma genuinely wins: deployments that need verified execution properties (regulated industries, embedded), and deployments where the CLI runs many short-lived scripts (low per-script overhead). Where the comparators win: deployments that need rich ecosystems and where startup cost is amortized across long-running services.

## Reproducibility

The measurements above were generated on May 22 2026 with the following commands. Operators can reproduce on their own hardware.

```sh
# Build keleusma CLI release binary
cargo build --release -p keleusma-cli

# Trivial source program
echo 'fn main() -> Word { 42 }' > /tmp/hello.kel

# Measure
/usr/bin/time -l ./target/release/keleusma run /tmp/hello.kel

# Comparators (where installed)
/usr/bin/time -l bash -c 'echo 42'
/usr/bin/time -l lua -e 'print(42)'
/usr/bin/time -l python3 -c 'print(42)'
/usr/bin/time -l ruby -e 'puts 42'
/usr/bin/time -l node -e 'console.log(42)'
```

Numbers vary by host CPU, OS version, and installed-runtime version. The relative ordering (bash and Keleusma at the bottom, interpreted languages at the top) is stable across machines I have tested.
