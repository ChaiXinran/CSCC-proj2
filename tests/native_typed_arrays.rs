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
fn typed_array_storage_methods_and_index_access_are_live() {
    assert_eq!(
        native_eval(
            "var u = new Uint8Array([1, 2, 3]); \
             u[1] = 260; \
             u.set([9, 8], 1); \
             [u[0], u[1], u[2], u.includes(8), u.indexOf(9), u.slice(1).join('-')].join(':');"
        ),
        "1:9:8:true:1:9-8"
    );
}

#[test]
fn typed_array_default_sort_is_numeric_and_handles_nan_and_negative_zero() {
    assert_eq!(
        native_eval(
            "var u = new Uint8Array([10, 2, 1]); \
             u.sort(); \
             var f = new Float64Array([NaN, 1, -0, 0, -1]); \
             f.sort(); \
             u.join(',') + ':' + f[0] + ':' + (1 / f[1] === -Infinity) + ':' + \
             (1 / f[2] === Infinity) + ':' + f[3] + ':' + (f[4] !== f[4]);"
        ),
        "1,2,10:-1:true:true:1:true"
    );
}

#[test]
fn typed_array_constructor_rejects_excessive_numeric_lengths_before_allocation() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { new Float64Array(0x800000000); } catch (e) { caught = e instanceof RangeError; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn typed_array_set_boxes_primitive_array_like_sources() {
    assert_eq!(
        native_eval(
            "var u = new Uint8Array([1, 2, 3, 4, 5]); \
             u.set('678', 1); \
             var n = new Uint8Array([1, 2, 3]); \
             n.set(0); \
             u.join(',') + ':' + n.join(',');"
        ),
        "1,6,7,8,5:1,2,3"
    );
}

#[test]
fn data_view_reads_and_writes_shared_array_buffer_storage() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(4); \
             var v = new DataView(b); \
             v.setUint16(0, 0x1234, false); \
             var u = new Uint8Array(b); \
             [v.getUint16(0, false), u[0], u[1], new Uint16Array(b)[0]].join(':');"
        ),
        "4660:18:52:13330"
    );
}

#[test]
fn data_view_undefined_byte_length_uses_remaining_buffer() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(6); \
             var v = new DataView(b, 2, undefined); \
             v.byteOffset + ':' + v.byteLength;"
        ),
        "2:4"
    );
}

#[test]
fn data_view_accessors_and_methods_reject_detached_buffers() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(4); \
             var v = new DataView(b); \
             b.transfer(); \
             var lengthThrow = false; \
             var getThrow = false; \
             try { v.byteLength; } catch (e) { lengthThrow = e instanceof TypeError; } \
             try { v.getUint8(99); } catch (e) { getThrow = e instanceof TypeError; } \
             (v.buffer === b) + ':' + lengthThrow + ':' + getThrow;",
        ),
        "true:true:true"
    );
}

#[test]
fn typed_array_constructor_and_from_consume_iterable_sources() {
    assert_eq!(
        native_eval(
            "var src = [1, 2, 3]; \
             var iterable = {}; \
             iterable[Symbol.iterator] = function() { return src[Symbol.iterator](); }; \
             var u = new Uint8Array(iterable); \
             var v = Uint8Array.from(iterable, function(x) { return x + 1; }); \
             u.join(',') + ':' + v.join(',') + ':' + \
             (Array.prototype[Symbol.iterator] === Array.prototype.values);"
        ),
        "1,2,3:2,3,4:true"
    );
}

#[test]
fn array_iterator_objects_are_js_visible_and_live() {
    assert_eq!(
        native_eval(
            "var a = []; \
             var it = a.values(); \
             a.push('x'); \
             var first = it.next(); \
             var second = it.next(); \
             typeof it.next + ':' + first.value + ':' + first.done + ':' + \
             (second.value === undefined) + ':' + second.done;"
        ),
        "function:x:false:true:true"
    );
}

