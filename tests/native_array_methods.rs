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
fn array_callback_methods_accept_primitive_string_receivers() {
    assert_eq!(
        native_eval(
            "Array.prototype.map.call('abc', function(ch, index, object) { \
               return ch + index + typeof object; \
             }).join('|');"
        ),
        "a0object|b1object|c2object"
    );
}

#[test]
fn array_map_preserves_sparse_holes() {
    assert_eq!(
        native_eval(
            "var a = [,,3]; \
             var calls = []; \
             var r = a.map(function(value, index) { calls.push(index); return value * 2; }); \
             calls.join(',') + ':' + (0 in r) + ':' + (1 in r) + ':' + r[2] + ':' + r.length;"
        ),
        "2:false:false:6:3"
    );
}

#[test]
fn array_reduce_uses_inherited_present_elements() {
    assert_eq!(
        native_eval(
            "var a = [,,3]; \
             Object.prototype[0] = 1; \
             var result; \
             try { \
               result = a.reduce(function(acc, value, index) { return acc + value + index; }); \
             } finally { \
               delete Object.prototype[0]; \
             } \
             result;"
        ),
        "6"
    );
}

#[test]
fn array_filter_skips_holes_and_packs_results() {
    assert_eq!(
        native_eval(
            "var a = [,1,,2]; \
             var r = a.filter(function() { return true; }); \
             r.length + ':' + r.join(',');"
        ),
        "2:1,2"
    );
}

#[test]
fn array_every_and_some_skip_holes() {
    assert_eq!(
        native_eval(
            "var count = 0; \
             var every = [,,].every(function() { count++; return false; }); \
             var some = [,,].some(function() { count++; return true; }); \
             every + ':' + some + ':' + count;"
        ),
        "true:false:0"
    );
}

#[test]
fn array_find_methods_are_generic_and_visit_holes() {
    assert_eq!(
        native_eval(
            "var seen = []; \
             var found = Array.prototype.find.call([, 2], function(value, index) { \
               seen.push(index + ':' + String(value)); \
               return value === undefined; \
             }); \
             var stringIndex = Array.prototype.findIndex.call('abc', function(ch) { return ch === 'b'; }); \
             String(found) + ':' + seen.join('|') + ':' + stringIndex;"
        ),
        "undefined:0:undefined:1"
    );
}

#[test]
fn array_find_last_methods_scan_from_the_end() {
    assert_eq!(
        native_eval(
            "var values = [1, 2, 3, 2]; \
             values.findLast(function(value) { return value % 2 === 0; }) + ':' + \
             values.findLastIndex(function(value) { return value < 3; });"
        ),
        "2:3"
    );
}

#[test]
fn array_search_methods_are_generic_and_honor_holes() {
    assert_eq!(
        native_eval(
            "var a = [, NaN, 2]; \
             var idx = Array.prototype.indexOf.call('abc', 'b'); \
             var last = Array.prototype.lastIndexOf.call({0: 'x', 2: 'x', length: 3}, 'x'); \
             var edge = [1, 2, 1].lastIndexOf(1, -3); \
             idx + ':' + last + ':' + edge + ':' + a.indexOf(undefined) + ':' + a.includes(undefined) + ':' + a.includes(NaN);"
        ),
        "1:2:0:-1:true:true"
    );
}

#[test]
fn array_slice_is_generic_and_preserves_holes() {
    assert_eq!(
        native_eval(
            "var sparse = [0,,2,3]; \
             var sliced = sparse.slice(1, 3); \
             var chars = Array.prototype.slice.call('abcd', 1, 3).join(''); \
             sliced.length + ':' + (0 in sliced) + ':' + sliced[1] + ':' + chars;"
        ),
        "2:false:2:bc"
    );
}

#[test]
fn array_slice_defines_own_result_properties() {
    assert_eq!(
        native_eval(
            "var out; \
             Object.defineProperty(Array.prototype, '0', { value: 'blocked', writable: false, configurable: true }); \
             try { \
               out = [1, 2].slice(0, 1); \
             } finally { \
               delete Array.prototype[0]; \
             } \
             out.hasOwnProperty('0') + ':' + out[0];"
        ),
        "true:1"
    );
}
