# Herdr TUI v0.9.5: UI inventory

This file lists every user-visible element and interaction of Herdr TUI v0.9.5, so drover can show the same elements and react to the mouse the same way. Source: `~/oss/thuic-ref/herdr` at tag `v0.9.5`. All paths are relative to that repo. Citations are `file:line`.

## 0. Where the UI lives

In v0.9.5 the TUI is a client. The client draws the chrome (sidebar, tab bar, mode bar, overlays, toasts) in `src/client/shell/**`. The server draws the pane surface (pane frames, borders, titles, pane content) with `src/ui/tab_surface.rs` + `src/ui/panes.rs` and sends it as a retained surface. The client blits it into the pane area (`src/client/shell/composition.rs:346`).

- Frame entry: `ClientShellState::compose` (`src/client/shell/composition.rs:150-725`).
- Chrome entry: `render_shell` (`src/client/shell/render.rs:253-343`).
- Overlay enum: `ClientShellOverlay` (`src/client/shell/state.rs:583-598`), with 13 kinds: Onboarding, ProductAnnouncement, ReleaseNotes, Rename, ConfirmClose, Help, Navigator, WorktreeCreate, WorktreeOpen, WorktreeRemove, ContextMenu, GlobalMenu, Settings. Only one overlay shows at a time.
- Client modes: `ClientShellMode { Terminal, Prefix, Navigate, Resize, Copy }` (`src/client/shell/state.rs:270-277`). The server has only `Mode { Navigate, Terminal }` (`src/app/state.rs:685-688`).
- App-side (server) UI state that reaches the screen: `PopupPaneState` (popup pane, `src/app/state.rs:18`, `src/app/popup.rs`), `ReleaseNotesState` (`src/app/state.rs:758`), `ProductAnnouncementState` (`:766`), `ToastNotification` (`:721`), `CopyFeedback` (`:753`), `TabBarStatusSegment` (`:784`), agent view (`src/app/agent_view.rs`), window title (`src/app/window_title.rs`), terminal titles (`src/app/terminal_titles.rs`).

## 1. Screen layout

### 1.1 Regions

`ClientShellConfig::layout` (`src/client/shell/config.rs:385-446`) returns `ClientShellLayout { sidebar, tab_bar, mobile_header, pane_surface }`.

Mobile layout, when `cols <= ui.mobile_width_threshold` (default 64, `src/config.rs:78`):
- `mobile_header` = `Rect(0, 0, cols, min(rows, 2))`.
- `pane_surface` = the rest below it.
- No sidebar and no tab bar (`config.rs:396-403`).

Desktop layout:
- Sidebar width:
  - Expanded: `sidebar_width.clamp(min, max)`. Defaults are `sidebar_width = 26`, `sidebar_min_width = 18`, `sidebar_max_width = 36`. If the bounds are not valid, the fallback is `(18, 36)` (`config.rs:408-416`, `src/config/model.rs:1201-1203`).
  - Collapsed: `compact` = 4 columns, `hidden` = 0 (`config.rs:403-407`).
  - The width is capped at `cols - 1`.
- Sidebar rect: `Rect(0, 0, sidebar_width, rows)`, full height on the left (`config.rs:442`).
- Main area: `Rect(sidebar_width, 0, cols - sidebar_width, rows)` (`config.rs:419`).
- Tab bar: 1 row. It shows if `rows > 1`, unless `hide_tab_bar_when_single_tab` is set and the focused workspace has exactly 1 tab (`config.rs:420-421`).
  - `tab_bar_position = "top"` (default): tab bar at row 0, pane surface below it.
  - `"bottom"`: pane surface on top, tab bar on the last row (`config.rs:422-439`).
- Pane surface: the rest of the main area. All panes, borders, scrollbars and the popup pane are inside it.
- Footer or status bar: none. The only bottom bar is the mode bar, and it shows only in non-Terminal modes (section 1.3).

Related config (`src/config/model.rs:937-1008`, defaults `:1196-1235`):

| Key | Default |
|---|---|
| `sidebar_start_collapsed` | false |
| `sidebar_collapsed_mode` | compact / hidden, default compact |
| `mouse_capture` | true |
| `copy_on_select` | true |
| `host_cursor` | auto |
| `right_click_passthrough_modifier` | — |
| `redraw_on_focus_gained` | true |
| `mouse_scroll_lines` | 3 |
| `confirm_close` | true |
| `prompt_new_tab_name` | true |
| `prompt_new_workspace_name` | false |
| `pane_borders` | auto / always / off, default auto |
| `pane_outer_borders` | true |
| `pane_scrollbars` | true |
| `pane_gaps` | true |
| `show_agent_labels_on_pane_borders` | false |
| `hide_tab_bar_when_single_tab` | false |
| `tab_bar_position` | top |
| `tab_bar_right` | [] |
| `tab_bar_right_separator` | `" "` |
| `window_title` | `"{hostname}: {workspace}"` |
| `agent_panel_sort` | spaces / priority, default spaces |
| `status_indicators` | dots / symbols, default dots |
| `sidebar` (row tokens) | — |
| `accent` | "cyan" |
| `toast` | — |
| `sound` | — |

### 1.2 Compose order (z-order, bottom to top)

From `composition.rs:150-725`:
1. `render_shell`: draws, in order, the mobile header, the sidebar (one of 4 variants, section 2), and the tab bar (section 3). With `mouse_capture = false` it clears the chrome hit rects (`render.rs:253-343`).
2. Mode bar (section 1.3). It is skipped when an overlay is open, and in the mobile Navigate panel (`composition.rs:310-334`). If the mode bar lands on the bottom tab bar, the tab hits are cleared (`:335-340`).
3. The pane surface is blitted into `layout.pane_surface` (`:346`). The mode bar cells are then restored on top (`:347`). The mode bar sits over the last row of the pane surface, or over the bottom tab bar.
4. Copy-mode search highlights (non-current matches), the selection highlight, then the current search match (`:349-414`).
5. Link hover underline (`:415`).
6. Copy-mode cursor cell: fg panel contrast, bg `accent`, BOLD (`:416-452`).
7. Config diagnostic banner (`:469-482`), endpoint lifecycle banner (`:483-493`), then either an endpoint notice or an agent notification toast (`:494-531`).
8. Copy feedback toast ("copied" etc.), moved down if it overlaps the toast (`:535-560`, `src/ui.rs:133-146`).
9. Popup pane: `Clear`, then a `Block` with all borders in fg `accent`, the popup title and bg `panel_bg`. The popup frame is blitted inside (`:561-596`).
10. Mobile switcher, full screen, in mobile Navigate mode (`:597-640`).
11. Mode bar restored again (`:641-644`).
12. The overlay. ContextMenu and GlobalMenu use their own renderers; all other kinds use `render_client_overlay` (`:645-703`).
13. If the active endpoint is not Online: no cursor, and the pane, split and popup hits are cleared (`:717-722`).

Unavailable state (no snapshot or pane surface yet), from `compose_unavailable` (`composition.rs:22-148`):
- Background: `panel_bg`.
- The sidebar, or the machines sidebar.
- In overlay0 at the top of the pane area: `"{endpoint label}: {status label}. Select a connected machine."`, or the endpoint error text.
- The mode bar, and the endpoint notice.

### 1.3 Mode bar (prefix / navigate / resize / copy / error indicator)

`render_mode_bar` is at `src/client/shell/render.rs:31-229`.
- **When shown:** the mode is not Terminal, or an endpoint error is set (`:41-43`).
- **Placement:** one row.
  - Default: the last row of the pane surface.
  - With `tab_bar_position = "bottom"` (and not mobile, and the tab bar is visible): it replaces the tab bar row (`composition.rs:310-317`).
- **Base style:** fg `overlay0` on `panel_bg`.
- **Key style:** fg `accent`, BOLD, on `panel_bg`.
- **Mode badge style:** fg panel contrast (`panel_bg`, or `surface_dim` when `panel_bg` is Reset), BOLD. The bg is `mauve` for RESIZE and `accent` for all other modes (`:47-72`).

Segments in order. `{prefix}` is the primary prefix label, and `{help}` / `{nav}` are the right-hand keys of the help and workspace_picker bindings. An unbound key shows as `unset` (`:73-78`).

| Mode | Segments |
|---|---|
| Error | ` ERROR ` + ` {error text}` (`:81-85`) |
| Prefix | ` PREFIX `, ` `, `esc` ` cancel  `, `{prefix}` ` send prefix  `, `{nav}` ` workspace nav  `, `{help}` ` keybinds` (`:88-100`) |
| Navigate | ` NAVIGATE `, ` esc back  `, `↑/↓` ` workspace  `, `tab` ` pane  `, `{help}` ` keybinds` (`:101-112`). If an update is available, ` update ready` is drawn right-aligned in a 13-column area, accent + BOLD (`:214-228`). |
| Resize | ` RESIZE `, `  `, `h/l` ` width  `, `j/k` ` height  `, `esc` ` done` (`:113-124`) |
| Copy, normal | ` COPY `, ` `, `h/j/k/l w/b/e { }` ` move  `, `/ ?` ` search  `, `n/N` ` repeat{ i/N or 0/0}  `, `v/space` ` select ` or ` selecting `, `y/enter` ` copy  `, then either `q/esc` ` exit` (no search and no selection) or `esc` ` clear  q exit` (`:158-196`) |
| Copy, search prompt | ` COPY ` badge. Then the marker `/` (forward) or `?` (backward) in key style at x+7. Then an editable query field from x+8: fg `text` on `panel_bg`, with a cursor cell in inverse (`panel_bg` on `text`). If the bar width is 50 or more, `  enter search  esc cancel` is right-aligned (`:127-157`). |

Segments are cut at the bar's right edge (`:200-213`).

### 1.4 Data needed for layout

| Data | Used for |
|---|---|
| Terminal size | layout |
| Focused workspace's tab count | hiding the tab bar |
| Sidebar collapsed flag, sidebar width, section split | sidebar geometry. The client saves these as preferences (`sidebar_width`, `sidebar_collapsed`, `sidebar_section_split`, `agent_panel_sort`, `collapsed_groups`) in `src/client/shell/preferences.rs`. |
| Keybind labels | mode bar |
| `update_available` | mode bar, menu attention |
| Endpoint list and status | sidebar, banners |
| Copy-mode state | mode bar |

Key label details:
- `{prefix}` is printed by `format_key_combo`, e.g. `ctrl+b` (`src/config/keybinds.rs:1173-1215`).
- `prefix_rhs_label` strips `prefix+` and joins multiple bindings with ` / ` (`src/config/keybinds.rs:283-301`).
- The default `{nav}` is `w` and the default `{help}` is `?` (`src/main.rs:147,152`).

## 2. Sidebar

Notes that apply to the whole sidebar:
- Palette roles are fields of `Palette` (`src/app/state.rs:35-74`). In the default catppuccin theme, `sidebar_bg` is `Reset`, and `active_row_bg` and `surface_dim` share one color.
- The sidebar has no spinners. Status icons are static (`configuration.mdx:360`). A spinner glyph can show only inside the text of the `terminal_title` token (`agents.mdx:150`).
- There is no git "dirty" indicator. The only git data is `branch` and `git_ahead_behind` (`src/protocol/wire.rs:1021-1036`).
- Expanded workspace rows show no number. Only the collapsed views show numbers.

### 2.1 Which sidebar variant is drawn

`render_shell` picks one of four variants (`src/client/shell/render.rs:270-311`):

| Endpoints | State | Renderer |
|---|---|---|
| more than 1 | collapsed | `endpoint_sidebar::render_collapsed` |
| more than 1 | expanded | `endpoint_sidebar::render_expanded` |
| 1 | collapsed | `render_collapsed_sidebar` (`src/client/shell/sidebar.rs:47`) |
| 1 | expanded | `render_sidebar` (`sidebar.rs:199`) |

- Nothing is drawn when the sidebar width is 0: hidden mode, or the mobile layout.
- `mouse_capture = false` clears these hits: sidebar_divider, sidebar_section_divider, workspace_scrollbar, agent_scrollbar, agent_sort_toggle, new_workspace, machines, workspaces, agents, endpoint_agents (`render.rs:324-339`).
- With `mouse_capture = false` the footer (`new` / `menu`) is not drawn at all (`sidebar.rs:379`, `endpoint_sidebar.rs:520`).
- Unavailable state: the sidebar uses `render_sidebar` only when there is one endpoint, it is Online, and the sidebar is expanded. In every other case it uses `endpoint_sidebar::render_expanded`, even when collapsed. If the width is 0, it draws into the fallback rect `Rect(0, 1, cols, rows-2)` (`composition.rs:33-37, 50-97`).

### 2.2 Geometry

- **Width and position:** see 1.1. The content area is the sidebar rect minus its last column. The last column holds the separator.
- **Saved preferences:** `sidebar_width` (set by dragging), `sidebar_collapsed`, `sidebar_section_split` (clamped 0.1–0.9, default 0.5), `agent_panel_sort`, `collapsed_groups` (`src/client/shell/state.rs:1002-1031`).
- **Section split** (`src/ui/sidebar.rs:30-66`), with `h` = content height:
  - If `h < 6`: the spaces section gets `ceil(h/2)` rows.
  - Otherwise: the spaces section gets `round(h * split)`, clamped to `[3, h-3]`.
  - The agents section takes the rest, directly below.
- **Section divider hit:** the first row of the agents section, full content width. It exists only when `h >= 6` (`ui/sidebar.rs:68-76`).
- **Spaces body:** starts at `y+2` (`WORKSPACE_HEADER_ROWS = 2`, `state.rs:5`). Its height is the section height minus 3: 2 header rows and 1 footer row (`sidebar.rs:230-237`).

### 2.3 Background and separator

`render_sidebar_background` (`render.rs:16-29`):
- Fills the sidebar rect with bg `sidebar_bg`.
- Draws `│` in fg `surface_dim` on every row of the last column.

All four variants call it first.

### 2.4 Expanded sidebar, one machine (`render_sidebar`, `sidebar.rs:199-456`)

```
 spaces                 │   row 0: header (overlay0, BOLD)
                        │   row 1: blank
 ● herdr                │   line 1: state_icon + workspace (x+1)
   main ↑2 ↓1           │   line 2 (x+3): branch + git_status
 ○ api                 ▾│   ▾/▸ = group toggle, last column of first line
    ├─ feat-x           │   linked worktree children
    └─ fix-y            │
 new                menu│   footer row (mouse_capture only)
────────────────────────│   agents row 0: divider (draggable)
 agents          grouped│   agents row 1: header + sort toggle
                        │   agents row 2: blank
 ● herdr · build        │   agent row line 1 (x+1)
   claude               │   agent row line 2 (x+3)
                       «│   collapse toggle at (right-2, bottom)
```

#### Spaces header

- `" spaces"` in fg overlay0, BOLD, at `workspace_area.y` (`sidebar.rs:218-227`).
- There is no empty-state text. With zero workspaces the body is blank.

#### Workspace entries and worktree groups (`workspace_entries`, `sidebar.rs:458-554`)

- A worktree key forms a group only when its members include at least one linked worktree and at least one non-linked workspace (`:468-484`).
- Ungrouped workspaces appear in snapshot order, not indented.
- A group appears at the position of its first member:
  1. All non-linked parents, not indented.
  2. The linked children, indented, in snapshot order. The last child is flagged `last_child` (`:500-551`).
- Collapsed group (its key is in `collapsed_groups`): only the parents show, plus the focused linked child if there is one, indented as `last_child` (`:518-533`).
- `WorkspaceEntry { index, indented, last_child }` (`state.rs:989-994`).

#### Rows of one workspace (`workspace_rows`, `sidebar.rs:663-690`)

- **Label:** an indented child without `custom_label` shows its `branch`, with a `worktree/` prefix stripped. If there is no branch, it uses `label`. All other workspaces use `label` (`:669-677`).
- **Rows:** come from `ui.sidebar.spaces.rows` (see 2.8). Indented children drop `branch` and `git_status` (`src/ui/sidebar/tokens.rs:153-161`).
- **Height:** the number of resolved rows, at least 1, capped at the body height.
- **Gap:** `spaces.row_gap` is added only when the next entry is not indented (`sidebar.rs:259-267, 352-355`).

#### Displayed status (`displayed_workspace_status`, `sidebar.rs:633-661`)

- Normally `workspace.agent_status`.
- For a non-linked parent of a collapsed group: the highest-priority status over all members of that key.

#### Drawing workspace rows (`render_workspace_rows`, `sidebar.rs:692-788`)

Indent prefixes, in fg overlay0 (`:711-730`):

| Row | First line | Continuation lines |
|---|---|---|
| Indented child | `"   ├─ "` | `"   │    "` |
| Indented last child | `"   └─ "` | 8 spaces |
| Not indented | starts at x+1 | starts at x+3 |

Text ends at `area.right() - 2` (`:764-768`).

Token styles:

| Token | Style |
|---|---|
| state_icon | `status_icon` with fg `status_color` |
| state_text | fg `status_color` |
| workspace | focused or dragged: fg `text` + BOLD. Otherwise: fg `subtext0` (`:736-747`). |
| branch and other secondary tokens | fg `mauve` if focused, else `overlay0` (`:748-752`) |
| custom, terminal_title | fg `overlay1` (`:762`) |
| git_status | `↑N` in green, `↓N` in red (`ui/sidebar.rs:246-265`) |

