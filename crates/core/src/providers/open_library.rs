use crate::details::Details;
use crate::http;
use crate::providers::{Candidate, Source, numeric, positive};
use crate::{Error, Result, text};
use serde::Deserialize;
use std::time::Duration;

const BASE_URL: &str = "https://openlibrary.org";
const SEARCH_PATH: &str = "/search.json";
const COVER_BASE_URL: &str = "https://covers.openlibrary.org/b/id";
const LIMIT: &str = "20";
const DETAIL_FIELDS: &str =
    "key,title,first_publish_year,cover_i,author_name,number_of_pages_median,subject";
const MAX_GENRES: usize = 4;
const MAX_AUTHORS: usize = 3;
const KNOWN_GENRES: [&str; 24] = [
    "Fantasy",
    "Science fiction",
    "Mystery",
    "Thriller",
    "Horror",
    "Romance",
    "Historical fiction",
    "Adventure",
    "Crime",
    "Dystopian",
    "Young adult",
    "Classics",
    "Poetry",
    "Drama",
    "Humor",
    "Short stories",
    "Graphic novels",
    "Biography",
    "Autobiography",
    "Memoir",
    "History",
    "Philosophy",
    "Science",
    "Travel",
];

#[derive(Deserialize)]
struct ApiSearch {
    docs: Vec<ApiDoc>,
}

#[derive(Deserialize)]
struct ApiDoc {
    key: Option<String>,
    title: Option<String>,
    first_publish_year: Option<i32>,
    cover_i: Option<i64>,
    #[serde(default)]
    author_name: Vec<String>,
    number_of_pages_median: Option<i64>,
    #[serde(default)]
    subject: Vec<String>,
}

#[derive(Deserialize)]
struct ApiWork {
    description: Option<ApiText>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ApiText {
    Plain(String),
    Typed { value: String },
}

impl ApiWork {
    fn synopsis(self) -> Option<String> {
        match self.description? {
            ApiText::Plain(value) | ApiText::Typed { value } => text::from_markdown(&value),
        }
    }
}

fn genres(subjects: &[String]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for subject in subjects {
        let lower = subject.trim().to_lowercase();
        let name = lower.strip_prefix("genre:").unwrap_or(&lower);
        if let Some(genre) = KNOWN_GENRES
            .iter()
            .find(|genre| genre.to_lowercase() == name)
            && !found.iter().any(|known| known == genre)
        {
            found.push((*genre).to_owned());
        }
    }
    found.truncate(MAX_GENRES);
    found
}

fn work_id(id: &str) -> Result<&str> {
    let digits = id
        .strip_prefix("OL")
        .and_then(|rest| rest.strip_suffix('W'))
        .ok_or(Error::HttpStatus { code: 404 })?;
    numeric(digits)?;
    Ok(id)
}

impl ApiDoc {
    fn details(&self, synopsis: Option<String>) -> Details {
        Details {
            synopsis,
            minutes: None,
            pages: positive(self.number_of_pages_median),
            genres: genres(&self.subject),
            creators: self.author_name.iter().take(MAX_AUTHORS).cloned().collect(),
            release_date: None,
            next_season: None,
            ended: None,
        }
    }

