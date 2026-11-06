use gpui_kit::component::ThemeMode;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Pixels, Point, Result, Role, Size, TestAppContext,
    WindowBounds, WindowOptions, px, size,
};
use loregalia_app::shell::Shell;
use loregalia_app::startup::StorageState;
use loregalia_app::theme;
use loregalia_core::{Database, Result as CoreResult, lists};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

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

fn seed_lists(path: &Path, names: &[&str]) -> CoreResult<Vec<lists::List>> {
    let database = Database::open(path)?;
    for name in names {
        lists::create(&database, name)?;
    }
    lists::all(&database)
}

#[gpui_kit::test]
fn wide_window_shows_the_full_sidebar_and_empty_state(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(1280.0), px(800.0)), ready(&temporary), None).unwrap();
    cx.run_until_parked();

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
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(window.find("sidebar").bounds().size.width, px(64.0));
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
        assert!(window.try_find("loading-state").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn loading_shows_before_the_list_load_finishes(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(1280.0), px(800.0)), ready(&temporary), None).unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let loading = window.find("loading-state");
        assert!(loading.visible());
        assert_eq!(loading.role(), Some(Role::Status));
        assert_eq!(loading.label(), Some("Loading lists"));
        assert!(window.try_find("empty-state").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn empty_state_shows_after_the_load_finishes_empty(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(1280.0), px(800.0)), ready(&temporary), None).unwrap();

    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.find("empty-state").visible());
        assert!(window.try_find("loading-state").is_none());
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

#[gpui_kit::test]
fn a_failed_log_write_does_not_change_the_ui(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let database_path = temporary.path().join("loregalia.db");
    std::fs::create_dir_all(&database_path).unwrap();
    let log_path = temporary.path().join("loregalia.log");
    std::fs::create_dir_all(&log_path).unwrap();

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
        assert!(window.try_find("loading-state").is_none());
    })
    .unwrap();

    assert!(log_path.is_dir());
}

#[gpui_kit::test]
fn rail_shows_every_list_and_create_with_labels(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Watch later", "Reading", "Games"]).unwrap();

    let handle = open_shell(
        cx,
        size(px(960.0), px(600.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(window.find("sidebar").bounds().size.width, px(64.0));
        assert!(window.find("lists").bounds().size.width >= px(32.0));
        assert_eq!(window.find("lists").role(), Some(Role::List));

        for list in &seeded {
            let row = window.find(format!("list-row-{}", list.id));
            assert!(row.visible());
            assert_eq!(row.role(), Some(Role::ListItem));
            assert_eq!(row.label(), Some(list.name.as_str()));

            let button = window.find(format!("list-button-{}", list.id));
            assert!(button.visible());
            assert_eq!(button.label(), Some(list.name.as_str()));
            assert_eq!(button.bounds().size, size(px(32.0), px(32.0)));
        }

        let create = window.find("create-list");
        assert!(create.visible());
        assert_eq!(create.label(), Some("Create list"));
        assert_eq!(create.bounds().size, size(px(32.0), px(32.0)));
    })
    .unwrap();
}

#[gpui_kit::test]
fn creating_a_list_works_in_the_rail(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");

    let handle = open_shell(
        cx,
        size(px(960.0), px(600.0)),
        StorageState::Ready(path.clone()),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("create-list", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let created = lists::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].name, "New list");

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(format!("list-row-{}", created[0].id)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn create_stays_reachable_with_fifty_lists(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let names: Vec<String> = (0..50).map(|index| format!("List {index}")).collect();
    let seeded: Vec<&str> = names.iter().map(String::as_str).collect();
    seed_lists(&path, &seeded).unwrap();

    let handle = open_shell(
        cx,
        size(px(960.0), px(800.0)),
        StorageState::Ready(path.clone()),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let create = window.find("create-list");
        assert!(create.visible());
        assert_eq!(create.bounds().size, size(px(32.0), px(32.0)));

        window.click("create-list", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let names: Vec<String> = lists::all(&Database::open(&path).unwrap())
        .unwrap()
        .into_iter()
        .map(|list| list.name)
        .collect();
    assert_eq!(names.len(), 51);
    assert_eq!(names.last().map(String::as_str), Some("New list"));
}

#[gpui_kit::test]
fn row_ids_come_from_the_list_id(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Watch later", "Reading", "Games"]).unwrap();

    let handle = open_shell(
        cx,
        size(px(960.0), px(600.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        for list in &seeded {
            assert!(window.find(format!("list-row-{}", list.id)).visible());
        }
        assert!(window.try_find("list-row-0").is_none());
    })
    .unwrap();
}
