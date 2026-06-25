//! A-group acceptance tests for the expanded Native V4 front end.
//!
//! These tests stop at `Program` and do not depend on bytecode, the VM,
//! Runtime, Builtins, or Boa.

use agentjs::{
    NativeError,
    contracts::{NativeFrontend, Program, SourceParser},
};

fn parse(source: &str) -> Result<Program, NativeError> {
    NativeFrontend.parse_source(source)
}

fn parse_ok(source: &str) -> Program {
    parse(source).unwrap_or_else(|error| panic!("V4 front end should parse source: {error}"))
}

#[test]
fn parses_expanded_v4_builtin_call_shapes() {
    let sources = [
        "Object.create(base);",
        "Object.defineProperty(object, 'x', { value: 1, writable: true });",
        "Object.getOwnPropertyDescriptor(object, 'x');",
        "Object.getPrototypeOf(object);",
        "Object.setPrototypeOf(object, prototype);",
        "Object.keys(object,);",
        "Array(1, 2);",
        "new Array(3);",
        "Array.isArray(array);",
        "array.push(1);",
        "array.pop();",
        "Function.prototype.call.call(fn, receiver, 1);",
    ];

    for source in sources {
        assert!(!parse_ok(source).body.is_empty(), "{source}");
    }
}

#[test]
fn parses_v4_object_and_array_forms_without_downstream_stages() {
    let source = "
        var base = { inherited: true };
        var object = {
            __proto__: base,
            get value() { return this.saved; },
            set value(next) { this.saved = next; },
            instanceof: 1,
            delete: 2
        };
        var array = [1, , 3,];
        delete object.delete;
        'inherited' in object;
        object instanceof Constructor;
    ";

    assert!(!parse_ok(source).body.is_empty());
}

#[test]
fn parses_positive_v4_test262_gate_files() {
    let files = [
        include_str!("../test262/test/language/expressions/delete/11.4.1-3-3.js"),
        include_str!("../test262/test/language/expressions/delete/11.4.1-4.a-12.js"),
        include_str!("../test262/test/language/expressions/delete/11.4.1-4.a-14.js"),
        include_str!("../test262/test/language/expressions/delete/11.4.1-4.a-15.js"),
        include_str!("../test262/test/language/expressions/delete/S11.4.1_A3.2_T3.js"),
        include_str!("../test262/test/language/expressions/delete/S11.4.1_A3.3_T3.js"),
        include_str!("../test262/test/language/expressions/delete/S8.12.7_A2_T1.js"),
        include_str!("../test262/test/language/expressions/in/S8.12.6_A1.js"),
        include_str!("../test262/test/language/expressions/in/S8.12.6_A3.js"),
        include_str!("../test262/test/language/expressions/array/11.1.4-0.js"),
    ];

    for source in files {
        assert!(!parse_ok(source).body.is_empty());
    }
}

#[test]
fn rejects_v4_early_errors_without_compiler_or_runtime() {
    for source in [
        "({ get value(argument) {} });",
        "({ set value() {} });",
        "({ set value(first, second) {} });",
        "({ __proto__: first, '__proto__': second });",
    ] {
        assert!(
            matches!(parse(source), Err(NativeError::Parse(_))),
            "expected ParseError for {source}"
        );
    }

    assert!(matches!(
        parse(include_str!(
            "../test262/test/language/expressions/object/__proto__-duplicate.js"
        )),
        Err(NativeError::Parse(_))
    ));
}
