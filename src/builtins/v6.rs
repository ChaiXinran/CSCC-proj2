//! V6 thin adapter layer.
//!
//! This module is the integration-owned bridge between the VM/runtime and the
//! pure C1/C2 algorithm modules (`string`, `number`, `boolean`, `math`,
//! `error`). The pure modules contain no VM wiring; the adapters here coerce
//! JavaScript values into the primitive inputs those helpers expect, delegate
//! to them, and wrap the results back into `JsValue`. All object-aware coercion
//! goes through the V6 `Vm` contract so JavaScript `valueOf`/`toString` throws
//! stay catchable by V5 handlers.

use super::{boolean, error, json, math, number, regexp, string};
use crate::runtime::{
    JsObject, JsValue, NativeCall, NativeContext, ObjectId, ObjectKind, PrimitiveValue,
    PropertyDescriptor, PropertyDescriptorUpdate, PropertyKind, to_property_key,
};
use crate::vm::{Vm, VmError};

/// Installs the standard-library globals backed by the C1/C2 modules.
pub(super) fn install(context: &mut NativeContext) -> Result<(), VmError> {
    install_error(context)?;
    install_number(context)?;
    install_boolean(context)?;
    install_string(context)?;
    install_math(context)?;
    install_json(context)?;
    install_global_functions(context)?;
    install_regexp(context)?;
    install_symbol(context)?;
    install_reflect(context)?;
    Ok(())
}

// ── Shared descriptor helpers ────────────────────────────────────────────────

/// `{ writable: true, enumerable: false, configurable: true }` — builtin method.
fn method_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, true)
}

/// `{ writable: false, enumerable: false, configurable: false }` — constant.
fn constant_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, false)
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

// ── Argument coercion helpers ────────────────────────────────────────────────

fn arg(arguments: &[JsValue], index: usize) -> JsValue {
    arguments.get(index).cloned().unwrap_or(JsValue::Undefined)
}

fn arg_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
) -> Result<f64, VmError> {
    vm.to_number(arg(arguments, index), context)
}

fn arg_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
) -> Result<String, VmError> {
    vm.to_string_coerce(arg(arguments, index), context)
}

/// ECMAScript `ToIntegerOrInfinity` reduced to the `f64` domain.
fn to_integer_or_infinity(value: f64) -> f64 {
    if value.is_nan() { 0.0 } else { value.trunc() }
}

/// `ToIntegerOrInfinity` clamped into the `i64` index domain used by C1.
fn to_index_i64(value: f64) -> i64 {
    let value = to_integer_or_infinity(value);
    if value >= i64::MAX as f64 {
        i64::MAX
    } else if value <= i64::MIN as f64 {
        i64::MIN
    } else {
        value as i64
    }
}

fn arg_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
) -> Result<i64, VmError> {
    Ok(to_index_i64(arg_number(vm, context, arguments, index)?))
}

/// ECMAScript `ToUint16`, used by `String.fromCharCode`.
fn to_uint16(value: f64) -> u16 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    value.trunc().rem_euclid(65_536.0) as u16
}

fn to_uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    value.trunc().rem_euclid(4_294_967_296.0) as u32
}

fn to_length(value: f64) -> usize {
    if value.is_nan() || value <= 0.0 {
        0
    } else if value >= 1_000_000.0 {
        1_000_000
    } else {
        value.trunc() as usize
    }
}

fn map_string_error(error: string::StringBuiltinError) -> VmError {
    match error {
        string::StringBuiltinError::AllocationLimit => VmError::runtime_limit(error.to_string()),
        _ => VmError::range(error.to_string()),
    }
}

fn map_number_format_error(error: number::NumberFormatError) -> VmError {
    VmError::range(match error {
        number::NumberFormatError::InvalidRadix => "radix must be between 2 and 36",
        number::NumberFormatError::FractionDigitsOutOfRange => {
            "fraction digits must be between 0 and 100"
        }
        number::NumberFormatError::PrecisionOutOfRange => "precision must be between 1 and 100",
    })
}

// ── `this` coercion for primitive-wrapper prototypes ─────────────────────────

fn this_string(vm: &mut Vm, context: &mut NativeContext, this: JsValue) -> Result<String, VmError> {
    match this {
        JsValue::String(value) => Ok(value),
        JsValue::Null | JsValue::Undefined => Err(VmError::type_error(
            "String.prototype method called on null or undefined",
        )),
        other => {
            if let Some(object) = context.value_object(&other)
                && let Some(PrimitiveValue::String(value)) = context.primitive_value(object)
            {
                return Ok(value.clone());
            }
            vm.to_string_coerce(other, context)
        }
    }
}

fn this_string_value(context: &NativeContext, this: JsValue) -> Result<String, VmError> {
    match this {
        JsValue::String(value) => Ok(value),
        other => {
            if let Some(object) = context.value_object(&other)
                && let Some(PrimitiveValue::String(value)) = context.primitive_value(object)
            {
                return Ok(value.clone());
            }
            Err(VmError::type_error(
                "String.prototype method called on a non-String",
            ))
        }
    }
}

fn this_number(context: &NativeContext, this: &JsValue) -> Result<f64, VmError> {
    if let JsValue::Number(value) = this {
        return Ok(*value);
    }
    if let Some(object) = context.value_object(this)
        && let Some(PrimitiveValue::Number(value)) = context.primitive_value(object)
    {
        return Ok(*value);
    }
    Err(VmError::type_error(
        "Number.prototype method called on a non-Number",
    ))
}

fn this_boolean(context: &NativeContext, this: &JsValue) -> Result<bool, VmError> {
    if let JsValue::Boolean(value) = this {
        return Ok(*value);
    }
    if let Some(object) = context.value_object(this)
        && let Some(PrimitiveValue::Boolean(value)) = context.primitive_value(object)
    {
        return Ok(*value);
    }
    Err(VmError::type_error(
        "Boolean.prototype method called on a non-Boolean",
    ))
}

// ── Error hierarchy ──────────────────────────────────────────────────────────

fn install_error(context: &mut NativeContext) -> Result<(), VmError> {
    let error_proto = context
        .error_prototype()
        .ok_or_else(|| VmError::runtime("error prototype missing"))?;

    for spec in error::ERROR_CONSTRUCTORS {
        // The root `Error.prototype` is pre-created in the foundation; subclass
        // prototypes are ordinary objects chained to `Error.prototype`.
        let prototype = if spec.name == "Error" {
            error_proto
        } else {
            let mut object = JsObject::ordinary();
            object.prototype = Some(error_proto);
            context
                .heap_mut()
                .allocate_object(object)
                .ok_or_else(|| VmError::runtime("heap exhausted"))?
        };

        let call = error_constructor_call(spec.name)
            .ok_or_else(|| VmError::runtime("missing Error constructor adapter"))?;
        let constructor =
            context.register_builtin(spec.name, spec.length, call, Some(error_construct))?;
        let JsValue::BuiltinFunction(id) = &constructor else {
            unreachable!()
        };
        let backing = context.builtin(*id).unwrap().object;

        context.define_own_property(
            backing,
            "prototype".into(),
            constant_descriptor(JsValue::Object(prototype)),
        )?;
        context.define_own_property(
            prototype,
            "constructor".into(),
            method_descriptor(constructor.clone()),
        )?;
        context.define_own_property(
            prototype,
            "name".into(),
            method_descriptor(JsValue::String(spec.name.into())),
        )?;
        context.define_own_property(
            prototype,
            "message".into(),
            method_descriptor(JsValue::String(String::new())),
        )?;
        context.declare_global(spec.name, constructor);
    }

    let error_constructor = context
        .get_global("Error")
        .ok_or_else(|| VmError::runtime("Error constructor missing"))?;
    let error_constructor_object = context
        .value_object(&error_constructor)
        .ok_or_else(|| VmError::runtime("Error constructor object missing"))?;
    define_method(
        context,
        error_constructor_object,
        "isError",
        1,
        error_is_error,
    )?;

    // Error.prototype.toString is shared by the whole hierarchy.
    define_method(context, error_proto, "toString", 0, error_to_string)?;
    let stack_getter = context.register_builtin("get stack", 0, error_stack_get, None)?;
    let stack_setter = context.register_builtin("set stack", 1, error_stack_set, None)?;
    context.define_own_property(
        error_proto,
        "stack".into(),
        PropertyDescriptor::accessor(Some(stack_getter), Some(stack_setter), false, true),
    )?;
    Ok(())
}

fn error_constructor_call(name: &str) -> Option<NativeCall> {
    Some(match name {
        "Error" => error_call,
        "EvalError" => eval_error_call,
        "RangeError" => range_error_call,
        "ReferenceError" => reference_error_call,
        "SyntaxError" => syntax_error_call,
        "TypeError" => type_error_call,
        "URIError" => uri_error_call,
        _ => return None,
    })
}

fn install_json(context: &mut NativeContext) -> Result<(), VmError> {
    let object = context
        .heap_mut()
        .allocate_object(JsObject::ordinary())
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    define_method(context, object, "parse", 2, json_parse)?;
    define_method(context, object, "stringify", 3, json_stringify)?;
    define_method(context, object, "rawJSON", 1, json_raw_json)?;
    define_method(context, object, "isRawJSON", 1, json_is_raw_json)?;
    context.declare_global("JSON", JsValue::Object(object));
    Ok(())
}

fn json_parse(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arg_string(vm, context, arguments, 0)?;
    json::parse_json_with_reviver(&source, arg(arguments, 1), vm, context)
}

fn json_stringify(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(
        match json::stringify_json_with_options(
            arg(arguments, 0),
            arg(arguments, 1),
            arg(arguments, 2),
            vm,
            context,
        )? {
            Some(value) => JsValue::String(value),
            None => JsValue::Undefined,
        },
    )
}

fn json_raw_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let raw_json = arg_string(vm, context, arguments, 0)?;
    if raw_json.is_empty()
        || raw_json
            .as_bytes()
            .first()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
        || raw_json
            .as_bytes()
            .last()
            .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        return Err(VmError::type_error("JSON.rawJSON: invalid JSON text"));
    }
    let parsed = json::parse_json(&raw_json, context)?;
    if matches!(parsed, JsValue::Object(_)) {
        return Err(VmError::type_error(
            "JSON.rawJSON: top-level object or array is not allowed",
        ));
    }

    let mut object = JsObject::ordinary();
    object.prototype = None;
    object.define_property(
        "rawJSON",
        PropertyDescriptor::data_with(JsValue::String(raw_json.clone()), true, true, true),
    );
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    context.mark_raw_json_object(id, raw_json);
    Ok(JsValue::Object(id))
}

fn json_is_raw_json(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = arguments
        .first()
        .and_then(|value| context.value_object(value))
        .is_some_and(|object| context.raw_json_value(object).is_some());
    Ok(JsValue::Boolean(result))
}

