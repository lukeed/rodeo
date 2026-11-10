---
title: Usage
summary: Open rodeo on an hc session, find your way around the screen, and leave it with every program still running.
group: Get started
order: 2
---

## Open rodeo

```sh
$ rodeo                    # open on the default hc session
$ rodeo --session work     # open on another hc session
```

An hc session is one hc core, with its own terminals and its own layout. rodeo starts the core when it is not running. If you use the Herdr desktop app on the same session, rodeo opens on the spaces that the app shows.

When there are no spaces yet, rodeo shows its hat and a hint. Press <kbd>prefix+shift+n</kbd> for a new space, or click `new` in the sidebar.

## The screen

| Part | What it shows |
| --- | --- |
| Sidebar | Your spaces, each with its agent state, git branch and git status. Below them, every agent with its state. `new` and `menu` sit between the two lists. |
| Tab bar | The tabs of the focused space, and `+` for a new tab. Set `ui.tab_bar_position = "bottom"` to move it. |
| Panes | The split layout of the focused tab. The focused pane has an accent border. |
| Mode bar | Outside terminal mode, a bar at the bottom shows the mode and its keys. |

<kbd>prefix+b</kbd> folds the sidebar into a narrow status column, or hides it with `ui.sidebar_collapsed_mode = "hidden"`.

On a terminal 64 columns wide or less, rodeo shows a 2-row header, one column of panes and a switcher. `ui.mobile_width_threshold` sets that width.

## Words

rodeo keeps the words of Herdr TUI on screen. The Herdr app and these docs say "space" where Herdr TUI says "workspace".

| Word | Means |
| --- | --- |
| Space | A named group of tabs that starts in one folder, on one machine. |
| Tab | One split layout of panes. |
| Pane | One terminal. `hc` runs it; rodeo only shows it. |
| Agent | A program in a pane that reports its state to `hc`. See [Agents](/docs/agents/). |
| Machine | A computer that `hc` reaches over ssh. See [Machines](/docs/machines/). |

## First steps

Every bind below starts with the prefix, <kbd>ctrl+b</kbd>. Press the prefix, let go, then press the key.

| Keys | Does |
| --- | --- |
| <kbd>prefix+shift+n</kbd> | New space. |
| <kbd>prefix+v</kbd> | Split the pane side by side. |
| <kbd>prefix+minus</kbd> | Split the pane top and bottom. |
| <kbd>prefix+h/j/k/l</kbd> | Focus the pane to the left, below, above or to the right. |
| <kbd>prefix+c</kbd> | New tab. |
| <kbd>prefix+1..9</kbd> | Go to tab 1 to 9. |
| <kbd>prefix+w</kbd> | Pick a space with <kbd>↑</kbd> <kbd>↓</kbd> and <kbd>enter</kbd>. |
| <kbd>prefix+g</kbd> | Find any space, pane or agent by name. |
| <kbd>prefix+z</kbd> | Zoom the pane. |
| <kbd>prefix+?</kbd> | Every key bind, with a filter. |

[Key binds](/docs/keys/) has the rest, and how to change them.

## Leave and come back

<kbd>prefix+q</kbd> detaches: rodeo quits, and your terminals keep running in `hc`. Close the terminal window and the same thing happens. Open rodeo again and every pane comes back as you left it, with the same space, tab, focus and zoom.

You can also run two rodeo at once: in two terminal windows, or one of them over ssh. Each one has its own focus and zoom, and they share one layout. The window you used last sets the size of a terminal that both show.

## With the Herdr desktop app

rodeo and the Herdr app read and write the same layout in `hc`. A split in one shows up in the other, and so do new tabs, closed panes and new names. Focus, zoom and the active space stay private to each window.

When two windows change the layout at the same moment, one change wins, and the other window shows the result.

## The mouse

- Click a space, tab, pane or agent to focus it.
- Drag a split border to resize two panes. Drag a tab to move it. Drag the edge of the sidebar to change its width.
- The wheel scrolls the pane. A program that asks for the mouse, such as `vim` or `htop`, gets the wheel instead.
- Select text to copy it. Double-click a word to select it. Set `ui.copy_on_select = false` to copy with <kbd>ctrl+c</kbd> instead.
- Right-click a space, tab or pane for its menu. Hold `ui.right_click_passthrough_modifier` to send a right-click to the program.
- Set `ui.mouse_capture = false` to give every click to your terminal.

## Copy mode

<kbd>prefix+[</kbd> freezes the pane and lets you move through its scrollback with the keyboard.

| Keys | Does |
| --- | --- |
| <kbd>h</kbd> <kbd>j</kbd> <kbd>k</kbd> <kbd>l</kbd>, arrows | Move the cursor. |
| <kbd>w</kbd> <kbd>b</kbd> <kbd>e</kbd>, <kbd>{</kbd> <kbd>}</kbd> | Move by word, or by paragraph. |
| <kbd>g</kbd> <kbd>G</kbd> | Go to the top or the bottom of the scrollback. |
| <kbd>/</kbd> <kbd>?</kbd>, <kbd>n</kbd> <kbd>N</kbd> | Search forward or back, then go to the next match. |
| <kbd>v</kbd> or <kbd>space</kbd>, <kbd>V</kbd> | Select from the cursor, or select lines. |
| <kbd>y</kbd> or <kbd>enter</kbd> | Copy the selection and leave copy mode. |
| <kbd>q</kbd> or <kbd>esc</kbd> | Leave copy mode. |

## More in a pane

- <kbd>prefix+e</kbd> opens the scrollback of the pane in `$EDITOR`, in a popup.
- <kbd>ctrl+v</kbd> pastes an image from your clipboard. rodeo sends the image beside the terminal with `hc`, then pastes its path. This also works for a pane on another machine.
- Links in a pane open on click: OSC 8 links, and URLs that rodeo finds in the text.
- Programs that draw with kitty graphics show their images, when your terminal supports them.
- <kbd>prefix+shift+g</kbd> makes a git worktree for a new branch, then a space in it.
