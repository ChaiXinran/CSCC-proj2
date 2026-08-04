//! Ordered object property storage.

use std::collections::HashMap;

use super::{JsString, JsValue, PropertyDescriptor, PropertyKind, Trace, Tracer};

pub type PropertyName = JsString;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropertySlotId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyEntry {
    pub key: PropertyName,
    pub descriptor: PropertyDescriptor,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PropertyMutation {
    pub slot: Option<PropertySlotId>,
    pub structural: bool,
    pub compacted: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropertyMap {
    entries: Vec<Option<PropertyEntry>>,
    index: HashMap<PropertyName, PropertySlotId>,
    tombstones: usize,
    compaction_count: u64,
    delete_count: u64,
    generation: u64,
}

impl PropertyMap {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&PropertyDescriptor> {
        self.descriptor_at(self.slot_of(key)?)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut PropertyDescriptor> {
        let slot = self.slot_of(key)?.0 as usize;
        self.entries
            .get_mut(slot)?
            .as_mut()
            .map(|entry| &mut entry.descriptor)
    }

    pub fn define(&mut self, key: impl Into<PropertyName>, descriptor: PropertyDescriptor) {
        let _ = self.define_with_outcome(key, descriptor);
    }

    pub(crate) fn define_with_outcome(
        &mut self,
        key: impl Into<PropertyName>,
        descriptor: PropertyDescriptor,
    ) -> PropertyMutation {
        let key = key.into();
        if let Some(slot) = self.index.get(key.as_str()).copied() {
            if let Some(entry) = self
                .entries
                .get_mut(slot.0 as usize)
                .and_then(Option::as_mut)
            {
                let structural = !is_data_value_only_update(&entry.descriptor, &descriptor);
                entry.descriptor = descriptor;
                if structural {
                    self.bump_generation();
                }
                return PropertyMutation {
                    slot: Some(slot),
                    structural,
                    compacted: false,
                };
            }
            debug_assert!(false, "property index points at a tombstone");
            self.index.remove(key.as_str());
        }

        let slot = PropertySlotId(
            u32::try_from(self.entries.len()).expect("property slot count exceeds u32 range"),
        );
        self.entries.push(Some(PropertyEntry {
            key: key.clone(),
            descriptor,
        }));
        self.index.insert(key, slot);
        self.bump_generation();
        PropertyMutation {
            slot: Some(slot),
            structural: true,
            compacted: false,
        }
    }

    pub fn delete(&mut self, key: &str) -> Option<PropertyDescriptor> {
        self.delete_with_outcome(key)
            .map(|(descriptor, _)| descriptor)
    }

    pub(crate) fn delete_with_outcome(
        &mut self,
        key: &str,
    ) -> Option<(PropertyDescriptor, PropertyMutation)> {
        let slot = self.index.remove(key)?;
        let entry = self.entries.get_mut(slot.0 as usize)?.take()?;
        self.tombstones = self.tombstones.saturating_add(1);
        self.delete_count = self.delete_count.saturating_add(1);
        let descriptor = entry.descriptor;
        self.bump_generation();
        let compacted = self.should_compact();
        if compacted {
            self.compact();
        }
        Some((
            descriptor,
            PropertyMutation {
                slot: Some(slot),
                structural: true,
                compacted,
            },
        ))
    }

    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    #[must_use]
    pub fn keys(&self) -> Vec<PropertyName> {
        let mut array_indices = Vec::new();
        let mut ordinary = Vec::new();

        for entry in self.entries.iter().flatten() {
            if let Some(index) = array_index(entry.key.as_str()) {
                array_indices.push((index, entry.key.clone()));
            } else {
                ordinary.push(entry.key.clone());
            }
        }

        array_indices.sort_by_key(|(index, _)| *index);
        array_indices
            .into_iter()
            .map(|(_, key)| key)
            .chain(ordinary)
            .collect()
    }

    #[must_use]
    pub fn enumerable_keys(&self) -> Vec<PropertyName> {
        self.keys()
            .into_iter()
            .filter(|key| {
                self.get(key.as_str())
                    .is_some_and(|descriptor| descriptor.enumerable)
            })
            .collect()
    }

    #[must_use]
    pub fn slot_of(&self, key: &str) -> Option<PropertySlotId> {
        self.index.get(key).copied()
    }

    #[must_use]
    pub fn descriptor_at(&self, slot: PropertySlotId) -> Option<&PropertyDescriptor> {
        self.entries
            .get(slot.0 as usize)?
            .as_ref()
            .map(|entry| &entry.descriptor)
    }

    pub(crate) fn set_data_value_at(&mut self, slot: PropertySlotId, value: JsValue) -> bool {
        let Some(descriptor) = self
            .entries
            .get_mut(slot.0 as usize)
            .and_then(Option::as_mut)
            .map(|entry| &mut entry.descriptor)
        else {
            return false;
        };
        let PropertyKind::Data {
            value: current,
            writable: true,
        } = &mut descriptor.kind
        else {
            return false;
        };
        *current = value;
        true
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn shape_entries(
        &self,
    ) -> impl Iterator<Item = (&PropertyName, PropertySlotId, &PropertyDescriptor)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                let entry = entry.as_ref()?;
                let slot = PropertySlotId(u32::try_from(index).ok()?);
                Some((&entry.key, slot, &entry.descriptor))
            })
    }

    #[must_use]
    pub fn property_count(&self) -> usize {
        self.index.len()
    }

    #[must_use]
    pub fn tombstone_count(&self) -> usize {
        self.tombstones
    }

    #[must_use]
    pub fn compaction_count(&self) -> u64 {
        self.compaction_count
    }

    #[must_use]
    pub fn delete_count(&self) -> u64 {
        self.delete_count
    }

    #[must_use]
    pub fn property_key_bytes(&self) -> usize {
        self.entries
            .iter()
            .flatten()
            .map(|entry| entry.key.len())
            .sum()
    }

    fn should_compact(&self) -> bool {
        self.entries.len() >= 64 && self.tombstones.saturating_mul(4) >= self.entries.len()
    }

    fn compact(&mut self) {
        let live_entries = self.entries.len().saturating_sub(self.tombstones);
        let mut entries = Vec::with_capacity(live_entries);
        let mut index = HashMap::with_capacity(live_entries);
        for entry in self.entries.drain(..).flatten() {
            let slot = PropertySlotId(
                u32::try_from(entries.len()).expect("property slot count exceeds u32 range"),
            );
            index.insert(entry.key.clone(), slot);
            entries.push(Some(entry));
        }
        self.entries = entries;
        self.index = index;
        self.tombstones = 0;
        self.compaction_count = self.compaction_count.saturating_add(1);
        self.bump_generation();
    }

    fn bump_generation(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

fn is_data_value_only_update(
    current: &PropertyDescriptor,
    replacement: &PropertyDescriptor,
) -> bool {
    matches!(
        (&current.kind, &replacement.kind),
        (PropertyKind::Data { writable: left, .. }, PropertyKind::Data { writable: right, .. })
            if left == right
                && current.enumerable == replacement.enumerable
                && current.configurable == replacement.configurable
    )
}

impl Trace for PropertyMap {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for entry in self.entries.iter().flatten() {
            entry.descriptor.trace(tracer);
        }
    }
}

impl PropertyMap {
    #[must_use]
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.entries
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Option<PropertyEntry>>()),
            )
            .saturating_add(
                self.index
                    .capacity()
                    .saturating_mul(std::mem::size_of::<(PropertyName, PropertySlotId)>()),
            )
            .saturating_add(self.property_key_bytes())
            .saturating_add(
                self.entries
                    .iter()
                    .flatten()
                    .map(|entry| entry.descriptor.estimated_bytes())
                    .sum::<usize>(),
            )
    }
}

