# AGENTS.md

Loregalia is a local-first desktop app for planning what to watch, read and play next, and for
recording what is finished. Rust, GPUI through `gpui-kit`, SQLite. User data never leaves the
computer; a small Rust relay only holds metadata API keys (D-11). Windows is the shipped platform;
Linux only has to build and pass tests, because agents work in a Linux sandbox.

## Commands

| Command                                     | Use                                                |
| ------------------------------------------- | -------------------------------------------------- |
| `cargo xtask gate`                          | Required before you finish or commit               |
| `cargo xtask gate --fast`                   | While iterating; also runs on every commit         |
| `cargo test -p loregalia --test shell NAME` | One UI test                                        |
| `cargo test -p loregalia-core NAME`         | One domain test, compiles in seconds               |
| `cargo fmt --all`                           | Fix formatting                                     |
| `cargo run`                                 | Opens the app window; only works on a real desktop |

Humans create sandboxes with `scripts/new-sandbox.ps1`, which runs setup. There, `docs/`, the rules
and the gate are mounted read-only, and secret files read as empty.

## Workflow

1. Create a branch: `git switch -c agent/<short-task>`. Never commit to `main`.
2. Read only the doc your task needs (table below).
3. Write the failing test first, unless the change cannot be tested.
4. Make the smallest change that passes it. Do not scaffold for future features.
5. Run `cargo xtask gate`. Fix the code until it passes.
6. Commit using Conventional Commits, for example `feat: add list creation`.
7. Report what changed, the gate result, and what a human must check by hand.

## Stop and ask a human when

- The task depends on a decision marked **Open** in `docs/DECISIONS.md`. Do not choose for them.
- You want to add a dependency, change `gpui-kit`'s version, or change the toolchain.
- You want to change a read-only file: `docs/`, `README.md`, this file, the gate, CI, `scripts/`,
  `.githooks/` or `opencode.json`. These edits are denied, so propose the change in your report.
- The same failure survives three different attempts. Report what you tried.
- You would need to delete user data, or weaken or remove a test.

## Never

- Push, merge, rebase, or rewrite history. A human reviews and merges `agent/*` branches.
- Read, write or print `.env` files or other secrets by any method, including the shell.
- Commit secrets, or add real API keys to the repo. Tests never use real keys or the network.
- Claim something works without running it. Name anything you could not verify.
- Pass the gate by adding `#[allow]`, skipping a test, or relaxing an assertion.
- Invent an API. `gpui-kit` is pre-1.0 and your training data is probably out of date.

## Find the real API

Read the pinned source, not your memory:

- `~/.cargo/registry/src/*/gpui-kit-0.9.0/`: `TESTING.md`, and `tests/` for working examples
- `~/.cargo/registry/src/*/gpui-component-0.9.0/src/`: components
- `~/.cargo/registry/src/*/gpui-pre-0.3.11/src/`: GPUI itself

## Code rules

The gate enforces the rules marked (gate).

- **Crates (gate):** `crates/core` is domain logic, storage and metadata providers, and must not
  depend on GPUI. `crates/app` is views only, with no SQL and no HTTP. `crates/relay` must not
  depend on GPUI or on `crates/app`.
- **Logic goes in `core`** as plain functions over values, so it tests in milliseconds.
- **No comments (gate).** Rename or restructure instead.
- **No panics in non-test code (gate):** no `unwrap`, `expect`, `panic!` or `todo!`. A release panic
  closes the window with no message. Return `Result`.
- **No `unsafe` (gate).**
- **Import by name (gate):** never `use gpui_kit::*`, which also shadows `#[test]`.
- **Keep the UI thread free:** disk and network work goes through `cx.background_spawn`, never
  `render`.
- **SQL:** bound parameters only. Migrations are append-only. Tables are `STRICT`, and every
  connection sets `PRAGMA foreign_keys = ON`.
- **Network:** only inside `core` providers, with a timeout on every request. Map each API's JSON to
  our own types at the edge.

## UI rules

- **Colors (gate):** only `Palette` tokens from `crates/app/src/theme.rs`. No raw colors in views.
- Follow `docs/DESIGN.md` for type, spacing, radius, covers and states.
- Prefer `gpui_kit::component` over custom controls.
- Repeated elements need IDs derived from domain data, never a list index.
- Give anything a test needs an `.id(...)`, `.test_support()` and an accessible label or role.

## Testing rules

- Logic: `#[test]` in the same file as the code, in `crates/core`.
- UI: `#[gpui_kit::test]` with `TestAppContext` in `crates/app/tests/`. These run without a window,
  click through real hit testing, and do not check pixels.
- Providers: test mapping against saved JSON in `tests/fixtures/`. Never call the network.
- Storage: use in-memory SQLite with the real migrations applied.
- Cause failures on purpose: no network, a bad key, a missing or older database.
- `empty` and `failed` must be different results, so the UI can show them differently.
- **Agents cannot see the app.** List visual checks for the human in your report.

## Where things are

| Need                                       | Read                   |
| ------------------------------------------ | ---------------------- |
| What the app does and what is out of scope | `docs/PRODUCT.md`      |
| Crate layout, threading, data, network     | `docs/ARCHITECTURE.md` |
| Colors, type, layout, states, statuses     | `docs/DESIGN.md`       |
| Metadata sources, limits and API terms     | `docs/METADATA.md`     |
| Decisions made and still open              | `docs/DECISIONS.md`    |
| What to build next                         | `docs/ROADMAP.md`      |

## Traps

When something costs real time, propose a line in your report: the surprising fact, its symptom,
the fix.

- **`gpui-kit` ships no working HTTP client on native.** Remote `img()` URLs fail until the app
  installs one with `cx.set_http_client(...)`.
- clippy.toml allows unwrap in test functions, but not in helper functions inside integration tests. A TempDir::new helper with unwrap() fails the gate; helpers must return Option/Result and only test bodies may unwrap.
- gpui-kit exposes the button at gpui_kit::component::button::Button, not gpui_kit::component::Button.
- ElementId does not implement From<(&str, String)>; use format!("row-{}", id) or ElementId::Name.
- One intermediate test run created a gitignored crates/app/loregalia.db. It is not in any commit; it is safe to delete.
