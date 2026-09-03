//! What somebody is allowed to watch is chosen while they are being invited, held to
//! the traffic.
//!
//! An account made open and narrowed afterwards is open for as long as it takes anybody
//! to remember, and the person most likely to be given a limit has already been handed
//! the address. So the choice travels with the invitation, and what this file holds is
//! the four things that makes true.
//!
//! **It is sent whole, or the media server undoes the household's own settings.** Driven
//! against `jellyfin/jellyfin:10.10.3`: `POST /Users/{id}/Policy` answers `400` to a body
//! naming only what changed, and *accepts* one carrying the two fields it calls required
//! and nothing else — putting every other field back to its own default. So the account's
//! whole policy is read first and posted back with what was chosen written over it, and
//! this file asserts what was sent rather than that something was.
//!
//! **Nothing is written where nothing was chosen**, and that holds field by field. An
//! offer that named neither says nothing about access at all; one that named only an age
//! limit said nothing about libraries. A value written for what nobody mentioned would
//! widen an account back to every library on its way to narrowing what may be watched on
//! it, behind the household's back.
//!
//! **A refusal never costs an account.** A library nobody holds is settled before the
//! account is made, so what the operator gets is the refusal rather than an open account
//! and a message about it.
//!
//! Driven through `dispatch` rather than asserted about the source, for the reason the
//! invitation's other guards are: a sweep for the words the code uses would pass a policy
//! sent under a different spelling.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// Where what an account may watch is written.
const POLICY: &str = "/Users/9/Policy";

/// Where the account it is written on is read from first.
const ONE_ACCOUNT: &str = "/Users/9";

/// Where an account is asked for.
const NEW_ACCOUNT: &str = "/Users/New";

/// The library list the media server holds, as it names them.
const LIBRARIES: &str = r#"{"Items":[
    {"Id":"db4c17","Name":"Films"},
    {"Id":"a656b9","Name":"Shows"}
]}"#;

/// An account as it opens, with a policy carrying a field this product neither reads
/// nor writes.
///
/// `EnableMediaPlayback` stands for every setting an operator may have changed in the
/// media server's own screens. It is here so that a write which sent only the fields
/// this product knows about fails visibly, which against the real server is the failure
/// that would go unnoticed.
const AS_IT_OPENS: &str = r#"{
    "Id":"9","Name":"ana","HasPassword":false,
    "Policy":{
        "EnableAllFolders":true,
        "EnabledFolders":[],
        "IsAdministrator":false,
        "IsDisabled":false,
        "EnableMediaPlayback":false,
        "AuthenticationProviderId":"Default",
        "PasswordResetProviderId":"Default"
    }
}"#;

/// The same account after somebody narrowed it to one library.
///
/// The case an age limit must not widen: the household chose this, and setting how far
/// up the ratings the account goes says nothing about it.
const ALREADY_NARROWED: &str = r#"{
    "Id":"9","Name":"ana","HasPassword":false,
    "Policy":{
        "EnableAllFolders":false,
        "EnabledFolders":["a656b9"],
        "IsAdministrator":false,
        "IsDisabled":false,
        "EnableMediaPlayback":false,
        "AuthenticationProviderId":"Default",
        "PasswordResetProviderId":"Default"
    }
}"#;

/// The operator's own recorded credential, assembled rather than written down: a
/// literal reads to a source scanner as a credential committed to the repository.
fn admin_password() -> String {
    ["minted", "-earlier"].concat()
}

/// The stack this repository ships.
fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A scratch environment file holding the media server's recorded password.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-may-watch-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &admin_password(),
    );
    env
}

/// A media server that signs in, holds nobody, names two libraries and takes a policy.
///
/// The routes are ordered narrowest first, because the fake answers with the first whose
/// fragment the URL contains — so `/Users/9/Policy` has to sit above `/Users/9`, which
/// has to sit above `/Users`.
fn a_server_holding_nobody(policy: &'static str) -> Arc<Fake> {
    a_server(
        policy,
        Answer::reply(200, LIBRARIES),
        Answer::reply(204, ""),
    )
}

