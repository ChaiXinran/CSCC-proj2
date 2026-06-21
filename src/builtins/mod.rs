//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

// C1/C2 pure algorithm modules. They contain no VM/runtime wiring; the thin
// adapter layer in `v6` bridges them into the runtime. `allow(dead_code)` keeps
// low-level helpers mandated by the V6 interface (e.g. `utf16_slice`) available
// without forcing a direct JavaScript-method home for each one.
#[allow(dead_code)]
mod boolean;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod math;
#[allow(dead_code)]
mod number;
#[allow(dead_code)]
mod string;
mod v6;

use crate::{
    runtime::{
        Intrinsics, JsObject, JsValue, NativeContext, NativeErrorKind, NativeErrorValue,
        ObjectKind, PrimitiveValue, PropertyDescriptor,
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

    // V6: Pre-create primitive wrapper prototypes so builtins can install methods on them.
    // Per ECMAScript: Number.prototype, Boolean.prototype, and String.prototype are themselves
    // wrapper objects (with internal [[NumberData]]/[[BooleanData]]/[[StringData]] set to their
    // default values). Error.prototype is an ordinary object.
    let mut num_proto_obj = JsObject::ordinary();
    num_proto_obj.prototype = Some(object_prototype);
    num_proto_obj.kind = ObjectKind::PrimitiveWrapper(PrimitiveValue::Number(0.0));
    let number_prototype = context
        .heap_mut()
        .allocate_object(num_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut bool_proto_obj = JsObject::ordinary();
    bool_proto_obj.prototype = Some(object_prototype);
    bool_proto_obj.kind = ObjectKind::PrimitiveWrapper(PrimitiveValue::Boolean(false));
    let boolean_prototype = context
        .heap_mut()
        .allocate_object(bool_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut str_proto_obj = JsObject::ordinary();
    str_proto_obj.prototype = Some(object_prototype);
    str_proto_obj.kind = ObjectKind::PrimitiveWrapper(PrimitiveValue::String(String::new()));
    let string_prototype = context
        .heap_mut()
        .allocate_object(str_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut error_proto_obj = JsObject::ordinary();
    error_proto_obj.prototype = Some(object_prototype);
    let error_prototype = context
        .heap_mut()
        .allocate_object(error_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    context.set_intrinsics(Intrinsics {
        object_prototype,
        function_prototype,
        array_prototype,
        object_constructor: object_constructor.clone(),
        function_constructor: function_constructor.clone(),
        array_constructor: array_constructor.clone(),
        string_prototype,
        number_prototype,
        boolean_prototype,
        error_prototype,
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
        return Err(VmError::type_error(
            "assert.throws: second argument must be callable",
        ));
    }
    if vm.call_value_threw(func, JsValue::Undefined, vec![], context) {
        Ok(JsValue::Undefined)
    } else {
        Err(VmError::test262(
            "assert.throws: no exception was thrown".to_string(),
        ))
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

// ── Standard globals (Error, Number, Boolean, String, Math) ──────────────────

/// Installs the standard-library globals by delegating to the V6 adapter layer,
/// which bridges the pure C1/C2 algorithm modules into the runtime.
fn install_std_globals(context: &mut NativeContext) -> Result<(), VmError> {
    v6::install(context)
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
