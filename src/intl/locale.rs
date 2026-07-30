use super::{IntlDataProvider, IntlService};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocaleOptions {
    pub locale_matcher: Option<String>,
    pub calendar: Option<String>,
    pub numbering_system: Option<String>,
    pub hour_cycle: Option<String>,
    pub collation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocale {
    pub locale: String,
    pub data_locale: String,
    pub calendar: Option<String>,
    pub numbering_system: Option<String>,
    pub hour_cycle: Option<String>,
    pub collation: Option<String>,
}

pub fn canonicalize_language_tag(input: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() || !input.is_ascii() || input.contains('_') {
        return Err("invalid language tag".into());
    }
    let mut result = Vec::new();
    let mut in_extension = false;
    for (index, part) in input.split('-').enumerate() {
        if part.is_empty()
            || part.len() > 8
            || !part.chars().all(|ch| ch.is_ascii_alphanumeric())
            || (index == 0
                && (!(matches!(part.len(), 2 | 3) || (5..=8).contains(&part.len()))
                    || !part.chars().all(|ch| ch.is_ascii_alphabetic())))
        {
            return Err("invalid language tag".into());
        }
        let canonical = if index == 0 {
            part.to_ascii_lowercase()
        } else if in_extension || part.len() == 1 {
            in_extension = true;
            part.to_ascii_lowercase()
        } else if part.len() == 4 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let mut chars = part.chars();
            let first = chars.next().expect("non-empty").to_ascii_uppercase();
            format!("{first}{}", chars.as_str().to_ascii_lowercase())
        } else if (part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic()))
            || (part.len() == 3 && part.chars().all(|ch| ch.is_ascii_digit()))
        {
            part.to_ascii_uppercase()
        } else {
            part.to_ascii_lowercase()
        };
        result.push(canonical);
    }
    Ok(result.join("-"))
}

pub fn unicode_extension_value(locale: &str, key: &str) -> Option<String> {
    let parts: Vec<_> = locale.split('-').collect();
    let mut index = parts
        .iter()
        .position(|part| part.eq_ignore_ascii_case("u"))?
        + 1;
    while index < parts.len() {
        if parts[index].len() != 2 {
            index += 1;
            continue;
        }
        let candidate = parts[index];
        index += 1;
        let start = index;
        while index < parts.len() && parts[index].len() > 2 {
            index += 1;
        }
        if candidate.eq_ignore_ascii_case(key) && start < index {
            return Some(parts[start..index].join("-").to_ascii_lowercase());
        }
    }
    None
}

pub fn resolve_locale(
    provider: &dyn IntlDataProvider,
    service: IntlService,
    requested: &[String],
    options: &LocaleOptions,
) -> Result<ResolvedLocale, String> {
    let canonical =
        provider.canonicalize_locale(requested.first().map(String::as_str).unwrap_or("en-US"))?;
    let data_locale = canonical
        .split("-u-")
        .next()
        .unwrap_or(&canonical)
        .to_string();
    let locale = if provider.supports_locale(service, &data_locale) {
        canonical
    } else {
        "en-US".into()
    };
    Ok(ResolvedLocale {
        calendar: options
            .calendar
            .clone()
            .or_else(|| unicode_extension_value(&locale, "ca")),
        numbering_system: options
            .numbering_system
            .clone()
            .or_else(|| unicode_extension_value(&locale, "nu")),
        hour_cycle: options
            .hour_cycle
            .clone()
            .or_else(|| unicode_extension_value(&locale, "hc")),
        collation: options
            .collation
            .clone()
            .or_else(|| unicode_extension_value(&locale, "co")),
        locale,
        data_locale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intl::MinimalIntlProvider;

    #[test]
    fn canonicalizes_base_tag_without_uppercasing_unicode_extension_keys() {
        assert_eq!(
            canonicalize_language_tag("zh-cn-u-ca-chinese").unwrap(),
            "zh-CN-u-ca-chinese"
        );
    }

    #[test]
    fn resolves_unicode_extensions_and_explicit_overrides() {
        let resolved = resolve_locale(
            &MinimalIntlProvider,
            IntlService::DateTimeFormat,
            &["zh-CN-u-ca-chinese-nu-hanidec".into()],
            &LocaleOptions {
                calendar: Some("gregory".into()),
                ..LocaleOptions::default()
            },
        )
        .unwrap();
        assert_eq!(resolved.calendar.as_deref(), Some("gregory"));
        assert_eq!(resolved.numbering_system.as_deref(), Some("hanidec"));
    }
}
