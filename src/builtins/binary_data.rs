//! Binary-data built-ins: ArrayBuffer, DataView, TypedArray constructors, and Intl skeleton.
//!
//! This module intentionally installs honest first-stage shapes: constructors,
//! prototypes, descriptors, and deterministic Intl option objects. Operations
//! that need real typed storage are present only when they can fail explicitly.

use super::{function, install_foundation, install_test262_harness};
use crate::{
    runtime::{
        ArrayBufferId, DataViewId, IteratorMode, JsObject, JsValue, NativeCall, NativeContext,
        ObjectId, ObjectKind, PreferredType, PropertyDescriptor, PropertyKind,
        TypedArrayElementKind, TypedArrayViewId,
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
const MAX_SKELETON_BUFFER_BYTES: usize = 1 << 24;

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
    if integer > MAX_SKELETON_BUFFER_BYTES as f64 {
        return Err(VmError::range("buffer length exceeds V8 skeleton limit"));
    }
    Ok(integer as usize)
}

fn to_length(vm: &mut Vm, context: &mut NativeContext, value: JsValue) -> Result<usize, VmError> {
    let number = vm.to_number(value, context)?;
    if number.is_nan() || number <= 0.0 {
        return Ok(0);
    }
    if number.is_infinite() || number > MAX_SKELETON_BUFFER_BYTES as f64 {
        return Ok(MAX_SKELETON_BUFFER_BYTES);
    }
    Ok(number.floor() as usize)
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
    let byte_length_getter =
        context.register_builtin("get byteLength", 0, array_buffer_byte_length_get, None)?;
    context.define_own_property(
        prototype,
        "byteLength".into(),
        PropertyDescriptor::accessor(Some(byte_length_getter), None, false, true),
    )?;
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
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("SharedArrayBuffer prototype missing"))?;
    create_array_buffer_object_with_options(
        context,
        byte_length,
        byte_length,
        false,
        false,
        prototype,
    )
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
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("ArrayBuffer prototype missing"))?;
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
    let object = context.require_object(&options, "ArrayBuffer options")?;
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

fn array_buffer_byte_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.byteLength")?;
    let buffer = array_buffer_id_from_object(context, object)?;
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
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.maxByteLength")?;
    let buffer = array_buffer_id_from_object(context, object)?;
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
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.resizable")?;
    let buffer = array_buffer_id_from_object(context, object)?;
    Ok(JsValue::Boolean(context.is_array_buffer_resizable(buffer)?))
}

fn array_buffer_detached_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.detached")?;
    let buffer = array_buffer_id_from_object(context, object)?;
    Ok(JsValue::Boolean(context.is_array_buffer_detached(buffer)?))
}

fn array_buffer_immutable_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.immutable")?;
    let buffer = array_buffer_id_from_object(context, object)?;
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
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.slice")?;
    let buffer = array_buffer_id_from_object(context, object)?;
    let length = context.array_buffer_byte_length(buffer)?;
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 1, length as f64)?,
        length,
    );
    let end = end.max(start);
    let copy = context.clone_array_buffer_range(buffer, start, end)?;
    let prototype = context
        .get_prototype_of(object)
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("ArrayBuffer prototype missing"))?;
    create_array_buffer_object_with_id(context, copy, prototype)
}

fn array_buffer_resize(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.resize")?;
    let buffer = array_buffer_id_from_object(context, object)?;
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
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.transfer")?;
    let buffer = array_buffer_id_from_object(context, object)?;
    let new_byte_length = match arguments.first() {
        None | Some(JsValue::Undefined) => None,
        Some(value) => Some(to_index(vm, context, value.clone())?),
    };
    let target = context.transfer_array_buffer(buffer, new_byte_length, immutable)?;
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
    array_buffer_transfer_common(vm, context, this_value, arguments, false)
}

fn array_buffer_transfer_to_fixed_length(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_transfer_common(vm, context, this_value, arguments, false)
}

fn array_buffer_transfer_to_immutable(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_transfer_common(vm, context, this_value, arguments, true)
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
    let buffer = array_buffer_id_from_object(context, object)?;
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
) -> Result<i128, VmError> {
    match value {
        JsValue::BigInt(value) => Ok(value),
        JsValue::Boolean(value) => Ok(i128::from(value)),
        JsValue::String(value) => parse_bigint_string(&value)
            .ok_or_else(|| VmError::syntax_error("Cannot convert string to BigInt")),
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            let primitive = vm.to_primitive(value, PreferredType::Number, context)?;
            to_bigint_for_data_view(vm, context, primitive)
        }
        _ => Err(VmError::type_error("Cannot convert value to BigInt")),
    }
}