Row background, in order of precedence (`sidebar.rs:772-787`, helpers `:7-25`). The final pass sets the background of every cell in the entry rect:
1. **Navigate target (selected):** `selection_bg`, or `active_row_bg` when `selection_bg` is Reset.
2. **Dragged:** `surface1`.
3. **Focused:** `active_row_bg`. While navigating with `selection_bg` = Reset, the focused row gets `sidebar_bg` instead, so only the cursor row is highlighted.

Gap rows are never highlighted.

"Selected" means one of these:
- the Navigate-mode target;
- a pending highlight that lasts up to 1 s after a focus request (`composition.rs:162-172`, `src/client/shell/workspace_navigation.rs:12-17, 187-196`).

#### Group toggle (`render_parent_group_toggle`, `sidebar.rs:603-631`)

- Drawn on a non-linked workspace that has at least one linked member with the same key (`:556-571`).
- Glyph: `▾` when expanded, `▸` when collapsed, in fg `accent`.
- Position: the last column of the entry's first line. The scrollbar column is excluded.
- Hit: `WorkspaceHit.group_toggle`.

#### Scrolling (`sidebar.rs:268-299, 358-362`)

- The scroll unit is one entry. `workspace_scroll` is the index of the first visible entry.
- Metrics: `list_scroll_metrics` (`src/client/shell/scroll.rs:5-54`).
- The list scrolls to show the focused workspace on first render and whenever `reveal_focused_workspace` is set (`scroll.rs:56-77`).
- **Scrollbar:** shown only when the list overflows and the body is wider than 1. It takes the last column of the body.
  - Track `▕` in fg `surface_dim`, thumb `▕` in fg `overlay0` (`scroll.rs:79-100`).
  - Thumb math: `src/ui/scrollbar.rs:37-71`.
- An entry that does not fully fit is not drawn. There are no other "more" markers.

#### Drag-and-drop reorder (`sidebar.rs:364-376`)

While a `ClientChromeDrag::Workspace` is active:
- The dragged row gets bg `surface1`. Its workspace text is BOLD in `text`.
- A drop line `─` × body width, in fg `accent`, is drawn at `workspace_drop_indicator_row`. It shows only when `workspace_area.y+1 <= row < workspace_area.bottom()-1`.
- The row is the target hit's `rect.y - 1`, or the last hit's `rect.bottom()` (`src/client/shell/mouse.rs:511-585`).
- Only non-indented rows (group parents) are drop slots.

#### Footer (`sidebar.rs:378-431`)

The footer is the last row of the spaces section. It is drawn only with `mouse_capture`.
- **Left:** `" new"` in fg overlay0. Hit: `hits.new_workspace`, 5 columns.
- **Right, normal:** `"menu"` in fg overlay0, right-aligned. Hit: `hits.global_launcher`, 6 columns.
- **Right, attention** (`update_available` or `integration_updates_available`, `src/client/shell/global_menu.rs:9-11`): `"● "` in fg accent + BOLD at `right-6`, then `"menu"` in overlay0. Hit: 8 columns.

#### Collapse toggle (`sidebar.rs:442-455`)

- `«` in fg overlay0 at `(area.right()-2, area.bottom()-1)`. It draws over the agent scrollbar's last cell. Hit: `hits.sidebar_toggle`.
- `hits.sidebar_divider` is the separator column, full height (`sidebar.rs:209-213`).

### 2.5 Agents section (`src/client/shell/agent_sidebar.rs`)

#### Header (`render_agent_panel_header`, `:90-152`)

- **Row 0:** `─` × width in fg `surface_dim`. This is the visible divider, and its row is the `sidebar_section_divider` hit.
- **Row 1, left:** `" agents"` in fg overlay0 + BOLD.
- **Row 1, right:** right-aligned, BOLD.
  - The sort label: `"grouped"` (sort = spaces) or `"priority"`, in fg overlay0.
  - When an agent view is active, `agent_view_label` replaces it, in fg `accent`. The server sends the view's label, or `"filtered"` (`src/server/client_shell.rs:182-186`).
  - Hit `hits.agent_sort_toggle` is the label rect. It is set only with mouse_capture and when no view is active.
- **Row 2:** blank. The body starts at `y+3`.
- If the area is less than 2 rows tall, only the divider row is drawn.

#### Empty state (`:172-187`)

- When an agent view is active and no rows match: `" no matching agents"` in fg overlay0 + DIM.
- Otherwise nothing is drawn.

#### Order (`ordered_agent_pane_ids`, `:20-50`)

- **Agent view active:** the server's `agent_order`.
- **`spaces`, shown as "grouped":** server order, which is workspace, then tab, then pane-layout order (`src/app/agents.rs:23-37`).
- **`priority`:** a stable sort by status priority (descending), then `state_change_seq` (descending).

#### Row data (`agent_row`, `:248-319`)

| Token | Value |
|---|---|
| workspace | the agent's workspace label |
| tab | the tab label, only if the workspace has more than 1 tab or the tab has `custom_label` |
| pane | `agent.title`, else `pane.label` |
| agent | `display_agent`, else `name`, else `agent`, else `title` |
| state_text | `state_labels[status]` if set, else `blocked` / `done` / `working` / `idle`. Unknown shows `idle` here (`:388-396`). |

The canonical agent ID for `rows_by_agent` comes from `parse_agent_label(agent.agent)`.

#### Rendering (`render_agent_row`, `:321-374`)

- Line 0 is indented 1 column. Later lines are indented 3 columns.
- The focused agent row (`agent.focused`) gets bg `active_row_bg` across the full width.
- If no row resolves, a single `state_icon` line is drawn as a fallback.

| Token | Style |
|---|---|
| workspace | focused: fg `text` + BOLD; otherwise fg `subtext0` + BOLD |
| machine, tab, pane, agent, branch, custom, terminal_title | fg `overlay0` |
| state tokens | fg `status_color` |

#### Scrolling (`:189-234`)

- The unit is one entry. `agents.row_gap` goes between entries.
- The scrollbar uses the last column.
- Hits: `hits.agent_body`, `hits.agent_scrollbar`, `hits.agents` (rect + pane_id).

### 2.6 Collapsed compact sidebar, one machine (`render_collapsed_sidebar`, `sidebar.rs:47-197`)

The sidebar is 4 columns wide: 3 content columns plus `│`.

**Sections** (`:27-45`):
- If the content height is under 7: workspaces only.
- Otherwise, top to bottom:
  1. Workspaces: `ceil(h/2)` rows.
  2. One divider row: `───` in fg `surface_dim`.
  3. Agents. The last agent row is kept for the toggle.

**Workspace rows:**
- One row per workspace, in snapshot order. There is no grouping and no scrolling; rows that do not fit are cut off.
- `format!("{:<2}", index+1)` in columns 0–1:
  - selected: fg `overlay1` on the selection bg
  - focused: fg `text` on `active_row_bg`
  - otherwise: fg `overlay0`
- Column 2 holds the status icon in fg `status_color` (`:72-103`).

**Agent rows:**
- `{:<2}` with index+1, in fg `text` when focused, else fg `overlay0`. Then the icon.
- The focused row gets bg `active_row_bg`.
- Order follows `agent_panel_sort` (`:130-172`).

**Toggle:**
- `»` in column 1 of the bottom row.
- Its fg is `accent` + BOLD when the global menu needs attention, else `overlay0` (`:173-196`). This is the only attention signal, because there is no menu button in this view.

**Hits:** `workspaces` (no group toggle), `agents`, `sidebar_toggle`. These are not set: `sidebar_divider`, `workspace_body`, `new_workspace`, `global_launcher`. So there is no wheel scrolling and no width dragging.

The token row config does not apply to the collapsed or mobile views (`configuration.mdx:480`).

**Hidden mode:** width 0, nothing is drawn. Use the toggle keybinding `prefix+b` to show the sidebar again.

### 2.7 Multi-machine sidebar (`src/client/shell/endpoint_sidebar.rs`, `endpoint_agents.rs`)

This is used when `endpoints.len() > 1`, i.e. Local plus saved SSH machines.
- `ClientShellEndpoint { endpoint_id, label, status, snapshot, agent_recency, ... }` (`src/client/shell/endpoints.rs:11-25`).
- The Local endpoint's label is `Local`.
- Possible statuses: Connecting, Online, Reconnecting, Attention, Disabled (`src/client/endpoint.rs:93-99`).

Status presentation (`endpoint_status_presentation`, `endpoints.rs:719-730`):

| Status | Glyph | Label | Color |
|---|---|---|---|
| Connecting | ◐ | connecting | yellow |
| Online | ● | online | green |
| Reconnecting | ◐ | reconnecting | yellow |
| Attention | ! | attention | red |
| Disabled | · | disabled | overlay0 |

Hovering a badge that has a diagnostic error adds bg `active_row_bg` + REVERSED (`src/client/shell/machine_diagnostics.rs:18-33`).

#### Expanded (`render_expanded`, `endpoint_sidebar.rs:237-580`)

- **Header:** `" machines"` in fg overlay0 + BOLD. It replaces `" spaces"`.
- **Rows:** for each endpoint, an endpoint row. If the endpoint is not collapsed and has a snapshot, its workspace entries follow, using that endpoint's collapsed groups.

Endpoint row (`render_endpoint_row`, `:590-649`):
- Text: `" {▾|▸} {label}"`, BOLD.
  - fg `text`, or `overlay0` if Disabled.
  - Hard-truncated, with no ellipsis.
- Right-aligned signal, in the presentation color. The first match wins:
  1. `"! auth"`: auth error
  2. `"! error"`: Attention
  3. empty: Local
  4. `"●"`: Online
  5. `"{glyph} {label}"`: any other status
- bg `active_row_bg` when the endpoint is collapsed and active.
- Hit: `MachineHit { rect, status_badge, collapse_toggle = (x+1, 1 cell) }` (`endpoints.rs:27-32`).

Workspace rows under an endpoint:
- Drawn by the same `render_workspace_rows`, but nested 2 columns to the right.
- "Focused" requires the endpoint to be active.
- No drag, and no drop indicator.
- If the endpoint is not Online, the rows get fg `overlay0` + DIM (`:461-491`).
- `row_gap` applies only between workspaces of the same endpoint.

Footer:
- `" new · {active endpoint label}"` in fg overlay0.
- Menu: `"● menu"` all in fg accent, not bold, on attention. Otherwise `"menu"` in fg overlay0 (`:519-555`).

Agents section:
- Same header and list.
- Rows come from `agent_row(..., machine = Some(endpoint.label))`, so the `machine` token shows (`endpoint_agents.rs:132-177`).
- "Focused" requires the active endpoint.
- If the endpoint is not Online, the row gets fg overlay0 + DIM.
- Hits: `hits.endpoint_agents` (rect, endpoint_id, pane_id).

Agent order (`src/client/shell/aggregate_navigation.rs:56-171`):
- `spaces`: endpoint by endpoint.
- `priority`: by stale, then priority (descending), then client recency (descending).
- Client recency is bumped whenever an agent's `state_change_seq` changes (`endpoints.rs:590-622`).

#### Compact (`render_collapsed`, `endpoint_sidebar.rs:15-235`)

- **Endpoint rows:** `"{▸|▾}{L or index+1}"`. `L` marks Local; others show their 1-based index.
  - fg `text` if Online, else `overlay0`.
  - bg `active_row_bg` if active and collapsed.
  - Remote endpoints get a right-aligned status glyph.
- **Workspace rows:** `" {workspace.number}"` plus the status icon. Stale endpoints get fg `overlay0` + DIM.
- **Agent rows:** `"{first char of machine label}{icon}"`.
- **Toggle:** `»`, always fg overlay0.

### 2.8 Agent state glyphs and colors (`src/client/shell.rs:178-237`)

| Status | `dots` (default) | `symbols` | Color | Priority | Text |
|---|---|---|---|---|---|
| Blocked | ● | × | red | 4 | blocked |
| Done | ● | ✓ | teal | 3 | done |
| Working | ● | ◐ | yellow | 2 | working |
| Idle | ○ | ○ | green | 1 | idle |
| Unknown | · | · | overlay0 | 0 | unknown |

- Config key: `ui.status_indicators = "dots" | "symbols"` (`src/config/model.rs:111-127`).
- An unknown status string on the wire becomes Unknown (`wire.rs:895-911`).

#### Done vs. idle (`src/client/shell/endpoint_agent_state.rs`)

The client re-projects each server status:
1. A new `boot_id` resets everything and marks every current `state_change_seq` as acknowledged. Nothing shows "done" at startup (`:48-59`).
2. Working records "observed work". Blocked clears completion (`:80-88`).
3. Idle or Done counts as completed if the client saw Working before, or if the server's `EndpointAgentCompletions` entry matches `state_change_seq` with the same boot, revision and generation (`:89-107`, `src/server/client_shell.rs:27-36`).
4. The displayed status is Done when completed and not yet acknowledged; otherwise it is Idle (`:156-175`).
5. The client acknowledges when a presented pane surface includes that agent's pane and the outer terminal is not known to be unfocused (`:118-154`).
6. Tab and workspace `agent_status` become the highest-priority status over their agents (`:178-201`).

### 2.9 Token system (`src/config/sidebar.rs`, `src/ui/sidebar/tokens.rs`, `src/ui/sidebar.rs`)

#### Config keys

- `[ui.sidebar.agents]`: `rows`, `rows_by_agent`, `row_gap`.
- `[ui.sidebar.spaces]`: `rows`, `row_gap`.

Defaults (`config/sidebar.rs:440-476`, `configuration.mdx:366-380`):

```toml
[ui.sidebar.agents]
row_gap = 0
rows = [["state_icon","machine","workspace","tab"],["agent"]]
[ui.sidebar.spaces]
row_gap = 0
rows = [["state_icon","workspace"],["branch","git_status"]]
```

Limits:
- At most 16 rows and 16 tokens per row (`config/sidebar.rs:11-40`).
- `rows_by_agent` keys are exact canonical agent IDs, such as `claude`, `codex` or `pi`. An entry replaces `rows` for that agent (`:404-437`).

#### Tokens

- **Agent tokens:** `state_icon`, `state_text`, `machine`, `workspace`, `tab`, `pane`, `agent`, `terminal_title`, `terminal_title_stripped`, `$name` (`:329-341`).
- **Space tokens:** `state_icon`, `state_text`, `workspace`, `branch`, `git_status`, `$name` (`:382-391`).
- **Custom names:** `$` + 1–32 characters from `[A-Za-z0-9_-]` (`:221-242`).

#### Entry forms

- A plain string, or `{ token, fg?, bold?, dim?, rules? }` (`:173-219`).
- `fg` is `#RGB` or `#RRGGBB`.
- At most 16 rules. Rules are not allowed on `state_icon` or `git_status`.

#### Rules (`src/config/sidebar/rules.rs`)

- Each rule has exactly one condition: `equals`, `contains`, `starts_with`, `gt` or `lt` (`:51-68`).
- Options: `ignore_case` (text only, ASCII), `fg`, `bold`, `dim`, `hide`.
- `gt` and `lt` need a finite threshold. The value must parse completely as a finite f64, and the comparison is strict (`:82-91, 129-171`).
- The first matching rule wins. `hide` removes the token.
- Rules match the full value, before truncation.

#### Resolution (`tokens.rs:67-178`)

| Token | When present |
|---|---|
| state_icon, state_text, workspace | always |
| machine | multi-machine sidebar only |
| tab, pane, agent, terminal_title | when the value exists |
| branch, git_status | not on indented children. git_status needs ahead > 0 or behind > 0. |
| `$name` | when the metadata token exists |

- A row with no tokens left is dropped.
- Custom token values come from server `MetadataTokens`, with an optional TTL (`src/metadata_tokens.rs:43-104`). Set them with `herdr pane|workspace report-metadata --token k=v`.

#### Rendering (`resolved_token_spans`, `ui/sidebar.rs:106-275`)

**Separators** (`tokens.rs:180-188`), in fg overlay0:
- `" "` after `state_icon` and before `git_status`.
- `" · "` everywhere else.

**git_status:** `↑{ahead}`, a space only when both counts are nonzero, then `↓{behind}`. A token `fg` overrides both colors.

**Width fitting:**
1. Icon and git_status widths are fixed.
2. The minimum width is the fixed widths, plus 1 per text token, plus the separators.
3. If the minimum is too wide: hide all text tokens, then add them back from last to first while they still fit.
4. Each visible text token gets 1 column.
5. The remaining width is shared round-robin, +1 column at a time, until each token reaches its full width.
6. Truncation uses `…` (`src/ui/text.rs:7-20`).

**Style patch** (`ui/sidebar.rs:277-296`):
- `fg` replaces the color. `bold` and `dim` add or remove the modifier.
- Separators and row backgrounds are never changed.

### 2.10 Sidebar runtime data (`src/protocol/wire.rs:916-1086`)

**Snapshot:** `boot_id`, `revision`, `focused_workspace_id`, `agent_view_label`, `agent_order`, `update_available`, `integration_updates_available`, `workspaces`, `tabs`, `panes`, `agents`.

**Workspace:**
- Fields: `workspace_id`, `number`, `label`, `custom_label`, `branch`, `git_ahead_behind`, `tokens`, `worktree { key, label, is_linked_worktree }`, `focused`, `agent_status`.
- `label` comes from `display_name_from` (`src/app/creation.rs:394`).
- The worktree key and label are the repo key and repo name (`src/server/client_shell.rs:76-97`).

**Tab:** `tab_id`, `workspace_id`, `label`, `custom_label`.

**Pane:** `pane_id`, `label`.

**Agent:** `pane_id`, `workspace_id`, `tab_id`, `name`, `display_agent`, `agent`, `title`, `terminal_title`, `terminal_title_stripped`, `agent_status`, `state_change_seq`, `state_labels`, `tokens`, `focused`.

