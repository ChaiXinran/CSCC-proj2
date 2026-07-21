//! JavaScript execution backend boundary.
//!
//! The native backend is AgentJS's default self-developed parser, bytecode
//! compiler, virtual machine, and runtime. Boa remains available as an external
//! comparison engine through the pinned submodule (`boa/`), built separately via
//! `cargo build --release --manifest-path boa/Cargo.toml -p boa_cli`.

mod native;

use std::path::Path;

use crate::engine::{EvalFailure, ExecutionOptions, RuntimeConfig};

pub use native::NativeRuntime;

/// Selects the JavaScript implementation used by [`crate::Runtime`].
///
/// V12 removed the embedded Boa dispatch path. Only the native self-developed
/// engine remains in-tree. Boa is still available as an external reference
/// engine built from the pinned submodule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// AgentJS's self-developed backend.
    Native,
}

impl Default for BackendKind {
    fn default() -> Self {
        BackendKind::Native
    }
}

/// Backend-neutral result produced by one evaluation.
#[derive(Debug)]
pub(crate) struct BackendExecution {
    pub value: String,
    pub output: Vec<String>,
}

/// Internal contract implemented by every persistent JavaScript isolate.
pub(crate) trait RuntimeBackend {
    fn eval(
        &mut self,
        source: &str,
        options: ExecutionOptions,
    ) -> Result<BackendExecution, EvalFailure>;

    fn parse_only(&mut self, source: &str, options: ExecutionOptions) -> Result<(), EvalFailure>;

    fn eval_fragment(&mut self, source: &str) -> Result<(), EvalFailure>;

    fn eval_module_source(
        &mut self,
        source: &str,
        path: &Path,
        drain_jobs: bool,
    ) -> Result<BackendExecution, EvalFailure>;

    fn run_jobs(&mut self) -> Result<(), EvalFailure>;

    fn set_strict(&mut self, strict: bool);

    fn clear_output(&mut self);

    fn take_output(&mut self) -> Vec<String>;
}

pub(crate) fn create_runtime(
    kind: BackendKind,
    config: RuntimeConfig,
) -> Result<Box<dyn RuntimeBackend>, EvalFailure> {
    match kind {
        BackendKind::Native => Ok(Box::new(NativeRuntime::new(config))),
    }
}
