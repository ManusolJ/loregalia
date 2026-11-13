use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{Selectable, button::Button};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, Entity, FontWeight, Role, TestSupportExt, Window, div, px, rgb,
};
use loregalia_core::{Database, keys, settings};

use super::metadata_view::{default_details, default_search};
use super::{Screen, Shell};
use crate::startup::StorageState;
use crate::theme::Palette;

pub(super) struct KeyInputs {
    tmdb: Entity<InputState>,
    igdb_client_id: Entity<InputState>,
    igdb_client_secret: Entity<InputState>,
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

impl Shell {
    pub(super) fn begin_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(Screen::Settings);
        self.settings_error = None;
        self.key_inputs = Some(KeyInputs {
            tmdb: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("TMDB API key")
                    .default_value(self.api_keys.tmdb_token.clone().unwrap_or_default())
            }),
            igdb_client_id: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("IGDB Client ID")
                    .default_value(self.api_keys.igdb_client_id.clone().unwrap_or_default())
            }),
            igdb_client_secret: cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder("IGDB Client Secret")
                    .default_value(self.api_keys.igdb_client_secret.clone().unwrap_or_default())
            }),
        });
        cx.notify();
    }

    pub(super) fn save_keys(&mut self, cx: &mut Context<Self>) {
        let keys = {
            let Some(inputs) = &self.key_inputs else {
                return;
            };
            keys::ApiKeys {
                tmdb_token: non_empty(inputs.tmdb.read(cx).value().trim()),
                igdb_client_id: non_empty(inputs.igdb_client_id.read(cx).value().trim()),
                igdb_client_secret: non_empty(inputs.igdb_client_secret.read(cx).value().trim()),
            }
        };
        self.persist_keys(keys, cx);
    }

    pub(super) fn remove_keys(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.key_inputs = Some(KeyInputs {
            tmdb: cx.new(|cx| InputState::new(window, cx).placeholder("TMDB API key")),
            igdb_client_id: cx.new(|cx| InputState::new(window, cx).placeholder("IGDB Client ID")),
            igdb_client_secret: cx
                .new(|cx| InputState::new(window, cx).placeholder("IGDB Client Secret")),
        });
        self.persist_keys(keys::ApiKeys::default(), cx);
    }

    fn persist_keys(&mut self, keys: keys::ApiKeys, cx: &mut Context<Self>) {
        let saved = keys.clone();
        let secrets = self.secrets.clone();
        let task = cx.background_spawn(async move { keys.save(secrets.as_ref()) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(()) => {
                        this.api_keys = saved;
                        this.apply_keys();
                        this.settings_error = None;
                    }
                    Err(error) => {
                        let message = format!("Could not save the keys: {error}");
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

    fn apply_keys(&mut self) {
        let keys = self.api_keys.clone();
        self.metadata_search = default_search(keys.clone());
        self.metadata_details = default_details(keys);
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
                        "Enter your own keys to call each source directly and skip the Loregalia \
                         service. IGDB needs both a client ID and a secret. Leave fields empty to \
                         use the service. Keys are kept in Windows Credential Manager, not in a \
                         file.",
                    ),
            );

        if let Some(inputs) = &self.key_inputs {
            let mut actions = div().flex().gap_2().child(
                Button::new("save-keys")
                    .label("Save keys")
                    .on_click(cx.listener(|this, _, _, cx| this.save_keys(cx))),
            );
            if self.api_keys.tmdb_token.is_some()
                || self.api_keys.igdb_client_id.is_some()
                || self.api_keys.igdb_client_secret.is_some()
            {
                actions = actions.child(
                    Button::new("remove-keys")
                        .label("Remove")
                        .on_click(cx.listener(|this, _, window, cx| this.remove_keys(window, cx))),
                );
            }

            section = section
                .child(
                    Input::new(&inputs.tmdb)
                        .id("tmdb-key-input")
                        .aria_label("TMDB API key"),
                )
                .child(
                    Input::new(&inputs.igdb_client_id)
                        .id("igdb-client-id-input")
                        .aria_label("IGDB Client ID"),
                )
                .child(
                    Input::new(&inputs.igdb_client_secret)
                        .id("igdb-client-secret-input")
                        .aria_label("IGDB Client Secret"),
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
