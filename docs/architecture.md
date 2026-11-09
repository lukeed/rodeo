# rodeo architecture

How the app crate (`crates/rodeo`) is built on the four library crates. Read `docs/plan.md` first for the decisions; this file is how the code carries them out.

## Crates

| Crate | Job | Depends on |
|---|---|---|
| `rodeo-hc` (`hc`) | async hc client: connect, calls, attach streams, event subscription, terminal fold, test core | tokio |
| `rodeo-vt` (`vt`) | one pane's terminal: feed bytes, encode keys/mouse/paste, render into a ratatui buffer, scrollback, selection, search, copy mode | libghostty-vt, ratatui, crossterm |
| `rodeo-doc` (`doc`) | `herdr-app` doc types and every edit, ids, names, weight splits, neighbors | serde |
| `rodeo-config` (`config`) | Herdr's config schema, load/reload/diagnostics, keymap and resolver, themes, sidebar tokens, window title, settings writes, preferences | serde, toml, ratatui, crossterm |
| `rodeo` | the binary: state, event loop, layout, rendering, input, actions, sync | all of the above |

Library crates never depend on `rodeo`. Only `rodeo` does IO to the host terminal.

## Process and threads

- One tokio current-thread runtime with a `LocalSet`. All app state lives on that one thread, because `vt::Terminal` is not `Send`.
- IO runs in tasks that send `Msg` values on one unbounded channel into the main loop: host input (crossterm `EventStream`), hc events, attach output per pane, call results, timers.
- The main loop takes one message, updates state, marks the frame dirty, and draws at most once per ~8 ms (coalesce bursts of output). Drawing reads state; it never does IO.
- Slow hc calls (`terminal.kill`, `terminal.list`, `machine.exec`) run in spawned tasks and report back with a `Msg`.

## State

- `Machines`: the local machine plus each connected, non-temporary machine from `machine.list`. Each has its name, state, its `herdr-app` doc (`Document` + version, or loading / undecodable), and its edit queue.
- `Terminals`: the hc fold from one unfiltered `events.subscribe` (unfiltered, so doc events arrive). It holds every machine's terminals as `machine/tN`.
- `Panes`: one entry per terminal block in any doc: the `vt::Terminal`, the attach writer, its current size, whether rodeo has made itself the size leader.
- `View` (private to this client, never in the doc): active machine and space, active tab per space, focused pane per tab, previous pane, zoom, mode, open overlay, sidebar preferences, scroll offsets, selection, copy mode state, pending highlights.
- `Git`: branch, ahead/behind and worktree info per space, from `machine.exec git` on the space's machine, refreshed on focus change and on a timer.

## Doc sync

- Read: on start, on every `doc` event, after `missed`, after reconnect, and when a machine comes back. If the version changed, replace that machine's `Document`, then drop view entries whose ids are gone.
- Write: every layout change is an edit closure `Fn(&mut Document) -> Edit`. rodeo applies it to its local copy at once (the screen updates now), then sends `doc.set` with the version it read. One write in flight per machine; later edits queue behind it.
- Conflict: re-read the doc, re-apply every queued edit to the fresh doc, and write again. rodeo never drops a user's edit on conflict, and never writes when nothing changed.
- Write only on real user actions. Never write to normalize, and never write a field or kind the app cannot parse.

## Panes and attach

- Attach every terminal block in every doc. A pane in a tab that is not shown attaches with `resumed: true`, so it does not take the terminal's size from other viewers.
- A pane that becomes visible sends RESIZE with its pane size, which makes rodeo the size leader for it. Hidden panes keep their last size.
- Output goes into the pane's VT. VT replies go back as AUTOMATIC_INPUT. Title, bell, clipboard and notification events from the VT go to the app.
- A stream CLOSE with `exited` or `killed` removes the pane from the doc (as the app does). A lost kept terminal keeps its pane and reattaches on `restarted`.

## Agents

- A terminal is an agent when its hc `status` is not null. Displayed state follows the app (`app/model.rs:527-536`): `working`; `blocked` or `error` → blocked; else done when `turn > herdr.seen`, else idle.
- rodeo raises `herdr.seen` to `turn` once per turn, only while the pane is visible, focused, and the host terminal has focus.
- Toasts, sound and `open_notification_target` fire on state transitions seen in hc events.

## Layout and rendering

- `layout` computes the regions (desktop: sidebar, tab bar, pane surface; mobile: header, pane surface) and each tab's pane rects from the doc tree, with Herdr's border, gap and outer border rules, using `doc`'s integer weight splits.
- Each render function draws one region into the ratatui buffer and records hit rects for the mouse in a `Hits` value the input side reads.
- Pane content is drawn by `vt::Terminal::render` into the pane's inner rect.

## Input

- Order per key: overlay, popup, copy mode, mode handler (terminal, prefix, navigate, resize), then the focused pane. `config::Keymap` resolves keys to actions.
- One `Action` enum covers every key bind action. `actions::run(action)` is the single place that changes view state, queues doc edits, or calls hc.
- Mouse events go through the `Hits` from the last frame.

## Testing

- Unit: each library crate, and pure app logic (layout, hit testing, action results on a `View` + `Document`).
- App: drive the app in-process with a ratatui `TestBackend` against an isolated `hc::testing::TestCore`; send synthetic key and mouse events; assert on buffer text and on the doc in hc.
- E2E: run the `rodeo` binary in tmux against an isolated core (`send-keys`, `capture-pane -p -e`).