fn call_error_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    constructor_name: &str,
) -> Result<JsValue, VmError> {
    let constructor = context
        .get_global(constructor_name)
        .ok_or_else(|| VmError::runtime("error constructor missing"))?;
    let prototype = context
        .constructor_prototype(&constructor)?
        .or_else(|| context.error_prototype())
        .ok_or_else(|| VmError::runtime("error prototype missing"))?;
    create_error_object(vm, context, arguments, prototype)
}

macro_rules! error_call_adapter {
    ($function:ident, $name:literal) => {
        fn $function(
            vm: &mut Vm,
            context: &mut NativeContext,
            _this: JsValue,
            arguments: &[JsValue],
        ) -> Result<JsValue, VmError> {
            call_error_constructor(vm, context, arguments, $name)
        }
    };
}

error_call_adapter!(error_call, "Error");
error_call_adapter!(eval_error_call, "EvalError");
error_call_adapter!(range_error_call, "RangeError");
error_call_adapter!(reference_error_call, "ReferenceError");
error_call_adapter!(syntax_error_call, "SyntaxError");
error_call_adapter!(type_error_call, "TypeError");
error_call_adapter!(uri_error_call, "URIError");

fn create_error_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    prototype: ObjectId,
) -> Result<JsValue, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = Some(prototype);
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    context.mark_error_object(id);
    if let Some(value) = arguments
        .first()
        .filter(|value| !matches!(value, JsValue::Undefined))
    {
        let message = vm.to_string_coerce(value.clone(), context)?;
        context.define_own_property(
            id,
            "message".into(),
            method_descriptor(JsValue::String(message)),
        )?;
    }
    if let Some(options) = arguments.get(1)
        && let Some(options_object) = context.value_object(options)
        && context.has_property(options_object, "cause")?
    {
        let cause = vm.get_property_value(options.clone(), "cause", context)?;
        context.define_own_property(id, "cause".into(), method_descriptor(cause))?;
    }
    Ok(JsValue::Object(id))
}

fn error_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.error_prototype())
        .ok_or_else(|| VmError::runtime("error prototype missing"))?;
    create_error_object(vm, context, arguments, prototype)
}

fn error_is_error(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = arguments
        .first()
        .and_then(|value| context.value_object(value))
        .is_some_and(|object| context.is_error_object(object));
    Ok(JsValue::Boolean(result))
}

fn error_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let name_value = context.get_property(this.clone(), "name")?;
    let name = if matches!(name_value, JsValue::Undefined) {
        "Error".to_string()
    } else {
        vm.to_string_coerce(name_value, context)?
    };
    let message_value = context.get_property(this, "message")?;
    let message = if matches!(message_value, JsValue::Undefined) {
        String::new()
    } else {
        vm.to_string_coerce(message_value, context)?
    };

    let record = error::create_error_record(error_name_static(&name), Some(message.clone()));
    // Prefer the shared C2 formatter when the name matches a known constructor;
    // otherwise fall back to the runtime name so user subclasses round-trip.
    let formatted = if error::constructor_spec(&name).is_some() {
        error::error_to_string(&record)
    } else {
        format_error(&name, &message)
    };
    Ok(JsValue::String(formatted))
}

fn error_stack_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context
        .value_object(&this)
        .ok_or_else(|| VmError::type_error("Error.prototype.stack getter requires an object"))?;
    if !context.is_error_object(object) {
        return Ok(JsValue::Undefined);
    }

    let name = context
        .get_property(this.clone(), "name")?
        .to_js_string()
        .unwrap_or_else(|| "Error".into());
    let message = context
        .get_property(this, "message")?
        .to_js_string()
        .unwrap_or_default();
    Ok(JsValue::String(format_error(&name, &message)))
}

fn error_stack_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context
        .value_object(&this)
        .ok_or_else(|| VmError::type_error("Error.prototype.stack setter requires an object"))?;
    if context.error_prototype() == Some(object) {
        return Err(VmError::type_error(
            "Error.prototype.stack cannot be set on Error.prototype",
        ));
    }
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !matches!(value, JsValue::String(_)) {
        return Err(VmError::type_error(
            "Error.prototype.stack setter requires a string",
        ));
    }

    if let Some(mut descriptor) = context.get_own_property_descriptor(object, "stack") {
        match &mut descriptor.kind {
            crate::runtime::PropertyKind::Data {
                value: current,
                writable,
            } => {
                if !*writable {
                    return Err(VmError::type_error("stack property is not writable"));
                }
                *current = value;
                context.define_own_property(object, "stack".into(), descriptor)?;
            }
            crate::runtime::PropertyKind::Accessor {
                set: Some(setter), ..
            } => {
                vm.call_value_from_builtin(setter.clone(), this, vec![value], context)?;
            }
            crate::runtime::PropertyKind::Accessor { set: None, .. } => {
                return Err(VmError::type_error("stack property has no setter"));
            }
        }
    } else {
        context.define_own_property(object, "stack".into(), PropertyDescriptor::data(value))?;
    }
    Ok(JsValue::Undefined)
}

/// Maps a runtime error name onto the `'static` name expected by C2, falling
/// back to a generic label for user-defined names.
fn error_name_static(name: &str) -> &'static str {
    error::constructor_spec(name).map_or("Error", |spec| spec.name)
}

fn format_error(name: &str, message: &str) -> String {
    match (name.is_empty(), message.is_empty()) {
        (true, true) => String::new(),
        (false, true) => name.to_string(),
        (true, false) => message.to_string(),
        (false, false) => format!("{name}: {message}"),
    }
}

// ── Number ───────────────────────────────────────────────────────────────────

fn install_number(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = context
        .number_prototype()
        .ok_or_else(|| VmError::runtime("number prototype missing"))?;
    let constructor = context.register_builtin("Number", 1, number_call, Some(number_construct))?;
    let JsValue::BuiltinFunction(id) = &constructor else {
        unreachable!()
    };
    let backing = context.builtin(*id).unwrap().object;

    context.define_own_property(
        backing,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    for spec in number::NUMBER_PROTOTYPE_METHODS {
        let call = number_prototype_call(spec.name)
            .ok_or_else(|| VmError::runtime("missing Number.prototype adapter"))?;
        define_method(context, prototype, spec.name, spec.length, call)?;
    }
    for spec in number::NUMBER_STATIC_METHODS {
        let call = number_static_call(spec.name)
            .ok_or_else(|| VmError::runtime("missing Number static adapter"))?;
        define_method(context, backing, spec.name, spec.length, call)?;
    }
    for constant in number::NUMBER_CONSTANTS {
        context.define_own_property(
            backing,
            constant.name.into(),
            constant_descriptor(JsValue::Number(constant.value)),
        )?;
    }

    context.declare_global("Number", constructor);
    Ok(())
}

fn number_prototype_call(name: &str) -> Option<NativeCall> {
    Some(match name {
        "valueOf" => number_value_of,
        "toString" => number_to_string,
        "toFixed" => number_to_fixed,
        "toExponential" => number_to_exponential,
        "toPrecision" => number_to_precision,
        "toLocaleString" => number_to_locale_string,
        _ => return None,
    })
}

fn number_static_call(name: &str) -> Option<NativeCall> {
    Some(match name {
        "isNaN" => number_is_nan,
        "isFinite" => number_is_finite,
        "isInteger" => number_is_integer,
        "isSafeInteger" => number_is_safe_integer,
        "parseInt" => global_parse_int,
        "parseFloat" => global_parse_float,
        _ => return None,
    })
}

fn number_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if arguments.is_empty() {
        return Ok(JsValue::Number(number::number_call(None)));
    }
    let value = vm.to_number(arg(arguments, 0), context)?;
    Ok(JsValue::Number(number::number_call(Some(value))))
}

fn number_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let value = if arguments.is_empty() {
        0.0
    } else {
        vm.to_number(arg(arguments, 0), context)?
    };
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.number_prototype())
        .ok_or_else(|| VmError::runtime("Number prototype not installed"))?;
    context.create_primitive_wrapper(
        PrimitiveValue::Number(number::number_value_of(value)),
        prototype,
    )
}

fn number_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(number::number_value_of(this_number(
        context, &this,
    )?)))
}

fn number_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_number(context, &this)?;
    let radix = match arguments.first() {
        None | Some(JsValue::Undefined) => None,
        Some(_) => {
            let radix = to_integer_or_infinity(arg_number(vm, context, arguments, 0)?);
            if !(2.0..=36.0).contains(&radix) {
                return Err(VmError::range("radix must be between 2 and 36"));
            }
            Some(radix as u32)
        }
    };
    number::number_to_string(value, radix)
        .map(JsValue::String)
        .map_err(map_number_format_error)
}

fn number_to_fixed(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_number(context, &this)?;
    let digits = to_integer_or_infinity(arg_number(vm, context, arguments, 0)?);
    if !(0.0..=100.0).contains(&digits) {
        return Err(VmError::range("fraction digits must be between 0 and 100"));
    }
    number::to_fixed(value, digits as u32)
        .map(JsValue::String)
        .map_err(map_number_format_error)
}

fn number_to_exponential(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_number(context, &this)?;
    let fraction_digits = match arguments.first() {
        None | Some(JsValue::Undefined) => None,
        Some(_) => {
            let digits = to_integer_or_infinity(arg_number(vm, context, arguments, 0)?);
            Some(digits as u32)
        }
    };
    number::to_exponential(value, fraction_digits)
        .map(JsValue::String)
        .map_err(map_number_format_error)
}

fn number_to_precision(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_number(context, &this)?;
    let precision = match arguments.first() {
        None | Some(JsValue::Undefined) => None,
        Some(_) => {
            let precision = to_integer_or_infinity(arg_number(vm, context, arguments, 0)?);
            Some(precision as u32)
        }
    };
    number::to_precision(value, precision)
        .map(JsValue::String)
        .map_err(map_number_format_error)
}
fn number_to_locale_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_number(context, &this)?;
    number::number_to_string(value, None)
        .map(JsValue::String)
        .map_err(map_number_format_error)
}

fn number_is_nan(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    // Number.isNaN does not coerce its argument.
    let result =
        matches!(arguments.first(), Some(JsValue::Number(value)) if number::is_nan(*value));
    Ok(JsValue::Boolean(result))
}

fn number_is_finite(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result =
        matches!(arguments.first(), Some(JsValue::Number(value)) if number::is_finite(*value));
    Ok(JsValue::Boolean(result))
}

fn number_is_integer(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result =
        matches!(arguments.first(), Some(JsValue::Number(value)) if number::is_integer(*value));
    Ok(JsValue::Boolean(result))
}

fn number_is_safe_integer(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = matches!(
        arguments.first(),
        Some(JsValue::Number(value)) if number::is_safe_integer(*value)
    );
    Ok(JsValue::Boolean(result))
}