    fn into_candidate(self) -> Option<Candidate> {
        let title = self.title?.trim().to_owned();
        let external_id = self.key?.trim_start_matches("/works/").to_owned();
        if title.is_empty() || external_id.is_empty() {
            return None;
        }

        Some(Candidate {
            source: Source::OpenLibrary,
            external_id,
            title,
            release_year: self.first_publish_year,
            cover_url: self
                .cover_i
                .map(|cover| format!("{COVER_BASE_URL}/{cover}-M.jpg?default=false")),
            details: None,
            edition: None,
        })
    }
}

pub fn search(query: &str) -> Result<Vec<Candidate>> {
    search_at(BASE_URL, query, http::TIMEOUT)
}

pub fn details(id: &str) -> Result<Candidate> {
    details_at(BASE_URL, id, http::TIMEOUT)
}

fn details_at(base_url: &str, id: &str, timeout: Duration) -> Result<Candidate> {
    let id = work_id(id)?;
    let found = http::get_text(
        &format!("{base_url}{SEARCH_PATH}"),
        &[
            ("q", &format!("key:\"/works/{id}\"")),
            ("fields", DETAIL_FIELDS),
            ("limit", "1"),
        ],
        None,
        timeout,
    )?;
    let work = http::get_text(&format!("{base_url}/works/{id}.json"), &[], None, timeout)?;

    parse_details(&found, &work)
}

pub fn parse_details(found: &str, work: &str) -> Result<Candidate> {
    let results: ApiSearch = serde_json::from_str(found)?;
    let work: ApiWork = serde_json::from_str(work)?;
    let doc = results
        .docs
        .into_iter()
        .next()
        .ok_or(Error::HttpStatus { code: 404 })?;
    let details = doc.details(work.synopsis());

    doc.into_candidate()
        .map(|candidate| Candidate {
            details: Some(details),
            ..candidate
        })
        .ok_or(Error::HttpStatus { code: 404 })
}

fn search_at(base_url: &str, query: &str, timeout: Duration) -> Result<Vec<Candidate>> {
    let body = http::get_text(
        &format!("{base_url}{SEARCH_PATH}"),
        &[("q", query), ("limit", LIMIT)],
        None,
        timeout,
    )?;
    parse_search(&body)
}

pub fn parse_search(body: &str) -> Result<Vec<Candidate>> {
    let results: ApiSearch = serde_json::from_str(body)?;

    Ok(results
        .docs
        .into_iter()
        .filter_map(ApiDoc::into_candidate)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{Source, parse_search, search_at};
    use crate::Error;
    use crate::http::test_server::{TEST_TIMEOUT, refuse, serve};

    #[test]
    fn maps_a_saved_search_response() {
        let body = include_str!("../../tests/fixtures/openlibrary/search-piranesi.json");

        let books = parse_search(body).unwrap();

        assert_eq!(books[0].source, Source::OpenLibrary);
        assert_eq!(books[0].external_id, "OL20893680W");
        assert_eq!(books[0].title, "Piranesi");
        assert_eq!(books[0].release_year, Some(2020));
        assert_eq!(
            books[0].cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/10226290-M.jpg?default=false")
        );
    }

    #[test]
    fn keeps_a_book_without_a_cover() {
        let body = include_str!("../../tests/fixtures/openlibrary/search-piranesi.json");

        let books = parse_search(body).unwrap();
        let book = books
            .iter()
            .find(|book| book.external_id == "OL12523946W")
            .unwrap();

        assert_eq!(book.title, "Piranesi");
        assert_eq!(book.release_year, Some(1910));
        assert_eq!(book.cover_url, None);
    }

    #[test]
    fn skips_entries_without_a_title_or_work_key() {
        let body = r#"{"docs":[
            {"key":"/works/OL1W","title":"Keep"},
            {"key":"/works/OL2W"},
            {"title":"No key"},
            {"key":"/works/OL3W","title":"   "}
        ]}"#;

        let books = parse_search(body).unwrap();

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "Keep");
    }

    #[test]
    fn an_empty_result_is_not_an_error() {
        assert_eq!(parse_search(r#"{"docs":[]}"#).unwrap(), Vec::new());
    }

    #[test]
    fn invalid_json_is_an_error() {
        assert!(matches!(parse_search("{"), Err(Error::Json(_))));
    }

    #[test]
    fn http_429_is_rate_limited() {
        let base = serve("429 Too Many Requests", b"");

        let error = search_at(&base, "piranesi", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn http_500_reports_its_status_code() {
        let base = serve("500 Internal Server Error", b"");

        let error = search_at(&base, "piranesi", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let error = search_at(&refuse(), "piranesi", TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn maps_saved_book_details() {
        let found = include_str!("../../tests/fixtures/openlibrary/details-piranesi.json");
        let work = include_str!("../../tests/fixtures/openlibrary/work-piranesi.json");
        let book = super::parse_details(found, work).unwrap();
        let details = book.details.unwrap();

        assert_eq!(book.external_id, "OL20893680W");
        assert_eq!(details.pages, Some(272));
        assert_eq!(details.creators, ["Susanna Clarke"]);
        assert_eq!(details.genres, ["Fantasy"]);
        let synopsis = details.synopsis.unwrap();
        assert!(synopsis.starts_with("From the New York Times bestselling author"));
        assert!(!synopsis.contains('*'));
    }

    #[test]
    fn a_description_may_be_plain_text_or_missing() {
        let found = r#"{"docs":[{"key":"/works/OL1W","title":"Book"}]}"#;
        let synopsis = |work| {
            super::parse_details(found, work)
                .unwrap()
                .details
                .unwrap()
                .synopsis
        };

        assert_eq!(
            synopsis(r#"{"description":"Plain."}"#).as_deref(),
            Some("Plain.")
        );
        assert_eq!(synopsis("{}"), None);
    }

    #[test]
    fn a_missing_book_is_not_found() {
        assert!(matches!(
            super::parse_details(r#"{"docs":[]}"#, "{}"),
            Err(Error::HttpStatus { code: 404 })
        ));
    }
}
