use std::{
    collections::{HashMap, HashSet, VecDeque},
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use crate::{
    backend::BackendKind,
    engine::{EvalFailure, ExecutionOptions, FailureKind, Runtime, RuntimeConfig},
};

/// Official Test262 files used as the Native V1 end-to-end acceptance gate.
pub const NATIVE_V1_TESTS: [&str; 6] = [
    "test/language/expressions/multiplication/line-terminator.js",
    "test/language/expressions/division/line-terminator.js",
    "test/language/expressions/division/no-magic-asi.js",
    "test/language/expressions/modulus/line-terminator.js",
    "test/language/expressions/unary-plus/11.4.6-2-1.js",
    "test/language/expressions/unary-minus/11.4.7-4-1.js",
];

/// Official Test262 files used as the Native V2 control-flow acceptance gate.
pub const NATIVE_V2_TESTS: [&str; 15] = [
    "test/language/statements/if/empty-statement.js",
    "test/language/statements/if/S12.5_A1_T1.js",
    "test/language/statements/if/S12.5_A1.1_T1.js",
    "test/language/statements/if/S12.5_A12_T1.js",
    "test/language/statements/if/S12.5_A12_T2.js",
    "test/language/statements/if/S12.5_A12_T3.js",
    "test/language/statements/if/S12.5_A12_T4.js",
    "test/language/expressions/conditional/S11.12_A3_T4.js",
    "test/language/expressions/conditional/S11.12_A4_T4.js",
    "test/language/statements/while/S12.6.2_A1.js",
    "test/language/statements/while/S12.6.2_A4_T1.js",
    "test/language/statements/if/S12.5_A6_T1.js",
    "test/language/statements/if/S12.5_A6_T2.js",
    "test/language/statements/if/S12.5_A8.js",
    "test/language/statements/while/S12.6.2_A6_T1.js",
];

/// Official Test262 files used as the Native V3 function acceptance gate.
///
/// These are the V3 candidates from `docs/native-v3-scope.md` that exist in
/// the pinned Test262 revision and only depend on the implemented V3 subset.
pub const NATIVE_V3_TESTS: [&str; 26] = [
    "test/language/statements/function/S10.1.1_A1_T1.js",
    "test/language/statements/function/S13_A1.js",
    "test/language/statements/function/S13_A3_T1.js",
    "test/language/statements/function/S13_A3_T2.js",
    "test/language/statements/function/S13_A3_T3.js",
    "test/language/statements/function/S13_A4_T1.js",
    "test/language/statements/function/S13_A7_T3.js",
    "test/language/statements/function/S13_A9.js",
    "test/language/statements/function/S13_A15_T1.js",
    "test/language/statements/function/S14_A2.js",
    "test/language/statements/return/line-terminators.js",
    "test/language/statements/return/S12.9_A1_T1.js",
    "test/language/statements/return/S12.9_A1_T2.js",
    "test/language/statements/return/S12.9_A1_T3.js",
    "test/language/statements/return/S12.9_A1_T4.js",
    "test/language/statements/return/S12.9_A1_T5.js",
    "test/language/statements/return/S12.9_A1_T6.js",
    "test/language/statements/return/S12.9_A1_T7.js",
    "test/language/statements/return/S12.9_A1_T8.js",
    "test/language/statements/return/S12.9_A1_T9.js",
    "test/language/statements/return/S12.9_A1_T10.js",
    "test/language/statements/return/S12.9_A3.js",
    "test/language/expressions/object/S11.1.5_A3.js",
    "test/language/expressions/object/S11.1.5_A4.1.js",
    "test/language/expressions/object/S11.1.5_A4.2.js",
    "test/language/expressions/object/S11.1.5_A4.3.js",
];

/// Official Test262 files used as the Native V4 object-model acceptance gate.
///
/// The list is intentionally pinned to files that are zero-failure and
/// zero-skip in the native backend. Broader V4 directories remain diagnostic
/// scan inputs for `--native-v4-scan`.
pub const NATIVE_V4_TESTS: [&str; 11] = [
    "test/language/expressions/delete/S8.12.7_A2_T1.js",
    "test/language/expressions/in/S8.12.6_A1.js",
    "test/language/expressions/in/S8.12.6_A3.js",
    "test/language/expressions/object/__proto__-value-obj.js",
    "test/language/expressions/instanceof/S11.8.6_A2.1_T1.js",
    "test/language/expressions/instanceof/S11.8.6_A7_T1.js",
    "test/language/expressions/object/11.1.5-2gs.js",
    "test/built-ins/Function/prototype/call/S15.3.4.4_A2_T1.js",
    "test/language/expressions/array/S11.1.4_A2.js",
    "test/built-ins/Array/constructor.js",
    "test/built-ins/Object/create/15.2.3.5-0-1.js",
];

/// V4-area directories scanned diagnostically by `--native-v4-scan`.
pub const NATIVE_V4_SCAN_SUITES: [&str; 8] = [
    "test/language/expressions/object",
    "test/language/expressions/array",
    "test/language/expressions/delete",
    "test/language/expressions/in",
    "test/language/expressions/instanceof",
    "test/built-ins/Object",
    "test/built-ins/Array",
    "test/built-ins/Function/prototype/call",
];

/// Initial zero-skip Native V5 gate covering try/catch, switch syntax, and
/// lexical TDZ runtime errors without relying on unsupported harness includes.
pub const NATIVE_V5_TESTS: [&str; 4] = [
    "test/language/statements/try/S12.14_A18_T5.js",
    "test/language/statements/switch/S12.11_A3_T1.js",
    "test/language/statements/let/global-use-before-initialization-in-prior-statement.js",
    "test/language/statements/const/global-use-before-initialization-in-prior-statement.js",
];

/// V5-area directories scanned diagnostically by `--native-v5 --progress`.
pub const NATIVE_V5_SCAN_SUITES: [&str; 4] = [
    "test/language/statements/try",
    "test/language/statements/switch",
    "test/language/statements/let",
    "test/language/statements/const",
];

/// Zero-skip Native V6 gate covering each core builtin family.
pub const NATIVE_V6_TESTS: [&str; 7] = [
    "test/built-ins/String/prototype/charAt/S15.5.4.4_A2.js",
    "test/built-ins/Number/isFinite/finite-numbers.js",
    "test/built-ins/Math/abs/absolute-value.js",
    "test/built-ins/Boolean/S15.6.1.1_A1_T1.js",
    "test/built-ins/Error/the-initial-value-of-errorprototypemessage-is-the-empty-string.js",
    "test/built-ins/JSON/parse/15.12.1.1-0-1.js",
    "test/built-ins/JSON/stringify/value-primitive-top-level.js",
];

/// Core builtin directories scanned diagnostically by `--native-v6-scan`.
pub const NATIVE_V6_SCAN_SUITES: [&str; 6] = [
    "test/built-ins/String",
    "test/built-ins/Number",
    "test/built-ins/Math",
    "test/built-ins/Boolean",
    "test/built-ins/Error",
    "test/built-ins/JSON",
];

/// Native V7 pinned integration gate.
///
/// V7 is an engineering-hardening milestone rather than a new syntax or
/// builtin-coverage milestone. Its pinned Test262 gate therefore aggregates the
/// already zero-failure, zero-skip V1-V6 gates and runs them through the V7
/// native runtime path. Broader V7 coverage remains diagnostic through
/// `--native-v7-scan` and the dashboard tests.
pub const NATIVE_V7_TEST_COUNT: usize = NATIVE_V1_TESTS.len()
    + NATIVE_V2_TESTS.len()
    + NATIVE_V3_TESTS.len()
    + NATIVE_V4_TESTS.len()
    + NATIVE_V5_TESTS.len()
    + NATIVE_V6_TESTS.len();

/// Lightweight frontend/cache-safety directories scanned by `--native-v7-scan`.
///
/// The selection intentionally covers a few thousand representative Test262
/// files without sweeping the very large `test/language` or `test/built-ins`
/// roots. It is diagnostic: skipped and failed tests remain visible and never
/// count as passes.
pub const NATIVE_V7_SCAN_SUITES: [&str; 9] = [
    "test/language/literals",
    "test/language/types",
    "test/language/block-scope",
    "test/language/function-code",
    "test/language/global-code",
    "test/built-ins/Function",
    "test/built-ins/String",
    "test/built-ins/Symbol",
    "test/built-ins/Reflect",
];

#[derive(Debug, Clone)]
pub struct RunnerOptions {
    pub test262_root: PathBuf,
    pub suite: PathBuf,
    pub filter: Option<String>,
    pub limit: Option<usize>,
    pub jobs: usize,
    pub backend: BackendKind,
    pub files: Vec<PathBuf>,
    pub suites: Vec<PathBuf>,
    pub progress: bool,
    pub skip_unsupported: bool,
    pub runtime: RuntimeConfig,
}

impl Default for RunnerOptions {
    fn default() -> Self {
        Self {
            test262_root: PathBuf::from("test262"),
            suite: PathBuf::from("test"),
            filter: None,
            limit: None,
            jobs: thread::available_parallelism().map_or(1, usize::from),
            backend: BackendKind::default(),
            files: Vec::new(),
            suites: Vec::new(),
            progress: false,
            skip_unsupported: false,
            runtime: RuntimeConfig {
                loop_limit: 100_000_000,
                // Kept at 256 (half the old 512) to leave headroom on the 32 MB
                // worker stack; each JS call adds ~5 Rust frames of non-trivial size.
                recursion_limit: 256,
                stack_limit: 128 * 1024,
                backtrace_limit: 20,
                script_cache_capacity: 0,
                install_test262_host: true,
                heap_object_limit: 500_000,
                heap_byte_limit: 512 * 1024 * 1024,
                wall_clock_limit: None,
                gc_allocation_threshold: 25_000,
            },
        }
    }
}

impl RunnerOptions {
    /// Selects the six official Test262 files that define the Native V1 gate.
    pub fn select_native_v1(&mut self) {
        self.backend = BackendKind::Native;
        self.files = NATIVE_V1_TESTS.iter().map(PathBuf::from).collect();
    }

    /// Selects the official Test262 files that define the Native V2 gate.
    pub fn select_native_v2(&mut self) {
        self.backend = BackendKind::Native;
        self.files = NATIVE_V2_TESTS.iter().map(PathBuf::from).collect();
    }

    /// Selects the official Test262 files that define the Native V3 gate.
    pub fn select_native_v3(&mut self) {
        self.backend = BackendKind::Native;
        self.files = NATIVE_V3_TESTS.iter().map(PathBuf::from).collect();
    }

    /// Selects the pinned zero-skip Native V4 Test262 gate.
    pub fn select_native_v4(&mut self) {
        self.backend = BackendKind::Native;
        self.files = NATIVE_V4_TESTS.iter().map(PathBuf::from).collect();
        self.suites.clear();
        self.skip_unsupported = false;
    }

    /// Selects V4-area directories as a diagnostic scan.
    pub fn select_native_v4_scan(&mut self) {
        self.backend = BackendKind::Native;
        self.files.clear();
        self.suites = NATIVE_V4_SCAN_SUITES.iter().map(PathBuf::from).collect();
        self.skip_unsupported = true;
    }

    /// Selects the initial zero-skip Native V5 Test262 gate.
    pub fn select_native_v5(&mut self) {
        self.backend = BackendKind::Native;
        self.files = NATIVE_V5_TESTS.iter().map(PathBuf::from).collect();
        self.suites.clear();
        self.skip_unsupported = false;
    }

    /// Selects V5-area directories as a diagnostic scan.
    pub fn select_native_v5_scan(&mut self) {
        self.backend = BackendKind::Native;
        self.files.clear();
        self.suites = NATIVE_V5_SCAN_SUITES.iter().map(PathBuf::from).collect();
        self.skip_unsupported = true;
    }

    /// Selects the pinned zero-skip Native V6 builtin gate.
    pub fn select_native_v6(&mut self) {
        self.backend = BackendKind::Native;
        self.files = NATIVE_V6_TESTS.iter().map(PathBuf::from).collect();
        self.suites.clear();
        self.skip_unsupported = false;
    }

    /// Selects V6 builtin directories as a diagnostic scan.
    pub fn select_native_v6_scan(&mut self) {
        self.backend = BackendKind::Native;
        self.files.clear();
        self.suites = NATIVE_V6_SCAN_SUITES.iter().map(PathBuf::from).collect();
        self.skip_unsupported = true;
    }

    /// Selects the Native V7 pinned integration gate.
    pub fn select_native_v7(&mut self) {
        self.backend = BackendKind::Native;
        self.files.clear();
        self.files.extend(NATIVE_V1_TESTS.iter().map(PathBuf::from));
        self.files.extend(NATIVE_V2_TESTS.iter().map(PathBuf::from));
        self.files.extend(NATIVE_V3_TESTS.iter().map(PathBuf::from));
        self.files.extend(NATIVE_V4_TESTS.iter().map(PathBuf::from));
        self.files.extend(NATIVE_V5_TESTS.iter().map(PathBuf::from));
        self.files.extend(NATIVE_V6_TESTS.iter().map(PathBuf::from));
        debug_assert_eq!(self.files.len(), NATIVE_V7_TEST_COUNT);
        self.suites.clear();
        self.skip_unsupported = false;
    }

    /// Selects V7 frontend/cache-safety directories as a diagnostic scan.
    pub fn select_native_v7_scan(&mut self) {
        self.backend = BackendKind::Native;
        self.files.clear();
        self.suites = NATIVE_V7_SCAN_SUITES.iter().map(PathBuf::from).collect();
        self.skip_unsupported = true;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct CaseResult {
    pub path: PathBuf,
    pub status: Status,
    pub detail: String,
    pub elapsed: Duration,
}

#[derive(Debug, Default, Clone)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub elapsed: Duration,
    pub cases: Vec<CaseResult>,
}

impl Summary {
    #[must_use]
    pub fn conformance_percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.passed as f64 * 100.0 / self.total as f64
        }
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        format!(
            concat!(
                "{{\n",
                "  \"total\": {},\n",
                "  \"passed\": {},\n",
                "  \"failed\": {},\n",
                "  \"skipped\": {},\n",
                "  \"conformance_percent\": {:.4},\n",
                "  \"elapsed_ms\": {}\n",
                "}}\n"
            ),
            self.total,
            self.passed,
            self.failed,
            self.skipped,
            self.conformance_percent(),
            self.elapsed.as_millis()
        )
    }
}

