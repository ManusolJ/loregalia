use crate::Result;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LOG_FILE: &str = "loregalia.log";
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

pub fn path_in(directory: &Path) -> PathBuf {
    directory.join(LOG_FILE)
}

pub fn append(path: &Path, message: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    rotate_if_full(path)?;

    let line = format!("{} {}\n", timestamp(SystemTime::now()), message);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())?;

    Ok(())
}

pub fn timestamp(time: SystemTime) -> String {
    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = duration.as_secs();
    let milliseconds = duration.subsec_millis();
    let days = (seconds / 86_400).min(i64::MAX as u64) as i64;
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milliseconds:03}Z")
}

fn rotate_if_full(path: &Path) -> Result<()> {
    let size = match std::fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
    };

    if size < MAX_LOG_BYTES {
        return Ok(());
    }

    std::fs::rename(path, rotated_path(path))?;

    Ok(())
}

fn rotated_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(".1");

    path.with_file_name(name)
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_position = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_position + 2) / 5 + 1;
    let month = if month_position < 10 {
        month_position + 3
    } else {
        month_position - 9
    };

    (
        if month <= 2 { year + 1 } else { year },
        month as u32,
        day as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::{MAX_LOG_BYTES, append, path_in, timestamp};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir().join(format!(
                "loregalia-log-{}-{nanos}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn the_epoch_is_iso_8601_utc() {
        assert_eq!(timestamp(UNIX_EPOCH), "1970-01-01T00:00:00.000Z".to_owned());
    }

    #[test]
    fn leap_days_and_milliseconds_are_formatted() {
        let time = UNIX_EPOCH + Duration::from_secs(1_709_210_096) + Duration::from_millis(789);

        assert_eq!(timestamp(time), "2024-02-29T12:34:56.789Z");
    }

    #[test]
    fn the_log_lives_in_the_data_folder() {
        assert_eq!(
            path_in(&PathBuf::from("/data")),
            PathBuf::from("/data/loregalia.log")
        );
    }

    #[test]
    fn appending_creates_folders_and_keeps_previous_lines() {
        let temporary = TempDir::new();
        let path = path_in(&temporary.path.join("nested"));

        append(&path, "first").unwrap();
        append(&path, "second").unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with(" first"));
        assert!(lines[1].ends_with(" second"));
        assert!(lines.iter().all(|line| line.contains("Z ")));
    }

    #[test]
    fn a_full_log_is_rotated_once() {
        let temporary = TempDir::new();
        let path = path_in(&temporary.path);
        std::fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize + 1]).unwrap();

        append(&path, "after rotation").unwrap();

        let rotated = temporary.path.join("loregalia.log.1");
        assert_eq!(std::fs::metadata(rotated).unwrap().len(), MAX_LOG_BYTES + 1);
        let contents = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents.lines().count(), 1);
        assert!(contents.ends_with(" after rotation\n"));
    }
}
