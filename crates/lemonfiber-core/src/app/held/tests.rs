use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{account as whose, held, unread, Ctx, Whom};
use crate::ports::service::{Access, Medium, Member};
use crate::test_support::{a_context, a_password, SeedFs};

/// A curator config carrying a readable key, so the stack resolves its targets.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// The media server's scripted answers to the two questions this read asks it.
struct Server {
    /// Who it says holds an account.
    accounts: &'static str,
    /// What it answers about one account's shelf, and with what status.
    shelf: (u16, &'static str),
}

impl Default for Server {
    fn default() -> Self {
        Self {
            accounts: r#"[{"Id":"a7f3","Name":"Ada","HasPassword":true,
                "Policy":{"EnableAllFolders":true}}]"#,
            shelf: (
                200,
                r#"{"Items":[
                    {"Id":"i1","Name":"Arrival","ProductionYear":2016,"Type":"Movie"},
                    {"Id":"i2","Name":"The Expanse","Type":"Series"}]}"#,
            ),
        }
    }
}

impl Server {
    /// The scripted answers as a transport, routed by what each call asks for.
    ///
    /// The shelf sits ahead of the account list because `/Users/{id}/Items`
    /// contains both fragments, and behind the sign-in for the same reason.
    fn transport(&self) -> Arc<Transport> {
        Transport::by_path(vec![
            (
                "/Users/AuthenticateByName",
                Answer::reply(200, r#"{"AccessToken":"token"}"#),
            ),
            ("/Items", Answer::reply(self.shelf.0, self.shelf.1)),
            ("/Users", Answer::reply(200, self.accounts)),
            ("", Answer::reply(200, "[]")),
        ])
    }
}

/// A context whose media server can be reached: the admin password is recorded, so
/// the media server is asked as its administrator. Tagged so each case keeps its own env file
/// rather than racing on a shared one.
fn ctx_with(server: &Server, tag: &str) -> Ctx {
    ctx_over(server.transport(), tag)
}

/// The same context over a transport the caller keeps, to read back what was asked.
fn ctx_over(transport: Arc<Transport>, tag: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("held-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(transport);
    context.settings.env_file = Some(dir.join(".env"));
    let _ = crate::app::targets::record_secret(
        &context,
        crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
        &a_password(),
    );
    context
}

fn account(id: &str, name: &str) -> Member {
    Member {
        id: id.to_owned(),
        name: name.to_owned(),
        claimed: true,
        access: Access::default(),
        last_seen: None,
    }
}

fn named(member: &str) -> Whom {
    Whom::Named(member.to_owned())
}

fn household() -> Vec<Member> {
    vec![account("a7f3", "Ada"), account("b2e9", "Bram")]
}

/// A member's own session carries the id, so the id is what has to answer — and it
/// has to answer before the name does, or a household where somebody is called by
/// another member's id would hand over the wrong shelf.
#[test]
fn an_id_names_whose_shelf_it_is() {
    assert_eq!(
        whose(&household(), "a7f3").map(|held| &held.name),
        Some(&"Ada".to_owned())
    );
}

/// An operator types a name, and types it the way they say it.
#[test]
fn a_name_names_it_too_however_it_was_typed() {
    assert_eq!(
        whose(&household(), "BRAM").map(|held| &held.id),
        Some(&"b2e9".to_owned())
    );
}

#[test]
fn nobody_of_that_name_is_nobody() {
    assert!(whose(&household(), "Cleo").is_none());
}

/// The load-bearing one. Everything that could not be read answers through here, and
/// what it must never do is come back looking like a household that owns nothing.
#[test]
fn a_shelf_that_could_not_be_read_says_so_rather_than_reading_as_empty() {
    let report = unread(&named("Ada"), "the server would not say");
    assert!(!report.available);
    assert!(report.holdings.is_empty());
    assert_eq!(report.findings, vec!["the server would not say".to_owned()]);
}

/// The whole read, as the server answers it.
///
/// Asserted on the items rather than only on the mark, because a report marked
/// readable and carrying nothing is the exact reading this must never produce.
#[tokio::test]
async fn a_shelf_that_reads_comes_back_as_the_server_answered_it() {
    let context = ctx_with(&Server::default(), "read");

    let report = held(&context, &named("Ada"), 100).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!(report.member, "Ada");
    assert_eq!(report.id, "a7f3");
    assert!(report.findings.is_empty(), "{report:?}");
    assert_eq!(
        report
            .holdings
            .iter()
            .map(|held| (held.title.clone(), held.year, held.medium))
            .collect::<Vec<_>>(),
        vec![
            ("Arrival".to_owned(), Some(2016), Medium::Film),
            ("The Expanse".to_owned(), None, Medium::Series),
        ]
    );
}

/// Asked for by the id a member's own session carries, rather than by the name an
/// operator types. One shelf either way, because one resolution answers both.
#[tokio::test]
async fn the_same_shelf_answers_an_id_as_answers_a_name() {
    let context = ctx_with(&Server::default(), "by-id");

    let report = held(&context, &named("a7f3"), 100)
        .await
        .unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!(report.member, "Ada");
    assert_eq!(report.holdings.len(), 2, "{report:?}");
}

/// A media server that will not say who holds an account leaves the shelf unread.
///
/// Not as a household that owns nothing: whose shelf this is was never established,
/// so there is no account to report an empty one about.
#[tokio::test]
async fn a_household_the_server_will_not_name_leaves_the_shelf_unread() {
    let context = ctx_with(
        &Server {
            accounts: "not json",
            ..Server::default()
        },
        "nameless",
    );

    let report = held(&context, &named("Ada"), 100).await.unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert!(report.holdings.is_empty(), "{report:?}");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("who holds an account")),
        "{report:?}"
    );
}

