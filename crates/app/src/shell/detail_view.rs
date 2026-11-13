use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Selectable, Sizable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    Animation, AnimationExt, AnyElement, Context, FontWeight, Role, TestSupportExt, div,
    ease_out_quint, px, rgb,
};
use loregalia_core::{items, media::MediaKind, providers::Source};

use super::Shell;
use super::tags_view::TagTarget;
use super::widgets::{Cover, TILE_COVER_WIDTH};
use crate::theme::{DISPLAY_FONT, Palette};

pub(super) const DETAIL_WIDTH: f32 = 360.0;
pub(super) const DOCKED_FROM: f32 = 1280.0;
const DRAWER_SLIDE: f32 = 24.0;
const DRAWER_MILLISECONDS: u64 = 180;

impl Shell {
    pub(super) fn open_detail(&mut self, id: String, cx: &mut Context<Self>) {
        if self.detail.as_deref() != Some(id.as_str()) {
            self.close_inline_panels();
        }
        self.detail = Some(id);
        cx.notify();
    }

    pub(super) fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.close_inline_panels();
        self.detail = None;
        cx.notify();
    }

    pub(super) fn detail_item(&self) -> Option<&items::Item> {
        let id = self.detail.as_deref()?;
        self.items.iter().find(|item| item.id == id)
    }

    pub(super) fn detail_element(
        &self,
        item: &items::Item,
        docked: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panel =
            div()
                .id("detail-panel")
                .role(Role::Region)
                .test_support()
                .aria_label(format!("Details for {}", item.title))
                .w(px(DETAIL_WIDTH))
                .h_full()
                .flex_none()
                .flex()
                .flex_col()
                .bg(rgb(palette.surface))
                .border_l_1()
                .border_color(rgb(palette.line))
                .child(
                    div().flex().justify_end().px_4().pt_3().child(
                        Button::new("detail-close")
                            .ghost()
                            .icon(IconName::Close)
                            .tooltip("Close details")
                            .accessibility_label("Close details")
                            .on_click(cx.listener(|this, _, _, cx| this.close_detail(cx))),
                    ),
                )
                .child(
                    div()
                        .id("detail-scroll")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .px_6()
                        .pb_6()
                        .flex()
                        .flex_col()
                        .gap_5()
                        .child(Self::detail_hero(item, palette))
                        .child(self.detail_status(item, palette, cx))
                        .when(item.media_kind == MediaKind::Show, |body| {
                            body.child(self.seasons_section(item, palette, cx))
                        })
                        .child(self.detail_tags(item, palette, cx))
                        .child(self.detail_lists(item, palette))
                        .child(self.detail_actions(item, cx))
                        .children(item.source.as_deref().and_then(Source::from_key).map(
                            |source| {
                                div()
                                    .text_xs()
                                    .text_color(rgb(palette.muted))
                                    .child(format!("Details from {}", source.label()))
                            },
                        )),
                );

        if docked {
            return panel.into_any_element();
        }

        div()
            .id("detail-drawer")
            .absolute()
            .top_0()
            .right_0()
            .h_full()
            .shadow_lg()
            .child(panel)
            .with_animation(
                format!("detail-drawer-{}", item.id),
                Animation::new(Duration::from_millis(DRAWER_MILLISECONDS))
                    .with_easing(ease_out_quint()),
                |drawer, progress| {
                    drawer
                        .opacity(progress)
                        .right(px(-DRAWER_SLIDE * (1.0 - progress)))
                },
            )
            .into_any_element()
    }

    fn detail_hero(item: &items::Item, palette: &Palette) -> AnyElement {
        div()
            .flex()
            .items_end()
            .gap_4()
            .child(Self::cover_element(
                Cover {
                    prefix: "detail-cover",
                    item,
                    width: TILE_COVER_WIDTH,
                },
                palette,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .min_w_0()
                    .child(
                        div()
                            .id("detail-title")
                            .test_support()
                            .aria_label(item.title.clone())
                            .font_family(DISPLAY_FONT)
                            .text_size(px(24.0))
                            .line_height(px(28.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(palette.muted))
                            .child(Self::item_metadata(item)),
                    ),
            )
            .into_any_element()
    }

    fn detail_status(
        &self,
        item: &items::Item,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let item_id = item.id.clone();

        div()
            .id(format!("status-picker-{}", item.id))
            .role(Role::RadioGroup)
            .test_support()
            .aria_label("Status")
            .flex()
            .gap_1()
            .p_1()
            .rounded(px(10.0))
            .bg(rgb(palette.sunken))
            .children(
                self.statuses
                    .iter()
                    .filter(|status| status.media_kind == item.media_kind)
                    .map(|status| {
                        let item_id = item_id.clone();
                        let status_id = status.id.clone();
                        let current = status.id == item.status_id;
                        let color = if current {
                            Self::category_color(palette, status.category)
                        } else {
                            palette.muted
                        };

                        Button::new(format!("item-status-{}-{}", item.id, status.id))
                            .ghost()
                            .flex_1()
                            .h(px(52.0))
                            .px_1()
                            .selected(current)
                            .accessibility_label(status.label.clone())
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        Icon::new(Self::category_icon(status.category))
                                            .with_size(px(18.0))
                                            .text_color(rgb(color)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(rgb(if current {
                                                palette.content
                                            } else {
                                                palette.muted
                                            }))
                                            .child(status.label.clone()),
                                    ),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_status(item_id.clone(), status_id.clone(), cx)
                            }))
                    }),
            )
            .into_any_element()
    }

    fn detail_tags(
        &self,
        item: &items::Item,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tag_picker = match &self.tag_editor {
            Some(editor) if matches!(&editor.target, TagTarget::Item(id) if id == &item.id) => {
                Some(self.tag_picker_element(editor, cx))
            }
            _ => None,
        };
        let target = TagTarget::Item(item.id.clone());

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Self::section_label("Tags", palette))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .children(
                        self.tags_for(&self.item_tags, &item.id)
                            .into_iter()
                            .map(|tag| {
                                Self::tag_chip(
                                    format!("detail-tag-{}-{}", item.id, tag.id),
                                    tag,
                                    palette,
                                )
                            }),
                    )
                    .child(
                        Button::new(format!("item-tags-{}", item.id))
                            .ghost()
                            .small()
                            .icon(IconName::Tag)
                            .label("Edit tags")
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.toggle_tag_editor(target.clone(), window, cx)
                            })),
                    ),
            )
            .children(tag_picker)
            .into_any_element()
    }

    fn detail_lists(&self, item: &items::Item, palette: &Palette) -> AnyElement {
        let lists: Vec<_> = self
            .lists
            .iter()
            .filter(|list| {
                self.memberships.iter().any(|membership| {
                    membership.list_id == list.id && membership.item_id == item.id
                })
            })
            .collect();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(Self::section_label("In lists", palette))
            .child(if lists.is_empty() {
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted))
                    .child("Not in any list yet")
                    .into_any_element()
            } else {
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(lists.into_iter().map(|list| {
                        div()
                            .id(format!("detail-list-{}-{}", item.id, list.id))
                            .test_support()
                            .aria_label(list.name.clone())
                            .h(px(26.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded(px(13.0))
                            .border_1()
                            .border_color(rgb(palette.line))
                            .text_xs()
                            .child(list.name.clone())
                    }))
                    .into_any_element()
            })
            .into_any_element()
    }

    fn detail_actions(&self, item: &items::Item, cx: &mut Context<Self>) -> AnyElement {
        let edit_id = item.id.clone();
        let delete_id = item.id.clone();
        let remove = self.selected.clone().filter(|list_id| {
            self.memberships
                .iter()
                .any(|membership| &membership.list_id == list_id && membership.item_id == item.id)
        });

        div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(
                Button::new(format!("item-edit-{}", item.id))
                    .icon(IconName::Pencil)
                    .label("Edit")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_edit_item(edit_id.clone(), window, cx)
                    })),
            )
            .children(remove.map(|list_id| {
                let item_id = item.id.clone();
                Button::new(format!("remove-from-list-{}", item.id))
                    .label("Remove from list")
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_from_list(list_id.clone(), item_id.clone(), cx)
                    }))
            }))
            .child(
                Button::new(format!("item-delete-{}", item.id))
                    .ghost()
                    .danger()
                    .icon(IconName::Trash)
                    .label("Delete")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_delete_item(delete_id.clone(), window, cx)
                    })),
            )
            .into_any_element()
    }
}
