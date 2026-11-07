# drover

A terminal UI for herdr-core (`hc`) with the key binds and visible elements of Herdr TUI 0.9.5, sharing spaces, tabs and panes with the Herdr desktop app.

- `HANDOFF.md`: read at the start of a session that has no context yet. It lists the reference repos in `~/oss/thuic-ref`.
- `docs/plan.md`: read before you change the design: decisions, processes, the `herdr-app` doc as source of truth, panes, config, testing.
- `docs/parity.md`: read when you add or change a user-visible feature. It is the feature inventory against Herdr TUI v0.9.5.
- `docs/research/`: read when you build a feature and need its exact behavior: Herdr keys and config, Herdr UI elements, the hc protocol and the `herdr-app` doc, with source lines.

hc owns every program. drover owns only the UI. When a feature needs runtime state, find it in hc (`hc schema`) before you add state to drover.

Test against an isolated hc core, so tests never touch the user's own terminals.
