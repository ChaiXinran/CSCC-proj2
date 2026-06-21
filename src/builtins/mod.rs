//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

use crate::{
    runtime::{
        Intrinsics, JsObject, JsValue, NativeContext, NativeErrorKind, NativeErrorValue,
        PropertyDescriptor,
    },
    vm::{Vm, VmError, VmErrorKind},
};

/// Installs the foundational constructors, prototypes, and V4 methods.
pub fn install_foundation(context: &mut NativeContext) {
    if context.intrinsics().is_some() {
        return;
    }
    install_globals(context).expect("builtin foundation installation must succeed");
    object::install_object(context);
    array::install_array(context);
    function::install_function(context);
    install_std_globals(context).expect("std globals installation must succeed");
}

fn install_globals(context: &mut NativeContext) -> Result<(), VmError> {
    let object_prototype = context
        .heap_mut()
        .allocate_object(JsObject::ordinary())
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut function_prototype_object = JsObject::ordinary();
    function_prototype_object.prototype = Some(object_prototype);
    let function_prototype = context
        .heap_mut()
        .allocate_object(function_prototype_object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let object_constructor = context.register_builtin(
        "Object",
        1,
        object::object_call,
        Some(object::object_construct),
    )?;
    let JsValue::BuiltinFunction(object_id) = object_constructor else {
        unreachable!()
    };
    let object_backing = context.builtin(object_id).unwrap().object;
    context.set_prototype_of(object_backing, Some(function_prototype))?;
    context.define_own_property(
        object_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(object_prototype), false, false, false),
    )?;
    context.define_own_property(
        object_prototype,
        "constructor".into(),
        PropertyDescriptor::data_with(object_constructor.clone(), true, false, true),
    )?;

    let function_constructor = context.register_builtin(
        "Function",
        1,
        function::function_call,
        Some(function::function_construct),
    )?;
    let JsValue::BuiltinFunction(function_id) = function_constructor else {
        unreachable!()
    };
    let function_backing = context.builtin(function_id).unwrap().object;
    context.set_prototype_of(function_backing, Some(function_prototype))?;
    context.define_own_property(
        function_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(function_prototype), false, false, false),
    )?;
    context.define_own_property(
        function_prototype,
        "constructor".into(),
        PropertyDescriptor::data_with(function_constructor.clone(), true, false, true),
    )?;

    let mut array_prototype_object = JsObject::sparse_array(0);
    array_prototype_object.prototype = Some(object_prototype);
    let array_prototype = context
        .heap_mut()
        .allocate_object(array_prototype_object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let array_constructor =
        context.register_builtin("Array", 1, array::array_call, Some(array::array_construct))?;
    let JsValue::BuiltinFunction(array_id) = array_constructor else {
        unreachable!()
    };
    let array_backing = context.builtin(array_id).unwrap().object;
    context.set_prototype_of(array_backing, Some(function_prototype))?;
    context.define_own_property(
        array_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(array_prototype), false, false, false),
    )?;
    context.define_own_property(
        array_prototype,
        "constructor".into(),
        PropertyDescriptor::data_with(array_constructor.clone(), true, false, true),
    )?;

    context.set_intrinsics(Intrinsics {
        object_prototype,
        function_prototype,
        array_prototype,
        object_constructor: object_constructor.clone(),
        function_constructor: function_constructor.clone(),
        array_constructor: array_constructor.clone(),
    });
    context.declare_global("Object", object_constructor);
    context.declare_global("Function", function_constructor);
    context.declare_global("Array", array_constructor);
    Ok(())
}

/// Installs the minimal Test262 host surface used by the native runtime.
pub fn install_test262_harness(context: &mut NativeContext) {
    // assert is a callable function AND has sub-methods as properties
    let assert_fn = context
        .register_builtin("assert", 1, assert_fn, None)
        .expect("install assert");
    let JsValue::BuiltinFunction(assert_id) = &assert_fn else {
        unreachable!()
    };
    let assert_backing = context.builtin(*assert_id).unwrap().object;

    let same_value = context
        .register_builtin("sameValue", 2, assert_same_value, None)
        .expect("install assert.sameValue");
    let not_same_value = context
        .register_builtin("notSameValue", 2, assert_not_same_value, None)
        .expect("install assert.notSameValue");
    let throws = context
        .register_builtin("throws", 1, assert_throws, None)
        .expect("install assert.throws");

    context
        .define_own_property(
            assert_backing,
            "sameValue".into(),
            PropertyDescriptor::data(same_value),
        )
        .expect("define assert.sameValue");
    context
        .define_own_property(
            assert_backing,
            "notSameValue".into(),
            PropertyDescriptor::data(not_same_value),
        )
        .expect("define assert.notSameValue");
    context
        .define_own_property(
            assert_backing,
            "throws".into(),
            PropertyDescriptor::data(throws),
        )
        .expect("define assert.throws");

    context.declare_global("assert", assert_fn);

    let test262_error = context
        .register_builtin(
            "Test262Error",
            1,
            test262_error_call,
            Some(test262_error_construct),
        )
        .expect("install Test262Error");
    context.declare_global("Test262Error", test262_error);
}

