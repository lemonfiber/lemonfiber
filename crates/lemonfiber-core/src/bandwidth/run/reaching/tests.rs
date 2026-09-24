use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer as Replies, Fake};

use super::{answer, holding, opened, said, DownloadKind, DownloadTarget, Fetch};
use crate::bandwidth::{Answer, Held, Period, Pulling, Verdict};
use crate::config::Settings;
use crate::ports::service::{Failure, Hours, Rates, Throttled, Wanted};
use crate::test_support::{a_context, a_password, env_at, SeedFs};

/// A `SABnzbd` configuration with a key in it, as the client writes one.
const KEYED: &str = "[misc]\napi_key = the-key\n";

/// The three parts of an answer, where the client answered at all.
fn parts(answer: &Answer) -> Option<(Held, Held, Option<Period>)> {
    match answer {
        Answer::Held { down, up, period } => Some((*down, *up, *period)),
        Answer::Silent { .. } => None,
    }
}

/// A megabyte a second, and a quarter of one.
const FAST: u64 = 1024 * 1024;
const SLOW: u64 = 256 * 1024;

/// Limits that hold the stack back while the house is awake and not after.
fn wanted() -> Wanted {
    Wanted {
        active: Rates {
            down: Some(SLOW),
            up: Some(SLOW),
        },
        quiet: Rates::default(),
        window: None,
    }
}

/// A client answering with these limits, on this side of the day.
fn held(rates: Rates, uploads: bool, hours: Option<Hours>) -> Throttled {
    Throttled {
        rates,
        uploads,
        hours,
    }
}

#[test]
fn a_client_in_its_quiet_hours_is_judged_against_the_quiet_figure() {
    // Judging it against the active one would report every well-behaved
    // stack as ignoring its limits every night.
    let answered = parts(&answer(
        &wanted(),
        &held(Rates::default(), true, Some(Hours::Quiet)),
        &Rates {
            down: Some(FAST),
            up: Some(FAST),
        },
    ));
    assert!(
        answered
            .is_some_and(|(down, _, period)| down.verdict == Verdict::Unasked
                && period == Some(Period::Quiet)),
        "the quiet hours ask nothing of it"
    );
}

#[test]
fn a_client_inside_the_household_s_hours_is_judged_against_the_active_figure() {
    let answered = parts(&answer(
        &wanted(),
        &held(
            Rates {
                down: Some(SLOW),
                up: Some(SLOW),
            },
            true,
            Some(Hours::Active),
        ),
        &Rates {
            down: Some(SLOW / 2),
            up: Some(0),
        },
    ));
    assert!(
        answered.is_some_and(|(down, up, period)| down.verdict == Verdict::Holding
            && up.verdict == Verdict::Holding
            && period == Some(Period::Active)),
        "both directions are inside the figure they were given"
    );
}

#[test]
fn a_client_with_no_schedule_of_its_own_is_judged_against_the_active_figure() {
    // It is held to the active rates around the clock, so that is the figure
    // it is answerable for.
    let answered = parts(&answer(
        &wanted(),
        &held(
            Rates {
                down: Some(SLOW),
                up: None,
            },
            false,
            None,
        ),
        &Rates {
            down: Some(0),
            up: None,
        },
    ));
    assert!(
        answered.is_some_and(|(down, up, period)| down.verdict == Verdict::Holding
            && up.verdict == Verdict::NothingToLimit
            && period.is_none()),
        "Usenet has no upload to have refused one, and keeps no hours"
    );
}

/// The torrent client as a read target.
fn torrent() -> DownloadTarget {
    DownloadTarget {
        base: "http://127.0.0.1:8081".to_owned(),
        kind: DownloadKind::Qbittorrent,
    }
}

/// The Usenet client, pointed at the configuration its key is read from.
fn usenet() -> DownloadTarget {
    DownloadTarget {
        base: "http://127.0.0.1:8080".to_owned(),
        kind: DownloadKind::Sabnzbd {
            config: PathBuf::from("/srv/config/sabnzbd/sabnzbd.ini"),
        },
    }
}

/// Both clients as read targets, in the order the manifest declares them.
fn both() -> Vec<DownloadTarget> {
    vec![torrent(), usenet()]
}

#[tokio::test]
async fn both_kinds_of_client_are_opened_by_what_each_authenticates_with() {
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env_at("bandwidth-open", &a_password())),
            ..Settings::default()
        })
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(None, Some(KEYED))));
    let names: Vec<&str> = opened(&ctx, &both())
        .await
        .iter()
        .map(super::Client::name)
        .collect();
    assert_eq!(names, ["qbittorrent", "sabnzbd"]);
}

