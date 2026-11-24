use crate::details::Details;
use crate::http;
use crate::providers::{Candidate, Source, full_date, numeric, positive};
use crate::{Error, Result, library, log, text};
use serde::Deserialize;
use serde_json::Value;
use std::cmp::Reverse;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BASE_URL: &str = "https://api.igdb.com/v4";
const TOKEN_URL: &str = "https://id.twitch.tv/oauth2/token";
const IMAGE_BASE_URL: &str = "https://images.igdb.com/igdb/image/upload/t_cover_big";
const FIELDS: &str =
    "name,cover.image_id,first_release_date,game_type.type,version_parent,total_rating_count";
const SEARCH_LIMIT: usize = 50;
const MAX_RESULTS: usize = 20;
const MAIN_GAME: &str = "main game";
const SECONDARY_WORDS: [&str; 10] = [
    "mod", "port", "fork", "pack", "addon", "update", "bundle", "episode", "season", "dlc",
];
const GAME_TYPES: [&str; 15] = [
    "Main game",
    "DLC",
    "Expansion",
    "Bundle",
    "Standalone expansion",
    "Mod",
    "Episode",
    "Season",
    "Remake",
    "Remaster",
    "Expanded game",
    "Port",
    "Fork",
    "Pack",
    "Update",
];
const DETAIL_FIELDS: &str = "name,cover.image_id,first_release_date,summary,genres.name,involved_companies.developer,involved_companies.company.name";
const TIME_FIELDS: &str = "normally,hastily,completely";

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
    summary: Option<String>,
    #[serde(default)]
    genres: Value,
    #[serde(default)]
    involved_companies: Value,
    #[serde(default)]
    game_type: Value,
    #[serde(default)]
    version_parent: Value,
    total_rating_count: Option<i64>,
}

#[derive(Deserialize)]
struct ApiTime {
    normally: Option<i64>,
    hastily: Option<i64>,
    completely: Option<i64>,
}

fn comparable(title: &str) -> String {
    library::fold(title)
        .split(|letter: char| !letter.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn entries(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn name_in(value: &Value) -> Option<String> {
    value.get("name")?.as_str().map(str::to_owned)
}

#[derive(Deserialize)]
struct ApiCover {
    image_id: Option<String>,
}

impl ApiGame {
    fn kind(&self) -> Option<String> {
        let name = match &self.game_type {
            Value::Number(number) => number
                .as_u64()
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| GAME_TYPES.get(index))
                .map(|name| (*name).to_owned())?,
            value => value.get("type")?.as_str()?.trim().replace('_', " "),
        };

        (!name.is_empty() && !name.eq_ignore_ascii_case(MAIN_GAME)).then_some(name)
    }

    fn edition(&self) -> Option<String> {
        self.kind()
            .or_else(|| (!self.version_parent.is_null()).then(|| "Edition".to_owned()))
    }

    fn is_secondary(&self) -> bool {
        !self.version_parent.is_null()
            || self.kind().is_some_and(|kind| {
                kind.to_lowercase()
                    .split(|letter: char| !letter.is_alphanumeric())
                    .any(|word| SECONDARY_WORDS.contains(&word))
            })
    }

    fn rank(&self, wanted: &str) -> (u8, Reverse<i64>) {
        let exact = self
            .name
            .as_deref()
            .is_some_and(|name| comparable(name) == wanted);
        match (self.is_secondary(), exact) {
            (false, true) => (0, Reverse(self.total_rating_count.unwrap_or(0))),
            (false, false) => (1, Reverse(0)),
            (true, _) => (2, Reverse(0)),
        }
    }

    fn details(&self) -> Details {
        Details {
            synopsis: self.summary.as_deref().and_then(text::tidy),
            minutes: None,
            pages: None,
            genres: entries(&self.genres).filter_map(name_in).collect(),
            creators: entries(&self.involved_companies)
                .filter(|company| company.get("developer").and_then(Value::as_bool) == Some(true))
                .filter_map(|company| name_in(company.get("company")?))
                .collect(),
            release_date: self
                .first_release_date
                .filter(|seconds| *seconds > 0)
                .and_then(|seconds| u64::try_from(seconds).ok())
                .map(|seconds| log::timestamp(UNIX_EPOCH + Duration::from_secs(seconds)))
                .and_then(|date| full_date(Some(&date))),
        }
    }

    fn into_candidate(self) -> Option<Candidate> {
        let edition = self.edition();
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
            details: None,
            edition,
        })
    }
}

const GAMES: &str = "games";
const TIMES: &str = "game_time_to_beats";
const REFRESH_MARGIN_SECONDS: u64 = 60;

