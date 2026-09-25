use lemonfiber_fixtures::http::{Answer as Replies, Fake};

use super::{bandwidth, counting, fetch, possible, settled, wanted, zone, Asked, Fetch};
use crate::bandwidth::capacity::Source;
use crate::bandwidth::{
    Answer, Cap, Capacity, Declared, Held, Holding, Limit, Pulling, Reached, Respite, Restraint,
    Rhythm, WhenExceeded, NOTHING_MEASURED, NOTHING_TO_LIMIT, NO_ZONE, UNREADABLE,
};
use crate::config::Settings;
use crate::ports::service::Rates;
use crate::test_support::{a_context, a_password, env_at};

/// A moment every case here reads against.
const NOW: u64 = 1_790_812_800;

/// A stack whose torrent client answers about its limits, keeping its own files
/// in a directory of this run's own.
///
/// Every caller passes a different name: the record and the environment file
/// are written to a real disk and these cases run at the same time, so two
/// sharing a directory would be one wiping the other's while it read it.
fn a_stack(scratch: &str, tz: Option<&str>) -> crate::app::Ctx {
    stacked(scratch, tz, a_transport())
}

/// The same stack, over a transport the case can read its requests back from.
fn stacked(scratch: &str, tz: Option<&str>, http: std::sync::Arc<Fake>) -> crate::app::Ctx {
    let env = env_at(&format!("bandwidth-{scratch}"), &a_password());
    if let Some(zone) = tz {
        assert!(crate::config::store::set(&env, "TZ", zone).is_ok());
    }
    a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// What the torrent client answers every question this command asks it.
fn a_transport() -> std::sync::Arc<Fake> {
    Fake::by_path(vec![
        ("/api/v2/auth/login", Replies::reply(200, "Ok.")),
        (
            "/api/v2/app/setPreferences",
            Replies::reply(200, String::new()),
        ),
        (
            "/api/v2/app/preferences",
            Replies::reply(
                200,
                r#"{"dl_limit":0,"up_limit":0,"alt_dl_limit":0,
                        "alt_up_limit":0,"scheduler_enabled":false,
                        "add_stopped_enabled":true}"#,
            ),
        ),
        ("/api/v2/torrents/info", Replies::reply(200, "[]")),
        ("/api/v2/torrents/stop", Replies::reply(200, String::new())),
        ("/api/v2/torrents/start", Replies::reply(200, String::new())),
        ("/api/v2/transfer/speedLimitsMode", Replies::reply(200, "0")),
        (
            "/api/v2/transfer/info",
            Replies::reply(
                200,
                r#"{"dl_info_speed":20971520,"up_info_speed":2097152,
                    "dl_info_data":900,"up_info_data":80}"#,
            ),
        ),
    ])
}

/// A request naming one thing.
fn asking(field: impl FnOnce(&mut Asked)) -> Asked {
    let mut asked = Asked::default();
    field(&mut asked);
    asked
}

/// Ten megabytes down, one up.
fn a_line() -> Capacity {
    Capacity {
        down: 10 * 1024 * 1024,
        up: 1024 * 1024,
        source: Source::Observed,
        taken: NOW,
        through_tunnel: false,
    }
}

/// A client reporting these figures in both directions.
fn client(down: Held, up: Held) -> Holding {
    Holding {
        client: "qbittorrent".to_owned(),
        answer: Answer::Held {
            down,
            up,
            period: None,
        },
        pulling: None,
    }
}

#[test]
fn the_quiet_hours_are_unlimited_by_construction_rather_than_by_setting() {
    // The whole point of declaring a household's day is to have the line back
    // when nobody is using it. A stack that stayed throttled overnight is one
    // somebody switches the limits off on.
    let declared = Declared {
        down: Some(Limit::Share(50)),
        up: Some(Limit::Share(25)),
        rhythm: Rhythm::read("07:00-23:00"),
        capacity: Some(a_line()),
        ..Declared::default()
    };
    let told = wanted(&declared, false, None);
    assert_eq!(told.active.down, Some(5 * 1024 * 1024));
    assert_eq!(told.active.up, Some(256 * 1024));
    assert_eq!(told.quiet, Rates::default());
    assert!(told.window.is_some_and(|window| window.from_hour == 7
        && window.to_hour == 23
        && window.to_minute == 0));
}