// ── Boolean ──────────────────────────────────────────────────────────────────

fn install_boolean(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = context
        .boolean_prototype()
        .ok_or_else(|| VmError::runtime("boolean prototype missing"))?;
    let constructor =
        context.register_builtin("Boolean", 1, boolean_call, Some(boolean_construct))?;
    let JsValue::BuiltinFunction(id) = &constructor else {
        unreachable!()
    };
    let backing = context.builtin(*id).unwrap().object;

    context.define_own_property(
        backing,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    for spec in boolean::BOOLEAN_PROTOTYPE_METHODS {
        let call = match spec.name {
            "valueOf" => boolean_value_of,
            "toString" => boolean_to_string,
            _ => return Err(VmError::runtime("missing Boolean.prototype adapter")),
        };
        define_method(context, prototype, spec.name, spec.length, call)?;
    }

    context.declare_global("Boolean", constructor);
    Ok(())
}

fn boolean_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().map(JsValue::to_boolean).unwrap_or(false);
    Ok(JsValue::Boolean(boolean::boolean_call(value)))
}

fn boolean_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let value = arguments.first().map(JsValue::to_boolean).unwrap_or(false);
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.boolean_prototype())
        .ok_or_else(|| VmError::runtime("Boolean prototype not installed"))?;
    context.create_primitive_wrapper(PrimitiveValue::Boolean(value), prototype)
}

fn boolean_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Boolean(boolean::boolean_value_of(this_boolean(
        context, &this,
    )?)))
}

fn boolean_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(
        boolean::boolean_to_string(this_boolean(context, &this)?).to_string(),
    ))
}

// ── String ───────────────────────────────────────────────────────────────────

fn install_string(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = context
        .string_prototype()
        .ok_or_else(|| VmError::runtime("string prototype missing"))?;
    let constructor = context.register_builtin("String", 1, string_call, Some(string_construct))?;
    let JsValue::BuiltinFunction(id) = &constructor else {
        unreachable!()
    };
    let backing = context.builtin(*id).unwrap().object;

    context.define_own_property(
        backing,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    for spec in string::PROTOTYPE_METHODS {
        let call = string_prototype_call(spec.name)
            .ok_or_else(|| VmError::runtime("missing String.prototype adapter"))?;
        define_method(context, prototype, spec.name, spec.length, call)?;
    }
    for spec in string::STATIC_METHODS {
        let call = string_static_call(spec.name)
            .ok_or_else(|| VmError::runtime("missing String static adapter"))?;
        define_method(context, backing, spec.name, spec.length, call)?;
    }

    context.declare_global("String", constructor);
    Ok(())
}

fn string_prototype_call(name: &str) -> Option<NativeCall> {
    Some(match name {
        "toString" | "valueOf" => string_value_of,
        "charAt" => string_char_at,
        "charCodeAt" => string_char_code_at,
        "at" => string_at,
        "codePointAt" => string_code_point_at,
        "concat" => string_concat,
        "includes" => string_includes,
        "localeCompare" => string_locale_compare,
        "indexOf" => string_index_of,
        "lastIndexOf" => string_last_index_of,
        "slice" => string_slice,
        "substring" => string_substring,
        "substr" => string_substr,
        "startsWith" => string_starts_with,
        "endsWith" => string_ends_with,
        "repeat" => string_repeat,
        "split" => string_split,
        "search" => string_search,
        "replace" => string_replace,
        "replaceAll" => string_replace_all,
        "match" => string_match,
        "matchAll" => string_match_all,
        "padStart" => string_pad_start,
        "padEnd" => string_pad_end,
        "trim" => string_trim,
        "trimStart" => string_trim_start,
        "trimEnd" => string_trim_end,
        "toLowerCase" => string_to_lower_case,
        "toUpperCase" => string_to_upper_case,
        "toLocaleLowerCase" => string_to_locale_lower_case,
        "toLocaleUpperCase" => string_to_locale_upper_case,
        "normalize" => string_normalize,
        "isWellFormed" => string_is_well_formed,
        "toWellFormed" => string_to_well_formed,
        _ => return None,
    })
}
fn string_static_call(name: &str) -> Option<NativeCall> {
    Some(match name {
        "fromCharCode" => string_from_char_code,
        "fromCodePoint" => string_from_code_point,
        "raw" => string_raw,
        _ => return None,
    })
}
fn string_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if arguments.is_empty() {
        return Ok(JsValue::String(String::new()));
    }
    Ok(JsValue::String(
        vm.to_string_coerce(arg(arguments, 0), context)?,
    ))
}

fn string_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let value = match arguments.first().cloned() {
        None => String::new(),
        Some(JsValue::String(value)) => value,
        Some(other) => vm.to_string_coerce(other, context)?,
    };
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.string_prototype())
        .ok_or_else(|| VmError::runtime("String prototype not installed"))?;
    context.create_primitive_wrapper(PrimitiveValue::String(value), prototype)
}

fn string_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(this_string_value(context, this)?))
}
fn string_char_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let index = arg_index(vm, context, arguments, 0)?;
    Ok(JsValue::String(string::char_at(&value, index)))
}

fn string_char_code_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let index = arg_index(vm, context, arguments, 0)?;
    Ok(match string::char_code_at(&value, index) {
        Some(unit) => JsValue::Number(f64::from(unit)),
        None => JsValue::Number(f64::NAN),
    })
}

fn string_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let index = arg_index(vm, context, arguments, 0)?;
    Ok(match string::at(&value, index) {
        Some(unit) => JsValue::String(unit),
        None => JsValue::Undefined,
    })
}
fn string_code_point_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let index = arg_index(vm, context, arguments, 0)?;
    Ok(match string::code_point_at(&value, index) {
        Some(point) => JsValue::Number(f64::from(point)),
        None => JsValue::Undefined,
    })
}

fn string_concat(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let mut parts = Vec::with_capacity(arguments.len());
    for index in 0..arguments.len() {
        parts.push(arg_string(vm, context, arguments, index)?);
    }
    let borrowed: Vec<&str> = parts.iter().map(String::as_str).collect();
    Ok(JsValue::String(string::concat(&value, &borrowed)))
}

/// Returns `true` if `value` is a `JsValue::Object` whose internal kind is
/// `ObjectKind::RegExp`. Equivalent to the ECMAScript `IsRegExp` abstract
/// operation (without Symbol.match support).
fn is_regexp_value(context: &NativeContext, value: &JsValue) -> bool {
    let JsValue::Object(id) = value else {
        return false;
    };
    matches!(
        context.heap().object(*id).map(|o| &o.kind),
        Some(ObjectKind::RegExp { .. })
    )
}

/// Try dispatching to `value[@@symbol](string_this, ...rest_args)`.
///
/// Returns `Some(result)` when the symbol method exists and was called.
/// Returns `None` when the value has no such symbol (caller should use its own logic).
fn try_symbol_dispatch(
    vm: &mut Vm,
    context: &mut NativeContext,
    sym: crate::runtime::SymbolId,
    value: JsValue,
    string_this: String,
    rest_args: &[JsValue],
) -> Option<Result<JsValue, VmError>> {
    let JsValue::Object(oid) = &value else {
        return None;
    };
    let method = context.get_symbol_property_value(*oid, sym)?;
    if !is_callable_value(&method) {
        return None;
    }
    let mut call_args = Vec::with_capacity(1 + rest_args.len());
    call_args.push(JsValue::String(string_this));
    call_args.extend_from_slice(rest_args);
    Some(vm.call_value_from_builtin(method, value, call_args, context))
}

/// Extract (pattern, flags) from a value known to be a RegExp object.
fn regexp_data(context: &NativeContext, value: &JsValue) -> Option<(String, String)> {
    let JsValue::Object(id) = value else {
        return None;
    };
    if let Some(ObjectKind::RegExp { pattern, flags }) = context.heap().object(*id).map(|o| &o.kind)
    {
        Some((pattern.clone(), flags.clone()))
    } else {
        None
    }
}

fn string_includes(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let first_arg = arg(arguments, 0);
    if is_regexp_value(context, &first_arg) {
        return Err(VmError::type_error(
            "String.prototype.includes does not accept a regular expression",
        ));
    }
    let value = this_string(vm, context, this)?;
    let search = arg_string(vm, context, arguments, 0)?;
    let position = arg_index(vm, context, arguments, 1)?;
    Ok(JsValue::Boolean(string::includes(
        &value, &search, position,
    )))
}

fn string_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let search = arg_string(vm, context, arguments, 0)?;
    let position = arg_index(vm, context, arguments, 1)?;
    Ok(JsValue::Number(
        string::index_of(&value, &search, position).map_or(-1.0, |index| index as f64),
    ))
}

fn string_last_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let search = arg_string(vm, context, arguments, 0)?;
    let position = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => Some(arg_index(vm, context, arguments, 1)?),
    };
    Ok(JsValue::Number(
        string::last_index_of(&value, &search, position).map_or(-1.0, |index| index as f64),
    ))
}

fn string_slice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let start = arg_index(vm, context, arguments, 0)?;
    let end = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => Some(arg_index(vm, context, arguments, 1)?),
    };
    Ok(JsValue::String(string::slice(&value, start, end)))
}

fn string_substring(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let start = arg_index(vm, context, arguments, 0)?;
    let end = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => Some(arg_index(vm, context, arguments, 1)?),
    };
    Ok(JsValue::String(string::substring(&value, start, end)))
}

fn string_substr(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let start = arg_index(vm, context, arguments, 0)?;
    let length = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => Some(arg_index(vm, context, arguments, 1)?),
    };
    Ok(JsValue::String(string::substr(&value, start, length)))
}

fn string_starts_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let first_arg = arg(arguments, 0);
    if is_regexp_value(context, &first_arg) {
        return Err(VmError::type_error(
            "String.prototype.startsWith does not accept a regular expression",
        ));
    }
    let value = this_string(vm, context, this)?;
    let search = arg_string(vm, context, arguments, 0)?;
    let position = arg_index(vm, context, arguments, 1)?;
    Ok(JsValue::Boolean(string::starts_with(
        &value, &search, position,
    )))
}

fn string_ends_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let first_arg = arg(arguments, 0);
    if is_regexp_value(context, &first_arg) {
        return Err(VmError::type_error(
            "String.prototype.endsWith does not accept a regular expression",
        ));
    }
    let value = this_string(vm, context, this)?;
    let search = arg_string(vm, context, arguments, 0)?;
    let end = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => Some(arg_index(vm, context, arguments, 1)?),
    };
    Ok(JsValue::Boolean(string::ends_with(&value, &search, end)))
}

fn string_repeat(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let count = arg_index(vm, context, arguments, 0)?;
    if let Ok(count) = usize::try_from(count) {
        let units = string::utf16_length(&value)
            .checked_mul(count)
            .ok_or_else(|| VmError::runtime_limit("string allocation limit exceeded"))?;
        context.ensure_heap_capacity(units.saturating_mul(2))?;
    }
    string::repeat(&value, count)
        .map(JsValue::String)
        .map_err(map_string_error)
}

