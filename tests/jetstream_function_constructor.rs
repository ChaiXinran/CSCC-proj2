use agentjs::{
    backend::NativeRuntime,
    engine::{ExecutionOptions, RuntimeConfig},
};

fn eval(source: &str) -> String {
    let mut runtime = NativeRuntime::new(RuntimeConfig::default());
    runtime
        .eval_source(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
}

fn eval_with_test262_host(source: &str) -> String {
    let mut runtime = NativeRuntime::new(RuntimeConfig {
        install_test262_host: true,
        ..RuntimeConfig::default()
    });
    runtime
        .eval_source(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
}

#[test]
fn dynamic_function_supports_call_and_construct_forms() {
    assert_eq!(
        eval(
            "Function('a', 'b', 'return a + b')(1, 2) + ':' +
             new Function('a', 'return a * 2')(3) + ':' +
             Function()();"
        ),
        "3:6:undefined"
    );
}

#[test]
fn dynamic_function_builds_parameters_in_order() {
    assert_eq!(
        eval(
            "var order = [];
             function part(label, text) {
               return { toString: function () { order.push(label); return text; } };
             }
             var fn = Function(
               part('first', 'a'),
               part('second', 'b'),
               part('body', 'return a + b')
             );
             order.join(',') + ':' + fn('x', 'y') + ':' + fn.length;"
        ),
        "first,second,body:xy:2"
    );
}

#[test]
fn dynamic_function_reports_parameter_and_body_syntax_errors() {
    assert_eq!(
        eval(
            "function errorName(thunk) {
               try { thunk(); } catch (error) { return error.name; }
             }
             errorName(function () { Function('a b', 'return 1'); }) + ':' +
             errorName(function () { Function('a', 'return )'); });"
        ),
        "SyntaxError:SyntaxError"
    );
}

#[test]
fn dynamic_function_uses_global_scope_and_honors_strict_directive() {
    assert_eq!(
        eval(
            "var globalMarker = 41;
             function make() {
               var localMarker = 1;
               return Function(
                 'return typeof localMarker + \":\" + globalMarker + \":\" + (this === globalThis)'
               );
             }
             var sloppy = make();
             var strict = Function('\"use strict\"; return this');
             sloppy() + ':' + (strict() === undefined);"
        ),
        "undefined:41:true:true"
    );
}

#[test]
fn dynamic_function_observes_finalized_global_builtins() {
    assert_eq!(
        eval("Function('return typeof RegExp.escape')()"),
        "function"
    );
}

#[test]
fn dynamic_function_has_expected_metadata_and_is_constructable() {
    assert_eq!(
        eval(
            "var fn = Function('a', 'b = 1', 'c', 'this.total = a + b + c');
             var value = new fn(1, 2, 3);
             var descriptor = Object.getOwnPropertyDescriptor(fn, 'prototype');
             fn.name + ':' + fn.length + ':' +
             fn.hasOwnProperty('prototype') + ':' +
             descriptor.writable + ':' + descriptor.enumerable + ':' + descriptor.configurable + ':' +
             (value instanceof fn) + ':' + value.total;"
        ),
        "anonymous:1:true:true:false:false:true:6"
    );
}

#[test]
fn dynamic_function_works_with_call_apply_and_bind() {
    assert_eq!(
        eval(
            "var fn = Function('a', 'b', 'return this.base + a + b');
             fn.call({base: 1}, 2, 3) + ':' +
             fn.apply({base: 4}, [5, 6]) + ':' +
             fn.bind({base: 7}, 8)(9);"
        ),
        "6:15:24"
    );
}

#[test]
fn dynamic_function_observes_new_target() {
    assert_eq!(
        eval(
            "var fn = Function('return new.target');
             (fn() === undefined) + ':' + (new fn() === fn);"
        ),
        "true:true"
    );
}

#[test]
fn reflect_construct_uses_new_target_prototype_for_created_function() {
    assert_eq!(
        eval(
            "function NewTarget() {}
             var custom = {};
             NewTarget.prototype = custom;
             var fn = Reflect.construct(Function, ['return 7'], NewTarget);
             (Object.getPrototypeOf(fn) === custom) + ':' + fn();"
        ),
        "true:7"
    );
}

#[test]
fn reflect_construct_uses_new_target_realm_fallback_and_constructor_realm_prototype() {
    assert_eq!(
        eval_with_test262_host(
            "var realmA = $262.createRealm().global;
             realmA.calls = 0;
             var realmB = $262.createRealm().global;
             var newTarget = new realmB.Function();
             newTarget.prototype = null;
             var fn = Reflect.construct(realmA.Function, ['calls += 1;'], newTarget);
             (Object.getPrototypeOf(fn) === realmB.Function.prototype) + ':' +
             (Object.getPrototypeOf(fn.prototype) === realmA.Object.prototype) + ':' +
             (realmA.calls === 0);"
        ),
        "true:true:true"
    );
}

#[test]
fn function_subclass_instances_use_the_subclass_prototype() {
    assert_eq!(
        eval(
            "class DerivedFunction extends Function {}
             var fn = new DerivedFunction('a', 'return a + 1');
             (Object.getPrototypeOf(fn) === DerivedFunction.prototype) + ':' +
             (fn instanceof DerivedFunction) + ':' + fn(2);"
        ),
        "true:true:3"
    );
}

#[test]
fn global_object_writes_update_global_var_bindings() {
    assert_eq!(
        eval(
            "var marker = { value: 1 };
             globalThis.marker = { value: 2 };
             marker.value + ':' + Function('return marker.value')();"
        ),
        "2:2"
    );
}

#[test]
fn dynamic_function_resolves_properties_added_to_the_global_object() {
    assert_eq!(
        eval(
            "var fn = Function(
                 '(function (global, factory) { global.DynamicExport = factory(); })' +
                 '(globalThis, function () { return { value: 7 }; });' +
                 'return DynamicExport.value;'
             );
             fn();"
        ),
        "7"
    );
}