### 2.11 Sidebar hit regions

Section 6 has the full mouse behavior.

| Hit | Action |
|---|---|
| `sidebar_toggle` | click: toggle collapsed |
| `sidebar_divider` | drag: resize width to `column + 1`, clamped. Double-click within 350 ms: reset to the configured width. |
| `sidebar_section_divider` | drag: set the split ratio |
| `workspace_scrollbar`, `agent_scrollbar` | drag the thumb, or click the track to jump |
| `agent_sort_toggle` | click: toggle spaces ↔ priority |
| `machines` | click: collapse toggle or activate. The status badge shows the error notice. |
| `global_launcher` | click: toggle the global menu |
| `new_workspace` | click: new workspace |
| `workspaces[].group_toggle` | click: toggle the group |
| `workspaces[]` | click: focus. Drag: reorder. Right-click: workspace context menu. |
| `agents[]`, `endpoint_agents[]` | click: focus that pane |
| `workspace_body`, `agent_body` | wheel: ±1 entry |

### 2.12 Sidebar quirks

- **Unknown state text differs.** `state_text` for Unknown is `idle` in agent rows (`agent_sidebar.rs:394`) but `unknown` in space rows (`shell.rs:221`).
- **The attention menu style differs.** The single-machine view uses `●` in accent + BOLD. The multi-machine view uses `● menu` all in accent, not bold.
- **Compact numbering differs.** The single-machine compact view numbers workspaces by position. The multi-machine compact view uses `workspace.number`.
- **Wheel scrolling (not verified).** The multi-machine compact view sets no `workspace_body`, so the wheel seems unable to scroll there. I read this from the code and did not test it.

## 3. Tab bar (`src/client/shell/tabs.rs`)

There is no per-tab agent status indicator. `ClientShellTab.agent_status` is sent (`src/server/client_shell.rs:120`), but `tabs.rs` never draws it. Tabs also have no hover style.

### 3.1 Constants and contents

| Constant | Value | Source |
|---|---|---|
| `MIN_TAB_WIDTH` | 8 | `src/client/shell/state.rs:3` |
| `NEW_TAB_WIDTH` | 3 | `src/client/shell/state.rs:4` |
| `TAB_SCROLL_BUTTON_WIDTH` | 3 | `tabs.rs:3` |
| `MIN_TAB_STRIP_WIDTH` | 17 | `tabs.rs:4-5` |

- The whole row is filled with bg `panel_bg` (`tabs.rs:18`).
- The bar shows only the tabs of the focused workspace, in snapshot order (`tabs.rs:19-23`).
- With 0 workspaces the bar still shows. It is empty except for `+`.

### 3.2 Tab label

- **Text:** `tab.label` is `tab_display_name`. That is the custom name, or else the tab's 1-based position as a number (`src/workspace.rs:442-449`, `src/app/creation.rs:224`). The automatic name is never the process name or the cwd.
- **Custom flag:** `custom_label = custom_name.is_some()` (`src/workspace/tab.rs:200-202`).
- **Zoom marker:** a zoomed tab shows `"{label} Z"` (`tabs.rs:374-380`).
- **Width:** `display_width(label) + 4`, minimum 8 (`tabs.rs:24-30`).
- **Placement:** centered, with `left = padding/2` (`tabs.rs:117-125`). Overlong text is clipped with no ellipsis.
- **Gap:** 1 blank cell between tabs (`tabs.rs:129`).
- **Partial tab:** a tab that does not fully fit is drawn at the remaining width, and no more tabs are drawn after it (`tabs.rs:97-101, 130-132`).

### 3.3 Tab styles (`tabs.rs:103-116`)

`contrast_fg` is `panel_bg`, or `surface_dim` when `panel_bg` is Reset (`src/client/shell.rs:239-244`).

| State | fg | bg | Modifier |
|---|---|---|---|
| focused, auto name | `contrast_fg` | `accent` | none |
| focused, custom name | `contrast_fg` | `accent` | BOLD |
| unfocused, custom name | `overlay1` | `surface0` | none |
| unfocused, auto name | `overlay0` | `surface0` | none |

### 3.4 New-tab button

- Text `" + "`, fg `overlay1` on `panel_bg` (`tabs.rs:169-178`).
- Drawn only with `mouse_capture` (`tabs.rs:32-33`).
- Without overflow, it sits after the last tab, past the 1-cell gap.
- With overflow, it sits after the right scroll button (`tabs.rs:152-167`).

### 3.5 Overflow (`tabs.rs:34-209`)

**When it overflows:** when the desired total width exceeds the content width. The desired total is the tab widths, plus `(n-1)` gaps, plus 3 for `+` when mouse capture is on. With mouse capture, there is also a condition: the content width must be at least 17.

**Scroll position (`tab_scroll`):**
- 0 when there is no overflow.
- When `reveal_focused_tab` is set, the scroll centers on the focused tab (`tabs.rs:330-353`).
- Otherwise it is clamped to `max_tab_scroll`, the start of the longest suffix that fits (`tabs.rs:355-372`).
- `reveal_focused_tab` is set again when the tab bar width changes (`composition.rs:189-192`).

**Scroll buttons** (overflow with mouse capture only):
- `" < "` at the left and `" > "` at `content.right - 6`.
- bg `surface0`. The fg is `overlay1` when that direction can scroll, else `overlay0` (`tabs.rs:63-151`).

**Ellipsis `…`** (fg `overlay0`; the bg is kept):
- Drawn after the left button when the first visible index is > 0.
- Drawn before the right button when more tabs are hidden on the right.
- Without mouse capture, overflow shows only the ellipses, with no buttons (`tabs.rs:180-209`).

### 3.6 Drag reorder indicator

- `│` in fg `accent` on the tab row, in the gap at the insert index (`tabs.rs:211-222, 294-328`).
- The drag starts when the pointer moves at least 1 cell from the press (`src/client/shell/mouse.rs:1240-1255`).
- The insert index comes from tab midpoints (`mouse.rs:465-508`).

### 3.7 Right status area (`ui.tab_bar_right`)

**Entry types** (serde tag `type`; at most 16 entries; `src/config/tab_bar.rs:7, 21-40`):

| Type | Fields |
|---|---|
| `zoom` | — |
| `hostname` | — |
| `datetime` | `format`, default `%H:%M`. `%z` and `%s` are rejected. |
| `text` | `text` |
| `command` | `command`, `interval_seconds` (default 5, range 1..=31,536,000), `timeout_seconds` (default 2, range 1..=3,600) |

**Server resolution** (`src/app/tab_bar_status.rs:45-347`, `src/server/client_shell.rs:190-221`):

| Type | Shows |
|---|---|
| `zoom` | `"ZOOM"` with `accent: true`, only while the focused tab is zoomed |
| `hostname` | the host name, control characters stripped, at most 80 chars |
| `datetime` | the formatted time, refreshed every 1 s |
| `command` | the last stdout line with ANSI/OSC/DCS sequences stripped. On failure, timeout, or empty output it is hidden. Runs via `/bin/sh -lc` on Unix. |

Segments with empty text are dropped.

**Separator:** `ui.tab_bar_right_separator`, default `" "`, control characters stripped. It shows only between visible segments.

**Wire format:** `tab_bar_right: Vec<{text, accent}>` (`src/protocol/wire.rs:942-943`).

**Layout** (`tabs.rs:226-255`):
- The area is right-aligned, with width `w` = the sum of the segment and separator widths.
- It shows only if `area.width - (w+1) >= 17`. Otherwise the whole area is hidden.
- When it shows, the tab content area shrinks by `w+1`.

**Styles** (`tabs.rs:257-292`):
- Separator: fg `overlay0` on `panel_bg`.
- Accent segment (`ZOOM`): fg `contrast_fg` on bg `accent`, BOLD, no padding.
- Other segments: fg `overlay1` on `panel_bg`.

"update ready" is not shown here. It appears in the Navigate mode bar (1.3).

### 3.8 Tab bar hit regions

| Hit | Action |
|---|---|
| `hits.tabs` (rect, tab_id) | Left click without a drag: TabFocus (`mouse.rs:1376-1383`). Drag: reorder (`mouse.rs:1240-1275`). Right click: tab context menu (`mouse.rs:1846-1855`). |
| tab row, including buttons | Wheel up: PreviousTab. Wheel down: NextTab (`mouse.rs:1868-1893`). |
| `hits.new_tab` | Click: NewTab (`mouse.rs:2058-2064`). |
| `hits.tab_scroll_left` | Click: `tab_scroll` -1 (`mouse.rs:2065-2089`). Hover during a drag: insert index 0 (`mouse.rs:477-482`). |
| `hits.tab_scroll_right` | Click: `tab_scroll` +1, up to `count-1`. Hover during a drag: insert index at the end. |

- When the mode bar replaces a bottom tab bar, the tab hits are cleared (`composition.rs:335-340`).
- A tab press needs `mouse_capture` (`mouse.rs:2116-2118`).

### 3.9 Tab bar runtime data

- Per tab: `tab_id`, `workspace_id`, `label`, `custom_label`, `zoomed`, `focused`.
- Snapshot: `focused_workspace_id`, `tab_bar_right[{text, accent}]`, `tab_bar_right_separator`.
- Client state: `tab_scroll` and drag state.
- Server code: `src/server/client_shell.rs:112-121`.

## 4. Panes

### 4.1 Surface production

**Server:**
- `render_pane_surface` lays out the panes with `compute_tab_surface_for` (`src/server/client_shell.rs:286-303`, `src/ui/tab_surface.rs:52-92`).
- It renders through `render_tab_surface_virtual` → `render_panes` (`src/server/render_stream.rs:547-575`, `src/ui/tab_surface.rs:119-133`).
- In a zoomed tab, only the focused pane is laid out, and there are no split borders (`tab_surface.rs:66-74`).

**Data sent per pane** (`client_shell.rs:381-401`):
- `pane_id`, `content_revision`
- `rect`, `inner_rect`, `scrollbar_rect`
- `scroll {offset_from_bottom, max_offset_from_bottom, viewport_rows}`
- `focused`, `mouse_reporting`, `sgr_pixel_mouse`, `alternate_screen_active`, pixel size

**Data sent per split** (`client_shell.rs:412-438`): `direction`, `pos`, `area`, `hit_rect`, `path`.

### 4.2 Borders (`src/ui/panes.rs`)

**Config** (defaults at `src/config/model.rs:1216-1220`):
- `ui.pane_borders`: auto, always or off. Legacy `true` means auto and `false` means off (`model.rs:884-899`).
- `ui.pane_outer_borders`, `ui.pane_scrollbars`, `ui.pane_gaps`: all default true.

**Which edges get borders** (`apply_pane_chrome`, `panes.rs:198-265`):
- **Single pane:**
  - `auto`: no borders.
  - `always`: a frame, but only while `pane_outer_borders` is on.
- **Borders off and gaps on, with more than one pane:** each pane shrinks by 1 column if it has a neighbor on the right, and by 1 row if it has a neighbor below. This leaves a blank gap (`panes.rs:225-232`).
- **Borders shown:** the pane starts with `Borders::ALL`, then:
  - Gaps off: the RIGHT edge is dropped if there is a neighbor on the right, and the BOTTOM edge if there is a neighbor below. Neighbors share one divider.
  - Gaps on: each pane keeps its own full box. Neighbor boxes touch.
  - `pane_outer_borders = false`: edges on the outer bounding box are dropped.
- **Zoomed pane:** `Borders::ALL` if `shows_borders(multi) && pane_outer_borders`, otherwise no borders (`panes.rs:53-59, 387-417`).
- **Inner rect:** the ratatui `Block` inner rect (`panes.rs:155-161`).

**Glyphs** (`panes.rs:563-608, 667-706, 785-804`):
- Square line set: `┼ ┤ ├ ┴ ┬ │ ─ ┌ ┐ └ ┘`. Cells are merged by neighbor directions.
- With gaps off, T and cross junctions are added where splits meet (`panes.rs:610-665`).

**Color:**
- `accent` when the cell touches the focused pane, else `overlay0`.
- No bg and no modifier.
- With gaps off, the shared divider lines next to the focused pane count as touching it (`panes.rs:708-730`).

### 4.3 Pane titles (`panes.rs:25-32, 732-783`)

**When drawn:** only when the pane has a top border and `rect.width > 4`.

**Text** comes from `terminal.border_label(show_agent_labels)` (`src/terminal/state.rs:2391-2402`). The first non-empty value wins:
1. `effective_title()`. This is the newest agent-metadata title from hooks or the API (`src/terminal/metadata.rs:295-298, 418-423`). It is not the OSC title, the process name, or the cwd.
2. `manual_label` (the pane name).
3. Only when `show_agent_labels_on_pane_borders` is on: the display agent, or else the agent label (e.g. `claude`).

**Format:** the text is trimmed. If it is empty, no title is drawn. Otherwise the title is `" {truncate_end(label, width-4)} "`.

**Placement:** starts at `rect.x + 1` on the top border and is clipped at `rect.right - 1`.

**Style:** focused pane: fg `accent` + BOLD. Unfocused pane: fg `overlay0`.

### 4.4 Zoom

The pane itself has no zoom marker. Zoom shows in two places:
- the tab label suffix ` Z`;
- the optional `ZOOM` status pill in the tab bar.

Key: `zoom = "prefix+z"`, legacy alias `fullscreen` (`src/main.rs:188`).

### 4.5 Pane scrollbar (`src/ui/scrollbar.rs`, `panes.rs:36-51, 290-311`)

**Gutter:**
- The gutter is the last column of the inner rect. It is reserved when `pane_scrollbars && !alternate_screen_active && inner.width > 4`, and the terminal gets `width - 1`.
- It is reserved even when there is no scrollback, so the width stays stable. Without scrollback it stays blank.

**When the bar is drawn:** only when `max_offset_from_bottom > 0` (`scrollbar.rs:27-29`).

**Glyphs** (`scrollbar.rs:136-184`):
- Focused pane: track `▕` in fg `overlay0`, thumb `▐` in fg `overlay1`.
- Unfocused pane: track `▕` in fg `surface_dim`, thumb `▕` in fg `overlay0`.

**Thumb math** (`scrollbar.rs:37-71`):
- `total = max_off + viewport`
- `len = round(viewport * track_h / total)`, clamped to `1..=track_h`
- `top = round((max_off - off) * (track_h - len) / max_off)`

**Mouse** (`scrollbar.rs:108-134`; `mouse.rs:2160-2201`):
- Click on the track: the thumb centers on the clicked row.
- Drag the thumb: it keeps the grab offset.
- A press focuses the pane and sets Terminal mode.

**Other scroll indicators:** none. There is no text indicator. While the pane is scrolled back, the terminal cursor is hidden (`panes.rs:502-505`, `tab_surface.rs:171-188`).

### 4.6 Pane states

- **Live runtime:** the terminal content is drawn in the inner rect (`panes.rs:500-506`).
- **Restore error (no runtime):** the `restore_error` text is drawn wrapped, in the default style (`panes.rs:507-518`). Examples from `src/app/agent_resume.rs:260, 291`:
  - "Saved directory is unavailable. Restore the directory and restart this session."
  - "Could not start the saved shell: {err}. Fix the shell configuration and restart this session."
- **Process exits:** there is no exited or dead-pane display.
  - `PaneDied` removes the pane.
  - If it was the last pane, the workspace closes.
  - If no workspaces remain, the mode becomes Navigate (`src/app/actions.rs:1394-1397, 2059-2099`).
- **No workspace:** the pane surface is blank (`panes.rs:492-494`). There is no empty-state text anywhere in `src/client` or `src/ui`.

### 4.7 Selection rendering (`panes.rs:806-891`, `src/selection.rs`)

- **Visible states:** only Dragging or Done. A plain click (Anchored) shows nothing (`selection.rs:141-144`).
- **Coordinates:** rows are absolute buffer rows, so the highlight moves correctly with scrolling (`selection.rs:241-259`).
- **Background:** start from the host terminal background; if it is not known, use `panel_bg` (or `surface_dim` when `panel_bg` is Reset). If the color is RGB, mix it 28% toward white when the luminance is below 0.5, else toward black.
- **Foreground:** black or white, whichever has the better WCAG contrast. The style starts from `Style::reset()`.
- **Mouse selection** (details in 6.4):
  - When a drag ends with `copy_on_select`, the text is copied and the selection clears at once (`mouse.rs:1764-1766`).
  - A double click selects a word (`mouse.rs:2250-2259`, `src/client/shell/word_selection.rs`). A word highlight with no drag clears 500 ms after it is copied (`word_selection.rs:118-160`).

### 4.8 Copy mode display (`composition.rs:349-452, 728-804`; `src/client/shell/copy_mode.rs`)

**Search matches:**
- Non-current match: fg `text` on bg `surface1`.
- Current match: fg `contrast_fg` on bg `accent`, BOLD.
- Draw order: non-current matches, then the selection, then the current match.

**Copy cursor:** the host cursor is hidden. The cursor cell is styled fg `contrast_fg` on bg `accent`, BOLD.

**Stale-data guard:** highlights are drawn only when the copy-mode geometry and scroll offsets match the pane (`composition.rs:728-738`).

**Selection kinds:** `v` or space for character-wise, `V` for line-wise (`copy_mode.rs:212-213, 444-470`).

**Entering copy mode:** the cursor starts at the host cursor if it is inside the pane, else at the bottom-left (`copy_mode.rs:40-68`).

**Leaving copy mode:** the scroll offset is restored to the value it had on entry (`copy_mode.rs:813-855`).

