//! Lexical environment records.

use std::collections::HashMap;

use super::JsValue;
use crate::vm::VmError;

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

    pub fn create_mutable_binding(
        &mut self,
        name: String,
        initialized: bool,
    ) -> Result<(), VmError> {
        if self.bindings.contains_key(&name) {
            return Err(VmError::type_error(format!("duplicate binding {name}")));
        }
        self.bindings.insert(
            name,
            Binding {
                value: JsValue::Undefined,
                mutable: true,
                initialized,
            },
        );
        Ok(())
    }

    pub fn create_immutable_binding(&mut self, name: String) -> Result<(), VmError> {
        if self.bindings.contains_key(&name) {
            return Err(VmError::type_error(format!("duplicate binding {name}")));
        }
        self.bindings.insert(
            name,
            Binding {
                value: JsValue::Undefined,
                mutable: false,
                initialized: false,
            },
        );
        Ok(())
    }

    pub fn initialize_binding(&mut self, name: &str, value: JsValue) -> Result<(), VmError> {
        let binding = self
            .bindings
            .get_mut(name)
            .ok_or_else(|| VmError::reference(format!("{name} is not defined")))?;
        if binding.initialized {
            return Err(VmError::type_error(format!(
                "binding {name} is already initialized"
            )));
        }
        binding.value = value;
        binding.initialized = true;
        Ok(())
    }

    #[must_use]
    pub fn binding(&self, name: &str) -> Option<&Binding> {
        self.bindings.get(name)
    }

    pub fn get_binding_value(&self, name: &str) -> Result<JsValue, VmError> {
        let binding = self
            .bindings
            .get(name)
            .ok_or_else(|| VmError::reference(format!("{name} is not defined")))?;
        if !binding.initialized {
            return Err(VmError::reference(format!(
                "cannot access {name} before initialization"
            )));
        }
        Ok(binding.value.clone())
    }

    #[must_use]
    pub fn has_binding(&self, name: &str) -> bool {
        self.bindings.contains_key(name)
    }

    pub fn set_mutable_binding(&mut self, name: &str, value: JsValue) -> Result<(), VmError> {
        let binding = self
            .bindings
            .get_mut(name)
            .ok_or_else(|| VmError::reference(format!("{name} is not defined")))?;
        if !binding.initialized {
            return Err(VmError::reference(format!(
                "cannot assign {name} before initialization"
            )));
        }
        if !binding.mutable {
            return Err(VmError::type_error(format!(
                "cannot update immutable binding {name}"
            )));
        }
        binding.value = value;
        Ok(())
    }
}
