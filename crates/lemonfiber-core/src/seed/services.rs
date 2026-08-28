//! The connections that are not download clients.
//!
//! Indexers pushed to the services that search them, a download client's own password,
//! and the media server made the identity source for requests — each a one-off shape
//! rather than a variation on wiring a client.

use super::drift::{reconcile, Observed};
use super::{
    observe_or_skip, same_base_url, unreached, wire_one, AppSync, Application, Journal,
    MediaServer, Naming, Qbittorrent, Random, Requests, State, Wiring, ADMIN,
};
use crate::baseline::Record;
use crate::ports::service::{FulfilmentTarget, RegisteredTarget, Telling};
use crate::secret;
use crate::seerr::OCCASIONS;

/// The field lemonfiber records what it set the household's telling to under.
pub(crate) const TELLING: &str = "notifications.household";

/// What lemonfiber would have the request service tell the household.
#[must_use]
pub(crate) const fn wanted_telling() -> Telling {
    Telling {
        enabled: true,
        occasions: OCCASIONS,
    }
}

/// A telling written down, so the three-way comparison has one shape to read.
///
/// The occasions are a set and the baseline holds strings, so the set is written out
/// rather than the number alone — a record that said only `222` would be a number
/// nobody reading the file could place.
#[must_use]
pub(crate) fn said(telling: Telling) -> String {
    let sending = if telling.enabled { "on" } else { "off" };
    format!("{sending}:{}", telling.occasions)
}

/// Make sure the request service will tell the household what became of what they
/// asked for, and say which way it was left.
///
/// **Its own step**, rather than part of pointing the service at the media server:
/// that one stops at a service already initialised, which is every install after the
/// first — exactly the ones this would otherwise never reach.
///
/// Its own connection in the report too, named for what it does rather than for the
/// agent it does it through: an operator reading the pass wants to know whether the
/// people in the house will hear back, not which of the service's notifiers carries
/// it. Hands back what the service holds as well as the state, because a value the
/// operator set before lemonfiber ever ran is theirs to adopt and the caller needs it
/// to write the baseline down.
pub async fn wire_household_telling(
    seerr: &dyn Requests,
    recorded: Option<&Record>,
) -> (Wiring, Telling) {
    let (state, held) = tell_the_household(seerr, recorded).await;
    (
        Wiring::settled("What the household is told".to_owned(), state),
        held,
    )
}

/// What lemonfiber sees for the telling, read from the three values.
///
/// Shared with the diagnosis that reads the same field without writing it, so the
/// two cannot come to different opinions about whose value is on the service — the
/// division `observe_client` makes for a download client, for the same reason.
///
/// A setting is always *there*, so there is no absent value the way an unregistered
/// download client is absent. The nearest thing is the service's untouched default
/// with nothing recorded against it: nobody has set this, lemonfiber included.
/// Without that, a service nobody has configured reads as the operator's own
/// pre-existing choice, and a diagnosis would tell them they had switched off
/// something they had never been offered. An operator who turned it off *after*
/// lemonfiber turned it on has a baseline, so that still reads as their edit.
#[must_use]
pub(crate) fn observed_telling(recorded: Option<&Record>, held: Telling) -> Observed {
    if recorded.is_none() && held == Telling::default() {
        Observed::Absent
    } else {
        reconcile(recorded, Some(said(held).as_str()), &said(wanted_telling()))
    }
}

/// The comparison and the write, apart from the reporting shape around them.
pub(crate) async fn tell_the_household(
    seerr: &dyn Requests,
    recorded: Option<&Record>,
) -> (State, Telling) {
    let held = match seerr.telling().await {
        Ok(held) => held,
        Err(failure) => return (unreached(&failure), Telling::default()),
    };
    let want = wanted_telling();
    let holding = said(held);
    let observed = observed_telling(recorded, held);

    let state = match observed {
        // `Unavailable` cannot arrive here — it is what a pass says about a service
        // that would not answer, and one that would not answer returned above with
        // its own words. Grouped the way the wiring check groups it, rather than
        // given an arm that nothing can reach.
        Observed::Absent | Observed::Unavailable => match seerr.tell(&want).await {
            Ok(()) => State::Wired,
            Err(failure) => unreached(&failure),
        },
        Observed::Present => State::AlreadyWired,
        // Theirs. Said, and no more than said — somebody who turned this off turned it
        // off, and a household that stopped being told is a thing to report rather
        // than a thing to correct.
        Observed::Drifted => State::Drifted,
        Observed::Stale => State::Stale,
        Observed::Conflicted => State::Conflicted {
            yours: Some(holding),
            ours: said(want),
        },
        Observed::Adopted => State::Adopted,
        Observed::Unmanaged => State::Unmanaged,
    };
    (state, held)
}

