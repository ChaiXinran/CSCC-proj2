use agentjs::{BackendKind, ExecutionOptions, Runtime, RuntimeConfig};

fn native_eval(source: &str) -> String {
    Runtime::with_backend(BackendKind::Native, RuntimeConfig::default())
        .expect("native runtime should initialize")
        .eval(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn proxy_constructor_creates_proxy_objects() {
    assert_eq!(
        native_eval("var p = new Proxy({}, {}); typeof Proxy + ':' + typeof p;"),
        "function:object"
    );
}

#[test]
fn array_is_array_recurses_through_proxy_targets() {
    assert_eq!(
        native_eval(
            "var objectProxy = new Proxy({}, {}); \
             var arrayProxy = new Proxy([], {}); \
             var arrayProxyProxy = new Proxy(arrayProxy, {}); \
             var handle = Proxy.revocable([], {}); \
             handle.revoke(); \
             var revokedThrows = false; \
             try { Array.isArray(handle.proxy); } catch (e) { revokedThrows = e.name === 'TypeError'; } \
             Array.isArray(objectProxy) + ':' + Array.isArray(arrayProxy) + ':' + \
             Array.isArray(arrayProxyProxy) + ':' + revokedThrows;"
        ),
        "false:true:true:true"
    );
}

#[test]
fn reflect_proxy_traps_preserve_abrupt_completions() {
    assert_eq!(
        native_eval(
            "function catches(fn) { \
               try { fn(); } catch (e) { return e === 'trap'; } \
               return false; \
             } \
             var results = []; \
             results.push(catches(function () { \
               Reflect.defineProperty(new Proxy({}, { defineProperty: function () { throw 'trap'; } }), 'x', {}); \
             })); \
             results.push(catches(function () { \
               Reflect.deleteProperty(new Proxy({}, { deleteProperty: function () { throw 'trap'; } }), 'x'); \
             })); \
             results.push(catches(function () { \
               Reflect.get(new Proxy({}, { get: function () { throw 'trap'; } }), 'x'); \
             })); \
             results.push(catches(function () { \
               Reflect.getOwnPropertyDescriptor(new Proxy({}, { getOwnPropertyDescriptor: function () { throw 'trap'; } }), 'x'); \
             })); \
             results.push(catches(function () { \
               Reflect.getPrototypeOf(new Proxy({}, { getPrototypeOf: function () { throw 'trap'; } })); \
             })); \
             results.push(catches(function () { \
               Reflect.has(new Proxy({}, { has: function () { throw 'trap'; } }), 'x'); \
             })); \
             results.push(catches(function () { \
               Reflect.isExtensible(new Proxy({}, { isExtensible: function () { throw 'trap'; } })); \
             })); \
             results.push(catches(function () { \
               Reflect.ownKeys(new Proxy({}, { ownKeys: function () { throw 'trap'; } })); \
             })); \
             results.push(catches(function () { \
               Reflect.preventExtensions(new Proxy({}, { preventExtensions: function () { throw 'trap'; } })); \
             })); \
             results.push(catches(function () { \
               Reflect.set(new Proxy({}, { set: function () { throw 'trap'; } }), 'x', 1); \
             })); \
             results.push(catches(function () { \
               Reflect.setPrototypeOf(new Proxy({}, { setPrototypeOf: function () { throw 'trap'; } }), null); \
             })); \
             results.join('|');"
        ),
        "true|true|true|true|true|true|true|true|true|true|true"
    );
}

#[test]
fn proxy_prevent_extensions_returns_boolean_and_checks_target_state() {
    assert_eq!(
        native_eval(
            "var p1 = new Proxy({}, { preventExtensions: function () { return false; } }); \
             var target = {}; \
             var p2 = new Proxy(target, { \
               preventExtensions: function (target) { Object.preventExtensions(target); return true; } \
             }); \
             Reflect.preventExtensions(p1) + ':' + Reflect.preventExtensions(p2) + ':' + Reflect.isExtensible(target);"
        ),
        "false:true:false"
    );
}

#[test]
fn object_assign_skips_proxy_keys_without_descriptors() {
    assert_eq!(
        native_eval(
            "var calls = 0; \
             var source = new Proxy({}, { ownKeys: function () { calls += 1; return ['missing']; } }); \
             var target = {}; \
             var result = Object.assign(target, source); \
             calls + ':' + ('missing' in target) + ':' + (result === target);"
        ),
        "1:false:true"
    );
}

#[test]
fn object_assign_propagates_proxy_key_and_descriptor_errors() {
    assert_eq!(
        native_eval(
            "function catches(fn) { \
               try { fn(); } catch (e) { return e === 'trap'; } \
               return false; \
             } \
             var ownKeysError = catches(function () { \
               Object.assign({}, new Proxy({}, { ownKeys: function () { throw 'trap'; } })); \
             }); \
             var descriptorError = catches(function () { \
               Object.assign({}, new Proxy({ x: 1 }, { getOwnPropertyDescriptor: function () { throw 'trap'; } })); \
             }); \
             ownKeysError + ':' + descriptorError;"
        ),
        "true:true"
    );
}

#[test]
fn object_assign_preserves_proxy_string_and_symbol_order() {
    assert_eq!(
        native_eval(
            "var sym = Symbol('s'); \
             var ownKeysResult = [sym, 'foo', '0']; \
             var seen = []; \
             var proxy = new Proxy({}, { \
               ownKeys: function () { return ownKeysResult; }, \
               getOwnPropertyDescriptor: function (_target, key) { \
                 seen.push(key); \
                 return { value: 1, enumerable: true, configurable: true }; \
               }, \
               get: function (_target, key) { return key === sym ? 'sym' : key; } \
             }); \
             var target = {}; \
             Object.assign(target, proxy); \
             (seen[0] === sym) + ':' + seen[1] + ':' + seen[2] + ':' + target.foo + ':' + target[0] + ':' + target[sym];"
        ),
        "true:foo:0:foo:0:sym"
    );
}