**Page sizes** (`src/copy_mode.rs:5-86`): a full page is `height - 2`, a half page is `height / 2`, and both are 1 when the height is 2 or less.

**Search prompt and status:** see the COPY mode bar in 1.3.

### 4.9 Link hover (`src/client/shell/link_hover.rs`)

**Trigger:** a mouse move with **Ctrl** held (`:81-88`). All of these must also be true (`:40-48`):
- mouse capture is on;
- the mode is Terminal;
- no overlay and no popup is open;
- the outer terminal has focus;
- no drag or pane mouse gesture is in progress.

**Link regions:**
1. OSC 8 hyperlinks with safe web URLs. A region may span rows (`:231-306`).
2. If there are none, the server method `pane.link.resolve` is used, when the endpoint advertises it (`:144-184`).

**Look:** the UNDERLINED modifier is added. Colors do not change (`:321-364`).

### 4.10 Popup pane

Sources: `composition.rs:561-596`, `src/popup_size.rs`, `src/app/popup.rs`, `src/server/client_shell.rs:497-539`.

**Trigger:** a custom command keybinding with `type = "popup"` and optional `width`/`height` (`configuration.mdx:197-226`). Each size is a cell count or a percentage string such as `"80%"`, from 1 to 100 (`popup_size.rs:6-41`). Only one popup can be open at a time (`app/popup.rs:116-118`).

**Geometry** (`popup_size.rs:50-90`):
- The default size is 50% × 50% of the pane surface, at least 6×4, and capped at the surface size. The popup is centered.
- The inner rect is inset by 1 on each side. When the inner width is greater than 4, 1 more column is reserved on the right and left blank.

**Frame:**
- `Clear`, then a `Block` with all borders (plain lines) in fg `accent` on bg `panel_bg`.
- The title is the ratatui default: top-left at `x+1`, with no padding and no style.
- The title text is the terminal's `manual_label`, or `"popup"`.

**Hit:** `hits.popup` with `popup: true`.

**Close:** the popup closes when its command exits. The mode then goes back to Terminal, or to Navigate when there is no workspace (`app/popup.rs:16-33`).

### 4.11 Copy feedback toast (`src/ui/status.rs:15-88`)

- **Text:** `copied to clipboard`. It shows for 2 s, only when `ui.toast.clipboard.enabled` is set (default true) (`src/client/shell/state.rs:1822-1830`).
- **Size:** width = message length + 4, height = 3.
- **Area:** the pane surface, or the full screen on mobile.
- **Position:** `ui.toast.clipboard.position`, one of `top_left`, `top_center`, `top_right`, `bottom_left`, `bottom_center` (default), `bottom_right` (`model.rs:85-93`).
- **Offset:** +1 when the config diagnostic banner shows. If it overlaps the notification toast, it moves by the toast's height (`src/ui.rs:133-146`).
- **Look:**
  - `Clear`, then a box with all borders in fg `green` on `panel_bg`.
  - Inside: `●` in green, a space, then the message in fg `text` + BOLD.

### 4.12 Config diagnostic banner (`status.rs:90-125`; `src/config/io.rs:202-242`)

**Text:** `"{config.toml}{impact}; herdr config check"`. The impact part is one of:
- ` invalid; using defaults`
- ` unreadable; using defaults`
- ` invalid; keeping current config`
- ` unreadable; keeping current config`
- ` has unknown keys`
- (empty)

**Client and endpoint messages** (`src/client/shell/config.rs:3-16`):
- Same text from both: `client + endpoint: {x}`.
- Different texts: two lines, `client: {a}` and `endpoint: {b}`.

**Look:**
- One row per line, from row 0 of the full screen (the pane surface on mobile).
- Each row is right-aligned as `" {line} "`.
- fg `contrast_fg` on bg `yellow`, BOLD.

### 4.13 Other pane-related files

| File | Role |
|---|---|
| `src/client/shell/graphics.rs` | Kitty graphics placement only. It uses the occlusion rects collected during compose. It draws no chrome. |
| `src/client/shell/scroll.rs` | List scroll math and the list scrollbar for the sidebar and overlays. |
| `src/app/agent_view.rs` | A server-side filter and sort for the agent sidebar list only. Labels are at most 32 chars. It does not draw panes. |

### 4.14 Pane runtime data

**Layout and focus:**
- Layout rects and the borders config.
- The focused pane.

**Per terminal:**
- The agent-metadata title.
- `manual_label`.
- The display agent and agent label.

**Per pane state:**
- Scroll metrics.
- `alternate_screen_active`.
- `restore_error`.
- `mouse_reporting` and `sgr_pixel_mouse`.

**Other:**
- The host background color, used for the selection color.
- Popup: `terminal_id`, title, size spec, frame.

## 5. Popups, dialogs, overlays and menus

### 5.0 Shared rules

**Dimming and the mode bar**
- `render_client_overlay` adds DIM to every cell on screen first. Navigator, ContextMenu and GlobalMenu skip this step (`src/client/shell/overlays.rs:45-57`).
- The mode bar is hidden while any overlay is open (`composition.rs:321`).
- If an overlay renderer returns `None` because the area is too small, `compose()` returns `None` and no frame is drawn. This is a Herdr quirk.

**`panel(b, rect, border, bg)`** (`overlays.rs:221-263`)
- Fills the rect with spaces on `bg` and removes DIM.
- Draws a single-line box `┌ ─ ┐ │ └ ┘` in fg `border`.
- Returns the inner rect, inset by 1 on each side.

**`popup(area, w, h)`** (`overlays.rs:264-280`)
- `w = min(w, area.w - 4)` and `h = min(h, area.h - 2)`.
- Centers the result. Returns `None` when `w < 4` or `h < 4`.

**`button(rect, text, style)`** (`overlays.rs:281-285`)
- Fills the rect with `style` and centers the text in it.

**`row(inner, widths, gap, off)`** (`overlays.rs:286-301`)
- Lays out a row of buttons, centered.
- The row sits at `inner.y + min(off, h-1)`.

**`contrast`** (`overlays.rs:302-307`)
- `panel_bg`, or `surface_dim` when `panel_bg` is Reset.
- This is the fg used on accent-colored buttons and on selected rows.

**`modal_stack_areas(inner, header, footer, actions, gap)`** (`src/ui/widgets.rs:38-88`)
- Splits the inner area into header, content, footer and actions, top to bottom.

**Fixed button rects** (`widgets.rs:90-109`)
- `close_button_rect`: `" esc close "`, 11 wide, right-aligned.
- `continue_button_rect`: `" ↵ continue "`, 12 wide, left-aligned.

**Overlay scrollbars** (`src/ui/scrollbar.rs:139-165`)
- Track `▕` in `overlay0`, thumb `▐` in `overlay1`. Used by ReleaseNotes, ProductAnnouncement and Navigator.
- Help draws `▐` for both track and thumb (`overlays.rs:1218-1231`).
- The bar takes the rightmost column of the body, and only when the content overflows.

**Key routing** (`src/client/shell/input.rs:610-641`)
1. Onboarding, ProductAnnouncement and ReleaseNotes get keys first, even before a popup terminal.
2. Then a popup terminal.
3. While `popup_pending`, keys are dropped.
4. Then the other overlays (`overlay_input.rs:572-630`).

Every key an overlay does not handle is swallowed. Nothing reaches the pane, and the prefix does nothing.

**Effect of a popup terminal**
- When a popup terminal appears or goes away, the overlay is reset. Only Onboarding and ProductAnnouncement survive (`state.rs:1665-1689`).

#### Text editor

The text editor (`src/client/shell/text_editor.rs`) is used by Rename, Help filter, Navigator search, worktree fields and copy-mode search.

**State**
- `TextEditor { text, cursor, replace_on_type, killed }` (`:5-11`).
- `replace_on_type` means a suggested value is replaced by the first keystroke. There is no visual selection to show it.

**Input normalization** (`:69-96`)
- CR, LF and TAB become a space.
- Other control characters are dropped.

**Keys** (`:163-238`, user doc in `keyboard.mdx` "Editing text fields")

| Keys | Action |
|---|---|
| Left/Right, Ctrl+B/F | move by grapheme |
| Home/End, Ctrl+A/E | move to start/end |
| Alt+B/F | move by word. Word classes: whitespace, alnum+`_`, other (`:115-148`) |
| Backspace, Ctrl+H | delete back. Clears all if `replace_on_type` is set |
| Delete, Ctrl+D | delete forward |
| Ctrl+U / Ctrl+K | kill to start / end |
| Ctrl+W, Ctrl+Backspace, Alt+Backspace | kill previous word |
| Alt+D | kill next word |
| Ctrl+Y | yank. The kill buffer is per field, not the clipboard |

- Plain chars and Shift+chars insert text.
- Enter and Esc are left to the dialog.

**Display** (`:240-288`)
- The viewport keeps the cursor visible. Text before the cursor uses at most `width - 1` cells.
- The field fills its area in the field style. The host cursor is placed at the edit point.

**Paste** (`input.rs:26-38, 248-314, 549-608`; `overlay_input.rs:447-471`)
- Ctrl+V (Cmd+V on macOS) reads the system clipboard into the active field.
- Bracketed paste and IME text also go into the active field.

### 5.1 Onboarding (`ClientShellOverlay::Onboarding`)

**Trigger**
- At startup when the config key `onboarding` is missing or true (`src/config.rs:94-96`, `src/client/shell/state.rs:999-1001`).
- It opens again after an endpoint reset (`state.rs:1273-1276`).
- While it is pending, product announcements are blocked (`state.rs:1533`).

**Page.** There is a single page (`src/ui/onboarding.rs:3-14`, `overlays.rs:545-636`).

Geometry:
- Width `popup_with_width_cap(area, max(hint_width+5, 64), 16, 3)`, which is 64×16 by default. Border `accent`.
- If the inner height is under 11, only the empty panel is drawn.
- Layout: `modal_stack_areas(inner, 2, 0, 1, 1)`.

Lines:
- Header:
  - `"  herdr"` in `text` BOLD.
  - `"  terminal workspace manager for coding agents"` in `overlay0`.
- Content, in `overlay1`:
  - `"  this is a mouse-first terminal."`
  - `"  click the sidebar to switch workspaces, drag pane"`
  - `"  borders to resize, right-click for context menus."`
  - a blank line
  - `"  "` + `{primary prefix}` (accent BOLD) + `" enters prefix mode · "` + `"?"` (accent BOLD) + `" shows keybinds and settings"`
  - `"  next: install optional agent integrations for more reliable state"`
- The `?` is hard-coded and does not follow the help binding.
- Action: `" ↵ continue "`, left-aligned, fg contrast on bg `accent`, BOLD.

**Keys** (`overlay_input.rs:480-488`)
- Enter, Right or `l` completes onboarding. All other keys are ignored. Text and paste are dropped.

**Mouse** (`mouse.rs:696-703`)
- Only Left Down on the continue button acts. Everything else is swallowed, including clicks outside.

**Complete** (`overlay_input.rs:177-191`)
1. Write `onboarding = false` to the local config.
2. Open Settings on the Integrations tab. This sends `IntegrationList`.

Before the first snapshot arrives, completing does nothing.

**Data:** primary prefix label, local config path.

### 5.2 Product announcement (`ProductAnnouncement`)

**Trigger** (`state.rs:1533-1569`; `src/product_announcements.rs:100-198`)
- The snapshot field `product_announcement {version, id, title, body, preview}` opens it automatically.
- It does not open while onboarding is pending, or if the user dismissed that announcement.
- It closes when the field goes away.
- Dev env vars `HERDR_FAKE_PRODUCT_ANNOUNCEMENT_*` create a preview.

**Geometry**
- `popup(88, 24)`, border `accent`. Below 20×8 inner, only the panel is drawn.
- Layout: `modal_stack_areas(inner, 2, 1, 0, 1)` (`overlays.rs:429-528`).

**Lines**
- Header row 0: the title in `text` BOLD, at x+1. A close button `" esc close "` sits flush right on the same row: contrast on `accent`, BOLD.
- Header row 1: `"product announcement · v{version}"`, or `"product announcement preview · v{version}"`, in `overlay1`.
- Body: the release-notes markup (5.3), word-wrapped and scrolled, with a scrollbar.
- Footer:
  - `" scroll "` in overlay0
  - `"wheel ↑↓"` in text
  - `"  ·  "` in overlay0
  - `"close"` in overlay0
  - `" esc / enter "` in text

**Keys** (`overlay_input.rs:490-533`)

| Key | Action |
|---|---|
| Enter, Esc | dismiss |
| Up/k, Down/j | ±1 |
| PageUp/PageDown | ±8 |
| Home / End | top / bottom |

**Mouse** (`mouse.rs:704-770`)
- Close button: dismiss.
- Scrollbar: drag the thumb or jump.
- Wheel: ±3.
- A click elsewhere does nothing (the dialog is fully modal).

**Dismiss** (`overlay_input.rs:4-33`)
- Sends `product_announcement.dismiss {version, id}`. If that call fails, the dialog reopens.

### 5.3 Release notes (`ReleaseNotes`, "what's new" / "update ready")

**Trigger**
- It never opens by itself at startup (test at `src/app/mod.rs:1675-1701`).
- It opens from the global menu item `update ready` (when `update_available` is set) or `what's new` (when `latest_release_notes_available` is set). The item does nothing while `snapshot.release_notes` is None (`global_menu.rs:39-48, 91-107`). Mobile has the same item (`mobile.rs:1071-1083`).
- On the server, the notes come from `release-notes.json`. `preview` is true when the stored version is newer than the running version (`src/release_notes.rs:73-119`).

**Geometry**
- `popup(80, 24)`. Same layout as 5.2 (`overlays.rs:314-412`).

**Lines**
- Header row 0: `"v{version}"` in `text` BOLD, plus the close button.
- Header row 1: `"update ready"` (preview) or `"what's new in this release"`, in `overlay1`.
- Body when `preview` is set, before the notes (`src/ui/release_notes.rs:144-187`):
  - `" ● update ready"`. The dot is accent BOLD; the text is text BOLD.
  - `" {update_install_instruction}"`, e.g. ``detach, run `herdr update`, then run Herdr again to reconnect``. The variants for brew, mise and nix are in `src/update.rs:1896-1911`.
  - A blank line.
- Footer: same as 5.2.

**Markup** (`release_notes.rs:13-142`)

| Source | Rendered as |
|---|---|
| ` ``` ` fence lines | Toggle a code block. The fence line is hidden. Each code line becomes `"▏"` (fg accent, bg surface1) + `" "` + text (fg text, bg surface1). |
| `### X` | `" " + X.to_uppercase()` in accent BOLD |
| `- X` | `" • "` in accent, then the inline text |
| Any other line (also `#` and `##`) | `" "` + the inline text |

Inline: `` `code` `` drops its backticks and is styled fg accent, bg surface0, BOLD. If the code contains `=`, its spaces become NBSP.

**Keys:** same as 5.2. End uses freshly computed geometry (`overlay_input.rs:104-152`).

**Mouse:** same as 5.2. The hit rects are recomputed from the last composed size.

**Dismiss** (`overlay_input.rs:73-102`)
- Sends `release_notes.dismiss {version}`.
- Switches the mode to Terminal, or to Navigate when there is no focused workspace.

### 5.4 Rename (`Rename`): one dialog, five targets

`ClientRenameOverlay { title, input, target }` (`state.rs:296-325`).

| Target | Title | Initial text | Replace on type | Opened by |
|---|---|---|---|---|
| NewWorkspace | `new workspace` | `derive_label_from_cwd(source cwd)` or `workspace` | yes | `new_workspace` (`prefix+shift+n`), the sidebar `new`, or mobile `+ new workspace`. Opens only when `ui.prompt_new_workspace_name` is true (default false). Otherwise it sends `WorkspaceCreate` directly (`actions.rs:83-102`, `overlay_input.rs:333-357`). |
| Workspace | `rename workspace` | the workspace label | no | `rename_workspace` (`prefix+shift+w`) on the navigate target or focused workspace, or the workspace context menu `Rename`. During a foreign preview it is blocked with the notice "Select an available workspace and press Enter before renaming or closing it" (`actions.rs:23-36, 103-107`). |
| NewTab | `new tab` | `tab count + 1` | yes | `new_tab` (`prefix+c`), the tab bar `+`, or the tab menu `New tab`. Opens only when `ui.prompt_new_tab_name` is true (default true) (`overlay_input.rs:380-402`). |
| Tab | `rename tab` | the tab label | no | `rename_tab` (`prefix+shift+t`), or the tab menu `Rename` |
| Pane | `rename pane` | the pane label or empty | only when there is no label | `rename_pane` (`prefix+shift+p`), or the pane menu `Rename pane` |

In Navigate mode, prefix actions work without the prefix (`input.rs:897-929`).

**Geometry** (`overlays.rs:649-688`)
- `popup(56, 7)`, border `accent`, inner 54×5.

| Inner row | Content |
|---|---|
| 0 | title, no leading space, `text` BOLD |
| 1 | blank |
| 2 | input. The full row has bg `surface0`. The editor starts at x+1 in `text` on `surface0`. Cursor shown. |
| 3 | buttons, `row([8, 10, 12], gap 2)` |

Buttons:

| Text | Style | Role |
|---|---|---|
| `" ↵ save "` | contrast on accent, BOLD | primary |
| `" ^c clear "` | text on surface0, BOLD | clear |
| `" esc cancel "` | text on surface0, BOLD | cancel |

**Keys** (`overlay_input.rs:898-931`)
- Enter saves. Esc closes.
- Ctrl+C clears. Super/Cmd+Backspace also clears.
- All other keys go to the editor.

