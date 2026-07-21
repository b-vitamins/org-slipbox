pub(crate) mod query;
pub(crate) mod write;

/// Resolve the reference date for glossary scheduling.
///
/// A caller-supplied ISO `YYYY-MM-DD` value wins; otherwise the daemon's local
/// date is used so due-selection and grading share one clock.
pub(crate) fn glossary_today(today: Option<String>) -> String {
    today.unwrap_or_else(|| {
        chrono::Local::now()
            .date_naive()
            .format("%Y-%m-%d")
            .to_string()
    })
}
