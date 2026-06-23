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
        name: "f16round",
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
        name: "sumPrecise",
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
    if exponent.is_nan() {
        return f64::NAN;
    }
    if exponent == 0.0 {
        return 1.0;
    }
    if base.is_nan() {
        return f64::NAN;
    }
    if base.abs() == 1.0 && exponent.is_infinite() {
        return f64::NAN;
    }
    base.powf(exponent)
}

#[must_use]
pub(crate) fn round(value: f64) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let floor = value.floor();
    let result = if value - floor < 0.5 {
        floor
    } else {
        floor + 1.0
    };
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
pub(crate) fn f16round(value: f64) -> f64 {
    if value.is_nan() || value.is_infinite() || value == 0.0 {
        return value;
    }

    let sign = if value.is_sign_negative() { -1.0 } else { 1.0 };
    let magnitude = value.abs();
    let rounded = if magnitude < 2_f64.powi(-14) {
        (magnitude / 2_f64.powi(-24)).round_ties_even() * 2_f64.powi(-24)
    } else {
        let exponent = magnitude.log2().floor() as i32;
        let step = 2_f64.powi(exponent - 10);
        (magnitude / step).round_ties_even() * step
    };

    if rounded >= 65_520.0 {
        sign * f64::INFINITY
    } else {
        sign * rounded
    }
}

#[must_use]
pub(crate) fn imul(left: f64, right: f64) -> i32 {
    let left = to_uint32(left) as i32;
    let right = to_uint32(right) as i32;
    left.wrapping_mul(right)
}

#[must_use]
pub(crate) fn hypot(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    if values.iter().any(|value| value.is_infinite()) {
        return f64::INFINITY;
    }
    if values.iter().any(|value| value.is_nan()) {
        return f64::NAN;
    }
    values.iter().map(|value| value * value).sum::<f64>().sqrt()
}

#[must_use]
#[allow(dead_code)]
pub(crate) fn sum_precise(values: &[f64]) -> f64 {
    let mut has_positive_infinity = false;
    let mut has_negative_infinity = false;
    let mut all_zeroes_are_negative = true;
    let mut sum = ExactBinarySum::default();

    for value in values {
        if value.is_nan() {
            return f64::NAN;
        }
        if *value == f64::INFINITY {
            has_positive_infinity = true;
            continue;
        }
        if *value == f64::NEG_INFINITY {
            has_negative_infinity = true;
            continue;
        }
        if *value == 0.0 {
            if value.is_sign_positive() {
                all_zeroes_are_negative = false;
            }
            continue;
        }

        all_zeroes_are_negative = false;
        sum.add_f64(*value);
    }

    match (has_positive_infinity, has_negative_infinity) {
        (true, true) => return f64::NAN,
        (true, false) => return f64::INFINITY,
        (false, true) => return f64::NEG_INFINITY,
        (false, false) => {}
    }

    if sum.is_zero() {
        return if all_zeroes_are_negative { -0.0 } else { 0.0 };
    }

    sum.to_f64()
}

#[derive(Default)]
struct ExactBinarySum {
    negative: bool,
    limbs: Vec<u64>,
}

impl ExactBinarySum {
    fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    fn add_f64(&mut self, value: f64) {
        let bits = value.to_bits();
        let negative = bits >> 63 != 0;
        let exponent = ((bits >> 52) & 0x7ff) as usize;
        let fraction = bits & ((1_u64 << 52) - 1);
        let (significand, shift) = if exponent == 0 {
            (fraction, 0)
        } else {
            ((1_u64 << 52) | fraction, exponent - 1)
        };
        let term = shifted_magnitude(significand, shift);
        if self.is_zero() {
            self.negative = negative;
            self.limbs = term;
        } else if self.negative == negative {
            add_magnitude(&mut self.limbs, &term);
        } else {
            match compare_magnitude(&self.limbs, &term) {
                std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => {
                    subtract_magnitude(&mut self.limbs, &term);
                }
                std::cmp::Ordering::Less => {
                    let mut result = term;
                    subtract_magnitude(&mut result, &self.limbs);
                    self.limbs = result;
                    self.negative = negative;
                }
            }
        }
        trim_zero_limbs(&mut self.limbs);
    }

