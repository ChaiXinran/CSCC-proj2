//! Lexical environment records.

use std::collections::HashMap;

use super::{JsValue, ObjectId, Trace, Tracer};
use crate::bytecode::{LocalLayout, LocalSlot};
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
    /// True when the binding was created by a `let`, `const`, `class`,
    /// catch-parameter, or function-parameter declaration. False for `var`
    /// and `function` declarations. Used by eval declaration instantiation
    /// to detect invalid redeclarations.
    pub lexical: bool,
}

/// One lexical scope and its outer scope.
#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub outer: Option<EnvironmentId>,
    pub with_object: Option<ObjectId>,
    slots: Vec<Binding>,
    slot_names: Vec<String>,
    slot_index: HashMap<String, LocalSlot>,
    bindings: HashMap<String, Binding>,
}

impl Environment {
    #[must_use]
    pub fn with_local_layout(outer: Option<EnvironmentId>, layout: &LocalLayout) -> Self {
        let mut slot_names = Vec::with_capacity(layout.bindings.len());
        let mut slot_index = HashMap::with_capacity(layout.bindings.len());
        let mut slots = Vec::with_capacity(layout.bindings.len());
        for (index, binding) in layout.bindings.iter().enumerate() {
            let slot = LocalSlot(u16::try_from(index).expect("validated local layout"));
            slot_names.push(binding.name.clone());
            slot_index.insert(binding.name.clone(), slot);
            slots.push(Binding {
                value: JsValue::Undefined,
                mutable: binding.mutable,
                initialized: binding.initialized_at_entry,
                lexical: binding.lexical,
            });
        }
        Self {
            outer,
            with_object: None,
            slots,
            slot_names,
            slot_index,
            bindings: HashMap::new(),
        }
    }

    #[must_use]
    pub fn local_slot(&self, name: &str) -> Option<LocalSlot> {
        self.slot_index.get(name).copied()
    }

    pub fn get_local(&self, slot: LocalSlot) -> Result<JsValue, VmError> {
        let index = usize::from(slot.0);
        let binding = self
            .slots
            .get(index)
            .ok_or_else(|| VmError::runtime(format!("local slot {} is out of bounds", slot.0)))?;
        if !binding.initialized {
            let name = self.slot_names.get(index).map_or("<local>", String::as_str);
            return Err(VmError::reference(format!(
                "cannot access {name} before initialization"
            )));
        }
        Ok(binding.value.clone())
    }

    pub fn set_local(&mut self, slot: LocalSlot, value: JsValue) -> Result<(), VmError> {
        let index = usize::from(slot.0);
        let binding = self
            .slots
            .get_mut(index)
            .ok_or_else(|| VmError::runtime(format!("local slot {} is out of bounds", slot.0)))?;
        let name = self.slot_names.get(index).map_or("<local>", String::as_str);
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

    pub fn initialize_local(&mut self, slot: LocalSlot, value: JsValue) -> Result<(), VmError> {
        let index = usize::from(slot.0);
        let binding = self
            .slots
            .get_mut(index)
            .ok_or_else(|| VmError::runtime(format!("local slot {} is out of bounds", slot.0)))?;
        if binding.initialized {
            let name = self.slot_names.get(index).map_or("<local>", String::as_str);
            return Err(VmError::type_error(format!(
                "binding {name} is already initialized"
            )));
        }
        binding.value = value;
        binding.initialized = true;
        Ok(())
    }

    pub fn create_binding(
        &mut self,
        name: impl Into<String>,
        value: JsValue,
        mutable: bool,
        lexical: bool,
    ) -> bool {
        let name = name.into();
        if let Some(slot) = self.local_slot(&name) {
            let binding = &self.slots[usize::from(slot.0)];
            return if binding.initialized {
                self.set_local(slot, value).is_ok()
            } else {
                self.initialize_local(slot, value).is_ok()
            };
        }
        if self.bindings.contains_key(&name) {
            return false;
        }
        self.bindings.insert(
            name,
            Binding {
                value,
                mutable,
                initialized: true,
                lexical,
            },
        );
        true
    }

    pub fn create_mutable_binding(
        &mut self,
        name: String,
        initialized: bool,
        lexical: bool,
    ) -> Result<(), VmError> {
        if self.bindings.contains_key(&name) {
            return Err(VmError::syntax_error(format!("duplicate binding {name}")));
        }
        self.bindings.insert(
            name,
            Binding {
                value: JsValue::Undefined,
                mutable: true,
                initialized,
                lexical,
            },
        );
        Ok(())
    }

    pub fn create_immutable_binding(&mut self, name: String, lexical: bool) -> Result<(), VmError> {
        if self.bindings.contains_key(&name) {
            return Err(VmError::syntax_error(format!("duplicate binding {name}")));
        }
        self.bindings.insert(
            name,
            Binding {
                value: JsValue::Undefined,
                mutable: false,
                initialized: false,
                lexical,
            },
        );
        Ok(())
    }

    pub fn initialize_binding(&mut self, name: &str, value: JsValue) -> Result<(), VmError> {
        if let Some(slot) = self.local_slot(name) {
            return self.initialize_local(slot, value);
        }
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
        self.local_slot(name)
            .and_then(|slot| self.slots.get(usize::from(slot.0)))
            .or_else(|| self.bindings.get(name))
    }

    pub fn get_binding_value(&self, name: &str) -> Result<JsValue, VmError> {
        if let Some(slot) = self.local_slot(name) {
            return self.get_local(slot);
        }
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
        self.slot_index.contains_key(name) || self.bindings.contains_key(name)
    }

    pub fn set_mutable_binding(&mut self, name: &str, value: JsValue) -> Result<(), VmError> {
        if let Some(slot) = self.local_slot(name) {
            return self.set_local(slot, value);
        }
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

    pub fn with_object(object: ObjectId, outer: Option<EnvironmentId>) -> Self {
        Self {
            outer,
            with_object: Some(object),
            slots: Vec::new(),
            slot_names: Vec::new(),
            slot_index: HashMap::new(),
            bindings: HashMap::new(),
        }
    }
}

impl Trace for Environment {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(outer) = self.outer {
            tracer.mark_environment(outer);
        }
        if let Some(object) = self.with_object {
            tracer.mark_object(object);
        }
        for binding in self.bindings.values() {
            binding.value.trace(tracer);
        }
        for binding in &self.slots {
            binding.value.trace(tracer);
        }
    }
}

impl Environment {
    #[must_use]
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(
                self.bindings
                    .iter()
                    .map(|(name, binding)| {
                        name.len()
                            .saturating_add(std::mem::size_of::<Binding>())
                            .saturating_add(binding.value.estimated_bytes())
                    })
                    .sum::<usize>(),
            )
            .saturating_add(
                self.slot_names
                    .iter()
                    .map(String::len)
                    .sum::<usize>()
                    .saturating_add(
                        self.slots
                            .len()
                            .saturating_mul(std::mem::size_of::<Binding>()),
                    ),
            )
    }
}
