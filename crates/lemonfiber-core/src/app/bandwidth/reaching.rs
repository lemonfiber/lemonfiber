//! Opening the download clients, and putting the limits to them.
//!
//! Every client is asked the same three things — what it is limited to, what it is
//! moving, and (where the run is applying) what it says after being told — and each
//! answers on its own shape. A client that will not answer is a line of its own in
//! the report rather than an absence, because an unknown limit rendered as no limit
//! is a report reading better than the stack is.
//!
//! Reading is done for every client whether or not anything is being written, so
//! the unconfirmed run and the applying one produce the same shape of answer and
//! there is no second rendering to fall out of step with the first.

use crate::app::targets::{DownloadKind, DownloadTarget};
use crate::app::Ctx;
use crate::bandwidth::{Answer, Held, Holding, Period};
use crate::ports::service::{
    Failure, Hours, Metering, Moved, Rates, Throttled, Throttling, Wanted,
};
use crate::qbittorrent::Qbittorrent;
use crate::sabnzbd::Sabnzbd;

/// A download client this command can reach.
///
/// Boxed on both arms because the two clients are very different sizes and an
/// enum as large as its largest arm would be carried around at that size for
/// every one of them.
pub(super) enum Client {
    /// The torrent client, which uploads and keeps a schedule of its own.
    Torrent(Box<Qbittorrent>),
    /// The Usenet client, which does neither.
    Usenet(Box<Sabnzbd>),
}

impl Client {
    /// The name the stack knows it under, which is what the report names it by.
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Torrent(_) => "qbittorrent",
            Self::Usenet(_) => "sabnzbd",
        }
    }

    /// The limits on it.
    fn throttling(&self) -> &dyn Throttling {
        match self {
            Self::Torrent(client) => client.as_ref(),
            Self::Usenet(client) => client.as_ref(),
        }
    }

    /// What it has moved.
    fn metering(&self) -> &dyn Metering {
        match self {
            Self::Torrent(client) => client.as_ref(),
            Self::Usenet(client) => client.as_ref(),
        }
    }

    /// What this client has moved in `month`, or nothing where it would not say.
    pub(super) async fn moved(&self, month: &str) -> Option<Moved> {
        self.metering().moved(month).await.ok()
    }
}

/// Every download client on this stack that can be opened, in the order the
/// manifest declares them.
///
/// A client lemonfiber cannot authenticate to is left out rather than reported as
/// unlimited: it is not a client with no limits, it is a client nothing here can
/// see, and the two must not render alike.
pub(super) async fn opened(ctx: &Ctx, targets: &[DownloadTarget]) -> Vec<Client> {
    let mut clients = Vec::new();
    for target in targets {
        match &target.kind {
            DownloadKind::Qbittorrent => {
                if let Some(password) = crate::app::targets::recorded_qbittorrent_password(ctx) {
                    clients.push(Client::Torrent(Box::new(Qbittorrent::authenticated(
                        ctx.http.clone(),
                        &target.base,
                        password,
                    ))));
                }
            }
            DownloadKind::Sabnzbd { config } => {
                if let Some(key) = ctx
                    .filesystem
                    .read(config)
                    .await
                    .as_deref()
                    .and_then(crate::sabnzbd::api_key)
                {
                    clients.push(Client::Usenet(Box::new(Sabnzbd::new(
                        ctx.http.clone(),
                        &target.base,
                        key,
                    ))));
                }
            }
        }
    }
    clients
}

/// Put the limits to one client where `writing`, and read back what it says.
///
/// The read-back is the same call in both cases, so the answer an unconfirmed run
/// reports and the answer an applying run reports come from one place — which is
/// what stops a rehearsal describing something the real thing would not do.
pub(super) async fn holding(client: &Client, wanted: &Wanted, writing: bool) -> Holding {
    let answered = if writing {
        client.throttling().restrain(wanted).await
    } else {
        client.throttling().throttled().await
    };
    Holding {
        client: client.name().to_owned(),
        answer: match answered {
            Ok(held) => {
                let moving = client.throttling().moving().await.unwrap_or_default();
                answer(wanted, &held, &moving)
            }
            Err(failure) => Answer::Silent {
                said: said(&failure),
            },
        },
    }
}

/// What one client's answer amounts to in both directions.
fn answer(wanted: &Wanted, held: &Throttled, moving: &Rates) -> Answer {
    // What was asked of it is whichever side of the day it says it is on, because
    // that is the limit it is under right now — comparing a client in its quiet
    // hours against the active figure would report every well-behaved stack as
    // having ignored its limits every night.
    let asked = match held.hours {
        Some(Hours::Quiet) => wanted.quiet,
        Some(Hours::Active) | None => wanted.active,
    };
    Answer::Held {
        down: Held::of(asked.down, held.rates.down, moving.down, true),
        up: Held::of(asked.up, held.rates.up, moving.up, held.uploads),
        period: held.hours.map(|hours| match hours {
            Hours::Active => Period::Active,
            Hours::Quiet => Period::Quiet,
        }),
    }
}

/// What a client that would not answer said, in its own words.
///
/// The service's own detail where it gave one, so the operator reads what refused
/// rather than an interpretation of it; the failure's own sentence otherwise.
fn said(failure: &Failure) -> String {
    match failure {
        Failure::Refused { detail, .. } | Failure::Unsupported { detail, .. } => detail.clone(),
        Failure::Unavailable { .. } | Failure::Unauthorised { .. } => failure.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use lemonfiber_fixtures::http::Fake;

    use super::{answer, holding, opened, said, DownloadKind, DownloadTarget};
    use crate::bandwidth::{Answer, Held, Period, Verdict};
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
            answered.is_some_and(|(down, _, period)| down.verdict == Verdict::Unasked
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
        let first = clients.first();
        assert!(first.is_some(), "the torrent client opened");

        for client in &clients {
            let said = holding(client, &wanted(), false).await;
            assert!(matches!(said.answer, Answer::Silent { .. }));
            assert!(said.worth_saying());
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
        for client in &opened(&ctx, &[torrent()]).await {
            assert!(client.moved("2026-09").await.is_none());
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
}