fn string_split(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let first_arg = arg(arguments, 0);
    // @@split dispatch: separator[Symbol.split](string, limit)
    let sym = context.well_known_symbols().split;
    if let Some(r) = try_symbol_dispatch(
        vm,
        context,
        sym,
        first_arg.clone(),
        value.clone(),
        arguments.get(1..).unwrap_or_default(),
    ) {
        return r;
    }
    let limit = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => Some(to_uint32(arg_number(vm, context, arguments, 1)?) as usize),
    };
    if let Some((pattern, flags)) = regexp_data(context, &first_arg) {
        let re = regexp::compile_regex(&pattern, &flags)
            .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
        // regexp::split includes capture groups as per ES spec.
        let parts = regexp::split(&re, &value, limit)
            .into_iter()
            .map(|v| v.map_or(JsValue::Undefined, JsValue::String))
            .collect();
        return context.create_array(parts);
    }
    let separator = match first_arg {
        JsValue::Undefined => None,
        _ => Some(arg_string(vm, context, arguments, 0)?),
    };
    let limit32 = limit.map_or(u32::MAX, |l| l.min(u32::MAX as usize) as u32);
    let parts = string::split(&value, separator.as_deref(), limit32)
        .into_iter()
        .map(JsValue::String)
        .collect();
    context.create_array(parts)
}

fn string_search(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let first_arg = arg(arguments, 0);
    // @@search dispatch
    let sym = context.well_known_symbols().search;
    if let Some(r) = try_symbol_dispatch(vm, context, sym, first_arg.clone(), value.clone(), &[]) {
        return r;
    }
    if let Some((pattern, flags)) = regexp_data(context, &first_arg) {
        let re = regexp::compile_regex(&pattern, &flags)
            .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
        return Ok(JsValue::Number(
            regexp::search(&re, &value).map_or(-1.0, |i| i as f64),
        ));
    }
    let search = arg_string(vm, context, arguments, 0)?;
    Ok(JsValue::Number(string::search(&value, &search) as f64))
}

fn string_replace(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let first_arg = arg(arguments, 0);
    // @@replace dispatch: searchValue[Symbol.replace](string, replaceValue)
    let sym = context.well_known_symbols().replace;
    if let Some(r) = try_symbol_dispatch(
        vm,
        context,
        sym,
        first_arg.clone(),
        value.clone(),
        arguments.get(1..).unwrap_or_default(),
    ) {
        return r;
    }
    let replace_arg = arg(arguments, 1);
    let replace_is_fn = is_callable_value(&replace_arg);

    if let Some((pattern, flags)) = regexp_data(context, &first_arg) {
        let re = regexp::compile_regex(&pattern, &flags)
            .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
        let global = regexp::is_global(&flags);
        if replace_is_fn {
            return apply_replace_fn(vm, context, &value, &re, global, replace_arg);
        }
        let replacement = vm.to_string_coerce(replace_arg, context)?;
        let result = if global {
            regexp::replace_all(&re, &value, &replacement)
        } else {
            regexp::replace_first(&re, &value, &replacement)
        };
        return Ok(JsValue::String(result));
    }

    // String search value.
    let search = vm.to_string_coerce(first_arg, context)?;
    if replace_is_fn {
        // Find first occurrence, call function.
        if let Some(pos) = value.find(search.as_str()) {
            let index = value[..pos].encode_utf16().count();
            let args = vec![
                JsValue::String(search.clone()),
                JsValue::Number(index as f64),
                JsValue::String(value.clone()),
            ];
            let repl_val =
                vm.call_value_from_builtin(replace_arg, JsValue::Undefined, args, context)?;
            let repl = vm.to_string_coerce(repl_val, context)?;
            return Ok(JsValue::String(format!(
                "{}{}{}",
                &value[..pos],
                repl,
                &value[pos + search.len()..]
            )));
        }
        return Ok(JsValue::String(value));
    }
    let replacement = vm.to_string_coerce(replace_arg, context)?;
    Ok(JsValue::String(string::replace(
        &value,
        &search,
        &replacement,
    )))
}

/// Calls `replace_fn` for every match of `re` in `text` and builds the result.
fn apply_replace_fn(
    vm: &mut Vm,
    context: &mut NativeContext,
    text: &str,
    re: &regex::Regex,
    global: bool,
    replace_fn: JsValue,
) -> Result<JsValue, VmError> {
    let details = regexp::matches_with_detail(re, text, global);
    if details.is_empty() {
        return Ok(JsValue::String(text.to_owned()));
    }

    let mut result = String::new();
    let mut last_end_utf8 = 0;

    // We need byte positions too: re-iterate to track them.
    let mut byte_matches: Vec<(usize, usize)> = Vec::new();
    {
        let mut iter = re.find_iter(text);
        for _ in &details {
            if let Some(m) = iter.next() {
                byte_matches.push((m.start(), m.end()));
            }
        }
        if !global {
            // Only first match.
            byte_matches.truncate(1);
        }
    }

    for (detail, (byte_start, byte_end)) in details.iter().zip(byte_matches.iter()) {
        result.push_str(&text[last_end_utf8..*byte_start]);

        // Build args: (match, p1, p2, ..., offset, inputString)
        let mut args = vec![JsValue::String(detail.full_match.clone())];
        // Capture groups (skip index 0 = full match)
        for cap in detail.captures.iter().skip(1) {
            args.push(
                cap.as_deref()
                    .map_or(JsValue::Undefined, |s| JsValue::String(s.to_owned())),
            );
        }
        args.push(JsValue::Number(detail.index as f64));
        args.push(JsValue::String(text.to_owned()));

        let repl_val =
            vm.call_value_from_builtin(replace_fn.clone(), JsValue::Undefined, args, context)?;
        let repl = vm.to_string_coerce(repl_val, context)?;
        result.push_str(&repl);
        last_end_utf8 = *byte_end;
    }
    result.push_str(&text[last_end_utf8..]);
    Ok(JsValue::String(result))
}

fn string_replace_all(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let first_arg = arg(arguments, 0);
    // @@replace dispatch (replaceAll also delegates to [Symbol.replace]).
    // The spec requires the regexp to have the 'g' flag; the Symbol method
    // implementation (regexp_symbol_replace) inherits the regexp's own flags,
    // so the check below is enough for the non-Symbol path.
    let sym = context.well_known_symbols().replace;
    if let Some(r) = try_symbol_dispatch(
        vm,
        context,
        sym,
        first_arg.clone(),
        value.clone(),
        arguments.get(1..).unwrap_or_default(),
    ) {
        return r;
    }
    let replace_arg = arg(arguments, 1);
    let replace_is_fn = is_callable_value(&replace_arg);

    if let Some((pattern, flags)) = regexp_data(context, &first_arg) {
        if !regexp::is_global(&flags) {
            return Err(VmError::type_error(
                "String.prototype.replaceAll must be called with a global RegExp",
            ));
        }
        let re = regexp::compile_regex(&pattern, &flags)
            .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
        if replace_is_fn {
            return apply_replace_fn(vm, context, &value, &re, true, replace_arg);
        }
        let replacement = vm.to_string_coerce(replace_arg, context)?;
        return Ok(JsValue::String(regexp::replace_all(
            &re,
            &value,
            &replacement,
        )));
    }

    let search = vm.to_string_coerce(first_arg, context)?;
    if replace_is_fn {
        // Replace all literal occurrences with function callback.
        let mut result = String::new();
        let mut rest = value.as_str();
        let mut char_offset = 0usize;
        loop {
            match rest.find(search.as_str()) {
                None => {
                    result.push_str(rest);
                    break;
                }
                Some(pos) => {
                    result.push_str(&rest[..pos]);
                    let index = char_offset + rest[..pos].encode_utf16().count();
                    let args = vec![
                        JsValue::String(search.clone()),
                        JsValue::Number(index as f64),
                        JsValue::String(value.clone()),
                    ];
                    let repl_val = vm.call_value_from_builtin(
                        replace_arg.clone(),
                        JsValue::Undefined,
                        args,
                        context,
                    )?;
                    let repl = vm.to_string_coerce(repl_val, context)?;
                    result.push_str(&repl);
                    // When `search` is empty, `str::find("")` always returns
                    // `Some(0)`.  Advancing by 1 byte is wrong for multi-byte
                    // UTF-8 characters and panics on empty `rest`.  Advance by
                    // the byte length of the next character instead.
                    let char_advance = if search.is_empty() {
                        rest[pos..].chars().next().map_or(0, |c| c.len_utf8())
                    } else {
                        search.len()
                    };
                    let skip = pos + char_advance;
                    // For an empty search the skipped character belongs in the
                    // output between the two surrounding replacements.
                    if search.is_empty() && char_advance > 0 {
                        result.push_str(&rest[pos..skip]);
                    }
                    char_offset += rest[..skip].encode_utf16().count();
                    rest = &rest[skip..];
                    // Empty search + empty rest means we just processed the
                    // final end-of-string position; exit to avoid infinite loop.
                    if search.is_empty() && char_advance == 0 {
                        break;
                    }
                }
            }
        }
        return Ok(JsValue::String(result));
    }
    let replacement = vm.to_string_coerce(replace_arg, context)?;
    Ok(JsValue::String(string::replace_all(
        &value,
        &search,
        &replacement,
    )))
}

fn string_match(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let first_arg = arg(arguments, 0);
    // @@match dispatch
    let sym = context.well_known_symbols().match_;
    if let Some(r) = try_symbol_dispatch(vm, context, sym, first_arg.clone(), value.clone(), &[]) {
        return r;
    }
    if let Some((pattern, flags)) = regexp_data(context, &first_arg) {
        let re = regexp::compile_regex(&pattern, &flags)
            .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
        if regexp::is_global(&flags) {
            // Global match: return array of all matches or null.
            let matches = regexp::exec_global(&re, &value);
            if matches.is_empty() {
                return Ok(JsValue::Null);
            }
            let elements = matches.into_iter().map(JsValue::String).collect();
            return context.create_array(elements);
        }
        // Non-global match: return first match array (like exec) or null.
        let Some(caps) = regexp::exec_once(&re, &value) else {
            return Ok(JsValue::Null);
        };
        let match_str = caps[0].clone().unwrap_or_default();
        let index = value
            .find(match_str.as_str())
            .map(|b| value[..b].encode_utf16().count())
            .unwrap_or(0);
        let elements = caps
            .into_iter()
            .map(|c| c.map_or(JsValue::Undefined, JsValue::String))
            .collect();
        let result = context.create_array(elements)?;
        if let JsValue::Object(object) = result {
            context.define_own_property(
                object,
                "index".into(),
                PropertyDescriptor::data_with(JsValue::Number(index as f64), true, true, true),
            )?;
            context.define_own_property(
                object,
                "input".into(),
                PropertyDescriptor::data_with(JsValue::String(value), true, true, true),
            )?;
            return Ok(JsValue::Object(object));
        }
        return Ok(result);
    }
    // String fallback: coerce to string, find first occurrence.
    let search = arg_string(vm, context, arguments, 0)?;
    let Some(index) = string::index_of(&value, &search, 0) else {
        return Ok(JsValue::Null);
    };
    let result = context.create_array(vec![JsValue::String(search.clone())])?;
    if let JsValue::Object(object) = result {
        context.define_own_property(
            object,
            "index".into(),
            PropertyDescriptor::data_with(JsValue::Number(index as f64), true, true, true),
        )?;
        context.define_own_property(
            object,
            "input".into(),
            PropertyDescriptor::data_with(JsValue::String(value), true, true, true),
        )?;
        Ok(JsValue::Object(object))
    } else {
        Ok(result)
    }
}

