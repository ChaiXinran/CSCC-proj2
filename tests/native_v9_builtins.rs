use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn collection_and_iterator_globals_are_installed() {
    assert_eq!(
        native_eval(
            "typeof Map + ':' + typeof Set + ':' + typeof WeakMap + ':' + \
             typeof WeakSet + ':' + typeof Iterator + ':' + \
             Object.getOwnPropertyDescriptor(Map, 'prototype').writable + ':' + \
             (Object.getOwnPropertyDescriptor(Map, Symbol.species).get.call(Set) === Set) + ':' + \
             (Set.prototype.keys === Set.prototype.values) + ':' + \
             Object.prototype.toString.call(new Map()) + ':' + \
             Object.prototype.toString.call(new Set());"
        ),
        "function:function:function:function:function:false:true:true:[object Map]:[object Set]"
    );
}

#[test]
fn map_supports_basic_storage_and_same_value_zero_keys() {
    assert_eq!(
        native_eval(
            "var key = {}; \
             var m = new Map([['a', 1], [key, 2], [NaN, 3]]); \
             m.set('a', 4).set(-0, 5); \
             m.size + ':' + m.get('a') + ':' + m.get(key) + ':' + m.get(NaN) + ':' + \
             m.has(0) + ':' + m.delete('a') + ':' + m.has('a') + ':' + m.size;"
        ),
        "4:4:2:3:true:true:false:3"
    );
}

#[test]
fn map_and_set_accept_null_or_undefined_initializers() {
    assert_eq!(
        native_eval("new Map(null).size + ':' + new Set(undefined).size;"),
        "0:0"
    );
}

#[test]
fn map_iterators_preserve_insertion_order() {
    assert_eq!(
        native_eval(
            "var m = new Map(); \
             m.set('a', 1); m.set('b', 2); \
             var i = m.entries(); \
             var a = i.next().value; \
             var b = i.next().value; \
             a[0] + a[1] + ':' + b[0] + b[1] + ':' + i.next().done;"
        ),
        "a1:b2:true"
    );
}

#[test]
fn set_supports_basic_storage_and_iteration() {
    assert_eq!(
        native_eval(
            "var s = new Set([1, 2, 1]); \
             s.add(3); \
             var removed = s.delete(2); \
             var i = s.values(); \
             s.size + ':' + s.has(1) + ':' + removed + ':' + s.has(2) + ':' + \
             i.next().value + ':' + i.next().value + ':' + i.next().done;"
        ),
        "2:true:true:false:1:3:true"
    );
}

#[test]
fn map_and_set_for_each_use_collection_callback_order() {
    assert_eq!(
        native_eval(
            "var out = ''; \
             new Map([['x', 1], ['y', 2]]).forEach(function(value, key) { out = out + key + value; }); \
             new Set(['a', 'b']).forEach(function(value, key) { out = out + key + value; }); \
             out;"
        ),
        "x1y2aabb"
    );
}

#[test]
fn weak_collections_support_object_keys_and_reject_strong_keys() {
    assert_eq!(
        native_eval(
            "var k = {}; var k2 = {}; \
             var wm = new WeakMap([[k, 7]]); wm.set(k2, 8); \
             var ws = new WeakSet([k]); ws.add(k2); \
             var caughtMap = false; var caughtSet = false; \
             try { wm.set('x', 1); } catch (e) { caughtMap = e.name === 'TypeError'; } \
             try { ws.add(1); } catch (e) { caughtSet = e.name === 'TypeError'; } \
             wm.get(k) + ':' + wm.has(k2) + ':' + wm.delete(k) + ':' + wm.has(k) + ':' + \
             ws.has(k) + ':' + ws.delete(k2) + ':' + ws.has(k2) + ':' + caughtMap + ':' + caughtSet;"
        ),
        "7:true:true:false:true:true:false:true:true"
    );
}

#[test]
fn iterator_helpers_cover_eager_collection_iterators() {
    assert_eq!(
        native_eval(
            "var m = new Map([['a', 1], ['b', 2]]); \
             var keys = m.keys().toArray(); \
             var found = m.values().find(function(value) { return value === 2; }); \
             var some = new Set([1, 3]).values().some(function(value) { return value > 2; }); \
             var every = new Set([1, 3]).values().every(function(value) { return value > 0; }); \
             var it = m.keys(); \
             keys.join(',') + ':' + found + ':' + some + ':' + every + ':' + (Iterator.from(it) === it);"
        ),
        "a,b:2:true:true:true"
    );
}

#[test]
fn unsupported_iterator_pipeline_helpers_throw_explicit_type_errors() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { new Set([1]).values().map(function(value) { return value; }); } \
             catch (e) { caught = e.name === 'TypeError'; } \
             caught;"
        ),
        "true"
    );
}
