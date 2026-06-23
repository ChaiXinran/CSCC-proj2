//! JavaScript execution backend boundary.
//!
//! The Boa backend preserves the current conformance baseline. The native
//! backend is the replacement target for AgentJS's self-developed parser,
//! bytecode compiler, virtual machine, and runtime.

#[cfg(feature = "boa-backend")]
mod boa;
mod native;

use crate::engine::{EvalFailure, ExecutionOptions, RuntimeConfig};

#[cfg(feature = "boa-backend")]
pub use boa::BoaRuntime;
pub use native::NativeRuntime;

/// Selects the JavaScript implementation used by [`crate::Runtime`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Compatibility backend built on Boa.
    #[cfg(feature = "boa-backend")]
    Boa,
    /// AgentJS's self-developed backend.
    Native,
}

#[allow(clippy::derivable_impls)] // default variant is cfg-conditional
impl Default for BackendKind {
    fn default() -> Self {
        #[cfg(feature = "boa-backend")]
        return BackendKind::Boa;
        #[cfg(not(feature = "boa-backend"))]
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
        #[cfg(feature = "boa-backend")]
        BackendKind::Boa => Ok(Box::new(BoaRuntime::new(config)?)),
        BackendKind::Native => Ok(Box::new(NativeRuntime::new(config))),
    }
}
