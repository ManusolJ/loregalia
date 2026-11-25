use crate::dates;
use crate::media::MediaKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct Details {
    pub synopsis: Option<String>,
    pub minutes: Option<u32>,
    pub pages: Option<u32>,
    pub genres: Vec<String>,
    pub creators: Vec<String>,
    pub release_date: Option<String>,
}

impl Details {
    pub fn length(&self, kind: MediaKind) -> Option<String> {
        match kind {
            MediaKind::Book => self.pages.map(|pages| format!("{pages} pages")),
            MediaKind::Movie => self.minutes.map(clock),
            MediaKind::Show => self
                .minutes
                .map(|minutes| format!("{} an episode", clock(minutes))),
            MediaKind::Game => self.minutes.map(play_time),
        }
    }
}

pub const fn creator_label(kind: MediaKind) -> &'static str {
    match kind {
        MediaKind::Movie => "Director",
        MediaKind::Show => "Network",
        MediaKind::Book => "Author",
        MediaKind::Game => "Developer",
    }
}

pub fn release(date: &str, today: &str) -> Option<String> {
    let shown = dates::short(date)?;

    Some(if date > today {
        format!("Coming {shown}")
    } else {
        shown
    })
}

pub(crate) fn clock(minutes: u32) -> String {
    match (minutes / 60, minutes % 60) {
        (0, minutes) => format!("{minutes} min"),
        (hours, 0) => format!("{hours} h"),
        (hours, minutes) => format!("{hours} h {minutes} min"),
    }
}

fn play_time(minutes: u32) -> String {
    if minutes < 60 {
        return format!("About {minutes} min");
    }
    format!("About {} h", (minutes + 30) / 60)
}

#[cfg(test)]
mod tests {
    use super::{Details, creator_label, release};
    use crate::media::MediaKind;

    fn minutes(minutes: u32) -> Details {
        Details {
            minutes: Some(minutes),
            ..Details::default()
        }
    }

    #[test]
    fn length_reads_naturally_for_each_kind() {
        let length = |kind, value| minutes(value).length(kind);

        assert_eq!(length(MediaKind::Movie, 155).as_deref(), Some("2 h 35 min"));
        assert_eq!(length(MediaKind::Movie, 120).as_deref(), Some("2 h"));
        assert_eq!(length(MediaKind::Movie, 45).as_deref(), Some("45 min"));
        assert_eq!(
            length(MediaKind::Show, 49).as_deref(),
            Some("49 min an episode")
        );
        assert_eq!(
            length(MediaKind::Game, 1_310).as_deref(),
            Some("About 22 h")
        );
        assert_eq!(length(MediaKind::Game, 40).as_deref(), Some("About 40 min"));
        assert_eq!(length(MediaKind::Book, 90), None);
        let book = Details {
            pages: Some(272),
            ..Details::default()
        };
        assert_eq!(book.length(MediaKind::Book).as_deref(), Some("272 pages"));
    }

    #[test]
    fn a_future_release_says_it_is_coming() {
        let today = "2026-11-24";

        assert_eq!(release("2021-09-15", today).as_deref(), Some("15 Sep 2021"));
        assert_eq!(release("2026-11-24", today).as_deref(), Some("24 Nov 2026"));
        assert_eq!(
            release("2027-03-05", today).as_deref(),
            Some("Coming 5 Mar 2027")
        );
        assert_eq!(release("soon", today), None);
    }

    #[test]
    fn every_kind_names_its_creator() {
        let labels = MediaKind::ALL.map(creator_label);

        assert_eq!(labels.len(), 4);
        assert_eq!(creator_label(MediaKind::Book), "Author");
    }

    #[test]
    fn details_saved_by_another_version_still_load() {
        let details: Details =
            serde_json::from_str(r#"{"minutes":90,"something_new":{"a":1}}"#).unwrap();

        assert_eq!(details, minutes(90));
    }
}
