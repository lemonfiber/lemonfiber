//! Adding one thing, end to end, with the operator watching.
//!
//! Setup leaves a machine where sixteen services are green and nothing has been proved.
//! This proves it: pick something, search for it, grab it, download it, import it, and see
//! it in the library — narrating each step so that afterwards the operator understands
//! what their stack does, because they watched it do it once.
//!
//! Every link it touches is a link that can be broken, and a broken one shows here, at the
//! one moment the operator is engaged and willing to fix things, rather than as a
//! mysterious absence three days later. That is why the failure paths carry as much care
//! as the happy one: failure is the useful case.
//!
//! The running is split by concern — whether to offer at all, what to walk, asking for it,
//! waiting on it, and what to say at the end — because each of those is a different set of
//! services and a different set of ways to go wrong.

#[cfg(test)]
mod fixtures;

mod acquire;
mod choose;
mod library;
mod offer;
mod settle;
mod walk;
mod watch;

use super::targets::open_servarrs;
use super::Ctx;
use crate::error::{Diagnose, Problem};
use crate::model::WalkthroughReport;
use crate::walkthrough::{Narrator, Shape, Why};

pub(super) use walk::Walk;

/// Walk one thing through the whole pipeline, saying what happens as it happens.
///
/// `term` is what the operator asked for, or nothing — in which case something safe is
/// suggested, because a first attempt that fails on an obscure choice teaches the wrong
/// lesson entirely.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack itself cannot be read. Everything else — no
/// indexers, nothing found, a tunnel that is down, an import that would not run — is a
/// walkthrough that stopped, which is a report rather than an error: the operator needs
/// the narration up to the stop as much as they need the stop.
pub async fn walkthrough(
    ctx: &Ctx,
    term: Option<&str>,
    narrator: &dyn Narrator,
) -> Result<WalkthroughReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let arrs = open_servarrs(ctx, &manifest.services).await;
    let mut walk = Walk::new(ctx, narrator);

    match offer::offered(ctx, &arrs).await {
        // Asked for by a stack that cannot search: not a walk that failed, but the one
        // thing missing before there could be one, said with what to do about it.
        Why::Not(reason) => Ok(walk.stopped(Shape::Pipeline, None, reason)),
        Why::Offer(Shape::LibraryOnly) => {
            Ok(library::walk(&mut walk, &manifest.services, term).await)
        }
        Why::Offer(Shape::Pipeline) => pipeline(&mut walk, &arrs, &manifest.services, term).await,
    }
}

/// The full walk: choose, ask for it, wait on it, and see it land.
async fn pipeline(
    walk: &mut Walk<'_>,
    arrs: &[super::targets::OpenArr],
    services: &[lemonfiber_manifest::Service],
    term: Option<&str>,
) -> Result<WalkthroughReport, Box<Problem>> {
    let chosen = match choose::choose(walk, arrs, term).await {
        Ok(chosen) => chosen,
        Err(choose::NotChosen::Stopped(reason)) => {
            return Ok(walk.stopped(Shape::Pipeline, None, reason))
        }
        Err(choose::NotChosen::AlreadyHere(title)) => return Ok(walk.already_here(&title, arrs)),
    };

    let item = match acquire::acquire(walk, &chosen).await {
        Ok(item) => item,
        Err(reason) => return Ok(walk.stopped(Shape::Pipeline, Some(chosen.named), reason)),
    };

    match watch::watch(walk, &chosen, &item).await {
        watch::Landed::Imported => Ok(settle::settle(walk, services, &chosen).await),
        // Left running rather than abandoned: the operator gets their terminal back and
        // the download keeps going, which is the promise the narration just made them.
        watch::Landed::StillGoing => Ok(walk.handed_off(&chosen.named)),
        watch::Landed::Stopped(reason) => {
            let logs = watch::what_was_said(walk.ctx, services, &chosen.named).await;
            Ok(walk.stopped_quoting(Shape::Pipeline, Some(chosen.named.clone()), reason, logs))
        }
    }
}