**Mouse** (`mouse.rs:1717-1739`, Left Down only)
- The primary button saves. The clear button clears.
- A click anywhere else closes the dialog, including the cancel button and clicks inside the dialog.

**Save** (`overlay_input.rs:933-995`). The input is trimmed first.

| Target | Request |
|---|---|
| NewWorkspace | `WorkspaceCreate{label}`. The label is sent only if it is non-empty and differs from the suggested name. |
| Workspace | `WorkspaceRename`, only if non-empty |
| NewTab | `TabCreate{label}`. The label is sent only if it is non-empty and differs from the default. |
| Tab | `TabRename`, unless the input is empty, or the tab is auto-named and the input is unchanged |
| Pane | `PaneRename`, sent even when the input is empty |

**Data:** workspaces (id, label, `new_workspace_cwd`), tabs, panes (label), the focused ids, the ui flags. `derive_label_from_cwd` runs git discovery on the client (`src/workspace/git/discovery.rs:24-43`).

### 5.5 Confirm close (`ConfirmClose`)

`ClientConfirmCloseOverlay { workspace_id, close_group, tab_target, title, detail }` (`state.rs:573-580`).

**Triggers** (all require `ui.confirm_close`, default true)
- **Workspace:** `close_workspace` (`prefix+shift+d`), or the workspace menu `Close` / `Close group` (`overlay_input.rs:997-1022`).
- **Tab:** `close_tab` (`prefix+shift+x`) or the tab menu `Close`. It asks only when this is the workspace's last tab (`overlay_input.rs:1058-1078`).
- **Pane:** `close_pane` (`prefix+x`) or the pane menu `Close pane`. It asks only when this is the last pane of the last tab (`overlay_input.rs:1024-1056`).
- **Server fallback:** if the server rejects a close with error `confirmation_required`, the workspace-level dialog opens (`actions.rs:394-405, 844-855`).

**Text** (`overlay_input.rs:1129-1220`)
- Title: `Close worktree group?` when the close covers a group with more than one member. Otherwise `Close workspace?`, also for tab and pane closes.
- Detail: `"{workspace label} — {scope}"`. The scope is `1 pane`, `{n} panes`, or `{k} workspaces, {n} panes`.

**Geometry** (`overlays.rs:1261-1303`)
- `popup(64, 6)`, border `red`.
- Row 0: `" {title}"` in red BOLD.
- Row 1: `" {detail}"` in text.
- Row 3: the buttons:
  - `" ↵ confirm "`, 13 wide, contrast on **red**, BOLD.
  - `" esc cancel "`, 12 wide, text on surface0, BOLD.

**Keys** (`overlay_input.rs:885-896`)
- Enter accepts.
- Esc closes the dialog and switches to Navigate mode, with the focused workspace selected.

**Mouse**
- Primary button: accept.
- Any other click closes the dialog but does not switch to Navigate mode.

**Accept** (`overlay_input.rs:1080-1123`)
- First the target is checked again. If it has changed, the notice "Close target changed; try closing it again" shows.
- Then one of `PaneClose`, `TabClose` or `WorkspaceClose{close_group}` is sent.

### 5.6 Keybind help (`Help`) with filter

`ClientHelpOverlay { query, search_focused, scroll }` (`state.rs:372-377`).

**Trigger**
- The `help` binding, `prefix+?` (`?` alone in Navigate mode).
- The global menu entry `keybinds` (`actions.rs:51-59`).

**Geometry** (`overlays.rs:1125-1244`)
- `popup(76, 22)`, border `accent`.
- Not drawn when the inner area is narrower than 20 or shorter than 6.

| Inner row | Content |
|---|---|
| 0 | `keybinds` in text BOLD. A 13-wide button flush right: `" esc back "` while search is focused, else `" esc close "`. contrast on accent, BOLD. |
| 1 | Search focused: `" / "` followed by the editor. Unfocused: `" / press / to filter by command or shortcut"` in overlay0. |
| 3 … h-3 | body |
| h-1 | footer (see below) |

Footer, in overlay0:
- Focused: `" edit ←→/home/end · kill ^u/^k · yank ^y · scroll ↑↓ · back esc"`
- Unfocused: `" search / · scroll j/k/↑↓/pgup/pgdn · close esc/enter"`

**Body** (`overlays.rs:1053-1115`)
- Per group: `" {group}"` in accent BOLD.
- Each entry: `" {key:<key_width} "` in mauve BOLD, then the label in text.
- A blank line after each group.
- With no matches: `" no matching keybinds"` in overlay1.
- The body word-wraps and has a scrollbar.

**Content** (`src/input/keybind_help.rs:75-229`)
- Key labels are joined with ` / `. An unbound action shows `unset`. `minus` prints as `-`. Indexed keys collapse to `X1..9`.
- Groups and entries (defaults from `src/config/model.rs:1123-1182`):

| Group | Entries |
|---|---|
| `global` | prefix keys → prefix mode; `prefix+?` keybinds; `prefix+s` settings; `prefix+q` detach; `prefix+shift+r` reload config; `prefix+o` open notification target |
| `navigation` | `esc` back; `up / down` workspace list; `h / j / k / l / left / right` move focus; `tab / shift+tab` cycle pane; `enter` open workspace; `1..9` switch workspace |
| `workspaces / tabs` | `prefix+w` workspace navigation; `prefix+g` session navigator; `prefix+shift+n` new workspace; `prefix+shift+g` new worktree; open worktree (unset); delete worktree checkout (unset); `prefix+shift+w` rename workspace; `prefix+shift+d` close workspace; previous / next workspace, switch workspace 1-9, previous / next agent, focus agent 1-9 (unset); `prefix+c` new tab; `prefix+shift+t` rename tab; `prefix+p` / `prefix+n` previous / next tab; move tab left / right (unset); `prefix+1..9` switch tab; `prefix+shift+x` close tab |
| `panes` | `prefix+v` split vertical; `prefix+-` split horizontal; `prefix+x` close pane; `prefix+shift+p` rename pane; `prefix+e` edit scrollback; clear pane (unset); `prefix+[` copy mode; `prefix+z` zoom; `prefix+r` resize mode; resize pane l/d/u/r (unset); `prefix+b` toggle sidebar; `prefix+h/j/k/l` focus pane; `prefix+tab` / `prefix+shift+tab` cycle; last pane (unset) |
| `custom` | custom command bindings → their description, or `custom command` |

- Swap-pane bindings are not listed.

**Filter** (`keybind_help.rs:231-251`)
- Case-insensitive substring match on the key label or the description.
- Group names are not searched. Empty groups are hidden.

**Keys, unfocused** (`overlay_input.rs:840-882`)

| Key | Action |
|---|---|
| Esc, Enter, `?` | close |
| Up/k, Down/j | scroll |
| PgUp/PgDn | scroll ±8 |
| Home/End | top / bottom |
| `/` | focus search |

**Keys, focused** (`overlay_input.rs:793-838`)
- The editor handles keys first. Any edit resets scroll to 0.
- Esc leaves search and clears the query.
- Enter closes the dialog.
- Arrows, PgUp/PgDn and Ctrl+P/N scroll.

**Mouse** (`mouse.rs:1573-1643`)
- Wheel: ±3.
- Scrollbar: drag the thumb or jump.
- Close button: leaves search if it is focused, otherwise closes.
- Click outside the dialog: close.
- The search row is not clickable.

**Data:** live keybind config, prefixes, custom commands.

### 5.7 Navigator / Goto picker (`Navigator`)

`ClientNavigatorOverlay { query, search_focused, selected, scroll, filter }`. Targets: `Machine`, `Workspace`, `Pane` (`state.rs:327-370`).

**Trigger** (`src/input/keybindings.rs:156`, `actions.rs:46-50`)
- `keys.goto`, default `prefix+g`. In Navigate mode, `g` alone.
- On open, the selected row is the current pane.
- `prefix+w` is a different feature: it opens the sidebar Navigate mode, not this dialog.

**Geometry** (`overlays.rs:711-1035`)
- Size `min(W-4, 116)` × `min(H-2, 42)`, centered.
- Not drawn when narrower than 4 or shorter than 9.
- No background dim. Border `accent`.
- Title `" Go to "` on the top border at x+2, in accent, not bold.

| Inner row | Content |
|---|---|
| 0 | Search text, see below. Right-aligned: `"{n} terminal(s)"` in overlay0. |
| 1 | `─` rule in `surface1` |
| 2 … h-4 | rows |
| h-3 | `" {selected.detail}"` in `subtext0` |
| h-2 | `" {selected.meta}"` in `overlay0` |
| h-1 | footer, see below |

Search text, first match wins:
1. Search focused: `" / "` followed by the editor.
2. A filter is set: `" / blocked|working|idle|done"`.
3. The query is empty: `" / search agents and terminals"`.
4. Otherwise: `" / {query}"`.

Footer:
- Search focused: `" search type · move ↑↓/ctrl+n/p · open enter · back esc"`
- Otherwise: `" ↑↓/j/k rows · ←→ workspace · / search · a/b/w/i/d filter · enter open · esc close"`

**Rows** (`src/client/shell/aggregate_navigation.rs:278-478`)

Machine rows:
- Shown only with more than 1 endpoint, at depth 0.

Workspace rows:
- `label` = the workspace label. `meta` = the branch. `detail` = `new_workspace_cwd`.

Pane rows:
- Status: the agent status, or Unknown.
- Label when the tab has one pane: the pane name, the tab name, or the title. If none exists, `"{agent kind|terminal} · {tab label}"`, or the workspace label.
- Label when the tab has several panes: `"{tab} · {pane} · {i+1}"` or `"{pane} · {i+1}"`.
- `meta` = `foreground_cwd` or `cwd`.
- `detail` = `"{workspace} / {tab} / {pane_id}"`.

Search:
- All whitespace-separated words of the query must match one of these fields: workspace, branch, tab, label, cwd, agent kind, title, display agent, pane_id, endpoint label.

**Row look**
- Prefix: `(depth - is_pane) * 2 + 1` spaces, then the connector (`├─ ` or `└─ ` in overlay0), then `◆ ` on the current pane, then the status dot.
- The status dot always uses the dot style (●/○/·) in the status color.
- Selected row: contrast on accent, BOLD.
- Stale endpoint row: overlay0, DIM, bg surface0 when selected.
- Machine row: subtext0. Rows that have no status are bold.

Right-hand columns on pane rows:
- With row width ≥ 64: 24 columns, holding the agent kind (11 wide) and the status text, or `shell`.
- With row width ≥ 36: 12 columns, holding the agent kind only.

Other row parts:
- A workspace row shows its branch right-aligned.
- A machine row shows its status glyph and label right-aligned.
- With no matches: `" No matching agents or terminals"` in overlay0.

**Keys** (`overlay_input.rs:631-781`)

Search focused:
- The editor handles keys first. An edit clears the filter and the selection.
- Up / Ctrl+P and Down / Ctrl+N move the selection.
- Esc leaves search.
- Enter opens the selection.

Search not focused:

| Key | Action |
|---|---|
| j / k / ↑ / ↓ | move by one row |
| ← / → | first pane of the previous / next workspace section. Does not wrap. |
| Home | default row |
| End / G | last row |
| Ctrl+D / Ctrl+U | move ±8 |
| `/` | focus search |
| b / w / i / d | filter blocked / working / idle / done |
| a | show all |
| Backspace | clear the filter |
| Enter | open the selection |
| Esc | close |

Moving the selection clamps at the ends.

**Mouse** (`mouse.rs:1644-1716`)
- Hover selects a row.
- Click a row: select it and open it.
- Click the search row: focus search.
- Scrollbar: drag the thumb or jump.
- Wheel: moves the selection ±3.
- Click outside: close.

**Open** (`endpoint_navigation.rs:228-306`)
- Machine: activate it.
- Workspace: `WorkspaceFocus`.
- Pane: `PaneFocus`.
- An endpoint that is not ready shows the notice "{label} is not ready".
- The dialog closes only on success.

**Data:** endpoints (label, status). Workspaces (id, label, branch, `new_workspace_cwd`). Tabs (id, label, `custom_label`). Panes (id, label, cwd, `foreground_cwd`). Agents (agent, display_agent, name, title, `terminal_title_stripped`, status). The focused pane.

### 5.8 Context menus (`ContextMenu`)

**Opening** (`mouse.rs:1784-1867`)

A right-click opens a menu only when all of these hold:
- No overlay is open.
- `mouse_capture` is on.

The click is checked in this order:
1. **Pane passthrough.** The click goes to the pane app instead of opening a menu when it is inside a pane's inner rect, the app has mouse reporting on, and one of these is true:
   - the pane owns right-click and the click has no modifiers, or
   - the modifiers equal `ui.right_click_passthrough_modifier`.
2. **Workspace row** in the expanded sidebar, on the active endpoint: workspace menu.
3. **Tab:** tab menu.
4. **Pane outer rect, border included:** pane menu.

Nothing opens on agent rows, empty sidebar space, the collapsed sidebar, empty tab bar space, or the footer.

**Entries** (`context_menu.rs:4-83`). No separators, glyphs or shortcut hints.

| Target | Entries in order |
|---|---|
| Workspace, not git | `Rename`, `Close` |
| Workspace, git, not linked, no linked children | `Rename`, `Close`, `New worktree`, `Open worktree...` |
| Workspace, linked worktree child | `Rename`, `Close`, `Delete worktree checkout...` |
| Workspace, group parent | `Rename`, `Close group` (if `close_group`) or `Close`, `New worktree`, `Open worktree...`, `Collapse` / `Expand` |
| Tab | `New tab`, `Rename`, `Close` |
| Pane | `Rename pane`; `Clear pane name` (only with a manual label); `Swap with focused pane` (only if not focused); `Split right`; `Split down`; `Zoom`; `Use Herdr right-click menu` / `Send right-clicks to pane`; `Close pane` |

Flags (`context_menu.rs:87-168`):
- `is_git` = the workspace has a worktree or a branch.
- `close_group` comes from `workspace_close_is_group` (`sidebar.rs:573-601`).

**Actions** (`context_menu.rs:182-453`)
- Tab actions send `TabFocus` first.
- `Swap` sends `PaneSwap{source=focused, target=clicked}`, then `PaneFocus(source)`.
- `Split right` / `Split down` send `PaneSplit{focus: true}`.
- `Zoom` sends `PaneZoom{Toggle}`.
- The right-click toggle sends `PaneInputSet{right_click: Herdr|Pane}`.
- Rename, Close and worktree entries open the dialogs in 5.4, 5.5 and 5.10.
- `Collapse` / `Expand` toggles the sidebar group and saves the preference.

**Look** (`overlays.rs:165-219`)
- Width: max label width + 4, at least 14. Height: number of items + 2.
- Placed at the click point, clamped to the screen.
- Box: `panel(accent, panel_bg)`.
- Labels start at `inner.x` with **no leading space**.
- Highlighted row: contrast on accent, BOLD. Other rows: text on panel_bg.
- The first row is highlighted on open.

**Keys** (`overlay_input.rs:602-626`)
- Esc closes.
- Up / Down move the highlight. It clamps at the ends. j and k do nothing.
- Enter activates the highlighted entry.

**Mouse** (`mouse.rs:1417-1444`)
- Hovering a row highlights it.
- Clicking a row activates it.
- A click anywhere else closes the menu and consumes the click.
- A right-click while the menu is open is swallowed.

### 5.9 Global menu (`GlobalMenu`)

**Opening.** Only from the `menu` launcher in the sidebar footer (2.4). There is no keybinding (`mouse.rs:1404, 2045`).

**Entries** (`global_menu.rs:22-54`), in order:
1. `settings`
2. `keybinds`
3. `reload config`
4. `update ready` if an update is available, else `what's new` if release notes are available. Otherwise this entry is absent.
5. `detach`

A badge `" ●"` shows on `update ready`, and on `settings` when `integration_updates_available` is true.

**Actions** (`global_menu.rs:80-109`, `actions.rs:10-82`)

| Entry | Action |
|---|---|
| settings | open Settings |
| keybinds | open Help |
| reload config | `ServerReloadConfig` plus a local config reload |
| update ready / what's new | open ReleaseNotes. Does nothing without notes; the menu stays open. |
| detach | detach the client |

**Look** (`overlays.rs:86-163`)
- Width: max label width (+2 if badged) + 4. Height: number of items + 2.
- Placed right-aligned with the launcher, directly above it.
- Rows are `" {label}"`. A badged row reads `" ● label"`, with the dot in accent BOLD.
- Highlighted row: contrast on accent, BOLD.
- The background is not dimmed.

**Keys** (`overlay_input.rs:572-596`)
- Esc closes.
- Up/k and Down/j move the highlight. It clamps at the ends.
- Enter activates.

**Mouse** (`mouse.rs:1386-1416`)
- Hover highlights a row.
- Clicking a row activates it.
- Clicking the launcher closes the menu.
- A click anywhere else closes the menu and is consumed.

### 5.10 Worktree dialogs

**Triggers** (`worktrees.rs:167-229`; keys `config/model.rs:1127-1129`)
- Context menu entries.
- `new_worktree` (`prefix+shift+g`). `open_worktree` and `remove_worktree` are unbound by default.

**Guards.** These go to the ERROR bar:
- "New and open worktree actions start from the repo parent workspace."
- "This workspace is not a Herdr-managed worktree checkout."

**Preflight.** `WorktreeList{workspace_id}` is sent first. Each reply entry has `{path, branch, label, is_bare, is_detached, is_prunable, is_linked_worktree, open_workspace_id}` (`api/schema/worktrees.rs:61-81`).

