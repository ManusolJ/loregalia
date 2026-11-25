use std::time::{SystemTime, UNIX_EPOCH};

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Selectable, Sizable, button::Button, button::ButtonVariants,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, FontWeight, Role, TestSupportExt, div, px, rgb};
use loregalia_core::{
    dates, items,
    library::category_of,
    media::MediaKind,
    pick::{self, Ask, SHOWN, Shelves, Suggestion},
    statuses::Category,
};

use super::widgets::{Cover, EmptyPanel};
use super::{Activate, ITEM_ROW_CONTEXT, Screen, Shell};
use crate::theme::{DISPLAY_FONT, HOVER_ALPHA, Palette, SELECTED_ALPHA};

const TIMES: [Option<u32>; 5] = [None, Some(30), Some(60), Some(120), Some(180)];
const COVER_WIDTH: f32 = 80.0;
const LABEL_WIDTH: f32 = 72.0;

#[derive(Default)]
pub(super) struct PickState {
    ask: Ask,
    suggestions: Vec<Suggestion>,
    genres: Vec<String>,
    page: usize,
}

impl PickState {
    fn shown(&self) -> &[Suggestion] {
        let start = (self.page * SHOWN).min(self.suggestions.len());
        let end = (start + SHOWN).min(self.suggestions.len());

        &self.suggestions[start..end]
    }
}

fn seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64)
}

impl Shell {
    pub(super) fn begin_pick(&mut self, cx: &mut Context<Self>) {
        self.show(Screen::Pick);
        self.refresh_suggestions();
        cx.notify();
    }

