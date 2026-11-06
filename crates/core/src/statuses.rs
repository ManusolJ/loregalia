#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Category {
    Planned,
    Active,
    Paused,
    Done,
    Dropped,
}

impl Category {
    pub const ALL: [Self; 5] = [
        Self::Planned,
        Self::Active,
        Self::Paused,
        Self::Done,
        Self::Dropped,
    ];

    pub const fn key(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Done => "done",
            Self::Dropped => "dropped",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|category| category.key() == key)
    }
}

#[cfg(test)]
mod tests {
    use super::Category;

    #[test]
    fn every_category_has_a_stable_key() {
        for category in Category::ALL {
            assert_eq!(Category::from_key(category.key()), Some(category));
        }
    }

    #[test]
    fn the_five_categories_are_planned_active_paused_done_dropped() {
        assert_eq!(
            Category::ALL.map(Category::key),
            ["planned", "active", "paused", "done", "dropped"]
        );
    }

    #[test]
    fn an_unknown_category_key_is_rejected() {
        assert_eq!(Category::from_key("someday"), None);
    }
}