#### WorktreeCreate (`worktree_overlays.rs:3-107`)

Size: `popup(68, 12)`, border accent.

| Row | Content |
|---|---|
| 0 | `new worktree` in text BOLD, no leading space |
| 2 | `" branch"` in overlay0 |
| 3 | input, text on surface0 |
| 5 | `" checkout"` in overlay0 |
| 6 | `" {checkout path}"` in subtext0 |
| 8 | `" creating…"` in accent, or `" {error}"` in red |
| 9 | buttons `" ↵ create and open "` (20 wide, contrast on accent, BOLD) and `" esc cancel "` (12 wide, text on surface0) |

**Branch seed:** `worktree/{adj}-{noun}-{hex4}`, replaced on first keystroke (`src/worktree.rs:21-32`).
- Adjectives: brave, calm, clear, green, lucky, quiet, rapid, silver.
- Nouns: river, cloud, field, forest, harbor, meadow, stone, valley.

**Checkout path:** `{worktrees dir}/{repo}/{slug(branch)}`.
- The slug is lowercase alnum with other runs turned into `-`, or `worktree` when empty (`worktree.rs:34-54`).
- The default directory is `~/.herdr/worktrees`.

**Submit:**
- An empty branch gives the error `branch is required`.
- Otherwise it sends `WorktreeCreate{branch, base: "HEAD"}`. On success it closes and sends `TabFocus(new tab)`.

**Keys:** editor keys, Esc (blocked while busy), Enter.

#### WorktreeOpen (`worktree_overlays.rs:109-306`)

**Entries.** Bare and prunable checkouts are removed. An empty list gives the ERROR "No Git worktrees found for this repo."

**Status text** (`state.rs:446-470`):
- `open` when the checkout already has a workspace.
- empty when it has a branch.
- `detached` when it is a detached linked checkout.
- `root` otherwise.

**Filter:** substring match on `"{label} {branch} {path} {status}"`.

Size: height `clamp(2n + 7, 12, 26)`, `popup(96, h)`, border accent.

| Row | Content |
|---|---|
| 0 | `open worktree` in text BOLD |
| 1 | `" / "` + editor when focused. Unfocused: `" / {query}"`, or the placeholder `" / filter worktrees"`. Right-aligned: `"{n} checkouts"` or `"{shown}/{n} checkouts"`. |
| 2 | `─` rule in surface1 |
| body | 2 lines per entry: `" {label}"` BOLD with the status right-aligned, then `" {path}"` in overlay0. Selected entry: contrast on accent. Empty: `" no matching worktrees"`. |
| h-3 | `" opening…"` or `" {error}"` |
| last | buttons `" ↵ open "` (10 wide) and `" esc cancel "` |

**Keys:**
- When the search is focused, the editor handles keys first.
- Esc closes.
- Enter opens.
- Up / Down / Ctrl+N / Ctrl+P move.
- `/` focuses the search.

**Submit:** sends `WorktreeOpen{path, focus: true}`.

#### WorktreeRemove (`worktree_overlays.rs:308-418`)

Size: `popup(72, 10)`, border **red**.

| Row | Content |
|---|---|
| 0 | `" delete worktree checkout?"` in red BOLD |
| 1 | `" This removes the checkout folder:"` |
| 2 | `" {path}"` in subtext0 |
| 3 | `" The branch is not deleted. The Herdr workspace will close."` |
| 4 | force mode only: `" Dirty or untracked files will be permanently deleted."` in red |
| 5 | `" removing…"` or `" {error}"` |
| 7 | buttons `" ↵ remove "` or `" ↵ delete anyway "` (18 wide, contrast on red, BOLD), and `" esc cancel "` |

**Force flow** (`worktrees.rs:341-366, 499-510`):
1. The first submit sends `force: false`.
2. If the reply is `dirty_worktree_requires_force`, or a "not a working tree" failure, the dialog switches to force mode.
3. The next Enter sends `force: true`.

#### Worktree mouse (`mouse.rs:1445-1521`)

- The cancel button closes the dialog, unless it is busy.
- Clicking the search row focuses the search.
- Clicking an entry row selects it and opens it at once.
- Clicking the primary button submits.
- In the Open dialog, the wheel moves the selection.
- A click outside the dialog does **nothing**.

#### Error notices

Every endpoint error also raises an `Action rejected` endpoint notice. The exceptions are `confirmation_required`, `stale_content` and `stale_target` (`actions.rs:527-567`).

### 5.11 Settings (`Settings`)

**Open**
- The `settings` binding (`prefix+s`), the global menu `settings` entry, or completing onboarding (which opens the Integrations tab).
- The initial tab is Theme, with the current theme selected (`settings.rs:35-46`).

**Layout** (`settings_overlay.rs:36-255`)
- Size: `popup(76, 22)`. On the Integrations tab the height is `max(22, 14 + max(1, n) + min(6, messages))`. Border accent. Below 20×8 inner, nothing is drawn.

| Row | Content |
|---|---|
| 0 | `" settings"` in text BOLD |
| 1 | tabs `" theme "`, `" indicators "`, `" sound "`, `" toasts "`, `" integrations "`, 1-cell gaps. With an update the last tab reads `" ● integrations "`. Active tab: contrast on accent, BOLD. Inactive: overlay1. An inactive tab's `●` is accent. |
| 2 | `─` rule in surface0 |
| 4 … | content |
| bottom-2 | `" ↑↓ select  tab section"` in overlay1 |
| bottom | buttons `" ↵ apply "` (10 wide) and `" esc close "` (12 wide, text on surface0, BOLD) |

On the Integrations tab, the apply button reads `" ↵ install "`. It shows only when some integration needs installing. The text is 11 cells wide in a 10-cell button, so it renders clipped.

**Choice rows** (`settings_overlay.rs:3-34`)
- Format: `" {▸ or space} {label}{ ✓}"`.
- `✓` shows only on the Theme tab, next to the theme that was active when Settings opened.
- Selected row: contrast on accent, BOLD.
- Choice tabs show a title at row y, a description at y+1, and choices from y+3. When there are more than 2 choices, they have a blank row between them.

**Tabs**
- **theme**: the 18 theme names in `THEME_NAMES` order (7.2). The list scrolls. Moving the selection previews the palette live (`settings.rs:119-152`).
- **indicators**: title `agent status indicators`, description `choose color dots or distinct symbols for each state`. Choices: `color dots  ● ● ● ○ ·` and `distinct symbols  × ◐ ✓ ○ ·`.
- **sound**: title `sound alerts`, description `play sounds when agents change state in background`. Choices: `on`, `off`.
- **toasts**: title `notification popups`, description `choose where background popup notifications should appear`. Choices: `off`, `inside herdr`, `via terminal`, `via system`.
- **integrations**: title `agent integrations`, description `enable session restore and, where supported, direct status updates`.
  - While loading: `" loading integrations…"`. With no targets: `" no integration targets available"`.
  - Row format: marker, then the label padded to 9 at x+3, then the status at x+14.

    | Marker | Status text |
    |---|---|
    | `✓` (green) | `installed` |
    | `↻` (yellow) | `update available` |
    | `+` (accent) | `available` |
    | `–` (overlay0) | `not found` |

  - Up to 6 install messages show below the rows, or `" installing…"`.
  - These rows cannot be clicked.

**Apply** (`settings.rs:162-348`; written keys at `src/config/write.rs:19-45`)
- Theme: writes `[theme] name` and `auto_switch = false`, then closes Settings.
- Indicators: writes `ui.status_indicators`.
- Sound: writes `ui.sound.enabled`.
- Toasts: writes `ui.toast.delivery`.
- Integrations: sends `IntegrationInstall` for each target that needs it, then lists them again.
- After a write: the config is reloaded and `ServerReloadConfig` is sent.
- A write failure shows on the ERROR bar.

**Cancel**
- Restores the original theme.
- Blocked while an install is running.

**Keys** (`settings.rs:350-399`)

| Key | Action |
|---|---|
| Esc | cancel |
| Tab, Right, `l` | next tab (wraps) |
| BackTab, Left, `h` | previous tab (wraps) |
| Up/k, Down/j | move the selection |
| Enter, Space | apply |

**Mouse** (`mouse.rs:1522-1572`)
- Left Down only. There is no hover effect.
- Clicking a tab selects it.
- Clicking a choice selects it. On indicators, sound and toasts, the click also applies it at once.
- Clicking the primary button applies.
- The close button, or a click outside, cancels.

**Data:** theme name, palette, `status_indicators`, sound enabled, toast delivery, `integration_updates_available`, `IntegrationList` (`{target, label, command, available, state}`).

### 5.12 Agent notification toasts (`visible_notification`)

**Kinds and text** (server: `src/server/headless/notifications.rs:70-290`, `src/app/actions.rs:149-164`)

| Kind | Title | Sound |
|---|---|---|
| NeedsAttention | `"{agent} needs attention"` | Request |
| Finished | `"{agent} finished"` | Done |
| UpdateInstalled | `"Herdr v{version} available"` | — |
| Custom (from `notification.show`) | title, at most 80 chars | optional |

- NeedsAttention and Finished body: `"{workspace label} · {ws number}"`, plus `" · {tab label}"` when the workspace has more than 1 tab.
- UpdateInstalled body: the install instruction.
- Custom body: at most 240 chars. It can carry a sound and a position.

**Policy** (`src/client/shell/notification_policy.rs`, details in 8.3)
- `ui.toast.delivery`: `off` (default in code, `config/model.rs:1249-1257`), `herdr`, `terminal`, or `system`.
- Delay: `ui.toast.delay_seconds`, default 1. Custom toasts skip the delay.
- An in-app toast is not shown when its target is the focused tab.

**Duration**

| Kind | Time |
|---|---|
| NeedsAttention | 8 s |
| Finished | 5 s |
| UpdateInstalled | 3 s |
| Custom | 5 s |

- One toast shows at a time. Up to 8 more wait in a FIFO queue.
- A new event for the same pane replaces the old one.

**Desktop card** (`src/client/shell/notifications.rs:107-206`)
- Width: `max(title, body) + 6`. Height: 4, or 3 with no body. The full screen is the bounding area.
- Position: `event.position`, else `ui.toast.herdr.position`. Values: `top-left`, `top-right`, `bottom-left`, `bottom-right` (default).
- Vertical offset: +1 for the config diagnostic, +1 for the lifecycle banner.
- Box: `Borders::ALL` in overlay0, bg panel_bg.
- Line 1: a dot, then the title in text BOLD.
- Line 2: the body at x+2, in overlay0.
- Dot color: red (NeedsAttention), blue (Finished), accent (other kinds).

**Mobile banner** (`notifications.rs:10-105`)
- One row on surface0, at the bottom (bottom-2 when banners are showing).
- Text: `" ● {title}"`, then `" · {body}"`.
- Titles are shortened: `X waiting`, `X done`, `update ready`.

**Click** (`mouse.rs:1008-1019`, `notification_policy.rs:65-97`)
- Left Down focuses the target pane or activates its endpoint. Works only with no overlay open and in Terminal mode.
- Key `open_notification_target` (`prefix+o`) does the same.

### 5.13 Endpoint notices, lifecycle banner, machine diagnostics (`src/client/shell/endpoint_notices.rs`, `machine_diagnostics.rs`)

**Notice kinds** (`state.rs:687-692`)

| Kind | Dot | Duration |
|---|---|---|
| Unsupported | red | 8 s |
| Rejected | red | 3 s |
| Timeout | yellow | 8 s |
| Unavailable | yellow | 8 s |

- A notice shows once per (boot, kind, code). A Rejected notice is shown again unless it is already on screen (`actions.rs:313-353`).
- An endpoint notice takes priority over an agent toast.

**Titles and bodies** (`actions.rs:374-564`, `input.rs:801-806`)

| Title | Body |
|---|---|
| `Action unavailable` | "This server does not support {method} yet. Update and restart it to enable this action." |
| `Server timed out` | "This server did not respond to {method}." |
| `Action interrupted` | server message |
| `Server unavailable` | server message |
| `Action rejected` | server message |
| `Endpoint unavailable` | "{label} is not ready" or "{label} is unavailable" |
| `Paste rejected` | server message |
| `Confirm workspace first` | "Select an available workspace and press Enter before using workspace or pane actions" |

**Look**
- Desktop: the same card as 5.12, always top-right.
- Mobile: a bottom banner with a red dot.
- Left Down on the notice dismisses it (`mouse.rs:1000-1007`).

**Machine diagnostic** (`machine_diagnostics.rs:54-129`)
- A click on an SSH machine's error badge opens a 15 s notice.
- Title: `"{label}: herdr machine reconnect {profile}"` for an auth error, or `… status …` for other errors. Body: the error text.
- Hovering the badge draws it reversed.

**Lifecycle banner** (`endpoint_notices.rs:22-54`)
- Shows when the active machine is not Online.
- Text: `"{symbol} {label} · {state}"`, right-aligned at row `top_offset`, on surface0, fg = the status color.

### 5.14 ERROR bar

This is a mode bar variant (1.3, `render.rs:79-83`).
- Lasts 5 s (`state.rs:6, 1858-1863`).
- Hidden while an overlay is open.
- Any input clears it (`input.rs:225-228`).

### 5.15 Navigate mode surface (desktop)

There is no dialog for this mode.
- Mode bar: the NAVIGATE bar (1.3).
- Sidebar: the workspace cursor row uses bg `selection_bg` (2.4). The sidebar scrolls to keep the target visible.
- Keys (help group `navigation`):

| Key | Action |
|---|---|
| ↑/↓ | move the workspace cursor |
| h/j/k/l, ←/→ | move pane focus |
| Tab / Shift+Tab | cycle panes |
| Enter | open the selected workspace |
| 1..9 | switch to that workspace |
| Esc | go back |

- Prefix actions work without the prefix (`input.rs:897-929`).
- Entry: `workspace_picker` (`prefix+w`) (`actions.rs:151-160`). Navigate is also the mode when there are no workspaces.
- Left Down while the remote preview is blocked returns to Terminal or Copy (`mouse.rs:686-695`).

### 5.16 Resize mode surface

- There is no overlay. Only the RESIZE mode bar shows (mauve badge).
- Keys: `h/l` change the width, `j/k` change the height, `esc` exits.
- Entry: `prefix+r`.
- Mouse border drag works in any mode (section 6).

### 5.17 Mobile layout (`src/client/shell/mobile.rs`)

Active when `cols <= mobile_width_threshold`, default 64.

**Header** (`mobile.rs:50-361`)
- 2 rows on panel_bg.
- Right side: a 10-column `switch` button on surface0, with a left `│` in surface_dim. If any agent is Blocked, a red blocked glyph shows in its top-right cell.
- Row 1:
  - `" {icon} "` followed by the focused workspace label in text BOLD. With no workspace: `" no workspace"`.
  - Right-aligned in overlay1: `"tab {label}"`, plus `" · {i}/{n}"` when there are several tabs.
- Row 2, agent summary:
  - `" no agents"` or `" all idle"`, or counts in the order blocked, done, working, idle, joined by `" · "`.
  - Glyphs: `◉` blocked and `●` done in dots mode. Otherwise each state uses its own symbol.
  - The first segment uses the status color and BOLD; done uses **blue**.

**Switcher** (Navigate mode, full screen, `mobile.rs:363-941`)
- Top: a `" switch"` title, a `close ×` button, and a rule.
- Sections, each headed in overlay1 BOLD UNDERLINED:

| Section | Contents |
|---|---|
| `machines` | Shown only when there are more than 1. |
| `agents` (or `agents · {view}`) | Two-line rows. |
| `spaces` | A `+ new workspace` row, then the workspace rows. |
| `tabs` | A `+ new tab` row, then the tab rows. |
| `menu` | The global menu items, with no badges. |

- Scrollbar: on the left, track `│`, thumb `▌` in accent.
- Wheel: scrolls 2 rows.
- A tap focuses the target and returns to Terminal mode (`mobile.rs:944-1089`).

**Behavior**
- Toasts are hidden while the switcher is open.
- There are no workspace or tab context menus. Pane menus still work.

## 6. Mouse

### 6.1 Plumbing and config

**Host capture**
- The client uses crossterm `EnableMouseCapture`. It also sends `\x1b[?1016h` when SGR pixel mouse is on (`src/client/terminal_setup.rs:615-650`).
- The server decides capture per client with `shell_surface_active && (mouse_capture || focused pane app has mouse reporting)` (`src/server/headless/render.rs:68-124`). So even with `mouse_capture = false`, the host captures the mouse while the focused app uses the mouse.

**Config** (`src/config/model.rs:951-962`)

| Key | Default |
|---|---|
| `ui.mouse_capture` | true |
| `ui.copy_on_select` | true |
| `ui.right_click_passthrough_modifier` | off (ctrl, alt, cmd/super, meta, hyper, combos; shift is rejected) |
| `ui.mouse_scroll_lines` | 3 |
| `ui.sidebar_min_width` / `ui.sidebar_max_width` | 18 / 36 |

**Forwarding to panes** (`mouse.rs:2338-2397`, `src/input/mouse.rs:43-129`)
- The position is relative to the pane inner rect.
- The event uses pixel coordinates when the pane wants SGR pixels and the host reported pixels.
- Every event carries `lines = mouse_scroll_lines`.

**Server-side encoding** (`src/server/pane_input.rs:154-300`)
- Down, Up and Drag are encoded as button events.
- Moved is sent only in any-motion mode (1003).
- Without app reporting, events are dropped.

