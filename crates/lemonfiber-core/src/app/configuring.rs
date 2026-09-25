//! Reading and changing one setting.
//!
//! Its own module rather than part of the lifecycle engine, because this is the
//! one command that *writes* what every other command then reads, and the writing
//! carries a duty the reading does not: a change with a consequence has to say so
//! at the moment it is made, which is the only moment the operator is deciding.
//!
//! So a change is weighed before it is written, never after. What the setting holds
//! now and what it would hold are put side by side; a change setup catalogued as
//! consequential is staged until somebody says yes to it; and a replacement for one
//! half of a credential is proven against the live service while the credential it
//! replaces is still the one on disk. A proposal that does not clear all of that
//! leaves the file exactly as it was and says which of them it failed.

mod proving;

use crate::config::{env::EnvFile, port_forward_from_env, store};
use crate::error::{Diagnose, Problem};
use crate::model::{ConfigReport, SettingReport};
use crate::origin;
use crate::reconfigure::{Consent, Review, Stance};
use crate::validate::{Credential, Validation};

use proving::Proving;

use super::{Ctx, Setting, Waiting};

/// What the operator said about this change beyond what the change is.
///
/// Two words a surface already has — a `--confirm` on a command line and a `confirm`
/// in a request body are one word, and so are the two `wait`s — carried together
/// because they are answers to the same question: something stands between this
/// change and the file, and here is what to do about it. One says go ahead anyway;
/// the other says let what is in flight finish first, which is the offer a reduction
/// makes rather than a way past it.
#[derive(Clone, Copy)]
struct Asked {
    /// Whether the operator has agreed to what the change costs.
    confirmed: bool,
    /// Whether they asked for what is still coming down to finish first.
    waiting: Waiting,
}

impl Asked {
    /// What the request said, taken off it.
    const fn of(change: &Setting) -> Self {
        Self {
            confirmed: change.confirmed,
            waiting: change.waiting,
        }
    }
}

/// Read or change settings.
///
/// A rehearsal reads and reports what it would have written without writing it,
/// so `--dry-run` means the same thing here as everywhere else.
///
/// The failure is boxed because a problem is a rare, cold thing that is cheaper to
/// move behind a pointer than to carry in every returned value.
///
/// # Errors
///
/// Returns the [`Problem`] for a machine with nowhere to keep settings, or for a
/// settings file that could not be read or written.
pub(crate) async fn set(ctx: &Ctx, change: Setting) -> Result<ConfigReport, Box<Problem>> {
    // Trimmed on the way in, for the same reason setup trims what is pasted into it:
    // a key copied from a dashboard carries a trailing newline, it authenticates
    // nowhere, and the file format has no way to mean the whitespace deliberately. The
    // parser already trims the name; the value was the half still taken literally.
    let change = Setting {
        value: change.value.trim().to_owned(),
        ..change
    };
    settings(ctx, Some(&change.key), Some(&change)).await
}

/// Read one setting, or all of them.
///
/// A read decides nothing, so there is no proposal to weigh and nothing to write.
///
/// # Errors
///
/// Returns the [`Problem`] for a machine with nowhere to keep settings, or for a
/// settings file that could not be read.
pub(crate) async fn get(ctx: &Ctx, key: Option<&str>) -> Result<ConfigReport, Box<Problem>> {
    settings(ctx, key, None).await
}

