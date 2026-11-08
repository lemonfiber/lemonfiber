//! What each curator and each download client says of the queue, and what could not be
//! asked.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use lemonfiber_contract::capabilities::library::curate;

use crate::app::targets::{
    curator_targets, download_targets, fillers_here, DownloadTarget, Downloading as Client,
};
use crate::app::Ctx;
use crate::model::Unreached;
use crate::ports::media::Kind;
use crate::ports::service::{Download, Downloading, Failure};

/// Why a client's downloads are judged from their curators' view alone.
const UNNAMED: &str = "it names no download by its own id, so its downloads are judged \
                       from their curators' view alone and none is found orphaned";

/// One curator, as a pass asked it.
pub(super) struct Curator {
    /// Its id.
    pub(super) id: String,
    /// What it files.
    pub(super) kind: Kind,
    /// What it is asked through.
    pub(super) asked: Arc<dyn curate::Fills>,
    /// Every entry of its queue.
    pub(super) entries: Vec<Downloading>,
    /// Each of its titles' names, by its id.
    pub(super) titles: BTreeMap<i64, String>,
    /// Why nothing of its queue may be acted on, where something forbids it.
    pub(super) held_back: Option<String>,
}

/// One download client's item.
pub(super) struct Item {
    /// The client holding it, by its id.
    pub(super) client: String,
    /// What the client says of it.
    pub(super) download: Download,
}

/// What a pass found.
#[derive(Default)]
pub(super) struct Gathered {
    /// Every curator that answered its queue.
    pub(super) curators: Vec<Curator>,
    /// Every item a download client holds.
    pub(super) items: Vec<Item>,
    /// Every category a curator files under.
    pub(super) categories: BTreeSet<String>,
    /// Every curator or client that could not be reached or verified.
    pub(super) unreachable: Vec<Unreached>,
    /// Each category a client files outside the curator's folder, with what it says.
    pub(super) mismatched: BTreeMap<String, String>,
    /// Whether something could not be read, so the list may be short.
    pub(super) incomplete: bool,
}

impl Gathered {
    /// Every category a client files outside the curator's folder, said in one sentence;
    /// nothing where none is.
    pub(super) fn mismatch(&self) -> Option<String> {
        (!self.mismatched.is_empty()).then(|| {
            self.mismatched
                .values()
                .cloned()
                .collect::<Vec<_>>()
                .join("; ")
        })
    }
}

/// What every curator and download client on this machine says of the queue; nothing
/// where the stack cannot be read.
pub(super) async fn gathered(ctx: &Ctx) -> Gathered {
    let mut gathered = Gathered::default();
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        gathered.incomplete = true;
        return gathered;
    };
    let fillers = fillers_here(ctx, &manifest);
    for target in download_targets(ctx, &fillers).await {
        clients(&mut gathered, &target).await;
    }
    for target in curator_targets(ctx, &fillers).await {
        let (Some(kind), Some(asked)) = (target.kind, target.asked) else {
            continue;
        };
        gathered.categories.insert(kind.media_type().to_owned());
        curator(&mut gathered, target.id, kind, asked).await;
    }
    gathered
}

/// What `target` holds and where it files each category, into `gathered`.
async fn clients(gathered: &mut Gathered, target: &DownloadTarget) {
    let client = target.client();
    let transfers = match &client {
        Client::Torrent(client) => client.transfers().await,
        Client::Usenet(client) => client.transfers().await,
    };
    match transfers {
        Ok(downloads) => {
            if downloads.iter().any(|download| download.id.is_none()) {
                gathered.unreachable.push(Unreached {
                    service: target.service.clone(),
                    why: UNNAMED.to_owned(),
                });
            }
            gathered
                .items
                .extend(downloads.into_iter().map(|download| Item {
                    client: target.service.clone(),
                    download,
                }));
        }
        Err(failure) => unreached(gathered, &target.service, &failure),
    }
    if let Ok(filed) = client.filing().filed().await {
        for one in filed {
            let wanted = crate::seed::run::completed(&one.category);
            if one.folder != wanted {
                let said = format!(
                    "{} files {} under {}, outside {wanted}",
                    target.service, one.category, one.folder
                );
                gathered.mismatched.insert(one.category, said);
            }
        }
    }
}

/// What the curator `id` holds of `kind`, into `gathered`, held back where its download
/// clients do not answer it or one files outside its folder.
async fn curator(gathered: &mut Gathered, id: String, kind: Kind, asked: Arc<dyn curate::Fills>) {
    let held_back = match asked.test_download_clients().await {
        Ok(probes) => probes
            .iter()
            .find(|probe| !probe.reachable)
            .map(|probe| format!("its download client {} does not answer it", probe.id)),
        Err(failure) => Some(failure.to_string()),
    };
    if let Some(why) = &held_back {
        gathered.unreachable.push(Unreached {
            service: id.clone(),
            why: why.clone(),
        });
    }
    let entries = match asked.downloading(kind).await {
        Ok(entries) => entries,
        Err(failure) => {
            unreached(gathered, &id, &failure);
            return;
        }
    };
    let titles = asked
        .titles(kind)
        .await
        .map(|titles| {
            titles
                .into_iter()
                .filter_map(|title| Some((title.id?, title.title)))
                .collect()
        })
        .unwrap_or_default();
    let held_back = held_back.or_else(|| gathered.mismatched.get(kind.media_type()).cloned());
    gathered.curators.push(Curator {
        id,
        kind,
        asked,
        entries,
        titles,
        held_back,
    });
}

/// Record that `service` could not be asked, and why.
fn unreached(gathered: &mut Gathered, service: &str, failure: &Failure) {
    gathered.incomplete = true;
    gathered.unreachable.push(Unreached {
        service: service.to_owned(),
        why: failure.to_string(),
    });
}