**Wheel routing** (`src/pane/terminal.rs:1979-2000`)
- If the app has mouse reporting (1000/1002/1003/X10): one report per notch, and the view resets to the bottom.
- Else on the alternate screen with mode 1007: one arrow key per notch.
- Else: scroll back by `lines` rows per notch.
- There is no acceleration.

**Effects of `mouse_capture = false`**
- Chrome hit regions are cleared (`render.rs:324-338`).
- No `new`, `menu`, `+` or tab scroll arrows are drawn.
- No right-click menus, Ctrl-click links or link hover.
- OSC 8 links stay in the host frame. With capture on, they are removed (`src/client/state.rs:568-572`).
- Still active: sidebar toggle, tab wheel, panes, popup, toasts, mobile.

**Any input clears the ERROR bar** (`input.rs:225-228`).

### 6.2 Dispatch order (`mouse.rs:683-2336`)

1. Machine badge.
2. Link hover update.
3. Navigate mode with blocked preview: a Left Down returns to Terminal/Copy mode and continues.
4. Onboarding, ProductAnnouncement, ReleaseNotes. These are fully modal.
5. Consumption of URL clicks and pending link activation.
6. An active pane gesture. Drag and Up go to the gesture pane wherever the pointer is.
7. Popup pending: swallow all.
8. Popup present: forward inside it, swallow outside it. There is no click to dismiss.
9. Ctrl+Left: link.
10. Endpoint notice click: dismiss.
11. Notification click: focus.
12. Mobile.
13. Chrome drag motion and press thresholds.
14. Left Up: finish the drag, or focus the pressed workspace or tab.
15. Overlays: GlobalMenu, ContextMenu, Worktree, Settings, Help, Navigator, Rename/ConfirmClose.
16. Selection drag, release and wheel.
17. Main match.

### 6.3 Per-region behavior

"—" means no Herdr action.

| Region (hit) | Left click | Double | Right | Middle | Drag | Wheel | Hover |
|---|---|---|---|---|---|---|---|
| Expanded workspace row (`workspaces`) | Down records a press. **Up focuses** (`mouse.rs:1372-1375, 2101-2115`). | — | Workspace menu (expanded, active endpoint) | — | Reorder after 1 cell, if draggable (active endpoint, not a linked child) and a drop target exists (`mouse.rs:1220-1239`) | ±1 entry | — |
| Group toggle ▾/▸ | Down toggles the group and saves it (`mouse.rs:2091-2100`) | toggles again | row menu | — | — | body scroll | — |
| Footer `new` / `menu` | New workspace / toggle global menu (`mouse.rs:2044-2057`) | — | — | — | — | — | — |
| Sidebar list scrollbars | Thumb: drag with grab offset. Track: jump to center (`mouse.rs:1977-2024`) | — | — | — | drag | ±1 | — |
| Section divider | Down starts the drag and sets the ratio (`mouse.rs:1972-1976`) | — | — | — | ratio = `(row - y)/height`, clamped 0.1–0.9. Up saves (`mouse.rs:24-36, 1360-1362`) | — | — |
| Agent sort toggle | Toggles spaces/priority, resets scroll, saves | — | — | — | — | — | — |
| Agent row | Down sends PaneFocus (`mouse.rs:2144-2161`) | — | — | — | — | ±1 | — |
| Sidebar edge (`sidebar_divider`) | Down starts the width drag: width = `col + 1`, clamped (`mouse.rs:8-22, 1953-1971`) | Two presses within **350 ms** reset to the config width | — | — | live resize. Up saves | — | — |
| Sidebar toggle `«` / `»` | Toggles collapsed and saves. Checked before the agent scrollbar (`mouse.rs:1943-1952`) | toggles again | — | — | — | — | — |
| Collapsed compact rows | Workspace: Up focuses. Agent: focus. | — | no menu | — | no reorder | none | — |
| Machine row | Arrow toggles collapse. Row activates the machine, or shows "not ready" (`endpoint_navigation.rs:45-79`) | — | — | — | — | — | — |
| Machine error badge | Down shows a 15 s diagnostic notice | — | — | — | — | — | Reversed on hover |
| Tab | Down records a press (capture only). **Up sends TabFocus** (`mouse.rs:1376-1384, 2116-2143`) | — | Tab menu | — | Reorder after 1 cell while on the tab row | Up: previous tab. Down: next tab (`mouse.rs:1868-1897`) | — |
| Tab arrows `<` `>` | Scroll by ±1 (`mouse.rs:2065-2090`) | — | — | — | Drop target 0 / end | prev/next tab | — |
| New tab `+` | New tab (`mouse.rs:2058-2064`) | — | — | — | — | prev/next tab | — |
| Pane inner, app not reporting | Clears selection, sets the anchor, PaneFocus (`mouse.rs:2243-2278`) | Word select (6.4) | Pane menu | — (not forwarded) | Text selection with autoscroll | Focus if unfocused, then forward. The server scrolls back or sends arrows (`mouse.rs:2312-2333`). | Ctrl+move: link hover |
| Pane inner, app reporting | Forward Down, start a gesture, PaneFocus (`mouse.rs:2234-2242`) | forwarded | Forwarded only with passthrough (own right-click without modifiers, or the configured modifier, which is stripped). Else Herdr menu. | Forward and start a gesture (no focus) | Forwarded through the gesture | Focus, then forward | Moved is forwarded |
| Pane border | PaneFocus | — | Pane menu always | — | — | — | — |
| Pane scrollbar | Mode becomes Terminal, PaneFocus. Thumb: drag. Track: jump (`mouse.rs:2162-2199`) | — | — | — | Offset sent when it changes and ≥33 ms since the last send. Final send on Up (`mouse.rs:1106-1145, 1310-1332`). | Not routed (unclear: depends on whether the bar is inside the inner rect) | — |
| Pane split (`pane_splits.hit_rect`: the border cell(s), or the gap cell; none when borders and gaps are both off) | Down starts the drag, `grab_offset = pos - pointer`. Focus does not change (`mouse.rs:2200-2226`). | — | — | — | ratio = `(pointer + grab - origin)/length`, clamped 0.1–0.9, ≥33 ms throttle. Cancelled if the topology changes. Final send on Up (`mouse.rs:429-451, 1146-1194, 1333-1359`). | — | — |
| Popup pane | All buttons are forwarded with modifiers, no focus request (`mouse.rs:916-944`) | fwd | fwd | fwd | gesture | fwd | Moved forwarded. **Outside: all swallowed.** |
| Notification toast | Endpoint notice: dismiss. Agent toast: focus its pane and dismiss | — | — | — | — | — | — |
| Copy feedback toast | not a hit | | | | | | |
| Mobile header `switch` | Opens the switcher (Navigate mode) | | | | | | |
| Mobile switcher | Close, or tap a target | — | swallowed | swallowed | swallowed | ±2 rows | — |

Overlay mouse rules are listed in each overlay's subsection of section 5. The overlay summary below gives the dismiss behavior:

| Overlay | Click outside closes | Wheel |
|---|---|---|
| GlobalMenu, ContextMenu | yes (click is consumed) | swallowed |
| Settings, Help, Navigator | yes | Help: scroll ±3. Navigator: selection ±3. |
| Rename, ConfirmClose | yes. Any click that is not on primary or clear closes, even inside the dialog. | swallowed |
| Worktree*, ProductAnnouncement, ReleaseNotes, Onboarding | no | worktree open: ±1. Notes: ±3. |
| Popup pane | no | forwarded |

### 6.4 Text selection (`src/selection.rs`, `mouse.rs:206-427, 1741-1782`, `word_selection.rs`)

**Selection model**
- Phases: Anchored, Dragging, Done. Only Dragging and Done are drawn.
- Rows are stored as absolute buffer rows.

**Start**
- A Left Down in a pane whose app is not reporting starts a selection.
- It first clears any old selection, autoscroll, highlight timer and word gesture.

**Double-click** (`state.rs:778-783`, `mouse.rs:2250-2264`)
- Same pane, within **350 ms**, at most 1 cell away in row and column, no modifiers.
- The client reads the row from the server (`PaneSelectionRead`).
- Word bounds (`src/app/actions.rs:1086-1334`) are tried in this order:
  1. An http(s) URL, with trailing punctuation and unbalanced closers trimmed.
  2. A quoted span that contains `/`.
  3. A token split on whitespace and `|()[]{},;!` and fullwidth `（）：、。，`, with brackets and quotes trimmed.
- Holding the button and dragging extends the selection by whole words.

**Triple-click.** There is none. A fast third click is a plain click.

**Drag**
- The drag becomes active once the pointer leaves the anchor cell. The pointer is clamped to the pane.
- Repaints are throttled to **16 ms**.

**Autoscroll**
- When the pointer is outside the inner rect, the view jumps right away by `clamp(dist*3, 3, 15)` lines.
- When the pointer is on the top or bottom row, autoscroll arms without a jump.
- After that the view scrolls 1 line every **30 ms**.

**Wheel during a selection.** It scrolls the selected pane by `mouse_scroll_lines` and extends the selection.

**Release**

| Gesture | `copy_on_select` on | `copy_on_select` off |
|---|---|---|
| Plain click | No selection. | No selection. |
| Drag | Copy, clear the selection, show the "copied to clipboard" toast for 2 s. Copy goes to the native clipboard or OSC 52 (`selection.rs:357-375`). | The selection stays until Ctrl+C or Super+C (`input.rs:646-660`). |
| Word selection | Copy. The highlight stays for 500 ms if there was no drag. | Same as drag. |

**The selection is cleared by:**
- the next Left Down;
- any key, text or paste outside Copy mode;
- a focus change;
- a pane resize or an alternate-screen switch;
- a content change, for word selections only;
- a popup change (`state.rs:1442-1469, 1690-1743`).

### 6.5 Drag reorder

**Threshold.** A drag starts when the pointer moves at least 1 cell (Chebyshev distance). A press with no valid drop target stays a click.

**Workspace drop** (`mouse.rs:511-677`)
- The slots are above each top-level row of the active endpoint, plus one after the last row.
- The nearest slot wins.
- Indicator: an accent `─` line.
- On drop, the client sends `WorkspaceMove{insert_index}`, or `WorkspaceMoveBlock` for a worktree group.

**Tab drop** (`mouse.rs:453-509, 1263-1295`)
- Uses the tab midpoints. The arrows count as targets for index 0 and the end.
- Indicator: an accent `│`.
- Off the tab row there is no target.
- On drop, the client sends `TabMove`.

### 6.6 Links

**Click** (`mouse.rs:948-999`, `actions.rs:664-726`)
- Ctrl+Left Down in a pane, with capture on and in Terminal mode.
- The client sends `PaneLinkActivate`. The following mouse events are buffered.
- If the result is `handled`, the click is consumed.
- If the result is a safe URL, the client opens it.
- Otherwise, the buffered events replay as a normal click.

**Hover:** see 4.9. Ctrl is the modifier on every platform.

### 6.7 Focus effects

| Event | Effect |
|---|---|
| Left Down in a pane or on its border | PaneFocus |
| Pane scrollbar press | PaneFocus and Terminal mode |
| Right-click passthrough | PaneFocus |
| Wheel over an unfocused pane | PaneFocus, then forward |
| Middle click, Moved, menu right-click, split press | No focus change |
| Workspace or tab click | Focus that target on Up |
| Agent click | Focus that target on Down |
| Outer terminal loses focus | A synthetic Up ends the active pane gesture (`input.rs:469-502`) |

### 6.8 Timing constants

| Constant | Value |
|---|---|
| Pane double-click | 350 ms, ±1 cell |
| Sidebar edge double-click | 350 ms |
| Autoscroll tick | 30 ms, 1 line |
| Drag repaint throttle | 16 ms |
| Split and scrollbar send throttle | 33 ms |
| Word highlight hold | 500 ms |
| Copy toast | 2 s |
| Diagnostic notice | 15 s |
| Drag threshold | 1 cell |

Wheel step per region:
- Pane: 3 lines.
- Sidebar lists: 1 entry.
- Help, Navigator and notes: 3.
- Mobile: 2.
- Tab bar: 1 tab.

## 7. Themes and color roles

### 7.1 Config (`src/config/theme.rs`, `src/app/mod.rs:237-373`)

**Keys**
- `theme.name`, default `catppuccin`.
- `theme.auto_switch`, default false.
- `theme.dark_name` and `theme.light_name`. When omitted, each defaults to the sibling of `name` (see below).
- `[theme.custom]`: per-role overrides.
- `[theme.custom.light]` and `[theme.custom.dark]`: per-role overrides for one appearance.
- Legacy `ui.accent`, default `cyan`. It applies only when its value is not `cyan` and `custom.accent` is unset.

**Siblings**

| Dark | Light |
|---|---|
| catppuccin | catppuccin-latte |
| tokyo-night | tokyo-night-day |
| gruvbox | gruvbox-light |
| one-dark | one-light |
| solarized | solarized-light |
| kanagawa | kanagawa-lotus |
| rose-pine | rose-pine-dawn |

Dracula, nord, vesper and terminal map to themselves.

**Name aliases** (`theme.rs:25-47`): `catppuccin-mocha`, `latte`, `light`, `tokyonight`, `tokyo-day`, `tokyonight-day`, `gruvbox-dark`, `onedark`, `onelight`, `solarized-dark`, `lotus`, `rosepine`, `rosepine-dawn`, `dawn`.

**Resolution order** (`mod.rs:292-318`)
1. The built-in theme.
2. `custom`.
3. The legacy accent.
4. The mode-specific overrides, only when `auto_switch` is on.

**Color syntax** (`theme.rs:155-219`)
- `reset`, `default`, `none` and `transparent` all mean `Reset`.
- Hex: `#rrggbb` or `#rgb`.
- `rgb(r,g,b)`.
- ANSI names.
- Any other value becomes `Cyan`, with a warning.

**Unknown theme name**
- A diagnostic is shown.
- The fallback is catppuccin, or catppuccin-latte for `light_name`.

### 7.2 Built-in themes (`THEME_NAMES`, `theme.rs:4-23`; values `src/app/state.rs:78-525`)

In every theme `sidebar_bg` is `Reset`. Column abbreviations: acc = accent, panel = panel_bg, row = active_row_bg, sel = selection_bg, s0 = surface0, s1 = surface1, sdim = surface_dim, o0 = overlay0, o1 = overlay1, sub0 = subtext0.

| theme | acc | panel | row | sel | s0 | s1 | sdim | o0 | o1 | text | sub0 | mauve | green | yellow | red | blue | teal | peach |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| catppuccin | #89b4fa | #181825 | #1e1e2e | #313244 | #313244 | #45475a | #1e1e2e | #6c7086 | #7f849c | #cdd6f4 | #a6adc8 | #cba6f7 | #a6e3a1 | #f9e2af | #f38ba8 | #89b4fa | #94e2d5 | #fab387 |
| catppuccin-latte | #1e66f5 | #eff1f5 | #e6e9ef | #bdd0f5 | #ccd0da | #bcc0cc | #e6e9ef | #9ca0b0 | #8c8fa1 | #4c4f69 | #6c6f85 | #8839ef | #40a02b | #df8e1d | #d20f39 | #1e66f5 | #179299 | #fe640b |
| terminal | Blue | Reset | DarkGray | Reset | Reset | DarkGray | DarkGray | Gray | White | Reset | Gray | Gray | Green | Yellow | LightRed | Blue | Cyan | Yellow |
| tokyo-night | #7aa2f7 | #1a1b26 | #232636 | #2d3650 | #24283b | #414868 | #1a1b26 | #565f89 | #697196 | #c0caf5 | #a9b1d6 | #bb9af7 | #9ece6a | #e0af68 | #f7768e | #7aa2f7 | #7dcfff | #ff9e64 |
| tokyo-night-day | #2e7de9 | #e1e2e7 | #d2d3da | #b6cae7 | #c4c8da | #a8aecb | #d2d3da | #8990b3 | #68709a | #3760bf | #6172b0 | #7847bd | #587539 | #8c6c3e | #f52a65 | #2e7de9 | #118c74 | #b15c00 |
| dracula | #bd93f9 | #282a36 | #373c52 | #463f5d | #44475a | #6272a4 | #282a36 | #6272a4 | #828cb4 | #f8f8f2 | #d2d2dc | #ff79c6 | #50fa7b | #f1fa8c | #ff5555 | #8be9fd | #8be9fd | #ffb86c |
| nord | #88c0d0 | #2e3440 | #434c5e | #40505d | #3b4252 | #434c5e | #2e3440 | #4c566a | #646e82 | #eceff4 | #d8dee9 | #b48ead | #a3be8c | #ebcb8b | #bf616a | #81a1c1 | #8fbcbb | #d08770 |
| gruvbox | #d79921 | #282828 | #323130 | #4b3f27 | #3c3836 | #504945 | #282828 | #928374 | #a89984 | #ebdbb2 | #d5c4a1 | #d3869b | #b8bb26 | #fabd2f | #fb4934 | #83a598 | #8ec07c | #fe8019 |
| gruvbox-light | #076678 | #fbf1c7 | #f2e5bc | #ebdbb2 | #ebdbb2 | #d5c4a1 | #f2e5bc | #928374 | #7c6f64 | #3c3836 | #504945 | #8f3f71 | #79740e | #b57614 | #9d0006 | #076678 | #427b58 | #af3a03 |
| one-dark | #61afef | #282c34 | #313640 | #334659 | #2c313a | #3e4451 | #282c34 | #5c6370 | #737a87 | #abb2bf | #969ca8 | #c678dd | #98c379 | #e5c07b | #e06c75 | #61afef | #56b6c2 | #d19a66 |
| one-light | #4078f2 | #fafafa | #d8dbe2 | #cddbf8 | #f0f0f1 | #e5e5e6 | #f5f5f6 | #a0a1a7 | #686b77 | #383a42 | #686b77 | #a626a4 | #50a14f | #c18401 | #e45649 | #4078f2 | #0184bc | #986801 |
| solarized | #268bd2 | #002b36 | #164b57 | #083e55 | #073642 | #586e75 | #002b36 | #586e75 | #657b83 | #93a1a1 | #839496 | #d33682 | #859900 | #b58900 | #dc322f | #268bd2 | #2aa198 | #cb4b16 |
| solarized-light | #268bd2 | #fdf6e3 | #eee8d5 | #c9dcdf | #eee8d5 | #93a1a1 | #eee8d5 | #93a1a1 | #586e75 | #657b83 | #839496 | #d33682 | #859900 | #b58900 | #dc322f | #268bd2 | #2aa198 | #cb4b16 |
| kanagawa | #7e9cd8 | #1f1f28 | #363646 | #32384b | #2a2a37 | #363646 | #1f1f28 | #727169 | #87867d | #dcd7ba | #c8c3aa | #957fb8 | #76946a | #c0a36e | #c34043 | #7e9cd8 | #7fb4ca | #ffa066 |
| kanagawa-lotus | #4d699b | #f2ecbc | #d5cea3 | #dcd5ac | #dcd5ac | #c9cbd1 | #d5cea3 | #a09cac | #8a8980 | #545464 | #43436c | #624c83 | #6f894e | #77713f | #c84053 | #4d699b | #4e8ca2 | #cc6d00 |
| rose-pine | #c4a7e7 | #191724 | #26233a | #3b344b | #1f1d2e | #26233a | #26233a | #6e6a86 | #908caa | #e0def4 | #c8c5dc | #c4a7e7 | #31748f | #f6c177 | #eb6f92 | #31748f | #9ccfd8 | #ea9a97 |
| rose-pine-dawn | #907aa9 | #faf4ed | #e3d9cf | #f2e9e1 | #f2e9e1 | #fffaf3 | #f2e9e1 | #9893a5 | #797593 | #464261 | #797593 | #907aa9 | #286983 | #ea9d34 | #b4637a | #286983 | #56949f | #d7827e |
| vesper | #ffc799 | #1a1a1a | #101010 | #232323 | #232323 | #282828 | #101010 | #5c5c5c | #7e7e7e | #ffffff | #a0a0a0 | #ffd1a8 | #99ffe4 | #ffc799 | #ff8080 | #b0b0b0 | #66ddcc | #ffc799 |

