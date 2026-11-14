use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use loregalia_core::Error as CoreError;
use loregalia_core::providers::relay::ErrorResponse;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayError {
    BadRequest,
    RateLimited,
    NotConfigured,
    NotFound,
    Cache,
    Internal,
    UpstreamBusy,
    UpstreamRejected,
    UpstreamRateLimited,
    UpstreamUnavailable,
}

impl RelayError {
    pub fn upstream(error: CoreError) -> Self {
        match error {
            CoreError::Unauthorized => Self::UpstreamRejected,
            CoreError::RateLimited => Self::UpstreamRateLimited,
            CoreError::HttpStatus { code: 404 } => Self::NotFound,
            _ => Self::UpstreamUnavailable,
        }
    }
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
            Self::UpstreamBusy => (StatusCode::SERVICE_UNAVAILABLE, "upstream_busy"),
            Self::UpstreamRejected => (StatusCode::BAD_GATEWAY, "upstream_credentials_rejected"),
            Self::UpstreamRateLimited => (StatusCode::SERVICE_UNAVAILABLE, "upstream_rate_limited"),
            Self::UpstreamUnavailable => (StatusCode::BAD_GATEWAY, "upstream_unavailable"),
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
            RelayError::upstream(CoreError::Unauthorized)
                .into_response()
                .status(),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn a_movie_tmdb_does_not_know_is_not_found() {
        assert_eq!(
            RelayError::upstream(CoreError::HttpStatus { code: 404 })
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            RelayError::upstream(CoreError::HttpStatus { code: 500 })
                .into_response()
                .status(),
            StatusCode::BAD_GATEWAY
        );
    }

    #[test]
    fn a_busy_upstream_queue_is_service_unavailable() {
        assert_eq!(
            RelayError::UpstreamBusy.into_response().status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            RelayError::upstream(CoreError::RateLimited)
                .into_response()
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
