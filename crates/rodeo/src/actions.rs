use std::rc::Rc;
use std::time::Duration;

use config::{Action, CommandKind};
use doc::{Axis, Direction, Document, RESIZE_STEP};
use hc::ExecParams;

use crate::app::{App, NoticeDot};
use crate::ids::{local, qualify};
use crate::overlay::{self, CloseTarget, ConfirmClose, Kind, Rename, RenameTarget};
use crate::sync::Edit;
use crate::view::{CopyState, Mode};

impl App {
    pub fn run_action(&mut self, action: Action) {
        tracing::debug!(?action, "action");
        match action {
            Action::Help => self.open_overlay(&Kind::Help),
            Action::Goto => self.open_overlay(&Kind::Goto),
            Action::Settings => self.open_overlay(&Kind::Settings),
            Action::Machines => self.open_machines(None),
            Action::NewWorktree => self.open_overlay(&Kind::NewWorktree),
            Action::OpenWorktree => self.open_overlay(&Kind::OpenWorktree),
            Action::RemoveWorktree => self.open_overlay(&Kind::RemoveWorktree),
            Action::NewWorkspace => self.new_workspace(),
            Action::RenameWorkspace => self.rename_workspace(),
            Action::CloseWorkspace => self.ask_close_workspace(),
            Action::WorkspacePicker => self.enter_navigate(),
            Action::Detach => self.quit = true,
            Action::ReloadConfig => self.reload_config(),
            Action::OpenNotificationTarget => self.open_notification_target(),
            Action::PreviousWorkspace => self.step_space(false),
            Action::NextWorkspace => self.step_space(true),
            Action::PreviousAgent => self.step_agent(false),
            Action::NextAgent => self.step_agent(true),
            Action::FocusAgent(index) => self.focus_agent(index),
            Action::NewTab => self.new_tab(),
            Action::RenameTab => self.rename_tab(),
            Action::PreviousTab => self.step_tab(false),
            Action::NextTab => self.step_tab(true),
            Action::MoveTabPrevious => self.move_tab(false),
            Action::MoveTabNext => self.move_tab(true),
            Action::SwitchTab(index) => self.switch_tab(index),
            Action::SwitchWorkspace(index) => self.switch_workspace(index),
            Action::CloseTab => self.ask_close_tab(),
            Action::RenamePane => self.rename_pane(),
            Action::EditScrollback => self.edit_scrollback(),
            Action::ClearPane => self.clear_pane(),
            Action::CopyMode => self.enter_copy_mode(),
            Action::FocusPaneLeft => self.focus_direction(Direction::Left),
            Action::FocusPaneDown => self.focus_direction(Direction::Down),
            Action::FocusPaneUp => self.focus_direction(Direction::Up),
            Action::FocusPaneRight => self.focus_direction(Direction::Right),
            Action::SwapPaneLeft => self.swap_direction(Direction::Left),
            Action::SwapPaneDown => self.swap_direction(Direction::Down),
            Action::SwapPaneUp => self.swap_direction(Direction::Up),
            Action::SwapPaneRight => self.swap_direction(Direction::Right),
            Action::CyclePaneNext => self.cycle_pane(true),
            Action::CyclePanePrevious => self.cycle_pane(false),
            Action::LastPane => self.last_pane(),
            Action::SplitVertical => self.split_now(Axis::Row, None, false),
            Action::SplitHorizontal => self.split_now(Axis::Column, None, false),
            Action::ClosePane => self.ask_close_pane(),
            Action::Zoom => self.toggle_zoom(),
            Action::ResizeMode => self.view.mode = Mode::Resize,
            Action::ResizePaneLeft => self.resize_focused(Direction::Left),
            Action::ResizePaneDown => self.resize_focused(Direction::Down),
            Action::ResizePaneUp => self.resize_focused(Direction::Up),
            Action::ResizePaneRight => self.resize_focused(Direction::Right),
            Action::ToggleSidebar => self.toggle_sidebar(),
            Action::NavigateWorkspaceUp => self.step_nav(false),
            Action::NavigateWorkspaceDown => self.step_nav(true),
            Action::Command(index) => self.run_command(index),
        }
        self.relayout();
        self.dirty = true;
    }

