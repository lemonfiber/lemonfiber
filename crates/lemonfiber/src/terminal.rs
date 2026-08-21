//! The terminal, the loop and the keyboard — the wire to a screen and a person.
//!
//! A file of its own, and deliberately the only untested one in this feature,
//! because it is the part no test can stand in for: a real terminal in raw mode,
//! a real keyboard, and a loop that runs until somebody stops it. Everything
//! about *what* a screen says lives elsewhere — [`crate::dashboard`] and
//! [`crate::logs`] — where it is drawn into a buffer and read back.
//!
//! Both full-screen views are here rather than one each, because what they share is
//! all of the hard part: taking the terminal, giving it back, and reading a keyboard
//! without letting the reading hold anything else up.
//!
//! Three properties are the whole reason this is shaped the way it is.
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
//!
//! **A flood must never lock the operator out.** A service in a restart loop can
//! write faster than any screen can draw, and a viewer that worked through all of it
//! would stop answering the one key that would narrow the filter. So a pass takes a
//! bounded number of the lines waiting and lets the oldest of the rest go, counted
//! and said on the screen.

use std::io::{stdout, Stdout};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use lemonfiber_core::app::{dashboard::gather, logs, Ctx};
use lemonfiber_core::bundle::Marks;
use lemonfiber_core::dashboard::Snapshot;
use lemonfiber_core::ports::docker::{Lifecycle, LogLine, LogQuery};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::crossterm::ExecutableCommand as _;
use ratatui::prelude::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc::Receiver;

use crate::exit::complain;
use crate::logs::{sampled, Asked, Press, Viewer};
use crate::setup::Bare;

/// How often the screen gathers afresh.
///
/// One second, which is what the operator sees as live. Only after the last one
/// finished, so a stack that takes three seconds to answer refreshes every three
/// rather than queueing gathers it will never catch up on.
const TICK: Duration = Duration::from_secs(1);

