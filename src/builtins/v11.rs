//! V11 RegExp / Annex B / descriptor builtin refinements.
//!
//! This module is a post-V6/V10 augmentation pass. It keeps parser and shared
//! object-model changes out of C scope while tightening JS-visible RegExp and
//! legacy builtin behavior that can be expressed through existing runtime APIs.

use super::regexp;
use crate::{
    runtime::{
        JsValue, NativeCall, NativeContext, ObjectId, ObjectKind, PropertyDescriptor, PropertyKind,
        to_property_key,
    },
    vm::{Vm, VmError},
};

pub(super) fn install(context: &mut NativeContext) -> Result<(), VmError> {
    install_regexp_refinements(context)?;
    install_annex_b_refinements(context)?;
    Ok(())
}

fn method_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, true, false, true)
}

fn constant_descriptor(value: JsValue) -> PropertyDescriptor {
    PropertyDescriptor::data_with(value, false, false, false)
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
    get: NativeCall,
    setter_name: Option<&'static str>,
    set: Option<NativeCall>,
) -> Result<(), VmError> {
    let getter = context.register_builtin(getter_name, 0, get, None)?;
    let setter = match (setter_name, set) {
        (Some(name), Some(call)) => Some(context.register_builtin(name, 1, call, None)?),
        _ => None,
    };
    context.define_own_property(
        target,
        name.into(),
        PropertyDescriptor::accessor(Some(getter), setter, false, true),
    )?;
    Ok(())
}

fn sort_regexp_flags(flags: &str) -> String {
    const ORDER: &[char] = &['d', 'g', 'i', 'm', 's', 'u', 'v', 'y'];
    ORDER.iter().filter(|&&flag| flags.contains(flag)).collect()
}

fn is_callable(value: &JsValue) -> bool {
    matches!(value, JsValue::Function(_) | JsValue::BuiltinFunction(_))
}

