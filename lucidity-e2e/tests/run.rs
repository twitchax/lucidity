//! Runs lucidity examples under a plain `lunatic run`, without a control node: local jobs work, remote ones are skipped.

use std::process::{Command, Output};

mod common;

use common::{example, lunatic};

#[test]
fn local_jobs_run_without_a_cluster() {
    let output = run(&example("pythagorean"));
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "{output:?}");
    assert!(stdout.contains("local: ok"), "{output:?}");
    assert!(stdout.contains("remote: skipped"), "{output:?}");
}

#[test]
fn a_dead_local_job_fails_the_run() {
    let output = run(&example("job_dies"));
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "{output:?}");
    assert!(stderr.contains("job `spin` on node 0 died before returning a result"), "{output:?}");
}

fn run(wasm: &std::path::Path) -> Output {
    Command::new(lunatic())
        .arg("run")
        .arg(wasm)
        // Otherwise `lunatic` may think it's running as a `cargo test` runner.
        .env_remove("CARGO_MANIFEST_DIR")
        .output()
        .unwrap()
}
