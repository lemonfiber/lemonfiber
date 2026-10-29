//! Wiring the stack's services to each other, idempotently.
//!
//! Each connection reads a credential and writes it where another service needs it, and
//! the accounts a service starts without are made here. The orchestration lives here so
//! [`crate::app::dispatch`] stays a table of one-line calls rather than carrying the whole
//! graph.

use std::path::Path;

use crate::app::targets::project_directory;
use crate::app::Ctx;
use crate::error::Diagnose;
use crate::error::Problem;

mod aggregators;
mod applications;
mod baseline;
mod clients;
mod curating;
mod extraction;
mod filing;
// What one service asking for what another fills comes to, by what each speaks.
mod connecting;
// The media server's cross-origin allow-list, held to the front door's origin on every
// pass.
mod cors;
mod crossing;
mod proxies;
// The decline service's own media-server key, minted for it alone.
mod decline;
pub(crate) mod encrypting;
mod enveloping;
mod fulfilment;
// The request gate's routes, and the key it holds for the media server.
mod gate;
// The media server keys minted for the services lemonfiber builds.
mod keys;
mod minted;
pub(crate) mod owning;
mod published;
mod quality;
pub(crate) mod replacing;
mod showing;
mod subtitles;
mod taken_back;
// The request service's connection to the media server, held at the request gate's route
// to it.
mod linking;
// The request gate's tokens, one per route, held raw by the request service alone.
pub(crate) mod tokens;
use fulfilment::{fulfilling, seed_fulfilment_targets};
pub(crate) mod identity;
mod reset;
mod restarting;
mod withholding;

use applications::{seed_applications, skipped};
use baseline::{escalate_broken_roots, wanted_roots, DATA_ROOT, SCHEMA_VERSION_FIELD};
use curating::{contested, seed_curator, wanted_clients, CuratorSeeding};
// Reached by reconfiguration as well as by seeding: what the curators that file media
// are, and the record of what lemonfiber last wrote. One answer to each, rather than a
// second reader beside this one that could disagree with it.
pub(crate) use applications::resync_application;
pub(crate) use baseline::{load_baseline, save_baseline, Loaded};
use clients::{category_for, held, Held};
pub(crate) use curating::curators;
pub(crate) use enveloping::guarding_installed;
pub(crate) use gate::reroute;
use identity::seed_request_identity;
pub(crate) use quality::reprofile;
pub(crate) use reset::reset_connections;
pub(crate) use showing::reshow;
pub(crate) use subtitles::rewatch;
use withholding::{withheld, withheld_brought};

/// Wire the stack's services to each other, idempotently, and report what was
/// wired and what a re-run still owes — or, on a run that only says what it would do,
/// report the same pass with every write left out.
///
/// **A rehearsal issues nothing but reads, and that is the rule rather than a summary
/// of one.** It is stricter than "registers no connection", because several of the
/// things this pass would call reading are `POST`s: a sign-in opens a session on
/// somebody else's service, and two of the keys published at the end are read by being
/// minted. Each is state left behind by a run that promised to leave none, so none of
/// them is made.
///
/// Three things the rule costs, each reported as something this pass could not tell
/// rather than told wrong:
///
/// - what the household is told, and which curators the request service hands a request
///   to — both read as the owner, and the owner's session is a sign-in;
/// - whether a drifted download client still reaches anything, which the curator answers
///   only by being asked to test it. The drift is reported; what is left out is the
///   claim that it broke something.
///
/// And the keys the stack's own services read are named rather than gathered: the media
/// server mints its key when it is asked for one, and a question that gathered it would
/// have created the very thing it promised only to describe.
///
/// Everything else is the same walk — the same reads, the same three-way comparison,
/// the same words — with the registering and the minting not done.
///
/// The gate is held at each write rather than above the pass, because the pass is where
/// the report comes from: a rehearsal that stopped at the door would have nothing to
/// say, and one that surveyed separately would be a second opinion about what
/// lemonfiber intends — and the one nobody runs is the one that goes wrong.
///
/// The graph reads a credential and writes a connection: each curator's root
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
    let fillers = crate::wiring::Fillers::of(&manifest, &installed, &chosen, project.as_deref())
        .numbered(crate::app::targets::ordinals(ctx));

    let held = held(ctx, &fillers);

    // Root folders and download clients for each curator, and the
    // pairs that come to no connection at all, said rather than left out.
    wirings.extend(connecting::unmatched(&fillers));
    // The host data root, read once, so each curator's root folders can be checked
    // against the filesystem they file into and a folder pointing nowhere raised as a
    // warning.
    let data_root = crate::app::targets::data_root(ctx);
    let curating = curators(&fillers);
    let contested = contested(&curating);
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

    let (filed, completed) = filing::seed_completed_folders(ctx, &fillers, &mut baseline).await;
    wirings.extend(filed);
    wirings.extend(extraction::seed_extraction(ctx, &fillers, &completed).await);

    // The indexer's app sync: register each curator it asks for back into it, so it
    // pushes them its indexers. A curator it has no application for is not left out:
    // it is among the pairs reported above as reached by nothing.
    wirings.extend(seed_applications(ctx, &fillers).await);

    // A curator the aggregator cannot register itself into: it keeps its own list of
    // aggregators and pulls from them, so it is told where one is instead.
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

    // What the stream guard enforces and the live-stream paths the pass service confirms,
    // from the declaration the media server answers once its administrator is set up.
    wirings.extend(Box::pin(enveloping::seed_envelope(ctx, &fillers, project.as_deref())).await);

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

    published::retire_keys(ctx, &fillers).await;
    wirings.extend(crossing::unhanded(ctx, &manifest, &installed, &fillers));

    wirings.extend(owning::owned(ctx, &fillers, &mut baseline).await);

    // The subtitle finder, told which curators to watch. Until it is, it has nothing
    // to look at, and a household gets subtitles for nothing — which looks exactly
    // like releases that happen to have none.
    wirings.extend(subtitles::seed_subtitles(ctx, &fillers).await);
    wirings.extend(quality::seed_quality(ctx, &fillers).await);

    wirings.extend(
        showing::seed_dashboard(
            ctx,
            &manifest.services,
            &installed,
            &fillers,
            project.as_deref(),
        )
        .await,
    );

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
    })
}

