---
title: Key binds
summary: rodeo starts from the keys of Herdr TUI 0.9.5, with the same actions, defaults and syntax. Set them in `[keys]`, and press `prefix+?` in rodeo to see every bind.
group: Using rodeo
order: 2
---

## The prefix

Most binds start with the prefix, <kbd>ctrl+b</kbd>. Press the prefix, let go, then press the key. <kbd>prefix+v</kbd> means <kbd>ctrl+b</kbd>, then <kbd>v</kbd>.

- Press the prefix two times to send it to the pane.
- <kbd>esc</kbd> cancels.
- A key with no bind is dropped.
- After the key, rodeo goes back to the mode you came from: copy mode if the pane was in copy mode, else terminal mode.

> Coming from tmux? The prefix is <kbd>ctrl+b</kbd> here too. Set `prefix = "ctrl+a"` in `[keys]` to move it.

## Modes

rodeo is always in one of five modes. Outside terminal mode, a bar at the bottom of the panes shows the mode and its keys.

<bongo:mode name="terminal">
Keys go to the focused pane. Binds without the prefix run first, then the prefix, then the pane.
</bongo:mode>

<bongo:mode name="prefix" enter="ctrl+b" bar=[["esc", "cancel"], ["ctrl+b", "send prefix"], ["w", "workspace nav"], ["?", "keybinds"]]>
The next key runs a `prefix+` bind.
</bongo:mode>

<bongo:mode name="navigate" enter="prefix+w" bar=[["esc", "back"], ["↑/↓", "workspace"], ["tab", "pane"], ["?", "keybinds"]]>
Move through spaces with <kbd>↑</kbd> <kbd>↓</kbd>, and focus one with <kbd>enter</kbd> or <kbd>1</kbd>..<kbd>9</kbd>. <kbd>h</kbd> <kbd>j</kbd> <kbd>k</kbd> <kbd>l</kbd> move between panes. The bare key of any `prefix+` bind runs it, so <kbd>c</kbd> makes a new tab.
</bongo:mode>

<bongo:mode name="resize" enter="prefix+r" bar=[["h/l", "width"], ["j/k", "height"], ["esc", "done"]]>
<kbd>h</kbd> <kbd>j</kbd> <kbd>k</kbd> <kbd>l</kbd> or the arrows resize the pane. Modifiers do nothing here.
</bongo:mode>

<bongo:mode name="copy" enter="prefix+[" bar=[["h/j/k/l w/b/e { }", "move"], ["/ ?", "search"], ["n/N", "repeat"], ["v/space", "select"], ["y/enter", "copy"], ["q/esc", "exit"]]>
Move, search and select in the scrollback, then copy with <kbd>y</kbd> or <kbd>enter</kbd>.
</bongo:mode>

## Default keys

Every action is a field in `[keys]`. An action marked <kbd>unset</kbd> has no default: bind it yourself.

### General

| Action | Default | Does |
| --- | --- | --- |
| `prefix` | <kbd>ctrl+b</kbd> | Enter prefix mode. |
| `help` | <kbd>prefix+?</kbd> | Show every key bind, with a filter. |
| `settings` | <kbd>prefix+s</kbd> | Theme, state marks, sound and toasts. |
| `goto` | <kbd>prefix+g</kbd> | Find a machine, space, pane or agent. |
| `open_notification_target` | <kbd>prefix+o</kbd> | Go to the pane of the last toast. |
| `reload_config` | <kbd>prefix+shift+r</kbd> | Reload the config. |
| `detach` | <kbd>prefix+q</kbd> | Quit rodeo. Terminals keep running in `hc`. |

### Spaces

| Action | Default | Does |
| --- | --- | --- |
| `new_workspace` | <kbd>prefix+shift+n</kbd> | New space, in the shared layout. |
| `rename_workspace` | <kbd>prefix+shift+w</kbd> | Rename the space. |
| `close_workspace` | <kbd>prefix+shift+d</kbd> | Close the space and end its terminals. |
| `workspace_picker` | <kbd>prefix+w</kbd> | Enter navigate mode. |
| `switch_workspace` | <kbd>unset</kbd> | Go to space 1 to 9. Takes a `1..9` bind. |
| `previous_workspace` / `next_workspace` | <kbd>unset</kbd> | Step through spaces on all machines. |
| `new_worktree` | <kbd>prefix+shift+g</kbd> | Add a git worktree, then a space in it. |
| `open_worktree` | <kbd>unset</kbd> | Open a space in a worktree of the repo. |
| `remove_worktree` | <kbd>unset</kbd> | Delete the worktree of the space. |

### Tabs

