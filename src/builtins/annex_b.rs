//! RegExp prototype refinements and ECMAScript Annex B legacy methods.
//!
//! This module is a post-V6/V10 augmentation pass. It keeps parser and shared
//! object-model changes out of C scope while tightening JS-visible RegExp and
//! legacy builtin behavior that can be expressed through existing runtime APIs.

use super::regexp;
use crate::{
    runtime::{
        JsObject, JsValue, NativeCall, NativeContext, ObjectId, ObjectKind, PropertyDescriptor,
        PropertyKind, to_property_key,
    },
    vm::{Vm, VmError},
};

const MAX_REGEXP_REPLACE_OUTPUT_BYTES: usize = 1 << 23;

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
    let species_getter =
        context.register_builtin("get [Symbol.species]", 0, regexp_species_get, None)?;
    context.define_symbol_own_property(
        constructor_object,
        context.well_known_symbols().species,
        PropertyDescriptor::accessor(Some(species_getter), None, false, true),
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
    context.delete_symbol_property(prototype, context.well_known_symbols().to_string_tag, false)?;
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
            "[Symbol.replace]",
            wk.replace,
            2,
            regexp_symbol_replace as NativeCall,
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

fn regexp_species_get(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    Ok(this_value)
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
        ObjectKind::Ordinary
        | ObjectKind::Array { .. }
        | ObjectKind::PrimitiveWrapper(_)
        | ObjectKind::ArrayBuffer { .. }
        | ObjectKind::DataView { .. }
        | ObjectKind::TypedArray { .. }
        | ObjectKind::Iterator { .. }
        | ObjectKind::Generator { .. }
        | ObjectKind::Promise { .. }
        | ObjectKind::Proxy { .. } => None,
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
    if let Some(object) = context.value_object(this_value)
        && context.regexp_prototype() == Some(object)
    {
        return Ok(JsValue::Undefined);
    }
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
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let object = context.require_object(&this_value, "RegExp.prototype.flags")?;
    if context.regexp_prototype() == Some(object) {
        return Ok(JsValue::String(String::new()));
    }
    let mut flags = String::new();
    for (name, flag) in [
        ("hasIndices", 'd'),
        ("global", 'g'),
        ("ignoreCase", 'i'),
        ("multiline", 'm'),
        ("dotAll", 's'),
        ("unicode", 'u'),
        ("unicodeSets", 'v'),
        ("sticky", 'y'),
    ] {
        if vm
            .get_property_value(this_value.clone(), name, context)?
            .to_boolean()
        {
            flags.push(flag);
        }
    }
    Ok(JsValue::String(flags))
}

fn regexp_source_get(
    _vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    _arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    if let Some(object) = context.value_object(&this_value)
        && context.regexp_prototype() == Some(object)
    {
        return Ok(JsValue::String("(?:)".into()));
    }
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
    let last_index = vm.get_property_value(this_value.clone(), "lastIndex", context)?;
    let start_index = if global_or_sticky {
        to_length(vm.to_number(last_index, context)?)
    } else {
        let _ = vm.to_number(last_index, context)?;
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
        set_last_index(vm, context, this_value.clone(), match_end)?;
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
    let groups = create_regexp_groups_object(context, &re, &captures)?;
    context.define_own_property(
        array,
        "input".into(),
        PropertyDescriptor::data_with(JsValue::String(string), true, true, true),
    )?;
    context.define_own_property(
        array,
        "groups".into(),
        PropertyDescriptor::data_with(groups, true, true, true),
    )?;
    Ok(JsValue::Object(array))
}

fn create_regexp_groups_object(
    context: &mut NativeContext,
    regex: &regex::Regex,
    captures: &regex::Captures<'_>,
) -> Result<JsValue, VmError> {
    let names: Vec<(usize, String)> = regex
        .capture_names()
        .enumerate()
        .filter_map(|(index, name)| name.map(|name| (index, name.to_string())))
        .collect();
    if names.is_empty() {
        return Ok(JsValue::Undefined);
    }
    let object = context
        .heap_mut()
        .allocate_object(JsObject::ordinary())
        .ok_or_else(|| VmError::runtime_limit("object arena exhausted"))?;
    for (index, name) in names {
        let value = captures.get(index).map_or(JsValue::Undefined, |capture| {
            JsValue::String(capture.as_str().into())
        });
        context.define_own_property(
            object,
            name,
            PropertyDescriptor::data_with(value, true, true, true),
        )?;
    }
    Ok(JsValue::Object(object))
}

fn to_length(value: f64) -> usize {
    if value.is_nan() || value <= 0.0 {
        0
    } else if !value.is_finite() || value >= usize::MAX as f64 {
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
    set_last_index_number(vm, context, receiver, value as f64)
}

fn set_last_index_number(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    value: f64,
) -> Result<(), VmError> {
    set_last_index_value(vm, context, receiver, JsValue::Number(value))
}

fn set_last_index_value(
    vm: &mut Vm,
    context: &mut NativeContext,
    receiver: JsValue,
    value: JsValue,
) -> Result<(), VmError> {
    vm.set_property_value_strict_from_builtin(receiver, "lastIndex", value, context)?;
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
    _pattern: &str,
    _flags: &str,
) -> Result<(), VmError> {
    context.define_own_property(
        object,
        "lastIndex".into(),
        PropertyDescriptor::data_with(JsValue::Number(0.0), true, false, false),
    )?;
    Ok(())
}

fn regexp_escape(
    _vm: &mut Vm,
    _context: &mut NativeContext,
    _this: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let Some(JsValue::String(string)) = arguments.first() else {
        return Err(VmError::type_error("RegExp.escape requires a string"));
    };
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
            ' ' => push_hex_escape(&mut result, ch as u32, 2),
            ',' | '-' | '=' | '<' | '>' | '#' | '&' | '!' | '%' | ':' | ';' | '@' | '~' | '\''
            | '"' | '`'
                if ch.is_ascii() =>
            {
                push_hex_escape(&mut result, ch as u32, 2)
            }
            _ if is_regexp_escape_whitespace_or_lineterminator(ch) => {
                if (ch as u32) <= 0xff {
                    push_hex_escape(&mut result, ch as u32, 2);
                } else {
                    let mut buffer = [0u16; 2];
                    for unit in ch.encode_utf16(&mut buffer) {
                        push_unicode_escape(&mut result, *unit as u32);
                    }
                }
            }
            _ => result.push(ch),
        }
    }
    result
}

fn is_regexp_escape_whitespace_or_lineterminator(ch: char) -> bool {
    matches!(
        ch,
        '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

fn regexp_symbol_match(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let flags = regexp_replace_flags(vm, context, this_value.clone())?;
    let global = flags.contains('g');
    if !global {
        return regexp_exec_abstract(vm, context, this_value, string);
    }
    let full_unicode = flags.contains('u');
    set_last_index_number(vm, context, this_value.clone(), 0.0)?;
    let mut values = Vec::new();
    loop {
        let result = regexp_exec_abstract(vm, context, this_value.clone(), string.clone())?;
        let JsValue::Object(object) = result else {
            break;
        };
        let result_value = JsValue::Object(object);
        let match_value = vm.get_property_value(result_value.clone(), "0", context)?;
        let match_string = vm.to_string_coerce(match_value, context)?;
        values.push(JsValue::String(match_string.clone()));
        if match_string.is_empty() {
            let last_index = regexp_last_index(vm, context, this_value.clone())?;
            let next_index = advance_string_index(&string, last_index, full_unicode);
            set_last_index_number(vm, context, this_value.clone(), next_index as f64)?;
        }
        if values.len() > 1 << 20 {
            return Err(VmError::runtime_limit("RegExp match result too large"));
        }
    }
    if values.is_empty() {
        Ok(JsValue::Null)
    } else {
        context.create_array(values)
    }
}

fn regexp_exec_abstract(
    vm: &mut Vm,
    context: &mut NativeContext,
    regexp: JsValue,
    string: String,
) -> Result<JsValue, VmError> {
    let exec = vm.get_property_value(regexp.clone(), "exec", context)?;
    if is_callable(&exec) {
        let result = vm.call_value_from_builtin(
            exec,
            regexp.clone(),
            vec![JsValue::String(string)],
            context,
        )?;
        if matches!(result, JsValue::Null) || context.value_object(&result).is_some() {
            return Ok(result);
        }
        return Err(VmError::type_error(
            "RegExp exec result must be an object or null",
        ));
    }
    if regexp_data(context, &regexp).is_some() {
        regexp_exec_value(vm, context, regexp, string)
    } else {
        Err(VmError::type_error("RegExp method called on non-RegExp"))
    }
}

fn regexp_last_index(
    vm: &mut Vm,
    context: &mut NativeContext,
    regexp: JsValue,
) -> Result<usize, VmError> {
    let value = vm.get_property_value(regexp, "lastIndex", context)?;
    Ok(to_length(vm.to_number(value, context)?))
}

fn advance_string_index(string: &str, index: usize, unicode: bool) -> usize {
    if !unicode {
        return index.saturating_add(1);
    }
    let Some(byte_index) = byte_index_from_utf16(string, index) else {
        return index.saturating_add(1);
    };
    let Some(ch) = string[byte_index..].chars().next() else {
        return index.saturating_add(1);
    };
    index.saturating_add(ch.len_utf16())
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
        if let JsValue::Object(object) = &result {
            let result_value = JsValue::Object(*object);
            let match_value = vm.get_property_value(result_value, "0", context)?;
            let match_string = vm.to_string_coerce(match_value, context)?;
            if match_string.is_empty() {
                let last_index = regexp_last_index(vm, context, this_value.clone())?;
                let next_index = advance_string_index(&string, last_index, flags.contains('u'));
                set_last_index_number(vm, context, this_value.clone(), next_index as f64)?;
            }
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
    let previous_last_index = vm.get_property_value(this_value.clone(), "lastIndex", context)?;
    if !previous_last_index.same_value(&JsValue::Number(0.0)) {
        set_last_index_number(vm, context, this_value.clone(), 0.0)?;
    }
    let result = regexp_exec_abstract(vm, context, this_value.clone(), string)?;
    let current_last_index = vm.get_property_value(this_value.clone(), "lastIndex", context)?;
    if !current_last_index.same_value(&previous_last_index) {
        set_last_index_value(vm, context, this_value.clone(), previous_last_index.clone())?;
    }
    if let JsValue::Object(object) = result {
        vm.get_property_value(JsValue::Object(object), "index", context)
    } else {
        Ok(JsValue::Number(-1.0))
    }
}

#[derive(Clone)]
struct ReplaceMatch {
    matched: String,
    position: usize,
    captures: Vec<JsValue>,
    groups: JsValue,
}

fn regexp_symbol_replace(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let replace_value = arguments.get(1).cloned().unwrap_or(JsValue::Undefined);
    let functional_replace = is_callable(&replace_value);
    let replacement = if functional_replace {
        None
    } else {
        Some(vm.to_string_coerce(replace_value.clone(), context)?)
    };
    let flags_value = vm.get_property_value(this_value.clone(), "flags", context)?;
    let flags = vm.to_string_coerce(flags_value, context)?;
    let global = flags.contains('g');
    let full_unicode = flags.contains('u');
    if global {
        set_last_index_number(vm, context, this_value.clone(), 0.0)?;
    }

    let mut results = Vec::new();
    loop {
        let result = regexp_exec_abstract(vm, context, this_value.clone(), string.clone())?;
        let JsValue::Object(object) = result else {
            break;
        };
        let result_value = JsValue::Object(object);
        let matched_value = vm.get_property_value(result_value.clone(), "0", context)?;
        let matched = vm.to_string_coerce(matched_value, context)?;
        let index_value = vm.get_property_value(result_value.clone(), "index", context)?;
        let position =
            to_length(vm.to_number(index_value, context)?).min(string.encode_utf16().count());
        let length_value = vm.get_property_value(result_value.clone(), "length", context)?;
        let length = to_length(vm.to_number(length_value, context)?);
        let mut captures = Vec::new();
        for index in 1..length {
            captures.push(vm.get_property_value(
                result_value.clone(),
                &index.to_string(),
                context,
            )?);
        }
        let groups = vm.get_property_value(result_value.clone(), "groups", context)?;
        results.push(ReplaceMatch {
            matched: matched.clone(),
            position,
            captures,
            groups,
        });
        if !global {
            break;
        }
        if matched.is_empty() {
            let last_index = regexp_last_index(vm, context, this_value.clone())?;
            let next_index = advance_string_index(&string, last_index, full_unicode);
            set_last_index_number(vm, context, this_value.clone(), next_index as f64)?;
        }
        if results.len() > 1 << 20 {
            return Err(VmError::runtime_limit("RegExp replace result too large"));
        }
    }

    if results.is_empty() {
        return Ok(JsValue::String(string));
    }
    let mut accumulated = String::new();
    let mut next_source_position = 0usize;
    for item in results {
        let byte_position = byte_index_from_utf16(&string, item.position).unwrap_or(string.len());
        let match_end = item
            .position
            .saturating_add(item.matched.encode_utf16().count());
        let byte_match_end = byte_index_from_utf16(&string, match_end).unwrap_or(string.len());
        if byte_position > next_source_position {
            push_str_replacement(
                &mut accumulated,
                &string[next_source_position..byte_position],
            )?;
        }
        let replacement_text = if functional_replace {
            let mut args = Vec::with_capacity(item.captures.len() + 4);
            args.push(JsValue::String(item.matched.clone()));
            args.extend(item.captures.clone());
            args.push(JsValue::Number(item.position as f64));
            args.push(JsValue::String(string.clone()));
            if !matches!(item.groups, JsValue::Undefined) {
                args.push(item.groups.clone());
            }
            let value = vm.call_value_from_builtin(
                replace_value.clone(),
                JsValue::Undefined,
                args,
                context,
            )?;
            vm.to_string_coerce(value, context)?
        } else {
            get_substitution(
                replacement.as_deref().unwrap_or_default(),
                &item.matched,
                &string[..byte_position],
                &string[byte_match_end..],
                &item.captures,
                item.groups.clone(),
                vm,
                context,
            )?
        };
        push_str_replacement(&mut accumulated, &replacement_text)?;
        next_source_position = byte_match_end;
    }
    push_str_replacement(&mut accumulated, &string[next_source_position..])?;
    Ok(JsValue::String(accumulated))
}

fn regexp_replace_flags(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
) -> Result<String, VmError> {
    if regexp_data(context, &this_value).is_some() {
        let flags_value = vm.get_property_value(this_value, "flags", context)?;
        return vm.to_string_coerce(flags_value, context);
    }
    if let Some(object) = context.value_object(&this_value)
        && context
            .get_own_property_descriptor(object, "flags")
            .is_none()
    {
        return Ok(String::new());
    }
    let flags_value = vm.get_property_value(this_value, "flags", context)?;
    vm.to_string_coerce(flags_value, context)
}

fn get_substitution(
    template: &str,
    matched: &str,
    prefix: &str,
    suffix: &str,
    captures: &[JsValue],
    groups: JsValue,
    vm: &mut Vm,
    context: &mut NativeContext,
) -> Result<String, VmError> {
    let mut result = String::new();
    let bytes = template.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] != b'$' || index + 1 >= bytes.len() {
            let ch = template[index..].chars().next().unwrap();
            push_char_replacement(&mut result, ch)?;
            index += ch.len_utf8();
            continue;
        }
        match bytes[index + 1] {
            b'$' => {
                push_char_replacement(&mut result, '$')?;
                index += 2;
            }
            b'&' => {
                push_str_replacement(&mut result, matched)?;
                index += 2;
            }
            b'`' => {
                push_str_replacement(&mut result, prefix)?;
                index += 2;
            }
            b'\'' => {
                push_str_replacement(&mut result, suffix)?;
                index += 2;
            }
            b'<' => {
                if let Some(close_offset) = template[index + 2..].find('>') {
                    if matches!(groups, JsValue::Undefined) {
                        push_char_replacement(&mut result, '$')?;
                        index += 1;
                        continue;
                    }
                    let name_start = index + 2;
                    let name_end = name_start + close_offset;
                    let name = &template[name_start..name_end];
                    let groups_object = vm.to_object(groups.clone(), context)?;
                    let capture =
                        vm.get_property_value(context.object_value(groups_object), name, context)?;
                    if !matches!(capture, JsValue::Undefined) {
                        let capture = vm.to_string_coerce(capture, context)?;
                        push_str_replacement(&mut result, &capture)?;
                    }
                    index = name_end + 1;
                } else {
                    push_char_replacement(&mut result, '$')?;
                    index += 1;
                }
            }
            digit @ b'0'..=b'9' => {
                let mut capture_index = (digit - b'0') as usize;
                let mut advance = 2usize;
                if index + 2 < bytes.len() && bytes[index + 2].is_ascii_digit() {
                    let two_digit = capture_index * 10 + (bytes[index + 2] - b'0') as usize;
                    if (1..=captures.len()).contains(&two_digit) {
                        capture_index = two_digit;
                        advance = 3;
                    }
                }
                if (1..=captures.len()).contains(&capture_index) {
                    let capture = captures[capture_index - 1].clone();
                    if !matches!(capture, JsValue::Undefined) {
                        let capture = vm.to_string_coerce(capture, context)?;
                        push_str_replacement(&mut result, &capture)?;
                    }
                    index += advance;
                } else {
                    push_char_replacement(&mut result, '$')?;
                    let ch = template[index + 1..].chars().next().unwrap();
                    push_char_replacement(&mut result, ch)?;
                    index += 1 + ch.len_utf8();
                }
            }
            _ => {
                push_char_replacement(&mut result, '$')?;
                index += 1;
            }
        }
    }
    Ok(result)
}

fn push_char_replacement(output: &mut String, ch: char) -> Result<(), VmError> {
    if output.len().saturating_add(ch.len_utf8()) > MAX_REGEXP_REPLACE_OUTPUT_BYTES {
        Err(VmError::runtime_limit(
            "regexp replacement allocation limit exceeded",
        ))
    } else {
        output.push(ch);
        Ok(())
    }
}

fn push_str_replacement(output: &mut String, value: &str) -> Result<(), VmError> {
    if output.len().saturating_add(value.len()) > MAX_REGEXP_REPLACE_OUTPUT_BYTES {
        Err(VmError::runtime_limit(
            "regexp replacement allocation limit exceeded",
        ))
    } else {
        output.push_str(value);
        Ok(())
    }
}

fn regexp_symbol_split(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
) -> Result<JsValue, VmError> {
    context.require_object(&this_value, "RegExp.prototype[@@split] receiver")?;
    let string = vm.to_string_coerce(
        arguments.first().cloned().unwrap_or(JsValue::Undefined),
        context,
    )?;
    let flags_value = vm.get_property_value(this_value.clone(), "flags", context)?;
    let flags = vm.to_string_coerce(flags_value, context)?;
    let new_flags = if flags.contains('y') {
        flags.clone()
    } else {
        format!("{flags}y")
    };
    let constructor = regexp_species_constructor(vm, context, this_value.clone())?;
    let splitter = vm.construct_value_from_builtin(
        constructor,
        vec![this_value.clone(), JsValue::String(new_flags.clone())],
        context,
    )?;
    let limit = match arguments.get(1) {
        Some(value) if !matches!(value, JsValue::Undefined) => {
            to_uint32(vm.to_number(value.clone(), context)?) as usize
        }
        _ => u32::MAX as usize,
    };
    let mut output = Vec::new();
    if limit == 0 {
        return context.create_array(output);
    }
    let size = string.encode_utf16().count();
    if size == 0 {
        let z = regexp_exec_abstract(vm, context, splitter, string.clone())?;
        if !matches!(z, JsValue::Null) {
            return context.create_array(output);
        }
        output.push(JsValue::String(string));
        return context.create_array(output);
    }

    let unicode_matching = new_flags.contains('u');
    let mut p = 0usize;
    let mut q = 0usize;
    while q < size {
        set_last_index(vm, context, splitter.clone(), q)?;
        let z = regexp_exec_abstract(vm, context, splitter.clone(), string.clone())?;
        if matches!(z, JsValue::Null) {
            q = advance_string_index(&string, q, unicode_matching);
            continue;
        }
        let JsValue::Object(match_object) = z else {
            return Err(VmError::type_error(
                "RegExp split result must be an object or null",
            ));
        };
        let last_index = vm.get_property_value(splitter.clone(), "lastIndex", context)?;
        let mut e = to_length(vm.to_number(last_index, context)?);
        e = e.min(size);
        if e == p {
            q = advance_string_index(&string, q, unicode_matching);
            continue;
        }
        output.push(JsValue::String(utf16_substring(&string, p, q)));
        if output.len() == limit {
            return context.create_array(output);
        }
        p = e;
        let match_value = JsValue::Object(match_object);
        let length_value = vm.get_property_value(match_value.clone(), "length", context)?;
        let number_of_captures = to_length(vm.to_number(length_value, context)?);
        for capture_index in 1..number_of_captures {
            let capture =
                vm.get_property_value(match_value.clone(), &capture_index.to_string(), context)?;
            output.push(capture);
            if output.len() == limit {
                return context.create_array(output);
            }
        }
        q = p;
    }
    output.push(JsValue::String(utf16_substring(&string, p, size)));
    context.create_array(output)
}

fn regexp_species_constructor(
    vm: &mut Vm,
    context: &mut NativeContext,
    rx: JsValue,
) -> Result<JsValue, VmError> {
    let default_constructor = context
        .get_global("RegExp")
        .ok_or_else(|| VmError::runtime("RegExp constructor missing"))?;
    let constructor = vm.get_property_value(rx, "constructor", context)?;
    if matches!(constructor, JsValue::Undefined) {
        return Ok(default_constructor);
    }
    if context.value_object(&constructor).is_none() {
        return Err(VmError::type_error(
            "RegExp species constructor is not an object",
        ));
    }
    let species_symbol = context.well_known_symbols().species;
    let species = vm.get_symbol_property_value_with_receiver_from_builtin(
        constructor.clone(),
        constructor,
        species_symbol,
        context,
    )?;
    if matches!(species, JsValue::Undefined | JsValue::Null) {
        return Ok(default_constructor);
    }
    if !context.is_constructable_value(&species) {
        return Err(VmError::type_error("RegExp species is not a constructor"));
    }
    Ok(species)
}

fn to_uint32(value: f64) -> u32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let int = value.signum() * value.abs().floor();
    int.rem_euclid(4_294_967_296.0) as u32
}

fn utf16_substring(value: &str, start: usize, end: usize) -> String {
    let start = byte_index_from_utf16(value, start).unwrap_or(value.len());
    let end = byte_index_from_utf16(value, end).unwrap_or(value.len());
    value[start.min(value.len())..end.min(value.len()).max(start.min(value.len()))].to_string()
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
    context.declare_global("escape", escape.clone());
    context.declare_global("unescape", unescape.clone());
    context.define_own_property(
        context.global_object(),
        "escape".into(),
        method_descriptor(escape),
    )?;
    context.define_own_property(
        context.global_object(),
        "unescape".into(),
        method_descriptor(unescape),
    )?;
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
    alias_method(context, prototype, "trimStart", "trimLeft")?;
    alias_method(context, prototype, "trimEnd", "trimRight")?;
    for (name, length, call) in [
        ("anchor", 1, string_anchor as NativeCall),
        ("big", 0, string_big as NativeCall),
        ("blink", 0, string_blink as NativeCall),
        ("bold", 0, string_bold as NativeCall),
        ("fixed", 0, string_fixed as NativeCall),
        ("fontcolor", 1, string_fontcolor as NativeCall),
        ("fontsize", 1, string_fontsize as NativeCall),
        ("italics", 0, string_italics as NativeCall),
        ("link", 1, string_link as NativeCall),
        ("small", 0, string_small as NativeCall),
        ("strike", 0, string_strike as NativeCall),
        ("sub", 0, string_sub as NativeCall),
        ("sup", 0, string_sup as NativeCall),
    ] {
        define_method(context, prototype, name, length, call)?;
    }
    Ok(())
}

fn alias_method(
    context: &mut NativeContext,
    target: ObjectId,
    source_name: &'static str,
    alias_name: &'static str,
) -> Result<(), VmError> {
    let value = context
        .get_own_property_descriptor(target, source_name)
        .and_then(|descriptor| descriptor.value_cloned())
        .ok_or_else(|| VmError::runtime(format!("missing String.prototype.{source_name}")))?;
    context.define_own_property(target, alias_name.into(), method_descriptor(value))?;
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
        if bytes[i] == b'%'
            && i + 5 < bytes.len()
            && bytes[i + 1] == b'u'
            && let Ok(unit) = u16::from_str_radix(&string[i + 2..i + 6], 16)
        {
            units.push(unit);
            i += 6;
            continue;
        }
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(unit) = u16::from_str_radix(&string[i + 1..i + 3], 16)
        {
            units.push(unit);
            i += 3;
            continue;
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

fn create_html(
    vm: &mut Vm,
    context: &mut NativeContext,
    this_value: JsValue,
    arguments: &[JsValue],
    tag: &'static str,
    attribute: &'static str,
) -> Result<JsValue, VmError> {
    let string = string_this(vm, context, this_value)?;
    let mut result = String::new();
    result.push('<');
    result.push_str(tag);
    if !attribute.is_empty() {
        let value = vm.to_string_coerce(
            arguments.first().cloned().unwrap_or(JsValue::Undefined),
            context,
        )?;
        result.push(' ');
        result.push_str(attribute);
        result.push_str("=\"");
        result.push_str(&value.replace('"', "&quot;"));
        result.push('"');
    }
    result.push('>');
    result.push_str(&string);
    result.push_str("</");
    result.push_str(tag);
    result.push('>');
    Ok(JsValue::String(result))
}

macro_rules! html_method {
    ($name:ident, $tag:literal) => {
        fn $name(
            vm: &mut Vm,
            context: &mut NativeContext,
            this_value: JsValue,
            arguments: &[JsValue],
        ) -> Result<JsValue, VmError> {
            create_html(vm, context, this_value, arguments, $tag, "")
        }
    };
    ($name:ident, $tag:literal, $attribute:literal) => {
        fn $name(
            vm: &mut Vm,
            context: &mut NativeContext,
            this_value: JsValue,
            arguments: &[JsValue],
        ) -> Result<JsValue, VmError> {
            create_html(vm, context, this_value, arguments, $tag, $attribute)
        }
    };
}

html_method!(string_anchor, "a", "name");
html_method!(string_big, "big");
html_method!(string_blink, "blink");
html_method!(string_bold, "b");
html_method!(string_fixed, "tt");
html_method!(string_fontcolor, "font", "color");
html_method!(string_fontsize, "font", "size");
html_method!(string_italics, "i");
html_method!(string_link, "a", "href");
html_method!(string_small, "small");
html_method!(string_strike, "strike");
html_method!(string_sub, "sub");
html_method!(string_sup, "sup");

fn push_hex_escape(output: &mut String, value: u32, width: usize) {
    output.push_str(if width == 2 { "\\x" } else { "\\u" });
    output.push_str(&format!("{value:0width$x}"));
}

fn push_unicode_escape(output: &mut String, value: u32) {
    output.push_str("\\u");
    output.push_str(&format!("{value:04x}"));
}
