use gpui_kit::assets::IconName;
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{
    Disableable, Selectable, Sizable, button::Button, button::ButtonVariants,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, Focusable, Role, TestSupportExt, Window, div, rgb,
};
use loregalia_core::{Database, items, media::MediaKind, seasons};

use super::{Shell, load};
use crate::startup::StorageState;
use crate::theme::Palette;

pub(super) fn season_number(value: &str) -> Option<i32> {
    value
        .trim()
        .parse::<i32>()
        .ok()
        .filter(|number| *number >= 0)
}

pub(super) struct SeasonEditor {
    pub(super) show_id: String,
    pub(super) number: Entity<InputState>,
}

impl Shell {
    pub(super) fn close_season_panels(&mut self) {
        self.season_editor = None;
        self.season_status_picker = None;
        self.confirm_delete_season = None;
    }

    pub(super) fn toggle_seasons(
        &mut self,
        show_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let same = self
            .season_editor
            .as_ref()
            .is_some_and(|editor| editor.show_id == show_id);
        self.close_inline_panels();
        if same {
            cx.notify();
            return;
        }

        let number = cx.new(|cx| InputState::new(window, cx).placeholder("Season number"));
        let focus = number.read(cx).focus_handle(cx);
        focus.focus(window, cx);
        Self::submit_on_enter(&number, window, cx, |this, window, cx| {
            this.add_season(window, cx)
        });

        self.season_editor = Some(SeasonEditor { show_id, number });
        cx.notify();
    }

