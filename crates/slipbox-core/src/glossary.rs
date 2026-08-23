//! Glossary terms and SM-2 spaced-repetition scheduling.
//!
//! A glossary term is an ordinary Org note carrying a `#+glossary:` marker and
//! an `SR_*` review drawer. The record types in [`crate::nodes`] mirror that
//! drawer verbatim as text, so they stay serde-friendly and comparable. This
//! module owns the typed scheduling surface ([`SrState`]) and the pure SM-2
//! update ([`sm2_schedule`]) used to reschedule a term after grading.
//!
//! The scheduler is deliberately date-library-free: dates are ISO
//! `YYYY-MM-DD` strings, and day arithmetic uses the integer civil-calendar
//! algorithm below so this crate keeps depending only on `serde`.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::nodes::NodeRecord;
use crate::validation::default_search_limit;

/// New-card ease factor, matching the SM-2 prototype.
const DEFAULT_EASE: f64 = 2.5;

/// Lowest ease factor SM-2 permits.
const MIN_EASE: f64 = 1.3;

/// Confirmation state of a glossary term.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryStatus {
    /// A placeholder term whose definition is not yet trusted.
    #[default]
    Stub,
    /// A term whose definition is complete.
    Confirmed,
}

impl GlossaryStatus {
    /// The lowercase token stored in the `GLOSSARY_STATUS` drawer key.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stub => "stub",
            Self::Confirmed => "confirmed",
        }
    }
}

impl FromStr for GlossaryStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "stub" => Ok(Self::Stub),
            "confirmed" => Ok(Self::Confirmed),
            _ => Err(()),
        }
    }
}

/// Typed SM-2 scheduling state for a term.
///
/// This is the computed form of the `SR_*` drawer keys. The record types mirror
/// the drawer as text; this struct is the numeric surface the scheduler reads
/// and writes. `ease` is a float, so this type is intentionally not `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SrState {
    /// SM-2 ease factor, never below [`MIN_EASE`].
    pub ease: f64,
    /// Current inter-repetition interval in days.
    pub interval: i64,
    /// Count of successful repetitions.
    pub reps: i64,
    /// ISO `YYYY-MM-DD` date the term is next due, if scheduled.
    pub due: Option<String>,
    /// ISO `YYYY-MM-DD` date the term was last graded, if ever.
    pub last: Option<String>,
}

impl Default for SrState {
    fn default() -> Self {
        Self {
            ease: DEFAULT_EASE,
            interval: 0,
            reps: 0,
            due: None,
            last: None,
        }
    }
}

impl SrState {
    /// Read scheduling state from a note's `SR_*` drawer text.
    ///
    /// Missing or malformed numeric fields fall back to the new-card defaults,
    /// so a hand-edited drawer degrades gracefully instead of failing a review.
    #[must_use]
    pub fn from_record(record: &NodeRecord) -> Self {
        let default = Self::default();
        Self {
            ease: parse_field(record.sr_ease.as_deref(), default.ease),
            interval: parse_field(record.sr_interval.as_deref(), default.interval),
            reps: parse_field(record.sr_reps.as_deref(), default.reps),
            due: record.sr_due.clone(),
            last: record.sr_last.clone(),
        }
    }

    /// Whether a term in this state is due for review on `today`.
    ///
    /// Never-reviewed terms (`reps == 0`) and terms with no due date are always
    /// due; otherwise a term is due once its ISO due date is at or before
    /// `today`. This mirrors the prototype's `Due` predicate.
    #[must_use]
    pub fn is_due(&self, today: &str) -> bool {
        if self.reps == 0 {
            return true;
        }
        match &self.due {
            Some(due) => due.as_str() <= today,
            None => true,
        }
    }

    /// The `SR_EASE` drawer value, formatted to two decimals.
    #[must_use]
    pub fn ease_string(&self) -> String {
        format!("{:.2}", self.ease)
    }

    /// The `SR_INTERVAL` drawer value.
    #[must_use]
    pub fn interval_string(&self) -> String {
        self.interval.to_string()
    }

    /// The `SR_REPS` drawer value.
    #[must_use]
    pub fn reps_string(&self) -> String {
        self.reps.to_string()
    }
}

fn parse_field<T: FromStr>(value: Option<&str>, fallback: T) -> T {
    value
        .and_then(|raw| raw.trim().parse().ok())
        .unwrap_or(fallback)
}

