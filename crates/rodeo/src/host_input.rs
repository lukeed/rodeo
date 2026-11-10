use std::os::fd::AsFd;
use std::time::Duration;

use config::Appearance;
use crossterm::event::Event;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::app::{App, Msg, Rgb};

const ESC: u8 = 0x1b;
const LONE_ESC: Duration = Duration::from_millis(30);
const PASTE_END: &[u8] = b"\x1b[201~";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Color { code: u8, rgb: Rgb },
    Scheme(Appearance),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Event(Event),
    Report(Report),
}

#[derive(Debug, Default)]
pub struct Splitter {
    buf: Vec<u8>,
}

enum Frame {
    Done(usize),
    Partial,
}

impl Splitter {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Piece> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        let mut at = 0;
        while at < self.buf.len() {
            match frame(&self.buf[at..]) {
                Frame::Done(len) => {
                    out.extend(decode(&self.buf[at..at + len]));
                    at += len;
                }
                Frame::Partial => break,
            }
        }
        self.buf.drain(..at);
        out
    }

    pub fn pending(&self) -> bool {
        !self.buf.is_empty()
    }

    pub fn flush(&mut self) -> Vec<Piece> {
        let rest = std::mem::take(&mut self.buf);
        if rest.first() != Some(&ESC) {
            return self.feed(&rest);
        }
        let mut out = vec![Piece::Event(Event::Key(
            crossterm::event::KeyCode::Esc.into(),
        ))];
        out.extend(self.feed(&rest[1..]));
        out
    }
}

fn frame(bytes: &[u8]) -> Frame {
    let Some(&first) = bytes.first() else {
        return Frame::Partial;
    };
    if first != ESC {
        return utf8_len(first).map_or(Frame::Done(1), |len| {
            if bytes.len() >= len {
                Frame::Done(len)
            } else {
                Frame::Partial
            }
        });
    }
    let Some(&second) = bytes.get(1) else {
        return Frame::Partial;
    };
    match second {
        b'[' => csi(bytes),
        b']' | b'P' | b'_' | b'^' | b'X' => string(bytes),
        b'O' => {
            if bytes.len() >= 3 {
                Frame::Done(3)
            } else {
                Frame::Partial
            }
        }
        ESC => Frame::Done(1),
        other => utf8_len(other).map_or(Frame::Done(2), |len| {
            if bytes.len() > len {
                Frame::Done(1 + len)
            } else {
                Frame::Partial
            }
        }),
    }
}

fn csi(bytes: &[u8]) -> Frame {
    let mut at = 2;
    while let Some(&b) = bytes.get(at) {
        match b {
            0x20..=0x3f => at += 1,
            0x40..=0x7e => {
                if b == b'M' && at == 2 {
                    return if bytes.len() >= 6 {
                        Frame::Done(6)
                    } else {
                        Frame::Partial
                    };
                }
                if &bytes[2..=at] == b"200~" {
                    return bytes[at + 1..]
                        .windows(PASTE_END.len())
                        .position(|w| w == PASTE_END)
                        .map_or(Frame::Partial, |end| {
                            Frame::Done(at + 1 + end + PASTE_END.len())
                        });
                }
                return Frame::Done(at + 1);
            }
            _ => return Frame::Done(at),
        }
    }
    Frame::Partial
}

fn string(bytes: &[u8]) -> Frame {
    let mut at = 2;
    while let Some(&b) = bytes.get(at) {
        match b {
            0x07 => return Frame::Done(at + 1),
            ESC => match bytes.get(at + 1) {
                Some(b'\\') => return Frame::Done(at + 2),
                Some(_) => return Frame::Done(at),
                None => return Frame::Partial,
            },
            _ => at += 1,
        }
    }
    Frame::Partial
}

fn utf8_len(first: u8) -> Option<usize> {
    match first {
        0xc0..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf7 => Some(4),
        _ => None,
    }
}

fn decode(token: &[u8]) -> Option<Piece> {
    if let Some(report) = report(token) {
        return Some(Piece::Report(report));
    }
    if is_reply(token) {
        return None;
    }
    let event = terminput::Event::parse_from(token).ok().flatten()?;
    terminput_crossterm::to_crossterm(event)
        .ok()
        .map(Piece::Event)
}

fn is_reply(token: &[u8]) -> bool {
    match token.get(1) {
        Some(b']' | b'P' | b'_' | b'^' | b'X') => true,
        Some(b'[') => {
            let last = token.last().copied().unwrap_or_default();
            let private = token.get(2) == Some(&b'?');
            (private && matches!(last, b'c' | b'u' | b'n' | b'y'))
                || last == b'R'
                || last == b't'
                || (last == b'y' && token.contains(&b'$'))
        }
        _ => false,
    }
}

