//! Persistent state shared by native execution and integration.

use super::{
    Environment, EnvironmentId, FunctionId, Heap, JsFunction, JsObject, JsValue, ObjectId,
};
use crate::vm::{CallFrame, VmError};

/// Per-isolate language state passed to the bytecode executor.
#[derive(Debug)]
pub struct NativeContext {
    heap: Heap,
    global_environment: EnvironmentId,
    current_environment: EnvironmentId,
    environment_stack: Vec<EnvironmentId>,
    call_frames: Vec<CallFrame>,
    strict: bool,
    output: Vec<String>,
    loop_budget_remaining: u64,
    call_stack_limit: u64,
    call_depth: u64,
}

impl Default for NativeContext {
    fn default() -> Self {
        let mut heap = Heap::default();
        let global_environment = heap
            .allocate_environment(Environment::default())
            .expect("a fresh heap can allocate the global environment");

        let mut context = Self {
            heap,
            global_environment,
            current_environment: global_environment,
            environment_stack: Vec::new(),
            call_frames: Vec::new(),
            strict: false,
            output: Vec::new(),
            loop_budget_remaining: u64::MAX,
            call_stack_limit: u64::MAX,
            call_depth: 0,
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
            .ok_or_else(|| VmError::runtime("environment arena exhausted"))?;
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

    pub fn declare_global(&mut self, name: impl Into<String>, value: JsValue) -> bool {
        let environment = self
            .heap
            .environment_mut(self.global_environment)
            .expect("global environment must exist");
        environment.create_binding(name, value, true)
    }

    #[must_use]
    pub fn get_global(&self, name: &str) -> Option<JsValue> {
        let environment = self.heap.environment(self.global_environment)?;
        environment
            .binding(name)
            .map(|binding| binding.value.clone())
    }

    pub fn set_global(&mut self, name: &str, value: JsValue) -> bool {
        let Some(environment) = self.heap.environment_mut(self.global_environment) else {
            return false;
        };
        environment.set_mutable_binding(name, value)
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
        if environment.set_mutable_binding(&name, value) {
            return Ok(());
        }
        Err(VmError::type_error(format!(
            "cannot update immutable binding {name}"
        )))
    }

    #[must_use]
    pub fn resolve_binding(&self, name: &str) -> Option<(EnvironmentId, JsValue)> {
        let mut current = Some(self.current_environment);
        while let Some(id) = current {
            let environment = self.heap.environment(id)?;
            if let Some(binding) = environment.binding(name) {
                return Some((id, binding.value.clone()));
            }
            current = environment.outer;
        }
        None
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
                    if environment.set_mutable_binding(name, value) {
                        return Ok(());
                    }
                    return Err(VmError::type_error(format!(
                        "cannot update immutable binding {name}"
                    )));
                }
                environment.outer
            };
            current = outer;
        }
        Err(VmError::reference(format!("{name} is not defined")))
    }

    pub fn allocate_function(&mut self, function: JsFunction) -> Result<FunctionId, VmError> {
        self.heap
            .allocate_function(function)
            .ok_or_else(|| VmError::runtime("function arena exhausted"))
    }

    #[must_use]
    pub fn function(&self, id: FunctionId) -> Option<&JsFunction> {
        self.heap.function(id)
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

    pub fn create_object(
        &mut self,
        properties: impl IntoIterator<Item = (String, JsValue)>,
    ) -> Result<JsValue, VmError> {
        let mut object = JsObject::ordinary();
        for (name, value) in properties {
            object.set_own_property_value(name, value);
        }
        let id = self
            .heap
            .allocate_object(object)
            .ok_or_else(|| VmError::runtime("object arena exhausted"))?;
        Ok(JsValue::Object(id))
    }

    pub fn create_array(&mut self, elements: Vec<JsValue>) -> Result<JsValue, VmError> {
        let id = self
            .heap
            .allocate_object(JsObject::array(elements))
            .ok_or_else(|| VmError::runtime("object arena exhausted"))?;
        Ok(JsValue::Object(id))
    }

    pub fn get_property(&self, object: JsValue, name: &str) -> Result<JsValue, VmError> {
        let JsValue::Object(id) = object else {
            return Err(VmError::type_error(format!("cannot read property {name}")));
        };
        Ok(get_object_property(&self.heap, id, name).unwrap_or(JsValue::Undefined))
    }

    pub fn set_property(
        &mut self,
        object: JsValue,
        name: impl Into<String>,
        value: JsValue,
    ) -> Result<JsValue, VmError> {
        let name = name.into();
        let JsValue::Object(id) = object else {
            return Err(VmError::type_error(format!("cannot write property {name}")));
        };
        let object = self
            .heap
            .object_mut(id)
            .ok_or_else(|| VmError::runtime("missing object"))?;
        if object.set_own_property_value(&name, value.clone()) {
            Ok(value)
        } else {
            Err(VmError::type_error(format!("cannot write property {name}")))
        }
    }

    pub fn get_element(&self, object: JsValue, key: JsValue) -> Result<JsValue, VmError> {
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

    pub fn reset_execution_budget(&mut self, loop_limit: u64) {
        self.loop_budget_remaining = loop_limit;
    }

    pub fn consume_loop_iteration(&mut self) -> Result<(), VmError> {
        if self.loop_budget_remaining == 0 {
            return Err(VmError::runtime_limit("loop iteration limit exceeded"));
        }

        self.loop_budget_remaining -= 1;
        Ok(())
    }

    #[must_use]
    pub const fn loop_budget_remaining(&self) -> u64 {
        self.loop_budget_remaining
    }

    pub fn reset_call_depth(&mut self, call_stack_limit: u64) {
        self.call_stack_limit = call_stack_limit;
        self.call_depth = 0;
    }

    pub fn consume_call_depth(&mut self) -> Result<(), VmError> {
        if self.call_depth >= self.call_stack_limit {
            return Err(VmError::runtime_limit("call stack limit exceeded"));
        }
        self.call_depth += 1;
        Ok(())
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

pub fn to_property_key(value: &JsValue) -> Result<String, VmError> {
    match value {
        JsValue::String(value) => Ok(value.clone()),
        JsValue::Number(value) if value.fract() == 0.0 => Ok(format!("{value:.0}")),
        JsValue::Number(value) => Ok(value.to_string()),
        JsValue::Boolean(value) => Ok(value.to_string()),
        JsValue::Null => Ok("null".into()),
        JsValue::Undefined => Ok("undefined".into()),
        JsValue::Object(_)
        | JsValue::Function(_)
        | JsValue::NativeFunction(_)
        | JsValue::Error(_) => Err(VmError::type_error(
            "object property keys are not supported in native V3",
        )),
    }
}

fn get_object_property(heap: &Heap, id: ObjectId, name: &str) -> Option<JsValue> {
    let object = heap.object(id)?;
    object.get_own_property_value(name).or_else(|| {
        object
            .prototype
            .and_then(|prototype| get_object_property(heap, prototype, name))
    })
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
