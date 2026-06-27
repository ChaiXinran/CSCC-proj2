//! Minimal JS-visible Promise builtins backed by the shared native job queue.

use crate::{
    runtime::{
        Job, JsObject, JsValue, NativeCall, NativeContext, ObjectId, PromiseId, PromiseJob,
        PromiseReaction, PromiseThenReaction, PropertyDescriptor,
    },
    vm::{Vm, VmError},
};

const PROMISE_RESOLVE_FUNCTION: &str = "__AgentJSPromiseResolveFunction";
const PROMISE_REJECT_FUNCTION: &str = "__AgentJSPromiseRejectFunction";
const PROMISE_CAPABILITY_EXECUTOR: &str = "__AgentJSPromiseCapabilityExecutor";
const PROMISE_ALL_FULFILL: &str = "__AgentJSPromiseAllFulfill";
const PROMISE_ALL_SETTLED_FULFILL: &str = "__AgentJSPromiseAllSettledFulfill";
const PROMISE_ALL_SETTLED_REJECT: &str = "__AgentJSPromiseAllSettledReject";
const PROMISE_ANY_REJECT: &str = "__AgentJSPromiseAnyReject";
const PROMISE_AGGREGATE_FULFILL: &str = "__AgentJSPromiseAggregateFulfill";
const PROMISE_AGGREGATE_REJECT: &str = "__AgentJSPromiseAggregateReject";

const AGGREGATE_TARGET: &str = "__agentjs_promise_aggregate_target__";
const AGGREGATE_VALUES: &str = "__agentjs_promise_aggregate_values__";
const AGGREGATE_REMAINING: &str = "__agentjs_promise_aggregate_remaining__";
const CAPABILITY_RESOLVE: &str = "__agentjs_promise_capability_resolve__";
const CAPABILITY_REJECT: &str = "__agentjs_promise_capability_reject__";

pub(super) fn install(context: &mut NativeContext) -> Result<(), VmError> {
    let constructor =
        context.register_builtin("Promise", 1, promise_call, Some(promise_construct))?;
    let JsValue::BuiltinFunction(constructor_id) = constructor else {
        unreachable!()
    };
    let constructor_object = context.builtin(constructor_id).unwrap().object;

    let mut prototype = JsObject::ordinary();
    prototype.prototype = context.object_prototype();
    let prototype = context
        .heap_mut()
        .allocate_object(prototype)
        .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;

    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    define_method(context, constructor_object, "resolve", 1, promise_resolve)?;
    define_method(context, constructor_object, "reject", 1, promise_reject)?;
    define_method(context, constructor_object, "all", 1, promise_all)?;
    define_method(
        context,
        constructor_object,
        "allSettled",
        1,
        promise_all_settled,
    )?;
    define_method(context, constructor_object, "any", 1, promise_any)?;
    define_method(context, constructor_object, "race", 1, promise_race)?;
    define_method(context, constructor_object, "try", 1, promise_try)?;
    define_method(
        context,
        constructor_object,
        "withResolvers",
        0,
        promise_with_resolvers,
    )?;
    let species_getter =
        context.register_builtin("get [Symbol.species]", 0, promise_species_get, None)?;
    context.define_symbol_own_property(
        constructor_object,
        context.well_known_symbols().species,
        PropertyDescriptor::accessor(Some(species_getter), None, false, true),
    )?;

    define_method(context, prototype, "then", 2, promise_then)?;
    define_method(context, prototype, "catch", 1, promise_catch)?;
    define_method(context, prototype, "finally", 1, promise_finally)?;
    context.define_symbol_own_property(
        prototype,
        context.well_known_symbols().to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Promise".into())),
    )?;

    context.register_builtin(PROMISE_RESOLVE_FUNCTION, 2, promise_resolve_executor, None)?;
    context.register_builtin(PROMISE_REJECT_FUNCTION, 2, promise_reject_executor, None)?;
    context.register_builtin(
        PROMISE_CAPABILITY_EXECUTOR,
        2,
        promise_capability_executor,
        None,
    )?;
    context.register_builtin(PROMISE_ALL_FULFILL, 1, promise_all_fulfill, None)?;
    context.register_builtin(
        PROMISE_ALL_SETTLED_FULFILL,
        1,
        promise_all_settled_fulfill,
        None,
    )?;
    context.register_builtin(
        PROMISE_ALL_SETTLED_REJECT,
        1,
        promise_all_settled_reject,
        None,
    )?;
    context.register_builtin(PROMISE_ANY_REJECT, 1, promise_any_reject, None)?;
    context.register_builtin(
        PROMISE_AGGREGATE_FULFILL,
        1,
        promise_aggregate_fulfill,
        None,
    )?;
    context.register_builtin(PROMISE_AGGREGATE_REJECT, 1, promise_aggregate_reject, None)?;

    context.declare_global("Promise", constructor);
    Ok(())
}

