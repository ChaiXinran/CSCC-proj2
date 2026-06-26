use agentjs::{BackendKind, ExecutionOptions, Runtime, RuntimeConfig};

fn native_eval(source: &str) -> String {
    Runtime::with_backend(BackendKind::Native, RuntimeConfig::default())
        .expect("native runtime should initialize")
        .eval(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

#[test]
fn object_keys_treats_u32_max_as_ordinary_string_key() {
    assert_eq!(
        native_eval(
            "var o = {}; \
             o['4294967295'] = 'max'; \
             o['1'] = 'one'; \
             o.a = 'a'; \
             Object.keys(o).join('|');"
        ),
        "1|4294967295|a"
    );
}

#[test]
fn get_own_property_names_orders_array_indices_before_strings() {
    assert_eq!(
        native_eval(
            "var o = {}; \
             o['4294967295'] = 'max'; \
             Object.defineProperty(o, '2', { value: 'two' }); \
             o.b = 'b'; \
             Object.getOwnPropertyNames(o).join('|');"
        ),
        "2|4294967295|b"
    );
}

#[test]
fn reflect_own_keys_keeps_symbols_after_string_keys() {
    assert_eq!(
        native_eval(
            "var s = Symbol('s'); \
             var o = {}; \
             o[s] = 'sym'; \
             o['4294967295'] = 'max'; \
             o['1'] = 'one'; \
             var keys = Reflect.ownKeys(o); \
             keys[0] === '1' && keys[1] === '4294967295' && typeof keys[2] === 'symbol';"
        ),
        "true"
    );
}

#[test]
fn symbol_define_property_obeys_descriptor_invariants() {
    assert_eq!(
        native_eval(
            "var s = Symbol('s'); \
             var o = {}; \
             Object.defineProperty(o, s, { value: 1, writable: false, configurable: false }); \
             var objectThrow = false; \
             try { Object.defineProperty(o, s, { value: 2 }); } \
             catch (e) { objectThrow = e.name === 'TypeError'; } \
             var reflectDefined = Reflect.defineProperty(o, s, { value: 2 }); \
             var objectDesc = Object.getOwnPropertyDescriptor(o, s); \
             var reflectDesc = Reflect.getOwnPropertyDescriptor(o, s); \
             objectThrow + ':' + reflectDefined + ':' + objectDesc.value + ':' + \
             reflectDesc.value + ':' + objectDesc.writable + ':' + reflectDesc.configurable;"
        ),
        "true:false:1:1:false:false"
    );
}

#[test]
fn reflect_symbol_delete_and_has_follow_property_rules() {
    assert_eq!(
        native_eval(
            "var s = Symbol('s'); \
             var proto = {}; \
             Object.defineProperty(proto, s, { value: 1, configurable: true }); \
             var child = Object.create(proto); \
             var inherited = Reflect.has(child, s); \
             Object.defineProperty(child, s, { value: 2, configurable: false }); \
             var deleted = Reflect.deleteProperty(child, s); \
             inherited + ':' + deleted + ':' + Reflect.has(child, s) + ':' + Reflect.get(child, s);"
        ),
        "true:false:true:2"
    );
}

#[test]
fn symbol_accessors_use_the_receiver_for_get_and_set() {
    assert_eq!(
        native_eval(
            "var s = Symbol('s'); \
             var proto = {}; \
             Object.defineProperty(proto, s, { \
               get: function () { return this.marker; }, \
               set: function (value) { this.saved = value; }, \
               configurable: true \
             }); \
             var object = Object.create(proto); \
             object.marker = 4; \
             var got = object[s]; \
             object[s] = 9; \
             var savedAfterAssignment = object.saved; \
             var reflected = Reflect.set(object, s, 11); \
             got + ':' + savedAfterAssignment + ':' + reflected + ':' + object.saved + ':' + Reflect.get(object, s);"
        ),
        "4:9:true:11:4"
    );
}

#[test]
fn computed_symbol_accessors_define_symbol_properties() {
    assert_eq!(
        native_eval(
            "var s = Symbol('s'); \
             var saved = 0; \
             var object = { get [s]() { return 3; }, set [s](value) { saved = value; } }; \
             var desc = Object.getOwnPropertyDescriptor(object, s); \
             object[s] = 5; \
             object[s] + ':' + saved + ':' + desc.enumerable + ':' + desc.configurable;"
        ),
        "3:5:true:true"
    );
}

#[test]
fn reflect_get_and_set_use_explicit_receiver() {
    assert_eq!(
        native_eval(
            "var symbol = Symbol('s'); \
             var getTarget = {}; \
             Object.defineProperty(getTarget, 'p', { get: function () { return this.value; } }); \
             var getReceiver = { value: 42 }; \
             var got = Reflect.get(getTarget, 'p', getReceiver); \
             var dataTarget = {}; \
             var dataReceiver = {}; \
             Object.defineProperty(dataTarget, 'p', { value: 1, writable: true }); \
             var dataSet = Reflect.set(dataTarget, 'p', 2, dataReceiver); \
             var symbolTarget = {}; \
             var symbolReceiver = {}; \
             Object.defineProperty(symbolTarget, symbol, { value: 3, writable: true }); \
             var symbolSet = Reflect.set(symbolTarget, symbol, 4, symbolReceiver); \
             var setterTarget = {}; \
             var setterReceiver = {}; \
             Object.defineProperty(setterTarget, 'q', { set: function (value) { this.saved = value; } }); \
             var accessorSet = Reflect.set(setterTarget, 'q', 5, setterReceiver); \
             got + ':' + dataSet + ':' + dataTarget.p + ':' + dataReceiver.p + ':' + \
             symbolSet + ':' + symbolTarget[symbol] + ':' + symbolReceiver[symbol] + ':' + \
             accessorSet + ':' + setterReceiver.saved;"
        ),
        "42:true:1:2:true:3:4:true:5"
    );
}

#[test]
fn object_static_methods_box_primitive_strings() {
    assert_eq!(
        native_eval(
            "var keys = Object.keys('ab').join('|'); \
             var names = Object.getOwnPropertyNames('ab').join('|'); \
             var assigned = Object.assign({}, 'xy'); \
             keys + ':' + names + ':' + assigned[0] + assigned[1];"
        ),
        "0|1:0|1|length:xy"
    );
}

#[test]
fn object_and_reflect_track_extensibility() {
    assert_eq!(
        native_eval(
            "var o = {}; \
             var before = Object.isExtensible(o); \
             var reflected = Reflect.preventExtensions(o); \
             var after = Reflect.isExtensible(o); \
             var defined = Reflect.defineProperty(o, 'x', { value: 1 }); \
             o.y = 2; \
             before + ':' + reflected + ':' + after + ':' + defined + ':' + ('y' in o);"
        ),
        "true:true:false:false:false"
    );
}

#[test]
fn object_assign_uses_strict_set_and_copies_symbols() {
    assert_eq!(
        native_eval(
            "var s = Symbol('s'); \
             var source = { a: 1 }; \
             source[s] = 2; \
             var target = {}; \
             Object.assign(target, source); \
             Object.preventExtensions(target); \
             var threw = false; \
             try { Object.assign(target, { b: 3 }); } catch (e) { threw = e.name === 'TypeError'; } \
             target.a + ':' + target[s] + ':' + threw;"
        ),
        "1:2:true"
    );
}

#[test]
fn object_assign_calls_existing_accessors_and_propagates_setter_errors() {
    assert_eq!(
        native_eval(
            "var value = 0; \
             var target = Object.preventExtensions({ set x(v) { value = v; } }); \
             Object.assign(target, { x: 7 }); \
             var threw = false; \
             try { Object.assign({ set y(v) { throw 'setter'; } }, { y: 1 }); } \
             catch (e) { threw = e === 'setter'; } \
             value + ':' + threw;"
        ),
        "7:true"
    );
}

#[test]
fn reflect_set_prototype_of_reports_false_for_impossible_changes() {
    assert_eq!(
        native_eval(
            "var target = {}; \
             var proto = {}; \
             Object.preventExtensions(target); \
             var same = Reflect.setPrototypeOf(target, Object.getPrototypeOf(target)); \
             var changed = Reflect.setPrototypeOf(target, proto); \
             same + ':' + changed;"
        ),
        "true:false"
    );
}

#[test]
fn reflect_property_keys_and_descriptors_preserve_abrupt_completions() {
    assert_eq!(
        native_eval(
            "var keyThrows = false; \
             try { Reflect.defineProperty({}, { toString: function() { throw 'key'; } }, {}); } \
             catch (e) { keyThrows = e === 'key'; } \
             var attr = {}; \
             Object.defineProperty(attr, 'enumerable', { get: function() { throw 'attr'; } }); \
             var attrThrows = false; \
             try { Reflect.defineProperty({}, 'x', attr); } \
             catch (e) { attrThrows = e === 'attr'; } \
             keyThrows + ':' + attrThrows;"
        ),
        "true:true"
    );
}

#[test]
fn reflect_construct_uses_new_target_prototype_and_reflect_descriptor_shape() {
    assert_eq!(
        native_eval(
            "function F() { this.marker = Object.getPrototypeOf(this) === Array.prototype; } \
             var result = Reflect.construct(F, [], Array); \
             var desc = Object.getOwnPropertyDescriptor(this, 'Reflect'); \
             var tag = Object.getOwnPropertyDescriptor(Reflect, Symbol.toStringTag); \
             result.marker + ':' + (Object.getPrototypeOf(result) === Array.prototype) + ':' + \
             desc.enumerable + ':' + desc.writable + ':' + desc.configurable + ':' + \
             tag.value + ':' + tag.enumerable + ':' + tag.writable + ':' + tag.configurable;"
        ),
        "true:true:false:true:true:Reflect:false:false:true"
    );
}
