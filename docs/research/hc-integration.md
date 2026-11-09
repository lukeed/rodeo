# hc integration: facts for drover

Research date: 2026-11-07. Everything here is cited to source. Items marked **verified** were run against an isolated hc 0.1.18 core on workbox (section 1.2).

## Sources

All paths are relative to `~/oss/thuic-ref/`. Short prefixes used below:

| Prefix | Path | Version |
|---|---|---|
| `core/` | `herdr-core/crates/herdr-core/src/` | hc 0.1.18 (`herdr-core` commit `cd12ddd`, `Cargo.toml` version 0.1.18). Installed `hc --version` is `hc 0.1.18`. `samples/hc-schema.json` is identical to `herdr-core/schema/hc.openrpc.json`. |
| `api-doc` | `herdr-core/schema/README.md` | the client guide for the protocol |
| `app/` | `herdr-app/src/` | herdr-app 0.1.13 (commit `7fe7ac3`) |
| `pkt/` | `pocketty/crates/` | pocketty commit `8e32da5` |
| `wux/` | `community/wuxiaobai24/` | `d0cf772` |
| `fol/` | `community/followcat/` | `f86345a` |
| `sut/` | `community/SuTang-vain/` | `c5f09df` |

---

## 1. Connection

### 1.1 Socket path resolution (Unix)

`Paths::from_env` (`core/paths.rs:16-49`):

1. Session name: `HC_SESSION`, trimmed; empty or unset means `default` (`core/paths.rs:83-93`). It must be a valid folder name (no `/ \ : * ? " < > |`, no control chars, not `.` or `..`).
2. State dir: `${XDG_STATE_HOME:-$HOME/.local/state}/herdr-core/<session>/` (`core/paths.rs:22-23`, `core/platform/unix.rs:85-88`).
3. Config: `${XDG_CONFIG_HOME:-$HOME/.config}/herdr-core/config.toml` (`core/paths.rs:24`). Config and templates are shared by all sessions (`herdr-core/docs/security.md:44`).
4. Socket, first match wins (`core/paths.rs:27-35`):
   - `HC_SOCKET` env (non-empty).
   - `socket` key in config.toml (`core/config.rs:24-25`).
   - `$XDG_RUNTIME_DIR/herdr-core/<session>.sock`; else `/run/user/<uid>/herdr-core/<session>.sock` if `/run/user/<uid>` exists; else `<state dir>/<session>.sock` (`core/platform/unix.rs:98-105`). On macOS there is normally no `XDG_RUNTIME_DIR` and no `/run/user`, so the socket is `~/.local/state/herdr-core/default/default.sock`.
5. The holder socket sits beside it: `<socket minus .sock>.holder.sock` (`core/paths.rs:33-34`). Path length is checked against `sun_path` (macOS 103, Linux 107 bytes) (`core/paths.rs:36-47`, `core/platform/unix.rs:92-96`). Keep test roots short.
6. The running core writes the socket path it listens on into `<state dir>/core.lock` (`core/server.rs:30-32`). The CLI uses it to say "the core listens at X, but this shell looks at Y" when env differs (`core/client.rs:107-118`). drover can read that file for the same diagnosis.

Directories are created with mode 0700 (`core/platform/unix.rs:107-111`, `herdr-core/docs/security.md:20-25`).

### 1.2 Sessions and an isolated test core (verified)

- A different `HC_SESSION` is a different core, terminals and state dir. It is not a security boundary (`herdr-core/docs/security.md:33-45`). The doc says tests should also use scratch XDG dirs.
- On Linux, autostart makes the core a systemd user service (`systemd-run`, unit name `hc-serve-<hex>`) only when `XDG_RUNTIME_DIR` is the default `/run/user/<uid>` and a user manager runs there. A custom `XDG_RUNTIME_DIR` makes it a plain detached process (`core/platform/systemd.rs:51-61`, `core/client.rs:177-195`). So a test core with its own `XDG_RUNTIME_DIR` never creates a systemd unit.
- hc's own test harness uses `HOME`, `HC_SESSION=test`, `XDG_RUNTIME_DIR`, `XDG_STATE_HOME`, `XDG_CONFIG_HOME` all under one temp root, `env_clear()`, and `HC_SETTLE_MS=0` (`herdr-core/crates/herdr-core/tests/common/mod.rs:19-74`). Teardown: `hc server status` then `hc server stop`, then it checks no process still has the root in its env (`tests/common/mod.rs:143-174`). pocketty does the same with `hc server start` / `hc server stop` (`pkt/hc/tests/common/mod.rs:22-82`).
- Holders `setsid` away: killing the core process does not end programs; only `hc server stop` with the same env does (`fol/docs/integration.md:41-48`).
- Test-only env knobs in hc: `HC_HEARTBEAT_MS` (default 15000, `core/server.rs:229-232`), `HC_FOREGROUND_MS` (500, `core/foreground.rs:156`), `HC_RETAIN_MS` (finished-record retention, `core/terminals.rs:458-461`), `HC_SPAWN_MS` (10000, `core/api/run.rs:180`), `HC_RECONNECT_MS` (`core/remote.rs:199-201`), `HC_SETTLE_MS`.

Verified recipe (run on workbox, then removed; no process was left):

```sh
R=$(mktemp -d /tmp/drv.XXXX); mkdir -p $R/run $R/state $R/config
export HOME=$R HC_SESSION=t XDG_RUNTIME_DIR=$R/run XDG_STATE_HOME=$R/state XDG_CONFIG_HOME=$R/config
hc server start        # prints server.status JSON; socket: $R/run/herdr-core/t.sock
hc server stop         # prints {"stopped":true}; ends programs and holders
rm -rf $R
```

### 1.3 How to connect

Two ways, both carry the same bytes:

- **Spawn `hc connect`** and speak on its stdin/stdout. It resolves the socket like any `hc` command, starts the core if none listens (`connect_or_start`), then pumps bytes both ways, unbuffered (`core/cli.rs:512-522`, `core/cli.rs:829-848`). `hc connect --session <name>` sets `HC_SESSION` for that call (`core/cli.rs:101-107`). The app does this (`app/hc.rs:297-304`), and so does pocketty (`pkt/daemon/src/hc.rs:26-44`, it also removes `HC_TERMINAL_ID` and `HC_TERMINAL_SOCKET` from the child env). On error, `hc connect` writes a JSON error object to stderr; the app parses `error.message` and `error.hint` from it (`app/hc.rs:285-293`).
- **Connect to the Unix socket directly** with the resolution in 1.1. Autostart is then drover's job.

Finding the binary: the app accepts a candidate only if `--version` prints `hc …`, because some window managers ship an unrelated `hc` (herbstclient). It also looks for `herdr-core` (`app/core_bin.rs:1-8`, `app/core_bin.rs:76-118`). Usual dirs: `~/.local/bin`, Homebrew, mise, Nix, cargo.

### 1.4 Plain mode, framed mode, hello, versions

- A connection starts in **plain mode**: JSON-RPC 2.0, one message per line (`api-doc:28-34`, `core/server.rs:141-186`). A request without `id` is a notification and gets no reply (`core/server.rs:437-438`).
- `hello {"framed": true}` switches the connection to **framed mode** after the reply. The reply itself is a plain line (`core/server.rs:183-185`, `core/server.rs:435`). `terminal.attach` needs framed mode (`api-doc:42-43`, error kind `needs_framed`, `core/api.rs:413-419`).
- In plain mode, `events.subscribe` turns the connection into a notification-only stream (`core/server.rs:179-182`, `core/server.rs:234-267`). A plain subscriber that does not read for 30 s is dropped (`core/server.rs:237`, `core/server.rs:261-264`).

hello params (`core/api/types.rs:98-113`): `generation?: u32`, `framed: bool`, plus `core`, `calls_you`, `features` (only for another core dialing in). hello result (`core/api/types.rs:115-123`):

