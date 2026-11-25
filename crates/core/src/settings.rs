use crate::{Database, Error, Result};
use rusqlite::OptionalExtension;

const THEME_KEY: &str = "theme";
const LAYOUT_KEY: &str = "layout";
const UPDATE_CHECKS_KEY: &str = "update_checks";
const OFF: &str = "off";
const ON: &str = "on";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    System,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::System];

    pub fn key(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::System => "System",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|theme| theme.key() == value)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Layout {
    #[default]
    List,
    Grid,
}

impl Layout {
    pub const ALL: [Self; 2] = [Self::List, Self::Grid];

    pub fn key(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Grid => "grid",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::List => "List",
            Self::Grid => "Grid",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| layout.key() == value)
    }
}

pub fn get(database: &Database, key: &str) -> Result<Option<String>> {
    database
        .connection()
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(Error::from)
}

pub fn set(database: &Database, key: &str, value: &str) -> Result<()> {
    database.connection().execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;

    Ok(())
}

pub fn theme(database: &Database) -> Result<Theme> {
    Ok(get(database, THEME_KEY)?
        .and_then(|value| Theme::from_key(&value))
        .unwrap_or_default())
}

pub fn set_theme(database: &Database, theme: Theme) -> Result<()> {
    set(database, THEME_KEY, theme.key())
}

pub fn layout(database: &Database) -> Result<Layout> {
    Ok(get(database, LAYOUT_KEY)?
        .and_then(|value| Layout::from_key(&value))
        .unwrap_or_default())
}

pub fn set_layout(database: &Database, layout: Layout) -> Result<()> {
    set(database, LAYOUT_KEY, layout.key())
}

pub fn update_checks(database: &Database) -> Result<bool> {
    Ok(get(database, UPDATE_CHECKS_KEY)?.as_deref() != Some(OFF))
}

pub fn set_update_checks(database: &Database, enabled: bool) -> Result<()> {
    set(database, UPDATE_CHECKS_KEY, if enabled { ON } else { OFF })
}

#[cfg(test)]
mod tests {
    use super::{Layout, Theme, get, layout, set, set_layout, set_theme, theme};
    use crate::Database;

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn a_fresh_database_defaults_to_the_dark_theme() {
        assert_eq!(theme(&database()).unwrap(), Theme::Dark);
    }

    #[test]
    fn every_theme_round_trips() {
        for expected in Theme::ALL {
            let database = database();
            set_theme(&database, expected).unwrap();

            assert_eq!(theme(&database).unwrap(), expected);
        }
    }

    #[test]
    fn setting_a_key_twice_replaces_the_value() {
        let database = database();
        set(&database, "theme", "light").unwrap();
        set(&database, "theme", "system").unwrap();

        assert_eq!(get(&database, "theme").unwrap(), Some("system".to_owned()));
    }

    #[test]
    fn an_unknown_theme_value_falls_back_to_dark() {
        let database = database();
        set(&database, "theme", "neon").unwrap();

        assert_eq!(theme(&database).unwrap(), Theme::Dark);
    }

    #[test]
    fn a_fresh_database_defaults_to_the_list_layout() {
        assert_eq!(layout(&database()).unwrap(), Layout::List);
    }

    #[test]
    fn every_layout_round_trips() {
        for expected in Layout::ALL {
            let database = database();
            set_layout(&database, expected).unwrap();

            assert_eq!(layout(&database).unwrap(), expected);
        }
    }

    #[test]
    fn an_unknown_layout_value_falls_back_to_the_list() {
        let database = database();
        set(&database, "layout", "spiral").unwrap();

        assert_eq!(layout(&database).unwrap(), Layout::List);
    }

    #[test]
    fn an_unknown_key_has_no_value() {
        assert_eq!(get(&database(), "missing").unwrap(), None);
    }
}
