//! The household's own ntfy signed in to with its plugin's account, and no other server
//! ever handed that account.

use super::reached_with;
use crate::app::Ctx;
use crate::model::{BackendKind, BackendProof, ChosenBy};
use crate::notify::backends::{Backend, Held};
use crate::settings::{Keeping, Source};
use crate::test_support::{a_placed, an_installed};

/// The port this machine publishes the plugin's ntfy on.
const PUBLISHED: u16 = 2586;

/// The password the plugin's account was minted with.
const PASSWORD: &str = "minted-for-the-household-ntfy";

/// A machine running the ntfy plugin's service on [`PUBLISHED`], its password kept where
/// `kept`.
fn a_machine(named: &str, kept: bool) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("bundled-{named}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    lemonfiber_fixtures::stack::manifest_without(&dir, "seerr");
    let mut ctx = crate::test_support::a_context()
        .over(crate::stack::Source::External(Box::leak(
            dir.clone().into_boxed_path(),
        )))
        .build();
    ctx.settings.env_file = Some(dir.join(".env"));
    let mut register = crate::plugin::Register::empty();
    let _ = register.record(an_installed(
        "ntfy",
        vec![a_placed("ntfy", &[], Some(PUBLISHED))],
    ));
    if let Some(at) = crate::app::plugins::kept_at(&ctx) {
        let _ = std::fs::write(at, register.to_json().unwrap_or_default());
    }
    if kept {
        if let Some(secrets) = crate::settings::secrets_at(&ctx) {
            let name = crate::settings::named("ntfy", "password");
            let _ = crate::settings::kept(
                &secrets,
                &[Keeping {
                    name: &name,
                    value: PASSWORD,
                    from: Source::Minted,
                    at: 0,
                }],
                ctx.seams.random.as_ref(),
                &|_| Ok(()),
            );
        }
    }
    ctx
}

/// A back-end speaking `kind` at `url`.
fn backend(kind: BackendKind, url: &str) -> Backend {
    Backend {
        id: "home".to_owned(),
        kind,
        url: url.to_owned(),
        topic: Some("stack".to_owned()),
        chosen_by: ChosenBy::Operator,
        hosted: false,
        members: None,
        receivers: Vec::new(),
        attempts: 3,
        delays: vec![1],
        proof: BackendProof::Unproven,
        testing: None,
        last: None,
        failures: Vec::new(),
        retry: None,
        owed: Vec::new(),
        added: 0,
    }
}

/// The token `backend` is reached with on `ctx` where nothing was handed over for it.
fn token(ctx: &Ctx, backend: &Backend) -> Option<String> {
    reached_with(ctx, backend, Held::default()).token
}

#[test]
fn the_households_own_ntfy_is_signed_in_to_as_its_plugins_account() {
    let ctx = a_machine("own", true);

    let signed = token(&ctx, &backend(BackendKind::Ntfy, "http://127.0.0.1:2586"));
    let named = token(&ctx, &backend(BackendKind::Ntfy, "http://localhost:2586"));
    let six = token(&ctx, &backend(BackendKind::Ntfy, "http://[::1]:2586"));

    assert_eq!(signed, Some(format!("lemonfiber:{PASSWORD}")));
    assert_eq!(named, signed);
    assert_eq!(six, signed);
}

#[test]
fn a_token_handed_over_is_the_one_used() {
    let ctx = a_machine("handed", true);
    let held = Held {
        token: Some("tk_operator".to_owned()),
        reader: None,
    };

    let used = reached_with(
        &ctx,
        &backend(BackendKind::Ntfy, "http://127.0.0.1:2586"),
        held.clone(),
    );

    assert_eq!(used, held);
}

#[test]
fn no_other_server_is_handed_the_account() {
    let ctx = a_machine("elsewhere", true);

    let handed: Vec<Option<String>> = [
        backend(BackendKind::Ntfy, "https://ntfy.sh"),
        backend(BackendKind::Ntfy, "http://192.168.1.20:2586"),
        backend(BackendKind::Ntfy, "http://127.0.0.1:8080"),
        backend(BackendKind::Webhook, "http://127.0.0.1:2586"),
        backend(BackendKind::Ntfy, "not an address"),
        backend(BackendKind::Ntfy, "unix:/run/ntfy.sock"),
        backend(BackendKind::Ntfy, "http://[2001:db8::1]:2586"),
    ]
    .iter()
    .map(|one| token(&ctx, one))
    .collect();

    assert_eq!(handed, [None, None, None, None, None, None, None]);
}

#[test]
fn with_no_password_kept_nothing_is_signed_in_with() {
    let ctx = a_machine("unkept", false);

    assert_eq!(
        token(&ctx, &backend(BackendKind::Ntfy, "http://127.0.0.1:2586")),
        None
    );
}

#[test]
fn a_stack_that_cannot_be_read_hands_the_account_to_nobody() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("bundled-unread").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let ctx = crate::test_support::a_context()
        .over(crate::stack::Source::External(Box::leak(
            dir.into_boxed_path(),
        )))
        .build();

    assert_eq!(
        token(&ctx, &backend(BackendKind::Ntfy, "http://127.0.0.1:2586")),
        None
    );
}