fn method_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, true)
}

fn constant_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, false)
}

fn readonly_configurable_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, true)
}

fn define_method(
    context: &mut NativeContext,
    object: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<(), VmError> {
    let function = context.register_builtin(name, length, call, None)?;
    context.define_own_property(object, name.into(), method_descriptor(function))?;
    Ok(())
}

fn promise_prototype(context: &NativeContext) -> Option<ObjectId> {
    let JsValue::BuiltinFunction(id) = context.get_global("Promise")? else {
        return None;
    };
    let constructor = context.builtin(id)?.object;
    let descriptor = context.get_own_property_descriptor(constructor, "prototype")?;
    let crate::runtime::PropertyKind::Data { value, .. } = descriptor.kind else {
        return None;
    };
    match value {
        JsValue::Object(object) => Some(object),
        _ => None,
    }
}

fn create_promise_object(
    context: &mut NativeContext,
) -> Result<(JsValue, crate::runtime::PromiseId), VmError> {
    let promise = context.create_promise()?;
    let object = context.create_promise_object(promise, promise_prototype(context))?;
    Ok((object, promise))
}

struct PromiseCapability {
    promise: JsValue,
    promise_id: Option<PromiseId>,
    resolve: JsValue,
    reject: JsValue,
}

fn new_promise_capability(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
) -> Result<PromiseCapability, VmError> {
    if !context.is_constructable_value(&constructor) {
        return Err(VmError::type_error("Promise receiver is not a constructor"));
    }
    let state = context.create_object([
        (CAPABILITY_RESOLVE.into(), JsValue::Undefined),
        (CAPABILITY_REJECT.into(), JsValue::Undefined),
    ])?;
    let promise = vm.with_root_from_builtin(state.clone(), |vm| {
        let target = context
            .find_builtin_by_name(PROMISE_CAPABILITY_EXECUTOR)
            .ok_or_else(|| VmError::runtime("missing Promise capability executor"))?;
        let executor = context.register_bound_function(
            target,
            JsValue::Undefined,
            vec![state.clone()],
            2.0,
            String::new(),
        )?;
        vm.construct_value_from_builtin(constructor, vec![executor], context)
    })?;
    let resolve = context.get(state.clone(), CAPABILITY_RESOLVE)?;
    let reject = context.get(state, CAPABILITY_REJECT)?;
    if !is_callable(&resolve) || !is_callable(&reject) {
        return Err(VmError::type_error(
            "Promise constructor did not provide resolving functions",
        ));
    }
    Ok(PromiseCapability {
        promise_id: context.promise_id_from_value(&promise),
        promise,
        resolve,
        reject,
    })
}

fn create_promise_resolving_functions(
    context: &mut NativeContext,
    promise_object: JsValue,
) -> Result<(JsValue, JsValue), VmError> {
    let resolve_target = context
        .find_builtin_by_name(PROMISE_RESOLVE_FUNCTION)
        .ok_or_else(|| VmError::runtime("missing Promise resolve function"))?;
    let reject_target = context
        .find_builtin_by_name(PROMISE_REJECT_FUNCTION)
        .ok_or_else(|| VmError::runtime("missing Promise reject function"))?;
    let resolve = context.register_bound_function(
        resolve_target,
        JsValue::Undefined,
        vec![promise_object.clone()],
        1.0,
        String::new(),
    )?;
    let reject = context.register_bound_function(
        reject_target,
        JsValue::Undefined,
        vec![promise_object.clone()],
        1.0,
        String::new(),
    )?;
    Ok((resolve, reject))
}

fn enqueue_settle(
    context: &mut NativeContext,
    promise: crate::runtime::PromiseId,
    reaction: PromiseReaction,
    value: JsValue,
) -> Result<(), VmError> {
    context.enqueue_job(Job::PromiseReaction(PromiseJob {
        promise,
        reaction,
        value,
    }))
}

fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn promise_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "Promise constructor cannot be invoked without new",
    ))
}

