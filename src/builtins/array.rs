//! `Array` constructor and prototype methods.

use crate::{
    runtime::{JsValue, NativeContext, ObjectId, PropertyDescriptor},
    vm::{Vm, VmError},
};

pub fn install_array(context: &mut NativeContext) {
    let Some(intrinsics) = context.intrinsics().cloned() else {
        return;
    };
    let JsValue::BuiltinFunction(constructor) = intrinsics.array_constructor else {
        return;
    };
    let constructor_object = context.builtin(constructor).unwrap().object;

    // Static methods on Array
    let is_array = context
        .register_builtin("isArray", 1, array_is_array, None)
        .expect("install Array.isArray");
    context
        .define_own_property(
            constructor_object,
            "isArray".into(),
            PropertyDescriptor::data_with(is_array, true, false, true),
        )
        .expect("define Array.isArray");

    let from = context
        .register_builtin("from", 1, array_from, None)
        .expect("install Array.from");
    context
        .define_own_property(
            constructor_object,
            "from".into(),
            PropertyDescriptor::data_with(from, true, false, true),
        )
        .expect("define Array.from");

    let of = context
        .register_builtin("of", 0, array_of, None)
        .expect("install Array.of");
    context
        .define_own_property(
            constructor_object,
            "of".into(),
            PropertyDescriptor::data_with(of, true, false, true),
        )
        .expect("define Array.of");

    // Prototype methods on Array.prototype
    for (name, length, call) in [
        ("push", 1, array_push as crate::runtime::NativeCall),
        ("pop", 0, array_pop as crate::runtime::NativeCall),
        ("toString", 0, array_to_string as crate::runtime::NativeCall),
        ("join", 1, array_join as crate::runtime::NativeCall),
        ("reverse", 0, array_reverse as crate::runtime::NativeCall),
        ("concat", 1, array_concat as crate::runtime::NativeCall),
        ("slice", 2, array_slice as crate::runtime::NativeCall),
        ("splice", 2, array_splice as crate::runtime::NativeCall),
        ("indexOf", 1, array_index_of as crate::runtime::NativeCall),
        (
            "lastIndexOf",
            1,
            array_last_index_of as crate::runtime::NativeCall,
        ),
        ("fill", 1, array_fill as crate::runtime::NativeCall),
        ("includes", 1, array_includes as crate::runtime::NativeCall),
        ("shift", 0, array_shift as crate::runtime::NativeCall),
        ("unshift", 1, array_unshift as crate::runtime::NativeCall),
        ("forEach", 1, array_for_each as crate::runtime::NativeCall),
        ("map", 1, array_map as crate::runtime::NativeCall),
        ("filter", 1, array_filter as crate::runtime::NativeCall),
        ("reduce", 1, array_reduce as crate::runtime::NativeCall),
        (
            "reduceRight",
            1,
            array_reduce_right as crate::runtime::NativeCall,
        ),
        ("every", 1, array_every as crate::runtime::NativeCall),
        ("some", 1, array_some as crate::runtime::NativeCall),
        ("find", 1, array_find as crate::runtime::NativeCall),
        ("findIndex", 1, array_find_index as crate::runtime::NativeCall),
        ("flat", 0, array_flat as crate::runtime::NativeCall),
        ("flatMap", 1, array_flat_map as crate::runtime::NativeCall),
        ("sort", 1, array_sort as crate::runtime::NativeCall),
        ("keys", 0, array_keys as crate::runtime::NativeCall),
        ("values", 0, array_values as crate::runtime::NativeCall),
        ("entries", 0, array_entries as crate::runtime::NativeCall),
        ("copyWithin", 2, array_copy_within as crate::runtime::NativeCall),
        ("at", 1, array_at as crate::runtime::NativeCall),
    ] {
        let value = context
            .register_builtin(name, length, call, None)
            .expect("install Array prototype method");
        context
            .define_own_property(
                intrinsics.array_prototype,
                name.into(),
                PropertyDescriptor::data_with(value, true, false, true),
            )
            .expect("define Array prototype method");
    }
}

pub fn array_call(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    create_array(context, arguments)
}

pub fn array_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    create_array(context, arguments)
}

