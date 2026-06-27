//! UTF-8 source cursor used by the lexer.

/// A small, byte-offset-aware cursor over UTF-8 source text.
#[derive(Debug, Clone)]
pub struct Cursor<'source> {
    source: &'source str,
    offset: usize,
}

impl<'source> Cursor<'source> {
    #[must_use]
    pub const fn new(source: &'source str) -> Self {
        Self { source, offset: 0 }
    }

    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    #[must_use]
    pub fn peek(&self) -> Option<char> {
        self.source[self.offset..].chars().next()
    }

    /// Returns the character one position past [`Cursor::peek`] without moving.
    #[must_use]
    pub fn second(&self) -> Option<char> {
        let mut chars = self.source[self.offset..].chars();
        chars.next();
        chars.next()
    }

    /// Returns the unconsumed source text. Useful for fixed-string matching such
    /// as comment introducers and multi-character operators.
    #[must_use]
    pub fn rest(&self) -> &'source str {
        &self.source[self.offset..]
    }

    pub fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }

    pub fn skip_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while self.peek().is_some_and(&mut predicate) {
            self.bump();
        }
    }

    /// Returns the full source text (from byte 0), regardless of the current cursor position.
    #[must_use]
    pub fn source(&self) -> &'source str {
        self.source
    }

    /// Advances the cursor until the byte offset reaches `target`.
    /// No-ops if the cursor is already at or past `target`.
    pub fn advance_to(&mut self, target: usize) {
        while self.offset < target {
            if self.bump().is_none() {
                break;
            }
        }
    }

    #[must_use]
    pub fn slice(&self, span: super::Span) -> &'source str {
        &self.source[span.start..span.end]
    }
}

#[cfg(test)]
mod tests {
    use super::Cursor;
    use crate::lexer::Span;

    #[test]
    fn tracks_utf8_byte_offsets() {
        let mut cursor = Cursor::new("a中");
        assert_eq!(cursor.bump(), Some('a'));
        assert_eq!(cursor.offset(), 1);
        assert_eq!(cursor.bump(), Some('中'));
        assert_eq!(cursor.offset(), 4);
        assert_eq!(cursor.slice(Span::new(1, 4)), "中");
    }
}
