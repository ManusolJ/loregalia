# Architecture

One Rust workspace builds one desktop binary, and later a small metadata relay (D-24). All logic,
storage and network code lives in a crate that knows nothing about the UI, so most work can be tested
in milliseconds without a window.

```text
crates/app    GPUI views, theme, window. Depends on core and gpui-kit.
crates/core   Domain types, storage, metadata providers. No GPUI.
crates/relay  The metadata relay server (D-27). Depends on core, never on GPUI or app.
xtask         The gate and project rules. Standard library only.
```

Add modules to `core` as features need them, such as `items`, `lists`, `tags`, `statuses`, `storage`
and `providers`. Do not create empty ones ahead of time.

## How a feature is built

Work inward from the screen, in this order:

1. **The shape:** what the screen needs to show, as a Rust type in `core`.
2. **Storage:** a migration first, so the schema is versioned from the first row.
3. **Pure logic:** normalizing titles and years, status rules, mapping provider JSON. These are
   functions over values, holding no connection and no client.
4. **The I/O edge:** queries for one feature, or one client per metadata source. Keep it thin.
5. **The view:** in `crates/app`, with a UI test that clicks through it.

If a test needs more infrastructure than it did yesterday, the code moved in the wrong direction.

## Threads

GPUI renders on the main thread, so a slow call in `render` or an event handler freezes the window.
Disk and network work runs in `cx.background_spawn`, and results come back through an entity update
followed by `cx.notify()`.

## Data

