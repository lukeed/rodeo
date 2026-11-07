use crate::schema::{Document, Node, Space, Tab};

#[derive(Clone, Copy, Debug)]
pub struct Located<'a> {
    pub space: &'a Space,
    pub tab: &'a Tab,
    pub node: &'a Node,
}

impl Document {
    pub fn space(&self, id: &str) -> Option<&Space> {
        self.spaces.iter().find(|s| s.id == id)
    }

    pub fn space_mut(&mut self, id: &str) -> Option<&mut Space> {
        self.spaces.iter_mut().find(|s| s.id == id)
    }

    pub fn tabs(&self) -> impl Iterator<Item = &Tab> {
        self.spaces.iter().flat_map(|s| s.tabs.iter())
    }

    pub fn tab(&self, id: &str) -> Option<&Tab> {
        self.tabs().find(|t| t.id == id)
    }

    pub fn tab_mut(&mut self, id: &str) -> Option<&mut Tab> {
        self.spaces
            .iter_mut()
            .flat_map(|s| s.tabs.iter_mut())
            .find(|t| t.id == id)
    }

    pub fn space_of(&self, id: &str) -> Option<&Space> {
        self.spaces.iter().find(|s| {
            s.tabs
                .iter()
                .any(|t| t.id == id || t.root.find(id).is_some())
        })
    }

    pub fn tab_of(&self, node: &str) -> Option<&Tab> {
        self.tabs().find(|t| t.root.find(node).is_some())
    }

    pub(crate) fn tab_of_mut(&mut self, node: &str) -> Option<&mut Tab> {
        self.spaces
            .iter_mut()
            .flat_map(|s| s.tabs.iter_mut())
            .find(|t| t.root.find(node).is_some())
    }

    pub fn tab_index(&self, tab: &str) -> Option<usize> {
        self.spaces.iter().find_map(|s| s.tab_index(tab))
    }

    pub fn locate(&self, node: &str) -> Option<Located<'_>> {
        self.spaces.iter().find_map(|space| {
            space
                .tabs
                .iter()
                .find_map(|tab| tab.root.find(node).map(|node| Located { space, tab, node }))
        })
    }

    pub fn pane(&self, id: &str) -> Option<Located<'_>> {
        self.locate(id).filter(|l| l.node.is_pane())
    }

    pub fn pane_of_terminal(&self, address: &str) -> Option<Located<'_>> {
        self.spaces.iter().find_map(|space| {
            space.tabs.iter().find_map(|tab| {
                tab.root
                    .terminals()
                    .into_iter()
                    .find(|n| n.address() == Some(address))
                    .map(|node| Located { space, tab, node })
            })
        })
    }

    pub fn addresses(&self) -> Vec<String> {
        self.tabs().flat_map(Tab::addresses).collect()
    }
}

impl Space {
    pub fn tab(&self, id: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn tab_index(&self, id: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    pub fn terminals(&self) -> Vec<&Node> {
        self.tabs.iter().flat_map(Tab::terminals).collect()
    }

    pub fn addresses(&self) -> Vec<String> {
        self.tabs.iter().flat_map(Tab::addresses).collect()
    }

    pub fn first_terminal(&self) -> Option<&Node> {
        self.tabs
            .iter()
            .find_map(|t| t.terminals().into_iter().next())
    }
}
