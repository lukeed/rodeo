# thuic

A terminal UI for herdr-core (`hc`): Herdr TUI 0.9.5 as it is, sharing spaces, tabs and panes with the Herdr desktop app.

- `HANDOFF.md`: read at the start of a session that has no context yet. It holds the decisions made, the open questions, and the reference repos in `~/oss/thuic-ref`.
- `docs/plan.md`: read before you change the design: processes, the `herdr-app` doc as source of truth, panes, testing.
- `docs/parity.md`: read when you add or change a user-visible feature. It is the feature inventory against Herdr TUI v0.9.5. If it does not exist yet, writing it is the first task (`docs/plan.md`, Parity).

hc owns every program. thuic owns only the UI. When a feature needs runtime state, find it in hc (`hc schema`) before you add state to thuic.

Test against an isolated hc core, so tests never touch the user's own terminals.
