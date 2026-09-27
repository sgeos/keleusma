//! **A DIRECTORY WITH ITS OWN TOOLCHAIN PIN DECLARES THE COMPONENTS ITS CI COMMANDS NEED.**
//!
//! This is a cross-level property, and it is the one that actually failed on 2026-09-27. The
//! `rtos-host` continuous-integration job installs a toolchain **with** `clippy`, and it still
//! failed, because `examples/rtos/rust-toolchain.toml` overrides the job's toolchain for
//! anything run inside that directory and its component list did not name `clippy`. The
//! runner auto-installed the pinned channel with its seven declared components and then said,
//! verbatim:
//!
//! ```text
//! error: 'cargo-clippy' is not installed for the toolchain '1.92-x86_64-unknown-linux-gnu'.
//! ```
//!
//! It had passed on a developer machine that happened to have clippy installed for that
//! channel. So the crate was never lintable from a clean checkout, and `cargo fmt --check`
//! passed throughout, because `rustfmt` **is** declared — which is exactly why the omission was
//! invisible.
//!
//! **A CHECK AT JOB LEVEL WOULD NOT HAVE CAUGHT IT, AND THAT WAS MEASURED.** Sweeping every
//! job in both workflows for "runs clippy or rustfmt but does not declare it" found **zero**
//! offenders. The declaration was present; the pin overrode it. Only a check that crosses the
//! two levels sees this.
//!
//! **THE POPULATION IS ONE DIRECTORY.** Said plainly so this file's scope is not mistaken for
//! broader coverage: `examples/rtos` is the only tracked directory carrying a pin, and it is
//! correct as of this file. The guard exists so that a second pinned crate, or a trimmed pin,
//! fails here rather than on a runner.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Every tracked `rust-toolchain.toml`, as a path relative to the repository root.
///
/// `target/` holds build output and `tmp/` is gitignored scratch space — it contains a vendored
/// scaffold with its own pin that is not part of this project, and including it would produce a
/// finding about a directory no workflow touches.
fn pinned_dirs() -> Vec<(String, String)> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<(String, String)>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if p.is_dir() {
                if matches!(name.as_str(), "target" | "tmp" | ".git" | "node_modules") {
                    continue;
                }
                walk(&p, base, out);
            } else if name.starts_with("rust-toolchain") {
                let rel = p
                    .strip_prefix(base)
                    .unwrap_or(&p)
                    .parent()
                    .map(|d| d.to_string_lossy().to_string())
                    .unwrap_or_default();
                let text = std::fs::read_to_string(&p).expect("read a toolchain pin");
                out.push((rel, text));
            }
        }
    }
    let mut out = Vec::new();
    let base = root();
    walk(&base, &base, &mut out);
    out.sort();
    out
}

/// The `components = [...]` entries a pin declares.
fn declared_components(pin: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    // Skip comment lines so a component named only in prose cannot satisfy this. The
    // repository's comment-matching sweep records nine guards that read a comment as code.
    for line in pin.lines() {
        let t = line.trim();
        if t.starts_with('#') {
            continue;
        }
        if let Some(rest) = t.strip_prefix("components") {
            let Some(open) = rest.find('[') else { continue };
            let seg = &rest[open + 1..];
            let seg = seg.split(']').next().unwrap_or(seg);
            for part in seg.split(',') {
                let c = part.trim().trim_matches('"').trim();
                if !c.is_empty() {
                    out.insert(c.to_string());
                }
            }
        }
    }
    out
}

/// Which cargo subcommands a workflow runs **inside** `dir`.
///
/// A `run:` body is a shell script: a `cd <dir>` applies to every later line of that same
/// body. So the scan tracks the current directory within each body and attributes each cargo
/// invocation to it. Comment lines are skipped.
fn subcommands_run_in(dir: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let wf_dir = root().join(".github/workflows");
    let Ok(entries) = std::fs::read_dir(&wf_dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.extension().is_some_and(|x| x == "yml" || x == "yaml") {
            continue;
        }
        let text = std::fs::read_to_string(&p).expect("read a workflow");
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0usize;
        while i < lines.len() {
            let t = lines[i].trim();
            let t = t.strip_prefix("- ").map(str::trim_start).unwrap_or(t);
            if let Some(after) = t.strip_prefix("run:") {
                let a = after.trim();
                let indent = lines[i].len() - lines[i].trim_start().len();
                let mut body: Vec<&str> = Vec::new();
                if a == "|" || a == ">" || a == "|-" || a == ">-" {
                    let mut j = i + 1;
                    while j < lines.len() {
                        if lines[j].trim().is_empty() {
                            j += 1;
                            continue;
                        }
                        let ind = lines[j].len() - lines[j].trim_start().len();
                        if ind <= indent {
                            break;
                        }
                        body.push(lines[j].trim());
                        j += 1;
                    }
                    i = j;
                } else {
                    body.push(a);
                    i += 1;
                }
                // Walk the body tracking the working directory.
                let mut cwd = String::new();
                for line in body {
                    if line.starts_with('#') {
                        continue;
                    }
                    if let Some(d) = line.strip_prefix("cd ") {
                        cwd = d.trim().trim_matches('"').to_string();
                        continue;
                    }
                    if cwd != dir {
                        continue;
                    }
                    for sub in ["fmt", "clippy", "miri"] {
                        let needle = format!("cargo {sub}");
                        let plus = "cargo +";
                        if line.contains(&needle)
                            || (line.contains(plus) && line.contains(&format!(" {sub} ")))
                        {
                            out.insert(sub.to_string());
                        }
                    }
                }
                continue;
            }
            i += 1;
        }
    }
    out
}

/// The component each subcommand requires.
fn component_for(sub: &str) -> &'static str {
    match sub {
        "fmt" => "rustfmt",
        "clippy" => "clippy",
        "miri" => "miri",
        other => panic!("no component mapping for `{other}`"),
    }
}

/// **EVERY PIN DECLARES WHAT THE WORKFLOWS RUN THERE.**
#[test]
fn every_toolchain_pin_declares_the_components_its_workflow_commands_need() {
    let pins = pinned_dirs();

    // NON-VACUITY. A walk that found no pin would satisfy the loop below trivially, and this
    // file's whole subject would silently vanish.
    assert!(
        !pins.is_empty(),
        "no rust-toolchain pin was found, so the walk has broken rather than the repository \
         having dropped its pins"
    );

    let mut problems = Vec::new();
    let mut checked_any_command = false;
    for (dir, pin) in &pins {
        let declared = declared_components(pin);
        assert!(
            !declared.is_empty(),
            "{dir}/rust-toolchain.toml declares no components at all, which the parser should \
             not produce for a pin that has a components line"
        );
        let subs = subcommands_run_in(dir);
        if !subs.is_empty() {
            checked_any_command = true;
        }
        for sub in &subs {
            let comp = component_for(sub);
            if !declared.contains(comp) {
                problems.push(format!(
                    "{dir} runs `cargo {sub}` in continuous integration but its pin does not \
                     declare `{comp}`. The job's own toolchain installation does not help: the \
                     pin overrides it for anything run in that directory."
                ));
            }
        }
    }

    // NON-VACUITY on the other side. If the workflow scan attributed no command to any pinned
    // directory, every pin passes for the wrong reason.
    assert!(
        checked_any_command,
        "no workflow command was attributed to any pinned directory, so the scan has broken. \
         At least one pinned directory is linted and tested by continuous integration."
    );

    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
