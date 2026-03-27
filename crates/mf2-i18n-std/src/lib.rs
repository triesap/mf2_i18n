#![forbid(unsafe_code)]

use std::str::FromStr;

use chrono::{TimeZone, Utc};
use intl_pluralrules::{PluralCategory as IntlPluralCategory, PluralRuleType, PluralRules};
use mf2_i18n_core::{
    CoreError, CoreResult, FormatBackend, FormatterOption, LanguageTag, PluralCategory,
};
use num_format::{Grouping, Locale as NumberLocale};
use pure_rust_locales::Locale as DateLocale;
use thiserror::Error;
use unic_langid::LanguageIdentifier;

#[derive(Debug, Error)]
pub enum StdFormatError {
    #[error("invalid locale tag: {0}")]
    InvalidLocale(String),
    #[error("missing plural rules for locale {0}")]
    MissingPluralRules(String),
}

pub struct StdFormatBackend {
    plural_rules: PluralRules,
    number_locale: NumberLocale,
    date_locale: DateLocale,
}

impl StdFormatBackend {
    pub fn new(locale: &str) -> Result<Self, StdFormatError> {
        let candidates = locale_candidates(locale)?;
        Ok(Self {
            plural_rules: resolve_plural_rules(&candidates)?,
            number_locale: resolve_number_locale(&candidates),
            date_locale: resolve_date_locale(&candidates),
        })
    }

    fn format_decimal(&self, value: f64) -> String {
        if value.is_nan() {
            return self.number_locale.nan().to_string();
        }

        let sign = if value.is_sign_negative() {
            self.number_locale.minus_sign().to_string()
        } else {
            String::new()
        };

        if value.is_infinite() {
            return format!("{sign}{}", self.number_locale.infinity());
        }

        let raw = value.abs().to_string();
        if let Some(index) = raw.find(|ch| ch == 'e' || ch == 'E') {
            let mantissa = localize_decimal_string(&raw[..index], &self.number_locale);
            return format!("{sign}{mantissa}{}", &raw[index..]);
        }

        format!(
            "{sign}{}",
            localize_decimal_string(&raw, &self.number_locale)
        )
    }

    fn format_datetime_with(&self, value: i64, pattern: &str) -> CoreResult<String> {
        let datetime = parse_timestamp(value)?;
        Ok(datetime
            .format_localized(pattern, self.date_locale)
            .to_string())
    }
}

impl FormatBackend for StdFormatBackend {
    fn plural_category(&self, value: f64) -> CoreResult<PluralCategory> {
        let category = self
            .plural_rules
            .select(value)
            .map_err(|_| CoreError::InvalidInput("invalid plural input"))?;
        Ok(match category {
            IntlPluralCategory::ZERO => PluralCategory::Zero,
            IntlPluralCategory::ONE => PluralCategory::One,
            IntlPluralCategory::TWO => PluralCategory::Two,
            IntlPluralCategory::FEW => PluralCategory::Few,
            IntlPluralCategory::MANY => PluralCategory::Many,
            IntlPluralCategory::OTHER => PluralCategory::Other,
        })
    }

    fn format_number(&self, value: f64, _options: &[FormatterOption]) -> CoreResult<String> {
        Ok(self.format_decimal(value))
    }

    fn format_date(&self, value: i64, _options: &[FormatterOption]) -> CoreResult<String> {
        self.format_datetime_with(value, "%x")
    }

    fn format_time(&self, value: i64, _options: &[FormatterOption]) -> CoreResult<String> {
        self.format_datetime_with(value, "%X")
    }

    fn format_datetime(&self, value: i64, _options: &[FormatterOption]) -> CoreResult<String> {
        self.format_datetime_with(value, "%c")
    }

    fn format_unit(
        &self,
        value: f64,
        unit_id: u32,
        _options: &[FormatterOption],
    ) -> CoreResult<String> {
        Ok(format!("{} u{unit_id}", self.format_decimal(value)))
    }

    fn format_currency(
        &self,
        value: f64,
        code: [u8; 3],
        _options: &[FormatterOption],
    ) -> CoreResult<String> {
        let code =
            core::str::from_utf8(&code).map_err(|_| CoreError::InvalidInput("currency code"))?;
        Ok(format!("{code} {}", self.format_decimal(value)))
    }
}

fn locale_candidates(locale: &str) -> Result<Vec<String>, StdFormatError> {
    let tag =
        LanguageTag::parse(locale).map_err(|err| StdFormatError::InvalidLocale(err.to_string()))?;
    let mut candidates = Vec::new();
    push_unique(&mut candidates, tag.normalized().to_owned());

    if !tag.match_subtags().is_empty() {
        for len in (1..=tag.match_subtags().len()).rev() {
            push_unique(&mut candidates, tag.match_subtags()[..len].join("-"));
        }
    }

    push_unique(&mut candidates, String::from("en"));
    Ok(candidates)
}

fn resolve_plural_rules(candidates: &[String]) -> Result<PluralRules, StdFormatError> {
    for candidate in candidates {
        if let Ok(identifier) = candidate.parse::<LanguageIdentifier>() {
            if let Ok(rules) = PluralRules::create(identifier, PluralRuleType::CARDINAL) {
                return Ok(rules);
            }
        }
    }

    Err(StdFormatError::MissingPluralRules(
        candidates
            .first()
            .cloned()
            .unwrap_or_else(|| String::from("en")),
    ))
}