#[test]
fn typed_array_iterator_objects_expose_keys_values_and_entries() {
    assert_eq!(
        native_eval(
            "var u = new Uint8Array([7, 8]); \
             var values = u.values(); \
             var keys = u.keys(); \
             var entries = u.entries(); \
             var firstEntry = entries.next().value; \
             var secondEntry = entries.next().value; \
             values.next().value + ':' + values.next().value + ':' + \
             keys.next().value + ':' + keys.next().value + ':' + \
             firstEntry[0] + ':' + firstEntry[1] + ':' + \
             secondEntry[0] + ':' + secondEntry[1] + ':' + \
             entries.next().done;"
        ),
        "7:8:0:1:0:7:1:8:true"
    );
}

#[test]
fn typed_array_and_array_iterators_share_iterator_prototype() {
    assert_eq!(
        native_eval(
            "var typed = new Uint8Array([1]).values(); \
             var array = [1][Symbol.iterator](); \
             (Object.getPrototypeOf(typed) === Object.getPrototypeOf(array)) + ':' + \
             (typed[Symbol.iterator]() === typed);"
        ),
        "true:true"
    );
}

#[test]
fn typed_array_views_track_resizable_array_buffer_bounds() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(3, { maxByteLength: 5 }); \
             var tracking = new Uint8Array(b); \
             var fixed = new Uint8Array(b, 1, 2); \
             b.resize(5); \
             var grown = tracking.length + ':' + tracking.byteLength + ':' + tracking.byteOffset; \
             b.resize(1); \
             var clipped = fixed.length + ':' + fixed.byteLength + ':' + fixed.byteOffset; \
             grown + ':' + clipped;"
        ),
        "5:5:0:0:0:0"
    );
}

#[test]
fn typed_array_iterators_stay_exhausted_after_resizable_buffer_regrows() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(3, { maxByteLength: 5 }); \
             var u = new Uint8Array(b); \
             u[0] = 11; u[1] = 22; u[2] = 33; \
             var it = u.values(); \
             var first = it.next(); \
             b.resize(0); \
             var exhausted = it.next(); \
             b.resize(5); \
             var stillExhausted = it.next(); \
             first.value + ':' + first.done + ':' + \
             (exhausted.value === undefined) + ':' + exhausted.done + ':' + \
             (stillExhausted.value === undefined) + ':' + stillExhausted.done;"
        ),
        "11:false:true:true:true:true"
    );
}

#[test]
fn fixed_length_typed_array_iterator_throws_after_resizable_buffer_shrinks_out_of_bounds() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(4, { maxByteLength: 4 }); \
             var u = new Uint8Array(b, 0, 4); \
             var it = u.values(); \
             it.next(); it.next(); \
             b.resize(3); \
             var result; \
             try { it.next(); result = 'no-throw'; } catch (e) { result = e instanceof TypeError; } \
             result;"
        ),
        "true"
    );
}

#[test]
fn array_buffer_slice_copies_backing_bytes() {
    assert_eq!(
        native_eval(
            "var u = new Uint8Array([5, 6, 7, 8]); \
             var copy = u.buffer.slice(1, 3); \
             var c = new Uint8Array(copy); \
             c.join(',') + ':' + copy.byteLength;"
        ),
        "6,7:2"
    );
}

#[test]
fn array_buffer_resize_and_transfer_methods_preserve_bytes() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(2, { maxByteLength: 4 }); \
             var u = new Uint8Array(b); \
             u[0] = 11; u[1] = 12; \
             b.resize(4); \
             var moved = b.transferToImmutable(); \
             [b.detached, moved.immutable, moved.resizable, moved.byteLength, \
              new Uint8Array(moved).join(',')].join(':');"
        ),
        "true:true:false:4:11,12,0,0"
    );
}

#[test]
fn data_view_bigint64_methods_round_trip_shared_storage() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(16); \
             var v = new DataView(b); \
             v.setBigInt64(0, -1n, false); \
             v.setBigUint64(8, 0x0102030405060708n, true); \
             var u = new Uint8Array(b); \
             [v.getBigInt64(0, false), \
              v.getBigUint64(8, true).toString(16), \
              u[0], u[8]].join(':');"
        ),
        "-1:102030405060708:255:8"
    );
}

