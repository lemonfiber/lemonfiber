use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{allowance, assemble, household, reaching, title_of, Ctx, Naming, Selection};
use crate::household::State;
use crate::model::{HouseholdReport, Restriction};
use crate::ports::service::Certificate;
use crate::ports::service::{Access, HouseholdRequest, Member};
use crate::recyclarr::Kind;
use crate::test_support::{a_context, a_password, SeedFs};
use std::collections::BTreeMap;
use std::time::SystemTime;

/// A Servarr config that opens a target, carrying a readable key.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

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
    statuses: (u8, u8),
) -> HouseholdRequest {
    HouseholdRequest {
        id: 0,
        made: None,
        member: member.to_owned(),
        kind,
        item,
        request_status: statuses.0,
        media_status: statuses.1,
    }
}

/// A title map holding one series and one film.
fn titles() -> BTreeMap<(&'static str, i64), String> {
    let mut titles = BTreeMap::new();
    titles.insert((Kind::Sonarr.section(), 11), "The Expanse".to_owned());
    titles.insert((Kind::Radarr.section(), 7), "Dune".to_owned());
    titles
}

/// A transport answering the media server's accounts, the request service's
/// sign-in and read, and the \*arr libraries, by the shape of the URL.
struct Fake {
    accounts: &'static str,
    folders: &'static str,
    ratings: &'static str,
    sign_in: &'static str,
    requests: &'static str,
    library: &'static str,
    refuse: bool,
    /// The account the request service holds for a member, where it holds one.
    ///
    /// Absent by default, which is a household nobody has signed into the request
    /// service — the state the cases about joining and naming are written against.
    account: Option<&'static str>,
    /// What the narrow permissions endpoint answers, to the read and to the write.
    ///
    /// One answer for both because the write only has to succeed: what it was sent
    /// is read back off the transport rather than out of its reply.
    permissions: (u16, &'static str),
}

impl Default for Fake {
    fn default() -> Self {
        Self {
            accounts: r#"[{"Id":"a1","Name":"Alex","HasPassword":true,
                "Policy":{"EnableAllFolders":true},
                "LastActivityDate":"2026-08-30T10:00:00Z"}]"#,
            folders: r#"{"Items":[{"Id":"lib-1","Name":"Films"}]}"#,
            // As the pinned image answers, including the row that carries no age
            // at all — its name for content it has no rating for.
            ratings: r#"[{"Name":"Unrated"},{"Name":"U","Value":0},
                {"Name":"12A","Value":12},{"Name":"15","Value":15}]"#,
            sign_in: "",
            requests: "",
            library: "[]",
            refuse: false,
            account: None,
            permissions: (200, r#"{"permissions":32}"#),
        }
    }
}

/// What one member's period has counted, as the request service works it out.
const COUNTS: &str = r#"{"movie":{"days":7,"limit":2,"used":0},"tv":{}}"#;

impl Fake {
    /// The scripted answers as a transport, routed by what each call asks for.
    fn transport(&self) -> Arc<Transport> {
        let mut routes = vec![
            // Ahead of `/Users`, whose text it contains: the media server signs
            // this program in before it will answer anything about accounts, and
            // a route matched by prefix would answer the sign-in with the list.
            (
                "/Users/AuthenticateByName",
                Answer::reply(200, r#"{"AccessToken":"token"}"#),
            ),
            ("/Library/MediaFolders", Answer::reply(200, self.folders)),
            (
                "/Localization/ParentalRatings",
                Answer::reply(200, self.ratings),
            ),
            ("/Users", Answer::reply(200, self.accounts)),
            (
                "/auth/jellyfin",
                Answer::reply(if self.refuse { 500 } else { 200 }, self.sign_in),
            ),
        ];
        if let Some(account) = self.account {
            // Ahead of the catch-all, which would answer an account with a library.
            routes.push(("/user/jellyfin/", Answer::reply(200, account)));
            routes.push(("/quota", Answer::reply(200, COUNTS)));
            routes.push((
                "/settings/permissions",
                Answer::reply(self.permissions.0, self.permissions.1),
            ));
        }
        routes.push(("/api/v1/request", Answer::reply(200, self.requests)));
        routes.push(("", Answer::reply(200, self.library)));
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

/// A context whose request service can be reached: the media-server password is
/// recorded, so `seerr_reader` resolves a client. Tagged so each test keeps its own
/// env file rather than racing on a shared one.
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
    let disk = SeedFs::keyed(Some(KEYED), None);
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
    crate::app::targets::record_secret(
        &context,
        crate::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    context
}

mod limits;
mod quota;
mod requests;
mod unreadable;
