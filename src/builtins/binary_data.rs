//! Binary-data built-ins: ArrayBuffer, DataView, TypedArray constructors, and Intl skeleton.
//!
//! This module intentionally installs honest first-stage shapes: constructors,
//! prototypes, descriptors, and deterministic Intl option objects. Operations
//! that need real typed storage are present only when they can fail explicitly.

use super::{function, install_foundation, install_test262_harness, proxy, string};
use crate::{
    intl::{
        CollatorRecord, DateTimeFieldStyle, DateTimeFormatRecord, DateTimeStyle, HourCycle,
        IntlDataProvider, IntlObjectData, IntlService, LocaleOptions, MinimalIntlProvider,
        NumberFormatRecord, NumberValue, TimeZoneNameStyle, canonicalize_language_tag,
        format_number, resolve_locale, unicode_extension_value,
    },
    runtime::{
        ArrayBufferId, BigIntValue, DataViewId, IteratorMode, JsObject, JsValue, NativeCall,
        NativeContext, ObjectId, ObjectKind, PreferredType, PrimitiveValue, PropertyDescriptor,
        PropertyKey, PropertyKind, SymbolId, TypedArrayElementKind, TypedArrayViewId, abstract_ops,
        bigint,
    },
    vm::{Vm, VmError},
};

const ARRAY_BUFFER_MARKER: &str = "__agentjs_array_buffer__";
const ARRAY_BUFFER_BYTE_LENGTH: &str = "__agentjs_array_buffer_byte_length__";
const ARRAY_BUFFER_DETACHED: &str = "__agentjs_array_buffer_detached__";
const TYPED_ARRAY_MARKER: &str = "__agentjs_typed_array__";
const TYPED_ARRAY_NAME: &str = "__agentjs_typed_array_name__";
const TYPED_ARRAY_LENGTH: &str = "__agentjs_typed_array_length__";
const TYPED_ARRAY_BYTE_LENGTH: &str = "__agentjs_typed_array_byte_length__";
const TYPED_ARRAY_BYTE_OFFSET: &str = "__agentjs_typed_array_byte_offset__";
const TYPED_ARRAY_BUFFER: &str = "__agentjs_typed_array_buffer__";
const DATA_VIEW_MARKER: &str = "__agentjs_data_view__";
const DATA_VIEW_BUFFER: &str = "__agentjs_data_view_buffer__";
const DATA_VIEW_BYTE_LENGTH: &str = "__agentjs_data_view_byte_length__";
const DATA_VIEW_BYTE_OFFSET: &str = "__agentjs_data_view_byte_offset__";
const INTL_KIND: &str = "__agentjs_intl_kind__";
const NUMBER_FORMAT_BOUND_FORMAT: &str = "__agentjs_number_format_bound_format__";
const INTL_FALLBACK_SYMBOL: &str = "__agentjs_intl_fallback_symbol__";
const MAX_SKELETON_BUFFER_BYTES: usize = 1 << 24;
const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

#[derive(Clone, Copy)]
struct TypedArrayIntrinsic {
    constructor_object: ObjectId,
    prototype: ObjectId,
}

pub(super) fn install(context: &mut NativeContext) -> Result<(), VmError> {
    install_array_buffer(context)?;
    install_shared_array_buffer(context)?;
    install_data_view(context)?;
    let typed_array_intrinsic = install_typed_array_intrinsic(context)?;
    for (name, bytes_per_element) in [
        ("Int8Array", 1),
        ("Uint8Array", 1),
        ("Uint8ClampedArray", 1),
        ("Int16Array", 2),
        ("Uint16Array", 2),
        ("Int32Array", 4),
        ("Uint32Array", 4),
        ("Float32Array", 4),
        ("Float64Array", 8),
        ("BigInt64Array", 8),
        ("BigUint64Array", 8),
    ] {
        install_typed_array_constructor(context, typed_array_intrinsic, name, bytes_per_element)?;
    }
    install_atomics(context)?;
    install_intl(context)?;
    Ok(())
}

fn method_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, true)
}

fn constant_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, false)
}

fn readonly_configurable_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, true)
}

fn hidden_slot_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, false)
}

fn define_method(
    context: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<(), VmError> {
    let function = context.register_builtin(name, length, call, None)?;
    context.define_own_property(target, name.into(), method_descriptor(function))?;
    Ok(())
}

fn declare_standard_global(
    context: &mut NativeContext,
    name: &'static str,
    value: JsValue,
) -> Result<(), VmError> {
    context.declare_global(name, value.clone());
    context.define_own_property(
        context.global_object(),
        name.into(),
        method_descriptor(value),
    )?;
    Ok(())
}

fn new_ordinary_object(
    context: &mut NativeContext,
    prototype: Option<ObjectId>,
) -> Result<ObjectId, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = prototype;
    context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))
}

fn define_hidden(
    context: &mut NativeContext,
    object: ObjectId,
    name: &'static str,
    value: JsValue,
) -> Result<(), VmError> {
    context.define_own_property(object, name.into(), hidden_slot_descriptor(value))?;
    Ok(())
}

fn own_data_value(context: &NativeContext, object: ObjectId, key: &str) -> Option<JsValue> {
    context
        .get_own_property_descriptor(object, key)
        .and_then(|descriptor| match descriptor.kind {
            PropertyKind::Data { value, .. } => Some(value),
            PropertyKind::Accessor { .. } => None,
        })
}

fn own_bool(context: &NativeContext, object: ObjectId, key: &str) -> Option<bool> {
    match own_data_value(context, object, key)? {
        JsValue::Boolean(value) => Some(value),
        _ => None,
    }
}

fn set_object_kind(
    context: &mut NativeContext,
    object: ObjectId,
    kind: ObjectKind,
) -> Result<(), VmError> {
    let object = context
        .heap_mut()
        .object_mut(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    object.kind = kind;
    Ok(())
}

fn array_buffer_id_from_object(
    context: &NativeContext,
    object: ObjectId,
) -> Result<ArrayBufferId, VmError> {
    context
        .array_buffer_id_for_object(object)
        .ok_or_else(|| VmError::type_error("receiver is not an ArrayBuffer"))
}

fn data_view_id_from_object(
    context: &NativeContext,
    object: ObjectId,
) -> Result<DataViewId, VmError> {
    context
        .data_view_id_for_object(object)
        .ok_or_else(|| VmError::type_error("receiver is not a DataView"))
}

fn typed_array_view_id_from_object(
    context: &NativeContext,
    object: ObjectId,
) -> Result<TypedArrayViewId, VmError> {
    context
        .typed_array_indexed_view(object)
        .map(|(view, _)| view)
        .ok_or_else(|| VmError::type_error("receiver is not a TypedArray"))
}

fn typed_array_name_from_object(
    context: &NativeContext,
    object: ObjectId,
) -> Result<String, VmError> {
    context
        .typed_array_name_for_object(object)
        .map(str::to_owned)
        .ok_or_else(|| VmError::type_error("receiver is not a TypedArray"))
}

fn is_typed_array_object(context: &NativeContext, object: ObjectId) -> bool {
    context.typed_array_indexed_view(object).is_some()
        || own_bool(context, object, TYPED_ARRAY_MARKER).unwrap_or(false)
}

fn is_data_view_object(context: &NativeContext, object: ObjectId) -> bool {
    context.data_view_id_for_object(object).is_some()
        || own_bool(context, object, DATA_VIEW_MARKER).unwrap_or(false)
}

fn object_from_this(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<ObjectId, VmError> {
    context.require_object(this_value, label)
}

fn to_index(vm: &mut Vm, context: &mut NativeContext, value: JsValue) -> Result<usize, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(0);
    }
    let number = vm.to_number(value, context)?;
    if number.is_infinite() {
        return Err(VmError::range("invalid buffer length"));
    }
    let integer = if number.is_nan() || number == 0.0 {
        0.0
    } else {
        number.trunc()
    };
    if integer < 0.0 {
        return Err(VmError::range("invalid buffer length"));
    }
    if integer > MAX_SAFE_INTEGER {
        return Err(VmError::range("invalid buffer length"));
    }
    Ok(integer as usize)
}

fn to_length(vm: &mut Vm, context: &mut NativeContext, value: JsValue) -> Result<usize, VmError> {
    let number = vm.to_number(value, context)?;
    Ok(abstract_ops::to_length(number))
}

fn argument_integer(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
    default: f64,
) -> Result<f64, VmError> {
    match arguments.get(index) {
        None | Some(JsValue::Undefined) => Ok(default),
        Some(value) => {
            let number = vm.to_number(value.clone(), context)?;
            if number.is_nan() || number == 0.0 {
                Ok(0.0)
            } else {
                Ok(number.trunc())
            }
        }
    }
}

fn normalize_relative_index(raw: f64, length: usize) -> usize {
    if raw < 0.0 {
        length.saturating_sub((-raw) as usize)
    } else {
        (raw as usize).min(length)
    }
}

fn typed_array_kind(name: &str) -> Result<TypedArrayElementKind, VmError> {
    match name {
        "Int8Array" => Ok(TypedArrayElementKind::Int8),
        "Uint8Array" => Ok(TypedArrayElementKind::Uint8),
        "Uint8ClampedArray" => Ok(TypedArrayElementKind::Uint8Clamped),
        "Int16Array" => Ok(TypedArrayElementKind::Int16),
        "Uint16Array" => Ok(TypedArrayElementKind::Uint16),
        "Int32Array" => Ok(TypedArrayElementKind::Int32),
        "Uint32Array" => Ok(TypedArrayElementKind::Uint32),
        "Float32Array" => Ok(TypedArrayElementKind::Float32),
        "Float64Array" => Ok(TypedArrayElementKind::Float64),
        "BigInt64Array" => Ok(TypedArrayElementKind::BigInt64),
        "BigUint64Array" => Ok(TypedArrayElementKind::BigUint64),
        _ => Err(VmError::type_error("unknown TypedArray constructor")),
    }
}

fn install_array_buffer(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor = context.register_builtin(
        "ArrayBuffer",
        1,
        array_buffer_call,
        Some(array_buffer_construct),
    )?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("ArrayBuffer constructor object missing"))?;

    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    let byte_length_getter =
        context.register_builtin("get byteLength", 0, array_buffer_byte_length_get, None)?;
    let max_byte_length_getter = context.register_builtin(
        "get maxByteLength",
        0,
        array_buffer_max_byte_length_get,
        None,
    )?;
    let resizable_getter =
        context.register_builtin("get resizable", 0, array_buffer_resizable_get, None)?;
    let detached_getter =
        context.register_builtin("get detached", 0, array_buffer_detached_get, None)?;
    for (name, getter) in [
        ("byteLength", byte_length_getter),
        ("maxByteLength", max_byte_length_getter),
        ("resizable", resizable_getter),
        ("detached", detached_getter),
    ] {
        context.define_own_property(
            prototype,
            name.into(),
            PropertyDescriptor::accessor(Some(getter), None, false, true),
        )?;
    }

    define_method(
        context,
        constructor_object,
        "isView",
        1,
        array_buffer_is_view,
    )?;
    define_method(context, prototype, "slice", 2, array_buffer_slice)?;
    define_method(context, prototype, "resize", 1, array_buffer_resize)?;
    define_method(context, prototype, "transfer", 0, array_buffer_transfer)?;
    define_method(
        context,
        prototype,
        "transferToFixedLength",
        0,
        array_buffer_transfer_to_fixed_length,
    )?;
    define_method(
        context,
        prototype,
        "sliceToImmutable",
        2,
        array_buffer_slice_to_immutable,
    )?;
    define_method(
        context,
        prototype,
        "transferToImmutable",
        0,
        array_buffer_transfer_to_immutable,
    )?;

    let immutable_getter =
        context.register_builtin("get immutable", 0, array_buffer_immutable_get, None)?;
    context.define_own_property(
        prototype,
        "immutable".into(),
        PropertyDescriptor::accessor(Some(immutable_getter), None, false, true),
    )?;

    let species_getter =
        context.register_builtin("get [Symbol.species]", 0, array_buffer_species_get, None)?;
    let species = context.well_known_symbols().species;
    context.define_symbol_own_property(
        constructor_object,
        species,
        PropertyDescriptor::accessor(Some(species_getter), None, false, true),
    )?;
    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        to_string_tag,
        readonly_configurable_descriptor(JsValue::String("ArrayBuffer".into())),
    )?;
    declare_standard_global(context, "ArrayBuffer", constructor)?;
    Ok(())
}

fn install_shared_array_buffer(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor = context.register_builtin(
        "SharedArrayBuffer",
        1,
        shared_array_buffer_call,
        Some(shared_array_buffer_construct),
    )?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("SharedArrayBuffer constructor object missing"))?;

    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    let byte_length_getter = context.register_builtin(
        "get byteLength",
        0,
        shared_array_buffer_byte_length_get,
        None,
    )?;
    let max_byte_length_getter = context.register_builtin(
        "get maxByteLength",
        0,
        shared_array_buffer_max_byte_length_get,
        None,
    )?;
    let growable_getter =
        context.register_builtin("get growable", 0, shared_array_buffer_growable_get, None)?;
    for (name, getter) in [
        ("byteLength", byte_length_getter),
        ("maxByteLength", max_byte_length_getter),
        ("growable", growable_getter),
    ] {
        context.define_own_property(
            prototype,
            name.into(),
            PropertyDescriptor::accessor(Some(getter), None, false, true),
        )?;
    }
    define_method(context, prototype, "grow", 1, shared_array_buffer_grow)?;
    define_method(context, prototype, "slice", 2, shared_array_buffer_slice)?;
    let species_getter = context.register_builtin(
        "get [Symbol.species]",
        0,
        shared_array_buffer_species_get,
        None,
    )?;
    let species = context.well_known_symbols().species;
    context.define_symbol_own_property(
        constructor_object,
        species,
        PropertyDescriptor::accessor(Some(species_getter), None, false, true),
    )?;
    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        to_string_tag,
        readonly_configurable_descriptor(JsValue::String("SharedArrayBuffer".into())),
    )?;
    declare_standard_global(context, "SharedArrayBuffer", constructor)?;
    Ok(())
}

fn shared_array_buffer_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "SharedArrayBuffer constructor requires 'new'",
    ))
}

fn shared_array_buffer_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let byte_length = to_index(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (max_byte_length, growable) =
        array_buffer_options(vm, context, arguments.get(1).cloned(), byte_length)?;
    let prototype =
        buffer_prototype_from_constructor(vm, context, new_target, "SharedArrayBuffer")?;
    let buffer =
        context.create_shared_array_buffer_with_options(byte_length, max_byte_length, growable)?;
    create_array_buffer_object_with_id(context, buffer, prototype)
}

fn shared_array_buffer_id_from_this(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<ArrayBufferId, VmError> {
    let object = object_from_this(context, this_value, label)?;
    let buffer = array_buffer_id_from_object(context, object)?;
    if !context.is_shared_array_buffer(buffer)? {
        return Err(VmError::type_error("receiver is not a SharedArrayBuffer"));
    }
    Ok(buffer)
}

fn shared_array_buffer_byte_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer = shared_array_buffer_id_from_this(
        context,
        &this_value,
        "SharedArrayBuffer.prototype.byteLength",
    )?;
    Ok(JsValue::Number(
        context.array_buffer_byte_length(buffer)? as f64
    ))
}

fn shared_array_buffer_max_byte_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer = shared_array_buffer_id_from_this(
        context,
        &this_value,
        "SharedArrayBuffer.prototype.maxByteLength",
    )?;
    Ok(JsValue::Number(
        context.array_buffer_max_byte_length(buffer)? as f64,
    ))
}

fn shared_array_buffer_growable_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer = shared_array_buffer_id_from_this(
        context,
        &this_value,
        "SharedArrayBuffer.prototype.growable",
    )?;
    Ok(JsValue::Boolean(context.is_array_buffer_resizable(buffer)?))
}

fn shared_array_buffer_grow(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer =
        shared_array_buffer_id_from_this(context, &this_value, "SharedArrayBuffer.prototype.grow")?;
    if !context.is_array_buffer_resizable(buffer)? {
        return Err(VmError::type_error("SharedArrayBuffer is not growable"));
    }
    let new_byte_length = to_index(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    if new_byte_length < context.array_buffer_byte_length(buffer)? {
        return Err(VmError::range("SharedArrayBuffer cannot shrink"));
    }
    context.resize_array_buffer(buffer, new_byte_length)?;
    Ok(JsValue::Undefined)
}

fn shared_array_buffer_slice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_slice_with_species(vm, context, this_value, arguments, true)
}

#[derive(Clone, Copy)]
enum AtomicRmwOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
}

fn install_atomics(context: &mut NativeContext) -> Result<(), VmError> {
    let atomics = new_ordinary_object(context, context.object_prototype())?;
    for (name, length, call) in [
        ("add", 3, atomics_add as NativeCall),
        ("and", 3, atomics_and as NativeCall),
        ("compareExchange", 4, atomics_compare_exchange as NativeCall),
        ("exchange", 3, atomics_exchange as NativeCall),
        ("isLockFree", 1, atomics_is_lock_free as NativeCall),
        ("load", 2, atomics_load as NativeCall),
        ("notify", 3, atomics_notify as NativeCall),
        ("or", 3, atomics_or as NativeCall),
        ("pause", 0, atomics_pause as NativeCall),
        ("store", 3, atomics_store as NativeCall),
        ("sub", 3, atomics_sub as NativeCall),
        ("wait", 4, atomics_wait as NativeCall),
        ("waitAsync", 4, atomics_wait_async as NativeCall),
        ("xor", 3, atomics_xor as NativeCall),
    ] {
        define_method(context, atomics, name, length, call)?;
    }
    let tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        atomics,
        tag,
        readonly_configurable_descriptor(JsValue::String("Atomics".into())),
    )?;
    declare_standard_global(context, "Atomics", JsValue::Object(atomics))
}

fn atomic_typed_array(
    context: &mut NativeContext,
    value: &JsValue,
    only_waitable: bool,
    require_shared: bool,
    label: &str,
) -> Result<
    (
        TypedArrayViewId,
        usize,
        TypedArrayElementKind,
        ArrayBufferId,
    ),
    VmError,
> {
    let object = context.require_object(value, label)?;
    let (view, length) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("value is not a TypedArray"))?;
    let view_record = context
        .typed_array_view(view)
        .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?;
    let kind = view_record.element_kind;
    let allowed = if only_waitable {
        matches!(
            kind,
            TypedArrayElementKind::Int32 | TypedArrayElementKind::BigInt64
        )
    } else {
        matches!(
            kind,
            TypedArrayElementKind::Int8
                | TypedArrayElementKind::Uint8
                | TypedArrayElementKind::Int16
                | TypedArrayElementKind::Uint16
                | TypedArrayElementKind::Int32
                | TypedArrayElementKind::Uint32
                | TypedArrayElementKind::BigInt64
                | TypedArrayElementKind::BigUint64
        )
    };
    if !allowed {
        return Err(VmError::type_error(
            "TypedArray type is not atomics-compatible",
        ));
    }
    let buffer = view_record.buffer;
    if require_shared && !context.is_shared_array_buffer(buffer)? {
        return Err(VmError::type_error(
            "Atomics operation requires SharedArrayBuffer",
        ));
    }
    context.validate_typed_array_view(view)?;
    Ok((view, length, kind, buffer))
}