The `terminal` theme uses named ANSI colors, not RGB.

### 7.3 Host appearance and auto-switch (`src/terminal_theme.rs`, `src/client/shell/input.rs:335-366`, `src/app/theme_sync.rs`)

**Queries sent to the host**
- OSC 10/11 for foreground and background: `\x1b]10;?\x1b\\\x1b]11;?\x1b\\`.
- On macOS only, OSC 4 for indexes 0..255.
- The color-scheme query `\x1b[?996n`.
- `?2031h` turns on color-scheme change reports. It is not used on Windows.

**When the client queries**
- At startup.
- On focus gain (996 only).
- On a color-scheme change, which sends the full query again.
- On Unix, on every resize or SIGWINCH.

**Appearance**
- A CSI 997 report (`;1n` = dark, `;2n` = light) sets the appearance explicitly.
- Without that report, the appearance is inferred from the OSC 11 luminance: `r*299 + g*587 + b*114 >= 128000` means light.
- When `auto_switch` is on, the palette is rebuilt for the new appearance.
- The values are also forwarded to the server. The server uses them for the pane surface palette, and for OSC 10/11/4 and color-scheme replies to the pane apps.

### 7.4 Palette roles and the elements that use them (`Palette`, `src/app/state.rs:35-74`)

| Role | Elements |
|---|---|
| accent | focused pane border and title; focused tab bg; drop indicators `│` and `─`; ZOOM pill bg; mode bar keys and mode badge bg (not RESIZE); `update ready`; all modal and menu borders; highlighted menu and list rows; help group headings; `●` badges; `»` and `●` menu attention; group toggle `▾`/`▸`; agent view label; copy-mode cursor bg; current search match bg; popup pane border; release-notes code fg, bullets and headings; Custom/Update toast dot |
| panel_bg | tab bar bg; `+` and status text bg; mode bar bg; modal fill; toast card bg; copy toast bg; unavailable-screen bg; fg on accent and yellow (contrast); mobile header |
| sidebar_bg | expanded sidebar fill; focused row while navigating when `selection_bg` is Reset |
| active_row_bg | focused workspace row; focused agent row; active collapsed endpoint row; hovered diagnostic badge; fallback for `selection_bg` |
| selection_bg | Navigate-mode cursor row in the sidebar |
| surface0 | unfocused tab bg; tab scroll arrows; input fields; secondary buttons; Settings rule; stale selected navigator row; mobile banner bg; lifecycle banner and notice bg; inline code bg |
| surface1 | dragged workspace row; non-current search matches; fenced code bg; navigator and worktree rules |
| surface_dim | sidebar separator `│`; section divider `─`; list scrollbar track; unfocused pane scrollbar track; contrast fallback |
| overlay0 | unfocused pane border and title; unfocused auto-named tab; disabled arrows; `…`; tab-right separator; mode bar base text; ` spaces` and ` agents` headers; numbers; tree prefixes; branch (unfocused); ` new`, `menu`, `«`; agent secondary tokens; token separators; Unknown state; Disabled endpoint; toast border and body; list scrollbar thumb; modal footers |
| overlay1 | selected collapsed number; custom-named unfocused tab; enabled arrows; `+`; tab-right segments; custom and title tokens; modal body text; focused pane scrollbar thumb; overlay scrollbar thumb |
| text | focused or dragged workspace name; focused numbers; focused agent workspace; menu items; modal titles; help labels; copy prompt text; toast title; copy toast message |
| subtext0 | unfocused workspace name; unfocused agent workspace; navigator rows; modal subtitles and paths |
| mauve | RESIZE badge bg; help key column; branch of the focused workspace |
| green | Idle; `↑N`; copy toast border and dot; Online; integration `✓` |
| yellow | Working; config diagnostic bg; Connecting/Reconnecting; Timeout/Unavailable notices; integration `↻` |
| red | Blocked; `↓N`; ConfirmClose and WorktreeRemove border, title and button; worktree errors; Attention; Unsupported/Rejected notices; NeedsAttention toast dot |
| blue | Finished toast dot; mobile Done count |
| teal | Done state (sidebar, agent panel, navigator) |
| peach | not used by the UI |

Status color inconsistency: Done is teal in the sidebar and navigator but blue in the mobile header and on the toast dot (`shell.rs:233`, `mobile.rs:326`, `notifications.rs:91`).

## 8. Window title, terminal titles, notifications, sound, bell

### 8.1 Outer window title (`src/config/window_title.rs`, `src/app/window_title.rs`, `src/server/headless.rs:1424-1556`)

**Config** `ui.window_title`, default `"{hostname}: {workspace}"`. An empty value means Herdr never writes a title.

**Tokens**
- `{hostname}`
- `{workspace}` (display name)
- `{tab}` (display name)
- `{pane}` (the focused pane's `manual_label`)
- `{terminal_title}` (the focused pane's OSC title, with the spinner stripped)

`{{` and `}}` give literal braces. A parse error gives a diagnostic and Herdr leaves the title alone.

**Rendering and sanitizing**
- The title is rendered on the server for the foreground client.
- Sanitizing removes ESC, BEL, 0x9C and control characters, cuts the text to 200 chars, and trims it.

**Output**
- The client writes `\x1b]0;{title}\x07`.
- A `None` title writes `herdr` (`src/terminal_effects.rs:15-23`).
- No OSC 2 and no title stack.

**Updates**
- On every full render, deduped.
- When the focused terminal title changes and the template uses `{terminal_title}`.
- On foreground activation.
- On endpoint reset, the title becomes `herdr`.

**API override**
- `client.window_title.set` and `client.window_title.clear`.
- CLI: `herdr terminal title set|clear` (`src/cli.rs:686-712`).

### 8.2 Pane terminal titles (`src/app/terminal_titles.rs`, `src/terminal/title.rs`)

- OSC 0/2 inside a pane stops at Herdr. Herdr stores it as `terminal_title`.
- `terminal_title_stripped` drops one leading activity glyph, followed by whitespace or the end of the title:
  - braille spinners U+2800–U+28FF;
  - `·✢✳✶✻✽◐◓◑◒`.
- `PaneUpdated` fires only when the stripped form changes.
- The sidebar redraws only if a token uses one of these titles.

### 8.3 Agent states and when notifications fire

**States** (`src/detect/mod.rs:11-20`; API `src/api/schema/common.rs:160-166`)
- `AgentState` has Idle, Working, Blocked and Unknown.
- The API adds `done`, which means Idle and not yet seen (`src/app/api_helpers.rs:96-107`).

**Known agent labels** (`detect/mod.rs:43-151`): pi, claude, codex, gemini, cursor, devin, agy, cline, omp, mastracode, opencode, copilot, kimi, kiro, droid, amp, grok, hermes, kilo, qodercli, qwen, letta, maki, muse.

**Server triggers** (`src/app/actions.rs:38-93`, `src/server/headless/notifications.rs:40-109, 341-363`)
- A change to Blocked raises NeedsAttention, with the Request sound.
- A change from Working or Blocked to Idle raises Finished, with the Done sound.
- Both are sent to every shell client as `SemanticNotification {kind, title, body, sound, agent, workspace_id, tab_id, pane_id}`.
- They are suppressed right after an agent process starts.

**Client policy** (`notification_policy.rs`)
- `delay_seconds` applies.
- At the deadline, NeedsAttention is shown only if the agent is still Blocked.
- At the deadline, Finished is shown only if the agent is Done. If it is Working again, the client re-checks every 50 ms for up to 1 s.
- `target_active` means the event's tab is the focused tab.
- `suppress_external` means `target_active` and the outer terminal is not known to be unfocused.
- **Sound** always plays for NeedsAttention. For Finished it plays unless `suppress_external` is true.

**Delivery by `ui.toast.delivery`**

| Value | What is shown |
|---|---|
| `off` | nothing |
| `herdr` | in-app toast, only when not `target_active` |
| `terminal` | OSC notification, only when not `suppress_external` |
| `system` | OS notification, only when not `suppress_external` |

### 8.4 Outer-terminal notifications (`src/terminal_notify.rs`)

**Backend detection**
- `TERM_PROGRAM` = ghostty, iTerm.app or WezTerm.
- Else `KITTY_WINDOW_ID`.
- Else `TERM` = xterm-ghostty, xterm-kitty, or a value that contains wezterm.
- If no backend is found, nothing is sent.

**Sequences**
- Ghostty, iTerm2 and WezTerm: OSC 9 `\x1b]9;{title}: {body}\x1b\\`.
- Kitty: OSC 99 `\x1b]99;i=1:d=0;{title}\x1b\\\x1b]99;i=1:p=body;{body}\x1b\\`.
- OSC 777 is not used.

**Other rules**
- Under tmux, the sequence is wrapped in DCS passthrough.
- Text is sanitized.

**`system` delivery**
- macOS: `terminal-notifier`, then `osascript`.
- Windows: an actionable toast.

### 8.5 Sound (`src/sound.rs`, `src/config/sound.rs`)

**Built-in sounds:** `done.mp3` and `request.mp3`, embedded in the binary.

**Players, tried in order**
- macOS: `afplay`.
- Windows: PowerShell MediaPlayer.
- Linux: `paplay`, `pw-play`, `ffplay`, `mpg123`, `mpv`, each with a 15 s timeout.

**Disabling:** the env vars `HERDR_DISABLE_SOUND` or `NEXTEST` turn sound off.

**Config `[ui.sound]`**
- `enabled` (default true).
- `path`, `done_path`, `request_path`: mp3 files, relative to the config dir.
- `[ui.sound.agents] <label> = default|on|off`. `droid` defaults to off.

**Where it plays:** on the local client.

### 8.6 Bell

- A live BEL from **any** pane, not only the focused one, becomes `TerminalBell {count}`. BELs that end an OSC sequence and BELs replayed from restored history are ignored (`src/pane/terminal.rs:1374-1377, 4168-4180`; `src/pane.rs:2195-2207`).
- The bell goes only to the foreground client, which writes `count` × `\x07` to the host (`src/server/headless/notifications.rs:325-333`; `src/terminal_effects.rs:5-13`).
- There is no visual bell and no bell indicator.

### 8.7 Host focus and other terminal modes

**Focus reporting**
- The client enables focus reporting with `?1004h`.
- On focus gained:
  - Tell the server.
  - Repaint if `ui.redraw_on_focus_gained` is set (default true).
  - Acknowledge visible Done agents, which makes them Idle.
  - Query the appearance.
- On focus lost:
  - Clear the link hover.
  - Release input leases.
  - Tell the server.

**Kitty keyboard**
- Flags 7 are pushed (not on Windows).
- modifyOtherKeys is used as a fallback.

**Startup probe:** `\x1b[?1016$p\x1b[?u\x1b[c`, with a 250 ms timeout.

**Cursor**
- The pane cursor shape is sent as DECSCUSR, only when it changes.
- On exit the client writes `\x1b[?25h\x1b[0 q`.

**Other modes:** bracketed paste is on and line wrap is off (`src/client/terminal_setup.rs`).

## 9. Runtime data checklist (for mapping to hc)

| Data | Used by |
|---|---|
| Workspace: id, number/index, label, custom flag, cwd for new panes, git branch, ahead/behind, worktree {key, repo name, is_linked}, agent status (aggregate), focused, metadata tokens | sidebar, compact, navigator, window title, menus, worktree dialogs, mobile |
| Tab: id, workspace id, label (custom or index), custom flag, zoomed, focused, aggregate agent status | tab bar, sidebar `tab` token, navigator, rename, close confirm |
| Pane: id, tab id, manual label, cwd, foreground cwd, rect/inner rect/scrollbar rect (layout), focused, scroll metrics, alternate screen, mouse reporting modes (1000/1002/1003/1006/1016/1007), right-click passthrough, restore error, content revision | pane surface, mouse, menus, navigator |
| Pane split tree: direction, position, ratio, path | border drag |
| Agent: pane id, agent id (canonical), display agent, name, title (metadata), terminal title + stripped, state (idle/working/blocked/unknown), seen/done, state_change_seq, state labels, metadata tokens | sidebar agents, navigator, pane border title, toasts, sound |
| Terminal output: cells, cursor, OSC 8 links, selection row reads, word bounds, link resolve | pane rendering, selection, links |
| Server-side: hostname, datetime, status-command output, update available, release notes, product announcement, integration states, worktree list, config diagnostics | tab_bar_right, menu, notes, settings, worktree dialogs, banners |
| Client-only: sidebar width/collapsed/section split, agent sort, collapsed groups, tab scroll, theme, host appearance, outer focus | layout, preferences |

## 10. Coverage checklist

`src/ui/**` render functions:

| Function | Section |
|---|---|
| `render_panes` / `apply_pane_chrome` / borders / titles (`ui/panes.rs`) | 4.2, 4.3 |
| `render_selection_highlight` | 4.7 |
| `compute_tab_surface` / `render_tab_surface` (`ui/tab_surface.rs`) | 4.1 |
| `render_scrollbar_buffer` / `render_pane_scrollbar_buffer` (`ui/scrollbar.rs`) | 4.5, 5.0 |
| `resolved_token_spans` / sidebar rows (`ui/sidebar.rs`, `ui/sidebar/tokens.rs`) | 2.9 |
| `render_copy_feedback_buffer` / `render_config_diagnostic_buffer` (`ui/status.rs`) | 4.11, 4.12 |
| release notes / product announcement display lines (`ui/release_notes.rs`) | 5.2, 5.3 |
| onboarding constants (`ui/onboarding.rs`) | 5.1 |
| `centered_popup_rect`, `modal_stack_areas`, buttons (`ui/widgets.rs`) | 5.0 |
| `truncate_end` (`ui/text.rs`) | 2.9, 4.3 |

Client render functions (`src/client/shell/**`):

| Function | Section |
|---|---|
| `render_shell`, `render_mode_bar`, `render_sidebar_background` | 1 |
| `render_sidebar`, `render_collapsed_sidebar`, endpoint sidebar | 2 |
| `render_agent_panel` | 2.5 |
| `render_tab_bar` | 3 |
| `render_client_overlay` | 5.1–5.7, 5.10, 5.11 |
| `render_context_menu` | 5.8 |
| `render_global_menu` | 5.9 |
| notifications | 5.12 |
| endpoint notices | 5.13 |
| mobile | 5.17 |
| link hover | 4.9 |
| copy-mode highlights | 4.8 |

Overlay kinds: all 13 `ClientShellOverlay` kinds are covered in section 5.

`src/app/**` UI state:

| Item | Section |
|---|---|
| `popup.rs` (popup pane) | 4.10 |
| `tab_bar_status.rs` | 3.7 |
| `agent_view.rs` | 2.5, 4.13 |
| `window_title.rs` | 8.1 |
| `terminal_titles.rs` | 8.2 |
| `creation.rs` (labels, tab status aggregation) | 2.10, 3.2 |
| `custom_commands.rs` | custom-command popups (4.10); no other UI |
| `state.rs` toast, release notes, announcement, copy feedback | 4.11, 5.2, 5.3, 5.12 |
