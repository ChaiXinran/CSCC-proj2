//! Stack-based native virtual machine.

mod frame;
mod interpreter;

pub use frame::CallFrame;
pub use interpreter::{Vm, VmError, VmErrorKind};