    fn shelves(&self) -> Shelves<'_> {
        Shelves {
            items: &self.items,
            statuses: &self.statuses,
            lists: &self.lists,
            memberships: &self.memberships,
        }
    }

    fn refresh_suggestions(&mut self) {
        let today = dates::today();
        let genres = pick::genres(&self.shelves(), self.pick.ask.kind, &today);
        self.pick.ask.genre = self
            .pick
            .ask
            .genre
            .as_deref()
            .and_then(|wanted| pick::same_genre(&genres, wanted));
        let suggestions = pick::suggest(&self.shelves(), &self.pick.ask, &today, seed());
        self.pick.genres = genres;
        self.pick.suggestions = suggestions;
        self.pick.page = 0;
    }

    fn change_ask(&mut self, change: impl FnOnce(&mut Ask), cx: &mut Context<Self>) {
        change(&mut self.pick.ask);
        self.detail = None;
        self.refresh_suggestions();
        cx.notify();
    }

    fn show_others(&mut self, cx: &mut Context<Self>) {
        let pages = self.pick.suggestions.len().div_ceil(SHOWN).max(1);
        self.detail = None;
        if self.pick.page + 1 < pages {
            self.pick.page += 1;
        } else {
            self.refresh_suggestions();
        }
        cx.notify();
    }

    fn start_item(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(status) = self
            .item(id)
            .and_then(|item| pick::start_status(&self.statuses, item.media_kind))
            .map(|status| status.id.clone())
        else {
            return;
        };
        self.change_status(id.to_owned(), status, cx);
    }

    pub(super) fn pick_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("pick-screen")
            .role(Role::Region)
            .test_support()
            .aria_label("Pick for me")
            .w_full()
            .flex()
            .flex_col()
            .gap_6()
            .child(Self::form_heading("Pick for me"))
            .child(self.pick_questions(palette, cx))
            .child(self.pick_answers(palette, cx))
            .into_any_element()
    }

    fn pick_questions(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let ask = &self.pick.ask;
        let kinds = std::iter::once(None).chain(MediaKind::ALL.map(Some));
        let kinds =
            Self::segmented(
                kinds.map(|kind| {
                    let (key, label) = kind.map_or(("any", "Anything"), |kind| {
                        (kind.key(), kind.plural_label())
                    });
                    Button::new(format!("pick-kind-{key}"))
                        .ghost()
                        .label(label)
                        .selected(ask.kind == kind)
                        .toggled(ask.kind == kind)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.change_ask(|ask| ask.kind = kind, cx)
                        }))
                }),
                palette,
            );
        let times = Self::segmented(
            TIMES.map(|minutes| {
                let key = minutes.map_or_else(|| "any".to_owned(), |minutes| minutes.to_string());
                Button::new(format!("pick-time-{key}"))
                    .ghost()
                    .label(pick::time_label(minutes))
                    .selected(ask.minutes == minutes)
                    .toggled(ask.minutes == minutes)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.change_ask(|ask| ask.minutes = minutes, cx)
                    }))
            }),
            palette,
        );
        let genres = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(self.pick.genres.iter().map(|genre| {
                let chosen = ask.genre.as_ref() == Some(genre);
                let next = (!chosen).then(|| genre.clone());

                Button::new(format!("pick-genre-{genre}"))
                    .small()
                    .label(genre.clone())
                    .selected(chosen)
                    .toggled(chosen)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let next = next.clone();
                        this.change_ask(|ask| ask.genre = next, cx)
                    }))
            }))
            .into_any_element();

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(Self::question("What", kinds, palette))
            .child(Self::question("How long", times, palette))
            .when(!self.pick.genres.is_empty(), |questions| {
                questions.child(Self::question("Genre", genres, palette))
            })
            .into_any_element()
    }

    fn question(label: &'static str, answer: AnyElement, palette: &Palette) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(px(LABEL_WIDTH))
                    .flex_none()
                    .child(Self::section_label(label, palette)),
            )
            .child(div().flex().min_w_0().child(answer))
            .into_any_element()
    }

    fn pick_answers(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let shown = self.pick.shown();
        if shown.is_empty() {
            return self.nothing_to_pick(palette, cx);
        }
        let total = self.pick.suggestions.len();
        let first = self.pick.page * SHOWN + 1;
        let last = first + shown.len() - 1;
        let seen = if total == 1 {
            "The only one that fits".to_owned()
        } else {
            format!("{first} to {last} of {total} that fit")
        };

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Button::new("pick-others")
                            .icon(IconName::Shuffle)
                            .label("Show others")
                            .disabled(total <= SHOWN)
                            .on_click(cx.listener(|this, _, _, cx| this.show_others(cx))),
                    )
                    .child(
                        div()
                            .id("pick-count")
                            .test_support()
                            .aria_label(seen.clone())
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(seen),
                    ),
            )
            .child(
                div()
                    .id("pick-cards")
                    .role(Role::List)
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .children(shown.iter().filter_map(|suggestion| {
                        let item = self.item(&suggestion.item_id)?;
                        Some(self.pick_card(item, &suggestion.reasons, palette, cx))
                    })),
            )
            .into_any_element()
    }

    fn pick_card(
        &self,
        item: &items::Item,
        reasons: &[String],
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.detail.as_deref() == Some(item.id.as_str());
        let started = pick::start_status(&self.statuses, item.media_kind)
            .is_some_and(|status| status.id == item.status_id);
        let on_hold = category_of(&self.statuses, &item.status_id) == Some(Category::Paused);
        let label = match (started, on_hold) {
            (true, _) => "Started",
            (false, true) => "Resume",
            (false, false) => "Start",
        };
        let open = item.id.clone();
        let start = item.id.clone();

        div()
            .id(format!("pick-card-{}", item.id))
            .role(Role::ListItem)
            .test_support()
            .aria_label(item.title.clone())
            .when_some(self.row_focus.get(&item.id), |card, handle| {
                card.track_focus(handle).tab_index(0)
            })
            .key_context(ITEM_ROW_CONTEXT)
            .on_action(cx.listener({
                let id = item.id.clone();
                move |this, _: &Activate, _, cx| this.open_detail(id.clone(), cx)
            }))
            .w_full()
            .p_3()
            .flex()
            .gap_4()
            .rounded(px(10.0))
            .border_2()
            .border_color(rgb(palette.line))
            .focus_visible(|style| style.border_color(rgb(palette.accent)))
            .bg(rgb(palette.surface))
            .cursor_pointer()
            .when(selected, |card| {
                card.bg(rgb(palette.accent).alpha(SELECTED_ALPHA))
            })
            .when(!selected, |card| {
                card.hover(|style| style.bg(rgb(palette.content).alpha(HOVER_ALPHA)))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.open_detail(open.clone(), cx)))
            .child(Self::cover_element(
                Cover::of("pick-cover", item, COVER_WIDTH),
                palette,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .truncate()
                            .font_family(DISPLAY_FONT)
                            .text_size(px(18.0))
                            .line_height(px(24.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(Self::item_metadata(item)),
                    )
                    .child(
                        div()
                            .id(format!("pick-reasons-{}", item.id))
                            .test_support()
                            .aria_label(reasons.join(". "))
                            .flex()
                            .flex_col()
                            .children(
                                reasons
                                    .iter()
                                    .map(|reason| div().truncate().child(reason.clone())),
                            ),
                    ),
            )
            .child(
                div().flex_none().child(
                    Button::new(format!("pick-start-{}", item.id))
                        .when(!started, |button| button.primary())
                        .icon(if started {
                            IconName::Check
                        } else {
                            IconName::Play
                        })
                        .label(label)
                        .disabled(started)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.start_item(&start, cx)
                        })),
                ),
            )
            .into_any_element()
    }

    fn nothing_to_pick(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let asked = self.pick.ask != Ask::default();
        let (message, hint) = if asked {
            ("Nothing fits", "Try more time, another kind or any genre.")
        } else {
            (
                "Nothing is waiting",
                "Add something to watch, read or play, and it can be picked for you.",
            )
        };
        let action = if asked {
            Button::new("pick-reset")
                .label("Start over")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.focus_handle.focus(window, cx);
                    this.change_ask(|ask| *ask = Ask::default(), cx)
                }))
        } else {
            Button::new("pick-add")
                .primary()
                .icon(IconName::Plus)
                .label("Add item")
                .on_click(cx.listener(|this, _, window, cx| this.begin_create_item(window, cx)))
        };

        Self::empty_panel(
            EmptyPanel {
                id: "pick-empty",
                icon: IconName::Dices,
                message: message.to_owned(),
                hint,
            },
            Some(action),
            palette,
        )
    }
}