/// Both halves: the settings as they stand, and what a change to one comes to.
async fn settings(
    ctx: &Ctx,
    key: Option<&str>,
    change: Option<&Setting>,
) -> Result<ConfigReport, Box<Problem>> {
    let Some(path) = ctx.settings.env_file.as_deref() else {
        return Err(Box::new(store::Failure::Nowhere.problem()));
    };

    // Read before anything is decided rather than after the write, because the diff an
    // operator is shown is the difference between this file and the one proposed, and a
    // file read afterwards is already the answer to the question.
    let held = match store::read(path) {
        Ok(file) => file,
        Err(err) => return Err(Box::new(err.problem())),
    };

    let (file, changed, consequence, review) = match change {
        Some(change) => {
            let asked = Asked::of(change);
            let proposal = applying(ctx, path, held, (&change.key, &change.value), asked).await?;
            (
                proposal.file,
                proposal.review.differs(),
                proposal.consequence,
                Some(proposal.review),
            )
        }
        None => (held, false, None, None),
    };
    // Read once for the whole listing rather than per row: it is one file, and a
    // machine that cannot produce it answers *unknown* for every setting instead of
    // failing a read that is otherwise perfectly good.
    let recorded = super::reconfiguring::recorded(ctx);
    // What a plugin set is read off the journal, against the record of what is installed
    // — and a record that will not read is carried as that, so no setting is called
    // orphaned by a plugin the machine may still have.
    let journalled = super::targets::layout(ctx)
        .map(|paths| {
            super::recover::journal_at(&paths.journal())
                .changes()
                .to_vec()
        })
        .unwrap_or_default();
    let installed: Option<Vec<String>> = super::plugins::read(ctx).ok().map(|register| {
        register
            .installed()
            .iter()
            .map(|one| one.plugin.clone())
            .collect()
    });
    let settings = store::shown(&file)
        .into_iter()
        .filter(|setting| key.is_none_or(|wanted| setting.key == wanted))
        .map(|shown| {
            let holds = file.get(&shown.key).unwrap_or_default();
            let origin =
                origin::of_journalled(&shown.key, holds, &journalled, installed.as_deref())
                    .unwrap_or_else(|| {
                        origin::of_setting(
                            &shown.key,
                            recorded.as_ref().and_then(|held| {
                                held.entry(super::reconfiguring::SETTINGS, &shown.key)
                            }),
                        )
                    });
            SettingReport::of(shown, origin)
        })
        .collect();

    Ok(ConfigReport {
        settings,
        changed,
        rehearsed: ctx.dry_run,
        consequence,
        review,
    })
}

/// A proposed change, the sentence saying what making it costs, and the settings as
/// they stand once it has been dealt with.
struct Proposal {
    /// The difference, and where it stands.
    review: Review,
    /// What making it decided, where it decided something worth stating.
    consequence: Option<String>,
    /// The file as it now is — changed where the proposal reached it, and exactly as
    /// it was where it did not.
    ///
    /// Carried rather than read back off the disk, because a second read would be a
    /// second failure to report for one command, and the copy that failed would be
    /// the one no test could reach.
    file: EnvFile,
}

/// The change weighed, proven where a service can prove it, and written where nothing
/// stands in the way.
///
/// # Errors
///
/// Returns the [`Problem`] for a settings file that could not be written.
async fn applying(
    ctx: &Ctx,
    path: &std::path::Path,
    held: EnvFile,
    change: (&str, &str),
    asked: Asked,
) -> Result<Proposal, Box<Problem>> {
    let (key, value) = change;
    // The offer to wait, taken up. It runs before anything is weighed, so what the
    // proposal then finds in flight is what is still in flight after the wait — and
    // only for a change that takes something away, since a wait asked of one that
    // does not would sit in front of every download on the machine for nothing.
    if asked.waiting == Waiting::ForTheDownloads
        && !ctx.dry_run
        && super::reconfiguring::waits_for_downloads(ctx, key, value)
    {
        super::engine::drained(ctx, &[]).await;
    }

    let review = weighed(ctx, &held, key, value, asked.confirmed).await;
    let consequence = stated(ctx, &review, &held, key, value);
    let mut file = held;
    if review.writes() {
        if let Err(err) = store::set(path, key, value) {
            return Err(Box::new(err.problem()));
        }
        file.set(key, value);
        // Recorded as what lemonfiber last wrote here, so the next change can tell an
        // operator's edit from lemonfiber's own value rather than overwriting one
        // without saying so.
        super::reconfiguring::record(ctx, key, value);
    }
    Ok(Proposal {
        review,
        consequence,
        file,
    })
}

