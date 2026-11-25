use crate::log;
use std::time::SystemTime;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

pub fn today() -> String {
    rusqlite::Connection::open_in_memory()
        .and_then(|connection| {
            connection.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
        })
        .unwrap_or_else(|_| log::timestamp(SystemTime::now())[..10].to_owned())
}

pub fn days_between(from: &str, to: &str) -> Option<i64> {
    Some(day_number(to)? - day_number(from)?)
}

fn day_number(date: &str) -> Option<i64> {
    short(date)?;
    let year: i64 = date.get(..4)?.parse().ok()?;
    let month: i64 = date.get(5..7)?.parse().ok()?;
    let day: i64 = date.get(8..10)?.parse().ok()?;
    let shifted_year = if month > 2 { year } else { year - 1 };
    let shifted_month = (month + 9) % 12;
    let leaps = shifted_year / 4 - shifted_year / 100 + shifted_year / 400;

    Some(365 * shifted_year + leaps + (153 * shifted_month + 2) / 5 + day)
}

pub fn short(date: &str) -> Option<String> {
    let mut parts = date.get(..10)?.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: usize = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=31).contains(&day) {
        return None;
    }
    let month = MONTHS.get(month.checked_sub(1)?)?;

    Some(format!("{day} {month} {year}"))
}

#[cfg(test)]
mod tests {
    use super::{days_between, short, today};

    #[test]
    fn days_are_counted_across_months_years_and_leap_days() {
        assert_eq!(days_between("2026-11-25", "2026-11-25"), Some(0));
        assert_eq!(days_between("2026-11-25", "2026-11-28"), Some(3));
        assert_eq!(days_between("2026-11-28", "2026-11-25"), Some(-3));
        assert_eq!(days_between("2026-12-31", "2027-01-01"), Some(1));
        assert_eq!(days_between("2027-02-28", "2027-03-01"), Some(1));
        assert_eq!(days_between("2028-02-28", "2028-03-01"), Some(2));
        assert_eq!(days_between("2026-11-25", "2027-11-25"), Some(365));
        assert_eq!(days_between("soon", "2027-11-25"), None);
    }

    #[test]
    fn today_is_a_date() {
        assert!(short(&today()).is_some());
        assert_eq!(today().len(), 10);
    }

    #[test]
    fn a_date_reads_as_day_month_year() {
        assert_eq!(short("2026-11-24"), Some("24 Nov 2026".to_owned()));
        assert_eq!(short("2026-01-05"), Some("5 Jan 2026".to_owned()));
    }

    #[test]
    fn a_timestamp_keeps_only_its_date() {
        assert_eq!(
            short("2026-12-31T23:59:59.000Z"),
            Some("31 Dec 2026".to_owned())
        );
    }

    #[test]
    fn text_that_is_not_a_date_has_no_short_form() {
        for text in [
            "",
            "2026",
            "2026-13-01",
            "2026-00-01",
            "2026-01-00",
            "2026-01-32",
            "tomorrow!!",
            "año 2026-01",
        ] {
            assert_eq!(short(text), None, "{text}");
        }
    }
}
