//! Native Test262 reporting helpers.
//!
//! These tests are ignored on purpose: they are long-running reporting tools,
//! not normal pass/fail CI gates.
//!
//! # Commands
//!
//! Top-level dashboard for all of `test262/test`:
//!
//! ```text
//! cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_top_level -- --ignored --nocapture
//! ```
//!
//! Child-directory dashboard for a hotspot suite, defaulting to `test/built-ins`:
//!
//! ```text
//! cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_dashboard_children -- --ignored --nocapture
//! ```
//!
//! Failure samples for one focused suite, defaulting to `test/built-ins/Symbol`:
//!
//! ```text
//! cargo test --release --no-default-features --test native_full_test262_by_dir native_test262_failure_samples -- --ignored --nocapture
//! ```
//!
//! # Environment variables
//!
//! - `AGENTJS_TEST262_JOBS`: worker count, defaults to `4`.
//! - `AGENTJS_TEST262_SUITE`: suite used by child/samples modes.
//! - `AGENTJS_TEST262_REPORT`: output JSON path.
//! - `AGENTJS_TEST262_SAMPLE_LIMIT`: failure sample cap for samples mode,
//!   defaults to `100`.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use agentjs::{
    test262::{run, CaseResult, RunnerOptions, Status},
    BackendKind,
};

const DEFAULT_JOBS: usize = 4;
const DEFAULT_SAMPLE_LIMIT: usize = 100;

#[derive(Debug)]
struct SuiteReport {
    suite: String,
    status: SuiteStatus,
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    conformance_percent: f64,
    elapsed_ms: u128,
    detail: String,
    failure_samples: Vec<FailureSample>,
}

#[derive(Debug, Clone, Copy)]
enum SuiteStatus {
    Completed,
    Crashed,
}

#[derive(Debug)]
struct FailureSample {
    path: String,
    status: Status,
    detail: String,
    elapsed_ms: u128,
}

#[derive(Debug)]
struct CliSummary {
    total: usize,
    passed: usize,
    failed: usize,
    skipped: usize,
    conformance_percent: f64,
    elapsed_ms: u128,
}

#[test]
#[ignore = "full Test262 dashboard is long-running; run explicitly when needed"]
fn native_test262_dashboard_top_level() {
    let config = Config::new(
        "reports/native-full-test262-summary.json",
        "per-top-level-directory",
    );
    let suites = direct_child_suites(&config.test262_root.join("test"), Path::new("test"));
    assert!(
        !suites.is_empty(),
        "expected top-level suites below {}",
        config.test262_root.join("test").display()
    );

    run_dashboard(config, suites);
}

#[test]
#[ignore = "hotspot Test262 dashboard is long-running; run explicitly when needed"]
fn native_test262_dashboard_children() {
    let config = Config::new(
        "reports/native-hotspot-test262-summary.json",
        "per-child-directory",
    );
    let suite = env::var("AGENTJS_TEST262_SUITE").unwrap_or_else(|_| "test/built-ins".into());
    let absolute_suite = config.test262_root.join(PathBuf::from(&suite));
    let mut suites = direct_child_suites(&absolute_suite, Path::new(&suite));
    if suites.is_empty() {
        suites.push(PathBuf::from(&suite));
    }

    run_dashboard(config, suites);
}