fn assert_fn(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let condition = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if condition.to_boolean() {
        Ok(JsValue::Undefined)
    } else {
        let message = arguments
            .get(1)
            .and_then(JsValue::to_js_string)
            .unwrap_or_else(|| "assertion failed".into());
        Err(VmError::test262(message))
    }
}

fn assert_throws(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let func = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !matches!(func, JsValue::Function(_) | JsValue::BuiltinFunction(_)) {
        return Err(VmError::type_error("assert.throws: second argument must be callable"));
    }
    let result = vm.call_value(func, JsValue::Undefined, vec![], context);
    match result {
        Err(_) => Ok(JsValue::Undefined),
        Ok(_) => Err(VmError::test262("assert.throws: no exception was thrown".to_string())),
    }
}

fn assert_same_value(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let actual = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let expected = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if actual.same_value(&expected) {
        Ok(JsValue::Undefined)
    } else {
        Err(assertion_error(
            arguments,
            format!("expected SameValue({actual}, {expected})"),
        ))
    }
}

fn assert_not_same_value(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let actual = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let unexpected = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !actual.same_value(&unexpected) {
        Ok(JsValue::Undefined)
    } else {
        Err(assertion_error(
            arguments,
            format!("expected values not to be SameValue: {actual}"),
        ))
    }
}

fn test262_error_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::test262(
        arguments
            .first()
            .and_then(JsValue::to_js_string)
            .unwrap_or_else(|| "Test262Error".into()),
    ))
}

fn test262_error_construct(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    let message = arguments
        .first()
        .and_then(JsValue::to_js_string)
        .unwrap_or_else(|| "Test262Error".into());
    Ok(JsValue::Error(NativeErrorValue::new(
        NativeErrorKind::Test262,
        message,
    )))
}

// ── Standard globals (Error hierarchy, Math, String, Number, Boolean) ────────

