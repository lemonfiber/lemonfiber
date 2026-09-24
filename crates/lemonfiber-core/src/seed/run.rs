//! Wiring the stack's services to each other, idempotently.
//!
//! One connection lemonfiber mints (qBittorrent's web UI password); the rest it
//! reads and writes. The orchestration lives here so [`crate::app::dispatch`] stays a
//! table of one-line calls rather than carrying the whole graph.

use std::path::{Path, PathBuf};

use crate::app::targets::{project_directory, target_for};
use crate::app::Ctx;
use crate::error::Diagnose;
use crate::error::Problem;
use crate::ports::docker::LogQuery;

mod aggregators;
mod applications;
mod arrs;
mod baseline;
mod clients;
mod fulfilment;
mod published;
pub(crate) use published::published_as;
mod subtitles;
use fulfilment::seed_fulfilment_targets;
pub(crate) mod identity;
mod reset;

use applications::{seed_applications, skipped};
use arrs::{arr_download_clients, read_servarr_key, seed_arr, ArrSeeding};
use baseline::{escalate_broken_roots, wanted_roots, DATA_ROOT, SCHEMA_VERSION_FIELD};
// Reached by reconfiguration as well as by seeding: what the \*arrs that file media
// are, and the record of what lemonfiber last wrote. One answer to each, rather than a
// second reader beside this one that could disagree with it.
pub(crate) use arrs::servarr_arrs;
pub(crate) use baseline::{load_baseline, save_baseline, Loaded};
use clients::{
    category_for, download_clients, qbittorrent_target, read_sabnzbd_key, seed_qbittorrent_password,
};
use identity::seed_jellyfin_identity;
pub(crate) use reset::reset_connections;