/// A cap of a hundred bytes with this chosen at it, which the fake client's
/// nine hundred and eighty moved bytes are well past.
fn spent(exceeded: WhenExceeded) -> Declared {
    Declared {
        down: Some(Limit::Share(50)),
        up: Some(Limit::Share(25)),
        rhythm: Rhythm::read("07:00-23:00"),
        capacity: Some(a_line()),
        cap: Some(Cap {
            monthly: 100,
            exceeded,
        }),
        ..Declared::default()
    }
}

#[test]
fn a_spent_cap_that_chose_a_crawl_hands_the_clients_the_crawl_around_the_clock() {
    // The month is over whatever hour it is, so there is no window left to
    // switch on: a household's quiet hours are not extra allowance.
    let told = wanted(
        &spent(WhenExceeded::Throttle),
        false,
        Some(Reached::Exceeded),
    );
    assert_eq!(told.active.down, Some(crate::bandwidth::CRAWL));
    assert_eq!(told.quiet.down, Some(crate::bandwidth::CRAWL));
    assert!(told.window.is_none());
}

#[test]
fn a_spent_cap_that_chose_to_pause_leaves_the_declared_rate_where_it_was() {
    // Stopping is not a rate, and writing one in its name is how `pause` comes
    // to mean a very slow download. What the clients are told about speed is
    // exactly what was declared; the stopping is a second request.
    let told = wanted(&spent(WhenExceeded::Pause), false, Some(Reached::Exceeded));
    assert_eq!(told.active.down, Some(5 * 1024 * 1024));
    assert!(told.window.is_some());
}

#[test]
fn a_spent_cap_outranks_an_override_that_would_lift_the_limits() {
    // The same order the headline is decided in: the cap is the one with a
    // bill behind it, and an hour's amnesty is not a thing to grant a month
    // that is already over.
    let told = wanted(
        &spent(WhenExceeded::Throttle),
        true,
        Some(Reached::Exceeded),
    );
    assert_eq!(told.active.down, Some(crate::bandwidth::CRAWL));

    let inside = wanted(&spent(WhenExceeded::Throttle), true, Some(Reached::Within));
    assert_eq!(
        inside.active,
        Rates::default(),
        "an override still lifts them"
    );
}

#[test]
fn stopping_never_consults_the_record_and_starting_always_does() {
    // A client somebody started by hand in the middle of a spent month is
    // stopped again; a client lemonfiber never stopped is one an operator
    // stopped for reasons of their own, and is left exactly as they left it.
    assert_eq!(fetch(Some(WhenExceeded::Pause), true, true), Fetch::Stop);
    assert_eq!(fetch(Some(WhenExceeded::Pause), false, true), Fetch::Stop);
    assert_eq!(
        fetch(Some(WhenExceeded::Throttle), true, true),
        Fetch::Resume
    );
    assert_eq!(fetch(None, true, true), Fetch::Resume);
    assert_eq!(fetch(None, false, true), Fetch::Ask);
    assert_eq!(
        fetch(Some(WhenExceeded::Pause), false, false),
        Fetch::Ask,
        "a run that writes nothing asks and no more"
    );
}

#[test]
fn only_a_run_that_told_the_clients_something_records_what_it_told_them() {
    // A rehearsal that wrote this down would leave the next run starting
    // clients nothing had ever stopped.
    let stopped = settled(
        Declared::default(),
        &[],
        NOW,
        false,
        Some(WhenExceeded::Pause),
        true,
    );
    assert!(stopped.stopped);
    assert!(!settled(stopped.clone(), &[], NOW, false, None, true).stopped);
    assert!(
        settled(stopped, &[], NOW, false, None, false).stopped,
        "and a run that wrote nothing leaves the record as it found it"
    );
}

