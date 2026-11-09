# References

Read only, in `~/oss/thuic-ref`. Not part of the repo. Read a source here to learn behavior; copy code only from `pocketty/` (the owner's own code).

| Path | What it is for |
|---|---|
| `herdr/` | Herdr TUI at tag v0.9.5 (`2ecf19a`): the parity target (behavior only, never copied) |
| `herdr-app/` | Desktop app source: the `herdr-app` doc schema (`src/layout.rs`, `src/mirror_sync.rs`), the labels, how it uses hc (`src/hc.rs`) |
| `herdr-core/` | hc source: the protocol, attach, docs, events, test helpers |
| `pocketty/` | The user's own app (private, branch `dev`): `crates/hc` (a Rust hc client to reuse), `crates/terminal` (libghostty-vt wrapper), `docs/adr/0012-hc-replaces-herdr.md` |
| `community/followcat/` | hc-tui, a parity-driven rewrite: `docs/` (requirements, parity, runtime), `scripts/parity/` |
| `community/wuxiaobai24/` | hc-tui that shares the `herdr-app` doc with the app (docs in Chinese) |
| `community/SuTang-vain/` | Herdr TUI copy with an hc pane backend in `src/core_native/` |
| `samples/hc-schema.json` | `hc schema` (OpenRPC) from this machine's hc 0.1.18 |
| `samples/herdr-app-doc.json` | A live `herdr-app` doc from the user's Mac (doc version 58) |
| `samples/herdr-app-AGENTS.md`, `herdr.d.ts`, `herdr-app-commands.json`, `herdr-app-skill.md` | The app's generated docs, types and command list (app 0.1.13) |
| `samples/hc-skill.md` | `hc --skill` |
| `samples/herdr-tui-config.mac.toml` | The user's Herdr TUI config on the Mac |

Use the community projects as maps of hc pitfalls, not as code to copy. Their authors and licenses: all Apache-2.0, unknown authors.
