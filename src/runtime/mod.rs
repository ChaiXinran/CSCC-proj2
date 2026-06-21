//! Native runtime data model.

mod coercion;
mod context;
mod environment;
mod function;
mod gc;
mod heap;
mod object;
mod property;
mod property_map;
mod value;

pub use coercion::PreferredType;
pub use context::{Intrinsics, NativeContext, to_property_key};
pub use environment::{Binding, Environment, EnvironmentId};
pub use function::{
    BoundFunction, BuiltinFunction, BuiltinId, FunctionId, JsFunction, NativeCall, NativeConstruct,
};
pub use gc::{CollectionStats, Collector};
pub use heap::Heap;
pub use object::{JsObject, ObjectId, ObjectKind, PrimitiveValue};
pub use property::{PropertyDescriptor, PropertyDescriptorUpdate, PropertyKind};
pub use property_map::{PropertyEntry, PropertyMap};
pub use value::{JsValue, NativeErrorKind, NativeErrorValue};
