//! Deterministic, dependency-free NumberFormat formatting primitives.

use super::NumberFormatRecord;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberFormatPart {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormattedNumber {
    pub text: String,
    pub parts: Vec<NumberFormatPart>,
}

#[derive(Debug, Clone, Copy)]
pub enum NumberValue<'a> {
    Number(f64),
    BigInt(&'a str),
    Decimal(&'a str),
}

pub fn format_number(record: &NumberFormatRecord, value: NumberValue<'_>) -> FormattedNumber {
    if let NumberValue::Decimal(text) = value
        && record.style == "decimal"
        && record.notation == "standard"
        && record.minimum_significant_digits.is_none()
        && record.maximum_significant_digits.is_none()
        && record.rounding_increment == 1
        && record.rounding_mode == "halfExpand"
        && record.rounding_priority == "auto"
        && let Some(formatted) = format_decimal(record, text)
    {
        return formatted;
    }
    let (negative, mut magnitude, special) = match value {
        NumberValue::Number(number) if number.is_nan() => (false, 0.0, Some("nan")),
        NumberValue::Number(number) if number.is_infinite() => {
            (number.is_sign_negative(), 0.0, Some("infinity"))
        }
        NumberValue::Number(number) => (number.is_sign_negative(), number.abs(), None),
        NumberValue::BigInt(text) => {
            let negative = text.starts_with('-');
            let digits = text.strip_prefix('-').unwrap_or(text);
            (
                negative,
                digits.parse::<f64>().unwrap_or(f64::INFINITY),
                None,
            )
        }
        NumberValue::Decimal(text) => {
            let number = text.parse::<f64>().unwrap_or(f64::NAN);
            if number.is_nan() {
                (false, 0.0, Some("nan"))
            } else if number.is_infinite() {
                (number.is_sign_negative(), 0.0, Some("infinity"))
            } else {
                (number.is_sign_negative(), number.abs(), None)
            }
        }
    };

    if let Some(kind) = special {
        let (mut prefix, accounting) = sign_prefix(record, negative, kind == "nan");
        let mut suffix = Vec::new();
        let value = if kind == "infinity" {
            "∞"
        } else if record.locale.starts_with("zh") {
            "非數值"
        } else {
            "NaN"
        };
        prefix.push(part(kind, value));
        if accounting {
            suffix.push(part("literal", ")"));
        }
        return finish(prefix, suffix);
    }

    if record.style == "percent" {
        magnitude *= 100.0;
    }
    if matches!(record.notation.as_str(), "scientific" | "engineering") && magnitude != 0.0 {
        let raw_exponent = magnitude.log10().floor() as i32;
        let exponent = if record.notation == "engineering" {
            raw_exponent.div_euclid(3) * 3
        } else {
            raw_exponent
        };
        let coefficient = magnitude / 10_f64.powi(exponent);
        let mut coefficient_record = record.clone();
        coefficient_record.style = "decimal".into();
        coefficient_record.notation = "standard".into();
        coefficient_record.use_grouping = "false".into();
        let mut formatted = format_number(
            &coefficient_record,
            NumberValue::Number(if negative { -coefficient } else { coefficient }),
        );
        formatted.parts.push(part("exponentSeparator", "E"));
        if exponent < 0 {
            formatted.parts.push(part("exponentMinusSign", "-"));
        }
        formatted
            .parts
            .push(part("exponentInteger", exponent.unsigned_abs().to_string()));
        formatted.parts.iter_mut().for_each(|item| {
            item.value = localize_digits(&item.value, &record.numbering_system);
        });
        formatted.text = formatted
            .parts
            .iter()
            .map(|item| item.value.as_str())
            .collect();
        return formatted;
    }
    let (compact_suffix, compact_separator) = if record.notation == "compact" {
        let (divisor, suffix, separator) = compact_pattern(record, magnitude);
        magnitude /= divisor;
        (suffix, separator)
    } else {
        (String::new(), "")
    };

    let integer_digit_count = if magnitude >= 1.0 {
        magnitude.log10().floor() as u8 + 1
    } else {
        1
    };
    let significant_min_fraction = record
        .minimum_significant_digits
        .map(|minimum_significant| {
            if magnitude == 0.0 {
                minimum_significant.saturating_sub(1)
            } else if magnitude < 1.0 {
                minimum_significant
                    .saturating_add((-magnitude.log10().floor() as i32 - 1).max(0) as u8)
            } else {
                minimum_significant.saturating_sub(integer_digit_count)
            }
        });
    let mut max_fraction = record.maximum_fraction_digits;
    if record.notation == "compact" && record.minimum_fraction_digits == 0 {
        max_fraction = max_fraction.min(1);
    }
    let significant_rounding_digits = record
        .maximum_significant_digits
        .or(record.minimum_significant_digits)
        .map(|max_significant| {
            if magnitude == 0.0 {
                i32::from(max_significant.saturating_sub(1))
            } else if magnitude < 1.0 {
                i32::from(max_significant) + (-magnitude.log10().floor() as i32 - 1).max(0)
            } else {
                i32::from(max_significant) - i32::from(integer_digit_count)
            }
        });
    let (rounding_digits, min_fraction) = if let Some(significant_digits) =
        significant_rounding_digits
    {
        match record.rounding_priority.as_str() {
            "morePrecision" if significant_digits < i32::from(max_fraction) => {
                (i32::from(max_fraction), record.minimum_fraction_digits)
            }
            "lessPrecision" if significant_digits > i32::from(max_fraction) => {
                (i32::from(max_fraction), record.minimum_fraction_digits)
            }
            _ => (significant_digits, significant_min_fraction.unwrap_or(0)),
        }
    } else if record.notation == "compact" && !(compact_suffix.is_empty() && magnitude >= 1_000.0) {
        let maximum_significant = if magnitude >= 1_000.0 {
            4
        } else if magnitude >= 100.0 {
            3
        } else {
            2
        };
        let digits = if magnitude == 0.0 {
            maximum_significant - 1
        } else if magnitude < 1.0 {
            maximum_significant + (-magnitude.log10().floor() as i32 - 1).max(0)
        } else {
            maximum_significant - i32::from(integer_digit_count)
        };
        (digits, record.minimum_fraction_digits)
    } else {
        (i32::from(max_fraction), record.minimum_fraction_digits)
    };
    let large_integer =
        if magnitude >= 1e15 && rounding_digits < 0 && record.rounding_increment == 1 {
            round_large_integer(
                magnitude,
                -rounding_digits as usize,
                &record.rounding_mode,
                negative,
            )
        } else {
            None
        };
    if large_integer.is_none() {
        magnitude = round_number(
            magnitude,
            rounding_digits,
            record.rounding_mode.as_str(),
            record.rounding_increment,
            negative,
        );
    }
    let (mut prefix, accounting) = sign_prefix(record, negative, magnitude == 0.0);
    let mut suffix = Vec::new();
    let fraction_digits = rounding_digits.max(0) as usize;
    let numeric = large_integer.unwrap_or_else(|| format_fixed(magnitude, fraction_digits));
    let (mut integer, mut fraction) = numeric
        .split_once('.')
        .map(|(left, right)| (left.to_string(), right.to_string()))
        .unwrap_or((numeric, String::new()));
    while fraction.len() > min_fraction as usize && fraction.ends_with('0') {
        fraction.pop();
    }
    if record.trailing_zero_display == "stripIfInteger" && fraction.chars().all(|ch| ch == '0') {
        fraction.clear();
    }
    if integer.len() < record.minimum_integer_digits as usize {
        integer = format!(
            "{}{}",
            "0".repeat(record.minimum_integer_digits as usize - integer.len()),
            integer
        );
    }

    let use_grouping = match record.use_grouping.as_str() {
        "false" => false,
        "min2" => integer.len() >= 5,
        _ => true,
    };
    let (group, decimal) = separators(&record.locale, &record.numbering_system);
    let grouped = if use_grouping {
        group_integer(&integer, &record.locale, group)
    } else {
        integer
    };
    prefix.extend(style_prefix(record));
    let mut parts = Vec::new();
    if let Some(unit_prefix) = unit_long_prefix(record) {
        parts.push(part("unit", unit_prefix));
        parts.push(part("literal", " "));
    }
    parts.extend(prefix);
    parts.extend(number_parts(&grouped, &fraction, decimal));
    if !compact_suffix.is_empty() {
        if !compact_separator.is_empty() {
            suffix.push(part("literal", compact_separator));
        }
        suffix.push(part("compact", compact_suffix));
    }
    if record.style == "percent" {
        suffix.push(part(
            "percentSign",
            if record.locale.starts_with("de") {
                " %"
            } else {
                "%"
            },
        ));
    }
    suffix.extend(style_suffix(record, magnitude));
    if accounting {
        suffix.push(part("literal", ")"));
    }
    parts.extend(suffix);
    let mut output = finish(parts, Vec::new());
    output.parts.iter_mut().for_each(|item| {
        item.value = localize_digits(&item.value, &record.numbering_system);
    });
    output.text = output
        .parts
        .iter()
        .map(|item| item.value.as_str())
        .collect();
    output
}

fn format_decimal(record: &NumberFormatRecord, text: &str) -> Option<FormattedNumber> {
    let (negative, unsigned) = text
        .strip_prefix('-')
        .map_or((false, text), |value| (true, value));
    let unsigned = unsigned.strip_prefix('+').unwrap_or(unsigned);
    if unsigned.is_empty()
        || unsigned.contains(['e', 'E'])
        || !unsigned.chars().all(|ch| ch.is_ascii_digit() || ch == '.')
        || unsigned.chars().filter(|&ch| ch == '.').count() > 1
    {
        return None;
    }
    let (integer, fraction) = unsigned
        .split_once('.')
        .map_or((unsigned, ""), |(integer, fraction)| (integer, fraction));
    let mut integer = integer.trim_start_matches('0').to_string();
    if integer.is_empty() {
        integer.push('0');
    }
    let mut fraction = fraction.to_string();
    let maximum = record.maximum_fraction_digits as usize;
    if fraction.len() > maximum {
        let round_up = fraction
            .as_bytes()
            .get(maximum)
            .is_some_and(|digit| *digit >= b'5');
        fraction.truncate(maximum);
        if round_up {
            increment_decimal(&mut integer, &mut fraction);
        }
    }
    while fraction.len() > record.minimum_fraction_digits as usize && fraction.ends_with('0') {
        fraction.pop();
    }
    while fraction.len() < record.minimum_fraction_digits as usize {
        fraction.push('0');
    }
    if integer.len() < record.minimum_integer_digits as usize {
        integer = format!(
            "{}{}",
            "0".repeat(record.minimum_integer_digits as usize - integer.len()),
            integer
        );
    }
    let is_zero = integer.chars().all(|ch| ch == '0') && fraction.chars().all(|ch| ch == '0');
    let (prefix, accounting) = sign_prefix(record, negative, is_zero);
    let (group, decimal) = separators(&record.locale, &record.numbering_system);
    let grouped = if record.use_grouping == "false" {
        integer
    } else {
        group_integer(&integer, &record.locale, group)
    };
    let mut parts = prefix;
    parts.extend(number_parts(&grouped, &fraction, decimal));
    if accounting {
        parts.push(part("literal", ")"));
    }
    let mut formatted = finish(parts, Vec::new());
    formatted.parts.iter_mut().for_each(|item| {
        item.value = localize_digits(&item.value, &record.numbering_system);
    });
    formatted.text = formatted
        .parts
        .iter()
        .map(|item| item.value.as_str())
        .collect();
    Some(formatted)
}

fn increment_decimal(integer: &mut String, fraction: &mut String) {
    let mut digits: Vec<u8> = fraction.bytes().collect();
    let mut carry = true;
    for digit in digits.iter_mut().rev() {
        if !carry {
            break;
        }
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            carry = false;
        }
    }
    *fraction = String::from_utf8(digits).expect("decimal digits are ASCII");
    if carry {
        let mut digits: Vec<u8> = integer.bytes().collect();
        for digit in digits.iter_mut().rev() {
            if *digit == b'9' {
                *digit = b'0';
            } else {
                *digit += 1;
                carry = false;
                break;
            }
        }
        if carry {
            digits.insert(0, b'1');
        }
        *integer = String::from_utf8(digits).expect("decimal digits are ASCII");
    }
}

fn compact_pattern(record: &NumberFormatRecord, magnitude: f64) -> (f64, String, &'static str) {
    let locale = record.locale.as_str();
    if locale.starts_with("ja") || locale.starts_with("zh") {
        return if magnitude >= 100_000_000.0 {
            (
                100_000_000.0,
                if locale.starts_with("zh-CN") {
                    "亿"
                } else {
                    "億"
                }
                .into(),
                "",
            )
        } else if magnitude >= 10_000.0 {
            (
                10_000.0,
                if locale.starts_with("zh") {
                    "萬"
                } else {
                    "万"
                }
                .into(),
                "",
            )
        } else {
            (1.0, String::new(), "")
        };
    }
    if locale.starts_with("ko") {
        return if magnitude >= 100_000_000.0 {
            (100_000_000.0, "억".into(), "")
        } else if magnitude >= 10_000.0 {
            (10_000.0, "만".into(), "")
        } else if magnitude >= 1_000.0 {
            (1_000.0, "천".into(), "")
        } else {
            (1.0, String::new(), "")
        };
    }
    if locale.starts_with("de") {
        if magnitude >= 1_000_000.0 {
            return (
                1_000_000.0,
                if record.compact_display == "long" {
                    "Millionen"
                } else {
                    "Mio."
                }
                .into(),
                if record.compact_display == "long" {
                    " "
                } else {
                    " "
                },
            );
        }
        if magnitude >= 1_000.0 && record.compact_display == "long" {
            return (1_000.0, "Tausend".into(), " ");
        }
        return (1.0, String::new(), "");
    }
    if locale.starts_with("en-IN") || locale.starts_with("hi-IN") {
        return if magnitude >= 10_000_000.0 {
            (10_000_000.0, "Cr".into(), "")
        } else if magnitude >= 100_000.0 {
            (100_000.0, "L".into(), "")
        } else if magnitude >= 1_000.0 {
            (1_000.0, "K".into(), "")
        } else {
            (1.0, String::new(), "")
        };
    }
    if magnitude >= 1_000_000_000.0 {
        (
            1_000_000_000.0,
            if record.compact_display == "long" {
                "billion"
            } else {
                "B"
            }
            .into(),
            if record.compact_display == "long" {
                " "
            } else {
                ""
            },
        )
    } else if magnitude >= 1_000_000.0 {
        (
            1_000_000.0,
            if record.compact_display == "long" {
                "million"
            } else {
                "M"
            }
            .into(),
            if record.compact_display == "long" {
                " "
            } else {
                ""
            },
        )
    } else if magnitude >= 1_000.0 {
        (
            1_000.0,
            if record.compact_display == "long" {
                "thousand"
            } else {
                "K"
            }
            .into(),
            if record.compact_display == "long" {
                " "
            } else {
                ""
            },
        )
    } else {
        (1.0, String::new(), "")
    }
}

fn part(kind: impl Into<String>, value: impl Into<String>) -> NumberFormatPart {
    NumberFormatPart {
        kind: kind.into(),
        value: value.into(),
    }
}

fn finish(mut prefix: Vec<NumberFormatPart>, suffix: Vec<NumberFormatPart>) -> FormattedNumber {
    prefix.extend(suffix);
    let text = prefix.iter().map(|item| item.value.as_str()).collect();
    FormattedNumber {
        text,
        parts: prefix,
    }
}

fn format_fixed(value: f64, fraction_digits: usize) -> String {
    if value < 1e15 {
        return format!("{value:.fraction_digits$}");
    }
    let mut integer = expand_exponent(&value.to_string());
    if let Some((whole, _)) = integer.split_once('.') {
        integer = whole.to_string();
    }
    if fraction_digits == 0 {
        integer
    } else {
        format!("{integer}.{}", "0".repeat(fraction_digits))
    }
}

fn expand_exponent(value: &str) -> String {
    let Some((mantissa, exponent)) = value.split_once(['e', 'E']) else {
        return value.into();
    };
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    let mut digits = mantissa.replace('.', "");
    let decimal_index = mantissa.find('.').map_or(mantissa.len(), |index| index) as i32;
    let target = decimal_index + exponent;
    if target <= 0 {
        format!("0.{}{}", "0".repeat((-target) as usize), digits)
    } else if target as usize >= digits.len() {
        digits.push_str(&"0".repeat(target as usize - digits.len()));
        digits
    } else {
        digits.insert(target as usize, '.');
        digits
    }
}

fn style_prefix(record: &NumberFormatRecord) -> Vec<NumberFormatPart> {
    if record.style != "currency" {
        return Vec::new();
    }
    let currency = currency_label(
        record.currency.as_deref().unwrap_or("USD"),
        &record.currency_display,
    );
    if record.currency_display == "name" || currency_uses_suffix(&record.locale) {
        Vec::new()
    } else if (record.locale.starts_with("zh") || record.locale.starts_with("ko"))
        && currency == "$"
    {
        vec![part("currency", "US$")]
    } else {
        vec![part("currency", currency)]
    }
}

fn style_suffix(record: &NumberFormatRecord, magnitude: f64) -> Vec<NumberFormatPart> {
    match record.style.as_str() {
        "currency" => {
            let currency = currency_label(
                record.currency.as_deref().unwrap_or("USD"),
                &record.currency_display,
            );
            if record.currency_display == "name" {
                let name = currency_name(
                    record.currency.as_deref().unwrap_or("USD"),
                    record.locale.as_str(),
                    magnitude == 1.0,
                );
                vec![part("literal", " "), part("currency", name)]
            } else if currency_uses_suffix(&record.locale) || record.currency_display == "code" {
                vec![part("literal", " "), part("currency", currency)]
            } else {
                Vec::new()
            }
        }
        "unit" => {
            let unit = record.unit.as_deref().unwrap_or("");
            let label = unit_label(
                unit,
                &record.unit_display,
                record.locale.as_str(),
                magnitude == 1.0,
            );
            let separated = unit != "percent"
                && !((record.unit_display == "narrow" && !record.locale.starts_with("de"))
                    || (matches!(record.unit_display.as_str(), "short" | "long")
                        && record.locale.starts_with("ko")));
            if separated {
                vec![part("literal", " "), part("unit", label)]
            } else {
                vec![part("unit", label)]
            }
        }
        _ => Vec::new(),
    }
}

fn currency_uses_suffix(locale: &str) -> bool {
    locale.starts_with("de")
        || locale.starts_with("es")
        || locale.starts_with("it")
        || locale.starts_with("pt")
}

fn unit_long_prefix(record: &NumberFormatRecord) -> Option<&'static str> {
    if record.style != "unit"
        || record.unit_display != "long"
        || record.unit.as_deref() != Some("kilometer-per-hour")
    {
        return None;
    }
    if record.locale.starts_with("ja") {
        Some("時速")
    } else if record.locale.starts_with("ko") {
        Some("시속")
    } else if record.locale.starts_with("zh") {
        Some("每小時")
    } else {
        None
    }
}