fn promise_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let executor = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&executor) {
        return Err(VmError::type_error("Promise resolver is not a function"));
    }
    let promise = context.create_promise()?;
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| promise_prototype(context));
    let promise_object = context.create_promise_object(promise, prototype)?;
    let (resolve, reject) = create_promise_resolving_functions(context, promise_object.clone())?;

    match vm.call_value_catching_from_builtin(
        executor,
        JsValue::Undefined,
        vec![resolve, reject],
        context,
    )? {
        Ok(_) => {}
        Err(value) => enqueue_settle(context, promise, PromiseReaction::Reject, value)?,
    }
    Ok(promise_object)
}

fn promise_resolve(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.promise_id_from_value(&value).is_some() {
        let value_constructor =
            vm.get_property_value_catching_from_builtin(value.clone(), "constructor", context)?;
        if value_constructor.is_ok_and(|constructor| constructor.same_value(&this)) {
            return Ok(value);
        }
    }
    let capability = new_promise_capability(vm, context, this)?;
    vm.call_value_from_builtin(capability.resolve, JsValue::Undefined, vec![value], context)?;
    Ok(capability.promise)
}

fn promise_reject(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let reason = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let capability = new_promise_capability(vm, context, this)?;
    vm.call_value_from_builtin(capability.reject, JsValue::Undefined, vec![reason], context)?;
    Ok(capability.promise)
}

fn promise_all(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    promise_combinator(vm, context, this, arguments, PromiseCombinator::All)
}

fn promise_all_settled(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    promise_combinator(vm, context, this, arguments, PromiseCombinator::AllSettled)
}

fn promise_any(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    promise_combinator(vm, context, this, arguments, PromiseCombinator::Any)
}

fn promise_race(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    promise_combinator(vm, context, this, arguments, PromiseCombinator::Race)
}

#[derive(Clone, Copy)]
enum PromiseCombinator {
    All,
    AllSettled,
    Any,
    Race,
}

fn promise_combinator(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    arguments: &[JsValue],
    combinator: PromiseCombinator,
) -> Result<JsValue, VmError> {
    let capability = new_promise_capability(vm, context, constructor.clone())?;
    let promise_object = capability.promise;
    let promise = capability.promise_id.ok_or_else(|| {
        VmError::type_error("Promise constructor did not create a native Promise")
    })?;
    let iterable = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    vm.with_root_from_builtin(promise_object.clone(), |vm| {
        initialize_promise_combinator(
            vm,
            context,
            constructor,
            iterable,
            combinator,
            promise_object.clone(),
            promise,
        )
    })?;
    Ok(promise_object)
}

