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
fn regexp_static_escape_and_descriptor_refinements_are_installed() {
    assert_eq!(
        native_eval(
            "var execDesc = Object.getOwnPropertyDescriptor(RegExp.prototype, 'exec'); \
             var flagsDesc = Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags'); \
             typeof RegExp.escape + ':' + RegExp.escape('a+b') + ':' + \
             execDesc.writable + ':' + execDesc.enumerable + ':' + execDesc.configurable + ':' + \
             typeof flagsDesc.get + ':' + flagsDesc.get.call(/a/gi);"
        ),
        "function:\\x61\\+b:true:false:true:function:gi"
    );
}

#[test]
fn regexp_flags_getter_is_generic_and_ordered() {
    assert_eq!(
        native_eval(
            "var get = Object.getOwnPropertyDescriptor(RegExp.prototype, 'flags').get; \
             var calls = ''; \
             var generic = { \
               get hasIndices() { calls += 'd'; return 1; }, \
               get global() { calls += 'g'; return 0; }, \
               get ignoreCase() { calls += 'i'; return true; }, \
               get multiline() { calls += 'm'; return ''; }, \
               get dotAll() { calls += 's'; return {}; }, \
               get unicode() { calls += 'u'; return false; }, \
               get unicodeSets() { calls += 'v'; return 0; }, \
               get sticky() { calls += 'y'; return 'yes'; } \
             }; \
             get.call(generic) + ':' + calls + ':' + \
             get.call(RegExp.prototype) + ':' + \
             Object.getOwnPropertyDescriptor(RegExp.prototype, 'source').get.call(RegExp.prototype);"
        ),
        "disy:dgimsuvy::(?:)"
    );
}

#[test]
fn regexp_exec_and_test_use_to_string_and_last_index() {
    assert_eq!(
        native_eval(
            "var r = /a/g; \
             var first = r.exec('ba'); \
             var afterFirst = r.lastIndex; \
             var second = r.exec('ba'); \
             var afterSecond = r.lastIndex; \
             first.index + ':' + first[0] + ':' + afterFirst + ':' + \
             (second === null) + ':' + afterSecond + ':' + /1/.test(123);"
        ),
        "1:a:2:true:0:true"
    );
}

#[test]
fn regexp_constructor_compile_and_error_shape_are_basic() {
    assert_eq!(
        native_eval(
            "var duplicate = false; \
             try { RegExp('a', 'gg'); } catch (e) { duplicate = e.name === 'SyntaxError'; } \
             var r = /a/g; \
             var same = r.compile('b', 'i'); \
             (same === r) + ':' + r.test('B') + ':' + r.flags + ':' + r.lastIndex + ':' + duplicate;"
        ),
        "true:true:i:0:true"
    );
}

#[test]
fn string_regexp_symbol_dispatch_uses_refined_regexp_methods() {
    assert_eq!(
        native_eval(
            "var r = /a/g; \
             r.lastIndex = 1; \
             var search = 'ba'.search(r); \
             var restored = r.lastIndex; \
             'aba'.match(/a/g).join(',') + ':' + search + ':' + restored + ':' + \
             'a-b'.split(/-/).join('|');"
        ),
        "a,a:1:1:a|b"
    );
}

#[test]
fn annex_b_globals_and_legacy_object_accessors_are_available() {
    assert_eq!(
        native_eval(
            "var o = {}; \
             o.__defineGetter__('x', function () { return 7; }); \
             var getterType = typeof o.__lookupGetter__('x'); \
             var proto = { z: 3 }; \
             o.__proto__ = proto; \
             escape('A B✓') + ':' + unescape('%41%20%u2713') + ':' + \
             o.x + ':' + getterType + ':' + o.z;"
        ),
        "A%20B%u2713:A ✓:7:function:3"
    );
}

#[test]
fn annex_b_string_trim_aliases_are_available() {
    assert_eq!(
        native_eval("'  x  '.trimLeft() + ':' + '  x  '.trimRight();"),
        "x  :  x"
    );
}
