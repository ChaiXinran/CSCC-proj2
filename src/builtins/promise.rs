//! Minimal JS-visible Promise builtins backed by the shared native job queue.

use crate::{
    runtime::{
        Job, JsObject, JsValue, NativeCall, NativeContext, ObjectId, PromiseJob, PromiseReaction,
        PromiseThenReaction, PropertyDescriptor,
    },
    vm::{Vm, VmError},
};

const PROMISE_RESOLVE_FUNCTION: &str = "__AgentJSPromiseResolveFunction";
const PROMISE_REJECT_FUNCTION: &str = "__AgentJSPromiseRejectFunction";

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

fn create_promise_capability(
    context: &mut NativeContext,
) -> Result<(JsValue, crate::runtime::PromiseId, JsValue, JsValue), VmError> {
    let (promise_object, promise) = create_promise_object(context)?;
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
        1,
    )?;
    let reject = context.register_bound_function(
        reject_target,
        JsValue::Undefined,
        vec![promise_object.clone()],
        1,
    )?;
    Ok((promise_object, promise, resolve, reject))
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
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    let executor = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&executor) {
        return Err(VmError::type_error("Promise resolver is not a function"));
    }
    let (promise_object, promise, resolve, reject) = create_promise_capability(context)?;

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
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if context.promise_id_from_value(&value).is_some() {
        return Ok(value);
    }
    let (promise_object, promise) = create_promise_object(context)?;
    enqueue_settle(context, promise, PromiseReaction::Fulfill, value)?;
    Ok(promise_object)
}

fn promise_reject(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let reason = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (promise_object, promise) = create_promise_object(context)?;
    enqueue_settle(context, promise, PromiseReaction::Reject, reason)?;
    Ok(promise_object)
}

fn promise_all(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let iterable = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let values = collect_simple_array_values(context, iterable)?;
    let array = context.create_array(values)?;
    let (promise_object, promise) = create_promise_object(context)?;
    enqueue_settle(context, promise, PromiseReaction::Fulfill, array)?;
    Ok(promise_object)
}

fn promise_all_settled(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let iterable = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let values = collect_simple_array_values(context, iterable)?;
    let mut settled = Vec::with_capacity(values.len());
    for value in values {
        settled.push(context.create_object([
            ("status".into(), JsValue::String("fulfilled".into())),
            ("value".into(), value),
        ])?);
    }
    let array = context.create_array(settled)?;
    let (promise_object, promise) = create_promise_object(context)?;
    enqueue_settle(context, promise, PromiseReaction::Fulfill, array)?;
    Ok(promise_object)
}

fn promise_any(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let iterable = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let values = collect_simple_array_values(context, iterable)?;
    let (promise_object, promise) = create_promise_object(context)?;
    if let Some(value) = values.into_iter().next() {
        enqueue_settle(context, promise, PromiseReaction::Fulfill, value)?;
    } else {
        enqueue_settle(
            context,
            promise,
            PromiseReaction::Reject,
            JsValue::Error(crate::runtime::NativeErrorValue::new(
                crate::runtime::NativeErrorKind::Error,
                "AggregateError",
            )),
        )?;
    }
    Ok(promise_object)
}

fn promise_race(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let iterable = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let values = collect_simple_array_values(context, iterable)?;
    let (promise_object, promise) = create_promise_object(context)?;
    if let Some(value) = values.into_iter().next() {
        enqueue_settle(context, promise, PromiseReaction::Fulfill, value)?;
    }
    Ok(promise_object)
}

fn promise_try(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (promise_object, promise) = create_promise_object(context)?;
    if !is_callable(&callback) {
        enqueue_settle(
            context,
            promise,
            PromiseReaction::Reject,
            JsValue::Error(crate::runtime::NativeErrorValue::new(
                crate::runtime::NativeErrorKind::Type,
                "Promise.try callback is not callable",
            )),
        )?;
        return Ok(promise_object);
    }
    let call_args = arguments.iter().skip(1).cloned().collect();
    match vm.call_value_catching_from_builtin(callback, JsValue::Undefined, call_args, context)? {
        Ok(value) => enqueue_settle(context, promise, PromiseReaction::Fulfill, value)?,
        Err(value) => enqueue_settle(context, promise, PromiseReaction::Reject, value)?,
    }
    Ok(promise_object)
}

fn promise_with_resolvers(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (promise, _promise_id, resolve, reject) = create_promise_capability(context)?;
    context.create_object([
        ("promise".into(), promise),
        ("resolve".into(), resolve),
        ("reject".into(), reject),
    ])
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

fn collect_simple_array_values(
    context: &mut NativeContext,
    iterable: JsValue,
) -> Result<Vec<JsValue>, VmError> {
    let object = context.require_object(&iterable, "Promise iterable")?;
    let Some(length) = context
        .heap()
        .object(object)
        .and_then(JsObject::array_length)
    else {
        return Err(VmError::type_error("Promise iterable must be an array"));
    };
    // ponytail: Fix2-B skeleton only handles concrete arrays. Upgrade path:
    // route this through the shared GetIterator/IteratorClose helper once
    // Promise combinators are ready to consume arbitrary iterables/thenables.
    let mut values = Vec::with_capacity(length);
    for index in 0..length {
        values.push(context.get(iterable.clone(), &index.to_string())?);
    }
    Ok(values)
}

fn promise_resolve_executor(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let promise_object = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let Some(promise) = context.promise_id_from_value(&promise_object) else {
        return Err(VmError::type_error("invalid Promise resolve function"));
    };
    if promise_object.strict_equals(&value) {
        return enqueue_settle(
            context,
            promise,
            PromiseReaction::Reject,
            JsValue::Error(crate::runtime::NativeErrorValue::new(
                crate::runtime::NativeErrorKind::Type,
                "Promise cannot resolve to itself",
            )),
        )
        .map(|_| JsValue::Undefined);
    }
    enqueue_settle(context, promise, PromiseReaction::Fulfill, value)?;
    Ok(JsValue::Undefined)
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
