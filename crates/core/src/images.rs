use crate::Result;
use crate::http;

const MAX_BYTES: u64 = 5 * 1024 * 1024;

pub fn fetch(url: &str) -> Result<Vec<u8>> {
    http::get_bytes(url, http::TIMEOUT, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::fetch;
    use crate::Error;
    use crate::http::test_server::{refuse, serve};

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
}
