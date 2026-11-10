use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable, Selectable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, Role, TestSupportExt, Window, div,
};
use loregalia_core::{Database, tags};

use super::{Shell, load};
use crate::startup::StorageState;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum TagTarget {
    Item(String),
    List(String),
}

pub(super) struct TagEditor {
    pub(super) target: TagTarget,
    pub(super) input: Entity<InputState>,
}

pub(super) struct TagRename {
    pub(super) id: String,
    pub(super) input: Entity<InputState>,
}

impl Shell {
    pub(super) fn toggle_tag_editor(
        &mut self,
        target: TagTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let same = self
            .tag_editor
            .as_ref()
            .is_some_and(|editor| editor.target == target);
        self.close_inline_panels();
        if same {
            cx.notify();
            return;
        }

        let input = cx.new(|cx| InputState::new(window, cx).placeholder("New tag"));
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        Self::submit_on_enter(&input, window, cx, |this, window, cx| {
            this.save_tag(window, cx)
        });

        self.tag_editor = Some(TagEditor { target, input });
        cx.notify();
    }

    pub(super) fn save_tag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

        self.reload_after(cx, async move {
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

    pub(super) fn toggle_tag(&mut self, target: TagTarget, tag_id: String, cx: &mut Context<Self>) {
        let attached = self.tag_is_attached(&target, &tag_id);
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
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

    pub(super) fn begin_rename_tag(
        &mut self,
        tag: &tags::Tag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            let mut input = InputState::new(window, cx).default_value(tag.name.clone());
            input.select_all(window, cx);
            input
        });
        let focus = input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        Self::submit_on_enter(&input, window, cx, |this, _, cx| this.save_tag_rename(cx));

        self.confirm_delete_tag = None;
        self.renaming_tag = Some(TagRename {
            id: tag.id.clone(),
            input,
        });
        cx.notify();
    }

    pub(super) fn save_tag_rename(&mut self, cx: &mut Context<Self>) {
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

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            tags::rename(&database, &id, &name)?;
            load(&database)
        });
    }

    pub(super) fn cancel_tag_rename(&mut self, cx: &mut Context<Self>) {
        self.renaming_tag = None;
        cx.notify();
    }

    pub(super) fn begin_delete_tag(&mut self, tag_id: String, cx: &mut Context<Self>) {
        self.renaming_tag = None;
        self.confirm_delete_tag = Some(tag_id);
        cx.notify();
    }

    pub(super) fn cancel_delete_tag(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete_tag = None;
        cx.notify();
    }

    pub(super) fn delete_tag(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete_tag.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            tags::delete(&database, &id)?;
            load(&database)
        });
    }

    pub(super) fn tag_is_attached(&self, target: &TagTarget, tag_id: &str) -> bool {
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

    pub(super) fn close_tag_panels(&mut self) {
        self.renaming_tag = None;
        self.confirm_delete_tag = None;
    }

    pub(super) fn tags_button(target: TagTarget, id: String, cx: &mut Context<Self>) -> Button {
        Button::new(id)
            .icon(IconName::Tag)
            .tooltip("Tags")
            .accessibility_label("Tags")
            .on_click(cx.listener(move |this, _, window, cx| {
                this.toggle_tag_editor(target.clone(), window, cx)
            }))
    }

    pub(super) fn tag_row(
        &self,
        tag: &tags::Tag,
        target: &TagTarget,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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

    pub(super) fn tag_picker_element(
        &self,
        editor: &TagEditor,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
}
