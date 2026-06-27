//! JavaScript objects and prototype links.

use super::iterator::IteratorKind;
use super::{
    ArrayBufferId, DataViewId, EnvironmentId, FunctionId, IteratorRecord, JsValue, PromiseId,
    PropertyDescriptor, PropertyMap, SymbolId, Trace, Tracer, TypedArrayViewId,
};

/// Stable handle into the runtime heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    String(String),
    Symbol(SymbolId),
}

impl PropertyKey {
    #[must_use]
    pub fn to_value(&self) -> JsValue {
        match self {
            Self::String(value) => JsValue::String(value.clone()),
            Self::Symbol(symbol) => JsValue::Symbol(*symbol),
        }
    }
}

/// Primitive value stored in a wrapper object's internal slot (ECMAScript [[PrimitiveValue]]).
/// Excludes objects, functions, errors, null, and undefined — only the three wrappable primitives.
#[derive(Debug, Clone, PartialEq)]
pub enum PrimitiveValue {
    Boolean(bool),
    Number(f64),
    BigInt(i128),
    String(String),
    Symbol(SymbolId),
}

/// Dense storage is capped at this many slots to prevent OOM from large-length arrays.
/// Indices >= this threshold are stored in the regular property map.
/// 64K × ~56 bytes ≈ 3.5 MB max per array; keeps 4 concurrent workers bounded.
const MAX_DENSE_SIZE: usize = 1 << 16; // 65536 elements