#[derive(Debug, Clone, Default)]
struct Metadata {
    includes: Vec<String>,
    flags: HashSet<String>,
    negative_phase: Option<String>,
    negative_type: Option<String>,
}

#[derive(Debug, Clone)]
struct Harness {
    assert: Arc<str>,
    #[cfg(feature = "boa-backend")]
    sta: Arc<str>,
    #[cfg(feature = "boa-backend")]
    doneprint: Arc<str>,
    includes: Arc<HashMap<String, Arc<str>>>,
}

#[cfg(test)]
impl Harness {
    /// An empty harness used by the parse-negative unit tests, which never
    /// reference harness includes.
    fn minimal_native() -> Self {
        Self {
            assert: Arc::from(""),
            #[cfg(feature = "boa-backend")]
            sta: Arc::from(""),
            #[cfg(feature = "boa-backend")]
            doneprint: Arc::from(""),
            includes: Arc::new(HashMap::new()),
        }
    }
}

pub fn run(options: RunnerOptions) -> Result<Summary, String> {
    let started = Instant::now();
    // Both backends load the full harness directory so that `includes`
    // (propertyHelper.js, nativeErrors.js, …) are available. The native runtime
    // provides assert/sta as host functions and simply does not eval those two.
    let harness = Arc::new(load_harness(&options.test262_root)?);
    let mut paths = if options.files.is_empty() && options.suites.is_empty() {
        let suite = options.test262_root.join(&options.suite);
        let mut paths = Vec::new();
        collect_tests(&suite, &mut paths)?;
        paths
    } else if options.files.is_empty() {
        let mut paths = Vec::new();
        for suite in &options.suites {
            collect_tests(&options.test262_root.join(suite), &mut paths)?;
        }
        paths
    } else {
        options
            .files
            .iter()
            .map(|path| options.test262_root.join(path))
            .collect()
    };
    for path in &paths {
        if !path.is_file() {
            return Err(format!("Test262 file does not exist: `{}`", path.display()));
        }
    }
    paths.sort();

    if let Some(filter) = &options.filter {
        paths.retain(|path| path.to_string_lossy().contains(filter));
    }
    if let Some(limit) = options.limit {
        paths.truncate(limit);
    }

    let total = paths.len();
    let progress = options.progress;
    let skip_unsupported = options.skip_unsupported;
    let queue = Arc::new(Mutex::new(VecDeque::from(paths)));
    let worker_count = options.jobs.max(1).min(total.max(1));
    let (sender, receiver) = mpsc::channel();
    let mut workers = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let queue = Arc::clone(&queue);
        let sender = sender.clone();
        let harness = Arc::clone(&harness);
        let config = options.runtime;
        let backend = options.backend;
        workers.push(
            thread::Builder::new()
                // Default Rust thread stack is 2 MB. With recursion_limit=256 each JS
                // call chain adds ~5 Rust frames; deep harness call chains plus the VM
                // dispatch loop can exhaust 2 MB, triggering SIGSEGV in the signal
                // handler itself and aborting the whole process. 32 MB gives comfortable
                // headroom for any realistic test case without per-test subprocess cost.
                .stack_size(32 * 1024 * 1024)
                .spawn(move || {
            loop {
                let path = match queue.lock() {
                    Ok(mut queue) => queue.pop_front(),
                    Err(_) => {
                        let _ = sender.send(CaseResult {
                            path: PathBuf::new(),
                            status: Status::Failed,
                            detail: "test queue lock was poisoned".into(),
                            elapsed: Duration::ZERO,
                        });
                        break;
                    }
                };
                let Some(path) = path else {
                    break;
                };
                let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_case(&path, &harness, backend, config, skip_unsupported)
                })) {
                    Ok(result) => result,
                    Err(payload) => CaseResult {
                        path,
                        status: Status::Failed,
                        detail: format!("engine panic: {}", panic_message(payload.as_ref())),
                        elapsed: Duration::ZERO,
                    },
                };
                if sender.send(result).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn test262 worker thread"),
        );
    }
    drop(sender);

    let mut summary = Summary {
        total,
        ..Summary::default()
    };
    let mut completed = 0usize;
    for case in receiver {
        completed += 1;
        match case.status {
            Status::Passed => summary.passed += 1,
            Status::Failed => summary.failed += 1,
            Status::Skipped => summary.skipped += 1,
        }
        if progress {
            print_progress(completed, total, &summary, &case);
        }
        summary.cases.push(case);
    }
    if progress {
        eprintln!(); // end the \r progress line
    }
    for worker in workers {
        worker
            .join()
            .map_err(|_| "a Test262 worker thread panicked".to_string())?;
    }

    summary.cases.sort_by(|a, b| a.path.cmp(&b.path));
    summary.elapsed = started.elapsed();
    Ok(summary)
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic payload")
}

