use super::canonicalize_language_tag;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntlService {
    Collator,
    DateTimeFormat,
    ListFormat,
    NumberFormat,
    PluralRules,
    RelativeTimeFormat,
}

pub trait IntlDataProvider {
    fn canonicalize_locale(&self, locale: &str) -> Result<String, String>;
    fn supports_locale(&self, service: IntlService, locale: &str) -> bool;
    fn available_calendars(&self) -> &[&'static str];
    fn available_numbering_systems(&self) -> &[&'static str];
    fn canonicalize_time_zone(&self, value: &str) -> Result<String, String>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MinimalIntlProvider;

const CALENDARS: &[&str] = &[
    "buddhist",
    "chinese",
    "coptic",
    "dangi",
    "ethioaa",
    "ethiopic",
    "gregory",
    "hebrew",
    "indian",
    "islamic",
    "islamic-civil",
    "islamic-rgsa",
    "islamic-tbla",
    "islamic-umalqura",
    "iso8601",
    "japanese",
    "persian",
    "roc",
];
const NUMBERING_SYSTEMS: &[&str] = &[
    "adlm", "ahom", "arab", "arabext", "bali", "beng", "bhks", "brah", "cakm", "cham", "deva",
    "diak", "fullwide", "gara", "gong", "gonm", "gujr", "gukh", "guru", "hanidec", "hmng", "hmnp",
    "java", "kali", "kawi", "khmr", "knda", "krai", "lana", "lanatham", "laoo", "latn", "lepc",
    "limb", "mathbold", "mathdbl", "mathmono", "mathsanb", "mathsans", "mlym", "modi", "mong",
    "mroo", "mtei", "mymr", "mymrepka", "mymrpao", "mymrshan", "mymrtlng", "nagm", "newa", "nkoo",
    "olck", "onao", "orya", "osma", "outlined", "rohg", "saur", "segment", "shrd", "sind", "sinh",
    "sora", "sund", "sunu", "takr", "talu", "tamldec", "telu", "thai", "tibt", "tirh", "tnsa",
    "tols", "vaii", "wara", "wcho",
];

impl IntlDataProvider for MinimalIntlProvider {
    fn canonicalize_locale(&self, locale: &str) -> Result<String, String> {
        canonicalize_language_tag(locale)
    }

    fn supports_locale(&self, _service: IntlService, locale: &str) -> bool {
        canonicalize_language_tag(locale).is_ok()
    }

    fn available_calendars(&self) -> &[&'static str] {
        CALENDARS
    }

    fn available_numbering_systems(&self) -> &[&'static str] {
        NUMBERING_SYSTEMS
    }

    fn canonicalize_time_zone(&self, value: &str) -> Result<String, String> {
        let value = value.trim();
        if value.eq_ignore_ascii_case("utc")
            || value.eq_ignore_ascii_case("etc/utc")
            || value.eq_ignore_ascii_case("etc/gmt")
        {
            return Ok("UTC".into());
        }
        if value.is_empty()
            || !value.is_ascii()
            || value.contains(char::is_whitespace)
            || !value.contains('/')
        {
            return Err("invalid time zone identifier".into());
        }
        Ok(value.into())
    }
}