#[test]
fn a_respite_lifts_the_limits_rather_than_changing_them() {
    let declared = Declared {
        down: Some(Limit::Share(50)),
        capacity: Some(a_line()),
        rhythm: Rhythm::read("07:00-23:00"),
        ..Declared::default()
    };
    let told = wanted(&declared, true, None);
    assert_eq!(told.active, Rates::default());
    assert_eq!(told.quiet, Rates::default());
    assert!(
        told.window.is_none(),
        "and there is nothing for a schedule to switch between"
    );
}

/// One download client this stack could reach, for the refusals that come after
/// the one about having none.
fn a_client() -> Vec<super::reaching::Client> {
    vec![super::reaching::Client::Torrent(Box::new(
        crate::qbittorrent::Qbittorrent::new(Fake::silent(), "http://127.0.0.1:8081"),
    ))]
}

#[test]
fn a_share_of_a_line_nothing_measured_is_refused_before_anything_is_written() {
    // Refused rather than written, because a share of an unknown number holds
    // nothing back — a setting the operator believes is in force while the
    // stack takes the whole line.
    let declared = Declared {
        down: Some(Limit::Share(50)),
        ..Declared::default()
    };
    assert!(possible(&declared, &a_client(), None)
        .is_err_and(|problem| problem.code == NOTHING_MEASURED));
    assert!(possible(&declared, &[], None).is_err_and(|problem| problem.code == NOTHING_TO_LIMIT));
}

#[test]
fn an_absolute_limit_needs_nothing_measured() {
    let declared = Declared {
        up: Some(Limit::Absolute(1_000)),
        ..Declared::default()
    };
    assert!(possible(&declared, &a_client(), None).is_ok());

    // And a share of the *upload* is weighed against the uplink, so it is
    // refused on the same terms rather than riding on the download's figure.
    let shared = Declared {
        up: Some(Limit::Share(25)),
        ..Declared::default()
    };
    assert!(
        possible(&shared, &a_client(), None).is_err_and(|problem| problem.code == NOTHING_MEASURED)
    );
}

#[test]
fn what_this_run_saw_the_line_do_is_folded_into_what_is_known_about_it() {
    // A client that was moving with nothing holding it back has just measured
    // the line, at no cost and disturbing nobody.
    let unrestrained = client(
        Held::of(None, None, Some(20 * 1024 * 1024), true),
        Held::of(None, None, Some(2 * 1024 * 1024), true),
    );
    let settled = settled(Declared::default(), &[unrestrained], NOW, true, None, false);
    assert!(settled
        .capacity
        .is_some_and(|line| line.down == 20 * 1024 * 1024
            && line.up == 2 * 1024 * 1024
            && line.through_tunnel
            && line.taken == NOW));
}

#[test]
fn a_rate_measured_under_a_limit_is_a_measurement_of_the_limit() {
    // So it says nothing about the line, and must not be recorded as though
    // it did — a stack throttled to a tenth would otherwise talk itself down
    // to a tenth of its own connection.
    let held = client(
        Held::of(Some(1_000), Some(1_000), Some(1_000), true),
        Held::of(Some(100), Some(100), Some(100), true),
    );
    assert!(
        settled(Declared::default(), &[held], NOW, false, None, false)
            .capacity
            .is_none()
    );
}

#[test]
fn a_respite_that_ran_out_is_cleared_by_the_run_that_reports_it() {
    let declared = Declared {
        respite: Some(Respite { until: NOW - 1 }),
        ..Declared::default()
    };
    assert!(settled(declared, &[], NOW, false, None, false)
        .respite
        .is_none());

    let running = Declared {
        respite: Some(Respite { until: NOW + 1 }),
        ..Declared::default()
    };
    assert!(settled(running, &[], NOW, false, None, false)
        .respite
        .is_some());
}

#[tokio::test]
async fn a_stack_with_no_cap_is_not_asked_what_it_has_moved() {
    // Traffic spent on a figure nothing would do anything with.
    let ctx = a_context().build();
    assert!(counting(&ctx, &[], &Declared::default()).await.is_none());
}

#[tokio::test]
async fn a_cap_with_no_client_answering_has_no_figure_rather_than_a_zero() {
    // A month reported as untouched on a stack whose clients would not answer
    // is the one reading that would let a cap be passed in silence.
    let ctx = a_context().build();
    assert!(counting(&ctx, &[], &a_cap()).await.is_none());
}

