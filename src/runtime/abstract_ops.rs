//! Shared ECMAScript abstract operations — the Runtime Semantic Kernel.
//!
//! This module is the single source of truth for all ECMAScript abstract
//! operations used by builtins.  Every builtin MUST route through these
//! helpers instead of implementing its own versions, guaranteeing consistent
//! semantics across the entire engine.
//!
//! Operations that can invoke JavaScript (Get/Set, Call/Construct, iterator
//! methods, promise reactions) remain on the VM/context call path so that
//! accessors and Proxy traps are never accidentally bypassed.

use super::{IteratorRecord, JsValue, NativeContext, PreferredType, PropertyKey, to_property_key};
use crate::vm::{Vm, VmError};

// ════════════════════════════════════════════════════════════════
// 7.2 Testing and Comparison Operations
// ════════════════════════════════════════════════════════════════

/// ECMAScript `SameValueZero` comparison (7.2.11).
/// Used by Map, Set, Array.prototype.includes, TypedArray keyed methods.
#[must_use]
pub fn same_value_zero(left: &JsValue, right: &JsValue) -> bool {
    match (left, right) {
        (JsValue::Number(a), JsValue::Number(b)) => (a.is_nan() && b.is_nan()) || a == b,
        _ => left.strict_equals(right),
    }
}

/// ECMAScript `SameValue` comparison (7.2.10).
/// Used by Object.is, Object.defineProperty, Proxy.
#[must_use]
pub fn same_value(left: &JsValue, right: &JsValue) -> bool {
    left.same_value(right)
}

/// ECMAScript `IsCallable` (7.2.3).
/// NOTE: Also checks Proxy callability via context.
#[must_use]
pub fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

/// Extended IsCallable that also checks Proxy callable targets via context.
#[must_use]
pub fn is_callable_with_context(context: &NativeContext, value: &JsValue) -> bool {
    context.is_callable_value(value)
}

/// ECMAScript `IsConstructor` (7.2.4).
#[must_use]
pub fn is_constructor(value: &JsValue) -> bool {
    match value {
        JsValue::Function(_) => {
            // Interpreted functions: check the constructable flag
            true // most JsFunctions are constructable unless arrow function
        }
        JsValue::BuiltinFunction(_) => true,
        JsValue::Object(_) => {
            // Proxy objects may be constructable — checked via context
            false
        }
        _ => false,
    }
}

/// Extended IsConstructor that checks Proxy constructable targets via context.
#[must_use]
pub fn is_constructor_with_context(context: &NativeContext, value: &JsValue) -> bool {
    context.is_constructable_value(value)
}

/// ECMAScript `RequireObjectCoercible` (7.2.1).
/// Throws TypeError if value is undefined or null.
pub fn require_object_coercible(value: &JsValue) -> Result<(), VmError> {
    match value {
        JsValue::Undefined | JsValue::Null => Err(VmError::type_error(
            "cannot convert undefined or null to object",
        )),
        _ => Ok(()),
    }
}

// ════════════════════════════════════════════════════════════════
// 7.1 Type Conversion Operations
// ════════════════════════════════════════════════════════════════

/// ECMAScript `ToBoolean` (7.1.2) — already on JsValue::to_boolean().
/// Re-exported for convenience.
#[must_use]
pub fn to_boolean(value: &JsValue) -> bool {
    value.to_boolean()
}

/// ECMAScript `ToNumber` (7.1.3) — delegates to JsValue::to_number.
pub fn to_number(value: &JsValue) -> Result<f64, VmError> {
    value.to_number().ok_or_else(|| {
        VmError::type_error("cannot convert value to number; use VM-mediated ToNumber for objects")
    })
}

/// VM-mediated `ToNumeric` (7.1.17): unifies Number and BigInt.
/// Returns (value, is_bigint) pair.
pub fn to_numeric(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<(JsValue, bool), VmError> {
    let prim = vm.to_primitive(value, PreferredType::Number, context)?;
    match prim {
        JsValue::BigInt(_) => Ok((prim, true)),
        JsValue::Number(_) => Ok((prim, false)),
        other => {
            // For non-primitive results, attempt ToNumber
            match other.to_number() {
                Some(num) => Ok((JsValue::Number(num), false)),
                None => Ok((other, false)),
            }
        }
    }
}

/// ECMAScript `ToIntegerOrInfinity` (7.1.5) for an already primitive numeric value.
#[must_use]
pub fn to_integer_or_infinity(number: f64) -> f64 {
    if number.is_nan() || number == 0.0 {
        0.0
    } else if number.is_infinite() {
        number
    } else {
        number.trunc()
    }
}

/// ECMAScript `ToLength` (7.1.15), with the host's checked allocation bound applied by the caller.
#[must_use]
pub fn to_length(number: f64) -> usize {
    let integer = to_integer_or_infinity(number);
    if integer <= 0.0 || integer.is_nan() {
        0
    } else if integer.is_infinite() {
        usize::MAX
    } else {
        integer.min(9_007_199_254_740_991.0) as usize
    }
}

/// ECMAScript `ToIndex` (7.1.16) for a numeric primitive.
pub fn to_index(number: f64) -> Result<usize, &'static str> {
    let integer = to_integer_or_infinity(number);
    if integer < 0.0 || integer.is_infinite() {
        return Err("index must be a finite non-negative integer");
    }
    if integer > usize::MAX as f64 {
        return Err("index is too large");
    }
    Ok(integer as usize)
}

