use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui_kit::component::input::Input;
use gpui_kit::component::{Disableable, button::Button};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, AppContext, Context, Role, TestSupportExt, Window, div, px, rgb};
use loregalia_core::{
    Database, Result as CoreResult, items, log,
    media::MediaKind,
    providers::{self, Candidate, Source},
};

use super::Shell;
use super::items_view::ItemEditor;
use crate::startup::StorageState;
use crate::theme::Palette;

pub type MetadataSearch = Arc<dyn Fn(MediaKind, &str) -> CoreResult<Vec<Candidate>> + Send + Sync>;

pub type MetadataDetails = Arc<dyn Fn(Source, String) -> CoreResult<Candidate> + Send + Sync>;

pub(super) const TMDB_REFRESH_AFTER_DAYS: u64 = 150;

pub(super) fn default_search(tmdb_token: Option<String>) -> MetadataSearch {
    Arc::new(move |kind, query| {
        providers::search_with_key(
            kind,
            query,
            providers::relay::base_url(),
            tmdb_token.as_deref(),
        )
    })
}

pub(super) fn default_details(tmdb_token: Option<String>) -> MetadataDetails {
    Arc::new(move |source, external_id| {
        providers::details_with_key(
            source,
            &external_id,
            providers::relay::base_url(),
            tmdb_token.as_deref(),
        )
    })
}

pub(super) fn candidate_label(candidate: &Candidate, already_added: bool) -> String {
    let label = match candidate.release_year {
        Some(year) => format!("{} ({year})", candidate.title),
        None => candidate.title.clone(),
    };

    if already_added {
        format!("{label} (already added)")
    } else {
        label
    }
}

