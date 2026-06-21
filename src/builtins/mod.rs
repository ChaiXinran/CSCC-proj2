//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

use crate::{
    runtime::{
        Intrinsics, JsObject, JsValue, NativeContext, NativeErrorKind, NativeErrorValue,
        NativeFunction, ObjectId, PropertyDescriptor,
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

    context.declare_global("assert", JsValue::Object(assert_id));
    context.declare_global(
        "Test262Error",
        JsValue::NativeFunction(NativeFunction::Test262Error),
    );
}

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

fn assert_not_same_value(arguments: &[JsValue]) -> Result<JsValue, VmError> {
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
            Some(JsValue::NativeFunction(NativeFunction::AssertSameValue))
        ));
        assert!(matches!(
            context.get_global("Test262Error"),
            Some(JsValue::NativeFunction(NativeFunction::Test262Error))
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
            )
            .unwrap(),
            JsValue::Undefined
        );

        let error = call_native(
            NativeFunction::AssertSameValue,
            &mut context,
            JsValue::Undefined,
            vec![JsValue::Number(0.0), JsValue::Number(-0.0)],
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
        )
        .unwrap();

        assert!(matches!(value, JsValue::Error(error) if error.message == "expected"));
    }
}
