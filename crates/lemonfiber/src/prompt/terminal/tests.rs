use std::path::{Path, PathBuf};

use lemonfiber_core::app::setup::{CredentialChoice, Prompt, ProviderEntry, StorageWarning};
use lemonfiber_core::config::Protocols;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::prerequisites::prerequisites;
use lemonfiber_core::validate::Validation;
use lemonfiber_core::wizard::{Answer, Indexer, Library, Plan, Provider, Wizard};

use super::{reviewed, Terminal, MASKED};
use crate::prompt::fixtures::{answered, answered_watching, wizard, Script};

/// Worth reading the first time and noise every time after — the rule the
/// footnote block follows inside one report, carried across the questions of one
/// setup rather than starting again at each.
#[test]
fn a_word_is_explained_once_in_one_conversation() {
    let terminal = answered(&[]);

    terminal.introduce("indexer");
    terminal.introduce("Indexer");
    terminal.introduce("a phrase this product does not explain");

    assert_eq!(
        terminal.met.borrow().as_slice(),
        ["indexer"],
        "said once, whatever case it was asked about, and nothing invented"
    );
}

/// Somebody who asked for none gets none, rather than a shorter version.
#[test]
fn a_run_that_wants_no_explanations_introduces_nothing() {
    let terminal = answered(&[]).explaining_nothing();

    terminal.introduce("indexer");

    assert!(terminal.met.borrow().is_empty(), "nothing was introduced");
}

/// The indexer credential the reviewable plan carries.
///
/// Named here, and asked about by name, rather than asked of `in_full`: that is
/// the decision under test, and a review checked against it would agree with it
/// however it answered — including when it answered that everything is safe.
const THE_KEY: &str = "the-key";

/// The provider credential it carries, named for the same reason.
const THE_LOGIN: &str = "the-login";

/// The settings of this plan whose values must not reach the review, by name.
///
/// `USENET_USER` is one of them and carries no word that reads as a credential:
/// a provider issues it, several issue an account number, and it is half of a
/// paid login. That is what the allow-list is for, and what a rule reading names
/// would miss.
const WITHHELD: [&str; 3] = ["INDEXER_APIKEY", "USENET_USER", "USENET_PASS"];

/// A plan carrying both kinds of credential this setup can hold — an indexer key
/// and a provider login — beside settings that are not credentials at all.
fn a_reviewable_plan() -> Plan {
    let mut wizard = wizard();
    let _ = wizard.answer(Answer::Protocols(Protocols::both()));
    let _ = wizard.answer(Answer::DataLocation(PathBuf::from("/srv/media")));
    let _ = wizard.answer(Answer::Credentials(Some(Indexer {
        url: "http://indexer.test".to_owned(),
        key: THE_KEY.to_owned(),
        validated: true,
    })));
    let _ = wizard.answer(Answer::Provider(Some(Provider {
        host: "news.test".to_owned(),
        port: 563,
        user: "me".to_owned(),
        pass: THE_LOGIN.to_owned(),
        tls: true,
        validated: true,
    })));
    wizard.plan()
}

#[test]
fn the_review_shows_each_setting_with_a_secret_marked_present_only() {
    // A review reaches the screen, scrollback and any session recording, so a
    // key has no business appearing in it — it is shown as present instead.
    let plan = a_reviewable_plan();
    assert!(
        !plan.settings().is_empty(),
        "the plan carries what was answered"
    );

    let review = reviewed(&plan).join("\n");

    assert!(
        review.contains("INDEXER_APIKEY = ********"),
        "the indexer key was not marked present only"
    );
    assert!(
        review.contains("USENET_PASS = ********"),
        "the provider password was not marked present only"
    );
    assert!(
        review.contains("USENET_USER = ********"),
        "the provider account was not marked present only"
    );
    assert!(
        !review.contains(THE_KEY),
        "the indexer key stands in the clear on the review"
    );
    assert!(
        !review.contains(THE_LOGIN),
        "the provider login stands in the clear on the review"
    );
    // And the settings somebody vouched for are shown as they will be written,
    // or a review that masked everything would say nothing at all.
    assert!(
        review.contains("INDEXER_URL = http://indexer.test"),
        "the indexer address was masked along with the key beside it"
    );
    assert!(
        review.contains("DATA_ROOT = /srv/media"),
        "where the files go was masked along with the credentials"
    );
    assert!(answered(&[""]).confirm(&plan));
}