/// Hand the request service the \*arrs that fulfil what the household asks for.
///
/// Until it is told, the request service knows of no \*arr: a request is accepted
/// and no downloader ever hears about it. It does not discover them.
///
/// Only the \*arrs actually in the stack are offered, and that is the half worth
/// stating — the request service offers what its targets can deliver, so television
/// is not offered where Sonarr is not running. An \*arr that is absent is simply
/// never handed over; one the operator registered themselves is left exactly as it
/// is, never rewritten, the same way an application already present is.
pub async fn wire_fulfilment_targets(
    seerr: &dyn Requests,
    wanted: &[FulfilmentTarget],
    journal: &mut Journal,
    at: &str,
) -> Vec<Wiring> {
    let existing = match observe_or_skip(seerr.fulfilment_targets().await, wanted, describe_target)
    {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    for target in wanted {
        let state = if holds(&existing, target) {
            State::AlreadyWired
        } else {
            wire_one(
                seerr.add_fulfilment_target(target),
                seerr.fulfilment_targets(),
                |rows| holding(rows, target).map(|have| have.id.clone()),
                Naming {
                    service: "seerr",
                    resource: "fulfilment target",
                    noun: "request target",
                },
                journal,
                at,
            )
            .await
        };
        wirings.push(Wiring::settled(describe_target(target), state));
    }
    wirings
}

/// The one the request service holds at this target's endpoint, if it holds one.
///
/// By host, port and which list it is in — never by name, so an operator who
/// renamed it is not handed a second copy of the same service.
fn holding<'a>(
    held: &'a [RegisteredTarget],
    want: &FulfilmentTarget,
) -> Option<&'a RegisteredTarget> {
    held.iter().find(|have| {
        have.host == want.host && have.port == want.port && have.television == want.television
    })
}

/// Whether the request service already reaches this \*arr.
fn holds(held: &[RegisteredTarget], want: &FulfilmentTarget) -> bool {
    holding(held, want).is_some()
}

/// A fulfilment target's description for the report.
fn describe_target(target: &FulfilmentTarget) -> String {
    format!("{} as a request target", target.name)
}

/// Wire Prowlarr's applications: register the media-filing \*arrs it lacks, leave
/// the ones it already has, and record each write as a change.
///
/// The same shape as [`wire_root_folders`], matched by the address Prowlarr
/// reaches an \*arr on rather than by a label, so an application an operator
/// renamed is recognised as the same connection and not registered a second time.
/// An application already present is left exactly as it is and never rewritten,
/// which is what preserves an operator's own change to its sync settings.
pub async fn wire_applications(
    prowlarr: &dyn AppSync,
    service: &str,
    wanted: &[Application],
    journal: &mut Journal,
    at: &str,
) -> Vec<Wiring> {
    let existing = match observe_or_skip(prowlarr.applications().await, wanted, |application| {
        describe_application(service, application)
    }) {
        Ok(existing) => existing,
        Err(skipped) => return skipped,
    };

    let mut wirings = Vec::new();
    for application in wanted {
        let already = existing
            .iter()
            .any(|have| same_base_url(&have.base_url, &application.base_url));
        let state = if already {
            State::AlreadyWired
        } else {
            wire_one(
                prowlarr.register_application(application),
                prowlarr.applications(),
                |rows| {
                    rows.iter()
                        .find(|have| same_base_url(&have.base_url, &application.base_url))
                        .map(|have| have.id.clone())
                },
                Naming {
                    service,
                    resource: "application",
                    noun: "application",
                },
                journal,
                at,
            )
            .await
        };
        wirings.push(Wiring::settled(
            describe_application(service, application),
            state,
        ));
    }
    wirings
}

/// An application connection's description for the report.
pub(super) fn describe_application(service: &str, application: &Application) -> String {
    format!("{} indexer sync via {service}", application.name)
}

/// Replace qBittorrent's temporary web UI password with a generated one, and hand
/// the generated value back so the surface can record it where the forwarded-port
/// push reads it.
///
/// Unlike every other connection, this one is a credential lemonfiber mints
/// rather than reads. Generating it needs randomness the operating system might
/// withhold; without it there is nothing to set, and the connection fails rather
/// than falling back to a guessable secret on the client the forwarded port
/// authenticates to. The client sets the password and confirms it by
/// authenticating again; only a confirmed change is wired, and only then is the
/// value returned to record — an unset or unconfirmed one records nothing.
pub async fn wire_qbittorrent_password(
    client: &Qbittorrent,
    random: &dyn Random,
    temporary: &str,
) -> (Wiring, Option<String>) {
    let connection = "qBittorrent web UI password".to_owned();
    let Some(password) = secret::generate(random) else {
        return (
            Wiring::settled(
                connection,
                State::Failed {
                    detail: "no randomness was available to generate a password".to_owned(),
                },
            ),
            None,
        );
    };

    match client.replace_password(temporary, &password).await {
        Ok(()) => (Wiring::settled(connection, State::Wired), Some(password)),
        Err(failure) => (Wiring::settled(connection, unreached(&failure)), None),
    }
}

