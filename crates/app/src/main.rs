#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::AppContext;
use gpui_kit::assets::Assets;
use loregalia_app::shell::Shell;
use loregalia_app::startup::{
    Startup, StorageState, install_panic_hook, prepare, record, window_open_failure,
};

fn main() {
    let Startup { storage, log_path } = prepare();
    if let StorageState::Failed(message) = &storage {
        eprintln!("{message}");
    }

    install_panic_hook(&log_path);
    let error_log_path = log_path.clone();

    gpui_kit::application().with_assets(Assets).run(move |cx| {
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
            let message = window_open_failure(&error);
            eprintln!("{message}");
            record(&error_log_path, &message);
            cx.quit();
        }
    });
}
