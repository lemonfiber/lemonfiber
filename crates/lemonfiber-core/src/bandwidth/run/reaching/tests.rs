use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer as Replies, Fake};

use lemonfiber_contract::Contracted;

use super::{answer, holding, opened, said, DownloadTarget, Fetch};
use crate::bandwidth::{Answer, Held, Period, Pulling, Verdict};
use crate::dashboard::Protocol;
use crate::ports::service::{Failure, Hours, Moved, Pulling as Answered, Rates, Throttled, Wanted};
use crate::test_support::{json, torrent, usenet};

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

/// The torrent client's service.
const TORRENT: &str = torrent::PLUGIN;

/// The torrent client as a read target, over its contract through `http`.
fn torrent_over(http: Arc<Fake>) -> DownloadTarget {
    DownloadTarget {
        service: TORRENT.to_owned(),
        protocol: Protocol::Torrent,
        adapter: Contracted::new(http, "http://127.0.0.1:8080", TORRENT, "torrent-key"),
    }
}

/// The Usenet client's service.
const USENET: &str = usenet::PLUGIN;

/// The Usenet client as a read target, over its contract through `http`.
fn usenet_over(http: Arc<Fake>) -> DownloadTarget {
    DownloadTarget {
        service: USENET.to_owned(),
        protocol: Protocol::Usenet,
        adapter: Contracted::new(http, "http://127.0.0.1:8080", USENET, "usenet-key"),
    }
}

#[test]
fn both_kinds_of_client_are_opened_in_the_order_they_are_declared() {
    let opened = opened(&[torrent_over(Fake::silent()), usenet_over(Fake::silent())]);
    let names: Vec<&str> = opened
        .iter()
        .map(|client| client.service.as_str())
        .collect();
    assert_eq!(names, [TORRENT, USENET]);
}

