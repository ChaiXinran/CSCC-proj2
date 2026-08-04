use std::path::Path;

use agentjs::{
    backend::NativeRuntime,
    bytecode::{Chunk, Instruction},
    engine::{ExecutionOptions, RuntimeConfig},
    runtime::{
        JsFunction, JsValue, ModuleEvaluationPromise, ModuleEvaluationState, NativeContext,
        PromiseThenReaction,
    },
    vm::Vm,
};

fn object_id(value: &JsValue) -> agentjs::runtime::ObjectId {
    let JsValue::Object(id) = value else {
        panic!("expected object value");
    };
    *id
}

fn assert_unique<T: std::fmt::Debug + PartialEq>(values: &[T]) {
    for (index, value) in values.iter().enumerate() {
        assert!(
            !values[..index].contains(value),
            "duplicate root at index {index}: {value:?}"
        );
    }
}

fn eval_with_gc_threshold(source: &str, gc_allocation_threshold: usize) -> (String, u64) {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        gc_allocation_threshold,
        ..RuntimeConfig::default()
    });
    let result = runtime
        .eval_source(source, ExecutionOptions::default())
        .unwrap_or_else(|error| {
            panic!("native eval failed at threshold {gc_allocation_threshold}: {error}")
        });
    (result, runtime.gc_metrics().collection_count)
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
fn property_tombstone_does_not_retain_deleted_descriptor_value() {
    let mut context = NativeContext::default();
    let child = context.create_object([]).unwrap();
    let child_id = object_id(&child);
    let container = context.create_object([("child".into(), child)]).unwrap();
    let container_id = object_id(&container);
    context.declare_global("container", container);

    context
        .heap_mut()
        .object_mut(container_id)
        .unwrap()
        .delete_own_property("child")
        .unwrap();
    context.collect_garbage_for_vm(&Vm::default()).unwrap();

    assert!(context.heap().object(container_id).is_some());
    assert!(context.heap().object(child_id).is_none());
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
        .declare_binding(outer, "captured", captured, true, false)
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
            function_body_start: 0,
            constant_index: None,
        }
        .into(),
        environment: Some(outer),
        is_async: false,
        is_generator: false,
        is_arrow: false,
        binds_name_in_activation: false,
        is_derived_constructor: false,
        is_constructable: true,
        has_own_prototype_property: true,
        prototype_writable: true,
        uses_arguments: false,
        lexical_this: None,
        lexical_new_target: None,
        home_object: None,
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
        .register_bound_function(
            target.clone(),
            JsValue::Undefined,
            vec![target],
            0.0,
            "bound target".into(),
        )
        .unwrap();
    context.declare_global("bound", bound);

    context.collect_garbage_for_vm(&Vm::default()).unwrap();

    assert!(context.heap().object(target_id).is_some());
}

#[test]
fn gc_reuses_object_slots_after_pruning_id_keyed_metadata() {
    let mut context = NativeContext::default();
    let stale = context
        .create_object([("stale".into(), JsValue::Boolean(true))])
        .unwrap();
    let stale_id = object_id(&stale);
    context.mark_raw_json_object(stale_id, "stale metadata".into());
    let slots_before = context.heap_stats().object_slots;

    context.collect_garbage_for_vm(&Vm::default()).unwrap();

    let replacement = context
        .create_object([("fresh".into(), JsValue::Boolean(true))])
        .unwrap();
    let replacement_id = object_id(&replacement);
    assert_eq!(replacement_id, stale_id);
    assert_eq!(context.heap_stats().object_slots, slots_before);
    assert!(context.raw_json_value(replacement_id).is_none());
    assert!(
        context
            .heap()
            .object(replacement_id)
            .unwrap()
            .own_property("stale")
            .is_none()
    );
}

#[test]
fn gc_metrics_track_collections_and_last_pass() {
    let mut context = NativeContext::default();
    context
        .create_object([("drop".into(), JsValue::Boolean(true))])
        .unwrap();

    let first = context.collect_garbage_for_vm(&Vm::default()).unwrap();
    context
        .create_object([("drop-again".into(), JsValue::Boolean(true))])
        .unwrap();
    let second = context.collect_garbage_for_vm(&Vm::default()).unwrap();
    let metrics = context.gc_metrics();

    assert_eq!(metrics.collection_count, 2);
    assert_eq!(
        metrics.collection_count,
        context.heap_stats().collection_count
    );
    assert_eq!(metrics.last_collection, second);
    assert!(metrics.total_pause_ns >= metrics.max_pause_ns);
    assert!(first.objects_after < first.objects_before);
    assert!(second.objects_after < second.objects_before);
}