#[test]
#[ignore = "failure sampling can be long-running; run explicitly for a focused suite"]
fn native_test262_failure_samples() {
    let config = Config::new(
        "reports/native-test262-failure-samples.json",
        "focused-failure-samples",
    );
    let suite =
        env::var("AGENTJS_TEST262_SUITE").unwrap_or_else(|_| "test/built-ins/Symbol".into());
    let sample_limit = env_usize("AGENTJS_TEST262_SAMPLE_LIMIT", DEFAULT_SAMPLE_LIMIT);
    let started = Instant::now();

    let options = RunnerOptions {
        test262_root: config.test262_root.clone(),
        suite: PathBuf::from(&suite),
        jobs: config.jobs,
        backend: BackendKind::Native,
        skip_unsupported: true,
        ..RunnerOptions::default()
    };

    println!("sampling failures from {suite} ...");
    let summary =
        run(options).unwrap_or_else(|error| panic!("Test262 suite `{suite}` should run: {error}"));
    let failure_samples = summary
        .cases
        .iter()
        .filter(|case| case.status != Status::Passed)
        .take(sample_limit)
        .map(|case| failure_sample(&config.test262_root, case))
        .collect::<Vec<_>>();

    let report = SuiteReport {
        suite: normalize_path(Path::new(&suite)),
        status: SuiteStatus::Completed,
        total: summary.total,
        passed: summary.passed,
        failed: summary.failed,
        skipped: summary.skipped,
        conformance_percent: summary.conformance_percent(),
        elapsed_ms: summary.elapsed.as_millis(),
        detail: format!("failure_sample_limit={sample_limit}"),
        failure_samples,
    };

    write_report(
        &config.report_path,
        true,
        &config,
        started.elapsed(),
        &[report],
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to write failure sample report `{}`: {error}",
            config.report_path.display()
        )
    });

    println!("report={}", config.report_path.display());
}

fn run_dashboard(config: Config, suites: Vec<PathBuf>) {
    let started = Instant::now();
    let mut reports = Vec::new();

    write_report(
        &config.report_path,
        false,
        &config,
        started.elapsed(),
        &reports,
    )
    .unwrap_or_else(|error| panic!("failed to initialize report: {error}"));

    for suite in suites {
        let suite_display = normalize_path(&suite);
        println!("running {suite_display} ...");

        let report = run_suite_in_child(&config, &suite);
        println!(
            "{} => status={} total={} passed={} failed={} skipped={} conformance={:.2}%",
            report.suite,
            suite_status_label(report.status),
            report.total,
            report.passed,
            report.failed,
            report.skipped,
            report.conformance_percent
        );
        if !report.detail.is_empty() {
            println!("  {}", report.detail);
        }

        reports.push(report);
        write_report(
            &config.report_path,
            false,
            &config,
            started.elapsed(),
            &reports,
        )
        .unwrap_or_else(|error| {
            panic!(
                "failed to update report `{}`: {error}",
                config.report_path.display()
            )
        });
    }

    write_report(
        &config.report_path,
        true,
        &config,
        started.elapsed(),
        &reports,
    )
    .unwrap_or_else(|error| panic!("failed to finalize report: {error}"));

    let total = reports.iter().map(|report| report.total).sum::<usize>();
    let passed = reports.iter().map(|report| report.passed).sum::<usize>();
    let failed = reports.iter().map(|report| report.failed).sum::<usize>();
    let skipped = reports.iter().map(|report| report.skipped).sum::<usize>();
    println!(
        "final => total={total} passed={passed} failed={failed} skipped={skipped} conformance={:.2}% report={}",
        percent(passed, total),
        config.report_path.display()
    );
}

