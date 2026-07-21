//! Date / Intl / Temporal built-ins.
//!
//! The implementation is intentionally a deterministic UTC-oriented subset.
//! It installs real JS-visible constructors, prototypes, descriptors, and a
//! small core of algorithms without trying to replace ICU or full Temporal.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    runtime::{
        JsObject, JsValue, NativeCall, NativeConstruct, NativeContext, ObjectId, PreferredType,
        PrimitiveValue, PropertyDescriptor, PropertyKind, bigint,
    },
    vm::{Vm, VmError},
};

const DATE_VALUE: &str = "__agentjs_date_value__";
const DATE_MARKER: &str = "__agentjs_date__";
const INTL_KIND: &str = "__agentjs_intl_kind__";
const INTL_LOCALE: &str = "__agentjs_intl_locale__";
const TEMPORAL_KIND: &str = "__agentjs_temporal_kind__";

const MS_PER_SECOND: f64 = 1_000.0;
const MS_PER_MINUTE: f64 = 60_000.0;
const MS_PER_HOUR: f64 = 3_600_000.0;
const MS_PER_DAY: f64 = 86_400_000.0;
const MAX_TIME_VALUE: f64 = 8_640_000_000_000_000.0;
const NS_PER_MILLISECOND_I128: i128 = 1_000_000;
const NS_PER_SECOND_I128: i128 = 1_000_000_000;
const NS_PER_MINUTE_I128: i128 = 60 * NS_PER_SECOND_I128;
const NS_PER_HOUR_I128: i128 = 60 * NS_PER_MINUTE_I128;
const NS_PER_DAY_I128: i128 = 24 * NS_PER_HOUR_I128;
const MAX_INSTANT_NS: i128 = 8_640_000_000_000_000_000_000;

fn bigint_to_i128_saturating(value: &crate::runtime::BigIntValue) -> i128 {
    bigint::to_i128_if_exact(value).unwrap_or_else(|| {
        if bigint::sign(value).is_lt() {
            i128::MIN
        } else {
            i128::MAX
        }
    })
}

pub(super) fn install(context: &mut NativeContext) -> Result<(), VmError> {
    install_date(context)?;
    augment_intl(context)?;
    install_temporal(context)?;
    Ok(())
}

fn method_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, true)
}

fn constant_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, false)
}

fn readonly_configurable_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, true)
}

fn hidden_slot_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, false)
}

fn define_method(
    context: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
) -> Result<JsValue, VmError> {
    let function = context.register_builtin(name, length, call, None)?;
    context.define_own_property(target, name.into(), method_descriptor(function.clone()))?;
    Ok(function)
}

fn define_accessor(
    context: &mut NativeContext,
    target: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    call: NativeCall,
) -> Result<(), VmError> {
    let getter = context.register_builtin(getter_name, 0, call, None)?;
    context.define_own_property(
        target,
        name.into(),
        PropertyDescriptor::accessor(Some(getter), None, false, true),
    )?;
    Ok(())
}

fn declare_standard_global(
    context: &mut NativeContext,
    name: &'static str,
    value: JsValue,
) -> Result<(), VmError> {
    context.declare_global(name, value.clone());
    context.define_own_property(
        context.global_object(),
        name.into(),
        method_descriptor(value),
    )?;
    Ok(())
}

fn new_ordinary_object(
    context: &mut NativeContext,
    prototype: Option<ObjectId>,
) -> Result<ObjectId, VmError> {
    let mut object = JsObject::ordinary();
    object.prototype = prototype;
    context
        .heap_mut()
        .allocate_object(object)
        .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))
}

fn define_hidden(
    context: &mut NativeContext,
    object: ObjectId,
    name: impl Into<String>,
    value: JsValue,
) -> Result<(), VmError> {
    context.define_own_property(object, name.into(), hidden_slot_descriptor(value))?;
    Ok(())
}

fn own_data_value(context: &NativeContext, object: ObjectId, key: &str) -> Option<JsValue> {
    context
        .get_own_property_descriptor(object, key)
        .and_then(|descriptor| match descriptor.kind {
            PropertyKind::Data { value, .. } => Some(value),
            PropertyKind::Accessor { .. } => None,
        })
}

fn own_string(context: &NativeContext, object: ObjectId, key: &str) -> Option<String> {
    match own_data_value(context, object, key)? {
        JsValue::String(value) => Some(value),
        _ => None,
    }
}

fn own_number(context: &NativeContext, object: ObjectId, key: &str) -> Option<f64> {
    match own_data_value(context, object, key)? {
        JsValue::Number(value) => Some(value),
        _ => None,
    }
}

fn arg_or_undefined(arguments: &[JsValue], index: usize) -> JsValue {
    arguments.get(index).cloned().unwrap_or(JsValue::Undefined)
}

fn object_from_pairs(
    context: &mut NativeContext,
    pairs: impl IntoIterator<Item = (&'static str, JsValue)>,
) -> Result<JsValue, VmError> {
    let object = new_ordinary_object(context, context.object_prototype())?;
    for (key, value) in pairs {
        context.define_own_property(object, key.into(), PropertyDescriptor::data(value))?;
    }
    Ok(JsValue::Object(object))
}

fn current_time_ms() -> f64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as f64,
        Err(error) => -(error.duration().as_millis() as f64),
    }
}

fn time_clip(value: f64) -> f64 {
    if !value.is_finite() || value.abs() > MAX_TIME_VALUE {
        f64::NAN
    } else {
        let clipped = value.trunc();
        if clipped == 0.0 { 0.0 } else { clipped }
    }
}

#[derive(Clone, Copy)]
struct DateFields {
    year: i32,
    month: u32,
    day: u32,
    weekday: u32,
    hour: u32,
    minute: u32,
    second: u32,
    millisecond: u32,
}

fn decompose_time(value: f64) -> Option<DateFields> {
    if !value.is_finite() {
        return None;
    }
    let day_number = (value / MS_PER_DAY).floor() as i64;
    let mut time_within_day = (value - (day_number as f64 * MS_PER_DAY)).round() as i64;
    if time_within_day < 0 {
        time_within_day += MS_PER_DAY as i64;
    }
    let (year, month, day) = civil_from_days(day_number);
    let hour = (time_within_day / MS_PER_HOUR as i64) as u32;
    time_within_day %= MS_PER_HOUR as i64;
    let minute = (time_within_day / MS_PER_MINUTE as i64) as u32;
    time_within_day %= MS_PER_MINUTE as i64;
    let second = (time_within_day / MS_PER_SECOND as i64) as u32;
    let millisecond = (time_within_day % MS_PER_SECOND as i64) as u32;
    let weekday = (day_number + 4).rem_euclid(7) as u32;
    Some(DateFields {
        year,
        month,
        day,
        weekday,
        hour,
        minute,
        second,
        millisecond,
    })
}

fn days_from_civil(mut year: i32, month: u32, day: u32) -> i64 {
    year -= i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let mp = month as i32 + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era as i64 * 146_097 + doe as i64 - 719_468
}

fn civil_from_days(days: i64) -> (i32, u32, u32) {
    // ECMAScript Date and Temporal never expose civil dates beyond this
    // representable window. Clamping here keeps hostile intermediate values
    // from overflowing the conversion arithmetic before callers reject them.
    let days = days.clamp(-100_000_000, 100_000_000);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = (mp + if mp < 10 { 3 } else { -9 }) as u32;
    year += i32::from(month <= 2);
    (year, month, day)
}

fn make_day(year: f64, month: f64, date: f64) -> f64 {
    if !year.is_finite() || !month.is_finite() || !date.is_finite() {
        return f64::NAN;
    }
    let year = year.trunc() as i32;
    let month = month.trunc() as i32;
    let date = date.trunc() as i64;
    let total_months = year as i64 * 12 + month as i64;
    let normalized_year = total_months.div_euclid(12) as i32;
    let normalized_month = total_months.rem_euclid(12) as u32 + 1;
    (days_from_civil(normalized_year, normalized_month, 1) + date - 1) as f64
}

fn make_time(hour: f64, minute: f64, second: f64, millisecond: f64) -> f64 {
    if !hour.is_finite() || !minute.is_finite() || !second.is_finite() || !millisecond.is_finite() {
        return f64::NAN;
    }
    hour.trunc() * MS_PER_HOUR
        + minute.trunc() * MS_PER_MINUTE
        + second.trunc() * MS_PER_SECOND
        + millisecond.trunc()
}

fn make_date(day: f64, time: f64) -> f64 {
    if !day.is_finite() || !time.is_finite() {
        f64::NAN
    } else {
        day * MS_PER_DAY + time
    }
}

fn date_from_components(
    year: f64,
    month: f64,
    date: f64,
    hour: f64,
    minute: f64,
    second: f64,
    millisecond: f64,
) -> f64 {
    let year = if year.is_finite() {
        let integer_year = year.trunc();
        if (0.0..=99.0).contains(&integer_year) {
            integer_year + 1900.0
        } else {
            year
        }
    } else {
        year
    };
    time_clip(make_date(
        make_day(year, month, date),
        make_time(hour, minute, second, millisecond),
    ))
}

fn month_day_count(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn temporal_day_of_week_from_day_number(day_number: i64) -> u32 {
    let sunday_zero = (day_number + 4).rem_euclid(7) as u32;
    if sunday_zero == 0 { 7 } else { sunday_zero }
}

fn temporal_day_of_week(year: i32, month: u32, day: u32) -> u32 {
    temporal_day_of_week_from_day_number(days_from_civil(year, month, day))
}

fn temporal_day_of_year(year: i32, month: u32, day: u32) -> u32 {
    (days_from_civil(year, month, day) - days_from_civil(year, 1, 1) + 1) as u32
}

fn iso_weeks_in_year(year: i32) -> u32 {
    let jan_first = temporal_day_of_week(year, 1, 1);
    if jan_first == 4 || (jan_first == 3 && is_leap_year(year)) {
        53
    } else {
        52
    }
}

fn temporal_week_fields(year: i32, month: u32, day: u32) -> (u32, i32) {
    let day_of_week = temporal_day_of_week(year, month, day) as i32;
    let day_of_year = temporal_day_of_year(year, month, day) as i32;
    let mut week = (day_of_year - day_of_week + 10).div_euclid(7);
    let mut week_year = year;
    if week < 1 {
        week_year -= 1;
        week = iso_weeks_in_year(week_year) as i32;
    } else if week > iso_weeks_in_year(year) as i32 {
        week_year += 1;
        week = 1;
    }
    (week as u32, week_year)
}

fn temporal_date_slots(
    year: f64,
    month: f64,
    day: f64,
    calendar_id: String,
) -> Vec<(&'static str, JsValue)> {
    let year_i = year.trunc() as i32;
    let month_u = month.trunc() as u32;
    let day_u = day.trunc() as u32;
    let day_number = days_from_civil(year_i, month_u, day_u);
    let day_of_year = temporal_day_of_year(year_i, month_u, day_u);
    let (week_of_year, year_of_week) = temporal_week_fields(year_i, month_u, day_u);
    let leap = is_leap_year(year_i);
    vec![
        ("year", JsValue::Number(year.trunc())),
        ("month", JsValue::Number(month.trunc())),
        ("monthCode", JsValue::String(month_code(month))),
        ("day", JsValue::Number(day.trunc())),
        ("calendarId", JsValue::String(calendar_id)),
        (
            "dayOfWeek",
            JsValue::Number(temporal_day_of_week_from_day_number(day_number) as f64),
        ),
        ("dayOfYear", JsValue::Number(day_of_year as f64)),
        ("weekOfYear", JsValue::Number(week_of_year as f64)),
        ("yearOfWeek", JsValue::Number(year_of_week as f64)),
        ("daysInWeek", JsValue::Number(7.0)),
        (
            "daysInMonth",
            JsValue::Number(month_day_count(year_i, month_u) as f64),
        ),
        (
            "daysInYear",
            JsValue::Number(if leap { 366.0 } else { 365.0 }),
        ),
        ("monthsInYear", JsValue::Number(12.0)),
        ("inLeapYear", JsValue::Boolean(leap)),
    ]
}

fn date_value_from_this(context: &NativeContext, this_value: &JsValue) -> Result<f64, VmError> {
    let object = context.require_object(this_value, "Date method")?;
    if own_data_value(context, object, DATE_MARKER).is_none() {
        return Err(VmError::type_error("receiver is not a Date object"));
    }
    Ok(own_number(context, object, DATE_VALUE).unwrap_or(f64::NAN))
}

fn set_date_value(
    context: &mut NativeContext,
    this_value: &JsValue,
    value: f64,
) -> Result<(), VmError> {
    let object = context.require_object(this_value, "Date method")?;
    if own_data_value(context, object, DATE_MARKER).is_none() {
        return Err(VmError::type_error("receiver is not a Date object"));
    }
    define_hidden(context, object, DATE_VALUE, JsValue::Number(value))
}

fn two_digit(value: u32) -> String {
    format!("{value:02}")
}

fn three_digit(value: u32) -> String {
    format!("{value:03}")
}

fn iso_year(year: i32) -> String {
    if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else if year >= 0 {
        format!("+{year:06}")
    } else {
        format!("-{:06}", year.unsigned_abs())
    }
}

fn iso_date_from_fields(fields: DateFields) -> String {
    format!(
        "{}-{}-{}",
        iso_year(fields.year),
        two_digit(fields.month),
        two_digit(fields.day)
    )
}

fn iso_time_from_fields(fields: DateFields) -> String {
    format!(
        "{}:{}:{}.{}",
        two_digit(fields.hour),
        two_digit(fields.minute),
        two_digit(fields.second),
        three_digit(fields.millisecond)
    )
}

fn format_iso(value: f64) -> Result<String, VmError> {
    let Some(fields) = decompose_time(value) else {
        return Err(VmError::range("Invalid time value"));
    };
    Ok(format!(
        "{}T{}Z",
        iso_date_from_fields(fields),
        iso_time_from_fields(fields)
    ))
}

fn format_date_fallback(value: f64) -> String {
    match decompose_time(value) {
        Some(fields) => iso_date_from_fields(fields),
        None => "Invalid Date".into(),
    }
}

fn format_utc_string(value: f64) -> String {
    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    match decompose_time(value) {
        Some(fields) => format!(
            "{}, {} {} {} {}:{}:{} GMT",
            WEEKDAYS[fields.weekday as usize],
            two_digit(fields.day),
            MONTHS[(fields.month - 1) as usize],
            iso_year(fields.year),
            two_digit(fields.hour),
            two_digit(fields.minute),
            two_digit(fields.second)
        ),
        None => "Invalid Date".into(),
    }
}

fn format_date_string(value: f64) -> String {
    const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    match decompose_time(value) {
        Some(fields) => format!(
            "{} {} {} {} {}:{}:{} GMT+0000 (UTC)",
            WEEKDAYS[fields.weekday as usize],
            MONTHS[(fields.month - 1) as usize],
            two_digit(fields.day),
            iso_year(fields.year),
            two_digit(fields.hour),
            two_digit(fields.minute),
            two_digit(fields.second)
        ),
        None => "Invalid Date".into(),
    }
}

fn parse_fixed_digits(value: &str, count: usize) -> Option<u32> {
    if value.len() != count || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

fn parse_iso_date_part(input: &str) -> Option<(i32, u32, u32)> {
    let (year_text, rest) = if input.starts_with(['+', '-']) {
        if input.len() < 7 {
            return None;
        }
        (&input[..7], &input[7..])
    } else {
        if input.len() < 4 {
            return None;
        }
        (&input[..4], &input[4..])
    };
    let signed = year_text.starts_with(['+', '-']);
    let digits = if signed { &year_text[1..] } else { year_text };
    if digits.len() != if signed { 6 } else { 4 }
        || !digits.chars().all(|ch| ch.is_ascii_digit())
        || year_text == "-000000"
    {
        return None;
    }
    let year = year_text.parse::<i32>().ok()?;
    if rest.is_empty() {
        return Some((year, 1, 1));
    }
    let rest = rest.strip_prefix('-')?;
    if rest.len() < 2 {
        return None;
    }
    let month = parse_fixed_digits(&rest[..2], 2)?;
    let rest = &rest[2..];
    let day = if rest.is_empty() {
        1
    } else {
        let rest = rest.strip_prefix('-')?;
        if rest.len() != 2 {
            return None;
        }
        parse_fixed_digits(rest, 2)?
    };
    Some((year, month, day))
}

fn parse_iso_date_string(input: &str) -> Option<f64> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let (date_part, time_part) = match input.find('T').or_else(|| input.find(' ')) {
        Some(index) => (&input[..index], Some(&input[index + 1..])),
        None => (input, None),
    };
    let (year, month, day) = parse_iso_date_part(date_part)?;
    if !(1..=12).contains(&month) || !(1..=month_day_count(year, month)).contains(&day) {
        return None;
    }

    let (hour, minute, second, millisecond, offset_ms) = if let Some(time_part) = time_part {
        parse_iso_time_and_offset(time_part)?
    } else {
        (0, 0, 0, 0, 0)
    };
    let local_ms = make_date(
        days_from_civil(year, month, day) as f64,
        make_time(
            hour as f64,
            minute as f64,
            second as f64,
            millisecond as f64,
        ),
    );
    Some(time_clip(local_ms - offset_ms as f64))
}

fn temporal_to_bigint(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<crate::runtime::BigIntValue, VmError> {
    match value {
        JsValue::BigInt(value) => Ok(value),
        JsValue::Boolean(value) => Ok(bigint::from_i64(i64::from(value))),
        JsValue::String(value) => bigint::parse_bigint_string(&value)
            .ok_or_else(|| VmError::syntax_error("Cannot convert string to BigInt")),
        JsValue::Object(object) => match context.primitive_value(object) {
            Some(PrimitiveValue::BigInt(value)) => Ok(value.clone()),
            Some(PrimitiveValue::Boolean(value)) => Ok(bigint::from_i64(i64::from(*value))),
            Some(PrimitiveValue::String(value)) => bigint::parse_bigint_string(value)
                .ok_or_else(|| VmError::syntax_error("Cannot convert string to BigInt")),
            _ => {
                let primitive =
                    vm.to_primitive(JsValue::Object(object), PreferredType::Number, context)?;
                temporal_to_bigint(vm, context, primitive)
            }
        },
        JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            let primitive = vm.to_primitive(value, PreferredType::Number, context)?;
            temporal_to_bigint(vm, context, primitive)
        }
        _ => Err(VmError::type_error("Cannot convert value to BigInt")),
    }
}

fn is_valid_instant_ns(epoch_ns: i128) -> bool {
    (-MAX_INSTANT_NS..=MAX_INSTANT_NS).contains(&epoch_ns)
}

fn validate_temporal_annotations(input: &str) -> Option<&str> {
    let mut body_end = input.len();
    let mut saw_annotation = false;
    let mut seen_time_zone = false;
    let mut rest = input;
    while let Some(start) = rest.find('[') {
        saw_annotation = true;
        body_end = body_end.min(input.len() - rest.len() + start);
        let after_start = &rest[start + 1..];
        let end = after_start.find(']')?;
        let annotation = &after_start[..end];
        if annotation.is_empty() {
            return None;
        }
        let annotation = annotation.strip_prefix('!').unwrap_or(annotation);
        if let Some((key, _value)) = annotation.split_once('=') {
            if key.chars().any(|ch| ch.is_ascii_uppercase()) {
                return None;
            }
        } else {
            if seen_time_zone {
                return None;
            }
            seen_time_zone = true;
        }
        rest = &after_start[end + 1..];
    }
    if (saw_annotation && !rest.is_empty()) || rest.contains(']') {
        return None;
    }
    if body_end < input.len() && !input[body_end..].starts_with('[') {
        return None;
    }
    Some(&input[..body_end])
}

fn validate_iso_calendar_annotations(input: &str) -> Option<&str> {
    let mut body_end = input.len();
    let mut saw_annotation = false;
    let mut rest = input;
    while let Some(start) = rest.find('[') {
        saw_annotation = true;
        body_end = body_end.min(input.len() - rest.len() + start);
        let after_start = &rest[start + 1..];
        let end = after_start.find(']')?;
        let annotation = &after_start[..end];
        if annotation.is_empty() {
            return None;
        }
        let critical = annotation.starts_with('!');
        let annotation = annotation.strip_prefix('!').unwrap_or(annotation);
        if let Some((key, value)) = annotation.split_once('=') {
            if key.chars().any(|ch| ch.is_ascii_uppercase()) {
                return None;
            }
            if key == "u-ca" {
                if value.to_ascii_lowercase() != "iso8601" {
                    return None;
                }
            } else if critical {
                return None;
            }
        } else if critical {
            return None;
        }
        rest = &after_start[end + 1..];
    }
    if (saw_annotation && !rest.is_empty()) || rest.contains(']') {
        return None;
    }
    Some(&input[..body_end])
}

fn parse_temporal_date_part(input: &str) -> Option<(i32, u32, u32)> {
    let signed = input.starts_with(['+', '-']);
    let (year_text, rest) = if signed {
        if input.len() < 7 {
            return None;
        }
        (&input[..7], &input[7..])
    } else {
        if input.len() < 4 {
            return None;
        }
        (&input[..4], &input[4..])
    };
    let digits = if signed { &year_text[1..] } else { year_text };
    if digits.len() != if signed { 6 } else { 4 }
        || !digits.chars().all(|ch| ch.is_ascii_digit())
        || year_text == "-000000"
    {
        return None;
    }
    let year = year_text.parse::<i32>().ok()?;
    if !(-271_821..=275_760).contains(&year) {
        return None;
    }
    let (month, day) = if let Some(rest) = rest.strip_prefix('-') {
        if rest.len() != 5 || !rest.as_bytes().get(2).is_some_and(|ch| *ch == b'-') {
            return None;
        }
        (
            parse_fixed_digits(&rest[..2], 2)?,
            parse_fixed_digits(&rest[3..], 2)?,
        )
    } else {
        if rest.len() != 4 {
            return None;
        }
        (
            parse_fixed_digits(&rest[..2], 2)?,
            parse_fixed_digits(&rest[2..], 2)?,
        )
    };
    if !(1..=12).contains(&month) || !(1..=month_day_count(year, month)).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

fn validate_plain_date_time_tail(input: &str) -> Option<()> {
    if input.is_empty()
        || input.contains('\u{2212}')
        || input.ends_with(['Z', 'z'])
        || input.contains("Z[")
        || input.contains("z[")
    {
        return None;
    }
    let offset_start = input.rfind('+').or_else(|| {
        input
            .get(1..)
            .and_then(|rest| rest.rfind('-').map(|index| index + 1))
    });
    let time = if let Some(offset_start) = offset_start {
        parse_time_zone_offset_ns(&input[offset_start..])?;
        &input[..offset_start]
    } else {
        input
    };
    let compact = time.replace(':', "");
    let (head, fraction) = compact
        .split_once('.')
        .map_or((compact.as_str(), ""), |(head, fraction)| (head, fraction));
    if !fraction.is_empty() && parse_fraction_to_ns(fraction).is_none() {
        return None;
    }
    if head.len() != 2 && head.len() != 4 && head.len() != 6 {
        return None;
    }
    if !head.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let hour = parse_fixed_digits(&head[..2], 2)?;
    let minute = if head.len() >= 4 {
        parse_fixed_digits(&head[2..4], 2)?
    } else {
        0
    };
    let second = if head.len() == 6 {
        parse_fixed_digits(&head[4..6], 2)?
    } else {
        0
    };
    (hour <= 23 && minute <= 59 && second <= 59).then_some(())
}

fn parse_temporal_plain_date_string(input: &str) -> Option<(f64, f64, f64)> {
    let input = input.trim();
    if input.is_empty() || input.contains('\u{2212}') {
        return None;
    }
    let body = validate_iso_calendar_annotations(input)?;
    let (date_part, time_part) = match body
        .find('T')
        .or_else(|| body.find('t'))
        .or_else(|| body.find(' '))
    {
        Some(index) => (&body[..index], Some(&body[index + 1..])),
        None => (body, None),
    };
    if let Some(time_part) = time_part {
        validate_plain_date_time_tail(time_part)?;
    }
    let (year, month, day) = parse_temporal_date_part(date_part)?;
    Some((year as f64, month as f64, day as f64))
}

fn parse_fraction_to_ns(fraction: &str) -> Option<i128> {
    if fraction.is_empty() || fraction.len() > 9 || !fraction.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let mut text = fraction.to_string();
    while text.len() < 9 {
        text.push('0');
    }
    text.parse::<i128>().ok()
}

fn parse_iso_time_and_offset_ns(input: &str) -> Option<(u32, u32, u32, i128, i128)> {
    let (time, offset_ns) =
        if let Some(stripped) = input.strip_suffix('Z').or_else(|| input.strip_suffix('z')) {
            (stripped, 0)
        } else if let Some(index) = input.rfind('+') {
            (&input[..index], parse_time_zone_offset_ns(&input[index..])?)
        } else if let Some(index) = input.get(1..).and_then(|rest| rest.rfind('-')) {
            let split = index + 1;
            (&input[..split], parse_time_zone_offset_ns(&input[split..])?)
        } else {
            return None;
        };
    if time.is_empty() {
        return None;
    }
    let mut pieces = time.split(':');
    let hour = parse_fixed_digits(pieces.next()?, 2)?;
    let minute = parse_fixed_digits(pieces.next().unwrap_or("00"), 2)?;
    let seconds_piece = pieces.next().unwrap_or("00");
    if pieces.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }
    let (second_text, fraction_text) = seconds_piece
        .split_once('.')
        .map_or((seconds_piece, ""), |(second, fraction)| (second, fraction));
    let mut second = parse_fixed_digits(second_text, 2)?;
    if second > 60 {
        return None;
    }
    if second == 60 {
        second = 59;
    }
    let fraction_ns = if fraction_text.is_empty() {
        0
    } else {
        parse_fraction_to_ns(fraction_text)?
    };
    Some((hour, minute, second, fraction_ns, offset_ns))
}

fn parse_time_zone_offset_ns(input: &str) -> Option<i128> {
    let sign = if input.starts_with('+') {
        1
    } else if input.starts_with('-') {
        -1
    } else {
        return None;
    };
    let body = &input[1..];
    if body.len() == 4 && body.chars().all(|ch| ch.is_ascii_digit()) {
        let hour = parse_fixed_digits(&body[..2], 2)?;
        let minute = parse_fixed_digits(&body[2..], 2)?;
        if hour > 23 || minute > 59 {
            return None;
        }
        return Some(
            sign * (hour as i128 * NS_PER_HOUR_I128 + minute as i128 * NS_PER_MINUTE_I128),
        );
    }
    let mut pieces = body.split(':');
    let hour = parse_fixed_digits(pieces.next()?, 2)?;
    let minute = parse_fixed_digits(pieces.next().unwrap_or("00"), 2)?;
    let seconds_piece = pieces.next();
    if pieces.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }
    if let Some(seconds_piece) = seconds_piece {
        let (second_text, fraction_text) = seconds_piece
            .split_once('.')
            .map_or((seconds_piece, ""), |(second, fraction)| (second, fraction));
        let second = parse_fixed_digits(second_text, 2)?;
        if second > 59 {
            return None;
        }
        let fraction_ns = if fraction_text.is_empty() {
            0
        } else {
            parse_fraction_to_ns(fraction_text)?
        };
        if second != 0 || fraction_ns != 0 {
            return None;
        }
    }
    Some(sign * (hour as i128 * NS_PER_HOUR_I128 + minute as i128 * NS_PER_MINUTE_I128))
}

fn parse_instant_string(input: &str) -> Option<i128> {
    let input = input.trim();
    if input.is_empty() || input.contains('\u{2212}') {
        return None;
    }
    let body = validate_temporal_annotations(input)?;
    let separator = body.find('T').or_else(|| body.find('t'))?;
    let (date_part, time_part) = (&body[..separator], &body[separator + 1..]);
    let (year, month, day) = parse_iso_date_part(date_part)?;
    if !(1..=12).contains(&month) || !(1..=month_day_count(year, month)).contains(&day) {
        return None;
    }
    let (hour, minute, second, fraction_ns, offset_ns) = parse_iso_time_and_offset_ns(time_part)?;
    let date_ns = days_from_civil(year, month, day) as i128 * NS_PER_DAY_I128;
    let time_ns = hour as i128 * NS_PER_HOUR_I128
        + minute as i128 * NS_PER_MINUTE_I128
        + second as i128 * NS_PER_SECOND_I128
        + fraction_ns;
    let epoch_ns = date_ns + time_ns - offset_ns;
    is_valid_instant_ns(epoch_ns).then_some(epoch_ns)
}

fn parse_iso_time_and_offset(input: &str) -> Option<(u32, u32, u32, u32, i64)> {
    let (time, offset_ms) = if let Some(stripped) = input.strip_suffix('Z') {
        (stripped, 0)
    } else if let Some(index) = input.rfind('+') {
        (&input[..index], parse_time_zone_offset(&input[index..])?)
    } else if let Some(index) = input.get(1..).and_then(|rest| rest.rfind('-')) {
        let split = index + 1;
        (&input[..split], parse_time_zone_offset(&input[split..])?)
    } else {
        (input, 0)
    };
    let mut pieces = time.split(':');
    let hour = parse_fixed_digits(pieces.next()?, 2)?;
    let minute = parse_fixed_digits(pieces.next().unwrap_or("00"), 2)?;
    let seconds_piece = pieces.next().unwrap_or("00");
    if pieces.next().is_some() || hour > 23 || minute > 59 {
        return None;
    }
    let (second_text, fraction_text) = seconds_piece
        .split_once('.')
        .map_or((seconds_piece, ""), |(second, fraction)| (second, fraction));
    let second = parse_fixed_digits(second_text, 2)?;
    if second > 59 {
        return None;
    }
    let mut fraction = fraction_text.chars().take(3).collect::<String>();
    while fraction.len() < 3 {
        fraction.push('0');
    }
    let millisecond = if fraction.is_empty() {
        0
    } else {
        parse_fixed_digits(&fraction, 3)?
    };
    Some((hour, minute, second, millisecond, offset_ms))
}