fn number_parts(integer: &str, fraction: &str, decimal: &str) -> Vec<NumberFormatPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for ch in integer.chars() {
        if ch == ',' || ch == '.' || ch == '٬' || ch == '٫' {
            if !current.is_empty() {
                parts.push(part("integer", std::mem::take(&mut current)));
            }
            parts.push(part("group", ch.to_string()));
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        parts.push(part("integer", current));
    }
    if !fraction.is_empty() {
        parts.push(part("decimal", decimal));
        parts.push(part("fraction", fraction));
    }
    parts
}

fn sign_prefix(
    record: &NumberFormatRecord,
    negative: bool,
    is_zero: bool,
) -> (Vec<NumberFormatPart>, bool) {
    let show_negative = negative
        && match record.sign_display.as_str() {
            "never" => false,
            "negative" | "exceptZero" => !is_zero,
            _ => true,
        };
    let show_plus = !negative
        && matches!(record.sign_display.as_str(), "always" | "exceptZero")
        && (record.sign_display == "always" || !is_zero);
    let accounting = show_negative
        && record.style == "currency"
        && record.currency_sign == "accounting"
        && !record.locale.starts_with("de");
    let parts = if accounting {
        vec![part("literal", "(")]
    } else if show_negative {
        vec![part("minusSign", "-")]
    } else if show_plus {
        vec![part("plusSign", "+")]
    } else {
        Vec::new()
    };
    (parts, accounting)
}

