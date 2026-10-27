use crate::ports::service::{MediaStatus, RequestStatus};
use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::naming::Titled;
use super::{allowance, assemble, household, reaching, titled, Ctx, Naming};
use crate::household::State;
use crate::model::{HouseholdReport, Restriction};
use crate::ports::media::Kind;
use crate::ports::service::{
    Access, Certificate, FoundItem, Headroom, Holding, HouseholdRequest, Left, Member,
    NamedLibrary, Requesting, Unrated,
};
use crate::quality::Selection;
use crate::test_support::curators::curated;
use crate::test_support::requests as request_service;
use crate::test_support::{a_context, a_password, json, media, SeedFs};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// A request service that answered nothing, which is what these cases are about.
fn nothing_asked() -> allowance::Asked {
    allowance::Asked {
        household: None,
        members: BTreeMap::new(),
    }
}

/// A request service that answered about each member named, and about nobody else.
///
/// Absent is not false here either: somebody left out is somebody it could not be
/// asked about, which is what these cases turn on.
fn asked_of(requesting: &BTreeMap<String, bool>) -> allowance::Asked {
    allowance::Asked {
        household: None,
        members: requesting
            .iter()
            .map(|(id, approves_own)| {
                (
                    id.clone(),
                    allowance::Held {
                        id: format!("seerr-{id}"),
                        approves_own: *approves_own,
                        headroom: crate::ports::service::Headroom::default(),
                    },
                )
            })
            .collect(),
    }
}

/// A request as the service records it, for the grouping tests.
fn request(
    member: &str,
    kind: Option<Kind>,
    item: Option<i64>,
    statuses: (RequestStatus, MediaStatus),
) -> HouseholdRequest {
    HouseholdRequest {
        arrived: None,
        shelf_id: None,
        id: 0,
        made: None,
        member: member.to_owned(),
        member_id: None,
        kind,
        title_id: None,
        item,
        request_status: Some(statuses.0),
        media_status: Some(statuses.1),
    }
}

/// The series every curator's library holds in these cases.
fn the_expanse() -> FoundItem {
    FoundItem {
        id: 1,
        title: "The Expanse".to_owned(),
        year: None,
        monitored: true,
    }
}

/// Alex's request for [`the_expanse`], approved and arrived.
fn arrived() -> HouseholdRequest {
    request(
        "Alex",
        Some(Kind::Tv),
        Some(1),
        (RequestStatus::Approved, MediaStatus::Available),
    )
}

/// A title map holding one series and one film.
fn titles() -> BTreeMap<(Kind, i64), Titled> {
    let named = |title: &str, year: Option<u16>| Titled {
        title: title.to_owned(),
        year,
    };
    let mut titles = BTreeMap::new();
    titles.insert((Kind::Tv, 11), named("The Expanse", Some(2015)));
    titles.insert((Kind::Movies, 7), named("Dune", None));
    titles
}

/// A transport answering the media server's accounts, the request service's check of
/// its own key and its reads, and the curators' libraries, each over its contract.
struct Fake {
    accounts: String,
    folders: String,
    ratings: String,
    /// What the request service answers when asked whether it takes lemonfiber's key.
    answers: Answer,
    /// What the household has asked for.
    requests: Answer,
    /// What every curator's library holds.
    library: Answer,
    /// What the household may ask for where nobody chose otherwise for one person.
    asking: Answer,
    /// The account the request service holds for a member, where it holds one.
    ///
    /// Absent by default, which is a household nobody has signed into the request
    /// service — the state the cases about joining and naming are written against.
    account: Option<&'static str>,
    /// What taking a member's asking away answers, and what giving it back answers.
    holding: (Answer, Answer),
    /// What the request service's own record of a title answers, where a case names one.
    film: Option<Answer>,
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            accounts: json(&[alex(None, Unrated::LetThrough)]),
            folders: json(&[NamedLibrary {
                id: "lib-1".to_owned(),
                name: "Films".to_owned(),
            }]),
            ratings: json(&[
                Certificate {
                    name: "U".to_owned(),
                    age: 0,
                },
                Certificate {
                    name: "12A".to_owned(),
                    age: 12,
                },
                Certificate {
                    name: "15".to_owned(),
                    age: 15,
                },
            ]),
            answers: request_service::taken(),
            requests: request_service::answered(&Vec::<HouseholdRequest>::new()),
            library: request_service::answered(&Vec::<FoundItem>::new()),
            asking: request_service::refusing(),
            account: None,
            holding: (
                request_service::answered(&Holding { taken: 32 }),
                request_service::taken(),
            ),
            film: None,
        }
    }
}

/// What one member's period has counted, as the request service works it out.
fn counts() -> Headroom {
    Headroom {
        films: Left {
            limit: Some(2),
            used: 0,
            days: Some(7),
        },
        television: Left::default(),
    }
}