    pub fn overlay_ctx_open(&mut self, kind: &Kind) -> Option<Box<dyn overlay::Overlay>> {
        let snapshot = self.snapshot();
        let ctx = overlay::Ctx {
            palette: &self.palette,
            keymap: &self.keymap,
            config: &self.config,
            snapshot: &snapshot,
        };
        overlay::open(kind, &ctx)
    }

    pub fn open_overlay(&mut self, kind: &Kind) {
        if let Some(opened) = self.overlay_ctx_open(kind) {
            self.set_overlay(opened);
        } else {
            tracing::info!(?kind, "overlay not available yet");
        }
    }

    pub fn set_overlay(&mut self, opened: Box<dyn overlay::Overlay>) {
        if self.view.mode == Mode::Navigate
            || self.view.mode == Mode::Prefix
            || self.view.mode == Mode::Resize
        {
            self.view.mode = if self.view.copy.is_some() {
                Mode::Copy
            } else {
                Mode::Terminal
            };
        }
        self.view.overlay = Some(opened);
        self.overlay_opened();
    }

    fn target_space(&self) -> Option<String> {
        match self.view.mode {
            Mode::Navigate => match &self.view.nav {
                Some(nav) => nav.space().map(str::to_owned),
                None => self.view.space.clone(),
            },
            _ => self.view.space.clone(),
        }
    }

    fn new_workspace(&mut self) {
        if self.config.ui.prompt_new_workspace_name {
            let machine = self.target_machine();
            let source = self.view.focused_pane(&self.machines);
            let space = (self.active_machine() == machine)
                .then(|| self.view.space.clone())
                .flatten();
            let folder = self
                .new_cwd(source.as_deref(), space.as_deref(), machine.as_deref())
                .unwrap_or_default();
            let suggested = if folder.is_empty() {
                "workspace".to_owned()
            } else {
                doc::folder_name(&folder, &self.home())
            };
            let target = RenameTarget::NewWorkspace {
                suggested: suggested.clone(),
            };
            self.set_overlay(Box::new(Rename::new(target, &suggested)));
        } else {
            self.new_workspace_now(String::new());
        }
    }

    fn rename_workspace(&mut self) {
        let Some(space) = self.target_space() else {
            return;
        };
        let name = self.space_name(&space, &self.home());
        self.set_overlay(Box::new(Rename::new(
            RenameTarget::Workspace { space },
            &name,
        )));
    }

    fn ask_close_workspace(&mut self) {
        let Some(space) = self.target_space() else {
            return;
        };
        self.ask_close(self.close_space_target(space));
    }

    fn ask_close_tab(&mut self) {
        if let Some(tab) = self.view.active_tab(&self.machines) {
            self.request_close_tab(tab);
        }
    }

    pub fn request_close_tab(&mut self, tab: String) {
        let last = self
            .machines
            .tab(&tab)
            .is_some_and(|t| t.space.tabs.len() == 1);
        if last {
            self.ask_close(CloseTarget::Tab { tab });
        } else {
            self.close(&CloseTarget::Tab { tab });
        }
    }

    fn ask_close_pane(&mut self) {
        if let Some(pane) = self.view.focused_pane(&self.machines) {
            self.request_close_pane(pane);
        }
    }

    pub fn request_close_pane(&mut self, pane: String) {
        let last = self
            .machines
            .pane(&pane)
            .is_some_and(|p| p.space.tabs.len() == 1 && p.tab.panes().len() == 1);
        if last {
            self.ask_close(CloseTarget::Pane { pane });
        } else {
            self.close(&CloseTarget::Pane { pane });
        }
    }

