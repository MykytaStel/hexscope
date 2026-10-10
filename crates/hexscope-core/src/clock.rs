//! Dates from counts of seconds, without a calendar library.

/// Seconds since 1970-01-01 UTC as `2024-03-02 09:38 UTC`.
pub(crate) fn minute_utc(secs: i64) -> String {
    let [y, m, d, hh, mm, _] = civil(secs);
    format!("{y:04}-{m:02}-{d:02} {hh:02}:{mm:02} UTC")
}

/// Seconds since 1970-01-01 UTC as year, month, day, hour, minute, second.
pub(crate) fn civil(secs: i64) -> [i64; 6] {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's days-to-civil.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    [y, m, d, rem / 3600, rem / 60 % 60, rem % 60]
}
