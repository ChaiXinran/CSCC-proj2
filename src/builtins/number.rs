//! Pure algorithms and metadata for the V6 `Number` builtins.
//!
//! Installer wiring intentionally lives outside this file until the V6
//! integration step. The functions here avoid VM/runtime dependencies so C2
//! can be developed without touching shared C0 or D-owned files.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NumberMethodSpec {
    pub name: &'static str,
    pub length: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct NumberConstantSpec {
    pub name: &'static str,
    pub value: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NumberFormatError {
    InvalidRadix,
    FractionDigitsOutOfRange,
    PrecisionOutOfRange,
}

pub(crate) const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;
pub(crate) const MIN_VALUE: f64 = f64::from_bits(1);
pub(crate) const MIN_SAFE_INTEGER: f64 = -9_007_199_254_740_991.0;

pub(crate) const NUMBER_CONSTANTS: &[NumberConstantSpec] = &[
    NumberConstantSpec {
        name: "EPSILON",
        value: f64::EPSILON,
    },
    NumberConstantSpec {
        name: "MAX_SAFE_INTEGER",
        value: MAX_SAFE_INTEGER,
    },
    NumberConstantSpec {
        name: "MAX_VALUE",
        value: f64::MAX,
    },
    NumberConstantSpec {
        name: "MIN_SAFE_INTEGER",
        value: MIN_SAFE_INTEGER,
    },
    NumberConstantSpec {
        name: "MIN_VALUE",
        value: MIN_VALUE,
    },
    NumberConstantSpec {
        name: "NaN",
        value: f64::NAN,
    },
    NumberConstantSpec {
        name: "NEGATIVE_INFINITY",
        value: f64::NEG_INFINITY,
    },
    NumberConstantSpec {
        name: "POSITIVE_INFINITY",
        value: f64::INFINITY,
    },
];

pub(crate) const NUMBER_STATIC_METHODS: &[NumberMethodSpec] = &[
    NumberMethodSpec {
        name: "isFinite",
        length: 1,
    },
    NumberMethodSpec {
        name: "isInteger",
        length: 1,
    },
    NumberMethodSpec {
        name: "isNaN",
        length: 1,
    },
    NumberMethodSpec {
        name: "isSafeInteger",
        length: 1,
    },
    NumberMethodSpec {
        name: "parseFloat",
        length: 1,
    },
    NumberMethodSpec {
        name: "parseInt",
        length: 2,
    },
];

pub(crate) const NUMBER_PROTOTYPE_METHODS: &[NumberMethodSpec] = &[
    NumberMethodSpec {
        name: "toExponential",
        length: 1,
    },
    NumberMethodSpec {
        name: "toFixed",
        length: 1,
    },
    NumberMethodSpec {
        name: "toPrecision",
        length: 1,
    },
    NumberMethodSpec {
        name: "toString",
        length: 1,
    },
    NumberMethodSpec {
        name: "toLocaleString",
        length: 0,
    },
    NumberMethodSpec {
        name: "valueOf",
        length: 0,
    },
];

#[must_use]
pub(crate) fn number_call(value: Option<f64>) -> f64 {
    value.unwrap_or(0.0)
}

#[must_use]
pub(crate) fn number_value_of(value: f64) -> f64 {
    value
}

#[must_use]
pub(crate) fn is_nan(value: f64) -> bool {
    value.is_nan()
}

#[must_use]
pub(crate) fn is_finite(value: f64) -> bool {
    value.is_finite()
}

#[must_use]
pub(crate) fn is_integer(value: f64) -> bool {
    value.is_finite() && value == value.trunc()
}

#[must_use]
pub(crate) fn is_safe_integer(value: f64) -> bool {
    is_integer(value) && (MIN_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&value)
}

pub(crate) fn number_to_string(
    value: f64,
    radix: Option<u32>,
) -> Result<String, NumberFormatError> {
    let radix = radix.unwrap_or(10);
    if !(2..=36).contains(&radix) {
        return Err(NumberFormatError::InvalidRadix);
    }
    if radix != 10 {
        return Ok(integer_to_radix_string(value, radix));
    }
    Ok(decimal_number_to_string(value))
}

pub(crate) fn to_fixed(value: f64, digits: u32) -> Result<String, NumberFormatError> {
    if digits > 100 {
        return Err(NumberFormatError::FractionDigitsOutOfRange);
    }
    if !value.is_finite() {
        return Ok(decimal_number_to_string(value));
    }
    if value.abs() >= 1e21 {
        return Ok(decimal_number_to_string(value));
    }
    Ok(format!("{value:.digits$}", digits = digits as usize))
}

pub(crate) fn to_exponential(
    value: f64,
    fraction_digits: Option<u32>,
) -> Result<String, NumberFormatError> {
    if !value.is_finite() {
        return Ok(decimal_number_to_string(value));
    }
    if fraction_digits.is_some_and(|digits| digits > 100) {
        return Err(NumberFormatError::FractionDigitsOutOfRange);
    }
    let value = if value == 0.0 { 0.0 } else { value };
    let formatted = match fraction_digits {
        Some(digits) => format!("{value:.digits$e}", digits = digits as usize),
        None => format!("{value:e}"),
    };
    Ok(normalize_exponential_notation(&formatted))
}

pub(crate) fn to_precision(
    value: f64,
    precision: Option<u32>,
) -> Result<String, NumberFormatError> {
    let Some(precision) = precision else {
        return Ok(decimal_number_to_string(value));
    };
    if !value.is_finite() {
        return Ok(decimal_number_to_string(value));
    }
    if !(1..=100).contains(&precision) {
        return Err(NumberFormatError::PrecisionOutOfRange);
    }
    if value == 0.0 {
        if precision == 1 {
            return Ok("0".into());
        }
        return Ok(format!("0.{}", "0".repeat(precision as usize - 1)));
    }

    let sign = if value.is_sign_negative() { "-" } else { "" };
    let raw = format!("{:.*e}", precision as usize - 1, value.abs());
    let Some((mantissa, exponent)) = raw.split_once('e') else {
        return Ok(format!("{sign}{raw}"));
    };
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    let digits: String = mantissa
        .chars()
        .filter(|character| *character != '.')
        .collect();

    if exponent < -6 || exponent >= precision as i32 {
        let mantissa = if precision == 1 {
            digits
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        return Ok(format!("{sign}{mantissa}e{exponent:+}"));
    }

    if exponent == precision as i32 - 1 {
        return Ok(format!("{sign}{digits}"));
    }
    if exponent >= 0 {
        let split = (exponent + 1) as usize;
        if split >= digits.len() {
            return Ok(format!(
                "{sign}{digits}{}",
                "0".repeat(split.saturating_sub(digits.len()))
            ));
        }
        return Ok(format!("{sign}{}.{}", &digits[..split], &digits[split..]));
    }

    let leading_zeros = (-exponent - 1) as usize;
    Ok(format!("{sign}0.{}{}", "0".repeat(leading_zeros), digits))
}

#[must_use]
pub(crate) fn parse_int(source: &str, radix: Option<i32>) -> f64 {
    let mut input = trim_ecmascript_whitespace(source);
    let sign = if let Some(rest) = input.strip_prefix('-') {
        input = rest;
        -1.0
    } else {
        input = input.strip_prefix('+').unwrap_or(input);
        1.0
    };

    let mut radix = radix.unwrap_or(0);
    if radix != 0 && !(2..=36).contains(&radix) {
        return f64::NAN;
    }

    let has_hex_prefix = input.starts_with("0x") || input.starts_with("0X");
    if radix == 0 {
        radix = if has_hex_prefix { 16 } else { 10 };
    }
    if radix == 16 && has_hex_prefix {
        input = &input[2..];
    }

    let mut value = 0.0;
    let mut consumed = false;
    for character in input.chars() {
        let Some(digit) = character.to_digit(radix as u32) else {
            break;
        };
        consumed = true;
        value = value * f64::from(radix) + f64::from(digit);
    }
    if consumed { sign * value } else { f64::NAN }
}

#[must_use]
pub(crate) fn parse_float(source: &str) -> f64 {
    let input = trim_ecmascript_whitespace(source);
    if infinity_prefix_is_complete(input, "Infinity") {
        return f64::INFINITY;
    }
    if infinity_prefix_is_complete(input, "+Infinity") {
        return f64::INFINITY;
    }
    if infinity_prefix_is_complete(input, "-Infinity") {
        return f64::NEG_INFINITY;
    }

    let mut best = None;
    for (index, _) in input.char_indices().skip(1) {
        let candidate = &input[..index];
        if let (true, Ok(value)) = (has_decimal_digit(candidate), candidate.parse::<f64>()) {
            best = Some(value);
        }
    }
    if let (true, Ok(value)) = (has_decimal_digit(input), input.parse::<f64>()) {
        best = Some(value);
    }
    best.unwrap_or(f64::NAN)
}

fn infinity_prefix_is_complete(input: &str, prefix: &str) -> bool {
    input.strip_prefix(prefix).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|character| !is_identifier_continue(character))
    })
}