/// A cap, so the clients are asked what they have moved at all.
fn a_cap() -> Declared {
    Declared {
        cap: Some(Cap {
            monthly: 100,
            exceeded: WhenExceeded::Pause,
        }),
        ..Declared::default()
    }
}

/// The torrent client, answering what it has moved from a transport of its
/// own so the client beside it can be given a different one.
fn a_client_that_says_what_it_moved() -> super::reaching::Client {
    super::reaching::Client::Torrent(Box::new(crate::qbittorrent::Qbittorrent::authenticated(
        Fake::by_path(vec![
            ("/api/v2/auth/login", Replies::reply(200, "Ok.")),
            (
                "/api/v2/transfer/info",
                Replies::reply(
                    200,
                    r#"{"dl_info_speed":0,"up_info_speed":0,
                            "dl_info_data":900,"up_info_data":80}"#,
                ),
            ),
        ]),
        "http://127.0.0.1:8081",
        a_password(),
    )))
}

/// The Usenet client on the same stack, which is not there.
fn a_client_that_will_not_say() -> super::reaching::Client {
    super::reaching::Client::Usenet(Box::new(crate::sabnzbd::Sabnzbd::new(
        Fake::silent(),
        "http://127.0.0.1:8080",
        "the-key",
    )))
}

#[tokio::test]
async fn a_client_that_will_not_say_what_it_has_moved_is_named_rather_than_counted_as_nothing() {
    // Taken as a zero it would be a month reading emptier than it is, which is
    // how a cap gets passed in silence. So the figure is what answered, and
    // the operator is told whose traffic it is short by rather than left to
    // work out why the number looks low.
    let ctx = a_context().build();
    let month = counting(
        &ctx,
        &[
            a_client_that_says_what_it_moved(),
            a_client_that_will_not_say(),
        ],
        &a_cap(),
    )
    .await;
    assert!(
        month.is_some_and(|month| month.moved() == 980
            && month.incomplete.iter().any(|missing| missing
                .contains("sabnzbd would not say what it has moved")
                && missing.contains("none of it is in this figure"))),
        "the figure is the client that answered, and the one that did not is named"
    );
}

#[test]
fn a_schedule_needs_the_stack_to_say_which_clock_the_clients_keep() {
    // The hours are kept by the clients themselves, on their own clocks. On a
    // stack that names no zone those clocks are UTC, and quiet hours would
    // start at the wrong time of night — so the schedule is refused rather
    // than applied to the wrong hours.
    let declared = Declared {
        rhythm: Rhythm::read("07:00-23:00"),
        ..Declared::default()
    };
    assert!(possible(&declared, &a_client(), None).is_err_and(|problem| problem.code == NO_ZONE));
    assert!(possible(&declared, &a_client(), Some("Europe/Amsterdam")).is_ok());
}

#[test]
fn a_stack_that_names_no_zone_reads_as_naming_none() {
    assert_eq!(zone(&a_context().build()), None);
    assert_eq!(zone(&a_stack("blank", Some("   "))), None);
}

#[test]
fn the_zone_the_stack_names_is_the_one_the_clients_keep() {
    let ctx = a_stack("zone", Some("Europe/Amsterdam"));
    assert_eq!(zone(&ctx).as_deref(), Some("Europe/Amsterdam"));
}

#[test]
fn a_request_that_asks_for_nothing_is_a_reading_rather_than_a_change() {
    assert!(!Asked::default().anything());
    assert!(Asked {
        down: Some("50%".to_owned()),
        ..Asked::default()
    }
    .anything());
}

#[tokio::test]
async fn a_run_that_asks_for_nothing_reads_the_line_and_writes_no_limit() {
    // The client is asked what it is limited to and what it is moving, and
    // nothing is put to it — which is what makes this half a read.
    let shared = bandwidth(&a_stack("reading", None), &Asked::default()).await;
    assert!(
        shared.is_ok_and(|shared| shared.restraint == Restraint::Unlimited
            && !shared.applied
            && shared.clients.len() == 1
            && shared
                .capacity
                .is_some_and(|line| line.down == 20 * 1024 * 1024)),
        "and what an unrestrained client achieved is what the line was seen to carry"
    );
}

