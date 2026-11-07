# thuic plan

## Goal

thuic is Herdr TUI 0.9.5 as it is (look, keys, config, behavior), rebuilt on herdr-core (`hc`). hc owns every program. thuic owns only the UI. thuic and the Herdr desktop app show the same spaces, tabs, panes and terminals, live.

Out of scope, because hc already does it: a PTY server, terminal persistence and recovery, agent status detection, SSH transport to other machines, self-update of a runtime.

## Design

### Processes

- One `thuic` process per client. It connects to the local hc socket. There is no thuic server: hc keeps the terminals alive, and the shared doc keeps the layout. Two thuic clients, or thuic and the app, stay in sync through `events.subscribe` and doc events.
- Other machines come through hc's machine link (`machine/t7` addresses, the `on` parameter). thuic never opens SSH itself.
- Over SSH, a person runs `thuic` on the remote host. It then talks to that host's core.

### Source of truth: the `herdr-app` doc

- Each core holds a JSON doc named `herdr-app` (`doc.get` / `doc.set`, version compare-and-swap, at most 1 MiB, `on` picks the machine). Shape: `spaces[] {id, name, folder, tabs[] {id, name, root}}`. `root` is a tree of `row` / `column` nodes with `block` leaves. A block holds `content`: `{terminal: "t12"}`, or other kinds such as `{mod: "x"}`.
- Schema source: `herdr-app/src/layout.rs` and `herdr-app/src/mirror_sync.rs` in the refs. A live sample: `samples/herdr-app-doc.json`.
- thuic reads the doc, draws it, and writes every layout change back with `doc.set` and the version it read. On a version conflict, it reads again, applies its change to the new doc, and retries.
- Terminals that thuic starts get the labels the app uses: `herdr.space=<id>`, `herdr.tab=<id>`, `where`, and keep on (`samples/herdr-app-AGENTS.md`, section on core terminals).
- An empty `name` means "the app picks the name" (the space folder name, the tab number). thuic uses the same rule, so both show the same names.

### Terminal panes

- Each visible pane: `terminal.attach` (framed connection) → a libghostty-vt terminal → cells drawn into the ratatui buffer. Input goes back on the same stream.
- Key encoding goes through libghostty, as in Herdr TUI (herdr PR #5017).
- Kitty graphics: pass through to the host terminal, as Herdr TUI does (`herdr/src/kitty_graphics`).
- Pane size: thuic sets each attached terminal's size to its pane size. Known hc bug: herdr-core issue #5814 (a smaller client leaves a stale screen size after detach).
- Panes that are not visible are not attached. The sidebar status comes from `events.subscribe`, not from screens.
- Image paste: read the clipboard image on the client, `terminal.put` it beside the terminal, then bracket-paste the returned path. This works for local and remote terminals the same way.

### UI

- ratatui + crossterm, as in Herdr TUI. Copy Herdr TUI's code where it gives the exact look and keys: sidebar, tab bar, pane borders, themes, keymap, config parser, copy mode, selection, mouse. Keep the Apache-2.0 notice for copied code.
- Config: Herdr TUI's `config.toml` schema. A sample from the user's Mac: `samples/herdr-tui-config.mac.toml`.

### Crates

- `hc` client: start from pocketty `crates/hc` (JSON-RPC, framed attach stream, tests).
- VT: libghostty-vt. Two existing bindings to compare: pocketty `crates/terminal` (crates.io `libghostty-vt`) and herdr `crates/ghostty-vt` (vendored, Zig build). Pick one. The other is a reference.

## Parity

The target is Herdr TUI at tag `v0.9.5` (refs `herdr/`). Make `docs/parity.md` an inventory of every user-visible feature, key and config option in that version. Mark each one as: match, maps to hc, or dropped (with the reason). A feature that belongs to the old runtime maps to hc (session restore, `--remote`, server handoff) or is dropped.

## Testing

- Unit: doc ↔ layout mapping, conflict retry, tree edits (split, close, move, resize), keymap, config parsing.
- E2E: drive `thuic` in tmux on workbox (`send-keys`, `capture-pane -p -e`) against a real hc. Use an isolated core for tests so they never touch the user's terminals. Find how hc selects its state dir or scope first (`HC_SCOPE`, `~/.local/state/herdr-core/<scope>`). See also `herdr-core/crates/herdr-core/tests/fake_tui.py` and followcat `scripts/parity/`.
- Integration with the app: the user's Mac app can add workbox as a machine (`hc machine add`, target `lukeed@workbox`). Then a change in thuic must show in the app, and a change in the app must show in thuic.
- Pixel check: compare `capture-pane -e` output of thuic and of `herdr` 0.9.5 on the same layout and theme.

## Known hc issues to design around

- herdr-core #5814: stale screen size after a smaller client detaches.
- herdr-core #111: `hc attach --observe` on a busy terminal uses about 99% CPU per stream.
- herdr #5710: hc terminals answer OSC 10/11 with a fixed dark palette until the app shows them.
- macOS: programs in hc cannot read `~/Desktop`, `~/Documents` or `~/Downloads` until Herdr Core gets access (TCC). This is a property of hc, not of thuic. A dropped file path there fails for any client.
