//! Pure RegExp algorithm helpers (no VM/runtime wiring).
//!
//! The thin adapter layer in `v6.rs` bridges these into the runtime.

use regex::{Regex, RegexBuilder};

/// Compile a JS regex pattern + flags string into a Rust [`Regex`].
/// Returns `Err(message)` if the pattern or flags are invalid.
pub fn compile_regex(pattern: &str, flags: &str) -> Result<Regex, String> {
    let mut builder = RegexBuilder::new(pattern);
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
            'u' | 'g' | 'y' | 'd' => {}
            other => return Err(format!("invalid flag `{other}`")),
        }
    }
    builder.build().map_err(|e| e.to_string())
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

/// Replaces the first match of `regex` in `text` with `replacement`.
/// `$1`, `$2`, … back-references are not expanded (basic implementation).
pub fn replace_first(regex: &Regex, text: &str, replacement: &str) -> String {
    regex.replacen(text, 1, replacement).into_owned()
}

/// Replaces all matches of `regex` in `text` with `replacement`.
pub fn replace_all(regex: &Regex, text: &str, replacement: &str) -> String {
    regex.replace_all(text, replacement).into_owned()
}

/// Splits `text` by every match of `regex`.
pub fn split(regex: &Regex, text: &str, limit: Option<usize>) -> Vec<String> {
    let parts: Vec<String> = regex.split(text).map(str::to_owned).collect();
    if let Some(limit) = limit {
        parts.into_iter().take(limit).collect()
    } else {
        parts
    }
}
