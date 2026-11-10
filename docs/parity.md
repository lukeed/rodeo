# Parity with Herdr TUI v0.9.5

This is the inventory of every key bind, visible element and config option in Herdr TUI v0.9.5, and what rodeo does with each one. The detail (defaults, glyphs, colors, exact keys, source lines) is in `docs/research/`:

- `herdr-keys-config.md`: key binds, modes, overlay keys, config options, config load and reload.
- `herdr-ui.md`: screen regions, sidebar, tab bar, panes, overlays, mouse, themes, titles, notifications.
- `hc-integration.md`: the hc protocol, the `herdr-app` doc, and the pocketty crates.

Status words:

- **match**: rodeo does the same thing. The look need not be exact, but the same element shows and the same key gives the same result.
- **hc**: the feature belonged to the Herdr runtime. rodeo gets it from hc or from the `herdr-app` doc, as the Notes column says.
- **dropped**: rodeo does not have it. The Notes column gives the reason.

Names: Herdr TUI's "workspace" is the doc's "space". The UI keeps Herdr's words ("workspace", "spaces", "agents") where Herdr shows them.

## Concepts

| Herdr 0.9.5 | Status | Notes |
|---|---|---|
| Workspace | hc | A `herdr-app` doc space. Order: local machine first, then machines by name; doc order inside a machine. |
| Workspace label | hc | Space `name`, else the app's rule: git top-level folder name of its first terminal, else that terminal's live folder, else `folder`; `~` for home. |
| Tab | hc | A doc tab. Label: `name`, else its 1-based number. |
| Last workspace closed | match | When a user closes the last workspace in this client (its last tab or pane too, or its group), one doc write removes it and adds a fresh workspace with a new shell, as Herdr does (folder: `terminal.new_cwd` with no source pane, so `follow` gives home). When the doc becomes empty through another client or the app, or because the last shell exits (every client sees the exit), rodeo adds nothing: it shows the empty spaces list, and `new` or `prefix+shift+n` starts a workspace. Herdr leaves the pane surface blank; rodeo centers the hat, the wordmark and a `new workspace` key hint there. |
| Pane | hc | A doc `block` with `{terminal}` content. The terminal is an hc terminal with `herdr.space`, `herdr.tab`, `herdr.seen=0` labels and `keep: true`. |
| Pane manual name | hc | Stored in the block's `title` field. The app parses and keeps `title`, so it survives app writes. |
| Mod blocks (`{mod}`) | match | Files: a text folder tree. Other mods: a placeholder with the mod name. |
| Split tree | hc | Doc `row` (side by side) and `column` (stacked) with `size.weight`. Split and remove follow the app's tree rules. |
| Zoom, focus, active workspace, active tab | match | Private to each rodeo client, as in the app. Never in the doc. Saved on quit beside the client preferences and restored on the next start, as Herdr's server keeps them. |
| Agent | hc | A terminal with an hc status (OSC 7501 reports). Kind from `status.app`. |
| Agent state blocked / working / done / idle / unknown | hc | From `status.state` and `turn` against the shared `herdr.seen` label, as the app computes it. `error` shows as blocked. `status: null` is not an agent. Agents that do not report OSC 7501 show no state (Herdr detected them from the screen). |
| Seen tracking | hc | `herdr.seen=<turn>` label, raised only while a person looks at the pane, once per turn. Shared with the app. |
| Session | hc | An hc session (`HC_SESSION`). `rodeo --session <name>` sets it. |
| Client / server | hc | hc is the server. rodeo is a client with no server of its own. Detach quits rodeo; terminals keep running. |
| Multiple clients, per-client view | match | Each rodeo client has its own focus and view. The last active viewer sets a terminal's size (hc rule). |
| Connected machines (endpoints) | hc | `machine.list`, each machine's own `herdr-app` doc (`on`), and `machine/tN` terminal addresses. Temporary machines are not shown. |

## Modes

| Mode | Status | Notes |
|---|---|---|
| Terminal | match | Keys go to the focused pane, encoded by libghostty for the program's modes. |
| Prefix | match | All rules in `herdr-keys-config.md` 1.7.2, including prefix twice to send it. |
| Navigate | match | All keys in 1.7.3, including the bare right-hand key of any `prefix+X` binding. |
| Resize | match | Fixed `hjkl` and arrows, modifiers ignored. |
| Copy | match | All keys in 1.7.5. History comes from the local VT scrollback, filled by `history` on attach. |

## Key binds

Every `[keys]` field, with Herdr's defaults and binding syntax (1.1 to 1.3: modifiers, named keys, `1..9` ranges, unbind with `""` or `[]`, conflict rules, unsafe direct binding rule, shifted key rules).

