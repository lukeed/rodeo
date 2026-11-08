mod confirm;
pub mod context;
mod field;
pub mod frame;
pub mod goto;
pub mod help;
pub mod menu;
mod rename;
pub mod scroll;
pub mod settings;
pub mod worktree;

use config::{Config, Keymap, Palette};
use crossterm::event::{KeyEvent, MouseEvent};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::model::Snapshot;

pub use confirm::{CloseTarget, ConfirmClose};
pub use field::TextField;
pub use rename::{Rename, RenameTarget};
pub use worktree::{Worktree, WorktreeOp};

pub struct Ctx<'a> {
    pub palette: &'a Palette,
    pub keymap: &'a Keymap,
    pub config: &'a Config,
    pub snapshot: &'a Snapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    Space(String),
    Tab(String),
    Pane(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Request {
    Action(config::Action),
    Rename {
        target: RenameTarget,
        text: String,
    },
    Close(CloseTarget),
    Navigate,
    Focus(Focus),
    Settings(config::Edit),
    Worktree(WorktreeOp),
    Preview(Option<String>),
    Menu {
        target: MenuTarget,
        pick: context::Pick,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Stay,
    Close,
    Done(Request),
    Keep(Request),
}

pub trait Overlay: std::any::Any {
    fn kind(&self) -> Kind;

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<(u16, u16)>;

    fn key(&mut self, key: &KeyEvent, ctx: &Ctx<'_>) -> Outcome;

    fn mouse(&mut self, _event: &MouseEvent, _ctx: &Ctx<'_>) -> Outcome {
        Outcome::Stay
    }

    fn paste(&mut self, _text: &str) -> Outcome {
        Outcome::Stay
    }

    fn dims(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Rename,
    ConfirmClose,
    Help,
    Goto,
    Settings,
    NewWorktree,
    OpenWorktree,
    RemoveWorktree,
    GlobalMenu,
    ContextMenu(MenuTarget),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    Space(String),
    Tab(String),
    Pane(String),
}

pub type Opener = fn(&Kind, &Ctx<'_>) -> Option<Box<dyn Overlay>>;

pub fn open(kind: &Kind, ctx: &Ctx<'_>) -> Option<Box<dyn Overlay>> {
    OPENERS.iter().find_map(|opener| opener(kind, ctx))
}

const OPENERS: &[Opener] = &[help::open, goto::open, settings::open, worktree::open];