fn array_index(key: &str) -> Option<usize> {
    if key.is_empty() || !key.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let index = key.parse::<u32>().ok()?;
    if index == u32::MAX {
        return None;
    }
    (index.to_string() == key).then_some(index as usize)
}

#[cfg(test)]
mod tests {
    use super::PropertyMap;
    use crate::runtime::{JsString, JsValue, PropertyDescriptor};

    fn descriptor(value: f64) -> PropertyDescriptor {
        PropertyDescriptor::data(JsValue::Number(value))
    }

    fn key_strings(map: &PropertyMap) -> Vec<String> {
        map.keys().into_iter().map(JsString::into_owned).collect()
    }

    #[test]
    fn delete_and_redefine_appends_without_moving_other_slots() {
        let mut map = PropertyMap::default();
        map.define("a", descriptor(1.0));
        map.define("b", descriptor(2.0));
        let b_slot = map.slot_of("b").unwrap();

        map.delete("a").unwrap();
        map.define("a", descriptor(3.0));

        assert_eq!(key_strings(&map), ["b", "a"]);
        assert_eq!(map.slot_of("b"), Some(b_slot));
        assert_eq!(map.tombstone_count(), 1);
    }

    #[test]
    fn compaction_preserves_ecmascript_key_order() {
        let mut map = PropertyMap::default();
        for index in 0..80 {
            map.define(format!("key-{index}"), descriptor(index as f64));
        }
        map.define("10", descriptor(10.0));
        map.define("2", descriptor(2.0));
        for index in 0..24 {
            map.delete(&format!("key-{index}")).unwrap();
        }

        assert_eq!(map.compaction_count(), 1);
        assert!(map.tombstone_count() * 4 < map.entries.len());
        let keys = key_strings(&map);
        assert_eq!(&keys[..2], ["2", "10"]);
        assert_eq!(&keys[2..4], ["key-24", "key-25"]);
    }

    #[test]
    fn updating_a_descriptor_does_not_change_order_or_slot() {
        let mut map = PropertyMap::default();
        map.define("a", descriptor(1.0));
        let slot = map.slot_of("a").unwrap();
        map.define("a", descriptor(2.0));

        assert_eq!(map.slot_of("a"), Some(slot));
        assert_eq!(key_strings(&map), ["a"]);
        assert_eq!(map.property_count(), 1);
    }

    #[test]
    fn generation_changes_only_for_structural_mutations() {
        let mut map = PropertyMap::default();
        map.define("value", descriptor(1.0));
        let after_insert = map.generation();
        let slot = map.slot_of("value").unwrap();

        map.define("value", descriptor(2.0));
        assert_eq!(map.generation(), after_insert);
        assert!(map.set_data_value_at(slot, JsValue::Number(3.0)));
        assert_eq!(map.generation(), after_insert);

        map.define(
            "value",
            PropertyDescriptor::data_with(JsValue::Number(4.0), false, true, true),
        );
        assert!(map.generation() > after_insert);
        let after_reconfigure = map.generation();
        map.delete("value").unwrap();
        assert!(map.generation() > after_reconfigure);
    }
}
