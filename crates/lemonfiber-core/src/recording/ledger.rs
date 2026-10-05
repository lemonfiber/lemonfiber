//! Where the record of what left this machine is written, and how.
//!
//! **Appended, never rewritten per line.** A record is read by an operator now and
//! then and written whenever something leaves, so the cost belongs on the writing
//! side being small: a line is appended to the end of the file, without a flush to
//! the disk, on a thread that may block rather than the runtime's own. Only once
//! the file has grown past [`TRIM_AT`] is it cut back to its newest [`KEPT`] lines.
//!
//! **Lines that arrive together are written together.** A line waits in a queue
//! while another write is in progress, and whoever writes next takes everything
//! queued. A caller's own line is on its way to the file before its request is
//! answered, so a command that ends at once leaves nothing unwritten.
//!
//! **One writer at a time, across processes.** The terminal, the web surface and a
//! command typed beside them each write here. Every append and every trim holds the
//! file's own lock, so a trim can never drop a line another process appended while
//! it was reading, and two appends never interleave.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

/// How many lines are kept when the record is cut back.
///
/// A record that grows without end is a disk problem somebody meets months later,
/// and one that is trimmed is still an answer to "what has this been doing" — which
/// is the question, rather than "what has it ever done". The oldest go first.
pub(super) const KEPT: usize = 500;

/// How large the record may grow before it is cut back to [`KEPT`] lines.
///
/// Well past what [`KEPT`] lines take, so the cut happens once in a long while
/// rather than on every append. Between cuts the record holds at least the newest
/// [`KEPT`] lines and never more than this.
pub(super) const TRIM_AT: u64 = 256 * 1024;

/// The record of what left this machine, kept in one file.
pub struct Ledger {
    /// Where the record is kept.
    at: PathBuf,
    /// Lines noted while another write was in progress, oldest first.
    waiting: std::sync::Mutex<Vec<String>>,
    /// Held by whoever is writing, so writes from this process go one at a time.
    writing: tokio::sync::Mutex<()>,
}

impl Ledger {
    /// A record kept at `at`.
    #[must_use]
    pub fn at(at: PathBuf) -> Self {
        Self {
            at,
            waiting: std::sync::Mutex::new(Vec::new()),
            writing: tokio::sync::Mutex::new(()),
        }
    }

    /// Write one line down, with whatever else is waiting.
    ///
    /// A record that could not be written is not worth failing a request over: the
    /// operator asked for the thing the request does, and telling them it could not
    /// be done because a log was unwritable would be this feature getting in the way
    /// of the product it is meant to make trustworthy.
    pub async fn note(&self, line: String) {
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.push(line);
        }
        // Whoever held this before took every line queued when it began, this one
        // included if it was queued by then, and wrote them before letting go.
        let _writing = self.writing.lock().await;
        let batch = self
            .waiting
            .lock()
            .map(|mut waiting| std::mem::take(&mut *waiting))
            .unwrap_or_default();
        if batch.is_empty() {
            return;
        }
        let at = self.at.clone();
        let _written = tokio::task::spawn_blocking(move || appended(&at, &batch)).await;
    }
}

/// Append these lines to the record, under its lock, and cut it back where it has
/// grown past [`TRIM_AT`].
fn appended(at: &Path, lines: &[String]) -> std::io::Result<()> {
    // A record named with no directory is kept where the process stands, and an
    // empty directory is one there is nothing to make.
    crate::config::store::make_private_dir(at.parent().unwrap_or(Path::new("")))?;
    let mut file = opened(at)?;
    file.lock()?;
    let mut text = lines.join("\n");
    text.push('\n');
    file.write_all(text.as_bytes())?;
    if file.metadata()?.len() > TRIM_AT {
        trimmed(&mut file)?;
    }
    file.unlock()
}

/// Cut the record back to its newest [`KEPT`] lines, in place.
///
/// In place rather than written beside it and renamed over it, because the lock is
/// held on this file: a process waiting on it would be handed the old file once
/// the rename had replaced it, and its line would go into a file nobody can open.
fn trimmed(file: &mut std::fs::File) -> std::io::Result<()> {
    let mut existing = Vec::new();
    std::io::Seek::rewind(file)?;
    file.read_to_end(&mut existing)?;
    let existing = String::from_utf8_lossy(&existing);
    let lines: Vec<&str> = existing.lines().filter(|line| !line.is_empty()).collect();
    let from = lines.len().saturating_sub(KEPT);
    let mut kept = lines.get(from..).unwrap_or_default().join("\n");
    kept.push('\n');
    file.set_len(0)?;
    file.write_all(kept.as_bytes())
}

/// The record opened to be appended to, created owner-only where the platform
/// tracks a file mode.
#[cfg(unix)]
fn opened(at: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .open(at)
}

/// Where the platform has no owner-only mode to set at creation, an ordinary open.
#[cfg(not(unix))]
fn opened(at: &Path) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(at)
}

#[cfg(test)]
mod tests;
