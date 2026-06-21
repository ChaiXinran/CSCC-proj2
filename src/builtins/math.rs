//! Pure algorithms and metadata for the V6 `Math` builtins.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MathMethodSpec {
    pub name: &'static str,
    pub length: u8,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MathConstantSpec {
    pub name: &'static str,
    pub value: f64,
}

pub(crate) const MATH_CONSTANTS: &[MathConstantSpec] = &[
    MathConstantSpec {
        name: "E",
        value: std::f64::consts::E,
    },
    MathConstantSpec {
        name: "LN10",
        value: std::f64::consts::LN_10,
    },
    MathConstantSpec {
        name: "LN2",
        value: std::f64::consts::LN_2,
    },
    MathConstantSpec {
        name: "LOG10E",
        value: std::f64::consts::LOG10_E,
    },
    MathConstantSpec {
        name: "LOG2E",
        value: std::f64::consts::LOG2_E,
    },
    MathConstantSpec {
        name: "PI",
        value: std::f64::consts::PI,
    },
    MathConstantSpec {
        name: "SQRT1_2",
        value: std::f64::consts::FRAC_1_SQRT_2,
    },
    MathConstantSpec {
        name: "SQRT2",
        value: std::f64::consts::SQRT_2,
    },
];

pub(crate) const MATH_METHODS: &[MathMethodSpec] = &[
    MathMethodSpec {
        name: "abs",
        length: 1,
    },
    MathMethodSpec {
        name: "acos",
        length: 1,
    },
    MathMethodSpec {
        name: "acosh",
        length: 1,
    },
    MathMethodSpec {
        name: "asin",
        length: 1,
    },
    MathMethodSpec {
        name: "asinh",
        length: 1,
    },
    MathMethodSpec {
        name: "atan",
        length: 1,
    },
    MathMethodSpec {
        name: "atan2",
        length: 2,
    },
    MathMethodSpec {
        name: "atanh",
        length: 1,
    },
    MathMethodSpec {
        name: "cbrt",
        length: 1,
    },
    MathMethodSpec {
        name: "ceil",
        length: 1,
    },
    MathMethodSpec {
        name: "clz32",
        length: 1,
    },
    MathMethodSpec {
        name: "cos",
        length: 1,
    },
    MathMethodSpec {
        name: "cosh",
        length: 1,
    },
    MathMethodSpec {
        name: "exp",
        length: 1,
    },
    MathMethodSpec {
        name: "expm1",
        length: 1,
    },
    MathMethodSpec {
        name: "floor",
        length: 1,
    },
    MathMethodSpec {
        name: "fround",
        length: 1,
    },
    MathMethodSpec {
        name: "hypot",
        length: 2,
    },
    MathMethodSpec {
        name: "imul",
        length: 2,
    },
    MathMethodSpec {
        name: "log",
        length: 1,
    },
    MathMethodSpec {
        name: "log10",
        length: 1,
    },
    MathMethodSpec {
        name: "log1p",
        length: 1,
    },
    MathMethodSpec {
        name: "log2",
        length: 1,
    },
    MathMethodSpec {
        name: "max",
        length: 2,
    },
    MathMethodSpec {
        name: "min",
        length: 2,
    },
    MathMethodSpec {
        name: "pow",
        length: 2,
    },
    MathMethodSpec {
        name: "random",
        length: 0,
    },
    MathMethodSpec {
        name: "round",
        length: 1,
    },
    MathMethodSpec {
        name: "sign",
        length: 1,
    },
    MathMethodSpec {
        name: "sin",
        length: 1,
    },
    MathMethodSpec {
        name: "sinh",
        length: 1,
    },
    MathMethodSpec {
        name: "sqrt",
        length: 1,
    },
    MathMethodSpec {
        name: "tan",
        length: 1,
    },
    MathMethodSpec {
        name: "tanh",
        length: 1,
    },
    MathMethodSpec {
        name: "trunc",
        length: 1,
    },
];

#[must_use]
pub(crate) fn abs(value: f64) -> f64 {
    value.abs()
}

#[must_use]
pub(crate) fn max(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::NEG_INFINITY;
    }
    let mut result = f64::NEG_INFINITY;
    for value in values {
        if value.is_nan() {
            return f64::NAN;
        }
        if *value > result || (is_negative_zero(result) && is_positive_zero(*value)) {
            result = *value;
        }
    }
    result
}

#[must_use]
pub(crate) fn min(values: &[f64]) -> f64 {
    if values.is_empty() {
        return f64::INFINITY;
    }
    let mut result = f64::INFINITY;
    for value in values {
        if value.is_nan() {
            return f64::NAN;
        }
        if *value < result || (is_positive_zero(result) && is_negative_zero(*value)) {
            result = *value;
        }
    }
    result
}

#[must_use]
pub(crate) fn pow(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
}

#[must_use]
pub(crate) fn round(value: f64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let result = (value + 0.5).floor();
    if result == 0.0 && value.is_sign_negative() {
        -0.0
    } else {
        result
    }
}

#[must_use]
pub(crate) fn sign(value: f64) -> f64 {
    if value.is_nan() || value == 0.0 {
        value
    } else if value.is_sign_negative() {
        -1.0
    } else {
        1.0
    }
}

#[must_use]
pub(crate) fn trunc(value: f64) -> f64 {
    value.trunc()
}

#[must_use]
pub(crate) fn clz32(value: f64) -> u32 {
    to_uint32(value).leading_zeros()
}

#[must_use]
pub(crate) fn fround(value: f64) -> f64 {
    f64::from(value as f32)
}

#[must_use]
pub(crate) fn imul(left: f64, right: f64) -> i32 {
    let left = to_uint32(left) as i32;
    let right = to_uint32(right) as i32;
    left.wrapping_mul(right)
}

#[must_use]
pub(crate) fn hypot(values: &[f64]) -> f64 {
    if values.iter().any(|value| value.is_infinite()) {
        return f64::INFINITY;
    }
    if values.iter().any(|value| value.is_nan()) {
        return f64::NAN;
    }
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}

#[must_use]
pub(crate) fn unary(name: &str, value: f64) -> Option<f64> {
    Some(match name {
        "acos" => value.acos(),
        "acosh" => value.acosh(),
        "asin" => value.asin(),
        "asinh" => value.asinh(),
        "atan" => value.atan(),
        "atanh" => value.atanh(),
        "cbrt" => value.cbrt(),
        "ceil" => value.ceil(),
        "cos" => value.cos(),
        "cosh" => value.cosh(),
        "exp" => value.exp(),
        "expm1" => value.exp_m1(),
        "floor" => value.floor(),
        "log" => value.ln(),
        "log10" => value.log10(),
        "log1p" => value.ln_1p(),
        "log2" => value.log2(),
        "sin" => value.sin(),
        "sinh" => value.sinh(),
        "sqrt" => value.sqrt(),
        "tan" => value.tan(),
        "tanh" => value.tanh(),
        _ => return None,
    })
}

fn to_uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let two_32 = 4_294_967_296.0;
    let integer = value.trunc().rem_euclid(two_32);
    integer as u32
}

fn is_positive_zero(value: f64) -> bool {
    value == 0.0 && value.is_sign_positive()
}

fn is_negative_zero(value: f64) -> bool {
    value == 0.0 && value.is_sign_negative()
}