fn install_regexp_refinements(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = context
        .regexp_prototype()
        .ok_or_else(|| VmError::runtime("RegExp prototype missing"))?;

    let constructor = context.register_builtin("RegExp", 2, regexp_call, Some(regexp_construct))?;
    let constructor_object = context
        .value_object(&constructor)
        .ok_or_else(|| VmError::runtime("RegExp constructor object missing"))?;
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
    context.set_global("RegExp", constructor.clone());

    define_method(context, constructor_object, "escape", 1, regexp_escape)?;
    for (name, length, call) in [
        ("exec", 1, regexp_exec as NativeCall),
        ("test", 1, regexp_test as NativeCall),
        ("compile", 2, regexp_compile as NativeCall),
        ("toString", 0, regexp_to_string as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    for (name, getter_name, getter) in [
        ("dotAll", "get dotAll", regexp_dot_all_get as NativeCall),
        ("flags", "get flags", regexp_flags_get as NativeCall),
        ("global", "get global", regexp_global_get as NativeCall),
        (
            "hasIndices",
            "get hasIndices",
            regexp_has_indices_get as NativeCall,
        ),
        (
            "ignoreCase",
            "get ignoreCase",
            regexp_ignore_case_get as NativeCall,
        ),
        (
            "multiline",
            "get multiline",
            regexp_multiline_get as NativeCall,
        ),
        ("source", "get source", regexp_source_get as NativeCall),
        ("sticky", "get sticky", regexp_sticky_get as NativeCall),
        ("unicode", "get unicode", regexp_unicode_get as NativeCall),
        (
            "unicodeSets",
            "get unicodeSets",
            regexp_unicode_sets_get as NativeCall,
        ),
    ] {
        define_accessor(context, prototype, name, getter_name, getter, None, None)?;
    }

    let wk = *context.well_known_symbols();
    for (name, symbol, length, call) in [
        (
            "[Symbol.match]",
            wk.match_,
            1,
            regexp_symbol_match as NativeCall,
        ),
        (
            "[Symbol.matchAll]",
            wk.match_all,
            1,
            regexp_symbol_match_all as NativeCall,
        ),
        (
            "[Symbol.search]",
            wk.search,
            1,
            regexp_symbol_search as NativeCall,
        ),
        (
            "[Symbol.split]",
            wk.split,
            2,
            regexp_symbol_split as NativeCall,
        ),
    ] {
        let function = context.register_builtin(name, length, call, None)?;
        context.define_symbol_own_property(prototype, symbol, method_descriptor(function))?;
    }
    Ok(())
}

fn regexp_call(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    make_regexp(vm, context, arguments, None)
}

fn regexp_construct(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: JsValue,
) -> Result<JsValue, VmError> {
    make_regexp(vm, context, arguments, Some(new_target))
}

fn make_regexp(
    vm: &mut Vm,
    context: &mut NativeContext,
    arguments: &[JsValue],
    new_target: Option<JsValue>,
) -> Result<JsValue, VmError> {
    let pattern_arg = arguments.first().cloned().unwrap_or(JsValue::Undefined);
    let flags_arg = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let pattern_is_regexp = regexp_data(context, &pattern_arg).is_some();
    let (pattern, default_flags) =
        if let Some((_, pattern, flags)) = regexp_data(context, &pattern_arg) {
            (pattern, flags)
        } else if matches!(pattern_arg, JsValue::Undefined) {
            (String::new(), String::new())
        } else {
            (vm.to_string_coerce(pattern_arg, context)?, String::new())
        };
    let flags = if matches!(flags_arg, JsValue::Undefined) {
        default_flags
    } else {
        vm.to_string_coerce(flags_arg, context)?
    };
    validate_regexp_flags(&flags)?;
    regexp::compile_regex(&pattern, &flags)
        .map_err(|error| VmError::syntax_error(format!("invalid regular expression: {error}")))?;

    if new_target.is_none() && pattern_is_regexp && arguments.get(1).is_none() {
        return Ok(arguments.first().cloned().unwrap_or(JsValue::Undefined));
    }

    let value = context.create_regexp(pattern, flags)?;
    if let Some(new_target) = new_target
        && let Some(object) = context.value_object(&value)
        && let Some(prototype) = context.constructor_prototype(&new_target)?
    {
        context.set_prototype_of(object, Some(prototype))?;
    }
    Ok(value)
}

fn validate_regexp_flags(flags: &str) -> Result<(), VmError> {
    let mut seen = Vec::new();
    for flag in flags.chars() {
        if !"dgimsuvy".contains(flag) {
            return Err(VmError::syntax_error(format!(
                "invalid regular expression flag `{flag}`"
            )));
        }
        if seen.contains(&flag) {
            return Err(VmError::syntax_error(format!(
                "duplicate regular expression flag `{flag}`"
            )));
        }
        seen.push(flag);
    }
    if flags.contains('u') && flags.contains('v') {
        return Err(VmError::syntax_error(
            "regular expression flags u and v are mutually exclusive",
        ));
    }
    Ok(())
}

fn regexp_data(context: &NativeContext, value: &JsValue) -> Option<(ObjectId, String, String)> {
    let object = context.value_object(value)?;
    match &context.heap().object(object)?.kind {
        ObjectKind::RegExp { pattern, flags } => Some((object, pattern.clone(), flags.clone())),
        ObjectKind::Ordinary | ObjectKind::Array { .. } | ObjectKind::PrimitiveWrapper(_) => None,
    }
}

fn require_regexp(
    context: &NativeContext,
    value: &JsValue,
) -> Result<(ObjectId, String, String), VmError> {
    regexp_data(context, value)
        .ok_or_else(|| VmError::type_error("RegExp method called on non-RegExp"))
}

fn regexp_boolean_get(
    context: &NativeContext,
    this_value: &JsValue,
    flag: char,
) -> Result<JsValue, VmError> {
    let (_, _, flags) = require_regexp(context, this_value)?;
    Ok(JsValue::Boolean(flags.contains(flag)))
}

fn regexp_dot_all_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 's')
}

fn regexp_global_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 'g')
}

fn regexp_has_indices_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 'd')
}

fn regexp_ignore_case_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 'i')
}

fn regexp_multiline_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 'm')
}

fn regexp_sticky_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 'y')
}

fn regexp_unicode_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 'u')
}

fn regexp_unicode_sets_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    regexp_boolean_get(context, &this_value, 'v')
}

fn regexp_flags_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, _, flags) = require_regexp(context, &this_value)?;
    Ok(JsValue::String(sort_regexp_flags(&flags)))
}