/// Make Jellyfin the identity source for Seerr: mint and set Jellyfin's admin
/// account where its wizard has not run, then point Seerr's authentication at it.
///
/// Two services in order. Jellyfin has no key to read, so — like qBittorrent —
/// its admin password is one lemonfiber mints, sets by driving the first-run
/// wizard, and hands back for the surface to record; a wizard already run by the
/// household leaves its password unknown, so the wiring is skipped rather than
/// reset. With the credential in hand, Seerr is signed in through Jellyfin, which
/// on a fresh Seerr also creates its owner. An already-initialised Seerr is never
/// re-pointed, since that would cost the household its existing sign-ins. The
/// minted password is returned to record whenever the account was created, even
/// if Seerr itself could not then be reached, because the account now holds it.
pub async fn wire_jellyfin_identity(
    jellyfin: &dyn MediaServer,
    seerr: &dyn Requests,
    random: &dyn Random,
    recorded_password: Option<&str>,
    server_url: &str,
) -> (Wiring, Option<String>) {
    let connection = "Jellyfin as Seerr's identity".to_owned();
    let (password, minted) = match jellyfin_admin(jellyfin, random, recorded_password).await {
        Ok(pair) => pair,
        Err(state) => return (Wiring::settled(connection, state), None),
    };
    let state = configure_seerr(seerr, &password, server_url).await;
    (Wiring::settled(connection, state), minted)
}

/// The Jellyfin admin credential, and the password to record if it was newly
/// minted: minted where the wizard has not run, read from what was recorded where
/// it has, and unknown — so the wiring cannot proceed — where the household ran
/// the wizard itself.
async fn jellyfin_admin(
    jellyfin: &dyn MediaServer,
    random: &dyn Random,
    recorded: Option<&str>,
) -> Result<(String, Option<String>), State> {
    let completed = match jellyfin.startup_completed().await {
        Ok(done) => done,
        Err(failure) => return Err(unreached(&failure)),
    };
    if completed {
        return match recorded {
            Some(password) => Ok((password.to_owned(), None)),
            None => Err(State::Skipped {
                reason: "Jellyfin was set up outside lemonfiber, so its admin password is unknown; a later run cannot complete this until it is set up through lemonfiber".to_owned(),
            }),
        };
    }
    let Some(password) = secret::generate(random) else {
        return Err(State::Failed {
            detail: "no randomness was available to generate a password".to_owned(),
        });
    };
    match jellyfin.create_admin(ADMIN, &password).await {
        Ok(()) => Ok((password.clone(), Some(password))),
        Err(failure) => Err(unreached(&failure)),
    }
}

/// Point Seerr at the media server, unless it is already initialised — which is
/// left untouched, whether lemonfiber initialised it on an earlier run or the
/// household set it up with accounts of its own. A fresh Seerr is signed in and
/// then read back: it must report itself initialised, or the write did not land.
async fn configure_seerr(seerr: &dyn Requests, password: &str, server_url: &str) -> State {
    let initialized = match seerr.initialized().await {
        Ok(done) => done,
        Err(failure) => return unreached(&failure),
    };
    if initialized {
        return State::AlreadyWired;
    }
    if let Err(failure) = seerr.configure_identity(ADMIN, password, server_url).await {
        return unreached(&failure);
    }
    match seerr.initialized().await {
        Ok(true) => State::Wired,
        Ok(false) => State::Failed {
            detail: "Seerr accepted the sign-in but did not report itself initialised".to_owned(),
        },
        Err(failure) => unreached(&failure),
    }
}

#[cfg(test)]
mod telling_tests {
    use super::{said, tell_the_household, wanted_telling, TELLING};
    use crate::baseline::Baseline;
    use crate::seed::drift::{intent, Intent, Observed};
    use crate::seed::State;
    use crate::seerr::Seerr;
    use lemonfiber_fixtures::http::{Answer, Fake};
    use std::sync::Arc;

    /// A telling that is on, but not for the occasions lemonfiber would choose.
    const SOME: &str = r#"{"enabled":true,"types":8}"#;

    /// The real client over a scripted service, so these read the request this
    /// product actually sends rather than a second description of it.
    fn service(answers: Vec<Answer>) -> (Seerr, Arc<Fake>) {
        let http = Fake::by_path_in_turn(vec![("/settings/notifications/webpush", answers)]);
        (Seerr::new(http.clone(), "http://seerr:5055", "seerr"), http)
    }

    /// A baseline holding one value for the telling.
    fn recorded(value: &str, adopted: bool) -> Baseline {
        let mut baseline = Baseline::new();
        if adopted {
            baseline.adopt("seerr", TELLING, value, "2026-08-28T00:00:00Z");
        } else {
            baseline.record("seerr", TELLING, value, "2026-08-28T00:00:00Z");
        }
        baseline
    }

