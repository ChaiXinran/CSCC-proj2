use agentjs::{
    backend::{BackendKind, NativeRuntime},
    engine::{Engine, ExecutionOptions, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn array_buffer_constructor_and_descriptor_skeletons_are_installed() {
    assert_eq!(
        native_eval(
            "typeof ArrayBuffer + ':' + ArrayBuffer.name + ':' + ArrayBuffer.length + ':' + \
             (ArrayBuffer.prototype.constructor === ArrayBuffer) + ':' + \
             Object.getOwnPropertyDescriptor(ArrayBuffer, 'prototype').writable;"
        ),
        "function:ArrayBuffer:1:true:false"
    );
}

#[test]
fn array_buffer_instances_expose_deterministic_metadata() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(8); \
             b.byteLength + ':' + b.maxByteLength + ':' + b.resizable + ':' + b.detached + ':' + \
             Object.prototype.toString.call(b);"
        ),
        "8:8:false:false:[object ArrayBuffer]"
    );
}

#[test]
fn typed_array_constructors_expose_first_batch_metadata() {
    assert_eq!(
        native_eval(
            "var u = new Uint8Array(4); \
             Float64Array.BYTES_PER_ELEMENT + ':' + Int32Array.BYTES_PER_ELEMENT + ':' + \
             Float32Array.BYTES_PER_ELEMENT + ':' + Int8Array.BYTES_PER_ELEMENT + ':' + \
             u.length + ':' + u.byteLength + ':' + u.byteOffset + ':' + \
             ArrayBuffer.isView(u) + ':' + Object.prototype.toString.call(u);"
        ),
        "8:4:4:1:4:4:0:true:[object Uint8Array]"
    );
}

#[test]
fn typed_array_intrinsic_shape_is_reachable_through_concrete_constructors() {
    assert_eq!(
        native_eval(
            "var TA = Object.getPrototypeOf(Uint8Array); \
             (typeof TypedArray) + ':' + TA.name + ':' + TA.length + ':' + \
             (Object.getPrototypeOf(Uint8Array.prototype) === TA.prototype) + ':' + \
             (Uint8Array.from === TA.from) + ':' + \
             (Object.getOwnPropertyDescriptor(TA, Symbol.species).get.call(Uint8Array) === Uint8Array);"
        ),
        "undefined:TypedArray:0:true:true:true"
    );
}

#[test]
fn data_view_constructor_exposes_skeleton_metadata() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(12); \
             var v = new DataView(b, 4, 6); \
             v.buffer === b ? \
               v.byteOffset + ':' + v.byteLength + ':' + ArrayBuffer.isView(v) + ':' + \
               Object.prototype.toString.call(v) : 'wrong-buffer';"
        ),
        "4:6:true:[object DataView]"
    );
}

#[test]
fn unsupported_typed_array_storage_methods_throw_explicit_type_errors() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { new Uint8Array(2).set([1]); } catch (e) { caught = e.name === 'TypeError'; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn intl_namespace_and_first_batch_constructors_are_installed() {
    assert_eq!(
        native_eval(
            "typeof Intl + ':' + typeof Intl.DateTimeFormat + ':' + typeof Intl.NumberFormat + ':' + \
             typeof Intl.Collator + ':' + Object.prototype.toString.call(Intl);"
        ),
        "object:function:function:function:[object Intl]"
    );
}

#[test]
fn intl_resolved_options_are_deterministic() {
    assert_eq!(
        native_eval(
            "var dtf = new Intl.DateTimeFormat().resolvedOptions(); \
             var nf = Intl.NumberFormat().resolvedOptions(); \
             var co = new Intl.Collator().resolvedOptions(); \
             dtf.locale + ':' + dtf.timeZone + ':' + nf.style + ':' + co.usage;"
        ),
        "en-US:UTC:decimal:sort"
    );
}

#[test]
fn intl_supported_locales_of_returns_supported_subset() {
    assert_eq!(
        native_eval("Intl.NumberFormat.supportedLocalesOf(['en-US', 'fr-FR']).join(',');"),
        "en-US"
    );
}

#[test]
fn test262_host_object_is_available_only_when_requested() {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        install_test262_host: true,
        ..RuntimeConfig::default()
    });

    let result = runtime
        .eval_source(
            "var b = new ArrayBuffer(4); \
             $262.detachArrayBuffer(b); \
             ($262.global === globalThis) + ':' + $262.evalScript('1 + 2') + ':' + b.detached;",
            ExecutionOptions::default(),
        )
        .expect("test262 host helpers should run");

    assert_eq!(result, "true:3:true");
    assert_eq!(native_eval("typeof $262"), "undefined");
}