fn round_large_integer(
    value: f64,
    remove_digits: usize,
    mode: &str,
    negative: bool,
) -> Option<String> {
    let expanded = expand_exponent(&value.to_string());
    let integer = expanded.split('.').next()?.trim_start_matches('0');
    if integer.is_empty() || !integer.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    if remove_digits == 0 {
        return Some(integer.into());
    }

    let keep = integer.len().saturating_sub(remove_digits);
    let (kept, removed) = integer.split_at(keep);
    let removed_nonzero = removed.bytes().any(|digit| digit != b'0');
    let first = removed.bytes().next().unwrap_or(b'0');
    let after_first_nonzero = removed.bytes().skip(1).any(|digit| digit != b'0');
    let greater_than_half = first > b'5' || (first == b'5' && after_first_nonzero);
    let exactly_half = first == b'5' && !after_first_nonzero;
    let last_kept_is_odd = kept
        .bytes()
        .last()
        .is_some_and(|digit| !(digit - b'0').is_multiple_of(2));
    let round_up = if !removed_nonzero {
        false
    } else {
        match mode {
            "ceil" => !negative,
            "floor" => negative,
            "expand" => true,
            "trunc" => false,
            "halfCeil" => greater_than_half || (exactly_half && !negative),
            "halfFloor" => greater_than_half || (exactly_half && negative),
            "halfTrunc" => greater_than_half,
            "halfEven" => greater_than_half || (exactly_half && last_kept_is_odd),
            _ => greater_than_half || exactly_half,
        }
    };

    let mut prefix = if kept.is_empty() {
        "0".into()
    } else {
        kept.into()
    };
    if round_up {
        increment_integer(&mut prefix);
    }
    let zeros = if prefix.len() > keep {
        remove_digits.saturating_sub(prefix.len() - keep)
    } else {
        remove_digits
    };
    prefix.push_str(&"0".repeat(zeros));
    Some(prefix)
}

