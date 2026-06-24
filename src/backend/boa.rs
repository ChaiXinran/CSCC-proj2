use std::{cell::RefCell, collections::VecDeque, path::Path};

use boa_engine::{
    Context, JsError, JsNativeErrorKind, JsResult, JsValue, Source, js_string,
    native_function::NativeFunction, script::Script,
};
use boa_runtime::test262::{self, WorkerHandles};

use crate::{
    backend::{BackendExecution, RuntimeBackend},
    engine::{EvalFailure, ExecutionOptions, FailureKind, RuntimeConfig},
};

thread_local! {
    static CAPTURED_OUTPUT: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Compatibility runtime backed by Boa.
pub struct BoaRuntime {
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

impl BoaRuntime {
    pub fn new(config: RuntimeConfig) -> Result<Self, EvalFailure> {
        clear_captured_output();

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

    fn evaluate_script(&mut self, source: &str) -> Result<JsValue, EvalFailure> {
        if let Some(index) = self
            .script_cache
            .iter()
            .position(|entry| entry.strict == self.strict && entry.source.as_ref() == source)
        {
            let Some(entry) = self.script_cache.remove(index) else {
                return Err(EvalFailure::new(
                    FailureKind::Other,
                    "script cache became inconsistent",
                ));
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

impl RuntimeBackend for BoaRuntime {
    fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<BackendExecution, EvalFailure> {
        clear_captured_output();
        self.strict = options.strict;
        self.context.strict(options.strict);

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

        Ok(BackendExecution {
            value,
            output: take_captured_output(),
        })
    }

    fn parse_only(&mut self, source: &str, options: ExecutionOptions) -> Result<(), EvalFailure> {
        clear_captured_output();
        self.strict = options.strict;
        self.context.strict(options.strict);
        Script::parse(Source::from_bytes(source), None, &mut self.context)
            .map(|_| ())
            .map_err(|error| classify_error(error, &mut self.context))
    }

    fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure> {
        self.evaluate_script(source).map(|_| ())
    }

    fn eval_module_source(
        &mut self,
        source: &str,
        _path: &Path,
        drain_jobs: bool,
    ) -> Result<BackendExecution, EvalFailure> {
        self.eval(
            source,
            ExecutionOptions {
                strict: true,
                drain_jobs,
                ..ExecutionOptions::default()
            },
        )
    }

    fn run_jobs(&mut self) -> Result<(), EvalFailure> {
        self.context
            .run_jobs()
            .map_err(|error| classify_error(error, &mut self.context))
    }

    fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
        self.context.strict(strict);
    }

    fn clear_output(&mut self) {
        clear_captured_output();
    }

    fn take_output(&mut self) -> Vec<String> {
        take_captured_output()
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

fn clear_captured_output() {
    CAPTURED_OUTPUT.with(|output| output.borrow_mut().clear());
}

fn take_captured_output() -> Vec<String> {
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

    EvalFailure::new(kind, rendered)
}
