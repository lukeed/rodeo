use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Instant;

use config::{NewCwd, ShellMode};
use doc::{Axis, Document, Node, Prefix, Tab};
use hc::{RunParams, Size};

use super::App;
use crate::agents::SEEN;
use crate::ids::{local, qualify};
use crate::layout::{self, Chrome};
use crate::sync::Edit;

const PLACEHOLDER: &str = "t0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spawn {
    pub machine: Option<String>,
    pub space: String,
    pub tab: String,
    pub cwd: Option<String>,
    pub size: (u16, u16),
    pub argv: Vec<String>,
    pub keep: bool,
}

impl App {
    pub fn active_machine(&self) -> Option<String> {
        let space = self.view.space.as_deref()?;
        self.machines.space(space)?.machine.map(str::to_owned)
    }

    pub fn live_cwd(&self, pane: &str) -> Option<String> {
        let address = self.machines.pane(pane)?.address()?;
        let terminal = self.terminals.get(&address)?;
        terminal
            .foreground
            .as_ref()
            .map(|f| f.cwd.clone())
            .filter(|cwd| !cwd.is_empty())
    }

    pub fn new_cwd(
        &self,
        source: Option<&str>,
        space: Option<&str>,
        machine: Option<&str>,
    ) -> Option<String> {
        let home = (machine.is_none()).then(|| self.home());
        let folder = space
            .and_then(|s| self.machines.space(s))
            .map(|s| s.space.folder.clone())
            .filter(|f| !f.is_empty());
        match &self.config.terminal.new_cwd {
            NewCwd::Follow => source
                .filter(|p| self.machines.pane(p).is_some_and(|p| p.machine == machine))
                .and_then(|p| self.live_cwd(p))
                .or(folder)
                .or(home),
            NewCwd::Home => home,
            NewCwd::Current => std::env::current_dir()
                .ok()
                .map(|d| d.to_string_lossy().into_owned())
                .filter(|_| machine.is_none())
                .or(home),
            NewCwd::Path(path) => Some(expand(path, &self.home())),
        }
    }

    pub fn shell_argv(&self) -> Vec<String> {
        let shell = self.config.terminal.default_shell.trim();
        if shell.is_empty() {
            return Vec::new();
        }
        let login = match self.config.terminal.shell_mode {
            ShellMode::Login => true,
            ShellMode::NonLogin => false,
            ShellMode::Auto => cfg!(target_os = "macos"),
        };
        let mut argv = vec![shell.to_owned()];
        if login {
            argv.push("-l".to_owned());
        }
        argv
    }

    pub fn predicted_size(
        &self,
        machine: Option<&str>,
        edit: &Edit,
        tab: &str,
        pane: &str,
    ) -> (u16, u16) {
        let fallback = (
            self.regions.surface.width.max(2) - 1,
            self.regions.surface.height.max(1),
        );
        let Some(slot) = self.machines.get(machine).map(|m| &m.slot) else {
            return fallback;
        };
        let mut doc = slot.doc().clone();
        if edit(&mut doc).is_err() {
            return fallback;
        }
        let Some(space) = doc.space_of(tab) else {
            return fallback;
        };
        let Some(found) = space.tab(tab) else {
            return fallback;
        };
        let regions = layout::regions(
            self.screen,
            &self.config.ui,
            self.sidebar_width(),
            space.tabs.len(),
        );
        let geom = layout::tab(
            found,
            regions.surface,
            Chrome::from_ui(&self.config.ui),
            regions.mobile.then_some(pane),
            self.cell(),
            &|_| true,
        );
        geom.pane(pane).map_or(fallback, |p| {
            (p.content.width.max(1), p.content.height.max(1))
        })
    }

    pub fn launch(
        &mut self,
        spawn: Spawn,
        then: impl FnOnce(&mut App, Result<String, String>) + 'static,
    ) {
        let Some(client) = self.client() else {
            then(self, Err("hc is not connected".into()));
            return;
        };
        let guard = (
            spawn.machine.clone(),
            spawn.space.clone(),
            spawn.tab.clone(),
        );
        self.launching.insert(guard.clone());
        let params = RunParams {
            argv: spawn.argv,
            size: Some(Size {
                cols: spawn.size.0.max(2),
                rows: spawn.size.1.max(1),
            }),
            display: Some(self.display()).filter(|d| !d.is_empty()),
            cwd: spawn.cwd,
            on: spawn.machine.clone(),
            labels: BTreeMap::from([
                ("herdr.space".to_owned(), spawn.space),
                ("herdr.tab".to_owned(), spawn.tab),
                (SEEN.to_owned(), "0".to_owned()),
            ]),
            keep: spawn.keep,
            ..RunParams::default()
        };
        let machine = spawn.machine;
        self.spawn(async move {
            let result = client.run(&params).await;
            move |app: &mut App| {
                app.launching.remove(&guard);
                let result = result.map_err(|e| e.to_string()).map(|terminal| {
                    let address = local(&terminal.address, machine.as_deref()).to_owned();
                    let qualified = qualify(&address, machine.as_deref());
                    app.born.insert(qualified, Instant::now());
                    address
                });
                then(app, result);
            }
        });
    }