| Action | Default | Status | Notes |
|---|---|---|---|
| `prefix` | `ctrl+b` | match | String or array. Hidden `extra_prefixes` accepted. |
| `help` | `prefix+?` | match | |
| `settings` | `prefix+s` | match | |
| `new_workspace` | `prefix+shift+n` | match | New space in the doc, folder from `terminal.new_cwd`. |
| `new_worktree` | `prefix+shift+g` | hc | `git worktree add` with `machine.exec` on the space's machine, into `worktrees.directory`, then a new space there. Same layout as the app's worktree mod. |
| `open_worktree` | unset | hc | `git worktree list` with `machine.exec`. |
| `remove_worktree` | unset | hc | `git worktree remove` with `machine.exec`, then close the space. |
| `rename_workspace` | `prefix+shift+w` | match | |
| `close_workspace` | `prefix+shift+d` | match | Kills the space's terminals, as the app does. |
| `workspace_picker` | `prefix+w` | match | Enters navigate mode. |
| `goto` | `prefix+g` | match | |
| `navigate_workspace_up` / `_down` | `up` / `down` | match | |
| `navigate_pane_left` / `_down` / `_up` / `_right` | `h` `j` `k` `l` | match | |
| `detach` | `prefix+q` | match | Quits rodeo. |
| `reload_config` | `prefix+shift+r` | match | Reloads rodeo's config only. There is no server config. |
| `open_notification_target` | `prefix+o` | match | |
| `previous_workspace` / `next_workspace` | unset | match | Across all machines. |
| `previous_agent` / `next_agent` | unset | match | |
| `focus_agent` | unset (indexed) | match | |
| `remote_image_paste` | `ctrl+v` | hc | Applies to every pane, not only remote ones: read the client clipboard image, `terminal.put` it, bracket-paste the path. |
| `new_tab` | `prefix+c` | match | |
| `rename_tab` | `prefix+shift+t` | match | |
| `previous_tab` / `next_tab` | `prefix+p` / `prefix+n` | match | |
| `move_tab_previous` / `move_tab_next` | unset | match | Reorders tabs in the doc. |
| `switch_tab` | `prefix+1..9` | match | |
| `switch_workspace` | unset (indexed) | match | |
| `close_tab` | `prefix+shift+x` | match | |
| `rename_pane` | `prefix+shift+p` | match | Writes the block `title`. |
| `edit_scrollback` | `prefix+e` | match | Writes the local VT scrollback to a temp file and opens `$EDITOR` on it in a popup terminal. |
| `clear_pane` | unset | match | Clears the local VT screen and scrollback. hc's own screen copy is not cleared, so a reattach can show old text. |
| `copy_mode` | `prefix+[` | match | |
| `focus_pane_left` / `_down` / `_up` / `_right` | `prefix+h/j/k/l` | match | |
| `swap_pane_left` / `_down` / `_up` / `_right` | `prefix+shift+h/j/k/l` | match | Swaps block contents in the doc. Not listed in keybind help, as in Herdr. |
| `cycle_pane_next` / `cycle_pane_previous` | `prefix+tab` / `prefix+shift+tab` | match | |
| `last_pane` | unset | match | |
| `split_vertical` / `split_horizontal` | `prefix+v` / `prefix+minus` | match | |
| `close_pane` | `prefix+x` | match | Kills the terminal, as the app does. |
| `zoom` (alias `fullscreen`) | `prefix+z` | match | |
| `resize_mode` | `prefix+r` | match | |
| `resize_pane_left` / `_down` / `_up` / `_right` | unset | match | Changes `size.weight` in the doc. |
| `toggle_sidebar` | `prefix+b` | match | |
| `[keys.indexed]` `tabs` / `workspaces` / `agents` | unset | match | Legacy modifier-only form. |
| `[[keys.command]]` type `shell` | | match | Runs `/bin/sh -lc` on the focused space's machine with `machine.exec`, with `HERDR_ACTIVE_*` env. `HERDR_SOCKET_PATH` and `HERDR_BIN_PATH` are not set. |
| `[[keys.command]]` type `pane` | | hc | A temporary zoomed pane with an hc terminal, closed when the command exits. |
| `[[keys.command]]` type `popup` | | hc | An hc terminal in the popup overlay, with `width` and `height`. |
| `[[keys.command]]` type `plugin_action` | | dropped | No plugins. Shows a config diagnostic. |

Fixed keys in overlays and fields: keybind help, Goto picker, settings, name dialogs, close confirm, worktree dialogs, context menu, global menu, text field editing keys, modal paste. Status: match for all, as listed in `herdr-keys-config.md` 1.8 to 1.10, except the settings integrations tab and onboarding (below).

