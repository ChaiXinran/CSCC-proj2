//! End-to-end acceptance gate for the first Native Test262 milestone.

use std::path::PathBuf;

use agentjs::test262::{NATIVE_V1_TESTS, RunnerOptions, Status, run};

#[test]
fn native_v1_passes_the_pinned_test262_files() {
    let mut options = RunnerOptions {
        test262_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test262"),
        jobs: 1,
        ..RunnerOptions::default()
    };
    options.select_native_v1();

    let summary = run(options).expect("the Native V1 Test262 gate should run");
    let failures: Vec<_> = summary
        .cases
        .iter()
        .filter(|case| case.status != Status::Passed)
        .map(|case| format!("{}: {}", case.path.display(), case.detail))
        .collect();

    assert_eq!(summary.total, NATIVE_V1_TESTS.len());
    assert_eq!(
        summary.passed,
        NATIVE_V1_TESTS.len(),
        "Native V1 Test262 failures:\n{}",
        failures.join("\n")
    );
    assert_eq!(summary.failed, 0);
    assert_eq!(summary.skipped, 0);
}
