use gpui_kit::component::link::Link;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, FontWeight, Role, TestSupportExt, div, px, rgb};

use super::{Screen, Shell};
use crate::startup::StorageState;
use crate::theme::Palette;

impl Shell {
    pub(super) fn begin_about(&mut self, cx: &mut Context<Self>) {
        self.show(Screen::About);
        cx.notify();
    }

    pub(super) fn about_element(&self, palette: &Palette) -> AnyElement {
        let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
        let data_path = match &self.storage {
            StorageState::Ready(path) => path.display().to_string(),
            StorageState::Failed(_) => String::new(),
        };

        div()
            .id("about-screen")
            .role(Role::Region)
            .test_support()
            .aria_label("About Loregalia")
            .flex()
            .flex_col()
            .gap_6()
            .w_full()
            .h_full()
            .max_w(px(640.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_2xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("About Loregalia"),
                    )
                    .child(
                        div()
                            .id("about-version")
                            .test_support()
                            .aria_label(version.clone())
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(version),
                    )
                    .child(
                        div()
                            .max_w(px(520.0))
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child(
                                "A local-first planner for what to watch, read and play next. \
                                 Your data stays on this computer; there is no account and no server.",
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Metadata"),
                    )
                    .child(
                        div()
                            .id("about-tvmaze")
                            .test_support()
                            .aria_label("TVmaze, used under CC BY-SA 4.0")
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child("Show metadata from")
                            .child(
                                div()
                                    .id("tvmaze-link")
                                    .test_support()
                                    .child(
                                        Link::new("tvmaze-anchor")
                                            .href("https://www.tvmaze.com/api")
                                            .child("TVmaze"),
                                    ),
                            )
                            .child(", used under")
                            .child(
                                Link::new("cc-by-sa-link")
                                    .href("https://creativecommons.org/licenses/by-sa/4.0/")
                                    .child("CC BY-SA 4.0"),
                            )
                            .child("."),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child("More sources will be credited here as they are added."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("License"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child("Loregalia is released under the")
                            .child(
                                Link::new("license-link")
                                    .href("https://github.com/ManusolJ/loregalia/blob/main/LICENSE")
                                    .child("MIT License"),
                            )
                            .child("."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Data"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(palette.muted))
                            .child(data_path),
                    ),
            )
            .into_any_element()
    }
}
