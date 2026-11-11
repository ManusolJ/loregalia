use gpui_kit::http_client::{AsyncBody, FakeHttpClient, HttpClient, Response};
use gpui_kit::test::{TestQueryExt, TestWindowExt};
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Context, Pixels, Point, Result, Role, Size,
    TestAppContext, WindowBounds, WindowOptions, px, size,
};
use loregalia_app::shell::{MetadataDetails, MetadataSearch, Shell};
use loregalia_app::startup::StorageState;
use loregalia_app::theme;
use loregalia_core::{
    Database, Error, Result as CoreResult, items, list_items, lists,
    media::MediaKind,
    providers::{Candidate, Source},
    seasons, settings, tags,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

fn open_window(
    cx: &mut TestAppContext,
    window_size: Size<Pixels>,
    choice: settings::Theme,
    client: Option<Arc<dyn HttpClient>>,
    build: impl FnOnce(&mut Context<Shell>) -> Shell,
) -> Result<AnyWindowHandle> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::apply_choice(choice, cx);
        if let Some(client) = client {
            cx.set_http_client(client);
        }

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: window_size,
            })),
            ..Default::default()
        };
        let (window, _) = gpui_kit::open_window(options, cx, |_, cx| cx.new(build))?;

        Ok(window)
    })
}

fn open_shell(
    cx: &mut TestAppContext,
    window_size: Size<Pixels>,
    storage: StorageState,
    log_path: Option<PathBuf>,
) -> Result<AnyWindowHandle> {
    open_window(cx, window_size, settings::Theme::Dark, None, |cx| {
        Shell::new(storage, log_path, settings::Theme::Dark, cx)
    })
}

fn open_shell_with_search(
    cx: &mut TestAppContext,
    window_size: Size<Pixels>,
    storage: StorageState,
    log_path: Option<PathBuf>,
    search: MetadataSearch,
) -> Result<AnyWindowHandle> {
    open_window(cx, window_size, settings::Theme::Dark, None, |cx| {
        Shell::new_with_search(storage, log_path, settings::Theme::Dark, search, cx)
    })
}

fn open_shell_with_theme(
    cx: &mut TestAppContext,
    window_size: Size<Pixels>,
    storage: StorageState,
    choice: settings::Theme,
) -> Result<AnyWindowHandle> {
    open_window(cx, window_size, choice, None, |cx| {
        Shell::new(storage, None, choice, cx)
    })
}

fn open_shell_with_services(
    cx: &mut TestAppContext,
    window_size: Size<Pixels>,
    storage: StorageState,
    search: MetadataSearch,
    details: MetadataDetails,
) -> Result<AnyWindowHandle> {
    open_window(cx, window_size, settings::Theme::Dark, None, |cx| {
        Shell::new_with_services(storage, None, settings::Theme::Dark, search, details, cx)
    })
}

fn open_shell_with_http(
    cx: &mut TestAppContext,
    window_size: Size<Pixels>,
    storage: StorageState,
    client: Arc<dyn HttpClient>,
) -> Result<AnyWindowHandle> {
    open_window(cx, window_size, settings::Theme::Dark, Some(client), |cx| {
        Shell::new(storage, None, settings::Theme::Dark, cx)
    })
}

const TINY_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 0, 3, 1,
    1, 0, 201, 254, 146, 239, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

fn found_show() -> Candidate {
    Candidate {
        source: Source::TvMaze,
        external_id: "42".to_owned(),
        title: "Dark".to_owned(),
        release_year: Some(2017),
        cover_url: Some("https://example.test/dark.jpg".to_owned()),
    }
}

fn found_book() -> Candidate {
    Candidate {
        source: Source::OpenLibrary,
        external_id: "OL20893680W".to_owned(),
        title: "Piranesi".to_owned(),
        release_year: Some(2020),
        cover_url: Some(
            "https://covers.openlibrary.org/b/id/10226290-M.jpg?default=false".to_owned(),
        ),
    }
}

fn found_movie() -> Candidate {
    Candidate {
        source: Source::Tmdb,
        external_id: "438631".to_owned(),
        title: "Dune".to_owned(),
        release_year: Some(2021),
        cover_url: Some(
            "https://image.tmdb.org/t/p/w342/v1tRXZ4JtD2Iv6fjkPvT4GiwslV.jpg".to_owned(),
        ),
    }
}

