//! Backend-neutral Unicode code-point and finite-string set representation.
//!
//! ECMAScript `v`-mode character classes are not ordinary regex character
//! classes: their elements may be code points or strings and they support set
//! union, intersection, and subtraction.  This module owns that representation
//! and its small parser so the lexer and RegExp backend share one grammar
//! without introducing a dependency between those stages.

use std::{collections::BTreeSet, ops::RangeInclusive};

const UTF16_ESCAPE: char = '\u{E000}';
const UTF16_SURROGATE_BASE: u32 = 0xE001;

/// Lossless UTF-16 code-unit string carried through the native runtime's
/// scalar `String` storage.
///
/// Well-formed scalar substrings remain readable. Ill-formed surrogate code
/// units use an escaped two-scalar representation, and a literal escape scalar
/// is doubled, making conversion through this type reversible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Utf16String {
    storage: String,
}

impl Utf16String {
    pub(crate) fn from_units(units: &[u16]) -> Self {
        let mut storage = String::new();
        for decoded in char::decode_utf16(units.iter().copied()) {
            match decoded {
                Ok(ch) => {
                    if ch == UTF16_ESCAPE {
                        storage.push(UTF16_ESCAPE);
                    }
                    storage.push(ch);
                }
                Err(error) => {
                    storage.push(UTF16_ESCAPE);
                    let encoded =
                        UTF16_SURROGATE_BASE + u32::from(error.unpaired_surrogate() - 0xD800);
                    storage.push(char::from_u32(encoded).expect("surrogate escape is scalar"));
                }
            }
        }
        Self { storage }
    }

    pub(crate) fn from_storage(storage: &str) -> Self {
        Self {
            storage: storage.to_owned(),
        }
    }

    pub(crate) fn into_storage(self) -> String {
        self.storage
    }

    pub(crate) fn units(&self) -> Vec<u16> {
        let chars: Vec<char> = self.storage.chars().collect();
        let mut output = Vec::new();
        let mut index = 0;
        while index < chars.len() {
            if chars[index] == UTF16_ESCAPE {
                if chars.get(index + 1) == Some(&UTF16_ESCAPE) {
                    output.push(UTF16_ESCAPE as u16);
                    index += 2;
                    continue;
                }
                if let Some(encoded) = chars.get(index + 1).map(|ch| *ch as u32)
                    && (UTF16_SURROGATE_BASE..UTF16_SURROGATE_BASE + 0x800).contains(&encoded)
                {
                    output.push(0xD800 + (encoded - UTF16_SURROGATE_BASE) as u16);
                    index += 2;
                    continue;
                }
            }
            let mut buffer = [0; 2];
            output.extend_from_slice(chars[index].encode_utf16(&mut buffer));
            index += 1;
        }
        output
    }