/// Wire the stack's services to each other, idempotently, and report what was
/// wired and what a re-run still owes — or, on a run that only says what it would do,
/// report the same pass with every write left out.
///
/// **A rehearsal issues nothing but reads, and that is the rule rather than a summary
/// of one.** It is stricter than "registers no connection", because several of the
/// things this pass would call reading are `POST`s: a sign-in opens a session on
/// somebody else's service, a torrent client answers a password test the same way, and
/// two of the keys published at the end are read by being minted. Each is state left
/// behind by a run that promised to leave none, so none of them is made.
///
/// Four things the rule costs, each reported as something this pass could not tell
/// rather than told wrong:
///
/// - whether the torrent password lemonfiber recorded is still the one in force, which
///   is answered by signing in;
/// - what the household is told, and which \*arrs the request service hands a request
///   to — both read as the owner, and the owner's session is a sign-in;
/// - whether a drifted download client still reaches anything, which the \*arr answers
///   only by being asked to test it. The drift is reported; what is left out is the
///   claim that it broke something.
///
/// And the keys the stack's own services read are named rather than gathered: the media
/// server mints its key when it is asked for one, and the listening server has no
/// account at all until this pass makes one. A question that gathered them would have
/// created the very things it promised only to describe.
///
/// Everything else is the same walk — the same reads, the same three-way comparison,
/// the same words — with the registering and the minting not done.
///
/// The gate is held at each write rather than above the pass, because the pass is where
/// the report comes from: a rehearsal that stopped at the door would have nothing to
/// say, and one that surveyed separately would be a second opinion about what
/// lemonfiber intends — and the one nobody runs is the one that goes wrong.
///
/// One connection is unlike the rest: qBittorrent's web UI password, the
/// credential lemonfiber mints rather than reads — its temporary password is
/// read from the container's log, replaced with a generated one, and the
/// generated one recorded where the forwarded-port push reads it. The rest of
/// the graph reads a credential and writes a connection: each media-filing
/// \*arr's root folders, and its download clients (`SABnzbd` and qBittorrent).
/// Prowlarr's app sync registers each of those \*arrs back into Prowlarr, so it
/// pushes them indexers. It then makes Jellyfin the identity source for Seerr, so
/// the household signs in once. Bindery wiring lands next.
pub(crate) async fn seed(ctx: &Ctx, adopt: bool) -> Result<crate::seed::Report, Box<Problem>> {
    let mut manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let mut wirings = Vec::new();

    wirings.extend(withheld(&mut manifest.services, &ctx.settings.unmanaged));

    // What each of the stack's asks comes to, settled once for the whole pass. The
    // connections below reach *whatever fills* what they ask for rather than a
    // service this crate names, which is what makes something standing in for a
    // bundled service a change to the manifest and the setting rather than a change
    // here. Settled after withholding, because a service the operator manages
    // themselves is not one this pass wires to.
    // What is installed is read here too, because a plugin's service that claims what
    // the stack asks for is a candidate like any other — and a record that is there and
    // will not read refuses the seed rather than letting it wire past a contest nobody
    // could see.
    let installed = crate::app::plugins::read(ctx)?;
    let filled = crate::wiring::filled(&crate::wiring::settle(
        &manifest,
        installed.installed(),
        &crate::app::targets::chosen_fillers(ctx),
    ));

    // qBittorrent's password, the one credential lemonfiber mints. Collecting the
    // optional target into a list wires it where the stack has it and does nothing
    // where it does not, without a branch a test could not reach. The generated
    // value is kept to register qBittorrent as a download client below.
    let mut qbittorrent_password = None;
    for target in qbittorrent_target(&manifest.services)
        .into_iter()
        .collect::<Vec<_>>()
    {
        let (wiring, generated) = seed_qbittorrent_password(ctx, &target).await;
        qbittorrent_password = generated.or(qbittorrent_password);
        wirings.push(wiring);
    }

    // A later run mints nothing — the password in force is the one already set —
    // so the value recorded on the run that minted it stands in. Without this an
    // \*arr that came up after the first seed would never learn about qBittorrent,
    // since its password cannot be read back from qBittorrent itself.
    let qbittorrent_password =
        qbittorrent_password.or_else(|| crate::app::targets::recorded_qbittorrent_password(ctx));

    // Root folders and download clients for each \*arr that files media. The
    // download clients' own credentials are read once: SABnzbd's key from its
    // config, qBittorrent's the password minted or recorded above.
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let sabnzbd_key = read_sabnzbd_key(ctx, &manifest.services, project.as_deref()).await;
    // The host data root, read once, so each \*arr's root folders can be checked
    // against the filesystem they file into and a folder pointing nowhere raised as a
    // warning.
    let data_root = crate::app::targets::data_root(ctx);
    let arrs = servarr_arrs(&manifest.services, project.as_deref());
    // A root folder one \*arr wants and another does too is contested: two \*arrs
    // on one folder would each rewrite the other's files, so it is refused rather
    // than wired. Detected across every \*arr up front, before any is wired.
    let root_claims: Vec<(&str, Vec<crate::ports::service::RootFolder>)> = arrs
        .iter()
        .map(|arr| (arr.target.name.as_str(), wanted_roots(&arr.media_types)))
        .collect();
    let contested = crate::seed::contested_roots(
        root_claims
            .iter()
            .map(|(name, roots)| (*name, roots.as_slice())),
    );
    // The expected-state baseline — what seeding last wrote into each service — is
    // loaded so this pass records against what earlier ones set, and saved once at
    // the end. Unlike the per-connection journal, it persists across runs: it is the
    // only memory of what lemonfiber wrote, which a later run reads to tell an
    // operator's edit from lemonfiber's own value.
    // The record may be genuinely absent (a first seed), read, or there but
    // unreadable — lost. A lost record cannot tell an operator's edit from
    // lemonfiber's own, so this pass cannot assess drift; rather than guess against an
    // empty baseline, it says so and offers re-baselining. The deliberate re-baseline
    // is `adopt`, which takes current state on as the new record, so an adopt pass
    // proceeds and re-forms it while an ordinary seed leaves the lost record untouched
    // rather than silently replacing it.
    let loaded = load_baseline(ctx);
    let lost = matches!(loaded, Loaded::Lost);
    let mut baseline = match loaded {
        Loaded::Formed(baseline) => baseline,
        Loaded::Fresh | Loaded::Lost => crate::baseline::Baseline::new(),
    };
    // Each \*arr's wiring is independent of the others, so the \*arrs are seeded at
    // once rather than in series: a pass's time then tracks the slowest \*arr, not
    // their sum. Each records what it wrote into its own baseline, read against the
    // loaded snapshot; the records are folded back into one below, and since a
    // field key carries the service, no two \*arrs collide.
    let seeding = ArrSeeding {
        contested: &contested,
        sabnzbd_key: sabnzbd_key.as_deref(),
        qbittorrent_password: qbittorrent_password.as_deref(),
        data_root: data_root.as_deref(),
        expected: &baseline,
        adopt,
    };
    let seeded =
        futures_util::future::join_all(arrs.iter().map(|arr| seed_arr(ctx, arr, &seeding))).await;
    for (arr_wirings, records) in seeded {
        wirings.extend(arr_wirings);
        baseline.merge(&records);
    }

    // Prowlarr's app sync: register each of those media-filing \*arrs back into
    // Prowlarr, so it pushes them its indexers. Bindery is left out here — it is
    // not one of Prowlarr's applications and is wired via Torznab instead.
    wirings.extend(seed_applications(ctx, &manifest.services, project.as_deref()).await);

    // The book *arr, which the aggregator cannot register itself into: it keeps its own
    // list of aggregators and pulls from them, so it is told where one is instead.
    wirings.extend(
        aggregators::seed_aggregators(ctx, &manifest.services, project.as_deref(), &filled).await,
    );

    // Jellyfin as Seerr's identity source: one household account, not two.
    // Jellyfin has no key to read, so its admin password is minted and recorded
    // like qBittorrent's, then Seerr is pointed at it.
    //
    // Ahead of everything that talks to the request service, because this is what
    // gives it an owner. A request service with none refuses the credential, and a
    // pass that asked it for anything first would report a fresh stack as broken and
    // then, in the same run, fix what it had just reported.
    let (identity_wirings, identity_records) =
        seed_jellyfin_identity(ctx, &manifest.services, &baseline, &filled).await;
    wirings.extend(identity_wirings);
    baseline.merge(&identity_records);

    // The *arrs the request service hands a request to. Without this the household
    // can ask and nothing downstream ever hears, and with it the request surface
    // offers only what the stack can actually deliver.
    wirings.extend(seed_fulfilment_targets(ctx, &manifest.services, project.as_deref()).await);

    // The keys the stack's own services read out of the environment. Three of them
    // are configured that way and by no other means — the quality sync, the archive
    // extractor and the dashboard — so without this they run with nothing, and the
    // quality sync refuses its whole configuration over a single undefined name.
    wirings.push(
        published::publish_keys(
            ctx,
            &manifest.services,
            project.as_deref(),
            sabnzbd_key.as_deref(),
        )
        .await,
    );

    // The subtitle finder, told which \*arrs to watch. Until it is, it has nothing
    // to look at, and a household gets subtitles for nothing — which looks exactly
    // like releases that happen to have none.
    wirings.extend(subtitles::seed_subtitles(ctx, &manifest.services, project.as_deref()).await);

    // Persist what this pass recorded as the baseline a later run compares against —
    // unless the record was lost and this is not an adopt pass, in which case the
    // lost record is left as it is rather than silently replaced, and re-baselining is
    // left to the deliberate `adopt`.
    //
    // And unless this run only said what it would do. The baseline is the only memory
    // of what lemonfiber wrote, so a rehearsal that saved one would have the next real
    // run compare against a record of connections nobody made — which is the drift
    // question answered wrong in the one direction that silently overwrites an
    // operator's own value. The whole pass records into `baseline` in memory either
    // way, because that is what the comparison is made from; this is the line that
    // makes the difference between a question and an answer.
    if (!lost || adopt) && !ctx.dry_run {
        save_baseline(ctx, &baseline);
    }

    let assessment = if lost && !adopt {
        crate::seed::Assessment::Unassessable
    } else {
        crate::seed::Assessment::Assessed
    };
    Ok(crate::seed::Report {
        wirings,
        assessment,
        rehearsed: ctx.dry_run,
        // Read from the manifest rather than from what this pass reached, because a
        // service it cannot speak to is one it never tried — and a list assembled from
        // what was attempted could only ever hold the attempts.
        unsupported: crate::app::targets::unsupported_here(&manifest.services, project.as_deref()),
    })
}