/// Object storage variants.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum ObjectKind {
    #[default]
    Ordinary,
    Array {
        elements: Vec<Option<PropertyDescriptor>>,
        /// Logical ECMAScript [[Length]] — may exceed `elements.len()` for sparse arrays.
        length: u32,
        length_writable: bool,
    },
    /// Primitive wrapper object (created by `new Number(…)`, `new String(…)`, `new Boolean(…)`).
    /// Stores the internal [[PrimitiveValue]] slot. Wrapper objects still have normal `properties`
    /// and a `prototype` link; only the internal slot is special.
    PrimitiveWrapper(PrimitiveValue),
    /// Regular expression object. The `pattern` and `flags` are the source strings.
    /// Used by `String.prototype.match`, `search`, `replace`, `split`, etc. to detect regexp args.
    RegExp {
        pattern: String,
        flags: String,
    },
    ArrayBuffer {
        buffer: ArrayBufferId,
    },
    DataView {
        view: DataViewId,
    },
    TypedArray {
        view: TypedArrayViewId,
        length: usize,
        name: String,
    },
    /// Internal iterator object created by `GetIterator`. Not directly observable from JS.
    Iterator {
        record: IteratorRecord,
    },
    /// Generator object produced by calling a generator function.
    Generator {
        record: GeneratorRecord,
    },
    /// Promise object backed by the shared native Promise registry.
    Promise {
        promise: PromiseId,
    },
    /// ECMAScript Proxy exotic object.
    Proxy {
        record: ProxyRecord,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GeneratorState {
    SuspendedStart,
    SuspendedYield,
    Executing,
    Completed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratorRecord {
    pub function: FunctionId,
    pub environment: Option<EnvironmentId>,
    pub this_value: JsValue,
    pub arguments: Vec<JsValue>,
    pub next_ip: usize,
    pub state: GeneratorState,
    pub stack: Vec<JsValue>,
    pub delegate_values: Vec<JsValue>,
    pub delegate_iterator: Option<JsValue>,
    pub delegate_return: Option<JsValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProxyRecord {
    pub target: JsValue,
    pub handler: JsValue,
    pub callable: bool,
    pub constructable: bool,
}

/// Ordinary object storage.
#[derive(Debug, Clone)]
pub struct JsObject {
    pub prototype: Option<ObjectId>,
    pub kind: ObjectKind,
    pub properties: PropertyMap,
    /// Symbol-keyed own properties stored separately from the string property map.
    /// Insertion order is preserved; lookup is linear but the expected count is small.
    pub symbol_properties: Vec<(SymbolId, PropertyDescriptor)>,
    pub extensible: bool,
}

impl Default for JsObject {
    fn default() -> Self {
        Self {
            prototype: None,
            kind: ObjectKind::default(),
            properties: PropertyMap::default(),
            symbol_properties: Vec::new(),
            extensible: true,
        }
    }
}

impl JsObject {
    #[must_use]
    pub fn ordinary() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn array(elements: Vec<JsValue>) -> Self {
        let elems: Vec<Option<PropertyDescriptor>> = elements
            .into_iter()
            .map(|value| Some(PropertyDescriptor::data(value)))
            .collect();
        let len = elems.len() as u32;
        Self {
            prototype: None,
            kind: ObjectKind::Array {
                elements: elems,
                length: len,
                length_writable: true,
            },
            properties: PropertyMap::default(),
            symbol_properties: Vec::new(),
            extensible: true,
        }
    }

    #[must_use]
    pub fn sparse_array(length: usize) -> Self {
        Self {
            prototype: None,
            kind: ObjectKind::Array {
                elements: Vec::new(),
                length: length.min(u32::MAX as usize) as u32,
                length_writable: true,
            },
            properties: PropertyMap::default(),
            symbol_properties: Vec::new(),
            extensible: true,
        }
    }

    #[must_use]
    pub fn iterator(record: IteratorRecord) -> Self {
        Self {
            prototype: None,
            kind: ObjectKind::Iterator { record },
            properties: PropertyMap::default(),
            symbol_properties: Vec::new(),
            extensible: true,
        }
    }

    #[must_use]
    pub fn proxy(target: JsValue, handler: JsValue, callable: bool, constructable: bool) -> Self {
        Self {
            prototype: None,
            kind: ObjectKind::Proxy {
                record: ProxyRecord {
                    target,
                    handler,
                    callable,
                    constructable,
                },
            },
            properties: PropertyMap::default(),
            symbol_properties: Vec::new(),
            extensible: true,
        }
    }

    pub fn define_property(&mut self, name: impl Into<String>, descriptor: PropertyDescriptor) {
        self.properties.define(name, descriptor);
    }

    /// Update the logical length if an index write extends past the current length.
    fn update_length_for_index(length: &mut u32, index: usize) {
        let new_len = (index + 1).min(u32::MAX as usize) as u32;
        if new_len > *length {
            *length = new_len;
        }
    }

    /// Define or replace a symbol-keyed own property.
    pub fn define_symbol_property(&mut self, id: SymbolId, descriptor: PropertyDescriptor) {
        if let Some((_, desc)) = self
            .symbol_properties
            .iter_mut()
            .find(|(sym_id, _)| *sym_id == id)
        {
            *desc = descriptor;
        } else {
            self.symbol_properties.push((id, descriptor));
        }
    }

    /// Look up a symbol-keyed own property descriptor (does not walk prototype chain).
    #[must_use]
    pub fn own_symbol_property(&self, id: SymbolId) -> Option<&PropertyDescriptor> {
        self.symbol_properties
            .iter()
            .find(|(sym_id, _)| *sym_id == id)
            .map(|(_, desc)| desc)
    }

    /// Delete a symbol-keyed own property, returning the removed descriptor.
    pub fn delete_own_symbol_property(&mut self, id: SymbolId) -> Option<PropertyDescriptor> {
        if let Some(pos) = self
            .symbol_properties
            .iter()
            .position(|(sym_id, _)| *sym_id == id)
        {
            Some(self.symbol_properties.remove(pos).1)
        } else {
            None
        }
    }

    #[must_use]
    pub fn own_property(&self, name: &str) -> Option<&PropertyDescriptor> {
        self.properties.get(name)
    }

    #[must_use]
    pub fn get_own_property_value(&self, name: &str) -> Option<JsValue> {
        if let ObjectKind::Array {
            elements, length, ..
        } = &self.kind
        {
            if name == "length" {
                return Some(JsValue::Number(*length as f64));
            }
            if let Some(index) = array_index(name)
                && index < elements.len()
            {
                return elements[index]
                    .as_ref()
                    .and_then(PropertyDescriptor::value_cloned);
                // index beyond dense range — fall through to property map
            }
        }

        self.own_property(name)
            .and_then(PropertyDescriptor::value_cloned)
    }

    pub fn set_own_property_value(&mut self, name: impl Into<String>, value: JsValue) -> bool {
        let name = name.into();
        if let ObjectKind::Array {
            elements,
            length,
            length_writable,
        } = &mut self.kind
        {
            if name == "length" {
                return false;
            }
            if let Some(index) = array_index(&name) {
                if index >= *length as usize && !*length_writable {
                    return false;
                }
                if index < MAX_DENSE_SIZE {
                    if index >= elements.len() {
                        elements.resize(index + 1, None);
                    }
                    if let Some(descriptor) = &mut elements[index] {
                        let ok = descriptor.set_value(value);
                        Self::update_length_for_index(length, index);
                        return ok;
                    }
                    elements[index] = Some(PropertyDescriptor::data(value));
                    Self::update_length_for_index(length, index);
                    return true;
                }
                // huge index: fall through to property map, but update length
                Self::update_length_for_index(length, index);
            }
        }

        if let Some(descriptor) = self.properties.get_mut(&name) {
            descriptor.set_value(value)
        } else {
            self.define_property(name, PropertyDescriptor::data(value));
            true
        }
    }

    #[must_use]
    pub fn has_own_property(&self, name: &str) -> bool {
        if let ObjectKind::TypedArray { length, .. } = &self.kind
            && let Some(index) = array_index(name)
        {
            return index < *length;
        }
        if let ObjectKind::PrimitiveWrapper(PrimitiveValue::String(value)) = &self.kind {
            if name == "length" {
                return true;
            }
            if let Some(index) = array_index(name) {
                return index < value.encode_utf16().count();
            }
        }
        if let ObjectKind::Array { elements, .. } = &self.kind {
            if name == "length" {
                return true;
            }
            if let Some(index) = array_index(name)
                && index < elements.len()
            {
                return elements[index].is_some();
                // beyond dense range — fall through to property map
            }
        }
        self.properties.contains_key(name)
    }

    pub fn delete_own_property(&mut self, name: &str) -> Option<PropertyDescriptor> {
        if let ObjectKind::Array { elements, .. } = &mut self.kind
            && let Some(index) = array_index(name)
            && index < elements.len()
        {
            if let Some(slot) = elements.get_mut(index)
                && let Some(descriptor) = slot.take()
            {
                return Some(descriptor);
            }
            return None; // in dense range but not set
            // huge index: fall through to property map
        }
        self.properties.delete(name)
    }

    #[must_use]
    pub fn array_length(&self) -> Option<usize> {
        match &self.kind {
            ObjectKind::Array { length, .. } => Some(*length as usize),
            ObjectKind::Ordinary
            | ObjectKind::PrimitiveWrapper(_)
            | ObjectKind::RegExp { .. }
            | ObjectKind::ArrayBuffer { .. }
            | ObjectKind::DataView { .. }
            | ObjectKind::TypedArray { .. }
            | ObjectKind::Iterator { .. }
            | ObjectKind::Generator { .. }
            | ObjectKind::Promise { .. }
            | ObjectKind::Proxy { .. } => None,
        }
    }

    #[must_use]
    pub fn array_length_writable(&self) -> Option<bool> {
        match &self.kind {
            ObjectKind::Array {
                length_writable, ..
            } => Some(*length_writable),
            ObjectKind::Ordinary
            | ObjectKind::PrimitiveWrapper(_)
            | ObjectKind::RegExp { .. }
            | ObjectKind::ArrayBuffer { .. }
            | ObjectKind::DataView { .. }
            | ObjectKind::TypedArray { .. }
            | ObjectKind::Iterator { .. }
            | ObjectKind::Generator { .. }
            | ObjectKind::Promise { .. }
            | ObjectKind::Proxy { .. } => None,
        }
    }

    pub fn primitive_value(&self) -> Option<&PrimitiveValue> {
        match &self.kind {
            ObjectKind::PrimitiveWrapper(value) => Some(value),
            _ => None,
        }
    }

    pub fn set_array_length(&mut self, new_len: usize) -> bool {
        let ObjectKind::Array {
            elements,
            length,
            length_writable,
        } = &mut self.kind
        else {
            return false;
        };
        if !*length_writable {
            return false;
        }
        let new_len32 = new_len.min(u32::MAX as usize) as u32;
        if new_len >= elements.len() {
            // Growing: just update the logical length; no allocation needed.
            *length = new_len32;
            return true;
        }

        // Shrinking: delete dense elements from the end.
        for index in (new_len..elements.len()).rev() {
            if elements[index]
                .as_ref()
                .is_some_and(|descriptor| !descriptor.configurable)
            {
                elements.truncate(index + 1);
                *length = (index + 1) as u32;
                return false;
            }
            elements[index] = None;
        }
        elements.truncate(new_len);
        *length = new_len32;
        true
    }

    pub fn set_array_length_writable(&mut self, writable: bool) -> bool {
        let ObjectKind::Array {
            length_writable, ..
        } = &mut self.kind
        else {
            return false;
        };
        *length_writable = writable;
        true
    }

    #[must_use]
    pub fn array_element_descriptor(&self, index: usize) -> Option<PropertyDescriptor> {
        let ObjectKind::Array { elements, .. } = &self.kind else {
            return None;
        };
        elements.get(index).cloned().flatten()
    }

    pub fn define_array_element(&mut self, index: usize, descriptor: PropertyDescriptor) -> bool {
        let ObjectKind::Array {
            elements,
            length,
            length_writable,
        } = &mut self.kind
        else {
            return false;
        };
        if index >= *length as usize && !*length_writable {
            return false;
        }
        if index < MAX_DENSE_SIZE {
            if index >= elements.len() {
                elements.resize(index + 1, None);
            }
            elements[index] = Some(descriptor);
            Self::update_length_for_index(length, index);
            return true;
        }
        // huge index: not supported in dense storage
        false
    }

    #[must_use]
    pub fn own_property_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if let ObjectKind::Array { elements, .. } = &self.kind {
            keys.extend(
                elements
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| value.is_some())
                    .map(|(index, _)| index.to_string()),
            );
            keys.push("length".into());
        } else if let ObjectKind::PrimitiveWrapper(PrimitiveValue::String(value)) = &self.kind {
            keys.extend((0..value.encode_utf16().count()).map(|index| index.to_string()));
            keys.push("length".into());
        } else if let ObjectKind::TypedArray { length, .. } = &self.kind {
            keys.extend((0..(*length).min(MAX_DENSE_SIZE)).map(|index| index.to_string()));
        }
        keys.extend(self.properties.keys());
        keys
    }

    /// Own enumerable string keys, used by `for-in`. Array index slots are
    /// enumerable; ordinary properties honor their `enumerable` attribute.
    #[must_use]
    pub fn enumerable_own_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        if let ObjectKind::Array { elements, .. } = &self.kind {
            keys.extend(
                elements
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| value.is_some())
                    .map(|(index, _)| index.to_string()),
            );
        } else if let ObjectKind::PrimitiveWrapper(PrimitiveValue::String(value)) = &self.kind {
            keys.extend((0..value.encode_utf16().count()).map(|index| index.to_string()));
        } else if let ObjectKind::TypedArray { length, .. } = &self.kind {
            keys.extend((0..(*length).min(MAX_DENSE_SIZE)).map(|index| index.to_string()));
        }
        keys.extend(self.properties.enumerable_keys());
        keys
    }
}

