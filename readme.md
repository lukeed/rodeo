<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/lockup-dark.svg">
  <img src="assets/lockup-light.svg" alt="rodeo" height="64">
</picture>

> Round up your terminals.

rodeo is a keyboard-first terminal UI for [herdr-core](https://github.com/herdrdev/herdr-core) (`hc`). It shares spaces, tabs & panes with the Herdr desktop app. Make a split in rodeo and it shows up in the app, and the other way around.

`hc` owns every program. rodeo only owns the UI, so you can close it, open it again, or run two at once. Your terminals keep running the whole time.

## Features

* Workspaces, tabs & splits, all from the keyboard (`ctrl+b` prefix) or the mouse
* Shares its layout with the Herdr desktop app & other rodeo clients
* Agent states (blocked, working, done, idle) at a glance, with toasts & sounds
* Other machines through `hc`'s machine links
* Worktrees, popups, copy mode, image paste, links & kitty graphics
* Your Herdr TUI keys & config work as is (see below)

## Requirements

* [`hc`](https://github.com/herdrdev/herdr-core) 0.1.18 or newer, on your `PATH`
* Linux or macOS

## Install

```sh
$ curl -fsSL https://rodeo.lukeed.com | sh
```

Or pick your installer:

```sh
$ brew install lukeed/tap/rodeo
$ mise use -g packslip:github.com/lukeed/rodeo
```

See the [install docs](https://rodeo.lukeed.com/docs/install/) for the details, or build from source (see [Development](#development)).

## Usage

```sh
$ rodeo                    # open on the default hc session
$ rodeo --session work     # open on another hc session
```

Detach with `prefix+q`. Your terminals keep running in `hc`, and come right back when you open rodeo again.

Press `prefix+?` to see every key bind.

## Config

rodeo reads `~/.config/rodeo/config.toml`.

```sh
$ rodeo --default-config   # print the full default config
$ rodeo config check       # check your config
$ rodeo config reset-keys  # drop [keys] from your config (keeps a backup)
```

## Coming from Herdr TUI?

rodeo started as Herdr TUI 0.9.5 on `hc`. Same key binds, same screen, same config schema. On the first start, if `~/.config/rodeo/config.toml` doesn't exist and `~/.config/herdr/config.toml` does, rodeo copies it over once. It never touches the Herdr file.

Why? Herdr TUI runs its own server, and the desktop app runs on `hc`. They share no terminals. I wanted my terminal and my app to see the same thing.

It'll drift from Herdr TUI over time. A few parts are already gone:

* **Plugins:** no plugin model
* **Self-update, release notes & onboarding:** rodeo doesn't update itself
* **Agent integrations:** agents report their own state to `hc` (OSC 7501)

Sessions, restore & remote machines belonged to Herdr's own server, so `hc` covers them now. [`docs/parity.md`](docs/parity.md) has the full list, row by row.

## Development

You need [`mise`](https://mise.jdx.dev) for the pinned Rust & zig versions:

```sh
$ git clone https://github.com/lukeed/rodeo.git
$ cd rodeo
$ mise install
$ cargo build --release
$ cargo install --path crates/rodeo --locked   # optional: put it on your PATH
$ mise run check                               # fmt, clippy & tests
```

The binary is `target/release/rodeo`, or `~/.cargo/bin/rodeo` after the optional step.

The tests start their own isolated `hc` core, so they never touch your terminals. Some of them also need `tmux`.

Start with [`docs/plan.md`](docs/plan.md) and [`docs/architecture.md`](docs/architecture.md).

## License

MIT © Luke Edwards
