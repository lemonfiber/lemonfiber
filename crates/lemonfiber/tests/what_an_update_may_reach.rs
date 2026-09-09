//! What the self-update family is allowed to reach.
//!
//! Three rules about one family, apart from the wider architecture guards for the
//! reason the migration ones are: they are about one seam, and that file had grown
//! past what one test file may cover.
//!
//! All three are prohibitions, which is why they are guards rather than lines of
//! code. What must not happen here is invisible in review — a check that starts a
//! container, a remedy that reaches for `sudo`, an address written down beside the
//! request that uses it — and each of them looks like a helpful line at the moment it
//! is added.

mod source_tree;

use std::path::Path;

use source_tree::{production, sources};

/// The seams the update family may reach, by the name they carry on a context.
///
/// Four, and each is there because the check cannot be made without it: the
/// filesystem says where this binary is and keeps what the last check read, the
/// transport asks the release list, the settings say whether it may, and the clock
/// says whether it is due. Nothing else is any of this command's business.
const ALLOWED: [&str; 4] = ["filesystem", "http", "settings", "seconds"];

/// Updating lemonfiber leaves the stack alone, and cannot do otherwise.
///
/// The binary is a control surface; containers run independently of it. An operator
/// has to be able to bring the tool up to date without touching a working system,
/// and the report says as much in words — but a sentence is a claim and this is the
/// property. Written as what the family *may* reach rather than what it may not,
/// because a list of forbidden names is a list of the ways somebody has already
/// thought of, and the day this matters is the day a seam nobody listed is wired in.
#[test]
fn updating_reaches_nothing_that_could_touch_the_stack() {
    let reaching: Vec<String> = family(updates, 8)
        .iter()
        .flat_map(|(path, shipped)| {
            shipped
                .lines()
                .flat_map(seams)
                .filter(|rest| !permitted(rest))
                .map(move |rest| format!("{} reaches ctx.{rest}", path.display()))
        })
        .collect();

    assert!(
        reaching.is_empty(),
        "updating lemonfiber stops, restarts and changes nothing in the stack, so it \
         may reach only {ALLOWED:?}: {reaching:?}"
    );
}

/// The words a program reaches for when it wants to be somebody else.
///
/// Named rather than derived, and a list of names is the right shape here for once:
/// the requirement is that no attempt is made, and an attempt is made by naming one
/// of these. A machine where the binary cannot be replaced is a fact to report with
/// the path, never a problem to solve on the operator's behalf.
const ESCALATION: [&str; 5] = ["sudo", "doas", "pkexec", "runas", "setuid"];

/// A directory that will not take a new file is reported, never worked around.
///
/// The failure this prevents is a familiar one: an updater that meets a permission
/// error and helpfully re-runs itself under something that will not meet one. What an
/// operator gets instead is the path and what the machine said about it, which is the
/// thing they can act on — and which is all this family knows how to do.
#[test]
fn nothing_in_the_update_family_reaches_for_a_way_to_become_somebody_else() {
    let named: Vec<String> = family(updates, 8)
        .iter()
        .flat_map(|(path, shipped)| {
            let lowered = shipped.to_lowercase();
            ESCALATION
                .iter()
                .filter(move |word| lowered.contains(*word))
                .map(move |word| format!("{} names {word}", path.display()))
        })
        .collect();

    assert!(
        named.is_empty(),
        "a binary this operator cannot replace is reported with its path and never \
         escalated around, so nothing here may name {ESCALATION:?}: {named:?}"
    );
}

/// The check asks where the enumeration says, and holds no address of its own.
///
/// The list an operator can read owns every address lemonfiber asks anything of, and
/// the check is handed one rather than carrying one. That is what makes the
/// enumeration worth reading: an address written down beside the request that uses it
/// is an address the operator was never told about, and it would pass every other
/// guard here.
#[test]
fn the_check_holds_no_address_of_its_own() {
    let carried: Vec<String> = family(asks, 3)
        .iter()
        .flat_map(|(path, shipped)| {
            shipped
                .lines()
                .filter(|line| line.contains("https://") || line.contains("http://"))
                .map(move |line| format!("{} writes {}", path.display(), line.trim()))
        })
        .collect();

    assert!(
        carried.is_empty(),
        "every address lemonfiber asks anything of is declared where the operator can \
         read it, so the check may hold none: {carried:?}"
    );
}

/// The shipped half of every file in the update family, asserted to be a family.
///
/// A guard whose subject can vanish silently is the defect this repository keeps
/// finding: a filter that matches nothing passes every rule beneath it while reading
/// as though it had looked. So what was found is asserted before what it holds.
fn family(matching: fn(&Path) -> bool, fewest: usize) -> Vec<(std::path::PathBuf, String)> {
    let found: Vec<(std::path::PathBuf, String)> = sources()
        .into_iter()
        .filter(|(path, _)| matching(path))
        .map(|(path, text)| {
            let shipped = production(&text).to_owned();
            (path, shipped)
        })
        .collect();
    assert!(
        found.len() >= fewest,
        "read {} files of the update family, fewer than the {fewest} it has never gone \
         below — the filter has stopped matching it",
        found.len()
    );
    found
}

/// Whether this file is part of the update family rather than a test about it.
fn updates(path: &Path) -> bool {
    let named = path.to_string_lossy().replace('\\', "/");
    named.contains("update") && !named.contains("/tests/")
}

/// Whether this file is the half of the family that reaches the network.
fn asks(path: &Path) -> bool {
    path.to_string_lossy()
        .replace('\\', "/")
        .contains("/app/update")
}

/// Every `ctx.` seam one line names, each with whatever follows it.
fn seams(line: &str) -> Vec<String> {
    line.match_indices("ctx.")
        .filter_map(|(at, _)| line.get(at + "ctx.".len()..))
        .map(std::borrow::ToOwned::to_owned)
        .collect()
}

/// The seam a `ctx.` reach names, without whatever follows it.
fn named(rest: &str) -> String {
    rest.chars()
        .take_while(|letter| letter.is_alphanumeric() || *letter == '_')
        .collect()
}

/// Whether a seam, with what follows it, is one the update family may reach.
fn permitted(rest: &str) -> bool {
    let seam = named(rest);
    seam.is_empty() || ALLOWED.contains(&seam.as_str())
}
