export default {
  why: [
    "Herdr TUI runs its own server, and the desktop app runs on <code>hc</code>. They share no terminals. I wanted my terminal and my app to see the same thing.",
    "<code>hc</code> owns every program. rodeo only owns the UI, so you can close it, open it again, or run two at once. Your terminals keep running the whole time.",
  ],
  features: [
    ["acc", "Keyboard first", "Workspaces, tabs &amp; splits from the keyboard (<code>ctrl+b</code> prefix), or the mouse."],
    ["yellow", "One glance", "Spaces, agents and machines in the sidebar, each with its state."],
    ["teal", "One shared layout", "Shared with the Herdr desktop app and other rodeo clients. A split in one shows up in the other."],
    ["green", "Your Herdr TUI setup", "Herdr TUI keys &amp; config work as is."],
    ["acc", "Agent states", "Blocked, working, done and idle from <code>hc</code>, with toasts &amp; sounds."],
    ["blue", "Other machines", "Spaces on other machines through <code>hc</code>&rsquo;s machine links."],
    ["mauve", "The details", "Worktrees, popups, copy mode, image paste, links &amp; kitty graphics."],
  ],
  installs: [
    ["curl", "curl -fsSL https://rodeo.tools/install.sh | sh"],
    ["homebrew", "brew install lukeed/tap/rodeo"],
    ["mise", "mise use -g packslip:github.com/lukeed/rodeo"],
  ],
  keys: [
    ["ctrl+b", "prefix"],
    ["prefix+v", "split the pane side by side"],
    ["prefix+minus", "split the pane top and bottom"],
    ["prefix+c", "new tab"],
    ["prefix+w", "pick a space"],
    ["prefix+z", "zoom the pane"],
    ["prefix+q", "detach; terminals keep running"],
    ["prefix+?", "every key bind"],
  ],
  gone: [
    ["Plugins", "There is no plugin model."],
    ["Self-update, release notes &amp; onboarding", "rodeo does not update itself."],
    ["Agent integrations", "Agents report their own state to <code>hc</code> (OSC 7501)."],
  ],
};
