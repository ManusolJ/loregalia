#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    SchemaTooNew { found: i64, supported: u32 },
    DataDirectoryUnavailable,
    Network(String),
    Json(serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "storage i/o error: {error}"),
            Self::Sqlite(error) => write!(formatter, "database error: {error}"),
            Self::SchemaTooNew { found, supported } => write!(
                formatter,
                "database schema version {found} is newer than supported {supported}"
            ),
            Self::DataDirectoryUnavailable => formatter.write_str(
                "no application data folder is available; set APPDATA, XDG_DATA_HOME or HOME",
            ),
            Self::Network(message) => write!(formatter, "network error: {message}"),
            Self::Json(error) => write!(formatter, "invalid response: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::SchemaTooNew { .. } | Self::DataDirectoryUnavailable | Self::Network(_) => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}
