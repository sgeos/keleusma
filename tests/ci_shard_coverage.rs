//! **EVERY PARTITIONED CONTINUOUS-INTEGRATION JOB RUNS EVERY ONE OF ITS SHARDS.**
//!
//! The workspace and self-host test jobs were split into hash partitions on 2026-10-21,
//! after both were cancelled at GitHub's six-hour job limit on pull request 484. A
//! partitioned job covers its feature set only when the matrix lists each shard from one
//! to the partition count exactly once.
//!
//! `tests/gate_ci_correspondence.rs` cannot see this. It deliberately treats flags other
//! than package, feature, target and test-binary selectors as irrelevant, so a command
//! carrying `--partition hash:1/4` satisfies it exactly as the whole run did. A matrix
//! that lost a shard, or a partition count raised without widening the matrix, would
//! leave that guard green while a quarter of the suite ran nowhere. This file is the
//! check for that case.
//!
//! The documentation tests are not partitioned by nextest, which skips them. Each
//! partitioned job must therefore still run its `--doc` step, and this file checks that
//! the step exists in the job. It does not check that the step runs on exactly one shard.
//!
//! **THE BINARIES THE WORKSPACE JOB FILTERS OUT RUN IN THE SELF-HOST JOB.** On 2026-10-24
//! the workspace job stopped being partitioned and began excluding the `selfhost_*`
//! binaries of `keleusma`, which it had been repeating because `keleusma-cli` enables the
//! `self-host` feature in a `--workspace` build. The correspondence guard treats `-E` as
//! irrelevant too, so it cannot see whether those binaries still run anywhere. A filter
//! widened by accident, or a filter added to the self-host job, would leave them running
//! nowhere with every other check green. This file checks that case as well.
//!
//! **THE WORKSPACE JOB KEEPS ONLY THE `keleusma` TESTS NO OTHER JOB RUNS.** On 2026-10-30
//! the operator ruled that redundant testing be removed barring a good reason, after the
//! kaizen timing pass found 2,446 tests running in both jobs. The workspace job's filter
//! became an allow-list for `keleusma`, the library's tests outside `selfhost::` and the
//! integration binaries whose source names the `encryption`, `signatures` or `shell`
//! feature, the features only that job enables together. A gated file the list omits would
//! run its `encryption` cases nowhere, so this file derives the gated files from `tests/`
//! and requires the filter to name exactly those. The `text` feature joined the list on
//! 2026-10-31, because the workspace job is the only one that enables it.
//!
//! **THE SHARDS RUN ONE ARCHIVE, BUILT ONCE.** On 2026-10-25 the self-host shards stopped
//! compiling the same binaries four times. `test-self-host-build` runs `cargo nextest
//! archive` and each shard runs that archive. The feature selection then lives on the
//! archive command, not on the shard's, so this file checks the archive for `-p keleusma
//! --features self-host` with no filter, and checks that every shard names the same
//! archive file. A shard running a different archive, or an archive built with a filter,
//! would run a different population with every other check green.
//!
//! **THE PINNED LISTS AND THE HASHED REMAINDER COVER EVERY SELF-HOST TEST ONCE.** On
//! 2026-10-31 the operator adopted option (a) of item N6 in
//! `docs/process/kaizen/2026-10-31-open-decisions.md`. Each self-host shard runs the
//! longest tests its `PINNED_<shard>` list names, then the hash partition of every test no
//! list names. A test named in two lists would run twice, and a hashed run that excluded a
//! list missing from its expression would leave that list's tests running nowhere. This
//! file checks that the lists are disjoint, that the hashed run excludes exactly their
//! union, and that each pinned name still appears in the sources. A stale name is not a
//! coverage loss, because the test it no longer names falls into the hashed part, but it
//! is a balance loss nobody would notice, so it fails here.
//!
//! The parse is line-based over a workflow this repository writes, and it is not a
//! general YAML reader. It fails closed: a partitioned command whose shard expression or
//! matrix it cannot read is a failure, not a skip.

use std::path::Path;

/// One job of `ci.yml`: its key and the raw lines of its body.
struct Job {
    key: String,
    lines: Vec<String>,
}

