//! Object and environment allocation.

use super::{Environment, EnvironmentId, FunctionId, JsFunction, JsObject, ObjectId};

/// Arena owning native runtime objects and lexical environments.
#[derive(Debug, Default)]
pub struct Heap {
    objects: Vec<Option<JsObject>>,
    environments: Vec<Option<Environment>>,
    functions: Vec<Option<JsFunction>>,
}

impl Heap {
    pub fn allocate_object(&mut self, object: JsObject) -> Option<ObjectId> {
        let id = ObjectId(u32::try_from(self.objects.len()).ok()?);
        self.objects.push(Some(object));
        Some(id)
    }

    #[must_use]
    pub fn object(&self, id: ObjectId) -> Option<&JsObject> {
        self.objects.get(id.0 as usize)?.as_ref()
    }

    pub fn object_mut(&mut self, id: ObjectId) -> Option<&mut JsObject> {
        self.objects.get_mut(id.0 as usize)?.as_mut()
    }

    pub fn allocate_environment(&mut self, environment: Environment) -> Option<EnvironmentId> {
        let id = EnvironmentId(u32::try_from(self.environments.len()).ok()?);
        self.environments.push(Some(environment));
        Some(id)
    }

    #[must_use]
    pub fn environment(&self, id: EnvironmentId) -> Option<&Environment> {
        self.environments.get(id.0 as usize)?.as_ref()
    }

    pub fn environment_mut(&mut self, id: EnvironmentId) -> Option<&mut Environment> {
        self.environments.get_mut(id.0 as usize)?.as_mut()
    }

    pub fn allocate_function(&mut self, function: JsFunction) -> Option<FunctionId> {
        let id = FunctionId(u32::try_from(self.functions.len()).ok()?);
        self.functions.push(Some(function));
        Some(id)
    }

    #[must_use]
    pub fn function(&self, id: FunctionId) -> Option<&JsFunction> {
        self.functions.get(id.0 as usize)?.as_ref()
    }

    #[must_use]
    pub fn object_count(&self) -> usize {
        self.objects.iter().flatten().count()
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::{Heap, JsObject, JsValue, PropertyDescriptor};

    #[test]
    fn allocates_and_reads_objects() {
        let mut heap = Heap::default();
        let mut object = JsObject::default();
        object.define_property("answer", PropertyDescriptor::data(JsValue::Number(42.0)));
        let id = heap.allocate_object(object).unwrap();

        assert_eq!(
            heap.object(id)
                .unwrap()
                .own_property("answer")
                .unwrap()
                .value_cloned(),
            Some(JsValue::Number(42.0))
        );
    }
}
