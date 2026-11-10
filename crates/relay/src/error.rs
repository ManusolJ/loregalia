use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use loregalia_core::Error as CoreError;
use loregalia_core::providers::relay::ErrorResponse;

#[derive(Debug)]
pub enum RelayError {
    BadRequest,
    RateLimited,
    NotConfigured,
    NotFound,
    Cache,
    Internal,
    Upstream(CoreError),
}

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            Self::BadRequest => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
            Self::NotConfigured => (StatusCode::SERVICE_UNAVAILABLE, "not_configured"),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            Self::Cache => (StatusCode::INTERNAL_SERVER_ERROR, "cache_error"),
            Self::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            Self::Upstream(CoreError::Unauthorized) => {
                (StatusCode::BAD_GATEWAY, "upstream_credentials_rejected")
            }
            Self::Upstream(CoreError::RateLimited) => {
                (StatusCode::SERVICE_UNAVAILABLE, "upstream_rate_limited")
            }
            Self::Upstream(CoreError::HttpStatus { code: 404 }) => {
                (StatusCode::NOT_FOUND, "not_found")
            }
            Self::Upstream(_) => (StatusCode::BAD_GATEWAY, "upstream_unavailable"),
        };

        (
            status,
            Json(ErrorResponse {
                error: error.to_owned(),
            }),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::RelayError;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use loregalia_core::Error as CoreError;

    #[test]
    fn errors_map_to_http_statuses() {
        assert_eq!(
            RelayError::BadRequest.into_response().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            RelayError::RateLimited.into_response().status(),
            StatusCode::TOO_MANY_REQUESTS
        );
        assert_eq!(
            RelayError::NotConfigured.into_response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            RelayError::NotFound.into_response().status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            RelayError::Upstream(CoreError::Unauthorized)
                .into_response()
                .status(),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn a_movie_tmdb_does_not_know_is_not_found() {
        assert_eq!(
            RelayError::Upstream(CoreError::HttpStatus { code: 404 })
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            RelayError::Upstream(CoreError::HttpStatus { code: 500 })
                .into_response()
                .status(),
            StatusCode::BAD_GATEWAY
        );
    }
}
