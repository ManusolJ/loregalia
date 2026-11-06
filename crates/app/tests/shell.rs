use gpui_kit::component::ThemeMode;
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Pixels, Point, Result, Role, Size, TestAppContext,
    WindowBounds, WindowOptions, px, size,
};
use loregalia_app::shell::Shell;
use loregalia_app::startup::StorageState;
use loregalia_app::theme;
use loregalia_core::{
    Database, Result as CoreResult, items, list_items, lists, media::MediaKind, seasons, tags,
};
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

fn seed_item(
    path: &Path,
    media_kind: MediaKind,
    title: &str,
    release_year: Option<i32>,
) -> CoreResult<items::Item> {
    let database = Database::open(path)?;
    items::create(&database, media_kind, title, release_year)
}

fn add_membership(path: &Path, list_id: &str, item_id: &str) -> CoreResult<()> {
    let database = Database::open(path)?;
    list_items::add(&database, list_id, item_id)?;

    Ok(())
}

fn seed_tag(path: &Path, name: &str) -> CoreResult<tags::Tag> {
    let database = Database::open(path)?;
    tags::create(&database, name)
}

fn seed_season(path: &Path, show_id: &str, number: i32) -> CoreResult<seasons::Season> {
    let database = Database::open(path)?;
    seasons::create(&database, show_id, number)
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
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("list-editor").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "New list");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-list", cx);
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
    assert!(contents.contains("Could not update the database"));
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

        let first_top = window
            .find(format!("list-row-{}", seeded[0].id))
            .bounds()
            .origin
            .y;
        let second_top = window
            .find(format!("list-row-{}", seeded[1].id))
            .bounds()
            .origin
            .y;
        assert_eq!(second_top - first_top, px(36.0));

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
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("list-editor").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Weekend");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-list", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let created = lists::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].name, "Weekend");

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
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("list-editor").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "New list");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-list", cx);
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

