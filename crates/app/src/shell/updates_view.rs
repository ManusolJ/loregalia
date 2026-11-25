use std::sync::Arc;
use std::time::SystemTime;

use gpui_kit::assets::IconName;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable, button::Button, button::ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, FontWeight, Role, TestSupportExt, div, px, rgb};
use loregalia_core::{
    Database, Result as CoreResult, settings,
    updates::{self, Known, RELEASES_PAGE, Version},
};

use super::{NavRow, Shell};
use crate::startup::StorageState;
use crate::theme::Palette;

pub type UpdateCheck = Arc<dyn Fn() -> CoreResult<Option<Version>> + Send + Sync>;

const UPDATE_NOTE: &str = "At most once a day, when it starts, Loregalia asks GitHub for the \
     newest version. GitHub sees your IP address and the version you have. Nothing is downloaded or \
     installed for you.";
const CHECKING: &str = "Checking for updates";
const UP_TO_DATE: &str = "You have the newest version.";
const CHECK_FAILED: &str = "Could not check for updates. Try again later.";
const SWITCH_LABEL: &str = "Check for updates once a day";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CheckStatus {
    Working,
    Done,
    Failed,
}

#[derive(Default)]
pub(super) struct UpdateState {
    check: Option<UpdateCheck>,
    enabled: bool,
    latest: Option<Version>,
    status: Option<CheckStatus>,
    asked_by_hand: bool,
}

impl UpdateState {
    fn available(&self) -> Option<Version> {
        self.latest
            .filter(|version| version.is_newer_than_this_one())
    }
}

fn available_label(version: Version) -> String {
    format!("Version {version} is available")
}

impl Shell {
    pub fn check_for_updates(&mut self, check: UpdateCheck, cx: &mut Context<Self>) {
        self.updates.check = Some(check);
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        let task = cx.background_spawn(async move {
            let database = Database::open(path)?;
            updates::known(&database, SystemTime::now())
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(known) => this.use_known_updates(known, cx),
                Err(error) => {
                    this.record(&format!("Could not read the update settings: {error}"), cx);
                }
            });
        })
        .detach();
    }

    fn use_known_updates(&mut self, known: Known, cx: &mut Context<Self>) {
        self.updates.enabled = known.enabled;
        self.updates.latest = known.latest;
        if known.enabled && known.due {
            self.ask_for_updates(cx);
        }
        cx.notify();
    }

    fn ask_for_updates(&mut self, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        let Some(check) = self.updates.check.clone() else {
            return;
        };
        if self.updates.status == Some(CheckStatus::Working) {
            return;
        }
        self.updates.status = Some(CheckStatus::Working);
        cx.notify();

        let task = cx.background_spawn(async move {
            let latest = check()?;
            let database = Database::open(path)?;
            updates::remember(&database, latest, SystemTime::now())?;
            CoreResult::Ok(latest)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(latest) => {
                        this.updates.latest = latest;
                        this.updates.status = Some(CheckStatus::Done);
                    }
                    Err(error) => {
                        this.record(&format!("Could not check for updates: {error}"), cx);
                        this.updates.status = Some(CheckStatus::Failed);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn check_updates_now(&mut self, cx: &mut Context<Self>) {
        self.updates.asked_by_hand = true;
        self.ask_for_updates(cx);
    }

    fn set_update_checks(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.updates.enabled = enabled;
        self.settings_error = None;
        cx.notify();
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        let task = cx.background_spawn(async move {
            let database = Database::open(path)?;
            settings::set_update_checks(&database, enabled)
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                if let Err(error) = result {
                    let message = format!("Could not save the update setting: {error}");
                    this.record(&message, cx);
                    this.settings_error = Some(message);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn forget_update_status(&mut self) {
        self.updates.asked_by_hand = false;
    }

    pub(super) fn update_row(
        &self,
        shows_labels: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.updates.enabled {
            return None;
        }
        let version = self.updates.available()?;

        Some(Self::nav_row(
            NavRow {
                id: "update-available".to_owned(),
                icon: IconName::Download,
                label: available_label(version),
                count: None,
                selected: false,
            },
            shows_labels,
            palette,
            |_, _, cx| cx.open_url(RELEASES_PAGE),
            cx,
        ))
    }

    pub(super) fn updates_section(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let ready = matches!(&self.storage, StorageState::Ready(_));
        let working = self.updates.status == Some(CheckStatus::Working);
        let available = self.updates.available();

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Updates"),
            )
            .child(
                div()
                    .max_w(px(520.0))
                    .text_sm()
                    .text_color(rgb(palette.muted))
                    .child(UPDATE_NOTE),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        Switch::new("update-checks")
                            .label(SWITCH_LABEL)
                            .checked(self.updates.enabled)
                            .disabled(!ready)
                            .on_change(cx.listener(|this, enabled: &bool, _, cx| {
                                this.set_update_checks(*enabled, cx)
                            })),
                    )
                    .child(
                        Button::new("check-updates")
                            .label("Check now")
                            .disabled(!ready || working)
                            .on_click(cx.listener(|this, _, _, cx| this.check_updates_now(cx))),
                    )
                    .children(available.map(|_| {
                        Button::new("get-update")
                            .primary()
                            .icon(IconName::Download)
                            .label("Open the download page")
                            .on_click(|_, _, cx| cx.open_url(RELEASES_PAGE))
                    })),
            )
            .children(
                self.update_message(available, palette)
                    .map(|(message, color, role)| {
                        div()
                            .id("update-message")
                            .role(role)
                            .test_support()
                            .aria_label(message.clone())
                            .text_xs()
                            .text_color(rgb(color))
                            .child(message)
                    }),
            )
            .into_any_element()
    }

    fn update_message(
        &self,
        available: Option<Version>,
        palette: &Palette,
    ) -> Option<(String, u32, Role)> {
        let by_hand = self
            .updates
            .asked_by_hand
            .then_some(self.updates.status)
            .flatten();
        match (by_hand, available) {
            (Some(CheckStatus::Working), _) => {
                Some((CHECKING.to_owned(), palette.muted, Role::Status))
            }
            (Some(CheckStatus::Failed), _) => {
                Some((CHECK_FAILED.to_owned(), palette.danger, Role::Alert))
            }
            (_, Some(version)) => Some((available_label(version), palette.accent, Role::Status)),
            (Some(CheckStatus::Done), None) => {
                Some((UP_TO_DATE.to_owned(), palette.status_done, Role::Status))
            }
            (None, None) => None,
        }
    }
}
