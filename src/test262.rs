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

#[derive(Debug, Clone)]
pub struct RunnerOptions {
    pub test262_root: PathBuf,
    pub suite: PathBuf,
    pub filter: Option<String>,
    pub limit: Option<usize>,
    pub jobs: usize,
    pub backend: BackendKind,
    pub files: Vec<PathBuf>,
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
            backend: BackendKind::Boa,
            files: Vec::new(),
            runtime: RuntimeConfig {
                loop_limit: 100_000_000,
                recursion_limit: 512,
                stack_limit: 128 * 1024,
                backtrace_limit: 20,
                script_cache_capacity: 0,
                install_test262_host: true,
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
    sta: Arc<str>,
    doneprint: Arc<str>,
    includes: Arc<HashMap<String, Arc<str>>>,
}

impl Harness {
    fn minimal_native() -> Self {
        Self {
            assert: Arc::from(""),
            sta: Arc::from(""),
            doneprint: Arc::from(""),
            includes: Arc::new(HashMap::new()),
        }
    }
}

pub fn run(options: RunnerOptions) -> Result<Summary, String> {
    let started = Instant::now();
    let harness = Arc::new(if options.backend == BackendKind::Boa {
        load_harness(&options.test262_root)?
    } else {
        Harness::minimal_native()
    });
    let mut paths = if options.files.is_empty() {
        let suite = options.test262_root.join(&options.suite);
        let mut paths = Vec::new();
        collect_tests(&suite, &mut paths)?;
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
        workers.push(thread::spawn(move || {
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
                    run_case(&path, &harness, backend, config)
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
        }));
    }
    drop(sender);

    let mut summary = Summary {
        total,
        ..Summary::default()
    };
    for case in receiver {
        match case.status {
            Status::Passed => summary.passed += 1,
            Status::Failed => summary.failed += 1,
            Status::Skipped => summary.skipped += 1,
        }
        summary.cases.push(case);
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

fn run_case(
    path: &Path,
    harness: &Harness,
    backend: BackendKind,
    config: RuntimeConfig,
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
            run_variant(path, &source, &metadata, harness, backend, config, *strict).map_err(
                |detail| {
                    format!(
                        "{} mode: {detail}",
                        if *strict { "strict" } else { "default" }
                    )
                },
            )?;
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

fn run_variant(
    path: &Path,
    source: &str,
    metadata: &Metadata,
    harness: &Harness,
    backend: BackendKind,
    config: RuntimeConfig,
    strict: bool,
) -> Result<(), String> {
    let mut runtime = Runtime::with_backend(backend, config).map_err(|error| error.to_string())?;
    runtime.clear_output();
    runtime.set_strict(false);

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
    } else if backend == BackendKind::Native && !metadata.includes.is_empty() {
        return Err(format!(
            "native V1 does not support harness includes: {}",
            metadata.includes.join(", ")
        ));
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
            return Err(format!("unexpected {error} in `{}`", path.display()));
        }
        (Some(expected), Ok(_)) => {
            return Err(format!("expected {expected}, but execution succeeded"));
        }
        (Some(expected), Err(error)) if failure_matches(&error, expected) => {}
        (Some(expected), Err(error)) => {
            return Err(format!("expected {expected}, got {error}"));
        }
    }

    if metadata.flags.contains("async") {
        runtime.run_jobs().map_err(|error| error.to_string())?;
        let output = runtime.take_output();
        if let Some(failure) = output
            .iter()
            .find(|line| line.as_str() != "Test262:AsyncTestComplete")
        {
            return Err(format!("async test reported: {failure}"));
        }
        if !output
            .iter()
            .any(|line| line == "Test262:AsyncTestComplete")
        {
            return Err("async test did not signal completion".into());
        }
    }

    // The current script API combines parse and execution. Preserve phase data
    // for reporting while matching the expected error type above.
    let _phase = metadata.negative_phase.as_deref();
    Ok(())
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
        sta: required("sta.js")?,
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
}