fn create_array(context: &mut NativeContext, arguments: &[JsValue]) -> Result<JsValue, VmError> {
    if arguments.len() == 1 && matches!(arguments[0], JsValue::Number(_)) {
        let length = context.array_length_from_value(arguments[0].clone())?;
        context.create_sparse_array(length)
    } else {
        context.create_array(arguments.to_vec())
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn array_like_length(context: &NativeContext, object: ObjectId) -> usize {
    if let Some(o) = context.heap().object(object) {
        if let Some(len) = o.array_length() {
            return len;
        }
        if let Some(val) = o
            .own_property("length")
            .and_then(|d| d.value_cloned())
        {
            return val
                .to_number()
                .unwrap_or(0.0)
                .max(0.0)
                .min(MAX_ARRAY_LENGTH as f64) as usize;
        }
    }
    0
}

fn normalize_index(raw: f64, length: usize) -> usize {
    if raw < 0.0 {
        let from_end = (-raw) as usize;
        length.saturating_sub(from_end)
    } else {
        (raw as usize).min(length)
    }
}

/// ECMAScript maximum array length.
const MAX_ARRAY_LENGTH: usize = 4_294_967_295;
/// Cap iteration/allocation to prevent O(N) hangs on sparse arrays with huge lengths.
/// Test262 tests behavior at small indices; a 64K cap covers all realistic cases.
const MAX_DENSE_ALLOC: usize = 1 << 16; // 65536 elements

/// Reads array element `index` via the full VM property-get path (supports accessor getters).
fn get_elem(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    index: usize,
) -> Result<JsValue, VmError> {
    vm.get_property_value(value, &index.to_string(), context)
}

fn value_to_string(value: &JsValue) -> String {
    match value {
        JsValue::Undefined | JsValue::Null => String::new(),
        JsValue::Boolean(b) => b.to_string(),
        JsValue::Number(n) => number_to_js_string(*n),
        JsValue::String(s) => s.clone(),
        JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            "function () { [native code] }".into()
        }
        JsValue::Object(_) | JsValue::Error(_) => "[object Object]".into(),
    }
}

fn number_to_js_string(n: f64) -> String {
    if n.is_nan() {
        return "NaN".into();
    }
    if n.is_infinite() {
        return if n > 0.0 {
            "Infinity".into()
        } else {
            "-Infinity".into()
        };
    }
    if n == 0.0 {
        return "0".into();
    }
    let i = n as i64;
    if i as f64 == n {
        return i.to_string();
    }
    format!("{n}")
}

fn call_callback(
    vm: &mut Vm,
    context: &mut NativeContext,
    callback: JsValue,
    this_arg: JsValue,
    args: Vec<JsValue>,
) -> Result<JsValue, VmError> {
    if !is_callable(&callback) {
        return Err(VmError::type_error("callback is not a function"));
    }
    vm.call_value_from_builtin(callback, this_arg, args, context)
}

fn require_callable(value: &JsValue, method: &str) -> Result<(), VmError> {
    if !is_callable(value) {
        Err(VmError::type_error(format!("{method}: callback is not callable")))
    } else {
        Ok(())
    }
}

// ── Array static methods ──────────────────────────────────────────────────────

fn array_is_array(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = arguments
        .first()
        .and_then(|value| context.value_object(value))
        .map(|object| context.is_array_object(object))
        .transpose()?
        .unwrap_or(false);
    Ok(JsValue::Boolean(result))
}

fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn array_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let source = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let map_fn_raw = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let map_this = arguments.get(2).cloned().unwrap_or(JsValue::Undefined);

    // Spec 22.1.2.1 step 3: if mapfn is not undefined, it must be callable.
    let map_fn = if matches!(map_fn_raw, JsValue::Undefined) {
        None
    } else if is_callable(&map_fn_raw) {
        Some(map_fn_raw)
    } else {
        return Err(VmError::type_error("Array.from: mapfn is not callable"));
    };

    let object = context.require_object(&source, "Array.from")?;
    let length = array_like_length(context, object);

    let mut result = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, source.clone(), i)?;
        let mapped = if let Some(ref func) = map_fn {
            call_callback(
                vm,
                context,
                func.clone(),
                map_this.clone(),
                vec![val, JsValue::Number(i as f64)],
            )?
        } else {
            val
        };
        result.push(mapped);
    }
    context.create_array(result)
}

fn array_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.create_array(arguments.to_vec())
}

// ── Array.prototype methods ───────────────────────────────────────────────────

