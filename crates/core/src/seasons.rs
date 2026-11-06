use crate::{Database, Error, Result};
use rusqlite::{OptionalExtension, Row, params};
use uuid::Uuid;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Season {
    pub id: String,
    pub show_id: String,
    pub season_number: i32,
    pub status_id: String,
    pub finished_at: Option<String>,
}

impl Season {
    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            show_id: row.get(1)?,
            season_number: row.get(2)?,
            status_id: row.get(3)?,
            finished_at: row.get(4)?,
        })
    }
}

pub fn create(database: &Database, show_id: &str, season_number: i32) -> Result<Season> {
    if season_number < 0 {
        return Err(Error::InvalidSeasonNumber);
    }

    let kind: Option<String> = database
        .connection()
        .query_row(
            "SELECT media_kind FROM items WHERE id = ?1",
            [show_id],
            |row| row.get(0),
        )
        .optional()?;
    match kind.as_deref() {
        None => return Err(Error::UnknownShow),
        Some("show") => {}
        Some(_) => return Err(Error::SeasonsOnlyForShows),
    }

    let exists: i64 = database.connection().query_row(
        "SELECT EXISTS(SELECT 1 FROM seasons WHERE show_id = ?1 AND season_number = ?2)",
        params![show_id, season_number],
        |row| row.get(0),
    )?;
    if exists != 0 {
        return Err(Error::DuplicateSeason);
    }

    let id = Uuid::now_v7().to_string();
    database
        .connection()
        .query_row(
            "INSERT INTO seasons (id, show_id, season_number, status_id)
             VALUES (?1, ?2, ?3,
                     (SELECT id FROM statuses
                      WHERE media_kind = 'show' AND category = 'planned'))
             RETURNING id, show_id, season_number, status_id, finished_at",
            params![id, show_id, season_number],
            Season::from_row,
        )
        .map_err(Error::from)
}

