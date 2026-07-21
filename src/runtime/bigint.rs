//! Arbitrary-precision ECMAScript BigInt values and operations.

use std::{cmp::Ordering, fmt};

use crate::vm::VmError;

const LIMB_BITS: usize = 32;
const MAX_RESULT_BITS: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct BigIntValue {
    negative: bool,
    limbs: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BigIntParseError {
    InvalidSyntax,
    UnsupportedRadix,
}

impl BigIntValue {
    fn new(negative: bool, mut limbs: Vec<u32>) -> Self {
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        Self {
            negative: negative && !limbs.is_empty(),
            limbs,
        }
    }

    #[must_use]
    pub(crate) fn estimated_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.limbs.capacity() * std::mem::size_of::<u32>()
    }

    fn bit_len(&self) -> usize {
        self.limbs.last().map_or(0, |last| {
            (self.limbs.len() - 1) * LIMB_BITS + (LIMB_BITS - last.leading_zeros() as usize)
        })
    }

    fn abs_cmp(&self, other: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&other.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }

    fn abs(&self) -> Self {
        Self::new(false, self.limbs.clone())
    }

    fn negated(&self) -> Self {
        Self::new(!self.negative, self.limbs.clone())
    }

    fn add_small(&mut self, value: u32) {
        let mut carry = u64::from(value);
        for limb in &mut self.limbs {
            let sum = u64::from(*limb) + carry;
            *limb = sum as u32;
            carry = sum >> 32;
            if carry == 0 {
                return;
            }
        }
        if carry != 0 {
            self.limbs.push(carry as u32);
        }
    }

    fn mul_small(&mut self, value: u32) {
        if value == 0 || self.limbs.is_empty() {
            self.limbs.clear();
            self.negative = false;
            return;
        }
        let mut carry = 0_u64;
        for limb in &mut self.limbs {
            let product = u64::from(*limb) * u64::from(value) + carry;
            *limb = product as u32;
            carry = product >> 32;
        }
        if carry != 0 {
            self.limbs.push(carry as u32);
        }
    }

    fn div_rem_small(&mut self, divisor: u32) -> u32 {
        let mut remainder = 0_u64;
        for limb in self.limbs.iter_mut().rev() {
            let current = (remainder << 32) | u64::from(*limb);
            *limb = (current / u64::from(divisor)) as u32;
            remainder = current % u64::from(divisor);
        }
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        remainder as u32
    }

    fn shl_unsigned(&self, shift: usize) -> Self {
        if self.limbs.is_empty() {
            return Self::default();
        }
        let word_shift = shift / LIMB_BITS;
        let bit_shift = shift % LIMB_BITS;
        let mut limbs = vec![0; word_shift + self.limbs.len() + usize::from(bit_shift != 0)];
        let mut carry = 0_u64;
        for (index, limb) in self.limbs.iter().copied().enumerate() {
            let value = (u64::from(limb) << bit_shift) | carry;
            limbs[index + word_shift] = value as u32;
            carry = value >> 32;
        }
        if bit_shift != 0 {
            limbs[word_shift + self.limbs.len()] = carry as u32;
        }
        Self::new(self.negative, limbs)
    }

    fn shr_magnitude(&self, shift: usize) -> Self {
        let word_shift = shift / LIMB_BITS;
        if word_shift >= self.limbs.len() {
            return Self::default();
        }
        let bit_shift = shift % LIMB_BITS;
        let mut limbs = Vec::with_capacity(self.limbs.len() - word_shift);
        for index in word_shift..self.limbs.len() {
            let mut value = u64::from(self.limbs[index]) >> bit_shift;
            if bit_shift != 0 && index + 1 < self.limbs.len() {
                value |= u64::from(self.limbs[index + 1]) << (32 - bit_shift);
            }
            limbs.push(value as u32);
        }
        Self::new(false, limbs)
    }

    fn has_low_bits(&self, count: usize) -> bool {
        let full = count / LIMB_BITS;
        if self.limbs.iter().take(full).any(|limb| *limb != 0) {
            return true;
        }
        let remaining = count % LIMB_BITS;
        remaining != 0
            && self
                .limbs
                .get(full)
                .is_some_and(|limb| *limb & ((1_u32 << remaining) - 1) != 0)
    }

    fn to_twos(&self, width: usize) -> Vec<u32> {
        let mut result = vec![0; width];
        result[..self.limbs.len().min(width)]
            .copy_from_slice(&self.limbs[..self.limbs.len().min(width)]);
        if self.negative {
            for limb in &mut result {
                *limb = !*limb;
            }
            let mut carry = true;
            for limb in &mut result {
                if !carry {
                    break;
                }
                let (next, overflow) = limb.overflowing_add(1);
                *limb = next;
                carry = overflow;
            }
        }
        result
    }

    fn from_twos(mut limbs: Vec<u32>) -> Self {
        let negative = limbs.last().is_some_and(|limb| limb & 0x8000_0000 != 0);
        if negative {
            let mut borrow = true;
            for limb in &mut limbs {
                if borrow {
                    let (next, overflow) = limb.overflowing_sub(1);
                    *limb = next;
                    borrow = overflow;
                }
                *limb = !*limb;
            }
        }
        Self::new(negative, limbs)
    }
}