fn run_suite_in_child(config: &Config, suite: &Path) -> SuiteReport {
    let suite_display = normalize_path(suite);
    let tmp_json = config
        .tmp_dir
        .join(format!("{}.json", sanitize(&suite_display)));
    let _ = fs::remove_file(&tmp_json);

    let output = Command::new(env!("CARGO_BIN_EXE_agentjs"))
        .arg("test262")
        .arg("--backend")
        .arg("native")
        .arg("--root")
        .arg(&config.test262_root)
        .arg("--suite")
        .arg(suite)
        .arg("--jobs")
        .arg(config.jobs.to_string())
        .arg("--json")
        .arg(&tmp_json)
        .output()
        .unwrap_or_else(|error| panic!("failed to spawn agentjs for `{suite_display}`: {error}"));

    if output.status.success() {
        let summary = read_cli_summary(&tmp_json).unwrap_or_else(|error| {
            panic!(
                "failed to read suite summary `{}`: {error}",
                tmp_json.display()
            )
        });
        SuiteReport {
            suite: suite_display,
            status: SuiteStatus::Completed,
            total: summary.total,
            passed: summary.passed,
            failed: summary.failed,
            skipped: summary.skipped,
            conformance_percent: summary.conformance_percent,
            elapsed_ms: summary.elapsed_ms,
            detail: String::new(),
            failure_samples: Vec::new(),
        }
    } else {
        SuiteReport {
            suite: suite_display,
            status: SuiteStatus::Crashed,
            total: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
            conformance_percent: 0.0,
            elapsed_ms: 0,
            detail: child_failure_detail(&output),
            failure_samples: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct Config {
    test262_root: PathBuf,
    report_path: PathBuf,
    tmp_dir: PathBuf,
    jobs: usize,
    mode: &'static str,
}

impl Config {
    fn new(default_report: &str, mode: &'static str) -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let report_path = env::var_os("AGENTJS_TEST262_REPORT")
            .map(PathBuf::from)
            .unwrap_or_else(|| manifest_dir.join(default_report));
        let tmp_dir = report_path
            .parent()
            .unwrap_or_else(|| Path::new("reports"))
            .join(".native-test262-tmp");

        fs::create_dir_all(&tmp_dir)
            .unwrap_or_else(|error| panic!("cannot create `{}`: {error}", tmp_dir.display()));

        Self {
            test262_root: manifest_dir.join("test262"),
            report_path,
            tmp_dir,
            jobs: env_usize("AGENTJS_TEST262_JOBS", DEFAULT_JOBS),
            mode,
        }
    }
}

fn direct_child_suites(absolute_parent: &Path, relative_parent: &Path) -> Vec<PathBuf> {
    let mut suites = fs::read_dir(absolute_parent)
        .unwrap_or_else(|error| panic!("cannot read `{}`: {error}", absolute_parent.display()))
        .map(|entry| entry.unwrap_or_else(|error| panic!("cannot read suite entry: {error}")))
        .filter_map(|entry| {
            let file_type = entry
                .file_type()
                .unwrap_or_else(|error| panic!("cannot read file type for {:?}: {error}", entry));
            file_type
                .is_dir()
                .then(|| relative_parent.join(entry.file_name()))
        })
        .collect::<Vec<_>>();
    suites.sort();
    suites
}

fn failure_sample(test262_root: &Path, case: &CaseResult) -> FailureSample {
    let path = case
        .path
        .strip_prefix(test262_root)
        .map_or_else(|_| case.path.as_path(), |path| path);

    FailureSample {
        path: normalize_path(path),
        status: case.status,
        detail: truncate(&case.detail, 4_000),
        elapsed_ms: case.elapsed.as_millis(),
    }
}

fn write_report(
    path: &Path,
    complete: bool,
    config: &Config,
    elapsed: Duration,
    suites: &[SuiteReport],
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, report_json(complete, config, elapsed, suites))
}

fn report_json(
    complete: bool,
    config: &Config,
    elapsed: Duration,
    suites: &[SuiteReport],
) -> String {
    let total = suites.iter().map(|report| report.total).sum::<usize>();
    let passed = suites.iter().map(|report| report.passed).sum::<usize>();
    let failed = suites.iter().map(|report| report.failed).sum::<usize>();
    let skipped = suites.iter().map(|report| report.skipped).sum::<usize>();
    let crashed = suites
        .iter()
        .filter(|report| matches!(report.status, SuiteStatus::Crashed))
        .count();

    let mut json = String::new();
    json.push_str("{\n");
    json.push_str(&format!("  \"complete\": {complete},\n"));
    json.push_str("  \"backend\": \"native\",\n");
    json.push_str("  \"suite\": \"test\",\n");
    json.push_str("  \"mode\": ");
    push_json_string(&mut json, config.mode);
    json.push_str(",\n");
    json.push_str("  \"test262_root\": ");
    push_json_string(&mut json, &normalize_path(&config.test262_root));
    json.push_str(",\n");
    json.push_str(&format!("  \"jobs\": {},\n", config.jobs));
    json.push_str(&format!("  \"total\": {total},\n"));
    json.push_str(&format!("  \"passed\": {passed},\n"));
    json.push_str(&format!("  \"failed\": {failed},\n"));
    json.push_str(&format!("  \"skipped\": {skipped},\n"));
    json.push_str(&format!("  \"crashed_suites\": {crashed},\n"));
    json.push_str(&format!(
        "  \"conformance_percent\": {:.4},\n",
        percent(passed, total)
    ));
    json.push_str(&format!("  \"elapsed_ms\": {},\n", elapsed.as_millis()));
    json.push_str("  \"suites\": [\n");

    for (index, suite) in suites.iter().enumerate() {
        if index > 0 {
            json.push_str(",\n");
        }
        push_suite_json(&mut json, suite);
    }

    json.push_str("\n  ]\n");
    json.push_str("}\n");
    json
}

fn push_suite_json(json: &mut String, report: &SuiteReport) {
    json.push_str("    {\n");
    json.push_str("      \"suite\": ");
    push_json_string(json, &report.suite);
    json.push_str(",\n");
    json.push_str("      \"status\": ");
    push_json_string(json, suite_status_label(report.status));
    json.push_str(",\n");
    json.push_str(&format!("      \"total\": {},\n", report.total));
    json.push_str(&format!("      \"passed\": {},\n", report.passed));
    json.push_str(&format!("      \"failed\": {},\n", report.failed));
    json.push_str(&format!("      \"skipped\": {},\n", report.skipped));
    json.push_str(&format!(
        "      \"conformance_percent\": {:.4},\n",
        report.conformance_percent
    ));
    json.push_str(&format!("      \"elapsed_ms\": {},\n", report.elapsed_ms));
    json.push_str("      \"detail\": ");
    push_json_string(json, &report.detail);
    json.push_str(",\n");
    json.push_str("      \"failure_samples\": [\n");

    for (index, sample) in report.failure_samples.iter().enumerate() {
        if index > 0 {
            json.push_str(",\n");
        }
        push_failure_sample_json(json, sample);
    }

    json.push_str("\n      ]\n");
    json.push_str("    }");
}

fn push_failure_sample_json(json: &mut String, sample: &FailureSample) {
    json.push_str("        { \"path\": ");
    push_json_string(json, &sample.path);
    json.push_str(", \"status\": ");
    push_json_string(json, status_label(sample.status));
    json.push_str(", \"elapsed_ms\": ");
    json.push_str(&sample.elapsed_ms.to_string());
    json.push_str(", \"detail\": ");
    push_json_string(json, &sample.detail);
    json.push_str(" }");
}

fn read_cli_summary(path: &Path) -> Result<CliSummary, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    Ok(CliSummary {
        total: json_usize(&source, "total")?,
        passed: json_usize(&source, "passed")?,
        failed: json_usize(&source, "failed")?,
        skipped: json_usize(&source, "skipped")?,
        conformance_percent: json_f64(&source, "conformance_percent")?,
        elapsed_ms: json_usize(&source, "elapsed_ms")? as u128,
    })
}

fn json_usize(source: &str, key: &str) -> Result<usize, String> {
    json_number(source, key)?
        .parse()
        .map_err(|error| format!("invalid `{key}` value: {error}"))
}

fn json_f64(source: &str, key: &str) -> Result<f64, String> {
    json_number(source, key)?
        .parse()
        .map_err(|error| format!("invalid `{key}` value: {error}"))
}

fn json_number<'a>(source: &'a str, key: &str) -> Result<&'a str, String> {
    let needle = format!("\"{key}\":");
    let start = source
        .find(&needle)
        .ok_or_else(|| format!("missing `{key}`"))?
        + needle.len();
    let rest = source[start..].trim_start();
    let end = rest
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .unwrap_or(rest.len());
    Ok(&rest[..end])
}

fn child_failure_detail(output: &std::process::Output) -> String {
    let status = output.status.to_string();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    truncate(
        &format!(
            "child process exited with {status}; stdout: {}; stderr: {}",
            stdout.trim(),
            stderr.trim()
        ),
        8_000,
    )
}

fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            ch if ch.is_control() => json.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => json.push(ch),
        }
    }
    json.push('"');
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect()
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut result = value.chars().take(max_chars).collect::<String>();
    if result.len() < value.len() {
        result.push('…');
    }
    result
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Passed => "passed",
        Status::Failed => "failed",
        Status::Skipped => "skipped",
    }
}

fn suite_status_label(status: SuiteStatus) -> &'static str {
    match status {
        SuiteStatus::Completed => "completed",
        SuiteStatus::Crashed => "crashed",
    }
}

fn percent(passed: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        passed as f64 * 100.0 / total as f64
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}