/// Whether a walkthrough is worth offering this stack — asked by setup, which offers it,
/// as well as by the walk itself.
///
/// Setup needs the answer before it offers rather than after, because offering a walk that
/// must stop at its first step is worse than not offering one.
///
/// # Errors
///
/// Returns a [`Problem`] where the stack itself cannot be read.
pub async fn worth_offering(ctx: &Ctx) -> Result<Why, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let arrs = open_servarrs(ctx, &manifest.services).await;
    Ok(offer::offered(ctx, &arrs).await)
}

#[cfg(test)]
mod tests {
    use super::fixtures::{
        ctx_library_only, ctx_through_a_tunnel, ctx_watching, ctx_with, ctx_with_torrents, Fake,
        Recording, ADDED, A_RELEASE, HAS_ITEM, HELD, IMPORTED, NOT_HELD, NO_ITEMS, ONE_INDEXER,
        ONE_WANTED, SIGNED_IN,
    };
    use super::{walkthrough, worth_offering};
    use crate::walkthrough::{Reason, Shape, State, Step};

    /// The whole walk, run against a stack answering as `fake` says.
    async fn walked(
        ctx: &crate::app::Ctx,
        term: Option<&str>,
    ) -> (
        crate::model::WalkthroughReport,
        Vec<crate::walkthrough::Line>,
    ) {
        let heard = Recording::default();
        // Built before the call, not in a fallback closure: these fixtures always produce
        // a report, so a lazily-built stand-in would be a line nothing ever runs.
        let blank = blank();
        let report = walkthrough(ctx, term, &heard).await.unwrap_or(blank);
        (report, heard.lines())
    }

    /// A report standing in for a stack that could not be read at all — which none of
    /// these fixtures produce, and which every assertion below would fail against.
    fn blank() -> crate::model::WalkthroughReport {
        crate::model::WalkthroughReport {
            shape: Shape::Pipeline,
            state: State::Abandoned,
            proves: String::new(),
            item: None,
            lines: Vec::new(),
            stopped: None,
            link: None,
            handover: None,
            suggestions: Vec::new(),
            in_background: false,
            already_here: false,
        }
    }

    #[test]
    fn no_two_stacks_built_here_keep_their_credential_in_one_file() {
        // The guard on the fixture: two stacks sharing one file have one rewritten
        // under the other, and a walk reading the media server's credential mid-rewrite
        // finds none and stops on a stack that has one. Which walk loses depends on what
        // else the suite is running, so nothing catches it except this.
        let one = ctx_watching(&Fake::default());
        let two = ctx_library_only(&Fake::default());
        assert!(
            one.settings.env_file.is_some(),
            "the credential is recorded somewhere"
        );
        assert_ne!(one.settings.env_file, two.settings.env_file);
    }

    #[tokio::test]
    async fn a_walk_that_works_ends_playable_and_points_somewhere() {
        // The whole point: something the operator asked for goes all the way through and
        // they are left with somewhere to go rather than a green dashboard.
        let ctx = ctx_watching(&Fake::default());
        let (report, said) = walked(&ctx, Some("Sintel")).await;

        assert_eq!(report.state, State::Complete);
        assert_eq!(report.shape, Shape::Pipeline);
        assert_eq!(report.item.as_deref(), Some("Sintel (2010)"));
        assert!(report.stopped.is_none(), "{:?}", report.stopped);
        assert!(report.handover.is_some(), "it points somewhere next");
        assert!(
            said.iter().any(|line| line.step == Step::Available),
            "the operator was told it worked: {said:?}"
        );
    }

