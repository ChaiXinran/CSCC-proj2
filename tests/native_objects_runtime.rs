use agentjs::{
    bytecode::{Chunk, Constant, EnvironmentCapturePolicy, FunctionTemplate, Instruction},
    runtime::{JsValue, NativeContext, PropertyDescriptor, PropertyDescriptorUpdate, PropertyKind},
    vm::{Vm, VmErrorKind},
};

fn string(chunk: &mut Chunk, value: &str) -> u16 {
    chunk.add_constant(Constant::String(value.into())).unwrap()
}

#[test]
fn descriptor_updates_preserve_non_configurable_invariants() {
    let mut context = NativeContext::default();
    let object = context.create_object([]).unwrap();
    let JsValue::Object(id) = object else {
        panic!("expected object");
    };

    context
        .define_own_property(
            id,
            "fixed".into(),
            PropertyDescriptor::data_with(JsValue::Number(1.0), false, true, false),
        )
        .unwrap();

    assert!(
        !context
            .validate_and_apply_property_descriptor(
                id,
                "fixed".into(),
                PropertyDescriptorUpdate {
                    value: Some(JsValue::Number(2.0)),
                    ..PropertyDescriptorUpdate::default()
                },
            )
            .unwrap()
    );
    assert_eq!(context.get(object, "fixed").unwrap(), JsValue::Number(1.0));
}

#[test]
fn prototype_chain_supports_get_has_set_and_cycle_rejection() {
    let mut context = NativeContext::default();
    let base = context
        .create_object([("x".into(), JsValue::Number(1.0))])
        .unwrap();
    let child = context.create_object([]).unwrap();
    let (JsValue::Object(base_id), JsValue::Object(child_id)) = (base.clone(), child.clone())
    else {
        panic!("expected objects");
    };

    context.set_prototype_of(child_id, Some(base_id)).unwrap();
    assert_eq!(
        context.get(child.clone(), "x").unwrap(),
        JsValue::Number(1.0)
    );
    assert!(context.has_property(child_id, "x").unwrap());

    context
        .set(child.clone(), "own", JsValue::Number(2.0), false)
        .unwrap();
    assert_eq!(context.get(child, "own").unwrap(), JsValue::Number(2.0));

    let cycle = context
        .set_prototype_of(base_id, Some(child_id))
        .unwrap_err();
    assert_eq!(cycle.kind, VmErrorKind::Type);
}

#[test]
fn delete_obeys_configurable_and_strict_rules() {
    let mut context = NativeContext::default();
    let object = context.create_object([]).unwrap();
    let JsValue::Object(id) = object else {
        panic!("expected object");
    };

    context
        .define_own_property(
            id,
            "locked".into(),
            PropertyDescriptor::data_with(JsValue::Number(1.0), true, true, false),
        )
        .unwrap();
    context
        .define_own_property(
            id,
            "open".into(),
            PropertyDescriptor::data_with(JsValue::Number(2.0), true, true, true),
        )
        .unwrap();

    assert!(!context.delete_property(id, "locked", false).unwrap());
    assert_eq!(
        context
            .delete_property(id, "locked", true)
            .unwrap_err()
            .kind,
        VmErrorKind::Type
    );
    assert!(context.delete_property(id, "open", false).unwrap());
    assert!(context.delete_property(id, "missing", false).unwrap());
}

