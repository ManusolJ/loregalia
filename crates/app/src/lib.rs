pub mod http;
pub mod shell;
pub mod startup;
pub mod theme;

use std::sync::Arc;

use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use loregalia_core::settings::Theme;
use loregalia_core::storage::APP_FOLDER;

const APP_TITLE: &str = "Loregalia";

pub fn init(cx: &mut App, theme: Theme) {
    gpui_kit::init(cx);
    theme::apply_choice(theme, cx);
    cx.set_http_client(Arc::new(http::ImageHttpClient::new(
        cx.background_executor().clone(),
    )));
}

pub fn window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1280.0), px(800.0)),
            cx,
        ))),
        window_min_size: Some(size(px(960.0), px(600.0))),
        titlebar: Some(TitlebarOptions {
            title: Some(APP_TITLE.into()),
            ..Default::default()
        }),
        app_id: Some(APP_FOLDER.to_owned()),
        ..Default::default()
    }
}
