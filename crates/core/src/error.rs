#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    SchemaTooNew { found: i64, supported: u32 },
    DataDirectoryUnavailable,
    UnknownStatus,
    StatusKindMismatch,
    EmptyTagName,
    DuplicateTagName,
    UnknownShow,
    DuplicateLookup,
    Unauthorized,
    RelayUnavailable,
    SeasonsOnlyForShows,
    DuplicateSeason,
    InvalidSeasonNumber,
    ConnectionFailed,
    RateLimited,
    HttpStatus { code: u16 },
    Json(serde_json::Error),
    SecretStore(String),
    BlockedAddress,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn is_temporary(&self) -> bool {
        matches!(
            self,
            Self::ConnectionFailed
                | Self::RateLimited
                | Self::RelayUnavailable
                | Self::HttpStatus { code: 500..=599 }
        )
    }
}

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
            Self::UnknownStatus => formatter.write_str("the status does not exist"),
            Self::StatusKindMismatch => {
                formatter.write_str("the status belongs to another media kind")
            }
            Self::EmptyTagName => formatter.write_str("a tag needs a name"),
            Self::DuplicateTagName => formatter.write_str("another tag already has that name"),
            Self::UnknownShow => formatter.write_str("the show does not exist"),
            Self::DuplicateLookup => {
                formatter.write_str("an item from that source is already in the library")
            }
            Self::Unauthorized => {
                formatter.write_str("the metadata source rejected the credentials")
            }
            Self::RelayUnavailable => formatter
                .write_str("the Loregalia metadata service is unavailable; try again later"),
            Self::SeasonsOnlyForShows => formatter.write_str("seasons belong only to shows"),
            Self::DuplicateSeason => formatter.write_str("the show already has that season"),
            Self::InvalidSeasonNumber => formatter.write_str("a season number cannot be negative"),
            Self::ConnectionFailed => {
                formatter.write_str("could not connect to the metadata server or it timed out")
            }
            Self::RateLimited => formatter.write_str(
                "the metadata server is rate limiting requests (HTTP 429); try again later",
            ),
            Self::HttpStatus { code } => {
                write!(formatter, "the metadata server returned HTTP {code}")
            }
            Self::Json(error) => write!(formatter, "invalid response: {error}"),
            Self::SecretStore(reason) => write!(formatter, "key storage error: {reason}"),
            Self::BlockedAddress => {
                formatter.write_str("the cover is not on a metadata source's image server")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::SchemaTooNew { .. }
            | Self::DataDirectoryUnavailable
            | Self::UnknownStatus
            | Self::StatusKindMismatch
            | Self::EmptyTagName
            | Self::DuplicateTagName
            | Self::UnknownShow
            | Self::DuplicateLookup
            | Self::Unauthorized
            | Self::RelayUnavailable
            | Self::SeasonsOnlyForShows
            | Self::DuplicateSeason
            | Self::InvalidSeasonNumber
            | Self::ConnectionFailed
            | Self::RateLimited
            | Self::HttpStatus { .. }
            | Self::SecretStore(_)
            | Self::BlockedAddress => None,
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

#[cfg(test)]
mod tests {
    use super::Error;

    #[test]
    fn only_failures_that_can_pass_are_temporary() {
        for error in [
            Error::ConnectionFailed,
            Error::RateLimited,
            Error::HttpStatus { code: 503 },
        ] {
            assert!(error.is_temporary(), "{error}");
        }
        for error in [
            Error::HttpStatus { code: 404 },
            Error::BlockedAddress,
            Error::Unauthorized,
        ] {
            assert!(!error.is_temporary(), "{error}");
        }
    }
}
