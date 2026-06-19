//! Persistent state shared by native execution and integration.

use super::{Environment, EnvironmentId, Heap, JsValue};
use crate::vm::VmError;

/// Per-isolate language state passed to the bytecode executor.
#[derive(Debug)]
pub struct NativeContext {
    heap: Heap,
    global_environment: EnvironmentId,
    strict: bool,
    output: Vec<String>,
    loop_budget_remaining: u64,
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
            strict: false,
            output: Vec::new(),
            loop_budget_remaining: u64::MAX,
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