fn parse_time_zone_offset(input: &str) -> Option<i64> {
    let sign = if input.starts_with('+') {
        1
    } else if input.starts_with('-') {
        -1
    } else {
        return None;
    };
    let body = &input[1..];
    let (hour, minute) = if let Some((hour, minute)) = body.split_once(':') {
        (parse_fixed_digits(hour, 2)?, parse_fixed_digits(minute, 2)?)
    } else if body.len() == 4 {
        (
            parse_fixed_digits(&body[..2], 2)?,
            parse_fixed_digits(&body[2..], 2)?,
        )
    } else {
        return None;
    };
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(sign * ((hour as i64 * 60 + minute as i64) * MS_PER_MINUTE as i64))
}

fn install_date(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;

    let constructor = context.register_builtin("Date", 7, date_call, Some(date_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Date constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;

    define_method(context, constructor_object, "now", 0, date_now)?;
    define_method(context, constructor_object, "parse", 1, date_parse)?;
    define_method(context, constructor_object, "UTC", 7, date_utc)?;

    for (name, length, call) in [
        ("valueOf", 0, date_value_of as NativeCall),
        ("getTime", 0, date_value_of as NativeCall),
        ("toISOString", 0, date_to_iso_string as NativeCall),
        ("toJSON", 1, date_to_json as NativeCall),
        (
            "toTemporalInstant",
            0,
            date_to_temporal_instant as NativeCall,
        ),
        ("toString", 0, date_to_string as NativeCall),
        ("toDateString", 0, date_to_date_string as NativeCall),
        ("toTimeString", 0, date_to_time_string as NativeCall),
        ("toLocaleString", 0, date_to_string as NativeCall),
        ("toLocaleDateString", 0, date_to_date_string as NativeCall),
        ("toLocaleTimeString", 0, date_to_time_string as NativeCall),
        ("getYear", 0, date_get_year as NativeCall),
        ("getFullYear", 0, date_get_utc_full_year as NativeCall),
        ("getUTCFullYear", 0, date_get_utc_full_year as NativeCall),
        ("getMonth", 0, date_get_utc_month as NativeCall),
        ("getUTCMonth", 0, date_get_utc_month as NativeCall),
        ("getDate", 0, date_get_utc_date as NativeCall),
        ("getUTCDate", 0, date_get_utc_date as NativeCall),
        ("getDay", 0, date_get_utc_day as NativeCall),
        ("getUTCDay", 0, date_get_utc_day as NativeCall),
        ("getHours", 0, date_get_utc_hours as NativeCall),
        ("getUTCHours", 0, date_get_utc_hours as NativeCall),
        ("getMinutes", 0, date_get_utc_minutes as NativeCall),
        ("getUTCMinutes", 0, date_get_utc_minutes as NativeCall),
        ("getSeconds", 0, date_get_utc_seconds as NativeCall),
        ("getUTCSeconds", 0, date_get_utc_seconds as NativeCall),
        (
            "getMilliseconds",
            0,
            date_get_utc_milliseconds as NativeCall,
        ),
        (
            "getUTCMilliseconds",
            0,
            date_get_utc_milliseconds as NativeCall,
        ),
        (
            "getTimezoneOffset",
            0,
            date_get_timezone_offset as NativeCall,
        ),
        ("setTime", 1, date_set_time as NativeCall),
        (
            "setMilliseconds",
            1,
            date_set_utc_milliseconds as NativeCall,
        ),
        (
            "setUTCMilliseconds",
            1,
            date_set_utc_milliseconds as NativeCall,
        ),
        ("setSeconds", 2, date_set_utc_seconds as NativeCall),
        ("setUTCSeconds", 2, date_set_utc_seconds as NativeCall),
        ("setMinutes", 3, date_set_utc_minutes as NativeCall),
        ("setUTCMinutes", 3, date_set_utc_minutes as NativeCall),
        ("setHours", 4, date_set_utc_hours as NativeCall),
        ("setUTCHours", 4, date_set_utc_hours as NativeCall),
        ("setDate", 1, date_set_utc_date as NativeCall),
        ("setUTCDate", 1, date_set_utc_date as NativeCall),
        ("setMonth", 2, date_set_utc_month as NativeCall),
        ("setUTCMonth", 2, date_set_utc_month as NativeCall),
        ("setFullYear", 3, date_set_utc_full_year as NativeCall),
        ("setUTCFullYear", 3, date_set_utc_full_year as NativeCall),
        ("setYear", 1, date_set_year as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    let to_utc_string = define_method(context, prototype, "toUTCString", 0, date_to_utc_string)?;
    context.define_own_property(
        prototype,
        "toGMTString".into(),
        method_descriptor(to_utc_string),
    )?;

    let to_primitive =
        context.register_builtin("[Symbol.toPrimitive]", 1, date_to_primitive, None)?;
    let to_primitive_symbol = context.well_known_symbols().to_primitive;
    context.define_symbol_own_property(
        prototype,
        to_primitive_symbol,
        readonly_configurable_descriptor(to_primitive),
    )?;
    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Date".into())),
    )?;

    declare_standard_global(context, "Date", constructor)?;
    Ok(())
}

fn date_call(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(format_date_string(current_time_ms())))
}

fn date_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Date prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(context, object, DATE_MARKER, JsValue::Boolean(true))?;
    let value = match arguments.len() {
        0 => current_time_ms(),
        1 => {
            let value = arguments[0].clone();
            match value {
                JsValue::String(text) => parse_iso_date_string(&text).unwrap_or(f64::NAN),
                other => time_clip(vm.to_number(other, context)?),
            }
        }
        _ => date_from_components(
            vm.to_number(
                arguments.first().cloned().unwrap_or(JsValue::Undefined),
                context,
            )?,
            vm.to_number(
                arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
                context,
            )?,
            number_or_default(vm, context, arguments, 2, 1.0)?,
            number_or_default(vm, context, arguments, 3, 0.0)?,
            number_or_default(vm, context, arguments, 4, 0.0)?,
            number_or_default(vm, context, arguments, 5, 0.0)?,
            number_or_default(vm, context, arguments, 6, 0.0)?,
        ),
    };
    define_hidden(context, object, DATE_VALUE, JsValue::Number(value))?;
    Ok(JsValue::Object(object))
}

fn number_or_default(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
    default: f64,
) -> Result<f64, VmError> {
    match arguments.get(index) {
        Some(value) => vm.to_number(value.clone(), context),
        None => Ok(default),
    }
}

fn date_now(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(current_time_ms()))
}

fn date_parse(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let text = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    Ok(JsValue::Number(
        parse_iso_date_string(&text).unwrap_or(f64::NAN),
    ))
}

fn date_utc(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let year = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let month = vm.to_number(
        arguments.get(1).cloned().unwrap_or(JsValue::Number(0.0)),
        context,
    )?;
    Ok(JsValue::Number(date_from_components(
        year,
        month,
        number_or_default(vm, context, arguments, 2, 1.0)?,
        number_or_default(vm, context, arguments, 3, 0.0)?,
        number_or_default(vm, context, arguments, 4, 0.0)?,
        number_or_default(vm, context, arguments, 5, 0.0)?,
        number_or_default(vm, context, arguments, 6, 0.0)?,
    )))
}

fn date_value_of(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::Number(date_value_from_this(context, &this_value)?))
}

fn date_to_iso_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(format_iso(date_value_from_this(
        context,
        &this_value,
    )?)?))
}

fn date_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if matches!(this_value, JsValue::Undefined | JsValue::Null) {
        return Err(VmError::type_error(
            "Date.prototype.toJSON receiver is null or undefined",
        ));
    }
    let primitive = vm.to_primitive(this_value.clone(), PreferredType::Number, context)?;
    if matches!(primitive, JsValue::Number(number) if !number.is_finite()) {
        return Ok(JsValue::Null);
    }
    let to_iso_string = vm.get_property_value(this_value.clone(), "toISOString", context)?;
    if !is_callable(&to_iso_string) {
        return Err(VmError::type_error(
            "Date.prototype.toJSON toISOString is not callable",
        ));
    }
    vm.call_value_from_builtin(to_iso_string, this_value, Vec::new(), context)
}

fn date_to_temporal_instant(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    if !value.is_finite() {
        return Err(VmError::range("Invalid time value"));
    }
    let prototype = temporal_instant_constructor_prototype(context)?;
    create_instant(context, prototype, value)
}

fn date_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(format_date_string(date_value_from_this(
        context,
        &this_value,
    )?)))
}

fn date_to_utc_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(format_utc_string(date_value_from_this(
        context,
        &this_value,
    )?)))
}

fn date_to_date_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    Ok(JsValue::String(match decompose_time(value) {
        Some(fields) => iso_date_from_fields(fields),
        None => "Invalid Date".into(),
    }))
}

fn date_to_time_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    Ok(JsValue::String(match decompose_time(value) {
        Some(fields) => format!("{} GMT+0000 (UTC)", iso_time_from_fields(fields)),
        None => "Invalid Date".into(),
    }))
}

fn date_field(
    context: &NativeContext,
    this_value: &JsValue,
    map: impl FnOnce(DateFields) -> f64,
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, this_value)?;
    Ok(JsValue::Number(decompose_time(value).map_or(f64::NAN, map)))
}

fn date_get_utc_full_year(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| fields.year as f64)
}

fn date_get_year(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| (fields.year - 1900) as f64)
}

fn date_get_utc_month(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| (fields.month - 1) as f64)
}

fn date_get_utc_date(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| fields.day as f64)
}

fn date_get_utc_day(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| fields.weekday as f64)
}

fn date_get_utc_hours(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| fields.hour as f64)
}

fn date_get_utc_minutes(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| fields.minute as f64)
}

fn date_get_utc_seconds(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| fields.second as f64)
}

fn date_get_utc_milliseconds(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    date_field(context, &this_value, |fields| fields.millisecond as f64)
}

fn date_get_timezone_offset(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    Ok(JsValue::Number(if value.is_finite() {
        0.0
    } else {
        f64::NAN
    }))
}

fn date_set_time(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = time_clip(vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?);
    set_date_value(context, &this_value, value)?;
    Ok(JsValue::Number(value))
}

fn date_number_or_default(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
    default: f64,
) -> Result<f64, VmError> {
    match arguments.get(index) {
        Some(value) => vm.to_number(value.clone(), context),
        None => Ok(default),
    }
}

fn time_within_day(fields: DateFields) -> f64 {
    make_time(
        fields.hour as f64,
        fields.minute as f64,
        fields.second as f64,
        fields.millisecond as f64,
    )
}

fn fields_for_set(value: f64, nan_as_epoch: bool) -> Option<DateFields> {
    decompose_time(value).or_else(|| nan_as_epoch.then(|| decompose_time(0.0)).flatten())
}

fn set_date_from_parts(
    context: &mut NativeContext,
    this_value: &JsValue,
    year: f64,
    month: f64,
    date: f64,
    time: f64,
) -> Result<JsValue, VmError> {
    let value = time_clip(make_date(make_day(year, month, date), time));
    set_date_value(context, this_value, value)?;
    Ok(JsValue::Number(value))
}

fn date_set_utc_milliseconds(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let millisecond = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let Some(fields) = fields_for_set(value, false) else {
        return set_date_value_and_return_nan(context, &this_value);
    };
    let time = make_time(
        fields.hour as f64,
        fields.minute as f64,
        fields.second as f64,
        millisecond,
    );
    set_date_from_parts(
        context,
        &this_value,
        fields.year as f64,
        (fields.month - 1) as f64,
        fields.day as f64,
        time,
    )
}

fn date_set_utc_seconds(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let Some(fields) = fields_for_set(value, false) else {
        vm.to_number(
            arguments.first().cloned().unwrap_or(JsValue::Undefined),
            context,
        )?;
        if let Some(argument) = arguments.get(1) {
            vm.to_number(argument.clone(), context)?;
        }
        return set_date_value_and_return_nan(context, &this_value);
    };
    let second = date_number_or_default(vm, context, arguments, 0, fields.second as f64)?;
    let millisecond = date_number_or_default(vm, context, arguments, 1, fields.millisecond as f64)?;
    let time = make_time(
        fields.hour as f64,
        fields.minute as f64,
        second,
        millisecond,
    );
    set_date_from_parts(
        context,
        &this_value,
        fields.year as f64,
        (fields.month - 1) as f64,
        fields.day as f64,
        time,
    )
}

fn date_set_utc_minutes(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let Some(fields) = fields_for_set(value, false) else {
        for index in 0..arguments.len().min(3) {
            vm.to_number(arguments[index].clone(), context)?;
        }
        return set_date_value_and_return_nan(context, &this_value);
    };
    let minute = date_number_or_default(vm, context, arguments, 0, fields.minute as f64)?;
    let second = date_number_or_default(vm, context, arguments, 1, fields.second as f64)?;
    let millisecond = date_number_or_default(vm, context, arguments, 2, fields.millisecond as f64)?;
    let time = make_time(fields.hour as f64, minute, second, millisecond);
    set_date_from_parts(
        context,
        &this_value,
        fields.year as f64,
        (fields.month - 1) as f64,
        fields.day as f64,
        time,
    )
}

fn date_set_utc_hours(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let Some(fields) = fields_for_set(value, false) else {
        for index in 0..arguments.len().min(4) {
            vm.to_number(arguments[index].clone(), context)?;
        }
        return set_date_value_and_return_nan(context, &this_value);
    };
    let hour = date_number_or_default(vm, context, arguments, 0, fields.hour as f64)?;
    let minute = date_number_or_default(vm, context, arguments, 1, fields.minute as f64)?;
    let second = date_number_or_default(vm, context, arguments, 2, fields.second as f64)?;
    let millisecond = date_number_or_default(vm, context, arguments, 3, fields.millisecond as f64)?;
    let time = make_time(hour, minute, second, millisecond);
    set_date_from_parts(
        context,
        &this_value,
        fields.year as f64,
        (fields.month - 1) as f64,
        fields.day as f64,
        time,
    )
}

fn date_set_utc_date(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let date = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let Some(fields) = fields_for_set(value, false) else {
        return set_date_value_and_return_nan(context, &this_value);
    };
    set_date_from_parts(
        context,
        &this_value,
        fields.year as f64,
        (fields.month - 1) as f64,
        date,
        time_within_day(fields),
    )
}

fn date_set_utc_month(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let Some(fields) = fields_for_set(value, false) else {
        for index in 0..arguments.len().min(2) {
            vm.to_number(arguments[index].clone(), context)?;
        }
        return set_date_value_and_return_nan(context, &this_value);
    };
    let month = date_number_or_default(vm, context, arguments, 0, (fields.month - 1) as f64)?;
    let date = date_number_or_default(vm, context, arguments, 1, fields.day as f64)?;
    set_date_from_parts(
        context,
        &this_value,
        fields.year as f64,
        month,
        date,
        time_within_day(fields),
    )
}

fn date_set_utc_full_year(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let Some(fields) = fields_for_set(value, true) else {
        return set_date_value_and_return_nan(context, &this_value);
    };
    let year = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let month = date_number_or_default(vm, context, arguments, 1, (fields.month - 1) as f64)?;
    let date = date_number_or_default(vm, context, arguments, 2, fields.day as f64)?;
    set_date_from_parts(
        context,
        &this_value,
        year,
        month,
        date,
        time_within_day(fields),
    )
}

fn date_set_year(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let value = date_value_from_this(context, &this_value)?;
    let year = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    if year.is_nan() {
        set_date_value(context, &this_value, f64::NAN)?;
        return Ok(JsValue::Number(f64::NAN));
    }
    let Some(fields) = fields_for_set(value, true) else {
        return set_date_value_and_return_nan(context, &this_value);
    };
    let integer_year = year.trunc();
    let full_year = if (0.0..=99.0).contains(&integer_year) {
        integer_year + 1900.0
    } else {
        integer_year
    };
    set_date_from_parts(
        context,
        &this_value,
        full_year,
        (fields.month - 1) as f64,
        fields.day as f64,
        time_within_day(fields),
    )
}

fn set_date_value_and_return_nan(
    context: &mut NativeContext,
    this_value: &JsValue,
) -> Result<JsValue, VmError> {
    set_date_value(context, this_value, f64::NAN)?;
    Ok(JsValue::Number(f64::NAN))
}

fn date_to_primitive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.require_object(&this_value, "Date @@toPrimitive")?;
    let hint = match arguments.first() {
        Some(JsValue::String(value)) => value.as_str(),
        _ => return Err(VmError::type_error("invalid Date @@toPrimitive hint")),
    };
    if hint == "number" {
        ordinary_to_primitive(vm, context, this_value, "valueOf", "toString")
    } else if hint == "string" || hint == "default" {
        ordinary_to_primitive(vm, context, this_value, "toString", "valueOf")
    } else {
        Err(VmError::type_error("invalid Date @@toPrimitive hint"))
    }
}

fn ordinary_to_primitive(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    first: &str,
    second: &str,
) -> Result<JsValue, VmError> {
    for method_name in [first, second] {
        let method = vm.get_property_value(value.clone(), method_name, context)?;
        if !is_callable(&method) {
            continue;
        }
        let result = vm.call_value_from_builtin(method, value.clone(), Vec::new(), context)?;
        if !is_object_like(&result) {
            return Ok(result);
        }
    }
    Err(VmError::type_error(
        "Cannot convert object to primitive value",
    ))
}

fn is_object_like(value: &JsValue) -> bool {
    matches!(
        value,
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)
    )
}

fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn augment_intl(context: &mut NativeContext) -> Result<(), VmError> {
    let intl = match context
        .get_global("Intl")
        .and_then(|value| context.value_object(&value))
    {
        Some(object) => object,
        None => {
            let object = new_ordinary_object(context, context.object_prototype())?;
            let to_string_tag = context.well_known_symbols().to_string_tag;
            context.define_symbol_own_property(
                object,
                to_string_tag,
                readonly_configurable_descriptor(JsValue::String("Intl".into())),
            )?;
            declare_standard_global(context, "Intl", JsValue::Object(object))?;
            object
        }
    };

    define_method(
        context,
        intl,
        "getCanonicalLocales",
        1,
        intl_get_canonical_locales,
    )?;
    augment_intl_prototype(
        context,
        intl,
        "DateTimeFormat",
        &[
            ("format", 1, intl_date_time_format_format as NativeCall),
            (
                "formatToParts",
                1,
                intl_date_time_format_format_to_parts as NativeCall,
            ),
            (
                "formatRange",
                2,
                intl_date_time_format_format_range as NativeCall,
            ),
            (
                "formatRangeToParts",
                2,
                intl_date_time_format_format_range_to_parts as NativeCall,
            ),
        ],
    )?;
    augment_intl_prototype(
        context,
        intl,
        "NumberFormat",
        &[(
            "formatToParts",
            1,
            intl_number_format_format_to_parts as NativeCall,
        )],
    )?;

    install_intl_constructor(
        context,
        intl,
        "PluralRules",
        0,
        intl_plural_rules_call,
        intl_plural_rules_construct,
        &[
            (
                "resolvedOptions",
                0,
                intl_plural_rules_resolved_options as NativeCall,
            ),
            ("select", 1, intl_plural_rules_select as NativeCall),
            (
                "selectRange",
                2,
                intl_plural_rules_select_range as NativeCall,
            ),
        ],
    )?;
    install_intl_constructor(
        context,
        intl,
        "RelativeTimeFormat",
        0,
        intl_relative_time_format_call,
        intl_relative_time_format_construct,
        &[
            (
                "resolvedOptions",
                0,
                intl_relative_time_format_resolved_options as NativeCall,
            ),
            ("format", 2, intl_relative_time_format_format as NativeCall),
            (
                "formatToParts",
                2,
                intl_relative_time_format_format_to_parts as NativeCall,
            ),
        ],
    )?;
    install_intl_constructor(
        context,
        intl,
        "ListFormat",
        0,
        intl_list_format_call,
        intl_list_format_construct,
        &[
            (
                "resolvedOptions",
                0,
                intl_list_format_resolved_options as NativeCall,
            ),
            ("format", 1, intl_list_format_format as NativeCall),
            (
                "formatToParts",
                1,
                intl_list_format_format_to_parts as NativeCall,
            ),
        ],
    )?;
    install_locale_constructor(context, intl)?;
    Ok(())
}

fn augment_intl_prototype(
    context: &mut NativeContext,
    intl: ObjectId,
    constructor_name: &'static str,
    methods: &[(&'static str, u8, NativeCall)],
) -> Result<(), VmError> {
    let Some(constructor) = context
        .get_own_property_descriptor(intl, constructor_name)
        .and_then(|descriptor| descriptor.value_cloned())
    else {
        return Ok(());
    };
    let Some(constructor_object) = context.value_object(&constructor) else {
        return Ok(());
    };
    let Some(prototype) = context
        .get_own_property_descriptor(constructor_object, "prototype")
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|value| context.value_object(&value))
    else {
        return Ok(());
    };
    for &(name, length, call) in methods {
        define_method(context, prototype, name, length, call)?;
    }
    Ok(())
}

fn install_intl_constructor(
    context: &mut NativeContext,
    intl: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
    construct: NativeConstruct,
    methods: &[(&'static str, u8, NativeCall)],
) -> Result<(), VmError> {
    if context.get_own_property_descriptor(intl, name).is_some() {
        return Ok(());
    }
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    define_hidden(context, prototype, INTL_KIND, JsValue::String(name.into()))?;
    let constructor = context.register_builtin(name, length, call, Some(construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Intl constructor object missing"))?;
    define_hidden(
        context,
        constructor_object,
        INTL_KIND,
        JsValue::String(name.into()),
    )?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    define_method(
        context,
        constructor_object,
        "supportedLocalesOf",
        1,
        intl_supported_locales_of,
    )?;
    for &(method_name, method_length, call) in methods {
        define_method(context, prototype, method_name, method_length, call)?;
    }
    let tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        tag,
        readonly_configurable_descriptor(JsValue::String(format!("Intl.{name}"))),
    )?;
    context.define_own_property(intl, name.into(), method_descriptor(constructor))?;
    Ok(())
}

fn install_locale_constructor(context: &mut NativeContext, intl: ObjectId) -> Result<(), VmError> {
    if context
        .get_own_property_descriptor(intl, "Locale")
        .is_some()
    {
        return Ok(());
    }
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    define_hidden(
        context,
        prototype,
        INTL_KIND,
        JsValue::String("Locale".into()),
    )?;
    let constructor =
        context.register_builtin("Locale", 1, intl_locale_call, Some(intl_locale_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Intl.Locale constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    for (name, getter, call) in [
        (
            "baseName",
            "get baseName",
            intl_locale_base_name_get as NativeCall,
        ),
        (
            "language",
            "get language",
            intl_locale_language_get as NativeCall,
        ),
    ] {
        define_accessor(context, prototype, name, getter, call)?;
    }
    define_method(context, prototype, "toString", 0, intl_locale_to_string)?;
    define_method(context, prototype, "maximize", 0, intl_locale_identity)?;
    define_method(context, prototype, "minimize", 0, intl_locale_identity)?;
    let tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        tag,
        readonly_configurable_descriptor(JsValue::String("Intl.Locale".into())),
    )?;
    context.define_own_property(intl, "Locale".into(), method_descriptor(constructor))?;
    Ok(())
}

fn construct_intl_by_name(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _arguments: &[JsValue],
    kind: &str,
) -> Result<JsValue, VmError> {
    let intl = context
        .get_global("Intl")
        .and_then(|value| context.value_object(&value))
        .ok_or_else(|| VmError::runtime("Intl missing"))?;
    let constructor = context
        .get_own_property_descriptor(intl, kind)
        .and_then(|descriptor| descriptor.value_cloned())
        .ok_or_else(|| VmError::runtime("Intl constructor missing"))?;
    let prototype = context
        .constructor_prototype(&constructor)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Intl prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(context, object, INTL_KIND, JsValue::String(kind.into()))?;
    Ok(JsValue::Object(object))
}

fn intl_plural_rules_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_intl_by_name(vm, context, arguments, "PluralRules")
}

fn intl_relative_time_format_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_intl_by_name(vm, context, arguments, "RelativeTimeFormat")
}

fn intl_list_format_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    construct_intl_by_name(vm, context, arguments, "ListFormat")
}

fn construct_simple_intl(
    context: &mut NativeContext,
    new_target: JsValue,
    kind: &'static str,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Intl prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(context, object, INTL_KIND, JsValue::String(kind.into()))?;
    Ok(JsValue::Object(object))
}

fn intl_plural_rules_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    construct_simple_intl(context, new_target, "PluralRules")
}

fn intl_relative_time_format_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    construct_simple_intl(context, new_target, "RelativeTimeFormat")
}

fn intl_list_format_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    construct_simple_intl(context, new_target, "ListFormat")
}

fn intl_locale_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let constructor = context
        .get_global("Intl")
        .and_then(|intl| context.value_object(&intl))
        .and_then(|intl| context.get_own_property_descriptor(intl, "Locale"))
        .and_then(|descriptor| descriptor.value_cloned())
        .ok_or_else(|| VmError::runtime("Intl.Locale missing"))?;
    intl_locale_construct(vm, context, arguments, constructor)
}

fn intl_locale_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Intl.Locale prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
    let locale = canonicalize_locale(
        &vm.to_string_coerce(
            arguments
                .first()
                .cloned()
                .unwrap_or_else(|| JsValue::String("en-US".into())),
            context,
        )?,
    );
    define_hidden(context, object, INTL_KIND, JsValue::String("Locale".into()))?;
    define_hidden(context, object, INTL_LOCALE, JsValue::String(locale))?;
    Ok(JsValue::Object(object))
}

fn require_intl_kind(
    context: &NativeContext,
    this_value: &JsValue,
    expected: &'static str,
) -> Result<ObjectId, VmError> {
    let object = context.require_object(this_value, "Intl receiver")?;
    match own_string(context, object, INTL_KIND) {
        Some(kind) if kind == expected => Ok(object),
        _ => Err(VmError::type_error(format!(
            "receiver is not an Intl.{expected} object"
        ))),
    }
}

fn collect_locale_list(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Vec<String>, VmError> {
    match value {
        JsValue::Undefined => Ok(Vec::new()),
        JsValue::String(locale) => Ok(vec![canonicalize_locale(&locale)]),
        other => {
            let Some(object) = context.value_object(&other) else {
                return Ok(vec![canonicalize_locale(
                    &vm.to_string_coerce(other, context)?,
                )]);
            };
            let length = context
                .get_property(context.object_value(object), "length")?
                .to_number()
                .unwrap_or(0.0)
                .max(0.0) as usize;
            let mut locales = Vec::new();
            for index in 0..length {
                let value =
                    context.get_property(context.object_value(object), &index.to_string())?;
                if !matches!(value, JsValue::Undefined) {
                    locales.push(canonicalize_locale(&vm.to_string_coerce(value, context)?));
                }
            }
            Ok(locales)
        }
    }
}

fn canonicalize_locale(locale: &str) -> String {
    let trimmed = locale.trim();
    if trimmed.is_empty() || trimmed == "und" {
        return "und".into();
    }
    let mut parts = trimmed.split('-');
    let language = parts.next().unwrap_or("und").to_ascii_lowercase();
    let region = parts.next().map(str::to_ascii_uppercase);
    match region {
        Some(region) if !region.is_empty() => format!("{language}-{region}"),
        _ => language,
    }
}

fn intl_get_canonical_locales(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locales = collect_locale_list(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?
    .into_iter()
    .map(JsValue::String)
    .collect();
    context.create_array(locales)
}

fn intl_supported_locales_of(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locales = collect_locale_list(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?
    .into_iter()
    .filter(|locale| matches!(locale.as_str(), "en" | "en-US" | "und"))
    .map(JsValue::String)
    .collect();
    context.create_array(locales)
}

fn date_time_format_ms(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<f64, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(current_time_ms());
    }
    if let Some(object) = context.value_object(&value)
        && own_data_value(context, object, DATE_MARKER).is_some()
    {
        return Ok(own_number(context, object, DATE_VALUE).unwrap_or(f64::NAN));
    }
    vm.to_number(value, context)
}

fn intl_date_time_format_format(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "DateTimeFormat")?;
    let ms = date_time_format_ms(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::String(format_date_fallback(ms)))
}

fn part(
    context: &mut NativeContext,
    kind: &'static str,
    value: String,
) -> Result<JsValue, VmError> {
    object_from_pairs(
        context,
        [
            ("type", JsValue::String(kind.into())),
            ("value", JsValue::String(value)),
        ],
    )
}

fn source_part(
    context: &mut NativeContext,
    kind: &'static str,
    value: String,
    source: &'static str,
) -> Result<JsValue, VmError> {
    object_from_pairs(
        context,
        [
            ("type", JsValue::String(kind.into())),
            ("value", JsValue::String(value)),
            ("source", JsValue::String(source.into())),
        ],
    )
}

fn date_time_parts(context: &mut NativeContext, ms: f64) -> Result<JsValue, VmError> {
    let Some(fields) = decompose_time(ms) else {
        let invalid = part(context, "literal", "Invalid Date".into())?;
        return context.create_array(vec![invalid]);
    };
    let parts = vec![
        part(context, "year", format!("{:04}", fields.year))?,
        part(context, "literal", "-".into())?,
        part(context, "month", two_digit(fields.month))?,
        part(context, "literal", "-".into())?,
        part(context, "day", two_digit(fields.day))?,
    ];
    context.create_array(parts)
}

fn intl_date_time_format_format_to_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "DateTimeFormat")?;
    let ms = date_time_format_ms(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    date_time_parts(context, ms)
}

fn intl_date_time_format_format_range(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "DateTimeFormat")?;
    let start = date_time_format_ms(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let end = date_time_format_ms(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::String(format!(
        "{} - {}",
        format_date_fallback(start),
        format_date_fallback(end)
    )))
}

