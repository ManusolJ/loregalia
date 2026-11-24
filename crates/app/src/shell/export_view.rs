use std::path::PathBuf;

use gpui_kit::component::{Disableable, button::Button};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, FontWeight, Role, TestSupportExt, div, px, rgb};
use loregalia_core::{
    dates,
    export::{self, Format},
};

use super::Shell;
use crate::startup::StorageState;
use crate::theme::Palette;

const EXPORT_NOTE: &str = "A backup is a complete copy of your library. JSON and CSV hold your own \
     items, statuses, dates, tags and lists for use in other programs, without the covers and \
     synopses that belong to the sources. API keys are never included.";

const CHOOSING: &str = "Choose where to save it";

pub(super) enum ExportStatus {
    Working,
    Saved(String),
    Failed(String),
}

fn documents_folder() -> PathBuf {
    let home = std::env::home_dir().unwrap_or_default();
    let documents = home.join("Documents");

    if documents.is_dir() { documents } else { home }
}

impl Shell {
    fn export(&mut self, format: Format, cx: &mut Context<Self>) {
        let StorageState::Ready(library) = self.storage.clone() else {
            return;
        };
        if matches!(self.export, Some(ExportStatus::Working)) {
            return;
        }
        self.export = Some(ExportStatus::Working);
        cx.notify();

        let name = format.file_name(&dates::today());
        let chosen = cx.prompt_for_new_path(&documents_folder(), Some(&name));
        cx.spawn(async move |this, cx| {
            let status = match chosen.await {
                Ok(Ok(Some(path))) => {
                    let target = path.clone();
                    let outcome = cx
                        .background_spawn(async move { export::save(&library, format, &target) })
                        .await;
                    Some(match outcome {
                        Ok(saved) => ExportStatus::Saved(format!(
                            "Saved {} to {}",
                            counted(saved.items),
                            saved.path.display()
                        )),
                        Err(error) => ExportStatus::Failed(export::failure_message(&error, &path)),
                    })
                }
                Ok(Ok(None)) => None,
                Ok(Err(_)) | Err(_) => Some(ExportStatus::Failed(
                    "Could not open the save dialog. Try again.".to_owned(),
                )),
            };
            let _ = this.update(cx, |this, cx| {
                if let Some(ExportStatus::Failed(message)) = &status {
                    this.record(message, cx);
                }
                this.export = status;
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn export_section(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let ready = matches!(&self.storage, StorageState::Ready(_));
        let working = matches!(self.export, Some(ExportStatus::Working));
        let message = match &self.export {
            None => None,
            Some(ExportStatus::Working) => Some((CHOOSING.to_owned(), palette.muted, Role::Status)),
            Some(ExportStatus::Saved(message)) => {
                Some((message.clone(), palette.status_done, Role::Status))
            }
            Some(ExportStatus::Failed(message)) => {
                Some((message.clone(), palette.danger, Role::Alert))
            }
        };

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Your library"),
            )
            .child(
                div()
                    .max_w(px(520.0))
                    .text_sm()
                    .text_color(rgb(palette.muted))
                    .child(EXPORT_NOTE),
            )
            .child(div().flex().gap_2().children(Format::ALL.map(|format| {
                Button::new(format!("export-{}", format.key()))
                    .label(format.label())
                    .disabled(!ready || working)
                    .on_click(cx.listener(move |this, _, _, cx| this.export(format, cx)))
            })))
            .children(message.map(|(message, color, role)| {
                div()
                    .id("export-message")
                    .role(role)
                    .test_support()
                    .aria_label(message.clone())
                    .text_xs()
                    .text_color(rgb(color))
                    .child(message)
            }))
            .into_any_element()
    }
}

fn counted(items: usize) -> String {
    match items {
        1 => "1 item".to_owned(),
        items => format!("{items} items"),
    }
}

#[cfg(test)]
mod tests {
    use super::counted;

    #[test]
    fn one_item_is_not_plural() {
        assert_eq!(counted(0), "0 items");
        assert_eq!(counted(1), "1 item");
        assert_eq!(counted(16), "16 items");
    }
}