fn print_progress(completed: usize, total: usize, summary: &Summary, case: &CaseResult) {
    use std::io::Write;
    let percent = if total == 0 {
        100.0
    } else {
        completed as f64 * 100.0 / total as f64
    };
    // Print failures/skips on their own line so they are not overwritten.
    if case.status != Status::Passed {
        let label = status_label(case.status);
        if case.detail.is_empty() {
            eprintln!("\n{label}\t{}", case.path.display());
        } else {
            eprintln!("\n{label}\t{}\n  {}", case.path.display(), case.detail);
        }
    }
    // Overwrite the current terminal line with a compact live counter.
    eprint!(
        "\r[{completed}/{total} {percent:5.1}%] pass={} fail={} skip={}   ",
        summary.passed, summary.failed, summary.skipped,
    );
    let _ = std::io::stderr().flush();
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Passed => "PASS",
        Status::Failed => "FAIL",
        Status::Skipped => "SKIP",
    }
}

fn run_case(
    path: &Path,
    harness: &Harness,
    backend: BackendKind,
    config: RuntimeConfig,
    skip_unsupported: bool,
) -> CaseResult {
    let started = Instant::now();
    let result = (|| {
        let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let metadata = parse_metadata(&source)?;

        if metadata.flags.contains("module") {
            return Ok((Status::Skipped, "module runner not implemented yet".into()));
        }
        if metadata.flags.contains("CanBlockIsFalse") {
            return Ok((
                Status::Skipped,
                "non-blocking agent tests are not enabled".into(),
            ));
        }
        let strict_modes: &[bool] = if metadata.flags.contains("raw") {
            &[false]
        } else if metadata.flags.contains("onlyStrict") {
            &[true]
        } else if metadata.flags.contains("noStrict") {
            &[false]
        } else {
            &[false, true]
        };

        for strict in strict_modes {
            match run_variant(VariantRun {
                path,
                source: &source,
                metadata: &metadata,
                harness,
                backend,
                config,
                strict: *strict,
                skip_unsupported,
            }) {
                Ok(()) => {}
                Err(VariantFailure::Skipped(detail)) => {
                    return Ok((
                        Status::Skipped,
                        format!(
                            "{} mode: {detail}",
                            if *strict { "strict" } else { "default" }
                        ),
                    ));
                }
                Err(VariantFailure::Failed(detail)) => {
                    return Err(format!(
                        "{} mode: {detail}",
                        if *strict { "strict" } else { "default" }
                    ));
                }
            }
        }
        Ok((Status::Passed, String::new()))
    })();

    let (status, detail) = match result {
        Ok(value) => value,
        Err(detail) => (Status::Failed, detail),
    };
    CaseResult {
        path: path.to_path_buf(),
        status,
        detail,
        elapsed: started.elapsed(),
    }
}

