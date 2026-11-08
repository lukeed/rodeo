use std::collections::HashMap;

use vt::Terminal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    Idle,
    Attaching,
    Attached,
    Lost(String),
    Ended(String),
    Failed(String),
}

impl Link {
    pub fn is_live(&self) -> bool {
        matches!(self, Self::Attaching | Self::Attached)
    }
}

const QUEUE_LIMIT: usize = 1 << 20;

pub struct Pane {
    pub address: String,
    queued: Vec<u8>,
    pub term: Terminal,
    pub writer: Option<hc::AttachWriter>,
    pub link: Link,
    pub attach: u64,
    pub size: (u16, u16),
    pub shown: bool,
    pub led: Option<(u16, u16)>,
    pub focus_sent: Option<bool>,
    pub notifications: Vec<(String, String)>,
    pub title: String,
}

impl Pane {
    pub fn new(address: String, size: (u16, u16), options: vt::Options) -> Result<Self, vt::Error> {
        Ok(Self {
            address,
            queued: Vec::new(),
            term: Terminal::new(size.0.max(1), size.1.max(1), options)?,
            writer: None,
            link: Link::Idle,
            attach: 0,
            size,
            shown: false,
            led: None,
            focus_sent: None,
            notifications: Vec::new(),
            title: String::new(),
        })
    }

    pub fn input(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        match &self.writer {
            Some(writer) => {
                if let Err(error) = writer.input(bytes.to_vec()) {
                    tracing::debug!(address = %self.address, %error, "input dropped");
                }
            }
            None if matches!(self.link, Link::Idle | Link::Attaching)
                && self.queued.len() + bytes.len() <= QUEUE_LIMIT =>
            {
                self.queued.extend_from_slice(bytes);
            }
            None => tracing::debug!(address = %self.address, "input dropped: not attached"),
        }
    }

    pub fn queued(&self) -> &[u8] {
        &self.queued
    }

    pub fn connect(&mut self, writer: hc::AttachWriter) {
        let queued = std::mem::take(&mut self.queued);
        if !queued.is_empty()
            && let Err(error) = writer.input(queued)
        {
            tracing::debug!(address = %self.address, %error, "queued input dropped");
        }
        self.writer = Some(writer);
        self.link = Link::Attached;
    }

    pub fn end(&mut self, link: Link) {
        self.queued.clear();
        self.link = link;
    }

    pub fn reply(&mut self) {
        let replies = self.term.take_replies();
        if replies.is_empty() {
            return;
        }
        if let Some(writer) = &self.writer {
            let _ = writer.automatic_input(replies);
        }
    }

    pub fn resize(&mut self, size: (u16, u16), cell_px: (u32, u32)) {
        let size = (size.0.max(1), size.1.max(1));
        if size == self.size && self.term.cell_px() == cell_px {
            return;
        }
        self.size = size;
        if let Err(error) = self.term.resize(size.0, size.1, cell_px) {
            tracing::warn!(address = %self.address, %error, "vt resize failed");
        }
        self.reply();
    }

    pub fn lead(&mut self) {
        if !self.shown || self.link != Link::Attached || self.led == Some(self.size) {
            return;
        }
        if let Some(writer) = &self.writer
            && writer.resize(self.size.0, self.size.1).is_ok()
        {
            self.led = Some(self.size);
        }
    }

    pub fn focus(&mut self, focused: bool) {
        if self.focus_sent == Some(focused) || self.link != Link::Attached {
            return;
        }
        if let Some(writer) = &self.writer
            && writer.focus(focused).is_ok()
        {
            self.focus_sent = Some(focused);
        }
    }

    pub fn detach(&mut self) {
        if let Some(writer) = self.writer.take() {
            let _ = writer.close();
        }
        self.led = None;
        self.focus_sent = None;
    }
}

#[derive(Default)]
pub struct Panes {
    map: HashMap<String, Pane>,
}

impl Panes {
    pub fn get(&self, address: &str) -> Option<&Pane> {
        self.map.get(address)
    }

    pub fn get_mut(&mut self, address: &str) -> Option<&mut Pane> {
        self.map.get_mut(address)
    }

    pub fn insert(&mut self, pane: Pane) {
        self.map.insert(pane.address.clone(), pane);
    }

    pub fn rename(&mut self, from: &str, to: &str) {
        if self.map.contains_key(to) {
            return;
        }
        if let Some(mut pane) = self.map.remove(from) {
            to.clone_into(&mut pane.address);
            self.map.insert(to.to_owned(), pane);
        }
    }

    pub fn remove(&mut self, address: &str) -> Option<Pane> {
        self.map.remove(address)
    }

    pub fn contains(&self, address: &str) -> bool {
        self.map.contains_key(address)
    }

    pub fn addresses(&self) -> Vec<String> {
        self.map.keys().cloned().collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Pane> {
        self.map.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Pane> {
        self.map.values_mut()
    }
}