fn increment_integer(integer: &mut String) {
    let mut digits: Vec<u8> = integer.bytes().collect();
    let mut carry = true;
    for digit in digits.iter_mut().rev() {
        if *digit == b'9' {
            *digit = b'0';
        } else {
            *digit += 1;
            carry = false;
            break;
        }
    }
    if carry {
        digits.insert(0, b'1');
    }
    *integer = String::from_utf8(digits).expect("decimal digits are ASCII");
}

fn round_number(value: f64, digits: i32, mode: &str, increment: u16, negative: bool) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let scale = 10_f64.powi(digits);
    let increment = f64::from(increment.max(1));
    let scaled = value * scale / increment * if negative { -1.0 } else { 1.0 };
    let floor = scaled.floor();
    let tie = ((scaled - floor) - 0.5).abs() <= 1e-10;
    let rounded = match mode {
        "ceil" => scaled.ceil(),
        "floor" => scaled.floor(),
        "trunc" => scaled.trunc(),
        "expand" => {
            if negative {
                scaled.floor()
            } else {
                scaled.ceil()
            }
        }
        "halfFloor" => {
            if tie {
                scaled.floor()
            } else {
                scaled.round()
            }
        }
        "halfCeil" => {
            if tie {
                scaled.ceil()
            } else {
                scaled.round()
            }
        }
        "halfTrunc" => {
            if tie {
                scaled.trunc()
            } else {
                scaled.round()
            }
        }
        "halfEven" if tie => {
            if (floor as i128).rem_euclid(2) == 0 {
                floor
            } else {
                floor + 1.0
            }
        }
        _ if tie => {
            if negative {
                scaled.floor()
            } else {
                scaled.ceil()
            }
        }
        _ => scaled.round(),
    };
    (rounded * increment / scale).abs()
}

