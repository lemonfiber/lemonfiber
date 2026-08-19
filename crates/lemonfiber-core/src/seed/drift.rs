//! Telling an operator's edit from a stale default.
//!
//! Three values, not two: what lemonfiber last wrote, what is there now, and what it
//! wants. Two of them can only say *different*; three can say which of the two changed,
//! which is the whole difference between preserving an edit and reverting one.

use super::{client_field, Baseline, DownloadClient, RegisteredClient};

/// What lemonfiber observed about a connection it wants to make — the outcome of
/// the three-way comparison of what it last wrote, what the service now holds, and
/// what it would write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observed {
    /// The prerequisite service is not answering.
    Unavailable,
    /// The connection is not there.
    Absent,
    /// The connection is there and already at what lemonfiber would write.
    Present,
    /// It differs from the baseline while lemonfiber's intent is unchanged — an
    /// operator's edit, to preserve.
    Drifted,
    /// It still holds lemonfiber's baseline value, but lemonfiber's intent has moved
    /// on — its own old value, to bring up to date.
    Stale,
    /// Both the service's value and lemonfiber's intent have moved away from the
    /// baseline — a conflict lemonfiber must not resolve on its own.
    Conflicted,
    /// The service holds a value the operator set and lemonfiber adopted as the
    /// accepted state — theirs to keep, left as it is.
    Adopted,
    /// The service holds a value lemonfiber never wrote and has no baseline for — the
    /// operator's own, pre-existing, to adopt as the baseline rather than report as
    /// drift from an expectation that was never formed.
    Unmanaged,
}

/// What seed will do about a connection, decided from what it observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intent {
    /// The prerequisite is not up: skip it, and a later run completes it.
    Skip,
    /// It is not there: write it.
    Wire,
    /// It is there and correct: do nothing — this is what keeps a second run from
    /// changing anything.
    Leave,
    /// It is there but the operator changed it: preserve their change and report
    /// the drift, rather than reverting it.
    Preserve,
    /// It is lemonfiber's own value, now behind lemonfiber's intent: it should be
    /// brought up to date. Reported until an update path exists to apply it.
    Update,
    /// Both sides changed: present the conflict and leave the value, never resolving
    /// it silently.
    Ask,
    /// An adopted operator value still stands: keep it, changing nothing — the
    /// baseline already holds it as the accepted state.
    Keep,
    /// A pre-existing operator value with no baseline: adopt what the service holds
    /// as the baseline, writing nothing to the service.
    Adopt,
}

/// What seed will do about a connection found in the given state.
///
/// The idempotent, drift-aware policy in one place: an absent connection is
/// written, a present-and-correct one is left untouched so a second run changes
/// nothing, an operator's edit is preserved, lemonfiber's own value behind its
/// intent is brought up to date, a conflict is presented rather than resolved, an
/// adopted edit is kept as the accepted state, a pre-existing value with no baseline
/// is adopted rather than reported as drift, and an unavailable prerequisite is
/// skipped rather than failed so the run stays resumable.
#[must_use]
pub const fn intent(observed: Observed) -> Intent {
    match observed {
        Observed::Unavailable => Intent::Skip,
        Observed::Absent => Intent::Wire,
        Observed::Present => Intent::Leave,
        Observed::Drifted => Intent::Preserve,
        Observed::Stale => Intent::Update,
        Observed::Conflicted => Intent::Ask,
        Observed::Adopted => Intent::Keep,
        Observed::Unmanaged => Intent::Adopt,
    }
}

/// The three-way comparison of one managed value: what lemonfiber last recorded
/// (`expected`, `None` where it recorded nothing — value and whether it was written
/// or adopted), what the service now holds (`actual`, `None` where it holds no
/// value), and what lemonfiber would write (`desired`).
///
/// Like a merge, the three together say what a bare two-way `actual != desired`
/// cannot: whether a difference is the operator's edit to preserve, lemonfiber's
/// own value fallen behind its intent, an adopted value to keep, or a genuine
/// conflict. Where there is no baseline to judge against, a value the service holds
/// is the operator's own, pre-existing and unmanaged — adopted rather than
/// overwritten on a guess.
#[must_use]
pub fn reconcile(
    expected: Option<&crate::baseline::Record>,
    actual: Option<&str>,
    desired: &str,
) -> Observed {
    // An adopted value is read before "already at desired": a value the operator
    // adopted that lemonfiber also happens to want stays adopted — theirs to keep
    // even once lemonfiber's desired later moves — rather than being taken back as
    // lemonfiber's own. While the service still holds it, it stands; moved away from
    // it, in sync if now at desired, otherwise a fresh edit to preserve.
    if let Some(base) = expected {
        if base.origin.is_adopted() {
            return if actual == Some(base.value.as_str()) {
                Observed::Adopted
            } else if actual == Some(desired) {
                Observed::Present
            } else {
                Observed::Drifted
            };
        }
    }
    if actual == Some(desired) {
        return Observed::Present;
    }
    match expected {
        // lemonfiber's intent is unchanged from the baseline, so the difference is
        // the operator's.
        Some(base) if base.value == desired => Observed::Drifted,
        // The service still holds lemonfiber's baseline value; only lemonfiber's
        // intent has moved.
        Some(base) if actual == Some(base.value.as_str()) => Observed::Stale,
        // A baseline that matches neither side: both have moved away from it.
        Some(_) => Observed::Conflicted,
        // No baseline to judge against: the value the service holds is the operator's
        // own, never written by lemonfiber, to adopt rather than overwrite on a guess.
        None => Observed::Unmanaged,
    }
}

