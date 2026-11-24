use crate::{Error, Result};
use std::sync::LazyLock;
use std::time::Duration;

pub(crate) const USER_AGENT: &str = concat!(
    "Loregalia/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ManusolJ/loregalia)"
);
pub(crate) const TIMEOUT: Duration = Duration::from_secs(10);

const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);
const RETRY_PAUSE: Duration = Duration::from_millis(200);

static AGENT: LazyLock<ureq::Agent> = LazyLock::new(|| {
    ureq::Agent::config_builder()
        .timeout_resolve(Some(CONNECT_TIMEOUT))
        .timeout_connect(Some(CONNECT_TIMEOUT))
        .build()
        .into()
});

type Outcome<T> = std::result::Result<T, ureq::Error>;

pub(crate) fn get_text(
    url: &str,
    query: &[(&str, &str)],
    bearer_token: Option<&str>,
    timeout: Duration,
) -> Result<String> {
    with_retry(|| {
        send(url, query, bearer_token, timeout)?
            .body_mut()
            .read_to_string()
    })
}

pub(crate) fn get_secure_bytes(url: &str, timeout: Duration, max_bytes: u64) -> Result<Vec<u8>> {
    with_retry(|| {
        let response = AGENT
            .get(url)
            .config()
            .https_only(true)
            .timeout_global(Some(timeout))
            .build()
            .header("User-Agent", USER_AGENT)
            .call()?;

        read_bytes(response, max_bytes)
    })
}

pub(crate) fn get_bytes(url: &str, timeout: Duration, max_bytes: u64) -> Result<Vec<u8>> {
    with_retry(|| read_bytes(send(url, &[], None, timeout)?, max_bytes))
}

pub(crate) fn post_text(
    url: &str,
    query: &[(&str, &str)],
    headers: &[(&str, &str)],
    body: &str,
    timeout: Duration,
) -> Result<String> {
    with_retry(|| {
        post(url, query, headers, body, timeout)?
            .body_mut()
            .read_to_string()
    })
}

fn with_retry<T>(attempt: impl Fn() -> Outcome<T>) -> Result<T> {
    match attempt() {
        Err(error) if never_answered(&error) => {
            std::thread::sleep(RETRY_PAUSE);
            attempt()
        }
        outcome => outcome,
    }
    .map_err(classify)
}

fn never_answered(error: &ureq::Error) -> bool {
    matches!(
        error,
        ureq::Error::Timeout(ureq::Timeout::Resolve | ureq::Timeout::Connect)
            | ureq::Error::ConnectionFailed
            | ureq::Error::HostNotFound
            | ureq::Error::Io(_)
    )
}

fn read_bytes(mut response: ureq::http::Response<ureq::Body>, max_bytes: u64) -> Outcome<Vec<u8>> {
    response
        .body_mut()
        .with_config()
        .limit(max_bytes)
        .read_to_vec()
}

fn send(
    url: &str,
    query: &[(&str, &str)],
    bearer_token: Option<&str>,
    timeout: Duration,
) -> Outcome<ureq::http::Response<ureq::Body>> {
    let mut request = AGENT
        .get(url)
        .config()
        .timeout_global(Some(timeout))
        .build()
        .header("User-Agent", USER_AGENT);

    if let Some(token) = bearer_token {
        request = request.header("Authorization", &format!("Bearer {token}"));
    }
    for (name, value) in query {
        request = request.query(name, value);
    }

    request.call()
}