```json
{"generation":1,"version":"0.1.18","core":{"id":"<32 hex>","name":"<hostname>"},"framed":true,
 "methods":["hello","server.status",...],"features":["framed","attach","events","status","shell","resize_notifications","automatic_input"]}
```

- Protocol generation is 1 (`core/api.rs:15`). A different `generation` fails with kind `unsupported_generation` (`core/api.rs:169-176`). `wux/docs/protocol-facts.md:34` says `needs_update`; the source says otherwise.
- Feature list: `core/api.rs:16-17`.
- The API only grows within a generation; `released.openrpc.json` guards it (`api-doc:14-19`).
- How the app gates a core (`app/hc.rs:178-236`): reject if `generation > 1` (app too old) or `< 1`; reject if `version` < `distribution/core-version` (= `0.1.11`); reject if any method in `USES` is missing: `terminal.run, terminal.list, terminal.kill, terminal.label, terminal.attach, terminal.keep, doc.get, doc.set, events.subscribe` (`app/hc.rs:181`).
- `core.id` is permanent per state dir; it changes when the disk turns up under another machine id (a cloned image), and then the old terminals are forgotten (`core/server.rs:100-134`).

### 1.5 Framed wire

Frame: `[channel u32 LE][kind u8][len u32 LE][payload]`, payload at most 16 MiB (`core/wire.rs:1-49`, `api-doc:45-58`).

| kind | name | direction | payload |
|---|---|---|---|
| 0 | DATA | both | channel 0: one JSON-RPC message; other channels: stream bytes |
| 1 | CREDIT | client → core | u32 LE: bytes consumed |
| 2 | CLOSE | both | core → client: JSON reason (empty when it answers a client CLOSE); client → core: empty |
| 3 | RESIZE | viewer → core; core → observer (only with `resize_notifications`) | u16 LE cols, u16 LE rows |
| 4 | FOCUS | viewer → core | 1 byte: 1 gained, 0 lost |
| 5 | FACTS | viewer → core | JSON `Display` |
| 6 | AUTOMATIC_INPUT | viewer → core | terminal-generated reply bytes |

- Channel 0 carries JSON-RPC both ways. Calls on it run side by side, so a slow call (kill, wait) does not hold up others (`core/server.rs:315-351`). Use unique ids and match replies by id. `$/cancelRequest {id}` only aborts `machine.exec` (`core/server.rs:325-329`).
- Flow control (`core/wire.rs:25-31`, `api-doc:103-106`): each stream starts with 256 KiB credit (`WINDOW`). Send CREDIT for bytes consumed. A viewer more than 4 MiB behind (`MAX_PENDING`) gets a fresh repaint, not the backlog. The core sends at most 16 KiB per DATA frame (`CHUNK`). Control (channel 0) replies queue up to 64 MiB (`MAX_CONTROL`); past that the connection is dropped (`core/wire.rs:170-180`, `core/server.rs:197-200`).
- The core never sends CREDIT to clients. Client → core DATA is not flow-controlled.
- Order the core guarantees: control frames go first; streams share the rest round-robin; a stalled stream never blocks channel 0 or other streams (`core/wire.rs:156-158`, `core/wire.rs:278-310`).
- An attach stream is opened **paused** until its reply is queued, so the reply always comes before the first paint (`core/wire.rs:189-199`, `core/server.rs:333-339`). An `events.subscribe` channel is opened **not paused** (`core/api.rs:577-578`), so an event or heartbeat on that channel can in theory arrive before the subscribe reply. The app buffers frames for unknown channels in `early` and replays them once the reply names the channel (`app/hc.rs:174-175`, `app/hc.rs:378-383`, `app/hc.rs:459`). pocketty drops them (`pkt/hc/src/client.rs:243-247`).

### 1.6 Errors

JSON-RPC error: `{"code":-32000,"message":"...","data":{"kind":"not_found","hint":"..."}}` (`core/rpc.rs:14-52`). Protocol errors: `parse_error` -32700, `invalid_request` -32600, `unknown_method` -32601, `invalid_params` -32602, `internal` -32603 (`core/rpc.rs:21-39`). Kinds drover will meet: `not_found`, `machine_unknown`, `unavailable` (incl. "lost, waiting for its new shell"), `finished`, `needs_framed`, `conflict`, `name_taken`, `spawn_failed`, `live`, `refused`, `needs_update`, `needs_login`, `blocked`, `invalid_params`, `denied`. Branch on `data.kind`, never on message text.

### 1.7 Auth

None. Anyone who can open the socket has full control. Trust is the 0700 dirs plus same-user processes (`herdr-core/docs/security.md:18-31`). Remote machines are reached by the core over SSH; drover never handles credentials (`herdr-core/docs/security.md:47-57`).

---

## 2. Methods drover needs

Method table with descriptions and error kinds: `core/api.rs:56-152`. Param and result types: `core/api/types.rs`. Full schema: `samples/hc-schema.json`.

### 2.1 Addresses and machines

- A local terminal address is `t` plus a base-30 counter over `0123456789bcdfghjkmnpqrstvwxyz` (no vowels, no `l`): `t1..t9, t10..t9z, t100..` (`core/terminals.rs:167-185`). It names that terminal forever on that core. A `name` (1-32 of `a-z 0-9 - _ .`, starts with a letter or digit, not id-shaped) names whoever holds it now (`core/addr.rs:6-15`, `core/terminals.rs:318-330`). The same name rules apply to doc names.
- `<machine>/t7` addresses a terminal on a machine this core added. `<own core id or name>/t7` also means local (`core/addr.rs:28-45`).
- Methods routed by the `terminal` param's machine prefix: `terminal.attach, get, kill, rm, label, keep, read, send, keys, interrupt, wait, put` (`core/remote.rs:721-734`, `core/remote.rs:773-786`).
- Methods routed by an `on` param: `terminal.run, machine.exec, file.read, file.write, file.edit, doc.get, doc.set` (`core/remote.rs:763-772`). Unknown machine: kind `machine_unknown`.
- `terminal.list` and the `events.subscribe` snapshot include every connected machine's terminals as `machine/t7` (`core/api.rs:257-273`, each machine gets 3 s; `api-doc:142-146`).
- Labels and ids written on a remote terminal are the remote machine's local ids. The app strips its `machine/` prefix before writing labels (`samples/herdr-app-AGENTS.md:140-142`, `app/mirror.rs:61-67`).

`machine.list` → `{machines: MachineView[]}` (`core/api.rs:676-679`). `MachineView` = `Machine` fields `name, target, via, temporary, session, id, host, version` (`core/machines.rs:13-34`) plus `state` (`connecting | connected | needs_login | needs_update | unreachable`), `since_ms?`, `why?` (`core/remote.rs:174-197`). The app ignores `temporary: true` machines entirely (`app/window.rs:1964`, `samples/herdr-app-AGENTS.md:106-108`) and polls `machine.list` every fifth 500 ms tick, about 2.5 s (`app/window.rs:652`, `app/window.rs:677`).

### 2.2 The Terminal record

Serialized as `Public`: the `Terminal` struct (`core/terminals.rs:13-141`) minus private fields (`core/terminals.rs:406-424`). Fields a client sees:

`address, name, labels{}, argv[], cwd (start folder), keep, state ("live"|"finished"), pid, exit_code, signal, reason, created_ms, finished_ms, screen ("exact"|"inexact"), cols, rows, identity, status{state,kind?,progress?,app?,title?,msg?}|null, status_ms, records{id: status}, turn, reported, title, parent, notify, die_with_parent, rm, template, first_message?, submit?, stop?, paste_gap_ms?, foreground{pid,name,argv,cwd}?`