fn intl_date_time_format_format_range_to_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "DateTimeFormat")?;
    let start = date_time_format_ms(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let end = date_time_format_ms(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let start_part = source_part(
        context,
        "literal",
        format_date_fallback(start),
        "startRange",
    )?;
    let separator = source_part(context, "literal", " - ".into(), "shared")?;
    let end_part = source_part(context, "literal", format_date_fallback(end), "endRange")?;
    context.create_array(vec![start_part, separator, end_part])
}

fn intl_number_format_format_to_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "NumberFormat")?;
    let value = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let text = if value.is_finite() {
        JsValue::Number(value).to_js_string().unwrap_or_default()
    } else if value.is_nan() {
        "NaN".into()
    } else if value.is_sign_negative() {
        "-Infinity".into()
    } else {
        "Infinity".into()
    };
    let mut parts = Vec::new();
    let unsigned = text.strip_prefix('-').unwrap_or(&text);
    if text.starts_with('-') {
        parts.push(part(context, "minusSign", "-".into())?);
    }
    if let Some((integer, fraction)) = unsigned.split_once('.') {
        parts.push(part(context, "integer", integer.into())?);
        parts.push(part(context, "decimal", ".".into())?);
        parts.push(part(context, "fraction", fraction.into())?);
    } else {
        parts.push(part(context, "integer", unsigned.into())?);
    }
    context.create_array(parts)
}

fn intl_plural_rules_resolved_options(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "PluralRules")?;
    let plural_categories = context.create_array(vec![
        JsValue::String("one".into()),
        JsValue::String("other".into()),
    ])?;
    object_from_pairs(
        context,
        [
            ("locale", JsValue::String("en-US".into())),
            ("type", JsValue::String("cardinal".into())),
            ("pluralCategories", plural_categories),
        ],
    )
}

fn intl_plural_rules_select(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "PluralRules")?;
    let value = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    Ok(JsValue::String(if value.abs() == 1.0 {
        "one".into()
    } else {
        "other".into()
    }))
}

fn intl_plural_rules_select_range(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "PluralRules")?;
    let start = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let end = vm.to_number(
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    Ok(JsValue::String(if start == end && start.abs() == 1.0 {
        "one".into()
    } else {
        "other".into()
    }))
}

fn intl_relative_time_format_resolved_options(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "RelativeTimeFormat")?;
    object_from_pairs(
        context,
        [
            ("locale", JsValue::String("en-US".into())),
            ("style", JsValue::String("long".into())),
            ("numeric", JsValue::String("always".into())),
            ("numberingSystem", JsValue::String("latn".into())),
        ],
    )
}

fn relative_time_text(value: f64, unit: &str) -> String {
    let count = value.abs();
    let plural = if count == 1.0 { "" } else { "s" };
    let number = JsValue::Number(count).to_js_string().unwrap_or_default();
    if value < 0.0 {
        format!("{number} {unit}{plural} ago")
    } else {
        format!("in {number} {unit}{plural}")
    }
}

fn intl_relative_time_format_format(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "RelativeTimeFormat")?;
    let value = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let unit = vm.to_string_coerce(
        arguments
            .get(1)
            .cloned()
            .unwrap_or(JsValue::String("second".into())),
        context,
    )?;
    Ok(JsValue::String(relative_time_text(value, &unit)))
}

fn intl_relative_time_format_format_to_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "RelativeTimeFormat")?;
    let value = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let unit = vm.to_string_coerce(
        arguments
            .get(1)
            .cloned()
            .unwrap_or(JsValue::String("second".into())),
        context,
    )?;
    let literal = part(context, "literal", relative_time_text(value, &unit))?;
    context.create_array(vec![literal])
}

fn intl_list_format_resolved_options(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "ListFormat")?;
    object_from_pairs(
        context,
        [
            ("locale", JsValue::String("en-US".into())),
            ("type", JsValue::String("conjunction".into())),
            ("style", JsValue::String("long".into())),
        ],
    )
}

fn collect_list_items(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Vec<String>, VmError> {
    let object = context.require_object(&value, "Intl.ListFormat list")?;
    let length = context
        .get_property(context.object_value(object), "length")?
        .to_number()
        .unwrap_or(0.0)
        .max(0.0) as usize;
    let mut values = Vec::new();
    for index in 0..length {
        values.push(vm.to_string_coerce(
            context.get_property(context.object_value(object), &index.to_string())?,
            context,
        )?);
    }
    Ok(values)
}

fn list_format_text(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [left, right] => format!("{left} and {right}"),
        _ => {
            let mut text = items[..items.len() - 1].join(", ");
            text.push_str(", and ");
            text.push_str(items.last().unwrap());
            text
        }
    }
}

fn intl_list_format_format(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "ListFormat")?;
    let items = collect_list_items(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::String(list_format_text(&items)))
}

fn intl_list_format_format_to_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "ListFormat")?;
    let items = collect_list_items(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let literal = part(context, "literal", list_format_text(&items))?;
    context.create_array(vec![literal])
}

fn intl_locale_value(context: &NativeContext, this_value: &JsValue) -> Result<String, VmError> {
    let object = require_intl_kind(context, this_value, "Locale")?;
    Ok(own_string(context, object, INTL_LOCALE).unwrap_or_else(|| "und".into()))
}

fn intl_locale_base_name_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(intl_locale_value(context, &this_value)?))
}

fn intl_locale_language_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    Ok(JsValue::String(
        locale.split('-').next().unwrap_or("und").into(),
    ))
}

fn intl_locale_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(intl_locale_value(context, &this_value)?))
}

fn intl_locale_identity(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
}

fn install_temporal(context: &mut NativeContext) -> Result<(), VmError> {
    let temporal = new_ordinary_object(context, context.object_prototype())?;
    let to_string_tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        temporal,
        to_string_tag,
        readonly_configurable_descriptor(JsValue::String("Temporal".into())),
    )?;

    install_temporal_duration(context, temporal)?;
    install_temporal_instant(context, temporal)?;
    install_temporal_plain_date(context, temporal)?;
    install_temporal_plain_time(context, temporal)?;
    install_temporal_plain_date_time(context, temporal)?;
    install_temporal_plain_year_month(context, temporal)?;
    install_temporal_plain_month_day(context, temporal)?;
    install_temporal_zoned_date_time(context, temporal)?;
    install_temporal_now(context, temporal)?;

    declare_standard_global(context, "Temporal", JsValue::Object(temporal))?;
    Ok(())
}

fn temporal_constructor(
    context: &mut NativeContext,
    namespace: ObjectId,
    name: &'static str,
    length: u8,
    call: NativeCall,
    construct: NativeConstruct,
    prototype_tag: &'static str,
) -> Result<(JsValue, ObjectId), VmError> {
    let prototype = new_ordinary_object(context, context.object_prototype())?;
    let constructor = context.register_builtin(name, length, call, Some(construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("Temporal constructor object missing"))?;
    context.define_own_property(
        constructor_object,
        "prototype".into(),
        constant_descriptor(JsValue::Object(prototype)),
    )?;
    context.define_own_property(
        prototype,
        "constructor".into(),
        method_descriptor(constructor.clone()),
    )?;
    let tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        prototype,
        tag,
        readonly_configurable_descriptor(JsValue::String(prototype_tag.into())),
    )?;
    context.define_own_property(
        namespace,
        name.into(),
        method_descriptor(constructor.clone()),
    )?;
    Ok((constructor, prototype))
}

fn temporal_constructor_call_error(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "Temporal constructors must be called with new",
    ))
}

fn create_temporal_object(
    context: &mut NativeContext,
    prototype: ObjectId,
    kind: &'static str,
    slots: impl IntoIterator<Item = (&'static str, JsValue)>,
) -> Result<JsValue, VmError> {
    let object = new_ordinary_object(context, Some(prototype))?;
    define_hidden(context, object, TEMPORAL_KIND, JsValue::String(kind.into()))?;
    for (slot, value) in slots {
        define_hidden(context, object, slot, value)?;
    }
    Ok(JsValue::Object(object))
}

fn require_temporal_kind(
    context: &NativeContext,
    this_value: &JsValue,
    expected: &'static str,
) -> Result<ObjectId, VmError> {
    let object = context.require_object(this_value, "Temporal receiver")?;
    match own_string(context, object, TEMPORAL_KIND) {
        Some(kind) if kind == expected => Ok(object),
        _ => Err(VmError::type_error(format!(
            "receiver is not a Temporal.{expected} object"
        ))),
    }
}

fn temporal_number_slot(context: &NativeContext, object: ObjectId, slot: &str) -> f64 {
    own_number(context, object, slot).unwrap_or(0.0)
}

fn temporal_value_of(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error(
        "Temporal objects cannot be converted to primitive values",
    ))
}

fn install_temporal_duration(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "Duration",
        0,
        temporal_constructor_call_error,
        temporal_duration_construct,
        "Temporal.Duration",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_duration_from,
    )?;
    define_method(
        context,
        constructor_object,
        "compare",
        2,
        temporal_duration_compare,
    )?;
    for (name, length, call) in [
        ("abs", 0, temporal_duration_abs as NativeCall),
        ("add", 1, temporal_duration_add as NativeCall),
        ("negated", 0, temporal_duration_negated as NativeCall),
        ("round", 1, temporal_duration_round as NativeCall),
        ("subtract", 1, temporal_duration_subtract as NativeCall),
        ("total", 1, temporal_duration_total as NativeCall),
        ("with", 1, temporal_duration_with as NativeCall),
        (
            "toLocaleString",
            0,
            temporal_duration_to_string as NativeCall,
        ),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_duration_to_string,
    )?;
    define_method(context, prototype, "toJSON", 0, temporal_duration_to_string)?;
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [
        ("years", "get years", "years"),
        ("months", "get months", "months"),
        ("weeks", "get weeks", "weeks"),
        ("days", "get days", "days"),
        ("hours", "get hours", "hours"),
        ("minutes", "get minutes", "minutes"),
        ("seconds", "get seconds", "seconds"),
        ("milliseconds", "get milliseconds", "milliseconds"),
        ("microseconds", "get microseconds", "microseconds"),
        ("nanoseconds", "get nanoseconds", "nanoseconds"),
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "Duration", slot)?;
    }
    let sign_getter = context.register_builtin("get sign", 0, temporal_duration_sign_get, None)?;
    context.define_own_property(
        prototype,
        "sign".into(),
        PropertyDescriptor::accessor(Some(sign_getter), None, false, true),
    )?;
    let blank_getter =
        context.register_builtin("get blank", 0, temporal_duration_blank_get, None)?;
    context.define_own_property(
        prototype,
        "blank".into(),
        PropertyDescriptor::accessor(Some(blank_getter), None, false, true),
    )?;
    Ok(())
}

fn define_temporal_slot_getter(
    context: &mut NativeContext,
    prototype: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    kind: &'static str,
    slot: &'static str,
) -> Result<(), VmError> {
    let getter = context.register_builtin(getter_name, 0, temporal_slot_get, None)?;
    let getter_object = context
        .value_object(&getter)
        .ok_or_else(|| VmError::runtime("Temporal getter object missing"))?;
    define_hidden(
        context,
        getter_object,
        TEMPORAL_KIND,
        JsValue::String(kind.into()),
    )?;
    define_hidden(context, getter_object, "slot", JsValue::String(slot.into()))?;
    context.define_own_property(
        prototype,
        name.into(),
        PropertyDescriptor::accessor(Some(getter), None, false, true),
    )?;
    Ok(())
}

fn define_temporal_string_slot_getter(
    context: &mut NativeContext,
    prototype: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    kind: &'static str,
    slot: &'static str,
) -> Result<(), VmError> {
    let getter = context.register_builtin(getter_name, 0, temporal_string_slot_get, None)?;
    let getter_object = context
        .value_object(&getter)
        .ok_or_else(|| VmError::runtime("Temporal getter object missing"))?;
    define_hidden(
        context,
        getter_object,
        TEMPORAL_KIND,
        JsValue::String(kind.into()),
    )?;
    define_hidden(context, getter_object, "slot", JsValue::String(slot.into()))?;
    context.define_own_property(
        prototype,
        name.into(),
        PropertyDescriptor::accessor(Some(getter), None, false, true),
    )?;
    Ok(())
}

fn define_temporal_bool_slot_getter(
    context: &mut NativeContext,
    prototype: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    kind: &'static str,
    slot: &'static str,
) -> Result<(), VmError> {
    let getter = context.register_builtin(getter_name, 0, temporal_bool_slot_get, None)?;
    let getter_object = context
        .value_object(&getter)
        .ok_or_else(|| VmError::runtime("Temporal getter object missing"))?;
    define_hidden(
        context,
        getter_object,
        TEMPORAL_KIND,
        JsValue::String(kind.into()),
    )?;
    define_hidden(context, getter_object, "slot", JsValue::String(slot.into()))?;
    context.define_own_property(
        prototype,
        name.into(),
        PropertyDescriptor::accessor(Some(getter), None, false, true),
    )?;
    Ok(())
}

fn define_temporal_undefined_getter(
    context: &mut NativeContext,
    prototype: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    kind: &'static str,
) -> Result<(), VmError> {
    let getter = context.register_builtin(getter_name, 0, temporal_undefined_get, None)?;
    let getter_object = context
        .value_object(&getter)
        .ok_or_else(|| VmError::runtime("Temporal getter object missing"))?;
    define_hidden(
        context,
        getter_object,
        TEMPORAL_KIND,
        JsValue::String(kind.into()),
    )?;
    context.define_own_property(
        prototype,
        name.into(),
        PropertyDescriptor::accessor(Some(getter), None, false, true),
    )?;
    Ok(())
}

fn temporal_slot_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (kind, slot) = context
        .current_builtin_object()
        .and_then(|object| {
            Some((
                own_string(context, object, TEMPORAL_KIND)?,
                own_string(context, object, "slot")?,
            ))
        })
        .unwrap_or_else(|| ("".into(), "".into()));
    let object = require_temporal_kind(context, &this_value, Box::leak(kind.into_boxed_str()))?;
    Ok(JsValue::Number(temporal_number_slot(
        context, object, &slot,
    )))
}

fn temporal_string_slot_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (kind, slot) = context
        .current_builtin_object()
        .and_then(|object| {
            Some((
                own_string(context, object, TEMPORAL_KIND)?,
                own_string(context, object, "slot")?,
            ))
        })
        .unwrap_or_else(|| ("".into(), "".into()));
    let object = require_temporal_kind(context, &this_value, Box::leak(kind.into_boxed_str()))?;
    Ok(JsValue::String(
        own_string(context, object, &slot).unwrap_or_default(),
    ))
}

fn temporal_bool_slot_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (kind, slot) = context
        .current_builtin_object()
        .and_then(|object| {
            Some((
                own_string(context, object, TEMPORAL_KIND)?,
                own_string(context, object, "slot")?,
            ))
        })
        .unwrap_or_else(|| ("".into(), "".into()));
    let object = require_temporal_kind(context, &this_value, Box::leak(kind.into_boxed_str()))?;
    Ok(own_data_value(context, object, &slot)
        .filter(|value| matches!(value, JsValue::Boolean(_)))
        .unwrap_or(JsValue::Boolean(false)))
}

fn temporal_undefined_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let kind = context
        .current_builtin_object()
        .and_then(|object| own_string(context, object, TEMPORAL_KIND))
        .unwrap_or_default();
    require_temporal_kind(context, &this_value, Box::leak(kind.into_boxed_str()))?;
    Ok(JsValue::Undefined)
}

fn temporal_duration_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Temporal.Duration prototype missing"))?;
    let values = duration_values_from_args(vm, context, arguments)?;
    create_duration(context, prototype, values)
}

#[derive(Clone, Copy, Default)]
struct DurationValues {
    years: f64,
    months: f64,
    weeks: f64,
    days: f64,
    hours: f64,
    minutes: f64,
    seconds: f64,
    milliseconds: f64,
    microseconds: f64,
    nanoseconds: f64,
}

impl DurationValues {
    fn map(self, mut f: impl FnMut(f64) -> f64) -> Self {
        Self {
            years: f(self.years),
            months: f(self.months),
            weeks: f(self.weeks),
            days: f(self.days),
            hours: f(self.hours),
            minutes: f(self.minutes),
            seconds: f(self.seconds),
            milliseconds: f(self.milliseconds),
            microseconds: f(self.microseconds),
            nanoseconds: f(self.nanoseconds),
        }
    }
}

fn duration_values_from_args(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<DurationValues, VmError> {
    validate_duration_values(DurationValues {
        years: duration_integer(number_or_default(vm, context, arguments, 0, 0.0)?)?,
        months: duration_integer(number_or_default(vm, context, arguments, 1, 0.0)?)?,
        weeks: duration_integer(number_or_default(vm, context, arguments, 2, 0.0)?)?,
        days: duration_integer(number_or_default(vm, context, arguments, 3, 0.0)?)?,
        hours: duration_integer(number_or_default(vm, context, arguments, 4, 0.0)?)?,
        minutes: duration_integer(number_or_default(vm, context, arguments, 5, 0.0)?)?,
        seconds: duration_integer(number_or_default(vm, context, arguments, 6, 0.0)?)?,
        milliseconds: duration_integer(number_or_default(vm, context, arguments, 7, 0.0)?)?,
        microseconds: duration_integer(number_or_default(vm, context, arguments, 8, 0.0)?)?,
        nanoseconds: duration_integer(number_or_default(vm, context, arguments, 9, 0.0)?)?,
    })
}

fn duration_integer(value: f64) -> Result<f64, VmError> {
    if !value.is_finite() || value.fract() != 0.0 {
        return Err(VmError::range("invalid Temporal.Duration field"));
    }
    Ok(value)
}

fn create_duration(
    context: &mut NativeContext,
    prototype: ObjectId,
    values: DurationValues,
) -> Result<JsValue, VmError> {
    let values = values.map(clean_zero);
    create_temporal_object(
        context,
        prototype,
        "Duration",
        [
            ("years", JsValue::Number(values.years)),
            ("months", JsValue::Number(values.months)),
            ("weeks", JsValue::Number(values.weeks)),
            ("days", JsValue::Number(values.days)),
            ("hours", JsValue::Number(values.hours)),
            ("minutes", JsValue::Number(values.minutes)),
            ("seconds", JsValue::Number(values.seconds)),
            ("milliseconds", JsValue::Number(values.milliseconds)),
            ("microseconds", JsValue::Number(values.microseconds)),
            ("nanoseconds", JsValue::Number(values.nanoseconds)),
        ],
    )
}

fn temporal_duration_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let constructor = context
        .get_global("Temporal")
        .and_then(|temporal| context.value_object(&temporal))
        .and_then(|temporal| context.get_own_property_descriptor(temporal, "Duration"))
        .and_then(|descriptor| descriptor.value_cloned())
        .ok_or_else(|| VmError::runtime("Temporal.Duration missing"))?;
    let prototype = context
        .constructor_prototype(&constructor)?
        .ok_or_else(|| VmError::runtime("Temporal.Duration prototype missing"))?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let values = match item {
        JsValue::String(text) => parse_duration(&text)
            .ok_or_else(|| VmError::range("invalid Temporal.Duration string"))?,
        value => {
            let object = context.require_object(&value, "Temporal.Duration.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("Duration") {
                DurationValues {
                    years: temporal_number_slot(context, object, "years"),
                    months: temporal_number_slot(context, object, "months"),
                    weeks: temporal_number_slot(context, object, "weeks"),
                    days: temporal_number_slot(context, object, "days"),
                    hours: temporal_number_slot(context, object, "hours"),
                    minutes: temporal_number_slot(context, object, "minutes"),
                    seconds: temporal_number_slot(context, object, "seconds"),
                    milliseconds: temporal_number_slot(context, object, "milliseconds"),
                    microseconds: temporal_number_slot(context, object, "microseconds"),
                    nanoseconds: temporal_number_slot(context, object, "nanoseconds"),
                }
            } else {
                DurationValues {
                    years: temporal_object_number(vm, context, object, "years")?,
                    months: temporal_object_number(vm, context, object, "months")?,
                    weeks: temporal_object_number(vm, context, object, "weeks")?,
                    days: temporal_object_number(vm, context, object, "days")?,
                    hours: temporal_object_number(vm, context, object, "hours")?,
                    minutes: temporal_object_number(vm, context, object, "minutes")?,
                    seconds: temporal_object_number(vm, context, object, "seconds")?,
                    milliseconds: temporal_object_number(vm, context, object, "milliseconds")?,
                    microseconds: temporal_object_number(vm, context, object, "microseconds")?,
                    nanoseconds: temporal_object_number(vm, context, object, "nanoseconds")?,
                }
            }
        }
    };
    create_duration(context, prototype, values)
}

fn temporal_duration_constructor_prototype(context: &NativeContext) -> Result<ObjectId, VmError> {
    context
        .get_global("Temporal")
        .and_then(|temporal| context.value_object(&temporal))
        .and_then(|temporal| context.get_own_property_descriptor(temporal, "Duration"))
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten())
        .ok_or_else(|| VmError::runtime("Temporal.Duration prototype missing"))
}

fn duration_values_from_object(context: &NativeContext, object: ObjectId) -> DurationValues {
    DurationValues {
        years: temporal_number_slot(context, object, "years"),
        months: temporal_number_slot(context, object, "months"),
        weeks: temporal_number_slot(context, object, "weeks"),
        days: temporal_number_slot(context, object, "days"),
        hours: temporal_number_slot(context, object, "hours"),
        minutes: temporal_number_slot(context, object, "minutes"),
        seconds: temporal_number_slot(context, object, "seconds"),
        milliseconds: temporal_number_slot(context, object, "milliseconds"),
        microseconds: temporal_number_slot(context, object, "microseconds"),
        nanoseconds: temporal_number_slot(context, object, "nanoseconds"),
    }
}

fn duration_values_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<DurationValues, VmError> {
    match value {
        JsValue::String(text) => validate_duration_values(
            parse_duration(&text)
                .ok_or_else(|| VmError::range("invalid Temporal.Duration string"))?,
        ),
        other => {
            let object = context.require_object(&other, "Temporal.Duration value")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("Duration") {
                Ok(duration_values_from_object(context, object))
            } else {
                let mut seen = false;
                let mut read = |name: &str| -> Result<f64, VmError> {
                    let value = temporal_get_property(vm, context, object, name)?;
                    if matches!(value, JsValue::Undefined) {
                        Ok(0.0)
                    } else {
                        seen = true;
                        duration_integer(vm.to_number(value, context)?)
                    }
                };
                let days = read("days")?;
                let hours = read("hours")?;
                let microseconds = read("microseconds")?;
                let milliseconds = read("milliseconds")?;
                let minutes = read("minutes")?;
                let months = read("months")?;
                let nanoseconds = read("nanoseconds")?;
                let seconds = read("seconds")?;
                let weeks = read("weeks")?;
                let years = read("years")?;
                if !seen {
                    return Err(VmError::type_error(
                        "Temporal duration-like object has no duration properties",
                    ));
                }
                validate_duration_values(DurationValues {
                    years,
                    months,
                    weeks,
                    days,
                    hours,
                    minutes,
                    seconds,
                    milliseconds,
                    microseconds,
                    nanoseconds,
                })
            }
        }
    }
}

fn validate_duration_values(values: DurationValues) -> Result<DurationValues, VmError> {
    let mut sign = 0_i8;
    for value in [
        values.years,
        values.months,
        values.weeks,
        values.days,
        values.hours,
        values.minutes,
        values.seconds,
        values.milliseconds,
        values.microseconds,
        values.nanoseconds,
    ] {
        duration_integer(value)?;
        if value != 0.0 {
            let next = if value < 0.0 { -1 } else { 1 };
            if sign != 0 && sign != next {
                return Err(VmError::range(
                    "Temporal.Duration fields must have the same sign",
                ));
            }
            sign = next;
        }
    }
    Ok(values)
}

fn duration_this_values(
    context: &NativeContext,
    this_value: &JsValue,
) -> Result<DurationValues, VmError> {
    let object = require_temporal_kind(context, this_value, "Duration")?;
    Ok(duration_values_from_object(context, object))
}

fn create_duration_with_default_prototype(
    context: &mut NativeContext,
    values: DurationValues,
) -> Result<JsValue, VmError> {
    let prototype = temporal_duration_constructor_prototype(context)?;
    create_duration(context, prototype, values)
}

fn temporal_duration_abs(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let values = duration_this_values(context, &this_value)?;
    create_duration_with_default_prototype(context, values.map(|value| value.abs()))
}

fn temporal_duration_negated(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let values = duration_this_values(context, &this_value)?;
    create_duration_with_default_prototype(
        context,
        values.map(|value| if value == 0.0 { 0.0 } else { -value }),
    )
}

fn temporal_duration_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    duration_additive(vm, context, this_value, arguments, 1.0)
}

fn temporal_duration_subtract(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    duration_additive(vm, context, this_value, arguments, -1.0)
}

fn duration_additive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: f64,
) -> Result<JsValue, VmError> {
    let left = duration_this_values(context, &this_value)?;
    let right = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    create_duration_with_default_prototype(
        context,
        balance_duration(DurationValues {
            years: left.years + sign * right.years,
            months: left.months + sign * right.months,
            weeks: left.weeks + sign * right.weeks,
            days: left.days + sign * right.days,
            hours: left.hours + sign * right.hours,
            minutes: left.minutes + sign * right.minutes,
            seconds: left.seconds + sign * right.seconds,
            milliseconds: left.milliseconds + sign * right.milliseconds,
            microseconds: left.microseconds + sign * right.microseconds,
            nanoseconds: left.nanoseconds + sign * right.nanoseconds,
        }),
    )
}

fn temporal_duration_round(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let values = duration_this_values(context, &this_value)?;
    let options = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (largest_unit, smallest_unit, increment, mode) =
        duration_round_options(vm, context, options, values)?;
    let quantum = temporal_unit_nanoseconds(&smallest_unit)
        .checked_mul(increment as i128)
        .ok_or_else(|| VmError::range("invalid Temporal rounding increment"))?;
    let total = duration_order_total(values).round() as i128;
    let rounded = round_signed_i128(total, quantum, mode);
    create_duration_from_nanoseconds(context, rounded, &largest_unit)
}

fn duration_largest_unit(values: DurationValues) -> String {
    [
        ("year", values.years),
        ("month", values.months),
        ("week", values.weeks),
        ("day", values.days),
        ("hour", values.hours),
        ("minute", values.minutes),
        ("second", values.seconds),
        ("millisecond", values.milliseconds),
        ("microsecond", values.microseconds),
        ("nanosecond", values.nanoseconds),
    ]
    .into_iter()
    .find(|(_, value)| *value != 0.0)
    .map(|(unit, _)| unit.to_string())
    .unwrap_or_else(|| "nanosecond".into())
}

fn duration_round_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    values: DurationValues,
) -> Result<(String, String, u64, TemporalRoundMode), VmError> {
    if let JsValue::String(unit) = value {
        let smallest = normalize_temporal_unit(unit)?;
        let largest = duration_largest_unit(values);
        let largest = if temporal_unit_nanoseconds(&smallest) > temporal_unit_nanoseconds(&largest)
        {
            smallest.clone()
        } else {
            largest
        };
        return Ok((largest, smallest, 1, TemporalRoundMode::HalfExpand));
    }
    let object = context.require_object(&value, "Temporal.Duration.prototype.round options")?;
    let largest = option_string(vm, context, object, "largestUnit")?;
    let _relative_to = temporal_get_property(vm, context, object, "relativeTo")?;
    let increment = option_rounding_increment(vm, context, object)?;
    let mode = temporal_round_mode(
        option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
        TemporalRoundMode::HalfExpand,
    )?;
    let smallest = option_string(vm, context, object, "smallestUnit")?;
    if largest.is_none() && smallest.is_none() {
        return Err(VmError::range("largestUnit or smallestUnit is required"));
    }
    let input_largest = duration_largest_unit(values);
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "nanosecond".into()))?;
    let largest_unit = match largest.as_deref() {
        Some("auto") | None => {
            if temporal_unit_nanoseconds(&smallest_unit) > temporal_unit_nanoseconds(&input_largest)
            {
                smallest_unit.clone()
            } else {
                input_largest
            }
        }
        Some(unit) => normalize_temporal_unit(unit.to_string())?,
    };
    if temporal_unit_nanoseconds(&largest_unit) < temporal_unit_nanoseconds(&smallest_unit) {
        return Err(VmError::range(
            "largestUnit must not be smaller than smallestUnit",
        ));
    }
    if increment == 0 {
        return Err(VmError::range("invalid Temporal rounding increment"));
    }
    Ok((largest_unit, smallest_unit, increment, mode))
}

fn temporal_duration_total(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let values = duration_this_values(context, &this_value)?;
    let unit = duration_total_unit(vm, context, arguments.first().cloned())?;
    let total_ns = duration_order_total(values);
    let value = match unit.as_str() {
        "year" => total_ns / (365.0 * NS_PER_DAY),
        "month" => total_ns / (30.0 * NS_PER_DAY),
        "week" => total_ns / (7.0 * NS_PER_DAY),
        "day" => total_ns / NS_PER_DAY,
        "hour" => total_ns / NS_PER_HOUR,
        "minute" => total_ns / NS_PER_MINUTE,
        "second" => total_ns / NS_PER_SECOND,
        "millisecond" => total_ns / NS_PER_MILLISECOND,
        "microsecond" => total_ns / NS_PER_MICROSECOND,
        "nanosecond" => total_ns,
        _ => return Err(VmError::range("invalid Temporal.Duration total unit")),
    };
    Ok(JsValue::Number(value))
}

