//! Stack-based native virtual machine.

mod frame;
mod interpreter;
mod invocation;

pub use frame::{CallFrame, Completion};
pub use interpreter::{Vm, VmError, VmErrorKind};
pub(crate) use invocation::{CallRequest, ConstructRequest, InvocationOutcome};