#[test]
fn dynamic_function_accepts_asi_return_before_a_closing_brace() {
    assert_eq!(
        eval(
            "Function(
                 'function nested() { if (true) return} return nested();'
             )() === undefined;"
        ),
        "true"
    );
}

#[test]
fn global_object_inherits_object_prototype_bindings() {
    assert_eq!(
        eval(
            "(Object.getPrototypeOf(globalThis) === Object.prototype) + ':' +
             (toString === Object.prototype.toString) + ':' +
             Function('return toString === Object.prototype.toString')();"
        ),
        "true:true:true"
    );
}

#[test]
fn object_to_string_identifies_regexp_instances() {
    assert_eq!(
        eval(
            "Object.prototype.toString.call(/x/) + ':' +
             Object.prototype.toString.call(new RegExp('x'));"
        ),
        "[object RegExp]:[object RegExp]"
    );
}

#[test]
fn regexp_backreferences_work_through_test_and_dynamic_function() {
    assert_eq!(
        eval(
            "var direct = /^(ab)\\1$/.test('abab');
             var dynamic = Function(
                 'return new RegExp(\"^(ab)\\\\\\\\1$\").test(\"abab\")'
             )();
             direct + ':' + dynamic;"
        ),
        "true:true"
    );
}

#[test]
fn in_operator_coerces_object_property_keys() {
    assert_eq!(
        eval(
            "var calls = 0;
             var key = { toString() { calls += 1; return 'present'; } };
             (key in { present: 1 }) + ':' + calls;"
        ),
        "true:1"
    );
}

#[test]
fn date_parse_accepts_date_to_string_and_utc_string_output() {
    assert_eq!(
        eval(
            "var date = new Date(2011, 8, 10);
             (Date.parse(date.toString()) === date.getTime()) + ':' +
             (Date.parse(date.toUTCString()) === date.getTime()) + ':' +
             (new Date(date.toString()).getTime() === date.getTime());"
        ),
        "true:true:true"
    );
}
