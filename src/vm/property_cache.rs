//! Per-VM named-property inline caches.

use std::collections::HashMap;

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

#[derive(Debug, Clone, Copy, Default)]
enum GetCacheState {
    #[default]
    Unobserved,
    Observed,
    Cached(GetPropertyCacheEntries),
    Rejected,
}

#[derive(Debug, Clone, Copy, Default)]
enum SetCacheState {
    #[default]
    Unobserved,
    Observed,
    Cached(SetPropertyCacheEntry),
    Rejected,
}

#[derive(Debug, Clone, Copy, Default)]
struct PropertyCacheSite {
    get: GetCacheState,
    set: SetCacheState,
}

#[derive(Debug)]
struct ChunkPropertyCaches {
    address: usize,
    sites: Vec<PropertyCacheSite>,
}

#[derive(Debug)]
pub struct PropertyInlineCaches {
    chunks: Vec<ChunkPropertyCaches>,
    chunk_indices: HashMap<usize, usize>,
    last_chunk: Option<(usize, usize)>,
    enabled: bool,
}

impl Default for PropertyInlineCaches {
    fn default() -> Self {
        Self {
            chunks: Vec::new(),
            chunk_indices: HashMap::new(),
            last_chunk: None,
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

    fn site_mut(&mut self, site: BytecodeSite) -> Option<&mut PropertyCacheSite> {
        if !self.enabled || site.instruction_offset == u32::MAX {
            return None;
        }
        let chunk_index = match self.last_chunk {
            Some((address, index)) if address == site.chunk_address => index,
            _ => {
                let index = if let Some(index) = self.chunk_indices.get(&site.chunk_address) {
                    *index
                } else {
                    let index = self.chunks.len();
                    self.chunks.push(ChunkPropertyCaches {
                        address: site.chunk_address,
                        sites: Vec::new(),
                    });
                    self.chunk_indices.insert(site.chunk_address, index);
                    index
                };
                self.last_chunk = Some((site.chunk_address, index));
                index
            }
        };
        let offset = site.instruction_offset as usize;
        let chunk = &mut self.chunks[chunk_index];
        if chunk.sites.len() <= offset {
            chunk.sites.resize(offset + 1, PropertyCacheSite::default());
        }
        Some(&mut chunk.sites[offset])
    }

    pub fn get(&mut self, site: BytecodeSite) -> Option<GetPropertyCacheEntries> {
        match self.site_mut(site)?.get {
            GetCacheState::Cached(entries) => Some(entries),
            GetCacheState::Unobserved | GetCacheState::Observed | GetCacheState::Rejected => None,
        }
    }

    pub fn update_get(&mut self, site: BytecodeSite, entry: GetPropertyCacheEntry) {
        let Some(cache_site) = self.site_mut(site) else {
            return;
        };
        match &mut cache_site.get {
            GetCacheState::Cached(entries) => {
                if !entries.insert(entry) {
                    cache_site.get = GetCacheState::Rejected;
                }
            }
            GetCacheState::Unobserved | GetCacheState::Observed => {
                let mut entries = GetPropertyCacheEntries {
                    entries: [None; GET_CACHE_POLYMORPHISM],
                };
                entries.insert(entry);
                cache_site.get = GetCacheState::Cached(entries);
            }
            GetCacheState::Rejected => {}
        }
    }

    pub fn should_specialize_get(&mut self, site: BytecodeSite) -> bool {
        let Some(cache_site) = self.site_mut(site) else {
            return false;
        };
        match cache_site.get {
            GetCacheState::Unobserved => {
                cache_site.get = GetCacheState::Observed;
                false
            }
            GetCacheState::Observed | GetCacheState::Cached(_) => true,
            GetCacheState::Rejected => false,
        }
    }

    pub fn reject_get(&mut self, site: BytecodeSite) {
        if let Some(cache_site) = self.site_mut(site) {
            cache_site.get = GetCacheState::Rejected;
        }
    }

    pub fn set(&mut self, site: BytecodeSite) -> Option<SetPropertyCacheEntry> {
        match self.site_mut(site)?.set {
            SetCacheState::Cached(entry) => Some(entry),
            SetCacheState::Unobserved | SetCacheState::Observed | SetCacheState::Rejected => None,
        }
    }

    pub fn update_set(&mut self, site: BytecodeSite, entry: SetPropertyCacheEntry) {
        if let Some(cache_site) = self.site_mut(site)
            && !matches!(cache_site.set, SetCacheState::Rejected)
        {
            cache_site.set = SetCacheState::Cached(entry);
        }
    }

    pub fn should_specialize_set(&mut self, site: BytecodeSite) -> bool {
        let Some(cache_site) = self.site_mut(site) else {
            return false;
        };
        match cache_site.set {
            SetCacheState::Unobserved => {
                cache_site.set = SetCacheState::Observed;
                false
            }
            SetCacheState::Observed | SetCacheState::Cached(_) => true,
            SetCacheState::Rejected => false,
        }
    }

    pub fn reject_set(&mut self, site: BytecodeSite) {
        if let Some(cache_site) = self.site_mut(site) {
            cache_site.set = SetCacheState::Rejected;
        }
    }

    pub fn clear(&mut self) {
        self.chunks.clear();
        self.chunk_indices.clear();
        self.last_chunk = None;
    }

    pub fn remove_chunk(&mut self, chunk: &Chunk) {
        let address = std::ptr::from_ref(chunk).addr();
        let Some(index) = self.chunk_indices.remove(&address) else {
            return;
        };
        self.chunks.swap_remove(index);
        if let Some(moved) = self.chunks.get(index) {
            self.chunk_indices.insert(moved.address, index);
        }
        self.last_chunk = None;
    }
}