    pub fn ask_close(&mut self, target: CloseTarget) {
        if !self.config.ui.confirm_close {
            self.close(&target);
            return;
        }
        let space = match &target {
            CloseTarget::Workspace { space } | CloseTarget::Group { space } => Some(space.clone()),
            CloseTarget::Tab { tab } => self.machines.tab(tab).map(|t| t.space_key()),
            CloseTarget::Pane { pane } => self.machines.pane(pane).map(|p| p.space_key()),
        };
        let Some(space) = space else {
            return;
        };
        let members = match &target {
            CloseTarget::Group { .. } => self.group_members(&space),
            _ => vec![space.clone()],
        };
        let panes = members
            .iter()
            .filter_map(|m| self.machines.space(m))
            .map(|s| s.tabs().map(|t| t.panes().count()).sum::<usize>())
            .sum();
        let name = self.space_name(&space, &self.home());
        let confirm = if members.len() > 1 {
            ConfirmClose::group(target, &name, members.len(), panes)
        } else {
            ConfirmClose::new(target, &name, panes)
        };
        self.set_overlay(Box::new(confirm));
    }

    pub fn close(&mut self, target: &CloseTarget) {
        let mut plans: Vec<(Option<String>, Vec<String>, Vec<Edit>)> = Vec::new();
        for (machine, terminals, edit) in self.closing(target) {
            match plans.iter_mut().find(|(m, _, _)| *m == machine) {
                Some((_, all, edits)) => {
                    all.extend(terminals);
                    edits.push(edit);
                }
                None => plans.push((machine, terminals, vec![edit])),
            }
        }
        for (machine, terminals, edits) in plans {
            let edit: Edit = match <[Edit; 1]>::try_from(edits) {
                Ok([edit]) => edit,
                Err(edits) => Rc::new(move |doc: &mut Document| {
                    for edit in &edits {
                        let _ = edit(doc);
                    }
                    Ok(())
                }),
            };
            if self.empties(machine.as_deref(), &edit) {
                self.replace_last(machine, edit, terminals);
            } else if self.edit(machine.as_deref(), edit) {
                for address in terminals {
                    self.kill(address);
                }
                if self.machines.spaces().is_empty() {
                    self.view.mode = Mode::Navigate;
                }
            }
        }
    }

    fn closing(&self, target: &CloseTarget) -> Vec<(Option<String>, Vec<String>, Edit)> {
        match target {
            CloseTarget::Group { space } => self
                .group_members(space)
                .into_iter()
                .flat_map(|member| self.closing(&CloseTarget::Workspace { space: member }))
                .collect(),
            CloseTarget::Workspace { space } => {
                let Some(found) = self.machines.space(space) else {
                    return Vec::new();
                };
                let id = found.space.id.clone();
                let terminals = found.tabs().flat_map(|t| t.addresses()).collect();
                vec![(
                    found.machine.map(str::to_owned),
                    terminals,
                    Rc::new(move |doc: &mut Document| doc.close_space(&id).map(|_| ())),
                )]
            }
            CloseTarget::Tab { tab } => {
                let Some(found) = self.machines.tab(tab) else {
                    return Vec::new();
                };
                let id = found.tab.id.clone();
                vec![(
                    found.machine.map(str::to_owned),
                    found.addresses(),
                    Rc::new(move |doc: &mut Document| doc.close_tab(&id).map(|_| ())),
                )]
            }
            CloseTarget::Pane { pane } => {
                let Some(found) = self.machines.pane(pane) else {
                    return Vec::new();
                };
                if !found.node.closable() {
                    return Vec::new();
                }
                let id = found.node.id.clone();
                vec![(
                    found.machine.map(str::to_owned),
                    found.address().into_iter().collect(),
                    Rc::new(move |doc: &mut Document| doc.close_pane(&id).map(|_| ())),
                )]
            }
        }
    }

