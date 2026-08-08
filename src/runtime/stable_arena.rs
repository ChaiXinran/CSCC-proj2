//! Stable, recyclable storage for runtime records referenced by compact IDs.

use std::marker::PhantomData;

pub trait StableId: Copy {
    fn from_u32(value: u32) -> Self;
    fn to_u32(self) -> u32;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SideSweepStats {
    pub before: usize,
    pub after: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StableArena<T, I> {
    slots: Vec<Option<T>>,
    free: Vec<u32>,
    len: usize,
    _id: PhantomData<I>,
}

impl<T, I> Default for StableArena<T, I> {
    fn default() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
            _id: PhantomData,
        }
    }
}

impl<T, I: StableId> StableArena<T, I> {
    pub fn allocate(&mut self, value: T) -> Option<I> {
        if let Some(index) = self.free.pop() {
            self.slots[index as usize] = Some(value);
            self.len += 1;
            return Some(I::from_u32(index));
        }
        let index = u32::try_from(self.slots.len()).ok()?;
        self.slots.push(Some(value));
        self.len += 1;
        Some(I::from_u32(index))
    }
    pub fn get(&self, id: I) -> Option<&T> {
        self.slots.get(id.to_u32() as usize)?.as_ref()
    }
    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.slots.get_mut(id.to_u32() as usize)?.as_mut()
    }
    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, value)| Some((I::from_u32(index as u32), value.as_ref()?)))
    }
    pub fn sweep_unmarked(&mut self, marks: &[bool]) -> SideSweepStats {
        let before = self.len;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_some() && !marks.get(index).copied().unwrap_or(false) {
                *slot = None;
                self.free.push(index as u32);
                self.len -= 1;
            }
        }
        self.shrink_if_sparse();
        SideSweepStats {
            before,
            after: self.len,
        }
    }
    fn shrink_if_sparse(&mut self) {
        if self.slots.capacity() < 1024 || self.slots.capacity() < self.len.saturating_mul(4) {
            return;
        }
        while self.slots.last().is_some_and(Option::is_none) {
            self.slots.pop();
        }
        self.free
            .retain(|index| (*index as usize) < self.slots.len());
        self.slots.shrink_to(self.len.saturating_mul(2).max(16));
        self.free
            .shrink_to(self.free.len().saturating_mul(2).max(16));
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn slots_len(&self) -> usize {
        self.slots.len()
    }
    pub fn capacity(&self) -> usize {
        self.slots.capacity()
    }
    pub fn capacity_bytes(&self) -> usize {
        self.slots
            .capacity()
            .saturating_mul(std::mem::size_of::<Option<T>>())
            .saturating_add(
                self.free
                    .capacity()
                    .saturating_mul(std::mem::size_of::<u32>()),
            )
    }
}