fn ci_jobs() -> Vec<Job> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml");
    let text = std::fs::read_to_string(&path).expect("the continuous-integration workflow");
    let mut jobs: Vec<Job> = Vec::new();
    let mut in_jobs = false;
    for raw in text.lines() {
        if raw == "jobs:" {
            in_jobs = true;
            continue;
        }
        if !in_jobs {
            continue;
        }
        // A job key sits at an indent of exactly two and ends with a colon.
        let is_key = raw.starts_with("  ")
            && !raw.starts_with("   ")
            && raw.trim_end().ends_with(':')
            && !raw.trim_start().starts_with('#');
        if is_key {
            jobs.push(Job {
                key: raw.trim().trim_end_matches(':').to_string(),
                lines: Vec::new(),
            });
        } else if let Some(job) = jobs.last_mut() {
            job.lines.push(raw.to_string());
        }
    }
    jobs
}

/// The partition count named by a `--partition hash:${{ matrix.shard }}/N` argument.
fn partition_count(line: &str) -> Option<Result<usize, String>> {
    let at = line.find("--partition")?;
    // Matched on the remainder of the line, because the space inside
    // `${{ matrix.shard }}` would split a whitespace-delimited argument.
    let whole = line[at + "--partition".len()..].trim_start();
    let Some(rest) = whole.strip_prefix("hash:${{ matrix.shard }}/") else {
        return Some(Err(format!(
            "a partition this check cannot read; expected `hash:${{{{ matrix.shard }}}}/N`: {line}"
        )));
    };
    let n = rest.split_whitespace().next().unwrap_or("");
    Some(
        n.parse()
            .map_err(|_| format!("unreadable partition count in: {line}")),
    )
}

/// The integers of a `shard: [1, 2, ...]` matrix line in this job.
fn shard_matrix(job: &Job) -> Result<Vec<usize>, String> {
    let mut found = None;
    for l in &job.lines {
        let t = l.trim();
        if let Some(list) = t.strip_prefix("shard:") {
            let list = list.trim();
            let inner = list
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
                .ok_or_else(|| format!("job `{}`: shard matrix is not an inline list", job.key))?;
            let values: Result<Vec<usize>, _> = inner
                .split(',')
                .map(|v| v.trim().parse::<usize>())
                .collect();
            let values =
                values.map_err(|_| format!("job `{}`: unreadable shard value", job.key))?;
            if found.replace(values).is_some() {
                return Err(format!("job `{}`: more than one shard matrix", job.key));
            }
        }
    }
    found.ok_or_else(|| format!("job `{}`: partitioned but has no shard matrix", job.key))
}

#[test]
fn every_partitioned_job_lists_each_shard_exactly_once() {
    let jobs = ci_jobs();
    // NON-VACUITY. A parse that found no jobs, or no partitioned jobs, satisfies every
    // check below.
    assert!(
        jobs.len() >= 10,
        "parsed only {} jobs from ci.yml",
        jobs.len()
    );

    let mut partitioned = Vec::new();
    let mut errors = Vec::new();
    for job in &jobs {
        let mut counts = Vec::new();
        for l in &job.lines {
            if l.trim_start().starts_with('#') {
                continue;
            }
            match partition_count(l) {
                Some(Ok(n)) => counts.push(n),
                Some(Err(e)) => errors.push(format!("job `{}`: {e}", job.key)),
                None => {}
            }
        }
        if counts.is_empty() {
            continue;
        }
        partitioned.push(job.key.clone());
        let n = counts[0];
        if counts.iter().any(|&c| c != n) {
            errors.push(format!(
                "job `{}`: partition counts disagree: {counts:?}",
                job.key
            ));
        }
        match shard_matrix(job) {
            Ok(mut shards) => {
                shards.sort_unstable();
                let expected: Vec<usize> = (1..=n).collect();
                if shards != expected {
                    errors.push(format!(
                        "job `{}`: partitions into {n} but the matrix lists {shards:?}",
                        job.key
                    ));
                }
            }
            Err(e) => errors.push(e),
        }
        let has_doc = job
            .lines
            .iter()
            .any(|l| !l.trim_start().starts_with('#') && l.contains("--doc"));
        if !has_doc {
            errors.push(format!(
                "job `{}`: partitioned, and its documentation-test step is gone",
                job.key
            ));
        }
    }

    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert!(
        partitioned.contains(&"test-self-host".to_string()),
        "expected the self-host test job to be partitioned; found {partitioned:?}"
    );
}

