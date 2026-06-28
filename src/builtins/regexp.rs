//! Pure RegExp algorithm helpers (no VM/runtime wiring).
//!
//! The thin adapter layer in `v6.rs` bridges these into the runtime.

use regex::{Regex, RegexBuilder};

const MAX_REPLACEMENT_OUTPUT_BYTES: usize = 1 << 23;

type ReplacementResult<T> = Result<T, &'static str>;

/// Compile a JS regex pattern + flags string into a Rust [`Regex`].
/// Returns `Err(message)` if the pattern or flags are invalid.
pub fn compile_regex(pattern: &str, flags: &str) -> Result<Regex, String> {
    let translated_pattern = translate_js_pattern_for_rust(pattern, flags);
    let mut builder = RegexBuilder::new(&translated_pattern);
    for flag in flags.chars() {
        match flag {
            'i' => {
                builder.case_insensitive(true);
            }
            'm' => {
                builder.multi_line(true);
            }
            's' => {
                builder.dot_matches_new_line(true);
            }
            'u' | 'v' | 'g' | 'y' | 'd' => {}
            other => return Err(format!("invalid flag `{other}`")),
        }
    }
    builder.build().map_err(|e| e.to_string())
}

fn translate_js_pattern_for_rust(pattern: &str, flags: &str) -> String {
    let unicode_mode = flags.contains('u') || flags.contains('v');
    let dot_replacement = if flags.contains('s') && unicode_mode {
        None
    } else if flags.contains('s') {
        Some(r"[^\u{10000}-\u{10FFFF}]")
    } else if unicode_mode {
        Some(r"[^\n\r\u{2028}\u{2029}]")
    } else {
        Some(r"[^\n\r\u{2028}\u{2029}\u{10000}-\u{10FFFF}]")
    };
    let mut output = String::with_capacity(pattern.len());
    let mut chars = pattern.chars().peekable();
    let mut in_class = false;
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                if next == '0' && !chars.peek().is_some_and(char::is_ascii_digit) {
                    output.push_str(r"\x00");
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
            in_class = true;
            output.push(ch);
            continue;
        }
        if ch == ']' {
            in_class = false;
            output.push(ch);
            continue;
        }
        if ch == '.' && !in_class {
            if let Some(replacement) = dot_replacement {
                output.push_str(replacement);
            } else {
                output.push(ch);
            }
        } else {
            output.push(ch);
        }
    }
    output
}

/// Returns `true` if the flags string contains the global flag `g`.
pub fn is_global(flags: &str) -> bool {
    flags.contains('g')
}

/// Returns the index (in UTF-16 code units) of the first match of `regex` in
/// `text`, or `None` if there is no match. Used by `String.prototype.search`.
pub fn search(regex: &Regex, text: &str) -> Option<usize> {
    regex
        .find(text)
        .map(|m| text[..m.start()].encode_utf16().count())
}

/// Returns the captures for the first match of `regex` in `text`.
/// The returned vector has the full match at index 0 followed by capture groups
/// (as `Option<String>` where `None` represents an unmatched optional group).
pub fn exec_once(regex: &Regex, text: &str) -> Option<Vec<Option<String>>> {
    regex.captures(text).map(|caps| {
        (0..caps.len())
            .map(|i| caps.get(i).map(|m| m.as_str().to_owned()))
            .collect()
    })
}

/// Returns all non-overlapping full-match strings (global match).
pub fn exec_global(regex: &Regex, text: &str) -> Vec<String> {
    regex
        .find_iter(text)
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
    let Some(caps) = regex.captures(text) else {
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
    let mut result = String::new();
    let mut last_end = 0;
    for caps in regex.captures_iter(text) {
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

/// Detailed match info for a single match, used by function-replacement callers.
pub struct MatchDetail {
    pub full_match: String,
    /// Capture groups: index 1…n (index 0 is the full match, kept for symmetry).
    pub captures: Vec<Option<String>>,
    /// UTF-16 start index of the match in the original string.
    pub index: usize,
}

/// Iterates all (non-overlapping) matches of `regex` in `text`, returning
/// [`MatchDetail`] entries. Used by builtin replace with function callback.
pub fn matches_with_detail(regex: &Regex, text: &str, global: bool) -> Vec<MatchDetail> {
    let mut out = Vec::new();
    for caps in regex.captures_iter(text) {
        let m = caps.get(0).unwrap();
        let index = text[..m.start()].encode_utf16().count();
        let full_match = m.as_str().to_owned();
        let captures = (0..caps.len())
            .map(|i| caps.get(i).map(|c| c.as_str().to_owned()))
            .collect();
        out.push(MatchDetail {
            full_match,
            captures,
            index,
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

    for caps in regex.captures_iter(text) {
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

        assert!(regex.is_match("\0"));
        assert!(regex.is_match("\t"));
        assert!(!regex.is_match("x"));
    }
}
