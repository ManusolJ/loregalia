use crate::details::{Details, NextSeason};
use crate::http;
use crate::providers::{Candidate, Source, full_date, numeric, positive, year_of};
use crate::{Result, text};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://api.tvmaze.com";
const ENDED: &str = "Ended";
const SEARCH_PATH: &str = "/search/shows";

#[derive(Deserialize)]
struct ApiSearchResult {
    show: ApiShow,
}

#[derive(Deserialize)]
struct ApiShow {
    id: i64,
    name: String,
    premiered: Option<String>,
    image: Option<ApiImage>,
    summary: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
    runtime: Option<i64>,
    #[serde(rename = "averageRuntime")]
    average_runtime: Option<i64>,
    network: Option<ApiChannel>,
    #[serde(rename = "webChannel")]
    web_channel: Option<ApiChannel>,
    status: Option<String>,
    #[serde(rename = "_embedded")]
    embedded: Option<ApiEmbedded>,
}

#[derive(Deserialize)]
struct ApiEmbedded {
    nextepisode: Option<ApiEpisode>,
}

#[derive(Deserialize)]
struct ApiEpisode {
    season: Option<i32>,
    number: Option<i32>,
    airdate: Option<String>,
}

impl ApiEpisode {
    fn next_season(&self) -> Option<NextSeason> {
        let number = self.season.filter(|season| *season > 1)?;
        if self.number != Some(1) {
            return None;
        }

        Some(NextSeason {
            number,
            date: full_date(self.airdate.as_deref())?,
        })
    }
}

#[derive(Deserialize)]
struct ApiChannel {
    name: Option<String>,
}

#[derive(Deserialize)]
struct ApiSeason {
    number: Option<i32>,
}

#[derive(Deserialize)]
struct ApiImage {
    medium: Option<String>,
    original: Option<String>,
}

impl ApiShow {
    fn details(&self) -> Details {
        Details {
            synopsis: self.summary.as_deref().and_then(text::from_html),
            minutes: positive(self.average_runtime).or(positive(self.runtime)),
            pages: None,
            genres: self.genres.clone(),
            creators: [&self.network, &self.web_channel]
                .into_iter()
                .flatten()
                .filter_map(|channel| channel.name.clone())
                .take(1)
                .collect(),
            release_date: full_date(self.premiered.as_deref()),
            next_season: self
                .embedded
                .as_ref()
                .and_then(|embedded| embedded.nextepisode.as_ref())
                .and_then(ApiEpisode::next_season),
            ended: Some(self.status.as_deref() == Some(ENDED)),
        }
    }

    fn into_candidate(self) -> Candidate {
        Candidate {
            source: Source::TvMaze,
            external_id: self.id.to_string(),
            title: self.name,
            release_year: year_of(self.premiered.as_deref()),
            cover_url: self.image.and_then(|image| image.medium.or(image.original)),
            details: None,
            edition: None,
        }
    }
}

pub fn search(query: &str) -> Result<Vec<Candidate>> {
    search_at(BASE_URL, query, http::TIMEOUT)
}

pub fn details(id: &str) -> Result<Candidate> {
    details_at(BASE_URL, id, http::TIMEOUT)
}

pub fn seasons(id: &str) -> Result<Vec<i32>> {
    seasons_at(BASE_URL, id, http::TIMEOUT)
}

fn details_at(base_url: &str, id: &str, timeout: Duration) -> Result<Candidate> {
    let url = format!("{base_url}/shows/{}", numeric(id)?);
    parse_details(&http::get_text(
        &url,
        &[("embed", "nextepisode")],
        None,
        timeout,
    )?)
}

fn seasons_at(base_url: &str, id: &str, timeout: Duration) -> Result<Vec<i32>> {
    let url = format!("{base_url}/shows/{}/seasons", numeric(id)?);
    parse_seasons(&http::get_text(&url, &[], None, timeout)?)
}

pub fn parse_details(body: &str) -> Result<Candidate> {
    let show: ApiShow = serde_json::from_str(body)?;
    let details = show.details();

    Ok(Candidate {
        details: Some(details),
        ..show.into_candidate()
    })
}

pub fn parse_seasons(body: &str) -> Result<Vec<i32>> {
    let seasons: Vec<ApiSeason> = serde_json::from_str(body)?;
    let mut numbers: Vec<i32> = seasons
        .into_iter()
        .filter_map(|season| season.number)
        .filter(|number| *number > 0)
        .collect();
    numbers.sort_unstable();
    numbers.dedup();

    Ok(numbers)
}

pub fn search_json(query: &str) -> Result<String> {
    search_json_at(BASE_URL, query, http::TIMEOUT)
}

fn search_at(base_url: &str, query: &str, timeout: Duration) -> Result<Vec<Candidate>> {
    let body = search_json_at(base_url, query, timeout)?;
    parse_search(&body)
}

fn search_json_at(base_url: &str, query: &str, timeout: Duration) -> Result<String> {
    http::get_text(
        &format!("{base_url}{SEARCH_PATH}"),
        &[("q", query)],
        None,
        timeout,
    )
}

