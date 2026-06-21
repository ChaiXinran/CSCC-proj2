//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

use crate::{
    runtime::{
        Intrinsics, JsObject, JsValue, NativeContext, NativeErrorKind, NativeErrorValue,
        NativeFunction, ObjectId, PropertyDescriptor,
        PropertyDescriptor,
    },
    vm::{VmError, VmErrorKind},
};

/// Installs the foundational native constructors and prototypes.
pub fn install_foundation(context: &mut NativeContext) {
    if context.intrinsics().is_some() {
        return;
    }

    let object_prototype = allocate_object(context, JsObject::ordinary());
    let function_prototype = register_native_function(
        context,
        NativeFunction::FunctionPrototype,
        Some(object_prototype),
    );
    let function_prototype_id = context
        .native_function_object(NativeFunction::FunctionPrototype)
        .expect("Function.prototype object should be registered");

    let object_constructor = register_native_function(
        context,
        NativeFunction::ObjectConstructor,
        Some(function_prototype_id),
    );
    let function_constructor = register_native_function(
        context,
        NativeFunction::FunctionConstructor,
        Some(function_prototype_id),
    );
    let array_constructor = register_native_function(
        context,
        NativeFunction::ArrayConstructor,
        Some(function_prototype_id),
    );

    let mut array_prototype = JsObject::sparse_array(0);
    array_prototype.prototype = Some(object_prototype);
    let array_prototype_id = allocate_object(context, array_prototype);

    context.set_intrinsics(Intrinsics {
        object_prototype,
        function_prototype: function_prototype_id,
        array_prototype: array_prototype_id,
        object_constructor: object_constructor.clone(),
        function_constructor: function_constructor.clone(),
        array_constructor: array_constructor.clone(),
    });

    define_data(
        context,
        object_prototype,
        "constructor",
        object_constructor.clone(),
        true,
        false,
        true,
    );
    define_data(
        context,
        function_prototype_id,
        "constructor",
        function_constructor.clone(),
        true,
        false,
        true,
    );
    define_data(
        context,
        array_prototype_id,
        "constructor",
        array_constructor.clone(),
        true,
        false,
        true,
    );

    let object_constructor_id = context
        .native_function_object(NativeFunction::ObjectConstructor)
        .expect("Object constructor object should be registered");
    let function_constructor_id = context
        .native_function_object(NativeFunction::FunctionConstructor)
        .expect("Function constructor object should be registered");
    let array_constructor_id = context
        .native_function_object(NativeFunction::ArrayConstructor)
        .expect("Array constructor object should be registered");

    define_data(
        context,
        object_constructor_id,
        "prototype",
        JsValue::Object(object_prototype),
        false,
        false,
        false,
    );
    define_data(
        context,
        function_constructor_id,
        "prototype",
        function_prototype,
        false,
        false,
        false,
    );
    define_data(
        context,
        array_constructor_id,
        "prototype",
        JsValue::Object(array_prototype_id),
        false,
        false,
        false,
    );

    object::install_object(context, object_constructor_id);
    function::install_function(context, function_prototype_id);
    array::install_array(context, array_constructor_id, array_prototype_id);

    context.declare_global("Object", object_constructor);
    context.declare_global("Function", function_constructor);
    context.declare_global("Array", array_constructor);
}