fn require_atomic_writable(context: &NativeContext, buffer: ArrayBufferId) -> Result<(), VmError> {
    if context.is_array_buffer_immutable(buffer)? {
        return Err(VmError::type_error(
            "Atomics operation cannot modify an immutable buffer",
        ));
    }
    Ok(())
}

fn atomic_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    length: usize,
) -> Result<usize, VmError> {
    let number = vm.to_number(value, context)?;
    let integer_number = if number.is_nan() || number == 0.0 {
        0.0
    } else {
        number.trunc()
    };
    if integer_number.is_infinite() || integer_number < 0.0 {
        return Err(VmError::range("Atomics index is out of range"));
    }
    let integer = integer_number as usize;
    if integer >= length {
        return Err(VmError::range("Atomics index is out of range"));
    }
    Ok(integer)
}

fn atomic_integer_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<f64, VmError> {
    let number = vm.to_number(value, context)?;
    Ok(if number.is_nan() || number == 0.0 {
        0.0
    } else {
        number.trunc()
    })
}

fn atomic_number_bits(kind: TypedArrayElementKind, value: f64) -> u64 {
    let bits = match kind {
        TypedArrayElementKind::Int8 | TypedArrayElementKind::Uint8 => 8,
        TypedArrayElementKind::Int16 | TypedArrayElementKind::Uint16 => 16,
        TypedArrayElementKind::Int32 | TypedArrayElementKind::Uint32 => 32,
        _ => 0,
    };
    if bits == 0 || !value.is_finite() || value == 0.0 {
        return 0;
    }
    value.trunc().rem_euclid(2_f64.powi(bits)) as u64
}

fn atomic_normalized_number(kind: TypedArrayElementKind, value: f64) -> JsValue {
    let bits = atomic_number_bits(kind, value);
    let result = match kind {
        TypedArrayElementKind::Int8 => (bits as u8 as i8) as f64,
        TypedArrayElementKind::Uint8 => bits as u8 as f64,
        TypedArrayElementKind::Int16 => (bits as u16 as i16) as f64,
        TypedArrayElementKind::Uint16 => bits as u16 as f64,
        TypedArrayElementKind::Int32 => (bits as u32 as i32) as f64,
        TypedArrayElementKind::Uint32 => bits as u32 as f64,
        _ => value,
    };
    JsValue::Number(result)
}

fn atomic_bigint_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<BigIntValue, VmError> {
    to_bigint_for_data_view(vm, context, value)
}

fn atomic_normalized_bigint(kind: TypedArrayElementKind, value: &BigIntValue) -> BigIntValue {
    if matches!(kind, TypedArrayElementKind::BigInt64) {
        bigint::as_int_n(64, value)
    } else {
        bigint::as_uint_n(64, value)
    }
}

fn atomic_rmw(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    op: AtomicRmwOp,
) -> Result<JsValue, VmError> {
    let (view, length, kind, buffer) = atomic_typed_array(
        context,
        arguments.first().unwrap_or(&JsValue::Undefined),
        false,
        false,
        "Atomics read-modify-write",
    )?;
    require_atomic_writable(context, buffer)?;
    let index = atomic_index(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        length,
    )?;
    let old = context.typed_array_load_element(view, index)?;
    if kind.is_bigint() {
        let operand = atomic_normalized_bigint(
            kind,
            &atomic_bigint_value(
                vm,
                context,
                arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
            )?,
        );
        let old_big = match &old {
            JsValue::BigInt(value) => value,
            _ => return Err(VmError::runtime("BigInt TypedArray returned non-BigInt")),
        };
        let new_value = match op {
            AtomicRmwOp::Add => bigint::add(old_big, &operand),
            AtomicRmwOp::Sub => bigint::sub(old_big, &operand),
            AtomicRmwOp::And => bigint::bitand(old_big, &operand),
            AtomicRmwOp::Or => bigint::bitor(old_big, &operand),
            AtomicRmwOp::Xor => bigint::bitxor(old_big, &operand),
        };
        context.typed_array_store_element(view, index, JsValue::BigInt(new_value))?;
    } else {
        let operand = atomic_integer_value(
            vm,
            context,
            arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
        )?;
        let old_number = old
            .to_number()
            .ok_or_else(|| VmError::runtime("numeric TypedArray returned non-number"))?;
        let new_value = match op {
            AtomicRmwOp::Add => JsValue::Number(old_number + operand),
            AtomicRmwOp::Sub => JsValue::Number(old_number - operand),
            AtomicRmwOp::And | AtomicRmwOp::Or | AtomicRmwOp::Xor => {
                let left = atomic_number_bits(kind, old_number);
                let right = atomic_number_bits(kind, operand);
                let bits = match op {
                    AtomicRmwOp::And => left & right,
                    AtomicRmwOp::Or => left | right,
                    AtomicRmwOp::Xor => left ^ right,
                    _ => unreachable!(),
                };
                atomic_normalized_number(kind, bits as f64)
            }
        };
        context.typed_array_store_element(view, index, new_value)?;
    }
    Ok(old)
}

fn atomics_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    atomic_rmw(vm, context, args, AtomicRmwOp::Add)
}
fn atomics_sub(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    atomic_rmw(vm, context, args, AtomicRmwOp::Sub)
}
fn atomics_and(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    atomic_rmw(vm, context, args, AtomicRmwOp::And)
}
fn atomics_or(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    atomic_rmw(vm, context, args, AtomicRmwOp::Or)
}
fn atomics_xor(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    atomic_rmw(vm, context, args, AtomicRmwOp::Xor)
}

fn atomics_load(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let (view, length, _, _) = atomic_typed_array(
        context,
        args.first().unwrap_or(&JsValue::Undefined),
        false,
        false,
        "Atomics.load",
    )?;
    let index = atomic_index(
        vm,
        context,
        args.get(1).cloned().unwrap_or(JsValue::Undefined),
        length,
    )?;
    context.typed_array_load_element(view, index)
}

fn atomics_store(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let (view, length, kind, buffer) = atomic_typed_array(
        context,
        args.first().unwrap_or(&JsValue::Undefined),
        false,
        false,
        "Atomics.store",
    )?;
    require_atomic_writable(context, buffer)?;
    let index = atomic_index(
        vm,
        context,
        args.get(1).cloned().unwrap_or(JsValue::Undefined),
        length,
    )?;
    let value = if kind.is_bigint() {
        JsValue::BigInt(atomic_bigint_value(
            vm,
            context,
            args.get(2).cloned().unwrap_or(JsValue::Undefined),
        )?)
    } else {
        JsValue::Number(atomic_integer_value(
            vm,
            context,
            args.get(2).cloned().unwrap_or(JsValue::Undefined),
        )?)
    };
    context.typed_array_store_element(view, index, value.clone())?;
    Ok(value)
}

fn atomics_exchange(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let (view, length, kind, buffer) = atomic_typed_array(
        context,
        args.first().unwrap_or(&JsValue::Undefined),
        false,
        false,
        "Atomics.exchange",
    )?;
    require_atomic_writable(context, buffer)?;
    let index = atomic_index(
        vm,
        context,
        args.get(1).cloned().unwrap_or(JsValue::Undefined),
        length,
    )?;
    let old = context.typed_array_load_element(view, index)?;
    let value = if kind.is_bigint() {
        JsValue::BigInt(atomic_bigint_value(
            vm,
            context,
            args.get(2).cloned().unwrap_or(JsValue::Undefined),
        )?)
    } else {
        JsValue::Number(atomic_integer_value(
            vm,
            context,
            args.get(2).cloned().unwrap_or(JsValue::Undefined),
        )?)
    };
    context.typed_array_store_element(view, index, value)?;
    Ok(old)
}

fn atomics_compare_exchange(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let (view, length, kind, buffer) = atomic_typed_array(
        context,
        args.first().unwrap_or(&JsValue::Undefined),
        false,
        false,
        "Atomics.compareExchange",
    )?;
    require_atomic_writable(context, buffer)?;
    let index = atomic_index(
        vm,
        context,
        args.get(1).cloned().unwrap_or(JsValue::Undefined),
        length,
    )?;
    let expected = if kind.is_bigint() {
        JsValue::BigInt(atomic_normalized_bigint(
            kind,
            &atomic_bigint_value(
                vm,
                context,
                args.get(2).cloned().unwrap_or(JsValue::Undefined),
            )?,
        ))
    } else {
        atomic_normalized_number(
            kind,
            atomic_integer_value(
                vm,
                context,
                args.get(2).cloned().unwrap_or(JsValue::Undefined),
            )?,
        )
    };
    let replacement = if kind.is_bigint() {
        JsValue::BigInt(atomic_bigint_value(
            vm,
            context,
            args.get(3).cloned().unwrap_or(JsValue::Undefined),
        )?)
    } else {
        JsValue::Number(atomic_integer_value(
            vm,
            context,
            args.get(3).cloned().unwrap_or(JsValue::Undefined),
        )?)
    };
    let old = context.typed_array_load_element(view, index)?;
    if old.same_value(&expected) {
        context.typed_array_store_element(view, index, replacement)?;
    }
    Ok(old)
}

fn atomics_is_lock_free(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = atomic_integer_value(
        vm,
        context,
        args.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Boolean(matches!(value, 1.0 | 2.0 | 4.0 | 8.0)))
}

fn atomics_count(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<usize, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(usize::MAX);
    }
    let number = vm.to_number(value, context)?;
    if number.is_nan() || number <= 0.0 {
        return Ok(0);
    }
    if number.is_infinite() {
        return Ok(usize::MAX);
    }
    Ok(number.floor() as usize)
}

fn atomics_notify(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let (view, length, _, buffer) = atomic_typed_array(
        context,
        args.first().unwrap_or(&JsValue::Undefined),
        true,
        false,
        "Atomics.notify",
    )?;
    let index = atomic_index(
        vm,
        context,
        args.get(1).cloned().unwrap_or(JsValue::Undefined),
        length,
    )?;
    let count = atomics_count(
        vm,
        context,
        args.get(2).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let _ = view;
    if !context.is_shared_array_buffer(buffer)? {
        return Ok(JsValue::Number(0.0));
    }
    let notified = context.agent_notify(buffer, index, count);
    Ok(JsValue::Number(notified as f64))
}

fn atomics_wait_result(
    vm: &mut Vm,
    context: &mut NativeContext,
    args: &[JsValue],
    async_result: bool,
) -> Result<JsValue, VmError> {
    let (view, length, kind, _) = atomic_typed_array(
        context,
        args.first().unwrap_or(&JsValue::Undefined),
        true,
        true,
        "Atomics.wait",
    )?;
    let index = atomic_index(
        vm,
        context,
        args.get(1).cloned().unwrap_or(JsValue::Undefined),
        length,
    )?;
    let expected = if kind.is_bigint() {
        JsValue::BigInt(atomic_normalized_bigint(
            kind,
            &atomic_bigint_value(
                vm,
                context,
                args.get(2).cloned().unwrap_or(JsValue::Undefined),
            )?,
        ))
    } else {
        atomic_normalized_number(
            kind,
            atomic_integer_value(
                vm,
                context,
                args.get(2).cloned().unwrap_or(JsValue::Undefined),
            )?,
        )
    };
    let raw_timeout = vm.to_number(args.get(3).cloned().unwrap_or(JsValue::Undefined), context)?;
    let timeout = if raw_timeout.is_nan() {
        f64::INFINITY
    } else {
        raw_timeout.max(0.0)
    };
    let current = context.typed_array_load_element(view, index)?;
    let status = if !current.same_value(&expected) {
        "not-equal"
    } else if (context.agent_is_worker() || async_result) && timeout > 0.0 {
        let buffer = context
            .typed_array_view(view)
            .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
            .buffer;
        let marker = context.agent_register_wait(buffer, index, timeout);
        if !async_result {
            return Ok(JsValue::String(marker.into()));
        }
        let promise_value = if timeout.is_infinite() {
            JsValue::String("ok".into())
        } else {
            JsValue::String(marker.into())
        };
        let promise = context.create_promise()?;
        let prototype = context
            .get_global("Promise")
            .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten());
        let promise_object = context.create_promise_object(promise, prototype)?;
        crate::builtins::promise::resolve_promise_id(vm, context, promise, promise_value)?;
        let result = new_ordinary_object(context, context.object_prototype())?;
        context.define_own_property(
            result,
            "async".into(),
            method_descriptor(JsValue::Boolean(true)),
        )?;
        context.define_own_property(result, "value".into(), method_descriptor(promise_object))?;
        return Ok(JsValue::Object(result));
    } else {
        "timed-out"
    };
    if async_result {
        let result = new_ordinary_object(context, context.object_prototype())?;
        context.define_own_property(
            result,
            "async".into(),
            method_descriptor(JsValue::Boolean(false)),
        )?;
        context.define_own_property(
            result,
            "value".into(),
            method_descriptor(JsValue::String(status.into())),
        )?;
        Ok(JsValue::Object(result))
    } else {
        Ok(JsValue::String(status.into()))
    }
}

fn atomics_wait(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    atomics_wait_result(vm, context, args, false)
}

fn atomics_wait_async(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    atomics_wait_result(vm, context, args, true)
}

fn atomics_pause(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some(value) = args.first() else {
        return Ok(JsValue::Undefined);
    };
    if matches!(value, JsValue::Undefined) {
        return Ok(JsValue::Undefined);
    }
    let JsValue::Number(number) = value else {
        return Err(VmError::type_error(
            "Atomics.pause iterationNumber must be a Number",
        ));
    };
    let _ = context;
    let _ = vm;
    if !number.is_finite() || number.fract() != 0.0 || *number < 0.0 {
        return Err(VmError::type_error(
            "Atomics.pause iterationNumber is invalid",
        ));
    }
    Ok(JsValue::Undefined)
}

fn shared_array_buffer_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn array_buffer_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "ArrayBuffer constructor requires 'new'",
    ))
}

fn array_buffer_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let byte_length = to_index(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (max_byte_length, resizable) =
        array_buffer_options(vm, context, arguments.get(1).cloned(), byte_length)?;
    let prototype = buffer_prototype_from_constructor(vm, context, new_target, "ArrayBuffer")?;
    create_array_buffer_object_with_options(
        context,
        byte_length,
        max_byte_length,
        resizable,
        false,
        prototype,
    )
}

fn create_array_buffer_object(
    context: &mut NativeContext,
    byte_length: usize,
    prototype: ObjectId,
) -> Result<JsValue, VmError> {
    create_array_buffer_object_with_options(
        context,
        byte_length,
        byte_length,
        false,
        false,
        prototype,
    )
}

fn create_array_buffer_object_with_options(
    context: &mut NativeContext,
    byte_length: usize,
    max_byte_length: usize,
    resizable: bool,
    immutable: bool,
    prototype: ObjectId,
) -> Result<JsValue, VmError> {
    let buffer = context.create_array_buffer_with_options(
        byte_length,
        max_byte_length,
        resizable,
        immutable,
    )?;
    create_array_buffer_object_with_id(context, buffer, prototype)
}

fn array_buffer_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    options: Option<JsValue>,
    byte_length: usize,
) -> Result<(usize, bool), VmError> {
    let Some(options) = options else {
        return Ok((byte_length, false));
    };
    if matches!(options, JsValue::Undefined) {
        return Ok((byte_length, false));
    }
    let Some(object) = context.value_object(&options) else {
        return Ok((byte_length, false));
    };
    if !context.has_property(object, "maxByteLength")? {
        return Ok((byte_length, false));
    }
    let max_value = vm.get_property_value(options, "maxByteLength", context)?;
    if matches!(max_value, JsValue::Undefined) {
        return Ok((byte_length, false));
    }
    let max_byte_length = to_index(vm, context, max_value)?;
    if max_byte_length < byte_length {
        return Err(VmError::range("ArrayBuffer maxByteLength is too small"));
    }
    Ok((max_byte_length, true))
}

fn buffer_prototype_from_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    new_target: JsValue,
    name: &str,
) -> Result<ObjectId, VmError> {
    let prototype = match vm.get_property_value_catching_from_builtin(
        new_target.clone(),
        "prototype",
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    if let Some(prototype) = context.value_object(&prototype) {
        return Ok(prototype);
    }
    let global = context.global_object_for_callable(&new_target);
    let constructor = match vm.get_property_value_catching_from_builtin(
        JsValue::Object(global),
        name,
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    let prototype =
        match vm.get_property_value_catching_from_builtin(constructor, "prototype", context)? {
            Ok(value) => value,
            Err(error) => return Err(vm.throw_value_from_builtin(error)),
        };
    context
        .value_object(&prototype)
        .ok_or_else(|| VmError::runtime(format!("{name} prototype missing")))
}

fn create_array_buffer_object_with_id(
    context: &mut NativeContext,
    buffer: ArrayBufferId,
    prototype: ObjectId,
) -> Result<JsValue, VmError> {
    let byte_length = context.array_buffer_byte_length(buffer)?;
    let object = new_ordinary_object(context, Some(prototype))?;
    set_object_kind(context, object, ObjectKind::ArrayBuffer { buffer })?;
    define_hidden(context, object, ARRAY_BUFFER_MARKER, JsValue::Boolean(true))?;
    define_hidden(
        context,
        object,
        ARRAY_BUFFER_BYTE_LENGTH,
        JsValue::Number(byte_length as f64),
    )?;
    define_hidden(
        context,
        object,
        ARRAY_BUFFER_DETACHED,
        JsValue::Boolean(false),
    )?;
    Ok(JsValue::Object(object))
}

fn ordinary_array_buffer_id_from_this(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<ArrayBufferId, VmError> {
    let object = object_from_this(context, this_value, label)?;
    let buffer = array_buffer_id_from_object(context, object)?;
    if context.is_shared_array_buffer(buffer)? {
        return Err(VmError::type_error("receiver is not an ArrayBuffer"));
    }
    Ok(buffer)
}

fn array_buffer_byte_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer = ordinary_array_buffer_id_from_this(
        context,
        &this_value,
        "ArrayBuffer.prototype.byteLength",
    )?;
    Ok(JsValue::Number(
        context.array_buffer_byte_length(buffer)? as f64
    ))
}

fn array_buffer_max_byte_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer = ordinary_array_buffer_id_from_this(
        context,
        &this_value,
        "ArrayBuffer.prototype.maxByteLength",
    )?;
    Ok(JsValue::Number(
        context.array_buffer_max_byte_length(buffer)? as f64,
    ))
}

fn array_buffer_resizable_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer = ordinary_array_buffer_id_from_this(
        context,
        &this_value,
        "ArrayBuffer.prototype.resizable",
    )?;
    Ok(JsValue::Boolean(context.is_array_buffer_resizable(buffer)?))
}