#[test]
fn a_terminal_can_be_built_without_reaching_the_keyboard() {
    // The keyboard is only reached when a question is actually put, which is
    // why this is safe to build here and why nothing is asked of it: a real
    // question would read real input and the test would sit there forever.
    drop(Terminal::new(
        Environment::MacOs,
        PathBuf::from("/srv/media"),
    ));
}

#[test]
fn each_way_of_fetching_content_can_be_chosen() {
    for (answer, usenet, torrent) in [
        ("1", true, false),
        ("2", false, true),
        ("3", true, true),
        ("4", false, false),
        // Anything else takes the default, which is both.
        ("", true, true),
    ] {
        let chosen = answered(&[answer]).protocols();
        assert_eq!(
            chosen,
            Protocols { usenet, torrent },
            "answering {answer:?}"
        );
    }
}

/// A run with prerequisites waits on the operator; one with none does not.
///
/// The waiting is the half of this that can be held. A library-only run needs
/// nothing and is told so rather than shown an empty list — an end state, not a
/// lesser one — and keeping somebody at a prompt for it would be asking them to
/// acknowledge a list that is not there. What each item says is written to the
/// terminal, and reading this process's own stream back would be a harness.
#[test]
fn a_run_with_prerequisites_waits_on_the_operator_and_one_with_none_does_not() {
    let (terminal, unasked) = answered_watching(&[""]);
    terminal.prerequisites(&prerequisites(Protocols::none()));
    assert_eq!(
        unasked.borrow().len(),
        1,
        "nobody was kept waiting for an empty list"
    );

    let (terminal, unasked) = answered_watching(&[""]);
    terminal.prerequisites(&prerequisites(Protocols::both()));
    assert!(
        unasked.borrow().is_empty(),
        "the operator was shown a list and not waited on"
    );
}

#[test]
fn the_data_location_takes_the_default_when_it_is_not_named() {
    assert_eq!(answered(&[""]).data_location(), PathBuf::from("/srv/media"));
    assert_eq!(
        answered(&["/mnt/big"]).data_location(),
        PathBuf::from("/mnt/big")
    );
}

/// Explaining hardlinking is telling, never asking, whichever way it was found.
///
/// Both branches are reached so neither can rot, and the property held is that
/// the operator is not stopped for either: an explanation that asked a question
/// would put a prompt in the middle of a walk that is not asking anything.
#[test]
fn explaining_hardlinking_asks_the_operator_nothing_either_way() {
    let (terminal, unasked) = answered_watching(&["unused"]);
    // Proven on the location itself.
    terminal.hardlinks(Path::new("/srv/media"), None);
    // Inferred from the parent, and said to be inferred.
    terminal.hardlinks(Path::new("/srv/media"), Some(Path::new("/srv")));
    assert_eq!(unasked.borrow().len(), 1, "nothing was asked");
}

#[test]
fn a_location_that_cannot_hardlink_is_explained_and_still_offered() {
    // Defaulting to no, so the operator is nudged toward one that links —
    // without the choice being taken away.
    assert!(!answered(&[""]).storage_warning(
        Path::new("/srv/media"),
        &StorageWarning::CopyOnly {
            limitation: Some("it is a network share".to_owned())
        }
    ));
    assert!(answered(&["y"]).storage_warning(
        Path::new("/srv/media"),
        &StorageWarning::CopyOnly { limitation: None }
    ));
    // One that could not be tested is a different sentence.
    assert!(!answered(&["n"]).storage_warning(
        Path::new("/srv/media"),
        &StorageWarning::Untested {
            reason: "the path does not exist".to_owned()
        }
    ));
}

#[test]
fn a_blank_indexer_url_sets_none_up_at_all() {
    assert_eq!(answered(&[""]).credential(), None);
    assert_eq!(
        answered(&["http://indexer.test", "the-key"]).credential(),
        Some(("http://indexer.test".to_owned(), "the-key".to_owned()))
    );
}

