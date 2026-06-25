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