struct VariantRun<'a> {
    path: &'a Path,
    source: &'a str,
    metadata: &'a Metadata,
    harness: &'a Harness,
    backend: BackendKind,
    config: RuntimeConfig,
    strict: bool,
    skip_unsupported: bool,
}

fn run_variant(run: VariantRun<'_>) -> Result<(), VariantFailure> {
    let VariantRun {
        path,
        source,
        metadata,
        harness,
        backend,
        config,
        strict,
        skip_unsupported,
    } = run;
    let mut runtime = Runtime::with_backend(backend, config).map_err(|error| error.to_string())?;
    runtime.clear_output();
    runtime.set_strict(false);

    if is_static_negative(metadata) {
        return run_static_negative_variant(
            &mut runtime,
            StaticNegativeRun {
                path,
                source,
                metadata,
                strict,
                skip_unsupported,
            },
        );
    }

    #[cfg(feature = "boa-backend")]
    if backend == BackendKind::Boa && !metadata.flags.contains("raw") {
        runtime
            .eval_fragment(&harness.assert)
            .map_err(|error| format!("assert.js failed: {error}"))?;
        runtime
            .eval_fragment(&harness.sta)
            .map_err(|error| format!("sta.js failed: {error}"))?;

        if metadata.flags.contains("async") {
            runtime
                .eval_fragment(&harness.doneprint)
                .map_err(|error| format!("doneprintHandle.js failed: {error}"))?;
        }
        for include in &metadata.includes {
            let code = harness
                .includes
                .get(include)
                .ok_or_else(|| format!("missing harness include `{include}`"))?;
            runtime
                .eval_fragment(code)
                .map_err(|error| format!("harness `{include}` failed: {error}"))?;
        }
    }

    if backend == BackendKind::Native {
        // Eval the official assert.js to provide the full assert suite:
        // assert.compareArray, assert.throws (with constructor check), assert._isSameValue,
        // isNegativeZero, compareArray, etc.  Test262Error stays as a Rust host function
        // so the test runner can detect assertion failures; assert.js calls it via
        // `throw new Test262Error(...)` which routes through our Rust builtin correctly.
        // sta.js is intentionally skipped: it redefines Test262Error as a plain JS class
        // which would shadow the Rust host function and break error detection.
        if !metadata.flags.contains("raw") {
            runtime
                .eval_fragment(&harness.assert)
                .map_err(|error| format!("assert.js failed: {error}"))?;
        }
        for include in &metadata.includes {
            let code = harness.includes.get(include).ok_or_else(|| {
                VariantFailure::Failed(format!("missing harness include `{include}`"))
            })?;
            if let Err(error) = runtime.eval_fragment(code) {
                if skip_unsupported && error.kind == FailureKind::Unsupported {
                    return Err(VariantFailure::Skipped(format!(
                        "unsupported native feature in harness `{include}`: {error}"
                    )));
                }
                return Err(VariantFailure::Failed(format!(
                    "harness `{include}` failed: {error}"
                )));
            }
        }
    }

    runtime.set_strict(strict);

    let outcome = runtime.eval(
        source,
        ExecutionOptions {
            strict,
            drain_jobs: !metadata.flags.contains("async"),
        },
    );

    match (&metadata.negative_type, outcome) {
        (None, Ok(_)) => {}
        (None, Err(error)) => {
            if skip_unsupported && error.kind == FailureKind::Unsupported {
                return Err(VariantFailure::Skipped(format!(
                    "unsupported native feature in `{}`: {error}",
                    path.display()
                )));
            }
            return Err(VariantFailure::Failed(format!(
                "unexpected {error} in `{}`",
                path.display()
            )));
        }
        (Some(expected), Ok(_)) => {
            return Err(VariantFailure::Failed(format!(
                "expected {expected}, but execution succeeded"
            )));
        }
        (Some(expected), Err(error)) if failure_matches(&error, expected) => {}
        (Some(expected), Err(error)) => {
            if skip_unsupported && error.kind == FailureKind::Unsupported {
                return Err(VariantFailure::Skipped(format!(
                    "unsupported native feature in `{}`: {error}",
                    path.display()
                )));
            }
            return Err(VariantFailure::Failed(format!(
                "expected {expected}, got {error}"
            )));
        }
    }

    if metadata.flags.contains("async") {
        runtime.run_jobs().map_err(|error| error.to_string())?;
        let output = runtime.take_output();
        if let Some(failure) = output
            .iter()
            .find(|line| line.as_str() != "Test262:AsyncTestComplete")
        {
            return Err(VariantFailure::Failed(format!(
                "async test reported: {failure}"
            )));
        }
        if !output
            .iter()
            .any(|line| line == "Test262:AsyncTestComplete")
        {
            return Err(VariantFailure::Failed(
                "async test did not signal completion".into(),
            ));
        }
    }

    Ok(())
}

