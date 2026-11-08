use config::Appearance;

use crate::app::{App, HostInfo, Rgb};
use crate::host_input::Report;
use crate::panes::Link;

pub const COLOR_QUERY: &str = "\x1b]10;?\x1b\\\x1b]11;?\x1b\\\x1b]12;?\x1b\\";
pub const SCHEME_QUERY: &str = "\x1b[?996n";

pub fn inferred(bg: Rgb) -> Appearance {
    let luma = u32::from(bg.0) * 299 + u32::from(bg.1) * 587 + u32::from(bg.2) * 114;
    if luma >= 128_000 {
        Appearance::Light
    } else {
        Appearance::Dark
    }
}

impl HostInfo {
    pub fn appearance(&self) -> Option<Appearance> {
        self.scheme.or_else(|| self.bg.map(inferred))
    }
}

impl App {
    pub fn host_report(&mut self, report: Report) {
        match report {
            Report::Scheme(appearance) => {
                self.host.scheme = Some(appearance);
                self.query_host_colors();
            }
            Report::Color { code, rgb } => {
                let slot = match code {
                    10 => &mut self.host.fg,
                    11 => &mut self.host.bg,
                    _ => &mut self.host.cursor,
                };
                if *slot == Some(rgb) {
                    return;
                }
                *slot = Some(rgb);
                self.send_facts();
            }
        }
        self.refresh_palette();
    }

    pub fn refresh_palette(&mut self) {
        let palette = self.config.palette(self.host.appearance());
        if palette != self.palette {
            self.palette = palette;
            self.dirty = true;
        }
    }

    pub fn query_host_colors(&mut self) {
        self.out.extend_from_slice(COLOR_QUERY.as_bytes());
    }

    pub fn query_host_scheme(&mut self) {
        self.out.extend_from_slice(SCHEME_QUERY.as_bytes());
    }

    fn send_facts(&mut self) {
        let display = self.display();
        for pane in self.panes.iter() {
            if pane.link != Link::Attached {
                continue;
            }
            if let Some(writer) = &pane.writer
                && let Err(error) = writer.facts(&display)
            {
                tracing::debug!(address = %pane.address, %error, "facts dropped");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backgrounds_infer_the_appearance() {
        assert_eq!(inferred(Rgb(0x1e, 0x1e, 0x2e)), Appearance::Dark);
        assert_eq!(inferred(Rgb(0xef, 0xf1, 0xf5)), Appearance::Light);
        assert_eq!(inferred(Rgb(128, 128, 128)), Appearance::Light);
        assert_eq!(inferred(Rgb(127, 127, 127)), Appearance::Dark);
        let explicit = HostInfo {
            bg: Some(Rgb(255, 255, 255)),
            scheme: Some(Appearance::Dark),
            ..HostInfo::default()
        };
        assert_eq!(explicit.appearance(), Some(Appearance::Dark));
        assert_eq!(HostInfo::default().appearance(), None);
    }
}
