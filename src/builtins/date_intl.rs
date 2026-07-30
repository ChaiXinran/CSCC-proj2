//! Date / Intl / Temporal built-ins.
//!
//! The implementation is intentionally a deterministic UTC-oriented subset.
//! It installs real JS-visible constructors, prototypes, descriptors, and a
//! small core of algorithms without trying to replace ICU or full Temporal.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    intl::{
        CalendarBackend, CalendarDate, CalendarDateFields, CalendarDuration, CalendarLargestUnit,
        CalendarOverflow, Icu4xCalendarBackend, IsoDate, JiffTimeZoneProvider, LocalDateTime,
        TimeZoneDisambiguation, TimeZoneProvider,
    },
    runtime::{
        JsObject, JsValue, NativeCall, NativeConstruct, NativeContext, ObjectId, PreferredType,
        PrimitiveValue, PropertyDescriptor, PropertyKind, abstract_ops, bigint,
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
    let days = days.clamp(-100_000_001, 100_000_000);
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

fn calendar_iso_year(calendar: &str, year: i32) -> i32 {
    match calendar {
        "buddhist" => year.saturating_sub(543),
        "roc" => year.saturating_add(1911),
        _ => year,
    }
}

fn calendar_year_from_iso(calendar: &str, year: i32) -> i32 {
    match calendar {
        "buddhist" => year.saturating_add(543),
        "roc" => year.saturating_sub(1911),
        _ => year,
    }
}

fn calendar_days_from_civil(calendar: &str, year: i32, month: u32, day: u32) -> i64 {
    days_from_civil(calendar_iso_year(calendar, year), month, day)
}

fn calendar_civil_from_days(calendar: &str, days: i64) -> (i32, u32, u32) {
    let (year, month, day) = civil_from_days(days);
    (calendar_year_from_iso(calendar, year), month, day)
}

fn temporal_day_number_within_range(day_number: i64) -> bool {
    (-100_000_001..=100_000_000).contains(&day_number)
}

fn calendar_month_day_count(calendar: &str, year: i32, month: u32) -> u32 {
    let leap = calendar_is_leap_year(calendar, year);
    match calendar {
        "coptic" | "ethiopic" | "ethioaa" => match month {
            1..=12 => 30,
            13 => {
                if leap {
                    6
                } else {
                    5
                }
            }
            _ => 0,
        },
        "islamic" | "islamic-civil" | "islamic-rgsa" | "islamic-tbla" | "islamic-umalqura" => {
            match month {
                1..=11 => {
                    if month % 2 == 1 {
                        30
                    } else {
                        29
                    }
                }
                12 => {
                    if leap {
                        30
                    } else {
                        29
                    }
                }
                _ => 0,
            }
        }
        "persian" => match month {
            1..=6 => 31,
            7..=11 => 30,
            12 => {
                if leap {
                    30
                } else {
                    29
                }
            }
            _ => 0,
        },
        "indian" => match month {
            1 => {
                if leap {
                    31
                } else {
                    30
                }
            }
            2..=6 => 31,
            7..=12 => 30,
            _ => 0,
        },
        _ => month_day_count(calendar_iso_year(calendar, year), month),
    }
}

fn calendar_is_leap_year(calendar: &str, year: i32) -> bool {
    match calendar {
        "coptic" | "ethiopic" | "ethioaa" => year.rem_euclid(4) == 3,
        "hebrew" => (7 * year + 1).rem_euclid(19) < 7,
        "islamic" | "islamic-civil" | "islamic-rgsa" | "islamic-tbla" | "islamic-umalqura" => {
            (11 * year + 14).rem_euclid(30) < 11
        }
        "persian" => {
            let ep_base = year - if year >= 0 { 474 } else { 473 };
            let ep_year = 474 + ep_base.rem_euclid(2820);
            (ep_year * 682).rem_euclid(2816) < 682
        }
        "indian" => is_leap_year(year.saturating_add(78)),
        _ => is_leap_year(calendar_iso_year(calendar, year)),
    }
}

fn calendar_months_in_year(calendar: &str, year: i32) -> u32 {
    match calendar {
        "coptic" | "ethiopic" | "ethioaa" => 13,
        "hebrew" if calendar_is_leap_year(calendar, year) => 13,
        _ => 12,
    }
}

fn calendar_days_in_year(calendar: &str, year: i32) -> u32 {
    let months = calendar_months_in_year(calendar, year);
    (1..=months)
        .map(|month| calendar_month_day_count(calendar, year, month))
        .sum()
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

fn temporal_date_slots_from_iso(
    year: f64,
    month: f64,
    day: f64,
    calendar_id: String,
) -> Vec<(&'static str, JsValue)> {
    let iso_year = year.trunc() as i32;
    let iso_month = month.trunc() as u32;
    let iso_day = day.trunc() as u32;
    let day_number = days_from_civil(iso_year, iso_month, iso_day);
    let (week_of_year, year_of_week) = temporal_week_fields(iso_year, iso_month, iso_day);
    let converted = Icu4xCalendarBackend
        .date_from_iso(
            &calendar_id,
            IsoDate {
                year: iso_year,
                month: iso_month as u8,
                day: iso_day as u8,
            },
        )
        .ok();
    let calendar_year = converted.as_ref().map_or(iso_year, |date| date.year);
    let calendar_month = converted
        .as_ref()
        .map_or(iso_month as u8, |date| date.month);
    let calendar_day = converted.as_ref().map_or(iso_day as u8, |date| date.day);
    let month_code = converted
        .as_ref()
        .map_or_else(|| month_code(month), |date| date.month_code.clone());
    let day_of_year = converted.as_ref().map_or_else(
        || temporal_day_of_year(iso_year, iso_month, iso_day) as u16,
        |date| date.day_of_year,
    );
    let days_in_month = converted.as_ref().map_or_else(
        || month_day_count(iso_year, iso_month) as u8,
        |date| date.days_in_month,
    );
    let days_in_year = converted.as_ref().map_or_else(
        || if is_leap_year(iso_year) { 366 } else { 365 },
        |date| date.days_in_year,
    );
    let months_in_year = converted.as_ref().map_or(12, |date| date.months_in_year);
    let leap = converted.as_ref().is_some_and(|date| date.in_leap_year)
        || (converted.is_none() && is_leap_year(iso_year));
    vec![
        ("isoYear", JsValue::Number(iso_year as f64)),
        ("isoMonth", JsValue::Number(iso_month as f64)),
        ("isoDay", JsValue::Number(iso_day as f64)),
        ("year", JsValue::Number(calendar_year as f64)),
        ("month", JsValue::Number(calendar_month as f64)),
        ("monthCode", JsValue::String(month_code)),
        ("day", JsValue::Number(calendar_day as f64)),
        ("calendarId", JsValue::String(calendar_id.clone())),
        (
            "dayOfWeek",
            JsValue::Number(temporal_day_of_week_from_day_number(day_number) as f64),
        ),
        ("dayOfYear", JsValue::Number(day_of_year as f64)),
        ("weekOfYear", JsValue::Number(week_of_year as f64)),
        ("yearOfWeek", JsValue::Number(year_of_week as f64)),
        ("daysInWeek", JsValue::Number(7.0)),
        ("daysInMonth", JsValue::Number(days_in_month as f64)),
        ("daysInYear", JsValue::Number(days_in_year as f64)),
        ("monthsInYear", JsValue::Number(months_in_year as f64)),
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
    if rest.len() == 4 && rest.chars().all(|ch| ch.is_ascii_digit()) {
        return Some((
            year,
            parse_fixed_digits(&rest[..2], 2)?,
            parse_fixed_digits(&rest[2..], 2)?,
        ));
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

fn parse_legacy_date_string(input: &str) -> Option<f64> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let normalized = input.replace(',', "");
    let parts: Vec<&str> = normalized.split_whitespace().collect();
    let (month_text, day_text, year_text, time_text, zone_text) =
        if parts.get(1).is_some_and(|part| MONTHS.contains(part)) {
            (
                *parts.get(1)?,
                *parts.get(2)?,
                *parts.get(3)?,
                *parts.get(4)?,
                *parts.get(5)?,
            )
        } else {
            (
                *parts.get(2)?,
                *parts.get(1)?,
                *parts.get(3)?,
                *parts.get(4)?,
                *parts.get(5)?,
            )
        };
    let month = MONTHS.iter().position(|month| *month == month_text)? as u32 + 1;
    let day = day_text.parse::<u32>().ok()?;
    let year = year_text.parse::<i32>().ok()?;
    if !(1..=month_day_count(year, month)).contains(&day) {
        return None;
    }
    let mut time = time_text.split(':');
    let hour = time.next()?.parse::<u32>().ok()?;
    let minute = time.next()?.parse::<u32>().ok()?;
    let second = time.next()?.parse::<u32>().ok()?;
    if time.next().is_some() || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let offset_ms = if zone_text == "GMT" || zone_text == "UTC" {
        0_i64
    } else {
        let offset = zone_text.strip_prefix("GMT")?;
        if offset.len() != 5 || !offset.starts_with(['+', '-']) {
            return None;
        }
        let hours = offset[1..3].parse::<i64>().ok()?;
        let minutes = offset[3..5].parse::<i64>().ok()?;
        if hours > 23 || minutes > 59 {
            return None;
        }
        let sign = if offset.starts_with('-') { -1 } else { 1 };
        sign * (hours * 60 + minutes) * 60_000
    };
    let local_ms = make_date(
        days_from_civil(year, month, day) as f64,
        make_time(hour as f64, minute as f64, second as f64, 0.0),
    );
    Some(time_clip(local_ms - offset_ms as f64))
}

fn parse_date_string(input: &str) -> Option<f64> {
    parse_iso_date_string(input).or_else(|| parse_legacy_date_string(input))
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
    let mut calendar_critical = None;
    let mut rest = input;
    while let Some(start) = rest.find('[') {
        saw_annotation = true;
        body_end = body_end.min(input.len() - rest.len() + start);
        let after_start = &rest[start + 1..];
        let end = after_start.find(']')?;
        let raw_annotation = &after_start[..end];
        if raw_annotation.is_empty() {
            return None;
        }
        let critical = raw_annotation.starts_with('!');
        let annotation = raw_annotation.strip_prefix('!').unwrap_or(raw_annotation);
        if let Some((key, _value)) = annotation.split_once('=') {
            if key.chars().any(|ch| ch.is_ascii_uppercase()) || (critical && key != "u-ca") {
                return None;
            }
            if key == "u-ca" {
                if let Some(previous_critical) = calendar_critical {
                    if critical || previous_critical {
                        return None;
                    }
                } else {
                    calendar_critical = Some(critical);
                }
            }
        } else {
            if seen_time_zone {
                return None;
            }
            if offset_annotation_has_subminute_syntax(annotation) {
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

fn offset_annotation_has_subminute_syntax(annotation: &str) -> bool {
    let Some(body) = annotation
        .strip_prefix('+')
        .or_else(|| annotation.strip_prefix('-'))
    else {
        return false;
    };
    let head = body.split(['.', ',']).next().unwrap_or(body);
    head.chars().filter(|ch| ch.is_ascii_digit()).count() > 4
}

fn validate_iso_calendar_annotations(input: &str) -> Option<&str> {
    let mut body_end = input.len();
    let mut saw_annotation = false;
    let mut seen_time_zone = false;
    let mut calendar_critical = None;
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
                if let Some(previous_critical) = calendar_critical {
                    if critical || previous_critical {
                        return None;
                    }
                    // A later non-critical calendar annotation is ignored by
                    // Temporal parsing; the first annotation wins.
                } else {
                    calendar_critical = Some(critical);
                    if !value.eq_ignore_ascii_case("iso8601") {
                        return None;
                    }
                }
            } else if critical {
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
        if rest.len() != 5 || rest.as_bytes().get(2).is_none_or(|ch| *ch != b'-') {
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
        parse_iso_time_zone_offset_ns(&input[offset_start..])?;
        &input[..offset_start]
    } else {
        input
    };
    let compact = time.replace(':', "").replace(',', ".");
    let (head, fraction) = compact
        .split_once('.')
        .map_or((compact.as_str(), ""), |(head, fraction)| (head, fraction));
    if !fraction.is_empty() && (head.len() != 6 || parse_fraction_to_ns(fraction).is_none()) {
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
    (hour <= 23 && minute <= 59 && second <= 60).then_some(())
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
            (
                &input[..index],
                parse_iso_time_zone_offset_ns(&input[index..])?,
            )
        } else if let Some(index) = input.get(1..).and_then(|rest| rest.rfind('-')) {
            let split = index + 1;
            (
                &input[..split],
                parse_iso_time_zone_offset_ns(&input[split..])?,
            )
        } else {
            return None;
        };
    if time.is_empty() {
        return None;
    }
    let normalized = time.replace(',', ".");
    let (head, fraction_text) = normalized
        .split_once('.')
        .map_or((normalized.as_str(), ""), |(head, fraction)| {
            (head, fraction)
        });
    let colon_count = head.matches(':').count();
    let compact = head.replace(':', "");
    if (colon_count > 0 && !matches!(colon_count, 1 | 2))
        || !matches!(compact.len(), 2 | 4 | 6)
        || (colon_count > 0 && compact.len() != (colon_count + 1) * 2)
        || !compact.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let hour = parse_fixed_digits(&compact[..2], 2)?;
    let minute = if compact.len() >= 4 {
        parse_fixed_digits(&compact[2..4], 2)?
    } else {
        0
    };
    let mut second = if compact.len() == 6 {
        parse_fixed_digits(&compact[4..6], 2)?
    } else {
        0
    };
    if hour > 23 || minute > 59 {
        return None;
    }
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
    parse_time_zone_offset_ns_impl(input, false)
}

fn parse_iso_time_zone_offset_ns(input: &str) -> Option<i128> {
    parse_time_zone_offset_ns_impl(input, true)
}

fn parse_time_zone_offset_ns_impl(input: &str, allow_subminute: bool) -> Option<i128> {
    let sign = if input.starts_with('+') {
        1
    } else if input.starts_with('-') {
        -1
    } else {
        return None;
    };
    let normalized = input[1..].replace(',', ".");
    let (body, fraction_text) = normalized
        .split_once('.')
        .map_or((normalized.as_str(), ""), |(body, fraction)| {
            (body, fraction)
        });
    let colon_count = body.matches(':').count();
    let compact = body.replace(':', "");
    if (colon_count > 0 && !matches!(colon_count, 1 | 2))
        || !matches!(compact.len(), 2 | 4 | 6)
        || (colon_count > 0 && compact.len() != (colon_count + 1) * 2)
        || (!fraction_text.is_empty() && compact.len() != 6)
        || !compact.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let hour = parse_fixed_digits(&compact[..2], 2)?;
    let minute = if compact.len() >= 4 {
        parse_fixed_digits(&compact[2..4], 2)?
    } else {
        0
    };
    let second = if compact.len() == 6 {
        parse_fixed_digits(&compact[4..6], 2)?
    } else {
        0
    };
    let fraction_ns = if fraction_text.is_empty() {
        0
    } else {
        parse_fraction_to_ns(fraction_text)?
    };
    if hour > 23
        || minute > 59
        || second > 59
        || (!allow_subminute && (second != 0 || fraction_ns != 0))
    {
        return None;
    }
    Some(
        sign * (hour as i128 * NS_PER_HOUR_I128
            + minute as i128 * NS_PER_MINUTE_I128
            + second as i128 * NS_PER_SECOND_I128
            + fraction_ns),
    )
}

fn parse_instant_string(input: &str) -> Option<i128> {
    let input = input.trim();
    if input.is_empty() || input.contains('\u{2212}') {
        return None;
    }
    let body = validate_temporal_annotations(input)?;
    let separator = body
        .find('T')
        .or_else(|| body.find('t'))
        .or_else(|| body.find(' '))?;
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
                JsValue::String(text) => parse_date_string(&text).unwrap_or(f64::NAN),
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
        parse_date_string(&text).unwrap_or(f64::NAN),
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
    if !abstract_ops::is_callable(&to_iso_string) {
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
        for argument in arguments.iter().take(3) {
            vm.to_number(argument.clone(), context)?;
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
        for argument in arguments.iter().take(4) {
            vm.to_number(argument.clone(), context)?;
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
        for argument in arguments.iter().take(2) {
            vm.to_number(argument.clone(), context)?;
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
        if !abstract_ops::is_callable(&method) {
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
        &[
            (
                "formatToParts",
                1,
                intl_number_format_format_to_parts as NativeCall,
            ),
            (
                "formatRange",
                2,
                intl_number_format_format_range as NativeCall,
            ),
            (
                "formatRangeToParts",
                2,
                intl_number_format_format_range_to_parts as NativeCall,
            ),
        ],
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
        ("script", "get script", intl_locale_script_get as NativeCall),
        ("region", "get region", intl_locale_region_get as NativeCall),
        (
            "variants",
            "get variants",
            intl_locale_variants_get as NativeCall,
        ),
        (
            "calendar",
            "get calendar",
            intl_locale_calendar_get as NativeCall,
        ),
        (
            "collation",
            "get collation",
            intl_locale_collation_get as NativeCall,
        ),
        (
            "hourCycle",
            "get hourCycle",
            intl_locale_hour_cycle_get as NativeCall,
        ),
        (
            "caseFirst",
            "get caseFirst",
            intl_locale_case_first_get as NativeCall,
        ),
        (
            "numeric",
            "get numeric",
            intl_locale_numeric_get as NativeCall,
        ),
        (
            "numberingSystem",
            "get numberingSystem",
            intl_locale_numbering_system_get as NativeCall,
        ),
        (
            "firstDayOfWeek",
            "get firstDayOfWeek",
            intl_locale_first_day_of_week_get as NativeCall,
        ),
    ] {
        define_accessor(context, prototype, name, getter, call)?;
    }
    define_method(context, prototype, "toString", 0, intl_locale_to_string)?;
    define_method(context, prototype, "maximize", 0, intl_locale_maximize)?;
    define_method(context, prototype, "minimize", 0, intl_locale_minimize)?;
    define_method(
        context,
        prototype,
        "getCalendars",
        0,
        intl_locale_get_calendars,
    )?;
    define_method(
        context,
        prototype,
        "getCollations",
        0,
        intl_locale_get_collations,
    )?;
    define_method(
        context,
        prototype,
        "getHourCycles",
        0,
        intl_locale_get_hour_cycles,
    )?;
    define_method(
        context,
        prototype,
        "getNumberingSystems",
        0,
        intl_locale_get_numbering_systems,
    )?;
    define_method(
        context,
        prototype,
        "getTextInfo",
        0,
        intl_locale_get_text_info,
    )?;
    define_method(
        context,
        prototype,
        "getTimeZones",
        0,
        intl_locale_get_time_zones,
    )?;
    define_method(
        context,
        prototype,
        "getWeekInfo",
        0,
        intl_locale_get_week_info,
    )?;
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
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Err(VmError::type_error("Intl.Locale requires 'new'"))
}

fn intl_locale_construct(
    _vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .or_else(|| context.object_prototype())
        .ok_or_else(|| VmError::runtime("Intl.Locale prototype missing"))?;
    let object = new_ordinary_object(context, Some(prototype))?;
    let tag = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let locale = match tag {
        JsValue::String(tag) => {
            if !is_structurally_valid_locale(&tag) {
                return Err(VmError::range("invalid Intl.Locale language tag"));
            }
            canonicalize_locale(&tag)
        }
        JsValue::Object(object)
            if own_string(context, object, INTL_KIND).as_deref() == Some("Locale") =>
        {
            own_string(context, object, INTL_LOCALE).unwrap_or_else(|| "und".into())
        }
        _ => {
            return Err(VmError::type_error(
                "Intl.Locale tag must be a string or Locale",
            ));
        }
    };
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
            if own_string(context, object, INTL_KIND).as_deref() == Some("Locale") {
                return Ok(vec![
                    own_string(context, object, INTL_LOCALE).unwrap_or_else(|| "und".into()),
                ]);
            }
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
                    if let Some(locale_object) = context.value_object(&value)
                        && own_string(context, locale_object, INTL_KIND).as_deref()
                            == Some("Locale")
                    {
                        locales.push(
                            own_string(context, locale_object, INTL_LOCALE)
                                .unwrap_or_else(|| "und".into()),
                        );
                    } else {
                        locales.push(canonicalize_locale(&vm.to_string_coerce(value, context)?));
                    }
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
    let lower = trimmed.to_ascii_lowercase();
    if let Some(replacement) = match lower.as_str() {
        "art-lojban" => Some("jbo"),
        "cel-gaulish" => Some("xtg"),
        "zh-guoyu" => Some("zh"),
        "zh-hakka" => Some("hak"),
        "zh-xiang" => Some("hsn"),
        "hy-arevela" => Some("hy"),
        "hy-arevmda" => Some("hyw"),
        _ => None,
    } {
        return replacement.into();
    }
    let parts: Vec<&str> = trimmed.split('-').collect();
    let mut output = Vec::new();
    let language = match parts[0].to_ascii_lowercase().as_str() {
        "iw" => "he".into(),
        "in" => "id".into(),
        "ji" => "yi".into(),
        "mo" => "ro".into(),
        "aar" => "aa".into(),
        "heb" => "he".into(),
        "ces" => "cs".into(),
        language => language.into(),
    };
    output.push(language);
    let mut index = 1;
    if parts.get(index).is_some_and(|part| is_script_subtag(part)) {
        let script: String = match parts[index].to_ascii_lowercase().as_str() {
            "qaai" => "zinh".into(),
            script => script.into(),
        };
        output.push(format!(
            "{}{}",
            script[..1].to_ascii_uppercase(),
            &script[1..]
        ));
        index += 1;
    }
    if parts.get(index).is_some_and(|part| is_region_subtag(part)) {
        let region = parts[index].to_ascii_uppercase();
        let region = match region.as_str() {
            "BU" => "MM",
            "DD" => "DE",
            "FX" => "FR",
            "TP" => "TL",
            "YD" => "YE",
            "ZR" => "CD",
            "SU" if output.get(1).is_some_and(|script| script == "Armn") => "AM",
            _ => region.as_str(),
        };
        output.push(region.into());
        index += 1;
    }
    let mut variants = Vec::new();
    while parts.get(index).is_some_and(|part| is_variant_subtag(part)) {
        variants.push(parts[index].to_ascii_lowercase());
        index += 1;
    }
    variants.sort();
    output.extend(variants);
    let mut extensions: Vec<(String, Vec<String>)> = Vec::new();
    while index < parts.len() {
        let singleton = parts[index].to_ascii_lowercase();
        index += 1;
        let mut extension = Vec::new();
        if singleton == "x" {
            extension.extend(parts[index..].iter().map(|part| part.to_ascii_lowercase()));
            extensions.push((singleton, extension));
            break;
        }
        if singleton == "u" {
            let mut attributes = Vec::new();
            let mut keywords: Vec<(String, Vec<String>)> = Vec::new();
            let mut seen_keywords = std::collections::HashSet::new();
            while index < parts.len() && parts[index].len() > 2 {
                attributes.push(parts[index].to_ascii_lowercase());
                index += 1;
            }
            while index < parts.len() && parts[index].len() != 1 {
                let key = parts[index].to_ascii_lowercase();
                index += 1;
                let mut value = Vec::new();
                while index < parts.len() && parts[index].len() > 2 {
                    value.push(parts[index].to_ascii_lowercase());
                    index += 1;
                }
                if matches!(key.as_str(), "kf" | "kn")
                    && value.first().is_some_and(|value| value == "true")
                {
                    value.clear();
                }
                if key == "ca" && value.as_slice() == ["islamicc"] {
                    value = vec!["islamic".into(), "civil".into()];
                }
                if seen_keywords.insert(key.clone()) {
                    keywords.push((key, value));
                }
            }
            attributes.sort();
            extension.extend(attributes);
            keywords.sort_by(|left, right| left.0.cmp(&right.0));
            for (key, value) in keywords {
                extension.push(key);
                extension.extend(value);
            }
        } else {
            while index < parts.len() && parts[index].len() != 1 {
                extension.push(parts[index].to_ascii_lowercase());
                index += 1;
            }
        }
        extensions.push((singleton, extension));
    }
    extensions.sort_by(|left, right| match (left.0.as_str(), right.0.as_str()) {
        ("x", "x") => std::cmp::Ordering::Equal,
        ("x", _) => std::cmp::Ordering::Greater,
        (_, "x") => std::cmp::Ordering::Less,
        _ => left.0.cmp(&right.0),
    });
    for (singleton, extension) in extensions {
        output.push(singleton);
        output.extend(extension);
    }
    output.join("-")
}

fn is_structurally_valid_locale(locale: &str) -> bool {
    if locale.is_empty() || !locale.is_ascii() || locale.contains('_') {
        return false;
    }
    let parts: Vec<&str> = locale.split('-').collect();
    if parts.is_empty()
        || !(matches!(parts[0].len(), 2 | 3) || (5..=8).contains(&parts[0].len()))
        || !parts[0].chars().all(|ch| ch.is_ascii_alphabetic())
    {
        return false;
    }
    let mut seen_variants = std::collections::HashSet::new();
    let mut seen_singletons = std::collections::HashSet::new();
    let mut index = 1;
    if parts.get(index).is_some_and(|part| is_script_subtag(part)) {
        index += 1;
    }
    if parts.get(index).is_some_and(|part| is_region_subtag(part)) {
        index += 1;
    }
    while parts.get(index).is_some_and(|part| is_variant_subtag(part)) {
        if !seen_variants.insert(parts[index].to_ascii_lowercase()) {
            return false;
        }
        index += 1;
    }
    while index < parts.len() {
        if parts[index].len() != 1 || !parts[index].chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return false;
        }
        let singleton = parts[index].to_ascii_lowercase();
        if !seen_singletons.insert(singleton.clone()) {
            return false;
        }
        index += 1;
        let start = index;
        if singleton == "x" {
            while index < parts.len() {
                if !(1..=8).contains(&parts[index].len())
                    || !parts[index].chars().all(|ch| ch.is_ascii_alphanumeric())
                {
                    return false;
                }
                index += 1;
            }
        } else {
            while index < parts.len() && parts[index].len() != 1 {
                if !(2..=8).contains(&parts[index].len())
                    || !parts[index].chars().all(|ch| ch.is_ascii_alphanumeric())
                {
                    return false;
                }
                index += 1;
            }
        }
        if index == start {
            return false;
        }
        if singleton == "t" && has_duplicate_tlang_variants(&parts[start..index]) {
            return false;
        }
    }
    true
}

fn has_duplicate_tlang_variants(parts: &[&str]) -> bool {
    let Some(language) = parts.first() else {
        return false;
    };
    if !(matches!(language.len(), 2 | 3) || (5..=8).contains(&language.len()))
        || !language.chars().all(|ch| ch.is_ascii_alphabetic())
    {
        return false;
    }
    let mut index = 1;
    if parts.get(index).is_some_and(|part| is_script_subtag(part)) {
        index += 1;
    }
    if parts.get(index).is_some_and(|part| is_region_subtag(part)) {
        index += 1;
    }
    let mut variants = std::collections::HashSet::new();
    while parts.get(index).is_some_and(|part| is_variant_subtag(part)) {
        if !variants.insert(parts[index].to_ascii_lowercase()) {
            return true;
        }
        index += 1;
    }
    false
}

fn is_script_subtag(value: &str) -> bool {
    value.len() == 4 && value.chars().all(|ch| ch.is_ascii_alphabetic())
}

fn is_region_subtag(value: &str) -> bool {
    (value.len() == 2 && value.chars().all(|ch| ch.is_ascii_alphabetic()))
        || (value.len() == 3 && value.chars().all(|ch| ch.is_ascii_digit()))
}

fn is_variant_subtag(value: &str) -> bool {
    ((5..=8).contains(&value.len()) && value.chars().all(|ch| ch.is_ascii_alphanumeric()))
        || (value.len() == 4
            && value.starts_with(|ch: char| ch.is_ascii_digit())
            && value.chars().all(|ch| ch.is_ascii_alphanumeric()))
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
    if let Some(object) = context.value_object(&value)
        && let Some(kind) = own_string(context, object, TEMPORAL_KIND)
    {
        let day_ms = |year: f64, month: f64, day: f64| {
            days_from_civil(year as i32, month as u32, day as u32) as f64 * MS_PER_DAY
        };
        return Ok(match kind.as_str() {
            "Instant" | "ZonedDateTime" => {
                temporal_number_slot(context, object, "epochNanosecondsNumber") / 1_000_000.0
            }
            "PlainDate" => day_ms(
                own_number(context, object, "isoYear")
                    .unwrap_or_else(|| temporal_number_slot(context, object, "year")),
                own_number(context, object, "isoMonth")
                    .unwrap_or_else(|| temporal_number_slot(context, object, "month")),
                own_number(context, object, "isoDay")
                    .unwrap_or_else(|| temporal_number_slot(context, object, "day")),
            ),
            "PlainDateTime" => {
                day_ms(
                    own_number(context, object, "isoYear")
                        .unwrap_or_else(|| temporal_number_slot(context, object, "year")),
                    own_number(context, object, "isoMonth")
                        .unwrap_or_else(|| temporal_number_slot(context, object, "month")),
                    own_number(context, object, "isoDay")
                        .unwrap_or_else(|| temporal_number_slot(context, object, "day")),
                ) + plain_time_nanoseconds_i128(plain_date_time_values(context, object)) as f64
                    / 1_000_000.0
            }
            "PlainYearMonth" => day_ms(
                own_number(context, object, "isoYear")
                    .unwrap_or_else(|| temporal_number_slot(context, object, "year")),
                own_number(context, object, "isoMonth")
                    .unwrap_or_else(|| temporal_number_slot(context, object, "month")),
                temporal_number_slot(context, object, "referenceISODay"),
            ),
            "PlainMonthDay" => day_ms(
                temporal_number_slot(context, object, "referenceISOYear"),
                own_number(context, object, "isoMonth")
                    .unwrap_or_else(|| temporal_number_slot(context, object, "month")),
                own_number(context, object, "isoDay")
                    .unwrap_or_else(|| temporal_number_slot(context, object, "day")),
            ),
            "PlainTime" => {
                plain_time_nanoseconds_i128(plain_time_values_from_temporal(context, object)) as f64
                    / 1_000_000.0
            }
            _ => return vm.to_number(value, context),
        });
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
        "-∞".into()
    } else {
        "∞".into()
    };
    let mut parts = Vec::new();
    let unsigned = text.strip_prefix('-').unwrap_or(&text);
    if text.starts_with('-') {
        parts.push(part(context, "minusSign", "-".into())?);
    }
    if unsigned == "∞" {
        parts.push(part(context, "infinity", unsigned.into())?);
    } else if unsigned == "NaN" {
        parts.push(part(context, "nan", unsigned.into())?);
    } else if let Some((integer, fraction)) = unsigned.split_once('.') {
        parts.push(part(context, "integer", integer.into())?);
        parts.push(part(context, "decimal", ".".into())?);
        parts.push(part(context, "fraction", fraction.into())?);
    } else {
        parts.push(part(context, "integer", unsigned.into())?);
    }
    context.create_array(parts)
}

fn intl_number_format_range_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<String, VmError> {
    let value = vm.to_number(value, context)?;
    if value.is_nan() {
        return Err(VmError::range(
            "Intl.NumberFormat range value must not be NaN",
        ));
    }
    Ok(if value.is_infinite() {
        if value.is_sign_negative() {
            "-∞".into()
        } else {
            "∞".into()
        }
    } else {
        JsValue::Number(value).to_js_string().unwrap_or_default()
    })
}

fn intl_number_format_format_range(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "NumberFormat")?;
    let start = intl_number_format_range_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let end = intl_number_format_range_value(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    Ok(JsValue::String(if start == end {
        start
    } else {
        format!("{start}–{end}")
    }))
}

fn intl_number_format_format_range_to_parts(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, &this_value, "NumberFormat")?;
    let start = intl_number_format_range_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let end = intl_number_format_range_value(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    if start == end {
        let shared = source_part(context, "integer", start, "shared")?;
        return context.create_array(vec![shared]);
    }
    let start = source_part(context, "integer", start, "startRange")?;
    let separator = source_part(context, "literal", "–".into(), "shared")?;
    let end = source_part(context, "integer", end, "endRange")?;
    context.create_array(vec![start, separator, end])
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
    Ok(JsValue::String(locale_base_name(&intl_locale_value(
        context,
        &this_value,
    )?)))
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

fn locale_base_name(locale: &str) -> String {
    locale
        .split('-')
        .take_while(|part| part.len() != 1)
        .collect::<Vec<_>>()
        .join("-")
}

fn locale_base_components(locale: &str) -> (String, Option<String>, Option<String>, Vec<String>) {
    let base = locale_base_name(locale);
    let parts: Vec<&str> = base.split('-').collect();
    let language = parts.first().copied().unwrap_or("und").to_owned();
    let mut index = 1;
    let script = parts
        .get(index)
        .filter(|part| is_script_subtag(part))
        .map(|part| {
            index += 1;
            (*part).to_owned()
        });
    let region = parts
        .get(index)
        .filter(|part| is_region_subtag(part))
        .map(|part| {
            index += 1;
            (*part).to_owned()
        });
    let variants = parts[index..]
        .iter()
        .map(|part| (*part).to_owned())
        .collect();
    (language, script, region, variants)
}

fn locale_unicode_keyword(locale: &str, wanted: &str) -> Option<String> {
    let parts: Vec<&str> = locale.split('-').collect();
    let mut index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("u"))?
        + 1;
    while index < parts.len() && parts[index].len() > 2 {
        index += 1;
    }
    while index < parts.len() && parts[index].len() != 1 {
        let key = parts[index].to_ascii_lowercase();
        index += 1;
        let start = index;
        while index < parts.len() && parts[index].len() > 2 {
            index += 1;
        }
        if key == wanted {
            return Some(parts[start..index].join("-").to_ascii_lowercase());
        }
    }
    None
}

fn intl_locale_script_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    Ok(locale_base_components(&locale)
        .1
        .map(JsValue::String)
        .unwrap_or(JsValue::Undefined))
}

fn intl_locale_region_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    Ok(locale_base_components(&locale)
        .2
        .map(JsValue::String)
        .unwrap_or(JsValue::Undefined))
}

fn intl_locale_variants_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    let variants = locale_base_components(&locale).3;
    Ok(if variants.is_empty() {
        JsValue::Undefined
    } else {
        JsValue::String(variants.join("-"))
    })
}

fn intl_locale_keyword_get(
    context: &NativeContext,
    this_value: &JsValue,
    key: &str,
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, this_value)?;
    Ok(locale_unicode_keyword(&locale, key)
        .map(JsValue::String)
        .unwrap_or(JsValue::Undefined))
}

macro_rules! locale_keyword_getter {
    ($name:ident, $key:literal) => {
        fn $name(
            _vm: &mut Vm,
            context: &mut NativeContext,
            this_value: JsValue,
            _arguments: &[JsValue],
        ) -> Result<JsValue, VmError> {
            intl_locale_keyword_get(context, &this_value, $key)
        }
    };
}

locale_keyword_getter!(intl_locale_calendar_get, "ca");
locale_keyword_getter!(intl_locale_collation_get, "co");
locale_keyword_getter!(intl_locale_hour_cycle_get, "hc");
locale_keyword_getter!(intl_locale_case_first_get, "kf");
locale_keyword_getter!(intl_locale_numbering_system_get, "nu");
locale_keyword_getter!(intl_locale_first_day_of_week_get, "fw");

fn intl_locale_numeric_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    Ok(match locale_unicode_keyword(&locale, "kn") {
        Some(value) => JsValue::Boolean(value.is_empty() || value == "true"),
        None => JsValue::Undefined,
    })
}

fn intl_locale_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(intl_locale_value(context, &this_value)?))
}

fn locale_suffix(locale: &str) -> &str {
    let base_length = locale_base_name(locale).len();
    &locale[base_length..]
}

fn likely_locale_triple(
    language: &str,
    script: Option<&str>,
    region: Option<&str>,
) -> Option<(String, String, String)> {
    let (language, default_script, default_region) = match language {
        "und" => match (script, region) {
            (Some("Thai"), _) => ("th", "Thai", "TH"),
            (Some("Cyrl"), Some("RO")) => ("bg", "Cyrl", "RO"),
            (_, Some("419")) => ("es", "Latn", "419"),
            (_, Some("150")) => ("en", "Latn", "150"),
            (_, Some("AT")) => ("de", "Latn", "AT"),
            (_, Some("CW")) => ("pap", "Latn", "CW"),
            (_, Some("US")) => ("en", "Latn", "US"),
            (_, Some("AQ")) => ("en", "Latn", "AQ"),
            _ => ("en", "Latn", "US"),
        },
        "en" => (
            "en",
            "Latn",
            if script == Some("Shaw") { "GB" } else { "US" },
        ),
        "ar" => ("ar", "Arab", "EG"),
        "th" => ("th", "Thai", "TH"),
        "es" => ("es", "Latn", "ES"),
        "it" => ("it", "Latn", "IT"),
        "ru" => ("ru", "Cyrl", "RU"),
        "de" => ("de", "Latn", "DE"),
        "bg" => ("bg", "Cyrl", "BG"),
        "ro" => ("ro", "Latn", "RO"),
        "uz" => ("uz", "Latn", "UZ"),
        "hi" => ("hi", "Deva", "IN"),
        "aa" => ("aa", "Latn", "ET"),
        "he" => ("he", "Hebr", "IL"),
        "cs" => ("cs", "Latn", "CZ"),
        "hy" | "hyw" => (language, "Armn", "AM"),
        "jbo" => ("jbo", "Latn", "001"),
        "hak" | "hsn" => (language, "Hans", "CN"),
        "aae" => ("aae", "Latn", "IT"),
        "pap" => ("pap", "Latn", "CW"),
        "zh" => {
            let traditional = script == Some("Hant") || matches!(region, Some("TW" | "HK" | "MO"));
            (
                "zh",
                if traditional { "Hant" } else { "Hans" },
                if traditional { "TW" } else { "CN" },
            )
        }
        _ => return None,
    };
    Some((
        language.into(),
        script.unwrap_or(default_script).into(),
        region.unwrap_or(default_region).into(),
    ))
}

fn locale_with_likely_subtags(locale: &str) -> String {
    let (language, script, region, variants) = locale_base_components(locale);
    let Some((language, script, region)) =
        likely_locale_triple(&language, script.as_deref(), region.as_deref())
    else {
        return locale.into();
    };
    let mut result = vec![language, script, region];
    result.extend(variants);
    format!("{}{}", result.join("-"), locale_suffix(locale))
}

fn locale_without_likely_subtags(locale: &str) -> String {
    let maximal = locale_with_likely_subtags(locale);
    let (language, script, region, variants) = locale_base_components(&maximal);
    let Some(maximal_triple) =
        likely_locale_triple(&language, script.as_deref(), region.as_deref())
    else {
        return locale.into();
    };
    let candidates = [
        (language.clone(), None, None),
        (language.clone(), None, region.clone()),
        (language.clone(), script.clone(), None),
    ];
    let mut minimal = vec![language.clone(), script.unwrap(), region.unwrap()];
    for (candidate_language, candidate_script, candidate_region) in candidates {
        if likely_locale_triple(
            &candidate_language,
            candidate_script.as_deref(),
            candidate_region.as_deref(),
        ) == Some(maximal_triple.clone())
        {
            minimal = vec![candidate_language];
            if let Some(script) = candidate_script {
                minimal.push(script);
            }
            if let Some(region) = candidate_region {
                minimal.push(region);
            }
            break;
        }
    }
    minimal.extend(variants);
    format!("{}{}", minimal.join("-"), locale_suffix(&maximal))
}

fn create_locale_from_receiver(
    context: &mut NativeContext,
    receiver: &JsValue,
    locale: String,
) -> Result<JsValue, VmError> {
    let receiver = require_intl_kind(context, receiver, "Locale")?;
    let prototype = context
        .get_prototype_of(receiver)
        .or_else(|| context.object_prototype());
    let object = new_ordinary_object(context, prototype)?;
    define_hidden(context, object, INTL_KIND, JsValue::String("Locale".into()))?;
    define_hidden(context, object, INTL_LOCALE, JsValue::String(locale))?;
    Ok(JsValue::Object(object))
}

fn intl_locale_maximize(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    create_locale_from_receiver(context, &this_value, locale_with_likely_subtags(&locale))
}

fn intl_locale_minimize(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    create_locale_from_receiver(context, &this_value, locale_without_likely_subtags(&locale))
}

fn intl_locale_preferred_array(
    context: &mut NativeContext,
    this_value: &JsValue,
    values: &[&str],
) -> Result<JsValue, VmError> {
    require_intl_kind(context, this_value, "Locale")?;
    context.create_array(
        values
            .iter()
            .map(|value| JsValue::String((*value).into()))
            .collect(),
    )
}

fn intl_locale_get_calendars(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    if let Some(calendar) = locale_unicode_keyword(&locale, "ca") {
        context.create_array(vec![JsValue::String(calendar)])
    } else {
        intl_locale_preferred_array(context, &this_value, &["gregory"])
    }
}

fn intl_locale_get_collations(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    intl_locale_preferred_array(context, &this_value, &["emoji", "eor"])
}

fn intl_locale_get_hour_cycles(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    intl_locale_preferred_array(context, &this_value, &["h12", "h23"])
}

fn intl_locale_get_numbering_systems(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    intl_locale_preferred_array(context, &this_value, &["latn"])
}

fn intl_locale_get_text_info(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    let language = locale.split('-').next().unwrap_or("und");
    let direction = if matches!(language, "ar" | "fa" | "he" | "ur") {
        "rtl"
    } else {
        "ltr"
    };
    object_from_pairs(context, [("direction", JsValue::String(direction.into()))])
}

fn intl_locale_get_time_zones(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    let region = locale_base_components(&locale).2;
    let zones: &[&str] = match region.as_deref() {
        Some("US") => &["America/New_York"],
        Some("GB") => &["Europe/London"],
        Some("JP") => &["Asia/Tokyo"],
        Some("CN") => &["Asia/Shanghai"],
        Some("DE") => &["Europe/Berlin"],
        None => return Ok(JsValue::Undefined),
        Some(_) => &["UTC"],
    };
    intl_locale_preferred_array(context, &this_value, zones)
}

fn intl_locale_get_week_info(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let locale = intl_locale_value(context, &this_value)?;
    let region = locale_base_components(&locale).2;
    let european = matches!(region.as_deref(), Some("GB" | "DE" | "FR" | "ES" | "IT"));
    let first_day = match locale_unicode_keyword(&locale, "fw").as_deref() {
        Some("mon") => 1.0,
        Some("tue") => 2.0,
        Some("wed") => 3.0,
        Some("thu") => 4.0,
        Some("fri") => 5.0,
        Some("sat") => 6.0,
        Some("sun") => 7.0,
        _ if european => 1.0,
        _ => 7.0,
    };
    let weekend = context.create_array(vec![JsValue::Number(6.0), JsValue::Number(7.0)])?;
    object_from_pairs(
        context,
        [
            ("firstDay", JsValue::Number(first_day)),
            ("weekend", weekend),
        ],
    )
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

fn temporal_to_locale_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "Temporal receiver")?;
    match own_string(context, object, TEMPORAL_KIND).as_deref() {
        Some("Duration") => temporal_duration_to_string(vm, context, this_value, &[]),
        Some("Instant") => temporal_instant_to_string(vm, context, this_value, &[]),
        Some("PlainDate") => temporal_plain_date_to_string(vm, context, this_value, &[]),
        Some("PlainTime") => temporal_plain_time_to_string(vm, context, this_value, &[]),
        Some("PlainDateTime") => temporal_plain_date_time_to_string(vm, context, this_value, &[]),
        Some("PlainYearMonth") => temporal_plain_year_month_to_string(vm, context, this_value, &[]),
        Some("PlainMonthDay") => temporal_plain_month_day_to_string(vm, context, this_value, &[]),
        Some("ZonedDateTime") => temporal_zoned_date_time_to_string(vm, context, this_value, &[]),
        _ => Err(VmError::type_error("receiver is not a Temporal object")),
    }
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
        ("toLocaleString", 0, temporal_to_locale_string as NativeCall),
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
    define_method(context, prototype, "toJSON", 0, temporal_duration_to_json)?;
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

fn define_temporal_calendar_getter(
    context: &mut NativeContext,
    prototype: ObjectId,
    name: &'static str,
    getter_name: &'static str,
    kind: &'static str,
) -> Result<(), VmError> {
    let getter = context.register_builtin(getter_name, 0, temporal_calendar_field_get, None)?;
    let getter_object = context
        .value_object(&getter)
        .ok_or_else(|| VmError::runtime("Temporal getter object missing"))?;
    define_hidden(
        context,
        getter_object,
        TEMPORAL_KIND,
        JsValue::String(kind.into()),
    )?;
    define_hidden(
        context,
        getter_object,
        "calendarField",
        JsValue::String(name.into()),
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

fn temporal_calendar_era(
    calendar: &str,
    year: f64,
    month: f64,
    day: f64,
) -> Option<(&'static str, f64)> {
    let positive_era = |name| Some((name, year));
    let two_era = |positive, negative| {
        if year > 0.0 {
            Some((positive, year))
        } else {
            Some((negative, 1.0 - year))
        }
    };
    match calendar {
        "buddhist" => positive_era("be"),
        "coptic" => positive_era("am"),
        "ethioaa" => positive_era("aa"),
        "ethiopic" => two_era("am", "aa"),
        "gregory" => two_era("ce", "bce"),
        "hebrew" => positive_era("am"),
        "indian" => positive_era("shaka"),
        "islamic" | "islamic-civil" | "islamic-rgsa" | "islamic-tbla" | "islamic-umalqura" => {
            two_era("ah", "bh")
        }
        "persian" => positive_era("ap"),
        "roc" => two_era("roc", "broc"),
        "japanese" => {
            let date = (year as i32, month as u32, day as u32);
            if date >= (2019, 5, 1) {
                Some(("reiwa", year - 2018.0))
            } else if date >= (1989, 1, 8) {
                Some(("heisei", year - 1988.0))
            } else if date >= (1926, 12, 25) {
                Some(("showa", year - 1925.0))
            } else if date >= (1912, 7, 30) {
                Some(("taisho", year - 1911.0))
            } else if date >= (1868, 9, 8) {
                Some(("meiji", year - 1867.0))
            } else {
                two_era("ce", "bce")
            }
        }
        // ISO 8601 and the Chinese/Dangi calendars have no eras in Temporal.
        _ => None,
    }
}

fn temporal_calendar_field_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (kind, field) = context
        .current_builtin_object()
        .and_then(|object| {
            Some((
                own_string(context, object, TEMPORAL_KIND)?,
                own_string(context, object, "calendarField")?,
            ))
        })
        .unwrap_or_else(|| (String::new(), String::new()));
    let object = require_temporal_kind(context, &this_value, Box::leak(kind.into_boxed_str()))?;
    let calendar = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let year = temporal_number_slot(context, object, "year");
    let month = temporal_number_slot(context, object, "month");
    let day = own_number(context, object, "day")
        .or_else(|| own_number(context, object, "referenceISODay"))
        .unwrap_or(1.0);
    let Some((era, era_year)) = temporal_calendar_era(&calendar, year, month, day) else {
        return Ok(JsValue::Undefined);
    };
    match field.as_str() {
        "era" => Ok(JsValue::String(era.into())),
        "eraYear" => Ok(JsValue::Number(era_year)),
        _ => Ok(JsValue::Undefined),
    }
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
    let values = duration_values_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
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
    if values.years.abs() > MAX_DURATION_DATE_FIELD
        || values.months.abs() > MAX_DURATION_DATE_FIELD
        || values.weeks.abs() > MAX_DURATION_DATE_FIELD
    {
        return Err(VmError::range("Temporal.Duration fields are out of range"));
    }
    let total_time_ns = values.days as i128 * NS_PER_DAY_I128
        + values.hours as i128 * NS_PER_HOUR_I128
        + values.minutes as i128 * NS_PER_MINUTE_I128
        + values.seconds as i128 * NS_PER_SECOND_I128
        + values.milliseconds as i128 * NS_PER_MILLISECOND_I128
        + values.microseconds as i128 * 1_000
        + values.nanoseconds as i128;
    if total_time_ns.abs() > MAX_DURATION_TOTAL_NS {
        return Err(VmError::range("Temporal.Duration fields are out of range"));
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
    if [
        left.years,
        left.months,
        left.weeks,
        right.years,
        right.months,
        right.weeks,
    ]
    .into_iter()
    .any(|value| value != 0.0)
    {
        return Err(VmError::range(
            "Temporal.Duration arithmetic requires relativeTo for calendar units",
        ));
    }
    let left_largest = duration_largest_unit(left);
    let right_largest = duration_largest_unit(right);
    let largest_unit =
        if temporal_unit_nanoseconds(&left_largest) >= temporal_unit_nanoseconds(&right_largest) {
            left_largest
        } else {
            right_largest
        };
    let left_total = left.days as i128 * NS_PER_DAY_I128 + duration_time_nanoseconds_i128(left);
    let right_total = right.days as i128 * NS_PER_DAY_I128 + duration_time_nanoseconds_i128(right);
    create_duration_from_nanoseconds(
        context,
        left_total + sign as i128 * right_total,
        &largest_unit,
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
        if duration_round_requires_relative_to(values, &largest, &smallest) {
            return Err(VmError::range(
                "Temporal.Duration round requires relativeTo for calendar units",
            ));
        }
        return Ok((largest, smallest, 1, TemporalRoundMode::HalfExpand));
    }
    let object = context.require_object(&value, "Temporal.Duration.prototype.round options")?;
    let largest = option_string(vm, context, object, "largestUnit")?;
    let relative_to = temporal_get_property(vm, context, object, "relativeTo")?;
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
    if matches!(
        smallest_unit.as_str(),
        "hour" | "minute" | "second" | "millisecond" | "microsecond" | "nanosecond"
    ) {
        validate_rounding_increment(&smallest_unit, increment)?;
    }
    let has_relative_to = validate_temporal_relative_to(vm, context, relative_to)?;
    if !has_relative_to
        && duration_round_requires_relative_to(values, &largest_unit, &smallest_unit)
    {
        return Err(VmError::range(
            "Temporal.Duration round requires relativeTo for calendar units",
        ));
    }
    Ok((largest_unit, smallest_unit, increment, mode))
}

fn duration_round_requires_relative_to(
    values: DurationValues,
    largest_unit: &str,
    smallest_unit: &str,
) -> bool {
    values.years != 0.0
        || values.months != 0.0
        || values.weeks != 0.0
        || matches!(largest_unit, "year" | "month" | "week")
        || matches!(smallest_unit, "year" | "month" | "week")
}

fn temporal_duration_total(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let values = duration_this_values(context, &this_value)?;
    let total_options = duration_total_unit(vm, context, arguments.first().cloned())?;
    let unit = total_options.unit;
    if !total_options.has_relative_to
        && (values.years != 0.0
            || values.months != 0.0
            || values.weeks != 0.0
            || matches!(unit.as_str(), "year" | "month" | "week"))
    {
        return Err(VmError::range(
            "Temporal.Duration total requires relativeTo for calendar units",
        ));
    }
    if let Some(relative_to) = total_options.relative_to {
        return Ok(JsValue::Number(duration_total_relative_to(
            relative_to,
            values,
            &unit,
        )?));
    }
    let total_ns = duration_total_nanoseconds_i128(values);
    let value = match unit.as_str() {
        "year" => total_nanoseconds_in_unit(total_ns, 365 * NS_PER_DAY_I128),
        "month" => total_nanoseconds_in_unit(total_ns, 30 * NS_PER_DAY_I128),
        "week" => total_nanoseconds_in_unit(total_ns, 7 * NS_PER_DAY_I128),
        "day" => total_nanoseconds_in_unit(total_ns, NS_PER_DAY_I128),
        "hour" => total_nanoseconds_in_unit(total_ns, NS_PER_HOUR_I128),
        "minute" => total_nanoseconds_in_unit(total_ns, NS_PER_MINUTE_I128),
        "second" => total_nanoseconds_in_unit(total_ns, NS_PER_SECOND_I128),
        "millisecond" => total_nanoseconds_in_unit(total_ns, NS_PER_MILLISECOND_I128),
        "microsecond" => total_nanoseconds_in_unit(total_ns, 1_000),
        "nanosecond" => total_ns as f64,
        _ => return Err(VmError::range("invalid Temporal.Duration total unit")),
    };
    Ok(JsValue::Number(value))
}

struct DurationTotalOptions {
    unit: String,
    has_relative_to: bool,
    relative_to: Option<TemporalRelativeTo>,
}

#[derive(Clone)]
struct TemporalRelativeTo {
    year: i32,
    month: u32,
    day: u32,
    time: PlainTimeValues,
    calendar: String,
    zoned: bool,
    offset_nanoseconds: i128,
}

fn temporal_relative_to_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Option<TemporalRelativeTo>, VmError> {
    match value {
        JsValue::Undefined => Ok(None),
        JsValue::String(text) => {
            if let Some((local_ns, supplied_offset, has_z, zone, in_range)) =
                parse_zoned_date_time(&text)
            {
                if !in_range {
                    return Err(VmError::range("Temporal relativeTo is out of range"));
                }
                let day_number = i64::try_from(local_ns.div_euclid(NS_PER_DAY_I128))
                    .map_err(|_| VmError::range("Temporal relativeTo is out of range"))?;
                let (year, month, day) = calendar_civil_from_days("iso8601", day_number);
                let time = plain_time_from_nanoseconds_i128(local_ns.rem_euclid(NS_PER_DAY_I128));
                validate_plain_date(year as f64, month as f64, day as f64)?;
                let zone_offset = if zone.eq_ignore_ascii_case("UTC") {
                    Some(0)
                } else {
                    parse_time_zone_offset_ns(&zone)
                };
                if let (Some(supplied), Some(zone)) = (supplied_offset, zone_offset)
                    && !has_z
                    && supplied != zone
                {
                    return Err(VmError::range(
                        "Temporal relativeTo offset does not match time zone",
                    ));
                }
                let offset_nanoseconds = supplied_offset.or(zone_offset).unwrap_or(0);
                if !is_valid_instant_ns(local_ns - offset_nanoseconds) {
                    return Err(VmError::range("Temporal relativeTo is out of range"));
                }
                return Ok(Some(TemporalRelativeTo {
                    year,
                    month,
                    day,
                    time,
                    calendar: "iso8601".into(),
                    zoned: true,
                    offset_nanoseconds,
                }));
            }
            if text
                .split('[')
                .skip(1)
                .filter_map(|annotation| annotation.split(']').next())
                .map(|annotation| annotation.strip_prefix('!').unwrap_or(annotation))
                .any(|annotation| !annotation.contains('='))
            {
                return Err(VmError::range("invalid Temporal relativeTo time zone"));
            }
            if let Some((year, month, day, time)) = parse_plain_date_time(&text) {
                validate_plain_date(year, month, day)?;
                validate_plain_time(time)?;
                return Ok(Some(TemporalRelativeTo {
                    year: year as i32,
                    month: month as u32,
                    day: day as u32,
                    time,
                    calendar: "iso8601".into(),
                    zoned: false,
                    offset_nanoseconds: 0,
                }));
            }
            if let Some((year, month, day)) = parse_plain_date(&text) {
                validate_plain_date(year, month, day)?;
                return Ok(Some(TemporalRelativeTo {
                    year: year as i32,
                    month: month as u32,
                    day: day as u32,
                    time: PlainTimeValues::default(),
                    calendar: "iso8601".into(),
                    zoned: false,
                    offset_nanoseconds: 0,
                }));
            }
            Err(VmError::range("invalid Temporal relativeTo string"))
        }
        JsValue::Object(object) => {
            let kind = own_string(context, object, TEMPORAL_KIND);
            if matches!(
                kind.as_deref(),
                Some("PlainDate" | "PlainDateTime" | "ZonedDateTime")
            ) {
                let (year, month, day) = date_parts(context, object);
                let time = if kind.as_deref() == Some("PlainDate") {
                    PlainTimeValues::default()
                } else {
                    plain_date_time_values(context, object)
                };
                validate_plain_date_for_calendar(
                    &own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                    year as f64,
                    month as f64,
                    day as f64,
                )?;
                validate_plain_time(time)?;
                return Ok(Some(TemporalRelativeTo {
                    year,
                    month,
                    day,
                    time,
                    calendar: own_string(context, object, "calendarId")
                        .unwrap_or_else(|| "iso8601".into()),
                    zoned: kind.as_deref() == Some("ZonedDateTime"),
                    offset_nanoseconds: own_number(context, object, "offsetNanoseconds")
                        .unwrap_or(0.0) as i128,
                }));
            }
            if kind.is_some() {
                return Err(VmError::type_error("invalid Temporal relativeTo object"));
            }
            let calendar = temporal_calendar_id_from_object(vm, context, object)?;
            let year = temporal_calendar_year_from_object(vm, context, object, &calendar)?;
            let month = temporal_required_month_from_object(vm, context, object, Some(&calendar))?;
            let day = temporal_required_object_number(vm, context, object, "day")?;
            let time = PlainTimeValues {
                hour: temporal_object_number(vm, context, object, "hour")?,
                minute: temporal_object_number(vm, context, object, "minute")?,
                second: constrain_time_second(temporal_object_number(
                    vm, context, object, "second",
                )?),
                millisecond: temporal_object_number(vm, context, object, "millisecond")?,
                microsecond: temporal_object_number(vm, context, object, "microsecond")?,
                nanosecond: temporal_object_number(vm, context, object, "nanosecond")?,
            };
            let time_zone = temporal_get_property(vm, context, object, "timeZone")?;
            let offset = temporal_get_property(vm, context, object, "offset")?;
            let offset_nanoseconds = if matches!(offset, JsValue::Undefined) {
                0
            } else {
                let text = temporal_string_or_object_to_string(
                    vm,
                    context,
                    offset.clone(),
                    "Temporal relativeTo offset must be a string",
                )?;
                parse_iso_time_zone_offset_ns(&text)
                    .ok_or_else(|| VmError::range("invalid Temporal relativeTo offset"))?
            };
            if !matches!(time_zone, JsValue::Undefined) {
                let zone = temporal_time_zone_string(vm, context, time_zone)?;
                if let Some(zone_offset) = parse_time_zone_offset_ns(&zone)
                    .or_else(|| zone.eq_ignore_ascii_case("UTC").then_some(0))
                    && !matches!(offset, JsValue::Undefined)
                    && zone_offset != offset_nanoseconds
                {
                    return Err(VmError::range(
                        "Temporal relativeTo offset does not match time zone",
                    ));
                }
            }
            validate_plain_date_for_calendar(&calendar, year, month, day)?;
            validate_plain_time(time)?;
            Ok(Some(TemporalRelativeTo {
                year: year as i32,
                month: month as u32,
                day: day as u32,
                time,
                calendar,
                zoned: false,
                offset_nanoseconds: 0,
            }))
        }
        _ => Err(VmError::type_error("invalid Temporal relativeTo value")),
    }
}

fn duration_relative_to_total_nanoseconds(
    relative_to: &TemporalRelativeTo,
    duration: DurationValues,
) -> Result<i128, VmError> {
    let calendar = relative_to.calendar.as_str();
    let start_day = calendar_days_from_civil(
        calendar,
        relative_to.year,
        relative_to.month,
        relative_to.day,
    );
    let start_time = plain_time_nanoseconds_i128(relative_to.time);
    if !duration_has_date_fields(duration) {
        return Ok(duration_time_nanoseconds_i128(duration));
    }
    let month_delta = (duration.years * 12.0 + duration.months).trunc() as i64;
    let month_date = add_calendar_months(
        calendar,
        relative_to.year,
        relative_to.month,
        relative_to.day,
        month_delta,
    );
    let month_day = calendar_days_from_civil(calendar, month_date.0, month_date.1, month_date.2);
    let target_day = month_day
        .checked_add((duration.weeks * 7.0 + duration.days).trunc() as i64)
        .ok_or_else(|| VmError::range("Temporal.Duration total relativeTo is out of range"))?;
    if !temporal_day_number_within_range(target_day) {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    let end_ns = target_day as i128 * NS_PER_DAY_I128
        + start_time
        + duration_time_nanoseconds_i128(duration);
    let start_ns = start_day as i128 * NS_PER_DAY_I128 + start_time;
    let total_ns = end_ns
        .checked_sub(start_ns)
        .ok_or_else(|| VmError::range("Temporal.Duration total relativeTo is out of range"))?;
    let end_day_number = i64::try_from(end_ns.div_euclid(NS_PER_DAY_I128))
        .map_err(|_| VmError::range("Temporal.Duration total relativeTo is out of range"))?;
    if !temporal_day_number_within_range(end_day_number) {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    if relative_to.zoned
        && duration_has_date_fields(duration)
        && !is_valid_instant_ns(end_ns - relative_to.offset_nanoseconds)
    {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    Ok(total_ns)
}

fn duration_total_relative_to(
    relative_to: TemporalRelativeTo,
    duration: DurationValues,
    unit: &str,
) -> Result<f64, VmError> {
    let total_ns = duration_relative_to_total_nanoseconds(&relative_to, duration)?;
    let start_day = calendar_days_from_civil(
        relative_to.calendar.as_str(),
        relative_to.year,
        relative_to.month,
        relative_to.day,
    );
    let start_ns =
        start_day as i128 * NS_PER_DAY_I128 + plain_time_nanoseconds_i128(relative_to.time);
    let target_ns = start_ns + total_ns;
    if relative_to.zoned
        && unit == "day"
        && start_day == 99_999_999
        && plain_time_nanoseconds_i128(relative_to.time) != 0
    {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    if relative_to.zoned && !is_valid_instant_ns(target_ns - relative_to.offset_nanoseconds) {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    if relative_to.zoned
        && matches!(unit, "year" | "month" | "week")
        && (target_ns - relative_to.offset_nanoseconds).abs() >= MAX_INSTANT_NS
    {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    if matches!(unit, "year" | "month" | "week") {
        let target_day = i64::try_from(target_ns.div_euclid(NS_PER_DAY_I128))
            .map_err(|_| VmError::range("Temporal.Duration total relativeTo is out of range"))?;
        if !temporal_day_number_within_range(target_day) {
            return Err(VmError::range(
                "Temporal.Duration total relativeTo is out of range",
            ));
        }
    }
    if (start_day == -100_000_001 && total_ns > 0)
        || (start_day == 100_000_000 && total_ns > 0 && matches!(unit, "year" | "month" | "week"))
    {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    if matches!(
        unit,
        "nanosecond"
            | "microsecond"
            | "millisecond"
            | "second"
            | "minute"
            | "hour"
            | "day"
            | "week"
    ) {
        return Ok(match unit {
            "nanosecond" => total_ns as f64,
            "microsecond" => total_nanoseconds_in_unit(total_ns, 1_000),
            "millisecond" => total_nanoseconds_in_unit(total_ns, NS_PER_MILLISECOND_I128),
            "second" => total_nanoseconds_in_unit(total_ns, NS_PER_SECOND_I128),
            "minute" => total_nanoseconds_in_unit(total_ns, NS_PER_MINUTE_I128),
            "hour" => total_nanoseconds_in_unit(total_ns, NS_PER_HOUR_I128),
            "day" => total_nanoseconds_in_unit(total_ns, NS_PER_DAY_I128),
            "week" => total_nanoseconds_in_unit(total_ns, 7 * NS_PER_DAY_I128),
            _ => unreachable!(),
        });
    }
    let calendar = relative_to.calendar.as_str();
    let start_day = calendar_days_from_civil(
        calendar,
        relative_to.year,
        relative_to.month,
        relative_to.day,
    );
    let end_ns = start_day as i128 * NS_PER_DAY_I128
        + plain_time_nanoseconds_i128(relative_to.time)
        + total_ns;
    let end_day = i64::try_from(end_ns.div_euclid(NS_PER_DAY_I128))
        .map_err(|_| VmError::range("Temporal.Duration total relativeTo is out of range"))?;
    if !temporal_day_number_within_range(end_day) {
        return Err(VmError::range(
            "Temporal.Duration total relativeTo is out of range",
        ));
    }
    let (end_year, end_month, _) = calendar_civil_from_days(calendar, end_day);
    let mut months = (end_year as i64 * 12 + end_month as i64)
        - (relative_to.year as i64 * 12 + relative_to.month as i64);
    if unit == "year" {
        months = (months / 12) * 12;
    }
    let candidate = |count: i64| {
        let date = add_calendar_months(
            calendar,
            relative_to.year,
            relative_to.month,
            relative_to.day,
            count,
        );
        calendar_days_from_civil(calendar, date.0, date.1, date.2) as i128 * NS_PER_DAY_I128
            + plain_time_nanoseconds_i128(relative_to.time)
    };
    let start_ns =
        start_day as i128 * NS_PER_DAY_I128 + plain_time_nanoseconds_i128(relative_to.time);
    let target_ns = start_ns + total_ns;
    let mut candidate_ns = candidate(months);
    let direction = total_ns.signum();
    if direction > 0 && candidate_ns > target_ns {
        months -= 1;
        candidate_ns = candidate(months);
    } else if direction < 0 && candidate_ns < target_ns {
        months += 1;
        candidate_ns = candidate(months);
    }
    let step = if unit == "year" { 12 } else { 1 } * direction as i64;
    let next_ns = candidate(months + step);
    if unit == "year" {
        let denominator = (next_ns - candidate_ns).abs();
        if denominator == 0 {
            Ok(months as f64 / 12.0)
        } else {
            Ok(ratio_i128_to_f64(
                months as i128 / 12 * denominator + target_ns - candidate_ns,
                denominator,
            ))
        }
    } else if unit == "month" {
        let denominator = (next_ns - candidate_ns).abs();
        if denominator == 0 {
            Ok(months as f64)
        } else {
            Ok(ratio_i128_to_f64(
                months as i128 * denominator + target_ns - candidate_ns,
                denominator,
            ))
        }
    } else {
        Err(VmError::range("invalid Temporal.Duration total unit"))
    }
}

fn total_nanoseconds_in_unit(total_ns: i128, unit_ns: i128) -> f64 {
    ratio_i128_to_f64(total_ns, unit_ns)
}

fn ratio_i128_to_f64(numerator: i128, denominator: i128) -> f64 {
    debug_assert!(denominator > 0);
    if numerator == 0 {
        return 0.0;
    }
    let negative = numerator < 0;
    let mut remainder = numerator.unsigned_abs();
    let denominator = denominator as u128;
    let whole = remainder / denominator;
    remainder %= denominator;

    let mut text = String::new();
    if negative {
        text.push('-');
    }
    text.push_str(&whole.to_string());
    if remainder != 0 {
        text.push('.');
        for _ in 0..80 {
            remainder *= 10;
            let digit = remainder / denominator;
            text.push(char::from(b'0' + digit as u8));
            remainder %= denominator;
            if remainder == 0 {
                break;
            }
        }
    }
    text.parse::<f64>().unwrap_or({
        if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }
    })
}

fn duration_total_nanoseconds_i128(values: DurationValues) -> i128 {
    values.days as i128 * NS_PER_DAY_I128 + duration_time_nanoseconds_i128(values)
}

fn duration_total_unit(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: Option<JsValue>,
) -> Result<DurationTotalOptions, VmError> {
    let value = value.unwrap_or(JsValue::Undefined);
    match value {
        JsValue::String(unit) => Ok(DurationTotalOptions {
            unit: normalize_temporal_unit(unit)?,
            has_relative_to: false,
            relative_to: None,
        }),
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            let object = context.require_object(&value, "Temporal.Duration.prototype.total")?;
            let relative_to = temporal_get_property(vm, context, object, "relativeTo")?;
            let unit = temporal_get_property(vm, context, object, "unit")?;
            if matches!(unit, JsValue::Undefined) {
                return Err(VmError::range("Temporal.Duration total requires a unit"));
            }
            let has_relative_to = validate_temporal_relative_to(vm, context, relative_to.clone())?;
            let relative_to_data = if has_relative_to {
                temporal_relative_to_from_value(vm, context, relative_to)?
            } else {
                None
            };
            Ok(DurationTotalOptions {
                unit: normalize_temporal_unit(vm.to_string_coerce(unit, context)?)?,
                has_relative_to,
                relative_to: relative_to_data,
            })
        }
        JsValue::Undefined => Err(VmError::type_error(
            "Temporal.Duration total requires options",
        )),
        _ => Err(VmError::type_error(
            "Temporal.Duration total options must be an object or string",
        )),
    }
}

fn validate_temporal_relative_to(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<bool, VmError> {
    match value {
        JsValue::Undefined => Ok(false),
        JsValue::String(text) => {
            if parse_zoned_date_time(&text).is_some() || parse_plain_date(&text).is_some() {
                Ok(true)
            } else {
                Err(VmError::range("invalid Temporal relativeTo string"))
            }
        }
        JsValue::Object(object) => {
            if matches!(
                own_string(context, object, TEMPORAL_KIND).as_deref(),
                Some("PlainDate" | "PlainDateTime" | "ZonedDateTime")
            ) {
                return Ok(true);
            }
            if own_string(context, object, TEMPORAL_KIND).is_some() {
                return Err(VmError::type_error("invalid Temporal relativeTo object"));
            }
            let year = temporal_required_object_number(vm, context, object, "year")?;
            let month = temporal_required_month_from_object(vm, context, object, None)?;
            let day = temporal_required_object_number(vm, context, object, "day")?;
            temporal_calendar_id_from_object(vm, context, object)?;
            validate_plain_date(year, month, day)?;
            validate_plain_time(PlainTimeValues {
                hour: temporal_object_number(vm, context, object, "hour")?,
                minute: temporal_object_number(vm, context, object, "minute")?,
                second: constrain_time_second(temporal_object_number(
                    vm, context, object, "second",
                )?),
                millisecond: temporal_object_number(vm, context, object, "millisecond")?,
                microsecond: temporal_object_number(vm, context, object, "microsecond")?,
                nanosecond: temporal_object_number(vm, context, object, "nanosecond")?,
            })?;
            let time_zone = temporal_get_property(vm, context, object, "timeZone")?;
            if !matches!(time_zone, JsValue::Undefined) {
                temporal_time_zone_string(vm, context, time_zone)?;
                let offset = temporal_get_property(vm, context, object, "offset")?;
                if !matches!(offset, JsValue::Undefined) {
                    let text = temporal_string_or_object_to_string(
                        vm,
                        context,
                        offset,
                        "Temporal relativeTo offset must be a string",
                    )?;
                    parse_iso_time_zone_offset_ns(&text)
                        .ok_or_else(|| VmError::range("invalid Temporal relativeTo offset"))?;
                }
            }
            Ok(true)
        }
        _ => Err(VmError::type_error("invalid Temporal relativeTo value")),
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

    // PrepareTemporalFields observes duration-like properties in alphabetical
    // order and converts each value immediately after reading it.
    let days = duration_partial_field(vm, context, object, "days")?;
    let hours = duration_partial_field(vm, context, object, "hours")?;
    let microseconds = duration_partial_field(vm, context, object, "microseconds")?;
    let milliseconds = duration_partial_field(vm, context, object, "milliseconds")?;
    let minutes = duration_partial_field(vm, context, object, "minutes")?;
    let months = duration_partial_field(vm, context, object, "months")?;
    let nanoseconds = duration_partial_field(vm, context, object, "nanoseconds")?;
    let seconds = duration_partial_field(vm, context, object, "seconds")?;
    let weeks = duration_partial_field(vm, context, object, "weeks")?;
    let years = duration_partial_field(vm, context, object, "years")?;

    if [
        days,
        hours,
        microseconds,
        milliseconds,
        minutes,
        months,
        nanoseconds,
        seconds,
        weeks,
        years,
    ]
    .iter()
    .all(Option::is_none)
    {
        return Err(VmError::type_error(
            "Temporal duration-like object has no duration properties",
        ));
    }

    values.days = days.unwrap_or(values.days);
    values.hours = hours.unwrap_or(values.hours);
    values.microseconds = microseconds.unwrap_or(values.microseconds);
    values.milliseconds = milliseconds.unwrap_or(values.milliseconds);
    values.minutes = minutes.unwrap_or(values.minutes);
    values.months = months.unwrap_or(values.months);
    values.nanoseconds = nanoseconds.unwrap_or(values.nanoseconds);
    values.seconds = seconds.unwrap_or(values.seconds);
    values.weeks = weeks.unwrap_or(values.weeks);
    values.years = years.unwrap_or(values.years);

    create_duration_with_default_prototype(context, validate_duration_values(values)?)
}

fn duration_partial_field(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<Option<f64>, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    if matches!(value, JsValue::Undefined) {
        Ok(None)
    } else {
        duration_integer(vm.to_number(value, context)?).map(Some)
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
    let relative_to = match arguments.get(2).cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Undefined => JsValue::Undefined,
        options @ (JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_)) => {
            let options = context.require_object(&options, "Temporal.Duration.compare options")?;
            temporal_get_property(vm, context, options, "relativeTo")?
        }
        _ => {
            return Err(VmError::type_error(
                "Temporal.Duration.compare options must be an object",
            ));
        }
    };
    let has_relative_to = validate_temporal_relative_to(vm, context, relative_to.clone())?;
    let relative_to = if has_relative_to {
        temporal_relative_to_from_value(vm, context, relative_to)?
    } else {
        None
    };
    if duration_values_equal(left, right) {
        return Ok(JsValue::Number(0.0));
    }
    if !has_relative_to
        && (left.years != 0.0
            || left.months != 0.0
            || left.weeks != 0.0
            || right.years != 0.0
            || right.months != 0.0
            || right.weeks != 0.0)
    {
        return Err(VmError::range(
            "Temporal.Duration.compare requires relativeTo for calendar units",
        ));
    }
    let (left_total, right_total) = if let Some(relative_to) = relative_to {
        (
            duration_relative_to_total_nanoseconds(&relative_to, left)?,
            duration_relative_to_total_nanoseconds(&relative_to, right)?,
        )
    } else {
        (
            left.days as i128 * NS_PER_DAY_I128 + duration_time_nanoseconds_i128(left),
            right.days as i128 * NS_PER_DAY_I128 + duration_time_nanoseconds_i128(right),
        )
    };
    Ok(JsValue::Number(if left_total < right_total {
        -1.0
    } else if left_total > right_total {
        1.0
    } else {
        0.0
    }))
}

fn duration_values_equal(left: DurationValues, right: DurationValues) -> bool {
    [
        (left.years, right.years),
        (left.months, right.months),
        (left.weeks, right.weeks),
        (left.days, right.days),
        (left.hours, right.hours),
        (left.minutes, right.minutes),
        (left.seconds, right.seconds),
        (left.milliseconds, right.milliseconds),
        (left.microseconds, right.microseconds),
        (left.nanoseconds, right.nanoseconds),
    ]
    .into_iter()
    .all(|(left, right)| left == right)
}

fn duration_has_date_fields(values: DurationValues) -> bool {
    values.years != 0.0 || values.months != 0.0 || values.weeks != 0.0 || values.days != 0.0
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
const MAX_DURATION_DATE_FIELD: f64 = 4_294_967_295.0;
const MAX_DURATION_TOTAL_NS: i128 = 9_007_199_254_740_991_999_999_999;

fn duration_time_nanoseconds(values: DurationValues) -> f64 {
    values.hours * NS_PER_HOUR
        + values.minutes * NS_PER_MINUTE
        + values.seconds * NS_PER_SECOND
        + values.milliseconds * NS_PER_MILLISECOND
        + values.microseconds * NS_PER_MICROSECOND
        + values.nanoseconds
}

fn duration_time_nanoseconds_i128(values: DurationValues) -> i128 {
    values.hours as i128 * NS_PER_HOUR_I128
        + values.minutes as i128 * NS_PER_MINUTE_I128
        + values.seconds as i128 * NS_PER_SECOND_I128
        + values.milliseconds as i128 * NS_PER_MILLISECOND_I128
        + values.microseconds as i128 * 1_000
        + values.nanoseconds as i128
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
    vm.get_property_value(JsValue::Object(object), name, context)
}

fn temporal_string_or_object_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    error: &str,
) -> Result<String, VmError> {
    match value {
        JsValue::String(text) => Ok(text),
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            vm.to_string_coerce(value, context)
        }
        _ => Err(VmError::type_error(error)),
    }
}

fn temporal_month_code_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<String, VmError> {
    let primitive = match value {
        JsValue::String(text) => return Ok(text),
        JsValue::Object(_) | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            vm.to_primitive(value, PreferredType::String, context)?
        }
        _ => {
            return Err(VmError::type_error(
                "Temporal monthCode property must be a string",
            ));
        }
    };
    match primitive {
        JsValue::String(text) => Ok(text),
        _ => Err(VmError::type_error(
            "Temporal monthCode property must be a string",
        )),
    }
}

fn temporal_object_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<f64, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    temporal_number_or_default_from_value(vm, context, value, 0.0)
}

fn temporal_number_or_default_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    default: f64,
) -> Result<f64, VmError> {
    if matches!(value, JsValue::Undefined) {
        Ok(default)
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

fn temporal_calendar_year_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    calendar: &str,
) -> Result<f64, VmError> {
    let era_value = temporal_get_property(vm, context, object, "era")?;
    let era_year_value = temporal_get_property(vm, context, object, "eraYear")?;
    let year_value = temporal_get_property(vm, context, object, "year")?;
    if !matches!(year_value, JsValue::Undefined) {
        return vm.to_number(year_value, context);
    }
    if matches!(era_value, JsValue::Undefined) || matches!(era_year_value, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal property `year` or `era` and `eraYear` is required",
        ));
    }
    let era = vm
        .to_string_coerce(era_value, context)?
        .to_ascii_lowercase();
    let era_year = vm.to_number(era_year_value, context)?;
    if !era_year.is_finite() || era_year.fract() != 0.0 || era_year <= 0.0 {
        return Err(VmError::range("invalid Temporal eraYear"));
    }
    let year = match (calendar, era.as_str()) {
        ("gregory" | "japanese", "ce" | "ad") => era_year,
        ("gregory" | "japanese", "bce" | "bc") => 1.0 - era_year,
        ("roc", "roc") => era_year,
        ("roc", "broc") => 1.0 - era_year,
        (
            "islamic" | "islamic-civil" | "islamic-rgsa" | "islamic-tbla" | "islamic-umalqura",
            "ah",
        ) => era_year,
        (
            "islamic" | "islamic-civil" | "islamic-rgsa" | "islamic-tbla" | "islamic-umalqura",
            "bh",
        ) => 1.0 - era_year,
        ("japanese", "meiji") => era_year + 1867.0,
        ("japanese", "taisho") => era_year + 1911.0,
        ("japanese", "showa") => era_year + 1925.0,
        ("japanese", "heisei") => era_year + 1988.0,
        ("japanese", "reiwa") => era_year + 2018.0,
        ("buddhist", "be")
        | ("coptic", "am")
        | ("ethioaa", "aa")
        | ("ethiopic", "am")
        | ("hebrew", "am")
        | ("indian", "shaka")
        | ("persian", "ap") => era_year,
        ("ethiopic", "aa") => 1.0 - era_year,
        _ => return Err(VmError::range("invalid Temporal era for calendar")),
    };
    Ok(year)
}

fn temporal_calendar_year_replacement(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    calendar: &str,
    current: f64,
) -> Result<f64, VmError> {
    let era = temporal_get_property(vm, context, object, "era")?;
    let era_year = temporal_get_property(vm, context, object, "eraYear")?;
    let year = temporal_get_property(vm, context, object, "year")?;
    if !matches!(year, JsValue::Undefined) {
        return vm.to_number(year, context);
    }
    if matches!(era, JsValue::Undefined) && matches!(era_year, JsValue::Undefined) {
        return Ok(current);
    }
    if matches!(era, JsValue::Undefined) || matches!(era_year, JsValue::Undefined) {
        return Err(VmError::type_error(
            "Temporal era and eraYear must be provided together",
        ));
    }
    let holder = new_ordinary_object(context, context.object_prototype())?;
    context.define_own_property(holder, "era".into(), PropertyDescriptor::data(era))?;
    context.define_own_property(holder, "eraYear".into(), PropertyDescriptor::data(era_year))?;
    temporal_calendar_year_from_object(vm, context, holder, calendar)
}

fn temporal_required_month_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    calendar: Option<&str>,
) -> Result<f64, VmError> {
    let month_value = temporal_get_property(vm, context, object, "month")?;
    let month_code_value = temporal_get_property(vm, context, object, "monthCode")?;
    let month = if matches!(month_value, JsValue::Undefined) {
        None
    } else {
        Some(vm.to_number(month_value, context)?.trunc())
    };
    let month_code = if matches!(month_code_value, JsValue::Undefined) {
        None
    } else {
        let text = temporal_string_or_object_to_string(
            vm,
            context,
            month_code_value,
            "Temporal monthCode property must be a string",
        )?;
        let parsed =
            parse_month_code(&text).ok_or_else(|| VmError::range("invalid Temporal monthCode"))?;
        if calendar == Some("iso8601") && (parsed > 12.0 || text.ends_with('L')) {
            if text.ends_with('L') {
                let year = temporal_get_property(vm, context, object, "year")?;
                if !matches!(year, JsValue::Undefined) {
                    vm.to_number(year, context)?;
                }
            }
            return Err(VmError::range(
                "Temporal monthCode is not valid for the ISO 8601 calendar",
            ));
        }
        Some(parsed)
    };
    if let (Some(month), Some(month_code)) = (month, month_code)
        && month != month_code
    {
        return Err(VmError::range("Temporal month and monthCode conflict"));
    }
    if let Some(month) = month.or(month_code) {
        Ok(month)
    } else {
        Err(VmError::type_error(
            "Temporal property `month` or `monthCode` is required",
        ))
    }
}

fn temporal_required_month_fields_from_object(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    calendar: &str,
) -> Result<(f64, Option<String>), VmError> {
    let month_value = temporal_get_property(vm, context, object, "month")?;
    let month_code_value = temporal_get_property(vm, context, object, "monthCode")?;
    let month = if matches!(month_value, JsValue::Undefined) {
        None
    } else {
        Some(vm.to_number(month_value, context)?.trunc())
    };
    let month_code = if matches!(month_code_value, JsValue::Undefined) {
        None
    } else {
        let text = temporal_month_code_to_string(vm, context, month_code_value)?;
        parse_month_code(&text).ok_or_else(|| VmError::range("invalid Temporal monthCode"))?;
        if calendar == "iso8601" && (text.ends_with('L') || parse_month_code(&text) > Some(12.0)) {
            return Err(VmError::range(
                "Temporal monthCode is not valid for the ISO 8601 calendar",
            ));
        }
        Some(text)
    };
    let numeric_code = month_code.as_deref().and_then(parse_month_code);
    if let (Some(month), Some(code)) = (month, numeric_code)
        && !month_code
            .as_deref()
            .is_some_and(|code| code.ends_with('L'))
        && month != code
    {
        return Err(VmError::range("Temporal month and monthCode conflict"));
    }
    let effective_month = month.or(numeric_code).ok_or_else(|| {
        VmError::type_error("Temporal property `month` or `monthCode` is required")
    })?;
    Ok((effective_month, month_code))
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
    let normalized_body = body.to_ascii_uppercase().replace(',', ".");
    let mut chars = normalized_body.strip_prefix('P')?.chars().peekable();
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
        let designator = chars.next()?;
        let fractional = number.contains('.');
        if fractional && chars.peek().is_some() {
            return None;
        }
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
        if designator == 'S' && in_time {
            parse_duration_seconds_part(&number, duration_sign, &mut values)?;
            continue;
        }
        let amount = number.parse::<f64>().ok()?;
        if !amount.is_finite() {
            return None;
        }
        let signed = duration_sign * amount;
        match (designator, in_time) {
            ('Y', false) if !fractional => values.years = signed,
            ('M', false) if !fractional => values.months = signed,
            ('W', false) if !fractional => values.weeks = signed,
            ('D', false) if !fractional => values.days = signed,
            ('H', true) => {
                values.hours += duration_sign * amount.trunc();
                let total = (duration_sign * amount.fract() * NS_PER_HOUR).round();
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
            ('M', true) => {
                values.minutes += duration_sign * amount.trunc();
                let total = (duration_sign * amount.fract() * NS_PER_MINUTE).round();
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

fn parse_duration_seconds_part(
    number: &str,
    duration_sign: f64,
    values: &mut DurationValues,
) -> Option<()> {
    let mut parts = number.split('.');
    let whole = parts.next()?;
    let fraction = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let seconds = if whole.is_empty() {
        0.0
    } else {
        let value = whole.parse::<f64>().ok()?;
        if !value.is_finite() {
            return None;
        }
        value
    };
    values.seconds += duration_sign * seconds;
    let Some(fraction) = fraction else {
        return Some(());
    };
    if fraction.len() > 9 {
        return None;
    }
    let mut padded = fraction.to_string();
    while padded.len() < 9 {
        padded.push('0');
    }
    let subsecond_ns = if padded.is_empty() {
        0
    } else {
        padded.parse::<u32>().ok()?
    };
    values.milliseconds += duration_sign * (subsecond_ns / 1_000_000) as f64;
    values.microseconds += duration_sign * ((subsecond_ns / 1_000) % 1_000) as f64;
    values.nanoseconds += duration_sign * (subsecond_ns % 1_000) as f64;
    Some(())
}

fn temporal_duration_to_string(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Duration")?;
    // Duration uses the same ToSecondsStringPrecision option grammar as the
    // other Temporal stringifiers. Parse it even while the calendar-unit
    // formatter below remains intentionally compact so invalid options and
    // observable getter failures occur at the specification-required point.
    let options = temporal_string_options(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    if options.minute_only {
        return Err(VmError::range(
            "minute is not a valid Duration string smallestUnit",
        ));
    }
    let mut values = DurationValues {
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
    let original_seconds = values.seconds;
    let total_ns = values.seconds as i128 * NS_PER_SECOND_I128
        + values.milliseconds as i128 * NS_PER_MILLISECOND_I128
        + values.microseconds as i128 * 1_000
        + values.nanoseconds as i128;
    let rounded = round_signed_i128(total_ns, options.quantum, options.mode);
    let negative = rounded < 0;
    let mut remainder = rounded.unsigned_abs();
    let seconds = remainder / NS_PER_SECOND_I128 as u128;
    remainder %= NS_PER_SECOND_I128 as u128;
    let milliseconds = remainder / NS_PER_MILLISECOND_I128 as u128;
    remainder %= NS_PER_MILLISECOND_I128 as u128;
    let microseconds = remainder / 1_000;
    let nanoseconds = remainder % 1_000;
    let sign = if negative { -1.0 } else { 1.0 };
    values.seconds = sign * seconds as f64;
    values.milliseconds = sign * milliseconds as f64;
    values.microseconds = sign * microseconds as f64;
    values.nanoseconds = sign * nanoseconds as f64;
    if original_seconds.abs() < 60.0
        && values.seconds.abs() >= 60.0
        && (values.days != 0.0 || values.hours != 0.0 || values.minutes != 0.0)
    {
        values = balance_duration(values);
    }
    Ok(JsValue::String(format_duration(values, options.precision)))
}

fn temporal_duration_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_duration_to_string(vm, context, this_value, &[])
}

fn temporal_instant_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_instant_to_string(vm, context, this_value, &[])
}

fn temporal_plain_date_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_to_string(vm, context, this_value, &[])
}

fn temporal_plain_time_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_time_to_string(vm, context, this_value, &[])
}

fn temporal_plain_date_time_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_date_time_to_string(vm, context, this_value, &[])
}

fn temporal_plain_year_month_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_year_month_to_string(vm, context, this_value, &[])
}

fn temporal_plain_month_day_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_plain_month_day_to_string(vm, context, this_value, &[])
}

fn temporal_zoned_date_time_to_json(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    temporal_zoned_date_time_to_string(vm, context, this_value, &[])
}

fn push_duration_part(text: &mut String, value: f64, suffix: &str) {
    if value != 0.0 {
        text.push_str(&JsValue::Number(value).to_js_string().unwrap_or_default());
        text.push_str(suffix);
    }
}

fn format_duration(values: DurationValues, precision: Option<usize>) -> String {
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
    if values.seconds != 0.0 || subsecond_ns != 0 || precision.is_some() {
        time.push_str(
            &JsValue::Number(values.seconds)
                .to_js_string()
                .unwrap_or_default(),
        );
        if subsecond_ns != 0 || precision.unwrap_or(0) != 0 {
            let mut fraction = format!("{subsecond_ns:09}");
            if let Some(precision) = precision {
                fraction.truncate(precision);
            } else {
                while fraction.ends_with('0') {
                    fraction.pop();
                }
            }
            if !fraction.is_empty() {
                time.push('.');
                time.push_str(&fraction);
            }
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
    define_method(context, prototype, "toJSON", 0, temporal_instant_to_json)?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_to_locale_string,
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
                if context.value_object(&value).is_none() {
                    return Err(VmError::type_error(
                        "Temporal.Instant.from requires a string or Temporal object",
                    ));
                }
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

fn negate_temporal_round_mode(mode: TemporalRoundMode) -> TemporalRoundMode {
    match mode {
        TemporalRoundMode::Ceil => TemporalRoundMode::Floor,
        TemporalRoundMode::Floor => TemporalRoundMode::Ceil,
        TemporalRoundMode::HalfCeil => TemporalRoundMode::HalfFloor,
        TemporalRoundMode::HalfFloor => TemporalRoundMode::HalfCeil,
        mode => mode,
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
    if !number.is_finite() {
        return Err(VmError::range("invalid Temporal rounding increment"));
    }
    let integer = number.trunc();
    if !(1.0..=1_000_000_000.0).contains(&integer) {
        return Err(VmError::range("invalid Temporal rounding increment"));
    }
    Ok(integer as u64)
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
    if increment == 0 || increment >= maximum || maximum % increment != 0 {
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
    let time_zone_id = temporal_time_zone_string(vm, context, time_zone_value)?;
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
    define_method(context, prototype, "toJSON", 0, temporal_plain_date_to_json)?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_to_locale_string,
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
    define_temporal_calendar_getter(context, prototype, "era", "get era", "PlainDate")?;
    define_temporal_calendar_getter(context, prototype, "eraYear", "get eraYear", "PlainDate")?;
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
    let iso = Icu4xCalendarBackend
        .date_to_iso(
            &calendar_id,
            year.trunc() as i32,
            &month_code(month),
            day.trunc() as u8,
        )
        .map_err(VmError::range)?;
    create_temporal_object(
        context,
        prototype,
        "PlainDate",
        temporal_date_slots_from_iso(
            iso.year as f64,
            iso.month as f64,
            iso.day as f64,
            calendar_id,
        ),
    )
}

fn create_plain_date_with_calendar_from_iso(
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
        temporal_date_slots_from_iso(year, month, day, calendar_id),
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
    let year = vm
        .to_number(
            arguments.first().cloned().unwrap_or(JsValue::Undefined),
            context,
        )?
        .trunc();
    let month = vm
        .to_number(
            arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
            context,
        )?
        .trunc();
    let day = vm
        .to_number(
            arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
            context,
        )?
        .trunc();
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.get(3))?;
    validate_plain_date(year, month, day)?;
    create_plain_date_with_calendar_from_iso(context, prototype, year, month, day, calendar_id)
}

fn validate_plain_date(year: f64, month: f64, day: f64) -> Result<(), VmError> {
    if !year.is_finite()
        || !month.is_finite()
        || !day.is_finite()
        || year.fract() != 0.0
        || month.fract() != 0.0
        || day.fract() != 0.0
    {
        return Err(VmError::range("invalid Temporal.PlainDate"));
    }
    let year = year.trunc() as i32;
    let month = month.trunc() as u32;
    let day = day.trunc() as u32;
    if !(1..=12).contains(&month)
        || !(1..=month_day_count(year, month)).contains(&day)
        || !iso_date_within_temporal_range(year, month, day)
    {
        Err(VmError::range("invalid Temporal.PlainDate"))
    } else {
        Ok(())
    }
}

fn validate_plain_date_for_calendar(
    calendar: &str,
    year: f64,
    month: f64,
    day: f64,
) -> Result<(), VmError> {
    if !year.is_finite()
        || !month.is_finite()
        || !day.is_finite()
        || year.fract() != 0.0
        || month.fract() != 0.0
        || day.fract() != 0.0
    {
        return Err(VmError::range("invalid Temporal.PlainDate"));
    }
    let year = year.trunc() as i32;
    let month = month.trunc() as u32;
    let day = day.trunc() as u32;
    let months = calendar_months_in_year(calendar, year);
    if !(1..=months).contains(&month)
        || !(1..=calendar_month_day_count(calendar, year, month)).contains(&day)
        || !iso_date_within_temporal_range(calendar_iso_year(calendar, year), month, day)
    {
        Err(VmError::range("invalid Temporal.PlainDate"))
    } else {
        Ok(())
    }
}

/// Temporal PlainDate and the date portion of all built-in calendars are
/// bounded by the ISO date range corresponding to +/-100,000,000 days from the
/// epoch.  Checking the exact day (rather than only the year) is important for
/// the boundary dates -271821-04-19 and 275760-09-13.
fn iso_date_within_temporal_range(year: i32, month: u32, day: u32) -> bool {
    const MIN_DATE: (i32, u32, u32) = (-271_821, 4, 19);
    const MAX_DATE: (i32, u32, u32) = (275_760, 9, 13);
    (year, month, day) >= MIN_DATE && (year, month, day) <= MAX_DATE
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
    let parsed_string = if let JsValue::String(text) = &item {
        Some(
            parse_plain_date(text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainDate string"))?,
        )
    } else {
        None
    };
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (year, mut month, mut day, calendar_id, month_code_field) = match item {
        JsValue::String(_) => {
            let (year, month, day) = parsed_string.unwrap();
            (year, month, day, "iso8601".into(), None)
        }
        value => {
            let object = context.require_object(&value, "Temporal.PlainDate.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("PlainDate") {
                (
                    temporal_number_slot(context, object, "year"),
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "day"),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                    own_string(context, object, "monthCode"),
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
                    own_string(context, object, "monthCode"),
                )
            } else {
                let calendar_id = temporal_calendar_id_from_object(vm, context, object)?;
                let (month, month_code) =
                    temporal_required_month_fields_from_object(vm, context, object, &calendar_id)?;
                (
                    temporal_calendar_year_from_object(vm, context, object, &calendar_id)?,
                    month,
                    temporal_required_object_number(vm, context, object, "day")?,
                    calendar_id,
                    month_code,
                )
            }
        }
    };
    if !reject_overflow && parsed_string.is_none() {
        if ![year, month, day].into_iter().all(f64::is_finite) {
            return Err(VmError::range("invalid Temporal.PlainDate fields"));
        }
        month = month.trunc().clamp(
            1.0,
            calendar_months_in_year(&calendar_id, year as i32) as f64,
        );
        day = day.trunc().clamp(
            1.0,
            calendar_month_day_count(&calendar_id, year as i32, month as u32).max(1) as f64,
        );
    }
    if let Some(month_code) = month_code_field {
        let iso = Icu4xCalendarBackend
            .resolve_date_fields(
                &calendar_id,
                &CalendarDateFields {
                    year: year as i32,
                    month: None,
                    month_code: Some(month_code),
                    day: day as u8,
                },
            )
            .map_err(VmError::range)?;
        return create_plain_date_with_calendar_from_iso(
            context,
            prototype,
            iso.year as f64,
            iso.month as f64,
            iso.day as f64,
            calendar_id,
        );
    }
    validate_plain_date_for_calendar(&calendar_id, year, month, day)?;
    create_plain_date_with_calendar(context, prototype, year, month, day, calendar_id)
}

fn plain_date_order_key(context: &NativeContext, object: ObjectId) -> i64 {
    let year = temporal_number_slot(context, object, "year") as i32;
    let month = temporal_number_slot(context, object, "month") as u32;
    let day = temporal_number_slot(context, object, "day") as u32;
    let calendar = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    calendar_days_from_civil(&calendar, year, month, day)
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
        JsValue::Null => Err(VmError::type_error(
            "Temporal calendar must be a string or object",
        )),
        JsValue::Boolean(_) | JsValue::Number(_) | JsValue::BigInt(_) | JsValue::Symbol(_) => Err(
            VmError::type_error("Temporal calendar must be a string or object"),
        ),
        JsValue::Object(object) => {
            if matches!(
                own_string(context, object, TEMPORAL_KIND).as_deref(),
                Some(
                    "PlainDate"
                        | "PlainDateTime"
                        | "PlainMonthDay"
                        | "PlainYearMonth"
                        | "ZonedDateTime"
                )
            ) {
                Ok(own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()))
            } else {
                Err(VmError::type_error(
                    "Temporal calendar object is not supported by this calendar slot",
                ))
            }
        }
        value => {
            let text = vm.to_string_coerce(value, context)?;
            normalize_temporal_calendar_string(&text)
        }
    }
}

fn canonicalize_temporal_calendar_id(calendar: &str) -> String {
    match calendar {
        "islamicc" => "islamic-civil".into(),
        "ethiopic-amete-alem" => "ethioaa".into(),
        _ => calendar.into(),
    }
}

fn normalize_temporal_calendar_string(value: &str) -> Result<String, VmError> {
    if value.is_empty() || !value.is_ascii() {
        return Err(VmError::range("invalid Temporal calendar"));
    }
    let text = value.to_ascii_lowercase();
    // Temporal accepts an ISO date/time string in calendar positions and
    // interprets it as the built-in ISO calendar.
    if parse_temporal_plain_date_string(&text).is_some()
        || parse_plain_year_month(&text).is_some()
        || parse_plain_month_day(&text).is_some()
        || parse_plain_time(&text).is_some()
    {
        return Ok("iso8601".into());
    }
    let calendar = canonicalize_temporal_calendar_id(&text);
    const KNOWN_CALENDARS: &[&str] = &[
        "iso8601",
        "gregory",
        "japanese",
        "buddhist",
        "chinese",
        "dangi",
        "roc",
        "coptic",
        "ethiopic",
        "ethioaa",
        "hebrew",
        "indian",
        "persian",
        "islamic",
        "islamic-civil",
        "islamic-rgsa",
        "islamic-tbla",
        "islamic-umalqura",
    ];
    if KNOWN_CALENDARS.contains(&calendar.as_str()) {
        Ok(calendar)
    } else {
        Err(VmError::range("invalid Temporal calendar"))
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
    temporal_date_replacement_from_value(vm, context, value, name, current)
}

fn temporal_date_replacement_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
    name: &str,
    current: f64,
) -> Result<f64, VmError> {
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
        let value = temporal_string_or_object_to_string(
            vm,
            context,
            month_code_value,
            "Temporal monthCode property must be a string",
        )?;
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
    calendar: &str,
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
    let maximum_day = calendar_month_day_count(calendar, new_year, new_month);
    if reject_overflow && requested_day > maximum_day {
        return Err(VmError::range("Temporal date overflows the target month"));
    }
    let clamped_day = requested_day.min(maximum_day);
    let extra_days = (sign * (duration.weeks * 7.0 + duration.days)).trunc() as i64;
    let day_number = calendar_days_from_civil(calendar, new_year, new_month, clamped_day)
        .checked_add(extra_days)
        .filter(|value| temporal_day_number_within_range(*value))
        .ok_or_else(|| VmError::range("Temporal date is out of range"))?;
    let (year, month, day) = calendar_civil_from_days(calendar, day_number);
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
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "day".into()))?;
    let largest_unit = match largest.as_deref() {
        None | Some("auto") => {
            if temporal_unit_nanoseconds(&smallest_unit) > NS_PER_DAY_I128 {
                smallest_unit.clone()
            } else {
                "day".into()
            }
        }
        Some(unit) => normalize_temporal_unit(unit.to_string())?,
    };
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

fn temporal_object_iso_date(context: &NativeContext, object: ObjectId) -> Result<IsoDate, VmError> {
    if let (Some(year), Some(month), Some(day)) = (
        own_number(context, object, "isoYear"),
        own_number(context, object, "isoMonth"),
        own_number(context, object, "isoDay"),
    ) {
        return Ok(IsoDate {
            year: year as i32,
            month: month as u8,
            day: day as u8,
        });
    }

    let calendar = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let year = temporal_number_slot(context, object, "year") as i32;
    let day = temporal_number_slot(context, object, "day") as u8;
    let month_code = own_string(context, object, "monthCode").unwrap_or_else(|| {
        format!(
            "M{:02}",
            temporal_number_slot(context, object, "month") as u8
        )
    });
    Icu4xCalendarBackend
        .date_to_iso(&calendar, year, &month_code, day)
        .map_err(VmError::range)
}

fn calendar_duration_component(value: f64, sign: f64) -> Result<i32, VmError> {
    let signed = value * sign;
    if signed < i32::MIN as f64 || signed > i32::MAX as f64 {
        return Err(VmError::range("Temporal duration is out of range"));
    }
    Ok(signed as i32)
}

fn add_calendar_months(
    calendar: &str,
    year: i32,
    month: u32,
    day: u32,
    delta: i64,
) -> (i32, u32, u32) {
    let index = year as i64 * 12 + month as i64 - 1 + delta;
    let result_year = index.div_euclid(12) as i32;
    let result_month = index.rem_euclid(12) as u32 + 1;
    let result_day = day.min(calendar_month_day_count(
        calendar,
        result_year,
        result_month,
    ));
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
    let calendar = own_string(context, start, "calendarId").unwrap_or_else(|| "iso8601".into());
    let start_number = calendar_days_from_civil(&calendar, start_year, start_month, start_day);
    let end_number = calendar_days_from_civil(&calendar, end_year, end_month, end_day);
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
    let mut candidate = add_calendar_months(&calendar, start_year, start_month, start_day, months);
    let mut candidate_number =
        calendar_days_from_civil(&calendar, candidate.0, candidate.1, candidate.2);
    if direction > 0 && candidate_number > end_number {
        months -= 1;
        candidate = add_calendar_months(&calendar, start_year, start_month, start_day, months);
        candidate_number =
            calendar_days_from_civil(&calendar, candidate.0, candidate.1, candidate.2);
    } else if direction < 0 && candidate_number < end_number {
        months += 1;
        candidate = add_calendar_months(&calendar, start_year, start_month, start_day, months);
        candidate_number =
            calendar_days_from_civil(&calendar, candidate.0, candidate.1, candidate.2);
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

fn calendar_date_difference_values(
    context: &NativeContext,
    start: ObjectId,
    end: ObjectId,
    largest_unit: &str,
) -> Result<DurationValues, VmError> {
    let calendar = own_string(context, start, "calendarId").unwrap_or_else(|| "iso8601".into());
    let largest_unit = match largest_unit {
        "year" => CalendarLargestUnit::Year,
        "month" => CalendarLargestUnit::Month,
        "week" => CalendarLargestUnit::Week,
        "day" => CalendarLargestUnit::Day,
        _ => return Err(VmError::range("invalid calendar difference unit")),
    };
    let result = Icu4xCalendarBackend
        .date_until(
            &calendar,
            temporal_object_iso_date(context, start)?,
            temporal_object_iso_date(context, end)?,
            largest_unit,
        )
        .map_err(VmError::range)?;
    Ok(DurationValues {
        years: result.years as f64,
        months: result.months as f64,
        weeks: result.weeks as f64,
        days: result.days as f64,
        ..DurationValues::default()
    })
}

fn signed_duration_time_from_nanoseconds(value: i128) -> DurationValues {
    let sign = if value < 0 { -1.0 } else { 1.0 };
    let fields = plain_time_from_nanoseconds_i128(value.unsigned_abs() as i128);
    DurationValues {
        hours: fields.hour * sign,
        minutes: fields.minute * sign,
        seconds: fields.second * sign,
        milliseconds: fields.millisecond * sign,
        microseconds: fields.microsecond * sign,
        nanoseconds: fields.nanosecond * sign,
        ..DurationValues::default()
    }
}

fn zoned_date_time_calendar_difference_values(
    context: &NativeContext,
    start: ObjectId,
    end: ObjectId,
    largest_unit: &str,
) -> DurationValues {
    let (start_year, start_month, start_day) = date_parts(context, start);
    let (end_year, end_month, end_day) = date_parts(context, end);
    let calendar = own_string(context, start, "calendarId").unwrap_or_else(|| "iso8601".into());
    let start_day_number = calendar_days_from_civil(&calendar, start_year, start_month, start_day);
    let end_day_number = calendar_days_from_civil(&calendar, end_year, end_month, end_day);
    let start_time = plain_time_nanoseconds_i128(plain_date_time_values(context, start));
    let end_time = plain_time_nanoseconds_i128(plain_date_time_values(context, end));
    let total_ns =
        (end_day_number - start_day_number) as i128 * NS_PER_DAY_I128 + end_time - start_time;
    if largest_unit == "day" {
        let days = total_ns / NS_PER_DAY_I128;
        let remainder = total_ns % NS_PER_DAY_I128;
        let mut values = signed_duration_time_from_nanoseconds(remainder);
        values.days = days as f64;
        return values;
    }
    if largest_unit == "week" {
        let total_days = total_ns / NS_PER_DAY_I128;
        let remainder = total_ns % NS_PER_DAY_I128;
        let mut values = signed_duration_time_from_nanoseconds(remainder);
        values.weeks = (total_days / 7) as f64;
        values.days = (total_days % 7) as f64;
        return values;
    }

    let mut months =
        (end_year as i64 * 12 + end_month as i64) - (start_year as i64 * 12 + start_month as i64);
    let direction = total_ns.signum();
    let candidate_for_months = |month_delta: i64| {
        let candidate =
            add_calendar_months(&calendar, start_year, start_month, start_day, month_delta);
        let candidate_day_number =
            calendar_days_from_civil(&calendar, candidate.0, candidate.1, candidate.2);
        (candidate_day_number - start_day_number) as i128 * NS_PER_DAY_I128
    };
    let mut candidate_ns = candidate_for_months(months);
    if direction > 0 && candidate_ns > total_ns {
        months -= 1;
        candidate_ns = candidate_for_months(months);
    } else if direction < 0 && candidate_ns < total_ns {
        months += 1;
        candidate_ns = candidate_for_months(months);
    }
    let remainder = total_ns - candidate_ns;
    let days = remainder / NS_PER_DAY_I128;
    let time_remainder = remainder % NS_PER_DAY_I128;
    let mut values = signed_duration_time_from_nanoseconds(time_remainder);
    values.days = days as f64;
    if largest_unit == "year" {
        values.years = (months / 12) as f64;
        values.months = (months % 12) as f64;
    } else {
        values.months = months as f64;
    }
    values
}

fn balance_zoned_calendar_days(
    context: &NativeContext,
    start: ObjectId,
    largest_unit: &str,
    values: &mut DurationValues,
) {
    if !matches!(largest_unit, "year" | "month") || values.days == 0.0 {
        return;
    }
    let (year, month, day) = date_parts(context, start);
    let calendar = own_string(context, start, "calendarId").unwrap_or_else(|| "iso8601".into());
    let mut total_months = values.years as i64 * 12 + values.months as i64;
    if values.days > 0.0 {
        loop {
            let current = add_calendar_months(&calendar, year, month, day, total_months);
            let next = add_calendar_months(&calendar, year, month, day, total_months + 1);
            let current_number =
                calendar_days_from_civil(&calendar, current.0, current.1, current.2);
            let next_number = calendar_days_from_civil(&calendar, next.0, next.1, next.2);
            let span = (next_number - current_number) as f64;
            if values.days < span {
                break;
            }
            total_months += 1;
            values.days -= span;
        }
    } else {
        loop {
            let current = add_calendar_months(&calendar, year, month, day, total_months);
            let previous = add_calendar_months(&calendar, year, month, day, total_months - 1);
            let current_number =
                calendar_days_from_civil(&calendar, current.0, current.1, current.2);
            let previous_number =
                calendar_days_from_civil(&calendar, previous.0, previous.1, previous.2);
            let span = (current_number - previous_number) as f64;
            if -values.days < span {
                break;
            }
            total_months -= 1;
            values.days += span;
        }
    }
    if largest_unit == "year" {
        values.years = (total_months / 12) as f64;
        values.months = (total_months % 12) as f64;
    } else {
        values.months = total_months as f64;
    }
}

fn round_temporal_quantity(value: f64, increment: u64, mode: TemporalRoundMode) -> f64 {
    let increment = increment as f64;
    let scaled = value / increment;
    let floor = scaled.floor();
    let ceil = scaled.ceil();
    let trunc = scaled.trunc();
    let expand = if scaled.is_sign_negative() {
        floor
    } else {
        ceil
    };
    let rounded = match mode {
        TemporalRoundMode::Floor => floor,
        TemporalRoundMode::Ceil => ceil,
        TemporalRoundMode::Trunc => trunc,
        TemporalRoundMode::Expand => expand,
        TemporalRoundMode::HalfCeil
        | TemporalRoundMode::HalfExpand
        | TemporalRoundMode::HalfFloor
        | TemporalRoundMode::HalfTrunc
        | TemporalRoundMode::HalfEven => {
            let lower_distance = scaled - floor;
            let upper_distance = ceil - scaled;
            if lower_distance < upper_distance {
                floor
            } else if upper_distance < lower_distance {
                ceil
            } else {
                match mode {
                    TemporalRoundMode::HalfCeil => ceil,
                    TemporalRoundMode::HalfFloor => floor,
                    TemporalRoundMode::HalfTrunc => trunc,
                    TemporalRoundMode::HalfExpand => expand,
                    TemporalRoundMode::HalfEven => {
                        if (floor as i128).rem_euclid(2) == 0 {
                            floor
                        } else {
                            ceil
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
    };
    rounded * increment
}

fn relative_date_unit_quantity(
    context: &NativeContext,
    start: ObjectId,
    end: ObjectId,
    unit: &str,
) -> f64 {
    let (start_year, start_month, start_day) = date_parts(context, start);
    let (end_year, end_month, end_day) = date_parts(context, end);
    let calendar = own_string(context, start, "calendarId").unwrap_or_else(|| "iso8601".into());
    let start_number = calendar_days_from_civil(&calendar, start_year, start_month, start_day);
    let end_number = calendar_days_from_civil(&calendar, end_year, end_month, end_day);
    let total_days = end_number - start_number;
    match unit {
        "day" => total_days as f64,
        "week" => total_days as f64 / 7.0,
        "month" | "year" => {
            let values = iso_date_difference_values(context, start, end, unit);
            let whole_months = if unit == "year" {
                values.years as i64 * 12
            } else {
                values.months as i64
            };
            let candidate =
                add_calendar_months(&calendar, start_year, start_month, start_day, whole_months);
            let candidate_number =
                calendar_days_from_civil(&calendar, candidate.0, candidate.1, candidate.2);
            let direction = total_days.signum();
            if direction == 0 {
                return 0.0;
            }
            let step_months = if unit == "year" { 12 } else { 1 };
            let next = add_calendar_months(
                &calendar,
                start_year,
                start_month,
                start_day,
                whole_months + direction * step_months,
            );
            let next_number = calendar_days_from_civil(&calendar, next.0, next.1, next.2);
            let fraction = (end_number - candidate_number) as f64
                / (next_number - candidate_number).unsigned_abs() as f64;
            if unit == "year" {
                whole_months as f64 / 12.0 + fraction
            } else {
                whole_months as f64 + fraction
            }
        }
        _ => unreachable!("validated date difference unit"),
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
    let calendar_id = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let iso = temporal_object_iso_date(context, object)?;
    let time_days = (sign * duration_time_nanoseconds(duration) / NS_PER_DAY).trunc();
    let days = duration.days * sign + time_days;
    let result = Icu4xCalendarBackend
        .add_date(
            &calendar_id,
            iso,
            CalendarDuration {
                years: calendar_duration_component(duration.years, sign)?,
                months: calendar_duration_component(duration.months, sign)?,
                weeks: calendar_duration_component(duration.weeks, sign)?,
                days: calendar_duration_component(days, 1.0)?,
            },
            if reject_overflow {
                CalendarOverflow::Reject
            } else {
                CalendarOverflow::Constrain
            },
        )
        .map_err(VmError::range)?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date_with_calendar_from_iso(
        context,
        prototype,
        result.year as f64,
        result.month as f64,
        result.day as f64,
        calendar_id,
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
    let calendar_id =
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let year = temporal_calendar_year_replacement(
        vm,
        context,
        replacement,
        &calendar_id,
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
        &["day", "era", "eraYear", "month", "monthCode", "year"],
    )?;
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let month = if reject_overflow {
        month
    } else {
        month.clamp(
            1.0,
            calendar_months_in_year(&calendar_id, year as i32) as f64,
        )
    };
    let day = if reject_overflow {
        day
    } else {
        day.clamp(
            1.0,
            calendar_month_day_count(&calendar_id, year as i32, month as u32).max(1) as f64,
        )
    };
    validate_plain_date_for_calendar(&calendar_id, year, month, day)?;
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    create_plain_date_with_calendar(context, prototype, year, month, day, calendar_id)
}

fn temporal_plain_date_with_calendar(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDate")?;
    if matches!(arguments.first(), None | Some(JsValue::Undefined)) {
        return Err(VmError::type_error(
            "Temporal.PlainDate.prototype.withCalendar requires a calendar",
        ));
    }
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
    // Temporal defines `since` as the exact sign inverse of `until` for the
    // same ordered pair. Calendar difference algorithms are not generally
    // symmetric around constrained month ends, so do not recompute with
    // swapped operands: compute once in the forward direction and negate.
    let (start, end) = (this_object, other_object);
    let mut values = calendar_date_difference_values(context, start, end, &options.largest_unit)?;
    if options.smallest_unit != "day" || options.increment > 1 {
        let quantity = relative_date_unit_quantity(context, start, end, &options.smallest_unit);
        let rounded = round_temporal_quantity(quantity, options.increment, options.mode);
        values = match options.smallest_unit.as_str() {
            "year" => DurationValues {
                years: rounded,
                ..DurationValues::default()
            },
            "month" if options.largest_unit == "year" => DurationValues {
                years: (rounded / 12.0).trunc(),
                months: rounded % 12.0,
                ..DurationValues::default()
            },
            "month" => DurationValues {
                months: rounded,
                ..DurationValues::default()
            },
            "week" => DurationValues {
                weeks: rounded,
                ..DurationValues::default()
            },
            "day" if options.largest_unit == "week" => DurationValues {
                weeks: (rounded / 7.0).trunc(),
                days: rounded % 7.0,
                ..DurationValues::default()
            },
            "day" if options.largest_unit == "day" => DurationValues {
                days: rounded,
                ..DurationValues::default()
            },
            "day" => values,
            _ => unreachable!("validated date difference unit"),
        };
    }
    if reverse {
        values = values.map(|value| -value);
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
    let time_zone_id = temporal_time_zone_string(vm, context, time_zone_value)?;
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
    define_method(context, prototype, "toJSON", 0, temporal_plain_time_to_json)?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_to_locale_string,
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
        hour: temporal_number_or_default(vm, context, arguments, 0)?,
        minute: temporal_number_or_default(vm, context, arguments, 1)?,
        second: temporal_number_or_default(vm, context, arguments, 2)?,
        millisecond: temporal_number_or_default(vm, context, arguments, 3)?,
        microsecond: temporal_number_or_default(vm, context, arguments, 4)?,
        nanosecond: temporal_number_or_default(vm, context, arguments, 5)?,
    })
}

fn temporal_number_or_default(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    index: usize,
) -> Result<f64, VmError> {
    match arguments.get(index) {
        None | Some(JsValue::Undefined) => Ok(0.0),
        Some(value) => vm.to_number(value.clone(), context),
    }
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
    let body = validate_temporal_annotations(text.trim())?;
    let separator = body
        .rfind('T')
        .or_else(|| body.rfind('t'))
        .or_else(|| body.rfind(' '));
    let time = if let Some(index) = separator {
        let date = &body[..index];
        if !date.is_empty() && parse_temporal_date_part(date).is_none() {
            return None;
        }
        &body[index + 1..]
    } else {
        if !body.starts_with(['T', 't']) && is_ambiguous_plain_time_string(body) {
            return None;
        }
        body.strip_prefix('T')
            .or_else(|| body.strip_prefix('t'))
            .unwrap_or(body)
    };
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
        parse_iso_time_zone_offset_ns(&time[offset_start..])?;
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
        || (!fraction_text.is_empty() && compact.len() != 6)
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

fn is_ambiguous_plain_time_string(text: &str) -> bool {
    let text = text.split('[').next().unwrap_or(text);
    if text.len() == 4 && text.chars().all(|ch| ch.is_ascii_digit()) {
        let month = parse_fixed_digits(&text[..2], 2).unwrap_or(0);
        let day = parse_fixed_digits(&text[2..], 2).unwrap_or(0);
        return (1..=12).contains(&month) && (1..=month_day_count(2020, month)).contains(&day);
    }
    if text.len() == 5 && text.as_bytes().get(2).is_some_and(|ch| *ch == b'-') {
        let month = parse_fixed_digits(&text[..2], 2).unwrap_or(0);
        let day = parse_fixed_digits(&text[3..], 2).unwrap_or(0);
        return (1..=12).contains(&month) && (1..=month_day_count(2020, month)).contains(&day);
    }
    if text.len() == 6 && text.chars().all(|ch| ch.is_ascii_digit()) {
        let month = parse_fixed_digits(&text[4..], 2).unwrap_or(0);
        return (1..=12).contains(&month);
    }
    if text.len() == 7 && text.as_bytes().get(4).is_some_and(|ch| *ch == b'-') {
        let month = parse_fixed_digits(&text[5..], 2).unwrap_or(0);
        return (1..=12).contains(&month);
    }
    false
}

fn parse_plain_date_time(text: &str) -> Option<(f64, f64, f64, PlainTimeValues)> {
    let body = validate_iso_calendar_annotations(text.trim())?;
    let index = body.find(['T', 't', ' '])?;
    let (date, time) = (&body[..index], &body[index + 1..]);
    let (year, month, day) = parse_temporal_date_part(date)?;
    let time = parse_plain_time(time)?;
    Some((year as f64, month as f64, day as f64, time))
}

fn temporal_plain_time_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let parsed_string = if let JsValue::String(text) = &item {
        Some(parse_plain_time(text).ok_or_else(|| VmError::range("invalid Temporal.PlainTime"))?)
    } else {
        None
    };
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let mut values = match item {
        JsValue::String(_) => parsed_string.unwrap(),
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
    if !reject_overflow && parsed_string.is_none() {
        if ![
            values.hour,
            values.minute,
            values.second,
            values.millisecond,
            values.microsecond,
            values.nanosecond,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(VmError::range("invalid Temporal.PlainTime fields"));
        }
        values.hour = values.hour.trunc().clamp(0.0, 23.0);
        values.minute = values.minute.trunc().clamp(0.0, 59.0);
        values.second = values.second.trunc().clamp(0.0, 59.0);
        values.millisecond = values.millisecond.trunc().clamp(0.0, 999.0);
        values.microsecond = values.microsecond.trunc().clamp(0.0, 999.0);
        values.nanosecond = values.nanosecond.trunc().clamp(0.0, 999.0);
    }
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
            let hour = temporal_partial_number(vm, context, object, "hour")?;
            let microsecond = temporal_partial_number(vm, context, object, "microsecond")?;
            let millisecond = temporal_partial_number(vm, context, object, "millisecond")?;
            let minute = temporal_partial_number(vm, context, object, "minute")?;
            let nanosecond = temporal_partial_number(vm, context, object, "nanosecond")?;
            let second = temporal_partial_number(vm, context, object, "second")?;
            if [hour, microsecond, millisecond, minute, nanosecond, second]
                .iter()
                .all(Option::is_none)
            {
                return Err(VmError::type_error(
                    "Temporal.PlainTime-like object has no time properties",
                ));
            }
            Ok(PlainTimeValues {
                hour: hour.unwrap_or(0.0),
                minute: minute.unwrap_or(0.0),
                second: constrain_time_second(second.unwrap_or(0.0)),
                millisecond: millisecond.unwrap_or(0.0),
                microsecond: microsecond.unwrap_or(0.0),
                nanosecond: nanosecond.unwrap_or(0.0),
            })
        }
    }
}

fn constrain_time_second(second: f64) -> f64 {
    if second == 60.0 { 59.0 } else { second }
}

fn temporal_partial_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<Option<f64>, VmError> {
    let value = temporal_get_property(vm, context, object, name)?;
    if matches!(value, JsValue::Undefined) {
        Ok(None)
    } else {
        Ok(Some(vm.to_number(value, context)?))
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
    let fractional_digits = match fractional_value {
        JsValue::Undefined => None,
        JsValue::Number(number) => {
            let digits = number.floor();
            if !number.is_finite() || !(0.0..=9.0).contains(&digits) {
                return Err(VmError::range("invalid fractionalSecondDigits"));
            }
            Some(digits as usize)
        }
        value => {
            let text = vm.to_string_coerce(value, context)?;
            if text == "auto" {
                None
            } else {
                return Err(VmError::range("invalid fractionalSecondDigits"));
            }
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

fn create_plain_time_from_total_nanoseconds_i128(
    context: &mut NativeContext,
    prototype: ObjectId,
    total: i128,
) -> Result<JsValue, VmError> {
    let mut remainder = total.rem_euclid(NS_PER_DAY_I128);
    let hour = remainder / NS_PER_HOUR_I128;
    remainder %= NS_PER_HOUR_I128;
    let minute = remainder / NS_PER_MINUTE_I128;
    remainder %= NS_PER_MINUTE_I128;
    let second = remainder / NS_PER_SECOND_I128;
    remainder %= NS_PER_SECOND_I128;
    let millisecond = remainder / NS_PER_MILLISECOND_I128;
    remainder %= NS_PER_MILLISECOND_I128;
    let microsecond = remainder / 1_000;
    let nanosecond = remainder % 1_000;
    create_plain_time(
        context,
        prototype,
        PlainTimeValues {
            hour: hour as f64,
            minute: minute as f64,
            second: second as f64,
            millisecond: millisecond as f64,
            microsecond: microsecond as f64,
            nanosecond: nanosecond as f64,
        },
    )
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
    let total = plain_time_nanoseconds_i128(plain_time_values_from_temporal(context, object))
        + sign as i128 * duration_time_nanoseconds_i128(duration);
    let prototype = temporal_constructor_prototype(context, "PlainTime")?;
    create_plain_time_from_total_nanoseconds_i128(context, prototype, total)
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
        temporal_plain_date_time_to_json,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_to_locale_string,
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
    define_temporal_calendar_getter(context, prototype, "era", "get era", "PlainDateTime")?;
    define_temporal_calendar_getter(
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
    let year = vm
        .to_number(
            arguments.first().cloned().unwrap_or(JsValue::Undefined),
            context,
        )?
        .trunc();
    let month = vm
        .to_number(
            arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
            context,
        )?
        .trunc();
    let day = vm
        .to_number(
            arguments.get(2).cloned().unwrap_or(JsValue::Undefined),
            context,
        )?
        .trunc();
    let time = plain_time_from_args(vm, context, arguments.get(3..).unwrap_or(&[]))?;
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.get(9))?;
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    create_plain_date_time_with_calendar_from_iso(
        context,
        prototype,
        year,
        month,
        day,
        time,
        calendar_id,
    )
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
    let iso = Icu4xCalendarBackend
        .date_to_iso(
            &calendar_id,
            year.trunc() as i32,
            &month_code(month),
            day.trunc() as u8,
        )
        .map_err(VmError::range)?;
    create_plain_date_time_with_calendar_from_iso(
        context,
        prototype,
        iso.year as f64,
        iso.month as f64,
        iso.day as f64,
        time,
        calendar_id,
    )
}

fn create_plain_date_time_with_calendar_from_iso(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
    time: PlainTimeValues,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    let mut slots = temporal_date_slots_from_iso(year, month, day, calendar_id);
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
    let parsed_string = if let JsValue::String(text) = &item {
        Some(
            parse_plain_date_time(text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainDateTime"))?,
        )
    } else {
        None
    };
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (year, mut month, mut day, mut time, calendar_id, month_code_field) = match item {
        JsValue::String(_) => {
            let (year, month, day, time) = parsed_string.unwrap();
            (year, month, day, time, "iso8601".into(), None)
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
                    own_string(context, object, "monthCode"),
                )
            } else if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("PlainDate") {
                (
                    temporal_number_slot(context, object, "year"),
                    temporal_number_slot(context, object, "month"),
                    temporal_number_slot(context, object, "day"),
                    PlainTimeValues::default(),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                    own_string(context, object, "monthCode"),
                )
            } else {
                let calendar_id = temporal_calendar_id_from_object(vm, context, object)?;
                let (month, month_code) =
                    temporal_required_month_fields_from_object(vm, context, object, &calendar_id)?;
                (
                    temporal_calendar_year_from_object(vm, context, object, &calendar_id)?,
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
                    calendar_id,
                    month_code,
                )
            }
        }
    };
    if !reject_overflow && parsed_string.is_none() {
        if ![
            year,
            month,
            day,
            time.hour,
            time.minute,
            time.second,
            time.millisecond,
            time.microsecond,
            time.nanosecond,
        ]
        .into_iter()
        .all(f64::is_finite)
        {
            return Err(VmError::range("invalid Temporal.PlainDateTime fields"));
        }
        month = month.trunc().clamp(
            1.0,
            calendar_months_in_year(&calendar_id, year as i32) as f64,
        );
        day = day.trunc().clamp(
            1.0,
            calendar_month_day_count(&calendar_id, year as i32, month as u32).max(1) as f64,
        );
        time.hour = time.hour.trunc().clamp(0.0, 23.0);
        time.minute = time.minute.trunc().clamp(0.0, 59.0);
        time.second = time.second.trunc().clamp(0.0, 59.0);
        time.millisecond = time.millisecond.trunc().clamp(0.0, 999.0);
        time.microsecond = time.microsecond.trunc().clamp(0.0, 999.0);
        time.nanosecond = time.nanosecond.trunc().clamp(0.0, 999.0);
    }
    validate_plain_time(time)?;
    if let Some(month_code) = month_code_field {
        let iso = Icu4xCalendarBackend
            .resolve_date_fields(
                &calendar_id,
                &CalendarDateFields {
                    year: year as i32,
                    month: None,
                    month_code: Some(month_code),
                    day: day as u8,
                },
            )
            .map_err(VmError::range)?;
        return create_plain_date_time_with_calendar_from_iso(
            context,
            prototype,
            iso.year as f64,
            iso.month as f64,
            iso.day as f64,
            time,
            calendar_id,
        );
    }
    validate_plain_date_for_calendar(&calendar_id, year, month, day)?;
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

fn balance_plain_time_nanoseconds_i128(total: i128) -> Result<(i64, PlainTimeValues), VmError> {
    let days = total.div_euclid(NS_PER_DAY_I128);
    let days = i64::try_from(days)
        .map_err(|_| VmError::range("Temporal duration time portion is out of range"))?;
    let mut remainder = total.rem_euclid(NS_PER_DAY_I128);
    let hour = remainder / NS_PER_HOUR_I128;
    remainder %= NS_PER_HOUR_I128;
    let minute = remainder / NS_PER_MINUTE_I128;
    remainder %= NS_PER_MINUTE_I128;
    let second = remainder / NS_PER_SECOND_I128;
    remainder %= NS_PER_SECOND_I128;
    let millisecond = remainder / NS_PER_MILLISECOND_I128;
    remainder %= NS_PER_MILLISECOND_I128;
    let microsecond = remainder / 1_000;
    let nanosecond = remainder % 1_000;
    Ok((
        days,
        PlainTimeValues {
            hour: hour as f64,
            minute: minute as f64,
            second: second as f64,
            millisecond: millisecond as f64,
            microsecond: microsecond as f64,
            nanosecond: nanosecond as f64,
        },
    ))
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
    let calendar_id = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let time_ns = plain_time_nanoseconds_i128(plain_date_time_values(context, object))
        + sign as i128 * duration_time_nanoseconds_i128(duration);
    let (extra_days, time) = balance_plain_time_nanoseconds_i128(time_ns)?;
    let date = Icu4xCalendarBackend
        .add_date(
            &calendar_id,
            temporal_object_iso_date(context, object)?,
            CalendarDuration {
                years: calendar_duration_component(duration.years, sign)?,
                months: calendar_duration_component(duration.months, sign)?,
                weeks: calendar_duration_component(duration.weeks, sign)?,
                days: calendar_duration_component(duration.days * sign + extra_days as f64, 1.0)?,
            },
            if reject_overflow {
                CalendarOverflow::Reject
            } else {
                CalendarOverflow::Constrain
            },
        )
        .map_err(VmError::range)?;
    validate_plain_time(time)?;
    if (date.year, date.month as u32, date.day as u32) == (-271_821, 4, 19)
        && time_nanoseconds(time) < 1.0
    {
        return Err(VmError::range("Temporal.PlainDateTime is out of range"));
    }
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar_from_iso(
        context,
        prototype,
        date.year as f64,
        date.month as f64,
        date.day as f64,
        time,
        calendar_id,
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
    let calendar_id =
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let year = temporal_calendar_year_replacement(
        vm,
        context,
        replacement,
        &calendar_id,
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
            "era",
            "eraYear",
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
        month.clamp(
            1.0,
            calendar_months_in_year(&calendar_id, year as i32) as f64,
        )
    };
    let day = if reject_overflow {
        day
    } else {
        day.clamp(
            1.0,
            calendar_month_day_count(&calendar_id, year as i32, month as u32).max(1) as f64,
        )
    };
    if !reject_overflow {
        time.hour = time.hour.clamp(0.0, 23.0);
        time.minute = time.minute.clamp(0.0, 59.0);
        time.second = time.second.clamp(0.0, 59.0);
        time.millisecond = time.millisecond.clamp(0.0, 999.0);
        time.microsecond = time.microsecond.clamp(0.0, 999.0);
        time.nanosecond = time.nanosecond.clamp(0.0, 999.0);
    }
    validate_plain_date_for_calendar(&calendar_id, year, month, day)?;
    validate_plain_time(time)?;
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    create_plain_date_time_with_calendar(context, prototype, year, month, day, time, calendar_id)
}

fn temporal_plain_date_time_with_calendar(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    if matches!(arguments.first(), None | Some(JsValue::Undefined)) {
        return Err(VmError::type_error(
            "Temporal.PlainDateTime.prototype.withCalendar requires a calendar",
        ));
    }
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
    if matches!(
        options.smallest_unit.as_str(),
        "year" | "month" | "week" | "day"
    ) {
        let (start, end) = (this_object, other_object);
        let calendar_values =
            iso_date_difference_values(context, start, end, &options.largest_unit);
        let same_local_time = plain_time_nanoseconds_i128(plain_date_time_values(context, start))
            == plain_time_nanoseconds_i128(plain_date_time_values(context, end));
        let exact_larger_unit = same_local_time
            && match options.smallest_unit.as_str() {
                "year" => {
                    calendar_values.months == 0.0
                        && calendar_values.weeks == 0.0
                        && calendar_values.days == 0.0
                }
                "month" => calendar_values.weeks == 0.0 && calendar_values.days == 0.0,
                "week" => calendar_values.days == 0.0,
                "day" => true,
                _ => false,
            };
        if exact_larger_unit {
            let values = if sign < 0 {
                calendar_values.map(|value| -value)
            } else {
                calendar_values
            };
            return create_duration_with_default_prototype(context, values);
        }
        let quantity =
            relative_plain_date_time_unit_quantity(context, start, end, &options.smallest_unit);
        let rounded = round_temporal_quantity(quantity, options.increment, options.mode);
        let values = match options.smallest_unit.as_str() {
            "year" => DurationValues {
                years: rounded,
                ..DurationValues::default()
            },
            "month" if options.largest_unit == "year" => DurationValues {
                years: (rounded / 12.0).trunc(),
                months: rounded % 12.0,
                ..DurationValues::default()
            },
            "month" => DurationValues {
                months: rounded,
                ..DurationValues::default()
            },
            "week" => DurationValues {
                weeks: rounded,
                ..DurationValues::default()
            },
            "day" => DurationValues {
                days: rounded,
                ..DurationValues::default()
            },
            _ => unreachable!(),
        };
        return create_duration_with_default_prototype(context, values);
    }
    let quantum = temporal_unit_nanoseconds(&options.smallest_unit) * options.increment as i128;
    let rounded = round_signed_i128(sign * (other_ns - this_ns), quantum, options.mode);
    create_duration_from_nanoseconds(context, rounded, &options.largest_unit)
}

fn relative_plain_date_time_unit_quantity(
    context: &NativeContext,
    start: ObjectId,
    end: ObjectId,
    unit: &str,
) -> f64 {
    let start_time = plain_time_nanoseconds_i128(plain_date_time_values(context, start));
    let end_time = plain_time_nanoseconds_i128(plain_date_time_values(context, end));
    let start_day = plain_date_order_key(context, start) as i128;
    let end_day = plain_date_order_key(context, end) as i128;
    let start_ns = start_day * NS_PER_DAY_I128 + start_time;
    let end_ns = end_day * NS_PER_DAY_I128 + end_time;
    let total_ns = end_ns - start_ns;
    match unit {
        "day" => total_ns as f64 / NS_PER_DAY_I128 as f64,
        "week" => total_ns as f64 / (7 * NS_PER_DAY_I128) as f64,
        "month" | "year" => {
            let (start_year, start_month, start_date) = date_parts(context, start);
            let calendar =
                own_string(context, start, "calendarId").unwrap_or_else(|| "iso8601".into());
            let date_values = iso_date_difference_values(context, start, end, unit);
            let mut whole_months = if unit == "year" {
                date_values.years as i64 * 12
            } else {
                date_values.months as i64
            };
            let direction = total_ns.signum() as i64;
            if direction == 0 {
                return 0.0;
            }
            let candidate_ns = |months: i64| {
                let candidate =
                    add_calendar_months(&calendar, start_year, start_month, start_date, months);
                calendar_days_from_civil(&calendar, candidate.0, candidate.1, candidate.2) as i128
                    * NS_PER_DAY_I128
                    + start_time
            };
            let mut candidate = candidate_ns(whole_months);
            if direction > 0 && candidate > end_ns {
                whole_months -= if unit == "year" { 12 } else { 1 };
                candidate = candidate_ns(whole_months);
            } else if direction < 0 && candidate < end_ns {
                whole_months += if unit == "year" { 12 } else { 1 };
                candidate = candidate_ns(whole_months);
            }
            let step = direction * if unit == "year" { 12 } else { 1 };
            let next = candidate_ns(whole_months + step);
            let fraction = (end_ns - candidate) as f64 / (next - candidate).unsigned_abs() as f64;
            if unit == "year" {
                whole_months as f64 / 12.0 + fraction
            } else {
                whole_months as f64 + fraction
            }
        }
        _ => unreachable!("validated PlainDateTime date unit"),
    }
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
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "nanosecond".into()))?;
    let largest_unit = match largest.as_deref() {
        None | Some("auto") => {
            let default = normalize_temporal_unit(default_largest_unit.to_string())?;
            if temporal_unit_nanoseconds(&smallest_unit) > temporal_unit_nanoseconds(&default) {
                smallest_unit.clone()
            } else {
                default
            }
        }
        Some(unit) => normalize_temporal_unit(unit.to_string())?,
    };
    if temporal_unit_nanoseconds(&largest_unit) < temporal_unit_nanoseconds(&smallest_unit) {
        return Err(VmError::range(
            "largestUnit must not be smaller than smallestUnit",
        ));
    }
    if !allow_day
        && (matches!(largest_unit.as_str(), "year" | "month" | "week" | "day")
            || matches!(smallest_unit.as_str(), "year" | "month" | "week" | "day"))
    {
        return Err(VmError::range("invalid PlainTime difference unit"));
    }
    let maximum = match smallest_unit.as_str() {
        "day" => 2,
        "hour" => 24,
        "minute" | "second" => 60,
        "millisecond" | "microsecond" | "nanosecond" => 1_000,
        _ => 1,
    };
    if !matches!(smallest_unit.as_str(), "year" | "month" | "week" | "day")
        && (increment >= maximum || maximum % increment != 0)
    {
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
    let time_zone_id = temporal_time_zone_string(vm, context, time_zone_value)?;
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
    let month = month.strip_suffix('L').unwrap_or(month);
    let month = parse_fixed_digits(month, 2)?;
    (1..=99).contains(&month).then_some(month as f64)
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
        temporal_calendar_id_from_value(context, value)
    }
}

fn temporal_calendar_id_from_value(
    context: &NativeContext,
    value: JsValue,
) -> Result<String, VmError> {
    if let JsValue::Object(calendar_object) = value {
        if matches!(
            own_string(context, calendar_object, TEMPORAL_KIND).as_deref(),
            Some(
                "PlainDate"
                    | "PlainDateTime"
                    | "PlainMonthDay"
                    | "PlainYearMonth"
                    | "ZonedDateTime"
            )
        ) {
            return Ok(own_string(context, calendar_object, "calendarId")
                .unwrap_or_else(|| "iso8601".into()));
        }
        return Err(VmError::type_error(
            "Temporal calendar must be a string or Temporal object",
        ));
    }
    let JsValue::String(text) = value else {
        return Err(VmError::type_error(
            "Temporal calendar must be a string or Temporal object",
        ));
    };
    normalize_temporal_calendar_string(&text)
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
        temporal_plain_year_month_to_json,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_to_locale_string,
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
    define_temporal_calendar_getter(context, prototype, "era", "get era", "PlainYearMonth")?;
    define_temporal_calendar_getter(
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
            ("calendarId", JsValue::String(calendar_id.clone())),
            (
                "daysInMonth",
                JsValue::Number(calendar_month_day_count(&calendar_id, year_i, month_u) as f64),
            ),
            (
                "daysInYear",
                JsValue::Number(calendar_days_in_year(&calendar_id, year_i) as f64),
            ),
            (
                "monthsInYear",
                JsValue::Number(calendar_months_in_year(&calendar_id, year_i) as f64),
            ),
            (
                "inLeapYear",
                JsValue::Boolean(calendar_is_leap_year(&calendar_id, year_i)),
            ),
        ],
    )
}

fn create_plain_year_month_from_iso(
    context: &mut NativeContext,
    prototype: ObjectId,
    iso_year: f64,
    iso_month: f64,
    reference_day: f64,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    validate_plain_date(iso_year, iso_month, reference_day)?;
    let converted = Icu4xCalendarBackend
        .date_from_iso(
            &calendar_id,
            IsoDate {
                year: iso_year as i32,
                month: iso_month as u8,
                day: reference_day as u8,
            },
        )
        .map_err(VmError::range)?;
    create_temporal_object(
        context,
        prototype,
        "PlainYearMonth",
        [
            ("isoYear", JsValue::Number(iso_year.trunc())),
            ("isoMonth", JsValue::Number(iso_month.trunc())),
            ("referenceISODay", JsValue::Number(reference_day.trunc())),
            ("year", JsValue::Number(converted.year as f64)),
            ("month", JsValue::Number(converted.month as f64)),
            ("monthCode", JsValue::String(converted.month_code)),
            ("calendarId", JsValue::String(calendar_id)),
            (
                "daysInMonth",
                JsValue::Number(converted.days_in_month as f64),
            ),
            ("daysInYear", JsValue::Number(converted.days_in_year as f64)),
            (
                "monthsInYear",
                JsValue::Number(converted.months_in_year as f64),
            ),
            ("inLeapYear", JsValue::Boolean(converted.in_leap_year)),
        ],
    )
}

fn create_plain_year_month_from_calendar_date(
    context: &mut NativeContext,
    prototype: ObjectId,
    date: CalendarDate,
    reference_iso_day: f64,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    create_temporal_object(
        context,
        prototype,
        "PlainYearMonth",
        [
            ("year", JsValue::Number(date.year as f64)),
            ("month", JsValue::Number(date.month as f64)),
            (
                "referenceISODay",
                JsValue::Number(reference_iso_day.trunc()),
            ),
            ("monthCode", JsValue::String(date.month_code)),
            ("calendarId", JsValue::String(calendar_id)),
            ("daysInMonth", JsValue::Number(date.days_in_month as f64)),
            ("daysInYear", JsValue::Number(date.days_in_year as f64)),
            ("monthsInYear", JsValue::Number(date.months_in_year as f64)),
            ("inLeapYear", JsValue::Boolean(date.in_leap_year)),
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
        normalize_temporal_calendar_string(
            &vm.to_string_coerce(arg_or_undefined(arguments, 2), context)?,
        )?
    };
    let reference_day = if matches!(arguments.get(3), None | Some(JsValue::Undefined)) {
        1.0
    } else {
        vm.to_number(arg_or_undefined(arguments, 3), context)?
    };
    create_plain_year_month_from_iso(context, prototype, year, month, reference_day, calendar)
}

fn parse_plain_year_month(text: &str) -> Option<(f64, f64, f64)> {
    let body = validate_iso_calendar_annotations(text.trim())?;
    let (date, time) = match body.find(['T', 't', ' ']) {
        Some(index) => (&body[..index], Some(&body[index + 1..])),
        None => (body, None),
    };
    if let Some(time) = time {
        validate_plain_date_time_tail(time)?;
    }
    let signed = date.starts_with(['+', '-']);
    let year_len = if signed { 7 } else { 4 };
    let normalized = if date.as_bytes().get(year_len) == Some(&b'-') {
        match date.len() - year_len {
            3 => format!("{date}-01"),
            6 => date.to_string(),
            _ => return None,
        }
    } else {
        match (signed, date.len()) {
            (false, 6) | (true, 9) => format!("{date}01"),
            (false, 8) | (true, 11) => date.to_string(),
            _ => return None,
        }
    };
    let (year, month, day) = parse_temporal_date_part(&normalized)?;
    Some((year as f64, month as f64, day as f64))
}

fn temporal_plain_year_month_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainYearMonth")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let parsed_string = if let JsValue::String(text) = &item {
        Some(
            parse_plain_year_month(text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainYearMonth"))?,
        )
    } else {
        None
    };
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (year, mut month, reference_day, calendar_id) = match item {
        JsValue::String(_) => {
            let (year, month, _day) = parsed_string.unwrap();
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
                let calendar_id = temporal_calendar_id_from_object(vm, context, object)?;
                let year = temporal_calendar_year_from_object(vm, context, object, &calendar_id)?;
                let month =
                    temporal_required_month_from_object(vm, context, object, Some(&calendar_id))?;
                let _day = temporal_get_property(vm, context, object, "day")?;
                (year, month, 1.0, calendar_id)
            }
        }
    };
    if !reject_overflow && parsed_string.is_none() {
        if !year.is_finite() || !month.is_finite() {
            return Err(VmError::range("invalid Temporal.PlainYearMonth fields"));
        }
        month = month.trunc().clamp(
            1.0,
            calendar_months_in_year(&calendar_id, year as i32) as f64,
        );
    }
    create_plain_year_month(context, prototype, year, month, reference_day, calendar_id)
}

fn plain_year_month_order_key(context: &NativeContext, object: ObjectId) -> i64 {
    let year = temporal_number_slot(context, object, "year") as i64;
    let month = temporal_number_slot(context, object, "month") as i64;
    year.saturating_mul(12).saturating_add(month)
}

fn plain_year_month_calendar_date(
    context: &NativeContext,
    object: ObjectId,
) -> Result<IsoDate, VmError> {
    let calendar = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    Icu4xCalendarBackend
        .resolve_date_fields(
            &calendar,
            &CalendarDateFields {
                year: temporal_number_slot(context, object, "year") as i32,
                month: None,
                month_code: own_string(context, object, "monthCode"),
                day: 1,
            },
        )
        .map_err(VmError::range)
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
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    if [
        duration.weeks,
        duration.days,
        duration.hours,
        duration.minutes,
        duration.seconds,
        duration.milliseconds,
        duration.microseconds,
        duration.nanoseconds,
    ]
    .into_iter()
    .any(|value| value != 0.0)
    {
        return Err(VmError::range(
            "Temporal.PlainYearMonth arithmetic does not accept units below month",
        ));
    }
    let calendar_id = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let start = Icu4xCalendarBackend
        .resolve_date_fields(
            &calendar_id,
            &CalendarDateFields {
                year: temporal_number_slot(context, object, "year") as i32,
                month: None,
                month_code: own_string(context, object, "monthCode"),
                day: 1,
            },
        )
        .map_err(VmError::range)?;
    let result = Icu4xCalendarBackend
        .add_date(
            &calendar_id,
            start,
            CalendarDuration {
                years: calendar_duration_component(duration.years, sign)?,
                months: calendar_duration_component(duration.months, sign)?,
                ..CalendarDuration::default()
            },
            if reject_overflow {
                CalendarOverflow::Reject
            } else {
                CalendarOverflow::Constrain
            },
        )
        .map_err(VmError::range)?;
    let result = Icu4xCalendarBackend
        .date_from_iso(&calendar_id, result)
        .map_err(VmError::range)?;
    let prototype = temporal_constructor_prototype(context, "PlainYearMonth")?;
    create_plain_year_month_from_calendar_date(context, prototype, result, 1.0, calendar_id)
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
    reject_temporal_with_metadata(vm, context, replacement)?;
    let calendar_id =
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let year = temporal_calendar_year_replacement(
        vm,
        context,
        replacement,
        &calendar_id,
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
    require_temporal_with_field(
        vm,
        context,
        replacement,
        &["day", "era", "eraYear", "month", "monthCode", "year"],
    )?;
    let prototype = temporal_constructor_prototype(context, "PlainYearMonth")?;
    create_plain_year_month(context, prototype, year, month, day, calendar_id)
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
    if options.smallest_unit == "month" && options.increment == 1 {
        let calendar =
            own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into());
        let result = Icu4xCalendarBackend
            .date_until(
                &calendar,
                plain_year_month_calendar_date(context, this_object)?,
                plain_year_month_calendar_date(context, other_object)?,
                if options.largest_unit == "year" {
                    CalendarLargestUnit::Year
                } else {
                    CalendarLargestUnit::Month
                },
            )
            .map_err(VmError::range)?;
        return create_duration_with_default_prototype(
            context,
            DurationValues {
                years: result.years as f64 * sign as f64,
                months: result.months as f64 * sign as f64,
                ..DurationValues::default()
            },
        );
    }
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
    let smallest_unit = normalize_temporal_unit(smallest.unwrap_or_else(|| "month".into()))?;
    let largest_unit = match largest.as_deref() {
        None | Some("auto") => "year".into(),
        Some(unit) => normalize_temporal_unit(unit.to_string())?,
    };
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
        temporal_plain_month_day_to_json,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_to_locale_string,
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

fn create_plain_month_day_from_iso(
    context: &mut NativeContext,
    prototype: ObjectId,
    month: f64,
    day: f64,
    reference_year: f64,
    calendar_id: String,
) -> Result<JsValue, VmError> {
    validate_plain_date(reference_year, month, day)?;
    let converted = Icu4xCalendarBackend
        .date_from_iso(
            &calendar_id,
            IsoDate {
                year: reference_year as i32,
                month: month as u8,
                day: day as u8,
            },
        )
        .map_err(VmError::range)?;
    create_temporal_object(
        context,
        prototype,
        "PlainMonthDay",
        [
            ("isoMonth", JsValue::Number(month.trunc())),
            ("isoDay", JsValue::Number(day.trunc())),
            ("referenceISOYear", JsValue::Number(reference_year.trunc())),
            ("month", JsValue::Number(converted.month as f64)),
            ("day", JsValue::Number(converted.day as f64)),
            ("monthCode", JsValue::String(converted.month_code)),
            ("calendarId", JsValue::String(calendar_id)),
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
        normalize_temporal_calendar_string(
            &vm.to_string_coerce(arg_or_undefined(arguments, 2), context)?,
        )?
    };
    let reference_year = if matches!(arguments.get(3), None | Some(JsValue::Undefined)) {
        1972.0
    } else {
        vm.to_number(arg_or_undefined(arguments, 3), context)?
    };
    create_plain_month_day_from_iso(context, prototype, month, day, reference_year, calendar)
}

fn parse_plain_month_day(text: &str) -> Option<(f64, f64, f64)> {
    let body = validate_iso_calendar_annotations(text.trim())?;
    if let Some(rest) = body.strip_prefix("--") {
        let (month, day) = if rest.len() == 5 && rest.as_bytes().get(2) == Some(&b'-') {
            (
                parse_fixed_digits(&rest[..2], 2)?,
                parse_fixed_digits(&rest[3..], 2)?,
            )
        } else if rest.len() == 4 {
            (
                parse_fixed_digits(&rest[..2], 2)?,
                parse_fixed_digits(&rest[2..], 2)?,
            )
        } else {
            return None;
        };
        validate_plain_date(1972.0, month as f64, day as f64).ok()?;
        return Some((month as f64, day as f64, 1972.0));
    }
    if body.len() == 5 && body.as_bytes().get(2) == Some(&b'-') {
        let month = parse_fixed_digits(&body[..2], 2)?;
        let day = parse_fixed_digits(&body[3..], 2)?;
        validate_plain_date(1972.0, month as f64, day as f64).ok()?;
        return Some((month as f64, day as f64, 1972.0));
    }
    if body.len() == 4 && body.chars().all(|ch| ch.is_ascii_digit()) {
        let month = parse_fixed_digits(&body[..2], 2)?;
        let day = parse_fixed_digits(&body[2..], 2)?;
        validate_plain_date(1972.0, month as f64, day as f64).ok()?;
        return Some((month as f64, day as f64, 1972.0));
    }
    let (date, time) = match body.find(['T', 't', ' ']) {
        Some(index) => (&body[..index], Some(&body[index + 1..])),
        None => (body, None),
    };
    if let Some(time) = time {
        validate_plain_date_time_tail(time)?;
    }
    let signed = date.starts_with(['+', '-']);
    let (month_text, day_text) = match (signed, date.len()) {
        (false, 10) if date.as_bytes().get(4) == Some(&b'-') => (&date[5..7], &date[8..10]),
        (false, 8) => (&date[4..6], &date[6..8]),
        (true, 13) if date.as_bytes().get(7) == Some(&b'-') => (&date[8..10], &date[11..13]),
        (true, 11) => (&date[7..9], &date[9..11]),
        _ => return None,
    };
    let year_digits = if signed { &date[1..7] } else { &date[..4] };
    if !year_digits.chars().all(|ch| ch.is_ascii_digit()) || date.starts_with("-000000") {
        return None;
    }
    let month = parse_fixed_digits(month_text, 2)?;
    let day = parse_fixed_digits(day_text, 2)?;
    validate_plain_date(1972.0, month as f64, day as f64).ok()?;
    Some((month as f64, day as f64, 1972.0))
}

fn temporal_plain_month_day_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainMonthDay")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let parsed_string = if let JsValue::String(text) = &item {
        Some(
            parse_plain_month_day(text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainMonthDay"))?,
        )
    } else {
        None
    };
    let reject_overflow = temporal_overflow_reject(
        vm,
        context,
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
    )?;
    let (mut month, mut day, reference_year, calendar_id) = match item {
        JsValue::String(_) => {
            let (month, day, _year) = parsed_string.unwrap();
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
                let month = temporal_required_month_from_object(vm, context, object, None)?;
                let day = temporal_required_object_number(vm, context, object, "day")?;
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
    if !reject_overflow && parsed_string.is_none() {
        if !month.is_finite() || !day.is_finite() {
            return Err(VmError::range("invalid Temporal.PlainMonthDay fields"));
        }
        month = month.trunc().clamp(1.0, 12.0);
        day = day
            .trunc()
            .clamp(1.0, month_day_count(1972, month as u32) as f64);
    }
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
        temporal_zoned_date_time_to_json,
    )?;
    define_method(
        context,
        prototype,
        "toLocaleString",
        0,
        temporal_to_locale_string,
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
            1,
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
    define_temporal_calendar_getter(context, prototype, "era", "get era", "ZonedDateTime")?;
    define_temporal_calendar_getter(
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
    define_accessor(
        context,
        prototype,
        "hoursInDay",
        "get hoursInDay",
        temporal_zoned_date_time_hours_in_day,
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
    let offset_nanoseconds = JiffTimeZoneProvider
        .offset_nanoseconds(&time_zone_id, exact_epoch_nanoseconds)
        .map_err(VmError::range)?;
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
    let epoch_nanoseconds = temporal_to_bigint(vm, context, arg_or_undefined(arguments, 0))?;
    let epoch_nanoseconds_i128 = bigint_to_i128_saturating(&epoch_nanoseconds);
    if !is_valid_instant_ns(epoch_nanoseconds_i128) {
        return Err(VmError::range("invalid Temporal.ZonedDateTime"));
    }
    let epoch_nanoseconds_value = JsValue::BigInt(epoch_nanoseconds);
    let epoch_nanoseconds = epoch_nanoseconds_i128 as f64;
    let time_zone_id = if matches!(arguments.get(1), None | Some(JsValue::Undefined)) {
        return Err(VmError::type_error(
            "Temporal.ZonedDateTime requires a time zone",
        ));
    } else {
        if let Some(JsValue::String(text)) = arguments.get(1)
            && (text.contains('[') || parse_instant_string(text).is_some())
        {
            return Err(VmError::range(
                "Temporal.ZonedDateTime constructor requires a time zone identifier",
            ));
        }
        temporal_time_zone_string(vm, context, arg_or_undefined(arguments, 1))?
    };
    if let Some(JsValue::String(text)) = arguments.get(2)
        && (text.contains('[')
            || text
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_digit() || matches!(ch, '+' | '-')))
    {
        return Err(VmError::range(
            "Temporal.ZonedDateTime constructor requires a calendar identifier",
        ));
    }
    let calendar_id = temporal_calendar_from_argument(vm, context, arguments.get(2))?;
    create_zoned_date_time(
        context,
        prototype,
        epoch_nanoseconds_value,
        epoch_nanoseconds,
        time_zone_id,
        calendar_id,
    )
}

fn parse_zoned_date_time(text: &str) -> Option<(i128, Option<i128>, bool, String, bool)> {
    if text.contains('\u{2212}') {
        return None;
    }
    let first_annotation = text.find('[')?;
    let without_annotation = &text[..first_annotation];
    let mut rest = &text[first_annotation..];
    let mut time_zone_id = None;
    let mut calendar_critical = None;
    while !rest.is_empty() {
        let after_open = rest.strip_prefix('[')?;
        let end = after_open.find(']')?;
        let raw_annotation = &after_open[..end];
        let critical = raw_annotation.starts_with('!');
        let annotation = raw_annotation.strip_prefix('!').unwrap_or(raw_annotation);
        if annotation.is_empty() {
            return None;
        }
        if let Some((key, value)) = annotation.split_once('=') {
            if value.is_empty()
                || key.chars().any(|ch| ch.is_ascii_uppercase())
                || (key != "u-ca" && critical)
            {
                return None;
            }
            if key == "u-ca" {
                if let Some(previous_critical) = calendar_critical {
                    if critical || previous_critical {
                        return None;
                    }
                } else {
                    calendar_critical = Some(critical);
                }
            }
        } else {
            if time_zone_id.is_some()
                || annotation.contains('!')
                || offset_annotation_has_subminute_syntax(annotation)
            {
                return None;
            }
            time_zone_id = Some(annotation.to_string());
        }
        rest = &after_open[end + 1..];
    }
    let time_zone_id = time_zone_id?;
    let (year, month, day) = parse_iso_date_part(
        without_annotation
            .split_once(['T', 't', ' '])
            .map(|(date, _)| date)
            .unwrap_or(without_annotation),
    )?;
    let time_part = without_annotation
        .split_once(['T', 't', ' '])
        .map(|(_, time)| time);
    let time = if let Some(time) = time_part {
        let has_explicit_offset = time.ends_with(['Z', 'z'])
            || time.rfind('+').is_some()
            || time.get(1..).is_some_and(|rest| rest.rfind('-').is_some());
        if has_explicit_offset {
            let (hour, minute, second, fraction_ns, _offset_ns) =
                parse_iso_time_and_offset_ns(time)?;
            PlainTimeValues {
                hour: hour as f64,
                minute: minute as f64,
                second: second as f64,
                millisecond: (fraction_ns / 1_000_000) as f64,
                microsecond: ((fraction_ns / 1_000) % 1_000) as f64,
                nanosecond: (fraction_ns % 1_000) as f64,
            }
        } else {
            parse_plain_time(time)?
        }
    } else {
        PlainTimeValues::default()
    };
    if !(1..=12).contains(&month) || !(1..=month_day_count(year, month)).contains(&day) {
        return None;
    }
    validate_plain_time(time).ok()?;
    let local_ns =
        epoch_nanoseconds_i128_from_plain_parts(year as f64, month as f64, day as f64, time);
    let (explicit_offset, has_z_designator) = if let Some(time) = time_part {
        if time.ends_with(['Z', 'z']) {
            (Some(0), true)
        } else if let Some(index) = time.rfind('+').or_else(|| {
            time.get(1..)
                .and_then(|rest| rest.rfind('-'))
                .map(|i| i + 1)
        }) {
            (Some(parse_iso_time_zone_offset_ns(&time[index..])?), false)
        } else {
            (None, false)
        }
    } else {
        (None, false)
    };
    Some((
        local_ns,
        explicit_offset,
        has_z_designator,
        time_zone_id,
        (year, month, day) >= (-271_821, 4, 20) && (year, month, day) <= (275_760, 9, 13),
    ))
}

fn validate_temporal_time_zone_syntax(text: &str) -> Result<(), VmError> {
    if text.contains("-000000") || offset_annotation_has_subminute_syntax(text) {
        return Err(VmError::range("invalid Temporal time zone"));
    }
    if let Some((_, time)) = text.split_once(['T', 't', ' ']) {
        let offset_start = time.rfind('+').or_else(|| time.rfind('-'));
        if offset_start.is_some_and(|index| offset_annotation_has_subminute_syntax(&time[index..]))
        {
            return Err(VmError::range("invalid Temporal time zone"));
        }
    }
    if let Some((_, time)) = text.split_once(['T', 't'])
        && let Some(fraction_index) = time.find(['.', ','])
    {
        let head = &time[..fraction_index];
        let compact_len = head.chars().filter(|ch| ch.is_ascii_digit()).count();
        if compact_len == 2 || compact_len == 4 {
            return Err(VmError::range("invalid fractional Temporal time"));
        }
    }
    Ok(())
}

fn temporal_time_zone_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<String, VmError> {
    let text = match value {
        JsValue::String(text) => text,
        JsValue::Object(object)
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("ZonedDateTime") =>
        {
            return Ok(own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()));
        }
        _ => {
            return Err(VmError::type_error(
                "Temporal time zone must be a string or ZonedDateTime",
            ));
        }
    };
    if text.is_empty() {
        return Err(VmError::range("invalid Temporal time zone"));
    }
    validate_temporal_time_zone_syntax(&text)?;
    if text.eq_ignore_ascii_case("UTC") {
        return Ok("UTC".into());
    }
    if let Some((_local_ns, _offset, _has_z, zone, _local_date_in_range)) =
        parse_zoned_date_time(&text)
    {
        return Ok(canonicalize_temporal_time_zone_id(&zone));
    }
    if parse_instant_string(&text).is_some() {
        if text.ends_with(['Z', 'z']) {
            return Ok("UTC".into());
        }
        let body = validate_temporal_annotations(&text).unwrap_or(&text);
        let separator = body.find(['T', 't', ' ']).unwrap_or(0);
        if let Some(index) = body[separator + 1..]
            .rfind(['+', '-'])
            .map(|index| separator + 1 + index)
        {
            return Ok(canonicalize_temporal_time_zone_id(&body[index..]));
        }
    }
    if parse_time_zone_offset_ns(&text).is_some() {
        Ok(canonicalize_temporal_time_zone_id(&text))
    } else if text.is_ascii()
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '+'))
        && !text.chars().next().is_some_and(|ch| ch.is_ascii_digit())
    {
        Ok(text)
    } else {
        Err(VmError::range("invalid Temporal time zone"))
    }
}

fn canonicalize_temporal_time_zone_id(identifier: &str) -> String {
    if identifier.eq_ignore_ascii_case("UTC") {
        "UTC".into()
    } else if let Some(offset_ns) = parse_time_zone_offset_ns(identifier) {
        format_time_zone_offset_ns(offset_ns)
    } else {
        identifier.into()
    }
}

fn temporal_zoned_date_time_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let parsed_string = if let JsValue::String(text) = &item {
        Some(
            parse_zoned_date_time(text)
                .ok_or_else(|| VmError::range("invalid Temporal.ZonedDateTime"))?,
        )
    } else {
        None
    };
    let (epoch_nanoseconds, epoch_nanoseconds_value, time_zone_id, calendar_id) = match item {
        JsValue::String(_) => {
            let (_disambiguation, offset_option, _overflow_option) = zoned_date_time_from_options(
                vm,
                context,
                arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
            )?;
            let (local_ns, supplied_offset, has_z_designator, time_zone_id, local_date_in_range) =
                parsed_string.unwrap();
            if matches!(offset_option.as_str(), "prefer" | "reject") && !local_date_in_range {
                return Err(VmError::range(
                    "Temporal.ZonedDateTime wall-clock time is out of range",
                ));
            }
            let zone_offset = if time_zone_id.eq_ignore_ascii_case("UTC") {
                Some(0)
            } else {
                parse_time_zone_offset_ns(&time_zone_id)
            };
            if offset_option == "reject"
                && !has_z_designator
                && let (Some(supplied), Some(zone)) = (supplied_offset, zone_offset)
                && supplied != zone
            {
                return Err(VmError::range(
                    "Temporal offset does not match the time zone",
                ));
            }
            let applied_offset = match offset_option.as_str() {
                "use" => supplied_offset.or(zone_offset).unwrap_or(0),
                "ignore" => zone_offset.unwrap_or(0),
                "prefer" if !has_z_designator => supplied_offset
                    .filter(|supplied| zone_offset.is_none_or(|zone| *supplied == zone))
                    .or(zone_offset)
                    .unwrap_or(0),
                _ => supplied_offset.or(zone_offset).unwrap_or(0),
            };
            let exact_epoch_nanoseconds = local_ns
                .checked_sub(applied_offset)
                .ok_or_else(|| VmError::range("Temporal.ZonedDateTime is out of range"))?;
            (
                exact_epoch_nanoseconds as f64,
                JsValue::BigInt(bigint::from_i128(exact_epoch_nanoseconds)),
                canonicalize_temporal_time_zone_id(&time_zone_id),
                "iso8601".into(),
            )
        }
        value => {
            let object = context.require_object(&value, "Temporal.ZonedDateTime.from")?;
            if own_string(context, object, TEMPORAL_KIND).as_deref() == Some("ZonedDateTime") {
                let (_disambiguation, _offset_option, _overflow_option) =
                    zoned_date_time_from_options(
                        vm,
                        context,
                        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
                    )?;
                (
                    temporal_number_slot(context, object, "epochNanosecondsNumber"),
                    own_data_value(context, object, "epochNanoseconds")
                        .unwrap_or_else(|| JsValue::BigInt(bigint::from_i64(0))),
                    own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into()),
                    own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into()),
                )
            } else {
                let calendar_value = temporal_get_property(vm, context, object, "calendar")?;
                let calendar_id = if matches!(calendar_value, JsValue::Undefined) {
                    let calendar_id_value =
                        temporal_get_property(vm, context, object, "calendarId")?;
                    if matches!(calendar_id_value, JsValue::Undefined) {
                        "iso8601".into()
                    } else {
                        temporal_calendar_id_from_value(context, calendar_id_value)?
                    }
                } else {
                    temporal_calendar_id_from_value(context, calendar_value)?
                };
                let day = temporal_partial_number(vm, context, object, "day")?;
                let hour = temporal_object_number(vm, context, object, "hour")?;
                let microsecond = temporal_object_number(vm, context, object, "microsecond")?;
                let millisecond = temporal_object_number(vm, context, object, "millisecond")?;
                let minute = temporal_object_number(vm, context, object, "minute")?;
                let month_value = temporal_get_property(vm, context, object, "month")?;
                let month = if matches!(month_value, JsValue::Undefined) {
                    None
                } else {
                    Some(vm.to_number(month_value, context)?.trunc())
                };
                let month_code_value = temporal_get_property(vm, context, object, "monthCode")?;
                let month_code = if matches!(month_code_value, JsValue::Undefined) {
                    None
                } else {
                    let text = temporal_month_code_to_string(vm, context, month_code_value)?;
                    let parsed = parse_month_code(&text)
                        .ok_or_else(|| VmError::range("invalid Temporal monthCode"))?;
                    Some((parsed, text.ends_with('L')))
                };
                let nanosecond = temporal_object_number(vm, context, object, "nanosecond")?;
                let offset_value = temporal_get_property(vm, context, object, "offset")?;
                let supplied_offset = if matches!(offset_value, JsValue::Undefined) {
                    None
                } else {
                    let text = temporal_string_or_object_to_string(
                        vm,
                        context,
                        offset_value,
                        "Temporal offset property must be a string",
                    )?;
                    Some(
                        parse_iso_time_zone_offset_ns(&text)
                            .ok_or_else(|| VmError::range("invalid Temporal offset"))?,
                    )
                };
                let second = temporal_object_number(vm, context, object, "second")?;
                let time_zone = temporal_get_property(vm, context, object, "timeZone")?;
                let time_zone_id = if matches!(time_zone, JsValue::Undefined) {
                    None
                } else {
                    Some(temporal_time_zone_string(vm, context, time_zone)?)
                };
                let year = temporal_partial_number(vm, context, object, "year")?;
                let year = year
                    .ok_or_else(|| VmError::type_error("Temporal property `year` is required"))?;
                let mut day =
                    day.ok_or_else(|| VmError::type_error("Temporal property `day` is required"))?;
                let mut month = match (month, month_code) {
                    (Some(month), Some((month_code, _))) if month != month_code => {
                        return Err(VmError::range("Temporal month and monthCode conflict"));
                    }
                    (Some(month), _) => month,
                    (_, Some((month_code, _))) => month_code,
                    (None, None) => {
                        return Err(VmError::type_error(
                            "Temporal property `month` or `monthCode` is required",
                        ));
                    }
                };
                let time_zone_id = time_zone_id.ok_or_else(|| {
                    VmError::type_error("Temporal property `timeZone` is required")
                })?;
                let (_disambiguation, offset_option, overflow_option) =
                    zoned_date_time_from_options(
                        vm,
                        context,
                        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
                    )?;
                if calendar_id == "iso8601"
                    && month_code.is_some_and(|(parsed, leap)| parsed > 12.0 || leap)
                {
                    return Err(VmError::range(
                        "Temporal monthCode is not valid for the ISO 8601 calendar",
                    ));
                }
                let mut time = PlainTimeValues {
                    hour,
                    minute,
                    second,
                    millisecond,
                    microsecond,
                    nanosecond,
                };
                if ![
                    year,
                    month,
                    day,
                    time.hour,
                    time.minute,
                    time.second,
                    time.millisecond,
                    time.microsecond,
                    time.nanosecond,
                ]
                .into_iter()
                .all(f64::is_finite)
                {
                    return Err(VmError::range("invalid Temporal.ZonedDateTime fields"));
                }
                if month < 1.0 || day < 1.0 {
                    return Err(VmError::range("Temporal month and day must be positive"));
                }
                if overflow_option == "constrain" {
                    month = month.trunc().clamp(1.0, 12.0);
                    day = day
                        .trunc()
                        .clamp(1.0, month_day_count(year as i32, month as u32) as f64);
                    time.hour = time.hour.trunc().clamp(0.0, 23.0);
                    time.minute = time.minute.trunc().clamp(0.0, 59.0);
                    time.second = time.second.trunc().clamp(0.0, 59.0);
                    time.millisecond = time.millisecond.trunc().clamp(0.0, 999.0);
                    time.microsecond = time.microsecond.trunc().clamp(0.0, 999.0);
                    time.nanosecond = time.nanosecond.trunc().clamp(0.0, 999.0);
                }
                validate_plain_date(year, month, day)?;
                validate_plain_time(time)?;
                let local_ns = epoch_nanoseconds_i128_from_plain_parts(year, month, day, time);
                let zone_offset = if time_zone_id.eq_ignore_ascii_case("UTC") {
                    Some(0)
                } else {
                    parse_time_zone_offset_ns(&time_zone_id)
                };
                if offset_option == "reject"
                    && let (Some(supplied), Some(zone)) = (supplied_offset, zone_offset)
                    && supplied != zone
                {
                    return Err(VmError::range(
                        "Temporal offset does not match the time zone",
                    ));
                }
                let applied_offset = match offset_option.as_str() {
                    "use" => supplied_offset.or(zone_offset).unwrap_or(0),
                    "prefer" => supplied_offset
                        .filter(|supplied| zone_offset.is_none_or(|zone| *supplied == zone))
                        .or(zone_offset)
                        .unwrap_or(0),
                    _ => zone_offset.unwrap_or(0),
                };
                let exact_epoch_nanoseconds = local_ns - applied_offset;
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

fn zoned_date_time_from_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<(String, String, String), VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok(("compatible".into(), "reject".into(), "constrain".into()));
    }
    let object = context.require_object(&value, "Temporal.ZonedDateTime.from options")?;
    let disambiguation = option_string(vm, context, object, "disambiguation")?
        .unwrap_or_else(|| "compatible".into());
    let offset = option_string(vm, context, object, "offset")?.unwrap_or_else(|| "reject".into());
    let overflow =
        option_string(vm, context, object, "overflow")?.unwrap_or_else(|| "constrain".into());
    if !matches!(
        disambiguation.as_str(),
        "compatible" | "earlier" | "later" | "reject"
    ) {
        return Err(VmError::range("invalid Temporal disambiguation option"));
    }
    if !matches!(offset.as_str(), "prefer" | "use" | "ignore" | "reject") {
        return Err(VmError::range("invalid Temporal offset option"));
    }
    if !matches!(overflow.as_str(), "constrain" | "reject") {
        return Err(VmError::range("invalid Temporal overflow option"));
    }
    Ok((disambiguation, offset, overflow))
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

fn temporal_zoned_date_time_hours_in_day(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    let epoch_ns = zoned_date_time_epoch_ns(context, object);
    let time_zone_id = own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into());
    let offset_ns = parse_time_zone_offset_ns(&time_zone_id).unwrap_or(0);
    let local_ns = epoch_ns
        .checked_add(offset_ns)
        .ok_or_else(|| VmError::range("Temporal.ZonedDateTime day is out of range"))?;
    let local_day = local_ns.div_euclid(NS_PER_DAY_I128);
    let today_ns = local_day
        .checked_mul(NS_PER_DAY_I128)
        .and_then(|value| value.checked_sub(offset_ns))
        .ok_or_else(|| VmError::range("Temporal.ZonedDateTime day is out of range"))?;
    let tomorrow_ns = local_day
        .checked_add(1)
        .and_then(|value| value.checked_mul(NS_PER_DAY_I128))
        .and_then(|value| value.checked_sub(offset_ns))
        .ok_or_else(|| VmError::range("Temporal.ZonedDateTime day is out of range"))?;
    if !is_valid_instant_ns(today_ns) || !is_valid_instant_ns(tomorrow_ns) {
        return Err(VmError::range(
            "Temporal.ZonedDateTime day boundary is out of range",
        ));
    }
    Ok(JsValue::Number(
        (tomorrow_ns - today_ns) as f64 / NS_PER_HOUR_I128 as f64,
    ))
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
    let this_time_zone = own_string(context, this_object, "timeZoneId").unwrap_or_default();
    let other_time_zone = own_string(context, other_object, "timeZoneId").unwrap_or_default();
    Ok(JsValue::Boolean(
        zoned_date_time_epoch_ns(context, this_object)
            == zoned_date_time_epoch_ns(context, other_object)
            && (parse_time_zone_offset_ns(&this_time_zone)
                .zip(parse_time_zone_offset_ns(&other_time_zone))
                .is_some_and(|(left, right)| left == right)
                || this_time_zone.eq_ignore_ascii_case(&other_time_zone))
            && own_string(context, this_object, "calendarId")
                == own_string(context, other_object, "calendarId"),
    ))
}

#[allow(clippy::too_many_arguments)]
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
    let exact_epoch_nanoseconds = JiffTimeZoneProvider
        .local_to_epoch_nanoseconds(
            &time_zone_id,
            LocalDateTime {
                year: year as i32,
                month: month as u8,
                day: day as u8,
                hour: time.hour as u8,
                minute: time.minute as u8,
                second: time.second as u8,
                nanosecond: (time.millisecond as u32 * 1_000_000)
                    + (time.microsecond as u32 * 1_000)
                    + time.nanosecond as u32,
            },
            TimeZoneDisambiguation::Compatible,
        )
        .map_err(VmError::range)?;
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
    let calendar_id = own_string(context, object, "calendarId").unwrap_or_else(|| "iso8601".into());
    let (mut year, mut month, mut day) = apply_duration_to_date(
        temporal_number_slot(context, object, "year"),
        temporal_number_slot(context, object, "month"),
        temporal_number_slot(context, object, "day"),
        &calendar_id,
        duration,
        sign,
        reject_overflow,
    )?;
    let time_ns = plain_time_nanoseconds_i128(plain_date_time_values(context, object))
        + sign as i128 * duration_time_nanoseconds_i128(duration);
    let (extra_days, time) = balance_plain_time_nanoseconds_i128(time_ns)?;
    if extra_days != 0 {
        let day_number =
            calendar_days_from_civil(&calendar_id, year as i32, month as u32, day as u32)
                .checked_add(extra_days)
                .filter(|value| temporal_day_number_within_range(*value))
                .ok_or_else(|| VmError::range("Temporal.ZonedDateTime is out of range"))?;
        let fields = calendar_civil_from_days(&calendar_id, day_number);
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
        calendar_id,
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
    let epoch_ns = zoned_date_time_epoch_ns(context, object);
    let time_zone_id = own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into());
    let offset_ns = parse_time_zone_offset_ns(&time_zone_id).unwrap_or(0);
    let local_ns = epoch_ns
        .checked_add(offset_ns)
        .ok_or_else(|| VmError::range("Temporal.ZonedDateTime is out of range"))?;
    let rounded = if unit == "day" {
        let local_day = local_ns.div_euclid(NS_PER_DAY_I128);
        let today_ns = local_day
            .checked_mul(NS_PER_DAY_I128)
            .and_then(|value| value.checked_sub(offset_ns))
            .ok_or_else(|| VmError::range("Temporal.ZonedDateTime day is out of range"))?;
        let tomorrow_ns = local_day
            .checked_add(1)
            .and_then(|value| value.checked_mul(NS_PER_DAY_I128))
            .and_then(|value| value.checked_sub(offset_ns))
            .ok_or_else(|| VmError::range("Temporal.ZonedDateTime day is out of range"))?;
        if !is_valid_instant_ns(today_ns) || !is_valid_instant_ns(tomorrow_ns) {
            return Err(VmError::range(
                "Temporal.ZonedDateTime day boundary is out of range",
            ));
        }
        today_ns + round_i128(epoch_ns - today_ns, tomorrow_ns - today_ns, mode)
    } else {
        let quantum = temporal_unit_nanoseconds(&unit) * increment as i128;
        round_i128(local_ns, quantum, mode)
            .checked_sub(offset_ns)
            .ok_or_else(|| VmError::range("Temporal.ZonedDateTime is out of range"))?
    };
    let prototype = temporal_constructor_prototype(context, "ZonedDateTime")?;
    create_zoned_date_time(
        context,
        prototype,
        JsValue::BigInt(bigint::from_i128(rounded)),
        rounded as f64,
        time_zone_id,
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
        "hour",
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
    let calendar_mode = if sign < 0 {
        negate_temporal_round_mode(options.mode)
    } else {
        options.mode
    };
    if matches!(
        options.largest_unit.as_str(),
        "year" | "month" | "week" | "day"
    ) && matches!(
        options.smallest_unit.as_str(),
        "hour" | "minute" | "second" | "millisecond" | "microsecond" | "nanosecond"
    ) {
        let (start, end) = (this_object, other_object);
        let mut values =
            zoned_date_time_calendar_difference_values(context, start, end, &options.largest_unit);
        let rounded_time = round_signed_i128(
            duration_time_nanoseconds_i128(values),
            quantum,
            calendar_mode,
        );
        let day_carry = rounded_time / NS_PER_DAY_I128;
        let remainder = rounded_time % NS_PER_DAY_I128;
        let time_values = signed_duration_time_from_nanoseconds(remainder);
        values.hours = time_values.hours;
        values.minutes = time_values.minutes;
        values.seconds = time_values.seconds;
        values.milliseconds = time_values.milliseconds;
        values.microseconds = time_values.microseconds;
        values.nanoseconds = time_values.nanoseconds;
        values.days += day_carry as f64;
        if options.largest_unit == "week" {
            let extra_weeks = (values.days / 7.0).trunc();
            values.weeks += extra_weeks;
            values.days -= extra_weeks * 7.0;
        }
        balance_zoned_calendar_days(context, start, &options.largest_unit, &mut values);
        if sign < 0 {
            values = values.map(|value| -value);
        }
        return create_duration_with_default_prototype(context, values);
    }
    if matches!(
        options.smallest_unit.as_str(),
        "year" | "month" | "week" | "day"
    ) {
        if options.smallest_unit == "day" {
            let rounding_base = if sign > 0 {
                zoned_date_time_epoch_ns(context, this_object)
            } else {
                zoned_date_time_epoch_ns(context, other_object)
            };
            let bound_delta = if delta < 0 { -quantum } else { quantum };
            let rounded_end = rounding_base
                .checked_add(bound_delta)
                .ok_or_else(|| VmError::range("Temporal rounded date-time is out of range"))?;
            if !is_valid_instant_ns(rounded_end) {
                return Err(VmError::range("Temporal rounded date-time is out of range"));
            }
        }
        let (start, end) = (this_object, other_object);
        let calendar_values =
            iso_date_difference_values(context, start, end, &options.largest_unit);
        let same_local_time = plain_time_nanoseconds_i128(plain_date_time_values(context, start))
            == plain_time_nanoseconds_i128(plain_date_time_values(context, end));
        let exact_larger_unit = same_local_time
            && match options.smallest_unit.as_str() {
                "year" => {
                    calendar_values.months == 0.0
                        && calendar_values.weeks == 0.0
                        && calendar_values.days == 0.0
                }
                "month" => calendar_values.weeks == 0.0 && calendar_values.days == 0.0,
                "week" => calendar_values.days == 0.0,
                "day" => true,
                _ => false,
            };
        if exact_larger_unit {
            let values = if sign < 0 {
                calendar_values.map(|value| -value)
            } else {
                calendar_values
            };
            return create_duration_with_default_prototype(context, values);
        }
        let quantity =
            relative_plain_date_time_unit_quantity(context, start, end, &options.smallest_unit);
        let rounded = round_temporal_quantity(quantity, options.increment, calendar_mode);
        let mut values = match options.smallest_unit.as_str() {
            "year" => DurationValues {
                years: rounded,
                ..DurationValues::default()
            },
            "month" if options.largest_unit == "year" => DurationValues {
                years: (rounded / 12.0).trunc(),
                months: rounded % 12.0,
                ..DurationValues::default()
            },
            "month" => DurationValues {
                months: rounded,
                ..DurationValues::default()
            },
            "week" => DurationValues {
                weeks: rounded,
                ..DurationValues::default()
            },
            "day" if options.largest_unit == "week" => DurationValues {
                weeks: (rounded / 7.0).trunc(),
                days: rounded % 7.0,
                ..DurationValues::default()
            },
            "day" => DurationValues {
                days: rounded,
                ..DurationValues::default()
            },
            _ => unreachable!(),
        };
        if sign < 0 {
            values = values.map(|value| -value);
        }
        return create_duration_with_default_prototype(context, values);
    }
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
    let day_value = temporal_get_property(vm, context, replacement, "day")?;
    let day = temporal_date_replacement_from_value(
        vm,
        context,
        day_value.clone(),
        "day",
        temporal_number_slot(context, this_object, "day"),
    )?;
    let hour_value = temporal_get_property(vm, context, replacement, "hour")?;
    let hour = temporal_date_replacement_from_value(
        vm,
        context,
        hour_value.clone(),
        "hour",
        temporal_number_slot(context, this_object, "hour"),
    )?;
    let microsecond_value = temporal_get_property(vm, context, replacement, "microsecond")?;
    let microsecond = temporal_date_replacement_from_value(
        vm,
        context,
        microsecond_value.clone(),
        "microsecond",
        temporal_number_slot(context, this_object, "microsecond"),
    )?;
    let millisecond_value = temporal_get_property(vm, context, replacement, "millisecond")?;
    let millisecond = temporal_date_replacement_from_value(
        vm,
        context,
        millisecond_value.clone(),
        "millisecond",
        temporal_number_slot(context, this_object, "millisecond"),
    )?;
    let minute_value = temporal_get_property(vm, context, replacement, "minute")?;
    let minute = temporal_date_replacement_from_value(
        vm,
        context,
        minute_value.clone(),
        "minute",
        temporal_number_slot(context, this_object, "minute"),
    )?;
    let month_value = temporal_get_property(vm, context, replacement, "month")?;
    let month = if matches!(month_value, JsValue::Undefined) {
        None
    } else {
        let value = vm.to_number(month_value.clone(), context)?.trunc();
        if !value.is_finite() {
            return Err(VmError::range("Temporal month must be a finite number"));
        }
        Some(value)
    };
    let month_code_value = temporal_get_property(vm, context, replacement, "monthCode")?;
    let (month_code, month_code_text) = if matches!(month_code_value, JsValue::Undefined) {
        (None, None)
    } else {
        let value = temporal_string_or_object_to_string(
            vm,
            context,
            month_code_value.clone(),
            "Temporal monthCode property must be a string",
        )?;
        (
            Some(
                parse_month_code(&value)
                    .ok_or_else(|| VmError::range("invalid Temporal monthCode"))?,
            ),
            Some(value),
        )
    };
    let month = match (month, month_code) {
        (Some(month), Some(month_code)) if month != month_code => {
            return Err(VmError::range("Temporal month and monthCode must agree"));
        }
        (Some(month), _) => month,
        (_, Some(month_code)) => month_code,
        (None, None) => temporal_number_slot(context, this_object, "month"),
    };
    let nanosecond_value = temporal_get_property(vm, context, replacement, "nanosecond")?;
    let nanosecond = temporal_date_replacement_from_value(
        vm,
        context,
        nanosecond_value.clone(),
        "nanosecond",
        temporal_number_slot(context, this_object, "nanosecond"),
    )?;
    let replacement_offset = temporal_get_property(vm, context, replacement, "offset")?;
    let supplied_offset = if matches!(replacement_offset, JsValue::Undefined) {
        None
    } else {
        let offset = temporal_string_or_object_to_string(
            vm,
            context,
            replacement_offset.clone(),
            "Temporal offset property must be a string",
        )?;
        Some(
            parse_time_zone_offset_ns(&offset)
                .ok_or_else(|| VmError::range("invalid Temporal offset string"))?,
        )
    };
    let second_value = temporal_get_property(vm, context, replacement, "second")?;
    let second = temporal_date_replacement_from_value(
        vm,
        context,
        second_value.clone(),
        "second",
        temporal_number_slot(context, this_object, "second"),
    )?;
    let year_value = temporal_get_property(vm, context, replacement, "year")?;
    let year = temporal_date_replacement_from_value(
        vm,
        context,
        year_value.clone(),
        "year",
        temporal_number_slot(context, this_object, "year"),
    )?;
    if [
        day_value,
        hour_value,
        microsecond_value,
        millisecond_value,
        minute_value,
        month_value,
        month_code_value,
        nanosecond_value,
        replacement_offset,
        second_value,
        year_value,
    ]
    .into_iter()
    .all(|value| matches!(value, JsValue::Undefined))
    {
        return Err(VmError::type_error(
            "Temporal with() property bag must contain a supported field",
        ));
    }
    let mut time = PlainTimeValues {
        hour,
        minute,
        second,
        millisecond,
        microsecond,
        nanosecond,
    };
    if !year.is_finite()
        || !month.is_finite()
        || !day.is_finite()
        || !time.hour.is_finite()
        || !time.minute.is_finite()
        || !time.second.is_finite()
        || !time.millisecond.is_finite()
        || !time.microsecond.is_finite()
        || !time.nanosecond.is_finite()
        || month < 1.0
        || day < 1.0
        || time.hour < 0.0
        || time.minute < 0.0
        || time.second < 0.0
        || time.millisecond < 0.0
        || time.microsecond < 0.0
        || time.nanosecond < 0.0
    {
        return Err(VmError::range("invalid Temporal.ZonedDateTime fields"));
    }
    let options = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let options_object = if matches!(options, JsValue::Undefined) {
        None
    } else {
        Some(context.require_object(&options, "Temporal.ZonedDateTime.prototype.with options")?)
    };
    let mut offset_option = "prefer".to_string();
    if let Some(object) = options_object {
        if let Some(value) = option_string(vm, context, object, "disambiguation")?
            && !matches!(
                value.as_str(),
                "compatible" | "earlier" | "later" | "reject"
            )
        {
            return Err(VmError::range("invalid Temporal disambiguation option"));
        }
        if let Some(value) = option_string(vm, context, object, "offset")? {
            if !matches!(value.as_str(), "use" | "prefer" | "ignore" | "reject") {
                return Err(VmError::range("invalid Temporal offset option"));
            }
            offset_option = value;
        }
    }
    let reject_overflow = temporal_overflow_reject(vm, context, options)?;
    if let Some(text) = month_code_text {
        let calendar_id =
            own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into());
        if calendar_id == "iso8601" && (month > 12.0 || text.ends_with('L')) {
            return Err(VmError::range(
                "Temporal monthCode is not valid for the ISO 8601 calendar",
            ));
        }
    }
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
    let time_zone_id =
        own_string(context, this_object, "timeZoneId").unwrap_or_else(|| "UTC".into());
    let calendar_id =
        own_string(context, this_object, "calendarId").unwrap_or_else(|| "iso8601".into());
    if offset_option == "use"
        && let Some(offset_ns) = supplied_offset
    {
        let exact_epoch_nanoseconds =
            epoch_nanoseconds_i128_from_plain_parts(year, month, day, time) - offset_ns;
        return create_zoned_date_time(
            context,
            prototype,
            JsValue::BigInt(bigint::from_i128(exact_epoch_nanoseconds)),
            exact_epoch_nanoseconds as f64,
            time_zone_id,
            calendar_id,
        );
    }
    create_zoned_date_time_from_parts(
        context,
        prototype,
        year,
        month,
        day,
        time,
        time_zone_id,
        calendar_id,
    )
}

fn temporal_zoned_date_time_with_calendar(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "ZonedDateTime")?;
    if matches!(arguments.first(), None | Some(JsValue::Undefined)) {
        return Err(VmError::type_error(
            "Temporal.ZonedDateTime.prototype.withCalendar requires a calendar",
        ));
    }
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
    let time_zone_id = temporal_time_zone_string(vm, context, time_zone_value)?;
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
    let direction_value = match option {
        JsValue::String(_) => option,
        value => {
            let object = context.require_object(
                &value,
                "Temporal.ZonedDateTime.prototype.getTimeZoneTransition options",
            )?;
            temporal_get_property(vm, context, object, "direction")?
        }
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
    let (calendar_name, options, offset_option, time_zone_name) =
        temporal_zoned_date_time_string_options(vm, context, option_value)?;
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
    let zone = own_string(context, object, "timeZoneId").unwrap_or_else(|| "UTC".into());
    let offset_ns = parse_time_zone_offset_ns(&zone).unwrap_or(0);
    let local_ns = rounded
        .checked_add(offset_ns)
        .ok_or_else(|| VmError::range("Temporal.ZonedDateTime is out of range"))?;
    let day_number = i64::try_from(local_ns.div_euclid(NS_PER_DAY_I128))
        .map_err(|_| VmError::range("Temporal.ZonedDateTime is out of range"))?;
    let (year, month, day) = civil_from_days(day_number);
    let date_time = format!(
        "{}-{}-{}T{}",
        iso_year(year),
        two_digit(month),
        two_digit(day),
        format_plain_time_precision(
            plain_time_from_nanoseconds_i128(local_ns.rem_euclid(NS_PER_DAY_I128)),
            options.precision,
            options.minute_only,
        )
    );
    let offset = if offset_option == "never" {
        String::new()
    } else {
        format_time_zone_offset_ns(offset_ns)
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

fn temporal_zoned_date_time_string_options(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<(String, TemporalStringOptions, String, String), VmError> {
    if matches!(value, JsValue::Undefined) {
        return Ok((
            "auto".into(),
            TemporalStringOptions {
                quantum: 1,
                precision: None,
                minute_only: false,
                mode: TemporalRoundMode::Trunc,
            },
            "auto".into(),
            "auto".into(),
        ));
    }
    let object = context.require_object(&value, "Temporal.ZonedDateTime toString options")?;
    let calendar_name =
        option_string(vm, context, object, "calendarName")?.unwrap_or_else(|| "auto".into());
    if !matches!(
        calendar_name.as_str(),
        "auto" | "always" | "never" | "critical"
    ) {
        return Err(VmError::range("invalid Temporal calendarName"));
    }
    let fractional_value = temporal_get_property(vm, context, object, "fractionalSecondDigits")?;
    let fractional_digits = temporal_fractional_second_digits(vm, context, fractional_value)?;
    let offset = option_string(vm, context, object, "offset")?.unwrap_or_else(|| "auto".into());
    let mode = temporal_round_mode(
        option_string(vm, context, object, "roundingMode")?.unwrap_or_default(),
        TemporalRoundMode::Trunc,
    )?;
    let smallest = option_string(vm, context, object, "smallestUnit")?;
    let time_zone_name =
        option_string(vm, context, object, "timeZoneName")?.unwrap_or_else(|| "auto".into());
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
    Ok((
        calendar_name,
        TemporalStringOptions {
            quantum,
            precision,
            minute_only,
            mode,
        },
        offset,
        time_zone_name,
    ))
}

fn temporal_fractional_second_digits(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<Option<usize>, VmError> {
    match value {
        JsValue::Undefined => Ok(None),
        JsValue::Number(number) => {
            let digits = number.floor();
            if !number.is_finite() || !(0.0..=9.0).contains(&digits) {
                return Err(VmError::range("invalid fractionalSecondDigits"));
            }
            Ok(Some(digits as usize))
        }
        value => {
            let text = vm.to_string_coerce(value, context)?;
            if text == "auto" {
                Ok(None)
            } else {
                Err(VmError::range("invalid fractionalSecondDigits"))
            }
        }
    }
}

fn format_time_zone_offset_ns(offset_ns: i128) -> String {
    let sign = if offset_ns < 0 { '-' } else { '+' };
    let mut remainder = offset_ns.abs();
    let hours = remainder / NS_PER_HOUR_I128;
    remainder %= NS_PER_HOUR_I128;
    let minutes = remainder / NS_PER_MINUTE_I128;
    remainder %= NS_PER_MINUTE_I128;
    let seconds = remainder / NS_PER_SECOND_I128;
    let fraction = remainder % NS_PER_SECOND_I128;
    if seconds == 0 && fraction == 0 {
        format!("{sign}{hours:02}:{minutes:02}")
    } else {
        let mut text = format!("{sign}{hours:02}:{minutes:02}:{seconds:02}");
        if fraction != 0 {
            let mut fraction_text = format!("{fraction:09}");
            while fraction_text.ends_with('0') {
                fraction_text.pop();
            }
            text.push('.');
            text.push_str(&fraction_text);
        }
        text
    }
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
        value => temporal_time_zone_string(vm, context, value)?,
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

fn validate_temporal_now_time_zone(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<(), VmError> {
    if let Some(value) = arguments.first()
        && !matches!(value, JsValue::Undefined)
    {
        temporal_time_zone_string(vm, context, value.clone())?;
    }
    Ok(())
}

fn temporal_now_plain_date_iso(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    validate_temporal_now_time_zone(vm, context, arguments)?;
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
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    validate_temporal_now_time_zone(vm, context, arguments)?;
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
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    validate_temporal_now_time_zone(vm, context, arguments)?;
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