/// `String.prototype.matchAll(regexp)` — ES2020.
///
/// Requires a global or sticky RegExp (`g` or `y` flag). Returns an array of
/// exec-style arrays (each with `index` and `input` properties) containing all
/// non-overlapping matches. Our engine has no lazy-iterator infrastructure so
/// we materialise all results eagerly into a plain Array — the spec requires a
/// RegExpStringIterator, but this approximation passes most basic tests.
fn string_match_all(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let first_arg = arg(arguments, 0);
    // @@matchAll dispatch
    let sym = context.well_known_symbols().match_all;
    if let Some(r) = try_symbol_dispatch(vm, context, sym, first_arg.clone(), value.clone(), &[]) {
        return r;
    }
    let (pattern, flags) = match regexp_data(context, &first_arg) {
        Some(pair) => pair,
        None => {
            // Coerce to string, treat as literal pattern with no flags.
            let s = vm.to_string_coerce(first_arg, context)?;
            (s, String::new())
        }
    };
    // matchAll requires the `g` or `y` flag on a regexp argument.
    if !flags.is_empty() && !flags.contains('g') && !flags.contains('y') {
        return Err(VmError::type_error(
            "String.prototype.matchAll called with a non-global RegExp",
        ));
    }
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;

    let mut entries = Vec::new();
    for caps in re.captures_iter(&value) {
        let m = caps.get(0).unwrap();
        let index = value[..m.start()].encode_utf16().count();
        let elements: Vec<JsValue> = (0..caps.len())
            .map(|i| {
                caps.get(i).map_or(JsValue::Undefined, |c| {
                    JsValue::String(c.as_str().to_owned())
                })
            })
            .collect();
        let entry = context.create_array(elements)?;
        if let JsValue::Object(oid) = entry {
            context.define_own_property(
                oid,
                "index".into(),
                PropertyDescriptor::data_with(JsValue::Number(index as f64), true, true, true),
            )?;
            context.define_own_property(
                oid,
                "input".into(),
                PropertyDescriptor::data_with(JsValue::String(value.clone()), true, true, true),
            )?;
            entries.push(JsValue::Object(oid));
        }
    }

    // Return a plain array (not a lazy iterator).
    context.create_array(entries)
}

fn string_pad_start(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    string_pad(vm, context, this, arguments, true)
}

fn string_pad_end(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    string_pad(vm, context, this, arguments, false)
}

fn string_pad(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
    at_start: bool,
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let target = to_integer_or_infinity(arg_number(vm, context, arguments, 0)?);
    let target = if target <= 0.0 {
        0usize
    } else {
        target as usize
    };
    let fill = match arguments.get(1) {
        None | Some(JsValue::Undefined) => " ".to_string(),
        Some(_) => arg_string(vm, context, arguments, 1)?,
    };
    let result = if at_start {
        string::pad_start(&value, target, &fill)
    } else {
        string::pad_end(&value, target, &fill)
    };
    result.map(JsValue::String).map_err(map_string_error)
}

fn string_trim(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::trim(&value)))
}

fn string_trim_start(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::trim_start(&value)))
}

fn string_trim_end(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::trim_end(&value)))
}

fn string_to_lower_case(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::to_lower_case(&value)))
}

fn string_to_upper_case(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::to_upper_case(&value)))
}

fn string_locale_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let other = arg_string(vm, context, arguments, 0)?;
    Ok(JsValue::Number(f64::from(string::locale_compare(
        &value, &other,
    ))))
}

fn string_to_locale_lower_case(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::to_lower_case(&value)))
}

fn string_to_locale_upper_case(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::to_upper_case(&value)))
}

fn string_normalize(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    let form = match arguments.first() {
        None | Some(JsValue::Undefined) => "NFC".to_string(),
        Some(_) => arg_string(vm, context, arguments, 0)?,
    };
    if !matches!(form.as_str(), "NFC" | "NFD" | "NFKC" | "NFKD") {
        return Err(VmError::range("invalid normalization form"));
    }
    Ok(JsValue::String(string::normalize(&value)))
}

fn string_is_well_formed(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::Boolean(string::is_well_formed(&value)))
}

fn string_to_well_formed(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = this_string(vm, context, this)?;
    Ok(JsValue::String(string::to_well_formed(&value)))
}

fn string_from_char_code(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut units = Vec::with_capacity(arguments.len());
    for index in 0..arguments.len() {
        units.push(to_uint16(arg_number(vm, context, arguments, index)?));
    }
    let units = string::from_char_codes(&units);
    Ok(JsValue::String(string::decode_utf16(&units)))
}

fn string_from_code_point(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut points = Vec::with_capacity(arguments.len());
    for index in 0..arguments.len() {
        let value = arg_number(vm, context, arguments, index)?;
        if value < 0.0 || value > f64::from(0x10_FFFF) || value.trunc() != value {
            return Err(VmError::range(format!("{value} is not a valid code point")));
        }
        points.push(value as u32);
    }
    let units = string::from_code_points(&points).map_err(map_string_error)?;
    Ok(JsValue::String(string::decode_utf16(&units)))
}

fn string_raw(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let template = arg(arguments, 0);
    let template_object = vm.to_object(template, context)?;
    let raw = vm.get_property_value(JsValue::Object(template_object), "raw", context)?;
    let raw_object = vm.to_object(raw, context)?;
    let length_value = vm.get_property_value(JsValue::Object(raw_object), "length", context)?;
    let length = to_length(vm.to_number(length_value, context)?);
    if length == 0 {
        return Ok(JsValue::String(String::new()));
    }

    let mut result = String::new();
    for index in 0..length {
        let literal =
            vm.get_property_value(JsValue::Object(raw_object), &index.to_string(), context)?;
        result.push_str(&vm.to_string_coerce(literal, context)?);
        if index + 1 < length
            && let Some(substitution) = arguments.get(index + 1)
        {
            result.push_str(&vm.to_string_coerce(substitution.clone(), context)?);
        }
    }
    Ok(JsValue::String(result))
}

// ── Math ─────────────────────────────────────────────────────────────────────

fn install_math(context: &mut NativeContext) -> Result<(), VmError> {
    let mut math = JsObject::ordinary();
    math.prototype = context.object_prototype();
    let math_object = context
        .heap_mut()
        .allocate_object(math)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    for constant in math::MATH_CONSTANTS {
        context.define_own_property(
            math_object,
            constant.name.into(),
            constant_descriptor(JsValue::Number(constant.value)),
        )?;
    }
    for spec in math::MATH_METHODS {
        let call =
            math_method_call(spec.name).ok_or_else(|| VmError::runtime("missing Math adapter"))?;
        define_method(context, math_object, spec.name, spec.length, call)?;
    }

    context.declare_global("Math", JsValue::Object(math_object));
    Ok(())
}

macro_rules! math_unary {
    ($function:ident, $name:literal) => {
        fn $function(
            vm: &mut Vm,
            context: &mut NativeContext,
            _this: JsValue,
            arguments: &[JsValue],
        ) -> Result<JsValue, VmError> {
            let value = arg_number(vm, context, arguments, 0)?;
            Ok(JsValue::Number(
                math::unary($name, value).unwrap_or(f64::NAN),
            ))
        }
    };
}

math_unary!(math_acos, "acos");
math_unary!(math_acosh, "acosh");
math_unary!(math_asin, "asin");
math_unary!(math_asinh, "asinh");
math_unary!(math_atan, "atan");
math_unary!(math_atanh, "atanh");
math_unary!(math_cbrt, "cbrt");
math_unary!(math_ceil, "ceil");
math_unary!(math_cos, "cos");
math_unary!(math_cosh, "cosh");
math_unary!(math_exp, "exp");
math_unary!(math_expm1, "expm1");
math_unary!(math_floor, "floor");
math_unary!(math_log, "log");
math_unary!(math_log10, "log10");
math_unary!(math_log1p, "log1p");
math_unary!(math_log2, "log2");
math_unary!(math_sin, "sin");
math_unary!(math_sinh, "sinh");
math_unary!(math_sqrt, "sqrt");
math_unary!(math_tan, "tan");
math_unary!(math_tanh, "tanh");

fn math_method_call(name: &str) -> Option<NativeCall> {
    Some(match name {
        "abs" => math_abs,
        "acos" => math_acos,
        "acosh" => math_acosh,
        "asin" => math_asin,
        "asinh" => math_asinh,
        "atan" => math_atan,
        "atan2" => math_atan2,
        "atanh" => math_atanh,
        "cbrt" => math_cbrt,
        "ceil" => math_ceil,
        "clz32" => math_clz32,
        "cos" => math_cos,
        "cosh" => math_cosh,
        "exp" => math_exp,
        "expm1" => math_expm1,
        "f16round" => math_f16round,
        "floor" => math_floor,
        "fround" => math_fround,
        "hypot" => math_hypot,
        "imul" => math_imul,
        "log" => math_log,
        "log10" => math_log10,
        "log1p" => math_log1p,
        "log2" => math_log2,
        "max" => math_max,
        "min" => math_min,
        "pow" => math_pow,
        "random" => math_random,
        "round" => math_round,
        "sign" => math_sign,
        "sin" => math_sin,
        "sinh" => math_sinh,
        "sumPrecise" => math_sum_precise,
        "sqrt" => math_sqrt,
        "tan" => math_tan,
        "tanh" => math_tanh,
        "trunc" => math_trunc,
        _ => return None,
    })
}

fn math_abs(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(math::abs(arg_number(
        vm, context, arguments, 0,
    )?)))
}

fn math_round(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(math::round(arg_number(
        vm, context, arguments, 0,
    )?)))
}

fn math_sign(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(math::sign(arg_number(
        vm, context, arguments, 0,
    )?)))
}

fn math_trunc(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(math::trunc(arg_number(
        vm, context, arguments, 0,
    )?)))
}

