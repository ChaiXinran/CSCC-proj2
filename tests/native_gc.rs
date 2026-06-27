use agentjs::{
    bytecode::{Chunk, Instruction},
    runtime::{JsFunction, JsValue, NativeContext},
    vm::Vm,
};

fn object_id(value: &JsValue) -> agentjs::runtime::ObjectId {
    let JsValue::Object(id) = value else {
        panic!("expected object value");
    };
    *id
}

#[test]
fn gc_collects_unreachable_objects_and_preserves_global_roots() {
    let mut context = NativeContext::default();
    let reachable = context
        .create_object([("name".into(), JsValue::String("keep".into()))])
        .unwrap();
    let reachable_id = object_id(&reachable);
    context.declare_global("keep", reachable);

    let unreachable = context
        .create_object([("name".into(), JsValue::String("drop".into()))])
        .unwrap();
    let unreachable_id = object_id(&unreachable);

    let before = context.heap_stats();
    assert!(before.live_objects >= 2);

    let stats = context.collect_garbage_for_vm(&Vm::default()).unwrap();

    assert!(stats.objects_after < stats.objects_before);
    assert!(context.heap().object(reachable_id).is_some());
    assert!(context.heap().object(unreachable_id).is_none());
}

#[test]
fn gc_preserves_closure_environment_and_captured_values() {
    let mut context = NativeContext::default();
    let outer = context
        .push_environment(Some(context.global_environment()))
        .unwrap();
    let captured = context
        .create_object([("answer".into(), JsValue::Number(42.0))])
        .unwrap();
    let captured_id = object_id(&captured);
    context
        .declare_binding(outer, "captured", captured, true)
        .unwrap();

    let function = JsFunction {
        name: Some("closure".into()),
        params: Vec::new(),
        rest_param: None,
        length_override: None,
        chunk: Chunk {
            instructions: vec![Instruction::ReturnUndefined],
            constants: Vec::new(),
            functions: Vec::new(),
            handlers: Vec::new(),
        },
        environment: Some(outer),
        is_generator: false,
    };
    let function_id = context.allocate_function(function).unwrap();
    context.declare_global("closure", JsValue::Function(function_id));
    context.pop_environment().unwrap();

    context.collect_garbage_for_vm(&Vm::default()).unwrap();

    assert!(context.heap().function(function_id).is_some());
    assert!(context.heap().environment(outer).is_some());
    assert!(context.heap().object(captured_id).is_some());
}

#[test]
fn gc_preserves_bound_function_targets_and_arguments() {
    let mut context = NativeContext::default();
    let target = context
        .create_object([("tag".into(), JsValue::String("target".into()))])
        .unwrap();
    let target_id = object_id(&target);

    let bound = context
        .register_bound_function(target.clone(), JsValue::Undefined, vec![target], 0)
        .unwrap();
    context.declare_global("bound", bound);

    context.collect_garbage_for_vm(&Vm::default()).unwrap();

    assert!(context.heap().object(target_id).is_some());
}
