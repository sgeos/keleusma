#!/usr/bin/env bash
# Regenerate the real failure transcripts that scripts/test_gate_summary.py replays.
#
# Each fixture is the merged stdout and stderr of a real cargo, nextest, clippy or
# rustfmt run against a throwaway crate, with its exit status on the first line.
# Temporary paths are replaced by /WORK so the fixtures carry no machine paths.
# Re-run after a toolchain upgrade changes output formats, then rerun the tests.
#
# Usage: scripts/gate_summary_fixtures/capture.sh
set -u
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d "${TMPDIR:-/tmp}/gate-fixture.XXXXXX")"
trap 'rm -rf "$work"' EXIT
export RUSTC_WRAPPER='' CARGO_TARGET_DIR="$work/target" CARGO_TERM_COLOR=never

crate() {
  rm -rf "$work/c" && mkdir -p "$work/c/src"
  printf '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2024"\n[workspace]\n' > "$work/c/Cargo.toml"
  printf '%s\n' "$1" > "$work/c/src/lib.rs"
}

record() {
  local name="$1"; shift
  local out status
  out="$(cd "$work/c" && "$@" 2>&1)"; status=$?
  # macOS reports the temporary directory both with and without /private, and rustdoc
  # uses a temporary directory of its own.
  { printf 'exit=%d\n' "$status"; printf '%s\n' "$out"; } |
    sed -E -e "s#(/private)?$work#/WORK#g" -e 's#(/private)?/(var/folders|tmp)/[A-Za-z0-9._/-]*rustdoctest[A-Za-z0-9]+#/TMP#g' \
      -e 's#(/private)?/tmp/claude-[0-9]+/[A-Za-z0-9._-]+#/TMP#g' > "$here/$name.txt"
}

crate '/// ```
/// assert_eq!(fixture::two(), 2);
/// ```
pub fn two() -> u32 { 2 }
#[cfg(test)]
mod tests {
    #[test] fn passes() { assert_eq!(super::two(), 2); }
    #[test] fn also_passes() { assert!(true); }
}'
record pass_cargo_test cargo test
record pass_nextest cargo nextest run

crate 'pub fn two() -> u32 { 2 }
#[cfg(test)]
mod tests {
    #[test] fn passes() { assert_eq!(super::two(), 2); }
    #[test] fn assertion_fails() { assert_eq!(super::two(), 3, "two is not three"); }
    #[test] fn unwrap_panics() { let v: Option<u32> = None; v.unwrap(); }
}'
record fail_cargo_test cargo test
record fail_nextest cargo nextest run --no-fail-fast

crate 'pub fn two() -> u32 { "two" }'
record fail_compile cargo test

crate '/// ```
/// assert_eq!(fixture::two(), 3);
/// ```
pub fn two() -> u32 { 2 }'
record fail_doctest cargo test --doc

crate 'pub fn f(x: u32) -> u32 { let y = x.clone(); y }'
record fail_clippy cargo clippy --all-targets -- -D warnings

crate 'pub fn f( x:u32 )->u32{x}'
record fail_fmt cargo fmt --check

crate '#[cfg(test)]
mod tests {
    #[test] fn aborts() { std::process::abort(); }
}'
record fail_abort cargo test

crate '#[cfg(test)]
mod tests {
    #[test] fn hangs() { std::thread::sleep(std::time::Duration::from_secs(30)); }
}'
mkdir -p "$work/c/.config"
printf '[profile.default]\nslow-timeout = { period = "1s", terminate-after = 2 }\n' > "$work/c/.config/nextest.toml"
record fail_nextest_timeout cargo nextest run

ls "$here"/*.txt
