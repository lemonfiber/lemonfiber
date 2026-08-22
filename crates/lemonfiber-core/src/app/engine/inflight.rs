//! What is still coming down when the operator asks to stop.
//!
//! Stopping a form takes its download clients with it, and a torrent at ninety-four
//! per cent does not resume where it left off on every tracker. The operator almost
//! never knows: they are thinking about the form they named, and what is in flight
//! inside it is not visible from the command they typed.
//!
//! So a teardown asks first. What comes back is the clients' own account of what they
//! are working on — a list rather than a count, because "3 downloads active" and a
//! list naming them lead to different decisions, and the one thing an operator wants
//! to know is whether the thing they have been waiting for is among them.
//!
//! Three properties keep this from being worse than no guard at all.
//!
//! **Only the clients the plan would actually stop are asked.** A form holding no
//! download client makes no requests whatsoever, which is what keeps this off the
//! common path: `lemonfiber down tv` should not go to the network to discover it has
//! nothing to go to the network about.
//!
//! **A client that will not answer contributes nothing rather than an alarm.** It is
//! already being torn down, or was never up; either way the honest answer to "what is
//! in flight" from a client that will not say is silence, and blocking a teardown on a
//! service that is not there would be a guard that fires when it is least wanted.
//!
//! **A finished download is not something stopping can interrupt.** Clients keep
//! completed items in the same list they report active ones from, and naming those
//! would be warning about work that is already done.

use lemonfiber_manifest::Service;

use crate::app::targets::{download_targets, project_directory, protocol_of, read_transfers};
use crate::app::Ctx;
use crate::dashboard::Protocol;
use crate::ports::service::Download;
use crate::stack::closure::resolve;

/// One download a teardown would interrupt.
///
/// Carries the protocol as well as the name, because "still downloading" is only
/// half of what an operator needs — the other half is which client to go and look in,
/// and a stack running both has two places it could be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interrupted {
    /// Which client has it.
    pub protocol: Protocol,
    /// What it is, as the client names it.
    pub name: String,
    /// How far along, from zero to a hundred.
    pub progress: u8,
}

/// What the download clients inside these forms are still working on.
///
/// Empty where the forms hold no client, where the clients hold nothing, where none
/// of them will answer, and where the stack cannot be read at all — the cases a
/// teardown should proceed through without comment. Nothing here is worth failing a
/// stop over: this exists to tell an operator something they would want to know, and
/// a guard that cannot find out has nothing to tell them.
pub async fn in_flight(ctx: &Ctx, forms: &[String]) -> Vec<Interrupted> {
    let targets = {
        let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
            return Vec::new();
        };
        let Ok(plan) = resolve(&manifest, forms, ctx.settings.protocols) else {
            return Vec::new();
        };
        let profiles: Vec<String> = plan.profiles.into_iter().collect();
        let stopping: Vec<Service> = manifest
            .services
            .iter()
            .filter(|service| profiles.contains(&service.profile))
            .cloned()
            .collect();
        let project = project_directory(&ctx.stack, ctx.settings.stack_dir.as_deref());
        download_targets(&stopping, project.as_deref())
    };

    // Asked at once rather than one client after another, for the reason the
    // dashboard and the diagnosis both are: these are independent HTTP calls, and an
    // operator waiting to be told whether they can stop should wait for the slowest
    // client rather than for the sum of them. An empty list asks nothing at all.
    let read = futures_util::future::join_all(targets.iter().map(|target| async move {
        (protocol_of(&target.kind), read_transfers(ctx, target).await)
    }))
    .await;

    read.into_iter()
        .flat_map(|(protocol, downloads)| {
            downloads
                .into_iter()
                .filter(underway)
                .map(move |download| Interrupted {
                    protocol,
                    name: download.name,
                    progress: download.progress,
                })
        })
        .collect()
}

