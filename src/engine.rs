use std::{
    fmt,
    path::Path,
    time::{Duration, Instant},
};

use crate::{
    backend::{BackendKind, RuntimeBackend, create_runtime, create_runtime_with_host},
    host::HostServices,
    runtime::{GcMetrics, HeapStats, RuntimeMemoryStats},
};

/// A monotonic deadline shared by every stage of one host-controlled run.
#[derive(Debug, Clone, Copy, Default)]
pub struct AbsoluteDeadline {
    at: Option<Instant>,
}

impl AbsoluteDeadline {
    #[must_use]
    pub fn from_duration(start: Instant, duration: Option<Duration>) -> Self {
        Self {
            at: duration.and_then(|duration| start.checked_add(duration)),
        }
    }

    pub fn check(self) -> Result<(), RuntimeLimitError> {
        if self.is_expired() {
            Err(RuntimeLimitError)
        } else {
            Ok(())
        }
    }

    #[must_use]
    pub fn remaining(self) -> Option<Duration> {
        self.at
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    #[must_use]
    pub fn is_expired(self) -> bool {
        self.at.is_some_and(|deadline| Instant::now() >= deadline)
    }

    #[must_use]
    pub(crate) const fn instant(self) -> Option<Instant> {
        self.at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeLimitError;

impl fmt::Display for RuntimeLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("wall-clock deadline exceeded")
    }
}

impl std::error::Error for RuntimeLimitError {}

#[derive(Debug, Clone, Copy, Default)]
pub struct RunControl {
    pub deadline: AbsoluteDeadline,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FrontendControl {
    pub deadline: AbsoluteDeadline,
}

impl FrontendControl {
    #[inline]
    pub fn checkpoint(self) -> Result<(), RuntimeLimitError> {
        self.deadline.check()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PhaseDiagnostics {
    pub phase: &'static str,
    pub elapsed_ms: u64,
    pub source_bytes: usize,
    pub token_count: Option<usize>,
    pub instruction_count: Option<usize>,
    pub constant_count: Option<usize>,
    pub function_count: Option<usize>,
    pub heap: HeapStats,
    pub gc: GcMetrics,
    pub runtime_memory: RuntimeMemoryStats,
}

/// Limits applied to one JavaScript isolate.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeConfig {
    pub loop_limit: u64,
    pub recursion_limit: usize,
    pub stack_limit: usize,
    pub backtrace_limit: usize,
    pub script_cache_capacity: usize,
    pub install_test262_host: bool,
    pub install_jetstream_host: bool,
    pub diagnostics: bool,
    /// Maximum number of heap objects (JsObject + JsFunction + Environment) per isolate.
    /// Exceeding this limit throws a RuntimeLimit error rather than OOM-ing the process.
    pub heap_object_limit: usize,
    /// Conservative byte budget for heap-owned and guarded large allocations.
    pub heap_byte_limit: usize,
    /// Cooperative wall-clock budget for one top-level evaluation.
    pub wall_clock_limit: Option<Duration>,
    /// Number of heap allocations between cooperative GC checks.
    pub gc_allocation_threshold: usize,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            loop_limit: 10_000_000,
            recursion_limit: 256,
            stack_limit: 64 * 1024,
            backtrace_limit: 20,
            script_cache_capacity: 32,
            install_test262_host: false,
            install_jetstream_host: false,
            diagnostics: false,
            heap_object_limit: 500_000,
            heap_byte_limit: 256 * 1024 * 1024,
            wall_clock_limit: None,
            gc_allocation_threshold: 10_000,
        }
    }
}

/// Source grammar selected for one evaluation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum SourceKind {
    #[default]
    Script,
    Module,
}

/// Options that can vary between evaluations in the same isolate.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionOptions {
    pub strict: bool,
    pub drain_jobs: bool,
    pub source_kind: SourceKind,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            strict: false,
            drain_jobs: true,
            source_kind: SourceKind::Script,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    Syntax,
    Reference,
    Range,
    Type,
    Eval,
    RuntimeLimit,
    Test262,
    Unsupported,
    Other,
}

