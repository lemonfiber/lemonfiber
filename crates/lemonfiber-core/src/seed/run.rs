//! Wiring the stack's services to each other, idempotently.
//!
//! One connection lemonfiber mints (qBittorrent's web UI password); the rest it
//! reads and writes. The orchestration lives here so [`crate::app::dispatch`] stays a
//! table of one-line calls rather than carrying the whole graph.

use std::path::Path;

use crate::app::targets::{project_directory, target_for};
use crate::app::Ctx;
use crate::error::Diagnose;
use crate::error::Problem;
use crate::ports::docker::LogQuery;

mod aggregators;
mod applications;
mod baseline;
mod clients;
mod curating;
// What one service asking for what another fills comes to, by what each speaks.
mod connecting;
// Jellyfin's cross-origin allow-list, held to the front door's origin on every pass.
mod cors;
mod proxies;
// The decline service's own media-server key, minted for it alone.
mod decline;
mod fulfilment;
// The request gate's routes, and the key it holds for the media server.
mod gate;
// The Jellyfin keys minted for the services lemonfiber builds.
mod claiming;
mod guarding;
mod keys;
mod minted;
mod published;
pub(crate) use published::published_as;
mod subtitles;
mod taken_back;
// Seerr's Jellyfin connection, held at the request gate's Jellyfin route.
mod linking;
// The request gate's tokens, one per route, held raw by the request service alone.
pub(crate) mod tokens;
use fulfilment::{fulfilling, seed_fulfilment_targets};
pub(crate) mod identity;
mod reset;

