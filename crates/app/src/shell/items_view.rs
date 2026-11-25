use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Disableable, Selectable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, FontWeight, Role, TestSupportExt, Window,
    div, px, rgb,
};
use loregalia_core::{
    Database, items, library::Shelf, list_items, media::MediaKind, providers, settings,
    upcoming::upcoming,
};

use super::detail_view::DETAIL_WIDTH;
use super::metadata_view::SearchStatus;
use super::widgets::{
    Confirmation, Cover, FORM_WIDTH, GRID_COVER_WIDTH, ROW_COVER_WIDTH, TILE_COVER_WIDTH,
};
use super::{
    Activate, CONTENT_WIDTH, ITEM_ROW_CONTEXT, PAGE_GUTTER, Screen, Shell, detail_docks, load,
    sidebar_width,
};
use crate::startup::StorageState;
use crate::theme::{HOVER_ALPHA, Palette, SELECTED_ALPHA};

const UP_NEXT_LIMIT: usize = 6;
const STATUS_COLUMN: f32 = 104.0;
const YEAR_FIELD_WIDTH: f32 = 120.0;
const GRID_TILE_INSET: f32 = 12.0;
const TILE_WIDTH: f32 = GRID_COVER_WIDTH + GRID_TILE_INSET;
const TILE_HEIGHT: f32 = 312.0;
const TILE_PITCH: f32 = TILE_HEIGHT + 16.0;
const ROW_PITCH: f32 = 85.0;

const DELETE_ITEM: Confirmation = Confirmation {
    id: "item-delete-confirmation",
    cancel_id: "cancel-item-delete",
    confirm_id: "confirm-item-delete",
    confirm_label: "Delete",
    consequence: "It is removed from every list too. This cannot be undone.",
};

pub(super) struct ItemEditor {
    pub(super) id: Option<String>,
    pub(super) kind: MediaKind,
    pub(super) title: Entity<InputState>,
    pub(super) year: Entity<InputState>,
    pub(super) search_input: Option<Entity<InputState>>,
    pub(super) search: SearchStatus,
    pub(super) search_request: u64,
    pub(super) lookup: Option<items::Lookup>,
}

pub(super) struct ListItemActions<'a> {
    pub(super) list_id: &'a str,
    pub(super) manual: bool,
    pub(super) can_move_up: bool,
    pub(super) can_move_down: bool,
}

impl Shell {
    pub(super) fn begin_create_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let title = cx.new(|cx| InputState::new(window, cx));
        let year = cx.new(|cx| InputState::new(window, cx));
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search by title"));
        let focus = search_input.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        for input in [&title, &year] {
            Self::submit_on_enter(input, window, cx, |this, _, cx| this.save_item(cx));
        }
        Self::submit_on_enter(&search_input, window, cx, |this, _, cx| {
            this.search_metadata(cx)
        });

