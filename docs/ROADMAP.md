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
- [x] Build an installer with `cargo-packager`, and make a second launch focus the open window
      (D-16, D-18)
- [ ] Human check: install it, start it from the Start menu, and confirm covers and data persist
- [x] Portable zip whose data stays beside the `.exe` (D-30)
- [x] App icon: an original design by the owner, embedded in the `.exe` and the installer

## 4. Features

Build each one as described in `ARCHITECTURE.md`, as a separate `agent/*` branch.

- [x] Items: add by hand, view, edit, delete
- [x] Statuses: statuses as data, categories, status chips
- [x] Lists: create, rename, delete, membership
- [x] Tags: on lists and items
- [x] Shows and seasons
- [x] Settings: optional user API keys that bypass the relay, stored outside the database and the
      repo
- [x] Metadata lookup: provider trait, then each chosen source
- [x] Relay: build the TMDB and IGDB relay in `crates/relay`; the gate already keeps GPUI and the
      app crate out of it
- [x] Metadata lookup for games (IGDB)
- [x] Relay: IGDB endpoints
- [x] Keyboard: Esc closes the innermost panel, Tab reaches item rows, Enter or Space opens them
- [x] About screen: attribution each source requires
- [x] Covers: loading, no-image tile, failure
- [x] Theme setting: Dark, Light, System

## 5. Release readiness

- [x] Failure paths: no network, bad key, missing database, older database
- [x] Recheck each source's terms before release
- [ ] Code signing so Windows shows no unknown-publisher warning: SignPath Foundation's free
      certificate for open source, or the Microsoft Store, which is free for individuals and signs
      MSIX packages
- [x] Credit every source in the About screen, with a privacy note on what the relay sees
- [x] Add TMDB's approved logo next to its notice in About
- [x] Store user API keys in Windows Credential Manager instead of `api-keys.json`
- [x] Release workflow: on a version tag, build the installer and portable zip in CI and attach them
      to a GitHub Release
- [x] Put the relay online behind HTTPS, and build releases with `LOREGALIA_RELAY_URL` set to it.
      `deploy/relay` and the relay README have the steps; the owner runs them
- [x] No key in the repo or the installer; the project's keys live only on the relay
- [ ] Human check on Windows: scaling at 100% and 150%, resizing, keyboard navigation
- [x] README with the reasons behind each major decision

## 6. After 1.1

- [x] Cover grid (D-15) and the layout refresh
- [x] Search the library, sort it, and filter by tag
- [x] Add an item to lists from its details, and search when adding existing items to a list
- [x] Show when an item was finished
- [x] Indexes on the columns that deletes cascade through
- [x] Details on every item, filled in gently in the background (D-35, D-36)
- [x] Seasons from TVmaze
- [x] Covers only from the sources' image servers (D-37)
- [x] Deploy the relay that serves details, then check a movie and a game against live data
- [x] Rank original games above ports and editions, and label those
- [x] Retry connections that stall, and covers that failed (D-38)
- [x] Large libraries: draw only what is visible, and keep your place in the list (D-39)
- [x] Overlapping changes never show an older result
- [x] Back up the library, and export it as JSON or CSV (D-40)
- [x] Pick for me: three suggestions from your own library, with reasons (D-41)
- [x] Coming soon: unreleased items and new seasons, with dates (D-42)
- [x] Deploy the relay that keeps unreleased titles for 3 days
- [x] Say when a newer version exists (D-43)
- [x] Restore a backup from Settings, and when the library is damaged (D-44)
- [x] Run on a clean Windows, without the Visual C++ runtime (D-45)
- [ ] Publish on winget: the manifest script is ready; the owner submits it after a release (D-45)
