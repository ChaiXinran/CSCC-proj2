//! Native runtime data model.

mod context;
mod environment;
mod function;
mod gc;
mod heap;
mod object;
mod property;
mod property_map;
mod value;

pub use context::{Intrinsics, NativeContext, to_property_key};
pub use environment::{Binding, Environment, EnvironmentId};
pub use function::{FunctionId, JsFunction};
pub use gc::{CollectionStats, Collector};
pub use heap::Heap;
pub use object::{JsObject, ObjectId, ObjectKind};
pub use property::{PropertyDescriptor, PropertyDescriptorUpdate, PropertyKind};
pub use property_map::{PropertyEntry, PropertyMap};
pub use value::{JsValue, NativeErrorKind, NativeErrorValue, NativeFunction};
