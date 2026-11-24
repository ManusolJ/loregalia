const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

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
    use super::short;

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