/// Nobody of that name is said as nobody of that name.
///
/// The distinction the finding has to carry: there being no such member and there
/// being a member with nothing on their shelf are different facts, and answering
/// the first with the second tells a household something untrue about somebody who
/// does not exist.
#[tokio::test]
async fn nobody_of_that_name_has_no_shelf_rather_than_an_empty_one() {
    let context = ctx_with(&Server::default(), "stranger");

    let report = held(&context, &named("Cleo"), 100)
        .await
        .unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert_eq!(report.member, "Cleo");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("nobody in this household")),
        "{report:?}"
    );
}

/// A shelf the server will not answer still says whose it was.
///
/// Who was asked about was established before the shelf was asked for, so it is
/// known — and dropping it would make an unread shelf indistinguishable from a
/// member nobody could find.
#[tokio::test]
async fn a_shelf_the_server_will_not_answer_is_unread_and_still_says_whose() {
    let context = ctx_with(
        &Server {
            shelf: (500, ""),
            ..Server::default()
        },
        "refused",
    );

    let report = held(&context, &named("Ada"), 100).await.unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert!(report.holdings.is_empty(), "{report:?}");
    assert_eq!(report.member, "Ada");
    assert_eq!(report.id, "a7f3");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("reported as unread rather than as empty")),
        "{report:?}"
    );
}

/// A stack that cannot be read is an error rather than a shelf.
///
/// The one refusal here that is not a finding: without a manifest there is no media
/// server to address the question to, so there is nothing to report about.
#[tokio::test]
async fn a_shelf_over_an_unreadable_stack_is_an_error() {
    let mut context = ctx_with(&Server::default(), "badstack");
    context.stack = crate::stack::Source::External(std::path::Path::new("/nowhere/at/all"));

    assert!(held(&context, &named("Ada"), 100).await.is_err());
}

/// The household's defaults are nobody's shelf: what comes back is what an account
/// with every library and no age limit holds, said under no name and no id.
#[tokio::test]
async fn the_defaults_shelf_reads_under_no_name() {
    let context = ctx_with(&Server::default(), "defaults");

    let report = held(&context, &Whom::Defaults, 100)
        .await
        .unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!((report.member.as_str(), report.id.as_str()), ("", ""));
    assert_eq!(report.holdings.len(), 2, "{report:?}");
    assert!(report.findings.is_empty(), "{report:?}");
}

/// Reading the defaults reads no account. Who holds one is never asked, so nothing
/// about any member can reach the answer.
#[tokio::test]
async fn the_defaults_shelf_asks_nothing_about_who_holds_an_account() {
    let transport = Server::default().transport();
    let context = ctx_over(Arc::clone(&transport), "defaults-asks");

    let _ = held(&context, &Whom::Defaults, 100).await;

    let asked: Vec<String> = transport
        .requests()
        .into_iter()
        .map(|one| one.url)
        .collect();
    assert!(
        asked.iter().any(|url| url.contains("/Items?")),
        "the shelf was never asked for: {asked:?}"
    );
    assert!(
        !asked
            .iter()
            .any(|url| url.contains("/Users") && !url.contains("/Users/AuthenticateByName")),
        "an account was read for the defaults: {asked:?}"
    );
}

/// A defaults shelf the server will not answer is unread, and still names nobody.
#[tokio::test]
async fn a_defaults_shelf_the_server_will_not_answer_is_unread() {
    let context = ctx_with(
        &Server {
            shelf: (500, ""),
            ..Server::default()
        },
        "defaults-refused",
    );

    let report = held(&context, &Whom::Defaults, 100)
        .await
        .unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert_eq!(report.member, "");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("reported as unread rather than as empty")),
        "{report:?}"
    );
}

/// With no media server to ask, the defaults shelf is unread under no name rather
/// than under the name of whoever the last read was about.
#[test]
fn an_unread_defaults_shelf_names_nobody() {
    let report = unread(&Whom::Defaults, "no media server");
    assert!(!report.available);
    assert_eq!(report.member, "");
}
