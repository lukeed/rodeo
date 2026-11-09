use std::rc::Rc;
use std::time::{Duration, Instant};

use doc::{Document, Place};
use serde_json::Value;

use super::App;
use crate::agents::{self, SEEN};
use crate::ids::{local, qualify};
use crate::sync::Edit;

const GRACE: Duration = Duration::from_secs(5);
const ADOPT_GRACE_MS: u64 = 2000;
const WHERE: &str = "where";

impl App {
    pub fn reconcile(&mut self) {
        if !self.terminals_ready || self.client().is_none() {
            return;
        }
        self.adopt_orphans();
        self.check_gone();
        self.write_where();
    }

    fn doc_ready(&self, machine: Option<&str>) -> bool {
        self.machines
            .get(machine)
            .is_some_and(|m| m.is_connected() && m.slot.is_ready())
    }

    fn adopt_orphans(&mut self) {
        let shown: Vec<String> = self.machines.panes().filter_map(|p| p.address()).collect();
        let now = now_ms();
        let mut orphans = Vec::new();
        for terminal in self.terminals.iter() {
            if !terminal.is_live()
                || now.saturating_sub(terminal.created_ms) < ADOPT_GRACE_MS
                || shown.contains(&terminal.address)
                || self.adopting.contains(&terminal.address)
                || self.killing.contains(&terminal.address)
            {
                continue;
            }
            let machine = self.owner(&terminal.address);
            if terminal.address.contains('/') && machine.is_none() {
                continue;
            }
            if !self.doc_ready(machine.as_deref()) {
                continue;
            }
            let Some(space) = terminal.label("herdr.space").filter(|s| !s.is_empty()) else {
                continue;
            };
            let tab = terminal.label("herdr.tab").filter(|s| !s.is_empty());
            let guard = (
                machine.clone(),
                space.to_owned(),
                tab.unwrap_or_default().to_owned(),
            );
            if self.launching.contains(&guard) {
                continue;
            }
            orphans.push(terminal.address.clone());
        }
        orphans.sort();
        for address in orphans {
            self.start_adoption(address);
        }
    }

    fn start_adoption(&mut self, address: String) {
        let Some(client) = self.client() else {
            return;
        };
        let Some(expected) = self.terminals.get(&address).cloned() else {
            return;
        };
        self.adopting.insert(address.clone());
        self.spawn(async move {
            let ready = async {
                let record = client.get(&address).await?;
                let same = record.is_live()
                    && record.label("herdr.space") == expected.label("herdr.space")
                    && record.label("herdr.tab") == expected.label("herdr.tab");
                if !same {
                    return Ok::<_, hc::HcError>(None);
                }
                if !record.keep {
                    client.keep(&address, true).await?;
                }
                Ok(Some(record))
            }
            .await;
            move |app: &mut App| {
                app.adopting.remove(&address);
                match ready {
                    Ok(Some(record)) => app.adopt(&address, &record),
                    Ok(None) => {}
                    Err(error) => tracing::debug!(%address, %error, "adoption skipped"),
                }
            }
        });
    }