/// Installs the minimal Test262 host surface used by the native runtime.
pub fn install_test262_harness(context: &mut NativeContext) {
    let assert = context
        .create_object([])
        .expect("a fresh heap can allocate the assert object");
    let JsValue::Object(assert_id) = assert else {
        unreachable!("create_object returns an object");
    };
    define_data(
        context,
        assert_id,
        "sameValue",
        JsValue::NativeFunction(NativeFunction::AssertSameValue),
        true,
        true,
        true,
    );
    define_data(
        context,
        assert_id,
        "notSameValue",
        JsValue::NativeFunction(NativeFunction::AssertNotSameValue),
        true,
        true,
        true,
    );
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

    context.declare_global("assert", JsValue::Object(assert_id));

pub fn call_native(
    function: NativeFunction,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    match function {
        NativeFunction::AssertSameValue => assert_same_value(&arguments),
        NativeFunction::AssertNotSameValue => assert_not_same_value(&arguments),
        NativeFunction::Test262Error => Err(VmError::test262(
            arguments
                .first()
                .and_then(JsValue::to_js_string)
                .unwrap_or_else(|| "Test262Error".into()),
        )),
        NativeFunction::ObjectConstructor
        | NativeFunction::ObjectCreate
        | NativeFunction::ObjectDefineProperty
        | NativeFunction::ObjectGetOwnPropertyDescriptor
        | NativeFunction::ObjectGetPrototypeOf
        | NativeFunction::ObjectSetPrototypeOf
        | NativeFunction::ObjectKeys => object::call(function, context, arguments),
        NativeFunction::ArrayConstructor
        | NativeFunction::ArrayIsArray
        | NativeFunction::ArrayPrototypePush
        | NativeFunction::ArrayPrototypePop => {
            array::call(function, context, this_value, arguments)
        }
        NativeFunction::FunctionConstructor | NativeFunction::FunctionPrototype => {
            function::call(function, context, arguments)
        }
        NativeFunction::FunctionPrototypeCall => Err(VmError::runtime(
            "Function.prototype.call requires the VM call path",
        )),
    }
}

pub fn construct_native(
    function: NativeFunction,
    context: &mut NativeContext,
    arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    match function {
        NativeFunction::Test262Error => {
            let message = arguments
                .first()
                .and_then(JsValue::to_js_string)
                .unwrap_or_else(|| "Test262Error".into());
            Ok(JsValue::Error(NativeErrorValue::new(
                NativeErrorKind::Test262,
                message,
            )))
        }
        NativeFunction::ObjectConstructor => object::construct(context, arguments),
        NativeFunction::ArrayConstructor => array::construct(context, arguments),
        NativeFunction::FunctionConstructor => function::construct(context, arguments),
        _ => Err(VmError::type_error(
            "native function is not a constructor in V4",
        )),
    }
}

pub(crate) fn register_native_function(
    context: &mut NativeContext,
    function: NativeFunction,
    prototype: Option<ObjectId>,
) -> JsValue {
    let mut object = JsObject::ordinary();
    object.prototype = prototype;
    object.define_property(
        "name",
        PropertyDescriptor::data_with(JsValue::String(function.name().into()), false, false, true),
    );
    object.define_property(
        "length",
        PropertyDescriptor::data_with(
            JsValue::Number(function.length() as f64),
            false,
            false,
            true,
        ),
    );
    let object_id = allocate_object(context, object);
    context.register_native_function_object(function, object_id);
    JsValue::NativeFunction(function)
}

pub(crate) fn define_data(
    context: &mut NativeContext,
    object: ObjectId,
    key: &str,
    value: JsValue,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) {
    context
        .define_own_property(
            object,
            key.into(),
            PropertyDescriptor::data_with(value, writable, enumerable, configurable),
        )
        .expect("foundation builtins define properties on existing objects");
}

fn allocate_object(context: &mut NativeContext, object: JsObject) -> ObjectId {
    context
        .heap_mut()
        .allocate_object(object)
        .expect("a fresh heap can allocate builtin objects")
}

fn assert_same_value(arguments: &[JsValue]) -> Result<JsValue, VmError> {
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
        builtins::{call_native, construct_native, install_foundation, install_test262_harness},
        runtime::{JsValue, NativeContext, NativeFunction},
        builtins::install_test262_harness,
        runtime::{JsValue, NativeContext},
        vm::VmErrorKind,
    };

    #[test]
    fn installs_foundation_globals_and_prototype_relationships() {
        let mut context = NativeContext::default();
        install_foundation(&mut context);

        assert!(matches!(
            context.get_global("Object"),
            Some(JsValue::NativeFunction(NativeFunction::ObjectConstructor))
        ));
        assert!(matches!(
            context.get_global("Array"),
            Some(JsValue::NativeFunction(NativeFunction::ArrayConstructor))
        ));
        assert!(matches!(
            context.get_global("Function"),
            Some(JsValue::NativeFunction(NativeFunction::FunctionConstructor))
        ));

        let array = context.get_global("Array").unwrap();
        let array_object = context.value_object(&array).unwrap();
        let function_prototype = context.get_prototype_of(array_object).unwrap();
        assert_eq!(
            context.object_value(function_prototype),
            JsValue::NativeFunction(NativeFunction::FunctionPrototype)
        );
    }

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
        assert_eq!(
            call_native(
                NativeFunction::AssertSameValue,
                &mut context,
                JsValue::Undefined,
                vec![JsValue::Number(f64::NAN), JsValue::Number(f64::NAN)]
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

        let error = call_native(
            NativeFunction::AssertSameValue,
            &mut context,
            JsValue::Undefined,
            vec![JsValue::Number(0.0), JsValue::Number(-0.0)],
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
        let value = construct_native(
            NativeFunction::Test262Error,
            &mut context,
            vec![JsValue::String("expected".into())],
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