fn initialize_promise_combinator(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    iterable: JsValue,
    combinator: PromiseCombinator,
    promise_object: JsValue,
    promise: PromiseId,
) -> Result<(), VmError> {
    let resolve = match vm.get_property_value_catching_from_builtin(
        constructor.clone(),
        "resolve",
        context,
    )? {
        Ok(resolve) if is_callable(&resolve) => resolve,
        Ok(_) => {
            enqueue_settle(
                context,
                promise,
                PromiseReaction::Reject,
                type_error_value("Promise resolve is not callable"),
            )?;
            return Ok(());
        }
        Err(reason) => {
            enqueue_settle(context, promise, PromiseReaction::Reject, reason)?;
            return Ok(());
        }
    };
    let values = match vm.collect_iterable_values_from_builtin(iterable, context) {
        Ok(values) => values,
        Err(error) => {
            let Some(reason) = vm.take_pending_exception_from_builtin() else {
                return Err(error);
            };
            enqueue_settle(context, promise, PromiseReaction::Reject, reason)?;
            return Ok(());
        }
    };

    if values.is_empty() {
        match combinator {
            PromiseCombinator::All | PromiseCombinator::AllSettled => {
                let array = context.create_array(Vec::new())?;
                enqueue_settle(context, promise, PromiseReaction::Fulfill, array)?;
            }
            PromiseCombinator::Any => {
                enqueue_settle(context, promise, PromiseReaction::Reject, aggregate_error())?
            }
            PromiseCombinator::Race => {}
        }
        return Ok(());
    }

    let result_values = context.create_array(vec![JsValue::Undefined; values.len()])?;
    let state = context.create_object([
        (AGGREGATE_TARGET.into(), promise_object.clone()),
        (AGGREGATE_VALUES.into(), result_values),
        (
            AGGREGATE_REMAINING.into(),
            JsValue::Number(values.len() as f64),
        ),
    ])?;

    vm.with_root_from_builtin(state.clone(), |vm| {
        for (index, value) in values.into_iter().enumerate() {
            let (on_fulfilled, on_rejected) =
                aggregate_callbacks(context, combinator, &state, index)?;
            let resolved = match vm.call_value_catching_from_builtin(
                resolve.clone(),
                constructor.clone(),
                vec![value],
                context,
            )? {
                Ok(resolved) => resolved,
                Err(reason) => {
                    enqueue_settle(context, promise, PromiseReaction::Reject, reason)?;
                    return Ok(());
                }
            };
            let registration = vm.with_root_from_builtin(resolved.clone(), |vm| {
                let then = match vm.get_property_value_catching_from_builtin(
                    resolved.clone(),
                    "then",
                    context,
                )? {
                    Ok(then) if is_callable(&then) => then,
                    Ok(_) => {
                        return Ok(Err(type_error_value("resolved value then is not callable")));
                    }
                    Err(reason) => return Ok(Err(reason)),
                };
                vm.call_value_catching_from_builtin(
                    then,
                    resolved,
                    vec![on_fulfilled, on_rejected],
                    context,
                )
            })?;
            if let Err(reason) = registration {
                enqueue_settle(context, promise, PromiseReaction::Reject, reason)?;
                return Ok(());
            }
        }
        Ok(())
    })
}

fn aggregate_callbacks(
    context: &mut NativeContext,
    combinator: PromiseCombinator,
    state: &JsValue,
    index: usize,
) -> Result<(JsValue, JsValue), VmError> {
    let indexed_args = vec![state.clone(), JsValue::Number(index as f64)];
    let state_arg = vec![state.clone()];
    match combinator {
        PromiseCombinator::All => Ok((
            bind_internal(context, PROMISE_ALL_FULFILL, indexed_args)?,
            bind_internal(context, PROMISE_AGGREGATE_REJECT, state_arg)?,
        )),
        PromiseCombinator::AllSettled => Ok((
            bind_internal(context, PROMISE_ALL_SETTLED_FULFILL, indexed_args.clone())?,
            bind_internal(context, PROMISE_ALL_SETTLED_REJECT, indexed_args)?,
        )),
        PromiseCombinator::Any => Ok((
            bind_internal(context, PROMISE_AGGREGATE_FULFILL, state_arg)?,
            bind_internal(context, PROMISE_ANY_REJECT, indexed_args)?,
        )),
        PromiseCombinator::Race => Ok((
            bind_internal(context, PROMISE_AGGREGATE_FULFILL, state_arg.clone())?,
            bind_internal(context, PROMISE_AGGREGATE_REJECT, state_arg)?,
        )),
    }
}

fn bind_internal(
    context: &mut NativeContext,
    name: &str,
    arguments: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    let target = context
        .find_builtin_by_name(name)
        .ok_or_else(|| VmError::runtime(format!("missing internal builtin {name}")))?;
    context.register_bound_function(
        target,
        JsValue::Undefined,
        arguments,
        1.0,
        format!("bound {name}"),
    )
}

fn aggregate_state(arguments: &[JsValue]) -> Result<JsValue, VmError> {
    arguments
        .first()
        .cloned()
        .filter(|value| matches!(value, JsValue::Object(_)))
        .ok_or_else(|| VmError::runtime("invalid Promise aggregate state"))
}

fn aggregate_target(context: &mut NativeContext, state: JsValue) -> Result<PromiseId, VmError> {
    let target = context.get(state, AGGREGATE_TARGET)?;
    context
        .promise_id_from_value(&target)
        .ok_or_else(|| VmError::runtime("invalid Promise aggregate target"))
}

fn aggregate_values(context: &mut NativeContext, state: JsValue) -> Result<JsValue, VmError> {
    context.get(state, AGGREGATE_VALUES)
}

