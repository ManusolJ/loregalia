use crate::Result;
use crate::http;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const MAX_BYTES: u64 = 5 * 1024 * 1024;
const DAY_SECONDS: u64 = 24 * 60 * 60;
const TMDB_IMAGE_HOST: &str = "image.tmdb.org";
const TMDB_REFRESH_AFTER: Duration = Duration::from_secs(150 * DAY_SECONDS);
const TMDB_KEEP_AT_MOST: Duration = Duration::from_secs(180 * DAY_SECONDS);
const FILE_EXTENSION: &str = "cover";

pub const COVER_FOLDER: &str = "covers";
pub const MAX_CACHE_BYTES: u64 = 200 * 1024 * 1024;

pub fn fetch(url: &str) -> Result<Vec<u8>> {
    http::get_bytes(url, http::TIMEOUT, MAX_BYTES)
}

#[derive(Clone, Debug)]
pub struct CoverCache {
    directory: PathBuf,
    max_bytes: u64,
}

impl CoverCache {
    pub fn new(directory: PathBuf, max_bytes: u64) -> Self {
        Self {
            directory,
            max_bytes,
        }
    }

    pub fn in_data_folder(data_directory: &Path) -> Self {
        Self::new(data_directory.join(COVER_FOLDER), MAX_CACHE_BYTES)
    }

    pub fn fetch(
        &self,
        url: &str,
        now: SystemTime,
        download: &dyn Fn(&str) -> Result<Vec<u8>>,
    ) -> Result<Vec<u8>> {
        let path = self.path_for(url);
        let age = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .ok()
            .map(|modified| now.duration_since(modified).unwrap_or_default());

        if let Some(age) = age
            && age < refresh_after(url)
            && let Ok(bytes) = std::fs::read(&path)
        {
            return Ok(bytes);
        }

        match download(url) {
            Ok(bytes) => {
                self.store(&path, &bytes);
                Ok(bytes)
            }
            Err(error) => match age {
                Some(age) if age < keep_at_most(url) => std::fs::read(&path).map_err(|_| error),
                _ => Err(error),
            },
        }
    }

    pub fn keep_only<'a>(&self, urls: impl IntoIterator<Item = &'a str>) -> Result<usize> {
        let keep: HashSet<PathBuf> = urls.into_iter().map(|url| self.path_for(url)).collect();
        let mut removed = 0;
        for path in self.cached_files()? {
            if !keep.contains(&path) && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }

    fn store(&self, path: &Path, bytes: &[u8]) {
        if std::fs::create_dir_all(&self.directory).is_err() {
            return;
        }
        let partial = path.with_extension("partial");
        if std::fs::write(&partial, bytes).is_ok() && std::fs::rename(&partial, path).is_ok() {
            let _ = self.trim();
        } else {
            let _ = std::fs::remove_file(&partial);
        }
    }

    fn trim(&self) -> Result<()> {
        let mut files: Vec<(PathBuf, u64, SystemTime)> = self
            .cached_files()?
            .into_iter()
            .filter_map(|path| {
                let metadata = std::fs::metadata(&path).ok()?;
                Some((path, metadata.len(), metadata.modified().ok()?))
            })
            .collect();
        let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();
        if total <= self.max_bytes {
            return Ok(());
        }

        files.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, _) in files {
            if total <= self.max_bytes {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total = total.saturating_sub(size);
            }
        }
        Ok(())
    }

    fn cached_files(&self) -> Result<Vec<PathBuf>> {
        let entries = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        Ok(entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == FILE_EXTENSION)
            })
            .collect())
    }

    fn path_for(&self, url: &str) -> PathBuf {
        self.directory
            .join(format!("{:016x}.{FILE_EXTENSION}", fnv1a(url.as_bytes())))
    }
}

fn is_tmdb(url: &str) -> bool {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .is_some_and(|host| host.eq_ignore_ascii_case(TMDB_IMAGE_HOST))
}

fn refresh_after(url: &str) -> Duration {
    if is_tmdb(url) {
        TMDB_REFRESH_AFTER
    } else {
        Duration::MAX
    }
}