impl fmt::Display for BigIntValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&to_radix_string(self, 10))
    }
}

pub fn from_i64(value: i64) -> BigIntValue {
    let magnitude = value.unsigned_abs();
    BigIntValue::new(value < 0, vec![magnitude as u32, (magnitude >> 32) as u32])
}

pub fn from_u64(value: u64) -> BigIntValue {
    BigIntValue::new(false, vec![value as u32, (value >> 32) as u32])
}

pub(crate) fn from_i128(value: i128) -> BigIntValue {
    let magnitude = value.unsigned_abs();
    BigIntValue::new(
        value < 0,
        (0..4)
            .map(|shift| (magnitude >> (shift * 32)) as u32)
            .collect(),
    )
}

pub(crate) fn from_f64_integer(value: f64) -> Option<BigIntValue> {
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    if value == 0.0 {
        return Some(BigIntValue::default());
    }
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023;
    let significand = (bits & ((1_u64 << 52) - 1)) | (1_u64 << 52);
    let mut result = from_u64(significand);
    if exponent >= 52 {
        result = result.shl_unsigned((exponent - 52) as usize);
    } else {
        result = result.shr_magnitude((52 - exponent) as usize);
    }
    result.negative = value.is_sign_negative() && !is_zero(&result);
    Some(result)
}

pub fn to_i128_if_exact(value: &BigIntValue) -> Option<i128> {
    if value.bit_len() > 127 + usize::from(value.negative) {
        return None;
    }
    let mut magnitude = 0_u128;
    for (index, limb) in value.limbs.iter().copied().enumerate() {
        magnitude |= u128::from(limb) << (index * 32);
    }
    if value.negative {
        if magnitude == 1_u128 << 127 {
            Some(i128::MIN)
        } else {
            i128::try_from(magnitude).ok().map(|number| -number)
        }
    } else {
        i128::try_from(magnitude).ok()
    }
}

pub fn to_f64_lossy(value: &BigIntValue) -> f64 {
    value.to_string().parse::<f64>().unwrap_or({
        if value.negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    })
}

pub fn is_zero(value: &BigIntValue) -> bool {
    value.limbs.is_empty()
}

pub fn sign(value: &BigIntValue) -> Ordering {
    if is_zero(value) {
        Ordering::Equal
    } else if value.negative {
        Ordering::Less
    } else {
        Ordering::Greater
    }
}

fn parse_digits(digits: &str, radix: u32, separators: bool) -> Option<BigIntValue> {
    if !(2..=36).contains(&radix) || digits.is_empty() {
        return None;
    }
    let mut result = BigIntValue::default();
    let mut saw_digit = false;
    let mut previous_separator = false;
    for character in digits.chars() {
        if character == '_' && separators {
            if !saw_digit || previous_separator {
                return None;
            }
            previous_separator = true;
            continue;
        }
        let digit = character.to_digit(radix)?;
        result.mul_small(radix);
        result.add_small(digit);
        saw_digit = true;
        previous_separator = false;
    }
    (!previous_separator && saw_digit).then_some(result)
}

pub fn parse_bigint_literal(raw: &str) -> Result<BigIntValue, BigIntParseError> {
    let body = raw
        .strip_suffix('n')
        .ok_or(BigIntParseError::InvalidSyntax)?;
    let (digits, radix) = prefixed_digits(body);
    parse_digits(digits, radix, true).ok_or(BigIntParseError::InvalidSyntax)
}

pub fn parse_bigint_string(input: &str) -> Option<BigIntValue> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Some(BigIntValue::default());
    }
    let (negative, unsigned, signed) = if let Some(rest) = trimmed.strip_prefix('-') {
        (true, rest, true)
    } else if let Some(rest) = trimmed.strip_prefix('+') {
        (false, rest, true)
    } else {
        (false, trimmed, false)
    };
    let (digits, radix) = prefixed_digits(unsigned);
    if signed && radix != 10 {
        return None;
    }
    let mut result = parse_digits(digits, radix, false)?;
    result.negative = negative && !is_zero(&result);
    Some(result)
}