/// Whether a download is still coming down.
fn underway(download: &Download) -> bool {
    download.progress < 100
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Duration;

    use super::in_flight;
    use crate::app::Ctx;
    use crate::config::{Protocols, Settings};
    use crate::ports::http::Http;
    use crate::test_support::{a_context, a_password, env_at, nowhere, SeedFs};
    use lemonfiber_fixtures::downloads::{
        downloads, QBIT_FINISHED, QBIT_TORRENTS, SAB_EMPTY, SAB_KEY_INI, SAB_QUEUE,
    };
    use lemonfiber_fixtures::http::Fake;

    /// The forms an operator names, as the engine takes them.
    fn named(forms: &[&str]) -> Vec<String> {
        forms.iter().map(|form| (*form).to_owned()).collect()
    }

    /// A context that can reach both download clients: `SABnzbd`'s key on a fake
    /// filesystem, qBittorrent's password in a scratch env file, and both protocols
    /// in play so a plan holds both.
    fn reaching(http: Arc<dyn Http>, env_file: Option<PathBuf>) -> Ctx {
        let settings = Settings {
            protocols: Protocols::both(),
            env_file,
            ..Settings::default()
        };
        a_context()
            .settings(settings)
            .build()
            .waiting(Duration::ZERO)
            .with_filesystem(Arc::new(SeedFs::keyed(None, Some(SAB_KEY_INI))))
            .with_http(http)
    }

    /// The whole reason this is safe to put on the teardown path: a form with no
    /// download client in it does not go to the network to find that out.
    #[tokio::test]
    async fn a_form_holding_no_client_asks_nothing_at_all() {
        let fake = Fake::silent();
        let ctx = reaching(Arc::clone(&fake) as Arc<dyn Http>, None);

        let found = in_flight(&ctx, &named(&["search"])).await;

        assert!(found.is_empty());
        assert!(
            fake.requests().is_empty(),
            "a form with no download client made requests anyway"
        );
    }

    #[tokio::test]
    async fn both_clients_say_what_they_are_working_on() {
        let fake = downloads(QBIT_TORRENTS, SAB_QUEUE);
        let ctx = reaching(
            Arc::clone(&fake) as Arc<dyn Http>,
            Some(env_at("in-flight", &a_password())),
        );

        let found = in_flight(&ctx, &named(&["dl"])).await;

        let names: Vec<&str> = found
            .iter()
            .map(|download| download.name.as_str())
            .collect();
        assert!(names.contains(&"Ubuntu.iso"), "{names:?}");
        assert!(names.contains(&"Linux.nzb"), "{names:?}");
    }

    /// Naming a finished download would be warning about work stopping cannot undo.
    #[tokio::test]
    async fn a_download_that_has_finished_is_not_in_flight() {
        let fake = downloads(QBIT_FINISHED, SAB_EMPTY);
        let ctx = reaching(
            Arc::clone(&fake) as Arc<dyn Http>,
            Some(env_at("finished", &a_password())),
        );

        assert!(in_flight(&ctx, &named(&["dl"])).await.is_empty());
    }

    /// A client already on its way down cannot be asked, and a teardown blocked on
    /// one that is not there would fire exactly when it is least wanted.
    #[tokio::test]
    async fn a_client_that_will_not_answer_contributes_nothing() {
        let ctx = reaching(Fake::scripted(Vec::new()), None);

        assert!(in_flight(&ctx, &named(&["dl"])).await.is_empty());
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_finds_nothing() {
        let settings = Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        };
        let ctx = a_context()
            .over(nowhere())
            .settings(settings)
            .build()
            .with_http(Fake::silent());

        assert!(in_flight(&ctx, &named(&["dl"])).await.is_empty());
    }

    #[tokio::test]
    async fn a_form_the_stack_does_not_declare_finds_nothing() {
        let ctx = reaching(Fake::silent(), None);

        assert!(in_flight(&ctx, &named(&["no-such-form"])).await.is_empty());
    }
}
