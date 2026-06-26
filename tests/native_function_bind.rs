use agentjs::{BackendKind, ExecutionOptions, Runtime, RuntimeConfig};

fn native_eval(source: &str) -> String {
    Runtime::with_backend(BackendKind::Native, RuntimeConfig::default())
        .expect("native runtime should initialize")
        .eval(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

fn native_eval_strict(source: &str) -> String {
    Runtime::with_backend(BackendKind::Native, RuntimeConfig::default())
        .expect("native runtime should initialize")
        .eval(
            source,
            ExecutionOptions {
                strict: true,
                drain_jobs: true,
                ..ExecutionOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("native strict eval failed for `{source}`: {error}"))
        .value
}

fn native_eval_err(source: &str) -> String {
    Runtime::with_backend(BackendKind::Native, RuntimeConfig::default())
        .expect("native runtime should initialize")
        .eval(source, ExecutionOptions::default())
        .expect_err("native eval should fail")
        .to_string()
}

#[test]
fn function_apply_spreads_array_like_arguments() {
    assert_eq!(
        native_eval(
            "function add(a, b) { return this.base + a + b; } \
             add.apply({ base: 10 }, [2, 3]);"
        ),
        "15"
    );
    assert_eq!(
        native_eval(
            "function join(a, b) { return a + '-' + b; } \
             join.apply(null, { 0: 'x', 1: 'y', length: 2 });"
        ),
        "x-y"
    );
}

#[test]
fn function_apply_accepts_null_or_undefined_argument_list() {
    assert_eq!(
        native_eval(
            "function count() { return arguments.length; } \
             count.apply(null, null) + count.apply(null, undefined);"
        ),
        "0"
    );
}

#[test]
fn function_apply_rejects_non_object_argument_list_as_catchable_type_error() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             var f = function () {}; \
             try { f.apply(null, 1); } \
             catch (e) { caught = e.constructor === TypeError; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn invalid_call_target_is_catchable_type_error() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { (1)(); } catch (e) { caught = e.constructor === TypeError; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn existing_call_and_bind_paths_still_forward_this_and_arguments() {
    assert_eq!(
        native_eval(
            "function read(a, b) { return this.x + a + b; } \
             var viaCall = read.call({ x: 1 }, 2, 3); \
             var bound = read.bind({ x: 4 }, 5); \
             viaCall + bound(6);"
        ),
        "21"
    );
}

#[test]
fn function_apply_on_non_callable_this_is_catchable_type_error() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { Function.prototype.apply.call(1, null, []); } \
             catch (e) { caught = e.constructor === TypeError; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn function_apply_errors_remain_engine_errors_when_uncaught() {
    assert!(native_eval_err("Function.prototype.apply.call(1, null, []);").contains("TypeError"));
}

#[test]
fn dynamic_function_call_and_construct_compile_native_source() {
    assert_eq!(
        native_eval("Function('a', 'b', 'return a + b;')(2, 3);"),
        "5"
    );
    assert_eq!(
        native_eval("var f = new Function('a', 'b', 'return a * b;'); f(4, 5);"),
        "20"
    );
}

#[test]
fn dynamic_function_exposes_name_length_and_prototype_properties() {
    assert_eq!(
        native_eval(
            "var f = Function('a', 'b', 'return a + b;'); \
             f.name + ':' + f.length + ':' + (f.prototype.constructor === f);"
        ),
        "anonymous:2:true"
    );
}

#[test]
fn function_prototype_exposes_standard_name_length_order() {
    assert_eq!(
        native_eval(
            "var names = Object.getOwnPropertyNames(Function.prototype); \
             var length = Object.getOwnPropertyDescriptor(Function.prototype, 'length'); \
             var name = Object.getOwnPropertyDescriptor(Function.prototype, 'name'); \
             names.indexOf('length') + ':' + names.indexOf('name') + ':' + \
             length.value + ':' + length.writable + ':' + length.enumerable + ':' + length.configurable + ':' + \
             name.value + ':' + name.writable + ':' + name.enumerable + ':' + name.configurable;"
        ),
        "0:1:0:false:false:true::false:false:true"
    );
}

#[test]
fn strict_functions_restrict_caller_and_arguments_properties() {
    assert_eq!(
        native_eval_strict(
            "function f() {} \
             var readCaller = false, writeCaller = false, readArguments = false, writeArguments = false; \
             try { f.caller; } catch (e) { readCaller = e.constructor === TypeError; } \
             try { f.caller = 1; } catch (e) { writeCaller = e.constructor === TypeError; } \
             try { f.arguments; } catch (e) { readArguments = e.constructor === TypeError; } \
             try { f.arguments = 1; } catch (e) { writeArguments = e.constructor === TypeError; } \
             [readCaller, writeCaller, readArguments, writeArguments].join(':');"
        ),
        "true:true:true:true"
    );
}

#[test]
fn dynamic_strict_function_inherits_restricted_properties_without_own_slots() {
    assert_eq!(
        native_eval(
            "var f = Function('\"use strict\";'); \
             var readCaller = false, readArguments = false; \
             try { f.caller; } catch (e) { readCaller = e.constructor === TypeError; } \
             try { f.arguments; } catch (e) { readArguments = e.constructor === TypeError; } \
             [f.hasOwnProperty('caller'), f.hasOwnProperty('arguments'), readCaller, readArguments].join(':');"
        ),
        "false:false:true:true"
    );
}

#[test]
fn dynamic_function_does_not_capture_local_environment() {
    assert_eq!(
        native_eval(
            "var outer = 7; \
             function make() { var outer = 99; return Function('return outer;'); } \
             make()();"
        ),
        "7"
    );
}

#[test]
fn dynamic_function_syntax_errors_are_catchable() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { Function('return ; ; ; var'); } \
             catch (e) { caught = e.constructor === SyntaxError; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn global_eval_executes_string_source_in_current_runtime() {
    assert_eq!(native_eval("eval('1 + 2');"), "3");
    assert_eq!(native_eval("var x = 1; eval('x = x + 4;'); x;"), "5");
}

#[test]
fn global_eval_returns_non_string_inputs_unchanged() {
    assert_eq!(native_eval("eval(42);"), "42");
    assert_eq!(native_eval("eval(undefined);"), "undefined");
}

#[test]
fn global_eval_syntax_errors_are_catchable() {
    assert_eq!(
        native_eval(
            "var caught = false; \
             try { eval('var ;'); } \
             catch (e) { caught = e.constructor === SyntaxError; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn top_level_this_and_global_this_reference_the_global_object() {
    assert_eq!(native_eval("this === globalThis;"), "true");
    assert_eq!(native_eval("var answer = 42; this.answer;"), "42");
}

#[test]
fn sloppy_function_call_boxes_undefined_receiver_to_global_object() {
    assert_eq!(
        native_eval("function f() { return this === globalThis; } f();"),
        "true"
    );
    assert_eq!(
        native_eval("function f() { return this === globalThis; } f.call(null);"),
        "true"
    );
}

#[test]
fn strict_function_call_preserves_undefined_receiver() {
    assert_eq!(
        native_eval_strict("function f() { return this === undefined; } f();"),
        "true"
    );
}

#[test]
fn symbol_has_instance_customizes_instanceof_and_is_catchable() {
    assert_eq!(
        native_eval(
            "var C = {}; \
             Object.defineProperty(C, Symbol.hasInstance, { value: function (value) { return value && value.mark === 1; } }); \
             ({ mark: 1 }) instanceof C;"
        ),
        "true"
    );
    assert_eq!(
        native_eval(
            "var C = {}; \
             Object.defineProperty(C, Symbol.hasInstance, { value: 1 }); \
             var caught = false; \
             try { ({} instanceof C); } \
             catch (e) { caught = e.constructor === TypeError; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn function_has_instance_default_and_bound_instanceof_use_ordinary_path() {
    assert_eq!(
        native_eval(
            "function C() {} \
             var object = new C(); \
             Function.prototype[Symbol.hasInstance].call(C, object);"
        ),
        "true"
    );
    assert_eq!(
        native_eval(
            "function C() {} \
             var object = new C(); \
             var Bound = C.bind(null); \
             object instanceof Bound;"
        ),
        "true"
    );
}
