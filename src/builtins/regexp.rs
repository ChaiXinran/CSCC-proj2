//! Pure RegExp algorithm helpers (no VM/runtime wiring).
//!
//! The thin adapter layer in `v6.rs` bridges these into the runtime.

use fancy_regex::{Regex, RegexBuilder};
use icu_properties::{
    CodePointMapData, CodePointSetData, PropertyParser,
    props::{GeneralCategory, GeneralCategoryGroup, Script},
    script::ScriptWithExtensions,
};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::unicode_set::{CodePointSet, UnicodeSet, translate_unicode_sets};

const MAX_REPLACEMENT_OUTPUT_BYTES: usize = 1 << 23;

type ReplacementResult<T> = Result<T, &'static str>;

fn format_unicode_ranges(ranges: impl Iterator<Item = std::ops::RangeInclusive<u32>>) -> String {
    let mut output = String::new();
    for range in ranges {
        let original_start = *range.start();
        let original_end = *range.end();
        for (start, end) in [
            (original_start, original_end.min(0xD7FF)),
            (original_start.max(0xE000), original_end),
        ] {
            if start > end {
                continue;
            }
            if start == end {
                output.push_str(&format!(r"\u{{{start:X}}}"));
            } else {
                output.push_str(&format!(r"\u{{{start:X}}}-\u{{{end:X}}}"));
            }
        }
    }
    output
}

fn general_category_group(value: &str) -> Option<GeneralCategoryGroup> {
    PropertyParser::<GeneralCategory>::new()
        .get_strict(value)
        .map(Into::into)
        .or_else(|| {
            Some(match value {
                "Cased_Letter" | "LC" => GeneralCategoryGroup::CasedLetter,
                "Letter" | "L" => GeneralCategoryGroup::Letter,
                "Mark" | "M" => GeneralCategoryGroup::Mark,
                "Number" | "N" => GeneralCategoryGroup::Number,
                "Separator" | "Z" => GeneralCategoryGroup::Separator,
                "Other" | "C" => GeneralCategoryGroup::Other,
                "Punctuation" | "P" => GeneralCategoryGroup::Punctuation,
                "Symbol" | "S" => GeneralCategoryGroup::Symbol,
                _ => return None,
            })
        })
}

fn ecma_property_ranges(property: &str) -> Option<String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(cached) = cache.lock().ok()?.get(property).cloned() {
        return cached;
    }

    let ranges = if property == "Any" {
        Some(r"\u{0}-\u{D7FF}\u{E000}-\u{10FFFF}".to_owned())
    } else if property == "ASCII" {
        Some(r"\u{0}-\u{7F}".to_owned())
    } else if property == "Assigned" {
        Some(format_unicode_ranges(
            CodePointMapData::<GeneralCategory>::new()
                .iter_ranges()
                .filter(|range| !GeneralCategoryGroup::Unassigned.contains(range.value))
                .map(|range| range.range),
        ))
    } else if let Some((name, value)) = property.split_once('=') {
        match name {
            "General_Category" | "gc" => general_category_group(value).map(|group| {
                format_unicode_ranges(
                    CodePointMapData::<GeneralCategory>::new().iter_ranges_for_group(group),
                )
            }),
            "Script" | "sc" => PropertyParser::<Script>::new()
                .get_strict(value)
                .map(|script| {
                    format_unicode_ranges(
                        CodePointMapData::<Script>::new().iter_ranges_for_value(script),
                    )
                }),
            "Script_Extensions" | "scx" => {
                PropertyParser::<Script>::new()
                    .get_strict(value)
                    .map(|script| {
                        format_unicode_ranges(
                            ScriptWithExtensions::new().get_script_extensions_ranges(script),
                        )
                    })
            }
            _ => None,
        }
    } else if let Some(group) = general_category_group(property) {
        Some(format_unicode_ranges(
            CodePointMapData::<GeneralCategory>::new().iter_ranges_for_group(group),
        ))
    } else {
        CodePointSetData::new_for_ecma262(property.as_bytes())
            .map(|set| format_unicode_ranges(set.iter_ranges()))
    };
    cache
        .lock()
        .ok()?
        .insert(property.to_owned(), ranges.clone());
    ranges
}

