#[path = "../src/builtins/boolean.rs"]
mod boolean;
#[path = "../src/builtins/error.rs"]
mod error;
#[path = "../src/builtins/math.rs"]
mod math;
#[path = "../src/builtins/number.rs"]
mod number;

use boolean::{boolean_call, boolean_to_string, boolean_value_of, BOOLEAN_PROTOTYPE_METHODS};
use error::{constructor_spec, create_error_record, error_to_string, ERROR_CONSTRUCTORS};
use math::{MATH_CONSTANTS, MATH_METHODS};
use number::{
    is_finite, is_integer, is_nan, is_safe_integer, number_call, number_to_string, number_value_of,
    parse_float, parse_int, to_exponential, to_fixed, to_precision, NumberFormatError,
    MAX_SAFE_INTEGER, MIN_SAFE_INTEGER, NUMBER_CONSTANTS, NUMBER_PROTOTYPE_METHODS,
    NUMBER_STATIC_METHODS,
};

#[test]
fn numeric_metadata_matches_the_v6_contract() {
    assert!(NUMBER_CONSTANTS.iter().any(|constant| {
        constant.name == "MAX_SAFE_INTEGER" && constant.value == MAX_SAFE_INTEGER
    }));
    assert!(NUMBER_CONSTANTS.iter().any(|constant| {
        constant.name == "MIN_SAFE_INTEGER" && constant.value == MIN_SAFE_INTEGER
    }));
    assert!(NUMBER_CONSTANTS
        .iter()
        .any(|constant| constant.name == "NaN" && constant.value.is_nan()));
    assert!(NUMBER_STATIC_METHODS.contains(&number::NumberMethodSpec {
        name: "parseInt",
        length: 2,
    }));
    assert!(
        NUMBER_PROTOTYPE_METHODS.contains(&number::NumberMethodSpec {
            name: "toFixed",
            length: 1,
        })
    );
    assert!(
        BOOLEAN_PROTOTYPE_METHODS.contains(&boolean::BooleanMethodSpec {
            name: "valueOf",
            length: 0,
        })
    );
}

#[test]
fn number_predicates_keep_nan_infinity_and_safe_integer_edges_distinct() {
    assert_eq!(number_call(None), 0.0);
    assert_eq!(number_value_of(-3.5), -3.5);
    assert!(is_nan(f64::NAN));
    assert!(!is_nan(0.0));
    assert!(is_finite(1.0));
    assert!(!is_finite(f64::INFINITY));
    assert!(is_integer(-0.0));
    assert!(is_integer(42.0));
    assert!(!is_integer(42.5));
    assert!(is_safe_integer(MAX_SAFE_INTEGER));
    assert!(!is_safe_integer(MAX_SAFE_INTEGER + 2.0));
}

#[test]
fn number_parsing_follows_longest_prefix_and_radix_rules() {
    assert_eq!(parse_int("  -0x10", None), -16.0);
    assert_eq!(parse_int("101", Some(2)), 5.0);
    assert_eq!(parse_int("19px", Some(10)), 19.0);
    assert!(parse_int("z", Some(10)).is_nan());
    assert!(parse_int("10", Some(1)).is_nan());

    assert_eq!(parse_float("  -3.5e2xyz"), -350.0);
    assert_eq!(parse_float("1e-"), 1.0);
    assert_eq!(parse_float("+Infinity and beyond"), f64::INFINITY);
    assert!(parse_float(".").is_nan());
}

#[test]
fn number_formatting_covers_radix_and_precision_errors() {
    assert_eq!(number_to_string(-15.0, Some(16)).unwrap(), "-f");
    assert_eq!(number_to_string(f64::INFINITY, None).unwrap(), "Infinity");
    assert_eq!(
        number_to_string(1.0, Some(1)),
        Err(NumberFormatError::InvalidRadix)
    );
    assert_eq!(to_fixed(1.25, 1).unwrap(), "1.2");
    assert_eq!(
        to_fixed(1.0, 101),
        Err(NumberFormatError::FractionDigitsOutOfRange)
    );
    assert!(to_exponential(12.5, Some(1)).unwrap().starts_with("1.2e"));
    assert_eq!(
        to_exponential(1.0, Some(101)),
        Err(NumberFormatError::FractionDigitsOutOfRange)
    );
    assert_eq!(to_precision(12.345, Some(3)).unwrap(), "12.345");
    assert_eq!(
        to_precision(1.0, Some(0)),
        Err(NumberFormatError::PrecisionOutOfRange)
    );
}

