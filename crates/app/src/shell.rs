use std::future::Future;
use std::path::PathBuf;

use gpui_kit::component::{Disableable, button::Button};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, FontWeight, Pixels, Role, TestSupportExt, Window, div, px, rgb,
};
use loregalia_core::{Database, Result as CoreResult, lists, log};

use crate::startup::StorageState;
use crate::theme::Palette;

const RAIL_WIDTH: f32 = 56.0;
const SIDEBAR_WIDTH: f32 = 240.0;
const SIDEBAR_LABELS_FROM: f32 = 1024.0;
const EMPTY_MESSAGE: &str = "Nothing planned yet";
const NEW_LIST_NAME: &str = "New list";

pub fn sidebar_width(window_width: Pixels) -> Pixels {
    if window_width < px(SIDEBAR_LABELS_FROM) {
        px(RAIL_WIDTH)
    } else {
        px(SIDEBAR_WIDTH)
    }
}

pub struct Shell {
    storage: StorageState,
    log_path: Option<PathBuf>,
    lists: Vec<lists::List>,
    error: Option<String>,
    loading: bool,
}

impl Shell {
    pub fn new(storage: StorageState, log_path: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let mut shell = Self {
            storage,
            log_path,
            lists: Vec::new(),
            error: None,
            loading: true,
        };
        shell.reload_lists(cx);
        shell
    }

    fn record(&self, message: &str) {
        if let Some(path) = &self.log_path {
            let _ = log::append(path, message);
        }
    }

    fn reload_lists(&mut self, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            lists::all(&database)
        });
    }

    fn create_list(&mut self, cx: &mut Context<Self>) {
        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };

        self.run(cx, async move {
            let database = Database::open(path)?;
            lists::create(&database, NEW_LIST_NAME)?;
            lists::all(&database)
        });
    }

    fn run<F>(&mut self, cx: &mut Context<Self>, work: F)
    where
        F: Future<Output = CoreResult<Vec<lists::List>>> + Send + 'static,
    {
        self.loading = true;
        let task = cx.background_spawn(work);

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(lists) => {
                        this.lists = lists;
                        this.error = None;
                    }
                    Err(error) => {
                        let message = format!("Could not update lists: {error}");
                        this.record(&message);
                        this.error = Some(message);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn main_element(&self, palette: &Palette) -> AnyElement {
        match (&self.storage, &self.error) {
            (StorageState::Failed(message), _) => div()
                .id("storage-error")
                .role(Role::Alert)
                .test_support()
                .aria_label(message.clone())
                .text_color(rgb(palette.danger))
                .text_xs()
                .child(message.clone())
                .into_any_element(),
            (StorageState::Ready(_), Some(message)) => div()
                .id("lists-error")
                .role(Role::Alert)
                .test_support()
                .aria_label(message.clone())
                .text_color(rgb(palette.danger))
                .text_xs()
                .child(message.clone())
                .into_any_element(),
            (StorageState::Ready(_), None) => div()
                .id("empty-state")
                .role(Role::Status)
                .test_support()
                .aria_label(EMPTY_MESSAGE)
                .text_color(rgb(palette.muted))
                .child(EMPTY_MESSAGE)
                .into_any_element(),
        }
    }

    fn list_rows(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .children(self.lists.iter().map(|list| {
                div()
                    .id(format!("list-row-{}", list.id))
                    .role(Role::ListItem)
                    .test_support()
                    .aria_label(list.name.clone())
                    .py_2()
                    .child(list.name.clone())
            }))
    }
}

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *cx.global::<Palette>();
        let width = sidebar_width(window.viewport_size().width);
        let shows_labels = width > px(RAIL_WIDTH);
        let can_create = matches!(&self.storage, StorageState::Ready(_));
        let main = self.main_element(&palette);

        div()
            .size_full()
            .flex()
            .bg(rgb(palette.bg))
            .text_color(rgb(palette.content))
            .text_sm()
            .child(
                div()
                    .id("sidebar")
                    .test_support()
                    .w(width)
                    .h_full()
                    .flex_none()
                    .p_3()
                    .bg(rgb(palette.surface))
                    .border_r_1()
                    .border_color(rgb(palette.line))
                    .when(shows_labels, |sidebar| {
                        let sidebar = sidebar
                            .child(
                                div()
                                    .text_color(rgb(palette.muted))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Lists"),
                            )
                            .child(div().mt_3().child(self.list_rows()));

                        if can_create {
                            sidebar.child(
                                div().mt_3().child(
                                    Button::new("create-list")
                                        .label("Create list")
                                        .disabled(self.loading)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.create_list(cx)),
                                        ),
                                ),
                            )
                        } else {
                            sidebar
                        }
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(main),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;

    use super::sidebar_width;

    #[test]
    fn sidebar_becomes_a_rail_below_1024() {
        assert_eq!(sidebar_width(px(960.0)), px(56.0));
        assert_eq!(sidebar_width(px(1023.0)), px(56.0));
        assert_eq!(sidebar_width(px(1024.0)), px(240.0));
        assert_eq!(sidebar_width(px(1280.0)), px(240.0));
    }
}
