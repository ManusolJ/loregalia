use gpui_kit::prelude::*;
use gpui_kit::{Context, FontWeight, Pixels, Role, TestSupportExt, Window, div, px, rgb};

use crate::theme::Palette;

const RAIL_WIDTH: f32 = 56.0;
const SIDEBAR_WIDTH: f32 = 240.0;
const SIDEBAR_LABELS_FROM: f32 = 1024.0;
const EMPTY_MESSAGE: &str = "Nothing planned yet";

pub fn sidebar_width(window_width: Pixels) -> Pixels {
    if window_width < px(SIDEBAR_LABELS_FROM) {
        px(RAIL_WIDTH)
    } else {
        px(SIDEBAR_WIDTH)
    }
}

pub struct Shell;

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *cx.global::<Palette>();
        let width = sidebar_width(window.viewport_size().width);
        let shows_labels = width > px(RAIL_WIDTH);

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
                    .p_3()
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
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .id("empty-state")
                            .role(Role::Status)
                            .test_support()
                            .aria_label(EMPTY_MESSAGE)
                            .text_color(rgb(palette.muted))
                            .child(EMPTY_MESSAGE),
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
        assert_eq!(sidebar_width(px(960.0)), px(56.0));
        assert_eq!(sidebar_width(px(1023.0)), px(56.0));
        assert_eq!(sidebar_width(px(1024.0)), px(240.0));
        assert_eq!(sidebar_width(px(1280.0)), px(240.0));
    }
}
