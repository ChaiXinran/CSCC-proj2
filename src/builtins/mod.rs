//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

use crate::{
    runtime::{
        Intrinsics, JsObject, JsValue, NativeContext, NativeErrorKind, NativeErrorValue,
        PropertyDescriptor,
    },
    vm::{VmError, VmErrorKind},
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
    let same_value = context
        .register_builtin("sameValue", 2, assert_same_value, None)
        .expect("install assert.sameValue");
    let not_same_value = context
        .register_builtin("notSameValue", 2, assert_not_same_value, None)
        .expect("install assert.notSameValue");

    let mut assert = JsObject::ordinary();
    assert.define_property("sameValue", PropertyDescriptor::data(same_value));
    assert.define_property("notSameValue", PropertyDescriptor::data(not_same_value));
    let assert_id = context
        .heap_mut()
        .allocate_object(assert)
        .expect("a fresh heap can allocate the assert object");
    context.declare_global("assert", JsValue::Object(assert_id));

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

fn assert_same_value(
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