pub(super) fn refresh_cutoff() -> String {
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(TMDB_REFRESH_AFTER_DAYS * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    log::timestamp(cutoff)
}

#[derive(Default)]
pub(super) enum SearchStatus {
    #[default]
    Idle,
    Searching,
    Empty,
    Failed(String),
    Results(Vec<Candidate>),
}

impl Shell {
    pub(super) fn start_metadata_refresh(&mut self, cx: &mut Context<Self>) {
        if self.refresh_started {
            return;
        }
        self.refresh_started = true;

        let cutoff = refresh_cutoff();
        let due: Vec<(String, String)> = items::due_for_refresh(&self.items, &cutoff)
            .into_iter()
            .filter_map(|item| {
                item.external_id
                    .clone()
                    .map(|external_id| (item.id.clone(), external_id))
            })
            .collect();
        if due.is_empty() {
            return;
        }

        let StorageState::Ready(path) = self.storage.clone() else {
            return;
        };
        let details = self.metadata_details.clone();

        let task = cx.background_spawn(async move {
            let database = Database::open(path)?;
            let mut updated = Vec::new();
            let mut failures = Vec::new();

            for (id, external_id) in due {
                match details(Source::Tmdb, external_id) {
                    Ok(candidate) => {
                        let refreshed_at = log::timestamp(SystemTime::now());
                        if items::apply_refresh(&database, &id, candidate.into(), &refreshed_at)?
                            && let Some(item) = items::get(&database, &id)?
                        {
                            updated.push(item);
                        }
                    }
                    Err(error) => failures.push(error.to_string()),
                }
            }

            Ok::<_, loregalia_core::Error>((updated, failures))
        });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((updated, failures)) => {
                        for item in updated {
                            if let Some(slot) =
                                this.items.iter_mut().find(|slot| slot.id == item.id)
                            {
                                *slot = item;
                            }
                        }
                        for failure in failures {
                            this.record(&format!("Metadata refresh failed: {failure}"), cx);
                        }
                    }
                    Err(error) => {
                        this.record(&format!("Metadata refresh failed: {error}"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn search_metadata(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &mut self.item_editor else {
            return;
        };
        if editor.id.is_some() || !providers::supported(editor.kind) {
            return;
        }
        let Some(input) = &editor.search_input else {
            return;
        };
        let query = input.read(cx).value().trim().to_owned();
        if query.is_empty() || matches!(editor.search, SearchStatus::Searching) {
            return;
        }

        editor.search_request += 1;
        let request = editor.search_request;
        let kind = editor.kind;
        editor.search = SearchStatus::Searching;
        let search = self.metadata_search.clone();
        cx.notify();

        let task = cx.background_spawn(async move { search(kind, &query) });

        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                let Some(editor) = &mut this.item_editor else {
                    return;
                };
                if editor.search_request != request || editor.kind != kind {
                    return;
                }

                let mut failure = None;
                editor.search = match result {
                    Ok(results) if results.is_empty() => SearchStatus::Empty,
                    Ok(results) => SearchStatus::Results(results),
                    Err(error) => {
                        let message = error.to_string();
                        failure = Some(message.clone());
                        SearchStatus::Failed(message)
                    }
                };

                if let Some(message) = failure {
                    this.record(&format!("Metadata search failed: {message}"), cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn select_search_result(
        &mut self,
        source: Source,
        external_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = &mut self.item_editor else {
            return;
        };
        let results = match &editor.search {
            SearchStatus::Results(results) => results,
            _ => return,
        };
        let Some(candidate) = results
            .iter()
            .find(|candidate| candidate.source == source && candidate.external_id == external_id)
            .cloned()
        else {
            return;
        };

        editor.lookup = Some(items::Lookup {
            source: candidate.source,
            external_id: candidate.external_id.clone(),
            cover_url: candidate.cover_url.clone(),
        });

        let title = candidate.title;
        let year = candidate
            .release_year
            .map(|year| year.to_string())
            .unwrap_or_default();
        editor
            .title
            .update(cx, |state, cx| state.set_value(title, window, cx));
        editor
            .year
            .update(cx, |state, cx| state.set_value(year, window, cx));

        cx.notify();
    }

    pub(super) fn has_candidate(&self, candidate: &Candidate) -> bool {
        self.items.iter().any(|item| {
            item.source.as_deref() == Some(candidate.source.key())
                && item.external_id.as_deref() == Some(candidate.external_id.as_str())
        })
    }

    pub(super) fn metadata_search_element(
        &self,
        editor: &ItemEditor,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(input) = &editor.search_input else {
            return div().into_any_element();
        };
        let query = input.read(cx).value().trim().to_owned();
        let searching = matches!(editor.search, SearchStatus::Searching);
        let source_label = providers::source(editor.kind)
            .map(Source::label)
            .unwrap_or("metadata");

        let mut panel = div()
            .id("metadata-search")
            .test_support()
            .aria_label("Search metadata")
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted))
                    .child(format!("Results from {source_label}")),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Input::new(input)
                            .id("metadata-search-input")
                            .aria_label("Search by title"),
                    )
                    .child(
                        Button::new("search-metadata")
                            .label("Search")
                            .disabled(query.is_empty() || searching)
                            .on_click(cx.listener(|this, _, _, cx| this.search_metadata(cx))),
                    ),
            );

        panel = match &editor.search {
            SearchStatus::Idle => panel,
            SearchStatus::Searching => panel.child(
                div()
                    .id("metadata-search-loading")
                    .role(Role::Status)
                    .test_support()
                    .aria_label(format!("Searching {source_label}"))
                    .text_color(rgb(palette.muted))
                    .text_xs()
                    .child(format!("Searching {source_label}")),
            ),
            SearchStatus::Empty => panel.child(
                div()
                    .id("metadata-search-empty")
                    .role(Role::Status)
                    .test_support()
                    .aria_label(format!("No matches on {source_label}"))
                    .text_color(rgb(palette.muted))
                    .text_xs()
                    .child(format!("No matches on {source_label}")),
            ),
            SearchStatus::Failed(message) => panel.child(
                div()
                    .id("metadata-search-failed")
                    .role(Role::Alert)
                    .test_support()
                    .aria_label(message.clone())
                    .text_color(rgb(palette.danger))
                    .text_xs()
                    .child(message.clone()),
            ),
            SearchStatus::Results(results) => {
                let results = results.iter().map(|candidate| {
                    let already_added = self.has_candidate(candidate);
                    let label = candidate_label(candidate, already_added);
                    let source = candidate.source;
                    let external_id = candidate.external_id.clone();
                    let id = format!("search-result-{}-{}", source.key(), external_id);

                    Button::new(id)
                        .label(label)
                        .disabled(already_added)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_search_result(source, external_id.clone(), window, cx)
                        }))
                });

                panel.child(
                    div()
                        .id("metadata-search-results")
                        .test_support()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .max_h(px(240.0))
                        .overflow_y_scroll()
                        .children(results),
                )
            }
        };

        panel.into_any_element()
    }
}
