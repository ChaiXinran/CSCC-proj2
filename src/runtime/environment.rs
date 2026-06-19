//! Lexical environment records.

use std::collections::HashMap;

use super::JsValue;

/// Stable handle into an environment arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnvironmentId(pub u32);

/// One variable binding.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub value: JsValue,
    pub mutable: bool,
    pub initialized: bool,
}

/// One lexical scope and its outer scope.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub outer: Option<EnvironmentId>,
    bindings: HashMap<String, Binding>,
}

impl Environment {
    pub fn create_binding(
        &mut self,
        name: impl Into<String>,
        value: JsValue,
        mutable: bool,
    ) -> bool {
        let name = name.into();
        if self.bindings.contains_key(&name) {
            return false;
        }
        self.bindings.insert(
            name,
            Binding {
                value,
                mutable,
                initialized: true,
            },
        );
        true
    }

    #[must_use]
    pub fn binding(&self, name: &str) -> Option<&Binding> {
        self.bindings.get(name)
    }
}