#[test]
fn boolean_helpers_define_call_value_of_and_string_forms() {
    assert!(boolean_call(true));
    assert!(!boolean_call(false));
    assert!(boolean_value_of(true));
    assert_eq!(boolean_to_string(true), "true");
    assert_eq!(boolean_to_string(false), "false");
}

#[test]
fn math_metadata_includes_constants_names_and_arities() {
    assert!(MATH_CONSTANTS
        .iter()
        .any(|constant| constant.name == "PI" && constant.value == std::f64::consts::PI));
    assert!(MATH_CONSTANTS
        .iter()
        .any(|constant| constant.name == "SQRT1_2"));
    assert!(MATH_METHODS.contains(&math::MathMethodSpec {
        name: "atan2",
        length: 2,
    }));
    assert!(MATH_METHODS.contains(&math::MathMethodSpec {
        name: "random",
        length: 0,
    }));
}

#[test]
fn math_edges_preserve_nan_infinity_and_signed_zero_semantics() {
    assert_eq!(math::abs(-0.0).to_bits(), 0.0f64.to_bits());
    assert!(math::max(&[1.0, f64::NAN]).is_nan());
    assert_eq!(math::max(&[]), f64::NEG_INFINITY);
    assert_eq!(math::min(&[]), f64::INFINITY);
    assert_eq!(math::max(&[-0.0, 0.0]).to_bits(), 0.0f64.to_bits());
    assert_eq!(math::min(&[0.0, -0.0]).to_bits(), (-0.0f64).to_bits());
    assert_eq!(math::round(-0.1).to_bits(), (-0.0f64).to_bits());
    assert_eq!(math::sign(-0.0).to_bits(), (-0.0f64).to_bits());
    assert_eq!(math::trunc(-1.9), -1.0);
}

#[test]
fn math_integer_helpers_follow_to_uint32_style_conversion() {
    assert_eq!(math::clz32(1.0), 31);
    assert_eq!(math::clz32(0.0), 32);
    assert_eq!(math::imul(0xffff_ffff_u32 as f64, 5.0), -5);
    assert_eq!(math::fround(1.337), f64::from(1.337f32));
    assert_eq!(math::hypot(&[3.0, 4.0]), 5.0);
    assert_eq!(math::hypot(&[f64::INFINITY, f64::NAN]), f64::INFINITY);
    assert_eq!(math::pow(2.0, 8.0), 256.0);
    assert_eq!(math::unary("sqrt", 9.0), Some(3.0));
    assert_eq!(math::unary("notAFunction", 1.0), None);
}

#[test]
fn error_metadata_models_constructor_and_prototype_hierarchy() {
    assert_eq!(ERROR_CONSTRUCTORS.len(), 7);
    let error = constructor_spec("Error").unwrap();
    assert_eq!(error.length, 1);
    assert_eq!(error.parent_prototype_name, "Object.prototype");
    let type_error = constructor_spec("TypeError").unwrap();
    assert_eq!(type_error.prototype_name, "TypeError.prototype");
    assert_eq!(type_error.parent_prototype_name, "Error.prototype");
    assert!(constructor_spec("AggregateError").is_none());
}

#[test]
fn error_records_preserve_name_message_and_to_string_behavior() {
    let error = create_error_record("TypeError", Some("bad value".into()));
    assert_eq!(error.name, "TypeError");
    assert_eq!(error.message.as_deref(), Some("bad value"));
    assert_eq!(error_to_string(&error), "TypeError: bad value");
    assert_eq!(
        error_to_string(&create_error_record("Error", None)),
        "Error"
    );
    assert_eq!(
        error_to_string(&create_error_record("", Some("message only".into()))),
        "message only"
    );
    assert_eq!(error_to_string(&create_error_record("", None)), "");
}
