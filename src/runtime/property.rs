//! ECMAScript property descriptors.

use super::JsValue;

/// Data property and its observable flags.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDescriptor {
    pub value: JsValue,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

impl PropertyDescriptor {
    #[must_use]
    pub const fn data(value: JsValue) -> Self {
        Self {
            value,
            writable: true,
            enumerable: true,
            configurable: true,
        }
    }
}