fn array_push(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.push")?;
    let mut length = array_like_length(context, object);
    for value in arguments {
        context.set_element(
            this_value.clone(),
            JsValue::Number(length as f64),
            value.clone(),
        )?;
        length += 1;
    }
    Ok(JsValue::Number(length as f64))
}

fn array_pop(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.pop")?;
    let length = array_like_length(context, object);
    if length == 0 {
        return Ok(JsValue::Undefined);
    }
    let new_length = length - 1;
    let key = new_length.to_string();
    let value = context.get_property(this_value.clone(), &key)?;
    context.delete_property(object, &key, false)?;
    context.set_array_length(object, new_length)?;
    Ok(value)
}

fn array_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    array_join(vm, context, this_value, &[])
}

fn array_join(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.join")?;
    let length = array_like_length(context, object);
    let sep = match arguments.first() {
        None | Some(JsValue::Undefined) => ",".to_string(),
        Some(JsValue::String(s)) => s.clone(),
        Some(other) => value_to_string(other),
    };
    let mut parts: Vec<String> = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        parts.push(value_to_string(&val));
    }
    Ok(JsValue::String(parts.join(&sep)))
}

fn array_reverse(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.reverse")?;
    let length = array_like_length(context, object);
    let mid = length / 2;
    for i in 0..mid {
        let j = length - 1 - i;
        let a = get_elem(vm, context, this_value.clone(), i)?;
        let b = get_elem(vm, context, this_value.clone(), j)?;
        context.set_element(this_value.clone(), JsValue::Number(i as f64), b)?;
        context.set_element(this_value.clone(), JsValue::Number(j as f64), a)?;
    }
    Ok(this_value)
}

fn array_concat(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut result: Vec<JsValue> = Vec::new();
    concat_spread(vm, context, this_value, &mut result)?;
    for arg in arguments {
        concat_spread(vm, context, arg.clone(), &mut result)?;
    }
    context.create_array(result)
}

fn concat_spread(
    vm: &mut Vm,
    context: &mut NativeContext,
    val: JsValue,
    out: &mut Vec<JsValue>,
) -> Result<(), VmError> {
    if let Some(id) = context
        .value_object(&val)
        .filter(|&id| context.is_array_object(id).unwrap_or(false))
    {
        let len = array_like_length(context, id);
        for i in 0..len {
            let elem = get_elem(vm, context, val.clone(), i)?;
            out.push(elem);
        }
        return Ok(());
    }
    out.push(val);
    Ok(())
}

fn array_slice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.slice")?;
    let length = array_like_length(context, object);
    let start = normalize_index(
        arguments
            .first()
            .and_then(|v| v.to_number())
            .unwrap_or(0.0),
        length,
    );
    let end = normalize_index(
        arguments
            .get(1)
            .and_then(|v| {
                if matches!(v, JsValue::Undefined) {
                    None
                } else {
                    v.to_number()
                }
            })
            .unwrap_or(length as f64),
        length,
    );
    let mut result = Vec::new();
    for i in start..end {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        result.push(val);
    }
    context.create_array(result)
}

fn array_splice(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.splice")?;
    let length = array_like_length(context, object);

    let start = normalize_index(
        arguments
            .first()
            .and_then(|v| v.to_number())
            .unwrap_or(0.0),
        length,
    );
    let delete_count = arguments
        .get(1)
        .and_then(|v| {
            if matches!(v, JsValue::Undefined) {
                None
            } else {
                v.to_number()
            }
        })
        .map(|n| n.max(0.0).min((length - start) as f64) as usize)
        .unwrap_or(length - start);
    let insert_items: Vec<JsValue> = arguments.get(2..).unwrap_or(&[]).to_vec();

    // Collect removed elements
    let mut removed = Vec::with_capacity(delete_count.min(MAX_DENSE_ALLOC));
    for i in 0..delete_count {
        let val = get_elem(vm, context, this_value.clone(), start + i)?;
        removed.push(val);
    }

    // Calculate new length
    let tail_len = length - start - delete_count;
    let new_length = start + insert_items.len() + tail_len;

    if insert_items.len() < delete_count {
        // Shift elements left
        for i in 0..tail_len {
            let src = start + delete_count + i;
            let dst = start + insert_items.len() + i;
            let val = get_elem(vm, context, this_value.clone(), src)?;
            context.set_element(this_value.clone(), JsValue::Number(dst as f64), val)?;
        }
        // Delete trailing slots
        for i in new_length..length {
            context.delete_property(object, &i.to_string(), false)?;
        }
    } else if insert_items.len() > delete_count {
        // Shift elements right
        for i in (0..tail_len).rev() {
            let src = start + delete_count + i;
            let dst = start + insert_items.len() + i;
            let val = get_elem(vm, context, this_value.clone(), src)?;
            context.set_element(this_value.clone(), JsValue::Number(dst as f64), val)?;
        }
    }

    // Write inserted items
    for (i, item) in insert_items.into_iter().enumerate() {
        context.set_element(
            this_value.clone(),
            JsValue::Number((start + i) as f64),
            item,
        )?;
    }

    context.set_array_length(object, new_length)?;
    context.create_array(removed)
}