        self.screen = Screen::Library;
        self.close_editors();
        self.clear_find();
        self.detail = None;
        self.item_editor = Some(ItemEditor {
            id: None,
            kind: MediaKind::Movie,
            title,
            year,
            search_input: Some(search_input),
            search: SearchStatus::Idle,
            search_request: 0,
            lookup: None,
        });
        cx.notify();
    }

    pub(super) fn begin_edit_item(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.item(&id) else {
            return;
        };
        let (kind, title_value, year_value) = (
            item.media_kind,
            item.title.clone(),
            item.release_year.map(|year| year.to_string()),
        );
        let title = cx.new(|cx| {
            let mut input = InputState::new(window, cx)
                .placeholder("Title")
                .default_value(title_value);
            input.select_all(window, cx);
            input
        });
        let year = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Year")
                .default_value(year_value.unwrap_or_default())
        });
        let focus = title.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        for input in [&title, &year] {
            Self::submit_on_enter(input, window, cx, |this, _, cx| this.save_item(cx));
        }

        self.close_editors();
        self.item_editor = Some(ItemEditor {
            id: Some(id),
            kind,
            title,
            year,
            search_input: None,
            search: SearchStatus::Idle,
            search_request: 0,
            lookup: None,
        });
        cx.notify();
    }

    pub(super) fn select_item_kind(&mut self, kind: MediaKind, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.item_editor else {
            return;
        };
        if editor.kind == kind {
            return;
        }

        editor.kind = kind;
        editor.search_request += 1;
        editor.search = SearchStatus::Idle;
        editor.lookup = None;

        cx.notify();
    }

    pub(super) fn save_item(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.item_editor.take() else {
            return;
        };
        let title = editor.title.read(cx).value().trim().to_owned();
        let year_text = editor.year.read(cx).value().trim().to_owned();
        let year = if year_text.is_empty() {
            None
        } else {
            match year_text.parse::<i32>() {
                Ok(year) => Some(year),
                Err(_) => {
                    self.item_editor = Some(editor);
                    return;
                }
            }
        };
        if title.is_empty() {
            self.item_editor = Some(editor);
            return;
        }
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        let id = editor.id;
        let kind = editor.kind;
        let lookup = editor.lookup;
        let list_id = self.selected.clone();
        self.close_season_panels();
        if lookup.is_some() {
            self.resume_metadata_refresh();
        }

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            match id {
                Some(id) => {
                    items::update(&database, &id, kind, &title, year)?;
                }
                None => {
                    let created = match lookup {
                        Some(lookup) => {
                            items::create_from_lookup(&database, kind, &title, year, &lookup)?
                        }
                        None => items::create(&database, kind, &title, year)?,
                    };
                    if let Some(list_id) = &list_id {
                        list_items::add(&database, list_id, &created.id)?;
                    }
                }
            }
            load(&database)
        });
    }

    pub(super) fn cancel_item(&mut self, cx: &mut Context<Self>) {
        self.close_editors();
        cx.notify();
    }

    pub(super) fn begin_delete_item(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_editors();
        self.detail = None;
        self.confirm_delete_item = Some(id);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub(super) fn cancel_delete_item(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete_item = None;
        cx.notify();
    }

    pub(super) fn delete_item(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete_item.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            items::delete(&database, &id)?;
            load(&database)
        });
    }

    pub(super) fn change_status(
        &mut self,
        item_id: String,
        status_id: String,
        cx: &mut Context<Self>,
    ) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            items::set_status(&database, &item_id, &status_id)?;
            load(&database)
        });
    }

    pub(super) fn item_editor_element(
        &self,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(editor) = &self.item_editor else {
            return self.empty_element(palette, cx);
        };
        let title = editor.title.read(cx).value().trim().to_owned();
        let year = editor.year.read(cx).value().trim().to_owned();
        let can_save = !title.is_empty() && (year.is_empty() || year.parse::<i32>().is_ok());
        let duplicate = if editor.id.is_none() && editor.lookup.is_none() {
            items::duplicate(&self.items, editor.kind, &title)
        } else {
            None
        };
        let save_label = if duplicate.is_some() {
            "Save anyway"
        } else {
            "Save"
        };
        let heading = if editor.id.is_some() {
            "Edit item"
        } else {
            "Add an item"
        };
        let adding = editor.id.is_none();

        let mut panel = div()
            .id("item-editor")
            .test_support()
            .w_full()
            .max_w(px(FORM_WIDTH))
            .flex()
            .flex_col()
            .gap_6()
            .child(Self::form_heading(heading))
            .child(div().flex().child(Self::segmented(
                MediaKind::ALL.map(|kind| {
                    Button::new(format!("item-kind-{}", kind.key()))
                        .ghost()
                        .icon(Self::media_kind_icon(kind))
                        .label(kind.label())
                        .selected(editor.kind == kind)
                        .toggled(editor.kind == kind)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.select_item_kind(kind, cx)),
                        )
                }),
                palette,
            )));

        if adding && providers::supported(editor.kind) {
            panel = panel.child(self.metadata_search_element(editor, palette, cx));
        }

        panel = panel.child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .when(adding, |fields| {
                    fields.child(
                        div()
                            .pt_4()
                            .border_t_1()
                            .border_color(rgb(palette.line))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if editor.lookup.is_some() {
                                "Check the details"
                            } else {
                                "Or fill it in yourself"
                            }),
                    )
                })
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(Self::section_label("Title", palette))
                                .child(
                                    Input::new(&editor.title)
                                        .id("item-title-input")
                                        .aria_label("Title"),
                                ),
                        )
                        .child(
                            div()
                                .w(px(YEAR_FIELD_WIDTH))
                                .flex_none()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(Self::section_label("Year", palette))
                                .child(
                                    Input::new(&editor.year)
                                        .id("item-year-input")
                                        .aria_label("Release year"),
                                ),
                        ),
                ),
        );

        if let Some(duplicate) = duplicate {
            let message = format!(
                "\"{}\" is already in your library. You can still save it.",
                duplicate.title
            );
            panel = panel.child(
                div()
                    .id("item-duplicate-warning")
                    .role(Role::Alert)
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(rgb(palette.status_paused))
                    .text_xs()
                    .child(message),
            );
        }

        panel
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("save-item")
                            .primary()
                            .label(save_label)
                            .disabled(!can_save)
                            .on_click(cx.listener(|this, _, _, cx| this.save_item(cx))),
                    )
                    .child(
                        Button::new("cancel-item")
                            .label("Cancel")
                            .on_click(cx.listener(|this, _, _, cx| this.cancel_item(cx))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn item_row(
        &self,
        item: &items::Item,
        list_actions: Option<ListItemActions<'_>>,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.detail.as_deref() == Some(item.id.as_str());
        let badge = self
            .status_of(&item.status_id)
            .map(|status| Self::status_badge(status, format!("item-status-{}", item.id), palette));
        let tags = self.tags_for(&self.item_tags, &item.id);
        let id = item.id.clone();

        let mut moves = div().flex().items_center().gap_1();
        if let Some(list_actions) = list_actions
            && list_actions.manual
        {
            let list_id = list_actions.list_id.to_owned();
            let up_list_id = list_id.clone();
            let up_id = id.clone();
            let down_id = id.clone();
            moves = moves
                .child(
                    Button::new(format!("move-up-{}", item.id))
                        .ghost()
                        .icon(IconName::ArrowUp)
                        .tooltip("Move up")
                        .accessibility_label("Move up")
                        .disabled(!list_actions.can_move_up)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.move_item_up(up_list_id.clone(), up_id.clone(), cx)
                        })),
                )
                .child(
                    Button::new(format!("move-down-{}", item.id))
                        .ghost()
                        .icon(IconName::ArrowDown)
                        .tooltip("Move down")
                        .accessibility_label("Move down")
                        .disabled(!list_actions.can_move_down)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.move_item_down(list_id.clone(), down_id.clone(), cx)
                        })),
                );
        }

        div()
            .id(format!("item-row-{}", item.id))
            .role(Role::ListItem)
            .test_support()
            .aria_label(item.title.clone())
            .when_some(self.row_focus.get(&item.id), |row, handle| {
                row.track_focus(handle).tab_index(0)
            })
            .key_context(ITEM_ROW_CONTEXT)
            .on_action(cx.listener({
                let id = item.id.clone();
                move |this, _: &Activate, _, cx| this.open_detail(id.clone(), cx)
            }))
            .flex()
            .items_center()
            .gap_3()
            .px_2()
            .py_2()
            .rounded(px(10.0))
            .border_2()
            .border_color(rgb(palette.accent).alpha(0.0))
            .focus_visible(|style| style.border_color(rgb(palette.accent)))
            .cursor_pointer()
            .when(selected, |row| {
                row.bg(rgb(palette.accent).alpha(SELECTED_ALPHA))
            })
            .when(!selected, |row| {
                row.hover(|style| style.bg(rgb(palette.content).alpha(HOVER_ALPHA)))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.open_detail(id.clone(), cx)))
            .child(Self::cover_element(
                Cover::of("cover", item, ROW_COVER_WIDTH),
                palette,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .font_weight(FontWeight::MEDIUM)
                            .truncate()
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .items_center()
                            .gap_x_2()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(div().flex_none().child(Self::item_metadata(item)))
                            .children(self.upcoming_label(item).map(|label| {
                                div()
                                    .id(format!("item-upcoming-{}", item.id))
                                    .test_support()
                                    .aria_label(label.clone())
                                    .flex_none()
                                    .text_color(rgb(palette.accent))
                                    .child(format!("· {label}"))
                            }))
                            .children(Self::finished_label(item).map(|finished| {
                                div()
                                    .id(format!("item-finished-{}", item.id))
                                    .test_support()
                                    .aria_label(finished.clone())
                                    .flex_none()
                                    .child(format!("· {finished}"))
                            }))
                            .children(tags.into_iter().map(|tag| {
                                div()
                                    .id(format!("item-tag-{}-{}", item.id, tag.id))
                                    .test_support()
                                    .aria_label(tag.name.clone())
                                    .flex_none()
                                    .child(format!("#{}", tag.name))
                            })),
                    ),
            )
            .child(moves)
            .child(
                div()
                    .w(px(STATUS_COLUMN))
                    .flex_none()
                    .flex()
                    .children(badge),
            )
            .into_any_element()
    }

    pub(super) fn up_next_element(
        &self,
        items: &[&items::Item],
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id("up-next")
            .role(Role::Region)
            .test_support()
            .aria_label("Up next")
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_base()
                            .child("Up next"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child("In progress and on hold"),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .children(items.iter().take(UP_NEXT_LIMIT).map(|item| {
                        let id = item.id.clone();
                        div()
                            .id(format!("up-next-{}", item.id))
                            .role(Role::Button)
                            .test_support()
                            .aria_label(item.title.clone())
                            .w(px(TILE_COVER_WIDTH))
                            .flex_none()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.open_detail(id.clone(), cx)),
                            )
                            .child(Self::cover_element(
                                Cover::of("up-next-cover", item, TILE_COVER_WIDTH),
                                palette,
                            ))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(item.title.clone()),
                            )
                            .children(self.status_of(&item.status_id).map(|status| {
                                div().flex().child(Self::status_badge(
                                    status,
                                    format!("up-next-status-{}", item.id),
                                    palette,
                                ))
                            }))
                    })),
            )
            .into_any_element()
    }

    fn upcoming_label(&self, item: &items::Item) -> Option<String> {
        if self.shelf != Shelf::ComingSoon || self.selected.is_some() {
            return None;
        }

        upcoming(item, &self.statuses, &self.today).map(|found| found.label(&self.today))
    }

    fn item_tile(
        &self,
        item: &items::Item,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.detail.as_deref() == Some(item.id.as_str());
        let id = item.id.clone();

        div()
            .id(format!("item-tile-{}", item.id))
            .role(Role::ListItem)
            .test_support()
            .aria_label(item.title.clone())
            .when_some(self.row_focus.get(&item.id), |tile, handle| {
                tile.track_focus(handle).tab_index(0)
            })
            .key_context(ITEM_ROW_CONTEXT)
            .on_action(cx.listener({
                let id = item.id.clone();
                move |this, _: &Activate, _, cx| this.open_detail(id.clone(), cx)
            }))
            .w(px(TILE_WIDTH))
            .h(px(TILE_HEIGHT))
            .flex_none()
            .p_1()
            .flex()
            .flex_col()
            .gap_2()
            .rounded(px(10.0))
            .border_2()
            .border_color(rgb(palette.accent).alpha(0.0))
            .focus_visible(|style| style.border_color(rgb(palette.accent)))
            .cursor_pointer()
            .when(selected, |tile| {
                tile.bg(rgb(palette.accent).alpha(SELECTED_ALPHA))
            })
            .when(!selected, |tile| {
                tile.hover(|style| style.bg(rgb(palette.content).alpha(HOVER_ALPHA)))
            })
            .on_click(cx.listener(move |this, _, _, cx| this.open_detail(id.clone(), cx)))
            .child(Self::cover_element(
                Cover::of("tile-cover", item, GRID_COVER_WIDTH),
                palette,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(
                        div()
                            .truncate()
                            .line_height(px(22.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_xs()
                            .line_height(px(20.0))
                            .text_color(rgb(palette.muted))
                            .child(match self.upcoming_label(item) {
                                Some(label) => div()
                                    .id(format!("item-upcoming-{}", item.id))
                                    .test_support()
                                    .aria_label(label.clone())
                                    .truncate()
                                    .child(label)
                                    .into_any_element(),
                                None => Self::item_metadata(item).into_any_element(),
                            }),
                    ),
            )
            .children(self.status_of(&item.status_id).map(|status| {
                div().flex().child(Self::status_badge(
                    status,
                    format!("item-status-{}", item.id),
                    palette,
                ))
            }))
            .into_any_element()
    }

    pub(super) fn layout_switch(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        Self::segmented(
            settings::Layout::ALL.map(|layout| {
                Button::new(format!("layout-{}", layout.key()))
                    .ghost()
                    .icon(match layout {
                        settings::Layout::List => IconName::List,
                        settings::Layout::Grid => IconName::LayoutGrid,
                    })
                    .tooltip(layout.label())
                    .accessibility_label(layout.label())
                    .selected(self.layout == layout)
                    .toggled(self.layout == layout)
                    .on_click(cx.listener(move |this, _, _, cx| this.set_layout(layout, cx)))
            }),
            palette,
        )
    }

    fn set_layout(&mut self, layout: settings::Layout, cx: &mut Context<Self>) {
        if self.layout == layout {
            return;
        }
        self.layout = layout;
        self.viewport.to_top();
        cx.notify();
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        let task = cx.background_spawn(async move {
            let database = Database::open(path)?;
            settings::set_layout(&database, layout)
        });

        cx.spawn(async move |this, cx| {
            if let Err(error) = task.await {
                let _ = this.update(cx, |this, cx| {
                    this.record(&format!("Could not save the layout: {error}"), cx);
                });
            }
        })
        .detach();
    }

    pub(super) fn items_element(
        &self,
        items: &[&items::Item],
        list_id: Option<&str>,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (columns, pitch) = self.list_shape();
        let rows = items.len().div_ceil(columns);
        let mut drawn: Vec<usize> = self.viewport.rows(pitch, rows).collect();
        if let Some(id) = &self.followed
            && let Some(index) = items.iter().position(|item| &item.id == id)
            && !drawn.contains(&(index / columns))
        {
            drawn.push(index / columns);
            drawn.sort_unstable();
        }
        let grid = self.layout == settings::Layout::Grid;
        let viewport = self.viewport.clone();

        div()
            .on_children_prepainted(move |bounds, window, _| {
                if let Some(marker) = bounds.first()
                    && viewport.place_list(marker.origin.y)
                {
                    window.on_next_frame(|window, _| window.refresh());
                }
            })
            .id("items")
            .role(Role::List)
            .test_support()
            .relative()
            .h(px(pitch * rows as f32))
            .when(grid, |list| list.mx(px(-GRID_TILE_INSET / 2.0)))
            .when(!grid, |list| list.w_full())
            .child(div().absolute().top_0().left_0().w_full().h(px(0.0)))
            .children(drawn.into_iter().map(|row| {
                let place = div()
                    .absolute()
                    .top(px(pitch * row as f32))
                    .left_0()
                    .right_0()
                    .h(px(pitch));
                if grid {
                    let start = row * columns;
                    let end = (start + columns).min(items.len());
                    return place.flex().children(
                        items[start..end]
                            .iter()
                            .map(|item| self.item_tile(item, palette, cx)),
                    );
                }

                let list_actions = list_id.map(|list_id| ListItemActions {
                    list_id,
                    manual: self.sort == list_items::Sort::Manual && !self.find.filter.is_active(),
                    can_move_up: row > 0,
                    can_move_down: row + 1 < items.len(),
                });
                place
                    .border_t_1()
                    .border_color(rgb(palette.line).alpha(if row > 0 { 1.0 } else { 0.0 }))
                    .py_0p5()
                    .child(self.item_row(items[row], list_actions, palette, cx))
            }))
            .into_any_element()
    }

    pub(super) fn list_shape(&self) -> (usize, f32) {
        if self.layout == settings::Layout::List {
            return (1, ROW_PITCH);
        }
        let window = px(self.window_width);
        let docked = detail_docks(window) && self.detail_item().is_some();
        let detail = if docked { DETAIL_WIDTH } else { 0.0 };
        let page =
            self.window_width - f32::from(sidebar_width(window)) - detail - 2.0 * PAGE_GUTTER;
        let content = page.min(CONTENT_WIDTH) + GRID_TILE_INSET;

        (((content / TILE_WIDTH).floor() as usize).max(1), TILE_PITCH)
    }

    pub(super) fn delete_item_element(
        &self,
        id: &str,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let title = self
            .items
            .iter()
            .find(|item| item.id == id)
            .map(|item| item.title.as_str())
            .unwrap_or("this item");

        Self::confirm_element(
            DELETE_ITEM,
            format!("Delete \"{title}\"?"),
            Self::cancel_delete_item,
            Self::delete_item,
            palette,
            cx,
        )
    }
}