impl Trace for JsObject {
    fn trace(&self, tracer: &mut Tracer<'_>) {
        if let Some(prototype) = self.prototype {
            tracer.mark_object(prototype);
        }
        match &self.kind {
            ObjectKind::Array { elements, .. } => {
                for descriptor in elements.iter().flatten() {
                    descriptor.trace(tracer);
                }
            }
            ObjectKind::Ordinary
            | ObjectKind::PrimitiveWrapper(_)
            | ObjectKind::RegExp { .. }
            | ObjectKind::ArrayBuffer { .. }
            | ObjectKind::DataView { .. }
            | ObjectKind::TypedArray { .. }
            | ObjectKind::Promise { .. } => {}
            ObjectKind::Proxy { record } => {
                record.target.trace(tracer);
                record.handler.trace(tracer);
            }
            ObjectKind::Iterator { record } => {
                // Trace the backing iterable/iterator so GC doesn't collect it.
                match &record.kind {
                    IteratorKind::Array { object, .. } => object.trace(tracer),
                    IteratorKind::Js {
                        iterator,
                        next_method,
                    } => {
                        iterator.trace(tracer);
                        if let Some(next_method) = next_method {
                            next_method.trace(tracer);
                        }
                    }
                    IteratorKind::String { .. } => {}
                }
            }
            ObjectKind::Generator { record } => {
                tracer.mark_function(record.function);
                if let Some(environment) = record.environment {
                    tracer.mark_environment(environment);
                }
                record.this_value.trace(tracer);
                for argument in &record.arguments {
                    argument.trace(tracer);
                }
                for value in &record.stack {
                    value.trace(tracer);
                }
                for value in &record.delegate_values {
                    value.trace(tracer);
                }
                if let Some(value) = &record.delegate_iterator {
                    value.trace(tracer);
                }
                if let Some(value) = &record.delegate_return {
                    value.trace(tracer);
                }
            }
        }
        self.properties.trace(tracer);
        for (_, descriptor) in &self.symbol_properties {
            descriptor.trace(tracer);
        }
    }
}

