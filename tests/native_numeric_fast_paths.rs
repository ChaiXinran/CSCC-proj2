use agentjs::{
    backend::BackendKind,
    engine::{Engine, ExecutionOptions, RuntimeConfig},
};

fn native_eval(source: &str) -> String {
    Engine::with_backend(BackendKind::Native, RuntimeConfig::default())
        .execute(source, ExecutionOptions::default())
        .unwrap_or_else(|error| panic!("native eval failed for {source:?}: {error}"))
        .value
}

#[test]
fn number_arithmetic_fast_paths_preserve_ieee_results() {
    assert_eq!(native_eval("1 + 2"), "3");
    assert_eq!(native_eval("7 - 9"), "-2");
    assert_eq!(native_eval("6 * 7"), "42");
    assert_eq!(native_eval("7 / 2"), "3.5");
    assert_eq!(native_eval("7 % 4"), "3");
    assert_eq!(native_eval("0 / 0"), "NaN");
    assert_eq!(native_eval("1 / (-0 * 1)"), "-Infinity");
}

#[test]
fn exponentiation_number_fast_path_preserves_ieee_results() {
    assert_eq!(native_eval("2 ** 10"), "1024");
    assert_eq!(native_eval("(-2) ** 3"), "-8");
    assert_eq!(native_eval("NaN ** 2"), "NaN");
    assert_eq!(native_eval("1 / ((-0) ** 3)"), "-Infinity");
}

#[test]
fn bitwise_fast_paths_apply_ecmascript_int32_modulo() {
    assert_eq!(native_eval("1e30 | 0"), "0");
    assert_eq!(native_eval("1e30 >>> 0"), "0");
    assert_eq!(native_eval("4294967295 | 0"), "-1");
    assert_eq!(native_eval("4294967296 | 0"), "0");
    assert_eq!(native_eval("(-1) >>> 0"), "4294967295");
    assert_eq!(native_eval("5 << 33"), "10");
    assert_eq!(native_eval("~1e30"), "-1");
}

#[test]
fn relational_fast_paths_preserve_nan_and_zero_semantics() {
    assert_eq!(native_eval("NaN < 1"), "false");
    assert_eq!(native_eval("NaN >= 1"), "false");
    assert_eq!(native_eval("-0 <= 0"), "true");
    assert_eq!(native_eval("2 < 10"), "true");
    assert_eq!(native_eval("10 > 2"), "true");
}

#[test]
fn non_number_operands_still_use_generic_coercion_paths() {
    assert_eq!(native_eval("'1' + 2"), "12");
    assert_eq!(native_eval("true - false"), "1");
    assert_eq!(native_eval("'2' < '10'"), "false");
    assert_eq!(
        native_eval(
            "var calls = 0; \
             var value = { valueOf: function () { calls++; return 6; } }; \
             (value * 7) + ':' + calls;"
        ),
        "42:1"
    );
    assert_eq!(
        native_eval(
            "var calls = 0; \
             var value = { valueOf: function () { calls++; return 6; } }; \
             (value < 7) + ':' + calls;"
        ),
        "true:1"
    );
    assert_eq!(
        native_eval(
            "var calls = 0; \
             var value = { valueOf: function () { calls++; return 2; } }; \
             (value ** 5) + ':' + calls;"
        ),
        "32:1"
    );
}

#[test]
fn bigint_operations_bypass_number_fast_paths() {
    assert_eq!(native_eval("(1n + 2n).toString()"), "3");
    assert_eq!(native_eval("(6n & 3n).toString()"), "2");
    assert_eq!(native_eval("(1n << 5n).toString()"), "32");
    assert_eq!(native_eval("(2n ** 5n).toString()"), "32");
    assert_eq!(
        native_eval(
            "var result = 'no'; try { 1n + 1; } catch (error) { result = 'caught'; } result;"
        ),
        "caught"
    );
}