fn math_clz32(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(f64::from(math::clz32(arg_number(
        vm, context, arguments, 0,
    )?))))
}

fn math_fround(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(math::fround(arg_number(
        vm, context, arguments, 0,
    )?)))
}

fn math_f16round(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(math::f16round(arg_number(
        vm, context, arguments, 0,
    )?)))
}

fn math_imul(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = arg_number(vm, context, arguments, 0)?;
    let right = arg_number(vm, context, arguments, 1)?;
    Ok(JsValue::Number(f64::from(math::imul(left, right))))
}

fn math_pow(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let base = arg_number(vm, context, arguments, 0)?;
    let exponent = arg_number(vm, context, arguments, 1)?;
    Ok(JsValue::Number(math::pow(base, exponent)))
}

fn math_atan2(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let y = arg_number(vm, context, arguments, 0)?;
    let x = arg_number(vm, context, arguments, 1)?;
    Ok(JsValue::Number(y.atan2(x)))
}

fn math_max(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut values = Vec::with_capacity(arguments.len());
    for index in 0..arguments.len() {
        values.push(arg_number(vm, context, arguments, index)?);
    }
    Ok(JsValue::Number(math::max(&values)))
}

fn math_min(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut values = Vec::with_capacity(arguments.len());
    for index in 0..arguments.len() {
        values.push(arg_number(vm, context, arguments, index)?);
    }
    Ok(JsValue::Number(math::min(&values)))
}

fn math_hypot(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut values = Vec::with_capacity(arguments.len());
    for index in 0..arguments.len() {
        values.push(arg_number(vm, context, arguments, index)?);
    }
    Ok(JsValue::Number(math::hypot(&values)))
}

fn math_sum_precise(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arg(arguments, 0);
    let object = context.require_object(&source, "Math.sumPrecise")?;
    let length = context
        .get_own_property_descriptor(object, "length")
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|value| value.to_number())
        .unwrap_or(0.0)
        .max(0.0) as usize;
    if length > 1 << 20 {
        return Err(VmError::range("Math.sumPrecise: iterable too large"));
    }
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        let value = vm.get_property_value(source.clone(), &index.to_string(), context)?;
        values.push(vm.to_number(value, context)?);
    }
    Ok(JsValue::Number(math::sum_precise(&values)))
}

fn math_random(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0x9E37_79B9);
    let scrambled = (u64::from(seed))
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
        >> 11;
    let value = (scrambled as f64) / (1u64 << 53) as f64;
    Ok(JsValue::Number(value.fract().abs()))
}

// ── Global functions ─────────────────────────────────────────────────────────

fn install_global_functions(context: &mut NativeContext) -> Result<(), VmError> {
    let parse_int = context.register_builtin("parseInt", 2, global_parse_int, None)?;
    context.declare_global("parseInt", parse_int);
    let parse_float = context.register_builtin("parseFloat", 1, global_parse_float, None)?;
    context.declare_global("parseFloat", parse_float);
    let is_nan = context.register_builtin("isNaN", 1, global_is_nan, None)?;
    context.declare_global("isNaN", is_nan);
    let is_finite = context.register_builtin("isFinite", 1, global_is_finite, None)?;
    context.declare_global("isFinite", is_finite);
    let decode_uri =
        context.register_builtin("decodeURIComponent", 1, decode_uri_component, None)?;
    context.declare_global("decodeURIComponent", decode_uri);
    let encode_uri =
        context.register_builtin("encodeURIComponent", 1, encode_uri_component, None)?;
    context.declare_global("encodeURIComponent", encode_uri);
    Ok(())
}

fn global_parse_int(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arg_string(vm, context, arguments, 0)?;
    let radix = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => {
            let radix = to_integer_or_infinity(arg_number(vm, context, arguments, 1)?);
            Some(radix.clamp(i32::MIN as f64, i32::MAX as f64) as i32)
        }
    };
    Ok(JsValue::Number(number::parse_int(&source, radix)))
}

fn global_parse_float(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arg_string(vm, context, arguments, 0)?;
    Ok(JsValue::Number(number::parse_float(&source)))
}

fn global_is_nan(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arg_number(vm, context, arguments, 0)?;
    Ok(JsValue::Boolean(number::is_nan(value)))
}

fn global_is_finite(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arg_number(vm, context, arguments, 0)?;
    Ok(JsValue::Boolean(number::is_finite(value)))
}

fn decode_uri_component(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(arguments.first().cloned().unwrap_or(JsValue::Undefined))
}

fn encode_uri_component(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(arguments.first().cloned().unwrap_or(JsValue::Undefined))
}

// ── RegExp ────────────────────────────────────────────────────────────────────

fn install_regexp(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = context
        .regexp_prototype()
        .ok_or_else(|| VmError::runtime("regexp prototype missing"))?;
    let constructor = context.register_builtin("RegExp", 2, regexp_call, Some(regexp_construct))?;
    let JsValue::BuiltinFunction(id) = &constructor else {
        unreachable!()
    };
    let backing = context.builtin(*id).unwrap().object;

    context.define_own_property(
        backing,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    define_method(context, prototype, "test", 1, regexp_test)?;
    define_method(context, prototype, "exec", 1, regexp_exec)?;
    define_method(context, prototype, "toString", 0, regexp_to_string)?;

    // Install @@match / @@replace / @@split / @@matchAll / @@search on RegExp.prototype.
    // These are the entry points String methods dispatch to.
    let wk = *context.well_known_symbols();
    let sym_methods: &[(&str, crate::runtime::SymbolId, u8, NativeCall)] = &[
        ("[Symbol.match]", wk.match_, 1, regexp_symbol_match),
        ("[Symbol.replace]", wk.replace, 2, regexp_symbol_replace),
        ("[Symbol.split]", wk.split, 2, regexp_symbol_split),
        (
            "[Symbol.matchAll]",
            wk.match_all,
            1,
            regexp_symbol_match_all,
        ),
        ("[Symbol.search]", wk.search, 1, regexp_symbol_search),
    ];
    for (name, sym, arity, f) in sym_methods {
        let fn_val = context.register_builtin(name, *arity, *f, None)?;
        context.define_symbol_own_property(prototype, *sym, method_descriptor(fn_val))?;
    }

    context.declare_global("RegExp", constructor);
    Ok(())
}

fn regexp_call(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_make(context, arguments)
}

fn regexp_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    regexp_make(context, arguments)
}

fn regexp_make(context: &mut NativeContext, arguments: &[JsValue]) -> Result<JsValue, VmError> {
    let first = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    // If the first argument is already a RegExp, copy it (with optional flags override).
    if let Some((pattern, src_flags)) = regexp_data(context, &first) {
        let flags = match arguments.get(1) {
            None | Some(JsValue::Undefined) => src_flags,
            Some(JsValue::String(f)) => f.clone(),
            _ => {
                return Err(VmError::type_error(
                    "RegExp flags must be a string or undefined",
                ));
            }
        };
        return context.create_regexp(pattern, flags);
    }
    let pattern = match &first {
        JsValue::Undefined => String::new(),
        JsValue::String(s) => s.clone(),
        _ => {
            return Err(VmError::type_error(
                "RegExp pattern must be a string or RegExp",
            ));
        }
    };
    let flags = match arguments.get(1) {
        None | Some(JsValue::Undefined) => String::new(),
        Some(JsValue::String(f)) => f.clone(),
        _ => {
            return Err(VmError::type_error(
                "RegExp flags must be a string or undefined",
            ));
        }
    };
    context.create_regexp(pattern, flags)
}

fn regexp_test(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some((pattern, flags)) = regexp_data(context, &this) else {
        return Err(VmError::type_error(
            "RegExp.prototype.test called on non-RegExp",
        ));
    };
    let text = match arguments.first() {
        Some(JsValue::String(s)) => s.clone(),
        _ => {
            return Err(VmError::type_error(
                "RegExp.prototype.test requires a string argument",
            ));
        }
    };
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
    Ok(JsValue::Boolean(re.is_match(&text)))
}

fn regexp_exec(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some((pattern, flags)) = regexp_data(context, &this) else {
        return Err(VmError::type_error(
            "RegExp.prototype.exec called on non-RegExp",
        ));
    };
    let text = match arguments.first() {
        Some(JsValue::String(s)) => s.clone(),
        _ => {
            return Err(VmError::type_error(
                "RegExp.prototype.exec requires a string argument",
            ));
        }
    };
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
    let Some(caps) = regexp::exec_once(&re, &text) else {
        return Ok(JsValue::Null);
    };
    let match_str = caps[0].clone().unwrap_or_default();
    let index = text
        .find(match_str.as_str())
        .map(|b| text[..b].encode_utf16().count())
        .unwrap_or(0);
    let elements: Vec<JsValue> = caps
        .into_iter()
        .map(|c| c.map_or(JsValue::Undefined, JsValue::String))
        .collect();
    let result = context.create_array(elements)?;
    if let JsValue::Object(object) = result {
        context.define_own_property(
            object,
            "index".into(),
            PropertyDescriptor::data_with(JsValue::Number(index as f64), true, true, true),
        )?;
        context.define_own_property(
            object,
            "input".into(),
            PropertyDescriptor::data_with(JsValue::String(text), true, true, true),
        )?;
        Ok(JsValue::Object(object))
    } else {
        Ok(result)
    }
}

fn regexp_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some((pattern, flags)) = regexp_data(context, &this) else {
        return Err(VmError::type_error(
            "RegExp.prototype.toString called on non-RegExp",
        ));
    };
    Ok(JsValue::String(format!("/{pattern}/{flags}")))
}

// ── RegExp.prototype[@@symbol] methods ────────────────────────────────────────
//
// These are the dispatch targets for String.prototype.replace / match / split /
// matchAll / search.  Each receives `this = regexp`, `arguments[0] = string`.

fn require_regexp_this(
    context: &NativeContext,
    this: &JsValue,
) -> Result<(String, String), VmError> {
    regexp_data(context, this)
        .ok_or_else(|| VmError::type_error("RegExp Symbol method called on non-RegExp"))
}

fn regexp_symbol_replace(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (pattern, flags) = require_regexp_this(context, &this)?;
    let string = vm.to_string_coerce(arg(arguments, 0), context)?;
    let replace_arg = arg(arguments, 1);
    let global = flags.contains('g');
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
    if is_callable_value(&replace_arg) {
        return apply_replace_fn(vm, context, &string, &re, global, replace_arg);
    }
    let replacement = vm.to_string_coerce(replace_arg, context)?;
    Ok(JsValue::String(if global {
        regexp::replace_all(&re, &string, &replacement)
    } else {
        regexp::replace_first(&re, &string, &replacement)
    }))
}

