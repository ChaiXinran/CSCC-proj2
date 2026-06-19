//! JavaScript objects and prototype links.

use std::collections::HashMap;

use super::{JsValue, PropertyDescriptor};

/// Stable handle into the runtime heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

/// Minimal object storage variants used by V3.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ObjectKind {
    #[default]
    Ordinary,
    Array {
        elements: Vec<JsValue>,
    },
}

/// Ordinary object storage.
#[derive(Debug, Clone, Default)]
pub struct JsObject {
    pub prototype: Option<ObjectId>,
    pub kind: ObjectKind,
    pub properties: HashMap<String, PropertyDescriptor>,
}

impl JsObject {
    #[must_use]
    pub fn ordinary() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn array(elements: Vec<JsValue>) -> Self {
        Self {
            prototype: None,
            kind: ObjectKind::Array { elements },
            properties: HashMap::new(),
        }
    }

    pub fn define_property(&mut self, name: impl Into<String>, descriptor: PropertyDescriptor) {
        self.properties.insert(name.into(), descriptor);
    }

    #[must_use]
    pub fn own_property(&self, name: &str) -> Option<&PropertyDescriptor> {
        self.properties.get(name)
    }

    #[must_use]
    pub fn get_own_property_value(&self, name: &str) -> Option<JsValue> {
        if let ObjectKind::Array { elements } = &self.kind {
            if name == "length" {
                return Some(JsValue::Number(elements.len() as f64));
            }
            if let Some(index) = array_index(name) {
                return elements.get(index).cloned();
            }
        }

        self.own_property(name)
            .map(|descriptor| descriptor.value.clone())
    }

    pub fn set_own_property_value(&mut self, name: impl Into<String>, value: JsValue) -> bool {
        let name = name.into();
        if let ObjectKind::Array { elements } = &mut self.kind {
            if name == "length" {
                return false;
            }
            if let Some(index) = array_index(&name) {
                if index > elements.len() {
                    return false;
                }
                if index == elements.len() {
                    elements.push(value);
                } else {
                    elements[index] = value;
                }
                return true;
            }
        }

        if let Some(descriptor) = self.properties.get_mut(&name) {
            if !descriptor.writable {
                return false;
            }
            descriptor.value = value;
            true
        } else {
            self.define_property(name, PropertyDescriptor::data(value));
            true
        }
    }
}

fn array_index(name: &str) -> Option<usize> {
    if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let index = name.parse::<usize>().ok()?;
    (index.to_string() == name).then_some(index)
}
