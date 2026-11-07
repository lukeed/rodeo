use std::io::{self, Write};
use std::os::fd::AsFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
    PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{cursor, execute};

use crate::app::{HostInfo, Rgb};

static ENTERED: AtomicBool = AtomicBool::new(false);
static PUSHED: AtomicBool = AtomicBool::new(false);

const PROBE: Duration = Duration::from_millis(300);

pub struct Guard(());

impl Drop for Guard {
    fn drop(&mut self) {
        restore();
    }
}

pub fn enter() -> io::Result<Guard> {
    enable_raw_mode()?;
    ENTERED.store(true, Ordering::SeqCst);
    let mut out = io::stdout();
    execute!(
        out,
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableFocusChange
    )?;
    if crossterm::terminal::supports_keyboard_enhancement().unwrap_or(false) {
        execute!(
            out,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
            )
        )?;
        PUSHED.store(true, Ordering::SeqCst);
    }
    Ok(Guard(()))
}

pub fn restore() {
    if !ENTERED.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut out = io::stdout();
    if PUSHED.swap(false, Ordering::SeqCst) {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(
        out,
        DisableMouseCapture,
        DisableFocusChange,
        DisableBracketedPaste,
        LeaveAlternateScreen,
        cursor::Show
    );
    let _ = out.write_all(b"\x1b[0 q");
    let _ = out.flush();
    let _ = disable_raw_mode();
}

pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        tracing::error!(%info, "panic");
        previous(info);
    }));
}

pub fn probe() -> HostInfo {
    let mut info = HostInfo::default();
    if let Ok(size) = crossterm::terminal::window_size()
        && size.columns > 0
        && size.rows > 0
        && size.width > 0
        && size.height > 0
    {
        info.cell_px = (
            u32::from(size.width / size.columns),
            u32::from(size.height / size.rows),
        );
    }
    let mut out = io::stdout();
    if out
        .write_all(b"\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\\x1b[c")
        .and_then(|()| out.flush())
        .is_err()
    {
        return info;
    }
    let replies = read_replies();
    let (fg, bg, cursor) = parse_colors(&replies);
    info.fg = fg;
    info.bg = bg;
    info.cursor = cursor;
    info
}

fn read_replies() -> Vec<u8> {
    let stdin = io::stdin();
    let fd = stdin.as_fd();
    let started = Instant::now();
    let mut got = Vec::new();
    let mut buf = [0u8; 512];
    while started.elapsed() < PROBE {
        let left = PROBE.saturating_sub(started.elapsed());
        let timeout = rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: i64::try_from(left.as_nanos()).unwrap_or(0),
        };
        let mut fds = [rustix::event::PollFd::new(
            &fd,
            rustix::event::PollFlags::IN,
        )];
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        match rustix::io::read(fd, &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => got.extend_from_slice(&buf[..n]),
        }
        if has_device_attributes(&got) {
            break;
        }
    }
    got
}

fn has_device_attributes(bytes: &[u8]) -> bool {
    bytes.windows(3).enumerate().any(|(i, w)| {
        w == b"\x1b[?"
            && bytes[i + 3..]
                .iter()
                .take_while(|b| b.is_ascii_digit() || **b == b';')
                .count()
                > 0
            && bytes[i + 3..]
                .iter()
                .find(|b| !(b.is_ascii_digit() || **b == b';'))
                == Some(&b'c')
    })
}

pub fn parse_colors(bytes: &[u8]) -> (Option<Rgb>, Option<Rgb>, Option<Rgb>) {
    let text = String::from_utf8_lossy(bytes);
    let find = |code: &str| {
        let marker = format!("\x1b]{code};rgb:");
        let at = text.find(&marker)? + marker.len();
        let rest = &text[at..];
        let end = rest.find(['\x1b', '\x07'])?;
        parse_rgb(&rest[..end])
    };
    (find("10"), find("11"), find("12"))
}

fn parse_rgb(value: &str) -> Option<Rgb> {
    let mut parts = value.split('/').map(|part| {
        let n = u32::from_str_radix(part, 16).ok()?;
        let max = (1u32 << (4 * u32::try_from(part.len()).ok()?)) - 1;
        u8::try_from(n * 255 / max.max(1)).ok()
    });
    Some(Rgb(parts.next()??, parts.next()??, parts.next()??))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_color_replies_parse() {
        let replies = b"\x1b]10;rgb:cdcd/d6d6/f4f4\x1b\\\x1b]11;rgb:1e/1e/2e\x07\x1b[?62;22c";
        let (fg, bg, cursor) = parse_colors(replies);
        assert_eq!(fg, Some(Rgb(0xcd, 0xd6, 0xf4)));
        assert_eq!(bg, Some(Rgb(0x1e, 0x1e, 0x2e)));
        assert_eq!(cursor, None);
        assert!(has_device_attributes(replies));
        assert!(!has_device_attributes(b"\x1b]10;rgb:0/0/0\x07"));
    }
}