    fn empties(&self, machine: Option<&str>, edit: &Edit) -> bool {
        let Some(entry) = self.machines.get(machine).filter(|m| m.slot.is_ready()) else {
            return false;
        };
        let mut doc = entry.slot.doc().clone();
        if edit(&mut doc).is_err() || !doc.spaces.is_empty() {
            return false;
        }
        self.machines.spaces().iter().all(|s| s.machine == machine)
    }

    pub fn leave_mode(&mut self) {
        self.view.mode = if self.view.copy.is_some() {
            Mode::Copy
        } else {
            Mode::Terminal
        };
    }

    pub fn focus_space(&mut self, key: &str) {
        if self.machines.space(key).is_none() {
            return;
        }
        let before = self.view.focused_pane(&self.machines);
        self.view.sidebar.picked = None;
        self.view.show_space(key);
        self.after_focus_change(before);
    }

    pub fn focus_tab(&mut self, key: &str) {
        let Some(tab) = self.machines.tab(key) else {
            return;
        };
        let space = tab.space_key();
        let before = self.view.focused_pane(&self.machines);
        self.view.sidebar.picked = None;
        self.view.show_tab(&space, key);
        self.after_focus_change(before);
    }

    pub fn focus_pane(&mut self, key: &str) {
        let Some(pane) = self.machines.pane(key) else {
            return;
        };
        let (space, tab) = (pane.space_key(), pane.tab_key());
        let before = self.view.focused_pane(&self.machines);
        self.view.sidebar.picked = None;
        self.view.show_tab(&space, &tab);
        self.view.focus.insert(tab, key.to_owned());
        self.after_focus_change(before);
    }

    fn after_focus_change(&mut self, before: Option<String>) {
        let after = self.view.focused_pane(&self.machines);
        if after != before {
            if before.is_some() {
                self.view.previous = before;
            }
            self.view.selection = None;
            if let Some(copy) = &self.view.copy
                && after
                    .as_deref()
                    .and_then(|p| self.machines.pane(p)?.address())
                    != Some(copy.address.clone())
            {
                self.view.copy = None;
                if self.view.mode == Mode::Copy {
                    self.view.mode = Mode::Terminal;
                }
            }
        }
        self.relayout();
    }

    fn step_space(&mut self, forward: bool) {
        let keys = self.space_order();
        let Some(next) = step(&keys, self.view.space.as_deref(), forward) else {
            return;
        };
        self.focus_space(&next);
    }

    fn switch_workspace(&mut self, index: usize) {
        let key = self.space_order().get(index).cloned();
        if let Some(key) = key {
            self.focus_space(&key);
        }
    }

    fn agent_order(&self) -> Vec<String> {
        self.snapshot()
            .ordered_agents()
            .into_iter()
            .map(|a| a.pane.clone())
            .collect()
    }

    fn step_agent(&mut self, forward: bool) {
        let order = self.agent_order();
        let focused = self.view.focused_pane(&self.machines);
        let next = match focused
            .as_ref()
            .and_then(|f| order.iter().position(|p| p == f))
        {
            Some(_) => step(&order, focused.as_deref(), forward),
            None if forward => order.first().cloned(),
            None => order.last().cloned(),
        };
        if let Some(next) = next {
            self.focus_pane(&next);
        }
    }

    fn focus_agent(&mut self, index: usize) {
        if let Some(pane) = self.agent_order().get(index).cloned() {
            self.focus_pane(&pane);
        }
    }

    fn open_notification_target(&mut self) {
        if let Some(target) = self.take_notice().and_then(|n| n.target) {
            self.focus_pane(&target);
        }
    }

