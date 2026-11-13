mod about_view;
mod detail_view;
mod items_view;
mod lists_view;
mod metadata_view;
mod seasons_view;
mod settings_view;
mod tags_view;
mod widgets;

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{
    Disableable, Selectable, Sizable, button::Button, button::ButtonVariants,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, FontWeight, KeyBinding, Pixels,
    Role, TestSupportExt, Window, actions, div, px, rgb,
};
use loregalia_core::library::{self, Shelf};
use loregalia_core::{
    Database, Result as CoreResult, items, keys, list_items, lists, log, media::MediaKind, seasons,
    settings, statuses, tags,
};

use crate::startup::StorageState;
use crate::theme::{DISPLAY_FONT, Palette};

use detail_view::DOCKED_FROM;
use items_view::ItemEditor;
use lists_view::Editor;
pub use metadata_view::{MetadataDetails, MetadataSearch};
use metadata_view::{default_details, default_search};
use seasons_view::SeasonEditor;
use settings_view::KeyInputs;
use tags_view::{TagEditor, TagRename};

const RAIL_WIDTH: f32 = 64.0;
const SIDEBAR_WIDTH: f32 = 240.0;
const SIDEBAR_LABELS_FROM: f32 = 1024.0;
const NAV_HEIGHT: f32 = 34.0;
const LOADING_MESSAGE: &str = "Loading lists";
const LOADING_BLOCK_WIDTH: f32 = 320.0;
const CONTENT_WIDTH: f32 = 760.0;

pub fn sidebar_width(window_width: Pixels) -> Pixels {
    if window_width < px(SIDEBAR_LABELS_FROM) {
        px(RAIL_WIDTH)
    } else {
        px(SIDEBAR_WIDTH)
    }
}

pub fn detail_docks(window_width: Pixels) -> bool {
    window_width >= px(DOCKED_FROM)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Screen {
    Library,
    About,
    Settings,
}

actions!(shell, [Dismiss, Activate]);

const SHELL_CONTEXT: &str = "Shell";
const ITEM_ROW_CONTEXT: &str = "ItemRow";

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Dismiss, Some(SHELL_CONTEXT)),
        KeyBinding::new("enter", Activate, Some(ITEM_ROW_CONTEXT)),
        KeyBinding::new("space", Activate, Some(ITEM_ROW_CONTEXT)),
    ]);
}

pub struct Shell {
    focus_handle: FocusHandle,
    row_focus: HashMap<String, FocusHandle>,
    storage: StorageState,
    log_path: Option<PathBuf>,
    metadata_search: MetadataSearch,
    metadata_details: MetadataDetails,
    refresh_started: bool,
    theme: settings::Theme,
    settings_error: Option<String>,
    api_keys: keys::ApiKeys,
    key_inputs: Option<KeyInputs>,
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
    screen: Screen,
    shelf: Shelf,
    kind_filter: Option<MediaKind>,
    sort: list_items::Sort,
    selected: Option<String>,
    detail: Option<String>,
    editor: Option<Editor>,
    confirm_delete: Option<String>,
    item_editor: Option<ItemEditor>,
    confirm_delete_item: Option<String>,
    add_picker: Option<String>,
    tag_editor: Option<TagEditor>,
    renaming_tag: Option<TagRename>,
    confirm_delete_tag: Option<String>,
    season_editor: Option<SeasonEditor>,
    season_status_picker: Option<String>,
    confirm_delete_season: Option<String>,
}

struct Snapshot {
    lists: Vec<lists::List>,
    items: Vec<items::Item>,
    statuses: Vec<statuses::Status>,
    memberships: Vec<list_items::Membership>,
    tags: Vec<tags::Tag>,
    item_tags: Vec<tags::Tagging>,
    list_tags: Vec<tags::Tagging>,
    seasons: Vec<seasons::Season>,
}