fn regexp_symbol_match(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (pattern, flags) = require_regexp_this(context, &this)?;
    let string = vm.to_string_coerce(arg(arguments, 0), context)?;
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
    if flags.contains('g') {
        let matches = regexp::exec_global(&re, &string);
        if matches.is_empty() {
            return Ok(JsValue::Null);
        }
        let elements = matches.into_iter().map(JsValue::String).collect();
        return context.create_array(elements);
    }
    let Some(caps) = regexp::exec_once(&re, &string) else {
        return Ok(JsValue::Null);
    };
    let match_str = caps[0].clone().unwrap_or_default();
    let index = string
        .find(match_str.as_str())
        .map(|b| string[..b].encode_utf16().count())
        .unwrap_or(0);
    let elements = caps
        .into_iter()
        .map(|c| c.map_or(JsValue::Undefined, JsValue::String))
        .collect();
    let result = context.create_array(elements)?;
    if let JsValue::Object(oid) = result {
        context.define_own_property(
            oid,
            "index".into(),
            PropertyDescriptor::data_with(JsValue::Number(index as f64), true, true, true),
        )?;
        context.define_own_property(
            oid,
            "input".into(),
            PropertyDescriptor::data_with(JsValue::String(string), true, true, true),
        )?;
        return Ok(JsValue::Object(oid));
    }
    Ok(result)
}

fn regexp_symbol_match_all(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (pattern, flags) = require_regexp_this(context, &this)?;
    if !flags.contains('g') && !flags.contains('y') {
        return Err(VmError::type_error(
            "String.prototype.matchAll called with a non-global RegExp",
        ));
    }
    let string = vm.to_string_coerce(arg(arguments, 0), context)?;
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
    let mut entries = Vec::new();
    for caps in re.captures_iter(&string) {
        let m = caps.get(0).unwrap();
        let index = string[..m.start()].encode_utf16().count();
        let elements: Vec<JsValue> = (0..caps.len())
            .map(|i| {
                caps.get(i).map_or(JsValue::Undefined, |c| {
                    JsValue::String(c.as_str().to_owned())
                })
            })
            .collect();
        let entry = context.create_array(elements)?;
        if let JsValue::Object(oid) = entry {
            context.define_own_property(
                oid,
                "index".into(),
                PropertyDescriptor::data_with(JsValue::Number(index as f64), true, true, true),
            )?;
            context.define_own_property(
                oid,
                "input".into(),
                PropertyDescriptor::data_with(JsValue::String(string.clone()), true, true, true),
            )?;
            entries.push(JsValue::Object(oid));
        }
    }
    context.create_array(entries)
}

fn regexp_symbol_split(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (pattern, flags) = require_regexp_this(context, &this)?;
    let string = vm.to_string_coerce(arg(arguments, 0), context)?;
    let limit = match arguments.get(1) {
        None | Some(JsValue::Undefined) => None,
        Some(_) => Some(to_uint32(arg_number(vm, context, arguments, 1)?) as usize),
    };
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
    let parts = regexp::split(&re, &string, limit)
        .into_iter()
        .map(|v| v.map_or(JsValue::Undefined, JsValue::String))
        .collect();
    context.create_array(parts)
}

fn regexp_symbol_search(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (pattern, flags) = require_regexp_this(context, &this)?;
    let string = vm.to_string_coerce(arg(arguments, 0), context)?;
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|e| VmError::type_error(format!("invalid regex: {e}")))?;
    Ok(JsValue::Number(
        regexp::search(&re, &string).map_or(-1.0, |i| i as f64),
    ))
}

// ── Symbol ──────────────────────────────────────────────────────────────────

// Reflect

fn install_reflect(context: &mut NativeContext) -> Result<(), VmError> {
    let reflect = context.ordinary_object_with_prototype(context.object_prototype())?;
    let JsValue::Object(object) = reflect.clone() else {
        unreachable!()
    };

    for (name, length, call) in [
        ("apply", 3, reflect_apply as NativeCall),
        ("construct", 2, reflect_construct as NativeCall),
        ("defineProperty", 3, reflect_define_property as NativeCall),
        ("deleteProperty", 2, reflect_delete_property as NativeCall),
        ("get", 2, reflect_get as NativeCall),
        (
            "getOwnPropertyDescriptor",
            2,
            reflect_get_own_property_descriptor as NativeCall,
        ),
        ("getPrototypeOf", 1, reflect_get_prototype_of as NativeCall),
        ("has", 2, reflect_has as NativeCall),
        ("isExtensible", 1, reflect_is_extensible as NativeCall),
        ("ownKeys", 1, reflect_own_keys as NativeCall),
        (
            "preventExtensions",
            1,
            reflect_prevent_extensions as NativeCall,
        ),
        ("set", 3, reflect_set as NativeCall),
        ("setPrototypeOf", 2, reflect_set_prototype_of as NativeCall),
    ] {
        define_method(context, object, name, length, call)?;
    }

    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        object,
        to_string_tag,
        constant_descriptor(JsValue::String("Reflect".into())),
    )?;
    context.declare_global("Reflect", reflect);
    Ok(())
}

fn reflect_apply(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    if !is_callable_value(&target) {
        return Err(VmError::type_error("Reflect.apply target is not callable"));
    }
    let this_arg = arg(arguments, 1);
    let args = array_like_to_vec(vm, context, arg(arguments, 2))?;
    vm.call_value_from_builtin(target, this_arg, args, context)
}

fn reflect_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    if !context.is_constructable_value(&target) {
        return Err(VmError::type_error(
            "Reflect.construct target is not a constructor",
        ));
    }
    let new_target = arguments.get(2).cloned().unwrap_or_else(|| target.clone());
    if !context.is_constructable_value(&new_target) {
        return Err(VmError::type_error(
            "Reflect.construct newTarget is not a constructor",
        ));
    }
    let args = array_like_to_vec(vm, context, arg(arguments, 1))?;
    vm.construct_value_from_builtin(target, args, context)
}

fn reflect_define_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.defineProperty")?;
    let key_arg = arg(arguments, 1);
    let descriptor_value = arg(arguments, 2);
    let descriptor_object =
        context.require_object(&descriptor_value, "read property descriptor")?;
    let update = reflect_descriptor_update_from_object(vm, context, descriptor_object)?;

    let defined = if let JsValue::Symbol(symbol) = key_arg {
        let descriptor = reflect_descriptor_from_update(update);
        context.define_symbol_own_property(object, symbol, descriptor)?
    } else {
        let key = to_property_key(&key_arg)?;
        context.validate_and_apply_property_descriptor(object, key, update)?
    };
    Ok(JsValue::Boolean(defined))
}

fn reflect_delete_property(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.deleteProperty")?;
    let key_arg = arg(arguments, 1);
    if matches!(key_arg, JsValue::Symbol(_)) {
        return Ok(JsValue::Boolean(true));
    }
    let key = to_property_key(&key_arg)?;
    Ok(JsValue::Boolean(
        context.delete_property(object, &key, false)?,
    ))
}

fn reflect_get(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.get")?;
    let key_arg = arg(arguments, 1);
    if let JsValue::Symbol(symbol) = key_arg {
        return Ok(context
            .get_symbol_property_value(object, symbol)
            .unwrap_or(JsValue::Undefined));
    }
    let key = to_property_key(&key_arg)?;
    vm.get_property_value(target, &key, context)
}

fn reflect_get_own_property_descriptor(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.getOwnPropertyDescriptor")?;
    let key_arg = arg(arguments, 1);
    if matches!(key_arg, JsValue::Symbol(_)) {
        return Ok(JsValue::Undefined);
    }
    let key = to_property_key(&key_arg)?;
    let Some(descriptor) = context.get_own_property_descriptor(object, &key) else {
        return Ok(JsValue::Undefined);
    };
    reflect_descriptor_to_object(context, descriptor)
}

fn reflect_get_prototype_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.getPrototypeOf")?;
    Ok(context
        .get_prototype_of(object)
        .map_or(JsValue::Null, |prototype| context.object_value(prototype)))
}

fn reflect_has(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.has")?;
    let key_arg = arg(arguments, 1);
    if let JsValue::Symbol(symbol) = key_arg {
        let found = context
            .heap()
            .object(object)
            .is_some_and(|value| value.own_symbol_property(symbol).is_some());
        return Ok(JsValue::Boolean(found));
    }
    let key = to_property_key(&key_arg)?;
    Ok(JsValue::Boolean(context.has_property(object, &key)?))
}

fn reflect_is_extensible(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    context.require_object(&target, "Reflect.isExtensible")?;
    Ok(JsValue::Boolean(true))
}

fn reflect_own_keys(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.ownKeys")?;
    let heap_object = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    let mut keys: Vec<JsValue> = heap_object
        .own_property_keys()
        .into_iter()
        .map(JsValue::String)
        .collect();
    keys.extend(
        heap_object
            .symbol_properties
            .iter()
            .map(|(symbol, _)| JsValue::Symbol(*symbol)),
    );
    context.create_array(keys)
}

fn reflect_prevent_extensions(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    context.require_object(&target, "Reflect.preventExtensions")?;
    Ok(JsValue::Boolean(true))
}

fn reflect_set(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.set")?;
    let key_arg = arg(arguments, 1);
    let value = arg(arguments, 2);
    if let JsValue::Symbol(symbol) = key_arg {
        context.define_symbol_own_property(
            object,
            symbol,
            PropertyDescriptor::data_with(value, true, true, true),
        )?;
        return Ok(JsValue::Boolean(true));
    }
    let key = to_property_key(&key_arg)?;
    let set = vm.set_property_value_from_builtin(target, &key, value, context)?;
    Ok(JsValue::Boolean(set))
}

fn reflect_set_prototype_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let target = arg(arguments, 0);
    let object = context.require_object(&target, "Reflect.setPrototypeOf")?;
    let prototype = match arg(arguments, 1) {
        JsValue::Null => None,
        value => Some(context.require_object(&value, "Reflect.setPrototypeOf")?),
    };
    Ok(JsValue::Boolean(
        context.set_prototype_of(object, prototype)?,
    ))
}

fn array_like_to_vec(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    let object = context.require_object(&value, "read argument list")?;
    let object_value = context.object_value(object);
    let length_value = vm.get_property_value(object_value.clone(), "length", context)?;
    let length_number = vm.to_number(length_value, context)?;
    let length = if !length_number.is_finite() || length_number <= 0.0 {
        0
    } else {
        length_number.floor() as usize
    };
    if length > 1_000_000 {
        return Err(VmError::range("argument list is too large"));
    }
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        values.push(vm.get_property_value(object_value.clone(), &index.to_string(), context)?);
    }
    Ok(values)
}

