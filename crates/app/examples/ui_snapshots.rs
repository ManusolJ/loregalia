use gpui_kit::assets::AllAssets;
use gpui_kit::component::ThemeMode;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AppContext, Bounds, HeadlessAppContext, Pixels, Point, Result, Size, WindowBounds,
    WindowOptions, px, size,
};
use loregalia_app::shell::Shell;
use loregalia_app::startup::StorageState;
use loregalia_app::theme;
use loregalia_core::{Database, items, list_items, lists, media::MediaKind, seasons, tags};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy)]
enum Step {
    None,
    Select,
    Create,
    AddItem,
    Rename,
    Delete,
    StatusPicker,
    AddPicker,
    ListTags,
    ItemTags,
    Seasons,
    About,
    SortTitle,
}

struct Shot {
    name: &'static str,
    size: Size<Pixels>,
    mode: ThemeMode,
    storage: StorageState,
    step: Step,
}

fn main() -> Result<()> {
    let output = output_directory()?;
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;

    let populated = populated_database(&output)?;
    let with_items = items_database(&output)?;
    let with_membership = membership_database(&output)?;
    let empty = empty_database(&output)?;
    let failure = StorageState::Failed("Could not open the database: the disk is full".to_owned());

    let shots = [
        Shot {
            name: "wide-lists-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: populated.clone(),
            step: Step::None,
        },
        Shot {
            name: "wide-empty-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: empty.clone(),
            step: Step::None,
        },
        Shot {
            name: "wide-lists-light",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Light,
            storage: populated.clone(),
            step: Step::None,
        },
        Shot {
            name: "narrow-rail-dark",
            size: size(px(960.0), px(600.0)),
            mode: ThemeMode::Dark,
            storage: populated.clone(),
            step: Step::None,
        },
        Shot {
            name: "items-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_items.clone(),
            step: Step::None,
        },
        Shot {
            name: "item-status-picker-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_items.clone(),
            step: Step::StatusPicker,
        },
        Shot {
            name: "item-statuses-light",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Light,
            storage: with_items.clone(),
            step: Step::None,
        },
        Shot {
            name: "item-tags-picker-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_items.clone(),
            step: Step::ItemTags,
        },
        Shot {
            name: "show-seasons-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_items.clone(),
            step: Step::Seasons,
        },
        Shot {
            name: "item-create-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: empty.clone(),
            step: Step::AddItem,
        },
        Shot {
            name: "list-selected-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: populated.clone(),
            step: Step::Select,
        },
        Shot {
            name: "list-membership-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_membership.clone(),
            step: Step::Select,
        },
        Shot {
            name: "list-add-picker-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_membership.clone(),
            step: Step::AddPicker,
        },
        Shot {
            name: "list-membership-light",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Light,
            storage: with_membership.clone(),
            step: Step::Select,
        },
        Shot {
            name: "list-tags-picker-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_membership.clone(),
            step: Step::ListTags,
        },
        Shot {
            name: "list-sorted-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: with_membership,
            step: Step::SortTitle,
        },
        Shot {
            name: "list-create-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: empty.clone(),
            step: Step::Create,
        },
        Shot {
            name: "list-rename-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: populated.clone(),
            step: Step::Rename,
        },
        Shot {
            name: "list-delete-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: populated,
            step: Step::Delete,
        },
        Shot {
            name: "about-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: empty.clone(),
            step: Step::About,
        },
        Shot {
            name: "about-light",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Light,
            storage: empty.clone(),
            step: Step::About,
        },
        Shot {
            name: "storage-failure-dark",
            size: size(px(1280.0), px(800.0)),
            mode: ThemeMode::Dark,
            storage: failure,
            step: Step::None,
        },
    ];

    for shot in shots {
        let path = output.join(format!("{}.png", shot.name));
        capture(&shot, &path)?;
        println!("wrote {}", path.display());
    }

    Ok(())
}

fn output_directory() -> Result<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/ui-snapshots"))
}

fn populated_database(output: &Path) -> Result<StorageState> {
    let state = database_at(output, "fixture-db")?;
    if let StorageState::Ready(path) = &state {
        let database = Database::open(path)?;
        lists::create(&database, "Watch later")?;
        lists::create(&database, "Reading")?;
        lists::create(&database, "Games")?;
    }
    Ok(state)
}

fn membership_database(output: &Path) -> Result<StorageState> {
    let state = database_at(output, "membership-db")?;
    if let StorageState::Ready(path) = &state {
        let database = Database::open(path)?;
        let watch = lists::create(&database, "Watch later")?;
        lists::create(&database, "Reading")?;
        let dune = items::create(&database, MediaKind::Movie, "Dune", Some(2021))?;
        let arrival = items::create(&database, MediaKind::Movie, "Arrival", Some(2016))?;
        let wilds = items::create(&database, MediaKind::Game, "Outer Wilds", None)?;
        items::create(&database, MediaKind::Book, "Piranesi", Some(2020))?;
        items::set_status(&database, &dune.id, "movie-done")?;
        items::set_status(&database, &arrival.id, "movie-active")?;
        list_items::add(&database, &watch.id, &dune.id)?;
        list_items::add(&database, &watch.id, &arrival.id)?;
        list_items::add(&database, &watch.id, &wilds.id)?;
        let science_fiction = tags::create(&database, "Sci-Fi")?;
        let weekend = tags::create(&database, "Weekend")?;
        tags::attach_to_item(&database, &dune.id, &science_fiction.id)?;
        tags::attach_to_list(&database, &watch.id, &science_fiction.id)?;
        tags::attach_to_list(&database, &watch.id, &weekend.id)?;
    }
    Ok(state)
}

fn empty_database(output: &Path) -> Result<StorageState> {
    database_at(output, "empty-db")
}