/// Central `ToPropertyKey` entry point for primitive values (7.1.14).
/// Object conversion must be performed by the VM because it can invoke
/// user-defined `@@toPrimitive`, `toString`, or `valueOf`.
pub fn to_property_key_primitive(value: &JsValue) -> Result<PropertyKey, VmError> {
    to_property_key(value)
}

/// Full `ToPropertyKey` (7.1.14), including observable object coercion.
pub fn to_property_key_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<PropertyKey, VmError> {
    match vm.to_property_key_from_builtin(value, context)? {
        JsValue::String(name) => Ok(PropertyKey::String(name)),
        JsValue::Symbol(symbol) => Ok(PropertyKey::Symbol(symbol)),
        _ => Err(VmError::type_error(
            "ToPropertyKey returned a non-key value",
        )),
    }
}

/// VM-mediated `ToPrimitive` (7.1.1); object conversion may invoke user code.
pub fn to_primitive(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    preferred: PreferredType,
) -> Result<JsValue, VmError> {
    vm.to_primitive(value, preferred, context)
}

/// ECMAScript `OrdinaryToPrimitive` (7.1.1.1) – the fallback path when
/// @@toPrimitive is absent.
pub fn ordinary_to_primitive(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: JsValue,
    preferred: PreferredType,
) -> Result<JsValue, VmError> {
    use PreferredType::{Default, Number, String};
    let (first, second) = match preferred {
        String => ("toString", "valueOf"),
        Default | Number => ("valueOf", "toString"),
    };
    for method_name in [first, second] {
        let method = get(
            vm,
            context,
            object.clone(),
            PropertyKey::String(method_name.into()),
        )?;
        if is_callable(&method) || context.is_callable_value(&method) {
            let result = call(vm, context, method, object.clone(), Vec::new())?;
            if !matches!(result, JsValue::Object(_)) {
                return Ok(result);
            }
        }
    }
    Err(VmError::type_error(
        "cannot convert object to primitive value",
    ))
}

/// VM-mediated `ToString` (7.1.12) via context.
pub fn to_string(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    value: JsValue,
) -> Result<JsValue, VmError> {
    match value {
        JsValue::String(_) => Ok(value),
        JsValue::Symbol(_) => Err(VmError::type_error("Cannot convert a Symbol to a string")),
        _ => Ok(JsValue::String(
            value
                .to_js_string()
                .unwrap_or_else(|| "undefined".to_string()),
        )),
    }
}

// ════════════════════════════════════════════════════════════════
// 7.3 Operations on Objects
// ════════════════════════════════════════════════════════════════