    fn adopt(&mut self, address: &str, record: &hc::Terminal) {
        if self.machines.pane_of_address(address).is_some() {
            return;
        }
        let machine = self.owner(address);
        let labels = serde_json::to_value(&record.labels).unwrap_or(Value::Null);
        let Some(place) = Place::from_labels(&labels) else {
            return;
        };
        let folder = record
            .foreground
            .as_ref()
            .map(|f| f.cwd.clone())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| record.cwd.clone());
        let space_key = qualify(&place.space, machine.as_deref());
        let shown_tab = self
            .view
            .tabs
            .get(&space_key)
            .map(|t| local(t, machine.as_deref()).to_owned());
        let id = local(address, machine.as_deref()).to_owned();
        tracing::info!(%address, space = %place.space, "adopting a labelled terminal");
        let edit: Edit = Rc::new(move |doc: &mut Document| {
            doc.adopt(&id, &place, &folder, shown_tab.as_deref())
                .map(|_| ())
        });
        let had_space = self.view.space.is_some();
        self.edit_quiet(machine.as_deref(), edit);
        if !had_space {
            self.view.fix(&self.machines);
        }
    }

    fn check_gone(&mut self) {
        let now = Instant::now();
        let missing: Vec<String> = self
            .machines
            .panes()
            .filter(|p| self.doc_ready(p.machine))
            .filter_map(|p| p.address())
            .filter(|a| !crate::ids::is_pending(a))
            .filter(|a| self.terminals.get(a).is_none_or(hc::Terminal::is_finished))
            .filter(|a| !self.checking.contains(a))
            .filter(|a| {
                self.born
                    .get(a)
                    .is_none_or(|at| now.duration_since(*at) >= GRACE)
            })
            .collect();
        let Some(client) = self.client() else {
            return;
        };
        for address in missing {
            self.checking.insert(address.clone());
            let client = client.clone();
            self.spawn(async move {
                let result = client.get(&address).await;
                move |app: &mut App| {
                    app.checking.remove(&address);
                    let closed = match &result {
                        Ok(record) => matches!(record.reason.as_deref(), Some("exited" | "killed")),
                        Err(error) => error.is_not_found(),
                    };
                    if closed {
                        tracing::info!(%address, "removing the pane of a closed terminal");
                        app.remove_terminal(&address);
                    }
                }
            });
        }
    }

    fn write_where(&mut self) {
        let Some(client) = self.client() else {
            return;
        };
        let home = self.home();
        let mut wanted = Vec::new();
        for space in self.machines.spaces() {
            let name = self.space_name(&space.key(), &home);
            for tab in space.tabs() {
                let place = doc::where_label(&name, &tab.name());
                for address in tab.addresses() {
                    wanted.push((address, place.clone()));
                }
            }
        }
        self.written_where.retain(|address, _| {
            self.terminals
                .get(address)
                .is_some_and(hc::Terminal::is_live)
        });
        for (address, place) in wanted {
            let Some(terminal) = self.terminals.get(&address) else {
                continue;
            };
            if !terminal.is_live()
                || terminal.label(WHERE) == Some(place.as_str())
                || self.written_where.get(&address) == Some(&place)
            {
                continue;
            }
            self.written_where.insert(address.clone(), place.clone());
            let client = client.clone();
            self.spawn(async move {
                let result = client.label(&address, [(WHERE, place.as_str())]).await;
                move |app: &mut App| {
                    if let Err(error) = result {
                        tracing::debug!(%address, %error, "where label failed");
                        if app.written_where.get(&address) == Some(&place) {
                            app.written_where.remove(&address);
                        }
                    }
                }
            });
        }
    }

    pub fn write_seen(&mut self) {
        if !self.view.host_focused || self.view.overlay.is_some() {
            return;
        }
        let Some(client) = self.client() else {
            return;
        };
        let Some(address) = self
            .view
            .focused_pane(&self.machines)
            .and_then(|p| self.machines.pane(&p)?.address())
        else {
            return;
        };
        if !self.panes.get(&address).is_some_and(|p| p.shown) {
            return;
        }
        let Some(terminal) = self.terminals.get(&address) else {
            return;
        };
        if terminal.status.is_none() || !terminal.is_live() {
            return;
        }
        let turn = terminal.turn;
        if self.looked.get(&address) == Some(&turn) {
            return;
        }
        self.looked.insert(address.clone(), turn);
        if agents::seen_turn(terminal).is_some_and(|seen| seen >= turn) {
            return;
        }
        self.spawn(async move {
            let result = client.label(&address, [(SEEN, turn.to_string())]).await;
            move |app: &mut App| {
                if let Err(error) = result {
                    tracing::debug!(%address, %error, "seen label failed");
                    if app.looked.get(&address) == Some(&turn) {
                        app.looked.remove(&address);
                    }
                }
            }
        });
    }

    pub fn looked(&self, address: &str) -> Option<u64> {
        self.looked.get(address).copied()
    }

    pub fn space_name(&self, key: &str, home: &str) -> String {
        let Some(space) = self.machines.space(key) else {
            return String::new();
        };
        let resolved = self
            .git
            .get(key)
            .and_then(|g| g.toplevel.clone())
            .or_else(|| {
                let first = space.space.first_terminal()?.address()?;
                let terminal = self.terminals.get(&qualify(first, space.machine))?;
                terminal
                    .foreground
                    .as_ref()
                    .map(|f| f.cwd.clone())
                    .filter(|c| !c.is_empty())
                    .or_else(|| Some(terminal.cwd.clone()).filter(|c| !c.is_empty()))
            });
        space.space.display_name(resolved.as_deref(), home)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}