    fn landed(&mut self, machine: Option<&str>, address: &str, edit: Edit, pane: String) {
        let qualified = qualify(address, machine);
        self.edit(machine, edit);
        let placed = self.machines.pane_of_address(&qualified).map(|p| p.key());
        match placed {
            Some(placed) => {
                self.view.want_focus = Some(if placed == pane { pane } else { placed });
                self.after_docs();
            }
            None => self.kill(qualified),
        }
    }

    pub fn new_workspace_now(&mut self, name: String) {
        let machine = self.active_machine();
        let source = self.view.focused_pane(&self.machines);
        let space_key = self.view.space.clone();
        let folder = self
            .new_cwd(source.as_deref(), space_key.as_deref(), machine.as_deref())
            .unwrap_or_default();
        self.new_space_in(machine, folder, name);
    }

    pub fn new_space_in(&mut self, machine: Option<String>, folder: String, name: String) {
        let Some(doc) = self
            .machines
            .get(machine.as_deref())
            .map(|m| m.slot.doc().clone())
        else {
            return;
        };
        let (space, tab, pane) = fresh_ids(&doc);
        let edit = new_space_edit(&space, &name, &folder, &tab, &pane, PLACEHOLDER);
        let size = self.predicted_size(machine.as_deref(), &edit, &tab, &pane);
        let spawn = Spawn {
            machine: machine.clone(),
            space: space.clone(),
            tab: tab.clone(),
            cwd: Some(folder.clone()).filter(|f| !f.is_empty()),
            size,
            argv: self.shell_argv(),
            keep: true,
        };
        self.launch(spawn, move |app, result| match result {
            Ok(address) => {
                let edit = new_space_edit(&space, &name, &folder, &tab, &pane, &address);
                let space_key = qualify(&space, machine.as_deref());
                let tab_key = qualify(&tab, machine.as_deref());
                app.view.show_tab(&space_key, &tab_key);
                app.landed(
                    machine.as_deref(),
                    &address,
                    edit,
                    qualify(&pane, machine.as_deref()),
                );
                app.view.mode = crate::view::Mode::Terminal;
            }
            Err(error) => app.show_error(format!("cannot start a shell: {error}")),
        });
    }

    pub fn new_tab_now(&mut self, name: String) {
        let Some(space_key) = self.view.space.clone() else {
            self.new_workspace_now(String::new());
            return;
        };
        let Some(space) = self.machines.space(&space_key) else {
            return;
        };
        let machine = space.machine.map(str::to_owned);
        let space_id = space.space.id.clone();
        let doc = self
            .machines
            .get(machine.as_deref())
            .map(|m| m.slot.doc().clone())
            .unwrap_or_default();
        let tab = doc.fresh_id(Prefix::Tab);
        let pane = doc.fresh_id(Prefix::Pane);
        let source = self.view.focused_pane(&self.machines);
        let cwd = self.new_cwd(source.as_deref(), Some(&space_key), machine.as_deref());
        let edit = new_tab_edit(&space_id, &tab, &name, &pane, PLACEHOLDER);
        let size = self.predicted_size(machine.as_deref(), &edit, &tab, &pane);
        let spawn = Spawn {
            machine: machine.clone(),
            space: space_id.clone(),
            tab: tab.clone(),
            cwd,
            size,
            argv: self.shell_argv(),
            keep: true,
        };
        self.launch(spawn, move |app, result| match result {
            Ok(address) => {
                let edit = new_tab_edit(&space_id, &tab, &name, &pane, &address);
                let tab_key = qualify(&tab, machine.as_deref());
                app.view.show_tab(&space_key, &tab_key);
                app.landed(
                    machine.as_deref(),
                    &address,
                    edit,
                    qualify(&pane, machine.as_deref()),
                );
            }
            Err(error) => app.show_error(format!("cannot start a shell: {error}")),
        });
    }

