# rodeo

A terminal UI for herdr-core (`hc`) with the key binds and visible elements of Herdr TUI 0.9.5, sharing spaces, tabs and panes with the Herdr desktop app.

- `docs/references.md`: read when you need a reference repo or sample (Herdr TUI, the desktop app, hc, pocketty) in `~/oss/thuic-ref`.
- `docs/plan.md`: read before you change the design: decisions, processes, the `herdr-app` doc as source of truth, panes, config, testing.
- `docs/architecture.md`: read before you change the app crate: crates, threads, state, doc sync, attach, input, testing.
- `docs/parity.md`: read when you add or change a user-visible feature. It is the feature inventory against Herdr TUI v0.9.5.
- `docs/releasing.md`: read before you cut a release or change a workflow: the steps, secrets, one-time setup and what each job makes.
- `docs/research/`: read when you build a feature and need its exact behavior: Herdr keys and config, Herdr UI elements, the hc protocol and the `herdr-app` doc, with source lines.

hc owns every program. rodeo owns only the UI. When a feature needs runtime state, find it in hc (`hc schema`) before you add state to rodeo.

Test against an isolated hc core, so tests never touch the user's own terminals.
