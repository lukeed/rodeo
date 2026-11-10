# Herdr TUI v0.9.5: keybindings and config inventory

Source: [`herdrdev/herdr`](https://github.com/herdrdev/herdr/tree/v0.9.5) at tag `v0.9.5` (commit `2ecf19a92fc3a7fff75b6ce3655ef04f4625ec01`). All `file:line` citations are relative to that repo. The code is the source of truth. Where the docs (`docs/preview/website/...`) or the generated reference (`docs/preview/website/src/data/config-reference.json`) add detail that agrees with the code, this file uses it.

Architecture note: in v0.9.5 all key handling for the TUI happens in the client shell (`src/client/shell/*`). `src/app/actions.rs` is server-side state logic (workspace/pane mutation, notifications, link/word detection). It has no key dispatch. The client shell turns a key into a `KeybindAction` (`src/input/keybindings.rs:20-73`) and then either changes local UI state or sends a socket API method to the server (`src/client/shell/actions.rs:4-180`, `:861-1119`).

## 0. Counts

- `[keys]` fields: 63 scalar keys (1 `prefix`, 58 bindable action fields, 1 `remote_image_paste`, 3 `keys.indexed.*`), plus the `[[keys.command]]` array (6 fields per entry) and 1 hidden field `extra_prefixes`. Source: `src/config/model.rs:335-465`, `:467-598`, `:857-866`, `src/config/keybinds.rs:121-137`.
- `KeybindAction` enum: 52 variants (`src/input/keybindings.rs:20-73`). The 6 `navigate_*` fields have no variant of their own; they map to workspace moves or `FocusPane*` inside navigate mode.
- Config options (leaf TOML paths, without `[keys]`): 147 canonical paths (same set as `docs/preview/website/src/data/config-reference.json`), plus 1 Windows-only path (`server.allow_unelevated_clients`), plus 4 legacy paths (`keys.fullscreen`, `advanced.scrollback_lines`, `ui.agent_panel_scope`, `ui.toast.enabled`), plus the fields of the inline tables (sidebar token style: 5, sidebar rule: 10, tab bar entry: 6, custom command: 6). The 147 break down as: 1 top level, 61 `theme`, 4 `terminal`, 2 `session`, 2 `server`, 3 `update`, 64 `ui` (incl. 22 sound agents), 1 `worktrees`, 1 `advanced`, 1 `remote`, 7 `experimental`. Detail in section 2.

## 1. Keybindings

### 1.1 Binding string syntax

Source: `src/config/keybinds.rs:1124-1353`, `:45-109`.

- A value is a string or an array of strings (`BindingConfig`, untagged, `keybinds.rs:45-50`). Every string in the array is one binding for the action.
- `prefix+<combo>`: the binding fires in prefix mode (press prefix, release, press combo). Only the leading literal `prefix+` is special (`keybinds.rs:1126`).
- `<combo>` without `prefix+`: a direct binding. It fires in terminal mode without the prefix.
- A combo is `+`-separated tokens: zero or more modifiers, then exactly one key. A second non-modifier token or an empty token is invalid (`keybinds.rs:1291-1309`). Tokens are trimmed and compared case-insensitively for modifiers and named keys.
- Modifiers (`keybinds.rs:1231-1240`):
  - `ctrl`, `control` -> Ctrl
  - `shift` -> Shift
  - `alt`, `option`, `meta` -> Alt (note: `meta` means Alt here, not the Meta modifier)
  - `cmd`, `command`, `super` -> Super
  - `hyper` -> Hyper
- Named keys (`keybinds.rs:1312-1349`): `space` (or a literal space), `enter`/`return`, `esc`/`escape`, `tab`, `backspace`/`bs`, `left`, `right`, `up`, `down`, `minus` (`-`), `comma` (`,`), `period` (`.`), `slash` (`/`), `backslash` (`\`), `quote` (`'`), `double_quote`/`double-quote` (`"`), `semicolon` (`;`), `colon` (`:`), `percent` (`%`), `ampersand` (`&`), `backtick` (`` ` ``), `plus` (`+`), `f<N>` where N parses as `u8` (so `f0`..`f255` all parse).
- Any single character is a key, including non-ASCII (`keybinds.rs:1339-1347`). An uppercase ASCII letter adds Shift and lowercases the key (`"N"` == `"shift+n"`).
- `shift+tab` normalizes to BackTab with no Shift (`keybinds.rs:1316-1319`, `:1406-1414`).
- Literal plus: `"+"` alone, or `"<mods>++"` (for example `ctrl++`) (`keybinds.rs:1285-1290`).
- Not supported as key names: `delete`, `home`, `end`, `pageup`, `pagedown`, `insert`. They fail to parse (no match in the table, not single chars, and `home`/`end`/`delete` do not start with `f`). These keys are only used by built-in, non-configurable handlers.
- Range syntax for indexed actions: a `1..9` token in place of the key, with optional modifiers and optional `prefix+`, for example `prefix+1..9`, `prefix+shift+1..9`, `ctrl+alt+1..9`. It expands to nine bindings for `1`..`9` (`keybinds.rs:1132-1155`, `:1242-1257`). A range in a non-indexed field gives the diagnostic `range keybinding is only valid for indexed actions` and is disabled (`keybinds.rs:881-885`).
- Indexed fields (`switch_tab`, `switch_workspace`, `focus_agent`) also accept single bindings, but the key must be a digit `1`..`9` (`keybinds.rs:979-987`).
- Unbind: `""` or `[]`. Empty and blank strings are skipped (`keybinds.rs:868-872`). A user value of `""` still counts as "user-configured", so it removes the default.
- Display labels are re-formatted from the parsed combo, in the order `ctrl`, `alt`, `shift`, `super` (shown as `cmd` on macOS), `hyper`, `meta`, key; `prefix+` is kept (`keybinds.rs:1173-1229`). Multiple bindings show joined with ` / ` (`keybinds.rs:274-281`).

### 1.2 Validation, conflicts and fallbacks

Source: `src/config/keybinds.rs:498-799`, `:1038-1122`, `:1365-1392`.

- Prefix (`keys.prefix`): string or array. Each entry is parsed with the combo parser (not the `prefix+` parser). Invalid entries are dropped with `invalid keybinding: keys.prefix = "..."; ignoring prefix`. Duplicates are dropped. If no valid entry remains (including an explicit empty array), the prefix falls back to `ctrl+b` and a rejection diagnostic `invalid keybinding: keys.prefix; using fallback ctrl+b` is emitted (`keybinds.rs:1368-1392`). On a live reload, a prefix rejection keeps the whole current keymap ("kept current keybinds") (`src/config.rs:167-176`, `src/app/mod.rs:862-883`). At startup the client uses the fallback (`src/client/shell/config.rs:138-144`).
- The first prefix is the primary one, shown in the compact status bar. All prefixes enter the same prefix mode (`keybinds.rs:16-43`).
- Every prefix combo is reserved as a direct key, so no direct binding can take it (`keybinds.rs:461-468`).
- A `prefix+X` binding where `X` equals a prefix key is rejected: pressing the prefix twice sends a literal prefix key to the pane (`keybinds.rs:1086-1097`).
- Two passes: all user-configured fields first, then defaults (`keybinds.rs:651`). A default that collides with a user binding is dropped silently. A user prefix that collides with a default `prefix+X` drops the default silently.
- Same-source conflicts: the first registered field wins; the later one is disabled with `<label>: kept <field>, disabled <field>` (`keybinds.rs:1099-1108`). Registration order is the order in `keybinds.rs:651-772`: navigate fields (separate registry), then `help`, `settings`, `new_workspace`, `new_worktree`, `open_worktree`, `remove_worktree`, `rename_workspace`, `close_workspace`, `workspace_picker`, `goto`, `detach`, `reload_config`, `open_notification_target`, `previous_workspace`, `next_workspace`, `previous_agent`, `next_agent`, `focus_agent`, `new_tab`, `rename_tab`, `previous_tab`, `next_tab`, `move_tab_previous`, `move_tab_next`, `switch_tab`, `switch_workspace`, `close_tab`, `rename_pane`, `edit_scrollback`, `clear_pane`, `copy_mode`, `focus_pane_*`, `swap_pane_*`, `last_pane`, `cycle_pane_next`, `cycle_pane_previous`, `split_vertical`, `split_horizontal`, `close_pane`, `zoom`, `resize_mode`, `resize_pane_*`, `toggle_sidebar`, legacy `[keys.indexed]`, then `[[keys.command]]` (user pass only).
- Unsafe direct binding: a direct binding whose key is a printable character with no modifier other than Shift is rejected with `unsafe direct keybinding: ... use "prefix+X" to require the prefix; disabling binding` (`keybinds.rs:1110-1119`, `:1569-1572`). This applies to action fields, indexed fields and custom commands, not to navigate fields.
- Navigate fields (`navigate_*`) use their own registry (`keybinds.rs:517-519`). They must not use `prefix+` and must not be `esc` (`keybinds.rs:1045-1063`). They also cannot use the reserved navigate runtime keys `esc`, `enter`, `tab`, `shift+tab`/BackTab, `left`, `right`, unmodified `1`..`9` (`keybinds.rs:779-799`), or any prefix key. Plain printable keys are allowed. They do not conflict with the general action bindings (for example `focus_pane_down = "prefix+j"` and `navigate_pane_down = "j"` coexist).
- Legacy `[keys.indexed]` values are modifier-only combos (`ctrl`, `ctrl+shift`, ...). Invalid ones give `invalid indexed keybinding` (`keybinds.rs:998-1036`). If a legacy value is set (non-blank), the modern default for that target is not applied, even if the legacy value is invalid (`keybinds.rs:614-636`).
- `keys.remote_image_paste` is a plain string, not an array. Empty disables it. Invalid gives `invalid keybinding: keys.remote_image_paste = ...; disabling binding` (`src/config.rs:157-165`).
- Every diagnostic is also logged with `tracing::warn!`.

### 1.3 Key matching rules

Source: `src/config/keybinds.rs:1406-1567`, `src/input/keybindings.rs:75-255`.

- Combos are normalized: Shift+Tab == BackTab (`keybinds.rs:1406-1414`).
- Exact modifier match is required, except for the shift rules below.
- Shifted letters: `shift+h` matches `H` sent with or without the Shift flag (legacy terminals send `H` with no modifier) (`keybinds.rs:1458-1502`). A plain `h` binding does not match `H`.
- Shifted punctuation: when the terminal reports the shifted code point (kitty protocol), the binding for `?` matches Shift+`/` (`keybinds.rs:1529-1567`).
- Indexed bindings with Shift also match the legacy shifted digit symbols `!@#$%^&*(` (`keybinds.rs:310-326`, `:1504-1527`).
- Prefix-mode fallback: if no prefix binding matches the key, the resolver tries again with the single character the host says the key typed (`generated_text`), with no modifiers (`src/input/keybindings.rs:82-91`, `:234-244`). This makes `prefix+?` work on layouts where `?` is Shift+something.
- Resolution order for one key (`src/input/keybindings.rs:223-232`): non-indexed actions in the fixed order of `resolve_non_indexed_action` (`:98-157`), then custom commands (first match in config order), then indexed actions (`switch_tab`, then `switch_workspace`, then `focus_agent`; exact modifier match first, then loose) (`:180-221`).

### 1.4 `[keys]` action table

Defaults: `src/config/model.rs:1120-1188`. Doc comments: `model.rs:335-465`. Default template comments: `src/main.rs:135-223`. Effect: `src/client/shell/actions.rs:4-180` and `:861-1119`, `src/client/shell/endpoint_navigation.rs:103-226`. Help labels: `src/input/keybind_help.rs:75-229`.

"Mode" is where the binding fires. "prefix" = after the prefix key. "direct" = in terminal mode with no prefix. A `prefix+X` binding also fires in navigate mode when you press `X` alone (section 1.8.3). "Help label" is the text in the keybind help overlay; "(not listed)" means the help overlay does not show the action.

| Config key | Default | Mode | What it does | Help label |
| --- | --- | --- | --- | --- |
| `prefix` | `"ctrl+b"` | terminal, copy | Enters prefix mode. String or array. In prefix mode, pressing any prefix key again sends that key to the pane and leaves prefix mode. | `prefix mode` (lists all prefixes joined by ` / `) |
| `help` | `"prefix+?"` | prefix | Opens the keybind help overlay (`actions.rs:51-58`). | `keybinds` |
| `settings` | `"prefix+s"` | prefix | Opens the settings overlay on the Theme section (`actions.rs:60-63`, `settings.rs:35-46`). | `settings` |
| `new_workspace` | `"prefix+shift+n"` | prefix | If `ui.prompt_new_workspace_name` is true, opens the "new workspace" name dialog prefilled with a name from the cwd; else creates the workspace at once and focuses it. Source workspace is the navigate-mode selection, else the focused one (`actions.rs:83-102`, `overlay_input.rs:318-357`). | `new workspace` |
| `new_worktree` | `"prefix+shift+g"` | prefix | Lists worktrees of the selected workspace's repo, then opens the "new worktree" dialog (branch field, path preview). Error if the workspace is a linked worktree (`worktrees.rs:167-229`). | `new worktree` |
| `open_worktree` | unset (`""`) | prefix | Opens the "open worktree" list for the repo. Same linked-worktree error. | `open worktree` |
| `remove_worktree` | unset (`""`) | prefix | Opens the delete-worktree-checkout confirmation. Error if the workspace is not a Herdr-managed linked worktree. | `delete worktree checkout` |
| `rename_workspace` | `"prefix+shift+w"` | prefix | Opens the "rename workspace" dialog (`overlay_input.rs:359-378`). Blocked on a foreign-endpoint preview in navigate mode (`actions.rs:23-36`). | `rename workspace` |
| `close_workspace` | `"prefix+shift+d"` | prefix | Closes the selected workspace. With `ui.confirm_close` it opens the "Close workspace?" (or "Close worktree group?") dialog (`overlay_input.rs:997-1022`). | `close workspace` |
| `workspace_picker` | `"prefix+w"` | prefix | Enters navigate mode with the focused workspace selected (`actions.rs:151-160`). | `workspace navigation` |
| `goto` | `"prefix+g"` | prefix | Opens the session navigator ("Goto picker") overlay (`actions.rs:46-50`, `overlay_input.rs:193-208`). | `session navigator` |
| `navigate_workspace_up` | `"up"` | navigate | Moves the workspace selection up. Wraps on desktop, clamps on mobile (`workspace_navigation.rs:99-164`). | `<up> / <down>` = `workspace list` |
| `navigate_workspace_down` | `"down"` | navigate | Moves the workspace selection down. | (same row) |
| `navigate_pane_left` | `"h"` | navigate | Focuses the pane to the left; stays in navigate mode. Left arrow is a fixed alias. | `<h> / <j> / <k> / <l> / left / right` = `move focus` |
| `navigate_pane_down` | `"j"` | navigate | Focuses the pane below; stays in navigate mode. | (same row) |
| `navigate_pane_up` | `"k"` | navigate | Focuses the pane above; stays in navigate mode. | (same row) |
| `navigate_pane_right` | `"l"` | navigate | Focuses the pane to the right; stays in navigate mode. Right arrow is a fixed alias. | (same row) |
| `detach` | `"prefix+q"` | prefix | Detaches the client; the server keeps running (`actions.rs:10-12`). | `detach` |
| `reload_config` | `"prefix+shift+r"` | prefix | Sends `server.reload_config` to the active server and reloads the client's local config (`actions.rs:69-82`). | `reload config` |
| `open_notification_target` | `"prefix+o"` | prefix | Focuses the pane of the visible notification (`actions.rs:65-68`). | `open notification target` |
| `previous_workspace` | unset | prefix/direct | Focuses the previous workspace in sidebar order, wrapping. Across all online machines when more than one is connected (`actions.rs:943-965`, `endpoint_navigation.rs:112-165`). | `previous workspace` |
| `next_workspace` | unset | prefix/direct | Focuses the next workspace, wrapping. | `next workspace` |
| `previous_agent` | unset | prefix/direct | Focuses the previous agent in Agent panel order (`ui.agent_panel_sort`), wrapping; scrolls the panel to reveal it (`actions.rs:902-932`). | `previous agent` |
| `next_agent` | unset | prefix/direct | Focuses the next agent, wrapping. | `next agent` |
| `focus_agent` | unset | prefix/direct (indexed) | Focuses agent N (1-9) in Agent panel order. Example `"prefix+alt+1..9"`. | `focus agent 1-9` |
| `remote_image_paste` | `"ctrl+v"` | direct, only in a `herdr --remote` client | When this exact key press arrives alone, the client sends the local clipboard image to the remote server (`src/client/clipboard_images.rs:45-120`). Empty disables. Single string only. | (not listed) |
| `new_tab` | `"prefix+c"` | prefix | If `ui.prompt_new_tab_name` is true, opens the "new tab" dialog prefilled with the next number; else creates and focuses a tab (`actions.rs:135-140`, `:1021-1029`). | `new tab` |
| `rename_tab` | `"prefix+shift+t"` | prefix | Opens the "rename tab" dialog (`overlay_input.rs:404-423`). | `rename tab` |
| `previous_tab` | `"prefix+p"` | prefix | Focuses the previous tab in the focused workspace, wrapping (`actions.rs:976-993`). | `previous tab` |
| `next_tab` | `"prefix+n"` | prefix | Focuses the next tab, wrapping. | `next tab` |
| `move_tab_previous` | unset | prefix/direct | Moves the focused tab one place toward the front; the first tab wraps to the end (`actions.rs:994-1020`). | `move tab left` |
| `move_tab_next` | unset | prefix/direct | Moves the focused tab one place toward the back; the last tab wraps to the front. | `move tab right` |
| `switch_tab` | `"prefix+1..9"` | prefix (indexed) | Focuses tab N of the focused workspace (`actions.rs:966-975`). | `switch tab 1-9` |
| `switch_workspace` | unset | prefix/direct (indexed) | Focuses workspace N in sidebar order (`actions.rs:933-942`). | `switch workspace 1-9` |
| `close_tab` | `"prefix+shift+x"` | prefix | Closes the focused tab. With `ui.confirm_close`, closing the last tab of a workspace asks first (`overlay_input.rs:1058-1078`). | `close tab` |
| `rename_pane` | `"prefix+shift+p"` | prefix | Opens the "rename pane" dialog. Saving an empty name sends an empty label (`overlay_input.rs:425-445`, `:984-989`). | `rename pane` |
| `edit_scrollback` | `"prefix+e"` | prefix | Opens the focused pane's scrollback in `$EDITOR` (server method `pane.edit_scrollback`) (`actions.rs:1106-1108`). | `edit scrollback` |
| `clear_pane` | unset | prefix/direct | Clears screen and scrollback of the focused pane, keeping the cursor line. Sends no input to the program. Does nothing on the alternate screen (`actions.rs:1103-1105`; behavior text from `config-reference.json`). Note: its default is `BindingConfig::default()`, same as empty (`model.rs:1160`). | `clear pane` |
| `copy_mode` | `"prefix+["` | prefix | Enters copy mode for the focused pane (`copy_mode.rs:12-102`). | `copy mode` |
| `focus_pane_left` | `"prefix+h"` | prefix | Focuses the pane to the left (`pane.focus_direction`). | `focus pane left` |
| `focus_pane_down` | `"prefix+j"` | prefix | Focuses the pane below. | `focus pane down` |
| `focus_pane_up` | `"prefix+k"` | prefix | Focuses the pane above. | `focus pane up` |
| `focus_pane_right` | `"prefix+l"` | prefix | Focuses the pane to the right. | `focus pane right` |
| `swap_pane_left` | `"prefix+shift+h"` | prefix | Swaps the focused pane with the pane to the left (`actions.rs:1039-1047`). | (not listed) |
| `swap_pane_down` | `"prefix+shift+j"` | prefix | Swaps with the pane below. | (not listed) |
| `swap_pane_up` | `"prefix+shift+k"` | prefix | Swaps with the pane above. | (not listed) |
| `swap_pane_right` | `"prefix+shift+l"` | prefix | Swaps with the pane to the right. | (not listed) |
| `cycle_pane_next` | `"prefix+tab"` | prefix | Focuses the next pane in the focused tab, wrapping (`actions.rs:1064-1087`). | `cycle pane next` |
| `cycle_pane_previous` | `"prefix+shift+tab"` | prefix | Focuses the previous pane, wrapping. | `cycle pane previous` |
| `last_pane` | unset | prefix/direct | Focuses the previously focused pane, across workspaces and tabs (client-tracked `previous_pane_id`) (`actions.rs:1088-1098`). | `last pane` |
| `split_vertical` | `"prefix+v"` | prefix | Splits the focused pane side by side (new pane to the right) and focuses it (`actions.rs:1048-1063`). | `split vertical` |
| `split_horizontal` | `"prefix+minus"` | prefix | Splits the focused pane stacked (new pane below) and focuses it. | `split horizontal` |
| `close_pane` | `"prefix+x"` | prefix | Closes the focused pane. With `ui.confirm_close`, closing the only pane of the only tab asks first (`overlay_input.rs:1024-1056`). | `close pane` |
| `zoom` | `"prefix+z"` | prefix | Toggles zoom of the focused pane. Legacy alias key name: `fullscreen` (`model.rs:444`). | `zoom pane` |
| `resize_mode` | `"prefix+r"` | prefix | Enters resize mode. Pressing the same binding again inside resize mode leaves it (`input.rs:1047-1056`). | `resize mode` |
| `resize_pane_left` | unset | prefix/direct | Resizes the focused pane toward the left by the server default amount (`actions.rs:1109-1116`). | `resize pane left` |
| `resize_pane_down` | unset | prefix/direct | Resizes downward. | `resize pane down` |
| `resize_pane_up` | unset | prefix/direct | Resizes upward. | `resize pane up` |
| `resize_pane_right` | unset | prefix/direct | Resizes toward the right. | `resize pane right` |
| `toggle_sidebar` | `"prefix+b"` | prefix | Collapses or expands the sidebar. Saved to client preferences (section 3.5) (`actions.rs:13-21`). | `toggle sidebar` |
| `indexed.tabs` | unset (`""`) | direct (legacy) | Modifier combo; `ctrl` makes `ctrl+1..9` switch tabs. | shown via `switch tab 1-9` |
| `indexed.workspaces` | unset | direct (legacy) | Modifier combo for workspaces 1-9. | shown via `switch workspace 1-9` |
| `indexed.agents` | unset | direct (legacy) | Modifier combo for agents 1-9. | shown via `focus agent 1-9` |
| `command` | `[]` | prefix/direct | Custom commands, section 1.5. | group `custom`, label = description or `custom command` |
| `extra_prefixes` | (none) | terminal | Hidden. Extra prefix keys, merged into `prefix` (`model.rs:472-476`, `:619-636`). Herdr writes it into the keybinding profile it publishes to old clients; the user config parser also accepts it. Not in docs. | - |

Unset actions can be bound with `prefix+` or a modified direct chord. "prefix/direct" above means the action has no default, so the mode depends on the binding the user writes. Any action can have direct bindings; the docs recommend `ctrl+alt+...` chords (`docs/.../keyboard.mdx`).

The `KeybindAction` enum (`src/input/keybindings.rs:20-73`) has these 52 variants: `NewWorkspace`, `NewWorktree`, `OpenWorktree`, `RemoveWorktree`, `RenameWorkspace`, `CloseWorkspace`, `SwitchWorkspace(usize)`, `SwitchTab(usize)`, `FocusAgent(usize)`, `WorkspacePicker`, `PreviousWorkspace`, `NextWorkspace`, `PreviousAgent`, `NextAgent`, `NewTab`, `RenameTab`, `PreviousTab`, `NextTab`, `MoveTabPrevious`, `MoveTabNext`, `CloseTab`, `RenamePane`, `FocusPaneLeft`, `FocusPaneDown`, `FocusPaneUp`, `FocusPaneRight`, `SwapPaneLeft`, `SwapPaneDown`, `SwapPaneUp`, `SwapPaneRight`, `SplitVertical`, `SplitHorizontal`, `ClosePane`, `EditScrollback`, `ClearPane`, `CopyMode`, `Zoom`, `EnterResizeMode` (= `resize_mode`), `ResizePaneLeft`, `ResizePaneDown`, `ResizePaneUp`, `ResizePaneRight`, `ToggleSidebar`, `CyclePaneNext`, `CyclePanePrevious`, `LastPane`, `Help`, `Settings`, `ReloadConfig`, `OpenNotificationTarget`, `Detach`, `OpenNavigator` (= `goto`). Every variant appears in the table above.

### 1.5 Custom commands: `[[keys.command]]`

Source: `src/config/keybinds.rs:111-150`, `:801-858`, `src/popup_size.rs:6-42`, `:104-150`, `src/client/shell/actions.rs:181-267`, `src/main.rs:205-216`.

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `key` | binding (string or array) | `""` | Same syntax as actions. `prefix+...` or direct. Ranges are rejected. |
| `command` | string | `""` | Required. Blank gives `empty custom command: keys.command[N].command; disabling custom command`. For `plugin_action` it is the plugin action id. |
| `type` | `"shell"` \| `"pane"` \| `"popup"` \| `"plugin_action"` | `"shell"` | `shell`: run detached in the background (`/bin/sh -lc` on Unix, `cmd.exe /d /c` on Windows). `pane`: temporary zoomed pane, closed when the command exits (`/bin/sh -c`). `popup`: session-modal terminal popup over the tab layout; it takes all input, including Esc and the prefix, until the command exits. `plugin_action`: invoke a plugin action; the client sends the visible mouse selection of the focused pane with it. |
| `description` | string | none | Label in the keybind help "custom" group. Default label: `custom command`. |
| `width` | integer cells (0-65535) or `"N%"` string (1-100) | half the area, min 6 | Popup only. On other types it is ignored with `popup size on non-popup custom command` diagnostic. |
| `height` | same | half the area, min 4 | Popup only. |

Custom commands run on the server where the panes live. They get `HERDR_SOCKET_PATH`, `HERDR_BIN_PATH`, `HERDR_ACTIVE_WORKSPACE_ID`, `HERDR_ACTIVE_TAB_ID`, `HERDR_ACTIVE_PANE_ID`, `HERDR_ACTIVE_PANE_CWD` (docs `configuration.mdx`, "Custom command keybindings"). The client matches its binding to a command id published in the server snapshot; if none matches it shows `custom command is not available on this endpoint; reload configuration` (`actions.rs:184-209`).

### 1.6 Legacy `[keys.indexed]`

Source: `src/config/model.rs:857-866`, `src/config/keybinds.rs:614-636`, `:738-763`, `:998-1036`.

`tabs`, `workspaces`, `agents`: each a modifier-only string (for example `"ctrl"`, `"ctrl+shift"`, `"alt"`). Each expands to nine direct bindings `<mods>+1`..`<mods>+9` appended to `switch_tab`, `switch_workspace`, `focus_agent`. A non-blank legacy value stops the modern default of that target (for example `tabs = "ctrl"` removes `prefix+1..9`).

### 1.7 Modes and dispatch

Source: `src/client/shell/input.rs:378-446`, `:610-739`. Modes: `Terminal`, `Prefix`, `Navigate`, `Resize`, `Copy` (`src/client/shell/state.rs:271-277`).

Order for each key press (`input.rs:610-739`):

1. Modal paste shortcut (section 1.10).
2. Onboarding, product announcement and release notes overlays take the key.
3. An open popup (custom command `type = "popup"`) takes every key.
4. While a popup is starting, keys are dropped.
5. Any other open overlay takes the key (section 1.9).
6. Modifier-only key events are ignored.
7. When `ui.copy_on_select = false` and a mouse selection is visible: `ctrl+c` or `super+c` copies it and clears it (`input.rs:646-661`). Outside copy mode, any other key clears a visible mouse selection, then continues.
8. The mode handler.

#### 1.7.1 Terminal mode

- Direct bindings first (actions, then custom commands, then indexed).
- Then the prefix key enters prefix mode.
- Else the key goes to the focused pane.

#### 1.7.2 Prefix mode

- Any prefix key again: send that key to the pane, return to the previous mode.
- `esc`: cancel, return to the previous mode.
- A `prefix+X` binding: run it, return to the previous mode.
- Any other key: dropped; return to the previous mode.
- "Previous mode" is copy mode when the focused pane is in copy mode, else terminal.

#### 1.7.3 Navigate mode (entered with `workspace_picker`)

Source: `input.rs:751-969`. Fixed keys and their order:

| Key | Effect |
| --- | --- |
| `esc` or any prefix key | Leave navigate mode (to copy or terminal). |
| `navigate_workspace_up` / `_down` | Move the workspace selection. |
| `enter` | Focus the selected workspace; return to terminal mode (`workspace_navigation.rs:166-202`). With no selection it just leaves the mode. |
| (foreign machine preview selected) | Every other key except `ctrl+[` shows "Confirm workspace first"; `ctrl+[` cancels like `esc`. |
| `1`..`9` (no modifiers) | Focus workspace N and return to terminal mode, if it exists. |
| `tab` | Focus next pane in the tab, then leave navigate mode. |
| `shift+tab` | Focus previous pane, then leave navigate mode. |
| `left` / `right` | Focus pane left/right; stay in navigate mode. |
| `navigate_pane_*` | Focus pane in that direction; stay in navigate mode. |
| Right-hand side of any `prefix+X` action (without the prefix) | Run the action, then leave navigate mode. The four `focus_pane_*` actions are excluded here. Custom commands with `prefix+X`, then indexed `prefix+...` bindings, are tried next. |
| `ctrl+[` | Fallback cancel when nothing else matched. |
| anything else | Ignored; stay in navigate mode. |

An indexed action whose target does not exist (no tab N, no workspace N, no agent N) does nothing and keeps the mode (`input.rs:949-1008`).

#### 1.7.4 Resize mode (entered with `resize_mode`)

Source: `input.rs:1042-1070`.

| Key | Effect |
| --- | --- |
| `esc`, `enter`, or any `resize_mode` binding: the bare right-hand side of its `prefix+X` form (default `r`) or a direct binding | Leave resize mode. |
| `h` or `left` | Resize left. |
| `j` or `down` | Resize down. |
| `k` or `up` | Resize up. |
| `l` or `right` | Resize right. |
| anything else, including the prefix key | Ignored; stay in resize mode. |

The match uses the key code only, so modifiers are ignored (for example `ctrl+h` also resizes left). Not configurable: the `resize_pane_*` fields do not change these keys.

#### 1.7.5 Copy mode (entered with `copy_mode`)

Source: `src/client/shell/copy_mode.rs:104-305`, `src/copy_mode.rs:33-86`. The prefix key still enters prefix mode, except while the search prompt is open (`input.rs:724-736`). No direct bindings run in copy mode. Character commands accept Shift only; shifted characters resolve by the reported shifted code point or a US-layout table (`src/copy_mode.rs:43-86`).

| Key | Effect |
| --- | --- |
| `esc` | If a selection or search is active, clear them; else leave copy mode without copying. |
| `q` | Leave without copying. |
| `enter`, `y` | Copy the selection (or the current search match when there is no selection) and leave. If a search is still pending, copy after it finishes. |
| `v`, `space` | Start a character selection at the cursor. |
| `V` | Start a line selection. |
| `h` `j` `k` `l`, arrows | Move the cursor. |
| `w` `b` `e` | Next word start, previous word start, next word end. |
| `W` `B` `E` | Same for big words. |
| `{` `}` | Previous / next paragraph. |
| `0`, `home` | Line start. |
| `^` | First non-blank. |
| `$`, `end` | Line end. |
| `g` / `G` | Top / bottom of history. |
| `pageup`, `ctrl+b` | Page up (height - 2 lines). |
| `pagedown`, `ctrl+f` | Page down. |
| `ctrl+u` / `ctrl+d` | Half page up / down. |
| `/` / `?` | Open forward / backward search prompt. |
| `n` / `N` | Repeat search in the same / opposite direction. |

Search prompt: `esc` closes it, `enter` submits, other keys edit the query with the text field keys (section 1.10). Search is literal, case-insensitive unless the query has an uppercase letter (docs `keyboard.mdx`). With the default prefix `ctrl+b`, `ctrl+b` enters prefix mode and does not page up.

### 1.8 Overlays (dialogs, pickers, menus)

Source: `src/client/shell/overlay_input.rs:473-931`, `src/client/shell/settings.rs:350-399`, `src/client/shell/worktrees.rs:46-165`. Overlay kinds: `src/client/shell/state.rs:280-294`. None of these keys are configurable. Matching usually normalizes the combo first, so Shift+Tab equals BackTab.

#### Keybind help (`help`)

| State | Key | Effect |
| --- | --- | --- |
| browse | `esc`, `enter`, `?` | Close. |
| browse | `/` | Focus the filter field. |
| browse | `up`/`k`, `down`/`j` | Scroll 1. |
| browse | `pageup`/`pagedown` | Scroll 8. |
| browse | `home`/`end` | Top / bottom. |
| filter | text field keys | Edit the filter; scroll resets to top. |
| filter | `esc` | Clear the filter and leave the field. |
| filter | `enter` | Close. |
| filter | `up`/`down`, `ctrl+p`/`ctrl+n` | Scroll 1. |
| filter | `pageup`/`pagedown` | Scroll 8. |

Filter matches key text or label, case-insensitive (`src/input/keybind_help.rs:231-251`). Groups: `global`, `navigation`, `workspaces / tabs`, `panes`, and `custom` when custom commands exist (`keybind_help.rs:75-229`).

#### Session navigator / Goto picker (`goto`)

Opens with the current pane selected, search not focused (`overlay_input.rs:193-208`).

| State | Key | Effect |
| --- | --- | --- |
| any | `esc` | In search: leave the search field. Else close. |
| any | `enter` | Go to the selected machine, workspace or pane and close. |
| search | text field keys | Edit the query (Left/Right move the text cursor). |
| search | `up`/`ctrl+p`, `down`/`ctrl+n` | Move the selection. |
| browse | `left`/`right` | Jump to the first pane of the previous/next workspace section, no wrap. |
| browse | `backspace` | Clear the state filter. |
| browse | `home` | Clear the selection, scroll to top. |
| browse | `end`, `G` | Select the last row. (`G` is checked with no modifiers after normalize; a kitty-protocol Shift+g carries Shift and may not match. Unclear in practice.) |
| browse | `/` | Focus search (typed character `/`). |
| browse | `down`/`j`, `up`/`k` | Move 1. |
| browse | `ctrl+d` / `ctrl+u` | Move 8 down / up. |
| browse | `b` `w` `i` `d` | Filter blocked / working / idle / done agents (clears the query). |
| browse | `a` | Show all (clears query and filter). |

#### Settings (`settings`)

Sections in order: theme, indicators, sound, toast, integrations (`state.rs:385-400`).

| Key | Effect |
| --- | --- |
| `esc` | Close and restore the previewed theme. Ignored while integrations install. |
| `tab`, `right`, `l` | Next section (wraps). |
| `shift+tab`, `left`, `h` | Previous section. |
| `up`/`k`, `down`/`j` | Move the choice. In the theme section, moving previews the theme. |
| `enter`, `space` | Apply the choice and write it to `config.toml`, then reload client and server config. Theme apply also closes the overlay. |
| other keys | Swallowed. |

What each section writes (`src/config/write.rs:19-46`): theme -> `[theme] name = "..."` and `auto_switch = false`; indicators -> `[ui] status_indicators`; sound -> `[ui.sound] enabled`; toast -> `[ui.toast] delivery` (choices off, herdr, terminal, system) and removes legacy `enabled`; integrations -> installs recommended agent integrations (server runtime feature).

#### Name dialogs (new workspace, rename workspace, new tab, rename tab, rename pane)

Source: `overlay_input.rs:898-995`.

| Key | Effect |
| --- | --- |
| `enter` | Save. Empty name: rename workspace/tab does nothing; new workspace/tab uses the suggested name. |
| `esc` | Cancel. |
| `ctrl+c` | Clear the field. |
| `super+backspace` | Clear the field. |
| text field keys | Edit. Prefilled "new" names are selected: the first typed text replaces them. |

#### Close confirmation

| Key | Effect |
| --- | --- |
| `enter` | Confirm close. |
| `esc` | Cancel and enter navigate mode with the focused workspace selected (`overlay_input.rs:885-896`). |

#### Worktree dialogs

- New worktree: text field keys edit the branch; the checkout path preview updates. `enter` creates (error `branch is required` when empty). `esc` cancels unless creating.
- Open worktree: search field starts unfocused. `/` focuses search. `up`/`down`, `ctrl+p`/`ctrl+n` move. `enter` opens. `esc` cancels unless opening. No `j`/`k`.
- Remove worktree: `enter` confirms (a second confirmation appears if Git refuses a safe remove); `esc` cancels unless removing.

#### Context menu (right-click on workspace, tab or pane)

`esc` close, `up`/`down` move (no `j`/`k`), `enter` activate (`overlay_input.rs:602-626`). Items (`context_menu.rs:3-84`):

- Workspace without git: Rename, Close.
- Git workspace with no worktree children: Rename, Close, New worktree, Open worktree...
- Linked worktree: Rename, Close, Delete worktree checkout...
- Worktree parent: Rename, Close (or Close group), New worktree, Open worktree..., Collapse/Expand.
- Tab: New tab, Rename, Close.
- Pane: Rename pane, Clear pane name (only with a manual name), Swap with focused pane (only for a non-focused pane), Split right, Split down, Zoom, Send right-clicks to pane / Use Herdr right-click menu, Close pane.

#### Global menu

`esc` close, `up`/`k`, `down`/`j` move, `enter` activate (`overlay_input.rs:572-596`). Items: settings, keybinds, reload config, then "update ready" or "what's new" when available, then detach (`global_menu.rs:22-54`).

#### Onboarding, product announcement, release notes

- Onboarding: `enter`, `right`, `l` continue (writes `onboarding = false`, opens settings on the integrations section). Other keys ignored (`overlay_input.rs:480-488`, `:177-191`).
- Product announcement and release notes: `enter`/`esc` dismiss; `up`/`k`, `down`/`j` scroll 1; `pageup`/`pagedown` scroll 8; `home`/`end`.

### 1.9 Text field editing keys

Source: `src/client/shell/text_editor.rs:163-238`. Used by name dialogs, worktree branch, help filter, navigator search, worktree search, copy-mode search.

| Keys | Effect |
| --- | --- |
| `left`, `ctrl+b` / `right`, `ctrl+f` | Move one grapheme. |
| `home`, `ctrl+a` / `end`, `ctrl+e` | Start / end. |
| `alt+b` / `alt+f` | Word left / right. |
| `backspace`, `ctrl+h` | Delete previous (or clear a prefilled selected value). |
| `delete`, `ctrl+d` | Delete next. |
| `ctrl+u` / `ctrl+k` | Cut to start / to end. |
| `ctrl+w`, `alt+backspace`, `ctrl+backspace` | Cut previous word. |
| `alt+d` | Cut next word. |
| `ctrl+y` | Insert the last cut text (field-local, not the clipboard). |
| printable (no modifier but Shift) | Insert. Host-reported text is authoritative (IME, AltGr). |

`enter` and `esc` are never handled by the field; the dialog handles them.

### 1.10 Other fixed keys

- Modal paste: in a text field (name dialogs, worktree create, focused searches, copy-mode search), `ctrl+v` (and `cmd+v` on macOS), with or without Shift, pastes clipboard text into the field when the host sent no text for the key (`input.rs:26-38`, `:549-608`).
- Retained mouse selection copy: `ctrl+c` or `super+c` when `ui.copy_on_select = false` (section 1.7 step 7).
- Popup: while a custom-command popup is open, every key goes to it (`input.rs:632-634`).

## 2. Config options

Config file: TOML. Top-level keys (`src/config/io.rs:7-20`, `src/config/model.rs:311-326`): `onboarding`, `theme`, `terminal`, `session`, `server`, `update`, `keys`, `ui`, `worktrees`, `advanced`, `experimental`, `remote`. Every struct uses `#[serde(default)]`, so every field is optional. Full commented template: `DEFAULT_CONFIG` in `src/main.rs:67-447` (printed by `herdr --default-config`).

Column "Reload" values:

- `live`: applied by `prefix+shift+r` / `herdr server reload-config` / settings writes. "client" or "server" says which process applies it (client = the TUI's local config; server = the config on the machine that runs the panes). Client side: `src/client/shell/config.rs:308-383`. Server side: `src/app/mod.rs:851-1054`.
- `restart`: needs a server restart (or client reattach where noted). A reload reports a diagnostic and keeps the old value.
- `launch`: read once when the process starts.
- `per use`: read from disk each time it is used.

Column "Owner": `UI` = presentation, belongs to rodeo. `RT` = old Herdr runtime/server concern (session, server, update, remote, integrations, pane creation, persistence); in rodeo this belongs to hc. `both` = UI-facing but resolved on the server in Herdr.

### 2.1 Top level

| Path | Type | Default | Allowed | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| `onboarding` | bool, optional | unset (treated as `true`) | `true`, `false` | Show first-run setup. Continuing writes `onboarding = false` to the top of the file (`src/config.rs:94-96`, `overlay_input.rs:177-191`). | launch | UI |

### 2.2 `[theme]`

Source: `src/config/theme.rs:4-151`, `src/app/mod.rs:237-318`.

| Path | Type | Default | Allowed | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| `theme.name` | string | `"catppuccin"` | `catppuccin`, `catppuccin-latte`, `terminal`, `tokyo-night`, `tokyo-night-day`, `dracula`, `nord`, `gruvbox`, `gruvbox-light`, `one-dark`, `one-light`, `solarized`, `solarized-light`, `kanagawa`, `kanagawa-lotus`, `rose-pine`, `rose-pine-dawn`, `vesper`. Aliases (case-insensitive, space/underscore = dash): `catppuccin-mocha`; `latte`, `light`; `tokyonight`; `tokyo-day`, `tokyonight-day`; `gruvbox-dark`; `onedark`; `onelight`; `solarized-dark`; `lotus`; `rosepine`; `rosepine-dawn`, `dawn` (`theme.rs:25-47`). | Built-in palette. `terminal` follows the host ANSI palette. Unknown: diagnostic, falls back to `catppuccin`. | live (client) | UI |
| `theme.auto_switch` | bool | `false` | | Follow host light/dark reports and switch between `dark_name` and `light_name`. Unknown appearance = dark. | live (client) | UI |
| `theme.dark_name` | string, optional | sibling of `name` (`sibling_theme_names`, `app/mod.rs:237-262`), else `name` | theme names | Theme for dark appearance. Unknown: falls back to `catppuccin`. | live (client) | UI |
| `theme.light_name` | string, optional | sibling of `name` (catppuccin->catppuccin-latte, tokyo-night->tokyo-night-day, gruvbox->gruvbox-light, one-dark->one-light, solarized->solarized-light, kanagawa->kanagawa-lotus, rose-pine->rose-pine-dawn), else `name` | theme names | Theme for light appearance. Unknown: falls back to `catppuccin-latte`. | live (client) | UI |
| `theme.custom.<token>` | color string, optional | unset | see color syntax | Override one palette token over the base theme. Tokens: `accent`, `panel_bg`, `sidebar_bg` (desktop sidebar background; unset = host background), `active_row_bg` (active Space / focused Agent row), `selection_bg` (navigate-mode cursor row in sidebar), `surface0`, `surface1`, `surface_dim`, `overlay0`, `overlay1`, `text`, `subtext0`, `mauve`, `green`, `yellow`, `red`, `blue`, `teal`, `peach` (19 tokens, `theme.rs:102-121`). | live (client) | UI |
| `theme.custom.light.<token>` | color string, optional | unset | same 19 tokens | Extra overrides when `auto_switch` picks light (`theme.rs:122-151`). | live (client) | UI |
| `theme.custom.dark.<token>` | color string, optional | unset | same 19 tokens | Extra overrides when `auto_switch` picks dark. | live (client) | UI |

Color syntax (`parse_color`, `theme.rs:155-219`): `#rrggbb`, `#rgb`, `rgb(r,g,b)`, names `black`, `red`, `green`, `yellow`, `blue`, `magenta`/`purple`, `cyan`, `white`, `gray`/`grey`, `darkgray`/`darkgrey`, `lightred`, `lightgreen`, `lightyellow`, `lightblue`, `lightmagenta`, `lightcyan`, and reset aliases `reset`, `default`, `none`, `transparent` (= terminal default color). Case-insensitive. An unknown color is not a diagnostic: it logs a warning and becomes cyan.

Palette order: built-in theme, `[theme.custom]`, legacy `ui.accent` (only when not `"cyan"` and `theme.custom.accent` unset), then `[theme.custom.light|dark]` (`app/mod.rs:264-318`).

### 2.3 `[terminal]`

Source: `src/config/model.rs:219-263`, `src/config.rs:98-103`.

| Path | Type | Default | Allowed | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| `terminal.default_shell` | string | `""` | executable name or path | Shell for new interactive panes. Empty: `$SHELL`, then `/bin/sh`; Windows: `pwsh.exe` if on PATH, else `powershell.exe`. | live (server), new panes only | RT |
| `terminal.shell_mode` | enum | `"auto"` | `auto`, `login`, `non_login` | `auto` = login shells on macOS, non-login elsewhere. | live (server), new panes | RT |
| `terminal.new_cwd` | string | `"follow"` | `""`/`follow`, `home`, `current`, any other string = fixed path (tilde allowed) | CWD policy for new panes, tabs, workspaces. `follow` inherits the source pane/workspace ($HOME without one); `current` = Herdr process dir. | live (server) | RT |
| `terminal.kitty_graphics` | bool, optional | unset (= `true`) | | Render Kitty graphics in compatible outer terminals. Takes precedence over `experimental.kitty_graphics`. Server controls parsing; client controls output. | restart (server) / reattach (client) | both |

### 2.4 `[session]` (runtime)

Source: `src/config/model.rs:265-282`.

| Path | Type | Default | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- |
| `session.resume_agents_on_restore` | bool | `true` | Resume supported agent panes into their native sessions when the server restores a session (`app/mod.rs:439`). | launch (server) | RT |
| `session.startup_per_agent_delay_ms` | u32 | `100` | Milliseconds between automatic agent restores; `0` = no spacing. | restart (reload reports `... require restarting Herdr; kept current setting`, `app/mod.rs:923-931`) | RT |

### 2.5 `[server]` (runtime)

Source: `src/config/model.rs:1039-1049`, `:1314-1323`, `src/config.rs:79-80`, `:129-144`.

| Path | Type | Default | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- |
| `server.headless_cols` | u16 | `120` | Virtual width when no client is attached. `0` is invalid: diagnostic, uses 120x40. | live (server); invalid keeps current | RT |
| `server.headless_rows` | u16 | `40` | Virtual height when no client is attached. | live (server) | RT |
| `server.allow_unelevated_clients` | bool | `false` | Windows only (`#[cfg(windows)]`). Let same-account non-elevated clients control an elevated server. On other platforms the key is unknown (ignored with diagnostic). | restart | RT |

### 2.6 `[update]` (runtime)

Source: `src/config/model.rs:14-59`.

| Path | Type | Default | Allowed | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| `update.channel` | enum | `"stable"`; `"preview"` on Windows preview builds | `stable`, `preview` | Channel for background checks and `herdr update`. | per use (`Config::load()` at use, `src/update.rs:109`) | RT |
| `update.version_check` | bool | `true` | | Background check for new Herdr versions. | live (server) | RT |
| `update.manifest_check` | bool | `true` | | Background check for agent-detection manifest updates. | live (server) | RT |

### 2.7 `[keys]`

See section 1.4 for every field and default, section 1.5 for `[[keys.command]]`, section 1.6 for `[keys.indexed]`. Types: every action field and `prefix` are "string or array of strings"; `remote_image_paste` and `indexed.*` are strings. Legacy alias: `keys.fullscreen` = `keys.zoom`. Reload: live (client and server). With `herdr --remote-keybindings server` (`HERDR_REMOTE_KEYBINDINGS=server`) the client uses the keybindings the server publishes instead of local ones (`src/client/handshake.rs:39-48`, `src/client/shell/config.rs:228-306`). Owner: UI, except custom command execution (RT).

### 2.8 `[ui]`

Source: `src/config/model.rs:937-1010` (fields), `:1198-1247` (defaults), `src/config.rs:76-78`. Client apply: `src/client/shell/config.rs:113-163`, `:338-370`. Server apply: `src/app/mod.rs:885-921`.

| Path | Type | Default | Allowed | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| `ui.sidebar_width` | u16 | `26` | clamped to min..max | Default expanded sidebar width. A width saved by mouse drag in client preferences wins (section 3.5). | live (client), unless a manual width is saved | UI |
| `ui.sidebar_min_width` | u16 | `18` | must be <= max | Min expanded width. | live (client) | UI |
| `ui.sidebar_max_width` | u16 | `36` | must be >= min | Max expanded width. If min > max: diagnostic; at startup layout uses 18..36; on reload the whole `[ui]` section is kept as before (`src/config.rs:146-155`, `app/mod.rs:885-892`). | live (client) | UI |
| `ui.sidebar_start_collapsed` | bool | `false` | | Start with the sidebar collapsed. A saved collapsed state in client preferences wins. | launch ("next launch") | UI |
| `ui.sidebar_collapsed_mode` | enum | `"compact"` | `compact` (4-column status rail), `hidden` (0 width) | Collapsed sidebar look (`client/shell/config.rs:403-408`). | live (client) | UI |
| `ui.mobile_width_threshold` | u16 | `64` | | At or below this terminal width, use the mobile single-column layout. | live (client) | UI |
| `ui.mouse_capture` | bool | `true` | | Capture mouse for Herdr's mouse UI. `false` lets the terminal handle clicks; panes that request mouse still get it. | live (client) | UI |
| `ui.copy_on_select` | bool | `true` | | Copy mouse selections at once. `false`: keep the selection until `ctrl+c`/`cmd+c`. | live (client) | UI |
| `ui.host_cursor` | enum | `"auto"` | `auto`, `native`, `drawn` | `auto` draws Herdr's cursor on native Windows and WSL, native cursor elsewhere. | live (server) | UI |
| `ui.right_click_passthrough_modifier` | string | `""` (off) | `""`, `off`, `none`, `disabled`, or `+`-joined `ctrl`/`control`, `alt`/`option`, `cmd`/`command`/`super`, `meta`, `hyper`. `shift` is rejected (deserialization error). | With the modifier held, right-click gestures go to the pane app instead of the pane menu (`model.rs:145-196`). Here `meta` is Meta, not Alt. | live (client) | UI |
| `ui.redraw_on_focus_gained` | bool | `true` | | Full redraw when the outer terminal regains focus. | live (client) | UI |
| `ui.mouse_scroll_lines` | non-zero integer, optional | unset (= `3`) | >= 1; `0` is a deserialization error | Lines per wheel notch. | live (client) | UI |
| `ui.confirm_close` | bool | `true` | | Confirm before closing a workspace, the last tab of a workspace, or the last pane of the last tab. | live (client + server) | UI |
| `ui.prompt_new_tab_name` | bool | `true` | | Ask for a tab name on `new_tab`. | live (client) | UI |
| `ui.prompt_new_workspace_name` | bool | `false` | | Ask for a workspace name on `new_workspace`. | live (client) | UI |
| `ui.pane_borders` | enum or bool | `"auto"` | `auto`, `always`, `off`; legacy `true` = auto, `false` = off. Other values: deserialization error. | Borders around split panes. `always` also frames a lone pane, only while `pane_outer_borders = true` (`model.rs:883-935`). | live (server) | UI |
| `ui.pane_outer_borders` | bool | `true` | | Borders on the outside edge of the pane area. | live (server) | UI |
| `ui.pane_scrollbars` | bool | `true` | | Interactive scrollbars beside panes. | live (server) | UI |
| `ui.pane_gaps` | bool | `true` | | Separate split panes instead of shared dividers. | live (server) | UI |
| `ui.show_agent_labels_on_pane_borders` | bool | `false` | | Agent label in split pane borders when no manual label. | live (server) | UI |
| `ui.hide_tab_bar_when_single_tab` | bool | `false` | | Hide the tab row when the workspace has one tab. | live (client) | UI |
| `ui.tab_bar_position` | enum | `"top"` | `top`, `bottom` | Desktop tab row placement. Mode bars replace the bottom tab row while active. | live (client) | UI |
| `ui.tab_bar_right` | array of tables | `[]` | see 2.8.1 | Ordered status entries at the right of the tab row. Max 16 (extras ignored with diagnostic). | live (server) | both |
| `ui.tab_bar_right_separator` | string | `" "` | | Text between visible right-side entries. | live (server) | UI |
| `ui.window_title` | string | `"{hostname}: {workspace}"` | tokens `{hostname}`, `{workspace}`, `{tab}`, `{pane}`, `{terminal_title}`; `{{`/`}}` literal braces; `""` disables | Title written to the outer terminal. Rendered on the server. Parse error: diagnostic, title left alone. Max 200 chars after sanitizing (`src/config/window_title.rs:1-127`). | live (server) | both |
| `ui.agent_panel_sort` | enum | `"spaces"` | `spaces` (alias `workspaces`), `priority` | Agent panel order. A sort chosen in the UI is saved in client preferences and wins. | live (client + server) | UI |
| `ui.agent_panel_scope` | legacy | - | `current`, `all` | Retired. Parsed and ignored (`model.rs:996-998`). | - | UI |
| `ui.status_indicators` | enum | `"dots"` | `dots`, `symbols` | Agent state indicator style. | live (client) | UI |
| `ui.accent` | color string | `"cyan"` | color syntax | Legacy accent. Used only when not `"cyan"` and `theme.custom.accent` is unset. | live (with theme) | UI |
| `ui.sidebar.agents.rows` | array of arrays of tokens | `[["state_icon","machine","workspace","tab"],["agent"]]` | see 2.8.2 | Expanded Agent panel row layout. | live (client + server) | UI |
| `ui.sidebar.agents.rows_by_agent` | table: agent id -> rows | `{}` | keys: canonical agent ids `pi`, `claude`, `codex`, `gemini`, `cursor`, `devin`, `agy`, `cline`, `omp`, `mastracode`, `opencode`, `copilot`, `kimi`, `kiro`, `droid`, `amp`, `grok`, `hermes`, `kilo`, `qodercli`, `qwen`, `letta`, `maki`, `muse` (`src/detect/mod.rs:124-151`, `:193-196`). Unknown id: deserialization error. | Full replacement rows for one agent. | live | UI |
| `ui.sidebar.agents.row_gap` | u16 | `0` | | Blank rows between agent entries. | live | UI |
| `ui.sidebar.spaces.rows` | array of arrays of tokens | `[["state_icon","workspace"],["branch","git_status"]]` | see 2.8.2 | Expanded Space panel row layout. | live | UI |
| `ui.sidebar.spaces.row_gap` | u16 | `0` | | Blank rows between top-level Spaces / worktree groups. | live | UI |
| `ui.toast.delivery` | enum | `"off"` | `off`, `herdr` (in-app), `terminal` (outer terminal notification), `system` (OS service) | Background agent notifications (`model.rs:61-69`). | live (client + server) | both |
| `ui.toast.enabled` | legacy bool | - | | Used only when `delivery` is unset: `true` -> `herdr`, `false` -> `off` (`model.rs:1277-1312`). | live | UI |
| `ui.toast.delay_seconds` | u64 | `1` | `0`..`3600`; above is a deserialization error | Wait before a finished / needs-input notification; notify only if the state is the same. | live | both |
| `ui.toast.herdr.position` | enum | `"bottom-right"` | `top-left`, `top-right`, `bottom-left`, `bottom-right` | In-app toast position. | live (client) | UI |
| `ui.toast.clipboard.enabled` | bool | `true` | | "Copied" popup after a mouse copy. | live (client) | UI |
| `ui.toast.clipboard.position` | enum | `"bottom-center"` | `top-left`, `top-center`, `top-right`, `bottom-left`, `bottom-center`, `bottom-right` | Copied popup position. | live (client) | UI |
| `ui.sound.enabled` | bool | `true` | | Sounds when background agents change state. Env `HERDR_DISABLE_SOUND` disables playback. | live (client + server) | both |
| `ui.sound.path` | path, optional | unset | mp3 only | One sound for all notifications. Relative to the config file dir (`src/config/io.rs:184-193`). Non-mp3, missing, or not-a-file: diagnostic, default sound. | live | UI |
| `ui.sound.done_path` | path, optional | unset | mp3 | Sound for "done". Falls back to `path`. | live | UI |
| `ui.sound.request_path` | path, optional | unset | mp3 | Sound for "needs input". Falls back to `path`. | live | UI |
| `ui.sound.agents.<agent>` | enum | `"default"` (but `droid` = `"off"`) | `default`, `on`, `off` | Per-agent sound override. Keys: `pi`, `claude`, `codex`, `gemini`, `cursor`, `devin`, `agy`, `cline`, `open_code`, `github_copilot`, `kimi`, `kiro`, `droid`, `amp`, `grok`, `hermes`, `kilo`, `qodercli`, `qwen`, `letta`, `maki`, `muse` (22 keys, `src/config/sound.rs:25-59`, `:166-193`). `omp` and `mastracode` have no key (always default). Note the key names `open_code` and `github_copilot` differ from the canonical ids `opencode` and `copilot`. | live | UI |

#### 2.8.1 `ui.tab_bar_right` entries

Source: `src/config/tab_bar.rs:1-113`. Tagged by `type`; unknown fields are rejected (`deny_unknown_fields`).

| `type` | Fields | Notes |
| --- | --- | --- |
| `zoom` | none | `ZOOM` pill while the active tab is zoomed. |
| `hostname` | none | Server host name. |
| `datetime` | `format` (string, default `"%H:%M"`) | strftime; empty or unsupported (`%z`, `%s`) hides the entry with diagnostic. Server-local time. |
| `text` | `text` (string, required) | Literal. |
| `command` | `command` (string, required), `interval_seconds` (u64, default `5`, 1..31536000), `timeout_seconds` (u64, default `2`, 1..3600) | Runs on the server (`/bin/sh -lc`), last line of successful output, control sequences stripped. Bad values hide the entry with diagnostic. |

#### 2.8.2 Sidebar tokens and rules

Source: `src/config/sidebar.rs:11-483`, `src/config/sidebar/rules.rs:5-194`.

- Limits: max 16 rows, max 16 tokens per row, max 16 rules per token. Over limit: deserialization error.
- Agent tokens: `state_icon`, `state_text`, `machine`, `workspace`, `tab`, `pane`, `agent`, `terminal_title`, `terminal_title_stripped`, `$name` (custom pane metadata).
- Space tokens: `state_icon`, `state_text`, `workspace`, `branch`, `git_status`, `$name` (custom workspace metadata).
- Custom name: `$` plus 1-32 chars of ASCII alphanumerics, `_`, `-`. Any other bare name: `unknown sidebar token` error.
- A token is a string, or an inline table (unknown fields rejected):

| Field | Type | Meaning |
| --- | --- | --- |
| `token` | string | Token name (required). |
| `fg` | string | Strict `#RGB` or `#RRGGBB` only (not the general color syntax). |
| `bold` | bool | Explicit `false` removes the default bold. |
| `dim` | bool | Same for dim. |
| `rules` | array of rule tables | Not allowed on `state_icon` and `git_status`. |

- Rule table (unknown fields rejected); exactly one condition:

| Field | Type | Meaning |
| --- | --- | --- |
| `equals` | string | Exact match. |
| `contains` | string | Substring. |
| `starts_with` | string | Prefix. |
| `gt` | float (finite) | Value parses as a finite number and is greater. |
| `lt` | float (finite) | Less. |
| `ignore_case` | bool | ASCII case-insensitive; text conditions only (error on `gt`/`lt`). |
| `fg`, `bold`, `dim` | as above | Override when matched; unset fields inherit. |
| `hide` | bool | `true` hides the token and its separator. |

First matching rule wins.

### 2.9 `[worktrees]` (runtime)

| Path | Type | Default | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- |
| `worktrees.directory` | string | `"~/.herdr/worktrees"` | Root for `<repo>/<branch-slug>` checkouts. Tilde and relative paths become absolute when applied (`model.rs:868-873`, `:1190-1196`, `app/mod.rs:1007-1010`). | live (server) | RT |

### 2.10 `[advanced]`

| Path | Type | Default | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- |
| `advanced.scrollback_limit_bytes` | usize | `10000000` | Max scrollback bytes per pane terminal. Legacy alias `scrollback_lines` (`model.rs:1051-1057`). | live (server), new panes | RT |

### 2.11 `[remote]` (runtime)

| Path | Type | Default | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- |
| `remote.manage_ssh_config` | bool | `true` | `herdr --remote` uses a generated SSH config with keepalive fallbacks and a private control socket. `false` runs plain ssh (`model.rs:1059-1073`, `src/remote/attach.rs:55-58`). | per use (each attach) | RT |

### 2.12 `[experimental]`

Source: `src/config/model.rs:1012-1037`, `:1075-1118`.

| Path | Type | Default | Allowed | What it does | Reload | Owner |
| --- | --- | --- | --- | --- | --- | --- |
| `experimental.allow_nested` | bool | `false` | | Allow launching herdr inside a herdr pane (`HERDR_ENV=1` is the nesting marker, `src/main.rs:452-458`). | launch | RT |
| `experimental.kitty_graphics` | bool, optional | unset | | Deprecated alias of `terminal.kitty_graphics` (lower precedence). | restart | both |
| `experimental.pane_history` | bool | `false` | | Save pane screen history to `session-history.json`. Turning it off on reload deletes the history. | live (server) | RT |
| `experimental.reveal_hidden_cursor_for_cjk_ime` | bool | `false` | | Expose the focused pane cursor anchor even when the pane hid it, for macOS IME candidate windows. | live (server) | UI |
| `experimental.cjk_ime_agents` | array of strings | `[]` | `pi`, `claude`, `codex`, `gemini`, `cursor`, `devin`, `cline`, `opencode`, `copilot`, `kimi`, `kiro`, `droid`, `amp`, `grok`, `hermes`, `kilo`, `qodercli`, `qoder`, `qwen`, `qwen-code`, `letta`, `letta-code`, `maki` (case-insensitive) | Restrict the reveal to these agents. Empty = all panes. Unknown names ignored; no valid name = no reveal. | live (server) | UI |
| `experimental.cjk_ime_cursor_shape` | enum | `"steady_block"` | `block`, `steady_block`, `underline`, `steady_underline`, `bar`, `steady_bar` (DECSCUSR 1-6) | Cursor shape for the IME anchor. | live (server) | UI |
| `experimental.switch_ascii_input_source_in_prefix` | bool | `false` | | macOS/Windows (Korean IME only): switch to an ASCII input source while prefix mode and prefix-launched modes are active; restore after. | live (client) | UI |

## 3. Loading, reload, errors, paths, env vars

### 3.1 Config path

Source: `src/config/io.rs:22-94`, `:195-200`, `src/config.rs:56`.

1. `HERDR_CONFIG_PATH` (full file path) wins.
2. Else `<config_dir>/config.toml`, where `config_dir` is:
   - `$XDG_CONFIG_HOME/herdr` if `XDG_CONFIG_HOME` is set (all platforms).
   - Unix: `$HOME/.config/herdr`, else `<tmp>/herdr`.
   - Windows: `%APPDATA%\herdr`, else `%USERPROFILE%\AppData\Roaming\herdr`, else `$HOME/.config/herdr`, else `<tmp>/herdr`.
3. Debug builds use `herdr-dev` instead of `herdr` (`io.rs:22-28`).

State dir (client preferences, history): `$XDG_STATE_HOME/herdr`, else `$HOME/.local/state/herdr` (Windows: `%LOCALAPPDATA%\herdr`, ...) (`io.rs:37-94`). Relative sound paths resolve against the config file's directory (`io.rs:184-193`). A UTF-8 BOM at the start is stripped; stray line-start BOMs that break TOML parsing are removed (`io.rs:96-128`). A missing file means all defaults with no diagnostic.

### 3.2 Startup load (`Config::load`)

Source: `src/config/io.rs:131-181`.

- Read error (not "not found"): all defaults, diagnostic `config read error: ...; using defaults`.
- The whole file is deserialized at once. Any TOML syntax error or any type/value error in any section (for example `pane_borders = "framed"`, `mouse_scroll_lines = 0`, `delay_seconds = 5000`, a bad sidebar token, an unknown `rows_by_agent` id, `right_click_passthrough_modifier = "shift"`) makes the whole config fall back to defaults, with `config parse error: ...; using defaults`. This is all-or-nothing, unlike reload.
- On success, soft diagnostics are collected (`src/config.rs:114-127`): unknown top-level sections (`unknown config section [x]; ignoring section`, with a `did you mean [ui.toast]?` hint for `[toast]`), unknown keys anywhere (`unknown config key a.b.c; ignoring key`, via `serde_ignored`), prefix and keybinding diagnostics, `remote_image_paste`, unknown theme names, sound file problems, `tab_bar_right` problems, `window_title` errors, inverted sidebar bounds, zero headless size.
- Soft diagnostics never block startup. Each has its own fallback (listed in the tables above).

### 3.3 Startup warning display

- The client builds a one-line summary (`src/config/io.rs:202-243`): `config.toml invalid; using defaults; herdr config check`, `config.toml unreadable; using defaults; ...`, `config.toml invalid; keeping current config; ...` (reload), `config.toml has unknown keys; herdr config check`, or `config.toml; herdr config check` for other soft diagnostics. The file name is taken from the resolved path.
- It renders as a bold bar on a yellow background at the top-right of the frame (or of the pane area in mobile layout), one row per line (`src/ui/status.rs:90-125`, `src/client/shell/composition.rs:455-480`).
- Client and server diagnostics merge: same text gives `client + endpoint: ...`; different text gives two lines `client: ...` and `endpoint: ...` (`src/client/shell/config.rs:3-16`).
- With server keybindings (`HERDR_REMOTE_KEYBINDINGS=server`), the client drops keybinding diagnostics from its own summary (`src/config.rs:58-75`).
- `herdr config check` prints `config: ok` or `config: issues found` plus every diagnostic; exit code 1 on issues (`src/cli.rs:289-313`).
- `herdr config reset-keys` backs up the file and removes `[keys]`, `[keys.*]`, `[[keys.command]]` and top-level `keys` assignments (`src/cli.rs:315-...`, `src/config/io.rs:638-688`).

### 3.4 Live reload

Source: `src/config/io.rs:245-386`, `src/app/mod.rs:811-1054`, `src/client/shell/config.rs:71-110`, `:308-383`.

- Triggers: `reload_config` key, global menu "reload config", `herdr server reload-config`, and every settings write. The key and menu reload both the client's local config and the active server's config.
- TOML syntax error or non-table top level: nothing changes; status `Failed`; diagnostic `config parse error: ...; keeping current config`.
- Otherwise each top-level section is parsed on its own. An invalid section keeps its current settings (`invalid <label>: ...; keeping current <section> settings`); other sections still apply. Status is `Applied` with no diagnostics, else `Partial`.
- `[keys]`: a prefix rejection keeps the whole current keymap; other keybinding problems only disable the bad bindings.
- `[ui]`: inverted sidebar bounds keep the whole previous `[ui]`.
- `[server]`: zero headless size keeps current `[server]`.
- Restart-only (reported, not applied): `session.startup_per_agent_delay_ms`, `terminal.kitty_graphics` / `experimental.kitty_graphics`. Not reapplied at all: `ui.sidebar_start_collapsed`, `session.resume_agents_on_restore`, `experimental.allow_nested`, `server.allow_unelevated_clients`.
- Success shows a toast "reloaded config" with "using config.toml" or "with warnings".
- Theme, sidebar layout, copy behavior and other presentation settings come from the client's local config, also when viewing an SSH machine. Pane defaults, worktrees, integrations, custom commands come from the server (docs `configuration.mdx`, "Reload config").

### 3.5 Client preferences that override config

Source: `src/client/shell/preferences.rs:10-56`, `src/client/shell/state.rs:995-1021`, `src/client/shell/config.rs:29-69`, `:86-91`.

The client saves UI changes made with the mouse or keys to `<state_dir>/client-shell/local-<hash>.json` (or `remote-<sha256>.json` per SSH target/session). Fields: `sidebar_width` (drag), `sidebar_section_split` (Agent/Space split, 0.1-0.9, default 0.5, no config key), `sidebar_collapsed` (`toggle_sidebar`), `agent_panel_sort`, `collapsed_groups`, `remote_collapsed_groups`. A saved value wins over `ui.sidebar_width`, `ui.sidebar_start_collapsed` and `ui.agent_panel_sort`, also across reloads.

### 3.6 Environment variables

| Variable | Effect | Source |
| --- | --- | --- |
| `HERDR_CONFIG_PATH` | Config file path override. | `src/config.rs:56`, `io.rs:195-200` |
| `XDG_CONFIG_HOME` | Config dir base. | `io.rs:30-35` |
| `XDG_STATE_HOME` | State dir base. | `io.rs:37-42` |
| `HOME`, `APPDATA`, `USERPROFILE`, `LOCALAPPDATA` | Platform dir fallbacks. | `io.rs:44-94` |
| `HERDR_REMOTE_KEYBINDINGS` | `server`: use server-published keybindings; other value: remote client uses local keys without custom commands; unset: local. Set by `herdr --remote`. | `src/client/handshake.rs:39-48`, `src/remote/args.rs:3` |
| `HERDR_REMOTE_PREFERENCES_IDENTITY` | JSON `[target, session]` that selects the remote preferences file. Set by `herdr --remote`. | `src/remote/args.rs:1`, `client/shell/config.rs:197-220` |
| `HERDR_ENV` | `1` inside a Herdr pane; blocks nested launch unless `experimental.allow_nested`. | `src/main.rs:3-4`, `:452-458` |
| `HERDR_DISABLE_SOUND` | Disable sound playback even with `ui.sound.enabled = true`. | `src/sound.rs:18` |
| `SHELL` | Default shell when `terminal.default_shell` is empty. | docs |
| `EDITOR` | Used by `edit_scrollback`. | `model.rs:410` |
| `HERDR_SESSION`, `HERDR_SOCKET_PATH`, `HERDR_LOG`, `HERDR_PROCESS_DETECTION` | Runtime, not config (docs `configuration.mdx`, "Environment variables"). | docs |

## 4. Open points (not verified in code)

- `update.channel` reload: not applied by `apply_live_config`; every user reads it with a fresh `Config::load()`, so it is effectively always current. Not tested.
- Whether `advanced.scrollback_limit_bytes` changes existing panes on reload: the value is stored in server state and passed at pane creation (`src/app/creation.rs:137`). It looks like new panes only.
- Navigator `G` with kitty keyboard protocol (Shift flag set): probably does not match, see 1.8.
- `ui.sound.path` playback side: the server decides whether to notify (`src/server/headless/notifications.rs:126`); the client plays the file (`src/sound.rs:46`). Which machine's path is used for remote sessions was not traced.
- The settings "integrations" install flow and the worktree remove force-confirmation flow were not traced key by key.

## 5. Surprises

- Startup is all-or-nothing: one bad value anywhere drops the whole config to defaults, while live reload is per section. The docs say "falls back to a safe default", which is only true for soft diagnostics.
- `swap_pane_*` have default bindings but are missing from the keybind help overlay.
- In keybinding strings `meta` means Alt; in `ui.right_click_passthrough_modifier` `meta` means Meta.
- Invalid colors in `[theme.custom]` and `ui.accent` are silent (log only) and become cyan; sidebar token `fg` instead is strict `#RGB`/`#RRGGBB` and an error.
- Resize mode ignores modifiers and the `resize_pane_*` fields; it is fixed to `hjkl` and arrows.
- In navigate mode the bare right-hand side of any `prefix+X` binding runs that action (for example `c` = new tab).
- Esc in the close confirmation dialog drops you into navigate mode, not back to terminal mode.
- `keys.extra_prefixes` is an undocumented accepted key.
- Sound agent keys `open_code` and `github_copilot` do not match the canonical agent ids (`opencode`, `copilot`) used by `rows_by_agent` and `cjk_ime_agents`. Herdr has three agent-name lists that differ.
- `clear_pane` is the only action with no doc comment in `KeysConfig` (`model.rs:412`).
- Client preference files silently override `ui.sidebar_width`, `ui.sidebar_start_collapsed` and `ui.agent_panel_sort` after the user once changes them in the UI.
- The user's own Herdr TUI config (inline as `SAMPLE` in `crates/config/tests/load.rs`) uses legacy `[ui.toast] enabled = true` (= `delivery = "herdr"`) and direct `alt+[`/`alt+]` for tabs, which replaces the default `prefix+p`/`prefix+n`.
