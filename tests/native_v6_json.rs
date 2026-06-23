use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native JSON evaluation failed for `{source}`: {error}"))
        .value
}

#[test]
fn parses_json_primitives_arrays_and_objects() {
    assert_eq!(native_eval("JSON.parse('null')"), "null");
    assert_eq!(native_eval("JSON.parse('true')"), "true");
    assert_eq!(native_eval("JSON.parse('12.5')"), "12.5");
    assert_eq!(native_eval("JSON.parse('[1,2,3]')[1]"), "2");
    assert_eq!(native_eval("JSON.parse('{\"x\":7}').x"), "7");
}

#[test]
fn parses_json_escapes_and_unicode() {
    assert_eq!(native_eval("JSON.parse('\"a\\\\nb\"')"), "a\nb");
    assert_eq!(native_eval("JSON.parse('\"\\\\u0041\"')"), "A");
}

#[test]
fn parse_reviver_walks_children_and_root() {
    assert_eq!(
        native_eval(
            "JSON.parse('{\"x\":2}', function (key, value) { \
                if (key === 'x') return value * 3; \
                return value; \
             }).x"
        ),
        "6"
    );
}

#[test]
fn rejects_malformed_json_with_a_catchable_error() {
    assert_eq!(
        native_eval("var r = 0; try { JSON.parse('{]'); } catch (e) { r = 1; } r;"),
        "1"
    );
}

#[test]
fn stringifies_primitives_arrays_and_objects() {
    assert_eq!(native_eval("JSON.stringify(null)"), "null");
    assert_eq!(native_eval("JSON.stringify('a\\nb')"), "\"a\\nb\"");
    assert_eq!(
        native_eval("JSON.stringify([1, undefined, 3])"),
        "[1,null,3]"
    );
    assert_eq!(
        native_eval("JSON.stringify({ a: 1, b: true })"),
        "{\"a\":1,\"b\":true}"
    );
}

#[test]
fn stringify_supports_replacer_space_and_to_json() {
    assert_eq!(
        native_eval(
            "JSON.stringify({ a: 1, b: 2 }, function (key, value) { \
                if (key === 'b') return undefined; \
                return value; \
             })"
        ),
        "{\"a\":1}"
    );
    assert_eq!(
        native_eval("JSON.stringify({ a: 1 }, null, 2)"),
        "{\n  \"a\": 1\n}"
    );
    assert_eq!(
        native_eval("JSON.stringify({ toJSON: function () { return 7; } })"),
        "7"
    );
}

#[test]
fn stringify_omits_unsupported_object_values_and_rejects_cycles() {
    assert_eq!(
        native_eval("JSON.stringify({ a: undefined, b: 2 })"),
        "{\"b\":2}"
    );
    assert_eq!(
        native_eval(
            "var a = {}; a.self = a; var r = 0; try { JSON.stringify(a); } catch (e) { r = 1; } r;"
        ),
        "1"
    );
}
