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
| D-23 | Approve `tempfile` as a dev-dependency only                            | Replaces four copies of a test helper across two crates; never ships in the app          | 2026-11-05 |
| D-10 | IGDB for games, through the Loregalia relay; a user's own Twitch credentials go straight to IGDB | Covers every platform. Steam's and GOG's store endpoints are unofficial, Steam's subscriber agreement bans automated access, and Epic blocks them | 2026-11-07 |
| D-11 | The app ships no keys. A small Loregalia relay holds the project's TMDB and IGDB keys, and users may enter their own keys, which bypass the relay | No setup for users, and the IGDB secret never leaves the server | 2026-11-07 |
| D-24 | Build the relay in Rust                                                  | The same language as the app, so it can share types and provider code with `core`        | 2026-11-07 |
| D-25 | Loregalia stays free; an optional Ko-fi link may go in About, never tied to a feature or to metadata | Keeps the app non-commercial under TMDB's and IGDB's terms | 2026-11-07 |
| D-26 | Host the relay on the owner's own server at home; move to a VPS if the app grows | No hosting cost while the app is small; the app reaches the relay by domain name, so a move needs no app update | 2026-11-07 |
| D-27 | The relay lives in this workspace as `crates/relay`                      | Shares `core`'s types and provider code; one gate, and one change updates both sides     | 2026-11-07 |
| D-28 | TVmaze for anime now; AniDB, through the relay, when more is needed      | AniDB has an API, and its strict limits and registered client suit a relay that caches   | 2026-11-07 |
| D-09 | Open Library for books, called directly; Google Books through the relay only if results are poor | No key, and enough coverage to start | 2026-11-07 |
| D-12 | Saved items keep their source data. Store each item's source ID, keep the user's own fields separate, and refresh TMDB data before it is six months old. Saved IGDB data follows IGDB's FAQ | Works offline, as a planner should; the relay's cache keeps to the stricter limits | 2026-11-07 |
| D-13 | The system font, Segoe UI on Windows                                      | Nothing to ship, and it looks native; easy to revisit once the screens exist              | 2026-11-07 |
| D-14 | Settings, including the theme, live in a `settings` table in the database; user API keys do not | One file to back up, using the existing migrations. Keys belong in the OS credential store, which needs its own dependency approval | 2026-11-07 |
| D-15 | A list view first, and a grid once covers work                           | Compact suits long planning lists, and covers are not built yet                          | 2026-11-07 |
| D-16 | `cargo-packager`, producing an NSIS `.exe` installer                     | One tool, with room for an updater. Code signing is a later decision                     | 2026-11-07 |
| D-18 | A second launch brings the open window to the front, using `interprocess` | What users expect, and two copies writing one SQLite file risk lock errors               | 2026-11-07 |
| D-21 | Open Library's User-Agent names the app and its repo, with no email      | The limit is per IP address, so 1 request per second per user is enough for a search that waits for typing to stop | 2026-11-07 |
| D-29 | The data folder is `%APPDATA%\loregalia`; an old `com.manu.loregalia` folder is renamed at startup | A clean path the owner can find, without losing anyone's library | 2026-11-11 |
| D-30 | An optional portable zip keeps its data beside the `.exe`, marked by a `portable` file | Runs from a USB stick or a synced folder, with no installer | 2026-11-11 |
| D-32 | User API keys live in Windows Credential Manager, through `keyring-core` and `windows-native-keyring-store`; an old `api-keys.json` is moved in at startup | Keys are secrets, and a plain-text file beside the library is easy to copy or sync by accident | 2026-11-13 |

The details these decisions imply are in `ARCHITECTURE.md`, under Data model.

## Open

None right now. Add a new question here with a recommendation, and move it up once decided.
