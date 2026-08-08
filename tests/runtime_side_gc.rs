use agentjs::runtime::{
    Job, JsObject, JsValue, NativeContext, ObjectKind, PromiseJob, PromiseReaction,
    TypedArrayElementKind,
};
use agentjs::vm::Vm;

#[test]
fn dead_side_records_are_reclaimed_and_ids_are_reused() {
    let mut context = NativeContext::default();
    let promise = context.create_promise().unwrap();
    let buffer = context.create_array_buffer(64 * 1024).unwrap();
    let typed = context
        .create_typed_array_view(buffer, TypedArrayElementKind::Uint8, 0, 16)
        .unwrap();
    let data = context.create_data_view(buffer, 0, 16).unwrap();

    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    let stats = context.runtime_side_memory_stats();
    assert_eq!(stats.promise_records, 0);
    assert_eq!(stats.array_buffer_records, 0);
    assert_eq!(stats.array_buffer_payload_bytes, 0);
    assert_eq!(stats.typed_array_views, 0);
    assert_eq!(stats.data_views, 0);

    assert_eq!(context.create_promise().unwrap(), promise);
    let replacement_buffer = context.create_array_buffer(8).unwrap();
    assert_eq!(replacement_buffer, buffer);
    assert_eq!(
        context
            .create_typed_array_view(replacement_buffer, TypedArrayElementKind::Uint8, 0, 8)
            .unwrap(),
        typed
    );
    assert_eq!(
        context.create_data_view(replacement_buffer, 0, 8).unwrap(),
        data
    );
}

#[test]
fn live_typed_array_keeps_view_and_backing_buffer_alive() {
    let mut context = NativeContext::default();
    let buffer = context.create_array_buffer(1024).unwrap();
    let view = context
        .create_typed_array_view(buffer, TypedArrayElementKind::Uint8, 0, 1024)
        .unwrap();
    let mut object = JsObject::ordinary();
    object.kind = ObjectKind::TypedArray {
        view,
        length: 1024,
        name: "Uint8Array".into(),
    };
    let object = context.heap_mut().allocate_object(object).unwrap();
    context.declare_global("view", JsValue::Object(object));

    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    let stats = context.runtime_side_memory_stats();
    assert_eq!(stats.typed_array_views, 1);
    assert_eq!(stats.array_buffer_records, 1);
    assert_eq!(
        context.typed_array_load_element(view, 0).unwrap(),
        JsValue::Number(0.0)
    );

    assert!(context.set_global("view", JsValue::Undefined));
    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert_eq!(context.runtime_side_memory_stats().array_buffer_records, 0);
}

#[test]
fn queued_promise_job_keeps_target_record_alive() {
    let mut context = NativeContext::default();
    let promise = context.create_promise().unwrap();
    context
        .enqueue_job(Job::PromiseReaction(PromiseJob {
            promise,
            reaction: PromiseReaction::Fulfill,
            value: JsValue::Number(7.0),
        }))
        .unwrap();

    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert_eq!(context.runtime_side_memory_stats().promise_records, 1);
    context.drain_jobs().unwrap();
    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert_eq!(context.runtime_side_memory_stats().promise_records, 0);
}
