//! JavaScript objects and prototype links.

use std::collections::HashMap;

use super::PropertyDescriptor;

/// Stable handle into the runtime heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

/// Ordinary object storage.
#[derive(Debug, Clone, Default)]
pub struct JsObject {
    pub prototype: Option<ObjectId>,
    pub properties: HashMap<String, PropertyDescriptor>,
}

impl JsObject {
    pub fn define_property(&mut self, name: impl Into<String>, descriptor: PropertyDescriptor) {
        self.properties.insert(name.into(), descriptor);
    }

    #[must_use]
    pub fn own_property(&self, name: &str) -> Option<&PropertyDescriptor> {
        self.properties.get(name)
    }

    #[must_use]
    pub fn get_own_property_value(&self, name: &str) -> Option<super::JsValue> {
        self.own_property(name)
            .map(|descriptor| descriptor.value.clone())
    }
}