fn ecma_property_set(property: &str) -> Option<UnicodeSet> {
    if is_string_property(property) {
        return emoji_string_property_set(property);
    }

    let includes_surrogates = property == "Assigned"
        || matches!(
            property,
            "Surrogate"
                | "Cs"
                | "Other"
                | "C"
                | "General_Category=Surrogate"
                | "General_Category=Cs"
                | "General_Category=Other"
                | "General_Category=C"
                | "gc=Surrogate"
                | "gc=Cs"
                | "gc=Other"
                | "gc=C"
                | "Script=Unknown"
                | "Script=Zzzz"
                | "sc=Unknown"
                | "sc=Zzzz"
                | "Script_Extensions=Unknown"
                | "Script_Extensions=Zzzz"
                | "scx=Unknown"
                | "scx=Zzzz"
        );
    let code_points = if property == "Any" {
        Some(CodePointSet::scalar_values())
    } else if property == "ASCII" {
        Some(CodePointSet::from_range(0, 0x7F))
    } else if property == "Assigned" {
        Some(CodePointSet::from_ranges(
            CodePointMapData::<GeneralCategory>::new()
                .iter_ranges()
                .filter(|range| !GeneralCategoryGroup::Unassigned.contains(range.value))
                .map(|range| range.range),
        ))
    } else if let Some((name, value)) = property.split_once('=') {
        match name {
            "General_Category" | "gc" => general_category_group(value).map(|group| {
                CodePointSet::from_ranges(
                    CodePointMapData::<GeneralCategory>::new().iter_ranges_for_group(group),
                )
            }),
            "Script" | "sc" => PropertyParser::<Script>::new()
                .get_strict(value)
                .map(|script| {
                    CodePointSet::from_ranges(
                        CodePointMapData::<Script>::new().iter_ranges_for_value(script),
                    )
                }),
            "Script_Extensions" | "scx" => {
                PropertyParser::<Script>::new()
                    .get_strict(value)
                    .map(|script| {
                        CodePointSet::from_ranges(
                            ScriptWithExtensions::new().get_script_extensions_ranges(script),
                        )
                    })
            }
            _ => None,
        }
    } else if let Some(group) = general_category_group(property) {
        Some(CodePointSet::from_ranges(
            CodePointMapData::<GeneralCategory>::new().iter_ranges_for_group(group),
        ))
    } else {
        CodePointSetData::new_for_ecma262(property.as_bytes())
            .map(|set| CodePointSet::from_ranges(set.iter_ranges()))
    };
    code_points.map(|code_points| {
        let strings = if includes_surrogates {
            (0xD800..=0xDFFF)
                .filter_map(crate::unicode_set::Utf16String::escaped_surrogate)
                .collect()
        } else {
            std::collections::BTreeSet::new()
        };
        UnicodeSet {
            code_points,
            strings,
        }
    })
}

fn is_string_property(property: &str) -> bool {
    matches!(
        property,
        "Basic_Emoji"
            | "Emoji_Keycap_Sequence"
            | "RGI_Emoji"
            | "RGI_Emoji_Flag_Sequence"
            | "RGI_Emoji_Modifier_Sequence"
            | "RGI_Emoji_Tag_Sequence"
            | "RGI_Emoji_ZWJ_Sequence"
    )
}

fn emoji_string_property_set(property: &str) -> Option<UnicodeSet> {
    static CACHE: OnceLock<Mutex<HashMap<String, UnicodeSet>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(set) = cache.lock().ok()?.get(property).cloned() {
        return Some(set);
    }

    let all_rgi: std::collections::BTreeSet<String> = emojis::iter()
        .flat_map(|emoji| {
            std::iter::once(emoji.as_str().to_owned()).chain(
                emoji
                    .skin_tones()
                    .into_iter()
                    .flatten()
                    .map(|variant| variant.as_str().to_owned()),
            )
        })
        .collect();
    let strings = match property {
        "Emoji_Keycap_Sequence" => all_rgi
            .iter()
            .filter(|value| value.ends_with('\u{20E3}'))
            .cloned()
            .collect(),
        "RGI_Emoji_Flag_Sequence" => all_rgi
            .iter()
            .filter(|value| {
                let chars: Vec<char> = value.chars().collect();
                chars.len() == 2
                    && chars
                        .iter()
                        .all(|ch| ('\u{1F1E6}'..='\u{1F1FF}').contains(ch))
            })
            .cloned()
            .collect(),
        "RGI_Emoji_Modifier_Sequence" => all_rgi
            .iter()
            .filter(|value| {
                value
                    .chars()
                    .any(|ch| ('\u{1F3FB}'..='\u{1F3FF}').contains(&ch))
                    && !value.contains('\u{200D}')
            })
            .cloned()
            .collect(),
        "RGI_Emoji_Tag_Sequence" => all_rgi
            .iter()
            .filter(|value| {
                value
                    .chars()
                    .any(|ch| ('\u{E0020}'..='\u{E007E}').contains(&ch))
            })
            .cloned()
            .collect(),
        "RGI_Emoji_ZWJ_Sequence" => all_rgi
            .iter()
            .filter(|value| value.contains('\u{200D}'))
            .cloned()
            .collect(),
        "Basic_Emoji" => all_rgi
            .iter()
            .filter(|value| {
                let scalar_count = value.chars().filter(|ch| *ch != '\u{FE0F}').count();
                scalar_count == 1
            })
            .cloned()
            .collect(),
        "RGI_Emoji" => all_rgi,
        _ => return None,
    };
    let code_points = if matches!(property, "Basic_Emoji" | "RGI_Emoji") {
        CodePointSetData::new_for_ecma262(b"Emoji_Presentation")
            .map(|set| CodePointSet::from_ranges(set.iter_ranges()))
            .unwrap_or_else(CodePointSet::empty)
    } else {
        CodePointSet::empty()
    };
    let set = UnicodeSet {
        code_points,
        strings,
    };
    cache.lock().ok()?.insert(property.to_owned(), set.clone());
    Some(set)
}