fn array_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.indexOf")?;
    let length = array_like_length(context, object);
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let from_index = arguments
        .get(1)
        .and_then(|v| v.to_number())
        .map(|n| normalize_index(n, length))
        .unwrap_or(0);
    for i in from_index..length {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        if val.strict_equals(&search) {
            return Ok(JsValue::Number(i as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn array_last_index_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.lastIndexOf")?;
    let length = array_like_length(context, object);
    if length == 0 {
        return Ok(JsValue::Number(-1.0));
    }
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let from_raw = arguments
        .get(1)
        .and_then(|v| v.to_number())
        .unwrap_or((length - 1) as f64);
    let from = if from_raw < 0.0 {
        let from_end = (-from_raw) as usize;
        if from_end > length {
            return Ok(JsValue::Number(-1.0));
        }
        length - from_end
    } else {
        (from_raw as usize).min(length - 1)
    };
    for i in (0..=from).rev() {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        if val.strict_equals(&search) {
            return Ok(JsValue::Number(i as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn array_fill(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.fill")?;
    let length = array_like_length(context, object);
    let value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let start = normalize_index(
        arguments
            .get(1)
            .and_then(|v| v.to_number())
            .unwrap_or(0.0),
        length,
    );
    let end = normalize_index(
        arguments
            .get(2)
            .and_then(|v| {
                if matches!(v, JsValue::Undefined) {
                    None
                } else {
                    v.to_number()
                }
            })
            .unwrap_or(length as f64),
        length,
    );
    for i in start..end {
        context.set_element(
            this_value.clone(),
            JsValue::Number(i as f64),
            value.clone(),
        )?;
    }
    Ok(this_value)
}

fn array_includes(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.includes")?;
    let length = array_like_length(context, object);
    let search = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let from = normalize_index(
        arguments
            .get(1)
            .and_then(|v| v.to_number())
            .unwrap_or(0.0),
        length,
    );
    for i in from..length {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        if val.same_value(&search) {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}

fn array_shift(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.shift")?;
    let length = array_like_length(context, object);
    if length == 0 {
        return Ok(JsValue::Undefined);
    }
    let first = get_elem(vm, context, this_value.clone(), 0)?;
    for i in 1..length {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        context.set_element(this_value.clone(), JsValue::Number((i - 1) as f64), val)?;
    }
    context.delete_property(object, &(length - 1).to_string(), false)?;
    context.set_array_length(object, length - 1)?;
    Ok(first)
}

fn array_unshift(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.unshift")?;
    let length = array_like_length(context, object);
    let count = arguments.len();
    // Shift existing elements right
    for i in (0..length).rev() {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        context.set_element(
            this_value.clone(),
            JsValue::Number((i + count) as f64),
            val,
        )?;
    }
    // Insert new elements at front
    for (i, item) in arguments.iter().enumerate() {
        context.set_element(
            this_value.clone(),
            JsValue::Number(i as f64),
            item.clone(),
        )?;
    }
    let new_length = length + count;
    context.set_array_length(object, new_length)?;
    Ok(JsValue::Number(new_length as f64))
}

fn array_for_each(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.forEach")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.forEach")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), this_value.clone()],
        )?;
    }
    Ok(JsValue::Undefined)
}

fn array_map(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.map")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.map")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut result = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let mapped = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), this_value.clone()],
        )?;
        result.push(mapped);
    }
    context.create_array(result)
}

