use std::fmt::Write;

use crate::schema::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prefix {
    Space,
    Tab,
    Pane,
    Split,
}

impl Prefix {
    pub fn as_str(self) -> &'static str {
        match self {
            Prefix::Space => "sp",
            Prefix::Tab => "tb",
            Prefix::Pane => "p",
            Prefix::Split => "s",
        }
    }
}

pub fn fresh_id(prefix: Prefix) -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the system random source works");
    bytes.iter().fold(prefix.as_str().to_owned(), |mut id, b| {
        write!(id, "{b:02x}").expect("writing to a String never fails");
        id
    })
}

pub fn is_id(id: &str, prefix: Prefix) -> bool {
    id.strip_prefix(prefix.as_str()).is_some_and(|hex| {
        hex.len() == 32 && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
    })
}

impl Document {
    pub fn fresh_id(&self, prefix: Prefix) -> String {
        loop {
            let id = fresh_id(prefix);
            if !self.contains_id(&id) {
                return id;
            }
        }
    }

    pub fn contains_id(&self, id: &str) -> bool {
        self.spaces.iter().any(|s| {
            s.id == id
                || s.tabs.iter().any(|t| {
                    t.id == id
                        || t.root
                            .walk()
                            .any(|n| n.id == id || n.floating.iter().any(|f| f.id == id))
                })
        })
    }
}