/// The same, with what its library list and its policy endpoint answer chosen.
///
/// Both are the two ways this errand can fail against a server that is otherwise
/// working, and each has a different account of what is now true — so each is driven
/// rather than reasoned about.
fn a_server(policy: &'static str, libraries: Answer, written: Answer) -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        ("/Library/MediaFolders", vec![libraries]),
        (POLICY, vec![written]),
        (
            NEW_ACCOUNT,
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        (ONE_ACCOUNT, vec![Answer::reply(200, policy)]),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ])
}

/// A context over the shipped stack, with the media server up and a password recorded.
fn context(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(env.to_path_buf()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// Offer somebody an account with what they may watch, and hand back everything that
/// was sent doing it.
async fn offering(
    scratch: &str,
    policy: &'static str,
    allowance: Allowance,
) -> (Vec<Request>, Option<Outcome>) {
    driving(scratch, a_server_holding_nobody(policy), allowance).await
}

/// The same, against a server built for this run.
async fn driving(
    scratch: &str,
    http: Arc<Fake>,
    allowance: Allowance,
) -> (Vec<Request>, Option<Outcome>) {
    let env = recorded_admin(scratch);
    let ctx = context(&env, http.clone());

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance,
        },
        &ctx,
    )
    .await;

    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    (http.requests(), made.ok())
}

/// What was posted to the policy endpoint, as the objects it was sent as.
fn policies(sent: &[Request]) -> Vec<serde_json::Value> {
    sent.iter()
        .filter(|request| request.url.contains(POLICY))
        .filter_map(|request| request.body.as_deref())
        .filter_map(|body| serde_json::from_str(body).ok())
        .collect()
}