fn regexp_source_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, pattern, _) = require_regexp(context, &this_value)?;
    Ok(JsValue::String(escape_regexp_source(&pattern)))
}

fn escape_regexp_source(pattern: &str) -> String {
    if pattern.is_empty() {
        return "(?:)".into();
    }
    let mut result = String::new();
    for ch in pattern.chars() {
        match ch {
            '/' => result.push_str("\\/"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\u{2028}' => result.push_str("\\u2028"),
            '\u{2029}' => result.push_str("\\u2029"),
            _ => result.push(ch),
        }
    }
    result
}

fn regexp_to_string(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, pattern, flags) = require_regexp(context, &this_value)?;
    Ok(JsValue::String(format!(
        "/{}/{}",
        escape_regexp_source(&pattern),
        sort_regexp_flags(&flags)
    )))
}

fn regexp_exec(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    regexp_exec_value(vm, context, this_value, string)
}

fn regexp_test(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let result = regexp_exec(vm, context, this_value, arguments)?;
    Ok(JsValue::Boolean(!matches!(result, JsValue::Null)))
}

fn regexp_exec_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    string: String,
) -> Result<JsValue, VmError> {
    let (_, pattern, flags) = require_regexp(context, &this_value)?;
    let global_or_sticky = flags.contains('g') || flags.contains('y');
    let start_index = if global_or_sticky {
        let last_index = vm.get_property_value(this_value.clone(), "lastIndex", context)?;
        to_length(vm.to_number(last_index, context)?)
    } else {
        0
    };
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|error| VmError::syntax_error(format!("invalid regular expression: {error}")))?;
    let Some(byte_start) = byte_index_from_utf16(&string, start_index) else {
        if global_or_sticky {
            set_last_index(vm, context, this_value, 0)?;
        }
        return Ok(JsValue::Null);
    };
    let Some(captures) = re.captures(&string[byte_start..]) else {
        if global_or_sticky {
            set_last_index(vm, context, this_value, 0)?;
        }
        return Ok(JsValue::Null);
    };
    let Some(full_match) = captures.get(0) else {
        if global_or_sticky {
            set_last_index(vm, context, this_value, 0)?;
        }
        return Ok(JsValue::Null);
    };
    if flags.contains('y') && full_match.start() != 0 {
        set_last_index(vm, context, this_value, 0)?;
        return Ok(JsValue::Null);
    }

    let absolute_start = byte_start + full_match.start();
    let absolute_end = byte_start + full_match.end();
    let match_index = string[..absolute_start].encode_utf16().count();
    let match_end = string[..absolute_end].encode_utf16().count();
    if global_or_sticky {
        let next_index = if absolute_start == absolute_end {
            match_end.saturating_add(1)
        } else {
            match_end
        };
        set_last_index(vm, context, this_value.clone(), next_index)?;
    }
    let elements = (0..captures.len())
        .map(|index| {
            captures.get(index).map_or(JsValue::Undefined, |capture| {
                JsValue::String(capture.as_str().into())
            })
        })
        .collect();
    let result = context.create_array(elements)?;
    let JsValue::Object(array) = result else {
        return Ok(result);
    };
    context.define_own_property(
        array,
        "index".into(),
        PropertyDescriptor::data_with(JsValue::Number(match_index as f64), true, true, true),
    )?;
    context.define_own_property(
        array,
        "input".into(),
        PropertyDescriptor::data_with(JsValue::String(string), true, true, true),
    )?;
    context.define_own_property(
        array,
        "groups".into(),
        PropertyDescriptor::data_with(JsValue::Undefined, true, true, true),
    )?;
    Ok(JsValue::Object(array))
}

fn to_length(value: f64) -> usize {
    if !value.is_finite() || value <= 0.0 {
        0
    } else if value >= usize::MAX as f64 {
        usize::MAX
    } else {
        value.trunc() as usize
    }
}

fn byte_index_from_utf16(text: &str, utf16_index: usize) -> Option<usize> {
    let mut units = 0usize;
    for (byte_index, ch) in text.char_indices() {
        if units >= utf16_index {
            return Some(byte_index);
        }
        units = units.saturating_add(ch.len_utf16());
    }
    (units >= utf16_index).then_some(text.len())
}

