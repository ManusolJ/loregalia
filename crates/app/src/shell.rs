use std::future::Future;
use std::path::PathBuf;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    Disableable, IconName, Selectable, button::Button, button::ButtonVariants,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, Pixels, Role, TestSupportExt,
    Window, div, px, rgb,
};
use loregalia_core::{Database, Result as CoreResult, items, lists, log, media::MediaKind};

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
    error: Option<String>,
    loading: bool,
    selected: Option<String>,
    editor: Option<Editor>,
    confirm_delete: Option<String>,
    item_editor: Option<ItemEditor>,
    confirm_delete_item: Option<String>,
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
}

fn load(database: &Database) -> CoreResult<Loaded> {
    Ok(Loaded {
        lists: lists::all(database)?,
        items: items::all(database)?,
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
            error: None,
            loading,
            selected: None,
            editor: None,
            confirm_delete: None,
            item_editor: None,
            confirm_delete_item: None,
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
        self.editor = Some(Editor::Create(input));
        cx.notify();
    }

    fn select_list(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.editor = None;
        self.confirm_delete = None;
        self.item_editor = None;
        self.confirm_delete_item = None;
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

        self.selected = None;
        self.editor = None;
        self.confirm_delete = None;
        self.confirm_delete_item = None;
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

        self.run(cx, async move {
            let database = Database::open(path)?;
            match id {
                Some(id) => {
                    items::update(&database, &id, kind, &title, year)?;
                }
                None => {
                    items::create(&database, kind, &title, year)?;
                }
            }
            load(&database)
        });
    }

    fn cancel_item(&mut self, cx: &mut Context<Self>) {
        self.item_editor = None;
        cx.notify();
    }

    fn begin_delete_item(&mut self, id: String, cx: &mut Context<Self>) {
        self.confirm_delete_item = Some(id);
        self.item_editor = None;
        self.selected = None;
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
                        }
                        this.lists = loaded.lists;
                        this.items = loaded.items;
                        this.error = None;
                    }
                    Err(error) => {
                        let message = format!("Could not update lists: {error}");
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
                } else if let Some(id) = &self.selected {
                    match self.lists.iter().find(|list| &list.id == id) {
                        Some(list) => self.list_panel(&list.name, cx),
                        None => Self::empty_element(palette),
                    }
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
                } else if self.items.is_empty() {
                    Self::empty_element(palette)
                } else {
                    self.items_element(palette, cx)
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

    fn list_panel(&self, name: &str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("list-panel")
            .role(Role::Status)
            .test_support()
            .aria_label(name.to_owned())
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(name.to_owned()),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("rename-list").label("Rename").on_click(
                            cx.listener(|this, _, window, cx| this.begin_rename(window, cx)),
                        ),
                    )
                    .child(
                        Button::new("delete-list")
                            .label("Delete")
                            .danger()
                            .on_click(cx.listener(|this, _, _, cx| this.begin_delete(cx))),
                    ),
            )
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

    fn items_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("items")
            .role(Role::List)
            .test_support()
            .flex()
            .flex_col()
            .gap_3()
            .w_full()
            .max_w(px(640.0))
            .children(self.items.iter().map(|item| {
                let metadata = match item.release_year {
                    Some(year) => format!("{} · {year}", item.media_kind.label()),
                    None => item.media_kind.label().to_owned(),
                };
                let id = item.id.clone();

                div()
                    .id(format!("item-row-{}", item.id))
                    .role(Role::ListItem)
                    .test_support()
                    .aria_label(item.title.clone())
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .p_4()
                    .bg(rgb(palette.surface))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(palette.line))
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
                                Button::new(format!("item-delete-{}", item.id))
                                    .label("Delete")
                                    .danger()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.begin_delete_item(id.clone(), cx)
                                    })),
                            ),
                    )
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
                let row = div()
                    .id(format!("list-row-{}", list.id))
                    .role(Role::ListItem)
                    .test_support()
                    .aria_label(list.name.clone());
                let id = list.id.clone();

                if shows_labels {
                    row.px_3()
                        .py_2()
                        .cursor_pointer()
                        .child(list.name.clone())
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.select_list(id.clone(), cx)),
                        )
                } else {
                    row.child(
                        Button::new(format!("list-button-{}", list.id))
                            .icon(IconName::Inbox)
                            .tooltip(list.name.clone())
                            .accessibility_label(list.name.clone())
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.select_list(id.clone(), cx)),
                            ),
                    )
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
