//! The lines a read of the logs says, taken where the web API answers with them.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_core::app::{dispatch, Command, Ctx, ReadingLogs};
use lemonfiber_core::error::Problem;
use lemonfiber_core::logs::Line;
use lemonfiber_core::ports::docker::LogLine;
use lemonfiber_core::ports::{Arrival, Narrator};

use crate::events::live::Live;
use crate::events::wire::{EventKind, Rendered};

/// Where each line goes.
enum Onto {
    /// Kept, to be answered all at once.
    Kept(Arc<Mutex<Vec<Line>>>),
    /// Said on the stream as it arrives.
    Live(Arc<Live>),
}

/// A narrator taking the lines a read of the logs says, and passing everything else on.
struct Tailing {
    beside: Arc<dyn Narrator>,
    onto: Onto,
}

#[async_trait]
impl Narrator for Tailing {
    async fn say(&self, said: &str) {
        self.beside.say(said).await;
    }

    async fn pulled(&self, line: &str) {
        self.beside.pulled(line).await;
    }

    async fn logged(&self, line: &LogLine) {
        taken(&self.onto, Line::from(line.clone())).await;
    }

    async fn arrived(&self, arrival: &Arrival<'_>) {
        self.beside.arrived(arrival).await;
    }

    async fn removed(&self, member: &str) {
        self.beside.removed(member).await;
    }
}

/// One line a read of the logs said, sent where it goes.
async fn taken(onto: &Onto, line: Line) {
    match onto {
        Onto::Kept(held) => {
            if let Ok(mut lines) = held.lock() {
                lines.push(line);
            }
        }
        Onto::Live(stream) => {
            stream
                .say_if_rendered(Rendered::of(EventKind::Log, &line))
                .await;
        }
    }
}

/// Read `asked` through dispatch, with every line it says sent `onto`.
async fn tailed(ctx: &Ctx, asked: ReadingLogs, onto: Onto) -> Result<(), Box<Problem>> {
    let tailing = Tailing {
        beside: Arc::clone(&ctx.narrator),
        onto,
    };
    let reading = ctx.clone().with_narrator(Arc::new(tailing));
    dispatch(Command::Logs(asked), &reading).await.map(drop)
}

/// The lines `asked` reads, in the order they were written.
///
/// # Errors
///
/// The [`Problem`] the read was refused with.
pub(crate) async fn read(ctx: &Ctx, asked: ReadingLogs) -> Result<Vec<Line>, Box<Problem>> {
    let kept = Arc::new(Mutex::new(Vec::new()));
    tailed(ctx, asked, Onto::Kept(Arc::clone(&kept))).await?;
    Ok(kept
        .lock()
        .map(|mut lines| std::mem::take(&mut *lines))
        .unwrap_or_default())
}

/// Say each line `asked` follows onto `live`, until the logs end.
///
/// # Errors
///
/// The [`Problem`] the read was refused with.
pub(crate) async fn followed(
    ctx: &Ctx,
    live: Arc<Live>,
    asked: ReadingLogs,
) -> Result<(), Box<Problem>> {
    tailed(ctx, asked, Onto::Live(live)).await
}

#[cfg(test)]
mod tests;
