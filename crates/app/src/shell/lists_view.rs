use std::collections::HashSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    Disableable, Icon, Selectable, Sizable, button::Button, button::ButtonVariants,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, Role, TestSupportExt, Window, div, px, rgb,
};
use loregalia_core::{Database, items, library, list_items, lists};

use super::tags_view::TagTarget;
use super::widgets::{Confirmation, Cover, EmptyPanel, FORM_WIDTH, ROW_COVER_WIDTH};
use super::{Screen, Shell, load};
use crate::startup::StorageState;
use crate::theme::Palette;

const DELETE_LIST: Confirmation = Confirmation {
    id: "delete-confirmation",
    cancel_id: "cancel-delete",
    confirm_id: "confirm-delete",
    consequence: "The items in it stay in your library. This cannot be undone.",
};

const ADD_PICKER_LIMIT: usize = 50;
const NO_LISTS: &str = "Create a list in the sidebar first";

pub(super) enum Editor {
    Create(Entity<InputState>),
    Rename {
        id: String,
        input: Entity<InputState>,
    },
}

pub(super) struct AddPicker {
    list_id: String,
    input: Entity<InputState>,
    text: String,
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
        self.viewport.to_top();
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

    pub(super) fn begin_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };

        self.close_inline_panels();
        self.confirm_delete = Some(id);
        self.focus_handle.focus(window, cx);
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

    fn adding_to(&self, list_id: &str) -> Option<&AddPicker> {
        self.add_picker
            .as_ref()
            .filter(|picker| picker.list_id == list_id)
    }

    pub(super) fn toggle_add_picker(
        &mut self,
        list_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let open = self.adding_to(&list_id).is_none();
        self.close_inline_panels();
        if open {
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("Find an item to add"));
            let focus = input.read(cx).focus_handle(cx);
            focus.focus(window, cx);
            Self::on_text_change(&input, cx, |this, text| {
                if let Some(picker) = &mut this.add_picker {
                    picker.text = text;
                }
            });
            self.add_picker = Some(AddPicker {
                list_id,
                input,
                text: String::new(),
            });
        }
        cx.notify();
    }

    pub(super) fn toggle_list_picker(&mut self, item_id: String, cx: &mut Context<Self>) {
        let open = self.list_picker.as_deref() != Some(item_id.as_str());
        self.close_inline_panels();
        if open {
            self.list_picker = Some(item_id);
        }
        cx.notify();
    }

    fn holds(&self, list_id: &str, item_id: &str) -> bool {
        self.memberships
            .iter()
            .any(|membership| membership.list_id == list_id && membership.item_id == item_id)
    }

    pub(super) fn list_picker_element(
        &self,
        item: &items::Item,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.lists.is_empty() {
            return div()
                .id("list-picker-empty")
                .role(Role::Status)
                .test_support()
                .aria_label(NO_LISTS)
                .text_xs()
                .text_color(rgb(palette.muted))
                .child(NO_LISTS)
                .into_any_element();
        }

        div()
            .id("list-picker")
            .test_support()
            .flex()
            .flex_col()
            .gap_1()
            .children(self.lists.iter().map(|list| {
                let held = self.holds(&list.id, &item.id);
                let list_id = list.id.clone();
                let item_id = item.id.clone();

                Button::new(format!("list-toggle-{}", list.id))
                    .w_full()
                    .justify_start()
                    .accessibility_label(list.name.clone())
                    .selected(held)
                    .toggled(held)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let (list_id, item_id) = (list_id.clone(), item_id.clone());
                        if held {
                            this.remove_from_list(list_id, item_id, cx)
                        } else {
                            this.add_to_list(list_id, item_id, cx)
                        }
                    }))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(div().min_w_0().truncate().child(list.name.clone()))
                            .when(held, |row| {
                                row.child(
                                    Icon::new(IconName::Check)
                                        .with_size(px(16.0))
                                        .text_color(rgb(palette.accent)),
                                )
                            }),
                    )
            }))
            .into_any_element()
    }

    pub(super) fn add_to_list(&mut self, list_id: String, item_id: String, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

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
            return self.empty_element(palette, cx);
        };
        let (title, input) = match editor {
            Editor::Create(input) => ("New list", input),
            Editor::Rename { input, .. } => ("Rename list", input),
        };
        let can_save = !input.read(cx).value().trim().is_empty();

        div()
            .id("list-editor")
            .test_support()
            .w_full()
            .max_w(px(FORM_WIDTH))
            .flex()
            .flex_col()
            .gap_6()
            .child(Self::form_heading(title))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(Self::section_label("Name", palette))
                    .child(
                        Input::new(input)
                            .id("list-name-input")
                            .aria_label("List name"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("save-list")
                            .primary()
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
            .filter_map(|membership| self.item(&membership.item_id))
            .collect()
    }

    pub(super) fn visible_members(&self, list_id: &str) -> Vec<&items::Item> {
        let mut members = self.items_in_list(list_id);
        self.find.filter.keep(&mut members, &self.item_tags);
        list_items::sort_items(&mut members, self.sort);
        members
    }

    pub(super) fn list_empty_element(palette: &Palette) -> AnyElement {
        Self::empty_panel(
            EmptyPanel {
                id: "list-empty",
                icon: IconName::Inbox,
                message: "No items in this list yet".to_owned(),
                hint: "Add something you already have, or search for something new.",
            },
            None,
            palette,
        )
    }

    fn add_picker_element(
        &self,
        picker: &AddPicker,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let held: HashSet<&str> = self
            .memberships
            .iter()
            .filter(|membership| membership.list_id == picker.list_id)
            .map(|membership| membership.item_id.as_str())
            .collect();
        let available: Vec<&items::Item> = self
            .items
            .iter()
            .filter(|item| !held.contains(item.id.as_str()))
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

        let mut matches: Vec<&items::Item> = available
            .into_iter()
            .filter(|item| library::matches_text(&item.title, &picker.text))
            .collect();
        list_items::sort_items(&mut matches, list_items::Sort::Title);
        let hidden = matches.len().saturating_sub(ADD_PICKER_LIMIT);
        let none = matches.is_empty();

        div()
            .id("add-picker")
            .test_support()
            .pt_1()
            .px_1()
            .flex()
            .flex_col()
            .gap_2()
            .max_w(px(FORM_WIDTH))
            .child(
                Input::new(&picker.input)
                    .id("add-picker-input")
                    .aria_label("Find an item to add"),
            )
            .child(
                div()
                    .id("add-picker-results")
                    .max_h(px(300.0))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .p_1()
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(palette.line))
                    .bg(rgb(palette.surface))
                    .when(none, |results| {
                        results.child(
                            div()
                                .id("add-picker-none")
                                .role(Role::Status)
                                .test_support()
                                .aria_label("No matches")
                                .p_2()
                                .text_xs()
                                .text_color(rgb(palette.muted))
                                .child("No matches"),
                        )
                    })
                    .children(matches.into_iter().take(ADD_PICKER_LIMIT).map(|item| {
                        let list_id = picker.list_id.clone();
                        let item_id = item.id.clone();

                        Button::new(format!("add-to-list-{}", item.id))
                            .ghost()
                            .accessibility_label(item.title.clone())
                            .w_full()
                            .h(px(72.0))
                            .flex_none()
                            .px_2()
                            .justify_start()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.add_to_list(list_id.clone(), item_id.clone(), cx)
                            }))
                            .child(Self::picker_row(item, palette))
                    }))
                    .when(hidden > 0, |results| {
                        results.child(
                            div()
                                .p_2()
                                .text_xs()
                                .text_color(rgb(palette.muted))
                                .child(format!("{hidden} more. Keep typing to narrow them down.")),
                        )
                    }),
            )
            .into_any_element()
    }

    fn picker_row(item: &items::Item, palette: &Palette) -> AnyElement {
        div()
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .child(Self::cover_element(
                Cover::of("picker-cover", item, ROW_COVER_WIDTH),
                palette,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_0p5()
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_color(rgb(palette.content))
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(Self::item_metadata(item)),
                    ),
            )
            .child(
                Icon::new(IconName::Plus)
                    .with_size(px(16.0))
                    .text_color(rgb(palette.muted)),
            )
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
                    .tooltip("Rename list")
                    .accessibility_label("Rename list")
                    .on_click(cx.listener(|this, _, window, cx| this.begin_rename(window, cx))),
            )
            .child(
                Button::new("delete-list")
                    .ghost()
                    .icon(IconName::Trash)
                    .tooltip("Delete list")
                    .accessibility_label("Delete list")
                    .on_click(cx.listener(|this, _, window, cx| this.begin_delete(window, cx))),
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
                    .selected(self.adding_to(&list.id).is_some())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.toggle_add_picker(list_id.clone(), window, cx)
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
                        .toggled(self.sort == sort)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_sort(sort, cx)))
                }),
                palette,
            ))
            .child(self.filter_button(false, cx))
            .child(self.layout_switch(palette, cx))
            .into_any_element()
    }

    pub(super) fn list_panel(
        &self,
        list: &lists::List,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let members = self.visible_members(&list.id);
        let adding = self
            .adding_to(&list.id)
            .map(|picker| self.add_picker_element(picker, palette, cx));
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
            .children(adding)
            .children(tag_picker)
            .child(if members.is_empty() && self.find.filter.is_active() {
                Self::no_matches_element(palette, cx)
            } else if members.is_empty() {
                Self::list_empty_element(palette)
            } else {
                self.items_element(&members, Some(&list.id), palette, cx)
            })
            .into_any_element()
    }

    pub(super) fn delete_list_element(
        &self,
        id: &str,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
            palette,
            cx,
        )
    }
}