fn array_buffer_detached_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer =
        ordinary_array_buffer_id_from_this(context, &this_value, "ArrayBuffer.prototype.detached")?;
    Ok(JsValue::Boolean(context.is_array_buffer_detached(buffer)?))
}

fn array_buffer_immutable_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer = ordinary_array_buffer_id_from_this(
        context,
        &this_value,
        "ArrayBuffer.prototype.immutable",
    )?;
    Ok(JsValue::Boolean(context.is_array_buffer_immutable(buffer)?))
}

fn array_buffer_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn array_buffer_is_view(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some(object) = arguments
        .first()
        .and_then(|value| context.value_object(value))
    else {
        return Ok(JsValue::Boolean(false));
    };
    Ok(JsValue::Boolean(
        is_typed_array_object(context, object) || is_data_view_object(context, object),
    ))
}

fn array_buffer_slice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_slice_with_species(vm, context, this_value, arguments, false)
}

fn array_buffer_slice_with_species(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    shared: bool,
) -> Result<JsValue, VmError> {
    let (buffer, name) = if shared {
        (
            shared_array_buffer_id_from_this(
                context,
                &this_value,
                "SharedArrayBuffer.prototype.slice",
            )?,
            "SharedArrayBuffer",
        )
    } else {
        (
            ordinary_array_buffer_id_from_this(
                context,
                &this_value,
                "ArrayBuffer.prototype.slice",
            )?,
            "ArrayBuffer",
        )
    };
    if context.is_array_buffer_detached(buffer)? {
        return Err(VmError::type_error("ArrayBuffer is detached"));
    }
    let length = context.array_buffer_byte_length(buffer)?;
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 1, length as f64)?,
        length,
    )
    .max(start);
    let new_length = end - start;
    let default_constructor = context
        .get_global(name)
        .ok_or_else(|| VmError::runtime(format!("{name} constructor missing")))?;
    let constructor =
        crate::builtins::array::species_constructor(vm, context, this_value, default_constructor)?;
    let result = vm.construct_value_from_builtin(
        constructor,
        vec![JsValue::Number(new_length as f64)],
        context,
    )?;
    let result_object = context.require_object(&result, "ArrayBuffer species result")?;
    let result_buffer = array_buffer_id_from_object(context, result_object)?;
    if context.is_shared_array_buffer(result_buffer)? != shared {
        return Err(VmError::type_error(
            "ArrayBuffer species result has the wrong buffer kind",
        ));
    }
    if result_buffer == buffer {
        return Err(VmError::type_error(
            "ArrayBuffer species result must be a new buffer",
        ));
    }
    if context.is_array_buffer_immutable(result_buffer)? {
        return Err(VmError::type_error(
            "ArrayBuffer species result is immutable",
        ));
    }
    if context.array_buffer_byte_length(result_buffer)? < new_length {
        return Err(VmError::type_error(
            "ArrayBuffer species result is too small",
        ));
    }
    if context.is_array_buffer_detached(buffer)? {
        return Err(VmError::type_error("ArrayBuffer is detached"));
    }
    let bytes = context
        .read_buffer_bytes(buffer, start, new_length)?
        .to_vec();
    context.write_buffer_bytes(result_buffer, 0, &bytes)?;
    Ok(result)
}

fn array_buffer_resize(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let buffer =
        ordinary_array_buffer_id_from_this(context, &this_value, "ArrayBuffer.prototype.resize")?;
    if context.is_array_buffer_immutable(buffer)? {
        return Err(VmError::type_error("ArrayBuffer is immutable"));
    }
    let new_byte_length = to_index(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    context.resize_array_buffer(buffer, new_byte_length)?;
    Ok(JsValue::Undefined)
}

fn array_buffer_transfer_common(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    immutable: bool,
    preserve_resizable: bool,
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.transfer")?;
    let buffer =
        ordinary_array_buffer_id_from_this(context, &this_value, "ArrayBuffer.prototype.transfer")?;
    let new_byte_length = match arguments.first() {
        None | Some(JsValue::Undefined) => None,
        Some(value) => Some(to_index(vm, context, value.clone())?),
    };
    if context.is_array_buffer_detached(buffer)? {
        return Err(VmError::type_error("ArrayBuffer is detached"));
    }
    if context.is_array_buffer_immutable(buffer)? {
        return Err(VmError::type_error("ArrayBuffer is immutable"));
    }
    let target =
        context.transfer_array_buffer(buffer, new_byte_length, immutable, preserve_resizable)?;
    let prototype = context
        .get_prototype_of(object)
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("ArrayBuffer prototype missing"))?;
    create_array_buffer_object_with_id(context, target, prototype)
}

fn array_buffer_transfer(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_transfer_common(vm, context, this_value, arguments, false, true)
}

fn array_buffer_transfer_to_fixed_length(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_transfer_common(vm, context, this_value, arguments, false, false)
}

fn array_buffer_transfer_to_immutable(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_transfer_common(vm, context, this_value, arguments, true, false)
}

fn array_buffer_slice_to_immutable(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(
        context,
        &this_value,
        "ArrayBuffer.prototype.sliceToImmutable",
    )?;
    let buffer = ordinary_array_buffer_id_from_this(
        context,
        &this_value,
        "ArrayBuffer.prototype.sliceToImmutable",
    )?;
    if context.is_array_buffer_detached(buffer)? {
        return Err(VmError::type_error("ArrayBuffer is detached"));
    }
    let length = context.array_buffer_byte_length(buffer)?;
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 1, length as f64)?,
        length,
    );
    let end = end.max(start);
    let copy = context.clone_array_buffer_range_with_immutable(buffer, start, end, true)?;
    let prototype = context
        .get_prototype_of(object)
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("ArrayBuffer prototype missing"))?;
    create_array_buffer_object_with_id(context, copy, prototype)
}

fn install_data_view(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor =
        context.register_builtin("DataView", 1, data_view_call, Some(data_view_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("DataView constructor object missing"))?;

    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    for (name, getter) in [
        (
            "buffer",
            context.register_builtin("get buffer", 0, data_view_buffer_get, None)?,
        ),
        (
            "byteLength",
            context.register_builtin("get byteLength", 0, data_view_byte_length_get, None)?,
        ),
        (
            "byteOffset",
            context.register_builtin("get byteOffset", 0, data_view_byte_offset_get, None)?,
        ),
    ] {
        context.define_own_property(
            prototype,
            name.into(),
            PropertyDescriptor::accessor(Some(getter), None, false, true),
        )?;
    }
    for (name, length, call) in [
        ("getInt8", 1, data_view_get_int8 as NativeCall),
        ("getUint8", 1, data_view_get_uint8 as NativeCall),
        ("getInt16", 1, data_view_get_int16 as NativeCall),
        ("getUint16", 1, data_view_get_uint16 as NativeCall),
        ("getInt32", 1, data_view_get_int32 as NativeCall),
        ("getUint32", 1, data_view_get_uint32 as NativeCall),
        ("getFloat16", 1, data_view_get_float16 as NativeCall),
        ("getFloat32", 1, data_view_get_float32 as NativeCall),
        ("getFloat64", 1, data_view_get_float64 as NativeCall),
        ("getBigInt64", 1, data_view_get_big_int64 as NativeCall),
        ("getBigUint64", 1, data_view_get_big_uint64 as NativeCall),
        ("setInt8", 2, data_view_set_int8 as NativeCall),
        ("setUint8", 2, data_view_set_uint8 as NativeCall),
        ("setInt16", 2, data_view_set_int16 as NativeCall),
        ("setUint16", 2, data_view_set_uint16 as NativeCall),
        ("setInt32", 2, data_view_set_int32 as NativeCall),
        ("setUint32", 2, data_view_set_uint32 as NativeCall),
        ("setFloat16", 2, data_view_set_float16 as NativeCall),
        ("setFloat32", 2, data_view_set_float32 as NativeCall),
        ("setFloat64", 2, data_view_set_float64 as NativeCall),
        ("setBigInt64", 2, data_view_set_big_int64 as NativeCall),
        ("setBigUint64", 2, data_view_set_big_uint64 as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }

    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        to_string_tag,
        readonly_configurable_descriptor(JsValue::String("DataView".into())),
    )?;
    declare_standard_global(context, "DataView", constructor)?;
    Ok(())
}

fn data_view_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("DataView constructor requires 'new'"))
}

fn data_view_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let buffer = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let buffer_object = context.require_object(&buffer, "DataView buffer")?;
    let buffer_id = array_buffer_id_from_object(context, buffer_object)?;
    let byte_offset = to_index(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    if context.is_array_buffer_detached(buffer_id)? {
        return Err(VmError::type_error("ArrayBuffer is detached"));
    }
    let buffer_length = context.array_buffer_byte_length(buffer_id)?;
    if byte_offset > buffer_length {
        return Err(VmError::range("DataView byteOffset is out of range"));
    }
    let (byte_length, length_tracking) = if let Some(value) = arguments.get(2)
        && !matches!(value, JsValue::Undefined)
    {
        (to_index(vm, context, value.clone())?, false)
    } else {
        (buffer_length - byte_offset, true)
    };
    if !length_tracking
        && byte_offset
            .checked_add(byte_length)
            .is_none_or(|end| end > buffer_length)
    {
        return Err(VmError::range("DataView byteLength is out of range"));
    }
    let prototype = data_view_prototype_from_constructor(vm, context, new_target)?;
    let view = context.create_data_view_with_tracking(
        buffer_id,
        byte_offset,
        byte_length,
        length_tracking,
    )?;
    let effective_byte_length = context.data_view_byte_length(view)?;
    let object = new_ordinary_object(context, Some(prototype))?;
    set_object_kind(context, object, ObjectKind::DataView { view })?;
    define_hidden(context, object, DATA_VIEW_MARKER, JsValue::Boolean(true))?;
    define_hidden(context, object, DATA_VIEW_BUFFER, buffer)?;
    define_hidden(
        context,
        object,
        DATA_VIEW_BYTE_LENGTH,
        JsValue::Number(effective_byte_length as f64),
    )?;
    define_hidden(
        context,
        object,
        DATA_VIEW_BYTE_OFFSET,
        JsValue::Number(byte_offset as f64),
    )?;
    Ok(JsValue::Object(object))
}

fn data_view_prototype_from_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    new_target: JsValue,
) -> Result<ObjectId, VmError> {
    let prototype_value = vm.get_property_value_with_receiver_from_builtin(
        new_target.clone(),
        new_target.clone(),
        "prototype",
        context,
    )?;
    if let Some(prototype) = context.value_object(&prototype_value) {
        return Ok(prototype);
    }
    let realm = context.realm_for_callable(&new_target);
    if let Some(realm) = realm
        && !context.is_current_realm(realm)
    {
        let activation = context.enter_realm(realm)?;
        let result = data_view_intrinsic_prototype(context);
        let leave_result = context.leave_realm(activation);
        return match (result, leave_result) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(_)) => Err(error),
        };
    }
    data_view_intrinsic_prototype(context)
}

fn data_view_intrinsic_prototype(context: &NativeContext) -> Result<ObjectId, VmError> {
    let constructor = context
        .get_global("DataView")
        .ok_or_else(|| VmError::runtime("DataView constructor missing"))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("DataView constructor object missing"))?;
    context
        .get_own_property_descriptor(constructor_object, "prototype")
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("DataView prototype missing"))
}

fn require_data_view(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<ObjectId, VmError> {
    let object = object_from_this(context, this_value, label)?;
    data_view_id_from_object(context, object)?;
    Ok(object)
}

fn data_view_buffer_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_data_view(context, &this_value, "DataView.prototype.buffer")?;
    Ok(own_data_value(context, object, DATA_VIEW_BUFFER).unwrap_or(JsValue::Undefined))
}

fn data_view_byte_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_data_view(context, &this_value, "DataView.prototype.byteLength")?;
    let view = data_view_id_from_object(context, object)?;
    Ok(JsValue::Number(context.data_view_byte_length(view)? as f64))
}

fn data_view_byte_offset_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_data_view(context, &this_value, "DataView.prototype.byteOffset")?;
    let view = data_view_id_from_object(context, object)?;
    Ok(JsValue::Number(context.data_view_byte_offset(view)? as f64))
}

fn data_view_get(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    kind: TypedArrayElementKind,
) -> Result<JsValue, VmError> {
    let object = require_data_view(context, &this_value, "DataView get")?;
    let view = data_view_id_from_object(context, object)?;
    let offset = to_index(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let little_endian = arguments.get(1).is_some_and(JsValue::to_boolean);
    context.data_view_get(view, offset, kind, little_endian)
}

fn data_view_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    kind: TypedArrayElementKind,
) -> Result<JsValue, VmError> {
    let object = require_data_view(context, &this_value, "DataView set")?;
    let view = data_view_id_from_object(context, object)?;
    let record = context
        .data_view_record(view)
        .ok_or_else(|| VmError::runtime("invalid DataView id"))?
        .clone();
    if context.is_array_buffer_immutable(record.buffer)? {
        return Err(VmError::type_error("ArrayBuffer is immutable"));
    }
    let offset = to_index(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let value = data_view_set_value(
        vm,
        context,
        kind,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let little_endian = arguments.get(2).is_some_and(JsValue::to_boolean);
    context.data_view_set(view, offset, kind, value, little_endian)?;
    Ok(JsValue::Undefined)
}

fn data_view_set_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    kind: TypedArrayElementKind,
    value: JsValue,
) -> Result<JsValue, VmError> {
    if kind.is_bigint() {
        return Ok(JsValue::BigInt(to_bigint_for_data_view(
            vm, context, value,
        )?));
    }
    Ok(JsValue::Number(vm.to_number(value, context)?))
}

fn to_bigint_for_data_view(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<BigIntValue, VmError> {
    match value {
        JsValue::BigInt(value) => Ok(value),
        JsValue::Boolean(value) => Ok(bigint::from_i64(i64::from(value))),
        JsValue::String(value) => bigint::parse_bigint_string(&value)
            .ok_or_else(|| VmError::syntax_error("Cannot convert string to BigInt")),
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            let primitive = vm.to_primitive(value, PreferredType::Number, context)?;
            to_bigint_for_data_view(vm, context, primitive)
        }
        _ => Err(VmError::type_error("Cannot convert value to BigInt")),
    }
}

pub(crate) fn coerce_typed_array_element(
    vm: &mut Vm,
    context: &mut NativeContext,
    kind: TypedArrayElementKind,
    value: JsValue,
) -> Result<JsValue, VmError> {
    if kind.is_bigint() {
        Ok(JsValue::BigInt(to_bigint_for_data_view(
            vm, context, value,
        )?))
    } else {
        Ok(JsValue::Number(vm.to_number(value, context)?))
    }
}

macro_rules! data_view_getter {
    ($name:ident, $kind:expr) => {
        fn $name(
            vm: &mut Vm,
            context: &mut NativeContext,
            this_value: JsValue,
            arguments: &[JsValue],
        ) -> Result<JsValue, VmError> {
            data_view_get(vm, context, this_value, arguments, $kind)
        }
    };
}

macro_rules! data_view_setter {
    ($name:ident, $kind:expr) => {
        fn $name(
            vm: &mut Vm,
            context: &mut NativeContext,
            this_value: JsValue,
            arguments: &[JsValue],
        ) -> Result<JsValue, VmError> {
            data_view_set(vm, context, this_value, arguments, $kind)
        }
    };
}

data_view_getter!(data_view_get_int8, TypedArrayElementKind::Int8);
data_view_getter!(data_view_get_uint8, TypedArrayElementKind::Uint8);
data_view_getter!(data_view_get_int16, TypedArrayElementKind::Int16);
data_view_getter!(data_view_get_uint16, TypedArrayElementKind::Uint16);
data_view_getter!(data_view_get_int32, TypedArrayElementKind::Int32);
data_view_getter!(data_view_get_uint32, TypedArrayElementKind::Uint32);
data_view_getter!(data_view_get_float16, TypedArrayElementKind::Float16);
data_view_getter!(data_view_get_float32, TypedArrayElementKind::Float32);
data_view_getter!(data_view_get_float64, TypedArrayElementKind::Float64);
data_view_getter!(data_view_get_big_int64, TypedArrayElementKind::BigInt64);
data_view_getter!(data_view_get_big_uint64, TypedArrayElementKind::BigUint64);
data_view_setter!(data_view_set_int8, TypedArrayElementKind::Int8);
data_view_setter!(data_view_set_uint8, TypedArrayElementKind::Uint8);
data_view_setter!(data_view_set_int16, TypedArrayElementKind::Int16);
data_view_setter!(data_view_set_uint16, TypedArrayElementKind::Uint16);
data_view_setter!(data_view_set_int32, TypedArrayElementKind::Int32);
data_view_setter!(data_view_set_uint32, TypedArrayElementKind::Uint32);
data_view_setter!(data_view_set_float16, TypedArrayElementKind::Float16);
data_view_setter!(data_view_set_float32, TypedArrayElementKind::Float32);
data_view_setter!(data_view_set_float64, TypedArrayElementKind::Float64);
data_view_setter!(data_view_set_big_int64, TypedArrayElementKind::BigInt64);
data_view_setter!(data_view_set_big_uint64, TypedArrayElementKind::BigUint64);

fn install_typed_array_intrinsic(
    context: &mut NativeContext,
) -> Result<TypedArrayIntrinsic, VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor = context.register_builtin(
        "TypedArray",
        0,
        typed_array_abstract_call,
        Some(typed_array_abstract_construct),
    )?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("%TypedArray% constructor object missing"))?;

    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    define_method(context, constructor_object, "from", 1, typed_array_from)?;
    define_method(context, constructor_object, "of", 0, typed_array_of)?;

    let species_getter =
        context.register_builtin("get [Symbol.species]", 0, typed_array_species_get, None)?;
    let species = context.well_known_symbols().species;
    context.define_symbol_own_property(
        constructor_object,
        species,
        PropertyDescriptor::accessor(Some(species_getter), None, false, true),
    )?;

    for (name, getter) in [
        (
            "buffer",
            context.register_builtin("get buffer", 0, typed_array_buffer_get, None)?,
        ),
        (
            "byteLength",
            context.register_builtin("get byteLength", 0, typed_array_byte_length_get, None)?,
        ),
        (
            "byteOffset",
            context.register_builtin("get byteOffset", 0, typed_array_byte_offset_get, None)?,
        ),
        (
            "length",
            context.register_builtin("get length", 0, typed_array_length_get, None)?,
        ),
    ] {
        context.define_own_property(
            prototype,
            name.into(),
            PropertyDescriptor::accessor(Some(getter), None, false, true),
        )?;
    }

    for (name, length, call) in [
        ("at", 1, typed_array_at as NativeCall),
        ("copyWithin", 2, typed_array_copy_within as NativeCall),
        ("entries", 0, typed_array_entries as NativeCall),
        ("every", 1, typed_array_every as NativeCall),
        ("fill", 1, typed_array_fill as NativeCall),
        ("filter", 1, typed_array_filter as NativeCall),
        ("find", 1, typed_array_find as NativeCall),
        ("findIndex", 1, typed_array_find_index as NativeCall),
        ("findLast", 1, typed_array_find_last as NativeCall),
        (
            "findLastIndex",
            1,
            typed_array_find_last_index as NativeCall,
        ),
        ("forEach", 1, typed_array_for_each as NativeCall),
        ("includes", 1, typed_array_includes as NativeCall),
        ("indexOf", 1, typed_array_index_of as NativeCall),
        ("join", 1, typed_array_join as NativeCall),
        ("keys", 0, typed_array_keys as NativeCall),
        ("lastIndexOf", 1, typed_array_last_index_of as NativeCall),
        ("map", 1, typed_array_map as NativeCall),
        ("reduce", 1, typed_array_reduce as NativeCall),
        ("reduceRight", 1, typed_array_reduce_right as NativeCall),
        ("reverse", 0, typed_array_reverse as NativeCall),
        ("set", 1, typed_array_set as NativeCall),
        ("slice", 2, typed_array_slice as NativeCall),
        ("some", 1, typed_array_some as NativeCall),
        ("sort", 1, typed_array_sort as NativeCall),
        ("subarray", 2, typed_array_subarray as NativeCall),
        (
            "toLocaleString",
            0,
            typed_array_to_locale_string as NativeCall,
        ),
        ("toReversed", 0, typed_array_to_reversed as NativeCall),
        ("toSorted", 1, typed_array_to_sorted as NativeCall),
        ("values", 0, typed_array_values as NativeCall),
        ("with", 2, typed_array_with as NativeCall),
    ] {
        let function = context.register_builtin(name, length, call, None)?;
        context.define_own_property(prototype, name.into(), method_descriptor(function.clone()))?;
        if name == "values" {
            let iterator = context.well_known_symbols().iterator;
            context.define_symbol_own_property(prototype, iterator, method_descriptor(function))?;
        }
    }

    let array_to_string = context
        .get_global("Array")
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten())
        .and_then(|array_prototype| {
            context.get_own_property_descriptor(array_prototype, "toString")
        })
        .ok_or_else(|| VmError::runtime("Array.prototype.toString missing"))?;
    context.define_own_property(prototype, "toString".into(), array_to_string)?;

    let to_string_tag_getter = context.register_builtin(
        "get [Symbol.toStringTag]",
        0,
        typed_array_to_string_tag_get,
        None,
    )?;
    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        to_string_tag,
        PropertyDescriptor::accessor(Some(to_string_tag_getter), None, false, true),
    )?;

    Ok(TypedArrayIntrinsic {
        constructor_object,
        prototype,
    })
}