static USER_TOKENS: TokenCache = TokenCache::new();

struct CachedToken {
    client_id: String,
    client_secret: String,
    value: String,
    expires_at: u64,
}

pub struct TokenCache {
    cached: Mutex<Option<CachedToken>>,
}

impl Default for TokenCache {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenCache {
    pub const fn new() -> Self {
        Self {
            cached: Mutex::new(None),
        }
    }

    fn token(
        &self,
        client_id: &str,
        client_secret: &str,
        now: u64,
        fetch: &dyn Fn() -> Result<IgdbToken>,
    ) -> Result<(String, bool)> {
        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(token) = cached.as_ref()
            && token.client_id == client_id
            && token.client_secret == client_secret
            && token.expires_at > now.saturating_add(REFRESH_MARGIN_SECONDS)
        {
            return Ok((token.value.clone(), true));
        }

        let fresh = fetch()?;
        let value = fresh.value.clone();
        *cached = Some(CachedToken {
            client_id: client_id.to_owned(),
            client_secret: client_secret.to_owned(),
            value: fresh.value,
            expires_at: now.saturating_add(fresh.expires_in),
        });
        Ok((value, false))
    }

    fn forget(&self) {
        *self
            .cached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }

    pub fn with_token<T>(
        &self,
        client_id: &str,
        client_secret: &str,
        now: u64,
        fetch: &dyn Fn() -> Result<IgdbToken>,
        call: &dyn Fn(&str) -> Result<T>,
    ) -> Result<T> {
        let (token, reused) = self.token(client_id, client_secret, now, fetch)?;
        match call(&token) {
            Err(Error::Unauthorized) if reused => {
                self.forget();
                let (fresh, _) = self.token(client_id, client_secret, now, fetch)?;
                call(&fresh)
            }
            result => result,
        }
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn search_with_credentials(
    client_id: &str,
    client_secret: &str,
    query: &str,
) -> Result<Vec<Candidate>> {
    USER_TOKENS.with_token(
        client_id,
        client_secret,
        unix_now(),
        &|| token(client_id, client_secret),
        &|access| search(client_id, access, query),
    )
}

pub fn details_with_credentials(
    client_id: &str,
    client_secret: &str,
    id: &str,
) -> Result<Candidate> {
    USER_TOKENS.with_token(
        client_id,
        client_secret,
        unix_now(),
        &|| token(client_id, client_secret),
        &|access| details(client_id, access, id),
    )
}

pub fn token(client_id: &str, client_secret: &str) -> Result<IgdbToken> {
    token_at(TOKEN_URL, client_id, client_secret, http::TIMEOUT)
}

fn search_clause(query: &str) -> String {
    let cleaned: String = query
        .chars()
        .map(|character| match character {
            '"' | '\\' => ' ',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect();
    format!(
        "search \"{}\"; fields {FIELDS}; limit {SEARCH_LIMIT};",
        cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

fn details_clause(id: &str) -> Result<String> {
    Ok(format!(
        "where id = {}; fields {DETAIL_FIELDS};",
        numeric(id)?
    ))
}

fn time_clause(id: &str) -> Result<String> {
    Ok(format!(
        "where game_id = {}; fields {TIME_FIELDS};",
        numeric(id)?
    ))
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
    let clause = search_clause(query);
    let text = post(base_url, GAMES, client_id, token, &clause, timeout)?;
    parse_search(&text, query)
}

pub fn details(client_id: &str, token: &str, id: &str) -> Result<Candidate> {
    let game = game(client_id, token, id)?;
    let minutes = time_to_beat(client_id, token, id)?;

    Ok(with_time(game, minutes))
}

pub fn game(client_id: &str, token: &str, id: &str) -> Result<Candidate> {
    details_at(BASE_URL, client_id, token, id, http::TIMEOUT)
}

pub fn time_to_beat(client_id: &str, token: &str, id: &str) -> Result<Option<u32>> {
    time_to_beat_at(BASE_URL, client_id, token, id, http::TIMEOUT)
}

pub fn with_time(game: Candidate, minutes: Option<u32>) -> Candidate {
    let details = Details {
        minutes,
        ..game.details.clone().unwrap_or_default()
    };

    Candidate {
        details: Some(details),
        ..game
    }
}

fn time_to_beat_at(
    base_url: &str,
    client_id: &str,
    token: &str,
    id: &str,
    timeout: Duration,
) -> Result<Option<u32>> {
    let clause = time_clause(id)?;
    let text = post(base_url, TIMES, client_id, token, &clause, timeout)?;
    parse_time_to_beat(&text)
}

pub fn parse_time_to_beat(body: &str) -> Result<Option<u32>> {
    let times: Vec<ApiTime> = serde_json::from_str(body)?;

    Ok(times.into_iter().next().and_then(|time| {
        [time.normally, time.hastily, time.completely]
            .into_iter()
            .find_map(positive)
            .map(|seconds| seconds.div_ceil(60))
    }))
}

fn details_at(
    base_url: &str,
    client_id: &str,
    token: &str,
    id: &str,
    timeout: Duration,
) -> Result<Candidate> {
    let clause = details_clause(id)?;
    let text = post(base_url, GAMES, client_id, token, &clause, timeout)?;
    parse_details(&text)
}

fn post(
    base_url: &str,
    path: &str,
    client_id: &str,
    token: &str,
    body: &str,
    timeout: Duration,
) -> Result<String> {
    let authorization = format!("Bearer {token}");
    http::post_text(
        &format!("{base_url}/{path}"),
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

pub fn parse_search(body: &str, query: &str) -> Result<Vec<Candidate>> {
    let mut games: Vec<ApiGame> = serde_json::from_str(body)?;
    let wanted = comparable(query);
    games.sort_by_key(|game| game.rank(&wanted));

    Ok(games
        .into_iter()
        .take(MAX_RESULTS)
        .filter_map(ApiGame::into_candidate)
        .collect())
}

pub fn parse_details(body: &str) -> Result<Candidate> {
    let mut games: Vec<ApiGame> = serde_json::from_str(body)?;

    games
        .drain(..)
        .next()
        .and_then(|game| {
            let details = game.details();
            game.into_candidate().map(|candidate| Candidate {
                details: Some(details),
                ..candidate
            })
        })
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
        FIELDS, IMAGE_BASE_URL, IgdbToken, TokenCache, civil_year, details_at, details_clause,
        parse_details, parse_search, parse_token, search_at, search_clause, token_at,
        year_from_unix,
    };
    use crate::Error;
    use std::cell::Cell;

    fn issued(value: &str, expires_in: u64) -> crate::Result<IgdbToken> {
        Ok(IgdbToken {
            value: value.to_owned(),
            expires_in,
        })
    }

    #[test]
    fn a_token_is_reused_until_it_nears_expiry() {
        let cache = TokenCache::new();
        let fetches = Cell::new(0);
        let fetch = || {
            fetches.set(fetches.get() + 1);
            issued(&format!("token-{}", fetches.get()), 1_000)
        };
        let echo = |token: &str| Ok(token.to_owned());

        assert_eq!(
            cache.with_token("id", "secret", 0, &fetch, &echo).unwrap(),
            "token-1"
        );
        assert_eq!(
            cache
                .with_token("id", "secret", 500, &fetch, &echo)
                .unwrap(),
            "token-1"
        );
        assert_eq!(
            cache
                .with_token("id", "secret", 950, &fetch, &echo)
                .unwrap(),
            "token-2"
        );
        assert_eq!(fetches.get(), 2);
    }

    #[test]
    fn changed_keys_get_their_own_token() {
        let cache = TokenCache::new();
        let fetches = Cell::new(0);
        let fetch = || {
            fetches.set(fetches.get() + 1);
            issued(&format!("token-{}", fetches.get()), 1_000)
        };
        let echo = |token: &str| Ok(token.to_owned());

        cache.with_token("id", "secret", 0, &fetch, &echo).unwrap();
        assert_eq!(
            cache.with_token("id", "other", 1, &fetch, &echo).unwrap(),
            "token-2"
        );
        assert_eq!(
            cache
                .with_token("other", "other", 2, &fetch, &echo)
                .unwrap(),
            "token-3"
        );
    }

    #[test]
    fn a_rejected_cached_token_is_replaced_once() {
        let cache = TokenCache::new();
        let fetches = Cell::new(0);
        let fetch = || {
            fetches.set(fetches.get() + 1);
            issued(&format!("token-{}", fetches.get()), 1_000)
        };
        cache
            .with_token("id", "secret", 0, &fetch, &|token: &str| {
                Ok(token.to_owned())
            })
            .unwrap();
        let only_fresh = |token: &str| {
            if token == "token-1" {
                Err(Error::Unauthorized)
            } else {
                Ok(token.to_owned())
            }
        };

        assert_eq!(
            cache
                .with_token("id", "secret", 1, &fetch, &only_fresh)
                .unwrap(),
            "token-2"
        );
    }

    #[test]
    fn a_rejected_new_token_is_not_retried() {
        let cache = TokenCache::new();
        let fetches = Cell::new(0);
        let fetch = || {
            fetches.set(fetches.get() + 1);
            issued("token", 1_000)
        };
        let reject = |_: &str| -> crate::Result<String> { Err(Error::Unauthorized) };

        assert!(matches!(
            cache.with_token("id", "secret", 0, &fetch, &reject),
            Err(Error::Unauthorized)
        ));
        assert_eq!(fetches.get(), 1);
    }

    #[test]
    fn a_search_cannot_close_the_quoted_string() {
        let clause = search_clause(r#"zelda"; fields *; limit 500; where id = 1 \"#);

        assert_eq!(
            clause,
            format!(
                "search \"zelda ; fields *; limit 500; where id = 1\"; fields {FIELDS}; limit {};",
                super::SEARCH_LIMIT
            )
        );
        assert_eq!(clause.matches('"').count(), 2);
    }

    #[test]
    fn details_accept_only_numeric_ids() {
        assert_eq!(
            details_clause("1942").unwrap(),
            format!("where id = 1942; fields {};", super::DETAIL_FIELDS)
        );
        assert!(details_clause("1942; fields *").is_err());
        assert!(details_clause("").is_err());
    }
    use crate::http::test_server::{TEST_TIMEOUT, serve};
    use crate::providers::{Candidate, Source};

    fn detailed_game() -> Candidate {
        Candidate {
            details: Some(super::Details {
                release_date: Some("2019-05-28".to_owned()),
                ..Default::default()
            }),
            ..game()
        }
    }

    fn game() -> Candidate {
        Candidate {
            source: Source::Igdb,
            external_id: "11737".to_owned(),
            title: "Outer Wilds".to_owned(),
            release_year: Some(2019),
            cover_url: Some(format!("{IMAGE_BASE_URL}/co65ac.jpg")),
            details: None,
            edition: None,
        }
    }

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/igdb/search-outer-wilds.json");

        let games = parse_search(body, "outer wilds").unwrap();

        assert_eq!(games[0], game());
        assert_eq!(games[1].title, "Outer Wilds: Echoes of the Eye");
        assert_eq!(games[1].release_year, Some(2021));
    }

    #[test]
    fn maps_a_saved_details_response() {
        let body = include_str!("../../tests/fixtures/igdb/details-outer-wilds.json");

        assert_eq!(parse_details(body).unwrap(), detailed_game());
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

        let games = parse_search(body, "outer wilds").unwrap();

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

        assert_eq!(result, detailed_game());
    }

    #[test]
    fn maps_game_details_with_the_developer_only() {
        let body = include_str!("../../tests/fixtures/igdb/details-outer-wilds-full.json");
        let details = parse_details(body).unwrap().details.unwrap();

        assert_eq!(details.genres, ["Role-playing (RPG)", "Adventure"]);
        assert_eq!(details.creators, ["Mobius Digital"]);
        assert_eq!(details.release_date.as_deref(), Some("2019-05-28"));
        assert!(details.synopsis.unwrap().starts_with("Explore"));
    }

    #[test]
    fn unexpanded_ids_leave_details_empty_instead_of_failing() {
        let body = r#"[{"id":1,"name":"Game","genres":[12,31],"involved_companies":[7]}]"#;
        let details = parse_details(body).unwrap().details.unwrap();

        assert_eq!(details, super::Details::default());
    }

    #[test]
    fn time_to_beat_is_in_minutes_and_prefers_the_normal_pace() {
        let parse = |body| super::parse_time_to_beat(body).unwrap();

        assert_eq!(
            parse(r#"[{"id":1,"normally":78600,"hastily":36000,"completely":90000}]"#),
            Some(1_310)
        );
        assert_eq!(parse(r#"[{"hastily":3600}]"#), Some(60));
        assert_eq!(parse(r#"[{"normally":0}]"#), None);
        assert_eq!(parse("[]"), None);
    }

    #[test]
    fn time_to_beat_joins_the_details() {
        let body = include_str!("../../tests/fixtures/igdb/details-outer-wilds-full.json");
        let game = super::with_time(parse_details(body).unwrap(), Some(1_310));
        let details = game.details.unwrap();

        assert_eq!(details.minutes, Some(1_310));
        assert_eq!(details.creators, ["Mobius Digital"]);
    }

    #[test]
    fn time_to_beat_asks_only_for_numeric_ids() {
        assert!(
            super::time_clause("11737")
                .unwrap()
                .contains("game_id = 11737;")
        );
        assert!(super::time_clause("1; fields *").is_err());
    }

    fn titles(body: &str, query: &str) -> Vec<(String, Option<i32>, Option<String>)> {
        parse_search(body, query)
            .unwrap()
            .into_iter()
            .map(|game| (game.title, game.release_year, game.edition))
            .collect()
    }

    #[test]
    fn the_original_comes_before_its_ports_and_editions() {
        let body = r#"[
            {"id":1,"name":"Hollow Knight","first_release_date":1699833600,"game_type":{"id":11,"type":"Port"}},
            {"id":2,"name":"Hollow Knight: Voidheart Edition","first_release_date":1537833600,"version_parent":4},
            {"id":3,"name":"Hollow Knight: Silksong","first_release_date":1757000000,"game_type":{"id":0,"type":"Main Game"}},
            {"id":4,"name":"Hollow Knight","first_release_date":1487894400,"game_type":{"id":0,"type":"Main Game"},"total_rating_count":1500}
        ]"#;

        assert_eq!(
            titles(body, "hollow knight"),
            [
                ("Hollow Knight".to_owned(), Some(2017), None),
                ("Hollow Knight: Silksong".to_owned(), Some(2025), None),
                (
                    "Hollow Knight".to_owned(),
                    Some(2023),
                    Some("Port".to_owned())
                ),
                (
                    "Hollow Knight: Voidheart Edition".to_owned(),
                    Some(2018),
                    Some("Edition".to_owned())
                ),
            ]
        );
    }

    #[test]
    fn the_best_known_game_wins_when_the_types_do_not_help() {
        let body = r#"[
            {"id":1,"name":"Disco Elysium","first_release_date":1754352000},
            {"id":2,"name":"Disco Elysium","first_release_date":1571097600,"total_rating_count":900}
        ]"#;

        assert_eq!(titles(body, "Disco  Elysium")[0].1, Some(2019));
    }

    #[test]
    fn a_remake_keeps_its_place_and_says_what_it_is() {
        let body = r#"[
            {"id":1,"name":"Dead Space","game_type":{"id":8,"type":"Remake"},"total_rating_count":700},
            {"id":2,"name":"Dead Space","game_type":{"id":0,"type":"Main Game"},"total_rating_count":600}
        ]"#;

        assert_eq!(titles(body, "dead space")[0].2.as_deref(), Some("Remake"));
        assert_eq!(titles(body, "dead space")[1].2, None);
    }

    #[test]
    fn an_exact_title_comes_before_longer_ones_whatever_its_punctuation() {
        let body = r#"[
            {"id":1,"name":"Elden Ring Nightreign"},
            {"id":2,"name":"ELDEN RING\u2122"}
        ]"#;

        assert_eq!(titles(body, "elden ring")[0].0, "ELDEN RING\u{2122}");
    }

    #[test]
    fn other_results_keep_the_order_igdb_gave_them() {
        let body = r#"[
            {"id":1,"name":"Zelda B","total_rating_count":1},
            {"id":2,"name":"Zelda A","total_rating_count":900},
            {"id":3,"name":"Zelda C"}
        ]"#;
        let names: Vec<String> = titles(body, "zelda")
            .into_iter()
            .map(|game| game.0)
            .collect();

        assert_eq!(names, ["Zelda B", "Zelda A", "Zelda C"]);
    }

    #[test]
    fn unexpanded_and_odd_types_are_read_or_ignored() {
        let body = r#"[
            {"id":1,"name":"A","game_type":11},
            {"id":2,"name":"B","game_type":0},
            {"id":3,"name":"C","game_type":99},
            {"id":4,"name":"D","game_type":"port"},
            {"id":5,"name":"E","game_type":{"id":1,"type":"Pack / Addon"}},
            {"id":6,"name":"F","game_type":{"type":"main_game"}}
        ]"#;
        let found = titles(body, "x");
        let edition = |name: &str| {
            found
                .iter()
                .find(|game| game.0 == name)
                .and_then(|game| game.2.clone())
        };

        assert_eq!(edition("A").as_deref(), Some("Port"));
        for main in ["B", "C", "D", "F"] {
            assert_eq!(edition(main), None, "{main}");
        }
        assert_eq!(edition("E").as_deref(), Some("Pack / Addon"));
        assert_eq!(found.last().map(|game| game.0.as_str()), Some("E"));
    }

    #[test]
    fn a_search_asks_for_more_than_it_shows() {
        let games: Vec<String> = (0..50)
            .map(|id| format!(r#"{{"id":{id},"name":"Game {id}"}}"#))
            .collect();
        let body = format!("[{}]", games.join(","));

        assert_eq!(titles(&body, "game").len(), super::MAX_RESULTS);
    }
}