/// Take the services the operator declared unmanaged out of the manifest, and say what
/// was taken, in the words they gave for taking it.
///
/// Removed at the top of a pass rather than gated at each of the dozen places that
/// would otherwise write to one. A gate per write point is a gate somebody adds a thirteenth
/// write beside, and the promise being kept — that lemonfiber observes and never
/// writes — is one a thirteenth write breaks silently.
///
/// Reported rather than simply absent, and reported as settled information rather than
/// as drift: a run that said nothing about a service the operator asked it to leave
/// alone would be indistinguishable from one that forgot the service existed.
fn withheld(
    services: &mut Vec<lemonfiber_manifest::Service>,
    declared: &[(String, String)],
) -> Vec<crate::seed::Wiring> {
    let mut observed = Vec::new();
    services.retain(|service| {
        let Some(because) = crate::unmanaged::covering(declared, &service.id) else {
            return true;
        };
        observed.push(crate::seed::Wiring::settled(
            service.name.clone(),
            crate::seed::State::Observed {
                reason: because.to_owned(),
            },
        ));
        false
    });
    observed
}

/// The download-client wirings lemonfiber manages, as a caller that only reads them needs
/// them: each \*arr, the clients lemonfiber would write there, and what it last recorded
/// for each.
///
/// Here rather than where it is used, so the read-only half of drift and the writing half
/// gather their inputs the same way. A diagnosis that worked out the wanted clients for
/// itself would be a second opinion about what lemonfiber intends, and the two would drift
/// apart exactly where an operator most needs them not to.
///
/// Nothing where the baseline could not be read. A record that is there but unreadable
/// cannot tell an operator's edit from lemonfiber's own value, and reporting drift against
/// a baseline that is not there would call every wiring in the stack an edit.
/// The request service to ask about the household's telling, and what lemonfiber
/// last recorded setting it to.
///
/// Both or neither: a stack with no request service has nothing to ask, and a
/// baseline that was never formed leaves the recorded value absent — which the check
/// reads as nobody having set this rather than as a value to have drifted from.
pub(crate) fn managed_telling(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
) -> (
    Option<std::sync::Arc<dyn crate::ports::service::Requests>>,
    Option<crate::baseline::Record>,
) {
    let seerr = identity::seerr_service(services).map(|base| {
        std::sync::Arc::new(crate::seerr::Seerr::new(ctx.http.clone(), &base, "seerr"))
            as std::sync::Arc<dyn crate::ports::service::Requests>
    });
    let recorded = match load_baseline(ctx) {
        Loaded::Formed(baseline) => baseline.entry("seerr", crate::seed::TELLING).cloned(),
        Loaded::Fresh | Loaded::Lost => None,
    };
    (seerr, recorded)
}

