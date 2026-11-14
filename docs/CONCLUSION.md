# Developer conclusion

My thoughts on building Loregalia 1.0: what was hard, what I learned, and what I would do
differently. The prompts under each heading are there to help me write; I delete each one once I have
answered it.

## The project in numbers

Taken from the repository on 2026-11-14.

|                            |                                     |
| -------------------------- | ----------------------------------- |
| First commit               | 2026-11-05                          |
| Version 1.0.0 released     | 2026-11-14                          |
| Commits on `main`          | 148                                 |
| Pull requests merged       | 58                                  |
| Rust code, including tests | 54 files, about 16,600 lines        |
| Tests                      | 414                                 |
| Decisions recorded         | 36, in [DECISIONS.md](DECISIONS.md) |
| Crates                     | `app`, `core`, `relay` and `xtask`  |

## Why I built it

> What problem did I want to solve for myself? Why a desktop app, and why local-first? Why did I
> choose to build it entirely with AI agents instead of writing the code myself?

## How it was built

> How did a normal day look: writing tasks, reviewing branches, merging pull requests? Which parts
> did I do myself, such as decisions, reviews, the server and the GitHub settings, and which did the
> agents do? How did the gate, the sandbox and the read-only docs change the way I worked with them?

## Difficulties

> What took the longest, or surprised me? Some candidates to think about:
>
> - The stack overflow when opening a list, caused by Windows' 1 MB main-thread stack
> - The first design feeling "sterile", and getting the redesign right
> - Deciding how to handle API keys: built-in keys, user keys or a relay
> - The sandbox running out of disk while building Rust
> - GitHub's rules on `main`, and learning the branch and pull request flow
> - Deploying the relay on my own server next to everything already running there
>
> Which of these were technical problems, and which were problems of direction or decision?

## What I learned

> About Rust, GPUI and SQLite? About API terms, licences and attribution? About packaging, installers,
> code signing and releases? About running a service people depend on? About reviewing code I did not
> write, and about giving instructions to agents?

## Mistakes

> What would I do differently from the start? Decisions I made too late, or too early? Things I
> trusted without checking, or checked too late? Where did an agent go wrong, and what in my
> instructions let it?

## Working with AI agents

> What were they good at, and where did they need the most help? Which rules in
> [AGENTS.md](../AGENTS.md) mattered most? Would I build my next project the same way? What would I
> tell another developer who wants to try this?

## What surprised me

> Did people's interest on GitHub surprise me? What did the first users say?

## What comes next

> What do I want Loregalia to become? What would I build next, and what will I deliberately leave
> out? See the [roadmap](ROADMAP.md).

## Final thoughts

> If I could tell myself one thing on 2026-11-05, what would it be?