#[test]
fn root_set_deduplicates_context_internal_roots() {
    let context = NativeContext::default();
    let roots = context.root_set(&Vm::default());

    assert_unique(&roots.environment_stack);
    assert_unique(&roots.object_roots);
    assert_unique(&roots.function_roots);
    assert_unique(&roots.value_roots);
}

#[test]
fn gc_threshold_does_not_change_closure_semantics() {
    let source = r#"
        class Holder {
            #captured;
            constructor(captured) { this.#captured = captured; }
            read() { return this.#captured.value; }
        }
        let saved;
        let holder;
        let proxy;
        let generator;
        (function () {
            let captured = { value: 42 };
            saved = function () { return captured.value; };
            holder = new Holder(captured);
            proxy = new Proxy(captured, {
                get: function (target, key) { return target[key]; }
            });
            generator = (function* () {
                let local = { value: 43 };
                yield local;
                return local.value;
            })();
        })();
        for (let index = 0; index < 30000; index++) {
            ({ index: index, payload: "temporary-" + index });
        }
        saved() + ":" + holder.read() + ":" + proxy.value + ":" +
            generator.next().value.value;
    "#;

    for threshold in [10_000, 100_000, 1_000_000, usize::MAX] {
        let (result, collection_count) = eval_with_gc_threshold(source, threshold);
        assert_eq!(result, "42:42:42:43");
        if threshold == 10_000 {
            assert!(collection_count >= 3);
        }
    }
}

#[test]
fn gc_preserves_private_slots_promise_reactions_and_jobs() {
    let mut context = NativeContext::default();

    let owner = context
        .create_object([("owner".into(), JsValue::Boolean(true))])
        .unwrap();
    let owner_id = object_id(&owner);
    context.declare_global("owner", owner);
    let private_value = context
        .create_object([("private".into(), JsValue::Number(1.0))])
        .unwrap();
    let private_value_id = object_id(&private_value);
    let brand = context.allocate_private_brand();
    context
        .define_private_slot(owner_id, "value".into(), brand, private_value)
        .unwrap();

    let reaction_value = context
        .create_object([("reaction".into(), JsValue::Number(2.0))])
        .unwrap();
    let reaction_value_id = object_id(&reaction_value);
    let promise = context.create_promise().unwrap();
    context
        .add_promise_reaction(
            promise,
            PromiseThenReaction {
                result_promise: None,
                resolve: reaction_value.clone(),
                reject: reaction_value,
                on_fulfilled: None,
                on_rejected: None,
                finally: false,
            },
        )
        .unwrap();

    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert!(context.heap().object(private_value_id).is_some());
    assert!(context.heap().object(reaction_value_id).is_some());

    context
        .fulfill_promise(promise, JsValue::Undefined)
        .unwrap();
    context.collect_garbage_for_vm(&Vm::default()).unwrap();
    assert!(context.heap().object(reaction_value_id).is_some());
}

#[test]
fn gc_preserves_module_environments_namespaces_and_evaluation_values() {
    let mut context = NativeContext::default();
    let environment = context
        .push_environment(Some(context.global_environment()))
        .unwrap();
    let exported = context
        .create_object([("module".into(), JsValue::Boolean(true))])
        .unwrap();
    let exported_id = object_id(&exported);
    context
        .declare_binding(environment, "exported", exported.clone(), true, false)
        .unwrap();
    context.pop_environment().unwrap();

    let module = context
        .module_registry_mut()
        .ensure_record(Path::new("gc-module.js"));
    context
        .module_registry_mut()
        .set_environment(module, environment);
    context
        .module_registry_mut()
        .set_namespace(module, exported.clone());
    context
        .module_registry_mut()
        .set_evaluation_state(module, ModuleEvaluationState::Rejected(exported.clone()));
    context.module_registry_mut().set_evaluation_promise(
        module,
        ModuleEvaluationPromise {
            promise: exported,
            promise_id: None,
        },
    );

    context.collect_garbage_for_vm(&Vm::default()).unwrap();

    assert!(context.heap().environment(environment).is_some());
    assert!(context.heap().object(exported_id).is_some());
}

#[test]
fn repeated_collections_keep_unreachable_growth_bounded() {
    let mut context = NativeContext::default();
    let baseline = context.heap_stats().live_objects;

    for round in 0..5 {
        for index in 0..1_000 {
            context
                .create_object([(
                    "temporary".into(),
                    JsValue::Number((round * 1_000 + index) as f64),
                )])
                .unwrap();
        }
        let stats = context.collect_garbage_for_vm(&Vm::default()).unwrap();
        assert_eq!(stats.objects_after, baseline);
    }

    assert_eq!(context.heap_stats().live_objects, baseline);
}
