//! Native runtime data model.

pub mod abstract_ops;
mod agent;
pub mod bigint;
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
mod private;
mod property;
mod property_map;
pub mod realm;
mod shape;
mod string_value;
mod symbol;
mod value;

pub(crate) use agent::AgentManager;
pub use bigint::BigIntValue;
pub use buffer::{
    ArrayBufferId, ArrayBufferRecord, DataViewId, DataViewRecord, TypedArrayElementKind,
    TypedArrayView, TypedArrayViewId,
};
pub use coercion::PreferredType;
pub(crate) use context::DisposableStackEntry;
pub use context::{
    ExecutionBudget, Intrinsics, NativeContext, checked_array_length, checked_string_repeat_len,
    checked_utf16_allocation, to_property_key,
};
pub use environment::{Binding, Environment, EnvironmentId};
pub use function::{
    BoundFunction, BuiltinFunction, BuiltinId, FunctionId, JsFunction, NativeCall, NativeConstruct,
};
pub use gc::{
    CallFrameRoots, CollectionStats, Collector, GcMetrics, HeapStats, RootSet, Trace, Tracer,
};
pub(crate) use gc::{HeapMarks, RootSink};
pub use heap::Heap;
pub(crate) use iterator::IteratorKind;
pub use iterator::{IteratorMode, IteratorRecord};
pub use job::{
    Job, JobQueue, NativeJob, PromiseCallbackJob, PromiseId, PromiseJob, PromiseReaction,
    PromiseRecord, PromiseState, PromiseThenReaction, ResolveThenableJob,
};
pub use module::{
    DynamicImportOutcome, DynamicImportRequest, ModuleEvaluationPromise, ModuleEvaluationState,
    ModuleExportBinding, ModuleId, ModuleImportBinding, ModuleLoadState, ModuleRecord,
    ModuleRegistry, ModuleStatus, normalize_module_path, resolve_module_specifier,
};
pub(crate) use object::array_index;
pub use object::{
    GeneratorRecord, GeneratorState, JsObject, ObjectId, ObjectKind, PrimitiveValue, PropertyKey,
    ProxyRecord,
};
pub use private::{PrivateBrandId, PrivateSlot};
pub use property::{PropertyDescriptor, PropertyDescriptorUpdate, PropertyKind};
pub use property_map::{
    PropertyEntry, PropertyMap, PropertyMutation, PropertyName, PropertySlotId,
};
pub use shape::{
    DICTIONARY_SHAPE, PropertyAttributes, PropertyCacheMetrics, PropertyKindTag, ROOT_SHAPE,
    ShapeId, ShapeMode, ShapeRecord, ShapeTable,
};
pub use string_value::JsString;
pub use symbol::{Symbol, SymbolId, SymbolRegistry, WellKnownSymbols};
pub use value::{JsValue, NativeErrorKind, NativeErrorValue};
