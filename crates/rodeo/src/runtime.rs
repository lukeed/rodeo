use std::io::{self, Write};
use std::time::{Duration, Instant};

use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::Rect;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::app::{App, Msg, Options};

const FRAME: Duration = Duration::from_millis(8);
const TICK: Duration = Duration::from_millis(250);
const IDLE: Duration = Duration::from_secs(3600);
const BATCH: usize = 512;

pub struct Runner<B: Backend> {
    pub app: App,
    pub terminal: Terminal<B>,
    out: Box<dyn Write>,
    rx: UnboundedReceiver<Msg>,
    tx: UnboundedSender<Msg>,
    last_draw: Option<Instant>,
}

impl<B: Backend> Runner<B>
where
    B::Error: Send + Sync + 'static,
{
    pub fn new(options: Options, backend: B, out: Box<dyn Write>) -> io::Result<Self> {
        let (tx, rx) = unbounded_channel();
        let terminal = Terminal::new(backend).map_err(io::Error::other)?;
        let size = terminal.size().map_err(io::Error::other)?;
        let mut app = App::new(options, tx.clone());
        app.start(Rect::new(0, 0, size.width, size.height));
        let ticker = tx.clone();
        tokio::task::spawn_local(async move {
            let mut interval = tokio::time::interval(TICK);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                interval.tick().await;
                if ticker.send(Msg::Tick).is_err() {
                    return;
                }
            }
        });
        Ok(Self {
            app,
            terminal,
            out,
            rx,
            tx,
            last_draw: None,
        })
    }

    pub fn sender(&self) -> UnboundedSender<Msg> {
        self.tx.clone()
    }

    pub async fn step(&mut self) -> bool {
        let wait = if self.app.dirty {
            self.last_draw
                .map_or(Duration::ZERO, |at| FRAME.saturating_sub(at.elapsed()))
        } else {
            IDLE
        };
        if self.app.dirty && wait.is_zero() {
            self.draw();
            return !self.app.quit;
        }
        tokio::select! {
            msg = self.rx.recv() => match msg {
                Some(msg) => {
                    self.app.handle(msg);
                    for _ in 0..BATCH {
                        match self.rx.try_recv() {
                            Ok(msg) => self.app.handle(msg),
                            Err(_) => break,
                        }
                    }
                }
                None => self.app.quit = true,
            },
            () = tokio::time::sleep(wait) => {}
        }
        if self.app.dirty && self.last_draw.is_none_or(|at| at.elapsed() >= FRAME) {
            self.draw();
        }
        !self.app.quit
    }

    pub async fn run(&mut self) {
        while self.step().await {}
        self.app.shutdown();
        self.flush();
        self.app.end_popup().await;
    }

    pub fn draw(&mut self) {
        self.app.relayout();
        self.app.update_title();
        if self.app.full_redraw {
            self.app.full_redraw = false;
            let _ = self.terminal.clear();
        }
        let app = &mut self.app;
        if let Err(error) = self.terminal.draw(|frame| crate::render::draw(app, frame)) {
            tracing::error!(%error, "draw failed");
        }
        self.app.dirty = false;
        self.last_draw = Some(Instant::now());
        self.flush();
    }

    fn flush(&mut self) {
        let bytes = self.app.take_output();
        if !bytes.is_empty() {
            let _ = self.out.write_all(&bytes);
            let _ = self.out.flush();
        }
    }
}
