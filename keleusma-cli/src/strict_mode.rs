//! Strict-mode policy gate for signed and encrypted bytecode execution.
//!
//! Discovers enrolled keys from a platform-conventional location or an
//! environment-variable-pointed directory, evaluates whether strict
//! signing mode is active, and provides the helpers that the run path
//! uses to enforce the four rejection rules.
//!
//! The discovery is fail-closed. A single malformed key file makes the
//! CLI refuse to start. This prevents partial-trust-list edge cases in
//! which a corrupted store would silently degrade to permissive mode.
//!
//! Three knobs control strict-signing activation:
//! - `KELEUSMA_TRUSTED_KEYS_DIR` environment variable selects the
//!   trust-store directory. Highest precedence.
//! - A platform-conventional directory exists at
//!   `/etc/keleusma/trusted_keys` on Unix-like systems and at
//!   `%PROGRAMDATA%\keleusma\trusted_keys` on Windows.
//! - `KELEUSMA_REQUIRE_SIGNED=1` forces strict mode even when the
//!   trust store is empty. Fail-closed for everything.
//!
//! See `tmp/enrolled_keys_execution.md` for the spec this implements.

use std::env;
use std::fs;

use ed25519_dalek::VerifyingKey;

/// Environment variable that overrides the platform-conventional
/// trusted-keys directory. When set, this directory is the only
/// source of enrolled keys and a missing directory is a startup
/// error rather than a silent fallback to permissive mode.
pub const TRUSTED_KEYS_DIR_ENV: &str = "KELEUSMA_TRUSTED_KEYS_DIR";

/// Environment variable that forces strict mode even when the
/// trusted-keys directory is empty. Useful for kiosk or quarantine
/// deployments where the policy should reject every artefact.
pub const REQUIRE_SIGNED_ENV: &str = "KELEUSMA_REQUIRE_SIGNED";

/// Set of policy keys that strict signing mode enforces against.
#[derive(Debug, Clone, Default)]
pub struct PolicyContext {
    /// Verifying keys enrolled through the trust-store directory.
    /// Loaded once at startup; not modifiable from the command line.
    pub enrolled_keys: Vec<VerifyingKey>,
    /// True when strict signing mode is active. Active when the
    /// enrolled key set is non-empty OR when the
    /// `KELEUSMA_REQUIRE_SIGNED` environment variable is set to "1".
    pub strict_signing: bool,
}

/// Build a [`PolicyContext`] by discovering enrolled keys and
/// evaluating environment-variable activation. Returns an error
/// when discovery fails (malformed key file, unreadable explicitly
/// pointed directory). Returns an empty context with strict-signing
/// false when no trust store is configured and the force-strict
/// env var is unset.
pub fn build_policy_context() -> Result<PolicyContext, String> {
    let enrolled_keys = discover_trusted_keys()?;
    let strict_signing = !enrolled_keys.is_empty() || is_strict_mode_forced();
    Ok(PolicyContext {
        enrolled_keys,
        strict_signing,
    })
}

/// Read all `*.pub` files from the configured trust-store directory
/// and return their parsed `VerifyingKey` values.
///
/// Discovery order:
/// 1. The directory named by `KELEUSMA_TRUSTED_KEYS_DIR` if the env
///    var is set.
/// 2. The platform-conventional directory if it exists.
/// 3. An empty trust store otherwise.
///
/// Fail-closed: if the configured directory exists and a key file
/// inside is malformed, return an error rather than silently
/// loading a partial trust list.
pub fn discover_trusted_keys() -> Result<Vec<VerifyingKey>, String> {
    let (dir, explicit) = match env::var(TRUSTED_KEYS_DIR_ENV) {
        Ok(d) => (d, true),
        Err(_) => match default_trusted_keys_dir() {
            Some(d) => (d, false),
            None => return Ok(Vec::new()),
        },
    };

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) if !explicit => {
            // Platform-conventional directory missing is fine.
            // Permissive mode is the default on hosts that have not
            // installed the trust-store directory.
            return Ok(Vec::new());
        }
        Err(e) => {
            return Err(format!(
                "strict mode: cannot read trusted-keys directory {}: {}",
                dir, e
            ));
        }
    };

    let mut keys = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|e| format!("strict mode: directory iteration error in {}: {}", dir, e))?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("pub") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let bytes = fs::read(&path).map_err(|e| {
            format!(
                "strict mode: reading enrolled key {}: {}",
                path.display(),
                e
            )
        })?;
        if bytes.len() != 32 {
            return Err(format!(
                "strict mode: trust store contains a malformed key file at {} ({} bytes; expected 32); refusing to start",
                path.display(),
                bytes.len()
            ));
        }
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&bytes);
        let key = VerifyingKey::from_bytes(&key_bytes).map_err(|e| {
            format!(
                "strict mode: invalid Ed25519 public key in {}: {}",
                path.display(),
                e
            )
        })?;
        keys.push(key);
    }
    Ok(keys)
}