impl FailureKind {
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Syntax => "SyntaxError",
            Self::Reference => "ReferenceError",
            Self::Range => "RangeError",
            Self::Type => "TypeError",
            Self::Eval => "EvalError",
            Self::RuntimeLimit => "RuntimeLimit",
            Self::Test262 => "Test262Error",
            Self::Unsupported => "Unsupported",
            Self::Other => "Error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalFailure {
    pub kind: FailureKind,
    pub message: String,
}

impl EvalFailure {
    pub(crate) fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for EvalFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind.name(), self.message)
    }
}

impl std::error::Error for EvalFailure {}

#[derive(Debug, Clone)]
pub struct ExecutionReport {
    pub value: String,
    pub output: Vec<String>,
    pub elapsed: Duration,
}

/// A persistent JavaScript isolate backed by the native engine.
pub struct Runtime {
    backend: Box<dyn RuntimeBackend>,
}

impl Runtime {
    /// Creates a runtime using the native backend.
    pub fn new(config: RuntimeConfig) -> Result<Self, EvalFailure> {
        Ok(Self {
            backend: create_runtime(BackendKind::Native, config)?,
        })
    }

    pub fn with_host(config: RuntimeConfig, host: HostServices) -> Result<Self, EvalFailure> {
        Ok(Self {
            backend: create_runtime_with_host(BackendKind::Native, config, host)?,
        })
    }

    pub fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<ExecutionReport, EvalFailure> {
        let started = Instant::now();
        let result = self.backend.eval(source, options)?;

        Ok(ExecutionReport {
            value: result.value,
            output: result.output,
            elapsed: started.elapsed(),
        })
    }

    /// Installs or clears host-level controls shared by subsequent evaluations.
    pub fn set_run_control(&mut self, control: Option<RunControl>) {
        self.backend.set_run_control(control);
    }

    /// Selects the stable stage label used by diagnostics for the next evaluation.
    pub fn set_diagnostic_phase(&mut self, phase: &'static str) {
        self.backend.set_diagnostic_phase(phase);
    }

    /// Evaluates setup code without clearing captured output. Used by Test262.
    pub(crate) fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure> {
        self.backend.eval_fragment(source)
    }

    pub(crate) fn eval_module_source(
        &mut self,
        source: &str,
        path: &Path,
        drain_jobs: bool,
    ) -> Result<ExecutionReport, EvalFailure> {
        let started = Instant::now();
        let result = self.backend.eval_module_source(source, path, drain_jobs)?;

        Ok(ExecutionReport {
            value: result.value,
            output: result.output,
            elapsed: started.elapsed(),
        })
    }

    pub(crate) fn parse_only(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<(), EvalFailure> {
        self.backend.parse_only(source, options)
    }

    pub(crate) fn run_jobs(&mut self) -> Result<(), EvalFailure> {
        self.backend.run_jobs()
    }

    pub(crate) fn set_strict(&mut self, strict: bool) {
        self.backend.set_strict(strict);
    }

    /// Supplies the source path used by B's local dynamic-import resolver.
    pub(crate) fn set_dynamic_import_referrer(&mut self, path: &Path) {
        self.backend.set_dynamic_import_referrer(path);
    }

    pub(crate) fn clear_output(&mut self) {
        self.backend.clear_output();
    }

    pub(crate) fn take_output(&mut self) -> Vec<String> {
        self.backend.take_output()
    }
}

/// Stateless facade: every call gets a fresh isolate, preventing global-state
/// leakage between unrelated agent actions.
#[derive(Debug, Clone, Copy)]
pub struct Engine {
    config: RuntimeConfig,
}

impl Engine {
    #[must_use]
    pub const fn new(config: RuntimeConfig) -> Self {
        Self { config }
    }

    pub fn execute(
        &self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<ExecutionReport, EvalFailure> {
        Runtime::new(self.config)?.eval(source, options)
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(RuntimeConfig::default())
    }
}