fn duration_total_unit(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: Option<JsValue>,
) -> Result<String, VmError> {
    match value.unwrap_or(JsValue::Undefined) {
        JsValue::String(unit) => normalize_temporal_unit(unit),
        JsValue::Object(object) => {
            let _relative_to = temporal_get_property(vm, context, object, "relativeTo")?;
            let unit = temporal_get_property(vm, context, object, "unit")?;
            if matches!(unit, JsValue::Undefined) {
                return Err(VmError::range("Temporal.Duration total requires a unit"));
            }
            normalize_temporal_unit(vm.to_string_coerce(unit, context)?)
        }
        JsValue::Undefined => Err(VmError::range("Temporal.Duration total requires a unit")),
        _ => Err(VmError::type_error(
            "Temporal.Duration total options must be an object or string",
        )),
    }
}

fn temporal_duration_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let mut values = duration_this_values(context, &this_value)?;
    let object = context.require_object(
        &arguments.first().cloned().unwrap_or(JsValue::Undefined),
        "Temporal.Duration.prototype.with",
    )?;
    values.years = duration_replacement(vm, context, object, "years", values.years)?;
    values.months = duration_replacement(vm, context, object, "months", values.months)?;
    values.weeks = duration_replacement(vm, context, object, "weeks", values.weeks)?;
    values.days = duration_replacement(vm, context, object, "days", values.days)?;
    values.hours = duration_replacement(vm, context, object, "hours", values.hours)?;
    values.minutes = duration_replacement(vm, context, object, "minutes", values.minutes)?;
    values.seconds = duration_replacement(vm, context, object, "seconds", values.seconds)?;
    values.milliseconds =
        duration_replacement(vm, context, object, "milliseconds", values.milliseconds)?;
    values.microseconds =
        duration_replacement(vm, context, object, "microseconds", values.microseconds)?;
    values.nanoseconds =
        duration_replacement(vm, context, object, "nanoseconds", values.nanoseconds)?;
    create_duration_with_default_prototype(context, values)
}

fn duration_replacement(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
    current: f64,
) -> Result<f64, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    if matches!(value, JsValue::Undefined) {
        Ok(current)
    } else {
        duration_integer(vm.to_number(value, context)?)
    }
}

fn temporal_duration_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let right = duration_values_from_value(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let left_total = duration_order_total(left);
    let right_total = duration_order_total(right);
    Ok(JsValue::Number(if left_total < right_total {
        -1.0
    } else if left_total > right_total {
        1.0
    } else {
        0.0
    }))
}

fn temporal_duration_sign_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let values = duration_this_values(context, &this_value)?;
    Ok(JsValue::Number(duration_sign(values) as f64))
}

fn temporal_duration_blank_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let values = duration_this_values(context, &this_value)?;
    Ok(JsValue::Boolean(duration_sign(values) == 0))
}

const NS_PER_MICROSECOND: f64 = 1_000.0;
const NS_PER_MILLISECOND: f64 = 1_000_000.0;
const NS_PER_SECOND: f64 = 1_000_000_000.0;
const NS_PER_MINUTE: f64 = 60.0 * NS_PER_SECOND;
const NS_PER_HOUR: f64 = 60.0 * NS_PER_MINUTE;
const NS_PER_DAY: f64 = 24.0 * NS_PER_HOUR;

fn duration_time_nanoseconds(values: DurationValues) -> f64 {
    values.hours * NS_PER_HOUR
        + values.minutes * NS_PER_MINUTE
        + values.seconds * NS_PER_SECOND
        + values.milliseconds * NS_PER_MILLISECOND
        + values.microseconds * NS_PER_MICROSECOND
        + values.nanoseconds
}

fn balance_duration(values: DurationValues) -> DurationValues {
    let total_ns = duration_time_nanoseconds(values);
    let sign = if total_ns < 0.0 { -1.0 } else { 1.0 };
    let mut remainder = total_ns.abs().round();
    let extra_days = (remainder / NS_PER_DAY).floor();
    remainder -= extra_days * NS_PER_DAY;
    let hours = (remainder / NS_PER_HOUR).floor();
    remainder -= hours * NS_PER_HOUR;
    let minutes = (remainder / NS_PER_MINUTE).floor();
    remainder -= minutes * NS_PER_MINUTE;
    let seconds = (remainder / NS_PER_SECOND).floor();
    remainder -= seconds * NS_PER_SECOND;
    let milliseconds = (remainder / NS_PER_MILLISECOND).floor();
    remainder -= milliseconds * NS_PER_MILLISECOND;
    let microseconds = (remainder / NS_PER_MICROSECOND).floor();
    remainder -= microseconds * NS_PER_MICROSECOND;
    DurationValues {
        years: values.years,
        months: values.months,
        weeks: values.weeks,
        days: values.days + sign * extra_days,
        hours: clean_zero(sign * hours),
        minutes: clean_zero(sign * minutes),
        seconds: clean_zero(sign * seconds),
        milliseconds: clean_zero(sign * milliseconds),
        microseconds: clean_zero(sign * microseconds),
        nanoseconds: clean_zero(sign * remainder),
    }
}

fn clean_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn duration_order_total(values: DurationValues) -> f64 {
    values.years * 365.0 * NS_PER_DAY
        + values.months * 30.0 * NS_PER_DAY
        + values.weeks * 7.0 * NS_PER_DAY
        + values.days * NS_PER_DAY
        + duration_time_nanoseconds(values)
}

fn duration_sign(values: DurationValues) -> i8 {
    let total = duration_order_total(values);
    if total > 0.0 {
        1
    } else if total < 0.0 {
        -1
    } else {
        0
    }
}

fn temporal_get_property(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<JsValue, VmError> {
    vm.get_property_value_with_receiver_from_builtin(
        JsValue::Object(object),
        JsValue::Object(object),
        name,
        context,
    )
}

fn temporal_object_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<f64, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    if matches!(value, JsValue::Undefined) {
        Ok(0.0)
    } else {
        vm.to_number(value, context)
    }
}

fn temporal_required_object_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<f64, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    if matches!(value, JsValue::Undefined) {
        Err(VmError::type_error(format!(
            "Temporal property `{name}` is required"
        )))
    } else {
        vm.to_number(value, context)
    }
}

fn temporal_required_month_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
) -> Result<f64, VmError> {
    let month_value = temporal_get_property(vm, context, object, "month")?;
    if !matches!(month_value, JsValue::Undefined) {
        return Ok(vm.to_number(month_value, context)?.trunc());
    }
    let month_code_value = temporal_get_property(vm, context, object, "monthCode")?;
    if matches!(month_code_value, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal property `month` or `monthCode` is required",
        ));
    }
    let month_code = vm.to_string_coerce(month_code_value, context)?;
    parse_month_code(&month_code).ok_or_else(|| VmError::range("invalid Temporal monthCode"))
}

fn parse_duration(text: &str) -> Option<DurationValues> {
    let text = text.trim();
    let (duration_sign, body) = if let Some(body) = text.strip_prefix('-') {
        (-1.0, body)
    } else if let Some(body) = text.strip_prefix('+') {
        (1.0, body)
    } else {
        (1.0, text)
    };
    let mut chars = body.strip_prefix('P')?.chars().peekable();
    let mut values = DurationValues::default();
    let mut in_time = false;
    let mut saw_part = false;
    let mut seen = std::collections::HashSet::new();
    while let Some(ch) = chars.peek().copied() {
        if ch == 'T' {
            if in_time {
                return None;
            }
            in_time = true;
            chars.next();
            continue;
        }
        let mut number = String::new();
        while let Some(digit) = chars.peek().copied() {
            if digit.is_ascii_digit() || digit == '.' {
                number.push(digit);
                chars.next();
            } else {
                break;
            }
        }
        if number.is_empty() || number == "." {
            return None;
        }
        let amount = number.parse::<f64>().ok()?;
        if !amount.is_finite() {
            return None;
        }
        let designator = chars.next()?;
        let key = if in_time && designator == 'M' {
            "time-minute"
        } else {
            match designator {
                'Y' => "year",
                'M' => "month",
                'W' => "week",
                'D' => "day",
                'H' => "hour",
                'S' => "second",
                _ => return None,
            }
        };
        if !seen.insert(key) {
            return None;
        }
        saw_part = true;
        let signed = duration_sign * amount;
        match (designator, in_time) {
            ('Y', false) => values.years = signed,
            ('M', false) => values.months = signed,
            ('W', false) => values.weeks = signed,
            ('D', false) => values.days = signed,
            ('H', true) => {
                let total = (signed * NS_PER_HOUR).round();
                let balanced = balance_duration(DurationValues {
                    nanoseconds: total,
                    ..DurationValues::default()
                });
                values.hours += balanced.hours;
                values.minutes += balanced.minutes;
                values.seconds += balanced.seconds;
                values.milliseconds += balanced.milliseconds;
                values.microseconds += balanced.microseconds;
                values.nanoseconds += balanced.nanoseconds;
            }
            ('M', true) => {
                let total = (signed * NS_PER_MINUTE).round();
                let balanced = balance_duration(DurationValues {
                    nanoseconds: total,
                    ..DurationValues::default()
                });
                values.minutes += balanced.minutes;
                values.seconds += balanced.seconds;
                values.milliseconds += balanced.milliseconds;
                values.microseconds += balanced.microseconds;
                values.nanoseconds += balanced.nanoseconds;
            }
            ('S', true) => {
                let total = (signed * NS_PER_SECOND).round();
                let balanced = balance_duration(DurationValues {
                    nanoseconds: total,
                    ..DurationValues::default()
                });
                values.seconds += balanced.seconds;
                values.milliseconds += balanced.milliseconds;
                values.microseconds += balanced.microseconds;
                values.nanoseconds += balanced.nanoseconds;
            }
            _ => return None,
        }
    }
    saw_part.then_some(values)
}

fn temporal_duration_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Duration")?;
    let values = DurationValues {
        years: temporal_number_slot(context, object, "years"),
        months: temporal_number_slot(context, object, "months"),
        weeks: temporal_number_slot(context, object, "weeks"),
        days: temporal_number_slot(context, object, "days"),
        hours: temporal_number_slot(context, object, "hours"),
        minutes: temporal_number_slot(context, object, "minutes"),
        seconds: temporal_number_slot(context, object, "seconds"),
        milliseconds: temporal_number_slot(context, object, "milliseconds"),
        microseconds: temporal_number_slot(context, object, "microseconds"),
        nanoseconds: temporal_number_slot(context, object, "nanoseconds"),
    };
    Ok(JsValue::String(format_duration(values)))
}

fn push_duration_part(text: &mut String, value: f64, suffix: &str) {
    if value != 0.0 {
        text.push_str(&JsValue::Number(value).to_js_string().unwrap_or_default());
        text.push_str(suffix);
    }
}

fn format_duration(values: DurationValues) -> String {
    let negative = duration_sign(values) < 0;
    let values = values.map(f64::abs);
    let mut text = if negative {
        String::from("-P")
    } else {
        String::from("P")
    };
    push_duration_part(&mut text, values.years, "Y");
    push_duration_part(&mut text, values.months, "M");
    push_duration_part(&mut text, values.weeks, "W");
    push_duration_part(&mut text, values.days, "D");
    let mut time = String::new();
    push_duration_part(&mut time, values.hours, "H");
    push_duration_part(&mut time, values.minutes, "M");
    let subsecond_ns = (values.milliseconds as u128)
        .saturating_mul(1_000_000)
        .saturating_add((values.microseconds as u128).saturating_mul(1_000))
        .saturating_add(values.nanoseconds as u128);
    if values.seconds != 0.0 || subsecond_ns != 0 {
        time.push_str(
            &JsValue::Number(values.seconds)
                .to_js_string()
                .unwrap_or_default(),
        );
        if subsecond_ns != 0 {
            let mut fraction = format!("{subsecond_ns:09}");
            while fraction.ends_with('0') {
                fraction.pop();
            }
            time.push('.');
            time.push_str(&fraction);
        }
        time.push('S');
    }
    if !time.is_empty() {
        text.push('T');
        text.push_str(&time);
    }
    if text == "P" || text == "-P" {
        "PT0S".into()
    } else {
        text
    }
}

fn install_temporal_instant(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "Instant",
        1,
        temporal_constructor_call_error,
        temporal_instant_construct,
        "Temporal.Instant",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_instant_from,
    )?;
    define_method(
        context,
        constructor_object,
        "fromEpochMilliseconds",
        1,
        temporal_instant_from_epoch_milliseconds,
    )?;
    define_method(
        context,
        constructor_object,
        "fromEpochNanoseconds",
        1,
        temporal_instant_from_epoch_nanoseconds,
    )?;
    define_method(
        context,
        constructor_object,
        "fromEpochSeconds",
        1,
        temporal_instant_from_epoch_seconds,
    )?;
    define_method(
        context,
        constructor_object,
        "compare",
        2,
        temporal_instant_compare,
    )?;
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_instant_to_string,
    )?;
    define_method(context, prototype, "toJSON", 0, temporal_instant_to_string)?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_instant_to_string,
    )?;
    for (name, length, call) in [
        ("add", 1, temporal_instant_add as NativeCall),
        ("subtract", 1, temporal_instant_subtract as NativeCall),
        ("equals", 1, temporal_instant_equals as NativeCall),
        ("round", 1, temporal_instant_round as NativeCall),
        ("until", 1, temporal_instant_until as NativeCall),
        ("since", 1, temporal_instant_since as NativeCall),
        (
            "toZonedDateTimeISO",
            1,
            temporal_instant_to_zoned_date_time_iso as NativeCall,
        ),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    define_temporal_slot_getter(
        context,
        prototype,
        "epochMilliseconds",
        "get epochMilliseconds",
        "Instant",
        "epochMilliseconds",
    )?;
    let epoch_nanoseconds_getter = context.register_builtin(
        "get epochNanoseconds",
        0,
        temporal_instant_epoch_nanoseconds_get,
        None,
    )?;
    context.define_own_property(
        prototype,
        "epochNanoseconds".into(),
        PropertyDescriptor::accessor(Some(epoch_nanoseconds_getter), None, false, true),
    )?;
    Ok(())
}

fn create_instant(
    context: &mut NativeContext,
    prototype: ObjectId,
    epoch_ms: f64,
) -> Result<JsValue, VmError> {
    let clipped = time_clip(epoch_ms);
    let epoch_ns = if clipped.is_finite() {
        (clipped as i128).saturating_mul(1_000_000)
    } else {
        0
    };
    create_instant_from_epoch_ns(context, prototype, epoch_ns)
}

fn create_instant_from_epoch_ns(
    context: &mut NativeContext,
    prototype: ObjectId,
    epoch_ns: i128,
) -> Result<JsValue, VmError> {
    if !is_valid_instant_ns(epoch_ns) {
        return Err(VmError::range("invalid Temporal.Instant"));
    }
    let epoch_ms = epoch_ns.div_euclid(NS_PER_MILLISECOND_I128) as f64;
    create_temporal_object(
        context,
        prototype,
        "Instant",
        [
            ("epochMilliseconds", JsValue::Number(epoch_ms)),
            (
                "epochNanoseconds",
                JsValue::BigInt(bigint::from_i128(epoch_ns)),
            ),
        ],
    )
}

fn instant_epoch_ns(context: &NativeContext, object: ObjectId) -> i128 {
    match own_data_value(context, object, "epochNanoseconds") {
        Some(JsValue::BigInt(value)) => bigint_to_i128_saturating(&value),
        Some(JsValue::Number(value)) => value as i128,
        _ => (temporal_number_slot(context, object, "epochMilliseconds") as i128)
            .saturating_mul(1_000_000),
    }
}

fn temporal_instant_epoch_nanoseconds_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Instant")?;
    Ok(own_data_value(context, object, "epochNanoseconds")
        .unwrap_or_else(|| JsValue::BigInt(bigint::from_i64(0))))
}

fn temporal_instant_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.Instant prototype missing"))?;
    let Some(argument) = arguments.first().cloned() else {
        return Err(VmError::type_error(
            "Temporal.Instant requires epochNanoseconds",
        ));
    };
    let epoch_ns = bigint_to_i128_saturating(&temporal_to_bigint(_vm, context, argument)?);
    create_instant_from_epoch_ns(context, prototype, epoch_ns)
}

fn temporal_instant_constructor_prototype(context: &NativeContext) -> Result<ObjectId, VmError> {
    context
        .get_global("Temporal")
        .and_then(|temporal| context.value_object(&temporal))
        .and_then(|temporal| context.get_own_property_descriptor(temporal, "Instant"))
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten())
        .ok_or_else(|| VmError::runtime("Temporal.Instant prototype missing"))
}

fn temporal_instant_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_instant_constructor_prototype(context)?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let epoch_ns = match item {
        JsValue::Undefined | JsValue::Null => {
            return Err(VmError::type_error(
                "Temporal.Instant.from requires an instant value",
            ));
        }
        JsValue::String(text) => {
            parse_instant_string(&text).ok_or_else(|| VmError::range("invalid Temporal.Instant"))?
        }
        value => {
            if let Some(object) = context.value_object(&value)
                && own_string(context, object, TEMPORAL_KIND).as_deref() == Some("Instant")
            {
                instant_epoch_ns(context, object)
            } else if let Some(object) = context.value_object(&value)
                && own_string(context, object, TEMPORAL_KIND).as_deref() == Some("ZonedDateTime")
            {
                zoned_date_time_epoch_ns(context, object)
            } else {
                let text = vm.to_string_coerce(value, context)?;
                parse_instant_string(&text)
                    .ok_or_else(|| VmError::range("invalid Temporal.Instant"))?
            }
        }
    };
    create_instant_from_epoch_ns(context, prototype, epoch_ns)
}

fn temporal_instant_from_epoch_milliseconds(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_instant_constructor_prototype(context)?;
    let argument = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let ms = vm.to_number(argument, context)?;
    if !ms.is_finite() || ms.fract() != 0.0 {
        return Err(VmError::range("invalid Temporal.Instant epochMilliseconds"));
    }
    create_instant_from_epoch_ns(context, prototype, (ms as i128) * NS_PER_MILLISECOND_I128)
}

fn temporal_instant_from_epoch_nanoseconds(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_instant_constructor_prototype(context)?;
    let Some(argument) = arguments.first().cloned() else {
        return Err(VmError::type_error(
            "Temporal.Instant.fromEpochNanoseconds requires an argument",
        ));
    };
    let epoch_ns = bigint_to_i128_saturating(&temporal_to_bigint(_vm, context, argument)?);
    create_instant_from_epoch_ns(context, prototype, epoch_ns)
}

fn temporal_instant_from_epoch_seconds(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_instant_constructor_prototype(context)?;
    let argument = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let seconds = vm.to_number(argument, context)?;
    if !seconds.is_finite() || seconds.fract() != 0.0 {
        return Err(VmError::range("invalid Temporal.Instant epochSeconds"));
    }
    create_instant_from_epoch_ns(context, prototype, (seconds as i128) * NS_PER_SECOND_I128)
}

fn instant_ns_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<i128, VmError> {
    if let Some(object) = context.value_object(&value)
        && own_string(context, object, TEMPORAL_KIND).as_deref() == Some("Instant")
    {
        return Ok(instant_epoch_ns(context, object));
    }
    if let Some(object) = context.value_object(&value)
        && own_string(context, object, TEMPORAL_KIND).as_deref() == Some("ZonedDateTime")
    {
        return Ok(zoned_date_time_epoch_ns(context, object));
    }
    match value {
        JsValue::Undefined | JsValue::Null => Err(VmError::type_error(
            "Temporal.Instant.compare requires instant values",
        )),
        JsValue::String(text) => {
            parse_instant_string(&text).ok_or_else(|| VmError::range("invalid Temporal.Instant"))
        }
        other => {
            let text = vm.to_string_coerce(other, context)?;
            parse_instant_string(&text).ok_or_else(|| VmError::range("invalid Temporal.Instant"))
        }
    }
}

fn temporal_instant_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = instant_ns_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let right = instant_ns_from_value(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::Number(if left < right {
        -1.0
    } else if left > right {
        1.0
    } else {
        0.0
    }))
}

fn temporal_instant_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Instant")?;
    let option_value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let options = temporal_string_options(vm, context, option_value.clone())?;
    let time_zone = if matches!(option_value, JsValue::Undefined) {
        None
    } else {
        let option_object =
            context.require_object(&option_value, "Temporal.Instant toString options")?;
        let value = temporal_get_property(vm, context, option_object, "timeZone")?;
        if matches!(value, JsValue::Undefined) {
            None
        } else if matches!(value, JsValue::Null) {
            return Err(VmError::type_error("invalid Temporal time zone"));
        } else {
            Some(vm.to_string_coerce(value, context)?)
        }
    };
    let epoch_ns = round_i128(
        instant_epoch_ns(context, object),
        options.quantum,
        options.mode,
    );
    let (offset_ns, suffix) = match time_zone.as_deref() {
        None => (0, "Z".to_string()),
        Some("UTC") => (0, "+00:00".to_string()),
        Some(zone) => {
            let offset = parse_time_zone_offset_ns(zone)
                .ok_or_else(|| VmError::range("invalid Temporal time zone"))?;
            (offset, zone.to_string())
        }
    };
    let local_ns = epoch_ns
        .checked_add(offset_ns)
        .ok_or_else(|| VmError::range("Temporal.Instant is out of range"))?;
    let day_number = i64::try_from(local_ns.div_euclid(NS_PER_DAY_I128))
        .map_err(|_| VmError::range("Temporal.Instant is out of range"))?;
    let (year, month, day) = civil_from_days(day_number);
    let time = format_plain_time_precision(
        plain_time_from_nanoseconds_i128(local_ns.rem_euclid(NS_PER_DAY_I128)),
        options.precision,
        options.minute_only,
    );
    Ok(JsValue::String(format!(
        "{}-{}-{}T{}{}",
        iso_year(year),
        two_digit(month),
        two_digit(day),
        time,
        suffix
    )))
}

fn temporal_instant_additive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: i128,
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Instant")?;
    let duration = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    if duration.years != 0.0
        || duration.months != 0.0
        || duration.weeks != 0.0
        || duration.days != 0.0
    {
        return Err(VmError::range(
            "Temporal.Instant arithmetic does not accept calendar units",
        ));
    }
    let components = [
        (duration.hours, NS_PER_HOUR_I128),
        (duration.minutes, NS_PER_MINUTE_I128),
        (duration.seconds, NS_PER_SECOND_I128),
        (duration.milliseconds, NS_PER_MILLISECOND_I128),
        (duration.microseconds, 1_000),
        (duration.nanoseconds, 1),
    ];
    let mut component_sign = 0_i8;
    let mut delta = 0_i128;
    for (value, scale) in components {
        if !value.is_finite() || value.fract() != 0.0 {
            return Err(VmError::range(
                "Temporal duration fields must be finite integers",
            ));
        }
        if value != 0.0 {
            let next_sign = if value < 0.0 { -1 } else { 1 };
            if component_sign != 0 && component_sign != next_sign {
                return Err(VmError::range(
                    "Temporal duration fields must have the same sign",
                ));
            }
            component_sign = next_sign;
        }
        delta = delta
            .checked_add((value as i128).saturating_mul(scale))
            .ok_or_else(|| VmError::range("Temporal duration is out of range"))?;
    }
    delta = delta
        .checked_mul(sign)
        .ok_or_else(|| VmError::range("Temporal duration is out of range"))?;
    let result = instant_epoch_ns(context, object)
        .checked_add(delta)
        .ok_or_else(|| VmError::range("Temporal.Instant is out of range"))?;
    let prototype = temporal_instant_constructor_prototype(context)?;
    create_instant_from_epoch_ns(context, prototype, result)
}

fn temporal_instant_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_instant_additive(vm, context, this_value, arguments, 1)
}

fn temporal_instant_subtract(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_instant_additive(vm, context, this_value, arguments, -1)
}

fn temporal_instant_equals(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "Instant")?;
    let other = temporal_instant_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    Ok(JsValue::Boolean(
        instant_epoch_ns(context, this_object) == instant_epoch_ns(context, other_object),
    ))
}

fn temporal_instant_round(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Instant")?;
    let options = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (smallest_unit, increment, mode) = instant_round_options(vm, context, options)?;
    let quantum = temporal_unit_nanoseconds(&smallest_unit)
        .checked_mul(increment as i128)
        .ok_or_else(|| VmError::range("invalid Temporal rounding increment"))?;
    let rounded = round_i128(instant_epoch_ns(context, object), quantum, mode);
    let prototype = temporal_instant_constructor_prototype(context)?;
    create_instant_from_epoch_ns(context, prototype, rounded)
}

#[derive(Clone, Copy)]
enum TemporalRoundMode {
    Ceil,
    Expand,
    Floor,
    Trunc,
    HalfCeil,
    HalfExpand,
    HalfFloor,
    HalfTrunc,
    HalfEven,
}

fn normalize_temporal_unit(unit: String) -> Result<String, VmError> {
    let normalized = unit.strip_suffix('s').unwrap_or(&unit).to_string();
    match normalized.as_str() {
        "year" | "month" | "week" | "day" | "hour" | "minute" | "second" | "millisecond"
        | "microsecond" | "nanosecond" => Ok(normalized),
        _ => Err(VmError::range("invalid Temporal unit")),
    }
}

fn temporal_unit_nanoseconds(unit: &str) -> i128 {
    match unit {
        "year" => 365 * NS_PER_DAY_I128,
        "month" => 30 * NS_PER_DAY_I128,
        "week" => 7 * NS_PER_DAY_I128,
        "day" => NS_PER_DAY_I128,
        "hour" => NS_PER_HOUR_I128,
        "minute" => NS_PER_MINUTE_I128,
        "second" => NS_PER_SECOND_I128,
        "millisecond" => NS_PER_MILLISECOND_I128,
        "microsecond" => 1_000,
        _ => 1,
    }
}

fn temporal_round_mode(
    text: String,
    default: TemporalRoundMode,
) -> Result<TemporalRoundMode, VmError> {
    if text.is_empty() {
        return Ok(default);
    }
    match text.as_str() {
        "ceil" => Ok(TemporalRoundMode::Ceil),
        "expand" => Ok(TemporalRoundMode::Expand),
        "floor" => Ok(TemporalRoundMode::Floor),
        "trunc" => Ok(TemporalRoundMode::Trunc),
        "halfCeil" => Ok(TemporalRoundMode::HalfCeil),
        "halfExpand" => Ok(TemporalRoundMode::HalfExpand),
        "halfFloor" => Ok(TemporalRoundMode::HalfFloor),
        "halfTrunc" => Ok(TemporalRoundMode::HalfTrunc),
        "halfEven" => Ok(TemporalRoundMode::HalfEven),
        _ => Err(VmError::range("invalid Temporal rounding mode")),
    }
}

fn round_i128(value: i128, quantum: i128, mode: TemporalRoundMode) -> i128 {
    let floor = value.div_euclid(quantum) * quantum;
    let ceil = if value.rem_euclid(quantum) == 0 {
        floor
    } else {
        floor + quantum
    };
    let trunc = floor;
    let expand = ceil;
    match mode {
        TemporalRoundMode::Floor => floor,
        TemporalRoundMode::Ceil => ceil,
        TemporalRoundMode::Trunc => trunc,
        TemporalRoundMode::Expand => expand,
        half_mode => {
            let distance_down = value - floor;
            let distance_up = ceil - value;
            if distance_down < distance_up {
                floor
            } else if distance_up < distance_down {
                ceil
            } else {
                match half_mode {
                    TemporalRoundMode::HalfCeil => ceil,
                    TemporalRoundMode::HalfFloor => floor,
                    TemporalRoundMode::HalfTrunc => trunc,
                    TemporalRoundMode::HalfEven => {
                        if floor.div_euclid(quantum) % 2 == 0 {
                            floor
                        } else {
                            ceil
                        }
                    }
                    _ => expand,
                }
            }
        }
    }
}

fn round_signed_i128(value: i128, quantum: i128, mode: TemporalRoundMode) -> i128 {
    let quotient = value / quantum;
    let trunc = quotient * quantum;
    let remainder = value % quantum;
    if remainder == 0 {
        return value;
    }
    let expand = trunc + if value < 0 { -quantum } else { quantum };
    match mode {
        TemporalRoundMode::Trunc => trunc,
        TemporalRoundMode::Expand => expand,
        TemporalRoundMode::Floor => {
            if value < 0 {
                expand
            } else {
                trunc
            }
        }
        TemporalRoundMode::Ceil => {
            if value < 0 {
                trunc
            } else {
                expand
            }
        }
        half_mode => {
            let twice = remainder.unsigned_abs() * 2;
            let quantum_abs = quantum as u128;
            if twice < quantum_abs {
                trunc
            } else if twice > quantum_abs {
                expand
            } else {
                match half_mode {
                    TemporalRoundMode::HalfCeil => {
                        if value < 0 {
                            trunc
                        } else {
                            expand
                        }
                    }
                    TemporalRoundMode::HalfFloor => {
                        if value < 0 {
                            expand
                        } else {
                            trunc
                        }
                    }
                    TemporalRoundMode::HalfTrunc => trunc,
                    TemporalRoundMode::HalfEven => {
                        if quotient % 2 == 0 {
                            trunc
                        } else {
                            expand
                        }
                    }
                    _ => expand,
                }
            }
        }
    }
}

fn option_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<Option<String>, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    if matches!(value, JsValue::Undefined) {
        Ok(None)
    } else {
        Ok(Some(vm.to_string_coerce(value, context)?))
    }
}

fn option_rounding_increment(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
) -> Result<u64, VmError> {
    let value = temporal_get_property(vm, context, object, "roundingIncrement")?;
    if matches!(value, JsValue::Undefined) {
        return Ok(1);
    }
    let number = vm.to_number(value, context)?;
    if !number.is_finite() || number < 1.0 {
        return Err(VmError::range("invalid Temporal rounding increment"));
    }
    Ok(number.trunc() as u64)
}