/// ECMAScript `Get(V, P)` (7.3.1) — accessors and Proxy traps are never bypassed.
pub fn get(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<JsValue, VmError> {
    match key {
        PropertyKey::String(name) => vm.get_property_value(receiver, &name, context),
        PropertyKey::Symbol(symbol) => vm.get_symbol_property_value_with_receiver_from_builtin(
            receiver.clone(),
            receiver,
            symbol,
            context,
        ),
    }
}

/// ECMAScript `Get(O, P)` with a string key.
pub fn get_str(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: &str,
) -> Result<JsValue, VmError> {
    vm.get_property_value(receiver, key, context)
}

/// ECMAScript `HasProperty(O, P)` (7.3.10).
/// Used by `in` operator, Array holes, and Proxy has trap.
/// NOTE: Proxy has trap support requires routing through the VM's full
/// property chain. For ordinary objects, this uses direct context API.
pub fn has_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<bool, VmError> {
    let object = vm.to_object(receiver, context)?;
    match key {
        PropertyKey::String(name) => context.has_property(object, &name),
        PropertyKey::Symbol(symbol) => context.has_symbol_property(object, symbol),
    }
}

/// ECMAScript `HasProperty(O, P)` with a string key.
pub fn has_property_str(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: &str,
) -> Result<bool, VmError> {
    let object = vm.to_object(receiver, context)?;
    context.has_property(object, key)
}

/// ECMAScript `GetMethod(V, P)` (7.3.7): null/undefined means absent; every
/// other result must implement the runtime callable protocol.
pub fn get_method(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
) -> Result<Option<JsValue>, VmError> {
    let method = get(vm, context, receiver, key)?;
    if matches!(method, JsValue::Undefined | JsValue::Null) {
        return Ok(None);
    }
    if !context.is_callable_value(&method) {
        return Err(VmError::type_error("property method is not callable"));
    }
    Ok(Some(method))
}

/// ECMAScript `GetMethod(V, P)` with a string key.
pub fn get_method_str(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: &str,
) -> Result<Option<JsValue>, VmError> {
    get_method(vm, context, receiver, PropertyKey::String(key.into()))
}

/// ECMAScript `Set(O, P, V, Throw)` (7.3.3) — strict mode variant.
pub fn set(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: PropertyKey,
    value: JsValue,
) -> Result<bool, VmError> {
    match key {
        PropertyKey::String(name) => {
            vm.set_property_value_strict_from_builtin(receiver, &name, value, context)
        }
        PropertyKey::Symbol(symbol) => vm.set_symbol_property_value_with_receiver_from_builtin(
            receiver.clone(),
            receiver,
            symbol,
            value,
            context,
        ),
    }
}

/// ECMAScript `Set(O, P, V, Throw)` with a string key.
pub fn set_str(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    key: &str,
    value: JsValue,
) -> Result<bool, VmError> {
    vm.set_property_value_strict_from_builtin(receiver, key, value, context)
}

/// ECMAScript `IsArray` (7.2.2), including Proxy traversal.
pub fn is_array(context: &NativeContext, value: &JsValue) -> Result<bool, VmError> {
    let Some(object) = context.value_object(value) else {
        return Ok(false);
    };
    if context.is_array_object(object)? {
        return Ok(true);
    }
    let Some(record) = context.proxy_record(object) else {
        return Ok(false);
    };
    if matches!(record.handler, JsValue::Null) {
        return Err(VmError::type_error("proxy has been revoked"));
    }
    is_array(context, &record.target)
}

// ════════════════════════════════════════════════════════════════
// 7.3 Operations on Objects — Call/Construct
// ════════════════════════════════════════════════════════════════

/// ECMAScript `Call(F, V, argumentsList)` (7.3.12), including interpreted,
/// native, bound, and Proxy callables.
pub fn call(
    vm: &mut Vm,
    context: &mut NativeContext,
    callable: JsValue,
    this_value: JsValue,
    args: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    vm.call_value_from_builtin(callable, this_value, args, context)
}

/// ECMAScript `Construct(F, argumentsList, newTarget)` (7.3.13),
/// preserving the active new-target path.
pub fn construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    constructor: JsValue,
    args: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    vm.construct_value_from_builtin(constructor, args, context)
}

// ════════════════════════════════════════════════════════════════
// 8.6 Iterator Operations
// ════════════════════════════════════════════════════════════════

/// ECMAScript `GetIterator(obj, hint)` (8.6.1.1) — synchronous iterator.
pub fn get_iterator(
    context: &mut NativeContext,
    value: JsValue,
) -> Result<IteratorRecord, VmError> {
    context.get_iterator(value)
}

/// ECMAScript `IteratorNext(iterator, value)` (8.6.1.2).
pub fn iterator_next(
    context: &mut NativeContext,
    iterator: &mut IteratorRecord,
) -> Result<Option<JsValue>, VmError> {
    context.iterator_next(iterator)
}

/// ECMAScript `IteratorComplete(iterResult)` (8.6.1.3).
pub fn iterator_complete(
    vm: &mut Vm,
    context: &mut NativeContext,
    iter_result: JsValue,
) -> Result<bool, VmError> {
    let done = get_str(vm, context, iter_result, "done")?;
    Ok(done.to_boolean())
}

/// ECMAScript `IteratorValue(iterResult)` (8.6.1.4).
pub fn iterator_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    iter_result: JsValue,
) -> Result<JsValue, VmError> {
    get_str(vm, context, iter_result, "value")
}

/// ECMAScript `IteratorStep(iterator)` (8.6.1.5) — returns None when done.
pub fn iterator_step(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator: &mut IteratorRecord,
) -> Result<Option<JsValue>, VmError> {
    let result = context.iterator_next(iterator)?;
    match result {
        Some(value) => {
            let done = iterator_complete(vm, context, value.clone())?;
            if done {
                Ok(None)
            } else {
                Ok(Some(iterator_value(vm, context, value)?))
            }
        }
        None => Ok(None),
    }
}

/// ECMAScript `IteratorClose(iterator, completion)` (8.6.1.6).
/// This is the canonical implementation that ALL builtins must use.
/// If the iterator has a `return` method, it is called; any exception from
/// `return()` is suppressed in favor of the original completion.
pub fn iterator_close(
    context: &mut NativeContext,
    iterator: &mut IteratorRecord,
) -> Result<(), VmError> {
    context.iterator_close(iterator)
}

