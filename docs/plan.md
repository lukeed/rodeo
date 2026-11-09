# rodeo plan

## Goal

rodeo is a terminal UI for herdr-core (`hc`) with the key binds and visible elements of Herdr TUI 0.9.5. hc owns every program. rodeo owns only the UI. rodeo and the Herdr desktop app show the same spaces, tabs, panes and terminals, live.

rodeo is a fresh build. Herdr TUI is the spec, not a code source: read its code to learn behavior, then write rodeo's own code.

Out of scope, because hc already does it: a PTY server, terminal persistence and recovery, agent status detection, SSH transport to other machines, self-update of a runtime.

Also out of scope: Herdr TUI plugins.

## Design

### Processes

- One `rodeo` process per client. It connects to the local hc socket. There is no rodeo server: hc keeps the terminals alive, and the shared doc keeps the layout. Two rodeo clients, or rodeo and the app, stay in sync through `events.subscribe` and doc events.
- Other machines come through hc's machine link (`machine/t7` addresses, the `on` parameter). rodeo never opens SSH itself.
- Over SSH, a person runs `rodeo` on the remote host. It then talks to that host's core.

### Source of truth: the `herdr-app` doc

- Each core holds a JSON doc named `herdr-app` (`doc.get` / `doc.set`, version compare-and-swap, at most 1 MiB, `on` picks the machine). Shape: `spaces[] {id, name, folder, tabs[] {id, name, root}}`. `root` is a tree of `row` / `column` nodes with `block` leaves. A block holds `content`: `{terminal: "t12"}`, or other kinds such as `{mod: "x"}`.
- Schema source: `herdr-app/src/layout.rs` and `herdr-app/src/mirror_sync.rs` in the refs. A live sample: `samples/herdr-app-doc.json`.
- rodeo reads the doc, draws it, and writes every layout change back with `doc.set` and the version it read. On a version conflict, it reads again, applies its change to the new doc, and retries.
- Terminals that rodeo starts get the labels the app uses: `herdr.space=<id>`, `herdr.tab=<id>`, `where`, and keep on (`samples/herdr-app-AGENTS.md`, section on core terminals).
- An empty `name` means "the app picks the name" (the space folder name, the tab number). rodeo uses the same rule, so both show the same names.

### Mod blocks

- Files: rodeo draws a text folder tree of the space folder.
- All other mods (Changes, and any others): a placeholder with the mod name. The block keeps its place in the layout.

### Terminal panes

- Each visible pane: `terminal.attach` (framed connection) → a libghostty-vt terminal → cells drawn into the ratatui buffer. Input goes back on the same stream.
- Key encoding goes through libghostty.
- Kitty graphics: pass through to the host terminal.
- Pane size: rodeo sets each attached terminal's size to its pane size. Known hc bug: herdr issue #5814 (a smaller client leaves a stale screen size after detach).
- Every terminal pane in the doc is attached, visible or not, as the app does. Hidden tabs are sized as their panes would be. Reason: when the last viewer detaches, hc resizes the program to its headless size (200x120), so detaching hidden panes would resize their programs on every tab switch. Observe-only attaches are not a fix (herdr-core #111).
- The sidebar status comes from `events.subscribe`, not from screens.
- Image paste: read the clipboard image on the client, `terminal.put` it beside the terminal, then bracket-paste the returned path. This works for local and remote terminals the same way.

### UI

- ratatui + crossterm.
- Same key binds as Herdr TUI, and the same elements on screen (sidebar, tab bar, pane borders, status). The look does not need to be exact.

### Config

- Path: `~/.config/rodeo/config.toml`. Schema: Herdr TUI's `config.toml`. A sample from the user's Mac: `samples/herdr-tui-config.mac.toml`.
- Migration: on start, if rodeo's file does not exist and `~/.config/herdr/config.toml` does, copy it once. After that, rodeo reads only its own file and never writes Herdr's.

### Crates

- `hc` client: start from pocketty `crates/hc` (JSON-RPC, framed attach stream, tests).
- VT: pocketty `crates/terminal` on crates.io `libghostty-vt`. Herdr's vendored `crates/ghostty-vt` is a reference only.

### Repo

- Branch: `main` only.
- License: MIT.

## Parity

The target is Herdr TUI at tag `v0.9.5` (refs `herdr/`): its key binds, its visible elements and its config options. Make `docs/parity.md` an inventory of them. Mark each one as: match, maps to hc, or dropped (with the reason). A feature that belongs to the old runtime maps to hc (session restore, `--remote`, server handoff) or is dropped. Plugins are dropped.

## Testing

- Unit: doc ↔ layout mapping, conflict retry, tree edits (split, close, move, resize), keymap, config parsing, config migration.
- E2E: drive `rodeo` in tmux on workbox (`send-keys`, `capture-pane -p -e`) against a real hc. Use an isolated core for tests so they never touch the user's terminals. Isolate with `HOME`, `HC_SESSION`, `XDG_RUNTIME_DIR`, `XDG_STATE_HOME` and `XDG_CONFIG_HOME` under one temp root, and stop with `hc server stop` (`docs/research/hc-integration.md` 1.2). See also `herdr-core/crates/herdr-core/tests/fake_tui.py` and followcat `scripts/parity/`.
- Integration with the app: the user's Mac app can add workbox as a machine (`hc machine add`, target `lukeed@workbox`). Then a change in rodeo must show in the app, and a change in the app must show in rodeo.
- Parity check: run rodeo and `herdr` 0.9.5 on the same layout. The same keys must give the same result, and the same elements must show.

## Known hc issues to design around

- herdr #5814: stale screen size after a smaller client detaches.
- herdr-core #111: `hc attach --observe` on a busy terminal uses about 99% CPU per stream.
- herdr #5710: hc terminals answer OSC 10/11 with a fixed dark palette until the app shows them.
- macOS: programs in hc cannot read `~/Desktop`, `~/Documents` or `~/Downloads` until Herdr Core gets access (TCC). This is a property of hc, not of rodeo. A dropped file path there fails for any client.