/// The workspace job's filter, exactly. Narrowing it would drop tests from the only job
/// that ran them, so a change here is meant to fail this file and be reviewed.
const WORKSPACE_FILTER: &str = "-E 'not package(keleusma) | (binary(=keleusma) & not test(/^selfhost::/)) | binary(=crypto_framing) | binary(=ecc_signature_ordering) | binary(=fuzz_corpus_replay) | binary(=hot_swap_trust_matrix) | binary(=native_declaration_agreement) | binary(=proven_execution) | binary(=signature_verify_bounds) | binary(=stage_executor) | binary(=text_ops) | binary(=text_ops_without_feature) | binary(=trap_policy)'";

/// The features that only the workspace job enables on `keleusma` together, through
/// `keleusma-cli`. A test gated on one of them may run nowhere else in that combination.
const WORKSPACE_ONLY_FEATURES: [&str; 4] = ["encryption", "signatures", "shell", "text"];

/// The `keleusma` integration binaries whose source names a workspace-only feature in a
/// `feature = "..."` predicate, sorted. Comments count too, which errs toward keeping a
/// binary in the workspace job rather than dropping one.
fn gated_binaries() -> Vec<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("tests/ is readable") {
        let path = entry.expect("a tests/ entry is readable").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a test file is UTF-8");
        let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
        if WORKSPACE_ONLY_FEATURES
            .iter()
            .any(|f| compact.contains(&format!("feature=\"{f}\"")))
        {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .expect("a UTF-8 name");
            found.push(stem.to_string());
        }
    }
    found.sort();
    found
}

/// The `binary(=name)` names a filter allows, other than the library's, sorted.
fn allowed_binaries(filter: &str) -> Vec<String> {
    let mut names: Vec<String> = filter
        .split("binary(=")
        .skip(1)
        .filter_map(|rest| rest.split(')').next())
        .filter(|name| *name != "keleusma")
        .map(str::to_string)
        .collect();
    names.sort();
    names
}

#[test]
fn the_workspace_job_keeps_every_binary_gated_on_its_own_features() {
    let gated = gated_binaries();
    assert!(
        gated.len() >= 8,
        "found only {gated:?}; the scan of tests/ may have stopped matching"
    );
    assert_eq!(
        allowed_binaries(WORKSPACE_FILTER),
        gated,
        "the workspace job's filter must name exactly the keleusma integration binaries \
         whose source names {WORKSPACE_ONLY_FEATURES:?}"
    );
    assert!(
        WORKSPACE_FILTER.contains("(binary(=keleusma) & not test(/^selfhost::/))"),
        "the library's tests outside selfhost:: hold its encryption-gated unit tests"
    );
}

/// The list comparison detects a gated binary the filter omits.
#[test]
fn the_allow_list_check_detects_an_omitted_binary() {
    let omitted = WORKSPACE_FILTER.replace(" | binary(=trap_policy)", "");
    assert_ne!(allowed_binaries(&omitted), gated_binaries());
    assert_eq!(
        allowed_binaries("-E 'binary(=keleusma) | binary(=b) | binary(=a)'"),
        ["a", "b"]
    );
}

/// The non-comment `cargo nextest run` commands of one job.
fn nextest_commands(job: &Job) -> Vec<&str> {
    job.lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.starts_with('#') && l.contains("cargo nextest run"))
        .collect()
}

/// Every `selfhost_*` integration-test binary outside `keleusma`'s own `tests/`. The
/// workspace filter names `package(keleusma)`, so a binary with that prefix in another
/// package would still run in the workspace job. It would also not run in the self-host
/// job, which selects `-p keleusma` only. Neither job would be wrong, but the split this
/// file guards would no longer describe where the heavy tests run.
fn foreign_selfhost_binaries() -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    let members = [
        "keleusma-arena",
        "keleusma-bench",
        "keleusma-cli",
        "keleusma-macros",
        "keleusma-wire",
        "keleusma-wire-derive",
    ];
    for member in members {
        let dir = root.join(member).join("tests");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("selfhost") {
                found.push(format!("{member}/tests/{name}"));
            }
        }
    }
    found
}

