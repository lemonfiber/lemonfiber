//! Seeding one curator.
//!
//! Everything a single curator needs pointed at it, and the order it has to happen in.

use lemonfiber_contract::capabilities::library::curate;

use super::clients::Held;
use super::connecting::{pairings, Connection};
use super::{
    category_for, escalate_broken_roots, skipped, wanted_roots, Ctx, Path, DATA_ROOT,
    SCHEMA_VERSION_FIELD,
};
use crate::app::targets::{spoken, Spoken};
use crate::doctor::credentials::Reach;
use crate::ports::service::{Client, DownloadClient};
use crate::wiring::{Filler, Fillers};

/// A service that files media, as the core asks it.
#[derive(Clone, Copy)]
pub(crate) struct Curator<'a>(&'a Filler);

impl<'a> Curator<'a> {
    /// Its id.
    pub(crate) const fn id(self) -> &'a str {
        self.0.id.as_str()
    }

    /// Its name.
    pub(crate) const fn name(self) -> &'a str {
        self.0.name.as_str()
    }

    /// The media it files.
    pub(crate) const fn media_types(self) -> &'a [String] {
        self.0.media_types.as_slice()
    }

    /// How the curator is asked: over `library.curate` where it speaks the contract,
    /// otherwise as the bundled curator. Nothing where it speaks the contract and cannot
    /// be asked over it.
    pub(crate) async fn reach(self, ctx: &Ctx) -> Option<Reach> {
        match spoken(ctx, self.0, curate::CAPABILITY, curate::MAJOR).await {
            Spoken::Over(adapter) => Some(Reach::Over(adapter)),
            Spoken::Unanswered => None,
            Spoken::Not => self.0.target().map(Reach::Bundled),
        }
    }

    /// The curator as a client, or nothing where it cannot be asked or its key is not
    /// written yet.
    pub(crate) async fn client(self, ctx: &Ctx) -> Option<Box<dyn Client>> {
        self.reach(ctx)
            .await?
            .open(&ctx.seams.http, ctx.seams.filesystem.as_ref())
            .await
    }
}

/// Every service that files media and is asked over `library.curate` or as the bundled
/// curator.
pub(crate) fn curators(fillers: &Fillers) -> Vec<Curator<'_>> {
    fillers
        .services()
        .filter(|filler| {
            !filler.media_types.is_empty()
                && (filler.contracted(curate::CAPABILITY, curate::MAJOR)
                    || filler.target().is_some())
        })
        .map(Curator)
        .collect()
}

/// The inputs a seed pass reads once and hands to every curator it seeds: the
/// contested-root map across every curator, who fills each ask and the credential each
/// download client answers to, the host data root each root folder is checked against,
/// the loaded baseline to compare with, and whether this is an adopt pass. Grouped so
/// seeding one curator takes the pass and the curator rather than a long list that only
/// `curator` varies across.
pub(super) struct CuratorSeeding<'a> {
    /// Root-folder paths more than one curator wants — refused rather than wired.
    pub(super) contested: &'a std::collections::BTreeMap<String, Vec<String>>,
    /// Who fills each of the stack's asks, and where each is reached.
    pub(super) fillers: &'a Fillers,
    /// The credential each download client answers to, where it is in hand.
    pub(super) held: &'a Held,
    /// The host directory `/data` resolves to, for the root-folder existence check.
    pub(super) data_root: Option<&'a Path>,
    /// What lemonfiber last recorded — the expected leg of the drift comparison.
    pub(super) expected: &'a crate::baseline::Baseline,
    /// Whether this pass adopts each drifted value as the accepted baseline.
    pub(super) adopt: bool,
}

