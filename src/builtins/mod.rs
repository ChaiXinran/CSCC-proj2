//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

pub use array::install_array;
pub use function::install_function;
pub use object::install_object;

use crate::{
    runtime::{
        Intrinsics, JsObject, JsValue, NativeContext, NativeErrorKind, NativeErrorValue,
        PropertyDescriptor,
    },
    vm::{VmError, VmErrorKind},
};

/// Installs the foundational native constructors, prototypes, and static methods.
pub fn install_foundation(context: &mut NativeContext) {
    install_globals(context).expect("builtin foundation installation must succeed");
    install_object(context);
    install_array(context);
    install_function(context);
}

/// Build Object / Function / Array prototypes and constructors, then set Intrinsics.
fn install_globals(context: &mut NativeContext) -> Result<(), VmError> {
    // 1. Object.prototype — no [[Prototype]]
    let object_prototype_id = context
        .heap_mut()
        .allocate_object(JsObject::ordinary())
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    // 2. Function.prototype — [[Prototype]] = Object.prototype
    let mut function_proto = JsObject::ordinary();
    function_proto.prototype = Some(object_prototype_id);
    let function_prototype_id = context
        .heap_mut()
        .allocate_object(function_proto)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    // 3. Object constructor
    let object_constructor = context.register_builtin(
        "Object",
        1,
        object::object_call,
        Some(object::object_construct),
    )?;
    let JsValue::BuiltinFunction(obj_bid) = object_constructor else {
        unreachable!()
    };
    let obj_backing = context.builtin(obj_bid).unwrap().object;
    context.set_prototype_of(obj_backing, Some(function_prototype_id))?;
    context.define_own_property(
        obj_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(object_prototype_id), false, false, false),
    )?;
    context.define_own_property(
        object_prototype_id,
        "constructor".into(),
        PropertyDescriptor::data_with(JsValue::BuiltinFunction(obj_bid), true, false, true),
    )?;

    // 4. Function constructor
    let function_constructor = context.register_builtin(
        "Function",
        1,
        function::function_call,
        Some(function::function_construct),
    )?;
    let JsValue::BuiltinFunction(fn_bid) = function_constructor else {
        unreachable!()
    };
    let fn_backing = context.builtin(fn_bid).unwrap().object;
    context.set_prototype_of(fn_backing, Some(function_prototype_id))?;
    context.define_own_property(
        fn_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(function_prototype_id), false, false, false),
    )?;
    context.define_own_property(
        function_prototype_id,
        "constructor".into(),
        PropertyDescriptor::data_with(JsValue::BuiltinFunction(fn_bid), true, false, true),
    )?;

    // 5. Array.prototype — [[Prototype]] = Object.prototype
    let mut array_proto = JsObject::ordinary();
    array_proto.prototype = Some(object_prototype_id);
    let array_prototype_id = context
        .heap_mut()
        .allocate_object(array_proto)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    // 6. Array constructor
    let array_constructor =
        context.register_builtin("Array", 1, array::array_call, Some(array::array_construct))?;
    let JsValue::BuiltinFunction(arr_bid) = array_constructor else {
        unreachable!()
    };
    let arr_backing = context.builtin(arr_bid).unwrap().object;
    context.set_prototype_of(arr_backing, Some(function_prototype_id))?;
    context.define_own_property(
        arr_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(array_prototype_id), false, false, false),
    )?;
    context.define_own_property(
        array_prototype_id,
        "constructor".into(),
        PropertyDescriptor::data_with(JsValue::BuiltinFunction(arr_bid), true, false, true),
    )?;

    // 7. Set Intrinsics
    context.set_intrinsics(Intrinsics {
        object_prototype: object_prototype_id,
        function_prototype: function_prototype_id,
        array_prototype: array_prototype_id,
        object_constructor: JsValue::BuiltinFunction(obj_bid),
        function_constructor: JsValue::BuiltinFunction(fn_bid),
        array_constructor: JsValue::BuiltinFunction(arr_bid),
    });

    // 8. Declare globals
    context.declare_global("Object", JsValue::BuiltinFunction(obj_bid));
    context.declare_global("Function", JsValue::BuiltinFunction(fn_bid));
    context.declare_global("Array", JsValue::BuiltinFunction(arr_bid));

    Ok(())
}