fn set_last_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    value: usize,
) -> Result<(), VmError> {
    let _ = vm.set_property_value_from_builtin(
        receiver,
        "lastIndex",
        JsValue::Number(value as f64),
        context,
    )?;
    Ok(())
}

fn regexp_compile(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (object, _, _) = require_regexp(context, &this_value)?;
    let new_regexp = make_regexp(vm, context, arguments, None)?;
    let (_, pattern, flags) = require_regexp(context, &new_regexp)?;
    {
        let regexp_object = context
            .heap_mut()
            .object_mut(object)
            .ok_or_else(|| VmError::runtime("missing RegExp object"))?;
        regexp_object.kind = ObjectKind::RegExp {
            pattern: pattern.clone(),
            flags: flags.clone(),
        };
    }
    refresh_regexp_own_properties(context, object, &pattern, &flags)?;
    Ok(this_value)
}

fn refresh_regexp_own_properties(
    context: &mut NativeContext,
    object: ObjectId,
    pattern: &str,
    flags: &str,
) -> Result<(), VmError> {
    for (name, value) in [
        ("source", JsValue::String(escape_regexp_source(pattern))),
        ("flags", JsValue::String(sort_regexp_flags(flags))),
        ("global", JsValue::Boolean(flags.contains('g'))),
        ("ignoreCase", JsValue::Boolean(flags.contains('i'))),
        ("multiline", JsValue::Boolean(flags.contains('m'))),
        ("dotAll", JsValue::Boolean(flags.contains('s'))),
        ("sticky", JsValue::Boolean(flags.contains('y'))),
        ("unicode", JsValue::Boolean(flags.contains('u'))),
        ("unicodeSets", JsValue::Boolean(flags.contains('v'))),
        ("hasIndices", JsValue::Boolean(flags.contains('d'))),
    ] {
        context.define_own_property(
            object,
            name.into(),
            PropertyDescriptor::data_with(value, false, false, true),
        )?;
    }
    context.define_own_property(
        object,
        "lastIndex".into(),
        PropertyDescriptor::data_with(JsValue::Number(0.0), true, false, false),
    )?;
    Ok(())
}

fn regexp_escape(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    Ok(JsValue::String(escape_regexp_pattern_literal(&string)))
}

fn escape_regexp_pattern_literal(value: &str) -> String {
    let mut result = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index == 0 && ch.is_ascii_alphanumeric() {
            push_hex_escape(&mut result, ch as u32, 2);
            continue;
        }
        match ch {
            '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
            | '/' => {
                result.push('\\');
                result.push(ch);
            }
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            '\u{000B}' => result.push_str("\\v"),
            '\u{000C}' => result.push_str("\\f"),
            ' ' => result.push_str("\\x20"),
            ',' | '-' | '=' | '<' | '>' | '#' | '&' | '!' | '%' | ':' | ';' | '@' | '~' | '\''
            | '"' | '`'
                if ch.is_ascii() =>
            {
                push_hex_escape(&mut result, ch as u32, 2)
            }
            _ => result.push(ch),
        }
    }
    result
}

fn regexp_symbol_match(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, _, flags) = require_regexp(context, &this_value)?;
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    if flags.contains('g') {
        set_last_index(vm, context, this_value.clone(), 0)?;
        let mut values = Vec::new();
        loop {
            let result = regexp_exec_value(vm, context, this_value.clone(), string.clone())?;
            let JsValue::Object(object) = result else {
                break;
            };
            let value = context
                .get_own_property_descriptor(object, "0")
                .and_then(|descriptor| descriptor.value_cloned())
                .unwrap_or(JsValue::Undefined);
            values.push(value);
            if values.len() > 1 << 20 {
                return Err(VmError::runtime_limit("RegExp match result too large"));
            }
        }
        return if values.is_empty() {
            Ok(JsValue::Null)
        } else {
            context.create_array(values)
        };
    }
    regexp_exec_value(vm, context, this_value, string)
}

