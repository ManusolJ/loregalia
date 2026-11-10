mod about_view;
mod items_view;
mod lists_view;
mod metadata_view;
mod seasons_view;
mod settings_view;
mod tags_view;
mod widgets;

use std::future::Future;
use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{Disableable, Selectable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, FontWeight, Pixels, Role, TestSupportExt, Window, div,
    px, rgb,
};
use loregalia_core::{
    Database, Result as CoreResult, items, list_items, lists, log, seasons, settings, statuses,
    tags,
};

use crate::startup::StorageState;
use crate::theme::Palette;

use items_view::ItemEditor;
use lists_view::Editor;
pub use metadata_view::{MetadataDetails, MetadataSearch};
use metadata_view::{default_details, default_search};
use seasons_view::SeasonEditor;
use tags_view::{TagEditor, TagRename};

const RAIL_WIDTH: f32 = 64.0;

const SIDEBAR_WIDTH: f32 = 240.0;

const SIDEBAR_LABELS_FROM: f32 = 1024.0;

const EMPTY_MESSAGE: &str = "Nothing planned yet";

const LOADING_MESSAGE: &str = "Loading lists";

const LOADING_BLOCK_WIDTH: f32 = 320.0;

pub fn sidebar_width(window_width: Pixels) -> Pixels {
    if window_width < px(SIDEBAR_LABELS_FROM) {
        px(RAIL_WIDTH)
    } else {
        px(SIDEBAR_WIDTH)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Screen {
    Library,
    About,
    Settings,
}

pub struct Shell {
    storage: StorageState,
    log_path: Option<PathBuf>,
    metadata_search: MetadataSearch,
    metadata_details: MetadataDetails,
    refresh_started: bool,
    theme: settings::Theme,
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
    screen: Screen,
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

impl Shell {
    pub fn new(
        storage: StorageState,
        log_path: Option<PathBuf>,
        theme: settings::Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_services(
            storage,
            log_path,
            theme,
            default_search(),
            default_details(),
            cx,
        )
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
            default_details(),
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
            storage,
            log_path,
            metadata_search,
            metadata_details,
            refresh_started: false,
            theme,
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
            screen: Screen::Library,
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
        shell.reload(cx);
        shell
    }

    fn show(&mut self, screen: Screen) {
        self.screen = screen;
        self.selected = None;
        self.close_editors();
    }

    fn show_all_items(&mut self, cx: &mut Context<Self>) {
        self.show(Screen::Library);
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
        self.status_picker = None;
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
                .items_center()
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
        let all_items = Button::new("all-items")
            .ghost()
            .accessibility_label("All items")
            .selected(self.screen == Screen::Library && self.selected.is_none())
            .on_click(cx.listener(|this, _, _, cx| this.show_all_items(cx)));
        let all_items = if shows_labels {
            all_items
                .w_full()
                .h(px(36.0))
                .px_3()
                .justify_start()
                .child(div().w_full().child("All items"))
        } else {
            all_items.icon(IconName::Library).tooltip("All items")
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
                    .child(div().mb_3().child(all_items))
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

    use super::sidebar_width;

    #[test]
    fn sidebar_becomes_a_rail_below_1024() {
        assert_eq!(sidebar_width(px(960.0)), px(64.0));
        assert_eq!(sidebar_width(px(1023.0)), px(64.0));
        assert_eq!(sidebar_width(px(1024.0)), px(240.0));
        assert_eq!(sidebar_width(px(1280.0)), px(240.0));
    }
}
