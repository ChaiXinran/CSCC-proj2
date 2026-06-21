//! User-defined and builtin JavaScript function values.

use std::fmt;

use crate::bytecode::Chunk;
use crate::vm::{Vm, VmError};

use super::{EnvironmentId, JsValue, NativeContext, ObjectId};

/// Stable handle into the runtime function arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

/// Runtime function object created from a bytecode template.
#[derive(Debug, Clone, PartialEq)]
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    pub chunk: Chunk,
    pub environment: Option<EnvironmentId>,
}

/// Stable handle into the builtin function registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BuiltinId(pub u16);

/// Signature for a native function invoked as a regular call.
pub type NativeCall = fn(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError>;

/// Signature for a native function invoked as a constructor (`new`).
pub type NativeConstruct = fn(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError>;

/// Bound-function state produced by `Function.prototype.bind`. When present on a
/// [`BuiltinFunction`], the VM ignores `call`/`construct` and instead forwards
/// to `target` with `this_value` and `args` prepended.
#[derive(Debug, Clone)]
pub struct BoundFunction {
    pub target: JsValue,
    pub this_value: JsValue,
    pub args: Vec<JsValue>,
}

/// Registry entry for a builtin function stored in `NativeContext`.
#[derive(Clone)]
pub struct BuiltinFunction {
    pub name: &'static str,
    pub length: u8,
    pub call: NativeCall,
    pub construct: Option<NativeConstruct>,
    /// Heap object backing this function value (holds `name`, `length`, `prototype`, etc.).
    pub object: ObjectId,
    /// Present iff this value was produced by `Function.prototype.bind`.
    pub bound: Option<BoundFunction>,
}

impl fmt::Debug for BuiltinFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuiltinFunction")
            .field("name", &self.name)
            .field("length", &self.length)
            .field("object", &self.object)
            .field("bound", &self.bound)
            .finish()
    }
}
