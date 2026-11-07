use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    Disableable, Icon, Selectable, Sizable, button::Button, button::ButtonVariants, link::Link,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, ObjectFit, Pixels, Role,
    TestSupportExt, Window, div, img, px, rgb,
};
use loregalia_core::{
    Database, Result as CoreResult, items, list_items, lists, log,
    media::MediaKind,
    providers::{self, Candidate, Source},
    seasons, settings, statuses, tags,
};

use crate::startup::StorageState;
use crate::theme::Palette;

pub type MetadataSearch = Arc<dyn Fn(MediaKind, &str) -> CoreResult<Vec<Candidate>> + Send + Sync>;

const RAIL_WIDTH: f32 = 64.0;
const SIDEBAR_WIDTH: f32 = 240.0;
const SIDEBAR_LABELS_FROM: f32 = 1024.0;
const EMPTY_MESSAGE: &str = "Nothing planned yet";
const LOADING_MESSAGE: &str = "Loading lists";
const LOADING_BLOCK_WIDTH: f32 = 320.0;

fn season_number(value: &str) -> Option<i32> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|number| *number >= 0)
}

fn candidate_label(candidate: &Candidate, already_added: bool) -> String {
    let label = match candidate.release_year {
        Some(year) => format!("{} ({year})", candidate.title),
        None => candidate.title.clone(),
    };

    if already_added {
        format!("{label} (already added)")
    } else {
        label
    }
}

pub fn sidebar_width(window_width: Pixels) -> Pixels {
    if window_width < px(SIDEBAR_LABELS_FROM) {
        px(RAIL_WIDTH)
    } else {
        px(SIDEBAR_WIDTH)
    }
}

pub struct Shell {
    storage: StorageState,
    log_path: Option<PathBuf>,
    metadata_search: MetadataSearch,
    theme: settings::Theme,
    settings_screen: bool,
    settings_error: Option<String>,
    lists: Vec<lists::List>,
    items: Vec<items::Item>,
    statuses: Vec<statuses::Status>,
    memberships: Vec<list_items::Membership>,
    tags: Vec<tags::Tag>,
    item_tags: Vec<tags::Tagging>,
    list_tags: Vec<tags::Tagging>,
    seasons: Vec<seasons::Season>,
    error: Option<String>,
    loading: bool,
    about: bool,
    sort: list_items::Sort,
    selected: Option<String>,
    editor: Option<Editor>,
    confirm_delete: Option<String>,
    item_editor: Option<ItemEditor>,
    confirm_delete_item: Option<String>,
    status_picker: Option<String>,
    add_picker: Option<String>,
    tag_editor: Option<TagEditor>,
    renaming_tag: Option<TagRename>,
    confirm_delete_tag: Option<String>,
    season_editor: Option<SeasonEditor>,
    season_status_picker: Option<String>,
    confirm_delete_season: Option<String>,
}

struct ItemEditor {
    id: Option<String>,
    kind: MediaKind,
    title: Entity<InputState>,
    year: Entity<InputState>,
    search_input: Option<Entity<InputState>>,
    search: SearchStatus,
    search_request: u64,
    lookup: Option<items::Lookup>,
}

#[derive(Default)]
enum SearchStatus {
    #[default]
    Idle,
    Searching,
    Empty,
    Failed(String),
    Results(Vec<Candidate>),
}

struct ListItemActions<'a> {
    list_id: &'a str,
    manual: bool,
    can_move_up: bool,
    can_move_down: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TagTarget {
    Item(String),
    List(String),
}

struct TagEditor {
    target: TagTarget,
    input: Entity<InputState>,
}

struct TagRename {
    id: String,
    input: Entity<InputState>,
}

struct SeasonEditor {
    show_id: String,
    number: Entity<InputState>,
}

struct Loaded {
    lists: Vec<lists::List>,
    items: Vec<items::Item>,
    statuses: Vec<statuses::Status>,
    memberships: Vec<list_items::Membership>,
    tags: Vec<tags::Tag>,
    item_tags: Vec<tags::Tagging>,
    list_tags: Vec<tags::Tagging>,
    seasons: Vec<seasons::Season>,
}

fn load(database: &Database) -> CoreResult<Loaded> {
    Ok(Loaded {
        lists: lists::all(database)?,
        items: items::all(database)?,
        statuses: statuses::all(database)?,
        memberships: list_items::memberships(database)?,
        tags: tags::all(database)?,
        item_tags: tags::item_taggings(database)?,
        list_tags: tags::list_taggings(database)?,
        seasons: seasons::all_for_every_show(database)?,
    })
}

enum Editor {
    Create(Entity<InputState>),
    Rename {
        id: String,
        input: Entity<InputState>,
    },
}

impl Shell {
    pub fn new(
        storage: StorageState,
        log_path: Option<PathBuf>,
        theme: settings::Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_search(storage, log_path, theme, Arc::new(providers::search), cx)
    }

    pub fn new_with_search(
        storage: StorageState,
        log_path: Option<PathBuf>,
        theme: settings::Theme,
        metadata_search: MetadataSearch,
        cx: &mut Context<Self>,
    ) -> Self {
        let loading = matches!(&storage, StorageState::Ready(_));
        let mut shell = Self {
            storage,
            log_path,
            metadata_search,
            theme,
            settings_screen: false,
            settings_error: None,
            lists: Vec::new(),
            items: Vec::new(),
            statuses: Vec::new(),
            memberships: Vec::new(),
            tags: Vec::new(),
            item_tags: Vec::new(),
            list_tags: Vec::new(),
            seasons: Vec::new(),
            error: None,
            loading,
            about: false,
            sort: list_items::Sort::Manual,
            selected: None,
            editor: None,
            confirm_delete: None,
            item_editor: None,
            confirm_delete_item: None,
            status_picker: None,
            add_picker: None,
            tag_editor: None,
            renaming_tag: None,
            confirm_delete_tag: None,
            season_editor: None,
            season_status_picker: None,
            confirm_delete_season: None,
        };
        shell.reload_lists(cx);
        shell
    }

    fn record(&self, message: &str, cx: &mut Context<Self>) {
        let Some(path) = self.log_path.clone() else {
            return;
        };
        let message = message.to_owned();

        cx.background_spawn(async move {
            let _ = log::append(&path, &message);
        })
        .detach();
    }

