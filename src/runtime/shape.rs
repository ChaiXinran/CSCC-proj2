//! Per-isolate ordinary-object shapes.

use std::collections::HashMap;

use super::{PropertyDescriptor, PropertyKind, PropertyName, PropertySlotId};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u32);

pub const ROOT_SHAPE: ShapeId = ShapeId(0);
pub const DICTIONARY_SHAPE: ShapeId = ShapeId(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeMode {
    Fast,
    Dictionary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyKindTag {
    Data,
    Accessor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertyAttributes {
    pub kind: PropertyKindTag,
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

impl PropertyAttributes {
    #[must_use]
    pub fn from_descriptor(descriptor: &PropertyDescriptor) -> Self {
        let (kind, writable) = match &descriptor.kind {
            PropertyKind::Data { writable, .. } => (PropertyKindTag::Data, *writable),
            PropertyKind::Accessor { .. } => (PropertyKindTag::Accessor, false),
        };
        Self {
            kind,
            writable,
            enumerable: descriptor.enumerable,
            configurable: descriptor.configurable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeRecord {
    pub parent: Option<ShapeId>,
    pub property: Option<PropertyName>,
    pub slot: Option<PropertySlotId>,
    pub attributes: Option<PropertyAttributes>,
    pub mode: ShapeMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ShapeTransitionKey {
    parent: ShapeId,
    property: PropertyName,
    slot: PropertySlotId,
    attributes: PropertyAttributes,
}

#[derive(Debug)]
pub struct ShapeTable {
    shapes: Vec<ShapeRecord>,
    transitions: HashMap<ShapeTransitionKey, ShapeId>,
}

impl Default for ShapeTable {
    fn default() -> Self {
        Self {
            shapes: vec![
                ShapeRecord {
                    parent: None,
                    property: None,
                    slot: None,
                    attributes: None,
                    mode: ShapeMode::Fast,
                },
                ShapeRecord {
                    parent: None,
                    property: None,
                    slot: None,
                    attributes: None,
                    mode: ShapeMode::Dictionary,
                },
            ],
            transitions: HashMap::new(),
        }
    }
}

impl ShapeTable {
    #[must_use]
    pub fn record(&self, shape: ShapeId) -> Option<&ShapeRecord> {
        self.shapes.get(shape.0 as usize)
    }

    pub fn transition(
        &mut self,
        parent: ShapeId,
        property: PropertyName,
        slot: PropertySlotId,
        attributes: PropertyAttributes,
    ) -> (ShapeId, bool) {
        let key = ShapeTransitionKey {
            parent,
            property,
            slot,
            attributes,
        };
        if let Some(shape) = self.transitions.get(&key) {
            return (*shape, false);
        }
        let shape =
            ShapeId(u32::try_from(self.shapes.len()).expect("shape table exceeds u32 index range"));
        self.shapes.push(ShapeRecord {
            parent: Some(parent),
            property: Some(key.property.clone()),
            slot: Some(slot),
            attributes: Some(attributes),
            mode: ShapeMode::Fast,
        });
        self.transitions.insert(key, shape);
        (shape, true)
    }

    #[must_use]
    pub fn shape_count(&self) -> usize {
        self.shapes.len()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PropertyCacheMetrics {
    pub get_hits: u64,
    pub get_misses: u64,
    pub set_hits: u64,
    pub set_misses: u64,
    pub shape_transitions: u64,
    pub dictionary_objects: u64,
    pub invalidations: u64,
}
