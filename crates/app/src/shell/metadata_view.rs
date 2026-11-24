use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use gpui_kit::assets::IconName;
use gpui_kit::component::input::Input;
use gpui_kit::component::{
    Disableable, Icon, Selectable, Sizable, button::Button, button::ButtonVariants,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AppContext, Context, FontWeight, Role, TestSupportExt, Window, div, px, rgb,
};
use loregalia_core::{
    Database, Error, Result as CoreResult, items, keys, log,
    media::MediaKind,
    providers::{self, Candidate, Source},
};

use super::Shell;
use super::items_view::ItemEditor;
use super::widgets::{Cover, ROW_COVER_WIDTH};
use crate::startup::StorageState;
use crate::theme::Palette;

pub type MetadataSearch = Arc<dyn Fn(MediaKind, &str) -> CoreResult<Vec<Candidate>> + Send + Sync>;

pub type MetadataDetails = Arc<dyn Fn(Source, String) -> CoreResult<Candidate> + Send + Sync>;

const RESULT_HEIGHT: f32 = 72.0;
const MAX_REFRESH_FAILURES: usize = 3;

pub const REFRESH_PACE: Duration = Duration::from_secs(4);

pub type MetadataSeasons = Arc<dyn Fn(&str) -> CoreResult<Vec<i32>> + Send + Sync>;

pub(super) const TMDB_REFRESH_AFTER_DAYS: u64 = 150;

pub(super) fn default_search(keys: keys::ApiKeys) -> MetadataSearch {
    Arc::new(move |kind, query| {
        providers::search_with_key(
            kind,
            query,
            providers::relay::base_url(),
            keys.tmdb_token(),
            keys.igdb_credentials(),
        )
    })
}

pub(super) fn default_details(keys: keys::ApiKeys) -> MetadataDetails {
    Arc::new(move |source, external_id| {
        providers::details_with_key(
            source,
            &external_id,
            providers::relay::base_url(),
            keys.tmdb_token(),
            keys.igdb_credentials(),
        )
    })
}

