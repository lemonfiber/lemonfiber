//! Taking the terminal, giving it back, and reading a keyboard off it.
//!
//! The half both full-screen views share, and the whole of what neither of them can
//! be tested through: a real device in raw mode and a blocking read of it.

use std::io::{stdout, Stdout};
use std::time::Duration;

use lemonfiber_core::dashboard::Snapshot;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand as _;
use ratatui::layout::Rect;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use crate::acting::Acting;
use crate::logs::Viewer;

/// How long the input reader waits before looking again.
///
/// Short enough that a keypress is picked up at once, long enough that an idle
/// screen is not a spin.
pub(super) const KEYS: Duration = Duration::from_millis(100);

/// Read the keyboard until the channel closes, on a thread of its own.
///
/// A thread rather than an async reader: reading a terminal blocks, and blocking
/// the runtime is what would make the refresh stutter.
///
/// What a key means is the caller's, because the two screens want different things
/// from the same keyboard — the dashboard wants two commands, the log viewer wants
/// most characters as text. What is shared is everything else: the poll, the press
/// filter, and knowing to stop when nobody is listening.
pub(super) fn read_keyboard<T>(
    keys: &tokio::sync::mpsc::Sender<T>,
    meaning: fn(KeyEvent) -> Option<T>,
) {
    loop {
        let Ok(true) = event::poll(KEYS) else {
            if keys.is_closed() {
                return;
            }
            continue;
        };
        let Ok(Event::Key(key)) = event::read() else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let Some(asked) = meaning(key) else {
            continue;
        };
        if keys.blocking_send(asked).is_err() {
            return;
        }
    }
}

/// The terminal, in the state a full-screen view needs it, for as long as it is held.
pub(super) struct Screen {
    /// The ratatui terminal drawing into the alternate screen.
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Screen {
    /// Take the terminal: raw mode on, alternate screen entered.
    pub(super) fn open() -> std::io::Result<Self> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        Ok(Self {
            terminal: Terminal::new(CrosstermBackend::new(stdout()))?,
        })
    }

    /// How big the screen is, for deciding what was on it.
    pub(super) fn area(&self) -> Rect {
        self.terminal.size().map_or_else(
            |_| Rect::default(),
            |size| Rect::new(0, 0, size.width, size.height),
        )
    }

    /// Draw one frame.
    pub(super) fn draw(&mut self, snapshot: &Snapshot, acting: &Acting) -> std::io::Result<()> {
        self.terminal
            .draw(|frame| crate::dashboard::draw(frame, snapshot, acting))?;
        Ok(())
    }

    /// Draw one frame of the log viewer.
    pub(super) fn tail(&mut self, viewer: &Viewer) -> std::io::Result<()> {
        self.terminal
            .draw(|frame| crate::logs::draw::draw(frame, viewer))?;
        Ok(())
    }
}

impl Drop for Screen {
    fn drop(&mut self) {
        // Best effort, and deliberately silent: this runs while something else may
        // already be going wrong, and a failure to tidy up must not replace the
        // reason the operator is about to read.
        let _ = disable_raw_mode();
        let _ = stdout().execute(LeaveAlternateScreen);
    }
}

/// A screen that could not be drawn, as a problem rather than a panic.
pub(super) fn drawing(what: &str, reason: &str) -> lemonfiber_core::error::Problem {
    lemonfiber_core::error::Problem::new(
        lemonfiber_core::error::Code::new("TUI-1"),
        lemonfiber_core::error::Severity::Error,
        format!("the {what} could not be drawn"),
        "The terminal stopped accepting output, which usually means it was closed or resized \
         out from under the process.",
        lemonfiber_core::error::Remedy::new("Run it again in a terminal that stays open"),
    )
    .with_detail(reason.to_owned())
}