fn regexp_symbol_match_all(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, _, flags) = require_regexp(context, &this_value)?;
    if !flags.contains('g') && !flags.contains('y') {
        return Err(VmError::type_error(
            "String.prototype.matchAll called with a non-global RegExp",
        ));
    }
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let saved_last_index = vm
        .get_property_value(this_value.clone(), "lastIndex", context)
        .ok()
        .and_then(|value| value.to_number())
        .map(to_length)
        .unwrap_or(0);
    set_last_index(vm, context, this_value.clone(), 0)?;
    let mut matches = Vec::new();
    loop {
        let result = regexp_exec_value(vm, context, this_value.clone(), string.clone())?;
        if matches!(result, JsValue::Null) {
            break;
        }
        matches.push(result);
        if matches.len() > 1 << 20 {
            return Err(VmError::runtime_limit("RegExp matchAll result too large"));
        }
    }
    set_last_index(vm, context, this_value, saved_last_index)?;
    context.create_array(matches)
}

fn regexp_symbol_search(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let saved_last_index = vm
        .get_property_value(this_value.clone(), "lastIndex", context)
        .ok()
        .and_then(|value| value.to_number())
        .map(to_length)
        .unwrap_or(0);
    set_last_index(vm, context, this_value.clone(), 0)?;
    let result = regexp_exec_value(vm, context, this_value.clone(), string)?;
    set_last_index(vm, context, this_value, saved_last_index)?;
    if let JsValue::Object(object) = result {
        Ok(context
            .get_own_property_descriptor(object, "index")
            .and_then(|descriptor| descriptor.value_cloned())
            .unwrap_or(JsValue::Number(-1.0)))
    } else {
        Ok(JsValue::Number(-1.0))
    }
}

fn regexp_symbol_split(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let (_, pattern, flags) = require_regexp(context, &this_value)?;
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let limit = arguments
        .get(1)
        .filter(|value| !matches!(value, JsValue::Undefined))
        .map(|value| vm.to_number(value.clone(), context))
        .transpose()?
        .map(to_length);
    let re = regexp::compile_regex(&pattern, &flags)
        .map_err(|error| VmError::syntax_error(format!("invalid regular expression: {error}")))?;
    let parts = regexp::split(&re, &string, limit)
        .into_iter()
        .map(|part| part.map_or(JsValue::Undefined, JsValue::String))
        .collect();
    context.create_array(parts)
}

fn install_annex_b_refinements(context: &mut NativeContext) -> Result<(), VmError> {
    install_global_annex_b(context)?;
    install_object_annex_b(context)?;
    install_string_annex_b(context)?;
    Ok(())
}

fn install_global_annex_b(context: &mut NativeContext) -> Result<(), VmError> {
    let escape = context.register_builtin("escape", 1, global_escape, None)?;
    let unescape = context.register_builtin("unescape", 1, global_unescape, None)?;
    context.declare_global("escape", escape);
    context.declare_global("unescape", unescape);
    Ok(())
}

fn install_object_annex_b(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = context
        .object_prototype()
        .ok_or_else(|| VmError::runtime("Object prototype missing"))?;
    define_method(
        context,
        prototype,
        "__defineGetter__",
        2,
        object_define_getter,
    )?;
    define_method(
        context,
        prototype,
        "__defineSetter__",
        2,
        object_define_setter,
    )?;
    define_method(
        context,
        prototype,
        "__lookupGetter__",
        1,
        object_lookup_getter,
    )?;
    define_method(
        context,
        prototype,
        "__lookupSetter__",
        1,
        object_lookup_setter,
    )?;
    define_accessor(
        context,
        prototype,
        "__proto__",
        "get __proto__",
        object_proto_get,
        Some("set __proto__"),
        Some(object_proto_set),
    )?;
    Ok(())
}

fn install_string_annex_b(context: &mut NativeContext) -> Result<(), VmError> {
    let prototype = context
        .string_prototype()
        .ok_or_else(|| VmError::runtime("String prototype missing"))?;
    define_method(context, prototype, "trimLeft", 0, string_trim_left)?;
    define_method(context, prototype, "trimRight", 0, string_trim_right)?;
    Ok(())
}

fn global_escape(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let mut output = String::new();
    for unit in string.encode_utf16() {
        let ch = char::from_u32(unit as u32);
        if ch.is_some_and(is_escape_unescaped) {
            output.push(ch.unwrap());
        } else if unit <= 0xFF {
            output.push('%');
            output.push_str(&format!("{unit:02X}"));
        } else {
            output.push_str("%u");
            output.push_str(&format!("{unit:04X}"));
        }
    }
    Ok(JsValue::String(output))
}

fn is_escape_unescaped(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '*' | '@' | '-' | '_' | '+' | '.' | '/')
}

