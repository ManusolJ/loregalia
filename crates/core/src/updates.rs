use crate::{Database, Error, Result, http, log, settings};
use serde::Deserialize;
use std::time::{Duration, SystemTime};

pub const RELEASES_PAGE: &str = "https://github.com/ManusolJ/loregalia/releases/latest";

const LATEST_RELEASE: &str = "https://api.github.com/repos/ManusolJ/loregalia/releases/latest";
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const CHECK_EVERY: Duration = Duration::from_secs(86_400);
const CHECKED_AT_KEY: &str = "update_checked_at";
const LATEST_KEY: &str = "update_latest";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.strip_prefix('v').unwrap_or(text).split('.');
        let version = Self {
            major: number(parts.next()?)?,
            minor: number(parts.next()?)?,
            patch: number(parts.next()?)?,
        };

        parts.next().is_none().then_some(version)
    }

    pub fn current() -> Option<Self> {
        Self::parse(env!("CARGO_PKG_VERSION"))
    }

    pub fn is_newer_than_this_one(self) -> bool {
        Self::current().is_some_and(|current| self > current)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Known {
    pub enabled: bool,
    pub due: bool,
    pub latest: Option<Version>,
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

pub fn known(database: &Database, now: SystemTime) -> Result<Known> {
    let checked_at = settings::get(database, CHECKED_AT_KEY)?;

    Ok(Known {
        enabled: settings::update_checks(database)?,
        due: is_due(checked_at.as_deref(), now),
        latest: settings::get(database, LATEST_KEY)?
            .as_deref()
            .and_then(Version::parse),
    })
}

pub fn remember(database: &Database, latest: Option<Version>, now: SystemTime) -> Result<()> {
    let latest = latest
        .map(|version| version.to_string())
        .unwrap_or_default();
    settings::set(database, LATEST_KEY, &latest)?;

    settings::set(database, CHECKED_AT_KEY, &log::timestamp(now))
}

pub fn latest() -> Result<Option<Version>> {
    latest_from(http::get_secure_bytes(
        LATEST_RELEASE,
        http::TIMEOUT,
        MAX_RESPONSE_BYTES,
    ))
}

fn latest_from(response: Result<Vec<u8>>) -> Result<Option<Version>> {
    match response {
        Ok(body) => parse_latest(&body),
        Err(Error::HttpStatus { code: 404 }) => Ok(None),
        Err(error) => Err(error),
    }
}

fn parse_latest(body: &[u8]) -> Result<Option<Version>> {
    let release: ApiRelease = serde_json::from_slice(body).map_err(Error::Json)?;
    if release.draft || release.prerelease {
        return Ok(None);
    }

    Ok(Version::parse(&release.tag_name))
}

fn is_due(checked_at: Option<&str>, now: SystemTime) -> bool {
    let Some(checked_at) = checked_at else {
        return true;
    };
    let oldest = now
        .checked_sub(CHECK_EVERY)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    checked_at <= log::timestamp(oldest).as_str() || checked_at > log::timestamp(now).as_str()
}

fn number(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    text.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{Version, known, latest_from, remember};
    use crate::{Database, Error, settings};
    use std::time::{Duration, SystemTime};

    const FIXTURE: &str = include_str!("../tests/fixtures/github/latest-release.json");

    fn version(text: &str) -> Version {
        Version::parse(text).unwrap()
    }

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(1_795_000_000 + seconds)
    }

    #[test]
    fn versions_are_compared_by_number_not_by_text() {
        assert!(version("1.10.0") > version("1.9.9"));
        assert!(version("2.0.0") > version("1.99.99"));
        assert!(version("1.4.10") > version("1.4.9"));
        assert_eq!(version("v1.4.0"), version("1.4.0"));
        assert_eq!(version("v1.4.0").to_string(), "1.4.0");
    }

    #[test]
    fn anything_that_is_not_a_plain_version_is_ignored() {
        for text in [
            "",
            "v",
            "1",
            "1.4",
            "1.4.0.1",
            "1.5.0-rc.1",
            "1.+5.0",
            "1..0",
            "latest",
            "1.4.99999999999999999999999",
        ] {
            assert_eq!(Version::parse(text), None, "{text}");
        }
    }

    #[test]
    fn only_a_later_version_is_newer_than_this_one() {
        let current = Version::current().unwrap();
        let next = Version {
            patch: current.patch + 1,
            ..current
        };

        assert!(next.is_newer_than_this_one());
        assert!(!current.is_newer_than_this_one());
        assert!(!version("0.0.1").is_newer_than_this_one());
    }

    #[test]
    fn the_real_response_names_the_latest_version() {
        let latest = latest_from(Ok(FIXTURE.as_bytes().to_vec())).unwrap();

        assert_eq!(latest, Some(version("1.4.0")));
    }

    #[test]
    fn drafts_previews_and_odd_tags_are_not_offered() {
        for body in [
            r#"{"tag_name":"v9.0.0","draft":true}"#,
            r#"{"tag_name":"v9.0.0","prerelease":true}"#,
            r#"{"tag_name":"nightly"}"#,
        ] {
            assert_eq!(latest_from(Ok(body.into())).unwrap(), None, "{body}");
        }
    }

    #[test]
    fn no_release_is_not_a_failure_but_everything_else_is() {
        let status = |code| latest_from(Err(Error::HttpStatus { code }));

        assert_eq!(status(404).unwrap(), None);
        assert!(status(500).is_err());
        assert!(latest_from(Err(Error::ConnectionFailed)).is_err());
        assert!(latest_from(Err(Error::RateLimited)).is_err());
        assert!(latest_from(Ok(b"<html>".to_vec())).is_err());
    }

    #[test]
    fn a_new_library_is_due_and_checks_by_default() {
        let database = Database::open_in_memory().unwrap();
        let known = known(&database, at(0)).unwrap();

        assert!(known.enabled);
        assert!(known.due);
        assert_eq!(known.latest, None);
    }

    #[test]
    fn a_check_is_remembered_and_repeated_after_a_day() {
        let database = Database::open_in_memory().unwrap();
        remember(&database, Some(version("9.1.0")), at(0)).unwrap();
        let due = |seconds| known(&database, at(seconds)).unwrap().due;

        assert_eq!(
            known(&database, at(1)).unwrap().latest,
            Some(version("9.1.0"))
        );
        assert!(!due(0));
        assert!(!due(86_399));
        assert!(due(86_400));
    }

    #[test]
    fn a_clock_set_back_does_not_stop_the_checks() {
        let database = Database::open_in_memory().unwrap();
        remember(&database, None, at(1_000_000)).unwrap();

        assert!(known(&database, at(0)).unwrap().due);
    }

    #[test]
    fn a_check_that_finds_nothing_forgets_the_older_answer() {
        let database = Database::open_in_memory().unwrap();
        remember(&database, Some(version("9.1.0")), at(0)).unwrap();
        remember(&database, None, at(10)).unwrap();

        assert_eq!(known(&database, at(20)).unwrap().latest, None);
    }

    #[test]
    fn checks_can_be_turned_off_and_on() {
        let database = Database::open_in_memory().unwrap();
        for enabled in [false, true] {
            settings::set_update_checks(&database, enabled).unwrap();
            assert_eq!(known(&database, at(0)).unwrap().enabled, enabled);
        }
    }
}
