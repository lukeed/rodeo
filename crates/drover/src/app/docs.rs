use crate::sync::{Command, Edit, Written};

use super::App;

impl App {
    pub fn edit(&mut self, machine: Option<&str>, edit: Edit) -> bool {
        self.try_edit(machine, edit).unwrap_or_else(|error| {
            self.show_error(error);
            false
        })
    }

    pub fn edit_quiet(&mut self, machine: Option<&str>, edit: Edit) -> bool {
        self.try_edit(machine, edit).unwrap_or_else(|error| {
            tracing::debug!(%error, "edit skipped");
            false
        })
    }

    fn try_edit(&mut self, machine: Option<&str>, edit: Edit) -> Result<bool, String> {
        let slot = &mut self
            .machines
            .get_mut(machine)
            .ok_or_else(|| format!("{} is not connected", machine.unwrap_or("Local")))?
            .slot;
        let changed = slot.edit(edit)?;
        if changed {
            self.pump(machine);
            self.after_docs();
        }
        Ok(changed)
    }

    pub(super) fn retry_docs(&mut self) {
        let mut retry = Vec::new();
        for machine in self.machines.iter_mut() {
            let failed = matches!(machine.slot.state(), crate::sync::State::Unreadable(_));
            if failed && machine.is_connected() && !machine.slot.is_busy() {
                machine.slot.refresh();
                retry.push(machine.name.clone());
            }
        }
        for name in retry {
            self.pump(name.as_deref());
        }
    }

    pub fn pump_all(&mut self) {
        let names: Vec<Option<String>> = self.machines.iter().map(|m| m.name.clone()).collect();
        for name in names {
            self.pump(name.as_deref());
        }
    }

    pub fn pump(&mut self, machine: Option<&str>) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(entry) = self.machines.get_mut(machine) else {
            return;
        };
        if !entry.is_connected() {
            return;
        }
        let errors = entry.slot.take_errors();
        let command = entry.slot.command();
        for error in errors {
            self.show_error(error);
        }
        let name = machine.map(str::to_owned);
        match command {
            Some(Command::Read) => self.spawn(async move {
                let result = client.doc_get(doc::NAME, name.as_deref()).await;
                move |app: &mut App| app.doc_read(name.as_deref(), result)
            }),
            Some(Command::Write { value, version }) => self.spawn(async move {
                let result = client
                    .doc_set(doc::NAME, &value, version, name.as_deref())
                    .await;
                move |app: &mut App| app.doc_written(name.as_deref(), result)
            }),
            None => {}
        }
    }

    fn doc_read(&mut self, machine: Option<&str>, result: Result<hc::Doc, hc::HcError>) {
        let Some(entry) = self.machines.get_mut(machine) else {
            return;
        };
        if let Err(error) = &result {
            tracing::warn!(machine = machine.unwrap_or("local"), %error, "doc.get failed");
        }
        entry.slot.read_done(
            result
                .map(|doc| (doc.value, doc.version))
                .map_err(|e| e.to_string()),
        );
        self.pump(machine);
        self.after_docs();
    }

    fn doc_written(&mut self, machine: Option<&str>, result: Result<u64, hc::HcError>) {
        let Some(entry) = self.machines.get_mut(machine) else {
            return;
        };
        let written = match result {
            Ok(version) => Written::Ok(version),
            Err(error) if error.is_conflict() => {
                tracing::info!(
                    machine = machine.unwrap_or("local"),
                    "doc conflict; merging"
                );
                Written::Conflict
            }
            Err(error) => Written::Failed(format!("layout write failed: {error}")),
        };
        entry.slot.write_done(written);
        self.pump(machine);
        self.after_docs();
    }

    pub fn after_docs(&mut self) {
        self.view.fix(&self.machines);
        if let Some(pane) = self.view.want_focus.clone()
            && self.machines.pane(&pane).is_some()
        {
            self.view.want_focus = None;
            self.focus_pane(&pane);
        }
        self.ensure_first_space();
        self.sync_panes();
        self.relayout();
        self.dirty = true;
    }

    fn ensure_first_space(&mut self) {
        if self.first_space || !self.terminals_ready || !self.machines.local().slot.is_ready() {
            return;
        }
        self.first_space = true;
        if self.machines.spaces().is_empty() {
            self.new_workspace_now(String::new());
        }
    }
}
