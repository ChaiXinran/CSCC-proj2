//! Native ECMAScript built-in registration.

mod array;
mod function;
mod json;
mod object;
pub(crate) mod promise;
pub(crate) mod proxy;

// C1/C2 pure algorithm modules. They contain no VM/runtime wiring; the thin
// adapter layer in `std_primitives` bridges them into the runtime.
// `allow(dead_code)` keeps low-level helpers (e.g. `utf16_slice`) available
// without requiring a direct JavaScript-method home for each one.
/// RegExp prototype refinements + ECMAScript Annex B legacy methods.
mod annex_b;
/// ArrayBuffer / DataView / TypedArray constructors + Intl skeleton.
mod binary_data;
#[allow(dead_code)]
mod boolean;
/// Map / Set / WeakMap / WeakSet + iterator infrastructure.
mod collections;
/// Date / Intl / Temporal built-ins.
mod date_intl;
#[allow(dead_code)]
mod error;
#[allow(dead_code)]
mod math;
#[allow(dead_code)]
mod number;
#[allow(dead_code)]
pub(crate) mod regexp;
/// String / Number / Boolean / Math / Error / JSON adapter layer.
mod std_primitives;
#[allow(dead_code)]
mod string;

use crate::{
    runtime::{
        Intrinsics, JsObject, JsValue, NativeCall, NativeConstruct, NativeContext, NativeErrorKind,
        NativeErrorValue, ObjectId, ObjectKind, PrimitiveValue, PropertyDescriptor,
    },
    vm::{Vm, VmError},
};

// ── P1-A: Unified builtin installer helpers ───────────────────────────────────

/// Descriptor attribute flags for builtin properties.
#[derive(Clone, Copy)]
pub struct BuiltinAttrs {
    pub writable: bool,
    pub enumerable: bool,
    pub configurable: bool,
}

impl BuiltinAttrs {
    /// Standard builtin method: writable, non-enumerable, configurable.
    pub const METHOD: Self = Self {
        writable: true,
        enumerable: false,
        configurable: true,
    };
    /// Standard constant: non-writable, non-enumerable, configurable.
    pub const CONSTANT: Self = Self {
        writable: false,
        enumerable: false,
        configurable: true,
    };
}

/// Install a data property on an object with explicit descriptor attributes.
pub fn define_data_property(
    ctx: &mut NativeContext,
    object: ObjectId,
    key: &str,
    value: JsValue,
    attrs: BuiltinAttrs,
) -> Result<(), VmError> {
    ctx.define_own_property(
        object,
        key.into(),
        PropertyDescriptor::data_with(value, attrs.writable, attrs.enumerable, attrs.configurable),
    )?;
    Ok(())
}

/// Install an accessor property (getter/setter) on an object.
pub fn define_accessor_property(
    ctx: &mut NativeContext,
    object: ObjectId,
    key: &str,
    get: Option<JsValue>,
    set: Option<JsValue>,
    enumerable: bool,
    configurable: bool,
) -> Result<(), VmError> {
    ctx.define_own_property(
        object,
        key.into(),
        PropertyDescriptor::accessor(get, set, enumerable, configurable),
    )?;
    Ok(())
}

/// Install a builtin method on `target`. Descriptor: writable, non-enumerable, configurable.
pub fn install_builtin_method(
    ctx: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<JsValue, VmError> {
    let func = ctx.register_builtin(name, length, call, None)?;
    define_data_property(ctx, target, name, func.clone(), BuiltinAttrs::METHOD)?;
    Ok(func)
}

/// Install a builtin function (standalone, not on a prototype) on `target`.
pub fn install_builtin_function(
    ctx: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<JsValue, VmError> {
    install_builtin_method(ctx, target, name, length, call)
}

/// Install a builtin accessor (getter only for now) on `target`.
pub fn install_builtin_accessor(
    ctx: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    getter: Option<NativeCall>,
    _setter_name: Option<&'static str>,
    setter: Option<NativeCall>,
) -> Result<(), VmError> {
    let get = if let Some(call) = getter {
        Some(ctx.register_builtin(getter_name, 0, call, None)?)
    } else {
        None
    };
    let set = if let Some(call) = setter {
        let setter_name_str = _setter_name.unwrap_or(getter_name);
        Some(ctx.register_builtin(setter_name_str, 1, call, None)?)
    } else {
        None
    };
    define_accessor_property(ctx, target, name, get, set, false, true)
}

/// Install a constructor on `namespace` and wire up its `prototype` property.
/// The given `prototype` ObjectId must already exist.
pub fn install_builtin_constructor(
    ctx: &mut NativeContext,
    namespace: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
    construct: Option<NativeConstruct>,
    prototype: ObjectId,
) -> Result<JsValue, VmError> {
    let constructor = ctx.register_builtin(name, length, call, construct)?;
    let constructor_object = ctx
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("constructor object missing"))?;
    ctx.define_own_property(
        constructor_object,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(prototype), false, false, false),
    )?;
    ctx.define_own_property(
        prototype,
        "constructor".into(),
        PropertyDescriptor::data_with(constructor.clone(), true, false, true),
    )?;
    define_data_property(
        ctx,
        namespace,
        name,
        constructor.clone(),
        BuiltinAttrs::METHOD,
    )?;
    Ok(constructor)
}