/// The non-comment `cargo nextest archive` commands of one job.
fn archive_commands(job: &Job) -> Vec<&str> {
    job.lines
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.starts_with('#') && l.contains("cargo nextest archive"))
        .collect()
}

/// The file named by `--archive-file`, when the command has one.
fn archive_file(command: &str) -> Option<&str> {
    command
        .split("--archive-file")
        .nth(1)?
        .split_whitespace()
        .next()
}

/// A filter or filtering profile, which could drop a binary the workspace job left to
/// the self-host job.
fn filters(command: &str) -> bool {
    command.contains(" -E ")
        || command.contains("--filterset")
        || command.contains("--profile quick")
}

/// The filter of the self-host shard's pinned run.
const PINNED_FILTER: &str = "-E \"$PINNED\"";

/// The filter of the self-host shard's hashed run, which excludes all `n` pinned lists.
fn hashed_filter(n: usize) -> String {
    let lists: Vec<String> = (1..=n).map(|k| format!("$PINNED_{k}")).collect();
    format!("-E \"not ({})\"", lists.join(" | "))
}

/// Whether the command's only filter is one half of the pinned split, which
/// `pinned_split_errors` checks covers every test once.
fn is_pinned_split(command: &str) -> bool {
    let mut rest = command.to_string();
    for n in 1..=8 {
        rest = rest.replace(&hashed_filter(n), "");
    }
    rest = rest.replace(PINNED_FILTER, "");
    rest != command && !filters(&rest)
}

/// Why the workspace and self-host jobs no longer split the suite as intended, or nothing.
/// `build` is the archive job, when the shards run an archive.
fn split_errors(workspace: &Job, self_host: &Job, build: Option<&Job>) -> Vec<String> {
    let mut errors = Vec::new();
    let commands = nextest_commands(workspace);
    if commands.len() != 1 {
        errors.push(format!(
            "expected one nextest command in `{}`, found {commands:?}",
            workspace.key
        ));
    } else if !(commands[0].contains("--workspace") && commands[0].ends_with(WORKSPACE_FILTER)) {
        errors.push(format!(
            "the workspace job's filter changed; review it against the self-host job: {}",
            commands[0]
        ));
    }

    let selects =
        |command: &str| command.contains("-p keleusma") && command.contains("--features self-host");
    // The archive, when there is one, carries the selection the shards run.
    let mut archive = None;
    if let Some(build) = build {
        let commands = archive_commands(build);
        if commands.len() != 1 {
            errors.push(format!(
                "expected one nextest archive command in `{}`, found {commands:?}",
                build.key
            ));
        } else if !selects(commands[0]) || filters(commands[0]) {
            errors.push(format!(
                "the self-host archive must select `-p keleusma --features self-host` \
                 unfiltered: {}",
                commands[0]
            ));
        } else {
            archive = archive_file(commands[0]);
            if archive.is_none() {
                errors.push(format!(
                    "the archive command names no file: {}",
                    commands[0]
                ));
            }
        }
    }

    let commands = nextest_commands(self_host);
    if commands.is_empty() {
        errors.push(format!("`{}` runs no nextest command", self_host.key));
    }
    for command in commands {
        match archive_file(command) {
            Some(file) if Some(file) != archive => errors.push(format!(
                "a self-host shard runs `{file}`, which is not the archive the build job \
                 makes: {command}"
            )),
            Some(_) => {}
            None if !selects(command) => errors.push(format!(
                "a self-host command no longer selects `-p keleusma --features self-host`: {command}"
            )),
            None => {}
        }
        if filters(command) && !is_pinned_split(command) {
            errors.push(format!(
                "the self-host job filters its tests, so the binaries the workspace job \
                 leaves to it may run nowhere: {command}"
            ));
        }
    }
    errors
}

#[test]
fn binaries_the_workspace_job_filters_out_run_in_the_self_host_job() {
    let jobs = ci_jobs();
    let job = |key: &str| {
        jobs.iter()
            .find(|j| j.key == key)
            .unwrap_or_else(|| panic!("ci.yml has no `{key}` job"))
    };
    let errors = split_errors(
        job("test"),
        job("test-self-host"),
        Some(job("test-self-host-build")),
    );
    assert!(errors.is_empty(), "{}", errors.join("\n"));

    let foreign = foreign_selfhost_binaries();
    assert!(
        foreign.is_empty(),
        "selfhost binaries outside `keleusma` escape the split: {foreign:?}"
    );
}

