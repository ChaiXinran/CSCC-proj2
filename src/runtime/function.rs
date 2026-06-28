//! User-defined and builtin JavaScript function values.

use std::fmt;

use crate::bytecode::{Chunk, Constant};
use crate::vm::{Vm, VmError};

use super::{EnvironmentId, JsValue, NativeContext, ObjectId, Trace, Tracer};

/// Stable handle into the runtime function arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunctionId(pub u32);

/// Runtime function object created from a bytecode template.
#[derive(Debug, Clone, PartialEq)]
pub struct JsFunction {
    pub name: Option<String>,
    pub params: Vec<String>,
    /// If present, collects remaining arguments into an array with this name.
    pub rest_param: Option<String>,
    /// Function.length override: params before first default/rest/pattern param.
    /// None means fall back to params.len().
    pub length_override: Option<u32>,
    pub chunk: Chunk,
    pub environment: Option<EnvironmentId>,
    pub is_async: bool,
    pub is_generator: bool,
    pub is_arrow: bool,
    pub uses_arguments: bool,
    pub lexical_this: Option<JsValue>,
    pub lexical_new_target: Option<JsValue>,
    pub home_object: Option<ObjectId>,
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

impl Trace for JsFunction {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(environment) = self.environment {
            tracer.mark_environment(environment);
        }
        if let Some(value) = &self.lexical_this {
            value.trace(tracer);
        }
        if let Some(value) = &self.lexical_new_target {
            value.trace(tracer);
        }
        if let Some(object) = self.home_object {
            tracer.mark_object(object);
        }
    }
}

impl JsFunction {
    #[must_use]
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.name.as_ref().map_or(0, String::len))
            .saturating_add(self.params.iter().map(String::len).sum::<usize>())
            .saturating_add(estimate_chunk_bytes(&self.chunk))
    }
}

impl Trace for BoundFunction {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        self.target.trace(tracer);
        self.this_value.trace(tracer);
        for arg in &self.args {
            arg.trace(tracer);
        }
    }
}

impl Trace for BuiltinFunction {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.mark_object(self.object);
        if let Some(bound) = &self.bound {
            bound.trace(tracer);
        }
    }
}

fn estimate_chunk_bytes(chunk: &Chunk) -> usize {
    chunk
        .instructions
        .len()
        .saturating_mul(std::mem::size_of::<crate::bytecode::Instruction>())
        .saturating_add(
            chunk
                .constants
                .iter()
                .map(|constant| match constant {
                    Constant::String(value) => value.len(),
                    _ => std::mem::size_of::<Constant>(),
                })
                .sum::<usize>(),
        )
        .saturating_add(chunk.functions.len().saturating_mul(64))
}
