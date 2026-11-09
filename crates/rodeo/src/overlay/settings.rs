use config::{Config, Edit, Palette, StatusIndicators, THEME_NAMES, ToastDelivery};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};

use super::frame::{button, contrast, panel, popup, row, text};
use super::{Ctx, Drawn, Kind, Outcome, Overlay, Request};
use crate::text::width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Theme,
    Indicators,
    Sound,
    Toast,
}

const SECTIONS: [Section; 4] = [
    Section::Theme,
    Section::Indicators,
    Section::Sound,
    Section::Toast,
];

const TOASTS: [ToastDelivery; 4] = [
    ToastDelivery::Off,
    ToastDelivery::Herdr,
    ToastDelivery::Terminal,
    ToastDelivery::System,
];

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Indicators => "indicators",
            Self::Sound => "sound",
            Self::Toast => "toasts",
        }
    }

    fn count(self) -> usize {
        match self {
            Self::Theme => THEME_NAMES.len(),
            Self::Indicators | Self::Sound => 2,
            Self::Toast => TOASTS.len(),
        }
    }

    fn current(self, config: &Config) -> usize {
        match self {
            Self::Theme => theme_index(&config.theme.active_name(None)),
            Self::Indicators => {
                usize::from(config.ui.status_indicators == StatusIndicators::Symbols)
            }
            Self::Sound => usize::from(!config.ui.sound.enabled),
            Self::Toast => TOASTS
                .iter()
                .position(|d| *d == config.ui.toast.delivery)
                .unwrap_or(0),
        }
    }

    fn edit(self, index: usize) -> Edit {
        match self {
            Self::Theme => Edit::Theme(THEME_NAMES[index.min(THEME_NAMES.len() - 1)].to_owned()),
            Self::Indicators => Edit::StatusIndicators(if index == 0 {
                StatusIndicators::Dots
            } else {
                StatusIndicators::Symbols
            }),
            Self::Sound => Edit::Sound(index == 0),
            Self::Toast => Edit::ToastDelivery(TOASTS[index.min(TOASTS.len() - 1)]),
        }
    }

    fn copy(self) -> Option<(&'static str, &'static str, &'static [&'static str])> {
        match self {
            Self::Theme => None,
            Self::Indicators => Some((
                "agent status indicators",
                "choose color dots or distinct symbols for each state",
                &["color dots  ● ● ● ○ ·", "distinct symbols  × ◐ ✓ ○ ·"],
            )),
            Self::Sound => Some((
                "sound alerts",
                "play sounds when agents change state in background",
                &["on", "off"],
            )),
            Self::Toast => Some((
                "notification popups",
                "choose where background popup notifications should appear",
                &["off", "inside herdr", "via terminal", "via system"],
            )),
        }
    }
}

fn normalized(name: &str) -> String {
    name.to_lowercase().replace([' ', '_'], "-")
}

fn theme_index(name: &str) -> usize {
    let name = config::canonical_theme_name(name).map_or_else(|| normalized(name), str::to_owned);
    THEME_NAMES
        .iter()
        .position(|t| normalized(t) == name)
        .unwrap_or(0)
}

pub struct Settings {
    section: Section,
    selected: usize,
    original: usize,
    previewed: bool,
    popup: Rect,
    tabs: Vec<(Rect, Section)>,
    choices: Vec<(Rect, usize)>,
    apply: Rect,
    close: Rect,
}

impl Settings {
    pub fn new(config: &Config) -> Self {
        let original = Section::Theme.current(config);
        Self {
            section: Section::Theme,
            selected: original,
            original,
            previewed: false,
            popup: Rect::default(),
            tabs: Vec::new(),
            choices: Vec::new(),
            apply: Rect::default(),
            close: Rect::default(),
        }
    }

