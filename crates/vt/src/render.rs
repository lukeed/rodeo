use std::num::NonZeroU16;

use libghostty_vt::RenderState;
use libghostty_vt::render::{CellIterator, CursorVisualStyle, RowIterator};
use libghostty_vt::screen::{CellContentTag, CellWide};
use libghostty_vt::style::{Palette, RgbColor, StyleColor, Underline};
use ratatui_core::buffer::{Buffer, Cell, CellDiffOption};
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier, Style};
use unicode_width::UnicodeWidthStr;

use crate::grid::Point;
use crate::term::{Error, Terminal, Vt};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    #[default]
    Block,
    BlockHollow,
    Underline,
    Bar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub x: u16,
    pub y: u16,
    pub shape: CursorShape,
    pub blinking: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Highlight {
    pub start: Point,
    pub end: Point,
    pub style: Style,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions<'a> {
    pub highlights: &'a [Highlight],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rendered {
    pub cursor: Option<Cursor>,
}

pub(crate) struct Renderer {
    state: RenderState<'static>,
    rows: RowIterator<'static>,
    cells: CellIterator<'static>,
    text: String,
}

struct Colors {
    fg: Color,
    bg: Color,
    palette: Palette,
    changed: [bool; 256],
}

impl Colors {
    fn new(vt: &Vt) -> Self {
        let palette = vt
            .color_palette()
            .unwrap_or(Palette([RgbColor::default(); 256]));
        let base = vt
            .default_color_palette()
            .unwrap_or(Palette([RgbColor::default(); 256]));
        let mut changed = [false; 256];
        for (i, flag) in changed.iter_mut().enumerate() {
            *flag = palette.0[i] != base.0[i];
        }
        let fixed = |current: Option<RgbColor>, default: Option<RgbColor>| match current {
            Some(color) if Some(color) != default => rgb(color),
            _ => Color::Reset,
        };
        Self {
            fg: fixed(
                vt.fg_color().ok().flatten(),
                vt.default_fg_color().ok().flatten(),
            ),
            bg: fixed(
                vt.bg_color().ok().flatten(),
                vt.default_bg_color().ok().flatten(),
            ),
            palette,
            changed,
        }
    }

    fn color(&self, color: StyleColor, default: Color) -> Color {
        match color {
            StyleColor::None => default,
            StyleColor::Rgb(value) => rgb(value),
            StyleColor::Palette(index) => self.indexed(index.0),
        }
    }

    fn indexed(&self, index: u8) -> Color {
        if self.changed[usize::from(index)] {
            rgb(self.palette.0[usize::from(index)])
        } else {
            Color::Indexed(index)
        }
    }
}