/// Get own property descriptor (wraps context method).
pub fn get_own_property_descriptor(
    ctx: &NativeContext,
    object: ObjectId,
    key: &str,
) -> Option<PropertyDescriptor> {
    ctx.get_own_property_descriptor(object, key)
}

/// Return property keys in ECMAScript integer-index-first order.
pub fn ordinary_own_property_keys(ctx: &NativeContext, object: ObjectId) -> Vec<String> {
    ctx.heap()
        .object(object)
        .map(|o| o.own_property_keys())
        .unwrap_or_default()
}

/// Mark a builtin function value as non-constructable (no-op: register_builtin with
/// construct=None already does this; this function exists for documentation/interface purposes).
pub fn mark_not_constructor(_ctx: &mut NativeContext, _function: &JsValue) {
    // register_builtin with construct = None already yields [[Construct]]:absent.
}

/// Installs the foundational constructors, prototypes, and V4 methods.
pub fn install_foundation(context: &mut NativeContext) {
    if context.intrinsics().is_some() {
        return;
    }
    install_globals(context).expect("builtin foundation installation must succeed");
    object::install_object(context);
    array::install_array(context);
    function::install_function(context);
    install_std_globals(context).expect("std globals installation must succeed");
}

fn install_globals(context: &mut NativeContext) -> Result<(), VmError> {
    let object_prototype = context
        .heap_mut()
        .allocate_object(JsObject::ordinary())
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut function_prototype_object = JsObject::ordinary();
    function_prototype_object.prototype = Some(object_prototype);
    function_prototype_object.define_property(
        "length",
        PropertyDescriptor::data_with(JsValue::Number(0.0), false, false, true),
    );
    function_prototype_object.define_property(
        "name",
        PropertyDescriptor::data_with(JsValue::String(String::new()), false, false, true),
    );
    let function_prototype = context
        .heap_mut()
        .allocate_object(function_prototype_object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let object_constructor = context.register_builtin(
        "Object",
        1,
        object::object_call,
        Some(object::object_construct),
    )?;
    let JsValue::BuiltinFunction(object_id) = object_constructor else {
        unreachable!()
    };
    let object_backing = context.builtin(object_id).unwrap().object;
    context.set_prototype_of(object_backing, Some(function_prototype))?;
    context.define_own_property(
        object_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(object_prototype), false, false, false),
    )?;
    context.define_own_property(
        object_prototype,
        "constructor".into(),
        PropertyDescriptor::data_with(object_constructor.clone(), true, false, true),
    )?;

    let function_constructor = context.register_builtin(
        "Function",
        1,
        function::function_call,
        Some(function::function_construct),
    )?;
    let JsValue::BuiltinFunction(function_id) = function_constructor else {
        unreachable!()
    };
    let function_backing = context.builtin(function_id).unwrap().object;
    context.set_prototype_of(function_backing, Some(function_prototype))?;
    context.define_own_property(
        function_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(function_prototype), false, false, false),
    )?;
    context.define_own_property(
        function_prototype,
        "constructor".into(),
        PropertyDescriptor::data_with(function_constructor.clone(), true, false, true),
    )?;
    let eval_function = context.register_builtin("eval", 1, function::eval_call, None)?;

    let mut array_prototype_object = JsObject::sparse_array(0);
    array_prototype_object.prototype = Some(object_prototype);
    let array_prototype = context
        .heap_mut()
        .allocate_object(array_prototype_object)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let array_constructor =
        context.register_builtin("Array", 1, array::array_call, Some(array::array_construct))?;
    let JsValue::BuiltinFunction(array_id) = array_constructor else {
        unreachable!()
    };
    let array_backing = context.builtin(array_id).unwrap().object;
    context.set_prototype_of(array_backing, Some(function_prototype))?;
    context.define_own_property(
        array_backing,
        "prototype".into(),
        PropertyDescriptor::data_with(JsValue::Object(array_prototype), false, false, false),
    )?;
    context.define_own_property(
        array_prototype,
        "constructor".into(),
        PropertyDescriptor::data_with(array_constructor.clone(), true, false, true),
    )?;

    // V6: Pre-create primitive wrapper prototypes so builtins can install methods on them.
    // Per ECMAScript: Number.prototype, Boolean.prototype, and String.prototype are themselves
    // wrapper objects (with internal [[NumberData]]/[[BooleanData]]/[[StringData]] set to their
    // default values). Error.prototype is an ordinary object.
    let mut num_proto_obj = JsObject::ordinary();
    num_proto_obj.prototype = Some(object_prototype);
    num_proto_obj.kind = ObjectKind::PrimitiveWrapper(PrimitiveValue::Number(0.0));
    let number_prototype = context
        .heap_mut()
        .allocate_object(num_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut bool_proto_obj = JsObject::ordinary();
    bool_proto_obj.prototype = Some(object_prototype);
    bool_proto_obj.kind = ObjectKind::PrimitiveWrapper(PrimitiveValue::Boolean(false));
    let boolean_prototype = context
        .heap_mut()
        .allocate_object(bool_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut str_proto_obj = JsObject::ordinary();
    str_proto_obj.prototype = Some(object_prototype);
    str_proto_obj.kind = ObjectKind::PrimitiveWrapper(PrimitiveValue::String(String::new()));
    let string_prototype = context
        .heap_mut()
        .allocate_object(str_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut error_proto_obj = JsObject::ordinary();
    error_proto_obj.prototype = Some(object_prototype);
    let error_prototype = context
        .heap_mut()
        .allocate_object(error_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    let mut regexp_proto_obj = JsObject::ordinary();
    regexp_proto_obj.prototype = Some(object_prototype);
    let regexp_prototype = context
        .heap_mut()
        .allocate_object(regexp_proto_obj)
        .ok_or_else(|| VmError::runtime("heap exhausted"))?;

    context.set_intrinsics(Intrinsics {
        object_prototype,
        function_prototype,
        array_prototype,
        object_constructor: object_constructor.clone(),
        function_constructor: function_constructor.clone(),
        array_constructor: array_constructor.clone(),
        string_prototype,
        number_prototype,
        boolean_prototype,
        error_prototype,
        regexp_prototype,
    });
    for (name, value) in [
        ("Object", object_constructor),
        ("Function", function_constructor),
        ("Array", array_constructor),
        ("eval", eval_function),
        ("globalThis", JsValue::Object(context.global_object())),
    ] {
        context.declare_global(name, value.clone());
        context.define_own_property(
            context.global_object(),
            name.into(),
            PropertyDescriptor::data_with(value, true, false, true),
        )?;
    }
    Ok(())
}

/// Installs the minimal Test262 host surface used by the native runtime.
///
/// `Test262Error` and `print` are wired as Rust host functions so that the test
/// runner can detect assertion failures and async completion. All other
/// harness globals (`assert`, `assert.sameValue`, `assert.compareArray`, …) are
/// provided by eval'ing the official `assert.js` at the start of each test case.
/// `sta.js` is intentionally NOT eval'd: it redefines `Test262Error` as a plain
/// JS class which would shadow our Rust host function and break error detection.
pub fn install_test262_harness(context: &mut NativeContext) {
    let test262_error = context
        .register_builtin(
            "Test262Error",
            1,
            test262_error_call,
            Some(test262_error_construct),
        )
        .expect("install Test262Error");
    context.declare_global("Test262Error", test262_error);
    let print = context
        .register_builtin("print", 1, test262_print, None)
        .expect("install Test262 print");
    context.declare_global("print", print);
    binary_data::install_test262_host_object(context);
}

fn test262_print(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut fields = Vec::with_capacity(arguments.len());
    for value in arguments {
        fields.push(vm.to_string_coerce(value.clone(), context)?);
    }
    context.push_output(fields.join(" "));
    Ok(JsValue::Undefined)
}

fn test262_error_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::test262(
        arguments
            .first()
            .and_then(JsValue::to_js_string)
            .unwrap_or_else(|| "Test262Error".into()),
    ))
}

fn test262_error_construct(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    arguments: &[JsValue],
    _new_target: JsValue,
) -> Result<JsValue, VmError> {
    let message = arguments
        .first()
        .and_then(JsValue::to_js_string)
        .unwrap_or_else(|| "Test262Error".into());
    Ok(JsValue::Error(NativeErrorValue::new(
        NativeErrorKind::Test262,
        message,
    )))
}

// ── Standard globals (Error, Number, Boolean, String, Math) ──────────────────

/// Installs the standard-library globals by delegating to the V6 adapter layer,
/// which bridges the pure C1/C2 algorithm modules into the runtime.
fn install_std_globals(context: &mut NativeContext) -> Result<(), VmError> {
    std_primitives::install(context)?;
    proxy::install(context)?;
    binary_data::install(context)?;
    collections::install(context)?;
    promise::install(context)?;
    date_intl::install(context)?;
    annex_b::install(context)
}

#[cfg(test)]
mod tests {
    use crate::{
        builtins::{install_foundation, install_test262_harness},
        runtime::{JsValue, NativeContext},
    };

    #[test]
    fn installs_foundation_and_harness_as_registered_builtins() {
        let mut context = NativeContext::default();
        install_foundation(&mut context);
        install_test262_harness(&mut context);

        assert!(context.intrinsics().is_some());
        assert!(matches!(
            context.get_global("Object"),
            Some(JsValue::BuiltinFunction(_))
        ));
        assert!(matches!(
            context.get_global("Test262Error"),
            Some(JsValue::BuiltinFunction(_))
        ));
    }
}
