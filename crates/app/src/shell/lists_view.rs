use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable, Selectable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, Role, TestSupportExt, Window,
    div, px, rgb,
};
use loregalia_core::{Database, items, list_items, lists};

use super::tags_view::TagTarget;
use super::widgets::Confirmation;
use super::{Screen, Shell, load};
use crate::startup::StorageState;
use crate::theme::Palette;

const DELETE_LIST: Confirmation = Confirmation {
    id: "delete-confirmation",
    cancel_id: "cancel-delete",
    confirm_id: "confirm-delete",
};

pub(super) enum Editor {
    Create(Entity<InputState>),
    Rename {
        id: String,
        input: Entity<InputState>,
    },
}

impl Shell {
    pub(super) fn begin_create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| InputState::new(window, cx));
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        Self::submit_on_enter(&input, window, cx, |this, _, cx| this.save_editor(cx));

        self.show(Screen::Library);
        self.editor = Some(Editor::Create(input));
        cx.notify();
    }

    pub(super) fn set_sort(&mut self, sort: list_items::Sort, cx: &mut Context<Self>) {
        self.sort = sort;
        cx.notify();
    }

    pub(super) fn select_list(&mut self, id: String, cx: &mut Context<Self>) {
        self.show(Screen::Library);
        self.sort = list_items::Sort::Manual;
        self.kind_filter = None;
        self.selected = Some(id);
        cx.notify();
    }

    pub(super) fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        Self::submit_on_enter(&input, window, cx, |this, _, cx| this.save_editor(cx));

        self.close_inline_panels();
        self.editor = Some(Editor::Rename { id, input });
        cx.notify();
    }

    pub(super) fn save_editor(&mut self, cx: &mut Context<Self>) {
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

        self.reload_after(cx, async move {
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

    pub(super) fn cancel_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        cx.notify();
    }

    pub(super) fn begin_delete(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };

        self.close_inline_panels();
        self.confirm_delete = Some(id);
        cx.notify();
    }

    pub(super) fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete = None;
        cx.notify();
    }

    pub(super) fn delete_selected(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.selected = None;

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            lists::delete(&database, &id)?;
            load(&database)
        });
    }

    pub(super) fn toggle_add_picker(&mut self, list_id: String, cx: &mut Context<Self>) {
        let open = self.add_picker.as_deref() != Some(list_id.as_str());
        self.close_inline_panels();
        if open {
            self.add_picker = Some(list_id);
        }
        cx.notify();
    }

    pub(super) fn add_to_list(&mut self, list_id: String, item_id: String, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.close_inline_panels();

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            list_items::add(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    pub(super) fn remove_from_list(
        &mut self,
        list_id: String,
        item_id: String,
        cx: &mut Context<Self>,
    ) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            list_items::remove(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    pub(super) fn move_item_up(
        &mut self,
        list_id: String,
        item_id: String,
        cx: &mut Context<Self>,
    ) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            list_items::move_up(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    pub(super) fn move_item_down(
        &mut self,
        list_id: String,
        item_id: String,
        cx: &mut Context<Self>,
    ) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            list_items::move_down(&database, &list_id, &item_id)?;
            load(&database)
        });
    }

    pub(super) fn editor_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(editor) = &self.editor else {
            return Self::empty_element(self.shelf, palette);
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

    pub(super) fn items_in_list(&self, list_id: &str) -> Vec<&items::Item> {
        self.memberships
            .iter()
            .filter(|membership| membership.list_id == list_id)
            .filter_map(|membership| self.items.iter().find(|item| item.id == membership.item_id))
            .collect()
    }

    pub(super) fn list_empty_element(palette: &Palette) -> AnyElement {
        div()
            .id("list-empty")
            .role(Role::Status)
            .test_support()
            .aria_label("No items in this list yet")
            .text_color(rgb(palette.muted))
            .child("No items in this list yet")
            .into_any_element()
    }

    pub(super) fn add_picker_element(
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

    pub(super) fn list_actions(list: &lists::List, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(Self::tags_button(
                TagTarget::List(list.id.clone()),
                "list-tags".to_owned(),
                cx,
            ))
            .child(
                Button::new("rename-list")
                    .ghost()
                    .icon(IconName::Pencil)
                    .label("Rename")
                    .on_click(cx.listener(|this, _, window, cx| this.begin_rename(window, cx))),
            )
            .child(
                Button::new("delete-list")
                    .ghost()
                    .danger()
                    .icon(IconName::Trash)
                    .label("Delete")
                    .on_click(cx.listener(|this, _, _, cx| this.begin_delete(cx))),
            )
            .into_any_element()
    }

    pub(super) fn list_toolbar(
        &self,
        list: &lists::List,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let list_id = list.id.clone();

        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("add-existing-item")
                    .ghost()
                    .icon(IconName::Plus)
                    .label("Add existing item")
                    .selected(self.add_picker.as_deref() == Some(list.id.as_str()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_add_picker(list_id.clone(), cx)
                    })),
            )
            .child(div().flex_1())
            .child(div().text_xs().text_color(rgb(palette.muted)).child("Sort"))
            .child(Self::segmented(
                list_items::Sort::ALL.map(|sort| {
                    Button::new(format!("sort-{}", sort.key()))
                        .ghost()
                        .label(sort.label())
                        .selected(self.sort == sort)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_sort(sort, cx)))
                }),
                palette,
            ))
            .into_any_element()
    }

    pub(super) fn list_panel(
        &self,
        list: &lists::List,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut members = self.items_in_list(&list.id);
        list_items::sort_items(&mut members, self.sort);
        let adding = self.add_picker.as_deref() == Some(list.id.as_str());
        let tag_picker = match &self.tag_editor {
            Some(editor) if matches!(&editor.target, TagTarget::List(id) if id == &list.id) => {
                Some(self.tag_picker_element(editor, cx))
            }
            _ => None,
        };
        let tags = self.tags_for(&self.list_tags, &list.id);

        div()
            .id("list-panel")
            .role(Role::Region)
            .test_support()
            .aria_label(list.name.clone())
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .when(!tags.is_empty(), |panel| {
                panel.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_2()
                        .children(tags.into_iter().map(|tag| {
                            Self::tag_chip(format!("list-tag-{}-{}", list.id, tag.id), tag, palette)
                        })),
                )
            })
            .when(adding, |panel| {
                panel.child(self.add_picker_element(&list.id, palette, cx))
            })
            .children(tag_picker)
            .child(if members.is_empty() {
                Self::list_empty_element(palette)
            } else {
                self.items_element(&members, Some(&list.id), palette, cx)
            })
            .into_any_element()
    }

    pub(super) fn delete_list_element(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let name = self
            .lists
            .iter()
            .find(|list| list.id == id)
            .map(|list| list.name.as_str())
            .unwrap_or("this list");

        Self::confirm_element(
            DELETE_LIST,
            format!("Delete \"{name}\"?"),
            Self::cancel_delete,
            Self::delete_selected,
            cx,
        )
    }
}
