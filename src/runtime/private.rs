use super::JsValue;

/// Identity of one class private environment. Equal spellings in distinct
/// classes deliberately receive different brands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PrivateBrandId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct PrivateSlot {
    pub brand: PrivateBrandId,
    pub value: JsValue,
}
