//! UTF-16 algorithms used by the V6 `String` builtins.
//!
//! This module intentionally contains no VM, runtime, or installer wiring.
//! C1 can therefore be developed and tested without modifying the C0 runtime
//! files or the integration-owned `builtins/mod.rs`. Once C0 lands, the thin
//! builtin adapters should coerce their inputs and delegate to these helpers.

use std::fmt;

/// Conservative allocation guard for operations such as `repeat` and padding.
pub(crate) const MAX_STRING_CODE_UNITS: usize = 1 << 28;

/// Metadata frozen for the V6 String installer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StringMethodSpec {
    pub name: &'static str,
    pub length: u8,
}

pub(crate) const STATIC_METHODS: &[StringMethodSpec] = &[
    StringMethodSpec {
        name: "fromCharCode",
        length: 1,
    },
    StringMethodSpec {
        name: "fromCodePoint",
        length: 1,
    },
];

pub(crate) const PROTOTYPE_METHODS: &[StringMethodSpec] = &[
    StringMethodSpec {
        name: "toString",
        length: 0,
    },
    StringMethodSpec {
        name: "valueOf",
        length: 0,
    },
    StringMethodSpec {
        name: "charAt",
        length: 1,
    },
    StringMethodSpec {
        name: "charCodeAt",
        length: 1,
    },
    StringMethodSpec {
        name: "at",
        length: 1,
    },
    StringMethodSpec {
        name: "concat",
        length: 1,
    },
    StringMethodSpec {
        name: "includes",
        length: 1,
    },
    StringMethodSpec {
        name: "indexOf",
        length: 1,
    },
    StringMethodSpec {
        name: "lastIndexOf",
        length: 1,
    },
    StringMethodSpec {
        name: "slice",
        length: 2,
    },
    StringMethodSpec {
        name: "substring",
        length: 2,
    },
    StringMethodSpec {
        name: "substr",
        length: 2,
    },
    StringMethodSpec {
        name: "startsWith",
        length: 1,
    },
    StringMethodSpec {
        name: "endsWith",
        length: 1,
    },
    StringMethodSpec {
        name: "repeat",
        length: 1,
    },
    StringMethodSpec {
        name: "padStart",
        length: 1,
    },
    StringMethodSpec {
        name: "padEnd",
        length: 1,
    },
    StringMethodSpec {
        name: "trim",
        length: 0,
    },
    StringMethodSpec {
        name: "trimStart",
        length: 0,
    },
    StringMethodSpec {
        name: "trimEnd",
        length: 0,
    },
    StringMethodSpec {
        name: "toLowerCase",
        length: 0,
    },
    StringMethodSpec {
        name: "toUpperCase",
        length: 0,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StringBuiltinError {
    InvalidCodePoint(u32),
    InvalidRepeatCount,
    AllocationLimit,
}

impl fmt::Display for StringBuiltinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCodePoint(value) => {
                write!(formatter, "{value} is not a valid Unicode code point")
            }
            Self::InvalidRepeatCount => formatter.write_str("invalid string repeat count"),
            Self::AllocationLimit => formatter.write_str("string allocation limit exceeded"),
        }
    }
}