fn expand_ecma_binary_property_escapes(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut output = String::with_capacity(pattern.len());
    let mut in_class = false;
    let mut index = 0usize;
    while index < chars.len() {
        match chars[index] {
            '[' => {
                in_class = true;
                output.push('[');
                index += 1;
            }
            ']' => {
                in_class = false;
                output.push(']');
                index += 1;
            }
            '\\' if matches!(chars.get(index + 1), Some('p') | Some('P'))
                && chars.get(index + 2) == Some(&'{') =>
            {
                let negated = chars[index + 1] == 'P';
                let mut end = index + 3;
                while chars.get(end).is_some_and(|ch| *ch != '}') {
                    end += 1;
                }
                if chars.get(end) != Some(&'}') {
                    output.extend(chars[index..].iter());
                    break;
                }
                let property: String = chars[index + 3..end].iter().collect();
                if !in_class && let Some(mut set) = ecma_property_set(&property) {
                    if negated {
                        set.code_points = set.code_points.complement();
                        let positive_surrogates = set.strings;
                        set.strings = (0xD800..=0xDFFF)
                            .filter_map(crate::unicode_set::Utf16String::escaped_surrogate)
                            .filter(|value| !positive_surrogates.contains(value))
                            .collect();
                    }
                    if negated || !set.strings.is_empty() {
                        output.push_str(&set.to_regex());
                        index = end + 1;
                        continue;
                    }
                }
                if let Some(ranges) = ecma_property_ranges(&property)
                    && (!in_class || !negated)
                {
                    if in_class {
                        if !ranges.is_empty() {
                            output.push_str(&ranges);
                        }
                    } else if ranges.is_empty() {
                        output.push_str(if negated { r"[\s\S]" } else { r"(?!)" });
                    } else {
                        output.push('[');
                        if negated {
                            output.push('^');
                        }
                        output.push_str(&ranges);
                        output.push(']');
                    }
                    index = end + 1;
                    continue;
                }
                output.extend(chars[index..=end].iter());
                index = end + 1;
            }
            '\\' => {
                output.push('\\');
                index += 1;
                if let Some(ch) = chars.get(index) {
                    output.push(*ch);
                    index += 1;
                }
            }
            ch => {
                output.push(ch);
                index += 1;
            }
        }
    }
    output
}

fn count_capturing_groups(pattern: &str) -> usize {
    let mut count = 0;
    let mut chars = pattern.chars().peekable();
    let mut in_class = false;
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                chars.next();
            }
            '[' => in_class = true,
            ']' => in_class = false,
            '(' if !in_class => {
                if chars.peek() != Some(&'?') {
                    count += 1;
                    continue;
                }
                let mut probe = chars.clone();
                probe.next();
                if probe.next() == Some('<') && !matches!(probe.next(), Some('=') | Some('!')) {
                    count += 1;
                }
            }
            _ => {}
        }
    }
    count
}

fn find_regexp_group_end(chars: &[char], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut in_class = false;
    let mut index = start;
    while index < chars.len() {
        match chars[index] {
            '\\' => index += 2,
            '[' if !in_class => {
                in_class = true;
                index += 1;
            }
            ']' if in_class => {
                in_class = false;
                index += 1;
            }
            '(' if !in_class => {
                depth += 1;
                index += 1;
            }
            ')' if !in_class => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
                index += 1;
            }
            _ => index += 1,
        }
    }
    None
}

fn legacy_assertion_quantifier(chars: &[char], start: usize) -> Option<(usize, bool, bool)> {
    let (mut end, minimum_is_zero) = match chars.get(start)? {
        '*' | '?' => (start + 1, true),
        '+' => (start + 1, false),
        '{' => {
            let mut index = start + 1;
            let minimum_start = index;
            while chars.get(index).is_some_and(char::is_ascii_digit) {
                index += 1;
            }
            if minimum_start == index {
                return None;
            }
            let minimum: usize = chars[minimum_start..index]
                .iter()
                .collect::<String>()
                .parse()
                .ok()?;
            if chars.get(index) == Some(&',') {
                index += 1;
                while chars.get(index).is_some_and(char::is_ascii_digit) {
                    index += 1;
                }
            }
            if chars.get(index) != Some(&'}') {
                return None;
            }
            (index + 1, minimum == 0)
        }
        _ => return None,
    };
    let lazy = chars.get(end) == Some(&'?');
    if lazy {
        end += 1;
    }
    Some((end, minimum_is_zero, lazy))
}