fn install_std_globals(context: &mut NativeContext) -> Result<(), VmError> {
    // Error hierarchy
    for name in ["Error", "TypeError", "RangeError", "ReferenceError", "SyntaxError", "EvalError", "URIError"] {
        let ctor = context.register_builtin(name, 1, error_ctor_call, Some(error_ctor_construct))?;
        context.declare_global(name, ctor);
    }

    // Number — with construct support
    let number = context.register_builtin("Number", 1, number_call, Some(number_construct))?;
    if let JsValue::BuiltinFunction(id) = &number {
        let obj = context.builtin(*id).unwrap().object;
        // Number.prototype object — prototype for all `new Number(...)` instances
        let num_proto = context.heap_mut().allocate_object(JsObject::ordinary())
            .ok_or_else(|| VmError::runtime("heap exhausted"))?;
        if let (Some(object_proto), Some(o)) = (context.object_prototype(), context.heap_mut().object_mut(num_proto)) { o.prototype = Some(object_proto); }
        let value_of = context.register_builtin("valueOf", 0, number_value_of, None)?;
        context.define_own_property(num_proto, "valueOf".into(), PropertyDescriptor::data_with(value_of, true, false, true))?;
        let to_string = context.register_builtin("toString", 0, number_to_string_method, None)?;
        context.define_own_property(num_proto, "toString".into(), PropertyDescriptor::data_with(to_string, true, false, true))?;
        context.define_own_property(num_proto, "constructor".into(), PropertyDescriptor::data_with(number.clone(), true, false, true))?;
        context.define_own_property(obj, "prototype".into(), PropertyDescriptor::data_with(JsValue::Object(num_proto), false, false, false))?;

        for (k, v) in &[
            ("MAX_VALUE", f64::MAX),
            ("MIN_VALUE", 5e-324),
            ("POSITIVE_INFINITY", f64::INFINITY),
            ("NEGATIVE_INFINITY", f64::NEG_INFINITY),
            ("NaN", f64::NAN),
            ("MAX_SAFE_INTEGER", 9_007_199_254_740_991.0_f64),
            ("MIN_SAFE_INTEGER", -9_007_199_254_740_991.0_f64),
            ("EPSILON", f64::EPSILON),
        ] {
            context.define_own_property(
                obj,
                (*k).into(),
                PropertyDescriptor::data_with(JsValue::Number(*v), false, false, false),
            )?;
        }
        let is_nan = context.register_builtin("isNaN", 1, number_is_nan, None)?;
        context.define_own_property(obj, "isNaN".into(), PropertyDescriptor::data_with(is_nan, true, false, true))?;
        let is_finite = context.register_builtin("isFinite", 1, number_is_finite, None)?;
        context.define_own_property(obj, "isFinite".into(), PropertyDescriptor::data_with(is_finite, true, false, true))?;
        let is_integer = context.register_builtin("isInteger", 1, number_is_integer, None)?;
        context.define_own_property(obj, "isInteger".into(), PropertyDescriptor::data_with(is_integer, true, false, true))?;
        let is_safe_int = context.register_builtin("isSafeInteger", 1, number_is_safe_integer, None)?;
        context.define_own_property(obj, "isSafeInteger".into(), PropertyDescriptor::data_with(is_safe_int, true, false, true))?;
        let parse_int = context.register_builtin("parseInt", 1, global_parse_int, None)?;
        context.define_own_property(obj, "parseInt".into(), PropertyDescriptor::data_with(parse_int, true, false, true))?;
        let parse_float = context.register_builtin("parseFloat", 1, global_parse_float, None)?;
        context.define_own_property(obj, "parseFloat".into(), PropertyDescriptor::data_with(parse_float, true, false, true))?;
    }
    context.declare_global("Number", number);

    // Boolean — with construct support
    let boolean = context.register_builtin("Boolean", 1, boolean_call, Some(boolean_construct))?;
    if let JsValue::BuiltinFunction(id) = &boolean {
        let obj = context.builtin(*id).unwrap().object;
        let bool_proto = context.heap_mut().allocate_object(JsObject::ordinary())
            .ok_or_else(|| VmError::runtime("heap exhausted"))?;
        if let (Some(object_proto), Some(o)) = (context.object_prototype(), context.heap_mut().object_mut(bool_proto)) { o.prototype = Some(object_proto); }
        let value_of = context.register_builtin("valueOf", 0, boolean_value_of, None)?;
        context.define_own_property(bool_proto, "valueOf".into(), PropertyDescriptor::data_with(value_of, true, false, true))?;
        context.define_own_property(bool_proto, "constructor".into(), PropertyDescriptor::data_with(boolean.clone(), true, false, true))?;
        context.define_own_property(obj, "prototype".into(), PropertyDescriptor::data_with(JsValue::Object(bool_proto), false, false, false))?;
    }
    context.declare_global("Boolean", boolean);

    // String — with construct support
    let string_fn = context.register_builtin("String", 1, string_call, Some(string_construct))?;
    if let JsValue::BuiltinFunction(id) = &string_fn {
        let obj = context.builtin(*id).unwrap().object;
        let str_proto = context.heap_mut().allocate_object(JsObject::ordinary())
            .ok_or_else(|| VmError::runtime("heap exhausted"))?;
        if let (Some(object_proto), Some(o)) = (context.object_prototype(), context.heap_mut().object_mut(str_proto)) { o.prototype = Some(object_proto); }
        let value_of = context.register_builtin("valueOf", 0, string_value_of, None)?;
        context.define_own_property(str_proto, "valueOf".into(), PropertyDescriptor::data_with(value_of, true, false, true))?;
        let to_string_str = context.register_builtin("toString", 0, string_to_string_method, None)?;
        context.define_own_property(str_proto, "toString".into(), PropertyDescriptor::data_with(to_string_str, true, false, true))?;
        context.define_own_property(str_proto, "constructor".into(), PropertyDescriptor::data_with(string_fn.clone(), true, false, true))?;
        context.define_own_property(obj, "prototype".into(), PropertyDescriptor::data_with(JsValue::Object(str_proto), false, false, false))?;

        let from_char_code = context.register_builtin("fromCharCode", 1, string_from_char_code, None)?;
        context.define_own_property(obj, "fromCharCode".into(), PropertyDescriptor::data_with(from_char_code, true, false, true))?;
    }
    context.declare_global("String", string_fn);

    // Math
    let math_obj = context.heap_mut().allocate_object(crate::runtime::JsObject::ordinary())
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    // Math constants
    for (k, v) in &[
        ("PI", std::f64::consts::PI),
        ("E", std::f64::consts::E),
        ("LN2", std::f64::consts::LN_2),
        ("LN10", std::f64::consts::LN_10),
        ("LOG2E", std::f64::consts::LOG2_E),
        ("LOG10E", std::f64::consts::LOG10_E),
        ("SQRT2", std::f64::consts::SQRT_2),
    ] {
        context.define_own_property(
            math_obj,
            (*k).into(),
            PropertyDescriptor::data_with(JsValue::Number(*v), false, false, false),
        )?;
    }
    // Math functions
    for (name, length, call) in [
        ("abs", 1, math_abs as crate::runtime::NativeCall),
        ("floor", 1, math_floor as crate::runtime::NativeCall),
        ("ceil", 1, math_ceil as crate::runtime::NativeCall),
        ("round", 1, math_round as crate::runtime::NativeCall),
        ("sqrt", 1, math_sqrt as crate::runtime::NativeCall),
        ("pow", 2, math_pow as crate::runtime::NativeCall),
        ("max", 2, math_max as crate::runtime::NativeCall),
        ("min", 2, math_min as crate::runtime::NativeCall),
        ("random", 0, math_random as crate::runtime::NativeCall),
        ("log", 1, math_log as crate::runtime::NativeCall),
        ("log2", 1, math_log2 as crate::runtime::NativeCall),
        ("log10", 1, math_log10 as crate::runtime::NativeCall),
        ("exp", 1, math_exp as crate::runtime::NativeCall),
        ("sin", 1, math_sin as crate::runtime::NativeCall),
        ("cos", 1, math_cos as crate::runtime::NativeCall),
        ("tan", 1, math_tan as crate::runtime::NativeCall),
        ("asin", 1, math_asin as crate::runtime::NativeCall),
        ("acos", 1, math_acos as crate::runtime::NativeCall),
        ("atan", 1, math_atan as crate::runtime::NativeCall),
        ("atan2", 2, math_atan2 as crate::runtime::NativeCall),
        ("sign", 1, math_sign as crate::runtime::NativeCall),
        ("trunc", 1, math_trunc as crate::runtime::NativeCall),
        ("hypot", 2, math_hypot as crate::runtime::NativeCall),
        ("cbrt", 1, math_cbrt as crate::runtime::NativeCall),
        ("clz32", 1, math_clz32 as crate::runtime::NativeCall),
        ("fround", 1, math_fround as crate::runtime::NativeCall),
        ("imul", 2, math_imul as crate::runtime::NativeCall),
    ] {
        let v = context.register_builtin(name, length, call, None)?;
        context.define_own_property(math_obj, name.into(), PropertyDescriptor::data_with(v, true, false, true))?;
    }
    context.declare_global("Math", JsValue::Object(math_obj));

    // Global functions
    let parse_int = context.register_builtin("parseInt", 2, global_parse_int, None)?;
    context.declare_global("parseInt", parse_int);
    let parse_float = context.register_builtin("parseFloat", 1, global_parse_float, None)?;
    context.declare_global("parseFloat", parse_float);
    let is_nan = context.register_builtin("isNaN", 1, global_is_nan, None)?;
    context.declare_global("isNaN", is_nan);
    let is_finite = context.register_builtin("isFinite", 1, global_is_finite, None)?;
    context.declare_global("isFinite", is_finite);
    let decode_uri = context.register_builtin("decodeURIComponent", 1, decode_uri_component, None)?;
    context.declare_global("decodeURIComponent", decode_uri);
    let encode_uri = context.register_builtin("encodeURIComponent", 1, encode_uri_component, None)?;
    context.declare_global("encodeURIComponent", encode_uri);

    Ok(())
}

