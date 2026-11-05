pub mod shell;
pub mod theme;

use gpui_kit::component::ThemeMode;
use gpui_kit::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

const APP_ID: &str = "com.manu.loregalia";
const APP_TITLE: &str = "Loregalia";

pub fn init(cx: &mut App) {
    gpui_kit::init(cx);
    theme::apply(ThemeMode::Dark, cx);
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
        app_id: Some(APP_ID.to_owned()),
        ..Default::default()
    }
}