fn refreshed_movie() -> Candidate {
    Candidate {
        source: Source::Tmdb,
        external_id: "438631".to_owned(),
        title: "Dune: Part One".to_owned(),
        release_year: Some(2021),
        cover_url: Some("https://image.tmdb.org/t/p/w342/refreshed.jpg".to_owned()),
    }
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

fn seed_cover_item(path: &Path, title: &str) -> CoreResult<items::Item> {
    let database = Database::open(path)?;
    let lookup = items::Lookup {
        source: Source::TvMaze,
        external_id: "42".to_owned(),
        cover_url: Some("https://example.test/dark.jpg".to_owned()),
    };
    items::create_from_lookup(&database, MediaKind::Show, title, Some(2017), &lookup)
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
        assert_eq!(second_top - first_top, px(34.0));

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
        window.click(format!("item-row-{}", item.id), cx);
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
fn searching_tvmaze_adds_the_chosen_show(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let search: MetadataSearch = Arc::new(|kind: MediaKind, query: &str| {
        if kind == MediaKind::Show && query == "dark" {
            Ok(vec![found_show()])
        } else {
            Ok(Vec::new())
        }
    });

    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path.clone()),
        None,
        search,
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
        window.click("item-kind-show", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "dark");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("search-result-tvmaze-42").label(),
            Some("Dark (2017)")
        );
        window.click("search-result-tvmaze-42", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].media_kind, MediaKind::Show);
    assert_eq!(stored[0].title, "Dark");
    assert_eq!(stored[0].release_year, Some(2017));
    assert_eq!(stored[0].source.as_deref(), Some(Source::TvMaze.key()));
    assert_eq!(stored[0].external_id.as_deref(), Some("42"));
    assert_eq!(
        stored[0].cover_url.as_deref(),
        Some("https://example.test/dark.jpg")
    );
}

#[gpui_kit::test]
fn searching_open_library_adds_the_chosen_book(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let search: MetadataSearch = Arc::new(|kind: MediaKind, query: &str| {
        if kind == MediaKind::Book && query == "piranesi" {
            Ok(vec![found_book()])
        } else {
            Ok(Vec::new())
        }
    });

    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path.clone()),
        None,
        search,
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
        window.click("item-kind-book", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "piranesi");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("search-result-openlibrary-OL20893680W").label(),
            Some("Piranesi (2020)")
        );
        window.click("search-result-openlibrary-OL20893680W", cx);
    })
    .unwrap();
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
    assert_eq!(stored[0].source.as_deref(), Some(Source::OpenLibrary.key()));
    assert_eq!(stored[0].external_id.as_deref(), Some("OL20893680W"));
    assert_eq!(
        stored[0].cover_url.as_deref(),
        Some("https://covers.openlibrary.org/b/id/10226290-M.jpg?default=false")
    );
}

#[gpui_kit::test]
fn searching_tmdb_adds_the_chosen_movie(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let search: MetadataSearch = Arc::new(|kind: MediaKind, query: &str| {
        if kind == MediaKind::Movie && query == "dune" {
            Ok(vec![found_movie()])
        } else {
            Ok(Vec::new())
        }
    });

    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path.clone()),
        None,
        search,
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
        window.click("item-kind-movie", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "dune");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("search-result-tmdb-438631").label(),
            Some("Dune (2021)")
        );
        window.click("search-result-tmdb-438631", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].media_kind, MediaKind::Movie);
    assert_eq!(stored[0].title, "Dune");
    assert_eq!(stored[0].release_year, Some(2021));
    assert_eq!(stored[0].source.as_deref(), Some(Source::Tmdb.key()));
    assert_eq!(stored[0].external_id.as_deref(), Some("438631"));
    assert_eq!(
        stored[0].cover_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w342/v1tRXZ4JtD2Iv6fjkPvT4GiwslV.jpg")
    );
    assert_eq!(
        stored[0].source_data,
        Some(items::SourceData {
            title: "Dune".to_owned(),
            release_year: Some(2021),
            cover_url: Some(
                "https://image.tmdb.org/t/p/w342/v1tRXZ4JtD2Iv6fjkPvT4GiwslV.jpg".to_owned()
            ),
        })
    );
    assert!(
        stored[0]
            .source_refreshed_at
            .as_deref()
            .is_some_and(|value| value.ends_with('Z'))
    );
}