## Screen

| Element | Status | Notes |
|---|---|---|
| Desktop layout: sidebar left, tab bar top or bottom, pane surface | match | |
| Mobile layout (`cols <= ui.mobile_width_threshold`): 2-row header, the full pane split below it, mobile switcher | match | |
| Mode bar (PREFIX, NAVIGATE, RESIZE, COPY, COPY search, ERROR) | match | Same segments and key labels. The navigate-mode `update ready` segment is dropped. |
| Expanded sidebar: spaces section, agents section, section split, footer with `menu` button | match | |
| Compact collapsed sidebar (4-column status rail), hidden mode | match | |
| Multi-machine sidebar (machine headers, per-machine spaces, machine states) | hc | From `machine.list` and each machine's doc. |
| Sidebar header | match | Deliberate difference: the expanded sidebar header is the `rodeo▍` wordmark (bold text, accent cursor) in both views, in place of Herdr's ` spaces` (one machine) and ` machines` (more machines). The blank row below it stays. |
| Agent state glyphs, `dots` and `symbols` styles, colors | match | |
| Sidebar tokens and rules (`ui.sidebar.*`) | match | `branch` and `git_status` come from `machine.exec git` on the space's machine. `machine`, `workspace`, `tab`, `pane`, `agent`, `terminal_title`, `terminal_title_stripped`, `state_icon`, `state_text`: match. |
| Pane metadata token `$name` | hc | Reads the hc label `name` on the pane's terminal. |
| Space metadata token `$name` | dropped | Was set by the Herdr socket API. hc has no space objects to label. |
| Worktree groups in the sidebar (parent, linked checkouts, collapse) | hc | Grouped by the git common dir from `machine.exec`. |
| Tab bar: labels, focus style, new-tab button, overflow arrows, drag reorder | match | |
| Tab bar right entries `zoom`, `hostname`, `datetime`, `text`, `command` | match | `hostname` and `command` run on the focused space's machine (`machine.exec`). |
| Pane borders (`auto`, `always`, `off`), outer borders, gaps, focus style | match | |
| Pane border title (manual name, else agent label when `show_agent_labels_on_pane_borders`) | match | |
| Zoom display | match | |
| Pane scrollbar | match | |
| Selection highlight, copy mode cursor and search highlights | match | |
| Link hover and click (OSC 8 and detected URLs) | match | |
| Popup pane (custom command `popup`) | hc | |
| Copy feedback toast ("copied") | match | |
| Config diagnostic banner | match | Text says `rodeo config check`. |
| Agent notification toasts | match | |
| Endpoint notices and lifecycle banner | hc | From machine states in `machine.list` and the events stream. |
| Unavailable state ("Select a connected machine") | hc | When hc is not reachable or a machine's doc cannot be read. The rodeo hat and wordmark show centered above the status line. |
| Kitty graphics | match | Pass-through to the host terminal with pane offset and clipping. Toasts, banners, the mode bar, popups, overlays, selections and copy-mode highlights cut the image to their exact cells, as in Herdr. Images inside a popup terminal are not shown. |

## Overlays

| Overlay | Status | Notes |
|---|---|---|
| Keybind help with filter | match | |
| Goto picker (session navigator) with search and state filters | match | |
| Rename dialog (new workspace, rename workspace, new tab, rename tab, rename pane) | match | |
| Confirm close | match | |
| Worktree create, open, remove | hc | `machine.exec git`. |
| Context menu: workspace, tab, pane | match | Pane menu "Send right-clicks to pane" writes the `herdr.right-click=program` label the app also reads. |
| Global menu | match | Items: settings, keybinds, reload config, detach. "update ready" and "what's new" are dropped. |
| Settings: theme, indicators, sound, toast | match | Writes rodeo's `config.toml`. |
| Settings: integrations | dropped | Agent integrations belong to the app and hc (OSC 7501 reporting). |
| Onboarding | dropped | Its only job was to lead to integrations. `onboarding` in config is accepted and ignored. |
| Product announcement | dropped | Herdr product news. |
| Release notes, "what's new", "update ready" | dropped | No self-update. |

## Mouse

| Behavior | Status | Notes |
|---|---|---|
| Click sidebar rows, tabs, panes, agents; act on release | match | |
| Drag split borders | match | Writes `size.weight` to the doc on release. |
| Drag sidebar edge and section split | match | |
| Drag tab reorder | match | |
| Wheel scroll in panes, sidebar, overlays | match | Programs with mouse reporting get the wheel. |
| Text selection, double-click word, `copy_on_select` | match | |
| Right-click menus, `right_click_passthrough_modifier` | match | |
| `mouse_capture = false` | match | |
| Scrollbar click and drag | match | |