use applications::{seed_applications, skipped};
use baseline::{escalate_broken_roots, wanted_roots, DATA_ROOT, SCHEMA_VERSION_FIELD};
use curating::{seed_curator, wanted_clients, CuratorSeeding};
// Reached by reconfiguration as well as by seeding: what the curators that file media
// are, and the record of what lemonfiber last wrote. One answer to each, rather than a
// second reader beside this one that could disagree with it.
pub(crate) use applications::resync_application;
pub(crate) use baseline::{load_baseline, save_baseline, Loaded};
use clients::{category_for, held, seed_passwords, Held};
pub(crate) use curating::curators;
pub(crate) use gate::reroute;
pub(crate) use guarding::exposure;
use identity::seed_request_identity;
pub(crate) use reset::reset_connections;
pub(crate) use subtitles::rewatch;

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
/// - what the household is told, and which curators the request service hands a request
///   to — both read as the owner, and the owner's session is a sign-in;
/// - whether a drifted download client still reaches anything, which the curator answers
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
/// One connection is unlike the rest: the torrent client's web UI password, the
/// credential lemonfiber mints rather than reads — its temporary password is
/// read from the container's log, replaced with a generated one, and the
/// generated one recorded where the forwarded-port push reads it. The rest of
/// the graph reads a credential and writes a connection: each curator's root
/// folders and its download clients, each curator registered into the indexer so
/// it pushes them indexers, the aggregator a book curator pulls from, the media
/// server as the request service's identity so the household signs in once, the
/// curators the request service hands requests to, and those the subtitle finder
/// watches.
pub(crate) async fn seed(ctx: &Ctx, adopt: bool) -> Result<crate::seed::Report, Box<Problem>> {
    let mut manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;

    let mut wirings = Vec::new();

    wirings.extend(withheld(&mut manifest.services, &ctx.settings.unmanaged));

    // What each of the stack's asks comes to, settled once for the whole pass and after
    // withholding. The connections below reach *whatever fills* what they ask for, a
    // plugin's service on the same terms as the stack's, which is what makes standing
    // in for a bundled service a change to the manifest and the setting rather than a
    // change here — and a record of what is installed that will not read refuses the
    // seed rather than letting it wire past a contest nobody could see.
    let register = crate::app::plugins::read(ctx)?;
    let (installed, kept_back) = withheld_brought(register.installed(), &ctx.settings.unmanaged);
    wirings.extend(kept_back);
    let chosen = crate::app::targets::chosen_fillers(ctx);
    let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
    let fillers = crate::wiring::Fillers::of(&manifest, &installed, &chosen, project.as_deref());

    // Each torrent client's password, the one credential lemonfiber mints. A later run
    // mints nothing — the password in force is the one already set — so the value
    // recorded on the run that minted it stands in. Without that a curator that came up
    // after the first seed would never learn about the client, since its password
    // cannot be read back from the client itself.
    let (minted, passwords) = seed_passwords(ctx, &fillers).await;
    wirings.extend(minted);
    let held = held(ctx, &fillers, &passwords).await;
    wirings.extend(clients::refused(&fillers, &held));

    // Root folders and download clients for each curator, and the
    // pairs that come to no connection at all, said rather than left out.
    wirings.extend(connecting::unmatched(&fillers));
    // The host data root, read once, so each curator's root folders can be checked
    // against the filesystem they file into and a folder pointing nowhere raised as a
    // warning.
    let data_root = crate::app::targets::data_root(ctx);
    let curating = curators(&fillers);
    // A root folder one curator wants and another does too is contested: two curators
    // on one folder would each rewrite the other's files, so it is refused rather
    // than wired. Detected across every curator up front, before any is wired.
    let root_claims: Vec<(&str, Vec<crate::ports::service::RootFolder>)> = curating
        .iter()
        .map(|curator| (curator.name(), wanted_roots(curator.media_types())))
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
    let (mut baseline, lost) = load_baseline(ctx).starting();
    // Each curator's wiring is independent of the others, so the curators are seeded at
    // once rather than in series: a pass's time then tracks the slowest curator, not
    // their sum. Each records what it wrote into its own baseline, read against the
    // loaded snapshot; the records are folded back into one below, and since a
    // field key carries the service, no two curators collide.
    let seeding = CuratorSeeding {
        contested: &contested,
        fillers: &fillers,
        held: &held,
        data_root: data_root.as_deref(),
        expected: &baseline,
        adopt,
    };
    let seeded = futures_util::future::join_all(
        curating
            .iter()
            .map(|curator| seed_curator(ctx, *curator, &seeding)),
    )
    .await;
    for (curator_wirings, records) in seeded {
        wirings.extend(curator_wirings);
        baseline.merge(&records);
    }

    // The indexer's app sync: register each curator it asks for back into it, so it
    // pushes them its indexers. A curator it has no application for is not left out:
    // it is among the pairs reported above as reached by nothing.
    wirings.extend(seed_applications(ctx, &fillers).await);

    // The book curator, which the aggregator cannot register itself into: it keeps its own
    // list of aggregators and pulls from them, so it is told where one is instead.
    wirings.extend(aggregators::seed_aggregators(ctx, &fillers).await);

    // The media server and everything that signs in to it as its administrator.
    wirings.extend(
        seed_media_server(
            ctx,
            &manifest.services,
            &fillers,
            project.as_deref(),
            &mut baseline,
        )
        .await,
    );

    // The curators the request service hands a request to, and the credentials it held
    // before the gate taken back.
    wirings.extend(
        seed_requests(
            ctx,
            &manifest.services,
            &fillers,
            project.as_deref(),
            &mut baseline,
        )
        .await,
    );

    // The keys the stack's own services read out of the environment. Two of them are
    // configured that way and by no other means — the quality sync and the archive
    // extractor — so without this they run with nothing, and the quality sync refuses
    // its whole configuration over a single undefined name.
    wirings.push(published::publish_keys(ctx, &manifest.services, project.as_deref(), &held).await);

    // The listening server's first account, which is anybody's until somebody makes it.
    wirings.extend(claiming::claimed(ctx, &manifest.services).await);

    // The Usenet indexer aggregator's authentication, without which it hands the indexer
    // accounts it holds to anything that can reach it.
    wirings.extend(guarding::guarded(ctx, &fillers, &mut baseline).await);

    // The subtitle finder, told which curators to watch. Until it is, it has nothing
    // to look at, and a household gets subtitles for nothing — which looks exactly
    // like releases that happen to have none.
    wirings.extend(subtitles::seed_subtitles(ctx, &fillers).await);

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

/// The same, for the services installed plugins brought: each one the operator declared
/// unmanaged is taken out of its plugin's record for the pass, and said.
///
/// A plugin's service is one this pass can write to — a torrent client's password is set
/// on the client itself — so the promise is the same one, and kept the same way.
fn withheld_brought(
    installed: &[crate::plugin::Installed],
    declared: &[(String, String)],
) -> (Vec<crate::plugin::Installed>, Vec<crate::seed::Wiring>) {
    let mut observed = Vec::new();
    let kept = installed
        .iter()
        .map(|one| {
            let mut one = one.clone();
            one.services.retain(|placed| {
                let Some(because) = crate::unmanaged::covering(declared, &placed.service) else {
                    return true;
                };
                observed.push(crate::seed::Wiring::settled(
                    placed.called().to_owned(),
                    crate::seed::State::Observed {
                        reason: because.to_owned(),
                    },
                ));
                false
            });
            one
        })
        .collect();
    (kept, observed)
}

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
        std::sync::Arc::new(crate::seerr::Seerr::new(
            ctx.seams.http.clone(),
            &base,
            "seerr",
        )) as std::sync::Arc<dyn crate::ports::service::Requests>
    });
    let recorded = match load_baseline(ctx) {
        Loaded::Formed(baseline) => baseline.entry("seerr", crate::seed::TELLING).cloned(),
        Loaded::Fresh | Loaded::Lost => None,
    };
    (seerr, recorded)
}