#[test]
fn sparse_arrays_track_holes_length_and_range_errors() {
    let mut context = NativeContext::default();
    let array = context.create_sparse_array(3).unwrap();
    let JsValue::Object(id) = array.clone() else {
        panic!("expected array");
    };

    context
        .set_element(array.clone(), JsValue::Number(0.0), JsValue::Number(1.0))
        .unwrap();
    context
        .set_element(array.clone(), JsValue::Number(2.0), JsValue::Number(3.0))
        .unwrap();

    assert_eq!(
        context.get_property(array.clone(), "length").unwrap(),
        JsValue::Number(3.0)
    );
    assert!(!context.has_property(id, "1").unwrap());
    assert_eq!(
        context
            .set_element(array.clone(), JsValue::Number(5.0), JsValue::Number(6.0))
            .unwrap(),
        JsValue::Number(6.0)
    );
    assert_eq!(
        context.get_property(array.clone(), "length").unwrap(),
        JsValue::Number(6.0)
    );

    context
        .set_property(array.clone(), "length", JsValue::Number(2.0))
        .unwrap();
    assert_eq!(
        context
            .get_element(array.clone(), JsValue::Number(2.0))
            .unwrap(),
        JsValue::Undefined
    );
    assert_eq!(
        context
            .set_property(array, "length", JsValue::Number(-1.0))
            .unwrap_err()
            .kind,
        VmErrorKind::Range
    );
}

#[test]
fn array_length_shrink_recovers_when_non_configurable_element_blocks_delete() {
    let mut context = NativeContext::default();
    let array = context
        .create_array(vec![
            JsValue::Number(1.0),
            JsValue::Number(2.0),
            JsValue::Number(3.0),
        ])
        .unwrap();
    let JsValue::Object(id) = array.clone() else {
        panic!("expected array");
    };

    assert!(
        context
            .validate_and_apply_property_descriptor(
                id,
                "2".into(),
                PropertyDescriptorUpdate {
                    configurable: Some(false),
                    ..PropertyDescriptorUpdate::default()
                },
            )
            .unwrap()
    );
    assert!(
        !context
            .validate_and_apply_property_descriptor(
                id,
                "length".into(),
                PropertyDescriptorUpdate {
                    value: Some(JsValue::Number(1.0)),
                    ..PropertyDescriptorUpdate::default()
                },
            )
            .unwrap()
    );

    assert_eq!(
        context.get_property(array.clone(), "length").unwrap(),
        JsValue::Number(3.0)
    );
    assert!(context.has_property(id, "2").unwrap());
}

#[test]
fn vm_constructs_user_functions_and_runtime_instanceof_uses_prototype_chain() {
    let mut function_chunk = Chunk::default();
    let this_name = string(&mut function_chunk, "x");
    let three = function_chunk.add_constant(Constant::Number(3.0)).unwrap();
    function_chunk.emit(Instruction::LoadThis);
    function_chunk.emit(Instruction::Constant(three));
    function_chunk.emit(Instruction::SetProperty(this_name));
    function_chunk.emit(Instruction::Pop);
    function_chunk.emit(Instruction::ReturnUndefined);

    let mut chunk = Chunk::default();
    let template = FunctionTemplate {
        name: Some("Point".into()),
        params: Vec::new(),
        rest_param: None,
        chunk: function_chunk,
        is_strict: false,
        is_generator: false,
        environment_policy: EnvironmentCapturePolicy::CaptureCurrent,
    };
    let function = chunk.add_function(template).unwrap();
    chunk.emit(Instruction::CreateFunction(function));
    chunk.emit(Instruction::Construct(0));
    chunk.emit(Instruction::Return);

    let mut context = NativeContext::default();
    let result = Vm::default()
        .execute_with_context(&chunk, &mut context)
        .unwrap();
    assert_eq!(
        context.get_property(result.clone(), "x").unwrap(),
        JsValue::Number(3.0)
    );

    let constructor = context
        .heap()
        .function(agentjs::runtime::FunctionId(0))
        .map(|_| JsValue::Function(agentjs::runtime::FunctionId(0)))
        .unwrap();
    assert!(context.instance_of(result, constructor).unwrap());
}

#[test]
fn descriptors_can_represent_accessors_without_value_fields() {
    let descriptor = PropertyDescriptor::accessor(Some(JsValue::Undefined), None, true, true);

    assert!(descriptor.value().is_none());
    assert!(matches!(descriptor.kind, PropertyKind::Accessor { .. }));
}