## Titles, notifications, sound

| Feature | Status | Notes |
|---|---|---|
| Outer window title (`ui.window_title` tokens) | match | Drawn by rodeo. `{hostname}` is the focused space's machine. |
| Pane terminal titles | hc | hc `title` event. |
| Toast delivery `off`, `herdr`, `terminal`, `system` | match | Fired on agent state changes from hc events. `system`: `terminal-notifier`, then `osascript` on macOS; `notify-send`, then `gdbus` on Linux. A system notification click does not focus the pane (Herdr does that on Windows only). |
| Sound per state and per agent | match | Players: `afplay` on macOS; `paplay`, `pw-play`, `aplay` (built-in sounds only), `ffplay`, `mpg123`, `mpv` on Linux. The built-in sounds are short chimes that rodeo makes itself, not Herdr's mp3 files. `RODEO_DISABLE_SOUND` or `HERDR_DISABLE_SOUND` turns playback off. |
| Bell pass-through | match | Only for attached panes; BEL is in the attach stream only. |
| Host light/dark detection and `theme.auto_switch` | match | |

## Config

rodeo reads `~/.config/rodeo/config.toml` with Herdr's schema. On first start it copies `~/.config/herdr/config.toml` if its own file does not exist. Same load rules: startup all-or-nothing, reload per section, soft diagnostics, unknown key warnings (`herdr-keys-config.md` section 3). `RODEO_CONFIG_PATH` replaces `HERDR_CONFIG_PATH`. Client preferences (sidebar width, collapsed state, section split, agent sort, collapsed groups) go to `~/.local/state/rodeo/`.

| Section | Status | Notes |
|---|---|---|
| `onboarding` | dropped | Accepted, ignored. |
| `[theme]` all fields, 18 themes, custom tokens, light/dark overrides | match | |
| `terminal.default_shell`, `terminal.shell_mode` | hc | Empty: hc runs the machine's shell (empty argv). Set: rodeo sends it as argv (with `-l` for `login`). |
| `terminal.new_cwd` | match | rodeo computes the folder and always sends an absolute path. |
| `terminal.kitty_graphics` (and `experimental.kitty_graphics`) | match | |
| `[session]` | hc | Agent resume and restore belong to hc (kept terminals). Accepted, ignored. |
| `[server]` | hc | Headless size is hc's `headless` config. Accepted, ignored. |
| `[update]` | dropped | No self-update. Accepted, ignored. |
| `[keys]` | match | See Key binds. |
| `[ui]` layout, mouse, cursor, borders, tab bar, title, agent panel, indicators, accent | match | |
| `ui.sidebar.*` | match | |
| `ui.toast.*`, `ui.sound.*` | match | |
| `worktrees.directory` | match | |
| `advanced.scrollback_limit_bytes` | match | Limits the local VT scrollback. |
| `remote.manage_ssh_config` | hc | hc machine links own SSH. Accepted, ignored. |
| `experimental.allow_nested` | match | Checks `HC_TERMINAL_ID` instead of `HERDR_ENV`. |
| `experimental.pane_history` | hc | hc keeps screens. Accepted, ignored. |
| `experimental.reveal_hidden_cursor_for_cjk_ime`, `cjk_ime_agents`, `cjk_ime_cursor_shape` | match | |
| `experimental.switch_ascii_input_source_in_prefix` | dropped | Accepted, ignored. Herdr calls macOS Carbon input source APIs (TIS) over FFI and pumps the main run loop. rodeo denies `unsafe_code`, and the switch cannot be tested on the Linux build host. |

## CLI

| Herdr 0.9.5 | Status | Notes |
|---|---|---|
| `herdr` (attach default session) | match | `rodeo` |
| `herdr session list/attach` | hc | `rodeo --session <name>`; `hc` lists sessions. |
| `herdr --remote` | hc | Run `rodeo` on the remote host, or add it as an hc machine. |
| `herdr server stop/reload-config` | hc | `hc server stop`. Reload is the `reload_config` key. |
| `herdr --default-config` | match | `rodeo --default-config` |
| `herdr config check` | match | `rodeo config check` |
| `herdr config reset-keys` | match | `rodeo config reset-keys` |
| `herdr plugin ...` | dropped | No plugins. |
| `herdr update` | dropped | No self-update. |
| Socket API and pane/workspace CLI commands | dropped | `hc` and `herdr-app do` cover programs and layout. |
| Agent integrations install | dropped | Belongs to the app and hc. |
