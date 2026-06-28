//! End-to-end tests for the V6 builtin surface wired through the thin adapter
//! layer (`src/builtins/v6.rs`) into the pure C1/C2 algorithm modules.

use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, FailureKind, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for `{source}`: {error}"))
        .value
}

// ── String.prototype ─────────────────────────────────────────────────────────

#[test]
fn string_character_access_methods() {
    assert_eq!(native_eval("'abc'.charAt(1)"), "b");
    assert_eq!(native_eval("'abc'.charCodeAt(0)"), "97");
    assert_eq!(native_eval("'abc'.at(-1)"), "c");
}

#[test]
fn primitive_string_index_access_uses_array_index_keys() {
    assert_eq!(native_eval("'abc'[1] + ':' + 'abc'['1']"), "b:b");
    assert_eq!(native_eval("'abc'['01']"), "undefined");
    assert_eq!(native_eval("'abc'['4294967295']"), "undefined");
}

#[test]
fn string_search_methods() {
    assert_eq!(native_eval("'banana'.indexOf('a')"), "1");
    assert_eq!(native_eval("'banana'.lastIndexOf('a')"), "5");
    assert_eq!(native_eval("'banana'.includes('nan')"), "true");
    assert_eq!(native_eval("'banana'.startsWith('ban')"), "true");
    assert_eq!(native_eval("'banana'.endsWith('na')"), "true");
}

#[test]
fn string_slice_family_follows_distinct_rules() {
    assert_eq!(native_eval("'abcdef'.slice(-3)"), "def");
    assert_eq!(native_eval("'abcdef'.substring(4, 2)"), "cd");
    assert_eq!(native_eval("'abcdef'.substr(-3, 2)"), "de");
}

#[test]
fn string_transform_methods() {
    assert_eq!(native_eval("'ab'.repeat(3)"), "ababab");
    assert_eq!(native_eval("'  x  '.trim()"), "x");
    assert_eq!(native_eval("'Agent'.toUpperCase()"), "AGENT");
    assert_eq!(native_eval("'Agent'.toLowerCase()"), "agent");
    assert_eq!(native_eval("'x'.padStart(4, 'ab')"), "abax");
    assert_eq!(native_eval("'x'.padEnd(4)"), "x   ");
    assert_eq!(native_eval("'a'.concat('b', 'c')"), "abc");
}

#[test]
fn string_static_constructors() {
    assert_eq!(native_eval("String.fromCharCode(72, 105)"), "Hi");
    assert_eq!(native_eval("String.fromCodePoint(65, 66)"), "AB");
}

#[test]
fn string_repeat_with_negative_count_throws_catchable_range_error() {
    assert_eq!(
        native_eval("var r = 'no throw'; try { 'x'.repeat(-1); } catch (e) { r = 'caught'; } r;"),
        "caught"
    );
}

// ── Number.prototype + statics ───────────────────────────────────────────────

#[test]
fn regexp_replacement_allocation_limit_is_catchable() {
    let error = Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(
            "function puff(x, n) { while (x.length < n) x += x; return x.substring(0, n); } \
             var x = puff('1', 1 << 20); \
             var rep = puff('$1', 1 << 16); \
             x.replace(/(.+)/g, rep);",
            ExecutionOptions::default(),
        )
        .expect_err("huge replacement should be rejected before allocation");
    assert_eq!(error.kind, FailureKind::RuntimeLimit);
}

#[test]
fn regexp_match_rejects_non_writable_last_index() {
    assert_eq!(
        native_eval(
            "var p = /x/g; \
             Object.defineProperty(p, 'lastIndex', { writable: false }); \
             var r = 'no throw'; \
             try { '0x'.match(p); } catch (e) { r = e.name; } \
             r;"
        ),
        "TypeError"
    );
}

#[test]
fn bigint_minimal_runtime_semantics() {
    assert_eq!(native_eval("typeof 1n"), "bigint");
    assert_eq!(native_eval("(1n + 2n) * 3n"), "9");
    assert_eq!(
        native_eval("BigInt.prototype.toString.call(BigInt('0x10'), 2)"),
        "10000"
    );
    assert_eq!(native_eval("BigInt.asIntN(4, 15n)"), "-1");
    assert_eq!(native_eval("Object(7n).valueOf() === 7n"), "true");
    assert_eq!(native_eval("var i = 10n; i++; i"), "11");
}

#[test]
fn bigint_arithmetic_rejects_mixed_number_operands() {
    assert_eq!(
        native_eval("var r = 'no throw'; try { 1n + 1; } catch (e) { r = e.name; } r;"),
        "TypeError"
    );
    assert_eq!(
        native_eval("var r = 'no throw'; try { 1n & 1; } catch (e) { r = e.name; } r;"),
        "TypeError"
    );
}

