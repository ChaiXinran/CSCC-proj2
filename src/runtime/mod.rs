//! Native runtime data model.

mod buffer;
mod coercion;
mod context;
mod environment;
mod function;
mod gc;
mod heap;
mod iterator;
mod job;
mod module;
mod object;
mod property;
mod property_map;
mod symbol;
mod value;

pub use buffer::{
    ArrayBufferId, ArrayBufferRecord, DataViewId, DataViewRecord, TypedArrayElementKind,
    TypedArrayView, TypedArrayViewId,
};
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
pub use iterator::IteratorRecord;
pub use job::{
    Job, JobQueue, NativeJob, PromiseId, PromiseJob, PromiseReaction, PromiseRecord, PromiseState,
};
pub use module::{
    ModuleExportBinding, ModuleId, ModuleImportBinding, ModuleRecord, ModuleRegistry, ModuleStatus,
    normalize_module_path, resolve_module_specifier,
};
pub use object::{JsObject, ObjectId, ObjectKind, PrimitiveValue};
pub use property::{PropertyDescriptor, PropertyDescriptorUpdate, PropertyKind};
pub use property_map::{PropertyEntry, PropertyMap};
pub use symbol::{Symbol, SymbolId, SymbolRegistry, WellKnownSymbols};
pub use value::{JsValue, NativeErrorKind, NativeErrorValue};