fn error_ctor_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let message = arguments
        .first()
        .and_then(JsValue::to_js_string)
        .unwrap_or_default();
    Ok(JsValue::Error(NativeErrorValue::new(NativeErrorKind::Error, message)))
}

fn error_ctor_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    let message = arguments
        .first()
        .and_then(JsValue::to_js_string)
        .unwrap_or_default();
    let obj = context.create_object(vec![])?;
    if let JsValue::Object(id) = obj {
        context.define_own_property(
            id,
            "message".into(),
            PropertyDescriptor::data_with(JsValue::String(message), true, false, true),
        )?;
    }
    Ok(obj)
}

fn number_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let val = arguments.first().cloned().unwrap_or(JsValue::Number(0.0));
    Ok(JsValue::Number(val.to_number().unwrap_or(f64::NAN)))
}

fn boolean_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let val = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    Ok(JsValue::Boolean(val.to_boolean()))
}

fn string_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let s = arguments
        .first()
        .cloned()
        .unwrap_or(JsValue::Undefined);
    match s {
        JsValue::Undefined => Ok(JsValue::String("undefined".into())),
        JsValue::Null => Ok(JsValue::String("null".into())),
        JsValue::Boolean(b) => Ok(JsValue::String(b.to_string())),
        JsValue::Number(n) => {
            if n.is_nan() { return Ok(JsValue::String("NaN".into())); }
            if n.is_infinite() { return Ok(JsValue::String(if n > 0.0 { "Infinity".into() } else { "-Infinity".into() })); }
            if n == 0.0 { return Ok(JsValue::String("0".into())); }
            let i = n as i64;
            if i as f64 == n { return Ok(JsValue::String(i.to_string())); }
            Ok(JsValue::String(format!("{n}")))
        }
        JsValue::String(s) => Ok(JsValue::String(s)),
        other => Ok(JsValue::String(other.to_js_string().unwrap_or_else(|| "[object Object]".into()))),
    }
}

