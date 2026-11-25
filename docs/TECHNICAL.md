# Technical guide

How Loregalia is built, run, tested, released and developed. For what the app does, see the
[README](../README.md) and [PRODUCT.md](PRODUCT.md).

## Tech stack

| Layer    | Choice                                                                  |
| -------- | ----------------------------------------------------------------------- |
| Language | Rust                                                                    |
| UI       | [GPUI](https://www.gpui.rs/), through [gpui-kit](https://gpui-kit.com/) |
| Storage  | SQLite, through `rusqlite` with the bundled library                     |
| Network  | `ureq` with timeouts, only inside `crates/core` providers               |
| Relay    | `axum` and `tokio`, sharing types and providers with `crates/core`      |
| Platform | Windows; Linux is built and tested for development, but not shipped     |

## Getting started

### Requirements

- [Rust through rustup](https://www.rust-lang.org/tools/install); `rust-toolchain.toml` selects the
  version
- On Windows: Visual Studio or Build Tools with the **Desktop development with C++** workload, and
  CMake on your `PATH`
- On Linux: the packages in [scripts/linux-packages.sh](../scripts/linux-packages.sh)

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

### Build the installer

Install the packager once, then build from the app crate:

```powershell
cargo install cargo-packager --locked
cd crates/app
cargo packager --release
```

The installer lands in `target/installer`. It installs for the current user without administrator
rights. Its uninstaller offers to delete the app's data folder, unticked by default. Starting
Loregalia while it is already open brings the open window to the front instead.

For a portable zip, which keeps its data in a `data` folder next to `loregalia.exe`, run:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\package-portable.ps1
```

Installed copies keep their data in `%APPDATA%\loregalia`.

Movie and game searches go to the relay at the address `LOREGALIA_RELAY_URL` held when the app was
built, or to `http://127.0.0.1:8080` if it was not set. To run the relay, see
[crates/relay](../crates/relay/README.md).

### Release a version

1. Set the version in the workspace `Cargo.toml`, run `cargo xtask gate`, and merge the change.
2. Make sure the repository variable `LOREGALIA_RELAY_URL` holds the relay's `https://` address,
   with no trailing slash.
3. Tag the merged commit and push the tag:

   ```powershell
   git tag v1.0.0
   git push origin v1.0.0
   ```

The [release workflow](../.github/workflows/release.yml) checks that the tag matches the version,
runs the gate, builds the installer and the portable zip with the relay address, and creates a
**draft** release with both files and their checksums. Read it over on GitHub, then publish it. Copies
already installed notice a published release within a day of starting; drafts and pre-releases are not
offered.

### Publish on winget

After the GitHub release is published, write the manifest and let winget check it:

```powershell
powershell -ExecutionPolicy Bypass -File scripts\winget-manifest.ps1 -Version 1.5.0
```

It lands in `target/winget/<version>`. Submit it with Microsoft's
[wingetcreate](https://github.com/microsoft/winget-create), which opens the pull request from your
GitHub account:

```powershell
winget install Microsoft.WingetCreate
wingetcreate submit target\winget\1.5.0
```

The first submission asks you to sign Microsoft's contributor agreement, and a moderator reviews it.
Submit 1.5.0 or later: up to 1.4.0 the app needs a Visual C++ runtime that a clean Windows does not
have (D-45).

## Project layout

```text
crates/app    GPUI views, theme and window
crates/core   Domain logic, storage and metadata providers, with no UI dependency
crates/relay  The metadata relay server that holds the TMDB and IGDB keys
deploy/relay  Docker files to run the relay behind a Cloudflare Tunnel
xtask         The gate and project rules
docs/         Product, architecture, design, decisions and roadmap
scripts/      Packaging, Linux and sandbox setup
AGENTS.md     Instructions for coding agents
```

## Why it's built this way

The full list, with dates, is in [DECISIONS.md](DECISIONS.md).

| Decision | Why |
| -------- | --- |
| Local-first, with one SQLite file and no account | A planner should work offline, and your library should be yours to keep and back up (D-01, D-12) |
| Rust and GPUI, through a pinned `gpui-kit` | A fast native app, with UI tests that run without a window; pinned because `gpui-kit` is pre-1.0 (A-01) |
| Windows first; Linux only builds and tests | Windows is where it is used and tested by hand; Linux is where the agents work (A-04) |
| No keys in the app; a small relay holds them | Nobody has to sign up for API keys, and the IGDB secret never leaves the server. Your own keys skip the relay (D-11) |
| Your own keys live in Windows Credential Manager | Keys are secrets, and a file beside your library is easy to copy or sync by accident (D-32) |
| TVmaze for shows, TMDB for movies, Open Library for books, IGDB for games | Each was the best free source for its kind with terms that allow this use; see [METADATA.md](METADATA.md) (D-09, D-10, D-20) |
| Saved items keep their source data | Your library still works when a source is down; TMDB data is refreshed before its six-month limit (D-12) |
| UUIDs as IDs | Sync between computers stays possible later without rewriting every table (D-02) |
| Seasons are their own items with their own status | You can plan one season at a time (D-03) |
| Statuses in five categories with labels per media kind | "To read" and "To play" read naturally, while filters still work across kinds (D-04) |
| Free, with no paid features | Keeps the app non-commercial under TMDB's and IGDB's terms (D-25) |
| An NSIS installer for the current user, plus a portable zip | No administrator rights needed, and the zip runs from a USB stick or a synced folder (D-16, D-30) |

## How it's built

| Who       | Does what                                                                         |
| --------- | --------------------------------------------------------------------------------- |
| AI agents | Write code and tests on `agent/*` branches, run the gate, and propose doc changes |
| The human | Sets direction, makes [product decisions](DECISIONS.md), reviews and merges       |

Agents follow [AGENTS.md](../AGENTS.md), and these limits are enforced rather than requested:

- **The gate** must pass, and it also runs in CI.
- **A commit hook** blocks commits to `main` and runs the fast gate.
- **Agent permissions** in [opencode.json](../opencode.json) deny pushing, merging, deleting files,
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

- [Product](PRODUCT.md): what the app does
- [Architecture](ARCHITECTURE.md): how the code and data are organized
- [Design](DESIGN.md): colors, type, layout and states
- [Metadata sources](METADATA.md): APIs, their limits and terms
- [Decisions](DECISIONS.md): what has been decided and what is still open
- [Roadmap](ROADMAP.md): what comes next
- [Relay](../crates/relay/README.md): the metadata server and how to deploy it
- [Developer conclusion](CONCLUSION.md): the author's thoughts on building 1.0

## Contributing

The project isn't set up for outside contributions yet, but issues with ideas or problems are welcome.
Anyone using an agent on this code should point it at [AGENTS.md](../AGENTS.md) first.