fn group_integer(integer: &str, locale: &str, separator: &str) -> String {
    let mut out = String::new();
    let indian = locale.starts_with("en-IN") || locale.starts_with("hi-IN");
    for (index, ch) in integer.chars().enumerate() {
        let remaining = integer.len() - index;
        let insert = if indian {
            index > 0 && remaining >= 3 && (remaining - 3).is_multiple_of(2)
        } else {
            index > 0 && remaining.is_multiple_of(3)
        };
        if insert {
            out.push_str(separator);
        }
        out.push(ch);
    }
    out
}

fn separators(locale: &str, numbering: &str) -> (&'static str, &'static str) {
    if numbering == "arab" || numbering == "arabext" {
        return ("٬", "٫");
    }
    if locale.starts_with("pt") {
        (" ", ",")
    } else if locale.starts_with("de") || locale.starts_with("es") || locale.starts_with("it") {
        (".", ",")
    } else {
        (",", ".")
    }
}

fn localize_digits(value: &str, numbering: &str) -> String {
    if numbering == "hanidec" {
        const DIGITS: &str = "〇一二三四五六七八九";
        return value
            .chars()
            .map(|ch| {
                ch.to_digit(10)
                    .and_then(|digit| DIGITS.chars().nth(digit as usize))
                    .unwrap_or(ch)
            })
            .collect();
    }
    let Some(zero) = numbering_system_zero(numbering) else {
        return value.into();
    };
    value
        .chars()
        .map(|ch| {
            ch.to_digit(10)
                .and_then(|digit| char::from_u32(zero + digit))
                .unwrap_or(ch)
        })
        .collect()
}