/// The split check detects a widened workspace filter and a filtered self-host job.
#[test]
fn the_split_check_rejects_a_widened_filter_and_a_filtered_self_host_job() {
    let job = |key: &str, run: &str| Job {
        key: key.to_string(),
        lines: vec![format!("      - run: {run}")],
    };
    let good_workspace = job(
        "test",
        &format!("cargo nextest run --profile ci --workspace {WORKSPACE_FILTER}"),
    );
    let good_self_host = job(
        "test-self-host",
        "cargo nextest run --profile ci -p keleusma --features self-host",
    );
    assert!(split_errors(&good_workspace, &good_self_host, None).is_empty());

    let changed = job(
        "test",
        "cargo nextest run --profile ci --workspace -E 'not binary(~selfhost)'",
    );
    assert_eq!(split_errors(&changed, &good_self_host, None).len(), 1);

    let filtered = job(
        "test-self-host",
        "cargo nextest run --profile ci -p keleusma --features self-host -E 'not test(slow)'",
    );
    assert_eq!(split_errors(&good_workspace, &filtered, None).len(), 1);

    let dropped = job(
        "test-self-host",
        "cargo test -p keleusma --features self-host --doc",
    );
    assert_eq!(split_errors(&good_workspace, &dropped, None).len(), 1);

    // The archive form. A good archive and a shard that runs it pass. A filtered
    // archive, an archive missing the feature, and a shard running another file fail.
    let build = |run: &str| job("test-self-host-build", run);
    let shard = job(
        "test-self-host",
        "cargo nextest run --profile ci --archive-file a.tar.zst --workspace-remap .",
    );
    let good_build =
        build("cargo nextest archive -p keleusma --features self-host --archive-file a.tar.zst");
    assert!(split_errors(&good_workspace, &shard, Some(&good_build)).is_empty());
    for bad in [
        "cargo nextest archive -p keleusma --features self-host -E 'not test(slow)' --archive-file a.tar.zst",
        "cargo nextest archive -p keleusma --archive-file a.tar.zst",
    ] {
        assert!(
            !split_errors(&good_workspace, &shard, Some(&build(bad))).is_empty(),
            "{bad}"
        );
    }
    let other = job(
        "test-self-host",
        "cargo nextest run --profile ci --archive-file b.tar.zst --workspace-remap .",
    );
    assert_eq!(
        split_errors(&good_workspace, &other, Some(&good_build)).len(),
        1
    );
}

/// The checker itself detects the two failures it exists for. A guard that cannot fail
/// is not evidence.
#[test]
fn the_shard_check_rejects_a_missing_shard_and_a_foreign_partition() {
    let job = Job {
        key: "t".to_string(),
        lines: vec![
            "    strategy:".to_string(),
            "      matrix:".to_string(),
            "        shard: [1, 2, 4]".to_string(),
            "      - run: cargo nextest run --workspace --partition hash:${{ matrix.shard }}/4"
                .to_string(),
        ],
    };
    let n = partition_count(&job.lines[3]).unwrap().unwrap();
    assert_eq!(n, 4);
    let mut shards = shard_matrix(&job).unwrap();
    shards.sort_unstable();
    assert_ne!(
        shards,
        (1..=n).collect::<Vec<_>>(),
        "a missing shard must be visible"
    );

    assert!(
        partition_count("cargo nextest run --partition count:1/4")
            .unwrap()
            .is_err()
    );
    assert!(partition_count("cargo nextest run --workspace").is_none());
}