fn string_from_char_code(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let s: String = arguments
        .iter()
        .filter_map(|v| v.to_number())
        .filter_map(|n| char::from_u32(n as u32))
        .collect();
    Ok(JsValue::String(s))
}

// ── Primitive wrapper constructors ───────────────────────────────────────────

/// Internal property name for primitive wrapper value (not accessible via normal JS property access).
const PRIMITIVE_VALUE_KEY: &str = "\u{0}primitiveValue";

fn make_wrapper_object(
    context: &mut NativeContext,
    new_target: &JsValue,
    primitive: JsValue,
) -> Result<JsValue, VmError> {
    let proto = context.constructor_prototype(new_target)?;
    let mut obj = JsObject::ordinary();
    obj.prototype = proto;
    // Store primitive value as non-enumerable property with NUL-prefixed name (inaccessible from JS).
    obj.define_property(
        PRIMITIVE_VALUE_KEY,
        PropertyDescriptor::data_with(primitive, false, false, false),
    );
    let id = context.heap_mut().allocate_object(obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;
    Ok(JsValue::Object(id))
}

fn get_primitive_value(context: &NativeContext, this: &JsValue) -> Option<JsValue> {
    let obj_id = context.value_object(this)?;
    context.heap().object(obj_id)
        .and_then(|o| o.own_property(PRIMITIVE_VALUE_KEY))
        .and_then(|d| d.value_cloned())
}

fn number_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let primitive = arguments.first()
        .and_then(|v| v.to_number())
        .unwrap_or(0.0);
    make_wrapper_object(context, &new_target, JsValue::Number(primitive))
}

fn boolean_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let primitive = arguments.first().map(|v| v.to_boolean()).unwrap_or(false);
    make_wrapper_object(context, &new_target, JsValue::Boolean(primitive))
}

fn string_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let primitive = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined => JsValue::String(String::new()),
        JsValue::String(s) => JsValue::String(s),
        v => JsValue::String(v.to_js_string().unwrap_or_default()),
    };
    make_wrapper_object(context, &new_target, primitive)
}

fn number_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _args: &[JsValue],
) -> Result<JsValue, VmError> {
    if let Some(JsValue::Number(_)) = get_primitive_value(context, &this) {
        return Ok(get_primitive_value(context, &this).unwrap());
    }
    if let JsValue::Number(_) = &this { return Ok(this); }
    Err(VmError::type_error("Number.prototype.valueOf: not a number"))
}