    pub fn split_now(&mut self, axis: Axis, argv: Option<Vec<String>>, zoom: bool) {
        let Some(target_key) = self.view.focused_pane(&self.machines) else {
            if argv.is_none() {
                self.new_workspace_now(String::new());
            }
            return;
        };
        let Some(target) = self.machines.pane(&target_key) else {
            return;
        };
        let machine = target.machine.map(str::to_owned);
        let target_id = target.node.id.clone();
        let holds = target.node.address().map(str::to_owned);
        let space_id = target.space.id.clone();
        let tab_id = target.tab.id.clone();
        let space_key = target.space_key();
        let doc = self
            .machines
            .get(machine.as_deref())
            .map(|m| m.slot.doc().clone())
            .unwrap_or_default();
        let pane = doc.fresh_id(Prefix::Pane);
        let cwd = self.new_cwd(Some(&target_key), Some(&space_key), machine.as_deref());
        let edit = split_edit(&target_id, holds.as_deref(), axis, &pane, PLACEHOLDER);
        let size = if zoom {
            let surface = self.regions.surface;
            (surface.width, surface.height)
        } else {
            self.predicted_size(machine.as_deref(), &edit, &tab_id, &pane)
        };
        let temporary = argv.is_some();
        let spawn = Spawn {
            machine: machine.clone(),
            space: space_id,
            tab: tab_id.clone(),
            cwd,
            size,
            argv: argv.unwrap_or_else(|| self.shell_argv()),
            keep: !temporary,
        };
        self.launch(spawn, move |app, result| match result {
            Ok(address) => {
                let edit = split_edit(&target_id, holds.as_deref(), axis, &pane, &address);
                let tab_key = qualify(&tab_id, machine.as_deref());
                app.landed(
                    machine.as_deref(),
                    &address,
                    edit,
                    qualify(&pane, machine.as_deref()),
                );
                if zoom {
                    app.view.zoomed.insert(tab_key);
                }
            }
            Err(error) => app.show_error(format!("cannot start a shell: {error}")),
        });
    }

    pub fn kill(&mut self, address: String) {
        let Some(client) = self.client() else {
            return;
        };
        self.killing.insert(address.clone());
        self.spawn(async move {
            let result = client.kill(&address).await;
            move |app: &mut App| {
                if let Err(error) = result
                    && !error.is_not_found()
                    && !error.is_finished()
                {
                    tracing::warn!(%address, %error, "kill failed");
                }
                app.killing.remove(&address);
            }
        });
    }
}

fn fresh_ids(doc: &Document) -> (String, String, String) {
    let space = doc.fresh_id(Prefix::Space);
    let mut tab = doc.fresh_id(Prefix::Tab);
    while tab == space {
        tab = doc.fresh_id(Prefix::Tab);
    }
    (space, tab, doc.fresh_id(Prefix::Pane))
}

fn new_space_edit(
    space: &str,
    name: &str,
    folder: &str,
    tab: &str,
    pane: &str,
    address: &str,
) -> Edit {
    let (space, name, folder, tab, pane, address) = (
        space.to_owned(),
        name.to_owned(),
        folder.to_owned(),
        tab.to_owned(),
        pane.to_owned(),
        address.to_owned(),
    );
    Rc::new(move |doc: &mut Document| {
        if doc.pane_of_terminal(&address).is_some() {
            return Ok(());
        }
        doc.new_space(
            &space,
            &name,
            &folder,
            Tab::new(
                tab.clone(),
                "",
                Node::terminal(pane.clone(), address.clone()),
            ),
        )
    })
}

fn new_tab_edit(space: &str, tab: &str, name: &str, pane: &str, address: &str) -> Edit {
    let (space, tab, name, pane, address) = (
        space.to_owned(),
        tab.to_owned(),
        name.to_owned(),
        pane.to_owned(),
        address.to_owned(),
    );
    Rc::new(move |doc: &mut Document| {
        if doc.pane_of_terminal(&address).is_some() {
            return Ok(());
        }
        doc.new_tab(
            &space,
            Tab::new(
                tab.clone(),
                name.clone(),
                Node::terminal(pane.clone(), address.clone()),
            ),
            None,
        )
    })
}

fn split_edit(target: &str, holds: Option<&str>, axis: Axis, pane: &str, address: &str) -> Edit {
    let (target, holds) = (target.to_owned(), holds.map(str::to_owned));
    let (pane, address) = (pane.to_owned(), address.to_owned());
    Rc::new(move |doc: &mut Document| {
        if doc.pane_of_terminal(&address).is_some() {
            return Ok(());
        }
        let target = holds
            .as_deref()
            .and_then(|held| doc.pane_of_terminal(held))
            .map_or_else(|| target.clone(), |found| found.node.id.clone());
        doc.split_pane(&target, axis, Node::terminal(pane.clone(), address.clone()))
            .map(|_| ())
    })
}

pub fn expand(path: &str, home: &str) -> String {
    let path = path.trim();
    if path == "~" {
        return home.to_owned();
    }
    match path.strip_prefix("~/") {
        Some(rest) => format!("{}/{rest}", home.trim_end_matches('/')),
        None => path.to_owned(),
    }
}
