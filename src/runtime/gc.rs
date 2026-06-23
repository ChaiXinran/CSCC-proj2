//! Garbage-collection boundary.

use std::collections::HashSet;

use super::{EnvironmentId, FunctionId, Heap, JsValue, ObjectId};
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

#[derive(Debug, Default)]
pub(crate) struct HeapMarks {
    pub objects: HashSet<ObjectId>,
    pub environments: HashSet<EnvironmentId>,
    pub functions: HashSet<FunctionId>,
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

    fn trace(&self, tracer: &mut Tracer<'_>) {
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

#[derive(Debug, Clone, PartialEq)]
pub struct CallFrameRoots {
    pub function: Option<FunctionId>,
    pub this_value: JsValue,
    pub environment: EnvironmentId,
    pub stack_base: usize,
}

impl From<&CallFrame> for CallFrameRoots {
    fn from(frame: &CallFrame) -> Self {
        Self {
            function: frame.function,
            this_value: frame.this_value.clone(),
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
            marks: HeapMarks::default(),
        }
    }

    pub fn mark_object(&mut self, id: ObjectId) {
        if !self.marks.objects.insert(id) {
            return;
        }
        if let Some(object) = self.heap.object(id) {
            object.trace(self);
        }
    }

    pub fn mark_environment(&mut self, id: EnvironmentId) {
        if !self.marks.environments.insert(id) {
            return;
        }
        if let Some(environment) = self.heap.environment(id) {
            environment.trace(self);
        }
    }

    pub fn mark_function(&mut self, id: FunctionId) {
        if !self.marks.functions.insert(id) {
            return;
        }
        if let Some(function) = self.heap.function(id) {
            function.trace(self);
        }
    }

    fn into_marks(self) -> HeapMarks {
        self.marks
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
