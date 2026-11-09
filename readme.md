<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/lockup-dark.svg">
    <img src="assets/lockup-light.svg" alt="rodeo" height="64">
  </picture>
</div>

> Herdr TUI, on top of herdr-core.

rodeo is a terminal UI for [herdr-core](https://github.com/herdrdev/herdr-core) (`hc`). It has the key binds and screen of Herdr TUI 0.9.5, and it shares its spaces, tabs & panes with the Herdr desktop app. Live, both ways.

Herdr TUI runs its own server, and the desktop app runs on `hc`. They share no terminals. I wanted Herdr TUI as it is, but on `hc`, so my terminal and my app see the same thing.

So rodeo is a fresh build that only uses `hc`. `hc` owns every program. rodeo only owns the UI. Herdr TUI 0.9.5 is the spec, not the code source.

## Features

* Same key binds as Herdr TUI 0.9.5, prefix and all (`ctrl+b`)
* Same screen: sidebar, tab bar, pane borders, mode bar, overlays, mouse
* Shares the `herdr-app` layout with the desktop app & other rodeo clients
* Reads your Herdr TUI config, as is
* Agent states (blocked, working, done, idle) from `hc`, with toasts & sounds
* Other machines through `hc`'s machine links
* Worktrees, popups, copy mode, image paste, links & kitty graphics

## Requirements

* [`hc`](https://github.com/herdrdev/herdr-core) 0.1.18 or newer, on your `PATH`
* Linux or macOS

## Install

There are no releases yet, so build from source. You need [`mise`](https://mise.jdx.dev) for the pinned Rust & zig versions:

```sh
$ git clone https://git.nexthub.sh/lukeed/rodeo.git
$ cd rodeo
$ mise install
$ cargo build --release
$ cargo install --path crates/rodeo --locked   # optional: put it on your PATH
```

The binary is `target/release/rodeo`, or `~/.cargo/bin/rodeo` after the optional step.

## Usage

```sh
$ rodeo                    # open on the default hc session
$ rodeo --session work     # open on another hc session
```

Detach with `prefix+q`. Your terminals keep running in `hc`, and come right back when you open rodeo again.

Press `prefix+?` to see every key bind.

## Config

rodeo reads `~/.config/rodeo/config.toml`. It uses the same schema as Herdr TUI.

On the first start, if that file doesn't exist and `~/.config/herdr/config.toml` does, rodeo copies it over once. It never touches the Herdr file.

```sh
$ rodeo --default-config   # print the full default config
$ rodeo config check       # check your config
$ rodeo config reset-keys  # drop [keys] from your config (keeps a backup)
```

## What's different

Most of Herdr TUI maps 1:1. Some parts belonged to Herdr's own server, so `hc` covers them now (sessions, restore, remote machines). A few are gone:

* **Plugins:** no plugin model
* **Self-update, release notes & onboarding:** rodeo doesn't update itself
* **Agent integrations:** agents report their own state to `hc` (OSC 7501)

[`docs/parity.md`](docs/parity.md) has the full list, row by row.

## Development

```sh
$ mise run check   # fmt, clippy & tests
```

The tests start their own isolated `hc` core, so they never touch your terminals. Some of them also need `tmux`.

Start with [`docs/plan.md`](docs/plan.md) and [`docs/architecture.md`](docs/architecture.md).

## License

MIT © Luke Edwards
