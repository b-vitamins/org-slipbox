use std::fmt::Display;
use std::str::FromStr;

use chrono::NaiveDate;

use crate::http::response::ApiError;

/// A parsed query string: the decoded key/value pairs of one request URL.
///
/// Values are percent-decoded once at construction, with `+` treated as a space
/// per `application/x-www-form-urlencoded`. An out-of-range bound is rejected
/// rather than clamped.
#[derive(Debug)]
pub(crate) struct Query {
    pairs: Vec<(String, String)>,
}

impl Query {
    /// Parse the portion of a request URL after `?`.
    pub(crate) fn parse(raw: &str) -> Result<Self, ApiError> {
        let pairs = raw
            .split('&')
            .filter(|segment| !segment.is_empty())
            .map(|segment| match segment.split_once('=') {
                Some((key, value)) => Ok((decode(key)?, decode(value)?)),
                None => Ok((decode(segment)?, String::new())),
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        Ok(Self { pairs })
    }

    /// The first value given for `key`, present or empty.
    fn first(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(candidate, _)| candidate == key)
            .map(|(_, value)| value.as_str())
    }

    /// The first non-empty value for `key`, reporting an empty one.
    fn given(&self, key: &'static str) -> Result<Option<&str>, ApiError> {
        match self.first(key) {
            None => Ok(None),
            Some("") => Err(ApiError::bad_request(format!(
                "query parameter `{key}` was given with no value"
            ))),
            Some(value) => Ok(Some(value)),
        }
    }

    /// Require a string parameter.
    pub(crate) fn require(&self, key: &'static str) -> Result<String, ApiError> {
        self.optional(key)?.ok_or_else(|| {
            ApiError::bad_request(format!("missing required query parameter `{key}`"))
        })
    }

    /// Return an optional string parameter.
    pub(crate) fn optional(&self, key: &'static str) -> Result<Option<String>, ApiError> {
        Ok(self.given(key)?.map(ToOwned::to_owned))
    }

    /// Parse an optional parameter of any [`FromStr`] type.
    pub(crate) fn optional_parsed<T>(&self, key: &'static str) -> Result<Option<T>, ApiError>
    where
        T: FromStr,
    {
        match self.given(key)? {
            None => Ok(None),
            Some(raw) => raw.parse::<T>().map(Some).map_err(|_| {
                ApiError::bad_request(format!(
                    "query parameter `{key}` has an invalid value `{raw}`"
                ))
            }),
        }
    }

    /// Parse an optional numeric parameter that must fall within `min..=max`. An
    /// out-of-range value is refused rather than pulled to the nearest bound.
    pub(crate) fn optional_bounded<T>(
        &self,
        key: &'static str,
        min: T,
        max: T,
    ) -> Result<Option<T>, ApiError>
    where
        T: FromStr + PartialOrd + Display,
    {
        let Some(value) = self.optional_parsed::<T>(key)? else {
            return Ok(None);
        };
        if value < min || value > max {
            return Err(ApiError::bad_request(format!(
                "query parameter `{key}` must be between {min} and {max}, got `{value}`"
            )));
        }
        Ok(Some(value))
    }

    /// Parse a bounded numeric parameter, substituting `default` when absent.
    pub(crate) fn bounded<T>(
        &self,
        key: &'static str,
        default: T,
        min: T,
        max: T,
    ) -> Result<T, ApiError>
    where
        T: FromStr + PartialOrd + Display,
    {
        Ok(self.optional_bounded(key, min, max)?.unwrap_or(default))
    }

    /// Parse an optional boolean parameter.
    pub(crate) fn optional_bool(&self, key: &'static str) -> Result<Option<bool>, ApiError> {
        match self.given(key)? {
            None => Ok(None),
            Some(raw) => match raw.to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" => Ok(Some(true)),
                "0" | "false" | "no" | "off" => Ok(Some(false)),
                _ => Err(ApiError::bad_request(format!(
                    "query parameter `{key}` must be a boolean, got `{raw}`"
                ))),
            },
        }
    }

    /// Parse an optional ISO `YYYY-MM-DD` calendar date, returned in that same
    /// spelling. The index compares it as a string against stored ISO dates,
    /// where an unparsable value would answer with the wrong set of terms
    /// instead of failing.
    pub(crate) fn optional_date(&self, key: &'static str) -> Result<Option<String>, ApiError> {
        match self.given(key)? {
            None => Ok(None),
            Some(raw) => match NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
                Ok(date) => Ok(Some(date.format("%Y-%m-%d").to_string())),
                Err(_) => Err(ApiError::bad_request(format!(
                    "query parameter `{key}` must be an ISO date (YYYY-MM-DD), got `{raw}`"
                ))),
            },
        }
    }
}

