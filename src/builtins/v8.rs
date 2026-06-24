//! V8 builtin skeletons for large global families.
//!
//! This module intentionally installs honest first-stage shapes: constructors,
//! prototypes, descriptors, and deterministic Intl option objects. Operations
//! that need real typed storage are present only when they can fail explicitly.

use super::function;
use crate::{
    runtime::{
        JsObject, JsValue, NativeCall, NativeContext, ObjectId, PropertyDescriptor, PropertyKind,
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

fn own_number(context: &NativeContext, object: ObjectId, key: &str) -> Option<f64> {
    match own_data_value(context, object, key)? {
        JsValue::Number(value) => Some(value),
        _ => None,
    }
}

fn is_array_buffer_object(context: &NativeContext, object: ObjectId) -> bool {
    own_bool(context, object, ARRAY_BUFFER_MARKER).unwrap_or(false)
}

fn is_typed_array_object(context: &NativeContext, object: ObjectId) -> bool {
    own_bool(context, object, TYPED_ARRAY_MARKER).unwrap_or(false)
}

fn is_data_view_object(context: &NativeContext, object: ObjectId) -> bool {
    own_bool(context, object, DATA_VIEW_MARKER).unwrap_or(false)
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
    if !number.is_finite() || number < 0.0 || number.fract() != 0.0 {
        return Err(VmError::range("invalid buffer length"));
    }
    if number > MAX_SKELETON_BUFFER_BYTES as f64 {
        return Err(VmError::range("buffer length exceeds V8 skeleton limit"));
    }
    Ok(number as usize)
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
    define_method(
        context,
        prototype,
        "slice",
        2,
        unsupported_array_buffer_method,
    )?;
    define_method(
        context,
        prototype,
        "resize",
        1,
        unsupported_array_buffer_method,
    )?;
    define_method(
        context,
        prototype,
        "transfer",
        0,
        unsupported_array_buffer_method,
    )?;
    define_method(
        context,
        prototype,
        "transferToFixedLength",
        0,
        unsupported_array_buffer_method,
    )?;
    define_method(
        context,
        prototype,
        "sliceToImmutable",
        2,
        unsupported_array_buffer_method,
    )?;
    define_method(
        context,
        prototype,
        "transferToImmutable",
        0,
        unsupported_array_buffer_method,
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
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("ArrayBuffer prototype missing"))?;
    create_array_buffer_object(context, byte_length, prototype)
}

fn create_array_buffer_object(
    context: &mut NativeContext,
    byte_length: usize,
    prototype: ObjectId,
) -> Result<JsValue, VmError> {
    context.ensure_heap_capacity(byte_length)?;
    let object = new_ordinary_object(context, Some(prototype))?;
    // ponytail: C-track skeleton keeps ArrayBuffer internal slots as hidden
    // ordinary properties until B exposes real byte storage metadata.
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
    if !is_array_buffer_object(context, object) {
        return Err(VmError::type_error("receiver is not an ArrayBuffer"));
    }
    if own_bool(context, object, ARRAY_BUFFER_DETACHED).unwrap_or(false) {
        return Ok(JsValue::Number(0.0));
    }
    Ok(JsValue::Number(
        own_number(context, object, ARRAY_BUFFER_BYTE_LENGTH).unwrap_or(0.0),
    ))
}

fn array_buffer_max_byte_length_get(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_buffer_byte_length_get(vm, context, this_value, arguments)
}

fn array_buffer_resizable_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.resizable")?;
    if !is_array_buffer_object(context, object) {
        return Err(VmError::type_error("receiver is not an ArrayBuffer"));
    }
    Ok(JsValue::Boolean(false))
}

fn array_buffer_detached_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.detached")?;
    if !is_array_buffer_object(context, object) {
        return Err(VmError::type_error("receiver is not an ArrayBuffer"));
    }
    Ok(JsValue::Boolean(
        own_bool(context, object, ARRAY_BUFFER_DETACHED).unwrap_or(false),
    ))
}

fn array_buffer_immutable_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = object_from_this(context, &this_value, "ArrayBuffer.prototype.immutable")?;
    if !is_array_buffer_object(context, object) {
        return Err(VmError::type_error("receiver is not an ArrayBuffer"));
    }
    Ok(JsValue::Boolean(false))
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