| Action | Default | Does |
| --- | --- | --- |
| `new_tab` | <kbd>prefix+c</kbd> | New tab. |
| `rename_tab` | <kbd>prefix+shift+t</kbd> | Rename the tab. |
| `previous_tab` / `next_tab` | <kbd>prefix+p</kbd> <kbd>prefix+n</kbd> | Step through tabs. |
| `switch_tab` | <kbd>prefix+1..9</kbd> | Go to tab 1 to 9. |
| `move_tab_previous` / `move_tab_next` | <kbd>unset</kbd> | Move the tab left or right. |
| `close_tab` | <kbd>prefix+shift+x</kbd> | Close the tab. |

### Panes

| Action | Default | Does |
| --- | --- | --- |
| `split_vertical` | <kbd>prefix+v</kbd> | Split side by side. |
| `split_horizontal` | <kbd>prefix+minus</kbd> | Split top and bottom. |
| `focus_pane_left` / `_down` / `_up` / `_right` | <kbd>prefix+h/j/k/l</kbd> | Focus the next pane that way. |
| `swap_pane_left` / `_down` / `_up` / `_right` | <kbd>prefix+shift+h/j/k/l</kbd> | Swap with the next pane that way. |
| `cycle_pane_next` / `_previous` | <kbd>prefix+tab</kbd> <kbd>prefix+shift+tab</kbd> | Cycle through panes. |
| `last_pane` | <kbd>unset</kbd> | Go back to the pane you focused before. |
| `zoom` | <kbd>prefix+z</kbd> | Zoom the pane. |
| `resize_mode` | <kbd>prefix+r</kbd> | Enter resize mode. |
| `resize_pane_left` / `_down` / `_up` / `_right` | <kbd>unset</kbd> | Resize the pane one step. |
| `copy_mode` | <kbd>prefix+[</kbd> | Enter copy mode. |
| `rename_pane` | <kbd>prefix+shift+p</kbd> | Name the pane. |
| `edit_scrollback` | <kbd>prefix+e</kbd> | Open the scrollback in `$EDITOR`. |
| `clear_pane` | <kbd>unset</kbd> | Clear the screen and scrollback of the pane. |
| `close_pane` | <kbd>prefix+x</kbd> | Close the pane and end its terminal. |
| `toggle_sidebar` | <kbd>prefix+b</kbd> | Fold or show the sidebar. |
| `remote_image_paste` | <kbd>ctrl+v</kbd> | Paste a clipboard image into the pane. |

### Agents

| Action | Default | Does |
| --- | --- | --- |
| `previous_agent` / `next_agent` | <kbd>unset</kbd> | Step through agents in the agent list. |
| `focus_agent` | <kbd>unset</kbd> | Go to agent 1 to 9. Takes a `1..9` bind. |

### Navigate mode

These work in navigate mode only, without the prefix.

| Action | Default | Does |
| --- | --- | --- |
| `navigate_workspace_up` / `_down` | <kbd>up</kbd> <kbd>down</kbd> | Move through the space list. |
| `navigate_pane_left` / `_down` / `_up` / `_right` | <kbd>h</kbd> <kbd>j</kbd> <kbd>k</kbd> <kbd>l</kbd> | Focus the next pane that way. |

## Change a key

```toml [~/.config/rodeo/config.toml]
[keys]
prefix = "ctrl+a"
split_vertical = ["prefix+v", "prefix+backslash"]
close_pane = ""                  # unbind
switch_workspace = "alt+1..9"
```

- `prefix+X` fires after the prefix. A combo without `prefix+` fires at once, in terminal mode.
- Modifiers: `ctrl`, `shift`, `alt`, `cmd`. Named keys: `minus`, `backslash`, `tab`, `space`, `enter`, `esc`, `f1` and more.
- `1..9` makes nine binds, for `switch_tab`, `switch_workspace` and `focus_agent`.
- A value is one bind or a list of binds. `""` or `[]` removes the default.
- A bind without the prefix on a plain letter is refused, so you can still type. Use `prefix+X`, or add a modifier such as `ctrl+alt`.

Run `rodeo config check` to see each bind that rodeo refused, and why. `rodeo config reset-keys` drops `[keys]` and keeps a backup.

## Custom commands

Bind a key to a command with `[[keys.command]]`. The command runs on the machine of the focused space.

```toml [~/.config/rodeo/config.toml]
[[keys.command]]
key = "prefix+y"
type = "popup"
command = "lazygit"
description = "lazygit"
width = "80%"
height = "80%"
```

| Type | Does |
| --- | --- |
| `shell` | Runs the command in the background. This is the default. |
| `pane` | Opens a zoomed pane that closes when the command exits. |
| `popup` | Opens a terminal over the tab. `width` and `height` are cells or a percent. |

The `description` is the label in <kbd>prefix+?</kbd>. A `shell` command gets the ids and the folder of the focused space, tab and pane in `HERDR_ACTIVE_*` variables.