fn lower_legacy_quantifiable_assertions(pattern: &str) -> String {
    let chars: Vec<char> = pattern.chars().collect();
    let mut output = String::with_capacity(pattern.len());
    let mut index = 0usize;
    while index < chars.len() {
        if chars.get(index) == Some(&'\\') {
            output.push(chars[index]);
            index += 1;
            if let Some(ch) = chars.get(index) {
                output.push(*ch);
                index += 1;
            }
            continue;
        }
        let is_lookahead = chars.get(index) == Some(&'(')
            && chars.get(index + 1) == Some(&'?')
            && matches!(chars.get(index + 2), Some('=') | Some('!'));
        if !is_lookahead {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let Some(group_end) = find_regexp_group_end(&chars, index) else {
            output.extend(chars[index..].iter());
            break;
        };
        let Some((quantifier_end, minimum_is_zero, lazy)) =
            legacy_assertion_quantifier(&chars, group_end + 1)
        else {
            output.extend(chars[index..=group_end].iter());
            index = group_end + 1;
            continue;
        };
        let assertion: String = chars[index..=group_end].iter().collect();
        if minimum_is_zero {
            if lazy {
                output.push_str("(?:|");
                output.push_str(&assertion);
                output.push(')');
            } else {
                output.push_str("(?:");
                output.push_str(&assertion);
                output.push('|');
                output.push(')');
            }
        } else {
            output.push_str(&assertion);
        }
        index = quantifier_end;
    }
    output
}

/// Compile a JS regex pattern + flags string into a Rust [`Regex`].
/// Returns `Err(message)` if the pattern or flags are invalid.
pub fn compile_regex(pattern: &str, flags: &str) -> Result<Regex, String> {
    let lowered_pattern = if flags.contains('u') || flags.contains('v') {
        pattern.to_owned()
    } else {
        lower_legacy_quantifiable_assertions(pattern)
    };
    let set_pattern = if flags.contains('v') {
        translate_unicode_sets(&lowered_pattern, &ecma_property_set)?
    } else {
        lowered_pattern
    };
    let property_pattern = expand_ecma_binary_property_escapes(&set_pattern);
    let translated_pattern = translate_js_pattern_for_rust(&property_pattern, flags);
    let scoped_flags: String = flags
        .chars()
        .filter(|flag| matches!(flag, 'i' | 'm' | 's'))
        .collect();
    let translated_pattern = if scoped_flags.is_empty() {
        translated_pattern
    } else {
        format!("(?{scoped_flags}:{translated_pattern})")
    };
    let builder = RegexBuilder::new(&translated_pattern);
    for flag in flags.chars() {
        match flag {
            'i' | 'm' | 's' | 'u' | 'v' | 'g' | 'y' | 'd' => {}
            other => return Err(format!("invalid flag `{other}`")),
        }
    }
    builder.build().map_err(|e| e.to_string())
}

fn translate_js_pattern_for_rust(pattern: &str, flags: &str) -> String {
    // Native strings store Unicode scalar values rather than exposed UTF-16
    // code units. A legacy JS pattern that explicitly matches a high-surrogate
    // followed by a low-surrogate therefore corresponds to one astral scalar.
    let normalized_surrogate_pairs =
        pattern.replace(r"[\uD800-\uDBFF][\uDC00-\uDFFF]", r"[\u{10000}-\u{10FFFF}]");
    let pattern = normalized_surrogate_pairs.as_str();
    let needs_dot = pattern.contains('.');
    // \0 in JS regex = null char; Rust regex treats \0 as a backreference (error).
    let needs_null = pattern.contains("\\0");
    // JavaScript control-letter escapes (`\cA` through `\cZ`) are not
    // recognized by the Rust regex parser.
    let needs_control_escape = pattern.contains("\\c");
    let needs_word_escape = ["\\w", "\\W", "\\b", "\\B"]
        .iter()
        .any(|escape| pattern.contains(escape));
    let unicode_mode = flags.contains('u') || flags.contains('v');
    let capture_count = count_capturing_groups(pattern);
    let needs_legacy_translation = !unicode_mode && pattern.contains('\\');
    let needs_legacy_class_open = !unicode_mode
        && pattern
            .chars()
            .filter(|character| *character == '[')
            .count()
            > 1;
    let needs_legacy_class_escape_range = !unicode_mode
        && [r"\d-", r"\D-", r"\s-", r"\S-", r"\w-", r"\W-"]
            .iter()
            .any(|escape| pattern.contains(escape));
    if !needs_null
        && !needs_control_escape
        && !needs_word_escape
        && !needs_legacy_translation
        && !needs_legacy_class_open
        && !needs_legacy_class_escape_range
        && (!needs_dot || (flags.contains('s') && unicode_mode))
    {
        return pattern.to_string();
    }
    let mut output = String::with_capacity(pattern.len() + 16);
    let mut chars = pattern.chars().peekable();
    let mut in_class = false;
    let mut dot_all = flags.contains('s');
    let mut ignore_case = flags.contains('i');
    let mut group_modifiers = Vec::new();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                let js_word = if unicode_mode && ignore_case {
                    r"A-Za-z0-9_\u{017F}\u{212A}"
                } else {
                    "A-Za-z0-9_"
                };
                if !in_class && matches!(next, 'b' | 'B') {
                    let boundary = format!(
                        r"(?:(?<!(?-i:[{js_word}]))(?=(?-i:[{js_word}]))|(?<=(?-i:[{js_word}]))(?!(?-i:[{js_word}])))"
                    );
                    if next == 'b' {
                        output.push_str(&boundary);
                    } else {
                        output.push_str("(?!");
                        output.push_str(&boundary);
                        output.push(')');
                    }
                } else if in_class && next == 'w' {
                    output.push_str(js_word);
                } else if in_class && next == 'W' {
                    output.push_str(r"\W");
                } else if next == 'w' {
                    output.push_str("(?-i:[");
                    output.push_str(js_word);
                    output.push_str("])");
                } else if next == 'W' {
                    output.push_str("(?-i:[^");
                    output.push_str(js_word);
                    output.push_str("])");
                } else if next == '0' && !chars.peek().is_some_and(char::is_ascii_digit) {
                    output.push_str(r"\x00");
                } else if !unicode_mode && matches!(next, '1'..='7') {
                    let mut digits = String::from(next);
                    while digits.len() < 3
                        && chars.peek().is_some_and(|digit| matches!(digit, '0'..='7'))
                    {
                        digits.push(chars.next().unwrap_or('0'));
                    }
                    let decimal = digits.parse::<usize>().unwrap_or(0);
                    if decimal <= capture_count {
                        output.push('\\');
                        output.push_str(&digits);
                    } else {
                        let mut octal_len = digits.len();
                        while octal_len > 1
                            && u16::from_str_radix(&digits[..octal_len], 8)
                                .is_ok_and(|value| value > 0xff)
                        {
                            octal_len -= 1;
                        }
                        let value =
                            u8::from_str_radix(&digits[..octal_len], 8).unwrap_or(next as u8);
                        output.push_str(&format!(r"\x{value:02X}"));
                        output.push_str(&digits[octal_len..]);
                    }
                } else if in_class
                    && !unicode_mode
                    && matches!(next, 'd' | 'D' | 's' | 'S' | 'w' | 'W')
                    && chars.peek() == Some(&'-')
                {
                    chars.next();
                    output.push('\\');
                    output.push(next);
                    output.push_str(r"\-");
                } else if next == 'c'
                    && let Some(control) = chars.peek().copied()
                    && (control.is_ascii_alphabetic()
                        || (!unicode_mode
                            && in_class
                            && (control.is_ascii_digit() || control == '_')))
                {
                    chars.next();
                    let code = (control.to_ascii_uppercase() as u8) & 0x1f;
                    output.push_str(&format!(r"\x{code:02X}"));
                } else if !unicode_mode && next == 'c' {
                    output.push_str(r"\\c");
                } else if !unicode_mode
                    && ((matches!(next, 'x' | 'u')
                        && !(next == 'u' && chars.peek() == Some(&'{'))
                        && {
                            let required = if next == 'x' { 2 } else { 4 };
                            let mut probe = chars.clone();
                            !(0..required).all(|_| {
                                probe.next().is_some_and(|digit| digit.is_ascii_hexdigit())
                            })
                        })
                        || (!matches!(
                            next,
                            'd' | 'D'
                                | 's'
                                | 'S'
                                | 'f'
                                | 'n'
                                | 'r'
                                | 't'
                                | 'v'
                                | 'b'
                                | 'B'
                                | 'x'
                                | 'u'
                                | 'k'
                        ) && !is_js_regexp_syntax_character(next)))
                {
                    output.push(next);
                } else {
                    output.push(ch);
                    output.push(next);
                }
            } else {
                output.push(ch);
            }
            continue;
        }
        if ch == '[' {
            if in_class && !flags.contains('v') {
                output.push_str(r"\[");
            } else {
                in_class = true;
                output.push(ch);
            }
            continue;
        }
        if ch == '-' && in_class && !unicode_mode && chars.peek() == Some(&'\\') {
            let mut probe = chars.clone();
            probe.next();
            if probe
                .next()
                .is_some_and(|escape| matches!(escape, 'd' | 'D' | 's' | 'S' | 'w' | 'W'))
            {
                output.push_str(r"\-");
                continue;
            }
        }
        if ch == '(' && !in_class {
            group_modifiers.push((dot_all, ignore_case));
            output.push(ch);
            if chars.peek() == Some(&'?') {
                let mut probe = chars.clone();
                probe.next();
                let mut add_dot_all = false;
                let mut remove_dot_all = false;
                let mut add_ignore_case = false;
                let mut remove_ignore_case = false;
                let mut saw_modifier = false;
                let mut removing = false;
                for flag in probe {
                    match flag {
                        'i' => {
                            saw_modifier = true;
                            if removing {
                                remove_ignore_case = true;
                            } else {
                                add_ignore_case = true;
                            }
                        }
                        'm' => saw_modifier = true,
                        's' => {
                            saw_modifier = true;
                            if removing {
                                remove_dot_all = true;
                            } else {
                                add_dot_all = true;
                            }
                        }
                        '-' if !removing => removing = true,
                        ':' if saw_modifier || removing => {
                            if add_dot_all {
                                dot_all = true;
                            }
                            if remove_dot_all {
                                dot_all = false;
                            }
                            if add_ignore_case {
                                ignore_case = true;
                            }
                            if remove_ignore_case {
                                ignore_case = false;
                            }
                            break;
                        }
                        _ => break,
                    }
                }
            }
            continue;
        }
        if ch == ')' && !in_class {
            output.push(ch);
            if let Some((parent_dot_all, parent_ignore_case)) = group_modifiers.pop() {
                dot_all = parent_dot_all;
                ignore_case = parent_ignore_case;
            }
            continue;
        }
        if ch == ']' {
            in_class = false;
            output.push(ch);
            continue;
        }
        if ch == '.' && !in_class {
            if dot_all && unicode_mode {
                output.push(ch);
            } else if dot_all {
                output.push_str(r"[^\u{10000}-\u{10FFFF}]");
            } else if unicode_mode {
                output.push_str(r"[^\n\r\u{2028}\u{2029}]");
            } else {
                output.push_str(r"[^\n\r\u{2028}\u{2029}\u{10000}-\u{10FFFF}]");
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn is_js_regexp_syntax_character(ch: char) -> bool {
    matches!(
        ch,
        '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '/'
    )
}

/// Returns `true` if the flags string contains the global flag `g`.
pub fn is_global(flags: &str) -> bool {
    flags.contains('g')
}

/// Returns the index (in UTF-16 code units) of the first match of `regex` in
/// `text`, or `None` if there is no match. Used by `String.prototype.search`.
pub fn search(regex: &Regex, text: &str) -> Option<usize> {
    regex.find(text).ok().flatten().map(|m| {
        if text.is_ascii() {
            m.start()
        } else {
            text[..m.start()].encode_utf16().count()
        }
    })
}

/// Returns the captures for the first match of `regex` in `text`.
/// The returned vector has the full match at index 0 followed by capture groups
/// (as `Option<String>` where `None` represents an unmatched optional group).
pub fn exec_once(regex: &Regex, text: &str) -> Option<Vec<Option<String>>> {
    regex.captures(text).ok().flatten().map(|caps| {
        (0..caps.len())
            .map(|i| caps.get(i).map(|m| m.as_str().to_owned()))
            .collect()
    })
}

/// Returns all non-overlapping full-match strings (global match).
pub fn exec_global(regex: &Regex, text: &str) -> Vec<String> {
    regex
        .find_iter(text)
        .filter_map(Result::ok)
        .map(|m| m.as_str().to_owned())
        .collect()
}

/// Expands ES replacement patterns inside `template`:
///   `$&`  → the entire matched substring
///   `$``  → the portion of the string before the match
///   `$'`  → the portion after the match
///   `$n`  → the n-th capture group (1-indexed; `$0` is ignored)
///   `$$`  → a literal `$`
pub(crate) fn expand_replacement(
    template: &str,
    full_match: &str,
    captures: &[Option<&str>],
    before: &str,
    after: &str,
) -> ReplacementResult<String> {
    let mut result = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'$' => {
                    push_checked(&mut result, '$')?;
                    i += 2;
                }
                b'&' => {
                    push_str_checked(&mut result, full_match)?;
                    i += 2;
                }
                b'`' => {
                    push_str_checked(&mut result, before)?;
                    i += 2;
                }
                b'\'' => {
                    push_str_checked(&mut result, after)?;
                    i += 2;
                }
                d if d.is_ascii_digit() && d != b'0' => {
                    // Try two-digit first ($nn), then one-digit ($n).
                    let mut group_num = (d - b'0') as usize;
                    let mut advance = 2;
                    if i + 2 < bytes.len() {
                        let d2 = bytes[i + 2];
                        if d2.is_ascii_digit() {
                            let two = group_num * 10 + (d2 - b'0') as usize;
                            if two < captures.len() {
                                group_num = two;
                                advance = 3;
                            }
                        }
                    }
                    if group_num < captures.len() {
                        if let Some(cap) = captures[group_num] {
                            push_str_checked(&mut result, cap)?;
                        }
                        // unmatched group → empty string (omit)
                    } else {
                        // No such group — keep literal text.
                        push_str_checked(&mut result, &template[i..i + advance])?;
                    }
                    i += advance;
                }
                _ => {
                    push_checked(&mut result, '$')?;
                    i += 1;
                }
            }
        } else {
            let ch = template[i..].chars().next().unwrap_or('\0');
            push_checked(&mut result, ch)?;
            i += ch.len_utf8().max(1);
        }
    }
    Ok(result)
}