pub fn all(database: &Database, show_id: &str) -> Result<Vec<Season>> {
    let mut statement = database.connection().prepare(
        "SELECT id, show_id, season_number, status_id, finished_at
         FROM seasons WHERE show_id = ?1 ORDER BY season_number",
    )?;
    let rows = statement.query_map([show_id], Season::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

pub fn all_for_every_show(database: &Database) -> Result<Vec<Season>> {
    let mut statement = database.connection().prepare(
        "SELECT id, show_id, season_number, status_id, finished_at
         FROM seasons ORDER BY show_id, season_number",
    )?;
    let rows = statement.query_map([], Season::from_row)?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Error::from)
}

pub fn set_status(database: &Database, season_id: &str, status_id: &str) -> Result<bool> {
    let status_kind: Option<String> = database
        .connection()
        .query_row(
            "SELECT media_kind FROM statuses WHERE id = ?1",
            [status_id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(status_kind) = status_kind else {
        return Err(Error::UnknownStatus);
    };
    if status_kind != "show" {
        return Err(Error::StatusKindMismatch);
    }

    let changed = database.connection().execute(
        "UPDATE seasons
         SET status_id = ?1,
             finished_at = CASE
                 WHEN (SELECT category FROM statuses WHERE id = ?1) = 'done'
                 THEN COALESCE(finished_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ELSE NULL
             END
         WHERE id = ?2",
        params![status_id, season_id],
    )?;

    Ok(changed > 0)
}

pub fn delete(database: &Database, id: &str) -> Result<bool> {
    let changed = database
        .connection()
        .execute("DELETE FROM seasons WHERE id = ?1", [id])?;

    Ok(changed > 0)
}

#[cfg(test)]
mod tests {
    use super::{all, all_for_every_show, create, delete, set_status};
    use crate::media::MediaKind;
    use crate::{Database, Error, items};

    fn database() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn show(database: &Database, title: &str) -> items::Item {
        items::create(database, MediaKind::Show, title, None).unwrap()
    }

    #[test]
    fn a_created_season_round_trips() {
        let database = database();
        let show = show(&database, "Dark");

        let season = create(&database, &show.id, 1).unwrap();

        assert_eq!(season.show_id, show.id);
        assert_eq!(season.season_number, 1);
        assert_eq!(season.status_id, "show-planned");
        assert_eq!(season.finished_at, None);
        assert_eq!(all(&database, &show.id).unwrap(), vec![season]);
    }

    #[test]
    fn seasons_are_ordered_by_number() {
        let database = database();
        let show = show(&database, "Dark");
        create(&database, &show.id, 3).unwrap();
        create(&database, &show.id, 1).unwrap();
        create(&database, &show.id, 2).unwrap();

        let numbers: Vec<i32> = all(&database, &show.id)
            .unwrap()
            .into_iter()
            .map(|season| season.season_number)
            .collect();

        assert_eq!(numbers, [1, 2, 3]);
    }

    #[test]
    fn seasons_belong_only_to_shows() {
        let database = database();
        let movie = items::create(&database, MediaKind::Movie, "Dune", None).unwrap();

        assert!(matches!(
            create(&database, &movie.id, 1),
            Err(Error::SeasonsOnlyForShows)
        ));
        assert!(matches!(
            create(&database, "missing", 1),
            Err(Error::UnknownShow)
        ));
        assert!(all(&database, &movie.id).unwrap().is_empty());
    }

    #[test]
    fn a_show_cannot_have_two_seasons_with_the_same_number() {
        let database = database();
        let show = show(&database, "Dark");
        create(&database, &show.id, 1).unwrap();

        assert!(matches!(
            create(&database, &show.id, 1),
            Err(Error::DuplicateSeason)
        ));
        assert_eq!(all(&database, &show.id).unwrap().len(), 1);
    }

    #[test]
    fn a_negative_season_number_is_rejected() {
        let database = database();
        let show = show(&database, "Dark");

        assert!(matches!(
            create(&database, &show.id, -1),
            Err(Error::InvalidSeasonNumber)
        ));
    }

    #[test]
    fn season_zero_is_allowed_for_specials() {
        let database = database();
        let show = show(&database, "Dark");

        let season = create(&database, &show.id, 0).unwrap();

        assert_eq!(season.season_number, 0);
    }

    #[test]
    fn deleting_a_show_deletes_its_seasons() {
        let database = database();
        let show = show(&database, "Dark");
        create(&database, &show.id, 1).unwrap();
        create(&database, &show.id, 2).unwrap();

        items::delete(&database, &show.id).unwrap();

        assert!(all_for_every_show(&database).unwrap().is_empty());
    }

    #[test]
    fn deleting_a_season_removes_only_that_season() {
        let database = database();
        let show = show(&database, "Dark");
        let first = create(&database, &show.id, 1).unwrap();
        let second = create(&database, &show.id, 2).unwrap();

        assert!(delete(&database, &first.id).unwrap());
        assert!(!delete(&database, &first.id).unwrap());

        assert_eq!(all(&database, &show.id).unwrap(), vec![second]);
    }

    #[test]
    fn setting_a_season_status_records_and_clears_finished_at() {
        let database = database();
        let show = show(&database, "Dark");
        let season = create(&database, &show.id, 1).unwrap();

        assert!(set_status(&database, &season.id, "show-done").unwrap());

        let stored = all(&database, &show.id)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(stored.status_id, "show-done");
        assert!(stored.finished_at.unwrap().ends_with('Z'));

        assert!(set_status(&database, &season.id, "show-active").unwrap());
        assert_eq!(all(&database, &show.id).unwrap()[0].finished_at, None);
    }

    #[test]
    fn a_season_status_must_be_a_show_status() {
        let database = database();
        let show = show(&database, "Dark");
        let season = create(&database, &show.id, 1).unwrap();

        assert!(matches!(
            set_status(&database, &season.id, "movie-done"),
            Err(Error::StatusKindMismatch)
        ));
        assert!(matches!(
            set_status(&database, &season.id, "missing"),
            Err(Error::UnknownStatus)
        ));
        assert_eq!(
            all(&database, &show.id).unwrap()[0].status_id,
            "show-planned"
        );
    }

    #[test]
    fn setting_a_status_on_a_missing_season_reports_no_change() {
        let database = database();

        assert!(!set_status(&database, "missing", "show-done").unwrap());
    }

    #[test]
    fn all_for_every_show_groups_seasons_by_show() {
        let database = database();
        let first = show(&database, "First");
        let second = show(&database, "Second");
        create(&database, &first.id, 2).unwrap();
        create(&database, &first.id, 1).unwrap();
        create(&database, &second.id, 1).unwrap();

        let stored = all_for_every_show(&database).unwrap();
        let first_seasons: Vec<i32> = stored
            .iter()
            .filter(|season| season.show_id == first.id)
            .map(|season| season.season_number)
            .collect();

        assert_eq!(stored.len(), 3);
        assert_eq!(first_seasons, [1, 2]);
    }
}