struct StaticNegativeRun<'a> {
    path: &'a Path,
    source: &'a str,
    metadata: &'a Metadata,
    strict: bool,
    skip_unsupported: bool,
}

fn run_static_negative_variant(
    runtime: &mut Runtime,
    run: StaticNegativeRun<'_>,
) -> Result<(), VariantFailure> {
    let StaticNegativeRun {
        path,
        source,
        metadata,
        strict,
        skip_unsupported,
    } = run;
    let phase = metadata
        .negative_phase
        .as_deref()
        .expect("static negative variant has a phase");
    let expected = metadata.negative_type.as_deref().ok_or_else(|| {
        VariantFailure::Failed(format!(
            "negative {phase} phase test is missing an expected error type"
        ))
    })?;

    runtime.set_strict(strict);
    let outcome = runtime.parse_only(
        source,
        ExecutionOptions {
            strict,
            drain_jobs: false,
        },
    );

    match outcome {
        Ok(()) => Err(VariantFailure::Failed(format!(
            "expected {expected} during {phase} phase, but parsing/compilation succeeded"
        ))),
        Err(error) if failure_matches(&error, expected) => Ok(()),
        Err(error) => {
            if skip_unsupported && error.kind == FailureKind::Unsupported {
                return Err(VariantFailure::Skipped(format!(
                    "unsupported native feature in `{}`: {error}",
                    path.display()
                )));
            }
            Err(VariantFailure::Failed(format!(
                "expected {expected} during {phase} phase, got {error}"
            )))
        }
    }
}