pub(crate) async fn managed_wirings(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    project: Option<&Path>,
) -> Vec<crate::doctor::wiring::Managed> {
    let Loaded::Formed(baseline) = load_baseline(ctx) else {
        return Vec::new();
    };
    let sabnzbd_key = read_sabnzbd_key(ctx, services, project).await;
    let qbittorrent_password = crate::app::targets::recorded_qbittorrent_password(ctx);
    servarr_arrs(services, project)
        .into_iter()
        .map(|arr| {
            let clients = arr_download_clients(
                &arr,
                sabnzbd_key.as_deref(),
                qbittorrent_password.as_deref(),
            )
            .into_iter()
            .map(|want| crate::doctor::wiring::Wired {
                recorded: baseline
                    .entry(&arr.target.name, &crate::seed::client_field(&want))
                    .cloned(),
                want,
            })
            .collect();
            crate::doctor::wiring::Managed {
                target: arr.target,
                clients,
            }
        })
        .collect()
}

/// The temporary password qBittorrent announced in its log, if it has.
async fn read_temporary_password(ctx: &Ctx, service: &str) -> Option<String> {
    let mut lines = ctx
        .engine
        .logs(
            &ctx.settings.project,
            &[service.to_owned()],
            LogQuery::recent(TEMP_PASSWORD_LOG_LINES),
        )
        .await
        .ok()?;

    let mut log = String::new();
    while let Some(line) = lines.recv().await {
        log.push_str(&line.line);
        log.push('\n');
    }
    crate::qbittorrent::temporary_password(&log)
}

/// Record the generated password where the forwarded-port push reads it — the
/// `QBITTORRENT_PASSWORD` setting in the environment file. Best-effort: a value
/// that could not be written is reported by the push's own missing-password
/// message rather than failing the wiring that did land.
fn record_qbittorrent_password(ctx: &Ctx, password: &str) {
    crate::app::targets::record_secret(ctx, crate::config::QBITTORRENT_PASSWORD_KEY, password);
}

/// How many lines back to read for qBittorrent's start-up announcement. Its
/// temporary password is printed once, early, so a generous tail finds it well
/// after start without pulling the whole log.
const TEMP_PASSWORD_LOG_LINES: u32 = 200;

#[cfg(test)]
mod tests;