fn prefixed_digits(input: &str) -> (&str, u32) {
    input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
        .map(|digits| (digits, 16))
        .or_else(|| {
            input
                .strip_prefix("0b")
                .or_else(|| input.strip_prefix("0B"))
                .map(|digits| (digits, 2))
        })
        .or_else(|| {
            input
                .strip_prefix("0o")
                .or_else(|| input.strip_prefix("0O"))
                .map(|digits| (digits, 8))
        })
        .unwrap_or((input, 10))
}

pub fn to_radix_string(value: &BigIntValue, radix: u32) -> String {
    debug_assert!((2..=36).contains(&radix));
    if is_zero(value) {
        return "0".into();
    }
    let mut magnitude = value.abs();
    let mut digits = Vec::new();
    while !is_zero(&magnitude) {
        let digit = magnitude.div_rem_small(radix) as u8;
        digits.push(if digit < 10 {
            b'0' + digit
        } else {
            b'a' + digit - 10
        } as char);
    }
    if value.negative {
        digits.push('-');
    }
    digits.iter().rev().collect()
}

fn add_magnitudes(left: &BigIntValue, right: &BigIntValue) -> Vec<u32> {
    let length = left.limbs.len().max(right.limbs.len());
    let mut result = Vec::with_capacity(length + 1);
    let mut carry = 0_u64;
    for index in 0..length {
        let sum = u64::from(left.limbs.get(index).copied().unwrap_or(0))
            + u64::from(right.limbs.get(index).copied().unwrap_or(0))
            + carry;
        result.push(sum as u32);
        carry = sum >> 32;
    }
    if carry != 0 {
        result.push(carry as u32);
    }
    result
}

fn sub_magnitudes(left: &BigIntValue, right: &BigIntValue) -> Vec<u32> {
    let mut result = Vec::with_capacity(left.limbs.len());
    let mut borrow = 0_i64;
    for index in 0..left.limbs.len() {
        let difference = i64::from(left.limbs[index])
            - i64::from(right.limbs.get(index).copied().unwrap_or(0))
            - borrow;
        if difference < 0 {
            result.push((difference + (1_i64 << 32)) as u32);
            borrow = 1;
        } else {
            result.push(difference as u32);
            borrow = 0;
        }
    }
    result
}

pub fn add(left: &BigIntValue, right: &BigIntValue) -> BigIntValue {
    if left.negative == right.negative {
        BigIntValue::new(left.negative, add_magnitudes(left, right))
    } else {
        match left.abs_cmp(right) {
            Ordering::Greater | Ordering::Equal => {
                BigIntValue::new(left.negative, sub_magnitudes(left, right))
            }
            Ordering::Less => BigIntValue::new(right.negative, sub_magnitudes(right, left)),
        }
    }
}

pub fn sub(left: &BigIntValue, right: &BigIntValue) -> BigIntValue {
    add(left, &right.negated())
}

pub fn mul(left: &BigIntValue, right: &BigIntValue) -> BigIntValue {
    if is_zero(left) || is_zero(right) {
        return BigIntValue::default();
    }
    let mut result = vec![0_u32; left.limbs.len() + right.limbs.len()];
    for (left_index, left_limb) in left.limbs.iter().copied().enumerate() {
        let mut carry = 0_u64;
        for (right_index, right_limb) in right.limbs.iter().copied().enumerate() {
            let index = left_index + right_index;
            let product =
                u64::from(left_limb) * u64::from(right_limb) + u64::from(result[index]) + carry;
            result[index] = product as u32;
            carry = product >> 32;
        }
        result[left_index + right.limbs.len()] = carry as u32;
    }
    BigIntValue::new(left.negative != right.negative, result)
}

fn div_rem_abs(left: &BigIntValue, right: &BigIntValue) -> (BigIntValue, BigIntValue) {
    if left.abs_cmp(right) == Ordering::Less {
        return (BigIntValue::default(), left.abs());
    }
    let mut remainder = left.abs();
    let shift = remainder.bit_len() - right.bit_len();
    let mut divisor = right.abs().shl_unsigned(shift);
    let mut quotient = BigIntValue::default();
    for bit in (0..=shift).rev() {
        if remainder.abs_cmp(&divisor) != Ordering::Less {
            remainder = BigIntValue::new(false, sub_magnitudes(&remainder, &divisor));
            let limb = bit / LIMB_BITS;
            if quotient.limbs.len() <= limb {
                quotient.limbs.resize(limb + 1, 0);
            }
            quotient.limbs[limb] |= 1 << (bit % LIMB_BITS);
        }
        divisor = divisor.shr_magnitude(1);
    }
    (BigIntValue::new(false, quotient.limbs), remainder)
}

