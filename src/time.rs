// Trello timestamps are ISO 8601 UTC, e.g. "2024-01-15T09:30:00.000Z". We only
// ever see that one shape in exports, so this parser doesn't try to handle
// offsets other than Z or dates before the epoch.

pub fn parse_iso8601_utc(s: &str) -> Option<u64> {
    let s = s.trim();
    let s = s.strip_suffix('Z').unwrap_or(s);

    let mut top = s.splitn(2, 'T');
    let date_part = top.next()?;
    let time_part = top.next()?;

    let mut d = date_part.split('-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    let mut t = time_part.split(':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    let second_field = t.next()?;
    let second: i64 = second_field.split('.').next()?.parse().ok()?;

    let days = days_from_civil(year, month, day);
    let total = days * 86_400 + hour * 3_600 + minute * 60 + second;
    if total < 0 {
        None
    } else {
        Some(total as u64)
    }
}

// Howard Hinnant's days-from-civil algorithm: converts a proleptic Gregorian
// y/m/d into a day count relative to 1970-01-01, without floating point and
// without needing a calendar table.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = (m + 9) % 12; // [0, 11]
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

pub fn format_duration(total_seconds: i64) -> String {
    let seconds = if total_seconds < 0 { 0 } else { total_seconds };
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_epoch() {
        assert_eq!(parse_iso8601_utc("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn parses_known_date_with_millis() {
        // 2024-01-01T00:00:00Z is 1704067200; add 14 days plus 9:30:00.
        assert_eq!(
            parse_iso8601_utc("2024-01-15T09:30:00.000Z"),
            Some(1_705_311_000)
        );
    }

    #[test]
    fn parses_without_trailing_z() {
        assert_eq!(
            parse_iso8601_utc("2024-01-15T09:30:00"),
            Some(1_705_311_000)
        );
    }

    #[test]
    fn rejects_pre_epoch_dates() {
        assert_eq!(parse_iso8601_utc("1969-12-31T23:59:59Z"), None);
    }

    #[test]
    fn rejects_invalid_month_or_day() {
        assert_eq!(parse_iso8601_utc("2024-13-01T00:00:00Z"), None);
        assert_eq!(parse_iso8601_utc("2024-01-32T00:00:00Z"), None);
        assert_eq!(parse_iso8601_utc("2024-00-01T00:00:00Z"), None);
    }

    #[test]
    fn rejects_malformed_strings() {
        assert_eq!(parse_iso8601_utc("not a date"), None);
        assert_eq!(parse_iso8601_utc("2024-01-15"), None); // missing time part
        assert_eq!(parse_iso8601_utc(""), None);
    }

    #[test]
    fn format_duration_minutes_only() {
        assert_eq!(format_duration(0), "0m");
        assert_eq!(format_duration(59), "0m");
        assert_eq!(format_duration(60), "1m");
        assert_eq!(format_duration(45 * 60), "45m");
    }

    #[test]
    fn format_duration_hours_and_minutes() {
        assert_eq!(format_duration(3_600), "1h 0m");
        assert_eq!(format_duration(3_600 * 2 + 60 * 15), "2h 15m");
    }

    #[test]
    fn format_duration_days_and_hours() {
        assert_eq!(format_duration(86_400), "1d 0h");
        assert_eq!(format_duration(86_400 * 3 + 3_600 * 6), "3d 6h");
    }

    #[test]
    fn format_duration_clamps_negative() {
        assert_eq!(format_duration(-100), "0m");
    }
}