fn global_unescape(
    vm: &mut Vm,
    context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let bytes = string.as_bytes();
    let mut units = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 5 < bytes.len() && matches!(bytes[i + 1], b'u' | b'U') {
            if let Ok(unit) = u16::from_str_radix(&string[i + 2..i + 6], 16) {
                units.push(unit);
                i += 6;
                continue;
            }
        }
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(unit) = u16::from_str_radix(&string[i + 1..i + 3], 16) {
                units.push(unit);
                i += 3;
                continue;
            }
        }
        let ch = string[i..].chars().next().unwrap();
        let mut buf = [0u16; 2];
        units.extend_from_slice(ch.encode_utf16(&mut buf));
        i += ch.len_utf8();
    }
    Ok(JsValue::String(String::from_utf16_lossy(&units)))
}

fn object_define_getter(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    define_legacy_accessor(vm, context, this_value, arguments, true)
}

fn object_define_setter(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    define_legacy_accessor(vm, context, this_value, arguments, false)
}

fn define_legacy_accessor(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    getter: bool,
) -> Result<JsValue, VmError> {
    let object = vm.to_object(this_value, context)?;
    let key = to_property_key(&arguments.first().cloned().unwrap_or(JsValue::Undefined))?;
    let callable = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    if !is_callable(&callable) {
        return Err(VmError::type_error("legacy accessor must be callable"));
    }
    let current = context.get_own_property_descriptor(object, &key);
    let (get, set) = match current.map(|descriptor| descriptor.kind) {
        Some(PropertyKind::Accessor { get, set }) => (get, set),
        _ => (None, None),
    };
    let descriptor = if getter {
        PropertyDescriptor::accessor(Some(callable), set, true, true)
    } else {
        PropertyDescriptor::accessor(get, Some(callable), true, true)
    };
    if !context.define_own_property(object, key, descriptor)? {
        return Err(VmError::type_error("cannot define legacy accessor"));
    }
    Ok(JsValue::Undefined)
}

fn object_lookup_getter(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    lookup_legacy_accessor(context, this_value, arguments, true)
}

fn object_lookup_setter(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    lookup_legacy_accessor(context, this_value, arguments, false)
}

fn lookup_legacy_accessor(
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    getter: bool,
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "legacy accessor lookup")?;
    let key = to_property_key(&arguments.first().cloned().unwrap_or(JsValue::Undefined))?;
    let Some((_, descriptor)) = context.find_property_descriptor(object, &key)? else {
        return Ok(JsValue::Undefined);
    };
    match descriptor.kind {
        PropertyKind::Accessor { get, set } => {
            Ok(if getter { get } else { set }.unwrap_or(JsValue::Undefined))
        }
        PropertyKind::Data { .. } => Ok(JsValue::Undefined),
    }
}

fn object_proto_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some(object) = context.value_object(&this_value) else {
        return Ok(JsValue::Undefined);
    };
    Ok(context
        .get_prototype_of(object)
        .map_or(JsValue::Null, |prototype| context.object_value(prototype)))
}

fn object_proto_set(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some(object) = context.value_object(&this_value) else {
        return Ok(JsValue::Undefined);
    };
    let prototype = match arguments.first().cloned().unwrap_or(JsValue::Undefined) {
        JsValue::Null => None,
        value => match context.value_object(&value) {
            Some(id) => Some(id),
            None => return Ok(JsValue::Undefined),
        },
    };
    context.set_prototype_of(object, prototype)?;
    Ok(JsValue::Undefined)
}

fn string_this(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
) -> Result<String, VmError> {
    if matches!(this_value, JsValue::Null | JsValue::Undefined) {
        return Err(VmError::type_error(
            "String.prototype method called on null or undefined",
        ));
    }
    vm.to_string_coerce(this_value, context)
}

fn string_trim_left(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(
        string_this(vm, context, this_value)?.trim_start().into(),
    ))
}

fn string_trim_right(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(JsValue::String(
        string_this(vm, context, this_value)?.trim_end().into(),
    ))
}

fn push_hex_escape(output: &mut String, value: u32, width: usize) {
    output.push_str(if width == 2 { "\\x" } else { "\\u" });
    output.push_str(&format!("{value:0width$X}"));
}
