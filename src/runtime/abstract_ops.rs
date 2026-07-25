//! ECMAScript abstract operations (§7).
//!
//! This module centralises shared abstract operations used by builtins, the
//! VM, and the runtime. It is maintained by the **B group** (runtime
//! protocols), per the native-v16-95-shared-interface-draft.
//!
//! Other modules must not duplicate equivalent helpers; file locks are
//! registered in `docs/version/native-v16-locks.md`.

use super::JsValue;

// ---------------------------------------------------------------------------
// §7.1.4  ToIntegerOrInfinity
// ---------------------------------------------------------------------------

/// `ToIntegerOrInfinity(argument)` — §7.1.4.
///
/// 1. Let _number_ be ? ToNumber(_argument_).
/// 2. If _number_ is NaN, +0𝔽, or -0𝔽, return 0.
/// 3. If _number_ is +∞𝔽, return +∞.
/// 4. If _number_ is -∞𝔽, return -∞.
/// 5. Return truncate(ℝ(_number_)).
#[must_use]
pub fn to_integer_or_infinity(value: &JsValue) -> f64 {
    let n = value.to_number().unwrap_or(f64::NAN);
    if n.is_nan() || n == 0.0 {
        return 0.0;
    }
    if n.is_infinite() {
        return n;
    }
    n.trunc()
}

// ---------------------------------------------------------------------------
// §7.1.20 ToLength
// ---------------------------------------------------------------------------

/// Maximum safe length value (§7.1.20 step 3).
const MAX_SAFE_LENGTH: f64 = 9_007_199_254_740_991.0; // 2^53 - 1

/// `ToLength(argument)` — §7.1.20.
///
/// 1. Let _len_ be ? ToIntegerOrInfinity(_argument_).
/// 2. If _len_ ≤ 0, return +0𝔽.
/// 3. Return min(_len_, 2⁵³ - 1).
#[must_use]
pub fn to_length(value: &JsValue) -> f64 {
    let len = to_integer_or_infinity(value);
    if len <= 0.0 {
        return 0.0;
    }
    len.min(MAX_SAFE_LENGTH)
}

// ---------------------------------------------------------------------------
// §7.1.22 ToIndex
// ---------------------------------------------------------------------------

/// `ToIndex(value)` — §7.1.22.
///
/// 1. If _value_ is undefined, return 0.
/// 2. Let _integerIndex_ be ? ToIntegerOrInfinity(_value_).
/// 3. If _integerIndex_ < 0 or _integerIndex_ ≥ 2⁵³ - 1, throw RangeError.
#[must_use]
pub fn to_index(value: &JsValue) -> Result<f64, crate::vm::VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(0.0);
    }
    let index = to_integer_or_infinity(value);
    if index < 0.0 || index >= MAX_SAFE_LENGTH {
        return Err(crate::vm::VmError::range(
            "index must be an integer in [0, 2^53-1]",
        ));
    }
    Ok(index)
}

// ---------------------------------------------------------------------------
// §7.1.1  ToPrimitive  (stub — requires VM for Symbol.toPrimitive / valueOf / toString)
// ---------------------------------------------------------------------------

/// `ToPrimitive(input [, preferredType])` — §7.1.1.
///
/// **Stub**: for values that are already primitives or have simple
/// conversions. The full implementation (with `Symbol.toPrimitive`,
/// `valueOf`, and `toString` callbacks) lives in the VM because it may
/// execute JavaScript code.
#[must_use]
pub fn to_primitive_simple(value: &JsValue, _preferred: super::coercion::PreferredType) -> Option<JsValue> {
    match value {
        JsValue::Undefined
        | JsValue::Null
        | JsValue::Boolean(_)
        | JsValue::Number(_)
        | JsValue::String(_)
        | JsValue::Symbol(_)
        | JsValue::BigInt(_) => Some(value.clone()),
        _ => None, // Objects need VM callbacks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_integer_or_infinity_numbers() {
        assert_eq!(to_integer_or_infinity(&JsValue::Number(3.2)), 3.0);
        assert_eq!(to_integer_or_infinity(&JsValue::Number(3.9)), 3.0);
        assert_eq!(to_integer_or_infinity(&JsValue::Number(-3.2)), -3.0);
        assert_eq!(to_integer_or_infinity(&JsValue::Number(-0.0)), 0.0);
        assert_eq!(to_integer_or_infinity(&JsValue::Number(f64::NAN)), 0.0);
        assert_eq!(
            to_integer_or_infinity(&JsValue::Number(f64::INFINITY)),
            f64::INFINITY
        );
    }

    #[test]
    fn to_length_clamping() {
        assert_eq!(to_length(&JsValue::Number(-1.0)), 0.0);
        assert_eq!(to_length(&JsValue::Number(0.0)), 0.0);
        assert_eq!(to_length(&JsValue::Number(5.3)), 5.0);
        assert_eq!(
            to_length(&JsValue::Number(MAX_SAFE_LENGTH + 1.0)),
            MAX_SAFE_LENGTH
        );
    }

    #[test]
    fn to_index_default_and_errors() {
        assert_eq!(to_index(&JsValue::Undefined).unwrap(), 0.0);
        assert_eq!(to_index(&JsValue::Number(42.0)).unwrap(), 42.0);
        assert!(to_index(&JsValue::Number(-1.0)).is_err());
        assert!(to_index(&JsValue::Number(MAX_SAFE_LENGTH)).is_err());
    }
}