    fn to_f64(&self) -> f64 {
        let sign = u64::from(self.negative) << 63;
        let highest = bit_length(&self.limbs) - 1;
        if highest < 52 {
            return f64::from_bits(sign | self.limbs.first().copied().unwrap_or(0));
        }

        let mut shift = highest - 52;
        let mut significand = shifted_low_u64(&self.limbs, shift);
        if shift > 0 {
            let half = bit_is_set(&self.limbs, shift - 1);
            let below_half = any_bits_below(&self.limbs, shift - 1);
            if half && (below_half || significand & 1 != 0) {
                significand += 1;
                if significand == 1_u64 << 53 {
                    significand >>= 1;
                    shift += 1;
                }
            }
        }

        let exponent = shift + 1;
        if exponent >= 0x7ff {
            return f64::from_bits(sign | (0x7ff_u64 << 52));
        }
        let fraction = significand & ((1_u64 << 52) - 1);
        f64::from_bits(sign | ((exponent as u64) << 52) | fraction)
    }
}

fn shifted_magnitude(value: u64, shift: usize) -> Vec<u64> {
    let word = shift / 64;
    let offset = shift % 64;
    let mut result = vec![0; word + 2];
    result[word] = value << offset;
    if offset != 0 {
        result[word + 1] = value >> (64 - offset);
    }
    trim_zero_limbs(&mut result);
    result
}

fn add_magnitude(left: &mut Vec<u64>, right: &[u64]) {
    left.resize(left.len().max(right.len()) + 1, 0);
    let mut carry = 0_u128;
    for (index, item) in left.iter_mut().enumerate() {
        let total = u128::from(*item) + u128::from(right.get(index).copied().unwrap_or(0)) + carry;
        *item = total as u64;
        carry = total >> 64;
    }
    trim_zero_limbs(left);
}

fn subtract_magnitude(left: &mut Vec<u64>, right: &[u64]) {
    let mut borrow = 0_u128;
    for (index, item) in left.iter_mut().enumerate() {
        let subtrahend = u128::from(right.get(index).copied().unwrap_or(0)) + borrow;
        let value = u128::from(*item);
        if value >= subtrahend {
            *item = (value - subtrahend) as u64;
            borrow = 0;
        } else {
            *item = ((1_u128 << 64) + value - subtrahend) as u64;
            borrow = 1;
        }
    }
    trim_zero_limbs(left);
}

fn compare_magnitude(left: &[u64], right: &[u64]) -> std::cmp::Ordering {
    left.len()
        .cmp(&right.len())
        .then_with(|| left.iter().rev().cmp(right.iter().rev()))
}

fn trim_zero_limbs(limbs: &mut Vec<u64>) {
    while limbs.last() == Some(&0) {
        limbs.pop();
    }
}

fn bit_length(limbs: &[u64]) -> usize {
    let last = limbs.last().copied().unwrap_or(0);
    (limbs.len() - 1) * 64 + (64 - last.leading_zeros() as usize)
}

fn bit_is_set(limbs: &[u64], bit: usize) -> bool {
    limbs
        .get(bit / 64)
        .is_some_and(|value| value & (1_u64 << (bit % 64)) != 0)
}

fn any_bits_below(limbs: &[u64], bit: usize) -> bool {
    let full_words = bit / 64;
    if limbs.iter().take(full_words).any(|value| *value != 0) {
        return true;
    }
    let remaining = bit % 64;
    remaining != 0
        && limbs
            .get(full_words)
            .is_some_and(|value| value & ((1_u64 << remaining) - 1) != 0)
}

fn shifted_low_u64(limbs: &[u64], shift: usize) -> u64 {
    let word = shift / 64;
    let offset = shift % 64;
    let low = limbs.get(word).copied().unwrap_or(0) >> offset;
    if offset == 0 {
        low
    } else {
        low | (limbs.get(word + 1).copied().unwrap_or(0) << (64 - offset))
    }
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
