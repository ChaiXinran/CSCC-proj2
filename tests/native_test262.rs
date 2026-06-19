//! End-to-end acceptance gate for the first Native Test262 milestone.

use std::path::PathBuf;

use agentjs::test262::{NATIVE_V1_TESTS, NATIVE_V2_TESTS, RunnerOptions, Status, run};

fn assert_native_gate(options: RunnerOptions, expected_count: usize, milestone: &str) {
    let summary = run(options)
        .unwrap_or_else(|error| panic!("the {milestone} Test262 gate should run: {error}"));
    let failures: Vec<_> = summary
        .cases
        .iter()
        .filter(|case| case.status != Status::Passed)
        .map(|case| format!("{}: {}", case.path.display(), case.detail))
        .collect();

    assert_eq!(summary.total, expected_count);
    assert_eq!(
        summary.passed,
        expected_count,
        "{milestone} Test262 failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.skipped, 0);
}

#[test]
fn native_v1_passes_the_pinned_test262_files() {
    let mut options = RunnerOptions {
        test262_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test262"),
        jobs: 1,
        ..RunnerOptions::default()
    };
    options.select_native_v1();

    assert_native_gate(options, NATIVE_V1_TESTS.len(), "Native V1");
}

#[test]
fn native_v2_passes_the_pinned_test262_files() {
    let mut options = RunnerOptions {
        test262_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test262"),
        jobs: 1,
        ..RunnerOptions::default()
    };
    options.select_native_v2();

    assert_native_gate(options, NATIVE_V2_TESTS.len(), "Native V2");
}
