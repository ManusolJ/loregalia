#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::AppContext;
use gpui_kit::assets::AllAssets;
use loregalia_app::shell::Shell;
use loregalia_app::single_instance::{self, Instance};
use loregalia_app::startup::{
    Startup, StorageState, install_panic_hook, open_secrets, prepare, record, window_open_failure,
};
use loregalia_core::updates;
use std::sync::Arc;

fn main() {
    let instance = single_instance::claim(&single_instance::current_instance_name());
    if matches!(instance, Ok(Instance::Secondary)) {
        return;
    }

    let Startup {
        storage,
        log_path,
        theme,
        window,
        damaged_library,
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
            loregalia_app::use_cover_cache(&storage, cx);

            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let options = loregalia_app::window_options(window, cx);
            match gpui_kit::open_window(options, cx, |window, cx| {
                loregalia_app::save_placement_on_close(&storage, log_path.clone(), window, cx);
                cx.new(|cx| {
                    let mut shell = Shell::new(storage, log_path, theme, secrets, cx);
                    shell.offer_restore(damaged_library);
                    shell.check_for_updates(Arc::new(updates::latest), cx);
                    shell
                })
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
