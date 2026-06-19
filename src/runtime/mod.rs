//! Native runtime data model.

mod environment;
mod gc;
mod heap;
mod object;
mod property;
mod value;

pub use environment::{Binding, Environment, EnvironmentId};
pub use gc::{CollectionStats, Collector};
pub use heap::Heap;
pub use object::{JsObject, ObjectId};
pub use property::PropertyDescriptor;
pub use value::JsValue;
