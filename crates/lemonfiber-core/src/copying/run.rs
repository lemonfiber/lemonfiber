//! Keeping the reading while a copy runs, and reading it back.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use lemonfiber_ports::time::Clock;

use super::kept::{self, Kept};
use super::pacing::{Monotonic, Stopwatch, Throttle};
use super::{Copying, Meter, What};
use crate::app::Ctx;
use crate::backup::Scope;
use crate::config::paths::Paths;
use crate::instant::Instant;

/// The longest a running copy's reading goes unwritten while bytes move.
const KEEP_EVERY: Duration = Duration::from_secs(1);

/// What a copy is started with: the clocks it reads, the job it runs as, and its limit.
#[derive(Clone)]
pub struct Starting {
    dir: Option<PathBuf>,
    clock: Arc<dyn Clock>,
    stopwatch: Arc<dyn Stopwatch>,
    job: Option<String>,
    limit: Option<u64>,
}

impl Starting {
    /// A copy started as `ctx` runs: its clock, its job, and the operator's limit.
    #[must_use]
    pub fn of(ctx: &Ctx) -> Self {
        Self {
            dir: ctx
                .archives
                .as_ref()
                .map(|archives| archives.paths.copying()),
            clock: Arc::clone(&ctx.seams.clock),
            stopwatch: Arc::new(Monotonic::started()),
            job: ctx.job.clone(),
            limit: crate::settings::backup_limit(ctx),
        }
    }

    /// A copy started on the clocks given, as `job`, held to `limit` bytes a second, keeping
    /// no reading.
    #[must_use]
    pub fn on(
        clock: Arc<dyn Clock>,
        stopwatch: Arc<dyn Stopwatch>,
        job: Option<String>,
        limit: Option<u64>,
    ) -> Self {
        Self {
            dir: None,
            clock,
            stopwatch,
            job,
            limit,
        }
    }

    /// The same start, keeping its reading under `paths`.
    #[must_use]
    pub fn kept_under(self, paths: &Paths) -> Self {
        Self {
            dir: Some(paths.copying()),
            ..self
        }
    }

    /// Begin a copy's reading, named from the `parts` it writes.
    ///
    /// Held to the limit where it is a backup. Kept unread where another copy keeps the
    /// reading already.
    #[must_use]
    pub fn begin(
        &self,
        what: What,
        scope: Scope,
        whole: u64,
        parts: Vec<(String, String)>,
    ) -> Arc<Recorder> {
        let lock = self.dir.as_deref().and_then(kept::held);
        let now = Instant::of(self.clock.now());
        let throttle = self
            .limit
            .filter(|_| what == What::Backup)
            .map(Throttle::at);
        let recorder = Recorder {
            at: self.dir.as_deref().map(kept::reading),
            lock,
            clock: Arc::clone(&self.clock),
            stopwatch: Arc::clone(&self.stopwatch),
            began: self.stopwatch.elapsed(),
            parts,
            moving: Mutex::new(Moving {
                kept: Kept {
                    what,
                    scope,
                    job: self.job.clone(),
                    started: now,
                    written: 0,
                    whole,
                    part: String::new(),
                    wrote: now,
                    limit: throttle.map(Throttle::limit),
                },
                kept_at: None,
                throttle,
            }),
        };
        recorder.keep_now();
        Arc::new(recorder)
    }
}

/// The reading a running copy keeps, and the lock it holds while it runs.
pub struct Recorder {
    at: Option<PathBuf>,
    lock: Option<File>,
    clock: Arc<dyn Clock>,
    stopwatch: Arc<dyn Stopwatch>,
    began: Duration,
    parts: Vec<(String, String)>,
    moving: Mutex<Moving>,
}

/// What a running copy has come to, and when its reading was last written.
struct Moving {
    kept: Kept,
    kept_at: Option<Duration>,
    throttle: Option<Throttle>,
}

impl Recorder {
    fn since(&self) -> Duration {
        self.stopwatch.elapsed().saturating_sub(self.began)
    }

    fn keep_now(&self) {
        let mut moving = self.moving.lock().unwrap_or_else(PoisonError::into_inner);
        self.keep(&mut moving);
    }

    fn keep(&self, moving: &mut Moving) {
        moving.kept_at = Some(self.since());
        if let (Some(at), Some(_)) = (&self.at, &self.lock) {
            let _ = crate::config::store::write(
                at,
                &serde_json::to_string(&moving.kept).unwrap_or_default(),
            );
        }
    }
}

impl Meter for Recorder {
    fn whole(&self, bytes: u64) {
        let mut moving = self.moving.lock().unwrap_or_else(PoisonError::into_inner);
        moving.kept.whole = bytes;
        self.keep(&mut moving);
    }

    fn writing(&self, entry: &str) {
        let Some(part) = crate::backup::part(entry, &self.parts) else {
            return;
        };
        let mut moving = self.moving.lock().unwrap_or_else(PoisonError::into_inner);
        if moving.kept.part != part {
            moving.kept.part = part;
            self.keep(&mut moving);
        }
    }

    fn wrote(&self, bytes: u64) {
        let since = self.since();
        let wait = {
            let mut moving = self.moving.lock().unwrap_or_else(PoisonError::into_inner);
            moving.kept.written = moving.kept.written.saturating_add(bytes);
            moving.kept.wrote = Instant::of(self.clock.now());
            if moving
                .kept_at
                .is_none_or(|kept| since.saturating_sub(kept) >= KEEP_EVERY)
            {
                self.keep(&mut moving);
            }
            moving
                .throttle
                .as_mut()
                .map_or(Duration::ZERO, |throttle| throttle.after(bytes, since))
        };
        if !wait.is_zero() {
            self.stopwatch.wait(wait);
        }
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        if let (Some(at), Some(_)) = (&self.at, &self.lock) {
            let _ = std::fs::remove_file(at);
        }
    }
}

/// The copy running on this machine now, or none.
#[must_use]
pub(crate) fn now(ctx: &Ctx) -> Copying {
    let Some(archives) = ctx.archives.as_ref() else {
        return Copying::default();
    };
    read(
        &archives.paths.copying(),
        ctx.stamp(),
        crate::settings::copy_stalled(ctx),
    )
}

/// The copy whose reading is kept under `dir`, read at `now`, stalled after `stalled` seconds.
pub(crate) fn read(dir: &Path, now: Instant, stalled: u64) -> Copying {
    Copying {
        running: kept::read(dir).map(|kept| kept.running(now, stalled)),
    }
}