fn keep_at_most(url: &str) -> Duration {
    if is_tmdb(url) {
        TMDB_KEEP_AT_MOST
    } else {
        Duration::MAX
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::{CoverCache, DAY_SECONDS, fetch, fnv1a};
    use crate::Error;
    use crate::http::test_server::{refuse, serve};
    use std::cell::Cell;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    const DAY: Duration = Duration::from_secs(DAY_SECONDS);

    const TVMAZE: &str = "https://static.tvmaze.com/uploads/images/medium_portrait/1/1.jpg";
    const TMDB: &str = "https://image.tmdb.org/t/p/w342/dune.jpg";

    fn cache(temporary: &TempDir, max_bytes: u64) -> CoverCache {
        CoverCache::new(temporary.path().join("covers"), max_bytes)
    }

    #[test]
    fn fetch_returns_the_body_bytes() {
        let base = serve("200 OK", b"image-bytes");

        assert_eq!(fetch(&base).unwrap(), b"image-bytes");
    }

    #[test]
    fn http_404_reports_its_status_code() {
        let base = serve("404 Not Found", b"");

        assert!(matches!(fetch(&base), Err(Error::HttpStatus { code: 404 })));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        assert!(matches!(fetch(&refuse()), Err(Error::ConnectionFailed)));
    }

    #[test]
    fn a_cover_is_downloaded_once_and_then_read_from_disk() {
        let temporary = TempDir::new().unwrap();
        let cache = cache(&temporary, 1_000_000);
        let downloads = Cell::new(0);
        let download = |_: &str| {
            downloads.set(downloads.get() + 1);
            Ok(b"cover".to_vec())
        };

        assert_eq!(
            cache.fetch(TVMAZE, SystemTime::now(), &download).unwrap(),
            b"cover"
        );
        assert_eq!(
            cache.fetch(TVMAZE, SystemTime::now(), &download).unwrap(),
            b"cover"
        );
        assert_eq!(downloads.get(), 1);
    }

    #[test]
    fn a_cached_cover_still_shows_offline() {
        let temporary = TempDir::new().unwrap();
        let cache = cache(&temporary, 1_000_000);
        cache
            .fetch(TVMAZE, SystemTime::now(), &|_| Ok(b"cover".to_vec()))
            .unwrap();

        let offline = |_: &str| Err(Error::ConnectionFailed);

        assert_eq!(
            cache.fetch(TVMAZE, SystemTime::now(), &offline).unwrap(),
            b"cover"
        );
    }

    #[test]
    fn a_tmdb_cover_is_refreshed_before_six_months() {
        let temporary = TempDir::new().unwrap();
        let cache = cache(&temporary, 1_000_000);
        cache
            .fetch(TMDB, SystemTime::now(), &|_| Ok(b"old".to_vec()))
            .unwrap();
        let later = SystemTime::now() + DAY * 151;

        assert_eq!(
            cache.fetch(TMDB, later, &|_| Ok(b"new".to_vec())).unwrap(),
            b"new"
        );
    }

    #[test]
    fn an_old_tmdb_cover_is_shown_offline_only_within_six_months() {
        let temporary = TempDir::new().unwrap();
        let cache = cache(&temporary, 1_000_000);
        cache
            .fetch(TMDB, SystemTime::now(), &|_| Ok(b"old".to_vec()))
            .unwrap();
        let offline = |_: &str| Err(Error::ConnectionFailed);

        assert_eq!(
            cache
                .fetch(TMDB, SystemTime::now() + DAY * 160, &offline)
                .unwrap(),
            b"old"
        );
        assert!(
            cache
                .fetch(TMDB, SystemTime::now() + DAY * 200, &offline)
                .is_err()
        );
    }

    #[test]
    fn a_failed_download_is_not_cached() {
        let temporary = TempDir::new().unwrap();
        let cache = cache(&temporary, 1_000_000);

        assert!(
            cache
                .fetch(TVMAZE, SystemTime::now(), &|_| Err(Error::ConnectionFailed))
                .is_err()
        );
        assert_eq!(cache.keep_only([]).unwrap(), 0);
    }

    #[test]
    fn the_oldest_covers_go_first_when_the_cache_is_full() {
        let temporary = TempDir::new().unwrap();
        let cache = cache(&temporary, 10);
        let urls = ["https://a.test/1", "https://a.test/2", "https://a.test/3"];
        for url in urls {
            cache
                .fetch(url, SystemTime::now(), &|_| Ok(vec![0; 4]))
                .unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let offline = |_: &str| Err(Error::ConnectionFailed);

        assert!(cache.fetch(urls[0], SystemTime::now(), &offline).is_err());
        assert!(cache.fetch(urls[1], SystemTime::now(), &offline).is_ok());
        assert!(cache.fetch(urls[2], SystemTime::now(), &offline).is_ok());
    }

    #[test]
    fn covers_no_longer_in_the_library_are_removed() {
        let temporary = TempDir::new().unwrap();
        let cache = cache(&temporary, 1_000_000);
        for url in [TVMAZE, TMDB] {
            cache
                .fetch(url, SystemTime::now(), &|_| Ok(b"cover".to_vec()))
                .unwrap();
        }

        assert_eq!(cache.keep_only([TVMAZE]).unwrap(), 1);
        let offline = |_: &str| Err(Error::ConnectionFailed);
        assert!(cache.fetch(TVMAZE, SystemTime::now(), &offline).is_ok());
        assert!(cache.fetch(TMDB, SystemTime::now(), &offline).is_err());
    }

    #[test]
    fn file_names_are_stable_across_versions() {
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
