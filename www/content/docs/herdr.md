---
title: Coming from Herdr TUI
summary: rodeo started as Herdr TUI 0.9.5 on `hc`, with the same keys, screen and config. Here is what moved to `hc`, what is gone, and what works another way.
group: Moving over
order: 1
---

## Why rodeo

Herdr TUI runs its own server, and the desktop app runs on `hc`. They share no terminals. I wanted my terminal and my app to see the same thing.

`hc` owns every program. rodeo only owns the UI, so you can close it, open it again, or run two at once. Your terminals keep running the whole time.

## What carries over

- Every key bind, with the same defaults and the same syntax. See [Key binds](/docs/keys/).
- The screen: sidebar, tab bar, panes, mode bars, overlays, menus, toasts and the mobile layout.
- The config schema. On the first start, if `~/.config/rodeo/config.toml` does not exist and `~/.config/herdr/config.toml` does, rodeo copies the Herdr file once. It never touches the Herdr file.

rodeo will drift from Herdr TUI over time.

## What `hc` does now

Sessions, restore and remote machines belonged to the Herdr server. `hc` covers them now.

| Herdr TUI | In rodeo |
| --- | --- |
| `herdr` | `rodeo` |
| `herdr session attach <name>` | `rodeo --session <name>` |
| `herdr --remote` | [Add the machine](/docs/machines/) to `hc`, or run rodeo on that machine. |
| `herdr server stop` | `hc server stop`. Kept terminals come back with a new shell. |
| `herdr server reload-config` | <kbd>prefix+shift+r</kbd>. There is no server config. |
| Restore after a restart | `hc` keeps the terminals running. Nothing needs a restore. |
| Agent detection | Agents report their own state to `hc` (OSC 7501). See [Agents](/docs/agents/). |
| Pane history | `hc` keeps the screen and scrollback of each terminal. |

rodeo reads `[session]`, `[server]`, `[update]`, `remote.manage_ssh_config` and `experimental.pane_history`, and does nothing with them.

## What is gone

- **Plugins.** There is no plugin model. A `plugin_action` command shows a config warning.
- **Self-update, release notes and onboarding.** rodeo does not update itself. The "what's new" and "update ready" items are gone from the menus.
- **Agent integrations.** Agents report their own state to `hc`, so the integrations tab of the settings is gone.
- **The socket API and the pane and workspace commands.** `hc` and the Herdr app cover programs and layout.
- **The `$name` token for spaces.** The Herdr socket API set it, and `hc` has no space objects to label. `$name` for panes still works: it reads the `name` label of the terminal.

## What works another way

- **Names.** The Herdr app and these docs say "space" for a Herdr TUI workspace. The screen keeps the Herdr words.
- **The sidebar header** shows the `rodeo▍` wordmark in place of `spaces` or `machines`.
- **Image paste** (<kbd>ctrl+v</kbd>) works in every pane, not only in remote ones.
- **Clear pane** clears the screen that rodeo shows. The copy that `hc` keeps is not cleared, so a new rodeo can show the old text.
- **Custom commands** get the `HERDR_ACTIVE_*` variables, but not `HERDR_SOCKET_PATH` or `HERDR_BIN_PATH`. They run on the machine of the focused space.
- **Sounds** are short chimes that rodeo makes itself, not Herdr's mp3 files. Your own mp3 files still work.
- **Variables.** `RODEO_CONFIG_PATH` replaces `HERDR_CONFIG_PATH`, and `RODEO_DISABLE_SOUND` joins `HERDR_DISABLE_SOUND`. A nested rodeo is found with `HC_TERMINAL_ID`.
- **Navigate mode** with more than one machine: <kbd>↑</kbd> and <kbd>↓</kbd> also stop on machine rows, and <kbd>←</kbd> <kbd>→</kbd> fold and show them. See [Machines](/docs/machines/).

[`docs/parity.md`](https://github.com/lukeed/rodeo/blob/main/docs/parity.md) has every key bind, screen element and config option of Herdr TUI 0.9.5, row by row, with what rodeo does with it.
