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
        partitioned.contains(&"test".to_string())
            && partitioned.contains(&"test-self-host".to_string()),
        "expected the workspace and self-host test jobs to be partitioned; found {partitioned:?}"
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