fn validate_rounding_increment(unit: &str, increment: u64) -> Result<(), VmError> {
    let maximum = match unit {
        "hour" => 24,
        "minute" => 24 * 60,
        "second" => 24 * 60 * 60,
        "millisecond" => 24 * 60 * 60 * 1_000,
        "microsecond" => 24 * 60 * 60 * 1_000_000,
        "nanosecond" => 24 * 60 * 60 * 1_000_000_000,
        _ => 1,
    };
    if increment == 0 || increment > maximum || maximum % increment != 0 {
        Err(VmError::range("invalid Temporal rounding increment"))
    } else {
        Ok(())
    }
}

fn instant_round_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<(String, u64, TemporalRoundMode), VmError> {
    if let JsValue::String(unit) = value {
        let unit = normalize_temporal_unit(unit)?;
        if matches!(unit.as_str(), "year" | "month" | "week" | "day") {
            return Err(VmError::range("day is not a valid Instant rounding unit"));
        }
        return Ok((unit, 1, TemporalRoundMode::HalfExpand));
    }
    let object = context.require_object(&value, "Temporal.Instant.prototype.round options")?;
    let increment = option_rounding_increment(vm, context, object)?;
    let mode = temporal_round_mode(
        option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
        TemporalRoundMode::HalfExpand,
    )?;
    let unit = option_string(vm, context, object, "smallestUnit")?
        .ok_or_else(|| VmError::range("smallestUnit is required"))?;
    let unit = normalize_temporal_unit(unit)?;
    if matches!(unit.as_str(), "year" | "month" | "week" | "day") {
        return Err(VmError::range("day is not a valid Instant rounding unit"));
    }
    validate_rounding_increment(&unit, increment)?;
    Ok((unit, increment, mode))
}

fn temporal_instant_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: i128,
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "Instant")?;
    let other = temporal_instant_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    let options = instant_difference_options(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let difference =
        sign * (instant_epoch_ns(context, other_object) - instant_epoch_ns(context, this_object));
    let quantum = temporal_unit_nanoseconds(&options.smallest_unit) * options.increment as i128;
    let rounded = round_i128(difference, quantum, options.mode);
    create_duration_from_nanoseconds(context, rounded, &options.largest_unit)
}

struct InstantDifferenceOptions {
    largest_unit: String,
    smallest_unit: String,
    increment: u64,
    mode: TemporalRoundMode,
}

fn instant_difference_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<InstantDifferenceOptions, VmError> {
    let object = if matches!(value, JsValue::Undefined) {
        None
    } else {
        Some(context.require_object(&value, "Temporal.Instant difference options")?)
    };
    let largest = match object {
        Some(object) => option_string(vm, context, object, "largestUnit")?,
        None => None,
    };
    let increment = match object {
        Some(object) => option_rounding_increment(vm, context, object)?,
        None => 1,
    };
    let mode = match object {
        Some(object) => temporal_round_mode(
            option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
            TemporalRoundMode::Trunc,
        )?,
        None => TemporalRoundMode::Trunc,
    };
    let smallest = match object {
        Some(object) => option_string(vm, context, object, "smallestUnit")?,
        None => None,
    };
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "nanosecond".into()))?;
    let largest_unit = normalize_temporal_unit(largest.unwrap_or_else(|| {
        if temporal_unit_nanoseconds(&smallest_unit) > NS_PER_SECOND_I128 {
            smallest_unit.clone()
        } else {
            "second".into()
        }
    }))?;
    if matches!(largest_unit.as_str(), "year" | "month" | "week" | "day")
        || matches!(smallest_unit.as_str(), "year" | "month" | "week" | "day")
    {
        return Err(VmError::range("day is not a valid Instant difference unit"));
    }
    if temporal_unit_nanoseconds(&largest_unit) < temporal_unit_nanoseconds(&smallest_unit) {
        return Err(VmError::range(
            "largestUnit must not be smaller than smallestUnit",
        ));
    }
    let maximum = match smallest_unit.as_str() {
        "hour" => 24,
        "minute" | "second" => 60,
        "millisecond" | "microsecond" | "nanosecond" => 1_000,
        _ => 1,
    };
    if increment >= maximum || maximum % increment != 0 {
        return Err(VmError::range("invalid Temporal rounding increment"));
    }
    Ok(InstantDifferenceOptions {
        largest_unit,
        smallest_unit,
        increment,
        mode,
    })
}

fn create_duration_from_nanoseconds(
    context: &mut NativeContext,
    value: i128,
    largest_unit: &str,
) -> Result<JsValue, VmError> {
    let sign = if value < 0 { -1.0 } else { 1.0 };
    let mut remainder = value.unsigned_abs();
    let mut values = DurationValues::default();
    let units = [
        ("year", 365 * NS_PER_DAY_I128),
        ("month", 30 * NS_PER_DAY_I128),
        ("week", 7 * NS_PER_DAY_I128),
        ("day", NS_PER_DAY_I128),
        ("hour", NS_PER_HOUR_I128),
        ("minute", NS_PER_MINUTE_I128),
        ("second", NS_PER_SECOND_I128),
        ("millisecond", NS_PER_MILLISECOND_I128),
        ("microsecond", 1_000),
        ("nanosecond", 1),
    ];
    let start = units
        .iter()
        .position(|(unit, _)| *unit == largest_unit)
        .unwrap_or(3);
    for (unit, size) in &units[start..] {
        let size = *size as u128;
        let part = (remainder / size) as f64 * sign;
        remainder %= size;
        match *unit {
            "year" => values.years = part,
            "month" => values.months = part,
            "week" => values.weeks = part,
            "day" => values.days = part,
            "hour" => values.hours = part,
            "minute" => values.minutes = part,
            "second" => values.seconds = part,
            "millisecond" => values.milliseconds = part,
            "microsecond" => values.microseconds = part,
            _ => values.nanoseconds = part,
        }
    }
    create_duration_with_default_prototype(context, values)
}

fn temporal_instant_until(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_instant_difference(vm, context, this_value, arguments, 1)
}

fn temporal_instant_since(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_instant_difference(vm, context, this_value, arguments, -1)
}

fn temporal_instant_to_zoned_date_time_iso(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Instant")?;
    let time_zone_value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if matches!(time_zone_value, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal.Instant.prototype.toZonedDateTimeISO requires a time zone",
        ));
    }
    let time_zone_id = vm.to_string_coerce(time_zone_value, context)?;
    let epoch_ns = instant_epoch_ns(context, object);
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        JsValue::BigInt(bigint::from_i128(epoch_ns)),
        epoch_ns as f64,
        time_zone_id,
        "iso8601".into(),
    )
}

fn install_temporal_plain_date(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "PlainDate",
        3,
        temporal_constructor_call_error,
        temporal_plain_date_construct,
        "Temporal.PlainDate",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_plain_date_from,
    )?;
    define_method(
        context,
        constructor_object,
        "compare",
        2,
        temporal_plain_date_compare,
    )?;
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_plain_date_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toJSON",
        0,
        temporal_plain_date_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_plain_date_to_string,
    )?;
    for (name, length, call) in [
        ("add", 1, temporal_plain_date_add as NativeCall),
        ("subtract", 1, temporal_plain_date_subtract as NativeCall),
        ("with", 1, temporal_plain_date_with as NativeCall),
        (
            "withCalendar",
            1,
            temporal_plain_date_with_calendar as NativeCall,
        ),
        ("equals", 1, temporal_plain_date_equals as NativeCall),
        ("until", 1, temporal_plain_date_until as NativeCall),
        ("since", 1, temporal_plain_date_since as NativeCall),
        (
            "toPlainDateTime",
            0,
            temporal_plain_date_to_plain_date_time as NativeCall,
        ),
        (
            "toPlainYearMonth",
            0,
            temporal_plain_date_to_plain_year_month as NativeCall,
        ),
        (
            "toPlainMonthDay",
            0,
            temporal_plain_date_to_plain_month_day as NativeCall,
        ),
        (
            "toZonedDateTime",
            1,
            temporal_plain_date_to_zoned_date_time as NativeCall,
        ),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [
        ("year", "get year", "year"),
        ("month", "get month", "month"),
        ("day", "get day", "day"),
        ("dayOfWeek", "get dayOfWeek", "dayOfWeek"),
        ("dayOfYear", "get dayOfYear", "dayOfYear"),
        ("weekOfYear", "get weekOfYear", "weekOfYear"),
        ("yearOfWeek", "get yearOfWeek", "yearOfWeek"),
        ("daysInWeek", "get daysInWeek", "daysInWeek"),
        ("daysInMonth", "get daysInMonth", "daysInMonth"),
        ("daysInYear", "get daysInYear", "daysInYear"),
        ("monthsInYear", "get monthsInYear", "monthsInYear"),
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "PlainDate", slot)?;
    }
    for (name, getter, slot) in [
        ("monthCode", "get monthCode", "monthCode"),
        ("calendarId", "get calendarId", "calendarId"),
    ] {
        define_temporal_string_slot_getter(context, prototype, name, getter, "PlainDate", slot)?;
    }
    define_temporal_bool_slot_getter(
        context,
        prototype,
        "inLeapYear",
        "get inLeapYear",
        "PlainDate",
        "inLeapYear",
    )?;
    define_temporal_undefined_getter(context, prototype, "era", "get era", "PlainDate")?;
    define_temporal_undefined_getter(context, prototype, "eraYear", "get eraYear", "PlainDate")?;
    Ok(())
}

fn create_plain_date(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
) -> Result<JsValue, VmError> {
    create_plain_date_with_calendar(context, prototype, year, month, day, "iso8601".into())
}

fn create_plain_date_with_calendar(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    create_temporal_object(
        context,
        prototype,
        "PlainDate",
        temporal_date_slots(year, month, day, calendar_id),
    )
}

fn temporal_plain_date_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.PlainDate prototype missing"))?;
    let year = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let month = vm.to_number(
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let day = vm.to_number(
        arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.get(3))?;
    validate_plain_date(year, month, day)?;
    create_plain_date_with_calendar(context, prototype, year, month, day, calendar_id)
}

fn validate_plain_date(year: f64, month: f64, day: f64) -> Result<(), VmError> {
    if !year.is_finite() || !month.is_finite() || !day.is_finite() {
        return Err(VmError::range("invalid Temporal.PlainDate"));
    }
    let year = year.trunc() as i32;
    let month = month.trunc() as u32;
    let day = day.trunc() as u32;
    if !(1..=12).contains(&month) || !(1..=month_day_count(year, month)).contains(&day) {
        Err(VmError::range("invalid Temporal.PlainDate"))
    } else {
        Ok(())
    }
}

fn temporal_constructor_prototype(
    context: &NativeContext,
    name: &str,
) -> Result<ObjectId, VmError> {
    context
        .get_global("Temporal")
        .and_then(|temporal| context.value_object(&temporal))
        .and_then(|temporal| context.get_own_property_descriptor(temporal, name))
        .and_then(|descriptor| descriptor.value_cloned())
        .and_then(|constructor| context.constructor_prototype(&constructor).ok().flatten())
        .ok_or_else(|| VmError::runtime(format!("Temporal.{name} prototype missing")))
}

fn parse_plain_date(text: &str) -> Option<(f64, f64, f64)> {
    parse_temporal_plain_date_string(text)
}

fn temporal_plain_date_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (year, month, day, calendar_id) = match item {
        JsValue::String(text) => parse_plain_date(&text)
            .map(|(year, month, day)| (year, month, day, "iso8601".into()))
            .ok_or_else(|| VmError::range("invalid Temporal.PlainDate string"))?,
        value => {
            let object = context.require_object(&value, "Temporal.PlainDate.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("PlainDate") {
                (
                    temporal_number_slot(context, object, "year"),
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "day"),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else if matches!(
                own_string(context, object, TEMPORAL_KIND).as_deref(),
                Some("PlainDateTime" | "ZonedDateTime")
            ) {
                (
                    temporal_number_slot(context, object, "year"),
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "day"),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else {
                let month = temporal_required_month_from_object(vm, context, object)?;
                (
                    temporal_required_object_number(vm, context, object, "year")?,
                    month,
                    temporal_required_object_number(vm, context, object, "day")?,
                    temporal_calendar_id_from_object(vm, context, object)?,
                )
            }
        }
    };
    validate_plain_date(year, month, day)?;
    create_plain_date_with_calendar(context, prototype, year, month, day, calendar_id)
}

fn plain_date_order_key(context: &NativeContext, object: ObjectId) -> i64 {
    let year = temporal_number_slot(context, object, "year") as i32;
    let month = temporal_number_slot(context, object, "month") as u32;
    let day = temporal_number_slot(context, object, "day") as u32;
    days_from_civil(year, month, day)
}

fn temporal_plain_date_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = temporal_plain_date_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let right = temporal_plain_date_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.get(1).cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let left = plain_date_order_key(context, context.value_object(&left).unwrap());
    let right = plain_date_order_key(context, context.value_object(&right).unwrap());
    Ok(JsValue::Number(if left < right {
        -1.0
    } else if left > right {
        1.0
    } else {
        0.0
    }))
}

fn temporal_plain_date_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    Ok(JsValue::String(format!(
        "{}-{}-{}",
        iso_year(temporal_number_slot(context, object, "year") as i32),
        two_digit(temporal_number_slot(context, object, "month") as u32),
        two_digit(temporal_number_slot(context, object, "day") as u32)
    )))
}

fn temporal_calendar_from_argument(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: Option<&JsValue>,
) -> Result<String, VmError> {
    match value.cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined => Ok("iso8601".into()),
        value => {
            let calendar = vm.to_string_coerce(value, context)?.to_ascii_lowercase();
            if calendar.is_empty() {
                Err(VmError::range("invalid Temporal calendar"))
            } else {
                Ok(calendar)
            }
        }
    }
}

fn temporal_date_replacement(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
    current: f64,
) -> Result<f64, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    if matches!(value, JsValue::Undefined) {
        Ok(current)
    } else {
        let number = vm.to_number(value, context)?.trunc();
        if number.is_finite() {
            Ok(number)
        } else {
            Err(VmError::range(format!(
                "Temporal {name} must be a finite number"
            )))
        }
    }
}

fn temporal_month_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    current: f64,
) -> Result<f64, VmError> {
    let month_value = temporal_get_property(vm, context, object, "month")?;
    let month_code_value = temporal_get_property(vm, context, object, "monthCode")?;
    let month_code = if matches!(month_code_value, JsValue::Undefined) {
        None
    } else {
        let value = vm.to_string_coerce(month_code_value, context)?;
        Some(parse_month_code(&value).ok_or_else(|| VmError::range("invalid Temporal monthCode"))?)
    };
    let month = if matches!(month_value, JsValue::Undefined) {
        None
    } else {
        let value = vm.to_number(month_value, context)?.trunc();
        if !value.is_finite() {
            return Err(VmError::range("Temporal month must be a finite number"));
        }
        Some(value)
    };
    match (month, month_code) {
        (Some(month), Some(month_code)) if month != month_code => {
            Err(VmError::range("Temporal month and monthCode must agree"))
        }
        (Some(month), _) => Ok(month),
        (_, Some(month_code)) => Ok(month_code),
        (None, None) => Ok(current),
    }
}

fn apply_duration_to_date(
    year: f64,
    month: f64,
    day: f64,
    duration: DurationValues,
    sign: f64,
    reject_overflow: bool,
) -> Result<(f64, f64, f64), VmError> {
    let year_i = year.trunc() as i32;
    let month_i = month.trunc() as i32;
    let month_delta = (sign * (duration.years * 12.0 + duration.months)).trunc() as i64;
    let month_index = (year_i as i64 * 12 + month_i as i64 - 1)
        .checked_add(month_delta)
        .ok_or_else(|| VmError::range("Temporal date is out of range"))?;
    let new_year = i32::try_from(month_index.div_euclid(12))
        .map_err(|_| VmError::range("Temporal date is out of range"))?;
    let new_month = month_index.rem_euclid(12) as u32 + 1;
    let requested_day = day.trunc() as u32;
    let maximum_day = month_day_count(new_year, new_month);
    if reject_overflow && requested_day > maximum_day {
        return Err(VmError::range("Temporal date overflows the target month"));
    }
    let clamped_day = requested_day.min(maximum_day);
    let extra_days = (sign * (duration.weeks * 7.0 + duration.days)).trunc() as i64;
    let day_number = days_from_civil(new_year, new_month, clamped_day)
        .checked_add(extra_days)
        .ok_or_else(|| VmError::range("Temporal date is out of range"))?;
    let (year, month, day) = civil_from_days(day_number);
    Ok((year as f64, month as f64, day as f64))
}

fn temporal_overflow_reject(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<bool, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(false);
    }
    let object = context.require_object(&value, "Temporal overflow options")?;
    let value = temporal_get_property(vm, context, object, "overflow")?;
    if matches!(value, JsValue::Undefined) {
        return Ok(false);
    }
    match vm.to_string_coerce(value, context)?.as_str() {
        "constrain" => Ok(false),
        "reject" => Ok(true),
        _ => Err(VmError::range("invalid Temporal overflow option")),
    }
}

fn reject_temporal_with_metadata(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
) -> Result<(), VmError> {
    if own_string(context, object, TEMPORAL_KIND).is_some() {
        return Err(VmError::type_error(
            "Temporal with() property bag must not be a Temporal object",
        ));
    }
    let calendar = temporal_get_property(vm, context, object, "calendar")?;
    if !matches!(calendar, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal with() property bag must not include calendar",
        ));
    }
    let time_zone = temporal_get_property(vm, context, object, "timeZone")?;
    if !matches!(time_zone, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal with() property bag must not include timeZone",
        ));
    }
    Ok(())
}

fn require_temporal_with_field(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    fields: &[&str],
) -> Result<(), VmError> {
    for field in fields {
        if !matches!(
            temporal_get_property(vm, context, object, field)?,
            JsValue::Undefined
        ) {
            return Ok(());
        }
    }
    Err(VmError::type_error(
        "Temporal with() property bag must contain a supported field",
    ))
}

struct DateDifferenceOptions {
    largest_unit: String,
    smallest_unit: String,
    increment: u64,
    mode: TemporalRoundMode,
}

fn date_difference_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<DateDifferenceOptions, VmError> {
    let object = if matches!(value, JsValue::Undefined) {
        None
    } else {
        Some(context.require_object(&value, "Temporal.PlainDate difference options")?)
    };
    let largest = match object {
        Some(object) => option_string(vm, context, object, "largestUnit")?,
        None => None,
    };
    let increment = match object {
        Some(object) => option_rounding_increment(vm, context, object)?,
        None => 1,
    };
    let mode = match object {
        Some(object) => temporal_round_mode(
            option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
            TemporalRoundMode::Trunc,
        )?,
        None => TemporalRoundMode::Trunc,
    };
    let smallest = match object {
        Some(object) => option_string(vm, context, object, "smallestUnit")?,
        None => None,
    };
    let largest_unit = normalize_temporal_unit(largest.unwrap_or_else(|| "day".into()))?;
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "day".into()))?;
    let valid = |unit: &str| matches!(unit, "year" | "month" | "week" | "day");
    if !valid(&largest_unit) || !valid(&smallest_unit) {
        return Err(VmError::range("invalid PlainDate difference unit"));
    }
    if temporal_unit_nanoseconds(&largest_unit) < temporal_unit_nanoseconds(&smallest_unit) {
        return Err(VmError::range(
            "largestUnit must not be smaller than smallestUnit",
        ));
    }
    Ok(DateDifferenceOptions {
        largest_unit,
        smallest_unit,
        increment,
        mode,
    })
}

fn date_parts(context: &NativeContext, object: ObjectId) -> (i32, u32, u32) {
    (
        temporal_number_slot(context, object, "year") as i32,
        temporal_number_slot(context, object, "month") as u32,
        temporal_number_slot(context, object, "day") as u32,
    )
}

fn add_iso_months(year: i32, month: u32, day: u32, delta: i64) -> (i32, u32, u32) {
    let index = year as i64 * 12 + month as i64 - 1 + delta;
    let result_year = index.div_euclid(12) as i32;
    let result_month = index.rem_euclid(12) as u32 + 1;
    let result_day = day.min(month_day_count(result_year, result_month));
    (result_year, result_month, result_day)
}

fn iso_date_difference_values(
    context: &NativeContext,
    start: ObjectId,
    end: ObjectId,
    largest_unit: &str,
) -> DurationValues {
    let (start_year, start_month, start_day) = date_parts(context, start);
    let (end_year, end_month, end_day) = date_parts(context, end);
    let start_number = days_from_civil(start_year, start_month, start_day);
    let end_number = days_from_civil(end_year, end_month, end_day);
    let total_days = end_number - start_number;
    if largest_unit == "day" {
        return DurationValues {
            days: total_days as f64,
            ..DurationValues::default()
        };
    }
    if largest_unit == "week" {
        return DurationValues {
            weeks: (total_days / 7) as f64,
            days: (total_days % 7) as f64,
            ..DurationValues::default()
        };
    }
    let mut months =
        (end_year as i64 * 12 + end_month as i64) - (start_year as i64 * 12 + start_month as i64);
    let direction = total_days.signum();
    let mut candidate = add_iso_months(start_year, start_month, start_day, months);
    let mut candidate_number = days_from_civil(candidate.0, candidate.1, candidate.2);
    if direction > 0 && candidate_number > end_number {
        months -= 1;
        candidate = add_iso_months(start_year, start_month, start_day, months);
        candidate_number = days_from_civil(candidate.0, candidate.1, candidate.2);
    } else if direction < 0 && candidate_number < end_number {
        months += 1;
        candidate = add_iso_months(start_year, start_month, start_day, months);
        candidate_number = days_from_civil(candidate.0, candidate.1, candidate.2);
    }
    let days = end_number - candidate_number;
    if largest_unit == "year" {
        DurationValues {
            years: (months / 12) as f64,
            months: (months % 12) as f64,
            days: days as f64,
            ..DurationValues::default()
        }
    } else {
        DurationValues {
            months: months as f64,
            days: days as f64,
            ..DurationValues::default()
        }
    }
}

fn temporal_plain_date_additive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: f64,
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let duration = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (mut year, mut month, mut day) = apply_duration_to_date(
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        duration,
        sign,
        reject_overflow,
    )?;
    let time_days = plain_time_from_nanoseconds(sign * duration_time_nanoseconds(duration)).0;
    if time_days != 0 {
        let day_number = days_from_civil(year as i32, month as u32, day as u32)
            .checked_add(time_days)
            .filter(|value| value.unsigned_abs() <= 100_000_000)
            .ok_or_else(|| VmError::range("Temporal.PlainDate is out of range"))?;
        (year, month, day) = {
            let fields = civil_from_days(day_number);
            (fields.0 as f64, fields.1 as f64, fields.2 as f64)
        };
    }
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date_with_calendar(
        context,
        prototype,
        year,
        month,
        day,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_additive(vm, context, this_value, arguments, 1.0)
}

fn temporal_plain_date_subtract(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_additive(vm, context, this_value, arguments, -1.0)
}

fn temporal_plain_date_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let replacement = context.require_object(
        &arguments.first().cloned().unwrap_or(JsValue::Undefined),
        "Temporal.PlainDate.prototype.with",
    )?;
    reject_temporal_with_metadata(vm, context, replacement)?;
    let year = temporal_date_replacement(
        vm,
        context,
        replacement,
        "year",
        temporal_number_slot(context, this_object, "year"),
    )?;
    let month = temporal_month_from_object(
        vm,
        context,
        replacement,
        temporal_number_slot(context, this_object, "month"),
    )?;
    let day = temporal_date_replacement(
        vm,
        context,
        replacement,
        "day",
        temporal_number_slot(context, this_object, "day"),
    )?;
    require_temporal_with_field(
        vm,
        context,
        replacement,
        &["day", "month", "monthCode", "year"],
    )?;
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let month = if reject_overflow {
        month
    } else {
        month.clamp(1.0, 12.0)
    };
    let day = if reject_overflow {
        day
    } else {
        day.clamp(1.0, month_day_count(year as i32, month as u32) as f64)
    };
    validate_plain_date(year, month, day)?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date_with_calendar(
        context,
        prototype,
        year,
        month,
        day,
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_with_calendar(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.first())?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date_with_calendar(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        calendar_id,
    )
}

fn temporal_plain_date_equals(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let other = temporal_plain_date_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    Ok(JsValue::Boolean(
        temporal_number_slot(context, this_object, "year")
            == temporal_number_slot(context, other_object, "year")
            && temporal_number_slot(context, this_object, "month")
                == temporal_number_slot(context, other_object, "month")
            && temporal_number_slot(context, this_object, "day")
                == temporal_number_slot(context, other_object, "day")
            && own_string(context, this_object, "calendarId")
                == own_string(context, other_object, "calendarId"),
    ))
}

fn temporal_plain_date_until(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_difference(vm, context, this_value, arguments, false)
}

fn temporal_plain_date_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    reverse: bool,
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let other = temporal_plain_date_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    if own_string(context, this_object, "calendarId")
        != own_string(context, other_object, "calendarId")
    {
        return Err(VmError::range("Temporal calendars must match"));
    }
    let options = date_difference_options(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (start, end) = if reverse {
        (other_object, this_object)
    } else {
        (this_object, other_object)
    };
    let mut values = iso_date_difference_values(context, start, end, &options.largest_unit);
    if options.smallest_unit == "day" && options.increment > 1 {
        let total_days = plain_date_order_key(context, end) - plain_date_order_key(context, start);
        let rounded_days =
            round_signed_i128(total_days as i128, options.increment as i128, options.mode) as f64;
        values = if options.largest_unit == "week" {
            DurationValues {
                weeks: (rounded_days / 7.0).trunc(),
                days: rounded_days % 7.0,
                ..DurationValues::default()
            }
        } else if options.largest_unit == "day" {
            DurationValues {
                days: rounded_days,
                ..DurationValues::default()
            }
        } else {
            values
        };
    }
    create_duration_with_default_prototype(context, values)
}

fn temporal_plain_date_since(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_difference(vm, context, this_value, arguments, true)
}