#[gpui_kit::test]
fn rail_list_selection_opens_the_list_actions(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Watchlist"]).unwrap();
    let list = &seeded[0];

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
        window.click(format!("list-button-{}", list.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("list-panel").visible());
        window.click("delete-list", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("delete-confirmation").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn renaming_a_list_updates_the_sidebar_and_database(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Watchlist"]).unwrap();
    let list = &seeded[0];

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
        window.click(format!("list-row-{}", list.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("list-panel").visible());
        window.click("rename-list", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("list-name-input").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Weekend");

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-list", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(
            window.find(format!("list-row-{}", list.id)).label(),
            Some("Weekend")
        );
        assert_eq!(window.find("list-panel").label(), Some("Weekend"));
    })
    .unwrap();

    let stored = lists::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, list.id);
    assert_eq!(stored[0].name, "Weekend");
}

#[gpui_kit::test]
fn deleting_a_list_asks_for_confirmation(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Watchlist"]).unwrap();
    let list = &seeded[0];

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
        window.click(format!("list-row-{}", list.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("delete-list", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("delete-confirmation").visible());
        assert!(window.find(format!("list-row-{}", list.id)).visible());
        window.click("cancel-delete", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("delete-confirmation").is_none());
        assert!(window.find(format!("list-row-{}", list.id)).visible());
        window.click("delete-list", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(format!("list-row-{}", list.id)).is_none());
        assert!(window.find("empty-state").visible());
    })
    .unwrap();

    assert_eq!(
        lists::all(&Database::open(&path).unwrap()).unwrap(),
        Vec::new()
    );
}

#[gpui_kit::test]
fn adding_an_item_by_hand_shows_it(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");

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
        window.click("add-item", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-editor").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Piranesi");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("item-kind-book", cx);
        window.click("item-year-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "2020");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].media_kind, MediaKind::Book);
    assert_eq!(stored[0].title, "Piranesi");
    assert_eq!(stored[0].release_year, Some(2020));

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(format!("item-row-{}", stored[0].id)).label(),
            Some("Piranesi")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn editing_an_item_updates_it(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Movie, "Dune", Some(1984)).unwrap();

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
        window.click(format!("item-edit-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-editor").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Dune Messiah");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("item-kind-book", cx);
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, item.id);
    assert_eq!(stored[0].title, "Dune Messiah");
    assert_eq!(stored[0].media_kind, MediaKind::Book);
    assert_eq!(stored[0].release_year, Some(1984));
}

#[gpui_kit::test]
fn item_rows_show_the_status_chip(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Movie, "Dune", Some(2021)).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let chip = window.find(format!("item-status-{}", item.id));
        assert!(chip.visible());
        assert_eq!(chip.label(), Some("To watch"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_saved_status_is_shown_after_reopening(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Book, "Piranesi", Some(2020)).unwrap();
    {
        let database = Database::open(&path).unwrap();
        items::set_status(&database, &item.id, "book-done").unwrap();
    }

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(
            window.find(format!("item-status-{}", item.id)).label(),
            Some("Read")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn changing_an_items_status_updates_the_chip_and_database(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Movie, "Dune", Some(2021)).unwrap();

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
        window.click(format!("item-status-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find(format!("item-status-{}-movie-done", item.id))
                .visible()
        );
        window.click(format!("item-status-{}-movie-done", item.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(
            window.find(format!("item-status-{}", item.id)).label(),
            Some("Watched")
        );
    })
    .unwrap();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored[0].status_id, "movie-done");
    assert!(stored[0].finished_at.is_some());
}

#[gpui_kit::test]
fn the_status_picker_offers_only_the_items_media_kind(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Book, "Piranesi", Some(2020)).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("item-status-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.find(format!("status-picker-{}", item.id)).visible());
        assert_eq!(
            window
                .find(format!("item-status-{}-book-planned", item.id))
                .label(),
            Some("To read")
        );
        assert_eq!(
            window
                .find(format!("item-status-{}-book-done", item.id))
                .label(),
            Some("Read")
        );
        assert!(
            window
                .try_find(format!("item-status-{}-movie-done", item.id))
                .is_none()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn clicking_the_chip_again_closes_the_status_picker(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Game, "Outer Wilds", None).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("item-status-{}", item.id), cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(format!("status-picker-{}", item.id)).visible());
        window.click(format!("item-status-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find(format!("status-picker-{}", item.id))
                .is_none()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn deleting_an_item_asks_for_confirmation(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Game, "Outer Wilds", None).unwrap();

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
        window.click(format!("item-delete-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-delete-confirmation").visible());
        window.click("cancel-item-delete", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("item-delete-confirmation").is_none());
        window.click(format!("item-delete-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-item-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find(format!("item-row-{}", item.id)).is_none());
        assert!(window.find("empty-state").visible());
    })
    .unwrap();

    assert_eq!(
        items::all(&Database::open(&path).unwrap()).unwrap(),
        Vec::new()
    );
}

#[gpui_kit::test]
fn selecting_a_list_shows_only_its_items_in_order(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue", "Other"]).unwrap();
    let first = seed_item(&path, MediaKind::Movie, "First", None).unwrap();
    let second = seed_item(&path, MediaKind::Book, "Second", None).unwrap();
    let outside = seed_item(&path, MediaKind::Game, "Outside", None).unwrap();
    add_membership(&path, &seeded[0].id, &first.id).unwrap();
    add_membership(&path, &seeded[0].id, &second.id).unwrap();
    add_membership(&path, &seeded[1].id, &outside.id).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(window.find("list-panel").role(), Some(Role::Region));
        assert!(window.find(format!("item-row-{}", first.id)).visible());
        assert!(window.find(format!("item-row-{}", second.id)).visible());
        assert!(
            window
                .try_find(format!("item-row-{}", outside.id))
                .is_none()
        );

        let first_top = window
            .find(format!("item-row-{}", first.id))
            .bounds()
            .origin
            .y;
        let second_top = window
            .find(format!("item-row-{}", second.id))
            .bounds()
            .origin
            .y;
        assert!(first_top < second_top);
    })
    .unwrap();
}

#[gpui_kit::test]
fn an_empty_list_shows_its_own_empty_state(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let empty = window.find("list-empty");
        assert!(empty.visible());
        assert_eq!(empty.label(), Some("No items in this list yet"));
        assert!(window.try_find("empty-state").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn adding_an_existing_item_to_a_list_writes_it(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let item = seed_item(&path, MediaKind::Movie, "Dune", None).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("add-existing-item", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.find("add-picker").visible());
        window.click(format!("add-to-list-{}", item.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(format!("item-row-{}", item.id)).visible());
    })
    .unwrap();

    let stored = list_items::all(&Database::open(&path).unwrap(), &seeded[0].id).unwrap();
    assert_eq!(stored, vec![item]);
}

#[gpui_kit::test]
fn the_add_picker_skips_items_already_in_the_list(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let inside = seed_item(&path, MediaKind::Movie, "Inside", None).unwrap();
    let outside = seed_item(&path, MediaKind::Book, "Outside", None).unwrap();
    add_membership(&path, &seeded[0].id, &inside.id).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("add-existing-item", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(
            window
                .try_find(format!("add-to-list-{}", inside.id))
                .is_none()
        );
        assert!(window.find(format!("add-to-list-{}", outside.id)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn removing_an_item_from_a_list_keeps_the_item(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let item = seed_item(&path, MediaKind::Game, "Outer Wilds", None).unwrap();
    add_membership(&path, &seeded[0].id, &item.id).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("remove-from-list-{}", item.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.try_find(format!("item-row-{}", item.id)).is_none());
        assert!(window.find("list-empty").visible());
    })
    .unwrap();

    let database = Database::open(&path).unwrap();
    assert_eq!(
        list_items::all(&database, &seeded[0].id).unwrap(),
        Vec::new()
    );
    assert_eq!(items::all(&database).unwrap(), vec![item]);
}

#[gpui_kit::test]
fn moving_a_list_item_down_changes_the_order(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let first = seed_item(&path, MediaKind::Movie, "First", None).unwrap();
    let second = seed_item(&path, MediaKind::Book, "Second", None).unwrap();
    add_membership(&path, &seeded[0].id, &first.id).unwrap();
    add_membership(&path, &seeded[0].id, &second.id).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("move-down-{}", first.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let first_top = window
            .find(format!("item-row-{}", first.id))
            .bounds()
            .origin
            .y;
        let second_top = window
            .find(format!("item-row-{}", second.id))
            .bounds()
            .origin
            .y;
        assert!(second_top < first_top);
    })
    .unwrap();

    let stored: Vec<String> = list_items::all(&Database::open(&path).unwrap(), &seeded[0].id)
        .unwrap()
        .into_iter()
        .map(|item| item.title)
        .collect();
    assert_eq!(stored, ["Second", "First"]);
}

#[gpui_kit::test]
fn moving_a_list_item_up_changes_the_order(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let first = seed_item(&path, MediaKind::Movie, "First", None).unwrap();
    let second = seed_item(&path, MediaKind::Book, "Second", None).unwrap();
    let third = seed_item(&path, MediaKind::Game, "Third", None).unwrap();
    add_membership(&path, &seeded[0].id, &first.id).unwrap();
    add_membership(&path, &seeded[0].id, &second.id).unwrap();
    add_membership(&path, &seeded[0].id, &third.id).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("move-up-{}", third.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored: Vec<String> = list_items::all(&Database::open(&path).unwrap(), &seeded[0].id)
        .unwrap()
        .into_iter()
        .map(|item| item.title)
        .collect();
    assert_eq!(stored, ["First", "Third", "Second"]);
}

#[gpui_kit::test]
fn list_item_move_buttons_stop_at_the_edges(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let first = seed_item(&path, MediaKind::Movie, "First", None).unwrap();
    let second = seed_item(&path, MediaKind::Book, "Second", None).unwrap();
    add_membership(&path, &seeded[0].id, &first.id).unwrap();
    add_membership(&path, &seeded[0].id, &second.id).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(format!("move-up-{}", first.id)).visible());
        assert!(window.find(format!("move-down-{}", second.id)).visible());
        window.click(format!("move-up-{}", first.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("move-down-{}", second.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored: Vec<String> = list_items::all(&Database::open(&path).unwrap(), &seeded[0].id)
        .unwrap()
        .into_iter()
        .map(|item| item.title)
        .collect();
    assert_eq!(stored, ["First", "Second"]);
}

#[gpui_kit::test]
fn editing_an_item_from_a_list_returns_to_the_list(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let item = seed_item(&path, MediaKind::Movie, "Dune", Some(1984)).unwrap();
    add_membership(&path, &seeded[0].id, &item.id).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("item-edit-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-editor").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Dune Messiah");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.find("list-panel").visible());
        assert_eq!(
            window.find(format!("item-row-{}", item.id)).label(),
            Some("Dune Messiah")
        );
    })
    .unwrap();

    let stored = list_items::all(&Database::open(&path).unwrap(), &seeded[0].id).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].title, "Dune Messiah");
}

#[gpui_kit::test]
fn creating_an_item_from_a_list_adds_it_to_that_list(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("add-item", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-editor").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Piranesi");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = list_items::all(&Database::open(&path).unwrap(), &seeded[0].id).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].title, "Piranesi");

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(format!("item-row-{}", stored[0].id)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn adding_a_tag_to_an_item_shows_a_chip(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Movie, "Dune", None).unwrap();

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
        window.click(format!("item-tags-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("tag-picker").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Sci-Fi");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-tag", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = tags::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].name, "Sci-Fi");

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let chip = window.find(format!("item-tag-{}-{}", item.id, stored[0].id));
        assert!(chip.visible());
        assert_eq!(chip.label(), Some("Sci-Fi"));
    })
    .unwrap();

    let attached = tags::for_item(&Database::open(&path).unwrap(), &item.id).unwrap();
    assert_eq!(attached, stored);
}

#[gpui_kit::test]
fn creating_a_tag_on_a_list_attaches_it(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();

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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("list-tags", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "Weekend");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-tag", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = tags::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let chip = window.find(format!("list-tag-{}-{}", seeded[0].id, stored[0].id));
        assert!(chip.visible());
        assert_eq!(chip.label(), Some("Weekend"));
    })
    .unwrap();

    let attached = tags::for_list(&Database::open(&path).unwrap(), &seeded[0].id).unwrap();
    assert_eq!(attached, stored);
}

#[gpui_kit::test]
fn toggling_an_existing_tag_on_an_item_attaches_and_detaches_it(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Book, "Piranesi", None).unwrap();
    let tag = seed_tag(&path, "Favourite").unwrap();

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
        window.click(format!("item-tags-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find(format!("tag-toggle-{}", tag.id)).visible());
        window.click(format!("tag-toggle-{}", tag.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find(format!("item-tag-{}-{}", item.id, tag.id))
                .visible()
        );
        window.click(format!("tag-toggle-{}", tag.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .try_find(format!("item-tag-{}-{}", item.id, tag.id))
                .is_none()
        );
    })
    .unwrap();

    let attached = tags::for_item(&Database::open(&path).unwrap(), &item.id).unwrap();
    assert!(attached.is_empty());
}

#[gpui_kit::test]
fn a_tag_can_be_shared_by_an_item_and_a_list(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let item = seed_item(&path, MediaKind::Movie, "Dune", None).unwrap();
    let tag = seed_tag(&path, "Sci-Fi").unwrap();
    add_membership(&path, &seeded[0].id, &item.id).unwrap();
    {
        let database = Database::open(&path).unwrap();
        tags::attach_to_item(&database, &item.id, &tag.id).unwrap();
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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("list-tags", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("tag-toggle-{}", tag.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(
            window
                .find(format!("item-tag-{}-{}", item.id, tag.id))
                .visible()
        );
        assert!(
            window
                .find(format!("list-tag-{}-{}", seeded[0].id, tag.id))
                .visible()
        );
    })
    .unwrap();

    let database = Database::open(&path).unwrap();
    assert_eq!(
        tags::for_item(&database, &item.id).unwrap(),
        vec![tag.clone()]
    );
    assert_eq!(tags::for_list(&database, &seeded[0].id).unwrap(), vec![tag]);
}

#[gpui_kit::test]
fn a_show_row_offers_seasons_and_other_kinds_do_not(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let show = seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();
    let movie = seed_item(&path, MediaKind::Movie, "Dune", None).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.find(format!("item-seasons-{}", show.id)).visible());
        assert!(
            window
                .try_find(format!("item-seasons-{}", movie.id))
                .is_none()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn adding_a_season_shows_it_with_a_status(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let show = seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();

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
        window.click(format!("item-seasons-{}", show.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("seasons-panel").visible());
        assert!(window.find("seasons-empty").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "1");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("add-season", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = seasons::all(&Database::open(&path).unwrap(), &show.id).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].season_number, 1);
    assert_eq!(stored[0].status_id, "show-planned");

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let row = window.find(format!("season-{}", stored[0].id));
        assert!(row.visible());
        assert_eq!(row.label(), Some("Season 1"));
        assert_eq!(
            window
                .find(format!("season-status-{}", stored[0].id))
                .label(),
            Some("To watch")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn seasons_are_shown_in_number_order(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let show = seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();
    let second = seed_season(&path, &show.id, 2).unwrap();
    let first = seed_season(&path, &show.id, 1).unwrap();

    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("item-seasons-{}", show.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let first_top = window
            .find(format!("season-{}", first.id))
            .bounds()
            .origin
            .y;
        let second_top = window
            .find(format!("season-{}", second.id))
            .bounds()
            .origin
            .y;
        assert!(first_top < second_top);
    })
    .unwrap();
}

#[gpui_kit::test]
fn changing_a_season_status_updates_the_chip(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let show = seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();
    let season = seed_season(&path, &show.id, 1).unwrap();

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
        window.click(format!("item-seasons-{}", show.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("season-status-{}", season.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window
                .find(format!("season-status-picker-{}", season.id))
                .visible()
        );
        window.click(format!("season-set-status-{}-show-done", season.id), cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(
            window.find(format!("season-status-{}", season.id)).label(),
            Some("Watched")
        );
    })
    .unwrap();

    let stored = seasons::all(&Database::open(&path).unwrap(), &show.id).unwrap();
    assert_eq!(stored[0].status_id, "show-done");
    assert!(stored[0].finished_at.is_some());
}

#[gpui_kit::test]
fn deleting_a_season_asks_for_confirmation(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let show = seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();
    let season = seed_season(&path, &show.id, 1).unwrap();

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
        window.click(format!("item-seasons-{}", show.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("season-delete-{}", season.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("season-delete-confirmation").visible());
        assert!(window.find(format!("season-{}", season.id)).visible());
        window.click("cancel-season-delete", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("season-delete-confirmation").is_none());
        window.click(format!("season-delete-{}", season.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-season-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.try_find(format!("season-{}", season.id)).is_none());
        assert!(window.find("seasons-empty").visible());
    })
    .unwrap();

    assert!(
        seasons::all(&Database::open(&path).unwrap(), &show.id)
            .unwrap()
            .is_empty()
    );
}

#[gpui_kit::test]
fn renaming_a_tag_updates_its_chip(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Movie, "Dune", None).unwrap();
    let tag = seed_tag(&path, "Sci-Fi").unwrap();
    {
        let database = Database::open(&path).unwrap();
        tags::attach_to_item(&database, &item.id, &tag.id).unwrap();
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
        window.click(format!("item-tags-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("rename-tag-{}", tag.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("tag-rename-input").visible());
    })
    .unwrap();
    cx.simulate_input(handle, "Science Fiction");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-tag-rename", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let chip = window.find(format!("item-tag-{}-{}", item.id, tag.id));
        assert!(chip.visible());
        assert_eq!(chip.label(), Some("Science Fiction"));
    })
    .unwrap();

    let stored = tags::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].id, tag.id);
    assert_eq!(stored[0].name, "Science Fiction");
}

#[gpui_kit::test]
fn deleting_a_tag_asks_for_confirmation(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Movie, "Dune", None).unwrap();
    let tag = seed_tag(&path, "Sci-Fi").unwrap();
    {
        let database = Database::open(&path).unwrap();
        tags::attach_to_item(&database, &item.id, &tag.id).unwrap();
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
        window.click(format!("item-tags-{}", item.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("delete-tag-{}", tag.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("tag-delete-confirmation").visible());
        assert!(window.try_find(format!("tag-toggle-{}", tag.id)).is_none());
        window.click("cancel-tag-delete", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("tag-delete-confirmation").is_none());
        window.click(format!("delete-tag-{}", tag.id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("confirm-tag-delete", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.try_find(format!("tag-toggle-{}", tag.id)).is_none());
        assert!(
            window
                .try_find(format!("item-tag-{}-{}", item.id, tag.id))
                .is_none()
        );
    })
    .unwrap();

    let database = Database::open(&path).unwrap();
    assert!(tags::all(&database).unwrap().is_empty());
    assert!(tags::for_item(&database, &item.id).unwrap().is_empty());
}