fn install_typed_array_constructor(
    context: &mut NativeContext,
    intrinsic: TypedArrayIntrinsic,
    name: &'static str,
    bytes_per_element: usize,
) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, Some(intrinsic.prototype))?;
    let construct = typed_array_construct_for_name(name)?;
    let constructor = context.register_builtin(name, 3, typed_array_call, Some(construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("typed array constructor object missing"))?;
    context.set_prototype_of(constructor_object, Some(intrinsic.constructor_object))?;

    define_hidden(
        context,
        constructor_object,
        TYPED_ARRAY_NAME,
        JsValue::String(name.into()),
    )?;
    define_hidden(
        context,
        constructor_object,
        TYPED_ARRAY_BYTE_LENGTH,
        JsValue::Number(bytes_per_element as f64),
    )?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        constructor_object,
        "BYTES_PER_ELEMENT".into(),
        constant_descriptor(JsValue::Number(bytes_per_element as f64)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    context.define_own_property(
        prototype,
        "BYTES_PER_ELEMENT".into(),
        constant_descriptor(JsValue::Number(bytes_per_element as f64)),
    )?;

    declare_standard_global(context, name, constructor)?;
    Ok(())
}

fn typed_array_abstract_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "%TypedArray% is an abstract intrinsic and cannot be called directly",
    ))
}

fn typed_array_abstract_construct(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "%TypedArray% is an abstract intrinsic and cannot be constructed directly",
    ))
}

fn typed_array_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("TypedArray constructor requires 'new'"))
}

fn typed_array_construct_impl(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
    name: &'static str,
) -> Result<JsValue, VmError> {
    let kind = typed_array_kind(name)?;
    let bytes_per_element = kind.bytes_per_element();
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);

    if let Some(buffer_object) = context.value_object(&source)
        && let Some(buffer_id) = context.array_buffer_id_for_object(buffer_object)
    {
        let buffer_length = context.array_buffer_byte_length(buffer_id)?;
        let byte_offset = to_index(
            vm,
            context,
            arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        )?;
        if !byte_offset.is_multiple_of(bytes_per_element) {
            return Err(VmError::range(
                "TypedArray byteOffset is not element-aligned",
            ));
        }
        let (length, length_tracking) = if let Some(value) = arguments.get(2)
            && !matches!(value, JsValue::Undefined)
        {
            (to_index(vm, context, value.clone())?, false)
        } else {
            if byte_offset > buffer_length {
                return Err(VmError::range("TypedArray byteOffset is out of range"));
            }
            if !context.is_array_buffer_resizable(buffer_id)?
                && !(buffer_length - byte_offset).is_multiple_of(bytes_per_element)
            {
                return Err(VmError::range(
                    "ArrayBuffer byte length is not element-aligned",
                ));
            }
            (
                (buffer_length - byte_offset) / bytes_per_element,
                context.is_array_buffer_resizable(buffer_id)?,
            )
        };
        let prototype =
            typed_array_prototype_from_constructor(vm, context, new_target.clone(), name)?;
        return create_typed_array_object_with_tracking(
            context,
            prototype,
            name.into(),
            kind,
            source,
            buffer_id,
            byte_offset,
            length,
            length_tracking,
        );
    }

    let array_buffer_proto = context
        .get_global("ArrayBuffer")
        .and_then(|ctor| context.value_object(&ctor))
        .and_then(|ctor| {
            context
                .get_own_property_descriptor(ctor, "prototype")
                .and_then(|descriptor| match descriptor.kind {
                    PropertyKind::Data {
                        value: JsValue::Object(prototype),
                        ..
                    } => Some(prototype),
                    _ => None,
                })
        })
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("ArrayBuffer prototype missing"))?;
    let values = typed_array_constructor_values(vm, context, kind, source)?;
    let byte_length = values
        .len()
        .checked_mul(bytes_per_element)
        .ok_or_else(|| VmError::range("typed array byte length overflow"))?;
    if byte_length > MAX_SKELETON_BUFFER_BYTES {
        return Err(VmError::range(
            "typed array length exceeds V8 skeleton limit",
        ));
    }
    let prototype = typed_array_prototype_from_constructor(vm, context, new_target, name)?;
    let buffer = create_array_buffer_object(context, byte_length, array_buffer_proto)?;
    let JsValue::Object(buffer_object) = buffer else {
        unreachable!()
    };
    let buffer_id = array_buffer_id_from_object(context, buffer_object)?;
    let result = create_typed_array_object(
        context,
        prototype,
        name.into(),
        kind,
        buffer.clone(),
        buffer_id,
        0,
        values.len(),
    )?;
    let result_object = context.require_object(&result, "TypedArray result")?;
    let view = typed_array_view_id_from_object(context, result_object)?;
    for (index, value) in values.into_iter().enumerate() {
        let value = coerce_typed_array_element(vm, context, kind, value)?;
        typed_array_set_if_in_bounds(context, view, index, value)?;
    }
    Ok(result)
}

fn typed_array_prototype_from_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    new_target: JsValue,
    name: &str,
) -> Result<ObjectId, VmError> {
    let prototype = match vm.get_property_value_catching_from_builtin(
        new_target.clone(),
        "prototype",
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    if let Some(prototype) = context.value_object(&prototype) {
        return Ok(prototype);
    }

    let global = context.global_object_for_callable(&new_target);
    let constructor = match vm.get_property_value_catching_from_builtin(
        JsValue::Object(global),
        name,
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    let prototype =
        match vm.get_property_value_catching_from_builtin(constructor, "prototype", context)? {
            Ok(value) => value,
            Err(error) => return Err(vm.throw_value_from_builtin(error)),
        };
    context
        .value_object(&prototype)
        .ok_or_else(|| VmError::runtime("typed array prototype missing"))
}

macro_rules! typed_array_constructor {
    ($function:ident, $name:literal) => {
        fn $function(
            vm: &mut Vm,
            context: &mut NativeContext,
            arguments: &[JsValue],
            new_target: JsValue,
        ) -> Result<JsValue, VmError> {
            typed_array_construct_impl(vm, context, arguments, new_target, $name)
        }
    };
}

typed_array_constructor!(int8_array_construct, "Int8Array");
typed_array_constructor!(uint8_array_construct, "Uint8Array");
typed_array_constructor!(uint8_clamped_array_construct, "Uint8ClampedArray");
typed_array_constructor!(int16_array_construct, "Int16Array");
typed_array_constructor!(uint16_array_construct, "Uint16Array");
typed_array_constructor!(int32_array_construct, "Int32Array");
typed_array_constructor!(uint32_array_construct, "Uint32Array");
typed_array_constructor!(float32_array_construct, "Float32Array");
typed_array_constructor!(float64_array_construct, "Float64Array");
typed_array_constructor!(bigint64_array_construct, "BigInt64Array");
typed_array_constructor!(biguint64_array_construct, "BigUint64Array");

fn typed_array_construct_for_name(name: &str) -> Result<crate::runtime::NativeConstruct, VmError> {
    Ok(match name {
        "Int8Array" => int8_array_construct,
        "Uint8Array" => uint8_array_construct,
        "Uint8ClampedArray" => uint8_clamped_array_construct,
        "Int16Array" => int16_array_construct,
        "Uint16Array" => uint16_array_construct,
        "Int32Array" => int32_array_construct,
        "Uint32Array" => uint32_array_construct,
        "Float32Array" => float32_array_construct,
        "Float64Array" => float64_array_construct,
        "BigInt64Array" => bigint64_array_construct,
        "BigUint64Array" => biguint64_array_construct,
        _ => return Err(VmError::type_error("unknown TypedArray constructor")),
    })
}

fn typed_array_constructor_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    kind: TypedArrayElementKind,
    source: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    if matches!(source, JsValue::Undefined) {
        return Ok(Vec::new());
    }
    if context.value_object(&source).is_none() {
        let length = to_index(vm, context, source)?;
        let byte_length = length
            .checked_mul(kind.bytes_per_element())
            .ok_or_else(|| VmError::range("typed array byte length overflow"))?;
        if byte_length > MAX_SKELETON_BUFFER_BYTES {
            return Err(VmError::range(
                "typed array length exceeds V8 skeleton limit",
            ));
        }
        let zero = if kind.is_bigint() {
            JsValue::BigInt(bigint::from_i64(0))
        } else {
            JsValue::Number(0.0)
        };
        return Ok(vec![zero; length]);
    }
    collect_typed_array_source_values(vm, context, source)
}

#[allow(clippy::too_many_arguments)]
fn create_typed_array_object(
    context: &mut NativeContext,
    prototype: ObjectId,
    name: String,
    kind: TypedArrayElementKind,
    buffer_value: JsValue,
    buffer: ArrayBufferId,
    byte_offset: usize,
    length: usize,
) -> Result<JsValue, VmError> {
    create_typed_array_object_with_tracking(
        context,
        prototype,
        name,
        kind,
        buffer_value,
        buffer,
        byte_offset,
        length,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn create_typed_array_object_with_tracking(
    context: &mut NativeContext,
    prototype: ObjectId,
    name: String,
    kind: TypedArrayElementKind,
    buffer_value: JsValue,
    buffer: ArrayBufferId,
    byte_offset: usize,
    length: usize,
    length_tracking: bool,
) -> Result<JsValue, VmError> {
    let byte_length = length
        .checked_mul(kind.bytes_per_element())
        .ok_or_else(|| VmError::range("typed array byte length overflow"))?;
    if byte_length > MAX_SKELETON_BUFFER_BYTES {
        return Err(VmError::range(
            "typed array length exceeds V8 skeleton limit",
        ));
    }
    let view = context.create_typed_array_view_with_tracking(
        buffer,
        kind,
        byte_offset,
        length,
        length_tracking,
    )?;
    let object = new_ordinary_object(context, Some(prototype))?;
    set_object_kind(
        context,
        object,
        ObjectKind::TypedArray {
            view,
            length,
            name: name.clone(),
        },
    )?;
    define_hidden(context, object, TYPED_ARRAY_MARKER, JsValue::Boolean(true))?;
    define_hidden(
        context,
        object,
        TYPED_ARRAY_NAME,
        JsValue::String(name.into()),
    )?;
    define_hidden(
        context,
        object,
        TYPED_ARRAY_LENGTH,
        JsValue::Number(length as f64),
    )?;
    define_hidden(
        context,
        object,
        TYPED_ARRAY_BYTE_LENGTH,
        JsValue::Number(byte_length as f64),
    )?;
    define_hidden(
        context,
        object,
        TYPED_ARRAY_BYTE_OFFSET,
        JsValue::Number(byte_offset as f64),
    )?;
    define_hidden(context, object, TYPED_ARRAY_BUFFER, buffer_value)?;
    Ok(JsValue::Object(object))
}

fn require_typed_array(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<ObjectId, VmError> {
    let object = object_from_this(context, this_value, label)?;
    typed_array_view_id_from_object(context, object)?;
    Ok(object)
}

fn typed_array_buffer_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_typed_array(context, &this_value, "TypedArray.prototype.buffer")?;
    Ok(own_data_value(context, object, TYPED_ARRAY_BUFFER).unwrap_or(JsValue::Undefined))
}

fn typed_array_byte_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_typed_array(context, &this_value, "TypedArray.prototype.byteLength")?;
    let view = typed_array_view_id_from_object(context, object)?;
    Ok(JsValue::Number(
        context.typed_array_byte_length(view)? as f64
    ))
}

fn typed_array_byte_offset_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_typed_array(context, &this_value, "TypedArray.prototype.byteOffset")?;
    let view = typed_array_view_id_from_object(context, object)?;
    Ok(JsValue::Number(
        context.typed_array_byte_offset(view)? as f64
    ))
}

fn typed_array_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_typed_array(context, &this_value, "TypedArray.prototype.length")?;
    let (_, length) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("receiver is not a TypedArray"))?;
    Ok(JsValue::Number(length as f64))
}

fn require_callable(value: &JsValue, label: &str) -> Result<(), VmError> {
    if abstract_ops::is_callable(value) {
        Ok(())
    } else {
        Err(VmError::type_error(format!(
            "{label} callback is not callable"
        )))
    }
}

fn typed_array_parts(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<
    (
        ObjectId,
        TypedArrayViewId,
        usize,
        String,
        TypedArrayElementKind,
    ),
    VmError,
> {
    let object = require_typed_array(context, this_value, label)?;
    let (view, _) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("receiver is not a TypedArray"))?;
    let length = context.validate_typed_array_view(view)?;
    let name = typed_array_name_from_object(context, object)?;
    let kind = typed_array_kind(&name)?;
    Ok((object, view, length, name, kind))
}

fn typed_array_element_or_undefined(
    context: &NativeContext,
    view: TypedArrayViewId,
    index: usize,
) -> Result<JsValue, VmError> {
    let Some(length) = context
        .typed_array_view(view)
        .and_then(|_| context.validate_typed_array_view(view).ok())
    else {
        return Ok(JsValue::Undefined);
    };
    if index >= length {
        return Ok(JsValue::Undefined);
    }
    context.typed_array_load_element(view, index)
}

fn typed_array_set_if_in_bounds(
    context: &mut NativeContext,
    view: TypedArrayViewId,
    index: usize,
    value: JsValue,
) -> Result<(), VmError> {
    let Some(length) = context
        .typed_array_view(view)
        .and_then(|_| context.validate_typed_array_view(view).ok())
    else {
        return Ok(());
    };
    if index < length {
        context.typed_array_store_element(view, index, value)?;
    }
    Ok(())
}

pub(crate) fn validate_typed_array(
    context: &NativeContext,
    value: JsValue,
) -> Result<TypedArrayViewId, VmError> {
    let object = context.require_object(&value, "value is not a TypedArray")?;
    let view = typed_array_view_id_from_object(context, object)?;
    require_not_detached(context, view)?;
    Ok(view)
}

pub(crate) fn require_not_detached(
    context: &NativeContext,
    view: TypedArrayViewId,
) -> Result<(), VmError> {
    context.validate_typed_array_view(view).map(|_| ())
}

fn require_typed_array_writable(
    context: &NativeContext,
    view: TypedArrayViewId,
) -> Result<(), VmError> {
    let buffer = context
        .typed_array_view(view)
        .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
        .buffer;
    if context.is_array_buffer_immutable(buffer)? {
        Err(VmError::type_error("ArrayBuffer is immutable"))
    } else {
        Ok(())
    }
}

pub(crate) fn typed_array_species_create(
    vm: &mut Vm,
    context: &mut NativeContext,
    exemplar: JsValue,
    length: usize,
) -> Result<JsValue, VmError> {
    let exemplar_object = context.require_object(&exemplar, "TypedArray exemplar")?;
    let exemplar_name = typed_array_name_from_object(context, exemplar_object)?;
    let exemplar_kind = typed_array_kind(&exemplar_name)?;
    let default_constructor = context
        .get_global(&exemplar_name)
        .ok_or_else(|| VmError::runtime("TypedArray constructor missing"))?;
    let constructor =
        crate::builtins::array::species_constructor(vm, context, exemplar, default_constructor)?;
    let result = vm.construct_value_from_builtin(
        constructor,
        vec![JsValue::Number(length as f64)],
        context,
    )?;
    let result_object = context.require_object(&result, "TypedArray species result")?;
    let result_view = validate_typed_array(context, result.clone())?;
    let result_length = context.validate_typed_array_view(result_view)?;
    if result_length < length {
        return Err(VmError::type_error(
            "TypedArray species constructor returned a short TypedArray",
        ));
    }
    let result_name = typed_array_name_from_object(context, result_object)?;
    let result_kind = typed_array_kind(&result_name)?;
    let exemplar_bigint = matches!(
        exemplar_kind,
        TypedArrayElementKind::BigInt64 | TypedArrayElementKind::BigUint64
    );
    let result_bigint = matches!(
        result_kind,
        TypedArrayElementKind::BigInt64 | TypedArrayElementKind::BigUint64
    );
    if exemplar_bigint != result_bigint {
        return Err(VmError::type_error(
            "TypedArray species result has a different content type",
        ));
    }
    Ok(result)
}