fn temporal_plain_date_to_plain_date_time(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let time = plain_time_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        time,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_to_plain_year_month(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let prototype = temporal_constructor_prototype(context, "PlainYearMonth")?;
    create_plain_year_month(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_to_plain_month_day(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let prototype = temporal_constructor_prototype(context, "PlainMonthDay")?;
    create_plain_month_day(
        context,
        prototype,
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        temporal_number_slot(context, object, "year"),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn epoch_nanoseconds_i128_from_plain_parts(
    year: f64,
    month: f64,
    day: f64,
    time: PlainTimeValues,
) -> i128 {
    let days = days_from_civil(year as i32, month as u32, day as u32) as i128;
    let seconds =
        days * 86_400 + time.hour as i128 * 3_600 + time.minute as i128 * 60 + time.second as i128;
    seconds * NS_PER_SECOND_I128
        + time.millisecond as i128 * 1_000_000
        + time.microsecond as i128 * 1_000
        + time.nanosecond as i128
}

fn temporal_plain_date_to_zoned_date_time(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (time_zone_value, time) = if let Some(item_object) = context.value_object(&item) {
        let time_zone = temporal_get_property(vm, context, item_object, "timeZone")?;
        let plain_time = temporal_get_property(vm, context, item_object, "plainTime")?;
        (
            if matches!(time_zone, JsValue::Undefined) {
                item.clone()
            } else {
                time_zone
            },
            plain_time_values_from_value(vm, context, plain_time)?,
        )
    } else {
        (item, PlainTimeValues::default())
    };
    if matches!(time_zone_value, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal.PlainDate.prototype.toZonedDateTime requires a time zone",
        ));
    }
    let time_zone_id = vm.to_string_coerce(time_zone_value, context)?;
    let exact_epoch_nanoseconds = epoch_nanoseconds_i128_from_plain_parts(
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        time,
    ) - parse_time_zone_offset_ns(&time_zone_id).unwrap_or(0);
    let epoch_nanoseconds = exact_epoch_nanoseconds as f64;
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        JsValue::BigInt(bigint::from_i128(exact_epoch_nanoseconds)),
        epoch_nanoseconds,
        time_zone_id,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn install_temporal_plain_time(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "PlainTime",
        0,
        temporal_constructor_call_error,
        temporal_plain_time_construct,
        "Temporal.PlainTime",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_plain_time_from,
    )?;
    define_method(
        context,
        constructor_object,
        "compare",
        2,
        temporal_plain_time_compare,
    )?;
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_plain_time_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toJSON",
        0,
        temporal_plain_time_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_plain_time_to_string,
    )?;
    for (name, length, call) in [
        ("add", 1, temporal_plain_time_add as NativeCall),
        ("subtract", 1, temporal_plain_time_subtract as NativeCall),
        ("with", 1, temporal_plain_time_with as NativeCall),
        ("equals", 1, temporal_plain_time_equals as NativeCall),
        ("round", 1, temporal_plain_time_round as NativeCall),
        ("until", 1, temporal_plain_time_until as NativeCall),
        ("since", 1, temporal_plain_time_since as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [
        ("hour", "get hour", "hour"),
        ("minute", "get minute", "minute"),
        ("second", "get second", "second"),
        ("millisecond", "get millisecond", "millisecond"),
        ("microsecond", "get microsecond", "microsecond"),
        ("nanosecond", "get nanosecond", "nanosecond"),
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "PlainTime", slot)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct PlainTimeValues {
    hour: f64,
    minute: f64,
    second: f64,
    millisecond: f64,
    microsecond: f64,
    nanosecond: f64,
}

impl Default for PlainTimeValues {
    fn default() -> Self {
        Self {
            hour: 0.0,
            minute: 0.0,
            second: 0.0,
            millisecond: 0.0,
            microsecond: 0.0,
            nanosecond: 0.0,
        }
    }
}

fn validate_plain_time(values: PlainTimeValues) -> Result<(), VmError> {
    let ranges = [
        (values.hour, 0.0, 23.0),
        (values.minute, 0.0, 59.0),
        (values.second, 0.0, 59.0),
        (values.millisecond, 0.0, 999.0),
        (values.microsecond, 0.0, 999.0),
        (values.nanosecond, 0.0, 999.0),
    ];
    if ranges
        .into_iter()
        .all(|(value, min, max)| value.is_finite() && value.trunc() >= min && value.trunc() <= max)
    {
        Ok(())
    } else {
        Err(VmError::range("invalid Temporal.PlainTime"))
    }
}

fn plain_time_from_args(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<PlainTimeValues, VmError> {
    Ok(PlainTimeValues {
        hour: number_or_default(vm, context, arguments, 0, 0.0)?,
        minute: number_or_default(vm, context, arguments, 1, 0.0)?,
        second: number_or_default(vm, context, arguments, 2, 0.0)?,
        millisecond: number_or_default(vm, context, arguments, 3, 0.0)?,
        microsecond: number_or_default(vm, context, arguments, 4, 0.0)?,
        nanosecond: number_or_default(vm, context, arguments, 5, 0.0)?,
    })
}

fn create_plain_time(
    context: &mut NativeContext,
    prototype: ObjectId,
    values: PlainTimeValues,
) -> Result<JsValue, VmError> {
    let values = PlainTimeValues {
        hour: clean_zero(values.hour),
        minute: clean_zero(values.minute),
        second: clean_zero(values.second),
        millisecond: clean_zero(values.millisecond),
        microsecond: clean_zero(values.microsecond),
        nanosecond: clean_zero(values.nanosecond),
    };
    create_temporal_object(
        context,
        prototype,
        "PlainTime",
        [
            ("hour", JsValue::Number(values.hour.trunc())),
            ("minute", JsValue::Number(values.minute.trunc())),
            ("second", JsValue::Number(values.second.trunc())),
            ("millisecond", JsValue::Number(values.millisecond.trunc())),
            ("microsecond", JsValue::Number(values.microsecond.trunc())),
            ("nanosecond", JsValue::Number(values.nanosecond.trunc())),
        ],
    )
}

fn temporal_plain_time_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.PlainTime prototype missing"))?;
    let values = plain_time_from_args(vm, context, arguments)?;
    validate_plain_time(values)?;
    create_plain_time(context, prototype, values)
}

fn parse_plain_time(text: &str) -> Option<PlainTimeValues> {
    let body = validate_iso_calendar_annotations(text.trim())?;
    let time = body
        .rfind('T')
        .or_else(|| body.rfind('t'))
        .map(|index| &body[index + 1..])
        .unwrap_or_else(|| {
            body.strip_prefix('T')
                .or_else(|| body.strip_prefix('t'))
                .unwrap_or(body)
        });
    if time.is_empty()
        || time.contains('\u{2212}')
        || time.ends_with(['Z', 'z'])
        || time.contains("Z[")
        || time.contains("z[")
    {
        return None;
    }
    let offset_start = time.rfind('+').or_else(|| {
        time.get(1..)
            .and_then(|rest| rest.rfind('-').map(|index| index + 1))
    });
    let time = if let Some(offset_start) = offset_start {
        parse_time_zone_offset_ns(&time[offset_start..])?;
        &time[..offset_start]
    } else {
        time
    };
    let normalized = time.replace(',', ".");
    let (head, fraction_text) = normalized
        .split_once('.')
        .map_or((normalized.as_str(), ""), |(head, fraction)| {
            (head, fraction)
        });
    let colon_count = head.matches(':').count();
    let compact = head.replace(':', "");
    if colon_count > 0 && !(colon_count == 1 || colon_count == 2) {
        return None;
    }
    if (colon_count > 0 && compact.len() != (colon_count + 1) * 2)
        || (colon_count == 0 && !matches!(compact.len(), 2 | 4 | 6))
    {
        return None;
    }
    if !compact.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    };
    let hour = parse_fixed_digits(&compact[..2], 2)? as f64;
    let minute = if compact.len() >= 4 {
        parse_fixed_digits(&compact[2..4], 2)? as f64
    } else {
        0.0
    };
    let mut second = if compact.len() == 6 {
        parse_fixed_digits(&compact[4..6], 2)? as f64
    } else {
        0.0
    };
    if second == 60.0 {
        second = 59.0;
    }
    let fraction = if fraction_text.is_empty() {
        0
    } else {
        parse_fraction_to_ns(fraction_text)? as u32
    };
    Some(PlainTimeValues {
        hour,
        minute,
        second,
        millisecond: (fraction / 1_000_000) as f64,
        microsecond: ((fraction / 1_000) % 1_000) as f64,
        nanosecond: (fraction % 1_000) as f64,
    })
}

fn temporal_plain_time_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let values = match item {
        JsValue::String(text) => {
            parse_plain_time(&text).ok_or_else(|| VmError::range("invalid Temporal.PlainTime"))?
        }
        value => {
            let object = context.require_object(&value, "Temporal.PlainTime.from")?;
            PlainTimeValues {
                hour: temporal_object_number(vm, context, object, "hour")?,
                minute: temporal_object_number(vm, context, object, "minute")?,
                second: temporal_object_number(vm, context, object, "second")?,
                millisecond: temporal_object_number(vm, context, object, "millisecond")?,
                microsecond: temporal_object_number(vm, context, object, "microsecond")?,
                nanosecond: temporal_object_number(vm, context, object, "nanosecond")?,
            }
        }
    };
    validate_plain_time(values)?;
    create_plain_time(context, prototype, values)
}

fn plain_time_values_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<PlainTimeValues, VmError> {
    match value {
        JsValue::Undefined => Ok(PlainTimeValues::default()),
        JsValue::String(text) => {
            parse_plain_time(&text).ok_or_else(|| VmError::range("invalid Temporal.PlainTime"))
        }
        value => {
            let object = context.require_object(&value, "Temporal.PlainTime value")?;
            Ok(PlainTimeValues {
                hour: temporal_object_number(vm, context, object, "hour")?,
                minute: temporal_object_number(vm, context, object, "minute")?,
                second: temporal_object_number(vm, context, object, "second")?,
                millisecond: temporal_object_number(vm, context, object, "millisecond")?,
                microsecond: temporal_object_number(vm, context, object, "microsecond")?,
                nanosecond: temporal_object_number(vm, context, object, "nanosecond")?,
            })
        }
    }
}

fn temporal_plain_time_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = temporal_plain_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let right = temporal_plain_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.get(1).cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let left = plain_time_values_from_temporal(context, context.value_object(&left).unwrap());
    let right = plain_time_values_from_temporal(context, context.value_object(&right).unwrap());
    Ok(ordering_number(
        time_nanoseconds(left),
        time_nanoseconds(right),
    ))
}

fn format_plain_time_precision(
    values: PlainTimeValues,
    precision: Option<usize>,
    minute_only: bool,
) -> String {
    if minute_only {
        return format!(
            "{}:{}",
            two_digit(values.hour as u32),
            two_digit(values.minute as u32)
        );
    }
    let mut text = format!(
        "{}:{}:{}",
        two_digit(values.hour as u32),
        two_digit(values.minute as u32),
        two_digit(values.second as u32)
    );
    let fraction = values.millisecond as u32 * 1_000_000
        + values.microsecond as u32 * 1_000
        + values.nanosecond as u32;
    if fraction != 0 || precision.unwrap_or(0) != 0 {
        let mut fraction_text = format!("{fraction:09}");
        if let Some(precision) = precision {
            fraction_text.truncate(precision);
        } else {
            while fraction_text.ends_with('0') {
                fraction_text.pop();
            }
        }
        if fraction_text.is_empty() {
            return text;
        }
        text.push('.');
        text.push_str(&fraction_text);
    }
    text
}

fn temporal_plain_time_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainTime")?;
    let options = temporal_string_options(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let total = plain_time_nanoseconds_i128(plain_time_values_from_temporal(context, object));
    let rounded = round_i128(total, options.quantum, options.mode).rem_euclid(NS_PER_DAY_I128);
    Ok(JsValue::String(format_plain_time_precision(
        plain_time_from_nanoseconds_i128(rounded),
        options.precision,
        options.minute_only,
    )))
}

struct TemporalStringOptions {
    quantum: i128,
    precision: Option<usize>,
    minute_only: bool,
    mode: TemporalRoundMode,
}

fn temporal_string_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<TemporalStringOptions, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(TemporalStringOptions {
            quantum: 1,
            precision: None,
            minute_only: false,
            mode: TemporalRoundMode::Trunc,
        });
    }
    let object = context.require_object(&value, "Temporal toString options")?;
    let fractional_value = temporal_get_property(vm, context, object, "fractionalSecondDigits")?;
    let fractional_digits = if matches!(fractional_value, JsValue::Undefined) {
        None
    } else {
        let text = vm.to_string_coerce(fractional_value, context)?;
        if text == "auto" {
            None
        } else {
            let number = text
                .parse::<f64>()
                .map_err(|_| VmError::range("invalid fractionalSecondDigits"))?;
            if !number.is_finite() || !(0.0..=9.0).contains(&number) {
                return Err(VmError::range("invalid fractionalSecondDigits"));
            }
            Some(number.floor() as usize)
        }
    };
    let mode = temporal_round_mode(
        option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
        TemporalRoundMode::Trunc,
    )?;
    let smallest = option_string(vm, context, object, "smallestUnit")?;
    let (precision, minute_only) = match smallest.as_deref() {
        None => (fractional_digits, false),
        Some("minute" | "minutes") => (Some(0), true),
        Some("second" | "seconds") => (Some(0), false),
        Some("millisecond" | "milliseconds") => (Some(3), false),
        Some("microsecond" | "microseconds") => (Some(6), false),
        Some("nanosecond" | "nanoseconds") => (Some(9), false),
        Some(_) => return Err(VmError::range("invalid Temporal string smallestUnit")),
    };
    let quantum = if minute_only {
        NS_PER_MINUTE_I128
    } else {
        match precision {
            None | Some(9) => 1,
            Some(digits) => 10_i128.pow((9 - digits) as u32),
        }
    };
    Ok(TemporalStringOptions {
        quantum,
        precision,
        minute_only,
        mode,
    })
}

fn plain_time_values_from_temporal(context: &NativeContext, object: ObjectId) -> PlainTimeValues {
    PlainTimeValues {
        hour: temporal_number_slot(context, object, "hour"),
        minute: temporal_number_slot(context, object, "minute"),
        second: temporal_number_slot(context, object, "second"),
        millisecond: temporal_number_slot(context, object, "millisecond"),
        microsecond: temporal_number_slot(context, object, "microsecond"),
        nanosecond: temporal_number_slot(context, object, "nanosecond"),
    }
}

fn create_plain_time_from_total_nanoseconds(
    context: &mut NativeContext,
    prototype: ObjectId,
    total: f64,
) -> Result<JsValue, VmError> {
    let (_, time) = plain_time_from_nanoseconds(total.rem_euclid(NS_PER_DAY));
    create_plain_time(context, prototype, time)
}

fn temporal_plain_time_additive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: f64,
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainTime")?;
    let duration = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let total = time_nanoseconds(plain_time_values_from_temporal(context, object))
        + sign * duration_time_nanoseconds(duration);
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    create_plain_time_from_total_nanoseconds(context, prototype, total)
}

fn temporal_plain_time_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_time_additive(vm, context, this_value, arguments, 1.0)
}

fn temporal_plain_time_subtract(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_time_additive(vm, context, this_value, arguments, -1.0)
}

fn temporal_plain_time_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainTime")?;
    let replacement = context.require_object(
        &arguments.first().cloned().unwrap_or(JsValue::Undefined),
        "Temporal.PlainTime.prototype.with",
    )?;
    reject_temporal_with_metadata(vm, context, replacement)?;
    let mut time = PlainTimeValues {
        hour: temporal_date_replacement(
            vm,
            context,
            replacement,
            "hour",
            temporal_number_slot(context, this_object, "hour"),
        )?,
        minute: temporal_date_replacement(
            vm,
            context,
            replacement,
            "minute",
            temporal_number_slot(context, this_object, "minute"),
        )?,
        second: temporal_date_replacement(
            vm,
            context,
            replacement,
            "second",
            temporal_number_slot(context, this_object, "second"),
        )?,
        millisecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "millisecond",
            temporal_number_slot(context, this_object, "millisecond"),
        )?,
        microsecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "microsecond",
            temporal_number_slot(context, this_object, "microsecond"),
        )?,
        nanosecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "nanosecond",
            temporal_number_slot(context, this_object, "nanosecond"),
        )?,
    };
    require_temporal_with_field(
        vm,
        context,
        replacement,
        &[
            "hour",
            "microsecond",
            "millisecond",
            "minute",
            "nanosecond",
            "second",
        ],
    )?;
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    if !reject_overflow {
        time.hour = time.hour.clamp(0.0, 23.0);
        time.minute = time.minute.clamp(0.0, 59.0);
        time.second = time.second.clamp(0.0, 59.0);
        time.millisecond = time.millisecond.clamp(0.0, 999.0);
        time.microsecond = time.microsecond.clamp(0.0, 999.0);
        time.nanosecond = time.nanosecond.clamp(0.0, 999.0);
    }
    validate_plain_time(time)?;
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    create_plain_time(context, prototype, time)
}

fn temporal_plain_time_equals(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainTime")?;
    let other = temporal_plain_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    Ok(JsValue::Boolean(
        time_nanoseconds(plain_time_values_from_temporal(context, this_object))
            == time_nanoseconds(plain_time_values_from_temporal(context, other_object)),
    ))
}

fn temporal_plain_time_round(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainTime")?;
    let (unit, increment, mode) = instant_round_options(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let quantum = temporal_unit_nanoseconds(&unit) * increment as i128;
    let total = plain_time_nanoseconds_i128(plain_time_values_from_temporal(context, object));
    let rounded = round_i128(total, quantum, mode).rem_euclid(NS_PER_DAY_I128);
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    create_plain_time(
        context,
        prototype,
        plain_time_from_nanoseconds_i128(rounded),
    )
}

fn temporal_plain_time_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: i128,
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainTime")?;
    let other = temporal_plain_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    let options = plain_date_time_difference_options(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        "hour",
        false,
    )?;
    let delta = sign
        * (plain_time_nanoseconds_i128(plain_time_values_from_temporal(context, other_object))
            - plain_time_nanoseconds_i128(plain_time_values_from_temporal(context, this_object)));
    let quantum = temporal_unit_nanoseconds(&options.smallest_unit) * options.increment as i128;
    let rounded = round_signed_i128(delta, quantum, options.mode);
    create_duration_from_nanoseconds(context, rounded, &options.largest_unit)
}

fn temporal_plain_time_until(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_time_difference(vm, context, this_value, arguments, 1)
}

fn temporal_plain_time_since(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_time_difference(vm, context, this_value, arguments, -1)
}

fn install_temporal_plain_date_time(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "PlainDateTime",
        3,
        temporal_constructor_call_error,
        temporal_plain_date_time_construct,
        "Temporal.PlainDateTime",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_plain_date_time_from,
    )?;
    define_method(
        context,
        constructor_object,
        "compare",
        2,
        temporal_plain_date_time_compare,
    )?;
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_plain_date_time_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toJSON",
        0,
        temporal_plain_date_time_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_plain_date_time_to_string,
    )?;
    for (name, length, call) in [
        ("add", 1, temporal_plain_date_time_add as NativeCall),
        (
            "subtract",
            1,
            temporal_plain_date_time_subtract as NativeCall,
        ),
        ("with", 1, temporal_plain_date_time_with as NativeCall),
        (
            "withCalendar",
            1,
            temporal_plain_date_time_with_calendar as NativeCall,
        ),
        (
            "withPlainTime",
            0,
            temporal_plain_date_time_with_plain_time as NativeCall,
        ),
        ("equals", 1, temporal_plain_date_time_equals as NativeCall),
        ("round", 1, temporal_plain_date_time_round as NativeCall),
        ("until", 1, temporal_plain_date_time_until as NativeCall),
        ("since", 1, temporal_plain_date_time_since as NativeCall),
        (
            "toPlainDate",
            0,
            temporal_plain_date_time_to_plain_date as NativeCall,
        ),
        (
            "toPlainTime",
            0,
            temporal_plain_date_time_to_plain_time as NativeCall,
        ),
        (
            "toZonedDateTime",
            1,
            temporal_plain_date_time_to_zoned_date_time as NativeCall,
        ),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [
        ("year", "get year", "year"),
        ("month", "get month", "month"),
        ("day", "get day", "day"),
        ("hour", "get hour", "hour"),
        ("minute", "get minute", "minute"),
        ("second", "get second", "second"),
        ("millisecond", "get millisecond", "millisecond"),
        ("microsecond", "get microsecond", "microsecond"),
        ("nanosecond", "get nanosecond", "nanosecond"),
        ("dayOfWeek", "get dayOfWeek", "dayOfWeek"),
        ("dayOfYear", "get dayOfYear", "dayOfYear"),
        ("weekOfYear", "get weekOfYear", "weekOfYear"),
        ("yearOfWeek", "get yearOfWeek", "yearOfWeek"),
        ("daysInWeek", "get daysInWeek", "daysInWeek"),
        ("daysInMonth", "get daysInMonth", "daysInMonth"),
        ("daysInYear", "get daysInYear", "daysInYear"),
        ("monthsInYear", "get monthsInYear", "monthsInYear"),
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "PlainDateTime", slot)?;
    }
    for (name, getter, slot) in [
        ("monthCode", "get monthCode", "monthCode"),
        ("calendarId", "get calendarId", "calendarId"),
    ] {
        define_temporal_string_slot_getter(
            context,
            prototype,
            name,
            getter,
            "PlainDateTime",
            slot,
        )?;
    }
    define_temporal_bool_slot_getter(
        context,
        prototype,
        "inLeapYear",
        "get inLeapYear",
        "PlainDateTime",
        "inLeapYear",
    )?;
    define_temporal_undefined_getter(context, prototype, "era", "get era", "PlainDateTime")?;
    define_temporal_undefined_getter(
        context,
        prototype,
        "eraYear",
        "get eraYear",
        "PlainDateTime",
    )?;
    Ok(())
}

fn temporal_plain_date_time_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.PlainDateTime prototype missing"))?;
    let year = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let month = vm.to_number(
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let day = vm.to_number(
        arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let time = plain_time_from_args(vm, context, arguments.get(3..).unwrap_or(&[]))?;
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.get(9))?;
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    create_plain_date_time_with_calendar(context, prototype, year, month, day, time, calendar_id)
}

fn create_plain_date_time(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
    time: PlainTimeValues,
) -> Result<JsValue, VmError> {
    create_plain_date_time_with_calendar(
        context,
        prototype,
        year,
        month,
        day,
        time,
        "iso8601".into(),
    )
}

fn create_plain_date_time_with_calendar(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
    time: PlainTimeValues,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    let mut slots = temporal_date_slots(year, month, day, calendar_id);
    slots.extend([
        ("hour", JsValue::Number(time.hour.trunc())),
        ("minute", JsValue::Number(time.minute.trunc())),
        ("second", JsValue::Number(time.second.trunc())),
        ("millisecond", JsValue::Number(time.millisecond.trunc())),
        ("microsecond", JsValue::Number(time.microsecond.trunc())),
        ("nanosecond", JsValue::Number(time.nanosecond.trunc())),
    ]);
    create_temporal_object(context, prototype, "PlainDateTime", slots)
}

fn temporal_plain_date_time_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (year, month, day, time, calendar_id) = match item {
        JsValue::String(text) => {
            let (year, month, day) = parse_plain_date(&text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainDateTime"))?;
            let time = if text.contains('T') {
                parse_plain_time(text.split_once('T').map(|(_, time)| time).unwrap_or(""))
                    .ok_or_else(|| VmError::range("invalid Temporal.PlainDateTime"))?
            } else {
                PlainTimeValues::default()
            };
            (year, month, day, time, "iso8601".into())
        }
        value => {
            let object = context.require_object(&value, "Temporal.PlainDateTime.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("PlainDateTime") {
                (
                    temporal_number_slot(context, object, "year"),
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "day"),
                    PlainTimeValues {
                        hour: temporal_number_slot(context, object, "hour"),
                        minute: temporal_number_slot(context, object, "minute"),
                        second: temporal_number_slot(context, object, "second"),
                        millisecond: temporal_number_slot(context, object, "millisecond"),
                        microsecond: temporal_number_slot(context, object, "microsecond"),
                        nanosecond: temporal_number_slot(context, object, "nanosecond"),
                    },
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("PlainDate") {
                (
                    temporal_number_slot(context, object, "year"),
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "day"),
                    PlainTimeValues::default(),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else {
                let month = temporal_required_month_from_object(vm, context, object)?;
                (
                    temporal_required_object_number(vm, context, object, "year")?,
                    month,
                    temporal_required_object_number(vm, context, object, "day")?,
                    PlainTimeValues {
                        hour: temporal_object_number(vm, context, object, "hour")?,
                        minute: temporal_object_number(vm, context, object, "minute")?,
                        second: temporal_object_number(vm, context, object, "second")?,
                        millisecond: temporal_object_number(vm, context, object, "millisecond")?,
                        microsecond: temporal_object_number(vm, context, object, "microsecond")?,
                        nanosecond: temporal_object_number(vm, context, object, "nanosecond")?,
                    },
                    temporal_calendar_id_from_object(vm, context, object)?,
                )
            }
        }
    };
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    create_plain_date_time_with_calendar(context, prototype, year, month, day, time, calendar_id)
}

fn temporal_plain_date_time_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let option_value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let calendar_name = temporal_calendar_name_option(vm, context, option_value.clone())?;
    let options = temporal_string_options(vm, context, option_value)?;
    let local_ns = plain_date_order_key(context, object) as i128 * NS_PER_DAY_I128
        + plain_time_nanoseconds_i128(plain_date_time_values(context, object));
    let rounded = round_i128(local_ns, options.quantum, options.mode);
    let day_number = i64::try_from(rounded.div_euclid(NS_PER_DAY_I128))
        .map_err(|_| VmError::range("Temporal.PlainDateTime is out of range"))?;
    let (year, month, day) = civil_from_days(day_number);
    validate_plain_date(year as f64, month as f64, day as f64)?;
    let date = format!("{}-{}-{}", iso_year(year), two_digit(month), two_digit(day));
    let time = format_plain_time_precision(
        plain_time_from_nanoseconds_i128(rounded.rem_euclid(NS_PER_DAY_I128)),
        options.precision,
        options.minute_only,
    );
    let calendar_id = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let annotation = match calendar_name.as_str() {
        "always" => format!("[u-ca={calendar_id}]"),
        "critical" => format!("[!u-ca={calendar_id}]"),
        "auto" if calendar_id != "iso8601" => format!("[u-ca={calendar_id}]"),
        _ => String::new(),
    };
    Ok(JsValue::String(format!("{date}T{time}{annotation}")))
}

fn temporal_calendar_name_option(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<String, VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok("auto".into());
    }
    let object = context.require_object(&value, "Temporal toString options")?;
    let value = temporal_get_property(vm, context, object, "calendarName")?;
    let name = if matches!(value, JsValue::Undefined) {
        "auto".into()
    } else {
        vm.to_string_coerce(value, context)?
    };
    match name.as_str() {
        "auto" | "always" | "never" | "critical" => Ok(name),
        _ => Err(VmError::range("invalid Temporal calendarName")),
    }
}

fn plain_date_time_values(context: &NativeContext, object: ObjectId) -> PlainTimeValues {
    PlainTimeValues {
        hour: temporal_number_slot(context, object, "hour"),
        minute: temporal_number_slot(context, object, "minute"),
        second: temporal_number_slot(context, object, "second"),
        millisecond: temporal_number_slot(context, object, "millisecond"),
        microsecond: temporal_number_slot(context, object, "microsecond"),
        nanosecond: temporal_number_slot(context, object, "nanosecond"),
    }
}

fn temporal_plain_date_time_order_key(context: &NativeContext, object: ObjectId) -> f64 {
    plain_date_order_key(context, object) as f64 * NS_PER_DAY
        + time_nanoseconds(plain_date_time_values(context, object))
}

fn ordering_number(left: f64, right: f64) -> JsValue {
    JsValue::Number(if left < right {
        -1.0
    } else if left > right {
        1.0
    } else {
        0.0
    })
}

fn time_nanoseconds(values: PlainTimeValues) -> f64 {
    values.hour * NS_PER_HOUR
        + values.minute * NS_PER_MINUTE
        + values.second * NS_PER_SECOND
        + values.millisecond * NS_PER_MILLISECOND
        + values.microsecond * NS_PER_MICROSECOND
        + values.nanosecond
}

fn plain_time_from_nanoseconds(total: f64) -> (i64, PlainTimeValues) {
    let days = (total / NS_PER_DAY).floor() as i64;
    let mut remainder = (total - days as f64 * NS_PER_DAY).round();
    let hours = (remainder / NS_PER_HOUR).floor();
    remainder -= hours * NS_PER_HOUR;
    let minutes = (remainder / NS_PER_MINUTE).floor();
    remainder -= minutes * NS_PER_MINUTE;
    let seconds = (remainder / NS_PER_SECOND).floor();
    remainder -= seconds * NS_PER_SECOND;
    let milliseconds = (remainder / NS_PER_MILLISECOND).floor();
    remainder -= milliseconds * NS_PER_MILLISECOND;
    let microseconds = (remainder / NS_PER_MICROSECOND).floor();
    remainder -= microseconds * NS_PER_MICROSECOND;
    (
        days,
        PlainTimeValues {
            hour: clean_zero(hours),
            minute: clean_zero(minutes),
            second: clean_zero(seconds),
            millisecond: clean_zero(milliseconds),
            microsecond: clean_zero(microseconds),
            nanosecond: clean_zero(remainder),
        },
    )
}

fn temporal_plain_date_time_additive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: f64,
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let duration = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (mut year, mut month, mut day) = apply_duration_to_date(
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        duration,
        sign,
        reject_overflow,
    )?;
    let time_ns = time_nanoseconds(plain_date_time_values(context, object))
        + sign * duration_time_nanoseconds(duration);
    let (extra_days, time) = plain_time_from_nanoseconds(time_ns);
    if extra_days != 0 {
        let day_number = days_from_civil(year as i32, month as u32, day as u32)
            .checked_add(extra_days)
            .filter(|value| value.unsigned_abs() <= 100_000_000)
            .ok_or_else(|| VmError::range("Temporal.PlainDateTime is out of range"))?;
        let fields = civil_from_days(day_number);
        year = fields.0 as f64;
        month = fields.1 as f64;
        day = fields.2 as f64;
    }
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar(
        context,
        prototype,
        year,
        month,
        day,
        time,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_time_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_time_additive(vm, context, this_value, arguments, 1.0)
}

fn temporal_plain_date_time_subtract(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_time_additive(vm, context, this_value, arguments, -1.0)
}

fn temporal_plain_date_time_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let replacement = context.require_object(
        &arguments.first().cloned().unwrap_or(JsValue::Undefined),
        "Temporal.PlainDateTime.prototype.with",
    )?;
    reject_temporal_with_metadata(vm, context, replacement)?;
    let year = temporal_date_replacement(
        vm,
        context,
        replacement,
        "year",
        temporal_number_slot(context, this_object, "year"),
    )?;
    let month = temporal_month_from_object(
        vm,
        context,
        replacement,
        temporal_number_slot(context, this_object, "month"),
    )?;
    let day = temporal_date_replacement(
        vm,
        context,
        replacement,
        "day",
        temporal_number_slot(context, this_object, "day"),
    )?;
    let mut time = PlainTimeValues {
        hour: temporal_date_replacement(
            vm,
            context,
            replacement,
            "hour",
            temporal_number_slot(context, this_object, "hour"),
        )?,
        minute: temporal_date_replacement(
            vm,
            context,
            replacement,
            "minute",
            temporal_number_slot(context, this_object, "minute"),
        )?,
        second: temporal_date_replacement(
            vm,
            context,
            replacement,
            "second",
            temporal_number_slot(context, this_object, "second"),
        )?,
        millisecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "millisecond",
            temporal_number_slot(context, this_object, "millisecond"),
        )?,
        microsecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "microsecond",
            temporal_number_slot(context, this_object, "microsecond"),
        )?,
        nanosecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "nanosecond",
            temporal_number_slot(context, this_object, "nanosecond"),
        )?,
    };
    require_temporal_with_field(
        vm,
        context,
        replacement,
        &[
            "day",
            "hour",
            "microsecond",
            "millisecond",
            "minute",
            "month",
            "monthCode",
            "nanosecond",
            "second",
            "year",
        ],
    )?;
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let month = if reject_overflow {
        month
    } else {
        month.clamp(1.0, 12.0)
    };
    let day = if reject_overflow {
        day
    } else {
        day.clamp(1.0, month_day_count(year as i32, month as u32) as f64)
    };
    if !reject_overflow {
        time.hour = time.hour.clamp(0.0, 23.0);
        time.minute = time.minute.clamp(0.0, 59.0);
        time.second = time.second.clamp(0.0, 59.0);
        time.millisecond = time.millisecond.clamp(0.0, 999.0);
        time.microsecond = time.microsecond.clamp(0.0, 999.0);
        time.nanosecond = time.nanosecond.clamp(0.0, 999.0);
    }
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar(
        context,
        prototype,
        year,
        month,
        day,
        time,
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_time_with_calendar(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.first())?;
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        plain_date_time_values(context, object),
        calendar_id,
    )
}

fn temporal_plain_date_time_with_plain_time(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let time = plain_time_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        time,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_time_equals(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let other = temporal_plain_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    Ok(JsValue::Boolean(
        temporal_number_slot(context, this_object, "year")
            == temporal_number_slot(context, other_object, "year")
            && temporal_number_slot(context, this_object, "month")
                == temporal_number_slot(context, other_object, "month")
            && temporal_number_slot(context, this_object, "day")
                == temporal_number_slot(context, other_object, "day")
            && time_nanoseconds(plain_date_time_values(context, this_object))
                == time_nanoseconds(plain_date_time_values(context, other_object))
            && own_string(context, this_object, "calendarId")
                == own_string(context, other_object, "calendarId"),
    ))
}

fn temporal_plain_date_time_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = temporal_plain_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let right = temporal_plain_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.get(1).cloned().unwrap_or(JsValue::Undefined)],
    )?;
    Ok(ordering_number(
        temporal_plain_date_time_order_key(context, context.value_object(&left).unwrap()),
        temporal_plain_date_time_order_key(context, context.value_object(&right).unwrap()),
    ))
}

