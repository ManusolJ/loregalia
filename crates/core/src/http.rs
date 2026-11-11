use crate::{Error, Result};
use std::time::Duration;

pub(crate) const USER_AGENT: &str = concat!(
    "Loregalia/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ManusolJ/loregalia)"
);
pub(crate) const TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn get_text(
    url: &str,
    query: &[(&str, &str)],
    bearer_token: Option<&str>,
    timeout: Duration,
) -> Result<String> {
    send(url, query, bearer_token, timeout)?
        .body_mut()
        .read_to_string()
        .map_err(classify)
}

pub(crate) fn get_bytes(url: &str, timeout: Duration, max_bytes: u64) -> Result<Vec<u8>> {
    send(url, &[], None, timeout)?
        .body_mut()
        .with_config()
        .limit(max_bytes)
        .read_to_vec()
        .map_err(classify)
}

pub(crate) fn post_text(
    url: &str,
    query: &[(&str, &str)],
    headers: &[(&str, &str)],
    body: &str,
    timeout: Duration,
) -> Result<String> {
    post(url, query, headers, body, timeout)?
        .body_mut()
        .read_to_string()
        .map_err(classify)
}

fn send(
    url: &str,
    query: &[(&str, &str)],
    bearer_token: Option<&str>,
    timeout: Duration,
) -> Result<ureq::http::Response<ureq::Body>> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent.get(url).header("User-Agent", USER_AGENT);

    if let Some(token) = bearer_token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    for (name, value) in query {
        request = request.query(name, value);
    }

    request.call().map_err(classify)
}

fn post(
    url: &str,
    query: &[(&str, &str)],
    headers: &[(&str, &str)],
    body: &str,
    timeout: Duration,
) -> Result<ureq::http::Response<ureq::Body>> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build();
    let agent: ureq::Agent = config.into();
    let mut request = agent.post(url).header("User-Agent", USER_AGENT);

    for &(name, value) in headers {
        request = request.header(name, value);
    }
    for &(name, value) in query {
        request = request.query(name, value);
    }

    request.send(body).map_err(classify)
}

pub(crate) fn classify(error: ureq::Error) -> Error {
    match error {
        ureq::Error::StatusCode(401 | 403) => Error::Unauthorized,
        ureq::Error::StatusCode(429) => Error::RateLimited,
        ureq::Error::StatusCode(code) => Error::HttpStatus { code },
        _ => Error::ConnectionFailed,
    }
}

#[cfg(test)]
pub(crate) mod test_server {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::Duration;

    pub(crate) const TEST_TIMEOUT: Duration = Duration::from_millis(250);

    pub(crate) fn serve(status: &str, body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let head = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        let body = body.to_vec();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(&head);
            let _ = stream.write_all(&body);
        });

        format!("http://{address}")
    }

    pub(crate) fn serve_silently() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();

        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(750));
            drop(stream);
        });

        format!("http://{address}")
    }

    pub(crate) fn refuse() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        format!("http://{address}")
    }
}

#[cfg(test)]
mod tests {
    use super::test_server::{TEST_TIMEOUT, refuse, serve, serve_silently};
    use super::{Error, get_bytes, get_text, post_text};

    #[test]
    fn a_rejected_token_is_unauthorized() {
        let base = serve("401 Unauthorized", b"");

        let error = get_text(&base, &[], Some("bad-token"), TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::Unauthorized));
    }

    #[test]
    fn http_429_is_rate_limited() {
        let base = serve("429 Too Many Requests", b"");

        let error = get_text(&base, &[], None, TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::RateLimited));
    }

    #[test]
    fn other_statuses_report_their_code() {
        let base = serve("500 Internal Server Error", b"");

        let error = get_text(&base, &[], None, TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::HttpStatus { code: 500 }));
    }

    #[test]
    fn a_refused_connection_is_a_connection_failure() {
        let error = get_text(&refuse(), &[], None, TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn a_silent_server_times_out() {
        let error = get_text(&serve_silently(), &[], None, TEST_TIMEOUT).unwrap_err();

        assert!(matches!(error, Error::ConnectionFailed));
    }

    #[test]
    fn bytes_are_read_up_to_the_limit() {
        let base = serve("200 OK", b"image-bytes");

        assert_eq!(
            get_bytes(&base, TEST_TIMEOUT, 1024).unwrap(),
            b"image-bytes"
        );
    }

    #[test]
    fn a_body_over_the_limit_is_an_error() {
        let base = serve("200 OK", b"image-bytes");

        assert!(get_bytes(&base, TEST_TIMEOUT, 4).is_err());
    }

    #[test]
    fn a_post_returns_the_body() {
        let base = serve("200 OK", b"posted");

        assert_eq!(
            post_text(&base, &[], &[("X-Test", "yes")], "query body", TEST_TIMEOUT).unwrap(),
            "posted"
        );
    }

    #[test]
    fn a_rejected_post_is_unauthorized() {
        let base = serve("401 Unauthorized", b"");

        assert!(matches!(
            post_text(&base, &[], &[], "", TEST_TIMEOUT),
            Err(Error::Unauthorized)
        ));
    }
}