fn unsupported_array_buffer_method(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "ArrayBuffer byte storage operations are not implemented in V8-C skeletons",
    ))
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
    for (name, length) in [
        ("getInt8", 1),
        ("getUint8", 1),
        ("getInt16", 1),
        ("getUint16", 1),
        ("getInt32", 1),
        ("getUint32", 1),
        ("getFloat32", 1),
        ("getFloat64", 1),
        ("setInt8", 2),
        ("setUint8", 2),
        ("setInt16", 2),
        ("setUint16", 2),
        ("setInt32", 2),
        ("setUint32", 2),
        ("setFloat32", 2),
        ("setFloat64", 2),
    ] {
        define_method(
            context,
            prototype,
            name,
            length,
            unsupported_data_view_method,
        )?;
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
    if !is_array_buffer_object(context, buffer_object) {
        return Err(VmError::type_error("DataView requires an ArrayBuffer"));
    }
    let buffer_length = own_number(context, buffer_object, ARRAY_BUFFER_BYTE_LENGTH).unwrap_or(0.0);
    let byte_offset = to_index(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    if byte_offset as f64 > buffer_length {
        return Err(VmError::range("DataView byteOffset is out of range"));
    }
    let byte_length = if let Some(value) = arguments.get(2) {
        to_index(vm, context, value.clone())?
    } else {
        (buffer_length as usize).saturating_sub(byte_offset)
    };
    if byte_offset.saturating_add(byte_length) as f64 > buffer_length {
        return Err(VmError::range("DataView byteLength is out of range"));
    }
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("DataView prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(context, object, DATA_VIEW_MARKER, JsValue::Boolean(true))?;
    define_hidden(context, object, DATA_VIEW_BUFFER, buffer)?;
    define_hidden(
        context,
        object,
        DATA_VIEW_BYTE_LENGTH,
        JsValue::Number(byte_length as f64),
    )?;
    define_hidden(
        context,
        object,
        DATA_VIEW_BYTE_OFFSET,
        JsValue::Number(byte_offset as f64),
    )?;
    Ok(JsValue::Object(object))
}

fn require_data_view(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<ObjectId, VmError> {
    let object = object_from_this(context, this_value, label)?;
    if !is_data_view_object(context, object) {
        return Err(VmError::type_error("receiver is not a DataView"));
    }
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
    Ok(JsValue::Number(
        own_number(context, object, DATA_VIEW_BYTE_LENGTH).unwrap_or(0.0),
    ))
}

fn data_view_byte_offset_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_data_view(context, &this_value, "DataView.prototype.byteOffset")?;
    Ok(JsValue::Number(
        own_number(context, object, DATA_VIEW_BYTE_OFFSET).unwrap_or(0.0),
    ))
}

fn unsupported_data_view_method(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "DataView byte storage operations are not implemented in V8-C skeletons",
    ))
}

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

    define_method(
        context,
        constructor_object,
        "from",
        1,
        unsupported_typed_array_static_method,
    )?;
    define_method(
        context,
        constructor_object,
        "of",
        0,
        unsupported_typed_array_static_method,
    )?;

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

    for (name, length) in [
        ("at", 1),
        ("copyWithin", 2),
        ("entries", 0),
        ("every", 1),
        ("fill", 1),
        ("filter", 1),
        ("find", 1),
        ("findIndex", 1),
        ("findLast", 1),
        ("findLastIndex", 1),
        ("forEach", 1),
        ("includes", 1),
        ("indexOf", 1),
        ("join", 1),
        ("keys", 0),
        ("lastIndexOf", 1),
        ("map", 1),
        ("reduce", 1),
        ("reduceRight", 1),
        ("reverse", 0),
        ("set", 1),
        ("slice", 2),
        ("some", 1),
        ("sort", 1),
        ("subarray", 2),
        ("toLocaleString", 0),
        ("toReversed", 0),
        ("toSorted", 1),
        ("toString", 0),
        ("values", 0),
        ("with", 2),
    ] {
        let function =
            context.register_builtin(name, length, unsupported_typed_array_method, None)?;
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
    let bytes_per_element = own_number(context, constructor_object, TYPED_ARRAY_BYTE_LENGTH)
        .unwrap_or(1.0)
        .max(1.0) as usize;
    let length = to_index(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let byte_length = length
        .checked_mul(bytes_per_element)
        .ok_or_else(|| VmError::range("typed array byte length overflow"))?;
    if byte_length > MAX_SKELETON_BUFFER_BYTES {
        return Err(VmError::range(
            "typed array length exceeds V8 skeleton limit",
        ));
    }
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("typed array prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
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
    let buffer = create_array_buffer_object(context, byte_length, array_buffer_proto)?;

    // ponytail: indexed element storage is deferred; metadata lets descriptor
    // and host-helper tests progress without pretending full typed semantics.
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
        JsValue::Number(0.0),
    )?;
    define_hidden(context, object, TYPED_ARRAY_BUFFER, buffer)?;
    Ok(JsValue::Object(object))
}

fn require_typed_array(
    context: &NativeContext,
    this_value: &JsValue,
    label: &str,
) -> Result<ObjectId, VmError> {
    let object = object_from_this(context, this_value, label)?;
    if !is_typed_array_object(context, object) {
        return Err(VmError::type_error("receiver is not a TypedArray"));
    }
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
    Ok(JsValue::Number(
        own_number(context, object, TYPED_ARRAY_BYTE_LENGTH).unwrap_or(0.0),
    ))
}

fn typed_array_byte_offset_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_typed_array(context, &this_value, "TypedArray.prototype.byteOffset")?;
    Ok(JsValue::Number(
        own_number(context, object, TYPED_ARRAY_BYTE_OFFSET).unwrap_or(0.0),
    ))
}

fn typed_array_length_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_typed_array(context, &this_value, "TypedArray.prototype.length")?;
    Ok(JsValue::Number(
        own_number(context, object, TYPED_ARRAY_LENGTH).unwrap_or(0.0),
    ))
}

fn unsupported_typed_array_method(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "TypedArray indexed element storage is not implemented in V8-C skeletons",
    ))
}

fn typed_array_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn unsupported_typed_array_static_method(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "TypedArray construction from element lists is not implemented in V8-C skeletons",
    ))
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
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    function::eval_call(vm, context, JsValue::Undefined, arguments)
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
    if !is_array_buffer_object(context, object) {
        return Err(VmError::type_error(
            "$262.detachArrayBuffer requires an ArrayBuffer",
        ));
    }
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
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "$262.createRealm is not implemented in the native V8-C host skeleton",
    ))
}
