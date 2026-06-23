//! ECMAScript property descriptors.

use super::{JsValue, Trace, Tracer};

/// Complete V4 property payload stored in the heap.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyKind {
    Data {
        value: JsValue,
        writable: bool,
    },
    Accessor {
        get: Option<JsValue>,
        set: Option<JsValue>,
    },
}

/// Complete property descriptor and its observable flags.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyDescriptor {
    pub kind: PropertyKind,
    pub enumerable: bool,
    pub configurable: bool,
}

/// Partial descriptor update used by Object.defineProperty-style operations.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropertyDescriptorUpdate {
    pub value: Option<JsValue>,
    pub writable: Option<bool>,
    pub get: Option<Option<JsValue>>,
    pub set: Option<Option<JsValue>>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

impl PropertyDescriptor {
    #[must_use]
    pub const fn data(value: JsValue) -> Self {
        Self::data_with(value, true, true, true)
    }

    #[must_use]
    pub const fn data_with(
        value: JsValue,
        writable: bool,
        enumerable: bool,
        configurable: bool,
    ) -> Self {
        Self {
            kind: PropertyKind::Data { value, writable },
            enumerable,
            configurable,
        }
    }

    #[must_use]
    pub const fn accessor(
        get: Option<JsValue>,
        set: Option<JsValue>,
        enumerable: bool,
        configurable: bool,
    ) -> Self {
        Self {
            kind: PropertyKind::Accessor { get, set },
            enumerable,
            configurable,
        }
    }

    #[must_use]
    pub fn value(&self) -> Option<&JsValue> {
        match &self.kind {
            PropertyKind::Data { value, .. } => Some(value),
            PropertyKind::Accessor { .. } => None,
        }
    }

    #[must_use]
    pub fn value_cloned(&self) -> Option<JsValue> {
        self.value().cloned()
    }

    #[must_use]
    pub const fn writable(&self) -> bool {
        match &self.kind {
            PropertyKind::Data { writable, .. } => *writable,
            PropertyKind::Accessor { .. } => false,
        }
    }

    pub fn set_value(&mut self, value: JsValue) -> bool {
        match &mut self.kind {
            PropertyKind::Data {
                value: existing,
                writable,
            } if *writable => {
                *existing = value;
                true
            }
            _ => false,
        }
    }
}

impl PropertyDescriptorUpdate {
    #[must_use]
    pub fn data_value(value: JsValue) -> Self {
        Self {
            value: Some(value),
            ..Self::default()
        }
    }
}

impl Trace for PropertyDescriptor {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        match &self.kind {
            PropertyKind::Data { value, .. } => value.trace(tracer),
            PropertyKind::Accessor { get, set } => {
                if let Some(value) = get {
                    value.trace(tracer);
                }
                if let Some(value) = set {
                    value.trace(tracer);
                }
            }
        }
    }
}

impl PropertyDescriptor {
    #[must_use]
    pub(crate) fn estimated_bytes(&self) -> usize {
        let value_bytes = match &self.kind {
            PropertyKind::Data { value, .. } => value.estimated_bytes(),
            PropertyKind::Accessor { get, set } => get
                .iter()
                .chain(set.iter())
                .map(JsValue::estimated_bytes)
                .sum(),
        };
        std::mem::size_of::<Self>().saturating_add(value_bytes)
    }
}
