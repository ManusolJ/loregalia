use rusqlite::{Connection, OptionalExtension, params};
use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CREATE_TABLE: &str = "CREATE TABLE IF NOT EXISTS cache (
    key TEXT PRIMARY KEY,
    body TEXT NOT NULL,
    expires_at INTEGER NOT NULL
) STRICT;";

pub struct Cache {
    connection: Mutex<Connection>,
}

impl Cache {
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        Self::from_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, CacheError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(connection: Connection) -> Result<Self, CacheError> {
        connection.execute_batch(CREATE_TABLE)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn get(&self, key: &str, now: i64) -> Result<Option<String>, CacheError> {
        let connection = self.connection.lock().map_err(|_| CacheError::Poisoned)?;
        connection
            .query_row(
                "SELECT body FROM cache WHERE key = ?1 AND expires_at > ?2",
                params![key, now],
                |row| row.get(0),
            )
            .optional()
            .map_err(CacheError::from)
    }

    pub fn put(
        &self,
        key: &str,
        body: &str,
        expires_at: i64,
        now: i64,
        max_entries: usize,
    ) -> Result<(), CacheError> {
        let connection = self.connection.lock().map_err(|_| CacheError::Poisoned)?;
        connection.execute(
            "INSERT INTO cache (key, body, expires_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET body = excluded.body, expires_at = excluded.expires_at",
            params![key, body, expires_at],
        )?;
        prune(&connection, now, max_entries).map_err(CacheError::from)
    }

    pub fn prune(&self, now: i64, max_entries: usize) -> Result<(), CacheError> {
        let connection = self.connection.lock().map_err(|_| CacheError::Poisoned)?;
        prune(&connection, now, max_entries).map_err(CacheError::from)
    }

    pub fn count(&self) -> Result<usize, CacheError> {
        let connection = self.connection.lock().map_err(|_| CacheError::Poisoned)?;
        let count: i64 =
            connection.query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))?;
        Ok(count as usize)
    }
}

fn prune(connection: &Connection, now: i64, max_entries: usize) -> rusqlite::Result<()> {
    connection.execute("DELETE FROM cache WHERE expires_at <= ?1", [now])?;

    if max_entries == 0 {
        return Ok(());
    }

    let count: i64 = connection.query_row("SELECT COUNT(*) FROM cache", [], |row| row.get(0))?;
    let excess = count - max_entries as i64;
    if excess > 0 {
        connection.execute(
            "DELETE FROM cache WHERE key IN (
                SELECT key FROM cache ORDER BY expires_at, rowid LIMIT ?1
            )",
            [excess],
        )?;
    }

    Ok(())
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug)]
pub enum CacheError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Poisoned,
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "cache i/o error: {error}"),
            Self::Sqlite(error) => write!(formatter, "cache database error: {error}"),
            Self::Poisoned => formatter.write_str("cache lock was poisoned"),
        }
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::Poisoned => None,
        }
    }
}

impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for CacheError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{Cache, now};
    use tempfile::TempDir;

    #[test]
    fn a_missing_key_has_no_value() {
        let cache = Cache::open_in_memory().unwrap();

        assert_eq!(cache.get("missing", 100).unwrap(), None);
    }

    #[test]
    fn a_fresh_value_round_trips() {
        let cache = Cache::open_in_memory().unwrap();
        cache.put("key", "body", 200, 100, 100).unwrap();

        assert_eq!(cache.get("key", 100).unwrap(), Some("body".to_owned()));
    }

    #[test]
    fn an_expired_value_is_not_returned() {
        let cache = Cache::open_in_memory().unwrap();
        cache.put("key", "body", 100, 50, 100).unwrap();

        assert_eq!(cache.get("key", 100).unwrap(), None);
    }

    #[test]
    fn putting_the_same_key_replaces_the_body() {
        let cache = Cache::open_in_memory().unwrap();
        cache.put("key", "first", 200, 100, 100).unwrap();
        cache.put("key", "second", 300, 100, 100).unwrap();

        assert_eq!(cache.get("key", 100).unwrap(), Some("second".to_owned()));
        assert_eq!(cache.count().unwrap(), 1);
    }

    #[test]
    fn the_cache_evicts_the_value_that_expires_first() {
        let cache = Cache::open_in_memory().unwrap();
        cache.put("first", "one", 110, 100, 2).unwrap();
        cache.put("second", "two", 120, 100, 2).unwrap();
        cache.put("third", "three", 130, 100, 2).unwrap();

        assert_eq!(cache.count().unwrap(), 2);
        assert_eq!(cache.get("first", 100).unwrap(), None);
        assert_eq!(cache.get("second", 100).unwrap(), Some("two".to_owned()));
        assert_eq!(cache.get("third", 100).unwrap(), Some("three".to_owned()));
    }

    #[test]
    fn pruning_removes_expired_values() {
        let cache = Cache::open_in_memory().unwrap();
        cache.put("expired", "old", 100, 50, 100).unwrap();
        cache.put("fresh", "new", 300, 100, 100).unwrap();

        cache.prune(100, 100).unwrap();

        assert_eq!(cache.count().unwrap(), 1);
        assert_eq!(cache.get("expired", 100).unwrap(), None);
    }

    #[test]
    fn the_cache_survives_reopening() {
        let temporary = TempDir::new().unwrap();
        let path = temporary.path().join("relay-cache.db");

        {
            let cache = Cache::open(&path).unwrap();
            cache.put("key", "body", 200, 100, 100).unwrap();
        }

        let cache = Cache::open(&path).unwrap();
        assert_eq!(cache.get("key", 100).unwrap(), Some("body".to_owned()));
    }

    #[test]
    fn the_current_time_is_after_the_epoch() {
        assert!(now() > 0);
    }
}