pub fn report(token: &[u8]) -> Option<Report> {
    if let Some(body) = token.strip_prefix(b"\x1b[?997;") {
        return match body {
            b"1n" => Some(Report::Scheme(Appearance::Dark)),
            b"2n" => Some(Report::Scheme(Appearance::Light)),
            _ => None,
        };
    }
    let body = token.strip_prefix(b"\x1b]")?;
    let body = body
        .strip_suffix(b"\x1b\\")
        .or_else(|| body.strip_suffix(b"\x07"))?;
    let text = std::str::from_utf8(body).ok()?;
    let (code, value) = text.split_once(';')?;
    let code: u8 = code.parse().ok()?;
    if !(10..=12).contains(&code) {
        return None;
    }
    Some(Report::Color {
        code,
        rgb: parse_color(value)?,
    })
}

pub fn parse_color(value: &str) -> Option<Rgb> {
    let parts: Vec<&str> = if let Some(rgb) = value.strip_prefix("rgb:") {
        rgb.split('/').collect()
    } else {
        let hex = value.strip_prefix('#')?;
        let digits = hex.len() / 3;
        if !(1..=4).contains(&digits) || hex.len() != digits * 3 || !hex.is_ascii() {
            return None;
        }
        vec![&hex[..digits], &hex[digits..2 * digits], &hex[2 * digits..]]
    };
    let [r, g, b] = parts.as_slice() else {
        return None;
    };
    Some(Rgb(component(r)?, component(g)?, component(b)?))
}

fn component(part: &str) -> Option<u8> {
    if part.is_empty() || part.len() > 4 || !part.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(part, 16).ok()?;
    let max = (1u32 << (4 * part.len())) - 1;
    u8::try_from((value * 255 + max / 2) / max).ok()
}

pub fn start(tx: UnboundedSender<Msg>) {
    let (pieces, mut rx) = unbounded_channel::<Option<Piece>>();
    let spawned = std::thread::Builder::new()
        .name("rodeo-input".into())
        .spawn(move || read_loop(&pieces));
    if let Err(error) = spawned {
        tracing::error!(%error, "cannot start the input thread");
        let _ = tx.send(Msg::Quit);
        return;
    }
    let input = tx.clone();
    tokio::task::spawn_local(async move {
        while let Some(piece) = rx.recv().await {
            let msg = match piece {
                Some(Piece::Event(event)) => {
                    let gained = matches!(event, Event::FocusGained);
                    if input.send(Msg::Host(event)).is_err() {
                        return;
                    }
                    if !gained {
                        continue;
                    }
                    apply(App::query_host_scheme)
                }
                Some(Piece::Report(report)) => apply(move |app| app.host_report(report)),
                None => Msg::Quit,
            };
            if input.send(msg).is_err() {
                return;
            }
        }
    });
    tokio::task::spawn_local(async move {
        use tokio::signal::unix::{SignalKind, signal};
        let Ok(mut winch) = signal(SignalKind::window_change()) else {
            return;
        };
        while winch.recv().await.is_some() {
            let Ok((cols, rows)) = crossterm::terminal::size() else {
                continue;
            };
            if tx.send(Msg::Host(Event::Resize(cols, rows))).is_err()
                || tx.send(apply(App::host_resized)).is_err()
            {
                return;
            }
        }
    });
}

fn apply(apply: impl FnOnce(&mut App) + 'static) -> Msg {
    Msg::Apply {
        generation: None,
        apply: Box::new(apply),
    }
}

