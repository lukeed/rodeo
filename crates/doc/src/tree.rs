use serde_json::Value;

use crate::schema::{Axis, Content, Kind, Look, Node, Size, Tab};

impl Node {
    pub fn new(id: impl Into<String>, kind: Kind) -> Node {
        Node {
            id: id.into(),
            cloned_from: None,
            kind,
            size: None,
            min: None,
            max: None,
            hidden: false,
            title: None,
            background: None,
            border: None,
            radius: None,
            shadow: None,
            padding: None,
            focus: None,
            look: Look::default(),
            gap: None,
            divider: None,
            movable: None,
            closable: None,
            resizable: None,
            floating: Vec::new(),
            titlebar: None,
            traffic_lights: None,
            preview: None,
            terminal_menu: None,
            window_drag: false,
        }
    }

    pub fn block(id: impl Into<String>, content: Content) -> Node {
        Node::new(id, Kind::Block { content })
    }

    pub fn terminal(id: impl Into<String>, address: impl Into<String>) -> Node {
        Node::block(
            id,
            Content::Terminal {
                terminal: address.into(),
            },
        )
    }

    pub fn module(id: impl Into<String>, name: impl Into<String>, props: Value) -> Node {
        Node::block(
            id,
            Content::Mod {
                name: name.into(),
                props,
            },
        )
    }

    pub fn container(id: impl Into<String>, axis: Axis, children: Vec<Node>) -> Node {
        Node::new(
            id,
            match axis {
                Axis::Row => Kind::Row { children },
                Axis::Column => Kind::Column { children },
            },
        )
    }

    pub fn children(&self) -> &[Node] {
        match &self.kind {
            Kind::Row { children } | Kind::Column { children } | Kind::Stack { children, .. } => {
                children
            }
            Kind::Block { .. } => &[],
        }
    }

    pub fn children_mut(&mut self) -> Option<&mut Vec<Node>> {
        match &mut self.kind {
            Kind::Row { children } | Kind::Column { children } | Kind::Stack { children, .. } => {
                Some(children)
            }
            Kind::Block { .. } => None,
        }
    }

    pub fn axis(&self) -> Option<Axis> {
        match self.kind {
            Kind::Row { .. } => Some(Axis::Row),
            Kind::Column { .. } => Some(Axis::Column),
            _ => None,
        }
    }

    pub fn content(&self) -> Option<&Content> {
        match &self.kind {
            Kind::Block { content } => Some(content),
            _ => None,
        }
    }

    pub fn address(&self) -> Option<&str> {
        match self.content() {
            Some(Content::Terminal { terminal }) => Some(terminal),
            _ => None,
        }
    }

    pub fn is_pane(&self) -> bool {
        matches!(
            self.content(),
            Some(Content::Terminal { .. } | Content::Mod { .. })
        )
    }

    pub fn closable(&self) -> bool {
        self.closable.unwrap_or(true)
    }

    pub fn resizable(&self) -> bool {
        self.resizable.unwrap_or(true)
    }