/// The download-client wirings lemonfiber manages, as a caller that only reads them needs
/// them: each curator, the clients lemonfiber would write there, and what it last recorded
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
pub(crate) async fn managed_wirings(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    installed: &[crate::plugin::Installed],
    project: Option<&Path>,
) -> Vec<crate::doctor::wiring::Managed> {
    let Loaded::Formed(baseline) = load_baseline(ctx) else {
        return Vec::new();
    };
    let (fillers, held) = reading(ctx, manifest, installed, project).await;
    let mut managed = Vec::new();
    for curator in curators(&fillers) {
        let clients = wanted_clients(curator, &fillers, &held)
            .into_iter()
            .map(|want| crate::doctor::wiring::Wired {
                recorded: baseline
                    .entry(curator.name(), &crate::seed::client_field(&want))
                    .cloned(),
                want,
            })
            .collect();
        managed.push(crate::doctor::wiring::Managed {
            id: curator.id().to_owned(),
            name: curator.name().to_owned(),
            reach: curator.reach(ctx).await,
            clients,
        });
    }
    managed
}

/// Who fills each ask and the credential each download client answers to, as a pass
/// that mints nothing reads them: every torrent client's password is the one recorded.
async fn reading(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    installed: &[crate::plugin::Installed],
    project: Option<&Path>,
) -> (crate::wiring::Fillers, Held) {
    let chosen = crate::app::targets::chosen_fillers(ctx);
    let fillers = crate::wiring::Fillers::of(manifest, installed, &chosen, project);
    let held = held(ctx, &fillers, &std::collections::BTreeMap::new()).await;
    (fillers, held)
}

/// The temporary password qBittorrent announced in its log, if it has.
async fn read_temporary_password(ctx: &Ctx, service: &str) -> Option<String> {
    let mut lines = ctx
        .seams
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

/// The curators the request service hands a request to. Without this the household can
/// ask and nothing downstream ever hears, and with it the request surface offers only
/// what the stack can actually deliver.
///
/// Which curators' keys the request service holds is noted first, because the move to the
/// gate is what hides it; once it reaches everything through the gate, those keys and
/// the Jellyfin key it minted itself are taken back.
async fn seed_requests(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    baseline: &mut crate::baseline::Baseline,
) -> Vec<crate::seed::Wiring> {
    taken_back::note_held(ctx, services, fillers, project, baseline).await;
    let mut wirings = seed_fulfilment_targets(ctx, services, fillers, project).await;
    wirings.extend(taken_back::seed_taken_back(ctx, services, fillers, project, baseline).await);
    wirings
}

/// How many lines back to read for qBittorrent's start-up announcement. Its
/// temporary password is printed once, early, so a generous tail finds it well
/// after start without pulling the whole log.
const TEMP_PASSWORD_LOG_LINES: u32 = 200;

/// The media server's administrator, and everything that signs in with it: who may
/// read the server from a browser, the decline service's key, the request gate's
/// routes, and the request service pointed at the server.
///
/// The media server is whatever fills the identity source the request service asks
/// for, the stack's or a plugin's, so a server standing in for the bundled one is set up
/// on the same terms with an administrator's password of its own.
async fn seed_media_server(
    ctx: &Ctx,
    services: &[lemonfiber_manifest::Service],
    fillers: &crate::wiring::Fillers,
    project: Option<&Path>,
    baseline: &mut crate::baseline::Baseline,
) -> Vec<crate::seed::Wiring> {
    // Boxed, because it is carried across every await of the passes below.
    let server = crate::app::targets::MediaServer::of(fillers).map(Box::new);
    let server = server.as_deref();

    // The media server as the request service's identity source: one household account,
    // not two. In two halves, because what the request service is pointed at may need
    // the first: the server has no key to read, so its administrator's password is
    // minted and recorded here, and the request gate's key below is minted with it.
    let mut wirings = Vec::new();
    let admin = identity::seed_media_server_admin(ctx, server).await;

    // Which origins a browser may read the media server from: the front door's alone.
    // After the identity's first half, because that is what records the administrator
    // credential this is written with.
    wirings.extend(cors::seed_cors(ctx, services, server).await);

    // The decline service's key, minted in the server the decline service names.
    wirings.extend(decline::seed_decline_key(ctx, services, project).await);

    // The request gate's routes, with the same session.
    wirings.extend(gate::seed_gate_routes(ctx, services, fillers, server, project).await);

    // The second half: the request service pointed at the media server, through the
    // gate where the stack runs one, whose routes the step above wrote.
    //
    // Ahead of everything that talks to the request service, because this is what
    // gives it an owner. A request service with none refuses the credential, and a
    // pass that asked it for anything first would report a fresh stack as broken and
    // then, in the same run, fix what it had just reported.
    let (identity_wirings, identity_records) =
        seed_request_identity(ctx, services, baseline, server, admin, project).await;
    baseline.merge(&identity_records);
    wirings.extend(identity_wirings);

    // Which address the media server believes about the client: the door's alone. Last,
    // because a change to it is read only when the server starts again, and everything
    // above talks to the server.
    wirings.extend(proxies::seed_proxies(ctx, project, server).await);
    wirings
}

#[cfg(test)]
mod tests;