/// Apply an SM-2 update for a term graded `quality` (`0..=5`) on `today`.
///
/// This is a faithful port of the `myglossary` `Grade` routine:
///
/// - `quality < 3` resets the schedule to `reps = 0`, `interval = 1`.
/// - otherwise the interval steps `0 -> 1`, `1 -> 6`, then
///   `round(interval * ease)` using the old ease, and `reps` increments.
/// - the ease factor is always adjusted by
///   `0.1 - (5 - quality) * (0.08 + (5 - quality) * 0.02)`, floored at
///   [`MIN_EASE`], even on a lapse.
/// - `last` becomes `today` and `due` becomes `today + interval` days.
#[must_use]
pub fn sm2_schedule(state: &SrState, quality: i64, today: &str) -> SrState {
    let quality = quality.clamp(0, 5);
    let mut next = state.clone();

    if quality < 3 {
        next.reps = 0;
        next.interval = 1;
    } else {
        next.interval = match next.reps {
            0 => 1,
            1 => 6,
            // Round half up, matching the prototype's `int(interval*ease + 0.5)`.
            _ => (next.interval as f64 * next.ease + 0.5) as i64,
        };
        next.reps += 1;
    }

    let miss = (5 - quality) as f64;
    next.ease += 0.1 - miss * (0.08 + miss * 0.02);
    if next.ease < MIN_EASE {
        next.ease = MIN_EASE;
    }

    next.last = Some(today.to_string());
    next.due = Some(add_days(today, next.interval));
    next
}

/// Add `days` to an ISO `YYYY-MM-DD` date, returning a new ISO date.
///
/// A malformed input is returned unchanged so the scheduler can never panic on
/// a hand-edited drawer value.
fn add_days(date: &str, days: i64) -> String {
    match parse_iso(date) {
        Some((year, month, day)) => {
            let serial = days_from_civil(year, month, day) + days;
            let (ny, nm, nd) = civil_from_days(serial);
            format!("{ny:04}-{nm:02}-{nd:02}")
        }
        None => date.to_string(),
    }
}

/// Parse an exact `YYYY-MM-DD` string into a `(year, month, day)` triple.
fn parse_iso(date: &str) -> Option<(i64, i64, i64)> {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year: i64 = date.get(0..4)?.parse().ok()?;
    let month: i64 = date.get(5..7)?.parse().ok()?;
    let day: i64 = date.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some((year, month, day))
}

/// Days since the Unix epoch for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = (if year >= 0 { year } else { year - 399 }) / 400;
    let year_of_era = year - era * 400; // [0, 399]
    let month_index = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_index + 2) / 5 + day - 1; // [0, 365]
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146097 + day_of_era - 719468
}

/// Civil date for a count of days since the Unix epoch (inverse of the above).
fn civil_from_days(serial: i64) -> (i64, i64, i64) {
    let serial = serial + 719468;
    let era = (if serial >= 0 { serial } else { serial - 146096 }) / 146097;
    let day_of_era = serial - era * 146097; // [0, 146096]
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365; // [0, 399]
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100); // [0, 365]
    let month_index = (5 * day_of_year + 2) / 153; // [0, 11]
    let day = day_of_year - (153 * month_index + 2) / 5 + 1; // [1, 31]
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

/// Parameters for listing glossary terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGlossaryTermsParams {
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    /// Opaque position handed back by an earlier page; the first page when absent.
    #[serde(default)]
    pub after: Option<String>,
}

impl ListGlossaryTermsParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }

    /// The position this page continues from, if any.
    #[must_use]
    pub fn normalized_after(&self) -> Option<&str> {
        normalized_position(self.after.as_deref())
    }
}

/// Result of listing glossary terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListGlossaryTermsResult {
    pub terms: Vec<NodeRecord>,
    /// Terms the whole listing holds, not just this page.
    pub total: usize,
    /// Whether terms follow this page.
    pub has_more: bool,
    /// Position to pass as `after` for the next page; absent at the listing's end.
    pub next_position: Option<String>,
}

/// Parameters for searching glossary terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchGlossaryParams {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

impl SearchGlossaryParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }
}

/// Result of searching glossary terms.
///
/// Search ranks by relevance rather than by a stored key, so it serves one page
/// and states the cut instead of handing out a position.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchGlossaryResult {
    pub terms: Vec<NodeRecord>,
    /// Terms matching the query, not just the ones this page holds.
    pub total: usize,
    /// Whether the match set continues past this page.
    pub has_more: bool,
}

