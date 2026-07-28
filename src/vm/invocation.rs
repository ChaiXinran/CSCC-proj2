use crate::runtime::JsValue;

/// The JavaScript completion produced by the VM invocation boundary.
///
/// Engine failures remain `Err(VmError)`; catchable JavaScript exceptions are
/// represented by `Throw` independently of the thrown value's runtime type.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum InvocationOutcome {
    Value(JsValue),
    Throw(JsValue),
}

impl InvocationOutcome {
    pub(crate) fn into_value(self) -> Result<JsValue, JsValue> {
        match self {
            Self::Value(value) => Ok(value),
            Self::Throw(value) => Err(value),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CallRequest {
    pub callee: JsValue,
    pub this_value: JsValue,
    pub arguments: Vec<JsValue>,
}

impl CallRequest {
    pub(crate) fn new(callee: JsValue, this_value: JsValue, arguments: Vec<JsValue>) -> Self {
        Self {
            callee,
            this_value,
            arguments,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConstructRequest {
    pub constructor: JsValue,
    pub arguments: Vec<JsValue>,
    pub new_target: JsValue,
}

impl ConstructRequest {
    pub(crate) fn ordinary(constructor: JsValue, arguments: Vec<JsValue>) -> Self {
        Self {
            new_target: constructor.clone(),
            constructor,
            arguments,
        }
    }

    pub(crate) fn with_new_target(
        constructor: JsValue,
        arguments: Vec<JsValue>,
        new_target: JsValue,
    ) -> Self {
        Self {
            constructor,
            arguments,
            new_target,
        }
    }
}