/// Register a curator's root folders, one per media type, under `/data/media`, and its
/// download clients beside them. Where it cannot be asked yet — it has not finished
/// starting — both are skipped for a re-run rather than failed.
pub(super) async fn seed_curator(
    ctx: &Ctx,
    curator: Curator<'_>,
    seeding: &CuratorSeeding<'_>,
) -> (Vec<crate::seed::Wiring>, crate::baseline::Baseline) {
    let wanted = wanted_roots(curator.media_types());
    let clients = wanted_clients(curator, seeding.fillers, seeding.held);
    // What this curator writes is recorded in its own baseline, against the loaded
    // snapshot, so several curators can be seeded at once without sharing one; the
    // caller folds them back into one afterwards.
    let mut records = crate::baseline::Baseline::new();

    // The curator is opened once, for both its root folders and its download clients.
    // One that cannot be asked yet has both skipped for a re-run and nothing recorded.
    let Some(client) = curator.client(ctx).await else {
        let mut wirings: Vec<_> = wanted
            .iter()
            .map(|folder| {
                skipped(
                    format!("{} root folder in {}", folder.media_type, curator.name()),
                    curator.name(),
                )
            })
            .collect();
        wirings.extend(clients.iter().map(|client| {
            skipped(
                format!("{} into {}", client.name, curator.name()),
                curator.name(),
            )
        }));
        return (wirings, records);
    };

    // The journal seed records each write into is not persisted: seeding is
    // idempotent, so a partial run is recovered by running it again, not reversed
    // — see the seed module doc.
    let at = ctx.stamp();
    let mut journal = crate::journal::Journal::new();

    // A schema change re-baselines rather than reporting mass drift. The service's own
    // version, read from its status, is compared with the one lemonfiber last recorded:
    // a change, taken with every download client drifted at once, is a service that
    // renamed its fields on upgrade — not the operator hand-editing each — so the
    // current shape is adopted as the new baseline instead of every field reported as
    // drift. A version change alone, with only some fields drifted, is left as the
    // genuine operator edits it is. The live version is recorded either way, so the
    // next run compares against what the service is now on.
    let live_version = client
        .identity()
        .await
        .ok()
        .map(|identity| identity.version);
    let version_changed = match (
        &live_version,
        seeding
            .expected
            .expected(curator.name(), SCHEMA_VERSION_FIELD),
    ) {
        (Some(live), Some(recorded)) => live != recorded,
        _ => false,
    };
    let re_baseline = version_changed
        && !clients.is_empty()
        && client.download_clients().await.is_ok_and(|existing| {
            crate::seed::wholesale_drift(&existing, &clients, seeding.expected, curator.name())
        });
    if let Some(live) = &live_version {
        records.record(curator.name(), SCHEMA_VERSION_FIELD, live, &at);
    }

    let mut wirings = crate::seed::wire_root_folders(
        client.as_ref(),
        curator.name(),
        &wanted,
        crate::seed::Placing {
            contested: seeding.contested,
            root: DATA_ROOT,
            backing: seeding.data_root.map(|data_root| crate::seed::Backing {
                filesystem: ctx.seams.filesystem.as_ref(),
                data_root,
            }),
        },
        &mut journal,
        &at,
        ctx.dry_run,
    )
    .await;
    // Before the download clients are appended, `wirings` holds exactly one entry per
    // wanted root folder in order, so each is escalated against the folder it reports
    // on: one the curator files into that resolves to nothing on the host is a root
    // folder pointing where nothing exists — a drift that breaks the stack.
    escalate_broken_roots(
        ctx.seams.filesystem.as_ref(),
        seeding.data_root,
        &wanted,
        &mut wirings,
    )
    .await;
    if !clients.is_empty() {
        wirings.extend(
            crate::seed::wire_download_clients(
                client.as_ref(),
                curator.name(),
                &clients,
                &mut journal,
                &mut crate::seed::Baselines {
                    expected: seeding.expected,
                    records: &mut records,
                    adopt: seeding.adopt || re_baseline,
                    reset: false,
                    rehearsing: ctx.dry_run,
                },
                &at,
            )
            .await,
        );
    }
    (wirings, records)
}

/// The download clients a curator is told about: each service filling one of its asks
/// that lemonfiber connects to it as a download client, at the address that service
/// declares, under the category the curator's first media type files as.
///
/// None where it manages no category. A filler whose credential is not in hand yet is
/// left out rather than told about with nothing to prove itself with, and a later run
/// that finds the credential tells the curator then.
pub(super) fn wanted_clients(
    curator: Curator<'_>,
    fillers: &Fillers,
    held: &Held,
) -> Vec<DownloadClient> {
    let Some(category) = curator
        .media_types()
        .first()
        .and_then(|media| category_for(media))
    else {
        return Vec::new();
    };
    let mut wanted = Vec::new();
    for pairing in pairings(fillers) {
        if pairing.asker != curator.0 {
            continue;
        }
        let Ok((Connection::DownloadClient(protocol), at, _)) = &pairing.made else {
            continue;
        };
        let Some(credential) = held.of(&super::clients::Holder::of(pairing.filler)) else {
            continue;
        };
        wanted.push(DownloadClient {
            name: pairing.filler.name.clone(),
            host: at.host.clone(),
            port: at.port,
            protocol: protocol.clone(),
            credential: credential.clone(),
            category: category.clone(),
        });
    }
    wanted
}
