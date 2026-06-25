//! Date / Intl / Temporal built-ins.
//!
//! The implementation is intentionally a deterministic UTC-oriented subset.
//! It installs real JS-visible constructors, prototypes, descriptors, and a
//! small core of algorithms without trying to replace ICU or full Temporal.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    runtime::{
        JsObject, JsValue, NativeCall, NativeConstruct, NativeContext, ObjectId,
        PropertyDescriptor, PropertyKind,
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
    let year = if (0.0..=99.0).contains(&year) {
        year + 1900.0
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

fn parse_iso_date_string(input: &str) -> Option<f64> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }
    let (date_part, time_part) = match input.find('T').or_else(|| input.find(' ')) {
        Some(index) => (&input[..index], Some(&input[index + 1..])),
        None => (input, None),
    };
    let mut date_pieces = date_part.split('-');
    let year = date_pieces.next()?.parse::<i32>().ok()?;
    let month = parse_fixed_digits(date_pieces.next()?, 2)?;
    let day = parse_fixed_digits(date_pieces.next()?, 2)?;
    if date_pieces.next().is_some()
        || !(1..=12).contains(&month)
        || !(1..=month_day_count(year, month)).contains(&day)
    {
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
    define_hidden(context, prototype, DATE_MARKER, JsValue::Boolean(true))?;
    define_hidden(context, prototype, DATE_VALUE, JsValue::Number(f64::NAN))?;

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
        ("toString", 0, date_to_string as NativeCall),
        ("toUTCString", 0, date_to_utc_string as NativeCall),
        ("toGMTString", 0, date_to_utc_string as NativeCall),
        ("toDateString", 0, date_to_date_string as NativeCall),
        ("toTimeString", 0, date_to_time_string as NativeCall),
        ("toLocaleString", 0, date_to_string as NativeCall),
        ("toLocaleDateString", 0, date_to_date_string as NativeCall),
        ("toLocaleTimeString", 0, date_to_time_string as NativeCall),
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
    ] {
        define_method(context, prototype, name, length, call)?;
    }

    let to_primitive =
        context.register_builtin("[Symbol.toPrimitive]", 1, date_to_primitive, None)?;
    let to_primitive_symbol = context.well_known_symbols().to_primitive;
    context.define_symbol_own_property(
        prototype,
        to_primitive_symbol,
        method_descriptor(to_primitive),
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
        arguments.get(1).cloned().unwrap_or(JsValue::Undefined),
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
    let primitive = vm.to_number(this_value.clone(), context)?;
    if !primitive.is_finite() {
        return Ok(JsValue::Null);
    }
    date_to_iso_string(vm, context, this_value, &[])
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

fn date_to_primitive(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let hint = arguments
        .first()
        .and_then(JsValue::to_js_string)
        .unwrap_or_else(|| "default".into());
    if hint == "number" {
        date_value_of(vm, context, this_value, &[])
    } else if hint == "string" || hint == "default" {
        date_to_string(vm, context, this_value, &[])
    } else {
        Err(VmError::type_error("invalid Date @@toPrimitive hint"))
    }
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

fn temporal_slot_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let getter = context.current_or_global_this();
    let (kind, slot) = context
        .value_object(&getter)
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

fn duration_values_from_args(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
) -> Result<DurationValues, VmError> {
    Ok(DurationValues {
        years: number_or_default(vm, context, arguments, 0, 0.0)?,
        months: number_or_default(vm, context, arguments, 1, 0.0)?,
        weeks: number_or_default(vm, context, arguments, 2, 0.0)?,
        days: number_or_default(vm, context, arguments, 3, 0.0)?,
        hours: number_or_default(vm, context, arguments, 4, 0.0)?,
        minutes: number_or_default(vm, context, arguments, 5, 0.0)?,
        seconds: number_or_default(vm, context, arguments, 6, 0.0)?,
        milliseconds: number_or_default(vm, context, arguments, 7, 0.0)?,
        microseconds: number_or_default(vm, context, arguments, 8, 0.0)?,
        nanoseconds: number_or_default(vm, context, arguments, 9, 0.0)?,
    })
}

fn create_duration(
    context: &mut NativeContext,
    prototype: ObjectId,
    values: DurationValues,
) -> Result<JsValue, VmError> {
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
        JsValue::String(text) => parse_duration(&text).unwrap_or_default(),
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

fn temporal_object_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    object: ObjectId,
    name: &str,
) -> Result<f64, VmError> {
    let value = context.get_property(JsValue::Object(object), name)?;
    if matches!(value, JsValue::Undefined) {
        Ok(0.0)
    } else {
        vm.to_number(value, context)
    }
}

fn parse_duration(text: &str) -> Option<DurationValues> {
    let mut chars = text.strip_prefix('P')?.chars().peekable();
    let mut values = DurationValues::default();
    let mut in_time = false;
    while let Some(ch) = chars.peek().copied() {
        if ch == 'T' {
            in_time = true;
            chars.next();
            continue;
        }
        let mut number = String::new();
        while let Some(digit) = chars.peek().copied() {
            if digit.is_ascii_digit() || digit == '-' || digit == '+' || digit == '.' {
                number.push(digit);
                chars.next();
            } else {
                break;
            }
        }
        let amount = number.parse::<f64>().ok()?;
        match chars.next()? {
            'Y' => values.years = amount,
            'M' if in_time => values.minutes = amount,
            'M' => values.months = amount,
            'W' => values.weeks = amount,
            'D' => values.days = amount,
            'H' => values.hours = amount,
            'S' => values.seconds = amount,
            _ => return None,
        }
    }
    Some(values)
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
    let mut text = String::from("P");
    push_duration_part(&mut text, values.years, "Y");
    push_duration_part(&mut text, values.months, "M");
    push_duration_part(&mut text, values.weeks, "W");
    push_duration_part(&mut text, values.days, "D");
    let mut time = String::new();
    push_duration_part(&mut time, values.hours, "H");
    push_duration_part(&mut time, values.minutes, "M");
    let seconds = values.seconds
        + values.milliseconds / 1_000.0
        + values.microseconds / 1_000_000.0
        + values.nanoseconds / 1_000_000_000.0;
    push_duration_part(&mut time, seconds, "S");
    if !time.is_empty() {
        text.push('T');
        text.push_str(&time);
    }
    if text == "P" { "PT0S".into() } else { text }
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
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    define_temporal_slot_getter(
        context,
        prototype,
        "epochMilliseconds",
        "get epochMilliseconds",
        "Instant",
        "epochMilliseconds",
    )?;
    Ok(())
}

fn create_instant(
    context: &mut NativeContext,
    prototype: ObjectId,
    epoch_ms: f64,
) -> Result<JsValue, VmError> {
    create_temporal_object(
        context,
        prototype,
        "Instant",
        [("epochMilliseconds", JsValue::Number(time_clip(epoch_ms)))],
    )
}

fn temporal_instant_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    let prototype = context
        .constructor_prototype(&new_target)?
        .ok_or_else(|| VmError::runtime("Temporal.Instant prototype missing"))?;
    let epoch_ns = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    create_instant(context, prototype, epoch_ns / 1_000_000.0)
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
    let ms = match item {
        JsValue::String(text) => parse_iso_date_string(&text).unwrap_or(f64::NAN),
        value => {
            if let Some(object) = context.value_object(&value)
                && own_string(context, object, TEMPORAL_KIND).as_deref() == Some("Instant")
            {
                temporal_number_slot(context, object, "epochMilliseconds")
            } else {
                vm.to_number(value, context)?
            }
        }
    };
    create_instant(context, prototype, ms)
}

fn temporal_instant_from_epoch_milliseconds(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_instant_constructor_prototype(context)?;
    let ms = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    create_instant(context, prototype, ms)
}

fn temporal_instant_from_epoch_seconds(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_instant_constructor_prototype(context)?;
    let seconds = vm.to_number(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    create_instant(context, prototype, seconds * 1_000.0)
}

fn instant_ms_from_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    value: JsValue,
) -> Result<f64, VmError> {
    if let Some(object) = context.value_object(&value)
        && own_string(context, object, TEMPORAL_KIND).as_deref() == Some("Instant")
    {
        return Ok(temporal_number_slot(context, object, "epochMilliseconds"));
    }
    match value {
        JsValue::String(text) => Ok(parse_iso_date_string(&text).unwrap_or(f64::NAN)),
        other => vm.to_number(other, context),
    }
}

fn temporal_instant_compare(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let left = instant_ms_from_value(
        vm,
        context,
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
    )?;
    let right = instant_ms_from_value(
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
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "Instant")?;
    Ok(JsValue::String(format_iso(temporal_number_slot(
        context,
        object,
        "epochMilliseconds",
    ))?))
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
    define_method(context, prototype, "valueOf", 0, temporal_value_of)?;
    for (name, getter, slot) in [
        ("year", "get year", "year"),
        ("month", "get month", "month"),
        ("day", "get day", "day"),
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "PlainDate", slot)?;
    }
    Ok(())
}

