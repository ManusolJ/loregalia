#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::AppContext;
use gpui_kit::assets::AllAssets;
use loregalia_app::shell::Shell;
use loregalia_app::single_instance::{self, Instance};
use loregalia_app::startup::{
    Startup, StorageState, install_panic_hook, open_secrets, prepare, record, window_open_failure,
};

fn main() {
    let instance = single_instance::claim(&single_instance::current_instance_name());
    if matches!(instance, Ok(Instance::Secondary)) {
        return;
    }

    let Startup {
        storage,
        log_path,
        theme,
    } = prepare();
    if let StorageState::Failed(message) = &storage {
        eprintln!("{message}");
    }
    let secrets = open_secrets(&storage, &log_path);
    let listener = match instance {
        Ok(Instance::Primary(listener)) => Some(listener),
        Ok(Instance::Secondary) => None,
        Err(error) => {
            record(
                &log_path,
                &format!("Could not check for another open window: {error}"),
            );
            None
        }
    };

    install_panic_hook(&log_path);
    let error_log_path = log_path.clone();

    gpui_kit::application()
        .with_assets(AllAssets)
        .run(move |cx| {
            loregalia_app::init(cx, theme);

            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let options = loregalia_app::window_options(cx);
            match gpui_kit::open_window(options, cx, |_, cx| {
                cx.new(|cx| Shell::new(storage, log_path, theme, secrets, cx))
            }) {
                Ok((window, _)) => {
                    if let Some(listener) = listener {
                        loregalia_app::focus_on_request(window, listener, cx);
                    }
                }
                Err(error) => {
                    let message = window_open_failure(&error);
                    eprintln!("{message}");
                    record(&error_log_path, &message);
                    cx.quit();
                }
            }
        });
}
