# Handoff

Written 2026-11-07 by a Claude session on the user's Mac. Read `docs/plan.md` next.

## Why thuic exists

- Herdr TUI (0.9.5) runs its own server. The Herdr desktop app runs on herdr-core (`hc`). They share no terminals (herdr.dev says so). Upstream has no plan to move the TUI onto hc: herdr-core issue #25 asks for it and has no maintainer reply.
- The user wants Herdr TUI as it is, on hc, sharing spaces and tabs with the app.

## Decisions made

- **Own build in Rust.** Not a fork of Herdr TUI: most of its ~291k lines do what hc now does (PTYs, persistence, SSH remotes, agent detection), and its workspace model differs from the app's. Not a continuation of a community project: all three are days old, written by agents, by authors with no track record. Copy code from Herdr TUI (Apache-2.0) where it gives the exact look and keys.
- **The `herdr-app` doc in core is the only source of truth** for spaces, tabs and the pane tree.

## Open questions: ask the user before you build these parts

1. Blocks that thuic can't draw (app mods such as Files and Changes): show a placeholder, leave them out of thuic's layout, or draw text versions of the built-in ones? Recommendation: placeholder.
2. Herdr TUI plugins: does the user use any? If yes, thuic needs a plugin model.
3. Branch: the repo is on `main`. Does the user want `dev` for work and `main` for releases, like their other repos?
4. Config path: read `~/.config/herdr/config.toml` as is, or a `~/.config/thuic/config.toml` with the same schema?
5. License for thuic.

## References: `~/oss/thuic-ref`

Read only. Not part of the repo.

| Path | What it is for |
|---|---|
| `herdr/` | Herdr TUI at tag v0.9.5 (`2ecf19a`): the parity target and the code to copy |
| `herdr-app/` | Desktop app source: the `herdr-app` doc schema (`src/layout.rs`, `src/mirror_sync.rs`), the labels, how it uses hc (`src/hc.rs`) |
| `herdr-core/` | hc source: the protocol, attach, docs, events, test helpers |
| `pocketty/` | The user's own app (private, branch `dev`): `crates/hc` (a Rust hc client to reuse), `crates/terminal` (libghostty-vt wrapper), `docs/adr/0012-hc-replaces-herdr.md` |
| `community/followcat/` | hc-tui, a parity-driven rewrite: `docs/` (requirements, parity, runtime), `scripts/parity/` |
| `community/wuxiaobai24/` | hc-tui that shares the `herdr-app` doc with the app (docs in Chinese) |
| `community/SuTang-vain/` | Herdr TUI copy with an hc pane backend in `src/core_native/` |
| `samples/hc-schema.json` | `hc schema` (OpenRPC) from this machine's hc 0.1.18 |
| `samples/herdr-app-doc.json` | A live `herdr-app` doc from the user's Mac (doc version 58) |
| `samples/herdr-app-AGENTS.md`, `herdr.d.ts`, `herdr-app-commands.json`, `herdr-app-skill.md` | The app's generated docs, types and command list (app 0.1.13) |
| `samples/hc-skill.md` | `hc --skill` |
| `samples/herdr-tui-config.mac.toml` | The user's Herdr TUI config on the Mac |

Use the community projects as maps of hc pitfalls, not as code to copy. Their authors and licenses: all Apache-2.0, unknown authors.

## Facts verified this session

- Herdr app shows only terminals with `herdr.space` and `herdr.tab` labels. A bare `hc run` stays out of the app. `herdr-app do terminal.keep --terminal <t>` adopts one.
- Herdr app drops arrive as one bracketed paste: `'<path>' `. Claude Code turns that into `[Image #1]` when it can read the file.
- On macOS, programs in hc can't read `~/Desktop`, `~/Documents` or `~/Downloads` (TCC). The user was told to give Herdr Core Full Disk Access.
- The user's local Mac user is `lukee`. On workbox it is `lukeed`.
- workbox has hc 0.1.18, tmux, screen, mise (cargo, zig), gh (HTTPS only: no GitHub SSH key).