fn decimal_number_to_string(value: f64) -> String {
    if value.is_nan() {
        "NaN".into()
    } else if value == f64::INFINITY {
        "Infinity".into()
    } else if value == f64::NEG_INFINITY {
        "-Infinity".into()
    } else if value == 0.0 {
        "0".into()
    } else {
        let magnitude = value.abs();
        if !(1e-6..1e21).contains(&magnitude) {
            js_scientific_number_to_string(value)
        } else {
            value.to_string()
        }
    }
}

fn js_scientific_number_to_string(value: f64) -> String {
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let raw = format!("{:e}", value.abs());
    let formatted = normalize_exponential_notation(&raw);
    format!("{sign}{formatted}")
}

fn normalize_exponential_notation(raw: &str) -> String {
    let Some((mantissa, exponent)) = raw.split_once('e') else {
        return raw.into();
    };
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    format!("{mantissa}e{exponent:+}")
}

fn integer_to_radix_string(value: f64, radix: u32) -> String {
    if value.is_nan() {
        return "NaN".into();
    }
    if value == f64::INFINITY {
        return "Infinity".into();
    }
    if value == f64::NEG_INFINITY {
        return "-Infinity".into();
    }
    if value == 0.0 {
        return "0".into();
    }

    let negative = value.is_sign_negative();
    let mut integer = value.abs().trunc() as u128;
    let mut digits = Vec::new();
    while integer > 0 {
        let digit = (integer % u128::from(radix)) as u8;
        digits.push(match digit {
            0..=9 => char::from(b'0' + digit),
            _ => char::from(b'a' + digit - 10),
        });
        integer /= u128::from(radix);
    }
    if negative {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

fn trim_ecmascript_whitespace(source: &str) -> &str {
    source.trim_matches(is_ecmascript_whitespace)
}

fn is_ecmascript_whitespace(character: char) -> bool {
    character.is_whitespace() || character == '\u{FEFF}'
}

fn has_decimal_digit(source: &str) -> bool {
    source.chars().any(|character| character.is_ascii_digit())
}

fn is_identifier_continue(character: char) -> bool {
    character == '$' || character == '_' || character.is_ascii_alphanumeric()
}