impl Fake {
    /// The scripted answers as a transport, routed by what each call asks for.
    fn transport(&self) -> Arc<Transport> {
        let mut routes = vec![
            (
                media::identity("libraries"),
                Answer::reply(200, self.folders.clone()),
            ),
            (
                media::identity("ratings"),
                Answer::reply(200, self.ratings.clone()),
            ),
            (media::identity("country"), Answer::reply(200, "null")),
            (
                media::identity("household"),
                Answer::reply(200, self.accounts.clone()),
            ),
            (request_service::intake("answers"), self.answers.clone()),
            (request_service::intake("requests"), self.requests.clone()),
            (request_service::intake("asking"), self.asking.clone()),
            (curated("library"), self.library.clone()),
        ];
        match self.account {
            Some(id) => routes.extend([
                (
                    request_service::intake("member_for"),
                    request_service::answered(&Some(id)),
                ),
                (
                    request_service::intake("requesting"),
                    request_service::answered(&Some(Requesting {
                        id: id.to_owned(),
                        approves_own: false,
                    })),
                ),
                (
                    request_service::intake("left"),
                    request_service::answered(&counts()),
                ),
                (
                    request_service::intake("hold_requests"),
                    self.holding.0.clone(),
                ),
            ]),
            None => routes.push((
                request_service::intake("member_for"),
                request_service::answered(&None::<String>),
            )),
        }
        routes.push((
            request_service::intake("release_requests"),
            self.holding.1.clone(),
        ));
        if let Some(film) = &self.film {
            routes.push((request_service::intake("detail"), film.clone()));
        }
        Transport::by_path(routes)
    }
}

/// An account the media server holds, for the joining tests.
///
/// Somebody who has claimed theirs has been seen; an unclaimed invitation has not,
/// which is the whole difference between the two.
fn account(name: &str, claimed: bool) -> Member {
    Member {
        id: format!("id-{}", name.to_lowercase()),
        name: name.to_owned(),
        claimed,
        access: Access {
            every_library: true,
            ..Access::default()
        },
        last_seen: claimed.then(|| "2026-08-30T10:00:00Z".to_owned()),
    }
}

/// No library names read, which the joining tests do not depend on.
fn unnamed() -> BTreeMap<String, String> {
    BTreeMap::new()
}

/// A context whose media server, curators and request service have moved into their
/// plugins and can be reached: the media-server password is recorded, so
/// `household_requests` resolves a client. Tagged so each test keeps its own env file
/// rather than racing on a shared one.
fn ctx_with(fake: &Fake, tag: &str) -> Ctx {
    ctx_over(fake.transport(), tag, false)
}

/// What a volume with nothing left on it reports.
///
/// A total that was read and nothing free, which is the one reading that halts
/// acquisitions — a total of nought reads as a volume nobody could measure.
fn exhausted() -> crate::ports::filesystem::StorageFacts {
    crate::ports::filesystem::StorageFacts {
        point: std::path::PathBuf::new(),
        kind: crate::ports::filesystem::FsKind::Linking("test".to_owned()),
        removable: false,
        available: 0,
        total: 4 * 1024 * 1024 * 1024 * 1024,
    }
}

/// A context over the given transport, with or without room on its disk.
fn ctx_over(transport: Arc<Transport>, tag: &str, no_room: bool) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("household-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let disk = SeedFs::keyed(None);
    let mut context = a_context()
        .build()
        .with_filesystem(Arc::new(if no_room {
            disk.with_facts(exhausted())
        } else {
            disk
        }))
        .with_http(transport);
    context.settings.env_file = Some(dir.join(".env"));
    if no_room {
        // Measured at all only where there is somewhere to measure, so the reading
        // that halts needs a data location as much as it needs a full volume.
        context.settings.data_root = Some(dir);
    }
    let _ = crate::app::targets::record_secret(
        &context,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    let mut context = crate::test_support::adapted::reaching(context, &PARTS);
    context.seams.engine =
        Arc::new(crate::test_support::adapted::running(&PARTS).alongside(media::running()));
    media::moved_in(&context);
    context
}

/// Every part besides the media server a household reading reaches: the curators whose
/// libraries name what was asked for, and the request service.
const PARTS: [crate::test_support::adapted::Adapted; 4] = [
    crate::test_support::curators::TELEVISION,
    crate::test_support::curators::FILM,
    crate::test_support::curators::MUSIC,
    crate::test_support::requests::REQUESTS,
];

/// Alex, the one account the media server holds, held to `age_limit` where one is set and
/// doing `unrated` with content it has no rating for.
fn alex(age_limit: Option<u32>, unrated: Unrated) -> Member {
    let mut alex = media::member("a1", "Alex", true);
    alex.last_seen = Some("2026-08-30T10:00:00Z".to_owned());
    alex.access.age_limit = age_limit;
    alex.access.unrated = unrated;
    alex
}

mod contracted;
mod defaults;
mod limits;
mod quota;
mod requests;
mod unreadable;