fn parse_bigint_string(input: &str) -> Option<i128> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    let (negative, unsigned) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest)
    } else {
        (false, trimmed)
    };
    if (negative || trimmed.starts_with('+'))
        && (unsigned.starts_with("0x")
            || unsigned.starts_with("0X")
            || unsigned.starts_with("0b")
            || unsigned.starts_with("0B")
            || unsigned.starts_with("0o")
            || unsigned.starts_with("0O"))
    {
        return None;
    }
    let (digits, radix) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
        .map(|digits| (digits, 16))
        .or_else(|| {
            unsigned
                .strip_prefix("0b")
                .or_else(|| unsigned.strip_prefix("0B"))
                .map(|digits| (digits, 2))
        })
        .or_else(|| {
            unsigned
                .strip_prefix("0o")
                .or_else(|| unsigned.strip_prefix("0O"))
                .map(|digits| (digits, 8))
        })
        .unwrap_or((unsigned, 10));
    if digits.is_empty() {
        return None;
    }
    let value = i128::from_str_radix(digits, radix).ok()?;
    Some(if negative { -value } else { value })
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
        ("toLocaleString", 0, typed_array_to_string as NativeCall),
        ("toReversed", 0, typed_array_to_reversed as NativeCall),
        ("toSorted", 1, typed_array_to_sorted as NativeCall),
        ("toString", 0, typed_array_to_string as NativeCall),
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
    let constructor =
        context.register_builtin(name, 3, typed_array_call, Some(typed_array_construct))?;
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

    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        to_string_tag,
        readonly_configurable_descriptor(JsValue::String(name.into())),
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

fn typed_array_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let constructor_object = context
        .value_object(&new_target)
        .ok_or_else(|| VmError::type_error("typed array new target must be a constructor"))?;
    let name = match own_data_value(context, constructor_object, TYPED_ARRAY_NAME) {
        Some(JsValue::String(name)) => name,
        _ => "TypedArray".into(),
    };
    let kind = typed_array_kind(&name)?;
    let bytes_per_element = kind.bytes_per_element();
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("typed array prototype missing"))?;
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
            ((buffer_length - byte_offset) / bytes_per_element, true)
        };
        return create_typed_array_object_with_tracking(
            context,
            prototype,
            name,
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
    let buffer = create_array_buffer_object(context, byte_length, array_buffer_proto)?;
    let JsValue::Object(buffer_object) = buffer else {
        unreachable!()
    };
    let buffer_id = array_buffer_id_from_object(context, buffer_object)?;
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
    let result_object = context.require_object(&result, "TypedArray result")?;
    let view = typed_array_view_id_from_object(context, result_object)?;
    for (index, value) in values.into_iter().enumerate() {
        context.typed_array_store_element(view, index, value)?;
    }
    Ok(result)
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
    if matches!(source, JsValue::Number(_)) {
        let length = to_index(vm, context, source)?;
        let zero = if kind.is_bigint() {
            JsValue::BigInt(0)
        } else {
            JsValue::Number(0.0)
        };
        return Ok(vec![zero; length]);
    }
    collect_typed_array_source_values(vm, context, source)
}

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
    define_hidden(context, object, TYPED_ARRAY_NAME, JsValue::String(name))?;
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

fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn require_callable(value: &JsValue, label: &str) -> Result<(), VmError> {
    if is_callable(value) {
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
    collect_iterator_values(vm, context, iterator).map(Some)
}

fn collect_iterator_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    if let Some(object) = context.value_object(&iterator)
        && (context.is_array_object(object).unwrap_or(false)
            || context.typed_array_indexed_view(object).is_some())
    {
        return collect_array_like_values(vm, context, iterator);
    }

    let next = vm.get_property_value(iterator.clone(), "next", context)?;
    require_callable(&next, "TypedArray iterator next")?;
    let mut values = Vec::new();
    while values.len() < MAX_SKELETON_BUFFER_BYTES {
        let step =
            vm.call_value_from_builtin(next.clone(), iterator.clone(), Vec::new(), context)?;
        let step_object = context.require_object(&step, "TypedArray iterator result")?;
        let done = vm
            .get_property_value(context.object_value(step_object), "done", context)?
            .to_boolean();
        if done {
            return Ok(values);
        }
        values.push(vm.get_property_value(context.object_value(step_object), "value", context)?);
    }
    Err(VmError::runtime_limit(
        "TypedArray iterator result limit exceeded",
    ))
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
        context.typed_array_store_element(view, index, value)?;
    }
    Ok(result)
}