fn is_callable_value(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn reflect_descriptor_update_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    descriptor_object: ObjectId,
) -> Result<PropertyDescriptorUpdate, VmError> {
    let mut update = PropertyDescriptorUpdate::default();
    if let Some(value) = reflect_descriptor_field(vm, context, descriptor_object, "value")? {
        update.value = Some(value);
    }
    if let Some(value) = reflect_descriptor_field(vm, context, descriptor_object, "writable")? {
        update.writable = Some(value.to_boolean());
    }
    if let Some(value) = reflect_descriptor_field(vm, context, descriptor_object, "enumerable")? {
        update.enumerable = Some(value.to_boolean());
    }
    if let Some(value) = reflect_descriptor_field(vm, context, descriptor_object, "configurable")? {
        update.configurable = Some(value.to_boolean());
    }
    if let Some(value) = reflect_descriptor_field(vm, context, descriptor_object, "get")? {
        update.get = Some(reflect_optional_callable(value, "getter")?);
    }
    if let Some(value) = reflect_descriptor_field(vm, context, descriptor_object, "set")? {
        update.set = Some(reflect_optional_callable(value, "setter")?);
    }
    Ok(update)
}

fn reflect_descriptor_field(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    key: &str,
) -> Result<Option<JsValue>, VmError> {
    if !context.has_property(object, key)? {
        return Ok(None);
    }
    vm.get_property_value(JsValue::Object(object), key, context)
        .map(Some)
}

fn reflect_optional_callable(value: JsValue, label: &str) -> Result<Option<JsValue>, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(None);
    }
    if is_callable_value(&value) {
        return Ok(Some(value));
    }
    Err(VmError::type_error(format!(
        "descriptor {label} is not callable"
    )))
}

fn reflect_descriptor_from_update(update: PropertyDescriptorUpdate) -> PropertyDescriptor {
    if update.get.is_some() || update.set.is_some() {
        return PropertyDescriptor::accessor(
            update.get.flatten(),
            update.set.flatten(),
            update.enumerable.unwrap_or(false),
            update.configurable.unwrap_or(false),
        );
    }
    PropertyDescriptor::data_with(
        update.value.unwrap_or(JsValue::Undefined),
        update.writable.unwrap_or(false),
        update.enumerable.unwrap_or(false),
        update.configurable.unwrap_or(false),
    )
}

fn reflect_descriptor_to_object(
    context: &mut NativeContext,
    descriptor: PropertyDescriptor,
) -> Result<JsValue, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = context.object_prototype();
    match descriptor.kind {
        PropertyKind::Data { value, writable } => {
            reflect_define_descriptor_field(&mut object, "value", value);
            reflect_define_descriptor_field(&mut object, "writable", JsValue::Boolean(writable));
        }
        PropertyKind::Accessor { get, set } => {
            reflect_define_descriptor_field(&mut object, "get", get.unwrap_or(JsValue::Undefined));
            reflect_define_descriptor_field(&mut object, "set", set.unwrap_or(JsValue::Undefined));
        }
    }
    reflect_define_descriptor_field(
        &mut object,
        "enumerable",
        JsValue::Boolean(descriptor.enumerable),
    );
    reflect_define_descriptor_field(
        &mut object,
        "configurable",
        JsValue::Boolean(descriptor.configurable),
    );
    let id = context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime("object arena exhausted"))?;
    Ok(JsValue::Object(id))
}

fn reflect_define_descriptor_field(object: &mut JsObject, name: &str, value: JsValue) {
    object.define_property(name, PropertyDescriptor::data_with(value, true, true, true));
}
fn install_symbol(context: &mut NativeContext) -> Result<(), VmError> {
    // Symbol is NOT a constructor — new Symbol() throws TypeError.
    let symbol_fn = context.register_builtin("Symbol", 0, symbol_call, None)?;
    let JsValue::BuiltinFunction(id) = &symbol_fn else {
        unreachable!()
    };
    let backing = context.builtin(*id).unwrap().object;

    // Install well-known symbols as non-writable, non-enumerable, non-configurable
    // properties on the Symbol function object (Symbol.toPrimitive etc.).
    let wk = *context.well_known_symbols();
    let well_known: &[(&str, crate::runtime::SymbolId)] = &[
        ("toPrimitive", wk.to_primitive),
        ("toStringTag", wk.to_string_tag),
        ("iterator", wk.iterator),
        ("hasInstance", wk.has_instance),
        ("isConcatSpreadable", wk.is_concat_spreadable),
        ("species", wk.species),
        ("match", wk.match_),
        ("replace", wk.replace),
        ("split", wk.split),
        ("matchAll", wk.match_all),
        ("search", wk.search),
    ];
    for (name, sym_id) in well_known {
        context.define_own_property(
            backing,
            (*name).into(),
            constant_descriptor(JsValue::Symbol(*sym_id)),
        )?;
    }

    // Symbol.prototype — a plain object; Symbol.prototype[@@toStringTag] = "Symbol"
    let proto = context
        .heap_mut()
        .allocate_object(JsObject::ordinary())
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    context.define_own_property(
        backing,
        "prototype".into(),
        constant_descriptor(JsValue::Object(proto)),
    )?;
    context.define_own_property(
        proto,
        "constructor".into(),
        method_descriptor(symbol_fn.clone()),
    )?;

    define_method(context, proto, "toString", 0, symbol_proto_to_string)?;
    define_method(context, proto, "valueOf", 0, symbol_proto_value_of)?;

    // `description` is an accessor getter — `Symbol('x').description` must return
    // the string "x", not the getter function itself.
    let desc_getter =
        context.register_builtin("get description", 0, symbol_proto_description, None)?;
    context.define_own_property(
        proto,
        "description".into(),
        PropertyDescriptor::accessor(Some(desc_getter), None, false, true),
    )?;

    // Symbol.prototype[@@toStringTag] = "Symbol"
    context.define_symbol_own_property(
        proto,
        wk.to_string_tag,
        constant_descriptor(JsValue::String("Symbol".into())),
    )?;

    // Symbol.for and Symbol.keyFor — global shared-key registry.
    define_method(context, backing, "for", 1, symbol_for)?;
    define_method(context, backing, "keyFor", 1, symbol_key_for)?;

    // Install Symbol.toStringTag on existing built-in prototypes so that
    // Object.prototype.toString returns the correct "[object X]" tag.
    install_to_string_tags(context, wk.to_string_tag)?;

    context.declare_global("Symbol", symbol_fn);
    Ok(())
}

/// Attach `Symbol.toStringTag` constants to the prototypes that need them.
fn install_to_string_tags(
    context: &mut NativeContext,
    to_string_tag: crate::runtime::SymbolId,
) -> Result<(), VmError> {
    // Collect (prototype_id, tag_string) pairs from already-installed globals.
    let mut pairs: Vec<(ObjectId, &'static str)> = Vec::new();

    macro_rules! push_proto {
        ($getter:ident, $tag:literal) => {
            if let Some(id) = context.$getter() {
                pairs.push((id, $tag));
            }
        };
    }

    push_proto!(object_prototype, "Object");
    push_proto!(function_prototype_object, "Function");
    push_proto!(array_prototype, "Array");
    push_proto!(string_prototype, "String");
    push_proto!(number_prototype, "Number");
    push_proto!(boolean_prototype, "Boolean");
    push_proto!(error_prototype, "Error");
    push_proto!(regexp_prototype, "RegExp");

    // Sub-error prototypes (TypeError, RangeError, etc.) get their own tag.
    for name in [
        "TypeError",
        "RangeError",
        "ReferenceError",
        "SyntaxError",
        "URIError",
        "EvalError",
    ] {
        if let Some(ctor) = context.get_global(name)
            && let Some(ctor_obj) = context.value_object(&ctor)
            && let Some(proto_desc) = context.get_own_property_descriptor(ctor_obj, "prototype")
            && let Some(JsValue::Object(proto_id)) = proto_desc.value_cloned()
        {
            pairs.push((proto_id, name));
        }
    }

    for (proto_id, tag) in pairs {
        context.define_symbol_own_property(
            proto_id,
            to_string_tag,
            constant_descriptor(JsValue::String(tag.into())),
        )?;
    }
    Ok(())
}

fn symbol_call(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let description = match arguments.first() {
        None | Some(JsValue::Undefined) => None,
        Some(JsValue::Symbol(_)) => {
            return Err(VmError::type_error(
                "Cannot convert a Symbol value to a string",
            ));
        }
        Some(other) => other.to_js_string(),
    };
    Ok(context.create_symbol(description))
}

fn symbol_proto_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let sym_id = extract_symbol(context, this)?;
    let desc = context.symbols().description(sym_id);
    let tag = match desc {
        Some(d) => format!("Symbol({d})"),
        None => "Symbol()".into(),
    };
    Ok(JsValue::String(tag))
}

fn symbol_proto_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let sym_id = extract_symbol(context, this.clone())?;
    Ok(JsValue::Symbol(sym_id))
}

fn symbol_proto_description(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let sym_id = extract_symbol(context, this)?;
    Ok(match context.symbols().description(sym_id) {
        Some(d) => JsValue::String(d.into()),
        None => JsValue::Undefined,
    })
}

/// ECMAScript `ToString(value)`: coerce a value to a Rust `String`.
/// For object types, calls the `toString()` method via the VM so that
/// custom `toString` implementations (as in test262 fixtures) are honoured.
fn vm_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<String, VmError> {
    match &value {
        JsValue::Symbol(_) => Err(VmError::type_error(
            "Cannot convert a Symbol value to a string",
        )),
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            // Call value.toString() via the VM so user-defined toString is respected.
            let to_string_fn = vm.get_property_value(value.clone(), "toString", context)?;
            let result = vm.call_value_from_builtin(to_string_fn, value, Vec::new(), context)?;
            result
                .to_js_string()
                .ok_or_else(|| VmError::type_error("toString did not return a string"))
        }
        other => other
            .to_js_string()
            .ok_or_else(|| VmError::type_error("Cannot convert value to string")),
    }
}

/// Extract the underlying SymbolId from a Symbol primitive or Symbol wrapper.
fn extract_symbol(
    context: &NativeContext,
    this: JsValue,
) -> Result<crate::runtime::SymbolId, crate::vm::VmError> {
    match this {
        JsValue::Symbol(id) => Ok(id),
        JsValue::Object(object) => match context.primitive_value(object) {
            Some(PrimitiveValue::Symbol(id)) => Ok(*id),
            _ => Err(VmError::type_error(
                "Symbol.prototype method called on non-symbol",
            )),
        },
        _ => Err(VmError::type_error(
            "Symbol.prototype method called on non-symbol",
        )),
    }
}

fn symbol_for(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let arg = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let key = vm_to_string(vm, context, arg)?;
    Ok(context.symbol_for(key))
}

fn symbol_key_for(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    match arguments.first() {
        Some(JsValue::Symbol(id)) => {
            let id = *id;
            Ok(context
                .symbol_key_for(id)
                .map(|k| JsValue::String(k.into()))
                .unwrap_or(JsValue::Undefined))
        }
        _ => Err(VmError::type_error("is not a symbol")),
    }
}
