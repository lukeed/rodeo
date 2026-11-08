use std::process::Stdio;
use std::time::Duration;

use crossterm::event::KeyEvent;
use tokio::io::AsyncReadExt;

use crate::app::App;

pub const MAX_IMAGE: usize = 8 * 1024 * 1024;
const READ_TIMEOUT: Duration = Duration::from_secs(3);
const KINDS: [(&str, &str); 6] = [
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/jpg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/bmp", "bmp"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

pub fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

pub fn is_image(extension: &str, bytes: &[u8]) -> bool {
    match extension {
        "png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "jpg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "bmp" => {
            bytes.len() >= 26 && bytes.starts_with(b"BM") && {
                let offset = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]);
                usize::try_from(offset).is_ok_and(|o| (26..=bytes.len()).contains(&o))
            }
        }
        _ => false,
    }
}

async fn output(env: &hc::Env, program: &str, args: &[&str]) -> Option<Vec<u8>> {
    let mut child = env
        .command(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let read = async {
        let mut bytes = Vec::new();
        let limit = u64::try_from(MAX_IMAGE + 1).unwrap_or(u64::MAX);
        (&mut stdout)
            .take(limit)
            .read_to_end(&mut bytes)
            .await
            .ok()?;
        let status = child.wait().await.ok()?;
        (status.success() && !bytes.is_empty() && bytes.len() <= MAX_IMAGE).then_some(bytes)
    };
    tokio::time::timeout(READ_TIMEOUT, read)
        .await
        .ok()
        .flatten()
}

async fn read_unix(env: &hc::Env) -> Option<Image> {
    let wayland = env.get("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty());
    let x11 = env.get("DISPLAY").is_some_and(|v| !v.is_empty());
    for (mime, extension) in KINDS {
        let tries: [(bool, &str, Vec<&str>); 2] = [
            (wayland, "wl-paste", vec!["--no-newline", "--type", mime]),
            (
                x11,
                "xclip",
                vec!["-selection", "clipboard", "-t", mime, "-o"],
            ),
        ];
        for (usable, program, args) in tries {
            if !usable {
                continue;
            }
            if let Some(bytes) = output(env, program, &args).await
                && is_image(extension, &bytes)
            {
                return Some(Image { bytes, extension });
            }
        }
    }
    None
}

async fn read_macos(env: &hc::Env) -> Option<Image> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let path = std::env::temp_dir().join(format!(
        "drover-clipboard-{}-{nanos}.png",
        std::process::id()
    ));
    let script = format!(
        "set png_data to (the clipboard as «class PNGf»)\nset fp to open for access POSIX file \"{}\" with write permission\nwrite png_data to fp\nclose access fp",
        path.display()
    );
    output(env, "osascript", &["-e", &script]).await;
    let bytes = tokio::fs::read(&path).await.ok();
    let _ = tokio::fs::remove_file(&path).await;
    let bytes = bytes?;
    (bytes.len() <= MAX_IMAGE && is_image("png", &bytes)).then_some(Image {
        bytes,
        extension: "png",
    })
}

pub async fn read_image(env: &hc::Env) -> Option<Image> {
    if cfg!(target_os = "macos") {
        read_macos(env).await
    } else {
        read_unix(env).await
    }
}

impl App {
    pub fn paste_target(&self) -> Option<String> {
        match &self.popup {
            Some(popup) => popup.address(),
            None => self.focused_address(),
        }
    }

    pub fn image_paste(&mut self, key: Option<KeyEvent>) {
        let Some(address) = self.paste_target() else {
            return;
        };
        let env = self.env.clone();
        let client = self.client();
        self.spawn(async move {
            let image = read_image(&env).await;
            let put = match (image, client) {
                (Some(image), Some(client)) => {
                    let name = format!("clipboard.{}", image.extension);
                    Some(client.put(&address, &name, &image.bytes).await)
                }
                _ => None,
            };
            move |app: &mut App| match put {
                Some(Ok(put)) => {
                    let text = format!("{} ", shell_quote(&put.path));
                    app.paste_into(&address, &text);
                }
                Some(Err(error)) => app.show_error(format!("cannot paste the image: {error}")),
                None => match key {
                    Some(key) => app.key_into(&address, &key),
                    None => app.paste_into(&address, ""),
                },
            }
        });
    }

    fn with_term(&mut self, address: &str, write: impl FnOnce(&mut crate::panes::Pane)) {
        if let Some(popup) = self.popup.as_mut()
            && let Some(pane) = popup.pane.as_mut()
            && pane.address == address
        {
            write(pane);
        } else if let Some(pane) = self.panes.get_mut(address) {
            write(pane);
        }
    }

    pub fn paste_into(&mut self, address: &str, text: &str) {
        self.with_term(address, |pane| {
            let bytes = pane.term.paste(text);
            pane.term.scroll_to_bottom();
            pane.input(&bytes);
        });
    }

    pub fn key_into(&mut self, address: &str, key: &KeyEvent) {
        self.with_term(address, |pane| {
            let bytes = pane.term.key(key);
            if !bytes.is_empty() {
                pane.term.scroll_to_bottom();
                pane.input(&bytes);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_for_any_shell() {
        assert_eq!(shell_quote("/a b/c.png"), "'/a b/c.png'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn checks_image_signatures() {
        assert!(is_image("png", b"\x89PNG\r\n\x1a\nrest"));
        assert!(!is_image("png", b"text"));
        assert!(is_image("jpg", &[0xFF, 0xD8, 0xFF, 0x00]));
        assert!(is_image("webp", b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(!is_image("bmp", b"BM"));
    }
}