fn read_loop(out: &UnboundedSender<Option<Piece>>) {
    let stdin = std::io::stdin();
    let fd = stdin.as_fd();
    let mut splitter = Splitter::default();
    let mut buf = [0u8; 4096];
    loop {
        let wait = splitter.pending().then(|| rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: i64::try_from(LONE_ESC.as_nanos()).unwrap_or(0),
        });
        let mut fds = [rustix::event::PollFd::new(
            &fd,
            rustix::event::PollFlags::IN,
        )];
        let ready = match rustix::event::poll(&mut fds, wait.as_ref()) {
            Ok(n) => n,
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => {
                tracing::error!(%error, "host input poll failed");
                break;
            }
        };
        let pieces = if ready == 0 {
            splitter.flush()
        } else {
            match rustix::io::read(fd, &mut buf) {
                Ok(0) => break,
                Ok(n) => splitter.feed(&buf[..n]),
                Err(rustix::io::Errno::INTR | rustix::io::Errno::AGAIN) => continue,
                Err(error) => {
                    tracing::error!(%error, "host input read failed");
                    break;
                }
            }
        };
        for piece in pieces {
            if out.send(Some(piece)).is_err() {
                return;
            }
        }
    }
    let _ = out.send(None);
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};

    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Piece {
        Piece::Event(Event::Key(KeyEvent::new(code, modifiers)))
    }

    #[test]
    fn host_reports_never_become_keys() {
        let mut s = Splitter::default();
        let pieces = s.feed(
            b"a\x1b]11;rgb:ffff/ffff/ffff\x1b\\\x1b[?997;2nb\x1b]10;#000\x07\x1b[?62;22c\x1b_Gi=1;OK\x1b\\c",
        );
        assert_eq!(
            pieces,
            [
                key(KeyCode::Char('a'), KeyModifiers::NONE),
                Piece::Report(Report::Color {
                    code: 11,
                    rgb: Rgb(255, 255, 255)
                }),
                Piece::Report(Report::Scheme(Appearance::Light)),
                key(KeyCode::Char('b'), KeyModifiers::NONE),
                Piece::Report(Report::Color {
                    code: 10,
                    rgb: Rgb(0, 0, 0)
                }),
                key(KeyCode::Char('c'), KeyModifiers::NONE),
            ]
        );
    }

    #[test]
    fn split_sequences_wait_for_the_rest() {
        let mut s = Splitter::default();
        assert_eq!(s.feed(b"\x1b]11;rgb:00"), []);
        assert_eq!(s.feed(b"00/0000/0000\x1b"), []);
        assert_eq!(
            s.feed(b"\\x"),
            [
                Piece::Report(Report::Color {
                    code: 11,
                    rgb: Rgb(0, 0, 0)
                }),
                key(KeyCode::Char('x'), KeyModifiers::NONE),
            ]
        );
        assert_eq!(s.feed(b"\x1b[1;5"), []);
        assert_eq!(s.feed(b"A"), [key(KeyCode::Up, KeyModifiers::CONTROL)]);
        assert_eq!(s.feed("é".as_bytes()[..1].as_ref()), []);
        assert_eq!(
            s.feed(&"é".as_bytes()[1..]),
            [key(KeyCode::Char('é'), KeyModifiers::NONE)]
        );
    }

    #[test]
    fn a_lone_escape_flushes_as_the_escape_key() {
        let mut s = Splitter::default();
        assert_eq!(s.feed(b"\x1b"), []);
        assert!(s.pending());
        assert_eq!(s.flush(), [key(KeyCode::Esc, KeyModifiers::NONE)]);
        assert_eq!(
            s.feed(b"\x1bx\x1b\x1b"),
            [
                key(KeyCode::Char('x'), KeyModifiers::ALT),
                key(KeyCode::Esc, KeyModifiers::NONE)
            ]
        );
        assert_eq!(s.flush(), [key(KeyCode::Esc, KeyModifiers::NONE)]);
        assert_eq!(s.feed(b"\x1b["), []);
        assert_eq!(
            s.flush(),
            [
                key(KeyCode::Esc, KeyModifiers::NONE),
                key(KeyCode::Char('['), KeyModifiers::NONE)
            ]
        );
    }

    #[test]
    fn keys_mouse_focus_and_paste_still_parse() {
        let mut s = Splitter::default();
        let pieces =
            s.feed(b"\x1b[<0;10;5M\x1b[I\x1b[O\x1b[97;5u\r\x7f\x1b[200~hi\x1b[0mthere\x1b[201~");
        assert!(matches!(
            &pieces[0],
            Piece::Event(Event::Mouse(m)) if m.kind == MouseEventKind::Down(crossterm::event::MouseButton::Left) && m.column == 9 && m.row == 4
        ));
        assert_eq!(pieces[1], Piece::Event(Event::FocusGained));
        assert_eq!(pieces[2], Piece::Event(Event::FocusLost));
        assert_eq!(pieces[3], key(KeyCode::Char('a'), KeyModifiers::CONTROL));
        assert_eq!(pieces[4], key(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(pieces[5], key(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(
            pieces[6],
            Piece::Event(Event::Paste("hi\x1b[0mthere".into()))
        );
        assert_eq!(pieces.len(), 7);
    }

    #[test]
    fn focus_reports_parse_however_the_host_splits_them() {
        let out = Piece::Event(Event::FocusLost);
        let back = Piece::Event(Event::FocusGained);
        let k = key(KeyCode::Char('k'), KeyModifiers::NONE);
        let splits: [&[&[u8]]; 5] = [
            &[b"\x1b[O\x1b[Ik"],
            &[b"\x1b[O", b"\x1b[I", b"k"],
            &[b"\x1b[O\x1b", b"[Ik"],
            &[b"\x1b[O\x1b[", b"I", b"k"],
            &[b"\x1b", b"[", b"O", b"\x1b", b"[", b"I", b"k"],
        ];
        for chunks in splits {
            let mut s = Splitter::default();
            let pieces: Vec<Piece> = chunks.iter().flat_map(|c| s.feed(c)).collect();
            assert_eq!(pieces, [out.clone(), back.clone(), k.clone()], "{chunks:?}");
            assert!(!s.pending(), "{chunks:?}");
        }
    }

    #[test]
    fn colors_parse_in_both_forms() {
        assert_eq!(
            parse_color("rgb:cccc/dddd/eeee"),
            Some(Rgb(0xcc, 0xdd, 0xee))
        );
        assert_eq!(parse_color("rgb:1e/1e/2e"), Some(Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(parse_color("#123456"), Some(Rgb(0x12, 0x34, 0x56)));
        assert_eq!(parse_color("#fff"), Some(Rgb(255, 255, 255)));
        assert_eq!(parse_color("rgb:1/2"), None);
        assert_eq!(parse_color("red"), None);
    }
}
