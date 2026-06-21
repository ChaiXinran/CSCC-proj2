//! Pure metadata and records for the V6 Error hierarchy.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ErrorConstructorSpec {
    pub name: &'static str,
    pub length: u8,
    pub prototype_name: &'static str,
    pub parent_prototype_name: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErrorRecord {
    pub name: &'static str,
    pub message: Option<String>,
}

pub(crate) const ERROR_CONSTRUCTORS: &[ErrorConstructorSpec] = &[
    ErrorConstructorSpec {
        name: "Error",
        length: 1,
        prototype_name: "Error.prototype",
        parent_prototype_name: "Object.prototype",
    },
    ErrorConstructorSpec {
        name: "EvalError",
        length: 1,
        prototype_name: "EvalError.prototype",
        parent_prototype_name: "Error.prototype",
    },
    ErrorConstructorSpec {
        name: "RangeError",
        length: 1,
        prototype_name: "RangeError.prototype",
        parent_prototype_name: "Error.prototype",
    },
    ErrorConstructorSpec {
        name: "ReferenceError",
        length: 1,
        prototype_name: "ReferenceError.prototype",
        parent_prototype_name: "Error.prototype",
    },
    ErrorConstructorSpec {
        name: "SyntaxError",
        length: 1,
        prototype_name: "SyntaxError.prototype",
        parent_prototype_name: "Error.prototype",
    },
    ErrorConstructorSpec {
        name: "TypeError",
        length: 1,
        prototype_name: "TypeError.prototype",
        parent_prototype_name: "Error.prototype",
    },
    ErrorConstructorSpec {
        name: "URIError",
        length: 1,
        prototype_name: "URIError.prototype",
        parent_prototype_name: "Error.prototype",
    },
];

#[must_use]
pub(crate) fn constructor_spec(name: &str) -> Option<&'static ErrorConstructorSpec> {
    ERROR_CONSTRUCTORS
        .iter()
        .find(|constructor| constructor.name == name)
}

#[must_use]
pub(crate) fn create_error_record(name: &'static str, message: Option<String>) -> ErrorRecord {
    ErrorRecord { name, message }
}

#[must_use]
pub(crate) fn error_to_string(record: &ErrorRecord) -> String {
    match (record.name, record.message.as_deref()) {
        ("", None | Some("")) => String::new(),
        (name, None | Some("")) => name.into(),
        ("", Some(message)) => message.into(),
        (name, Some(message)) => format!("{name}: {message}"),
    }
}
