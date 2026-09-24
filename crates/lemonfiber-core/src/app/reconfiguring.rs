//! What one proposed change comes to on this machine, worked out before it is written.
//!
//! The gathering half of reconfiguration: this reaches the configuration file, the
//! services and the download clients, and hands what it read to
//! [`crate::reconfigure`], which decides what it amounts to. Nothing here writes.
//!
//! It runs *before* the write and never after, because every one of these answers is
//! only worth having in advance. An operator told after the fact that their library
//! now points at nothing, that a hand-edit was overwritten, or that a download was
//! abandoned has been told about something they can no longer choose.
//!
//! Which is why it runs on a staged proposal too, not only on one about to land: a
//! review that withheld what a change would do until after the yes was given would be
//! asking for a yes to something unstated.

use crate::config::env::EnvFile;
use crate::config::store::is_secret;
use crate::config::{store::REDACTED, DATA_ROOT_KEY};
use crate::plural::s;
use crate::reconfigure::edits::{standing, Standing};
use crate::reconfigure::{relocating, Edited, Findings, Review};

use super::Ctx;

mod capability;
mod library;

pub(crate) use capability::waits_for_downloads;

/// The service name the settings baseline is kept under.
///
/// Reserved: a service is a Compose service id, which cannot carry a colon, so
/// nothing the stack declares can collide with this. Kept in the same record seeding
/// keeps rather than a second file, so one memory of what lemonfiber last wrote
/// covers both the services and the settings.
pub(crate) const SETTINGS: &str = "lemonfiber:settings";

/// The proposal, carrying what changing this setting comes to here — and turned away
/// where what was found says it must not go ahead.
///
/// The file is the one already read by the caller rather than read again, so the
/// value compared against is the one the write is about to land on.
/// The key and value are the ones the caller was given, not the ones the diff shows:
/// a credential's diff is two redactions, and everything worked out here is about the
/// value itself — where a path resolves, what a protocol switch comes to.
pub(crate) async fn assessed(
    ctx: &Ctx,
    review: Review,
    held: &EnvFile,
    change: (&str, &str),
    settled: bool,
) -> Review {
    let (key, value) = change;
    let mut findings = Findings {
        edited: edited(ctx, key, held.get(key), value),
        ..Findings::default()
    };
    capability::opening(ctx, &mut findings, key, value).await;
    let unread = if key == DATA_ROOT_KEY {
        let found = library::moving(ctx, std::path::Path::new(value)).await;
        findings.library = found.paths;
        found.unread
    } else {
        None
    };
    let refusal = refusal(&findings, unread, settled);
    let review = review.finding(findings);
    match refusal {
        Some(said) => review.blocked(said),
        None => review,
    }
}

/// Why this change must not simply be made, where it must not.
///
/// Ordered by whose decision it is. A library that would be left pointing at nothing
/// is nobody's to override — there is no version of that an operator is better off
/// with — so it is settled first and says what to do instead. What is still coming
/// down and a hand-edit found underneath are both the operator's own call, and each
/// says how to make it.
fn refusal(findings: &Findings, unread: Option<String>, settled: bool) -> Option<String> {
    if let Some(said) = unread.or_else(|| unresolvable(findings)) {
        return Some(said);
    }
    if settled {
        return None;
    }
    let active = findings.active.len();
    if active > 0 {
        return Some(format!(
            "{active} download{} still coming down, so nothing was written. Re-run with --wait \
             to let them finish first, or with --confirm to stop them where they are.",
            s(active)
        ));
    }
    findings.edited.as_ref().map(|_| {
        "this setting was changed outside lemonfiber since it last wrote there, so nothing was \
         written. Re-run with --confirm to write over that edit."
            .to_owned()
    })
}

/// Why the library would not survive the move, where it would not.
fn unresolvable(findings: &Findings) -> Option<String> {
    relocating::unresolvable(&findings.library)
}

/// The hand-edit found under this setting, where one was found.
///
/// Three values decide it, and the third is the record seeding already keeps: what
/// lemonfiber last wrote here. Without one there is no edit to find — a value the
/// file holds that lemonfiber never wrote cannot be told from the one setup left, and
/// calling it an edit would refuse the first change made on every machine.
///
/// Both values are withheld where the setting is one a listing withholds, because
/// this report is one a script can log.
fn edited(ctx: &Ctx, key: &str, found: Option<&str>, writing: &str) -> Option<Edited> {
    let recorded = recorded(ctx)?;
    let record = recorded.entry(SETTINGS, key)?;
    if standing(Some(record), found, writing) != Standing::Edited {
        return None;
    }
    let secret = is_secret(key);
    let shown = |value: &str| {
        if secret {
            REDACTED.to_owned()
        } else {
            value.to_owned()
        }
    };
    Some(Edited {
        wrote: shown(&record.value),
        found: found.map_or_else(|| "nothing — the line is gone".to_owned(), shown),
        secret,
    })
}

/// What lemonfiber last wrote, where a record was kept and could be read.
///
/// A record that is there but unreadable is left alone rather than judged against:
/// it is the same loss seeding refuses to overwrite, and a change that treated it as
/// absent would silently re-form it around whatever the file happens to hold.
pub(crate) fn recorded(ctx: &Ctx) -> Option<crate::baseline::Baseline> {
    match super::seed::load_baseline(ctx) {
        super::seed::Loaded::Formed(baseline) => Some(baseline),
        super::seed::Loaded::Fresh | super::seed::Loaded::Lost => None,
    }
}

/// Record what lemonfiber has just written, as the expected state the next change
/// compares against.
///
/// Best-effort, like every other record seeding keeps: a run that cannot persist it
/// still made the change, and what a lost write costs is that the next change reads
/// this setting as one lemonfiber has no record of. A credential is deliberately left
/// out — a second file holding a password would be a second file to leak one.
pub(crate) fn record(ctx: &Ctx, key: &str, value: &str) {
    if is_secret(key) {
        return;
    }
    let mut baseline = match super::seed::load_baseline(ctx) {
        super::seed::Loaded::Formed(baseline) => baseline,
        super::seed::Loaded::Fresh => crate::baseline::Baseline::new(),
        super::seed::Loaded::Lost => return,
    };
    baseline.record(SETTINGS, key, value, &ctx.stamp());
    super::seed::save_baseline(ctx, &baseline);
}

#[cfg(test)]
mod tests;
