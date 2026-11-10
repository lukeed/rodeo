---
title: Config
summary: rodeo reads one TOML file, in the format of Herdr TUI 0.9.5, so a Herdr config works as is. Every setting is optional.
group: Using rodeo
order: 1
---

## The file

rodeo reads `~/.config/rodeo/config.toml`. When `XDG_CONFIG_HOME` is set, it reads `$XDG_CONFIG_HOME/rodeo/config.toml`. `RODEO_CONFIG_PATH` names any other file.

On the first start, if that file does not exist and `~/.config/herdr/config.toml` does, rodeo copies the Herdr file once. It never writes to the Herdr file.

```sh
$ rodeo --default-config     # print the full default config
$ rodeo config check         # check your config
$ rodeo config reset-keys    # drop [keys] from your config (keeps a backup)
```

A missing file is not an error: rodeo uses the defaults.

## Load and reload

- At start, rodeo reads the whole file at once. One value of the wrong type, anywhere, makes rodeo use every default, and a bar at the top of the screen says so.
- <kbd>prefix+shift+r</kbd> reloads the file. Each section applies on its own: a section with a bad value keeps its current settings, and the others still change.
- An unknown section or key is skipped, with a warning.
- `rodeo config check` prints every problem in the file and exits with code 1 when it finds one.

The settings overlay, <kbd>prefix+s</kbd>, writes the theme, the state marks, sound and toasts to the file for you.

## Sections

| Section | Sets |
| --- | --- |
| `[theme]` | The color theme, light and dark themes, and single colors. |
| `[terminal]` | The shell and the start folder of new panes, and kitty graphics. |
| `[keys]` | Every key bind and your own commands. See [Key binds](/docs/keys/). |
| `[ui]` | Sidebar, mouse, borders, tab bar, window title and agent marks. |
| `[ui.sidebar]` | What each row of the space and agent lists shows. |
| `[ui.toast]` | Notices when an agent finishes or needs you. See [Agents](/docs/agents/). |
| `[ui.sound]` | Sounds when an agent changes state. |
| `[worktrees]` | The folder for new git worktrees. |
| `[advanced]` | How much scrollback each pane keeps. |
| `[experimental]` | Nested rodeo, and the cursor for CJK input methods. |

rodeo reads these keys and does nothing with them, because `hc` does that work now: `onboarding`, `[session]`, `[server]`, `[update]`, `remote.manage_ssh_config`, `experimental.pane_history`. `experimental.switch_ascii_input_source_in_prefix` is also read and not used.

## Themes

```toml [~/.config/rodeo/config.toml]
[theme]
name = "tokyo-night"
```

The built-in themes are `catppuccin` (the default), `catppuccin-latte`, `tokyo-night`, `tokyo-night-day`, `dracula`, `nord`, `gruvbox`, `gruvbox-light`, `one-dark`, `one-light`, `solarized`, `solarized-light`, `kanagawa`, `kanagawa-lotus`, `rose-pine`, `rose-pine-dawn` and `vesper`. `terminal` uses the 16 colors of your terminal.

Set `auto_switch = true` to follow your terminal between light and dark mode. `dark_name` and `light_name` pick the two themes; by default they are the dark and light forms of `name`.

`[theme.custom]` changes single colors on top of the theme:

```toml [~/.config/rodeo/config.toml]
[theme.custom]
accent = "#f5c2e7"
sidebar_bg = "reset"
```

A color is `"#rrggbb"`, `"#rgb"`, `"rgb(r, g, b)"`, a name such as `"blue"`, or `"reset"` for the color of your terminal. `[theme.custom.light]` and `[theme.custom.dark]` add changes for one mode only.

## Saved preferences

Some changes you make with the mouse or keys are saved in `~/.local/state/rodeo/`, and they win over the config: the sidebar width, a folded sidebar, the split between spaces and agents, the agent sort and folded groups. Focus, zoom and the active space and tab are saved there too, so rodeo opens where you left it.

## Environment

| Variable | Does |
| --- | --- |
| `RODEO_CONFIG_PATH` | The config file to read. Replaces `HERDR_CONFIG_PATH`. |
| `XDG_CONFIG_HOME` | The folder that holds `rodeo/config.toml`. |
| `XDG_STATE_HOME` | The folder that holds `rodeo/` for saved preferences. |
| `RODEO_DISABLE_SOUND` | Turns sound off. `HERDR_DISABLE_SOUND` works too. |
| `EDITOR` | The editor for <kbd>prefix+e</kbd>. |

## The default config

Every setting, with its default value. `rodeo --default-config` prints the same file.

<bongo:source file="crates/config/src/default.toml" lang="toml" title="rodeo --default-config" />