#[test]
fn a_credential_that_did_not_prove_offers_the_three_ways_out() {
    let rejected = Validation::Rejected {
        detail: "401".to_owned(),
    };
    assert!(matches!(
        answered(&["1"]).credential_failed(&rejected),
        CredentialChoice::Retry
    ));
    assert!(matches!(
        answered(&["2"]).credential_failed(&rejected),
        CredentialChoice::Proceed
    ));
    assert!(matches!(
        answered(&["3"]).credential_failed(&rejected),
        CredentialChoice::Skip
    ));
    // Each cause is named as itself, because their remedies differ.
    for outcome in [
        Validation::Unreachable {
            detail: "no answer".to_owned(),
        },
        Validation::Degraded {
            detail: "no search capability".to_owned(),
        },
        Validation::Valid {
            observed: "Prowlarr".to_owned(),
        },
    ] {
        let _ = answered(&["1"]).credential_failed(&outcome);
    }
    // And one that proved is simply said so.
    answered(&[]).credential_valid("Prowlarr 1.2");
}

#[test]
fn a_blank_provider_host_sets_none_up_at_all() {
    assert_eq!(answered(&[""]).usenet_provider(), None);
}

#[test]
fn a_provider_takes_the_standard_port_and_tls_unless_told_otherwise() {
    let entry = answered(&["news.test", "", "me", "secret", ""]).usenet_provider();
    assert_eq!(
        entry,
        Some(ProviderEntry {
            host: "news.test".to_owned(),
            port: 563,
            user: "me".to_owned(),
            pass: "secret".to_owned(),
            tls: true,
        })
    );
    // Named otherwise, both are taken as given.
    let plain = answered(&["news.test", "119", "me", "secret", "n"]).usenet_provider();
    assert!(plain.is_some_and(|entry| entry.port == 119 && !entry.tls));
}

#[test]
fn the_library_choice_offers_the_native_option_only_where_it_applies() {
    assert!(matches!(
        answered(&["1"]).library(),
        Library::JellyfinDocker
    ));
    assert!(matches!(answered(&["3"]).library(), Library::None));
    // macOS offers a native media server, so choosing it is possible.
    assert!(matches!(
        answered(&["2"]).library(),
        Library::JellyfinNative
    ));
    // Where it is not offered, the same answer falls back rather than taking a
    // choice this platform never showed.
    let linux = Terminal::answered_by(
        Environment::LinuxNative,
        PathBuf::from("/srv/media"),
        Box::new(Script::of(&["2"])),
    );
    assert!(matches!(linux.library(), Library::JellyfinDocker));
}

#[test]
fn the_yes_or_no_questions_take_their_own_defaults() {
    // Household defaults to no, autostart to no; a bare enter takes each.
    assert!(!answered(&[""]).household());
    assert!(answered(&["yes"]).household());
    // Autostart keeps its no even though what precedes it now says what a no
    // costs. The sentence is there to make the answer informed, not to push it:
    // a laptop that is not always on has no business starting a media stack at
    // login, and that operator presses enter.
    assert!(!answered(&[""]).autostart());
    assert!(answered(&["y"]).autostart());
    // An answer that is neither takes the default.
    assert!(!answered(&["maybe"]).household());
}

#[test]
fn the_vpn_question_defaults_to_yes_and_its_warning_defaults_to_no() {
    // The checklist has just said what a VPN is for, so yes is the answer that
    // follows from what they were told and a bare enter takes it.
    assert!(answered(&[""]).vpn());
    assert!(!answered(&["no"]).vpn());

    // The warning is the other way round: pressing enter goes back to the
    // question rather than past the exposure. Going on has to be typed.
    assert!(!answered(&[""]).unprotected());
    assert!(answered(&["yes"]).unprotected());
}

#[test]
fn the_review_shows_every_setting_and_never_a_secret_in_the_clear() {
    let plan = a_reviewable_plan();

    let review = reviewed(&plan);

    // Every setting the plan carries is on the review, shown as it will be
    // written — except the two that carry a credential, which are shown as
    // present and nothing more. One left off would be written without ever
    // having been shown.
    assert_eq!(review.len(), plan.settings().len());
    for (key, value) in plan.settings() {
        let shown = if WITHHELD.contains(&key.as_str()) {
            MASKED
        } else {
            value.as_str()
        };
        assert!(
            review.contains(&format!("  {key} = {shown}")),
            "{key} is shown the wrong way on the review"
        );
    }
    // And neither credential reaches the review by any other route.
    let said = review.join("\n");
    for credential in [THE_KEY, THE_LOGIN] {
        assert!(
            !said.contains(credential),
            "a credential reached the review by another route"
        );
    }

    // Confirmed by default: a bare enter applies.
    let empty = Wizard::new(Environment::MacOs).plan();
    assert!(answered(&[""]).confirm(&empty));
    assert!(!answered(&["n"]).confirm(&empty));
}
