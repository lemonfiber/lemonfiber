use super::{carried, KEY_CALLABLE, OFFERED};
use crate::actions::{Arguments, TAKES_AGREEMENT};
use crate::entitled::callable_by_a_key;

/// A carrier holding something for every argument any action reads, agreed to or not.
///
/// Full on purpose, and only for the one question below: whether an action's command
/// changes with `confirm`. An action reading an argument it was not handed would come
/// to the same command either way and say nothing, so every argument is handed.
fn everything(confirm: bool) -> Arguments {
    Arguments {
        forms: vec!["tv".to_owned()],
        services: vec!["sonarr".to_owned()],
        wait: true.into(),
        service: Some("sonarr".to_owned()),
        name: Some("ana".to_owned()),
        libraries: vec!["Films".to_owned()],
        age_limit: Some(12),
        unrated: Some("allow".to_owned()),
        key: Some("DATA_ROOT".to_owned()),
        value: Some("/srv".to_owned()),
        preset: Some("balanced".to_owned()),
        media_type: Some("tv".to_owned()),
        archive: Some("lemonfiber-full-1700000000.tar.gz".to_owned()),
        at: Some("1700000000".to_owned()),
        repoint: true,
        write: true,
        logs: Some(12),
        filenames: true.into(),
        reveal: vec!["INDEXER_KEY".to_owned()],
        only: Some("vpn.killswitch".to_owned()),
        check: Some("vpn.unprotected".to_owned()),
        disruptive: true.into(),
        offer: Some("0f0f0f0f".to_owned()),
        agreed: vec!["vpn.unprotected".to_owned()],
        confirm,
        item: Some("Sintel".to_owned()),
        term: Some("The Expanse".to_owned()),
        season: Some(2),
        download: Some("A.Show.S01E01.1080p".to_owned()),
        policy: Some("within-a-limit".to_owned()),
        requests: Some(5),
        days: Some(30),
        request: Some(7),
        reason: Some("there is no room this month".to_owned()),
        capability: Some("indexer.search".to_owned()),
        plugin: Some("komga".to_owned()),
        source: Some("./plugins/komga".to_owned()),
        approved: vec!["token@metadata.example.org".to_owned()],
        tier: Some("services".to_owned()),
        kept: Some("watch".to_owned()),
        down: Some("37%".to_owned()),
        up: Some("37%".to_owned()),
        active: Some("06:30-22:45".to_owned()),
        line: Some("60MiB/6MiB".to_owned()),
        cap: Some("1TiB".to_owned()),
        exceeded: Some("pause".to_owned()),
        unrestricted_for: Some(37),
        dry_run: true.into(),
    }
}

/// An action whose command changes with `confirm` reads it, and one the table does not
/// list as taking it is refused before it gets to — which is how every migration was
/// refusing the yes it carried out on. So every action that reads it is listed.
#[test]
fn every_action_that_reads_the_agreement_is_listed_as_taking_it() {
    let mut reading: Vec<&str> = Vec::new();
    let mut unanswered: Vec<&str> = Vec::new();
    for action in OFFERED {
        let without = carried(action, everything(false));
        let with = carried(action, everything(true));
        if without.is_err() && with.is_err() {
            unanswered.push(action);
        }
        if without != with {
            reading.push(action);
        }
    }
    // An action refusing a full carrier agreed to or not tells this nothing about
    // itself, so none may. One refused only without the yes has said it reads it.
    assert_eq!(
        unanswered,
        Vec::<&str>::new(),
        "refused a full carrier either way"
    );
    let unlisted: Vec<&&str> = reading
        .iter()
        .filter(|action| !TAKES_AGREEMENT.contains(action))
        .collect();
    assert_eq!(
        unlisted,
        Vec::<&&str>::new(),
        "these read `confirm`, and the table refuses it before they can"
    );
    assert!(
        reading.contains(&"migrate-adopt") && reading.contains(&"reset"),
        "and the sweep sees the reads it exists to find: {reading:?}"
    );
}

/// The published list and the rule that decides are the same list.
///
/// Every action is reached from nothing, from a full carrier and from a full carrier
/// agreed to, and is callable by a key if any of those reaches a command the rule lets
/// a key call. A name published that no key could call would send a program to a
/// refusal; an action a key can call that is not published is one an operator was
/// never told about.
#[test]
fn what_a_key_may_call_is_exactly_what_is_published() {
    let callable: Vec<&str> = OFFERED
        .iter()
        .copied()
        .filter(|action| {
            [Arguments::default(), everything(false), everything(true)]
                .into_iter()
                .filter_map(|given| carried(action, given).ok())
                .any(|command| callable_by_a_key(&command))
        })
        .collect();
    let mut published: Vec<&str> = KEY_CALLABLE.iter().map(|by| by.action).collect();
    let mut reached = callable.clone();
    published.sort_unstable();
    reached.sort_unstable();
    assert_eq!(reached, published);
}

/// A key may not accept a disturbing check's offer: only the operator says yes to
/// taking the system down, even where diagnosing it is a key's to ask.
#[test]
fn a_key_may_diagnose_but_not_accept_what_a_diagnosis_offers() {
    let accepting = carried("accept", everything(true));
    assert!(accepting.is_ok_and(|command| !callable_by_a_key(&command)));
    let widened = Arguments {
        disruptive: true.into(),
        ..Arguments::default()
    };
    let diagnosing = carried("diagnose", widened);
    assert!(diagnosing.is_ok_and(|command| callable_by_a_key(&command)));
}