#[test]
fn data_view_float16_methods_round_trip_half_precision_values() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(4); \
             var v = new DataView(b); \
             v.setFloat16(0, 1.5, false); \
             v.setFloat16(2, -0, true); \
             [v.getFloat16(0, false), 1 / v.getFloat16(2, true) === -Infinity].join(':');"
        ),
        "1.5:true"
    );
}

#[test]
fn data_view_to_index_truncates_offsets_and_uses_object_primitives() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(4); \
             var v = new DataView(b); \
             v.setUint8(0, 11); v.setUint8(1, 22); v.setUint8(2, 33); \
             var obj = { valueOf: function() { return 2.9; } }; \
             [v.getUint8(0.9), v.getUint8(1.9), v.getUint8(obj), v.getUint8(NaN)].join(':');"
        ),
        "11:22:33:11"
    );
}

#[test]
fn data_view_set_converts_value_before_range_and_detached_checks() {
    assert_eq!(
        native_eval(
            "var b = new ArrayBuffer(1); \
             var v = new DataView(b); \
             var calls = []; \
             var value = { valueOf: function() { calls.push('value'); return 7; } }; \
             try { v.setInt8(100, value); } catch (e) { calls.push(e.constructor === RangeError); } \
             b.transfer(); \
             try { v.setInt8(0, value); } catch (e) { calls.push(e.constructor === TypeError); } \
             calls.join(':');"
        ),
        "value:true:value:true"
    );
}

#[test]
fn shared_array_buffer_can_back_data_view_storage() {
    assert_eq!(
        native_eval(
            "var b = new SharedArrayBuffer(4); \
             var v1 = new DataView(b); \
             var v2 = new DataView(b); \
             v1.setUint8(0, 77); \
             [b.byteLength, v2.getUint8(0), v1.buffer === b, Object.prototype.toString.call(b)].join(':');"
        ),
        "4:77:true:[object SharedArrayBuffer]"
    );
}

#[test]
fn bigint_typed_array_storage_uses_bigint_values() {
    assert_eq!(
        native_eval(
            "var a = new BigInt64Array(2); \
             a[0] = -1n; \
             a[1] = 0x0102030405060708n; \
             var b = new BigUint64Array(a.buffer); \
             [a[0], a[1].toString(16), b[0].toString(16), typeof a[0]].join(':');"
        ),
        "-1:102030405060708:ffffffffffffffff:bigint"
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
             ($262.global === globalThis) + ':' + \
             ($262.createRealm().global === globalThis) + ':' + \
             $262.evalScript('1 + 2') + ':' + b.detached;",
            ExecutionOptions::default(),
        )
        .expect("test262 host helpers should run");

    assert_eq!(result, "true:true:3:true");
    assert_eq!(native_eval("typeof $262"), "undefined");
}

#[test]
fn test262_assert_harness_keeps_builtin_constructors_constructable() {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        install_test262_host: true,
        ..RuntimeConfig::default()
    });

    runtime
        .eval_source(
            include_str!("../test262/harness/assert.js"),
            ExecutionOptions::default(),
        )
        .expect("assert.js should evaluate");

    let result = runtime
        .eval_source(
            "(new Array() instanceof Array) + ':' + (({}) instanceof Object)",
            ExecutionOptions::default(),
        )
        .expect("builtin constructors should remain constructable");

    assert_eq!(result, "true:true");

    runtime
        .eval_source(
            include_str!("../test262/test/language/expressions/array/S11.1.4_A2.js"),
            ExecutionOptions::default(),
        )
        .expect("array literal instanceof Test262 case should pass");
    runtime
        .eval_source(
            include_str!("../test262/test/language/expressions/instanceof/S11.8.6_A2.1_T1.js"),
            ExecutionOptions::default(),
        )
        .expect("Object instanceof Test262 case should pass");
}