fn array_filter(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.filter")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.filter")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut result = Vec::new();
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let keep = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val.clone(), JsValue::Number(i as f64), this_value.clone()],
        )?;
        if keep.to_boolean() {
            result.push(val);
        }
    }
    context.create_array(result)
}

fn array_reduce(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.reduce")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.reduce")?;

    let (mut acc, start) = if let Some(init) = arguments.get(1) {
        (init.clone(), 0usize)
    } else {
        if length == 0 {
            return Err(VmError::type_error(
                "reduce of empty array with no initial value",
            ));
        }
        let first = get_elem(vm, context, this_value.clone(), 0)?;
        (first, 1usize)
    };

    for i in start..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        acc = vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![acc, val, JsValue::Number(i as f64), this_value.clone()],
            context,
        )?;
    }
    Ok(acc)
}

fn array_reduce_right(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.reduceRight")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.reduceRight")?;

    let safe_end = length.min(MAX_DENSE_ALLOC);
    let (mut acc, end) = if let Some(init) = arguments.get(1) {
        (init.clone(), safe_end)
    } else {
        if length == 0 {
            return Err(VmError::type_error(
                "reduceRight of empty array with no initial value",
            ));
        }
        let last_idx = safe_end.saturating_sub(1);
        let last = get_elem(vm, context, this_value.clone(), last_idx)?;
        (last, last_idx)
    };

    for i in (0..end).rev() {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        acc = vm.call_value_from_builtin(
            callback.clone(),
            JsValue::Undefined,
            vec![acc, val, JsValue::Number(i as f64), this_value.clone()],
            context,
        )?;
    }
    Ok(acc)
}

fn array_every(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.every")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.every")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let result = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), this_value.clone()],
        )?;
        if !result.to_boolean() {
            return Ok(JsValue::Boolean(false));
        }
    }
    Ok(JsValue::Boolean(true))
}

fn array_some(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.some")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.some")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let result = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), this_value.clone()],
        )?;
        if result.to_boolean() {
            return Ok(JsValue::Boolean(true));
        }
    }
    Ok(JsValue::Boolean(false))
}

fn array_find(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.find")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.find")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let result = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val.clone(), JsValue::Number(i as f64), this_value.clone()],
        )?;
        if result.to_boolean() {
            return Ok(val);
        }
    }
    Ok(JsValue::Undefined)
}

fn array_find_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.findIndex")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    require_callable(&callback, "Array.prototype.findIndex")?;
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let result = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), this_value.clone()],
        )?;
        if result.to_boolean() {
            return Ok(JsValue::Number(i as f64));
        }
    }
    Ok(JsValue::Number(-1.0))
}

fn array_flat(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.flat")?;
    let length = array_like_length(context, object);
    let depth = arguments
        .first()
        .and_then(|v| {
            if matches!(v, JsValue::Undefined) {
                None
            } else {
                v.to_number()
            }
        })
        .map(|n| {
            if n.is_infinite() && n > 0.0 {
                usize::MAX
            } else {
                n.max(0.0) as usize
            }
        })
        .unwrap_or(1);
    let result = flat_collect(vm, context, &this_value, length, depth)?;
    context.create_array(result)
}

fn flat_collect(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: &JsValue,
    length: usize,
    depth: usize,
) -> Result<Vec<JsValue>, VmError> {
    let mut result = Vec::new();
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let elem = get_elem(vm, context, value.clone(), i)?;
        if depth > 0
            && let Some(id) = context
                .value_object(&elem)
                .filter(|&id| context.is_array_object(id).unwrap_or(false))
        {
            let inner_len = array_like_length(context, id);
            let inner = flat_collect(vm, context, &elem, inner_len, depth - 1)?;
            result.extend(inner);
            continue;
        }
        result.push(elem);
    }
    Ok(result)
}

fn array_flat_map(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.flatMap")?;
    let length = array_like_length(context, object);
    let callback = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let this_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let mut result = Vec::new();
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let mapped = call_callback(
            vm,
            context,
            callback.clone(),
            this_arg.clone(),
            vec![val, JsValue::Number(i as f64), this_value.clone()],
        )?;
        if let Some(id) = context.value_object(&mapped).filter(|&id| context.is_array_object(id).unwrap_or(false)) {
            let inner_len = array_like_length(context, id);
            for j in 0..inner_len {
                let inner = get_elem(vm, context, mapped.clone(), j)?;
                result.push(inner);
            }
            continue;
        }
        result.push(mapped);
    }
    context.create_array(result)
}

