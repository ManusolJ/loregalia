use std::future::Future;
use std::pin::Pin;

use gpui_kit::BackgroundExecutor;
use gpui_kit::http_client::{
    AsyncBody, HttpClient, Method, Request, Response, StatusCode, Url, http::HeaderValue,
};
use loregalia_core::images::{self, CoverCache};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

pub struct ImageHttpClient {
    executor: BackgroundExecutor,
    covers: Option<CoverCache>,
    download: Download,
}

pub type Download = Arc<dyn Fn(&str) -> loregalia_core::Result<Vec<u8>> + Send + Sync>;

pub const COVER_RETRIES: [Duration; 2] = [Duration::from_secs(5), Duration::from_secs(20)];

impl ImageHttpClient {
    pub fn new(executor: BackgroundExecutor) -> Self {
        Self::with_download(executor, Arc::new(images::fetch))
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
            download: Arc::new(images::fetch),
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
        let download = self.download.clone();

        Box::pin(async move {
            let mut waits = COVER_RETRIES.into_iter();
            let bytes = loop {
                let (url, covers, download) = (url.clone(), covers.clone(), download.clone());
                let outcome = executor
                    .spawn(async move {
                        match covers {
                            Some(covers) => {
                                covers.fetch(&url, SystemTime::now(), download.as_ref())
                            }
                            None => download(&url),
                        }
                    })
                    .await;
                match (outcome, waits.next()) {
                    (Err(error), Some(wait)) if error.is_temporary() => {
                        executor.timer(wait).await;
                    }
                    (outcome, _) => break outcome?,
                }
            };
            Response::builder()
                .status(StatusCode::OK)
                .body(AsyncBody::from(bytes))
                .map_err(|error| gpui_kit::http_client::anyhow!("{error}"))
        })
    }
}