fn items_database(output: &Path) -> Result<StorageState> {
    let state = database_at(output, "items-db")?;
    if let StorageState::Ready(path) = &state {
        let database = Database::open(path)?;
        let movie = items::create(&database, MediaKind::Movie, "Dune", Some(2021))?;
        items::set_status(&database, &movie.id, "movie-active")?;
        let book = items::create(&database, MediaKind::Book, "Piranesi", Some(2020))?;
        items::set_status(&database, &book.id, "book-done")?;
        items::create(&database, MediaKind::Game, "Outer Wilds", None)?;
        let science_fiction = tags::create(&database, "Sci-Fi")?;
        let favourite = tags::create(&database, "Favourite")?;
        tags::attach_to_item(&database, &book.id, &science_fiction.id)?;
        tags::attach_to_item(&database, &book.id, &favourite.id)?;
        let show = items::create(&database, MediaKind::Show, "Dark", Some(2017))?;
        items::set_status(&database, &show.id, "show-active")?;
        let first = seasons::create(&database, &show.id, 1)?;
        seasons::set_status(&database, &first.id, "show-done")?;
        seasons::create(&database, &show.id, 2)?;
    }
    Ok(state)
}

fn database_at(_output: &Path, name: &str) -> Result<StorageState> {
    let root = std::env::temp_dir().join("loregalia-ui-snapshots");
    std::fs::create_dir_all(&root)?;
    let directory = root.join(name);
    if directory.exists() {
        std::fs::remove_dir_all(&directory)?;
    }
    let path = directory.join("loregalia.db");
    Database::open(&path)?;
    Ok(StorageState::Ready(path))
}

fn capture(shot: &Shot, path: &Path) -> Result<()> {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(AllAssets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::apply(shot.mode, cx);
    });

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: shot.size,
        })),
        focus: false,
        show: false,
        ..Default::default()
    };
    let (handle, _) = cx.update(|cx| {
        gpui_kit::open_window(options, cx, |_, cx| {
            cx.new(|cx| Shell::new(shot.storage.clone(), None, cx))
        })
    })?;

    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;

    let first_id = match (&shot.storage, shot.step) {
        (
            StorageState::Ready(path),
            Step::Select
            | Step::Rename
            | Step::Delete
            | Step::AddPicker
            | Step::ListTags
            | Step::SortTitle,
        ) => {
            let database = Database::open(path)?;
            lists::all(&database)?
                .into_iter()
                .next()
                .map(|list| list.id)
        }
        _ => None,
    };

    let first_item = match (&shot.storage, shot.step) {
        (StorageState::Ready(path), Step::StatusPicker | Step::ItemTags) => {
            let database = Database::open(path)?;
            items::all(&database)?
                .into_iter()
                .next()
                .map(|item| item.id)
        }
        (StorageState::Ready(path), Step::Seasons) => {
            let database = Database::open(path)?;
            items::all(&database)?
                .into_iter()
                .find(|item| item.media_kind == MediaKind::Show)
                .map(|item| item.id)
        }
        _ => None,
    };

    match shot.step {
        Step::None => {}
        Step::Select => {
            if let Some(id) = first_id {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("list-row-{id}"), cx);
                    window.render_frame(cx);
                })?;
            }
        }
        Step::Create => {
            cx.update_window(handle, |_, window, cx| {
                window.click("create-list", cx);
                window.render_frame(cx);
            })?;
        }
        Step::AddItem => {
            cx.update_window(handle, |_, window, cx| {
                window.click("add-item", cx);
                window.render_frame(cx);
            })?;
        }
        Step::StatusPicker => {
            if let Some(id) = first_item {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("item-status-{id}"), cx);
                    window.render_frame(cx);
                })?;
            }
        }
        Step::AddPicker => {
            if let Some(id) = first_id {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("list-row-{id}"), cx);
                    window.render_frame(cx);
                    window.click("add-existing-item", cx);
                    window.render_frame(cx);
                })?;
            }
        }
        Step::ListTags => {
            if let Some(id) = first_id {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("list-row-{id}"), cx);
                    window.render_frame(cx);
                    window.click("list-tags", cx);
                    window.render_frame(cx);
                })?;
            }
        }
        Step::SortTitle => {
            if let Some(id) = first_id {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("list-row-{id}"), cx);
                    window.render_frame(cx);
                    window.click("sort-title", cx);
                    window.render_frame(cx);
                })?;
            }
        }
        Step::ItemTags => {
            if let Some(id) = first_item {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("item-tags-{id}"), cx);
                    window.render_frame(cx);
                })?;
            }
        }
        Step::Seasons => {
            if let Some(id) = first_item {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("item-seasons-{id}"), cx);
                    window.render_frame(cx);
                })?;
            }
        }
        Step::About => {
            cx.update_window(handle, |_, window, cx| {
                window.click("about-button", cx);
                window.render_frame(cx);
            })?;
        }
        Step::Rename | Step::Delete => {
            if let Some(id) = first_id {
                cx.update_window(handle, |_, window, cx| {
                    window.click(format!("list-row-{id}"), cx);
                    window.render_frame(cx);
                })?;
                let action = match shot.step {
                    Step::Rename => "rename-list",
                    _ => "delete-list",
                };
                cx.update_window(handle, |_, window, cx| {
                    window.click(action, cx);
                    window.render_frame(cx);
                })?;
            }
        }
    }
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))?;

    let image = cx.capture_screenshot(handle)?;
    let first = image.pixels().next().copied();
    if image.pixels().all(|pixel| Some(*pixel) == first) {
        return Err(std::io::Error::other("the screenshot is blank").into());
    }
    image.save(path)?;

    Ok(())
}
