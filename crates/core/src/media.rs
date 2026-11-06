#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MediaKind {
    Movie,
    Show,
    Book,
    Game,
}

impl MediaKind {
    pub const ALL: [Self; 4] = [Self::Movie, Self::Show, Self::Book, Self::Game];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Movie => "Movie",
            Self::Show => "Show",
            Self::Book => "Book",
            Self::Game => "Game",
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Show => "show",
            Self::Book => "book",
            Self::Game => "game",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }

    pub const fn tracks_seasons(self) -> bool {
        matches!(self, Self::Show)
    }
}

#[cfg(test)]
mod tests {
    use super::MediaKind;

    #[test]
    fn only_shows_track_seasons() {
        let tracked: Vec<_> = MediaKind::ALL
            .into_iter()
            .filter(|kind| kind.tracks_seasons())
            .collect();

        assert_eq!(tracked, [MediaKind::Show]);
    }

    #[test]
    fn every_kind_has_a_label() {
        for kind in MediaKind::ALL {
            assert!(!kind.label().is_empty());
        }
    }

    #[test]
    fn every_kind_has_a_stable_key() {
        for kind in MediaKind::ALL {
            assert_eq!(MediaKind::from_key(kind.key()), Some(kind));
        }
        assert_eq!(MediaKind::from_key("unknown"), None);
    }
}
