use gpui_kit::component::link::Link;
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, FontWeight, Role, TestSupportExt, div, px, rgb};

use super::{Screen, Shell};
use crate::startup::StorageState;
use crate::theme::Palette;

pub(super) const PRIVACY_NOTES: [&str; 4] = [
    "Your lists, items, tags and statuses stay in a file on this computer. Loregalia has no \
     account and sends none of it anywhere.",
    "Show and book searches go straight from this computer to TVmaze and Open Library, and covers \
     load from their image servers, so those services see your IP address.",
    "Movie and game searches go through the Loregalia server, which holds the API keys. It keeps \
     search results for up to a day so repeated searches are fast, with nothing that says who \
     searched, and it holds your IP address in memory only to limit how often one computer can \
     search. It keeps no log of requests.",
    "If you enter your own TMDB or IGDB keys in Settings, those searches skip the Loregalia server \
     and go straight to TMDB and IGDB.",
];

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
                                 Your library stays on this computer, and there is no account.",
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
                            .id("about-tmdb")
                            .test_support()
                            .aria_label(
                                "This product uses the TMDB API but is not endorsed or certified by TMDB",
                            )
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child("Movie metadata from")
                            .child(
                                div()
                                    .id("tmdb-link")
                                    .test_support()
                                    .child(
                                        Link::new("tmdb-anchor")
                                            .href("https://www.themoviedb.org/")
                                            .child("TMDB"),
                                    ),
                            )
                            .child(
                                ". This product uses the TMDB API but is not endorsed or certified by TMDB.",
                            ),
                    )
                    .child(
                        div()
                            .id("about-openlibrary")
                            .test_support()
                            .aria_label("Book metadata from Open Library")
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child("Book metadata and covers from")
                            .child(
                                div()
                                    .id("openlibrary-link")
                                    .test_support()
                                    .child(
                                        Link::new("openlibrary-anchor")
                                            .href("https://openlibrary.org/")
                                            .child("Open Library"),
                                    ),
                            )
                            .child("."),
                    )
                    .child(
                        div()
                            .id("about-igdb")
                            .test_support()
                            .aria_label("Game metadata from IGDB")
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_1()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child("Game metadata and covers from")
                            .child(
                                div()
                                    .id("igdb-link")
                                    .test_support()
                                    .child(
                                        Link::new("igdb-anchor")
                                            .href("https://www.igdb.com/")
                                            .child("IGDB"),
                                    ),
                            )
                            .child("."),
                    ),
            )
            .child(
                div()
                    .id("about-privacy")
                    .role(Role::Region)
                    .test_support()
                    .aria_label("Privacy")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .max_w(px(560.0))
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Privacy"),
                    )
                    .children(PRIVACY_NOTES.map(|note| {
                        div()
                            .text_base()
                            .text_color(rgb(palette.content))
                            .child(note)
                    })),
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