    fn new_tab(&mut self) {
        let machine = self.target_machine();
        let space = if self.is_picked() {
            self.machine_space(machine.as_deref())
        } else {
            self.view.space.clone()
        };
        let Some(space) = space else {
            self.new_workspace_on(machine, String::new());
            return;
        };
        if self.config.ui.prompt_new_tab_name {
            let count = self
                .machines
                .space(&space)
                .map_or(0, |s| s.space.tabs.len());
            let suggested = (count + 1).to_string();
            let target = RenameTarget::NewTab {
                space,
                suggested: suggested.clone(),
            };
            self.set_overlay(Box::new(Rename::new(target, &suggested)));
        } else {
            self.new_tab_in(&space, String::new());
        }
    }

    fn rename_tab(&mut self) {
        let Some(tab) = self.view.active_tab(&self.machines) else {
            return;
        };
        let Some(found) = self.machines.tab(&tab) else {
            return;
        };
        let original = found.name();
        let custom = !found.tab.name.is_empty();
        let target = RenameTarget::Tab {
            tab,
            original: original.clone(),
            custom,
        };
        self.set_overlay(Box::new(Rename::new(target, &original)));
    }

    fn rename_pane(&mut self) {
        let Some(pane) = self.view.focused_pane(&self.machines) else {
            return;
        };
        let title = self
            .machines
            .pane(&pane)
            .and_then(|p| p.node.title.clone())
            .unwrap_or_default();
        self.set_overlay(Box::new(Rename::new(RenameTarget::Pane { pane }, &title)));
    }

    pub fn finish_rename(&mut self, target: RenameTarget, text: String) {
        match target {
            RenameTarget::NewWorkspace { suggested } => {
                let name = if text.is_empty() || text == suggested {
                    String::new()
                } else {
                    text
                };
                self.new_workspace_now(name);
            }
            RenameTarget::NewTab { space, suggested } => {
                let name = if text.is_empty() || text == suggested {
                    String::new()
                } else {
                    text
                };
                self.new_tab_in(&space, name);
            }
            RenameTarget::Workspace { space } => {
                if text.is_empty() {
                    return;
                }
                let Some(found) = self.machines.space(&space) else {
                    return;
                };
                let (machine, id) = (found.machine.map(str::to_owned), found.space.id.clone());
                self.edit(
                    machine.as_deref(),
                    Rc::new(move |doc: &mut Document| doc.rename_space(&id, &text)),
                );
            }
            RenameTarget::Tab {
                tab,
                original,
                custom,
            } => {
                if text.is_empty() || (!custom && text == original) {
                    return;
                }
                let Some(found) = self.machines.tab(&tab) else {
                    return;
                };
                let (machine, id) = (found.machine.map(str::to_owned), found.tab.id.clone());
                self.edit(
                    machine.as_deref(),
                    Rc::new(move |doc: &mut Document| doc.rename_tab(&id, &text)),
                );
            }
            RenameTarget::Pane { pane } => {
                let Some(found) = self.machines.pane(&pane) else {
                    return;
                };
                let (machine, id) = (found.machine.map(str::to_owned), found.node.id.clone());
                self.edit(
                    machine.as_deref(),
                    Rc::new(move |doc: &mut Document| doc.set_pane_title(&id, &text)),
                );
            }
        }
    }

    fn step_tab(&mut self, forward: bool) {
        let Some(space) = self
            .view
            .space
            .clone()
            .and_then(|s| self.machines.space(&s))
        else {
            return;
        };
        let keys: Vec<String> = space.tabs().map(|t| t.key()).collect();
        let current = self.view.active_tab(&self.machines);
        if let Some(next) = step(&keys, current.as_deref(), forward) {
            self.focus_tab(&next);
        }
    }

    fn switch_tab(&mut self, index: usize) {
        let key = self
            .view
            .space
            .clone()
            .and_then(|s| self.machines.space(&s))
            .and_then(|s| s.tabs().nth(index).map(|t| t.key()));
        if let Some(key) = key {
            self.focus_tab(&key);
        }
    }