/// Percent-decode a single query token, mapping `+` to space first.
///
/// A decoded NUL fails the request: the index reaches SQLite through
/// NUL-terminated strings, where the byte would cut the value short or fail deep
/// in the query engine.
fn decode(token: &str) -> Result<String, ApiError> {
    let plus_normalized = token.replace('+', " ");
    let decoded = match urlencoding::decode(&plus_normalized) {
        Ok(decoded) => decoded.into_owned(),
        Err(_) => {
            return Err(ApiError::bad_request(format!(
                "query string contains an invalid percent-encoded sequence: `{token}`"
            )));
        }
    };
    if decoded.contains('\0') {
        return Err(ApiError::bad_request(format!(
            "query string contains a NUL byte: `{token}`"
        )));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::Query;

    fn query(raw: &str) -> Query {
        Query::parse(raw).expect("the raw query parses")
    }

    #[test]
    fn decodes_percent_escapes_and_plus_as_space() {
        let query = query("q=k%2Dmeans+objective&key=notes%2Fa.org%3A%3A0");
        assert_eq!(
            query.optional("q").expect("q is well formed"),
            Some("k-means objective".to_owned())
        );
        assert_eq!(
            query.optional("key").expect("key is well formed"),
            Some("notes/a.org::0".to_owned())
        );
    }

    #[test]
    fn an_undecodable_escape_is_a_bad_request() {
        // `%FF` is not valid UTF-8.
        let error = Query::parse("q=%FF").expect_err("an invalid escape is refused");
        assert_eq!(error.status, 400);
    }

    #[test]
    fn a_nul_byte_is_a_bad_request_wherever_it_appears() {
        for raw in ["q=a%00b", "q=%00", "key=notes%2Fa.org%00"] {
            let error = Query::parse(raw).expect_err("a NUL is refused");
            assert_eq!(error.status, 400);
            assert!(error.body().contains("NUL byte"));
        }
    }

    #[test]
    fn an_absent_parameter_differs_from_an_empty_one() {
        assert_eq!(
            query("").optional("q").expect("absent is not an error"),
            None
        );
        let empty = query("q=")
            .optional("q")
            .expect_err("an empty value is an error");
        assert_eq!(empty.status, 400);
        assert!(empty.body().contains("given with no value"));
        let missing = query("")
            .require("q")
            .expect_err("a required parameter is missing");
        assert_eq!(missing.status, 400);
        assert!(missing.body().contains("missing required"));
    }

    #[test]
    fn a_bounded_value_defaults_when_absent() {
        assert_eq!(
            query("")
                .bounded("limit", 50_usize, 1, 200)
                .expect("default"),
            50
        );
        assert_eq!(
            query("limit=10")
                .bounded("limit", 50_usize, 1, 200)
                .expect("in range"),
            10
        );
    }

    #[test]
    fn a_bounded_value_out_of_range_is_refused_not_clamped() {
        for raw in ["limit=0", "limit=201"] {
            let error = query(raw)
                .bounded("limit", 50_usize, 1, 200)
                .expect_err("out of range is refused");
            assert_eq!(error.status, 400);
        }
    }

    #[test]
    fn a_boolean_accepts_the_common_spellings() {
        assert_eq!(
            query("unique=TRUE").optional_bool("unique").expect("true"),
            Some(true)
        );
        assert_eq!(
            query("unique=off").optional_bool("unique").expect("false"),
            Some(false)
        );
        assert_eq!(
            query("unique=perhaps")
                .optional_bool("unique")
                .expect_err("not a boolean")
                .status,
            400
        );
    }

    #[test]
    fn a_date_must_be_a_real_calendar_day() {
        assert_eq!(
            query("today=2026-07-25")
                .optional_date("today")
                .expect("a real date"),
            Some("2026-07-25".to_owned())
        );
        for raw in ["today=garbage", "today=2026-13-01", "today=2026-02-30"] {
            let error = query(raw)
                .optional_date("today")
                .expect_err("not a calendar date");
            assert_eq!(error.status, 400);
        }
    }

    #[test]
    fn the_first_value_wins_for_a_repeated_parameter() {
        assert_eq!(
            query("q=first&q=second").optional("q").expect("first wins"),
            Some("first".to_owned())
        );
    }
}
