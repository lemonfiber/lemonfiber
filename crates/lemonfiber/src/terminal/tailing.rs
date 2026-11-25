//! The log viewer's own loop: a live tail, and what a key does to it.
//!
//! What the view *is* and what a key means to it are both [`crate::logs`]'s. What is
//! here is the wire: the stream from the engine, the file an export lands in, and
//! the periodic ask about what each service is doing.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use lemonfiber_core::app::surfacing::Answer;
use lemonfiber_core::app::{
    dispatch, Command, Ctx, Exporting, Outcome, Polled, ReadingLogs, Surfacing,
};
use lemonfiber_core::docker::State;
use lemonfiber_core::ports::docker::LogLine;
use lemonfiber_core::ports::{Arrival, Narrator};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{oneshot, Mutex};
use tokio::task::{JoinHandle, JoinSet};

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
    let (sender, lines) = tokio::sync::mpsc::channel(LINES);
    let (opening, opened) = oneshot::channel();
    let sending = Sending {
        beside: Arc::clone(&ctx.narrator),
        opening: Mutex::new(Some(opening)),
        lines: sender,
    };
    let reading = ctx.clone().with_narrator(Arc::new(sending));
    let asked = Command::Logs(ReadingLogs {
        forms: forms.to_vec(),
        services: services.to_vec(),
        tail,
        follow: true,
    });
    let read = tokio::spawn(async move { dispatch(asked, &reading).await });
    if opened.await.is_err() {
        return match read.await {
            Ok(Err(problem)) => complain(&problem),
            _ => ExitCode::FAILURE,
        };
    }
    let mut screen = match Screen::open() {
        Ok(screen) => screen,
        Err(err) => {
            complain!("error: this terminal cannot show the log viewer: {err}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = following(&mut screen, ctx, forms, lines).await;
    read.abort();
    drop(screen);
    outcome
}

/// Say once that the read opened, to whoever is waiting to hear it.
async fn opened(opening: &Mutex<Option<oneshot::Sender<()>>>) {
    if let Some(opening) = opening.lock().await.take() {
        let _ = opening.send(());
    }
}

/// How many lines wait for the viewer before the read waits for it.
const LINES: usize = 1024;

/// A narrator handing the viewer each line a read of the logs says, and saying when the
/// read opened, passing everything else on.
struct Sending {
    beside: Arc<dyn Narrator>,
    opening: Mutex<Option<oneshot::Sender<()>>>,
    lines: Sender<LogLine>,
}

#[async_trait]
impl Narrator for Sending {
    async fn say(&self, said: &str) {
        self.beside.say(said).await;
    }

    async fn pulled(&self, line: &str) {
        self.beside.pulled(line).await;
    }

    async fn logged(&self, line: &LogLine) {
        drop(self.lines.send(line.clone()).await);
    }

    async fn logs_opened(&self) {
        opened(&self.opening).await;
    }

    async fn arrived(&self, arrival: &Arrival<'_>) {
        self.beside.arrived(arrival).await;
    }

    async fn removed(&self, member: &str) {
        self.beside.removed(member).await;
    }
}

/// The loop itself, with the terminal already in hand.
async fn following(
    screen: &mut Screen,
    ctx: &Ctx,
    forms: &[String],
    mut lines: Receiver<LogLine>,
) -> ExitCode {
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
    let shared = Arc::new(ctx.clone());
    let mut asking: Option<JoinHandle<Doing>> = None;
    let mut exports: JoinSet<String> = JoinSet::new();
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
                        let asked = Exporting { text: viewer.exported(), written };
                        written_apart(&mut exports, Arc::clone(&shared), asked);
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
            _ = looking.tick() => {
                if asking.is_none() {
                    asking = Some(asked(Arc::clone(&shared), forms.to_vec()));
                }
            },
            now = super::finished(&mut asking) => {
                asking = None;
                if let Some(Some(now)) = now {
                    viewer.doing(&now);
                }
            },
            Some(said) = exports.join_next(), if !exports.is_empty() => {
                if let Ok(said) = said {
                    viewer.remarked(&said);
                }
            },
        }
    }
    if let Some(asked) = asking {
        asked.abort();
    }
    while exports.join_next().await.is_some() {}
    drop(arriving);
    let _ = reader.join();
    ExitCode::SUCCESS
}

/// What each service is doing, where the engine says.
type Doing = Option<Vec<(String, State)>>;

async fn kept(ctx: &Ctx, asked: Exporting) -> String {
    let answered = dispatch(Command::Surface(Surfacing::Export(asked)), ctx).await;
    match answered {
        Ok(Outcome::Surfaced(Answer::Exported(exported))) => {
            crate::render::logs::exported(&exported).text()
        }
        _ => answered
            .err()
            .map(|problem| problem.summary)
            .unwrap_or_default(),
    }
}

fn written_apart(exports: &mut JoinSet<String>, ctx: Arc<Ctx>, asked: Exporting) {
    exports.spawn(async move { kept(&ctx, asked).await });
}

fn asked(ctx: Arc<Ctx>, forms: Vec<String>) -> JoinHandle<Doing> {
    tokio::spawn(async move { states(&ctx, forms).await })
}

async fn states(ctx: &Ctx, forms: Vec<String>) -> Doing {
    let polled = Command::Surface(Surfacing::Polled(Polled::Status { forms }));
    let Ok(Outcome::Status(report)) = dispatch(polled, ctx).await else {
        return None;
    };
    Some(
        report
            .services
            .into_iter()
            .map(|service| (service.id, service.state))
            .collect(),
    )
}

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