    pub(crate) fn escaped_surrogate(unit: u16) -> Option<String> {
        (0xD800..=0xDFFF)
            .contains(&unit)
            .then(|| Self::from_units(&[unit]).into_storage())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CodePointSet {
    ranges: Vec<(u32, u32)>,
}

impl CodePointSet {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn scalar_values() -> Self {
        Self {
            ranges: vec![(0, 0xD7FF), (0xE000, 0x10FFFF)],
        }
    }

    pub(crate) fn from_range(start: u32, end: u32) -> Self {
        let mut set = Self::empty();
        set.insert_range(start, end);
        set
    }

    pub(crate) fn from_ranges(ranges: impl Iterator<Item = RangeInclusive<u32>>) -> Self {
        let mut set = Self::empty();
        for range in ranges {
            set.insert_range(*range.start(), *range.end());
        }
        set
    }

    pub(crate) fn insert_range(&mut self, start: u32, end: u32) {
        if start > end {
            return;
        }
        for (scalar_start, scalar_end) in [
            (start, end.min(0xD7FF)),
            (start.max(0xE000), end.min(0x10FFFF)),
        ] {
            if scalar_start <= scalar_end {
                self.ranges.push((scalar_start, scalar_end));
            }
        }
        self.normalize();
    }

    pub(crate) fn union(&self, other: &Self) -> Self {
        let mut ranges = self.ranges.clone();
        ranges.extend_from_slice(&other.ranges);
        let mut result = Self { ranges };
        result.normalize();
        result
    }

    pub(crate) fn intersection(&self, other: &Self) -> Self {
        let mut output = Vec::new();
        let (mut left, mut right) = (0, 0);
        while left < self.ranges.len() && right < other.ranges.len() {
            let (left_start, left_end) = self.ranges[left];
            let (right_start, right_end) = other.ranges[right];
            let start = left_start.max(right_start);
            let end = left_end.min(right_end);
            if start <= end {
                output.push((start, end));
            }
            if left_end < right_end {
                left += 1;
            } else {
                right += 1;
            }
        }
        Self { ranges: output }
    }

    pub(crate) fn difference(&self, other: &Self) -> Self {
        let mut output = Vec::new();
        for &(start, end) in &self.ranges {
            let mut cursor = start;
            for &(remove_start, remove_end) in &other.ranges {
                if remove_end < cursor {
                    continue;
                }
                if remove_start > end {
                    break;
                }
                if cursor < remove_start {
                    output.push((cursor, remove_start - 1));
                }
                cursor = cursor.max(remove_end.saturating_add(1));
                if cursor > end {
                    break;
                }
            }
            if cursor <= end {
                output.push((cursor, end));
            }
        }
        Self { ranges: output }
    }

    pub(crate) fn complement(&self) -> Self {
        Self::scalar_values().difference(self)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub(crate) fn regex_class_contents(&self) -> String {
        let mut output = String::new();
        for &(start, end) in &self.ranges {
            if start == end {
                output.push_str(&format!(r"\u{{{start:X}}}"));
            } else {
                output.push_str(&format!(r"\u{{{start:X}}}-\u{{{end:X}}}"));
            }
        }
        output
    }

    fn normalize(&mut self) {
        self.ranges.sort_unstable();
        let mut merged: Vec<(u32, u32)> = Vec::with_capacity(self.ranges.len());
        for (start, end) in self.ranges.drain(..) {
            if let Some((_, previous_end)) = merged.last_mut()
                && start <= previous_end.saturating_add(1)
            {
                *previous_end = (*previous_end).max(end);
            } else {
                merged.push((start, end));
            }
        }
        self.ranges = merged;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct UnicodeSet {
    pub(crate) code_points: CodePointSet,
    pub(crate) strings: BTreeSet<String>,
}

impl UnicodeSet {
    pub(crate) fn from_code_points(code_points: CodePointSet) -> Self {
        Self {
            code_points,
            strings: BTreeSet::new(),
        }
    }

    pub(crate) fn from_string(value: String) -> Self {
        if value.chars().count() == 1 {
            return Self::from_code_points(CodePointSet::from_range(
                value.chars().next().unwrap_or('\0') as u32,
                value.chars().next().unwrap_or('\0') as u32,
            ));
        }
        Self {
            code_points: CodePointSet::empty(),
            strings: BTreeSet::from([value]),
        }
    }

    fn union(&self, other: &Self) -> Self {
        Self {
            code_points: self.code_points.union(&other.code_points),
            strings: self.strings.union(&other.strings).cloned().collect(),
        }
    }

    fn intersection(&self, other: &Self) -> Self {
        Self {
            code_points: self.code_points.intersection(&other.code_points),
            strings: self.strings.intersection(&other.strings).cloned().collect(),
        }
    }

    fn difference(&self, other: &Self) -> Self {
        Self {
            code_points: self.code_points.difference(&other.code_points),
            strings: self.strings.difference(&other.strings).cloned().collect(),
        }
    }

    pub(crate) fn to_regex(&self) -> String {
        let mut alternatives: Vec<String> = self
            .strings
            .iter()
            .map(|value| regex::escape(value))
            .collect();
        alternatives.sort_by(|left, right| {
            right
                .chars()
                .count()
                .cmp(&left.chars().count())
                .then_with(|| left.cmp(right))
        });
        if !self.code_points.is_empty() {
            alternatives.push(format!("[{}]", self.code_points.regex_class_contents()));
        }
        match alternatives.len() {
            0 => "(?:(?!)a)".to_owned(),
            1 => alternatives.pop().unwrap_or_default(),
            _ => format!("(?:{})", alternatives.join("|")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetOperator {
    Union,
    Intersection,
    Difference,
}

struct SetParser<'a, F> {
    chars: Vec<char>,
    index: usize,
    resolve_property: &'a F,
}

impl<'a, F> SetParser<'a, F>
where
    F: Fn(&str) -> Option<UnicodeSet>,
{
    fn new(source: &str, resolve_property: &'a F) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            resolve_property,
        }
    }

    fn parse_class(&mut self) -> Result<UnicodeSet, String> {
        self.expect('[')?;
        let negated = self.consume('^');
        let mut value: Option<UnicodeSet> = None;
        let mut operator = SetOperator::Union;
        while self.peek() != Some(']') {
            if self.at_end() {
                return Err("unterminated Unicode set character class".into());
            }
            if self.consume_pair('&', '&') {
                operator = SetOperator::Intersection;
                continue;
            }
            if self.consume_pair('-', '-') {
                operator = SetOperator::Difference;
                continue;
            }
            let operand = self.parse_operand()?;
            value = Some(match (value, operator) {
                (None, _) => operand,
                (Some(left), SetOperator::Union) => left.union(&operand),
                (Some(left), SetOperator::Intersection) => left.intersection(&operand),
                (Some(left), SetOperator::Difference) => left.difference(&operand),
            });
            operator = SetOperator::Union;
        }
        self.expect(']')?;
        let mut value = value.unwrap_or_default();
        if negated {
            if !value.strings.is_empty() {
                return Err("negated Unicode set cannot contain strings".into());
            }
            value.code_points = value.code_points.complement();
        }
        Ok(value)
    }

    fn parse_operand(&mut self) -> Result<UnicodeSet, String> {
        if self.peek() == Some('[') {
            return self.parse_class();
        }
        if self.peek() == Some('\\') {
            return self.parse_escape();
        }
        let start = self.next().ok_or("missing Unicode set operand")?;
        if self.peek() == Some('-') && self.peek_n(1) != Some('-') {
            self.index += 1;
            let end = self.parse_character()?;
            if start as u32 > end as u32 {
                return Err("Unicode set range is out of order".into());
            }
            return Ok(UnicodeSet::from_code_points(CodePointSet::from_range(
                start as u32,
                end as u32,
            )));
        }
        Ok(UnicodeSet::from_string(start.to_string()))
    }

    fn parse_character(&mut self) -> Result<char, String> {
        if self.peek() != Some('\\') {
            return self.next().ok_or_else(|| "missing range endpoint".into());
        }
        self.index += 1;
        self.parse_character_escape()
    }

    fn parse_escape(&mut self) -> Result<UnicodeSet, String> {
        self.expect('\\')?;
        let escape = self.next().ok_or("unterminated Unicode set escape")?;
        match escape {
            'd' => Ok(UnicodeSet::from_code_points(CodePointSet::from_range(
                '0' as u32, '9' as u32,
            ))),
            'D' => Ok(UnicodeSet::from_code_points(
                CodePointSet::from_range('0' as u32, '9' as u32).complement(),
            )),
            'w' => {
                let letters = CodePointSet::from_range('A' as u32, 'Z' as u32)
                    .union(&CodePointSet::from_range('a' as u32, 'z' as u32))
                    .union(&CodePointSet::from_range('0' as u32, '9' as u32))
                    .union(&CodePointSet::from_range('_' as u32, '_' as u32));
                Ok(UnicodeSet::from_code_points(letters))
            }
            'W' => {
                let mut parser = Self {
                    chars: vec!['\\', 'w'],
                    index: 0,
                    resolve_property: self.resolve_property,
                };
                let word = parser.parse_escape()?;
                Ok(UnicodeSet::from_code_points(word.code_points.complement()))
            }
            's' | 'S' => {
                let whitespace = (self.resolve_property)("White_Space")
                    .ok_or("Unicode White_Space property is unavailable")?;
                if escape == 's' {
                    Ok(whitespace)
                } else {
                    Ok(UnicodeSet::from_code_points(
                        whitespace.code_points.complement(),
                    ))
                }
            }
            'p' | 'P' => {
                self.expect('{')?;
                let start = self.index;
                while self.peek().is_some_and(|ch| ch != '}') {
                    self.index += 1;
                }
                let property: String = self.chars[start..self.index].iter().collect();
                self.expect('}')?;
                let mut set = (self.resolve_property)(&property)
                    .ok_or_else(|| format!("unsupported Unicode property `{property}`"))?;
                if escape == 'P' {
                    if !set.strings.is_empty() {
                        return Err("a property of strings cannot be complemented".into());
                    }
                    set.code_points = set.code_points.complement();
                }
                Ok(set)
            }
            'q' => {
                self.expect('{')?;
                let mut result = UnicodeSet::default();
                let mut current = String::new();
                while let Some(ch) = self.next() {
                    match ch {
                        '}' => {
                            result = result.union(&UnicodeSet::from_string(current));
                            return Ok(result);
                        }
                        '|' => {
                            result = result.union(&UnicodeSet::from_string(current));
                            current = String::new();
                        }
                        '\\' => current.push(self.parse_character_escape()?),
                        _ => current.push(ch),
                    }
                }
                Err("unterminated Unicode string set".into())
            }
            _ => Ok(UnicodeSet::from_string(
                self.decode_simple_escape(escape)?.to_string(),
            )),
        }
    }

    fn parse_character_escape(&mut self) -> Result<char, String> {
        let escape = self.next().ok_or("unterminated character escape")?;
        if escape == 'u' {
            if self.consume('{') {
                let start = self.index;
                while self.peek().is_some_and(|ch| ch != '}') {
                    self.index += 1;
                }
                let digits: String = self.chars[start..self.index].iter().collect();
                self.expect('}')?;
                let value = u32::from_str_radix(&digits, 16)
                    .map_err(|_| "invalid Unicode code point escape")?;
                return char::from_u32(value).ok_or_else(|| "invalid Unicode scalar value".into());
            }
            let digits: String = (0..4)
                .map(|_| self.next().ok_or("short Unicode escape"))
                .collect::<Result<_, _>>()?;
            let value = u32::from_str_radix(&digits, 16).map_err(|_| "invalid Unicode escape")?;
            return char::from_u32(value).ok_or_else(|| "invalid Unicode scalar value".into());
        }
        self.decode_simple_escape(escape)
    }

    fn decode_simple_escape(&self, escape: char) -> Result<char, String> {
        Ok(match escape {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            'f' => '\u{000C}',
            'v' => '\u{000B}',
            'b' => '\u{0008}',
            other => other,
        })
    }

    fn at_end(&self) -> bool {
        self.index >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_n(&self, offset: usize) -> Option<char> {
        self.chars.get(self.index + offset).copied()
    }

    fn next(&mut self) -> Option<char> {
        let value = self.peek()?;
        self.index += 1;
        Some(value)
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn consume_pair(&mut self, first: char, second: char) -> bool {
        if self.peek() == Some(first) && self.peek_n(1) == Some(second) {
            self.index += 2;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: char) -> Result<(), String> {
        if self.consume(expected) {
            Ok(())
        } else {
            Err(format!("expected `{expected}` in Unicode set"))
        }
    }
}

pub(crate) fn parse_unicode_set<F>(source: &str, resolve_property: &F) -> Result<UnicodeSet, String>
where
    F: Fn(&str) -> Option<UnicodeSet>,
{
    let mut parser = SetParser::new(source, resolve_property);
    let set = parser.parse_class()?;
    if !parser.at_end() {
        return Err("trailing input in Unicode set".into());
    }
    Ok(set)
}

/// Replace every top-level `v`-mode class with a backend-compatible atom.
pub(crate) fn translate_unicode_sets<F>(
    pattern: &str,
    resolve_property: &F,
) -> Result<String, String>
where
    F: Fn(&str) -> Option<UnicodeSet>,
{
    let chars: Vec<char> = pattern.chars().collect();
    let mut output = String::with_capacity(pattern.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '\\' {
            if matches!(chars.get(index + 1), Some('p') | Some('P'))
                && chars.get(index + 2) == Some(&'{')
            {
                let negated = chars[index + 1] == 'P';
                let mut end = index + 3;
                while chars.get(end).is_some_and(|ch| *ch != '}') {
                    end += 1;
                }
                if chars.get(end) != Some(&'}') {
                    return Err("unterminated Unicode property escape".into());
                }
                let property: String = chars[index + 3..end].iter().collect();
                if let Some(set) = resolve_property(&property)
                    && !set.strings.is_empty()
                {
                    if negated {
                        return Err("a property of strings cannot be complemented".into());
                    }
                    output.push_str(&set.to_regex());
                    index = end + 1;
                    continue;
                }
            }
            output.push(chars[index]);
            index += 1;
            if let Some(ch) = chars.get(index) {
                output.push(*ch);
                index += 1;
            }
            continue;
        }
        if chars[index] != '[' {
            output.push(chars[index]);
            index += 1;
            continue;
        }
        let start = index;
        let mut depth = 0usize;
        while index < chars.len() {
            match chars[index] {
                '\\' => index += 2,
                '[' => {
                    depth += 1;
                    index += 1;
                }
                ']' => {
                    depth = depth.saturating_sub(1);
                    index += 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => index += 1,
            }
        }
        if depth != 0 {
            return Err("unterminated Unicode set character class".into());
        }
        let source: String = chars[start..index].iter().collect();
        output.push_str(&parse_unicode_set(&source, resolve_property)?.to_regex());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_properties(_: &str) -> Option<UnicodeSet> {
        None
    }

    #[test]
    fn set_operations_keep_code_points_and_strings_separate() {
        let set = parse_unicode_set(r"[[0-9]--[3-7]\q{ok|no}]", &no_properties).unwrap();
        assert_eq!(
            set.code_points.regex_class_contents(),
            r"\u{30}-\u{32}\u{38}-\u{39}"
        );
        assert_eq!(
            set.strings.into_iter().collect::<Vec<_>>(),
            vec!["no", "ok"]
        );
    }

    #[test]
    fn translates_nested_classes_to_one_regex_atom() {
        let translated = translate_unicode_sets(r"^[[0-9]&&[5-9]]+$", &no_properties).unwrap();
        assert_eq!(translated, r"^[\u{35}-\u{39}]+$");
    }

    #[test]
    fn utf16_storage_round_trips_lone_surrogates_and_escape_scalars() {
        let units = [0xD800, 0x0041, 0xE000, 0xDFFF];
        let value = Utf16String::from_units(&units);
        assert_eq!(value.units(), units);
    }
}
