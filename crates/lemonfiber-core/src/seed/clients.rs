//! Pointing each service at the download clients it should use.
//!
//! The connection that carries drift: a category is the one value an operator edits and
//! lemonfiber also writes, so this is where the three-way reconcile actually bites.

use super::{
    intent, observe_client, observe_or_skip, record_outcome, same_endpoint, unreached, wire_one,
    Baseline, Client, DownloadClient, Intent, Journal, Naming, Observed, RegisteredClient, State,
    Wiring,
};

/// Wire a service's download clients: register the ones it lacks, leave the ones
/// it already has, and record each write as a change.
///
/// The same shape as [`wire_root_folders`], and the same two gates — an
/// unanswering service skips every client so a later run completes them, a
/// refusal fails. The difference is what "already there" means: a client is
/// matched by the endpoint it reaches, its host and port, not by its label, so a
/// client the operator renamed is recognised as the same connection and left
/// alone rather than registered a second time under lemonfiber's name.
pub async fn wire_download_clients(
    client: &dyn Client,
    service: &str,
    wanted: &[DownloadClient],
    journal: &mut Journal,
    baselines: &mut Baselines<'_>,
    at: &str,
) -> Vec<Wiring> {
    let existing = match observe_or_skip(client.download_clients().await, wanted, |want| {
        describe_client(service, want)
    }) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    // The id of the service's client behind each drifted wiring, parallel to
    // `wirings` — `None` where the wiring is not a drift. Kept alongside so a drift
    // the service can no longer reach can be raised from information to a warning
    // once the loop has decided every state, without a second three-way pass.
    let mut drifted_ids: Vec<Option<String>> = Vec::new();
    for want in wanted {
        let (wiring, drifted_id) =
            wire_one_client(client, service, want, &existing, baselines, journal, at).await;
        wirings.push(wiring);
        drifted_ids.push(drifted_id);
    }
    escalate_unreachable(client, &mut wirings, &drifted_ids).await;
    wirings
}

/// Wire one download client against what the service holds, returning how it turned
/// out and — where it is a drift — the id of the client it sits on, so a later pass
/// can ask the service whether that drift left the client unreachable.
async fn wire_one_client(
    client: &dyn Client,
    service: &str,
    want: &DownloadClient,
    existing: &[RegisteredClient],
    baselines: &mut Baselines<'_>,
    journal: &mut Journal,
    at: &str,
) -> (Wiring, Option<String>) {
    // What lemonfiber last recorded here — the expected leg of the three-way
    // comparison, read before the pass writes into `records` so the two do not both
    // borrow it. The wanted client is matched to one the service already holds by the
    // endpoint it reaches, found once so the observation, the value to present on a
    // conflict, and the value to adopt are all read from the same client.
    let field = client_field(want);
    let base = baselines.expected.entry(service, &field);
    let have = existing.iter().find(|have| same_endpoint(have, want));
    let found = have
        .and_then(|have| have.category.as_ref())
        .map(|category| category.value.clone());
    let observed = observe_client(have, want, base);
    // Adoption is deliberate, so only an adopt pass takes a value on, and only where
    // there is one to take: it promotes a drift or an unmanaged value to the accepted
    // baseline; an ordinary seed reports both and records neither.
    let adopting = baselines.adopt
        && found.is_some()
        && matches!(observed, Observed::Drifted | Observed::Unmanaged);
    // A reset reverts what an ordinary seed would preserve or report — an edit,
    // lemonfiber's own value fallen behind, a conflict, or a previously adopted value
    // — by the id the drifted client carries, which it always has.
    let drifted_from_ours = matches!(
        observed,
        Observed::Drifted | Observed::Stale | Observed::Conflicted | Observed::Adopted
    );
    let reverting_id = if baselines.reset && drifted_from_ours {
        have.map(|have| have.id.clone())
    } else {
        None
    };
    let state = if let Some(id) = reverting_id {
        // Write lemonfiber's category back over the operator's, in place. A revert that
        // lands records lemonfiber's value below; one the service refuses leaves the
        // value as it was, reported as the failure it is.
        match client.update_download_client(&id, want).await {
            Ok(()) => State::Wired,
            Err(failure) => unreached(&failure),
        }
    } else if baselines.reset {
        // A reset reverts drift and only drift. A client that is not a drift to revert
        // — absent, already at lemonfiber's value, or the operator's own unmanaged one
        // — is left exactly as it is: never registered, adopted or recorded anew, so a
        // confirmed reset does no more than the preview showed.
        State::AlreadyWired
    } else if adopting {
        State::Adopted
    } else {
        wire_by_intent(client, service, want, observed, found.as_ref(), journal, at).await
    };
    record_outcome(
        baselines,
        service,
        want,
        &state,
        adopting,
        found.as_ref(),
        at,
    );
    // A drift carries the id of the client it sits on — the one to ask the service
    // about below. Every other state carries none.
    let drifted_id = if matches!(state, State::Drifted) {
        have.map(|have| have.id.clone())
    } else {
        None
    };
    (
        Wiring::settled(describe_client(service, want), state),
        drifted_id,
    )
}

