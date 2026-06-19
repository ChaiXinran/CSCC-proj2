use crate::{
    backend::{BackendExecution, RuntimeBackend},
    builtins,
    contracts::NativePipeline,
    engine::{EvalFailure, ExecutionOptions, FailureKind, RuntimeConfig},
    runtime::Heap,
};

/// Self-developed AgentJS runtime.
///
/// This type is intentionally a compiling skeleton. Lexer, parser, bytecode,
/// value representation, and VM work will be implemented behind this boundary.
pub struct NativeRuntime {
    _config: RuntimeConfig,
    strict: bool,
    output: Vec<String>,
    _heap: Heap,
    _pipeline: NativePipeline,
}

impl NativeRuntime {
    #[must_use]
    pub fn new(config: RuntimeConfig) -> Self {
        let mut heap = Heap::default();
        builtins::install_foundation(&mut heap);
        Self {
            _config: config,
            strict: false,
            output: Vec::new(),
            _heap: heap,
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
        self.strict = strict;
    }

    fn clear_output(&mut self) {
        self.output.clear();
    }

    fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }
}
