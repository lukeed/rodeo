use std::collections::HashMap;
use std::ops::Range;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use libghostty_vt::osc::{CommandType, Parser};

const ESC: u8 = 0x1b;
const BEL: u8 = 0x07;
const CAN: u8 = 0x18;
const SUB: u8 = 0x1a;
const MAX_HELD: usize = 4 * 1024 * 1024;
const MAX_TEXT: usize = 64 * 1024;
const MAX_PENDING: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Found {
    Notify { title: String, body: String },
    Reply(Vec<u8>),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Op {
    Pass(Range<usize>),
    Held(Vec<u8>),
    Found(Found),
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum State {
    #[default]
    Ground,
    Esc,
    EscIntermediate,
    Csi,
    Text {
        osc: bool,
        esc: bool,
    },
}

#[derive(Default)]
struct Pending {
    title: String,
    body: String,
}

pub(crate) struct Scanner {
    state: State,
    held: Vec<u8>,
    holding: bool,
    parser: Parser<'static>,
    kitty: HashMap<String, Pending>,
}

struct Ops<'b> {
    bytes: &'b [u8],
    out: Vec<Op>,
    from: usize,
}

impl Ops<'_> {
    fn pass(&mut self, to: usize) {
        if to > self.from {
            self.out.push(Op::Pass(self.from..to));
        }
        self.from = to;
    }
}

impl Scanner {
    pub(crate) fn new() -> Result<Self, libghostty_vt::Error> {
        Ok(Self {
            state: State::Ground,
            held: Vec::new(),
            holding: false,
            parser: Parser::new()?,
            kitty: HashMap::new(),
        })
    }

    pub(crate) fn cancel(&mut self) -> bool {
        let busy = self.state != State::Ground;
        self.state = State::Ground;
        self.holding = false;
        self.held.clear();
        busy
    }

    pub(crate) fn scan(&mut self, bytes: &[u8]) -> Vec<Op> {
        let mut ops = Ops {
            bytes,
            out: Vec::new(),
            from: 0,
        };
        for (i, &byte) in bytes.iter().enumerate() {
            match self.state {
                State::Text { osc, esc } => self.text(byte, i, osc, esc, &mut ops),
                _ => self.idle(byte, i, &mut ops),
            }
        }
        if self.holding {
            self.held.extend_from_slice(&bytes[ops.from..]);
            ops.from = bytes.len();
            if self.held.len() > MAX_HELD {
                self.holding = false;
                ops.out.push(Op::Held(std::mem::take(&mut self.held)));
            }
        } else {
            ops.pass(bytes.len());
        }
        ops.out
    }

    fn idle(&mut self, byte: u8, at: usize, ops: &mut Ops) {
        self.state = match (self.state, byte) {
            (State::Ground, CAN | SUB) => {
                ops.pass(at);
                ops.from = at + 1;
                State::Ground
            }
            (_, CAN | SUB) => State::Ground,
            (_, ESC) => State::Esc,
            (State::Esc | State::EscIntermediate, 0x20..=0x2f) => State::EscIntermediate,
            (State::Csi, 0x40..=0x7e) => State::Ground,
            (State::Esc, b'[') | (State::Csi, _) => State::Csi,
            (State::Esc, b']' | b'P' | b'_' | b'^' | b'X') => {
                ops.pass(at);
                self.holding = true;
                self.held.clear();
                State::Text {
                    osc: byte == b']',
                    esc: false,
                }
            }
            (State::Esc, b'c') => {
                ops.pass(at + 1);
                ops.out.push(Op::Reset);
                State::Ground
            }
            _ => State::Ground,
        };
    }

    fn text(&mut self, byte: u8, at: usize, osc: bool, esc: bool, ops: &mut Ops) {
        match byte {
            CAN | SUB => {
                self.drop_held(at, ops);
                self.state = State::Ground;
            }
            b'\\' if esc => self.close(at + 1, ops),
            _ if esc => {
                self.close(at, ops);
                self.state = State::Esc;
                self.idle(byte, at, ops);
            }
            ESC => self.state = State::Text { osc, esc: true },
            BEL if osc => self.close(at + 1, ops),
            _ => {}
        }
    }