    fn move_tab(&mut self, forward: bool) {
        let Some(tab) = self.view.active_tab(&self.machines) else {
            return;
        };
        let Some(found) = self.machines.tab(&tab) else {
            return;
        };
        let (machine, id) = (found.machine.map(str::to_owned), found.tab.id.clone());
        self.view.reveal_tab = true;
        self.edit(
            machine.as_deref(),
            Rc::new(move |doc: &mut Document| doc.step_tab(&id, forward)),
        );
    }

    pub fn move_tab_to(&mut self, tab: &str, to: usize) {
        let Some(found) = self.machines.tab(tab) else {
            return;
        };
        let (machine, id) = (found.machine.map(str::to_owned), found.tab.id.clone());
        self.edit(
            machine.as_deref(),
            Rc::new(move |doc: &mut Document| doc.move_tab(&id, to)),
        );
    }

    fn clear_pane(&mut self) {
        if let Some(address) = self.focused_address()
            && let Some(pane) = self.panes.get_mut(&address)
        {
            pane.term.clear();
        }
    }

    pub fn focused_address(&self) -> Option<String> {
        let pane = self.view.focused_pane(&self.machines)?;
        self.machines.pane(&pane)?.address()
    }

    pub fn enter_copy_mode(&mut self) {
        let Some(address) = self.focused_address() else {
            return;
        };
        let Some(pane) = self.panes.get(&address) else {
            return;
        };
        let mode = vt::CopyMode::enter(&pane.term);
        self.view.selection = None;
        self.view.copy = Some(CopyState {
            address,
            mode,
            prompt: None,
        });
        self.view.mode = Mode::Copy;
    }

    fn focus_direction(&mut self, direction: Direction) {
        let Some((tab, pane)) = self.focused_local() else {
            return;
        };
        let Some(geom) = self.geoms.get(&tab) else {
            return;
        };
        let machine = self
            .machines
            .tab(&tab)
            .and_then(|t| t.machine.map(str::to_owned));
        if let Some(next) = geom.neighbor(&pane, direction) {
            self.focus_pane(&qualify(&next, machine.as_deref()));
        }
    }

    fn focused_local(&self) -> Option<(String, String)> {
        let tab = self.view.active_tab(&self.machines)?;
        let pane = self.view.focused_in(&self.machines, &tab)?;
        let machine = self.machines.tab(&tab)?.machine;
        Some((tab, local(&pane, machine).to_owned()))
    }

    fn swap_direction(&mut self, direction: Direction) {
        let Some((tab, pane)) = self.focused_local() else {
            return;
        };
        if self.view.is_zoomed(&tab) {
            return;
        }
        let Some(other) = self
            .geoms
            .get(&tab)
            .and_then(|g| g.neighbor(&pane, direction))
        else {
            return;
        };
        let machine = self
            .machines
            .tab(&tab)
            .and_then(|t| t.machine.map(str::to_owned));
        self.view
            .focus
            .insert(tab, qualify(&pane, machine.as_deref()));
        self.edit(
            machine.as_deref(),
            Rc::new(move |doc: &mut Document| doc.swap_panes(&pane, &other)),
        );
    }

    fn cycle_pane(&mut self, forward: bool) {
        let Some((tab, pane)) = self.focused_local() else {
            return;
        };
        let Some(found) = self.machines.tab(&tab) else {
            return;
        };
        let machine = found.machine.map(str::to_owned);
        if let Some(next) = found.tab.cycle(&pane, forward).map(str::to_owned) {
            self.focus_pane(&qualify(&next, machine.as_deref()));
        }
    }

    fn last_pane(&mut self) {
        if let Some(previous) = self.view.previous.clone() {
            self.focus_pane(&previous);
        }
    }

    pub fn toggle_zoom(&mut self) {
        let Some(tab) = self.view.active_tab(&self.machines) else {
            return;
        };
        if !self.view.zoomed.remove(&tab) {
            self.view.zoomed.insert(tab);
        }
        self.view.reveal_tab = true;
    }