/// Installs the minimal Test262 host surface used by the native runtime.
pub fn install_test262_harness(context: &mut NativeContext) {
    let same_value = context
        .register_builtin("sameValue", 2, assert_same_value_call, None)
        .expect("install assert.sameValue");
    let not_same_value = context
        .register_builtin("notSameValue", 2, assert_not_same_value_call, None)
        .expect("install assert.notSameValue");

    let mut assert = JsObject::default();
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

// ---------------------------------------------------------------------------
// Test262 harness function implementations
// ---------------------------------------------------------------------------

fn assert_same_value_call(
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

fn assert_not_same_value_call(
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
        builtins::install_test262_harness,
        runtime::{JsValue, NativeContext},
        vm::VmErrorKind,
    };

    #[test]
    fn installs_minimal_assert_object() {
        let mut context = NativeContext::default();
        install_test262_harness(&mut context);

        let Some(JsValue::Object(assert_id)) = context.get_global("assert") else {
            panic!("expected assert object");
        };
        let assert = context.heap().object(assert_id).unwrap();
        assert!(matches!(
            assert.get_own_property_value("sameValue"),
            Some(JsValue::BuiltinFunction(_))
        ));
        assert!(matches!(
            context.get_global("Test262Error"),
            Some(JsValue::BuiltinFunction(_))
        ));
    }

    #[test]
    fn same_value_uses_test262_same_value_semantics() {
        let mut context = NativeContext::default();
        install_test262_harness(&mut context);

        let assert_id = match context.get_global("assert").unwrap() {
            JsValue::Object(id) => id,
            _ => panic!("expected assert object"),
        };
        let same_value_fn = context
            .heap()
            .object(assert_id)
            .unwrap()
            .get_own_property_value("sameValue")
            .unwrap();
        let JsValue::BuiltinFunction(bid) = same_value_fn else {
            panic!("expected builtin function");
        };
        let def = context.builtin(bid).unwrap().clone();

        assert_eq!(
            (def.call)(
                &mut context,
                JsValue::Undefined,
                &[JsValue::Number(f64::NAN), JsValue::Number(f64::NAN)]
            )
            .unwrap(),
            JsValue::Undefined
        );

        let error = (def.call)(
            &mut context,
            JsValue::Undefined,
            &[JsValue::Number(0.0), JsValue::Number(-0.0)],
        )
        .unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Test262);
    }

    #[test]
    fn constructs_minimal_test262_error_value() {
        let mut context = NativeContext::default();
        install_test262_harness(&mut context);

        let JsValue::BuiltinFunction(bid) = context.get_global("Test262Error").unwrap() else {
            panic!("expected builtin function");
        };
        let def = context.builtin(bid).unwrap().clone();
        let construct = def.construct.expect("Test262Error should be a constructor");

        let value = construct(
            &mut context,
            &[JsValue::String("expected".into())],
            JsValue::Undefined,
        )
        .unwrap();

        assert!(matches!(value, JsValue::Error(error) if error.message == "expected"));
    }

    #[test]
    fn install_foundation_sets_intrinsics_and_globals() {
        let mut context = NativeContext::default();
        crate::builtins::install_foundation(&mut context);

        assert!(context.intrinsics().is_some());
        assert!(matches!(
            context.get_global("Object"),
            Some(JsValue::BuiltinFunction(_))
        ));
        assert!(matches!(
            context.get_global("Array"),
            Some(JsValue::BuiltinFunction(_))
        ));
        assert!(matches!(
            context.get_global("Function"),
            Some(JsValue::BuiltinFunction(_))
        ));
    }

    #[test]
    fn builtin_typeof_is_function() {
        let mut context = NativeContext::default();
        crate::builtins::install_foundation(&mut context);
        let obj = context.get_global("Object").unwrap();
        assert_eq!(obj.type_of(), "function");
    }
}