fn post(
    url: &str,
    query: &[(&str, &str)],
    headers: &[(&str, &str)],
    body: &str,
    timeout: Duration,
) -> Outcome<ureq::http::Response<ureq::Body>> {
    let mut request = AGENT
        .post(url)
        .config()
        .timeout_global(Some(timeout))
        .build()
        .header("User-Agent", USER_AGENT);

    for &(name, value) in headers {
        request = request.header(name, value);
    }
    for &(name, value) in query {
        request = request.query(name, value);
    }

    request.send(body)
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
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    pub(crate) const TEST_TIMEOUT: Duration = Duration::from_millis(250);

    fn declared_length(head: &[u8]) -> usize {
        String::from_utf8_lossy(head)
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.trim()
                    .eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().ok())
                    .flatten()
            })
            .unwrap_or(0)
    }

    fn read_request(stream: &mut TcpStream) {
        let mut received = Vec::new();
        let mut chunk = [0_u8; 1024];

        while let Ok(read) = stream.read(&mut chunk) {
            if read == 0 {
                return;
            }
            received.extend_from_slice(&chunk[..read]);
            if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n")
                && received.len() >= end + 4 + declared_length(&received[..end])
            {
                return;
            }
        }
    }

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
            read_request(&mut stream);
            let _ = stream.write_all(&head);
            let _ = stream.write_all(&body);
        });

        format!("http://{address}")
    }

    pub(crate) fn serve_after_dropping(drops: usize, body: &[u8]) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let connections = Arc::new(AtomicUsize::new(0));
        let seen = connections.clone();
        let response = [
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes(),
            body.to_vec(),
        ]
        .concat();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                read_request(&mut stream);
                if seen.fetch_add(1, Ordering::SeqCst) >= drops {
                    let _ = stream.write_all(&response);
                }
            }
        });

        (format!("http://{address}"), connections)
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

    pub(crate) fn serve_on_one_connection(responses: usize, body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = [
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).into_bytes(),
            body.to_vec(),
        ]
        .concat();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            for _ in 0..responses {
                read_request(&mut stream);
                let _ = stream.write_all(&response);
            }
            std::thread::sleep(Duration::from_millis(500));
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
    use super::test_server::{
        TEST_TIMEOUT, refuse, serve, serve_after_dropping, serve_on_one_connection, serve_silently,
    };
    use super::{Error, get_bytes, get_text, post_text, with_retry};
    use std::cell::Cell;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    const PATIENT: Duration = Duration::from_secs(5);

    fn failing_once(error: ureq::Error) -> (u32, bool) {
        let attempts = Cell::new(0);
        let first = Cell::new(Some(error));
        let result = with_retry(|| {
            attempts.set(attempts.get() + 1);
            match first.take() {
                Some(error) => Err(error),
                None => Ok(()),
            }
        });

        (attempts.get(), result.is_ok())
    }

    #[test]
    fn a_dropped_connection_is_tried_once_more() {
        let (base, connections) = serve_after_dropping(1, b"ok");

        assert_eq!(get_text(&base, &[], None, PATIENT).unwrap(), "ok");
        assert_eq!(connections.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_dropped_post_and_a_dropped_cover_are_tried_once_more() {
        let (base, _) = serve_after_dropping(1, b"posted");
        assert_eq!(
            post_text(&base, &[], &[], "query", PATIENT).unwrap(),
            "posted"
        );

        let (base, _) = serve_after_dropping(1, b"cover");
        assert_eq!(get_bytes(&base, PATIENT, 1024).unwrap(), b"cover");
    }

    #[test]
    fn a_connection_that_keeps_dropping_is_given_up_after_two_tries() {
        let (base, connections) = serve_after_dropping(9, b"ok");

        assert!(matches!(
            get_text(&base, &[], None, PATIENT),
            Err(Error::ConnectionFailed)
        ));
        assert_eq!(connections.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_stalled_connection_is_tried_once_more() {
        for stall in [ureq::Timeout::Connect, ureq::Timeout::Resolve] {
            assert_eq!(failing_once(ureq::Error::Timeout(stall)), (2, true));
        }
        assert_eq!(failing_once(ureq::Error::ConnectionFailed), (2, true));
        assert_eq!(failing_once(ureq::Error::HostNotFound), (2, true));
    }

    #[test]
    fn an_answer_or_a_slow_server_is_not_asked_again() {
        for error in [
            ureq::Error::StatusCode(401),
            ureq::Error::StatusCode(404),
            ureq::Error::StatusCode(429),
            ureq::Error::StatusCode(503),
            ureq::Error::Timeout(ureq::Timeout::Global),
            ureq::Error::BodyExceedsLimit(4),
        ] {
            assert_eq!(failing_once(error), (1, false));
        }
    }

    #[test]
    fn requests_to_one_host_reuse_the_connection() {
        let base = serve_on_one_connection(2, b"ok");

        assert_eq!(get_text(&base, &[], None, TEST_TIMEOUT).unwrap(), "ok");
        assert_eq!(get_text(&base, &[], None, TEST_TIMEOUT).unwrap(), "ok");
    }

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