pub fn parse_search(body: &str) -> Result<Vec<Candidate>> {
    let results: Vec<ApiSearchResult> = serde_json::from_str(body)?;

    Ok(results
        .into_iter()
        .map(|result| result.show.into_candidate())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{Candidate, Source, parse_search, search_at};
    use crate::Error;
    use crate::http::test_server::{TEST_TIMEOUT, refuse, serve, serve_silently};

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/tvmaze/search-a.json");

        let shows = parse_search(body).unwrap();

        let show = shows
            .iter()
            .find(|show| show.external_id == "39235")
            .expect("fixture contains A Confession");
        assert_eq!(
            show,
            &Candidate {
                source: Source::TvMaze,
                external_id: "39235".to_owned(),
                title: "A Confession".to_owned(),
                release_year: Some(2019),
                cover_url: Some(
                    "https://static.tvmaze.com/uploads/images/medium_portrait/210/525047.jpg"
                        .to_owned()
                ),
                details: None,
                edition: None,
            }
        );
    }

    #[test]
    fn the_saved_response_keeps_shows_without_details() {
        let body = include_str!("../../tests/fixtures/tvmaze/search-a.json");

        let shows = parse_search(body).unwrap();

        let show = shows
            .iter()
            .find(|show| show.external_id == "67073")
            .expect("fixture contains A Lot");
        assert_eq!(show.title, "A Lot");
        assert_eq!(show.release_year, None);
        assert_eq!(show.cover_url, None);
    }

    #[test]
    fn a_show_without_a_premiere_or_image_maps_to_none() {
        let body = r#"[{"score":1.0,"show":{"id":7,"name":"Bare","premiered":null,"image":null}}]"#;

        let shows = parse_search(body).unwrap();

        assert_eq!(
            shows,
            vec![Candidate {
                source: Source::TvMaze,
                external_id: "7".to_owned(),
                title: "Bare".to_owned(),
                release_year: None,
                cover_url: None,
                details: None,
                edition: None,
            }]
        );
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(parse_search("{").is_err());
    }

    #[test]
    fn an_empty_result_is_not_an_error() {
        assert_eq!(parse_search("[]").unwrap(), Vec::<Candidate>::new());
    }

    #[test]
    fn original_is_used_when_medium_is_missing() {
        let body = r#"[{"show":{"id":8,"name":"Fallback","premiered":null,"image":{"medium":null,"original":"https://example.test/original.jpg"}}}]"#;

        let shows = parse_search(body).unwrap();

        assert_eq!(
            shows[0].cover_url.as_deref(),
            Some("https://example.test/original.jpg")
        );
    }

    #[test]
    fn http_429_is_rate_limited() {
        let base = serve("429 Too Many Requests", b"");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn http_500_reports_its_status_code() {
        let base = serve("500 Internal Server Error", b"");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let error = search_at(&refuse(), "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn a_silent_server_times_out() {
        let error = search_at(&serve_silently(), "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn invalid_json_after_a_200_is_a_json_error() {
        let base = serve("200 OK", b"{");

        let error = search_at(&base, "a", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::Json(_)));
    }

    #[test]
    fn an_empty_result_after_a_200_is_not_an_error() {
        let base = serve("200 OK", b"[]");

        assert_eq!(
            search_at(&base, "a", TEST_TIMEOUT).unwrap(),
            Vec::<Candidate>::new()
        );
    }

    #[test]
    fn maps_saved_show_details() {
        let body = include_str!("../../tests/fixtures/tvmaze/details-severance.json");
        let show = super::parse_details(body).unwrap();
        let details = show.details.unwrap();

        assert_eq!(show.title, "Severance");
        assert_eq!(show.external_id, "44933");
        assert_eq!(details.minutes, Some(49));
        assert_eq!(details.genres, ["Drama", "Science-Fiction", "Mystery"]);
        assert_eq!(details.creators, ["Apple TV"]);
        assert_eq!(details.release_date.as_deref(), Some("2022-02-18"));
        let synopsis = details.synopsis.unwrap();
        assert!(synopsis.starts_with("Mark Scout leads a team at Lumon Industries"));
        assert!(!synopsis.contains('<') && !synopsis.contains('&'));
    }

    #[test]
    fn maps_saved_seasons_in_order() {
        let body = include_str!("../../tests/fixtures/tvmaze/seasons-severance.json");

        assert_eq!(super::parse_seasons(body).unwrap(), [1, 2, 3]);
    }

    #[test]
    fn seasons_skip_specials_gaps_and_repeats() {
        let body = r#"[{"number":2},{"number":0},{"number":null},{"number":1},{"number":2}]"#;

        assert_eq!(super::parse_seasons(body).unwrap(), [1, 2]);
    }

    #[test]
    fn a_show_with_nothing_but_a_name_has_empty_details() {
        let show = super::parse_details(r#"{"id":1,"name":"Plain"}"#).unwrap();

        assert_eq!(
            show.details,
            Some(super::Details {
                ended: Some(false),
                ..super::Details::default()
            })
        );
    }

    #[test]
    fn an_announced_season_is_kept_with_its_date() {
        let show = |episode: &str| {
            let body = format!(
                r#"{{"id":1,"name":"Silo","status":"Running","_embedded":{{"nextepisode":{episode}}}}}"#
            );
            super::parse_details(&body).unwrap().details.unwrap()
        };

        let season = show(r#"{"season":4,"number":1,"airdate":"2027-07-09"}"#);
        assert_eq!(
            season.next_season,
            Some(super::NextSeason {
                number: 4,
                date: "2027-07-09".to_owned()
            })
        );
        assert_eq!(season.ended, Some(false));
        for other in [
            r#"{"season":4,"number":2,"airdate":"2027-07-16"}"#,
            r#"{"season":1,"number":1,"airdate":"2027-07-09"}"#,
            r#"{"season":4,"number":1,"airdate":""}"#,
            r#"{"season":4,"number":null,"airdate":"2027-07-09"}"#,
            "null",
        ] {
            assert_eq!(show(other).next_season, None, "{other}");
        }
    }

    #[test]
    fn an_ended_show_says_so() {
        let show = super::parse_details(r#"{"id":1,"name":"Andor","status":"Ended"}"#).unwrap();

        assert_eq!(show.details.unwrap().ended, Some(true));
    }
}
