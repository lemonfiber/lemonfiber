//! The log viewer's own loop: a live tail, and what a key does to it.
//!
//! What the view *is* and what a key means to it are both [`crate::logs`]'s. What is
//! here is the wire: the stream from the engine, the file an export lands in, and
//! the periodic ask about what each service is doing.

use std::process::ExitCode;
use std::time::Duration;

use lemonfiber_core::app::{logs, Ctx};
use lemonfiber_core::bundle::Marks;
use lemonfiber_core::ports::docker::{Lifecycle, LogLine, LogQuery};
use tokio::sync::mpsc::Receiver;

use super::screen::{drawing, read_keyboard, Screen};
use crate::exit::complain;
use crate::logs::{colours, sampled, wanted, Asked, Press, Viewer};
use crate::say::complain;

/// How often the viewer asks the engine what each service is doing.
///
/// Two seconds rather than the screen's own tick: a restart is not something an
/// operator needs told within the second, and this is the one call the log viewer
/// makes that the stack has to answer.
const LOOKING: Duration = Duration::from_secs(2);

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
            complain!("error: this terminal cannot show the log viewer: {err}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = following(&mut screen, ctx, lines).await;
    drop(screen);
    outcome
}

/// The loop itself, with the terminal already in hand.
async fn following(screen: &mut Screen, ctx: &Ctx, mut lines: Receiver<LogLine>) -> ExitCode {
    let (presses, mut arriving) = tokio::sync::mpsc::channel::<Press>(16);
    let reader = std::thread::spawn(move || read_keyboard(&presses, wanted));

    // Read here rather than deeper in, because this is the edge: everything below
    // decides what to do about the answer, and only this knows where it came from.
    let mut viewer = Viewer::opened();
    if !colours(std::env::var("NO_COLOR").ok().as_deref()) {
        viewer = viewer.without_colour();
    }
    if !crate::render::glossary::wanted() {
        viewer = viewer.without_explanations();
    }
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
                Some(press) => match viewer.pressed(press) {
                    Asked::Export => {
                        written += 1;
                        export(&mut viewer, ctx, written).await;
                    }
                    // Opening them is the asking, so what was opened is recorded.
                    Asked::Learned => {
                        let area = screen.area();
                        // What the view had room for, which is what was behind the
                        // pane — the same arithmetic the drawing does.
                        let rows = usize::from(area.height.saturating_sub(4));
                        super::learned(&viewer.showing_words(rows), area, ctx.dry_run);
                    }
                    Asked::Nothing => {}
                },
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
    // Stamped from the clock port rather than a date, so two runs on the same day
    // cannot write over each other's export.
    let stamp = ctx
        .clock
        .now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    let path = std::path::PathBuf::from(format!("lemonfiber-logs-{stamp}-{written}.txt"));
    ctx.filesystem.write(&path, &viewer.exported(&marks)).await;

    // Writing is best effort and says nothing about whether it worked, so the file is
    // read back before the screen claims it is there. An operator told their export
    // landed, on a directory they cannot write to, would find out when they went
    // looking for it — which is the moment they were relying on it.
    if ctx.filesystem.read(&path).await.is_some() {
        viewer.remarked(&format!("written to {}", path.display()));
    } else {
        viewer.remarked(&format!("could not write {}", path.display()));
    }
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