fn create_plain_date(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
) -> Result<JsValue, VmError> {
    create_temporal_object(
        context,
        prototype,
        "PlainDate",
        [
            ("year", JsValue::Number(year.trunc())),
            ("month", JsValue::Number(month.trunc())),
            ("day", JsValue::Number(day.trunc())),
        ],
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
    validate_plain_date(year, month, day)?;
    create_plain_date(context, prototype, year, month, day)
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
    let date = text.split('T').next().unwrap_or(text);
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parse_fixed_digits(parts.next()?, 2)?;
    let day = parse_fixed_digits(parts.next()?, 2)?;
    if parts.next().is_some() {
        return None;
    }
    Some((year as f64, month as f64, day as f64))
}

fn temporal_plain_date_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainDate")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (year, month, day) = match item {
        JsValue::String(text) => parse_plain_date(&text)
            .ok_or_else(|| VmError::range("invalid Temporal.PlainDate string"))?,
        value => {
            let object = context.require_object(&value, "Temporal.PlainDate.from")?;
            (
                temporal_object_number(vm, context, object, "year")?,
                temporal_object_number(vm, context, object, "month")?,
                temporal_object_number(vm, context, object, "day")?,
            )
        }
    };
    validate_plain_date(year, month, day)?;
    create_plain_date(context, prototype, year, month, day)
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
    let time = text.split('T').next_back().unwrap_or(text);
    let time = time.strip_suffix('Z').unwrap_or(time);
    let mut parts = time.split(':');
    let hour = parse_fixed_digits(parts.next()?, 2)? as f64;
    let minute = parse_fixed_digits(parts.next().unwrap_or("00"), 2)? as f64;
    let seconds_piece = parts.next().unwrap_or("00");
    if parts.next().is_some() {
        return None;
    }
    let (second_text, fraction_text) = seconds_piece
        .split_once('.')
        .map_or((seconds_piece, ""), |(second, fraction)| (second, fraction));
    let second = parse_fixed_digits(second_text, 2)? as f64;
    let mut fraction = fraction_text.chars().take(9).collect::<String>();
    while fraction.len() < 9 {
        fraction.push('0');
    }
    let fraction = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<u32>().ok()?
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