fn create_typed_array_species_from_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    exemplar: JsValue,
    values: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let target = typed_array_species_create(vm, context, exemplar, values.len())?;
    store_values_into_typed_array(vm, context, target, values)
}

fn typed_array_values_vec(
    context: &NativeContext,
    view: TypedArrayViewId,
    length: usize,
) -> Result<Vec<JsValue>, VmError> {
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        values.push(context.typed_array_load_element(view, index)?);
    }
    Ok(values)
}

fn collect_array_like_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    source: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    let object = vm.to_object(source, context)?;
    let source_value = context.object_value(object);
    let length_value = vm.get_property_value(source_value.clone(), "length", context)?;
    let length = to_length(vm, context, length_value)?.min(MAX_SKELETON_BUFFER_BYTES);
    if length >= MAX_SKELETON_BUFFER_BYTES {
        return Err(VmError::range("array-like is too large for TypedArray"));
    }
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        values.push(vm.get_property_value(source_value.clone(), &index.to_string(), context)?);
    }
    Ok(values)
}

fn collect_typed_array_source_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    source: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    if let Some(values) = collect_iterable_values(vm, context, source.clone())? {
        return Ok(values);
    }
    collect_array_like_values(vm, context, source)
}

fn collect_iterable_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    source: JsValue,
) -> Result<Option<Vec<JsValue>>, VmError> {
    if context.value_object(&source).is_none() {
        return Ok(None);
    }
    let iterator_symbol = context.well_known_symbols().iterator;
    let method = vm.get_symbol_property_value_with_receiver_from_builtin(
        source.clone(),
        source.clone(),
        iterator_symbol,
        context,
    )?;
    if matches!(method, JsValue::Undefined | JsValue::Null) {
        return Ok(None);
    }
    require_callable(&method, "TypedArray iterable")?;
    let iterator = vm.call_value_from_builtin(method, source, Vec::new(), context)?;
    context.require_object(&iterator, "iterator method result")?;
    let next =
        match vm.get_property_value_catching_from_builtin(iterator.clone(), "next", context)? {
            Ok(value) => value,
            Err(error) => return Err(vm.throw_value_from_builtin(error)),
        };
    require_callable(&next, "iterator next")?;

    let mut values = Vec::new();
    loop {
        context.consume_loop_iteration()?;
        let result =
            vm.call_value_from_builtin(next.clone(), iterator.clone(), Vec::new(), context)?;
        context.require_object(&result, "iterator result")?;
        let done =
            match vm.get_property_value_catching_from_builtin(result.clone(), "done", context)? {
                Ok(value) => value.to_boolean(),
                Err(error) => return Err(vm.throw_value_from_builtin(error)),
            };
        if done {
            return Ok(Some(values));
        }
        let value = match vm.get_property_value_catching_from_builtin(result, "value", context)? {
            Ok(value) => value,
            Err(error) => return Err(vm.throw_value_from_builtin(error)),
        };
        values.push(value);
        if values.len() > MAX_SKELETON_BUFFER_BYTES {
            return Err(VmError::range("iterable is too large for TypedArray"));
        }
    }
}

fn array_buffer_prototype(context: &NativeContext) -> Result<ObjectId, VmError> {
    context
        .get_global("ArrayBuffer")
        .and_then(|ctor| context.value_object(&ctor))
        .and_then(|ctor| {
            context
                .get_own_property_descriptor(ctor, "prototype")
                .and_then(|descriptor| descriptor.value_cloned())
                .and_then(|value| context.value_object(&value))
        })
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("ArrayBuffer prototype missing"))
}

fn typed_array_prototype_for_name(
    context: &NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<ObjectId, VmError> {
    context
        .get_global(name)
        .and_then(|ctor| context.constructor_prototype(&ctor).ok().flatten())
        .or_else(|| context.get_prototype_of(object))
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("TypedArray prototype missing"))
}

fn create_typed_array_from_values(
    context: &mut NativeContext,
    source_object: ObjectId,
    name: String,
    kind: TypedArrayElementKind,
    values: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let byte_length = values
        .len()
        .checked_mul(kind.bytes_per_element())
        .ok_or_else(|| VmError::range("typed array byte length overflow"))?;
    let buffer =
        create_array_buffer_object(context, byte_length, array_buffer_prototype(context)?)?;
    let JsValue::Object(buffer_object) = buffer else {
        unreachable!()
    };
    let buffer_id = array_buffer_id_from_object(context, buffer_object)?;
    let prototype = typed_array_prototype_for_name(context, source_object, &name)?;
    let result = create_typed_array_object(
        context,
        prototype,
        name,
        kind,
        buffer.clone(),
        buffer_id,
        0,
        values.len(),
    )?;
    let object = context.require_object(&result, "TypedArray result")?;
    let view = typed_array_view_id_from_object(context, object)?;
    for (index, value) in values.into_iter().enumerate() {
        typed_array_set_if_in_bounds(context, view, index, value)?;
    }
    Ok(result)
}

fn store_values_into_typed_array(
    vm: &mut Vm,
    context: &mut NativeContext,
    target: JsValue,
    values: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let object = context.require_object(&target, "TypedArray target")?;
    let view = typed_array_view_id_from_object(context, object)?;
    require_typed_array_writable(context, view)?;
    let (_, length) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("target is not a TypedArray"))?;
    if values.len() > length {
        return Err(VmError::type_error(
            "source is too large for TypedArray target",
        ));
    }
    let kind = context
        .typed_array_view(view)
        .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
        .element_kind;
    for (index, value) in values.into_iter().enumerate() {
        let value = coerce_typed_array_element(vm, context, kind, value)?;
        typed_array_set_if_in_bounds(context, view, index, value)?;
    }
    Ok(target)
}

fn construct_typed_array_with_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    values: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let target = vm.construct_value_from_builtin(
        constructor,
        vec![JsValue::Number(values.len() as f64)],
        context,
    )?;
    store_values_into_typed_array(vm, context, target, values)
}

fn typed_array_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let map_fn = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !matches!(map_fn, JsValue::Undefined) {
        require_callable(&map_fn, "TypedArray.from")?;
    }

    if let Some(values) = collect_iterable_values(vm, context, source.clone())? {
        return typed_array_from_values(
            vm,
            context,
            this_value,
            values,
            map_fn,
            arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
        );
    }

    let source_object = vm.to_object(source, context)?;
    let source_value = context.object_value(source_object);
    let length_value = match vm.get_property_value_catching_from_builtin(
        source_value.clone(),
        "length",
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    let source_length = to_length(vm, context, length_value)?.min(MAX_SKELETON_BUFFER_BYTES);
    let target = vm.construct_value_from_builtin(
        this_value,
        vec![JsValue::Number(source_length as f64)],
        context,
    )?;
    let object = context.require_object(&target, "TypedArray.from result")?;
    let view = typed_array_view_id_from_object(context, object)?;
    require_typed_array_writable(context, view)?;
    let kind = context
        .typed_array_view(view)
        .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
        .element_kind;
    let (_, result_length) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("TypedArray.from result is not a TypedArray"))?;
    if result_length < source_length {
        return Err(VmError::type_error(
            "TypedArray.from constructor returned a short TypedArray",
        ));
    }

    let this_arg = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    for index in 0..source_length {
        let value = match vm.get_property_value_catching_from_builtin(
            source_value.clone(),
            &index.to_string(),
            context,
        )? {
            Ok(value) => value,
            Err(error) => return Err(vm.throw_value_from_builtin(error)),
        };
        let value = if matches!(map_fn, JsValue::Undefined) {
            value
        } else {
            vm.call_value_from_builtin(
                map_fn.clone(),
                this_arg.clone(),
                vec![value, JsValue::Number(index as f64)],
                context,
            )?
        };
        let value = coerce_typed_array_element(vm, context, kind, value)?;
        typed_array_set_if_in_bounds(context, view, index, value)?;
    }
    Ok(target)
}

fn typed_array_from_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    values: Vec<JsValue>,
    map_fn: JsValue,
    this_arg: JsValue,
) -> Result<JsValue, VmError> {
    let target = vm.construct_value_from_builtin(
        constructor,
        vec![JsValue::Number(values.len() as f64)],
        context,
    )?;
    let object = context.require_object(&target, "TypedArray.from result")?;
    let view = typed_array_view_id_from_object(context, object)?;
    require_typed_array_writable(context, view)?;
    let kind = context
        .typed_array_view(view)
        .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
        .element_kind;
    let (_, length) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("TypedArray.from result is not a TypedArray"))?;
    if values.len() > length {
        return Err(VmError::type_error(
            "TypedArray.from constructor returned a short TypedArray",
        ));
    }
    for (index, value) in values.into_iter().enumerate() {
        let value = if matches!(map_fn, JsValue::Undefined) {
            value
        } else {
            vm.call_value_from_builtin(
                map_fn.clone(),
                this_arg.clone(),
                vec![value, JsValue::Number(index as f64)],
                context,
            )?
        };
        let value = coerce_typed_array_element(vm, context, kind, value)?;
        typed_array_set_if_in_bounds(context, view, index, value)?;
    }
    Ok(target)
}

fn typed_array_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_typed_array_with_values(vm, context, this_value, arguments.to_vec())
}

fn typed_array_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.at")?;
    let raw = argument_integer(vm, context, arguments, 0, 0.0)? as isize;
    let index = if raw < 0 {
        let from_end = raw.unsigned_abs();
        if from_end > length {
            return Ok(JsValue::Undefined);
        }
        length - from_end
    } else {
        raw as usize
    };
    if index >= length {
        return Ok(JsValue::Undefined);
    }
    typed_array_element_or_undefined(context, view, index)
}

fn typed_array_keys(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, _, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.keys")?;
    context.create_array_iterator_object(
        this_value,
        length,
        IteratorMode::Key,
        iterator_prototype(context),
    )
}

fn typed_array_values(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, _, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.values")?;
    context.create_array_iterator_object(
        this_value,
        length,
        IteratorMode::Value,
        iterator_prototype(context),
    )
}

fn typed_array_entries(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, _, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.entries")?;
    context.create_array_iterator_object(
        this_value,
        length,
        IteratorMode::KeyAndValue,
        iterator_prototype(context),
    )
}

fn iterator_prototype(context: &NativeContext) -> Option<ObjectId> {
    context
        .get_global("Iterator")
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten())
        .or_else(|| context.object_prototype())
}

fn typed_array_join(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.join")?;
    let sep = match arguments.first() {
        None | Some(JsValue::Undefined) => ",".to_string(),
        Some(value) => vm.to_string_coerce(value.clone(), context)?.to_string(),
    };
    let mut parts = Vec::with_capacity(length);
    for index in 0..length {
        let value = typed_array_element_or_undefined(context, view, index)?;
        parts.push(if matches!(value, JsValue::Undefined | JsValue::Null) {
            String::new()
        } else {
            vm.to_string_coerce(value, context)?.to_string()
        });
    }
    Ok(JsValue::String(parts.join(&sep).into()))
}

fn typed_array_to_locale_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.toLocaleString")?;
    let mut parts = Vec::with_capacity(length);
    for index in 0..length {
        let element = typed_array_element_or_undefined(context, view, index)?;
        if matches!(element, JsValue::Undefined | JsValue::Null) {
            parts.push(String::new());
            continue;
        }
        let to_locale = vm.get_property_value(element.clone(), "toLocaleString", context)?;
        if !abstract_ops::is_callable_with_context(context, &to_locale) {
            return Err(VmError::type_error(
                "TypedArray element toLocaleString is not callable",
            ));
        }
        let localized = vm.call_value_from_builtin(to_locale, element, Vec::new(), context)?;
        parts.push(vm.to_string_coerce(localized, context)?.to_string());
    }
    Ok(JsValue::String(parts.join(",").into()))
}

fn typed_array_fill(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.fill")?;
    require_typed_array_writable(context, view)?;
    let value = coerce_typed_array_element(
        vm,
        context,
        kind,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 2, length as f64)?,
        length,
    );
    require_not_detached(context, view)?;
    for index in start..end.max(start).min(length) {
        context.typed_array_store_element(view, index, value.clone())?;
    }
    Ok(this_value)
}

fn typed_array_includes(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.includes")?;
    if length == 0 {
        return Ok(JsValue::Boolean(false));
    }
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    for index in start..length {
        if abstract_ops::same_value_zero(
            &typed_array_element_or_undefined(context, view, index)?,
            &search,
        ) {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}

fn typed_array_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.indexOf")?;
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    let current_length = context
        .typed_array_indexed_view(object)
        .map_or(0, |(_, current_length)| current_length)
        .min(length);
    for index in start..current_length {
        if typed_array_element_or_undefined(context, view, index)?.strict_equals(&search) {
            return Ok(JsValue::Number(index as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn typed_array_last_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.lastIndexOf")?;
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let raw = if let Some(value) = arguments.get(1) {
        let number = vm.to_number(value.clone(), context)?;
        if number.is_nan() || number == 0.0 {
            0.0
        } else {
            number.trunc()
        }
    } else {
        (length - 1) as f64
    };
    let current_length = context
        .typed_array_indexed_view(object)
        .map_or(0, |(_, current_length)| current_length)
        .min(length);
    if current_length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let start = if raw < 0.0 {
        let from_end = (-raw) as usize;
        if from_end > length {
            return Ok(JsValue::Number(-1.0));
        }
        length - from_end
    } else {
        (raw as usize).min(length - 1)
    };
    for index in (0..=start).rev() {
        if typed_array_element_or_undefined(context, view, index)?.strict_equals(&search) {
            return Ok(JsValue::Number(index as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn typed_array_for_each(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.forEach")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.forEach")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for index in 0..length {
        vm.call_value_from_builtin(
            callback.clone(),
            this_arg.clone(),
            vec![
                typed_array_element_or_undefined(context, view, index)?,
                JsValue::Number(index as f64),
                this_value.clone(),
            ],
            context,
        )?;
    }
    Ok(JsValue::Undefined)
}

fn typed_array_predicate_loop(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    label: &str,
    mode: &str,
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) = typed_array_parts(context, &this_value, label)?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, label)?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for index in 0..length {
        let value = typed_array_element_or_undefined(context, view, index)?;
        let keep = vm
            .call_value_from_builtin(
                callback.clone(),
                this_arg.clone(),
                vec![
                    value.clone(),
                    JsValue::Number(index as f64),
                    this_value.clone(),
                ],
                context,
            )?
            .to_boolean();
        match mode {
            "every" if !keep => return Ok(JsValue::Boolean(false)),
            "some" if keep => return Ok(JsValue::Boolean(true)),
            "find" if keep => return Ok(value),
            "findIndex" if keep => return Ok(JsValue::Number(index as f64)),
            _ => {}
        }
    }
    Ok(match mode {
        "every" => JsValue::Boolean(true),
        "some" => JsValue::Boolean(false),
        "find" => JsValue::Undefined,
        "findIndex" => JsValue::Number(-1.0),
        _ => JsValue::Undefined,
    })
}

fn typed_array_every(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    typed_array_predicate_loop(
        vm,
        context,
        this_value,
        arguments,
        "TypedArray.prototype.every",
        "every",
    )
}

fn typed_array_some(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    typed_array_predicate_loop(
        vm,
        context,
        this_value,
        arguments,
        "TypedArray.prototype.some",
        "some",
    )
}

fn typed_array_find(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    typed_array_predicate_loop(
        vm,
        context,
        this_value,
        arguments,
        "TypedArray.prototype.find",
        "find",
    )
}

fn typed_array_find_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    typed_array_predicate_loop(
        vm,
        context,
        this_value,
        arguments,
        "TypedArray.prototype.findIndex",
        "findIndex",
    )
}

fn typed_array_find_last(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.findLast")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.findLast")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for index in (0..length).rev() {
        let value = typed_array_element_or_undefined(context, view, index)?;
        if vm
            .call_value_from_builtin(
                callback.clone(),
                this_arg.clone(),
                vec![
                    value.clone(),
                    JsValue::Number(index as f64),
                    this_value.clone(),
                ],
                context,
            )?
            .to_boolean()
        {
            return Ok(value);
        }
    }
    Ok(JsValue::Undefined)
}

fn typed_array_find_last_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.findLastIndex")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.findLastIndex")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for index in (0..length).rev() {
        let value = typed_array_element_or_undefined(context, view, index)?;
        if vm
            .call_value_from_builtin(
                callback.clone(),
                this_arg.clone(),
                vec![value, JsValue::Number(index as f64), this_value.clone()],
                context,
            )?
            .to_boolean()
        {
            return Ok(JsValue::Number(index as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn typed_array_map(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.map")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.map")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let target = typed_array_species_create(vm, context, this_value.clone(), length)?;
    let target_object = context.require_object(&target, "TypedArray.prototype.map result")?;
    let target_view = typed_array_view_id_from_object(context, target_object)?;
    require_typed_array_writable(context, target_view)?;
    let target_kind = typed_array_kind(&typed_array_name_from_object(context, target_object)?)?;
    for index in 0..length {
        let value = vm.call_value_from_builtin(
            callback.clone(),
            this_arg.clone(),
            vec![
                typed_array_element_or_undefined(context, view, index)?,
                JsValue::Number(index as f64),
                this_value.clone(),
            ],
            context,
        )?;
        let value = coerce_typed_array_element(vm, context, target_kind, value)?;
        typed_array_set_if_in_bounds(context, target_view, index, value)?;
    }
    Ok(target)
}

fn typed_array_filter(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.filter")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.filter")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut values = Vec::new();
    for index in 0..length {
        let value = typed_array_element_or_undefined(context, view, index)?;
        if vm
            .call_value_from_builtin(
                callback.clone(),
                this_arg.clone(),
                vec![
                    value.clone(),
                    JsValue::Number(index as f64),
                    this_value.clone(),
                ],
                context,
            )?
            .to_boolean()
        {
            values.push(value);
        }
    }
    create_typed_array_species_from_values(vm, context, this_value, values)
}

fn typed_array_reduce_common(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    reverse: bool,
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.reduce")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.reduce")?;
    if length == 0 && arguments.get(1).is_none() {
        return Err(VmError::type_error("reduce of empty TypedArray"));
    }
    let mut indices: Vec<usize> = (0..length).collect();
    if reverse {
        indices.reverse();
    }
    let mut iter = indices.into_iter();
    let mut acc = if let Some(initial) = arguments.get(1) {
        initial.clone()
    } else {
        typed_array_element_or_undefined(context, view, iter.next().unwrap())?
    };
    for index in iter {
        acc = vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![
                acc,
                typed_array_element_or_undefined(context, view, index)?,
                JsValue::Number(index as f64),
                this_value.clone(),
            ],
            context,
        )?;
    }
    Ok(acc)
}

fn typed_array_reduce(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    typed_array_reduce_common(vm, context, this_value, arguments, false)
}

fn typed_array_reduce_right(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    typed_array_reduce_common(vm, context, this_value, arguments, true)
}

fn typed_array_reverse(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.reverse")?;
    require_typed_array_writable(context, view)?;
    let values = typed_array_values_vec(context, view, length)?;
    for (index, value) in values.into_iter().rev().enumerate() {
        context.typed_array_store_element(view, index, value)?;
    }
    Ok(this_value)
}

fn typed_array_copy_within(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.copyWithin")?;
    require_typed_array_writable(context, view)?;
    let target =
        normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 2, length as f64)?,
        length,
    );
    let current_length = context.validate_typed_array_view(view)?;
    let effective_length = current_length.min(length);
    let count = end
        .min(effective_length)
        .saturating_sub(start)
        .min(effective_length.saturating_sub(target));
    let mut values = Vec::with_capacity(count);
    for index in start..start + count {
        values.push(typed_array_element_or_undefined(context, view, index)?);
    }
    for (offset, value) in values.into_iter().enumerate() {
        typed_array_set_if_in_bounds(context, view, target + offset, value)?;
    }
    Ok(this_value)
}

fn typed_array_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.set")?;
    require_typed_array_writable(context, view)?;
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let offset = to_index(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    require_not_detached(context, view)?;
    if let Some(source_object) = context.value_object(&source)
        && let Some((source_view, source_length)) = context.typed_array_indexed_view(source_object)
    {
        require_not_detached(context, source_view)?;
        if offset
            .checked_add(source_length)
            .is_none_or(|end| end > length)
        {
            return Err(VmError::range("source is too large for target TypedArray"));
        }
        let values = typed_array_values_vec(context, source_view, source_length)?;
        for (index, value) in values.into_iter().enumerate() {
            typed_array_set_if_in_bounds(context, view, offset + index, value)?;
        }
        return Ok(JsValue::Undefined);
    }

    let source_object = vm.to_object(source, context)?;
    let source_value = context.object_value(source_object);
    let length_value = match vm.get_property_value_catching_from_builtin(
        source_value.clone(),
        "length",
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    let source_length = to_length(vm, context, length_value)?;
    if offset
        .checked_add(source_length)
        .is_none_or(|end| end > length)
    {
        return Err(VmError::range("source is too large for target TypedArray"));
    }
    for index in 0..source_length {
        let value = match vm.get_property_value_catching_from_builtin(
            source_value.clone(),
            &index.to_string(),
            context,
        )? {
            Ok(value) => value,
            Err(error) => return Err(vm.throw_value_from_builtin(error)),
        };
        let value = coerce_typed_array_element(vm, context, kind, value)?;
        typed_array_set_if_in_bounds(context, view, offset + index, value)?;
    }
    Ok(JsValue::Undefined)
}

fn typed_array_slice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.slice")?;
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 1, length as f64)?,
        length,
    );
    let end = end.max(start).min(length);
    let count = end - start;
    let target = typed_array_species_create(vm, context, this_value, count)?;
    if count == 0 {
        return Ok(target);
    }
    let current_length = context.validate_typed_array_view(view)?;
    let copy_end = end.min(current_length);
    let target_object = context.require_object(&target, "TypedArray.prototype.slice result")?;
    let target_view = typed_array_view_id_from_object(context, target_object)?;
    let target_kind = typed_array_kind(&typed_array_name_from_object(context, target_object)?)?;
    let zero = if kind.is_bigint() {
        JsValue::BigInt(bigint::from_i64(0))
    } else {
        JsValue::Number(0.0)
    };
    for offset in 0..count {
        let source_index = start + offset;
        let value = if source_index < copy_end {
            context.typed_array_load_element(view, source_index)?
        } else {
            zero.clone()
        };
        let value = coerce_typed_array_element(vm, context, target_kind, value)?;
        context.typed_array_store_element(target_view, offset, value)?;
    }
    Ok(target)
}