/// Parameters for inspecting one glossary term by node key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryTermParams {
    pub node_key: String,
}

/// Result of inspecting one glossary term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryTermResult {
    pub term: Option<NodeRecord>,
}

/// Parameters for selecting terms due for review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryDueParams {
    /// ISO `YYYY-MM-DD` reference date; the server's date when omitted.
    #[serde(default)]
    pub today: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    /// Opaque position handed back by an earlier page; the first page when absent.
    #[serde(default)]
    pub after: Option<String>,
}

impl GlossaryDueParams {
    #[must_use]
    pub fn normalized_limit(&self) -> usize {
        self.limit.clamp(1, 200)
    }

    /// The position this page continues from, if any.
    #[must_use]
    pub fn normalized_after(&self) -> Option<&str> {
        normalized_position(self.after.as_deref())
    }
}

/// Result of selecting due terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryDueResult {
    pub terms: Vec<NodeRecord>,
    /// Terms the whole listing holds, not just this page.
    pub total: usize,
    /// Whether terms follow this page.
    pub has_more: bool,
    /// Position to pass as `after` for the next page; absent at the listing's end.
    pub next_position: Option<String>,
}

/// Read a listing position, trimming the request's own padding.
///
/// A given token stays given even when nothing is left of it: the index mints no
/// blank position, so the listing refuses it rather than answering the first page
/// under a token it cannot read.
fn normalized_position(after: Option<&str>) -> Option<&str> {
    after.map(str::trim)
}

/// Parameters for grading a term and rescheduling it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradeTermParams {
    pub node_key: String,
    /// SM-2 quality score; clamped to `0..=5` during scheduling.
    pub quality: i64,
    /// ISO `YYYY-MM-DD` reference date; the server's date when omitted.
    #[serde(default)]
    pub today: Option<String>,
}

/// Result of grading a term, carrying the rescheduled record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradeTermResult {
    pub term: NodeRecord,
}

/// Parameters for marking an existing note as a glossary term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkGlossaryTermParams {
    pub node_key: String,
    /// Confirmation status to record; defaults to [`GlossaryStatus::Stub`].
    #[serde(default)]
    pub status: Option<GlossaryStatus>,
}

