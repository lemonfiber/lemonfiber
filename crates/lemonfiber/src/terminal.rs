//! The terminal, the loop and the keyboard — the wire to a screen and a person.
//!
//! A file of its own, and deliberately the only untested one in this feature,
//! because it is the part no test can stand in for: a real terminal in raw mode,
//! a real keyboard, and a loop that runs until somebody stops it. Everything
//! about *what* the screen says is in [`crate::dashboard`], where it is drawn into
//! a buffer and read back.
//!
//! Two properties are the whole reason this is shaped the way it is.
//!
//! **A refresh must never hold up a keypress.** Gathering talks to Docker and to
//! half a dozen services, and any of them may take seconds. So the gather runs as
//! a task and the loop waits on whichever arrives first — the next frame's facts
//! or the operator's key — rather than doing one and then the other. A quit
//! typed during a slow refresh is acted on immediately.
//!
//! **The terminal is put back whatever happens.** Raw mode and the alternate
//! screen are global state on a device this process does not own; leaving them on
//! hands the operator a shell that no longer echoes. The restore is a `Drop`, so
//! it runs on the ordinary way out, on an error, and on a panic alike.

use std::io::{stdout, Stdout};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::app::{dashboard::gather, Ctx};
use lemonfiber_core::dashboard::Snapshot;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand as _;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;

use crate::exit::complain;
use crate::setup::Bare;

/// How often the screen gathers afresh.
///
/// One second, which is what the operator sees as live. Only after the last one
/// finished, so a stack that takes three seconds to answer refreshes every three
/// rather than queueing gathers it will never catch up on.
const TICK: Duration = Duration::from_secs(1);

/// How long the input reader waits before looking again.
///
/// Short enough that a keypress is picked up at once, long enough that an idle
/// screen is not a spin.
const KEYS: Duration = Duration::from_millis(100);

/// What a bare run on a machine that is already set up does about it.
///
/// The decision and the words are both [`crate::setup`]'s, and tested there. What
/// is here is only the doing of it: taking a terminal, or printing to one.
pub(crate) async fn configured(ctx: Ctx, bare: Bare) -> ExitCode {
    match bare {
        Bare::Dashboard => dashboard(ctx).await,
        Bare::Guidance => {
            for line in crate::setup::already_set_up() {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
    }
}

/// Run the dashboard until the operator leaves it.
async fn dashboard(ctx: Ctx) -> ExitCode {
    let mut screen = match Screen::open() {
        Ok(screen) => screen,
        // A terminal that will not go into raw mode is not a fault in the stack,
        // and saying so beats a blank screen.
        Err(err) => {
            eprintln!("error: this terminal cannot show the dashboard: {err}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = run(&mut screen, ctx).await;
    drop(screen);
    outcome
}

/// The loop itself, with the terminal already in hand.
async fn run(screen: &mut Screen, ctx: Ctx) -> ExitCode {
    let ctx = Arc::new(ctx);
    let (keys, mut typed) = tokio::sync::mpsc::channel(16);
    let reader = std::thread::spawn(move || read_keys(&keys));

    let mut snapshot: Option<Snapshot> = None;
    let mut refreshing = tokio::spawn(refresh(Arc::clone(&ctx), None));
    loop {
        if let Some(snapshot) = snapshot.as_ref() {
            if let Err(err) = screen.draw(snapshot) {
                return complain(&drawing(&err.to_string()));
            }
        }
        tokio::select! {
            // Whichever arrives first. A refresh in flight never holds up a key,
            // which is the difference between a screen that feels alive and one
            // that ignores you for three seconds at a time.
            key = typed.recv() => match key {
                Some(Key::Quit) | None => break,
                Some(Key::Refresh) => {
                    refreshing.abort();
                    refreshing = tokio::spawn(refresh(Arc::clone(&ctx), None));
                }
            },
            gathered = &mut refreshing => {
                if let Ok(fresh) = gathered {
                    snapshot = Some(fresh);
                }
                let previous = snapshot.clone();
                let next = Arc::clone(&ctx);
                refreshing = tokio::spawn(async move {
                    tokio::time::sleep(TICK).await;
                    refresh(next, previous).await
                });
            }
        }
    }
    refreshing.abort();
    drop(typed);
    let _ = reader.join();
    ExitCode::SUCCESS
}

/// One gather, with the last one to carry a stale reading forward from.
async fn refresh(ctx: Arc<Ctx>, previous: Option<Snapshot>) -> Snapshot {
    gather(&ctx, previous.as_ref()).await
}

/// What the operator asked for.
enum Key {
    /// Leave.
    Quit,
    /// Gather again now rather than at the next tick.
    Refresh,
}

/// Read the keyboard until the channel closes, on a thread of its own.
///
/// A thread rather than an async reader: reading a terminal blocks, and blocking
/// the runtime is what would make the refresh stutter.
fn read_keys(keys: &tokio::sync::mpsc::Sender<Key>) {
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

/// What a keypress means, or nothing for one this screen has no use for.
const fn meaning(key: KeyEvent) -> Option<Key> {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => Some(Key::Quit),
        // Ctrl-C, because a terminal in raw mode no longer turns it into a signal
        // and an operator who cannot leave with it is trapped.
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Key::Quit),
        KeyCode::Char('r') => Some(Key::Refresh),
        _ => None,
    }
}

/// The terminal, in the state the dashboard needs it, for as long as it is held.
struct Screen {
    /// The ratatui terminal drawing into the alternate screen.
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Screen {
    /// Take the terminal: raw mode on, alternate screen entered.
    fn open() -> std::io::Result<Self> {
        enable_raw_mode()?;
        stdout().execute(EnterAlternateScreen)?;
        Ok(Self {
            terminal: Terminal::new(CrosstermBackend::new(stdout()))?,
        })
    }

    /// Draw one frame.
    fn draw(&mut self, snapshot: &Snapshot) -> std::io::Result<()> {
        self.terminal
            .draw(|frame| crate::dashboard::draw(frame, snapshot))?;
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
fn drawing(reason: &str) -> lemonfiber_core::error::Problem {
    lemonfiber_core::error::Problem::new(
        lemonfiber_core::error::Code::new("TUI-1"),
        lemonfiber_core::error::Severity::Error,
        "the dashboard could not be drawn",
        "The terminal stopped accepting output, which usually means it was closed or resized \
         out from under the process.",
        lemonfiber_core::error::Remedy::new("Run it again in a terminal that stays open"),
    )
    .with_detail(reason.to_owned())
}