fn temporal_plain_date_time_round(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let (unit, increment, mode) = plain_date_time_round_options(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let year = temporal_number_slot(context, object, "year") as i32;
    let month = temporal_number_slot(context, object, "month") as u32;
    let day = temporal_number_slot(context, object, "day") as u32;
    let time = plain_date_time_values(context, object);
    let local_ns = days_from_civil(year, month, day) as i128 * NS_PER_DAY_I128
        + plain_time_nanoseconds_i128(time);
    let quantum = temporal_unit_nanoseconds(&unit)
        .checked_mul(increment as i128)
        .ok_or_else(|| VmError::range("invalid Temporal rounding increment"))?;
    let rounded = round_i128(local_ns, quantum, mode);
    let rounded_day = rounded.div_euclid(NS_PER_DAY_I128);
    let rounded_time = plain_time_from_nanoseconds_i128(rounded.rem_euclid(NS_PER_DAY_I128));
    let rounded_day = i64::try_from(rounded_day)
        .map_err(|_| VmError::range("Temporal.PlainDateTime is out of range"))?;
    let (year, month, day) = civil_from_days(rounded_day);
    validate_plain_date(year as f64, month as f64, day as f64)?;
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar(
        context,
        prototype,
        year as f64,
        month as f64,
        day as f64,
        rounded_time,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn plain_time_nanoseconds_i128(values: PlainTimeValues) -> i128 {
    values.hour as i128 * NS_PER_HOUR_I128
        + values.minute as i128 * NS_PER_MINUTE_I128
        + values.second as i128 * NS_PER_SECOND_I128
        + values.millisecond as i128 * NS_PER_MILLISECOND_I128
        + values.microsecond as i128 * 1_000
        + values.nanosecond as i128
}

fn plain_time_from_nanoseconds_i128(mut value: i128) -> PlainTimeValues {
    let hour = value / NS_PER_HOUR_I128;
    value %= NS_PER_HOUR_I128;
    let minute = value / NS_PER_MINUTE_I128;
    value %= NS_PER_MINUTE_I128;
    let second = value / NS_PER_SECOND_I128;
    value %= NS_PER_SECOND_I128;
    let millisecond = value / NS_PER_MILLISECOND_I128;
    value %= NS_PER_MILLISECOND_I128;
    let microsecond = value / 1_000;
    let nanosecond = value % 1_000;
    PlainTimeValues {
        hour: hour as f64,
        minute: minute as f64,
        second: second as f64,
        millisecond: millisecond as f64,
        microsecond: microsecond as f64,
        nanosecond: nanosecond as f64,
    }
}

fn plain_date_time_round_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<(String, u64, TemporalRoundMode), VmError> {
    if let JsValue::String(unit) = value {
        let unit = normalize_temporal_unit(unit)?;
        if matches!(unit.as_str(), "year" | "month" | "week") {
            return Err(VmError::range("invalid PlainDateTime rounding unit"));
        }
        return Ok((unit, 1, TemporalRoundMode::HalfExpand));
    }
    let object =
        context.require_object(&value, "Temporal.PlainDateTime.prototype.round options")?;
    let increment = option_rounding_increment(vm, context, object)?;
    let mode = temporal_round_mode(
        option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
        TemporalRoundMode::HalfExpand,
    )?;
    let unit = option_string(vm, context, object, "smallestUnit")?
        .ok_or_else(|| VmError::range("smallestUnit is required"))?;
    let unit = normalize_temporal_unit(unit)?;
    if matches!(unit.as_str(), "year" | "month" | "week") {
        return Err(VmError::range("invalid PlainDateTime rounding unit"));
    }
    let maximum = match unit.as_str() {
        "day" => 2,
        "hour" => 24,
        "minute" | "second" => 60,
        "millisecond" | "microsecond" | "nanosecond" => 1_000,
        _ => 1,
    };
    if increment >= maximum || maximum % increment != 0 {
        return Err(VmError::range("invalid Temporal rounding increment"));
    }
    Ok((unit, increment, mode))
}

fn temporal_plain_date_time_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: i128,
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let other = temporal_plain_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    if own_string(context, this_object, "calendarId")
        != own_string(context, other_object, "calendarId")
    {
        return Err(VmError::range("Temporal calendars must match"));
    }
    let options = plain_date_time_difference_options(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        "day",
        true,
    )?;
    let this_ns = plain_date_order_key(context, this_object) as i128 * NS_PER_DAY_I128
        + plain_time_nanoseconds_i128(plain_date_time_values(context, this_object));
    let other_ns = plain_date_order_key(context, other_object) as i128 * NS_PER_DAY_I128
        + plain_time_nanoseconds_i128(plain_date_time_values(context, other_object));
    let quantum = temporal_unit_nanoseconds(&options.smallest_unit) * options.increment as i128;
    let rounded = round_signed_i128(sign * (other_ns - this_ns), quantum, options.mode);
    create_duration_from_nanoseconds(context, rounded, &options.largest_unit)
}

fn plain_date_time_difference_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    default_largest_unit: &str,
    allow_day: bool,
) -> Result<InstantDifferenceOptions, VmError> {
    let object = if matches!(value, JsValue::Undefined) {
        None
    } else {
        Some(context.require_object(&value, "Temporal.PlainDateTime difference options")?)
    };
    let largest = match object {
        Some(object) => option_string(vm, context, object, "largestUnit")?,
        None => None,
    };
    let increment = match object {
        Some(object) => option_rounding_increment(vm, context, object)?,
        None => 1,
    };
    let mode = match object {
        Some(object) => temporal_round_mode(
            option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
            TemporalRoundMode::Trunc,
        )?,
        None => TemporalRoundMode::Trunc,
    };
    let smallest = match object {
        Some(object) => option_string(vm, context, object, "smallestUnit")?,
        None => None,
    };
    let largest_unit =
        normalize_temporal_unit(largest.unwrap_or_else(|| default_largest_unit.into()))?;
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "nanosecond".into()))?;
    if temporal_unit_nanoseconds(&largest_unit) < temporal_unit_nanoseconds(&smallest_unit) {
        return Err(VmError::range(
            "largestUnit must not be smaller than smallestUnit",
        ));
    }
    if matches!(smallest_unit.as_str(), "year" | "month" | "week") {
        return Err(VmError::range("invalid PlainDateTime smallestUnit"));
    }
    if !allow_day && (largest_unit == "day" || smallest_unit == "day") {
        return Err(VmError::range("day is not a valid PlainTime unit"));
    }
    let maximum = match smallest_unit.as_str() {
        "day" => 2,
        "hour" => 24,
        "minute" | "second" => 60,
        "millisecond" | "microsecond" | "nanosecond" => 1_000,
        _ => 1,
    };
    if increment >= maximum || maximum % increment != 0 {
        return Err(VmError::range("invalid Temporal rounding increment"));
    }
    Ok(InstantDifferenceOptions {
        largest_unit,
        smallest_unit,
        increment,
        mode,
    })
}

fn temporal_plain_date_time_until(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_time_difference(vm, context, this_value, arguments, 1)
}

fn temporal_plain_date_time_since(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_time_difference(vm, context, this_value, arguments, -1)
}

fn temporal_plain_date_time_to_plain_date(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date_with_calendar(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_date_time_to_plain_time(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    create_plain_time(context, prototype, plain_date_time_values(context, object))
}

fn temporal_plain_date_time_to_zoned_date_time(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let time_zone_value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if matches!(time_zone_value, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal.PlainDateTime.prototype.toZonedDateTime requires a time zone",
        ));
    }
    let time_zone_id = vm.to_string_coerce(time_zone_value, context)?;
    let exact_epoch_nanoseconds = epoch_nanoseconds_i128_from_plain_parts(
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        plain_date_time_values(context, object),
    ) - parse_time_zone_offset_ns(&time_zone_id).unwrap_or(0);
    let epoch_nanoseconds = exact_epoch_nanoseconds as f64;
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        JsValue::BigInt(bigint::from_i128(exact_epoch_nanoseconds)),
        epoch_nanoseconds,
        time_zone_id,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn month_code(month: f64) -> String {
    format!("M{}", two_digit(month.trunc() as u32))
}

fn parse_month_code(value: &str) -> Option<f64> {
    let month = value.strip_prefix('M')?;
    Some(parse_fixed_digits(month, 2)? as f64)
}

fn temporal_calendar_id_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
) -> Result<String, VmError> {
    let value = temporal_get_property(vm, context, object, "calendar")?;
    let value = if matches!(value, JsValue::Undefined) {
        temporal_get_property(vm, context, object, "calendarId")?
    } else {
        value
    };
    if matches!(value, JsValue::Undefined) {
        Ok("iso8601".into())
    } else {
        let text = vm.to_string_coerce(value, context)?.to_ascii_lowercase();
        if text.is_empty() {
            Err(VmError::range("invalid Temporal calendar"))
        } else {
            Ok(text)
        }
    }
}

fn install_temporal_plain_year_month(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "PlainYearMonth",
        2,
        temporal_constructor_call_error,
        temporal_plain_year_month_construct,
        "Temporal.PlainYearMonth",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_plain_year_month_from,
    )?;
    define_method(
        context,
        constructor_object,
        "compare",
        2,
        temporal_plain_year_month_compare,
    )?;
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_plain_year_month_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toJSON",
        0,
        temporal_plain_year_month_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_plain_year_month_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toPlainDate",
        1,
        temporal_plain_year_month_to_plain_date,
    )?;
    define_method(
        context,
        prototype,
        "equals",
        1,
        temporal_plain_year_month_equals,
    )?;
    for (name, length, call) in [
        ("add", 1, temporal_plain_year_month_add as NativeCall),
        (
            "subtract",
            1,
            temporal_plain_year_month_subtract as NativeCall,
        ),
        ("with", 1, temporal_plain_year_month_with as NativeCall),
        ("until", 1, temporal_plain_year_month_until as NativeCall),
        ("since", 1, temporal_plain_year_month_since as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [
        ("year", "get year", "year"),
        ("month", "get month", "month"),
        ("daysInMonth", "get daysInMonth", "daysInMonth"),
        ("daysInYear", "get daysInYear", "daysInYear"),
        ("monthsInYear", "get monthsInYear", "monthsInYear"),
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "PlainYearMonth", slot)?;
    }
    for (name, getter, slot) in [
        ("monthCode", "get monthCode", "monthCode"),
        ("calendarId", "get calendarId", "calendarId"),
    ] {
        define_temporal_string_slot_getter(
            context,
            prototype,
            name,
            getter,
            "PlainYearMonth",
            slot,
        )?;
    }
    define_temporal_bool_slot_getter(
        context,
        prototype,
        "inLeapYear",
        "get inLeapYear",
        "PlainYearMonth",
        "inLeapYear",
    )?;
    define_temporal_undefined_getter(context, prototype, "era", "get era", "PlainYearMonth")?;
    define_temporal_undefined_getter(
        context,
        prototype,
        "eraYear",
        "get eraYear",
        "PlainYearMonth",
    )?;
    Ok(())
}

fn create_plain_year_month(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    reference_day: f64,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    validate_plain_date(year, month, reference_day)?;
    let year_i = year.trunc() as i32;
    let month_u = month.trunc() as u32;
    create_temporal_object(
        context,
        prototype,
        "PlainYearMonth",
        [
            ("year", JsValue::Number(year.trunc())),
            ("month", JsValue::Number(month.trunc())),
            ("referenceISODay", JsValue::Number(reference_day.trunc())),
            ("monthCode", JsValue::String(month_code(month))),
            ("calendarId", JsValue::String(calendar_id)),
            (
                "daysInMonth",
                JsValue::Number(month_day_count(year_i, month_u) as f64),
            ),
            (
                "daysInYear",
                JsValue::Number(if is_leap_year(year_i) { 366.0 } else { 365.0 }),
            ),
            ("monthsInYear", JsValue::Number(12.0)),
            ("inLeapYear", JsValue::Boolean(is_leap_year(year_i))),
        ],
    )
}

fn temporal_plain_year_month_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.PlainYearMonth prototype missing"))?;
    let year = vm.to_number(arg_or_undefined(arguments, 0), context)?;
    let month = vm.to_number(arg_or_undefined(arguments, 1), context)?;
    let calendar = if matches!(arguments.get(2), None | Some(JsValue::Undefined)) {
        "iso8601".into()
    } else {
        vm.to_string_coerce(arg_or_undefined(arguments, 2), context)?
            .to_ascii_lowercase()
    };
    let reference_day = if matches!(arguments.get(3), None | Some(JsValue::Undefined)) {
        1.0
    } else {
        vm.to_number(arg_or_undefined(arguments, 3), context)?
    };
    create_plain_year_month(context, prototype, year, month, reference_day, calendar)
}

fn parse_plain_year_month(text: &str) -> Option<(f64, f64, f64)> {
    let text = text.split('[').next().unwrap_or(text);
    let date = text.split('T').next().unwrap_or(text);
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parse_fixed_digits(parts.next()?, 2)?;
    let day = match parts.next() {
        Some(value) => parse_fixed_digits_day(value)?,
        None => 1,
    };
    Some((year as f64, month as f64, day as f64))
}

fn parse_fixed_digits_day(value: &str) -> Option<u32> {
    parse_fixed_digits(value, 2)
}

fn temporal_plain_year_month_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainYearMonth")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (year, month, reference_day, calendar_id) = match item {
        JsValue::String(text) => {
            let (year, month, _day) = parse_plain_year_month(&text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainYearMonth"))?;
            (year, month, 1.0, "iso8601".into())
        }
        value => {
            let object = context.require_object(&value, "Temporal.PlainYearMonth.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("PlainYearMonth") {
                (
                    temporal_number_slot(context, object, "year"),
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "referenceISODay"),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else {
                let year = temporal_required_object_number(vm, context, object, "year")?;
                let month_value = temporal_get_property(vm, context, object, "month")?;
                let month = if matches!(month_value, JsValue::Undefined) {
                    let month_code_value = temporal_get_property(vm, context, object, "monthCode")?;
                    let month_code = vm.to_string_coerce(month_code_value, context)?;
                    parse_month_code(&month_code).ok_or_else(|| {
                        VmError::range("invalid Temporal.PlainYearMonth monthCode")
                    })?
                } else {
                    vm.to_number(month_value, context)?
                };
                let _day = temporal_get_property(vm, context, object, "day")?;
                (
                    year,
                    month,
                    1.0,
                    temporal_calendar_id_from_object(vm, context, object)?,
                )
            }
        }
    };
    create_plain_year_month(context, prototype, year, month, reference_day, calendar_id)
}

fn plain_year_month_order_key(context: &NativeContext, object: ObjectId) -> i64 {
    let year = temporal_number_slot(context, object, "year") as i64;
    let month = temporal_number_slot(context, object, "month") as i64;
    year.saturating_mul(12).saturating_add(month)
}

fn temporal_plain_year_month_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = temporal_plain_year_month_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let right = temporal_plain_year_month_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.get(1).cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let left = plain_year_month_order_key(context, context.value_object(&left).unwrap());
    let right = plain_year_month_order_key(context, context.value_object(&right).unwrap());
    Ok(JsValue::Number(if left < right {
        -1.0
    } else if left > right {
        1.0
    } else {
        0.0
    }))
}

fn temporal_plain_year_month_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainYearMonth")?;
    let calendar_name = temporal_calendar_name_option(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    if matches!(calendar_name.as_str(), "always" | "critical") {
        return Ok(JsValue::String(format!(
            "{}-{}-{}[{}u-ca={}]",
            iso_year(temporal_number_slot(context, object, "year") as i32),
            two_digit(temporal_number_slot(context, object, "month") as u32),
            two_digit(temporal_number_slot(context, object, "referenceISODay") as u32),
            if calendar_name == "critical" { "!" } else { "" },
            own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into())
        )));
    }
    Ok(JsValue::String(format!(
        "{}-{}",
        iso_year(temporal_number_slot(context, object, "year") as i32),
        two_digit(temporal_number_slot(context, object, "month") as u32)
    )))
}

fn temporal_plain_year_month_to_plain_date(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainYearMonth")?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    let day_arg = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let day = if let Some(object) = context.value_object(&day_arg) {
        temporal_object_number(vm, context, object, "day")?
    } else if matches!(day_arg, JsValue::Undefined) {
        temporal_number_slot(context, object, "referenceISODay").max(1.0)
    } else {
        vm.to_number(day_arg, context)?
    };
    create_plain_date(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        day,
    )
}

fn temporal_plain_year_month_equals(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainYearMonth")?;
    let other = temporal_plain_year_month_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    Ok(JsValue::Boolean(
        temporal_number_slot(context, this_object, "year")
            == temporal_number_slot(context, other_object, "year")
            && temporal_number_slot(context, this_object, "month")
                == temporal_number_slot(context, other_object, "month")
            && own_string(context, this_object, "calendarId")
                == own_string(context, other_object, "calendarId"),
    ))
}

fn temporal_plain_year_month_additive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: f64,
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainYearMonth")?;
    let duration = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let month_delta = (sign * (duration.years * 12.0 + duration.months)).trunc() as i64;
    let month_index = (temporal_number_slot(context, object, "year") as i64 * 12
        + temporal_number_slot(context, object, "month") as i64
        - 1)
    .checked_add(month_delta)
    .ok_or_else(|| VmError::range("Temporal.PlainYearMonth is out of range"))?;
    let year = month_index.div_euclid(12) as f64;
    let month = (month_index.rem_euclid(12) + 1) as f64;
    let prototype = temporal_constructor_prototype(context, "PlainYearMonth")?;
    create_plain_year_month(
        context,
        prototype,
        year,
        month,
        temporal_number_slot(context, object, "referenceISODay").max(1.0),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_year_month_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_year_month_additive(vm, context, this_value, arguments, 1.0)
}

fn temporal_plain_year_month_subtract(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_year_month_additive(vm, context, this_value, arguments, -1.0)
}

fn temporal_plain_year_month_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainYearMonth")?;
    let replacement = context.require_object(
        &arguments.first().cloned().unwrap_or(JsValue::Undefined),
        "Temporal.PlainYearMonth.prototype.with",
    )?;
    let year = temporal_date_replacement(
        vm,
        context,
        replacement,
        "year",
        temporal_number_slot(context, this_object, "year"),
    )?;
    let month = temporal_month_from_object(
        vm,
        context,
        replacement,
        temporal_number_slot(context, this_object, "month"),
    )?;
    let day = temporal_date_replacement(
        vm,
        context,
        replacement,
        "day",
        temporal_number_slot(context, this_object, "referenceISODay").max(1.0),
    )?;
    let prototype = temporal_constructor_prototype(context, "PlainYearMonth")?;
    create_plain_year_month(
        context,
        prototype,
        year,
        month,
        day,
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_plain_year_month_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: i64,
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainYearMonth")?;
    let other = temporal_plain_year_month_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    if own_string(context, this_object, "calendarId")
        != own_string(context, other_object, "calendarId")
    {
        return Err(VmError::range("Temporal calendars must match"));
    }
    let options = year_month_difference_options(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let months = sign
        * (plain_year_month_order_key(context, other_object)
            - plain_year_month_order_key(context, this_object));
    let quantum = if options.smallest_unit == "year" {
        12
    } else {
        1
    } * options.increment as i128;
    let months = round_signed_i128(months as i128, quantum, options.mode) as i64;
    let (years, months) = if options.largest_unit == "year" {
        ((months / 12) as f64, (months % 12) as f64)
    } else {
        (0.0, months as f64)
    };
    create_duration_with_default_prototype(
        context,
        DurationValues {
            years,
            months,
            ..DurationValues::default()
        },
    )
}

fn year_month_difference_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<DateDifferenceOptions, VmError> {
    let object = if matches!(value, JsValue::Undefined) {
        None
    } else {
        Some(context.require_object(&value, "Temporal.PlainYearMonth difference options")?)
    };
    let largest = match object {
        Some(object) => option_string(vm, context, object, "largestUnit")?,
        None => None,
    };
    let increment = match object {
        Some(object) => option_rounding_increment(vm, context, object)?,
        None => 1,
    };
    let mode = match object {
        Some(object) => temporal_round_mode(
            option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
            TemporalRoundMode::Trunc,
        )?,
        None => TemporalRoundMode::Trunc,
    };
    let smallest = match object {
        Some(object) => option_string(vm, context, object, "smallestUnit")?,
        None => None,
    };
    let largest_unit = normalize_temporal_unit(largest.unwrap_or_else(|| "month".into()))?;
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "month".into()))?;
    if !matches!(largest_unit.as_str(), "year" | "month")
        || !matches!(smallest_unit.as_str(), "year" | "month")
        || temporal_unit_nanoseconds(&largest_unit) < temporal_unit_nanoseconds(&smallest_unit)
    {
        return Err(VmError::range("invalid PlainYearMonth difference units"));
    }
    Ok(DateDifferenceOptions {
        largest_unit,
        smallest_unit,
        increment,
        mode,
    })
}

fn temporal_plain_year_month_until(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_year_month_difference(vm, context, this_value, arguments, 1)
}

fn temporal_plain_year_month_since(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_year_month_difference(vm, context, this_value, arguments, -1)
}

fn install_temporal_plain_month_day(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "PlainMonthDay",
        2,
        temporal_constructor_call_error,
        temporal_plain_month_day_construct,
        "Temporal.PlainMonthDay",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_plain_month_day_from,
    )?;
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_plain_month_day_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toJSON",
        0,
        temporal_plain_month_day_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_plain_month_day_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toPlainDate",
        1,
        temporal_plain_month_day_to_plain_date,
    )?;
    define_method(
        context,
        prototype,
        "equals",
        1,
        temporal_plain_month_day_equals,
    )?;
    define_method(context, prototype, "with", 1, temporal_plain_month_day_with)?;
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [("month", "get month", "month"), ("day", "get day", "day")] {
        define_temporal_slot_getter(context, prototype, name, getter, "PlainMonthDay", slot)?;
    }
    for (name, getter, slot) in [
        ("monthCode", "get monthCode", "monthCode"),
        ("calendarId", "get calendarId", "calendarId"),
    ] {
        define_temporal_string_slot_getter(
            context,
            prototype,
            name,
            getter,
            "PlainMonthDay",
            slot,
        )?;
    }
    Ok(())
}

fn create_plain_month_day(
    context: &mut NativeContext,
    prototype: ObjectId,
    month: f64,
    day: f64,
    reference_year: f64,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    validate_plain_date(reference_year, month, day)?;
    create_temporal_object(
        context,
        prototype,
        "PlainMonthDay",
        [
            ("month", JsValue::Number(month.trunc())),
            ("day", JsValue::Number(day.trunc())),
            ("monthCode", JsValue::String(month_code(month))),
            ("calendarId", JsValue::String(calendar_id)),
            ("referenceISOYear", JsValue::Number(reference_year.trunc())),
        ],
    )
}

fn temporal_plain_month_day_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.PlainMonthDay prototype missing"))?;
    let month = vm.to_number(arg_or_undefined(arguments, 0), context)?;
    let day = vm.to_number(arg_or_undefined(arguments, 1), context)?;
    let calendar = if matches!(arguments.get(2), None | Some(JsValue::Undefined)) {
        "iso8601".into()
    } else {
        vm.to_string_coerce(arg_or_undefined(arguments, 2), context)?
            .to_ascii_lowercase()
    };
    let reference_year = if matches!(arguments.get(3), None | Some(JsValue::Undefined)) {
        1972.0
    } else {
        vm.to_number(arg_or_undefined(arguments, 3), context)?
    };
    create_plain_month_day(context, prototype, month, day, reference_year, calendar)
}

fn parse_plain_month_day(text: &str) -> Option<(f64, f64, f64)> {
    let text = text.split('[').next().unwrap_or(text);
    if let Some(rest) = text.strip_prefix("--") {
        let mut parts = rest.split('-');
        let month = parse_fixed_digits(parts.next()?, 2)?;
        let day = parse_fixed_digits(parts.next()?, 2)?;
        return Some((month as f64, day as f64, 1972.0));
    }
    let (year, month, day) = parse_plain_date(text)?;
    Some((month, day, year))
}

fn temporal_plain_month_day_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainMonthDay")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (month, day, reference_year, calendar_id) = match item {
        JsValue::String(text) => {
            let (month, day, _year) = parse_plain_month_day(&text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainMonthDay"))?;
            (month, day, 1972.0, "iso8601".into())
        }
        value => {
            let object = context.require_object(&value, "Temporal.PlainMonthDay.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("PlainMonthDay") {
                (
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "day"),
                    temporal_number_slot(context, object, "referenceISOYear"),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else {
                let month_value = temporal_get_property(vm, context, object, "month")?;
                let month = if matches!(month_value, JsValue::Undefined) {
                    let month_code_value = temporal_get_property(vm, context, object, "monthCode")?;
                    let month_code = vm.to_string_coerce(month_code_value, context)?;
                    parse_month_code(&month_code)
                        .ok_or_else(|| VmError::range("invalid Temporal.PlainMonthDay monthCode"))?
                } else {
                    vm.to_number(month_value, context)?
                };
                let day = temporal_object_number(vm, context, object, "day")?;
                let _year = temporal_get_property(vm, context, object, "year")?;
                (
                    month,
                    day,
                    1972.0,
                    temporal_calendar_id_from_object(vm, context, object)?,
                )
            }
        }
    };
    create_plain_month_day(context, prototype, month, day, reference_year, calendar_id)
}

fn temporal_plain_month_day_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainMonthDay")?;
    let calendar_name = temporal_calendar_name_option(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    if matches!(calendar_name.as_str(), "always" | "critical") {
        return Ok(JsValue::String(format!(
            "{}-{}-{}[{}u-ca={}]",
            iso_year(temporal_number_slot(context, object, "referenceISOYear") as i32),
            two_digit(temporal_number_slot(context, object, "month") as u32),
            two_digit(temporal_number_slot(context, object, "day") as u32),
            if calendar_name == "critical" { "!" } else { "" },
            own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into())
        )));
    }
    Ok(JsValue::String(format!(
        "{}-{}",
        two_digit(temporal_number_slot(context, object, "month") as u32),
        two_digit(temporal_number_slot(context, object, "day") as u32)
    )))
}

fn temporal_plain_month_day_to_plain_date(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainMonthDay")?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    let year_arg = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let year = if let Some(object) = context.value_object(&year_arg) {
        temporal_object_number(vm, context, object, "year")?
    } else if matches!(year_arg, JsValue::Undefined) {
        temporal_number_slot(context, object, "referenceISOYear").max(1972.0)
    } else {
        vm.to_number(year_arg, context)?
    };
    create_plain_date(
        context,
        prototype,
        year,
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
    )
}

fn temporal_plain_month_day_equals(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainMonthDay")?;
    let other = temporal_plain_month_day_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    Ok(JsValue::Boolean(
        temporal_number_slot(context, this_object, "month")
            == temporal_number_slot(context, other_object, "month")
            && temporal_number_slot(context, this_object, "day")
                == temporal_number_slot(context, other_object, "day")
            && own_string(context, this_object, "calendarId")
                == own_string(context, other_object, "calendarId"),
    ))
}