fn is_static_negative(metadata: &Metadata) -> bool {
    matches!(metadata.negative_phase.as_deref(), Some("parse" | "early"))
}

#[derive(Debug, Clone)]
enum VariantFailure {
    Failed(String),
    Skipped(String),
}

impl From<String> for VariantFailure {
    fn from(value: String) -> Self {
        Self::Failed(value)
    }
}

fn failure_matches(error: &EvalFailure, expected: &str) -> bool {
    error.kind.name() == expected
        || (expected == "Test262Error" && error.kind == FailureKind::Test262)
        || error.message.contains(expected)
}

fn load_harness(root: &Path) -> Result<Harness, String> {
    let root = root.join("harness");
    let mut files = HashMap::new();
    load_harness_dir(&root, &root, &mut files)?;

    let required = |name: &str| {
        files
            .get(name)
            .cloned()
            .ok_or_else(|| format!("missing test262/harness/{name}"))
    };

    Ok(Harness {
        assert: required("assert.js")?,
        #[cfg(feature = "boa-backend")]
        sta: required("sta.js")?,
        #[cfg(feature = "boa-backend")]
        doneprint: required("doneprintHandle.js")?,
        includes: Arc::new(files),
    })
}

fn load_harness_dir(
    root: &Path,
    current: &Path,
    files: &mut HashMap<String, Arc<str>>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            load_harness_dir(root, &path, files)?;
        } else if path.extension() == Some(OsStr::new("js")) {
            let key = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let code: Arc<str> = fs::read_to_string(&path)
                .map_err(|error| error.to_string())?
                .into();
            files.insert(key, code);
        }
    }
    Ok(())
}

