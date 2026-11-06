use std::future::Future;
use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    Disableable, Icon, Selectable, Sizable, button::Button, button::ButtonVariants,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, Pixels, Role, TestSupportExt,
    Window, div, px, rgb,
};
use loregalia_core::{
    Database, Result as CoreResult, items, list_items, lists, log, media::MediaKind, statuses,
};

use crate::startup::StorageState;
use crate::theme::Palette;

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

pub struct Shell {
    storage: StorageState,
    log_path: Option<PathBuf>,
    lists: Vec<lists::List>,
    items: Vec<items::Item>,
    statuses: Vec<statuses::Status>,
    memberships: Vec<list_items::Membership>,
    error: Option<String>,
    loading: bool,
    selected: Option<String>,
    editor: Option<Editor>,
    confirm_delete: Option<String>,
    item_editor: Option<ItemEditor>,
    confirm_delete_item: Option<String>,
    status_picker: Option<String>,
    add_picker: Option<String>,
}

struct ItemEditor {
    id: Option<String>,
    kind: MediaKind,
    title: Entity<InputState>,
    year: Entity<InputState>,
}

struct Loaded {
    lists: Vec<lists::List>,
    items: Vec<items::Item>,
    statuses: Vec<statuses::Status>,
    memberships: Vec<list_items::Membership>,
}

fn load(database: &Database) -> CoreResult<Loaded> {
    Ok(Loaded {
        lists: lists::all(database)?,
        items: items::all(database)?,
        statuses: statuses::all(database)?,
        memberships: list_items::memberships(database)?,
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
    pub fn new(storage: StorageState, log_path: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let loading = matches!(&storage, StorageState::Ready(_));
        let mut shell = Self {
            storage,
            log_path,
            lists: Vec::new(),
            items: Vec::new(),
            statuses: Vec::new(),
            memberships: Vec::new(),
            error: None,
            loading,
            selected: None,
            editor: None,
            confirm_delete: None,
            item_editor: None,
            confirm_delete_item: None,
            status_picker: None,
            add_picker: None,
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

        self.selected = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.editor = Some(Editor::Create(input));
        cx.notify();
    }

    fn select_list(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.editor = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
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

        self.editor = None;
        self.confirm_delete = None;
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        self.item_editor = Some(ItemEditor {
            id: None,
            kind: MediaKind::Movie,
            title,
            year,
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
        });
        self.confirm_delete_item = None;
        self.status_picker = None;
        self.add_picker = None;
        cx.notify();
    }

    fn select_item_kind(&mut self, kind: MediaKind, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.item_editor {
            editor.kind = kind;
            cx.notify();
        }
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
        let list_id = self.selected.clone();

        self.run(cx, async move {
            let database = Database::open(path)?;
            match id {
                Some(id) => {
                    items::update(&database, &id, kind, &title, year)?;
                }
                None => {
                    let created = items::create(&database, kind, &title, year)?;
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
        cx.notify();
    }

    fn begin_delete_item(&mut self, id: String, cx: &mut Context<Self>) {
        self.confirm_delete_item = Some(id);
        self.item_editor = None;
        self.selected = None;
        self.status_picker = None;
        self.add_picker = None;
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
                        }
                        this.lists = loaded.lists;
                        this.items = loaded.items;
                        this.statuses = loaded.statuses;
                        this.memberships = loaded.memberships;
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
                if self.editor.is_some() {
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
        let members = self.items_in_list(&list.id);
        let adding = self.add_picker.as_deref() == Some(list.id.as_str());
        let list_id = list.id.clone();

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
        let heading = if editor.id.is_some() {
            "Edit item"
        } else {
            "New item"
        };

        div()
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
                    .on_click(cx.listener(move |this, _, _, cx| this.select_item_kind(kind, cx)))
            })))
            .child(
                Input::new(&editor.year)
                    .id("item-year-input")
                    .aria_label("Release year"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("save-item")
                            .label("Save")
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

    fn status_chip(
        &self,
        item: &items::Item,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(status) = self
            .statuses
            .iter()
            .find(|status| status.id == item.status_id)
        else {
            return div().into_any_element();
        };
        let color = rgb(Self::category_color(palette, status.category));
        let item_id = item.id.clone();

        Button::new(format!("item-status-{}", item.id))
            .ghost()
            .h(px(24.0))
            .px_2()
            .rounded(px(6.0))
            .bg(color.alpha(0.15))
            .accessibility_label(status.label.clone())
            .on_click(
                cx.listener(move |this, _, _, cx| this.toggle_status_picker(item_id.clone(), cx)),
            )
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
            .into_any_element()
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

    fn item_row(
        &self,
        item: &items::Item,
        remove_from: Option<&str>,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let metadata = match item.release_year {
            Some(year) => format!("{} · {year}", item.media_kind.label()),
            None => item.media_kind.label().to_owned(),
        };
        let id = item.id.clone();
        let chip = self.status_chip(item, palette, cx);
        let picker = (self.status_picker.as_deref() == Some(item.id.as_str()))
            .then(|| self.status_picker_element(item, cx));

        let actions = match remove_from {
            Some(list_id) => {
                let list_id = list_id.to_owned();

                div()
                    .flex()
                    .items_center()
                    .gap_2()
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
                            .flex_col()
                            .gap_1()
                            .child(item.title.clone())
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(palette.muted))
                                    .child(metadata),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(chip)
                            .child(actions),
                    ),
            )
            .children(picker)
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
            .children(
                items
                    .iter()
                    .map(|item| self.item_row(item, remove_from, palette, cx)),
            )
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
                    }),
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
