# Decisions

Agents: if a task depends on an **Open** decision, stop and ask. Do not choose. When a human decides
one, move it to Decided, with the date.

## Decided

| ID   | Decision                                                                 | Why                                                                                     | Date       |
| ---- | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------- | ---------- |
| A-01 | Use `gpui-kit` 0.9.0, pinned exactly, not crates.io `gpui`               | Recent, runs UI tests on Windows, one dependency; crates.io `gpui` dates from Oct 2025  | 2026-11-05 |
| A-02 | Start a fresh Git repo with no remote                                    | The Tauri history in `loregalia-old` shares no code with this project                   | 2026-11-05 |
| A-03 | Agents commit only on `agent/*`; they never push or merge                | Keeps a history of attempts, and keeps review and `main` with the human                 | 2026-11-05 |
| A-04 | Windows ships; Linux only has to build and pass tests                    | Agents work in a Linux sandbox, but only Windows gets an installer and manual testing   | 2026-11-05 |
| A-05 | Agent-first development, replacing the earlier "the user writes the code" | Project direction                                                                       | 2026-11-05 |
| D-01 | `rusqlite` with `bundled` for SQLite                                     | Simple, synchronous and run on background tasks; no system SQLite needed                | 2026-11-05 |
| D-02 | UUIDs identify items, lists and tags                                     | Sync is a possible future, and changing keys later touches every table                  | 2026-11-05 |
| D-03 | A season is its own row, a child of the show, with its own status        | Lets you plan one season at a time                                                      | 2026-11-05 |
| D-04 | Five status categories, with labels per media kind                       | Labels read naturally: To watch, To read, To play                                       | 2026-11-05 |
| D-05 | Release dates are an optional year plus an optional full date            | Covers books with only a year and unannounced games with nothing                        | 2026-11-05 |
| D-06 | Hand-typed duplicates get a warning but are allowed                      | Remakes and same-named books are real; lookups stay unique by source and external ID    | 2026-11-05 |
| D-07 | Lists have a manual order, with sorting as a temporary view              | A list can act as a personal queue of what to pick up next                              | 2026-11-05 |
| D-08 | Deleting is final, after confirmation; memberships cascade               | Simple, and fits a small personal app                                                   | 2026-11-05 |

| D-17 | Use `ManusolJ/loregalia` as `origin`, starting from an empty remote      | The repo was prepared for this project                                                  | 2026-11-05 |
| D-19 | A show's status is set separately from its seasons                       | A show works before its seasons are known                                               | 2026-11-05 |
| D-20 | TVmaze for shows, not TMDB                                               | Seasons, no key, and images may be cached indefinitely                                  | 2026-11-05 |
| D-22 | Approve `ureq`, `serde` and `serde_json` in `crates/core`                | Blocking HTTP with timeouts for providers, and JSON mapping at the provider edge         | 2026-11-05 |

The details these decisions imply are in `ARCHITECTURE.md`, under Data model.

## Open: needed before metadata lookup

The findings behind these are in `METADATA.md`. Recommendations are suggestions, not decisions.

| ID   | Question                                                          | Recommendation                                                                |
| ---- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| D-09 | Which source for books?                                           | Open Library: no key. Google Books stays a possible fallback                  |
| D-10 | Which source for games?                                           | IGDB: RAWG timed out when checked, and Giant Bomb's API is unavailable        |
| D-11 | Do credentials ship in the build, or does each user bring their own? | Users bring TMDB and IGDB credentials in Settings; shows and books need none |
| D-12 | How long may covers and details be stored?                        | Follow each source's limit: refresh TMDB within 6 months; resolve IGDB's conflict before release |
| D-21 | What contact goes in the User-Agent for Open Library's higher limit? | A project address, never a user's personal one                             |

## Open: can wait

| ID   | Question                                               | Recommendation                                |
| ---- | ------------------------------------------------------ | --------------------------------------------- |
| D-13 | Bundle Inter, or keep the system font?                 | System font until the UI exists, then compare |
| D-14 | Where is the theme choice stored?                      | The SQLite database, in a settings table      |
| D-15 | Does the main area offer both a grid and a list view?  | Both are designed; start with one             |
| D-16 | Which tool builds the Windows installer?               | Compare `cargo-packager` and `cargo-wix` in the walking skeleton |
| D-18 | What happens on a second launch?                       | Focus the existing window, not a second app on one database |
