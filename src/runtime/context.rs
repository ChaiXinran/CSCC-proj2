//! Persistent state shared by native execution and integration.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use super::{
    ArrayBufferId, ArrayBufferRecord, BoundFunction, BuiltinFunction, BuiltinId, CollectionStats,
    Collector, DataViewId, DataViewRecord, Environment, EnvironmentId, FunctionId, Heap, HeapStats,
    IteratorRecord, Job, JobQueue, JsFunction, JsObject, JsValue, NativeCall, NativeConstruct,
    NativeErrorKind, NativeJob, ObjectId, ObjectKind, PrimitiveValue, PromiseId, PromiseJob,
    PromiseReaction, PromiseRecord, PromiseState, PropertyDescriptor, PropertyDescriptorUpdate,
    PropertyKind, RootSet, SymbolId, SymbolRegistry, TypedArrayElementKind, TypedArrayView,
    TypedArrayViewId, WellKnownSymbols, iterator::IteratorKind, object::array_index,
};
use crate::vm::{CallFrame, Vm, VmError};

/// Stable references to all fundamental constructors and prototypes installed during
/// `install_foundation`. The V6 additions (string/number/boolean/error prototypes) are
/// pre-created as ordinary objects in `install_foundation` so that builtins can install
/// methods on them without needing a second intrinsics update.
#[derive(Debug, Clone, PartialEq)]
pub struct Intrinsics {
    pub object_prototype: ObjectId,
    pub function_prototype: ObjectId,
    pub array_prototype: ObjectId,
    pub object_constructor: JsValue,
    pub function_constructor: JsValue,
    pub array_constructor: JsValue,
    // V6: primitive wrapper prototypes
    pub string_prototype: ObjectId,
    pub number_prototype: ObjectId,
    pub boolean_prototype: ObjectId,
    pub error_prototype: ObjectId,
    pub regexp_prototype: ObjectId,
}

const PROTOTYPE_CHAIN_LIMIT: usize = 1024;
pub const MAX_ARRAY_LENGTH: usize = 1_000_000;
pub const MAX_ARRAY_BUFFER_BYTE_LENGTH: usize = 1 << 26;
const MAX_UTF16_ALLOCATION_UNITS: usize = 1 << 23;
/// Cooperative per-evaluation execution limits shared by VM and builtins.
#[derive(Debug, Clone)]
pub struct ExecutionBudget {
    pub loop_remaining: u64,
    pub call_depth_limit: u64,
    pub stack_limit: usize,
    pub deadline: Option<Instant>,
}

impl Default for ExecutionBudget {
    fn default() -> Self {
        Self {
            loop_remaining: u64::MAX,
            call_depth_limit: u64::MAX,
            stack_limit: usize::MAX,
            deadline: None,
        }
    }
}

impl ExecutionBudget {
    pub fn check_loop(&mut self) -> Result<(), VmError> {
        self.check_deadline()?;
        if self.loop_remaining == 0 {
            return Err(VmError::runtime_limit("loop iteration limit exceeded"));
        }
        self.loop_remaining -= 1;
        Ok(())
    }

    pub fn check_call_depth(&self, depth: u64) -> Result<(), VmError> {
        self.check_deadline()?;
        if depth >= self.call_depth_limit {
            return Err(VmError::runtime_limit("call stack limit exceeded"));
        }
        Ok(())
    }

    pub fn check_stack_depth(&self, depth: usize) -> Result<(), VmError> {
        self.check_deadline()?;
        if depth > self.stack_limit {
            return Err(VmError::runtime_limit("operand stack limit exceeded"));
        }
        Ok(())
    }

    pub fn check_deadline(&self) -> Result<(), VmError> {
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            return Err(VmError::runtime_limit("wall-clock deadline exceeded"));
        }
        Ok(())
    }
}

/// Per-isolate language state passed to the bytecode executor.
#[derive(Debug)]
pub struct NativeContext {
    heap: Heap,
    global_environment: EnvironmentId,
    global_object: ObjectId,
    current_environment: EnvironmentId,
    environment_stack: Vec<EnvironmentId>,
    call_frames: Vec<CallFrame>,
    function_prototypes: HashMap<FunctionId, ObjectId>,
    function_objects: HashMap<FunctionId, ObjectId>,
    strict_functions: HashSet<FunctionId>,
    object_values: HashMap<ObjectId, JsValue>,
    error_objects: HashSet<ObjectId>,
    raw_json_objects: HashMap<ObjectId, String>,
    builtin_registry: Vec<BuiltinFunction>,
    intrinsics: Option<Intrinsics>,
    function_prototype_call: Option<BuiltinId>,
    function_prototype_apply: Option<BuiltinId>,
    symbol_registry: SymbolRegistry,
    symbol_for_registry: HashMap<String, SymbolId>,
    job_queue: JobQueue,
    promises: Vec<PromiseRecord>,
    array_buffers: Vec<ArrayBufferRecord>,
    typed_array_views: Vec<TypedArrayView>,
    data_views: Vec<DataViewRecord>,
    strict: bool,
    top_level_this: JsValue,
    output: Vec<String>,
    budget: ExecutionBudget,
    call_depth: u64,
    gc_allocation_threshold: usize,
}

impl Default for NativeContext {
    fn default() -> Self {
        Self::build(Heap::default())
    }
}

impl NativeContext {
    pub fn with_heap_limit(limit: usize) -> Self {
        Self::build(Heap::with_limit(limit))
    }

    pub fn with_heap_limits(object_limit: usize, byte_limit: usize) -> Self {
        Self::build(Heap::with_limits(object_limit, byte_limit))
    }

    fn build(mut heap: Heap) -> Self {
        let global_environment = heap
            .allocate_environment(Environment::default())
            .expect("a fresh heap can allocate the global environment");
        let global_object = heap
            .allocate_object(JsObject::ordinary())
            .expect("a fresh heap can allocate the global object");

        let mut context = Self {
            heap,
            global_environment,
            global_object,
            current_environment: global_environment,
            environment_stack: Vec::new(),
            call_frames: Vec::new(),
            function_prototypes: HashMap::new(),
            function_objects: HashMap::new(),
            strict_functions: HashSet::new(),
            object_values: HashMap::new(),
            error_objects: HashSet::new(),
            raw_json_objects: HashMap::new(),
            builtin_registry: Vec::new(),
            intrinsics: None,
            function_prototype_call: None,
            function_prototype_apply: None,
            symbol_registry: SymbolRegistry::new(),
            symbol_for_registry: HashMap::new(),
            job_queue: JobQueue::default(),
            promises: Vec::new(),
            array_buffers: Vec::new(),
            typed_array_views: Vec::new(),
            data_views: Vec::new(),
            strict: false,
            top_level_this: JsValue::Object(global_object),
            output: Vec::new(),
            budget: ExecutionBudget::default(),
            call_depth: 0,
            gc_allocation_threshold: 10_000,
        };
        context.declare_global("undefined", JsValue::Undefined);
        context.declare_global("NaN", JsValue::Number(f64::NAN));
        context.declare_global("Infinity", JsValue::Number(f64::INFINITY));
        context
    }
}

impl NativeContext {
    #[must_use]
    pub fn heap(&self) -> &Heap {
        &self.heap
    }

    pub fn heap_mut(&mut self) -> &mut Heap {
        &mut self.heap
    }

    pub fn configure_heap_limits(
        &mut self,
        heap_byte_limit: usize,
        gc_allocation_threshold: usize,
    ) {
        self.heap.set_byte_limit(heap_byte_limit);
        self.gc_allocation_threshold = gc_allocation_threshold;
    }

    #[must_use]
    pub fn heap_stats(&self) -> HeapStats {
        self.heap.stats()
    }

    pub fn ensure_heap_capacity(&mut self, additional_bytes: usize) -> Result<(), VmError> {
        if self.heap.charge_bytes(additional_bytes) {
            Ok(())
        } else {
            Err(VmError::runtime_limit("heap byte limit exceeded"))
        }
    }

    #[must_use]
    pub fn should_collect_garbage(&self) -> bool {
        self.heap.should_collect(self.gc_allocation_threshold)
    }

    pub fn maybe_collect_garbage(&mut self, roots: &RootSet) -> Result<CollectionStats, VmError> {
        let roots = self.complete_root_set(roots);
        let mut collector = Collector;
        let stats = collector.collect(&mut self.heap, &roots);
        self.prune_swept_metadata();
        Ok(stats)
    }

    pub fn collect_garbage_for_vm(&mut self, vm: &Vm) -> Result<CollectionStats, VmError> {
        let roots = self.root_set(vm);
        self.maybe_collect_garbage(&roots)
    }

    #[must_use]
    pub fn root_set(&self, vm: &Vm) -> RootSet {
        let mut roots = RootSet::new(self.global_environment, self.current_environment);
        roots.environment_stack = self.environment_stack.clone();
        roots.call_frames = self.call_frames.iter().map(Into::into).collect();
        roots.operand_stack = vm.operand_stack_roots();
        roots.pending_exception = vm.pending_exception_root();
        self.add_internal_roots(&mut roots);
        roots
    }

    fn complete_root_set(&self, roots: &RootSet) -> RootSet {
        let mut roots = roots.clone();
        self.add_internal_roots(&mut roots);
        roots
    }

    fn add_internal_roots(&self, roots: &mut RootSet) {
        roots.object_roots.push(self.global_object);
        roots.value_roots.push(self.top_level_this.clone());
        if let Some(intrinsics) = &self.intrinsics {
            roots.object_roots.extend([
                intrinsics.object_prototype,
                intrinsics.function_prototype,
                intrinsics.array_prototype,
                intrinsics.string_prototype,
                intrinsics.number_prototype,
                intrinsics.boolean_prototype,
                intrinsics.error_prototype,
                intrinsics.regexp_prototype,
            ]);
            roots.value_roots.extend([
                intrinsics.object_constructor.clone(),
                intrinsics.function_constructor.clone(),
                intrinsics.array_constructor.clone(),
            ]);
        }
        roots
            .object_roots
            .extend(self.function_prototypes.values().copied());
        roots
            .object_roots
            .extend(self.function_objects.values().copied());
        for builtin in &self.builtin_registry {
            roots.object_roots.push(builtin.object);
            if let Some(bound) = &builtin.bound {
                roots.value_roots.push(bound.target.clone());
                roots.value_roots.push(bound.this_value.clone());
                roots.value_roots.extend(bound.args.iter().cloned());
            }
        }
        for promise in &self.promises {
            match &promise.state {
                PromiseState::Fulfilled(value) | PromiseState::Rejected(value) => {
                    roots.value_roots.push(value.clone());
                }
                PromiseState::Pending => {}
            }
        }
        for job in self.job_queue.iter() {
            if let Job::PromiseReaction(job) = job {
                roots.value_roots.push(job.value.clone());
            }
        }
    }

    fn prune_swept_metadata(&mut self) {
        self.function_prototypes.retain(|function, object| {
            self.heap.contains_function(*function) && self.heap.contains_object(*object)
        });
        self.function_objects.retain(|function, object| {
            self.heap.contains_function(*function) && self.heap.contains_object(*object)
        });
        self.strict_functions
            .retain(|function| self.heap.contains_function(*function));
        self.object_values.retain(|object, value| {
            self.heap.contains_object(*object) && value_references_live_heap(value, &self.heap)
        });
        self.error_objects
            .retain(|object| self.heap.contains_object(*object));
        self.raw_json_objects
            .retain(|object, _| self.heap.contains_object(*object));
    }