/// Whether every download client the service holds for the wanted set reads as
/// drift at once.
///
/// One drift is the operator's edit; every managed value of a service drifting
/// together is the shape of a schema change — a service upgrade renamed the fields
/// under lemonfiber, so what it wrote no longer matches where it reads, not the
/// operator hand-editing each. Taken with a version change, this is the sign to
/// re-baseline rather than report mass drift. False where the service holds none of
/// the wanted clients, since nothing drifted; only the ones actually present are
/// judged, so a client not there yet is not counted as un-drifted and does not veto
/// the re-baseline.
#[must_use]
pub fn wholesale_drift(
    existing: &[RegisteredClient],
    wanted: &[DownloadClient],
    expected: &Baseline,
    service: &str,
) -> bool {
    let present: Vec<&DownloadClient> = wanted
        .iter()
        .filter(|want| existing.iter().any(|have| same_endpoint(have, want)))
        .collect();
    !present.is_empty()
        && present.iter().all(|want| {
            let have = existing.iter().find(|have| same_endpoint(have, want));
            let base = expected.entry(service, &client_field(want));
            matches!(observe_client(have, want, base), Observed::Drifted)
        })
}

/// What a seed pass observes about a wanted client against the one the service
/// holds at its endpoint (`have`, `None` where it holds none) and what lemonfiber
/// last recorded: absent, or the three-way outcome of comparing the service's
/// category with lemonfiber's baseline and its desired one.
///
/// Shared with a reset's preview so the read-only "what would revert" and the pass
/// that reverts it judge drift the same way — the same canonicalised three-way
/// comparison, not two hand-inlined ones that could drift apart.
pub(crate) fn observe_client(
    have: Option<&RegisteredClient>,
    want: &DownloadClient,
    expected: Option<&crate::baseline::Record>,
) -> Observed {
    match have {
        None => Observed::Absent,
        // The three legs are compared by their canonical form, so a category a service
        // normalises on write is not read as drift from what lemonfiber holds. The
        // baseline still keeps the raw value it recorded; only the comparison is
        // canonicalised, here at the one place that knows the value is a category.
        Some(have) => reconcile(
            expected
                .map(|record| crate::baseline::Record {
                    value: canonical_category(&record.value).to_owned(),
                    at: record.at.clone(),
                    origin: record.origin,
                })
                .as_ref(),
            have.category
                .as_ref()
                .map(|category| canonical_category(&category.value)),
            canonical_category(&want.category.value),
        ),
    }
}

/// A download-client category compared without regard to surrounding whitespace,
/// which a service may trim on write — so a value differing only by it is not read
/// as drift, the same way [`same_path`] disregards a trailing slash a service adds.
/// Case is left as it is: a category can be case-sensitive, and folding it could
/// merge two the operator means to keep apart, which is worse than a reported
/// difference.
pub(super) fn canonical_category(value: &str) -> &str {
    value.trim()
}

/// Whether a registered client reaches the same endpoint as a wanted one.
///
/// The host and port together, never the name: this is what makes the match by
/// connection rather than by label, so a client already present under a different
/// name is not registered again.
pub fn same_endpoint(have: &RegisteredClient, want: &DownloadClient) -> bool {
    have.host == want.host && have.port == want.port
}

/// Whether two application addresses reach the same \*arr.
///
/// A trailing separator is ignored, as it is for a folder path: Prowlarr may
/// store the canonical form of the address it was given, and the wanted address
/// and its stored form must be recognised as one so a write is not made every run.
pub(super) fn same_base_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/') == b.trim_end_matches('/')
}

/// Whether two paths name the same folder.
///
/// A trailing separator is ignored, because a service commonly stores the
/// canonical form of the path it was given — dropping a trailing slash — and the
/// wanted path and its stored canonical form must be recognised as one. Without
/// this, a folder is re-registered every run and read-back never confirms the
/// write it just made.
pub(super) fn same_path(a: &str, b: &str) -> bool {
    canonical_root(a) == canonical_root(b)
}

/// A root-folder path in the form paths are compared in — the trailing separator
/// dropped, as [`same_path`] drops it — so one folder wanted by two \*arrs keys to
/// a single entry however each spells it.
pub(super) fn canonical_root(path: &str) -> String {
    path.trim_end_matches('/').to_owned()
}