fn set_aggregate_value(
    context: &mut NativeContext,
    state: JsValue,
    index: usize,
    value: JsValue,
) -> Result<(), VmError> {
    let values = aggregate_values(context, state)?;
    context.set(values, &index.to_string(), value, true)?;
    Ok(())
}

fn decrement_aggregate_remaining(
    context: &mut NativeContext,
    state: JsValue,
) -> Result<bool, VmError> {
    let JsValue::Number(remaining) = context.get(state.clone(), AGGREGATE_REMAINING)? else {
        return Err(VmError::runtime("invalid Promise aggregate counter"));
    };
    let remaining = remaining - 1.0;
    context.set(state, AGGREGATE_REMAINING, JsValue::Number(remaining), true)?;
    Ok(remaining == 0.0)
}

fn aggregate_index(arguments: &[JsValue]) -> Result<usize, VmError> {
    match arguments.get(1) {
        Some(JsValue::Number(index)) if *index >= 0.0 => Ok(*index as usize),
        _ => Err(VmError::runtime("invalid Promise aggregate index")),
    }
}

fn promise_all_fulfill(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let state = aggregate_state(arguments)?;
    set_aggregate_value(
        context,
        state.clone(),
        aggregate_index(arguments)?,
        arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
    )?;
    if decrement_aggregate_remaining(context, state.clone())? {
        let target = aggregate_target(context, state.clone())?;
        let values = aggregate_values(context, state)?;
        context.fulfill_promise(target, values)?;
    }
    Ok(JsValue::Undefined)
}

fn promise_all_settled_fulfill(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    settle_all_settled(context, arguments, true)
}

fn promise_all_settled_reject(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    settle_all_settled(context, arguments, false)
}

fn settle_all_settled(
    context: &mut NativeContext,
    arguments: &[JsValue],
    fulfilled: bool,
) -> Result<JsValue, VmError> {
    let state = aggregate_state(arguments)?;
    let value = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    let result = if fulfilled {
        context.create_object([
            ("status".into(), JsValue::String("fulfilled".into())),
            ("value".into(), value),
        ])?
    } else {
        context.create_object([
            ("status".into(), JsValue::String("rejected".into())),
            ("reason".into(), value),
        ])?
    };
    set_aggregate_value(context, state.clone(), aggregate_index(arguments)?, result)?;
    if decrement_aggregate_remaining(context, state.clone())? {
        let target = aggregate_target(context, state.clone())?;
        let values = aggregate_values(context, state)?;
        context.fulfill_promise(target, values)?;
    }
    Ok(JsValue::Undefined)
}

fn promise_any_reject(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let state = aggregate_state(arguments)?;
    set_aggregate_value(
        context,
        state.clone(),
        aggregate_index(arguments)?,
        arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
    )?;
    if decrement_aggregate_remaining(context, state.clone())? {
        let target = aggregate_target(context, state)?;
        context.reject_promise(target, aggregate_error())?;
    }
    Ok(JsValue::Undefined)
}

fn promise_aggregate_fulfill(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let state = aggregate_state(arguments)?;
    let target = aggregate_target(context, state)?;
    context.fulfill_promise(
        target,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Undefined)
}

fn promise_aggregate_reject(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let state = aggregate_state(arguments)?;
    let target = aggregate_target(context, state)?;
    context.reject_promise(
        target,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Undefined)
}

fn aggregate_error() -> JsValue {
    JsValue::Error(crate::runtime::NativeErrorValue::new(
        crate::runtime::NativeErrorKind::Error,
        "AggregateError",
    ))
}

fn type_error_value(message: &str) -> JsValue {
    JsValue::Error(crate::runtime::NativeErrorValue::new(
        crate::runtime::NativeErrorKind::Type,
        message,
    ))
}

fn promise_try(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let capability = new_promise_capability(vm, context, this)?;
    let call_args = arguments.iter().skip(1).cloned().collect();
    let (settler, value) = match vm.call_value_catching_from_builtin(
        callback,
        JsValue::Undefined,
        call_args,
        context,
    )? {
        Ok(value) => (capability.resolve.clone(), value),
        Err(value) => (capability.reject.clone(), value),
    };
    vm.call_value_from_builtin(settler, JsValue::Undefined, vec![value], context)?;
    Ok(capability.promise)
}