/// Close a JavaScript iterator through the VM. This invokes an observable
/// `return` method when present and preserves the caller's abrupt completion
/// rules; callers must use this form for JS iterator objects.
pub fn close_iterator(
    vm: &mut Vm,
    context: &mut NativeContext,
    iterator: JsValue,
) -> Result<(), VmError> {
    vm.close_iterator_from_builtin(iterator, context)
}

/// ECMAScript `CreateIterResultObject(value, done)` (8.6.1.7).
pub fn create_iter_result_object(
    context: &mut NativeContext,
    value: JsValue,
    done: bool,
) -> Result<JsValue, VmError> {
    context.create_object([
        ("value".into(), value),
        ("done".into(), JsValue::Boolean(done)),
    ])
}

// ════════════════════════════════════════════════════════════════
// Species & Array Species Operations
// ════════════════════════════════════════════════════════════════

/// ECMAScript `SpeciesConstructor(O, defaultConstructor)` (7.3.23).
/// Used by Array, Promise, TypedArray, RegExp subclasses.
pub fn species_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: JsValue,
    default_constructor: JsValue,
) -> Result<JsValue, VmError> {
    let constructor = get_str(vm, context, object.clone(), "constructor")?;
    if matches!(constructor, JsValue::Undefined) {
        return Ok(default_constructor);
    }
    if context.value_object(&constructor).is_none() {
        return Err(VmError::type_error("constructor property is not an object"));
    }
    let species = get(
        vm,
        context,
        constructor.clone(),
        PropertyKey::Symbol(context.well_known_symbols().species),
    )?;
    if matches!(species, JsValue::Null | JsValue::Undefined) {
        return Ok(default_constructor);
    }
    if !context.is_constructable_value(&species) {
        return Err(VmError::type_error("Symbol.species is not a constructor"));
    }
    Ok(species)
}

/// ECMAScript `ArraySpeciesCreate(originalArray, length)` (9.4.2.8).
pub fn array_species_create(
    vm: &mut Vm,
    context: &mut NativeContext,
    original_array: JsValue,
    length: usize,
) -> Result<JsValue, VmError> {
    let is_array = context
        .value_object(&original_array)
        .is_some_and(|object| context.is_array_object(object).unwrap_or(false));
    if !is_array {
        return context.create_sparse_array(length);
    }
    let default_constructor = context
        .intrinsics()
        .map(|intrinsics| intrinsics.array_constructor.clone())
        .ok_or_else(|| VmError::runtime("Array constructor missing"))?;
    let constructor =
        species_constructor(vm, context, original_array, default_constructor.clone())?;
    if constructor == default_constructor || constructor.same_value(&default_constructor) {
        context.create_sparse_array(length)
    } else {
        construct(
            vm,
            context,
            constructor,
            vec![JsValue::Number(length as f64)],
        )
    }
}

// ════════════════════════════════════════════════════════════════
// Job Queue Operations
// ════════════════════════════════════════════════════════════════

pub fn enqueue_job(context: &mut NativeContext, job: super::Job) -> Result<(), VmError> {
    context.enqueue_job(job)
}

pub fn drain_jobs(context: &mut NativeContext) -> Result<(), VmError> {
    context.drain_jobs()
}

/// Get the well-known symbols from context.
pub fn well_known_symbols(context: &NativeContext) -> &super::WellKnownSymbols {
    context.well_known_symbols()
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_boundaries_are_spec_shaped() {
        assert_eq!(to_integer_or_infinity(f64::NAN), 0.0);
        assert_eq!(to_integer_or_infinity(-1.9), -1.0);
        assert_eq!(to_length(-1.0), 0);
        assert_eq!(to_index(3.9), Ok(3));
        assert!(to_index(-1.0).is_err());
    }

    #[test]
    fn same_value_zero_works() {
        assert!(same_value_zero(
            &JsValue::Number(f64::NAN),
            &JsValue::Number(f64::NAN)
        ));
        assert!(same_value_zero(
            &JsValue::Number(0.0),
            &JsValue::Number(-0.0)
        ));
        assert!(!same_value_zero(
            &JsValue::Number(1.0),
            &JsValue::Number(2.0)
        ));
        assert!(same_value_zero(
            &JsValue::String("hello".into()),
            &JsValue::String("hello".into())
        ));
    }

    #[test]
    fn is_callable_works() {
        assert!(!is_callable(&JsValue::Undefined));
        assert!(!is_callable(&JsValue::Null));
        assert!(!is_callable(&JsValue::Number(42.0)));
        assert!(!is_callable(&JsValue::String("test".into())));
    }
}