fn store_values_into_typed_array(
    context: &mut NativeContext,
    target: JsValue,
    values: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let object = context.require_object(&target, "TypedArray target")?;
    let view = typed_array_view_id_from_object(context, object)?;
    let (_, length) = context
        .typed_array_indexed_view(object)
        .ok_or_else(|| VmError::type_error("target is not a TypedArray"))?;
    if values.len() > length {
        return Err(VmError::range("source is too large for TypedArray target"));
    }
    for (index, value) in values.into_iter().enumerate() {
        context.typed_array_store_element(view, index, value)?;
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
    store_values_into_typed_array(context, target, values)
}

fn typed_array_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let mut values = collect_typed_array_source_values(vm, context, source)?;
    let map_fn = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !matches!(map_fn, JsValue::Undefined) {
        require_callable(&map_fn, "TypedArray.from")?;
        let this_arg = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
        for (index, value) in values.iter_mut().enumerate() {
            *value = vm.call_value_from_builtin(
                map_fn.clone(),
                this_arg.clone(),
                vec![value.clone(), JsValue::Number(index as f64)],
                context,
            )?;
        }
    }
    construct_typed_array_with_values(vm, context, this_value, values)
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
    context.typed_array_load_element(view, index)
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
        Some(value) => vm.to_string_coerce(value.clone(), context)?,
    };
    let mut parts = Vec::with_capacity(length);
    for index in 0..length {
        parts.push(vm.to_string_coerce(context.typed_array_load_element(view, index)?, context)?);
    }
    Ok(JsValue::String(parts.join(&sep)))
}

fn typed_array_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    typed_array_join(vm, context, this_value, &[])
}

fn typed_array_fill(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.fill")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 2, length as f64)?,
        length,
    );
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
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    for index in start..length {
        if context
            .typed_array_load_element(view, index)?
            .same_value(&search)
        {
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
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.indexOf")?;
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    for index in start..length {
        if context
            .typed_array_load_element(view, index)?
            .strict_equals(&search)
        {
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
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.lastIndexOf")?;
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let raw = argument_integer(vm, context, arguments, 1, (length - 1) as f64)?;
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
        if context
            .typed_array_load_element(view, index)?
            .strict_equals(&search)
        {
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
                context.typed_array_load_element(view, index)?,
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
        let value = context.typed_array_load_element(view, index)?;
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
        let value = context.typed_array_load_element(view, index)?;
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
        let value = context.typed_array_load_element(view, index)?;
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
    let (object, view, length, name, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.map")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.map")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        values.push(vm.call_value_from_builtin(
            callback.clone(),
            this_arg.clone(),
            vec![
                context.typed_array_load_element(view, index)?,
                JsValue::Number(index as f64),
                this_value.clone(),
            ],
            context,
        )?);
    }
    create_typed_array_from_values(context, object, name, kind, values)
}

fn typed_array_filter(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, name, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.filter")?;
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "TypedArray.prototype.filter")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut values = Vec::new();
    for index in 0..length {
        let value = context.typed_array_load_element(view, index)?;
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
    create_typed_array_from_values(context, object, name, kind, values)
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
        context.typed_array_load_element(view, iter.next().unwrap())?
    };
    for index in iter {
        acc = vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![
                acc,
                context.typed_array_load_element(view, index)?,
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
    let target =
        normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 1, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 2, length as f64)?,
        length,
    );
    let count = end.saturating_sub(start).min(length.saturating_sub(target));
    let mut values = Vec::with_capacity(count);
    for index in start..start + count {
        values.push(context.typed_array_load_element(view, index)?);
    }
    for (offset, value) in values.into_iter().enumerate() {
        context.typed_array_store_element(view, target + offset, value)?;
    }
    Ok(this_value)
}

fn typed_array_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.set")?;
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let offset = to_index(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let values = collect_array_like_values(vm, context, source)?;
    if offset
        .checked_add(values.len())
        .is_none_or(|end| end > length)
    {
        return Err(VmError::range("source is too large for target TypedArray"));
    }
    for (index, value) in values.into_iter().enumerate() {
        context.typed_array_store_element(view, offset + index, value)?;
    }
    Ok(JsValue::Undefined)
}

fn typed_array_slice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, name, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.slice")?;
    let start = normalize_relative_index(argument_integer(vm, context, arguments, 0, 0.0)?, length);
    let end = normalize_relative_index(
        argument_integer(vm, context, arguments, 1, length as f64)?,
        length,
    );
    let mut values = Vec::new();
    for index in start..end.max(start).min(length) {
        values.push(context.typed_array_load_element(view, index)?);
    }
    create_typed_array_from_values(context, object, name, kind, values)
}

fn typed_array_subarray(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, view, length, name, kind) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.subarray")?;
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
    let prototype = typed_array_prototype_for_name(context, object, &name)?;
    create_typed_array_object(
        context,
        prototype,
        name,
        kind,
        buffer_value,
        record.buffer,
        byte_offset,
        end - start,
    )
}