fn push_checked(result: &mut String, ch: char) -> ReplacementResult<()> {
    if result.len().saturating_add(ch.len_utf8()) > MAX_REPLACEMENT_OUTPUT_BYTES {
        return Err("regexp replacement allocation limit exceeded");
    }
    result.push(ch);
    Ok(())
}

fn push_str_checked(result: &mut String, value: &str) -> ReplacementResult<()> {
    if result.len().saturating_add(value.len()) > MAX_REPLACEMENT_OUTPUT_BYTES {
        return Err("regexp replacement allocation limit exceeded");
    }
    result.push_str(value);
    Ok(())
}

/// Replaces the first match of `regex` in `text` with `replacement`, expanding
/// ES replacement patterns (`$&`, `$1`, etc.).
pub fn replace_first(regex: &Regex, text: &str, replacement: &str) -> ReplacementResult<String> {
    if !replacement.contains('$') {
        return replace_first_literal(regex, text, replacement);
    }
    let Some(caps) = regex
        .captures(text)
        .map_err(|_| "regexp execution limit exceeded")?
    else {
        return Ok(text.to_owned());
    };
    let m = caps.get(0).unwrap();
    let (before, full_match, after) = (&text[..m.start()], m.as_str(), &text[m.end()..]);
    let groups: Vec<Option<&str>> = (0..caps.len())
        .map(|i| caps.get(i).map(|c| c.as_str()))
        .collect();
    let repl = expand_replacement(replacement, full_match, &groups, before, after)?;
    let mut result = String::new();
    push_str_checked(&mut result, before)?;
    push_str_checked(&mut result, &repl)?;
    push_str_checked(&mut result, after)?;
    Ok(result)
}