/// The seed policy's verdict for a client that is neither being reverted nor adopted:
/// an absent one is written, one already at lemonfiber's value is left, an operator's
/// edit is preserved, lemonfiber's own value behind its intent is reported stale, a
/// two-sided change is presented as a conflict, an adopted value is kept, and a
/// pre-existing value with no baseline is reported unmanaged. Unavailable never
/// reaches here — a read-back failure was handled above — so it folds onto `Leave`.
async fn wire_by_intent(
    client: &dyn Client,
    service: &str,
    want: &DownloadClient,
    observed: Observed,
    found: Option<&String>,
    journal: &mut Journal,
    at: &str,
) -> State {
    match intent(observed) {
        Intent::Wire => {
            wire_one(
                client.register_download_client(want),
                client.download_clients(),
                |rows| {
                    rows.iter()
                        .find(|have| same_endpoint(have, want))
                        .map(|have| have.id.clone())
                },
                Naming {
                    service,
                    resource: CLIENT,
                    noun: "download client",
                },
                journal,
                at,
            )
            .await
        }
        Intent::Preserve => State::Drifted,
        Intent::Update => State::Stale,
        // Present both sides of the conflict: the value the operator set, drawn from the
        // client already matched above, beside the one lemonfiber would write. The value
        // is left as it is — presenting is not resolving.
        Intent::Ask => State::Conflicted {
            yours: found.cloned(),
            ours: want.category.value.clone(),
        },
        Intent::Keep => State::Adopted,
        Intent::Adopt => State::Unmanaged,
        Intent::Leave | Intent::Skip => State::AlreadyWired,
    }
}

/// Raise each drifted download client the service can no longer reach from
/// information to a warning.
///
/// A drift is the operator's own edit and reported as information; but a drifted
/// client the service's own test finds unreachable has broken the stack — nothing
/// downloads through it — so it is raised with the failure named and a remediation
/// offered. The service is asked to test its clients only when a drift was found,
/// and a service that will not run the test at all leaves the drifts as the
/// information they already are, since nothing was proven broken.
async fn escalate_unreachable(
    client: &dyn Client,
    wirings: &mut [Wiring],
    drifted_ids: &[Option<String>],
) {
    let wanted: Vec<&String> = drifted_ids.iter().flatten().collect();
    if wanted.is_empty() {
        return;
    }
    let Ok(probes) = client.test_download_clients().await else {
        return;
    };
    for (wiring, drifted_id) in wirings.iter_mut().zip(drifted_ids) {
        let Some(id) = drifted_id else { continue };
        let Some(probe) = probes.iter().find(|probe| &probe.id == id) else {
            continue;
        };
        if probe.reachable {
            continue;
        }
        let detail = probe
            .detail
            .clone()
            .unwrap_or_else(|| "the service could not reach it".to_owned());
        wiring.escalate(
            format!("the download client is unreachable: {detail}"),
            "check the client is running and reachable at the address the service uses".to_owned(),
        );
    }
}

/// A download-client connection's description for the report.
pub(super) fn describe_client(service: &str, client: &DownloadClient) -> String {
    format!("{} into {service}", client.name)
}

/// The kind of resource a download-client wiring is, as the journal and the service both
/// name it. Named once so a change seeding creates and a change a repair rewrites read as
/// one story about download clients rather than two about differently-spelled things.
pub const CLIENT: &str = "downloadclient";

/// The baseline field a download client's value is recorded under: the endpoint it
/// reaches, host and port, so it is keyed the way the client itself is matched —
/// by connection, not by the label an operator can rename.
pub fn client_field(client: &DownloadClient) -> String {
    format!("{CLIENT}:{}:{}", client.host, client.port)
}

/// The baseline a drift-aware wiring reads against and records into, and whether
/// this is an adopt pass. Grouped so the wiring call carries one baseline argument
/// rather than three: what lemonfiber last recorded, where this run records what it
/// leaves, and whether an operator's edit is promoted to adopted rather than merely
/// preserved.
pub struct Baselines<'a> {
    /// What lemonfiber last recorded — the expected leg of the comparison.
    pub expected: &'a Baseline,
    /// Where this run records what it leaves as the baseline a later run reads.
    pub records: &'a mut Baseline,
    /// Whether this pass promotes each drifted value to adopted, recording what the
    /// service holds as the operator's accepted state.
    pub adopt: bool,
    /// Whether this pass reverts each drifted value to lemonfiber's own — the opposite of
    /// adopt: it writes lemonfiber's category over the operator's edit rather than keeping
    /// it, the connection side of a full reset. Never set with `adopt`.
    pub reset: bool,
}