#[test]
fn bigint_bitwise_and_shift_operations() {
    assert_eq!(native_eval("(6n & 3n).toString()"), "2");
    assert_eq!(native_eval("(4n | 1n).toString()"), "5");
    assert_eq!(native_eval("(7n ^ 3n).toString()"), "4");
    assert_eq!(native_eval("(~0n).toString()"), "-1");
    assert_eq!(native_eval("(1n << 5n).toString()"), "32");
    assert_eq!(native_eval("(32n >> 2n).toString()"), "8");
    assert_eq!(
        native_eval("var r = 'no throw'; try { 1n >>> 0n; } catch (e) { r = e.name; } r;"),
        "TypeError"
    );
}

#[test]
fn bigint_division_by_zero_is_catchable_range_error() {
    assert_eq!(
        native_eval("var r = 'no throw'; try { 1n / 0n; } catch (e) { r = e.name; } r;"),
        "RangeError"
    );
    assert_eq!(
        native_eval("var r = 'no throw'; try { 1n % 0n; } catch (e) { r = e.name; } r;"),
        "RangeError"
    );
}

#[test]
fn number_to_string_supports_radix() {
    assert_eq!(native_eval("(255).toString(16)"), "ff");
    assert_eq!(native_eval("(255).toString()"), "255");
}

#[test]
fn number_formatting_methods() {
    assert_eq!(native_eval("(3.14159).toFixed(2)"), "3.14");
    assert_eq!(native_eval("(0.5).toFixed(0)"), "0");
}

#[test]
fn number_to_string_invalid_radix_throws_catchable_range_error() {
    assert_eq!(
        native_eval("var r = 'no throw'; try { (5).toString(1); } catch (e) { r = 'caught'; } r;"),
        "caught"
    );
}

#[test]
fn number_static_predicates_do_not_coerce() {
    assert_eq!(native_eval("Number.isInteger(5)"), "true");
    assert_eq!(native_eval("Number.isInteger(5.5)"), "false");
    assert_eq!(
        native_eval("Number.isSafeInteger(9007199254740991)"),
        "true"
    );
    // Strings are never integers because Number.isInteger does not coerce.
    assert_eq!(native_eval("Number.isInteger('5')"), "false");
    assert_eq!(native_eval("Number.isFinite(Infinity)"), "false");
}

#[test]
fn number_parsers_match_globals() {
    assert_eq!(native_eval("Number.parseInt('0xff')"), "255");
    assert_eq!(native_eval("Number.parseFloat('3.14abc')"), "3.14");
    assert_eq!(native_eval("parseInt('101', 2)"), "5");
    assert_eq!(native_eval("parseFloat('  2.5  ')"), "2.5");
}

#[test]
fn number_wrapper_valueof_round_trips() {
    assert_eq!(native_eval("new Number(42).valueOf()"), "42");
    assert_eq!(
        native_eval("new Number({ valueOf: function () { return 5; } }).valueOf()"),
        "5"
    );
    assert_eq!(native_eval("typeof new Number(42)"), "object");
}

// ── Boolean ──────────────────────────────────────────────────────────────────

#[test]
fn boolean_coercion_and_prototype() {
    assert_eq!(native_eval("Boolean(0)"), "false");
    assert_eq!(native_eval("Boolean('x')"), "true");
    assert_eq!(native_eval("true.toString()"), "true");
    assert_eq!(native_eval("new Boolean(false).valueOf()"), "false");
}

// ── Math ─────────────────────────────────────────────────────────────────────

#[test]
fn math_constants_and_core_functions() {
    assert_eq!(native_eval("Math.abs(-5)"), "5");
    assert_eq!(native_eval("Math.max(1, 7, 3)"), "7");
    assert_eq!(native_eval("Math.min(1, 7, 3)"), "1");
    assert_eq!(native_eval("Math.sqrt(16)"), "4");
    assert_eq!(native_eval("Math.pow(2, 10)"), "1024");
    assert_eq!(native_eval("Math.floor(3.9)"), "3");
    assert_eq!(native_eval("Math.sign(-3)"), "-1");
    assert_eq!(native_eval("Math.PI > 3.14 && Math.PI < 3.15"), "true");
}

#[test]
fn math_extended_functions() {
    assert_eq!(native_eval("Math.hypot(3, 4)"), "5");
    assert_eq!(native_eval("Math.trunc(4.7)"), "4");
    assert_eq!(native_eval("Math.cbrt(27)"), "3");
    assert_eq!(native_eval("Math.imul(3, 4)"), "12");
    assert_eq!(native_eval("Math.f16round(1.337)"), "1.3369140625");
}

// ── Error hierarchy ──────────────────────────────────────────────────────────

#[test]
fn error_instances_carry_name_and_message() {
    assert_eq!(native_eval("new TypeError('boom').message"), "boom");
    assert_eq!(native_eval("new TypeError('boom').name"), "TypeError");
    assert_eq!(native_eval("new RangeError('oops').name"), "RangeError");
}

