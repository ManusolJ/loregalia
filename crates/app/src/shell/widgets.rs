use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, FontWeight, ObjectFit, Role, TestSupportExt, div, img, px, rgb,
};
use loregalia_core::{items, media::MediaKind, statuses, tags};

use super::Shell;
use crate::theme::{DISPLAY_FONT, Palette, SELECTED_ALPHA};

pub(super) const ROW_COVER_WIDTH: f32 = 40.0;
pub(super) const TILE_COVER_WIDTH: f32 = 112.0;
pub(super) const GRID_COVER_WIDTH: f32 = 144.0;
pub(super) const FORM_WIDTH: f32 = 640.0;

pub(super) struct Confirmation {
    pub(super) id: &'static str,
    pub(super) cancel_id: &'static str,
    pub(super) confirm_id: &'static str,
    pub(super) consequence: &'static str,
}

pub(super) struct EmptyPanel {
    pub(super) id: &'static str,
    pub(super) icon: IconName,
    pub(super) message: String,
    pub(super) hint: &'static str,
}

pub(super) struct Cover<'a> {
    pub(super) prefix: &'static str,
    pub(super) key: String,
    pub(super) title: &'a str,
    pub(super) kind: MediaKind,
    pub(super) url: Option<&'a str>,
    pub(super) width: f32,
}

impl<'a> Cover<'a> {
    pub(super) fn of(prefix: &'static str, item: &'a items::Item, width: f32) -> Self {
        Self {
            prefix,
            key: item.id.clone(),
            title: &item.title,
            kind: item.media_kind,
            url: item.cover_url.as_deref(),
            width,
        }
    }
}

impl Shell {
    pub(super) fn confirm_element(
        confirmation: Confirmation,
        message: String,
        cancel: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        confirm: impl Fn(&mut Self, &mut Context<Self>) + 'static,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(confirmation.id)
            .role(Role::Alert)
            .test_support()
            .aria_label(message.clone())
            .max_w(px(FORM_WIDTH))
            .p_6()
            .flex()
            .flex_col()
            .items_start()
            .gap_4()
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.line))
            .bg(rgb(palette.surface))
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(message),
            )
            .child(
                div()
                    .text_color(rgb(palette.muted))
                    .child(confirmation.consequence),
            )
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

    pub(super) fn section_label(label: &'static str, palette: &Palette) -> AnyElement {
        div()
            .text_xs()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(palette.muted))
            .child(label)
            .into_any_element()
    }

    pub(super) fn form_heading(title: &'static str) -> AnyElement {
        div()
            .id("form-heading")
            .role(Role::Heading)
            .test_support()
            .aria_label(title)
            .font_family(DISPLAY_FONT)
            .text_size(px(28.0))
            .line_height(px(34.0))
            .font_weight(FontWeight::SEMIBOLD)
            .child(title)
            .into_any_element()
    }

    pub(super) fn empty_panel(
        empty: EmptyPanel,
        action: Option<Button>,
        palette: &Palette,
    ) -> AnyElement {
        div()
            .id(empty.id)
            .role(Role::Status)
            .test_support()
            .aria_label(empty.message.clone())
            .w_full()
            .py(px(64.0))
            .px_6()
            .flex()
            .flex_col()
            .items_center()
            .gap_2()
            .child(
                div()
                    .size(px(56.0))
                    .mb_2()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(rgb(palette.surface))
                    .border_1()
                    .border_color(rgb(palette.line))
                    .child(
                        Icon::new(empty.icon)
                            .with_size(px(24.0))
                            .text_color(rgb(palette.muted)),
                    ),
            )
            .child(
                div()
                    .text_base()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(empty.message),
            )
            .child(
                div()
                    .max_w(px(420.0))
                    .text_center()
                    .text_color(rgb(palette.muted))
                    .child(empty.hint),
            )
            .children(action.map(|action| div().pt_4().child(action)))
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

    pub(super) fn status_of(&self, status_id: &str) -> Option<&statuses::Status> {
        self.statuses.iter().find(|status| status.id == status_id)
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

    fn status_content(status: &statuses::Status, palette: &Palette) -> gpui_kit::Div {
        let color = rgb(Self::category_color(palette, status.category));

        div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                Icon::new(Self::category_icon(status.category))
                    .with_size(px(14.0))
                    .text_color(color),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(palette.content))
                    .child(status.label.clone()),
            )
    }

    pub(super) fn status_chip(status: &statuses::Status, id: String, palette: &Palette) -> Button {
        let color = rgb(Self::category_color(palette, status.category));

        Button::new(id)
            .ghost()
            .h(px(24.0))
            .px_2()
            .rounded(px(6.0))
            .bg(color.alpha(SELECTED_ALPHA))
            .accessibility_label(status.label.clone())
            .child(Self::status_content(status, palette))
    }

    pub(super) fn status_badge(
        status: &statuses::Status,
        id: String,
        palette: &Palette,
    ) -> AnyElement {
        let color = rgb(Self::category_color(palette, status.category));

        div()
            .id(id)
            .test_support()
            .aria_label(status.label.clone())
            .flex_none()
            .h(px(24.0))
            .px_2()
            .flex()
            .items_center()
            .rounded(px(6.0))
            .bg(color.alpha(SELECTED_ALPHA))
            .child(Self::status_content(status, palette))
            .into_any_element()
    }

    pub(super) fn tag_chip(id: String, tag: &tags::Tag, palette: &Palette) -> AnyElement {
        div()
            .id(id)
            .test_support()
            .aria_label(tag.name.clone())
            .h(px(26.0))
            .px_3()
            .flex()
            .items_center()
            .rounded(px(13.0))
            .border_1()
            .border_color(rgb(palette.line))
            .text_xs()
            .text_color(rgb(palette.content))
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

    pub(super) fn cover_element(cover: Cover<'_>, palette: &Palette) -> AnyElement {
        let Cover {
            prefix,
            key,
            title,
            kind,
            url,
            width,
        } = cover;
        let icon_size = if width >= TILE_COVER_WIDTH {
            28.0
        } else {
            16.0
        };
        let mut element = div()
            .id(format!("{prefix}-{key}"))
            .role(Role::Image)
            .test_support()
            .aria_label(format!("Cover for {title}"))
            .relative()
            .w(px(width))
            .h(px(width * 1.5))
            .flex_none()
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(palette.line))
            .overflow_hidden()
            .bg(rgb(palette.sunken))
            .child(
                div()
                    .id(format!("{prefix}-fallback-{key}"))
                    .test_support()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        Icon::new(Self::media_kind_icon(kind))
                            .with_size(px(icon_size))
                            .text_color(rgb(palette.muted)),
                    ),
            );

        if let Some(url) = url {
            element = element.child(
                img(url.to_owned())
                    .id(format!("{prefix}-image-{key}"))
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .test_support(),
            );
        }

        element.into_any_element()
    }

    pub(super) fn item_metadata(item: &items::Item) -> String {
        match item.release_year {
            Some(year) => format!("{} · {year}", item.media_kind.label()),
            None => item.media_kind.label().to_owned(),
        }
    }
}