    /// Register a builtin function and return `JsValue::BuiltinFunction(id)`.
    /// Creates a backing heap object with `name` and `length` properties.
    pub fn register_builtin(
        &mut self,
        name: &'static str,
        length: u8,
        call: NativeCall,
        construct: Option<NativeConstruct>,
    ) -> Result<JsValue, crate::vm::VmError> {
        let mut object = JsObject::ordinary();
        // Set Function.prototype as the prototype so .call/.apply/.bind are accessible
        object.prototype = self.function_prototype_object();
        object.define_property(
            "name",
            PropertyDescriptor::data_with(JsValue::String(name.into()), false, false, true),
        );
        object.define_property(
            "length",
            PropertyDescriptor::data_with(JsValue::Number(f64::from(length)), false, false, true),
        );
        let object_id = self
            .heap
            .allocate_object(object)
            .ok_or_else(|| crate::vm::VmError::runtime_limit("object arena exhausted"))?;
        let idx = self.builtin_registry.len();
        let id = BuiltinId(
            u16::try_from(idx).map_err(|_| crate::vm::VmError::runtime("builtin registry full"))?,
        );
        self.builtin_registry.push(BuiltinFunction {
            name,
            length,
            call,
            construct,
            object: object_id,
            bound: None,
        });
        Ok(JsValue::BuiltinFunction(id))
    }

    /// Registers a bound function produced by `Function.prototype.bind`. The
    /// returned value is callable/constructable; the VM forwards invocations to
    /// `target` with `this_value` and `args` prepended.
    pub fn register_bound_function(
        &mut self,
        target: JsValue,
        this_value: JsValue,
        args: Vec<JsValue>,
        length: u8,
    ) -> Result<JsValue, VmError> {
        let mut object = JsObject::ordinary();
        object.prototype = self.function_prototype_object();
        object.define_property(
            "name",
            PropertyDescriptor::data_with(JsValue::String("bound".into()), false, false, true),
        );
        object.define_property(
            "length",
            PropertyDescriptor::data_with(JsValue::Number(f64::from(length)), false, false, true),
        );
        let object_id = self
            .heap
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
        let idx = self.builtin_registry.len();
        let id =
            BuiltinId(u16::try_from(idx).map_err(|_| VmError::runtime("builtin registry full"))?);
        self.builtin_registry.push(BuiltinFunction {
            name: "bound",
            length,
            // Never invoked directly: the VM dispatches bound functions by
            // forwarding to the target. These are unreachable fallbacks.
            call: bound_call_unreachable,
            construct: Some(bound_construct_unreachable),
            object: object_id,
            bound: Some(BoundFunction {
                target,
                this_value,
                args,
            }),
        });
        Ok(JsValue::BuiltinFunction(id))
    }

    #[must_use]
    pub fn builtin(&self, id: BuiltinId) -> Option<&BuiltinFunction> {
        self.builtin_registry.get(id.0 as usize)
    }

    /// Find a registered builtin by name and return it as a `JsValue::BuiltinFunction`.
    #[must_use]
    pub fn find_builtin_by_name(&self, name: &str) -> Option<JsValue> {
        self.builtin_registry
            .iter()
            .enumerate()
            .find(|(_, bf)| bf.name == name)
            .map(|(i, _)| JsValue::BuiltinFunction(BuiltinId(i as u16)))
    }

    pub fn set_function_prototype_call(&mut self, id: BuiltinId) {
        self.function_prototype_call = Some(id);
    }

    #[must_use]
    pub fn is_function_prototype_call(&self, id: BuiltinId) -> bool {
        self.function_prototype_call == Some(id)
    }

    pub fn set_function_prototype_apply(&mut self, id: BuiltinId) {
        self.function_prototype_apply = Some(id);
    }

    #[must_use]
    pub fn is_function_prototype_apply(&self, id: BuiltinId) -> bool {
        self.function_prototype_apply == Some(id)
    }

    #[must_use]
    pub fn intrinsics(&self) -> Option<&Intrinsics> {
        self.intrinsics.as_ref()
    }

    pub fn set_intrinsics(&mut self, intrinsics: Intrinsics) {
        self.intrinsics = Some(intrinsics);
    }

    // ── Symbol registry ──────────────────────────────────────────────────────

    #[must_use]
    pub fn symbols(&self) -> &SymbolRegistry {
        &self.symbol_registry
    }

    pub fn symbols_mut(&mut self) -> &mut SymbolRegistry {
        &mut self.symbol_registry
    }

    #[must_use]
    pub fn well_known_symbols(&self) -> &WellKnownSymbols {
        &self.symbol_registry.well_known
    }

    /// Allocate a new user Symbol and return `JsValue::Symbol(id)`.
    pub fn create_symbol(&mut self, description: Option<String>) -> JsValue {
        JsValue::Symbol(self.symbol_registry.create(description))
    }

    /// `Symbol.for(key)` — return the same symbol for the same key string.
    pub fn symbol_for(&mut self, key: String) -> JsValue {
        if let Some(&id) = self.symbol_for_registry.get(&key) {
            return JsValue::Symbol(id);
        }
        let id = self.symbol_registry.create(Some(key.clone()));
        self.symbol_for_registry.insert(key, id);
        JsValue::Symbol(id)
    }

    /// `Symbol.keyFor(sym)` — return the key if `sym` was created via `Symbol.for`.
    #[must_use]
    pub fn symbol_key_for(&self, id: SymbolId) -> Option<&str> {
        self.symbol_for_registry
            .iter()
            .find(|&(_, &v)| v == id)
            .map(|(k, _)| k.as_str())
    }

    /// Define a symbol-keyed own property on an object.
    pub fn define_symbol_own_property(
        &mut self,
        object: ObjectId,
        symbol: SymbolId,
        descriptor: PropertyDescriptor,
    ) -> Result<bool, crate::vm::VmError> {
        let obj = self
            .heap
            .object_mut(object)
            .ok_or_else(|| crate::vm::VmError::runtime("missing object"))?;
        obj.define_symbol_property(symbol, descriptor);
        Ok(true)
    }

    pub fn validate_and_apply_symbol_property_descriptor(
        &mut self,
        object: ObjectId,
        symbol: SymbolId,
        update: PropertyDescriptorUpdate,
    ) -> Result<bool, VmError> {
        if descriptor_update_has_data(&update) && descriptor_update_has_accessor(&update) {
            return Err(VmError::type_error("invalid mixed property descriptor"));
        }
        let current = self.get_own_symbol_property_descriptor(object, symbol);
        let Some(descriptor) = validate_and_apply_descriptor_update(current, update) else {
            return Ok(false);
        };
        self.define_symbol_own_property(object, symbol, descriptor)
    }

    /// Walk the prototype chain and return the first data value for a symbol key.
    /// Accessor-keyed symbol properties return `None` (the caller can use the VM path).
    #[must_use]
    pub fn get_symbol_property_value(&self, object: ObjectId, symbol: SymbolId) -> Option<JsValue> {
        let mut current = Some(object);
        let mut depth = 0usize;
        while let Some(id) = current {
            if depth > 1024 {
                return None;
            }
            let obj = self.heap.object(id)?;
            if let Some(descriptor) = obj.own_symbol_property(symbol) {
                return match &descriptor.kind {
                    crate::runtime::PropertyKind::Data { value, .. } => Some(value.clone()),
                    crate::runtime::PropertyKind::Accessor { .. } => None,
                };
            }
            current = obj.prototype;
            depth += 1;
        }
        None
    }

    /// Return the own symbol property descriptor (does not walk prototype chain).
    #[must_use]
    pub fn get_own_symbol_property_descriptor(
        &self,
        object: ObjectId,
        symbol: SymbolId,
    ) -> Option<PropertyDescriptor> {
        self.heap
            .object(object)?
            .own_symbol_property(symbol)
            .cloned()
    }

    pub fn has_symbol_property(&self, object: ObjectId, symbol: SymbolId) -> Result<bool, VmError> {
        Ok(self
            .find_symbol_property_descriptor(object, symbol)?
            .is_some())
    }

    pub fn delete_symbol_property(
        &mut self,
        object: ObjectId,
        symbol: SymbolId,
        strict: bool,
    ) -> Result<bool, VmError> {
        if let Some(descriptor) = self.get_own_symbol_property_descriptor(object, symbol)
            && !descriptor.configurable
        {
            return strict_error_or_false(strict, "cannot delete non-configurable property");
        }

        let Some(object) = self.heap.object_mut(object) else {
            return Err(VmError::runtime("missing object"));
        };
        if object.own_symbol_property(symbol).is_none() {
            return Ok(true);
        }
        Ok(object.delete_own_symbol_property(symbol).is_some())
    }

    pub fn set_symbol_property(
        &mut self,
        object: ObjectId,
        symbol: SymbolId,
        value: JsValue,
        strict: bool,
    ) -> Result<bool, VmError> {
        if let Some(mut descriptor) = self.get_own_symbol_property_descriptor(object, symbol) {
            return match &mut descriptor.kind {
                PropertyKind::Data { writable, .. } if !*writable => {
                    strict_error_or_false(strict, "cannot write non-writable property")
                }
                PropertyKind::Data {
                    value: current_value,
                    ..
                } => {
                    *current_value = value;
                    self.define_symbol_own_property(object, symbol, descriptor)
                }
                PropertyKind::Accessor { set: None, .. } => {
                    strict_error_or_false(strict, "property setter is undefined")
                }
                PropertyKind::Accessor { set: Some(_), .. } => Err(VmError::type_error(
                    "accessor setter invocation requires the VM call path",
                )),
            };
        }

        if let Some(prototype) = self.get_prototype_of(object)
            && let Some((_, descriptor)) =
                self.find_symbol_property_descriptor(prototype, symbol)?
        {
            match descriptor.kind {
                PropertyKind::Data {
                    writable: false, ..
                } => {
                    return strict_error_or_false(
                        strict,
                        "cannot write inherited non-writable property",
                    );
                }
                PropertyKind::Accessor { set: None, .. } => {
                    return strict_error_or_false(strict, "inherited property setter is undefined");
                }
                PropertyKind::Accessor { set: Some(_), .. } => {
                    return Err(VmError::type_error(
                        "accessor setter invocation requires the VM call path",
                    ));
                }
                PropertyKind::Data { .. } => {}
            }
        }

        self.define_symbol_own_property(object, symbol, PropertyDescriptor::data(value))
    }

    pub fn register_function_object(&mut self, function: FunctionId, object: ObjectId) {
        self.function_objects.insert(function, object);
        self.object_values
            .insert(object, JsValue::Function(function));
    }

    pub fn mark_strict_function(&mut self, function: FunctionId) {
        self.strict_functions.insert(function);
    }

    #[must_use]
    pub fn is_strict_function(&self, function: FunctionId) -> bool {
        self.strict_functions.contains(&function)
    }

    #[must_use]
    pub fn function_object(&self, function: FunctionId) -> Option<ObjectId> {
        self.function_objects.get(&function).copied()
    }

    #[must_use]
    pub fn value_object(&self, value: &JsValue) -> Option<ObjectId> {
        match value {
            JsValue::Object(object) => Some(*object),
            JsValue::Function(function) => self.function_object(*function),
            JsValue::BuiltinFunction(id) => self.builtin(*id).map(|builtin| builtin.object),
            _ => None,
        }
    }

