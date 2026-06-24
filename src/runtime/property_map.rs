//! Ordered object property storage.

use std::collections::HashMap;

use super::{PropertyDescriptor, Trace, Tracer};

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyEntry {
    pub key: String,
    pub descriptor: PropertyDescriptor,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PropertyMap {
    entries: Vec<PropertyEntry>,
    index: HashMap<String, usize>,
}

impl PropertyMap {
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&PropertyDescriptor> {
        let index = self.index.get(key)?;
        Some(&self.entries[*index].descriptor)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut PropertyDescriptor> {
        let index = *self.index.get(key)?;
        Some(&mut self.entries[index].descriptor)
    }

    pub fn define(&mut self, key: impl Into<String>, descriptor: PropertyDescriptor) {
        let key = key.into();
        if let Some(index) = self.index.get(&key).copied() {
            self.entries[index].descriptor = descriptor;
            return;
        }

        let index = self.entries.len();
        self.entries.push(PropertyEntry {
            key: key.clone(),
            descriptor,
        });
        self.index.insert(key, index);
    }

    pub fn delete(&mut self, key: &str) -> Option<PropertyDescriptor> {
        let index = self.index.remove(key)?;
        let entry = self.entries.remove(index);
        for value in self.index.values_mut() {
            if *value > index {
                *value -= 1;
            }
        }
        Some(entry.descriptor)
    }

    #[must_use]
    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        let mut array_indices = Vec::new();
        let mut ordinary = Vec::new();

        for entry in &self.entries {
            if let Some(index) = array_index(&entry.key) {
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
    pub fn enumerable_keys(&self) -> Vec<String> {
        self.keys()
            .into_iter()
            .filter(|key| {
                self.get(key)
                    .is_some_and(|descriptor| descriptor.enumerable)
            })
            .collect()
    }
}

impl Trace for PropertyMap {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        for entry in &self.entries {
            entry.descriptor.trace(tracer);
        }
    }
}

impl PropertyMap {
    #[must_use]
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(
            self.entries
                .iter()
                .map(|entry| {
                    entry
                        .key
                        .len()
                        .saturating_add(entry.descriptor.estimated_bytes())
                })
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