/// The `PINNED_<k>` lists of a job's `env`, in shard order, each as its test names. Each
/// list is a folded scalar, `PINNED_<k>: >-`, whose more-indented lines are `test(=name)`
/// terms joined by `|`.
fn pinned_lists(job: &Job) -> Result<Vec<(usize, Vec<String>)>, String> {
    let mut lists = Vec::new();
    let mut lines = job.lines.iter().peekable();
    while let Some(line) = lines.next() {
        let t = line.trim_start();
        let Some(rest) = t.strip_prefix("PINNED_") else {
            continue;
        };
        let Some(k) = rest.strip_suffix(": >-") else {
            return Err(format!("a pinned list this check cannot read: {line}"));
        };
        let k: usize = k
            .parse()
            .map_err(|_| format!("an unreadable pinned list key: {line}"))?;
        let indent = line.len() - t.len();
        let mut body = String::new();
        while let Some(next) = lines.peek() {
            let nt = next.trim_start();
            if nt.is_empty() || next.len() - nt.len() <= indent {
                break;
            }
            body.push(' ');
            body.push_str(nt);
            lines.next();
        }
        let mut names = Vec::new();
        for term in body.split('|') {
            let name = term
                .trim()
                .strip_prefix("test(=")
                .and_then(|n| n.strip_suffix(')'))
                .filter(|n| !n.is_empty() && !n.contains(['(', ')', ' ']))
                .ok_or_else(|| {
                    format!(
                        "PINNED_{k}: a term other than `test(=name)`: `{}`",
                        term.trim()
                    )
                })?;
            names.push(name.to_string());
        }
        lists.push((k, names));
    }
    Ok(lists)
}

/// The text of every `.rs` file under `tests/` and `src/`, which a pinned name's segments
/// must appear in.
fn test_sources() -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut dirs = vec![root.join("tests"), root.join("src")];
    let mut text = String::new();
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).expect("a source directory is readable") {
            let path = entry.expect("a source entry is readable").path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                text.push_str(&std::fs::read_to_string(&path).expect("a source file is UTF-8"));
                text.push('\n');
            }
        }
    }
    text
}

/// Whether `word` occurs in `text` as a whole identifier.
fn has_word(text: &str, word: &str) -> bool {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
}

/// Why the self-host job's pinned lists and hashed run no longer cover every test once,
/// or nothing. `sources` is the text the pinned names must appear in.
fn pinned_split_errors(job: &Job, sources: &str) -> Vec<String> {
    let mut errors = Vec::new();
    let lists = match pinned_lists(job) {
        Ok(lists) => lists,
        Err(e) => return vec![e],
    };
    let n = match shard_matrix(job) {
        Ok(shards) => shards.len(),
        Err(e) => return vec![e],
    };
    let keys: Vec<usize> = lists.iter().map(|(k, _)| *k).collect();
    if keys != (1..=n).collect::<Vec<_>>() {
        errors.push(format!(
            "the job has {n} shards but pinned lists {keys:?}; each shard needs one list, in order"
        ));
    }
    let mut seen: Vec<&str> = Vec::new();
    for (k, names) in &lists {
        if names.is_empty() {
            errors.push(format!("PINNED_{k} names no test"));
        }
        for name in names {
            if seen.contains(&name.as_str()) {
                errors.push(format!("`{name}` is pinned twice, so it would run twice"));
            }
            seen.push(name);
            for segment in name.split("::") {
                if !has_word(sources, segment) {
                    errors.push(format!(
                        "PINNED_{k} names `{name}`, and `{segment}` appears in no source; \
                         re-measure and update the list"
                    ));
                }
            }
        }
    }

    let commands = nextest_commands(job);
    let pinned: Vec<&&str> = commands
        .iter()
        .filter(|c| c.contains(PINNED_FILTER))
        .collect();
    let hashed: Vec<&&str> = commands.iter().filter(|c| c.contains("$PINNED_")).collect();
    if pinned.len() != 1 || pinned[0].contains("--partition") {
        errors.push(format!(
            "expected one unpartitioned run with `{PINNED_FILTER}`, found {pinned:?}"
        ));
    }
    let want = hashed_filter(n);
    if hashed.len() != 1 || !hashed[0].contains(&want) || partition_count(hashed[0]).is_none() {
        errors.push(format!(
            "expected one partitioned run with `{want}`, found {hashed:?}"
        ));
    }
    let selects_shard = "PINNED: ${{ env[format('PINNED_{0}', matrix.shard)] }}";
    if !job.lines.iter().any(|l| l.trim() == selects_shard) {
        errors.push(format!(
            "the pinned run must take its list from `{selects_shard}`"
        ));
    }
    errors
}

