use gpui_kit::component::{Selectable, button::Button};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, AppContext, Context, FontWeight, Role, TestSupportExt, div, px, rgb};
use loregalia_core::{Database, settings};

use super::{Screen, Shell};
use crate::startup::StorageState;
use crate::theme::Palette;

impl Shell {
    pub(super) fn begin_settings(&mut self, cx: &mut Context<Self>) {
        self.show(Screen::Settings);
        self.settings_error = None;
        cx.notify();
    }

    pub(super) fn set_theme(&mut self, theme: settings::Theme, cx: &mut Context<Self>) {
        self.theme = theme;
        crate::theme::apply_choice(theme, cx);
        self.settings_error = None;

        let StorageState::Ready(path) = self.storage.clone() else {
            self.settings_error = Some("Settings need a working database.".to_owned());
            cx.notify();
            return;
        };

        let task = cx.background_spawn(async move {
            let database = Database::open(path)?;
            settings::set_theme(&database, theme)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    let message = format!("Could not save the theme: {error}");
                    this.record(&message, cx);
                    this.settings_error = Some(message);
                }
                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    pub(super) fn settings_element(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut panel =
            div()
                .id("settings-screen")
                .role(Role::Region)
                .test_support()
                .aria_label(format!("Settings, {} theme", self.theme.label()))
                .flex()
                .flex_col()
                .gap_6()
                .w_full()
                .h_full()
                .max_w(px(640.0))
                .child(
                    div()
                        .text_2xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Settings"),
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
                                .child("Theme"),
                        )
                        .child(
                            div()
                                .max_w(px(520.0))
                                .text_sm()
                                .text_color(rgb(palette.muted))
                                .child("Follow the system theme, or keep a fixed one."),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .children(settings::Theme::ALL.map(|theme| {
                                    Button::new(format!("theme-{}", theme.key()))
                                        .label(theme.label())
                                        .selected(self.theme == theme)
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.set_theme(theme, cx)
                                        }))
                                })),
                        ),
                );

        if let Some(message) = &self.settings_error {
            panel = panel.child(
                div()
                    .id("settings-error")
                    .role(Role::Alert)
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(rgb(palette.danger))
                    .text_xs()
                    .child(message.clone()),
            );
        }

        panel.into_any_element()
    }
}