fn numbering_system_zero(numbering: &str) -> Option<u32> {
    Some(match numbering {
        "latn" => 0x0030,
        "arab" => 0x0660,
        "arabext" => 0x06F0,
        "adlm" => 0x1E950,
        "ahom" => 0x11730,
        "bali" => 0x1B50,
        "beng" => 0x09E6,
        "bhks" => 0x11C50,
        "brah" => 0x11066,
        "cakm" => 0x11136,
        "cham" => 0xAA50,
        "deva" => 0x0966,
        "diak" => 0x11950,
        "fullwide" => 0xFF10,
        "gara" => 0x10D40,
        "gong" => 0x11DA0,
        "gonm" => 0x11D50,
        "gujr" => 0x0AE6,
        "gukh" => 0x16130,
        "guru" => 0x0A66,
        "hmng" => 0x16B50,
        "hmnp" => 0x1E140,
        "java" => 0xA9D0,
        "kali" => 0xA900,
        "kawi" => 0x11F50,
        "khmr" => 0x17E0,
        "knda" => 0x0CE6,
        "krai" => 0x16D70,
        "lana" => 0x1A80,
        "lanatham" => 0x1A90,
        "laoo" => 0x0ED0,
        "lepc" => 0x1C40,
        "limb" => 0x1946,
        "mathbold" => 0x1D7CE,
        "mathdbl" => 0x1D7D8,
        "mathmono" => 0x1D7F6,
        "mathsanb" => 0x1D7EC,
        "mathsans" => 0x1D7E2,
        "mlym" => 0x0D66,
        "modi" => 0x11650,
        "mong" => 0x1810,
        "mroo" => 0x16A60,
        "mtei" => 0xABF0,
        "mymr" => 0x1040,
        "mymrepka" => 0x116DA,
        "mymrpao" => 0x116D0,
        "mymrshan" => 0x1090,
        "mymrtlng" => 0xA9F0,
        "nagm" => 0x1E4F0,
        "newa" => 0x11450,
        "nkoo" => 0x07C0,
        "olck" => 0x1C50,
        "onao" => 0x1E5F1,
        "orya" => 0x0B66,
        "osma" => 0x104A0,
        "outlined" => 0x1CCF0,
        "rohg" => 0x10D30,
        "saur" => 0xA8D0,
        "segment" => 0x1FBF0,
        "shrd" => 0x111D0,
        "sind" => 0x112F0,
        "sinh" => 0x0DE6,
        "sora" => 0x110F0,
        "sund" => 0x1BB0,
        "sunu" => 0x11BF0,
        "takr" => 0x116C0,
        "talu" => 0x19D0,
        "tamldec" => 0x0BE6,
        "telu" => 0x0C66,
        "thai" => 0x0E50,
        "tibt" => 0x0F20,
        "tirh" => 0x114D0,
        "tnsa" => 0x16AC0,
        "tols" => 0x11DE0,
        "vaii" => 0xA620,
        "wara" => 0x118E0,
        "wcho" => 0x1E2F0,
        _ => return None,
    })
}