/// How often the viewer asks the engine what each service is doing.
///
/// Two seconds rather than the screen's own tick: a restart is not something an
/// operator needs told within the second, and this is the one call the log viewer
/// makes that the stack has to answer.
const LOOKING: Duration = Duration::from_secs(2);

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
    let reader = std::thread::spawn(move || read_keyboard(&keys, meaning));

    let mut snapshot: Option<Snapshot> = None;
    let mut refreshing = tokio::spawn(refresh(Arc::clone(&ctx), None));
    loop {
        if let Some(snapshot) = snapshot.as_ref() {
            if let Err(err) = screen.draw(snapshot) {
                return complain(&drawing("dashboard", &err.to_string()));
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
///
/// What a key means is the caller's, because the two screens want different things
/// from the same keyboard — the dashboard wants two commands, the log viewer wants
/// most characters as text. What is shared is everything else: the poll, the press
/// filter, and knowing to stop when nobody is listening.
fn read_keyboard<T>(keys: &tokio::sync::mpsc::Sender<T>, meaning: fn(KeyEvent) -> Option<T>) {
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

/// Read a live log tail on a screen of its own, until the operator leaves it.
pub(crate) async fn watching(
    ctx: &Ctx,
    forms: &[String],
    services: &[String],
    tail: u32,
) -> ExitCode {
    // Opened before the terminal is taken, so a stack that cannot be read says why
    // on an ordinary terminal rather than flashing an alternate screen and leaving.
    let lines = match logs(ctx, forms, services, LogQuery { tail, follow: true }).await {
        Ok(opened) => opened,
        Err(problem) => return complain(&problem),
    };
    let mut screen = match Screen::open() {
        Ok(screen) => screen,
        Err(err) => {
            eprintln!("error: this terminal cannot show the log viewer: {err}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = following(&mut screen, ctx, lines).await;
    drop(screen);
    outcome
}

/// The loop itself, with the terminal already in hand.
async fn following(screen: &mut Screen, ctx: &Ctx, mut lines: Receiver<LogLine>) -> ExitCode {
    let (presses, mut arriving) = tokio::sync::mpsc::channel(16);
    let reader = std::thread::spawn(move || read_keyboard(&presses, wanted));

    let mut viewer = Viewer::opened();
    let mut looking = tokio::time::interval(LOOKING);
    let mut written = 0_usize;
    // The stream ending is not the screen ending. A stopped service has plenty
    // worth reading in what it said on the way down, and closing the view at that
    // moment would take it away exactly when it is wanted.
    let mut ended = false;
    while viewer.open() {
        if let Err(err) = screen.tail(&viewer) {
            return complain(&drawing("log viewer", &err.to_string()));
        }
        tokio::select! {
            press = arriving.recv() => match press {
                Some(press) => {
                    if viewer.pressed(press) == Asked::Export {
                        written += 1;
                        export(&mut viewer, ctx, written).await;
                    }
                }
                None => break,
            },
            line = lines.recv(), if !ended => match line {
                Some(line) => {
                    viewer.take(line);
                    absorb(&mut viewer, &mut lines);
                }
                None => ended = true,
            },
            // Awaited here rather than on a task of its own, unlike the dashboard's
            // gather: this is one call to the local engine socket, not a dozen to
            // services over the network, and spawning it would buy microseconds at
            // the cost of a second moving part.
            _ = looking.tick() => {
                if let Some(now) = states(ctx).await {
                    viewer.doing(&now);
                }
            },
        }
    }
    drop(arriving);
    let _ = reader.join();
    ExitCode::SUCCESS
}

/// Write the view out and say where it went.
///
/// The redacting is [`Viewer::exported`]'s, where it can be tested; what is here is
/// only the randomness the stand-ins need and the file they go into.
///
/// A stand-in an operator can predict is a way back to the value it stands for, so an
/// export cannot proceed without real randomness. Saying so beats writing a file whose
/// redaction is only as good as a fixed salt.
async fn export(viewer: &mut Viewer, ctx: &Ctx, written: usize) {
    let Some(marks) = Marks::new(ctx.random.as_ref()) else {
        viewer.remarked("this machine would not provide the randomness an export needs");
        return;
    };
    let path = std::path::PathBuf::from(format!("lemonfiber-logs-{}-{written}.txt", ctx.today()));
    ctx.filesystem.write(&path, &viewer.exported(&marks)).await;
    viewer.remarked(&format!("written to {}", path.display()));
}

/// What each service is doing, or nothing where the engine will not say.
///
/// Nothing rather than an empty list where the engine will not say. Silence is not
/// "everything stopped", and handing the viewer an empty list would both invent that
/// and throw away what it knew — so a hiccup leaves its account of the stack intact
/// and the next answer is compared against the last real one.
async fn states(ctx: &Ctx) -> Option<Vec<(String, Lifecycle)>> {
    let containers = ctx.engine.list(&ctx.settings.project).await.ok()?;
    Some(
        containers
            .into_iter()
            .map(|container| (container.service, container.lifecycle))
            .collect(),
    )
}

/// Take what is already waiting, up to what one pass allows.
///
/// The oldest of a backlog go, not the newest. On a live tail what matters is what
/// is happening now, and keeping the front of the queue would show the operator a
/// view that falls further behind the longer the flood lasts.
fn absorb(viewer: &mut Viewer, lines: &mut Receiver<LogLine>) {
    let (taking, letting_go) = sampled(lines.len());
    for _ in 0..letting_go {
        if lines.try_recv().is_err() {
            break;
        }
    }
    if letting_go > 0 {
        viewer.outpaced_by(letting_go);
    }
    for _ in 0..taking {
        let Ok(line) = lines.try_recv() else {
            break;
        };
        viewer.take(line);
    }
}

/// What a keypress asks the viewer for, or nothing for one it has no use for.
///
/// Ctrl-C is read as giving up rather than as the character it is: raw mode no
/// longer turns it into a signal, so an operator who reaches for it is asking to
/// back out — of a filter they were typing, or of the screen where they were not.
const fn wanted(key: KeyEvent) -> Option<Press> {
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') => Some(Press::Abandon),
            _ => None,
        };
    }
    match key.code {
        KeyCode::Char(character) => Some(Press::Typed(character)),
        KeyCode::Backspace => Some(Press::Rubout),
        KeyCode::Enter => Some(Press::Accept),
        KeyCode::Esc => Some(Press::Abandon),
        KeyCode::Up => Some(Press::Back),
        KeyCode::Down => Some(Press::Forward),
        KeyCode::End => Some(Press::Tail),
        _ => None,
    }
}

/// The terminal, in the state a full-screen view needs it, for as long as it is held.
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

    /// Draw one frame of the log viewer.
    fn tail(&mut self, viewer: &Viewer) -> std::io::Result<()> {
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
fn drawing(what: &str, reason: &str) -> lemonfiber_core::error::Problem {
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