#[tokio::test]
async fn a_run_that_declares_a_limit_puts_it_to_the_client_and_reads_it_back() {
    let ctx = a_stack("declaring", Some("Europe/Amsterdam"));
    // The line first, so a share has a figure to be a share of.
    assert!(bandwidth(
        &ctx,
        &asking(|asked| asked.line = Some("60MiB/6MiB".to_owned()))
    )
    .await
    .is_ok());

    let shared = bandwidth(&ctx, &asking(|asked| asked.down = Some("50%".to_owned()))).await;
    assert!(
        shared.is_ok_and(|shared| {
            shared.applied
                && shared.down.says.contains("50%")
                // The fake answers with no limit whatever it is told, which is
                // exactly the client this half exists to catch.
                && shared.clients.iter().any(Holding::worth_saying)
        }),
        "a client that took the write and did not apply it is not a client that did"
    );
}

#[tokio::test]
async fn a_rehearsal_says_what_it_would_declare_and_keeps_nothing() {
    let ctx = a_stack("rehearsing", None).rehearsing();
    let shared = bandwidth(&ctx, &asking(|asked| asked.down = Some("2MiB".to_owned()))).await;
    assert!(shared.is_ok_and(|shared| !shared.applied));
    // Nothing was kept, so the next run starts from nothing declared.
    let after = bandwidth(&ctx, &Asked::default()).await;
    assert!(after.is_ok_and(|after| after.down.limit == Limit::Unlimited));
}

#[tokio::test]
async fn a_share_of_a_line_nothing_measured_is_refused_before_the_client_is_told() {
    let refused = bandwidth(
        &a_stack("unmeasured", None),
        &asking(|asked| asked.down = Some("50%".to_owned())),
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code == NOTHING_MEASURED));
}

#[tokio::test]
async fn a_schedule_on_a_stack_that_names_no_zone_is_refused_rather_than_applied() {
    // Quiet hours in UTC on a household that lives somewhere else is the wrong
    // part of the night, which is exactly the failure the requirement names.
    let refused = bandwidth(
        &a_stack("zoneless", None),
        &asking(|asked| asked.active = Some("07:00-23:00".to_owned())),
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code == NO_ZONE));
}

#[tokio::test]
async fn a_word_this_does_not_read_never_reaches_a_client() {
    let refused = bandwidth(
        &a_stack("unreadable", None),
        &asking(|asked| asked.down = Some("half of it".to_owned())),
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code == UNREADABLE));
}

#[tokio::test]
async fn a_stack_with_no_download_client_has_nothing_to_hold_to_a_limit() {
    let refused = bandwidth(
        &a_context().build(),
        &asking(|asked| asked.down = Some("2MiB".to_owned())),
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code == NOTHING_TO_LIMIT));
}

#[tokio::test]
async fn a_client_that_will_not_answer_is_a_line_of_its_own_rather_than_an_absence() {
    // An unknown limit rendered as no limit is a report reading better than the
    // stack is.
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env_at("bandwidth-silent", &a_password())),
            ..Settings::default()
        })
        .build()
        .with_http(Fake::silent());
    let shared = bandwidth(&ctx, &Asked::default()).await;
    assert!(shared.is_ok_and(|shared| shared
        .clients
        .iter()
        .all(|client| matches!(client.answer, Answer::Silent { .. }))));
}

#[tokio::test]
async fn a_cap_is_counted_against_what_the_clients_say_they_have_moved() {
    let ctx = a_stack("capped", None);
    let declaring = Asked {
        cap: Some("512".to_owned()),
        exceeded: Some("pause".to_owned()),
        ..Asked::default()
    };
    assert!(bandwidth(&ctx, &declaring).await.is_ok());

    let shared = bandwidth(&ctx, &Asked::default()).await;
    assert!(
        shared.is_ok_and(|shared| shared.restraint == Restraint::CapExceeded
            && shared
                .metered
                .is_some_and(|month| month.moved() == 980 && !month.incomplete.is_empty())),
        "and what the count is short by is said rather than left to be assumed"
    );
}