#[tokio::test]
async fn a_client_lemonfiber_cannot_authenticate_to_is_left_out_rather_than_read_as_open() {
    // It is not a client with no limits; it is a client nothing here can see,
    // and reporting the two alike would be a report reading better than the
    // stack is. One has no recorded password and the other has written no key.
    let ctx = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    assert!(opened(&ctx, &both()).await.is_empty());
}

#[tokio::test]
async fn a_client_that_would_not_answer_is_reported_with_what_it_said() {
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env_at("bandwidth-silent-client", &a_password())),
            ..Settings::default()
        })
        .build()
        .with_http(Fake::silent());
    let clients = opened(&ctx, &[torrent()]).await;
    assert_eq!(clients.len(), 1, "the torrent client opened");

    for client in &clients {
        let reported = holding(client, &wanted(), None, false).await;
        let refusal = &reported.answer;
        assert!(
            parts(refusal).is_none(),
            "no figures came back, and an unknown limit rendered as no limit \
             is a report reading better than the stack is"
        );
        assert!(matches!(refusal, Answer::Silent { said } if said.contains("not answering")));
        assert!(reported.worth_saying());
    }
}

/// A torrent client answering every question, with `running` torrents and this
/// standing on whether it would start another.
fn a_client_that_answers(running: &'static str, would_start: &'static str) -> Arc<Fake> {
    Fake::by_path(vec![
        ("/api/v2/auth/login", Replies::reply(200, "Ok.")),
        ("/api/v2/torrents/info", Replies::reply(200, running)),
        (
            "/api/v2/app/setPreferences",
            Replies::reply(200, String::new()),
        ),
        (
            "/api/v2/app/preferences",
            Replies::reply(
                200,
                format!(
                    r#"{{"dl_limit":0,"up_limit":0,"alt_dl_limit":0,"alt_up_limit":0,
                        "scheduler_enabled":false,"add_stopped_enabled":{would_start}}}"#
                ),
            ),
        ),
        ("/api/v2/transfer/speedLimitsMode", Replies::reply(200, "0")),
        (
            "/api/v2/transfer/info",
            Replies::reply(200, r#"{"dl_info_speed":0,"up_info_speed":0}"#),
        ),
        ("/api/v2/torrents/stop", Replies::reply(200, String::new())),
        ("/api/v2/torrents/start", Replies::reply(200, String::new())),
    ])
}