/// Where the proposal stands once everything that could stop it has been asked.
///
/// The classification comes first and the service second, deliberately: a change
/// nobody has agreed to is not going to happen, and reaching a live indexer to
/// prove a key for it would be spending somebody's rate limit on a decision that
/// has not been taken.
async fn weighed(ctx: &Ctx, held: &EnvFile, key: &str, value: &str, confirmed: bool) -> Review {
    let review = Review::proposed(
        key,
        held.get(key),
        value,
        &Consent {
            settled: confirmed,
            rehearsing: ctx.dry_run,
        },
    );
    if !review.differs() {
        return review;
    }
    // An area the operator declared unmanaged is not lemonfiber's to write, and the
    // refusal carries the reason they gave rather than one of lemonfiber's own. Asked
    // after the comparison above on purpose: a setting already holding what was asked
    // for has nothing to refuse, and saying "you told me to leave this alone" about a
    // change that would do nothing reads as an obstacle where there is none.
    if let Some(because) = crate::unmanaged::covering(&ctx.settings.unmanaged, key) {
        return review.blocked(format!(
            "you declared this unmanaged, so lemonfiber does not write it: {because}"
        ));
    }
    // What the change comes to on this machine — where the library would land, what
    // is still coming down, what was edited underneath, what it opens and keeps —
    // worked out for a staged proposal as well as one about to land. A review that
    // withheld this until after the yes was given would be asking for a yes to
    // something unstated.
    let review = super::reconfiguring::assessed(ctx, review, held, (key, value), confirmed).await;
    if !review.writes() {
        return review;
    }
    match proving::wanted(held, key, value) {
        Proving::Nothing | Proving::Incomplete => review,
        Proving::Unreadable(why) => review.blocked(why),
        Proving::Replacement(replacement) => answered(ctx, review, &replacement, confirmed).await,
    }
}

/// The proposal once the live service has answered about the replacement.
///
/// A service that answered and *refused* is the one answer no confirmation gets past.
/// The whole point of proving a replacement first is that a bad paste must not cost the
/// operator the credential that works, and a blanket yes is exactly what a bad paste
/// would be waved through by.
///
/// Nothing answering at all is a different thing, and it is confirmable: an operator
/// working offline, or reaching a provider this machine cannot see, may know the
/// credential is right, and refusing them forever would make the setting unchangeable —
/// which is the trap reconfiguration exists to close. It is stored unproven and said to
/// be unproven.
///
/// A credential that authenticated but cannot do its job is stored. It is the right
/// credential; what is wrong is the account behind it, and that is not fixed by keeping
/// the old one.
async fn answered(ctx: &Ctx, review: Review, replacement: &Credential, confirmed: bool) -> Review {
    let proof = ctx.validator.validate(replacement).await.withheld();
    let refusal = match &proof {
        Validation::Rejected { detail } => Some(format!(
            "the service refused the replacement, so the one in force was kept: {detail}"
        )),
        Validation::Unreachable { detail } if !confirmed => Some(format!(
            "the replacement could not be proven, so the one in force was kept: {detail}. \
             Confirm the change to store it unproven"
        )),
        Validation::Valid { .. } | Validation::Degraded { .. } | Validation::Unreachable { .. } => {
            None
        }
    };
    let review = review.proven(proof);
    match refusal {
        Some(why) => review.blocked(why),
        None => review,
    }
}

/// What the proposed change costs, where it costs anything.
///
/// One sentence rather than a list, because one call changes one setting: the change
/// either has a cost worth stating or it has none. Stated for a change that is only
/// staged as well as for one that landed — a review step that withheld the cost until
/// after the write would be a review step in name only.
///
/// Naming a front door is the one change whose consequence does not depend on what
/// the setting was before. Every other answer this product gives about the door is
/// worked out afresh, so a stack that changes is answered about as it is; a named
/// one is answered about as it was decided, and saying so belongs at the moment it
/// is decided.
///
/// The forwarded port is nothing where the stack does not torrent: a forwarded port
/// buys it nothing, so the sentence would be about a problem this operator cannot
/// have. It is worked out from the difference rather than from the file on disk, so a
/// rehearsal is told what it would cost as plainly as a write is told what it did.
fn stated(ctx: &Ctx, review: &Review, held: &EnvFile, key: &str, value: &str) -> Option<String> {
    if review.stance == Stance::Unchanged {
        return None;
    }
    if key == crate::config::FRONT_DOOR_KEY {
        return Some(crate::door::KEPT.to_owned());
    }
    // Every answer setup wrote says what changing it affects, and the catalogue is the
    // one place that knows. A surface that writes a setting cannot then state a cost
    // the rest of the product disagrees with, and a decision nobody catalogued says
    // nothing rather than a guess.
    if let Some(entry) = crate::reconfigure::decision(key) {
        return Some(format!("changing this affects {}", entry.affects));
    }
    if !ctx.settings.protocols.torrent {
        return None;
    }
    let mut proposed = held.clone();
    proposed.set(key, value);
    super::unforwarded::on_change(
        &port_forward_from_env(held),
        &port_forward_from_env(&proposed),
    )
    .map(str::to_owned)
}

#[cfg(test)]
mod tests;