pub(crate) fn utf16_units(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

pub(crate) fn utf16_length(value: &str) -> usize {
    value.encode_utf16().count()
}

pub(crate) fn utf16_code_unit_at(value: &str, index: usize) -> Option<u16> {
    value.encode_utf16().nth(index)
}

pub(crate) fn utf16_slice(value: &str, start: usize, end: usize) -> String {
    let units = utf16_units(value);
    decode_utf16(&units[start.min(units.len())..end.min(units.len()).max(start.min(units.len()))])
}

pub(crate) fn char_at(value: &str, index: i64) -> String {
    let Ok(index) = usize::try_from(index) else {
        return String::new();
    };
    utf16_code_unit_at(value, index).map_or_else(String::new, |unit| decode_utf16(&[unit]))
}

pub(crate) fn char_code_at(value: &str, index: i64) -> Option<u16> {
    usize::try_from(index)
        .ok()
        .and_then(|index| utf16_code_unit_at(value, index))
}

pub(crate) fn at(value: &str, index: i64) -> Option<String> {
    let units = utf16_units(value);
    let index = relative_index(index, units.len())?;
    Some(decode_utf16(&[units[index]]))
}

pub(crate) fn concat(value: &str, values: &[&str]) -> String {
    let additional = values.iter().map(|value| value.len()).sum::<usize>();
    let mut result = String::with_capacity(value.len().saturating_add(additional));
    result.push_str(value);
    for value in values {
        result.push_str(value);
    }
    result
}

pub(crate) fn includes(value: &str, search: &str, position: i64) -> bool {
    index_of(value, search, position).is_some()
}

pub(crate) fn index_of(value: &str, search: &str, position: i64) -> Option<usize> {
    let source = utf16_units(value);
    let search = utf16_units(search);
    let start = clamp_index(position, source.len());
    find_units(&source, &search, start)
}

pub(crate) fn last_index_of(value: &str, search: &str, position: Option<i64>) -> Option<usize> {
    let source = utf16_units(value);
    let search = utf16_units(search);
    let start = position.map_or(source.len(), |position| clamp_index(position, source.len()));
    rfind_units(&source, &search, start)
}

pub(crate) fn slice(value: &str, start: i64, end: Option<i64>) -> String {
    let units = utf16_units(value);
    let start = normalize_relative_bound(start, units.len());
    let end = end.map_or(units.len(), |end| {
        normalize_relative_bound(end, units.len())
    });
    if end <= start {
        String::new()
    } else {
        decode_utf16(&units[start..end])
    }
}

pub(crate) fn substring(value: &str, start: i64, end: Option<i64>) -> String {
    let units = utf16_units(value);
    let mut start = clamp_index(start, units.len());
    let mut end = end.map_or(units.len(), |end| clamp_index(end, units.len()));
    if start > end {
        std::mem::swap(&mut start, &mut end);
    }
    decode_utf16(&units[start..end])
}

pub(crate) fn substr(value: &str, start: i64, length: Option<i64>) -> String {
    let units = utf16_units(value);
    let start = normalize_relative_bound(start, units.len());
    let count = length.map_or(units.len().saturating_sub(start), |length| {
        usize::try_from(length.max(0)).unwrap_or(usize::MAX)
    });
    let end = start.saturating_add(count).min(units.len());
    decode_utf16(&units[start..end])
}

pub(crate) fn starts_with(value: &str, search: &str, position: i64) -> bool {
    let source = utf16_units(value);
    let search = utf16_units(search);
    let start = clamp_index(position, source.len());
    source
        .get(start..start.saturating_add(search.len()))
        .is_some_and(|candidate| candidate == search)
}

pub(crate) fn ends_with(value: &str, search: &str, end_position: Option<i64>) -> bool {
    let source = utf16_units(value);
    let search = utf16_units(search);
    let end = end_position.map_or(source.len(), |end| clamp_index(end, source.len()));
    let Some(start) = end.checked_sub(search.len()) else {
        return false;
    };
    source
        .get(start..end)
        .is_some_and(|candidate| candidate == search)
}

pub(crate) fn repeat(value: &str, count: i64) -> Result<String, StringBuiltinError> {
    let count = usize::try_from(count).map_err(|_| StringBuiltinError::InvalidRepeatCount)?;
    let result_units = utf16_length(value)
        .checked_mul(count)
        .ok_or(StringBuiltinError::AllocationLimit)?;
    if result_units > MAX_STRING_CODE_UNITS {
        return Err(StringBuiltinError::AllocationLimit);
    }
    Ok(value.repeat(count))
}

pub(crate) fn pad_start(
    value: &str,
    target_length: usize,
    fill: &str,
) -> Result<String, StringBuiltinError> {
    pad(value, target_length, fill, true)
}

pub(crate) fn pad_end(
    value: &str,
    target_length: usize,
    fill: &str,
) -> Result<String, StringBuiltinError> {
    pad(value, target_length, fill, false)
}

pub(crate) fn trim(value: &str) -> String {
    value.trim_matches(is_ecmascript_whitespace).to_string()
}

pub(crate) fn trim_start(value: &str) -> String {
    value
        .trim_start_matches(is_ecmascript_whitespace)
        .to_string()
}

pub(crate) fn trim_end(value: &str) -> String {
    value.trim_end_matches(is_ecmascript_whitespace).to_string()
}

pub(crate) fn to_lower_case(value: &str) -> String {
    value.to_lowercase()
}

pub(crate) fn to_upper_case(value: &str) -> String {
    value.to_uppercase()
}

/// Produces the exact ECMAScript UTF-16 sequence for `String.fromCharCode`.
pub(crate) fn from_char_codes(values: &[u16]) -> Vec<u16> {
    values.to_vec()
}

/// Produces the exact ECMAScript UTF-16 sequence for `String.fromCodePoint`.
pub(crate) fn from_code_points(values: &[u32]) -> Result<Vec<u16>, StringBuiltinError> {
    let mut result = Vec::with_capacity(values.len());
    for value in values {
        match *value {
            0..=0xFFFF => result.push(*value as u16),
            0x10000..=0x10FFFF => {
                let value = *value - 0x10000;
                result.push(0xD800 | ((value >> 10) as u16));
                result.push(0xDC00 | ((value & 0x3FF) as u16));
            }
            value => return Err(StringBuiltinError::InvalidCodePoint(value)),
        }
    }
    Ok(result)
}

/// Converts UTF-16 to the current Rust `String` storage representation.
///
/// Valid scalar sequences are preserved. Lone surrogates become U+FFFD because
/// Rust strings cannot store them; the C0 integration must keep the UTF-16
/// sequence until the engine adopts a lossless ECMAScript string storage.
pub(crate) fn decode_utf16(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

fn pad(
    value: &str,
    target_length: usize,
    fill: &str,
    at_start: bool,
) -> Result<String, StringBuiltinError> {
    let value_units = utf16_units(value);
    if target_length <= value_units.len() || fill.is_empty() {
        return Ok(value.to_string());
    }
    if target_length > MAX_STRING_CODE_UNITS {
        return Err(StringBuiltinError::AllocationLimit);
    }

    let fill_units = utf16_units(fill);
    let required = target_length - value_units.len();
    let mut padding = Vec::with_capacity(required);
    while padding.len() < required {
        let remaining = required - padding.len();
        padding.extend(fill_units.iter().copied().take(remaining));
    }

    let mut result = Vec::with_capacity(target_length);
    if at_start {
        result.extend(padding);
        result.extend(value_units);
    } else {
        result.extend(value_units);
        result.extend(padding);
    }
    Ok(decode_utf16(&result))
}

fn find_units(source: &[u16], search: &[u16], start: usize) -> Option<usize> {
    if search.is_empty() {
        return Some(start.min(source.len()));
    }
    if search.len() > source.len() || start > source.len().saturating_sub(search.len()) {
        return None;
    }
    (start..=source.len() - search.len()).find(|index| {
        source
            .get(*index..*index + search.len())
            .is_some_and(|candidate| candidate == search)
    })
}

fn rfind_units(source: &[u16], search: &[u16], start: usize) -> Option<usize> {
    if search.is_empty() {
        return Some(start.min(source.len()));
    }
    if search.len() > source.len() {
        return None;
    }
    let last_start = start.min(source.len() - search.len());
    (0..=last_start).rev().find(|index| {
        source
            .get(*index..*index + search.len())
            .is_some_and(|candidate| candidate == search)
    })
}

fn relative_index(index: i64, length: usize) -> Option<usize> {
    let length = i128::try_from(length).ok()?;
    let index = i128::from(index);
    let index = if index < 0 { length + index } else { index };
    if !(0..length).contains(&index) {
        None
    } else {
        usize::try_from(index).ok()
    }
}

fn normalize_relative_bound(index: i64, length: usize) -> usize {
    let length_i128 = i128::try_from(length).unwrap_or(i128::MAX);
    let index = i128::from(index);
    let normalized = if index < 0 {
        (length_i128 + index).max(0)
    } else {
        index.min(length_i128)
    };
    usize::try_from(normalized).unwrap_or(length)
}

fn clamp_index(index: i64, length: usize) -> usize {
    if index <= 0 {
        0
    } else {
        usize::try_from(index).unwrap_or(usize::MAX).min(length)
    }
}

fn is_ecmascript_whitespace(character: char) -> bool {
    character.is_whitespace() || character == '\u{FEFF}'
}
