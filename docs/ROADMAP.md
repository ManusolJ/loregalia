# Roadmap

Work top to bottom. Tick items only when they are verified, not when the code compiles.

## 1. Foundations

- [x] Workspace, GPUI window with theme tokens, sidebar and empty state
- [x] Gate: project rules, format, clippy, unit and headless UI tests
- [x] CI workflow file, sandbox setup, commit hook, agent permissions
- [x] First CI run is green on Windows and Linux (needs the first push)

## 2. Human decisions

- [x] Data model decisions D-02 to D-08 and D-19
- [x] Check API terms (`METADATA.md`)
- [x] Choose the game source and credential handling: D-10 and D-11
- [x] Choose the remaining sources: D-09, D-12 and D-21

## 3. Walking skeleton

Prove the painful parts while the app is still small.

- [x] Open a SQLite file in the app data folder, creating the folder on a fresh install
- [x] Run migrations at startup, including on a database from an older version
- [x] One view writes a row and reads it back, off the UI thread
- [x] Write a log file, because the installed app has no console
- [x] One real call to the first source from `core`, with its response saved as a fixture
- [ ] Build an installer with `cargo-packager`, and make a second launch focus the open window
      (D-16, D-18)
- [ ] Human check: install it, start it from the Start menu, and confirm covers and data persist

## 4. Features

Build each one as described in `ARCHITECTURE.md`, as a separate `agent/*` branch.

- [x] Items: add by hand, view, edit, delete
- [x] Statuses: statuses as data, categories, status chips
- [x] Lists: create, rename, delete, membership
- [x] Tags: on lists and items
- [x] Shows and seasons
- [ ] Settings: optional user API keys that bypass the relay, stored outside the database and the
      repo
- [ ] Metadata lookup: provider trait, then each chosen source
- [ ] Relay: build the TMDB and IGDB relay in `crates/relay`; the gate already keeps GPUI and the
      app crate out of it
- [x] About screen: attribution each source requires
- [ ] Covers: loading, no-image tile, failure
- [ ] Theme setting: Dark, Light, System

## 5. Release readiness

- [ ] Failure paths: no network, bad key, missing database, older database
- [ ] Recheck each source's terms before release
- [ ] No key in the repo or the installer; the project's keys live only on the relay
- [ ] Human check on Windows: scaling at 100% and 150%, resizing, keyboard navigation
- [ ] README with the reasons behind each major decision
