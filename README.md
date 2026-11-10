# Loregalia

A local-first desktop app for planning what to watch, read and play next, and for keeping a record
of what you have finished. It brings movies, shows, books and games together in one place.

> [!IMPORTANT]
> **This project is built entirely with agentic AI.** AI coding agents write the code, tests and
> documentation. A human sets the direction, makes the product decisions, reviews the changes and
> merges them. Treat the code accordingly: it is checked by automated tests and human review, but it
> has not been written or audited line by line by a person. See [How it's built](#how-its-built).

## Status

**Early development: not ready to use yet.** Lists, items, statuses, tags, seasons and lookup for
shows, books and movies work, but there is no installer yet. See the [roadmap](docs/ROADMAP.md).

## Planned features

Loregalia is a planner, not a progress tracker. It helps you decide what to pick up next, and it does
not record which episode or page you stopped at.

- **Lists:** custom lists in your own order, where an item can belong to several lists
- **Tags:** your own labels on lists and items
- **Media:** movies, books and games as single items, and shows with their seasons
- **Statuses:** To watch, In progress, On hold, Watched, Dropped, with labels that fit each type of
  media, and room to add more
- **Metadata lookup:** search public APIs to fill in titles, covers and details, or type items by hand
- **Local-first:** your data stays in a SQLite file on your computer, with no account. Movie and game
  searches go through a small Loregalia server that holds the API keys, unless you add your own

The full scope, including what is deliberately left out, is in [PRODUCT.md](docs/PRODUCT.md).

## Tech stack

| Layer    | Choice                                                                  |
| -------- | ----------------------------------------------------------------------- |
| Language | Rust                                                                    |
| UI       | [GPUI](https://www.gpui.rs/), through [gpui-kit](https://gpui-kit.com/) |
| Storage  | SQLite                                                                  |
| Platform | Windows; Linux is built and tested for development, but not shipped     |

## Getting started

### Requirements

- [Rust through rustup](https://www.rust-lang.org/tools/install); `rust-toolchain.toml` selects the
  version
- On Windows: Visual Studio or Build Tools with the **Desktop development with C++** workload, and
  CMake on your `PATH`
- On Linux: the packages in [scripts/linux-packages.sh](scripts/linux-packages.sh)

### Run it

```powershell
git clone https://github.com/ManusolJ/loregalia.git
cd loregalia
cargo run
```

The first build takes a few minutes.

### Check it

```powershell
cargo xtask gate
```

This runs the project rules, formatting, clippy and all tests. Use `cargo xtask gate --fast` for the
quick version.

## Project layout

```text
crates/app    GPUI views, theme and window
crates/core   Domain logic, storage and metadata providers, with no UI dependency
crates/relay  The metadata relay server that holds the TMDB and IGDB keys
xtask         The gate and project rules
docs/         Product, architecture, design, decisions and roadmap
scripts/      Linux and sandbox setup
AGENTS.md     Instructions for coding agents
```

## How it's built

| Who         | Does what                                                                       |
| ----------- | ------------------------------------------------------------------------------- |
| AI agents   | Write code and tests on `agent/*` branches, run the gate, and propose doc changes |
| The human   | Sets direction, makes [product decisions](docs/DECISIONS.md), reviews and merges |

Agents follow [AGENTS.md](AGENTS.md), and these limits are enforced rather than requested:

- **The gate** must pass, and it also runs in CI.
- **A commit hook** blocks commits to `main` and runs the fast gate.
- **Agent permissions** in [opencode.json](opencode.json) deny pushing, merging, deleting files,
  touching secrets, and editing the docs or the rules themselves.
- **A sandbox** keeps agents away from the rest of the computer. Its script also mounts the docs and
  the rules as read-only, and covers secret files such as `.env` with an empty file.

Create a sandbox with the script, then start the agent:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\new-sandbox.ps1 -Name <name>
sbx run --name <name> -- --auto
```

Permission rules match text, so they can be worked around. The read-only mounts can't be, but they are
set when a sandbox is created, so create a new sandbox after adding a secret file. Agents can still
change `.git`, which is why GitHub's rules on `main` and your review remain necessary.

To review an agent's work, check it locally, then merge it through a pull request:

```powershell
git log main..agent/<task> --oneline
git diff main...agent/<task>
git switch agent/<task>
cargo xtask gate
cargo run
git push -u origin agent/<task>
```

Agents never push, so this last step is always done by the human. After merging the pull request on
GitHub, update your local copy:

```powershell
git switch main
git pull
```

If creating a sandbox fails with `erofs` and `No space left on device` even with free disk space, run
`sbx daemon restart` and try again. Keep any file you mount into a sandbox out of `%LOCALAPPDATA%`,
because creating the sandbox failed with mask files stored there.

## Documentation

- [Product](docs/PRODUCT.md): what the app does
- [Architecture](docs/ARCHITECTURE.md): how the code and data are organized
- [Design](docs/DESIGN.md): colors, type, layout and states
- [Metadata sources](docs/METADATA.md): candidate APIs, their limits and terms
- [Decisions](docs/DECISIONS.md): what has been decided and what is still open
- [Roadmap](docs/ROADMAP.md): what comes next

## Contributing

The project isn't set up for outside contributions yet, but issues with ideas or problems are welcome.
Anyone using an agent on this code should point it at [AGENTS.md](AGENTS.md) first.

## License

[MIT](LICENSE).
