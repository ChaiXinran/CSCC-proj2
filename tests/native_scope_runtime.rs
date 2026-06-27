use agentjs::{
    bytecode::{Chunk, Instruction},
    runtime::{JsFunction, JsValue, NativeContext, to_property_key},
    vm::{CallFrame, VmErrorKind},
};

fn empty_function_chunk() -> Chunk {
    Chunk {
        instructions: vec![Instruction::ReturnUndefined],
        constants: Vec::new(),
        functions: Vec::new(),
        handlers: Vec::new(),
    }
}

#[test]
fn runtime_environments_resolve_and_restore_local_bindings() {
    let mut context = NativeContext::default();
    let global = context.current_environment();
    context
        .declare_binding(global, "x", JsValue::Number(1.0), true)
        .unwrap();

    let local = context.push_environment(Some(global)).unwrap();
    context
        .declare_binding(local, "x", JsValue::Number(2.0), true)
        .unwrap();

    assert_eq!(
        context.resolve_binding("x"),
        Some((local, JsValue::Number(2.0)))
    );

    context.pop_environment().unwrap();
    assert_eq!(
        context.resolve_binding("x"),
        Some((global, JsValue::Number(1.0)))
    );
}

#[test]
fn runtime_can_allocate_function_values_with_captured_environment() {
    let mut context = NativeContext::default();
    let captured = context.current_environment();
    let function = JsFunction {
        name: Some("add".into()),
        params: vec!["a".into(), "b".into()],
        rest_param: None,
        length_override: None,
        chunk: empty_function_chunk(),
        environment: Some(captured),
        is_generator: false,
    };

    let id = context.allocate_function(function).unwrap();
    let value = JsValue::Function(id);

    assert_eq!(value.type_of(), "function");
    let stored = context.function(id).unwrap();
    assert_eq!(stored.name.as_deref(), Some("add"));
    assert_eq!(stored.params, ["a", "b"]);
    assert_eq!(stored.environment, Some(captured));
}

#[test]
fn runtime_call_frames_track_this_and_call_depth_limit() {
    let mut context = NativeContext::default();
    context.reset_call_depth(1);
    let this_value = context
        .create_object([("value".into(), JsValue::Number(7.0))])
        .unwrap();
    let frame = CallFrame::new(
        None,
        0,
        context.current_environment(),
        this_value.clone(),
        0,
    );

    context.push_call_frame(frame).unwrap();
    assert_eq!(context.current_this(), this_value);

    let overflow = context
        .push_call_frame(CallFrame::new(
            None,
            0,
            context.current_environment(),
            JsValue::Undefined,
            0,
        ))
        .unwrap_err();
    assert_eq!(overflow.kind, VmErrorKind::RuntimeLimit);

    context.pop_call_frame().unwrap();
    assert_eq!(context.current_this(), JsValue::Undefined);
}

#[test]
fn runtime_objects_and_arrays_support_basic_property_operations() {
    let mut context = NativeContext::default();
    let object = context
        .create_object([("a".into(), JsValue::Number(1.0))])
        .unwrap();

    assert_eq!(
        context.get_property(object.clone(), "a").unwrap(),
        JsValue::Number(1.0)
    );
    assert_eq!(
        context
            .set_element(
                object.clone(),
                JsValue::String("a".into()),
                JsValue::Number(5.0),
            )
            .unwrap(),
        JsValue::Number(5.0)
    );
    assert_eq!(
        context.get_property(object, "a").unwrap(),
        JsValue::Number(5.0)
    );

    let array = context
        .create_array(vec![
            JsValue::Number(1.0),
            JsValue::Number(2.0),
            JsValue::Number(3.0),
        ])
        .unwrap();
    assert_eq!(
        context
            .get_element(array.clone(), JsValue::Number(0.0))
            .unwrap(),
        JsValue::Number(1.0)
    );
    assert_eq!(
        context.get_property(array.clone(), "length").unwrap(),
        JsValue::Number(3.0)
    );
    context
        .set_element(array.clone(), JsValue::Number(0.0), JsValue::Number(9.0))
        .unwrap();
    assert_eq!(
        context.get_element(array, JsValue::Number(0.0)).unwrap(),
        JsValue::Number(9.0)
    );
}

#[test]
fn runtime_property_key_conversion_matches_the_v3_minimum() {
    assert_eq!(to_property_key(&JsValue::String("x".into())).unwrap(), "x");
    assert_eq!(to_property_key(&JsValue::Number(7.0)).unwrap(), "7");
    assert_eq!(to_property_key(&JsValue::Boolean(true)).unwrap(), "true");
    assert_eq!(to_property_key(&JsValue::Null).unwrap(), "null");
    assert_eq!(to_property_key(&JsValue::Undefined).unwrap(), "undefined");

    let mut context = NativeContext::default();
    let object = context.create_object([]).unwrap();
    assert_eq!(
        to_property_key(&object).unwrap_err().kind,
        VmErrorKind::Type
    );
}