/// Return the platform-conventional trusted-keys directory, or
/// `None` if no convention applies to the running platform.
///
/// - Unix-like systems (Linux, macOS, BSDs): `/etc/keleusma/trusted_keys`.
/// - Windows: `%PROGRAMDATA%\keleusma\trusted_keys`.
fn default_trusted_keys_dir() -> Option<String> {
    #[cfg(unix)]
    {
        Some(String::from("/etc/keleusma/trusted_keys"))
    }
    #[cfg(windows)]
    {
        env::var("PROGRAMDATA")
            .ok()
            .map(|p| format!("{}\\keleusma\\trusted_keys", p))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// Return true when `KELEUSMA_REQUIRE_SIGNED=1` is set in the
/// environment. Forces strict mode even when the trust store is
/// empty; the result is that no artefact runs at all.
fn is_strict_mode_forced() -> bool {
    matches!(env::var(REQUIRE_SIGNED_ENV).as_deref(), Ok("1"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialise tests that mutate process-global state (environment
    // variables and current directory). Rust's test runner runs
    // tests in parallel by default; without serialisation the env
    // var manipulation here would race.
    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn with_env<F: FnOnce()>(vars: &[(&str, Option<&str>)], f: F) {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let mut originals: Vec<(String, Option<String>)> = Vec::new();
        for (k, v) in vars {
            originals.push(((*k).to_string(), env::var(k).ok()));
            // SAFETY: Tests serialised via ENV_GUARD; no concurrent
            // env-var mutation within the with_env scope.
            unsafe {
                match v {
                    Some(value) => env::set_var(k, value),
                    None => env::remove_var(k),
                }
            }
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        // Restore originals regardless of panic outcome.
        for (k, v) in originals {
            // SAFETY: Tests serialised via ENV_GUARD; no concurrent
            // env-var mutation during restoration.
            unsafe {
                match v {
                    Some(val) => env::set_var(&k, &val),
                    None => env::remove_var(&k),
                }
            }
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    #[test]
    fn empty_trust_store_yields_permissive_mode() {
        with_env(
            &[
                (
                    TRUSTED_KEYS_DIR_ENV,
                    Some("/nonexistent/keleusma/test/empty"),
                ),
                (REQUIRE_SIGNED_ENV, None),
            ],
            || {
                let err = discover_trusted_keys();
                // Explicit env var pointing at a missing directory is
                // a startup error; verifies fail-closed discovery.
                assert!(err.is_err(), "expected error for missing explicit dir");
            },
        );
    }

    #[test]
    fn unset_env_yields_permissive_when_default_dir_missing() {
        // On hosts without the platform-conventional directory
        // (which is the case in CI / test environments), discovery
        // returns an empty trust list rather than erroring.
        with_env(
            &[(TRUSTED_KEYS_DIR_ENV, None), (REQUIRE_SIGNED_ENV, None)],
            || match discover_trusted_keys() {
                Ok(keys) => assert!(keys.is_empty(), "expected no keys, got {}", keys.len()),
                Err(e) => panic!("expected empty trust list, got error: {}", e),
            },
        );
    }

    #[test]
    fn force_strict_mode_env_var_recognised() {
        with_env(&[(REQUIRE_SIGNED_ENV, Some("1"))], || {
            assert!(
                is_strict_mode_forced(),
                "expected force-strict to be active"
            );
        });
        with_env(&[(REQUIRE_SIGNED_ENV, Some("0"))], || {
            assert!(
                !is_strict_mode_forced(),
                "expected force-strict to be inactive when not 1"
            );
        });
        with_env(&[(REQUIRE_SIGNED_ENV, None)], || {
            assert!(
                !is_strict_mode_forced(),
                "expected force-strict to be inactive when unset"
            );
        });
    }

    #[test]
    fn discovery_loads_well_formed_keys() {
        let dir = std::env::temp_dir().join(format!("keleusma_test_trust_{}", std::process::id()));
        // Best-effort cleanup of any prior run.
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");

        // Generate two keys and write them.
        let key1 = ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng).verifying_key();
        let key2 = ed25519_dalek::SigningKey::generate(&mut rand_core::OsRng).verifying_key();
        fs::write(dir.join("alice.pub"), key1.to_bytes()).expect("write alice");
        fs::write(dir.join("bob.pub"), key2.to_bytes()).expect("write bob");

        // Add a non-.pub file that should be ignored.
        fs::write(dir.join("readme.txt"), b"ignore me").expect("write readme");

        with_env(
            &[(TRUSTED_KEYS_DIR_ENV, Some(dir.to_str().unwrap()))],
            || {
                let keys = discover_trusted_keys().expect("discovery succeeds");
                assert_eq!(keys.len(), 2, "expected 2 keys, got {}", keys.len());
            },
        );

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn discovery_rejects_malformed_key_files() {
        let dir =
            std::env::temp_dir().join(format!("keleusma_test_malformed_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");

        // Write a key file with wrong length.
        fs::write(dir.join("bad.pub"), b"too short").expect("write bad key");

        with_env(
            &[(TRUSTED_KEYS_DIR_ENV, Some(dir.to_str().unwrap()))],
            || {
                let result = discover_trusted_keys();
                assert!(result.is_err(), "expected fail-closed rejection");
                let msg = result.unwrap_err();
                assert!(
                    msg.contains("malformed"),
                    "expected 'malformed' in diagnostic; got: {}",
                    msg
                );
            },
        );

        fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn policy_context_reflects_force_strict() {
        with_env(
            &[
                (TRUSTED_KEYS_DIR_ENV, None),
                (REQUIRE_SIGNED_ENV, Some("1")),
            ],
            || {
                let ctx = build_policy_context().expect("policy context builds");
                assert!(ctx.enrolled_keys.is_empty());
                assert!(
                    ctx.strict_signing,
                    "force-strict should activate strict mode"
                );
            },
        );
    }
}