fn resolve_number_locale(candidates: &[String]) -> NumberLocale {
    for candidate in expanded_locale_names(candidates) {
        if let Ok(locale) = NumberLocale::from_str(&candidate) {
            return locale;
        }
    }
    NumberLocale::en
}

fn resolve_date_locale(candidates: &[String]) -> DateLocale {
    for candidate in expanded_locale_names(candidates) {
        if let Ok(locale) = DateLocale::from_str(&candidate) {
            return locale;
        }
    }
    DateLocale::POSIX
}

fn expanded_locale_names(candidates: &[String]) -> Vec<String> {
    let mut names = Vec::new();
    for candidate in candidates {
        push_unique(&mut names, candidate.clone());
        if candidate.contains('-') {
            push_unique(&mut names, candidate.replace('-', "_"));
        }
    }
    names
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn localize_decimal_string(raw: &str, locale: &NumberLocale) -> String {
    let decimal = locale.decimal();
    if let Some((integer, fraction)) = raw.split_once('.') {
        let fraction = fraction.trim_end_matches('0');
        if fraction.is_empty() {
            return group_digits(integer, locale);
        }
        return format!("{}{}{fraction}", group_digits(integer, locale), decimal);
    }
    group_digits(raw, locale)
}

fn group_digits(integer: &str, locale: &NumberLocale) -> String {
    let separator = locale.separator();
    match locale.grouping() {
        Grouping::Posix => integer.to_owned(),
        Grouping::Standard => join_groups(integer, separator, 3, None),
        Grouping::Indian => join_groups(integer, separator, 3, Some(2)),
    }
}

fn join_groups(
    integer: &str,
    separator: &str,
    last_group: usize,
    leading_group: Option<usize>,
) -> String {
    if integer.len() <= last_group {
        return integer.to_owned();
    }

    let split_at = integer.len() - last_group;
    let (leading, trailing) = integer.split_at(split_at);
    let mut groups = Vec::new();

    if let Some(width) = leading_group {
        let first_width = if leading.len() % width == 0 {
            width
        } else {
            leading.len() % width
        };
        let mut start = 0;
        while start < leading.len() {
            let width = if start == 0 { first_width } else { width };
            let end = start + width;
            groups.push(leading[start..end].to_owned());
            start = end;
        }
    } else {
        let first_width = if leading.len() % last_group == 0 {
            last_group
        } else {
            leading.len() % last_group
        };
        let mut start = 0;
        while start < leading.len() {
            let width = if start == 0 { first_width } else { last_group };
            let end = start + width;
            groups.push(leading[start..end].to_owned());
            start = end;
        }
    }

    groups.push(trailing.to_owned());
    groups.join(separator)
}

fn parse_timestamp(value: i64) -> CoreResult<chrono::DateTime<Utc>> {
    let absolute = value.unsigned_abs();
    let datetime = if absolute >= 1_000_000_000_000 {
        Utc.timestamp_millis_opt(value).single()
    } else {
        Utc.timestamp_opt(value, 0).single()
    };

    datetime.ok_or(CoreError::InvalidInput("invalid datetime value"))
}

#[cfg(test)]
mod tests {
    use mf2_i18n_core::{FormatBackend, PluralCategory};

    use super::StdFormatBackend;

    #[test]
    fn formats_numbers_with_locale_separators() {
        let english = StdFormatBackend::new("en-US").expect("backend");
        let french = StdFormatBackend::new("fr-BE").expect("backend");

        assert_eq!(
            english.format_number(12345.5, &[]).expect("english number"),
            "12,345.5"
        );
        assert_eq!(
            french.format_number(12345.5, &[]).expect("french number"),
            "12\u{202f}345,5"
        );
    }

    #[test]
    fn selects_plural_categories_from_locale_rules() {
        let french = StdFormatBackend::new("fr").expect("backend");
        assert_eq!(
            french.plural_category(1.0).expect("plural"),
            PluralCategory::One
        );
        assert_eq!(
            french.plural_category(2.0).expect("plural"),
            PluralCategory::Other
        );
    }

    #[test]
    fn formats_dates_and_times_with_locale_patterns() {
        let english = StdFormatBackend::new("en-US").expect("backend");
        let french = StdFormatBackend::new("fr-BE").expect("backend");
        let timestamp = 994550400;

        assert_eq!(
            english.format_date(timestamp, &[]).expect("date"),
            "07/08/2001"
        );
        assert_eq!(
            french.format_date(timestamp, &[]).expect("date"),
            "08/07/01"
        );
        assert_eq!(
            english.format_time(timestamp, &[]).expect("time"),
            "12:00:00 AM"
        );
        assert_eq!(
            french.format_time(timestamp, &[]).expect("time"),
            "00:00:00"
        );
    }

    #[test]
    fn formats_currency_and_unit_with_localized_numbers() {
        let french = StdFormatBackend::new("fr-BE").expect("backend");

        assert_eq!(
            french
                .format_currency(12345.5, *b"USD", &[])
                .expect("currency"),
            "USD 12\u{202f}345,5"
        );
        assert_eq!(
            french.format_unit(12345.5, 7, &[]).expect("unit"),
            "12\u{202f}345,5 u7"
        );
    }
}