/// Replaces all matches of `regex` in `text` with `replacement`, expanding ES
/// replacement patterns.
pub fn replace_all(regex: &Regex, text: &str, replacement: &str) -> ReplacementResult<String> {
    if !replacement.contains('$') {
        return replace_all_literal(regex, text, replacement);
    }
    let mut result = String::new();
    let mut last_end = 0;
    for caps in regex.captures_iter(text) {
        let caps = caps.map_err(|_| "regexp execution limit exceeded")?;
        let m = caps.get(0).unwrap();
        let before = &text[last_end..m.start()];
        let full_match = m.as_str();
        let after = &text[m.end()..]; // everything after this match
        let groups: Vec<Option<&str>> = (0..caps.len())
            .map(|i| caps.get(i).map(|c| c.as_str()))
            .collect();
        let repl = expand_replacement(replacement, full_match, &groups, before, after)?;
        push_str_checked(&mut result, before)?;
        push_str_checked(&mut result, &repl)?;
        last_end = m.end();
        // Zero-length match guard: advance by at least one char to avoid infinite loop.
        if m.start() == m.end() && m.end() < text.len() {
            let ch = text[m.end()..].chars().next().unwrap_or('\0');
            push_checked(&mut result, ch)?;
            last_end = m.end() + ch.len_utf8();
        }
    }
    push_str_checked(&mut result, &text[last_end..])?;
    Ok(result)
}

