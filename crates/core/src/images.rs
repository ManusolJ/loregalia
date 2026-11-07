use crate::{Error, Result};
use std::time::Duration;

const USER_AGENT: &str = concat!(
    "Loregalia/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/ManusolJ/loregalia)"
);
const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BYTES: u64 = 5 * 1024 * 1024;

pub fn fetch(url: &str) -> Result<Vec<u8>> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .build();
    let agent: ureq::Agent = config.into();
    let mut response = agent
        .get(url)
        .header("User-Agent", USER_AGENT)
        .call()
        .map_err(classify)?;

    response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES)
        .read_to_vec()
        .map_err(classify)
}

fn classify(error: ureq::Error) -> Error {
    match error {
        ureq::Error::StatusCode(429) => Error::RateLimited,
        ureq::Error::StatusCode(code) => Error::HttpStatus { code },
        _ => Error::ConnectionFailed,
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, fetch};
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn serve(status: &str, body: &[u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        let body = body.to_vec();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(&response);
            let _ = stream.write_all(&body);
        });

        format!("http://{address}")
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
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);

        assert!(matches!(
            fetch(&format!("http://{address}")),
            Err(Error::ConnectionFailed)
        ));
    }
}
