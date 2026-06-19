//! JavaScript value representation.

use std::fmt;

use super::ObjectId;

/// Value representation used by the native VM and runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Object(ObjectId),
}

impl fmt::Display for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Undefined => f.write_str("undefined"),
            Self::Null => f.write_str("null"),
            Self::Boolean(value) => value.fmt(f),
            Self::Number(value) => value.fmt(f),
            Self::String(value) => f.write_str(value),
            Self::Object(id) => write!(f, "[object #{}]", id.0),
        }
    }
}