    pub fn section(&self) -> Section {
        self.section
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    fn select_section(&mut self, section: Section, config: &Config) {
        self.section = section;
        self.selected = section.current(config);
    }

    fn move_section(&mut self, delta: isize, config: &Config) {
        let at = SECTIONS
            .iter()
            .position(|s| *s == self.section)
            .unwrap_or(0);
        let next = (at + SECTIONS.len()).saturating_add_signed(delta) % SECTIONS.len();
        self.select_section(SECTIONS[next], config);
    }

    fn choose(&mut self, index: usize) -> Outcome {
        self.selected = index.min(self.section.count() - 1);
        if self.section != Section::Theme {
            return Outcome::Stay;
        }
        self.previewed = true;
        Outcome::Keep(Request::Preview(Some(
            THEME_NAMES[self.selected].to_owned(),
        )))
    }

    fn apply(&self) -> Outcome {
        let request = Request::Settings(self.section.edit(self.selected));
        if self.section == Section::Theme {
            Outcome::Done(request)
        } else {
            Outcome::Keep(request)
        }
    }

    fn cancel(&self) -> Outcome {
        if self.previewed {
            Outcome::Done(Request::Preview(None))
        } else {
            Outcome::Close
        }
    }

    fn draw_choice(
        buf: &mut Buffer,
        rect: Rect,
        label: &str,
        selected: bool,
        current: bool,
        p: &Palette,
    ) {
        let style = if selected {
            Style::new()
                .fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(p.text).bg(p.panel_bg)
        };
        buf.set_style(rect, style);
        let marker = if selected { "▸" } else { " " };
        let check = if current { " ✓" } else { "" };
        text(
            buf,
            rect.x,
            rect.y,
            rect.width,
            &format!(" {marker} {label}{check}"),
            style,
        );
    }

    fn draw_content(&mut self, buf: &mut Buffer, area: Rect, p: &Palette) {
        let Some((title, description, choices)) = self.section.copy() else {
            let visible = usize::from(area.height);
            let scroll = self.selected.saturating_sub(visible.saturating_sub(1));
            for (shown, (index, name)) in THEME_NAMES
                .iter()
                .enumerate()
                .skip(scroll)
                .take(visible)
                .enumerate()
            {
                let rect = Rect::new(
                    area.x,
                    area.y + u16::try_from(shown).unwrap_or(0),
                    area.width,
                    1,
                );
                Self::draw_choice(
                    buf,
                    rect,
                    name,
                    index == self.selected,
                    index == self.original,
                    p,
                );
                self.choices.push((rect, index));
            }
            return;
        };
        text(
            buf,
            area.x,
            area.y,
            area.width,
            title,
            Style::new()
                .fg(p.text)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD),
        );
        text(
            buf,
            area.x,
            area.y + 1,
            area.width,
            description,
            Style::new().fg(p.overlay1).bg(p.panel_bg),
        );
        let gap = u16::from(choices.len() > 2);
        for (index, choice) in choices.iter().enumerate() {
            let y = area.y + 3 + u16::try_from(index).unwrap_or(0) * (1 + gap);
            if y >= area.bottom() {
                break;
            }
            let rect = Rect::new(area.x, y, area.width, 1);
            Self::draw_choice(buf, rect, choice, index == self.selected, false, p);
            self.choices.push((rect, index));
        }
    }
}

impl Overlay for Settings {
    fn kind(&self) -> Kind {
        Kind::Settings
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer, ctx: &Ctx<'_>) -> Option<Drawn> {
        let p = ctx.palette;
        let outer = popup(area, 76, 22)?;
        let inner = panel(buf, outer, p.accent, p.panel_bg)?;
        self.popup = outer;
        self.tabs.clear();
        self.choices.clear();
        if inner.width < 20 || inner.height < 8 {
            return Some(Drawn::new(outer, None));
        }
        text(
            buf,
            inner.x,
            inner.y,
            inner.width,
            " settings",
            Style::new()
                .fg(p.text)
                .bg(p.panel_bg)
                .add_modifier(Modifier::BOLD),
        );
        let mut x = inner.x;
        for section in SECTIONS {
            let label = format!(" {} ", section.label());
            let w = width(&label).min(inner.right().saturating_sub(x));
            let rect = Rect::new(x, inner.y + 1, w, 1);
            let style = if section == self.section {
                Style::new()
                    .fg(contrast(p))
                    .bg(p.accent)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(p.overlay1).bg(p.panel_bg)
            };
            buf.set_style(rect, style);
            text(buf, rect.x, rect.y, rect.width, &label, style);
            self.tabs.push((rect, section));
            x = x.saturating_add(w + 1);
            if x >= inner.right() {
                break;
            }
        }
        text(
            buf,
            inner.x,
            inner.y + 2,
            inner.width,
            &"─".repeat(usize::from(inner.width)),
            Style::new().fg(p.surface0).bg(p.panel_bg),
        );
        let content = Rect::new(
            inner.x,
            inner.y + 4,
            inner.width,
            inner.height.saturating_sub(7),
        );
        self.draw_content(buf, content, p);
        let buttons = row(inner, &[10, 12], 2, inner.height.saturating_sub(1));
        (self.apply, self.close) = (buttons[0], buttons[1]);
        button(
            buf,
            self.apply,
            " ↵ apply ",
            Style::new()
                .fg(contrast(p))
                .bg(p.accent)
                .add_modifier(Modifier::BOLD),
        );
        button(
            buf,
            self.close,
            " esc close ",
            Style::new()
                .fg(p.text)
                .bg(p.surface0)
                .add_modifier(Modifier::BOLD),
        );
        text(
            buf,
            inner.x,
            inner.bottom().saturating_sub(2),
            inner.width,
            " ↑↓ select  tab section",
            Style::new().fg(p.overlay1).bg(p.panel_bg),
        );
        Some(Drawn::new(outer, None))
    }

