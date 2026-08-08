use agentjs::runtime::{GcPolicy, GcTriggerReason, JsValue, NativeContext};
use agentjs::vm::Vm;

fn test_policy() -> GcPolicy {
    GcPolicy {
        min_allocations: 1,
        min_pressure_bytes: 64,
        growth_factor_num: usize::MAX,
        growth_factor_den: 1,
        max_allocations: 100,
        min_reclaim_percent: 10,
    }
}

#[test]
fn runtime_memory_stats_include_side_registry_capacity() {
    let mut context = NativeContext::default();
    let before = context.runtime_memory_stats();
    context.create_array_buffer(4096).unwrap();
    let after = context.runtime_memory_stats();

    assert_eq!(after.array_buffer_records, before.array_buffer_records + 1);
    assert!(after.array_buffer_payload_bytes >= before.array_buffer_payload_bytes + 4096);
    assert!(after.tracked_runtime_bytes > before.tracked_runtime_bytes);
}

#[test]
fn charged_byte_pressure_triggers_collection() {
    let mut context = NativeContext::default();
    context.configure_gc_policy(test_policy());
    context
        .create_object([("allocation".into(), JsValue::Boolean(true))])
        .unwrap();
    context.ensure_heap_capacity(64).unwrap();

    assert!(context.should_collect_garbage());
    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert_eq!(
        context.gc_metrics().last_trigger_reason,
        GcTriggerReason::Bytes
    );
    assert_eq!(context.runtime_memory_stats().charged_bytes_since_gc, 0);
}

#[test]
fn allocation_hard_cap_remains_a_compatible_trigger() {
    let mut context = NativeContext::default();
    context.configure_gc_policy(GcPolicy {
        min_allocations: 3,
        min_pressure_bytes: usize::MAX,
        growth_factor_num: usize::MAX,
        growth_factor_den: 1,
        max_allocations: 3,
        min_reclaim_percent: 10,
    });
    for index in 0..3 {
        context
            .create_object([("index".into(), JsValue::Number(index as f64))])
            .unwrap();
    }

    assert!(context.should_collect_garbage());
    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert_eq!(
        context.gc_metrics().last_trigger_reason,
        GcTriggerReason::Allocation
    );
}

#[test]
fn tracked_growth_can_trigger_before_the_hard_cap() {
    let mut context = NativeContext::default();
    context.configure_gc_policy(GcPolicy {
        min_allocations: 1,
        min_pressure_bytes: usize::MAX,
        growth_factor_num: 1,
        growth_factor_den: 1,
        max_allocations: 100,
        min_reclaim_percent: 10,
    });
    context
        .create_object([("growth".into(), JsValue::Boolean(true))])
        .unwrap();

    assert!(context.should_collect_garbage());
    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert_eq!(
        context.gc_metrics().last_trigger_reason,
        GcTriggerReason::Growth
    );
}