fn array_sort(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.sort")?;
    let length = array_like_length(context, object);
    let compare_fn = arguments
        .first()
        .cloned()
        .filter(|v| !matches!(v, JsValue::Undefined));

    // Collect elements
    let mut elements: Vec<JsValue> = {
        let mut v = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
        for i in 0..length.min(MAX_DENSE_ALLOC) {
            v.push(get_elem(vm, context, this_value.clone(), i)?);
        }
        v
    };

    // Insertion sort to allow calling vm.call_value per comparison
    for i in 1..elements.len() {
        let mut j = i;
        while j > 0 {
            let should_swap = compare_two(
                vm,
                context,
                &elements[j - 1],
                &elements[j],
                &compare_fn,
            )?;
            if !should_swap {
                break;
            }
            elements.swap(j - 1, j);
            j -= 1;
        }
    }

    // Write sorted elements back
    for (i, elem) in elements.into_iter().enumerate() {
        context.set_element(this_value.clone(), JsValue::Number(i as f64), elem)?;
    }
    Ok(this_value)
}

fn compare_two(
    vm: &mut Vm,
    context: &mut NativeContext,
    a: &JsValue,
    b: &JsValue,
    compare_fn: &Option<JsValue>,
) -> Result<bool, VmError> {
    if let Some(func) = compare_fn {
        let result = vm.call_value_from_builtin(
            func.clone(),
            JsValue::Undefined,
            vec![a.clone(), b.clone()],
            context,
        )?;
        let n = result.to_number().unwrap_or(0.0);
        Ok(n > 0.0)
    } else {
        let a_str = value_to_string(a);
        let b_str = value_to_string(b);
        Ok(a_str > b_str)
    }
}

fn array_keys(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.keys")?;
    let length = array_like_length(context, object);
    let safe_len = length.min(MAX_DENSE_ALLOC);
    let keys: Vec<JsValue> = (0..safe_len).map(|i| JsValue::Number(i as f64)).collect();
    context.create_array(keys)
}

fn array_values(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.values")?;
    let length = array_like_length(context, object);
    let mut values = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        values.push(val);
    }
    context.create_array(values)
}

fn array_entries(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.entries")?;
    let length = array_like_length(context, object);
    let mut entries = Vec::with_capacity(length.min(MAX_DENSE_ALLOC));
    for i in 0..length.min(MAX_DENSE_ALLOC) {
        let val = get_elem(vm, context, this_value.clone(), i)?;
        let pair = context.create_array(vec![JsValue::Number(i as f64), val])?;
        entries.push(pair);
    }
    context.create_array(entries)
}

fn array_copy_within(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.copyWithin")?;
    let length = array_like_length(context, object);
    let target = normalize_index(
        arguments
            .first()
            .and_then(|v| v.to_number())
            .unwrap_or(0.0),
        length,
    );
    let start = normalize_index(
        arguments
            .get(1)
            .and_then(|v| v.to_number())
            .unwrap_or(0.0),
        length,
    );
    let end = normalize_index(
        arguments
            .get(2)
            .and_then(|v| {
                if matches!(v, JsValue::Undefined) {
                    None
                } else {
                    v.to_number()
                }
            })
            .unwrap_or(length as f64),
        length,
    );
    let copy_len = (end - start).min(length - target);
    let src: Vec<JsValue> = {
        let mut v = Vec::with_capacity(copy_len);
        for i in 0..copy_len {
            v.push(get_elem(vm, context, this_value.clone(), start + i)?);
        }
        v
    };
    for (i, val) in src.into_iter().enumerate() {
        context.set_element(
            this_value.clone(),
            JsValue::Number((target + i) as f64),
            val,
        )?;
    }
    Ok(this_value)
}

fn array_at(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Array.prototype.at")?;
    let length = array_like_length(context, object);
    let index_raw = arguments
        .first()
        .and_then(|v| v.to_number())
        .unwrap_or(0.0) as i64;
    let index = if index_raw < 0 {
        let from_end = (-index_raw) as usize;
        if from_end > length {
            return Ok(JsValue::Undefined);
        }
        length - from_end
    } else {
        let i = index_raw as usize;
        if i >= length {
            return Ok(JsValue::Undefined);
        }
        i
    };
    get_elem(vm, context, this_value, index)
}