#[gpui_kit::test]
fn a_stale_tmdb_item_refreshes_in_the_background(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let database = Database::open(&path).unwrap();
    let item = items::create_from_lookup(
        &database,
        MediaKind::Movie,
        "Dune",
        Some(2021),
        &items::Lookup {
            source: Source::Tmdb,
            external_id: "438631".to_owned(),
            cover_url: Some("https://image.example/old.jpg".to_owned()),
        },
    )
    .unwrap();
    let old_data = item.source_data.clone().unwrap();
    items::apply_refresh(&database, &item.id, old_data, "2026-01-01T00:00:00.000Z").unwrap();
    drop(database);

    let search: MetadataSearch = Arc::new(|_, _| Ok(Vec::new()));
    let details: MetadataDetails = Arc::new(|source: Source, external_id: String| {
        if source == Source::Tmdb && external_id == "438631" {
            Ok(refreshed_movie())
        } else {
            Err(Error::UnsupportedLookup)
        }
    });
    let handle = open_shell_with_services(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path.clone()),
        search,
        details,
    )
    .unwrap();
    cx.run_until_parked();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].title, "Dune: Part One");
    assert_eq!(
        stored[0].cover_url.as_deref(),
        Some("https://image.tmdb.org/t/p/w342/refreshed.jpg")
    );
    assert_ne!(
        stored[0].source_refreshed_at.as_deref(),
        Some("2026-01-01T00:00:00.000Z")
    );

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(format!("item-row-{}", item.id)).label(),
            Some("Dune: Part One")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn an_empty_show_search_is_not_a_failure(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let search: MetadataSearch = Arc::new(|_, _| Ok(Vec::new()));
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        ready(&temporary),
        None,
        search,
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
        window.click("item-kind-show", cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "zzz");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let empty = window.find("metadata-search-empty");
        assert!(empty.visible());
        assert_eq!(empty.role(), Some(Role::Status));
        assert_eq!(empty.label(), Some("No matches on TVmaze"));
        assert!(window.try_find("metadata-search-failed").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_failed_show_search_is_shown_as_an_alert(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let log_path = temporary.path().join("loregalia.log");
    let search: MetadataSearch = Arc::new(|_, _| Err(Error::ConnectionFailed));
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        ready(&temporary),
        Some(log_path.clone()),
        search,
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
        window.click("item-kind-show", cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "private-query-42");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let failed = window.find("metadata-search-failed");
        assert!(failed.visible());
        assert_eq!(failed.role(), Some(Role::Alert));
        assert!(
            failed
                .label()
                .is_some_and(|label| label.contains("could not connect"))
        );
        assert!(window.try_find("metadata-search-empty").is_none());
    })
    .unwrap();

    let contents = std::fs::read_to_string(&log_path).unwrap();
    assert!(contents.contains("Metadata search failed"));
    assert!(contents.contains("could not connect"));
    assert!(!contents.contains("private-query-42"));
}

