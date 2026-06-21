//! JavaScript value representation.

use std::fmt;

use super::{BuiltinId, FunctionId, ObjectId};

/// Minimal native error categories used by V2 `throw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeErrorKind {
    Error,
    Test262,
}

/// Minimal error value carried by `JsValue::Error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeErrorValue {
    pub kind: NativeErrorKind,
    pub message: String,
}

impl NativeErrorValue {
    #[must_use]
    pub fn new(kind: NativeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Value representation used by the native VM and runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum JsValue {
    Undefined,
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
    Object(ObjectId),
    Function(FunctionId),
    /// A builtin function registered in `NativeContext::register_builtin`.
    BuiltinFunction(BuiltinId),
    Error(NativeErrorValue),
}

impl JsValue {
    #[must_use]
    pub fn to_boolean(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Boolean(value) => *value,
            Self::Number(value) => *value != 0.0 && !value.is_nan(),
            Self::String(value) => !value.is_empty(),
            Self::Object(_) | Self::Function(_) | Self::BuiltinFunction(_) | Self::Error(_) => true,
        }
    }

    #[must_use]
    pub fn to_number(&self) -> Option<f64> {
        match self {
            Self::Undefined => Some(f64::NAN),
            Self::Null => Some(0.0),
            Self::Boolean(true) => Some(1.0),
            Self::Boolean(false) => Some(0.0),
            Self::Number(value) => Some(*value),
            Self::String(value) => Some(string_to_number(value)),
            Self::Object(_) | Self::Function(_) | Self::BuiltinFunction(_) | Self::Error(_) => None,
        }
    }

    #[must_use]
    pub fn to_js_string(&self) -> Option<String> {
        match self {
            Self::Undefined => Some("undefined".into()),
            Self::Null => Some("null".into()),
            Self::Boolean(value) => Some(value.to_string()),
            Self::Number(value) => Some(number_to_string(*value)),
            Self::String(value) => Some(value.clone()),
            Self::Object(id) => Some(format!("[object #{}]", id.0)),
            Self::Function(_) | Self::BuiltinFunction(_) => {
                Some("function () { [native code] }".into())
            }
            Self::Error(error) => Some(error.message.clone()),
        }
    }

    #[must_use]
    pub fn type_of(&self) -> &'static str {
        match self {
            Self::Undefined => "undefined",
            Self::Null | Self::Object(_) | Self::Error(_) => "object",
            Self::Boolean(_) => "boolean",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Function(_) | Self::BuiltinFunction(_) => "function",
        }
    }

    #[must_use]
    pub fn strict_equals(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Undefined, Self::Undefined) | (Self::Null, Self::Null) => true,
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (Self::Number(left), Self::Number(right)) => {
                !left.is_nan() && !right.is_nan() && left == right
            }
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Object(left), Self::Object(right)) => left == right,
            (Self::Function(left), Self::Function(right)) => left == right,
            (Self::BuiltinFunction(left), Self::BuiltinFunction(right)) => left == right,
            (Self::Error(left), Self::Error(right)) => left == right,
            _ => false,
        }
    }

    #[must_use]
    pub fn same_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(left), Self::Number(right)) => {
                if left.is_nan() && right.is_nan() {
                    true
                } else if *left == 0.0 && *right == 0.0 {
                    left.to_bits() == right.to_bits()
                } else {
                    left == right
                }
            }
            _ => self.strict_equals(other),
        }
    }
}

impl fmt::Display for JsValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_js_string().unwrap_or_else(|| "<value>".into()))
    }
}

fn string_to_number(value: &str) -> f64 {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return 0.0;
    }

    match trimmed {
        "Infinity" | "+Infinity" => f64::INFINITY,
        "-Infinity" => f64::NEG_INFINITY,
        _ => trimmed.parse::<f64>().unwrap_or(f64::NAN),
    }
}

fn number_to_string(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "Infinity".into()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".into()
    } else if value == 0.0 && value.is_sign_negative() {
        "0".into()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::JsValue;

    #[test]
    fn implements_to_boolean_false_values() {
        assert!(!JsValue::Undefined.to_boolean());
        assert!(!JsValue::Null.to_boolean());
        assert!(!JsValue::Boolean(false).to_boolean());
        assert!(!JsValue::Number(0.0).to_boolean());
        assert!(!JsValue::Number(-0.0).to_boolean());
        assert!(!JsValue::Number(f64::NAN).to_boolean());
        assert!(!JsValue::String(String::new()).to_boolean());
    }

    #[test]
    fn reports_typeof_strings_for_v2_values() {
        assert_eq!(JsValue::Undefined.type_of(), "undefined");
        assert_eq!(JsValue::Null.type_of(), "object");
        assert_eq!(JsValue::Boolean(false).type_of(), "boolean");
        assert_eq!(JsValue::Number(1.0).type_of(), "number");
        assert_eq!(JsValue::String("x".into()).type_of(), "string");
    }

    #[test]
    fn implements_basic_to_number() {
        assert_eq!(JsValue::Null.to_number(), Some(0.0));
        assert_eq!(JsValue::Boolean(true).to_number(), Some(1.0));
        assert_eq!(JsValue::String(String::new()).to_number(), Some(0.0));
        assert_eq!(JsValue::String("  -3.5  ".into()).to_number(), Some(-3.5));
        assert!(JsValue::Undefined.to_number().unwrap().is_nan());
        assert!(
            JsValue::String("not a number".into())
                .to_number()
                .unwrap()
                .is_nan()
        );
    }

    #[test]
    fn strict_equality_matches_number_edge_cases() {
        assert!(!JsValue::Number(f64::NAN).strict_equals(&JsValue::Number(f64::NAN)));
        assert!(JsValue::Number(0.0).strict_equals(&JsValue::Number(-0.0)));
    }

    #[test]
    fn same_value_distinguishes_signed_zero_and_matches_nan() {
        assert!(JsValue::Number(f64::NAN).same_value(&JsValue::Number(f64::NAN)));
        assert!(!JsValue::Number(0.0).same_value(&JsValue::Number(-0.0)));
    }
}
