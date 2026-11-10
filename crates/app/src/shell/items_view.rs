use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable, Selectable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, Role, TestSupportExt, Window,
    div, px, rgb,
};
use loregalia_core::{Database, items, list_items, media::MediaKind, providers};

use super::metadata_view::SearchStatus;
use super::tags_view::TagTarget;
use super::widgets::Confirmation;
use super::{Screen, Shell, load};
use crate::startup::StorageState;
use crate::theme::Palette;

const DELETE_ITEM: Confirmation = Confirmation {
    id: "item-delete-confirmation",
    cancel_id: "cancel-item-delete",
    confirm_id: "confirm-item-delete",
};

pub(super) struct ItemEditor {
    pub(super) id: Option<String>,
    pub(super) kind: MediaKind,
    pub(super) title: Entity<InputState>,
    pub(super) year: Entity<InputState>,
    pub(super) search_input: Option<Entity<InputState>>,
    pub(super) search: SearchStatus,
    pub(super) search_request: u64,
    pub(super) lookup: Option<items::Lookup>,
}

pub(super) struct ListItemActions<'a> {
    pub(super) list_id: &'a str,
    pub(super) manual: bool,
    pub(super) can_move_up: bool,
    pub(super) can_move_down: bool,
}

impl Shell {
    pub(super) fn begin_create_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = cx.new(|cx| InputState::new(window, cx).placeholder("Title"));
        let year = cx.new(|cx| InputState::new(window, cx).placeholder("Year"));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search by title"));
        let focus = title.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        for input in [&title, &year] {
            Self::submit_on_enter(input, window, cx, |this, _, cx| this.save_item(cx));
        }
        Self::submit_on_enter(&search_input, window, cx, |this, _, cx| {
            this.search_metadata(cx)
        });

        self.screen = Screen::Library;
        self.close_editors();
        self.item_editor = Some(ItemEditor {
            id: None,
            kind: MediaKind::Movie,
            title,
            year,
            search_input: Some(search_input),
            search: SearchStatus::Idle,
            search_request: 0,
            lookup: None,
        });
        cx.notify();
    }

    pub(super) fn begin_edit_item(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        for input in [&title, &year] {
            Self::submit_on_enter(input, window, cx, |this, _, cx| this.save_item(cx));
        }

        self.close_editors();
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
        cx.notify();
    }

    pub(super) fn select_item_kind(&mut self, kind: MediaKind, cx: &mut Context<Self>) {
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

        cx.notify();
    }

    pub(super) fn save_item(&mut self, cx: &mut Context<Self>) {
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

        self.reload_after(cx, async move {
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

    pub(super) fn cancel_item(&mut self, cx: &mut Context<Self>) {
        self.close_editors();
        cx.notify();
    }

    pub(super) fn begin_delete_item(&mut self, id: String, cx: &mut Context<Self>) {
        self.close_editors();
        self.selected = None;
        self.confirm_delete_item = Some(id);
        cx.notify();
    }

    pub(super) fn cancel_delete_item(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete_item = None;
        cx.notify();
    }

    pub(super) fn delete_item(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete_item.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            items::delete(&database, &id)?;
            load(&database)
        });
    }

    pub(super) fn toggle_status_picker(&mut self, id: String, cx: &mut Context<Self>) {
        let open = self.status_picker.as_deref() != Some(id.as_str());
        self.close_inline_panels();
        if open {
            self.status_picker = Some(id);
        }
        cx.notify();
    }

    pub(super) fn change_status(
        &mut self,
        item_id: String,
        status_id: String,
        cx: &mut Context<Self>,
    ) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.status_picker = None;

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            items::set_status(&database, &item_id, &status_id)?;
            load(&database)
        });
    }

    pub(super) fn item_editor_element(
        &self,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
                    .on_click(cx.listener(move |this, _, _, cx| this.select_item_kind(kind, cx)))
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

    pub(super) fn status_picker_element(
        &self,
        item: &items::Item,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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

    pub(super) fn item_row(
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

        let mut actions = div().flex().items_center().gap_2();
        if let Some(list_actions) = &list_actions
            && list_actions.manual
        {
            let list_id = list_actions.list_id.to_owned();
            let up_list_id = list_id.clone();
            let up_id = id.clone();
            let down_id = id.clone();
            actions = actions
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
                            this.move_item_down(list_id.clone(), down_id.clone(), cx)
                        })),
                );
        }
        actions = actions
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
            );
        let actions = match list_actions {
            Some(list_actions) => {
                let list_id = list_actions.list_id.to_owned();
                actions.child(
                    Button::new(format!("remove-from-list-{}", item.id))
                        .label("Remove")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.remove_from_list(list_id.clone(), id.clone(), cx)
                        })),
                )
            }
            None => actions.child(
                Button::new(format!("item-delete-{}", item.id))
                    .label("Delete")
                    .danger()
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.begin_delete_item(id.clone(), cx)),
                    ),
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
                                        self.tags_for(&self.item_tags, &item.id).into_iter().map(
                                            |tag| {
                                                Self::tag_chip(
                                                    format!("item-tag-{}-{}", item.id, tag.id),
                                                    tag,
                                                    palette,
                                                )
                                            },
                                        ),
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

    pub(super) fn items_element(
        &self,
        items: &[&items::Item],
        list_id: Option<&str>,
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
                let list_actions = list_id.map(|list_id| ListItemActions {
                    list_id,
                    manual: self.sort == list_items::Sort::Manual,
                    can_move_up: index > 0,
                    can_move_down: index + 1 < items.len(),
                });

                self.item_row(item, list_actions, palette, cx)
            }))
            .into_any_element()
    }

    pub(super) fn delete_item_element(&self, id: &str, cx: &mut Context<Self>) -> AnyElement {
        let title = self
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.title.as_str())
            .unwrap_or("this item");

        Self::confirm_element(
            DELETE_ITEM,
            format!("Delete \"{title}\"?"),
            Self::cancel_delete_item,
            Self::delete_item,
            cx,
        )
    }
}