fn promise_with_resolvers(
    vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let capability = new_promise_capability(vm, context, this)?;
    context.create_object([
        ("promise".into(), capability.promise),
        ("resolve".into(), capability.resolve),
        ("reject".into(), capability.reject),
    ])
}

fn promise_capability_executor(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let state = arguments
        .first()
        .cloned()
        .ok_or_else(|| VmError::runtime("missing Promise capability state"))?;
    if !matches!(
        context.get(state.clone(), CAPABILITY_RESOLVE)?,
        JsValue::Undefined
    ) || !matches!(
        context.get(state.clone(), CAPABILITY_REJECT)?,
        JsValue::Undefined
    ) {
        return Err(VmError::type_error(
            "Promise capability executor called more than once",
        ));
    }
    let resolve = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let reject = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);
    context.set(state.clone(), CAPABILITY_RESOLVE, resolve, true)?;
    context.set(state, CAPABILITY_REJECT, reject, true)?;
    Ok(JsValue::Undefined)
}

fn promise_then(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    promise_then_with_finally(context, this, arguments, false)
}

fn promise_catch(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let on_rejected = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let args = [JsValue::Undefined, on_rejected];
    promise_then_with_finally(context, this, &args, false)
}

fn promise_finally(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    promise_then_with_finally(context, this, arguments, true)
}

fn promise_then_with_finally(
    context: &mut NativeContext,
    this: JsValue,
    arguments: &[JsValue],
    finally: bool,
) -> Result<JsValue, VmError> {
    let Some(source) = context.promise_id_from_value(&this) else {
        return Err(VmError::type_error("Promise method called on non-promise"));
    };
    let (result_object, result_promise) = create_promise_object(context)?;
    let on_fulfilled = arguments
        .first()
        .filter(|value| is_callable(value))
        .cloned();
    let on_rejected = if finally {
        on_fulfilled.clone()
    } else {
        arguments.get(1).filter(|value| is_callable(value)).cloned()
    };
    context.add_promise_reaction(
        source,
        PromiseThenReaction {
            result_promise,
            on_fulfilled,
            on_rejected,
            finally,
        },
    )?;
    Ok(result_object)
}

fn promise_resolve_executor(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let promise_object = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let Some(promise) = context.promise_id_from_value(&promise_object) else {
        return Err(VmError::type_error("invalid Promise resolve function"));
    };
    resolve_promise_value(vm, context, promise_object, promise, value)?;
    Ok(JsValue::Undefined)
}

fn resolve_promise_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    promise_object: JsValue,
    promise: crate::runtime::PromiseId,
    value: JsValue,
) -> Result<(), VmError> {
    if promise_object.strict_equals(&value) {
        return enqueue_settle(
            context,
            promise,
            PromiseReaction::Reject,
            JsValue::Error(crate::runtime::NativeErrorValue::new(
                crate::runtime::NativeErrorKind::Type,
                "Promise cannot resolve to itself",
            )),
        );
    }

    if let Some(source) = context.promise_id_from_value(&value) {
        return context.add_promise_reaction(
            source,
            PromiseThenReaction {
                result_promise: promise,
                on_fulfilled: None,
                on_rejected: None,
                finally: false,
            },
        );
    }

    if context.value_object(&value).is_some() {
        let then =
            match vm.get_property_value_catching_from_builtin(value.clone(), "then", context)? {
                Ok(then) => then,
                Err(reason) => {
                    return enqueue_settle(context, promise, PromiseReaction::Reject, reason);
                }
            };
        if is_callable(&then) {
            let (resolve, reject) = create_promise_resolving_functions(context, promise_object)?;
            if let Err(reason) =
                vm.call_value_catching_from_builtin(then, value, vec![resolve, reject], context)?
            {
                enqueue_settle(context, promise, PromiseReaction::Reject, reason)?;
            }
            return Ok(());
        }
    }

    enqueue_settle(context, promise, PromiseReaction::Fulfill, value)
}

fn promise_reject_executor(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let promise_object = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let reason = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let Some(promise) = context.promise_id_from_value(&promise_object) else {
        return Err(VmError::type_error("invalid Promise reject function"));
    };
    enqueue_settle(context, promise, PromiseReaction::Reject, reason)?;
    Ok(JsValue::Undefined)
}

fn promise_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this)
}
