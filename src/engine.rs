use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    time::{Duration, Instant},
};

use boa_engine::{
    Context, JsError, JsNativeErrorKind, JsResult, JsValue, Source, js_string,
    native_function::NativeFunction, script::Script,
};
use boa_runtime::test262::{self, WorkerHandles};

thread_local! {
    static CAPTURED_OUTPUT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
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
        }
    }
}

/// Options that can vary between evaluations in the same isolate.
#[derive(Debug, Clone, Copy)]
pub struct ExecutionOptions {
    pub strict: bool,
    pub drain_jobs: bool,
}

impl Default for ExecutionOptions {
    fn default() -> Self {
        Self {
            strict: false,
            drain_jobs: true,
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
            Self::Other => "Error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalFailure {
    pub kind: FailureKind,
    pub message: String,
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

/// A persistent isolate. Use this for a REPL or a sequence of related calls.
pub struct Runtime {
    context: Context,
    strict: bool,
    script_cache_capacity: usize,
    script_cache: VecDeque<CachedScript>,
    _worker_handles: Option<WorkerHandles>,
}

struct CachedScript {
    source: Box<str>,
    strict: bool,
    script: Script,
}

impl Runtime {
    pub fn new(config: RuntimeConfig) -> Result<Self, EvalFailure> {
        clear_output();

        let mut context = Context::default();
        let limits = context.runtime_limits_mut();
        limits.set_loop_iteration_limit(config.loop_limit);
        limits.set_recursion_limit(config.recursion_limit);
        limits.set_stack_size_limit(config.stack_limit);
        limits.set_backtrace_limit(config.backtrace_limit);

        context
            .register_global_builtin_callable(
                js_string!("print"),
                1,
                NativeFunction::from_fn_ptr(host_print),
            )
            .map_err(|error| classify_error(error, &mut context))?;
        context
            .register_global_builtin_callable(
                js_string!("__agentjsLoadString"),
                1,
                NativeFunction::from_fn_ptr(host_load_string),
            )
            .map_err(|error| classify_error(error, &mut context))?;

        let worker_handles = config.install_test262_host.then(|| {
            let handles = WorkerHandles::new();
            test262::register_js262(handles.clone(), false, &mut context);
            handles
        });

        // Keep the host API deliberately small and deterministic.
        context
            .eval(Source::from_bytes(
                r#"
                Object.defineProperty(globalThis, "console", {
                    value: Object.freeze({
                        log: (...args) => print(...args),
                        info: (...args) => print(...args),
                        warn: (...args) => print(...args),
                        error: (...args) => print(...args)
                    }),
                    writable: false,
                    enumerable: false,
                    configurable: false
                });
                "#,
            ))
            .map_err(|error| classify_error(error, &mut context))?;

        Ok(Self {
            context,
            strict: false,
            script_cache_capacity: config.script_cache_capacity,
            script_cache: VecDeque::with_capacity(config.script_cache_capacity),
            _worker_handles: worker_handles,
        })
    }

    pub fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<ExecutionReport, EvalFailure> {
        clear_output();
        self.strict = options.strict;
        self.context.strict(options.strict);
        let started = Instant::now();

        let value = self.evaluate_script(source)?;

        if options.drain_jobs {
            self.context
                .run_jobs()
                .map_err(|error| classify_error(error, &mut self.context))?;
        }

        let value = value
            .to_string(&mut self.context)
            .map_err(|error| classify_error(error, &mut self.context))?
            .to_std_string_escaped();

        Ok(ExecutionReport {
            value,
            output: take_output(),
            elapsed: started.elapsed(),
        })
    }

    /// Evaluate setup code without clearing captured output. Used by Test262.
    pub(crate) fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure> {
        self.evaluate_script(source).map(|_| ())
    }

    pub(crate) fn run_jobs(&mut self) -> Result<(), EvalFailure> {
        self.context
            .run_jobs()
            .map_err(|error| classify_error(error, &mut self.context))
    }

    pub(crate) fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
        self.context.strict(strict);
    }

    pub(crate) fn clear_output(&self) {
        clear_output();
    }

    pub(crate) fn take_output(&self) -> Vec<String> {
        take_output()
    }

    fn evaluate_script(&mut self, source: &str) -> Result<JsValue, EvalFailure> {
        if let Some(index) = self
            .script_cache
            .iter()
            .position(|entry| entry.strict == self.strict && entry.source.as_ref() == source)
        {
            let Some(entry) = self.script_cache.remove(index) else {
                return Err(EvalFailure {
                    kind: FailureKind::Other,
                    message: "script cache became inconsistent".into(),
                });
            };
            let result = entry
                .script
                .evaluate(&mut self.context)
                .map_err(|error| classify_error(error, &mut self.context));
            self.script_cache.push_back(entry);
            return result;
        }

        let script = Script::parse(Source::from_bytes(source), None, &mut self.context)
            .map_err(|error| classify_error(error, &mut self.context))?;
        let result = script
            .evaluate(&mut self.context)
            .map_err(|error| classify_error(error, &mut self.context));

        if self.script_cache_capacity > 0 {
            if self.script_cache.len() >= self.script_cache_capacity {
                self.script_cache.pop_front();
            }
            self.script_cache.push_back(CachedScript {
                source: source.into(),
                strict: self.strict,
                script,
            });
        }
        result
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

fn host_print(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let mut fields = Vec::with_capacity(args.len());
    for value in args {
        fields.push(value.to_string(context)?.to_std_string_escaped());
    }
    CAPTURED_OUTPUT.with(|output| output.borrow_mut().push(fields.join(" ")));
    Ok(JsValue::undefined())
}

fn host_load_string(_this: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let source = args
        .first()
        .unwrap_or(&JsValue::undefined())
        .to_string(context)?
        .to_std_string_escaped();
    context.eval(Source::from_bytes(&source))
}

fn clear_output() {
    CAPTURED_OUTPUT.with(|output| output.borrow_mut().clear());
}

fn take_output() -> Vec<String> {
    CAPTURED_OUTPUT.with(|output| std::mem::take(&mut *output.borrow_mut()))
}

fn classify_error(error: JsError, context: &mut Context) -> EvalFailure {
    let rendered = error.to_string();
    let kind = if rendered.contains("RuntimeLimit") {
        FailureKind::RuntimeLimit
    } else if rendered.contains("Test262Error") {
        FailureKind::Test262
    } else {
        match error.try_native(context) {
            Ok(native) => match native.kind() {
                JsNativeErrorKind::Syntax => FailureKind::Syntax,
                JsNativeErrorKind::Reference => FailureKind::Reference,
                JsNativeErrorKind::Range => FailureKind::Range,
                JsNativeErrorKind::Type => FailureKind::Type,
                JsNativeErrorKind::Eval => FailureKind::Eval,
                _ => FailureKind::Other,
            },
            Err(_) => FailureKind::Other,
        }
    };

    EvalFailure {
        kind,
        message: rendered,
    }
}