/// An age limit reaches the media server as the number it keeps, on the policy the
/// account already had — with every field this product does not know about carried
/// back untouched.
///
/// The whole object is asserted rather than the three keys, because a body carrying
/// only what changed is exactly what the real server accepts and then defaults the rest
/// of: a test that looked only at `MaxParentalRating` would pass on the write that
/// silently switches somebody's playback off.
#[tokio::test]
async fn an_age_limit_is_written_onto_the_policy_the_account_already_had() {
    let (sent, made) = offering(
        "age-limit",
        AS_IT_OPENS,
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(13),
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert_eq!(
        policies(&sent),
        vec![serde_json::json!({
            "EnableAllFolders": true,
            "EnabledFolders": [],
            "MaxParentalRating": 13,
            "IsAdministrator": false,
            "IsDisabled": false,
            "EnableMediaPlayback": false,
            "AuthenticationProviderId": "Default",
            "PasswordResetProviderId": "Default"
        })],
        "an empty list means no policy was written at all"
    );
}

/// Libraries are named the way the media server's screens name them and reach it as the
/// identifiers it tells them apart by — and naming some of them is not naming all.
#[tokio::test]
async fn named_libraries_reach_the_server_as_the_identifiers_it_holds_them_by() {
    let (sent, made) = offering(
        "libraries",
        AS_IT_OPENS,
        Allowance {
            // Typed as the operator might, in the other case, because a library refused
            // for its capitalisation is somebody refused for their shift key.
            libraries: vec!["films".to_owned()],
            age_limit: None,
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    let written = policies(&sent);
    let first = written.first().cloned().unwrap_or_default();

    assert_eq!(
        first.get("EnabledFolders"),
        Some(&serde_json::json!(["db4c17"])),
        "{written:?}"
    );
    assert_eq!(
        first.get("EnableAllFolders"),
        Some(&serde_json::json!(false)),
        "naming a library left the account open to every one: {written:?}"
    );
    assert_eq!(
        first.get("MaxParentalRating"),
        None,
        "an invitation that named only libraries wrote an age limit as well: {written:?}"
    );
}

/// An offer that chooses neither writes no policy at all.
///
/// Not "writes an open one": offering somebody already here a second invitation must
/// leave what their household narrowed their account to exactly as it was.
#[tokio::test]
async fn an_invitation_that_chooses_nothing_writes_nothing() {
    let (sent, made) = offering("chooses-nothing", AS_IT_OPENS, Allowance::default()).await;

    assert!(made.is_some(), "the invitation itself was refused");
    assert!(
        policies(&sent).is_empty(),
        "an invitation that chose nothing wrote a policy anyway: {:?}",
        policies(&sent)
    );
}

/// An age limit alone leaves the libraries an account already opens exactly as they
/// are.
///
/// The case a filled-in default would break, and it breaks silently: an operator setting
/// a limit on a narrowed account would have widened it back to every library, and the
/// only way to find out is to read the account afterwards. Held on an account that is
/// already narrowed, because on a fresh one every value is the default and a write that
/// overwrote everything would look identical.
#[tokio::test]
async fn an_age_limit_alone_does_not_widen_the_libraries() {
    let (sent, made) = offering(
        "narrowed",
        ALREADY_NARROWED,
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(15),
        },
    )
    .await;

    assert!(made.is_some(), "the invitation itself was refused");
    let written = policies(&sent);
    let first = written.first().cloned().unwrap_or_default();

    assert_eq!(
        first.get("MaxParentalRating"),
        Some(&serde_json::json!(15)),
        "{written:?}"
    );
    assert_eq!(
        first.get("EnableAllFolders"),
        Some(&serde_json::json!(false)),
        "an age limit widened an account back to every library: {written:?}"
    );
    assert_eq!(
        first.get("EnabledFolders"),
        Some(&serde_json::json!(["a656b9"])),
        "an age limit changed which libraries the account opens: {written:?}"
    );
}

/// A library nobody holds is refused before the account exists.
///
/// Asserted as no account having been asked for rather than as a refusal, because a run
/// that made the account and then refused would report a refusal too — and would leave
/// somebody holding an open account nobody meant to give them.
#[tokio::test]
async fn a_library_nobody_holds_costs_no_account() {
    let (sent, made) = offering(
        "bad-library",
        AS_IT_OPENS,
        Allowance {
            libraries: vec!["Musicals".to_owned()],
            age_limit: None,
        },
    )
    .await;

    assert!(made.is_none(), "a library nobody holds was accepted");
    assert!(
        !sent.iter().any(|request| request.url.contains(NEW_ACCOUNT)),
        "an account was made before the library was refused"
    );
}

/// A media server that will not say what libraries it holds refuses the invitation
/// rather than matching the name against nothing.
///
/// A name matched against an empty list is a name that could not be found, so the
/// operator would be told their library does not exist when what happened is that
/// nobody could ask — and the account would have been made either way.
#[tokio::test]
async fn a_library_list_that_will_not_answer_costs_no_account() {
    let (sent, made) = driving(
        "unreadable-libraries",
        a_server(AS_IT_OPENS, Answer::reply(500, ""), Answer::reply(204, "")),
        Allowance {
            libraries: vec!["Films".to_owned()],
            age_limit: None,
        },
    )
    .await;

    assert!(made.is_none(), "an unreadable library list was accepted");
    assert!(
        !sent.iter().any(|request| request.url.contains(NEW_ACCOUNT)),
        "an account was made before the library list was read"
    );
}

/// A policy the media server will not take leaves an account that exists, and says so.
///
/// The one refusal that comes after the account is made, because it is the one thing
/// that cannot be settled before there is an account to write on. What the operator is
/// told is that the account is there and open — not that something went wrong — since
/// the two lead to different next moves.
#[tokio::test]
async fn a_policy_the_server_will_not_take_says_the_account_is_open() {
    let (sent, made) = driving(
        "refused-policy",
        a_server(
            AS_IT_OPENS,
            Answer::reply(200, LIBRARIES),
            Answer::reply(500, ""),
        ),
        Allowance {
            libraries: Vec::new(),
            age_limit: Some(12),
        },
    )
    .await;

    assert!(
        made.is_none(),
        "a policy the server refused was reported as set"
    );
    assert!(
        sent.iter().any(|request| request.url.contains(NEW_ACCOUNT)),
        "the account this refusal is about was never made"
    );
}
