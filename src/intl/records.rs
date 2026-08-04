#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HourCycle {
    H11,
    H12,
    H23,
    H24,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeFieldStyle {
    Numeric,
    TwoDigit,
    Narrow,
    Short,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTimeStyle {
    Full,
    Long,
    Medium,
    Short,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZoneNameStyle {
    Short,
    Long,
    ShortOffset,
    LongOffset,
    ShortGeneric,
    LongGeneric,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeFormatRecord {
    pub locale: String,
    pub calendar: String,
    pub numbering_system: String,
    pub time_zone: String,
    pub hour_cycle: Option<HourCycle>,
    pub weekday: Option<DateTimeFieldStyle>,
    pub era: Option<DateTimeFieldStyle>,
    pub year: Option<DateTimeFieldStyle>,
    pub month: Option<DateTimeFieldStyle>,
    pub day: Option<DateTimeFieldStyle>,
    pub hour: Option<DateTimeFieldStyle>,
    pub minute: Option<DateTimeFieldStyle>,
    pub second: Option<DateTimeFieldStyle>,
    pub fractional_second_digits: Option<u8>,
    pub time_zone_name: Option<TimeZoneNameStyle>,
    pub date_style: Option<DateTimeStyle>,
    pub time_style: Option<DateTimeStyle>,
}

impl Default for DateTimeFormatRecord {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            calendar: "gregory".into(),
            numbering_system: "latn".into(),
            time_zone: "UTC".into(),
            hour_cycle: None,
            weekday: None,
            era: None,
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            second: None,
            fractional_second_digits: None,
            time_zone_name: None,
            date_style: None,
            time_style: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumberFormatRecord {
    pub locale: String,
    pub numbering_system: String,
    pub style: String,
    pub currency: Option<String>,
    pub unit: Option<String>,
    pub minimum_integer_digits: u8,
    pub minimum_fraction_digits: u8,
    pub maximum_fraction_digits: u8,
    pub use_grouping: String,
    pub notation: String,
    pub sign_display: String,
    pub currency_display: String,
    pub currency_sign: String,
    pub unit_display: String,
    pub compact_display: String,
    pub minimum_significant_digits: Option<u8>,
    pub maximum_significant_digits: Option<u8>,
    pub minimum_significant_digits_explicit: bool,
    pub rounding_increment: u16,
    pub rounding_mode: String,
    pub rounding_priority: String,
    pub trailing_zero_display: String,
}

impl Default for NumberFormatRecord {
    fn default() -> Self {
        Self {
            locale: "en-US".into(),
            numbering_system: "latn".into(),
            style: "decimal".into(),
            currency: None,
            unit: None,
            minimum_integer_digits: 1,
            minimum_fraction_digits: 0,
            maximum_fraction_digits: 3,
            use_grouping: "auto".into(),
            notation: "standard".into(),
            sign_display: "auto".into(),
            currency_display: "symbol".into(),
            currency_sign: "standard".into(),
            unit_display: "short".into(),
            compact_display: "short".into(),
            minimum_significant_digits: None,
            maximum_significant_digits: None,
            minimum_significant_digits_explicit: false,
            rounding_increment: 1,
            rounding_mode: "halfExpand".into(),
            rounding_priority: "auto".into(),
            trailing_zero_display: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CollatorRecord {
    pub locale: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LocaleRecord {
    pub locale: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluralRulesRecord {
    pub locale: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RelativeTimeFormatRecord {
    pub locale: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListFormatRecord {
    pub locale: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IntlObjectData {
    DateTimeFormat(DateTimeFormatRecord),
    NumberFormat(Box<NumberFormatRecord>),
    Collator(CollatorRecord),
    Locale(LocaleRecord),
    PluralRules(PluralRulesRecord),
    RelativeTimeFormat(RelativeTimeFormatRecord),
    ListFormat(ListFormatRecord),
}