    fn key(&mut self, key: &KeyEvent, ctx: &Ctx<'_>) -> Outcome {
        let bare = key.modifiers.is_empty();
        let shifted = key.modifiers.difference(KeyModifiers::SHIFT).is_empty();
        match key.code {
            KeyCode::Esc => self.cancel(),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') if bare => {
                self.move_section(1, ctx.config);
                Outcome::Stay
            }
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') if shifted => {
                self.move_section(-1, ctx.config);
                Outcome::Stay
            }
            KeyCode::Up | KeyCode::Char('k') if bare => {
                self.choose(self.selected.saturating_sub(1))
            }
            KeyCode::Down | KeyCode::Char('j') if bare => self.choose(self.selected + 1),
            KeyCode::Enter | KeyCode::Char(' ') if bare => self.apply(),
            _ => Outcome::Stay,
        }
    }

    fn mouse(&mut self, event: &MouseEvent, ctx: &Ctx<'_>) -> Outcome {
        if event.kind != MouseEventKind::Down(MouseButton::Left) {
            return Outcome::Stay;
        }
        let at = Position::new(event.column, event.row);
        if let Some((_, section)) = self.tabs.iter().find(|(r, _)| r.contains(at)) {
            let section = *section;
            self.select_section(section, ctx.config);
            return Outcome::Stay;
        }
        if let Some((_, index)) = self.choices.iter().find(|(r, _)| r.contains(at)) {
            let index = *index;
            let chosen = self.choose(index);
            return if self.section == Section::Theme {
                chosen
            } else {
                self.apply()
            };
        }
        if self.apply.contains(at) {
            return self.apply();
        }
        if self.close.contains(at) || !self.popup.contains(at) {
            return self.cancel();
        }
        Outcome::Stay
    }
}

pub fn open(kind: &Kind, ctx: &Ctx<'_>) -> Option<Box<dyn Overlay>> {
    (*kind == Kind::Settings).then(|| Box::new(Settings::new(ctx.config)) as Box<dyn Overlay>)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_names_find_their_row() {
        assert_eq!(theme_index("catppuccin"), 0);
        assert_eq!(theme_index("Tokyo Night"), 3);
        assert_eq!(theme_index("latte"), 1);
        assert_eq!(theme_index("nonsense"), 0);
    }

    #[test]
    fn sections_wrap_and_reset_the_choice() {
        let config = Config::default();
        let mut settings = Settings::new(&config);
        settings.move_section(-1, &config);
        assert_eq!(settings.section(), Section::Toast);
        assert_eq!(settings.selected(), 0);
        settings.move_section(1, &config);
        assert_eq!(settings.section(), Section::Theme);
        assert_eq!(
            settings.choose(2),
            Outcome::Keep(Request::Preview(Some("terminal".into())))
        );
        assert_eq!(settings.cancel(), Outcome::Done(Request::Preview(None)));
        assert_eq!(
            settings.apply(),
            Outcome::Done(Request::Settings(Edit::Theme("terminal".into())))
        );
    }
}
