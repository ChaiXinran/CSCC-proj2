//! User-defined JavaScript function values.

use crate::bytecode::Chunk;

use super::EnvironmentId;

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