fn format_plain_time(values: PlainTimeValues) -> String {
    let mut text = format!(
        "{}:{}:{}",
        two_digit(values.hour as u32),
        two_digit(values.minute as u32),
        two_digit(values.second as u32)
    );
    let fraction = values.millisecond as u32 * 1_000_000
        + values.microsecond as u32 * 1_000
        + values.nanosecond as u32;
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

fn temporal_plain_time_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainTime")?;
    Ok(JsValue::String(format_plain_time(PlainTimeValues {
        hour: temporal_number_slot(context, object, "hour"),
        minute: temporal_number_slot(context, object, "minute"),
        second: temporal_number_slot(context, object, "second"),
        millisecond: temporal_number_slot(context, object, "millisecond"),
        microsecond: temporal_number_slot(context, object, "microsecond"),
        nanosecond: temporal_number_slot(context, object, "nanosecond"),
    })))
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
    ] {
        define_temporal_slot_getter(context, prototype, name, getter, "PlainDateTime", slot)?;
    }
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
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    create_plain_date_time(context, prototype, year, month, day, time)
}

fn create_plain_date_time(
    context: &mut NativeContext,
    prototype: ObjectId,
    year: f64,
    month: f64,
    day: f64,
    time: PlainTimeValues,
) -> Result<JsValue, VmError> {
    create_temporal_object(
        context,
        prototype,
        "PlainDateTime",
        [
            ("year", JsValue::Number(year.trunc())),
            ("month", JsValue::Number(month.trunc())),
            ("day", JsValue::Number(day.trunc())),
            ("hour", JsValue::Number(time.hour.trunc())),
            ("minute", JsValue::Number(time.minute.trunc())),
            ("second", JsValue::Number(time.second.trunc())),
            ("millisecond", JsValue::Number(time.millisecond.trunc())),
            ("microsecond", JsValue::Number(time.microsecond.trunc())),
            ("nanosecond", JsValue::Number(time.nanosecond.trunc())),
        ],
    )
}

