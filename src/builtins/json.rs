//! Core JSON parser and stringifier for the native V6 runtime.

use std::collections::HashSet;

use crate::{
    runtime::{JsValue, NativeContext, ObjectId, ObjectKind, PropertyKind},
    vm::VmError,
};

pub(crate) fn parse_json(source: &str, context: &mut NativeContext) -> Result<JsValue, VmError> {
    let mut parser = Parser {
        source,
        offset: 0,
        context,
    };
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.offset != source.len() {
        return Err(parser.error("unexpected trailing input"));
    }
    Ok(value)
}

pub(crate) fn stringify_json(
    value: JsValue,
    context: &NativeContext,
) -> Result<Option<String>, VmError> {
    let mut stack = HashSet::new();
    stringify_value(&value, context, &mut stack, false)
}

struct Parser<'a> {
    source: &'a str,
    offset: usize,
    context: &'a mut NativeContext,
}

impl Parser<'_> {
    fn parse_value(&mut self) -> Result<JsValue, VmError> {
        self.skip_whitespace();
        match self.peek_byte() {
            Some(b'n') => {
                self.expect_keyword("null")?;
                Ok(JsValue::Null)
            }
            Some(b't') => {
                self.expect_keyword("true")?;
                Ok(JsValue::Boolean(true))
            }
            Some(b'f') => {
                self.expect_keyword("false")?;
                Ok(JsValue::Boolean(false))
            }
            Some(b'"') => self.parse_string().map(JsValue::String),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(JsValue::Number),
            _ => Err(self.error("expected JSON value")),
        }
    }

    fn parse_array(&mut self) -> Result<JsValue, VmError> {
        self.consume_byte(b'[')?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.take_byte(b']') {
            return self.context.create_array(values);
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.take_byte(b']') {
                break;
            }
            self.consume_byte(b',')?;
            self.skip_whitespace();
        }
        self.context.create_array(values)
    }

    fn parse_object(&mut self) -> Result<JsValue, VmError> {
        self.consume_byte(b'{')?;
        self.skip_whitespace();
        let mut properties = Vec::new();
        if self.take_byte(b'}') {
            return self.context.create_object(properties);
        }
        loop {
            if self.peek_byte() != Some(b'"') {
                return Err(self.error("object key must be a string"));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            self.consume_byte(b':')?;
            let value = self.parse_value()?;
            properties.push((key, value));
            self.skip_whitespace();
            if self.take_byte(b'}') {
                break;
            }
            self.consume_byte(b',')?;
            self.skip_whitespace();
        }
        self.context.create_object(properties)
    }

    fn parse_string(&mut self) -> Result<String, VmError> {
        self.consume_byte(b'"')?;
        let mut result = String::new();
        loop {
            let Some(byte) = self.peek_byte() else {
                return Err(self.error("unterminated JSON string"));
            };
            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(result);
                }
                b'\\' => {
                    self.offset += 1;
                    self.parse_escape(&mut result)?;
                }
                0x00..=0x1F => return Err(self.error("control character in JSON string")),
                0x20..=0x7F => {
                    result.push(char::from(byte));
                    self.offset += 1;
                }
                _ => {
                    let remaining = &self.source[self.offset..];
                    let character = remaining
                        .chars()
                        .next()
                        .ok_or_else(|| self.error("invalid UTF-8 in JSON string"))?;
                    result.push(character);
                    self.offset += character.len_utf8();
                }
            }
        }
    }

    fn parse_escape(&mut self, output: &mut String) -> Result<(), VmError> {
        let Some(escape) = self.peek_byte() else {
            return Err(self.error("unterminated JSON escape"));
        };
        self.offset += 1;
        match escape {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{0008}'),
            b'f' => output.push('\u{000C}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
            b'u' => {
                let first = self.parse_hex_quad()?;
                if (0xD800..=0xDBFF).contains(&first)
                    && self.source.as_bytes().get(self.offset..self.offset + 2) == Some(b"\\u")
                {
                    self.offset += 2;
                    let second = self.parse_hex_quad()?;
                    if (0xDC00..=0xDFFF).contains(&second) {
                        let scalar =
                            0x10000 + ((u32::from(first) - 0xD800) << 10) + u32::from(second)
                                - 0xDC00;
                        output.push(char::from_u32(scalar).unwrap_or('\u{FFFD}'));
                    } else {
                        output.push('\u{FFFD}');
                        output.push(char::from_u32(u32::from(second)).unwrap_or('\u{FFFD}'));
                    }
                } else {
                    output.push(char::from_u32(u32::from(first)).unwrap_or('\u{FFFD}'));
                }
            }
            _ => return Err(self.error("invalid JSON escape")),
        }
        Ok(())
    }

    fn parse_hex_quad(&mut self) -> Result<u16, VmError> {
        let end = self.offset.saturating_add(4);
        let Some(text) = self.source.get(self.offset..end) else {
            return Err(self.error("incomplete Unicode escape"));
        };
        if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(self.error("invalid Unicode escape"));
        }
        self.offset = end;
        u16::from_str_radix(text, 16).map_err(|_| self.error("invalid Unicode escape"))
    }

    fn parse_number(&mut self) -> Result<f64, VmError> {
        let start = self.offset;
        self.take_byte(b'-');
        match self.peek_byte() {
            Some(b'0') => {
                self.offset += 1;
                if self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                    return Err(self.error("leading zero in JSON number"));
                }
            }
            Some(b'1'..=b'9') => {
                while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.offset += 1;
                }
            }
            _ => return Err(self.error("invalid JSON number")),
        }
        if self.take_byte(b'.') {
            let digits = self.offset;
            while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                self.offset += 1;
            }
            if self.offset == digits {
                return Err(self.error("missing fractional digits"));
            }
        }
        if matches!(self.peek_byte(), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.peek_byte(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            let digits = self.offset;
            while self.peek_byte().is_some_and(|byte| byte.is_ascii_digit()) {
                self.offset += 1;
            }
            if self.offset == digits {
                return Err(self.error("missing exponent digits"));
            }
        }
        self.source[start..self.offset]
            .parse::<f64>()
            .map_err(|_| self.error("invalid JSON number"))
    }

    fn expect_keyword(&mut self, keyword: &str) -> Result<(), VmError> {
        if self.source[self.offset..].starts_with(keyword) {
            self.offset += keyword.len();
            Ok(())
        } else {
            Err(self.error("invalid JSON keyword"))
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek_byte(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.offset += 1;
        }
    }

    fn consume_byte(&mut self, expected: u8) -> Result<(), VmError> {
        if self.take_byte(expected) {
            Ok(())
        } else {
            Err(self.error(&format!("expected `{}`", char::from(expected))))
        }
    }

    fn take_byte(&mut self, expected: u8) -> bool {
        if self.peek_byte() == Some(expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        self.source.as_bytes().get(self.offset).copied()
    }

    fn error(&self, message: &str) -> VmError {
        VmError::type_error(format!("JSON.parse: {message} at byte {}", self.offset))
    }
}

fn stringify_value(
    value: &JsValue,
    context: &NativeContext,
    stack: &mut HashSet<ObjectId>,
    in_array: bool,
) -> Result<Option<String>, VmError> {
    match value {
        JsValue::Undefined | JsValue::Function(_) | JsValue::BuiltinFunction(_) => {
            Ok(in_array.then(|| "null".into()))
        }
        JsValue::Null => Ok(Some("null".into())),
        JsValue::Boolean(value) => Ok(Some(value.to_string())),
        JsValue::Number(value) => Ok(Some(if value.is_finite() {
            number_to_json(*value)
        } else {
            "null".into()
        })),
        JsValue::String(value) => Ok(Some(quote_json_string(value))),
        JsValue::Error(_) => Ok(Some("{}".into())),
        JsValue::Object(object) => {
            if let Some(raw_json) = context.raw_json_value(*object) {
                Ok(Some(raw_json.to_string()))
            } else {
                stringify_object(*object, context, stack)
            }
        }
    }
}

fn stringify_object(
    object: ObjectId,
    context: &NativeContext,
    stack: &mut HashSet<ObjectId>,
) -> Result<Option<String>, VmError> {
    if !stack.insert(object) {
        return Err(VmError::type_error("JSON.stringify: cyclic object value"));
    }
    let object_value = context
        .heap()
        .object(object)
        .ok_or_else(|| VmError::runtime("missing object"))?;
    let result = match &object_value.kind {
        ObjectKind::Array { elements, .. } => {
            let mut parts = Vec::with_capacity(elements.len());
            for element in elements {
                let value = element
                    .as_ref()
                    .and_then(|descriptor| descriptor.value())
                    .unwrap_or(&JsValue::Undefined);
                parts.push(
                    stringify_value(value, context, stack, true)?.unwrap_or_else(|| "null".into()),
                );
            }
            format!("[{}]", parts.join(","))
        }
        ObjectKind::PrimitiveWrapper(value) => match value {
            crate::runtime::PrimitiveValue::Boolean(value) => value.to_string(),
            crate::runtime::PrimitiveValue::Number(value) => {
                if value.is_finite() {
                    number_to_json(*value)
                } else {
                    "null".into()
                }
            }
            crate::runtime::PrimitiveValue::String(value) => quote_json_string(value),
        },
        ObjectKind::Ordinary => {
            let mut parts = Vec::new();
            for key in object_value.own_property_keys() {
                let Some(descriptor) = object_value.own_property(&key) else {
                    continue;
                };
                if !descriptor.enumerable {
                    continue;
                }
                let PropertyKind::Data { value, .. } = &descriptor.kind else {
                    continue;
                };
                if let Some(value) = stringify_value(value, context, stack, false)? {
                    parts.push(format!("{}:{value}", quote_json_string(&key)));
                }
            }
            format!("{{{}}}", parts.join(","))
        }
    };
    stack.remove(&object);
    Ok(Some(result))
}

fn quote_json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\u{0008}' => output.push_str("\\b"),
            '\u{000C}' => output.push_str("\\f"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\u{0000}'..='\u{001F}' => {
                output.push_str(&format!("\\u{:04x}", u32::from(character)));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn number_to_json(value: f64) -> String {
    if value == 0.0 {
        "0".into()
    } else {
        value.to_string()
    }
}
