#![forbid(unsafe_code)]

use std::str::FromStr;

use chrono::{TimeZone, Utc};
use intl_pluralrules::{PluralCategory as IntlPluralCategory, PluralRuleType, PluralRules};
use mf2_i18n_core::{
    CoreError, CoreResult, DateTimeValue, FormatBackend, FormatterOption, FormatterOptionValue,
    LanguageTag, PluralCategory,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StdFormatResolution {
    requested_locale: String,
    plural_locale: String,
    number_locale: Option<String>,
    date_locale: Option<String>,
}

impl StdFormatResolution {
    pub fn requested_locale(&self) -> &str {
        &self.requested_locale
    }

    pub fn plural_locale(&self) -> &str {
        &self.plural_locale
    }

    pub fn number_locale(&self) -> Option<&str> {
        self.number_locale.as_deref()
    }

    pub fn date_locale(&self) -> Option<&str> {
        self.date_locale.as_deref()
    }

    pub fn uses_fallback(&self) -> bool {
        self.plural_locale != self.requested_locale
            || self.number_locale.as_deref() != Some(self.requested_locale.as_str())
            || self.date_locale.as_deref() != Some(self.requested_locale.as_str())
    }
}

pub struct StdFormatBackend {
    plural_rules: PluralRules,
    number_locale: Option<NumberLocale>,
    date_locale: Option<DateLocale>,
    resolution: StdFormatResolution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CurrencyPattern {
    PrefixCode,
    SuffixCode,
}

impl StdFormatBackend {
    pub fn new(locale: &str) -> Result<Self, StdFormatError> {
        let candidates = locale_candidates(locale)?;
        let requested_locale = candidates
            .first()
            .cloned()
            .unwrap_or_else(|| locale.to_string());
        let (plural_rules, plural_locale) = resolve_plural_rules(&candidates)?;
        let number_locale = resolve_number_locale(&candidates);
        let date_locale = resolve_date_locale(&candidates);
        Ok(Self {
            plural_rules,
            number_locale: number_locale.as_ref().map(|(locale, _)| *locale),
            date_locale: date_locale.as_ref().map(|(locale, _)| *locale),
            resolution: StdFormatResolution {
                requested_locale,
                plural_locale,
                number_locale: number_locale.map(|(_, locale)| locale),
                date_locale: date_locale.map(|(_, locale)| locale),
            },
        })
    }

    pub fn resolution(&self) -> &StdFormatResolution {
        &self.resolution
    }

    fn number_locale(&self) -> CoreResult<&NumberLocale> {
        self.number_locale.as_ref().ok_or(CoreError::Unsupported(
            "number formatting data unavailable for locale",
        ))
    }

    fn date_locale(&self) -> CoreResult<DateLocale> {
        self.date_locale.ok_or(CoreError::Unsupported(
            "date formatting data unavailable for locale",
        ))
    }

    fn format_decimal(&self, value: f64) -> CoreResult<String> {
        let number_locale = self.number_locale()?;
        if value.is_nan() {
            return Ok(number_locale.nan().to_string());
        }

        let sign = if value.is_sign_negative() {
            number_locale.minus_sign().to_string()
        } else {
            String::new()
        };

        if value.is_infinite() {
            return Ok(format!("{sign}{}", number_locale.infinity()));
        }

        let raw = value.abs().to_string();
        if let Some(index) = raw.find(|ch| ch == 'e' || ch == 'E') {
            let mantissa = localize_decimal_string(&raw[..index], number_locale);
            return Ok(format!("{sign}{mantissa}{}", &raw[index..]));
        }

        Ok(format!(
            "{sign}{}",
            localize_decimal_string(&raw, number_locale)
        ))
    }

    fn format_datetime_with(&self, value: DateTimeValue, pattern: &str) -> CoreResult<String> {
        let datetime = parse_timestamp(value)?;
        let date_locale = self.date_locale()?;
        Ok(datetime.format_localized(pattern, date_locale).to_string())
    }

    fn format_currency_code(&self, code: &str, value: f64) -> CoreResult<String> {
        let amount = self.format_decimal(value)?;
        Ok(
            match resolve_currency_pattern(self.resolution.number_locale()) {
                CurrencyPattern::PrefixCode => format!("{code} {amount}"),
                CurrencyPattern::SuffixCode => format!("{amount} {code}"),
            },
        )
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
        self.format_decimal(value)
    }

    fn format_date(
        &self,
        value: DateTimeValue,
        _options: &[FormatterOption],
    ) -> CoreResult<String> {
        self.format_datetime_with(value, "%x")
    }

    fn format_time(
        &self,
        value: DateTimeValue,
        _options: &[FormatterOption],
    ) -> CoreResult<String> {
        self.format_datetime_with(value, "%X")
    }

    fn format_datetime(
        &self,
        value: DateTimeValue,
        _options: &[FormatterOption],
    ) -> CoreResult<String> {
        self.format_datetime_with(value, "%c")
    }

    fn format_unit(
        &self,
        _value: f64,
        _unit_id: u32,
        options: &[FormatterOption],
    ) -> CoreResult<String> {
        reject_supported_options(options, &[], "unit")?;
        Err(CoreError::Unsupported(
            "unit formatting requires unit label data",
        ))
    }

    fn format_currency(
        &self,
        value: f64,
        code: [u8; 3],
        options: &[FormatterOption],
    ) -> CoreResult<String> {
        reject_supported_options(options, &["display"], "currency")?;
        let display = match option_value(options, "display") {
            Some(FormatterOptionValue::Str(value)) => value.as_str(),
            Some(_) => {
                return Err(CoreError::InvalidInput(
                    "currency display option must be a string",
                ));
            }
            None => "code",
        };
        if display != "code" {
            return Err(CoreError::Unsupported(
                "currency formatting supports display=code only",
            ));
        }
        self.number_locale()?;
        let code =
            core::str::from_utf8(&code).map_err(|_| CoreError::InvalidInput("currency code"))?;
        self.format_currency_code(code, value)
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

    Ok(candidates)
}

fn resolve_plural_rules(candidates: &[String]) -> Result<(PluralRules, String), StdFormatError> {
    for candidate in candidates {
        if let Ok(identifier) = candidate.parse::<LanguageIdentifier>() {
            if let Ok(rules) = PluralRules::create(identifier, PluralRuleType::CARDINAL) {
                return Ok((rules, candidate.clone()));
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

fn resolve_number_locale(candidates: &[String]) -> Option<(NumberLocale, String)> {
    for candidate in candidates {
        for name in locale_name_variants(candidate) {
            if let Ok(locale) = NumberLocale::from_str(&name) {
                return Some((locale, candidate.clone()));
            }
        }
    }
    None
}

fn resolve_date_locale(candidates: &[String]) -> Option<(DateLocale, String)> {
    for candidate in candidates {
        for name in locale_name_variants(candidate) {
            if let Ok(locale) = DateLocale::from_str(&name) {
                return Some((locale, candidate.clone()));
            }
        }
    }
    None
}

fn resolve_currency_pattern(locale: Option<&str>) -> CurrencyPattern {
    let language = locale
        .and_then(|candidate| candidate.split(['-', '_']).next())
        .unwrap_or("en");

    match language {
        "en" => CurrencyPattern::PrefixCode,
        _ => CurrencyPattern::SuffixCode,
    }
}

fn locale_name_variants(candidate: &str) -> Vec<String> {
    let mut names = Vec::new();
    push_unique(&mut names, candidate.to_string());
    if candidate.contains('-') {
        push_unique(&mut names, candidate.replace('-', "_"));
    }
    names
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

fn option_value<'a>(options: &'a [FormatterOption], key: &str) -> Option<&'a FormatterOptionValue> {
    options
        .iter()
        .find(|option| option.key == key)
        .map(|option| &option.value)
}

fn reject_supported_options(
    options: &[FormatterOption],
    supported: &[&str],
    formatter: &'static str,
) -> CoreResult<()> {
    for option in options {
        if !supported
            .iter()
            .any(|supported_key| *supported_key == option.key)
        {
            return Err(CoreError::Unsupported(match formatter {
                "currency" => "currency formatter option not supported",
                "unit" => "unit formatter option not supported",
                _ => "formatter option not supported",
            }));
        }
    }
    Ok(())
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

fn parse_timestamp(value: DateTimeValue) -> CoreResult<chrono::DateTime<Utc>> {
    let datetime = match value {
        DateTimeValue::UnixSeconds(value) => Utc.timestamp_opt(value, 0).single(),
        DateTimeValue::UnixMilliseconds(value) => Utc.timestamp_millis_opt(value).single(),
    };
    datetime.ok_or(CoreError::InvalidInput("invalid datetime value"))
}

#[cfg(test)]
mod tests {
    use mf2_i18n_core::{
        DateTimeValue, FormatBackend, FormatterOption, FormatterOptionValue, PluralCategory,
    };

    use super::{StdFormatBackend, StdFormatError};

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
    fn exposes_locale_resolution_without_global_fallback() {
        let backend = StdFormatBackend::new("haw-US").expect("backend");
        let resolution = backend.resolution();

        assert_eq!(resolution.requested_locale(), "haw-US");
        assert_eq!(resolution.plural_locale(), "haw");
        assert_eq!(resolution.number_locale(), Some("haw"));
        assert_eq!(resolution.date_locale(), None);
        assert!(resolution.uses_fallback());
    }

    #[test]
    fn formats_dates_and_times_with_locale_patterns() {
        let english = StdFormatBackend::new("en-US").expect("backend");
        let french = StdFormatBackend::new("fr-BE").expect("backend");
        let seconds = DateTimeValue::unix_seconds(994550400);
        let milliseconds = DateTimeValue::unix_milliseconds(994550400000);

        assert_eq!(
            english.format_date(seconds, &[]).expect("date"),
            "07/08/2001"
        );
        assert_eq!(
            french.format_date(milliseconds, &[]).expect("date"),
            "08/07/01"
        );
        assert_eq!(
            english.format_time(seconds, &[]).expect("time"),
            "12:00:00 AM"
        );
        assert_eq!(
            french.format_time(milliseconds, &[]).expect("time"),
            "00:00:00"
        );
    }

    #[test]
    fn formats_currency_with_locale_sensitive_code_placement() {
        let english = StdFormatBackend::new("en-US").expect("backend");
        let french = StdFormatBackend::new("fr-BE").expect("backend");

        assert_eq!(
            english
                .format_currency(12345.5, *b"USD", &[])
                .expect("currency"),
            "USD 12,345.5"
        );
        assert_eq!(
            french
                .format_currency(12345.5, *b"USD", &[])
                .expect("currency"),
            "12\u{202f}345,5 USD"
        );
    }

    #[test]
    fn number_and_date_formatting_report_missing_locale_data() {
        let backend = StdFormatBackend::new("haw-US").expect("backend");
        let when = DateTimeValue::unix_seconds(994550400);

        assert_eq!(
            backend.format_number(12345.5, &[]).expect("number"),
            "12,345.5"
        );
        assert_eq!(
            backend
                .format_date(when, &[])
                .expect_err("date")
                .to_string(),
            "unsupported: date formatting data unavailable for locale"
        );
        assert_eq!(
            backend
                .format_time(when, &[])
                .expect_err("time")
                .to_string(),
            "unsupported: date formatting data unavailable for locale"
        );
        assert_eq!(
            backend
                .format_datetime(when, &[])
                .expect_err("datetime")
                .to_string(),
            "unsupported: date formatting data unavailable for locale"
        );
    }

    #[test]
    fn rejects_unsupported_currency_display_options() {
        let english = StdFormatBackend::new("en-US").expect("backend");
        let err = english
            .format_currency(
                12345.5,
                *b"USD",
                &[FormatterOption {
                    key: "display".to_string(),
                    value: FormatterOptionValue::Str("symbol".to_string()),
                }],
            )
            .expect_err("unsupported display");

        assert_eq!(
            err.to_string(),
            "unsupported: currency formatting supports display=code only"
        );
    }

    #[test]
    fn rejects_non_string_currency_display_options() {
        let english = StdFormatBackend::new("en-US").expect("backend");
        let err = english
            .format_currency(
                12345.5,
                *b"USD",
                &[FormatterOption {
                    key: "display".to_string(),
                    value: FormatterOptionValue::Bool(true),
                }],
            )
            .expect_err("invalid display");

        assert_eq!(
            err.to_string(),
            "invalid input: currency display option must be a string"
        );
    }

    #[test]
    fn unit_formatting_requires_label_data() {
        let french = StdFormatBackend::new("fr-BE").expect("backend");
        let err = french
            .format_unit(12345.5, 7, &[])
            .expect_err("unit formatting unsupported");

        assert_eq!(
            err.to_string(),
            "unsupported: unit formatting requires unit label data"
        );
    }

    #[test]
    fn unsupported_locale_requires_plural_rules() {
        let err = match StdFormatBackend::new("zz") {
            Ok(_) => panic!("expected missing plural rules"),
            Err(err) => err,
        };
        assert!(matches!(err, StdFormatError::MissingPluralRules(locale) if locale == "zz"));
    }
}