fn currency_label(currency: &str, display: &str) -> String {
    if display == "code" {
        currency.into()
    } else {
        match currency {
            "USD" => "$",
            "EUR" => "€",
            "JPY" => "¥",
            "INR" => "₹",
            "CNY" => "CN¥",
            "KRW" => "₩",
            "GBP" => "£",
            _ => currency,
        }
        .into()
    }
}

fn currency_name(currency: &str, locale: &str, singular: bool) -> String {
    match (currency, locale.starts_with("de"), singular) {
        ("USD", true, _) => "US-Dollar",
        ("EUR", true, _) => "Euro",
        ("JPY", true, _) => "Japanische Yen",
        ("USD", false, true) => "US dollar",
        ("USD", false, false) => "US dollars",
        ("EUR", false, true) => "euro",
        ("EUR", false, false) => "euros",
        ("JPY", false, _) => "Japanese yen",
        _ => currency,
    }
    .into()
}

fn unit_label(unit: &str, display: &str, locale: &str, singular: bool) -> String {
    if display == "narrow" || display == "short" {
        return match unit {
            "percent" => "%".into(),
            "meter" => "m".into(),
            "kilometer" => "km".into(),
            "centimeter" => "cm".into(),
            "millimeter" => "mm".into(),
            "liter" => "L".into(),
            "kilogram" => "kg".into(),
            "gram" => "g".into(),
            "celsius" => "°C".into(),
            "fahrenheit" => "°F".into(),
            "hour" => "hr".into(),
            "minute" => "min".into(),
            "second" => "sec".into(),
            "kilometer-per-hour" if locale.starts_with("zh") => "公里/小時".into(),
            "kilometer-per-hour" => "km/h".into(),
            _ => unit.replace('-', " "),
        };
    }
    if unit == "kilometer-per-hour" {
        return if locale.starts_with("de") {
            "Kilometer pro Stunde".into()
        } else if locale.starts_with("zh") {
            "公里".into()
        } else if locale.starts_with("ko") {
            "킬로미터".into()
        } else if locale.starts_with("ja") {
            "キロメートル".into()
        } else if singular {
            "kilometer per hour".into()
        } else {
            "kilometers per hour".into()
        };
    }
    let base = unit.replace('-', " ");
    if locale.starts_with("de") {
        return match unit {
            "meter" => "Meter".into(),
            "kilometer" => "Kilometer".into(),
            "liter" => "Liter".into(),
            "hour" => "Stunde".into(),
            "day" => "Tag".into(),
            _ => base,
        };
    }
    if singular { base } else { format!("{}s", base) }
}
