use crate::{
    backend::{BackendExecution, RuntimeBackend},
    builtins,
    contracts::{NativeContext, NativeError, NativePipeline, VmErrorKind},
    engine::{EvalFailure, ExecutionOptions, FailureKind, RuntimeConfig},
};

/// Self-developed AgentJS runtime.
///
/// This type is intentionally a compiling skeleton. Lexer, parser, bytecode,
/// value representation, and VM work will be implemented behind this boundary.
pub struct NativeRuntime {
    config: RuntimeConfig,
    context: NativeContext,
    pipeline: NativePipeline,
}

impl NativeRuntime {
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        let mut context = NativeContext::default();
        builtins::install_foundation(&mut context);
        if config.install_test262_host {
            builtins::install_test262_harness(&mut context);
        }
        Self {
            config,
            context,
            pipeline: NativePipeline::default(),
        }
    }

    fn evaluate(&mut self, source: &str) -> Result<crate::runtime::JsValue, EvalFailure> {
        self.context.reset_execution_budget(self.config.loop_limit);
        self.context
            .reset_call_depth(self.config.recursion_limit as u64);
        self.pipeline
            .evaluate(source, &mut self.context)
            .map_err(classify_native_error)
    }
}

impl RuntimeBackend for NativeRuntime {
    fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<BackendExecution, EvalFailure> {
        self.context.clear_output();
        self.context.set_strict(options.strict);
        let value = self.evaluate(source)?;
        Ok(BackendExecution {
            value: value.to_string(),
            output: self.context.take_output(),
        })
    }

    fn parse_only(&mut self, source: &str, options: ExecutionOptions) -> Result<(), EvalFailure> {
        self.context.set_strict(options.strict);
        let program = self.pipeline.parse(source).map_err(classify_native_error)?;
        self.pipeline
            .compile(&program)
            .map_err(classify_native_error)?;
        Ok(())
    }

    fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure> {
        self.evaluate(source).map(|_| ())
    }

    fn run_jobs(&mut self) -> Result<(), EvalFailure> {
        Ok(())
    }

    fn set_strict(&mut self, strict: bool) {
        self.context.set_strict(strict);
    }

    fn clear_output(&mut self) {
        self.context.clear_output();
    }

    fn take_output(&mut self) -> Vec<String> {
        self.context.take_output()
    }
}

fn classify_native_error(error: NativeError) -> EvalFailure {
    let kind = match &error {
        NativeError::Lex(_) | NativeError::Parse(_) => FailureKind::Syntax,
        NativeError::Compile(_) => FailureKind::Unsupported,
        NativeError::Execute(error) => match error.kind {
            VmErrorKind::Reference => FailureKind::Reference,
            VmErrorKind::Type => FailureKind::Type,
            VmErrorKind::Range => FailureKind::Range,
            VmErrorKind::Test262 => FailureKind::Test262,
            VmErrorKind::RuntimeLimit => FailureKind::RuntimeLimit,
            VmErrorKind::Runtime => FailureKind::Other,
        },
    };
    EvalFailure::new(kind, error.to_string())
}