fn number_to_string_method(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _args: &[JsValue],
) -> Result<JsValue, VmError> {
    let n = if let Some(JsValue::Number(n)) = get_primitive_value(context, &this) {
        n
    } else if let JsValue::Number(n) = this {
        n
    } else {
        return Err(VmError::type_error("Number.prototype.toString: not a number"));
    };
    if n.is_nan() { return Ok(JsValue::String("NaN".into())); }
    if n.is_infinite() { return Ok(JsValue::String(if n > 0.0 { "Infinity".into() } else { "-Infinity".into() })); }
    let i = n as i64;
    if i as f64 == n { return Ok(JsValue::String(i.to_string())); }
    Ok(JsValue::String(format!("{n}")))
}

fn boolean_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _args: &[JsValue],
) -> Result<JsValue, VmError> {
    if let Some(JsValue::Boolean(b)) = get_primitive_value(context, &this) {
        return Ok(JsValue::Boolean(b));
    }
    if let JsValue::Boolean(b) = this { return Ok(JsValue::Boolean(b)); }
    Err(VmError::type_error("Boolean.prototype.valueOf: not a boolean"))
}

fn string_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _args: &[JsValue],
) -> Result<JsValue, VmError> {
    if let Some(JsValue::String(s)) = get_primitive_value(context, &this) {
        return Ok(JsValue::String(s));
    }
    if let JsValue::String(s) = this { return Ok(JsValue::String(s)); }
    Err(VmError::type_error("String.prototype.valueOf: not a string"))
}

fn string_to_string_method(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    args: &[JsValue],
) -> Result<JsValue, VmError> {
    string_value_of(vm, context, this, args)
}

fn number_is_nan(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    let v = args.first().and_then(|v| v.to_number());
    Ok(JsValue::Boolean(v.map(f64::is_nan).unwrap_or(false)))
}

fn number_is_finite(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    match args.first() {
        Some(JsValue::Number(n)) => Ok(JsValue::Boolean(n.is_finite())),
        _ => Ok(JsValue::Boolean(false)),
    }
}

fn number_is_integer(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    match args.first() {
        Some(JsValue::Number(n)) if n.is_finite() => Ok(JsValue::Boolean(*n == n.trunc())),
        _ => Ok(JsValue::Boolean(false)),
    }
}

fn number_is_safe_integer(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    match args.first() {
        Some(JsValue::Number(n)) if n.is_finite() && *n == n.trunc() =>
            Ok(JsValue::Boolean(n.abs() <= 9_007_199_254_740_991.0)),
        _ => Ok(JsValue::Boolean(false)),
    }
}

fn global_parse_int(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    let s = match args.first() {
        Some(JsValue::String(s)) => s.trim().to_string(),
        Some(JsValue::Number(n)) => {
            if n.is_nan() { return Ok(JsValue::Number(f64::NAN)); }
            return Ok(JsValue::Number(n.trunc()));
        }
        _ => return Ok(JsValue::Number(f64::NAN)),
    };
    let radix = args.get(1).and_then(|v| v.to_number()).map(|n| n as u32).unwrap_or(10);
    let radix = if radix == 0 { 10 } else if !(2..=36).contains(&radix) { return Ok(JsValue::Number(f64::NAN)); } else { radix };
    let s = if s.starts_with("0x") || s.starts_with("0X") { &s[2..] } else { &s };
    match i64::from_str_radix(s, radix) {
        Ok(n) => Ok(JsValue::Number(n as f64)),
        Err(_) => {
            // Parse as many valid digits as possible
            let valid: String = s.chars().take_while(|c| c.is_digit(radix)).collect();
            if valid.is_empty() { return Ok(JsValue::Number(f64::NAN)); }
            Ok(JsValue::Number(i64::from_str_radix(&valid, radix).map(|n| n as f64).unwrap_or(f64::NAN)))
        }
    }
}

fn global_parse_float(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    let s = match args.first() {
        Some(JsValue::String(s)) => s.trim().to_string(),
        Some(JsValue::Number(n)) => return Ok(JsValue::Number(*n)),
        _ => return Ok(JsValue::Number(f64::NAN)),
    };
    Ok(JsValue::Number(s.parse::<f64>().unwrap_or(f64::NAN)))
}

fn global_is_nan(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Boolean(n.is_nan()))
}

fn global_is_finite(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Boolean(n.is_finite()))
}

fn decode_uri_component(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(args.first().cloned().unwrap_or(JsValue::Undefined))
}

fn encode_uri_component(
    _vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(args.first().cloned().unwrap_or(JsValue::Undefined))
}