- One SQLite file in `%APPDATA%\com.manu.loregalia\`, never beside the executable, which is
  read-only under Program Files.
- Access through `rusqlite` with the `bundled` feature (D-01). Add it with the first storage task.
- `PRAGMA foreign_keys = ON` on every connection, because SQLite ships with it off and
  `ON DELETE CASCADE` silently does nothing without it.
- Tables are `STRICT`, or an `INTEGER` column will happily store text.
- Migrations are append-only and run at startup, before the first window opens. A wrong migration
  gets a new one that fixes it. Test on a database the previous version wrote, not only a fresh one.
- The app identifier `com.manu.loregalia` names the data folder. Changing it after real data exists
  starts the app on an empty database.

## Data model

These rules follow from D-02 to D-08. They are not a schema: add tables only when a feature needs
them.

- **IDs:** a UUID v7, stored as `TEXT`. Approved dependency: `uuid`, added with the first table that
  needs it.
- **Items:** unique by `(source, external_id)` when they come from a lookup. Hand-typed items have no
  source, and `source` being null means "typed by hand", not "the lookup failed".
- **Duplicates:** when a hand-typed item matches an existing title and media kind, ignoring case,
  warn and let the user continue.
- **Seasons:** a child table of shows, with `season_number` and its own status. They appear only for
  `MediaKind::Show`. Deleting a show deletes its seasons.
- **Show status:** set separately from its seasons and never calculated from them (D-19).
- **Statuses:** rows in a table, never a Rust enum of statuses, so the set can grow. Each row has one
  of the five categories, which can be a Rust enum, and each category's labels start as:

| Category  | Movie and show | Book      | Game      |
| --------- | -------------- | --------- | --------- |
| `planned` | To watch       | To read   | To play   |
| `active`  | In progress    | In progress | In progress |
| `paused`  | On hold        | On hold   | On hold   |
| `done`    | Watched        | Read      | Played    |
| `dropped` | Dropped        | Dropped   | Dropped   |

- **Release dates:** `release_year` and `release_date` are both optional. Sort by year, with unknown
  years last. Null means unknown, not unreleased.
- **Lists:** membership is a join table with a `position` column, which holds the manual order.
  Sorting by title or year is a view over the list and never changes `position`.
- **Tags:** two join tables, one for items and one for lists, so both keep real foreign keys.
- **Deletes:** final, after a confirmation in the UI. Memberships go with `ON DELETE CASCADE`, which
  needs a test, because it silently does nothing if the foreign key pragma is off.
- **Timestamps:** ISO-8601 UTC text ending in `Z`, shown in local time by the UI.
- **`finished_at`:** record it from the first version. Statistics are out of scope, but they can't
  be recovered for items finished before the column existed.
- **Source data (D-12):** store each item's source and external ID, and keep the user's own fields,
  such as status and notes, separate from what a source provided. Refresh TMDB data in the
  background before it is six months old.
- **Settings (D-14):** a `settings` table holds preferences such as the theme. API keys a user
  enters never go in the database.
- **Units in names:** `runtime_minutes`, `page_count`, `season_number`. Never a bare `length` or
  `size`.

## Network

- Only metadata lookups use the network, from `core` providers. TVmaze and Open Library are called
  directly. TMDB and IGDB go through the Loregalia relay, unless the user entered their own key.
- One trait, with one implementation per source, chosen by `MediaKind`. No API's field names travel
  beyond its provider module.
- Every request has a timeout. Debounce search-as-you-type, respect each source's rate limit, and
  bound every cache.
- Never build a URL from unchecked input. Choose the source by enum and pass values as parameters.
- Save each API's real response as a fixture the first time you call it. Tests never use the network.
- Remote covers through `img()` need an HTTP client installed with `cx.set_http_client(...)`. Native
  builds fail every request until one is set.

## Relay

Built for TMDB search and details; IGDB comes next (D-11, D-24). Its configuration, endpoints and
cache are in `crates/relay/README.md`. It runs on the owner's server at home for now, and moves to a VPS
if the app grows (D-26). Its code lives in `crates/relay` (D-27), and it ships as its own binary,
deployed separately from the app.

- **Relay only:** it forwards search and details for TMDB and IGDB, and nothing else, so it cannot
  be used as a general proxy for the project's keys.
- **Keys:** they live in the server's configuration, never in the repo, the app or a log.
- **A stable address:** the app reaches the relay by a domain name over HTTPS, never by an IP
  address, so moving it to a VPS needs no app update.
- **Public by design:** a desktop app cannot hide a password, so anyone can call the relay. Narrow
  endpoints and per-client rate limits are the protection. IGDB's 4 requests per second is shared by
  every user.
- **Cache within each source's limits:** TMDB for six months at most, IGDB for 24 hours
  (`METADATA.md`).
- **No personal data:** do not store searches, and keep IP addresses only as long as rate limiting
  needs them. Say so in the app's privacy note.
- **The app works without it:** saved items, hand entry, TVmaze and Open Library keep working when
  the relay is down, and a user's own keys skip it entirely.
- **AniDB later:** when anime moves to AniDB (D-28), the relay holds the registered client, caches
  results and the daily title dump, and keeps to AniDB's limits for every user at once.
- **Deploying is a human task:** agents build and test it against saved responses only.
- **Sync is separate:** sync across devices stays out of scope (`PRODUCT.md`). If it comes later,
  build it as its own service, so the relay stays small and the app stays usable offline.

## Failure

- A release build uses `panic = "abort"`, so a panic closes the window with no message. Return
  `Result`, and show the failure in the UI.
- The installed app has no console, so it needs a log file before it is first released (see the
  roadmap).
- "No matches" and "the source did not answer" are different results and must look different.

## Verification limits

- `cargo xtask gate` proves formatting, lints, logic and simulated UI behavior.
- It does not check pixels, real GPU startup, fonts, the installer, or Windows integration. Those
  need a person running the release build on Windows.
- `gpui-kit` runs its own pixel tests only on macOS and Linux, because its pinned platform crate has
  no Windows headless renderer.

## Dependencies

`gpui-kit` is pre-1.0 and changes often, so it is pinned to an exact version. Upgrade it on its own
branch, because releases can include breaking changes.

`crates/core` may also use `rusqlite` (D-01), `uuid`, and `ureq`, `serde` and `serde_json` (D-22).
Tests may use `tempfile` (D-23). `crates/app` may use `interprocess` to detect a second launch
(D-18). The relay uses `axum` and `tokio`, plus `core`'s `rusqlite`, `serde` and `serde_json`.
Shared versions live in the workspace `Cargo.toml`. Any other dependency
needs a human's approval first.
