use crate::{
    backend::{BackendExecution, RuntimeBackend},
    builtins,
    contracts::{NativeContext, NativePipeline},
    engine::{EvalFailure, ExecutionOptions, FailureKind, RuntimeConfig},
};

/// Self-developed AgentJS runtime.
///
/// This type is intentionally a compiling skeleton. Lexer, parser, bytecode,
/// value representation, and VM work will be implemented behind this boundary.
pub struct NativeRuntime {
    _config: RuntimeConfig,
    context: NativeContext,
    _pipeline: NativePipeline,
}

impl NativeRuntime {
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        let mut context = NativeContext::default();
        builtins::install_foundation(context.heap_mut());
        if config.install_test262_host {
            builtins::install_test262_harness(&mut context);
        }
        Self {
            _config: config,
            context,
            _pipeline: NativePipeline::default(),
        }
    }

    fn not_implemented() -> EvalFailure {
        EvalFailure::new(
            FailureKind::Unsupported,
            "the native AgentJS backend is not implemented yet",
        )
    }
}

impl RuntimeBackend for NativeRuntime {
    fn eval(
        &mut self,
        _source: &str,
        _options: ExecutionOptions,
    ) -> Result<BackendExecution, EvalFailure> {
        Err(Self::not_implemented())
    }

    fn eval_fragment(&mut self, _source: &str) -> Result<(), EvalFailure> {
        Err(Self::not_implemented())
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