fn replace_first_literal(
    regex: &Regex,
    text: &str,
    replacement: &str,
) -> ReplacementResult<String> {
    let Some(m) = regex
        .find(text)
        .map_err(|_| "regexp execution limit exceeded")?
    else {
        return Ok(text.to_owned());
    };
    let mut result = String::with_capacity(
        text.len()
            .saturating_sub(m.end().saturating_sub(m.start()))
            .saturating_add(replacement.len()),
    );
    push_str_checked(&mut result, &text[..m.start()])?;
    push_str_checked(&mut result, replacement)?;
    push_str_checked(&mut result, &text[m.end()..])?;
    Ok(result)
}

fn replace_all_literal(regex: &Regex, text: &str, replacement: &str) -> ReplacementResult<String> {
    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    for m in regex.find_iter(text) {
        let m = m.map_err(|_| "regexp execution limit exceeded")?;
        push_str_checked(&mut result, &text[last_end..m.start()])?;
        push_str_checked(&mut result, replacement)?;
        last_end = m.end();
        if m.start() == m.end() && m.end() < text.len() {
            let ch = text[m.end()..].chars().next().unwrap_or('\0');
            push_checked(&mut result, ch)?;
            last_end = m.end() + ch.len_utf8();
        }
    }
    push_str_checked(&mut result, &text[last_end..])?;
    Ok(result)
}

/// Detailed match info for a single match, used by function-replacement callers.
pub struct MatchDetail {
    pub full_match: String,
    /// Capture groups: index 1…n (index 0 is the full match, kept for symmetry).
    pub captures: Vec<Option<String>>,
    /// UTF-16 start index of the match in the original string.
    pub index: usize,
    /// UTF-8 byte range of the match in the original string.
    pub byte_start: usize,
    pub byte_end: usize,
}

/// Iterates all (non-overlapping) matches of `regex` in `text`, returning
/// [`MatchDetail`] entries. Used by builtin replace with function callback.
pub fn matches_with_detail(regex: &Regex, text: &str, global: bool) -> Vec<MatchDetail> {
    let mut out = Vec::new();
    let mut indexed_byte = 0usize;
    let mut indexed_utf16 = 0usize;
    for caps in regex.captures_iter(text).filter_map(Result::ok) {
        let m = caps.get(0).unwrap();
        let index = if text.is_ascii() {
            m.start()
        } else {
            indexed_utf16 += text[indexed_byte..m.start()].encode_utf16().count();
            indexed_byte = m.start();
            indexed_utf16
        };
        let full_match = m.as_str().to_owned();
        let captures = (0..caps.len())
            .map(|i| caps.get(i).map(|c| c.as_str().to_owned()))
            .collect();
        out.push(MatchDetail {
            full_match,
            captures,
            index,
            byte_start: m.start(),
            byte_end: m.end(),
        });
        if !global {
            break;
        }
    }
    out
}

