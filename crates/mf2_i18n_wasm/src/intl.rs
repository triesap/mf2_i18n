use js_sys::{Array, Date, Object, Reflect};
use mf2_i18n_core::{
    CoreError, CoreResult, DateTimeValue, FormatBackend, FormatterOption, FormatterOptionValue,
    PluralCategory,
};
use wasm_bindgen::JsValue;

const MAX_JS_DATE_MILLISECONDS: i64 = 8_640_000_000_000_000;

pub(crate) struct BrowserIntlBackend {
    locale: String,
}

impl BrowserIntlBackend {
    pub(crate) fn new(locale: &str) -> Self {
        Self {
            locale: locale.to_owned(),
        }
    }

    fn format_number_with(&self, value: f64, options: &Object) -> CoreResult<String> {
        validate_finite(value, "number value")?;
        let locales = locale_array(&self.locale);
        let formatter = js_sys::Intl::NumberFormat::new(&locales, options);
        let rendered = formatter
            .format()
            .call1(&formatter, &JsValue::from_f64(value))
            .map_err(|_| CoreError::InvalidInput("number formatting failed"))?;
        rendered
            .as_string()
            .ok_or(CoreError::Internal("Intl number formatting result"))
    }
}

impl FormatBackend for BrowserIntlBackend {
    fn plural_category(&self, value: f64) -> CoreResult<PluralCategory> {
        validate_finite(value, "plural value")?;
        let locales = locale_array(&self.locale);
        let options = Object::new();
        let rules = js_sys::Intl::PluralRules::new(&locales, &options);
        let keyword = rules
            .select(value)
            .as_string()
            .ok_or(CoreError::Internal("Intl plural result"))?;
        plural_category_from_keyword(&keyword)
    }

    fn format_number(&self, value: f64, options: &[FormatterOption]) -> CoreResult<String> {
        let options = options_object(&mapped_number_options(options, NumberFormatTarget::Number)?)?;
        self.format_number_with(value, &options)
    }

    fn format_date(&self, value: DateTimeValue, options: &[FormatterOption]) -> CoreResult<String> {
        let options = options_object(&mapped_datetime_options(options, DateTimeTarget::Date)?)?;
        format_datetime_value(&self.locale, value, &options)
    }

    fn format_time(&self, value: DateTimeValue, options: &[FormatterOption]) -> CoreResult<String> {
        let options = options_object(&mapped_datetime_options(options, DateTimeTarget::Time)?)?;
        format_datetime_value(&self.locale, value, &options)
    }

    fn format_datetime(
        &self,
        value: DateTimeValue,
        options: &[FormatterOption],
    ) -> CoreResult<String> {
        let options = options_object(&mapped_datetime_options(options, DateTimeTarget::DateTime)?)?;
        format_datetime_value(&self.locale, value, &options)
    }