pub fn div(left: &BigIntValue, right: &BigIntValue) -> Result<BigIntValue, VmError> {
    if is_zero(right) {
        return Err(VmError::range("BigInt division by zero"));
    }
    let (mut quotient, _) = div_rem_abs(left, right);
    quotient.negative = !is_zero(&quotient) && left.negative != right.negative;
    Ok(quotient)
}

pub fn rem(left: &BigIntValue, right: &BigIntValue) -> Result<BigIntValue, VmError> {
    if is_zero(right) {
        return Err(VmError::range("BigInt division by zero"));
    }
    let (_, mut remainder) = div_rem_abs(left, right);
    remainder.negative = !is_zero(&remainder) && left.negative;
    Ok(remainder)
}

pub fn pow(left: &BigIntValue, right: &BigIntValue) -> Result<BigIntValue, VmError> {
    if right.negative {
        return Err(VmError::range("BigInt exponent must be non-negative"));
    }
    let exponent = to_u64(right).ok_or_else(|| VmError::range("BigInt exponent is too large"))?;
    if left.bit_len().saturating_mul(exponent as usize) > MAX_RESULT_BITS {
        return Err(VmError::range("BigInt result exceeds implementation limit"));
    }
    let mut exponent = exponent;
    let mut base = left.clone();
    let mut result = from_u64(1);
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = mul(&result, &base);
        }
        exponent >>= 1;
        if exponent != 0 {
            base = mul(&base, &base);
        }
    }
    Ok(result)
}

fn bitop(left: &BigIntValue, right: &BigIntValue, op: impl Fn(u32, u32) -> u32) -> BigIntValue {
    let width = left.limbs.len().max(right.limbs.len()) + 1;
    let left = left.to_twos(width);
    let right = right.to_twos(width);
    BigIntValue::from_twos(left.into_iter().zip(right).map(|(a, b)| op(a, b)).collect())
}

pub fn bitand(left: &BigIntValue, right: &BigIntValue) -> BigIntValue {
    bitop(left, right, |a, b| a & b)
}
pub fn bitor(left: &BigIntValue, right: &BigIntValue) -> BigIntValue {
    bitop(left, right, |a, b| a | b)
}
pub fn bitxor(left: &BigIntValue, right: &BigIntValue) -> BigIntValue {
    bitop(left, right, |a, b| a ^ b)
}
pub fn bitnot(value: &BigIntValue) -> BigIntValue {
    sub(&value.negated(), &from_u64(1))
}

fn shift_count(value: &BigIntValue) -> Result<(bool, usize), VmError> {
    let magnitude = to_u64(&value.abs())
        .and_then(|count| usize::try_from(count).ok())
        .filter(|count| *count <= MAX_RESULT_BITS)
        .ok_or_else(|| VmError::range("BigInt shift count exceeds implementation limit"))?;
    Ok((value.negative, magnitude))
}

pub fn shl(value: &BigIntValue, shift: &BigIntValue) -> Result<BigIntValue, VmError> {
    let (negative, count) = shift_count(shift)?;
    if negative {
        shr_count(value, count)
    } else {
        if value.bit_len().saturating_add(count) > MAX_RESULT_BITS {
            return Err(VmError::range("BigInt result exceeds implementation limit"));
        }
        Ok(value.shl_unsigned(count))
    }
}

pub fn shr(value: &BigIntValue, shift: &BigIntValue) -> Result<BigIntValue, VmError> {
    let (negative, count) = shift_count(shift)?;
    if negative {
        if value.bit_len().saturating_add(count) > MAX_RESULT_BITS {
            return Err(VmError::range("BigInt result exceeds implementation limit"));
        }
        Ok(value.shl_unsigned(count))
    } else {
        shr_count(value, count)
    }
}

fn shr_count(value: &BigIntValue, count: usize) -> Result<BigIntValue, VmError> {
    let discarded = value.negative && value.has_low_bits(count);
    let mut result = value.abs().shr_magnitude(count);
    if value.negative {
        if discarded {
            result.add_small(1);
        }
        result.negative = !is_zero(&result);
    }
    Ok(result)
}