    pub fn walk(&self) -> Walk<'_> {
        Walk { stack: vec![self] }
    }

    pub fn find(&self, id: &str) -> Option<&Node> {
        self.walk().find(|n| n.id == id)
    }

    pub fn find_mut(&mut self, id: &str) -> Option<&mut Node> {
        let path = self.path_to(id)?;
        Some(self.at_mut(&path))
    }

    pub fn path_to(&self, id: &str) -> Option<Vec<usize>> {
        if self.id == id {
            return Some(Vec::new());
        }
        self.children().iter().enumerate().find_map(|(i, c)| {
            c.path_to(id).map(|mut p| {
                p.insert(0, i);
                p
            })
        })
    }

    pub fn at(&self, path: &[usize]) -> &Node {
        path.iter().fold(self, |n, &i| &n.children()[i])
    }

    pub fn at_mut(&mut self, path: &[usize]) -> &mut Node {
        let mut node = self;
        for &i in path {
            node = &mut node.children_mut().expect("path through a container")[i];
        }
        node
    }

    pub fn parent_of(&self, id: &str) -> Option<&Node> {
        let path = self.path_to(id)?;
        let (_, parent) = path.split_last()?;
        Some(self.at(parent))
    }

    pub fn panes(&self) -> Vec<&Node> {
        self.walk().filter(|n| n.is_pane()).collect()
    }

    pub fn terminals(&self) -> Vec<&Node> {
        self.walk().filter(|n| n.address().is_some()).collect()
    }

    pub fn addresses(&self) -> Vec<String> {
        self.walk()
            .filter_map(Node::address)
            .map(str::to_owned)
            .collect()
    }

    pub fn visible_blocks(&self) -> Vec<&Node> {
        if self.hidden {
            return Vec::new();
        }
        match &self.kind {
            Kind::Block { .. } => vec![self],
            Kind::Stack {
                children, active, ..
            } => shown_child(children, active.as_deref())
                .map(Node::visible_blocks)
                .unwrap_or_default(),
            Kind::Row { children } | Kind::Column { children } => {
                children.iter().flat_map(Node::visible_blocks).collect()
            }
        }
    }

    pub fn visible_panes(&self) -> Vec<&Node> {
        self.visible_blocks()
            .into_iter()
            .filter(|n| n.is_pane())
            .collect()
    }

    pub fn active_child(&self) -> Option<&Node> {
        match &self.kind {
            Kind::Stack {
                children, active, ..
            } => shown_child(children, active.as_deref()),
            _ => None,
        }
    }

    pub(crate) fn split(
        &mut self,
        target: &str,
        axis: Axis,
        new: Node,
        container: String,
    ) -> Split {
        let Some(path) = self.path_to(target) else {
            return Split::Missing;
        };
        if let Some((&i, parent)) = path.split_last()
            && self.at(parent).axis() == Some(axis)
        {
            let children = self.at_mut(parent).children_mut().expect("a row or column");
            let share = match children[i].size {
                Some(Size::Weight { weight }) => weight,
                None | Some(Size::Px { .. }) => 1.0,
            };
            if !matches!(children[i].size, Some(Size::Px { .. })) {
                children[i].size = Some(Size::Weight {
                    weight: share / 2.0,
                });
            }
            let mut new = new;
            new.size = Some(Size::Weight {
                weight: share / 2.0,
            });
            children.insert(i + 1, new);
            return Split::Inserted;
        }
        let slot = self.at_mut(&path);
        let mut old = std::mem::replace(slot, Node::container(String::new(), axis, Vec::new()));
        let size = old.size.take();
        let mut wrapper = Node::container(container.clone(), axis, vec![old, new]);
        wrapper.size = size;
        *slot = wrapper;
        Split::Wrapped(container)
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Node> {
        let removed = take(self, id)?;
        collapse(self);
        Some(removed)
    }

    pub(crate) fn swap(&mut self, a: &str, b: &str) -> bool {
        let (Some(pa), Some(pb)) = (self.path_to(a), self.path_to(b)) else {
            return false;
        };
        if pa.starts_with(&pb) || pb.starts_with(&pa) {
            return false;
        }
        let na = self.at(&pa).clone();
        let nb = self.at(&pb).clone();
        let (sa, sb) = (na.size, nb.size);
        *self.at_mut(&pa) = Node { size: sa, ..nb };
        *self.at_mut(&pb) = Node { size: sb, ..na };
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Split {
    Missing,
    Inserted,
    Wrapped(String),
}

pub struct Walk<'a> {
    stack: Vec<&'a Node>,
}

impl<'a> Iterator for Walk<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<&'a Node> {
        let node = self.stack.pop()?;
        self.stack.extend(node.children().iter().rev());
        Some(node)
    }
}

fn shown_child<'a>(children: &'a [Node], active: Option<&str>) -> Option<&'a Node> {
    let mut shown = children.iter().filter(|c| !c.hidden);
    active
        .and_then(|a| children.iter().find(|c| !c.hidden && c.id == a))
        .or_else(|| shown.next())
}

fn flex(size: Option<Size>) -> Option<f32> {
    match size {
        Some(Size::Weight { weight }) => Some(weight),
        None => Some(1.0),
        Some(Size::Px { .. }) => None,
    }
}

fn take(node: &mut Node, id: &str) -> Option<Node> {
    let lines = node.axis().is_some();
    let children = node.children_mut()?;
    let Some(i) = children.iter().position(|n| n.id == id) else {
        return children.iter_mut().find_map(|n| take(n, id));
    };
    let removed = children.remove(i);
    if lines
        && let Some(gone) = flex(removed.size)
        && let Some(heir) = children.get_mut(i.saturating_sub(1))
        && let Some(kept) = flex(heir.size)
    {
        heir.size = Some(Size::Weight {
            weight: kept + gone,
        });
    }
    if let Kind::Stack {
        children, active, ..
    } = &mut node.kind
        && active.as_deref() == Some(id)
    {
        *active = children.get(i).or(children.last()).map(|n| n.id.clone());
    }
    Some(removed)
}

fn collapse(node: &mut Node) {
    let stack = matches!(node.kind, Kind::Stack { .. });
    let Some(children) = node.children_mut() else {
        return;
    };
    children.iter_mut().for_each(collapse);
    if stack {
        return;
    }
    children.retain(|n| n.axis().is_none() || !n.children().is_empty());
    for slot in children.iter_mut() {
        if slot.axis().is_some() && slot.children().len() == 1 {
            let size = slot.size;
            let mut only = slot.children_mut().expect("a row or column").remove(0);
            only.size = size;
            *slot = only;
        }
    }
}

impl Tab {
    pub fn new(id: impl Into<String>, name: impl Into<String>, pane: Node) -> Tab {
        let id = id.into();
        Tab {
            root: Node::container(id.clone(), Axis::Row, vec![pane]),
            id,
            name: name.into(),
        }
    }

    pub fn panes(&self) -> Vec<&Node> {
        self.root.panes()
    }

    pub fn terminals(&self) -> Vec<&Node> {
        self.root.terminals()
    }

    pub fn addresses(&self) -> Vec<String> {
        self.root.addresses()
    }

    pub fn cycle(&self, from: &str, forward: bool) -> Option<&str> {
        let panes = self.root.visible_panes();
        let n = panes.len();
        let i = panes.iter().position(|p| p.id == from)?;
        let j = if forward {
            (i + 1) % n
        } else {
            (i + n - 1) % n
        };
        Some(panes[j].id.as_str())
    }
}
