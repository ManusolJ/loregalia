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
- [ ] Choose sources and credential handling: D-09 to D-12 and D-21

## 3. Walking skeleton

Prove the painful parts while the app is still small.

- [x] Open a SQLite file in the app data folder, creating the folder on a fresh install
- [x] Run migrations at startup, including on a database from an older version
- [x] One view writes a row and reads it back, off the UI thread
- [x] Write a log file, because the installed app has no console
- [x] One real call to the first source from `core`, with its response saved as a fixture
- [ ] Decide D-16 and D-18, then build an installer
- [ ] Human check: install it, start it from the Start menu, and confirm covers and data persist

## 4. Features

Build each one as described in `ARCHITECTURE.md`, as a separate `agent/*` branch.

- [x] Items: add by hand, view, edit, delete
- [x] Statuses: statuses as data, categories, status chips
- [x] Lists: create, rename, delete, membership
- [x] Tags: on lists and items
- [x] Shows and seasons
- [ ] Settings: API credentials, stored outside the database and the repo
- [ ] Metadata lookup: provider trait, then each chosen source
- [x] About screen: attribution each source requires
- [ ] Covers: loading, no-image tile, failure
- [ ] Theme setting: Dark, Light, System

## 5. Release readiness

- [ ] Failure paths: no network, bad key, missing database, older database
- [ ] Recheck each source's terms, and resolve IGDB's storage conflict
- [ ] No key in the repo or the installer unless the API's terms allow it
- [ ] Human check on Windows: scaling at 100% and 150%, resizing, keyboard navigation
- [ ] README with the reasons behind each major decision
