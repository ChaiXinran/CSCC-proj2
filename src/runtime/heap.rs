//! Object and environment allocation.

use super::gc::{CollectionStats, HeapMarks, HeapStats};
use super::{Environment, EnvironmentId, FunctionId, JsFunction, JsObject, ObjectId};

/// Arena owning native runtime objects and lexical environments.
#[derive(Debug)]
pub struct Heap {
    objects: Vec<Option<JsObject>>,
    environments: Vec<Option<Environment>>,
    functions: Vec<Option<JsFunction>>,
    /// Hard cap on total live allocations.
    limit: usize,
    /// Conservative cap on heap-owned and guarded allocation bytes.
    byte_limit: usize,
    estimated_bytes: usize,
    allocation_count: u64,
    allocations_since_collection: usize,
    collection_count: u64,
}

impl Default for Heap {
    fn default() -> Self {
        Self {
            objects: Vec::new(),
            environments: Vec::new(),
            functions: Vec::new(),
            limit: usize::MAX,
            byte_limit: usize::MAX,
            estimated_bytes: 0,
            allocation_count: 0,
            allocations_since_collection: 0,
            collection_count: 0,
        }
    }
}

impl Heap {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }

    pub fn with_limits(object_limit: usize, byte_limit: usize) -> Self {
        Self {
            limit: object_limit,
            byte_limit,
            ..Self::default()
        }
    }

    pub fn set_byte_limit(&mut self, byte_limit: usize) {
        self.byte_limit = byte_limit;
    }

    fn total_live_count(&self) -> usize {
        self.live_objects() + self.live_environments() + self.live_functions()
    }

    fn can_allocate(&self, additional_bytes: usize) -> bool {
        self.total_live_count() < self.limit
            && self
                .estimated_bytes
                .checked_add(additional_bytes)
                .is_some_and(|total| total <= self.byte_limit)
    }

    pub fn can_charge_bytes(&self, additional_bytes: usize) -> bool {
        self.estimated_bytes
            .checked_add(additional_bytes)
            .is_some_and(|total| total <= self.byte_limit)
    }

    pub fn charge_bytes(&mut self, additional_bytes: usize) -> bool {
        if !self.can_charge_bytes(additional_bytes) {
            return false;
        }
        self.estimated_bytes = self.estimated_bytes.saturating_add(additional_bytes);
        true
    }

    fn note_allocation(&mut self, estimated_bytes: usize) {
        self.estimated_bytes = self.estimated_bytes.saturating_add(estimated_bytes);
        self.allocation_count = self.allocation_count.saturating_add(1);
        self.allocations_since_collection = self.allocations_since_collection.saturating_add(1);
    }

    pub fn should_collect(&self, threshold: usize) -> bool {
        threshold > 0 && self.allocations_since_collection >= threshold
    }

    pub fn allocate_object(&mut self, object: JsObject) -> Option<ObjectId> {
        let estimated_bytes = object.estimated_bytes();
        if !self.can_allocate(estimated_bytes) {
            return None;
        }
        if let Some((index, slot)) = self
            .objects
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.is_none())
        {
            *slot = Some(object);
            self.note_allocation(estimated_bytes);
            return Some(ObjectId(u32::try_from(index).ok()?));
        }
        let id = ObjectId(u32::try_from(self.objects.len()).ok()?);
        self.objects.push(Some(object));
        self.note_allocation(estimated_bytes);
        Some(id)
    }

    #[must_use]
    pub fn object(&self, id: ObjectId) -> Option<&JsObject> {
        self.objects.get(id.0 as usize)?.as_ref()
    }

    pub fn object_mut(&mut self, id: ObjectId) -> Option<&mut JsObject> {
        self.objects.get_mut(id.0 as usize)?.as_mut()
    }

    #[must_use]
    pub fn contains_object(&self, id: ObjectId) -> bool {
        self.object(id).is_some()
    }

    pub fn allocate_environment(&mut self, environment: Environment) -> Option<EnvironmentId> {
        let estimated_bytes = environment.estimated_bytes();
        if !self.can_allocate(estimated_bytes) {
            return None;
        }
        if let Some((index, slot)) = self
            .environments
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.is_none())
        {
            *slot = Some(environment);
            self.note_allocation(estimated_bytes);
            return Some(EnvironmentId(u32::try_from(index).ok()?));
        }
        let id = EnvironmentId(u32::try_from(self.environments.len()).ok()?);
        self.environments.push(Some(environment));
        self.note_allocation(estimated_bytes);
        Some(id)
    }

    #[must_use]
    pub fn environment(&self, id: EnvironmentId) -> Option<&Environment> {
        self.environments.get(id.0 as usize)?.as_ref()
    }

    pub fn environment_mut(&mut self, id: EnvironmentId) -> Option<&mut Environment> {
        self.environments.get_mut(id.0 as usize)?.as_mut()
    }

    #[must_use]
    pub fn contains_environment(&self, id: EnvironmentId) -> bool {
        self.environment(id).is_some()
    }

    pub fn allocate_function(&mut self, function: JsFunction) -> Option<FunctionId> {
        let estimated_bytes = function.estimated_bytes();
        if !self.can_allocate(estimated_bytes) {
            return None;
        }
        if let Some((index, slot)) = self
            .functions
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.is_none())
        {
            *slot = Some(function);
            self.note_allocation(estimated_bytes);
            return Some(FunctionId(u32::try_from(index).ok()?));
        }
        let id = FunctionId(u32::try_from(self.functions.len()).ok()?);
        self.functions.push(Some(function));
        self.note_allocation(estimated_bytes);
        Some(id)
    }

    #[must_use]
    pub fn function(&self, id: FunctionId) -> Option<&JsFunction> {
        self.functions.get(id.0 as usize)?.as_ref()
    }

    #[must_use]
    pub fn contains_function(&self, id: FunctionId) -> bool {
        self.function(id).is_some()
    }

    #[must_use]
    pub fn object_count(&self) -> usize {
        self.live_objects()
    }

    #[must_use]
    pub fn stats(&self) -> HeapStats {
        HeapStats {
            object_slots: self.objects.len(),
            live_objects: self.live_objects(),
            live_environments: self.live_environments(),
            live_functions: self.live_functions(),
            estimated_bytes: self.estimated_bytes,
            allocation_count: self.allocation_count,
            collection_count: self.collection_count,
        }
    }

    pub(crate) fn sweep(&mut self, marks: &HeapMarks) -> CollectionStats {
        let before = self.stats();
        for (index, slot) in self.objects.iter_mut().enumerate() {
            if slot.is_some() && !marks.objects.contains(&ObjectId(index as u32)) {
                *slot = None;
            }
        }
        for (index, slot) in self.environments.iter_mut().enumerate() {
            if slot.is_some() && !marks.environments.contains(&EnvironmentId(index as u32)) {
                *slot = None;
            }
        }
        for (index, slot) in self.functions.iter_mut().enumerate() {
            if slot.is_some() && !marks.functions.contains(&FunctionId(index as u32)) {
                *slot = None;
            }
        }
        self.collection_count = self.collection_count.saturating_add(1);
        self.allocations_since_collection = 0;
        self.estimated_bytes = self.recalculate_estimated_bytes();
        let after = self.stats();
        CollectionStats {
            objects_before: before.live_objects,
            objects_after: after.live_objects,
            environments_before: before.live_environments,
            environments_after: after.live_environments,
            functions_before: before.live_functions,
            functions_after: after.live_functions,
            bytes_before: before.estimated_bytes,
            bytes_after: after.estimated_bytes,
        }
    }

    fn live_objects(&self) -> usize {
        self.objects.iter().flatten().count()
    }

    fn live_environments(&self) -> usize {
        self.environments.iter().flatten().count()
    }

    fn live_functions(&self) -> usize {
        self.functions.iter().flatten().count()
    }

    fn recalculate_estimated_bytes(&self) -> usize {
        self.objects
            .iter()
            .flatten()
            .map(JsObject::estimated_bytes)
            .chain(
                self.environments
                    .iter()
                    .flatten()
                    .map(Environment::estimated_bytes),
            )
            .chain(
                self.functions
                    .iter()
                    .flatten()
                    .map(JsFunction::estimated_bytes),
            )
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::{Heap, JsObject, JsValue, PropertyDescriptor};

    #[test]
    fn allocates_and_reads_objects() {
        let mut heap = Heap::default();
        let mut object = JsObject::default();
        object.define_property("answer", PropertyDescriptor::data(JsValue::Number(42.0)));
        let id = heap.allocate_object(object).unwrap();

        assert_eq!(
            heap.object(id)
                .unwrap()
                .own_property("answer")
                .unwrap()
                .value_cloned(),
            Some(JsValue::Number(42.0))
        );
    }
}