    async fn against(seerr: &Seerr, baseline: &Baseline) -> State {
        tell_the_household(seerr, baseline.entry("seerr", TELLING))
            .await
            .0
    }

    /// Whether the service was asked to change anything.
    fn written_to(http: &Fake) -> bool {
        http.requests()
            .iter()
            .any(|asked| asked.method == crate::ports::http::Method::Post)
    }

    #[tokio::test]
    async fn a_service_holding_what_was_wanted_is_left_exactly_as_it_is() {
        let held = format!(
            r#"{{"enabled":true,"types":{}}}"#,
            wanted_telling().occasions
        );
        let (seerr, http) = service(vec![Answer::reply(200, held)]);

        let state = against(&seerr, &recorded(&said(wanted_telling()), false)).await;

        assert_eq!(state, State::AlreadyWired);
        assert!(!written_to(&http), "a correct value was written again");
    }

    #[tokio::test]
    async fn lemonfibers_own_value_behind_its_intent_is_reported_rather_than_rewritten() {
        // The baseline and the service agree; it is lemonfiber that has moved on.
        let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

        let state = against(&seerr, &recorded("on:8", false)).await;

        assert_eq!(state, State::Stale);
        assert!(!written_to(&http), "a value nobody edited was overwritten");
    }

    #[tokio::test]
    async fn both_sides_moved_is_put_to_the_operator_rather_than_settled() {
        // The baseline matches neither what the service holds nor what is wanted.
        let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

        let state = against(&seerr, &recorded("on:2", false)).await;

        assert!(
            matches!(&state, State::Conflicted { yours, ours }
                if yours.as_deref() == Some("on:8") && ours == &said(wanted_telling())),
            "{state:?}"
        );
        assert!(!written_to(&http), "a conflict was resolved by writing");
    }

    #[tokio::test]
    async fn a_value_the_operator_had_adopted_stays_theirs() {
        let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

        let state = against(&seerr, &recorded("on:8", true)).await;

        assert_eq!(state, State::Adopted);
        assert!(!written_to(&http));
    }

    #[tokio::test]
    async fn a_value_set_before_lemonfiber_ever_ran_is_taken_on_rather_than_flagged() {
        // Something is set, and lemonfiber never wrote it: theirs, pre-existing.
        let (seerr, http) = service(vec![Answer::reply(200, SOME)]);

        let state = against(&seerr, &Baseline::new()).await;

        assert_eq!(state, State::Unmanaged);
        assert!(!written_to(&http), "a pre-existing value was overwritten");
    }

    #[tokio::test]
    async fn a_service_that_will_not_answer_is_reported_rather_than_guessed_at() {
        let (seerr, http) = service(vec![Answer::Silent]);

        let state = against(&seerr, &Baseline::new()).await;

        assert!(matches!(state, State::Skipped { .. }), "{state:?}");
        assert!(
            !written_to(&http),
            "a service that would not answer was written to anyway"
        );
    }

    /// A write that does not land is reported, not assumed.
    ///
    /// The household hears nothing either way; the difference is whether the operator
    /// is told. A pass that reported `Wired` on a refused write would leave them
    /// believing the loop closes.
    #[tokio::test]
    async fn a_write_the_service_refuses_is_reported_in_its_own_words() {
        let (seerr, _) = service(vec![
            Answer::reply(200, r#"{"enabled":false,"types":0}"#),
            Answer::reply(500, "no"),
        ]);

        let state = against(&seerr, &Baseline::new()).await;

        assert!(
            matches!(state, State::Failed { .. } | State::Skipped { .. }),
            "a refused write was not reported: {state:?}"
        );
    }

    /// The states above are the shared policy, not a second opinion about it.
    ///
    /// This maps the observation straight to a state rather than going through
    /// `intent`, because one of `intent`'s outcomes cannot arise here and an arm
    /// nothing reaches is an arm nothing checks. The correspondence is asserted
    /// instead, so the two cannot drift apart in silence.
    #[test]
    fn every_state_here_is_the_one_the_shared_policy_asks_for() {
        let paired = [
            (Observed::Absent, Intent::Wire),
            (Observed::Present, Intent::Leave),
            (Observed::Drifted, Intent::Preserve),
            (Observed::Stale, Intent::Update),
            (Observed::Conflicted, Intent::Ask),
            (Observed::Adopted, Intent::Keep),
            (Observed::Unmanaged, Intent::Adopt),
        ];
        for (observed, expected) in paired {
            assert_eq!(
                intent(observed),
                expected,
                "the telling treats {observed:?} as {expected:?}, and the shared policy \
                 no longer agrees"
            );
        }
    }
}
