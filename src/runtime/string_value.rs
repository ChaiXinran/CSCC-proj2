//! Shared runtime string storage.

use std::{borrow::Borrow, fmt, ops::Deref, sync::Arc};

#[derive(Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct JsString(Arc<str>);

impl JsString {
    #[must_use]
    pub fn new(value: impl AsRef<str>) -> Self {
        Self(Arc::from(value.as_ref()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn into_owned(self) -> String {
        self.0.as_ref().to_owned()
    }

    #[must_use]
    pub fn ptr_eq(left: &Self, right: &Self) -> bool {
        Arc::ptr_eq(&left.0, &right.0)
    }
}

impl Deref for JsString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for JsString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for JsString {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for JsString {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl From<String> for JsString {
    fn from(value: String) -> Self {
        Self(Arc::from(value))
    }
}

impl From<Arc<str>> for JsString {
    fn from(value: Arc<str>) -> Self {
        Self(value)
    }
}

impl fmt::Display for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for JsString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl PartialEq<str> for JsString {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for JsString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::hash_map::DefaultHasher, hash::Hash, hash::Hasher, sync::Arc};

    use super::JsString;

    #[test]
    fn clone_shares_the_backing_buffer() {
        let value = JsString::from("shared runtime string");
        let clone = value.clone();

        assert!(JsString::ptr_eq(&value, &clone));
        assert_eq!(value, clone);
    }

    #[test]
    fn conversions_preserve_unicode_content_and_hash() {
        let source: Arc<str> = Arc::from("\u{5b57}\u{7b26}\u{4e32}\u{1f600}");
        let shared = JsString::from(source.clone());
        let equal = JsString::new(source.as_ref());
        let mut left = DefaultHasher::new();
        let mut right = DefaultHasher::new();
        shared.hash(&mut left);
        equal.hash(&mut right);

        assert_eq!(left.finish(), right.finish());
        assert_eq!(shared.into_owned(), source.as_ref());
    }
}
