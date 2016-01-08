
use chrono::{DateTime, TimeZone, UTC};

/// Format the duration between "now" and "then" into a relative time. A duration of over 30 days
/// will be rendered as the Y-m-d HH:MM; from 1 to 30 days will be rendered as Nd HH:MM:SS; under 1
/// day will be rendered as HH:MM:SS.
//pub fn relative_duration_format<T: TimeZone>(now: &DateTime<T>, then: &DateTime<T>) -> String {
pub fn relative_duration_format(now: &DateTime<UTC>, then: &DateTime<UTC>) -> String {
    let dur = *now - *then;
    match dur.num_days() {
        days if days > 30 => {
            then.format("%Y-%m-%d %H:%M").to_string()
        },
        days if days > 0 => {
            let h = dur.num_hours() % 24;
            let m = dur.num_minutes() % 60;
            let s = dur.num_seconds() % 60;
            format!("{}d {:02}:{:02}:{:02}", days, h, m, s)
        },
        _ => {
            let h = dur.num_hours() % 24;
            let m = dur.num_minutes() % 60;
            let s = dur.num_seconds() % 60;
            format!("{:02}:{:02}:{:02}", h, m, s)
        },
    }
}
