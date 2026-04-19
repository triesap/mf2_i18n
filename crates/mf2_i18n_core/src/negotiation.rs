use alloc::string::String;
use alloc::vec::Vec;

use crate::LanguageTag;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NegotiationResult {
    pub selected: LanguageTag,
    pub requested: LanguageTag,
    pub trace: Option<NegotiationTrace>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NegotiationTrace {
    pub attempts: Vec<String>,
}

pub fn negotiate_lookup(
    requested: &[LanguageTag],
    supported: &[LanguageTag],
    default_locale: &LanguageTag,
) -> NegotiationResult {
    negotiate_lookup_internal(requested, supported, default_locale, false)
}

pub fn negotiate_lookup_with_trace(
    requested: &[LanguageTag],
    supported: &[LanguageTag],
    default_locale: &LanguageTag,
) -> NegotiationResult {
    negotiate_lookup_internal(requested, supported, default_locale, true)
}

fn negotiate_lookup_internal(
    requested: &[LanguageTag],
    supported: &[LanguageTag],
    default_locale: &LanguageTag,
    with_trace: bool,
) -> NegotiationResult {
    let mut trace = if with_trace {
        Some(NegotiationTrace {
            attempts: Vec::new(),
        })
    } else {
        None
    };

    for requested_tag in requested {
        let mut tried = Vec::new();
        tried.push(String::from(requested_tag.normalized()));

        let mut match_parts = requested_tag.match_subtags().to_vec();
        if !match_parts.is_empty() {
            let full_match = match_parts.join("-");
            if full_match != requested_tag.normalized() {
                tried.push(full_match);
            }
            while match_parts.len() > 1 {
                match_parts.pop();
                tried.push(match_parts.join("-"));
            }
        }

        for attempt in tried {
            if let Some(trace) = trace.as_mut() {
                trace.attempts.push(attempt.clone());
            }
            if let Some(selected) = find_supported(&attempt, supported) {
                return NegotiationResult {
                    selected,
                    requested: requested_tag.clone(),
                    trace,
                };
            }
        }
    }

    NegotiationResult {
        selected: default_locale.clone(),
        requested: requested
            .first()
            .cloned()
            .unwrap_or_else(|| default_locale.clone()),
        trace,
    }
}

fn find_supported(tag: &str, supported: &[LanguageTag]) -> Option<LanguageTag> {
    supported
        .iter()
        .find(|candidate| candidate.normalized() == tag)
        .cloned()
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::{negotiate_lookup, negotiate_lookup_with_trace};
    use crate::LanguageTag;

    fn tag(value: &str) -> LanguageTag {
        LanguageTag::parse(value).expect("valid tag")
    }

    #[test]
    fn lookup_falls_back_by_truncation() {
        let requested = vec![tag("en-GB")];
        let supported = vec![tag("en"), tag("fr")];
        let default_locale = tag("fr");
        let result = negotiate_lookup(&requested, &supported, &default_locale);
        assert_eq!(result.selected.normalized(), "en");
        assert_eq!(result.requested.normalized(), "en-GB");
    }

    #[test]
    fn lookup_prefers_exact_micro_locale() {
        let requested = vec![tag("es-PE-x-northperu")];
        let supported = vec![tag("es-PE-x-northperu"), tag("es-PE")];
        let default_locale = tag("en");
        let result = negotiate_lookup(&requested, &supported, &default_locale);
        assert_eq!(result.selected.normalized(), "es-PE-x-northperu");
    }

    #[test]
    fn lookup_drops_extensions_for_matching() {
        let requested = vec![tag("de-DE-u-co-phonebk")];
        let supported = vec![tag("de-DE")];
        let default_locale = tag("en");
        let result = negotiate_lookup(&requested, &supported, &default_locale);
        assert_eq!(result.selected.normalized(), "de-DE");
    }

    #[test]
    fn lookup_returns_default_when_missing() {
        let requested = vec![tag("ja-JP")];
        let supported = vec![tag("en")];
        let default_locale = tag("en");
        let result = negotiate_lookup(&requested, &supported, &default_locale);
        assert_eq!(result.selected.normalized(), "en");
    }

    #[test]
    fn trace_records_attempts() {
        let requested = vec![tag("de-DE-u-co-phonebk")];
        let supported = vec![tag("de-DE")];
        let default_locale = tag("en");
        let result = negotiate_lookup_with_trace(&requested, &supported, &default_locale);
        let trace = result.trace.expect("trace should be present");
        assert_eq!(
            trace.attempts,
            vec![String::from("de-DE-u-co-phonebk"), String::from("de-DE")]
        );
    }
}