fn typed_array_subarray(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_typed_array(context, &this_value, "TypedArray.prototype.subarray")?;
    let (view, length) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("receiver is not a TypedArray"))?;
    let name = typed_array_name_from_object(context, object)?;
    let kind = typed_array_kind(&name)?;
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 1, length as f64)?,
        length,
    )
    .max(start)
    .min(length);
    let record = context
        .typed_array_view(view)
        .ok_or_else(|| VmError::runtime("invalid TypedArray view id"))?
        .clone();
    let byte_offset = record
        .byte_offset
        .checked_add(start * kind.bytes_per_element())
        .ok_or_else(|| VmError::range("typed array byteOffset overflow"))?;
    let buffer_value =
        own_data_value(context, object, TYPED_ARRAY_BUFFER).unwrap_or(JsValue::Undefined);
    let default_constructor = context
        .get_global(&name)
        .ok_or_else(|| VmError::runtime("TypedArray constructor missing"))?;
    let constructor =
        crate::builtins::array::species_constructor(vm, context, this_value, default_constructor)?;
    let arguments = if record.length_tracking
        && arguments
            .get(1)
            .is_none_or(|value| matches!(value, JsValue::Undefined))
    {
        vec![buffer_value, JsValue::Number(byte_offset as f64)]
    } else {
        vec![
            buffer_value,
            JsValue::Number(byte_offset as f64),
            JsValue::Number((end - start) as f64),
        ]
    };
    let result = vm.construct_value_from_builtin(constructor, arguments, context)?;
    let result_view = validate_typed_array(context, result.clone())?;
    let result_object = context.require_object(&result, "TypedArray species result")?;
    let result_kind = typed_array_kind(&typed_array_name_from_object(context, result_object)?)?;
    let source_bigint = matches!(
        kind,
        TypedArrayElementKind::BigInt64 | TypedArrayElementKind::BigUint64
    );
    let result_bigint = matches!(
        result_kind,
        TypedArrayElementKind::BigInt64 | TypedArrayElementKind::BigUint64
    );
    if source_bigint != result_bigint {
        return Err(VmError::type_error(
            "TypedArray species result has a different content type",
        ));
    }
    require_not_detached(context, result_view)?;
    Ok(result)
}

fn typed_array_sort_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    mut values: Vec<JsValue>,
    compare_fn: Option<JsValue>,
) -> Result<Vec<JsValue>, VmError> {
    if compare_fn.is_none() {
        values.sort_by(typed_array_default_compare);
        return Ok(values);
    }

    for i in 1..values.len() {
        let mut j = i;
        while j > 0 {
            let func = compare_fn.as_ref().expect("compare_fn checked above");
            let compared = vm.call_value_from_builtin(
                func.clone(),
                JsValue::Undefined,
                vec![values[j - 1].clone(), values[j].clone()],
                context,
            )?;
            let swap = vm.to_number(compared, context)? > 0.0;
            if !swap {
                break;
            }
            values.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(values)
}

fn typed_array_default_compare(left: &JsValue, right: &JsValue) -> std::cmp::Ordering {
    match (left, right) {
        (JsValue::BigInt(left), JsValue::BigInt(right)) => bigint::cmp(left, right),
        _ => compare_typed_array_numbers(
            left.to_number().unwrap_or(f64::NAN),
            right.to_number().unwrap_or(f64::NAN),
        ),
    }
}

fn compare_typed_array_numbers(left: f64, right: f64) -> std::cmp::Ordering {
    match (left.is_nan(), right.is_nan()) {
        (true, true) => return std::cmp::Ordering::Equal,
        (true, false) => return std::cmp::Ordering::Greater,
        (false, true) => return std::cmp::Ordering::Less,
        (false, false) => {}
    }
    if left == right {
        return match (left.is_sign_negative(), right.is_sign_negative()) {
            (true, false) if left == 0.0 => std::cmp::Ordering::Less,
            (false, true) if left == 0.0 => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        };
    }
    left.partial_cmp(&right)
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn typed_array_sort(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.sort")?;
    require_typed_array_writable(context, view)?;
    let compare_fn = arguments
        .first()
        .cloned()
        .filter(|value| !matches!(value, JsValue::Undefined));
    if let Some(func) = &compare_fn {
        require_callable(func, "TypedArray.prototype.sort")?;
    }
    let values = typed_array_sort_values(
        vm,
        context,
        typed_array_values_vec(context, view, length)?,
        compare_fn,
    )?;
    for (index, value) in values.into_iter().enumerate() {
        typed_array_set_if_in_bounds(context, view, index, value)?;
    }
    Ok(this_value)
}

fn typed_array_to_reversed(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, name, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.toReversed")?;
    let mut values = typed_array_values_vec(context, view, length)?;
    values.reverse();
    create_typed_array_from_values(context, object, name, kind, values)
}

fn typed_array_to_sorted(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, name, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.toSorted")?;
    let compare_fn = arguments
        .first()
        .cloned()
        .filter(|value| !matches!(value, JsValue::Undefined));
    if let Some(func) = &compare_fn {
        require_callable(func, "TypedArray.prototype.toSorted")?;
    }
    let values = typed_array_sort_values(
        vm,
        context,
        typed_array_values_vec(context, view, length)?,
        compare_fn,
    )?;
    create_typed_array_from_values(context, object, name, kind, values)
}

fn typed_array_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, name, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.with")?;
    let raw = argument_integer(vm, context, arguments, 0, 0.0)? as isize;
    let value = coerce_typed_array_element(
        vm,
        context,
        kind,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let index = if raw < 0 {
        let from_end = raw.unsigned_abs();
        if from_end > length {
            return Err(VmError::range("TypedArray index is out of range"));
        }
        length - from_end
    } else {
        raw as usize
    };
    let current_length = context
        .validate_typed_array_view(view)
        .map_err(|_| VmError::range("TypedArray index is out of range"))?;
    if index >= current_length {
        return Err(VmError::range("TypedArray index is out of range"));
    }
    let mut values = Vec::with_capacity(length);
    for source_index in 0..length {
        values.push(typed_array_element_or_undefined(
            context,
            view,
            source_index,
        )?);
    }
    if index < values.len() {
        values[index] = value;
    }
    create_typed_array_from_values(context, object, name, kind, values)
}

fn typed_array_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn typed_array_to_string_tag_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some(object) = context.value_object(&this_value) else {
        return Ok(JsValue::Undefined);
    };
    if !is_typed_array_object(context, object) {
        return Ok(JsValue::Undefined);
    }
    Ok(own_data_value(context, object, TYPED_ARRAY_NAME).unwrap_or(JsValue::Undefined))
}

fn install_intl(context: &mut NativeContext) -> Result<(), VmError> {
    let intl = new_ordinary_object(context, context.object_prototype())?;
    let fallback_symbol = context.create_symbol(Some("IntlLegacyConstructedSymbol".into()));
    define_hidden(context, intl, INTL_FALLBACK_SYMBOL, fallback_symbol)?;
    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        intl,
        to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Intl".into())),
    )?;

    for spec in [
        IntlConstructorSpec {
            name: "DateTimeFormat",
            kind: "DateTimeFormat",
            construct: intl_date_time_format_construct,
            resolved_options: intl_date_time_format_resolved_options,
        },
        IntlConstructorSpec {
            name: "NumberFormat",
            kind: "NumberFormat",
            construct: intl_number_format_construct,
            resolved_options: intl_number_format_resolved_options,
        },
        IntlConstructorSpec {
            name: "Collator",
            kind: "Collator",
            construct: intl_collator_construct,
            resolved_options: intl_collator_resolved_options,
        },
    ] {
        install_intl_constructor(context, intl, spec)?;
    }

    if let Some(number_prototype) = context.number_prototype() {
        define_method(
            context,
            number_prototype,
            "toLocaleString",
            0,
            intl_number_to_locale_string,
        )?;
    }

    declare_standard_global(context, "Intl", JsValue::Object(intl))?;
    Ok(())
}

#[derive(Clone, Copy)]
struct IntlConstructorSpec {
    name: &'static str,
    kind: &'static str,
    construct: crate::runtime::NativeConstruct,
    resolved_options: NativeCall,
}

fn install_intl_constructor(
    context: &mut NativeContext,
    intl: ObjectId,
    spec: IntlConstructorSpec,
) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor =
        context.register_builtin(spec.name, 0, spec.construct_as_call(), Some(spec.construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Intl constructor object missing"))?;
    define_hidden(
        context,
        constructor_object,
        INTL_KIND,
        JsValue::String(spec.kind.into()),
    )?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    define_method(
        context,
        constructor_object,
        "supportedLocalesOf",
        1,
        intl_supported_locales_of,
    )?;
    define_method(
        context,
        prototype,
        "resolvedOptions",
        0,
        spec.resolved_options,
    )?;
    if spec.kind == "NumberFormat" {
        let getter =
            context.register_builtin("get format", 0, intl_number_format_format_get, None)?;
        context.define_own_property(
            prototype,
            "format".into(),
            PropertyDescriptor::accessor(Some(getter), None, false, true),
        )?;
    } else if spec.kind == "Collator" {
        define_method(context, prototype, "compare", 2, intl_collator_compare)?;
    }
    let tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        tag,
        readonly_configurable_descriptor(JsValue::String(format!("Intl.{}", spec.name).into())),
    )?;
    context.define_own_property(intl, spec.name.into(), method_descriptor(constructor))?;
    Ok(())
}

impl IntlConstructorSpec {
    fn construct_as_call(self) -> NativeCall {
        match self.kind {
            "DateTimeFormat" => intl_date_time_format_call,
            "NumberFormat" => intl_number_format_call,
            "Collator" => intl_collator_call,
            _ => intl_unsupported_call,
        }
    }
}

fn intl_unsupported_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("unsupported Intl constructor"))
}

fn intl_date_time_format_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_intl_object(
        vm,
        context,
        arguments,
        "DateTimeFormat",
        "Intl.DateTimeFormat",
    )
}

fn intl_date_time_format_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    construct_intl_object_with_new_target(vm, context, arguments, new_target, "DateTimeFormat")
}

fn intl_number_format_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let number_format =
        construct_intl_object(vm, context, arguments, "NumberFormat", "Intl.NumberFormat")?;
    let constructor = context
        .get_global("Intl")
        .and_then(|intl| context.value_object(&intl))
        .and_then(|intl| context.get_own_property_descriptor(intl, "NumberFormat"))
        .and_then(|descriptor| descriptor.value_cloned())
        .ok_or_else(|| VmError::runtime("Intl.NumberFormat missing"))?;
    if context.instance_of(this_value.clone(), constructor)?
        && let Some(object) = context.value_object(&this_value)
        && let Some(symbol) = intl_fallback_symbol(context)
    {
        context.define_symbol_own_property(object, symbol, constant_descriptor(number_format))?;
        return Ok(this_value);
    }
    Ok(number_format)
}

fn intl_number_format_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    construct_intl_object_with_new_target(vm, context, arguments, new_target, "NumberFormat")
}

fn intl_collator_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_intl_object(vm, context, arguments, "Collator", "Intl.Collator")
}

fn intl_collator_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    construct_intl_object_with_new_target(vm, context, arguments, new_target, "Collator")
}

fn construct_intl_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    kind: &'static str,
    constructor_name: &str,
) -> Result<JsValue, VmError> {
    let constructor = context
        .get_global("Intl")
        .and_then(|intl| context.value_object(&intl))
        .and_then(|intl| context.get_own_property_descriptor(intl, kind))
        .and_then(|descriptor| descriptor.value_cloned())
        .ok_or_else(|| VmError::runtime(format!("{constructor_name} missing")))?;
    let _ = vm;
    construct_intl_object_with_new_target(vm, context, arguments, constructor, kind)
}

fn construct_intl_object_with_new_target(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let data = match kind {
        "DateTimeFormat" => {
            IntlObjectData::DateTimeFormat(initialize_date_time_format(vm, context, arguments)?)
        }
        "NumberFormat" => IntlObjectData::NumberFormat(Box::new(initialize_number_format(
            vm, context, arguments,
        )?)),
        "Collator" => IntlObjectData::Collator(CollatorRecord {
            locale: "en-US".into(),
        }),
        _ => return Err(VmError::type_error("unsupported Intl constructor")),
    };
    let prototype = intl_prototype_from_constructor(vm, context, new_target, kind)?;
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(context, object, INTL_KIND, JsValue::String(kind.into()))?;
    context.set_intl_object_data(object, data);
    Ok(JsValue::Object(object))
}

fn intl_prototype_from_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    new_target: JsValue,
    kind: &str,
) -> Result<ObjectId, VmError> {
    let prototype = match vm.get_property_value_catching_from_builtin(
        new_target.clone(),
        "prototype",
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    if let Some(prototype) = context.value_object(&prototype) {
        return Ok(prototype);
    }

    let global = context.global_object_for_callable(&new_target);
    let intl = match vm.get_property_value_catching_from_builtin(
        JsValue::Object(global),
        "Intl",
        context,
    )? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    let constructor = match vm.get_property_value_catching_from_builtin(intl, kind, context)? {
        Ok(value) => value,
        Err(error) => return Err(vm.throw_value_from_builtin(error)),
    };
    let prototype =
        match vm.get_property_value_catching_from_builtin(constructor, "prototype", context)? {
            Ok(value) => value,
            Err(error) => return Err(vm.throw_value_from_builtin(error)),
        };
    context
        .value_object(&prototype)
        .ok_or_else(|| VmError::runtime(format!("Intl.{kind} prototype missing")))
}

fn date_time_string_option(
    vm: &mut Vm,
    context: &mut NativeContext,
    options: &JsValue,
    key: &str,
    allowed: &[&str],
) -> Result<Option<String>, VmError> {
    let value = vm.get_property_value(options.clone(), key, context)?;
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    let value = vm.to_string_coerce(value, context)?;
    if !allowed.is_empty() && !allowed.contains(&value.as_str()) {
        return Err(VmError::range(format!(
            "invalid Intl.DateTimeFormat {key} option"
        )));
    }
    Ok(Some(value.to_string()))
}

fn initialize_date_time_format(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<DateTimeFormatRecord, VmError> {
    let requested = collect_locale_list(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?
    .into_iter()
    .map(|locale| {
        canonicalize_language_tag(&locale)
            .map_err(|_| VmError::range("invalid Intl.DateTimeFormat locale"))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let options = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if matches!(options, JsValue::Null) {
        return Err(VmError::type_error(
            "Intl.DateTimeFormat options cannot be null",
        ));
    }
    let is_object = context.value_object(&options).is_some();
    let mut locale_options = LocaleOptions::default();
    let mut record = DateTimeFormatRecord::default();
    if is_object {
        locale_options.locale_matcher = date_time_string_option(
            vm,
            context,
            &options,
            "localeMatcher",
            &["lookup", "best fit"],
        )?;
        locale_options.calendar = date_time_string_option(vm, context, &options, "calendar", &[])?;
        locale_options.numbering_system =
            date_time_string_option(vm, context, &options, "numberingSystem", &[])?;
        let hour12 = vm.get_property_value(options.clone(), "hour12", context)?;
        locale_options.hour_cycle = date_time_string_option(
            vm,
            context,
            &options,
            "hourCycle",
            &["h11", "h12", "h23", "h24"],
        )?;
        record.time_zone = match vm.get_property_value(options.clone(), "timeZone", context)? {
            JsValue::Undefined => "UTC".into(),
            value => {
                let value = vm.to_string_coerce(value, context)?;
                MinimalIntlProvider
                    .canonicalize_time_zone(&value)
                    .map_err(VmError::range)?
            }
        };
        record.weekday = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "weekday",
            &["long", "short", "narrow"],
        )?);
        record.era = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "era",
            &["long", "short", "narrow"],
        )?);
        record.year = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "year",
            &["numeric", "2-digit"],
        )?);
        record.month = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "month",
            &["numeric", "2-digit", "long", "short", "narrow"],
        )?);
        record.day = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "day",
            &["numeric", "2-digit"],
        )?);
        record.hour = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "hour",
            &["numeric", "2-digit"],
        )?);
        record.minute = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "minute",
            &["numeric", "2-digit"],
        )?);
        record.second = field_style(date_time_string_option(
            vm,
            context,
            &options,
            "second",
            &["numeric", "2-digit"],
        )?);
        let fractional =
            vm.get_property_value(options.clone(), "fractionalSecondDigits", context)?;
        if !matches!(fractional, JsValue::Undefined) {
            let value = vm.to_number(fractional, context)?;
            if !value.is_finite() || !(1.0..=3.0).contains(&value) {
                return Err(VmError::range("invalid fractionalSecondDigits"));
            }
            record.fractional_second_digits = Some(value.floor() as u8);
        }
        record.time_zone_name = time_zone_name(date_time_string_option(
            vm,
            context,
            &options,
            "timeZoneName",
            &[
                "short",
                "long",
                "shortOffset",
                "longOffset",
                "shortGeneric",
                "longGeneric",
            ],
        )?);
        record.date_style = date_time_style(date_time_string_option(
            vm,
            context,
            &options,
            "dateStyle",
            &["full", "long", "medium", "short"],
        )?);
        record.time_style = date_time_style(date_time_string_option(
            vm,
            context,
            &options,
            "timeStyle",
            &["full", "long", "medium", "short"],
        )?);
        record.hour_cycle = if !matches!(hour12, JsValue::Undefined) {
            Some(if hour12.to_boolean() {
                HourCycle::H12
            } else {
                HourCycle::H23
            })
        } else {
            hour_cycle(locale_options.hour_cycle.as_deref())
        };
    }
    let resolved = resolve_locale(
        &MinimalIntlProvider,
        IntlService::DateTimeFormat,
        &requested,
        &locale_options,
    )
    .map_err(VmError::range)?;
    record.locale = resolved.locale;
    record.calendar = resolved.calendar.unwrap_or_else(|| "gregory".into());
    record.numbering_system = resolved.numbering_system.unwrap_or_else(|| "latn".into());
    Ok(record)
}