fn load(database: &Database) -> CoreResult<Snapshot> {
    Ok(Snapshot {
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

fn load_keys(storage: &StorageState) -> keys::ApiKeys {
    let StorageState::Ready(path) = storage else {
        return keys::ApiKeys::default();
    };
    let Some(path) = keys::file_for_database(path) else {
        return keys::ApiKeys::default();
    };
    keys::ApiKeys::load(&path).unwrap_or_default()
}

struct NavRow {
    id: String,
    icon: IconName,
    label: String,
    count: Option<usize>,
    selected: bool,
}

impl Shell {
    pub fn new(
        storage: StorageState,
        log_path: Option<PathBuf>,
        theme: settings::Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        let api_keys = load_keys(&storage);
        let mut shell = Self::new_with_services(
            storage,
            log_path,
            theme,
            default_search(api_keys.clone()),
            default_details(api_keys.clone()),
            cx,
        );
        shell.api_keys = api_keys;
        shell
    }

    pub fn new_with_search(
        storage: StorageState,
        log_path: Option<PathBuf>,
        theme: settings::Theme,
        metadata_search: MetadataSearch,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_services(
            storage,
            log_path,
            theme,
            metadata_search,
            default_details(keys::ApiKeys::default()),
            cx,
        )
    }

    pub fn new_with_services(
        storage: StorageState,
        log_path: Option<PathBuf>,
        theme: settings::Theme,
        metadata_search: MetadataSearch,
        metadata_details: MetadataDetails,
        cx: &mut Context<Self>,
    ) -> Self {
        let loading = matches!(&storage, StorageState::Ready(_));
        let mut shell = Self {
            focus_handle: cx.focus_handle(),
            row_focus: HashMap::new(),
            storage,
            log_path,
            metadata_search,
            metadata_details,
            refresh_started: false,
            theme,
            settings_error: None,
            api_keys: keys::ApiKeys::default(),
            key_inputs: None,
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
            screen: Screen::Library,
            shelf: Shelf::All,
            kind_filter: None,
            sort: list_items::Sort::Manual,
            selected: None,
            detail: None,
            editor: None,
            confirm_delete: None,
            item_editor: None,
            confirm_delete_item: None,
            add_picker: None,
            tag_editor: None,
            renaming_tag: None,
            confirm_delete_tag: None,
            season_editor: None,
            season_status_picker: None,
            confirm_delete_season: None,
        };
        shell.reload(cx);
        shell
    }

    fn show(&mut self, screen: Screen) {
        self.screen = screen;
        self.selected = None;
        self.detail = None;
        self.close_editors();
    }

    fn has_inline_panel(&self) -> bool {
        self.add_picker.is_some()
            || self.tag_editor.is_some()
            || self.renaming_tag.is_some()
            || self.confirm_delete_tag.is_some()
            || self.season_editor.is_some()
            || self.season_status_picker.is_some()
            || self.confirm_delete_season.is_some()
    }

    fn has_editor(&self) -> bool {
        self.editor.is_some()
            || self.confirm_delete.is_some()
            || self.item_editor.is_some()
            || self.confirm_delete_item.is_some()
    }

    fn dismiss(&mut self, _: &Dismiss, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_inline_panel() {
            self.close_inline_panels();
            self.focus_handle.focus(window, cx);
        } else if self.has_editor() {
            self.close_editors();
            self.focus_handle.focus(window, cx);
        } else if self.detail.is_some() {
            self.detail = None;
        } else {
            cx.propagate();
            return;
        }
        cx.notify();
    }

    fn sync_row_focus(&mut self, cx: &mut Context<Self>) {
        let items = &self.items;
        self.row_focus
            .retain(|id, _| items.iter().any(|item| &item.id == id));
        for item in &self.items {
            if !self.row_focus.contains_key(&item.id) {
                self.row_focus.insert(
                    item.id.clone(),
                    cx.focus_handle().tab_index(0).tab_stop(true),
                );
            }
        }
    }

    fn show_shelf(&mut self, shelf: Shelf, cx: &mut Context<Self>) {
        self.show(Screen::Library);
        self.shelf = shelf;
        cx.notify();
    }

    fn set_kind_filter(&mut self, kind: Option<MediaKind>, cx: &mut Context<Self>) {
        self.kind_filter = kind;
        cx.notify();
    }

    fn close_editors(&mut self) {
        self.editor = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
        self.close_inline_panels();
    }

    fn close_inline_panels(&mut self) {
        self.add_picker = None;
        self.tag_editor = None;
        self.close_tag_panels();
        self.close_season_panels();
    }

    fn submit_on_enter(
        input: &Entity<InputState>,
        window: &Window,
        cx: &mut Context<Self>,
        submit: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        cx.subscribe_in(
            input,
            window,
            move |this, _, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    submit(this, window, cx);
                }
            },
        )
        .detach();
    }

    fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        cx.notify();
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

    fn reload(&mut self, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            load(&database)
        });
    }

    fn reload_after<F>(&mut self, cx: &mut Context<Self>, work: F)
    where
        F: Future<Output = CoreResult<Snapshot>> + Send + 'static,
    {
        self.loading = true;
        let task = cx.background_spawn(work);

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(snapshot) => {
                        if let Some(id) = this.selected.clone()
                            && !snapshot.lists.iter().any(|list| list.id == id)
                        {
                            this.selected = None;
                            this.close_editors();
                        }
                        if let Some(id) = this.detail.clone()
                            && !snapshot.items.iter().any(|item| item.id == id)
                        {
                            this.detail = None;
                        }
                        this.lists = snapshot.lists;
                        this.items = snapshot.items;
                        this.statuses = snapshot.statuses;
                        this.memberships = snapshot.memberships;
                        this.tags = snapshot.tags;
                        this.item_tags = snapshot.item_tags;
                        this.list_tags = snapshot.list_tags;
                        this.seasons = snapshot.seasons;
                        this.error = None;
                        this.start_metadata_refresh(cx);
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

    fn selected_list(&self) -> Option<&lists::List> {
        let id = self.selected.as_deref()?;
        self.lists.iter().find(|list| list.id == id)
    }

    fn shelved(&self, shelf: Shelf) -> Vec<&items::Item> {
        library::shelve(&self.items, &self.statuses, shelf, self.kind_filter)
    }

    fn shelf_count(&self, shelf: Shelf) -> usize {
        library::shelve(&self.items, &self.statuses, shelf, None).len()
    }

    fn list_count(&self, list_id: &str) -> usize {
        self.memberships
            .iter()
            .filter(|membership| membership.list_id == list_id)
            .count()
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
                .flex()
                .flex_col()
                .items_start()
                .gap_3()
                .text_color(rgb(palette.danger))
                .text_xs()
                .child(message.clone())
                .child(
                    Button::new("dismiss-error")
                        .label("Dismiss")
                        .on_click(cx.listener(|this, _, _, cx| this.dismiss_error(cx))),
                )
                .into_any_element(),
            (StorageState::Ready(_), None) => match self.screen {
                Screen::About => self.about_element(palette),
                Screen::Settings => self.settings_element(palette, cx),
                Screen::Library => self.library_element(palette, cx),
            },
        }
    }

    fn library_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.editor.is_some() {
            self.editor_element(palette, cx)
        } else if let Some(id) = &self.confirm_delete {
            self.delete_list_element(id, cx)
        } else if self.item_editor.is_some() {
            self.item_editor_element(palette, cx)
        } else if let Some(id) = &self.confirm_delete_item {
            self.delete_item_element(id, cx)
        } else if self.selected.is_some() {
            match self.selected_list() {
                Some(list) => self.list_panel(list, palette, cx),
                None => Self::empty_element(self.shelf, palette),
            }
        } else {
            self.shelf_element(palette, cx)
        }
    }

    fn shelf_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let shelved = self.shelved(self.shelf);
        if shelved.is_empty() {
            return Self::empty_element(self.shelf, palette);
        }
        let up_next = if self.shelf == Shelf::All {
            self.shelved(Shelf::UpNext)
        } else {
            Vec::new()
        };

        div()
            .flex()
            .flex_col()
            .gap_6()
            .when(!up_next.is_empty(), |content| {
                content.child(self.up_next_element(&up_next, palette, cx))
            })
            .child(self.items_element(&shelved, None, palette, cx))
            .into_any_element()
    }

    fn empty_element(shelf: Shelf, palette: &Palette) -> AnyElement {
        let message = shelf.empty_message();

        div()
            .id("empty-state")
            .role(Role::Status)
            .test_support()
            .aria_label(message)
            .text_color(rgb(palette.muted))
            .child(message)
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

    fn segmented(buttons: impl IntoIterator<Item = Button>, palette: &Palette) -> AnyElement {
        div()
            .flex()
            .gap_0p5()
            .p_0p5()
            .rounded(px(8.0))
            .bg(rgb(palette.sunken))
            .children(buttons)
            .into_any_element()
    }

    fn header_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let list = self.selected_list();
        let (title, count) = match list {
            Some(list) => (list.name.clone(), self.list_count(&list.id)),
            None => (
                self.shelf.label().to_owned(),
                self.shelved(self.shelf).len(),
            ),
        };

        let title_row = div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .id("page-title")
                    .role(Role::Heading)
                    .test_support()
                    .aria_label(title.clone())
                    .min_w_0()
                    .truncate()
                    .font_family(DISPLAY_FONT)
                    .text_size(px(28.0))
                    .line_height(px(34.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title),
            )
            .child(
                div()
                    .id("page-count")
                    .test_support()
                    .aria_label(count.to_string())
                    .text_color(rgb(palette.muted))
                    .child(count.to_string()),
            )
            .child(div().flex_1())
            .children(list.map(|list| Self::list_actions(list, cx)))
            .child(
                Button::new("add-item")
                    .primary()
                    .icon(IconName::Plus)
                    .label("Add item")
                    .disabled(self.loading)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.begin_create_item(window, cx)),
                    ),
            );

        let toolbar = match list {
            Some(list) => self.list_toolbar(list, palette, cx),
            None => {
                let mut filters = vec![
                    Button::new("filter-all")
                        .ghost()
                        .label("All")
                        .selected(self.kind_filter.is_none())
                        .on_click(cx.listener(|this, _, _, cx| this.set_kind_filter(None, cx))),
                ];
                filters.extend(MediaKind::ALL.map(|kind| {
                    Button::new(format!("filter-{}", kind.key()))
                        .ghost()
                        .label(kind.plural_label())
                        .selected(self.kind_filter == Some(kind))
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.set_kind_filter(Some(kind), cx)),
                        )
                }));
                div()
                    .flex()
                    .child(Self::segmented(filters, palette))
                    .into_any_element()
            }
        };

        div()
            .id("page-header")
            .test_support()
            .flex()
            .flex_col()
            .gap_4()
            .px_8()
            .pt_6()
            .pb_4()
            .child(title_row)
            .child(toolbar)
            .into_any_element()
    }

    fn nav_row(
        row: NavRow,
        shows_labels: bool,
        palette: &Palette,
        on_click: impl Fn(&mut Self, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let button = Button::new(row.id.clone())
            .ghost()
            .role(Role::ListItem)
            .accessibility_label(row.label.clone())
            .selected(row.selected)
            .on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)));

        if !shows_labels {
            return button.icon(row.icon).tooltip(row.label).into_any_element();
        }

        button
            .w_full()
            .h(px(NAV_HEIGHT))
            .px_2()
            .justify_start()
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        gpui_kit::component::Icon::new(row.icon)
                            .with_size(px(18.0))
                            .text_color(rgb(if row.selected {
                                palette.accent
                            } else {
                                palette.muted
                            })),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(row.label))
                    .children(row.count.map(|count| {
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(count.to_string())
                    })),
            )
            .into_any_element()
    }

    fn group_label(label: &'static str, palette: &Palette) -> AnyElement {
        div()
            .px_2()
            .pb_1()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(palette.muted))
            .child(label)
            .into_any_element()
    }

    fn sidebar_element(
        &self,
        width: Pixels,
        shows_labels: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let can_create = matches!(&self.storage, StorageState::Ready(_));
        let in_library = self.screen == Screen::Library && self.selected.is_none();

        let shelves: Vec<AnyElement> = Shelf::ALL
            .into_iter()
            .map(|shelf| {
                let id = if shelf == Shelf::All {
                    "all-items".to_owned()
                } else {
                    format!("shelf-{}", shelf.key())
                };
                let icon = match shelf {
                    Shelf::All => IconName::Library,
                    Shelf::UpNext => IconName::ArrowRight,
                    Shelf::Finished => IconName::Check,
                };
                Self::nav_row(
                    NavRow {
                        id,
                        icon,
                        label: shelf.label().to_owned(),
                        count: Some(self.shelf_count(shelf)),
                        selected: in_library && self.shelf == shelf,
                    },
                    shows_labels,
                    palette,
                    move |this, _window, cx| this.show_shelf(shelf, cx),
                    cx,
                )
            })
            .collect();

        let list_rows: Vec<AnyElement> = self
            .lists
            .iter()
            .map(|list| {
                let id = list.id.clone();
                let row = Self::nav_row(
                    NavRow {
                        id: if shows_labels {
                            format!("list-row-{}", list.id)
                        } else {
                            format!("list-button-{}", list.id)
                        },
                        icon: IconName::Inbox,
                        label: list.name.clone(),
                        count: Some(self.list_count(&list.id)),
                        selected: self.selected.as_deref() == Some(list.id.as_str()),
                    },
                    shows_labels,
                    palette,
                    move |this, _window, cx| this.select_list(id.clone(), cx),
                    cx,
                );
                if shows_labels {
                    row
                } else {
                    div()
                        .id(format!("list-row-{}", list.id))
                        .role(Role::ListItem)
                        .test_support()
                        .aria_label(list.name.clone())
                        .child(row)
                        .into_any_element()
                }
            })
            .collect();

        let create = Button::new("create-list")
            .ghost()
            .icon(IconName::Plus)
            .tooltip("Create list")
            .accessibility_label("Create list")
            .disabled(self.loading)
            .on_click(cx.listener(|this, _, window, cx| this.begin_create(window, cx)));

        let footer: Vec<AnyElement> = vec![
            Self::nav_row(
                NavRow {
                    id: "settings-button".to_owned(),
                    icon: IconName::Settings,
                    label: "Settings".to_owned(),
                    count: None,
                    selected: self.screen == Screen::Settings,
                },
                shows_labels,
                palette,
                |this, window, cx| this.begin_settings(window, cx),
                cx,
            ),
            Self::nav_row(
                NavRow {
                    id: "about-button".to_owned(),
                    icon: IconName::Info,
                    label: "About".to_owned(),
                    count: None,
                    selected: self.screen == Screen::About,
                },
                shows_labels,
                palette,
                |this, _window, cx| this.begin_about(cx),
                cx,
            ),
        ];

        div()
            .id("sidebar")
            .test_support()
            .w(width)
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap_5()
            .px_3()
            .py_4()
            .when(!shows_labels, |sidebar| sidebar.items_center())
            .bg(rgb(palette.surface))
            .border_r_1()
            .border_color(rgb(palette.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .child(
                        div()
                            .size(px(22.0))
                            .flex_none()
                            .rounded(px(6.0))
                            .bg(rgb(palette.accent)),
                    )
                    .when(shows_labels, |brand| {
                        brand.child(
                            div()
                                .font_family(DISPLAY_FONT)
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_base()
                                .child("Loregalia"),
                        )
                    }),
            )
            .child(
                div()
                    .id("library-nav")
                    .role(Role::List)
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .when(shows_labels, |group| {
                        group.child(Self::group_label("Library", palette))
                    })
                    .children(shelves),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .when(shows_labels, |row| {
                                row.child(Self::group_label("Lists", palette))
                            })
                            .when(can_create, |row| row.child(create)),
                    )
                    .child(
                        div()
                            .id("list-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .id("lists")
                                    .role(Role::List)
                                    .test_support()
                                    .flex()
                                    .flex_col()
                                    .gap_0p5()
                                    .when(!shows_labels, |lists| lists.items_center())
                                    .children(list_rows),
                            ),
                    ),
            )
            .child(div().flex().flex_col().gap_0p5().children(footer))
            .into_any_element()
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_row_focus(cx);
        if window.focused(cx).is_none() {
            self.focus_handle.focus(window, cx);
        }
        let palette = *cx.global::<Palette>();
        let window_width = window.viewport_size().width;
        let width = sidebar_width(window_width);
        let shows_labels = width > px(RAIL_WIDTH);
        let docked = detail_docks(window_width);
        let show_header = matches!(&self.storage, StorageState::Ready(_))
            && self.screen == Screen::Library
            && self.error.is_none();
        let sidebar = self.sidebar_element(width, shows_labels, &palette, cx);
        let header = show_header.then(|| self.header_element(&palette, cx));
        let main_element = self.main_element(&palette, cx);
        let detail = (self.screen == Screen::Library)
            .then(|| self.detail_item())
            .flatten()
            .map(|item| self.detail_element(item, docked, &palette, cx));

        div()
            .id("shell")
            .key_context(SHELL_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::dismiss))
            .relative()
            .size_full()
            .flex()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.content))
            .text_sm()
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .children(header)
                    .child(
                        div()
                            .id("main-scroll")
                            .test_support()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px_8()
                            .pb_6()
                            .when(!show_header, |scroll| scroll.pt_8())
                            .child(div().w_full().max_w(px(CONTENT_WIDTH)).child(main_element)),
                    ),
            )
            .children(detail)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;

    use super::{detail_docks, sidebar_width};

    #[test]
    fn sidebar_becomes_a_rail_below_1024() {
        assert_eq!(sidebar_width(px(960.0)), px(64.0));
        assert_eq!(sidebar_width(px(1023.0)), px(64.0));
        assert_eq!(sidebar_width(px(1024.0)), px(240.0));
        assert_eq!(sidebar_width(px(1280.0)), px(240.0));
    }

    #[test]
    fn the_detail_panel_docks_from_1280() {
        assert!(!detail_docks(px(1279.0)));
        assert!(detail_docks(px(1280.0)));
    }
}
