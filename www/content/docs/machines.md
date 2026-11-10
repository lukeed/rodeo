---
title: Machines
summary: rodeo shows the spaces of every machine that `hc` links to, beside your own. `hc` owns the ssh link; rodeo never opens one.
group: Using rodeo
order: 3
---

## Add a machine

`hc` reaches other machines over ssh, and runs its own core on each one. Add a machine once, with `hc`:

```sh
$ hc machine add vps                  # a Host from ~/.ssh/config, or user@host
$ hc machine add can@10.0.0.7:2222    # a user, an address and a port
$ hc machine ls                       # the machines added here
```

`hc machine add --via` reaches a machine with another command than ssh, such as `fly ssh console`. The Herdr desktop app adds machines the same way, so a machine that you add in the app shows up in rodeo too.

`hc` copies itself to the machine when it has no `hc`, or one that is too old. It asks first.

## The sidebar

With more than one machine, the sidebar lists machines, and the spaces of each machine below it. Your own machine comes first, then the others by name. The agent list shows the machine of each agent.

| Mark | Means |
| --- | --- |
| <span style="color:var(--green)">●</span> | Connected. |
| <span style="color:var(--yellow)">◐ connecting</span> | `hc` is connecting, or rodeo is reading the layout. |
| <span style="color:var(--yellow)">◐ reconnecting</span> | `hc` cannot reach the machine, and tries again. Check the network or ssh. |
| <span style="color:var(--red)">! auth</span> | The machine needs a password or a code. Run `hc machine login <name>`. |
| <span style="color:var(--red)">! error</span> | The machine needs a newer `hc`, or rodeo cannot read its layout. Run `hc machine update <name>` for the first. |

The spaces of a machine that is not connected show dim. A machine marked as temporary, such as a sandbox for one task, does not show.

## Work on a machine

Click a machine row to open the space you used last on it. On a machine with no spaces, the click makes it the active machine. `new`, <kbd>prefix+c</kbd> and <kbd>prefix+shift+n</kbd> then work on the active machine: a new tab goes to the space you used last there, and with no space yet, rodeo makes the first one.

Each space runs on one machine. Its panes, its git branch, its worktrees, the `hostname` of the tab bar and your custom commands all run there.

In navigate mode, <kbd>↑</kbd> and <kbd>↓</kbd> also stop on machine rows. On a machine row, <kbd>←</kbd> folds its spaces and <kbd>→</kbd> shows them. On a space row, <kbd>←</kbd> goes to its machine row. <kbd>enter</kbd> on a machine row opens the space you used last there.

`previous_workspace` and `next_workspace` step through the spaces of all machines.

## One layout per machine

Each machine keeps the layout of its own spaces in its own core. The Herdr desktop app reads the same layout, so a split on a remote machine shows up in the app and in rodeo.

You can also run rodeo on the other machine itself, over ssh. It shows the same spaces there.