#[tokio::test]
async fn a_client_that_would_not_answer_is_reported_with_what_it_said() {
    let clients = opened(&[torrent_over(Fake::silent())]);
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

/// A torrent client answering over its contract that it is `pulling`, before and after
/// a stop or a resume.
fn a_client_that_answers(pulling: Answered) -> Arc<Fake> {
    Fake::by_path(vec![
        (
            torrent::operation("pulling"),
            Replies::reply(200, json(&pulling)),
        ),
        (
            torrent::operation("stop"),
            Replies::reply(200, json(&pulling)),
        ),
        (
            torrent::operation("resume"),
            Replies::reply(200, json(&pulling)),
        ),
    ])
}

#[tokio::test]
async fn a_cap_makes_whether_the_client_is_fetching_part_of_what_it_answers() {
    // Both readings, because the two are what a spent cap is judged by: a client still
    // fetching has not stopped, and one that says it is stopped has.
    for (said, judged) in [
        (Answered::Fetching, Pulling::Fetching),
        (Answered::Stopped, Pulling::Stopped),
    ] {
        let clients = &opened(&[torrent_over(a_client_that_answers(said))]);
        assert!(
            !clients.is_empty(),
            "no client answered, so nothing below was asked"
        );
        for client in clients {
            let asked = holding(client, &wanted(), Some(Fetch::Ask), false).await;
            assert_eq!(asked.pulling, Some(judged));
            if judged == Pulling::Stopped {
                assert!(
                    asked.worth_saying(),
                    "a stopped client is always worth saying"
                );
            }
        }
    }
}

#[tokio::test]
async fn stopping_and_starting_are_two_named_requests_rather_than_one_flag() {
    let http = a_client_that_answers(Answered::Stopped);
    let clients = &opened(&[torrent_over(http.clone())]);
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
    assert!(http.asked_for(torrent::operation("stop")));
    assert!(http.asked_for(torrent::operation("resume")));
}

#[tokio::test]
async fn a_client_that_will_not_say_whether_it_is_fetching_reports_nothing_rather_than_a_guess() {
    // "Stopped" is exactly the wrong thing to say about a client nobody could
    // reach: it is the answer that reads as a cap being kept.
    let clients = &opened(&[torrent_over(Fake::silent())]);
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
    let clients = &opened(&[torrent_over(Fake::silent())]);
    assert!(
        !clients.is_empty(),
        "no client answered, so nothing below was asked"
    );
    for client in clients {
        assert!(client.moved("2026-09").await.is_none());
    }
}

/// The Usenet client over its contract: holding down at a quarter of a megabyte and
/// pulling under it, with no upload and no hours of its own, and two thousand bytes
/// moved in the month asked about.
fn the_usenet_client_over_its_contract() -> DownloadTarget {
    let held = json(&Throttled {
        rates: Rates {
            down: Some(SLOW),
            up: None,
        },
        uploads: false,
        hours: None,
    });
    let http = Fake::by_path(vec![
        (
            usenet::operation("restrain"),
            Replies::reply(200, held.clone()),
        ),
        (usenet::operation("throttled"), Replies::reply(200, held)),
        (
            usenet::operation("moving"),
            Replies::reply(
                200,
                json(&Rates {
                    down: Some(SLOW / 2),
                    up: None,
                }),
            ),
        ),
        (
            usenet::operation("moved"),
            Replies::reply(
                200,
                json(&Moved {
                    down: 2_000,
                    up: 0,
                    since_start: false,
                }),
            ),
        ),
        (
            usenet::operation("pulling"),
            Replies::reply(200, json(&Answered::Fetching)),
        ),
    ]);
    usenet_over(http)
}

/// The Usenet client is judged as one that has no upload and keeps no hours, and is asked
/// for a month it can answer for. A stack that judged it as a torrent client would hold
/// the household to an upload limit nothing had ever put on it.
#[tokio::test]
async fn the_usenet_client_is_judged_with_no_upload_and_no_hours_over_its_contract() {
    let clients = opened(&[the_usenet_client_over_its_contract()]);
    assert_eq!(clients.len(), 1, "the Usenet client opened");

    for client in &clients {
        let reported = holding(client, &wanted(), None, false).await;
        assert_eq!(reported.client, USENET);
        let asked = holding(client, &wanted(), Some(Fetch::Ask), false).await;
        assert_eq!(
            asked.pulling,
            Some(Pulling::Fetching),
            "asked whether it is fetching at all, which is what a spent cap is judged by"
        );
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

/// A client asked over the contract of `protocol`, answering every limit and metering
/// question with figures of its own, uploading where `uploads`.
fn speaking_its_contract(protocol: Protocol, uploads: bool) -> (DownloadTarget, Arc<Fake>) {
    let held = json(&Throttled {
        rates: wanted().active,
        uploads,
        hours: None,
    });
    let http = Fake::by_path(vec![
        ("/restrain", Replies::reply(200, held.clone())),
        ("/throttled", Replies::reply(200, held)),
        ("/moving", Replies::reply(200, json(&Rates::default()))),
        (
            "/moved",
            Replies::reply(
                200,
                json(&Moved {
                    down: 900,
                    up: 80,
                    since_start: false,
                }),
            ),
        ),
        ("/pulling", Replies::reply(200, json(&Answered::Fetching))),
    ]);
    let target = DownloadTarget {
        service: "fetcher".to_owned(),
        protocol,
        adapter: Contracted::new(http.clone(), "http://127.0.0.1:8080", "fetcher", "the-key"),
    };
    (target, http)
}

#[tokio::test]
async fn a_client_speaking_its_protocols_contract_is_limited_and_metered_over_it() {
    for (protocol, capability, uploads) in [
        (Protocol::Torrent, "download.torrent", true),
        (Protocol::Usenet, "download.usenet", false),
    ] {
        let (target, http) = speaking_its_contract(protocol, uploads);
        let clients = opened(&[target]);
        assert_eq!(clients.len(), 1, "the contracted client opened");
        for client in &clients {
            let reported = holding(client, &wanted(), Some(Fetch::Ask), true).await;
            assert_eq!(reported.client, "fetcher");
            assert_eq!(reported.pulling, Some(Pulling::Fetching));
            let up = if uploads {
                Verdict::Holding
            } else {
                Verdict::NothingToLimit
            };
            assert!(
                parts(&reported.answer).is_some_and(|(down, held_up, period)| {
                    down.accepted == Some(SLOW)
                        && down.verdict == Verdict::Holding
                        && held_up.verdict == up
                        && period.is_none()
                }),
                "{:?}",
                reported.answer
            );
            assert!(client
                .moved("2026-09")
                .await
                .is_some_and(|moved| moved.down == 900 && moved.up == 80));
        }
        let base = format!("http://127.0.0.1:8080/lemonfiber/{capability}/v1/");
        let asked: Vec<String> = http.requests().into_iter().map(|one| one.url).collect();
        assert!(
            asked.iter().any(|url| url.ends_with("/restrain")),
            "{asked:?}"
        );
        assert!(asked.iter().all(|url| url.starts_with(&base)), "{asked:?}");
    }
}