    fn drop_held(&mut self, at: usize, ops: &mut Ops) {
        if self.holding {
            self.holding = false;
            self.held.clear();
            ops.from = at;
        } else {
            ops.pass(at);
        }
    }

    fn close(&mut self, end: usize, ops: &mut Ops) {
        let State::Text { osc, .. } = self.state else {
            return;
        };
        self.state = State::Ground;
        if !self.holding {
            ops.pass(end);
            return;
        }
        self.holding = false;
        let mut held = std::mem::take(&mut self.held);
        held.extend_from_slice(&ops.bytes[ops.from..end]);
        ops.from = end;
        let found = if osc { self.inspect(&held) } else { None };
        ops.out.push(Op::Held(held));
        if let Some(found) = found {
            ops.out.push(Op::Found(found));
        }
    }

    fn inspect(&mut self, held: &[u8]) -> Option<Found> {
        let body = held.strip_prefix(b"]")?;
        let (payload, terminator) = match body {
            [rest @ .., BEL] => (rest, BEL),
            [rest @ .., ESC, b'\\'] => (rest, b'\\'),
            _ => (body.strip_suffix(&[ESC]).unwrap_or(body), ESC),
        };
        let code = payload.split(|&b| b == b';').next()?;
        match code {
            b"99" => self.kitty(payload.get(3..).unwrap_or_default(), terminator),
            b"9" | b"777" if payload.len() <= MAX_TEXT => self.desktop(payload, terminator),
            _ => None,
        }
    }

    fn desktop(&mut self, payload: &[u8], terminator: u8) -> Option<Found> {
        self.parser.reset();
        for &byte in payload {
            self.parser.next_byte(byte);
        }
        let kind = self.parser.end(terminator).command_type();
        if !matches!(kind, CommandType::ShowDesktopNotification) {
            return None;
        }
        let text = String::from_utf8_lossy(payload);
        if let Some(rest) = text.strip_prefix("777;notify;") {
            let (title, body) = rest.split_once(';').unwrap_or((rest, ""));
            return Some(Found::Notify {
                title: title.to_owned(),
                body: body.to_owned(),
            });
        }
        Some(Found::Notify {
            title: String::new(),
            body: text.strip_prefix("9;")?.to_owned(),
        })
    }

    fn kitty(&mut self, rest: &[u8], terminator: u8) -> Option<Found> {
        let text = String::from_utf8_lossy(rest);
        let (meta, data) = text.split_once(';').unwrap_or((&text, ""));
        let mut id = "";
        let mut done = true;
        let mut kind = "title";
        let mut base64 = false;
        for pair in meta.split(':') {
            match pair.split_once('=') {
                Some(("i", value)) => id = value,
                Some(("d", value)) => done = value != "0",
                Some(("p", value)) => kind = value,
                Some(("e", value)) => base64 = value == "1",
                _ => {}
            }
        }
        if kind == "?" {
            let end = if terminator == BEL { "\x07" } else { "\x1b\\" };
            return Some(Found::Reply(
                format!("\x1b]99;i={id}:p=?;p=title,body{end}").into_bytes(),
            ));
        }
        let data = if base64 {
            String::from_utf8_lossy(&STANDARD.decode(data.trim()).ok()?).into_owned()
        } else {
            data.to_owned()
        };
        if !self.kitty.contains_key(id) && self.kitty.len() >= MAX_PENDING {
            self.kitty.clear();
        }
        let pending = self.kitty.entry(id.to_owned()).or_default();
        let target = match kind {
            "title" => &mut pending.title,
            "body" => &mut pending.body,
            _ => return None,
        };
        if target.len() + data.len() <= MAX_TEXT {
            target.push_str(&data);
        }
        if !done {
            return None;
        }
        let pending = self.kitty.remove(id)?;
        Some(Found::Notify {
            title: pending.title,
            body: pending.body,
        })
    }
}
