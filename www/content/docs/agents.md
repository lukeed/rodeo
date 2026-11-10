---
title: Agents
summary: An agent is a program in a pane that reports its state to `hc`. rodeo shows that state, and tells you when an agent finishes or needs you.
group: Using rodeo
order: 4
---

## What counts as an agent

A program becomes an agent when it reports its state to `hc` with OSC 7501, the Program Status Protocol. `hc` keeps the state with the terminal, so every client sees the same thing: rodeo, the Herdr desktop app, and `hc get`.

rodeo does not read the screen to guess. A program that does not report has no state, and does not show in the agent list. Herdr TUI found some agents on the screen, so it can show more of them.

## States

| State | Means |
| --- | --- |
| <span style="color:var(--red)">●</span> blocked | It needs you: a permission, a question or a login. An error shows as blocked too. |
| <span style="color:var(--yellow)">●</span> working | It is on a turn. |
| <span style="color:var(--teal)">●</span> done | It finished a turn that you did not see yet. |
| <span style="color:var(--green)">○</span> idle | It finished, and you saw it. |

An agent turns from done to idle when you look at its pane. `hc` keeps that as the `herdr.seen` label, so the Herdr app sees it too.

A space or a tab shows the most urgent state of its agents: blocked, then done, then working, then idle. Set `ui.status_indicators = "symbols"` for `×` `✓` `◐` `○` in place of dots.

## The agent list

The sidebar lists every agent under the spaces, with its state, machine, space and tab. Click the label at the top right to sort it:

- `grouped` keeps the order of spaces, tabs and panes.
- `priority` puts the most urgent first, then the most recent change.

`[ui.sidebar.agents]` sets what each row shows, and `rows_by_agent` gives one agent its own rows. Drag the line above the list to give it more or less room.

| Keys | Does |
| --- | --- |
| <kbd>prefix+g</kbd> | Find an agent by name. In the list, <kbd>b</kbd> <kbd>w</kbd> <kbd>i</kbd> <kbd>d</kbd> show only blocked, working, idle or done agents, and <kbd>a</kbd> shows all. |
| `previous_agent` / `next_agent` | Step through the agent list. Not bound by default. |
| `focus_agent` | Go to agent 1 to 9, with a bind such as `prefix+alt+1..9`. |

## Toasts

A toast tells you when an agent in the background finishes or needs you. Toasts are off by default. Turn them on in `[ui.toast]`, or with <kbd>prefix+s</kbd>:

```toml [~/.config/rodeo/config.toml]
[ui.toast]
delivery = "herdr"
delay_seconds = 1
```

| `delivery` | Shows |
| --- | --- |
| `off` | Nothing. |
| `herdr` | A toast in rodeo, at `ui.toast.herdr.position`. |
| `terminal` | A notice from your terminal, in Ghostty, iTerm2, WezTerm and kitty. |
| `system` | A notice from your system: `terminal-notifier` or `osascript` on macOS, `notify-send` or `gdbus` on Linux. |

rodeo waits `delay_seconds`, then shows the toast only if the agent is still in that state. There is no toast for the tab you look at. <kbd>prefix+o</kbd> goes to the pane of the last toast.

## Sounds

rodeo plays a short chime when an agent in the background finishes or needs you. Sound is on by default.

```toml [~/.config/rodeo/config.toml]
[ui.sound]
enabled = true
done_path = "sounds/done.mp3"       # relative to the config folder
request_path = "sounds/request.mp3"

[ui.sound.agents]
droid = "off"
```

On macOS rodeo plays with `afplay`. On Linux it tries `paplay`, `pw-play`, `aplay`, `ffplay`, `mpg123` and `mpv`, in that order; `aplay` plays the built-in chimes only. Set `RODEO_DISABLE_SOUND` to turn sound off for one run.
