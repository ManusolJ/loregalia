pub mod http;
pub mod shell;
pub mod single_instance;
pub mod startup;
pub mod theme;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Pixels, TitlebarOptions, Window, WindowBounds,
    WindowOptions, point, px, size,
};
use interprocess::local_socket::Listener;
use loregalia_core::images::CoverCache;
use loregalia_core::settings::Theme;
use loregalia_core::window::{self, Area, Placement};
use loregalia_core::{Database, items};

use crate::startup::{StorageState, record};
pub const APP_ID: &str = "com.manu.loregalia";

const APP_TITLE: &str = "Loregalia";

pub fn init(cx: &mut App, theme: Theme) {
    gpui_kit::init(cx);
    shell::bind_keys(cx);
    theme::apply_choice(theme, cx);
    cx.set_http_client(Arc::new(http::ImageHttpClient::new(
        cx.background_executor().clone(),
    )));
}

pub fn use_cover_cache(storage: &StorageState, cx: &mut App) {
    let StorageState::Ready(database_path) = storage else {
        return;
    };
    let Some(data_directory) = database_path.parent() else {
        return;
    };
    let covers = CoverCache::in_data_folder(data_directory);
    cx.set_http_client(Arc::new(http::ImageHttpClient::with_cover_cache(
        cx.background_executor().clone(),
        covers.clone(),
    )));

    let database_path = database_path.clone();
    cx.background_spawn(async move {
        if let Ok(database) = Database::open(&database_path)
            && let Ok(library) = items::all(&database)
        {
            let _ = covers.keep_only(library.iter().filter_map(|item| item.cover_url.as_deref()));
        }
    })
    .detach();
}

pub fn focus_on_request(window: AnyWindowHandle, listener: Listener, cx: &mut App) {
    let executor = cx.background_executor().clone();

    cx.spawn(async move |cx| {
        let mut listener = listener;
        loop {
            let (returned, request) = executor
                .spawn(async move {
                    let request = single_instance::wait_for_focus_request(&listener);
                    (listener, request)
                })
                .await;
            listener = returned;

            match request {
                Ok(true) => {
                    let _ = cx.update_window(window, |_, window, _| window.activate_window());
                }
                Ok(false) => {}
                Err(_) => break,
            }
        }
    })
    .detach();
}

const MIN_WIDTH: f32 = 960.0;
const MIN_HEIGHT: f32 = 600.0;

pub fn window_options(saved: Option<Placement>, cx: &App) -> WindowOptions {
    let displays = cx.displays();
    let areas: Vec<Area> = displays
        .iter()
        .map(|display| area_of(display.bounds()))
        .collect();
    let restored = saved.and_then(|placement| {
        let display = displays.get(placement.display_among(&areas)?)?;
        Some((placement, display.id()))
    });
    let (window_bounds, display_id) = match restored {
        Some((placement, display)) => (bounds_of(placement), Some(display)),
        None => (
            WindowBounds::Windowed(Bounds::centered(None, size(px(1280.0), px(800.0)), cx)),
            None,
        ),
    };

    WindowOptions {
        window_bounds: Some(window_bounds),
        display_id,
        window_min_size: Some(size(px(MIN_WIDTH), px(MIN_HEIGHT))),
        titlebar: Some(TitlebarOptions {
            title: Some(APP_TITLE.into()),
            ..Default::default()
        }),
        app_id: Some(APP_ID.to_owned()),
        ..Default::default()
    }
}

fn area_of(bounds: Bounds<Pixels>) -> Area {
    Area {
        x: bounds.origin.x.as_f32(),
        y: bounds.origin.y.as_f32(),
        width: bounds.size.width.as_f32(),
        height: bounds.size.height.as_f32(),
    }
}

fn bounds_of(placement: Placement) -> WindowBounds {
    let Area {
        x,
        y,
        width,
        height,
    } = placement.area;
    let bounds = Bounds {
        origin: point(px(x), px(y)),
        size: size(px(width.max(MIN_WIDTH)), px(height.max(MIN_HEIGHT))),
    };
    if placement.maximized {
        WindowBounds::Maximized(bounds)
    } else {
        WindowBounds::Windowed(bounds)
    }
}

pub fn placement_of(bounds: WindowBounds) -> Placement {
    let (area, maximized) = match bounds {
        WindowBounds::Windowed(bounds) => (bounds, false),
        WindowBounds::Maximized(bounds) | WindowBounds::Fullscreen(bounds) => (bounds, true),
    };
    Placement {
        area: area_of(area),
        maximized,
    }
}

pub fn save_placement_on_close(
    storage: &StorageState,
    log_path: Option<PathBuf>,
    window: &mut Window,
    cx: &App,
) {
    let StorageState::Ready(path) = storage.clone() else {
        return;
    };
    let saving = Rc::new(Cell::new(false));
    window.on_window_should_close(cx, move |window, cx| {
        if saving.replace(true) {
            return true;
        }
        let placement = placement_of(window.window_bounds());
        let path = path.clone();
        let log_path = log_path.clone();
        let handle = window.window_handle();
        let task = cx.background_spawn(async move {
            let database = Database::open(path)?;
            window::set_placement(&database, placement)
        });
        cx.spawn(async move |cx| {
            if let Err(error) = task.await {
                record(
                    &log_path,
                    &format!("Could not save the window size: {error}"),
                );
            }
            let _ = cx.update_window(handle, |_, window, _| window.remove_window());
        })
        .detach();
        false
    });
}

#[cfg(test)]
mod tests {
    use super::{bounds_of, placement_of};
    use gpui_kit::{Bounds, WindowBounds, point, px, size};
    use loregalia_core::window::{Area, Placement};

    fn bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<gpui_kit::Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(width), px(height)),
        }
    }

    #[test]
    fn a_closed_window_is_saved_with_its_restore_size() {
        let saved = placement_of(WindowBounds::Maximized(bounds(
            -1600.0, 20.0, 1400.0, 900.0,
        )));

        assert_eq!(
            saved,
            Placement {
                area: Area {
                    x: -1600.0,
                    y: 20.0,
                    width: 1400.0,
                    height: 900.0
                },
                maximized: true
            }
        );
        assert!(placement_of(WindowBounds::Fullscreen(bounds(0.0, 0.0, 1280.0, 800.0))).maximized);
        assert!(!placement_of(WindowBounds::Windowed(bounds(0.0, 0.0, 1280.0, 800.0))).maximized);
    }

    #[test]
    fn a_saved_window_reopens_the_same_way() {
        for closed in [
            WindowBounds::Maximized(bounds(10.0, 20.0, 1300.0, 820.0)),
            WindowBounds::Windowed(bounds(2000.0, 40.0, 1024.0, 700.0)),
        ] {
            assert_eq!(bounds_of(placement_of(closed)), closed);
        }
    }

    #[test]
    fn a_saved_window_never_opens_below_the_minimum_size() {
        let tiny = Placement {
            area: Area {
                x: 0.0,
                y: 0.0,
                width: 300.0,
                height: 200.0,
            },
            maximized: false,
        };

        assert_eq!(
            bounds_of(tiny),
            WindowBounds::Windowed(bounds(0.0, 0.0, 960.0, 600.0))
        );
    }
}