fn typed_array_sort_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    mut values: Vec<JsValue>,
    compare_fn: Option<JsValue>,
) -> Result<Vec<JsValue>, VmError> {
    for i in 1..values.len() {
        let mut j = i;
        while j > 0 {
            let swap = if let Some(func) = &compare_fn {
                let compared = vm.call_value_from_builtin(
                    func.clone(),
                    JsValue::Undefined,
                    vec![values[j - 1].clone(), values[j].clone()],
                    context,
                )?;
                vm.to_number(compared, context)? > 0.0
            } else {
                vm.to_string_coerce(values[j - 1].clone(), context)?
                    > vm.to_string_coerce(values[j].clone(), context)?
            };
            if !swap {
                break;
            }
            values.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(values)
}

fn typed_array_sort(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, view, length, _, _) =
        typed_array_parts(context, &this_value, "TypedArray.prototype.sort")?;
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
        context.typed_array_store_element(view, index, value)?;
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
    let index = if raw < 0 {
        let from_end = raw.unsigned_abs();
        if from_end > length {
            return Err(VmError::range("TypedArray index is out of range"));
        }
        length - from_end
    } else {
        raw as usize
    };
    if index >= length {
        return Err(VmError::range("TypedArray index is out of range"));
    }
    let mut values = typed_array_values_vec(context, view, length)?;
    values[index] = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
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
    define_hidden(
        context,
        prototype,
        INTL_KIND,
        JsValue::String(spec.kind.into()),
    )?;
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
        define_method(context, prototype, "format", 1, intl_number_format_format)?;
    } else if spec.kind == "Collator" {
        define_method(context, prototype, "compare", 2, intl_collator_compare)?;
    }
    let tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        tag,
        readonly_configurable_descriptor(JsValue::String(format!("Intl.{}", spec.name))),
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
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_intl_object(vm, context, arguments, "NumberFormat", "Intl.NumberFormat")
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
    _vm: &mut Vm,
    context: &mut NativeContext,
    _arguments: &[JsValue],
    new_target: JsValue,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Intl prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(context, object, INTL_KIND, JsValue::String(kind.into()))?;
    Ok(JsValue::Object(object))
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
    require_intl_kind(context, &this_value, "DateTimeFormat")?;
    object_from_pairs(
        context,
        [
            ("locale", JsValue::String("en-US".into())),
            ("calendar", JsValue::String("gregory".into())),
            ("numberingSystem", JsValue::String("latn".into())),
            ("timeZone", JsValue::String("UTC".into())),
        ],
    )
}

fn intl_number_format_resolved_options(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "NumberFormat")?;
    object_from_pairs(
        context,
        [
            ("locale", JsValue::String("en-US".into())),
            ("numberingSystem", JsValue::String("latn".into())),
            ("style", JsValue::String("decimal".into())),
            ("minimumIntegerDigits", JsValue::Number(1.0)),
            ("minimumFractionDigits", JsValue::Number(0.0)),
            ("maximumFractionDigits", JsValue::Number(3.0)),
            ("useGrouping", JsValue::String("auto".into())),
            ("notation", JsValue::String("standard".into())),
            ("signDisplay", JsValue::String("auto".into())),
        ],
    )
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
        .map(JsValue::String)
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
        JsValue::String(locale) => Ok(vec![locale]),
        other => {
            let object = match context.value_object(&other) {
                Some(object) => object,
                None => return Ok(vec![vm.to_string_coerce(other, context)?]),
            };
            let length = context
                .get_property(context.object_value(object), "length")?
                .to_number()
                .unwrap_or(0.0)
                .max(0.0) as usize;
            let mut locales = Vec::new();
            for index in 0..length {
                let value =
                    context.get_property(context.object_value(object), &index.to_string())?;
                if !matches!(value, JsValue::Undefined) {
                    locales.push(vm.to_string_coerce(value, context)?);
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
    require_intl_kind(context, &this_value, "NumberFormat")?;
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::String(vm.to_string_coerce(value, context)?))
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
    context.define_own_property(
        host,
        "global".into(),
        method_descriptor(context.global_this_value()),
    )?;
    context.define_own_property(host, "evalScript".into(), method_descriptor(eval_script))?;
    context.define_own_property(host, "gc".into(), method_descriptor(gc))?;
    context.define_own_property(host, "detachArrayBuffer".into(), method_descriptor(detach))?;
    context.define_own_property(host, "createRealm".into(), method_descriptor(create_realm))?;
    context.declare_global("$262", JsValue::Object(host));
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
