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
mod symbol;
mod value;

pub use coercion::PreferredType;
pub use context::{
    ExecutionBudget, Intrinsics, NativeContext, checked_array_length, checked_string_repeat_len,
    checked_utf16_allocation, to_property_key,
};
pub use environment::{Binding, Environment, EnvironmentId};
pub use function::{
    BoundFunction, BuiltinFunction, BuiltinId, FunctionId, JsFunction, NativeCall, NativeConstruct,
};
pub use gc::{CallFrameRoots, CollectionStats, Collector, HeapStats, RootSet, Trace, Tracer};
pub use heap::Heap;
pub use object::{JsObject, ObjectId, ObjectKind, PrimitiveValue};
pub use property::{PropertyDescriptor, PropertyDescriptorUpdate, PropertyKind};
pub use property_map::{PropertyEntry, PropertyMap};
pub use symbol::{Symbol, SymbolId, SymbolRegistry, WellKnownSymbols};
pub use value::{JsValue, NativeErrorKind, NativeErrorValue};