pub(super) fn candidate_label(candidate: &Candidate, already_added: bool) -> String {
    let year = candidate.release_year.map(|year| year.to_string());
    let facts: Vec<String> = year.into_iter().chain(candidate.edition.clone()).collect();
    let label = if facts.is_empty() {
        candidate.title.clone()
    } else {
        format!("{} ({})", candidate.title, facts.join(", "))
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
pub(super) struct RefreshState {
    running: bool,
    stopped: bool,
    failures: usize,
    attempted: HashSet<String>,
}

fn refresh_item(
    path: &Path,
    due: &items::Due,
    details: &MetadataDetails,
) -> CoreResult<Option<items::Item>> {
    let found = details(due.source, due.external_id.clone());
    let database = Database::open(path)?;
    match found {
        Ok(candidate) => {
            let refreshed_at = log::timestamp(SystemTime::now());
            items::apply_refresh(&database, &due.id, candidate.into(), &refreshed_at)?;
        }
        Err(Error::HttpStatus { code: 404 }) => {
            items::mark_without_details(&database, &due.id)?;
        }
        Err(error) => return Err(error),
    }
    items::get(&database, &due.id)
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
        if self.refresh.running || self.refresh.stopped || self.next_refresh().is_none() {
            return;
        }
        self.refresh.running = true;

        cx.spawn(async move |this, cx| {
            while let Ok(Some((path, due))) = this.update(cx, |this, _| this.take_refresh()) {
                let Ok(details) = this.update(cx, |this, _| this.metadata_details.clone()) else {
                    return;
                };
                let outcome = cx
                    .background_spawn(async move { refresh_item(&path, &due, &details) })
                    .await;
                let resume = this.update(cx, |this, cx| this.finish_refresh(outcome, cx));
                if !matches!(resume, Ok(true)) {
                    break;
                }
                cx.background_executor().timer(REFRESH_PACE).await;
            }
            let _ = this.update(cx, |this, _| this.refresh.running = false);
        })
        .detach();
    }

    pub(super) fn resume_metadata_refresh(&mut self) {
        self.refresh.stopped = false;
        self.refresh.failures = 0;
        self.refresh.attempted.clear();
    }

    fn next_refresh(&self) -> Option<(PathBuf, items::Due)> {
        let StorageState::Ready(path) = &self.storage else {
            return None;
        };
        let due = items::next_due(&self.items, &refresh_cutoff(), &self.refresh.attempted)?;

        Some((path.clone(), due))
    }

    fn take_refresh(&mut self) -> Option<(PathBuf, items::Due)> {
        let next = self.next_refresh()?;
        self.refresh.attempted.insert(next.1.id.clone());
        Some(next)
    }

    fn finish_refresh(
        &mut self,
        outcome: CoreResult<Option<items::Item>>,
        cx: &mut Context<Self>,
    ) -> bool {
        match outcome {
            Ok(updated) => {
                self.refresh.failures = 0;
                self.reloads.interrupt();
                if let Some(item) = updated
                    && let Some(index) = self.item_index.get(&item.id)
                    && let Some(slot) = self.items.get_mut(*index)
                {
                    *slot = item;
                }
                cx.notify();
            }
            Err(error) => {
                self.record(&format!("Metadata refresh failed: {error}"), cx);
                self.refresh.failures += 1;
                if matches!(error, Error::RateLimited)
                    || self.refresh.failures >= MAX_REFRESH_FAILURES
                {
                    self.refresh.stopped = true;
                }
            }
        }
        !self.refresh.stopped
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
        let route = providers::route(
            kind,
            self.api_keys.tmdb_token(),
            self.api_keys.igdb_credentials(),
        );
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
                        let shown = match providers::source(kind) {
                            Some(source) => providers::failure_message(&error, source, route),
                            None => error.to_string(),
                        };
                        failure = Some(error.to_string());
                        SearchStatus::Failed(shown)
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

    fn candidate_row(
        candidate: &Candidate,
        kind: MediaKind,
        already_added: bool,
        chosen: bool,
        palette: &Palette,
    ) -> AnyElement {
        let year = candidate
            .release_year
            .map_or_else(|| "Year unknown".to_owned(), |year| year.to_string());
        let facts = match &candidate.edition {
            Some(edition) => format!("{year} · {edition}"),
            None => year,
        };

        div()
            .w_full()
            .flex()
            .items_center()
            .gap_3()
            .child(Self::cover_element(
                Cover {
                    prefix: "search-cover",
                    key: format!("{}-{}", candidate.source.key(), candidate.external_id),
                    title: &candidate.title,
                    kind,
                    url: candidate.cover_url.as_deref(),
                    width: ROW_COVER_WIDTH,
                },
                palette,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap_0p5()
                    .child(
                        div()
                            .w_full()
                            .truncate()
                            .text_color(rgb(palette.content))
                            .font_weight(FontWeight::MEDIUM)
                            .child(candidate.title.clone()),
                    )
                    .child(div().text_xs().text_color(rgb(palette.muted)).child(facts)),
            )
            .when(already_added, |row| {
                row.child(
                    div()
                        .flex_none()
                        .text_xs()
                        .text_color(rgb(palette.muted))
                        .child("In your library"),
                )
            })
            .when(chosen, |row| {
                row.child(
                    div()
                        .id(format!(
                            "search-chosen-{}-{}",
                            candidate.source.key(),
                            candidate.external_id
                        ))
                        .test_support()
                        .flex_none()
                        .child(
                            Icon::new(IconName::Check)
                                .with_size(px(18.0))
                                .text_color(rgb(palette.accent)),
                        ),
                )
            })
            .into_any_element()
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
                    .flex()
                    .gap_2()
                    .child(
                        Input::new(input)
                            .id("metadata-search-input")
                            .aria_label("Search by title"),
                    )
                    .child(
                        Button::new("search-metadata")
                            .primary()
                            .icon(IconName::Search)
                            .label("Search")
                            .disabled(query.is_empty() || searching)
                            .on_click(cx.listener(|this, _, _, cx| this.search_metadata(cx))),
                    ),
            );

        panel = match &editor.search {
            SearchStatus::Idle => panel.child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted))
                    .child(format!("Searches {source_label} and fills in the rest.")),
            ),
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
                    let chosen = editor.lookup.as_ref().is_some_and(|lookup| {
                        lookup.source == source && lookup.external_id == external_id
                    });
                    let content =
                        Self::candidate_row(candidate, editor.kind, already_added, chosen, palette);

                    Button::new(id)
                        .disabled(already_added)
                        .accessibility_label(label)
                        .ghost()
                        .selected(chosen)
                        .toggled(chosen)
                        .w_full()
                        .h(px(RESULT_HEIGHT))
                        .flex_none()
                        .px_2()
                        .justify_start()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.select_search_result(source, external_id.clone(), window, cx)
                        }))
                        .child(content)
                });

                panel
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted))
                            .child(format!("Results from {source_label}")),
                    )
                    .child(
                        div()
                            .id("metadata-search-results")
                            .test_support()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .p_1()
                            .max_h(px(300.0))
                            .overflow_y_scroll()
                            .rounded(px(8.0))
                            .border_1()
                            .border_color(rgb(palette.line))
                            .bg(rgb(palette.surface))
                            .children(results),
                    )
            }
        };

        panel.into_any_element()
    }
}