    pub fn require_object(&self, value: &JsValue, operation: &str) -> Result<ObjectId, VmError> {
        self.value_object(value)
            .ok_or_else(|| VmError::type_error(format!("cannot {operation} on {value}")))
    }

    pub fn mark_error_object(&mut self, object: ObjectId) {
        self.error_objects.insert(object);
    }

    #[must_use]
    pub fn is_error_object(&self, object: ObjectId) -> bool {
        self.error_objects.contains(&object)
    }

    pub fn mark_raw_json_object(&mut self, object: ObjectId, raw_json: String) {
        self.raw_json_objects.insert(object, raw_json);
    }

    #[must_use]
    pub fn raw_json_value(&self, object: ObjectId) -> Option<&str> {
        self.raw_json_objects
            .get(&object)
            .map(std::string::String::as_str)
    }

    #[must_use]
    pub fn object_value(&self, object: ObjectId) -> JsValue {
        if let Some((index, _)) = self
            .builtin_registry
            .iter()
            .enumerate()
            .find(|(_, builtin)| builtin.object == object)
        {
            return JsValue::BuiltinFunction(BuiltinId(index as u16));
        }
        self.object_values
            .get(&object)
            .cloned()
            .unwrap_or(JsValue::Object(object))
    }

    #[must_use]
    pub fn object_prototype(&self) -> Option<ObjectId> {
        self.intrinsics
            .as_ref()
            .map(|intrinsics| intrinsics.object_prototype)
    }

    #[must_use]
    pub fn function_prototype_object(&self) -> Option<ObjectId> {
        self.intrinsics
            .as_ref()
            .map(|intrinsics| intrinsics.function_prototype)
    }

    #[must_use]
    pub fn array_prototype(&self) -> Option<ObjectId> {
        self.intrinsics
            .as_ref()
            .map(|intrinsics| intrinsics.array_prototype)
    }

    #[must_use]
    pub fn regexp_prototype(&self) -> Option<ObjectId> {
        self.intrinsics
            .as_ref()
            .map(|intrinsics| intrinsics.regexp_prototype)
    }

    #[must_use]
    pub fn string_prototype(&self) -> Option<ObjectId> {
        self.intrinsics.as_ref().map(|i| i.string_prototype)
    }

    #[must_use]
    pub fn number_prototype(&self) -> Option<ObjectId> {
        self.intrinsics.as_ref().map(|i| i.number_prototype)
    }

    #[must_use]
    pub fn boolean_prototype(&self) -> Option<ObjectId> {
        self.intrinsics.as_ref().map(|i| i.boolean_prototype)
    }

    #[must_use]
    pub fn error_prototype(&self) -> Option<ObjectId> {
        self.intrinsics.as_ref().map(|i| i.error_prototype)
    }

    /// Create a primitive wrapper object with the given prototype.
    /// The internal `[[PrimitiveValue]]` slot is stored as `ObjectKind::PrimitiveWrapper`.
    pub fn create_primitive_wrapper(
        &mut self,
        value: PrimitiveValue,
        prototype: ObjectId,
    ) -> Result<JsValue, VmError> {
        let mut obj = JsObject::ordinary();
        obj.prototype = Some(prototype);
        obj.kind = ObjectKind::PrimitiveWrapper(value);
        let id = self
            .heap
            .allocate_object(obj)
            .ok_or_else(|| VmError::runtime_limit("heap exhausted"))?;
        Ok(JsValue::Object(id))
    }

    /// Return the internal `[[PrimitiveValue]]` slot of a wrapper object, if any.
    #[must_use]
    pub fn primitive_value(&self, object: ObjectId) -> Option<&PrimitiveValue> {
        self.heap.object(object)?.primitive_value()
    }

    /// Collects the `for-in` enumeration keys for `object`: own enumerable
    /// string keys followed by inherited ones, de-duplicated, walking the
    /// prototype chain.
    pub fn own_enumerable_keys(&self, object: ObjectId) -> Vec<String> {
        self.heap.object(object)
            .map(|o| o.enumerable_own_keys())
            .unwrap_or_default()
    }

