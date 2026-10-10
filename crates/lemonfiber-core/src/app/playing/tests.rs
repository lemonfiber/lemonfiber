use std::sync::Arc;

use lemonfiber_fixtures::http::{Answer, Fake as Transport};

use super::{account as whose, asked, playing, unread, Ctx};
use crate::app::Outcome;
use crate::ports::service::{Access, Medium, Member};
use crate::test_support::{a_context, a_password, SeedFs};

/// A curator config carrying a readable key, so the stack resolves its targets.
const KEYED: &str = "<Config><ApiKey>the-key</ApiKey></Config>";

/// Ada watching an episode and Bram paused on a film, in the server's own words.
const BOTH_WATCHING: &str = r#"[
    {"UserId":"a7f3","UserName":"Ada","DeviceName":"TV",
     "NowPlayingItem":{"Name":"Pilot","SeriesName":"The Expanse","ParentIndexNumber":1,
                       "IndexNumber":1,"Type":"Episode"}},
    {"UserId":"b2e9","UserName":"Bram","DeviceName":"Phone",
     "NowPlayingItem":{"Name":"Arrival","Type":"Movie"},"PlayState":{"IsPaused":true}}
]"#;

/// The media server's scripted answers to the two questions this read asks it.
struct Server {
    /// Who it says holds an account.
    accounts: &'static str,
    /// What it answers about its sessions, and with what status.
    sessions: (u16, &'static str),
}

impl Default for Server {
    fn default() -> Self {
        Self {
            accounts: r#"[{"Id":"a7f3","Name":"Ada","HasPassword":true},
                          {"Id":"b2e9","Name":"Bram","HasPassword":true}]"#,
            sessions: (200, BOTH_WATCHING),
        }
    }
}

impl Server {
    /// The scripted answers as a transport, routed by what each call asks for.
    fn transport(&self) -> Arc<Transport> {
        Transport::by_path(vec![
            (
                "/Users/AuthenticateByName",
                Answer::reply(200, r#"{"AccessToken":"token"}"#),
            ),
            ("/Sessions", Answer::reply(self.sessions.0, self.sessions.1)),
            ("/Users", Answer::reply(200, self.accounts)),
            ("", Answer::reply(200, "[]")),
        ])
    }
}

/// A context whose media server can be signed in to, or not where `signs_in` is false.
fn ctx_over(transport: Arc<Transport>, tag: &str, signs_in: bool) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("playing-{tag}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let mut context = a_context()
        .build()
        .with_filesystem(Arc::new(SeedFs::keyed(Some(KEYED), None)))
        .with_http(transport);
    context.settings.env_file = Some(dir.join(".env"));
    if signs_in {
        let _ = crate::app::targets::record_secret(
            &context,
            crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
            &a_password(),
        );
    }
    context
}

fn ctx_with(server: &Server, tag: &str) -> Ctx {
    ctx_over(server.transport(), tag, true)
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

fn household() -> Vec<Member> {
    vec![account("a7f3", "Ada"), account("b2e9", "Bram")]
}

/// The members whose sessions a report carries.
fn watching(report: &crate::model::PlayingReport) -> Vec<String> {
    report
        .sessions
        .iter()
        .map(|one| one.member.clone())
        .collect()
}

/// A member's own session carries the id, so the id answers first.
#[test]
fn an_id_names_whose_sessions_they_are() {
    assert_eq!(
        whose(&household(), "b2e9").map(|held| &held.name),
        Some(&"Bram".to_owned())
    );
}

/// An operator types a name, the way they say it.
#[test]
fn a_name_names_them_however_it_was_typed() {
    assert_eq!(
        whose(&household(), "ADA").map(|held| &held.id),
        Some(&"a7f3".to_owned())
    );
}

#[test]
fn a_reading_that_could_not_be_made_says_so_rather_than_reading_as_nobody_watching() {
    let report = unread(None, "the server would not say");
    assert!(!report.available);
    assert!(report.sessions.is_empty());
    assert_eq!(report.member, "");
    assert_eq!(report.findings, vec!["the server would not say".to_owned()]);
}

/// Naming nobody is the whole house: every session playing something.
#[tokio::test]
async fn naming_nobody_is_every_session_in_the_house() {
    let context = ctx_with(&Server::default(), "everybody");

    let report = playing(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!(report.member, "");
    assert!(report.findings.is_empty(), "{report:?}");
    assert_eq!(watching(&report), vec!["Ada".to_owned(), "Bram".to_owned()]);
    assert_eq!(
        report
            .sessions
            .iter()
            .map(|one| (one.title.clone(), one.medium, one.paused))
            .collect::<Vec<_>>(),
        vec![
            ("Pilot".to_owned(), Medium::Series, false),
            ("Arrival".to_owned(), Medium::Film, true),
        ]
    );
}

/// **The narrowing, asserted.** A member's reading carries their sessions and no one
/// else's, asked for by the id their own session carries.
#[tokio::test]
async fn a_member_is_answered_with_their_own_sessions_and_nobody_elses() {
    let context = ctx_with(&Server::default(), "theirs");

    let report = playing(&context, Some("a7f3")).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!(report.member, "Ada");
    assert_eq!(watching(&report), vec!["Ada".to_owned()]);
    assert!(
        report.sessions.iter().all(|one| one.member_id == "a7f3"),
        "{report:?}"
    );
}

/// An operator naming somebody by name reads that person's sessions.
#[tokio::test]
async fn an_operator_naming_somebody_reads_theirs() {
    let context = ctx_with(&Server::default(), "named");

    let report = playing(&context, Some("bram")).await.unwrap_or_default();

    assert_eq!(report.member, "Bram");
    assert_eq!(watching(&report), vec!["Bram".to_owned()]);
}

/// Nobody of that name is said as nobody of that name, never as somebody watching
/// nothing — and never answered with everybody's.
#[tokio::test]
async fn nobody_of_that_name_is_unread_rather_than_everybody() {
    let context = ctx_with(&Server::default(), "stranger");

    let report = playing(&context, Some("Cleo")).await.unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert!(report.sessions.is_empty(), "{report:?}");
    assert_eq!(report.member, "Cleo");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("nobody in this household")),
        "{report:?}"
    );
}

/// A server that will not say who holds an account leaves a member's reading unread,
/// and never falls back to every session.
#[tokio::test]
async fn a_household_the_server_will_not_name_leaves_it_unread() {
    let context = ctx_with(
        &Server {
            accounts: "not json",
            ..Server::default()
        },
        "nameless",
    );

    let report = playing(&context, Some("Ada")).await.unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert!(report.sessions.is_empty(), "{report:?}");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("who holds an account")),
        "{report:?}"
    );
}