pub fn cmp(left: &BigIntValue, right: &BigIntValue) -> Ordering {
    match (left.negative, right.negative) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => left.abs_cmp(right),
        (true, true) => right.abs_cmp(left),
    }
}

pub fn compare_bigint_number(value: &BigIntValue, number: f64) -> Option<Ordering> {
    if number.is_nan() {
        return None;
    }
    if number == f64::INFINITY {
        return Some(Ordering::Less);
    }
    if number == f64::NEG_INFINITY {
        return Some(Ordering::Greater);
    }
    let truncated = number.trunc();
    let integer = from_f64_integer(truncated)?;
    let ordering = cmp(value, &integer);
    if ordering != Ordering::Equal || number == truncated {
        return Some(ordering);
    }
    Some(if number.is_sign_positive() {
        Ordering::Less
    } else {
        Ordering::Greater
    })
}

pub fn number_equals_bigint(number: f64, value: &BigIntValue) -> bool {
    number.is_finite()
        && number.fract() == 0.0
        && from_f64_integer(number).is_some_and(|number| &number == value)
}

pub fn as_uint_n(bits: u64, value: &BigIntValue) -> BigIntValue {
    if bits == 0 {
        return BigIntValue::default();
    }
    let width = usize::try_from(bits.div_ceil(32)).unwrap_or(usize::MAX);
    if width > MAX_RESULT_BITS / 32 {
        return value.clone();
    }
    let mut twos = value.to_twos(width);
    let remaining = bits % 32;
    if remaining != 0 {
        *twos.last_mut().unwrap() &= (1_u32 << remaining) - 1;
    }
    BigIntValue::new(false, twos)
}

pub fn as_int_n(bits: u64, value: &BigIntValue) -> BigIntValue {
    let unsigned = as_uint_n(bits, value);
    if bits == 0 || bits > MAX_RESULT_BITS as u64 {
        return unsigned;
    }
    let sign_bit = usize::try_from(bits - 1).unwrap();
    if unsigned
        .limbs
        .get(sign_bit / 32)
        .is_some_and(|limb| limb & (1 << (sign_bit % 32)) != 0)
    {
        sub(&unsigned, &from_u64(1).shl_unsigned(bits as usize))
    } else {
        unsigned
    }
}

pub(crate) fn to_u64(value: &BigIntValue) -> Option<u64> {
    if value.negative || value.limbs.len() > 2 {
        return None;
    }
    Some(
        u64::from(value.limbs.first().copied().unwrap_or(0))
            | (u64::from(value.limbs.get(1).copied().unwrap_or(0)) << 32),
    )
}

pub(crate) fn low_u64_bits(value: &BigIntValue) -> u64 {
    let twos = value.to_twos(2);
    u64::from(twos[0]) | (u64::from(twos[1]) << 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(value: &str) -> BigIntValue {
        parse_bigint_string(value).unwrap()
    }

    #[test]
    fn arbitrary_precision_arithmetic_and_radix_round_trip() {
        let left = parse("340282366920938463463374607431768211456");
        let right = parse("18446744073709551617");
        assert_eq!(
            add(&left, &right).to_string(),
            "340282366920938463481821351505477763073"
        );
        assert_eq!(
            mul(&right, &right).to_string(),
            "340282366920938463500268095579187314689"
        );
        assert_eq!(
            div(&left, &right).unwrap().to_string(),
            "18446744073709551615"
        );
        assert_eq!(rem(&left, &right).unwrap().to_string(), "1");
    }

    #[test]
    fn signed_bitwise_and_shift_semantics() {
        assert_eq!(bitand(&parse("-2"), &parse("5")).to_string(), "4");
        assert_eq!(bitor(&parse("-8"), &parse("3")).to_string(), "-5");
        assert_eq!(bitnot(&parse("0")).to_string(), "-1");
        assert_eq!(shr(&parse("-3"), &parse("1")).unwrap().to_string(), "-2");
        assert_eq!(shl(&parse("8"), &parse("-2")).unwrap().to_string(), "2");
    }

    #[test]
    fn string_to_bigint_rules_and_exact_number_comparison() {
        assert_eq!(parse_bigint_string("  ").unwrap().to_string(), "0");
        assert!(parse_bigint_string("-0x1").is_none());
        assert!(parse_bigint_string("1_0").is_none());
        let beyond_safe = parse("9007199254740993");
        assert!(!number_equals_bigint(9_007_199_254_740_992.0, &beyond_safe));
        assert_eq!(
            compare_bigint_number(&beyond_safe, 9_007_199_254_740_992.0),
            Some(Ordering::Greater)
        );
    }
}
