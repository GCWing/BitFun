use chrono::{DateTime, Duration, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

pub(super) fn parse_time_zone(value: Option<&str>) -> Result<Tz, String> {
    let value = value.map(str::trim).filter(|value| !value.is_empty());
    match value {
        Some(value) => value
            .parse::<Tz>()
            .map_err(|error| format!("Invalid token usage time zone '{value}': {error}")),
        None => Ok(chrono_tz::UTC),
    }
}

pub(super) fn local_date_start_utc(
    date: NaiveDate,
    time_zone: Tz,
) -> Result<DateTime<Utc>, String> {
    let midnight = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| format!("Invalid local date boundary: {date}"))?;

    // A few zones transition at midnight. Choose the earliest occurrence when
    // the boundary is ambiguous, or the first valid local instant when the
    // nominal midnight was skipped.
    for minutes_after_midnight in 0..=24 * 60 {
        let candidate = midnight
            .checked_add_signed(Duration::minutes(minutes_after_midnight))
            .ok_or_else(|| format!("Local date boundary overflowed for {date}"))?;
        match time_zone.from_local_datetime(&candidate) {
            LocalResult::Single(value) => return Ok(value.with_timezone(&Utc)),
            LocalResult::Ambiguous(first, second) => {
                return Ok(first.min(second).with_timezone(&Utc));
            }
            LocalResult::None => {}
        }
    }

    Err(format!(
        "Unable to resolve local date boundary for {date} in {time_zone}"
    ))
}
