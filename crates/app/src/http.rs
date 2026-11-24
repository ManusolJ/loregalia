use std::future::Future;
use std::pin::Pin;

use gpui_kit::BackgroundExecutor;
use gpui_kit::http_client::{
    AsyncBody, HttpClient, Method, Request, Response, StatusCode, Url, http::HeaderValue,
};
use loregalia_core::images::{self, CoverCache};
use std::time::SystemTime;

pub struct ImageHttpClient {
    executor: BackgroundExecutor,
    covers: Option<CoverCache>,
    download: Download,
}

pub type Download = fn(&str) -> loregalia_core::Result<Vec<u8>>;

impl ImageHttpClient {
    pub fn new(executor: BackgroundExecutor) -> Self {
        Self::with_download(executor, images::fetch)
    }

    pub fn with_download(executor: BackgroundExecutor, download: Download) -> Self {
        Self {
            executor,
            covers: None,
            download,
        }
    }

    pub fn with_cover_cache(executor: BackgroundExecutor, covers: CoverCache) -> Self {
        Self {
            executor,
            covers: Some(covers),
            download: images::fetch,
        }
    }
}

type ResponseFuture = Pin<
    Box<dyn Future<Output = gpui_kit::http_client::Result<Response<AsyncBody>>> + Send + 'static>,
>;

impl HttpClient for ImageHttpClient {
    fn user_agent(&self) -> Option<&HeaderValue> {
        None
    }

    fn proxy(&self) -> Option<&Url> {
        None
    }

    fn send(&self, request: Request<AsyncBody>) -> ResponseFuture {
        if request.method() != Method::GET {
            return Box::pin(async {
                Err(gpui_kit::http_client::anyhow!("only GET is supported"))
            });
        }

        let url = request.uri().to_string();
        let executor = self.executor.clone();
        let covers = self.covers.clone();
        let download = self.download;

        Box::pin(async move {
            let bytes = executor
                .spawn(async move {
                    match covers {
                        Some(covers) => covers.fetch(&url, SystemTime::now(), &download),
                        None => download(&url),
                    }
                })
                .await?;
            Response::builder()
                .status(StatusCode::OK)
                .body(AsyncBody::from(bytes))
                .map_err(|error| gpui_kit::http_client::anyhow!("{error}"))
        })
    }
}