fn field_style(value: Option<String>) -> Option<DateTimeFieldStyle> {
    value.map(|value| match value.as_str() {
        "numeric" => DateTimeFieldStyle::Numeric,
        "2-digit" => DateTimeFieldStyle::TwoDigit,
        "narrow" => DateTimeFieldStyle::Narrow,
        "short" => DateTimeFieldStyle::Short,
        _ => DateTimeFieldStyle::Long,
    })
}

fn hour_cycle(value: Option<&str>) -> Option<HourCycle> {
    value.map(|value| match value {
        "h11" => HourCycle::H11,
        "h12" => HourCycle::H12,
        "h24" => HourCycle::H24,
        _ => HourCycle::H23,
    })
}

fn date_time_style(value: Option<String>) -> Option<DateTimeStyle> {
    value.map(|value| match value.as_str() {
        "full" => DateTimeStyle::Full,
        "long" => DateTimeStyle::Long,
        "medium" => DateTimeStyle::Medium,
        _ => DateTimeStyle::Short,
    })
}

fn time_zone_name(value: Option<String>) -> Option<TimeZoneNameStyle> {
    value.map(|value| match value.as_str() {
        "short" => TimeZoneNameStyle::Short,
        "long" => TimeZoneNameStyle::Long,
        "shortOffset" => TimeZoneNameStyle::ShortOffset,
        "longOffset" => TimeZoneNameStyle::LongOffset,
        "shortGeneric" => TimeZoneNameStyle::ShortGeneric,
        _ => TimeZoneNameStyle::LongGeneric,
    })
}

fn number_format_option(
    vm: &mut Vm,
    context: &mut NativeContext,
    options: &JsValue,
    key: &str,
) -> Result<JsValue, VmError> {
    match vm.get_property_value_catching_from_builtin(options.clone(), key, context)? {
        Ok(value) => Ok(value),
        Err(error) => Err(vm.throw_value_from_builtin(error)),
    }
}

fn number_format_string_option(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    allowed: &[&str],
) -> Result<Option<String>, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    let value = vm.to_string_coerce(value, context)?;
    if !allowed.contains(&value.as_str()) {
        return Err(VmError::range("invalid Intl.NumberFormat option"));
    }
    Ok(Some(value.to_string()))
}

fn number_format_get_string_option(
    vm: &mut Vm,
    context: &mut NativeContext,
    options: &JsValue,
    key: &str,
    allowed: &[&str],
) -> Result<Option<String>, VmError> {
    let value = number_format_option(vm, context, options, key)?;
    number_format_string_option(vm, context, value, allowed)
}

fn number_format_digit_option(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    minimum: f64,
    maximum: f64,
) -> Result<Option<f64>, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    let number = vm.to_number(value, context)?;
    if !number.is_finite() || number < minimum || number > maximum {
        return Err(VmError::range(
            "Intl.NumberFormat digit option is out of range",
        ));
    }
    Ok(Some(number.floor()))
}

fn number_format_get_digit_option(
    vm: &mut Vm,
    context: &mut NativeContext,
    options: &JsValue,
    key: &str,
    minimum: f64,
    maximum: f64,
) -> Result<Option<f64>, VmError> {
    let value = number_format_option(vm, context, options, key)?;
    number_format_digit_option(vm, context, value, minimum, maximum)
}

fn is_well_formed_numbering_system(value: &str) -> bool {
    value.split('-').all(|part| {
        (3..=8).contains(&part.len()) && part.chars().all(|ch| ch.is_ascii_alphanumeric())
    })
}

