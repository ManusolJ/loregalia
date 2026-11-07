use crate::{Database, Error, Result};
use rusqlite::OptionalExtension;

const THEME_KEY: &str = "theme";

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

#[cfg(test)]
mod tests {
    use super::{Theme, get, set, set_theme, theme};
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
    fn an_unknown_key_has_no_value() {
        assert_eq!(get(&database(), "missing").unwrap(), None);
    }
}
