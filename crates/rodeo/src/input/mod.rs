mod copy;
mod keys;
mod mouse;
mod sidebar;

use crossterm::event::Event;
use ratatui::layout::Rect;

use crate::app::App;
use crate::overlay::Outcome;
use crate::view::Mode;

impl App {
    pub fn host_event(&mut self, event: Event) {
        match event {
            Event::Key(key) => {
                self.error = None;
                self.links.hover = None;
                self.key(key);
            }
            Event::Mouse(mouse) => {
                self.error = None;
                self.mouse(mouse);
            }
            Event::Paste(text) => {
                self.error = None;
                self.paste(&text);
            }
            Event::FocusGained => self.focus_changed(true),
            Event::FocusLost => self.focus_changed(false),
            Event::Resize(cols, rows) => {
                self.screen = Rect::new(0, 0, cols, rows);
                self.view.reveal_tab = true;
                self.relayout();
            }
        }
        self.dirty = true;
    }

    fn paste(&mut self, text: &str) {
        if self.popup_paste(text) {
            return;
        }
        if let Some(mut overlay) = self.view.overlay.take() {
            let outcome = overlay.paste(text);
            self.view.overlay = Some(overlay);
            self.overlay_outcome(outcome);
            return;
        }
        if let Some(prompt) = self.view.copy.as_mut().and_then(|c| c.prompt.as_mut()) {
            prompt.field.insert(text);
            return;
        }
        if matches!(self.view.mode, Mode::Copy | Mode::Navigate) {
            return;
        }
        self.view.selection = None;
        if text.is_empty() {
            self.image_paste(None);
            return;
        }
        let Some(address) = self.focused_address() else {
            return;
        };
        if let Some(pane) = self.panes.get_mut(&address) {
            let bytes = pane.term.paste(text);
            pane.term.scroll_to_bottom();
            pane.input(&bytes);
        }
    }

    fn focus_changed(&mut self, focused: bool) {
        self.view.host_focused = focused;
        if focused {
            self.full_redraw = self.config.ui.redraw_on_focus_gained;
        } else if matches!(
            self.view.drag,
            Some(crate::view::Drag::Program { .. } | crate::view::Drag::Select { .. })
        ) {
            self.view.drag = None;
        }
        self.relayout();
        if focused {
            self.write_seen();
        }
    }

    pub fn overlay_outcome(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Stay => {}
            Outcome::Close => self.view.overlay = None,
            Outcome::Done(request) => {
                self.view.overlay = None;
                self.overlay_request(request);
            }
            Outcome::Keep(request) => self.overlay_request(request),
        }
    }

    fn overlay_request(&mut self, request: crate::overlay::Request) {
        use crate::overlay::Request;
        match request {
            Request::Action(action) => self.run_action(action),
            Request::Rename { target, text } => self.finish_rename(target, text),
            Request::Close(target) => self.close(&target),
            Request::Navigate => self.enter_navigate(),
            Request::Focus(crate::overlay::Focus::Space(space)) => self.focus_space(&space),
            Request::Focus(crate::overlay::Focus::Tab(tab)) => self.focus_tab(&tab),
            Request::Focus(crate::overlay::Focus::Pane(pane)) => self.focus_pane(&pane),
            Request::Worktree(op) => self.worktree_op(op),
            Request::Machine(op) => self.machine_op(op),
            Request::Menu { target, pick } => self.menu_pick(target, pick),
            Request::Preview(theme) => self.preview_theme(theme.as_deref()),
            Request::Settings(edit) => match edit.write(&self.config_path) {
                Ok(()) => self.reload_config(),
                Err(error) => {
                    self.preview_theme(None);
                    self.show_error(format!("cannot save the {}: {error}", edit.description()));
                }
            },
        }
    }
}
