use gpui_kit::component::ThemeMode;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Pixels, Point, Result, Role, Size, TestAppContext,
    WindowBounds, WindowOptions, px, size,
};
use loregalia_app::shell::Shell;
use loregalia_app::startup::StorageState;
use loregalia_app::theme;
use loregalia_core::{Database, lists};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Option<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "loregalia-shell-{}-{nanos}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).ok()?;
        Some(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn open_shell(
    cx: &mut TestAppContext,
    window_size: Size<Pixels>,
    storage: StorageState,
    log_path: Option<PathBuf>,
) -> Result<AnyWindowHandle> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::apply(ThemeMode::Dark, cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: window_size,
            })),
            ..Default::default()
        };
        let (window, _) = gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|cx| Shell::new(storage, log_path, cx))
        })?;

        Ok(window)
    })
}

fn ready(temporary: &TempDir) -> StorageState {
    StorageState::Ready(temporary.path().join("loregalia.db"))
}

#[gpui_kit::test]
fn wide_window_shows_the_full_sidebar_and_empty_state(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(1280.0), px(800.0)), ready(&temporary), None).unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(window.find("sidebar").bounds().size.width, px(240.0));

        let empty_state = window.find("empty-state");
        assert!(empty_state.visible());
        assert_eq!(empty_state.role(), Some(Role::Status));
        assert_eq!(empty_state.label(), Some("Nothing planned yet"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn narrow_window_collapses_the_sidebar_to_a_rail(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(960.0), px(600.0)), ready(&temporary), None).unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(window.find("sidebar").bounds().size.width, px(56.0));
        assert!(window.find("empty-state").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn storage_failure_is_shown_as_an_alert(cx: &mut TestAppContext) {
    let message = "Could not open the database: the disk is full";
    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Failed(message.to_owned()),
        None,
    )
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let alert = window.find("storage-error");
        assert!(alert.visible());
        assert_eq!(alert.role(), Some(Role::Alert));
        assert_eq!(alert.label(), Some(message));
        assert!(window.try_find("empty-state").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn creating_a_list_writes_and_reads_it_back(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    {
        let database = Database::open(&path).unwrap();
        lists::create(&database, "Existing list").unwrap();
    }

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path.clone()),
        None,
    )
    .unwrap();

    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find_by_label("Existing list").visible());
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.click("create-list", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find_by_label("New list").visible());
    })
    .unwrap();

    let database = Database::open(&path).unwrap();
    let names: Vec<String> = lists::all(&database)
        .unwrap()
        .into_iter()
        .map(|list| list.name)
        .collect();
    assert_eq!(names, ["Existing list", "New list"]);
}

#[gpui_kit::test]
fn list_failures_are_shown_and_logged(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let database_path = temporary.path().join("loregalia.db");
    std::fs::create_dir_all(&database_path).unwrap();
    let log_path = temporary.path().join("loregalia.log");

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(database_path),
        Some(log_path.clone()),
    )
    .unwrap();

    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let alert = window.find("lists-error");
        assert!(alert.visible());
        assert_eq!(alert.role(), Some(Role::Alert));
    })
    .unwrap();

    let contents = std::fs::read_to_string(&log_path).unwrap();
    assert!(contents.contains("Could not update lists"));
}
