//! Garbage-collection boundary.

use super::{
    EnvironmentId, FunctionId, GcControllerState, GcTriggerReason, Heap, JsValue, ObjectId,
};
use crate::vm::CallFrame;

/// Heap statistics exposed to runtime tests and benchmark/reporting code.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeapStats {
    pub object_slots: usize,
    pub live_objects: usize,
    pub live_environments: usize,
    pub live_functions: usize,
    pub estimated_bytes: usize,
    pub allocation_count: u64,
    pub collection_count: u64,
}

/// Statistics returned by one collection pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CollectionStats {
    pub objects_before: usize,
    pub objects_after: usize,
    pub environments_before: usize,
    pub environments_after: usize,
    pub functions_before: usize,
    pub functions_after: usize,
    pub bytes_before: usize,
    pub bytes_after: usize,
}

/// Cumulative garbage-collection timing and last-pass statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GcMetrics {
    pub collection_count: u64,
    pub total_pause_ns: u64,
    pub max_pause_ns: u64,
    pub last_collection: CollectionStats,
    pub last_trigger_reason: GcTriggerReason,
    pub controller: GcControllerState,
}

pub(crate) trait RootSink {
    fn mark_object_root(&mut self, id: ObjectId);
    fn mark_environment_root(&mut self, id: EnvironmentId);
    fn mark_function_root(&mut self, id: FunctionId);
    fn mark_value_root(&mut self, value: &JsValue);
}

#[derive(Debug, Default)]
pub(crate) struct HeapMarks {
    objects: Vec<bool>,
    environments: Vec<bool>,
    functions: Vec<bool>,
}

impl HeapMarks {
    pub(crate) fn for_heap(heap: &Heap) -> Self {
        Self {
            objects: vec![false; heap.object_slots()],
            environments: vec![false; heap.environment_slots()],
            functions: vec![false; heap.function_slots()],
        }
    }

    pub(crate) fn prepare_for_heap(&mut self, heap: &Heap) {
        reset_marks(&mut self.objects, heap.object_slots());
        reset_marks(&mut self.environments, heap.environment_slots());
        reset_marks(&mut self.functions, heap.function_slots());
    }

    pub(crate) fn mark_object(&mut self, id: ObjectId) -> bool {
        mark_slot(&mut self.objects, id.0 as usize)
    }

    pub(crate) fn mark_environment(&mut self, id: EnvironmentId) -> bool {
        mark_slot(&mut self.environments, id.0 as usize)
    }

    pub(crate) fn mark_function(&mut self, id: FunctionId) -> bool {
        mark_slot(&mut self.functions, id.0 as usize)
    }

    pub(crate) fn contains_object(&self, id: ObjectId) -> bool {
        self.objects.get(id.0 as usize).copied().unwrap_or(false)
    }

    pub(crate) fn contains_environment(&self, id: EnvironmentId) -> bool {
        self.environments
            .get(id.0 as usize)
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn contains_function(&self, id: FunctionId) -> bool {
        self.functions.get(id.0 as usize).copied().unwrap_or(false)
    }
}

fn reset_marks(marks: &mut Vec<bool>, slots: usize) {
    marks.resize(slots, false);
    marks.fill(false);
}

fn mark_slot(slots: &mut [bool], index: usize) -> bool {
    let Some(marked) = slots.get_mut(index) else {
        return false;
    };
    if *marked {
        return false;
    }
    *marked = true;
    true
}

/// Explicit roots supplied by NativeContext and the VM before a collection.
#[derive(Debug, Clone, PartialEq)]
pub struct RootSet {
    pub global_environment: EnvironmentId,
    pub current_environment: EnvironmentId,
    pub environment_stack: Vec<EnvironmentId>,
    pub call_frames: Vec<CallFrameRoots>,
    pub operand_stack: Vec<JsValue>,
    pub pending_exception: Option<JsValue>,
    /// Internal native roots such as intrinsics and builtin backing objects.
    pub object_roots: Vec<ObjectId>,
    pub function_roots: Vec<FunctionId>,
    pub value_roots: Vec<JsValue>,
}

impl RootSet {
    #[must_use]
    pub fn new(global_environment: EnvironmentId, current_environment: EnvironmentId) -> Self {
        Self {
            global_environment,
            current_environment,
            environment_stack: Vec::new(),
            call_frames: Vec::new(),
            operand_stack: Vec::new(),
            pending_exception: None,
            object_roots: Vec::new(),
            function_roots: Vec::new(),
            value_roots: Vec::new(),
        }
    }

