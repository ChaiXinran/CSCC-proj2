//! JavaScript objects and prototype links.

use super::{JsValue, PropertyDescriptor, PropertyMap};

/// Stable handle into the runtime heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

/// Minimal object storage variants used by V3.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ObjectKind {
    #[default]
    Ordinary,
    Array {
        elements: Vec<Option<PropertyDescriptor>>,
        length_writable: bool,
    },
}

/// Ordinary object storage.
#[derive(Debug, Clone, Default)]
pub struct JsObject {
    pub prototype: Option<ObjectId>,
    pub kind: ObjectKind,
    pub properties: PropertyMap,
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
            kind: ObjectKind::Array {
                elements: elements
                    .into_iter()
                    .map(|value| Some(PropertyDescriptor::data(value)))
                    .collect(),
                length_writable: true,
            },
            properties: PropertyMap::default(),
        }
    }

    #[must_use]
    pub fn sparse_array(length: usize) -> Self {
        Self {
            prototype: None,
            kind: ObjectKind::Array {
                elements: vec![None; length],
                length_writable: true,
            },
            properties: PropertyMap::default(),
        }
    }

    pub fn define_property(&mut self, name: impl Into<String>, descriptor: PropertyDescriptor) {
        self.properties.define(name, descriptor);
    }

    #[must_use]
    pub fn own_property(&self, name: &str) -> Option<&PropertyDescriptor> {
        self.properties.get(name)
    }

    #[must_use]
    pub fn get_own_property_value(&self, name: &str) -> Option<JsValue> {
        if let ObjectKind::Array { elements, .. } = &self.kind {
            if name == "length" {
                return Some(JsValue::Number(elements.len() as f64));
            }
            if let Some(index) = array_index(name) {
                return elements
                    .get(index)
                    .and_then(|descriptor| descriptor.as_ref())
                    .and_then(PropertyDescriptor::value_cloned);
            }
        }

        self.own_property(name)
            .and_then(PropertyDescriptor::value_cloned)
    }

    pub fn set_own_property_value(&mut self, name: impl Into<String>, value: JsValue) -> bool {
        let name = name.into();
        if let ObjectKind::Array {
            elements,
            length_writable,
        } = &mut self.kind
        {
            if name == "length" {
                return false;
            }
            if let Some(index) = array_index(&name) {
                if index >= elements.len() && !*length_writable {
                    return false;
                }
                if index >= elements.len() {
                    elements.resize(index + 1, None);
                }
                if let Some(descriptor) = &mut elements[index] {
                    return descriptor.set_value(value);
                }
                elements[index] = Some(PropertyDescriptor::data(value));
                return true;
            }
        }

        if let Some(descriptor) = self.properties.get_mut(&name) {
            descriptor.set_value(value)
        } else {
            self.define_property(name, PropertyDescriptor::data(value));
            true
        }
    }

    #[must_use]
    pub fn has_own_property(&self, name: &str) -> bool {
        if let ObjectKind::Array { elements, .. } = &self.kind {
            if name == "length" {
                return true;
            }
            if let Some(index) = array_index(name) {
                return elements.get(index).is_some_and(Option::is_some);
            }
        }
        self.properties.contains_key(name)
    }

    pub fn delete_own_property(&mut self, name: &str) -> Option<PropertyDescriptor> {
        if let ObjectKind::Array { elements, .. } = &mut self.kind
            && let Some(index) = array_index(name)
        {
            if let Some(slot) = elements.get_mut(index)
                && let Some(descriptor) = slot.take()
            {
                return Some(descriptor);
            }
            return None;
        }
        self.properties.delete(name)
    }

    #[must_use]
    pub fn array_length(&self) -> Option<usize> {
        match &self.kind {
            ObjectKind::Array { elements, .. } => Some(elements.len()),
            ObjectKind::Ordinary => None,
        }
    }

    #[must_use]
    pub fn array_length_writable(&self) -> Option<bool> {
        match &self.kind {
            ObjectKind::Array {
                length_writable, ..
            } => Some(*length_writable),
            ObjectKind::Ordinary => None,
        }
    }

    pub fn set_array_length(&mut self, length: usize) -> bool {
        let ObjectKind::Array {
            elements,
            length_writable,
        } = &mut self.kind
        else {
            return false;
        };
        if !*length_writable {
            return false;
        }
        if length >= elements.len() {
            elements.resize(length, None);
            return true;
        }

        for index in (length..elements.len()).rev() {
            if elements[index]
                .as_ref()
                .is_some_and(|descriptor| !descriptor.configurable)
            {
                elements.truncate(index + 1);
                return false;
            }
            elements[index] = None;
        }
        elements.truncate(length);
        true
    }

    pub fn set_array_length_writable(&mut self, writable: bool) -> bool {
        let ObjectKind::Array {
            length_writable, ..
        } = &mut self.kind
        else {
            return false;
        };
        *length_writable = writable;
        true
    }

    #[must_use]
    pub fn array_element_descriptor(&self, index: usize) -> Option<PropertyDescriptor> {
        let ObjectKind::Array { elements, .. } = &self.kind else {
            return None;
        };
        elements.get(index).cloned().flatten()
    }

    pub fn define_array_element(&mut self, index: usize, descriptor: PropertyDescriptor) -> bool {
        let ObjectKind::Array {
            elements,
            length_writable,
        } = &mut self.kind
        else {
            return false;
        };
        if index >= elements.len() && !*length_writable {
            return false;
        }
        if index >= elements.len() {
            elements.resize(index + 1, None);
        }
        elements[index] = Some(descriptor);
        true
    }

    #[must_use]
    pub fn own_property_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if let ObjectKind::Array { elements, .. } = &self.kind {
            keys.extend(
                elements
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| value.is_some())
                    .map(|(index, _)| index.to_string()),
            );
        }
        keys.extend(self.properties.keys());
        keys
    }
}

pub(crate) fn array_index(name: &str) -> Option<usize> {
    if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let index = name.parse::<usize>().ok()?;
    (index.to_string() == name).then_some(index)
}