fn is_well_formed_currency(value: &str) -> bool {
    value.len() == 3 && value.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn is_sanctioned_simple_unit(value: &str) -> bool {
    matches!(
        value,
        "acre"
            | "bit"
            | "byte"
            | "celsius"
            | "centimeter"
            | "day"
            | "degree"
            | "fahrenheit"
            | "fluid-ounce"
            | "foot"
            | "gallon"
            | "gigabit"
            | "gigabyte"
            | "gram"
            | "hectare"
            | "hour"
            | "inch"
            | "kilobit"
            | "kilobyte"
            | "kilogram"
            | "kilometer"
            | "liter"
            | "megabit"
            | "megabyte"
            | "meter"
            | "microsecond"
            | "mile"
            | "mile-scandinavian"
            | "milliliter"
            | "millimeter"
            | "millisecond"
            | "minute"
            | "month"
            | "nanosecond"
            | "ounce"
            | "percent"
            | "petabyte"
            | "pound"
            | "second"
            | "stone"
            | "terabit"
            | "terabyte"
            | "week"
            | "yard"
            | "year"
    )
}

fn is_well_formed_unit(value: &str) -> bool {
    if is_sanctioned_simple_unit(value) {
        return true;
    }
    value.split_once("-per-").is_some_and(|(left, right)| {
        is_sanctioned_simple_unit(left) && is_sanctioned_simple_unit(right)
    })
}

fn initialize_number_format(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<NumberFormatRecord, VmError> {
    if matches!(arguments.first(), Some(JsValue::Null)) {
        return Err(VmError::type_error(
            "Intl.NumberFormat locales cannot be null",
        ));
    }
    let requested = collect_locale_list(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?
    .into_iter()
    .map(|locale| {
        canonicalize_language_tag(&locale)
            .map_err(|_| VmError::range("invalid Intl.NumberFormat locale"))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let options = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if matches!(options, JsValue::Null) {
        return Err(VmError::type_error(
            "Intl.NumberFormat options cannot be null",
        ));
    }
    let is_object = matches!(
        options,
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)
    );
    let mut locale_options = LocaleOptions::default();
    let mut record = NumberFormatRecord::default();
    if !is_object {
        let resolved = resolve_locale(
            &MinimalIntlProvider,
            IntlService::NumberFormat,
            &requested,
            &locale_options,
        )
        .map_err(VmError::range)?;
        let base_locale = resolved
            .locale
            .split("-u-")
            .next()
            .unwrap_or(&resolved.locale);
        let extension_numbering = requested
            .first()
            .and_then(|locale| unicode_extension_value(locale, "nu"))
            .filter(|numbering| {
                MinimalIntlProvider
                    .available_numbering_systems()
                    .contains(&numbering.as_str())
            });
        record.numbering_system = extension_numbering.clone().unwrap_or_else(|| "latn".into());
        record.locale = extension_numbering.map_or_else(
            || base_locale.to_string(),
            |numbering| format!("{base_locale}-u-nu-{numbering}"),
        );
        return Ok(record);
    }

    locale_options.locale_matcher = number_format_get_string_option(
        vm,
        context,
        &options,
        "localeMatcher",
        &["lookup", "best fit"],
    )?;
    let numbering_value = number_format_option(vm, context, &options, "numberingSystem")?;
    if !matches!(numbering_value, JsValue::Undefined) {
        let numbering = vm.to_string_coerce(numbering_value, context)?;
        if !is_well_formed_numbering_system(&numbering) {
            return Err(VmError::range("invalid Intl.NumberFormat numberingSystem"));
        }
        if MinimalIntlProvider
            .available_numbering_systems()
            .contains(&numbering.as_str())
        {
            locale_options.numbering_system = Some(numbering.to_string());
        }
    }
    record.style = number_format_get_string_option(
        vm,
        context,
        &options,
        "style",
        &["decimal", "percent", "currency", "unit"],
    )?
    .unwrap_or_else(|| "decimal".into());

    let currency_value = number_format_option(vm, context, &options, "currency")?;
    if !matches!(currency_value, JsValue::Undefined) {
        let currency = vm
            .to_string_coerce(currency_value, context)?
            .to_ascii_uppercase();
        if !is_well_formed_currency(&currency) {
            return Err(VmError::range("invalid Intl.NumberFormat currency"));
        }
        record.currency = Some(currency);
    }
    if record.style == "currency" && record.currency.is_none() {
        return Err(VmError::type_error(
            "currency style requires a currency option",
        ));
    }
    record.currency_display = number_format_get_string_option(
        vm,
        context,
        &options,
        "currencyDisplay",
        &["code", "symbol", "narrowSymbol", "name"],
    )?
    .unwrap_or_else(|| "symbol".into());
    record.currency_sign = number_format_get_string_option(
        vm,
        context,
        &options,
        "currencySign",
        &["standard", "accounting"],
    )?
    .unwrap_or_else(|| "standard".into());

    let unit_value = number_format_option(vm, context, &options, "unit")?;
    if !matches!(unit_value, JsValue::Undefined) {
        let unit = vm.to_string_coerce(unit_value, context)?;
        if !is_well_formed_unit(&unit) {
            return Err(VmError::range("invalid Intl.NumberFormat unit"));
        }
        record.unit = Some(unit.to_string());
    }
    if record.style == "unit" && record.unit.is_none() {
        return Err(VmError::type_error("unit style requires a unit option"));
    }
    record.unit_display = number_format_get_string_option(
        vm,
        context,
        &options,
        "unitDisplay",
        &["short", "narrow", "long"],
    )?
    .unwrap_or_else(|| "short".into());
    record.notation = number_format_get_string_option(
        vm,
        context,
        &options,
        "notation",
        &["standard", "scientific", "engineering", "compact"],
    )?
    .unwrap_or_else(|| "standard".into());
    record.minimum_integer_digits =
        number_format_get_digit_option(vm, context, &options, "minimumIntegerDigits", 1.0, 21.0)?
            .unwrap_or(1.0) as u8;
    let min_fraction =
        number_format_get_digit_option(vm, context, &options, "minimumFractionDigits", 0.0, 100.0)?;
    let max_fraction =
        number_format_get_digit_option(vm, context, &options, "maximumFractionDigits", 0.0, 100.0)?;
    let currency_default: f64 = record
        .currency
        .as_deref()
        .map(|currency| match currency {
            "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3.0,
            "CLF" => 4.0,
            "JPY" | "KRW" => 0.0,
            _ => 2.0,
        })
        .unwrap_or(0.0);
    let default_min_fraction = if record.notation != "standard" {
        0.0
    } else if record.style == "currency" {
        max_fraction.map_or(currency_default, |maximum| currency_default.min(maximum))
    } else {
        0.0
    };
    let default_max_fraction = if record.notation == "compact" {
        0.0
    } else if matches!(record.notation.as_str(), "scientific" | "engineering") {
        3.0
    } else if record.style == "currency" {
        currency_default
    } else if record.style == "percent" {
        0.0
    } else {
        3.0
    };
    record.minimum_fraction_digits = min_fraction.unwrap_or(default_min_fraction).min(100.0) as u8;
    record.maximum_fraction_digits = max_fraction
        .unwrap_or(default_max_fraction.max(f64::from(record.minimum_fraction_digits)))
        .min(100.0) as u8;
    if record.maximum_fraction_digits < record.minimum_fraction_digits {
        return Err(VmError::range(
            "maximumFractionDigits is less than minimumFractionDigits",
        ));
    }
    let minimum_significant_digits = number_format_get_digit_option(
        vm,
        context,
        &options,
        "minimumSignificantDigits",
        1.0,
        21.0,
    )?
    .map(|value| value as u8);
    record.minimum_significant_digits_explicit = minimum_significant_digits.is_some();
    record.minimum_significant_digits = minimum_significant_digits;
    record.maximum_significant_digits = number_format_get_digit_option(
        vm,
        context,
        &options,
        "maximumSignificantDigits",
        1.0,
        21.0,
    )?
    .map(|value| value as u8);
    if record.minimum_significant_digits.is_some() || record.maximum_significant_digits.is_some() {
        record.minimum_significant_digits = Some(record.minimum_significant_digits.unwrap_or(1));
        record.maximum_significant_digits = Some(record.maximum_significant_digits.unwrap_or(21));
    }
    if let Some(maximum) = record.maximum_significant_digits {
        let minimum = record.minimum_significant_digits.unwrap_or(1);
        if maximum < minimum {
            return Err(VmError::range(
                "maximumSignificantDigits is less than minimumSignificantDigits",
            ));
        }
    }
    let increment_value = number_format_option(vm, context, &options, "roundingIncrement")?;
    if !matches!(increment_value, JsValue::Undefined) {
        let increment = vm.to_number(increment_value, context)?;
        const ALLOWED: &[f64] = &[
            1.0, 2.0, 5.0, 10.0, 20.0, 25.0, 50.0, 100.0, 200.0, 250.0, 500.0, 1000.0, 2000.0,
            2500.0, 5000.0,
        ];
        if !ALLOWED.contains(&increment) {
            return Err(VmError::range(
                "invalid Intl.NumberFormat roundingIncrement",
            ));
        }
        record.rounding_increment = increment as u16;
    }
    record.rounding_mode = number_format_get_string_option(
        vm,
        context,
        &options,
        "roundingMode",
        &[
            "ceil",
            "floor",
            "expand",
            "trunc",
            "halfCeil",
            "halfFloor",
            "halfExpand",
            "halfTrunc",
            "halfEven",
        ],
    )?
    .unwrap_or_else(|| "halfExpand".into());
    record.rounding_priority = number_format_get_string_option(
        vm,
        context,
        &options,
        "roundingPriority",
        &["auto", "morePrecision", "lessPrecision"],
    )?
    .unwrap_or_else(|| "auto".into());
    record.trailing_zero_display = number_format_get_string_option(
        vm,
        context,
        &options,
        "trailingZeroDisplay",
        &["auto", "stripIfInteger"],
    )?
    .unwrap_or_else(|| "auto".into());
    if record.rounding_increment != 1 {
        if record.rounding_priority != "auto"
            || record.minimum_significant_digits.is_some()
            || record.maximum_significant_digits.is_some()
        {
            return Err(VmError::type_error(
                "roundingIncrement is incompatible with significant-digit rounding",
            ));
        }
        if record.minimum_fraction_digits != record.maximum_fraction_digits {
            return Err(VmError::range(
                "roundingIncrement requires equal fraction digit bounds",
            ));
        }
    }
    record.compact_display = number_format_get_string_option(
        vm,
        context,
        &options,
        "compactDisplay",
        &["short", "long"],
    )?
    .unwrap_or_else(|| "short".into());
    record.use_grouping = match number_format_option(vm, context, &options, "useGrouping")? {
        JsValue::Boolean(false) | JsValue::Null | JsValue::Number(0.0) => "false".into(),
        JsValue::Boolean(true) => "always".into(),
        JsValue::Undefined => {
            if record.notation == "compact" {
                "min2".into()
            } else {
                "auto".into()
            }
        }
        JsValue::String(value) if value.is_empty() => "false".into(),
        JsValue::String(value) if value == "false" => "auto".into(),
        JsValue::String(value) if value == "true" => "auto".into(),
        JsValue::String(value) if matches!(value.as_str(), "auto" | "always" | "min2") => {
            value.to_string()
        }
        other => {
            let value = vm.to_string_coerce(other, context)?;
            if matches!(value.as_str(), "auto" | "always" | "min2") {
                value.to_string()
            } else {
                return Err(VmError::range("invalid Intl.NumberFormat useGrouping"));
            }
        }
    };
    record.sign_display = number_format_get_string_option(
        vm,
        context,
        &options,
        "signDisplay",
        &["auto", "never", "always", "exceptZero", "negative"],
    )?
    .unwrap_or_else(|| "auto".into());

    let resolved = resolve_locale(
        &MinimalIntlProvider,
        IntlService::NumberFormat,
        &requested,
        &locale_options,
    )
    .map_err(VmError::range)?;
    let base_locale = resolved
        .locale
        .split("-u-")
        .next()
        .unwrap_or(&resolved.locale)
        .to_string();
    let extension_numbering = requested
        .first()
        .and_then(|locale| unicode_extension_value(locale, "nu"))
        .filter(|numbering| {
            MinimalIntlProvider
                .available_numbering_systems()
                .contains(&numbering.as_str())
        });
    record.numbering_system = locale_options
        .numbering_system
        .clone()
        .or_else(|| extension_numbering.clone())
        .unwrap_or_else(|| "latn".into());
    record.locale = if extension_numbering.as_deref() == Some(record.numbering_system.as_str()) {
        format!("{base_locale}-u-nu-{}", record.numbering_system)
    } else {
        base_locale
    };
    Ok(record)
}

fn require_intl_kind(
    context: &NativeContext,
    this_value: &JsValue,
    expected: &'static str,
) -> Result<ObjectId, VmError> {
    let object = object_from_this(context, this_value, "Intl receiver")?;
    match own_data_value(context, object, INTL_KIND) {
        Some(JsValue::String(kind)) if kind == expected => Ok(object),
        _ => Err(VmError::type_error(format!(
            "receiver is not an Intl.{expected} object"
        ))),
    }
}

fn intl_fallback_symbol(context: &NativeContext) -> Option<SymbolId> {
    let intl = context.get_global("Intl")?;
    let intl = context.value_object(&intl)?;
    match own_data_value(context, intl, INTL_FALLBACK_SYMBOL) {
        Some(JsValue::Symbol(symbol)) => Some(symbol),
        _ => None,
    }
}

fn unwrap_number_format(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: &JsValue,
) -> Result<ObjectId, VmError> {
    if let Some(object) = context.value_object(this_value)
        && matches!(
            own_data_value(context, object, INTL_KIND),
            Some(JsValue::String(kind)) if kind == "NumberFormat"
        )
    {
        return Ok(object);
    }

    object_from_this(context, this_value, "Intl.NumberFormat receiver")?;
    let symbol = intl_fallback_symbol(context)
        .ok_or_else(|| VmError::runtime("Intl fallback symbol missing"))?;
    let fallback = proxy::internal_get(
        vm,
        context,
        this_value.clone(),
        &PropertyKey::Symbol(symbol),
        this_value.clone(),
    )?;
    require_intl_kind(context, &fallback, "NumberFormat")
}

fn object_from_pairs(
    context: &mut NativeContext,
    pairs: impl IntoIterator<Item = (&'static str, JsValue)>,
) -> Result<JsValue, VmError> {
    let object = new_ordinary_object(context, context.object_prototype())?;
    for (key, value) in pairs {
        context.define_own_property(object, key.into(), PropertyDescriptor::data(value))?;
    }
    Ok(JsValue::Object(object))
}

fn intl_date_time_format_resolved_options(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_intl_kind(context, &this_value, "DateTimeFormat")?;
    let record = match context.intl_object_data(object) {
        Some(IntlObjectData::DateTimeFormat(record)) => record.clone(),
        _ => return Err(VmError::type_error("invalid Intl.DateTimeFormat state")),
    };
    let mut pairs = vec![
        ("locale", JsValue::String(record.locale.into())),
        ("calendar", JsValue::String(record.calendar.into())),
        (
            "numberingSystem",
            JsValue::String(record.numbering_system.into()),
        ),
        ("timeZone", JsValue::String(record.time_zone.into())),
    ];
    if let Some(hour_cycle) = record.hour_cycle {
        let hour_cycle = match hour_cycle {
            HourCycle::H11 => "h11",
            HourCycle::H12 => "h12",
            HourCycle::H23 => "h23",
            HourCycle::H24 => "h24",
        };
        pairs.push(("hourCycle", JsValue::String(hour_cycle.into())));
        pairs.push((
            "hour12",
            JsValue::Boolean(matches!(hour_cycle, "h11" | "h12")),
        ));
    }
    object_from_pairs(context, pairs)
}

fn intl_number_format_resolved_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = unwrap_number_format(vm, context, &this_value)?;
    let record = match context.intl_object_data(object) {
        Some(IntlObjectData::NumberFormat(record)) => record.as_ref().clone(),
        _ => return Err(VmError::type_error("invalid Intl.NumberFormat state")),
    };
    let mut pairs = vec![
        ("locale", JsValue::String(record.locale.into())),
        (
            "numberingSystem",
            JsValue::String(record.numbering_system.into()),
        ),
        ("style", JsValue::String(record.style.clone().into())),
    ];
    if record.style == "currency" {
        if let Some(currency) = record.currency {
            pairs.push(("currency", JsValue::String(currency.into())));
        }
        pairs.push((
            "currencyDisplay",
            JsValue::String(record.currency_display.into()),
        ));
        pairs.push(("currencySign", JsValue::String(record.currency_sign.into())));
    } else if record.style == "unit" {
        if let Some(unit) = record.unit {
            pairs.push(("unit", JsValue::String(unit.into())));
        }
        pairs.push(("unitDisplay", JsValue::String(record.unit_display.into())));
    }
    pairs.push((
        "minimumIntegerDigits",
        JsValue::Number(record.minimum_integer_digits.into()),
    ));
    if let Some(minimum) = record.minimum_significant_digits {
        pairs.push(("minimumSignificantDigits", JsValue::Number(minimum.into())));
        pairs.push((
            "maximumSignificantDigits",
            JsValue::Number(record.maximum_significant_digits.unwrap_or(21).into()),
        ));
    } else {
        pairs.push((
            "minimumFractionDigits",
            JsValue::Number(record.minimum_fraction_digits.into()),
        ));
        pairs.push((
            "maximumFractionDigits",
            JsValue::Number(record.maximum_fraction_digits.into()),
        ));
    }
    pairs.push((
        "useGrouping",
        if record.use_grouping == "false" {
            JsValue::Boolean(false)
        } else {
            JsValue::String(record.use_grouping.into())
        },
    ));
    pairs.push(("notation", JsValue::String(record.notation.clone().into())));
    if record.notation == "compact" {
        pairs.push((
            "compactDisplay",
            JsValue::String(record.compact_display.into()),
        ));
    }
    pairs.push(("signDisplay", JsValue::String(record.sign_display.into())));
    pairs.push((
        "roundingIncrement",
        JsValue::Number(record.rounding_increment.into()),
    ));
    pairs.push(("roundingMode", JsValue::String(record.rounding_mode.into())));
    pairs.push((
        "roundingPriority",
        JsValue::String(record.rounding_priority.into()),
    ));
    pairs.push((
        "trailingZeroDisplay",
        JsValue::String(record.trailing_zero_display.into()),
    ));
    object_from_pairs(context, pairs)
}

fn intl_collator_resolved_options(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "Collator")?;
    object_from_pairs(
        context,
        [
            ("locale", JsValue::String("en-US".into())),
            ("usage", JsValue::String("sort".into())),
            ("sensitivity", JsValue::String("variant".into())),
            ("ignorePunctuation", JsValue::Boolean(false)),
            ("collation", JsValue::String("default".into())),
            ("numeric", JsValue::Boolean(false)),
            ("caseFirst", JsValue::String("false".into())),
        ],
    )
}

fn intl_supported_locales_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locales = collect_locale_list(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let supported = locales
        .into_iter()
        .filter(|locale| matches!(locale.as_str(), "en" | "en-US" | "und"))
        .map(|value: String| JsValue::String(value.into()))
        .collect();
    context.create_array(supported)
}

fn collect_locale_list(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Vec<String>, VmError> {
    match value {
        JsValue::Undefined => Ok(Vec::new()),
        JsValue::String(locale) => Ok(vec![locale.to_string()]),
        other => {
            let object = vm.to_object(other, context)?;
            let object_value = context.object_value(object);
            let length_value = vm.get_property_value(object_value.clone(), "length", context)?;
            let length = vm.to_number(length_value, context)?.max(0.0) as usize;
            let mut locales = Vec::new();
            for index in 0..length {
                let key = index.to_string();
                if proxy::internal_has_property(
                    vm,
                    context,
                    object_value.clone(),
                    &PropertyKey::String(key.clone()),
                )? {
                    let value = vm.get_property_value(object_value.clone(), &key, context)?;
                    if !matches!(
                        value,
                        JsValue::String(_)
                            | JsValue::Object(_)
                            | JsValue::Function(_)
                            | JsValue::BuiltinFunction(_)
                    ) {
                        return Err(VmError::type_error(
                            "Intl locale list elements must be strings or objects",
                        ));
                    }
                    locales.push(vm.to_string_coerce(value, context)?.to_string());
                }
            }
            Ok(locales)
        }
    }
}

fn intl_number_format_format(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_intl_kind(context, &this_value, "NumberFormat")?;
    let record = match context.intl_object_data(object) {
        Some(IntlObjectData::NumberFormat(record)) => record.as_ref().clone(),
        _ => return Err(VmError::type_error("invalid Intl.NumberFormat state")),
    };
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let formatted = if let JsValue::String(text) = &value
        && !matches!(text.as_str(), "Infinity" | "-Infinity" | "+Infinity")
    {
        format_number(&record, NumberValue::Decimal(text))
    } else {
        match vm.to_numeric(value, context)? {
            JsValue::Number(number) => format_number(&record, NumberValue::Number(number)),
            JsValue::BigInt(number) => {
                let text = number.to_string();
                format_number(&record, NumberValue::BigInt(&text))
            }
            _ => unreachable!("ToNumeric must return Number or BigInt"),
        }
    };
    Ok(JsValue::String(formatted.text.into()))
}

fn intl_number_to_locale_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let number = match &this_value {
        JsValue::Number(number) => *number,
        _ => context
            .value_object(&this_value)
            .and_then(|object| context.primitive_value(object))
            .and_then(|value| match value {
                PrimitiveValue::Number(number) => Some(*number),
                _ => None,
            })
            .ok_or_else(|| {
                VmError::type_error("Number.prototype.toLocaleString called on a non-Number")
            })?,
    };
    let record = initialize_number_format(vm, context, arguments)?;
    Ok(JsValue::String(
        format_number(&record, NumberValue::Number(number))
            .text
            .into(),
    ))
}

fn intl_number_format_format_get(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = unwrap_number_format(vm, context, &this_value)?;
    if let Some(bound) = own_data_value(context, object, NUMBER_FORMAT_BOUND_FORMAT) {
        return Ok(bound);
    }
    let target = context.register_builtin("", 1, intl_number_format_format, None)?;
    let bound =
        context.register_bound_function(target, this_value, Vec::new(), 1.0, String::new())?;
    define_hidden(context, object, NUMBER_FORMAT_BOUND_FORMAT, bound.clone())?;
    Ok(bound)
}

fn intl_collator_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "Collator")?;
    let left = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let right = vm.to_string_coerce(
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    Ok(JsValue::Number(match left.cmp(&right) {
        std::cmp::Ordering::Less => -1.0,
        std::cmp::Ordering::Equal => 0.0,
        std::cmp::Ordering::Greater => 1.0,
    }))
}

pub(super) fn install_test262_host_object(context: &mut NativeContext) {
    let _ = install_test262_host_object_inner(context);
}

fn install_test262_host_object_inner(context: &mut NativeContext) -> Result<(), VmError> {
    let host = new_ordinary_object(context, context.object_prototype())?;
    let eval_script = context.register_builtin("evalScript", 1, test262_eval_script, None)?;
    let gc = context.register_builtin("gc", 0, test262_gc, None)?;
    let detach =
        context.register_builtin("detachArrayBuffer", 1, test262_detach_array_buffer, None)?;
    let create_realm = context.register_builtin("createRealm", 0, test262_create_realm, None)?;
    let build_string = context.register_builtin("buildString", 1, test262_build_string, None)?;
    let agent = install_test262_agent(context)?;
    context.define_own_property(
        host,
        "global".into(),
        method_descriptor(context.global_this_value()),
    )?;
    context.define_own_property(host, "evalScript".into(), method_descriptor(eval_script))?;
    context.define_own_property(host, "gc".into(), method_descriptor(gc))?;
    context.define_own_property(host, "detachArrayBuffer".into(), method_descriptor(detach))?;
    context.define_own_property(host, "createRealm".into(), method_descriptor(create_realm))?;
    context.define_own_property(host, "buildString".into(), method_descriptor(build_string))?;
    context.define_own_property(
        host,
        "agent".into(),
        method_descriptor(JsValue::Object(agent)),
    )?;
    context.declare_global("$262", JsValue::Object(host));
    Ok(())
}

fn install_test262_agent(context: &mut NativeContext) -> Result<ObjectId, VmError> {
    let agent = new_ordinary_object(context, context.object_prototype())?;
    for (name, length, call) in [
        ("start", 1, test262_agent_start as NativeCall),
        ("broadcast", 1, test262_agent_broadcast as NativeCall),
        (
            "receiveBroadcast",
            1,
            test262_agent_receive_broadcast as NativeCall,
        ),
        ("report", 1, test262_agent_report as NativeCall),
        ("getReport", 0, test262_agent_get_report as NativeCall),
        ("sleep", 1, test262_agent_sleep as NativeCall),
        ("monotonicNow", 0, test262_agent_monotonic_now as NativeCall),
        ("leaving", 0, test262_agent_leaving as NativeCall),
    ] {
        let method = context.register_builtin(name, length, call, None)?;
        context.define_own_property(agent, name.into(), method_descriptor(method))?;
    }
    Ok(agent)
}

fn test262_agent_start(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    context.agent_start_worker();
    let isolated_source = JsValue::String(format!("\"use strict\";\n{source}").into());
    let result = function::eval_direct_call(
        vm,
        context,
        JsValue::Undefined,
        std::slice::from_ref(&isolated_source),
    );
    context.agent_finish_start();
    result.map(|_| JsValue::Undefined)
}

fn test262_agent_receive_broadcast(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let receiver = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !context.is_callable_value(&receiver) {
        return Err(VmError::type_error(
            "$262.agent.receiveBroadcast requires a callback",
        ));
    }
    if !context.agent_set_receiver(receiver) {
        return Err(VmError::runtime(
            "$262.agent.receiveBroadcast called outside an agent",
        ));
    }
    Ok(JsValue::Undefined)
}

fn test262_agent_broadcast(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    for (worker, receiver) in context.agent_receivers() {
        context.agent_enter_worker(worker);
        let result =
            vm.call_value_from_builtin(receiver, JsValue::Undefined, vec![value.clone()], context);
        context.agent_leave_worker_call();
        result?;
    }
    Ok(JsValue::Undefined)
}

fn test262_agent_report(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let report = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    context.agent_report(report.to_string());
    Ok(JsValue::Undefined)
}

fn test262_agent_get_report(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(context
        .agent_get_report()
        .map(|value: String| JsValue::String(value.into()))
        .unwrap_or(JsValue::Null))
}

fn test262_agent_sleep(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let duration = vm
        .to_number(
            arguments.first().cloned().unwrap_or(JsValue::Undefined),
            context,
        )?
        .max(0.0);
    context.agent_sleep(duration);
    Ok(JsValue::Undefined)
}

fn test262_agent_leaving(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.agent_mark_leaving();
    Ok(JsValue::Undefined)
}

fn test262_agent_monotonic_now(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(context.agent_monotonic_now()))
}

fn test262_build_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let args = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let lone_code_points = vm.get_property_value(args.clone(), "loneCodePoints", context)?;
    let ranges = vm.get_property_value(args, "ranges", context)?;
    let mut units = Vec::new();
    append_code_point_array(vm, context, lone_code_points, &mut units)?;

    let range_count = array_like_len(vm, context, ranges.clone())?;
    for index in 0..range_count {
        let range = vm.get_property_value(ranges.clone(), &index.to_string(), context)?;
        let start = code_point_from_value(
            vm.get_property_value(range.clone(), "0", context)?,
            "buildString range start",
        )?;
        let end = code_point_from_value(
            vm.get_property_value(range, "1", context)?,
            "buildString range end",
        )?;
        if start > end {
            continue;
        }
        units.reserve(((end - start + 1) as usize).saturating_mul(2));
        for code_point in start..=end {
            push_code_point_units(&mut units, code_point)?;
        }
    }

    Ok(JsValue::String(string::decode_utf16(&units).into()))
}

fn append_code_point_array(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    units: &mut Vec<u16>,
) -> Result<(), VmError> {
    let length = array_like_len(vm, context, value.clone())?;
    units.reserve(length);
    for index in 0..length {
        let value = vm.get_property_value(value.clone(), &index.to_string(), context)?;
        let code_point = code_point_from_value(value, "buildString code point")?;
        push_code_point_units(units, code_point)?;
    }
    Ok(())
}

fn array_like_len(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<usize, VmError> {
    let length = vm
        .get_property_value(value, "length", context)?
        .to_js_string()
        .and_then(|text| text.parse::<usize>().ok())
        .unwrap_or(0);
    Ok(length)
}

fn code_point_from_value(value: JsValue, label: &str) -> Result<u32, VmError> {
    let Some(number) = value.to_number() else {
        return Err(VmError::range(format!("{label} is not a number")));
    };
    if number < 0.0 || number > f64::from(0x10_FFFF) || number.trunc() != number {
        return Err(VmError::range(format!(
            "{number} is not a valid code point"
        )));
    }
    Ok(number as u32)
}

fn push_code_point_units(units: &mut Vec<u16>, code_point: u32) -> Result<(), VmError> {
    match code_point {
        0..=0xFFFF => units.push(code_point as u16),
        0x10000..=0x10FFFF => {
            let value = code_point - 0x10000;
            units.push(0xD800 | ((value >> 10) as u16));
            units.push(0xDC00 | ((value & 0x3FF) as u16));
        }
        _ => return Err(VmError::range("invalid code point in buildString")),
    }
    Ok(())
}

fn test262_eval_script(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let realm = match this {
        JsValue::Object(host) => context.realm_for_host(host),
        _ => None,
    };
    let Some(realm) = realm else {
        return function::eval_call(vm, context, JsValue::Undefined, arguments);
    };
    if context.is_current_realm(realm) {
        return function::eval_call(vm, context, JsValue::Undefined, arguments);
    }
    let activation = context.enter_realm(realm)?;
    let result = function::eval_call(vm, context, JsValue::Undefined, arguments);
    let leave_result = context.leave_realm(activation);
    match (result, leave_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(_)) => Err(error),
    }
}

fn test262_gc(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Undefined)
}

fn test262_detach_array_buffer(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let object = context.require_object(&value, "$262.detachArrayBuffer")?;
    let buffer = array_buffer_id_from_object(context, object)?;
    context.detach_array_buffer(buffer)?;
    context.define_own_property(
        object,
        ARRAY_BUFFER_DETACHED.into(),
        hidden_slot_descriptor(JsValue::Boolean(true)),
    )?;
    context.define_own_property(
        object,
        ARRAY_BUFFER_BYTE_LENGTH.into(),
        hidden_slot_descriptor(JsValue::Number(0.0)),
    )?;
    Ok(JsValue::Undefined)
}

fn test262_create_realm(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (global_environment, global_object) = context.allocate_realm_globals()?;
    let activation = context.enter_uninitialized_realm(global_environment, global_object)?;
    install_foundation(context);
    install_test262_harness(context);
    let host = context
        .get_global("$262")
        .ok_or_else(|| VmError::runtime("new realm $262 host missing"))?;
    let realm = context.register_current_realm()?;
    let JsValue::Object(host_object) = host.clone() else {
        let _ = context.leave_realm(activation);
        return Err(VmError::runtime("new realm $262 host is not an object"));
    };
    context.register_realm_host(host_object, realm);
    context.leave_realm(activation)?;
    Ok(host)
}
