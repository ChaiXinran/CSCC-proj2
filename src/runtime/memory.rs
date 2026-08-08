#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeMemoryStats {
    pub heap_estimated_bytes: usize,
    pub heap_object_slots: usize,
    pub heap_live_objects: usize,
    pub heap_live_environments: usize,
    pub heap_live_functions: usize,
    pub object_arena_capacity_bytes: usize,
    pub environment_arena_capacity_bytes: usize,
    pub function_arena_capacity_bytes: usize,
    pub promise_records: usize,
    pub promise_capacity: usize,
    pub promise_reaction_capacity: usize,
    pub job_queue_len: usize,
    pub job_queue_capacity: usize,
    pub array_buffer_records: usize,
    pub array_buffer_capacity: usize,
    pub array_buffer_payload_bytes: usize,
    pub typed_array_views: usize,
    pub typed_array_view_capacity: usize,
    pub data_views: usize,
    pub data_view_capacity: usize,
    pub private_slot_entries: usize,
    pub function_object_entries: usize,
    pub object_value_entries: usize,
    pub module_records: usize,
    pub realm_records: usize,
    pub shape_count: usize,
    pub property_ic_entries: usize,
    pub regexp_cache_entries: usize,
    pub tracked_runtime_bytes: usize,
    pub charged_bytes_since_gc: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryClass {
    HeapObject,
    HeapEnvironment,
    HeapFunction,
    ObjectMutation,
    PromiseRegistry,
    JobQueue,
    ArrayBuffer,
    TypedArrayMetadata,
    DataViewMetadata,
    ModuleRegistry,
    RealmRegistry,
    RuntimeCache,
    OtherNative,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AllocationPressure {
    pub allocations_since_gc: usize,
    pub charged_bytes_since_gc: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GcPolicy {
    pub min_allocations: usize,
    pub min_pressure_bytes: usize,
    pub growth_factor_num: usize,
    pub growth_factor_den: usize,
    pub max_allocations: usize,
    pub min_reclaim_percent: u8,
}

impl Default for GcPolicy {
    fn default() -> Self {
        Self {
            min_allocations: 20_000,
            min_pressure_bytes: 16 * 1024 * 1024,
            growth_factor_num: 3,
            growth_factor_den: 2,
            max_allocations: 250_000,
            min_reclaim_percent: 10,
        }
    }
}

impl GcPolicy {
    pub(crate) fn from_legacy_threshold(threshold: usize) -> Self {
        let threshold = threshold.max(1);
        let adaptive = Self::default();
        if threshold <= adaptive.max_allocations {
            return Self {
                min_allocations: threshold,
                min_pressure_bytes: usize::MAX,
                growth_factor_num: usize::MAX,
                growth_factor_den: 1,
                max_allocations: threshold,
                min_reclaim_percent: adaptive.min_reclaim_percent,
            };
        }
        Self {
            max_allocations: adaptive.max_allocations.min(threshold),
            ..adaptive
        }
    }

    pub(crate) fn normalized(self) -> Self {
        let max_allocations = self.max_allocations.max(1);
        Self {
            min_allocations: self.min_allocations.min(max_allocations),
            min_pressure_bytes: self.min_pressure_bytes.max(1),
            growth_factor_num: self.growth_factor_num.max(1),
            growth_factor_den: self.growth_factor_den.max(1),
            max_allocations,
            min_reclaim_percent: self.min_reclaim_percent.min(100),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcControllerState {
    pub last_live_bytes: usize,
    pub last_tracked_runtime_bytes: usize,
    pub allocations_since_gc: usize,
    pub pressure_bytes_since_gc: usize,
    pub last_reclaim_percent: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GcTriggerReason {
    #[default]
    Manual,
    Allocation,
    Bytes,
    Growth,
}