fn temporal_plain_date_time_from(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let prototype = temporal_constructor_prototype(context, "PlainDateTime")?;
    let item = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let (year, month, day, time) = match item {
        JsValue::String(text) => {
            let (year, month, day) = parse_plain_date(&text)
                .ok_or_else(|| VmError::range("invalid Temporal.PlainDateTime"))?;
            let time = if text.contains('T') {
                parse_plain_time(text.split_once('T').map(|(_, time)| time).unwrap_or(""))
                    .ok_or_else(|| VmError::range("invalid Temporal.PlainDateTime"))?
            } else {
                PlainTimeValues::default()
            };
            (year, month, day, time)
        }
        value => {
            let object = context.require_object(&value, "Temporal.PlainDateTime.from")?;
            (
                temporal_object_number(vm, context, object, "year")?,
                temporal_object_number(vm, context, object, "month")?,
                temporal_object_number(vm, context, object, "day")?,
                PlainTimeValues {
                    hour: temporal_object_number(vm, context, object, "hour")?,
                    minute: temporal_object_number(vm, context, object, "minute")?,
                    second: temporal_object_number(vm, context, object, "second")?,
                    millisecond: temporal_object_number(vm, context, object, "millisecond")?,
                    microsecond: temporal_object_number(vm, context, object, "microsecond")?,
                    nanosecond: temporal_object_number(vm, context, object, "nanosecond")?,
                },
            )
        }
    };
    validate_plain_date(year, month, day)?;
    validate_plain_time(time)?;
    create_plain_date_time(context, prototype, year, month, day, time)
}

fn temporal_plain_date_time_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = require_temporal_kind(context, &this_value, "PlainDateTime")?;
    let date = format!(
        "{}-{}-{}",
        iso_year(temporal_number_slot(context, object, "year") as i32),
        two_digit(temporal_number_slot(context, object, "month") as u32),
        two_digit(temporal_number_slot(context, object, "day") as u32)
    );
    let time = format_plain_time(PlainTimeValues {
        hour: temporal_number_slot(context, object, "hour"),
        minute: temporal_number_slot(context, object, "minute"),
        second: temporal_number_slot(context, object, "second"),
        millisecond: temporal_number_slot(context, object, "millisecond"),
        microsecond: temporal_number_slot(context, object, "microsecond"),
        nanosecond: temporal_number_slot(context, object, "nanosecond"),
    });
    Ok(JsValue::String(format!("{date}T{time}")))
}

fn install_temporal_now(context: &mut NativeContext, temporal: ObjectId) -> Result<(), VmError> {
    let now = new_ordinary_object(context, context.object_prototype())?;
    define_method(context, now, "instant", 0, temporal_now_instant)?;
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