#[test]
fn the_pinned_lists_and_the_hashed_run_cover_every_self_host_test_once() {
    let jobs = ci_jobs();
    let job = jobs
        .iter()
        .find(|j| j.key == "test-self-host")
        .expect("ci.yml has no `test-self-host` job");
    // NON-VACUITY. A parse that found no names satisfies the disjointness check.
    let lists = pinned_lists(job).expect("the pinned lists parse");
    let total: usize = lists.iter().map(|(_, names)| names.len()).sum();
    assert!(total >= 20, "parsed only {total} pinned names");
    let errors = pinned_split_errors(job, &test_sources());
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

/// The pinned-split check detects a doubly pinned name, a stale name, and a hashed run
/// that omits a list.
#[test]
fn the_pinned_split_check_rejects_overlap_staleness_and_an_omitted_list() {
    let job = |a: &str, hashed: &str| Job {
        key: "test-self-host".to_string(),
        lines: [
            "    strategy:",
            "      matrix:",
            "        shard: [1, 2]",
            "    env:",
            "      PINNED_1: >-",
            "        test(=alpha) |",
            "        test(=mod_b::beta)",
            "      PINNED_2: >-",
            a,
            "    steps:",
            "      - name: Run the tests pinned to this shard",
            "        env:",
            "          PINNED: ${{ env[format('PINNED_{0}', matrix.shard)] }}",
            "        run: cargo nextest run --archive-file a.tar.zst -E \"$PINNED\"",
            hashed,
        ]
        .map(str::to_string)
        .to_vec(),
    };
    let sources = "fn alpha() {} mod mod_b { fn beta() {} } fn gamma() {}";
    let good_hashed = "        run: cargo nextest run --archive-file a.tar.zst -E \"not ($PINNED_1 | $PINNED_2)\" --partition hash:${{ matrix.shard }}/2";
    let good = job("        test(=gamma)", good_hashed);
    assert_eq!(pinned_split_errors(&good, sources), Vec::<String>::new());
    assert!(is_pinned_split(good.lines[13].trim()));
    assert!(is_pinned_split(good_hashed.trim()));

    assert_eq!(
        pinned_split_errors(&job("        test(=alpha)", good_hashed), sources).len(),
        1
    );
    assert_eq!(
        pinned_split_errors(&job("        test(=delta)", good_hashed), sources).len(),
        1
    );
    let omitted = "        run: cargo nextest run --archive-file a.tar.zst -E \"not ($PINNED_1)\" --partition hash:${{ matrix.shard }}/2";
    assert_eq!(
        pinned_split_errors(&job("        test(=gamma)", omitted), sources).len(),
        1
    );
    assert_eq!(
        pinned_split_errors(&job("        test(~gamma)", good_hashed), sources).len(),
        1
    );
    // A pinned filter with anything else added is a filter, not the split.
    assert!(!is_pinned_split(
        "cargo nextest run -E \"$PINNED\" -E 'not test(slow)'"
    ));
}

/// **THE REQUIRED CHECK SEES EVERY JOB.** `main` requires `CI result`, which fails when any
/// job it needs failed. A job missing from its `needs` could fail while the required check
/// passed, so every other job of `ci.yml` must be listed there.
#[test]
fn the_ci_result_job_needs_every_other_job() {
    let jobs = ci_jobs();
    let result = jobs
        .iter()
        .find(|j| j.key == "ci-result")
        .expect("ci.yml has no `ci-result` job");
    let needs_line = result
        .lines
        .iter()
        .map(|l| l.trim())
        .find(|l| l.starts_with("needs:"))
        .expect("`ci-result` has no needs line");
    let needs: Vec<&str> = needs_line
        .trim_start_matches("needs:")
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(str::trim)
        .collect();
    let missing: Vec<&str> = jobs
        .iter()
        .map(|j| j.key.as_str())
        .filter(|k| *k != "ci-result" && !needs.contains(k))
        .collect();
    assert!(
        jobs.len() >= 10,
        "parsed only {} jobs from ci.yml",
        jobs.len()
    );
    assert!(
        missing.is_empty(),
        "`ci-result` does not need {missing:?}, so their failures would not fail the required check"
    );
}