fn collect_tests(path: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(path)
        .map_err(|error| format!("cannot read suite `{}`: {error}", path.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_tests(&path, paths)?;
        } else if path.extension() == Some(OsStr::new("js"))
            && !path
                .file_stem()
                .is_some_and(|name| name.to_string_lossy().ends_with("_FIXTURE"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn parse_metadata(source: &str) -> Result<Metadata, String> {
    let (_, rest) = source
        .split_once("/*---")
        .ok_or_else(|| "missing Test262 frontmatter".to_string())?;
    let (frontmatter, _) = rest
        .split_once("---*/")
        .ok_or_else(|| "unterminated Test262 frontmatter".to_string())?;

    Ok(Metadata {
        includes: parse_yaml_list(frontmatter, "includes"),
        flags: parse_yaml_list(frontmatter, "flags").into_iter().collect(),
        negative_phase: parse_nested_value(frontmatter, "negative", "phase"),
        negative_type: parse_nested_value(frontmatter, "negative", "type"),
    })
}

fn parse_yaml_list(frontmatter: &str, key: &str) -> Vec<String> {
    let lines: Vec<_> = frontmatter.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let Some(value) = trimmed.strip_prefix(&format!("{key}:")) else {
            continue;
        };
        let value = value.trim();
        if value.starts_with('[') && value.ends_with(']') {
            return value[1..value.len() - 1]
                .split(',')
                .map(|item| unquote(item.trim()))
                .filter(|item| !item.is_empty())
                .collect();
        }

        let base_indent = line.len() - line.trim_start().len();
        let mut result = Vec::new();
        for next in lines.iter().skip(index + 1) {
            let indent = next.len() - next.trim_start().len();
            let trimmed = next.trim();
            if trimmed.is_empty() {
                continue;
            }
            if indent <= base_indent {
                break;
            }
            if let Some(item) = trimmed.strip_prefix("- ") {
                result.push(unquote(item.trim()));
            }
        }
        return result;
    }
    Vec::new()
}

fn parse_nested_value(frontmatter: &str, parent: &str, key: &str) -> Option<String> {
    let lines: Vec<_> = frontmatter.lines().collect();
    let parent_index = lines
        .iter()
        .position(|line| line.trim() == format!("{parent}:"))?;
    let base_indent = lines[parent_index].len() - lines[parent_index].trim_start().len();

    for line in lines.iter().skip(parent_index + 1) {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if indent <= base_indent {
            break;
        }
        if let Some(value) = trimmed.strip_prefix(&format!("{key}:")) {
            return Some(unquote(value.trim()));
        }
    }
    None
}

fn unquote(value: &str) -> String {
    value
        .trim_matches(|character| character == '\'' || character == '"')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parses_inline_and_nested_metadata() {
        let metadata = parse_metadata(
            r#"
            /*---
            description: sample
            includes: [compareArray.js, propertyHelper.js]
            flags: [onlyStrict]
            negative:
              phase: parse
              type: SyntaxError
            ---*/
            "#,
        )
        .unwrap();

        assert_eq!(metadata.includes.len(), 2);
        assert!(metadata.flags.contains("onlyStrict"));
        assert_eq!(metadata.negative_phase.as_deref(), Some("parse"));
        assert_eq!(metadata.negative_type.as_deref(), Some("SyntaxError"));
    }

    #[test]
    fn native_parse_negative_does_not_execute_source() {
        let metadata = Metadata {
            negative_phase: Some("parse".into()),
            negative_type: Some("SyntaxError".into()),
            ..Metadata::default()
        };

        let result = run_variant(VariantRun {
            path: Path::new("synthetic-parse-negative.js"),
            source: "$DONOTEVALUATE(); 1;",
            metadata: &metadata,
            harness: &Harness::minimal_native(),
            backend: BackendKind::Native,
            config: test_config(),
            strict: false,
            skip_unsupported: false,
        });

        let Err(VariantFailure::Failed(detail)) = result else {
            panic!("syntactically valid negative parse test should fail statically");
        };
        assert!(detail.contains("expected SyntaxError during parse phase"));
        assert!(!detail.contains("ReferenceError"));
        assert!(!detail.contains("$DONOTEVALUATE"));
    }

    #[test]
    fn native_parse_negative_accepts_syntax_error_without_execution() {
        let metadata = Metadata {
            negative_phase: Some("parse".into()),
            negative_type: Some("SyntaxError".into()),
            ..Metadata::default()
        };

        run_variant(VariantRun {
            path: Path::new("synthetic-parse-negative.js"),
            source: "$DONOTEVALUATE(); var ;",
            metadata: &metadata,
            harness: &Harness::minimal_native(),
            backend: BackendKind::Native,
            config: test_config(),
            strict: false,
            skip_unsupported: false,
        })
        .expect("parse phase SyntaxError should satisfy negative metadata");
    }

    #[test]
    fn native_static_negative_with_includes_is_not_skipped_before_parse() {
        let path = write_temp_test(
            "native-static-negative-includes",
            r#"
            /*---
            includes: [compareArray.js]
            negative:
              phase: parse
              type: SyntaxError
            ---*/

            $DONOTEVALUATE(); 1;
            "#,
        );

        let result = run_case(
            &path,
            &Harness::minimal_native(),
            BackendKind::Native,
            test_config(),
            true,
        );

        let _ = fs::remove_file(&path);
        assert_eq!(result.status, Status::Failed);
        assert!(
            result
                .detail
                .contains("expected SyntaxError during parse phase")
        );
        assert!(
            !result
                .detail
                .contains("native backend does not support harness includes"),
            "{}",
            result.detail
        );
    }

    fn test_config() -> RuntimeConfig {
        RunnerOptions::default().runtime
    }

    fn write_temp_test(name: &str, source: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "agentjs-test262-{name}-{}-{unique}.js",
            std::process::id()
        ));
        fs::write(&path, source).expect("synthetic Test262 file should be writable");
        path
    }
}