/// Splits `text` by every match of `regex`, **including** capture groups in the
/// result as specified by ECMAScript `String.prototype.split`.
pub fn split(regex: &Regex, text: &str, limit: Option<usize>) -> Vec<Option<String>> {
    let limit = limit.unwrap_or(usize::MAX);
    if limit == 0 {
        return vec![];
    }

    let mut result: Vec<Option<String>> = Vec::new();
    let mut last_end = 0;

    for caps in regex.captures_iter(text).filter_map(Result::ok) {
        let m = caps.get(0).unwrap();
        if result.len() >= limit {
            break;
        }
        // Push the substring before this match.
        result.push(Some(text[last_end..m.start()].to_owned()));
        if result.len() >= limit {
            break;
        }
        // Push capture groups (indices 1…).
        for i in 1..caps.len() {
            if result.len() >= limit {
                break;
            }
            result.push(caps.get(i).map(|c| c.as_str().to_owned()));
        }
        last_end = m.end();
    }

    if result.len() < limit {
        result.push(Some(text[last_end..].to_owned()));
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_nul_escape_compiles_for_rust_regex() {
        let regex = compile_regex(r"[\0\t]", "").expect("JS NUL escape should compile");

        assert!(regex.is_match("\0").unwrap());
        assert!(regex.is_match("\t").unwrap());
        assert!(!regex.is_match("x").unwrap());
    }

    #[test]
    fn js_control_letter_escape_compiles_for_rust_regex() {
        let regex = compile_regex(r"^[^\cX]+$", "").expect("JS control escape should compile");

        assert!(regex.is_match("plain text").unwrap());
        assert!(!regex.is_match("\u{18}").unwrap());
    }

    #[test]
    fn numeric_backreferences_require_the_same_captured_text() {
        let regex = compile_regex(
            r"^(?:[0-9a-fA-F]{2}([-:\s]))([0-9a-fA-F]{2}\1){4}([0-9a-fA-F]{2})$",
            "",
        )
        .expect("numeric backreference should compile");

        assert!(regex.is_match("00:11:22:33:44:55").unwrap());
        assert!(regex.is_match("00-11-22-33-44-55").unwrap());
        assert!(!regex.is_match("00:11-22:33:44:55").unwrap());
    }

    #[test]
    fn non_unicode_character_class_accepts_a_literal_open_bracket() {
        let regex = compile_regex(r"[-[\]{}()*+?.,\\^$|#\s]", "g")
            .expect("literal open bracket in a legacy character class should compile");

        assert!(regex.is_match("[").unwrap());
        assert!(regex.is_match("]").unwrap());
        assert!(regex.is_match(" ").unwrap());
        assert!(!regex.is_match("a").unwrap());
    }

    #[test]
    fn legacy_surrogate_pair_range_matches_an_astral_scalar() {
        let regex = compile_regex(r"[\uD800-\uDBFF][\uDC00-\uDFFF]", "g")
            .expect("legacy surrogate-pair range should compile");

        assert!(regex.is_match("😀").unwrap());
        assert!(!regex.is_match("a").unwrap());
    }

    #[test]
    fn legacy_class_escape_range_treats_the_hyphen_as_a_literal() {
        let regex =
            compile_regex(r"^[^\s-_]$", "").expect("legacy class-escape range should compile");

        assert!(regex.is_match("a").unwrap());
        assert!(regex.is_match("s").unwrap());
        assert!(!regex.is_match(" ").unwrap());
        assert!(!regex.is_match("-").unwrap());
        assert!(!regex.is_match("_").unwrap());
    }

    #[test]
    fn scoped_modifiers_add_and_remove_flags() {
        let regex =
            compile_regex(r"^(?i:a)(?-i:b)(?s:.)$", "i").expect("scoped modifiers should compile");

        assert!(regex.is_match("Ab\n").unwrap());
        assert!(!regex.is_match("AB\n").unwrap());
    }

    #[test]
    fn scoped_dot_all_does_not_leak_outside_its_group() {
        let regex = compile_regex(r"^(?s:.).$", "u").expect("scoped dotAll should compile");

        assert!(regex.is_match("\na").unwrap());
        assert!(!regex.is_match("a\n").unwrap());
    }

    #[test]
    fn legacy_quantifiable_assertions_lower_to_equivalent_patterns() {
        let greedy_optional =
            compile_regex(r"^.(?=Z)*", "").expect("legacy assertion star should compile");
        assert!(greedy_optional.is_match("a").unwrap());
        assert!(greedy_optional.is_match("bZ").unwrap());

        let required =
            compile_regex(r"^.(?=Z)+", "").expect("legacy assertion plus should compile");
        assert!(!required.is_match("a").unwrap());
        assert!(required.is_match("bZ").unwrap());

        let negative =
            compile_regex(r"^[a-e](?!Z){2,3}", "").expect("legacy assertion range should compile");
        assert!(negative.is_match("a").unwrap());
        assert!(!negative.is_match("bZ").unwrap());
    }

    #[test]
    fn ecma_binary_properties_use_current_icu_data() {
        let alphabetic =
            compile_regex(r"^\p{Alphabetic}+$", "u").expect("Alphabetic should compile");
        assert!(alphabetic.is_match("AgentJS").unwrap());
        assert!(alphabetic.is_match("\u{1E5D0}").unwrap());
        assert!(!alphabetic.is_match("123").unwrap());

        let non_alphabetic =
            compile_regex(r"^\P{Alphabetic}+$", "u").expect("complement should compile");
        assert!(non_alphabetic.is_match("123").unwrap());
        assert!(!non_alphabetic.is_match("AgentJS").unwrap());
    }
}
