//! Persistent state shared by native execution and integration.

use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use super::{
    BoundFunction, BuiltinFunction, BuiltinId, CollectionStats, Collector, Environment,
    EnvironmentId, FunctionId, Heap, HeapStats, JsFunction, JsObject, JsValue, NativeCall,
    NativeConstruct, NativeErrorKind, ObjectId, ObjectKind, PrimitiveValue, PropertyDescriptor,
    PropertyDescriptorUpdate, PropertyKind, RootSet, SymbolId, SymbolRegistry, WellKnownSymbols,
    object::array_index,
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
    strict: bool,
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
            strict: false,
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
        self.call_frames
            .last()
            .map_or(JsValue::Object(self.global_object), |frame| {
                frame.this_value.clone()
            })
    }

    #[must_use]
    pub const fn global_object(&self) -> ObjectId {
        self.global_object
    }

    #[must_use]
    pub const fn global_this_value(&self) -> JsValue {
        JsValue::Object(self.global_object)
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

        let object = self.require_object(&receiver, "read property")?;
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
        let Some(current) = current else {
            let descriptor = descriptor_from_update(update);
            return self.define_own_property(object, key, descriptor);
        };

        if !current.configurable {
            if update.configurable == Some(true) {
                return Ok(false);
            }
            if let Some(enumerable) = update.enumerable
                && enumerable != current.enumerable
            {
                return Ok(false);
            }

            match &current.kind {
                PropertyKind::Data { value, writable } => {
                    if descriptor_update_has_accessor(&update) {
                        return Ok(false);
                    }
                    if !*writable {
                        if update.writable == Some(true) {
                            return Ok(false);
                        }
                        if let Some(new_value) = &update.value
                            && !value.same_value(new_value)
                        {
                            return Ok(false);
                        }
                    }
                }
                PropertyKind::Accessor { get, set } => {
                    if descriptor_update_has_data(&update) {
                        return Ok(false);
                    }
                    if let Some(new_get) = &update.get
                        && !same_optional_value(get.as_ref(), new_get.as_ref())
                    {
                        return Ok(false);
                    }
                    if let Some(new_set) = &update.set
                        && !same_optional_value(set.as_ref(), new_set.as_ref())
                    {
                        return Ok(false);
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
                        return Ok(false);
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
        | JsValue::String(_)
        | JsValue::Symbol(_)
        | JsValue::BuiltinFunction(_)
        | JsValue::Error(_) => true,
    }
}
pub fn to_property_key(value: &JsValue) -> Result<String, VmError> {
    match value {
        JsValue::String(value) => Ok(value.clone()),
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