impl JsObject {
    #[must_use]
    pub(crate) fn estimated_bytes(&self) -> usize {
        let kind_bytes = match &self.kind {
            ObjectKind::Ordinary => 0,
            ObjectKind::Array { elements, .. } => elements
                .iter()
                .flatten()
                .map(PropertyDescriptor::estimated_bytes)
                .sum::<usize>()
                .saturating_add(
                    elements.capacity() * std::mem::size_of::<Option<PropertyDescriptor>>(),
                ),
            ObjectKind::PrimitiveWrapper(PrimitiveValue::String(value)) => value.len(),
            ObjectKind::PrimitiveWrapper(_) => std::mem::size_of::<PrimitiveValue>(),
            ObjectKind::RegExp { pattern, flags } => pattern.len().saturating_add(flags.len()),
            ObjectKind::ArrayBuffer { .. } | ObjectKind::DataView { .. } => {
                std::mem::size_of::<usize>()
            }
            ObjectKind::TypedArray { name, .. } => name.len().saturating_add(16),
            ObjectKind::Iterator { .. } => std::mem::size_of::<IteratorRecord>(),
            ObjectKind::Generator { record } => std::mem::size_of::<GeneratorRecord>()
                .saturating_add(record.arguments.capacity() * std::mem::size_of::<JsValue>())
                .saturating_add(record.stack.capacity() * std::mem::size_of::<JsValue>())
                .saturating_add(record.delegate_values.capacity() * std::mem::size_of::<JsValue>()),
            ObjectKind::Promise { .. } => std::mem::size_of::<PromiseId>(),
            ObjectKind::Proxy { .. } => std::mem::size_of::<ProxyRecord>(),
        };
        std::mem::size_of::<Self>()
            .saturating_add(kind_bytes)
            .saturating_add(self.properties.estimated_bytes())
            .saturating_add(
                self.symbol_properties
                    .iter()
                    .map(|(_, descriptor)| descriptor.estimated_bytes())
                    .sum::<usize>(),
            )
    }
}
pub(crate) fn array_index(name: &str) -> Option<usize> {
    if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let index = name.parse::<u32>().ok()?;
    if index == u32::MAX {
        return None;
    }
    (index.to_string() == name)
        .then_some(index)
        .map(|index| index as usize)
}