    pub(super) fn add_season(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editor) = &self.season_editor else {
            return;
        };
        let Some(number) = season_number(editor.number.read(cx).value().as_ref()) else {
            return;
        };
        let show_id = editor.show_id.clone();
        let input = editor.number.clone();
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        input.update(cx, |input, cx| input.set_value("", window, cx));

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            seasons::create(&database, &show_id, number)?;
            load(&database)
        });
    }

    pub(super) fn toggle_season_status(&mut self, season_id: String, cx: &mut Context<Self>) {
        self.season_status_picker =
            if self.season_status_picker.as_deref() == Some(season_id.as_str()) {
                None
            } else {
                Some(season_id)
            };
        cx.notify();
    }

    pub(super) fn change_season_status(
        &mut self,
        season_id: String,
        status_id: String,
        cx: &mut Context<Self>,
    ) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        self.season_status_picker = None;

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            seasons::set_status(&database, &season_id, &status_id)?;
            load(&database)
        });
    }

    pub(super) fn begin_delete_season(&mut self, season_id: String, cx: &mut Context<Self>) {
        self.season_status_picker = None;
        self.confirm_delete_season = Some(season_id);
        cx.notify();
    }

    pub(super) fn cancel_delete_season(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete_season = None;
        cx.notify();
    }

    pub(super) fn delete_season(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.confirm_delete_season.take() else {
            return;
        };
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.reload_after(cx, async move {
            let database = Database::open(path)?;
            seasons::delete(&database, &id)?;
            load(&database)
        });
    }

    pub(super) fn seasons_for(&self, show_id: &str) -> Vec<&seasons::Season> {
        self.seasons
            .iter()
            .filter(|season| season.show_id == show_id)
            .collect()
    }

    pub(super) fn season_row(
        &self,
        season: &seasons::Season,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = format!("Season {}", season.season_number);
        let chip =
            self.statuses
                .iter()
                .find(|status| status.id == season.status_id)
                .map(|status| {
                    let season_id = season.id.clone();
                    Self::status_chip(status, format!("season-status-{}", season.id), palette)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_season_status(season_id.clone(), cx)
                        }))
                });
        let picker = (self.season_status_picker.as_deref() == Some(season.id.as_str()))
            .then(|| self.season_status_picker_element(season, cx));

        div()
            .id(format!("season-{}", season.id))
            .role(Role::ListItem)
            .test_support()
            .aria_label(label.clone())
            .flex()
            .flex_col()
            .gap_2()
            .py_1p5()
            .border_b_1()
            .border_color(rgb(palette.line))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(div().child(label))
                    .child(
                        div().flex().items_center().gap_1().children(chip).child(
                            Button::new(format!("season-delete-{}", season.id))
                                .ghost()
                                .icon(IconName::Trash)
                                .tooltip("Delete season")
                                .accessibility_label("Delete season")
                                .on_click(cx.listener({
                                    let season_id = season.id.clone();
                                    move |this, _, _, cx| {
                                        this.begin_delete_season(season_id.clone(), cx)
                                    }
                                })),
                        ),
                    ),
            )
            .children(picker)
            .into_any_element()
    }

    pub(super) fn season_status_picker_element(
        &self,
        season: &seasons::Season,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let season_id = season.id.clone();

        div()
            .id(format!("season-status-picker-{}", season.id))
            .test_support()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(
                self.statuses
                    .iter()
                    .filter(|status| status.media_kind == MediaKind::Show)
                    .map(|status| {
                        let season_id = season_id.clone();
                        let status_id = status.id.clone();

                        Button::new(format!("season-set-status-{}-{}", season.id, status.id))
                            .label(status.label.clone())
                            .selected(status.id == season.status_id)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_season_status(season_id.clone(), status_id.clone(), cx)
                            }))
                    }),
            )
            .into_any_element()
    }

    pub(super) fn season_delete_element(
        &self,
        season_id: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(season_id) = season_id else {
            return div().into_any_element();
        };
        let message = self
            .seasons
            .iter()
            .find(|season| season.id == season_id)
            .map(|season| format!("Delete season {}?", season.season_number))
            .unwrap_or_else(|| "Delete this season?".to_owned());

        div()
            .id("season-delete-confirmation")
            .role(Role::Alert)
            .test_support()
            .aria_label(message.clone())
            .flex()
            .items_center()
            .gap_2()
            .child(message)
            .child(
                Button::new("cancel-season-delete")
                    .label("Cancel")
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_delete_season(cx))),
            )
            .child(
                Button::new("confirm-season-delete")
                    .label("Delete")
                    .danger()
                    .on_click(cx.listener(|this, _, _, cx| this.delete_season(cx))),
            )
            .into_any_element()
    }

    pub(super) fn seasons_section(
        &self,
        show: &items::Item,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let seasons = self.seasons_for(&show.id);
        let editor = self
            .season_editor
            .as_ref()
            .filter(|editor| editor.show_id == show.id);
        let confirming = self.confirm_delete_season.clone();
        let show_id = show.id.clone();

        div()
            .id("seasons-panel")
            .test_support()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(Self::section_label("Seasons", palette))
                    .child(
                        Button::new(format!("item-seasons-{}", show.id))
                            .ghost()
                            .small()
                            .icon(IconName::Plus)
                            .label("Add season")
                            .selected(editor.is_some())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.toggle_seasons(show_id.clone(), window, cx)
                            })),
                    ),
            )
            .child(if seasons.is_empty() {
                div()
                    .id("seasons-empty")
                    .role(Role::Status)
                    .test_support()
                    .aria_label("No seasons yet")
                    .text_color(rgb(palette.muted))
                    .text_xs()
                    .child("No seasons yet")
                    .into_any_element()
            } else {
                div()
                    .id("seasons")
                    .role(Role::List)
                    .test_support()
                    .flex()
                    .flex_col()
                    .children(
                        seasons
                            .iter()
                            .map(|season| self.season_row(season, palette, cx)),
                    )
                    .into_any_element()
            })
            .children(editor.map(|editor| {
                let can_add =
                    season_number(editor.number.read(cx).value().as_ref()).is_some_and(|number| {
                        !seasons.iter().any(|season| season.season_number == number)
                    });

                div()
                    .flex()
                    .gap_2()
                    .child(
                        Input::new(&editor.number)
                            .id("season-number-input")
                            .aria_label("Season number"),
                    )
                    .child(
                        Button::new("add-season")
                            .primary()
                            .label("Add")
                            .disabled(!can_add)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_season(window, cx)),
                            ),
                    )
            }))
            .when(confirming.is_some(), |panel| {
                panel.child(self.season_delete_element(confirming.as_deref(), cx))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::season_number;

    #[test]
    fn season_numbers_are_whole_non_negative_numbers() {
        assert_eq!(season_number("0"), Some(0));
        assert_eq!(season_number(" 12 "), Some(12));
        assert_eq!(season_number("-1"), None);
        assert_eq!(season_number("1.5"), None);
        assert_eq!(season_number(""), None);
    }
}
