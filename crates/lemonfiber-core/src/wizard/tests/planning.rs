//! What a reviewed setup plans from the answers it gathered.

use super::*;
use crate::wizard::ContainerUser;

#[test]
fn a_reviewed_wizard_plans_every_setting_it_gathered() {
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "LEMONFIBER_USENET"), Some("on"));
    assert_eq!(setting(&plan, "LEMONFIBER_TORRENT"), Some("on"));
    assert_eq!(setting(&plan, "DATA_ROOT"), Some("/srv/media"));
    assert_eq!(setting(&plan, "PUID"), Some("1000"));
    // Distinct from PUID, so a swapped mapping would not pass unnoticed.
    assert_eq!(setting(&plan, "PGID"), Some("1001"));
    assert_eq!(setting(&plan, "MEDIA_SERVER_MODE"), Some("docker"));
}

/// Every answer setup writes has a row saying what changing it costs.
///
/// What holds the catalogue to the wizard. A setting setup starts writing without
/// one is a decision the operator can make and cannot be told the price of, which
/// is the trap A4 exists to close — and it would be closed silently, since nothing
/// else notices a missing row.
#[test]
fn every_answer_setup_writes_is_catalogued_as_a_decision() {
    // Not decisions: nobody chooses these. They record what proving the credential
    // came to, so there is no cost to state for changing one.
    const RECORDED_RATHER_THAN_CHOSEN: [&str; 2] = [
        crate::config::INDEXER_VALIDATED_KEY,
        crate::config::PROVIDER_VALIDATED_KEY,
    ];

    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    wizard
        .answer(Answer::Credentials(Some(super::super::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        })))
        .unwrap_or(());
    wizard
        .answer(Answer::Provider(Some(super::super::Provider {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "the-login".to_owned(),
            tls: true,
            validated: true,
        })))
        .unwrap_or(());

    let plan = wizard.plan();
    let unpriced: Vec<&str> = plan
        .settings()
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !RECORDED_RATHER_THAN_CHOSEN.contains(name))
        .filter(|name| crate::reconfigure::decision(name).is_none())
        .collect();
    assert!(
        unpriced.is_empty(),
        "setup writes these and nothing says what changing them costs: {unpriced:?}"
    );
    // An empty plan would pass the filter above without proving anything.
    assert!(
        plan.settings().len() > 10,
        "a plan this small is not exercising the writer"
    );
}

#[test]
fn every_setting_a_plan_writes_is_one_lemonfiber_declares() {
    // What holds the writer to `config::SETTINGS`. That list is what the guard on
    // withholding reads to know which settings exist at all, and a key written here
    // and absent there is a setting served to a browser that no guard has looked at.
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    wizard
        .answer(Answer::Credentials(Some(super::super::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        })))
        .unwrap_or(());
    wizard
        .answer(Answer::Provider(Some(super::super::Provider {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "the-login".to_owned(),
            tls: true,
            validated: true,
        })))
        .unwrap_or(());

    let plan = wizard.plan();
    let undeclared: Vec<&str> = plan
        .settings()
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !crate::config::SETTINGS.contains(name))
        .collect();
    assert!(
        undeclared.is_empty(),
        "setup writes these and config::SETTINGS does not name them, so nothing \
         checks whether they are displayed: {undeclared:?}"
    );
    assert!(
        plan.settings().len() > 10,
        "a plan this small is not exercising the writer"
    );
}

#[test]
fn every_setting_a_step_names_is_one_the_plan_it_belongs_to_writes() {
    // The other end of the same loop. `Step::settings` is what reconfiguration
    // reads to say which credentials adding a protocol opens, and a key named
    // there that setup never writes would send an operator looking for a question
    // this product does not ask.
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    wizard
        .answer(Answer::Credentials(Some(super::super::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        })))
        .unwrap_or(());
    wizard
        .answer(Answer::Provider(Some(super::super::Provider {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "the-login".to_owned(),
            tls: true,
            validated: true,
        })))
        .unwrap_or(());

    let plan = wizard.plan();
    let written: Vec<&str> = plan
        .settings()
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let named: Vec<&str> = Step::ORDER
        .into_iter()
        .flat_map(|step| step.settings().iter().copied())
        .collect();
    assert!(named.len() > 10, "the steps name too little to be a loop");
    let unwritten: Vec<&&str> = named.iter().filter(|key| !written.contains(key)).collect();
    assert!(
        unwritten.is_empty(),
        "these steps name settings setup never writes: {unwritten:?}"
    );
}

#[test]
fn a_given_indexer_is_planned_but_a_stale_one_is_dropped_when_it_no_longer_applies() {
    let indexer = Answer::Credentials(Some(super::super::Indexer {
        url: "http://indexer.test/api".to_owned(),
        key: "the-key".to_owned(),
        validated: true,
    }));

    // Chosen with a download protocol, the indexer is written.
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard.answer(indexer.clone()).unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(
        setting(&plan, "INDEXER_URL"),
        Some("http://indexer.test/api")
    );
    assert_eq!(setting(&plan, "INDEXER_APIKEY"), Some("the-key"));
    assert_eq!(setting(&plan, "INDEXER_VALIDATED"), Some("on"));

    // Then neither protocol is chosen, so the step no longer applies. The
    // answer lingers, but its key must not be written for a stack with no
    // service to use it.
    wizard
        .answer(Answer::Protocols(Protocols::none()))
        .unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "INDEXER_URL"), None);
    assert_eq!(
        setting(&plan, "INDEXER_APIKEY"),
        None,
        "no stale key is written"
    );
}