    pub(crate) fn trace(&self, tracer: &mut Tracer<'_>) {
        tracer.mark_environment(self.global_environment);
        tracer.mark_environment(self.current_environment);
        for environment in &self.environment_stack {
            tracer.mark_environment(*environment);
        }
        for frame in &self.call_frames {
            frame.trace(tracer);
        }
        for value in &self.operand_stack {
            value.trace(tracer);
        }
        if let Some(value) = &self.pending_exception {
            value.trace(tracer);
        }
        for object in &self.object_roots {
            tracer.mark_object(*object);
        }
        for function in &self.function_roots {
            tracer.mark_function(*function);
        }
        for value in &self.value_roots {
            value.trace(tracer);
        }
    }
}

impl RootSink for RootSet {
    fn mark_object_root(&mut self, id: ObjectId) {
        if !self.object_roots.contains(&id) {
            self.object_roots.push(id);
        }
    }

    fn mark_environment_root(&mut self, id: EnvironmentId) {
        if !self.environment_stack.contains(&id)
            && id != self.global_environment
            && id != self.current_environment
        {
            self.environment_stack.push(id);
        }
    }

    fn mark_function_root(&mut self, id: FunctionId) {
        if !self.function_roots.contains(&id) {
            self.function_roots.push(id);
        }
    }

    fn mark_value_root(&mut self, value: &JsValue) {
        if !self.value_roots.iter().any(|root| root == value) {
            self.value_roots.push(value.clone());
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CallFrameRoots {
    pub function: Option<FunctionId>,
    pub this_value: JsValue,
    pub new_target: JsValue,
    pub environment: EnvironmentId,
    pub stack_base: usize,
}

impl From<&CallFrame> for CallFrameRoots {
    fn from(frame: &CallFrame) -> Self {
        Self {
            function: frame.function,
            this_value: frame.this_value.clone(),
            new_target: frame.new_target.clone(),
            environment: frame.environment,
            stack_base: frame.stack_base,
        }
    }
}

impl Trace for CallFrameRoots {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(function) = self.function {
            tracer.mark_function(function);
        }
        self.this_value.trace(tracer);
        self.new_target.trace(tracer);
        tracer.mark_environment(self.environment);
    }
}

pub trait Trace {
    fn trace(&self, tracer: &mut Tracer<'_>);
}

pub struct Tracer<'a> {
    pub heap: &'a Heap,
    marks: HeapMarks,
}

impl<'a> Tracer<'a> {
    #[must_use]
    pub fn new(heap: &'a Heap) -> Self {
        Self {
            heap,
            marks: HeapMarks::for_heap(heap),
        }
    }

    pub(crate) fn with_marks(heap: &'a Heap, mut marks: HeapMarks) -> Self {
        marks.prepare_for_heap(heap);
        Self { heap, marks }
    }

    pub fn mark_object(&mut self, id: ObjectId) {
        if !self.marks.mark_object(id) {
            return;
        }
        if let Some(object) = self.heap.object(id) {
            object.trace(self);
        }
    }

    pub fn mark_environment(&mut self, id: EnvironmentId) {
        if !self.marks.mark_environment(id) {
            return;
        }
        if let Some(environment) = self.heap.environment(id) {
            environment.trace(self);
        }
    }

    pub fn mark_function(&mut self, id: FunctionId) {
        if !self.marks.mark_function(id) {
            return;
        }
        if let Some(function) = self.heap.function(id) {
            function.trace(self);
        }
    }

    pub(crate) fn into_marks(self) -> HeapMarks {
        self.marks
    }

    pub(crate) fn is_object_marked(&self, id: ObjectId) -> bool {
        self.marks.contains_object(id)
    }

    pub(crate) fn marked_object_count(&self) -> usize {
        self.marks.objects.iter().filter(|marked| **marked).count()
    }
}

impl RootSink for Tracer<'_> {
    fn mark_object_root(&mut self, id: ObjectId) {
        self.mark_object(id);
    }

    fn mark_environment_root(&mut self, id: EnvironmentId) {
        self.mark_environment(id);
    }

    fn mark_function_root(&mut self, id: FunctionId) {
        self.mark_function(id);
    }

    fn mark_value_root(&mut self, value: &JsValue) {
        value.trace(self);
    }
}

/// Non-moving mark-and-sweep collector.
#[derive(Debug, Default)]
pub struct Collector;

impl Collector {
    pub fn collect(&mut self, heap: &mut Heap, roots: &RootSet) -> CollectionStats {
        let marks = {
            let mut tracer = Tracer::new(heap);
            roots.trace(&mut tracer);
            tracer.into_marks()
        };
        heap.sweep(&marks)
    }
}