/// The request service to ask about the household's telling, and what lemonfiber
/// last recorded setting it to, under the id of the service its adapter fronts.
///
/// Both or neither: a stack with no request service has nothing to ask, and a
/// baseline that was never formed leaves the recorded value absent — which the check
/// reads as nobody having set this rather than as a value to have drifted from.
pub(crate) async fn managed_telling(
    ctx: &Ctx,
    fillers: &crate::wiring::Fillers,
) -> (
    Option<std::sync::Arc<dyn crate::ports::service::Requests>>,
    Option<crate::baseline::Record>,
) {
    let Some(filler) = crate::app::targets::request_service(fillers) else {
        return (None, None);
    };
    let requests = crate::app::targets::requests_as_owner(ctx, filler)
        .await
        .map(|requests| requests as std::sync::Arc<dyn crate::ports::service::Requests>);
    let recorded = match load_baseline(ctx) {
        Loaded::Formed(baseline) => baseline
            .entry(
                &fillers.fronted_by(filler).unwrap_or(filler).id,
                crate::seed::TELLING,
            )
            .cloned(),
        Loaded::Fresh | Loaded::Lost => None,
    };
    (requests, recorded)
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
            asked: curator.client(ctx).await.map(|adapter| {
                std::sync::Arc::new(adapter) as std::sync::Arc<dyn crate::ports::service::Client>
            }),
            clients,
        });
    }
    managed
}

/// Who fills each ask and the credential each download client answers to.
async fn reading(
    ctx: &Ctx,
    manifest: &lemonfiber_manifest::Manifest,
    installed: &[crate::plugin::Installed],
    project: Option<&Path>,
) -> (crate::wiring::Fillers, Held) {
    let chosen = crate::app::targets::chosen_fillers(ctx);
    let fillers = crate::wiring::Fillers::of(manifest, installed, &chosen, project)
        .numbered(crate::app::targets::ordinals(ctx));
    let held = held(ctx, &fillers);
    (fillers, held)
}

/// The curators the request service hands a request to. Without this the household can
/// ask and nothing downstream ever hears, and with it the request surface offers only
/// what the stack can actually deliver.
///
/// Which curators' keys the request service holds is noted first, because the move to the
/// gate is what hides it; once it reaches everything through the gate, those keys and
/// the media server key it minted itself are taken back.
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
    wirings.extend(cors::seed_cors(ctx, services, fillers, server).await);
    wirings.extend(encrypting::seed_internal_tls(ctx, server).await);

    // The decline service's key, minted in the server the decline service names.
    wirings.extend(decline::seed_decline_key(ctx, services, fillers, project).await);

    // The request gate's routes, with the same session.
    wirings.extend(
        Box::pin(gate::seed_gate_routes(
            ctx, services, fillers, server, project,
        ))
        .await,
    );

    // A replacement of the media server's internal certificate taken as far as the holders
    // handed its SHA-256 just above let it go.
    let stack = replacing::Stack {
        services,
        fillers,
        project,
    };
    wirings.extend(Box::pin(replacing::advanced(ctx, stack, server)).await);

    // The second half: the request service pointed at the media server, through the
    // gate where the stack runs one, whose routes the step above wrote.
    //
    // Ahead of everything that talks to the request service, because this is what
    // gives it an owner. A request service with none refuses the credential, and a
    // pass that asked it for anything first would report a fresh stack as broken and
    // then, in the same run, fix what it had just reported.
    let (identity_wirings, identity_records) =
        seed_request_identity(ctx, services, fillers, baseline, server, admin, project).await;
    baseline.merge(&identity_records);
    wirings.extend(identity_wirings);

    // Which address the media server believes about the client: the door's alone. Last,
    // because a change to it is read only when the server starts again, and everything
    // above talks to the server.
    let door = ctx
        .stack
        .checked_manifest(ctx.today())
        .ok()
        .and_then(|manifest| crate::app::project::topology(ctx, &manifest).ok())
        .and_then(|topology| crate::screening::door::upstream(&topology));
    wirings.extend(proxies::seed_proxies(ctx, door, server).await);
    wirings
}

#[cfg(test)]
mod tests;
