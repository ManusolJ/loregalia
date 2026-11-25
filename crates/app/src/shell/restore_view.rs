use std::path::PathBuf;

use gpui_kit::component::{Disableable, button::Button};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, PathPromptOptions, Role, TestSupportExt, div, rgb};
use loregalia_core::restore::{self, Contents};

use super::export_view::{TransferStatus, counted};
use super::widgets::Confirmation;
use super::{Screen, Shell};
use crate::startup::StorageState;
use crate::theme::Palette;

const CHOOSING: &str = "Choose a backup";
const RESTORING: &str = "Restoring your library";
const DIALOG_FAILED: &str = "Could not open the file dialog. Try again.";

const REPLACE_LIBRARY: Confirmation = Confirmation {
    id: "restore-confirmation",
    cancel_id: "cancel-restore",
    confirm_id: "confirm-restore",
    confirm_label: "Restore",
    consequence: "The backup replaces everything in your library. A copy of the library you have \
                  now is kept in the data folder. Settings and API keys stay as they are.",
};

const REPLACE_DAMAGED: Confirmation = Confirmation {
    consequence: "The backup takes the place of the damaged file, which is kept beside it under \
                  another name.",
    ..REPLACE_LIBRARY
};

pub(super) struct PendingRestore {
    backup: PathBuf,
    contents: Contents,
}

fn described(contents: &Contents) -> String {
    format!(
        "{} and {}",
        counted(contents.items, "item"),
        counted(contents.lists, "list")
    )
}

impl Shell {
    pub fn offer_restore(&mut self, damaged_library: Option<PathBuf>) {
        self.damaged_library = damaged_library;
    }

    fn restore_target(&self) -> Option<PathBuf> {
        match &self.storage {
            StorageState::Ready(library) => Some(library.clone()),
            StorageState::Failed(_) => self.damaged_library.clone(),
        }
    }

    fn can_confirm_restore(&self) -> bool {
        self.screen == Screen::Settings || matches!(&self.storage, StorageState::Failed(_))
    }

    fn begin_restore(&mut self, cx: &mut Context<Self>) {
        let Some(library) = self.restore_target() else {
            return;
        };
        if matches!(self.transfer, Some(TransferStatus::Working(_))) {
            return;
        }
        self.transfer = Some(TransferStatus::Working(CHOOSING));
        cx.notify();

        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(CHOOSING.into()),
        });
        cx.spawn(async move |this, cx| {
            let chosen = match chosen.await {
                Ok(Ok(paths)) => Ok(paths.and_then(|paths| paths.into_iter().next())),
                Ok(Err(_)) | Err(_) => Err(DIALOG_FAILED.to_owned()),
            };
            let found = match chosen {
                Ok(Some(backup)) => {
                    let inspected = backup.clone();
                    cx.background_spawn(async move { restore::inspect(&library, &inspected) })
                        .await
                        .map_err(|error| restore::failure_message(&error, &backup))
                        .map(|contents| Some(PendingRestore { backup, contents }))
                }
                Ok(None) => Ok(None),
                Err(message) => Err(message),
            };
            let _ = this.update(cx, |this, cx| {
                this.transfer = None;
                match found {
                    Ok(pending) if this.can_confirm_restore() => this.pending_restore = pending,
                    Ok(_) => {}
                    Err(message) => this.restore_failed(message, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn restore_failed(&mut self, message: String, cx: &mut Context<Self>) {
        self.record(&message, cx);
        self.transfer = Some(TransferStatus::Failed(message));
    }

    fn cancel_restore(&mut self, cx: &mut Context<Self>) {
        self.pending_restore = None;
        cx.notify();
    }

    fn confirm_restore(&mut self, cx: &mut Context<Self>) {
        let (Some(library), Some(pending)) = (self.restore_target(), self.pending_restore.take())
        else {
            return;
        };
        self.transfer = Some(TransferStatus::Working(RESTORING));
        cx.notify();

        let backup = pending.backup;
        cx.spawn(async move |this, cx| {
            let chosen = backup.clone();
            let outcome = cx
                .background_spawn(async move { restore::restore(&library, &chosen) })
                .await;
            let _ = this.update(cx, |this, cx| {
                match outcome {
                    Ok(restored) => {
                        let kept = restored.previous.display().to_string();
                        let restart = if this.damaged_library.is_some() {
                            "Close Loregalia and start it again to open it. "
                        } else {
                            ""
                        };
                        let message = format!(
                            "Restored {}. {restart}The library you had is kept as {kept}",
                            described(&restored.contents)
                        );
                        this.record(&message, cx);
                        this.transfer = Some(TransferStatus::Done(message));
                        this.close_editors();
                        this.reload(cx);
                    }
                    Err(error) => {
                        this.restore_failed(restore::failure_message(&error, &backup), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn restore_button(&self, cx: &mut Context<Self>) -> Button {
        let working = matches!(self.transfer, Some(TransferStatus::Working(_)));

        Button::new("restore-backup")
            .label("Restore a backup")
            .disabled(working || self.restore_target().is_none())
            .on_click(cx.listener(|this, _, _, cx| this.begin_restore(cx)))
    }

    pub(super) fn confirm_restore_element(
        &self,
        pending: &PendingRestore,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let name = pending
            .backup
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let found = described(&pending.contents);
        let current = Contents {
            items: self.items.len(),
            lists: self.lists.len(),
        };
        let (confirmation, message) = match &self.storage {
            StorageState::Ready(_) => (
                REPLACE_LIBRARY,
                format!(
                    "Replace your {} with the {found} in \"{name}\"?",
                    described(&current)
                ),
            ),
            StorageState::Failed(_) => {
                (REPLACE_DAMAGED, format!("Restore {found} from \"{name}\"?"))
            }
        };

        Self::confirm_element(
            confirmation,
            message,
            Self::cancel_restore,
            Self::confirm_restore,
            palette,
            cx,
        )
    }

    pub(super) fn storage_failed_element(
        &self,
        message: &str,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(pending) = &self.pending_restore {
            return self.confirm_restore_element(pending, palette, cx);
        }
        let restored = matches!(self.transfer, Some(TransferStatus::Done(_)));
        let action = self.damaged_library.is_some().then(|| {
            if restored {
                Button::new("close-app")
                    .label("Close Loregalia")
                    .on_click(|_, _, cx| cx.quit())
            } else {
                self.restore_button(cx)
            }
        });

        div()
            .flex()
            .flex_col()
            .items_start()
            .gap_3()
            .when(!restored, |panel| {
                panel.child(
                    div()
                        .id("storage-error")
                        .role(Role::Alert)
                        .test_support()
                        .aria_label(message.to_owned())
                        .text_color(rgb(palette.danger))
                        .text_xs()
                        .child(message.to_owned()),
                )
            })
            .children(self.transfer_message(palette))
            .children(action)
            .into_any_element()
    }
}
