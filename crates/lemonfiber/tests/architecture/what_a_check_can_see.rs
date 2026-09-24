//! What a diagnostic check is built able to observe, and what it must say first.
//!
//! Two rules about the same seam, which is the difference between a check that
//! looks and one that infers. A check reading a port number out of the settings and
//! calling it bound passes on the machine where nothing is listening, and passes
//! loudest on the broken one; a check that goes and looks needs something to look
//! through. And a check that disturbs the stack to answer has to say how long it
//! disturbs it for, before it does.
//!
//! Both are structural, because both are structural properties. Neither proves a
//! particular finding was observed rather than assumed — what they prove is that
//! the check was built able to observe, and they fail the moment one is added that
//! was not.

use crate::source_tree::{production, sources};

/// Every diagnostic check is given something to ask.
///
/// A check must establish what it reports rather than infer it from the operator's
/// configuration. The difference matters most exactly where an operator is least able
/// to tell: a check that reads a port number out of the settings and calls it "bound"
/// will pass on a machine where nothing is listening, and it will pass loudest on the
/// machine that is broken.
///
/// Asserted structurally, because it is a structural property. A check that only reads
/// configuration needs nothing but data; one that goes and looks needs a seam to look
/// through, and in this crate that seam is always a trait object — the engine, a
/// runner, an HTTP client, a filesystem, or a narrower port like `Validator` or
/// `UsenetAccounts`.
///
/// Read from what the check is **handed**, not only from what it stores: several keep
/// their ports inside a helper of their own rather than as a field, which is a detail
/// of how they are built rather than of whether they can see.
///
/// This does not prove any particular finding was observed rather than assumed. It
/// proves the check was built able to observe, and it fails the moment somebody adds
/// one that was not — which is the change worth catching, since the finding itself
/// reads the same either way.
#[test]
fn every_check_is_given_something_to_ask() {
    let mut assuming: Vec<String> = Vec::new();
    let mut seen = 0_usize;

    for (path, text) in sources() {
        if !path.to_string_lossy().contains("doctor") {
            continue;
        }
        for name in text.lines().filter_map(|line| {
            line.strip_prefix("pub struct ")
                .and_then(|rest| rest.strip_suffix(" {"))
                .filter(|name| name.ends_with("Check"))
        }) {
            seen += 1;
            // What it holds, and what it is handed. Either is a way to go and look;
            // a check with neither can only be repeating the configuration back.
            let fields = text
                .split(&format!("pub struct {name} {{"))
                .nth(1)
                .and_then(|rest| rest.split("\n}").next())
                .unwrap_or_default()
                .to_owned();
            let built = text
                .split(&format!("impl {name} {{"))
                .nth(1)
                .and_then(|rest| rest.split(") -> Self").next())
                .unwrap_or_default()
                .to_owned();

            if !fields.contains("Arc<dyn ") && !built.contains("Arc<dyn ") {
                assuming.push(format!("{} ({name})", path.display()));
            }
        }
    }

    assert!(
        seen > 5,
        "the scan found {seen} checks, which means it is looking in the wrong place"
    );
    assert!(
        assuming.is_empty(),
        "a check with nothing to ask can only be repeating the configuration back: {}",
        assuming.join(", ")
    );
}
/// The gate a disturbing check puts in front of itself, whatever its receiver is called.
///
/// `!self.disruptive` was the whole pattern, and it stopped matching the day the bodies
/// moved out of their `#[async_trait]` methods so the coverage gate could see them: the
/// receiver is a parameter there and is not called `self`. The rule did not change and
/// neither did the code it is about, so the reading is by shape — a negation in front of
/// whatever holds the field — rather than by one spelling of it.
fn refuses_unless_asked(shipped: &str) -> bool {
    shipped.match_indices(".disruptive").any(|(at, _)| {
        let before = &shipped[..at];
        let holder = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
        holder.ends_with('!')
    })
}
/// A check that disturbs the running system says how long it disturbs it for.
///
/// Opting in is a decision an operator makes about a cost, and half the cost is the
/// length of it: a tunnel down for one probe and a tunnel down for ten minutes are
/// different answers to "shall I run this now". The killswitch stated what it disturbed
/// for four releases and never how long, and nothing in the build noticed.
///
/// Read structurally, from the gate rather than from the words: a check that refuses to
/// act unless it was asked for is a check that disturbs something, and it must reach for
/// the one place a length is put into words. A sentence with the number typed into it
/// would pass a reading of the prose and still go stale the day the budget changed.
#[test]
fn every_disturbing_check_says_how_long_it_disturbs_for() {
    let mut silent: Vec<String> = Vec::new();
    let mut seen = 0_usize;

    for (path, text) in sources() {
        if !path.to_string_lossy().contains("doctor") {
            continue;
        }
        let shipped = production(&text);
        if !refuses_unless_asked(shipped) {
            continue;
        }
        seen += 1;
        if !shipped.contains("disturbing_for") {
            silent.push(path.display().to_string());
        }
    }

    assert!(
        seen > 1,
        "the scan found {seen} checks gated on being asked for, which means it is looking \
         in the wrong place"
    );
    assert!(
        silent.is_empty(),
        "a check that disturbs the stack must say how long for: {}",
        silent.join(", ")
    );
}