#[tokio::test]
async fn an_override_lifts_the_limits_and_cannot_be_asked_for_beyond_an_evening() {
    let ctx = a_stack("respite", None);
    assert!(
        bandwidth(&ctx, &asking(|asked| asked.unrestricted_for = Some(1)))
            .await
            .is_ok_and(|shared| shared.restraint == Restraint::Overridden),
        "and the clients are told nothing at all while it runs"
    );
    // Time-boxed by construction: the length that would still be running
    // tomorrow morning is refused rather than recorded.
    let refused = bandwidth(
        &ctx,
        &asking(|asked| asked.unrestricted_for = Some(24 * 60)),
    )
    .await;
    assert!(refused.is_err_and(|problem| problem.code == UNREADABLE));
}

#[tokio::test]
async fn a_spent_cap_is_acted_on_by_a_run_that_asked_for_nothing() {
    // The whole of what declaring a cap buys. The choice was made in the calm;
    // the month runs out at two in the morning, and the run that next reads the
    // line is the one that carries it out — rather than reporting the month as
    // over and handing the clients the declared limits anyway.
    let ctx = a_stack("spent", None);
    assert!(bandwidth(
        &ctx,
        &asking(|asked| {
            asked.cap = Some("100".to_owned());
            asked.exceeded = Some("pause".to_owned());
        })
    )
    .await
    .is_ok());

    let shared = bandwidth(&ctx, &Asked::default()).await;
    assert!(
        shared.is_ok_and(|shared| shared.restraint == Restraint::CapExceeded
            && shared.applied
            && shared
                .acting
                .is_some_and(|said| said.contains("nothing new is fetched"))
            && shared
                .clients
                .iter()
                .all(|client| client.pulling == Some(Pulling::Stopped))),
        "a run that asked for nothing found the month over and stopped the clients"
    );
}

#[tokio::test]
async fn what_lemonfiber_stopped_is_what_it_lets_fetch_again() {
    let transport = a_transport();
    let ctx = stacked("cap-lifted", None, transport.clone());
    assert!(bandwidth(
        &ctx,
        &asking(|asked| {
            asked.cap = Some("100".to_owned());
            asked.exceeded = Some("pause".to_owned());
        })
    )
    .await
    .is_ok());
    assert!(transport.asked_for("/torrents/stop"));

    // The allowance is raised, so the month is no longer over and what was
    // stopped for that reason is let go again.
    let shared = bandwidth(&ctx, &asking(|asked| asked.cap = Some("1TiB".to_owned()))).await;
    assert!(shared
        .is_ok_and(|shared| shared.reached == Some(Reached::Within) && shared.acting.is_none()));
    assert!(transport.asked_for("/torrents/start"));
}

#[tokio::test]
async fn a_stack_with_no_cap_is_never_asked_whether_it_is_fetching() {
    // Traffic spent on a figure nothing would act on, and a question that
    // would put a word in the report nothing on this stack could ever change.
    let transport = a_transport();
    let ctx = stacked("uncapped", None, transport.clone());
    let shared = bandwidth(&ctx, &asking(|asked| asked.down = Some("2MiB".to_owned()))).await;
    assert!(shared.is_ok_and(|shared| shared.clients.iter().all(|client| client.pulling.is_none())));
    assert!(!transport.asked_for("/torrents/stop"));
    assert!(!transport.asked_for("filter=running"));
}

#[test]
fn every_refusal_this_command_makes_is_about_the_request() {
    for problem in [
        super::nothing_to_limit(),
        super::nothing_measured(),
        super::no_zone(),
    ] {
        let code = problem.code.as_str();
        assert!(code.starts_with("RATE-"), "{code}");
        assert!(!problem.remedies.is_empty(), "{code} says what to do");
    }
    assert_eq!(super::nothing_measured().code, NOTHING_MEASURED);
    assert_eq!(super::no_zone().code, NO_ZONE);
}