    fn format_unit(
        &self,
        _value: f64,
        _unit_id: u32,
        options: &[FormatterOption],
    ) -> CoreResult<String> {
        reject_options(options, "unit formatter option not supported")?;
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
        validate_finite(value, "currency value")?;
        let options = options_object(&mapped_currency_options(code, options)?)?;
        self.format_number_with(value, &options)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumberFormatTarget {
    Number,
    Currency,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DateTimeTarget {
    Date,
    Time,
    DateTime,
}

#[derive(Clone, Debug, PartialEq)]
struct MappedIntlOption {
    key: &'static str,
    value: MappedIntlValue,
}

#[derive(Clone, Debug, PartialEq)]
enum MappedIntlValue {
    Str(String),
    Num(f64),
    Bool(bool),
}

fn plural_category_from_keyword(keyword: &str) -> CoreResult<PluralCategory> {
    Ok(match keyword {
        "zero" => PluralCategory::Zero,
        "one" => PluralCategory::One,
        "two" => PluralCategory::Two,
        "few" => PluralCategory::Few,
        "many" => PluralCategory::Many,
        "other" => PluralCategory::Other,
        _ => return Err(CoreError::Unsupported("Intl plural category")),
    })
}

fn mapped_number_options(
    options: &[FormatterOption],
    target: NumberFormatTarget,
) -> CoreResult<Vec<MappedIntlOption>> {
    let mut mapped = Vec::with_capacity(options.len());
    for option in options {
        mapped.push(mapped_number_option(option, target)?);
    }
    validate_number_option_pairs(&mapped)?;
    Ok(mapped)
}

fn mapped_currency_options(
    code: [u8; 3],
    options: &[FormatterOption],
) -> CoreResult<Vec<MappedIntlOption>> {
    let mut mapped = Vec::with_capacity(options.len() + 2);
    mapped.push(str_option("style", "currency"));
    mapped.push(MappedIntlOption {
        key: "currency",
        value: MappedIntlValue::Str(currency_code(code)?),
    });
    mapped.extend(mapped_number_options(
        options,
        NumberFormatTarget::Currency,
    )?);
    Ok(mapped)
}

fn mapped_number_option(
    option: &FormatterOption,
    target: NumberFormatTarget,
) -> CoreResult<MappedIntlOption> {
    match option.key.as_str() {
        "style" if target == NumberFormatTarget::Number => mapped_string_option(
            option,
            "style",
            &["decimal", "percent"],
            "number style option not supported",
        ),
        "style" | "currency" => Err(CoreError::Unsupported(
            "currency formatter option not supported",
        )),
        "notation" => mapped_string_option(
            option,
            "notation",
            &["standard", "scientific", "engineering", "compact"],
            "number notation option not supported",
        ),
        "compact-display" => mapped_string_option(
            option,
            "compactDisplay",
            &["short", "long"],
            "number compact display option not supported",
        ),
        "sign-display" => mapped_string_option(
            option,
            "signDisplay",
            &["auto", "never", "always", "exceptZero"],
            "number sign display option not supported",
        ),
        "use-grouping" => mapped_bool_option(option, "useGrouping"),
        "minimum-integer-digits" => mapped_integer_option(
            option,
            "minimumIntegerDigits",
            1.0,
            21.0,
            "minimum integer digits option",
        ),
        "minimum-fraction-digits" => mapped_integer_option(
            option,
            "minimumFractionDigits",
            0.0,
            100.0,
            "minimum fraction digits option",
        ),
        "maximum-fraction-digits" => mapped_integer_option(
            option,
            "maximumFractionDigits",
            0.0,
            100.0,
            "maximum fraction digits option",
        ),
        "minimum-significant-digits" => mapped_integer_option(
            option,
            "minimumSignificantDigits",
            1.0,
            21.0,
            "minimum significant digits option",
        ),
        "maximum-significant-digits" => mapped_integer_option(
            option,
            "maximumSignificantDigits",
            1.0,
            21.0,
            "maximum significant digits option",
        ),
        "display" if target == NumberFormatTarget::Currency => mapped_string_option(
            option,
            "currencyDisplay",
            &["symbol", "narrowSymbol", "code", "name"],
            "currency display option not supported",
        ),
        "currency-sign" if target == NumberFormatTarget::Currency => mapped_string_option(
            option,
            "currencySign",
            &["standard", "accounting"],
            "currency sign option not supported",
        ),
        _ => Err(CoreError::Unsupported(
            "number formatter option not supported",
        )),
    }
}

fn mapped_datetime_options(
    options: &[FormatterOption],
    target: DateTimeTarget,
) -> CoreResult<Vec<MappedIntlOption>> {
    let mut mapped = Vec::with_capacity(options.len() + 6);
    let mut has_shape = false;

    for option in options {
        let mapped_option = mapped_datetime_option(option, target)?;
        if is_datetime_shape_key(mapped_option.key) {
            has_shape = true;
        }
        mapped.push(mapped_option);
    }

    validate_datetime_option_pairs(&mapped)?;

    if !has_shape {
        let mut defaults = default_datetime_options(target);
        defaults.extend(mapped);
        return Ok(defaults);
    }

    Ok(mapped)
}

fn mapped_datetime_option(
    option: &FormatterOption,
    target: DateTimeTarget,
) -> CoreResult<MappedIntlOption> {
    match option.key.as_str() {
        "date-style" if target != DateTimeTarget::Time => mapped_string_option(
            option,
            "dateStyle",
            &["full", "long", "medium", "short"],
            "date style option not supported",
        ),
        "time-style" if target != DateTimeTarget::Date => mapped_string_option(
            option,
            "timeStyle",
            &["full", "long", "medium", "short"],
            "time style option not supported",
        ),
        "date-style" | "time-style" => Err(CoreError::Unsupported(
            "datetime style option not supported for formatter",
        )),
        "time-zone" => mapped_unrestricted_string_option(option, "timeZone"),
        "hour12" => mapped_bool_option(option, "hour12"),
        "weekday" => mapped_string_option(
            option,
            "weekday",
            &["long", "short", "narrow"],
            "weekday option not supported",
        ),
        "era" => mapped_string_option(
            option,
            "era",
            &["long", "short", "narrow"],
            "era option not supported",
        ),
        "year" => mapped_string_option(
            option,
            "year",
            &["numeric", "2-digit"],
            "year option not supported",
        ),
        "month" => mapped_string_option(
            option,
            "month",
            &["numeric", "2-digit", "long", "short", "narrow"],
            "month option not supported",
        ),
        "day" => mapped_string_option(
            option,
            "day",
            &["numeric", "2-digit"],
            "day option not supported",
        ),
        "hour" => mapped_string_option(
            option,
            "hour",
            &["numeric", "2-digit"],
            "hour option not supported",
        ),
        "minute" => mapped_string_option(
            option,
            "minute",
            &["numeric", "2-digit"],
            "minute option not supported",
        ),
        "second" => mapped_string_option(
            option,
            "second",
            &["numeric", "2-digit"],
            "second option not supported",
        ),
        "fractional-second-digits" => mapped_integer_option(
            option,
            "fractionalSecondDigits",
            1.0,
            3.0,
            "fractional second digits option",
        ),
        "time-zone-name" => mapped_string_option(
            option,
            "timeZoneName",
            &[
                "short",
                "long",
                "shortOffset",
                "longOffset",
                "shortGeneric",
                "longGeneric",
            ],
            "time zone name option not supported",
        ),
        _ => Err(CoreError::Unsupported(
            "datetime formatter option not supported",
        )),
    }
}

fn default_datetime_options(target: DateTimeTarget) -> Vec<MappedIntlOption> {
    match target {
        DateTimeTarget::Date => vec![
            str_option("year", "numeric"),
            str_option("month", "2-digit"),
            str_option("day", "2-digit"),
        ],
        DateTimeTarget::Time => vec![
            str_option("hour", "2-digit"),
            str_option("minute", "2-digit"),
            str_option("second", "2-digit"),
        ],
        DateTimeTarget::DateTime => vec![
            str_option("year", "numeric"),
            str_option("month", "2-digit"),
            str_option("day", "2-digit"),
            str_option("hour", "2-digit"),
            str_option("minute", "2-digit"),
            str_option("second", "2-digit"),
        ],
    }
}

fn is_datetime_shape_key(key: &str) -> bool {
    matches!(
        key,
        "dateStyle"
            | "timeStyle"
            | "weekday"
            | "era"
            | "year"
            | "month"
            | "day"
            | "hour"
            | "minute"
            | "second"
            | "fractionalSecondDigits"
            | "timeZoneName"
    )
}

fn is_datetime_style_key(key: &str) -> bool {
    matches!(key, "dateStyle" | "timeStyle")
}

fn is_datetime_component_key(key: &str) -> bool {
    is_datetime_shape_key(key) && !is_datetime_style_key(key)
}

fn validate_number_option_pairs(options: &[MappedIntlOption]) -> CoreResult<()> {
    validate_number_min_max(
        options,
        "minimumFractionDigits",
        "maximumFractionDigits",
        "fraction digit options",
    )?;
    validate_number_min_max(
        options,
        "minimumSignificantDigits",
        "maximumSignificantDigits",
        "significant digit options",
    )
}

fn validate_number_min_max(
    options: &[MappedIntlOption],
    min_key: &'static str,
    max_key: &'static str,
    label: &'static str,
) -> CoreResult<()> {
    let min = mapped_number_value(options, min_key);
    let max = mapped_number_value(options, max_key);
    if let (Some(min), Some(max)) = (min, max)
        && min > max
    {
        return Err(CoreError::InvalidInput(label));
    }
    Ok(())
}

fn mapped_number_value(options: &[MappedIntlOption], key: &'static str) -> Option<f64> {
    options
        .iter()
        .find(|option| option.key == key)
        .and_then(|option| match &option.value {
            MappedIntlValue::Num(value) => Some(*value),
            _ => None,
        })
}

fn validate_datetime_option_pairs(options: &[MappedIntlOption]) -> CoreResult<()> {
    let has_style = options
        .iter()
        .any(|option| is_datetime_style_key(option.key));
    let has_component = options
        .iter()
        .any(|option| is_datetime_component_key(option.key));
    if has_style && has_component {
        return Err(CoreError::InvalidInput(
            "datetime style options cannot be combined with component options",
        ));
    }
    Ok(())
}

fn mapped_string_option(
    option: &FormatterOption,
    key: &'static str,
    allowed: &[&str],
    unsupported: &'static str,
) -> CoreResult<MappedIntlOption> {
    let FormatterOptionValue::Str(value) = &option.value else {
        return Err(CoreError::InvalidInput("formatter option must be a string"));
    };
    if !allowed.iter().any(|allowed| *allowed == value) {
        return Err(CoreError::Unsupported(unsupported));
    }
    Ok(MappedIntlOption {
        key,
        value: MappedIntlValue::Str(value.clone()),
    })
}

fn mapped_unrestricted_string_option(
    option: &FormatterOption,
    key: &'static str,
) -> CoreResult<MappedIntlOption> {
    let FormatterOptionValue::Str(value) = &option.value else {
        return Err(CoreError::InvalidInput("formatter option must be a string"));
    };
    if value.is_empty() {
        return Err(CoreError::InvalidInput("formatter option string is empty"));
    }
    Ok(MappedIntlOption {
        key,
        value: MappedIntlValue::Str(value.clone()),
    })
}

fn mapped_bool_option(option: &FormatterOption, key: &'static str) -> CoreResult<MappedIntlOption> {
    let FormatterOptionValue::Bool(value) = &option.value else {
        return Err(CoreError::InvalidInput("formatter option must be a bool"));
    };
    Ok(MappedIntlOption {
        key,
        value: MappedIntlValue::Bool(*value),
    })
}

fn mapped_integer_option(
    option: &FormatterOption,
    key: &'static str,
    min: f64,
    max: f64,
    label: &'static str,
) -> CoreResult<MappedIntlOption> {
    let FormatterOptionValue::Num(value) = &option.value else {
        return Err(CoreError::InvalidInput("formatter option must be a number"));
    };
    if !value.is_finite() || value.fract() != 0.0 || *value < min || *value > max {
        return Err(CoreError::InvalidInput(label));
    }
    Ok(MappedIntlOption {
        key,
        value: MappedIntlValue::Num(*value),
    })
}

fn str_option(key: &'static str, value: &'static str) -> MappedIntlOption {
    MappedIntlOption {
        key,
        value: MappedIntlValue::Str(value.to_owned()),
    }
}

fn options_object(options: &[MappedIntlOption]) -> CoreResult<Object> {
    let object = Object::new();
    for option in options {
        let value = match &option.value {
            MappedIntlValue::Str(value) => JsValue::from_str(value),
            MappedIntlValue::Num(value) => JsValue::from_f64(*value),
            MappedIntlValue::Bool(value) => JsValue::from_bool(*value),
        };
        let did_set = Reflect::set(&object, &JsValue::from_str(option.key), &value)
            .map_err(|_| CoreError::Internal("Intl option set failed"))?;
        if !did_set {
            return Err(CoreError::Internal("Intl option set failed"));
        }
    }
    Ok(object)
}

fn format_datetime_value(
    locale: &str,
    value: DateTimeValue,
    options: &Object,
) -> CoreResult<String> {
    let milliseconds = date_milliseconds(value)?;
    let date = Date::new(&JsValue::from_f64(milliseconds));
    let locales = locale_array(locale);
    let formatter = js_sys::Intl::DateTimeFormat::new(&locales, options);
    let rendered = formatter
        .format()
        .call1(&formatter, &JsValue::from(date))
        .map_err(|_| CoreError::InvalidInput("datetime formatting failed"))?;
    rendered
        .as_string()
        .ok_or(CoreError::Internal("Intl datetime formatting result"))
}

fn date_milliseconds(value: DateTimeValue) -> CoreResult<f64> {
    let milliseconds = match value {
        DateTimeValue::UnixSeconds(value) => value
            .checked_mul(1000)
            .ok_or(CoreError::InvalidInput("datetime value out of range"))?,
        DateTimeValue::UnixMilliseconds(value) => value,
    };
    if !(-MAX_JS_DATE_MILLISECONDS..=MAX_JS_DATE_MILLISECONDS).contains(&milliseconds) {
        return Err(CoreError::InvalidInput("datetime value out of range"));
    }
    Ok(milliseconds as f64)
}

fn currency_code(code: [u8; 3]) -> CoreResult<String> {
    if !code.iter().all(u8::is_ascii_alphabetic) {
        return Err(CoreError::InvalidInput("currency code"));
    }
    let code = core::str::from_utf8(&code).map_err(|_| CoreError::InvalidInput("currency code"))?;
    Ok(code.to_ascii_uppercase())
}

fn locale_array(locale: &str) -> Array {
    let locales = Array::new();
    locales.push(&JsValue::from_str(locale));
    locales
}

fn validate_finite(value: f64, label: &'static str) -> CoreResult<()> {
    if !value.is_finite() {
        return Err(CoreError::InvalidInput(label));
    }
    Ok(())
}

fn reject_options(options: &[FormatterOption], message: &'static str) -> CoreResult<()> {
    if options.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Unsupported(message))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DateTimeTarget, MappedIntlOption, MappedIntlValue, NumberFormatTarget, currency_code,
        date_milliseconds, mapped_currency_options, mapped_datetime_options, mapped_number_options,
        plural_category_from_keyword,
    };
    use mf2_i18n_core::{
        CoreError, DateTimeValue, FormatterOption, FormatterOptionValue, PluralCategory,
    };

    fn string_option(key: &str, value: &str) -> FormatterOption {
        FormatterOption {
            key: key.to_owned(),
            value: FormatterOptionValue::Str(value.to_owned()),
        }
    }

    fn number_option(key: &str, value: f64) -> FormatterOption {
        FormatterOption {
            key: key.to_owned(),
            value: FormatterOptionValue::Num(value),
        }
    }

    fn bool_option(key: &str, value: bool) -> FormatterOption {
        FormatterOption {
            key: key.to_owned(),
            value: FormatterOptionValue::Bool(value),
        }
    }

    fn mapped_str(key: &'static str, value: &str) -> MappedIntlOption {
        MappedIntlOption {
            key,
            value: MappedIntlValue::Str(value.to_owned()),
        }
    }

    fn mapped_num(key: &'static str, value: f64) -> MappedIntlOption {
        MappedIntlOption {
            key,
            value: MappedIntlValue::Num(value),
        }
    }

    fn mapped_bool(key: &'static str, value: bool) -> MappedIntlOption {
        MappedIntlOption {
            key,
            value: MappedIntlValue::Bool(value),
        }
    }

    #[test]
    fn plural_category_mapping_covers_intl_keywords() {
        assert_eq!(
            plural_category_from_keyword("zero").expect("zero"),
            PluralCategory::Zero
        );
        assert_eq!(
            plural_category_from_keyword("one").expect("one"),
            PluralCategory::One
        );
        assert_eq!(
            plural_category_from_keyword("two").expect("two"),
            PluralCategory::Two
        );
        assert_eq!(
            plural_category_from_keyword("few").expect("few"),
            PluralCategory::Few
        );
        assert_eq!(
            plural_category_from_keyword("many").expect("many"),
            PluralCategory::Many
        );
        assert_eq!(
            plural_category_from_keyword("other").expect("other"),
            PluralCategory::Other
        );
        assert_eq!(
            plural_category_from_keyword("all").expect_err("unsupported"),
            CoreError::Unsupported("Intl plural category")
        );
    }

    #[test]
    fn number_options_map_to_intl_names() {
        let mapped = mapped_number_options(
            &[
                string_option("style", "percent"),
                number_option("minimum-fraction-digits", 2.0),
                bool_option("use-grouping", true),
            ],
            NumberFormatTarget::Number,
        )
        .expect("options");

        assert_eq!(
            mapped,
            vec![
                mapped_str("style", "percent"),
                mapped_num("minimumFractionDigits", 2.0),
                mapped_bool("useGrouping", true),
            ]
        );
    }

    #[test]
    fn number_options_reject_unsupported_inputs() {
        assert_eq!(
            mapped_number_options(
                &[string_option("style", "currency")],
                NumberFormatTarget::Number,
            )
            .expect_err("style"),
            CoreError::Unsupported("number style option not supported")
        );
        assert_eq!(
            mapped_number_options(
                &[string_option("compact-display", "wide")],
                NumberFormatTarget::Number,
            )
            .expect_err("display"),
            CoreError::Unsupported("number compact display option not supported")
        );
        assert_eq!(
            mapped_number_options(
                &[string_option("minimum-fraction-digits", "2")],
                NumberFormatTarget::Number,
            )
            .expect_err("type"),
            CoreError::InvalidInput("formatter option must be a number")
        );
        assert_eq!(
            mapped_number_options(&[bool_option("unknown", true)], NumberFormatTarget::Number)
                .expect_err("unknown"),
            CoreError::Unsupported("number formatter option not supported")
        );
        assert_eq!(
            mapped_number_options(
                &[
                    number_option("minimum-fraction-digits", 3.0),
                    number_option("maximum-fraction-digits", 2.0),
                ],
                NumberFormatTarget::Number,
            )
            .expect_err("range"),
            CoreError::InvalidInput("fraction digit options")
        );
    }

    #[test]
    fn datetime_options_default_and_map_explicit_values() {
        let defaults =
            mapped_datetime_options(&[], DateTimeTarget::Time).expect("default time options");
        assert_eq!(
            defaults,
            vec![
                mapped_str("hour", "2-digit"),
                mapped_str("minute", "2-digit"),
                mapped_str("second", "2-digit"),
            ]
        );

        let mapped = mapped_datetime_options(
            &[
                string_option("date-style", "short"),
                string_option("time-zone", "UTC"),
            ],
            DateTimeTarget::Date,
        )
        .expect("date options");

        assert_eq!(
            mapped,
            vec![
                mapped_str("dateStyle", "short"),
                mapped_str("timeZone", "UTC"),
            ]
        );
    }

    #[test]
    fn datetime_options_reject_unsupported_inputs() {
        assert_eq!(
            mapped_datetime_options(
                &[string_option("time-style", "short")],
                DateTimeTarget::Date
            )
            .expect_err("style"),
            CoreError::Unsupported("datetime style option not supported for formatter")
        );
        assert_eq!(
            mapped_datetime_options(&[bool_option("hour", true)], DateTimeTarget::DateTime)
                .expect_err("type"),
            CoreError::InvalidInput("formatter option must be a string")
        );
        assert_eq!(
            mapped_datetime_options(
                &[string_option("calendar", "iso8601")],
                DateTimeTarget::Date
            )
            .expect_err("unknown"),
            CoreError::Unsupported("datetime formatter option not supported")
        );
        assert_eq!(
            mapped_datetime_options(
                &[
                    string_option("date-style", "short"),
                    string_option("year", "numeric"),
                ],
                DateTimeTarget::Date
            )
            .expect_err("style component"),
            CoreError::InvalidInput(
                "datetime style options cannot be combined with component options"
            )
        );
    }

    #[test]
    fn currency_options_validate_code_and_map_intl_options() {
        assert_eq!(currency_code(*b"usd").expect("code"), "USD");
        assert_eq!(
            currency_code(*b"US1").expect_err("invalid"),
            CoreError::InvalidInput("currency code")
        );

        let mapped = mapped_currency_options(
            *b"usd",
            &[
                string_option("display", "code"),
                string_option("currency-sign", "accounting"),
            ],
        )
        .expect("currency options");

        assert_eq!(
            mapped,
            vec![
                mapped_str("style", "currency"),
                mapped_str("currency", "USD"),
                mapped_str("currencyDisplay", "code"),
                mapped_str("currencySign", "accounting"),
            ]
        );
    }

    #[test]
    fn currency_options_reject_conflicting_or_invalid_options() {
        assert_eq!(
            mapped_currency_options(*b"USD", &[string_option("style", "decimal")])
                .expect_err("style"),
            CoreError::Unsupported("currency formatter option not supported")
        );
        assert_eq!(
            mapped_currency_options(*b"USD", &[string_option("display", "wide")])
                .expect_err("display"),
            CoreError::Unsupported("currency display option not supported")
        );
    }

    #[test]
    fn datetime_values_map_to_js_milliseconds() {
        assert_eq!(
            date_milliseconds(DateTimeValue::unix_seconds(994550400)).expect("seconds"),
            994550400000.0
        );
        assert_eq!(
            date_milliseconds(DateTimeValue::unix_milliseconds(994550400000))
                .expect("milliseconds"),
            994550400000.0
        );
        assert_eq!(
            date_milliseconds(DateTimeValue::unix_milliseconds(8_640_000_000_000_001))
                .expect_err("range"),
            CoreError::InvalidInput("datetime value out of range")
        );
    }
}