    #[tokio::test]
    async fn a_proved_tunnel_lets_the_walk_go_on() {
        // The gate stops a torrent stack that cannot prove its tunnel; one that can is
        // walked like any other.
        let ctx = ctx_through_a_tunnel(&Fake::default());
        let (report, said) = walked(&ctx, Some("Sintel")).await;
        assert_ne!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::TunnelDown),
            "the tunnel held"
        );
        assert!(
            said.iter().any(|line| line.step >= Step::Grabbing),
            "it got past the gate: {said:?}"
        );
    }

    #[tokio::test]
    async fn the_narration_and_the_report_are_the_same_run() {
        // A walkthrough that narrated one thing and reported another would be two
        // accounts of one event, and nothing would say which was true.
        let ctx = ctx_watching(&Fake::default());
        let (report, said) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(report.lines, said);
        assert!(!said.is_empty());
    }

    #[tokio::test]
    async fn something_already_here_is_detected_rather_than_fetched_again() {
        let ctx = ctx_with(&Fake {
            lookup: HELD,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;

        assert!(report.already_here);
        assert_eq!(report.state, State::Complete);
        assert!(
            !report.suggestions.is_empty(),
            "and something else is offered instead"
        );
        assert!(
            report
                .suggestions
                .iter()
                .all(|said| !said.contains("Sintel")),
            "not the thing they already have: {:?}",
            report.suggestions
        );
    }

    #[tokio::test]
    async fn a_stack_with_nothing_to_search_is_pointed_at_the_prerequisite() {
        // Offering a walk that must stop at its first step is the product demonstrating
        // it does not know its own state.
        let ctx = ctx_with(&Fake {
            indexers: "[]",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;

        assert_eq!(report.state, State::Failed);
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NoIndexers)
        );
    }

    #[tokio::test]
    async fn an_indexer_that_is_switched_off_is_not_an_indexer() {
        let ctx = ctx_with(&Fake {
            indexers: r#"[{"enableAutomaticSearch":false,"enableInteractiveSearch":false}]"#,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, None).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NoIndexers)
        );
    }

    #[tokio::test]
    async fn indexers_that_answered_with_nothing_are_not_indexers_that_failed() {
        // The single most-confused pair in the product, and the acceptance criterion that
        // says so.
        let ctx = ctx_with(&Fake {
            releases: "[]",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        let stopped = report.stopped.map(|stopped| stopped.reason);
        assert_eq!(stopped, Some(Reason::NothingMatched));
        assert!(
            stopped.is_some_and(|reason| !reason.is_a_fault()),
            "nothing here is broken"
        );
    }

    #[tokio::test]
    async fn releases_that_the_preset_refuses_are_their_own_answer() {
        let ctx = ctx_with(&Fake {
            releases: r#"[{"rejections":["quality"]}]"#,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NoneMetThePreset)
        );
    }

    #[tokio::test]
    async fn a_catalogue_that_knows_nothing_by_that_name_says_so() {
        let ctx = ctx_with(&Fake {
            lookup: "[]",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NothingMatched)
        );
    }

    #[tokio::test]
    async fn a_catalogue_that_will_not_answer_is_a_different_problem_again() {
        let ctx = ctx_with(&Fake {
            refuses: "lookup",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::IndexersFailed)
        );
    }

    #[tokio::test]
    async fn a_result_with_no_identifier_cannot_be_asked_for() {
        let ctx = ctx_with(&Fake {
            lookup: r#"[{"id":0,"title":"Sintel"}]"#,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NothingMatched)
        );
    }

    #[tokio::test]
    async fn nothing_is_grabbed_for_a_torrent_stack_whose_tunnel_is_not_proved() {
        // The one gate that exists to prevent an action rather than report one. A
        // tutorial is never worth a torrent outside the tunnel.
        let ctx = ctx_with_torrents(&Fake::default());
        let (report, said) = walked(&ctx, Some("Sintel")).await;

        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::TunnelDown)
        );
        assert!(
            said.iter().all(|line| line.step < Step::Grabbing),
            "it stopped before grabbing: {said:?}"
        );
    }

    #[tokio::test]
    async fn a_service_that_will_not_take_it_on_is_reported_as_not_grabbed() {
        let ctx = ctx_with(&Fake {
            refuses: "/rootfolder",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NotGrabbed)
        );
    }

    #[tokio::test]
    async fn a_stack_with_no_root_folder_yet_is_a_stack_that_was_never_finished() {
        let ctx = ctx_with(&Fake {
            folders: "[]",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NotGrabbed)
        );
    }

    #[tokio::test]
    async fn a_service_that_refuses_to_take_it_on_is_reported_as_not_grabbed() {
        let ctx = ctx_with(&Fake {
            refuses_writes: true,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NotGrabbed)
        );
    }

    #[tokio::test]
    async fn indexers_that_could_not_be_searched_at_all_are_told_apart_from_empty_ones() {
        // The release search is where the two are distinguished, and it is the request
        // that fails when the indexers are unreachable.
        let ctx = ctx_with(&Fake {
            refuses: "/wanted",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        let stopped = report.stopped.map(|stopped| stopped.reason);
        assert_eq!(stopped, Some(Reason::IndexersFailed));
        assert!(
            stopped.is_some_and(Reason::is_a_fault),
            "this one is broken"
        );
    }

    #[tokio::test]
    async fn a_download_that_outlives_the_wait_is_left_running() {
        // Nothing is cancelled by the operator walking away, and the report says which of
        // the two endings this is.
        let ctx = ctx_with(&Fake {
            history: r#"{"records":[{"eventType":"grabbed","date":"2026-08-08T00:00:00Z"}]}"#,
            queue: r#"{"records":[{"seriesId":7,"movieId":7,"trackedDownloadState":"downloading","trackedDownloadStatus":"ok"}],"totalRecords":1}"#,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;

        assert!(report.in_background);
        assert_eq!(report.state, State::Downloading);
        assert!(
            !report.state.is_a_problem(),
            "still coming is not a failure"
        );
    }

    #[tokio::test]
    async fn a_download_that_never_started_is_a_diagnosis_rather_than_a_handoff() {
        let ctx = ctx_with(&Fake {
            history: "{}",
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NotGrabbed)
        );
    }

    #[tokio::test]
    async fn a_stuck_download_stops_the_walk_with_what_the_services_were_saying() {
        let ctx = ctx_with(&Fake {
            history: r#"{"records":[{"eventType":"grabbed","date":"2026-08-08T00:00:00Z"}]}"#,
            queue: r#"{"records":[{"seriesId":7,"movieId":7,"trackedDownloadState":"downloading","trackedDownloadStatus":"warning"}],"totalRecords":1}"#,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::Stalled)
        );
    }

    #[tokio::test]
    async fn imported_with_no_media_server_to_play_it_from_is_said_plainly() {
        // Not a broken pipeline — a form that does not include a media server.
        let ctx = ctx_with(&Fake::default());
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NoMediaServer)
        );
    }

    #[tokio::test]
    async fn imported_and_still_not_in_the_library_is_its_own_answer() {
        let ctx = ctx_watching(&Fake {
            library: NO_ITEMS,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NotVisible)
        );
    }

    #[tokio::test]
    async fn a_library_only_household_is_asked_whether_its_own_media_is_visible() {
        let ctx = ctx_library_only(&Fake::default());
        let (report, said) = walked(&ctx, None).await;

        assert_eq!(report.shape, Shape::LibraryOnly);
        assert_eq!(report.state, State::Complete);
        assert!(report.link.is_none(), "nothing was imported to link");
        assert!(
            said.iter().any(|line| line.step == Step::Scanning),
            "it told the library to look: {said:?}"
        );
    }

    #[tokio::test]
    async fn a_library_only_household_whose_media_is_not_there_is_told() {
        let ctx = ctx_library_only(&Fake {
            library: NO_ITEMS,
            ..Fake::default()
        });
        let (report, _) = walked(&ctx, Some("Sintel")).await;
        assert_eq!(report.shape, Shape::LibraryOnly);
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NotVisible)
        );
    }

    #[tokio::test]
    async fn a_library_only_stack_with_no_media_server_has_nothing_to_walk() {
        let mut ctx = ctx_with(&Fake::default());
        ctx.settings.protocols = super::fixtures::acquires_nothing().protocols;
        let (report, _) = walked(&ctx, None).await;
        assert_eq!(report.shape, Shape::LibraryOnly);
        assert_eq!(
            report.stopped.map(|stopped| stopped.reason),
            Some(Reason::NoMediaServer)
        );
    }

    #[tokio::test]
    async fn nothing_named_is_answered_with_something_safe() {
        // An operator with an empty library has no way to know what their indexers carry,
        // so a first attempt is chosen for them rather than guessed by them.
        let ctx = ctx_watching(&Fake::default());
        let (report, said) = walked(&ctx, None).await;
        assert!(report.item.is_some());
        assert!(
            said.first().is_some_and(|line| line.step == Step::Choosing),
            "the choosing is narrated: {said:?}"
        );
    }

    #[tokio::test]
    async fn what_a_stack_is_offered_is_answerable_before_anything_is_promised() {
        // Setup asks this before it offers, so it never puts a question it cannot honour.
        let offered = worth_offering(&ctx_with(&Fake::default())).await;
        assert_eq!(
            offered.ok().and_then(crate::walkthrough::Why::shape),
            Some(Shape::Pipeline)
        );

        let bare = worth_offering(&ctx_with(&Fake {
            indexers: "[]",
            ..Fake::default()
        }))
        .await;
        assert_eq!(bare.ok().and_then(crate::walkthrough::Why::shape), None);
    }

    #[tokio::test]
    async fn a_walk_asked_for_as_a_command_says_its_steps_where_the_context_says() {
        // The whole of what a surface has to supply: a walk dispatched like every
        // other command narrates to whoever the context is listening with, so a
        // browser hears the steps a terminal would have printed.
        let heard = std::sync::Arc::new(Recording::default());
        let ctx = ctx_watching(&Fake::default())
            .narrating_steps(heard.clone() as std::sync::Arc<dyn crate::walkthrough::Narrator>);
        let outcome = crate::app::dispatch(
            crate::app::Command::Walkthrough {
                item: Some("Sintel".to_owned()),
            },
            &ctx,
        )
        .await;

        // The same report the walk comes to when it is called directly, so
        // dispatching changes only where the steps are said.
        let (expected, _) = walked(&ctx_watching(&Fake::default()), Some("Sintel")).await;
        // Every line of it reached the narrator the context carries — the run's whole
        // account rather than one early step, which a walk that stopped anywhere after
        // it would satisfy just as well.
        assert_eq!(heard.lines(), expected.lines);
        assert!(
            expected
                .lines
                .iter()
                .any(|line| line.step == Step::Available),
            "it got all the way through: {:?}",
            expected.lines
        );
        assert_eq!(
            outcome.ok(),
            Some(crate::app::Outcome::Walkthrough(expected))
        );
    }

    #[tokio::test]
    async fn a_walk_nobody_is_watching_is_the_same_walk_as_one_somebody_is() {
        // The default narrator says nothing, and saying nothing changes nothing:
        // whether anyone is listening is the surface's business and never the
        // walk's, so the two runs come to the same report.
        let watched = walked(&ctx_watching(&Fake::default()), Some("Sintel"))
            .await
            .0;
        let alone = crate::app::dispatch(
            crate::app::Command::Walkthrough {
                item: Some("Sintel".to_owned()),
            },
            &ctx_watching(&Fake::default()),
        )
        .await;

        assert_eq!(
            alone.ok(),
            Some(crate::app::Outcome::Walkthrough(watched)),
            "the report is the same whether or not anybody heard it"
        );
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_at_all_is_a_problem_rather_than_a_report() {
        // Everything else is a walk that stopped; only the stack itself failing to read
        // is an error, because there is nothing to narrate at all.
        let mut ctx = ctx_with(&Fake::default());
        ctx.stack = crate::stack::Source::External(std::path::Path::new("/not-a-stack"));
        let heard = Recording::default();
        assert!(walkthrough(&ctx, None, &heard).await.is_err());
        assert!(worth_offering(&ctx).await.is_err());
        assert!(heard.lines().is_empty(), "nothing was said about nothing");
    }

    #[test]
    fn the_fixtures_are_the_shapes_the_services_actually_send() {
        // Guards the fixtures themselves: a test proving something about the wrong shape
        // proves nothing at all.
        for body in [
            NOT_HELD,
            HELD,
            ADDED,
            ONE_WANTED,
            A_RELEASE,
            ONE_INDEXER,
            IMPORTED,
            SIGNED_IN,
            HAS_ITEM,
            NO_ITEMS,
        ] {
            assert!(
                serde_json::from_str::<serde_json::Value>(body).is_ok(),
                "{body}"
            );
        }
    }
}
