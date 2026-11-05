#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::AppContext;
use gpui_kit::assets::Assets;
use loregalia_app::shell::Shell;
use loregalia_app::startup::{Startup, StorageState, prepare};

fn main() {
    let Startup { storage, log_path } = prepare();
    if let StorageState::Failed(message) = &storage {
        eprintln!("{message}");
    }

    gpui_kit::application().with_assets(Assets).run(|cx| {
        loregalia_app::init(cx);

        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let options = loregalia_app::window_options(cx);
        if let Err(error) = gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|cx| Shell::new(storage, log_path, cx))
        }) {
            eprintln!("Failed to open the Loregalia window: {error}");
            cx.quit();
        }
    });
}
