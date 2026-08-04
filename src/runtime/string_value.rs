//! Shared runtime string handle used by phase-two hot paths.

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
        &self.0
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
        Self::new(value)
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
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Debug for JsString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(formatter)
    }
}

#[cfg(test)]
mod tests {
    use super::JsString;

    #[test]
    fn clone_shares_the_backing_buffer() {
        let original = JsString::from("shared property name");
        let clone = original.clone();
        assert!(JsString::ptr_eq(&original, &clone));
        assert_eq!(original, clone);
    }
}