fn temporal_plain_month_day_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "PlainMonthDay")?;
    let replacement = context.require_object(
        &arguments.first().cloned().unwrap_or(JsValue::Undefined),
        "Temporal.PlainMonthDay.prototype.with",
    )?;
    let month = temporal_month_from_object(
        vm,
        context,
        replacement,
        temporal_number_slot(context, this_object, "month"),
    )?;
    let day = temporal_date_replacement(
        vm,
        context,
        replacement,
        "day",
        temporal_number_slot(context, this_object, "day"),
    )?;
    let reference_year = temporal_date_replacement(
        vm,
        context,
        replacement,
        "year",
        temporal_number_slot(context, this_object, "referenceISOYear").max(1972.0),
    )?;
    let prototype = temporal_constructor_prototype(context, "PlainMonthDay")?;
    create_plain_month_day(
        context,
        prototype,
        month,
        day,
        reference_year,
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn install_temporal_zoned_date_time(
    context: &mut NativeContext,
    temporal: ObjectId,
) -> Result<(), VmError> {
    let (constructor, prototype) = temporal_constructor(
        context,
        temporal,
        "ZonedDateTime",
        2,
        temporal_constructor_call_error,
        temporal_zoned_date_time_construct,
        "Temporal.ZonedDateTime",
    )?;
    let constructor_object = context.value_object(&constructor).unwrap();
    define_method(
        context,
        constructor_object,
        "from",
        1,
        temporal_zoned_date_time_from,
    )?;
    define_method(
        context,
        constructor_object,
        "compare",
        2,
        temporal_zoned_date_time_compare,
    )?;
    define_method(
        context,
        prototype,
        "toString",
        0,
        temporal_zoned_date_time_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toJSON",
        0,
        temporal_zoned_date_time_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_zoned_date_time_to_string,
    )?;
    define_method(
        context,
        prototype,
        "toInstant",
        0,
        temporal_zoned_date_time_to_instant,
    )?;
    define_method(
        context,
        prototype,
        "toPlainDateTime",
        0,
        temporal_zoned_date_time_to_plain_date_time,
    )?;
    define_method(
        context,
        prototype,
        "toPlainDate",
        0,
        temporal_zoned_date_time_to_plain_date,
    )?;
    define_method(
        context,
        prototype,
        "toPlainTime",
        0,
        temporal_zoned_date_time_to_plain_time,
    )?;
    define_method(
        context,
        prototype,
        "equals",
        1,
        temporal_zoned_date_time_equals,
    )?;
    for (name, length, call) in [
        ("add", 1, temporal_zoned_date_time_add as NativeCall),
        (
            "subtract",
            1,
            temporal_zoned_date_time_subtract as NativeCall,
        ),
        ("round", 1, temporal_zoned_date_time_round as NativeCall),
        ("until", 1, temporal_zoned_date_time_until as NativeCall),
        ("since", 1, temporal_zoned_date_time_since as NativeCall),
        ("with", 1, temporal_zoned_date_time_with as NativeCall),
        (
            "withCalendar",
            1,
            temporal_zoned_date_time_with_calendar as NativeCall,
        ),
        (
            "withPlainTime",
            0,
            temporal_zoned_date_time_with_plain_time as NativeCall,
        ),
        (
            "withTimeZone",
            1,
            temporal_zoned_date_time_with_time_zone as NativeCall,
        ),
        (
            "startOfDay",
            0,
            temporal_zoned_date_time_start_of_day as NativeCall,
        ),
        (
            "getTimeZoneTransition",
            0,
            temporal_zoned_date_time_get_time_zone_transition as NativeCall,
        ),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [
        (
            "epochMilliseconds",
            "get epochMilliseconds",
            "epochMilliseconds",
        ),
        ("year", "get year", "year"),
        ("month", "get month", "month"),
        ("day", "get day", "day"),
        ("hour", "get hour", "hour"),
        ("minute", "get minute", "minute"),
        ("second", "get second", "second"),
        ("millisecond", "get millisecond", "millisecond"),
        ("microsecond", "get microsecond", "microsecond"),
        ("nanosecond", "get nanosecond", "nanosecond"),
        (
            "offsetNanoseconds",
            "get offsetNanoseconds",
            "offsetNanoseconds",
        ),
        ("dayOfWeek", "get dayOfWeek", "dayOfWeek"),
        ("dayOfYear", "get dayOfYear", "dayOfYear"),
        ("weekOfYear", "get weekOfYear", "weekOfYear"),
        ("yearOfWeek", "get yearOfWeek", "yearOfWeek"),
        ("daysInWeek", "get daysInWeek", "daysInWeek"),
        ("daysInMonth", "get daysInMonth", "daysInMonth"),
        ("daysInYear", "get daysInYear", "daysInYear"),
        ("monthsInYear", "get monthsInYear", "monthsInYear"),
        ("hoursInDay", "get hoursInDay", "hoursInDay"),
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "ZonedDateTime", slot)?;
    }
    for (name, getter, slot) in [
        ("calendarId", "get calendarId", "calendarId"),
        ("timeZoneId", "get timeZoneId", "timeZoneId"),
        ("offset", "get offset", "offset"),
        ("monthCode", "get monthCode", "monthCode"),
    ] {
        define_temporal_string_slot_getter(
            context,
            prototype,
            name,
            getter,
            "ZonedDateTime",
            slot,
        )?;
    }
    define_temporal_bool_slot_getter(
        context,
        prototype,
        "inLeapYear",
        "get inLeapYear",
        "ZonedDateTime",
        "inLeapYear",
    )?;
    define_temporal_undefined_getter(context, prototype, "era", "get era", "ZonedDateTime")?;
    define_temporal_undefined_getter(
        context,
        prototype,
        "eraYear",
        "get eraYear",
        "ZonedDateTime",
    )?;
    define_accessor(
        context,
        prototype,
        "epochNanoseconds",
        "get epochNanoseconds",
        temporal_zoned_date_time_epoch_nanoseconds,
    )?;
    Ok(())
}

fn epoch_nanoseconds_to_i128(value: &JsValue) -> Result<i128, VmError> {
    match value {
        JsValue::BigInt(value) => Ok(bigint_to_i128_saturating(value)),
        JsValue::Number(value) if value.is_finite() && value.fract() == 0.0 => Ok(*value as i128),
        JsValue::Undefined => Err(VmError::type_error(
            "Temporal.ZonedDateTime requires epochNanoseconds",
        )),
        _ => Err(VmError::type_error("invalid Temporal epochNanoseconds")),
    }
}

fn create_zoned_date_time(
    context: &mut NativeContext,
    prototype: ObjectId,
    epoch_nanoseconds_value: JsValue,
    epoch_nanoseconds: f64,
    time_zone_id: String,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    if !epoch_nanoseconds.is_finite() {
        return Err(VmError::range("invalid Temporal.ZonedDateTime"));
    }
    let exact_epoch_nanoseconds = epoch_nanoseconds_to_i128(&epoch_nanoseconds_value)?;
    if !is_valid_instant_ns(exact_epoch_nanoseconds) {
        return Err(VmError::range("invalid Temporal.ZonedDateTime"));
    }
    let offset_nanoseconds = parse_time_zone_offset_ns(&time_zone_id).unwrap_or(0);
    let local_epoch_nanoseconds = exact_epoch_nanoseconds + offset_nanoseconds;
    let epoch_milliseconds = local_epoch_nanoseconds.div_euclid(1_000_000) as f64;
    let offset_string = if offset_nanoseconds == 0 {
        "+00:00".into()
    } else {
        time_zone_id.clone()
    };
    let fields = decompose_time(epoch_milliseconds).unwrap_or(DateFields {
        year: 1970,
        month: 1,
        day: 1,
        hour: 0,
        minute: 0,
        second: 0,
        millisecond: 0,
        weekday: 4,
    });
    let day_of_year = days_from_civil(fields.year, fields.month, fields.day)
        - days_from_civil(fields.year, 1, 1)
        + 1;
    let day_of_week = temporal_day_of_week(fields.year, fields.month, fields.day);
    let (week_of_year, year_of_week) = temporal_week_fields(fields.year, fields.month, fields.day);
    let leap = is_leap_year(fields.year);
    create_temporal_object(
        context,
        prototype,
        "ZonedDateTime",
        [
            ("epochNanoseconds", epoch_nanoseconds_value),
            ("epochNanosecondsNumber", JsValue::Number(epoch_nanoseconds)),
            ("epochMilliseconds", JsValue::Number(epoch_milliseconds)),
            ("timeZoneId", JsValue::String(time_zone_id)),
            ("calendarId", JsValue::String(calendar_id)),
            ("offset", JsValue::String(offset_string)),
            (
                "offsetNanoseconds",
                JsValue::Number(offset_nanoseconds as f64),
            ),
            ("year", JsValue::Number(fields.year as f64)),
            ("month", JsValue::Number(fields.month as f64)),
            (
                "monthCode",
                JsValue::String(month_code(fields.month as f64)),
            ),
            ("day", JsValue::Number(fields.day as f64)),
            ("hour", JsValue::Number(fields.hour as f64)),
            ("minute", JsValue::Number(fields.minute as f64)),
            ("second", JsValue::Number(fields.second as f64)),
            ("millisecond", JsValue::Number(fields.millisecond as f64)),
            (
                "microsecond",
                JsValue::Number(
                    local_epoch_nanoseconds
                        .rem_euclid(1_000_000)
                        .div_euclid(1_000) as f64,
                ),
            ),
            (
                "nanosecond",
                JsValue::Number(local_epoch_nanoseconds.rem_euclid(1_000) as f64),
            ),
            ("dayOfWeek", JsValue::Number(day_of_week as f64)),
            ("dayOfYear", JsValue::Number(day_of_year as f64)),
            ("weekOfYear", JsValue::Number(week_of_year as f64)),
            ("yearOfWeek", JsValue::Number(year_of_week as f64)),
            ("daysInWeek", JsValue::Number(7.0)),
            (
                "daysInMonth",
                JsValue::Number(month_day_count(fields.year, fields.month) as f64),
            ),
            (
                "daysInYear",
                JsValue::Number(if leap { 366.0 } else { 365.0 }),
            ),
            ("monthsInYear", JsValue::Number(12.0)),
            ("hoursInDay", JsValue::Number(24.0)),
            ("inLeapYear", JsValue::Boolean(leap)),
        ],
    )
}

fn temporal_zoned_date_time_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.ZonedDateTime prototype missing"))?;
    let epoch_nanoseconds_value = arg_or_undefined(arguments, 0);
    let epoch_nanoseconds_i128 = epoch_nanoseconds_to_i128(&epoch_nanoseconds_value)?;
    if !is_valid_instant_ns(epoch_nanoseconds_i128) {
        return Err(VmError::range("invalid Temporal.ZonedDateTime"));
    }
    let epoch_nanoseconds = epoch_nanoseconds_i128 as f64;
    let time_zone_id = if matches!(arguments.get(1), None | Some(JsValue::Undefined)) {
        return Err(VmError::type_error(
            "Temporal.ZonedDateTime requires a time zone",
        ));
    } else {
        vm.to_string_coerce(arg_or_undefined(arguments, 1), context)?
    };
    let calendar_id = if matches!(arguments.get(2), None | Some(JsValue::Undefined)) {
        "iso8601".into()
    } else {
        vm.to_string_coerce(arg_or_undefined(arguments, 2), context)?
            .to_ascii_lowercase()
    };
    create_zoned_date_time(
        context,
        prototype,
        epoch_nanoseconds_value,
        epoch_nanoseconds,
        time_zone_id,
        calendar_id,
    )
}

fn parse_zoned_date_time(text: &str) -> Option<(i128, String)> {
    let time_zone_id = text
        .rsplit_once('[')
        .and_then(|(_, zone)| zone.strip_suffix(']'))
        .unwrap_or("UTC")
        .to_string();
    let without_annotation = text.split('[').next().unwrap_or(text);
    if let Some(epoch_nanoseconds) = parse_instant_string(without_annotation) {
        return Some((epoch_nanoseconds, time_zone_id));
    }
    let (year, month, day) = parse_plain_date(without_annotation)?;
    let time = if without_annotation.contains('T') {
        parse_plain_time(
            without_annotation
                .split_once('T')
                .map(|(_, time)| time)
                .unwrap_or(""),
        )?
    } else {
        PlainTimeValues::default()
    };
    validate_plain_date(year, month, day).ok()?;
    validate_plain_time(time).ok()?;
    Some((
        epoch_nanoseconds_i128_from_plain_parts(year, month, day, time),
        time_zone_id,
    ))
}

fn temporal_zoned_date_time_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (epoch_nanoseconds, epoch_nanoseconds_value, time_zone_id, calendar_id) = match item {
        JsValue::String(text) => {
            let (exact_epoch_nanoseconds, time_zone_id) = parse_zoned_date_time(&text)
                .ok_or_else(|| VmError::range("invalid Temporal.ZonedDateTime"))?;
            (
                exact_epoch_nanoseconds as f64,
                JsValue::BigInt(bigint::from_i128(exact_epoch_nanoseconds)),
                time_zone_id,
                "iso8601".into(),
            )
        }
        value => {
            let object = context.require_object(&value, "Temporal.ZonedDateTime.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("ZonedDateTime") {
                (
                    temporal_number_slot(context, object, "epochNanosecondsNumber"),
                    own_data_value(context, object, "epochNanoseconds")
                        .unwrap_or_else(|| JsValue::BigInt(bigint::from_i64(0))),
                    own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else {
                let time_zone = temporal_get_property(vm, context, object, "timeZone")?;
                let time_zone_id = if matches!(time_zone, JsValue::Undefined) {
                    "UTC".into()
                } else {
                    vm.to_string_coerce(time_zone, context)?
                };
                let calendar_id = temporal_calendar_id_from_object(vm, context, object)?;
                let year = temporal_object_number(vm, context, object, "year")?;
                let month = temporal_required_month_from_object(vm, context, object)?;
                let day = temporal_object_number(vm, context, object, "day")?;
                let time = PlainTimeValues {
                    hour: temporal_object_number(vm, context, object, "hour")?,
                    minute: temporal_object_number(vm, context, object, "minute")?,
                    second: temporal_object_number(vm, context, object, "second")?,
                    millisecond: temporal_object_number(vm, context, object, "millisecond")?,
                    microsecond: temporal_object_number(vm, context, object, "microsecond")?,
                    nanosecond: temporal_object_number(vm, context, object, "nanosecond")?,
                };
                validate_plain_date(year, month, day)?;
                validate_plain_time(time)?;
                let exact_epoch_nanoseconds =
                    epoch_nanoseconds_i128_from_plain_parts(year, month, day, time)
                        - parse_time_zone_offset_ns(&time_zone_id).unwrap_or(0);
                let epoch_nanoseconds = exact_epoch_nanoseconds as f64;
                (
                    epoch_nanoseconds,
                    JsValue::BigInt(bigint::from_i128(exact_epoch_nanoseconds)),
                    time_zone_id,
                    calendar_id,
                )
            }
        }
    };
    create_zoned_date_time(
        context,
        prototype,
        epoch_nanoseconds_value,
        epoch_nanoseconds,
        time_zone_id,
        calendar_id,
    )
}

fn zoned_date_time_epoch_ns(context: &NativeContext, object: ObjectId) -> i128 {
    match own_data_value(context, object, "epochNanoseconds") {
        Some(JsValue::BigInt(value)) => bigint_to_i128_saturating(&value),
        Some(JsValue::Number(value)) => value as i128,
        _ => temporal_number_slot(context, object, "epochNanosecondsNumber") as i128,
    }
}

fn temporal_zoned_date_time_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = temporal_zoned_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let right = temporal_zoned_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.get(1).cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let left = zoned_date_time_epoch_ns(context, context.value_object(&left).unwrap());
    let right = zoned_date_time_epoch_ns(context, context.value_object(&right).unwrap());
    Ok(JsValue::Number(if left < right {
        -1.0
    } else if left > right {
        1.0
    } else {
        0.0
    }))
}

fn temporal_zoned_date_time_epoch_nanoseconds(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    Ok(own_data_value(context, object, "epochNanoseconds")
        .unwrap_or_else(|| JsValue::BigInt(bigint::from_i64(0))))
}

fn temporal_zoned_date_time_to_instant(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let prototype = temporal_instant_constructor_prototype(context)?;
    create_instant_from_epoch_ns(
        context,
        prototype,
        zoned_date_time_epoch_ns(context, object),
    )
}

fn temporal_zoned_date_time_to_plain_date_time(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        PlainTimeValues {
            hour: temporal_number_slot(context, object, "hour"),
            minute: temporal_number_slot(context, object, "minute"),
            second: temporal_number_slot(context, object, "second"),
            millisecond: temporal_number_slot(context, object, "millisecond"),
            microsecond: temporal_number_slot(context, object, "microsecond"),
            nanosecond: temporal_number_slot(context, object, "nanosecond"),
        },
    )
}

fn temporal_zoned_date_time_to_plain_date(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
    )
}

fn temporal_zoned_date_time_to_plain_time(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    create_plain_time(
        context,
        prototype,
        PlainTimeValues {
            hour: temporal_number_slot(context, object, "hour"),
            minute: temporal_number_slot(context, object, "minute"),
            second: temporal_number_slot(context, object, "second"),
            millisecond: temporal_number_slot(context, object, "millisecond"),
            microsecond: temporal_number_slot(context, object, "microsecond"),
            nanosecond: temporal_number_slot(context, object, "nanosecond"),
        },
    )
}

fn temporal_zoned_date_time_equals(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let other = temporal_zoned_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    Ok(JsValue::Boolean(
        zoned_date_time_epoch_ns(context, this_object)
            == zoned_date_time_epoch_ns(context, other_object)
            && own_string(context, this_object, "timeZoneId")
                == own_string(context, other_object, "timeZoneId")
            && own_string(context, this_object, "calendarId")
                == own_string(context, other_object, "calendarId"),
    ))
}

fn create_zoned_date_time_from_parts(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
    time: PlainTimeValues,
    time_zone_id: String,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    let offset_nanoseconds = parse_time_zone_offset_ns(&time_zone_id).unwrap_or(0);
    let exact_epoch_nanoseconds =
        epoch_nanoseconds_i128_from_plain_parts(year, month, day, time) - offset_nanoseconds;
    let epoch_nanoseconds = exact_epoch_nanoseconds as f64;
    create_zoned_date_time(
        context,
        prototype,
        JsValue::BigInt(bigint::from_i128(exact_epoch_nanoseconds)),
        epoch_nanoseconds,
        time_zone_id,
        calendar_id,
    )
}

fn temporal_zoned_date_time_additive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: f64,
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let duration = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (mut year, mut month, mut day) = apply_duration_to_date(
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        duration,
        sign,
        reject_overflow,
    )?;
    let time_ns = time_nanoseconds(plain_date_time_values(context, object))
        + sign * duration_time_nanoseconds(duration);
    let (extra_days, time) = plain_time_from_nanoseconds(time_ns);
    if extra_days != 0 {
        let day_number = days_from_civil(year as i32, month as u32, day as u32)
            .checked_add(extra_days)
            .filter(|value| value.unsigned_abs() <= 100_000_000)
            .ok_or_else(|| VmError::range("Temporal.ZonedDateTime is out of range"))?;
        let fields = civil_from_days(day_number);
        year = fields.0 as f64;
        month = fields.1 as f64;
        day = fields.2 as f64;
    }
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time_from_parts(
        context,
        prototype,
        year,
        month,
        day,
        time,
        own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_zoned_date_time_add(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_zoned_date_time_additive(vm, context, this_value, arguments, 1.0)
}

fn temporal_zoned_date_time_subtract(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_zoned_date_time_additive(vm, context, this_value, arguments, -1.0)
}

fn temporal_zoned_date_time_round(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let (unit, increment, mode) = plain_date_time_round_options(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let quantum = temporal_unit_nanoseconds(&unit) * increment as i128;
    let rounded = round_i128(zoned_date_time_epoch_ns(context, object), quantum, mode);
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        JsValue::BigInt(bigint::from_i128(rounded)),
        rounded as f64,
        own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_zoned_date_time_difference(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    sign: i128,
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let other = temporal_zoned_date_time_from(
        vm,
        context,
        JsValue::Undefined,
        &[arguments.first().cloned().unwrap_or(JsValue::Undefined)],
    )?;
    let other_object = context.value_object(&other).unwrap();
    if own_string(context, this_object, "calendarId")
        != own_string(context, other_object, "calendarId")
    {
        return Err(VmError::range("Temporal calendars must match"));
    }
    let options = plain_date_time_difference_options(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
        "day",
        true,
    )?;
    if matches!(
        options.largest_unit.as_str(),
        "year" | "month" | "week" | "day"
    ) && own_string(context, this_object, "timeZoneId")
        != own_string(context, other_object, "timeZoneId")
    {
        return Err(VmError::range("Temporal time zones must match"));
    }
    let delta = sign
        * (zoned_date_time_epoch_ns(context, other_object)
            - zoned_date_time_epoch_ns(context, this_object));
    let quantum = temporal_unit_nanoseconds(&options.smallest_unit) * options.increment as i128;
    let rounded = round_signed_i128(delta, quantum, options.mode);
    create_duration_from_nanoseconds(context, rounded, &options.largest_unit)
}

fn temporal_zoned_date_time_until(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_zoned_date_time_difference(vm, context, this_value, arguments, 1)
}

fn temporal_zoned_date_time_since(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_zoned_date_time_difference(vm, context, this_value, arguments, -1)
}

fn temporal_zoned_date_time_with(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let this_object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let replacement = context.require_object(
        &arguments.first().cloned().unwrap_or(JsValue::Undefined),
        "Temporal.ZonedDateTime.prototype.with",
    )?;
    reject_temporal_with_metadata(vm, context, replacement)?;
    let year = temporal_date_replacement(
        vm,
        context,
        replacement,
        "year",
        temporal_number_slot(context, this_object, "year"),
    )?;
    let month = temporal_month_from_object(
        vm,
        context,
        replacement,
        temporal_number_slot(context, this_object, "month"),
    )?;
    let day = temporal_date_replacement(
        vm,
        context,
        replacement,
        "day",
        temporal_number_slot(context, this_object, "day"),
    )?;
    let mut time = PlainTimeValues {
        hour: temporal_date_replacement(
            vm,
            context,
            replacement,
            "hour",
            temporal_number_slot(context, this_object, "hour"),
        )?,
        minute: temporal_date_replacement(
            vm,
            context,
            replacement,
            "minute",
            temporal_number_slot(context, this_object, "minute"),
        )?,
        second: temporal_date_replacement(
            vm,
            context,
            replacement,
            "second",
            temporal_number_slot(context, this_object, "second"),
        )?,
        millisecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "millisecond",
            temporal_number_slot(context, this_object, "millisecond"),
        )?,
        microsecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "microsecond",
            temporal_number_slot(context, this_object, "microsecond"),
        )?,
        nanosecond: temporal_date_replacement(
            vm,
            context,
            replacement,
            "nanosecond",
            temporal_number_slot(context, this_object, "nanosecond"),
        )?,
    };
    let replacement_offset = temporal_get_property(vm, context, replacement, "offset")?;
    if !matches!(replacement_offset, JsValue::Undefined) {
        let JsValue::String(offset) = replacement_offset else {
            return Err(VmError::type_error(
                "Temporal offset property must be a string",
            ));
        };
        if parse_time_zone_offset_ns(&offset).is_none() {
            return Err(VmError::range("invalid Temporal offset string"));
        }
    }
    require_temporal_with_field(
        vm,
        context,
        replacement,
        &[
            "day",
            "hour",
            "microsecond",
            "millisecond",
            "minute",
            "month",
            "monthCode",
            "nanosecond",
            "offset",
            "second",
            "year",
        ],
    )?;
    let options = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let options_object = if matches!(options, JsValue::Undefined) {
        None
    } else {
        Some(context.require_object(&options, "Temporal.ZonedDateTime.prototype.with options")?)
    };
    if let Some(object) = options_object {
        if let Some(value) = option_string(vm, context, object, "disambiguation")? {
            if !matches!(
                value.as_str(),
                "compatible" | "earlier" | "later" | "reject"
            ) {
                return Err(VmError::range("invalid Temporal disambiguation option"));
            }
        }
        if let Some(value) = option_string(vm, context, object, "offset")? {
            if !matches!(value.as_str(), "use" | "prefer" | "ignore" | "reject") {
                return Err(VmError::range("invalid Temporal offset option"));
            }
        }
    }
    let reject_overflow = temporal_overflow_reject(vm, context, options)?;
    let month = if reject_overflow {
        month
    } else {
        month.clamp(1.0, 12.0)
    };
    let day = if reject_overflow {
        day
    } else {
        day.clamp(1.0, month_day_count(year as i32, month as u32) as f64)
    };
    if !reject_overflow {
        time.hour = time.hour.clamp(0.0, 23.0);
        time.minute = time.minute.clamp(0.0, 59.0);
        time.second = time.second.clamp(0.0, 59.0);
        time.millisecond = time.millisecond.clamp(0.0, 999.0);
        time.microsecond = time.microsecond.clamp(0.0, 999.0);
        time.nanosecond = time.nanosecond.clamp(0.0, 999.0);
    }
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time_from_parts(
        context,
        prototype,
        year,
        month,
        day,
        time,
        own_string(context, this_object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_zoned_date_time_with_calendar(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.first())?;
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        own_data_value(context, object, "epochNanoseconds")
            .unwrap_or_else(|| JsValue::BigInt(bigint::from_i64(0))),
        temporal_number_slot(context, object, "epochNanosecondsNumber"),
        own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
        calendar_id,
    )
}

fn temporal_zoned_date_time_with_plain_time(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let time = plain_time_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time_from_parts(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        time,
        own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_zoned_date_time_with_time_zone(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let time_zone_value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if matches!(time_zone_value, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal.ZonedDateTime.prototype.withTimeZone requires a time zone",
        ));
    }
    let time_zone_id = vm.to_string_coerce(time_zone_value, context)?;
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        own_data_value(context, object, "epochNanoseconds")
            .unwrap_or_else(|| JsValue::BigInt(bigint::from_i64(0))),
        temporal_number_slot(context, object, "epochNanosecondsNumber"),
        time_zone_id,
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_zoned_date_time_start_of_day(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time_from_parts(
        context,
        prototype,
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        PlainTimeValues::default(),
        own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
        own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
    )
}

fn temporal_zoned_date_time_get_time_zone_transition(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let option = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    if matches!(option, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal.ZonedDateTime.prototype.getTimeZoneTransition requires a direction",
        ));
    }
    let direction_value = if let Some(object) = context.value_object(&option) {
        temporal_get_property(vm, context, object, "direction")?
    } else {
        option
    };
    let direction = vm.to_string_coerce(direction_value, context)?;
    match direction.as_str() {
        "next" | "previous" => Ok(JsValue::Null),
        _ => Err(VmError::range(
            "invalid Temporal time zone transition direction",
        )),
    }
}

fn temporal_zoned_date_time_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let option_value = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let calendar_name = temporal_calendar_name_option(vm, context, option_value.clone())?;
    let options = temporal_string_options(vm, context, option_value.clone())?;
    let (offset_option, time_zone_name) = if matches!(option_value, JsValue::Undefined) {
        ("auto".to_string(), "auto".to_string())
    } else {
        let option_object =
            context.require_object(&option_value, "Temporal.ZonedDateTime toString options")?;
        let offset =
            option_string(vm, context, option_object, "offset")?.unwrap_or_else(|| "auto".into());
        let time_zone = option_string(vm, context, option_object, "timeZoneName")?
            .unwrap_or_else(|| "auto".into());
        (offset, time_zone)
    };
    if !matches!(offset_option.as_str(), "auto" | "never") {
        return Err(VmError::range("invalid Temporal offset display option"));
    }
    if !matches!(time_zone_name.as_str(), "auto" | "never" | "critical") {
        return Err(VmError::range("invalid Temporal timeZoneName"));
    }
    let rounded = round_i128(
        zoned_date_time_epoch_ns(context, object),
        options.quantum,
        options.mode,
    );
    let day_number = i64::try_from(rounded.div_euclid(NS_PER_DAY_I128))
        .map_err(|_| VmError::range("Temporal.ZonedDateTime is out of range"))?;
    let (year, month, day) = civil_from_days(day_number);
    let date_time = format!(
        "{}-{}-{}T{}",
        iso_year(year),
        two_digit(month),
        two_digit(day),
        format_plain_time_precision(
            plain_time_from_nanoseconds_i128(rounded.rem_euclid(NS_PER_DAY_I128)),
            options.precision,
            options.minute_only,
        )
    );
    let zone = own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into());
    let offset = if offset_option == "never" {
        ""
    } else {
        "+00:00"
    };
    let zone_annotation = match time_zone_name.as_str() {
        "never" => String::new(),
        "critical" => format!("[!{zone}]"),
        _ => format!("[{zone}]"),
    };
    let calendar_id = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let calendar_annotation = match calendar_name.as_str() {
        "always" => format!("[u-ca={calendar_id}]"),
        "critical" => format!("[!u-ca={calendar_id}]"),
        "auto" if calendar_id != "iso8601" => format!("[u-ca={calendar_id}]"),
        _ => String::new(),
    };
    Ok(JsValue::String(format!(
        "{date_time}{offset}{zone_annotation}{calendar_annotation}"
    )))
}

fn install_temporal_now(context: &mut NativeContext, temporal: ObjectId) -> Result<(), VmError> {
    let now = new_ordinary_object(context, context.object_prototype())?;
    define_method(context, now, "instant", 0, temporal_now_instant)?;
    define_method(
        context,
        now,
        "zonedDateTimeISO",
        0,
        temporal_now_zoned_date_time_iso,
    )?;
    define_method(context, now, "plainDateISO", 0, temporal_now_plain_date_iso)?;
    define_method(context, now, "plainTimeISO", 0, temporal_now_plain_time_iso)?;
    define_method(
        context,
        now,
        "plainDateTimeISO",
        0,
        temporal_now_plain_date_time_iso,
    )?;
    define_method(context, now, "timeZoneId", 0, temporal_now_time_zone_id)?;
    let tag = context.well_known_symbols().to_string_tag;
    context.define_symbol_own_property(
        now,
        tag,
        readonly_configurable_descriptor(JsValue::String("Temporal.Now".into())),
    )?;
    context.define_own_property(
        temporal,
        "Now".into(),
        method_descriptor(JsValue::Object(now)),
    )?;
    Ok(())
}

fn temporal_now_instant(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_instant_constructor_prototype(context)?;
    create_instant(context, prototype, current_time_ms())
}

fn temporal_now_zoned_date_time_iso(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let time_zone_id = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined => "UTC".into(),
        value => vm.to_string_coerce(value, context)?,
    };
    let epoch_ms = current_time_ms();
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        JsValue::BigInt(bigint::from_i128(
            (epoch_ms as i128).saturating_mul(1_000_000),
        )),
        epoch_ms * 1_000_000.0,
        time_zone_id,
        "iso8601".into(),
    )
}

fn temporal_now_fields() -> DateFields {
    decompose_time(current_time_ms()).unwrap()
}

fn temporal_now_plain_date_iso(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let fields = temporal_now_fields();
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date(
        context,
        prototype,
        fields.year as f64,
        fields.month as f64,
        fields.day as f64,
    )
}

fn temporal_now_plain_time_iso(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let fields = temporal_now_fields();
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    create_plain_time(
        context,
        prototype,
        PlainTimeValues {
            hour: fields.hour as f64,
            minute: fields.minute as f64,
            second: fields.second as f64,
            millisecond: fields.millisecond as f64,
            microsecond: 0.0,
            nanosecond: 0.0,
        },
    )
}

fn temporal_now_plain_date_time_iso(
    _vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let fields = temporal_now_fields();
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time(
        context,
        prototype,
        fields.year as f64,
        fields.month as f64,
        fields.day as f64,
        PlainTimeValues {
            hour: fields.hour as f64,
            minute: fields.minute as f64,
            second: fields.second as f64,
            millisecond: fields.millisecond as f64,
            microsecond: 0.0,
            nanosecond: 0.0,
        },
    )
}

fn temporal_now_time_zone_id(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String("UTC".into()))
}
