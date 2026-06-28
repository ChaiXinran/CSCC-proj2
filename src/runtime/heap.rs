//! Object and environment allocation.

use super::gc::{CollectionStats, HeapMarks, HeapStats};
use super::{Environment, EnvironmentId, FunctionId, JsFunction, JsObject, ObjectId};

/// Arena owning native runtime objects and lexical environments.
#[derive(Debug)]
pub struct Heap {
    objects: Vec<Option<JsObject>>,
    /// Vacant indices in `objects`. Keeping this explicit avoids scanning the
    /// arena on every allocation, including the common no-vacancy case.
    free_objects: Vec<u32>,
    environments: Vec<Option<Environment>>,
    free_environments: Vec<u32>,
    functions: Vec<Option<JsFunction>>,
    free_functions: Vec<u32>,
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
            free_objects: Vec::new(),
            environments: Vec::new(),
            free_environments: Vec::new(),
            functions: Vec::new(),
            free_functions: Vec::new(),
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
        if let Some(index) = self.free_objects.pop() {
            let slot = self
                .objects
                .get_mut(index as usize)
                .expect("free object index must belong to the object arena");
            debug_assert!(slot.is_none(), "free object slot must be vacant");
            *slot = Some(object);
            self.note_allocation(estimated_bytes);
            return Some(ObjectId(index));
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
        if let Some(index) = self.free_environments.pop() {
            let slot = self
                .environments
                .get_mut(index as usize)
                .expect("free environment index must belong to the environment arena");
            debug_assert!(slot.is_none(), "free environment slot must be vacant");
            *slot = Some(environment);
            self.note_allocation(estimated_bytes);
            return Some(EnvironmentId(index));
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
        if let Some(index) = self.free_functions.pop() {
            let slot = self
                .functions
                .get_mut(index as usize)
                .expect("free function index must belong to the function arena");
            debug_assert!(slot.is_none(), "free function slot must be vacant");
            *slot = Some(function);
            self.note_allocation(estimated_bytes);
            return Some(FunctionId(index));
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

    pub fn function_mut(&mut self, id: FunctionId) -> Option<&mut JsFunction> {
        self.functions.get_mut(id.0 as usize)?.as_mut()
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
                self.free_objects.push(index as u32);
            }
        }
        for (index, slot) in self.environments.iter_mut().enumerate() {
            if slot.is_some() && !marks.environments.contains(&EnvironmentId(index as u32)) {
                *slot = None;
                self.free_environments.push(index as u32);
            }
        }
        for (index, slot) in self.functions.iter_mut().enumerate() {
            if slot.is_some() && !marks.functions.contains(&FunctionId(index as u32)) {
                *slot = None;
                self.free_functions.push(index as u32);
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
        debug_assert!(self.free_objects.len() <= self.objects.len());
        self.objects.len() - self.free_objects.len()
    }

    fn live_environments(&self) -> usize {
        debug_assert!(self.free_environments.len() <= self.environments.len());
        self.environments.len() - self.free_environments.len()
    }

    fn live_functions(&self) -> usize {
        debug_assert!(self.free_functions.len() <= self.functions.len());
        self.functions.len() - self.free_functions.len()
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
    use crate::{
        bytecode::Chunk,
        runtime::{Environment, Heap, JsFunction, JsObject, JsValue, PropertyDescriptor},
    };

    use super::HeapMarks;

    fn empty_function() -> JsFunction {
        JsFunction {
            name: None,
            params: Vec::new(),
            rest_param: None,
            length_override: None,
            chunk: Chunk::default(),
            environment: None,
            is_async: false,
            is_generator: false,
            is_arrow: false,
            uses_arguments: false,
            lexical_this: None,
            lexical_new_target: None,
            home_object: None,
        }
    }

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

    #[test]
    fn sweep_recycles_all_arena_slot_kinds() {
        let mut heap = Heap::default();
        let object = heap.allocate_object(JsObject::ordinary()).unwrap();
        let environment = heap.allocate_environment(Environment::default()).unwrap();
        let function = heap.allocate_function(empty_function()).unwrap();

        let collected = heap.sweep(&HeapMarks::default());
        assert_eq!(collected.objects_after, 0);
        assert_eq!(collected.environments_after, 0);
        assert_eq!(collected.functions_after, 0);

        assert_eq!(heap.allocate_object(JsObject::ordinary()).unwrap(), object);
        assert_eq!(
            heap.allocate_environment(Environment::default()).unwrap(),
            environment
        );
        assert_eq!(heap.allocate_function(empty_function()).unwrap(), function);
        assert_eq!(heap.total_live_count(), 3);
    }

    #[test]
    fn repeated_sweeps_do_not_duplicate_free_slots() {
        let mut heap = Heap::default();
        let object = heap.allocate_object(JsObject::ordinary()).unwrap();

        heap.sweep(&HeapMarks::default());
        heap.sweep(&HeapMarks::default());

        assert_eq!(heap.free_objects, vec![object.0]);
        assert_eq!(heap.allocate_object(JsObject::ordinary()).unwrap(), object);
        assert!(heap.free_objects.is_empty());
        assert_eq!(heap.object_count(), 1);
    }

    #[test]
    fn live_slots_are_not_added_to_free_lists() {
        let mut heap = Heap::default();
        let object = heap.allocate_object(JsObject::ordinary()).unwrap();
        let environment = heap.allocate_environment(Environment::default()).unwrap();
        let function = heap.allocate_function(empty_function()).unwrap();
        let mut marks = HeapMarks::default();
        marks.objects.insert(object);
        marks.environments.insert(environment);
        marks.functions.insert(function);

        heap.sweep(&marks);

        assert!(heap.free_objects.is_empty());
        assert!(heap.free_environments.is_empty());
        assert!(heap.free_functions.is_empty());
        assert_eq!(heap.total_live_count(), 3);
    }

    #[test]
    fn live_allocation_limit_allows_reusing_a_collected_slot() {
        let mut heap = Heap::with_limit(1);
        let first = heap.allocate_object(JsObject::ordinary()).unwrap();
        assert!(heap.allocate_object(JsObject::ordinary()).is_none());

        heap.sweep(&HeapMarks::default());

        assert_eq!(heap.allocate_object(JsObject::ordinary()).unwrap(), first);
        assert!(heap.allocate_object(JsObject::ordinary()).is_none());
    }
}
