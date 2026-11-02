use std::cell::RefCell;

use lemonfiber_core::app::{offline, Ctx, InstallOffered};

use super::{agreed, answered, is_yes, Agreed, ASK_TO_INSTALL};

/// Only an explicit yes installs; a no, a stray return and any other word start nothing.
#[test]
fn only_an_explicit_yes_installs() {
    for yes in ["y", "Y", "yes", " Yes \n"] {
        assert!(is_yes(yes), "{yes:?}");
    }
    for no in ["", "n", "no", "sure", "\n"] {
        assert!(!is_yes(no), "{no:?}");
    }
    assert!(ASK_TO_INSTALL.ends_with("[y/N]"), "{ASK_TO_INSTALL}");
}

/// The form these tests start, which holds parts this build installs.
fn library() -> Vec<String> {
    vec!["library".to_owned()]
}

/// A machine with lemonfiber's own files set up beneath `name` that has never run a part,
/// so a start of the library form offers to install each one.
fn never_ran(name: &str) -> Ctx {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&dir);
    lemonfiber_testing::a_context()
        .settings(lemonfiber_core::config::Settings {
            env_file: Some(dir.join("config").join(".env")),
            stack_dir: Some(dir.join("data").join("stack")),
            ..lemonfiber_core::config::Settings::default()
        })
        .build()
}

/// What `agreed` came to over the library form, answered with `says`, and every question
/// it put.
async fn asked(ctx: &Ctx, interactive: bool, says: &str) -> (Option<Agreed>, Vec<String>) {
    let put = RefCell::new(Vec::new());
    let ask = |question: &str| {
        put.borrow_mut().push(question.to_owned());
        says.to_owned()
    };
    let agreed = agreed(ctx, &library(), interactive, &ask).await.ok();
    (agreed, put.into_inner())
}

/// Something on offer with nobody to ask is not agreed to, and nothing is asked.
#[tokio::test]
async fn an_install_on_offer_with_nobody_to_ask_is_not_agreed() {
    let ctx = never_ran("installing-unasked");

    let (agreed, put) = asked(&ctx, false, "y").await;

    assert_eq!(agreed, Some(Agreed::No));
    assert!(put.is_empty(), "{put:?}");
}

/// An install on offer is put to the operator, and a yes answers the offer standing now
/// while a no answers none.
#[tokio::test]
async fn an_install_on_offer_is_asked_and_a_yes_answers_its_offer() {
    let ctx = never_ran("installing-asked");
    let standing = offline(&ctx, &library())
        .await
        .ok()
        .flatten()
        .map(|offline| offline.offer);
    assert!(standing.is_some());

    let (yes, put) = asked(&ctx, true, "y").await;
    let (no, _) = asked(&ctx, true, "n").await;

    assert_eq!(yes, standing.clone().map(Agreed::Yes));
    assert_eq!(put, [ASK_TO_INSTALL]);
    assert_eq!(no, Some(Agreed::No));
    assert_eq!(
        answered(&ctx, &library(), true, &|_| "yes".to_owned()).await,
        standing
    );
    assert_eq!(
        answered(&ctx, &library(), true, &|_| String::new()).await,
        None
    );
}

/// Forms that do not resolve answer no offer, so the start refuses them in its own
/// pre-flight.
#[tokio::test]
async fn forms_that_do_not_resolve_answer_no_offer() {
    let ctx = never_ran("installing-unresolved");

    let offer = answered(&ctx, &["no-such-form".to_owned()], true, &|_| {
        "y".to_owned()
    })
    .await;

    assert_eq!(offer, None);
}

/// An install whose reading could not be made says why, and the offer names the command
/// that answers it.
#[test]
fn an_install_that_could_not_be_read_says_why() {
    let said = crate::render::moving::installing(
        &[InstallOffered {
            plugin: "jellyfin".to_owned(),
            offer: None,
            why: Some("its tree could not be written".to_owned()),
        }],
        Some(("up", "the-offer")),
    )
    .text();

    assert!(
        said.contains("runs jellyfin as a plugin, and its install could not be read: its tree could not be written"),
        "{said}"
    );
    assert!(
        said.contains(
            "Run the same `lemonfiber up` with `--offer the-offer` to install and start."
        ),
        "{said}"
    );
}
