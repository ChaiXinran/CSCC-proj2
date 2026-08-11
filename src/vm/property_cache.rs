//! Per-VM monomorphic named-property inline caches.

use std::collections::{HashMap, HashSet};

use crate::{
    bytecode::Chunk,
    runtime::{PropertySlotId, ShapeId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BytecodeSite {
    pub chunk_address: usize,
    pub instruction_offset: u32,
}

impl BytecodeSite {
    #[must_use]
    pub fn new(chunk: &Chunk, instruction_offset: usize) -> Self {
        Self {
            chunk_address: std::ptr::from_ref(chunk).addr(),
            instruction_offset: u32::try_from(instruction_offset).unwrap_or(u32::MAX),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetPropertyCacheEntry {
    pub receiver_shape: ShapeId,
    pub property_generation: u64,
    pub slot: PropertySlotId,
}

const GET_CACHE_POLYMORPHISM: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GetPropertyCacheEntries {
    entries: [Option<GetPropertyCacheEntry>; GET_CACHE_POLYMORPHISM],
}

impl GetPropertyCacheEntries {
    pub fn iter(self) -> impl Iterator<Item = GetPropertyCacheEntry> {
        self.entries.into_iter().flatten()
    }

    pub fn find(self, receiver_shape: ShapeId) -> Option<GetPropertyCacheEntry> {
        self.iter()
            .find(|entry| entry.receiver_shape == receiver_shape)
    }

    fn insert(&mut self, entry: GetPropertyCacheEntry) -> bool {
        if let Some(existing) = self
            .entries
            .iter_mut()
            .flatten()
            .find(|existing| existing.receiver_shape == entry.receiver_shape)
        {
            *existing = entry;
            return true;
        }
        let Some(empty) = self.entries.iter_mut().find(|entry| entry.is_none()) else {
            return false;
        };
        *empty = Some(entry);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetPropertyCacheEntry {
    pub receiver_shape: ShapeId,
    pub property_generation: u64,
    pub slot: PropertySlotId,
}

#[derive(Debug)]
pub struct PropertyInlineCaches {
    get: HashMap<BytecodeSite, GetPropertyCacheEntries>,
    set: HashMap<BytecodeSite, SetPropertyCacheEntry>,
    observed_get: HashSet<BytecodeSite>,
    observed_set: HashSet<BytecodeSite>,
    rejected_get: HashSet<BytecodeSite>,
    rejected_set: HashSet<BytecodeSite>,
    enabled: bool,
}

impl Default for PropertyInlineCaches {
    fn default() -> Self {
        Self {
            get: HashMap::new(),
            set: HashMap::new(),
            observed_get: HashSet::new(),
            observed_set: HashSet::new(),
            rejected_get: HashSet::new(),
            rejected_set: HashSet::new(),
            enabled: true,
        }
    }
}

impl PropertyInlineCaches {
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
        self.clear();
    }

    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn get(&self, site: BytecodeSite) -> Option<GetPropertyCacheEntries> {
        self.enabled.then(|| self.get.get(&site).copied()).flatten()
    }

    pub fn update_get(&mut self, site: BytecodeSite, entry: GetPropertyCacheEntry) {
        if self.enabled {
            let entries = self.get.entry(site).or_insert(GetPropertyCacheEntries {
                entries: [None; GET_CACHE_POLYMORPHISM],
            });
            if !entries.insert(entry) {
                self.reject_get(site);
            }
        }
    }

    pub fn should_specialize_get(&mut self, site: BytecodeSite) -> bool {
        self.enabled && !self.rejected_get.contains(&site) && !self.observed_get.insert(site)
    }

    pub fn reject_get(&mut self, site: BytecodeSite) {
        self.get.remove(&site);
        self.rejected_get.insert(site);
    }

    #[must_use]
    pub fn set(&self, site: BytecodeSite) -> Option<SetPropertyCacheEntry> {
        self.enabled.then(|| self.set.get(&site).copied()).flatten()
    }

    pub fn update_set(&mut self, site: BytecodeSite, entry: SetPropertyCacheEntry) {
        if self.enabled {
            self.set.insert(site, entry);
        }
    }

    pub fn should_specialize_set(&mut self, site: BytecodeSite) -> bool {
        self.enabled && !self.rejected_set.contains(&site) && !self.observed_set.insert(site)
    }

    pub fn reject_set(&mut self, site: BytecodeSite) {
        self.set.remove(&site);
        self.rejected_set.insert(site);
    }

    pub fn clear(&mut self) {
        self.get.clear();
        self.set.clear();
        self.observed_get.clear();
        self.observed_set.clear();
        self.rejected_get.clear();
        self.rejected_set.clear();
    }

    pub fn remove_chunk(&mut self, chunk: &Chunk) {
        let address = std::ptr::from_ref(chunk).addr();
        self.get.retain(|site, _| site.chunk_address != address);
        self.set.retain(|site, _| site.chunk_address != address);
        self.observed_get
            .retain(|site| site.chunk_address != address);
        self.observed_set
            .retain(|site| site.chunk_address != address);
        self.rejected_get
            .retain(|site| site.chunk_address != address);
        self.rejected_set
            .retain(|site| site.chunk_address != address);
    }
}