/// Result of marking a note as a glossary term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkGlossaryTermResult {
    pub term: NodeRecord,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nodes::{NodeKind, NodeRecord};

    fn base_state() -> SrState {
        SrState::default()
    }

    #[test]
    fn new_card_defaults_match_prototype() {
        let state = SrState::default();
        assert!((state.ease - 2.5).abs() < f64::EPSILON);
        assert_eq!(state.interval, 0);
        assert_eq!(state.reps, 0);
        assert_eq!(state.due, None);
        assert_eq!(state.last, None);
    }

    #[test]
    fn first_good_grade_schedules_one_day() {
        let next = sm2_schedule(&base_state(), 5, "2026-07-21");
        assert!((next.ease - 2.6).abs() < 1e-9);
        assert_eq!(next.interval, 1);
        assert_eq!(next.reps, 1);
        assert_eq!(next.due.as_deref(), Some("2026-07-22"));
        assert_eq!(next.last.as_deref(), Some("2026-07-21"));
    }

    #[test]
    fn second_good_grade_schedules_six_days() {
        let first = sm2_schedule(&base_state(), 5, "2026-07-21");
        let second = sm2_schedule(&first, 5, "2026-07-22");
        assert!((second.ease - 2.7).abs() < 1e-9);
        assert_eq!(second.interval, 6);
        assert_eq!(second.reps, 2);
        assert_eq!(second.due.as_deref(), Some("2026-07-28"));
    }

    #[test]
    fn third_good_grade_uses_rounded_interval_and_old_ease() {
        // ease 2.7, interval 6, reps 2 -> int(6 * 2.7 + 0.5) = 16.
        let state = SrState {
            ease: 2.7,
            interval: 6,
            reps: 2,
            due: Some("2026-07-28".to_string()),
            last: Some("2026-07-22".to_string()),
        };
        // quality 4: ease delta is exactly zero, so ease stays 2.7.
        let next = sm2_schedule(&state, 4, "2026-07-28");
        assert!((next.ease - 2.7).abs() < 1e-9);
        assert_eq!(next.interval, 16);
        assert_eq!(next.reps, 3);
        assert_eq!(next.due.as_deref(), Some("2026-08-13"));
    }

    #[test]
    fn lapse_resets_interval_and_lowers_ease() {
        let state = SrState {
            ease: 2.7,
            interval: 16,
            reps: 3,
            due: Some("2026-08-13".to_string()),
            last: Some("2026-07-28".to_string()),
        };
        let next = sm2_schedule(&state, 1, "2026-08-13");
        // 0.1 - 4 * (0.08 + 4 * 0.02) = 0.1 - 0.64 = -0.54.
        assert!((next.ease - 2.16).abs() < 1e-9);
        assert_eq!(next.interval, 1);
        assert_eq!(next.reps, 0);
        assert_eq!(next.due.as_deref(), Some("2026-08-14"));
    }

    #[test]
    fn ease_never_drops_below_floor() {
        let state = SrState {
            ease: 1.3,
            interval: 1,
            reps: 0,
            due: None,
            last: None,
        };
        let next = sm2_schedule(&state, 0, "2026-01-01");
        assert!((next.ease - 1.3).abs() < f64::EPSILON);
        assert_eq!(next.interval, 1);
        assert_eq!(next.reps, 0);
    }

    #[test]
    fn quality_is_clamped_to_range() {
        let high = sm2_schedule(&base_state(), 42, "2026-07-21");
        let five = sm2_schedule(&base_state(), 5, "2026-07-21");
        assert_eq!(high.due, five.due);
        assert!((high.ease - five.ease).abs() < f64::EPSILON);

        let low = sm2_schedule(&base_state(), -9, "2026-07-21");
        let zero = sm2_schedule(&base_state(), 0, "2026-07-21");
        assert_eq!(low.reps, zero.reps);
        assert!((low.ease - zero.ease).abs() < f64::EPSILON);
    }

    #[test]
    fn add_days_crosses_month_year_and_leap_boundaries() {
        assert_eq!(add_days("2026-07-28", 16), "2026-08-13");
        assert_eq!(add_days("2026-12-31", 1), "2027-01-01");
        assert_eq!(add_days("2024-02-28", 1), "2024-02-29");
        assert_eq!(add_days("2023-02-28", 1), "2023-03-01");
        assert_eq!(add_days("2026-01-01", 0), "2026-01-01");
    }

    #[test]
    fn add_days_passes_through_malformed_input() {
        assert_eq!(add_days("not-a-date", 5), "not-a-date");
        assert_eq!(add_days("2026-13-01", 1), "2026-13-01");
    }

    #[test]
    fn due_predicate_matches_prototype() {
        // Never reviewed is always due.
        assert!(SrState::default().is_due("2026-07-21"));

        let reviewed = SrState {
            ease: 2.5,
            interval: 6,
            reps: 2,
            due: Some("2026-07-21".to_string()),
            last: Some("2026-07-15".to_string()),
        };
        // Due today counts as due.
        assert!(reviewed.is_due("2026-07-21"));
        // Overdue counts as due.
        assert!(reviewed.is_due("2026-07-22"));
        // Scheduled in the future is not due.
        assert!(!reviewed.is_due("2026-07-20"));

        // A reviewed term with no due date is treated as due.
        let no_due = SrState {
            reps: 1,
            due: None,
            ..SrState::default()
        };
        assert!(no_due.is_due("2026-07-21"));
    }

    #[test]
    fn state_round_trips_through_record_text() {
        let scheduled = sm2_schedule(&base_state(), 5, "2026-07-21");
        let record = NodeRecord {
            node_key: "file:term.org".to_string(),
            explicit_id: None,
            file_path: "term.org".to_string(),
            title: "Derivative".to_string(),
            outline_path: String::new(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: Vec::new(),
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            glossary: true,
            glossary_status: Some("confirmed".to_string()),
            sr_due: scheduled.due.clone(),
            sr_ease: Some(scheduled.ease_string()),
            sr_interval: Some(scheduled.interval_string()),
            sr_reps: Some(scheduled.reps_string()),
            sr_last: scheduled.last.clone(),
            level: 0,
            line: 1,
            kind: NodeKind::File,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        };

        let recovered = SrState::from_record(&record);
        assert_eq!(recovered.interval, scheduled.interval);
        assert_eq!(recovered.reps, scheduled.reps);
        assert_eq!(recovered.due, scheduled.due);
        assert_eq!(recovered.last, scheduled.last);
        assert!((recovered.ease - scheduled.ease).abs() < 1e-9);
    }

    #[test]
    fn from_record_falls_back_on_missing_or_malformed_fields() {
        let record = NodeRecord {
            node_key: "file:term.org".to_string(),
            explicit_id: None,
            file_path: "term.org".to_string(),
            title: "Term".to_string(),
            outline_path: String::new(),
            aliases: Vec::new(),
            tags: Vec::new(),
            refs: Vec::new(),
            todo_keyword: None,
            scheduled_for: None,
            deadline_for: None,
            closed_at: None,
            glossary: true,
            glossary_status: None,
            sr_due: None,
            sr_ease: Some("garbage".to_string()),
            sr_interval: None,
            sr_reps: Some(String::new()),
            sr_last: None,
            level: 0,
            line: 1,
            kind: NodeKind::File,
            file_mtime_ns: 0,
            backlink_count: 0,
            forward_link_count: 0,
        };

        let state = SrState::from_record(&record);
        assert!((state.ease - 2.5).abs() < f64::EPSILON);
        assert_eq!(state.interval, 0);
        assert_eq!(state.reps, 0);
    }

    #[test]
    fn a_listing_page_is_bounded_whatever_limit_is_asked_for() {
        for limit in [0, 1, 50, 200, 10_000] {
            let listed = ListGlossaryTermsParams { limit, after: None };
            let due = GlossaryDueParams {
                today: None,
                limit,
                after: None,
            };
            assert!((1..=200).contains(&listed.normalized_limit()), "{limit}");
            assert_eq!(listed.normalized_limit(), due.normalized_limit(), "{limit}");
        }
    }

    #[test]
    fn only_an_absent_position_reads_as_the_first_page() {
        let listed = ListGlossaryTermsParams {
            limit: 50,
            after: None,
        };
        assert_eq!(listed.normalized_after(), None);
        let due = GlossaryDueParams {
            today: None,
            limit: 50,
            after: None,
        };
        assert_eq!(due.normalized_after(), None);

        // A given position stays a position however little of it there is: a
        // blank token reaches the index, which mints none, so it is refused
        // rather than answered with the first page.
        for after in [String::new(), "  ".to_owned(), "\t\n".to_owned()] {
            let listed = ListGlossaryTermsParams {
                limit: 50,
                after: Some(after.clone()),
            };
            assert_eq!(listed.normalized_after(), Some(""), "{after:?}");
            let due = GlossaryDueParams {
                today: None,
                limit: 50,
                after: Some(after.clone()),
            };
            assert_eq!(due.normalized_after(), Some(""), "{after:?}");
        }
    }

    #[test]
    fn a_position_reaches_the_index_as_the_page_spelled_it() {
        // The token is the index's own spelling, so normalization trims the
        // request's own padding and changes nothing else.
        let listed = ListGlossaryTermsParams {
            limit: 50,
            after: Some(" 7409".to_owned()),
        };
        assert_eq!(listed.normalized_after(), Some("7409"));
        let due = GlossaryDueParams {
            today: None,
            limit: 50,
            after: Some("7409".to_owned()),
        };
        assert_eq!(due.normalized_after(), Some("7409"));
    }

    #[test]
    fn params_default_to_no_position_when_the_caller_omits_one() {
        let listed: ListGlossaryTermsParams =
            serde_json::from_str("{}").expect("params default entirely");
        assert_eq!(listed.after, None);
        assert_eq!(listed.limit, default_search_limit());
        let due: GlossaryDueParams = serde_json::from_str("{}").expect("params default entirely");
        assert_eq!(due.after, None);
    }

    #[test]
    fn glossary_status_round_trips_as_text() {
        assert_eq!(GlossaryStatus::Stub.as_str(), "stub");
        assert_eq!(GlossaryStatus::Confirmed.as_str(), "confirmed");
        assert_eq!("stub".parse::<GlossaryStatus>(), Ok(GlossaryStatus::Stub));
        assert_eq!(
            "confirmed".parse::<GlossaryStatus>(),
            Ok(GlossaryStatus::Confirmed)
        );
        assert!("other".parse::<GlossaryStatus>().is_err());
        assert_eq!(GlossaryStatus::default(), GlossaryStatus::Stub);
    }
}