    fn resize_focused(&mut self, direction: Direction) {
        let Some((tab, pane)) = self.focused_local() else {
            return;
        };
        if self.view.is_zoomed(&tab) {
            return;
        }
        let Some(geom) = self.geoms.get(&tab).cloned() else {
            return;
        };
        let machine = self
            .machines
            .tab(&tab)
            .and_then(|t| t.machine.map(str::to_owned));
        let cell = self.cell();
        self.edit_quiet(
            machine.as_deref(),
            Rc::new(move |doc: &mut Document| {
                doc.resize_pane(&pane, direction, RESIZE_STEP, |id| geom.rect_of(id), cell)
                    .map(|_| ())
            }),
        );
    }

    pub fn drag_divider(
        &mut self,
        machine: Option<&str>,
        tab: &str,
        container: &str,
        index: usize,
        new_a: f32,
    ) {
        let Some(geom) = self.geoms.get(tab).cloned() else {
            return;
        };
        let cell = self.cell();
        let container = container.to_owned();
        self.edit_quiet(
            machine,
            Rc::new(move |doc: &mut Document| {
                doc.drag_divider(&container, index, new_a, |id| geom.rect_of(id), cell)
                    .map(|_| ())
            }),
        );
    }

    pub fn toggle_sidebar(&mut self) {
        let collapsed = self
            .prefs
            .sidebar_collapsed
            .unwrap_or(self.config.ui.sidebar_start_collapsed);
        self.prefs.sidebar_collapsed = Some(!collapsed);
        self.save_prefs();
    }

    fn run_command(&mut self, index: usize) {
        let Some(command) = self.keymap.command(index).cloned() else {
            return;
        };
        match command.kind {
            CommandKind::Shell => self.run_shell_command(&command.command),
            CommandKind::Pane => self.split_now(
                Axis::Row,
                Some(vec!["/bin/sh".into(), "-c".into(), command.command.clone()]),
                true,
            ),
            CommandKind::Popup => self.open_popup(&command),
            CommandKind::PluginAction => self.show_error("plugin actions are not supported"),
        }
    }

    fn run_shell_command(&mut self, command: &str) {
        let Some(client) = self.client() else {
            return;
        };
        let machine = self.active_machine();
        let (env, cwd) = self.active_env();
        let params = ExecParams {
            argv: vec!["/bin/sh".into(), "-lc".into(), command.to_owned()],
            on: machine,
            cwd,
            env,
            timeout: Some(Duration::from_secs(600)),
            ..ExecParams::default()
        };
        let label = command.to_owned();
        self.spawn(async move {
            let result = client.exec(&params).await;
            move |app: &mut App| match result {
                Ok(exec) if exec.success() => {}
                Ok(exec) => app.show_notice(
                    "Command failed",
                    &format!(
                        "{label}: exit {}",
                        exec.code.map_or_else(|| "signal".into(), |c| c.to_string())
                    ),
                    NoticeDot::Red,
                    None,
                    Duration::from_secs(8),
                ),
                Err(error) => app.show_error(format!("{label}: {error}")),
            }
        });
    }
}

pub fn step(keys: &[String], current: Option<&str>, forward: bool) -> Option<String> {
    if keys.is_empty() {
        return None;
    }
    let n = keys.len();
    let next = match current.and_then(|c| keys.iter().position(|k| k == c)) {
        None => 0,
        Some(i) if forward => (i + 1) % n,
        Some(i) => (i + n - 1) % n,
    };
    Some(keys[next].clone())
}

#[cfg(test)]
mod tests {
    use super::step;

    #[test]
    fn steps_wrap_both_ways() {
        let keys = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
        assert_eq!(step(&keys, Some("c"), true).as_deref(), Some("a"));
        assert_eq!(step(&keys, Some("a"), false).as_deref(), Some("c"));
        assert_eq!(step(&keys, None, false).as_deref(), Some("a"));
        assert_eq!(step(&[], None, true), None);
    }
}
