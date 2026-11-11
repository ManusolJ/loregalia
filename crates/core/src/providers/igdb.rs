use crate::http;
use crate::providers::{Candidate, Source};
use crate::{Error, Result};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://api.igdb.com/v4";
const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const IMAGE_BASE_URL: &str = "https://images.igdb.com/igdb/image/upload/t_cover_big";
const FIELDS: &str = "name,cover.image_id,first_release_date";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IgdbToken {
    pub value: String,
    pub expires_in: u64,
}

#[derive(Deserialize)]
struct ApiToken {
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct ApiGame {
    id: i64,
    name: Option<String>,
    cover: Option<ApiCover>,
    first_release_date: Option<i64>,
}

#[derive(Deserialize)]
struct ApiCover {
    image_id: Option<String>,
}

impl ApiGame {
    fn into_candidate(self) -> Option<Candidate> {
        let title = self.name?.trim().to_owned();
        if title.is_empty() {
            return None;
        }

        Some(Candidate {
            source: Source::Igdb,
            external_id: self.id.to_string(),
            title,
            release_year: self.first_release_date.and_then(year_from_unix),
            cover_url: self
                .cover
                .and_then(|cover| cover.image_id)
                .map(|image_id| format!("{IMAGE_BASE_URL}/{image_id}.jpg")),
        })
    }
}

pub fn token(client_id: &str, client_secret: &str) -> Result<IgdbToken> {
    token_at(TOKEN_URL, client_id, client_secret, http::TIMEOUT)
}

fn token_at(
    base_url: &str,
    client_id: &str,
    client_secret: &str,
    timeout: Duration,
) -> Result<IgdbToken> {
    let body = http::post_text(
        base_url,
        &[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("grant_type", "client_credentials"),
        ],
        &[],
        "",
        timeout,
    )?;
    parse_token(&body)
}

pub fn search(client_id: &str, token: &str, query: &str) -> Result<Vec<Candidate>> {
    search_at(BASE_URL, client_id, token, query, http::TIMEOUT)
}

fn search_at(
    base_url: &str,
    client_id: &str,
    token: &str,
    query: &str,
    timeout: Duration,
) -> Result<Vec<Candidate>> {
    let body = format!("search \"{query}\"; fields {FIELDS}; limit 20;");
    let text = post(base_url, client_id, token, &body, timeout)?;
    parse_search(&text)
}

pub fn details(client_id: &str, token: &str, id: &str) -> Result<Candidate> {
    details_at(BASE_URL, client_id, token, id, http::TIMEOUT)
}

fn details_at(
    base_url: &str,
    client_id: &str,
    token: &str,
    id: &str,
    timeout: Duration,
) -> Result<Candidate> {
    let body = format!("where id = {id}; fields {FIELDS};");
    let text = post(base_url, client_id, token, &body, timeout)?;
    parse_details(&text)
}

fn post(
    base_url: &str,
    client_id: &str,
    token: &str,
    body: &str,
    timeout: Duration,
) -> Result<String> {
    let authorization = format!("Bearer {token}");
    http::post_text(
        &format!("{base_url}/games"),
        &[],
        &[("Client-ID", client_id), ("Authorization", &authorization)],
        body,
        timeout,
    )
}

pub fn parse_token(body: &str) -> Result<IgdbToken> {
    let token: ApiToken = serde_json::from_str(body)?;

    Ok(IgdbToken {
        value: token.access_token,
        expires_in: token.expires_in,
    })
}

pub fn parse_search(body: &str) -> Result<Vec<Candidate>> {
    let games: Vec<ApiGame> = serde_json::from_str(body)?;

    Ok(games
        .into_iter()
        .filter_map(ApiGame::into_candidate)
        .collect())
}

pub fn parse_details(body: &str) -> Result<Candidate> {
    let mut games: Vec<ApiGame> = serde_json::from_str(body)?;

    games
        .drain(..)
        .next()
        .and_then(ApiGame::into_candidate)
        .ok_or(Error::HttpStatus { code: 404 })
}

fn year_from_unix(timestamp: i64) -> Option<i32> {
    if timestamp <= 0 {
        return None;
    }

    Some(civil_year(timestamp / 86_400))
}

fn civil_year(days: i64) -> i32 {
    let shifted = days + 719_468;
    let era = if shifted >= 0 {
        shifted
    } else {
        shifted - 146_096
    } / 146_097;
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let month = month_part + if month_part < 10 { 3 } else { -9 };

    (year + if month <= 2 { 1 } else { 0 }) as i32
}

#[cfg(test)]
mod tests {
    use super::{
        FIELDS, IMAGE_BASE_URL, civil_year, details_at, parse_details, parse_search, parse_token,
        search_at, token_at, year_from_unix,
    };
    use crate::Error;
    use crate::http::test_server::{TEST_TIMEOUT, serve};
    use crate::providers::{Candidate, Source};