#[test]
fn a_given_provider_is_planned_but_dropped_when_usenet_is_no_longer_chosen() {
    let provider = Answer::Provider(Some(super::super::Provider {
        host: "news.provider.test".to_owned(),
        port: 563,
        user: "person".to_owned(),
        pass: "secret".to_owned(),
        tls: true,
        validated: true,
    }));

    // Chosen with Usenet, the provider login is written.
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard.answer(provider.clone()).unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "USENET_HOST"), Some("news.provider.test"));
    assert_eq!(setting(&plan, "USENET_PORT"), Some("563"));
    assert_eq!(setting(&plan, "USENET_USER"), Some("person"));
    assert_eq!(setting(&plan, "USENET_VALIDATED"), Some("on"));

    // Then torrents only, so the provider step no longer applies; its login,
    // password and all, must not be written for a stack that will not use it.
    wizard
        .answer(Answer::Protocols(Protocols {
            usenet: false,
            torrent: true,
        }))
        .unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "USENET_HOST"), None);
    assert_eq!(
        setting(&plan, "USENET_PASS"),
        None,
        "no stale password is written"
    );
}

#[test]
fn the_vpn_answer_is_written_while_torrents_are_chosen_and_dropped_when_they_are_not() {
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard.answer(Answer::Vpn(Vpn::Absent)).unwrap_or(());
    assert_eq!(setting(&wizard.plan(), "LEMONFIBER_VPN"), Some("off"));

    wizard
        .answer(Answer::Vpn(Vpn::Carrying(Guarded::default())))
        .unwrap_or(());
    assert_eq!(setting(&wizard.plan(), "LEMONFIBER_VPN"), Some("on"));

    wizard
        .answer(Answer::Protocols(Protocols {
            usenet: true,
            torrent: false,
        }))
        .unwrap_or(());
    assert_eq!(setting(&wizard.plan(), "LEMONFIBER_VPN"), None);
}

#[test]
fn an_unanswered_question_contributes_no_setting() {
    // A fresh wizard writes only the house's zone and locale, which stand as proposed;
    // a partly answered one writes only what it has, never a guessed default for what it
    // does not.
    assert_eq!(
        on_native_linux()
            .plan()
            .settings()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["TZ", "LEMONFIBER_LOCALE"]
    );

    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols {
            usenet: true,
            torrent: false,
        }))
        .unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "LEMONFIBER_USENET"), Some("on"));
    // A declined protocol is written off, not omitted.
    assert_eq!(setting(&plan, "LEMONFIBER_TORRENT"), Some("off"));
    assert_eq!(setting(&plan, "DATA_ROOT"), None);
    assert_eq!(setting(&plan, "MEDIA_SERVER_MODE"), None);
}

#[test]
fn a_declined_container_user_writes_no_ids() {
    let mut wizard = on_native_linux();
    wizard.answer(Answer::ServiceUser(None)).unwrap_or(());
    // The step is answered — recorded as "no id" rather than left open — and
    // that answered-with-nothing still writes neither id.
    assert_eq!(wizard.answers().service_user, ContainerUser::Kept);
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "PUID"), None);
    assert_eq!(setting(&plan, "PGID"), None);
}

#[test]
fn the_library_choice_maps_to_its_mode_or_to_nothing() {
    let mut docker = on_native_linux();
    docker
        .answer(Answer::Library(Library::Docker))
        .unwrap_or(());
    assert_eq!(setting(&docker.plan(), "MEDIA_SERVER_MODE"), Some("docker"));

    let mut native = on_macos();
    native
        .answer(Answer::Library(Library::Native))
        .unwrap_or(());
    assert_eq!(setting(&native.plan(), "MEDIA_SERVER_MODE"), Some("native"));

    let mut none = on_native_linux();
    none.answer(Answer::Library(Library::None)).unwrap_or(());
    assert_eq!(setting(&none.plan(), "MEDIA_SERVER_MODE"), None);
}

#[test]
fn the_house_s_zone_and_locale_are_planned_as_proposed_until_answered() {
    let proposed = super::super::House {
        zone: crate::zone::Zone::proposed(Some("Asia/Tokyo")),
        locale: crate::locale::Locale::proposed(Some("ja_JP.UTF-8")),
    };
    let mut wizard = on_native_linux().proposing(proposed.clone());
    assert_eq!(wizard.house(), &proposed);
    assert_eq!(setting(&wizard.plan(), "TZ"), Some("Asia/Tokyo"));
    assert_eq!(setting(&wizard.plan(), "LEMONFIBER_LOCALE"), Some("ja-JP"));
    assert!(!wizard.unanswered().contains(&Step::Preflight));

    let given = super::super::House {
        zone: crate::zone::Zone::proposed(Some("America/Chicago")),
        locale: crate::locale::Locale::proposed(Some("en_US")),
    };
    assert!(wizard.answer(Answer::House(given.clone())).is_ok());
    assert_eq!(wizard.house(), &given);
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "TZ"), Some("America/Chicago"));
    assert_eq!(setting(&plan, "LEMONFIBER_LOCALE"), Some("en-US"));
    assert!(plan.written().all(|(key, _)| key != "LEMONFIBER_LOCALE"));
    assert_eq!(
        plan.stored().cloned().collect::<Vec<_>>(),
        [("LEMONFIBER_LOCALE".to_owned(), "en-US".to_owned())]
    );
    assert!(Step::Preflight.is_question());
}
