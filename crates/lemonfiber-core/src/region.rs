//! A bounded region of a file somebody else owns, which lemonfiber writes and owns.
//!
//! Some wiring has no file of its own to go in. The stack's proxy reads one
//! configuration file and its dashboard reads another, and a service that should be
//! reachable through one and listed on the other has to be written *into* those files
//! rather than beside them. The file is not lemonfiber's to rewrite, but the region
//! is. It is marked out by a line before and a line after, both naming whose it is.
//! Everything between them is what lemonfiber wrote, and nothing outside them is
//! touched.
//!
//! **The markers are the contract.** Which lines are lemonfiber's is decided by
//! finding both of them exactly as they were written. A region whose markers were
//! edited or removed is not found at all, and that is the safe answer: what to take
//! out can no longer be told apart from what somebody else wrote, so nothing is taken
//! out. The same goes for a region whose content was edited, which is found but is no
//! longer what was written. The rollback layer reads both as drift and refuses.
//!
//! Every marker is a `#` comment, which both files this is used for read as one.
//! Nothing here touches a disk. It is text in and text out, so every arrangement can
//! be put in front of a test without a file under it.

/// What every opening marker starts with, before the owner's name.
const OPENS: &str = "# >>> lemonfiber: ";

/// What every closing marker starts with, before the owner's name.
const CLOSES: &str = "# <<< lemonfiber: ";

/// What follows the owner's name on the opening marker, so a reader of the file who
/// has never heard of this module knows what they are looking at.
const OPENS_AFTER: &str = " — written by lemonfiber; an edit inside this region is kept, and \
                           stops lemonfiber taking it out";

/// The line that opens `owner`'s region.
fn opening(owner: &str) -> String {
    format!("{OPENS}{owner}{OPENS_AFTER}")
}

/// The line that closes `owner`'s region.
fn closing(owner: &str) -> String {
    format!("{CLOSES}{owner}")
}

/// One region found intact in a file: whose it is, where it sits, and what it holds.
struct Found {
    /// Whose region it is, as its markers name it.
    owner: String,
    /// Where the region's lines begin in the text, including the blank line [`put`]
    /// sets it apart with, where it has one.
    start: usize,
    /// Where they end, just past the closing marker's line.
    end: usize,
    /// What sits between the two markers.
    body: String,
}

/// Every region in `text` whose two markers are both intact, in the order they sit.
///
/// A marker with no partner is not a region. An opening marker is paired with the
/// first closing marker after it that names the same owner, and a second opening
/// marker for the same owner before that close means the first one's region was
/// edited. Either way the half-region is left out, which leaves it the operator's.
fn regions(text: &str) -> Vec<Found> {
    let mut found = Vec::new();
    let mut open: Option<(String, usize, usize)> = None;
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if let Some(owner) = bare
            .strip_prefix(OPENS)
            .and_then(|rest| rest.strip_suffix(OPENS_AFTER))
        {
            // Set apart by one blank line when `put` wrote it, and that line is part of
            // what taking it out has to take.
            let start = if text[..at].ends_with("\n\n") {
                at - 1
            } else {
                at
            };
            open = Some((owner.to_owned(), start, at + line.len()));
        } else if let Some(owner) = bare.strip_prefix(CLOSES) {
            if let Some((opened, start, body)) = open.take() {
                if opened == owner {
                    found.push(Found {
                        owner: opened,
                        start,
                        end: at + line.len(),
                        body: text[body..at].to_owned(),
                    });
                }
            }
        }
        at += line.len();
    }
    found
}

/// What `owner`'s region holds, where the file has one intact.
#[must_use]
pub fn within(text: &str, owner: &str) -> Option<String> {
    regions(text)
        .into_iter()
        .find(|found| found.owner == owner)
        .map(|found| found.body)
}

/// The text with `owner`'s region taken out, where it has one intact.
///
/// Exactly the lines [`put`] added: taking out a region that was put into a file
/// ending in a newline gives back that file byte for byte.
#[must_use]
pub fn without(text: &str, owner: &str) -> Option<String> {
    regions(text)
        .into_iter()
        .find(|found| found.owner == owner)
        .map(|found| format!("{}{}", &text[..found.start], &text[found.end..]))
}

/// The text with `owner`'s region holding `body`, set apart at the end of the file.
///
/// A region the file already holds for that owner is taken out first, so there is
/// only ever one. At the end rather than anywhere else, because the end is the one
/// place in a file that nobody else's line depends on being next to.
#[must_use]
pub fn put(text: &str, owner: &str, body: &str) -> String {
    let base = without(text, owner).unwrap_or_else(|| text.to_owned());
    let mut written = base;
    // Set apart from what is above it by one blank line, where anything is above it.
    if !written.is_empty() {
        if !written.ends_with('\n') {
            written.push('\n');
        }
        written.push('\n');
    }
    written.push_str(&opening(owner));
    written.push('\n');
    written.push_str(body);
    if !body.is_empty() && !body.ends_with('\n') {
        written.push('\n');
    }
    written.push_str(&closing(owner));
    written.push('\n');
    written
}

/// `desired` with every region `on_disk` holds intact carried into it, in order.
///
/// For the pass that writes a stack file lemonfiber ships. What it intends for such a
/// file is the shipped content, and a region written into that file since is also
/// lemonfiber's intent. Leaving the regions out would have the next pass read the
/// file as out of date and write the shipped copy over them, which takes a plugin off
/// the proxy while the plugin is still installed.
#[must_use]
pub fn carried(on_disk: &str, desired: &str) -> String {
    regions(on_disk)
        .into_iter()
        .fold(desired.to_owned(), |text, found| {
            put(&text, &found.owner, &found.body)
        })
}

#[cfg(test)]
mod tests;
