use std::rc::Rc;

use config::Action;
use doc::{Axis, Document};

use crate::app::App;
use crate::ids::local;
use crate::overlay::context::{
    ContextMenu, Pick, SpaceFlags, pane_flags, pane_items, space_items, tab_items,
};
use crate::overlay::menu::GlobalMenu;
use crate::overlay::{CloseTarget, MenuTarget, Rename, RenameTarget};

pub const RIGHT_CLICK: &str = "herdr.right-click";

impl App {
    pub fn context_menu(&mut self, target: MenuTarget, at: (u16, u16)) {
        if self.view.overlay.is_some() || !self.config.ui.mouse_capture {
            return;
        }
        let snapshot = self.snapshot();
        let mobile = self.regions.mobile;
        let items = match &target {
            MenuTarget::Space(space) if !mobile && !snapshot.collapsed => {
                let Some(row) = snapshot.space(space) else {
                    return;
                };
                space_items(SpaceFlags {
                    git: row.branch.is_some(),
                    ..SpaceFlags::default()
                })
            }
            MenuTarget::Tab(_) if !mobile => tab_items(),
            MenuTarget::Pane(pane) => {
                let Some(flags) = pane_flags(&snapshot, pane) else {
                    return;
                };
                pane_items(flags)
            }
            _ => return,
        };
        self.set_overlay(Box::new(ContextMenu::new(target, items, at)));
    }

    pub fn global_menu(&mut self) {
        if let Some(launcher) = self.hits.sidebar.menu {
            self.set_overlay(Box::new(GlobalMenu::new(launcher)));
        }
    }

    pub fn preview_theme(&mut self, theme: Option<&str>) {
        self.palette = match theme {
            Some(name) => {
                let mut preview = self.config.theme.clone();
                preview.name = Some(name.to_owned());
                preview.auto_switch = false;
                preview.palette(Some(&self.config.ui.accent), None)
            }
            None => self.config.palette(None),
        };
    }

    pub fn menu_pick(&mut self, target: MenuTarget, pick: Pick) {
        match target {
            MenuTarget::Space(space) => self.space_pick(space, pick),
            MenuTarget::Tab(tab) => self.tab_pick(tab, pick),
            MenuTarget::Pane(pane) => self.pane_pick(pane, pick),
        }
        self.relayout();
    }

    fn space_pick(&mut self, space: String, pick: Pick) {
        if self.machines.space(&space).is_none() {
            return;
        }
        let worktree = match pick {
            Pick::Rename => {
                let name = self.space_name(&space, &self.home());
                self.set_overlay(Box::new(Rename::new(
                    RenameTarget::Workspace { space },
                    &name,
                )));
                return;
            }
            Pick::Close => {
                self.ask_close(CloseTarget::Workspace { space });
                return;
            }
            Pick::NewWorktree => Action::NewWorktree,
            Pick::OpenWorktree => Action::OpenWorktree,
            Pick::RemoveWorktree => Action::RemoveWorktree,
            _ => return,
        };
        self.focus_space(&space);
        self.run_action(worktree);
    }

    fn tab_pick(&mut self, tab: String, pick: Pick) {
        if self.machines.tab(&tab).is_none() {
            return;
        }
        self.focus_tab(&tab);
        match pick {
            Pick::NewTab => self.run_action(Action::NewTab),
            Pick::Rename => self.run_action(Action::RenameTab),
            Pick::Close => self.request_close_tab(tab),
            _ => {}
        }
    }

    fn pane_pick(&mut self, pane: String, pick: Pick) {
        let Some(found) = self.machines.pane(&pane) else {
            return;
        };
        let title = found.node.title.clone();
        let address = found.address();
        match pick {
            Pick::RenamePane => {
                let initial = title.unwrap_or_default();
                self.set_overlay(Box::new(Rename::new(RenameTarget::Pane { pane }, &initial)));
            }
            Pick::ClearPaneName => self.finish_rename(RenameTarget::Pane { pane }, String::new()),
            Pick::Swap => self.swap_with_focused(&pane),
            Pick::SplitRight | Pick::SplitDown => {
                self.focus_pane(&pane);
                let axis = if pick == Pick::SplitRight {
                    Axis::Row
                } else {
                    Axis::Column
                };
                self.split_now(axis, None, false);
            }
            Pick::Zoom => {
                self.focus_pane(&pane);
                self.toggle_zoom();
            }
            Pick::RightClick => {
                if let Some(address) = address {
                    self.toggle_right_click(address);
                }
            }
            Pick::ClosePane => self.request_close_pane(pane),
            _ => {}
        }
    }

    fn swap_with_focused(&mut self, target: &str) {
        let Some(source) = self.view.focused_pane(&self.machines) else {
            return;
        };
        let (Some(from), Some(to)) = (self.machines.pane(&source), self.machines.pane(target))
        else {
            return;
        };
        if from.machine != to.machine || source == target {
            return;
        }
        let machine = from.machine.map(str::to_owned);
        let a = local(&source, machine.as_deref()).to_owned();
        let b = local(target, machine.as_deref()).to_owned();
        self.edit(
            machine.as_deref(),
            Rc::new(move |doc: &mut Document| doc.swap_panes(&a, &b)),
        );
        self.focus_pane(&source);
    }

    fn toggle_right_click(&mut self, address: String) {
        let Some(client) = self.client() else {
            return;
        };
        let to_program = self
            .terminals
            .get(&address)
            .and_then(|t| t.label(RIGHT_CLICK))
            != Some("program");
        let value = if to_program { "program" } else { "menu" };
        self.spawn(async move {
            let result = client.label(&address, [(RIGHT_CLICK, value)]).await;
            move |app: &mut App| {
                if let Err(error) = result {
                    app.show_error(format!("cannot set the right-click target: {error}"));
                }
            }
        });
    }
}
