use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Selectable, button::Button};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, FontWeight, Role, TestSupportExt, Window, div, px, rgb,
};
use loregalia_core::{Database, keys, settings};

use super::metadata_view::{default_details, default_search};
use super::{Screen, Shell};
use crate::startup::StorageState;
use crate::theme::Palette;

impl Shell {
    pub(super) fn begin_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(Screen::Settings);
        self.settings_error = None;
        let value = self.tmdb_token.clone().unwrap_or_default();
        self.tmdb_input = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("TMDB API key")
                .default_value(value)
        }));
        cx.notify();
    }

    pub(super) fn save_tmdb_key(&mut self, cx: &mut Context<Self>) {
        let Some(input) = &self.tmdb_input else {
            return;
        };
        let value = input.read(cx).value().trim().to_owned();
        let token = if value.is_empty() { None } else { Some(value) };
        self.persist_tmdb_key(token, cx);
    }

    pub(super) fn remove_tmdb_key(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.tmdb_input =
            Some(cx.new(|cx| InputState::new(window, cx).placeholder("TMDB API key")));
        self.persist_tmdb_key(None, cx);
    }

    fn persist_tmdb_key(&mut self, token: Option<String>, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            self.settings_error = Some("Keys need a working data folder.".to_owned());
            cx.notify();
            return;
        };
        let Some(keys_path) = keys::file_for_database(&path) else {
            self.settings_error = Some("Could not locate the keys file.".to_owned());
            cx.notify();
            return;
        };

        let saved = token.clone();
        let task = cx
            .background_spawn(async move { keys::ApiKeys { tmdb_token: token }.save(&keys_path) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.tmdb_token = saved;
                        this.apply_tmdb_token();
                        this.settings_error = None;
                    }
                    Err(error) => {
                        let message = format!("Could not save the TMDB key: {error}");
                        this.record(&message, cx);
                        this.settings_error = Some(message);
                    }
                }
                cx.notify();
            });
        })
        .detach();

        cx.notify();
    }

    fn apply_tmdb_token(&mut self) {
        let token = self.tmdb_token.clone();
        self.metadata_search = default_search(token.clone());
        self.metadata_details = default_details(token);
    }

    fn keys_section(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let mut section = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Metadata keys"),
            )
            .child(
                div()
                    .max_w(px(520.0))
                    .text_sm()
                    .text_color(rgb(palette.muted))
                    .child(
                        "Enter your own TMDB key to call TMDB directly and skip the Loregalia \
                         service. Leave it empty to use the service.",
                    ),
            );

        if let Some(input) = &self.tmdb_input {
            let mut actions = div().flex().gap_2().child(
                Button::new("save-tmdb-key")
                    .label("Save key")
                    .on_click(cx.listener(|this, _, _, cx| this.save_tmdb_key(cx))),
            );
            if self.tmdb_token.is_some() {
                actions =
                    actions.child(Button::new("remove-tmdb-key").label("Remove").on_click(
                        cx.listener(|this, _, window, cx| this.remove_tmdb_key(window, cx)),
                    ));
            }

            section = section
                .child(
                    Input::new(input)
                        .id("tmdb-key-input")
                        .aria_label("TMDB API key"),
                )
                .child(actions);
        }

        section.into_any_element()
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
                )
                .child(self.keys_section(palette, cx));

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