/// Sessions the server will not list are unread, still saying whose they were.
#[tokio::test]
async fn sessions_the_server_will_not_list_are_unread_and_still_say_whose() {
    let context = ctx_with(
        &Server {
            sessions: (500, ""),
            ..Server::default()
        },
        "refused",
    );

    let report = playing(&context, Some("Ada")).await.unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert_eq!(report.member, "Ada");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("unread rather than as nobody watching")),
        "{report:?}"
    );
}

/// No password to sign in with is said, rather than read as a quiet house.
#[tokio::test]
async fn no_media_server_to_ask_is_unread() {
    let context = ctx_over(Server::default().transport(), "no-password", false);

    let report = playing(&context, None).await.unwrap_or_default();

    assert!(!report.available, "{report:?}");
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("no media server to ask")),
        "{report:?}"
    );
}

/// A stack that cannot be read is an error rather than a reading.
#[tokio::test]
async fn a_reading_over_an_unreadable_stack_is_an_error() {
    let mut context = ctx_with(&Server::default(), "badstack");
    context.stack = crate::stack::Source::External(std::path::Path::new("/nowhere/at/all"));

    assert!(playing(&context, None).await.is_err());
}

/// The command answers with the reading as its outcome, the same reading the read
/// makes.
#[tokio::test]
async fn the_command_answers_with_the_reading() {
    let context = ctx_with(&Server::default(), "outcome");

    let answered = asked(&context, Some("Bram")).await;

    assert!(
        matches!(&answered, Ok(Outcome::Playing(report)) if report.member == "Bram"),
        "{answered:?}"
    );
}

/// **A plugin's media server is asked, at its own address, with its own password.**
/// Where a plugin fills the household's identity, what is playing is read from that
/// server through the same lookup every other read of the household takes, so a stack
/// whose media server is a stand-in answers rather than reading as a quiet house.
#[tokio::test]
async fn a_plugin_standing_in_for_the_media_server_is_the_one_asked() {
    let mut placed = crate::test_support::a_placed(
        "emby",
        &["identity.source", "media.serve"],
        Some(lemonfiber_manifest::Api {
            kind: lemonfiber_manifest::ApiKind::Jellyfin,
            key_source: lemonfiber_manifest::KeySource::Generated,
            path: None,
            version: None,
        }),
        Some(8920),
    );
    placed.tag = "4.9.1".to_owned();
    let mut register = crate::plugin::Register::empty();
    assert!(register
        .record(crate::test_support::an_installed(
            "emby-server",
            vec![placed]
        ))
        .is_ok());

    let transport = Server::default().transport();
    let context = ctx_over(std::sync::Arc::clone(&transport), "plugin-server", false);
    let env = context.settings.env_file.clone().unwrap_or_default();
    assert!(
        crate::config::store::set(&env, crate::wiring::FILLS_KEY, "identity.source=emby").is_ok()
    );
    assert!(crate::config::store::set(
        &env,
        "PLUGIN_EMBY__SERVER_EMBY_ADMIN_PASSWORD",
        &a_password(),
    )
    .is_ok());
    let kept = crate::app::plugins::kept_at(&context).unwrap_or_default();
    assert!(std::fs::write(&kept, register.to_json().unwrap_or_default()).is_ok());

    let report = playing(&context, None).await.unwrap_or_default();

    assert!(report.available, "{report:?}");
    assert_eq!(watching(&report), vec!["Ada".to_owned(), "Bram".to_owned()]);
    let asked: Vec<String> = transport
        .requests()
        .into_iter()
        .map(|one| one.url)
        .collect();
    assert!(
        asked.iter().any(|url| url.contains(":8920/Sessions")),
        "the plugin's server was not the one asked: {asked:?}"
    );
    assert!(
        !asked.iter().any(|url| url.contains(":8096/")),
        "the stack's own media server was asked: {asked:?}"
    );
}