    fn reload_lists(&mut self, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            load(&database)
        });
    }

    fn begin_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx));
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        self.about = false;
        self.settings_screen = false;
        self.selected = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        self.editor = Some(Editor::Create(input));
        cx.notify();
    }

    fn set_sort(&mut self, sort: list_items::Sort, cx: &mut Context<Self>) {
        self.sort = sort;
        cx.notify();
    }

    fn begin_about(&mut self, cx: &mut Context<Self>) {
        self.about = true;
        self.settings_screen = false;
        self.selected = None;
        self.editor = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        cx.notify();
    }

    fn begin_settings(&mut self, cx: &mut Context<Self>) {
        self.about = false;
        self.settings_screen = true;
        self.settings_error = None;
        self.selected = None;
        self.editor = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        cx.notify();
    }

    fn set_theme(&mut self, theme: settings::Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        crate::theme::apply_choice(theme, cx);
        self.settings_error = None;

        let StorageState::Ready(path) = self.storage.clone() else {
            self.settings_error = Some("Settings need a working database.".to_owned());
            cx.notify();
            return;
        };

        let task = cx.background_spawn(async move {
            let database = Database::open(path)?;
            settings::set_theme(&database, theme)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    let message = format!("Could not save the theme: {error}");
                    this.record(&message, cx);
                    this.settings_error = Some(message);
                }
                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    fn select_list(&mut self, id: String, cx: &mut Context<Self>) {
        self.about = false;
        self.settings_screen = false;
        self.sort = list_items::Sort::Manual;
        self.selected = Some(id);
        self.editor = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        cx.notify();
    }

    fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(name) = self
            .lists
            .iter()
            .find(|list| list.id == id)
            .map(|list| list.name.clone())
        else {
            return;
        };
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).default_value(name);
            input.select_all(window, cx);
            input
        });
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        self.editor = Some(Editor::Rename { id, input });
        cx.notify();
    }

    fn save_editor(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.editor.take() else {
            return;
        };
        let (id, input) = match editor {
            Editor::Create(input) => (None, input),
            Editor::Rename { id, input } => (Some(id), input),
        };
        let name = input.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.editor = Some(match id {
                Some(id) => Editor::Rename { id, input },
                None => Editor::Create(input),
            });
            return;
        }
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            match &id {
                Some(id) => {
                    lists::rename(&database, id, &name)?;
                }
                None => {
                    lists::create(&database, &name)?;
                }
            }
            load(&database)
        });
    }

    fn cancel_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        cx.notify();
    }

    fn begin_delete(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };

        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        self.confirm_delete = Some(id);
        cx.notify();
    }

    fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete = None;
        cx.notify();
    }

    fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.selected = None;

        self.run(cx, async move {
            let database = Database::open(path)?;
            lists::delete(&database, &id)?;
            load(&database)
        });
    }

    fn begin_create_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Title"));
        let year = cx.new(|cx| InputState::new(window, cx).placeholder("Year"));
        let focus = title.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        self.about = false;
        self.settings_screen = false;
        self.editor = None;
        self.confirm_delete = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        self.item_editor = Some(ItemEditor {
            id: None,
            kind: MediaKind::Movie,
            title,
            year,
            search_input: None,
            search: SearchStatus::Idle,
            search_request: 0,
            lookup: None,
        });
        cx.notify();
    }

    fn begin_edit_item(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(item) = self.items.iter().find(|item| item.id == id) else {
            return;
        };
        let (kind, title_value, year_value) = (
            item.media_kind,
            item.title.clone(),
            item.release_year.map(|year| year.to_string()),
        );
        let title = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .placeholder("Title")
                .default_value(title_value);
            input.select_all(window, cx);
            input
        });
        let year = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Year")
                .default_value(year_value.unwrap_or_default())
        });
        let focus = title.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        self.item_editor = Some(ItemEditor {
            id: Some(id),
            kind,
            title,
            year,
            search_input: None,
            search: SearchStatus::Idle,
            search_request: 0,
            lookup: None,
        });
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        cx.notify();
    }

    fn select_item_kind(&mut self, kind: MediaKind, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.item_editor else {
            return;
        };
        if editor.kind == kind {
            return;
        }

        editor.kind = kind;
        editor.search_request += 1;
        editor.search = SearchStatus::Idle;
        editor.lookup = None;

        if providers::supported(kind) && editor.id.is_none() && editor.search_input.is_none() {
            editor.search_input =
                Some(cx.new(|cx| InputState::new(window, cx).placeholder("Search by title")));
        }

        cx.notify();
    }

    fn search_metadata(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.item_editor else {
            return;
        };
        if editor.id.is_some() || !providers::supported(editor.kind) {
            return;
        }
        let Some(input) = &editor.search_input else {
            return;
        };
        let query = input.read(cx).value().trim().to_owned();
        if query.is_empty() || matches!(editor.search, SearchStatus::Searching) {
            return;
        }

        editor.search_request += 1;
        let request = editor.search_request;
        let kind = editor.kind;
        editor.search = SearchStatus::Searching;
        let search = self.metadata_search.clone();
        cx.notify();

        let task = cx.background_spawn(async move { search(kind, &query) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                let Some(editor) = &mut this.item_editor else {
                    return;
                };
                if editor.search_request != request || editor.kind != kind {
                    return;
                }

                let mut failure = None;
                editor.search = match result {
                    Ok(results) if results.is_empty() => SearchStatus::Empty,
                    Ok(results) => SearchStatus::Results(results),
                    Err(error) => {
                        let message = error.to_string();
                        failure = Some(message.clone());
                        SearchStatus::Failed(message)
                    }
                };

                if let Some(message) = failure {
                    this.record(&format!("Metadata search failed: {message}"), cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn select_search_result(
        &mut self,
        source: Source,
        external_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &mut self.item_editor else {
            return;
        };
        let results = match &editor.search {
            SearchStatus::Results(results) => results,
            _ => return,
        };
        let Some(candidate) = results
            .iter()
            .find(|candidate| candidate.source == source && candidate.external_id == external_id)
            .cloned()
        else {
            return;
        };

        editor.lookup = Some(items::Lookup {
            source: candidate.source,
            external_id: candidate.external_id.clone(),
            cover_url: candidate.cover_url.clone(),
        });

        let title = candidate.title;
        let year = candidate
            .release_year
            .map(|year| year.to_string())
            .unwrap_or_default();
        editor
            .title
            .update(cx, |state, cx| state.set_value(title, window, cx));
        editor
            .year
            .update(cx, |state, cx| state.set_value(year, window, cx));

        cx.notify();
    }

    fn save_item(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.item_editor.take() else {
            return;
        };
        let title = editor.title.read(cx).value().trim().to_owned();
        let year_text = editor.year.read(cx).value().trim().to_owned();
        let year = if year_text.is_empty() {
            None
        } else {
            match year_text.parse::<i32>() {
                Ok(year) => Some(year),
                Err(_) => {
                    self.item_editor = Some(editor);
                    return;
                }
            }
        };
        if title.is_empty() {
            self.item_editor = Some(editor);
            return;
        }
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        let id = editor.id;
        let kind = editor.kind;
        let lookup = editor.lookup;
        let list_id = self.selected.clone();
        self.close_season_panels();

        self.run(cx, async move {
            let database = Database::open(path)?;
            match id {
                Some(id) => {
                    items::update(&database, &id, kind, &title, year)?;
                }
                None => {
                    let created = match lookup {
                        Some(lookup) => {
                            items::create_from_lookup(&database, kind, &title, year, &lookup)?
                        }
                        None => items::create(&database, kind, &title, year)?,
                    };
                    if let Some(list_id) = &list_id {
                        list_items::add(&database, list_id, &created.id)?;
                    }
                }
            }
            load(&database)
        });
    }

    fn cancel_item(&mut self, cx: &mut Context<Self>) {
        self.item_editor = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        cx.notify();
    }

    fn begin_delete_item(&mut self, id: String, cx: &mut Context<Self>) {
        self.confirm_delete_item = Some(id);
        self.item_editor = None;
        self.selected = None;
        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        cx.notify();
    }

    fn cancel_delete_item(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete_item = None;
        cx.notify();
    }

    fn delete_item(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete_item.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            items::delete(&database, &id)?;
            load(&database)
        });
    }

    fn toggle_status_picker(&mut self, id: String, cx: &mut Context<Self>) {
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        self.status_picker = if self.status_picker.as_deref() == Some(id.as_str()) {
            None
        } else {
            Some(id)
        };
        cx.notify();
    }

    fn change_status(&mut self, item_id: String, status_id: String, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.status_picker = None;

        self.run(cx, async move {
            let database = Database::open(path)?;
            items::set_status(&database, &item_id, &status_id)?;
            load(&database)
        });
    }

    fn toggle_add_picker(&mut self, list_id: String, cx: &mut Context<Self>) {
        self.status_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
        self.add_picker = if self.add_picker.as_deref() == Some(list_id.as_str()) {
            None
        } else {
            Some(list_id)
        };
        cx.notify();
    }

    fn add_to_list(&mut self, list_id: String, item_id: String, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();

        self.run(cx, async move {
            let database = Database::open(path)?;
            list_items::add(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    fn remove_from_list(&mut self, list_id: String, item_id: String, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            list_items::remove(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    fn move_item_up(&mut self, list_id: String, item_id: String, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            list_items::move_up(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    fn move_item_down(&mut self, list_id: String, item_id: String, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            list_items::move_down(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    fn toggle_tag_editor(
        &mut self,
        target: TagTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let same = self
            .tag_editor
            .as_ref()
            .is_some_and(|editor| editor.target == target);
        if same {
            self.tag_editor = None;
            self.close_tag_panels();
            self.close_season_panels();
            cx.notify();
            return;
        }

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("New tag"));
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        self.status_picker = None;
        self.add_picker = None;
        self.close_tag_panels();
        self.close_season_panels();
        self.tag_editor = Some(TagEditor { target, input });
        cx.notify();
    }

    fn save_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.tag_editor else {
            return;
        };
        let name = editor.input.read(cx).value().trim().to_owned();
        if name.is_empty() {
            return;
        }
        let target = editor.target.clone();
        let input = editor.input.clone();
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        input.update(cx, |input, cx| input.set_value("", window, cx));

        self.run(cx, async move {
            let database = Database::open(path)?;
            let tag = tags::create(&database, &name)?;
            match target {
                TagTarget::Item(item_id) => {
                    tags::attach_to_item(&database, &item_id, &tag.id)?;
                }
                TagTarget::List(list_id) => {
                    tags::attach_to_list(&database, &list_id, &tag.id)?;
                }
            }
            load(&database)
        });
    }

    fn toggle_tag(&mut self, target: TagTarget, tag_id: String, cx: &mut Context<Self>) {
        let attached = self.tag_is_attached(&target, &tag_id);
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            match (&target, attached) {
                (TagTarget::Item(item_id), true) => {
                    tags::detach_from_item(&database, item_id, &tag_id)?;
                }
                (TagTarget::Item(item_id), false) => {
                    tags::attach_to_item(&database, item_id, &tag_id)?;
                }
                (TagTarget::List(list_id), true) => {
                    tags::detach_from_list(&database, list_id, &tag_id)?;
                }
                (TagTarget::List(list_id), false) => {
                    tags::attach_to_list(&database, list_id, &tag_id)?;
                }
            }
            load(&database)
        });
    }

    fn begin_rename_tag(&mut self, tag: &tags::Tag, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).default_value(tag.name.clone());
            input.select_all(window, cx);
            input
        });
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        self.confirm_delete_tag = None;
        self.renaming_tag = Some(TagRename {
            id: tag.id.clone(),
            input,
        });
        cx.notify();
    }

    fn save_tag_rename(&mut self, cx: &mut Context<Self>) {
        let Some(rename) = self.renaming_tag.take() else {
            return;
        };
        let name = rename.input.read(cx).value().trim().to_owned();
        if name.is_empty() {
            self.renaming_tag = Some(rename);
            return;
        }
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        let id = rename.id;

        self.run(cx, async move {
            let database = Database::open(path)?;
            tags::rename(&database, &id, &name)?;
            load(&database)
        });
    }

    fn cancel_tag_rename(&mut self, cx: &mut Context<Self>) {
        self.renaming_tag = None;
        cx.notify();
    }

    fn begin_delete_tag(&mut self, tag_id: String, cx: &mut Context<Self>) {
        self.renaming_tag = None;
        self.confirm_delete_tag = Some(tag_id);
        cx.notify();
    }

    fn cancel_delete_tag(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete_tag = None;
        cx.notify();
    }

    fn delete_tag(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete_tag.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            tags::delete(&database, &id)?;
            load(&database)
        });
    }

    fn tag_is_attached(&self, target: &TagTarget, tag_id: &str) -> bool {
        match target {
            TagTarget::Item(id) => self
                .item_tags
                .iter()
                .any(|link| &link.owner_id == id && link.tag_id == tag_id),
            TagTarget::List(id) => self
                .list_tags
                .iter()
                .any(|link| &link.owner_id == id && link.tag_id == tag_id),
        }
    }

    fn close_tag_panels(&mut self) {
        self.renaming_tag = None;
        self.confirm_delete_tag = None;
    }

    fn close_season_panels(&mut self) {
        self.season_editor = None;
        self.season_status_picker = None;
        self.confirm_delete_season = None;
    }

    fn toggle_seasons(&mut self, show_id: String, window: &mut Window, cx: &mut Context<Self>) {
        let same = self
            .season_editor
            .as_ref()
            .is_some_and(|editor| editor.show_id == show_id);
        if same {
            self.close_season_panels();
            cx.notify();
            return;
        }

        let number = cx.new(|cx| InputState::new(window, cx).placeholder("Season number"));
        let focus = number.read(cx).focus_handle(cx);
        focus.focus(window, cx);

        self.status_picker = None;
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.season_status_picker = None;
        self.confirm_delete_season = None;
        self.season_editor = Some(SeasonEditor { show_id, number });
        cx.notify();
    }

    fn add_season(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.season_editor else {
            return;
        };
        let Some(number) = season_number(editor.number.read(cx).value().as_ref()) else {
            return;
        };
        let show_id = editor.show_id.clone();
        let input = editor.number.clone();
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        input.update(cx, |input, cx| input.set_value("", window, cx));

        self.run(cx, async move {
            let database = Database::open(path)?;
            seasons::create(&database, &show_id, number)?;
            load(&database)
        });
    }

    fn toggle_season_status(&mut self, season_id: String, cx: &mut Context<Self>) {
        self.season_status_picker =
            if self.season_status_picker.as_deref() == Some(season_id.as_str()) {
                None
            } else {
                Some(season_id)
            };
        cx.notify();
    }

    fn change_season_status(
        &mut self,
        season_id: String,
        status_id: String,
        cx: &mut Context<Self>,
    ) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.season_status_picker = None;

        self.run(cx, async move {
            let database = Database::open(path)?;
            seasons::set_status(&database, &season_id, &status_id)?;
            load(&database)
        });
    }

    fn begin_delete_season(&mut self, season_id: String, cx: &mut Context<Self>) {
        self.season_status_picker = None;
        self.confirm_delete_season = Some(season_id);
        cx.notify();
    }

    fn cancel_delete_season(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete_season = None;
        cx.notify();
    }

    fn delete_season(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete_season.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            seasons::delete(&database, &id)?;
            load(&database)
        });
    }

    fn run<F>(&mut self, cx: &mut Context<Self>, work: F)
    where
        F: Future<Output = CoreResult<Loaded>> + Send + 'static,
    {
        self.loading = true;
        let task = cx.background_spawn(work);

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(loaded) => {
                        if let Some(id) = this.selected.clone()
                            && !loaded.lists.iter().any(|list| list.id == id)
                        {
                            this.selected = None;
                            this.editor = None;
                            this.confirm_delete = None;
                            this.add_picker = None;
                            this.tag_editor = None;
                        }
                        this.lists = loaded.lists;
                        this.items = loaded.items;
                        this.statuses = loaded.statuses;
                        this.memberships = loaded.memberships;
                        this.tags = loaded.tags;
                        this.item_tags = loaded.item_tags;
                        this.list_tags = loaded.list_tags;
                        this.seasons = loaded.seasons;
                        this.error = None;
                    }
                    Err(error) => {
                        let message = format!("Could not update the database: {error}");
                        this.record(&message, cx);
                        this.error = Some(message);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn main_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        match (&self.storage, &self.error) {
            (StorageState::Failed(message), _) => div()
                .id("storage-error")
                .role(Role::Alert)
                .test_support()
                .aria_label(message.clone())
                .text_color(rgb(palette.danger))
                .text_xs()
                .child(message.clone())
                .into_any_element(),
            (StorageState::Ready(_), _) if self.loading => Self::loading_element(palette),
            (StorageState::Ready(_), Some(message)) => div()
                .id("lists-error")
                .role(Role::Alert)
                .test_support()
                .aria_label(message.clone())
                .text_color(rgb(palette.danger))
                .text_xs()
                .child(message.clone())
                .into_any_element(),
            (StorageState::Ready(_), None) => {
                if self.about {
                    self.about_element(palette)
                } else if self.settings_screen {
                    self.settings_element(palette, cx)
                } else if self.editor.is_some() {
                    self.editor_element(palette, cx)
                } else if let Some(id) = &self.confirm_delete {
                    let name = self
                        .lists
                        .iter()
                        .find(|list| &list.id == id)
                        .map(|list| list.name.as_str())
                        .unwrap_or("this list");
                    self.delete_element(name, cx)
                } else if self.item_editor.is_some() {
                    self.item_editor_element(palette, cx)
                } else if let Some(id) = &self.confirm_delete_item {
                    let title = self
                        .items
                        .iter()
                        .find(|item| &item.id == id)
                        .map(|item| item.title.as_str())
                        .unwrap_or("this item");
                    self.delete_item_element(title, cx)
                } else if let Some(id) = &self.selected {
                    match self.lists.iter().find(|list| &list.id == id) {
                        Some(list) => self.list_panel(list, palette, cx),
                        None => Self::empty_element(palette),
                    }
                } else if self.items.is_empty() {
                    Self::empty_element(palette)
                } else {
                    let items: Vec<&items::Item> = self.items.iter().collect();
                    self.items_element(&items, None, palette, cx)
                }
            }
        }
    }

    fn empty_element(palette: &Palette) -> AnyElement {
        div()
            .id("empty-state")
            .role(Role::Status)
            .test_support()
            .aria_label(EMPTY_MESSAGE)
            .text_color(rgb(palette.muted))
            .child(EMPTY_MESSAGE)
            .into_any_element()
    }

    fn about_element(&self, palette: &Palette) -> AnyElement {
        let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
        let data_path = match &self.storage {
            StorageState::Ready(path) => path.display().to_string(),
            StorageState::Failed(_) => String::new(),
        };

        div()
            .id("about-screen")
            .role(Role::Region)
            .test_support()
            .aria_label("About Loregalia")
            .flex()
            .flex_col()
            .gap_6()
            .w_full()
            .h_full()
            .max_w(px(640.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("About Loregalia"),
                    )
                    .child(
                        div()
                            .id("about-version")
                            .test_support()
                            .aria_label(version.clone())
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(version),
                    )
                    .child(
                        div()
                            .max_w(px(520.0))
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child(
                                "A local-first planner for what to watch, read and play next. \
                                 Your data stays on this computer; there is no account and no server.",
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Metadata"),
                    )
                    .child(
                        div()
                            .id("about-tvmaze")
                            .test_support()
                            .aria_label("TVmaze, used under CC BY-SA 4.0")
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child("Show metadata from")
                            .child(
                                div()
                                    .id("tvmaze-link")
                                    .test_support()
                                    .child(
                                        Link::new("tvmaze-anchor")
                                            .href("https://www.tvmaze.com/api")
                                            .child("TVmaze"),
                                    ),
                            )
                            .child(", used under")
                            .child(
                                Link::new("cc-by-sa-link")
                                    .href("https://creativecommons.org/licenses/by-sa/4.0/")
                                    .child("CC BY-SA 4.0"),
                            )
                            .child("."),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child("More sources will be credited here as they are added."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("License"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child("Loregalia is released under the")
                            .child(
                                Link::new("license-link")
                                    .href("https://github.com/ManusolJ/loregalia/blob/main/LICENSE")
                                    .child("MIT License"),
                            )
                            .child("."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Data"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(palette.muted))
                            .child(data_path),
                    ),
            )
            .into_any_element()
    }

    fn settings_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut panel =
            div()
                .id("settings-screen")
                .role(Role::Region)
                .test_support()
                .aria_label(format!("Settings, {} theme", self.theme.label()))
                .flex()
                .flex_col()
                .gap_6()
                .w_full()
                .h_full()
                .max_w(px(640.0))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Settings"),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Theme"),
                        )
                        .child(
                            div()
                                .max_w(px(520.0))
                                .text_sm()
                                .text_color(rgb(palette.muted))
                                .child("Follow the system theme, or keep a fixed one."),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .children(settings::Theme::ALL.map(|theme| {
                                    Button::new(format!("theme-{}", theme.key()))
                                        .label(theme.label())
                                        .selected(self.theme == theme)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.set_theme(theme, cx)
                                        }))
                                })),
                        ),
                );

        if let Some(message) = &self.settings_error {
            panel = panel.child(
                div()
                    .id("settings-error")
                    .role(Role::Alert)
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(rgb(palette.danger))
                    .text_xs()
                    .child(message.clone()),
            );
        }

        panel.into_any_element()
    }

    fn editor_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.editor else {
            return Self::empty_element(palette);
        };
        let (title, input) = match editor {
            Editor::Create(input) => ("New list", input),
            Editor::Rename { input, .. } => ("Rename list", input),
        };
        let can_save = !input.read(cx).value().trim().is_empty();

        div()
            .id("list-editor")
            .test_support()
            .flex()
            .flex_col()
            .gap_3()
            .w(px(320.0))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
            .child(
                Input::new(input)
                    .id("list-name-input")
                    .aria_label("List name"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("save-list")
                            .label("Save")
                            .disabled(!can_save)
                            .on_click(cx.listener(|this, _, _, cx| this.save_editor(cx))),
                    )
                    .child(
                        Button::new("cancel-list")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_editor(cx))),
                    ),
            )
            .into_any_element()
    }

    fn items_in_list(&self, list_id: &str) -> Vec<&items::Item> {
        self.memberships
            .iter()
            .filter(|membership| membership.list_id == list_id)
            .filter_map(|membership| self.items.iter().find(|item| item.id == membership.item_id))
            .collect()
    }

    fn list_empty_element(palette: &Palette) -> AnyElement {
        div()
            .id("list-empty")
            .role(Role::Status)
            .test_support()
            .aria_label("No items in this list yet")
            .text_color(rgb(palette.muted))
            .child("No items in this list yet")
            .into_any_element()
    }

    fn add_picker_element(
        &self,
        list_id: &str,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let available: Vec<&items::Item> = self
            .items
            .iter()
            .filter(|item| {
                !self.memberships.iter().any(|membership| {
                    membership.list_id == list_id && membership.item_id == item.id
                })
            })
            .collect();

        if available.is_empty() {
            let message = if self.items.is_empty() {
                "Add an item first"
            } else {
                "Every item is already in this list"
            };

            return div()
                .id("add-picker-empty")
                .role(Role::Status)
                .test_support()
                .aria_label(message)
                .text_color(rgb(palette.muted))
                .text_xs()
                .child(message)
                .into_any_element();
        }

        let list_id = list_id.to_owned();

        div()
            .id("add-picker")
            .test_support()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(available.iter().map(|item| {
                let list_id = list_id.clone();
                let item_id = item.id.clone();

                Button::new(format!("add-to-list-{}", item.id))
                    .label(item.title.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.add_to_list(list_id.clone(), item_id.clone(), cx)
                    }))
            }))
            .into_any_element()
    }

    fn list_panel(
        &self,
        list: &lists::List,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut members = self.items_in_list(&list.id);
        list_items::sort_items(&mut members, self.sort);
        let adding = self.add_picker.as_deref() == Some(list.id.as_str());
        let list_id = list.id.clone();
        let tag_picker = match &self.tag_editor {
            Some(editor) if matches!(&editor.target, TagTarget::List(id) if id == &list.id) => {
                Some(self.tag_picker_element(editor, cx))
            }
            _ => None,
        };

        div()
            .id("list-panel")
            .role(Role::Region)
            .test_support()
            .aria_label(list.name.clone())
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .h_full()
            .max_w(px(640.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(list.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(Self::tags_button(
                                TagTarget::List(list.id.clone()),
                                "list-tags".to_owned(),
                                cx,
                            ))
                            .child(Button::new("rename-list").label("Rename").on_click(
                                cx.listener(|this, _, window, cx| this.begin_rename(window, cx)),
                            ))
                            .child(
                                Button::new("delete-list")
                                    .label("Delete")
                                    .danger()
                                    .on_click(cx.listener(|this, _, _, cx| this.begin_delete(cx))),
                            ),
                    ),
            )
            .child(div().flex().flex_wrap().gap_1().children(
                self.tags_for_list(&list.id).into_iter().map(|tag| {
                    Self::tag_chip(format!("list-tag-{}-{}", list.id, tag.id), tag, palette)
                }),
            ))
            .child(
                div().flex().gap_2().child(
                    Button::new("add-existing-item")
                        .label("Add existing item")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_add_picker(list_id.clone(), cx)
                        })),
                ),
            )
            .when(adding, |panel| {
                panel.child(self.add_picker_element(&list.id, palette, cx))
            })
            .children(tag_picker)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_xs().text_color(rgb(palette.muted)).child("Sort"))
                    .child(
                        Button::new("sort-manual")
                            .label("Manual")
                            .selected(self.sort == list_items::Sort::Manual)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_sort(list_items::Sort::Manual, cx)
                            })),
                    )
                    .child(
                        Button::new("sort-title")
                            .label("Title")
                            .selected(self.sort == list_items::Sort::Title)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_sort(list_items::Sort::Title, cx)
                            })),
                    )
                    .child(
                        Button::new("sort-year")
                            .label("Year")
                            .selected(self.sort == list_items::Sort::Year)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.set_sort(list_items::Sort::Year, cx)
                            })),
                    ),
            )
            .child(if members.is_empty() {
                Self::list_empty_element(palette)
            } else {
                self.items_element(&members, Some(&list.id), palette, cx)
            })
            .into_any_element()
    }

    fn delete_element(&self, name: &str, cx: &mut Context<Self>) -> AnyElement {
        let message = format!("Delete \"{name}\"?");

        div()
            .id("delete-confirmation")
            .role(Role::Alert)
            .test_support()
            .aria_label(message.clone())
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .child(message)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("cancel-delete")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_delete(cx))),
                    )
                    .child(
                        Button::new("confirm-delete")
                            .label("Delete")
                            .danger()
                            .on_click(cx.listener(|this, _, _, cx| this.delete_selected(cx))),
                    ),
            )
            .into_any_element()
    }

    fn item_editor_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.item_editor else {
            return Self::empty_element(palette);
        };
        let title = editor.title.read(cx).value().trim().to_owned();
        let year = editor.year.read(cx).value().trim().to_owned();
        let can_save = !title.is_empty() && (year.is_empty() || year.parse::<i32>().is_ok());
        let duplicate = if editor.id.is_none() && editor.lookup.is_none() {
            items::duplicate(&self.items, editor.kind, &title)
        } else {
            None
        };
        let save_label = if duplicate.is_some() {
            "Save anyway"
        } else {
            "Save"
        };
        let heading = if editor.id.is_some() {
            "Edit item"
        } else {
            "New item"
        };

        let mut panel = div()
            .id("item-editor")
            .test_support()
            .flex()
            .flex_col()
            .gap_3()
            .w(px(320.0))
            .child(div().font_weight(FontWeight::SEMIBOLD).child(heading))
            .child(
                Input::new(&editor.title)
                    .id("item-title-input")
                    .aria_label("Title"),
            )
            .child(div().flex().gap_2().children(MediaKind::ALL.map(|kind| {
                Button::new(format!("item-kind-{}", kind.key()))
                    .label(kind.label())
                    .selected(editor.kind == kind)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_item_kind(kind, window, cx)
                    }))
            })))
            .child(
                Input::new(&editor.year)
                    .id("item-year-input")
                    .aria_label("Release year"),
            );

        if editor.id.is_none() && providers::supported(editor.kind) {
            panel = panel.child(self.metadata_search_element(editor, palette, cx));
        }

        if let Some(duplicate) = duplicate {
            let message = format!(
                "\"{}\" is already in your library. You can still save it.",
                duplicate.title
            );
            panel = panel.child(
                div()
                    .id("item-duplicate-warning")
                    .role(Role::Alert)
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(rgb(palette.status_paused))
                    .text_xs()
                    .child(message),
            );
        }

        panel
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("save-item")
                            .label(save_label)
                            .disabled(!can_save)
                            .on_click(cx.listener(|this, _, _, cx| this.save_item(cx))),
                    )
                    .child(
                        Button::new("cancel-item")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_item(cx))),
                    ),
            )
            .into_any_element()
    }

    fn has_candidate(&self, candidate: &Candidate) -> bool {
        self.items.iter().any(|item| {
            item.source.as_deref() == Some(candidate.source.key())
                && item.external_id.as_deref() == Some(candidate.external_id.as_str())
        })
    }

    fn metadata_search_element(
        &self,
        editor: &ItemEditor,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(input) = &editor.search_input else {
            return div().into_any_element();
        };
        let query = input.read(cx).value().trim().to_owned();
        let searching = matches!(editor.search, SearchStatus::Searching);
        let source_label = providers::source(editor.kind)
            .map(Source::label)
            .unwrap_or("metadata");

        let mut panel = div()
            .id("metadata-search")
            .test_support()
            .aria_label("Search metadata")
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted))
                    .child(format!("Results from {source_label}")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Input::new(input)
                            .id("metadata-search-input")
                            .aria_label("Search by title"),
                    )
                    .child(
                        Button::new("search-metadata")
                            .label("Search")
                            .disabled(query.is_empty() || searching)
                            .on_click(cx.listener(|this, _, _, cx| this.search_metadata(cx))),
                    ),
            );

        panel = match &editor.search {
            SearchStatus::Idle => panel,
            SearchStatus::Searching => panel.child(
                div()
                    .id("metadata-search-loading")
                    .role(Role::Status)
                    .test_support()
                    .aria_label(format!("Searching {source_label}"))
                    .text_color(rgb(palette.muted))
                    .text_xs()
                    .child(format!("Searching {source_label}")),
            ),
            SearchStatus::Empty => panel.child(
                div()
                    .id("metadata-search-empty")
                    .role(Role::Status)
                    .test_support()
                    .aria_label(format!("No matches on {source_label}"))
                    .text_color(rgb(palette.muted))
                    .text_xs()
                    .child(format!("No matches on {source_label}")),
            ),
            SearchStatus::Failed(message) => panel.child(
                div()
                    .id("metadata-search-failed")
                    .role(Role::Alert)
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(rgb(palette.danger))
                    .text_xs()
                    .child(message.clone()),
            ),
            SearchStatus::Results(results) => {
                let results = results.iter().map(|candidate| {
                    let already_added = self.has_candidate(candidate);
                    let label = candidate_label(candidate, already_added);
                    let source = candidate.source;
                    let external_id = candidate.external_id.clone();
                    let id = format!("search-result-{}-{}", source.key(), external_id);

                    Button::new(id)
                        .label(label)
                        .disabled(already_added)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_search_result(source, external_id.clone(), window, cx)
                        }))
                });

                panel.child(
                    div()
                        .id("metadata-search-results")
                        .test_support()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .max_h(px(240.0))
                        .overflow_y_scroll()
                        .children(results),
                )
            }
        };

        panel.into_any_element()
    }

    fn category_color(palette: &Palette, category: statuses::Category) -> u32 {
        match category {
            statuses::Category::Planned => palette.status_planned,
            statuses::Category::Active => palette.status_active,
            statuses::Category::Paused => palette.status_paused,
            statuses::Category::Done => palette.status_done,
            statuses::Category::Dropped => palette.status_dropped,
        }
    }

    fn category_icon(category: statuses::Category) -> IconName {
        match category {
            statuses::Category::Planned => IconName::CircleDashed,
            statuses::Category::Active => IconName::CirclePlay,
            statuses::Category::Paused => IconName::CirclePause,
            statuses::Category::Done => IconName::CircleCheck,
            statuses::Category::Dropped => IconName::CircleX,
        }
    }

    fn status_chip(status: &statuses::Status, id: String, palette: &Palette) -> Button {
        let color = rgb(Self::category_color(palette, status.category));

        Button::new(id)
            .ghost()
            .h(px(24.0))
            .px_2()
            .rounded(px(6.0))
            .bg(color.alpha(0.15))
            .accessibility_label(status.label.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Icon::new(Self::category_icon(status.category))
                            .with_size(px(16.0))
                            .text_color(color),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(palette.content))
                            .child(status.label.clone()),
                    ),
            )
    }

    fn status_picker_element(&self, item: &items::Item, cx: &mut Context<Self>) -> AnyElement {
        let item_id = item.id.clone();

        div()
            .id(format!("status-picker-{}", item.id))
            .test_support()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(
                self.statuses
                    .iter()
                    .filter(|status| status.media_kind == item.media_kind)
                    .map(|status| {
                        let item_id = item_id.clone();
                        let status_id = status.id.clone();

                        Button::new(format!("item-status-{}-{}", item.id, status.id))
                            .label(status.label.clone())
                            .selected(status.id == item.status_id)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_status(item_id.clone(), status_id.clone(), cx)
                            }))
                    }),
            )
            .into_any_element()
    }

    fn tags_for_item(&self, item_id: &str) -> Vec<&tags::Tag> {
        let mut attached: Vec<&tags::Tag> = self
            .item_tags
            .iter()
            .filter(|link| link.owner_id == item_id)
            .filter_map(|link| self.tags.iter().find(|tag| tag.id == link.tag_id))
            .collect();
        attached.sort_by(|left, right| left.name.cmp(&right.name));
        attached
    }

    fn tags_for_list(&self, list_id: &str) -> Vec<&tags::Tag> {
        let mut attached: Vec<&tags::Tag> = self
            .list_tags
            .iter()
            .filter(|link| link.owner_id == list_id)
            .filter_map(|link| self.tags.iter().find(|tag| tag.id == link.tag_id))
            .collect();
        attached.sort_by(|left, right| left.name.cmp(&right.name));
        attached
    }

    fn tag_chip(id: String, tag: &tags::Tag, palette: &Palette) -> AnyElement {
        div()
            .id(id)
            .test_support()
            .aria_label(tag.name.clone())
            .h(px(24.0))
            .px_2()
            .flex()
            .items_center()
            .rounded(px(6.0))
            .bg(rgb(palette.raised))
            .border_1()
            .border_color(rgb(palette.line))
            .text_xs()
            .text_color(rgb(palette.muted))
            .child(tag.name.clone())
            .into_any_element()
    }

    fn tags_button(target: TagTarget, id: String, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .icon(IconName::Tag)
            .tooltip("Tags")
            .accessibility_label("Tags")
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_tag_editor(target.clone(), window, cx)
            }))
    }

    fn tag_row(&self, tag: &tags::Tag, target: &TagTarget, cx: &mut Context<Self>) -> AnyElement {
        if let Some(rename) = &self.renaming_tag
            && rename.id == tag.id
        {
            let can_save = !rename.input.read(cx).value().trim().is_empty();

            return div()
                .id(format!("tag-row-{}", tag.id))
                .test_support()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Input::new(&rename.input)
                        .id("tag-rename-input")
                        .aria_label("Tag name"),
                )
                .child(
                    Button::new("save-tag-rename")
                        .label("Save")
                        .disabled(!can_save)
                        .on_click(cx.listener(|this, _, _, cx| this.save_tag_rename(cx))),
                )
                .child(
                    Button::new("cancel-tag-rename")
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_tag_rename(cx))),
                )
                .into_any_element();
        }

        if self.confirm_delete_tag.as_deref() == Some(tag.id.as_str()) {
            let message = format!("Delete tag \"{}\"?", tag.name);

            return div()
                .id("tag-delete-confirmation")
                .role(Role::Alert)
                .test_support()
                .aria_label(message.clone())
                .flex()
                .items_center()
                .gap_2()
                .child(message)
                .child(
                    Button::new("cancel-tag-delete")
                        .label("Cancel")
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_delete_tag(cx))),
                )
                .child(
                    Button::new("confirm-tag-delete")
                        .label("Delete")
                        .danger()
                        .on_click(cx.listener(|this, _, _, cx| this.delete_tag(cx))),
                )
                .into_any_element();
        }

        let target = target.clone();
        let tag_id = tag.id.clone();
        let attached = self.tag_is_attached(&target, &tag.id);
        let rename_target = tag.clone();
        let delete_id = tag.id.clone();

        div()
            .id(format!("tag-row-{}", tag.id))
            .test_support()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div().flex_1().child(
                    Button::new(format!("tag-toggle-{}", tag.id))
                        .w_full()
                        .justify_start()
                        .accessibility_label(tag.name.clone())
                        .selected(attached)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_tag(target.clone(), tag_id.clone(), cx)
                        }))
                        .child(div().w_full().child(tag.name.clone())),
                ),
            )
            .child(
                Button::new(format!("rename-tag-{}", tag.id))
                    .icon(IconName::Pencil)
                    .tooltip("Rename tag")
                    .accessibility_label("Rename tag")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_rename_tag(&rename_target, window, cx)
                    })),
            )
            .child(
                Button::new(format!("delete-tag-{}", tag.id))
                    .icon(IconName::Trash)
                    .tooltip("Delete tag")
                    .accessibility_label("Delete tag")
                    .danger()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.begin_delete_tag(delete_id.clone(), cx)
                    })),
            )
            .into_any_element()
    }

    fn tag_picker_element(&self, editor: &TagEditor, cx: &mut Context<Self>) -> AnyElement {
        let target = editor.target.clone();
        let can_save = !editor.input.read(cx).value().trim().is_empty();

        div()
            .id("tag-picker")
            .test_support()
            .flex()
            .flex_col()
            .gap_2()
            .children(self.tags.iter().map(|tag| self.tag_row(tag, &target, cx)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Input::new(&editor.input)
                            .id("tag-name-input")
                            .aria_label("Tag name"),
                    )
                    .child(
                        Button::new("save-tag")
                            .label("Add")
                            .disabled(!can_save)
                            .on_click(cx.listener(|this, _, window, cx| this.save_tag(window, cx))),
                    ),
            )
            .into_any_element()
    }

    fn seasons_for(&self, show_id: &str) -> Vec<&seasons::Season> {
        self.seasons
            .iter()
            .filter(|season| season.show_id == show_id)
            .collect()
    }

    fn seasons_button(item: &items::Item, cx: &mut Context<Self>) -> Option<Button> {
        if item.media_kind != MediaKind::Show {
            return None;
        }
        let id = item.id.clone();

        Some(
            Button::new(format!("item-seasons-{}", item.id))
                .icon(IconName::ListOrdered)
                .tooltip("Seasons")
                .accessibility_label("Seasons")
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.toggle_seasons(id.clone(), window, cx)
                })),
        )
    }

    fn season_row(
        &self,
        season: &seasons::Season,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = format!("Season {}", season.season_number);
        let chip =
            self.statuses
                .iter()
                .find(|status| status.id == season.status_id)
                .map(|status| {
                    let season_id = season.id.clone();
                    Self::status_chip(status, format!("season-status-{}", season.id), palette)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_season_status(season_id.clone(), cx)
                        }))
                });
        let picker = (self.season_status_picker.as_deref() == Some(season.id.as_str()))
            .then(|| self.season_status_picker_element(season, cx));

        div()
            .id(format!("season-{}", season.id))
            .role(Role::ListItem)
            .test_support()
            .aria_label(label.clone())
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(div().child(label))
                    .child(
                        div().flex().items_center().gap_2().children(chip).child(
                            Button::new(format!("season-delete-{}", season.id))
                                .label("Delete")
                                .danger()
                                .on_click(cx.listener({
                                    let season_id = season.id.clone();
                                    move |this, _, _, cx| {
                                        this.begin_delete_season(season_id.clone(), cx)
                                    }
                                })),
                        ),
                    ),
            )
            .children(picker)
            .into_any_element()
    }

    fn season_status_picker_element(
        &self,
        season: &seasons::Season,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let season_id = season.id.clone();

        div()
            .id(format!("season-status-picker-{}", season.id))
            .test_support()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(
                self.statuses
                    .iter()
                    .filter(|status| status.media_kind == MediaKind::Show)
                    .map(|status| {
                        let season_id = season_id.clone();
                        let status_id = status.id.clone();

                        Button::new(format!("season-set-status-{}-{}", season.id, status.id))
                            .label(status.label.clone())
                            .selected(status.id == season.status_id)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_season_status(season_id.clone(), status_id.clone(), cx)
                            }))
                    }),
            )
            .into_any_element()
    }

    fn season_delete_element(&self, season_id: Option<&str>, cx: &mut Context<Self>) -> AnyElement {
        let Some(season_id) = season_id else {
            return div().into_any_element();
        };
        let message = self
            .seasons
            .iter()
            .find(|season| season.id == season_id)
            .map(|season| format!("Delete season {}?", season.season_number))
            .unwrap_or_else(|| "Delete this season?".to_owned());

        div()
            .id("season-delete-confirmation")
            .role(Role::Alert)
            .test_support()
            .aria_label(message.clone())
            .flex()
            .items_center()
            .gap_2()
            .child(message)
            .child(
                Button::new("cancel-season-delete")
                    .label("Cancel")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_delete_season(cx))),
            )
            .child(
                Button::new("confirm-season-delete")
                    .label("Delete")
                    .danger()
                    .on_click(cx.listener(|this, _, _, cx| this.delete_season(cx))),
            )
            .into_any_element()
    }

    fn season_panel_element(
        &self,
        editor: &SeasonEditor,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let show_id = editor.show_id.clone();
        let seasons = self.seasons_for(&show_id);
        let typed = season_number(editor.number.read(cx).value().as_ref());
        let can_add = typed
            .is_some_and(|number| !seasons.iter().any(|season| season.season_number == number));
        let confirming = self.confirm_delete_season.clone();

        div()
            .id("seasons-panel")
            .test_support()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded(px(8.0))
            .bg(rgb(palette.raised))
            .border_1()
            .border_color(rgb(palette.line))
            .child(if seasons.is_empty() {
                div()
                    .id("seasons-empty")
                    .role(Role::Status)
                    .test_support()
                    .aria_label("No seasons yet")
                    .text_color(rgb(palette.muted))
                    .text_xs()
                    .child("No seasons yet")
                    .into_any_element()
            } else {
                div()
                    .id("seasons")
                    .role(Role::List)
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(
                        seasons
                            .iter()
                            .map(|season| self.season_row(season, palette, cx)),
                    )
                    .into_any_element()
            })
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Input::new(&editor.number)
                            .id("season-number-input")
                            .aria_label("Season number"),
                    )
                    .child(
                        Button::new("add-season")
                            .label("Add season")
                            .disabled(!can_add)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_season(window, cx)),
                            ),
                    ),
            )
            .when(confirming.is_some(), |panel| {
                panel.child(self.season_delete_element(confirming.as_deref(), cx))
            })
            .into_any_element()
    }

    fn media_kind_icon(kind: MediaKind) -> IconName {
        match kind {
            MediaKind::Movie => IconName::Film,
            MediaKind::Show => IconName::Tv,
            MediaKind::Book => IconName::BookOpen,
            MediaKind::Game => IconName::Gamepad2,
        }
    }

    fn cover_element(item: &items::Item, palette: &Palette) -> AnyElement {
        let mut cover = div()
            .id(format!("cover-{}", item.id))
            .role(Role::Image)
            .test_support()
            .aria_label(format!("Cover for {}", item.title))
            .relative()
            .w(px(32.0))
            .h(px(48.0))
            .flex_none()
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(palette.line))
            .overflow_hidden()
            .bg(rgb(palette.surface))
            .child(
                div()
                    .id(format!("cover-fallback-{}", item.id))
                    .test_support()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(Self::media_kind_icon(item.media_kind))
                            .with_size(px(16.0))
                            .text_color(rgb(palette.muted)),
                    ),
            );

        if let Some(url) = &item.cover_url {
            cover = cover.child(
                img(url.clone())
                    .id(format!("cover-image-{}", item.id))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .test_support(),
            );
        }

        cover.into_any_element()
    }

    fn item_row(
        &self,
        item: &items::Item,
        list_actions: Option<ListItemActions<'_>>,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let metadata = match item.release_year {
            Some(year) => format!("{} · {year}", item.media_kind.label()),
            None => item.media_kind.label().to_owned(),
        };
        let id = item.id.clone();
        let chip = self
            .statuses
            .iter()
            .find(|status| status.id == item.status_id)
            .map(|status| {
                let item_id = item.id.clone();
                Self::status_chip(status, format!("item-status-{}", item.id), palette).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.toggle_status_picker(item_id.clone(), cx)
                    }),
                )
            });
        let picker = (self.status_picker.as_deref() == Some(item.id.as_str()))
            .then(|| self.status_picker_element(item, cx));
        let tag_picker = match &self.tag_editor {
            Some(editor) if matches!(&editor.target, TagTarget::Item(id) if id == &item.id) => {
                Some(self.tag_picker_element(editor, cx))
            }
            _ => None,
        };
        let season_panel = match &self.season_editor {
            Some(editor) if editor.show_id == item.id => {
                Some(self.season_panel_element(editor, palette, cx))
            }
            _ => None,
        };

        let actions = match list_actions {
            Some(list_actions) => {
                let list_id = list_actions.list_id.to_owned();
                let up_list_id = list_id.clone();
                let down_list_id = list_id.clone();
                let up_id = id.clone();
                let down_id = id.clone();

                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(list_actions.manual, |actions| {
                        actions
                            .child(
                                Button::new(format!("move-up-{}", item.id))
                                    .icon(IconName::ArrowUp)
                                    .tooltip("Move up")
                                    .accessibility_label("Move up")
                                    .disabled(!list_actions.can_move_up)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.move_item_up(up_list_id.clone(), up_id.clone(), cx)
                                    })),
                            )
                            .child(
                                Button::new(format!("move-down-{}", item.id))
                                    .icon(IconName::ArrowDown)
                                    .tooltip("Move down")
                                    .accessibility_label("Move down")
                                    .disabled(!list_actions.can_move_down)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.move_item_down(
                                            down_list_id.clone(),
                                            down_id.clone(),
                                            cx,
                                        )
                                    })),
                            )
                    })
                    .children(Self::seasons_button(item, cx))
                    .child(Self::tags_button(
                        TagTarget::Item(id.clone()),
                        format!("item-tags-{}", item.id),
                        cx,
                    ))
                    .child(
                        Button::new(format!("item-edit-{}", item.id))
                            .label("Edit")
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, window, cx| {
                                    this.begin_edit_item(id.clone(), window, cx)
                                }
                            })),
                    )
                    .child(
                        Button::new(format!("remove-from-list-{}", item.id))
                            .label("Remove")
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    this.remove_from_list(list_id.clone(), id.clone(), cx)
                                }
                            })),
                    )
            }
            None => div()
                .flex()
                .items_center()
                .gap_2()
                .children(Self::seasons_button(item, cx))
                .child(Self::tags_button(
                    TagTarget::Item(id.clone()),
                    format!("item-tags-{}", item.id),
                    cx,
                ))
                .child(
                    Button::new(format!("item-edit-{}", item.id))
                        .label("Edit")
                        .on_click(cx.listener({
                            let id = id.clone();
                            move |this, _, window, cx| this.begin_edit_item(id.clone(), window, cx)
                        })),
                )
                .child(
                    Button::new(format!("item-delete-{}", item.id))
                        .label("Delete")
                        .danger()
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.begin_delete_item(id.clone(), cx)
                        })),
                ),
        };

        div()
            .id(format!("item-row-{}", item.id))
            .role(Role::ListItem)
            .test_support()
            .aria_label(item.title.clone())
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .bg(rgb(palette.surface))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_3()
                            .child(Self::cover_element(item, palette))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .min_w_0()
                                    .child(item.title.clone())
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(palette.muted))
                                            .child(metadata),
                                    )
                                    .child(div().flex().flex_wrap().gap_1().children(
                                        self.tags_for_item(&item.id).into_iter().map(|tag| {
                                            Self::tag_chip(
                                                format!("item-tag-{}-{}", item.id, tag.id),
                                                tag,
                                                palette,
                                            )
                                        }),
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .children(chip)
                            .child(actions),
                    ),
            )
            .children(picker)
            .children(tag_picker)
            .children(season_panel)
            .into_any_element()
    }

    fn items_element(
        &self,
        items: &[&items::Item],
        remove_from: Option<&str>,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("items")
            .role(Role::List)
            .test_support()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .max_w(px(640.0))
            .children(items.iter().enumerate().map(|(index, item)| {
                let list_actions = remove_from.map(|list_id| ListItemActions {
                    list_id,
                    manual: self.sort == list_items::Sort::Manual,
                    can_move_up: index > 0,
                    can_move_down: index + 1 < items.len(),
                });

                self.item_row(item, list_actions, palette, cx)
            }))
            .into_any_element()
    }

    fn delete_item_element(&self, title: &str, cx: &mut Context<Self>) -> AnyElement {
        let message = format!("Delete \"{title}\"?");

        div()
            .id("item-delete-confirmation")
            .role(Role::Alert)
            .test_support()
            .aria_label(message.clone())
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .child(message)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("cancel-item-delete")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_delete_item(cx))),
                    )
                    .child(
                        Button::new("confirm-item-delete")
                            .label("Delete")
                            .danger()
                            .on_click(cx.listener(|this, _, _, cx| this.delete_item(cx))),
                    ),
            )
            .into_any_element()
    }

    fn loading_element(palette: &Palette) -> AnyElement {
        div()
            .id("loading-state")
            .role(Role::Status)
            .test_support()
            .aria_label(LOADING_MESSAGE)
            .flex()
            .flex_col()
            .gap_3()
            .w(px(LOADING_BLOCK_WIDTH))
            .children([1.0_f32, 0.85, 0.6].map(|scale| {
                div()
                    .h(px(16.0))
                    .w(px(LOADING_BLOCK_WIDTH * scale))
                    .rounded(px(6.0))
                    .bg(rgb(palette.content))
                    .opacity(0.05)
            }))
            .into_any_element()
    }

    fn list_rows(&self, shows_labels: bool, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("lists")
            .role(Role::List)
            .test_support()
            .flex()
            .flex_col()
            .when(!shows_labels, |lists| lists.gap_1())
            .children(self.lists.iter().map(|list| {
                let selected = self.selected.as_deref() == Some(list.id.as_str());
                let id = list.id.clone();

                if shows_labels {
                    Button::new(format!("list-row-{}", list.id))
                        .ghost()
                        .w_full()
                        .h(px(36.0))
                        .px_3()
                        .justify_start()
                        .role(Role::ListItem)
                        .accessibility_label(list.name.clone())
                        .selected(selected)
                        .child(div().w_full().child(list.name.clone()))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.select_list(id.clone(), cx)),
                        )
                        .into_any_element()
                } else {
                    div()
                        .id(format!("list-row-{}", list.id))
                        .role(Role::ListItem)
                        .test_support()
                        .aria_label(list.name.clone())
                        .child(
                            Button::new(format!("list-button-{}", list.id))
                                .icon(IconName::Inbox)
                                .tooltip(list.name.clone())
                                .accessibility_label(list.name.clone())
                                .selected(selected)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.select_list(id.clone(), cx)
                                })),
                        )
                        .into_any_element()
                }
            }))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *cx.global::<Palette>();
        let width = sidebar_width(window.viewport_size().width);
        let shows_labels = width > px(RAIL_WIDTH);
        let can_create = matches!(&self.storage, StorageState::Ready(_));
        let main_element = self.main_element(&palette, cx);
        let create = Button::new("create-list")
            .disabled(self.loading)
            .on_click(cx.listener(|this, _, window, cx| this.begin_create(window, cx)));
        let create = if shows_labels {
            create.label("Create list")
        } else {
            create
                .icon(IconName::Plus)
                .tooltip("Create list")
                .accessibility_label("Create list")
        };
        let about = Button::new("about-button")
            .ghost()
            .on_click(cx.listener(|this, _, _, cx| this.begin_about(cx)));
        let about = if shows_labels {
            about.label("About")
        } else {
            about
                .icon(IconName::Info)
                .tooltip("About")
                .accessibility_label("About")
        };
        let settings = Button::new("settings-button")
            .ghost()
            .on_click(cx.listener(|this, _, _, cx| this.begin_settings(cx)));
        let settings = if shows_labels {
            settings.label("Settings")
        } else {
            settings
                .icon(IconName::Settings)
                .tooltip("Settings")
                .accessibility_label("Settings")
        };

        div()
            .size_full()
            .flex()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.content))
            .text_sm()
            .child(
                div()
                    .id("sidebar")
                    .test_support()
                    .w(width)
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .p_3()
                    .when(!shows_labels, |sidebar| sidebar.items_center())
                    .bg(rgb(palette.surface))
                    .border_r_1()
                    .border_color(rgb(palette.line))
                    .when(shows_labels, |sidebar| {
                        sidebar.child(
                            div()
                                .text_color(rgb(palette.muted))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Lists"),
                        )
                    })
                    .child(
                        div()
                            .id("list-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .when(shows_labels, |lists| lists.mt_3())
                            .child(self.list_rows(shows_labels, cx)),
                    )
                    .when(can_create, |sidebar| {
                        sidebar.child(div().mt_3().child(create))
                    })
                    .child(div().mt_3().child(settings))
                    .child(div().mt_2().child(about)),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .gap_6()
                    .p_6()
                    .when(can_create, |main| {
                        main.child(
                            div().flex().child(
                                Button::new("add-item")
                                    .label("Add item")
                                    .disabled(self.loading)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.begin_create_item(window, cx)
                                    })),
                            ),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(main_element),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;

    use super::{season_number, sidebar_width};

    #[test]
    fn season_numbers_are_whole_non_negative_numbers() {
        assert_eq!(season_number("0"), Some(0));
        assert_eq!(season_number(" 12 "), Some(12));
        assert_eq!(season_number("-1"), None);
        assert_eq!(season_number("1.5"), None);
        assert_eq!(season_number(""), None);
    }

    #[test]
    fn sidebar_becomes_a_rail_below_1024() {
        assert_eq!(sidebar_width(px(960.0)), px(64.0));
        assert_eq!(sidebar_width(px(1023.0)), px(64.0));
        assert_eq!(sidebar_width(px(1024.0)), px(240.0));
        assert_eq!(sidebar_width(px(1280.0)), px(240.0));
    }
}