    fn game() -> Candidate {
        Candidate {
            source: Source::Igdb,
            external_id: "11737".to_owned(),
            title: "Outer Wilds".to_owned(),
            release_year: Some(2019),
            cover_url: Some(format!("{IMAGE_BASE_URL}/co65ac.jpg")),
        }
    }

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/igdb/search-outer-wilds.json");

        let games = parse_search(body).unwrap();

        assert_eq!(games[0], game());
        assert_eq!(games[1].title, "Outer Wilds: Echoes of the Eye");
        assert_eq!(games[1].release_year, Some(2021));
    }

    #[test]
    fn maps_a_saved_details_response() {
        let body = include_str!("../../tests/fixtures/igdb/details-outer-wilds.json");

        assert_eq!(parse_details(body).unwrap(), game());
    }

    #[test]
    fn an_empty_details_response_is_a_missing_game() {
        assert!(matches!(
            parse_details("[]"),
            Err(Error::HttpStatus { code: 404 })
        ));
    }

    #[test]
    fn a_game_without_a_name_or_cover_is_skipped() {
        let body = r#"[
            {"id":1,"name":null,"cover":null,"first_release_date":null},
            {"id":2,"name":"Bare","cover":null,"first_release_date":null}
        ]"#;

        let games = parse_search(body).unwrap();

        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Bare");
        assert_eq!(games[0].release_year, None);
        assert_eq!(games[0].cover_url, None);
    }

    #[test]
    fn parses_an_oauth_token() {
        let body = r#"{"access_token":"abc","expires_in":5184000,"token_type":"bearer"}"#;

        let token = parse_token(body).unwrap();

        assert_eq!(token.value, "abc");
        assert_eq!(token.expires_in, 5_184_000);
    }

    #[test]
    fn a_timestamp_becomes_its_utc_year() {
        assert_eq!(year_from_unix(1_559_001_600), Some(2019));
        assert_eq!(year_from_unix(1_632_787_200), Some(2021));
        assert_eq!(year_from_unix(0), None);
        assert_eq!(year_from_unix(-1), None);
    }

    #[test]
    fn the_civil_year_matches_known_dates() {
        assert_eq!(civil_year(0), 1970);
        assert_eq!(civil_year(18_044), 2019);
        assert_eq!(civil_year(18_898), 2021);
    }

    #[test]
    fn the_search_body_asks_for_the_needed_fields() {
        assert!(FIELDS.contains("first_release_date"));
        assert!(FIELDS.contains("cover.image_id"));
    }

    #[test]
    fn a_rejected_token_is_unauthorized() {
        let base = serve("401 Unauthorized", b"");

        let error = token_at(&base, "id", "secret", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::Unauthorized));
    }

    #[test]
    fn a_missing_token_is_an_error() {
        let base = serve("200 OK", b"{}");

        assert!(matches!(
            token_at(&base, "id", "secret", TEST_TIMEOUT),
            Err(Error::Json(_))
        ));
    }

    #[test]
    fn a_search_reads_a_saved_response() {
        let body = include_str!("../../tests/fixtures/igdb/search-outer-wilds.json");
        let base = serve("200 OK", body.as_bytes());

        let games = search_at(&base, "id", "token", "outer wilds", TEST_TIMEOUT).unwrap();

        assert_eq!(games[0], game());
    }

    #[test]
    fn a_details_reads_a_saved_response() {
        let body = include_str!("../../tests/fixtures/igdb/details-outer-wilds.json");
        let base = serve("200 OK", body.as_bytes());

        let result = details_at(&base, "id", "token", "11737", TEST_TIMEOUT).unwrap();

        assert_eq!(result, game());
    }
}
