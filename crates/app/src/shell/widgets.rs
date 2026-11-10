use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, FontWeight, ObjectFit, Role, TestSupportExt, div, img, px, rgb,
};
use loregalia_core::{items, media::MediaKind, statuses, tags};

use super::Shell;
use crate::theme::Palette;

pub(super) struct Confirmation {
    pub(super) id: &'static str,
    pub(super) cancel_id: &'static str,
    pub(super) confirm_id: &'static str,
}

impl Shell {
    pub(super) fn confirm_element(
        confirmation: Confirmation,
        message: String,
        cancel: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        confirm: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(confirmation.id)
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
                        Button::new(confirmation.cancel_id)
                            .label("Cancel")
                            .on_click(cx.listener(move |this, _, _, cx| cancel(this, cx))),
                    )
                    .child(
                        Button::new(confirmation.confirm_id)
                            .label("Delete")
                            .danger()
                            .on_click(cx.listener(move |this, _, _, cx| confirm(this, cx))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn tags_for<'a>(
        &'a self,
        links: &'a [tags::Tagging],
        owner_id: &str,
    ) -> Vec<&'a tags::Tag> {
        let mut attached: Vec<&tags::Tag> = links
            .iter()
            .filter(|link| link.owner_id == owner_id)
            .filter_map(|link| self.tags.iter().find(|tag| tag.id == link.tag_id))
            .collect();
        attached.sort_by(|left, right| left.name.cmp(&right.name));
        attached
    }

    pub(super) fn category_color(palette: &Palette, category: statuses::Category) -> u32 {
        match category {
            statuses::Category::Planned => palette.status_planned,
            statuses::Category::Active => palette.status_active,
            statuses::Category::Paused => palette.status_paused,
            statuses::Category::Done => palette.status_done,
            statuses::Category::Dropped => palette.status_dropped,
        }
    }

    pub(super) fn category_icon(category: statuses::Category) -> IconName {
        match category {
            statuses::Category::Planned => IconName::CircleDashed,
            statuses::Category::Active => IconName::CirclePlay,
            statuses::Category::Paused => IconName::CirclePause,
            statuses::Category::Done => IconName::CircleCheck,
            statuses::Category::Dropped => IconName::CircleX,
        }
    }

    pub(super) fn status_chip(status: &statuses::Status, id: String, palette: &Palette) -> Button {
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

    pub(super) fn tag_chip(id: String, tag: &tags::Tag, palette: &Palette) -> AnyElement {
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

    pub(super) fn media_kind_icon(kind: MediaKind) -> IconName {
        match kind {
            MediaKind::Movie => IconName::Film,
            MediaKind::Show => IconName::Tv,
            MediaKind::Book => IconName::BookOpen,
            MediaKind::Game => IconName::Gamepad2,
        }
    }

    pub(super) fn cover_element(item: &items::Item, palette: &Palette) -> AnyElement {
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
}