#[gpui_kit::test]
fn a_rate_limited_search_names_the_rate_limit(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let search: MetadataSearch = Arc::new(|_, _| Err(Error::RateLimited));
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        ready(&temporary),
        None,
        search,
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
        window.click("item-kind-show", cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "dark");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let failed = window.find("metadata-search-failed");
        assert!(failed.visible());
        assert!(
            failed
                .label()
                .is_some_and(|label| label.contains("rate limiting"))
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_show_already_in_the_library_cannot_be_added_twice(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let lookup = items::Lookup {
        source: Source::TvMaze,
        external_id: "42".to_owned(),
        cover_url: None,
    };
    items::create_from_lookup(
        &Database::open(&path).unwrap(),
        MediaKind::Show,
        "Dark",
        Some(2017),
        &lookup,
    )
    .unwrap();
    let search: MetadataSearch = Arc::new(|_, _| Ok(vec![found_show()]));
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
        search,
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
        window.click("item-kind-show", cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "dark");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let result = window.find("search-result-tvmaze-42");
        assert!(result.visible());
        assert_eq!(result.label(), Some("Dark (2017) (already added)"));
        assert_eq!(result.disabled(), Some(true));
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_metadata_search_appears_only_for_supported_new_items(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();
    let search: MetadataSearch = Arc::new(|_, _| Ok(vec![found_show()]));
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        None,
        search,
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
        assert!(window.find("metadata-search").visible());
        window.click("item-kind-show", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("metadata-search").visible());
        assert_eq!(window.find("search-metadata").disabled(), Some(true));
        window.click("item-kind-book", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("metadata-search").visible());
        window.click("item-kind-movie", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("metadata-search").visible());
        window.click("item-kind-game", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("metadata-search").is_none());
        window.click("cancel-item", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("item-row-{}", item.id), cx);
        window.render_frame(cx);
        window.click(format!("item-edit-{}", item.id), cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-editor").visible());
        assert!(window.try_find("metadata-search").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn switching_away_from_shows_clears_the_lookup(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let search: MetadataSearch = Arc::new(|_, _| Ok(vec![found_show()]));
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path.clone()),
        None,
        search,
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
        window.click("item-kind-show", cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "dark");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-result-tvmaze-42", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("item-kind-game", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("metadata-search").is_none());
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].media_kind, MediaKind::Game);
    assert_eq!(stored[0].title, "Dark");
    assert_eq!(stored[0].source, None);
    assert_eq!(stored[0].external_id, None);
    assert_eq!(stored[0].cover_url, None);
}

#[gpui_kit::test]
fn a_hand_typed_duplicate_warns_but_can_be_saved(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    seed_item(&path, MediaKind::Movie, "Dune", Some(1984)).unwrap();
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
    cx.simulate_input(handle, "dune");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let warning = window.find("item-duplicate-warning");
        assert!(warning.visible());
        assert_eq!(warning.role(), Some(Role::Alert));
        assert_eq!(
            warning.label(),
            Some(r#""Dune" is already in your library. You can still save it."#)
        );
        assert_eq!(window.find("save-item").label(), Some("Save anyway"));
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 2);
    assert!(
        stored
            .iter()
            .all(|item| item.media_kind == MediaKind::Movie)
    );
    assert!(
        stored
            .iter()
            .all(|item| item.title.eq_ignore_ascii_case("dune"))
    );
}

#[gpui_kit::test]
fn a_duplicate_of_another_media_kind_does_not_warn(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    seed_item(&path, MediaKind::Book, "Dune", Some(1965)).unwrap();
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
        window.click("add-item", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "Dune");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.try_find("item-duplicate-warning").is_none());
        assert_eq!(window.find("save-item").label(), Some("Save"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_duplicate_warning_tracks_the_selected_kind(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    seed_item(&path, MediaKind::Movie, "Dune", Some(1984)).unwrap();
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
        window.click("add-item", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "Dune");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-duplicate-warning").visible());
        window.click("item-kind-book", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("item-duplicate-warning").is_none());
        window.click("item-kind-movie", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-duplicate-warning").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_looked_up_show_does_not_warn_about_the_same_hand_typed_title(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();
    let search: MetadataSearch = Arc::new(|_, _| Ok(vec![found_show()]));
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path.clone()),
        None,
        search,
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
        window.click("item-kind-show", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "dark");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-metadata", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("search-result-tvmaze-42", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.try_find("item-duplicate-warning").is_none());
        assert_eq!(window.find("save-item").label(), Some("Save"));
        window.click("save-item", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let stored = items::all(&Database::open(&path).unwrap()).unwrap();
    assert_eq!(stored.len(), 2);
    assert_eq!(stored[0].source, None);
    assert_eq!(stored[1].source.as_deref(), Some(Source::TvMaze.key()));
    assert!(stored.iter().all(|item| item.title == "Dark"));
}

#[gpui_kit::test]
fn the_image_http_client_fetches_a_cover(cx: &mut TestAppContext) {
    use gpui_kit::http_client::{Method, Request, StatusCode};
    use loregalia_app::http::ImageHttpClient;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = [0_u8; 1024];
        let _ = stream.read(&mut request);
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\nPNG!");
    });

    let client = ImageHttpClient::new(cx.background_executor.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("http://{address}/cover.png"))
        .body(AsyncBody::default())
        .unwrap();
    let result = Arc::new(Mutex::new(None));
    let slot = result.clone();
    cx.background_executor
        .spawn(async move {
            *slot.lock().unwrap() = Some(client.send(request).await);
        })
        .detach();
    cx.run_until_parked();

    let response = result
        .lock()
        .unwrap()
        .take()
        .expect("the image request did not finish")
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[gpui_kit::test]
fn an_item_cover_loads_into_the_row(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_cover_item(&path, "Dark").unwrap();
    let client = FakeHttpClient::create(|_| async {
        Ok(Response::builder()
            .status(200)
            .body(AsyncBody::from(TINY_PNG.to_vec()))
            .unwrap())
    });

    let handle = open_shell_with_http(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        client,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let cover = window.find(format!("cover-{}", item.id));
        assert!(cover.visible());
        assert_eq!(cover.role(), Some(Role::Image));
        assert_eq!(cover.label(), Some("Cover for Dark"));
        assert!(window.find(format!("cover-image-{}", item.id)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn an_item_without_a_cover_shows_the_fallback_tile(cx: &mut TestAppContext) {
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

        assert!(window.find(format!("cover-fallback-{}", item.id)).visible());
        assert!(
            window
                .try_find(format!("cover-image-{}", item.id))
                .is_none()
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_failed_cover_keeps_the_fallback_tile(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_cover_item(&path, "Dark").unwrap();
    let client = FakeHttpClient::create(|_| async {
        Ok(Response::builder()
            .status(404)
            .body(AsyncBody::default())
            .unwrap())
    });

    let handle = open_shell_with_http(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        client,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.find(format!("cover-fallback-{}", item.id)).visible());
    })
    .unwrap();
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
fn closing_the_detail_panel_hides_its_status_picker(cx: &mut TestAppContext) {
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
        window.click(format!("item-row-{}", item.id), cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("detail-panel").visible());
        assert!(window.find(format!("status-picker-{}", item.id)).visible());
        assert_eq!(window.find("detail-title").label(), Some("Outer Wilds"));
        window.click("detail-close", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("detail-panel").is_none());
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
        window.click(format!("item-row-{}", item.id), cx);
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
        window.click(format!("item-row-{}", item.id), cx);
        window.render_frame(cx);
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
        window.click(format!("item-row-{}", item.id), cx);
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
        window.click(format!("item-row-{}", item.id), cx);
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
        window.click(format!("item-row-{}", item.id), cx);
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
        window.click(format!("item-row-{}", item.id), cx);
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
fn clicking_about_shows_the_credits(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(1280.0), px(800.0)), ready(&temporary), None).unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("about-button", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let screen = window.find("about-screen");
        assert!(screen.visible());
        assert_eq!(screen.label(), Some("About Loregalia"));

        let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
        assert_eq!(window.find("about-version").label(), Some(version.as_str()));
        assert!(window.find("about-tvmaze").visible());
        assert!(window.find("tvmaze-link").visible());
        assert!(window.find("about-tmdb").visible());
        assert_eq!(
            window.find("about-tmdb").label(),
            Some("This product uses the TMDB API but is not endorsed or certified by TMDB")
        );
        assert!(window.find("tmdb-link").visible());
        assert!(window.find("about-openlibrary").visible());
        assert!(window.find("openlibrary-link").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn selecting_a_list_leaves_the_about_screen(cx: &mut TestAppContext) {
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
        window.click("about-button", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("about-screen").visible());
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.try_find("about-screen").is_none());
        assert!(window.find("list-panel").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_about_button_is_reachable_in_the_rail(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(960.0), px(600.0)), ready(&temporary), None).unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let button = window.find("about-button");
        assert!(button.visible());
        assert_eq!(button.label(), Some("About"));
        window.click("about-button", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("about-screen").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_settings_screen_offers_every_theme(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(1280.0), px(800.0)), ready(&temporary), None).unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-button", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let screen = window.find("settings-screen");
        assert!(screen.visible());
        assert_eq!(screen.label(), Some("Settings, Dark theme"));
        assert!(window.find("theme-dark").visible());
        assert!(window.find("theme-light").visible());
        assert!(window.find("theme-system").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn selecting_the_light_theme_applies_and_saves(cx: &mut TestAppContext) {
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
        window.click("settings-button", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-light", cx);
    })
    .unwrap();
    cx.run_until_parked();

    let palette = cx.update(|cx| *cx.global::<theme::Palette>());
    assert_eq!(palette, theme::LIGHT);

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("settings-screen").label(),
            Some("Settings, Light theme")
        );
    })
    .unwrap();

    let database = Database::open(&path).unwrap();
    assert_eq!(settings::theme(&database).unwrap(), settings::Theme::Light);
}

#[gpui_kit::test]
fn selecting_the_system_theme_persists_it(cx: &mut TestAppContext) {
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
        window.click("settings-button", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-system", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("settings-screen").label(),
            Some("Settings, System theme")
        );
    })
    .unwrap();

    let database = Database::open(&path).unwrap();
    assert_eq!(settings::theme(&database).unwrap(), settings::Theme::System);
}

#[gpui_kit::test]
fn a_failed_theme_save_is_shown(cx: &mut TestAppContext) {
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

    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-button", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("theme-light", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let error = window.find("settings-error");
        assert!(error.visible());
        assert_eq!(error.role(), Some(Role::Alert));
        assert!(
            error
                .label()
                .is_some_and(|label| label.contains("Could not save the theme"))
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_saved_theme_opens_in_the_shell(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let database = Database::open(&path).unwrap();
    settings::set_theme(&database, settings::Theme::Light).unwrap();
    drop(database);

    let handle = open_shell_with_theme(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(path),
        settings::Theme::Light,
    )
    .unwrap();
    cx.run_until_parked();

    let palette = cx.update(|cx| *cx.global::<theme::Palette>());
    assert_eq!(palette, theme::LIGHT);

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("settings-button", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find("settings-screen").label(),
            Some("Settings, Light theme")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_settings_button_is_reachable_in_the_rail(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(960.0), px(600.0)), ready(&temporary), None).unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let button = window.find("settings-button");
        assert!(button.visible());
        assert_eq!(button.label(), Some("Settings"));
        window.click("settings-button", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("settings-screen").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn selecting_a_list_leaves_the_settings_screen(cx: &mut TestAppContext) {
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
        window.click("settings-button", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("settings-screen").visible());
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert!(window.try_find("settings-screen").is_none());
        assert!(window.find("list-panel").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_detail_of_a_show_offers_seasons_and_other_kinds_do_not(cx: &mut TestAppContext) {
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
        window.click(format!("item-row-{}", show.id), cx);
        window.render_frame(cx);
        assert!(window.find(format!("item-seasons-{}", show.id)).visible());

        window.click(format!("item-row-{}", movie.id), cx);
        window.render_frame(cx);
        assert!(window.find("detail-panel").visible());
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
        window.click(format!("item-row-{}", show.id), cx);
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
        window.click(format!("item-row-{}", show.id), cx);
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
        window.click(format!("item-row-{}", show.id), cx);
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
        window.click(format!("item-row-{}", show.id), cx);
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
fn sorting_a_list_changes_the_view_but_not_the_positions(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
    let zebra = seed_item(&path, MediaKind::Movie, "Zebra", Some(2001)).unwrap();
    let apple = seed_item(&path, MediaKind::Book, "apple", Some(1999)).unwrap();
    let mango = seed_item(&path, MediaKind::Game, "Mango", None).unwrap();
    add_membership(&path, &seeded[0].id, &zebra.id).unwrap();
    add_membership(&path, &seeded[0].id, &apple.id).unwrap();
    add_membership(&path, &seeded[0].id, &mango.id).unwrap();

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
        window.click("sort-title", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let apple_top = window
            .find(format!("item-row-{}", apple.id))
            .bounds()
            .origin
            .y;
        let mango_top = window
            .find(format!("item-row-{}", mango.id))
            .bounds()
            .origin
            .y;
        let zebra_top = window
            .find(format!("item-row-{}", zebra.id))
            .bounds()
            .origin
            .y;
        assert!(apple_top < mango_top);
        assert!(mango_top < zebra_top);
        assert!(window.try_find(format!("move-up-{}", zebra.id)).is_none());
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("sort-year", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let apple_top = window
            .find(format!("item-row-{}", apple.id))
            .bounds()
            .origin
            .y;
        let mango_top = window
            .find(format!("item-row-{}", mango.id))
            .bounds()
            .origin
            .y;
        let zebra_top = window
            .find(format!("item-row-{}", zebra.id))
            .bounds()
            .origin
            .y;
        assert!(apple_top < zebra_top);
        assert!(zebra_top < mango_top);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("sort-manual", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let apple_top = window
            .find(format!("item-row-{}", apple.id))
            .bounds()
            .origin
            .y;
        let mango_top = window
            .find(format!("item-row-{}", mango.id))
            .bounds()
            .origin
            .y;
        let zebra_top = window
            .find(format!("item-row-{}", zebra.id))
            .bounds()
            .origin
            .y;
        assert!(zebra_top < apple_top);
        assert!(apple_top < mango_top);
        assert!(window.find(format!("move-up-{}", apple.id)).visible());
    })
    .unwrap();

    let stored: Vec<String> = list_items::all(&Database::open(&path).unwrap(), &seeded[0].id)
        .unwrap()
        .into_iter()
        .map(|item| item.title)
        .collect();
    assert_eq!(stored, ["Zebra", "apple", "Mango"]);
}

#[gpui_kit::test]
fn selecting_another_list_resets_the_sort_to_manual(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["First", "Second"]).unwrap();
    let zebra = seed_item(&path, MediaKind::Movie, "Zebra", None).unwrap();
    let apple = seed_item(&path, MediaKind::Book, "apple", None).unwrap();

    for list in &seeded {
        add_membership(&path, &list.id, &zebra.id).unwrap();
        add_membership(&path, &list.id, &apple.id).unwrap();
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
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("sort-title", cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click(format!("list-row-{}", seeded[1].id), cx);
    })
    .unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        let apple_top = window
            .find(format!("item-row-{}", apple.id))
            .bounds()
            .origin
            .y;
        let zebra_top = window
            .find(format!("item-row-{}", zebra.id))
            .bounds()
            .origin
            .y;
        assert!(zebra_top < apple_top);
    })
    .unwrap();
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
        window.click(format!("item-row-{}", item.id), cx);
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
        window.click(format!("item-row-{}", item.id), cx);
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

#[gpui_kit::test]
fn pressing_enter_saves_the_list_name(cx: &mut TestAppContext) {
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
        window.click("create-list", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "Queue");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("list-editor").is_none());
        assert!(window.find_by_label("Queue").visible());
    })
    .unwrap();
    let names: Vec<String> = lists::all(&Database::open(&path).unwrap())
        .unwrap()
        .into_iter()
        .map(|list| list.name)
        .collect();
    assert_eq!(names, ["Queue"]);
}

#[gpui_kit::test]
fn pressing_enter_in_the_search_box_searches(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let search: MetadataSearch = Arc::new(|kind: MediaKind, query: &str| {
        if kind == MediaKind::Show && query == "dark" {
            Ok(vec![found_show()])
        } else {
            Ok(Vec::new())
        }
    });
    let handle = open_shell_with_search(
        cx,
        size(px(1280.0), px(800.0)),
        ready(&temporary),
        None,
        search,
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
        window.click("item-kind-show", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("metadata-search-input", cx);
    })
    .unwrap();
    cx.simulate_input(handle, "dark");
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("item-editor").visible());
        assert_eq!(
            window.find("search-result-tvmaze-42").label(),
            Some("Dark (2017)")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn all_items_returns_from_a_list_or_the_about_screen(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let seeded = seed_lists(&path, &["Queue"]).unwrap();
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
        let all_items = window.find("all-items");
        assert_eq!(all_items.label(), Some("All items"));
        window.click(format!("list-row-{}", seeded[0].id), cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("list-panel").visible());
        window.click("all-items", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("list-panel").is_none());
        assert!(window.find(format!("item-row-{}", item.id)).visible());
        window.click("about-button", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("about-screen").visible());
        window.click("all-items", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("about-screen").is_none());
        assert!(window.find(format!("item-row-{}", item.id)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn all_items_is_reachable_in_the_rail(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(960.0), px(600.0)), ready(&temporary), None).unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let all_items = window.find("all-items");
        assert!(all_items.visible());
        assert_eq!(all_items.label(), Some("All items"));
        assert_eq!(all_items.bounds().size, size(px(32.0), px(32.0)));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_database_error_can_be_dismissed(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let database_path = temporary.path().join("loregalia.db");
    std::fs::create_dir_all(&database_path).unwrap();
    let handle = open_shell(
        cx,
        size(px(1280.0), px(800.0)),
        StorageState::Ready(database_path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("lists-error").visible());
        window.click("dismiss-error", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("lists-error").is_none());
        assert!(window.find("empty-state").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn add_item_appears_only_in_the_library(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let handle = open_shell(cx, size(px(1280.0), px(800.0)), ready(&temporary), None).unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("add-item").visible());
        window.click("about-button", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("about-screen").visible());
        assert!(window.try_find("add-item").is_none());
        window.click("settings-button", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("settings-screen").visible());
        assert!(window.try_find("add-item").is_none());
        window.click("all-items", cx);
    })
    .unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("add-item").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn content_starts_at_the_top_left_of_the_main_area(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    seed_item(&path, MediaKind::Movie, "Dune", Some(2021)).unwrap();
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
        let items = window.find("items").bounds();
        let title = window.find("page-title").bounds();
        let header = window.find("page-header").bounds();

        assert_eq!(items.origin.x, title.origin.x);
        assert!(items.origin.y - (header.origin.y + header.size.height) <= px(24.0));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_long_library_scrolls_instead_of_overflowing(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    for number in 0..30 {
        seed_item(&path, MediaKind::Book, &format!("Book {number}"), None).unwrap();
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
        let viewport = window.find("main-scroll").bounds();

        assert!(viewport.origin.y + viewport.size.height <= px(800.0));
        assert!(window.find("items").bounds().size.height > viewport.size.height);
    })
    .unwrap();
}

fn seed_status(path: &Path, item_id: &str, status_id: &str) -> CoreResult<()> {
    let database = Database::open(path)?;
    items::set_status(&database, item_id, status_id)?;

    Ok(())
}

#[gpui_kit::test]
fn shelves_show_their_items_and_counts(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let active = seed_item(&path, MediaKind::Show, "Dark", Some(2017)).unwrap();
    let done = seed_item(&path, MediaKind::Movie, "Dune", Some(2021)).unwrap();
    let planned = seed_item(&path, MediaKind::Book, "Piranesi", Some(2020)).unwrap();
    seed_status(&path, &active.id, "show-active").unwrap();
    seed_status(&path, &done.id, "movie-done").unwrap();
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
        assert_eq!(window.find("page-title").label(), Some("All items"));
        assert_eq!(window.find("page-count").label(), Some("3"));
        window.click("shelf-up-next", cx);
        window.render_frame(cx);

        assert_eq!(window.find("page-title").label(), Some("Up next"));
        assert_eq!(window.find("page-count").label(), Some("1"));
        assert!(window.find(format!("item-row-{}", active.id)).visible());
        assert!(
            window
                .try_find(format!("item-row-{}", planned.id))
                .is_none()
        );

        window.click("shelf-finished", cx);
        window.render_frame(cx);
        assert_eq!(window.find("page-title").label(), Some("Finished"));
        assert!(window.find(format!("item-row-{}", done.id)).visible());
        assert!(window.try_find(format!("item-row-{}", active.id)).is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn an_empty_shelf_says_what_it_holds(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    seed_item(&path, MediaKind::Book, "Piranesi", None).unwrap();
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
        window.click("shelf-finished", cx);
        window.render_frame(cx);

        let empty = window.find("empty-state");
        assert_eq!(empty.label(), Some("Nothing finished yet"));
        assert_eq!(empty.role(), Some(Role::Status));
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_kind_filter_narrows_the_library(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let movie = seed_item(&path, MediaKind::Movie, "Dune", Some(2021)).unwrap();
    let book = seed_item(&path, MediaKind::Book, "Piranesi", Some(2020)).unwrap();
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
        window.click("filter-book", cx);
        window.render_frame(cx);

        assert_eq!(window.find("page-count").label(), Some("1"));
        assert!(window.find(format!("item-row-{}", book.id)).visible());
        assert!(window.try_find(format!("item-row-{}", movie.id)).is_none());

        window.click("filter-all", cx);
        window.render_frame(cx);
        assert!(window.find(format!("item-row-{}", movie.id)).visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn up_next_tiles_lead_the_library_and_open_details(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let active = seed_item(&path, MediaKind::Game, "Outer Wilds", Some(2019)).unwrap();
    let planned = seed_item(&path, MediaKind::Book, "Piranesi", Some(2020)).unwrap();
    seed_status(&path, &active.id, "game-active").unwrap();
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
        let shelf = window.find("up-next").bounds();
        let items = window.find("items").bounds();

        assert!(shelf.origin.y < items.origin.y);
        assert!(window.find(format!("up-next-{}", active.id)).visible());
        assert!(window.try_find(format!("up-next-{}", planned.id)).is_none());

        window.click(format!("up-next-{}", active.id), cx);
        window.render_frame(cx);
        assert_eq!(window.find("detail-title").label(), Some("Outer Wilds"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn a_list_title_and_count_head_the_page(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let list = seed_lists(&path, &["Winter reading"]).unwrap().remove(0);
    let book = seed_item(&path, MediaKind::Book, "Piranesi", None).unwrap();
    add_membership(&path, &list.id, &book.id).unwrap();
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
        window.click(format!("list-row-{}", list.id), cx);
        window.render_frame(cx);

        let title = window.find("page-title");
        assert_eq!(title.label(), Some("Winter reading"));
        assert_eq!(title.role(), Some(Role::Heading));
        assert_eq!(window.find("page-count").label(), Some("1"));
        assert!(window.find("rename-list").visible());
        assert!(window.find("add-item").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_detail_panel_docks_beside_the_list_on_a_wide_window(cx: &mut TestAppContext) {
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
        window.click(format!("item-row-{}", item.id), cx);
        window.render_frame(cx);

        let panel = window.find("detail-panel").bounds();
        let main = window.find("main-scroll").bounds();
        assert_eq!(panel.size.width, px(360.0));
        assert_eq!(panel.origin.x + panel.size.width, px(1280.0));
        assert!(main.origin.x + main.size.width <= panel.origin.x);
    })
    .unwrap();
}

#[gpui_kit::test]
fn the_detail_panel_is_a_drawer_over_the_list_below_1280(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let item = seed_item(&path, MediaKind::Movie, "Dune", Some(2021)).unwrap();
    let handle = open_shell(
        cx,
        size(px(1100.0), px(800.0)),
        StorageState::Ready(path),
        None,
    )
    .unwrap();
    cx.run_until_parked();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let main_before = window.find("main-scroll").bounds();
        window.click(format!("item-row-{}", item.id), cx);
        window.render_frame(cx);

        let main_after = window.find("main-scroll").bounds();
        assert_eq!(main_after.size.width, main_before.size.width);
        assert!(window.find("detail-panel").visible());
        assert!(window.find("detail-close").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn clicking_a_row_selects_it_for_the_detail_panel(cx: &mut TestAppContext) {
    let temporary = TempDir::new().unwrap();
    let path = temporary.path().join("loregalia.db");
    let first = seed_item(&path, MediaKind::Movie, "Dune", Some(2021)).unwrap();
    let second = seed_item(&path, MediaKind::Book, "Piranesi", Some(2020)).unwrap();
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
        window.click(format!("item-row-{}", first.id), cx);
        window.render_frame(cx);
        assert_eq!(window.find("detail-title").label(), Some("Dune"));

        window.click(format!("item-row-{}", second.id), cx);
        window.render_frame(cx);
        assert_eq!(window.find("detail-title").label(), Some("Piranesi"));
        assert!(window.find(format!("item-delete-{}", second.id)).visible());
        assert!(
            window
                .try_find(format!("item-delete-{}", first.id))
                .is_none()
        );
    })
    .unwrap();
}