    pub fn for_in_keys(&self, object: ObjectId) -> Result<Vec<String>, VmError> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        let mut current = Some(object);
        let mut depth = 0usize;
        while let Some(id) = current {
            self.check_deadline()?;
            if depth > PROTOTYPE_CHAIN_LIMIT {
                return Err(VmError::runtime_limit("prototype chain limit exceeded"));
            }
            let Some(obj) = self.heap.object(id) else {
                break;
            };
            for key in obj.enumerable_own_keys() {
                if seen.insert(key.clone()) {
                    if result.len() >= MAX_ARRAY_LENGTH {
                        return Err(VmError::runtime_limit(
                            "property enumeration limit exceeded",
                        ));
                    }
                    result.push(key);
                }
            }
            current = obj.prototype;
            depth += 1;
        }
        Ok(result)
    }

    #[must_use]
    pub const fn global_environment(&self) -> EnvironmentId {
        self.global_environment
    }

    #[must_use]
    pub const fn current_environment(&self) -> EnvironmentId {
        self.current_environment
    }

    pub fn push_environment(
        &mut self,
        outer: Option<EnvironmentId>,
    ) -> Result<EnvironmentId, VmError> {
        let mut environment = Environment::default();
        environment.outer = outer;
        let id = self
            .heap
            .allocate_environment(environment)
            .ok_or_else(|| VmError::runtime_limit("environment arena exhausted"))?;
        self.environment_stack.push(self.current_environment);
        self.current_environment = id;
        Ok(id)
    }

    pub fn push_existing_environment(&mut self, id: EnvironmentId) -> Result<(), VmError> {
        if self.heap.environment(id).is_none() {
            return Err(VmError::runtime("missing lexical environment"));
        }
        self.environment_stack.push(self.current_environment);
        self.current_environment = id;
        Ok(())
    }

    pub fn pop_environment(&mut self) -> Result<(), VmError> {
        let previous = self
            .environment_stack
            .pop()
            .ok_or_else(|| VmError::runtime("environment stack underflow"))?;
        self.current_environment = previous;
        Ok(())
    }

    #[must_use]
    pub fn environment_depth(&self) -> usize {
        self.environment_stack.len()
    }

    pub fn restore_environment_depth(&mut self, depth: usize) -> Result<(), VmError> {
        if depth > self.environment_stack.len() {
            return Err(VmError::runtime(format!(
                "cannot restore environment depth {} from {}",
                depth,
                self.environment_stack.len()
            )));
        }
        while self.environment_stack.len() > depth {
            self.pop_environment()?;
        }
        Ok(())
    }

    pub fn declare_global(&mut self, name: impl Into<String>, value: JsValue) -> bool {
        let name = name.into();
        let created = {
            let environment = self
                .heap
                .environment_mut(self.global_environment)
                .expect("global environment must exist");
            environment.create_binding(name.clone(), value.clone(), true)
        };
        let _ = self.define_own_property(
            self.global_object,
            name,
            PropertyDescriptor::data_with(value, true, true, true),
        );
        created
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<JsValue> {
        let environment = self.heap.environment(self.global_environment)?;
        environment.get_binding_value(name).ok()
    }

    pub fn set_global(&mut self, name: &str, value: JsValue) -> bool {
        let ok = {
            let Some(environment) = self.heap.environment_mut(self.global_environment) else {
                return false;
            };
            environment.set_mutable_binding(name, value.clone()).is_ok()
        };
        if ok {
            let _ = self.define_own_property(
                self.global_object,
                name.into(),
                PropertyDescriptor::data_with(value, true, true, true),
            );
        }
        ok
    }

    pub fn declare_binding(
        &mut self,
        environment: EnvironmentId,
        name: impl Into<String>,
        value: JsValue,
        mutable: bool,
    ) -> Result<(), VmError> {
        let name = name.into();
        let environment = self
            .heap
            .environment_mut(environment)
            .ok_or_else(|| VmError::runtime("missing lexical environment"))?;
        if environment.create_binding(name.clone(), value.clone(), mutable) {
            return Ok(());
        }
        environment.set_mutable_binding(&name, value)
    }

    pub fn create_mutable_binding(
        &mut self,
        environment: EnvironmentId,
        name: String,
        initialized: bool,
    ) -> Result<(), VmError> {
        self.heap
            .environment_mut(environment)
            .ok_or_else(|| VmError::runtime("missing lexical environment"))?
            .create_mutable_binding(name, initialized)
    }

    pub fn create_immutable_binding(
        &mut self,
        environment: EnvironmentId,
        name: String,
    ) -> Result<(), VmError> {
        self.heap
            .environment_mut(environment)
            .ok_or_else(|| VmError::runtime("missing lexical environment"))?
            .create_immutable_binding(name)
    }

    pub fn initialize_binding(
        &mut self,
        environment: EnvironmentId,
        name: &str,
        value: JsValue,
    ) -> Result<(), VmError> {
        self.heap
            .environment_mut(environment)
            .ok_or_else(|| VmError::runtime("missing lexical environment"))?
            .initialize_binding(name, value)
    }

    #[must_use]
    pub fn resolve_binding(&self, name: &str) -> Option<(EnvironmentId, JsValue)> {
        self.resolve_binding_value(name).ok().flatten()
    }

    pub fn resolve_binding_value(
        &self,
        name: &str,
    ) -> Result<Option<(EnvironmentId, JsValue)>, VmError> {
        let mut current = Some(self.current_environment);
        while let Some(id) = current {
            let environment = self
                .heap
                .environment(id)
                .ok_or_else(|| VmError::runtime("missing lexical environment"))?;
            if let Some(binding) = environment.binding(name) {
                if !binding.initialized {
                    return Err(VmError::reference(format!(
                        "cannot access {name} before initialization"
                    )));
                }
                return Ok(Some((id, binding.value.clone())));
            }
            current = environment.outer;
        }
        Ok(None)
    }

    pub fn resolve_binding_environment(
        &self,
        name: &str,
    ) -> Result<Option<EnvironmentId>, VmError> {
        let mut current = Some(self.current_environment);
        while let Some(id) = current {
            let environment = self
                .heap
                .environment(id)
                .ok_or_else(|| VmError::runtime("missing lexical environment"))?;
            if environment.has_binding(name) {
                return Ok(Some(id));
            }
            current = environment.outer;
        }
        Ok(None)
    }

    pub fn set_binding(&mut self, name: &str, value: JsValue) -> Result<(), VmError> {
        let mut current = Some(self.current_environment);
        while let Some(id) = current {
            let outer = {
                let environment = self
                    .heap
                    .environment_mut(id)
                    .ok_or_else(|| VmError::runtime("missing lexical environment"))?;
                if environment.has_binding(name) {
                    return environment.set_mutable_binding(name, value);
                }
                environment.outer
            };
            current = outer;
        }
        Err(VmError::reference(format!("{name} is not defined")))
    }

    pub fn allocate_function(&mut self, function: JsFunction) -> Result<FunctionId, VmError> {
        let function_name = function.name.clone().unwrap_or_default();
        let function_length = function.params.len();
        let mut function_object = JsObject::ordinary();
        function_object.prototype = self.function_prototype_object();
        let function_object_id = self
            .heap
            .allocate_object(function_object)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;

        let id = self
            .heap
            .allocate_function(function)
            .ok_or_else(|| VmError::runtime_limit("function arena exhausted"))?;
        self.register_function_object(id, function_object_id);

        let mut prototype = JsObject::ordinary();
        prototype.prototype = self.object_prototype();
        prototype.define_property(
            "constructor",
            PropertyDescriptor::data_with(JsValue::Function(id), true, false, true),
        );
        let prototype_id = self
            .heap
            .allocate_object(prototype)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
        self.function_prototypes.insert(id, prototype_id);

        let function_object = self
            .heap
            .object_mut(function_object_id)
            .ok_or_else(|| VmError::runtime("missing function object"))?;
        function_object.define_property(
            "name",
            PropertyDescriptor::data_with(JsValue::String(function_name), false, false, true),
        );
        function_object.define_property(
            "length",
            PropertyDescriptor::data_with(
                JsValue::Number(function_length as f64),
                false,
                false,
                true,
            ),
        );
        function_object.define_property(
            "prototype",
            PropertyDescriptor::data_with(JsValue::Object(prototype_id), true, false, false),
        );
        Ok(id)
    }

    #[must_use]
    pub fn function(&self, id: FunctionId) -> Option<&JsFunction> {
        self.heap.function(id)
    }

    #[must_use]
    pub fn function_prototype(&self, id: FunctionId) -> Option<ObjectId> {
        self.function_prototypes.get(&id).copied()
    }

    pub fn push_call_frame(&mut self, frame: CallFrame) -> Result<(), VmError> {
        self.consume_call_depth()?;
        self.call_frames.push(frame);
        Ok(())
    }

    pub fn pop_call_frame(&mut self) -> Result<CallFrame, VmError> {
        let frame = self
            .call_frames
            .pop()
            .ok_or_else(|| VmError::runtime("call frame stack underflow"))?;
        self.call_depth = self.call_depth.saturating_sub(1);
        Ok(frame)
    }

    #[must_use]
    pub fn current_this(&self) -> JsValue {
        self.call_frames
            .last()
            .map_or(JsValue::Undefined, |frame| frame.this_value.clone())
    }

    #[must_use]
    pub fn current_or_global_this(&self) -> JsValue {
        self.call_frames.last().map_or_else(
            || self.top_level_this.clone(),
            |frame| frame.this_value.clone(),
        )
    }

    #[must_use]
    pub const fn global_object(&self) -> ObjectId {
        self.global_object
    }

    #[must_use]
    pub const fn global_this_value(&self) -> JsValue {
        JsValue::Object(self.global_object)
    }

    pub fn set_top_level_this(&mut self, value: JsValue) {
        self.top_level_this = value;
    }

    pub fn create_object(
        &mut self,
        properties: impl IntoIterator<Item = (String, JsValue)>,
    ) -> Result<JsValue, VmError> {
        let mut object = JsObject::ordinary();
        object.prototype = self.object_prototype();
        for (name, value) in properties {
            object.define_property(name, PropertyDescriptor::data(value));
        }
        let id = self
            .heap
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
        Ok(JsValue::Object(id))
    }

    pub fn create_array(&mut self, elements: Vec<JsValue>) -> Result<JsValue, VmError> {
        if elements.len() > MAX_ARRAY_LENGTH {
            return Err(VmError::range("invalid array length"));
        }
        let mut array = JsObject::array(elements);
        array.prototype = self.array_prototype();
        let id = self
            .heap
            .allocate_object(array)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
        Ok(JsValue::Object(id))
    }

    pub fn create_regexp(&mut self, pattern: String, flags: String) -> Result<JsValue, VmError> {
        let mut object = JsObject::ordinary();
        object.prototype = self.regexp_prototype().or_else(|| self.object_prototype());

        // ES2023 §22.2.6 — RegExp instance own properties.
        // `source` and `flags` are non-writable, non-enumerable, configurable.
        let sorted_flags = sort_regexp_flags(&flags);
        let props = [
            (
                "source",
                JsValue::String(if pattern.is_empty() {
                    "(?:)".into()
                } else {
                    pattern.clone()
                }),
            ),
            ("flags", JsValue::String(sorted_flags)),
            ("global", JsValue::Boolean(flags.contains('g'))),
            ("ignoreCase", JsValue::Boolean(flags.contains('i'))),
            ("multiline", JsValue::Boolean(flags.contains('m'))),
            ("dotAll", JsValue::Boolean(flags.contains('s'))),
            ("sticky", JsValue::Boolean(flags.contains('y'))),
            ("unicode", JsValue::Boolean(flags.contains('u'))),
            ("unicodeSets", JsValue::Boolean(flags.contains('v'))),
            ("hasIndices", JsValue::Boolean(flags.contains('d'))),
        ];
        for (name, value) in props {
            object.properties.define(
                name,
                PropertyDescriptor::data_with(value, false, false, true),
            );
        }
        // `lastIndex` is writable, non-enumerable, non-configurable.
        object.properties.define(
            "lastIndex",
            PropertyDescriptor::data_with(JsValue::Number(0.0), true, false, false),
        );

        object.kind = ObjectKind::RegExp { pattern, flags };
        let id = self
            .heap
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
        Ok(JsValue::Object(id))
    }

    pub fn create_sparse_array(&mut self, length: usize) -> Result<JsValue, VmError> {
        if length > MAX_ARRAY_LENGTH {
            return Err(VmError::range("invalid array length"));
        }
        let mut array = JsObject::sparse_array(length);
        array.prototype = self.array_prototype();
        let id = self
            .heap
            .allocate_object(array)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
        Ok(JsValue::Object(id))
    }

    pub fn get(&mut self, receiver: JsValue, key: &str) -> Result<JsValue, VmError> {
        // JsValue::Error is not a heap object; synthesize the standard own properties
        // so that `thrown.constructor`, `thrown.name`, and `thrown.message` work
        // inside `assert.throws` and similar harness helpers.
        if let JsValue::Error(ref error) = receiver {
            return match key {
                "message" => Ok(JsValue::String(error.message.clone())),
                "name" => Ok(JsValue::String(error_kind_name(&error.kind).into())),
                "constructor" => Ok(self
                    .find_builtin_by_name(error_kind_name(&error.kind))
                    .unwrap_or(JsValue::Undefined)),
                "stack" | "cause" => Ok(JsValue::Undefined),
                _ => Ok(JsValue::Undefined),
            };
        }

        let object = self.property_lookup_object(&receiver)?;
        let Some((_, descriptor)) = self.find_property_descriptor(object, key)? else {
            return Ok(JsValue::Undefined);
        };

        match descriptor.kind {
            PropertyKind::Data { value, .. } => Ok(value),
            PropertyKind::Accessor { get: None, .. } => Ok(JsValue::Undefined),
            PropertyKind::Accessor { get: Some(_), .. } => Err(VmError::type_error(
                "accessor getter invocation requires the VM call path",
            )),
        }
    }

    fn property_lookup_object(&mut self, receiver: &JsValue) -> Result<ObjectId, VmError> {
        match receiver {
            JsValue::Boolean(value) => {
                let prototype = self
                    .boolean_prototype()
                    .ok_or_else(|| VmError::runtime("Boolean prototype not installed"))?;
                let wrapper =
                    self.create_primitive_wrapper(PrimitiveValue::Boolean(*value), prototype)?;
                self.require_object(&wrapper, "read property")
            }
            JsValue::Number(value) => {
                let prototype = self
                    .number_prototype()
                    .ok_or_else(|| VmError::runtime("Number prototype not installed"))?;
                let wrapper =
                    self.create_primitive_wrapper(PrimitiveValue::Number(*value), prototype)?;
                self.require_object(&wrapper, "read property")
            }
            JsValue::BigInt(value) => {
                let prototype = self
                    .get_global("BigInt")
                    .and_then(|constructor| self.value_object(&constructor))
                    .and_then(|constructor| {
                        self.find_property_descriptor(constructor, "prototype")
                            .ok()
                            .flatten()
                            .and_then(|(_, descriptor)| descriptor.value_cloned())
                            .and_then(|value| self.value_object(&value))
                    })
                    .ok_or_else(|| VmError::runtime("BigInt prototype not installed"))?;
                let wrapper =
                    self.create_primitive_wrapper(PrimitiveValue::BigInt(*value), prototype)?;
                self.require_object(&wrapper, "read property")
            }
            JsValue::String(value) => {
                let prototype = self
                    .string_prototype()
                    .ok_or_else(|| VmError::runtime("String prototype not installed"))?;
                let wrapper = self
                    .create_primitive_wrapper(PrimitiveValue::String(value.clone()), prototype)?;
                self.require_object(&wrapper, "read property")
            }
            JsValue::Symbol(value) => {
                let prototype = self
                    .get_global("Symbol")
                    .and_then(|constructor| self.value_object(&constructor))
                    .and_then(|constructor| {
                        self.find_property_descriptor(constructor, "prototype")
                            .ok()
                            .flatten()
                            .and_then(|(_, descriptor)| descriptor.value_cloned())
                            .and_then(|value| self.value_object(&value))
                    })
                    .ok_or_else(|| VmError::runtime("Symbol prototype not installed"))?;
                let wrapper =
                    self.create_primitive_wrapper(PrimitiveValue::Symbol(*value), prototype)?;
                self.require_object(&wrapper, "read property")
            }
            _ => self.require_object(receiver, "read property"),
        }
    }

    pub fn set(
        &mut self,
        receiver: JsValue,
        key: &str,
        value: JsValue,
        strict: bool,
    ) -> Result<bool, VmError> {
        let object = self.require_object(&receiver, "write property")?;
        self.set_object_property(object, key, value, strict)
    }

    pub fn define_own_property(
        &mut self,
        object: ObjectId,
        key: String,
        descriptor: PropertyDescriptor,
    ) -> Result<bool, VmError> {
        if self.is_array_object(object)? {
            if key == "length" {
                return self.define_array_length_property(object, descriptor);
            }
            if let Some(index) = array_index(&key) {
                return self.define_array_index_property(object, index, descriptor);
            }
        }

        let object = self
            .heap
            .object_mut(object)
            .ok_or_else(|| VmError::runtime("missing object"))?;
        object.define_property(key, descriptor);
        Ok(true)
    }

    pub fn validate_and_apply_property_descriptor(
        &mut self,
        object: ObjectId,
        key: String,
        update: PropertyDescriptorUpdate,
    ) -> Result<bool, VmError> {
        if descriptor_update_has_data(&update) && descriptor_update_has_accessor(&update) {
            return Err(VmError::type_error("invalid mixed property descriptor"));
        }

        if self.is_array_object(object)? && key == "length" {
            return self.validate_and_apply_array_length_descriptor(object, update);
        }

        let current = self.get_own_property_descriptor(object, &key);
        let Some(descriptor) = validate_and_apply_descriptor_update(current, update) else {
            return Ok(false);
        };
        self.define_own_property(object, key, descriptor)
    }

    #[must_use]
    pub fn get_own_property(&self, object: ObjectId, key: &str) -> Option<&PropertyDescriptor> {
        self.heap.object(object)?.own_property(key)
    }

    #[must_use]
    pub fn get_own_property_descriptor(
        &self,
        object: ObjectId,
        key: &str,
    ) -> Option<PropertyDescriptor> {
        let object = self.heap.object(object)?;
        if key == "length"
            && let Some(length) = object.array_length()
        {
            return Some(PropertyDescriptor::data_with(
                JsValue::Number(length as f64),
                object.array_length_writable().unwrap_or(false),
                false,
                false,
            ));
        }
        if let Some(index) = array_index(key)
            && matches!(object.kind, ObjectKind::Array { .. })
            && let Some(descriptor) = object.array_element_descriptor(index)
        {
            return Some(descriptor);
        }
        object.own_property(key).cloned()
    }

    pub fn delete_property(
        &mut self,
        object: ObjectId,
        key: &str,
        strict: bool,
    ) -> Result<bool, VmError> {
        if key == "length" && self.is_array_object(object)? {
            return strict_error_or_false(strict, "cannot delete array length");
        }

        if let Some(descriptor) = self.get_own_property_descriptor(object, key)
            && !descriptor.configurable
        {
            return strict_error_or_false(strict, "cannot delete non-configurable property");
        }

        let Some(object) = self.heap.object_mut(object) else {
            return Err(VmError::runtime("missing object"));
        };
        if !object.has_own_property(key) {
            return Ok(true);
        }
        Ok(object.delete_own_property(key).is_some())
    }

    pub fn has_property(&self, object: ObjectId, key: &str) -> Result<bool, VmError> {
        let mut current = Some(object);
        let mut depth = 0usize;
        while let Some(id) = current {
            if depth > PROTOTYPE_CHAIN_LIMIT {
                return Err(VmError::runtime_limit("prototype chain limit exceeded"));
            }
            let object = self
                .heap
                .object(id)
                .ok_or_else(|| VmError::runtime("missing object"))?;
            if object.has_own_property(key) {
                return Ok(true);
            }
            current = object.prototype;
            depth += 1;
        }
        Ok(false)
    }

    #[must_use]
    pub fn get_prototype_of(&self, object: ObjectId) -> Option<ObjectId> {
        self.heap.object(object)?.prototype
    }

    pub fn set_prototype_of(
        &mut self,
        object: ObjectId,
        prototype: Option<ObjectId>,
    ) -> Result<bool, VmError> {
        if prototype == Some(object) {
            return Err(VmError::type_error("prototype cycle rejected"));
        }

        let mut current = prototype;
        let mut depth = 0usize;
        while let Some(id) = current {
            if id == object {
                return Err(VmError::type_error("prototype cycle rejected"));
            }
            if depth > PROTOTYPE_CHAIN_LIMIT {
                return Err(VmError::runtime_limit("prototype chain limit exceeded"));
            }
            current = self
                .heap
                .object(id)
                .ok_or_else(|| VmError::runtime("missing prototype object"))?
                .prototype;
            depth += 1;
        }

        let object = self
            .heap
            .object_mut(object)
            .ok_or_else(|| VmError::runtime("missing object"))?;
        object.prototype = prototype;
        Ok(true)
    }

    pub fn ordinary_object_with_prototype(
        &mut self,
        prototype: Option<ObjectId>,
    ) -> Result<JsValue, VmError> {
        let mut object = JsObject::ordinary();
        object.prototype = prototype;
        let id = self
            .heap
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
        Ok(JsValue::Object(id))
    }

    pub fn instance_of(&self, value: JsValue, constructor: JsValue) -> Result<bool, VmError> {
        self.ordinary_instance_of(value, constructor)
    }

    pub fn ordinary_instance_of(
        &self,
        value: JsValue,
        constructor: JsValue,
    ) -> Result<bool, VmError> {
        if let JsValue::BuiltinFunction(id) = &constructor
            && let Some(bound) = self.builtin(*id).and_then(|builtin| builtin.bound.as_ref())
        {
            return self.ordinary_instance_of(value, bound.target.clone());
        }

        // Native error values (JsValue::Error) are not heap objects, so handle them
        // separately: check the error kind against the constructor name hierarchy.
        if let JsValue::Error(ref error) = value {
            if !self.is_constructable_value(&constructor) {
                return Err(VmError::type_error(
                    "right-hand side of instanceof is not a constructor",
                ));
            }
            let JsValue::BuiltinFunction(id) = &constructor else {
                return Ok(false);
            };
            let Some(builtin) = self.builtin(*id) else {
                return Ok(false);
            };
            return Ok(native_error_is_instance_of(&error.kind, builtin.name));
        }
        let Some(object) = self.value_object(&value) else {
            return Ok(false);
        };
        let constructor_object = self.value_object(&constructor).ok_or_else(|| {
            VmError::type_error("right-hand side of instanceof is not a constructor")
        })?;
        if !self.is_constructable_value(&constructor) {
            return Err(VmError::type_error(
                "right-hand side of instanceof is not a constructor",
            ));
        }
        let prototype = self
            .find_property_descriptor(constructor_object, "prototype")?
            .and_then(|(_, descriptor)| descriptor.value_cloned())
            .and_then(|value| self.value_object(&value))
            .ok_or_else(|| VmError::type_error("constructor prototype is not an object"))?;

        let mut current = self.get_prototype_of(object);
        let mut depth = 0usize;
        while let Some(id) = current {
            if depth > PROTOTYPE_CHAIN_LIMIT {
                return Err(VmError::runtime_limit("prototype chain limit exceeded"));
            }
            if id == prototype {
                return Ok(true);
            }
            current = self.get_prototype_of(id);
            depth += 1;
        }
        Ok(false)
    }

    #[must_use]
    pub fn is_constructable_value(&self, value: &JsValue) -> bool {
        match value {
            JsValue::Function(_) => true,
            JsValue::BuiltinFunction(id) => self
                .builtin(*id)
                .is_some_and(|builtin| builtin.construct.is_some()),
            _ => false,
        }
    }

    pub fn constructor_prototype(
        &self,
        constructor: &JsValue,
    ) -> Result<Option<ObjectId>, VmError> {
        let Some(constructor_object) = self.value_object(constructor) else {
            return Err(VmError::type_error("value is not a constructor"));
        };
        let prototype = self
            .find_property_descriptor(constructor_object, "prototype")?
            .and_then(|(_, descriptor)| descriptor.value_cloned())
            .and_then(|value| self.value_object(&value));
        Ok(prototype.or_else(|| self.object_prototype()))
    }

    pub fn get_property(&mut self, object: JsValue, name: &str) -> Result<JsValue, VmError> {
        self.get(object, name)
    }

    pub fn set_property(
        &mut self,
        object: JsValue,
        name: impl Into<String>,
        value: JsValue,
    ) -> Result<JsValue, VmError> {
        let name = name.into();
        let strict = self.strict;
        // `set` returns Ok(false) for non-writable in sloppy mode (silent ignore per spec),
        // and Err(TypeError) for non-writable in strict mode — both are correct here.
        self.set(object, &name, value.clone(), strict)?;
        Ok(value)
    }

    pub fn get_element(&mut self, object: JsValue, key: JsValue) -> Result<JsValue, VmError> {
        let key = to_property_key(&key)?;
        self.get_property(object, &key)
    }

    pub fn get_iterator(&mut self, value: JsValue) -> Result<IteratorRecord, VmError> {
        match value {
            JsValue::String(string) => Ok(IteratorRecord::string(string)),
            value => {
                let object = self.require_object(&value, "iterate")?;
                if self.is_array_object(object)? {
                    let length = self
                        .heap
                        .object(object)
                        .and_then(JsObject::array_length)
                        .ok_or_else(|| VmError::runtime("missing array object"))?;
                    Ok(IteratorRecord::array(value, length))
                } else if let Some(JsObject {
                    kind: ObjectKind::PrimitiveWrapper(PrimitiveValue::String(string)),
                    ..
                }) = self.heap.object(object)
                {
                    Ok(IteratorRecord::string(string.clone()))
                } else {
                    Err(VmError::type_error("value is not iterable"))
                }
            }
        }
    }

    pub fn iterator_next(
        &mut self,
        iterator: &mut IteratorRecord,
    ) -> Result<Option<JsValue>, VmError> {
        if iterator.done {
            return Ok(None);
        }
        match &mut iterator.kind {
            IteratorKind::Array {
                object,
                index,
                length,
            } => {
                if *index >= *length {
                    iterator.done = true;
                    return Ok(None);
                }
                let key = JsValue::Number(*index as f64);
                *index += 1;
                self.get_element(object.clone(), key).map(Some)
            }
            IteratorKind::String { chars, index } => {
                if *index >= chars.len() {
                    iterator.done = true;
                    return Ok(None);
                }
                let value = JsValue::String(chars[*index].clone());
                *index += 1;
                Ok(Some(value))
            }
            IteratorKind::Js { .. } => Err(VmError::runtime(
                "JS iterator records must be advanced by the VM",
            )),
        }
    }

    pub fn iterator_close(&mut self, iterator: &mut IteratorRecord) -> Result<(), VmError> {
        iterator.done = true;
        Ok(())
    }

    /// Create a heap-allocated iterator object from an iterable value.
    /// Returns a `JsValue::Object` that `IteratorNext` / `IteratorClose` can use.
    pub fn create_iterator_object(&mut self, iterable: JsValue) -> Result<JsValue, VmError> {
        let record = self.get_iterator(iterable)?;
        let obj = JsObject::iterator(record);
        let id = self
            .heap_mut()
            .allocate_object(obj)
            .ok_or_else(|| VmError::runtime("heap full: cannot allocate iterator object"))?;
        Ok(JsValue::Object(id))
    }

    /// Advance an iterator object one step.
    /// Returns `(value, done)` — `done = true` means the iteration has finished.
    pub fn step_iterator_object(
        &mut self,
        iterator_val: JsValue,
    ) -> Result<(JsValue, bool), VmError> {
        let id = match &iterator_val {
            JsValue::Object(id) => *id,
            _ => return Err(VmError::type_error("value is not an iterator object")),
        };
        // Clone the IteratorRecord to avoid holding an immutable borrow on
        // `self.heap` while calling `iterator_next` (which may access the heap
        // for array element lookups).
        let record_clone = {
            let obj = self
                .heap()
                .object(id)
                .ok_or_else(|| VmError::runtime("invalid iterator object"))?;
            match &obj.kind {
                ObjectKind::Iterator { record } => record.clone(),
                _ => return Err(VmError::type_error("object is not an iterator")),
            }
        };
        let mut record = record_clone;
        let result = self.iterator_next(&mut record)?;
        // Write the updated index / done flag back into the heap object.
        if let Some(obj) = self.heap_mut().object_mut(id)
            && let ObjectKind::Iterator { record: r } = &mut obj.kind
        {
            *r = record;
        }
        match result {
            Some(value) => Ok((value, false)),
            None => Ok((JsValue::Undefined, true)),
        }
    }

    /// Mark an iterator object as exhausted (used on `break` / early exit).
    pub fn close_iterator_object(&mut self, iterator_val: JsValue) -> Result<(), VmError> {
        let id = match &iterator_val {
            JsValue::Object(id) => *id,
            _ => return Ok(()), // not our iterator object — nothing to close
        };
        if let Some(obj) = self.heap_mut().object_mut(id)
            && let ObjectKind::Iterator { record } = &mut obj.kind
        {
            record.done = true;
        }
        Ok(())
    }

    pub fn set_element(
        &mut self,
        object: JsValue,
        key: JsValue,
        value: JsValue,
    ) -> Result<JsValue, VmError> {
        let key = to_property_key(&key)?;
        self.set_property(object, key, value)
    }

    /// Like `set_property` but always uses strict-mode semantics (throws TypeError for
    /// non-writable targets). Used by intrinsic functions whose spec steps say `Set(O, P, V, true)`.
    pub fn set_property_strict(
        &mut self,
        object: JsValue,
        name: impl Into<String>,
        value: JsValue,
    ) -> Result<JsValue, VmError> {
        let name = name.into();
        if self.set(object, &name, value.clone(), true)? {
            Ok(value)
        } else {
            Err(VmError::type_error(format!("cannot write property {name}")))
        }
    }

    /// Like `set_element` but always uses strict-mode semantics.
    pub fn set_element_strict(
        &mut self,
        object: JsValue,
        key: JsValue,
        value: JsValue,
    ) -> Result<JsValue, VmError> {
        let key = to_property_key(&key)?;
        self.set_property_strict(object, key, value)
    }

    fn set_object_property(
        &mut self,
        object: ObjectId,
        key: &str,
        value: JsValue,
        strict: bool,
    ) -> Result<bool, VmError> {
        if key == "length" && self.is_array_object(object)? {
            let length = self.array_length_from_value(value)?;
            return self.set_array_length(object, length);
        }

        if let Some(descriptor) = self.get_own_property_descriptor(object, key) {
            return match descriptor.kind {
                PropertyKind::Data {
                    writable: false, ..
                } => strict_error_or_false(strict, "cannot write non-writable property"),
                PropertyKind::Data { .. } => {
                    let object = self
                        .heap
                        .object_mut(object)
                        .ok_or_else(|| VmError::runtime("missing object"))?;
                    Ok(object.set_own_property_value(key, value))
                }
                PropertyKind::Accessor { set: None, .. } => {
                    strict_error_or_false(strict, "property setter is undefined")
                }
                PropertyKind::Accessor { set: Some(_), .. } => Err(VmError::type_error(
                    "accessor setter invocation requires the VM call path",
                )),
            };
        }

        if let Some(index) = array_index(key)
            && self.is_array_object(object)?
            && index >= MAX_ARRAY_LENGTH
        {
            return Err(VmError::range("invalid array length"));
        }

        if let Some(prototype) = self.get_prototype_of(object)
            && let Some((_, descriptor)) = self.find_property_descriptor(prototype, key)?
        {
            match descriptor.kind {
                PropertyKind::Data {
                    writable: false, ..
                } => {
                    return strict_error_or_false(
                        strict,
                        "cannot write inherited non-writable property",
                    );
                }
                PropertyKind::Accessor { set: None, .. } => {
                    return strict_error_or_false(strict, "inherited property setter is undefined");
                }
                PropertyKind::Accessor { set: Some(_), .. } => {
                    return Err(VmError::type_error(
                        "accessor setter invocation requires the VM call path",
                    ));
                }
                PropertyKind::Data { .. } => {}
            }
        }

        self.define_own_property(object, key.into(), PropertyDescriptor::data(value))
    }

    pub(crate) fn find_property_descriptor(
        &self,
        object: ObjectId,
        key: &str,
    ) -> Result<Option<(ObjectId, PropertyDescriptor)>, VmError> {
        let mut current = Some(object);
        let mut depth = 0usize;
        while let Some(id) = current {
            if depth > PROTOTYPE_CHAIN_LIMIT {
                return Err(VmError::runtime_limit("prototype chain limit exceeded"));
            }
            if let Some(descriptor) = self.get_own_property_descriptor(id, key) {
                return Ok(Some((id, descriptor)));
            }
            current = self
                .heap
                .object(id)
                .ok_or_else(|| VmError::runtime("missing object"))?
                .prototype;
            depth += 1;
        }
        Ok(None)
    }

    pub(crate) fn find_symbol_property_descriptor(
        &self,
        object: ObjectId,
        symbol: SymbolId,
    ) -> Result<Option<(ObjectId, PropertyDescriptor)>, VmError> {
        let mut current = Some(object);
        let mut depth = 0usize;
        while let Some(id) = current {
            if depth > PROTOTYPE_CHAIN_LIMIT {
                return Err(VmError::runtime_limit("prototype chain limit exceeded"));
            }
            let object = self
                .heap
                .object(id)
                .ok_or_else(|| VmError::runtime("missing object"))?;
            if let Some(descriptor) = object.own_symbol_property(symbol) {
                return Ok(Some((id, descriptor.clone())));
            }
            current = object.prototype;
            depth += 1;
        }
        Ok(None)
    }

    pub fn is_array_object(&self, object: ObjectId) -> Result<bool, VmError> {
        Ok(matches!(
            self.heap
                .object(object)
                .ok_or_else(|| VmError::runtime("missing object"))?
                .kind,
            ObjectKind::Array { .. }
        ))
    }

    fn define_array_index_property(
        &mut self,
        object: ObjectId,
        index: usize,
        descriptor: PropertyDescriptor,
    ) -> Result<bool, VmError> {
        if index >= MAX_ARRAY_LENGTH {
            return Err(VmError::range("invalid array length"));
        }
        let object = self
            .heap
            .object_mut(object)
            .ok_or_else(|| VmError::runtime("missing object"))?;
        Ok(object.define_array_element(index, descriptor))
    }

    fn define_array_length_property(
        &mut self,
        object: ObjectId,
        descriptor: PropertyDescriptor,
    ) -> Result<bool, VmError> {
        if descriptor.configurable || descriptor.enumerable {
            return Ok(false);
        }
        let PropertyKind::Data { value, writable } = descriptor.kind else {
            return Ok(false);
        };
        let length = self.array_length_from_value(value)?;
        if !self.set_array_length(object, length)? {
            return Ok(false);
        }
        self.set_array_length_writable(object, writable)
    }

    fn validate_and_apply_array_length_descriptor(
        &mut self,
        object: ObjectId,
        update: PropertyDescriptorUpdate,
    ) -> Result<bool, VmError> {
        if descriptor_update_has_accessor(&update) {
            return Ok(false);
        }
        if update.configurable == Some(true) || update.enumerable == Some(true) {
            return Ok(false);
        }

        let current = self
            .get_own_property_descriptor(object, "length")
            .ok_or_else(|| VmError::runtime("missing array length descriptor"))?;
        let current_length = current
            .value_cloned()
            .and_then(|value| value.to_number())
            .unwrap_or(0.0) as usize;
        let current_writable = current.writable();

        if !current_writable {
            if update.writable == Some(true) {
                return Ok(false);
            }
            if let Some(value) = update.value {
                let length = self.array_length_from_value(value)?;
                if length != current_length {
                    return Ok(false);
                }
            }
            return Ok(true);
        }

        if let Some(value) = update.value {
            let length = self.array_length_from_value(value)?;
            if !self.set_array_length(object, length)? {
                return Ok(false);
            }
        }
        if update.writable == Some(false) {
            return self.set_array_length_writable(object, false);
        }
        Ok(true)
    }

    pub fn set_array_length(&mut self, object: ObjectId, length: usize) -> Result<bool, VmError> {
        if length > MAX_ARRAY_LENGTH {
            return Err(VmError::range("invalid array length"));
        }
        let object = self
            .heap
            .object_mut(object)
            .ok_or_else(|| VmError::runtime("missing object"))?;
        Ok(object.set_array_length(length))
    }

    fn set_array_length_writable(
        &mut self,
        object: ObjectId,
        writable: bool,
    ) -> Result<bool, VmError> {
        let object = self
            .heap
            .object_mut(object)
            .ok_or_else(|| VmError::runtime("missing object"))?;
        Ok(object.set_array_length_writable(writable))
    }

    pub fn array_length_from_value(&self, value: JsValue) -> Result<usize, VmError> {
        let Some(length) = value.to_number() else {
            return Err(VmError::range("invalid array length"));
        };
        checked_array_length(length)
    }

    pub fn reset_execution_budget(&mut self, loop_limit: u64) {
        self.budget.loop_remaining = loop_limit;
    }

    pub fn reset_stack_limit(&mut self, stack_limit: usize) {
        self.budget.stack_limit = stack_limit;
    }

    pub fn reset_deadline(&mut self, limit: Option<Duration>) {
        self.budget.deadline = limit.map(|limit| Instant::now() + limit);
    }

    pub fn check_deadline(&self) -> Result<(), VmError> {
        self.budget.check_deadline()
    }

    pub fn check_stack_depth(&self, depth: usize) -> Result<(), VmError> {
        self.budget.check_stack_depth(depth)
    }

    pub fn consume_loop_iteration(&mut self) -> Result<(), VmError> {
        self.budget.check_loop()
    }

    #[must_use]
    pub const fn loop_budget_remaining(&self) -> u64 {
        self.budget.loop_remaining
    }

    pub fn reset_call_depth(&mut self, call_stack_limit: u64) {
        self.budget.call_depth_limit = call_stack_limit;
        self.call_depth = 0;
    }

    pub fn consume_call_depth(&mut self) -> Result<(), VmError> {
        self.budget.check_call_depth(self.call_depth)?;
        self.call_depth += 1;
        Ok(())
    }

    pub fn release_call_depth(&mut self) {
        self.call_depth = self.call_depth.saturating_sub(1);
    }
    #[must_use]
    pub const fn strict(&self) -> bool {
        self.strict
    }

    pub fn set_strict(&mut self, strict: bool) {
        self.strict = strict;
    }

    pub fn push_output(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

    pub fn create_promise(&mut self) -> Result<PromiseId, VmError> {
        let index = u32::try_from(self.promises.len())
            .map_err(|_| VmError::runtime("promise registry full"))?;
        self.promises.push(PromiseRecord::default());
        Ok(PromiseId(index))
    }

    #[must_use]
    pub fn promise_state(&self, promise: PromiseId) -> Option<PromiseState> {
        self.promises
            .get(promise.0 as usize)
            .map(|record| record.state.clone())
    }

    pub fn fulfill_promise(&mut self, promise: PromiseId, value: JsValue) -> Result<bool, VmError> {
        self.settle_promise(promise, PromiseState::Fulfilled(value))
    }

    pub fn reject_promise(&mut self, promise: PromiseId, value: JsValue) -> Result<bool, VmError> {
        self.settle_promise(promise, PromiseState::Rejected(value))
    }

    fn settle_promise(&mut self, promise: PromiseId, state: PromiseState) -> Result<bool, VmError> {
        let record = self
            .promises
            .get_mut(promise.0 as usize)
            .ok_or_else(|| VmError::runtime("invalid promise id"))?;
        if !matches!(record.state, PromiseState::Pending) {
            return Ok(false);
        }
        record.state = state;
        Ok(true)
    }

    pub fn enqueue_job(&mut self, job: Job) -> Result<(), VmError> {
        self.job_queue.push(job);
        Ok(())
    }

    pub fn drain_jobs(&mut self) -> Result<(), VmError> {
        while let Some(job) = self.job_queue.pop() {
            self.run_job(job)?;
        }
        Ok(())
    }

    fn run_job(&mut self, job: Job) -> Result<(), VmError> {
        match job {
            Job::PromiseReaction(PromiseJob {
                promise,
                reaction: PromiseReaction::Fulfill,
                value,
            }) => {
                self.fulfill_promise(promise, value)?;
            }
            Job::PromiseReaction(PromiseJob {
                promise,
                reaction: PromiseReaction::Reject,
                value,
            }) => {
                self.reject_promise(promise, value)?;
            }
            Job::HostCallback(NativeJob::PushOutput(line)) => self.push_output(line),
        }
        Ok(())
    }

    #[must_use]
    pub fn pending_job_count(&self) -> usize {
        self.job_queue.len()
    }

    pub fn create_array_buffer(&mut self, byte_length: usize) -> Result<ArrayBufferId, VmError> {
        if byte_length > MAX_ARRAY_BUFFER_BYTE_LENGTH {
            return Err(VmError::runtime_limit(
                "ArrayBuffer allocation limit exceeded",
            ));
        }
        self.ensure_heap_capacity(byte_length)?;
        let index = u32::try_from(self.array_buffers.len())
            .map_err(|_| VmError::runtime("ArrayBuffer registry full"))?;
        self.array_buffers.push(ArrayBufferRecord::new(byte_length));
        Ok(ArrayBufferId(index))
    }

    pub fn array_buffer_record(&self, buffer: ArrayBufferId) -> Option<&ArrayBufferRecord> {
        self.array_buffers.get(buffer.0 as usize)
    }

    pub fn array_buffer_byte_length(&self, buffer: ArrayBufferId) -> Result<usize, VmError> {
        Ok(self
            .array_buffer_record(buffer)
            .ok_or_else(|| VmError::runtime("invalid ArrayBuffer id"))?
            .byte_length())
    }

    pub fn is_array_buffer_detached(&self, buffer: ArrayBufferId) -> Result<bool, VmError> {
        Ok(self
            .array_buffer_record(buffer)
            .ok_or_else(|| VmError::runtime("invalid ArrayBuffer id"))?
            .detached)
    }

    pub fn detach_array_buffer(&mut self, buffer: ArrayBufferId) -> Result<(), VmError> {
        let record = self
            .array_buffers
            .get_mut(buffer.0 as usize)
            .ok_or_else(|| VmError::runtime("invalid ArrayBuffer id"))?;
        record.bytes.clear();
        record.detached = true;
        Ok(())
    }

    pub fn create_typed_array_view(
        &mut self,
        buffer: ArrayBufferId,
        element_kind: TypedArrayElementKind,
        byte_offset: usize,
        length: usize,
    ) -> Result<TypedArrayViewId, VmError> {
        let byte_length = checked_view_byte_length(length, element_kind.bytes_per_element())?;
        if !byte_offset.is_multiple_of(element_kind.bytes_per_element()) {
            return Err(VmError::range(
                "TypedArray byteOffset is not element-aligned",
            ));
        }
        self.validate_buffer_range(buffer, byte_offset, byte_length)?;
        let index = u32::try_from(self.typed_array_views.len())
            .map_err(|_| VmError::runtime("TypedArray view registry full"))?;
        self.typed_array_views.push(TypedArrayView {
            buffer,
            byte_offset,
            length,
            element_kind,
        });
        Ok(TypedArrayViewId(index))
    }

    pub fn typed_array_view(&self, view: TypedArrayViewId) -> Option<&TypedArrayView> {
        self.typed_array_views.get(view.0 as usize)
    }

    pub fn typed_array_byte_length(&self, view: TypedArrayViewId) -> Result<usize, VmError> {
        self.typed_array_view(view)
            .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
            .byte_length()
            .ok_or_else(|| VmError::runtime_limit("TypedArray byteLength overflow"))
    }

    pub fn typed_array_load_element(
        &self,
        view: TypedArrayViewId,
        index: usize,
    ) -> Result<JsValue, VmError> {
        let view = self
            .typed_array_view(view)
            .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?;
        if index >= view.length {
            return Err(VmError::range("TypedArray index is out of range"));
        }
        let byte_offset = typed_array_element_offset(view, index)?;
        let bytes = self.read_buffer_bytes(
            view.buffer,
            byte_offset,
            view.element_kind.bytes_per_element(),
        )?;
        Ok(JsValue::Number(decode_typed_array_number(
            view.element_kind,
            bytes,
            true,
        )?))
    }

    pub fn typed_array_store_element(
        &mut self,
        view: TypedArrayViewId,
        index: usize,
        value: JsValue,
    ) -> Result<(), VmError> {
        let view = self
            .typed_array_view(view)
            .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
            .clone();
        if index >= view.length {
            return Err(VmError::range("TypedArray index is out of range"));
        }
        let byte_offset = typed_array_element_offset(&view, index)?;
        let bytes = encode_typed_array_number(view.element_kind, value)?;
        self.write_buffer_bytes(view.buffer, byte_offset, &bytes)
    }

    pub fn create_data_view(
        &mut self,
        buffer: ArrayBufferId,
        byte_offset: usize,
        byte_length: usize,
    ) -> Result<DataViewId, VmError> {
        self.validate_buffer_range(buffer, byte_offset, byte_length)?;
        let index = u32::try_from(self.data_views.len())
            .map_err(|_| VmError::runtime("DataView registry full"))?;
        self.data_views.push(DataViewRecord {
            buffer,
            byte_offset,
            byte_length,
        });
        Ok(DataViewId(index))
    }

    pub fn data_view_record(&self, view: DataViewId) -> Option<&DataViewRecord> {
        self.data_views.get(view.0 as usize)
    }

    pub fn data_view_get(
        &self,
        view: DataViewId,
        request_index: usize,
        element_kind: TypedArrayElementKind,
        little_endian: bool,
    ) -> Result<JsValue, VmError> {
        let view = self
            .data_view_record(view)
            .ok_or_else(|| VmError::runtime("invalid DataView id"))?;
        if element_kind.is_bigint() {
            return Err(VmError::type_error(
                "BigInt DataView elements are not implemented",
            ));
        }
        let width = element_kind.bytes_per_element();
        if request_index
            .checked_add(width)
            .is_none_or(|end| end > view.byte_length)
        {
            return Err(VmError::range("DataView byteOffset is out of range"));
        }
        let byte_offset = view
            .byte_offset
            .checked_add(request_index)
            .ok_or_else(|| VmError::runtime_limit("DataView byteOffset overflow"))?;
        let bytes = self.read_buffer_bytes(view.buffer, byte_offset, width)?;
        Ok(JsValue::Number(decode_typed_array_number(
            element_kind,
            bytes,
            little_endian,
        )?))
    }

    pub fn data_view_set(
        &mut self,
        view: DataViewId,
        request_index: usize,
        element_kind: TypedArrayElementKind,
        value: JsValue,
        little_endian: bool,
    ) -> Result<(), VmError> {
        let view = self
            .data_view_record(view)
            .ok_or_else(|| VmError::runtime("invalid DataView id"))?
            .clone();
        if element_kind.is_bigint() {
            return Err(VmError::type_error(
                "BigInt DataView elements are not implemented",
            ));
        }
        let width = element_kind.bytes_per_element();
        if request_index
            .checked_add(width)
            .is_none_or(|end| end > view.byte_length)
        {
            return Err(VmError::range("DataView byteOffset is out of range"));
        }
        let byte_offset = view
            .byte_offset
            .checked_add(request_index)
            .ok_or_else(|| VmError::runtime_limit("DataView byteOffset overflow"))?;
        let mut bytes = encode_typed_array_number(element_kind, value)?;
        if !little_endian && width > 1 {
            bytes.reverse();
        }
        self.write_buffer_bytes(view.buffer, byte_offset, &bytes)
    }

    fn validate_buffer_range(
        &self,
        buffer: ArrayBufferId,
        byte_offset: usize,
        byte_length: usize,
    ) -> Result<(), VmError> {
        let record = self
            .array_buffer_record(buffer)
            .ok_or_else(|| VmError::runtime("invalid ArrayBuffer id"))?;
        if record.detached {
            return Err(VmError::type_error("ArrayBuffer is detached"));
        }
        if byte_offset
            .checked_add(byte_length)
            .is_none_or(|end| end > record.bytes.len())
        {
            return Err(VmError::range("ArrayBuffer view is out of range"));
        }
        Ok(())
    }

    fn read_buffer_bytes(
        &self,
        buffer: ArrayBufferId,
        byte_offset: usize,
        byte_length: usize,
    ) -> Result<&[u8], VmError> {
        let record = self
            .array_buffer_record(buffer)
            .ok_or_else(|| VmError::runtime("invalid ArrayBuffer id"))?;
        if record.detached {
            return Err(VmError::type_error("ArrayBuffer is detached"));
        }
        let end = byte_offset
            .checked_add(byte_length)
            .ok_or_else(|| VmError::runtime_limit("ArrayBuffer byte range overflow"))?;
        record
            .bytes
            .get(byte_offset..end)
            .ok_or_else(|| VmError::range("ArrayBuffer byte range is out of range"))
    }

    fn write_buffer_bytes(
        &mut self,
        buffer: ArrayBufferId,
        byte_offset: usize,
        bytes: &[u8],
    ) -> Result<(), VmError> {
        let record = self
            .array_buffers
            .get_mut(buffer.0 as usize)
            .ok_or_else(|| VmError::runtime("invalid ArrayBuffer id"))?;
        if record.detached {
            return Err(VmError::type_error("ArrayBuffer is detached"));
        }
        let end = byte_offset
            .checked_add(bytes.len())
            .ok_or_else(|| VmError::runtime_limit("ArrayBuffer byte range overflow"))?;
        let target = record
            .bytes
            .get_mut(byte_offset..end)
            .ok_or_else(|| VmError::range("ArrayBuffer byte range is out of range"))?;
        target.copy_from_slice(bytes);
        Ok(())
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    pub fn take_output(&mut self) -> Vec<String> {
        std::mem::take(&mut self.output)
    }
}

pub fn checked_string_repeat_len(unit_len: usize, count: usize) -> Result<usize, VmError> {
    let len = unit_len
        .checked_mul(count)
        .ok_or_else(|| VmError::runtime_limit("string allocation limit exceeded"))?;
    checked_utf16_allocation(len)?;
    Ok(len)
}

pub fn checked_array_length(length: f64) -> Result<usize, VmError> {
    if !length.is_finite() || length < 0.0 || length.fract() != 0.0 || length > u32::MAX as f64 {
        return Err(VmError::range("invalid array length"));
    }
    let length = length as usize;
    if length > MAX_ARRAY_LENGTH {
        return Err(VmError::runtime_limit("array allocation limit exceeded"));
    }
    Ok(length)
}

fn checked_view_byte_length(length: usize, bytes_per_element: usize) -> Result<usize, VmError> {
    length
        .checked_mul(bytes_per_element)
        .ok_or_else(|| VmError::runtime_limit("typed array byteLength overflow"))
}

fn typed_array_element_offset(view: &TypedArrayView, index: usize) -> Result<usize, VmError> {
    let element_offset = checked_view_byte_length(index, view.element_kind.bytes_per_element())?;
    view.byte_offset
        .checked_add(element_offset)
        .ok_or_else(|| VmError::runtime_limit("typed array byteOffset overflow"))
}

fn encode_typed_array_number(
    kind: TypedArrayElementKind,
    value: JsValue,
) -> Result<Vec<u8>, VmError> {
    if kind.is_bigint() {
        return Err(VmError::type_error(
            "BigInt typed array elements are not implemented",
        ));
    }
    let number = value
        .to_number()
        .ok_or_else(|| VmError::type_error("typed array element value must be numeric"))?;
    let bytes = match kind {
        TypedArrayElementKind::Int8 => vec![(to_uint_n(number, 8) as u8) as i8 as u8],
        TypedArrayElementKind::Uint8 => vec![to_uint_n(number, 8) as u8],
        TypedArrayElementKind::Uint8Clamped => vec![to_uint8_clamp(number)],
        TypedArrayElementKind::Int16 => {
            (to_uint_n(number, 16) as u16 as i16).to_le_bytes().to_vec()
        }
        TypedArrayElementKind::Uint16 => (to_uint_n(number, 16) as u16).to_le_bytes().to_vec(),
        TypedArrayElementKind::Int32 => {
            (to_uint_n(number, 32) as u32 as i32).to_le_bytes().to_vec()
        }
        TypedArrayElementKind::Uint32 => (to_uint_n(number, 32) as u32).to_le_bytes().to_vec(),
        TypedArrayElementKind::Float32 => (number as f32).to_le_bytes().to_vec(),
        TypedArrayElementKind::Float64 => number.to_le_bytes().to_vec(),
        TypedArrayElementKind::BigInt64 | TypedArrayElementKind::BigUint64 => unreachable!(),
    };
    Ok(bytes)
}

fn decode_typed_array_number(
    kind: TypedArrayElementKind,
    bytes: &[u8],
    little_endian: bool,
) -> Result<f64, VmError> {
    if kind.is_bigint() {
        return Err(VmError::type_error(
            "BigInt typed array elements are not implemented",
        ));
    }
    let mut data = bytes.to_vec();
    if !little_endian && data.len() > 1 {
        data.reverse();
    }
    let number = match kind {
        TypedArrayElementKind::Int8 => i8::from_le_bytes([data[0]]) as f64,
        TypedArrayElementKind::Uint8 | TypedArrayElementKind::Uint8Clamped => data[0] as f64,
        TypedArrayElementKind::Int16 => i16::from_le_bytes(
            data.try_into()
                .map_err(|_| VmError::runtime("invalid Int16 byte length"))?,
        ) as f64,
        TypedArrayElementKind::Uint16 => u16::from_le_bytes(
            data.try_into()
                .map_err(|_| VmError::runtime("invalid Uint16 byte length"))?,
        ) as f64,
        TypedArrayElementKind::Int32 => i32::from_le_bytes(
            data.try_into()
                .map_err(|_| VmError::runtime("invalid Int32 byte length"))?,
        ) as f64,
        TypedArrayElementKind::Uint32 => u32::from_le_bytes(
            data.try_into()
                .map_err(|_| VmError::runtime("invalid Uint32 byte length"))?,
        ) as f64,
        TypedArrayElementKind::Float32 => f32::from_le_bytes(
            data.try_into()
                .map_err(|_| VmError::runtime("invalid Float32 byte length"))?,
        ) as f64,
        TypedArrayElementKind::Float64 => f64::from_le_bytes(
            data.try_into()
                .map_err(|_| VmError::runtime("invalid Float64 byte length"))?,
        ),
        TypedArrayElementKind::BigInt64 | TypedArrayElementKind::BigUint64 => unreachable!(),
    };
    Ok(number)
}

fn to_uint_n(number: f64, bits: u32) -> u64 {
    if !number.is_finite() || number == 0.0 {
        return 0;
    }
    let modulo = 2_f64.powi(bits as i32);
    number.trunc().rem_euclid(modulo) as u64
}

fn to_uint8_clamp(number: f64) -> u8 {
    if number.is_nan() || number <= 0.0 {
        return 0;
    }
    if number >= 255.0 {
        return 255;
    }
    let floor = number.floor();
    let fraction = number - floor;
    if fraction < 0.5 {
        floor as u8
    } else if fraction > 0.5 || floor as u64 % 2 == 1 {
        floor as u8 + 1
    } else {
        floor as u8
    }
}

pub fn checked_utf16_allocation(units: usize) -> Result<(), VmError> {
    if units > MAX_UTF16_ALLOCATION_UNITS {
        return Err(VmError::runtime_limit("string allocation limit exceeded"));
    }
    Ok(())
}
fn value_references_live_heap(value: &JsValue, heap: &Heap) -> bool {
    match value {
        JsValue::Object(object) => heap.contains_object(*object),
        JsValue::Function(function) => heap.contains_function(*function),
        JsValue::Undefined
        | JsValue::Null
        | JsValue::Boolean(_)
        | JsValue::Number(_)
        | JsValue::BigInt(_)
        | JsValue::String(_)
        | JsValue::Symbol(_)
        | JsValue::BuiltinFunction(_)
        | JsValue::Error(_) => true,
    }
}
/// Returns the canonical flags string for a RegExp, sorted in the order
/// specified by ES2023 §22.2.6.5 (d, g, i, m, s, u, v, y).
pub fn sort_regexp_flags(flags: &str) -> String {
    const ORDER: &[char] = &['d', 'g', 'i', 'm', 's', 'u', 'v', 'y'];
    ORDER.iter().filter(|&&c| flags.contains(c)).collect()
}

pub fn to_property_key(value: &JsValue) -> Result<String, VmError> {
    match value {
        JsValue::String(value) => Ok(value.clone()),
        JsValue::BigInt(value) => Ok(value.to_string()),
        JsValue::Number(value) if value.fract() == 0.0 => Ok(format!("{value:.0}")),
        JsValue::Number(value) => Ok(value.to_string()),
        JsValue::Boolean(value) => Ok(value.to_string()),
        JsValue::Null => Ok("null".into()),
        JsValue::Undefined => Ok("undefined".into()),
        JsValue::Symbol(_) => Err(VmError::type_error("Cannot convert a Symbol to a string")),
        JsValue::Object(_)
        | JsValue::Function(_)
        | JsValue::BuiltinFunction(_)
        | JsValue::Error(_) => Err(VmError::type_error(
            "object property keys are not supported in native V4",
        )),
    }
}

fn descriptor_update_has_data(update: &PropertyDescriptorUpdate) -> bool {
    update.value.is_some() || update.writable.is_some()
}

fn descriptor_update_has_accessor(update: &PropertyDescriptorUpdate) -> bool {
    update.get.is_some() || update.set.is_some()
}

fn same_optional_value(left: Option<&JsValue>, right: Option<&JsValue>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.same_value(right),
        (None, None) => true,
        _ => false,
    }
}

fn descriptor_from_update(update: PropertyDescriptorUpdate) -> PropertyDescriptor {
    if update.get.is_some() || update.set.is_some() {
        return PropertyDescriptor::accessor(
            update.get.flatten(),
            update.set.flatten(),
            update.enumerable.unwrap_or(false),
            update.configurable.unwrap_or(false),
        );
    }

    PropertyDescriptor::data_with(
        update.value.unwrap_or(JsValue::Undefined),
        update.writable.unwrap_or(false),
        update.enumerable.unwrap_or(false),
        update.configurable.unwrap_or(false),
    )
}

fn validate_and_apply_descriptor_update(
    current: Option<PropertyDescriptor>,
    update: PropertyDescriptorUpdate,
) -> Option<PropertyDescriptor> {
    let Some(current) = current else {
        return Some(descriptor_from_update(update));
    };

    if !current.configurable {
        if update.configurable == Some(true) {
            return None;
        }
        if let Some(enumerable) = update.enumerable
            && enumerable != current.enumerable
        {
            return None;
        }

        match &current.kind {
            PropertyKind::Data { value, writable } => {
                if descriptor_update_has_accessor(&update) {
                    return None;
                }
                if !*writable {
                    if update.writable == Some(true) {
                        return None;
                    }
                    if let Some(new_value) = &update.value
                        && !value.same_value(new_value)
                    {
                        return None;
                    }
                }
            }
            PropertyKind::Accessor { get, set } => {
                if descriptor_update_has_data(&update) {
                    return None;
                }
                if let Some(new_get) = &update.get
                    && !same_optional_value(get.as_ref(), new_get.as_ref())
                {
                    return None;
                }
                if let Some(new_set) = &update.set
                    && !same_optional_value(set.as_ref(), new_set.as_ref())
                {
                    return None;
                }
            }
        }
    }

    let PropertyDescriptorUpdate {
        value: update_value,
        writable: update_writable,
        get: update_get,
        set: update_set,
        enumerable,
        configurable,
    } = update;
    let mut descriptor = current;
    match &mut descriptor.kind {
        PropertyKind::Data {
            value: current_value,
            writable: current_writable,
        } => {
            if update_get.is_some() || update_set.is_some() {
                if !descriptor.configurable {
                    return None;
                }
                descriptor.kind = PropertyKind::Accessor {
                    get: update_get.flatten(),
                    set: update_set.flatten(),
                };
            } else {
                if let Some(new_value) = update_value {
                    *current_value = new_value;
                }
                if let Some(new_writable) = update_writable {
                    *current_writable = new_writable;
                }
            }
        }
        PropertyKind::Accessor {
            get: current_get,
            set: current_set,
        } => {
            if update_value.is_some() || update_writable.is_some() {
                descriptor.kind = PropertyKind::Data {
                    value: update_value.unwrap_or(JsValue::Undefined),
                    writable: update_writable.unwrap_or(false),
                };
            } else {
                if let Some(new_get) = update_get {
                    *current_get = new_get;
                }
                if let Some(new_set) = update_set {
                    *current_set = new_set;
                }
            }
        }
    }
    if let Some(enumerable) = enumerable {
        descriptor.enumerable = enumerable;
    }
    if let Some(configurable) = configurable {
        descriptor.configurable = configurable;
    }
    Some(descriptor)
}

fn strict_error_or_false(strict: bool, message: &str) -> Result<bool, VmError> {
    if strict {
        Err(VmError::type_error(message))
    } else {
        Ok(false)
    }
}

/// Unreachable fallback used as the `call` slot of a bound function. The VM
/// forwards bound invocations to their target, so this is never executed.
fn bound_call_unreachable(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "bound function must be dispatched by the VM",
    ))
}

