use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{Icon, Selectable, Sizable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, Role, TestSupportExt, Window, div, px, rgb,
};
use loregalia_core::{
    library::{self, Filter, Shelf},
    settings,
};

use super::widgets::EmptyPanel;
use super::{Find, Screen, Shell};
use crate::theme::Palette;

const SEARCH_WIDTH: f32 = 200.0;
const FILTER_LABEL_WIDTH: f32 = 40.0;
const TAGS_MAX_HEIGHT: f32 = 120.0;

#[derive(Default)]
pub(super) struct FindState {
    pub(super) filter: Filter,
    pub(super) sort: library::Sort,
    pub(super) panel_open: bool,
    search: Option<Entity<InputState>>,
}

impl Shell {
    pub(super) fn on_text_change(
        input: &Entity<InputState>,
        cx: &mut Context<Self>,
        changed: impl Fn(&mut Self, String) + 'static,
    ) {
        cx.subscribe(input, move |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                let text = input.read(cx).value().to_string();
                changed(this, text);
                cx.notify();
            }
        })
        .detach();
    }

    pub(super) fn ensure_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.find.search.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Search"));
        Self::on_text_change(&input, cx, |this, text| {
            this.find.filter.text = text;
            this.viewport.to_top();
        });
        self.find.search = Some(input);
    }

    pub(super) fn clear_find(&mut self) {
        self.viewport.to_top();
        self.find.filter = Filter::default();
        self.find.search = None;
        self.find.panel_open = false;
    }

    pub(super) fn focus_search(&mut self, _: &Find, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen != Screen::Library || self.has_editor() {
            cx.propagate();
            return;
        }
        if let Some(search) = &self.find.search {
            let focus = search.read(cx).focus_handle(cx);
            focus.focus(window, cx);
        }
    }

    pub(super) fn dismiss_search(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(search) = self.find.search.clone() else {
            return false;
        };
        if !search.read(cx).focus_handle(cx).is_focused(window) {
            return false;
        }

        self.find.filter.text.clear();
        self.viewport.to_top();
        search.update(cx, |search, cx| search.set_value("", window, cx));
        self.focus_handle.focus(window, cx);
        true
    }

    pub(super) fn forget_missing_tags(&mut self) {
        let tags = &self.tags;
        self.find
            .filter
            .tags
            .retain(|id| tags.iter().any(|tag| &tag.id == id));
    }

    pub(super) fn search_element(&self, palette: &Palette) -> Option<AnyElement> {
        self.find.search.as_ref().map(|search| {
            div()
                .w(px(SEARCH_WIDTH))
                .child(
                    Input::new(search)
                        .id("library-search")
                        .aria_label("Search by title")
                        .cleanable(true)
                        .prefix(
                            Icon::new(IconName::Search)
                                .with_size(px(16.0))
                                .text_color(rgb(palette.muted)),
                        ),
                )
                .into_any_element()
        })
    }

    pub(super) fn filter_button(&self, sorts: bool, cx: &mut Context<Self>) -> AnyElement {
        let chosen = self.find.filter.tags.len();
        let name = if sorts { "Sort and filter" } else { "Filter" };
        let label = if chosen == 0 {
            name.to_owned()
        } else {
            format!("{name} ({chosen})")
        };
        let open = self.find.panel_open;

        Button::new("open-filters")
            .ghost()
            .icon(IconName::SlidersHorizontal)
            .label(label)
            .selected(open || chosen > 0)
            .toggled(open)
            .on_click(cx.listener(|this, _, _, cx| {
                this.find.panel_open = !this.find.panel_open;
                cx.notify();
            }))
            .into_any_element()
    }

    pub(super) fn filter_panel(
        &self,
        sorts: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.find.panel_open {
            return None;
        }
        let shelf = self.shelf;
        let panel = div()
            .id("filter-panel")
            .role(Role::Region)
            .test_support()
            .aria_label("Sort and filter")
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.line))
            .bg(rgb(palette.surface))
            .when(sorts, |panel| {
                panel.child(Self::filter_row(
                    "Sort",
                    Self::segmented(
                        library::Sort::ALL.map(|sort| {
                            Button::new(format!("library-sort-{}", sort.key()))
                                .ghost()
                                .label(sort.label(shelf))
                                .selected(self.find.sort == sort)
                                .toggled(self.find.sort == sort)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.find.sort = sort;
                                    this.viewport.to_top();
                                    cx.notify();
                                }))
                        }),
                        palette,
                    ),
                    palette,
                ))
            })
            .child(Self::filter_row(
                "Tags",
                self.tag_filters(palette, cx),
                palette,
            ));

        Some(panel.into_any_element())
    }

    fn filter_row(label: &'static str, content: AnyElement, palette: &Palette) -> AnyElement {
        div()
            .flex()
            .items_center()
            .gap_3()
            .child(
                div()
                    .w(px(FILTER_LABEL_WIDTH))
                    .flex_none()
                    .child(Self::section_label(label, palette)),
            )
            .child(div().min_w_0().flex().child(content))
            .into_any_element()
    }

    fn tag_filters(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.tags.is_empty() {
            return div()
                .text_xs()
                .text_color(rgb(palette.muted))
                .child("No tags yet. Add them from an item's details.")
                .into_any_element();
        }

        div()
            .id("filter-tags")
            .test_support()
            .max_h(px(TAGS_MAX_HEIGHT))
            .overflow_y_scroll()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(self.tags.iter().map(|tag| {
                let chosen = self.find.filter.tags.contains(&tag.id);
                let id = tag.id.clone();

                Button::new(format!("filter-tag-{}", tag.id))
                    .small()
                    .label(tag.name.clone())
                    .selected(chosen)
                    .toggled(chosen)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.find.filter.toggle_tag(&id);
                        this.viewport.to_top();
                        cx.notify();
                    }))
            }))
            .into_any_element()
    }

    pub(super) fn no_matches_element(palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        Self::empty_panel(
            EmptyPanel {
                id: "no-matches",
                icon: IconName::SearchX,
                message: "No matches".to_owned(),
                hint: "Try fewer words or fewer tags.",
            },
            Some(
                Button::new("clear-filters")
                    .label("Clear search and filters")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clear_find();
                        cx.notify();
                    })),
            ),
            palette,
        )
    }

    pub(super) fn shows_up_next(&self) -> bool {
        self.shelf == Shelf::All
            && self.layout == settings::Layout::List
            && !self.find.filter.is_active()
    }
}
