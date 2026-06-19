//! Native ECMAScript built-in registration.

mod array;
mod function;
mod object;

pub use array::install_array;
pub use function::install_function;
pub use object::install_object;

use crate::{
    runtime::{Heap, JsObject, JsValue, NativeContext, NativeFunction, PropertyDescriptor},
    vm::{VmError, VmErrorKind},
};

/// Installs the foundational native constructors and prototypes.
pub fn install_foundation(heap: &mut Heap) {
    install_object(heap);
    install_function(heap);
    install_array(heap);
}

/// Installs the minimal Test262 host surface used by the native V1 runtime.
pub fn install_test262_harness(context: &mut NativeContext) {
    let mut assert = JsObject::default();
    assert.define_property(
        "sameValue",
        PropertyDescriptor::data(JsValue::NativeFunction(NativeFunction::AssertSameValue)),
    );
    assert.define_property(
        "notSameValue",
        PropertyDescriptor::data(JsValue::NativeFunction(NativeFunction::AssertNotSameValue)),
    );

    let assert_id = context
        .heap_mut()
        .allocate_object(assert)
        .expect("a fresh heap can allocate the assert object");
    context.declare_global("assert", JsValue::Object(assert_id));
    context.declare_global(
        "Test262Error",
        JsValue::NativeFunction(NativeFunction::Test262Error),
    );
}

pub fn call_native(function: NativeFunction, arguments: Vec<JsValue>) -> Result<JsValue, VmError> {
    match function {
        NativeFunction::AssertSameValue => assert_same_value(&arguments),
        NativeFunction::AssertNotSameValue => assert_not_same_value(&arguments),
        NativeFunction::Test262Error => Err(VmError::test262(
            arguments
                .first()
                .and_then(JsValue::to_js_string)
                .unwrap_or_else(|| "Test262Error".into()),
        )),
    }
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
        builtins::{call_native, install_test262_harness},
        runtime::{JsValue, NativeContext, NativeFunction},
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
            Some(JsValue::NativeFunction(NativeFunction::AssertSameValue))
        ));
        assert!(matches!(
            context.get_global("Test262Error"),
            Some(JsValue::NativeFunction(NativeFunction::Test262Error))
        ));
    }

    #[test]
    fn same_value_uses_test262_same_value_semantics() {
        assert_eq!(
            call_native(
                NativeFunction::AssertSameValue,
                vec![JsValue::Number(f64::NAN), JsValue::Number(f64::NAN)]
            )
            .unwrap(),
            JsValue::Undefined
        );

        let error = call_native(
            NativeFunction::AssertSameValue,
            vec![JsValue::Number(0.0), JsValue::Number(-0.0)],
        )
        .unwrap_err();
        assert_eq!(error.kind, VmErrorKind::Test262);
    }
}