/// Unreachable fallback used as the `construct` slot of a bound function.
fn bound_construct_unreachable(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    Err(VmError::runtime(
        "bound function must be dispatched by the VM",
    ))
}

/// Maps a `NativeErrorKind` to the JS constructor name seen in `error.name` / `error.constructor.name`.
fn error_kind_name(kind: &NativeErrorKind) -> &'static str {
    match kind {
        NativeErrorKind::Type => "TypeError",
        NativeErrorKind::Reference => "ReferenceError",
        NativeErrorKind::Syntax => "SyntaxError",
        NativeErrorKind::Range | NativeErrorKind::RuntimeLimit => "RangeError",
        NativeErrorKind::Error | NativeErrorKind::Test262 => "Error",
    }
}

/// Returns true if a native error with `kind` is an ECMAScript instance of the named constructor.
/// Error → all; TypeError → TypeError + Error; SyntaxError → SyntaxError + Error; etc.
fn native_error_is_instance_of(kind: &NativeErrorKind, constructor_name: &str) -> bool {
    match constructor_name {
        "Error" => true,
        "TypeError" => matches!(kind, NativeErrorKind::Type),
        "SyntaxError" => matches!(kind, NativeErrorKind::Syntax),
        "ReferenceError" => matches!(kind, NativeErrorKind::Reference),
        "RangeError" => matches!(kind, NativeErrorKind::Range),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{JsValue, NativeContext};

    #[test]
    fn installs_basic_global_values() {
        let context = NativeContext::default();
        assert_eq!(context.get_global("undefined"), Some(JsValue::Undefined));
        assert!(
            matches!(context.get_global("NaN"), Some(JsValue::Number(value)) if value.is_nan())
        );
        assert_eq!(
            context.get_global("Infinity"),
            Some(JsValue::Number(f64::INFINITY))
        );
    }

    #[test]
    fn isolates_global_environments() {
        let mut first = NativeContext::default();
        let second = NativeContext::default();

        first.declare_global("secret", JsValue::Number(42.0));
        assert_eq!(first.get_global("secret"), Some(JsValue::Number(42.0)));
        assert_eq!(second.get_global("secret"), None);
    }

    #[test]
    fn consumes_loop_budget() {
        let mut context = NativeContext::default();
        context.reset_execution_budget(1);

        context.consume_loop_iteration().unwrap();
        let error = context.consume_loop_iteration().unwrap_err();
        assert!(error.message.contains("loop"));
    }
}