- `reason`: `exited, killed, server_stop, host_shutdown, signaled, holder_lost, stopped_by_restart, spawn_pending` (`core/terminals.rs:40-46`, `api-doc:165-173`).
- A kept terminal whose program is lost stays `state: "live"` with `pid: null` and a `reason` until `restarted` (`core/terminals.rs:332-355`).
- `foreground` is what runs now and its live folder; it is the only live cwd. `cwd` stays the start folder; `last_cwd` is private (`core/foreground.rs:11-20`, `core/terminals.rs:406-424`).

### 2.3 terminal.run

Params (`core/api/types.rs:198-286`):

| field | notes |
|---|---|
| `argv: string[]` | program and args. Empty argv and no `line`: that machine's shell (config `shell`, else own shell; on macOS a login shell) (`core/api/run.rs:12-20`, `samples/hc-skill.md:203-206`). **If `argv[0]` is the name of a template, hc runs the template** (`core/api/run.rs:6-15`). |
| `line?: string` | a shell line instead of argv |
| `size?: {cols, rows}` | initial size; default is config `headless`, 200x120 (`core/api/run.rs:105-108`, `core/config.rs:20-21`) |
| `display?: Display` | initial colors and cell size, applied before spawn so the first OSC 10/11/12 and CSI 16 t queries get them (`core/api/run.rs:163-165`, herdr-core PR #88) |
| `name?` | unique among live terminals; else `name_taken` |
| `cwd?` | relative paths join the core's own cwd; with no caller from afar the default is the core's cwd, not home (`core/exec.rs:121-130`, `core/api/run.rs:112-114`). Always send an absolute path. |
| `on?` | another machine |
| `env{}` | added on top of the core's env. hc strips the core's `TERM, TERM_PROGRAM, TERM_PROGRAM_VERSION, COLORTERM`, then defaults `TERM=xterm-256color`, `COLORTERM=truecolor`, and replaces `TERM_PROGRAM` unset or `herdr` with `ghostty` (`core/api.rs:20`, `core/api/run.rs:228-248`). It always sets `HC_TERMINAL_ID`, `HC_TERMINAL_SOCKET`, `PWD` (`core/api/run.rs:212-224`). |
| `labels{}` | empty values are dropped; keys must be non-empty without `=` (`core/terminals.rs:261`, `core/terminals.rs:467-471`) |
| `parent?`, `no_parent`, `no_notify`, `outlive_parent`, `rm`, `keep`, `wait`, `timeout?`, `caller?` | drover sends `keep: true` and no `caller`. Without `parent` or `caller`, the terminal has no parent (`core/api/run.rs:121-131`). |

Result: the whole Terminal record (flattened) plus optional `first_message`, `wait`, `output` (`core/api/types.rs:782-795`). It returns once the program started; a spawn that fails or does not answer in 10 s returns `spawn_failed`; with `keep` the record stays live with `reason: "spawn_pending"`, else it is removed (`core/api/run.rs:180-209`).

What the app sends (`app/hc.rs:438-450`):

```json
{"argv":[...],"cwd":"<abs or empty>","env":{"TERM":"xterm-256color","COLORTERM":"truecolor","TERM_PROGRAM":"herdr"},
 "labels":{"herdr.space":"<space id>","herdr.tab":"<tab id>","herdr.seen":"0"},"on":null,"keep":true,
 "display":{...},"size":{"cols":C,"rows":R}}
```

Labels come from `app/window.rs:2375`. The space and tab ids in labels are local to the target machine (`crate::mirror::local`).

### 2.4 terminal.list, terminal.get

- `terminal.list {all?: bool, labels?: {k: v}}` → `{terminals: Terminal[], hidden_finished, machines?: MachineView[]}` (`core/api/types.rs:298-322`). Live only unless `all`.
- `terminal.get {terminal}` → Terminal, live or finished (`core/api.rs:282-288`).

### 2.5 terminal.kill, terminal.keep, terminal.label

- `terminal.kill {terminal}` → Terminal. Hangup, then terminate, then kill; it **blocks** until the program is gone (`core/api.rs:81-83`, `core/api.rs:290-300`). Call it off the UI path. Kill is final even for kept terminals. Verified: `sh -c "sleep 30"` ended `{state: finished, reason: killed, exit_code: null, signal: 1}`.
- `terminal.keep {terminal, keep: bool}` → Terminal. Only live terminals (`core/api.rs:319-340`). Emits `keep`.
- `terminal.label {terminal, labels: {k: v}}` → Terminal. `k: ""` removes a key (`core/api/types.rs:324-333`, `core/terminals.rs:357-371`). Emits `labels` with the whole map.

### 2.6 terminal.attach and the stream

Params (`core/api/types.rs:375-409`): `terminal`, `cols`, `rows` (the viewer size; required in practice), `observe?`, `resize_notifications?` (observers only), `history?` (scrollback lines before the first paint, main screen only), `display?: Display`, `resumed?` (after a dropped line: take size only once active again), `caller?`.

`Display` (`core/viewers.rs:18-29`): `{fg?: "#rrggbb", bg?: "#rrggbb", cursor?: "#rrggbb", cell?: [w_px, h_px]}`.

Result (`core/api/types.rs:411-422`): `{channel, terminal: Terminal, resize_notifications: bool, automatic_input: bool}`. Verified for a non-observer: `automatic_input: true, resize_notifications: false`.

Errors: `not_found`, `finished` (no screen left), `unavailable` ("lost, waiting for its new shell" or `spawn_pending`), `refused` (a terminal attaching to itself, only detected when `caller` is set), `needs_framed` (`core/api.rs:412-446`, `core/terminals.rs:155-164`).

On the stream (`api-doc:62-111`):

- **First paint**: `CAN (0x18)`, the screen (and `history` lines first), `CSI ?9001h|l`, then the latest OSC 7501 report of each status record (`core/viewers.rs:400-422`). Then the program's raw output, minus terminal queries the core already answered.
- **Input**: DATA on the channel. The core never encodes keys; the client encodes them for the program's modes (`api-doc:68-70`; herdr-core issue #5, closed).
- **Replies the client's VT generates** (DA, mode 2048 in-band resize reports, kitty graphics answers, XTGETTCAP) go as AUTOMATIC_INPUT (kind 6), but only if `hello.features` has `automatic_input` **and** the attach result says `automatic_input: true`; else send DATA (`api-doc:73-83`). Sending kind 6 to an old core loses the input. AUTOMATIC_INPUT does not make the viewer "active" (`core/viewers.rs:254-267`). The app does exactly this (`app/hc.rs:478-496`).
- **Size**: the most recently *active* non-observe viewer sets size and display facts. Active means it attached (unless `resumed`), typed DATA, sent RESIZE, or sent FOCUS 1 (`core/viewers.rs:243-252`, `core/viewers.rs:325-352`, `api-doc:70-72`). With no non-observe viewer left, the terminal goes back to the **headless size (200x120 default)** (`core/viewers.rs:309-330`, `core/runtime.rs:204-205`). Verified: run at 80x24, attach at 100x30, RESIZE 120x40, CLOSE → `terminal.get` reports 200x120. Display facts (colors) stay after the last viewer leaves (`core/viewers.rs:383-393`).
- In-band resize reports (`CSI 48;...t`) generated by the core's screen are dropped; they belong to the viewer, which knows the pixel size (`core/relay.rs:33-51`). After each resize the app takes its VT's replies and sends them as automatic input, because "Mode 2048 programs wait for this report, not SIGWINCH" (`app/terminal.rs:197-207`).
- **FOCUS**: 1/0; forwarded to the program as `CSI I`/`CSI O` only if it enabled mode 1004 (`core/viewers.rs:293-307`).
- **FACTS**: a JSON `Display`; merged field by field into the viewer's facts; applied only if this viewer is the leader (`core/viewers.rs:274-291`).
- **Detach**: send CLOSE with empty payload; the core answers with an empty CLOSE (`core/server.rs:368-371`, verified). `wux/docs/protocol-facts.md:289-290` says no CLOSE comes back; that is wrong for 0.1.18.
- **Program end**: CLOSE with `{"reason","exit_code","signal","finished_ms"}` after the last output is delivered (`core/wire.rs:228-235`, `core/runtime.rs:737-739`). A kept terminal that is **lost** also closes the stream with its reason (`holder_lost`, `signaled`, ...); wait for the `restarted` event and attach again (`api-doc:148-163`).
- **Falling behind**: past 4 MiB queued, the backlog is dropped and replaced by a fresh paint that starts with CAN; a pending RESIZE keeps its place (`core/wire.rs:237-252`, `core/viewers.rs:166-177`). The VT must accept a full repaint mid-stream.
- Kitty graphics: the core's screen eats APC graphics (they move the cursor) and drops its own replies; the bytes pass through to viewers, and "a viewer that can answers" (`core/relay.rs:1-6`, `core/relay.rs:210-225`). A program that sends `a=q` waits for the viewer's answer (`wux/docs/protocol-facts.md:278-280`).

### 2.7 terminal.put

`terminal.put {terminal, name, data: base64}` → `{path, bytes}` (`core/api/types.rs:566-582`). At most 8 MiB raw (`core/put.rs:6`, `core/api.rs:520-528`). Live terminals only (`finished` otherwise). The file lands in `<state dir>/files/<tN>/<clean name>`, with `-2`, `-3` suffixes when taken; the name keeps only `[A-Za-z0-9._-]` and loses leading dots; it is removed with the terminal record (`core/put.rs:36-66`, `core/runtime.rs:775-783`). It types nothing: paste the returned path yourself. The app pastes dropped paths as one bracketed paste of shell-quoted paths (`app/terminal.rs:579-591`; seen on the user's Mac: `'<path>' `). Routed with `machine/t7`, so the path is on that machine.

### 2.8 events.subscribe

Params (`core/api/types.rs:424-437`): `terminal?`, `children_of?`, `labels?{}`. Result (`core/api/types.rs:439-450`): `{seq, terminals: Terminal[], channel}` (framed). The snapshot includes finished records and every connected machine's terminals (`core/events.rs:70-75`). Lines on the channel are JSON-RPC notifications without id, newline-terminated, split across DATA frames anywhere (`app/hc.rs:32-33`). Credit the events channel like any stream; the app credits 1 MiB at once, then each chunk (`app/hc.rs:462`, `app/window.rs:668-671`). Without credit the stream stalls silently after 256 KiB (`wux/docs/protocol-facts.md:70-77`).

```
{"jsonrpc":"2.0","method":"event","params":{"seq":42,"type":"status","terminal":"t7","data":{...}}}
{"jsonrpc":"2.0","method":"heartbeat","params":{"seq":42}}
{"jsonrpc":"2.0","method":"missed","params":{"seq":57,"terminals":[...]}}
```

Event types and how to apply them (`api-doc:125-140`; emit sites in brackets):

| type | data | apply |
|---|---|---|
| `created` | whole record | add (`core/runtime.rs:669`) |
| `restarted` | whole record | replace; reattach (`core/runtime.rs:669`) |
| `lost` | `{reason, exit_code, signal, finished_ms: null}` | stays live; set these, `pid = null`, drop `foreground` (`core/runtime.rs:723`) |
| `finished` | `{reason, exit_code, signal, finished_ms}` | `state = "finished"`, set these, drop `foreground`; a `status` event comes just before (`core/runtime.rs:703-723`) |
| `removed` | `{}` | drop (`core/runtime.rs:779`) |
| `keep` | `{keep}` | set (`core/api.rs:330`) |
| `status` | `{status, records, reported}` | replace all three (`core/viewers.rs:219-225`) |
| `turn` | `{turn, state}` | set `turn`; comes after `status` (`core/viewers.rs:227-230`) |
| `title` | `{title}` | set (`core/viewers.rs:185-189`) |
| `labels` | `{labels}` + `record` when the change makes it newly match your filter | replace labels; with `record`, add it; with a label filter, a terminal that stops matching still gets this event, then leaves your view (`core/api.rs:342-355`, `core/events.rs:105-112`) |
| `size` | `{cols, rows}` | set (`core/viewers.rs:352`) |
| `foreground` | `{pid, name, argv, cwd}` or `null` | set; polled every 500 ms (`core/foreground.rs:153-175`) |
| `doc` | `{name, version}`, no `terminal`, plus `machine` for a remote doc | re-read with `doc.get` (`core/events.rs:121-128`, `core/remote.rs:471-478`) |

Rules:

- `seq` rises by one per event. Drop events with `seq <= ` the last applied or snapshot seq. `seq` restarts with each core process: take a new snapshot after reconnect (`api-doc:171-177`). The app's fold: `app/hc.rs:80-165`. pocketty's fold: `pkt/hc/src/terminals.rs:54-196`.
- `heartbeat` every 15 s when nothing is queued (`core/server.rs:269-288`).
- `missed` + fresh snapshot replaces a backlog for a subscriber that fell behind (`core/events.rs:147-155`). Plain-mode queue: 1024 lines (`core/events.rs:13-14`).
- **Doc events go only to unfiltered subscribers.** A `terminal`, `children_of` or `labels` filter excludes them (`core/events.rs:34-41`, `core/events.rs:121-128`, `api-doc:195-204`). Verified: a `labels: {x: "1"}` subscription got no `doc` event; an unfiltered one did. drover must keep one unfiltered subscription.
- Snapshots carry terminals only, never docs. Read docs after subscribing, after reconnecting, after `missed`, and when a remote machine reconnects (that is reported locally as `missed`) (`api-doc:206-212`, `core/events.rs:172-202`).
- A remote machine coming back re-sends its changed terminals as `created` and the gone ones as `removed`; forgetting a machine removes its terminals (`api-doc:142-146`).

Verified event order for run → attach 100x30 → RESIZE 120x40 → CLOSE → label → kill: `created`, `size 100x30`, `size 120x40`, `foreground`, `size 200x120`, `labels`, `finished`.

### 2.9 Agent status: what it is and where it comes from

- Status comes **only** from the program's own OSC 7501 reports (Program Status Protocol). hc has no screen detection and no agent list (`herdr-core/AGENTS.md:34-36`, `herdr-core/README.md:116-120`, `api-doc:214-219`). A program that never reports has `status: null`, `reported: false`. Herdr TUI 0.9.5 detects agents from the screen and with hooks (`herdr/src/detect/`, `herdr/src/integration/claude_settings.rs`); under hc only programs that emit 7501 have a status. This is a parity gap, not something drover fixes.
- `status.state` words: `idle, working, done, blocked, error` (stored as reported). `kind`: `permission | question | auth` (blocked only). `progress` 0-100. `app` e.g. `claude-code`, `codex`, `pi` (`core/status.rs:12-29`, `samples/herdr.d.ts:168`). `records` holds sub-records by id (`core/status.rs:98-120`).
- `turn` goes up each time the root status leaves `working` (`core/status.rs:72-82`).
- The app's displayed word (`app/model.rs:527-536`): `working` → working; `blocked` or `error` → blocked; otherwise **done** when `turn > herdr.seen` label (or, with no label, when the person has not looked since it stopped), else **idle**. Rollup for a tab or space: blocked > done > working > idle (`app/model.rs:91-100`, `app/model.rs:553-556`, `samples/herdr-app-AGENTS.md:150-153`).
- Shared seen state is the `herdr.seen=<turn>` label; pinned is `herdr.pinned=1`. Rules: write `herdr.seen` only to raise it, once per turn, only while a person really looks; decide on the `turn` event; "mark unread" writes `turn - 1`; a terminal a client starts gets `herdr.seen=0`. A missing label is not 0 (`herdr-app/docs/labels.md:16-61`). App writer: `app/window.rs:1516-1545`.
- Bells and notifications: **hc has no bell or notification event.** The app makes its "needs you" and "finished" toasts from status transitions (`app/model.rs:481-503`, `app/window.rs:2110-2120`). BEL and OSC 9/777 exist only inside an attach stream, so drover sees them only for attached panes.
- Title: `title` event (from the core's screen, OSC 0/2). Live folder: `foreground.cwd` via `foreground` event. There is no OSC 7 cwd tracking in hc.

### 2.10 doc.get / doc.set

- `doc.get {name, on?}` → `{value, version}`; never written: `{value: null, version: 0}` (`core/api/types.rs:153-176`, `core/api/doc.rs:4-7`).
- `doc.set {name, value, version, on?}` → `{version}`. Compare-and-swap: `version` must equal the current one, else kind `conflict` with message `document "herdr-app" is at version N, not M` (verified) and hint "read doc.get and retry with its version" (`core/api/doc.rs:9-22`). Size limit: 1 MiB of serialized JSON, else `invalid_params` (`core/api/doc.rs:11-13`). `null` deletes the value but keeps the version (`api-doc:181-186`).
- The core writes all docs atomically to `<state>/docs.json` and emits a `doc` event (`core/api/doc.rs:23-38`). Docs survive restart and handover; a broken file is renamed `docs.json.bad` (`api-doc:188-193`). The core never reads the content.
- Doc names follow terminal name rules: `Herdr App` is rejected with `invalid_params` (verified).
- With `on`, the call is forwarded to that machine's core; that machine's doc changes arrive in the local stream as `doc` events with `machine` (`core/remote.rs:471-478`).

### 2.11 Other calls drover may use

- `machine.exec {argv | line, on?, cwd?, timeout?, input?}` → exit code and base64 output (`core/api.rs:113-115`). The app runs `git -C <cwd> rev-parse --show-toplevel --abbrev-ref HEAD` this way on remote machines to name spaces (`app/window.rs:4561-4581`). The Files mod lists folders with `exec` on the machine (`herdr-app/defaults/mods/files/main.js:15-40`).
- `server.status` → `{core, version, pid, uptime_ms, holder, holders, live, finished, installed_by}` (`core/api/types.rs:135-151`).
- `terminal.read {terminal, lines}` for a text preview without attaching (`core/api.rs:357-410`).

---

## 3. The `herdr-app` doc

### 3.1 Where it lives

One doc named `herdr-app` **per machine**, in that machine's core. Each machine's doc holds only the spaces whose terminals run on that machine (`samples/herdr-app-AGENTS.md:94-98`, `app/mirror.rs:89-113`). The app reads the local one with no `on`, and each connected, non-temporary machine's with `on: <machine name>` (`app/mirror_sync.rs:74-84`).

### 3.2 Schema, exactly as the app parses it

Doc root (`app/mirror.rs:9-36`):

```rust
struct Document { #[serde(default)] spaces: Vec<SharedSpace> }
#[serde(default)] struct SharedSpace { id: String, name: String, folder: String, tabs: Vec<SharedTab> }
struct SharedTab { #[serde(default)] id: String, #[serde(default)] name: String,
                   #[serde(default = "empty_root")] root: Node }   // empty_root = {"id":"","type":"row","children":[]}
```

- Every space field may be missing (defaults `""` / `[]`); `{}` parses as no spaces (`app/mirror.rs:297-305`).
- **No `deny_unknown_fields`, and no catch-all.** Unknown fields are dropped when parsed, and the app writes the whole doc from its model, so any field the app does not know is lost at its next write (`app/mirror.rs:89-113`, `app/window.rs:1835-1838`). drover must not store anything extra in this doc.
- **A value the app cannot parse breaks that machine's layout in the app.** `doc.get` result → `serde_json::from_value::<Document>` → on error `Outcome::Undecodable`, the slot becomes unavailable, and structural commands fail with "Layout unavailable" (`app/mirror_sync.rs:74-84`, `app/window.rs:1929-1944`, `app/window.rs:2353-2362`).

`root` is a full `layout::Node` (`app/layout.rs:315-381`). Fields:

| field | type | default / meaning |
|---|---|---|
| `id` | string | `""` |
| `type` | `"block" \| "row" \| "column" \| "stack"` | **required** (serde tag) (`app/layout.rs:272-291`) |
| `content` | Content | `block` only, required there |
| `children` | Node[] | `row`, `column`, `stack` |
| `active`, `header` | | `stack` only |
| `size` | `{"weight": f}` or `{"px": f}` | absent = weight 1 (`app/layout.rs:79-84`, `app/window/pane_size.rs:31-37`) |
| `min`, `max` | px | |
| `hidden` | bool | false |
| `title`, `background`, `border`, `radius`, `shadow`, `padding`, `focus`, look fields (`theme, colors, font, font_size, ui_font, terminal_padding, terminal_opacity`), `gap`, `divider`, `movable`, `closable`, `resizable`, `floating`, `titlebar`, `traffic_lights`, `preview`, `terminal_menu`, `window_drag`, `cloned_from` | | layout-file features; the app does not put them in tab trees it creates. drover reads and keeps them unchanged; `closable: false` means pane.close does nothing (`app/window.rs:2993-2995`). |

Content is untagged; the first variant that matches wins (`app/layout.rs:10-31`):

| JSON | kind | in docs? |
|---|---|---|
| `{"terminal": "t12"}` | Terminal | yes: a terminal pane |
| `{"mod": "files", "props": {...}}` | Mod (`props` omitted when null) | yes: a mod pane (`pane.split {mod}`, `tab.new {mod}`) |
| `{"tab": "active", "pane": {...}}` (alias `context`) | Context | layout files only |
| `{"agent": "focused"}` | Agent | layout files only |

Mod names the app ships: `agents changes confirm files help machine-add menu new-menu pane peek picker prompt spaces tabs terminals-bar terminals-pane toast win98 workspace-chrome workspace-status worktree` (`herdr-app/defaults/mods/`). Any name is valid; users add their own in `mods/`. Mod `props` are stored as given and never machine-qualified (`app/mirror.rs:243-253`, `samples/herdr-app-AGENTS.md:100`). The Files mod shows the focused pane's live folder (else the space folder) on the space's machine (`herdr-app/defaults/mods/files/main.js:5-11`).

### 3.3 What the app writes

- Ids (`app/model.rs:166-174`): `<prefix><16 hex><16 hex>` from random hashers. Prefixes: `sp` space, `tb` tab, `p` pane block, `s` split container (`app/window.rs:2428`, `app/window.rs:2448`, `app/window.rs:2482`, `app/model.rs:287`). Ids must be unique across the whole doc: lookups search every tab (`app/model.rs:226-238`).
- A tab root is a `row` whose `id` equals the tab id, with the panes as children (`app/model.rs:102-119`). The root never collapses, even with one child (`app/layout.rs:856-859`).
- Axis: `row` = side by side (split right); `column` = stacked (split down) (`app/window.rs:2979-2981`).
- Split (`app/layout.rs:577-608`): if the target's parent runs the same axis, insert the new pane after the target and give both half the target's weight (absent = 1). Else replace the target by a new container (id = fresh `s…`) holding `[target, new]`; the container takes the target's slot size and both children have no size.
- Remove (`app/layout.rs:564-570`, `app/layout.rs:656-709`): the removed weight goes to the previous sibling (else the next); rows/columns left empty vanish; a row/column with one child is replaced by that child, which takes its slot size; stacks and the root are kept.
- Divider drag (`app/window.rs:4276-4285`): px sizes stay px; weights are rescaled so the pair keeps its sum. `layout.resize {id, px|weight}` sets one node (`app/window.rs:3267-3275`).
- Sample `samples/herdr-app-doc.json` (version 58) matches: 4 spaces with `id: sp+32hex`, `folder` absolute, `name` `""` or free text (`tidy`, `rdna/www`); tabs `tb+32hex` with `name ""`; each root `{"type":"row","id":<tab id>,"children":[{"type":"block","id":"p+32hex","content":{"terminal":"t12"}}]}`; no `size` fields; addresses such as `t1h`, `t1m`. Keys are sorted (serde_json map order); the app writes compact JSON in `doc.set`.

### 3.4 Names

- Space: `name` if non-empty; else the folder name of its first terminal's checkout: `git -C <cwd> rev-parse --show-toplevel` (run on the space's machine), else that pane's live folder, else the space's `folder`. Folder name = last path part; `~` for the home folder or an empty path (`app/model.rs:241-252`, `app/model.rs:565-578`, `app/window.rs:4561-4581`). The cwd used is `foreground.cwd`, else the pid's cwd (local only), else the record's `cwd` (`app/window.rs:2232-2242`). An unnamed space follows its first terminal as it moves (`samples/herdr-app-AGENTS.md:108-115`).
- Tab: `name` if non-empty, else its 1-based position in the space (`app/model.rs:254-260`).
- Renames are trimmed; renaming to `""` returns to the automatic name (`app/window.rs:2881-2886`, `app/window.rs:2934-2939`). No length or character limit.

### 3.5 Space folder

`folder` is where the space was made (`app/model.rs:27-30`). For `space.new`: the given folder, else the focused pane's live cwd if it is on the same machine, else home; for a remote space with no folder: `""`, which means that machine's home (`app/window.rs:2420-2437`). New tabs start in the live cwd of the focused pane of the space's shown tab, else `folder` (`app/window.rs:2347-2351`). Splits start in the target pane's live cwd, else `folder` (`app/window.rs:2473-2492`). An adopted terminal's new space gets its live cwd as `folder` (`app/window.rs:2193-2200`).

### 3.6 Labels on terminals

| label | value | who writes | source |
|---|---|---|---|
| `herdr.space` | space id, local to the terminal's machine | at `terminal.run`; on adopt/keep | `app/window.rs:2375`, `app/window.rs:2580-2586` |
| `herdr.tab` | tab id, local | same | same |
| `herdr.seen` | last seen turn | `"0"` at run; then raised while looked at | `herdr-app/docs/labels.md:16-46` |
| `herdr.pinned` | `"1"` or removed | pin command | `herdr-app/docs/labels.md:54-61` |
| `where` | `"<space display name> / <tab display name>"` | the app, for every live terminal it shows; sub-agents get their parent's; written only when the computed value changes | `app/window.rs:1774-1827` |

`keep: true` is set at run and on adoption (`app/hc.rs:440`, `app/window.rs:2164-2166`). Park removes `herdr.space` and `herdr.tab` and sets keep off (`app/window.rs:2586`). The app also reads an optional `herdr.right-click=program` label (`app/window.rs:2000`, written at `app/window.rs:3095`).

### 3.7 How the app reacts

- **Reading** (`app/window.rs:1906-1990`): on a `doc` event, `missed`, reconnect, or a machine coming back, it reads the doc. If the version differs, it replaces that machine's spaces in its model with the doc (`app/mirror.rs:115-139`), then puts back its private view (active space, active tab per space, focus, previous focus, zoom, settled) where the ids still exist (`app/mirror.rs:196-216`).
- **Writing** (`app/window.rs:1829-1861`): after each change it captures its model as a Document; if that differs from the last doc it read or wrote, it sends `doc.set` with that version. One request per machine at a time, no queue (`app/mirror_sync.rs:1`).
- **Conflict: no merge.** On `conflict` it reads the current doc and replaces its model with it; its own change is dropped, with the toast "Layout changed elsewhere / The other screen's layout is shown." (`app/mirror_sync.rs:99-102`, `app/window.rs:1928`, `app/window.rs:1954-1960`, `samples/herdr-app-AGENTS.md:97-99`). So every drover write that lands between an app read and an app write throws away the app user's edit.
- **Private, never in the doc**: focus, previous focus, zoom, active space, active tab, settled, recent spaces, seen attention (`app/mirror.rs:38-53`, `app/mirror.rs:228-242`, `samples/herdr-app-AGENTS.md:135`).
- **Space order**: local machine first, then machines by name; inside a machine, doc order (`app/model.rs:176-191`).
- **Terminals not in the doc** (`app/window.rs:2122-2152`, `app/window.rs:2154-2224`, `app/window.rs:4771-4777`): a live terminal with a non-empty `herdr.space` label and no pane is adopted, if that machine's doc is available. The app re-reads it with `terminal.get`, checks it is still live with the same labels, turns keep on, then: if the space is unknown, makes a new space with that id, `name ""`, and folder = its live cwd; else if `herdr.tab` names a known tab, splits that tab's last pane (row) without moving focus; else adds a tab (label id, or fresh `tb…` id). Terminals without `herdr.space`, and other machines' terminals not started for a space on that machine, stay out (`samples/herdr-app-AGENTS.md:136-149`).
- **Doc panes whose terminal is not live**: `terminal.get` each; if `reason` is `exited` or `killed`, or the record is `not_found`, the pane is removed (5 s grace for just-started terminals) (`app/window.rs:2133-2138`, `app/window.rs:2509-2541`). A stream CLOSE with reason `exited`, `killed` or `finished` also removes the pane at once (`app/window.rs:2290-2320`). Lost kept terminals keep their pane.
- **Closing**: pane.close / tab.close / space.close remove the structure and `terminal.kill` its terminals and their non-kept children, recursively (`app/window.rs:2867-2881`, `app/window.rs:2922-2933`, `app/window.rs:2990-3002`, `app/window.rs:2650-2683`). Closing the last pane closes the tab; the last tab closes the space.
- **Attaching**: the app attaches every terminal pane in every tab, not only visible ones, and sizes hidden tabs as their pane would be (`app/window.rs:1388-1431`, `app/terminal.rs:560-563`). It detaches only when the pane leaves the model. Attach params: `history: 2000` and `display` (`app/hc.rs:466-482`).

---

## 4. pocketty `crates/hc` and `crates/terminal`

### 4.1 `pocketty-hc` (`pkt/hc/`)

- Size: 1,164 lines of source plus 672 of tests (`client.rs 316, terminals.rs 365, model.rs 282, wire.rs 190, stream.rs 166, error.rs 30, lib.rs 15`).
- Deps (`pkt/hc/Cargo.toml`): `bytes 1.12`, `futures-util 0.3 (sink)`, `serde 1 (derive)`, `serde_json 1`, `thiserror 2`, `tokio 1.53 (rt, sync, io-util, macros)`, `tokio-util 0.7 (codec)`. Dev: `pocketty-terminal`, `tempfile`, tokio `process, time`. Edition 2024, rust-version 1.95, workspace lints (`unsafe_code = "deny"`, clippy all deny, pedantic warn) (`pocketty/Cargo.toml:6-21`).
- Model: **async, tokio**. `Client::connect(io)` takes any `AsyncRead + AsyncWrite + Send + 'static`, writes the plain hello `{"generation":1,"framed":true}`, reads the reply line, then `tokio::spawn`s one writer task and one reader task (`pkt/hc/src/client.rs:107-142`). Needs a tokio runtime (current-thread is enough). `Client` is `Clone` (Arc inside). It has no socket or `hc connect` helper; callers spawn `hc connect` (`pkt/daemon/src/hc.rs:26-44`).
- Public API (`pkt/hc/src/lib.rs:8-15`):
  - `Client::hello() -> &Hello`, `is_closed()`, `call<T: DeserializeOwned>(method, params: Value) -> Result<T, HcError>` (generic; no typed wrappers for run, kill, label, keep, doc, put), `attach(&AttachOptions) -> Attach`, `subscribe(&Filter) -> (Snapshot, Events)` (`pkt/hc/src/client.rs:144-159`, `pkt/hc/src/stream.rs:146-165`).
  - `AttachOptions {terminal, cols, rows, history, observe, resumed, display}` (`pkt/hc/src/stream.rs:12-24`). No `resize_notifications`.
  - `Attach { pub terminal }`: `next() -> Option<Output>` (`Bytes` or `Closed(Ending)`), `write(bytes)`, `resize(cols, rows)`, `focus(bool)`, `facts(&Display)` (`pkt/hc/src/stream.rs:50-83`). Dropping it sends CLOSE (`pkt/hc/src/client.rs:310-316`). CREDIT is sent when a chunk is *received*, not when it is rendered (`pkt/hc/src/client.rs:292-303`).
  - `Events::next() -> Notice` (`Event | Heartbeat | Missed`) (`pkt/hc/src/stream.rs:102-139`); `Event {seq, terminal, machine, change: Change}` with every documented type plus `Unknown` for future ones (`pkt/hc/src/model.rs:112-209`).
  - `Terminals`: a fold of snapshot + notices with `seq` dedupe and label-filter removal; `apply()` returns `Update::{Nothing, Changed, Removed, Reset, Doc{machine,name,version}}` (`pkt/hc/src/terminals.rs:1-137`).
  - `Terminal`, `Status`, `Foreground`, `Display`, `Ending`, `Hello`, `CoreInfo` serde types (`pkt/hc/src/model.rs:6-110`). `Terminal` needs `address, state, argv, cwd, cols, rows, created_ms`.
  - `HcError::{Io, Wire, Closed, Json, Remote{kind,message,hint}, NoChannel}`, `kind()` (`pkt/hc/src/error.rs`).
- Gaps for drover:
  1. No AUTOMATIC_INPUT (kind 6) in `Kind`; replies would go as DATA and count as activity (`pkt/hc/src/wire.rs:9-42`).
  2. An unknown frame kind from the core is a decode error that ends the connection (`pkt/hc/src/wire.rs:115`). Today the core only sends 0, 2 and (to observers that asked) 3.
  3. Core → client RESIZE frames are ignored (`pkt/hc/src/client.rs:253`).
  4. Frames for a channel whose reply is not yet routed are dropped (see 1.5); matters for `subscribe`.
  5. No call timeout (the app uses 10 s, `app/hc.rs:412-434`).
- Tests (`pkt/hc/tests/`): live tests need `hc` on PATH; each starts an isolated core in a `/tmp/hc*` tempdir (`pkt/hc/tests/common/mod.rs:22-82`). `attach.rs`: hello, typed bytes and exit CLOSE, first paint carries modes, history, stalled stream does not block calls, attach to finished. `events.rs`: status order and details, blocked ends a turn, last active viewer sets size, kept terminal comes back after a signal, folding events equals `terminal.list`. Unit tests for the codec and event parsing (`pkt/hc/src/wire.rs:142-189`, `pkt/hc/src/model.rs:224-281`). No test covers `doc.*`.
- Copy as is? Yes for the code (the user owns it; pocketty has no LICENSE file), with crate rename and the gaps above fixed in drover. It drags no pocketty-internal deps except the test dev-dep on `pocketty-terminal`.

### 4.2 `pocketty-terminal` (`pkt/terminal/`)

- Size: 592 lines (`term.rs 235, input.rs 202, frame.rs 129, theme.rs 18, lib.rs 8`) plus 191 lines of tests and a binary fixture `tests/fixtures/herdr-frames.bin`.
- Deps (`pkt/terminal/Cargo.toml`): `libghostty-vt = { version = "0.2.2", default-features = false }` (so **no kitty-graphics feature**; crates.io default enables it), `pocketty-screen` (path dep, `pkt/screen/`, 312 lines, no deps), `thiserror 2`. To copy it, copy `pocketty-screen` too, or render straight from libghostty.
- libghostty-vt 0.2.2 is the newest on crates.io (checked with `cargo info`). Its `-sys` crate builds libghostty-vt from Ghostty source with **zig** at build time: it `git clone`s `ghostty-org/ghostty` at commit `a887df42c56f6de86c0fe6da9c4eeca37931e083` into `OUT_DIR` (needs `git` and network), unless `GHOSTTY_SOURCE_DIR` points at a checkout or `GHOSTTY_ZIG_SYSTEM_DIR` at a pre-fetched zig cache (`pocketty/vendor/libghostty-vt-sys/build.rs:6-7`, `build.rs:112-191`, `build.rs:355-395`, `README.md:5-23`). CPU defaults to `baseline`; override with `LIBGHOSTTY_VT_SYS_CPU`; optimize with `LIBGHOSTTY_VT_SYS_OPTIMIZE`. pocketty pins zig `0.15.2` in `pocketty/mise.toml` (`MISE_ZIG_VERSION` too). workbox has mise zig 0.15.2 and 0.16.0 installed, none global.
- pocketty patches `libghostty-vt-sys` (`[patch.crates-io]`, `pocketty/Cargo.toml:23-24`). Diff against crates.io 0.2.2: only iOS CPU defaults (`apple_a12`, `apple_a14`) and iOS zig targets (commit `b88aa40`: "libghostty-rs has no iOS targets, and its baseline CPU fails on simdutf NEON code"). drover (Linux, macOS) can use the crates.io crate unpatched. Not verified here: a build on `aarch64-apple-darwin` with `baseline` CPU.
- Threading: `Terminal` holds `libghostty_vt::Terminal<'static,'static>` and `Rc<RefCell<…>>` callbacks, so it is **not `Send`** (`pkt/terminal/src/term.rs:30-45`). Keep all VTs on one thread (the UI thread) and move bytes to it.
- API (`pkt/terminal/src/term.rs`):
  - `Terminal::new(cols, rows, Theme) -> Result<Self, TermError>`; scrollback 10,000 lines (`term.rs:15`, `term.rs:48-69`). `Theme {foreground, background, cursor}` sets VT defaults (`theme.rs`).
  - `feed(&[u8])`, `resize(cols, rows, cell_px)`, `size()`, `frame() -> Frame`, `take_replies() -> Vec<u8>` (VT replies to send back to the program, as automatic input), `take_events() -> Vec<Event>` (`Bell`, `Title(String)`, `Clipboard(String)` from OSC 52), `title()`, `wants_mouse()`, `scroll(rows)`, `scroll_to_bottom()`, `scrolled_back()` (`term.rs:71-117`, `term.rs:203-227`).
  - Keys: `key(&KeyInput) -> Vec<u8>`. `KeyInput {code, text, modifiers, action}` where `code` is a **W3C `KeyboardEvent.code` string** (`KeyA`, `Digit7`, `Enter`, `ArrowUp`, `F1`..`F12`, punctuation names). It maps to libghostty `key::Key` plus the unshifted codepoint, then `key::Encoder::set_options_from_terminal(&vt)` encodes for the program's current modes (legacy, kitty keyboard, modifyOtherKeys). An unknown `code` falls back to sending `text` as is (`term.rs:119-145`, `input.rs:66-178`). drover gets crossterm `KeyEvent`s, so it needs a crossterm → `code` map (no numpad, media keys, or F13+ today).
  - `paste(&str)` wraps in bracketed paste when mode 2004 is on (`term.rs:147-160`). `mouse(&MouseInput)` encodes for the current tracking mode using `cell_px` (`term.rs:162-200`).
- Cell access (`pkt/terminal/src/frame.rs:26-96`): `RenderState::update(&vt)` → row iterator → cell iterator. Per cell: `raw_cell().wide()` (narrow, wide, spacer), `style()` (bold, italic, faint, blink, inverse, invisible, strikethrough, overline, underline kind, underline color), `fg_color()` / `bg_color()` as resolved RGB, `graphemes_utf8()`. Cursor: visible, viewport x/y, visual style, blink. The `Frame` it builds has **RGB colors only**: default fg becomes the theme RGB and palette indexes are resolved (`frame.rs:52-58`). For a TUI that should follow the host terminal's palette, read `cell.style().fg_color` / `bg_color`, which are `StyleColor::{None, Palette(i), Rgb}` (`libghostty-vt-0.2.2/src/style.rs:31-44`), and map them to ratatui `Reset`, `Indexed(i)`, `Rgb`.
- Tests (`pkt/terminal/tests/terminal.rs`): real Herdr frames, theme defaults, SGR mapping, DECSCUSR cursor shape, host query replies, title/bell/clipboard events, key encoding per mode, kitty keyboard protocol, soft keyboard text, bracketed paste, mouse only when tracking, local scrollback, resize reflow.
- Copy as is? Yes, with `pocketty-screen`, or keep `term.rs` and `input.rs` and replace `frame.rs` with a ratatui-buffer writer that keeps palette indexes. Build needs zig 0.15.2 + git + network on every clean build (CI cache matters).

---

## 5. Pitfalls

### 5.1 hc issues named in plan.md

- **herdr#5814** (in `herdrdev/herdr`, not herdr-core; open; hc 0.1.14 and 0.1.18): after a smaller client detaches, the pty goes back to its size but the core's screen keeps the client's size, so `terminal.read` and new first paints are wrong and nvim draws stale cells. Combined with the verified rule that the last detach returns the terminal to the headless 200x120, every detach of a lone viewer is a resize of the program.
- **herdr-core#111** (open, hc 0.1.17, darwin/arm64): `hc attach --observe` on a busy terminal holds ~99% of a core per stream for as long as it is attached; idle is ~2%. It scales per attach on the owning machine. Avoid many `observe` attaches (followcat keeps up to 32 off-tab observers, `fol/docs/layout.md:41-47`).
- **herdr#5710** (open, hc 0.1.15): terminals answer OSC 10/11 with a fixed dark palette until a viewer gives display facts. Since 0.1.18, `terminal.run` takes `display` (PR #88, `core/api/run.rs:163-165`); also send `display` on attach and FACTS when the host colors are known. Display facts persist after detach (`core/viewers.rs:383-393`).
- **herdr-core#25** (open, no maintainer reply): request for an official hc TUI.
- **herdr-core#5** (closed): keys stay encoded by the client; the core never encodes (`api-doc:68-70`).
- macOS TCC (plan.md): `terminal.put` files land in hc's state dir, not in Desktop/Documents/Downloads, so pasted images are not affected (`core/put.rs:36-39`).

### 5.2 From the community projects (as maps, not code)

1. Credit every DATA frame on every stream, the events channel included; a missing credit stalls silently (`wux/docs/protocol-facts.md:70-77`).
2. A stream can contain a full repaint at any time (starts with CAN after falling 4 MiB behind or after a resize for observers); the first paint can be an empty screen that still resets everything (`wux/docs/protocol-facts.md:76-77`, `wux/docs/protocol-facts.md:258-260`).
3. Use CLOSE `reason` to tell a program exit (`exited`, `killed`) from a core-side end (`server_stop`, `holder_lost`, ...); `exit_code` can be null in both (`wux/docs/protocol-facts.md:262-272`).
4. Do not answer DA, DSR, OSC 10/11/12: the core answers them and strips them from the stream. Do answer what passes through, kitty graphics `a=q` included, or the program waits (`wux/docs/protocol-facts.md:178-191`, `wux/docs/protocol-facts.md:278-280`, `core/relay.rs:1-6`).
5. The first paint does not carry earlier DECSCUSR cursor shape, or OSC 10/11/4 color overrides the program set before the attach (`fol/docs/parity.md:123-190`). A pane's cursor shape and recolors are lost on every reattach until the program sets them again.
6. A first paint while the program is on the alternate screen replays only that screen (`wux/docs/protocol-facts.md:282-283`).
7. Two non-observe viewers of one terminal fight over its size; the last active wins, and the core does not say who (`wux/docs/protocol-facts.md:130-141`, `fol/docs/runtime.md:29-34`).
8. Holders survive a killed core; stop test cores only with `hc server stop` (`fol/docs/integration.md:41-48`).
9. Older records may carry `status` as a plain string; accept both (`wux/docs/protocol-facts.md:312`, `sut/src/core_native/discovery.rs:49-55`).
10. wuxiaobai24 "keeps unknown fields" in the shared doc and does three-way merges (`wux/docs/sync-contract.md:7`, `wux/docs/herdr-app-alignment.md:10-17`), but the app drops unknown fields on its next write (3.2), so that guarantee lasts only until the app writes. followcat never writes `herdr-app` at all and keeps its own doc (`fol/docs/layout.md:1-28`), so its layouts never reach the app.
11. Several facts in `wux/docs/protocol-facts.md` are wrong for 0.1.18: hello mismatch kind (it is `unsupported_generation`), viewer CLOSE gets no reply (it gets an empty CLOSE), `resize_notifications` only appears in results (it is also an attach param).

### 5.3 Found in the source while writing this

1. `argv[0]` matching a template name runs the template (`core/api/run.rs:6-15`). For "new shell" send empty argv.
2. Relative or missing `cwd` resolves against the core's cwd, not the caller's or home (`core/exec.rs:121-130`). Send absolute paths; for remote spaces, the space `folder` or the remote home from `machine.exec`.
3. A label-filtered subscription never sees doc events (2.8).
4. Detaching the last non-observe viewer resizes the program to 200x120 (2.6). The app keeps every pane attached (3.7), so while the app runs, its size wins anyway.
5. If drover runs inside an hc terminal (`HC_TERMINAL_ID` set and `HC_TERMINAL_SOCKET` equal to its socket), attaching to that terminal shows its own output forever; the core refuses only when `caller` is sent (`core/api.rs:421-430`, `core/client.rs:260-266`).
6. `terminal.kill` and `terminal.list` (3 s per remote machine) can block; run them off the input/render path.
7. Any content kind or node `type` the app cannot parse makes the whole machine's layout unavailable in the app (3.2).

---

## 6. Biggest risks for drover

1. **Doc write races lose app edits.** The app does not merge: a drover write between an app read and an app write makes the app drop its own change and show "Layout changed elsewhere" (3.7). drover must write only on real user actions, never to normalize, and never write fields or kinds the app does not parse.
2. **Hidden panes not attached → size churn.** plan.md says panes that are not visible are not attached. With no other viewer, each detach resizes the program to 200x120 and each reattach resizes it back; with herdr#5814 the core's screen can then be stale. Options: keep all panes in the current tab (or all tabs, like the app) attached and sized, or set `headless` in hc config, or accept the churn. Avoid `observe` as the workaround (herdr-core#111).
3. **Status parity.** Only programs that emit OSC 7501 have a status; Herdr TUI 0.9.5 detects agents itself. drover will show less agent state than Herdr TUI for agents that do not report.
4. **Kitty graphics pass-through** needs drover to pull APC `_G` sequences out of each stream, answer `a=q`, and place images on the host terminal with pane offsets and clipping; pocketty-terminal builds libghostty without kitty-graphics.
5. **Build chain**: libghostty-vt-sys needs zig 0.15.2, git and network at build time; aarch64 macOS with the `baseline` CPU is not verified.
6. **pocketty-hc gaps**: AUTOMATIC_INPUT, early-frame buffering for `subscribe`, call timeouts, typed wrappers.
