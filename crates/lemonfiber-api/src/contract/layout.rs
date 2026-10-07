//! A generated artefact kept as a directory of files, and how it is written and read.
//!
//! The contract and its surface are each committed as a directory rather than as one
//! document, so no file in them grows past what a reviewer can read. Both are written
//! the same way and compared the same way, which is what this holds.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::Serialize;

/// Every file of a generated directory: its path inside the directory, with `/`
/// between the parts, to what the file holds.
pub type Files = BTreeMap<String, String>;

/// One file's text as it is committed: two-space indent and one trailing newline,
/// with the keys in the order the value serialises them.
///
/// `None` only if it cannot serialise, which a tree of schemas cannot.
#[must_use]
pub fn rendered<T: Serialize + ?Sized>(value: &T) -> Option<String> {
    let mut text = serde_json::to_string_pretty(value).ok()?;
    text.push('\n');
    Some(text)
}

/// Every file under `dir`, read back the way [`replace`] wrote it.
///
/// # Errors
///
/// Where the directory or any file under it cannot be read.
pub fn read(dir: &Path) -> io::Result<Files> {
    let mut files = Files::new();
    gather(dir, "", &mut files)?;
    Ok(files)
}

/// Every file in one directory, and every file in the directories under it.
fn gather(dir: &Path, prefix: &str, files: &mut Files) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let inside = format!("{prefix}{name}");
        if entry.file_type()?.is_dir() {
            gather(&entry.path(), &format!("{inside}/"), files)?;
        } else {
            files.insert(inside, std::fs::read_to_string(entry.path())?);
        }
    }
    Ok(())
}

/// Puts `files` where `dir` is, as the whole of it.
///
/// Written into a sibling first and swapped in, so a run that fails part way leaves
/// the committed directory as it was, and a file the new set no longer holds is gone
/// rather than left behind describing something nothing generates.
///
/// # Errors
///
/// Where the sibling cannot be written or either rename fails.
pub fn replace(dir: &Path, files: &Files) -> io::Result<()> {
    let next = dir.with_extension("next");
    let old = dir.with_extension("old");
    for stale in [&next, &old] {
        if stale.exists() {
            std::fs::remove_dir_all(stale)?;
        }
    }
    for (inside, text) in files {
        let path = next.join(inside);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
    }
    if dir.exists() {
        std::fs::rename(dir, &old)?;
    }
    std::fs::rename(&next, dir)?;
    if old.exists() {
        std::fs::remove_dir_all(&old)?;
    }
    Ok(())
}

/// Every way `stored` is not `fresh`, one line per file: missing, left over, or
/// holding something else.
///
/// Empty where the two agree. A gate saying only that a directory is stale costs
/// whoever reads it a regeneration to find out which file moved.
#[must_use]
pub fn differing(stored: &Files, fresh: &Files) -> Vec<String> {
    let mut found = Vec::new();
    for (inside, made) in fresh {
        match stored.get(inside) {
            None => found.push(format!("{inside} is generated and not committed")),
            Some(held) if held != made => found.push(format!(
                "{inside} is committed and differs from what the types generate{}",
                parting(held, made)
            )),
            Some(_) => {}
        }
    }
    for inside in stored.keys().filter(|inside| !fresh.contains_key(*inside)) {
        found.push(format!("{inside} is committed and nothing generates it"));
    }
    found
}

/// How much of each rendering is shown either side of the first difference.
const AROUND: usize = 140;

/// Where two renderings of one file first part company, in words.
///
/// The answer is already in the two strings being compared, and a reader told only
/// that a file moved has to regenerate it to find out where.
fn parting(stored: &str, fresh: &str) -> String {
    let alike = stored
        .chars()
        .zip(fresh.chars())
        .take_while(|(held, made)| held == made)
        .count();
    let held: String = stored.chars().skip(alike).take(AROUND).collect();
    let made: String = fresh.chars().skip(alike).take(AROUND).collect();
    format!(
        " — they part company {alike} characters in: the file has {held:?} where the \
         types make {made:?}"
    )
}

#[cfg(test)]
mod tests;