// ── Math functions ────────────────────────────────────────────────────────────

macro_rules! math_f64_fn {
    ($name:ident, $method:ident) => {
        fn $name(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
            let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
            Ok(JsValue::Number(n.$method()))
        }
    };
}

math_f64_fn!(math_abs, abs);
math_f64_fn!(math_floor, floor);
math_f64_fn!(math_ceil, ceil);
math_f64_fn!(math_round, round);
math_f64_fn!(math_sqrt, sqrt);
math_f64_fn!(math_log, ln);
math_f64_fn!(math_log2, log2);
math_f64_fn!(math_log10, log10);
math_f64_fn!(math_exp, exp);
math_f64_fn!(math_sin, sin);
math_f64_fn!(math_cos, cos);
math_f64_fn!(math_tan, tan);
math_f64_fn!(math_asin, asin);
math_f64_fn!(math_acos, acos);
math_f64_fn!(math_atan, atan);
math_f64_fn!(math_sign, signum);
math_f64_fn!(math_trunc, trunc);
math_f64_fn!(math_cbrt, cbrt);

fn math_pow(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let base = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
    let exp = args.get(1).and_then(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Number(base.powf(exp)))
}

fn math_atan2(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let y = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
    let x = args.get(1).and_then(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Number(y.atan2(x)))
}

fn math_max(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    if args.is_empty() { return Ok(JsValue::Number(f64::NEG_INFINITY)); }
    let mut m = f64::NEG_INFINITY;
    for v in args {
        let n = v.to_number().unwrap_or(f64::NAN);
        if n.is_nan() { return Ok(JsValue::Number(f64::NAN)); }
        if n > m { m = n; }
    }
    Ok(JsValue::Number(m))
}

fn math_min(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    if args.is_empty() { return Ok(JsValue::Number(f64::INFINITY)); }
    let mut m = f64::INFINITY;
    for v in args {
        let n = v.to_number().unwrap_or(f64::NAN);
        if n.is_nan() { return Ok(JsValue::Number(f64::NAN)); }
        if n < m { m = n; }
    }
    Ok(JsValue::Number(m))
}

fn math_random(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, _args: &[JsValue]) -> Result<JsValue, VmError> {
    // Simple LCG - deterministic but satisfies Math.random() type
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(1234567);
    let v = ((seed as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) >> 33) as f64 / u32::MAX as f64;
    Ok(JsValue::Number(v.fract().abs()))
}

fn math_hypot(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let mut sum = 0.0f64;
    for v in args {
        let n = v.to_number().unwrap_or(f64::NAN);
        if n.is_nan() { return Ok(JsValue::Number(f64::NAN)); }
        sum += n * n;
    }
    Ok(JsValue::Number(sum.sqrt()))
}

fn math_clz32(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let n = args.first().and_then(|v| v.to_number()).unwrap_or(0.0) as u32;
    Ok(JsValue::Number(n.leading_zeros() as f64))
}

fn math_fround(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let n = args.first().and_then(|v| v.to_number()).unwrap_or(f64::NAN);
    Ok(JsValue::Number((n as f32) as f64))
}

fn math_imul(_vm: &mut Vm, _ctx: &mut NativeContext, _this: JsValue, args: &[JsValue]) -> Result<JsValue, VmError> {
    let a = args.first().and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
    let b = args.get(1).and_then(|v| v.to_number()).unwrap_or(0.0) as i32;
    Ok(JsValue::Number(a.wrapping_mul(b) as f64))
}

fn assertion_error(arguments: &[JsValue], fallback: String) -> VmError {
    let message = arguments
        .get(2)
        .and_then(JsValue::to_js_string)
        .filter(|message| !message.is_empty())
        .unwrap_or(fallback);
    VmError {
        kind: VmErrorKind::Test262,
        message,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        builtins::{install_foundation, install_test262_harness},
        runtime::{JsValue, NativeContext},
    };

    #[test]
    fn installs_foundation_and_harness_as_registered_builtins() {
        let mut context = NativeContext::default();
        install_foundation(&mut context);
        install_test262_harness(&mut context);

        assert!(context.intrinsics().is_some());
        assert!(matches!(
            context.get_global("Object"),
            Some(JsValue::BuiltinFunction(_))
        ));
        assert!(matches!(
            context.get_global("Test262Error"),
            Some(JsValue::BuiltinFunction(_))
        ));
    }
}
