use doc::{Node, Space, Tab};

use crate::ids::qualify;
use crate::sync::Slot;

pub const LOCAL_LABEL: &str = "Local";

#[derive(Debug)]
pub struct Machine {
    pub name: Option<String>,
    pub info: Option<hc::MachineView>,
    pub slot: Slot,
}

impl Machine {
    pub fn local() -> Self {
        Self {
            name: None,
            info: None,
            slot: Slot::default(),
        }
    }

    pub fn label(&self) -> &str {
        self.name.as_deref().unwrap_or(LOCAL_LABEL)
    }

    pub fn key(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn is_local(&self) -> bool {
        self.name.is_none()
    }

    pub fn is_connected(&self) -> bool {
        self.info.as_ref().is_none_or(hc::MachineView::is_connected)
    }

    pub fn state_word(&self) -> &str {
        self.info
            .as_ref()
            .map_or("connected", |info| info.state.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SpaceRef<'a> {
    pub machine: Option<&'a str>,
    pub space: &'a Space,
}

impl<'a> SpaceRef<'a> {
    pub fn key(&self) -> String {
        qualify(&self.space.id, self.machine)
    }

    pub fn tab_key(&self, tab: &Tab) -> String {
        qualify(&tab.id, self.machine)
    }

    pub fn tabs(&self) -> impl Iterator<Item = TabRef<'a>> + 'a {
        let (machine, space) = (self.machine, self.space);
        space.tabs.iter().map(move |tab| TabRef {
            machine,
            space,
            tab,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TabRef<'a> {
    pub machine: Option<&'a str>,
    pub space: &'a Space,
    pub tab: &'a Tab,
}

impl<'a> TabRef<'a> {
    pub fn key(&self) -> String {
        qualify(&self.tab.id, self.machine)
    }

    pub fn space_key(&self) -> String {
        qualify(&self.space.id, self.machine)
    }

    pub fn index(&self) -> usize {
        self.space.tab_index(&self.tab.id).unwrap_or(0)
    }

    pub fn name(&self) -> String {
        self.space
            .tab_name(&self.tab.id)
            .unwrap_or_else(|| (self.index() + 1).to_string())
    }

    pub fn panes(&self) -> impl Iterator<Item = PaneRef<'a>> + 'a {
        let (machine, space, tab) = (self.machine, self.space, self.tab);
        tab.root.panes().into_iter().map(move |node| PaneRef {
            machine,
            space,
            tab,
            node,
        })
    }

    pub fn addresses(&self) -> Vec<String> {
        self.tab
            .addresses()
            .iter()
            .map(|a| qualify(a, self.machine))
            .collect()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PaneRef<'a> {
    pub machine: Option<&'a str>,
    pub space: &'a Space,
    pub tab: &'a Tab,
    pub node: &'a Node,
}

impl PaneRef<'_> {
    pub fn key(&self) -> String {
        qualify(&self.node.id, self.machine)
    }

    pub fn tab_key(&self) -> String {
        qualify(&self.tab.id, self.machine)
    }

    pub fn space_key(&self) -> String {
        qualify(&self.space.id, self.machine)
    }

    pub fn address(&self) -> Option<String> {
        self.node.address().map(|a| qualify(a, self.machine))
    }
}

#[derive(Debug)]
pub struct Machines {
    list: Vec<Machine>,
}

impl Default for Machines {
    fn default() -> Self {
        Self {
            list: vec![Machine::local()],
        }
    }
}

impl Machines {
    pub fn iter(&self) -> impl Iterator<Item = &Machine> {
        self.list.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Machine> {
        self.list.iter_mut()
    }

    pub fn local(&self) -> &Machine {
        &self.list[0]
    }

    pub fn get(&self, name: Option<&str>) -> Option<&Machine> {
        self.list.iter().find(|m| m.key() == name)
    }

    pub fn get_mut(&mut self, name: Option<&str>) -> Option<&mut Machine> {
        self.list.iter_mut().find(|m| m.key() == name)
    }

    pub fn is_multi(&self) -> bool {
        self.list.len() > 1
    }

    pub fn update(&mut self, views: Vec<hc::MachineView>) -> Vec<Option<String>> {
        let views: Vec<hc::MachineView> = views.into_iter().filter(|v| !v.temporary).collect();
        self.list
            .retain(|m| m.is_local() || views.iter().any(|v| Some(v.name.as_str()) == m.key()));
        let mut changed = Vec::new();
        for view in views {
            let connected = view.is_connected();
            if let Some(machine) = self.get_mut(Some(&view.name)) {
                let was = machine.is_connected();
                machine.info = Some(view);
                if was != connected {
                    machine.slot.reset();
                    changed.push(machine.name.clone());
                }
            } else {
                changed.push(Some(view.name.clone()));
                self.list.push(Machine {
                    name: Some(view.name.clone()),
                    info: Some(view),
                    slot: Slot::default(),
                });
            }
        }
        self.list[1..].sort_by(|a, b| a.name.cmp(&b.name));
        changed
    }

    pub fn spaces(&self) -> Vec<SpaceRef<'_>> {
        self.list
            .iter()
            .filter(|m| m.is_connected() && m.slot.is_ready())
            .flat_map(|m| {
                m.slot.doc().spaces.iter().map(|space| SpaceRef {
                    machine: m.key(),
                    space,
                })
            })
            .collect()
    }

    pub fn space(&self, key: &str) -> Option<SpaceRef<'_>> {
        self.spaces().into_iter().find(|s| s.key() == key)
    }

    pub fn tab(&self, key: &str) -> Option<TabRef<'_>> {
        self.spaces()
            .into_iter()
            .flat_map(|s| s.tabs().collect::<Vec<_>>())
            .find(|t| t.key() == key)
    }

    pub fn pane(&self, key: &str) -> Option<PaneRef<'_>> {
        self.tabs()
            .flat_map(|t| t.panes().collect::<Vec<_>>())
            .find(|p| p.key() == key)
    }

    pub fn tabs(&self) -> impl Iterator<Item = TabRef<'_>> {
        self.spaces()
            .into_iter()
            .flat_map(|s| s.tabs().collect::<Vec<_>>())
    }

    pub fn panes(&self) -> impl Iterator<Item = PaneRef<'_>> {
        self.tabs().flat_map(|t| t.panes().collect::<Vec<_>>())
    }

    pub fn pane_of_address(&self, address: &str) -> Option<PaneRef<'_>> {
        self.panes()
            .find(|p| p.address().as_deref() == Some(address))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str, state: &str, temporary: bool) -> hc::MachineView {
        hc::MachineView {
            name: name.into(),
            state: state.into(),
            temporary,
            ..hc::MachineView::default()
        }
    }

    #[test]
    fn machines_keep_local_first_and_skip_temporary_ones() {
        let mut machines = Machines::default();
        let changed = machines.update(vec![
            view("zed", "connected", false),
            view("tmp", "connected", true),
            view("arc", "unreachable", false),
        ]);
        assert_eq!(changed, [Some("zed".to_owned()), Some("arc".to_owned())]);
        let names: Vec<_> = machines.iter().map(Machine::label).collect();
        assert_eq!(names, ["Local", "arc", "zed"]);
        assert!(!machines.get(Some("arc")).unwrap().is_connected());
        machines.update(vec![view("arc", "connected", false)]);
        let names: Vec<_> = machines.iter().map(Machine::label).collect();
        assert_eq!(names, ["Local", "arc"]);
        assert!(machines.get(Some("arc")).unwrap().is_connected());
    }
}