impl Renderer {
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self {
            state: RenderState::new()?,
            rows: RowIterator::new()?,
            cells: CellIterator::new()?,
            text: String::with_capacity(16),
        })
    }

    pub(crate) fn cursor_style(&mut self, vt: &Vt) -> (CursorShape, bool) {
        let Ok(snapshot) = self.state.update(vt) else {
            return (CursorShape::Block, false);
        };
        (
            shape(snapshot.cursor_visual_style().ok()),
            snapshot.cursor_blinking().unwrap_or(false),
        )
    }

    fn draw(
        &mut self,
        vt: &Vt,
        area: Rect,
        buf: &mut Buffer,
        scrolled: bool,
    ) -> Result<Option<Cursor>, Error> {
        let colors = Colors::new(vt);
        let Self {
            state,
            rows,
            cells,
            text,
        } = self;
        let snapshot = state.update(vt)?;
        let (cols, height) = (snapshot.cols()?, snapshot.rows()?);
        let width = cols.min(area.width);
        let mut row_iter = rows.update(&snapshot)?;
        let mut y = 0u16;
        while let Some(row) = row_iter.next() {
            if y >= height.min(area.height) {
                break;
            }
            let mut cell_iter = cells.update(row)?;
            let mut x = 0u16;
            while let Some(cell) = cell_iter.next() {
                if x >= width {
                    break;
                }
                let raw = cell.raw_cell()?;
                let vt_style = cell.style()?;
                let mut style = Style::new()
                    .fg(colors.color(vt_style.fg_color, colors.fg))
                    .bg(colors.color(vt_style.bg_color, colors.bg))
                    .underline_color(colors.color(vt_style.underline_color, Color::Reset));
                style = match raw.content_tag()? {
                    CellContentTag::BgColorPalette => {
                        style.bg(colors.indexed(raw.bg_color_palette()?.0))
                    }
                    CellContentTag::BgColorRgb => style.bg(rgb(raw.bg_color_rgb()?)),
                    _ => style,
                };
                let modifier = [
                    (vt_style.bold, Modifier::BOLD),
                    (vt_style.faint, Modifier::DIM),
                    (vt_style.italic, Modifier::ITALIC),
                    (vt_style.underline != Underline::None, Modifier::UNDERLINED),
                    (vt_style.blink, Modifier::SLOW_BLINK),
                    (vt_style.inverse, Modifier::REVERSED),
                    (vt_style.invisible, Modifier::HIDDEN),
                    (vt_style.strikethrough, Modifier::CROSSED_OUT),
                ]
                .into_iter()
                .filter(|(on, _)| *on)
                .fold(Modifier::empty(), |out, (_, flag)| out | flag);
                style = style.add_modifier(modifier);
                text.clear();
                cell.graphemes_utf8(text)?;
                let target = &mut buf[(area.x + x, area.y + y)];
                target.reset();
                target.set_style(style);
                match raw.wide()? {
                    CellWide::Wide if x + 1 < width => {
                        symbol(target, text, 2);
                    }
                    CellWide::Wide | CellWide::SpacerHead => {
                        target.set_symbol(" ");
                    }
                    CellWide::SpacerTail => {}
                    CellWide::Narrow => symbol(target, text, 1),
                }
                x += 1;
            }
            for blank in x..area.width {
                buf[(area.x + blank, area.y + y)].reset();
            }
            y += 1;
        }
        for row in y..area.height {
            for col in 0..area.width {
                buf[(area.x + col, area.y + row)].reset();
            }
        }
        if scrolled || !snapshot.cursor_visible()? {
            return Ok(None);
        }
        let shape = shape(snapshot.cursor_visual_style().ok());
        let blinking = snapshot.cursor_blinking().unwrap_or(false);
        Ok(snapshot
            .cursor_viewport()?
            .filter(|at| at.x < width && at.y < area.height.min(height))
            .map(|at| Cursor {
                x: area.x + at.x,
                y: area.y + at.y,
                shape,
                blinking,
            }))
    }
}

fn symbol(target: &mut Cell, text: &str, width: u16) {
    if text.is_empty() || text.starts_with('\u{10EEEE}') {
        target.set_symbol(" ");
        return;
    }
    target.set_symbol(text);
    if text.width() != usize::from(width) {
        target.set_diff_option(CellDiffOption::ForcedWidth(
            NonZeroU16::new(width).unwrap_or(NonZeroU16::MIN),
        ));
    }
}

impl Terminal {
    pub fn render(&self, area: Rect, buf: &mut Buffer, options: &RenderOptions<'_>) -> Rendered {
        let area = area.intersection(buf.area);
        let scrolled = self.is_scrolled_back();
        let cursor = self
            .renderer()
            .borrow_mut()
            .draw(self.vt(), area, buf, scrolled)
            .ok()
            .flatten();
        let (cols, rows) = self.size();
        let (width, height) = (cols.min(area.width), rows.min(area.height));
        if width == 0 || height == 0 {
            return Rendered { cursor };
        }
        let top = self.viewport_top();
        let bottom = top + u64::from(height) - 1;
        for highlight in options.highlights {
            let (start, end) = if highlight.start <= highlight.end {
                (highlight.start, highlight.end)
            } else {
                (highlight.end, highlight.start)
            };
            for row in start.row.max(top)..=end.row.min(bottom) {
                let y = area.y + u16::try_from(row - top).unwrap_or(0);
                let from = if row == start.row { start.col } else { 0 };
                let to = if row == end.row { end.col } else { cols - 1 };
                for col in from..=to.min(width - 1) {
                    buf[(area.x + col, y)].set_style(highlight.style);
                }
            }
        }
        Rendered { cursor }
    }
}

fn rgb(color: RgbColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

fn shape(style: Option<CursorVisualStyle>) -> CursorShape {
    match style {
        Some(CursorVisualStyle::Bar) => CursorShape::Bar,
        Some(CursorVisualStyle::Underline) => CursorShape::Underline,
        Some(CursorVisualStyle::BlockHollow) => CursorShape::BlockHollow,
        _ => CursorShape::Block,
    }
}