/// The stack that client belongs to.
fn a_stack(scratch: &str, http: Arc<Fake>) -> crate::app::Ctx {
    a_context()
        .settings(Settings {
            env_file: Some(env_at(scratch, &a_password())),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

#[tokio::test]
async fn a_cap_makes_whether_the_client_is_fetching_part_of_what_it_answers() {
    // Both readings, because the two are what a spent cap is judged by: a
    // client still running something has not stopped, and one running nothing
    // that would start the next thing handed to it has not either.
    let ctx = a_stack("bandwidth-pulling", a_client_that_answers("[{}]", "false"));
    let clients = &opened(&ctx, &[torrent()]).await;
    assert!(
        !clients.is_empty(),
        "no client answered, so nothing below was asked"
    );
    for client in clients {
        let asked = holding(client, &wanted(), Some(Fetch::Ask), false).await;
        assert_eq!(asked.pulling, Some(Pulling::Fetching));
    }

    let idle = a_stack("bandwidth-idle", a_client_that_answers("[]", "true"));
    let clients = &opened(&idle, &[torrent()]).await;
    assert!(
        !clients.is_empty(),
        "no client answered, so nothing below was asked"
    );
    for client in clients {
        let asked = holding(client, &wanted(), Some(Fetch::Ask), false).await;
        assert_eq!(asked.pulling, Some(Pulling::Stopped));
        assert!(
            asked.worth_saying(),
            "a stopped client is always worth saying"
        );
    }
}

#[tokio::test]
async fn stopping_and_starting_are_two_named_requests_rather_than_one_flag() {
    let http = a_client_that_answers("[]", "true");
    let ctx = a_stack("bandwidth-stopping", http.clone());
    let clients = &opened(&ctx, &[torrent()]).await;
    assert!(
        !clients.is_empty(),
        "no client answered, so nothing below was asked"
    );
    for client in clients {
        assert_eq!(
            holding(client, &wanted(), Some(Fetch::Stop), false)
                .await
                .pulling,
            Some(Pulling::Stopped)
        );
        assert_eq!(
            holding(client, &wanted(), Some(Fetch::Resume), false)
                .await
                .pulling,
            Some(Pulling::Stopped),
            "what it reports afterwards, not what it was asked for"
        );
    }
    assert!(http.asked_for("/torrents/stop"));
    assert!(http.asked_for("/torrents/start"));
}

#[tokio::test]
async fn a_client_that_will_not_say_whether_it_is_fetching_reports_nothing_rather_than_a_guess() {
    // "Stopped" is exactly the wrong thing to say about a client nobody could
    // reach: it is the answer that reads as a cap being kept.
    let ctx = a_stack("bandwidth-unfetchable", Fake::silent());
    let clients = &opened(&ctx, &[torrent()]).await;
    assert!(
        !clients.is_empty(),
        "no client answered, so nothing below was asked"
    );
    for client in clients {
        assert!(holding(client, &wanted(), Some(Fetch::Ask), false)
            .await
            .pulling
            .is_none());
    }
}

#[tokio::test]
async fn what_a_client_has_moved_is_nothing_where_it_would_not_say() {
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(env_at("bandwidth-unmoved", &a_password())),
            ..Settings::default()
        })
        .build()
        .with_http(Fake::silent());
    let clients = &opened(&ctx, &[torrent()]).await;
    assert!(
        !clients.is_empty(),
        "no client answered, so nothing below was asked"
    );
    for client in clients {
        assert!(client.moved("2026-09").await.is_none());
    }
}

/// `SABnzbd`'s queue, holding at a quarter of a megabyte and pulling under it.
const QUEUED: &str = r#"{"queue":{"speedlimit_abs":"262144","kbpersec":"128.0"}}"#;

/// Its account statistics, kept by the day rather than as a running total.
const DAILY: &str = r#"{"servers":{"one":{"total":9000,"daily":{"2026-09-01":2000}}}}"#;

/// Its schedule, with nothing in it that would switch the rate.
const UNSCHEDULED: &str = r#"{"config":{"misc":{"schedlines":[]}}}"#;

#[tokio::test]
async fn the_usenet_client_is_asked_on_its_own_shape_rather_than_the_torrent_one() {
    // It answers about a limit and a month through entirely different calls,
    // and it has no upload and keeps no hours. A stack that reached it the
    // torrent client's way would report a working client as silent, and the
    // household would be held to a limit nothing had ever put on it.
    let ctx = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(None, Some(KEYED))))
        .with_http(Fake::by_path(vec![
            ("mode=queue", Replies::reply(200, QUEUED)),
            ("mode=get_config", Replies::reply(200, UNSCHEDULED)),
            ("mode=server_stats", Replies::reply(200, DAILY)),
        ]));
    let clients = opened(&ctx, &[usenet()]).await;
    assert_eq!(clients.len(), 1, "the Usenet client opened");

    for client in &clients {
        let reported = holding(client, &wanted(), None, false).await;
        assert_eq!(reported.client, "sabnzbd");
        // Asked whether it is fetching at all, which is what a spent cap is
        // judged by. It answers from its own queue flag rather than the
        // torrent client's pair of readings, and a stack that never asked it
        // would judge a Usenet-only household's cap on nothing.
        let asked = holding(client, &wanted(), Some(Fetch::Ask), false).await;
        assert_eq!(asked.pulling, Some(Pulling::Fetching));
        let answered = parts(&reported.answer);
        assert!(
            answered.is_some_and(|(down, up, period)| down.accepted == Some(SLOW)
                && down.verdict == Verdict::Holding
                && up.verdict == Verdict::NothingToLimit
                && period.is_none()),
            "it took the download figure, has no upload to have refused one, \
             and keeps no hours of its own"
        );
        assert!(
            client
                .moved("2026-09")
                .await
                .is_some_and(|moved| moved.down == 2_000 && moved.up == 0 && !moved.since_start),
            "a month it can answer for properly, rather than a total since it \
             last started"
        );
    }
}

#[test]
fn a_client_that_would_not_answer_says_so_in_its_own_words() {
    // The service's own detail where it gave one, so the operator reads what
    // refused rather than lemonfiber's interpretation of it.
    assert_eq!(
        said(&Failure::Refused {
            service: "sabnzbd".to_owned(),
            detail: "it said no".to_owned(),
        }),
        "it said no"
    );
    assert_eq!(
        said(&Failure::Unsupported {
            service: "sabnzbd".to_owned(),
            detail: "too old for this".to_owned(),
        }),
        "too old for this"
    );
    assert!(said(&Failure::Unavailable {
        service: "qbittorrent".to_owned(),
    })
    .contains("not answering"));
    assert!(said(&Failure::Unauthorised {
        service: "qbittorrent".to_owned(),
    })
    .contains("credential"));
}