#[test]
fn error_to_string_formats_name_and_message() {
    assert_eq!(
        native_eval("new TypeError('boom').toString()"),
        "TypeError: boom"
    );
    assert_eq!(native_eval("new Error('').toString()"), "Error");
    assert_eq!(native_eval("Error('boom').toString()"), "Error: boom");
    assert_eq!(
        native_eval("TypeError('boom').toString()"),
        "TypeError: boom"
    );
    assert_eq!(
        native_eval("Error({ toString: function () { return 'converted'; } }).message"),
        "converted"
    );
}

#[test]
fn error_stack_accessor_and_cause_follow_v6_contract() {
    assert_eq!(native_eval("typeof new Error('boom').stack"), "string");
    assert_eq!(
        native_eval(
            "var e = new Error(); e.stack = 'custom'; \
             Object.prototype.hasOwnProperty.call(e, 'stack') && e.stack;"
        ),
        "custom"
    );
    assert_eq!(native_eval("new Error('boom', { cause: 7 }).cause"), "7");
}

#[test]
fn v4_builtins_use_v6_object_aware_coercion() {
    assert_eq!(native_eval("Object(7).valueOf()"), "7");
    assert_eq!(
        native_eval("[1, 2, 3].slice({ valueOf: function () { return 1; } })[0]"),
        "2"
    );
    assert_eq!(
        native_eval("[1, 2].join({ toString: function () { return '-'; } })"),
        "1-2"
    );
    assert_eq!(
        native_eval("[3, 1, 2].sort(function (a, b) { return a - b; })[0]"),
        "1"
    );
}

#[test]
fn reflect_object_exposes_common_static_methods() {
    assert_eq!(native_eval("typeof Reflect"), "object");
    assert_eq!(
        native_eval("Object.prototype.toString.call(Reflect)"),
        "[object Reflect]"
    );
    assert_eq!(
        native_eval("var o = { a: 1 }; Reflect.set(o, 'b', 2); Reflect.get(o, 'a') + o.b;"),
        "3"
    );
    assert_eq!(native_eval("Reflect.has({ a: 1 }, 'a')"), "true");
    assert_eq!(
        native_eval(
            "var o = {}; Object.defineProperty(o, 'x', { value: 1, writable: false }); Reflect.set(o, 'x', 2);"
        ),
        "false"
    );
    assert_eq!(
        native_eval("var o = { a: 1 }; Reflect.deleteProperty(o, 'a'); Reflect.has(o, 'a');"),
        "false"
    );
}

#[test]
fn object_to_string_reports_function_values() {
    assert_eq!(
        native_eval("Object.prototype.toString.call(function () {})"),
        "[object Function]"
    );
    assert_eq!(
        native_eval("Object.prototype.toString.call(Object)"),
        "[object Function]"
    );
}

#[test]
fn strict_function_this_is_not_replaced_with_global_object() {
    assert_eq!(
        native_eval("function f() { 'use strict'; return this === null; } f.call(null);"),
        "true"
    );
    assert_eq!(
        native_eval(
            "function f() { 'use strict'; return this === undefined; } f.apply(undefined, []);"
        ),
        "true"
    );
}

#[test]
fn strict_function_null_this_property_write_throws() {
    assert_eq!(
        native_eval(
            "function f() { 'use strict'; this.y = 1; } \
             var caught = false; \
             try { f.call(null); } catch (e) { caught = true; } \
             caught;"
        ),
        "true"
    );
}

#[test]
fn reflect_define_property_and_own_keys_work() {
    assert_eq!(
        native_eval(
            "var o = {}; Reflect.defineProperty(o, 'x', { value: 7, enumerable: true }); o.x;"
        ),
        "7"
    );
    assert_eq!(
        native_eval(
            "var o = { a: 1 }; Object.defineProperty(o, 'b', { value: 2 }); Reflect.ownKeys(o).length;"
        ),
        "2"
    );
    assert_eq!(
        native_eval(
            "var proto = {}; var o = {}; Reflect.setPrototypeOf(o, proto); Reflect.getPrototypeOf(o) === proto;"
        ),
        "true"
    );
}

#[test]
fn reflect_apply_and_construct_enter_vm_call_paths() {
    assert_eq!(
        native_eval("Reflect.apply(function (a, b) { return this.x + a + b; }, { x: 1 }, [2, 3]);"),
        "6"
    );
    assert_eq!(
        native_eval("function Box(x) { this.x = x; } Reflect.construct(Box, [7]).x;"),
        "7"
    );
}

#[test]
fn bigint_and_template_literals_parse_through_native_pipeline() {
    assert_eq!(
        native_eval("var r = 'no throw'; try { 1n + 2; } catch (e) { r = e.name; } r;"),
        "TypeError"
    );
    assert_eq!(native_eval("`hello`"), "hello");
    assert_eq!(native_eval("`a\\n`"), "a\n");
}
