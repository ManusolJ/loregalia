use std::sync::{Arc, LazyLock};

use gpui_kit::assets::IconName;
use gpui_kit::component::link::Link;
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, Context, FontWeight, Image, ImageFormat, ObjectFit, Role, TestSupportExt, div, img,
    px, rgb,
};
use loregalia_core::media::MediaKind;

use super::{Screen, Shell};
use crate::startup::StorageState;
use crate::theme::{DISPLAY_FONT, Palette};

const SOURCE_COLUMN: f32 = 112.0;
const TMDB_LOGO_HEIGHT: f32 = 12.0;
const TMDB_LOGO_RATIO: f32 = 273.42 / 35.52;
const TMDB_NOTICE: &str = "This product uses the TMDB API but is not endorsed or certified by TMDB";

static TMDB_LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("../../assets/tmdb-logo.svg").to_vec(),
    ))
});

pub(super) const PRIVACY_NOTES: [(IconName, &str); 3] = [
    (
        IconName::HardDrive,
        "Your library stays on this computer. There is no account, and nothing is uploaded.",
    ),
    (
        IconName::Globe,
        "Shows and books are searched directly at TVmaze and Open Library, which see your IP \
         address like any website.",
    ),
    (
        IconName::Server,
        "Movies and games go through the Loregalia server, which keeps no logs and nothing that \
         identifies you. Your own keys in Settings skip it.",
    ),
];

struct SourceRow {
    id: &'static str,
    kind: MediaKind,
    name: &'static str,
    link_id: &'static str,
    href: &'static str,
}

const SOURCES: [SourceRow; 4] = [
    SourceRow {
        id: "about-tvmaze",
        kind: MediaKind::Show,
        name: "TVmaze",
        link_id: "tvmaze-link",
        href: "https://www.tvmaze.com/api",
    },
    SourceRow {
        id: "about-tmdb",
        kind: MediaKind::Movie,
        name: "TMDB",
        link_id: "tmdb-link",
        href: "https://www.themoviedb.org/",
    },
    SourceRow {
        id: "about-openlibrary",
        kind: MediaKind::Book,
        name: "Open Library",
        link_id: "openlibrary-link",
        href: "https://openlibrary.org/",
    },
    SourceRow {
        id: "about-igdb",
        kind: MediaKind::Game,
        name: "IGDB",
        link_id: "igdb-link",
        href: "https://www.igdb.com/",
    },
];

impl Shell {
    pub(super) fn begin_about(&mut self, cx: &mut Context<Self>) {
        self.show(Screen::About);
        cx.notify();
    }

    fn about_card(
        id: &'static str,
        title: &'static str,
        palette: &Palette,
        body: Vec<AnyElement>,
    ) -> AnyElement {
        div()
            .id(id)
            .role(Role::Region)
            .test_support()
            .aria_label(title)
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded(px(8.0))
            .bg(rgb(palette.surface))
            .border_1()
            .border_color(rgb(palette.line))
            .child(Self::section_label(title, palette))
            .children(body)
            .into_any_element()
    }

    fn source_row(source: &SourceRow, palette: &Palette) -> AnyElement {
        let extra = match source.kind {
            MediaKind::Show => Some(
                Link::new("cc-by-sa-link")
                    .href("https://creativecommons.org/licenses/by-sa/4.0/")
                    .child("CC BY-SA 4.0")
                    .into_any_element(),
            ),
            MediaKind::Movie => Some(
                img(TMDB_LOGO.clone())
                    .h(px(TMDB_LOGO_HEIGHT))
                    .w(px(TMDB_LOGO_HEIGHT * TMDB_LOGO_RATIO))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element(),
            ),
            MediaKind::Book | MediaKind::Game => None,
        };

        div()
            .id(source.id)
            .test_support()
            .aria_label(match source.kind {
                MediaKind::Movie => TMDB_NOTICE.to_string(),
                _ => format!("{} from {}", source.kind.plural_label(), source.name),
            })
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(px(SOURCE_COLUMN))
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(rgb(palette.muted))
                    .child(
                        Icon::new(Self::media_kind_icon(source.kind))
                            .with_size(px(16.0))
                            .text_color(rgb(palette.muted)),
                    )
                    .child(source.kind.plural_label()),
            )
            .child(
                div()
                    .id(source.link_id)
                    .test_support()
                    .font_weight(FontWeight::MEDIUM)
                    .child(
                        Link::new(format!("{}-anchor", source.link_id))
                            .href(source.href)
                            .child(source.name),
                    ),
            )
            .child(div().flex_1())
            .children(
                extra.map(|extra| div().text_xs().text_color(rgb(palette.muted)).child(extra)),
            )
            .into_any_element()
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
            .gap_5()
            .w_full()
            .max_w(px(600.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_family(DISPLAY_FONT)
                            .text_size(px(28.0))
                            .line_height(px(34.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("About Loregalia"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_sm()
                            .text_color(rgb(palette.muted))
                            .child(
                                div()
                                    .id("about-version")
                                    .test_support()
                                    .aria_label(version.clone())
                                    .child(version),
                            )
                            .child("·")
                            .child(
                                Link::new("license-link")
                                    .href("https://github.com/ManusolJ/loregalia/blob/main/LICENSE")
                                    .child("MIT License"),
                            ),
                    )
                    .child(
                        div()
                            .pt_2()
                            .text_base()
                            .child("A local-first planner for what to watch, read and play next."),
                    ),
            )
            .child(Self::about_card(
                "about-sources",
                "Metadata sources",
                palette,
                SOURCES
                    .iter()
                    .map(|source| Self::source_row(source, palette))
                    .chain([div()
                        .id("tmdb-notice")
                        .test_support()
                        .aria_label(TMDB_NOTICE)
                        .pt_1()
                        .text_xs()
                        .text_color(rgb(palette.muted))
                        .child(format!("{TMDB_NOTICE}."))
                        .into_any_element()])
                    .collect(),
            ))
            .child(Self::about_card(
                "about-privacy",
                "Privacy",
                palette,
                PRIVACY_NOTES
                    .iter()
                    .map(|(icon, note)| {
                        div()
                            .flex()
                            .items_start()
                            .gap_3()
                            .child(
                                div().pt_0p5().child(
                                    Icon::new(*icon)
                                        .with_size(px(16.0))
                                        .text_color(rgb(palette.accent)),
                                ),
                            )
                            .child(div().flex_1().min_w_0().child(*note))
                            .into_any_element()
                    })
                    .collect(),
            ))
            .child(
                div()
                    .id("about-data")
                    .test_support()
                    .aria_label(format!("Data file: {data_path}"))
                    .text_xs()
                    .text_color(rgb(palette.muted))
                    .child(format!("Data file: {data_path}")),
            )
            .into_any_element()
    }
}
